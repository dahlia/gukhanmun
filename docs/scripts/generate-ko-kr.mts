// Gukhanmun: Generates the hangul-only (ko-KR) docs locale from the ko-Kore source.
// Copyright (C) 2026  Hong Minhee
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

// Run via `mise run docs-ko-kr`, which first builds the release `gukhanmun`
// CLI. The hangul-only Korean locale (ko-KR) is never hand-written: it is a
// projection of the mixed-script ko-Kore tree produced by the project's own
// converter, so editors only ever touch en/ and ko-Kore/. The output tree is
// gitignored. We drive the CLI rather than @gukhanmun/napi because only the
// CLI converts selected YAML front matter values (hero tagline, page titles),
// which every doc relies on. Command and filesystem plumbing uses dax
// (https://dax.land/).

import $ from "dax";
import process from "node:process";
import { fileURLToPath } from "node:url";

type Path = ReturnType<typeof $.path>;

const here = $.path(fileURLToPath(import.meta.url)).parentOrThrow();
const docsDir = here.parentOrThrow();
const workspaceRoot = docsDir.parentOrThrow();
const bin = workspaceRoot.join("target", "release", "gukhanmun");
const srcDir = docsDir.join("ko-Kore");
const outDir = docsDir.join("ko-KR");

// GFM everywhere: the JavaScript install page uses a table, and the variant is
// a no-op for files without GFM syntax.
const MARKDOWN_FORMAT = "text/markdown; variant=GFM";

if (!bin.existsSync()) {
  console.error(
    `gukhanmun CLI not found at ${bin}.\n` +
      "Build it first: cargo build --release -p gukhanmun-cli",
  );
  process.exit(1);
}

// ── Plain-text labels (nav/sidebar) ───────────────────────────────────────────

// Convert one UI label as its own plain-text document. Per-string conversion
// keeps each label in its own homophone window, so labels never gloss each
// other; the CLI is fast enough (~2 ms) that a cache is the only optimisation
// worth making.
const plainCache = new Map<string, string>();
async function convertPlain(text: string): Promise<string> {
  const cached = plainCache.get(text);
  if (cached !== undefined) return cached;
  const converted = await $`${bin} --format text/plain`.stdinText(text).text();
  plainCache.set(text, converted);
  return converted;
}

type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

// Recursively convert every `text`/`label` string (the only human-facing fields
// in _nav.json / _meta.json); `name`, `link`, `type`, etc. pass through.
async function convertMetaJson(node: Json): Promise<Json> {
  if (Array.isArray(node)) {
    const out: Json[] = [];
    for (const item of node) out.push(await convertMetaJson(item));
    return out;
  }
  if (node !== null && typeof node === "object") {
    const out: { [key: string]: Json } = {};
    for (const [key, value] of Object.entries(node)) {
      out[key] = (key === "text" || key === "label") && typeof value === "string"
        ? await convertPlain(value)
        : await convertMetaJson(value);
    }
    return out;
  }
  return node;
}

// ── Markdown / MDX ─────────────────────────────────────────────────────────────

function splitFrontmatter(src: string): { frontmatter: string; body: string } {
  const match = src.match(/^(---\r?\n[\s\S]*?\r?\n---\r?\n?)([\s\S]*)$/);
  return match ? { frontmatter: match[1], body: match[2] } : { frontmatter: "", body: src };
}

// The home page carries its prose in front matter (hero + feature cards); every
// other page only has a title/description. Selecting absent keys would warn, so
// we probe the raw block before adding the title/description selectors.
function frontmatterSelectors(relPosix: string, frontmatter: string): string[] {
  if (relPosix === "index.md") {
    return [
      "$.hero.tagline",
      "$.hero.actions[*].text",
      "$.features[*].title",
      "$.features[*].details",
    ];
  }
  const selectors: string[] = [];
  if (/^title:/m.test(frontmatter)) selectors.push("$.title");
  if (/^description:/m.test(frontmatter)) selectors.push("$.description");
  return selectors;
}

// MDX-only constructs (ESM import/export statements and JSX element blocks) are
// not CommonMark; the converter would reflow or escape them. The docs keep each
// such construct in its own blank-line-delimited block, so we swap each one for
// a sentinel-word placeholder and restore it afterwards. The word survives the
// markdown round-trip untouched (no hanja, no markdown-special characters) and,
// being a paragraph, keeps the blank-line separation between adjacent blocks
// (an HTML-comment placeholder would get collapsed). Prose, headings, and
// tables in between are converted normally.
const ESM_RE = /^\s*(?:import|export)\s/;
const JSX_RE = /^\s*</;
const PLACEHOLDER_RE = /GUKHANMUNMDX(\d+)PLACEHOLDER/g;

function maskMdx(body: string): { masked: string; stash: string[] } {
  const stash: string[] = [];
  const masked = body.split(/\n{2,}/).map((block) => {
    // Trim so a block's stray leading/trailing newlines (e.g. the blank line
    // after front matter) do not leak back in on restore; internal indentation
    // of multi-line JSX is preserved.
    const content = block.trim();
    if (content === "") return block;
    if (ESM_RE.test(content) || JSX_RE.test(content)) {
      const token = `GUKHANMUNMDX${stash.length}PLACEHOLDER`;
      stash.push(content);
      return token;
    }
    return block;
  });
  return { masked: masked.join("\n\n"), stash };
}

function unmaskMdx(out: string, stash: string[]): string {
  return out.replace(PLACEHOLDER_RE, (_match, index: string) => stash[Number(index)] ?? "");
}

// The converter rewrites heading text (and therefore the heading slugs Rspress
// derives) to hangul, but leaves Markdown link destinations untouched, so a
// fragment such as `#使用者-定義` no longer matches its converted `#사용자-정의`
// heading. Convert the hanja inside each link fragment the same way; the
// hyphenated slug shape and the path before `#` are preserved, so the fragment
// matches the heading slug again (`使用者-定義` and `使用者 定義` yield the same
// hangul).
const LINK_RE = /\]\(([^)]+)\)/g;
async function rewriteAnchorFragments(md: string): Promise<string> {
  const fragments = new Set<string>();
  for (const match of md.matchAll(LINK_RE)) {
    const hash = match[1].indexOf("#");
    if (hash >= 0 && /\p{Script=Han}/u.test(match[1].slice(hash + 1))) {
      fragments.add(match[1].slice(hash + 1));
    }
  }
  if (fragments.size === 0) return md;
  const converted = new Map<string, string>();
  for (const fragment of fragments) converted.set(fragment, await convertPlain(fragment));
  return md.replace(LINK_RE, (whole, dest: string) => {
    const hash = dest.indexOf("#");
    const replacement = hash >= 0 ? converted.get(dest.slice(hash + 1)) : undefined;
    return replacement === undefined ? whole : `](${dest.slice(0, hash)}#${replacement})`;
  });
}

// Generated ko-KR pages have no editable source in the repository (the tree is
// gitignored), so the site-wide "Edit this page" link would point at a file
// that does not exist on GitHub. Suppress it via front matter unless the source
// already set `editLink` explicitly.
function disableEditLink(md: string): string {
  const frontmatter = md.match(/^(---\r?\n)([\s\S]*?)(\r?\n---\r?\n?)/);
  if (frontmatter === null) return `---\neditLink: false\n---\n\n${md}`;
  if (/^\s*editLink\s*:/m.test(frontmatter[2])) return md;
  return md.replace(/^---\r?\n/, "---\neditLink: false\n");
}

async function convertMarkdown(src: string, relPosix: string, isMdx: boolean): Promise<string> {
  const { frontmatter, body } = splitFrontmatter(src);
  const selectors = frontmatterSelectors(relPosix, frontmatter).flatMap(
    (selector) => ["--markdown-frontmatter-convert", selector],
  );
  const { masked, stash } = isMdx ? maskMdx(body) : { masked: body, stash: [] };
  // `.text()` strips the converter's trailing newline; restore it so every file
  // ends with exactly one.
  const converted = await $`${bin} --format ${MARKDOWN_FORMAT} ${selectors}`
    .stdinText(frontmatter + masked)
    .text();
  let out = `${converted}\n`;
  if (isMdx) out = unmaskMdx(out, stash);
  out = await rewriteAnchorFragments(out);
  return disableEditLink(out);
}

// ── Tree walk ────────────────────────────────────────────────────────────────

let fileCount = 0;

async function processFile(file: Path): Promise<void> {
  const rel = srcDir.relative(file);
  const outPath = outDir.join(rel);
  outPath.parentOrThrow().ensureDirSync();
  const base = file.basename().toLowerCase();
  if (base.endsWith(".md") || base.endsWith(".mdx")) {
    outPath.writeTextSync(await convertMarkdown(file.readTextSync(), rel, base.endsWith(".mdx")));
  } else if (base === "_meta.json" || base === "_nav.json") {
    const json = JSON.parse(file.readTextSync()) as Json;
    outPath.writeTextSync(`${JSON.stringify(await convertMetaJson(json), null, 2)}\n`);
  } else {
    file.copyFileSync(outPath);
  }
  fileCount += 1;
}

// `isDirSync`/`isFileSync` follow symlinks, so ko-Kore/internals/design.md (a
// symlink to the repo-root DESIGN.ko-Kore.md) is read through and written as a
// real converted file in ko-KR/ rather than a dangling link.
async function walk(dir: Path): Promise<void> {
  for (const entry of dir.readDirSync()) {
    const child = dir.join(entry.name);
    if (child.isDirSync()) {
      await walk(child);
    } else if (child.isFileSync()) {
      await processFile(child);
    }
  }
}

outDir.ensureRemoveSync({ recursive: true });
outDir.ensureDirSync();
await walk(srcDir);
console.log(`Generated ko-KR locale (${fileCount} files) at ${outDir}`);

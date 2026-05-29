// Gukhanmun: Subsets Noto Sans/Serif KR to the glyphs used by the docs hero.
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

// Run via `mise run docs-fonts`, which puts the pipx-managed fonttools CLIs
// (fonttools, pyftsubset) on PATH. The hero only ever renders the fixed set of
// characters in hero-sentences.json, so we ship a tiny self-hosted subset of
// each face instead of depending on Google Fonts at runtime. The hanja and the
// converted hangul use the sans face; the surrounding native-hangul glue uses
// the serif face (see HeroConversion.css), so each face only needs its half of
// the glyphs.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const themeDir = join(here, "..", "theme");
const outDir = join(themeDir, "fonts");

// The same variable fonts Google Fonts serves, so the instanced weight 600
// keeps the metrics HeroConversion.css relies on (hanja 1em, hangul 0.92em).
// Pin to an immutable commit and verify each download against a known
// SHA-256, so regenerating always reproduces the committed woff2.
const WEIGHT = 600;
const SOURCE_COMMIT = "69430e34bc2619bbef2a6944bb42ec461b900d43";
const rawUrl = (path) => `https://raw.githubusercontent.com/google/fonts/${SOURCE_COMMIT}/${path}`;
const FACES = [
  {
    name: "Noto Sans KR",
    url: rawUrl("ofl/notosanskr/NotoSansKR%5Bwght%5D.ttf"),
    sha256: "194018e6b2b293a7964f037b25c0249ce1418bc9ab3c971060a03aa57861e252",
    out: "noto-sans-kr-hero.woff2",
    // The converting word, hanja and resulting hangul alike.
    pick: (seg) => ("h" in seg ? seg.h + seg.k : ""),
  },
  {
    name: "Noto Serif KR",
    url: rawUrl("ofl/notoserifkr/NotoSerifKR%5Bwght%5D.ttf"),
    sha256: "11f8d5de6f1b79195efba3828aaa2ec95c1178f5ae976fb23c8d53250a9938f3",
    out: "noto-serif-kr-hero.woff2",
    // The native-hangul glue: particles, endings, spaces, punctuation.
    pick: (seg) => ("t" in seg ? seg.t : ""),
  },
];

// fonttools stamps the head table's created/modified fields with the current
// time unless SOURCE_DATE_EPOCH is set; pin it so the woff2 are byte-identical
// on every regeneration (no spurious diffs). The exact value (2025-05-23Z) is
// arbitrary; it only has to stay constant.
process.env.SOURCE_DATE_EPOCH = "1747958400";

const sentences = JSON.parse(
  readFileSync(join(themeDir, "hero-sentences.json"), "utf8"),
);

// Collect, for a face, the unique characters it must contain.
function charsFor(pick) {
  const set = new Set();
  for (const sentence of sentences) {
    for (const seg of sentence) {
      for (const ch of pick(seg)) set.add(ch);
    }
  }
  return [...set].sort().join("");
}

async function download(url, dest, expectedSha256) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Failed to download ${url}: ${res.status}`);
  const bytes = Buffer.from(await res.arrayBuffer());
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== expectedSha256) {
    throw new Error(
      `Checksum mismatch for ${url}\n  expected ${expectedSha256}\n  got      ${actual}`,
    );
  }
  writeFileSync(dest, bytes);
}

mkdirSync(outDir, { recursive: true });

for (const face of FACES) {
  const text = charsFor(face.pick);
  console.log(`${face.name}: ${[...text].length} glyphs (${text})`);

  const src = join(tmpdir(), `${face.out}.src.ttf`);
  const instance = join(tmpdir(), `${face.out}.wght${WEIGHT}.ttf`);
  const dest = join(outDir, face.out);

  await download(face.url, src, face.sha256);
  // Pin the weight axis to 600, then keep only the glyphs we render.
  execFileSync("fonttools", ["varLib.instancer", src, `wght=${WEIGHT}`, "-o", instance], {
    stdio: "inherit",
  });
  execFileSync("pyftsubset", [
    instance,
    `--text=${text}`,
    "--flavor=woff2",
    `--output-file=${dest}`,
  ], { stdio: "inherit" });

  console.log(`  -> ${dest} (${(statSync(dest).size / 1024).toFixed(1)} KiB)\n`);
}

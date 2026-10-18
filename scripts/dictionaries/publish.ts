// Gukhanmun: Publishes only dictionary trees that passed the offline target gate.
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

import { command, hash, json, read, safePath, updatePaths } from "./model.ts";
import type { Execute } from "./model.ts";
import { botBranch, target, validTarget } from "./targets.ts";
import type { Result } from "./update.ts";
const repository = "dahlia/gukhanmun";
const bot = "41898282+github-actions[bot]@users.noreply.github.com";
/** Immutable binding between prepared file bytes, the tested tree, and controller. */
export interface Manifest {
  version: number;
  controller: string;
  base: string;
  tree: string;
  target: string;
  status: "updated" | "unchanged";
  summary: string;
  files: { path: string; blob: string; sha256: string; mode: string }[];
}
/** Captures only approved files; callers run the complete CI gate before bundling. */
export async function bundle(
  directory: string,
  root: string,
  result: Result,
  branch: string,
  execute: Execute,
): Promise<void> {
  if (!validTarget(branch)) throw new Error("invalid target for bundle");
  const changed = new Set<string>();
  for (
    const args of [["diff", "--name-only", "--no-renames", "-z", "HEAD"], [
      "ls-files",
      "--others",
      "--exclude-standard",
      "-z",
    ]]
  ) {
    for (const path of (await execute("git", args)).split("\0").filter(Boolean)) changed.add(path);
  }
  if ([...changed].some((p) => !updatePaths.includes(p))) {
    throw new Error("tested tree has non-data changes");
  }
  if ((result.status === "unchanged") !== (changed.size === 0)) {
    throw new Error("inconsistent update result");
  }
  await Deno.mkdir(directory);
  const manifest: Manifest = {
    version: 1,
    controller: Deno.env.get("GITHUB_SHA") ?? "",
    base: await execute("git", ["rev-parse", "HEAD"]),
    tree: "",
    target: branch,
    status: result.status,
    summary: result.summary,
    files: [],
  };
  for (const path of [...changed].sort()) {
    const bytes = await read(root, path);
    if (!bytes) throw new Error("dictionary update cannot delete files");
    const blob = `file-${manifest.files.length}`;
    await Deno.writeFile(`${directory}/${blob}`, bytes, { mode: 0o644 });
    manifest.files.push({ path, blob, sha256: await hash(bytes), mode: "100644" });
  }
  await execute("git", ["read-tree", manifest.base]);
  if (manifest.files.length) {
    await execute("git", [
      "add",
      "--",
      ...manifest.files.map((f) => f.path),
    ]);
  }
  const entries = await execute("git", ["ls-files", "--stage"]);
  for (const f of manifest.files) {
    if (
      !entries.split("\n").some((line) =>
        line.startsWith(`${f.mode} `) && line.endsWith(`\t${f.path}`)
      )
    ) throw new Error("non-data file mode in tested tree");
  }
  manifest.tree = await execute("git", ["write-tree"]);
  await Deno.writeFile(`${directory}/manifest.json`, json(manifest));
}
/** Checks artifact syntax and every bounded, regular, nonexecutable blob before writes. */
export async function validate(directory: string): Promise<Manifest> {
  const raw = await read(directory, "manifest.json");
  if (!raw || raw.length > 128 * 1024) throw new Error("invalid dictionary manifest size");
  const m: Manifest = JSON.parse(new TextDecoder().decode(raw));
  if (
    m.version !== 1 ||
    ![m.controller, m.base, m.tree].every((s) =>
      typeof s === "string" && /^[a-f0-9]{40,64}$/.test(s)
    ) ||
    !validTarget(m.target) || !["updated", "unchanged"].includes(m.status) ||
    typeof m.summary !== "string" || m.summary.length > 32768 || !Array.isArray(m.files) ||
    m.files.length > updatePaths.length ||
    (m.status === "unchanged" && m.files.length) || (m.status === "updated" && !m.files.length)
  ) throw new Error("invalid dictionary manifest");
  const paths = new Set<string>(), blobs = new Set<string>();
  let total = 0;
  for (const f of m.files) {
    if (
      !f || !updatePaths.includes(f.path) || paths.has(f.path) || !/^file-[0-9]+$/.test(f.blob) ||
      blobs.has(f.blob) ||
      f.mode !== "100644" || !/^[a-f0-9]{64}$/.test(f.sha256)
    ) throw new Error("invalid dictionary artifact path/mode");
    paths.add(f.path);
    blobs.add(f.blob);
    const bytes = await read(directory, f.blob);
    if (!bytes || await hash(bytes) !== f.sha256) {
      throw new Error("dictionary artifact checksum mismatch");
    }
    total += bytes.length;
    if (total > 256 * 1024 * 1024) throw new Error("oversized dictionary artifact");
  }
  return m;
}
/** Replaces only the bot-owned report block, preserving human PR prose. */
export function proposalBody(previous: string, summary: string, base: string): string {
  const begin = "<!-- dictionary-update:begin -->", end = "<!-- dictionary-update:end -->";
  const block =
    `${begin}\n${summary}\n\nFull offline CI passed on base \`${base}\`. Review before merging.\n\nhttps://github.com/dahlia/gukhanmun/issues/29\n${end}`;
  const start = previous.indexOf(begin), finish = previous.indexOf(end);
  if (start < 0 && finish < 0) {
    return `${previous.trimEnd()}${previous.trim() ? "\n\n" : ""}${block}\n`;
  }
  if (
    start < 0 || finish < start || previous.indexOf(begin, start + begin.length) >= 0 ||
    previous.indexOf(end, finish + end.length) >= 0
  ) {
    throw new Error("ambiguous bot report markers; preserve human PR text");
  }
  return previous.slice(0, start) + block + previous.slice(finish + end.length);
}
async function status(oid: string, execute: Execute): Promise<void> {
  await execute("gh", [
    "api",
    `repos/${repository}/statuses/${oid}`,
    "-X",
    "POST",
    "-f",
    "state=success",
    "-f",
    "context=dictionary-data-update",
    "-f",
    "description=Complete target CI passed for this tree",
  ]);
}
/** Rebuilds the tested tree, verifies target freshness, and protects human bot-branch work. */
export async function publish(directory: string, root: string, execute: Execute): Promise<void> {
  if (
    Deno.env.get("GITHUB_ACTIONS") !== "true" || Deno.env.get("GITHUB_REPOSITORY") !== repository ||
    Deno.env.get("GITHUB_REF") !== "refs/heads/main"
  ) throw new Error("publication requires trusted main Actions execution");
  const m = await validate(directory);
  if (m.controller !== Deno.env.get("GITHUB_SHA")) throw new Error("artifact controller mismatch");
  const fresh = await target(execute);
  if (fresh.branch !== m.target || fresh.oid !== m.base) {
    throw new Error("latest maintenance target advanced; rerun");
  }
  const branch = botBranch(m.target);
  const old =
    (await execute("git", ["ls-remote", "origin", `refs/heads/${branch}`])).split(/\s/)[0];
  if (old) {
    if (!/^[a-f0-9]{40,64}$/.test(old)) throw new Error("invalid bot head object");
    await execute("git", ["fetch", "origin", `refs/heads/${branch}`]);
    const authors = await execute("git", ["log", "--format=%ae|%ce", `${m.base}..${old}`]);
    if (authors.split("\n").filter(Boolean).some((a) => a !== `${bot}|${bot}`)) {
      throw new Error("bot branch contains human commits; refusing replacement");
    }
  }
  const prs = JSON.parse(
    await execute("gh", [
      "api",
      `repos/${repository}/pulls?head=dahlia:${branch}&state=open&per_page=100`,
    ]),
  );
  if (!Array.isArray(prs) || prs.length > 1) throw new Error("ambiguous dictionary update PR");
  const pr = prs[0];
  if (pr && pr.base?.ref !== m.target) throw new Error("existing dictionary PR has wrong target");
  if (m.status === "unchanged") return;
  if (await execute("git", ["status", "--porcelain"])) {
    throw new Error("publisher target checkout is dirty");
  }
  if (await execute("git", ["rev-parse", "HEAD"]) !== m.base) {
    throw new Error("publisher checkout is not pinned base");
  }
  for (const f of m.files) {
    await safePath(root, f.path);
    const separator = f.path.lastIndexOf("/");
    if (separator >= 0) {
      await Deno.mkdir(`${root}/${f.path.slice(0, separator)}`, { recursive: true });
    }
    await Deno.writeFile(`${root}/${f.path}`, (await read(directory, f.blob))!);
    if (Deno.build.os !== "windows") await Deno.chmod(`${root}/${f.path}`, 0o644);
  }
  await execute("git", ["read-tree", m.base]);
  await execute("git", ["add", "--", ...m.files.map((f) => f.path)]);
  const tree = await execute("git", ["write-tree"]);
  if (tree !== m.tree) throw new Error("published tree differs from tested tree");
  if (
    old && pr && await execute("git", ["rev-parse", `${old}^{tree}`]) === tree &&
    await execute("git", ["show", "-s", "--format=%P", old]) === m.base
  ) {
    await status(old, execute);
    return;
  }
  await execute("git", [
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "commit.gpgsign=false",
    "-c",
    "user.name=github-actions[bot]",
    "-c",
    `user.email=${bot}`,
    "commit",
    "-m",
    "Update bundled Korean dictionary data",
    "-m",
    "https://github.com/dahlia/gukhanmun/issues/29",
  ]);
  const oid = await execute("git", ["rev-parse", "HEAD"]);
  if (await execute("git", ["rev-parse", `${oid}^{tree}`]) !== m.tree) {
    throw new Error("committed tree differs from tested tree");
  }
  // Refresh again after staging/commit, immediately before the external mutation.
  const latest = await target(execute);
  if (latest.branch !== m.target || latest.oid !== m.base) {
    throw new Error("target changed before publication; rerun");
  }
  await execute("git", [
    "push",
    `--force-with-lease=refs/heads/${branch}:${old}`,
    "origin",
    `HEAD:refs/heads/${branch}`,
  ]);
  await status(oid, execute);
  const body = `${directory}/pr-body.md`;
  await Deno.writeTextFile(
    body,
    proposalBody(typeof pr?.body === "string" ? pr.body : "", m.summary, m.base),
  );
  if (pr) {
    await execute("gh", [
      "pr",
      "edit",
      String(pr.number),
      "--repo",
      repository,
      "--body-file",
      body,
    ]);
  } else {await execute("gh", [
      "pr",
      "create",
      "--repo",
      repository,
      "--base",
      m.target,
      "--head",
      branch,
      "--title",
      "Update bundled Korean dictionary data",
      "--body-file",
      body,
    ]);}
}
if (import.meta.main) {
  try {
    const [mode, directory] = Deno.args;
    if (!directory) throw new Error("artifact directory required");
    const root = Deno.env.get("GUKHANMUN_DICTS_ROOT") ?? Deno.cwd();
    const execute: Execute = (cmd, args, optional) => command(cmd, args, optional, root);
    if (mode === "validate") await validate(directory);
    else if (mode === "bundle") {
      const result: Result = JSON.parse(
        await Deno.readTextFile(Deno.env.get("GUKHANMUN_DICTS_RESULT")!),
      );
      await bundle(directory, root, result, Deno.env.get("GUKHANMUN_DICTS_TARGET")!, execute);
    } else if (mode === "publish") await publish(directory, root, execute);
    else throw new Error("unknown dictionary publication operation");
  } catch (e) {
    console.error(e instanceof Error ? e.message : "dictionary publication failed");
    Deno.exit(1);
  }
}

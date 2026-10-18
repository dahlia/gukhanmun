// Gukhanmun: Publishes validated loanword update pull requests.
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

/** Transfers only verified data paths between the read-only and publishing jobs. */
import { hash, updatePaths } from "./update.ts";
import { botBranch, targets, validTarget } from "./targets.ts";
const decoder = new TextDecoder();
const repository = "dahlia/gukhanmun";
const bot = "41898282+github-actions[bot]@users.noreply.github.com";
const data = "crates/gukhanmun-loanwords/data";
interface Manifest {
  base: string;
  tree: string;
  target: string;
  primary: string;
  controller: string;
  status: "updated" | "pending" | "unchanged";
  fingerprint: string;
  files: { path: string; file: string | null; sha256: string | null }[];
}
async function command(
  cmd: string,
  args: string[],
  optional = false,
  cwd?: string,
): Promise<string> {
  const r = await new Deno.Command(cmd, { args, cwd, stdout: "piped", stderr: "piped" }).output();
  if (!r.success && !optional) throw new Error(`${cmd} operation failed`);
  const output = decoder.decode(r.stdout);
  return args.includes("-z") ? output : output.trim();
}
async function get(path: string): Promise<Uint8Array | null> {
  try {
    if (!(await Deno.lstat(path)).isFile) throw new Error("non-regular artifact refused");
    return await Deno.readFile(path);
  } catch (e) {
    if (e instanceof Deno.errors.NotFound) return null;
    throw e;
  }
}
/** Copies a prior proposal's snapshot to private staging for validated metadata reuse. */
export async function candidate(
  destination: string,
  target: string,
  execute: typeof command = command,
): Promise<void> {
  const branch = botBranch(target);
  try {
    await Deno.remove(destination);
  } catch (error) {
    if (!(error instanceof Deno.errors.NotFound)) throw error;
  }
  const remote = await execute("git", ["ls-remote", "origin", `refs/heads/${branch}`]);
  const oid = remote.split(/\s/)[0];
  if (!oid) return;
  if (!/^[a-f0-9]{40,64}$/.test(oid)) throw new Error("invalid candidate object");
  await execute("git", ["fetch", "origin", `refs/heads/${branch}`]);
  const snapshot = await execute("git", ["show", `${oid}:${data}/snapshot.json`], true);
  if (snapshot) await Deno.writeTextFile(destination, snapshot + "\n");
}
async function bundle(directory: string): Promise<void> {
  const root = Deno.env.get("GUKHANMUN_LOANWORDS_ROOT") ?? Deno.cwd();
  const execute = (cmd: string, args: string[], optional = false) =>
    command(cmd, args, optional, root);
  for (
    const args of [
      ["diff", "--name-only", "--no-renames", "-z", "HEAD"],
      ["ls-files", "--others", "--exclude-standard", "-z"],
    ]
  ) {
    const paths = (await execute("git", args)).split("\0").filter(Boolean);
    if (paths.some((p) => !updatePaths.includes(p))) {
      throw new Error("tested tree has non-update changes");
    }
  }
  const resultFile = Deno.env.get("GUKHANMUN_LOANWORDS_RESULT");
  if (!resultFile) throw new Error("update result required");
  const { status, fingerprint } = JSON.parse(await Deno.readTextFile(resultFile));
  if (
    !["updated", "pending", "unchanged"].includes(status ?? "") ||
    !/^[a-f0-9]{64}$/.test(fingerprint ?? "")
  ) throw new Error("invalid update outcome");
  await Deno.mkdir(directory, { recursive: true });
  const manifest: Manifest = {
    base: await execute("git", ["rev-parse", "HEAD"]),
    tree: "",
    target: Deno.env.get("GUKHANMUN_LOANWORDS_TARGET") ?? "",
    primary: Deno.env.get("GUKHANMUN_LOANWORDS_PRIMARY") ?? "",
    controller: Deno.env.get("GITHUB_SHA") ?? "",
    status: status as Manifest["status"],
    fingerprint: fingerprint!,
    files: [],
  };
  let index = 0;
  for (const path of updatePaths) {
    const changed = await execute("git", ["diff", "--name-only", "HEAD", "--", path]);
    const untracked = await execute("git", [
      "ls-files",
      "--others",
      "--exclude-standard",
      "--",
      path,
    ]);
    if (!changed && !untracked) continue;
    const bytes = await get(`${root}/${path}`);
    if (bytes !== null) {
      const text = decoder.decode(bytes);
      const key = Deno.env.get("NIKL_LOANWORD_API_KEY");
      if (
        key &&
        [key, encodeURIComponent(key), decodeURIComponent(key)].some((k) => k && text.includes(k))
      ) throw new Error("credential-bearing artifact refused");
      const file = `file-${index++}`;
      await Deno.writeFile(`${directory}/${file}`, bytes);
      manifest.files.push({ path, file, sha256: await hash(bytes) });
    } else manifest.files.push({ path, file: null, sha256: null });
  }
  await execute("git", ["read-tree", manifest.base]);
  if (manifest.files.length) {
    await execute("git", [
      "add",
      "--",
      ...manifest.files.map((f) => f.path),
    ]);
  }
  manifest.tree = await execute("git", ["write-tree"]);
  await Deno.writeTextFile(`${directory}/manifest.json`, JSON.stringify(manifest));
}
/** Verifies an artifact before any repository write or publication. */
export async function validate(directory: string): Promise<Manifest> {
  const raw = await get(`${directory}/manifest.json`);
  if (!raw || raw.length > 128 * 1024) throw new Error("invalid artifact manifest");
  const m: Manifest = JSON.parse(decoder.decode(raw));
  if (
    !/^[a-f0-9]{40,64}$/.test(m.base) || !/^[a-f0-9]{40,64}$/.test(m.tree) ||
    !/^[a-f0-9]{40,64}$/.test(m.controller) || !validTarget(m.target) || !validTarget(m.primary) ||
    !/^[a-f0-9]{64}$/.test(m.fingerprint) ||
    !["updated", "pending", "unchanged"].includes(m.status) || !Array.isArray(m.files) ||
    m.files.length > updatePaths.length
  ) throw new Error("invalid update manifest");
  if (m.status === "unchanged" && m.files.length) throw new Error("no-op artifact contains writes");
  const paths = new Set<string>();
  const blobs = new Set<string>();
  for (const f of m.files) {
    if (!updatePaths.includes(f.path) || paths.has(f.path)) {
      throw new Error("artifact path outside update allowlist");
    }
    paths.add(f.path);
    if (m.status === "pending" && f.path !== `${data}/pending-update.json`) {
      throw new Error("pending update cannot replace successful data");
    }
    if (f.file === null) {
      if (f.sha256 !== null) throw new Error("invalid deletion");
      continue;
    }
    if (!/^file-[0-9]+$/.test(f.file) || blobs.has(f.file)) {
      throw new Error("invalid artifact blob");
    }
    blobs.add(f.file);
    const stat = await Deno.lstat(`${directory}/${f.file}`);
    if (!stat.isFile || ((stat.mode ?? 0) & 0o111) !== 0) {
      throw new Error("non-regular or executable artifact refused");
    }
    const bytes = await get(`${directory}/${f.file}`);
    if (!bytes || bytes.length > 256 * 1024 * 1024 || await hash(bytes) !== f.sha256) {
      throw new Error("artifact checksum mismatch");
    }
  }
  return m;
}
async function setStatus(
  oid: string,
  state: "success" | "failure",
  execute: typeof command,
): Promise<void> {
  await execute("gh", [
    "api",
    `repos/${repository}/statuses/${oid}`,
    "-X",
    "POST",
    "-f",
    `state=${state}`,
    "-f",
    "context=loanword-data-update",
    "-f",
    `description=${
      state === "success" ? "Offline data validation passed" : "Editorial decisions require review"
    }`,
  ]);
}
async function reconcilePr(
  pr: { number: number; draft: boolean },
  status: "updated" | "pending",
  directory: string,
  execute: typeof command,
): Promise<void> {
  const body = `${directory}/pr-body.md`;
  await Deno.copyFile(
    new URL(status === "pending" ? "./conflicts-pr.md" : "./update-pr.md", import.meta.url),
    body,
  );
  await execute("gh", ["pr", "edit", String(pr.number), "--repo", repository, "--body-file", body]);
  if (status === "pending" && !pr.draft) {
    await execute("gh", ["pr", "ready", String(pr.number), "--repo", repository, "--undo"]);
  }
  if (status === "updated" && pr.draft) {
    await execute("gh", ["pr", "ready", String(pr.number), "--repo", repository]);
  }
}
/** Applies a validated artifact and maintains the trusted bot PR for its target. */
export async function publish(
  directory: string,
  execute: typeof command = command,
  root = Deno.cwd(),
): Promise<void> {
  if (
    Deno.env.get("GITHUB_ACTIONS") !== "true" || Deno.env.get("GITHUB_REPOSITORY") !== repository ||
    Deno.env.get("GITHUB_REF") !== "refs/heads/main"
  ) throw new Error("publication is restricted to trusted main workflow runs");
  const m = await validate(directory);
  if (m.controller !== Deno.env.get("GITHUB_SHA")) throw new Error("artifact controller mismatch");
  const selected = await targets(execute);
  if (selected[0].branch !== m.primary || !selected.some((t) => t.branch === m.target)) {
    throw new Error("update target selection changed; rerun the workflow");
  }
  if (selected.find((t) => t.branch === m.target)!.oid !== m.base) {
    throw new Error(`${m.target} advanced during collection; rerun the workflow`);
  }
  const branch = botBranch(m.target);
  const remote = await execute("git", ["ls-remote", "origin", `refs/heads/${branch}`]);
  const old = remote.split(/\s/)[0];
  if (old) {
    await execute("git", ["fetch", "origin", `${branch}:refs/remotes/origin/${branch}`]);
    const authors = await execute("git", ["log", "--format=%ae|%ce", `${m.base}..${old}`]);
    if (authors && authors.split("\n").some((a) => a !== `${bot}|${bot}`)) {
      throw new Error(
        "bot branch contains human commits; move them to a separate branch before retrying",
      );
    }
  }
  const prs = JSON.parse(
    await execute("gh", [
      "api",
      `repos/${repository}/pulls?head=dahlia:${branch}&state=open&per_page=100`,
    ]),
  );
  if (prs.length > 1) throw new Error("multiple update PRs require maintainer review");
  const pr = prs[0];
  if (pr && pr.base?.ref !== m.target) throw new Error("existing update PR has the wrong base");
  if (m.status === "unchanged") {
    if (pr) await execute("gh", ["pr", "close", String(pr.number), "--repo", repository]);
    return;
  }
  await execute("git", ["switch", "--detach", m.base]);
  for (const f of m.files) {
    if (f.file === null) {
      try {
        await Deno.remove(`${root}/${f.path}`);
      } catch (e) {
        if (!(e instanceof Deno.errors.NotFound)) throw e;
      }
    } else {
      if (
        await get(`${root}/${f.path}`) !== null && (await Deno.lstat(`${root}/${f.path}`)).isSymlink
      ) {
        throw new Error("symlink destination refused");
      }
      const separator = f.path.lastIndexOf("/");
      if (separator >= 0) {
        await Deno.mkdir(`${root}/${f.path.slice(0, separator)}`, { recursive: true });
      }
      await Deno.copyFile(`${directory}/${f.file}`, `${root}/${f.path}`);
    }
  }
  await execute("git", ["config", "user.name", "github-actions[bot]"]);
  await execute("git", ["config", "user.email", bot]);
  await execute("git", ["read-tree", m.base]);
  if (m.files.length) await execute("git", ["add", "--", ...m.files.map((f) => f.path)]);
  const tree = await execute("git", ["write-tree"]);
  if (tree !== m.tree) throw new Error("published tree differs from tested tree");
  if (
    old && pr && await execute("git", ["rev-parse", `${old}^{tree}`]) === tree &&
    await execute("git", ["show", "-s", "--format=%P", old]) === m.base
  ) {
    await setStatus(old, m.status === "pending" ? "failure" : "success", execute);
    if (pr.draft !== (m.status === "pending")) await reconcilePr(pr, m.status, directory, execute);
    return;
  }
  if (!await execute("git", ["diff", "--cached", "--name-only"])) return;
  await execute("git", [
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "commit.gpgsign=false",
    "commit",
    "-m",
    m.status === "pending" ? "Report loanword editorial conflicts" : "Update NIKL loanword data",
    "-m",
    "https://github.com/dahlia/gukhanmun/issues/18",
  ]);
  const oid = await execute("git", ["rev-parse", "HEAD"]);
  if (await execute("git", ["rev-parse", `${oid}^{tree}`]) !== m.tree) {
    throw new Error("committed tree differs from tested tree");
  }
  await execute("git", [
    "push",
    `--force-with-lease=refs/heads/${branch}:${old}`,
    "origin",
    `HEAD:refs/heads/${branch}`,
  ]);
  await setStatus(oid, m.status === "pending" ? "failure" : "success", execute);
  if (pr) {
    await reconcilePr(pr, m.status, directory, execute);
  } else {
    const body = `${directory}/pr-body.md`;
    await Deno.copyFile(
      new URL(m.status === "pending" ? "./conflicts-pr.md" : "./update-pr.md", import.meta.url),
      body,
    );
    await execute("gh", [
      "pr",
      "create",
      "--repo",
      repository,
      "--base",
      m.target,
      "--head",
      branch,
      "--title",
      "Update NIKL loanword data",
      "--body-file",
      body,
      ...(m.status === "pending" ? ["--draft"] : []),
    ]);
  }
}
if (import.meta.main) {
  const [mode, directory] = Deno.args;
  try {
    if (!directory) throw new Error("artifact directory required");
    if (mode === "candidate") {
      await candidate(directory, Deno.env.get("GUKHANMUN_LOANWORDS_TARGET") ?? "");
    } else if (mode === "bundle") await bundle(directory);
    else if (mode === "validate") await validate(directory);
    else if (mode === "publish") {
      const root = Deno.env.get("GUKHANMUN_LOANWORDS_ROOT") ?? Deno.cwd();
      await publish(directory, (cmd, args, optional) => command(cmd, args, optional, root), root);
    } else throw new Error("unknown publication operation");
  } catch (e) {
    console.error(e instanceof Error ? e.message : "publication failed");
    Deno.exit(1);
  }
}

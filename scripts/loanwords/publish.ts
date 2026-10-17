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
import { generationFingerprint, hash, updatePaths } from "./update.ts";
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const repository = "dahlia/gukhanmun";
const branch = "automation/loanword-data";
const bot = "41898282+github-actions[bot]@users.noreply.github.com";
const data = "crates/gukhanmun-loanwords/data";
interface Manifest {
  base: string;
  status: "updated" | "pending" | "unchanged";
  fingerprint: string;
  files: { path: string; file: string | null; sha256: string | null }[];
}
async function command(cmd: string, args: string[], optional = false): Promise<string> {
  const r = await new Deno.Command(cmd, { args, stdout: "piped", stderr: "piped" }).output();
  if (!r.success && !optional) throw new Error(`${cmd} operation failed`);
  return decoder.decode(r.stdout).trim();
}
async function get(path: string): Promise<Uint8Array | null> {
  try {
    if ((await Deno.lstat(path)).isSymlink) throw new Error("symlink artifact refused");
    return await Deno.readFile(path);
  } catch (e) {
    if (e instanceof Deno.errors.NotFound) return null;
    throw e;
  }
}
async function bundle(directory: string): Promise<void> {
  const resultFile = Deno.env.get("GUKHANMUN_LOANWORDS_RESULT");
  if (!resultFile) throw new Error("update result required");
  const { status, fingerprint } = JSON.parse(await Deno.readTextFile(resultFile));
  if (
    !["updated", "pending", "unchanged"].includes(status ?? "") ||
    !/^[a-f0-9]{64}$/.test(fingerprint ?? "")
  ) throw new Error("invalid update outcome");
  await Deno.mkdir(directory, { recursive: true });
  const manifest: Manifest = {
    base: await command("git", ["rev-parse", "HEAD"]),
    status: status as Manifest["status"],
    fingerprint: fingerprint!,
    files: [],
  };
  let index = 0;
  for (const path of updatePaths) {
    const changed = await command("git", ["diff", "--name-only", "HEAD", "--", path]);
    const untracked = await command("git", [
      "ls-files",
      "--others",
      "--exclude-standard",
      "--",
      path,
    ]);
    if (!changed && !untracked) continue;
    const bytes = await get(path);
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
  await Deno.writeTextFile(`${directory}/manifest.json`, JSON.stringify(manifest));
}
/** Verifies an artifact before any repository write or publication. */
export async function validate(directory: string): Promise<Manifest> {
  const raw = await get(`${directory}/manifest.json`);
  if (!raw || raw.length > 128 * 1024) throw new Error("invalid artifact manifest");
  const m: Manifest = JSON.parse(decoder.decode(raw));
  if (
    !/^[a-f0-9]{40,64}$/.test(m.base) || !/^[a-f0-9]{64}$/.test(m.fingerprint) ||
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
    if (((stat.mode ?? 0) & 0o111) !== 0) throw new Error("executable artifact refused");
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
/** Applies a validated artifact and maintains the single trusted bot PR. */
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
  await execute("git", ["fetch", "origin", "main"]);
  if (await execute("git", ["rev-parse", "origin/main"]) !== m.base) {
    throw new Error("main advanced during collection; rerun the workflow");
  }
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
      `repos/${repository}/pulls?head=dahlia:${branch}&base=main&state=open&per_page=100`,
    ]),
  );
  if (prs.length > 1) throw new Error("multiple update PRs require maintainer review");
  const pr = prs[0];
  if (m.status === "unchanged") {
    if (pr) await execute("gh", ["pr", "close", String(pr.number), "--repo", repository]);
    return;
  }
  if (old) {
    const pending = await execute("git", ["show", `${old}:${data}/pending-update.json`], true);
    const oldFingerprint = m.status === "pending" && pending
      ? await hash(encoder.encode(pending + "\n"))
      : !pending
      ? await generationFingerprint(
        JSON.parse(await execute("git", ["show", `${old}:${data}/snapshot.json`])),
        JSON.parse(await execute("git", ["show", `${old}:${data}/report.json`])),
      )
      : "";
    if (pr && oldFingerprint === m.fingerprint) {
      await setStatus(old, m.status === "pending" ? "failure" : "success", execute);
      // A prior push may have succeeded before PR metadata reconciliation failed.
      if (pr.draft !== (m.status === "pending")) {
        await reconcilePr(pr, m.status, directory, execute);
      }
      return;
    }
  }
  await execute("git", ["switch", "--create", branch, m.base]);
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
  await execute("git", ["add", "--", ...m.files.map((f) => f.path)]);
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
  await execute("git", [
    "push",
    `--force-with-lease=refs/heads/${branch}:${old}`,
    "origin",
    `HEAD:refs/heads/${branch}`,
  ]);
  const oid = await execute("git", ["rev-parse", "HEAD"]);
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
      "main",
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
    if (mode === "bundle") await bundle(directory);
    else if (mode === "validate") await validate(directory);
    else if (mode === "publish") await publish(directory);
    else throw new Error("unknown publication operation");
  } catch (e) {
    console.error(e instanceof Error ? e.message : "publication failed");
    Deno.exit(1);
  }
}

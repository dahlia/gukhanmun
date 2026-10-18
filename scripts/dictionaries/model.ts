// Gukhanmun: Defines the dictionary update artifact and command boundary.
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

/** Public dataset identifiers understood by interface version 1. */
export const kinds = ["stdict", "opendict"] as const;
/** One dictionary corpus, never an arbitrary source name. */
export type Kind = typeof kinds[number];
/** Canonical generated files, excluding editorial decisions. */
export const outputs: Record<Kind, string[]> = {
  stdict: ["stdict.tsv", "suffix.tsv"],
  opendict: ["general.tsv", "north-korean.tsv", "dialect.tsv", "archaic.tsv"],
};
/** Components that bundle or expose each dataset. */
export function sections(kind: Kind): string[] {
  return [
    `crates/gukhanmun-${kind}`,
    "crates/gukhanmun",
    "crates/gukhanmun-cli",
    `packages/${kind}-fst`,
    `packages/${kind}-cdb`,
  ];
}
/** Fixed fragment paths allow idempotent replacement of the bot's own note. */
export function fragments(kind: Kind): string[] {
  return sections(kind).map((section) => `changes.d/${section}/${kind}-data-update.md`);
}
/** Returns the crate-relative canonical data directory. */
export function data(kind: Kind): string {
  return `crates/gukhanmun-${kind}/data`;
}
/** Snapshot-bearing notices that also ship with the open dictionary packages. */
export const notices = [
  "NOTICE.md",
  "crates/gukhanmun-opendict/ATTRIBUTION.md",
  "packages/opendict-fst/ATTRIBUTION.md",
  "packages/opendict-cdb/ATTRIBUTION.md",
];
/** Complete transfer allowlist: no executable code, policies, manifests or binaries. */
export const updatePaths = [
  ...new Set([
    ...kinds.flatMap((kind) => [
      ...[...outputs[kind], "source.json", "metadata.json"].map((name) => `${data(kind)}/${name}`),
      ...fragments(kind),
    ]),
    ...notices,
    "CHANGES.md",
  ]),
];
/** Command injection boundary for offline real-Git tests. */
export type Execute = (cmd: string, args: string[], optional?: boolean) => Promise<string>;
/** Runs without publishing/account credentials and suppresses unsafe command diagnostics. */
export async function command(
  cmd: string,
  args: string[],
  optional = false,
  cwd = Deno.cwd(),
): Promise<string> {
  const result = await new Deno.Command(cmd, { args, cwd, stdout: "piped", stderr: "piped" })
    .output();
  if (!result.success && !optional) throw new Error(`${cmd} operation failed`);
  const text = new TextDecoder().decode(result.stdout);
  return args.includes("-z") ? text : text.trim();
}
/** SHA-256 for file and artifact integrity, independent of timestamps. */
export async function hash(bytes: Uint8Array): Promise<string> {
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes as BufferSource))]
    .map((n) => n.toString(16).padStart(2, "0")).join("");
}
/** Rejects final and ancestor symlinks before reading/writing an allowlisted path. */
export async function safePath(root: string, path: string): Promise<void> {
  let current = root;
  for (const part of path.split("/")) {
    if (!part || part === "." || part === "..") throw new Error("invalid relative update path");
    current += `/${part}`;
    try {
      if ((await Deno.lstat(current)).isSymlink) throw new Error("symlink update path refused");
    } catch (e) {
      if (!(e instanceof Deno.errors.NotFound)) throw e;
    }
  }
}
/** Reads regular nonexecutable data, preserving absent-file information for rollback. */
export async function read(root: string, path: string): Promise<Uint8Array | null> {
  await safePath(root, path);
  try {
    const stat = await Deno.lstat(`${root}/${path}`);
    if (!stat.isFile || ((stat.mode ?? 0) & 0o111)) throw new Error("non-data update path");
    if (stat.size > 64 * 1024 * 1024) throw new Error("oversized update file");
    return await Deno.readFile(`${root}/${path}`);
  } catch (e) {
    if (e instanceof Deno.errors.NotFound) return null;
    throw e;
  }
}
/** Deterministic JSON bytes for public metadata and reports. */
export function json(value: unknown): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(value, null, 2) + "\n");
}

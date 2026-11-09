// Gukhanmun: Resolves trusted changelog layouts for scheduled data updates.
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

/** Supported fragment layouts; source trees remain crates/ and packages/. */
export type ChangelogLayout = "legacy" | "language";
/** A layout obtained from the independently pinned target commit, never an artifact. */
export interface PinnedLayout {
  readonly base: string;
  readonly layout: ChangelogLayout;
}
/** Read-only process boundary for pinned Git object inspection. */
export type Inspect = (cmd: string, args: string[], optional?: boolean) => Promise<string>;
const decode = new TextDecoder();
/** Parses TOML with the project's Python stdlib and accepts only known complete families. */
export async function parseLayout(config: string): Promise<ChangelogLayout> {
  const child = new Deno.Command("python", {
    args: [
      "-I",
      "-c",
      "import json, sys, tomllib; print(json.dumps(tomllib.loads(sys.stdin.read())))",
    ],
    stdin: "piped",
    stdout: "piped",
    stderr: "piped",
  }).spawn();
  const writer = child.stdin.getWriter();
  await writer.write(new TextEncoder().encode(config));
  await writer.close();
  const result = await child.output();
  if (!result.success) throw new Error("invalid target Sacho TOML");
  const value = JSON.parse(decode.decode(result.stdout));
  const patterns = value["section-patterns"];
  if (
    (value.changelog !== undefined &&
      (typeof value.changelog !== "object" || Array.isArray(value.changelog))) ||
    (value.changelog?.path ?? "CHANGES.md") !== "CHANGES.md" ||
    (value.fragments !== undefined &&
      (typeof value.fragments !== "object" || Array.isArray(value.fragments))) ||
    (value.fragments?.directory ?? "changes.d") !== "changes.d" ||
    (value.sections !== undefined && (!Array.isArray(value.sections) || value.sections.length)) ||
    !Array.isArray(patterns) || patterns.length < 2 ||
    patterns.length > 3
  ) {
    throw new Error("unsupported target changelog layout");
  }
  const actual = patterns.map((p: Record<string, unknown>) => [p.source, p.id, p.directory]);
  for (const layout of ["legacy", "language"] as const) {
    const expected = layout === "legacy"
      ? [
        ["crates/{name}", "{name}", "crates/{name}"],
        ["packages/{name}", "@gukhanmun/{name}", "packages/{name}"],
      ]
      : [
        ["crates/{name}", "{name} (Rust)", "rust/{name}"],
        ["packages/{name}", "@gukhanmun/{name} (JavaScript/TypeScript)", "javascript/{name}"],
      ];
    if (layout === "language" && patterns.length === 3) {
      expected.push(["python/{name}", "{name} (Python)", "python/{name}"]);
    }
    if (JSON.stringify(actual) === JSON.stringify(expected)) return layout;
  }
  throw new Error("unsupported target changelog layout");
}
/** Resolves one immutable layout before touching a target's update files. */
export async function readLayout(root: string): Promise<ChangelogLayout> {
  return await parseLayout(await Deno.readTextFile(`${root}/sacho.toml`));
}
/** Resolves configuration from a pinned commit in the controller's object database. */
export async function pinnedLayout(execute: Inspect, oid?: string): Promise<PinnedLayout> {
  const base = oid ?? await execute("git", ["rev-parse", "HEAD"]);
  if (!/^[a-f0-9]{40,64}$/.test(base)) throw new Error("invalid pinned target object");
  const layout = await parseLayout(await execute("git", ["show", `${base}:sacho.toml`]));
  return Object.freeze({ base, layout });
}
/** Maps component source roots to one target's fragment directories. */
export function fragmentSections(sources: readonly string[], layout: ChangelogLayout): string[] {
  return sources.map((source) =>
    layout === "legacy"
      ? source
      : source.replace(/^crates\//, "rust/").replace(/^packages\//, "javascript/")
  );
}

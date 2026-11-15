// Gukhanmun: Selects released branches for weekly loanword updates.
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

/** A pinned data-update destination, selected independently of unreleased versions. */
export interface Target {
  branch: string;
  oid: string;
}
/** Injection boundary for read-only Git inspection and publisher tests. */
export type Execute = (cmd: string, args: string[], optional?: boolean) => Promise<string>;
const feature = "crates/gukhanmun-loanwords/Cargo.toml";
const component = "(0|[1-9][0-9]*)";
const maintenance = new RegExp(`^${component}\\.${component}-maintenance$`);
/** Accepts only canonical supported target branch names. */
export function validTarget(branch: unknown): branch is string {
  return typeof branch === "string" && (branch === "main" || maintenance.test(branch));
}
/** Keeps the development and each maintenance PR on distinct bot heads. */
export function botBranch(target: string): string {
  if (!validTarget(target)) throw new Error("invalid update target");
  return target === "main" ? "automation/loanword-data" : `automation/loanword-data-${target}`;
}
/** Selects the oldest existing loanword maintenance line, or main if none exists. */
export async function targets(execute: Execute): Promise<Target[]> {
  await execute("git", [
    "fetch",
    "--prune",
    "origin",
    "+refs/heads/*:refs/remotes/origin/*",
  ]);
  const heads = (await execute("git", [
    "for-each-ref",
    "--format=%(refname:strip=3) %(objectname)",
    "refs/remotes/origin/",
  ])).split("\n").filter(Boolean).map((line) => {
    const [branch, oid] = line.split(" ");
    if (!/^[a-f0-9]{40,64}$/.test(oid ?? "")) throw new Error("invalid remote branch object");
    return { branch, oid };
  });
  const main = heads.find((t) => t.branch === "main");
  const candidates = heads.filter((t) => maintenance.test(t.branch));
  candidates.sort((a, b) => {
    const aa = maintenance.exec(a.branch)!, bb = maintenance.exec(b.branch)!;
    for (const i of [1, 2]) {
      if (BigInt(aa[i]) !== BigInt(bb[i])) return BigInt(aa[i]) < BigInt(bb[i]) ? -1 : 1;
    }
    return 0;
  });
  for (const candidate of candidates) {
    if (
      await execute("git", ["ls-tree", "--name-only", candidate.oid, "--", feature]) === feature
    ) {
      return [candidate];
    }
  }
  if (!main) throw new Error("main branch is missing");
  return [main];
}
async function command(cmd: string, args: string[], optional = false): Promise<string> {
  const result = await new Deno.Command(cmd, { args, stdout: "piped", stderr: "piped" }).output();
  if (!result.success && !optional) throw new Error(`${cmd} target inspection failed`);
  return new TextDecoder().decode(result.stdout).trim();
}
if (import.meta.main) {
  try {
    const selected = await targets(command);
    console.log(JSON.stringify(selected));
    const output = Deno.env.get("GITHUB_OUTPUT");
    if (output) {
      await Deno.writeTextFile(
        output,
        `targets=${JSON.stringify(selected)}\nprimary=${selected[0].branch}\n`,
        { append: true },
      );
    }
    const prs = await command("gh", [
      "api",
      "--paginate",
      "repos/dahlia/gukhanmun/pulls?state=open&per_page=100",
      "--jq",
      ".[] | [.number, .base.ref, .head.ref] | @json",
    ]);
    for (const line of prs.split("\n").filter(Boolean)) {
      const [number, base, head] = JSON.parse(line);
      if (
        typeof head === "string" &&
        (head === "automation/loanword-data" || head.startsWith("automation/loanword-data-")) &&
        !selected.some((t) => t.branch === base)
      ) {
        console.log(
          `::warning::Older loanword update PR #${Number(number)} needs maintainer cleanup`,
        );
      }
    }
  } catch (error) {
    console.error(`::error::${error instanceof Error ? error.message : "target selection failed"}`);
    Deno.exit(1);
  }
}

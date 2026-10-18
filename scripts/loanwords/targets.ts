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
const stable = new RegExp(`^${component}\\.${component}\\.${component}$`);
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
/** Selects the newest shipped loanword series, failing closed on missing maintenance. */
export async function targets(execute: Execute): Promise<Target[]> {
  await execute("git", [
    "fetch",
    "--prune",
    "origin",
    "+refs/heads/*:refs/remotes/origin/*",
    "--tags",
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
  if (!main) throw new Error("main branch is missing");
  const tags = (await execute("git", ["for-each-ref", "--format=%(refname:strip=2)", "refs/tags/"]))
    .split("\n").filter((name) => stable.test(name));
  const shipped: string[] = [];
  for (const tag of tags) {
    const oid = await execute("git", ["rev-parse", `refs/tags/${tag}^{commit}`]);
    if (await execute("git", ["ls-tree", "--name-only", oid, "--", feature]) === feature) {
      shipped.push(tag);
    }
  }
  shipped.sort((a, b) => {
    const aa = a.split(".").map(BigInt), bb = b.split(".").map(BigInt);
    for (let i = 0; i < 3; i++) if (aa[i] !== bb[i]) return aa[i] > bb[i] ? -1 : 1;
    return 0;
  });
  if (!shipped.length) return [main];
  const latest = shipped[0];
  const branch = `${latest.split(".").slice(0, 2).join(".")}-maintenance`;
  const target = heads.find((t) => t.branch === branch);
  const release = await execute("git", ["rev-parse", `refs/tags/${latest}^{commit}`]);
  if (
    !target ||
    await execute("git", ["ls-tree", "--name-only", target.oid, "--", feature]) !== feature ||
    await execute("git", ["merge-base", release, target.oid], true) !== release
  ) {
    throw new Error(
      `released loanwords ${latest} have no eligible ${branch}; create or repair that branch`,
    );
  }
  return [target, main];
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
        typeof head === "string" && head.startsWith("automation/loanword-data-") &&
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

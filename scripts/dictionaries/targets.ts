// Gukhanmun: Selects the oldest dictionary maintenance line, or main.
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

import { command } from "./model.ts";
import type { Execute } from "./model.ts";
/** A target head pinned before credential-bearing collection. */
export interface Target {
  branch: string;
  oid: string;
}
const component = "(0|[1-9][0-9]*)";
const pattern = new RegExp(`^${component}\\.${component}-maintenance$`);
/** Accepts canonical maintenance names and the main fallback. */
export function validTarget(value: unknown): value is string {
  return typeof value === "string" && (value === "main" || pattern.test(value));
}
/** Stable proposal head for one maintenance version line. */
export function botBranch(branch: string): string {
  if (!validTarget(branch)) throw new Error("invalid dictionary target");
  return `automation/dictionary-data-${branch}`;
}
/** Selects the oldest line containing both dictionaries, then checks update compatibility. */
export async function target(execute: Execute = command, remote = "origin"): Promise<Target> {
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(remote)) throw new Error("invalid remote name");
  await execute("git", ["fetch", "--prune", remote, `+refs/heads/*:refs/remotes/${remote}/*`]);
  const heads = (await execute("git", [
    "for-each-ref",
    "--format=%(refname:strip=3) %(objectname)",
    `refs/remotes/${remote}/`,
  ]))
    .split("\n").filter(Boolean).map((line) => {
      const [branch, oid] = line.split(" ");
      if (!/^[a-f0-9]{40,64}$/.test(oid ?? "")) throw new Error("invalid target object");
      return { branch, oid };
    });
  const candidates = heads.filter((t) => pattern.test(t.branch));
  candidates.sort((a, b) => {
    const aa = pattern.exec(a.branch)!, bb = pattern.exec(b.branch)!;
    for (const i of [1, 2]) {
      if (BigInt(aa[i]) !== BigInt(bb[i])) return BigInt(aa[i]) < BigInt(bb[i]) ? -1 : 1;
    }
    return 0;
  });
  let selected = heads.find((t) => t.branch === "main");
  for (const candidate of candidates) {
    const features = ["stdict", "opendict"].map((kind) => `crates/gukhanmun-${kind}/Cargo.toml`);
    const present = await execute("git", [
      "ls-tree",
      "--name-only",
      candidate.oid,
      "--",
      ...features,
    ]);
    if (features.every((path) => present.split("\n").includes(path))) {
      selected = candidate;
      break;
    }
  }
  if (!selected) throw new Error("no dictionary maintenance branch or main exists");
  try {
    const marker = JSON.parse(
      await execute("git", ["show", `${selected.oid}:scripts/dictionaries/interface.json`]),
    );
    if (marker.version !== 1) throw new Error("unsupported interface");
    const config = await execute("git", ["show", `${selected.oid}:mise.toml`]);
    for (const task of ["stdict-extract", "opendict-extract", "ci"]) {
      if (!config.includes(`[tasks.${task}]`)) throw new Error("missing task");
    }
    if (!config.includes("GUKHANMUN_DICTS_OUTPUT_DIR")) throw new Error("missing staging support");
    await execute("git", ["show", `${selected.oid}:sacho.toml`]);
    for (const kind of ["stdict", "opendict"]) {
      const metadata = JSON.parse(
        await execute("git", [
          "show",
          `${selected.oid}:crates/gukhanmun-${kind}/data/metadata.json`,
        ]),
      );
      if (
        ![metadata.source, metadata.license, metadata.build_date].every((v) =>
          typeof v === "string" && v
        )
      ) throw new Error("missing metadata");
    }
  } catch {
    throw new Error(
      `${selected.branch} needs dictionary update interface 1, staging tasks, metadata and Sacho; backport the infrastructure before retrying`,
    );
  }
  return selected;
}
if (import.meta.main) {
  try {
    const selected = await target(command, Deno.args[0] ?? "origin");
    console.log(JSON.stringify(selected));
    if (Deno.env.get("GITHUB_OUTPUT")) {
      await Deno.writeTextFile(
        Deno.env.get("GITHUB_OUTPUT")!,
        `branch=${selected.branch}\noid=${selected.oid}\n`,
        { append: true },
      );
    }
  } catch (e) {
    console.error(e instanceof Error ? e.message : "dictionary target selection failed");
    Deno.exit(1);
  }
}

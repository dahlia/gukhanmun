// Gukhanmun: Tests numeric maintenance selection and fail-closed compatibility.
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

import assert from "node:assert/strict";
import { botBranch, target, validTarget } from "./targets.ts";
import type { Execute } from "./model.ts";
function fixture(
  branches: string[],
  compatible = true,
  withoutFeature: string[] = [],
  onlyStdict: string[] = [],
): Execute {
  return (_cmd, args) => {
    if (args[0] === "fetch") return Promise.resolve("");
    if (args[0] === "for-each-ref") {
      return Promise.resolve(branches.map((b, i) => `${b} ${String(i + 1).repeat(40)}`).join("\n"));
    }
    if (args[0] === "ls-tree") {
      const branch = branches[Number(args[2][0]) - 1];
      if (onlyStdict.includes(branch)) return Promise.resolve(args[4]);
      return Promise.resolve(withoutFeature.includes(branch) ? "" : args.slice(4).join("\n"));
    }
    if (args[0] === "show") {
      if (args[1].endsWith("interface.json")) {
        return Promise.resolve(JSON.stringify({ version: compatible ? 1 : 2 }));
      }
      if (args[1].endsWith("metadata.json")) {
        return Promise.resolve('{"source":"source","license":"license","build_date":"date"}');
      }
      return Promise.resolve(
        "[tasks.stdict-extract]\n[tasks.opendict-extract]\n[tasks.ci]\nGUKHANMUN_DICTS_OUTPUT_DIR",
      );
    }
    throw new Error("unexpected inspection");
  };
}
Deno.test("oldest numeric maintenance containing the dictionaries, or main", async () => {
  for (
    const [branches, expected] of [
      [["main", "0.10-maintenance", "0.9-maintenance"], "0.9-maintenance"],
      [["1.99-maintenance", "2.0-maintenance", "02.1-maintenance"], "1.99-maintenance"],
      [["main", "01.2-maintenance"], "main"],
      [["main"], "main"],
    ] as [string[], string][]
  ) assert.equal((await target(fixture(branches))).branch, expected);
  assert.equal(
    (await target(
      fixture(["main", "0.2-maintenance", "0.3-maintenance"], true, ["0.2-maintenance"]),
    )).branch,
    "0.3-maintenance",
  );
  assert.equal(
    (await target(fixture(["main", "0.2-maintenance"], true, ["0.2-maintenance"]))).branch,
    "main",
  );
  assert.equal(
    (await target(
      fixture(["main", "0.1-maintenance", "0.2-maintenance"], true, [], ["0.1-maintenance"]),
    )).branch,
    "0.2-maintenance",
  );
  // No compatibility fallback to newer lines or main once a dictionary line is chosen.
  await assert.rejects(
    () => target(fixture(["main", "0.9-maintenance", "0.10-maintenance"], false)),
    /0.9-maintenance needs/,
  );
  await assert.rejects(() => target(fixture(["main"], false)), /main needs/);
  await assert.rejects(() => target(fixture(["01.2-maintenance"])), /no dictionary/);
  await assert.rejects(
    () => target(() => Promise.reject(new Error("fetch failed"))),
    /fetch failed/,
  );
  for (
    const value of ["0.2-maintenance/evil", "../0.2-maintenance", "01.2-maintenance", null]
  ) assert.equal(validTarget(value), false);
  assert.equal(botBranch("0.10-maintenance"), "automation/dictionary-data-0.10-maintenance");
  assert.equal(validTarget("main"), true);
  assert.equal(botBranch("main"), "automation/dictionary-data-main");
  await assert.rejects(
    () => target(fixture(["0.2-maintenance"]), "-upload-pack=evil"),
    /invalid remote/,
  );
});

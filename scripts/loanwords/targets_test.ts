// Gukhanmun: Tests oldest-existing loanword update destinations.
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

import { configuration } from "../dictionaries/changelog_fixture.ts";
import assert from "node:assert/strict";
import { botBranch, targets, validTarget } from "./targets.ts";
import type { Execute } from "./targets.ts";
const main = "a".repeat(40);
function catalog(options: {
  branches?: string[];
  withoutFeature?: string[];
  noMain?: boolean;
  fail?: string;
  config?: string;
} = {}): Execute {
  return (_cmd, args) => {
    const c = args.join(" ");
    if (options.fail && c.includes(options.fail)) {
      return Promise.reject(new Error("Git inspection failed"));
    }
    if (args[0] === "fetch") return Promise.resolve("");
    if (c.includes("refs/remotes/origin/")) {
      return Promise.resolve(
        [
          ...(options.noMain ? [] : [`main ${main}`]),
          ...(options.branches ?? []).map((name, i) => `${name} ${String(i + 1).repeat(40)}`),
        ].join("\n"),
      );
    }
    if (args[0] === "show" && args[1].endsWith(":sacho.toml")) {
      return Promise.resolve(options.config ?? configuration("legacy"));
    }
    if (args[0] === "ls-tree") {
      const name = options.branches?.[Number(args[2][0]) - 1];
      const has = !options.withoutFeature?.includes(name!);
      return Promise.resolve(has ? "crates/gukhanmun-loanwords/Cargo.toml" : "");
    }
    return Promise.reject(new Error(`unexpected inspection ${c}`));
  };
}
Deno.test("one oldest existing loanword branch is selected numerically without release tags", async () => {
  assert.deepEqual(
    await targets(catalog({ noMain: true, branches: ["0.3-maintenance"] })),
    [{ branch: "0.3-maintenance", oid: "1".repeat(40) }],
  );
  assert.deepEqual(
    await targets(
      catalog({ branches: ["0.10-maintenance", "0.9-maintenance", "1.0-maintenance"] }),
    ),
    [{ branch: "0.9-maintenance", oid: "2".repeat(40) }],
  );
  assert.deepEqual(
    await targets(catalog({
      branches: ["0.2-maintenance", "0.3-maintenance", "0.4-maintenance"],
      withoutFeature: ["0.2-maintenance"],
    })),
    [{ branch: "0.3-maintenance", oid: "2".repeat(40) }],
  );
  // Deleting the oldest line advances the destination, not an external support list.
  assert.deepEqual(
    await targets(catalog({ branches: ["0.10-maintenance", "1.0-maintenance"] })),
    [{ branch: "0.10-maintenance", oid: "1".repeat(40) }],
  );
});
Deno.test("main is the fallback only when no existing maintenance line contains loanwords", async () => {
  for (
    const options of [
      {},
      { branches: ["01.2-maintenance", "0.2-maintenance/evil"] },
      { branches: ["0.2-maintenance"], withoutFeature: ["0.2-maintenance"] },
    ]
  ) {
    assert.deepEqual(await targets(catalog(options)), [{ branch: "main", oid: main }]);
  }
  await assert.rejects(() => targets(catalog({ noMain: true })), /main branch is missing/);
  for (const fail of ["fetch", "for-each-ref", "ls-tree"]) {
    await assert.rejects(
      () => targets(catalog({ branches: ["0.3-maintenance"], fail })),
      /inspection failed/,
    );
  }
});
Deno.test("target and bot branch names are canonical, not lexicographic aliases", () => {
  assert.equal(botBranch("main"), "automation/loanword-data");
  assert.equal(botBranch("0.10-maintenance"), "automation/loanword-data-0.10-maintenance");
  for (
    const name of [
      "01.2-maintenance",
      "0.2-maintenance-old",
      "0.2-maintenance/x",
      "../main",
      "0.2.0",
      null,
    ]
  ) assert.equal(validTarget(name), false);
});

Deno.test("loanword selection validates target layouts before collection", async () => {
  assert.equal((await targets(catalog({ config: configuration("language") })))[0].branch, "main");
  assert.equal(
    (await targets(catalog({
      branches: ["0.3-maintenance", "0.4-maintenance"],
      config: configuration("language"),
    })))[0].branch,
    "0.3-maintenance",
  );
  await assert.rejects(
    () =>
      targets(
        catalog({
          config: configuration("language").replace('id = "{name} (Rust)"', 'id = "{name}"'),
        }),
      ),
    /target changelog/,
  );
  await assert.rejects(
    () =>
      targets(catalog({
        branches: ["0.3-maintenance", "0.4-maintenance"],
        config: configuration("language").replace('id = "{name} (Rust)"', 'id = "{name}"'),
      })),
    /target changelog/,
  );
});

// Gukhanmun: Tests release-aware loanword update destinations.
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
import { botBranch, targets, validTarget } from "./targets.ts";
import type { Execute } from "./targets.ts";
const main = "a".repeat(40), branch = "b".repeat(40), release = "c".repeat(40);
function catalog(options: {
  tags?: string[];
  branches?: string[];
  shipped?: string[];
  feature?: boolean;
  ancestor?: boolean;
  fail?: string;
} = {}): Execute {
  return (_cmd, args) => {
    const c = args.join(" ");
    if (options.fail && c.includes(options.fail)) {
      return Promise.reject(new Error("Git inspection failed"));
    }
    if (args[0] === "fetch") return Promise.resolve("");
    if (c.includes("refs/remotes/origin/")) {
      return Promise.resolve(
        [`main ${main}`, ...(options.branches ?? []).map((name) => `${name} ${branch}`)].join("\n"),
      );
    }
    if (args[0] === "for-each-ref") return Promise.resolve((options.tags ?? []).join("\n"));
    if (args[0] === "rev-parse") return Promise.resolve(release);
    if (args[0] === "ls-tree") {
      const has = args[2] === branch
        ? options.feature !== false
        : options.shipped?.includes(args[2]);
      // Peeling is exercised separately with real annotated/lightweight Git tags.
      return Promise.resolve(has ? "crates/gukhanmun-loanwords/Cargo.toml" : "");
    }
    if (args[0] === "merge-base") return Promise.resolve(options.ancestor === false ? "" : release);
    return Promise.reject(new Error(`unexpected inspection ${c}`));
  };
}
Deno.test("targets fall back before first shipment and fail closed after shipment", async () => {
  assert.deepEqual(
    await targets(catalog({ tags: ["0.2.0", "0.3.0-rc.1"], branches: ["0.2-maintenance"] })),
    [{ branch: "main", oid: main }],
  );
  const shipped = { tags: ["0.9.0", "0.10.1"], shipped: [release] };
  assert.deepEqual(
    await targets(
      catalog({
        ...shipped,
        branches: ["0.9-maintenance", "0.10-maintenance", "0.11-maintenance"],
      }),
    ),
    [{ branch: "0.10-maintenance", oid: branch }, { branch: "main", oid: main }],
  );
  for (
    const options of [
      { branches: ["0.9-maintenance"] },
      { branches: ["0.10-maintenance"], feature: false },
      { branches: ["0.10-maintenance"], ancestor: false },
    ]
  ) {
    await assert.rejects(
      () => targets(catalog({ ...shipped, ...options })),
      /0.10.1.*0.10-maintenance/,
    );
  }
  for (const fail of ["fetch", "refs/tags/", "rev-parse", "ls-tree"]) {
    await assert.rejects(() => targets(catalog({ ...shipped, fail })), /inspection failed/);
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

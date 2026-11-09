// Gukhanmun: Tests target-owned fragment layouts and their artifact boundaries.
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
import { parseLayout, pinnedLayout } from "./changelog.ts";
import { configuration } from "./changelog_fixture.ts";
import { hash, json } from "./model.ts";
import { update as updateDictionary } from "./update.ts";
import { update as updateLoanword } from "../loanwords/update.ts";
import { validate as validateDictionary } from "./publish.ts";
import { validate as validateLoanword } from "../loanwords/publish.ts";

Deno.test("target layouts require complete matching source, ID and directory families", async () => {
  for (const layout of ["legacy", "language"] as const) {
    assert.equal(await parseLayout(configuration(layout)), layout);
  }
  for (
    const invalid of [
      "[[section-patterns]", // Invalid TOML must not be mistaken for a known layout.
      configuration("legacy").replace('directory = "crates/{name}"', 'directory = "rust/{name}"'),
      configuration("language").replace('id = "{name} (Rust)"', 'id = "{name}"'),
      configuration("legacy").replace('source = "packages/{name}"', 'source = "other/{name}"'),
      configuration("language") + '\n[[sections]]\nid="gukhanmun (Rust)"\ndirectory="elsewhere"\n',
      configuration("language").replace('directory = "changes.d"', 'directory = "other"'),
      configuration("language").replace('path = "CHANGES.md"', 'path = "OTHER.md"'),
      configuration("legacy").replace(
        '[changelog]\npath = "CHANGES.md"\ntitle = "Gukhanmun changelog"\nmaterialize = true\nregion-detection = "marker"\n',
        'changelog = "OTHER.md"\n',
      ),
    ]
  ) await assert.rejects(() => parseLayout(invalid), /target/);
  const oid = "a".repeat(40), calls: string[][] = [];
  const context = await pinnedLayout((_cmd, args) => {
    calls.push(args);
    return Promise.resolve(configuration("legacy"));
  }, oid);
  assert.deepEqual(context, { base: oid, layout: "legacy" });
  assert.ok(Object.isFrozen(context));
  assert.deepEqual(calls, [["show", `${oid}:sacho.toml`]]);
  await assert.rejects(() => pinnedLayout(() => Promise.resolve(""), "../main"), /invalid pinned/);
});

Deno.test("artifact validation permits exactly the pinned target fragment layout", async () => {
  const root = await Deno.makeTempDir();
  const base = "a".repeat(40), bytes = new TextEncoder().encode(" - Updated synthetic data.\n");
  try {
    await Deno.writeFile(`${root}/file-0`, bytes);
    for (const layout of ["legacy", "language"] as const) {
      const family = layout === "legacy" ? "crates" : "rust";
      const other = layout === "legacy" ? "rust" : "crates";
      const dictionary = {
        version: 1,
        base,
        controller: base,
        tree: base,
        target: "0.10-maintenance",
        status: "updated",
        summary: "Synthetic",
        files: [{
          path: `changes.d/${family}/gukhanmun/stdict-data-update.md`,
          blob: "file-0",
          sha256: await hash(bytes),
          mode: "100644",
        }],
      };
      const loanword = {
        base,
        controller: base,
        tree: base,
        target: "main",
        primary: "main",
        fingerprint: "b".repeat(64),
        status: "updated",
        files: [{
          path: `changes.d/${family}/gukhanmun/loanword-data-update.md`,
          file: "file-0",
          sha256: await hash(bytes),
        }],
      };
      for (
        const [manifest, validate] of [
          [dictionary, validateDictionary],
          [loanword, validateLoanword],
        ] as const
      ) {
        await Deno.writeFile(`${root}/manifest.json`, json(manifest));
        await validate(root, { base, layout });
        await assert.rejects(() => validate(root, { base: "c".repeat(40), layout }));
        await Deno.writeFile(
          `${root}/manifest.json`,
          json({
            ...manifest,
            files: manifest.files.map((file) => ({
              ...file,
              path: file.path.replace(`/${family}/`, `/${other}/`),
            })),
          }),
        );
        await assert.rejects(() => validate(root, { base, layout }), /path/);
      }
    }
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});

Deno.test("unsupported layouts stop both updaters before extraction or collection", async () => {
  const root = await Deno.makeTempDir();
  try {
    await Deno.writeTextFile(
      `${root}/sacho.toml`,
      configuration("legacy").replace(
        'directory = "packages/{name}"',
        'directory = "javascript/{name}"',
      ),
    );
    const noExecution = () => {
      throw new Error("tools must not run for unsupported target layout");
    };
    await assert.rejects(
      () => updateDictionary(root, "/unused-source", noExecution),
      /target changelog/,
    );
    await assert.rejects(
      () => updateLoanword(root, "update", "/unused-source"),
      /target changelog/,
    );
    assert.deepEqual([...Deno.readDirSync(root)].map((entry) => entry.name), ["sacho.toml"]);
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});

// Gukhanmun: Tests reversible offline dictionary updates and stable fragments.
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
import { fileURLToPath } from "node:url";
import { data, fragments, hash, json, kinds, notices, outputs, updatePaths } from "./model.ts";
import { mappingReport, run, update } from "./update.ts";
import type { Run, Source } from "./update.ts";
const project = fileURLToPath(new URL("../../", import.meta.url));
const encoder = new TextEncoder();
const old = encoder.encode("hanja\thangul\n甲\t갑\n乙\t을\n");
const revised = encoder.encode("hanja\thangul\n甲\t가\n甲\t갑\n丙\t병\n");
async function fixture() {
  const root = await Deno.makeTempDir();
  await Deno.copyFile(`${project}/mise.toml`, `${root}/mise.toml`);
  await Deno.copyFile(`${project}/sacho.toml`, `${root}/sacho.toml`);
  for (const path of ["Cargo.toml", "Cargo.lock", "scripts/dictionaries/interface.json"]) {
    await Deno.mkdir(`${root}/${path.split("/").slice(0, -1).join("/")}`, { recursive: true });
    await Deno.writeTextFile(`${root}/${path}`, "synthetic");
  }
  await Deno.mkdir(`${root}/changes.d`);
  await Deno.writeTextFile(`${root}/changes.d/next`, "99.0.0\n");
  await Deno.writeTextFile(
    `${root}/CHANGES.md`,
    "Gukhanmun changelog\n==================\n\n<!-- sacho:unreleased:begin -->\n\n<!-- sacho:unreleased:end -->\n",
  );
  for (const kind of kinds) {
    await Deno.mkdir(`${root}/${data(kind)}`, { recursive: true });
    await Deno.writeFile(
      `${root}/${data(kind)}/metadata.json`,
      json({
        source: `${kind}-json-20260603`,
        build_date: "2026-06-03T00:00:00Z",
        license: "synthetic",
      }),
    );
    for (const name of outputs[kind]) await Deno.writeFile(`${root}/${data(kind)}/${name}`, old);
    for (const path of fragments(kind)) {
      await Deno.mkdir(`${root}/${path.slice(0, path.lastIndexOf("/"))}`, { recursive: true });
      const section = path.replace(/^changes.d\//, "").replace(/\/[^/]+$/, "");
      await Deno.mkdir(`${root}/${section}`, { recursive: true });
      await Deno.writeTextFile(`${root}/${path}`, " - Preserved independent note. [[#29]]\n");
    }
  }
  for (const path of notices) {
    await Deno.mkdir(`${root}/${path.split("/").slice(0, -1).join("/")}`, { recursive: true });
    await Deno.writeTextFile(
      `${root}/${path}`,
      "Attribution\n===========\n\n - Snapshot: *전체 내려받기\\_표준국어대사전\\_JSON\\_20260606.zip*\n - Snapshot: *전체 내려받기\\_우리말샘\\_json\\_20260603.zip*\n",
    );
  }
  const sources = Object.fromEntries(kinds.map((kind) => [kind, {
    status: "downloaded",
    source: {
      version: 1,
      dictionary: kind,
      link_key: "123",
      archive_sha256: "a".repeat(64),
      archive_date: "2026-10-12",
      archive_filename: `전체 내려받기_${
        kind === "stdict" ? "표준국어대사전" : "우리말샘"
      }_JSON_20261012.zip`,
    } as Source,
  }]));
  return { root, sources };
}
async function snapshot(root: string) {
  return await Promise.all(updatePaths.map(async (path) => {
    try {
      return [path, await hash(await Deno.readFile(`${root}/${path}`))];
    } catch (e) {
      if (e instanceof Deno.errors.NotFound) return [path, null];
      throw e;
    }
  }));
}
Deno.test("reports preserve full candidate row groups", async () => {
  assert.deepEqual(await mappingReport(old, revised), {
    sha256: await hash(revised),
    rows: 3,
    keys: 2,
    added: 1,
    removed: 1,
    changed: 1,
  });
});
Deno.test("two-dictionary transaction, idempotent fragments and rollback after formatter failure", async () => {
  const { root, sources } = await fixture();
  let fail: "none" | "extract" | "format" = "none";
  const execute: Run = async (cmd, args, cwd, env = {}) => {
    if (cmd === "python" && args.includes("validate")) {
      return JSON.stringify({ version: 1, dictionaries: sources });
    }
    if (cmd === "mise" && args[0] === "run") {
      if (fail === "extract" && args[1] === "opendict-extract") {
        throw new Error("synthetic candidate set changed");
      }
      const kind = args[1] === "stdict-extract" ? "stdict" : "opendict";
      for (const name of outputs[kind]) {
        await Deno.writeFile(`${env.GUKHANMUN_DICTS_OUTPUT_DIR}/${name}`, revised);
      }
      return "";
    }
    if (cmd === "mise" && args.includes("hongdown") && fail === "format") {
      throw new Error("synthetic formatter failure");
    }
    return await run(cmd, args, cwd, { ...env, MISE_TRUSTED_CONFIG_PATHS: root });
  };
  try {
    // Make the starting fixture a valid materialized Sacho tree.
    await run("mise", ["exec", "--", "sacho", "sync", "--force"], root, {
      MISE_TRUSTED_CONFIG_PATHS: root,
    });
    await run("mise", ["exec", "--", "sacho", "fmt"], root, { MISE_TRUSTED_CONFIG_PATHS: root });
    const before = await snapshot(root);
    fail = "extract";
    await assert.rejects(() => update(root, "/unused", execute), /candidate set changed/);
    assert.deepEqual(await snapshot(root), before);
    fail = "format";
    await assert.rejects(() => update(root, "/unused", execute), /formatter failure/);
    assert.deepEqual(await snapshot(root), before);
    fail = "none";
    const result = await update(root, "/unused", execute);
    assert.equal(result.status, "updated");
    assert.match(result.summary, /1 added, 1 removed, 1 changed/);
    for (const kind of kinds) {
      const text = await Deno.readTextFile(`${root}/${fragments(kind)[0]}`);
      assert.match(text, /Preserved independent note/);
      assert.equal((text.match(/Updated the bundled/g) ?? []).length, 1);
    }
    const after = await snapshot(root);
    assert.equal((await update(root, "/unused", execute)).status, "unchanged");
    assert.deepEqual(await snapshot(root), after);
    // Recipe-only drift re-extracts but does not produce a provenance PR.
    await Deno.writeTextFile(`${root}/Cargo.toml`, "changed unrelated code inputs");
    assert.equal((await update(root, "/unused", execute)).status, "unchanged");
    assert.deepEqual(await snapshot(root), after);
    // A re-issued identifier with unchanged data is provenance-only. An empty
    // notice list must never invoke Hongdown on the entire repository.
    const unrelated = "#   Keep this unrelated Markdown byte for byte\n";
    await Deno.writeTextFile(`${root}/unrelated.md`, unrelated);
    sources.stdict.source.link_key = "456";
    assert.equal((await update(root, "/unused", execute)).status, "updated");
    assert.equal(await Deno.readTextFile(`${root}/unrelated.md`), unrelated);
    // Skips are revalidated against this root, never trusted from the collector.
    sources.stdict = { status: "unchanged", source: { ...sources.stdict.source, link_key: "999" } };
    await assert.rejects(() => update(root, "/unused", execute), /skipped source/);
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});

Deno.test("local collection helper accepts filesystem paths with spaces", async () => {
  const root = await Deno.makeTempDir({ prefix: "dictionary checkout " });
  try {
    await Deno.copyFile(`${project}/scripts/dictionaries/local.ts`, `${root}/local.ts`);
    await Deno.writeTextFile(
      `${root}/collect.py`,
      'import pathlib,sys\nassert sys.argv[1] == "collect"\npathlib.Path(sys.argv[2]).write_text("synthetic collector reached")\n',
    );
    const result = await new Deno.Command("deno", {
      args: ["run", "-A", `${root}/local.ts`, `${root}/result`],
      cwd: root,
      stdout: "piped",
      stderr: "piped",
    }).output();
    assert.ok(result.success, new TextDecoder().decode(result.stderr));
    assert.equal(await Deno.readTextFile(`${root}/result`), "synthetic collector reached");
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});

// Gukhanmun: Tests dictionary artifacts and publication in local Git repositories.
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
import { bundle, proposalBody, publish, validate } from "./publish.ts";
import type { Manifest } from "./publish.ts";
import { command, hash, json } from "./model.ts";
import type { Execute } from "./model.ts";
import { botBranch } from "./targets.ts";
const bot = "41898282+github-actions[bot]@users.noreply.github.com";
const path = "crates/gukhanmun-stdict/data/stdict.tsv";
Deno.test("artifact validation refuses forged paths, blobs, modes and controller syntax", async () => {
  const directory = await Deno.makeTempDir();
  try {
    const bytes = new TextEncoder().encode("hanja\thangul\n甲\t갑\n");
    await Deno.writeFile(`${directory}/file-0`, bytes);
    const file = { path, blob: "file-0", sha256: await hash(bytes), mode: "100644" };
    const m: Manifest = {
      version: 1,
      controller: "a".repeat(40),
      base: "b".repeat(40),
      tree: "c".repeat(40),
      target: "0.10-maintenance",
      status: "updated",
      summary: "Public report",
      files: [file],
    };
    const save = (v: unknown) => Deno.writeFile(`${directory}/manifest.json`, json(v));
    await save(m);
    assert.equal((await validate(directory)).target, "0.10-maintenance");
    for (
      const files of [
        [{ ...file, path: "../escape" }],
        [{ ...file, path: "crates/gukhanmun-stdict/build.rs" }],
        [{ ...file, blob: "../file-0" }],
        [{ ...file, mode: "100755" }],
        [{ ...file, sha256: "0".repeat(64) }],
        [file, file],
      ]
    ) {
      await save({ ...m, files });
      await assert.rejects(() => validate(directory));
    }
    for (
      const value of [{ ...m, target: "main" }, { ...m, status: "unchanged" }, {
        ...m,
        controller: "arbitrary",
      }]
    ) {
      await save(value);
      await assert.rejects(() => validate(directory));
    }
    await save(m);
    await Deno.writeTextFile(`${directory}/file-0`, "corruption");
    await assert.rejects(() => validate(directory), /checksum/);
    if (Deno.build.os !== "windows") {
      await Deno.remove(`${directory}/file-0`);
      await Deno.symlink("manifest.json", `${directory}/file-0`);
      await assert.rejects(() => validate(directory), /symlink/);
    }
  } finally {
    await Deno.remove(directory, { recursive: true });
  }
});
async function fixture() {
  const parent = await Deno.makeTempDir({ prefix: "dictionary-publish-" }),
    root = `${parent}/work`,
    remote = `${parent}/remote`;
  await Deno.mkdir(root);
  const git: Execute = (cmd, args, optional) => command(cmd, args, optional, root);
  await git("git", ["init", "--quiet", "--initial-branch=main"]);
  for (
    const [key, value] of [
      ["user.name", "github-actions[bot]"],
      ["user.email", bot],
      ["commit.gpgsign", "false"],
      ["tag.gpgsign", "false"],
      ["push.gpgsign", "false"],
      ["core.hooksPath", `${parent}/no-hooks`],
      ["core.autocrlf", "false"],
    ]
  ) await git("git", ["config", key, value]);
  const files = {
    "scripts/dictionaries/interface.json": '{"version":1}',
    "mise.toml":
      "[tasks.stdict-extract]\n[tasks.opendict-extract]\n[tasks.ci]\n# GUKHANMUN_DICTS_OUTPUT_DIR",
    "sacho.toml": "# synthetic",
    "crates/gukhanmun-stdict/data/metadata.json": '{"source":"s","build_date":"d","license":"l"}',
    "crates/gukhanmun-opendict/data/metadata.json": '{"source":"s","build_date":"d","license":"l"}',
    [path]: "hanja\thangul\n甲\t갑\n",
  };
  for (const [p, text] of Object.entries(files)) {
    await Deno.mkdir(`${root}/${p.slice(0, p.lastIndexOf("/")) || "."}`, { recursive: true });
    await Deno.writeTextFile(`${root}/${p}`, text);
  }
  await git("git", ["add", "."]);
  await git("git", ["commit", "--quiet", "-m", "Synthetic base"]);
  const base = await git("git", ["rev-parse", "HEAD"]);
  await git("git", ["branch", "0.10-maintenance"]);
  await git("git", ["init", "--bare", "--quiet", remote]);
  await git("git", ["remote", "add", "origin", remote]);
  await git("git", ["push", "--quiet", "origin", "--all"]);
  return { parent, root, base, git };
}
async function workflow<T>(sha: string, callback: () => Promise<T>): Promise<T> {
  const values = {
    GITHUB_ACTIONS: "true",
    GITHUB_REPOSITORY: "dahlia/gukhanmun",
    GITHUB_REF: "refs/heads/main",
    GITHUB_SHA: sha,
  };
  const old = new Map(Object.keys(values).map((k) => [k, Deno.env.get(k)]));
  try {
    for (const [k, v] of Object.entries(values)) Deno.env.set(k, v);
    return await callback();
  } finally {
    for (const [k, v] of old) {
      if (v === undefined) Deno.env.delete(k);
      else Deno.env.set(k, v);
    }
  }
}
Deno.test("real Git publication binds tested trees and refuses human or stale proposals", async () => {
  for (
    const scenario of [
      "new",
      "identical",
      "human",
      "wrong-tree",
      "wrong-controller",
      "advanced",
      "newer-line",
      "wrong-pr-base",
      "symlink-ancestor",
    ] as const
  ) {
    const { parent, root, base, git } = await fixture();
    try {
      await workflow(base, async () => {
        const directory = `${parent}/artifact`, calls: string[] = [];
        await Deno.writeTextFile(`${root}/${path}`, "hanja\thangul\n甲\t가\n");
        await bundle(
          directory,
          root,
          { status: "updated", summary: "Public source change", sources: {} },
          "0.10-maintenance",
          git,
        );
        let m = await validate(directory);
        await git("git", ["restore", "--staged", "--worktree", "."]);
        if (scenario === "wrong-tree" || scenario === "wrong-controller") {
          m = { ...m, [scenario === "wrong-tree" ? "tree" : "controller"]: "f".repeat(40) };
          await Deno.writeFile(`${directory}/manifest.json`, json(m));
        }
        let existing = scenario === "identical" || scenario === "human" ||
          scenario === "wrong-pr-base";
        if (existing) {
          await Deno.writeTextFile(`${root}/${path}`, "hanja\thangul\n甲\t가\n");
          await git("git", ["add", path]);
          if (scenario === "human") {
            await git("git", [
              "-c",
              "user.email=human@example.test",
              "commit",
              "--quiet",
              "-m",
              "Human change",
            ]);
          } else await git("git", ["commit", "--quiet", "-m", "Previous proposal"]);
          await git("git", [
            "push",
            "--quiet",
            "origin",
            `HEAD:refs/heads/${botBranch("0.10-maintenance")}`,
          ]);
          await git("git", ["switch", "--detach", base]);
        }
        if (scenario === "advanced") {
          await Deno.writeTextFile(`${root}/extra`, "target advancement");
          await git("git", ["add", "extra"]);
          await git("git", ["commit", "--quiet", "-m", "Advance target"]);
          await git("git", ["push", "--quiet", "origin", "HEAD:refs/heads/0.10-maintenance"]);
          await git("git", ["switch", "--detach", base]);
        }
        if (scenario === "newer-line") {
          await git("git", ["push", "--quiet", "origin", "HEAD:refs/heads/1.0-maintenance"]);
        }
        if (scenario === "symlink-ancestor") {
          if (Deno.build.os === "windows") return;
          await Deno.rename(`${root}/crates`, `${root}/real-crates`);
          await Deno.symlink("real-crates", `${root}/crates`);
        }
        const execute: Execute = async (cmd, args, optional) => {
          if (cmd === "gh") {
            calls.push(args.join(" "));
            if (args[0] === "api" && args[1].includes("/pulls?")) {
              return JSON.stringify(
                existing
                  ? [{
                    number: 9,
                    base: { ref: scenario === "wrong-pr-base" ? "main" : "0.10-maintenance" },
                    body: "Human note",
                  }]
                  : [],
              );
            }
            if (args[0] === "pr" && args[1] === "create") existing = true;
            return "";
          }
          calls.push(`git ${args.join(" ")}`);
          return await git(cmd, args, optional);
        };
        if (
          [
            "human",
            "wrong-tree",
            "wrong-controller",
            "advanced",
            "newer-line",
            "wrong-pr-base",
            "symlink-ancestor",
          ].includes(scenario)
        ) {
          await assert.rejects(() => publish(directory, root, execute));
          assert.ok(
            !calls.some((s) =>
              s.startsWith("git push") || s.startsWith("pr create") || s.includes("/statuses/")
            ),
            scenario,
          );
        } else {
          await publish(directory, root, execute);
          const head = (await git("git", [
            "ls-remote",
            "origin",
            `refs/heads/${botBranch("0.10-maintenance")}`,
          ])).split(/\s/)[0];
          assert.equal(await git("git", ["rev-parse", `${head}^{tree}`]), m.tree);
          assert.ok(calls.some((s) => s.includes(`/statuses/${head}`)));
          assert.equal(calls.some((s) => s.startsWith("git push")), scenario === "new");
          if (scenario === "identical") assert.ok(!calls.some((s) => s.startsWith("pr edit")));
        }
      });
    } finally {
      await Deno.remove(parent, { recursive: true });
    }
  }
});
Deno.test("bundling refuses unrelated files and no-change results with writes", async () => {
  const { parent, root, base, git } = await fixture();
  try {
    await workflow(base, async () => {
      await Deno.writeTextFile(`${root}/unreviewed`, "unrelated");
      await assert.rejects(
        () =>
          bundle(
            `${parent}/artifact`,
            root,
            { status: "updated", summary: "", sources: {} },
            "0.10-maintenance",
            git,
          ),
        /non-data changes/,
      );
      await Deno.remove(`${root}/unreviewed`);
      await Deno.writeTextFile(`${root}/${path}`, "changed");
      await assert.rejects(
        () =>
          bundle(
            `${parent}/artifact`,
            root,
            { status: "unchanged", summary: "", sources: {} },
            "0.10-maintenance",
            git,
          ),
        /inconsistent/,
      );
    });
  } finally {
    await Deno.remove(parent, { recursive: true });
  }
});

Deno.test("proposal reports preserve human text outside owned markers", () => {
  const first = proposalBody("Human introduction", "First report", "base");
  const second = proposalBody(first + "\nHuman follow-up", "Second report", "base");
  assert.match(second, /^Human introduction/);
  assert.match(second, /Human follow-up$/);
  assert.match(second, /Second report/);
  assert.ok(!second.includes("First report"));
  assert.throws(
    () => proposalBody("<!-- dictionary-update:begin -->", "report", "base"),
    /ambiguous/,
  );
});

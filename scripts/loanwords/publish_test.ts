// Gukhanmun: Tests loanword publication artifact validation.
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
import { validate } from "./publish.ts";
import { hash } from "./update.ts";
Deno.test("publication rejects traversal, checksum changes and inconsistent conflict artifacts", async () => {
  const dir = await Deno.makeTempDir();
  try {
    const bytes = new TextEncoder().encode("synthetic audit\n");
    await Deno.writeFile(`${dir}/file-0`, bytes);
    const file = {
      path: "crates/gukhanmun-loanwords/data/pending-update.json",
      file: "file-0",
      sha256: await hash(bytes),
    };
    const manifest = {
      base: "a".repeat(40),
      tree: "a".repeat(40),
      controller: "a".repeat(40),
      target: "main",
      primary: "main",
      fingerprint: "b".repeat(64),
      status: "pending",
      files: [file],
    };
    const save = async (value: unknown) =>
      await Deno.writeTextFile(`${dir}/manifest.json`, JSON.stringify(value));
    await save(manifest);
    assert.equal((await validate(dir)).status, "pending");
    for (
      const files of [
        [{ ...file, path: "../../credentials" }],
        [{ ...file, file: "../file-0" }],
        [{ ...file, sha256: "0".repeat(64) }],
        [{ ...file, path: "crates/gukhanmun-loanwords/data/snapshot.json" }],
        [file, file],
      ]
    ) {
      await save({ ...manifest, files });
      await assert.rejects(() => validate(dir));
    }
    await save({ ...manifest, status: "unchanged" });
    await assert.rejects(() => validate(dir));
    await save(manifest);
    await Deno.writeTextFile(`${dir}/file-0`, "modified in transit");
    await assert.rejects(() => validate(dir));
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});

Deno.test("bot lifecycle preserves human commits and skips identical unmerged generations", async () => {
  const { publish } = await import("./publish.ts");
  const { generationFingerprint } = await import("./update.ts");
  const directory = await Deno.makeTempDir();
  const variables = ["GITHUB_ACTIONS", "GITHUB_REPOSITORY", "GITHUB_REF", "GITHUB_SHA"];
  const before = variables.map((v) => Deno.env.get(v));
  try {
    Deno.env.set("GITHUB_ACTIONS", "true");
    Deno.env.set("GITHUB_REPOSITORY", "dahlia/gukhanmun");
    Deno.env.set("GITHUB_REF", "refs/heads/main");
    Deno.env.set("GITHUB_SHA", "a".repeat(40));
    const base = "a".repeat(40), old = "b".repeat(40);
    const snapshot = { records_sha256: "c".repeat(64) };
    const report = { decisions_sha256: "d".repeat(64), tsv_sha256: "e".repeat(64), audit: {} };
    const fingerprint = await generationFingerprint(snapshot, report);
    const manifest = {
      base,
      tree: "e".repeat(40),
      target: "main",
      primary: "main",
      controller: base,
      fingerprint,
      status: "updated",
      files: [],
    };
    const bot = "41898282+github-actions[bot]@users.noreply.github.com";
    let human = false, advanced = false, draft = false;
    const calls: string[] = [];
    const mock = (cmd: string, args: string[]): Promise<string> => {
      const c = `${cmd} ${args.join(" ")}`;
      calls.push(c);
      if (c.includes("refs/remotes/origin/")) {
        return Promise.resolve(`main ${advanced ? "f".repeat(40) : base}`);
      }
      if (c.startsWith("git for-each-ref")) return Promise.resolve("");
      if (c === "git write-tree" || c === `git rev-parse ${old}^{tree}`) {
        return Promise.resolve(manifest.tree);
      }
      if (c === `git show -s --format=%P ${old}`) return Promise.resolve(base);
      if (c.startsWith("git ls-remote")) {
        return Promise.resolve(`${old}\trefs/heads/automation/loanword-data`);
      }
      if (c.startsWith("git log")) {
        return Promise.resolve(human ? "human@example.test|human@example.test" : `${bot}|${bot}`);
      }
      if (c.includes("/pulls?head=")) {
        return Promise.resolve(JSON.stringify([{ number: 99, draft, base: { ref: "main" } }]));
      }
      if (c.endsWith("/pending-update.json")) return Promise.resolve("");
      if (c.endsWith("/snapshot.json")) return Promise.resolve(JSON.stringify(snapshot));
      if (c.endsWith("/report.json")) return Promise.resolve(JSON.stringify(report));
      if (
        c.startsWith("git push") ||
        (!draft && c.includes("gh pr edit"))
      ) {
        throw new Error("identical data must not refresh the branch");
      }
      return Promise.resolve("");
    };
    await Deno.writeTextFile(`${directory}/manifest.json`, JSON.stringify(manifest));
    await publish(directory, mock, directory);
    assert.ok(calls.some((c) => c.includes("/statuses/")));
    assert.ok(!calls.some((c) => c.startsWith("git push")));
    calls.length = 0;
    draft = true;
    await publish(directory, mock, directory);
    assert.ok(calls.some((c) => c.startsWith("gh pr edit 99")));
    assert.ok(calls.some((c) => c.startsWith("gh pr ready 99")));
    assert.ok(!calls.some((c) => c.startsWith("git push")));
    draft = false;
    calls.length = 0;
    human = true;
    await assert.rejects(() => publish(directory, mock, directory), /human commits/);
    assert.ok(!calls.some((c) => c.startsWith("gh ")));
    human = false;
    advanced = true;
    await assert.rejects(() => publish(directory, mock, directory), /main advanced/);
    advanced = false;
    await Deno.writeTextFile(
      `${directory}/manifest.json`,
      JSON.stringify({ ...manifest, status: "unchanged" }),
    );
    await publish(directory, mock, directory);
    assert.ok(calls.some((c) => c.startsWith("gh pr close 99")));
  } finally {
    variables.forEach((v, i) => {
      if (before[i] === undefined) Deno.env.delete(v);
      else Deno.env.set(v, before[i]!);
    });
    await Deno.remove(directory, { recursive: true });
  }
});

const botEmail = "41898282+github-actions[bot]@users.noreply.github.com";
const dataDirectory = "crates/gukhanmun-loanwords/data";
async function gitFixture(maintenance: boolean) {
  const parent = await Deno.makeTempDir({ prefix: "loanword-publish-git-" });
  const root = `${parent}/work`, remote = `${parent}/remote`;
  const git = async (args: string[], optional = false): Promise<string> => {
    const result = await new Deno.Command("git", {
      args,
      cwd: root,
      stdout: "piped",
      stderr: "piped",
    }).output();
    if (!result.success && !optional) throw new Error(new TextDecoder().decode(result.stderr));
    return new TextDecoder().decode(result.stdout).trim();
  };
  await Deno.mkdir(root);
  await git(["init", "--quiet", "--initial-branch=main"]);
  await git(["config", "user.name", "github-actions[bot]"]);
  await git(["config", "user.email", botEmail]);
  await git(["config", "commit.gpgsign", "false"]);
  await git(["config", "tag.gpgsign", "false"]);
  await git(["config", "push.gpgsign", "false"]);
  await git(["config", "core.hooksPath", `${parent}/no-hooks`]);
  await git(["config", "core.autocrlf", "false"]);
  await Deno.mkdir(`${root}/${dataDirectory}`, { recursive: true });
  await Deno.writeTextFile(
    `${root}/crates/gukhanmun-loanwords/Cargo.toml`,
    "synthetic feature marker\n",
  );
  await Deno.writeTextFile(
    `${root}/${dataDirectory}/snapshot.json`,
    '{"records_sha256":"synthetic"}\n',
  );
  await Deno.writeTextFile(
    `${root}/${dataDirectory}/report.json`,
    '{"tsv_sha256":"synthetic","audit":{}}\n',
  );
  await git(["add", "."]);
  await git(["commit", "--quiet", "-m", "Synthetic base"]);
  const base = await git(["rev-parse", "HEAD"]);
  if (maintenance) {
    await git(["branch", "0.10-maintenance"]);
    await git(["tag", "-a", "0.10.0", "-m", "Synthetic annotated release"]);
    await git(["tag", "0.10.1"]);
  }
  await git(["init", "--bare", "--quiet", remote]);
  await git(["remote", "add", "origin", remote]);
  await git(["push", "--quiet", "origin", "--all"]);
  await git(["push", "--quiet", "origin", "--tags"]);
  return { parent, root, base, git };
}
async function workflowEnvironment(controller: string, action: () => Promise<void>) {
  const values = {
    GITHUB_ACTIONS: "true",
    GITHUB_REPOSITORY: "dahlia/gukhanmun",
    GITHUB_REF: "refs/heads/main",
    GITHUB_SHA: controller,
  };
  const before = new Map(Object.keys(values).map((key) => [key, Deno.env.get(key)]));
  try {
    for (const [key, value] of Object.entries(values)) Deno.env.set(key, value);
    await action();
  } finally {
    for (const [key, value] of before) {
      if (value === undefined) Deno.env.delete(key);
      else Deno.env.set(key, value);
    }
  }
}

Deno.test("real Git publication rebuilds forged bot trees and parents, and binds trusted status", async () => {
  const { publish, candidate } = await import("./publish.ts");
  const { botBranch, targets } = await import("./targets.ts");
  for (
    const scenario of [
      "extra-file",
      "wrong-parent",
      "identical",
      "wrong-tree",
      "human",
      "wrong-pr-base",
      "advanced",
    ]
  ) {
    const f = await gitFixture(true);
    try {
      const target = "0.10-maintenance", branch = botBranch(target);
      const artifact = `${f.parent}/artifact`;
      await Deno.mkdir(artifact);
      const bytes = new TextEncoder().encode('{"synthetic":"validated update"}\n');
      await Deno.writeFile(`${artifact}/file-0`, bytes);
      const path = `${dataDirectory}/metadata.json`;
      await Deno.writeFile(`${f.root}/${path}`, bytes);
      await f.git(["add", "--", path]);
      const expectedTree = await f.git(["write-tree"]);
      await f.git(["reset", "--hard", f.base]);
      if (scenario === "wrong-parent") {
        await f.git(["commit", "--quiet", "--allow-empty", "-m", "Forged intermediate history"]);
      }
      await Deno.writeFile(`${f.root}/${path}`, bytes);
      if (scenario === "extra-file") {
        await Deno.writeTextFile(`${f.root}/unrelated-script.sh`, "untrusted\n");
      }
      if (scenario === "human") await f.git(["config", "user.email", "human@example.test"]);
      await f.git(["add", "."]);
      await f.git(["commit", "--quiet", "-m", "Bot update"]);
      const old = await f.git(["rev-parse", "HEAD"]);
      await f.git(["push", "--quiet", "origin", `HEAD:refs/heads/${branch}`]);
      await f.git(["switch", "--detach", f.base]);
      // Main may advance without invalidating an unchanged maintenance target.
      await f.git(["commit", "--quiet", "--allow-empty", "-m", "Independent main advance"]);
      await f.git(["push", "--quiet", "origin", "HEAD:refs/heads/main"]);
      await f.git(["switch", "--detach", f.base]);
      if (scenario === "advanced") {
        await f.git(["commit", "--quiet", "--allow-empty", "-m", "Maintenance advance"]);
        await f.git(["push", "--quiet", "origin", "HEAD:refs/heads/0.10-maintenance"]);
        await f.git(["switch", "--detach", f.base]);
      }
      const calls: string[][] = [];
      const execute = (cmd: string, args: string[], optional = false): Promise<string> => {
        if (cmd === "git") return f.git(args, optional);
        assert.equal(cmd, "gh");
        calls.push(args);
        return Promise.resolve(
          args[1]?.includes("/pulls?")
            ? JSON.stringify([{
              number: 99,
              draft: false,
              base: { ref: scenario === "wrong-pr-base" ? "main" : target },
            }])
            : "",
        );
      };
      const prior = `${f.parent}/candidate.json`;
      await candidate(prior, target, execute);
      assert.equal(JSON.parse(await Deno.readTextFile(prior)).records_sha256, "synthetic");
      const selected = await targets(execute);
      assert.equal(selected[0].branch, target); // Both annotated and lightweight release tags are inspected.
      await Deno.writeTextFile(
        `${artifact}/manifest.json`,
        JSON.stringify({
          base: f.base,
          controller: f.base,
          target,
          primary: target,
          tree: scenario === "wrong-tree" ? "0".repeat(40) : expectedTree,
          status: "updated",
          fingerprint: "c".repeat(64),
          files: [{ path, file: "file-0", sha256: await hash(bytes) }],
        }),
      );
      await workflowEnvironment(f.base, async () => {
        if (["human", "wrong-tree", "wrong-pr-base", "advanced"].includes(scenario)) {
          const error = scenario === "human"
            ? /human commits/
            : scenario === "wrong-tree"
            ? /differs from tested tree/
            : scenario === "wrong-pr-base"
            ? /wrong base/
            : /advanced during collection/;
          await assert.rejects(() => publish(artifact, execute, f.root), error);
          assert.ok(!calls.some((args) => args[1]?.includes("/statuses/")));
          return;
        }
        await publish(artifact, execute, f.root);
        const tip = (await f.git(["ls-remote", "origin", `refs/heads/${branch}`])).split(/\s/)[0];
        assert.equal(await f.git(["rev-parse", `${tip}^{tree}`]), expectedTree);
        assert.equal(await f.git(["show", "-s", "--format=%P", tip]), f.base);
        assert.equal(tip === old, scenario === "identical");
        assert.ok(
          calls.some((args) =>
            args[1]?.endsWith(`/statuses/${tip}`) && args.includes("state=success")
          ),
        );
        if (scenario !== "identical") {
          assert.ok(!calls.some((args) => args[1]?.endsWith(`/statuses/${old}`)));
        }
      });
    } finally {
      await Deno.remove(f.parent, { recursive: true });
    }
  }
});

Deno.test("bundling rejects untested paths and records only the complete staged update tree", async () => {
  const f = await gitFixture(false);
  try {
    const artifact = `${f.parent}/artifact`, resultFile = `${f.parent}/result.json`;
    const path = `${dataDirectory}/metadata.json`;
    await Deno.writeTextFile(`${f.root}/${path}`, "synthetic update\n");
    await Deno.writeTextFile(
      resultFile,
      JSON.stringify({ status: "updated", fingerprint: "a".repeat(64) }),
    );
    const bundle = () =>
      new Deno.Command(Deno.execPath(), {
        args: [
          "run",
          "-A",
          fileURLToPath(new URL("./publish.ts", import.meta.url)),
          "bundle",
          artifact,
        ],
        env: {
          GUKHANMUN_LOANWORDS_ROOT: f.root,
          GUKHANMUN_LOANWORDS_TARGET: "main",
          GUKHANMUN_LOANWORDS_PRIMARY: "main",
          GUKHANMUN_LOANWORDS_RESULT: resultFile,
          GITHUB_SHA: f.base,
        },
        stdout: "piped",
        stderr: "piped",
      }).output();
    // A leading space must not be stripped from a NUL-delimited untracked path.
    await Deno.writeTextFile(`${f.root}/ unintended-output`, "not attested\n");
    let outcome = await bundle();
    assert.equal(outcome.success, false);
    assert.match(new TextDecoder().decode(outcome.stderr), /non-update changes/);
    await Deno.remove(`${f.root}/ unintended-output`);
    outcome = await bundle();
    assert.ok(outcome.success, new TextDecoder().decode(outcome.stderr));
    const manifest = await validate(artifact);
    assert.equal(manifest.base, f.base);
    assert.equal(manifest.target, "main");
    assert.equal(manifest.tree, await f.git(["write-tree"]));
    assert.deepEqual(manifest.files.map((entry) => entry.path), [path]);
    for (
      const overrides of [
        { target: "../main" },
        { primary: "feature" },
        { tree: "invalid" },
        { controller: "" },
      ]
    ) {
      await Deno.writeTextFile(
        `${artifact}/manifest.json`,
        JSON.stringify({ ...manifest, ...overrides }),
      );
      await assert.rejects(() => validate(artifact), /invalid update manifest/);
    }
    await Deno.writeTextFile(`${artifact}/manifest.json`, JSON.stringify(manifest));
    await Deno.remove(`${artifact}/file-0`);
    await Deno.mkdir(`${artifact}/file-0`);
    await assert.rejects(() => validate(artifact), /non-regular/);
  } finally {
    await Deno.remove(f.parent, { recursive: true });
  }
});

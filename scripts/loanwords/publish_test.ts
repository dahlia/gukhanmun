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
  const variables = ["GITHUB_ACTIONS", "GITHUB_REPOSITORY", "GITHUB_REF"];
  const before = variables.map((v) => Deno.env.get(v));
  try {
    Deno.env.set("GITHUB_ACTIONS", "true");
    Deno.env.set("GITHUB_REPOSITORY", "dahlia/gukhanmun");
    Deno.env.set("GITHUB_REF", "refs/heads/main");
    const base = "a".repeat(40), old = "b".repeat(40);
    const snapshot = { records_sha256: "c".repeat(64) };
    const report = { decisions_sha256: "d".repeat(64), tsv_sha256: "e".repeat(64), audit: {} };
    const fingerprint = await generationFingerprint(snapshot, report);
    const manifest = { base, fingerprint, status: "updated", files: [] };
    const bot = "41898282+github-actions[bot]@users.noreply.github.com";
    let human = false, advanced = false, draft = false;
    const calls: string[] = [];
    const mock = (cmd: string, args: string[]): Promise<string> => {
      const c = `${cmd} ${args.join(" ")}`;
      calls.push(c);
      if (c === "git rev-parse origin/main") {
        return Promise.resolve(advanced ? "f".repeat(40) : base);
      }
      if (c.startsWith("git ls-remote")) {
        return Promise.resolve(`${old}\trefs/heads/automation/loanword-data`);
      }
      if (c.startsWith("git log")) {
        return Promise.resolve(human ? "human@example.test|human@example.test" : `${bot}|${bot}`);
      }
      if (c.includes("/pulls?head=")) {
        return Promise.resolve(JSON.stringify([{ number: 99, draft }]));
      }
      if (c.endsWith("/pending-update.json")) return Promise.resolve("");
      if (c.endsWith("/snapshot.json")) return Promise.resolve(JSON.stringify(snapshot));
      if (c.endsWith("/report.json")) return Promise.resolve(JSON.stringify(report));
      if (
        c.startsWith("git switch") || c.startsWith("git push") ||
        (!draft && c.includes("gh pr edit"))
      ) {
        throw new Error("identical data must not refresh the branch");
      }
      return Promise.resolve("");
    };
    await Deno.writeTextFile(`${directory}/manifest.json`, JSON.stringify(manifest));
    await publish(directory, mock, directory);
    assert.ok(calls.some((c) => c.includes("/statuses/")));
    assert.ok(!calls.some((c) => c.startsWith("git switch")));
    calls.length = 0;
    draft = true;
    await publish(directory, mock, directory);
    assert.ok(calls.some((c) => c.startsWith("gh pr edit 99")));
    assert.ok(calls.some((c) => c.startsWith("gh pr ready 99")));
    assert.ok(!calls.some((c) => c.startsWith("git switch") || c.startsWith("git push")));
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

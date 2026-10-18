// Gukhanmun: Tests pinned loanword update transactions.
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

import { canonical, hash, redactDiagnostics, update } from "./update.ts";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
const project = fileURLToPath(new URL("../../", import.meta.url));
const data = "crates/gukhanmun-loanwords/data";
const sections = [
  "crates/gukhanmun-loanwords",
  "crates/gukhanmun",
  "crates/gukhanmun-cli",
  "packages/loanwords-fst",
  "packages/loanwords-cdb",
];
const encoder = new TextEncoder();
const introduction = "Preserve the synthetic introductory loanword note.";
const credentialNames = ["NIKL_LOANWORD_API_KEY", "KOREAN_GO_KR_SERVICE_KEY"];

Deno.test("diagnostics preserve useful errors and redact both service-key encodings", () => {
  const before = credentialNames.map((name) => Deno.env.get(name));
  try {
    const first = String.raw`synthetic+key/with=padding?(x)[y]$\.`;
    const second = "other+key/=";
    Deno.env.set(credentialNames[0], first);
    Deno.env.set(credentialNames[1], encodeURIComponent(second));
    const forms = [
      first,
      second,
      encodeURIComponent(first),
      encodeURIComponent(second),
      encodeURIComponent(encodeURIComponent(second)),
      new URLSearchParams({ serviceKey: first }).toString().slice("serviceKey=".length),
      encodeURIComponent(second).replace("%2F", "%2f"),
    ];
    const message = redactDiagnostics(`HTTP 403; snapshot schema error\n${forms.join("\n")}`);
    assert.ok(message.includes("HTTP 403; snapshot schema error"));
    assert.ok(message.includes("[redacted]"));
    assert.ok(forms.every((value) => !message.includes(value)));
    Deno.env.set(credentialNames[0], "a'b c");
    assert.equal(redactDiagnostics("request: a%27b%20c"), "request: [redacted]");
    Deno.env.set(credentialNames[0], "a'b~c");
    assert.equal(redactDiagnostics("request: a%27b~c"), "request: [redacted]");
    Deno.env.set(credentialNames[1], "malformed%escape");
    assert.equal(redactDiagnostics("malformed%escape"), "[redacted]");
  } finally {
    credentialNames.forEach((name, i) => {
      if (before[i] === undefined) Deno.env.delete(name);
      else Deno.env.set(name, before[i]!);
    });
  }
});

Deno.test("failed child diagnostics reach the updater after redaction", async () => {
  const root = await fixture();
  const before = credentialNames.map((name) => Deno.env.get(name));
  const original = console.error;
  const messages: string[] = [];
  try {
    Deno.env.set(credentialNames[0], "invalid");
    Deno.env.delete(credentialNames[1]);
    console.error = (...values: unknown[]) => messages.push(values.join(" "));
    const incoming = `${root}/bad-source.json`;
    await Deno.writeTextFile(incoming, "{}");
    await assert.rejects(() => update(root, "update", incoming));
    const message = messages.join("\n");
    assert.ok(message.includes("operation failed"));
    assert.ok(message.includes("snapshot schema"));
    assert.ok(message.includes("[redacted]"));
    assert.ok(!message.includes("invalid"));
  } finally {
    console.error = original;
    credentialNames.forEach((name, i) => {
      if (before[i] === undefined) Deno.env.delete(name);
      else Deno.env.set(name, before[i]!);
    });
    await Deno.remove(root, { recursive: true });
  }
});
async function fixture(): Promise<string> {
  const root = await Deno.makeTempDir({ prefix: "loanword-test-" });
  await Deno.mkdir(`${root}/${data}`, { recursive: true });
  await Deno.writeTextFile(`${root}/${data}/decisions.json`, '{"format_version":1,"decisions":[]}');
  await Deno.copyFile(`${project}/sacho.toml`, `${root}/sacho.toml`);
  await Deno.mkdir(`${root}/changes.d`, { recursive: true });
  await Deno.writeTextFile(`${root}/changes.d/next`, "99.0.0\n");
  await Deno.writeTextFile(
    `${root}/CHANGES.md`,
    "Gukhanmun changelog\n===================\n\n<!-- sacho:unreleased:begin -->\n\n<!-- sacho:unreleased:end -->\n",
  );
  // Releases remove real fragments, so this fixture owns its introductory notes.
  for (const section of sections) {
    const directory = `${root}/changes.d/${section}`;
    await Deno.mkdir(directory, { recursive: true });
    await Deno.writeTextFile(
      `${directory}/loanword-data-update.md`,
      `---\nlinks:\n  '#18': https://github.com/dahlia/gukhanmun/issues/18\n---\n\n - ${introduction} [[#18]]\n`,
    );
  }
  for (const section of sections) {
    await Deno.mkdir(`${root}/${section}`, { recursive: true });
  }
  return root;
}
async function snapshot(root: string, readings: string[]): Promise<string> {
  const records = readings.map((reading, i) => ({
    id: `synthetic-${i}`,
    fields: {
      example_no: `synthetic-${i}`,
      srclang_mark: i < 2 ? "香港" : "日本",
      korean_mark: reading,
      regltn_code: "0003",
    },
  }));
  const path = `${root}/incoming.json`;
  await Deno.writeTextFile(
    path,
    JSON.stringify({
      format_version: 1,
      endpoint: "https://korean.go.kr/kornorms/exampleReqList.do",
      selector: "0003",
      collected_at: "synthetic",
      collector_version: "0.3.0",
      total_count: records.length,
      records_sha256: await hash(encoder.encode(canonical(records.map((r) => r.fields)))),
      records,
    }),
  );
  return path;
}
Deno.test("two data updates reuse stable fragments; a no-op and release recreation are idempotent", async () => {
  const root = await fixture();
  try {
    const fragment = `${root}/changes.d/crates/gukhanmun/loanword-data-update.md`;
    let source = await snapshot(root, ["홍콩"]);
    assert.equal((await update(root, "update", source)).status, "updated");
    await update(root, "check");
    source = await snapshot(root, ["홍콩", "홍콩", "일본"]);
    const changed = await update(root, "update", source);
    const fragmentAfter = await Deno.readTextFile(fragment);
    assert.ok(fragmentAfter.includes(introduction));
    assert.equal(
      (fragmentAfter.match(/Updated the NIKL loanword examples snapshot/g) ?? []).length,
      1,
    );

    assert.equal(changed.status, "updated");
    const before = await Deno.readTextFile(`${root}/CHANGES.md`);
    const again = await update(root, "update", source);
    assert.equal(again.status, "unchanged");
    assert.equal(changed.fingerprint, again.fingerprint);
    assert.equal(await Deno.readTextFile(`${root}/CHANGES.md`), before);
    for (const section of sections) {
      const entries = [];
      for await (const entry of Deno.readDir(`${root}/changes.d/${section}`)) {
        if (entry.name === "loanword-data-update.md") entries.push(entry.name);
      }
      assert.equal(entries.length, 1);
      await Deno.remove(`${root}/changes.d/${section}/loanword-data-update.md`);
    }
    source = await snapshot(root, ["홍콩", "홍콩", "니혼"]);
    assert.equal((await update(root, "update", source)).status, "updated");
    for (const section of sections) {
      await Deno.stat(`${root}/changes.d/${section}/loanword-data-update.md`);
    }
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});
Deno.test("new reading conflicts preserve the successful corpus and changelog", async () => {
  const root = await fixture();
  try {
    await update(root, "update", await snapshot(root, ["홍콩"]));
    const saved = await Promise.all(
      ["snapshot.json", "loanwords.tsv", "report.json"].map((f) =>
        Deno.readTextFile(`${root}/${data}/${f}`)
      ),
    );
    const changes = await Deno.readTextFile(`${root}/CHANGES.md`);
    const result = await update(root, "update", await snapshot(root, ["홍콩", "샹강"]));
    assert.equal(result.status, "pending");
    assert.deepEqual(
      await Promise.all(
        ["snapshot.json", "loanwords.tsv", "report.json"].map((f) =>
          Deno.readTextFile(`${root}/${data}/${f}`)
        ),
      ),
      saved,
    );
    assert.equal(await Deno.readTextFile(`${root}/CHANGES.md`), changes);
    const pending = JSON.parse(await Deno.readTextFile(`${root}/${data}/pending-update.json`));
    assert.equal(pending.affected_count, 1);
    assert.equal(pending.candidates["香港"].length, 2);
    assert.equal(pending.source_records.length, 2);
    assert.equal(pending.source_records_truncated, false);
    assert.equal(
      (await update(root, "update", await snapshot(root, ["홍콩", "샹강"]))).fingerprint,
      result.fingerprint,
    );
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});
Deno.test("formatter failure rolls back owned data and leaves unrelated files intact", async () => {
  const root = await fixture();
  const oldPath = Deno.env.get("PATH")!;
  const oldBinary = Deno.env.get("GUKHANMUN_LOANWORDS_BIN");
  try {
    await update(root, "update", await snapshot(root, ["홍콩"]));
    const before = await Deno.readTextFile(`${root}/${data}/snapshot.json`);
    await Deno.writeTextFile(`${root}/unrelated`, "preserve");
    const source = await snapshot(root, ["홍콩", "홍콩", "일본"]);
    Deno.env.set(
      "GUKHANMUN_LOANWORDS_BIN",
      `${project}/target/loanwords-tools/debug/gukhanmun-loanwords${
        Deno.build.os === "windows" ? ".exe" : ""
      }`,
    );
    Deno.env.set("PATH", `${root}/no-tools`);
    await assert.rejects(() => update(root, "update", source));
    assert.equal(await Deno.readTextFile(`${root}/${data}/snapshot.json`), before);
    assert.equal(await Deno.readTextFile(`${root}/unrelated`), "preserve");
  } finally {
    Deno.env.set("PATH", oldPath);
    if (oldBinary) Deno.env.set("GUKHANMUN_LOANWORDS_BIN", oldBinary);
    else Deno.env.delete("GUKHANMUN_LOANWORDS_BIN");
    await Deno.remove(root, { recursive: true });
  }
});

Deno.test("source numbers survive regeneration, pending evidence and strict validation", async () => {
  const root = await fixture();
  try {
    const field = (id: string, reading: string) =>
      `{"example_no":${id},"korean_mark":"${reading}","regltn_code":"0003","srclang_mark":"香港","unknown_integer":18446744073709551615,"unknown_real":1.0}`;
    const document = async (records: [string, string][], time = "synthetic") => {
      const fields = records.map(([id, reading]) => field(id, reading));
      return `{"format_version":1,"endpoint":"https://korean.go.kr/kornorms/exampleReqList.do","selector":"0003","collected_at":"${time}","collector_version":"0.3.0","total_count":${records.length},"records_sha256":"${await hash(
        encoder.encode(`[${fields.join(",")}]`),
      )}","records":[${
        records.map(([id], i) => `{"id":"${id}","fields":${fields[i]}}`).join(",")
      }]}`;
    };
    const pinned = `${root}/${data}/snapshot.json`;
    await Deno.writeTextFile(pinned, await document([["9007199254740993", "홍콩"]]));
    assert.equal((await update(root, "regenerate")).status, "updated");
    assert.equal((await update(root, "check")).status, "unchanged");
    const original = await Deno.readTextFile(pinned);
    assert.ok(original.includes('"example_no":9007199254740993'));
    assert.ok(original.includes('"unknown_integer":18446744073709551615'));
    assert.ok(original.includes('"unknown_real":1.0'));
    const incoming = `${root}/incoming.json`;
    await Deno.writeTextFile(incoming, await document([["9007199254740993", "홍콩"]], "new time"));
    assert.equal((await update(root, "update", incoming)).status, "unchanged");
    // Invalid JSON must reach Rust before a JavaScript round-trip can hide duplicate keys.
    await Deno.writeTextFile(
      incoming,
      original.replace('"srclang_mark":"香港"', '"srclang_mark":"wrong","srclang_mark":"香港"'),
    );
    await assert.rejects(() => update(root, "update", incoming));
    assert.equal(await Deno.readTextFile(pinned), original);
    await Deno.writeTextFile(
      incoming,
      await document([["9007199254740993", "홍콩"], ["9007199254740994", "샹강"]]),
    );
    assert.equal((await update(root, "update", incoming)).status, "pending");
    const evidence = await Deno.readTextFile(`${root}/${data}/pending-update.json`);
    assert.match(evidence, /"example_no": 9007199254740993/);
    assert.match(evidence, /"unknown_integer": 18446744073709551615/);
    assert.match(evidence, /"unknown_real": 1.0/);
    assert.equal(await Deno.readTextFile(pinned), original);
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});

Deno.test("trusted controller regenerates a separate maintenance layout and rejects unknown source versions", async () => {
  const root = await fixture();
  const controller = await Deno.makeTempDir({ prefix: "loanword-controller-test-" });
  try {
    await Deno.mkdir(`${controller}/scripts/loanwords`, { recursive: true });
    await Deno.copyFile(
      `${project}/scripts/loanwords/update.ts`,
      `${controller}/scripts/loanwords/update.ts`,
    );
    await Deno.copyFile(`${project}/mise.toml`, `${root}/mise.toml`);
    await Deno.mkdir(`${root}/crates/gukhanmun-dict-extract`, { recursive: true });
    await Deno.writeTextFile(
      `${root}/crates/gukhanmun-dict-extract/Cargo.toml`,
      "# synthetic target layout\n",
    );
    const source = await snapshot(root, ["홍콩"]);
    const invoke = (mode = "update") =>
      new Deno.Command(Deno.execPath(), {
        args: ["run", "-A", `${controller}/scripts/loanwords/update.ts`, mode, source],
        env: {
          GUKHANMUN_LOANWORDS_ROOT: root,
          GUKHANMUN_LOANWORDS_BIN: `${project}/target/loanwords-tools/debug/gukhanmun-loanwords${
            Deno.build.os === "windows" ? ".exe" : ""
          }`,
          GUKHANMUN_LOANWORDS_RESULT: `${controller}/result.json`,
          GITHUB_OUTPUT: `${controller}/output`,
        },
        stdout: "piped",
        stderr: "piped",
      }).output();
    const outcome = await invoke();
    assert.ok(outcome.success, new TextDecoder().decode(outcome.stderr));
    assert.ok((await invoke("check")).success);
    assert.equal(
      JSON.parse(await Deno.readTextFile(`${controller}/result.json`)).status,
      "updated",
    );
    const changes = await Deno.readTextFile(`${root}/CHANGES.md`);
    assert.ok(changes.includes("99.0.0"));
    assert.ok(changes.includes("Updated the NIKL loanword examples snapshot"));
    const before = await Deno.readTextFile(`${root}/${data}/snapshot.json`);
    const incoming = JSON.parse(await Deno.readTextFile(source));
    await Deno.writeTextFile(source, JSON.stringify({ ...incoming, format_version: 99 }));
    assert.equal((await invoke()).success, false);
    assert.equal(await Deno.readTextFile(`${root}/${data}/snapshot.json`), before);
    assert.equal(await Deno.readTextFile(`${root}/CHANGES.md`), changes);
  } finally {
    await Deno.remove(root, { recursive: true });
    await Deno.remove(controller, { recursive: true });
  }
});

Deno.test("unmerged identical proposals retain validated collection metadata before regeneration", async () => {
  const root = await fixture();
  try {
    await update(root, "update", await snapshot(root, ["홍콩"]));
    const paths = [
      ...["snapshot.json", "loanwords.tsv", "report.json", "metadata.json"].map((file) =>
        `${data}/${file}`
      ),
      "CHANGES.md",
      ...sections.map((section) => `changes.d/${section}/loanword-data-update.md`),
    ];
    const capture = async () =>
      new Map(
        await Promise.all(
          paths.map(async (path) => [path, await Deno.readFile(`${root}/${path}`)] as const),
        ),
      );
    const base = await capture();
    const source = await snapshot(root, ["홍콩", "홍콩", "일본"]);
    const original = JSON.parse(await Deno.readTextFile(source));
    await Deno.writeTextFile(
      source,
      JSON.stringify({ ...original, collected_at: "unix:100", collector_version: "0.3.0" }),
    );
    const first = await update(root, "update", source);
    const proposed = await capture();
    const candidate = `${root}/prior-candidate.json`;
    await Deno.writeFile(candidate, proposed.get(`${data}/snapshot.json`)!);
    const restore = async () => {
      for (const [path, bytes] of base) await Deno.writeFile(`${root}/${path}`, bytes);
    };
    await restore();
    await Deno.writeTextFile(
      source,
      JSON.stringify({ ...original, collected_at: "unix:200", collector_version: "0.4.0" }),
    );
    const repeated = await update(root, "update", source, candidate);
    assert.equal(repeated.fingerprint, first.fingerprint);
    assert.equal(repeated.status, "updated"); // Still differs from the unmerged base.
    for (const [path, bytes] of await capture()) assert.deepEqual(bytes, proposed.get(path), path);
    await restore();
    const forged = JSON.parse(await Deno.readTextFile(candidate));
    forged.records[0].fields.korean_mark = "위조"; // Retains the real hash claim but changes content.
    await Deno.writeTextFile(candidate, JSON.stringify(forged));
    await update(root, "update", source, candidate);
    assert.equal(
      JSON.parse(await Deno.readTextFile(`${root}/${data}/snapshot.json`)).collected_at,
      "unix:200",
    );
  } finally {
    await Deno.remove(root, { recursive: true });
  }
});

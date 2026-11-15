// Gukhanmun: Tests npm launcher resolution, streams, arguments and termination.
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

"use strict";

const assert = require("node:assert/strict");
const { spawn, spawnSync } = require("node:child_process");
const { once } = require("node:events");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");
const { targetFor } = require("../../packages/cli/lib/launcher.cjs");

const ROOT = path.resolve(__dirname, "../..");
const LIB = path.join(ROOT, "packages/cli/lib/launcher.cjs");
const FILES = {
  bash: "gukhanmun.bash",
  zsh: "_gukhanmun",
  fish: "gukhanmun.fish",
  powershell: "_gukhanmun.ps1",
  nushell: "gukhanmun.nu",
};

function fixture(t) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "gukhanmun npm 한글 "));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const parent = path.join(dir, "installed", "cli");
  fs.cpSync(path.join(ROOT, "packages/cli"), parent, { recursive: true });
  const bin = path.join(parent, "bin/gukhanmun.cjs");
  const target = targetFor(process.platform, process.arch);
  const name = `@gukhanmun/cli-${target}`;
  const payload = path.join(parent, "node_modules", name);
  fs.mkdirSync(path.join(parent, "completions"), { recursive: true });
  for (const name of Object.values(FILES)) {
    fs.writeFileSync(path.join(parent, "completions", name), `# ${name}\n`);
  }
  const install = (
    version = JSON.parse(fs.readFileSync(path.join(parent, "package.json"), "utf8")).version,
  ) => {
    fs.mkdirSync(payload, { recursive: true });
    fs.writeFileSync(
      path.join(payload, "package.json"),
      JSON.stringify({ name, version }),
    );
    return path.join(
      payload,
      process.platform === "win32" ? "gukhanmun.exe" : "gukhanmun",
    );
  };
  const invoke = (args, options = {}) =>
    spawnSync(process.execPath, [bin, ...args], {
      encoding: "utf8",
      cwd: dir,
      ...options,
    });
  return { dir, parent, bin, payload, name, install, invoke };
}

test("maps exactly the six supported Node.js platform pairs", () => {
  for (
    const [platform, suffix] of [
      ["darwin", "apple-darwin"],
      ["win32", "pc-windows-msvc"],
      ["linux", "unknown-linux-musl"],
    ]
  ) {
    for (const [arch, prefix] of [["arm64", "aarch64"], ["x64", "x86_64"]]) {
      assert.equal(targetFor(platform, arch), `${prefix}-${suffix}`);
    }
  }
  for (
    const [platform, arch] of [["freebsd", "x64"], ["linux", "ia32"], [
      "darwin",
      "__proto__",
    ]]
  ) {
    assert.throws(() => targetFor(platform, arch), /Unsupported platform/);
  }
});

test("locates every completion without an optional executable", (t) => {
  const f = fixture(t);
  for (const [shell, file] of Object.entries(FILES)) {
    const result = f.invoke(["--print-completion-path", shell]);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(
      result.stdout.trim(),
      path.join(f.parent, "completions", file),
    );
    assert.equal(fs.readFileSync(result.stdout.trim(), "utf8"), `# ${file}\n`);
  }
  const result = f.invoke(["--print-completion-path", "__proto__"]);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Unknown completion shell/);
});

test("reports missing optional dependencies instead of fetching a binary", (t) => {
  const f = fixture(t);
  const result = f.invoke(["--version"]);
  assert.equal(result.status, 1);
  assert.equal(result.stdout, "");
  assert.ok(result.stderr.includes(f.name));
  assert.match(result.stderr, /--include=optional/);
});

test("rejects wrong-version, damaged and incomplete platform packages", (t) => {
  const f = fixture(t);
  f.install("0.3.0");
  assert.match(f.invoke([]).stderr, /does not match/);
  f.install();
  assert.match(f.invoke([]).stderr, /Executable missing/);
  fs.writeFileSync(path.join(f.payload, "package.json"), "{");
  assert.equal(f.invoke([]).status, 1);
});

test("fixture payloads follow a bumped parent version", (t) => {
  const f = fixture(t);
  const manifest = path.join(f.parent, "package.json");
  const metadata = JSON.parse(fs.readFileSync(manifest, "utf8"));
  for (const version of ["0.4.1", "0.5.0-dev.42+abcdef0"]) {
    fs.writeFileSync(manifest, JSON.stringify({ ...metadata, version }));
    f.install();
    assert.match(f.invoke(["--version"]).stderr, /Executable missing/);
  }
});

test("resolution is anchored to the installation, not a competing cwd package", (t) => {
  const f = fixture(t);
  const caller = path.join(f.dir, "caller");
  const impostor = path.join(caller, "node_modules", f.name);
  fs.mkdirSync(impostor, { recursive: true });
  fs.writeFileSync(path.join(impostor, "package.json"), '{"version":"0.4.0"}');
  assert.match(
    f.invoke(["--version"], { cwd: caller }).stderr,
    /Missing optional package/,
  );
});

test("only the exact two-argument locator is handled by the launcher", {
  skip: process.platform === "win32",
}, (t) => {
  const f = fixture(t);
  const binary = f.install();
  fs.writeFileSync(
    binary,
    "#!/usr/bin/env node\nconsole.log(JSON.stringify(process.argv.slice(2)));\n",
  );
  fs.chmodSync(binary, 0o755);
  for (
    const args of [
      ["--print-completion-path"],
      ["--print-completion-path", "bash", "--help"],
      ["--", "--print-completion-path"],
      ["--generate-completion", "fish"],
    ]
  ) {
    const result = f.invoke(args);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(JSON.parse(result.stdout), args);
  }
});

test(
  "preserves exact arguments, cwd, environment, streams and numeric status",
  {
    skip: process.platform === "win32",
  },
  (t) => {
    const f = fixture(t);
    const binary = f.install();
    fs.writeFileSync(
      binary,
      `#!/usr/bin/env node
const fs = require("node:fs");
console.log(JSON.stringify({
  args:process.argv.slice(2), input:fs.readFileSync(0,"utf8"),
  cwd:process.cwd(), env:process.env.GUKHANMUN_TEST
}));
process.stderr.write("native stderr");
process.exitCode=37;
`,
    );
    fs.chmodSync(binary, 0o755);
    const args = [
      "space name",
      "한글",
      "",
      "'quoted'",
      '"double"',
      "$HOME",
      "&",
      ";",
      "--",
      "-name",
    ];
    const result = f.invoke(args, {
      input: "漢字\nsecond line",
      env: { ...process.env, GUKHANMUN_TEST: "passed through" },
    });
    assert.equal(result.status, 37);
    assert.equal(result.stderr, "native stderr");
    assert.deepEqual(JSON.parse(result.stdout), {
      args,
      input: "漢字\nsecond line",
      cwd: f.dir,
      env: "passed through",
    });
  },
);

test("spawn errors return a diagnostic and nonzero status", () => {
  const result = spawnSync(process.execPath, [
    "-e",
    `require(${JSON.stringify(LIB)}).runBinary("missing-gukhanmun-executable-3c927", [])`,
  ], { encoding: "utf8" });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /cannot execute/);
});

test("runBinary preserves Windows-hostile arguments without a shell", (t) => {
  const f = fixture(t);
  const script = path.join(f.dir, "argument witness.cjs");
  fs.writeFileSync(
    script,
    "console.log(JSON.stringify(process.argv.slice(2)))",
  );
  const args = [
    "",
    "a b",
    "한글",
    "&whoami",
    "%PATH%",
    "!value!",
    'a"b',
    "c\\",
    "a;b",
  ];
  const result = spawnSync(process.execPath, [
    "-e",
    `require(${JSON.stringify(LIB)}).runBinary(process.execPath,${
      JSON.stringify([script, ...args])
    })`,
  ], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(JSON.parse(result.stdout), args);
});

for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  test(
    `forwards launcher-directed ${signal} and re-raises the actual child signal`,
    {
      skip: process.platform === "win32",
      timeout: 10000,
    },
    async (t) => {
      const f = fixture(t);
      const script = path.join(f.dir, "wait.cjs");
      fs.writeFileSync(
        script,
        'console.log("ready"); setInterval(()=>{},1000)',
      );
      const child = spawn(process.execPath, [
        "-e",
        `require(${JSON.stringify(LIB)}).runBinary(process.execPath,${JSON.stringify([script])})`,
      ], { stdio: ["ignore", "pipe", "pipe"] });
      t.after(() => {
        if (child.exitCode === null && child.signalCode === null) {
          child.kill("SIGKILL");
        }
      });
      const closed = once(child, "close");
      await once(child.stdout, "data");
      child.kill(signal);
      assert.deepEqual(await closed, [null, signal]);
    },
  );
}

test("a handled interruption preserves the child's explicit numeric exit", {
  skip: process.platform === "win32",
  timeout: 10000,
}, async (t) => {
  const f = fixture(t);
  const script = path.join(f.dir, "handled.cjs");
  fs.writeFileSync(
    script,
    'process.on("SIGTERM",()=>process.exit(42)); console.log("ready"); setInterval(()=>{},1000)',
  );
  const child = spawn(process.execPath, [
    "-e",
    `require(${JSON.stringify(LIB)}).runBinary(process.execPath,${JSON.stringify([script])})`,
  ], { stdio: ["ignore", "pipe", "pipe"] });
  t.after(() => {
    if (child.exitCode === null && child.signalCode === null) {
      child.kill("SIGKILL");
    }
  });
  const closed = once(child, "close");
  await once(child.stdout, "data");
  child.kill("SIGTERM");
  assert.deepEqual(await closed, [42, null]);
});

test("process-group interruption terminates both launcher and child", {
  skip: process.platform === "win32",
  timeout: 10000,
}, async (t) => {
  const f = fixture(t);
  const script = path.join(f.dir, "group.cjs");
  fs.writeFileSync(
    script,
    "console.log(process.pid); setInterval(()=>{},1000)",
  );
  const child = spawn(process.execPath, [
    "-e",
    `require(${JSON.stringify(LIB)}).runBinary(process.execPath,${JSON.stringify([script])})`,
  ], { detached: true, stdio: ["ignore", "pipe", "pipe"] });
  t.after(() => {
    if (child.exitCode === null && child.signalCode === null) {
      process.kill(-child.pid, "SIGKILL");
    }
  });
  const closed = once(child, "close");
  const [data] = await once(child.stdout, "data");
  const nativePid = Number(String(data).trim());
  process.kill(-child.pid, "SIGINT");
  assert.deepEqual(await closed, [null, "SIGINT"]);
  assert.throws(() => process.kill(nativePid, 0), { code: "ESRCH" });
});

test("a signal received during spawn is queued for the child", {
  skip: process.platform === "win32",
  timeout: 10000,
}, () => {
  const result = spawnSync(process.execPath, [
    "-e",
    `const cp=require("node:child_process");
const original=cp.spawn;
cp.spawn=(...args)=>{process.emit("SIGTERM"); return original(...args)};
require(${JSON.stringify(LIB)}).runBinary(process.execPath,["-e","setInterval(()=>{},1000)"]);`,
  ], { encoding: "utf8", timeout: 5000 });
  assert.equal(result.error, undefined);
  assert.equal(result.signal, "SIGTERM");
  assert.equal(result.status, null);
});

test("other child signals use the documented numeric fallback", {
  skip: process.platform === "win32",
}, () => {
  const result = spawnSync(process.execPath, [
    "-e",
    `require(${
      JSON.stringify(LIB)
    }).runBinary(process.execPath,["-e",'process.kill(process.pid,"SIGPIPE")'])`,
  ], { encoding: "utf8" });
  // Node.js ignores SIGPIPE by default, unlike the Rust CLI. Force SIGKILL
  // instead to exercise a non-forwarded native termination unambiguously.
  assert.equal(result.status, 0);
  const killed = spawnSync(process.execPath, [
    "-e",
    `require(${
      JSON.stringify(LIB)
    }).runBinary(process.execPath,["-e",'process.kill(process.pid,"SIGKILL")'])`,
  ], { encoding: "utf8" });
  assert.equal(killed.signal, null);
  assert.equal(killed.status, 128 + os.constants.signals.SIGKILL);
});

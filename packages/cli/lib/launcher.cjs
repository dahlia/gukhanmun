// Gukhanmun: Resolves and supervises the npm-distributed native CLI.
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

const { spawn } = require("node:child_process");
const { existsSync } = require("node:fs");
const { constants } = require("node:os");
const { dirname, join } = require("node:path");
const { version } = require("../package.json");

const TARGETS = {
  "darwin:arm64": "aarch64-apple-darwin",
  "darwin:x64": "x86_64-apple-darwin",
  "win32:arm64": "aarch64-pc-windows-msvc",
  "win32:x64": "x86_64-pc-windows-msvc",
  "linux:arm64": "aarch64-unknown-linux-musl",
  "linux:x64": "x86_64-unknown-linux-musl",
};
const COMPLETIONS = {
  bash: "gukhanmun.bash",
  zsh: "_gukhanmun",
  fish: "gukhanmun.fish",
  powershell: "_gukhanmun.ps1",
  nushell: "gukhanmun.nu",
};

/** Select an executable target from Node.js's platform and architecture. */
function targetFor(platform, arch) {
  const target = TARGETS[`${platform}:${arch}`];
  if (!target) {
    throw new Error(
      `Unsupported platform ${platform}/${arch}. ` +
        "Gukhanmun npm packages support macOS, Windows and Linux on x64/arm64.",
    );
  }
  return target;
}

/** Resolve only from the installed launcher, never from the caller's directory. */
function binaryFor(platform = process.platform, arch = process.arch, command = "gukhanmun") {
  if (!["gukhanmun", "gukhanmun-mkdict"].includes(command)) {
    throw new Error(`Unknown executable ${command}.`);
  }
  const target = targetFor(platform, arch);
  const name = `@gukhanmun/cli-${target}`;
  let manifestPath;
  try {
    manifestPath = require.resolve(`${name}/package.json`);
  } catch (error) {
    if (error.code !== "MODULE_NOT_FOUND") throw error;
    throw new Error(
      `Missing optional package ${name}@${version}. ` +
        "Do not omit optional dependencies; reinstall with " +
        `npm install -g @gukhanmun/cli@${version} --include=optional.`,
    );
  }
  const installed = require(manifestPath);
  if (installed.name !== name || installed.version !== version) {
    throw new Error(
      `Platform package ${name} does not match @gukhanmun/cli@${version}; ` +
        "reinstall the CLI and its optional dependencies together.",
    );
  }
  const binary = join(
    dirname(manifestPath),
    platform === "win32" ? `${command}.exe` : command,
  );
  if (!existsSync(binary)) {
    throw new Error(
      `Executable missing from ${name}; reinstall @gukhanmun/cli@${version}.`,
    );
  }
  return binary;
}

/** Locate packaged completion data even when optional executables are omitted. */
function completionPath(shell) {
  if (!Object.hasOwn(COMPLETIONS, shell)) {
    throw new Error(
      `Unknown completion shell ${shell}; choose ${Object.keys(COMPLETIONS).join(", ")}.`,
    );
  }
  const path = join(__dirname, "..", "completions", COMPLETIONS[shell]);
  if (!existsSync(path)) {
    throw new Error(`Packaged completion file is missing: ${path}`);
  }
  return path;
}

/** Inherit the native CLI's streams and reflect its actual exit outcome. */
function runBinary(binary, args, command = "gukhanmun") {
  const handlers = new Map();
  const pending = [];
  let child;
  let settled = false;
  const cleanup = () => {
    for (const [signal, handler] of handlers) {
      process.removeListener(signal, handler);
    }
  };
  const forward = (signal) => {
    if (settled) return;
    if (!child) {
      pending.push(signal);
      return;
    }
    if (child.exitCode !== null || child.signalCode !== null) return;
    try {
      child.kill(signal);
    } catch (error) {
      if (error.code !== "ESRCH") {
        process.stderr.write(
          `${command}: cannot forward ${signal}: ${error.message}\n`,
        );
      }
    }
  };
  const signals = process.platform === "win32"
    ? ["SIGINT", "SIGBREAK"]
    : ["SIGINT", "SIGTERM", "SIGHUP"];
  for (const signal of signals) {
    // Windows console events already reach the native child. Killing it here
    // would force TerminateProcess instead of preserving console semantics.
    const handler = process.platform === "win32" ? () => {} : () => forward(signal);
    handlers.set(signal, handler);
    process.on(signal, handler);
  }
  try {
    child = spawn(binary, args, { stdio: "inherit", shell: false });
  } catch (error) {
    cleanup();
    throw error;
  }
  child.once("error", (error) => {
    if (settled) return;
    settled = true;
    cleanup();
    process.stderr.write(
      `${command}: cannot execute ${binary}: ${error.message}\n`,
    );
    process.exitCode = 1;
  });
  child.once("close", (code, signal) => {
    if (settled) return;
    settled = true;
    cleanup();
    if (code !== null) {
      process.exitCode = code;
      return;
    }
    process.exitCode = 128 + (constants.signals[signal] || 1);
    if (
      process.platform !== "win32" &&
      ["SIGINT", "SIGTERM", "SIGHUP"].includes(signal)
    ) {
      try {
        process.kill(process.pid, signal);
      } catch {
        // Keep the documented numeric fallback when re-raising is unavailable.
      }
    }
  });
  for (const signal of pending) forward(signal);
  return child;
}

/** Handle the one launcher-specific invocation; pass every other argument on. */
function main(args = process.argv.slice(2), command = "gukhanmun") {
  try {
    if (command === "gukhanmun" && args.length === 2 && args[0] === "--print-completion-path") {
      process.stdout.write(`${completionPath(args[1])}\n`);
    } else {
      runBinary(binaryFor(process.platform, process.arch, command), args, command);
    }
  } catch (error) {
    process.stderr.write(`${command}: ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { targetFor, binaryFor, completionPath, runBinary, main };

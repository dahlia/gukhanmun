// Gukhanmun: Tests for @gukhanmun/wasm (run with node --test, deno test, or bun test).
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
import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { GukhanmunError, load } from "../../packages/wasm/index.ts";

// ── Helpers ──────────────────────────────────────────────────────────────────

async function readFixture(relPath: string): Promise<string> {
  const url = new URL(`../fixtures/${relPath}`, import.meta.url);
  const specifier: string = "node:fs/promises";
  type Fs = { readFile(path: URL, encoding: string): Promise<string> };
  const { readFile } = (await import(specifier)) as unknown as Fs;
  return readFile(url, "utf8");
}

async function streamAll(
  stream: TransformStream<string, string>,
  chunks: readonly string[],
): Promise<string> {
  const parts: string[] = [];
  // Read concurrently with writing to avoid backpressure deadlock: writer.close()
  // may not resolve until the reader has drained the readable side.
  const readDone = (async () => {
    const reader = stream.readable.getReader();
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value) parts.push(value);
    }
  })();
  const writer = stream.writable.getWriter();
  for (const chunk of chunks) await writer.write(chunk);
  await writer.close();
  await readDone;
  return parts.join("");
}

// ── Fixture parity ───────────────────────────────────────────────────────────

// These fixtures use use_bundled_stdict = false — Unihan fallback only.
// The JS load() also carries no bundled dictionary, so they match directly.

describe("fixture parity", () => {
  test("html/initial-sound-raw", async () => {
    const input = await readFixture("html/initial-sound-raw.input.html");
    const expected = await readFixture("html/initial-sound-raw.expected.html");
    const g = await load({ preset: "ko-kr" });
    assert.equal(g.convert(input, "html"), expected);
  });

  test("html/i-reon-nal", async () => {
    const input = await readFixture("html/i-reon-nal.input.html");
    const expected = await readFixture("html/i-reon-nal.expected.html");
    // disambiguation = "off" in the fixture TOML → homophoneWindow: "off"
    const g = await load({ preset: "ko-kr", homophoneWindow: "off" });
    assert.equal(g.convert(input, "html"), expected);
  });

  test("html/preservation", async () => {
    const input = await readFixture("html/preservation.input.html");
    const expected = await readFixture("html/preservation.expected.html");
    // disambiguation = "off" in the fixture TOML → homophoneWindow: "off"
    const g = await load({ preset: "ko-kr", homophoneWindow: "off" });
    assert.equal(g.convert(input, "html"), expected);
  });
});

// ── Streaming equivalence ────────────────────────────────────────────────────

// The stream guarantee: concatenated output equals convert() on the full input,
// regardless of how the input is split into chunks.

describe("streaming equivalence", () => {
  test("single-character chunks produce same output as convert()", async () => {
    const input = await readFixture("html/initial-sound-raw.input.html");
    const g = await load({ preset: "ko-kr" });
    const expected = g.convert(input, "html");
    const actual = await streamAll(g.stream("html"), [...input]);
    assert.equal(actual, expected);
  });

  test("two equal halves produce same output as convert()", async () => {
    const input = await readFixture("html/i-reon-nal.input.html");
    const g = await load({ preset: "ko-kr", homophoneWindow: "off" });
    const expected = g.convert(input, "html");
    const mid = Math.floor(input.length / 2);
    const actual = await streamAll(g.stream("html"), [
      input.slice(0, mid),
      input.slice(mid),
    ]);
    assert.equal(actual, expected);
  });
});

// ── Error handling ───────────────────────────────────────────────────────────

describe("error handling", () => {
  test("GukhanmunError is an Error subclass", async () => {
    await assert.rejects(
      () => load({ preset: "invalid-preset" as never }),
      GukhanmunError,
    );
  });
});

// ── Options round-trip ───────────────────────────────────────────────────────

describe("options", () => {
  test("resolved options are exposed on the instance", async () => {
    const g = await load({ preset: "ko-kr", homophoneWindow: "off" });
    assert.equal(g.options.preset, "ko-kr");
    assert.equal(g.options.homophoneWindow, "off");
    assert.equal(typeof g.options.rendering, "string");
  });
});

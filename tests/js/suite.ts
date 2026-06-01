// Gukhanmun: Shared, binding-agnostic integration suite run against every JS binding.
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

// The suite is written once here and registered against each binding
// (`@gukhanmun/wasm`, `@gukhanmun/napi`) by a thin `*.test.ts` entry point that
// injects that binding's `load` / `GukhanmunError`.  It uses only `node:test`
// and `node:assert/strict`, which Node.js, Deno, and Bun all implement, so the
// same cases run unchanged on all three runtimes.

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { stdictFst } from "../../packages/stdict-fst/index.ts";
import { stdictCdb } from "../../packages/stdict-cdb/index.ts";
import type {
  FileDictionarySource,
  Gukhanmun,
  GukhanmunOptions,
} from "../../packages/types/index.ts";

// ── Injected binding ─────────────────────────────────────────────────────────

/** The two values each binding package exports that the suite exercises. */
export interface Binding {
  /** The package's `load` factory. */
  load: (options?: GukhanmunOptions) => Promise<Gukhanmun>;
  /**
   * The package's `GukhanmunError` class.  Typed as a loose error constructor
   * so both bindings' concrete classes (whose constructors take
   * `(code, message, chain?)`) are assignable here; the suite only uses it for
   * `instanceof` checks, never to construct.
   */
  // deno-lint-ignore no-explicit-any
  GukhanmunError: new (...args: any[]) => Error & { readonly code: string };
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async function readFixture(relPath: string): Promise<string> {
  const url = new URL(`../fixtures/${relPath}`, import.meta.url);
  // Non-literal specifier keeps `deno check` from statically resolving
  // node:fs/promises types (which would require @types/node).
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

/**
 * The bundled Standard Korean Language Dictionary in each available backend
 * format.  `packages/stdict-{fst,cdb}/` are built from the same
 * `gukhanmun-stdict` data, so both must yield identical conversions.
 */
const dictionaryBackends: readonly (readonly [
  string,
  () => Promise<FileDictionarySource>,
])[] = [
  ["fst", stdictFst],
  ["cdb", stdictCdb],
];

// ── Suite ────────────────────────────────────────────────────────────────────

/**
 * Registers the full integration suite against one binding.
 *
 * Call once per binding from a `node --test` / `deno test` / `bun test` entry
 * file.  `name` only labels the test output (e.g. `"wasm"`, `"napi"`).
 */
export function registerSuite(name: string, binding: Binding): void {
  const { load, GukhanmunError } = binding;

  // ── Fixture parity without a dictionary ──────────────────────────────────
  // These fixtures use use_bundled_stdict = false (Unihan fallback only), so
  // the JS load() with no dictionary matches the Rust fixture directly.
  describe(`${name}: fixture parity (no dictionary)`, () => {
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
      const g = await load({ preset: "ko-kr", homophoneWindow: "off" });
      assert.equal(g.convert(input, "html"), expected);
    });
  });

  // ── Fixture parity with the bundled dictionary ───────────────────────────
  // packages/stdict-fst/stdict.fst is a copy of the gukhanmun-stdict crate's
  // FST output, so loading it reproduces the Rust use_bundled_stdict path.
  describe(`${name}: bundled-dictionary parity`, () => {
    test("text/constitution-preamble matches the Rust fixture", async () => {
      const input = await readFixture("text/constitution-preamble.input.txt");
      const expected = await readFixture("text/constitution-preamble.expected.txt");
      const g = await load({ preset: "ko-kr", dictionaries: [await stdictFst()] });
      assert.equal(g.convert(input, "text"), expected);
    });
  });

  // ── Dictionary-backed conversion (the headline use case) ─────────────────
  // Both backends carry the same dictionary and must convert identically.
  describe(`${name}: dictionary-backed conversion`, () => {
    for (const [backend, loadDict] of dictionaryBackends) {
      test(`${backend}: 漢字를 한글로 → 한자를 한글로`, async () => {
        const g = await load({ dictionaries: [await loadDict()] });
        assert.equal(g.convert("漢字를 한글로"), "한자를 한글로");
      });
    }
  });

  // ── Render modes ─────────────────────────────────────────────────────────
  describe(`${name}: render modes`, () => {
    test("hangul-hanja-parens glosses with the source hanja", async () => {
      const g = await load({
        dictionaries: [await stdictFst()],
        rendering: "hangul-hanja-parens",
      });
      assert.equal(g.convert("漢字"), "한자(漢字)");
    });
  });

  // ── Format coverage ──────────────────────────────────────────────────────
  describe(`${name}: format coverage`, () => {
    test("text is the default format", async () => {
      const g = await load({ dictionaries: [await stdictFst()] });
      assert.equal(g.convert("漢字"), g.convert("漢字", "text"));
    });

    test("markdown converts prose but leaves code fences untouched", async () => {
      const g = await load({ dictionaries: [await stdictFst()] });
      const out = g.convert("學校\n\n```\n學校\n```\n", "markdown");
      // The leading prose 學校 converts to 학교; the fenced 學校 is preserved
      // verbatim (the adapter emits fenced content as Verbatim).
      assert.match(out, /^학교/);
      assert.ok(out.includes("學校"), "fenced hanja must be preserved");
    });
  });

  // ── Streaming equivalence ────────────────────────────────────────────────
  // Concatenated stream output equals convert() on the full input, regardless
  // of how the input is split into chunks.
  describe(`${name}: streaming equivalence`, () => {
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

  // ── Resolved options ─────────────────────────────────────────────────────
  describe(`${name}: resolved options`, () => {
    test("ko-kr defaults are exposed on the instance", async () => {
      const g = await load({ preset: "ko-kr", homophoneWindow: "off" });
      assert.equal(g.options.preset, "ko-kr");
      assert.equal(g.options.homophoneWindow, "off");
      assert.equal(g.options.initialSoundLaw, true);
      assert.equal(g.options.collapseRedundantParens, true);
      assert.equal(typeof g.options.rendering, "string");
    });

    test("ko-kp disables initial sound law and homophone window", async () => {
      const g = await load({ preset: "ko-kp" });
      assert.equal(g.options.preset, "ko-kp");
      assert.equal(g.options.initialSoundLaw, false);
      assert.equal(g.options.homophoneWindow, "off");
    });
  });

  // ── Redundant parenthetical collapsing ───────────────────────────────────
  describe(`${name}: redundant parenthetical collapsing`, () => {
    test("collapses an explicit reading gloss by default", async () => {
      // 蔣介石 converts to 장개석 via the per-character fallback, so the matching
      // parenthetical collapses without needing a dictionary.
      const g = await load({ homophoneWindow: "off" });
      assert.equal(g.convert("蔣介石(장개석)"), "장개석(蔣介石)");
    });

    test("collapseRedundantParens: false keeps the parenthetical", async () => {
      const g = await load({
        homophoneWindow: "off",
        collapseRedundantParens: false,
      });
      assert.equal(g.options.collapseRedundantParens, false);
      assert.equal(g.convert("蔣介石(장개석)"), "장개석(장개석)");
    });
  });

  // ── Error handling ───────────────────────────────────────────────────────
  describe(`${name}: error handling`, () => {
    test("an unknown preset rejects with a coded GukhanmunError", async () => {
      await assert.rejects(
        () => load({ preset: "invalid-preset" as never }),
        (err: unknown) => {
          assert.ok(err instanceof GukhanmunError);
          assert.equal(typeof err.code, "string");
          assert.ok(err.code.length > 0, "error code must be non-empty");
          return true;
        },
      );
    });
  });
}

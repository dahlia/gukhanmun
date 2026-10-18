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
import { opendictGeneralFst, opendictNorthKoreanFst } from "../../packages/opendict-fst/index.ts";
import { opendictGeneralCdb, opendictNorthKoreanCdb } from "../../packages/opendict-cdb/index.ts";
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

  describe(`${name}: punctuation profiles`, () => {
    test("universal inputs, HTML references, and streamed chunks", async () => {
      for (const punctuation of ["modern", "traditional", "traditional-vertical"] as const) {
        const g = await load({ punctuation });
        const western = "café, world IMF, école cafe\u0301, world";
        assert.equal(g.convert(western), western);
        const source = "가&#12290;  나、「다」";
        const expected = g.convert(source, "html");
        assert.equal(await streamAll(g.stream("html"), Array.from(source)), expected);
        assert.equal(g.convert(g.convert("가。 나、〈다〉")), g.convert("가。 나、〈다〉"));
      }
      const g = await load({ punctuation: "modern" });
      assert.equal(g.convert("가&#12290;나", "html"), "가. 나");
      assert.equal(g.convert("가。\uFE0F나"), "가.\uFE0F나");
      assert.equal(g.convert("가。&#xFE0F;나", "html"), "가.&#xFE0F;나");
      assert.equal(g.convert("가&#12290;나"), "가&#12290;나");
      assert.equal(
        g.convert("1,000 3.14 Hello, world example.com"),
        "1,000 3.14 Hello, world example.com",
      );
    });
    test("normalized Markdown keeps emphasis, breaks and inline HTML boundaries", async () => {
      const g = await load({ punctuation: "modern" });
      assert.ok(g.convert("*끝。* 다음", "markdown").includes("<em>끝&#46;</em> 다음"));
      assert.ok(g.convert("**\\\n끝** 다음", "markdown").includes("<strong><br />"));
      assert.ok(g.convert("앞 <em>가。</em>나", "markdown").includes("</em> 나"));
      assert.ok(
        g.convert("~~끝。~~ 다음", { format: "markdown", gfm: true }).includes(
          "<del>끝&#46;</del> 다음",
        ),
      );
    });
    test("HTML recovery and effective nested language policies", async () => {
      const g = await load({ punctuation: "modern" });
      assert.equal(
        g.convert("<div lang=en><span lang=ko>韓國。文化</span></div>", "html"),
        "<div lang=en><span lang=ko>한국. 문화</span></div>",
      );
      assert.equal(
        g.convert("<span lang=en>keep<span lang=ko>가。  </span> tail。</span>", "html"),
        "<span lang=en>keep<span lang=ko>가.  </span> tail。</span>",
      );
      for (const [mark, entity] of [["!", "&#33;"], ["?", "&#63;"], ["/", "&#47;"]] as const) {
        const custom = await load({
          punctuation: { period: { mark, after: "remove" } },
          recovery: "lenient",
        });
        assert.equal(custom.convert("<。--hello-->VISIBLE", "html"), `<${entity}--hello-->VISIBLE`);
      }
    });
    test("Markdown serialization respects preserved inline HTML text", async () => {
      const modern = await load({ punctuation: "modern" }), preserve = await load();
      for (
        const source of [
          "앞 <script>alert(1);</script> 뒤",
          "앞 <script>let x=2**3**2;</script> 뒤",
          "앞 <style>.x { color: red; }</style> 뒤",
          "앞 <code>a&b</code> 뒤",
        ]
      ) {
        assert.equal(modern.convert(source, "markdown"), preserve.convert(source, "markdown"));
      }
    });
    test("decoded Markdown entity line endings remain text", async () => {
      const g = await load({ punctuation: "modern" });
      assert.ok(
        g.convert("**&#10;[글](dest)** 다음", "markdown").includes(
          "<strong>&#10;[글](dest)</strong> 다음",
        ),
      );
    });
    test("inheritance, null, undefined and detached frozen resolved styles", async () => {
      const custom = { base: "modern" as const, comma: undefined, period: null };
      const g = await load({ punctuation: custom });
      assert.equal(g.convert("가、나。 다"), "가, 나。 다");
      assert.equal(g.options.punctuation.period, null);
      assert.deepEqual(g.options.punctuation.comma, { mark: ",", after: "single" });
      assert.ok(Object.isFrozen(g.options.punctuation));
      assert.ok(Object.isFrozen(g.options.punctuation.comma));
      assert.deepEqual((await load()).options.punctuation, {
        comma: null,
        period: null,
        single: null,
        double: null,
      });
    });
    test("custom Unicode scalar marks and invalid options", async () => {
      const g = await load({ punctuation: { period: { mark: "💬", after: "remove" } } });
      assert.equal(g.convert("가。 나"), "가💬나");
      for (
        const punctuation of [
          "bogus",
          null,
          { typo: true },
          { period: { mark: "xx", after: "single" } },
          { period: { mark: "\ud800", after: "single" } },
          { period: { mark: "&", after: "single" } },
          { single: { open: "〉", close: "〈" } },
          { period: { mark: ";", after: "bogus" } },
        ]
      ) {
        await assert.rejects(
          () => load({ punctuation } as unknown as GukhanmunOptions),
          (error: unknown) => error instanceof GukhanmunError && error.code === "invalid-input",
        );
      }
    });
  });

  // ── Fixture parity without a dictionary ──────────────────────────────────
  // These fixtures use use_bundled_stdict = false (Unihan fallback only), so
  // the JS load() with no dictionary matches the Rust fixture directly.
  describe(`${name}: fixture parity (no dictionary)`, () => {
    test("html/initial-sound-raw", async () => {
      const input = await readFixture("html/initial-sound-raw.input.html");
      const expected = await readFixture(
        "html/initial-sound-raw.expected.html",
      );
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
      const expected = await readFixture(
        "text/constitution-preamble.expected.txt",
      );
      const g = await load({
        preset: "ko-kr",
        dictionaries: [await stdictFst()],
      });
      assert.equal(g.convert(input, "text"), expected);
    });
  });

  // ── Dictionary-backed conversion (the headline use case) ─────────────────
  // Both backends carry the same dictionary and must convert identically.
  describe(`${name}: dictionary-backed conversion`, () => {
    for (const [backend, loadDict] of dictionaryBackends) {
      test(`${backend}: candidate defaults and positional readings agree`, async () => {
        const g = await load({ dictionaries: [await loadDict()], homophoneWindow: "off" });
        assert.equal(
          g.convert("北京 數字 1990年代 1998年度 비率"),
          "베이징 숫자 1990년대 1998년도 비율",
        );
      });
      test(`${backend}: particle evidence uses bundled candidates in conversion and streams`, async () => {
        for (const segmentation of ["eager", "lattice"] as const) {
          for (const rendering of ["hangul-only", "hangul-hanja-parens"] as const) {
            const g = await load({
              dictionaries: [await loadDict()],
              homophoneWindow: "off",
              segmentation,
              rendering,
            });
            if (rendering === "hangul-only") {
              assert.equal(g.convert("東京을 東京를 東京이다"), "동경을 도쿄를 동경이다");
              assert.equal(g.convert("東京으로는 간다"), "동경으로는 간다");
            }
            for (
              const input of [
                "東京을",
                "東京으로는 간다",
                "東京이다",
                "東京을아아아아아아아아아아아아",
              ]
            ) {
              const expected = g.convert(input);
              assert.equal(await streamAll(g.stream("text"), [...input]), expected);
              const chars = [...input];
              for (let split = 0; split <= chars.length; split++) {
                assert.equal(
                  await streamAll(g.stream("text"), [
                    chars.slice(0, split).join(""),
                    chars.slice(split).join(""),
                  ]),
                  expected,
                );
              }
            }
          }
        }
      });
      test(`${backend}: 漢字를 한글로 → 한자를 한글로`, async () => {
        const g = await load({ dictionaries: [await loadDict()] });
        assert.equal(g.convert("漢字를 한글로"), "한자를 한글로");
      });
    }
  });

  describe(`${name}: regenerated Open Korean Dictionary candidates`, () => {
    for (
      const [backend, generalLoader, northLoader] of [
        ["fst", opendictGeneralFst, opendictNorthKoreanFst],
        ["cdb", opendictGeneralCdb, opendictNorthKoreanCdb],
      ] as const
    ) {
      test(`${backend}: pinned competing reading survives package bytes`, async () => {
        const g = await load({ dictionaries: [await generalLoader()], homophoneWindow: "off" });
        assert.equal(g.convert("俳句"), "하이쿠");
      });
      test(`${backend}: category priority replaces the whole candidate set`, async () => {
        const general = await generalLoader();
        const north = await northLoader();
        const nkFirst = await load({
          dictionaries: [north, general],
          preset: "ko-kp",
          homophoneWindow: "off",
        });
        const generalFirst = await load({
          dictionaries: [general, north],
          preset: "ko-kp",
          homophoneWindow: "off",
        });
        assert.equal(nkFirst.convert("齘齒 歷史"), "계치 력사");
        assert.equal(generalFirst.convert("齘齒 歷史"), "해치 역사");
        const input = "俳句 齘齒 歷史 비率 1990年代";
        assert.equal(await streamAll(nkFirst.stream("text"), [...input]), nkFirst.convert(input));
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

    test("variant recognition and output sets are shared by every binding", async () => {
      const dictionary = await stdictFst();
      const canonical = await load({
        dictionaries: [dictionary],
        rendering: "original",
      });
      const shinjitai = await load({
        dictionaries: [dictionary],
        rendering: "original",
        hanjaVariantSet: "shinjitai",
      });
      assert.equal(canonical.convert("芸術"), "藝術");
      assert.equal(shinjitai.convert("芸術"), "芸術");
      assert.equal(shinjitai.options.hanjaVariantSet, "shinjitai");
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

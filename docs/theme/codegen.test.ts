// Gukhanmun: Checks dictionary selection in generated playground examples.
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
import { test } from "node:test";
import { type CodegenConfig, DICT_ORDER, generateExamples } from "./codegen.ts";

function config(
  enabled: string[],
  activePreset: CodegenConfig["activePreset"] = null,
): CodegenConfig {
  return {
    activePreset,
    format: "text",
    input: "香港",
    rendering: "hangul-only",
    originalGloss: "parens",
    hanjaVariantSet: "as-dictionary",
    segmentation: "lattice",
    numerals: "hangul-phonetic",
    initialSoundLaw: true,
    homophoneWindow: "per-block",
    homophoneDetection: "context-local",
    firstOccurrenceWindow: "off",
    recovery: "strict",
    dicts: Object.fromEntries(
      DICT_ORDER.map((id) => [id, enabled.includes(id)]),
    ) as CodegenConfig["dicts"],
    requireHanja: [],
    requireHangul: [],
    skipAnnotation: [],
    preserveClasses: [],
    preserveAttributes: [],
  };
}

test("ko-kr examples include loanwords by default and load them before stdict in JS", () => {
  const examples = generateExamples(config(["loanwords", "stdict"], "ko-kr"));
  assert.doesNotMatch(examples.cli, /--no-bundled-dictionaries/);
  assert.match(examples.rust, /Preset::KoKr/);
  assert.match(
    examples.js,
    /dictionaries: \[await loanwordsFst\(\), await stdictFst\(\)\]/,
  );
});

test("turning off loanwords disables the default bundle in CLI and Rust examples", () => {
  const examples = generateExamples(config(["stdict"]));
  assert.match(examples.cli, /--no-bundled-dictionaries/);
  assert.match(examples.cli, /--dictionary stdict.fst/);
  assert.match(examples.rust, /\.no_bundled_dictionaries\(\)/);
  assert.doesNotMatch(examples.js, /loanwordsFst/);
});

test("custom combinations keep loanwords ahead of every other selected dictionary", () => {
  const examples = generateExamples(
    config(["loanwords", "stdict", "opendict-north-korean"]),
  );
  assert.ok(
    examples.cli.indexOf("--dictionary stdict.fst") <
      examples.cli.indexOf("--dictionary loanwords.fst"),
  );
  assert.ok(
    examples.rust.indexOf("gukhanmun::loanwords::dictionary()") <
      examples.rust.indexOf("gukhanmun::opendict::north_korean()"),
  );
  assert.match(
    examples.js,
    /dictionaries: \[await loanwordsFst\(\), await opendictNorthKoreanFst\(\), await stdictFst\(\)\]/,
  );
});

test("ko-kp examples leave loanwords off", () => {
  const examples = generateExamples(config(["opendict-north-korean"], "ko-kp"));
  assert.match(examples.cli, /--preset ko-kp/);
  assert.match(examples.rust, /Preset::KoKp/);
  assert.doesNotMatch(examples.js, /loanwordsFst/);
});

test("Python without dictionaries needs only the core package", () => {
  const python = generateExamples(config([])).python;
  assert.equal(
    python,
    [
      "# python -m pip install gukhanmun",
      "",
      "from gukhanmun import Converter",
      "",
      "converter = Converter()",
      'output = converter.convert("香港")',
      "print(output)",
    ].join("\n"),
  );
});

test("Python lists every selected dictionary in priority order and deduplicates extras", () => {
  const python = generateExamples(config([...DICT_ORDER].reverse())).python;
  assert.match(python, /^# python -m pip install 'gukhanmun\[loanwords,opendict,stdict\]'/);
  assert.equal(python.match(/# python -m pip install/g)?.length, 1);
  assert.match(
    python,
    /dictionaries=\[loanwords\(\), opendict_north_korean\(\), opendict_general\(\), opendict_dialect\(\), opendict_archaic\(\), stdict\(\)\]/,
  );
  for (
    const loader of [
      "loanwords",
      "stdict",
      "opendict_general",
      "opendict_north_korean",
      "opendict_dialect",
      "opendict_archaic",
    ]
  ) {
    assert.match(python, new RegExp(`    ${loader},`));
  }
  assert.doesNotMatch(python, /\bawait\b|async |load\(/);
});

test("Python presets keep dictionary loading explicit", () => {
  const python = generateExamples(config(["loanwords", "stdict"], "ko-kr")).python;
  assert.match(python, /preset=Preset\.KO_KR/);
  assert.match(python, /from gukhanmun\.dictionaries import loanwords, stdict/);
  assert.match(python, /dictionaries=\[loanwords\(\), stdict\(\)\]/);
  assert.match(python, /gukhanmun\[loanwords,stdict\]/);
});

test("Python preserves ko-kp overrides relative to that preset's defaults", () => {
  const cfg = config([], "ko-kp");
  // The configuration helper uses ko-kr values; these must override ko-kp.
  let python = generateExamples(cfg).python;
  assert.match(python, /preset=Preset\.KO_KP/);
  assert.match(python, /initial_sound_law=True/);
  assert.match(python, /homophone_window=ContextWindow\.PER_BLOCK/);
  cfg.initialSoundLaw = false;
  cfg.homophoneWindow = "off";
  python = generateExamples(cfg).python;
  assert.doesNotMatch(python, /initial_sound_law=|homophone_window=|ContextWindow/);
});

test("Python translates every nondefault option to its actual enum and keyword", () => {
  const cfg: CodegenConfig = {
    ...config([]),
    format: "html",
    rendering: "original",
    originalGloss: "ruby",
    hanjaVariantSet: "simplified",
    segmentation: "eager",
    numerals: "smart",
    initialSoundLaw: false,
    homophoneWindow: "per-document",
    homophoneDetection: "dictionary-wide",
    firstOccurrenceWindow: "per-section",
    recovery: "lenient",
  };
  const python = generateExamples(cfg).python;
  for (
    const field of [
      "rendering=RenderMode.ORIGINAL",
      "original_gloss=OriginalGloss.RUBY",
      "hanja_variant_set=HanjaVariantSet.SIMPLIFIED",
      "segmentation=Segmentation.EAGER",
      "numerals=NumeralStrategy.SMART",
      "initial_sound_law=False",
      "homophone_window=ContextWindow.PER_DOCUMENT",
      "homophone_detection=HomophoneDetection.DICTIONARY_WIDE",
      "first_occurrence_window=ContextWindow.PER_SECTION",
      "recovery=Recovery.LENIENT",
    ]
  ) assert.ok(python.includes(field), field);
  assert.doesNotMatch(python, /converter\.options\.[a-z_]+\s*=/);
});

test("Python converts text, HTML and CommonMark with the proper format enum", () => {
  const cfg = config([]);
  cfg.input = "<p>漢字</p>";
  assert.match(generateExamples(cfg).python, /converter\.convert\("<p>漢字<\/p>"\)/);
  cfg.format = "html";
  assert.match(generateExamples(cfg).python, /format=InputFormat\.HTML/);
  cfg.format = "markdown";
  const python = generateExamples(cfg).python;
  assert.match(python, /format=InputFormat\.MARKDOWN/);
  assert.doesNotMatch(python, /gfm=True/);
});

test("Python constructs typed directives and HTML preservation records", () => {
  const cfg: CodegenConfig = {
    ...config([]),
    format: "html",
    recovery: "lenient",
    requireHanja: ["漢字"],
    requireHangul: ["漢字"],
    skipAnnotation: ["東京"],
    preserveClasses: ["keep", 'quoted"class'],
    preserveAttributes: ["data-keep", "translate"],
  };
  const python = generateExamples(cfg).python;
  assert.match(python, /directives=Directives\(/);
  assert.match(python, /require_hanja=\["漢字"\]/);
  assert.match(python, /require_hangul=\["漢字"\]/);
  assert.match(python, /skip_annotation=\["東京"\]/);
  assert.match(python, /html=HtmlOptions\(/);
  assert.ok(python.includes(String.raw`preserve_classes=["keep", "quoted\"class"]`));
  assert.match(
    python,
    /preserve_attributes=\[AttributeMatcher\("data-keep"\), AttributeMatcher\("translate"\)\]/,
  );
  cfg.format = "markdown";
  const markdown = generateExamples(cfg).python;
  assert.match(markdown, /directives=Directives\(/);
  assert.doesNotMatch(markdown, /HtmlOptions|AttributeMatcher|recovery=/);
});

test("Python string literals preserve controls, astral scalars and lone surrogates", () => {
  const cfg = config([]);
  cfg.input = [
    'quote"',
    "slash\\",
    "\n\r\t",
    "\0",
    "\x01",
    "\x7f",
    "\x85",
    "\u2028",
    "\u2029",
    "😀",
    "\u{20000}",
    "\ud800",
    "\udfff",
  ].join("|");
  const python = generateExamples(cfg).python;
  const literal = String
    .raw`"quote\"|slash\\|\n\r\t|\x00|\x01|\x7f|\x85|\u2028|\u2029|\U0001f600|\U00020000|\ud800|\udfff"`;
  assert.ok(python.includes(`output = converter.convert(${literal})`));
  assert.doesNotMatch(python, /\ud800|\udfff|\u2028|\u2029/);
});

test("Python escapes user-controlled directive and preservation strings too", () => {
  const cfg = config([]);
  cfg.format = "html";
  cfg.requireHanja = ['漢"字\\'];
  cfg.preserveClasses = ['keep"\\'];
  cfg.preserveAttributes = ['data-"\\'];
  const python = generateExamples(cfg).python;
  assert.ok(python.includes(String.raw`require_hanja=["漢\"字\\"]`));
  assert.ok(python.includes(String.raw`preserve_classes=["keep\"\\"]`));
  assert.ok(python.includes(String.raw`AttributeMatcher("data-\"\\")`));
});

test("Python generation leaves frozen live settings unchanged", () => {
  const cfg = config(["stdict"], "ko-kp");
  Object.freeze(cfg.dicts);
  for (
    const items of [
      cfg.requireHanja,
      cfg.requireHangul,
      cfg.skipAnnotation,
      cfg.preserveClasses,
      cfg.preserveAttributes,
    ]
  ) Object.freeze(items);
  Object.freeze(cfg);
  assert.doesNotThrow(() => generateExamples(cfg).python);
});

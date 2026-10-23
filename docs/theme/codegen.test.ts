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

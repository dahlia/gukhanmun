// Gukhanmun: Measures converter construction with raw and loaded dictionaries.
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
import process from "node:process";
import { load, loadDictionary } from "../../packages/napi/index.ts";
import { stdictFst } from "../../packages/stdict-fst/index.ts";

const args = process.argv.slice(2);
const [mode, countArg, sourcePolicy = "reference-authorized"] = args;
const count = Number(countArg);
if (
  args.length > 3 ||
  (mode !== "raw" && mode !== "loaded") ||
  !Number.isSafeInteger(count) || count < 1 ||
  (sourcePolicy !== "reference-authorized" && sourcePolicy !== "ordered") ||
  typeof globalThis.gc !== "function"
) {
  console.error(
    "Usage: mise run bench-loaded-dictionaries -- <raw|loaded> <count> [reference-authorized|ordered]",
  );
  process.exit(1);
}

await load();
const source = await stdictFst();
globalThis.gc();
const before = process.memoryUsage.rss();
const start = performance.now();
const reference = mode === "loaded" ? await loadDictionary(source) : source;
const parsed = performance.now();
const converters = [];
const readings = ["도쿄", "동경"];
for (let i = 0; i < count; i++) {
  converters.push(
    await load({
      dictionaries: [{ entries: [{ hanja: "東京", reading: readings[i % 2] }] }, reference],
      homophoneWindow: "off",
      sourcePolicy,
    }),
  );
}
const built = performance.now();
globalThis.gc();
const rssDeltaMiB = (process.memoryUsage.rss() - before) / 2 ** 20;

// Keep converters live through the RSS sample, and verify the overrides
// without including conversion in either measurement.
for (const [i, converter] of converters.entries()) {
  assert.equal(converter.convert("東京"), readings[i % 2]);
}
console.log(JSON.stringify({
  mode,
  count,
  sourcePolicy,
  platform: process.platform,
  arch: process.arch,
  node: process.version,
  sourceBytes: source.data.byteLength,
  loadDictionaryMs: mode === "loaded" ? parsed - start : 0,
  constructionMs: built - parsed,
  totalMs: built - start,
  rssDeltaMiB,
}));

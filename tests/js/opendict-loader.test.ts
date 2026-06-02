// Gukhanmun: Cross-runtime regression tests for the opendict-{fst,cdb} byte loaders.
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
import {
  opendictArchaicCdbBytes,
  opendictArchaicCdbUrl,
  opendictCdbBytes,
  opendictDialectCdbBytes,
  opendictDialectCdbUrl,
  opendictGeneralCdbBytes,
  opendictGeneralCdbUrl,
  opendictNorthKoreanCdbBytes,
  opendictNorthKoreanCdbUrl,
} from "../../packages/opendict-cdb/index.ts";
import {
  opendictArchaicFstBytes,
  opendictArchaicFstUrl,
  opendictDialectFstBytes,
  opendictDialectFstUrl,
  opendictFstBytes,
  opendictGeneralFstBytes,
  opendictGeneralFstUrl,
  opendictNorthKoreanFstBytes,
  opendictNorthKoreanFstUrl,
} from "../../packages/opendict-fst/index.ts";

/** The byte loaders under test, paired with their bundled-binary URL. */
const loaders: readonly (readonly [
  string,
  (url?: URL) => Promise<Uint8Array<ArrayBuffer>>,
  URL,
])[] = [
  ["opendict-general-fst", opendictGeneralFstBytes, opendictGeneralFstUrl],
  [
    "opendict-north-korean-fst",
    opendictNorthKoreanFstBytes,
    opendictNorthKoreanFstUrl,
  ],
  ["opendict-dialect-fst", opendictDialectFstBytes, opendictDialectFstUrl],
  ["opendict-archaic-fst", opendictArchaicFstBytes, opendictArchaicFstUrl],
  ["opendict-general-cdb", opendictGeneralCdbBytes, opendictGeneralCdbUrl],
  [
    "opendict-north-korean-cdb",
    opendictNorthKoreanCdbBytes,
    opendictNorthKoreanCdbUrl,
  ],
  ["opendict-dialect-cdb", opendictDialectCdbBytes, opendictDialectCdbUrl],
  ["opendict-archaic-cdb", opendictArchaicCdbBytes, opendictArchaicCdbUrl],
];

/** Builds a `data:` URL carrying `bytes` as base64. */
function dataUrl(bytes: Uint8Array): URL {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return new URL(`data:application/octet-stream;base64,${btoa(binary)}`);
}

for (const [name, load, bundledUrl] of loaders) {
  describe(`${name}: byte loader`, () => {
    test("default load reads the bundled binary from disk", async () => {
      assert.equal(bundledUrl.protocol, "file:");
      const bytes = await load();
      assert.ok(bytes.byteLength > 0, "bundled binary must be non-empty");
    });

    test("a non-file: URL is fetched rather than read from disk", async () => {
      const payload = new Uint8Array([0x47, 0x55, 0x4b, 0x00, 0xff, 0x80]);
      const bytes = await load(dataUrl(payload));
      assert.deepEqual([...bytes], [...payload]);
    });

    test("an explicit file: URL is read from disk", async () => {
      const selfUrl = new URL(import.meta.url);
      assert.equal(selfUrl.protocol, "file:");
      const bytes = await load(selfUrl);
      const head = new TextDecoder().decode(bytes.subarray(0, 12));
      assert.equal(head, "// Gukhanmun");
    });
  });
}

describe("generic opendict byte loaders", () => {
  test("FST loader accepts explicit URLs", async () => {
    const payload = new Uint8Array([0x46, 0x53, 0x54]);
    assert.deepEqual([...(await opendictFstBytes(dataUrl(payload)))], [...payload]);
  });

  test("CDB loader accepts explicit URLs", async () => {
    const payload = new Uint8Array([0x43, 0x44, 0x42]);
    assert.deepEqual([...(await opendictCdbBytes(dataUrl(payload)))], [...payload]);
  });
});

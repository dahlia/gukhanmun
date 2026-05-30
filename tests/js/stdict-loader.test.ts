// Gukhanmun: Cross-runtime regression tests for the stdict-{fst,cdb} byte loaders.
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

// `stdictFstBytes` / `stdictCdbBytes` pick fs vs. fetch from the URL *scheme*,
// not from the host runtime.  An earlier version sniffed
// `process.versions.node` instead, which Deno also defines, so a JSR install
// run on Deno wrongly took the `node:fs` path and `readFile` rejected the
// `https:` module URL with "The URL must be of scheme file".
//
// These tests reproduce that failure mode without a network or a real JSR
// install by feeding the loaders a non-`file:` URL: a `data:` URL, whose
// bytes every runtime can `fetch()` but `node:fs` readFile rejects exactly
// like the `https:` URL did.  The buggy runtime-sniffing loader fails this on
// Node.js, Deno, and Bun alike; the scheme-based loader passes everywhere.
//
// Uses only `node:test` and `node:assert/strict`, which Node.js, Deno, and
// Bun all implement, so the file runs unchanged on all three.

import { describe, test } from "node:test";
import assert from "node:assert/strict";
import { stdictFstBytes, stdictFstUrl } from "../../packages/stdict-fst/index.ts";
import { stdictCdbBytes, stdictCdbUrl } from "../../packages/stdict-cdb/index.ts";

/** The byte loaders under test, paired with their bundled-binary URL. */
const loaders: readonly (readonly [
  string,
  (url?: URL) => Promise<Uint8Array<ArrayBuffer>>,
  URL,
])[] = [
  ["stdict-fst", stdictFstBytes, stdictFstUrl],
  ["stdict-cdb", stdictCdbBytes, stdictCdbUrl],
];

/** Builds a `data:` URL carrying `bytes` as base64. */
function dataUrl(bytes: Uint8Array): URL {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return new URL(`data:application/octet-stream;base64,${btoa(binary)}`);
}

for (const [name, load, bundledUrl] of loaders) {
  describe(`${name}: byte loader`, () => {
    // The bundled URL is `file:` in a local checkout, so the default call
    // exercises the disk-read branch and proves the binary is non-empty.
    test("default load reads the bundled binary from disk", async () => {
      assert.equal(bundledUrl.protocol, "file:");
      const bytes = await load();
      assert.ok(bytes.byteLength > 0, "bundled binary must be non-empty");
    });

    // Regression: a non-`file:` URL must be retrieved with `fetch`, not handed
    // to `node:fs` readFile.  The runtime-sniffing loader threw "The URL must
    // be of scheme file" here; the scheme-based loader round-trips the bytes.
    test("a non-file: URL is fetched rather than read from disk", async () => {
      const payload = new Uint8Array([0x47, 0x55, 0x4b, 0x00, 0xff, 0x80]);
      const bytes = await load(dataUrl(payload));
      assert.deepEqual([...bytes], [...payload]);
    });

    // The disk-read branch still works when handed an explicit `file:` URL:
    // point it at this very test file and confirm it reads back its own bytes.
    test("an explicit file: URL is read from disk", async () => {
      const selfUrl = new URL(import.meta.url);
      assert.equal(selfUrl.protocol, "file:");
      const bytes = await load(selfUrl);
      // The file opens with the GPLv3 notice block's first line.
      const head = new TextDecoder().decode(bytes.subarray(0, 12));
      assert.equal(head, "// Gukhanmun");
    });
  });
}

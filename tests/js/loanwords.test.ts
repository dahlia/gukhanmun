// Gukhanmun: Tests NIKL loanword packages across native and WASM bindings.
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
import { load as wasm } from "../../packages/wasm/index.ts";
import { load as napi } from "../../packages/napi/index.ts";
import { loanwordsFst, loanwordsFstBytes } from "../../packages/loanwords-fst/index.ts";
import { loanwordsCdb, loanwordsCdbBytes } from "../../packages/loanwords-cdb/index.ts";
import { stdictFst } from "../../packages/stdict-fst/index.ts";

for (const [name, load] of [["wasm", wasm], ["napi", napi]] as const) {
  for (const [format, dictionary] of [["fst", loanwordsFst], ["cdb", loanwordsCdb]] as const) {
    describe(`${name}: NIKL loanwords ${format}`, () => {
      test("explicit loanwords precede stdict, and a user dictionary overrides both", async () => {
        const loanwords = await dictionary(), standard = await stdictFst();
        const user = {
          format: "fst" as const,
          data: new URL("../../target/loanword-user.gukfst", import.meta.url),
        };
        const selected = await load({
          dictionaries: [loanwords, standard],
          homophoneWindow: "off",
        });
        assert.equal(selected.convert("香港"), "홍콩");
        const override = await load({
          dictionaries: [user, loanwords, standard],
          homophoneWindow: "off",
        });
        assert.equal(override.convert("香港"), "사용자");
        const reversed = await load({
          dictionaries: [standard, loanwords],
          homophoneWindow: "off",
        });
        assert.equal(reversed.convert("香港"), "샹강");
      });
      test("loanword streaming matches one-shot conversion at every character split", async () => {
        const selected = await load({ dictionaries: [await dictionary()], homophoneWindow: "off" });
        const input = "香港과 日本";
        const expected = selected.convert(input);
        for (let split = 0; split <= input.length; split++) {
          const stream = selected.stream();
          const output: string[] = [];
          const drain = (async () => {
            const reader = stream.readable.getReader();
            for (;;) {
              const item = await reader.read();
              if (item.done) break;
              output.push(item.value);
            }
          })();
          const writer = stream.writable.getWriter();
          await writer.write(input.slice(0, split));
          await writer.write(input.slice(split));
          await writer.close();
          await drain;
          assert.equal(output.join(""), expected);
        }
      });
      test("ko-kp has no implicit loanwords, but permits explicit selection", async () => {
        const plain = await load({ preset: "ko-kp", homophoneWindow: "off" });
        assert.notEqual(plain.convert("香港"), "홍콩");
        const selected = await load({
          preset: "ko-kp",
          dictionaries: [await dictionary()],
          homophoneWindow: "off",
        });
        assert.equal(selected.convert("香港"), "홍콩");
      });
    });
  }
}
for (const bytes of [loanwordsFstBytes, loanwordsCdbBytes]) {
  test("loanword byte loaders fetch non-file URLs and load their bundled binary", async () => {
    assert.deepEqual([...await bytes(new URL("data:application/octet-stream;base64,AQID"))], [
      1,
      2,
      3,
    ]);
    assert.ok((await bytes()).length > 0);
  });
}

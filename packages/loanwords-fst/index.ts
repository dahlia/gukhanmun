// Gukhanmun: NIKL loanword examples FST binary package.
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

/**
 * NIKL loanword examples prebuilt as an FST
 * binary for use with `{@link load}` from `@gukhanmun/wasm` or
 * `@gukhanmun/napi`.
 *
 * @example
 * ```ts
 * import { load } from "@gukhanmun/wasm";
 * import { loanwordsFst } from "@gukhanmun/loanwords-fst";
 *
 * const g = await load({ dictionaries: [await loanwordsFst()] });
 * console.log(g.convert("香港"));  // "홍콩"
 * ```
 *
 * @module @gukhanmun/loanwords-fst
 */

import type { FileDictionarySource } from "@gukhanmun/types";

export type { FileDictionarySource } from "@gukhanmun/types";

/** URL of the bundled NIKL loanword examples FST binary. */
export const loanwordsFstUrl: URL = new URL("./loanwords.fst", import.meta.url);

// Minimal typing for the Node.js Buffer returned by readFile.
interface NodeBuffer {
  readonly buffer: ArrayBuffer;
  readonly byteOffset: number;
  readonly byteLength: number;
}

// Minimal fs/promises interface—only what we use.
interface NodeFsPromises {
  readFile(path: URL): Promise<NodeBuffer>;
}

/**
 * Loads the bundled NIKL loanword examples as raw bytes.
 *
 * The access strategy is chosen from the URL scheme, not from the host
 * runtime.  A `file:` URL is read from disk with `node:fs/promises` (Node.js,
 * Deno, and Bun all provide it, and it is the only option since Node.js's
 * `fetch` rejects `file:` URLs); any other scheme is retrieved with `fetch`.
 *
 * Branching on the scheme is what makes a JSR install work on Deno: there
 * `import.meta.url` (and therefore {@link loanwordsFstUrl}) is an `https:` URL,
 * yet Deno also exposes `process.versions.node`, so a runtime sniff would
 * wrongly take the `node:fs` path and `readFile` would reject the `https:`
 * URL with "The URL must be of scheme file".
 *
 * @param url Location of the FST binary to read.  Defaults to the bundled
 *   {@link loanwordsFstUrl}; pass another URL to load a relocated copy.
 * @returns The FST binary as a `Uint8Array`.
 */
export async function loanwordsFstBytes(
  url: URL = loanwordsFstUrl,
): Promise<Uint8Array<ArrayBuffer>> {
  if (url.protocol === "file:") {
    // Use a non-literal specifier so TypeScript does not statically resolve
    // the node:fs/promises module type (which would require @types/node).
    const specifier: string = "node:fs/promises";
    const fs = (await import(
      /* webpackIgnore: true */ specifier
    )) as unknown as NodeFsPromises;
    const buf = await fs.readFile(url);
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  }
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch loanwords FST: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

/**
 * Loads the bundled NIKL loanword examples as a
 * {@link FileDictionarySource} ready to pass to `load({ dictionaries: [...] })`.
 *
 * @returns A `FileDictionarySource` with `format: "fst"`.
 */
export async function loanwordsFst(): Promise<FileDictionarySource> {
  return { data: await loanwordsFstBytes(), format: "fst" };
}

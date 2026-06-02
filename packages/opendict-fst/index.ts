// Gukhanmun: Open Korean Dictionary (우리말샘) FST binary package.
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
 * Open Korean Dictionary (우리말샘) categories prebuilt as FST binaries for use
 * with `{@link load}` from `@gukhanmun/wasm` or `@gukhanmun/napi`.
 *
 * @example
 * ```ts
 * import { load } from "@gukhanmun/wasm";
 * import { opendictNorthKoreanFst } from "@gukhanmun/opendict-fst";
 *
 * const g = await load({
 *   preset: "ko-kp",
 *   dictionaries: [await opendictNorthKoreanFst()],
 * });
 * console.log(g.convert("歷史와 來日"));  // "력사와 래일"
 * ```
 *
 * @module @gukhanmun/opendict-fst
 */

import type { FileDictionarySource } from "@gukhanmun/types";

export type { FileDictionarySource } from "@gukhanmun/types";

/** URL of the bundled Open Korean Dictionary 일반어 FST binary. */
export const opendictGeneralFstUrl: URL = new URL(
  "./general.fst",
  import.meta.url,
);

/** URL of the bundled Open Korean Dictionary 북한어 FST binary. */
export const opendictNorthKoreanFstUrl: URL = new URL(
  "./north-korean.fst",
  import.meta.url,
);

/** URL of the bundled Open Korean Dictionary 방언 FST binary. */
export const opendictDialectFstUrl: URL = new URL(
  "./dialect.fst",
  import.meta.url,
);

/** URL of the bundled Open Korean Dictionary 옛말 FST binary. */
export const opendictArchaicFstUrl: URL = new URL(
  "./archaic.fst",
  import.meta.url,
);

// Minimal typing for the Node.js Buffer returned by readFile.
interface NodeBuffer {
  readonly buffer: ArrayBuffer;
  readonly byteOffset: number;
  readonly byteLength: number;
}

// Minimal fs/promises interface, only what we use.
interface NodeFsPromises {
  readFile(path: URL): Promise<NodeBuffer>;
}

function canReadLocalFile(): boolean {
  const globals = globalThis as {
    Deno?: unknown;
    process?: { versions?: { bun?: unknown; node?: unknown } };
  };
  return globals.Deno !== undefined ||
    typeof globals.process?.versions?.bun === "string" ||
    typeof globals.process?.versions?.node === "string";
}

/**
 * Loads an Open Korean Dictionary FST binary as raw bytes.
 *
 * The access strategy is chosen from the URL scheme, not from the host
 * runtime. A `file:` URL is read from disk with `node:fs/promises` in
 * Node.js, Deno, and Bun; any other scheme is retrieved with `fetch`.
 *
 * @param url Location of the FST binary to read.
 * @returns The FST binary as a `Uint8Array`.
 */
export async function opendictFstBytes(
  url: URL,
): Promise<Uint8Array> {
  if (url.protocol === "file:" && canReadLocalFile()) {
    const specifier: string = "node:fs/promises";
    const fs = (await import(
      /* webpackIgnore: true */ specifier
    )) as unknown as NodeFsPromises;
    const buf = await fs.readFile(url);
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  }
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch opendict FST: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

/**
 * Loads the bundled Open Korean Dictionary 일반어 FST binary as raw bytes.
 *
 * @param url Location of the FST binary to read. Defaults to the bundled
 *   {@link opendictGeneralFstUrl}.
 * @returns The FST binary as a `Uint8Array`.
 */
export function opendictGeneralFstBytes(
  url: URL = opendictGeneralFstUrl,
): Promise<Uint8Array> {
  return opendictFstBytes(url);
}

/**
 * Loads the bundled Open Korean Dictionary 북한어 FST binary as raw bytes.
 *
 * @param url Location of the FST binary to read. Defaults to the bundled
 *   {@link opendictNorthKoreanFstUrl}.
 * @returns The FST binary as a `Uint8Array`.
 */
export function opendictNorthKoreanFstBytes(
  url: URL = opendictNorthKoreanFstUrl,
): Promise<Uint8Array> {
  return opendictFstBytes(url);
}

/**
 * Loads the bundled Open Korean Dictionary 방언 FST binary as raw bytes.
 *
 * @param url Location of the FST binary to read. Defaults to the bundled
 *   {@link opendictDialectFstUrl}.
 * @returns The FST binary as a `Uint8Array`.
 */
export function opendictDialectFstBytes(
  url: URL = opendictDialectFstUrl,
): Promise<Uint8Array> {
  return opendictFstBytes(url);
}

/**
 * Loads the bundled Open Korean Dictionary 옛말 FST binary as raw bytes.
 *
 * @param url Location of the FST binary to read. Defaults to the bundled
 *   {@link opendictArchaicFstUrl}.
 * @returns The FST binary as a `Uint8Array`.
 */
export function opendictArchaicFstBytes(
  url: URL = opendictArchaicFstUrl,
): Promise<Uint8Array> {
  return opendictFstBytes(url);
}

/**
 * Loads the bundled Open Korean Dictionary 일반어 dictionary.
 *
 * @returns A `FileDictionarySource` with `format: "fst"`.
 */
export async function opendictGeneralFst(): Promise<FileDictionarySource> {
  return { data: await opendictGeneralFstBytes(), format: "fst" };
}

/**
 * Loads the bundled Open Korean Dictionary 북한어 dictionary.
 *
 * @returns A `FileDictionarySource` with `format: "fst"`.
 */
export async function opendictNorthKoreanFst(): Promise<FileDictionarySource> {
  return { data: await opendictNorthKoreanFstBytes(), format: "fst" };
}

/**
 * Loads the bundled Open Korean Dictionary 방언 dictionary.
 *
 * @returns A `FileDictionarySource` with `format: "fst"`.
 */
export async function opendictDialectFst(): Promise<FileDictionarySource> {
  return { data: await opendictDialectFstBytes(), format: "fst" };
}

/**
 * Loads the bundled Open Korean Dictionary 옛말 dictionary.
 *
 * @returns A `FileDictionarySource` with `format: "fst"`.
 */
export async function opendictArchaicFst(): Promise<FileDictionarySource> {
  return { data: await opendictArchaicFstBytes(), format: "fst" };
}

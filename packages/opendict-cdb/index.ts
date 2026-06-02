// Gukhanmun: Open Korean Dictionary (우리말샘) CDB binary package.
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
 * Open Korean Dictionary (우리말샘) categories prebuilt as CDB binaries for use
 * with `{@link load}` from `@gukhanmun/wasm` or `@gukhanmun/napi`.
 *
 * @example
 * ```ts
 * import { load } from "@gukhanmun/napi";
 * import { opendictNorthKoreanCdb } from "@gukhanmun/opendict-cdb";
 *
 * const g = await load({
 *   preset: "ko-kp",
 *   dictionaries: [await opendictNorthKoreanCdb()],
 * });
 * console.log(g.convert("歷史와 來日"));  // "력사와 래일"
 * ```
 *
 * @module @gukhanmun/opendict-cdb
 */

import type { FileDictionarySource } from "@gukhanmun/types";

export type { FileDictionarySource } from "@gukhanmun/types";

/** URL of the bundled Open Korean Dictionary 일반어 CDB binary. */
export const opendictGeneralCdbUrl: URL = new URL(
  "./general.cdb",
  import.meta.url,
);

/** URL of the bundled Open Korean Dictionary 북한어 CDB binary. */
export const opendictNorthKoreanCdbUrl: URL = new URL(
  "./north-korean.cdb",
  import.meta.url,
);

/** URL of the bundled Open Korean Dictionary 방언 CDB binary. */
export const opendictDialectCdbUrl: URL = new URL(
  "./dialect.cdb",
  import.meta.url,
);

/** URL of the bundled Open Korean Dictionary 옛말 CDB binary. */
export const opendictArchaicCdbUrl: URL = new URL(
  "./archaic.cdb",
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

/**
 * Loads an Open Korean Dictionary CDB binary as raw bytes.
 *
 * The access strategy is chosen from the URL scheme, not from the host
 * runtime. A `file:` URL is read from disk with `node:fs/promises`; any other
 * scheme is retrieved with `fetch`.
 *
 * @param url Location of the CDB binary to read.
 * @returns The CDB binary as a `Uint8Array`.
 */
export async function opendictCdbBytes(
  url: URL,
): Promise<Uint8Array> {
  if (url.protocol === "file:") {
    const specifier: string = "node:fs/promises";
    const fs = (await import(
      /* webpackIgnore: true */ specifier
    )) as unknown as NodeFsPromises;
    const buf = await fs.readFile(url);
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  }
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch opendict CDB: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

/**
 * Loads the bundled Open Korean Dictionary 일반어 CDB binary as raw bytes.
 *
 * @param url Location of the CDB binary to read. Defaults to the bundled
 *   {@link opendictGeneralCdbUrl}.
 * @returns The CDB binary as a `Uint8Array`.
 */
export function opendictGeneralCdbBytes(
  url: URL = opendictGeneralCdbUrl,
): Promise<Uint8Array> {
  return opendictCdbBytes(url);
}

/**
 * Loads the bundled Open Korean Dictionary 북한어 CDB binary as raw bytes.
 *
 * @param url Location of the CDB binary to read. Defaults to the bundled
 *   {@link opendictNorthKoreanCdbUrl}.
 * @returns The CDB binary as a `Uint8Array`.
 */
export function opendictNorthKoreanCdbBytes(
  url: URL = opendictNorthKoreanCdbUrl,
): Promise<Uint8Array> {
  return opendictCdbBytes(url);
}

/**
 * Loads the bundled Open Korean Dictionary 방언 CDB binary as raw bytes.
 *
 * @param url Location of the CDB binary to read. Defaults to the bundled
 *   {@link opendictDialectCdbUrl}.
 * @returns The CDB binary as a `Uint8Array`.
 */
export function opendictDialectCdbBytes(
  url: URL = opendictDialectCdbUrl,
): Promise<Uint8Array> {
  return opendictCdbBytes(url);
}

/**
 * Loads the bundled Open Korean Dictionary 옛말 CDB binary as raw bytes.
 *
 * @param url Location of the CDB binary to read. Defaults to the bundled
 *   {@link opendictArchaicCdbUrl}.
 * @returns The CDB binary as a `Uint8Array`.
 */
export function opendictArchaicCdbBytes(
  url: URL = opendictArchaicCdbUrl,
): Promise<Uint8Array> {
  return opendictCdbBytes(url);
}

/**
 * Loads the bundled Open Korean Dictionary 일반어 dictionary.
 *
 * @returns A `FileDictionarySource` with `format: "cdb"`.
 */
export async function opendictGeneralCdb(): Promise<FileDictionarySource> {
  return { data: await opendictGeneralCdbBytes(), format: "cdb" };
}

/**
 * Loads the bundled Open Korean Dictionary 북한어 dictionary.
 *
 * @returns A `FileDictionarySource` with `format: "cdb"`.
 */
export async function opendictNorthKoreanCdb(): Promise<FileDictionarySource> {
  return { data: await opendictNorthKoreanCdbBytes(), format: "cdb" };
}

/**
 * Loads the bundled Open Korean Dictionary 방언 dictionary.
 *
 * @returns A `FileDictionarySource` with `format: "cdb"`.
 */
export async function opendictDialectCdb(): Promise<FileDictionarySource> {
  return { data: await opendictDialectCdbBytes(), format: "cdb" };
}

/**
 * Loads the bundled Open Korean Dictionary 옛말 dictionary.
 *
 * @returns A `FileDictionarySource` with `format: "cdb"`.
 */
export async function opendictArchaicCdb(): Promise<FileDictionarySource> {
  return { data: await opendictArchaicCdbBytes(), format: "cdb" };
}

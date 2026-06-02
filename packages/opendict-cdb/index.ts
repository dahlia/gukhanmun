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

/**
 * URL of the bundled Open Korean Dictionary 一般語 CDB binary.
 *
 * The bundled artifact is stored gzip-compressed (hence the `.cdb.gz`
 * extension) to stay within registry per-file size limits;
 * {@link opendictCdbBytes} inflates it transparently.
 */
export const opendictGeneralCdbUrl: URL = new URL(
  "./general.cdb.gz",
  import.meta.url,
);

/**
 * URL of the bundled Open Korean Dictionary 北韓語 CDB binary.
 *
 * Stored gzip-compressed; {@link opendictCdbBytes} inflates it transparently.
 */
export const opendictNorthKoreanCdbUrl: URL = new URL(
  "./north-korean.cdb.gz",
  import.meta.url,
);

/**
 * URL of the bundled Open Korean Dictionary 方言 CDB binary.
 *
 * Stored gzip-compressed; {@link opendictCdbBytes} inflates it transparently.
 */
export const opendictDialectCdbUrl: URL = new URL(
  "./dialect.cdb.gz",
  import.meta.url,
);

/**
 * URL of the bundled Open Korean Dictionary 옛말 CDB binary.
 *
 * Stored gzip-compressed; {@link opendictCdbBytes} inflates it transparently.
 */
export const opendictArchaicCdbUrl: URL = new URL(
  "./archaic.cdb.gz",
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

// Reads the raw bytes at `url` from disk (Node.js, Deno, Bun) or over the
// network (browsers and non-`file:` schemes), without any decompression.  The
// returned view is backed by a plain `ArrayBuffer` so it satisfies the web
// stream APIs used to inflate gzip members.
async function readBytes(url: URL): Promise<Uint8Array<ArrayBuffer>> {
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
    throw new Error(`Failed to fetch opendict CDB: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

// Returns whether `bytes` begins with the gzip magic number (0x1f 0x8b).
function isGzip(bytes: Uint8Array): boolean {
  return bytes.length >= 2 && bytes[0] === 0x1f && bytes[1] === 0x8b;
}

// Inflates a single gzip member back into its original bytes using the
// standard `DecompressionStream`, which is available in Node.js, Deno, Bun,
// and browsers.  Consumption starts before the write is awaited so the reader
// relieves backpressure instead of the writer deadlocking against it.
async function gunzip(bytes: Uint8Array<ArrayBuffer>): Promise<Uint8Array> {
  const decompressor = new DecompressionStream("gzip");
  const inflated = new Response(decompressor.readable).arrayBuffer();
  const writer = decompressor.writable.getWriter();
  await writer.write(bytes);
  await writer.close();
  return new Uint8Array(await inflated);
}

/**
 * Loads an Open Korean Dictionary CDB binary as raw bytes.
 *
 * A `file:` URL is read from disk with `node:fs/promises` when running in
 * Node.js, Deno, or Bun; in other runtimes (e.g. browsers) and for all
 * other schemes the bytes are retrieved with `fetch`.
 *
 * The bundled binaries are stored gzip-compressed to stay within registry
 * per-file size limits. Bytes that begin with the gzip magic number are
 * therefore inflated transparently, so the returned value is always the raw
 * CDB ready to hand to `load`; bytes that are not gzip-compressed are returned
 * unchanged.
 *
 * @param url Location of the CDB binary to read.
 * @returns The CDB binary as a `Uint8Array`.
 */
export async function opendictCdbBytes(
  url: URL,
): Promise<Uint8Array> {
  const bytes = await readBytes(url);
  return isGzip(bytes) ? await gunzip(bytes) : bytes;
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

/**
 * Standard Korean Language Dictionary (標準國語大辭典) prebuilt as a CDB
 * binary for use with `{@link load}` from `@gukhanmun/wasm` or
 * `@gukhanmun/napi`.
 *
 * @example
 * ```ts
 * import { load } from "@gukhanmun/wasm";
 * import { stdictCdb } from "@gukhanmun/stdict-cdb";
 *
 * const g = await load({ dictionaries: [await stdictCdb()] });
 * console.log(g.convert("漢字를 한글로"));  // "한자를 한글로"
 * ```
 *
 * @module @gukhanmun/stdict-cdb
 */

import type { FileDictionarySource } from "@gukhanmun/types";

export type { FileDictionarySource } from "@gukhanmun/types";

/** URL of the bundled Standard Korean Language Dictionary CDB binary. */
export const stdictCdbUrl: URL = new URL("./stdict.cdb", import.meta.url);

// Minimal typing for the Node.js Buffer returned by readFile.
interface NodeBuffer {
  readonly buffer: ArrayBuffer;
  readonly byteOffset: number;
  readonly byteLength: number;
}

// Minimal fs/promises interface — only what we use.
interface NodeFsPromises {
  readFile(path: URL): Promise<NodeBuffer>;
}

/**
 * Loads the bundled Standard Korean Language Dictionary as raw bytes.
 *
 * On Node.js uses `node:fs/promises`; on all other runtimes uses `fetch`.
 *
 * @returns The CDB binary as a `Uint8Array`.
 */
export async function stdictCdbBytes(): Promise<Uint8Array<ArrayBuffer>> {
  if (
    typeof (globalThis as { process?: { versions?: { node?: unknown } } })
      .process?.versions?.node === "string"
  ) {
    // Use a non-literal specifier so TypeScript does not statically resolve
    // the node:fs/promises module type (which would require @types/node).
    const specifier: string = "node:fs/promises";
    const fs = (await import(specifier)) as unknown as NodeFsPromises;
    const buf = await fs.readFile(stdictCdbUrl);
    return new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  }
  const response = await fetch(stdictCdbUrl);
  if (!response.ok) {
    throw new Error(`Failed to fetch stdict CDB: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

/**
 * Loads the bundled Standard Korean Language Dictionary as a
 * {@link FileDictionarySource} ready to pass to `load({ dictionaries: [...] })`.
 *
 * @returns A `FileDictionarySource` with `format: "cdb"`.
 */
export async function stdictCdb(): Promise<FileDictionarySource> {
  return { data: await stdictCdbBytes(), format: "cdb" };
}

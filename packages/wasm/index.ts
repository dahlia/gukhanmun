/**
 * @module
 *
 * WebAssembly implementation of the Gukhanmun hanja-to-hangul converter.
 *
 * Provides the same `{@link load}` / `{@link Gukhanmun}` contract as
 * `@gukhanmun/napi` but runs in any WebAssembly-capable environment —
 * browsers, Deno 2.0+, Node 20+, and Bun 1.0+.
 *
 * The WASM module is initialised lazily on the first `load()` call and
 * cached for subsequent calls.  Dictionary data (FST format) must be
 * supplied explicitly via `{@link GukhanmunOptions.dictionaries}`.
 *
 * @example
 * ```ts
 * import { load } from "@gukhanmun/wasm";
 * import { stdictFst } from "@gukhanmun/stdict-fst";
 *
 * const g = await load({ dictionaries: [await stdictFst()] });
 * console.log(g.convert("漢字를 한글로"));  // "한자를 한글로"
 * ```
 */

import type {
  ContextWindow,
  DictionarySource,
  ErrorCode,
  Format,
  Gukhanmun,
  GukhanmunOptions,
  NumeralStrategy,
  Preset,
  Recovery,
  RenderMode,
  Segmentation,
} from "@gukhanmun/types";

export type {
  ContextWindow,
  DictionaryEntry,
  DictionarySource,
  Directives,
  ErrorCode,
  FileDictionarySource,
  Format,
  Gukhanmun,
  GukhanmunFactory,
  GukhanmunOptions,
  HtmlOptions,
  NumeralStrategy,
  OriginalGloss,
  Preset,
  Recovery,
  RenderMode,
  Segmentation,
} from "@gukhanmun/types";

// ── Internal types matching wasm-bindgen exports ─────────────────────────────

/** Low-level handle returned by `new WasmGukhanmun(...)`. */
interface WasmHandleInternal {
  /** One-shot convert. */
  convert(source: string, format: unknown): string;
  /** Opens a streaming handle. */
  open_stream(format: unknown): WasmStreamInternal;
  /** Drops the Rust value. */
  free(): void;
}

/** Low-level streaming handle returned by `open_stream(...)`. */
interface WasmStreamInternal {
  /** Appends a chunk; returns partial output (empty in the current impl). */
  push(chunk: string): string;
  /** Flushes and returns the final output. */
  finish(): string;
  /** Drops the Rust value. */
  free(): void;
}

/** Shape of the wasm-bindgen generated glue module. */
interface WasmGlue {
  /**
   * Initialises the WASM module.  Accepts the new object form
   * `{ module_or_path }` expected by wasm-bindgen ≥ 0.2.93.
   */
  default(
    input?: { module_or_path: ArrayBuffer | ArrayBufferView | string | URL },
  ): Promise<unknown>;
  /** The exported `WasmGukhanmun` class constructor. */
  WasmGukhanmun: new (options: unknown, dictionaries: unknown) => WasmHandleInternal;
}

// ── Node.js detection ────────────────────────────────────────────────────────

interface NodeFsPromises {
  readFile(path: URL): Promise<{ buffer: ArrayBuffer; byteOffset: number; byteLength: number }>;
}

function isNodeLike(): boolean {
  return (
    typeof (globalThis as { process?: { versions?: { node?: unknown } } })
      .process?.versions?.node === "string"
  );
}

// ── GukhanmunError ───────────────────────────────────────────────────────────

/**
 * Error thrown by `{@link load}`, `{@link Gukhanmun.convert}`, and
 * `{@link Gukhanmun.stream}` when the Rust engine reports a failure.
 *
 * `code` identifies the failure class; `chain` carries the full causal chain
 * materialised at the FFI boundary so callers do not need additional round
 * trips.
 */
export class GukhanmunError extends Error {
  /**
   * Machine-readable error code.
   *
   * @see {@link ErrorCode}
   */
  readonly code: ErrorCode;

  /**
   * Full causal chain from the Rust `Error::source()` traversal, materialised
   * at the FFI boundary.  The first element is the root cause; the last is
   * the immediate error.
   */
  readonly chain: readonly { readonly code: ErrorCode; readonly message: string }[];

  /**
   * Creates a new `GukhanmunError`.
   *
   * @param code - Machine-readable error code.
   * @param message - Human-readable description.
   * @param chain - Optional causal chain.
   */
  constructor(
    code: ErrorCode,
    message: string,
    chain: readonly { code: ErrorCode; message: string }[] = [],
  ) {
    super(message);
    this.name = "GukhanmunError";
    this.code = code;
    this.chain = chain;
  }
}

// ── WASM initialisation ──────────────────────────────────────────────────────

let wasmInit: Promise<WasmGlue> | undefined;

/**
 * Loads and caches the WASM module.  The module URL is resolved relative to
 * this source file so it works with Deno's module graph and `import.meta.url`.
 *
 * Node.js `fetch()` does not support `file://` URLs, so on Node.js the WASM
 * binary is read via `node:fs/promises` and passed directly as a `Uint8Array`
 * to the web-target init function, bypassing the streaming-fetch path.
 * Deno and Bun support `fetch()` for `file://` URLs and use the default path.
 */
function ensureWasm(): Promise<WasmGlue> {
  if (!wasmInit) {
    const glueHref = new URL(
      "./wasm/web/gukhanmun_wasm.js",
      import.meta.url,
    ).href;
    wasmInit = (async () => {
      // Dynamic import with a runtime URL — not statically analysed so deno
      // check does not fail when the generated glue files are absent at dev
      // time; they are produced by `mise run wasm-build`.
      const mod = (await import(glueHref)) as unknown as WasmGlue;
      if (isNodeLike()) {
        const wasmUrl = new URL("./wasm/web/gukhanmun_wasm_bg.wasm", import.meta.url);
        // Non-literal specifier prevents deno check from statically resolving
        // node:fs/promises types, which would require @types/node.
        const specifier: string = "node:fs/promises";
        const fs = (await import(specifier)) as unknown as NodeFsPromises;
        const buf = await fs.readFile(wasmUrl);
        const bytes = new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
        await mod.default({ module_or_path: bytes });
      } else {
        await mod.default();
      }
      return mod;
    })();
  }
  return wasmInit;
}

// ── Dictionary loading ───────────────────────────────────────────────────────

async function resolveDictionary(
  source: DictionarySource,
): Promise<{ format: string; bytes: Uint8Array }> {
  if (source.data instanceof ArrayBuffer) {
    return { format: source.format, bytes: new Uint8Array(source.data) };
  }
  if (ArrayBuffer.isView(source.data)) {
    const view = source.data as ArrayBufferView;
    return {
      format: source.format,
      bytes: new Uint8Array(view.buffer, view.byteOffset, view.byteLength),
    };
  }
  let url: URL;
  if (source.data instanceof URL) {
    url = source.data;
  } else {
    const str = String(source.data);
    if (str.includes("://")) {
      url = new URL(str);
    } else if (isNodeLike()) {
      const specifier: string = "node:url";
      const nodeUrl = (await import(specifier)) as unknown as {
        pathToFileURL(path: string): URL;
      };
      url = nodeUrl.pathToFileURL(str);
    } else {
      throw new GukhanmunError(
        "invalid-input",
        "File path strings require a Node.js environment; use URL or ArrayBuffer in browsers",
      );
    }
  }
  if (isNodeLike() && url.protocol === "file:") {
    const specifier: string = "node:fs/promises";
    const fs = (await import(specifier)) as unknown as NodeFsPromises;
    let buf: { buffer: ArrayBuffer; byteOffset: number; byteLength: number };
    try {
      buf = await fs.readFile(url);
    } catch (e) {
      throw new GukhanmunError(
        "dictionary-load",
        `Failed to read dictionary: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
    return {
      format: source.format,
      bytes: new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength),
    };
  }
  let response: Response;
  try {
    response = await fetch(url);
  } catch (e) {
    throw new GukhanmunError(
      "dictionary-load",
      `Failed to fetch dictionary: ${e instanceof Error ? e.message : String(e)}`,
    );
  }
  if (!response.ok) {
    throw new GukhanmunError(
      "dictionary-load",
      `Failed to fetch dictionary: HTTP ${response.status}`,
    );
  }
  return { format: source.format, bytes: new Uint8Array(await response.arrayBuffer()) };
}

// ── GukhanmunImpl ────────────────────────────────────────────────────────────

/** Resolved default options stored on the `Gukhanmun` instance. */
interface ResolvedOptions {
  readonly preset: Preset;
  readonly rendering: RenderMode;
  readonly segmentation: Segmentation;
  readonly numerals: NumeralStrategy;
  readonly initialSoundLaw: boolean;
  readonly homophoneWindow: ContextWindow;
  readonly firstOccurrenceWindow: ContextWindow;
  readonly recovery: Recovery;
}

/** Internal implementation of the `Gukhanmun` contract. */
class GukhanmunImpl implements Gukhanmun {
  readonly #handle: WasmHandleInternal;
  readonly options: Readonly<ResolvedOptions>;

  constructor(handle: WasmHandleInternal, resolvedOpts: ResolvedOptions) {
    this.#handle = handle;
    this.options = resolvedOpts;
  }

  /**
   * Converts `source` in one shot.
   *
   * @param source - Input string.
   * @param format - `"text"` (default), `"html"`, `"markdown"`, or
   *   `{ format: "markdown"; gfm?: boolean }`.
   * @returns Converted string.
   * @throws {@link GukhanmunError} on conversion failure.
   */
  convert(source: string, format?: Format): string {
    try {
      return this.#handle.convert(source, format ?? null);
    } catch (e) {
      throw liftError(e);
    }
  }

  /**
   * Returns a `TransformStream` that converts string chunks.
   *
   * Chunks are buffered internally and the full conversion runs on `flush`.
   * This satisfies batch-equivalence: the output of any chunk partition equals
   * the output of `{@link convert}` on the concatenated input.
   *
   * @param format - Same as {@link convert}.
   * @returns A `TransformStream<string, string>`.
   */
  stream(format?: Format): TransformStream<string, string> {
    let streamHandle: WasmStreamInternal;
    try {
      streamHandle = this.#handle.open_stream(format ?? null);
    } catch (e) {
      throw liftError(e);
    }
    return new TransformStream<string, string>({
      transform(chunk, controller): void {
        const out = streamHandle.push(chunk);
        if (out) controller.enqueue(out);
      },
      flush(controller): void {
        try {
          const out = streamHandle.finish();
          if (out) controller.enqueue(out);
        } catch (e) {
          throw liftError(e);
        } finally {
          streamHandle.free();
        }
      },
    });
  }
}

// ── Error lifting ────────────────────────────────────────────────────────────

/** Converts a raw error object thrown from the WASM boundary into a `GukhanmunError`. */
function liftError(raw: unknown): GukhanmunError {
  if (raw instanceof GukhanmunError) return raw;
  if (raw != null && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    const code = (typeof obj["code"] === "string" ? obj["code"] : "internal") as ErrorCode;
    const message = typeof obj["message"] === "string" ? obj["message"] : String(raw);
    const chain = Array.isArray(obj["chain"])
      ? (obj["chain"] as { code: ErrorCode; message: string }[])
      : [];
    return new GukhanmunError(code, message, chain);
  }
  return new GukhanmunError("internal", String(raw));
}

// ── Resolved options helpers ─────────────────────────────────────────────────

function resolveOptions(opts: GukhanmunOptions = {}): ResolvedOptions {
  const preset = opts.preset ?? "ko-kr";
  const koKp = preset === "ko-kp";
  return {
    preset,
    rendering: opts.rendering ?? "hangul-only",
    segmentation: opts.segmentation ?? "lattice",
    numerals: opts.numerals ?? "hangul-phonetic",
    initialSoundLaw: opts.initialSoundLaw ?? (koKp ? false : true),
    homophoneWindow: opts.homophoneWindow ?? (koKp ? "off" : "per-block"),
    firstOccurrenceWindow: opts.firstOccurrenceWindow ?? "off",
    recovery: opts.recovery ?? "strict",
  };
}

// ── load() ───────────────────────────────────────────────────────────────────

/**
 * Creates a Gukhanmun converter with the given options.
 *
 * Initialises the WASM module on the first call (subsequent calls reuse the
 * cached module).  Dictionaries supplied via
 * `{@link GukhanmunOptions.dictionaries}` are fetched and passed to the Rust
 * engine as `FileDictionarySource` values.
 *
 * Note: unlike the Rust `ko-kr` preset, the JavaScript preset never includes a
 * bundled dictionary.  Pass `dictionaries: [await stdictFst()]` to include the
 * Standard Korean Language Dictionary.
 *
 * @param options - Conversion options.  All fields are optional; defaults match
 *   the `ko-kr` preset.
 * @returns A `{@link Gukhanmun}` instance.
 * @throws {@link GukhanmunError} on invalid options or dictionary load failure.
 */
export async function load(options: GukhanmunOptions = {}): Promise<Gukhanmun> {
  const wasm = await ensureWasm();
  const resolved = resolveOptions(options);

  const dicts = await Promise.all(
    (options.dictionaries ?? []).map(resolveDictionary),
  );

  const rawOpts = buildRawOptions(options, resolved);

  let handle: WasmHandleInternal;
  try {
    handle = new wasm.WasmGukhanmun(rawOpts, dicts);
  } catch (e) {
    throw liftError(e);
  }

  return new GukhanmunImpl(handle, resolved);
}

// ── Options serialisation ────────────────────────────────────────────────────

/** Builds the plain-object options passed across the WASM boundary. */
function buildRawOptions(
  opts: GukhanmunOptions,
  resolved: ResolvedOptions,
): Record<string, unknown> {
  const raw: Record<string, unknown> = {
    preset: resolved.preset,
    rendering: resolved.rendering,
    segmentation: resolved.segmentation,
    numerals: resolved.numerals,
    initialSoundLaw: resolved.initialSoundLaw,
    homophoneWindow: resolved.homophoneWindow,
    firstOccurrenceWindow: resolved.firstOccurrenceWindow,
    recovery: resolved.recovery,
  };
  if (resolved.rendering === "original" && opts.originalGloss != null) {
    raw["originalGloss"] = opts.originalGloss;
  }
  if (opts.directives != null) {
    raw["directives"] = {
      requireHanja: opts.directives.requireHanja ?? [],
      requireHangul: opts.directives.requireHangul ?? [],
      skipAnnotation: opts.directives.skipAnnotation ?? [],
    };
  }
  if (opts.html != null) {
    raw["html"] = {
      preserveClasses: opts.html.preserveClasses ?? [],
      preserveAttributes: opts.html.preserveAttributes ?? [],
    };
  }
  return raw;
}

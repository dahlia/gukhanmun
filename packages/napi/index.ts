/**
 * @module
 *
 * Node.js native addon (napi-rs) implementation of the Gukhanmun
 * hanja-to-hangul converter.
 *
 * Provides the same `{@link load}` / `{@link Gukhanmun}` contract as
 * `@gukhanmun/wasm` but uses a precompiled native addon for maximum
 * throughput.  Node.js 20+ is required.  The native addon binary
 * (`gukhanmun_napi.node`) must be present in the package directory; build it
 * locally with `mise run napi-build`.
 *
 * The `load()` factory is asynchronous for API uniformity with the WASM
 * backend, but the native addon is synchronously ready — dictionary data is
 * the only async part.
 *
 * @example
 * ```ts
 * import { load } from "@gukhanmun/napi";
 * import { stdictFst } from "@gukhanmun/stdict-fst";
 *
 * const g = await load({ dictionaries: [await stdictFst()] });
 * console.log(g.convert("漢字를 한글로"));  // "한자를 한글로"
 * ```
 */

import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";

import type {
  ContextWindow,
  DictionarySource,
  FileDictionarySource,
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
  MapDictionarySource,
  NumeralStrategy,
  OriginalGloss,
  Preset,
  Recovery,
  RenderMode,
  Segmentation,
} from "@gukhanmun/types";

// ── Native addon types ────────────────────────────────────────────────────────

/** Raw dictionary record passed to the native `NapiGukhanmun.load` factory. */
interface RawDictInput {
  format: string;
  bytes: Buffer;
}

/**
 * Opaque stream handle (`External<StreamState>` on the Rust side) returned by
 * `openStream` and passed back to `streamPush` / `streamFinish`.
 */
// deno-lint-ignore no-explicit-any
type NapiStreamHandle = any;

/** Instance methods on the native `NapiGukhanmun` class. */
interface NapiHandle {
  convert(source: string, formatJson: string | null): string;
  openStream(formatJson: string | null): NapiStreamHandle;
  streamPush(stream: NapiStreamHandle, chunk: string): string;
  streamFinish(stream: NapiStreamHandle): string;
}

/** Shape of the napi-rs generated module (`gukhanmun_napi.node`). */
interface NapiAddon {
  NapiGukhanmun: {
    load(optionsJson: string | null, dicts: RawDictInput[]): NapiHandle;
  };
}

// ── Native addon loading ──────────────────────────────────────────────────────

/**
 * Loads the native addon at module initialisation time.
 *
 * Tries `./gukhanmun_napi.node` first (when running tests directly from
 * source), then `../gukhanmun_napi.node` for the compiled output in `dist/`.
 */
const nativeAddon = (() => {
  const req = createRequire(import.meta.url);
  try {
    return req("./gukhanmun_napi.node") as NapiAddon;
  } catch {
    return req("../gukhanmun_napi.node") as NapiAddon;
  }
})();

// ── GukhanmunError ────────────────────────────────────────────────────────────

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

// ── Error lifting ─────────────────────────────────────────────────────────────

/**
 * Converts a raw error thrown at the NAPI boundary into a `GukhanmunError`.
 *
 * napi-rs encodes structured errors as JSON in the `message` field:
 * `{"code":"…","message":"…","chain":[…]}`.
 */
function liftNapiError(raw: unknown): GukhanmunError {
  if (raw instanceof GukhanmunError) return raw;
  if (raw instanceof Error) {
    try {
      const parsed = JSON.parse(raw.message) as {
        code?: string;
        message?: string;
        chain?: { code: ErrorCode; message: string }[];
      };
      if (typeof parsed.code === "string") {
        return new GukhanmunError(
          parsed.code as ErrorCode,
          typeof parsed.message === "string" ? parsed.message : raw.message,
          Array.isArray(parsed.chain) ? parsed.chain : [],
        );
      }
    } catch {
      // Not a JSON-encoded napi error; fall through.
    }
  }
  return new GukhanmunError("internal", String(raw));
}

// ── Dictionary loading ────────────────────────────────────────────────────────

async function resolveDictionary(source: DictionarySource): Promise<RawDictInput> {
  if ("format" in source) {
    const file = source as FileDictionarySource;
    let bytes: Buffer;
    if (file.data instanceof ArrayBuffer) {
      bytes = Buffer.from(file.data);
    } else if (ArrayBuffer.isView(file.data)) {
      const view = file.data as ArrayBufferView;
      bytes = Buffer.from(view.buffer as ArrayBuffer, view.byteOffset, view.byteLength);
    } else {
      let url: URL;
      if (file.data instanceof URL) {
        url = file.data;
      } else {
        const str = String(file.data);
        url = str.includes("://") ? new URL(str) : pathToFileURL(str);
      }
      if (url.protocol === "file:") {
        try {
          bytes = await readFile(fileURLToPath(url));
        } catch (e) {
          throw new GukhanmunError(
            "dictionary-load",
            `Failed to read dictionary: ${e instanceof Error ? e.message : String(e)}`,
          );
        }
      } else {
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
        bytes = Buffer.from(await response.arrayBuffer());
      }
    }
    return { format: file.format, bytes };
  }
  throw new GukhanmunError(
    "unsupported-content-type",
    "MapDictionarySource is not supported in the NAPI backend; " +
      "use FileDictionarySource with FST or CDB data",
  );
}

// ── Resolved options ──────────────────────────────────────────────────────────

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

// ── Options serialisation ─────────────────────────────────────────────────────

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

// ── GukhanmunImpl ─────────────────────────────────────────────────────────────

/** Internal implementation of the `Gukhanmun` contract backed by the native addon. */
class GukhanmunImpl implements Gukhanmun {
  readonly #handle: NapiHandle;
  readonly options: Readonly<ResolvedOptions>;

  constructor(handle: NapiHandle, resolvedOpts: ResolvedOptions) {
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
      return this.#handle.convert(source, format != null ? JSON.stringify(format) : null);
    } catch (e) {
      throw liftNapiError(e);
    }
  }

  /**
   * Returns a `TransformStream` that converts string chunks.
   *
   * Chunks are buffered internally; all output is produced on `flush`.
   * This satisfies batch-equivalence: the output of any chunk partition equals
   * the output of `{@link convert}` on the concatenated input.
   *
   * @param format - Same as {@link convert}.
   * @returns A `TransformStream<string, string>`.
   */
  stream(format?: Format): TransformStream<string, string> {
    const formatJson = format != null ? JSON.stringify(format) : null;
    let streamHandle: NapiStreamHandle;
    try {
      streamHandle = this.#handle.openStream(formatJson);
    } catch (e) {
      throw liftNapiError(e);
    }
    const handle = this.#handle;
    return new TransformStream<string, string>({
      transform(chunk, controller): void {
        const out = handle.streamPush(streamHandle, chunk);
        if (out) controller.enqueue(out);
      },
      flush(controller): void {
        try {
          const out = handle.streamFinish(streamHandle);
          if (out) controller.enqueue(out);
        } catch (e) {
          throw liftNapiError(e);
        }
      },
    });
  }
}

// ── load() ───────────────────────────────────────────────────────────────────

/**
 * Creates a Gukhanmun converter with the given options.
 *
 * The native addon is synchronously ready; dictionaries supplied via
 * `{@link GukhanmunOptions.dictionaries}` are fetched or read from disk and
 * passed to the Rust engine.  `MapDictionarySource` is not supported in the
 * NAPI backend.
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
  const resolved = resolveOptions(options);
  const dicts = await Promise.all((options.dictionaries ?? []).map(resolveDictionary));
  const optionsJson = JSON.stringify(buildRawOptions(options, resolved));
  let handle: NapiHandle;
  try {
    handle = nativeAddon.NapiGukhanmun.load(optionsJson, dicts);
  } catch (e) {
    throw liftNapiError(e);
  }
  return new GukhanmunImpl(handle, resolved);
}

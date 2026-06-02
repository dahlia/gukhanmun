// Gukhanmun: TypeScript API contract for Gukhanmun.
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
 * Canonical TypeScript API contract for Gukhanmun.
 *
 * This package contains only TypeScript type declarations and carries no
 * runtime code.  Both `@gukhanmun/wasm` and `@gukhanmun/napi` satisfy this
 * contract structurally.  All TSDoc lives here as the single source of
 * truth for the JavaScript API.
 *
 * @module @gukhanmun/types
 */

// ── Preset ────────────────────────────────────────────────────────────────

/**
 * Named configuration preset that sets orthographic and lexical defaults.
 *
 * - `"ko-kr"` — South Korean orthography: dictionary-driven readings, the
 *   initial sound law applied to fallback fragments, per-block homophone
 *   disambiguation, and the bundled *Standard Korean Language Dictionary*
 *   (標準國語大辭典).  Corresponds to Rust `Preset::KoKr`.
 * - `"ko-kp"` — North Korean orthography: no initial sound law (래일,
 *   류행, 녀자), no bundled dictionary.  Corresponds to Rust `Preset::KoKp`.
 *
 * Both presets default `rendering` to `"hangul-only"` and `segmentation` to
 * `"lattice"`.  Individual options passed to {@link GukhanmunOptions} override
 * the preset.
 */
export type Preset = "ko-kr" | "ko-kp";

// ── Render mode ────────────────────────────────────────────────────────────

/**
 * Controls how the renderer expands each converted hanja annotation into
 * output text or markup.  Corresponds to Rust `RenderMode`.
 *
 * - `"hangul-only"` — Emit only the hangul reading.  When `homophone` or
 *   `require_hanja` is set on an annotation the reading is followed by the
 *   original hanja in parentheses: `한글(漢字)`.  Corresponds to Rust
 *   `RenderMode::HangulOnly`.
 * - `"hangul-hanja-parens"` — Always emit `한글(漢字)`.  Corresponds to
 *   Rust `RenderMode::HangulHanjaParens`.
 * - `"hanja-hangul-parens"` — Always emit `漢字(한글)`.  Useful for
 *   academic and historical-document styles.  Corresponds to Rust
 *   `RenderMode::HanjaHangulParens`.
 * - `"ruby-on-hangul"` — Emit `<ruby>한글<rt>漢字</rt></ruby>`.  Falls
 *   back to parentheses when the current scope does not permit inline markup
 *   (e.g., inside `<pre>`).  Corresponds to Rust
 *   `RenderMode::Ruby(RubyBase::OnHangul)`.
 * - `"ruby-on-hanja"` — Emit `<ruby>漢字<rt>한글</rt></ruby>`.
 *   Corresponds to Rust `RenderMode::Ruby(RubyBase::OnHanja)`.
 * - `"original"` — Keep the original mixed-script form; only annotations
 *   with `require_hangul` or a user directive receive a hangul gloss, which
 *   appears either in parentheses or as a ruby element depending on
 *   {@link GukhanmunOptions.originalGloss}.  Corresponds to Rust
 *   `RenderMode::Original`.
 */
export type RenderMode =
  | "hangul-only"
  | "hangul-hanja-parens"
  | "hanja-hangul-parens"
  | "ruby-on-hangul"
  | "ruby-on-hanja"
  | "original";

/**
 * Selects how glosses are rendered when {@link RenderMode} is `"original"`.
 *
 * - `"parens"` — Wrap the gloss in parentheses: `漢字(한글)` (default).
 *   Corresponds to Rust `OriginalGloss::Parens`.
 * - `"ruby"` — Wrap the gloss in a `<ruby>` element.  Falls back to
 *   parentheses in scopes that do not permit inline markup.  Corresponds to
 *   Rust `OriginalGloss::Ruby`.
 *
 * This option is ignored when `rendering` is not `"original"`.
 */
export type OriginalGloss = "parens" | "ruby";

// ── Segmentation ───────────────────────────────────────────────────────────

/**
 * Controls how the engine segments a hanja-containing span into dictionary
 * words and fallback fragments.  Corresponds to Rust `SegmentationStrategy`.
 *
 * - `"lattice"` — Dynamic programming over all possible dictionary matches
 *   at each position; selects the segmentation that maximises dictionary
 *   coverage and then prefers fewer segments.  This is the default and
 *   produces better results than greedy approaches when a longer prefix
 *   would leave a suffix uncovered by the dictionary.  Corresponds to Rust
 *   `SegmentationStrategy::Lattice`.
 * - `"eager"` — Left-to-right longest-match (greedy).  Lower overhead per
 *   span at the cost of occasional mis-segmentation.  Corresponds to Rust
 *   `SegmentationStrategy::Eager`.
 */
export type Segmentation = "lattice" | "eager";

// ── Numeral strategy ────────────────────────────────────────────────────────

/**
 * Controls how runs of hanja numerals are converted.  Corresponds to Rust
 * `NumeralStrategy`.
 *
 * | Strategy              | `二〇一六年` | `十一月` | `一千二百三十四` |
 * | --------------------- | ------------ | -------- | ---------------- |
 * | `"hangul-phonetic"`   | 이공일륙년   | 십일월   | 일천이백삼십사   |
 * | `"positional-arabic"` | 2016년       | (n/a)    | (n/a)            |
 * | `"additive-arabic"`   | (n/a)        | 11월     | 1234             |
 * | `"smart"`             | 2016년       | 11월     | 1234             |
 *
 * - `"hangul-phonetic"` — Read every digit character-by-character in
 *   Korean phonetics.  This is Seonbi's behaviour and the preset default.
 *   Corresponds to Rust `NumeralStrategy::HangulPhonetic`.
 * - `"positional-arabic"` — Treat a run of digit-only hanja
 *   (`〇一二三四五六七八九` and variants) as positional (place-value)
 *   notation and convert to Arabic.  Corresponds to Rust
 *   `NumeralStrategy::PositionalArabic`.
 * - `"additive-arabic"` — Parse sequences containing place markers
 *   (`十百千萬億兆京`) using stack-based accumulation and produce Arabic,
 *   respecting the Korean convention that bare `十` means 10 not `一十`.
 *   Corresponds to Rust `NumeralStrategy::AdditiveArabic`.
 * - `"smart"` — Uses `"additive-arabic"` when a unit hanja follows the
 *   numeral (`年月日時分秒號世紀` and others); uses `"positional-arabic"`
 *   for pure-digit runs of four or more characters (year convention);
 *   otherwise falls back to `"hangul-phonetic"`.  Corresponds to Rust
 *   `NumeralStrategy::Smart`.
 */
export type NumeralStrategy =
  | "hangul-phonetic"
  | "positional-arabic"
  | "additive-arabic"
  | "smart";

// ── Context window ─────────────────────────────────────────────────────────

/**
 * Defines the scope within which the homophone marker and first-occurrence
 * filter track previously seen readings.  Corresponds to Rust `ContextWindow`.
 *
 * - `"off"` — Disable the corresponding middleware entirely.
 * - `"per-block"` — Reset at each block boundary (paragraph, list item,
 *   heading, …).  This is the default for both homophone marking and first-
 *   occurrence filtering.  In plain text, which has no block scopes, `per-block`
 *   is document-wide.
 * - `"per-section"` — Reset at each heading boundary (HTML `<h1>`–`<h6>`,
 *   Markdown ATX/setext headings).
 * - `"per-document"` — Track across the entire document.  This buffers the
 *   entire token stream and is appropriate only for small inputs or when full
 *   accuracy matters more than latency.
 */
export type ContextWindow =
  | "off"
  | "per-block"
  | "per-section"
  | "per-document";

// ── Homophone detection ──────────────────────────────────────────────────────

/**
 * Selects how the homophone marker decides that a reading needs its hanja shown
 * in `rendering: "hangul-only"`.  Corresponds to Rust `HomophoneDetection`.
 *
 * - `"context-local"` — Gloss a reading only when a different-meaning homophone
 *   actually appears within the {@link ContextWindow}.  This keeps hangul-only
 *   output clean and is the default.
 * - `"dictionary-wide"` — Also gloss readings shared by other hanja forms
 *   anywhere in the dictionary, even when no homophone appears in the text.
 *   With a large reference dictionary this glosses most Sino-Korean words; words
 *   that should always be glossed are better expressed with `requireHanja`.
 */
export type HomophoneDetection = "context-local" | "dictionary-wide";

// ── Recovery ───────────────────────────────────────────────────────────────

/**
 * Controls how the pipeline handles reader errors encountered during HTML
 * scanning.  Corresponds to Rust `Recovery`.
 *
 * - `"strict"` — Propagate the error and stop (default).
 * - `"lenient"` — Log the error via `tracing` and emit a verbatim token for
 *   the unrecognised region so that downstream tokens continue to flow.
 *
 * This option is meaningful only for `format: "html"`.  Markdown parsing does
 * not produce recoverable errors, so this option is ignored for Markdown input.
 */
export type Recovery = "strict" | "lenient";

// ── Dictionary sources ─────────────────────────────────────────────────────

/**
 * A single dictionary entry returned by a dictionary lookup.
 */
export interface DictionaryEntry {
  /** The hanja form (key), e.g. `"漢字"`. */
  readonly hanja: string;
  /** The hangul reading, e.g. `"한자"`. */
  readonly reading: string;
  /**
   * When `true`, the renderer should always show the original hanja
   * alongside the hangul reading, regardless of ambiguity.
   *
   * Corresponds to Rust `MatchMark::require_hanja`.
   */
  readonly requireHanja?: boolean;
  /**
   * When `true`, the renderer should always show a hangul gloss alongside
   * the original hanja (used with `rendering: "original"`).
   *
   * Corresponds to Rust `MatchMark::require_hangul`.
   */
  readonly requireHangul?: boolean;
}

/**
 * Specifies a dictionary loaded from a binary file or URL.
 *
 * The `data` field accepts:
 * - A `BufferSource` (`ArrayBuffer` or `ArrayBufferView`) — supported in all
 *   environments.
 * - A `URL` — resolved via `fetch` in browsers; via `node:fs/promises`
 *   in Node.js, Deno 2.0+, and Bun.
 * - A `string` — treated as a filesystem path; supported in Node.js,
 *   Deno 2.0+, and Bun only.  Throws in browser environments.
 *
 * At runtime, a `FileDictionarySource` is distinguished from other values by
 * the presence of a `format` property (`"format" in source`).
 */
export interface FileDictionarySource {
  /**
   * The binary dictionary data or a reference to where it can be loaded.
   *
   * Pass a `BufferSource` for data already in memory, a `URL` for a remote
   * or local URL (resolved via `fetch` or `readFile`), or a path `string`
   * for filesystem paths (Node.js / Deno 2.0+ / Bun only).
   */
  readonly data: ArrayBuffer | ArrayBufferView | URL | string;
  /**
   * The on-disk format of the dictionary file.
   *
   * - `"fst"` — Gukhanmun FST file (`*.gukfst`); preferred for small
   *   WebAssembly bundles.  Supported in all runtimes.
   * - `"cdb"` — Gukhanmun CDB-trie file (`*.gukcdb`); preferred when code
   *   auditability or trivial mmap support matters.  Requires a filesystem
   *   or in-memory bytes; supported in Node-API and (with `from_bytes`) in
   *   WASM builds that include the `cdb` feature.
   *
   * The `"tsv"` format is reserved for future use; passing it throws
   * `GukhanmunError` with code `"unsupported-content-type"`.
   */
  readonly format: "fst" | "cdb";
}

/**
 * A dictionary source accepted by {@link GukhanmunOptions.dictionaries}.
 *
 * Currently only {@link FileDictionarySource} (binary file / URL / path) is
 * supported.  Sources are tried in array order; the first match wins.
 *
 * @example
 * ```ts
 * import { stdictFst } from "@gukhanmun/stdict-fst";
 * const g = await load({ dictionaries: [await stdictFst()] });
 * ```
 */
export type DictionarySource = FileDictionarySource;

// ── HTML options ────────────────────────────────────────────────────────────

/**
 * Fine-grained HTML preservation rules passed in
 * {@link GukhanmunOptions.html}.
 *
 * These are additive: a scope is preserved when *any* rule matches.  They
 * correspond to the CLI flags `--html-preserve-class` and
 * `--html-preserve-attr`, and to the Rust `Builder::html_preserve_when`
 * predicate.
 */
export interface HtmlOptions {
  /**
   * Class names whose containing element (and all descendants) should be
   * treated as a preserved region—the engine skips their text content.
   *
   * Equivalent to passing `--html-preserve-class NAME` to the CLI one or
   * more times.
   */
  readonly preserveClasses?: readonly string[];
  /**
   * Attribute matchers; an element is preserved when it carries a matching
   * attribute.  Each entry is either:
   * - A bare string — preserve any element that has the attribute, regardless
   *   of value (e.g. `"data-no-translate"`).
   * - An object `{ name, value? }` — preserve elements where the attribute
   *   equals `value`, or has the attribute when `value` is omitted.
   *
   * Equivalent to `--html-preserve-attr KEY[=VALUE]` on the CLI.
   */
  readonly preserveAttributes?: readonly (
    | string
    | { readonly name: string; readonly value?: string }
  )[];
}

// ── Directives ─────────────────────────────────────────────────────────────

/**
 * Per-hanja rendering directives that override the dictionary's own marks.
 *
 * Each list contains hanja forms (exact string matches, e.g. `"漢字"`).
 * JavaScript bindings expose only the literal-set form; glob and predicate
 * variants are available in the Rust API only.
 *
 * Corresponds to Rust `UserDirectives` with `DirectiveAction::RequireHanja`,
 * `DirectiveAction::RequireHangul`, and `DirectiveAction::SkipAnnotation`.
 */
export interface Directives {
  /**
   * Hanja forms that must always be shown with their original hanja
   * alongside the hangul reading, as if `requireHanja` were set in the
   * dictionary.
   */
  readonly requireHanja?: readonly string[];
  /**
   * Hanja forms that must always be shown with a hangul gloss alongside the
   * original hanja (relevant for `rendering: "original"`).
   */
  readonly requireHangul?: readonly string[];
  /**
   * Hanja forms whose annotation should be suppressed entirely; the renderer
   * emits only the primary plain text form (hangul or hanja depending on
   * `rendering`).
   */
  readonly skipAnnotation?: readonly string[];
}

// ── Conversion options ─────────────────────────────────────────────────────

/**
 * Full set of options passed to {@link GukhanmunFactory.load} (or the
 * top-level `load` function) to configure a {@link Gukhanmun} instance.
 *
 * All fields are optional.  When a `preset` is specified it supplies
 * defaults; individual fields override those defaults.  When no preset is
 * given, `"ko-kr"` is implicitly used.
 */
export interface GukhanmunOptions {
  /**
   * Named configuration preset.  Defaults to `"ko-kr"`.
   *
   * @see {@link Preset}
   */
  readonly preset?: Preset;
  /**
   * How annotations are rendered into output text or markup.  Defaults to
   * `"hangul-only"`.
   *
   * @see {@link RenderMode}
   */
  readonly rendering?: RenderMode;
  /**
   * How glosses are rendered when `rendering` is `"original"`.  Ignored for
   * all other render modes.  Defaults to `"parens"`.
   *
   * @see {@link OriginalGloss}
   */
  readonly originalGloss?: OriginalGloss;
  /**
   * Hanja-span segmentation algorithm.  Defaults to `"lattice"`.
   *
   * @see {@link Segmentation}
   */
  readonly segmentation?: Segmentation;
  /**
   * How runs of hanja numerals are converted.  Defaults to
   * `"hangul-phonetic"`.
   *
   * @see {@link NumeralStrategy}
   */
  readonly numerals?: NumeralStrategy;
  /**
   * Whether to apply the Korean initial sound law (頭音法則) to fallback
   * phonetic readings.  Defaults to `true` for `"ko-kr"` and `false` for
   * `"ko-kp"`.
   *
   * Note: dictionary entries are assumed to encode the correct reading
   * already; this flag only affects the character-by-character fallback path.
   */
  readonly initialSoundLaw?: boolean;
  /**
   * Context window for homophone disambiguation.  The `HomophoneMarker`
   * middleware sets `homophone = true` on annotations whose hangul reading
   * collides within this window.  Defaults to `"per-block"`.
   *
   * @see {@link ContextWindow}
   */
  readonly homophoneWindow?: ContextWindow;
  /**
   * Strategy that decides which readings count as homophones needing a hanja
   * gloss.  Defaults to `"context-local"`, which glosses a reading only when a
   * different-meaning homophone appears within {@link homophoneWindow}.  Use
   * `"dictionary-wide"` to also gloss readings shared by other dictionary
   * entries.
   *
   * @see {@link HomophoneDetection}
   */
  readonly homophoneDetection?: HomophoneDetection;
  /**
   * Context window for first-occurrence filtering.  The
   * `FirstOccurrenceFilter` middleware clears `requireHanja` /
   * `requireHangul` on repeated occurrences of the same word within this
   * window, so the gloss appears only the first time.  Defaults to `"off"`
   * (filter disabled) in both presets.
   *
   * @see {@link ContextWindow}
   */
  readonly firstOccurrenceWindow?: ContextWindow;
  /**
   * Whether to collapse redundant parenthetical reading annotations.  Defaults
   * to `true`.
   *
   * When enabled, an explicit gloss such as `庫間(곳간)` or `곳간(庫間)` is
   * recognised, the redundant parenthetical is removed, and the annotation is
   * shown in both scripts in every render mode.  A parenthetical that pins an
   * alternative reading (for example `數字(수자)`) overrides the dictionary
   * reading for that occurrence.  A parenthetical that is a definition rather
   * than a reading (for example `庫間(물건을 간직하여 두는 곳)`) is left
   * untouched.
   */
  readonly collapseRedundantParens?: boolean;
  /**
   * Error recovery policy for HTML scanning.  Defaults to `"strict"`.
   * Ignored for non-HTML input formats.
   *
   * @see {@link Recovery}
   */
  readonly recovery?: Recovery;
  /**
   * Ordered list of dictionary sources.  Sources are queried in order;
   * earlier entries take precedence.  When omitted (or empty), only the
   * fallback Unihan character map is used (no stdict).
   *
   * Unlike the Rust and CLI presets, JavaScript presets do **not**
   * automatically include bundled dictionary data.  To use the *Standard
   * Korean Language Dictionary*, add `@gukhanmun/stdict-fst` or
   * `@gukhanmun/stdict-cdb` explicitly.  To use Open Korean Dictionary
   * categories, add `@gukhanmun/opendict-fst` or
   * `@gukhanmun/opendict-cdb` explicitly.
   *
   * @see {@link DictionarySource}
   */
  readonly dictionaries?: readonly DictionarySource[];
  /**
   * Per-hanja rendering directives that override dictionary marks.
   *
   * @see {@link Directives}
   */
  readonly directives?: Directives;
  /**
   * HTML-specific preservation rules.  Ignored for non-HTML input formats.
   *
   * @see {@link HtmlOptions}
   */
  readonly html?: HtmlOptions;
}

// ── Format ─────────────────────────────────────────────────────────────────

/**
 * Input / output format for {@link Gukhanmun.convert} and
 * {@link Gukhanmun.stream}.
 *
 * - `"text"` — Plain text (default).  No markup interpretation; ruby
 *   rendering falls back to parentheses.
 * - `"html"` — HTML fragment.  The scanner is fragment-oriented and recovers
 *   from minor malformations.
 * - `"markdown"` — CommonMark Markdown (GFM disabled by default).
 * - `{ format: "markdown"; gfm?: boolean }` — Markdown with optional GFM
 *   extensions.  Set `gfm: true` to enable GitHub Flavored Markdown tables,
 *   strikethrough, and task lists.
 *
 * The object form `{ format: "markdown" }` is equivalent to the string
 * `"markdown"`.
 */
export type Format =
  | "text"
  | "html"
  | "markdown"
  | { readonly format: "markdown"; readonly gfm?: boolean };

// ── Core interface ─────────────────────────────────────────────────────────

/**
 * A configured hanja-to-hangul converter.  Created by calling
 * {@link GukhanmunFactory.load} (or the top-level `load` function).
 *
 * The instance is immutable after creation; call `load` again to obtain a
 * converter with different options.
 */
export interface Gukhanmun {
  /**
   * Converts `source` to hangul in one shot.  Buffers the entire input
   * before returning.
   *
   * @param source - The text to convert.
   * @param format - Input / output format.  Defaults to `"text"`.
   * @returns The converted text.
   * @throws {@link GukhanmunError} on conversion failure.
   */
  convert(source: string, format?: Format): string;

  /**
   * Returns a `TransformStream<string, string>` that converts chunks
   * incrementally.  Chunks are JavaScript strings; byte-level encoding is
   * the caller's responsibility (`TextDecoderStream` / `TextEncoderStream`).
   *
   * The stream guarantees that the concatenated output equals the result of
   * calling `convert` on the concatenated input, regardless of chunk
   * boundaries.  Document-wide middlewares (e.g., homophone marking with
   * `homophoneWindow: "per-document"`) buffer until the writable side is
   * closed.
   *
   * @param format - Input / output format.  Defaults to `"text"`.
   * @returns A platform `TransformStream<string, string>`.
   * @throws {@link GukhanmunError} on initialisation failure (not on chunk
   *   errors; those are signalled via the stream's error channel).
   */
  stream(format?: Format): TransformStream<string, string>;

  /**
   * Read-only view of the resolved options (after preset defaults are
   * applied).  Excludes `dictionaries`, `directives`, `html`, and
   * `originalGloss`, which are not meaningfully representable as plain
   * values.
   */
  readonly options: Readonly<
    Required<
      Omit<
        GukhanmunOptions,
        "dictionaries" | "directives" | "html" | "originalGloss"
      >
    >
  >;
}

// ── Factory interface ───────────────────────────────────────────────────────

/**
 * Factory interface satisfied by both `@gukhanmun/wasm` and
 * `@gukhanmun/napi`.
 *
 * @example
 * ```ts
 * import { load } from "@gukhanmun/wasm";
 * import { stdictFst } from "@gukhanmun/stdict-fst";
 *
 * const g = await load({
 *   preset: "ko-kr",
 *   dictionaries: [await stdictFst()],
 * });
 * console.log(g.convert("漢字를 한글로"));
 * ```
 */
export interface GukhanmunFactory {
  /**
   * Loads and initialises a {@link Gukhanmun} converter with the given
   * options.
   *
   * In the WASM implementation this involves asynchronous `.wasm` binary
   * initialisation; in the Node-API implementation the native addon is
   * synchronously ready but still returns a `Promise` for API uniformity.
   * Dictionary sources with `URL` or string `data` are fetched / read during
   * this call.
   *
   * @param options - Conversion options.  All fields are optional; unset
   *   fields inherit defaults from the selected `preset` (or `"ko-kr"` when
   *   no preset is given).
   * @returns A ready-to-use {@link Gukhanmun} instance.
   * @throws {@link GukhanmunError} when an option value is unrecognised
   *   (`code: "invalid-input"`) or a dictionary fails to load
   *   (`code: "dictionary-load"`).
   */
  load(options?: GukhanmunOptions): Promise<Gukhanmun>;
}

/**
 * Top-level entry point exported by both `@gukhanmun/wasm` and
 * `@gukhanmun/napi` as a named export.
 *
 * Equivalent to `new GukhanmunFactory().load(options)`.  Declared here so
 * that code that `import { load }` from either implementation package
 * type-checks against the same signature.
 */
export declare const load: GukhanmunFactory["load"];

// ── Error ──────────────────────────────────────────────────────────────────

/**
 * Discriminant code carried by every {@link GukhanmunError}.
 *
 * - `"dictionary-load"` — A dictionary file could not be opened, read, or
 *   decoded.
 * - `"segmentation"` — The lattice segmenter encountered an internal
 *   inconsistency.
 * - `"invalid-reading"` — A dictionary entry's hangul reading is not valid
 *   hangul.
 * - `"html-scan"` — The HTML scanner encountered an unrecoverable error.
 * - `"html-malformed-attr"` — An HTML attribute string could not be parsed.
 * - `"markdown"` — The Markdown adapter encountered a parsing error.
 * - `"unsupported-content-type"` — An unrecognised format string was passed
 *   to `convert` or `stream`.
 * - `"invalid-input"` — An option value is not in the expected set (e.g. an
 *   unrecognised preset or render mode string).
 * - `"io"` — An I/O error occurred (file read, network, …).
 * - `"internal"` — An internal invariant was violated; this is a bug.
 * - `"other"` — Any other error not covered by the above codes.
 */
export type ErrorCode =
  | "dictionary-load"
  | "segmentation"
  | "invalid-reading"
  | "html-scan"
  | "html-malformed-attr"
  | "markdown"
  | "unsupported-content-type"
  | "invalid-input"
  | "io"
  | "internal"
  | "other";

/**
 * Error class thrown by all Gukhanmun operations.
 *
 * Declared here as a `declare class` so this package remains purely
 * type-level.  The actual class (with identical shape) is provided by each
 * runtime package (`@gukhanmun/wasm` and `@gukhanmun/napi`).
 *
 * The `chain` property exposes the Rust `Error::source()` chain materialised
 * at the FFI boundary, allowing callers to inspect underlying causes without
 * additional FFI calls.
 */
export declare class GukhanmunError extends Error {
  /**
   * Machine-readable discriminant for the error.
   *
   * @see {@link ErrorCode}
   */
  readonly code: ErrorCode;
  /**
   * Ordered list of underlying causes, derived from the Rust
   * `std::error::Error::source()` chain.  The first entry is the immediate
   * cause; subsequent entries are deeper causes.  Empty when there is no
   * chain.
   */
  readonly chain: readonly {
    readonly code: ErrorCode;
    readonly message: string;
  }[];
}

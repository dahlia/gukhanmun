// Gukhanmun: Generates CLI/Rust/JavaScript example code from playground state.
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

import type {
  ContextWindow,
  HomophoneDetection,
  NumeralStrategy,
  OriginalGloss,
  Recovery,
  RenderMode,
  Segmentation,
} from "@gukhanmun/wasm";

// Identifiers for every built-in dictionary the playground can toggle: the
// Standard Korean Language Dictionary plus the four Open Korean Dictionary
// (우리말샘) categories.
export type DictId =
  | "stdict"
  | "opendict-general"
  | "opendict-north-korean"
  | "opendict-dialect"
  | "opendict-archaic";

// Array order = lookup priority (earlier wins).  The Open Korean Dictionary
// entries are listed before stdict so North Korean orthography can outrank the
// South Korean reading when both are enabled, matching the opendict crate's
// documented intent that `north_korean()` be prioritized above the South
// Korean dictionary.
export const DICT_ORDER: DictId[] = [
  "opendict-north-korean",
  "opendict-general",
  "opendict-dialect",
  "opendict-archaic",
  "stdict",
];

// Per-target identifiers for each dictionary: how to refer to it from the
// JavaScript packages, the Rust crates, and the CLI's `--dictionary` flag.
interface DictMeta {
  jsPackage: string; // import source for the loader
  jsLoader: string; // FileDictionarySource loader function
  rustExpr: string; // &'static dictionary accessor expression
  cliFile: string; // file name for the CLI `--dictionary` fallback
}

// The Rust accessors use the umbrella `gukhanmun` crate's re-exports
// (`gukhanmun::stdict`, `gukhanmun::opendict`) so an example needs only the one
// dependency rather than the individual dictionary crates.
const DICT_META: Record<DictId, DictMeta> = {
  "stdict": {
    jsPackage: "@gukhanmun/stdict-fst",
    jsLoader: "stdictFst",
    rustExpr: "gukhanmun::stdict::ko_kr()",
    cliFile: "stdict.fst",
  },
  "opendict-general": {
    jsPackage: "@gukhanmun/opendict-fst",
    jsLoader: "opendictGeneralFst",
    rustExpr: "gukhanmun::opendict::general()",
    cliFile: "opendict-general.fst",
  },
  "opendict-north-korean": {
    jsPackage: "@gukhanmun/opendict-fst",
    jsLoader: "opendictNorthKoreanFst",
    rustExpr: "gukhanmun::opendict::north_korean()",
    cliFile: "opendict-north-korean.fst",
  },
  "opendict-dialect": {
    jsPackage: "@gukhanmun/opendict-fst",
    jsLoader: "opendictDialectFst",
    rustExpr: "gukhanmun::opendict::dialect()",
    cliFile: "opendict-dialect.fst",
  },
  "opendict-archaic": {
    jsPackage: "@gukhanmun/opendict-fst",
    jsLoader: "opendictArchaicFst",
    rustExpr: "gukhanmun::opendict::archaic()",
    cliFile: "opendict-archaic.fst",
  },
};

// The configuration the generators turn into code.  Mirrors the playground's
// live state; `activePreset` is the derived preset (null when the settings
// match no preset).
export interface CodegenConfig {
  activePreset: "ko-kr" | "ko-kp" | null;
  format: "text" | "markdown" | "html";
  input: string;
  rendering: RenderMode;
  originalGloss: OriginalGloss;
  segmentation: Segmentation;
  numerals: NumeralStrategy;
  initialSoundLaw: boolean;
  homophoneWindow: ContextWindow;
  homophoneDetection: HomophoneDetection;
  firstOccurrenceWindow: ContextWindow;
  recovery: Recovery;
  dicts: Record<DictId, boolean>;
  requireHanja: string[];
  requireHangul: string[];
  skipAnnotation: string[];
  preserveClasses: string[];
  preserveAttributes: string[];
}

// The library default configuration, which equals the ko-kr preset.  Options
// matching these are omitted from the generated code (concise output).
const DEFAULTS = {
  rendering: "hangul-only",
  originalGloss: "parens",
  segmentation: "lattice",
  numerals: "hangul-phonetic",
  initialSoundLaw: true,
  homophoneWindow: "per-block",
  homophoneDetection: "context-local",
  firstOccurrenceWindow: "off",
  recovery: "strict",
} as const;

const CLI_DEFAULTS = {
  ...DEFAULTS,
  numerals: "smart",
} as const;

// Whether the input format is HTML (gates recovery and HTML preservation).
function isHtml(cfg: CodegenConfig): boolean {
  return cfg.format === "html";
}

// The enabled dictionaries in priority order.
function enabledDicts(cfg: CodegenConfig): DictId[] {
  return DICT_ORDER.filter((id) => cfg.dicts[id]);
}

// ── Rust value maps ────────────────────────────────────────────────────────

const RUST_RENDER: Record<RenderMode, string> = {
  "hangul-only": "RenderMode::HangulOnly",
  "hangul-hanja-parens": "RenderMode::HangulHanjaParens",
  "hanja-hangul-parens": "RenderMode::HanjaHangulParens",
  "ruby-on-hangul": "RenderMode::Ruby(RubyBase::OnHangul)",
  "ruby-on-hanja": "RenderMode::Ruby(RubyBase::OnHanja)",
  "original": "RenderMode::Original",
};

const RUST_SEGMENTATION: Record<Segmentation, string> = {
  "lattice": "SegmentationStrategy::Lattice",
  "eager": "SegmentationStrategy::Eager",
};

const RUST_NUMERALS: Record<NumeralStrategy, string> = {
  "hangul-phonetic": "NumeralStrategy::HangulPhonetic",
  "positional-arabic": "NumeralStrategy::PositionalArabic",
  "additive-arabic": "NumeralStrategy::AdditiveArabic",
  "smart": "NumeralStrategy::Smart",
};

const RUST_WINDOW: Record<ContextWindow, string> = {
  "off": "ContextWindow::Off",
  "per-block": "ContextWindow::PerBlock",
  "per-section": "ContextWindow::PerSection",
  "per-document": "ContextWindow::PerDocument",
};

const RUST_DETECTION: Record<HomophoneDetection, string> = {
  "context-local": "HomophoneDetection::ContextLocal",
  "dictionary-wide": "HomophoneDetection::DictionaryWide",
};

const RUST_RECOVERY: Record<Recovery, string> = {
  "strict": "Recovery::Strict",
  "lenient": "Recovery::Lenient",
};

const RUST_ORIGINAL_GLOSS: Record<OriginalGloss, string> = {
  "parens": "OriginalGloss::Parens",
  "ruby": "OriginalGloss::Ruby",
};

const RUST_PRESET: Record<"ko-kr" | "ko-kp", string> = {
  "ko-kr": "Preset::KoKr",
  "ko-kp": "Preset::KoKp",
};

// ── Input literal escaping ─────────────────────────────────────────────────

// A Rust raw string literal with enough `#` guards to contain the input.
function rustRawString(input: string): string {
  let hashes = 1;
  // Need more `#` than the longest `"#…#` run already present.
  const runs = input.match(/"#+/g) ?? [];
  for (const run of runs) hashes = Math.max(hashes, run.length); // run.length = 1 (") + #count
  const guard = "#".repeat(hashes);
  return `r${guard}"${input}"${guard}`;
}

// A JavaScript template-literal body, escaping the three sequences that would
// otherwise terminate or interpolate it.
function jsTemplate(input: string): string {
  const escaped = input
    .replace(/\\/g, "\\\\")
    .replace(/`/g, "\\`")
    .replace(/\$\{/g, "\\${");
  return `\`${escaped}\``;
}

// A double-quoted string for a list item (directive/class/attribute name).
function quote(value: string): string {
  return JSON.stringify(value);
}

// A POSIX-shell single-quoted argument, so user-supplied values (which may hold
// spaces, globs, or shell metacharacters) cannot alter the copied command.
function shellQuote(value: string): string {
  return `'${value.replace(/'/g, "'\\''")}'`;
}

// ── Rust ───────────────────────────────────────────────────────────────────

export function generateRust(cfg: CodegenConfig): string {
  const html = isHtml(cfg);
  const symbols = new Set<string>(["Builder"]);
  const lines: string[] = [];

  if (cfg.activePreset) {
    symbols.add("Preset");
    lines.push(`let converter = Builder::with_preset(${RUST_PRESET[cfg.activePreset]})`);
  } else {
    lines.push(`let converter = Builder::new()`);
    // Dictionaries: stdict is the default bundle, so {stdict} needs nothing.
    const dicts = enabledDicts(cfg);
    const onlyStdict = dicts.length === 1 && dicts[0] === "stdict";
    if (!onlyStdict) {
      lines.push(`    .no_bundled_dictionaries()`);
      for (const id of dicts) {
        lines.push(`    .push_dictionary(${DICT_META[id].rustExpr})`);
      }
    }
    // Option setters (only those differing from the defaults).
    if (cfg.rendering !== DEFAULTS.rendering) {
      symbols.add("RenderMode");
      if (cfg.rendering.startsWith("ruby")) symbols.add("RubyBase");
      if (cfg.rendering === "original" && cfg.originalGloss !== DEFAULTS.originalGloss) {
        symbols.add("RenderOptions");
        symbols.add("OriginalGloss");
        lines.push(
          `    .rendering(RenderOptions { mode: ${RUST_RENDER[cfg.rendering]}, ` +
            `original_gloss: ${RUST_ORIGINAL_GLOSS[cfg.originalGloss]} })`,
        );
      } else {
        lines.push(`    .rendering(${RUST_RENDER[cfg.rendering]})`);
      }
    }
    if (cfg.segmentation !== DEFAULTS.segmentation) {
      symbols.add("SegmentationStrategy");
      lines.push(`    .segmentation(${RUST_SEGMENTATION[cfg.segmentation]})`);
    }
    if (cfg.numerals !== DEFAULTS.numerals) {
      symbols.add("NumeralStrategy");
      lines.push(`    .numerals(${RUST_NUMERALS[cfg.numerals]})`);
    }
    if (cfg.initialSoundLaw !== DEFAULTS.initialSoundLaw) {
      lines.push(`    .initial_sound_law(${cfg.initialSoundLaw})`);
    }
    if (cfg.homophoneWindow !== DEFAULTS.homophoneWindow) {
      symbols.add("ContextWindow");
      lines.push(`    .homophone_window(${RUST_WINDOW[cfg.homophoneWindow]})`);
    }
    if (cfg.homophoneDetection !== DEFAULTS.homophoneDetection) {
      symbols.add("HomophoneDetection");
      lines.push(`    .homophone_detection(${RUST_DETECTION[cfg.homophoneDetection]})`);
    }
    if (cfg.firstOccurrenceWindow !== DEFAULTS.firstOccurrenceWindow) {
      symbols.add("ContextWindow");
      lines.push(`    .first_occurrence_window(${RUST_WINDOW[cfg.firstOccurrenceWindow]})`);
    }
    if (html && cfg.recovery !== DEFAULTS.recovery) {
      symbols.add("Recovery");
      lines.push(`    .recovery(${RUST_RECOVERY[cfg.recovery]})`);
    }
  }

  // Directives apply regardless of preset.
  const directives: [string[], string][] = [
    [cfg.requireHanja, "RequireHanja"],
    [cfg.requireHangul, "RequireHangul"],
    [cfg.skipAnnotation, "SkipAnnotation"],
  ];
  for (const [items, action] of directives) {
    for (const item of items) {
      symbols.add("DirectiveAction");
      lines.push(`    .directive(${quote(item)}, DirectiveAction::${action})`);
    }
  }

  // HTML preservation (simplified attribute-text match; the engine's real
  // has_class/has_attribute helpers are internal to the bindings).
  if (html && (cfg.preserveClasses.length || cfg.preserveAttributes.length)) {
    const checks: string[] = [
      ...cfg.preserveClasses.map((c) => `info.raw_attributes.contains(${quote(`class="${c}"`)})`),
      ...cfg.preserveAttributes.map((a) => `info.raw_attributes.contains(${quote(a)})`),
    ];
    lines.push(`    // Illustrative substring check. A real predicate should match`);
    lines.push(`    // whole class tokens and exact attribute names/values rather`);
    lines.push(`    // than raw \`raw_attributes\` substrings.`);
    lines.push(`    .html_preserve_when(|info| ${checks.join(" || ")})`);
  }

  lines.push(`    .build()?;`);

  // Conversion call.
  const raw = rustRawString(cfg.input);
  if (cfg.format === "html") {
    lines.push(`let output = converter.convert_html_fragment_to_string(${raw})?;`);
  } else if (cfg.format === "markdown") {
    symbols.add("__markdown");
    lines.push(
      `let output = converter.convert_markdown_to_string(${raw}, MarkdownVariant::CommonMark)?;`,
    );
  } else {
    lines.push(`let output = converter.convert_text_to_string(${raw})?;`);
  }
  lines.push(`println!("{output}");`);

  // Imports: one `use gukhanmun::…;` line plus the markdown variant.  A single
  // symbol omits the braces.
  const wantsMarkdown = symbols.delete("__markdown");
  const useList = [...symbols].sort();
  const useInner = useList.length === 1 ? useList[0] : `{${useList.join(", ")}}`;
  const imports = [`use gukhanmun::${useInner};`];
  if (wantsMarkdown) imports.push(`use gukhanmun::markdown::MarkdownVariant;`);

  return `${imports.join("\n")}\n\n${lines.join("\n")}`;
}

// ── JavaScript ─────────────────────────────────────────────────────────────

export function generateJs(cfg: CodegenConfig): string {
  const html = isHtml(cfg);
  const dicts = enabledDicts(cfg);

  // Imports: load plus each dictionary loader, grouped by package.
  const imports = [`import { load } from "@gukhanmun/wasm";`];
  const byPackage = new Map<string, string[]>();
  for (const id of dicts) {
    const { jsPackage, jsLoader } = DICT_META[id];
    const list = byPackage.get(jsPackage) ?? [];
    list.push(jsLoader);
    byPackage.set(jsPackage, list);
  }
  for (const [pkg, loaders] of byPackage) {
    imports.push(`import { ${loaders.join(", ")} } from "${pkg}";`);
  }

  // Options object fields.
  const fields: string[] = [];
  if (cfg.activePreset) {
    fields.push(`preset: ${quote(cfg.activePreset)},`);
  } else {
    if (cfg.rendering !== DEFAULTS.rendering) fields.push(`rendering: ${quote(cfg.rendering)},`);
    if (cfg.rendering === "original" && cfg.originalGloss !== DEFAULTS.originalGloss) {
      fields.push(`originalGloss: ${quote(cfg.originalGloss)},`);
    }
    if (cfg.segmentation !== DEFAULTS.segmentation) {
      fields.push(`segmentation: ${quote(cfg.segmentation)},`);
    }
    if (cfg.numerals !== DEFAULTS.numerals) fields.push(`numerals: ${quote(cfg.numerals)},`);
    if (cfg.initialSoundLaw !== DEFAULTS.initialSoundLaw) {
      fields.push(`initialSoundLaw: ${cfg.initialSoundLaw},`);
    }
    if (cfg.homophoneWindow !== DEFAULTS.homophoneWindow) {
      fields.push(`homophoneWindow: ${quote(cfg.homophoneWindow)},`);
    }
    if (cfg.homophoneDetection !== DEFAULTS.homophoneDetection) {
      fields.push(`homophoneDetection: ${quote(cfg.homophoneDetection)},`);
    }
    if (cfg.firstOccurrenceWindow !== DEFAULTS.firstOccurrenceWindow) {
      fields.push(`firstOccurrenceWindow: ${quote(cfg.firstOccurrenceWindow)},`);
    }
    if (html && cfg.recovery !== DEFAULTS.recovery) {
      fields.push(`recovery: ${quote(cfg.recovery)},`);
    }
  }

  // Dictionaries (JavaScript never bundles, so even stdict is listed).
  if (dicts.length) {
    const loaders = dicts.map((id) => `await ${DICT_META[id].jsLoader}()`);
    fields.push(`dictionaries: [${loaders.join(", ")}],`);
  }

  // Directives.
  const directiveFields: string[] = [];
  if (cfg.requireHanja.length) {
    directiveFields.push(`requireHanja: [${cfg.requireHanja.map(quote).join(", ")}]`);
  }
  if (cfg.requireHangul.length) {
    directiveFields.push(`requireHangul: [${cfg.requireHangul.map(quote).join(", ")}]`);
  }
  if (cfg.skipAnnotation.length) {
    directiveFields.push(`skipAnnotation: [${cfg.skipAnnotation.map(quote).join(", ")}]`);
  }
  if (directiveFields.length) {
    fields.push(`directives: { ${directiveFields.join(", ")} },`);
  }

  // HTML preservation.
  if (html && (cfg.preserveClasses.length || cfg.preserveAttributes.length)) {
    const htmlFields: string[] = [];
    if (cfg.preserveClasses.length) {
      htmlFields.push(`preserveClasses: [${cfg.preserveClasses.map(quote).join(", ")}]`);
    }
    if (cfg.preserveAttributes.length) {
      htmlFields.push(`preserveAttributes: [${cfg.preserveAttributes.map(quote).join(", ")}]`);
    }
    fields.push(`html: { ${htmlFields.join(", ")} },`);
  }

  const optionsObject = fields.length ? `{\n  ${fields.join("\n  ")}\n}` : `{}`;
  const tpl = jsTemplate(cfg.input);
  const convertArgs = cfg.format === "text" ? tpl : `${tpl}, ${quote(cfg.format)}`;

  return `${imports.join("\n")}\n\n` +
    `const g = await load(${optionsObject});\n` +
    `console.log(g.convert(${convertArgs}));`;
}

// ── CLI ────────────────────────────────────────────────────────────────────

export function generateCli(cfg: CodegenConfig): string {
  const html = isHtml(cfg);
  const args: string[] = [];
  let dictComment: string | null = null;

  if (cfg.format === "html") args.push(`--format text/html`);
  else if (cfg.format === "markdown") args.push(`--format text/markdown`);

  if (cfg.activePreset === "ko-kp") {
    args.push(`--preset ko-kp`);
  }
  if (cfg.activePreset === null) {
    if (cfg.rendering !== DEFAULTS.rendering) args.push(`--rendering ${cfg.rendering}`);
    if (cfg.rendering === "original" && cfg.originalGloss !== DEFAULTS.originalGloss) {
      args.push(`--original-gloss ${cfg.originalGloss}`);
    }
    if (cfg.segmentation !== DEFAULTS.segmentation) args.push(`--segmentation ${cfg.segmentation}`);
    if (cfg.initialSoundLaw !== DEFAULTS.initialSoundLaw) args.push(`--no-initial-sound-law`);
    if (cfg.homophoneWindow !== DEFAULTS.homophoneWindow) {
      args.push(`--disambiguation ${cfg.homophoneWindow}`);
    }
    if (cfg.homophoneDetection !== DEFAULTS.homophoneDetection) {
      args.push(`--homophone-detection ${cfg.homophoneDetection}`);
    }
    if (cfg.firstOccurrenceWindow !== DEFAULTS.firstOccurrenceWindow) {
      args.push(`--first-occurrence ${cfg.firstOccurrenceWindow}`);
    }
    if (html && cfg.recovery !== DEFAULTS.recovery) args.push(`--recovery ${cfg.recovery}`);

    // Dictionaries: the CLI bundles only via presets, so anything other than
    // the default {stdict} uses --no-bundled-dictionaries + --dictionary files.
    // The CLI gives *later* --dictionary flags higher priority (the opposite of
    // the Rust/JS APIs), so emit DICT_ORDER reversed to preserve the same
    // priority (highest-priority dictionary last).
    const dicts = enabledDicts(cfg);
    const onlyStdict = dicts.length === 1 && dicts[0] === "stdict";
    if (!onlyStdict) {
      args.push(`--no-bundled-dictionaries`);
      for (const id of [...dicts].reverse()) {
        args.push(`--dictionary ${DICT_META[id].cliFile}`);
      }
      if (dicts.length) {
        dictComment = `# dictionary FST files ship in the ` +
          `@gukhanmun/stdict-fst and @gukhanmun/opendict-fst packages`;
      }
    }
  }
  if (cfg.numerals !== CLI_DEFAULTS.numerals) args.push(`--numerals ${cfg.numerals}`);

  for (const h of cfg.requireHanja) args.push(`--require-hanja ${shellQuote(h)}`);
  for (const h of cfg.requireHangul) args.push(`--require-hangul ${shellQuote(h)}`);
  for (const h of cfg.skipAnnotation) args.push(`--skip-annotation ${shellQuote(h)}`);
  if (html) {
    for (const c of cfg.preserveClasses) args.push(`--html-preserve-class ${shellQuote(c)}`);
    for (const a of cfg.preserveAttributes) args.push(`--html-preserve-attr ${shellQuote(a)}`);
  }

  // Pipe the exact input bytes via `printf '%s'`.  Unlike a heredoc or here
  // string (both of which append a trailing newline), this feeds precisely the
  // textarea contents, so the CLI converts the same text as the Rust/JS
  // snippets and the live preview.
  const pipe = `printf '%s' ${shellQuote(cfg.input)} | `;
  const cmd = args.length
    ? `gukhanmun \\\n${args.map((a) => `  ${a}`).join(" \\\n")}`
    : `gukhanmun`;
  const comment = dictComment ? `${dictComment}\n` : "";
  return `${comment}${pipe}${cmd}`;
}

// Generates all three example snippets for the given configuration.
export function generateExamples(
  cfg: CodegenConfig,
): { cli: string; rust: string; js: string } {
  return {
    cli: generateCli(cfg),
    rust: generateRust(cfg),
    js: generateJs(cfg),
  };
}

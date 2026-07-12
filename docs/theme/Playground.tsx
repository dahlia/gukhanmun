// Gukhanmun: Interactive playground component for the documentation site.
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

import "./Playground.css";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { useEffect, useId, useRef, useState } from "react";
import { useI18n } from "@rspress/core/runtime";
import type {
  ContextWindow,
  Format,
  Gukhanmun,
  GukhanmunOptions,
  HanjaVariantSet,
  HomophoneDetection,
  NumeralStrategy,
  OriginalGloss,
  Preset,
  Recovery,
  RenderMode,
  Segmentation,
} from "@gukhanmun/wasm";
import type { Highlighter } from "shiki";
import { DICT_ORDER, type DictId, generateExamples } from "./codegen";

type CodeTab = "cli" | "rust" | "js";

type FormatKey = "text" | "markdown" | "html";
type Status = "loading" | "ready" | "error";

// Shiki language id for each example tab.
const CODE_LANG: Record<CodeTab, string> = {
  cli: "bash",
  rust: "rust",
  js: "javascript",
};

// Lazily-created Shiki highlighter, shared across renders and component
// instances.  Loaded on first use so the sizeable grammar/theme data stays out
// of the initial bundle; only the three languages and two themes we render are
// requested.
let highlighterPromise: Promise<Highlighter> | null = null;
function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = import("shiki").then((shiki) =>
      shiki.createHighlighter({
        themes: ["github-light", "github-dark"],
        langs: ["bash", "rust", "javascript"],
      })
    );
  }
  return highlighterPromise;
}

const DEFAULT_TEXT = "悠久한 歷史와 傳統에 빛나는 우리 大韓國民은 3·1運動으로 建立된 " +
  "大韓民國臨時政府의 法統과 不義에 抗拒한 4·19民主理念을 계승하고, " +
  "祖國의 民主改革과 平和的 統一의 使命에 입각하여 正義·人道와 同胞愛로써 " +
  "民族의 團結을 공고히 하고, 모든 社會的 弊習과 不義를 타파하며, " +
  "自律과 調和를 바탕으로 自由民主的 基本秩序를 더욱 확고히 하여 " +
  "政治·經濟·社會·文化의 모든 領域에 있어서 各人의 機會를 균등히 하고, " +
  "能力을 最高度로 발휘하게 하며, 自由와 權利에 따르는 責任과 義務를 완수하게 하여, " +
  "안으로는 國民生活의 균등한 향상을 기하고 밖으로는 항구적인 世界平和와 " +
  "人類共榮에 이바지함으로써 우리들과 우리들의 子孫의 安全과 自由와 幸福을 " +
  "영원히 확보할 것을 다짐하면서 1948年 7月 12日에 制定되고 8次에 걸쳐 改正된 " +
  "憲法을 이제 國會의 議決을 거쳐 國民投票에 의하여 改正한다.";

const DEFAULT_MARKDOWN = `[大韓民國憲法]\n` +
  `==============\n` +
  `\n` +
  `施行 1988年 2月 25日. 憲法 第10號, 1987年 10月 29日, 全部改正.\n` +
  `\n` +
  `\n` +
  `前文\n` +
  `----\n` +
  `\n` +
  `悠久한 歷史와 傳統에 빛나는 우리 大韓國民은 3·1運動으로 建立된\n` +
  `大韓民國臨時政府의 法統과 不義에 抗拒한 4·19民主理念을 계승하고,\n` +
  `祖國의 民主改革과 平和的 統一의 使命에 입각하여 正義·人道와 同胞愛로써\n` +
  `民族의 團結을 공고히 하고, 모든 社會的 弊習과 不義를 타파하며,\n` +
  `自律과 調和를 바탕으로 自由民主的 基本秩序를 더욱 확고히 하여\n` +
  `政治·經濟·社會·文化의 모든 領域에 있어서 各人의 機會를 균등히 하고,\n` +
  `能力을 最高度로 발휘하게 하며, 自由와 權利에 따르는 責任과 義務를 완수하게 하여,\n` +
  `안으로는 國民生活의 균등한 향상을 기하고 밖으로는 항구적인 世界平和와\n` +
  `人類共榮에 이바지함으로써 우리들과 우리들의 子孫의 安全과 自由와 幸福을\n` +
  `영원히 확보할 것을 다짐하면서 1948年 7月 12日에 制定되고 8次에 걸쳐 改正된\n` +
  `憲法을 이제 國會의 議決을 거쳐 國民投票에 의하여 改正한다.\n` +
  `\n` +
  `[大韓民國憲法]: https://www.law.go.kr/lsInfoP.do?lsiSeq=61603&chrClsCd=010201`;

const DEFAULT_HTML = `<article>\n` +
  `<h1><a href="https://www.law.go.kr/lsInfoP.do?lsiSeq=61603&amp;chrClsCd=010201">大韓民國憲法</a></h1>\n` +
  `<p>施行 1988年 2月 25日. 憲法 第10號, 1987年 10月 29日, 全部改正.</p>\n` +
  `<h2>前文</h2>\n` +
  `<p>悠久한 歷史와 傳統에 빛나는 우리 大韓國民은 3·1運動으로 建立된\n` +
  `大韓民國臨時政府의 法統과 不義에 抗拒한 4·19民主理念을 계승하고,\n` +
  `祖國의 民主改革과 平和的 統一의 使命에 입각하여 正義·人道와 同胞愛로써\n` +
  `民族의 團結을 공고히 하고, 모든 社會的 弊習과 不義를 타파하며,\n` +
  `自律과 調和를 바탕으로 自由民主的 基本秩序를 더욱 확고히 하여\n` +
  `政治·經濟·社會·文化의 모든 領域에 있어서 各人의\n` +
  `機會를 균등히 하고,\n` +
  `能力을 最高度로 발휘하게 하며, 自由와 權利에 따르는 責任과 義務를 완수하게 하여,\n` +
  `안으로는 國民生活의 균등한 향상을 기하고 밖으로는 항구적인 世界平和와\n` +
  `人類共榮에 이바지함으로써 우리들과 우리들의 子孫의 安全과 自由와 幸福을\n` +
  `영원히 확보할 것을 다짐하면서 1948年 7月 12日에 制定되고 8次에 걸쳐 改正된\n` +
  `憲法을 이제 國會의 議決을 거쳐 國民投票에 의하여 改正한다.</p>\n` +
  `</article>`;

const DEFAULTS: Record<FormatKey, string> = {
  text: DEFAULT_TEXT,
  markdown: DEFAULT_MARKDOWN,
  html: DEFAULT_HTML,
};

// `DictId` and `DICT_ORDER` are defined in ./codegen and shared with the
// example generators so the runtime dictionary set and the generated code stay
// in lockstep.

// Lazy byte loaders, one per dictionary.  Each dynamically imports the matching
// FST package so a category's binary is only downloaded once the user enables
// it; `ensureDictBytes` memoizes the result per id.
const DICT_LOADERS: Record<DictId, () => Promise<Uint8Array>> = {
  "stdict": async () => (await import("@gukhanmun/stdict-fst")).stdictFstBytes(),
  "opendict-general": async () =>
    (await import("@gukhanmun/opendict-fst")).opendictGeneralFstBytes(),
  "opendict-north-korean": async () =>
    (await import("@gukhanmun/opendict-fst")).opendictNorthKoreanFstBytes(),
  "opendict-dialect": async () =>
    (await import("@gukhanmun/opendict-fst")).opendictDialectFstBytes(),
  "opendict-archaic": async () =>
    (await import("@gukhanmun/opendict-fst")).opendictArchaicFstBytes(),
};

// The complete setting set each preset stands for.  A preset is not stored as
// state; it is a *derived indicator* (see `matchesPreset`/`activePreset`): the
// button lights up exactly when the live configuration equals one of these
// sets, however the user reached it, and clears the moment any one governed
// setting diverges.  Clicking a preset writes every field below at once.
//
// Unlike the JS/TS bindings (where a preset never auto-selects a bundled
// dictionary), the playground applies each preset's bundled set exclusively:
// ko-kr enables only stdict, ko-kp only the 북한어 category.
//
// originalGloss is intentionally not part of a preset: it only applies under
// the "original" rendering mode, which neither preset uses.  Input format and
// the free-text directives are content, not preset settings, so they are
// excluded too.
type PresetConfig = {
  rendering: RenderMode;
  segmentation: Segmentation;
  numerals: NumeralStrategy;
  initialSoundLaw: boolean;
  homophoneWindow: ContextWindow;
  homophoneDetection: HomophoneDetection;
  firstOccurrenceWindow: ContextWindow;
  recovery: Recovery;
  dicts: Record<DictId, boolean>;
};

const PRESET_CONFIGS: Record<Preset, PresetConfig> = {
  "ko-kr": {
    rendering: "hangul-only",
    segmentation: "lattice",
    numerals: "hangul-phonetic",
    initialSoundLaw: true,
    homophoneWindow: "per-block",
    homophoneDetection: "context-local",
    firstOccurrenceWindow: "off",
    recovery: "strict",
    dicts: {
      "stdict": true,
      "opendict-general": false,
      "opendict-north-korean": false,
      "opendict-dialect": false,
      "opendict-archaic": false,
    },
  },
  "ko-kp": {
    rendering: "hangul-only",
    segmentation: "lattice",
    numerals: "hangul-phonetic",
    initialSoundLaw: false,
    homophoneWindow: "off",
    homophoneDetection: "context-local",
    firstOccurrenceWindow: "off",
    recovery: "strict",
    dicts: {
      "stdict": false,
      "opendict-general": false,
      "opendict-north-korean": true,
      "opendict-dialect": false,
      "opendict-archaic": false,
    },
  },
};

// Split a whitespace/comma-separated list of hanja forms into an array.
function parseList(value: string): string[] {
  return value.split(/[\s,]+/).map((s) => s.trim()).filter(Boolean);
}

// The textarea is user-controlled and the converter preserves inline markup, so
// sanitize before injecting.  Ruby tags are allowed for the ruby rendering modes.
function sanitize(html: string): string {
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
    ADD_TAGS: ["ruby", "rp", "rt", "rb"],
  });
}

// HTML to inject into the preview for the markup formats.  Markdown output is
// rendered to HTML first so the preview shows the formatted result, matching
// how the HTML format is previewed.
function renderPreview(output: string, format: FormatKey): string {
  const html = format === "markdown" ? (marked.parse(output, { async: false }) as string) : output;
  return sanitize(html);
}

function Field<T extends string>(
  { label, value, options, onChange, disabled }: {
    label: string;
    value: T;
    options: [T, string][];
    onChange: (value: T) => void;
    disabled?: boolean;
  },
) {
  return (
    <label className="playground-control">
      <span className="playground-control-label">{label}</span>
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as T)}
      >
        {options.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
      </select>
    </label>
  );
}

function Toggle(
  { label, checked, onChange }: {
    label: string;
    checked: boolean;
    onChange: (checked: boolean) => void;
  },
) {
  return (
    <label className="playground-control playground-control--checkbox">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{label}</span>
    </label>
  );
}

function TextField(
  { label, value, placeholder, onChange }: {
    label: string;
    value: string;
    placeholder?: string;
    onChange: (value: string) => void;
  },
) {
  return (
    <label className="playground-control playground-control--text">
      <span className="playground-control-label">{label}</span>
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        spellCheck={false}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );
}

export function Playground() {
  const t = useI18n<typeof import("i18n")>();
  const opendictGroupId = useId();

  const [status, setStatus] = useState<Status>("loading");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const [format, setFormat] = useState<FormatKey>("text");
  const [input, setInput] = useState(DEFAULTS.text);
  const [output, setOutput] = useState("");

  // Engine options (baked into the Gukhanmun instance at load() time).
  const [enabledDicts, setEnabledDicts] = useState<Record<DictId, boolean>>({
    "stdict": true,
    "opendict-general": false,
    "opendict-north-korean": false,
    "opendict-dialect": false,
    "opendict-archaic": false,
  });
  const [rendering, setRendering] = useState<RenderMode>("hangul-only");
  const [originalGloss, setOriginalGloss] = useState<OriginalGloss>("parens");
  const [hanjaVariantSet, setHanjaVariantSet] = useState<HanjaVariantSet>(
    "as-dictionary",
  );
  const [segmentation, setSegmentation] = useState<Segmentation>("lattice");
  const [numerals, setNumerals] = useState<NumeralStrategy>("hangul-phonetic");
  const [initialSoundLaw, setInitialSoundLaw] = useState(true);
  const [homophoneWindow, setHomophoneWindow] = useState<ContextWindow>(
    "per-block",
  );
  const [homophoneDetection, setHomophoneDetection] = useState<
    HomophoneDetection
  >(
    "context-local",
  );
  const [firstOccurrenceWindow, setFirstOccurrenceWindow] = useState<
    ContextWindow
  >("off");
  const [recovery, setRecovery] = useState<Recovery>("strict");
  const [requireHanja, setRequireHanja] = useState("");
  const [requireHangul, setRequireHangul] = useState("");
  const [skipAnnotation, setSkipAnnotation] = useState("");
  const [preserveClasses, setPreserveClasses] = useState("");
  const [preserveAttributes, setPreserveAttributes] = useState("");

  // Which generated-example tab is shown below the editor.
  const [codeTab, setCodeTab] = useState<CodeTab>("cli");
  const [copied, setCopied] = useState(false);
  // Shiki-highlighted HTML for the active tab; null until the first highlight
  // finishes (or when highlighting fails), in which case a plain block shows.
  const [highlightedCode, setHighlightedCode] = useState<string | null>(null);

  // Option labels are localized via the i18n table, so they follow the active
  // documentation language.  The option *values* are the engine's stable keys.
  const RENDERING_OPTIONS: [RenderMode, string][] = [
    ["hangul-only", t("pgRenderHangulOnly")],
    ["hangul-hanja-parens", t("pgRenderHangulHanja")],
    ["hanja-hangul-parens", t("pgRenderHanjaHangul")],
    ["ruby-on-hangul", t("pgRenderRubyHangul")],
    ["ruby-on-hanja", t("pgRenderRubyHanja")],
    ["original", t("pgRenderOriginal")],
  ];
  const PRESET_OPTIONS: [Preset, string][] = [
    ["ko-kr", t("pgPresetKoKr")],
    ["ko-kp", t("pgPresetKoKp")],
  ];
  const ORIGINAL_GLOSS_OPTIONS: [OriginalGloss, string][] = [
    ["parens", t("pgGlossParens")],
    ["ruby", t("pgGlossRuby")],
  ];
  const HANJA_VARIANT_SET_OPTIONS: [HanjaVariantSet, string][] = [
    ["as-dictionary", t("pgVariantSetAsDictionary")],
    ["shinjitai", t("pgVariantSetShinjitai")],
    ["kanxi", t("pgVariantSetKanxi")],
    ["simplified", t("pgVariantSetSimplified")],
    ["asahimoji", t("pgVariantSetAsahimoji")],
  ];
  const SEGMENTATION_OPTIONS: [Segmentation, string][] = [
    ["lattice", t("pgSegLattice")],
    ["eager", t("pgSegEager")],
  ];
  const NUMERAL_OPTIONS: [NumeralStrategy, string][] = [
    ["hangul-phonetic", t("pgNumHangulPhonetic")],
    ["positional-arabic", t("pgNumPositional")],
    ["additive-arabic", t("pgNumAdditive")],
    ["smart", t("pgNumSmart")],
  ];
  const WINDOW_OPTIONS: [ContextWindow, string][] = [
    ["off", t("pgWindowOff")],
    ["per-block", t("pgWindowPerBlock")],
    ["per-section", t("pgWindowPerSection")],
    ["per-document", t("pgWindowPerDocument")],
  ];
  const DETECTION_OPTIONS: [HomophoneDetection, string][] = [
    ["context-local", t("pgDetectContextLocal")],
    ["dictionary-wide", t("pgDetectDictionaryWide")],
  ];
  const RECOVERY_OPTIONS: [Recovery, string][] = [
    ["strict", t("pgRecoveryStrict")],
    ["lenient", t("pgRecoveryLenient")],
  ];

  const gRef = useRef<Gukhanmun | null>(null);
  // Per-dictionary byte cache; populated lazily the first time a dictionary is
  // enabled (or eagerly for the default-on stdict during the mount effect).
  const dictBytesRef = useRef<Map<DictId, Uint8Array>>(new Map());

  async function ensureDictBytes(id: DictId): Promise<Uint8Array> {
    const cached = dictBytesRef.current.get(id);
    if (cached) return cached;
    const bytes = await DICT_LOADERS[id]();
    dictBytesRef.current.set(id, bytes);
    return bytes;
  }

  // Refs so async callbacks always see the latest value without extra deps.
  const inputRef = useRef(input);
  const formatRef = useRef<FormatKey>(format);
  useEffect(() => {
    inputRef.current = input;
  }, [input]);
  useEffect(() => {
    formatRef.current = format;
  }, [format]);

  function buildOptions(
    dictionaries: { data: Uint8Array; format: "fst" }[],
  ): GukhanmunOptions {
    const rh = parseList(requireHanja);
    const rg = parseList(requireHangul);
    const sa = parseList(skipAnnotation);
    const pc = parseList(preserveClasses);
    const pa = parseList(preserveAttributes);
    return {
      // No `preset`: every preset-governed field is set explicitly below, so
      // the preset is purely a UI indicator (the fields a preset would seed are
      // all overridden here anyway).
      rendering,
      originalGloss,
      hanjaVariantSet,
      segmentation,
      numerals,
      initialSoundLaw,
      homophoneWindow,
      homophoneDetection,
      firstOccurrenceWindow,
      recovery,
      dictionaries,
      ...(rh.length || rg.length || sa.length
        ? {
          directives: {
            ...(rh.length ? { requireHanja: rh } : {}),
            ...(rg.length ? { requireHangul: rg } : {}),
            ...(sa.length ? { skipAnnotation: sa } : {}),
          },
        }
        : {}),
      ...(pc.length || pa.length
        ? {
          html: {
            ...(pc.length ? { preserveClasses: pc } : {}),
            ...(pa.length ? { preserveAttributes: pa } : {}),
          },
        }
        : {}),
    };
  }

  // Load the WASM module and dictionary bytes once on mount.
  useEffect(() => {
    let cancelled = false;

    void (async () => {
      try {
        // Prime the cached WASM module while the default-on stdict bytes
        // download, so first paint has its dictionary ready.  Other dictionary
        // categories load lazily when the user enables them.
        const [{ load }] = await Promise.all([
          import("@gukhanmun/wasm"),
          ensureDictBytes("stdict"),
        ]);
        if (cancelled) return;

        await load({});
        if (cancelled) return;

        setStatus("ready");
      } catch (e) {
        if (!cancelled) {
          setStatus("error");
          setErrorMsg(e instanceof Error ? e.message : String(e));
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  // Rebuild the converter whenever an instance-level option changes.
  useEffect(() => {
    if (status !== "ready") return;
    let cancelled = false;

    void (async () => {
      try {
        // Resolve the bytes for every enabled dictionary (lazy-loading on first
        // use) in priority order before building the converter.
        const enabled = DICT_ORDER.filter((id) => enabledDicts[id]);
        const dictionaries = await Promise.all(
          enabled.map(async (id) => ({
            data: await ensureDictBytes(id),
            format: "fst" as const,
          })),
        );
        if (cancelled) return;

        const { load } = await import("@gukhanmun/wasm");
        const g = await load(buildOptions(dictionaries));
        if (cancelled) return;
        gRef.current = g;
        setOutput(g.convert(inputRef.current, formatRef.current as Format));
        setErrorMsg(null);
      } catch (e) {
        if (!cancelled) setErrorMsg(e instanceof Error ? e.message : String(e));
      }
    })();

    return () => {
      cancelled = true;
    };
    // buildOptions reads all of these; rebuild when any changes.
  }, [
    status,
    enabledDicts,
    rendering,
    originalGloss,
    hanjaVariantSet,
    segmentation,
    numerals,
    initialSoundLaw,
    homophoneWindow,
    homophoneDetection,
    firstOccurrenceWindow,
    recovery,
    requireHanja,
    requireHangul,
    skipAnnotation,
    preserveClasses,
    preserveAttributes,
  ]);

  // Debounced re-conversion when only the input text or format changes.
  useEffect(() => {
    if (status !== "ready" || !gRef.current) return;

    const timer = setTimeout(() => {
      if (!gRef.current) return;
      try {
        setOutput(gRef.current.convert(input, format as Format));
        setErrorMsg(null);
      } catch (e) {
        setErrorMsg(e instanceof Error ? e.message : String(e));
      }
    }, 200);

    return () => clearTimeout(timer);
  }, [input, format, status]);

  function handleFormatChange(newFormat: FormatKey) {
    setFormat(newFormat);
    setInput(DEFAULTS[newFormat]);
  }

  // True when the live configuration matches a preset's complete setting set.
  function matchesPreset(cfg: PresetConfig): boolean {
    return rendering === cfg.rendering &&
      segmentation === cfg.segmentation &&
      numerals === cfg.numerals &&
      initialSoundLaw === cfg.initialSoundLaw &&
      homophoneWindow === cfg.homophoneWindow &&
      homophoneDetection === cfg.homophoneDetection &&
      firstOccurrenceWindow === cfg.firstOccurrenceWindow &&
      recovery === cfg.recovery &&
      DICT_ORDER.every((id) => enabledDicts[id] === cfg.dicts[id]);
  }

  // The preset currently in effect, or null when the settings match none.  This
  // is derived from the live settings every render, so the highlight tracks
  // both preset clicks and manual edits.
  const activePreset: Preset | null =
    (Object.keys(PRESET_CONFIGS) as Preset[]).find((p) => matchesPreset(PRESET_CONFIGS[p])) ?? null;

  // Apply every field of a preset's setting set at once.
  function applyPreset(p: Preset) {
    const cfg = PRESET_CONFIGS[p];
    setRendering(cfg.rendering);
    setSegmentation(cfg.segmentation);
    setNumerals(cfg.numerals);
    setInitialSoundLaw(cfg.initialSoundLaw);
    setHomophoneWindow(cfg.homophoneWindow);
    setHomophoneDetection(cfg.homophoneDetection);
    setFirstOccurrenceWindow(cfg.firstOccurrenceWindow);
    setRecovery(cfg.recovery);
    setEnabledDicts({ ...cfg.dicts });
  }

  function toggleDict(id: DictId, on: boolean) {
    setEnabledDicts((prev) => ({ ...prev, [id]: on }));
  }

  // Example code for the current configuration, regenerated every render so the
  // snippets track the live options, dictionaries, directives, and input.
  const examples = generateExamples({
    activePreset,
    format,
    input,
    rendering,
    originalGloss,
    hanjaVariantSet,
    segmentation,
    numerals,
    initialSoundLaw,
    homophoneWindow,
    homophoneDetection,
    firstOccurrenceWindow,
    recovery,
    dicts: enabledDicts,
    requireHanja: parseList(requireHanja),
    requireHangul: parseList(requireHangul),
    skipAnnotation: parseList(skipAnnotation),
    preserveClasses: parseList(preserveClasses),
    preserveAttributes: parseList(preserveAttributes),
  });

  function copyExample() {
    void navigator.clipboard?.writeText(examples[codeTab]).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }

  // Syntax-highlight the active snippet with Shiki.  The previous highlight is
  // kept on screen until the new one is ready (no flash to plain text), and a
  // short debounce avoids re-highlighting on every keystroke.
  const code = examples[codeTab];
  useEffect(() => {
    if (status !== "ready") return;
    let cancelled = false;
    const timer = setTimeout(() => {
      void getHighlighter()
        .then((hl) => {
          if (cancelled) return;
          setHighlightedCode(
            hl.codeToHtml(code, {
              lang: CODE_LANG[codeTab],
              themes: { light: "github-light", dark: "github-dark" },
            }),
          );
        })
        .catch(() => {
          if (!cancelled) setHighlightedCode(null);
        });
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [code, codeTab, status]);

  const CODE_TABS: [CodeTab, string][] = [
    ["cli", "CLI"],
    ["rust", "Rust"],
    ["js", "JavaScript"],
  ];

  return (
    <div className="playground">
      {status === "loading" && (
        <div className="playground-loading">
          <div className="playground-spinner" aria-hidden="true" />
          <p>{t("pgLoading")}</p>
        </div>
      )}

      {status === "error" && (
        <div className="playground-error" role="alert">
          <strong>{t("pgErrorTitle")}</strong>
          {errorMsg && <pre className="playground-error-msg">{errorMsg}</pre>}
        </div>
      )}

      {status === "ready" && (
        <>
          <div className="playground-presets">
            <span className="playground-presets-label">{t("pgPreset")}</span>
            {PRESET_OPTIONS.map(([value, label]) => (
              <button
                key={value}
                type="button"
                className={"playground-preset" +
                  (activePreset === value ? " playground-preset--active" : "")}
                aria-pressed={activePreset === value}
                onClick={() => applyPreset(value)}
              >
                {label}
              </button>
            ))}
          </div>

          <div className="playground-options">
            <div className="playground-options-group">
              <span className="playground-options-group-label">
                {t("pgGroupInputRendering")}
              </span>
              <div className="playground-options-group-controls">
                <Field
                  label={t("pgInputFormat")}
                  value={format}
                  options={[["text", t("pgFormatText")], [
                    "markdown",
                    t("pgFormatMarkdown"),
                  ], [
                    "html",
                    t("pgFormatHtml"),
                  ]]}
                  onChange={handleFormatChange}
                />
                <Field
                  label={t("pgRendering")}
                  value={rendering}
                  options={RENDERING_OPTIONS}
                  onChange={setRendering}
                />
                {rendering === "original" && (
                  <Field
                    label={t("pgOriginalGloss")}
                    value={originalGloss}
                    options={ORIGINAL_GLOSS_OPTIONS}
                    onChange={setOriginalGloss}
                  />
                )}
                <Field
                  label={t("pgHanjaVariantSet")}
                  value={hanjaVariantSet}
                  options={HANJA_VARIANT_SET_OPTIONS}
                  onChange={setHanjaVariantSet}
                />
              </div>
            </div>

            <div className="playground-options-group">
              <span className="playground-options-group-label">
                {t("pgGroupProcessing")}
              </span>
              <div className="playground-options-group-controls">
                <Field
                  label={t("pgSegmentation")}
                  value={segmentation}
                  options={SEGMENTATION_OPTIONS}
                  onChange={setSegmentation}
                />
                <Field
                  label={t("pgNumerals")}
                  value={numerals}
                  options={NUMERAL_OPTIONS}
                  onChange={setNumerals}
                />
                <Toggle
                  label={t("pgInitialSoundLaw")}
                  checked={initialSoundLaw}
                  onChange={setInitialSoundLaw}
                />
                {format === "html" && (
                  <Field
                    label={t("pgHtmlRecovery")}
                    value={recovery}
                    options={RECOVERY_OPTIONS}
                    onChange={setRecovery}
                  />
                )}
              </div>
            </div>

            <div className="playground-options-group">
              <span className="playground-options-group-label">
                {t("pgGroupHomophone")}
              </span>
              <div className="playground-options-group-controls">
                <Field
                  label={t("pgHomophoneWindow")}
                  value={homophoneWindow}
                  options={WINDOW_OPTIONS}
                  onChange={setHomophoneWindow}
                />
                <Field
                  label={t("pgHomophoneDetection")}
                  value={homophoneDetection}
                  options={DETECTION_OPTIONS}
                  onChange={setHomophoneDetection}
                />
                <Field
                  label={t("pgFirstOccurrenceWindow")}
                  value={firstOccurrenceWindow}
                  options={WINDOW_OPTIONS}
                  onChange={setFirstOccurrenceWindow}
                />
              </div>
            </div>
          </div>

          <div className="playground-dicts">
            <span className="playground-dicts-label">
              {t("pgDictionaries")}
            </span>
            <div className="playground-dicts-body">
              <Toggle
                label={t("pgStdict")}
                checked={enabledDicts["stdict"]}
                onChange={(v) => toggleDict("stdict", v)}
              />
              <div
                className="playground-dicts-opendict"
                role="group"
                aria-labelledby={opendictGroupId}
              >
                <span
                  id={opendictGroupId}
                  className="playground-dicts-opendict-label"
                >
                  {t("pgOpendictGroup")}
                </span>
                <div className="playground-dicts-opendict-toggles">
                  <Toggle
                    label={t("pgOpendictCatGeneral")}
                    checked={enabledDicts["opendict-general"]}
                    onChange={(v) => toggleDict("opendict-general", v)}
                  />
                  <Toggle
                    label={t("pgOpendictCatNorthKorean")}
                    checked={enabledDicts["opendict-north-korean"]}
                    onChange={(v) => toggleDict("opendict-north-korean", v)}
                  />
                  <Toggle
                    label={t("pgOpendictCatDialect")}
                    checked={enabledDicts["opendict-dialect"]}
                    onChange={(v) => toggleDict("opendict-dialect", v)}
                  />
                  <Toggle
                    label={t("pgOpendictCatArchaic")}
                    checked={enabledDicts["opendict-archaic"]}
                    onChange={(v) => toggleDict("opendict-archaic", v)}
                  />
                </div>
              </div>
            </div>
          </div>

          <details className="playground-advanced">
            <summary>
              {format === "html" ? t("pgDirectivesHtml") : t("pgDirectives")}
            </summary>
            <div className="playground-advanced-grid">
              <TextField
                label={t("pgRequireHanja")}
                value={requireHanja}
                placeholder={t("pgPhRequireHanja")}
                onChange={setRequireHanja}
              />
              <TextField
                label={t("pgRequireHangul")}
                value={requireHangul}
                placeholder={t("pgPhRequireHangul")}
                onChange={setRequireHangul}
              />
              <TextField
                label={t("pgSkipAnnotation")}
                value={skipAnnotation}
                placeholder={t("pgPhSkipAnnotation")}
                onChange={setSkipAnnotation}
              />
              {format === "html" && (
                <>
                  <TextField
                    label={t("pgPreserveClasses")}
                    value={preserveClasses}
                    placeholder={t("pgPhPreserveClasses")}
                    onChange={setPreserveClasses}
                  />
                  <TextField
                    label={t("pgPreserveAttributes")}
                    value={preserveAttributes}
                    placeholder={t("pgPhPreserveAttributes")}
                    onChange={setPreserveAttributes}
                  />
                </>
              )}
            </div>
          </details>

          <div className="playground-editor">
            <div className="playground-panel">
              <span className="playground-panel-label">{t("pgInput")}</span>
              <textarea
                className="playground-textarea"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                spellCheck={false}
                aria-label={t("pgInputAria")}
              />
            </div>

            <div className="playground-panel">
              <span className="playground-panel-label">{t("pgPreview")}</span>
              {format === "text"
                ? (
                  <pre className="playground-output playground-output--text">
                    {output}
                  </pre>
                )
                : (
                  <div
                    className="playground-output playground-output--html"
                    dangerouslySetInnerHTML={{
                      __html: renderPreview(output, format),
                    }}
                  />
                )}
              {errorMsg && (
                <p className="playground-convert-error" role="alert">
                  {errorMsg}
                </p>
              )}
            </div>
          </div>

          <div className="playground-examples">
            <div className="playground-examples-header">
              <span className="playground-panel-label">{t("pgExamples")}</span>
              <div className="playground-tabs" role="tablist">
                {CODE_TABS.map(([value, label]) => (
                  <button
                    key={value}
                    type="button"
                    role="tab"
                    aria-selected={codeTab === value}
                    className={"playground-tab" +
                      (codeTab === value ? " playground-tab--active" : "")}
                    onClick={() => setCodeTab(value)}
                  >
                    {label}
                  </button>
                ))}
              </div>
              <button
                type="button"
                className="playground-copy"
                onClick={copyExample}
              >
                {copied ? t("pgCopied") : t("pgCopy")}
              </button>
            </div>
            {highlightedCode
              ? (
                <div
                  className="playground-code"
                  // Shiki escapes the code text, so the only HTML here is its
                  // own generated token markup.
                  dangerouslySetInnerHTML={{ __html: highlightedCode }}
                />
              )
              : (
                <div className="playground-code">
                  <pre><code>{examples[codeTab]}</code></pre>
                </div>
              )}
          </div>
        </>
      )}
    </div>
  );
}

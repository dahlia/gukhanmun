// Gukhanmun: Private Python extension with a safely shared conversion runtime.
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

//! Private owning bridge; public Python types live in the Python facade.

mod html;

use std::panic::{AssertUnwindSafe, catch_unwind};

use gukhanmun::cdb::CdbDictionary;
use gukhanmun::fst::FstDictionary;
use gukhanmun::markdown::MarkdownVariant;
use gukhanmun::{
    ContextWindow, DirectiveAction, HanjaDictionary, HanjaVariantSet, HomophoneDetection,
    MapDictionary, MatchMark, NumeralStrategy, OriginalGloss, Preset, ReadingCandidate, Recovery,
    RenderMode, RenderOptions, RubyBase, SegmentationStrategy, SharedBuilder, SharedConverter,
    SourcePolicy, SourceRole,
};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

pyo3::create_exception!(
    _native,
    NativeError,
    pyo3::exceptions::PyException,
    "Private serialized conversion failure, translated by the Python facade."
);

type Result<T> = std::result::Result<T, Failure>;
type DictionaryInput = (
    Option<String>,
    Option<String>,
    Option<Vec<u8>>,
    Option<String>,
);

#[derive(Debug, Serialize)]
struct Cause {
    code: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct Failure {
    code: &'static str,
    message: String,
    chain: Vec<Cause>,
}

impl Failure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            chain: Vec::new(),
        }
    }

    fn into_python(self) -> PyErr {
        // This record contains only strings and cannot fail JSON serialization.
        NativeError::new_err(serde_json::to_string(&self).expect("serializable failure"))
    }
}

fn invalid(message: impl Into<String>) -> Failure {
    Failure::new("invalid-input", message)
}

fn map_error(error: gukhanmun::Error) -> Failure {
    use std::error::Error;
    let code = match &error {
        gukhanmun::Error::Core(_) => "segmentation",
        gukhanmun::Error::Html(_) => "html-scan",
        gukhanmun::Error::Markdown(_) => "markdown",
        gukhanmun::Error::Fst(_) | gukhanmun::Error::Cdb(_) => "dictionary-load",
        gukhanmun::Error::Io(_) => "io",
        gukhanmun::Error::Config(_) => "invalid-input",
        _ => "internal",
    };
    let mut chain = Vec::new();
    let mut cause = error.source();
    while let Some(current) = cause {
        chain.push(Cause {
            code: "internal",
            message: current.to_string(),
        });
        cause = current.source();
    }
    chain.reverse();
    Failure {
        code,
        message: error.to_string(),
        chain,
    }
}

// Both entry points use exactly this boundary: only owned/shared Rust data may
// cross detach. Signals take precedence over either success or a Rust failure.
fn detached<T: Send>(py: Python<'_>, work: impl FnOnce() -> Result<T> + Send) -> PyResult<T> {
    let result = py.detach(move || catch_unwind(AssertUnwindSafe(work)));
    py.check_signals()?;
    match result {
        Ok(value) => value.map_err(Failure::into_python),
        Err(_) => {
            Err(Failure::new("internal", "unexpected panic in native conversion").into_python())
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Options {
    punctuation: Option<gukhanmun::PunctuationConfig>,
    preset: Option<String>,
    rendering: Option<String>,
    original_gloss: Option<String>,
    hanja_variant_set: Option<String>,
    segmentation: Option<String>,
    source_policy: Option<String>,
    reference_only: Option<bool>,
    numerals: Option<String>,
    initial_sound_law: Option<bool>,
    homophone_window: Option<String>,
    homophone_detection: Option<String>,
    first_occurrence_window: Option<String>,
    collapse_redundant_parens: Option<bool>,
    recovery: Option<String>,
    directives: Option<Directives>,
    html: Option<html::Options>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Directives {
    #[serde(default)]
    require_hanja: Vec<String>,
    #[serde(default)]
    require_hangul: Vec<String>,
    #[serde(default)]
    skip_annotation: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    hanja: String,
    reading: String,
    #[serde(default)]
    suffix_reading: Option<String>,
    #[serde(default)]
    preference: u32,
    #[serde(default)]
    require_hanja: bool,
    #[serde(default)]
    require_hangul: bool,
}

/// Immutable private handle; all document state is allocated by each call.
#[pyclass(name = "Converter", module = "gukhanmun._native", frozen)]
struct NativeConverter {
    converter: SharedConverter,
    resolved: String,
}

#[pymethods]
impl NativeConverter {
    #[new]
    #[pyo3(signature = (options_json, dictionaries))]
    fn new(
        py: Python<'_>,
        options_json: String,
        dictionaries: Vec<DictionaryInput>,
    ) -> PyResult<Self> {
        detached(py, move || build(&options_json, dictionaries))
    }

    #[pyo3(signature = (source, format, gfm))]
    fn convert(
        &self,
        py: Python<'_>,
        source: String,
        format: String,
        gfm: bool,
    ) -> PyResult<String> {
        detached(py, move || {
            match format.as_str() {
                "text" => self.converter.convert_text_to_string(&source),
                "html" => self.converter.convert_html_fragment_to_string(&source),
                "markdown" => self.converter.convert_markdown_to_string(
                    &source,
                    if gfm {
                        MarkdownVariant::Gfm
                    } else {
                        MarkdownVariant::CommonMark
                    },
                ),
                _ => {
                    return Err(Failure::new(
                        "unsupported-content-type",
                        format!("unknown input format: {format}"),
                    ));
                }
            }
            .map_err(map_error)
        })
    }

    fn options_json(&self) -> &str {
        &self.resolved
    }
}

fn build(json: &str, dictionaries: Vec<DictionaryInput>) -> Result<NativeConverter> {
    let opts: Options = serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
    let preset_name = opts.preset.as_deref().unwrap_or("ko-kr");
    let mut builder =
        SharedBuilder::with_preset(parse_preset(preset_name)?).no_bundled_dictionaries();
    if let Some(config) = opts.punctuation {
        builder = builder.punctuation(config.resolve().map_err(|e| invalid(e.to_string()))?);
    }
    builder = builder.rendering(parse_render_mode(
        opts.rendering.as_deref().unwrap_or("hangul-only"),
        opts.original_gloss.as_deref(),
    )?);
    // Rendering initializes its variant field; the explicit override follows it.
    if let Some(value) = &opts.hanja_variant_set {
        builder = builder.hanja_variant_set(parse_hanja_variant_set(value)?);
    }
    if let Some(value) = &opts.source_policy {
        builder = builder.source_policy(parse_source_policy(value)?);
    }
    if let Some(value) = opts.reference_only {
        builder = builder.reference_only(value);
    }
    if let Some(value) = &opts.segmentation {
        builder = builder.segmentation(parse_segmentation(value)?);
    }
    if let Some(value) = &opts.numerals {
        builder = builder.numerals(parse_numerals(value)?);
    }
    if let Some(value) = opts.initial_sound_law {
        builder = builder.initial_sound_law(value);
    }
    if let Some(value) = &opts.homophone_window {
        builder = builder.homophone_window(parse_context_window(value)?);
    }
    if let Some(value) = &opts.homophone_detection {
        builder = builder.homophone_detection(parse_homophone_detection(value)?);
    }
    if let Some(value) = &opts.first_occurrence_window {
        builder = builder.first_occurrence_window(parse_context_window(value)?);
    }
    if let Some(value) = opts.collapse_redundant_parens {
        builder = builder.collapse_redundant_parens(value);
    }
    if let Some(value) = &opts.recovery {
        builder = builder.recovery(parse_recovery(value)?);
    }
    if let Some(directives) = opts.directives {
        for hanja in directives.require_hanja {
            builder = builder.directive(hanja, DirectiveAction::RequireHanja);
        }
        for hanja in directives.require_hangul {
            builder = builder.directive(hanja, DirectiveAction::RequireHangul);
        }
        for hanja in directives.skip_annotation {
            builder = builder.directive(hanja, DirectiveAction::SkipAnnotation);
        }
    }
    if let Some(rules) = opts.html {
        builder = builder.html_preserve_when(move |info| rules.matches(info.raw_attributes));
    }
    for (role, format, bytes, entries) in dictionaries {
        let role = role.as_deref().map(parse_source_role).transpose()?;
        match (format.as_deref(), bytes, entries) {
            (None, None, Some(entries)) => {
                builder = builder.push_dictionary_with_role(
                    entry_dictionary(&entries)?,
                    role.unwrap_or(SourceRole::Override),
                );
            }
            (Some("fst"), Some(bytes), None) => {
                // The umbrella error keeps the parser's source() chain for Python.
                let dictionary =
                    FstDictionary::from_bytes(&bytes).map_err(|e| map_error(e.into()))?;
                let role = role
                    .or_else(|| dictionary.source_evidence().map(|e| e.role))
                    .unwrap_or(SourceRole::Override);
                builder = builder.push_dictionary_with_role(dictionary, role);
            }
            (Some("cdb"), Some(bytes), None) => {
                let dictionary =
                    CdbDictionary::from_bytes(&bytes).map_err(|e| map_error(e.into()))?;
                let role = role
                    .or_else(|| dictionary.source_evidence().map(|e| e.role))
                    .unwrap_or(SourceRole::Override);
                builder = builder.push_dictionary_with_role(dictionary, role);
            }
            (Some(other), Some(_), None) => {
                return Err(Failure::new(
                    "unsupported-content-type",
                    format!("unknown dictionary format: {other}"),
                ));
            }
            _ => {
                return Err(invalid(
                    "dictionary must contain either entries or a format and bytes",
                ));
            }
        }
    }
    let converter = builder.build().map_err(map_error)?;
    let resolved = resolved_options(&converter, preset_name)?;
    Ok(NativeConverter {
        converter,
        resolved,
    })
}

fn entry_dictionary(json: &str) -> Result<MapDictionary> {
    let values: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| invalid(e.to_string()))?;
    let mut candidates = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        let entry: Entry = serde_json::from_value(value)
            .map_err(|e| invalid(format!("dictionary entry {index} is invalid: {e}")))?;
        if entry.hanja.is_empty()
            || entry.reading.is_empty()
            || entry.suffix_reading.as_deref() == Some("")
        {
            return Err(invalid(format!(
                "dictionary entry {index} has an empty hanja, reading or suffix_reading"
            )));
        }
        candidates.push((
            entry.hanja,
            ReadingCandidate {
                reading: entry.reading,
                suffix_reading: entry.suffix_reading,
                preference: entry.preference,
                mark: MatchMark {
                    require_hanja: entry.require_hanja,
                    require_hangul: entry.require_hangul,
                },
                alias: None,
            },
        ));
    }
    MapDictionary::from_candidates(candidates).map_err(|e| invalid(e.to_string()))
}

fn parse_preset(s: &str) -> Result<Preset> {
    match s {
        "ko-kr" => Ok(Preset::KoKr),
        "ko-kp" => Ok(Preset::KoKp),
        other => Err(invalid(format!("unknown preset: {other}"))),
    }
}

fn parse_render_mode(mode: &str, gloss: Option<&str>) -> Result<RenderOptions> {
    let render_mode = match mode {
        "hangul-only" => RenderMode::HangulOnly,
        "hangul-hanja-parens" => RenderMode::HangulHanjaParens,
        "hanja-hangul-parens" => RenderMode::HanjaHangulParens,
        "ruby-on-hangul" => RenderMode::Ruby(RubyBase::OnHangul),
        "ruby-on-hanja" => RenderMode::Ruby(RubyBase::OnHanja),
        "original" => RenderMode::Original,
        other => {
            return Err(invalid(format!("unknown rendering mode: {other}")));
        }
    };
    let original_gloss = if mode == "original" {
        match gloss.unwrap_or("parens") {
            "parens" => OriginalGloss::Parens,
            "ruby" => OriginalGloss::Ruby,
            other => {
                return Err(invalid(format!("unknown originalGloss: {other}")));
            }
        }
    } else {
        OriginalGloss::Parens
    };
    Ok(RenderOptions {
        mode: render_mode,
        original_gloss,
        ..RenderOptions::default()
    })
}

fn parse_hanja_variant_set(variant_set: &str) -> Result<HanjaVariantSet> {
    match variant_set {
        "as-dictionary" => Ok(HanjaVariantSet::AsDictionary),
        "shinjitai" => Ok(HanjaVariantSet::Shinjitai),
        "kanxi" => Ok(HanjaVariantSet::Kanxi),
        "simplified" => Ok(HanjaVariantSet::Simplified),
        "asahimoji" => Ok(HanjaVariantSet::Asahimoji),
        other => Err(invalid(format!("unknown hanjaVariantSet: {other}"))),
    }
}

fn parse_source_policy(value: &str) -> Result<SourcePolicy> {
    match value {
        "reference-authorized" => Ok(SourcePolicy::ReferenceAuthorized),
        "reference-first" => Ok(SourcePolicy::ReferenceFirst),
        "loanword-first" => Ok(SourcePolicy::LoanwordFirst),
        "ordered" => Ok(SourcePolicy::Ordered),
        _ => Err(invalid(format!("unknown source policy: {value}"))),
    }
}

fn parse_source_role(value: &str) -> Result<SourceRole> {
    match value {
        "override" => Ok(SourceRole::Override),
        "reference" => Ok(SourceRole::Reference),
        "loanword" => Ok(SourceRole::Loanword),
        _ => Err(invalid(format!("unknown dictionary source role: {value}"))),
    }
}

fn parse_segmentation(s: &str) -> Result<SegmentationStrategy> {
    match s {
        "lattice" => Ok(SegmentationStrategy::Lattice),
        "eager" => Ok(SegmentationStrategy::Eager),
        other => Err(invalid(format!("unknown segmentation strategy: {other}"))),
    }
}

fn parse_numerals(s: &str) -> Result<NumeralStrategy> {
    match s {
        "hangul-phonetic" => Ok(NumeralStrategy::HangulPhonetic),
        "positional-arabic" => Ok(NumeralStrategy::PositionalArabic),
        "additive-arabic" => Ok(NumeralStrategy::AdditiveArabic),
        "smart" => Ok(NumeralStrategy::Smart),
        other => Err(invalid(format!("unknown numeral strategy: {other}"))),
    }
}

fn parse_context_window(s: &str) -> Result<ContextWindow> {
    match s {
        "off" => Ok(ContextWindow::Off),
        "per-block" => Ok(ContextWindow::PerBlock),
        "per-section" => Ok(ContextWindow::PerSection),
        "per-document" => Ok(ContextWindow::PerDocument),
        other => Err(invalid(format!("unknown context window: {other}"))),
    }
}

fn parse_homophone_detection(s: &str) -> Result<HomophoneDetection> {
    match s {
        "context-local" => Ok(HomophoneDetection::ContextLocal),
        "dictionary-wide" => Ok(HomophoneDetection::DictionaryWide),
        other => Err(invalid(format!("unknown homophone detection: {other}"))),
    }
}

fn parse_recovery(s: &str) -> Result<Recovery> {
    match s {
        "strict" => Ok(Recovery::Strict),
        "lenient" => Ok(Recovery::Lenient),
        other => Err(invalid(format!("unknown recovery policy: {other}"))),
    }
}

fn resolved_options(converter: &SharedConverter, preset: &str) -> Result<String> {
    let options = converter.options();
    macro_rules! name {
        ($value:expr, {$($variant:path => $text:literal),+ $(,)?}) => {
            match $value { $($variant => $text),+ }
        };
    }
    let rendering = match options.rendering.mode {
        RenderMode::HangulOnly => "hangul-only",
        RenderMode::HangulHanjaParens => "hangul-hanja-parens",
        RenderMode::HanjaHangulParens => "hanja-hangul-parens",
        RenderMode::Ruby(RubyBase::OnHangul) => "ruby-on-hangul",
        RenderMode::Ruby(RubyBase::OnHanja) => "ruby-on-hanja",
        RenderMode::Original => "original",
    };
    Ok(serde_json::json!({
        "preset": preset,
        "rendering": rendering,
        "hanja_variant_set": name!(options.rendering.hanja_variant_set, {
            HanjaVariantSet::AsDictionary => "as-dictionary", HanjaVariantSet::Shinjitai => "shinjitai",
            HanjaVariantSet::Kanxi => "kanxi", HanjaVariantSet::Simplified => "simplified", HanjaVariantSet::Asahimoji => "asahimoji"
        }),
        "segmentation": match options.engine.segmentation {
            SegmentationStrategy::Lattice => "lattice", SegmentationStrategy::Eager => "eager",
            _ => return Err(Failure::new("internal", "unrecognized native segmentation")),
        },
        "source_policy": name!(options.source_policy, { SourcePolicy::ReferenceAuthorized => "reference-authorized", SourcePolicy::ReferenceFirst => "reference-first", SourcePolicy::LoanwordFirst => "loanword-first", SourcePolicy::Ordered => "ordered" }),
        "reference_only": options.reference_only,
        "numerals": match options.engine.numeral_strategy {
            NumeralStrategy::HangulPhonetic => "hangul-phonetic", NumeralStrategy::PositionalArabic => "positional-arabic",
            NumeralStrategy::AdditiveArabic => "additive-arabic", NumeralStrategy::Smart => "smart",
            _ => return Err(Failure::new("internal", "unrecognized native numeral strategy")),
        },
        "initial_sound_law": options.engine.initial_sound_law,
        "homophone_window": context_name(options.homophone_window),
        "homophone_detection": name!(options.homophone_detection, { HomophoneDetection::ContextLocal => "context-local", HomophoneDetection::DictionaryWide => "dictionary-wide" }),
        "first_occurrence_window": context_name(options.first_occurrence_window),
        "collapse_redundant_parens": options.collapse_redundant_parens,
        "recovery": name!(options.recovery, { Recovery::Strict => "strict", Recovery::Lenient => "lenient" }),
        "punctuation": options.punctuation,
    }).to_string())
}

fn context_name(value: ContextWindow) -> &'static str {
    match value {
        ContextWindow::Off => "off",
        ContextWindow::PerBlock => "per-block",
        ContextWindow::PerSection => "per-section",
        ContextWindow::PerDocument => "per-document",
    }
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeConverter>()?;
    module.add("NativeError", module.py().get_type::<NativeError>())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn interpreter_initializes_and_imports_standard_library() {
        Python::initialize();
        Python::attach(|py| {
            py.import("encodings")
                .expect("Python standard library must be discoverable");
        });
    }

    #[test]
    fn shared_runtime_is_send_and_sync() {
        fn assert_shared<T: Send + Sync + 'static>() {}
        assert_shared::<NativeConverter>();
    }

    #[test]
    fn common_boundary_detaches_and_translates_rust_panics() {
        Python::initialize();
        Python::attach(|py| {
            py.import("encodings").unwrap();
            let (sender, receiver) = mpsc::channel();
            let other = std::thread::spawn(move || {
                Python::attach(|_| sender.send(()).unwrap());
            });
            detached(py, move || {
                receiver
                    .recv_timeout(Duration::from_secs(10))
                    .map_err(|_| Failure::new("internal", "another thread could not attach"))
            })
            .expect("Rust work must release interpreter attachment");
            other.join().unwrap();
            let error = detached::<()>(py, || panic!("test native unwind"))
                .expect_err("panic must become a Python exception");
            assert!(error.is_instance_of::<NativeError>(py));
            let text = error.value(py).to_string();
            let value: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(value["code"], "internal");
        });
    }

    #[test]
    fn presets_resolve_actual_options_without_bundled_data() {
        let south = build("{}", vec![]).unwrap();
        let north = build(
            r#"{"preset":"ko-kp","initial_sound_law":null,"homophone_window":null}"#,
            vec![],
        )
        .unwrap();
        for (converter, sound, window) in [(south, true, "per-block"), (north, false, "off")] {
            let options: serde_json::Value = serde_json::from_str(&converter.resolved).unwrap();
            assert_eq!(options.as_object().unwrap().len(), 14);
            assert_eq!(options["initial_sound_law"], sound);
            assert_eq!(options["homophone_window"], window);
            assert_eq!(options["punctuation"]["comma"], serde_json::Value::Null);
        }
    }

    #[test]
    fn native_inputs_validate_private_json_and_dictionary_shapes() {
        for options in [
            "null",
            "[]",
            "{",
            r#"{"unknown":true}"#,
            r#"{"reference_only":1}"#,
            r#"{"preset":"unknown"}"#,
        ] {
            assert_eq!(build(options, vec![]).err().unwrap().code, "invalid-input");
        }
        for input in [
            (None, None, None, None),
            (None, Some("fst".into()), Some(vec![]), Some("[]".into())),
            (None, Some("fst".into()), None, None),
        ] {
            assert_eq!(
                build("{}", vec![input]).err().unwrap().code,
                "invalid-input"
            );
        }
        for entries in [
            r#"[{"hanja":"漢","reading":"한","preference":true}]"#,
            r#"[{"hanja":"漢","reading":"한","preference":4294967296}]"#,
            r#"[{"hanja":"","reading":"한"}]"#,
            r#"[{"hanja":"漢","reading":"한","suffix_reading":""}]"#,
        ] {
            assert_eq!(
                entry_dictionary(entries).err().unwrap().code,
                "invalid-input"
            );
        }
    }

    #[test]
    fn entries_preserve_competing_candidates_suffixes_and_marks() {
        let entries = r#"[
            {"hanja":"東京","reading":"도쿄","preference":10},
            {"hanja":"東京","reading":"동경"},
            {"hanja":"年代","reading":"연대","suffix_reading":"년대"},
            {"hanja":"漢字","reading":"한자","require_hanja":true}
        ]"#;
        let converter = build(
            r#"{"homophone_window":"off"}"#,
            vec![(None, None, None, Some(entries.into()))],
        )
        .unwrap();
        assert_eq!(
            converter
                .converter
                .convert_text_to_string("東京를 東京을 1990年代 漢字")
                .unwrap(),
            "도쿄를 동경을 1990년대 한자(漢字)"
        );
    }

    #[test]
    fn punctuation_overrides_inherit_preserve_and_validate() {
        let converter =
            build(r#"{"punctuation":{"base":"modern","period":null}}"#, vec![]).unwrap();
        let options: serde_json::Value = serde_json::from_str(&converter.resolved).unwrap();
        assert_eq!(options["punctuation"]["comma"]["mark"], ",");
        assert!(options["punctuation"]["period"].is_null());
        assert_eq!(
            build(
                r#"{"punctuation":{"comma":{"mark":"ab","after":"single"}}}"#,
                vec![]
            )
            .err()
            .unwrap()
            .code,
            "invalid-input"
        );
    }
    #[test]
    fn mapped_causes_are_root_first_and_keep_native_codes() {
        #[derive(Debug)]
        struct Layer(&'static str, Option<Box<Layer>>);
        impl std::fmt::Display for Layer {
            fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                output.write_str(self.0)
            }
        }
        impl std::error::Error for Layer {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.1
                    .as_deref()
                    .map(|layer| layer as &dyn std::error::Error)
            }
        }
        let error = std::io::Error::other(Layer("immediate", Some(Box::new(Layer("root", None)))));
        let mapped = map_error(error.into());
        assert_eq!(mapped.code, "io");
        assert_eq!(
            mapped
                .chain
                .iter()
                .map(|cause| cause.message.as_str())
                .collect::<Vec<_>>(),
            ["root", "immediate"]
        );
        assert!(mapped.chain.iter().all(|cause| cause.code == "internal"));
    }
}

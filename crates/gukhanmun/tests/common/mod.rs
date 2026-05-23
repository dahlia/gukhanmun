// Gukhanmun: umbrella library that wires the engine and adapters together.
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

//! Shared fixture loader, sidecar parser, and runner used by the umbrella
//! crate's `fixtures` integration test binary.
//!
//! The harness scans the workspace-level `tests/fixtures/` tree, pairs each
//! `*.input.<ext>` file with its sibling `*.expected.<ext>` and an optional
//! `*.toml` sidecar, builds a [`gukhanmun::Builder`] according to the
//! sidecar's configuration, runs the matching converter entry point, and
//! asserts the result against the expected file.
//!
//! Helpers here are dev-only — the file is a regular `mod common` consumed by
//! `tests/fixtures.rs`, so nothing here is published as part of the umbrella
//! crate's API surface.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use gukhanmun::{Builder, ContextWindow, MapDictionary, MatchMark, Preset, Recovery};
use serde::Deserialize;

/// Format inferred from a fixture's second-to-last extension (`*.input.html`
/// → `Html`, `*.input.md` → `Markdown`, `*.input.txt` → `Text`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureFormat {
    /// HTML fragment fixture (`.input.html` / `.expected.html`).
    Html,
    /// Markdown fixture (`.input.md` / `.expected.md`).
    Markdown,
    /// Plain-text fixture (`.input.txt` / `.expected.txt`).
    Text,
}

impl FixtureFormat {
    fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            "html" => Some(Self::Html),
            "md" => Some(Self::Markdown),
            "txt" => Some(Self::Text),
            _ => None,
        }
    }

    fn ext_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Markdown => "md",
            Self::Text => "txt",
        }
    }
}

/// A discovered fixture ready to be executed by the harness.
///
/// Discovery records only filesystem locations; the per-fixture file reads
/// and sidecar parsing happen inside [`run_fixture`] so that an unreadable or
/// malformed individual fixture surfaces as a failing `libtest-mimic` trial
/// rather than aborting the whole binary at discovery time.
#[derive(Debug)]
pub struct Fixture {
    /// Test name reported to `libtest-mimic`; derived from category +
    /// fixture stem (e.g. `html::initial_sound_raw`).
    pub name: String,
    /// Detected format (HTML, Markdown, or plain text).
    pub format: FixtureFormat,
    /// Absolute path of `<stem>.input.<ext>`.
    pub input_path: PathBuf,
    /// Absolute path of `<stem>.expected.<ext>`; existence is verified at
    /// run time, not at discovery.
    pub expected_path: PathBuf,
    /// Absolute path of `<stem>.toml`; `None` if no sidecar was discovered.
    pub sidecar_path: Option<PathBuf>,
}

/// Sidecar TOML schema understood by the harness.  Every field is optional;
/// an empty sidecar is equivalent to no sidecar.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sidecar {
    /// Human-readable description; ignored by the runner but useful when the
    /// fixture diverges from a Seonbi original or otherwise needs context.
    #[serde(default)]
    pub description: Option<String>,
    /// Preset used to seed the [`Builder`].  Defaults to `ko-kr`.
    #[serde(default)]
    pub preset: Option<PresetName>,
    /// Whether the bundled *Standard Korean Language Dictionary* is loaded.
    /// Defaults to `false` so fixtures stay reproducible without it.
    #[serde(default)]
    pub use_bundled_stdict: Option<bool>,
    /// Engine-level option overrides (segmentation strategy, initial sound
    /// law, numeral strategy).
    #[serde(default)]
    pub options: SidecarOptions,
    /// Stream-shaping middleware overrides (homophone window).
    #[serde(default)]
    pub engine: SidecarEngine,
    /// Assertion strategy used to compare the conversion result to
    /// `<stem>.expected.<ext>`.
    #[serde(default)]
    pub assertion: SidecarAssertion,
    /// In-memory dictionary records added to the lookup chain via
    /// [`MapDictionary`].
    #[serde(default)]
    pub dictionary: SidecarDictionary,
    /// Reader-level error recovery policy.  Defaults to the preset's
    /// [`Recovery`] (currently `strict` for both `ko-kr` and `ko-kp`).
    #[serde(default)]
    pub recovery: Option<RecoveryName>,
    /// Markdown variant for `.md` fixtures.  Ignored for HTML / text inputs.
    #[serde(default)]
    pub markdown: SidecarMarkdown,
}

/// `recovery = "strict" | "lenient"`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryName {
    /// `Recovery::Strict` — reader errors propagate immediately.
    Strict,
    /// `Recovery::Lenient` — reader errors are logged and recovered into a
    /// verbatim token stream.
    Lenient,
}

impl RecoveryName {
    fn into_recovery(self) -> Recovery {
        match self {
            RecoveryName::Strict => Recovery::Strict,
            RecoveryName::Lenient => Recovery::Lenient,
        }
    }
}

/// `preset = "ko-kr" | "ko-kp"`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PresetName {
    /// South Korean preset (defaults).
    KoKr,
    /// North Korean preset (no bundled dictionary, no initial sound law).
    KoKp,
}

impl PresetName {
    fn into_preset(self) -> Preset {
        match self {
            PresetName::KoKr => Preset::KoKr,
            PresetName::KoKp => Preset::KoKp,
        }
    }
}

/// Engine options that fixtures may override (`[options]` table).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarOptions {
    /// Toggles the initial sound law.  Default follows the preset.
    #[serde(default)]
    pub initial_sound_law: Option<bool>,
}

/// Stream-shaping middleware overrides (`[engine]` table).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarEngine {
    /// Homophone disambiguation window.  Default follows the preset.
    #[serde(default)]
    pub disambiguation: Option<DisambiguationWindow>,
}

/// `disambiguation = "per-block" | "per-document" | "off"`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisambiguationWindow {
    /// `ContextWindow::PerBlock`.
    PerBlock,
    /// `ContextWindow::PerDocument`.
    PerDocument,
    /// `ContextWindow::Off`.
    Off,
}

impl DisambiguationWindow {
    fn into_window(self) -> ContextWindow {
        match self {
            DisambiguationWindow::PerBlock => ContextWindow::PerBlock,
            DisambiguationWindow::PerDocument => ContextWindow::PerDocument,
            DisambiguationWindow::Off => ContextWindow::Off,
        }
    }
}

/// Assertion configuration (`[assertion]` table).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarAssertion {
    /// Comparison kind; `"exact"` (default) compares the entire string,
    /// `"contains"` checks that every substring listed in `needles` appears
    /// in the converted output.
    #[serde(default)]
    pub kind: AssertionKind,
    /// Substrings that must appear in the converted output; only consulted
    /// when `kind = "contains"`.
    #[serde(default)]
    pub needles: Vec<String>,
}

/// `kind = "exact" | "contains"`.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssertionKind {
    /// Compare the converter output to `<stem>.expected.<ext>` for byte
    /// equality.
    #[default]
    Exact,
    /// Verify that each `needles` entry appears in the converter output.
    Contains,
}

/// In-memory dictionary records (`[dictionary]` table).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarDictionary {
    /// Individual dictionary entries.  Each record contributes a single
    /// [`MapDictionary`] insertion.
    #[serde(default)]
    pub records: Vec<DictionaryRecord>,
}

/// One dictionary entry from the sidecar.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DictionaryRecord {
    /// Hanja key.
    pub hanja: String,
    /// Hangul reading.
    pub reading: String,
    /// Whether this entry should be marked `require_hanja`.
    #[serde(default)]
    pub require_hanja: bool,
    /// Whether this entry should be marked `require_hangul`.
    #[serde(default)]
    pub require_hangul: bool,
}

/// Markdown-specific sidecar fields (`[markdown]` table).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarMarkdown {
    /// Markdown variant (`"common-mark"` default or `"gfm"`).
    #[serde(default)]
    pub variant: Option<MarkdownVariantName>,
}

/// `variant = "common-mark" | "gfm"`.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkdownVariantName {
    /// Strict CommonMark (default).
    CommonMark,
    /// GitHub Flavored Markdown.
    Gfm,
}

#[cfg(feature = "markdown")]
impl MarkdownVariantName {
    fn into_variant(self) -> gukhanmun::markdown::MarkdownVariant {
        match self {
            MarkdownVariantName::CommonMark => gukhanmun::markdown::MarkdownVariant::CommonMark,
            MarkdownVariantName::Gfm => gukhanmun::markdown::MarkdownVariant::Gfm,
        }
    }
}

/// Returns the absolute path of the workspace's `tests/fixtures` root,
/// resolved relative to this crate's `CARGO_MANIFEST_DIR`.
pub fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
}

/// Walks the fixture tree under `root`, returning every discovered
/// [`Fixture`].
///
/// The discovery rule is: every regular file whose name matches
/// `<stem>.input.<ext>` (where `<ext>` is a [`FixtureFormat`] extension) is
/// treated as a fixture.  A sibling `<stem>.expected.<ext>` is required at
/// run time; a sibling `<stem>.toml` is optional.
///
/// # Panics
///
/// Panics if `root` does not exist or cannot be enumerated.  A missing root
/// is a hard configuration error (the harness was wired up against the wrong
/// path), and silently producing zero trials would mask it.  Symlinks are
/// rejected during the walk to prevent a fixture from escaping the
/// `tests/fixtures` subtree.
pub fn discover(root: &Path) -> Vec<Fixture> {
    assert!(
        root.exists(),
        "fixtures root does not exist: {}",
        root.display()
    );
    let root_meta = fs::symlink_metadata(root)
        .unwrap_or_else(|e| panic!("stat fixtures root {}: {e}", root.display()));
    assert!(
        !root_meta.file_type().is_symlink(),
        "fixtures root must not be a symlink: {}",
        root.display()
    );

    let mut fixtures = Vec::new();
    let categories = read_subdirs(root);
    for category in categories {
        let category_name = category
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_else(|| panic!("non-utf8 category name: {}", category.display()))
            .to_owned();
        for path in read_files(&category) {
            if let Some(fixture) = try_discover_fixture(&category_name, &path) {
                fixtures.push(fixture);
            }
        }
    }
    assert!(
        !fixtures.is_empty(),
        "fixtures root {} contains no fixtures; expected `*.input.<ext>` files \
         under category subdirectories",
        root.display()
    );
    fixtures
}

fn read_subdirs(root: &Path) -> Vec<PathBuf> {
    let entries =
        fs::read_dir(root).unwrap_or_else(|e| panic!("read fixtures root {}: {e}", root.display()));
    let mut out = Vec::new();
    for entry in entries {
        let entry =
            entry.unwrap_or_else(|e| panic!("enumerate fixtures root {}: {e}", root.display()));
        let file_type = entry
            .file_type()
            .unwrap_or_else(|e| panic!("stat fixture entry {}: {e}", entry.path().display()));
        if file_type.is_symlink() {
            panic!(
                "symlinks are not permitted in the fixtures tree: {}",
                entry.path().display()
            );
        }
        if file_type.is_dir() {
            out.push(entry.path());
        }
    }
    out.sort();
    out
}

fn read_files(category: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(category)
        .unwrap_or_else(|e| panic!("read fixture category {}: {e}", category.display()));
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry
            .unwrap_or_else(|e| panic!("enumerate fixture category {}: {e}", category.display()));
        let file_type = entry
            .file_type()
            .unwrap_or_else(|e| panic!("stat fixture entry {}: {e}", entry.path().display()));
        if file_type.is_symlink() {
            panic!(
                "symlinks are not permitted in the fixtures tree: {}",
                entry.path().display()
            );
        }
        if file_type.is_file() {
            out.push(entry.path());
        }
    }
    out.sort();
    out
}

fn try_discover_fixture(category: &str, input_path: &Path) -> Option<Fixture> {
    let file_name = input_path.file_name()?.to_str()?;
    let ext = input_path.extension()?.to_str()?;
    let format = FixtureFormat::from_ext(ext)?;
    let stem = file_name.strip_suffix(&format!(".input.{ext}"))?.to_owned();
    let expected_path =
        input_path.with_file_name(format!("{}.expected.{}", stem, format.ext_str()));
    let sidecar_candidate = input_path.with_file_name(format!("{stem}.toml"));
    let sidecar_path = match fs::symlink_metadata(&sidecar_candidate) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                panic!(
                    "symlinks are not permitted in the fixtures tree: {}",
                    sidecar_candidate.display()
                );
            }
            Some(sidecar_candidate)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("stat fixture sidecar {}: {e}", sidecar_candidate.display()),
    };
    let name = format!("{category}::{}", stem.replace('-', "_"));
    Some(Fixture {
        name,
        format,
        input_path: input_path.to_owned(),
        expected_path,
        sidecar_path,
    })
}

/// Outcome of running a [`Fixture`] through the converter.  Returned as a
/// `Result` rather than panicking so the caller can map it to the
/// `libtest-mimic` outcome variants.
pub type RunResult = Result<(), String>;

/// Drives `fixture` through a [`Converter`](gukhanmun::Converter) configured
/// from its sidecar, then validates the conversion result against
/// `<stem>.expected.<ext>` per the sidecar's assertion kind.
///
/// All file reads and the sidecar parse happen here so that a single
/// malformed fixture fails as a `libtest-mimic` trial instead of aborting
/// the harness binary.
pub fn run_fixture(fixture: &Fixture) -> RunResult {
    let input = fs::read_to_string(&fixture.input_path).map_err(|e| {
        format!(
            "input file read failed at {}: {e}",
            fixture.input_path.display()
        )
    })?;
    let expected = fs::read_to_string(&fixture.expected_path).map_err(|e| {
        format!(
            "expected file read failed at {}: {e}",
            fixture.expected_path.display()
        )
    })?;
    let sidecar = match &fixture.sidecar_path {
        Some(path) => {
            let text = fs::read_to_string(path)
                .map_err(|e| format!("sidecar read failed at {}: {e}", path.display()))?;
            Some(
                toml::from_str::<Sidecar>(&text)
                    .map_err(|e| format!("sidecar parse failed at {}: {e}", path.display()))?,
            )
        }
        None => None,
    };
    let sidecar = sidecar.as_ref();

    let mut builder = match sidecar.and_then(|s| s.preset) {
        Some(preset) => Builder::with_preset(preset.into_preset()),
        None => Builder::new(),
    };
    let use_bundled = sidecar.and_then(|s| s.use_bundled_stdict).unwrap_or(false);
    if !use_bundled {
        builder = builder.no_bundled_stdict();
    } else {
        builder = builder.bundled_stdict();
    }
    if let Some(opts) = sidecar.map(|s| &s.options)
        && let Some(law) = opts.initial_sound_law
    {
        builder = builder.initial_sound_law(law);
    }
    if let Some(engine) = sidecar.map(|s| &s.engine)
        && let Some(window) = engine.disambiguation
    {
        builder = builder.homophone_window(window.into_window());
    }
    if let Some(recovery) = sidecar.and_then(|s| s.recovery) {
        builder = builder.recovery(recovery.into_recovery());
    }
    if let Some(dict) = sidecar.map(|s| &s.dictionary)
        && !dict.records.is_empty()
    {
        let mut map = MapDictionary::new();
        for record in &dict.records {
            let mark = MatchMark {
                require_hanja: record.require_hanja,
                require_hangul: record.require_hangul,
            };
            map.insert_marked(record.hanja.clone(), record.reading.clone(), mark);
        }
        builder = builder.push_dictionary(map);
    }
    let converter = builder
        .build()
        .map_err(|e| format!("builder failed: {e}"))?;

    let actual = match fixture.format {
        FixtureFormat::Text => converter
            .convert_text_to_string(&input)
            .map_err(|e| format!("plain-text conversion failed: {e}"))?,
        FixtureFormat::Html => {
            #[cfg(feature = "html")]
            {
                converter
                    .convert_html_fragment_to_string(&input)
                    .map_err(|e| format!("HTML conversion failed: {e}"))?
            }
            #[cfg(not(feature = "html"))]
            {
                return Err("HTML fixture requires the `html` feature".into());
            }
        }
        FixtureFormat::Markdown => {
            #[cfg(feature = "markdown")]
            {
                let variant = sidecar
                    .and_then(|s| s.markdown.variant)
                    .map(MarkdownVariantName::into_variant)
                    .unwrap_or(gukhanmun::markdown::MarkdownVariant::CommonMark);
                converter
                    .convert_markdown_to_string(&input, variant)
                    .map_err(|e| format!("Markdown conversion failed: {e}"))?
            }
            #[cfg(not(feature = "markdown"))]
            {
                return Err("Markdown fixture requires the `markdown` feature".into());
            }
        }
    };

    let assertion = sidecar.map(|s| &s.assertion).cloned().unwrap_or_default();
    let description = sidecar.and_then(|s| s.description.as_deref());
    match assertion.kind {
        AssertionKind::Exact => {
            if actual == expected {
                Ok(())
            } else {
                Err(annotate(description, diff_message(&expected, &actual)))
            }
        }
        AssertionKind::Contains => {
            if assertion.needles.is_empty() {
                return Err(annotate(
                    description,
                    "assertion.kind = \"contains\" requires a non-empty \
                     `assertion.needles` array; an empty needles list would \
                     accept any converter output unconditionally"
                        .to_owned(),
                ));
            }
            let missing: Vec<&str> = assertion
                .needles
                .iter()
                .filter(|needle| !actual.contains(needle.as_str()))
                .map(String::as_str)
                .collect();
            if missing.is_empty() {
                Ok(())
            } else {
                Err(annotate(
                    description,
                    format!(
                        "expected substrings missing from converter output: {missing:?}\n\
                         actual output:\n{actual}",
                    ),
                ))
            }
        }
    }
}

fn annotate(description: Option<&str>, body: String) -> String {
    match description {
        Some(text) => format!("{}\n{body}", text.trim()),
        None => body,
    }
}

impl Clone for SidecarAssertion {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind,
            needles: self.needles.clone(),
        }
    }
}

fn diff_message(expected: &str, actual: &str) -> String {
    let mut msg = String::new();
    msg.push_str("converter output diverged from expected fixture\n");
    msg.push_str("--- expected ---\n");
    msg.push_str(expected);
    if !expected.ends_with('\n') {
        msg.push('\n');
    }
    msg.push_str("--- actual ---\n");
    msg.push_str(actual);
    if !actual.ends_with('\n') {
        msg.push('\n');
    }
    msg
}

// Gukhanmun: Command-line interface for Gukhanmun plain-text conversion.
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

use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, ValueEnum};
use gukhanmun_cdb::CdbDictionary;
use gukhanmun_core::{
    ChainDictionary, ContextWindow, DirectiveAction, Engine, EngineOptions, HanjaDictionary,
    InputToken, NumeralStrategy, OriginalGloss, OutputToken, PlainScopeData, RenderMode,
    RenderOptions, RubyBase, ScopeData, SegmentationStrategy, UserDirectives,
    apply_user_directives, filter_first_occurrences, mark_homophones,
    process_tokens_iter_with_options, render_tokens_iter, write_plain_text,
};
use gukhanmun_fst::FstDictionary;
use gukhanmun_html::{
    HtmlElementInfo, HtmlReaderOptions, read_html_fragment_with_options, write_html_fragment,
};
use gukhanmun_markdown::{MarkdownVariant, read_markdown, write_markdown};

const FST_MAGIC: &[u8; 8] = b"GUKHMFST";

type CliDictionary = ChainDictionary<Box<dyn HanjaDictionary>>;

#[derive(Debug, Parser)]
#[command(
    name = "gukhanmun",
    about = "Convert Korean mixed-script plain text into hangul text."
)]
struct Cli {
    #[command(flatten)]
    io: IoArgs,

    #[command(flatten)]
    language: LanguageArgs,

    #[command(flatten)]
    conversion: ConversionArgs,

    #[command(flatten)]
    rendering: RenderingPolicyArgs,

    #[command(flatten)]
    directives: DirectiveArgs,

    #[command(flatten)]
    html: HtmlArgs,
}

#[derive(Debug, Args)]
#[command(next_help_heading = "Input/output")]
struct IoArgs {
    /// Input file to read from; reads from standard input when omitted.
    #[arg(value_name = "INPUT")]
    input: Option<PathBuf>,

    /// Output file to write to; writes to standard output when omitted.
    /// When the same path as INPUT, the file is replaced atomically.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// Input/output format.  Inferred from the input file extension when omitted
    /// (text/html for .html/.htm, text/markdown for .md/.markdown, text/plain
    /// otherwise); falls back to text/plain when reading from standard input.
    /// Accepted values: text/plain, text/html, text/markdown.  MIME parameters
    /// are accepted for text/markdown; use "text/markdown; variant=GFM" to
    /// enable GitHub Flavored Markdown (tables, footnotes, strikethrough, task
    /// lists).  Unrecognised parameters are ignored.
    #[arg(short, long, value_name = "MIME", value_parser = parse_format)]
    format: Option<Format>,
}

#[derive(Debug, Args)]
#[command(next_help_heading = "Language and dictionaries")]
struct LanguageArgs {
    /// Language variant preset.  ko-kr (default) enables the bundled Standard
    /// Korean Dictionary (標準國語大辭典) and the initial sound law (頭音法則).  ko-kp disables
    /// both, targeting North Korean orthography.
    #[arg(short, long, value_enum, default_value_t = Preset::KoKr)]
    preset: Preset,

    /// Path to a user-supplied dictionary file (.gukfst or .gukcdb).  May be
    /// repeated; later dictionaries take priority over earlier ones and over the
    /// bundled Standard Korean Dictionary (標準國語大辭典).
    #[arg(short = 'd', long = "dictionary", value_name = "PATH")]
    dictionaries: Vec<PathBuf>,

    /// Disable the bundled Standard Korean Dictionary (標準國語大辭典).
    #[arg(short = 'S', long)]
    no_stdict: bool,
}

#[derive(Debug, Args)]
#[command(next_help_heading = "Conversion")]
struct ConversionArgs {
    /// Segmentation strategy for hanja-containing spans.  lattice (default)
    /// chooses the best path through all dictionary matches.  eager greedily
    /// takes the longest match at each cursor for lower overhead.
    #[arg(short = 's', long, value_enum, default_value_t = Segmentation::Lattice)]
    segmentation: Segmentation,

    /// Numeral conversion strategy for fallback hanja numerals.
    /// hangul-phonetic (default) emits hangul annotations; positional-arabic
    /// normalizes digit-only runs; additive-arabic normalizes place-marker
    /// numerals; smart chooses Arabic only for common numeric forms.
    #[arg(long, visible_alias = "numeral-strategy", value_enum)]
    numerals: Option<Numerals>,

    /// Enable the initial sound law (頭音法則), overriding the preset default.
    #[arg(short = 'i', long, visible_alias = "dueum")]
    initial_sound_law: bool,

    /// Disable the initial sound law (頭音法則), overriding the preset default.
    #[arg(short = 'I', long, visible_alias = "no-dueum")]
    no_initial_sound_law: bool,
}

#[derive(Debug, Args)]
#[command(next_help_heading = "Rendering policy")]
struct RenderingPolicyArgs {
    /// Controls how hanja annotations appear in the output.  hangul-only
    /// (default for most presets) emits hangul, adding parenthesized hanja only
    /// when disambiguation requires it.  hangul-hanja-parens always emits
    /// 한글(漢字).  hanja-hangul-parens always emits 漢字(한글).  ruby-on-hangul
    /// and ruby-on-hanja emit a `ruby` element with hangul or hanja as the
    /// base; scopes that disallow inline markup fall back to parens.  original
    /// keeps the source hanja, adding a hangul gloss only where required.
    #[arg(short, long, value_enum)]
    rendering: Option<Rendering>,

    /// Gloss form used by the original renderer.  parens (default) emits
    /// 漢字(한글); ruby emits a `ruby` element with hanja as the base and
    /// hangul as the rt gloss, falling back to parens in scopes that disallow
    /// inline markup.  Only valid with --rendering original.
    #[arg(long = "original-gloss", value_enum)]
    original_gloss: Option<OriginalGlossArg>,

    /// Homophone disambiguation context.  off disables homophone marking;
    /// per-block resets at block scopes; per-section resets at headings;
    /// per-document uses one window for the full input.
    #[arg(long, value_enum)]
    disambiguation: Option<CliContextWindow>,

    /// Context for clearing repeated dictionary presentation requirements.
    /// off leaves every occurrence as marked by dictionaries and directives.
    #[arg(long, value_enum)]
    first_occurrence: Option<CliContextWindow>,
}

#[derive(Debug, Args)]
#[command(next_help_heading = "User directives")]
struct DirectiveArgs {
    /// Require visible hanja for a literal hanja form.  May be repeated.
    #[arg(long = "require-hanja", value_name = "HANJA")]
    require_hanja: Vec<String>,

    /// Require visible hanja for hanja forms matched by a CLI glob pattern.
    /// The pattern supports `*` for any sequence and `?` for one character.
    #[arg(long = "require-hanja-glob", value_name = "PATTERN")]
    require_hanja_glob: Vec<String>,

    /// Require a hangul gloss for a literal hanja form.  May be repeated.
    #[arg(long = "require-hangul", value_name = "HANJA")]
    require_hangul: Vec<String>,

    /// Require a hangul gloss for hanja forms matched by a CLI glob pattern.
    /// The pattern supports `*` for any sequence and `?` for one character.
    #[arg(long = "require-hangul-glob", value_name = "PATTERN")]
    require_hangul_glob: Vec<String>,

    /// Collapse annotation rendering for a literal hanja form.  May be repeated.
    #[arg(long = "skip-annotation", value_name = "HANJA")]
    skip_annotation: Vec<String>,

    /// Collapse annotation rendering for hanja forms matched by a CLI glob
    /// pattern.  The pattern supports `*` for any sequence and `?` for one
    /// character.
    #[arg(long = "skip-annotation-glob", value_name = "PATTERN")]
    skip_annotation_glob: Vec<String>,
}

#[derive(Debug, Args)]
#[command(next_help_heading = "HTML")]
struct HtmlArgs {
    /// Preserve any HTML element whose `class` attribute contains the given
    /// CSS class.  Matching elements and their descendants pass through the
    /// converter untouched, in addition to the built-in preserved tags
    /// (`pre`, `code`, `kbd`, `script`, `style`, `textarea`) and the
    /// inherited `lang` rule.  May be repeated.  Only valid with
    /// `--format text/html`.
    #[arg(long = "html-preserve-class", value_name = "CLASS")]
    html_preserve_class: Vec<String>,

    /// Preserve any HTML element matching an attribute predicate.  Accepts
    /// either `KEY` (matches when the attribute is present with any value or
    /// none, including as a boolean attribute) or `KEY=VALUE` (matches when
    /// the attribute is present with the exact value).  Matched elements and
    /// their descendants pass through untouched.  May be repeated.  Only
    /// valid with `--format text/html`.
    #[arg(long = "html-preserve-attr", value_name = "KEY[=VALUE]")]
    html_preserve_attr: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Format {
    PlainText,
    Html,
    Markdown(MarkdownVariant),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Preset {
    KoKr,
    KoKp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Rendering {
    HangulOnly,
    HangulHanjaParens,
    HanjaHangulParens,
    RubyOnHangul,
    RubyOnHanja,
    Original,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OriginalGlossArg {
    Parens,
    Ruby,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Segmentation {
    Lattice,
    Eager,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Numerals {
    HangulPhonetic,
    PositionalArabic,
    AdditiveArabic,
    Smart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliContextWindow {
    Off,
    PerBlock,
    PerSection,
    PerDocument,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolvedOptions {
    rendering: RenderOptions,
    engine: EngineOptions,
    bundled_stdict: bool,
    homophone_window: ContextWindow,
    first_occurrence_window: ContextWindow,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    let options = resolve_options(&cli)?;
    let directives = build_user_directives(&cli);
    let dictionary = load_dictionary(&cli.language.dictionaries, options.bundled_stdict)?;
    let format = cli.io.format.unwrap_or_else(|| {
        cli.io
            .input
            .as_deref()
            .map(detect_format)
            .unwrap_or(Format::PlainText)
    });
    let html_reader_options = build_html_reader_options(&cli.html, format)?;

    if let (Some(input_path), Some(output_path)) = (&cli.io.input, &cli.io.output)
        && is_same_existing_file(input_path, output_path)?
    {
        return convert_file_in_place(
            input_path,
            output_path,
            &dictionary,
            options,
            &directives,
            &html_reader_options,
            format,
        );
    }

    let input: Box<dyn BufRead> = match &cli.io.input {
        Some(path) => {
            Box::new(BufReader::new(fs::File::open(path).with_context(|| {
                format!("failed to open input {}", path.display())
            })?))
        }
        None => Box::new(BufReader::new(io::stdin().lock())),
    };
    let output: Box<dyn Write> = match &cli.io.output {
        Some(path) => Box::new(BufWriter::new(
            fs::File::create(path)
                .with_context(|| format!("failed to create output {}", path.display()))?,
        )),
        None => Box::new(BufWriter::new(io::stdout().lock())),
    };

    convert_document(
        input,
        output,
        &dictionary,
        options,
        &directives,
        &html_reader_options,
        format,
    )
}

fn convert_file_in_place(
    input_path: &Path,
    output_path: &Path,
    dictionary: &CliDictionary,
    options: ResolvedOptions,
    directives: &UserDirectives<'_>,
    html_reader_options: &HtmlReaderOptions<'_>,
    format: Format,
) -> Result<()> {
    let original_permissions = fs::metadata(input_path)
        .with_context(|| format!("failed to inspect input {}", input_path.display()))?
        .permissions();
    let (temp_path, temp_file) = create_temp_output_file(output_path)?;
    let result = (|| {
        temp_file
            .set_permissions(original_permissions)
            .with_context(|| {
                format!(
                    "failed to copy permissions to temporary output {}",
                    temp_path.display()
                )
            })?;
        let input = BufReader::new(
            fs::File::open(input_path)
                .with_context(|| format!("failed to open input {}", input_path.display()))?,
        );
        let output = BufWriter::new(temp_file);
        convert_document(
            input,
            output,
            dictionary,
            options,
            directives,
            html_reader_options,
            format,
        )?;
        fs::rename(&temp_path, output_path).with_context(|| {
            format!(
                "failed to replace {} with temporary output {}",
                output_path.display(),
                temp_path.display()
            )
        })
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

fn create_temp_output_file(output_path: &Path) -> Result<(PathBuf, fs::File)> {
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");

    for attempt in 0..100 {
        let candidate = parent.join(format!(
            ".{file_name}.gukhanmun-tmp-{}-{attempt}",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temporary output {}", candidate.display())
                });
            }
        }
    }

    bail!(
        "failed to create a unique temporary output path next to {}",
        output_path.display()
    )
}

fn is_same_existing_file(input_path: &Path, output_path: &Path) -> Result<bool> {
    let input_metadata = fs::metadata(input_path)
        .with_context(|| format!("failed to inspect input {}", input_path.display()))?;
    let output_metadata = match fs::metadata(output_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect output {}", output_path.display()));
        }
    };

    if same_file_metadata(&input_metadata, &output_metadata) {
        return Ok(true);
    }

    let input_path = fs::canonicalize(input_path)
        .with_context(|| format!("failed to canonicalize input {}", input_path.display()))?;
    let output_path = fs::canonicalize(output_path)
        .with_context(|| format!("failed to canonicalize output {}", output_path.display()))?;
    Ok(input_path == output_path)
}

#[cfg(unix)]
fn same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_metadata(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

fn resolve_options(cli: &Cli) -> Result<ResolvedOptions> {
    if cli.conversion.initial_sound_law && cli.conversion.no_initial_sound_law {
        bail!("--initial-sound-law and --no-initial-sound-law cannot be used together");
    }

    let mut options = match cli.language.preset {
        Preset::KoKr => ResolvedOptions {
            rendering: RenderOptions::default(),
            engine: EngineOptions {
                initial_sound_law: true,
                numeral_strategy: NumeralStrategy::HangulPhonetic,
                ..EngineOptions::default()
            },
            bundled_stdict: true,
            homophone_window: ContextWindow::PerBlock,
            first_occurrence_window: ContextWindow::Off,
        },
        Preset::KoKp => ResolvedOptions {
            rendering: RenderOptions::default(),
            engine: EngineOptions {
                initial_sound_law: false,
                numeral_strategy: NumeralStrategy::HangulPhonetic,
                ..EngineOptions::default()
            },
            bundled_stdict: false,
            homophone_window: ContextWindow::Off,
            first_occurrence_window: ContextWindow::Off,
        },
    };

    if let Some(rendering) = cli.rendering.rendering {
        options.rendering.mode = rendering.into();
    }
    if let Some(gloss) = cli.rendering.original_gloss {
        if !matches!(options.rendering.mode, RenderMode::Original) {
            bail!("--original-gloss is only valid with --rendering original");
        }
        options.rendering.original_gloss = gloss.into();
    }
    if let Some(disambiguation) = cli.rendering.disambiguation {
        options.homophone_window = disambiguation.into();
    }
    if let Some(first_occurrence) = cli.rendering.first_occurrence {
        options.first_occurrence_window = first_occurrence.into();
    }
    options.engine.segmentation = cli.conversion.segmentation.into();
    if let Some(numerals) = cli.conversion.numerals {
        options.engine.numeral_strategy = numerals.into();
    }
    if cli.language.no_stdict {
        options.bundled_stdict = false;
    }
    if cli.conversion.initial_sound_law {
        options.engine.initial_sound_law = true;
    }
    if cli.conversion.no_initial_sound_law {
        options.engine.initial_sound_law = false;
    }

    Ok(options)
}

fn build_user_directives(cli: &Cli) -> UserDirectives<'static> {
    let mut directives = UserDirectives::new();
    add_literal_directives(
        &mut directives,
        &cli.directives.require_hanja,
        DirectiveAction::RequireHanja,
    );
    add_glob_directives(
        &mut directives,
        &cli.directives.require_hanja_glob,
        DirectiveAction::RequireHanja,
    );
    add_literal_directives(
        &mut directives,
        &cli.directives.require_hangul,
        DirectiveAction::RequireHangul,
    );
    add_glob_directives(
        &mut directives,
        &cli.directives.require_hangul_glob,
        DirectiveAction::RequireHangul,
    );
    add_literal_directives(
        &mut directives,
        &cli.directives.skip_annotation,
        DirectiveAction::SkipAnnotation,
    );
    add_glob_directives(
        &mut directives,
        &cli.directives.skip_annotation_glob,
        DirectiveAction::SkipAnnotation,
    );
    directives
}

fn add_literal_directives(
    directives: &mut UserDirectives<'static>,
    values: &[String],
    action: DirectiveAction,
) {
    for value in values {
        directives.add_literal(value.clone(), action);
    }
}

fn add_glob_directives(
    directives: &mut UserDirectives<'static>,
    patterns: &[String],
    action: DirectiveAction,
) {
    for pattern in patterns {
        let pattern = pattern.clone();
        directives.add_predicate(
            move |annotation| glob_matches(&pattern, &annotation.hanja),
            action,
        );
    }
}

fn build_html_reader_options(
    args: &HtmlArgs,
    format: Format,
) -> Result<HtmlReaderOptions<'static>> {
    if !args.html_preserve_class.is_empty() && !matches!(format, Format::Html) {
        bail!("--html-preserve-class is only valid with --format text/html");
    }
    if !args.html_preserve_attr.is_empty() && !matches!(format, Format::Html) {
        bail!("--html-preserve-attr is only valid with --format text/html");
    }

    if args.html_preserve_class.is_empty() && args.html_preserve_attr.is_empty() {
        return Ok(HtmlReaderOptions::new());
    }

    let classes: Vec<String> = args.html_preserve_class.clone();
    let attrs: Vec<(String, Option<String>)> = args
        .html_preserve_attr
        .iter()
        .map(|spec| match spec.split_once('=') {
            Some((key, value)) => (key.to_owned(), Some(value.to_owned())),
            None => (spec.clone(), None),
        })
        .collect();

    Ok(
        HtmlReaderOptions::new().preserve_when(move |info: &HtmlElementInfo<'_>| -> bool {
            if !classes.is_empty()
                && let Some(class_value) = find_attribute_value(info.raw_attributes, "class")
                && let Some(class_value) = class_value
                && classes
                    .iter()
                    .any(|needle| class_value.split_ascii_whitespace().any(|c| c == needle))
            {
                return true;
            }

            for (key, expected) in &attrs {
                let Some(value) = find_attribute_value(info.raw_attributes, key) else {
                    continue;
                };
                match expected {
                    None => return true,
                    Some(expected) if value.as_deref() == Some(expected.as_str()) => return true,
                    _ => {}
                }
            }

            false
        }),
    )
}

/// Returns the value of the first attribute matching `name` (case-insensitive)
/// in `raw_attributes`.
///
/// The outer `Option` distinguishes attribute absence (`None`) from presence:
/// `Some(None)` is a boolean attribute (no `=`), `Some(Some(value))` carries
/// the decoded value.  The scanner is intentionally narrow — it understands
/// the same attribute shape as [`gukhanmun_html`]'s lang parser but does not
/// touch DOCTYPE quirks or attribute aliases.
fn find_attribute_value(raw_attributes: &str, name: &str) -> Option<Option<String>> {
    let bytes = raw_attributes.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let name_start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'-' | b':' | b'_'))
        {
            index += 1;
        }
        if name_start == index {
            index += 1;
            continue;
        }
        let attribute_name = &raw_attributes[name_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let matched = attribute_name.eq_ignore_ascii_case(name);
        if bytes.get(index) != Some(&b'=') {
            if matched {
                return Some(None);
            }
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let value = if matches!(bytes.get(index), Some(b'\'' | b'"')) {
            let quote = bytes[index];
            index += 1;
            let value_start = index;
            while index < bytes.len() && bytes[index] != quote {
                index += 1;
            }
            let value = &raw_attributes[value_start..index];
            if index < bytes.len() {
                index += 1;
            }
            value
        } else {
            let value_start = index;
            while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            &raw_attributes[value_start..index]
        };
        if matched {
            return Some(Some(decode_html_attribute_value(value)));
        }
    }
    None
}

/// Decodes the common subset of HTML character references that may appear in
/// an attribute value.
///
/// Handles the five named entities mandated by HTML (`&amp;`, `&lt;`, `&gt;`,
/// `&quot;`, `&apos;`) and decimal/hexadecimal numeric character references
/// (`&#NNN;` and `&#xHHH;`).  Any malformed or unknown reference is left
/// verbatim so callers can still match against raw bytes when desired.
fn decode_html_attribute_value(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'&' {
            let next = raw[index..]
                .find('&')
                .map_or(raw.len(), |offset| index + offset);
            output.push_str(&raw[index..next]);
            index = next;
            continue;
        }
        let Some(semi_relative) = raw[index + 1..].find(';') else {
            output.push_str(&raw[index..]);
            break;
        };
        let semi = index + 1 + semi_relative;
        let reference = &raw[index + 1..semi];
        if let Some(ch) = decode_html_entity(reference) {
            output.push(ch);
            index = semi + 1;
        } else {
            output.push_str(&raw[index..=semi]);
            index = semi + 1;
        }
    }
    output
}

fn decode_html_entity(reference: &str) -> Option<char> {
    match reference {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ if reference.starts_with('#') => {
            let digits = &reference[1..];
            let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                digits.parse::<u32>().ok()?
            };
            char::from_u32(code)
        }
        _ => None,
    }
}

fn parse_format(s: &str) -> Result<Format, String> {
    let mut parts = s.split(';');
    let base = parts.next().unwrap_or("").trim();
    match base {
        "text/plain" => Ok(Format::PlainText),
        "text/html" => Ok(Format::Html),
        "text/markdown" => Ok(Format::Markdown(parse_markdown_variant(parts)?)),
        _ => Err(format!(
            "unknown format {base:?}: expected text/plain, text/html, or text/markdown"
        )),
    }
}

fn unquote_mime_param(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn parse_markdown_variant<'a>(
    params: impl Iterator<Item = &'a str>,
) -> Result<MarkdownVariant, String> {
    for param in params {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }
        if let Some((k, v)) = param.split_once('=')
            && k.trim().eq_ignore_ascii_case("variant")
        {
            let v = unquote_mime_param(v.trim());
            return match v {
                v if v.eq_ignore_ascii_case("GFM") => Ok(MarkdownVariant::Gfm),
                v if v.eq_ignore_ascii_case("CommonMark") => Ok(MarkdownVariant::CommonMark),
                v => Err(format!("unknown Markdown variant {v:?}")),
            };
        }
    }
    Ok(MarkdownVariant::CommonMark)
}

fn detect_format(path: &Path) -> Format {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => Format::Html,
        Some("md") | Some("markdown") | Some("mdown") | Some("mkd") => {
            Format::Markdown(MarkdownVariant::CommonMark)
        }
        _ => Format::PlainText,
    }
}

fn convert_document(
    input: impl BufRead,
    output: impl Write,
    dictionary: &CliDictionary,
    options: ResolvedOptions,
    directives: &UserDirectives<'_>,
    html_reader_options: &HtmlReaderOptions<'_>,
    format: Format,
) -> Result<()> {
    match format {
        Format::PlainText => convert_plain_stream(input, output, dictionary, options, directives),
        Format::Html => convert_html(
            input,
            output,
            dictionary,
            options,
            directives,
            html_reader_options,
        ),
        Format::Markdown(variant) => {
            convert_markdown_stream(input, output, dictionary, options, directives, variant)
        }
    }
}

fn convert_plain_stream(
    input: impl BufRead,
    output: impl Write,
    dictionary: &CliDictionary,
    options: ResolvedOptions,
    directives: &UserDirectives<'_>,
) -> Result<()> {
    let engine = Engine::<PlainScopeData, _>::with_options(dictionary, options.engine);
    if options.homophone_window == ContextWindow::Off
        && options.first_occurrence_window == ContextWindow::Off
    {
        return convert_plain_stream_without_homophone_lookahead(
            input,
            output,
            engine,
            options.rendering,
            directives,
        );
    }
    // Plain text has no block or section scopes.  For homophone correctness,
    // PerBlock and PerSection therefore behave like a document-wide window:
    // a later line can force disambiguating hanja on an earlier line, and stdout
    // cannot revise bytes already written.  Only the Off path can stream ready
    // output before EOF without changing rendering semantics.
    convert_plain_stream_with_document_homophone_lookahead(
        input,
        output,
        engine,
        dictionary,
        options,
        directives,
        options.rendering,
    )
}

fn convert_plain_stream_with_document_homophone_lookahead(
    mut input: impl BufRead,
    mut output: impl Write,
    mut engine: Engine<PlainScopeData, CliDictionary>,
    dictionary: &CliDictionary,
    options: ResolvedOptions,
    directives: &UserDirectives<'_>,
    rendering: RenderOptions,
) -> Result<()> {
    let mut output_tokens = Vec::new();
    let mut bytes = [0; 8192];
    let mut pending = Vec::new();

    loop {
        let bytes_read = input
            .read(&mut bytes)
            .context("failed to read input stream")?;
        if bytes_read == 0 {
            break;
        }

        pending.extend_from_slice(&bytes[..bytes_read]);
        process_utf8_prefix(&mut pending, &mut engine, &mut output_tokens)?;
    }
    flush_utf8_tail(&mut pending, &mut engine, &mut output_tokens)?;
    output_tokens.extend(engine.finish());

    let output_tokens = apply_annotation_policy(output_tokens, dictionary, options, directives);
    write_plain_stream_chunk(&mut output, output_tokens, rendering)?;
    output.flush().context("failed to flush output")
}

fn convert_plain_stream_without_homophone_lookahead(
    mut input: impl BufRead,
    mut output: impl Write,
    mut engine: Engine<PlainScopeData, CliDictionary>,
    rendering: RenderOptions,
    directives: &UserDirectives<'_>,
) -> Result<()> {
    let mut bytes = [0; 8192];
    let mut pending = Vec::new();

    loop {
        let bytes_read = input
            .read(&mut bytes)
            .context("failed to read input stream")?;
        if bytes_read == 0 {
            break;
        }

        pending.extend_from_slice(&bytes[..bytes_read]);
        let mut output_tokens = Vec::new();
        process_utf8_prefix_flushing_lines(&mut pending, &mut engine, &mut output_tokens, |_| {
            Ok(())
        })?;
        if !directives.is_empty() {
            output_tokens = apply_user_directives(output_tokens, directives);
        }
        write_plain_stream_chunk(&mut output, output_tokens, rendering)?;
    }
    let mut output_tokens = Vec::new();
    flush_utf8_tail_flushing_lines(&mut pending, &mut engine, &mut output_tokens, |_| Ok(()))?;
    output_tokens.extend(engine.finish());
    if !directives.is_empty() {
        output_tokens = apply_user_directives(output_tokens, directives);
    }
    write_plain_stream_chunk(&mut output, output_tokens, rendering)?;
    output.flush().context("failed to flush output")
}

fn write_plain_stream_chunk(
    output: &mut impl Write,
    output_tokens: Vec<OutputToken<PlainScopeData>>,
    rendering: RenderOptions,
) -> Result<()> {
    if output_tokens.is_empty() {
        return Ok(());
    }

    let converted = write_plain_text(render_tokens_iter(output_tokens, rendering));
    output
        .write_all(converted.as_bytes())
        .context("failed to write output")?;
    output.flush().context("failed to flush output")
}

fn apply_annotation_policy<S>(
    output_tokens: impl IntoIterator<Item = OutputToken<S>>,
    dictionary: &CliDictionary,
    options: ResolvedOptions,
    directives: &UserDirectives<'_>,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
{
    let output_tokens = match options.homophone_window {
        ContextWindow::Off => output_tokens.into_iter().collect(),
        window => mark_homophones(output_tokens, dictionary, window),
    };
    let output_tokens = match options.first_occurrence_window {
        ContextWindow::Off => output_tokens,
        window => filter_first_occurrences(output_tokens, window),
    };
    if directives.is_empty() {
        output_tokens
    } else {
        apply_user_directives(output_tokens, directives)
    }
}

fn process_utf8_prefix(
    pending: &mut Vec<u8>,
    engine: &mut Engine<PlainScopeData, CliDictionary>,
    output: &mut Vec<gukhanmun_core::OutputToken<PlainScopeData>>,
) -> Result<()> {
    process_utf8_prefix_with(pending, |text| {
        output.extend(engine.push_token(InputToken::Text(text.to_owned())));
        Ok(())
    })
}

fn process_utf8_prefix_flushing_lines(
    pending: &mut Vec<u8>,
    engine: &mut Engine<PlainScopeData, CliDictionary>,
    output: &mut Vec<gukhanmun_core::OutputToken<PlainScopeData>>,
    mut on_completed_line: impl FnMut(&mut Vec<OutputToken<PlainScopeData>>) -> Result<()>,
) -> Result<()> {
    process_utf8_prefix_with(pending, |text| {
        push_plain_text_flushing_lines(text, engine, output, &mut on_completed_line)
    })
}

fn process_utf8_prefix_with(
    pending: &mut Vec<u8>,
    mut push_text: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    match std::str::from_utf8(pending) {
        Ok(text) => {
            push_text(text)?;
            pending.clear();
            Ok(())
        }
        Err(error) if error.error_len().is_none() => {
            let valid_up_to = error.valid_up_to();
            if valid_up_to > 0 {
                let text = std::str::from_utf8(&pending[..valid_up_to])
                    .expect("valid_up_to marks valid UTF-8");
                push_text(text)?;
                pending.drain(..valid_up_to);
            }
            Ok(())
        }
        Err(_) => bail!("failed to read UTF-8 input"),
    }
}

fn flush_utf8_tail(
    pending: &mut Vec<u8>,
    engine: &mut Engine<PlainScopeData, CliDictionary>,
    output: &mut Vec<gukhanmun_core::OutputToken<PlainScopeData>>,
) -> Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let text =
        std::str::from_utf8(pending).map_err(|_| anyhow::anyhow!("failed to read UTF-8 input"))?;
    output.extend(engine.push_token(InputToken::Text(text.to_owned())));
    pending.clear();
    Ok(())
}

fn flush_utf8_tail_flushing_lines(
    pending: &mut Vec<u8>,
    engine: &mut Engine<PlainScopeData, CliDictionary>,
    output: &mut Vec<gukhanmun_core::OutputToken<PlainScopeData>>,
    mut on_completed_line: impl FnMut(&mut Vec<OutputToken<PlainScopeData>>) -> Result<()>,
) -> Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let text =
        std::str::from_utf8(pending).map_err(|_| anyhow::anyhow!("failed to read UTF-8 input"))?;
    push_plain_text_flushing_lines(text, engine, output, &mut on_completed_line)?;
    pending.clear();
    Ok(())
}

fn push_plain_text_flushing_lines<F>(
    text: &str,
    engine: &mut Engine<PlainScopeData, CliDictionary>,
    output: &mut Vec<gukhanmun_core::OutputToken<PlainScopeData>>,
    on_completed_line: &mut F,
) -> Result<()>
where
    F: FnMut(&mut Vec<OutputToken<PlainScopeData>>) -> Result<()>,
{
    let mut start = 0;
    for (index, ch) in text.char_indices() {
        if ch != '\n' {
            continue;
        }

        let end = index + ch.len_utf8();
        output.extend(engine.push_token(InputToken::Text(text[start..end].to_owned())));
        output.extend(engine.flush());
        on_completed_line(output)?;
        start = end;
    }

    if start < text.len() {
        output.extend(engine.push_token(InputToken::Text(text[start..].to_owned())));
    }

    Ok(())
}

fn convert_html(
    mut input: impl BufRead,
    mut output: impl Write,
    dictionary: &CliDictionary,
    options: ResolvedOptions,
    directives: &UserDirectives<'_>,
    html_reader_options: &HtmlReaderOptions<'_>,
) -> Result<()> {
    let mut content = String::new();
    input
        .read_to_string(&mut content)
        .context("failed to read UTF-8 input")?;
    let input_tokens = read_html_fragment_with_options(&content, html_reader_options);
    let output_tokens = process_tokens_iter_with_options(input_tokens, dictionary, options.engine);
    let output_tokens = apply_annotation_policy(output_tokens, dictionary, options, directives);
    let rendered_tokens = render_tokens_iter(output_tokens, options.rendering);
    let converted = write_html_fragment(rendered_tokens);
    output
        .write_all(converted.as_bytes())
        .context("failed to write output")?;
    output.flush().context("failed to flush output")
}

fn markdown_format_options() -> hongdown::Options {
    hongdown::Options {
        curly_double_quotes: false,
        curly_single_quotes: false,
        curly_apostrophes: false,
        ellipsis: false,
        em_dash: hongdown::DashSetting::Disabled,
        en_dash: hongdown::DashSetting::Disabled,
        ..Default::default()
    }
}

fn convert_markdown_stream(
    mut input: impl BufRead,
    mut output: impl Write,
    dictionary: &CliDictionary,
    options: ResolvedOptions,
    directives: &UserDirectives<'_>,
    variant: MarkdownVariant,
) -> Result<()> {
    let mut content = String::new();
    input
        .read_to_string(&mut content)
        .context("failed to read UTF-8 input")?;
    let input_tokens = read_markdown(&content, variant);
    let output_tokens = process_tokens_iter_with_options(input_tokens, dictionary, options.engine);
    let output_tokens = apply_annotation_policy(output_tokens, dictionary, options, directives);
    let rendered_tokens = render_tokens_iter(output_tokens, options.rendering);
    let converted =
        write_markdown(rendered_tokens).context("failed to serialize Markdown output")?;
    let converted = hongdown::format(&converted, &markdown_format_options())
        .context("failed to format Markdown output")?;
    output
        .write_all(converted.as_bytes())
        .context("failed to write output")?;
    output.flush().context("failed to flush output")
}

impl From<Rendering> for RenderMode {
    fn from(rendering: Rendering) -> Self {
        match rendering {
            Rendering::HangulOnly => Self::HangulOnly,
            Rendering::HangulHanjaParens => Self::HangulHanjaParens,
            Rendering::HanjaHangulParens => Self::HanjaHangulParens,
            Rendering::RubyOnHangul => Self::Ruby(RubyBase::OnHangul),
            Rendering::RubyOnHanja => Self::Ruby(RubyBase::OnHanja),
            Rendering::Original => Self::Original,
        }
    }
}

impl From<OriginalGlossArg> for OriginalGloss {
    fn from(arg: OriginalGlossArg) -> Self {
        match arg {
            OriginalGlossArg::Parens => Self::Parens,
            OriginalGlossArg::Ruby => Self::Ruby,
        }
    }
}

impl From<Segmentation> for SegmentationStrategy {
    fn from(segmentation: Segmentation) -> Self {
        match segmentation {
            Segmentation::Lattice => Self::Lattice,
            Segmentation::Eager => Self::Eager,
        }
    }
}

impl From<Numerals> for NumeralStrategy {
    fn from(numerals: Numerals) -> Self {
        match numerals {
            Numerals::HangulPhonetic => Self::HangulPhonetic,
            Numerals::PositionalArabic => Self::PositionalArabic,
            Numerals::AdditiveArabic => Self::AdditiveArabic,
            Numerals::Smart => Self::Smart,
        }
    }
}

impl From<CliContextWindow> for ContextWindow {
    fn from(window: CliContextWindow) -> Self {
        match window {
            CliContextWindow::Off => Self::Off,
            CliContextWindow::PerBlock => Self::PerBlock,
            CliContextWindow::PerSection => Self::PerSection,
            CliContextWindow::PerDocument => Self::PerDocument,
        }
    }
}

fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let text = text.chars().collect::<Vec<_>>();
    let mut matches = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    matches[pattern.len()][text.len()] = true;

    for pattern_index in (0..pattern.len()).rev() {
        for text_index in (0..=text.len()).rev() {
            matches[pattern_index][text_index] = match pattern[pattern_index] {
                '*' => {
                    matches[pattern_index + 1][text_index]
                        || (text_index < text.len() && matches[pattern_index][text_index + 1])
                }
                '?' => text_index < text.len() && matches[pattern_index + 1][text_index + 1],
                character => {
                    text_index < text.len()
                        && character == text[text_index]
                        && matches[pattern_index + 1][text_index + 1]
                }
            };
        }
    }

    matches[0][0]
}

fn load_dictionary(user_paths: &[PathBuf], bundled_stdict: bool) -> Result<CliDictionary> {
    let mut dictionaries = ChainDictionary::new();

    for path in user_paths.iter().rev() {
        dictionaries.push(open_user_dictionary(path)?);
    }
    if bundled_stdict {
        dictionaries.push(Box::new(gukhanmun_stdict::ko_kr()));
    }

    Ok(dictionaries)
}

fn open_user_dictionary(path: &Path) -> Result<Box<dyn HanjaDictionary>> {
    if has_fst_magic(path)? {
        return Ok(Box::new(FstDictionary::open(path).with_context(|| {
            format!("failed to load dictionary {}", path.display())
        })?));
    }

    Ok(Box::new(CdbDictionary::open(path).with_context(|| {
        format!("failed to load dictionary {}", path.display())
    })?))
}

fn has_fst_magic(path: &Path) -> Result<bool> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("failed to load dictionary {}", path.display()))?;
    let mut header = [0; 8];
    let bytes_read = file
        .read(&mut header)
        .with_context(|| format!("failed to load dictionary {}", path.display()))?;
    Ok(bytes_read == FST_MAGIC.len() && &header == FST_MAGIC)
}

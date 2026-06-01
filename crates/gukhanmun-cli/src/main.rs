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
use gukhanmun::cdb::CdbDictionary;
use gukhanmun::fst::FstDictionary;
use gukhanmun::html::{HtmlElementInfo, HtmlFragmentReader, HtmlFragmentWriter, HtmlScopeData};
use gukhanmun::markdown::MarkdownVariant;
use gukhanmun::{
    Builder, ContextWindow, DirectiveAction, Engine, FirstOccurrenceFilter, HanjaDictionary,
    HomophoneDetection, HomophoneMarker, InputToken, NumeralStrategy, OriginalGloss, OutputToken,
    PlainScopeData, Preset as UmbrellaPreset, RecoverableInputError, Recovery,
    RedundantParenCollapser, RenderMode, RenderOptions, RenderedToken, Renderer, RubyBase,
    ScopeData, SegmentationStrategy, UserDirectives, apply_user_directives, recover_input_token,
    render_tokens_iter, write_plain_text,
};
use tracing_subscriber::EnvFilter;

const FST_MAGIC: &[u8; 8] = b"GUKHMFST";

#[derive(Debug, Parser)]
#[command(
    name = "gukhanmun",
    version,
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

    #[command(flatten)]
    markdown: MarkdownArgs,

    /// Enable debug-level logging to stderr.  Equivalent to RUST_LOG=debug
    /// when RUST_LOG is not already set.  Use RUST_LOG for finer control
    /// (e.g. RUST_LOG=trace).
    #[arg(short, long)]
    verbose: bool,
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

    /// Reader error recovery policy.  strict (default) stops at recoverable
    /// reader errors.  lenient preserves recoverable bad regions and continues;
    /// currently this is meaningful for malformed HTML fragments.
    #[arg(long, value_enum, default_value_t = RecoveryArg::Strict)]
    recovery: RecoveryArg,

    /// Enable the initial sound law (頭音法則), overriding the preset default.
    #[arg(short = 'i', long, visible_alias = "dueum")]
    initial_sound_law: bool,

    /// Disable the initial sound law (頭音法則), overriding the preset default.
    #[arg(short = 'I', long, visible_alias = "no-dueum")]
    no_initial_sound_law: bool,

    /// Keep redundant parenthetical reading annotations instead of collapsing
    /// them.  By default an explicit gloss such as 庫間(곳간) or 곳간(庫間) is
    /// collapsed to show both scripts once; this flag leaves the input as is.
    #[arg(long)]
    no_collapse_parens: bool,
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

    /// Homophone detection strategy.  context-local glosses a reading only when
    /// a different-meaning homophone appears within the disambiguation window;
    /// dictionary-wide also glosses readings shared by other dictionary entries.
    #[arg(long = "homophone-detection", value_enum)]
    homophone_detection: Option<CliHomophoneDetection>,

    /// Context for clearing repeated dictionary presentation requirements.
    /// off leaves every occurrence as marked by dictionaries and directives.
    #[arg(long, value_enum)]
    first_occurrence: Option<CliContextWindow>,
}

#[derive(Debug, Args)]
#[command(next_help_heading = "User directives")]
struct DirectiveArgs {
    /// Path to a UTF-8 TSV directive file.  The header must be
    /// `action<TAB>pattern<TAB>kind`; actions are require-hanja,
    /// require-hangul, or skip-annotation, and kind is literal or glob.  May be
    /// repeated.
    #[arg(long = "directives", value_name = "PATH")]
    directive_files: Vec<PathBuf>,

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

#[derive(Debug, Args)]
#[command(next_help_heading = "Markdown")]
struct MarkdownArgs {
    /// Also convert selected YAML front matter values, addressed by a JSONPath
    /// expression (for example `$.hero.tagline` or `$.hero.actions[*].text`).
    /// Each matched string scalar is converted from mixed script to hangul;
    /// non-string matches are left untouched.  May be repeated.  The whole
    /// front matter block is reformatted on output, but only matched values
    /// change.  When given without a leading YAML front matter block, a warning
    /// is logged and only the Markdown body is converted.  Only valid with
    /// `--format text/markdown`.
    #[arg(long = "markdown-frontmatter-convert", value_name = "JSONPATH")]
    frontmatter_convert: Vec<String>,
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
enum RecoveryArg {
    Strict,
    Lenient,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliContextWindow {
    Off,
    PerBlock,
    PerSection,
    PerDocument,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliHomophoneDetection {
    ContextLocal,
    DictionaryWide,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let default_level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_writer(io::stderr)
        .init();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    let format = cli.io.format.unwrap_or_else(|| {
        cli.io
            .input
            .as_deref()
            .map(detect_format)
            .unwrap_or(Format::PlainText)
    });
    if !cli.markdown.frontmatter_convert.is_empty() && !matches!(format, Format::Markdown(_)) {
        bail!("--markdown-frontmatter-convert is only valid with --format text/markdown");
    }
    let converter = build_converter(&cli, format)?;
    let frontmatter_selectors = cli.markdown.frontmatter_convert.as_slice();

    if let (Some(input_path), Some(output_path)) = (&cli.io.input, &cli.io.output)
        && is_same_existing_file(input_path, output_path)?
    {
        return convert_file_in_place(
            input_path,
            output_path,
            &converter,
            format,
            frontmatter_selectors,
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

    convert_document(input, output, &converter, format, frontmatter_selectors)
}

fn convert_file_in_place(
    input_path: &Path,
    output_path: &Path,
    converter: &gukhanmun::Converter<'_>,
    format: Format,
    frontmatter_selectors: &[String],
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
        convert_document(input, output, converter, format, frontmatter_selectors)?;
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

impl From<Preset> for UmbrellaPreset {
    fn from(preset: Preset) -> Self {
        match preset {
            Preset::KoKr => UmbrellaPreset::KoKr,
            Preset::KoKp => UmbrellaPreset::KoKp,
        }
    }
}

fn build_converter(cli: &Cli, format: Format) -> Result<gukhanmun::Converter<'static>> {
    if cli.conversion.initial_sound_law && cli.conversion.no_initial_sound_law {
        bail!("--initial-sound-law and --no-initial-sound-law cannot be used together");
    }

    let mut builder =
        Builder::with_preset(cli.language.preset.into()).recovery(cli.conversion.recovery.into());

    if let Some(rendering) = cli.rendering.rendering {
        let mode: RenderMode = rendering.into();
        let mut render_options = RenderOptions {
            mode,
            ..RenderOptions::default()
        };
        if let Some(gloss) = cli.rendering.original_gloss {
            if !matches!(mode, RenderMode::Original) {
                bail!("--original-gloss is only valid with --rendering original");
            }
            render_options.original_gloss = gloss.into();
        }
        builder = builder.rendering(render_options);
    } else if cli.rendering.original_gloss.is_some() {
        bail!("--original-gloss is only valid with --rendering original");
    }

    if let Some(disambiguation) = cli.rendering.disambiguation {
        builder = builder.homophone_window(disambiguation.into());
    }
    if let Some(detection) = cli.rendering.homophone_detection {
        builder = builder.homophone_detection(detection.into());
    }
    if let Some(first_occurrence) = cli.rendering.first_occurrence {
        builder = builder.first_occurrence_window(first_occurrence.into());
    }
    builder = builder.segmentation(cli.conversion.segmentation.into());
    if let Some(numerals) = cli.conversion.numerals {
        builder = builder.numerals(numerals.into());
    }
    if cli.conversion.initial_sound_law {
        builder = builder.initial_sound_law(true);
    }
    if cli.conversion.no_initial_sound_law {
        builder = builder.initial_sound_law(false);
    }
    if cli.conversion.no_collapse_parens {
        builder = builder.collapse_redundant_parens(false);
    }
    if cli.language.no_stdict {
        builder = builder.no_bundled_stdict();
    }

    for path in cli.language.dictionaries.iter().rev() {
        builder = builder.push_boxed_dictionary(open_user_dictionary(path)?);
    }

    builder = builder.directives(build_user_directives(cli)?);

    if let Some(predicate) = build_html_preserve_predicate(&cli.html, format)? {
        builder = builder.html_preserve_when(predicate);
    }

    builder
        .build()
        .map_err(|error| anyhow::anyhow!("failed to assemble converter: {error}"))
}

fn build_user_directives(cli: &Cli) -> Result<UserDirectives<'static>> {
    let mut directives = UserDirectives::new();
    for path in &cli.directives.directive_files {
        add_directives_file(&mut directives, path)?;
    }
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
    Ok(directives)
}

fn add_directives_file(directives: &mut UserDirectives<'static>, path: &Path) -> Result<()> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open directives {}", path.display()))?;
    let mut lines = BufReader::new(file).lines();
    let header = lines
        .next()
        .transpose()
        .with_context(|| format!("failed to read directives {}", path.display()))?
        .ok_or_else(|| anyhow::anyhow!("{}: directives file is empty", path.display()))?;
    if header != "action\tpattern\tkind" {
        bail!(
            "{}:1: expected directives TSV header `action<TAB>pattern<TAB>kind`",
            path.display()
        );
    }

    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let line = line.with_context(|| format!("failed to read directives {}", path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        add_directive_file_row(directives, path, line_number, &line)?;
    }

    Ok(())
}

fn add_directive_file_row(
    directives: &mut UserDirectives<'static>,
    path: &Path,
    line_number: usize,
    line: &str,
) -> Result<()> {
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() != 3 {
        bail!(
            "{}:{line_number}: expected 3 TSV fields, got {}",
            path.display(),
            fields.len()
        );
    }

    let action = parse_directive_action(fields[0], path, line_number)?;
    let pattern = fields[1];
    if pattern.is_empty() {
        bail!(
            "{}:{line_number}: `pattern` must not be empty",
            path.display()
        );
    }
    match fields[2] {
        "literal" => directives.add_literal(pattern.to_owned(), action),
        "glob" => {
            let pattern = pattern.to_owned();
            directives.add_predicate(
                move |annotation| glob_matches(&pattern, &annotation.hanja),
                action,
            );
        }
        kind => bail!(
            "{}:{line_number}: unknown directive kind `{kind}`; expected `literal` or `glob`",
            path.display()
        ),
    }

    Ok(())
}

fn parse_directive_action(
    action: &str,
    path: &Path,
    line_number: usize,
) -> Result<DirectiveAction> {
    match action {
        "require-hanja" => Ok(DirectiveAction::RequireHanja),
        "require-hangul" => Ok(DirectiveAction::RequireHangul),
        "skip-annotation" => Ok(DirectiveAction::SkipAnnotation),
        action => bail!(
            "{}:{line_number}: unknown directive action `{action}`; expected `require-hanja`, `require-hangul`, or `skip-annotation`",
            path.display()
        ),
    }
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

type PreservePredicate = Box<dyn Fn(&HtmlElementInfo<'_>) -> bool + 'static>;

fn build_html_preserve_predicate(
    args: &HtmlArgs,
    format: Format,
) -> Result<Option<PreservePredicate>> {
    if !args.html_preserve_class.is_empty() && !matches!(format, Format::Html) {
        bail!("--html-preserve-class is only valid with --format text/html");
    }
    if !args.html_preserve_attr.is_empty() && !matches!(format, Format::Html) {
        bail!("--html-preserve-attr is only valid with --format text/html");
    }

    if args.html_preserve_class.is_empty() && args.html_preserve_attr.is_empty() {
        return Ok(None);
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

    let predicate: PreservePredicate = Box::new(move |info: &HtmlElementInfo<'_>| -> bool {
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
    });

    Ok(Some(predicate))
}

/// Returns the value of the first attribute matching `name` (case-insensitive)
/// in `raw_attributes`.
///
/// The outer `Option` distinguishes attribute absence (`None`) from presence:
/// `Some(None)` is a boolean attribute (no `=`), `Some(Some(value))` carries
/// the decoded value.  The scanner is intentionally narrow—it understands
/// the same attribute shape as [`gukhanmun::html`]'s lang parser but does not
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
    converter: &gukhanmun::Converter<'_>,
    format: Format,
    frontmatter_selectors: &[String],
) -> Result<()> {
    match format {
        Format::PlainText => convert_plain_document(input, output, converter),
        Format::Html => convert_html_document(input, output, converter),
        Format::Markdown(variant) => {
            convert_markdown_document(input, output, converter, variant, frontmatter_selectors)
        }
    }
}

fn convert_plain_document(
    input: impl BufRead,
    output: impl Write,
    converter: &gukhanmun::Converter<'_>,
) -> Result<()> {
    let options = converter.options();
    if options.homophone_window == ContextWindow::Off
        && options.first_occurrence_window == ContextWindow::Off
    {
        let engine =
            Engine::<PlainScopeData, _>::with_options(converter.dictionary(), options.engine);
        return convert_plain_stream_without_homophone_lookahead(
            input,
            output,
            engine,
            options.rendering,
            converter.directives(),
            options.collapse_redundant_parens,
        );
    }
    // Plain text has no block or section scopes.  For homophone correctness,
    // PerBlock and PerSection therefore behave like a document-wide window:
    // a later line can force disambiguating hanja on an earlier line, and stdout
    // cannot revise bytes already written.  Only the Off path can stream ready
    // output before EOF without changing rendering semantics.
    convert_plain_document_buffered(input, output, converter)
}

fn convert_plain_document_buffered(
    mut input: impl BufRead,
    mut output: impl Write,
    converter: &gukhanmun::Converter<'_>,
) -> Result<()> {
    let mut content = String::new();
    input
        .read_to_string(&mut content)
        .context("failed to read UTF-8 input")?;
    let converted = converter
        .convert_text_to_string(&content)
        .map_err(|error| anyhow::anyhow!("failed to convert plain text: {error}"))?;
    output
        .write_all(converted.as_bytes())
        .context("failed to write output")?;
    output.flush().context("failed to flush output")
}

fn convert_plain_stream_without_homophone_lookahead<D>(
    mut input: impl BufRead,
    mut output: impl Write,
    mut engine: Engine<PlainScopeData, D>,
    rendering: RenderOptions,
    directives: &UserDirectives<'_>,
    collapse_parens: bool,
) -> Result<()>
where
    D: HanjaDictionary + ?Sized,
{
    // The collapser runs immediately after the engine, mirroring the umbrella
    // pipeline order; with no homophone/first-occurrence lookahead it is the
    // only middleware between the engine and the renderer.
    let mut collapser = RedundantParenCollapser::<PlainScopeData>::new(collapse_parens);
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
        let mut output_tokens = run_collapser(&mut collapser, output_tokens);
        if !directives.is_empty() {
            output_tokens = apply_user_directives(output_tokens, directives);
        }
        write_plain_stream_chunk(&mut output, output_tokens, rendering)?;
    }
    let mut output_tokens = Vec::new();
    flush_utf8_tail_flushing_lines(&mut pending, &mut engine, &mut output_tokens, |_| Ok(()))?;
    output_tokens.extend(engine.finish());
    let mut output_tokens = run_collapser(&mut collapser, output_tokens);
    output_tokens.extend(collapser.finish());
    if !directives.is_empty() {
        output_tokens = apply_user_directives(output_tokens, directives);
    }
    write_plain_stream_chunk(&mut output, output_tokens, rendering)?;
    output.flush().context("failed to flush output")
}

/// Feeds a batch of engine output tokens through a [`RedundantParenCollapser`],
/// returning whatever it releases.  The collapser keeps its cross-batch state,
/// so its [`RedundantParenCollapser::finish`] must still be drained at EOF.
fn run_collapser<S>(
    collapser: &mut RedundantParenCollapser<S>,
    tokens: Vec<OutputToken<S>>,
) -> Vec<OutputToken<S>>
where
    S: ScopeData,
{
    let mut collapsed = Vec::new();
    for token in tokens {
        collapsed.extend(collapser.push_token(token));
    }
    collapsed
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

fn process_utf8_prefix_flushing_lines<D>(
    pending: &mut Vec<u8>,
    engine: &mut Engine<PlainScopeData, D>,
    output: &mut Vec<OutputToken<PlainScopeData>>,
    mut on_completed_line: impl FnMut(&mut Vec<OutputToken<PlainScopeData>>) -> Result<()>,
) -> Result<()>
where
    D: HanjaDictionary + ?Sized,
{
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

fn flush_utf8_tail_flushing_lines<D>(
    pending: &mut Vec<u8>,
    engine: &mut Engine<PlainScopeData, D>,
    output: &mut Vec<OutputToken<PlainScopeData>>,
    mut on_completed_line: impl FnMut(&mut Vec<OutputToken<PlainScopeData>>) -> Result<()>,
) -> Result<()>
where
    D: HanjaDictionary + ?Sized,
{
    if pending.is_empty() {
        return Ok(());
    }
    let text =
        std::str::from_utf8(pending).map_err(|_| anyhow::anyhow!("failed to read UTF-8 input"))?;
    push_plain_text_flushing_lines(text, engine, output, &mut on_completed_line)?;
    pending.clear();
    Ok(())
}

fn push_plain_text_flushing_lines<D, F>(
    text: &str,
    engine: &mut Engine<PlainScopeData, D>,
    output: &mut Vec<OutputToken<PlainScopeData>>,
    on_completed_line: &mut F,
) -> Result<()>
where
    D: HanjaDictionary + ?Sized,
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

fn convert_html_document(
    mut input: impl BufRead,
    output: impl Write,
    converter: &gukhanmun::Converter<'_>,
) -> Result<()> {
    let options = converter.options();
    let mut reader = HtmlFragmentReader::with_options(converter.html_reader_options());
    let mut engine =
        Engine::<HtmlScopeData, _>::with_options(converter.dictionary(), options.engine);
    let mut collapser =
        RedundantParenCollapser::<HtmlScopeData>::new(options.collapse_redundant_parens);
    let mut homophones = HomophoneMarker::with_detection(
        converter.dictionary(),
        options.homophone_window,
        options.homophone_detection,
    );
    let mut first_occurrences = FirstOccurrenceFilter::new(options.first_occurrence_window);
    let mut renderer = Renderer::new(options.rendering);
    let mut writer = HtmlFragmentWriter::new(output);
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
        {
            let mut pipeline = HtmlStreamPipeline {
                engine: &mut engine,
                collapser: &mut collapser,
                homophones: &mut homophones,
                first_occurrences: &mut first_occurrences,
                directives: converter.directives(),
                renderer: &mut renderer,
                writer: &mut writer,
                recovery: options.recovery,
            };
            process_utf8_prefix_with(&mut pending, |text| {
                pipeline.process_input_tokens(reader.push_str(text))
            })?;
        }
        writer.flush().context("failed to flush output")?;
    }

    {
        let mut pipeline = HtmlStreamPipeline {
            engine: &mut engine,
            collapser: &mut collapser,
            homophones: &mut homophones,
            first_occurrences: &mut first_occurrences,
            directives: converter.directives(),
            renderer: &mut renderer,
            writer: &mut writer,
            recovery: options.recovery,
        };
        flush_utf8_tail_with(&mut pending, |text| {
            pipeline.process_input_tokens(reader.push_str(text))
        })?;
    }
    writer.flush().context("failed to flush output")?;
    {
        let mut pipeline = HtmlStreamPipeline {
            engine: &mut engine,
            collapser: &mut collapser,
            homophones: &mut homophones,
            first_occurrences: &mut first_occurrences,
            directives: converter.directives(),
            renderer: &mut renderer,
            writer: &mut writer,
            recovery: options.recovery,
        };
        pipeline.process_input_tokens(reader.finish())?;
    }
    let engine_tail = engine.finish();
    let collapsed_engine_tail = run_collapser(&mut collapser, engine_tail);
    process_html_output_tokens(
        collapsed_engine_tail,
        &mut homophones,
        &mut first_occurrences,
        converter.directives(),
        &mut renderer,
        &mut writer,
    )?;
    let collapser_tail = collapser.finish();
    process_html_output_tokens(
        collapser_tail,
        &mut homophones,
        &mut first_occurrences,
        converter.directives(),
        &mut renderer,
        &mut writer,
    )?;
    let homophone_tail = homophones.finish();
    process_html_first_occurrence_tokens(
        homophone_tail,
        &mut first_occurrences,
        converter.directives(),
        &mut renderer,
        &mut writer,
    )?;
    let first_occurrence_tail = first_occurrences.finish();
    write_html_rendered_tokens(
        first_occurrence_tail,
        converter.directives(),
        &mut renderer,
        &mut writer,
    )?;
    writer.finish().context("failed to flush output")?;
    Ok(())
}

struct HtmlStreamPipeline<'p, 'd, D, W>
where
    D: HanjaDictionary + ?Sized,
    W: Write,
{
    engine: &'p mut Engine<'d, HtmlScopeData, D>,
    collapser: &'p mut RedundantParenCollapser<HtmlScopeData>,
    homophones: &'p mut HomophoneMarker<'d, HtmlScopeData>,
    first_occurrences: &'p mut FirstOccurrenceFilter<HtmlScopeData>,
    directives: &'p UserDirectives<'d>,
    renderer: &'p mut Renderer<HtmlScopeData>,
    writer: &'p mut HtmlFragmentWriter<W>,
    recovery: Recovery,
}

impl<D, W> HtmlStreamPipeline<'_, '_, D, W>
where
    D: HanjaDictionary + ?Sized,
    W: Write,
{
    fn process_input_tokens(
        &mut self,
        input_tokens: Vec<Result<InputToken<HtmlScopeData>, RecoverableInputError>>,
    ) -> Result<()> {
        for token in input_tokens {
            let token = recover_input_token(token, self.recovery)
                .map_err(|error| anyhow::anyhow!("failed to convert HTML fragment: {error}"))?;
            let output_tokens = self.engine.push_token(token);
            let collapsed = run_collapser(self.collapser, output_tokens);
            process_html_output_tokens(
                collapsed,
                self.homophones,
                self.first_occurrences,
                self.directives,
                self.renderer,
                self.writer,
            )?;
        }
        Ok(())
    }
}

fn flush_utf8_tail_with(
    pending: &mut Vec<u8>,
    mut push_text: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let text =
        std::str::from_utf8(pending).map_err(|_| anyhow::anyhow!("failed to read UTF-8 input"))?;
    push_text(text)?;
    pending.clear();
    Ok(())
}

fn process_html_output_tokens<W>(
    output_tokens: Vec<OutputToken<HtmlScopeData>>,
    homophones: &mut HomophoneMarker<'_, HtmlScopeData>,
    first_occurrences: &mut FirstOccurrenceFilter<HtmlScopeData>,
    directives: &UserDirectives<'_>,
    renderer: &mut Renderer<HtmlScopeData>,
    writer: &mut HtmlFragmentWriter<W>,
) -> Result<()>
where
    W: Write,
{
    for token in output_tokens {
        let tokens = homophones.push_token(token);
        process_html_first_occurrence_tokens(
            tokens,
            first_occurrences,
            directives,
            renderer,
            writer,
        )?;
    }
    Ok(())
}

fn process_html_first_occurrence_tokens<W>(
    output_tokens: Vec<OutputToken<HtmlScopeData>>,
    first_occurrences: &mut FirstOccurrenceFilter<HtmlScopeData>,
    directives: &UserDirectives<'_>,
    renderer: &mut Renderer<HtmlScopeData>,
    writer: &mut HtmlFragmentWriter<W>,
) -> Result<()>
where
    W: Write,
{
    for token in output_tokens {
        let tokens = first_occurrences.push_token(token);
        write_html_rendered_tokens(tokens, directives, renderer, writer)?;
    }
    Ok(())
}

fn write_html_rendered_tokens<W>(
    output_tokens: Vec<OutputToken<HtmlScopeData>>,
    directives: &UserDirectives<'_>,
    renderer: &mut Renderer<HtmlScopeData>,
    writer: &mut HtmlFragmentWriter<W>,
) -> Result<()>
where
    W: Write,
{
    for token in output_tokens {
        let rendered: RenderedToken<_> = renderer.push_token(directives.apply(token));
        writer
            .write_token(rendered)
            .context("failed to write output")?;
    }
    Ok(())
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

fn convert_markdown_document(
    mut input: impl BufRead,
    mut output: impl Write,
    converter: &gukhanmun::Converter<'_>,
    variant: MarkdownVariant,
    frontmatter_selectors: &[String],
) -> Result<()> {
    let mut content = String::new();
    input
        .read_to_string(&mut content)
        .context("failed to read UTF-8 input")?;

    // A leading YAML front matter block is always split off so it is never
    // mangled by the Markdown converter.  By default it passes through verbatim;
    // it is only parsed and (selectively) converted when JSONPath selectors are
    // supplied.
    let (front_matter, body) = match split_front_matter(&content) {
        Some((raw, inner, body)) => (Some((raw, inner)), body),
        None => {
            if !frontmatter_selectors.is_empty() {
                tracing::warn!(
                    "--markdown-frontmatter-convert was given but the input has no YAML front \
                     matter; converting the Markdown body only"
                );
            }
            (None, content.as_str())
        }
    };

    // hongdown must format the body alone: it would otherwise treat the `---`
    // front matter fences as thematic breaks and reflow the YAML block.
    let converted_body = converter
        .convert_markdown_to_string(body, variant)
        .map_err(|error| anyhow::anyhow!("failed to convert Markdown: {error}"))?;
    let converted_body = hongdown::format(&converted_body, &markdown_format_options())
        .context("failed to format Markdown output")?;

    if let Some((raw, inner)) = front_matter {
        if frontmatter_selectors.is_empty() {
            // No selectors: preserve the original front matter byte-for-byte.
            output
                .write_all(raw.as_bytes())
                .context("failed to write output")?;
        } else {
            let converted_front_matter =
                convert_front_matter(inner, frontmatter_selectors, converter)?;
            output
                .write_all(b"---\n")
                .context("failed to write output")?;
            output
                .write_all(converted_front_matter.as_bytes())
                .context("failed to write output")?;
            output
                .write_all(b"---\n")
                .context("failed to write output")?;
        }
    }
    output
        .write_all(converted_body.as_bytes())
        .context("failed to write output")?;
    output.flush().context("failed to flush output")
}

/// Splits a leading YAML front matter block from a Markdown document.
///
/// Returns `Some((raw, inner, body))` when `content` begins with a line that is
/// exactly `---` (trailing whitespace and an optional leading UTF-8 BOM are
/// tolerated) and a later line is exactly `---` or `...`.  `raw` is the whole
/// original block including any leading BOM and both delimiter lines (so
/// passthrough stays byte-for-byte), `inner` is the text between the fences (for
/// parsing), and `body` is the remainder after the closing fence.  `raw` and
/// `body` together reconstruct `content` exactly.  Returns `None` when no such
/// block is present, so a leading `---` without a closing fence stays ordinary
/// Markdown.
fn split_front_matter(content: &str) -> Option<(&str, &str, &str)> {
    // Tolerate a leading BOM when detecting the opening fence, but keep it in
    // `raw` so the byte-for-byte passthrough does not silently drop it.
    let bom_len = if content.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let after_bom = &content[bom_len..];
    let first_newline = after_bom.find('\n')?;
    if after_bom[..first_newline].trim_end() != "---" {
        return None;
    }
    let after_open = bom_len + first_newline + 1;
    let rest = &content[after_open..];

    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        if matches!(trimmed.trim_end(), "---" | "...") {
            let close_end = offset + line.len();
            let raw = &content[..after_open + close_end];
            let inner = &rest[..offset];
            let body = &rest[close_end..];
            return Some((raw, inner, body));
        }
        offset += line.len();
    }
    None
}

/// Converts the YAML front matter values addressed by the given JSONPath
/// `selectors`, returning the re-serialised YAML (always newline-terminated).
///
/// Each matched string scalar is converted from mixed script to hangul via the
/// plain-text converter; non-string matches are left untouched.  A selector
/// that matches no node logs a warning and is skipped.  The block is round
/// tripped through `serde_json::Value`, so the whole front matter is reformatted
/// even though only matched values change.
fn convert_front_matter(
    yaml: &str,
    selectors: &[String],
    converter: &gukhanmun::Converter<'_>,
) -> Result<String> {
    use jsonpath_rust::JsonPath;
    use jsonpath_rust::query::queryable::Queryable;

    // An empty front matter block parses to a YAML null, which would serialise
    // back as the literal `null`; preserve it verbatim instead.
    if yaml.trim().is_empty() {
        return Ok(if yaml.ends_with('\n') || yaml.is_empty() {
            yaml.to_owned()
        } else {
            format!("{yaml}\n")
        });
    }

    let mut value: serde_json::Value =
        noyalib::from_str(yaml).context("failed to parse YAML front matter")?;

    for selector in selectors {
        let paths = value.query_only_path(selector).map_err(|error| {
            anyhow::anyhow!("invalid front matter JSONPath `{selector}`: {error}")
        })?;
        if paths.is_empty() {
            tracing::warn!("front matter JSONPath `{selector}` matched no nodes");
            continue;
        }
        for path in paths {
            if let Some(serde_json::Value::String(text)) = value.reference_mut(path) {
                let converted = converter.convert_text_to_string(text).map_err(|error| {
                    anyhow::anyhow!("failed to convert front matter value: {error}")
                })?;
                *text = converted;
            }
        }
    }

    let mut serialized =
        noyalib::to_string(&value).context("failed to serialise YAML front matter")?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    Ok(serialized)
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

impl From<RecoveryArg> for Recovery {
    fn from(recovery: RecoveryArg) -> Self {
        match recovery {
            RecoveryArg::Strict => Self::Strict,
            RecoveryArg::Lenient => Self::Lenient,
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

impl From<CliHomophoneDetection> for HomophoneDetection {
    fn from(detection: CliHomophoneDetection) -> Self {
        match detection {
            CliHomophoneDetection::ContextLocal => Self::ContextLocal,
            CliHomophoneDetection::DictionaryWide => Self::DictionaryWide,
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

fn open_user_dictionary(path: &Path) -> Result<Box<dyn HanjaDictionary>> {
    if has_fst_magic(path)? {
        let dict = FstDictionary::open(path)
            .with_context(|| format!("failed to load dictionary {}", path.display()))?;
        tracing::info!(path = %path.display(), format = "fst", "loaded user dictionary");
        return Ok(Box::new(dict));
    }

    let dict = CdbDictionary::open(path)
        .with_context(|| format!("failed to load dictionary {}", path.display()))?;
    tracing::info!(path = %path.display(), format = "cdb", "loaded user dictionary");
    Ok(Box::new(dict))
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

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
    InputToken, NumeralStrategy, OutputToken, PlainScopeData, RenderMode, ScopeData,
    SegmentationStrategy, UserDirectives, apply_user_directives, filter_first_occurrences,
    mark_homophones, process_tokens_iter_with_options, render_tokens_iter, write_plain_text,
};
use gukhanmun_fst::FstDictionary;
use gukhanmun_html::{read_html_fragment, write_html_fragment};
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
    /// 한글(漢字).  hanja-hangul-parens always emits 漢字(한글).  original keeps
    /// the source hanja, adding a hangul gloss only where required.
    #[arg(short, long, value_enum)]
    rendering: Option<Rendering>,

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
    Original,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Segmentation {
    Lattice,
    Eager,
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
    rendering: RenderMode,
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

    if let (Some(input_path), Some(output_path)) = (&cli.io.input, &cli.io.output)
        && is_same_existing_file(input_path, output_path)?
    {
        return convert_file_in_place(
            input_path,
            output_path,
            &dictionary,
            options,
            &directives,
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

    convert_document(input, output, &dictionary, options, &directives, format)
}

fn convert_file_in_place(
    input_path: &Path,
    output_path: &Path,
    dictionary: &CliDictionary,
    options: ResolvedOptions,
    directives: &UserDirectives<'_>,
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
        convert_document(input, output, dictionary, options, directives, format)?;
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
            rendering: RenderMode::HangulOnly,
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
            rendering: RenderMode::HangulOnly,
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
        options.rendering = rendering.into();
    }
    if let Some(disambiguation) = cli.rendering.disambiguation {
        options.homophone_window = disambiguation.into();
    }
    if let Some(first_occurrence) = cli.rendering.first_occurrence {
        options.first_occurrence_window = first_occurrence.into();
    }
    options.engine.segmentation = cli.conversion.segmentation.into();
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
    format: Format,
) -> Result<()> {
    match format {
        Format::PlainText => convert_plain_stream(input, output, dictionary, options, directives),
        Format::Html => convert_html(input, output, dictionary, options, directives),
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
    rendering: RenderMode,
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
    rendering: RenderMode,
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
    rendering: RenderMode,
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
) -> Result<()> {
    let mut content = String::new();
    input
        .read_to_string(&mut content)
        .context("failed to read UTF-8 input")?;
    let input_tokens = read_html_fragment(&content);
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
            Rendering::Original => Self::Original,
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

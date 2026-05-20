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

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use gukhanmun_cdb::CdbDictionary;
use gukhanmun_core::{
    ContextWindow, EngineOptions, HanjaDictionary, Match, NumeralStrategy, RenderMode,
    mark_homophones, process_tokens_with_options, read_plain_text, render_tokens, write_plain_text,
};
use gukhanmun_fst::FstDictionary;
use gukhanmun_html::{read_html_fragment, write_html_fragment};
use gukhanmun_markdown::{read_markdown, write_markdown};

const FST_MAGIC: &[u8; 8] = b"GUKHMFST";

#[derive(Debug, Parser)]
#[command(
    name = "gukhanmun",
    about = "Convert Korean mixed-script plain text into hangul text."
)]
struct Cli {
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
    #[arg(short, long, value_enum, value_name = "MIME")]
    format: Option<Format>,

    /// Language variant preset.  ko-kr (default) enables the bundled Standard
    /// Korean Dictionary (標準國語大辭典) and the initial sound law (頭音法則).  ko-kp disables
    /// both, targeting North Korean orthography.
    #[arg(short, long, value_enum, default_value_t = Preset::KoKr)]
    preset: Preset,

    /// Controls how hanja annotations appear in the output.  hangul-only
    /// (default for most presets) emits hangul, adding parenthesized hanja only
    /// when disambiguation requires it.  hangul-hanja-parens always emits
    /// 한글(漢字).  hanja-hangul-parens always emits 漢字(한글).  original keeps
    /// the source hanja, adding a hangul gloss only where required.
    #[arg(short, long, value_enum)]
    rendering: Option<Rendering>,

    /// Path to a user-supplied dictionary file (.gukfst or .gukcdb).  May be
    /// repeated; later dictionaries take priority over earlier ones and over the
    /// bundled Standard Korean Dictionary (標準國語大辭典).
    #[arg(short = 'd', long = "dictionary", value_name = "PATH")]
    dictionaries: Vec<PathBuf>,

    /// Disable the bundled Standard Korean Dictionary (標準國語大辭典).
    #[arg(short = 'S', long)]
    no_stdict: bool,

    /// Enable the initial sound law (頭音法則), overriding the preset default.
    #[arg(short = 'i', long, visible_alias = "dueum")]
    initial_sound_law: bool,

    /// Disable the initial sound law (頭音法則), overriding the preset default.
    #[arg(short = 'I', long, visible_alias = "no-dueum")]
    no_initial_sound_law: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Format {
    #[value(name = "text/plain")]
    PlainText,
    #[value(name = "text/html")]
    Html,
    #[value(name = "text/markdown")]
    Markdown,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolvedOptions {
    rendering: RenderMode,
    engine: EngineOptions,
    bundled_stdict: bool,
    homophone_window: ContextWindow,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    let options = resolve_options(&cli)?;
    let dictionary = CombinedDictionary::load(&cli.dictionaries, options.bundled_stdict)?;
    let format = cli.format.unwrap_or_else(|| {
        cli.input
            .as_deref()
            .map(detect_format)
            .unwrap_or(Format::PlainText)
    });

    if let (Some(input_path), Some(output_path)) = (&cli.input, &cli.output)
        && is_same_existing_file(input_path, output_path)?
    {
        return convert_file_in_place(input_path, output_path, &dictionary, options, format);
    }

    let input: Box<dyn BufRead> = match &cli.input {
        Some(path) => {
            Box::new(BufReader::new(fs::File::open(path).with_context(|| {
                format!("failed to open input {}", path.display())
            })?))
        }
        None => Box::new(BufReader::new(io::stdin().lock())),
    };
    let output: Box<dyn Write> = match &cli.output {
        Some(path) => Box::new(BufWriter::new(
            fs::File::create(path)
                .with_context(|| format!("failed to create output {}", path.display()))?,
        )),
        None => Box::new(BufWriter::new(io::stdout().lock())),
    };

    convert_document(input, output, &dictionary, options, format)
}

fn convert_file_in_place(
    input_path: &Path,
    output_path: &Path,
    dictionary: &CombinedDictionary,
    options: ResolvedOptions,
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
        convert_document(input, output, dictionary, options, format)?;
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
    if cli.initial_sound_law && cli.no_initial_sound_law {
        bail!("--initial-sound-law and --no-initial-sound-law cannot be used together");
    }

    let mut options = match cli.preset {
        Preset::KoKr => ResolvedOptions {
            rendering: RenderMode::HangulOnly,
            engine: EngineOptions {
                initial_sound_law: true,
                numeral_strategy: NumeralStrategy::HangulPhonetic,
            },
            bundled_stdict: true,
            homophone_window: ContextWindow::PerBlock,
        },
        Preset::KoKp => ResolvedOptions {
            rendering: RenderMode::HangulOnly,
            engine: EngineOptions {
                initial_sound_law: false,
                numeral_strategy: NumeralStrategy::HangulPhonetic,
            },
            bundled_stdict: false,
            homophone_window: ContextWindow::Off,
        },
    };

    if let Some(rendering) = cli.rendering {
        options.rendering = rendering.into();
    }
    if cli.no_stdict {
        options.bundled_stdict = false;
    }
    if cli.initial_sound_law {
        options.engine.initial_sound_law = true;
    }
    if cli.no_initial_sound_law {
        options.engine.initial_sound_law = false;
    }

    Ok(options)
}

fn detect_format(path: &Path) -> Format {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => Format::Html,
        Some("md") | Some("markdown") | Some("mdown") | Some("mkd") => Format::Markdown,
        _ => Format::PlainText,
    }
}

fn convert_document(
    input: impl BufRead,
    output: impl Write,
    dictionary: &CombinedDictionary,
    options: ResolvedOptions,
    format: Format,
) -> Result<()> {
    match format {
        Format::PlainText => convert_plain_stream(input, output, dictionary, options),
        Format::Html => convert_html(input, output, dictionary, options),
        Format::Markdown => convert_markdown(input, output, dictionary, options),
    }
}

fn convert_plain_stream(
    mut input: impl BufRead,
    mut output: impl Write,
    dictionary: &CombinedDictionary,
    options: ResolvedOptions,
) -> Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = input
            .read_line(&mut line)
            .context("failed to read UTF-8 input")?;
        if bytes == 0 {
            break;
        }

        let converted = convert_plain_line(&line, dictionary, options);
        output
            .write_all(converted.as_bytes())
            .context("failed to write output")?;
    }
    output.flush().context("failed to flush output")
}

fn convert_plain_line(
    line: &str,
    dictionary: &CombinedDictionary,
    options: ResolvedOptions,
) -> String {
    let input_tokens = read_plain_text(line);
    let output_tokens = process_tokens_with_options(input_tokens, dictionary, options.engine);
    let output_tokens = match options.homophone_window {
        ContextWindow::Off => output_tokens,
        window => mark_homophones(output_tokens, window),
    };
    let rendered_tokens = render_tokens(output_tokens, options.rendering);
    write_plain_text(rendered_tokens)
}

fn convert_html(
    mut input: impl BufRead,
    mut output: impl Write,
    dictionary: &CombinedDictionary,
    options: ResolvedOptions,
) -> Result<()> {
    let mut content = String::new();
    input
        .read_to_string(&mut content)
        .context("failed to read UTF-8 input")?;
    let input_tokens = read_html_fragment(&content);
    let output_tokens = process_tokens_with_options(input_tokens, dictionary, options.engine);
    let output_tokens = match options.homophone_window {
        ContextWindow::Off => output_tokens,
        window => mark_homophones(output_tokens, window),
    };
    let rendered_tokens = render_tokens(output_tokens, options.rendering);
    let converted = write_html_fragment(rendered_tokens);
    output
        .write_all(converted.as_bytes())
        .context("failed to write output")?;
    output.flush().context("failed to flush output")
}

fn convert_markdown(
    mut input: impl BufRead,
    mut output: impl Write,
    dictionary: &CombinedDictionary,
    options: ResolvedOptions,
) -> Result<()> {
    let mut content = String::new();
    input
        .read_to_string(&mut content)
        .context("failed to read UTF-8 input")?;
    let input_tokens = read_markdown(&content);
    let output_tokens = process_tokens_with_options(input_tokens, dictionary, options.engine);
    let output_tokens = match options.homophone_window {
        ContextWindow::Off => output_tokens,
        window => mark_homophones(output_tokens, window),
    };
    let rendered_tokens = render_tokens(output_tokens, options.rendering);
    let converted =
        write_markdown(rendered_tokens).context("failed to serialize Markdown output")?;
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

struct CombinedDictionary {
    dictionaries: Vec<DictionarySource>,
}

impl CombinedDictionary {
    fn load(user_paths: &[PathBuf], bundled_stdict: bool) -> Result<Self> {
        let mut dictionaries = Vec::new();

        for path in user_paths.iter().rev() {
            dictionaries.push(DictionarySource::open_user(path)?);
        }
        if bundled_stdict {
            dictionaries.push(DictionarySource::Bundled(gukhanmun_stdict::ko_kr()));
        }

        Ok(Self { dictionaries })
    }
}

impl HanjaDictionary for CombinedDictionary {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        let mut seen_lengths = BTreeSet::new();
        let mut matches = Vec::new();

        for dictionary in &self.dictionaries {
            for matched in dictionary.matches_at(s) {
                if seen_lengths.insert(matched.byte_len) {
                    matches.push(matched);
                }
            }
        }

        matches.sort_by_key(|matched| matched.byte_len);
        Box::new(matches.into_iter())
    }

    fn max_word_chars(&self) -> Option<usize> {
        self.dictionaries
            .iter()
            .filter_map(HanjaDictionary::max_word_chars)
            .max()
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        self.dictionaries
            .iter()
            .any(|dictionary| dictionary.has_homophone(hanja, reading))
    }
}

enum DictionarySource {
    UserFst(FstDictionary),
    UserCdb(CdbDictionary),
    Bundled(&'static FstDictionary),
}

impl DictionarySource {
    fn open_user(path: &Path) -> Result<Self> {
        if has_fst_magic(path)? {
            return Ok(Self::UserFst(FstDictionary::open(path).with_context(
                || format!("failed to load dictionary {}", path.display()),
            )?));
        }

        Ok(Self::UserCdb(CdbDictionary::open(path).with_context(
            || format!("failed to load dictionary {}", path.display()),
        )?))
    }
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

impl HanjaDictionary for DictionarySource {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        match self {
            Self::UserFst(dictionary) => dictionary.matches_at(s),
            Self::UserCdb(dictionary) => dictionary.matches_at(s),
            Self::Bundled(dictionary) => dictionary.matches_at(s),
        }
    }

    fn max_word_chars(&self) -> Option<usize> {
        match self {
            Self::UserFst(dictionary) => dictionary.max_word_chars(),
            Self::UserCdb(dictionary) => dictionary.max_word_chars(),
            Self::Bundled(dictionary) => dictionary.max_word_chars(),
        }
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        match self {
            Self::UserFst(dictionary) => dictionary.has_homophone(hanja, reading),
            Self::UserCdb(dictionary) => dictionary.has_homophone(hanja, reading),
            Self::Bundled(dictionary) => dictionary.has_homophone(hanja, reading),
        }
    }
}

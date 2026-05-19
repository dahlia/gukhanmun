use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, ValueEnum};
use gukhanmun_core::{
    ContextWindow, EngineOptions, HanjaDictionary, Match, NumeralStrategy, RenderMode,
    mark_homophones, process_tokens_with_options, read_plain_text, render_tokens, write_plain_text,
};
use gukhanmun_fst::FstDictionary;

#[derive(Debug, Parser)]
#[command(
    name = "gukhanmun",
    about = "Convert Korean mixed-script plain text into hangul text."
)]
struct Cli {
    #[arg(value_name = "INPUT")]
    input: Option<PathBuf>,

    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,

    #[arg(long, value_enum, default_value_t = Preset::KoKr)]
    preset: Preset,

    #[arg(long, value_enum)]
    rendering: Option<Rendering>,

    #[arg(long = "dictionary", value_name = "PATH")]
    dictionaries: Vec<PathBuf>,

    #[arg(long)]
    no_stdict: bool,

    #[arg(long)]
    initial_sound_law: bool,

    #[arg(long)]
    no_initial_sound_law: bool,
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

    if let (Some(input_path), Some(output_path)) = (&cli.input, &cli.output)
        && is_same_existing_file(input_path, output_path)?
    {
        return convert_file_in_place(input_path, output_path, &dictionary, options);
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

    convert_stream(input, output, &dictionary, options)
}

fn convert_file_in_place(
    input_path: &Path,
    output_path: &Path,
    dictionary: &CombinedDictionary,
    options: ResolvedOptions,
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
        convert_stream(input, output, dictionary, options)?;
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

fn convert_stream(
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

        let converted = convert_line(&line, dictionary, options);
        output
            .write_all(converted.as_bytes())
            .context("failed to write output")?;
    }
    output.flush().context("failed to flush output")
}

fn convert_line(line: &str, dictionary: &CombinedDictionary, options: ResolvedOptions) -> String {
    let input_tokens = read_plain_text(line);
    let output_tokens = process_tokens_with_options(input_tokens, dictionary, options.engine);
    let output_tokens = match options.homophone_window {
        ContextWindow::Off => output_tokens,
        window => mark_homophones(output_tokens, window),
    };
    let rendered_tokens = render_tokens(output_tokens, options.rendering);
    write_plain_text(rendered_tokens)
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

#[derive(Clone, Debug)]
struct CombinedDictionary {
    dictionaries: Vec<DictionarySource>,
}

impl CombinedDictionary {
    fn load(user_paths: &[PathBuf], bundled_stdict: bool) -> Result<Self> {
        let mut dictionaries = Vec::new();

        for path in user_paths.iter().rev() {
            dictionaries.push(DictionarySource::User(
                FstDictionary::open(path)
                    .with_context(|| format!("failed to load dictionary {}", path.display()))?,
            ));
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

#[derive(Clone, Debug)]
enum DictionarySource {
    User(FstDictionary),
    Bundled(&'static FstDictionary),
}

impl HanjaDictionary for DictionarySource {
    fn matches_at<'a>(&'a self, s: &'a str) -> Box<dyn Iterator<Item = Match> + 'a> {
        match self {
            Self::User(dictionary) => dictionary.matches_at(s),
            Self::Bundled(dictionary) => dictionary.matches_at(s),
        }
    }

    fn max_word_chars(&self) -> Option<usize> {
        match self {
            Self::User(dictionary) => dictionary.max_word_chars(),
            Self::Bundled(dictionary) => dictionary.max_word_chars(),
        }
    }

    fn has_homophone(&self, hanja: &str, reading: &str) -> bool {
        match self {
            Self::User(dictionary) => dictionary.has_homophone(hanja, reading),
            Self::Bundled(dictionary) => dictionary.has_homophone(hanja, reading),
        }
    }
}

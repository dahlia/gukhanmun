use std::fs;
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "gukhanmun-stdict-extract",
    about = "Extract canonical TSV from Standard Korean Language Dictionary JSON."
)]
struct Cli {
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut output = Vec::new();
    let stats = gukhanmun_stdict::extract::extract_path_to_tsv(&cli.input, &mut output)?;
    if let Some(path) = cli.output {
        fs::write(path, output)?;
    } else {
        print!("{}", String::from_utf8(output)?);
    }
    eprintln!(
        "items_seen={} entries_written={} duplicate_keys={} skipped_items={}",
        stats.items_seen, stats.entries_written, stats.duplicate_keys, stats.skipped_items
    );
    Ok(())
}

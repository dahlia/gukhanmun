// Gukhanmun: Bundled Standard Korean Language Dictionary for Gukhanmun.
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

use std::fs;
use std::io;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

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

    /// Path to write the multi-syllable suffix override TSV
    /// (`hanja\tinitial\tsuffix`).
    #[arg(long, value_name = "PATH")]
    suffix_output: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(io::stderr)
        .init();
    let mut output = Vec::new();
    let stats = if let Some(suffix_path) = cli.suffix_output {
        let mut suffix_output = Vec::new();
        let stats = gukhanmun_stdict::extract::extract_path_to_files(
            &cli.input,
            &mut output,
            &mut suffix_output,
        )?;
        fs::write(suffix_path, suffix_output)?;
        stats
    } else {
        gukhanmun_stdict::extract::extract_path_to_tsv(&cli.input, &mut output)?
    };
    if let Some(path) = cli.output {
        fs::write(path, output)?;
    } else {
        print!("{}", String::from_utf8(output)?);
    }
    tracing::info!(
        items_seen = stats.items_seen,
        entries_written = stats.entries_written,
        duplicate_keys = stats.duplicate_keys,
        skipped_items = stats.skipped_items,
        "extraction complete"
    );
    Ok(())
}

// Gukhanmun: Bundled Open Korean Dictionary data for Gukhanmun.
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
use std::io::BufWriter;
use std::path::PathBuf;

use clap::Parser;
use gukhanmun_opendict::extract::{CategoryWriters, extract_path_to_files};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "gukhanmun-opendict-extract",
    about = "Extract canonical TSVs from Open Korean Dictionary JSON."
)]
struct Cli {
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    #[arg(long, value_name = "PATH")]
    general_output: PathBuf,

    #[arg(long, value_name = "PATH")]
    north_korean_output: PathBuf,

    #[arg(long, value_name = "PATH")]
    dialect_output: PathBuf,

    #[arg(long, value_name = "PATH")]
    archaic_output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(io::stderr)
        .init();

    let general = BufWriter::new(fs::File::create(&cli.general_output)?);
    let north_korean = BufWriter::new(fs::File::create(&cli.north_korean_output)?);
    let dialect = BufWriter::new(fs::File::create(&cli.dialect_output)?);
    let archaic = BufWriter::new(fs::File::create(&cli.archaic_output)?);
    let stats = extract_path_to_files(
        &cli.input,
        CategoryWriters {
            general,
            north_korean,
            dialect,
            archaic,
        },
    )?;
    tracing::info!(
        general_entries = stats.general.entries_written,
        north_korean_entries = stats.north_korean.entries_written,
        dialect_entries = stats.dialect.entries_written,
        archaic_entries = stats.archaic.entries_written,
        "extraction complete"
    );
    Ok(())
}

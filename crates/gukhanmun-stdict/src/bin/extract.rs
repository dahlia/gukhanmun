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

    /// Path to write derived compound aliases.
    #[arg(
        long,
        value_name = "PATH",
        requires = "relations_output",
        requires = "manifest_output"
    )]
    aliases_output: Option<PathBuf>,

    /// Path to write sibling spelling relations.
    #[arg(long, value_name = "PATH", requires = "aliases_output")]
    relations_output: Option<PathBuf>,

    /// Path to write the JSONL alias derivation manifest.
    #[arg(long, value_name = "PATH", requires = "aliases_output")]
    manifest_output: Option<PathBuf>,

    /// Reviewed alias TSV merged over derived alias rows.
    #[arg(long, value_name = "PATH")]
    seed_aliases: Option<PathBuf>,

    /// Path to write additive typed source evidence JSON.
    #[arg(long, value_name = "PATH")]
    evidence_output: Option<PathBuf>,

    /// Check that output files match extraction without modifying them.
    #[arg(long, requires = "output")]
    check: bool,
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
    let stats = if let Some(aliases_path) = cli.aliases_output {
        let mut suffix_output = Vec::new();
        let mut aliases_output = Vec::new();
        let mut relations_output = Vec::new();
        let mut manifest_output = Vec::new();
        let stats = gukhanmun_stdict::extract::extract_path_to_all_files(
            &cli.input,
            &mut output,
            &mut suffix_output,
            &mut aliases_output,
            &mut relations_output,
            &mut manifest_output,
            cli.seed_aliases.as_deref(),
        )?;
        if let Some(path) = cli.suffix_output {
            write_or_check(path, &suffix_output, cli.check)?;
        }
        write_or_check(aliases_path, &aliases_output, cli.check)?;
        write_or_check(
            cli.relations_output
                .expect("clap requires relations output"),
            &relations_output,
            cli.check,
        )?;
        write_or_check(
            cli.manifest_output.expect("clap requires manifest output"),
            &manifest_output,
            cli.check,
        )?;
        stats
    } else if let Some(suffix_path) = cli.suffix_output {
        let mut suffix_output = Vec::new();
        let stats = gukhanmun_stdict::extract::extract_path_to_files(
            &cli.input,
            &mut output,
            &mut suffix_output,
        )?;
        write_or_check(suffix_path, &suffix_output, cli.check)?;
        stats
    } else {
        gukhanmun_stdict::extract::extract_path_to_tsv(&cli.input, &mut output)?
    };
    if let Some(path) = cli.evidence_output {
        let mut evidence = Vec::new();
        gukhanmun_stdict::extract::extract_path_to_evidence_json(&cli.input, &mut evidence)?;
        write_or_check(path, &evidence, cli.check)?;
    }
    if let Some(path) = cli.output {
        write_or_check(path, &output, cli.check)?;
    } else {
        print!("{}", String::from_utf8(output)?);
    }
    tracing::info!(
        items_seen = stats.items_seen,
        entries_written = stats.entries_written,
        duplicate_keys = stats.duplicate_keys,
        skipped_items = stats.skipped_items,
        skipped_default_pins = stats.skipped_default_pins,
        "extraction complete"
    );
    Ok(())
}

fn write_or_check(path: PathBuf, bytes: &[u8], check: bool) -> io::Result<()> {
    if check {
        if fs::read(&path)? != bytes {
            return Err(io::Error::other(format!(
                "{} differs from extracted data",
                path.display()
            )));
        }
        Ok(())
    } else {
        fs::write(path, bytes)
    }
}

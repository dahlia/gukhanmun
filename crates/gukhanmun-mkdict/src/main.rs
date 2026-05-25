// Gukhanmun: Builds Gukhanmun dictionary backend files from canonical TSV input.
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

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use gukhanmun_mkdict::{
    BuildOptions, DEFAULT_MAX_KEY_BYTES, DictionaryFormat, MergePolicy, build_dictionary,
    parse_metadata_arg,
};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "gukhanmun-mkdict",
    about = "Build Gukhanmun dictionary backend files from canonical TSV input."
)]
struct Cli {
    #[arg(value_name = "INPUT", required = true)]
    inputs: Vec<PathBuf>,

    #[arg(short, long, value_name = "PATH")]
    output: PathBuf,

    #[arg(short, long, value_enum, default_value_t = CliFormat::Fst)]
    format: CliFormat,

    #[arg(long, value_enum, default_value_t = CliMergePolicy::Error)]
    merge: CliMergePolicy,

    #[arg(long)]
    validate: bool,

    #[arg(long, default_value_t = DEFAULT_MAX_KEY_BYTES)]
    max_key_bytes: usize,

    #[arg(long = "metadata", value_parser = parse_metadata_arg)]
    metadata: Vec<(String, String)>,

    /// Path to a rules TSV file whose entries OR-merge marks into the
    /// dictionary entries before serialization.  May be repeated.
    #[arg(long = "rules", value_name = "PATH")]
    rules: Vec<PathBuf>,

    /// Treat rules that match no dictionary entry as a warning instead of an
    /// error.  Useful for partial dictionaries that share a rules file with a
    /// larger build.
    #[arg(long)]
    allow_unmatched_rules: bool,

    /// Enable debug-level diagnostic output (overridden by RUST_LOG).
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliFormat {
    Fst,
    Cdb,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliMergePolicy {
    Error,
    FirstWins,
    LastWins,
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
    let options = BuildOptions {
        format: match cli.format {
            CliFormat::Fst => DictionaryFormat::Fst,
            CliFormat::Cdb => DictionaryFormat::Cdb,
        },
        merge: match cli.merge {
            CliMergePolicy::Error => MergePolicy::Error,
            CliMergePolicy::FirstWins => MergePolicy::FirstWins,
            CliMergePolicy::LastWins => MergePolicy::LastWins,
        },
        validate: cli.validate,
        max_key_bytes: cli.max_key_bytes,
        metadata: cli.metadata.into_iter().collect::<BTreeMap<_, _>>(),
        rules: cli.rules,
        allow_unmatched_rules: cli.allow_unmatched_rules,
    };

    build_dictionary(&cli.inputs, cli.output, &options)?;
    Ok(())
}

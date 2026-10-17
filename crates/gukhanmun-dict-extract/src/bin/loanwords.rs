// Gukhanmun: Experimental source-independent loanword extraction tooling.
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

//! Explicit-input collection and extraction for the pinned loanword dictionary.
use clap::{Args, Parser, Subcommand};
use gukhanmun_dict_extract::loanwords::{Error, ExtractionPaths, process_files};
use std::path::PathBuf;
#[derive(Parser)]
#[command(about = "NIKL loanword collection and extraction tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Extract TSV and hashed report, failing on unresolved/stale defaults.
    Extract(Paths),
    /// Validate and compare existing artifacts without writing any paths.
    Check(Paths),
    /// Write an audit-only report without a successful TSV or default claim.
    Audit(Paths),
    /// Collect two full sweeps to an explicit external snapshot.
    #[cfg(feature = "loanwords-collect")]
    Collect(Collect),
}
#[derive(Args)]
struct Paths {
    #[arg(long)]
    snapshot: PathBuf,
    #[arg(long)]
    decisions: Option<PathBuf>,
    #[arg(long)]
    prior_snapshot: Option<PathBuf>,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    diagnostic: Option<PathBuf>,
}
impl From<Paths> for ExtractionPaths {
    fn from(p: Paths) -> Self {
        Self {
            snapshot: p.snapshot,
            decisions: p.decisions,
            prior_snapshot: p.prior_snapshot,
            tsv: p.output,
            report: p.report,
            diagnostic: p.diagnostic,
        }
    }
}
#[cfg(feature = "loanwords-collect")]
#[derive(Args)]
struct Collect {
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    check: bool,
    #[arg(long, default_value_t = 100)]
    rows: u64,
    #[arg(long, default_value_t = 1000)]
    max_pages: u64,
    #[arg(long, default_value_t = 4194304)]
    max_body_bytes: u64,
    #[arg(long, default_value_t = 134217728)]
    max_total_bytes: u64,
    #[arg(long, default_value_t = 30000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 1000)]
    delay_ms: u64,
    #[arg(long, default_value_t = 2)]
    retries: u32,
}
fn run(cli: Cli) -> Result<(), Error> {
    match cli.command {
        Command::Extract(p) => process_files(&p.into(), false, false),
        Command::Check(p) => process_files(&p.into(), true, false),
        Command::Audit(p) => process_files(&p.into(), false, true),
        #[cfg(feature = "loanwords-collect")]
        Command::Collect(p) => {
            use gukhanmun_dict_extract::loanwords::{
                collect::{Limits, collect},
                collect_destination, preflight_paths, publish_snapshot,
            };
            use std::time::{Duration, SystemTime, UNIX_EPOCH};
            collect_destination(&p.output)?;
            preflight_paths(&[], &[&p.output])?;
            let l = Limits {
                rows: p.rows,
                max_pages: p.max_pages,
                max_body_bytes: p.max_body_bytes,
                max_total_bytes: p.max_total_bytes,
                timeout: Duration::from_millis(p.timeout_ms),
                delay: Duration::from_millis(p.delay_ms),
                retries: p.retries,
                ..Limits::default()
            };
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Invalid("system clock"))?
                .as_secs();
            let snapshot = collect(&l, format!("unix:{now}"))?;
            publish_snapshot(&p.output, &snapshot, p.check)
        }
    }
}
fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}

// Gukhanmun: Bundled NIKL loanword examples for Gukhanmun.
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

use anyhow::Result;
use gukhanmun_mkdict::{BuildOptions, build_dictionary_with_evidence};
use std::{collections::BTreeMap, env, path::PathBuf};
fn main() -> Result<()> {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let tsv = root.join("data/loanwords.tsv");
    let evidence_path = root.join("data/source-evidence.json");
    println!("cargo:rerun-if-changed={}", evidence_path.display());
    let evidence = serde_json::from_slice(&std::fs::read(evidence_path)?)?;
    let metadata = root.join("data/metadata.json");
    println!("cargo:rerun-if-changed={}", tsv.display());
    println!("cargo:rerun-if-changed={}", metadata.display());
    let provenance: BTreeMap<String, String> = serde_json::from_slice(&std::fs::read(metadata)?)?;
    build_dictionary_with_evidence(
        &[tsv],
        PathBuf::from(env::var_os("OUT_DIR").expect("output directory")).join("loanwords.gukfst"),
        &BuildOptions {
            validate: true,
            metadata: provenance,
            ..BuildOptions::default()
        },
        &evidence,
    )?;
    Ok(())
}

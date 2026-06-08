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

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use gukhanmun_mkdict::{BuildOptions, DictionaryFormat, MergePolicy, build_dictionary};

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("out dir"));

    build_category(
        &manifest_dir,
        &out_dir,
        "general",
        "general.tsv",
        "general.gukfst",
    )?;
    build_category(
        &manifest_dir,
        &out_dir,
        "north-korean",
        "north-korean.tsv",
        "north-korean.gukfst",
    )?;
    build_category(
        &manifest_dir,
        &out_dir,
        "dialect",
        "dialect.tsv",
        "dialect.gukfst",
    )?;
    build_category(
        &manifest_dir,
        &out_dir,
        "archaic",
        "archaic.tsv",
        "archaic.gukfst",
    )?;

    Ok(())
}

fn build_category(
    manifest_dir: &Path,
    out_dir: &Path,
    category: &str,
    input_name: &str,
    output_name: &str,
) -> Result<()> {
    let input = manifest_dir.join("data").join(input_name);
    let output = out_dir.join(output_name);
    println!("cargo:rerun-if-changed={}", input.display());
    build_dictionary(
        &[input],
        output,
        &BuildOptions {
            format: DictionaryFormat::Fst,
            merge: MergePolicy::Error,
            validate: true,
            max_key_bytes: gukhanmun_mkdict::DEFAULT_MAX_KEY_BYTES,
            metadata: BTreeMap::from([
                ("source".to_owned(), "opendict-json-20260603".to_owned()),
                ("license".to_owned(), "CC-BY-SA-2.0-KR".to_owned()),
                ("category".to_owned(), category.to_owned()),
                ("build_date".to_owned(), "2026-06-03T00:00:00Z".to_owned()),
            ]),
            rules: Vec::new(),
            allow_unmatched_rules: false,
        },
    )?;
    Ok(())
}

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

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

use anyhow::Result;
use gukhanmun_mkdict::{BuildOptions, DictionaryFormat, MergePolicy, build_dictionary};

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let input = manifest_dir.join("data").join("stdict.tsv");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("out dir")).join("stdict.gukfst");

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
                ("source".to_owned(), "stdict-json-20260506".to_owned()),
                (
                    "license".to_owned(),
                    "National Institute of Korean Language".to_owned(),
                ),
                ("build_date".to_owned(), "2026-05-06T00:00:00Z".to_owned()),
            ]),
        },
    )?;

    Ok(())
}

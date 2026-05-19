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

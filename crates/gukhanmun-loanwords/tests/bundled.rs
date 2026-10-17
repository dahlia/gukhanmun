// Gukhanmun: Tests bundled NIKL dictionary fidelity and streaming behavior.
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

use gukhanmun_core::{
    Engine, EngineOptions, HanjaDictionary, InputToken, PlainScopeData, RenderMode,
    SegmentationStrategy, process_tokens_with_options, render_tokens, write_plain_text,
};
use gukhanmun_mkdict::{BuildOptions, DictionaryFormat, build_dictionary};
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn complete_bundled_candidates_roundtrip_through_cdb() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let metadata: BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read(root.join("data/metadata.json")).unwrap()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loanwords.gukcdb");
    build_dictionary(
        &[root.join("data/loanwords.tsv")],
        &path,
        &BuildOptions {
            format: DictionaryFormat::Cdb,
            validate: true,
            metadata,
            ..BuildOptions::default()
        },
    )
    .unwrap();
    let cdb = gukhanmun_cdb::CdbDictionary::open(&path).unwrap();
    let fst = gukhanmun_loanwords::dictionary();
    let mut expected = fst.candidate_entries().unwrap().collect::<Vec<_>>();
    let mut actual = cdb.candidate_entries().unwrap().collect::<Vec<_>>();
    expected.sort_by(|a, b| a.hanja.cmp(&b.hanja));
    actual.sort_by(|a, b| a.hanja.cmp(&b.hanja));
    assert_eq!(expected, actual);
    assert_eq!(fst.entry_count(), expected.len() as u64);
    let hong_kong = fst.lookup("香港").unwrap().unwrap();
    assert_eq!(hong_kong.reading(), "홍콩");
    assert!(
        hong_kong
            .candidates()
            .as_slice()
            .iter()
            .any(|c| c.reading == "샹강")
    );
    let expected_homophone = expected
        .iter()
        .any(|entry| entry.hanja != "香港" && entry.select_default().reading == "샹강");
    assert_eq!(fst.has_homophone("香港", "샹강"), expected_homophone);
    assert_eq!(cdb.has_homophone("香港", "샹강"), expected_homophone);
}

#[test]
fn real_dictionary_streams_identically_at_every_character_boundary() {
    let dictionary = gukhanmun_loanwords::dictionary();
    let input = "香港과 日本을 방문했다";
    for segmentation in [SegmentationStrategy::Lattice, SegmentationStrategy::Eager] {
        let options = EngineOptions {
            segmentation,
            ..EngineOptions::default()
        };
        let expected = write_plain_text(render_tokens(
            process_tokens_with_options(
                [InputToken::<PlainScopeData>::Text(input.into())],
                dictionary,
                options,
            ),
            RenderMode::HangulHanjaParens,
        ));
        for split in input.char_indices().map(|(i, _)| i).chain([input.len()]) {
            let mut engine = Engine::<PlainScopeData, _>::with_options(dictionary, options);
            let mut tokens = engine.push_token(InputToken::Text(input[..split].into()));
            tokens.extend(engine.push_token(InputToken::Text(input[split..].into())));
            tokens.extend(engine.finish());
            assert_eq!(
                write_plain_text(render_tokens(tokens, RenderMode::HangulHanjaParens)),
                expected
            );
        }
    }
}

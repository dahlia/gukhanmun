// Gukhanmun: umbrella library that wires the engine and adapters together.
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

//! End-to-end tests for redundant parenthetical annotation collapsing through
//! the umbrella `Builder` / `Converter`, including the streaming path.

use gukhanmun::{
    Builder, ContextWindow, InputToken, MapDictionary, PlainScopeData, RenderMode, Scope,
    write_plain_text,
};

/// Builds a fresh converter per render mode (the `Converter` is immutable) and
/// converts `input` through the buffered string API.
fn convert_mode(records: &[(&str, &str)], mode: RenderMode, input: &str) -> String {
    let mut dict = MapDictionary::new();
    for (hanja, reading) in records {
        dict.insert(*hanja, *reading);
    }
    Builder::new()
        .no_bundled_stdict()
        .push_dictionary(dict)
        .homophone_window(ContextWindow::Off)
        .rendering(mode)
        .build()
        .expect("builder")
        .convert_text_to_string(input)
        .expect("convert")
}

#[test]
fn hanja_first_collapses_in_hangul_only() {
    assert_eq!(
        convert_mode(&[("庫間", "곳간")], RenderMode::HangulOnly, "庫間(곳간)"),
        "곳간(庫間)"
    );
}

#[test]
fn hangul_first_collapses_in_hangul_only() {
    assert_eq!(
        convert_mode(&[("庫間", "곳간")], RenderMode::HangulOnly, "곳간(庫間)"),
        "곳간(庫間)"
    );
}

#[test]
fn hanja_first_collapses_in_original() {
    assert_eq!(
        convert_mode(&[("庫間", "곳간")], RenderMode::Original, "庫間(곳간)"),
        "庫間(곳간)"
    );
}

#[test]
fn hangul_first_collapses_in_original() {
    assert_eq!(
        convert_mode(&[("庫間", "곳간")], RenderMode::Original, "곳간(庫間)"),
        "庫間(곳간)"
    );
}

#[test]
fn alternative_reading_is_pinned() {
    assert_eq!(
        convert_mode(&[("數字", "숫자")], RenderMode::HangulOnly, "數字(수자)"),
        "수자(數字)"
    );
    assert_eq!(
        convert_mode(&[("數字", "숫자")], RenderMode::Original, "數字(수자)"),
        "數字(수자)"
    );
}

#[test]
fn definition_gloss_passes_through() {
    assert_eq!(
        convert_mode(
            &[("庫間", "곳간")],
            RenderMode::HangulOnly,
            "庫間(물건을 간직하여 두는 곳)"
        ),
        "곳간(물건을 간직하여 두는 곳)"
    );
}

#[test]
fn disabled_keeps_the_redundant_parenthetical() {
    let output = Builder::new()
        .no_bundled_stdict()
        .push_dictionary({
            let mut dict = MapDictionary::new();
            dict.insert("庫間", "곳간");
            dict
        })
        .homophone_window(ContextWindow::Off)
        .collapse_redundant_parens(false)
        .build()
        .expect("builder")
        .convert_text_to_string("庫間(곳간)")
        .expect("convert");
    assert_eq!(output, "곳간(곳간)");
}

/// A word the author glosses on every occurrence must stay fully annotated
/// even when first-occurrence filtering would otherwise clear the requirement
/// on the repeat.
#[test]
fn first_occurrence_filter_preserves_repeated_explicit_glosses() {
    let mut dict = MapDictionary::new();
    dict.insert("庫間", "곳간");
    let output = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(dict)
        .homophone_window(ContextWindow::Off)
        .first_occurrence_window(ContextWindow::PerDocument)
        .build()
        .expect("builder")
        .convert_text_to_string("庫間(곳간) 庫間(곳간)")
        .expect("convert");
    assert_eq!(output, "곳간(庫間) 곳간(庫間)");
}

/// Streaming the parenthetical across a chunk boundary must match the one-shot
/// (buffered) result; this guards the project's streaming-equals-one-shot
/// invariant for the collapser.
#[test]
fn streaming_across_chunk_boundaries_matches_one_shot() {
    let mut dict = MapDictionary::new();
    dict.insert("庫間", "곳간");
    let converter = Builder::new()
        .no_bundled_stdict()
        .push_dictionary(dict)
        .homophone_window(ContextWindow::Off)
        .build()
        .expect("builder");

    let one_shot = converter
        .convert_text_to_string("庫間(곳간)")
        .expect("convert");

    for split in 1.."庫間(곳간)".chars().count() {
        let prefix: String = "庫間(곳간)".chars().take(split).collect();
        let suffix: String = "庫間(곳간)".chars().skip(split).collect();
        let input_tokens = vec![
            InputToken::Open(Scope::new(PlainScopeData)),
            InputToken::Text(prefix.clone()),
            InputToken::Text(suffix.clone()),
            InputToken::Close,
        ];
        let streamed = write_plain_text(converter.convert_tokens(input_tokens));
        assert_eq!(
            streamed, one_shot,
            "chunk split after {split} chars (`{prefix}` | `{suffix}`) diverged from one-shot"
        );
    }
}

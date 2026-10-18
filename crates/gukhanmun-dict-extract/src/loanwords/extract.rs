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

use super::{Error, Snapshot, canonical_json, parse_json, sha256};
use gukhanmun_core::{MatchMark, ReadingCandidate, ReadingCandidates};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// Whole-record evidence belonging to one normalized reading.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEvidence {
    /// Opaque source identity.
    pub id: String,
    /// SHA-256 of every preserved record field.
    pub sha256: String,
}
/// Distinct reading and all its evidence, sorted by source identity.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    /// Trimmed hangul output; single ordinary spaces may separate syllable runs.
    pub reading: String,
    /// Complete source evidence; even same-reading additions invalidate decisions.
    pub sources: Vec<SourceEvidence>,
}
/// Editorial default scoped to exactly one key and complete candidate evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// Normalized lookup key.
    pub key: String,
    /// Selected reading, which receives strictly higher preference.
    pub chosen: String,
    /// Exact competing candidates and every source fingerprint, order irrelevant.
    pub expected: Vec<Candidate>,
    /// Human explanation for this editorial choice.
    pub rationale: String,
    /// Supporting references, retained in the hashed decision document.
    pub references: Vec<String>,
}
/// Versioned caller-owned editorial decisions; no global defaults are inferred.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decisions {
    /// Decision format, currently 1.
    pub format_version: u32,
    /// Decisions for this exact extraction input.
    pub decisions: Vec<Decision>,
}
impl Default for Decisions {
    fn default() -> Self {
        Self {
            format_version: 1,
            decisions: Vec::new(),
        }
    }
}
impl Decisions {
    /// Parses strict JSON and checks the decision format version.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let d: Self = serde_json::from_value(parse_json(bytes)?)
            .map_err(|_| Error::Invalid("decisions schema"))?;
        if d.format_version != 1 {
            return Err(Error::Invalid("decisions version"));
        }
        Ok(d)
    }
    /// Hashes canonical decision content; formatting/object order do not matter.
    pub fn fingerprint(&self) -> String {
        sha256(&canonical_json(&json!(self)))
    }
}
/// A record excluded by the deliberately conservative initial grammar.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Exclusion {
    /// Source identity.
    pub id: String,
    /// Fixed machine-readable reason.
    pub reason: String,
}
/// Per-record normalization audit, preserving raw source text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Normalization {
    /// Source identity.
    pub id: String,
    /// Unchanged original spelling.
    pub original: String,
    /// Unchanged source reading.
    pub reading: String,
    /// Resulting continuous lookup key.
    pub key: String,
    /// Normalization operations that were applied.
    pub reasons: Vec<String>,
}
/// Deterministic audit, usable even before editorial decisions exist.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Audit {
    /// Accepted keys and complete candidate signatures.
    pub candidates: BTreeMap<String, Vec<Candidate>>,
    /// Unsupported records and exclusion reasons.
    pub exclusions: Vec<Exclusion>,
    /// Accepted records, including unchanged raw fields and normalization reasons.
    pub normalizations: Vec<Normalization>,
    /// Keys requiring new decisions.
    pub unresolved: Vec<String>,
    /// Decisions with changed evidence, removed keys, or invalid selections.
    pub stale_decisions: Vec<String>,
    /// Greatest accepted key length in Unicode scalar values.
    pub max_key_chars: usize,
    /// Greatest accepted key length in UTF-8 bytes.
    pub max_key_bytes: usize,
}
/// Validated successful output; report publication adds input/output hashes.
#[derive(Clone, Debug)]
pub struct Extraction {
    /// Canonical candidate TSV accepted by existing mkdict builders.
    pub tsv: Vec<u8>,
    /// Full extraction audit.
    pub audit: Audit,
}

fn hangul(c: char) -> bool {
    matches!(c, '\u{ac00}'..='\u{d7a3}')
}
fn hanja(c: char) -> bool {
    crate::is_hanja(c) && !matches!(c, '\u{2f00}'..='\u{2fff}')
}
fn pronunciation(c: char) -> bool {
    c.is_ascii_alphabetic()
        || matches!(c,
        '\u{00c0}'..='\u{00d6}' | '\u{00d8}'..='\u{00f6}' | '\u{00f8}'..='\u{024f}' |
        '\u{0300}'..='\u{036f}' | '\u{3041}'..='\u{3096}' | '\u{30a1}'..='\u{30fa}' |
        ' ' | '\'' | '’' | '-' | '·' | '.' | 'ー')
}
fn key(raw: &str) -> Result<(String, Vec<String>), &'static str> {
    let mut reasons = Vec::new();
    let mut s = raw.trim();
    if s != raw {
        reasons.push("outer-whitespace".into());
    }
    if s.ends_with(')') || s.ends_with('）') {
        let (open, close) = if s.ends_with(')') {
            ('(', ')')
        } else {
            ('（', '）')
        };
        let i = s.rfind(open).ok_or("unbalanced-annotation")?;
        let note = &s[i + open.len_utf8()..s.len() - close.len_utf8()];
        if note.trim().is_empty()
            || !note.chars().all(pronunciation)
            || !note
                .chars()
                .any(|c| c.is_alphabetic() && !hangul(c) && !hanja(c))
        {
            return Err("unsupported-annotation");
        }
        s = s[..i].trim();
        reasons.push("pronunciation-annotation".into());
    }
    if s.is_empty() {
        return Err("empty-original");
    }
    if s.chars().any(char::is_whitespace) {
        return Err("internal-key-whitespace");
    }
    if !s.chars().all(|c| hanja(c) || hangul(c)) {
        return Err("unsupported-key-character");
    }
    if !s.chars().any(hanja) {
        return Err("no-hanja");
    }
    if s.len() > 1024 {
        return Err("key-byte-limit");
    }
    Ok((s.into(), reasons))
}
fn reading(raw: &str) -> Result<String, &'static str> {
    let s = raw.trim();
    if s.is_empty()
        || !s
            .split(' ')
            .all(|word| !word.is_empty() && word.chars().all(hangul))
    {
        return Err("unsupported-reading");
    }
    if s.len() > u16::MAX as usize {
        return Err("reading-byte-limit");
    }
    Ok(s.into())
}

/// Audits independent snapshot records without selecting defaults or writing files.
/// Only `srclang_mark` supplies lookup keys; related/incorrect spellings never do.
pub fn audit(snapshot: &Snapshot, decisions: &Decisions) -> Audit {
    let mut out = Audit::default();
    let mut grouped = BTreeMap::<String, BTreeMap<String, Vec<SourceEvidence>>>::new();
    for r in &snapshot.records {
        if !r.fields["srclang_mark"].is_string() || !r.fields["korean_mark"].is_string() {
            out.exclusions.push(Exclusion {
                id: r.id.clone(),
                reason: "missing-spelling".into(),
            });
            continue;
        }
        let raw = r.fields["srclang_mark"].as_str().unwrap_or("");
        let raw_reading = r.fields["korean_mark"].as_str().unwrap_or("");
        let (key, mut reasons) = match key(raw) {
            Ok(v) => v,
            Err(reason) => {
                out.exclusions.push(Exclusion {
                    id: r.id.clone(),
                    reason: reason.into(),
                });
                continue;
            }
        };
        let reading = match reading(raw_reading) {
            Ok(v) => v,
            Err(reason) => {
                out.exclusions.push(Exclusion {
                    id: r.id.clone(),
                    reason: reason.into(),
                });
                continue;
            }
        };
        if reading != raw_reading {
            reasons.push("reading-outer-whitespace".into());
        }
        out.max_key_chars = out.max_key_chars.max(key.chars().count());
        out.max_key_bytes = out.max_key_bytes.max(key.len());
        out.normalizations.push(Normalization {
            id: r.id.clone(),
            original: raw.into(),
            reading: raw_reading.into(),
            key: key.clone(),
            reasons,
        });
        grouped
            .entry(key)
            .or_default()
            .entry(reading)
            .or_default()
            .push(SourceEvidence {
                id: r.id.clone(),
                sha256: r.fingerprint(),
            });
    }
    for (key, group) in grouped {
        let candidates = group
            .into_iter()
            .map(|(reading, mut sources)| {
                sources.sort();
                Candidate { reading, sources }
            })
            .collect::<Vec<_>>();
        out.candidates.insert(key, candidates);
    }
    out.normalizations.sort_by(|a, b| a.id.cmp(&b.id));
    out.exclusions.sort_by(|a, b| a.id.cmp(&b.id));
    let mut seen = BTreeSet::new();
    for d in &decisions.decisions {
        let mut expected = d.expected.clone();
        for c in &mut expected {
            c.sources.sort();
        }
        expected.sort();
        let valid = out.candidates.get(&d.key).is_some_and(|actual| {
            actual.len() > 1 && actual == &expected && actual.iter().any(|c| c.reading == d.chosen)
        });
        if decisions.format_version != 1
            || !valid
            || d.rationale.trim().is_empty()
            || d.references.iter().any(|r| r.trim().is_empty())
            || !seen.insert(&d.key)
        {
            out.stale_decisions.push(d.key.clone());
        }
    }
    for (key, c) in &out.candidates {
        if c.len() > 1 && !seen.contains(key) {
            out.unresolved.push(key.clone());
        }
    }
    out.stale_decisions.sort();
    out.stale_decisions.dedup();
    out
}

/// Fails on unresolved/stale decisions and validates backend limits before TSV.
/// Singletons use zero preference; a chosen alternative uses one, all others zero.
pub fn extract(snapshot: &Snapshot, decisions: &Decisions) -> Result<Extraction, Error> {
    // Public structs are inspectable and mutable; never publish an invalidated
    // snapshot even when a caller bypasses the parsing constructor.
    Snapshot::parse(&snapshot.bytes())?;
    let out = audit(snapshot, decisions);
    if decisions.format_version != 1
        || !out.unresolved.is_empty()
        || !out.stale_decisions.is_empty()
    {
        return Err(Error::Decisions);
    }
    let choices = decisions
        .decisions
        .iter()
        .map(|d| (&d.key, &d.chosen))
        .collect::<BTreeMap<_, _>>();
    let mut tsv = String::from(
        "hanja\thangul\trequire_hanja\trequire_hangul\tpreference\tsuffix_reading\tsource_ids\n",
    );
    for (key, candidates) in &out.candidates {
        let runtime = candidates
            .iter()
            .map(|c| ReadingCandidate {
                reading: c.reading.clone(),
                suffix_reading: None,
                mark: MatchMark::default(),
                preference: u32::from(
                    choices
                        .get(key)
                        .is_some_and(|selected| **selected == c.reading),
                ),
            })
            .collect();
        let runtime =
            ReadingCandidates::new(runtime).map_err(|_| Error::Invalid("candidate set"))?;
        if !runtime.is_scalar() {
            runtime
                .encode_payload()
                .map_err(|_| Error::Invalid("candidate payload byte limit"))?;
        }
        for c in candidates {
            // Public Snapshot construction is revalidated by the filesystem API;
            // pure callers also cannot inject delimiter-bearing source identities.
            if c.sources.iter().any(|s| {
                s.id.is_empty()
                    || !s
                        .id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            }) {
                return Err(Error::Invalid("unsafe source ID"));
            }
            let preference = u32::from(
                choices
                    .get(key)
                    .is_some_and(|selected| **selected == c.reading),
            );
            tsv.push_str(&format!(
                "{key}\t{}\tfalse\tfalse\t{preference}\t\t{}\n",
                c.reading,
                c.sources
                    .iter()
                    .map(|s| s.id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
    }
    Ok(Extraction {
        tsv: tsv.into_bytes(),
        audit: out,
    })
}

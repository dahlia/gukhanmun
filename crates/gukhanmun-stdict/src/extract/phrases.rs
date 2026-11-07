// Gukhanmun: Compiles source-attested phrases and typed terminal extensions.
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

//! Conservative compilation of raw phrase records, separate from word admission.

use super::*;
use gukhanmun_core::{
    PhraseCandidateLink, PhraseChunk, PhraseEvidence, PhraseRule, PhraseSpelling,
    SubheadwordWitness, TerminalExtension,
};
use gukhanmun_dict_extract::aliases::{Chunk, SiblingDonors};
use serde::Serialize;

// These are compiler admission limits, not inferred corpus maxima.
const MAX_CHUNKS: usize = 16;
const MAX_SPAN: usize = 128;
const MAX_ALTERNATIVES: usize = 32;
const MAX_WITNESSES: usize = 256;
const MAX_RULES: usize = 65_536;
const MAX_EXTENSION: usize = 16;
const MAX_COMBINATIONS: usize = 4096;

pub(super) struct RawWord {
    id: Option<u64>,
    word: String,
    unit: String,
    originals: Vec<OriginalLanguageInfo>,
    categories: BTreeSet<String>,
    children: Vec<Option<u64>>,
}

impl RawWord {
    pub(super) fn from_item(item: &Item) -> Self {
        let info = item.word_info.as_ref();
        Self {
            id: item.target_code,
            word: info.and_then(|w| w.word.clone()).unwrap_or_default(),
            unit: info.and_then(|w| w.word_unit.clone()).unwrap_or_default(),
            originals: info
                .and_then(|w| w.original_language_info.clone())
                .unwrap_or_default(),
            categories: info
                .into_iter()
                .flat_map(|w| &w.pos_info)
                .flat_map(|p| &p.comm_pattern_info)
                .flat_map(|p| &p.sense_info)
                .flat_map(|s| &s.cat_info)
                .filter_map(|c| c.cat.clone())
                .collect(),
            children: info
                .into_iter()
                .flat_map(|w| &w.relation_info)
                .filter(|r| r.kind.as_deref() == Some("부표제어"))
                // relation_info is explicitly a word relationship. A supplied
                // conflicting unit must never be reinterpreted as a WordId.
                .map(|r| {
                    if r.unit.as_deref().is_none_or(|u| u == "어휘") {
                        r.link_target_code.as_ref().and_then(parse_id)
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }
}

fn parse_id(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_str()
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))?
            .parse()
            .ok()
    })
}

/// Extracts strict source phrases and their deterministic admission audit.
///
/// Raw phrase and word relationships are collected before ordinary TSV admission.
/// This export never adds phrase keys to the word dictionary or changes defaults.
pub fn extract_path_to_phrase_files(
    path: &Path,
    evidence: impl Write,
    manifest: impl Write,
) -> Result<()> {
    let mut extractor = Extractor {
        phrase_sources: Some(Vec::new()),
        ..Extractor::default()
    };
    extractor.read_path(path)?;
    compile(extractor, evidence, manifest)
}

/// Compiles one JSON shard into phrase evidence and an audit for focused tools.
///
/// Cross-shard relationships missing from this input withhold terminal extensions.
pub fn extract_json_reader_to_phrase_files(
    reader: impl Read,
    evidence: impl Write,
    manifest: impl Write,
) -> Result<()> {
    let mut extractor = Extractor {
        phrase_sources: Some(Vec::new()),
        ..Extractor::default()
    };
    extractor.read_json(reader)?;
    compile(extractor, evidence, manifest)
}

#[derive(Serialize)]
struct Audit {
    schema_version: u32,
    policy_version: u32,
    limits: BTreeMap<&'static str, usize>,
    excluded_categories: Vec<&'static str>,
    separator_policy: &'static str,
    records: Vec<AuditRecord>,
}
#[derive(Serialize)]
struct AuditRecord {
    original_spellings: Vec<String>,
    categories: Vec<String>,
    source_id: Option<u64>,
    word: String,
    outcome: String,
    terminal_extensions: usize,
    terminal_outcome: String,
    terminal_withheld: Vec<WithheldTerminal>,
}
#[derive(Serialize)]
struct WithheldTerminal {
    literal_suffix: String,
    reason: &'static str,
}

fn hangul(c: char) -> bool {
    ('가'..='힣').contains(&c)
}

fn strict_chunks(raw: &RawWord) -> std::result::Result<Vec<Chunk>, &'static str> {
    if raw.id.is_none() {
        return Err("missing-word-id");
    }
    if raw.originals.len() != 1 {
        return Err("original-count");
    }
    let original = &raw.originals[0];
    if original.language_type.as_deref() != Some("한자") {
        return Err("original-language");
    }
    let key = original.original_language.as_deref().unwrap_or_default();
    if key.is_empty() || !key.chars().all(is_hanja) {
        return Err("original-not-all-hanja");
    }
    // A hyphen is a morpheme marker, never an optional spacing boundary.
    let parts = raw.word.split('^').map(normalize_word).collect::<Vec<_>>();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty() || !p.chars().all(hangul)) {
        return Err("chunk-alignment");
    }
    if parts.len() > MAX_CHUNKS {
        return Err("chunk-limit");
    }
    if key.chars().count() + parts.len() - 1 > MAX_SPAN {
        return Err("span-limit");
    }
    let mut chunks = Vec::new();
    let mut offset = 0;
    for part in parts {
        let len = part.chars().count();
        let hanja = key.chars().skip(offset).take(len).collect::<String>();
        if hanja.chars().count() != len {
            return Err("chunk-alignment");
        }
        chunks.push(Chunk {
            start: offset,
            reading: part,
            hanja,
        });
        offset += len;
    }
    if offset != key.chars().count() {
        return Err("chunk-alignment");
    }
    Ok(chunks)
}

fn links(extractor: &Extractor, key: &str, reading: &str) -> Vec<PhraseCandidateLink> {
    extractor
        .entries
        .get(key)
        .into_iter()
        .flat_map(|entries| extraction_candidates(key, entries))
        .filter(|c| c.reading == reading || c.suffix_reading.as_deref() == Some(reading))
        .map(|c| PhraseCandidateLink::new(c.reading, c.suffix_reading))
        .collect()
}

fn compile(
    mut extractor: Extractor,
    mut evidence_writer: impl Write,
    mut manifest: impl Write,
) -> Result<()> {
    let mut raws = extractor.phrase_sources.take().unwrap_or_default();
    raws.sort_by(|a, b| {
        (&a.id, &a.word, &a.unit, &a.categories, &a.children)
            .cmp(&(&b.id, &b.word, &b.unit, &b.categories, &b.children))
            .then_with(|| {
                a.originals
                    .iter()
                    .map(|o| (&o.language_type, &o.original_language))
                    .cmp(
                        b.originals
                            .iter()
                            .map(|o| (&o.language_type, &o.original_language)),
                    )
            })
    });
    let mut ids = BTreeMap::<u64, Vec<&RawWord>>::new();
    for raw in &raws {
        if let Some(id) = raw.id {
            ids.entry(id).or_default().push(raw);
        }
    }
    let mut words_by_reading = BTreeMap::<String, Vec<&RawWord>>::new();
    for raw in &raws {
        if raw.unit == "단어" {
            words_by_reading
                .entry(normalize_word(&raw.word))
                .or_default()
                .push(raw);
        }
    }
    let policy = AliasPolicy::default();
    let donors = SiblingDonors::new(&extractor.alias_sources, &policy);
    // Include every raw phrase spelling in collision detection, even phrases
    // that fail strict admission. An unsafe row cannot disappear from checks.
    let mut readings = BTreeMap::<String, BTreeSet<String>>::new();
    for raw in &raws {
        if let Some(keys) = keys_from_originals(&raw.originals) {
            for key in keys {
                readings
                    .entry(key)
                    .or_default()
                    .insert(normalize_word(&raw.word));
            }
        }
    }
    let mut rules = Vec::new();
    let mut collision_keys = BTreeSet::new();
    let mut audit = Vec::new();
    for raw in raws.iter().filter(|r| r.unit == "구") {
        let mut record = AuditRecord {
            original_spellings: raw
                .originals
                .iter()
                .filter_map(|o| o.original_language.clone())
                .collect(),
            categories: raw.categories.iter().cloned().collect(),
            source_id: raw.id,
            word: raw.word.clone(),
            outcome: "admitted".into(),
            terminal_extensions: 0,
            terminal_outcome: "none".into(),
            terminal_withheld: Vec::new(),
        };
        let result = (|| {
            let chunks = strict_chunks(raw)?;
            if ids.get(&raw.id.unwrap()).is_none_or(|rows| rows.len() != 1) {
                return Err("duplicate-word-id");
            }
            if raw
                .categories
                .iter()
                .any(|c| policy.excluded_categories.contains(&c.as_str()))
            {
                return Err("excluded-category");
            }
            let key = chunks.iter().map(|c| c.hanja.as_str()).collect::<String>();
            let reading = chunks
                .iter()
                .map(|c| c.reading.as_str())
                .collect::<String>();
            if readings
                .get(&key)
                .is_some_and(|r| r.iter().any(|r| r != &reading))
            {
                return Err("spelling-reading-collision");
            }
            let mut output = Vec::new();
            for chunk in &chunks {
                let mut spellings = vec![PhraseSpelling::new(
                    chunk.hanja.clone(),
                    links(&extractor, &chunk.hanja, &chunk.reading),
                )];
                if let Some((alternatives, witnesses)) = donors.lookup(chunk) {
                    if alternatives.len() > MAX_ALTERNATIVES || witnesses.len() > MAX_WITNESSES {
                        return Err("sibling-limit");
                    }
                    for alternative in alternatives {
                        if alternative == &chunk.hanja {
                            continue;
                        }
                        let alternative_key = chunks
                            .iter()
                            .enumerate()
                            .map(|(i, c)| {
                                if i == output.len() {
                                    alternative.as_str()
                                } else {
                                    c.hanja.as_str()
                                }
                            })
                            .collect::<String>();
                        // Quarantine the complete derived spelling on disagreement.
                        if readings
                            .get(&alternative_key)
                            .is_some_and(|r| r.iter().any(|r| r != &reading))
                        {
                            collision_keys.insert(alternative_key);
                            return Err("sibling-spelling-reading-collision");
                        }
                        spellings.push(
                            PhraseSpelling::new(
                                alternative.clone(),
                                links(&extractor, alternative, &chunk.reading),
                            )
                            .with_sibling_witnesses(
                                witnesses.iter().copied().map(WordId).collect(),
                            ),
                        );
                    }
                }
                output.push(PhraseChunk::new(chunk.reading.clone(), spellings));
            }
            let keys = complete_keys(&output).ok_or("combination-limit")?;
            let collisions = keys
                .iter()
                .filter(|key| {
                    readings
                        .get(*key)
                        .is_some_and(|r| r.iter().any(|r| r != &reading))
                })
                .cloned()
                .collect::<Vec<_>>();
            if !collisions.is_empty() {
                collision_keys.extend(collisions);
                return Err("derived-spelling-reading-collision");
            }
            let (mut extensions, terminal_outcome) =
                extensions(&words_by_reading, &ids, &extractor, output.last().unwrap());
            record.terminal_outcome = terminal_outcome.into();
            extensions.retain(|extension| {
                let Some(keys) = complete_terminal_keys(&output, extension) else {
                    record.terminal_withheld.push(WithheldTerminal {
                        literal_suffix: extension.literal_suffix.clone(),
                        reason: "terminal-combination-limit",
                    });
                    return false;
                };
                let projected_reading = format!("{reading}{}", extension.literal_suffix);
                let collisions = keys
                    .into_iter()
                    .filter(|key| {
                        readings
                            .get(key)
                            .is_some_and(|r| r.iter().any(|r| r != &projected_reading))
                    })
                    .collect::<Vec<_>>();
                if collisions.is_empty() {
                    return true;
                }
                collision_keys.extend(collisions);
                record.terminal_withheld.push(WithheldTerminal {
                    literal_suffix: extension.literal_suffix.clone(),
                    reason: "raw-terminal-spelling-reading-collision",
                });
                false
            });
            record_terminal_count(&mut record, extensions.len());
            let rule = PhraseRule::new(vec![WordId(raw.id.unwrap())], output)
                .with_terminal_extensions(extensions);
            if rule.max_span_chars() > MAX_SPAN {
                return Err("extended-span-limit");
            }
            if PhraseEvidence::new(vec![rule.clone()]).validate().is_err() {
                return Err("typed-structure-or-limit");
            }
            Ok(rule)
        })();
        match result {
            Ok(rule) => rules.push(rule),
            Err(reason) => record_whole_withheld(&mut record, reason),
        }
        audit.push(record);
    }
    let mut derived_readings = BTreeMap::<String, BTreeSet<String>>::new();
    for rule in &rules {
        let reading = complete_reading(&rule.chunks);
        for key in complete_keys(&rule.chunks).expect("admission checks product") {
            derived_readings
                .entry(key)
                .or_default()
                .insert(reading.clone());
        }
        // With pure hanja base spellings and literal hangul suffixes, a
        // conflicting projected reading also implies a base conflict. Keep
        // projections in this inventory explicitly rather than relying on that
        // representation invariant for collision safety.
        for extension in &rule.terminal_extensions {
            let projected_reading = format!("{reading}{}", extension.literal_suffix);
            for key in
                complete_terminal_keys(&rule.chunks, extension).expect("admission checks product")
            {
                derived_readings
                    .entry(key)
                    .or_default()
                    .insert(projected_reading.clone());
            }
        }
    }
    let collides = |key: &String| {
        collision_keys.contains(key)
            || derived_readings
                .get(key)
                .is_some_and(|readings| readings.len() > 1)
    };
    rules.retain_mut(|rule| {
        let mut withheld = Vec::new();
        rule.terminal_extensions.retain(|extension| {
            if complete_terminal_keys(&rule.chunks, extension)
                .expect("admission checks product")
                .iter()
                .any(&collides)
            {
                withheld.push(WithheldTerminal {
                    literal_suffix: extension.literal_suffix.clone(),
                    reason: "derived-terminal-spelling-reading-collision",
                });
                false
            } else {
                true
            }
        });
        let collision = complete_keys(&rule.chunks)
            .expect("admission checks product")
            .iter()
            .any(&collides);
        if collision || !withheld.is_empty() {
            let record = audit
                .iter_mut()
                .find(|record| {
                    record
                        .source_id
                        .is_some_and(|id| rule.source_ids.contains(&WordId(id)))
                })
                .expect("admitted rule has an audit record");
            record.terminal_withheld.extend(withheld);
            if collision {
                record_whole_withheld(record, "derived-phrase-collision");
            } else {
                record_terminal_count(record, rule.terminal_extensions.len());
            }
        }
        !collision
    });
    rules.sort();
    rules.dedup();
    // Global overflow withholds all rules; taking an arbitrary prefix would
    // hide constraints that may conflict with admitted rules.
    if rules.len() > MAX_RULES {
        rules.clear();
        for record in &mut audit {
            if record.outcome == "admitted" {
                record_whole_withheld(record, "rule-limit");
            }
        }
    }
    let evidence = PhraseEvidence::new(rules);
    evidence
        .validate()
        .map_err(|e| Error::DefaultDecision(e.to_string()))?;
    serde_json::to_writer(&mut evidence_writer, &evidence)?;
    writeln!(evidence_writer)?;
    let limits = BTreeMap::from([
        ("max_combinations", MAX_COMBINATIONS),
        ("max_chunks", MAX_CHUNKS),
        ("max_span_chars", MAX_SPAN),
        ("max_alternatives", MAX_ALTERNATIVES),
        ("max_witnesses", MAX_WITNESSES),
        ("max_rules", MAX_RULES),
        ("max_extension_chars", MAX_EXTENSION),
    ]);
    serde_json::to_writer(
        &mut manifest,
        &Audit {
            schema_version: 1,
            policy_version: 1,
            limits,
            excluded_categories: policy.excluded_categories.to_vec(),
            separator_policy: "caret-zero-or-one-u0020;hyphen-inside-chunk",
            records: audit,
        },
    )?;
    writeln!(manifest)?;
    Ok(())
}

fn complete_reading(chunks: &[PhraseChunk]) -> String {
    chunks.iter().map(|chunk| chunk.reading.as_str()).collect()
}

fn record_whole_withheld(record: &mut AuditRecord, reason: &str) {
    record.outcome = format!("withheld:{reason}");
    record.terminal_extensions = 0;
    if matches!(
        record.terminal_outcome.as_str(),
        "validated" | "partially-withheld"
    ) {
        record.terminal_outcome = "withheld".into();
    }
}

fn record_terminal_count(record: &mut AuditRecord, count: usize) {
    record.terminal_extensions = count;
    if !record.terminal_withheld.is_empty() {
        record.terminal_outcome = if count == 0 {
            "withheld"
        } else {
            "partially-withheld"
        }
        .into();
    }
}

fn complete_terminal_keys(
    chunks: &[PhraseChunk],
    extension: &TerminalExtension,
) -> Option<Vec<String>> {
    let prefixes = complete_keys(&chunks[..chunks.len().checked_sub(1)?])?;
    if prefixes.len().checked_mul(extension.spellings.len())? > MAX_COMBINATIONS {
        return None;
    }
    Some(
        prefixes
            .iter()
            .flat_map(|prefix| {
                extension
                    .spellings
                    .iter()
                    .map(move |spelling| format!("{prefix}{}", spelling.key))
            })
            .collect(),
    )
}

fn complete_keys(chunks: &[PhraseChunk]) -> Option<Vec<String>> {
    let mut keys = vec![String::new()];
    for chunk in chunks {
        if keys.len().checked_mul(chunk.spellings.len())? > MAX_COMBINATIONS {
            return None;
        }
        keys = keys
            .iter()
            .flat_map(|prefix| {
                chunk
                    .spellings
                    .iter()
                    .map(move |s| format!("{prefix}{}", s.key))
            })
            .collect();
    }
    Some(keys)
}

// Every possible parent must explicitly license the same suffix and spelling
// set through a unique typed word target. Sense identifiers are never consulted.
fn extensions(
    words_by_reading: &BTreeMap<String, Vec<&RawWord>>,
    ids: &BTreeMap<u64, Vec<&RawWord>>,
    extractor: &Extractor,
    chunk: &PhraseChunk,
) -> (Vec<TerminalExtension>, &'static str) {
    let parents = words_by_reading
        .get(&chunk.reading)
        .into_iter()
        .flatten()
        .copied()
        .filter(|r| {
            keys_from_originals(&r.originals)
                .is_some_and(|keys| chunk.spellings.iter().any(|s| keys.contains(&s.key)))
        })
        .collect::<Vec<_>>();
    if parents.is_empty() {
        return (Vec::new(), "no-parent");
    }
    let mut consensus: Option<BTreeMap<String, BTreeSet<String>>> = None;
    let mut witnesses = BTreeMap::<String, Vec<SubheadwordWitness>>::new();
    for parent in parents {
        let Some(parent_id) = parent
            .id
            .filter(|id| ids.get(id).is_some_and(|rows| rows.len() == 1))
        else {
            return (Vec::new(), "ambiguous-parent");
        };
        if explicit_original_keys(&parent.originals).is_none() {
            return (Vec::new(), "unsupported-parent-original");
        }
        if parent.children.is_empty() {
            return (Vec::new(), "parent-without-subheadword");
        }
        let mut local = BTreeMap::<String, BTreeSet<String>>::new();
        for child_id in &parent.children {
            let Some(child_id) = child_id else {
                return (Vec::new(), "invalid-word-target");
            };
            let Some(children) = ids.get(child_id).filter(|rows| rows.len() == 1) else {
                return (Vec::new(), "missing-or-ambiguous-word-target");
            };
            let child = children[0];
            if parent.categories.iter().chain(&child.categories).any(|c| {
                AliasPolicy::default()
                    .excluded_categories
                    .contains(&c.as_str())
            }) {
                return (Vec::new(), "excluded-extension-category");
            }
            if child.unit != "단어" {
                return (Vec::new(), "target-not-word");
            }
            let child_word = child.word.trim_end_matches(|ch: char| ch.is_ascii_digit());
            let Some((base_word, suffix)) = child_word.rsplit_once('-') else {
                return (Vec::new(), "target-headword-mismatch");
            };
            if normalize_word(base_word) != chunk.reading {
                return (Vec::new(), "target-headword-mismatch");
            }
            if suffix.is_empty()
                || suffix.chars().count() > MAX_EXTENSION
                || !suffix.chars().all(hangul)
            {
                return (Vec::new(), "invalid-literal-suffix");
            }
            let Some(keys) = explicit_original_keys(&child.originals) else {
                return (Vec::new(), "missing-target-original");
            };
            let reading = format!("{}{suffix}", chunk.reading);
            let mut spellings = BTreeSet::new();
            for base in &chunk.spellings {
                let key = format!("{}{suffix}", base.key);
                if !keys.contains(&key) || links(extractor, &key, &reading).is_empty() {
                    return (Vec::new(), "target-spelling-or-candidate-mismatch");
                }
                spellings.insert(key);
            }
            local.entry(suffix.into()).or_default().extend(spellings);
            witnesses
                .entry(suffix.into())
                .or_default()
                .push(SubheadwordWitness::new(
                    WordId(parent_id),
                    WordId(*child_id),
                ));
        }
        if consensus
            .as_ref()
            .is_some_and(|expected| expected != &local)
        {
            return (Vec::new(), "parent-disagreement");
        }
        consensus = Some(local);
    }
    let mut output = Vec::new();
    for (suffix, keys) in consensus.unwrap_or_default() {
        let reading = format!("{}{suffix}", chunk.reading);
        let mut witness = witnesses.remove(&suffix).unwrap_or_default();
        witness.sort();
        witness.dedup();
        if witness.len() > MAX_WITNESSES || keys.len() > MAX_ALTERNATIVES {
            return (Vec::new(), "extension-limit");
        }
        let spellings = chunk
            .spellings
            .iter()
            .filter_map(|base| {
                let key = format!("{}{suffix}", base.key);
                keys.contains(&key).then(|| {
                    PhraseSpelling::new(key.clone(), links(extractor, &key, &reading))
                        .with_sibling_witnesses(base.sibling_witnesses.clone())
                })
            })
            .collect();
        output.push(TerminalExtension::new(suffix, spellings, witness));
    }
    if output.len() > MAX_ALTERNATIVES {
        return (Vec::new(), "extension-limit");
    }
    (output, "validated")
}

// Raw explicit spelling groups only. Recognition-oriented extraction helpers may
// extract hanja from foreign names; those transformations cannot prove a literal
// subheadword projection.
fn explicit_original_keys(originals: &[OriginalLanguageInfo]) -> Option<Vec<String>> {
    let mut keys = Vec::new();
    let mut key = String::new();
    for original in originals {
        let text = original.original_language.as_deref()?;
        match original.language_type.as_deref()? {
            "/(병기)" if text == "/" && !key.is_empty() => keys.push(std::mem::take(&mut key)),
            "한자" if !text.is_empty() && text.chars().all(is_hanja) => key.push_str(text),
            "고유어" if !text.is_empty() && text.chars().all(hangul) => key.push_str(text),
            _ => return None,
        }
    }
    if key.is_empty() {
        return None;
    }
    keys.push(key);
    Some(keys)
}

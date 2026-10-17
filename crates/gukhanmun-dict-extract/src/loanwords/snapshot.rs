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

use super::{Error, canonical_json, parse_json, sha256};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Fixed official HTTPS endpoint; authenticated URLs must never be logged.
pub const ENDPOINT: &str = "https://korean.go.kr/kornorms/exampleReqList.do";

/// One opaque source identity with all original record fields preserved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// Nonempty ASCII letters, digits, hyphens and underscores only.
    pub id: String,
    /// Original record object, including unknown fields and unchanged strings.
    pub fields: Value,
}

fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str, Error> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("missing or non-string record field"))
}
fn id(v: &Value) -> Result<String, Error> {
    let s = match v {
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_u64() || n.is_i64() => n.to_string(),
        _ => return Err(Error::Invalid("record ID must be string or integer")),
    };
    if s.is_empty()
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(Error::Invalid("unsafe record ID"));
    }
    Ok(s)
}
impl Record {
    /// Validates identity, nullable spellings and selector without inventing status.
    /// Null spellings remain source evidence and are excluded by the lookup audit.
    pub fn from_fields(fields: Value) -> Result<Self, Error> {
        if !fields.is_object() {
            return Err(Error::Invalid("record is not an object"));
        }
        let id = id(fields
            .get("example_no")
            .ok_or(Error::Invalid("missing record ID"))?)?;
        for field in ["srclang_mark", "korean_mark"] {
            if !fields
                .get(field)
                .is_some_and(|v| v.is_null() || v.is_string())
            {
                return Err(Error::Invalid("missing or invalid record spelling"));
            }
        }
        if text(&fields, "regltn_code")? != "0003" {
            return Err(Error::Invalid("wrong dataset selector"));
        }
        Ok(Self { id, fields })
    }
    /// Hashes the entire source record, including fields not used for lookup.
    pub fn fingerprint(&self) -> String {
        sha256(&canonical_json(&self.fields))
    }
}

fn alias<'a>(v: &'a Value, a: &str, b: &str) -> Result<&'a Value, Error> {
    match (v.get(a), v.get(b)) {
        (Some(x), None) | (None, Some(x)) => Ok(x),
        _ => Err(Error::Invalid("missing or simultaneous envelope aliases")),
    }
}
fn integer(v: &Value) -> Result<u64, Error> {
    v.as_u64()
        .ok_or(Error::Invalid("count must be an unsigned integer"))
}

/// A validated API response page; operational session/IP metadata is discarded.
#[derive(Clone, Debug)]
pub struct Page {
    /// Echoed one-based page number.
    pub number: u64,
    /// Declared total record count, checked across the whole sweep.
    pub total: u64,
    /// Complete records for this page, including non-hanja originals.
    pub records: Vec<Record>,
}
impl Page {
    /// Parses documented camel-case or lowercase envelope names, never both.
    /// Accepts the live `response` envelope with numeric success code zero and
    /// the legacy flat experimental envelope. Requires page/count echoes and items.
    pub fn parse(bytes: &[u8], expected_page: u64, rows: u64) -> Result<Self, Error> {
        let envelope = parse_json(bytes)?;
        let live = envelope.get("response");
        if live.is_some()
            && [
                "resultCode",
                "resultcode",
                "resultMsg",
                "resultmsg",
                "pageNo",
                "pageno",
                "numOfRows",
                "numofrows",
                "totalCount",
                "totalcount",
                "items",
            ]
            .iter()
            .any(|k| envelope.get(k).is_some())
        {
            return Err(Error::Invalid("ambiguous response envelope"));
        }
        let v = live.unwrap_or(&envelope);
        let result_code = alias(v, "resultCode", "resultcode")?;
        let success = if live.is_some() {
            result_code.as_u64() == Some(0)
        } else {
            result_code.as_str() == Some("00")
        };
        if !success {
            return Err(Error::Invalid("API result is not success"));
        }
        let total = integer(alias(v, "totalCount", "totalcount")?)?;
        let number = integer(alias(v, "pageNo", "pageno")?)?;
        if number == 0 {
            return Err(Error::Invalid("page number must be positive"));
        }
        alias(v, "resultMsg", "resultmsg")?
            .as_str()
            .ok_or(Error::Invalid("missing result message"))?;
        if number != expected_page
            || rows == 0
            || integer(alias(v, "numOfRows", "numofrows")?)? != rows
        {
            return Err(Error::Invalid("pagination echo mismatch"));
        }
        let items = v
            .get("items")
            .and_then(Value::as_array)
            .ok_or(Error::Invalid("missing items array"))?;
        let offset = (number - 1)
            .checked_mul(rows)
            .ok_or(Error::Invalid("page overflow"))?;
        if offset > total || items.len() as u64 != rows.min(total - offset) {
            return Err(Error::Invalid("page length disagrees with total"));
        }
        let records = items
            .iter()
            .cloned()
            .map(Record::from_fields)
            .collect::<Result<Vec<_>, _>>()?;
        let mut seen = BTreeSet::new();
        if records.iter().any(|r| !seen.insert(&r.id)) {
            return Err(Error::Invalid("duplicate record ID"));
        }
        Ok(Self {
            number,
            total,
            records,
        })
    }
}

/// Self-contained versioned snapshot. Records are sorted by opaque identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    /// Snapshot format, currently 1.
    pub format_version: u32,
    /// Fixed official endpoint, without credentials.
    pub endpoint: String,
    /// Dataset selector, currently 0003.
    pub selector: String,
    /// Caller-supplied collection timestamp (audit metadata, not hash input).
    pub collected_at: String,
    /// Collector software version.
    pub collector_version: String,
    /// Count of distinct source records.
    pub total_count: u64,
    /// SHA-256 of canonical record fields in opaque-ID order.
    pub records_sha256: String,
    /// All records, including excluded/non-hanja records.
    pub records: Vec<Record>,
}
impl Snapshot {
    /// Assembles a complete sweep, rejecting duplicate identities and count drift.
    pub fn from_pages(pages: Vec<Page>, collected_at: String) -> Result<Self, Error> {
        let total = pages.first().ok_or(Error::Invalid("no pages"))?.total;
        let mut records = Vec::new();
        for (i, p) in pages.into_iter().enumerate() {
            if p.number != i as u64 + 1 || p.total != total {
                return Err(Error::Invalid("inconsistent pages or totals"));
            }
            records.extend(p.records);
        }
        let distinct = records.iter().map(|r| &r.id).collect::<BTreeSet<_>>().len() as u64;
        if records.len() as u64 != total || distinct != total {
            return Err(Error::CountMismatch {
                declared: total,
                distinct,
            });
        }
        Self::new(records, collected_at)
    }
    /// Creates a snapshot from independently supplied records, preserving content.
    pub fn new(mut records: Vec<Record>, collected_at: String) -> Result<Self, Error> {
        records.sort_by(|a, b| a.id.cmp(&b.id));
        let mut seen = BTreeSet::new();
        for r in &records {
            if Record::from_fields(r.fields.clone())?.id != r.id || !seen.insert(&r.id) {
                return Err(Error::Invalid("duplicate or inconsistent record ID"));
            }
        }
        let hash = sha256(&canonical_json(&Value::Array(
            records.iter().map(|r| r.fields.clone()).collect(),
        )));
        Ok(Self {
            format_version: 1,
            endpoint: ENDPOINT.into(),
            selector: "0003".into(),
            collected_at,
            collector_version: env!("CARGO_PKG_VERSION").into(),
            total_count: records.len() as u64,
            records_sha256: hash,
            records,
        })
    }
    /// Parses and verifies the canonical order, schema, count and content checksum.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let value = parse_json(bytes)?;
        let s: Self =
            serde_json::from_value(value).map_err(|_| Error::Invalid("snapshot schema"))?;
        let rebuilt = Self::new(s.records.clone(), s.collected_at.clone())?;
        if s.format_version != 1
            || s.endpoint != ENDPOINT
            || s.selector != "0003"
            || s.total_count != rebuilt.total_count
            || s.records_sha256 != rebuilt.records_sha256
            || s.records != rebuilt.records
        {
            return Err(Error::Invalid("snapshot integrity"));
        }
        Ok(s)
    }
    /// Serializes deterministic JSON with one record per line and collection metadata.
    pub fn bytes(&self) -> Vec<u8> {
        let mut value = json!(self);
        value.as_object_mut().unwrap().remove("records");
        let mut bytes = canonical_json(&value);
        bytes.pop();
        bytes.extend_from_slice(b",\n\"records\":[\n");
        for (index, record) in self.records.iter().enumerate() {
            if index > 0 {
                bytes.extend_from_slice(b",\n");
            }
            bytes.extend(canonical_json(&json!(record)));
        }
        bytes.extend_from_slice(b"\n]}\n");
        bytes
    }
    /// Compares whole-record evidence, capturing corrections and deletions.
    pub fn diff(&self, old: &Self) -> SnapshotDiff {
        let before: BTreeMap<_, _> = old
            .records
            .iter()
            .map(|r| (&r.id, r.fingerprint()))
            .collect();
        let now: BTreeMap<_, _> = self
            .records
            .iter()
            .map(|r| (&r.id, r.fingerprint()))
            .collect();
        SnapshotDiff {
            added: now
                .keys()
                .filter(|id| !before.contains_key(*id))
                .map(|id| (*id).clone())
                .collect(),
            removed: before
                .keys()
                .filter(|id| !now.contains_key(*id))
                .map(|id| (*id).clone())
                .collect(),
            changed: now
                .iter()
                .filter(|(id, h)| before.get(*id).is_some_and(|old| old != *h))
                .map(|(id, _)| (*id).clone())
                .collect(),
        }
    }
}
/// Deterministic source-identity changes between full snapshots.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotDiff {
    /// Newly observed identities.
    pub added: Vec<String>,
    /// Identities absent from the new full sweep.
    pub removed: Vec<String>,
    /// Same identity with any preserved field changed.
    pub changed: Vec<String>,
}

// Gukhanmun: Allocation preflight for untrusted dictionary container metadata.
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

use crate::PhraseEvidence;

/// Admission charge for ordinary metadata and its decoded alias index.
///
/// This is an allocation estimate, not an RSS limit. Container backing storage,
/// dictionary indexes and allocator bookkeeping have separate costs.
pub const MAX_METADATA_ALLOCATION_CHARGE: usize = 64 * 1024 * 1024;
/// Separate admission charge for the larger typed candidate and phrase evidence.
pub const MAX_CRITICAL_METADATA_ALLOCATION_CHARGE: usize = 1024 * 1024 * 1024;
/// Maximum CBOR items, including keys, values, tags and indefinite string chunks.
pub const MAX_METADATA_ITEMS: usize = 8 * 1024 * 1024;
/// Admission charge for the derived phrase lookup index.
pub const MAX_PHRASE_INDEX_ALLOCATION_CHARGE: usize = 64 * 1024 * 1024;

const ITEM_CHARGE: usize = 128;
const STRING_BYTE_CHARGE: usize = 4;
// Reserve 1.5 KiB per row for the nested B-tree index; string ownership is
// also covered by STRING_BYTE_CHARGE. This policy leaves room for 32,768 short
// aliases within the ordinary budget without claiming an allocator/RSS bound.
// Charge every row, including duplicates and ignored malformed rows, rather
// than allocating a deduplication table during admission.
const ALIAS_ROW_CHARGE: usize = 1536;
const MAX_DEPTH: usize = 32;

/// Failure of allocation-free metadata admission before typed deserialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum MetadataResourceError {
    /// An aggregate allocation, item-count or nesting budget was exceeded.
    #[error("dictionary metadata exceeds {resource} limit")]
    LimitExceeded {
        /// The exhausted resource, suitable for a container load error.
        resource: &'static str,
    },
    /// CBOR structure is incomplete or malformed.
    #[error("invalid dictionary metadata CBOR at byte {offset}")]
    InvalidCbor {
        /// Byte position of the invalid structure.
        offset: usize,
    },
}

fn limit(resource: &'static str) -> MetadataResourceError {
    MetadataResourceError::LimitExceeded { resource }
}

#[derive(Clone, Copy)]
struct Limits {
    ordinary: usize,
    critical: usize,
    items: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            ordinary: MAX_METADATA_ALLOCATION_CHARGE,
            critical: MAX_CRITICAL_METADATA_ALLOCATION_CHARGE,
            items: MAX_METADATA_ITEMS,
        }
    }
}

/// Checks dictionary CBOR and alias expansion without allocating decoded values.
///
/// Both container backends call this before their existing serde decoders. Every
/// CBOR item costs 128 admission bytes and every string/byte-string byte costs
/// four, accounting for owned buffers, vector growth, wire-to-typed conversion
/// and validation collections. Alias rows cost a further 1536 bytes each for
/// nested maps and owned strings. Charges are deliberately conservative policy
/// estimates, not an allocator-specific bound on resident memory.
///
/// Ordinary metadata (including aliases) shares a 64 MiB charge; the two critical
/// evidence fields share a separate 1 GiB charge to accommodate bundled source
/// evidence. All fields also share an eight-million-item limit and nesting is
/// limited to 32 levels. Declared lengths are checked before traversing children;
/// indefinite collections, string chunks and tags consume the same budgets.
/// Duplicate fields are counted independently and remain errors in the typed
/// decoder. This does not replace semantic or candidate-link validation.
///
/// Only the first CBOR value is inspected, preserving legacy containers' trailing
/// byte behavior. Critical containers must still reject trailing bytes afterward.
pub fn check_metadata_resources(bytes: &[u8]) -> Result<(), MetadataResourceError> {
    check_with_limits(bytes, Limits::default())
}

fn check_with_limits(bytes: &[u8], limits: Limits) -> Result<(), MetadataResourceError> {
    let mut scanner = Scanner {
        bytes,
        offset: 0,
        limits,
    };
    scanner.item(0, false, false, true).map(|_| ())
}

// Only top-level field names need recognition. A fixed buffer also recognizes
// chunked and tagged strings without allocating attacker-controlled key storage.
#[derive(Default)]
struct Key {
    bytes: [u8; 18],
    len: usize,
}
impl Key {
    fn extend(&mut self, bytes: &[u8]) {
        if let Some(end) = self.len.checked_add(bytes.len()) {
            if end <= self.bytes.len() {
                self.bytes[self.len..end].copy_from_slice(bytes);
            }
            self.len = end;
        } else {
            self.len = usize::MAX;
        }
    }
    fn is(&self, expected: &[u8]) -> bool {
        self.len == expected.len() && self.bytes.get(..self.len) == Some(expected)
    }
}

struct Scanner<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: Limits,
}
impl Scanner<'_> {
    fn invalid(&self) -> MetadataResourceError {
        MetadataResourceError::InvalidCbor {
            offset: self.offset,
        }
    }
    fn take(&mut self, len: usize) -> Result<&[u8], MetadataResourceError> {
        let end = self.offset.checked_add(len).ok_or_else(|| self.invalid())?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| self.invalid())?;
        self.offset = end;
        Ok(bytes)
    }
    fn charge(&mut self, amount: usize, critical: bool) -> Result<(), MetadataResourceError> {
        let (remaining, resource) = if critical {
            (
                &mut self.limits.critical,
                "critical metadata allocation charge",
            )
        } else {
            (
                &mut self.limits.ordinary,
                "ordinary metadata allocation charge",
            )
        };
        *remaining = remaining
            .checked_sub(amount)
            .ok_or_else(|| limit(resource))?;
        Ok(())
    }
    fn items_fit(&self, count: usize, critical: bool) -> Result<(), MetadataResourceError> {
        if count > self.limits.items {
            return Err(limit("CBOR item count"));
        }
        let remaining = if critical {
            self.limits.critical
        } else {
            self.limits.ordinary
        };
        if count > remaining / ITEM_CHARGE {
            return Err(limit(if critical {
                "critical metadata allocation charge"
            } else {
                "ordinary metadata allocation charge"
            }));
        }
        Ok(())
    }
    fn header(&mut self) -> Result<(u8, Option<u64>), MetadataResourceError> {
        let byte = self.take(1)?[0];
        let argument = match byte & 31 {
            value @ 0..=23 => Some(u64::from(value)),
            width @ 24..=27 => {
                let bytes = self.take(1 << (width - 24))?;
                let mut value = 0u64;
                for byte in bytes {
                    value = (value << 8) | u64::from(*byte);
                }
                Some(value)
            }
            31 => None,
            _ => return Err(self.invalid()),
        };
        Ok((byte >> 5, argument))
    }
    fn length(&self, value: u64) -> Result<usize, MetadataResourceError> {
        usize::try_from(value).map_err(|_| limit("declared CBOR length"))
    }
    fn at_break(&mut self) -> bool {
        if self.bytes.get(self.offset) == Some(&0xff) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
    fn item(
        &mut self,
        depth: usize,
        critical: bool,
        aliases: bool,
        root: bool,
    ) -> Result<Key, MetadataResourceError> {
        if depth > MAX_DEPTH {
            return Err(limit("CBOR nesting depth"));
        }
        self.items_fit(1, critical)?;
        self.limits.items -= 1;
        self.charge(ITEM_CHARGE, critical)?;
        let (major, argument) = self.header()?;
        let mut key = Key::default();
        match (major, argument) {
            (0 | 1 | 7, Some(_)) => {}
            (2 | 3, length) => {
                if aliases {
                    self.charge(ALIAS_ROW_CHARGE, false)?;
                }
                if let Some(length) = length {
                    self.string_chunk(length, critical, aliases, &mut key)?;
                } else {
                    while !self.at_break() {
                        self.items_fit(1, critical)?;
                        self.limits.items -= 1;
                        self.charge(ITEM_CHARGE, critical)?;
                        let (chunk_major, Some(length)) = self.header()? else {
                            return Err(self.invalid());
                        };
                        if chunk_major != major {
                            return Err(self.invalid());
                        }
                        self.string_chunk(length, critical, aliases, &mut key)?;
                    }
                }
            }
            (4 | 5, length) => {
                let count = length.map(|n| self.length(n)).transpose()?;
                if let Some(count) = count {
                    let children = count
                        .checked_mul(if major == 5 { 2 } else { 1 })
                        .ok_or_else(|| limit("declared CBOR length"))?;
                    // Root map values may belong to the larger critical budget.
                    // Item count still bounds a dishonest root-map size hint.
                    if root && major == 5 {
                        if children > self.limits.items {
                            return Err(limit("CBOR item count"));
                        }
                    } else {
                        self.items_fit(children, critical)?;
                    }
                }
                let mut remaining = count;
                loop {
                    match remaining {
                        Some(0) => break,
                        None if self.at_break() => break,
                        _ => {}
                    }
                    let field = self.item(depth + 1, critical, false, false)?;
                    if major == 5 {
                        let is_critical = critical
                            || (root
                                && (field.is(b"candidate_evidence")
                                    || field.is(b"phrase_evidence")));
                        self.item(depth + 1, is_critical, root && field.is(b"aliases"), false)?;
                    }
                    if let Some(left) = &mut remaining {
                        *left -= 1;
                    }
                }
            }
            (6, Some(_)) => return self.item(depth + 1, critical, aliases, root),
            _ => return Err(self.invalid()),
        }
        Ok(key)
    }
    fn string_chunk(
        &mut self,
        length: u64,
        critical: bool,
        aliases: bool,
        key: &mut Key,
    ) -> Result<(), MetadataResourceError> {
        let length = self.length(length)?;
        let charge = length
            .checked_mul(STRING_BYTE_CHARGE)
            .ok_or_else(|| limit("metadata string allocation charge"))?;
        self.charge(charge, critical)?;
        // Count rows across chunk boundaries; a trailing newline is conservatively
        // charged as one additional row. No joined string is built by preflight.
        let bytes = self.take(length)?;
        let rows = if aliases {
            bytes.iter().filter(|byte| **byte == b'\n').count()
        } else {
            0
        };
        key.extend(bytes);
        let charge = rows
            .checked_mul(ALIAS_ROW_CHARGE)
            .ok_or_else(|| limit("alias allocation charge"))?;
        self.charge(charge, false)
    }
}

/// Checks phrase-index expansion before allocating prefix maps or handle vectors.
///
/// Each first-chunk spelling and reading is inserted for every terminal extension
/// plus the base rule. Charge every insertion 512 bytes plus four times its key
/// length, covering singleton maps, string copies and vector growth without
/// allocating a second index. The 64 MiB policy charge is not an RSS guarantee.
/// Call this after CBOR admission and before validation/index construction; it
/// does not validate phrase semantics or replace existing structural checks.
pub fn check_phrase_index_resources(
    evidence: &PhraseEvidence,
) -> Result<(), MetadataResourceError> {
    check_phrase_with_limit(evidence, MAX_PHRASE_INDEX_ALLOCATION_CHARGE)
}
fn check_phrase_with_limit(
    evidence: &PhraseEvidence,
    mut remaining: usize,
) -> Result<(), MetadataResourceError> {
    for rule in &evidence.rules {
        let Some(first) = rule.chunks.first() else {
            continue;
        };
        let copies = rule
            .terminal_extensions
            .len()
            .checked_add(1)
            .ok_or_else(|| limit("phrase index allocation charge"))?;
        for key in first
            .spellings
            .iter()
            .map(|spelling| spelling.key.as_str())
            .chain(core::iter::once(first.reading.as_str()))
        {
            let charge = key
                .len()
                .checked_mul(4)
                .and_then(|bytes| bytes.checked_add(512))
                .and_then(|bytes| bytes.checked_mul(copies))
                .ok_or_else(|| limit("phrase index allocation charge"))?;
            remaining = remaining
                .checked_sub(charge)
                .ok_or_else(|| limit("phrase index allocation charge"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PhraseChunk, PhraseRule, PhraseSpelling, TerminalExtension, WordId};
    use alloc::{vec, vec::Vec};

    fn ordinary(bytes: usize, items: usize) -> Limits {
        Limits {
            ordinary: bytes,
            critical: 4096,
            items,
        }
    }

    #[test]
    fn exact_allocation_and_item_boundaries_are_inclusive() {
        // One map, two strings and two payload bytes.
        let input = b"\xa1\x61k\x61v";
        let charge = 3 * ITEM_CHARGE + 2 * STRING_BYTE_CHARGE;
        assert_eq!(check_with_limits(input, ordinary(charge, 3)), Ok(()));
        assert!(check_with_limits(input, ordinary(charge - 1, 3)).is_err());
        assert!(check_with_limits(input, ordinary(charge, 2)).is_err());
    }

    #[test]
    fn collection_headers_cannot_allocate_from_untrusted_hints() {
        for header in [0x9b, 0xbb, 0x7b, 0x5b] {
            let mut input = vec![header];
            input.extend_from_slice(&u64::MAX.to_be_bytes());
            assert!(matches!(
                check_metadata_resources(&input),
                Err(MetadataResourceError::LimitExceeded { .. })
            ));
        }
        // The same protection applies inside candidate/phrase evidence, including
        // empty arrays that would otherwise each allocate a Vec's minimum capacity.
        for field in [b"candidate_evidence".as_slice(), b"phrase_evidence"] {
            for header in [0x9b, 0xbb, 0x7b, 0x5b] {
                let mut input = vec![0xa1, 0x60 + field.len() as u8];
                input.extend_from_slice(field);
                input.push(header);
                input.extend_from_slice(&u64::MAX.to_be_bytes());
                assert!(matches!(
                    check_metadata_resources(&input),
                    Err(MetadataResourceError::LimitExceeded { .. })
                ));
            }
        }
    }

    #[test]
    fn aliases_are_counted_before_tiny_rows_expand_into_nested_maps() {
        let row = b"a\tb\tlexical\tc";
        let mut input = b"\xa1\x67aliases".to_vec();
        input.push(0x60 + row.len() as u8);
        input.extend_from_slice(row);
        let charge = 3 * ITEM_CHARGE + (7 + row.len()) * STRING_BYTE_CHARGE + ALIAS_ROW_CHARGE;
        assert_eq!(check_with_limits(&input, ordinary(charge, 3)), Ok(()));
        assert!(check_with_limits(&input, ordinary(charge - 1, 3)).is_err());
        let mut duplicate = b"\xa2".to_vec();
        duplicate.extend_from_slice(&input[1..]);
        duplicate.extend_from_slice(&input[1..]);
        assert!(check_with_limits(&duplicate, ordinary(charge, 20)).is_err());
    }

    #[test]
    fn tagged_and_chunked_alias_keys_and_values_cannot_bypass_row_charges() {
        let inputs: &[(&[u8], usize)] = &[
            (b"\xa1\xd8\x2a\x67aliases\x6da\tb\tlexical\tc", 4),
            (b"\xa1\x67aliases\xd8\x2a\x6da\tb\tlexical\tc", 4),
            (
                b"\xbf\x7f\x63ali\x64ases\xff\x7f\x62a\t\x62b\t\x68lexical\t\x61c\xff\xff",
                9,
            ),
            (b"\xd8\x2a\xa1\x67aliases\x6da\tb\tlexical\tc", 4),
        ];
        for &(input, items) in inputs {
            let charge = items * ITEM_CHARGE + 20 * STRING_BYTE_CHARGE + ALIAS_ROW_CHARGE;
            assert_eq!(check_with_limits(input, ordinary(charge, items)), Ok(()));
            assert!(matches!(
                check_with_limits(input, ordinary(charge - 1, items)),
                Err(MetadataResourceError::LimitExceeded { .. })
            ));
        }
        // A newline in its own chunk must still charge another alias row.
        let plain = b"\xa1\x67aliases\x7f\x61a\x61b\x61c\xff";
        let newline = b"\xa1\x67aliases\x7f\x61a\x61\n\x61b\xff";
        let single_row = 6 * ITEM_CHARGE + 10 * STRING_BYTE_CHARGE + ALIAS_ROW_CHARGE;
        assert_eq!(check_with_limits(plain, ordinary(single_row, 6)), Ok(()));
        assert!(check_with_limits(newline, ordinary(single_row, 6)).is_err());
        let two_rows = single_row + ALIAS_ROW_CHARGE;
        assert_eq!(check_with_limits(newline, ordinary(two_rows, 6)), Ok(()));
        assert!(check_with_limits(newline, ordinary(two_rows - 1, 6)).is_err());
    }

    #[test]
    fn indefinite_nested_containers_and_chunks_share_aggregate_limits() {
        for input in [
            b"\x9f\x80\x80\x80\xff".as_slice(),
            b"\xbf\x00\xa0\x01\xa0\xff",
            b"\x7f\x60\x60\x60\xff",
            b"\x5f\x40\x40\x40\xff",
            b"\xc0\xc0\xc0\x00",
        ] {
            assert!(check_with_limits(input, ordinary(4096, 3)).is_err());
            assert_eq!(check_with_limits(input, ordinary(4096, 10)), Ok(()));
        }
        let mut deep = vec![0x81; MAX_DEPTH + 1];
        deep.push(0);
        assert!(matches!(
            check_metadata_resources(&deep),
            Err(MetadataResourceError::LimitExceeded {
                resource: "CBOR nesting depth"
            })
        ));
        deep.remove(0);
        assert_eq!(check_metadata_resources(&deep), Ok(()));
    }

    #[test]
    fn critical_fields_share_one_budget_and_do_not_exempt_ordinary_fields() {
        let mut input = b"\xa2\x72candidate_evidence\x80\x6fphrase_evidence\x80".to_vec();
        let limits = Limits {
            ordinary: 4096,
            critical: ITEM_CHARGE * 2,
            items: 100,
        };
        assert_eq!(check_with_limits(&input, limits), Ok(()));
        assert!(
            check_with_limits(
                &input,
                Limits {
                    critical: ITEM_CHARGE * 2 - 1,
                    ..limits
                }
            )
            .is_err()
        );
        input[0] = 0xa3;
        input.extend_from_slice(b"\x61x\x80");
        assert!(
            check_with_limits(
                &input,
                Limits {
                    ordinary: 3 * ITEM_CHARGE + 33 * 4,
                    critical: usize::MAX,
                    items: 100
                }
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_cbor_is_rejected_without_entering_the_allocating_decoder() {
        for input in [
            b"".as_slice(),
            b"\xff",
            b"\x81",
            b"\xbf\x00\xff",
            b"\x7f\x40\xff",
            b"\x7f\x7f\xff\xff",
            b"\x1f",
            b"\x1c",
            b"\x63x",
        ] {
            assert!(matches!(
                check_metadata_resources(input),
                Err(MetadataResourceError::InvalidCbor { .. })
            ));
        }
        // Existing legacy handling ignores trailing CBOR; critical loaders check it.
        assert_eq!(check_metadata_resources(b"\xa0\xff"), Ok(()));
    }

    #[test]
    fn phrase_index_charges_terminal_cross_product_before_construction() {
        let evidence = PhraseEvidence::new(vec![
            PhraseRule::new(
                vec![WordId(1)],
                vec![PhraseChunk::new(
                    "가".into(),
                    vec![PhraseSpelling::new("家".into(), vec![])],
                )],
            )
            .with_terminal_extensions(vec![TerminalExtension::new(
                "하다".into(),
                vec![],
                vec![],
            )]),
        ]);
        let charge = (512 + 4 * "家".len()) * 2 * 2;
        assert_eq!(check_phrase_with_limit(&evidence, charge), Ok(()));
        assert!(check_phrase_with_limit(&evidence, charge - 1).is_err());
        assert_eq!(
            check_phrase_with_limit(&PhraseEvidence::new(Vec::new()), 0),
            Ok(())
        );
    }
}

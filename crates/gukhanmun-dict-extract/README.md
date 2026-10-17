gukhanmun-dict-extract
======================

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

Shared dictionary dump extraction helpers for Gukhanmun.

This library crate contains shared key assembly and word normalization logic
for National Institute of Korean Language dictionary dumps such as the *Standard
Korean Language Dictionary* (標準國語大辭典) and the *Open Korean Dictionary*
(우리말샘). Source-specific extractors depend on this crate to focus on parsing
logic and category-specific policies.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-dict-extract?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-dict-extract
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-dict-extract
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-dict-extract = "0.2"
~~~~


Usage
-----

This crate provides helpers to extract and normalize dictionary headwords:

~~~~ rust
use gukhanmun_dict_extract::{keys_from_originals, normalize_word};

// Assemble lookup keys from original-language records
let keys = keys_from_originals(&original_language_infos);

// Normalize dictionary head words (stripping homograph digits, hyphens, and spaces)
let clean_word = normalize_word("힐난-조03");
assert_eq!(clean_word, "힐난조");
~~~~


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.


Loanword tools
--------------

The `loanwords` module collects complete official source snapshots, audits
lookup candidates and applies explicit editorial decisions for [issue #18].
The pinned corpus and runtime dictionary live in `gukhanmun-loanwords`.
See its *ATTRIBUTION.md* and the repository's *NOTICE.md* for the source's
free-use statements, separately from the software license. Checksums and
matching sweeps detect changes; they do not independently prove authenticity.

The offline CLI needs `loanwords-cli`; HTTPS collection additionally needs
`loanwords-collect`. Neither feature is enabled by default, so existing
*Standard Korean Dictionary* and *Open Korean Dictionary* builds acquire no TLS
stack. The optional collector pins ureq 3.4.2 built with rustls and ring, with
compression disabled. Enabling `loanwords-collect` also compiles out
trace-level messages from dependencies using the `log` facade by default:
ureq's Trace output can include credential-bearing queries and raw request
bytes. A downstream release filter can override this cap, so collection refuses
credential-bearing requests if the compiled maximum exceeds Debug. This cap
does not apply to the separate `tracing` facade. The
`ring` crate requires native build tools; local verification covers Linux, not
every release platform.

[issue #18]: https://github.com/dahlia/gukhanmun/issues/18

### Explicit inputs and outputs

Given an authorized versioned snapshot and a decisions file:

~~~~ sh
mise run loanwords-extract -- /data/snapshot.json /data/decisions.json /data/loanwords.tsv /data/report.json
mise run loanwords-extract-check -- /data/snapshot.json /data/decisions.json /data/loanwords.tsv /data/report.json
~~~~

Start with `{"format_version":1,"decisions":[]}` when auditing a new input.
To inspect candidates before choosing defaults, use the CLI's `audit` command:

~~~~ sh
mise exec -- cargo run -p gukhanmun-dict-extract --features loanwords-cli --bin gukhanmun-loanwords -- audit --snapshot /data/snapshot.json --output /data/loanwords.tsv --report /data/audit.json
~~~~

Audit validates `--output` for aliasing but writes only the report; no TSV is
published. Its `audit.candidates` object lists normalized keys,
readings and complete source fingerprints. For each conflicting key, add a
`decisions` entry with `key`, `chosen`, `expected`, `rationale` and
`references`. Copy that key's complete candidate array into `expected`; choose
one of its readings in `chosen`. References are a list of strings and may be
empty when no supporting reference exists. The rationale must be nonempty.
Extraction rejects missing, duplicate, removed and stale decisions. A changed
unknown source field or a new source record with the same reading requires
editorial review too. The chosen reading receives preference one, every
alternative zero, and a singleton zero. No positional readings or annotation
marks are inferred.

The `extract` and `check` commands accept `--prior-snapshot` for source and key
differences. `--diagnostic` names an optional separate audit-only document
written on extraction failure; it never replaces the previous TSV or success
report. All destinations must be distinct from all inputs and each other,
including relative, symlink-parent and hardlink aliases. ASCII case-only path
differences are rejected even on case-sensitive filesystems. Temporary probes
in the destination filesystem detect Unicode-equivalent absent output names.
Parent directories must already exist. Writes use a temporary file in the
destination directory, fsync and rename. Existing file permissions are
preserved; new Unix files respect the umask. The TSV is published before its
report; if report publication fails, the command reports an incomplete pair.
The report hashes the snapshot, decisions and TSV, so `check` detects a stale
pair without writing any file. Generated artifacts are compared in fixed-size
chunks against the expected bytes; the 256 MiB offline input cap does not limit
report verification.

### Conservative extraction grammar

Only `srclang_mark` supplies lookup keys. Definitions, `relate_mark_o` and other
related spellings never do. Outer whitespace is trimmed; accepted Unicode code
points, including compatibility ideographs, are preserved without NFC folding.
Keys contain only continuous hanja and hangul syllables, with at least one
hanja. Internal whitespace, radicals, variation selectors, Latin letters,
kana, digits and punctuation in the resulting key are excluded with named
reasons. Keys are limited to 1,024 UTF-8 bytes.

One trailing ASCII or fullwidth parenthesized pronunciation annotation may be
removed before key validation. Its alphabet is ASCII letters, Latin letters in
U+00C0–U+024F excluding multiplication and division signs, combining marks in
U+0300–U+036F, hiragana U+3041–U+3096, katakana U+30A1–U+30FA, the katakana
long-vowel mark, ordinary spaces, apostrophes, right single quotation marks,
hyphens, middle dots and periods. At least one letter is required. Nested or
multiple annotations, hanja notes, digits and unsupported notation are excluded.
A separating space before an annotation is trimmed after stripping it; internal
spaces in keys remain unsupported by the current engine.

Readings are trimmed nonempty hangul syllable runs separated by single ordinary
spaces. Repeated spaces, controls and alternative separators are excluded.
Scalar readings and aggregate encoded candidate payloads must each fit the
backend's 65,535-byte limit. The audit retains original spellings and readings,
normalization reasons, exclusions and maximum accepted key lengths. This
initial grammar deliberately covers only a subset of possible records.

### Collection limits and live contract

The fixed HTTPS endpoint is the one in the [official API documentation]. The
collector reads `NIKL_LOANWORD_API_KEY` from the environment. The old
`KOREAN_GO_KR_SERVICE_KEY` name remains a deprecated fallback; simultaneous
unequal values are rejected. The
collector accepts a decoded key or a percent-encoded key and decodes percent
escapes exactly once before URL encoding the query parameter. Literal `+`
characters remain pluses; malformed escapes and control characters are rejected
without echoing the key. Set it in your local environment or secret manager,
then choose an external output path:

~~~~ sh
mise run loanwords-collect -- /data/loanword-snapshot.json
~~~~

Writing a collected snapshot inside a Git checkout is refused. The CLI exposes
no arbitrary authenticated URL, follows no redirects and reports fixed errors
or numeric HTTP statuses. It retries only transport failures, 429 and 5xx
responses, at most three retries per request. Timeouts and delays have a
60-second ceiling; response bodies, total response bytes and pages are bounded.
Default requests use 100 rows, a one-second minimum delay, two retries, a
30-second timeout, 4 MiB per response and 128 MiB across two sweeps. These are
local safeguards, not verified official request limits. `collect --help` lists
overrides.

Two complete sweeps must agree by canonical record content before the single
self-contained snapshot is written or replaced. Full sweeps capture deletions,
but do not prove a server-pinned generation. `collect --check` compares records
without writing and ignores changed collection timestamps. A snapshot records
format version, fixed endpoint, selector, collection timestamp, collector
version, distinct count, record-content SHA-256 and records sorted by opaque
identity. JSON object keys are sorted recursively regardless of `serde_json`'s
`preserve_order` feature; arrays retain order. Strings are preserved, while
numbers use `serde_json`'s finite parsed representation. Duplicate object
members are rejected, including unknown fields. IDs accept integer JSON numbers
or nonempty ASCII letters, digits, hyphens and underscores. Numeric IDs use
exact decimal text; collisions between string and numeric identities are
rejected.

Authenticated requests on 2026-10-18 verified the live JSON `response` object,
numeric `resultcode` zero, lowercase numeric `pageno`, `numofrows` and
`totalcount`, and an `items` array. The legacy flat experimental envelope
remains readable, but simultaneous envelopes or field aliases are rejected.
Records require `example_no`, nullable string `srclang_mark`/`korean_mark`, and
`regltn_code` equal to `0003`. Unknown record fields are retained; top-level
operational metadata such as `StatsVO` is discarded. Null/unsupported spellings
are preserved in the snapshot and excluded from lookup. No absent status field
is treated as publication evidence.

For pinned regeneration and weekly automation, use `mise run loanwords-check`,
`mise run loanwords-regenerate` and `mise run loanwords-update`. The explicit
local variant `mise run loanwords-update-local` loads *.env*. Ordinary builds
and tests never require the API key.

[official API documentation]: https://www.korean.go.kr/kornorms/main/openAPI.do

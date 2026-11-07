---
title: "Dictionaries"
description: |-
  Loading and composing dictionaries in the Gukhanmun Rust library.
---

Dictionaries
============

Gukhanmun looks up hanja readings from one or more `HanjaDictionary`
implementations.  The `gukhanmun` crate ships with FST and CDB backends and the
bundled *Standard Korean Dictionary*, *Loanword spelling examples*, and the
*Open Korean Dictionary* (우리말샘) North Korean dictionary used by the `ko-kp`
preset.


Using the bundled dictionary
----------------------------

The bundled dictionary selected by the preset is included automatically.  To
disable every bundled dictionary and rely only on user dictionaries or fallback
readings:

~~~~ rust
let converter = Builder::with_preset(Preset::KoKr)
    .no_bundled_dictionaries()
    .build()?;
~~~~

Use `no_bundled_stdict()` or `no_bundled_opendict()` to disable only one
bundled dictionary family.  To re-enable the *Standard Korean Dictionary*
explicitly after calling `no_bundled_stdict()`:

~~~~ rust
builder.bundled_stdict();
~~~~

Use the `ko-kp` preset for North Korean orthography.  It disables the initial
sound law and includes the bundled *Open Korean Dictionary* (우리말샘) North
Korean (北韓語) dictionary:

~~~~ rust
let converter = Builder::with_preset(Preset::KoKp).build()?;
assert_eq!(converter.convert_text_to_string("歷史와 來日")?, "력사와 래일");
~~~~

To keep the `ko-kp` orthographic options but disable its bundled *Open Korean
Dictionary* data:

~~~~ rust
let converter = Builder::with_preset(Preset::KoKp)
    .no_bundled_opendict()
    .build()?;
~~~~


Loanword spelling examples
--------------------------

The `gukhanmun-loanwords` crate bundles the National Institute of Korean
Language's *Loanword spelling examples* as an offline FST dictionary. The
`loanwords` feature is enabled by default, and `Preset::KoKr` includes this
dictionary automatically:

~~~~ rust
use gukhanmun::{Builder, Preset};

let converter = Builder::with_preset(Preset::KoKr)
    .build()?;
assert_eq!(converter.convert_text_to_string("香港")?, "홍콩");
~~~~

`Preset::KoKp` leaves this dictionary off; call `.bundled_loanwords()` to
enable it explicitly. User dictionaries take priority. Bundled reference and
loanword sources cooperate under the policy described below.
`.no_bundled_loanwords()` disables only this dictionary;
`.no_bundled_dictionaries()` disables it along with the other bundled
dictionaries.

For direct use through `HanjaDictionary`, add the `gukhanmun-loanwords` crate
and call `gukhanmun_loanwords::dictionary()`. With the umbrella crate's
`loanwords` feature, the same function is available as
`gukhanmun::loanwords::dictionary()`.


Loading a dictionary from a file
--------------------------------

Requires the `fst` or `cdb` feature.

~~~~ rust
use gukhanmun::FstDictionary;  // or CdbDictionary

let dict = FstDictionary::open("custom.gukfst")?;
let converter = Builder::with_preset(Preset::KoKr)
    .push_dictionary(dict)
    .build()?;
~~~~

Dictionaries added with `push_dictionary` are consulted before the bundled
dictionary. Earlier user dictionaries replace later candidates for the same
spelling and consumed length, including candidates from bundled sources.


Zero-copy static dictionaries
-----------------------------

Embed a dictionary directly in your binary with `include_bytes!` and load it
without any file I/O:

~~~~ rust
use gukhanmun::FstDictionary;

static MY_DICT: &[u8] = include_bytes!("../data/custom.gukfst");

let dict = FstDictionary::from_static_bytes(MY_DICT)?;
~~~~

`from_static_bytes` does not copy the data; it creates a zero-copy view backed
by the static slice.


Loading from owned bytes
------------------------

When the bytes come from a runtime source (network, database, etc.) wrap them
in an `Arc<[u8]>`:

~~~~ rust
use std::sync::Arc;
use gukhanmun::CdbDictionary;

let bytes: Vec<u8> = std::fs::read("custom.gukcdb")?;
let dict = CdbDictionary::from_bytes(Arc::from(bytes.as_slice()))?;
~~~~


Chaining multiple dictionaries
------------------------------

`ChainDictionary` lets you compose several dictionaries with explicit priority
ordering.  The first dictionary in the chain that has a match wins:

~~~~ rust
use gukhanmun::{ChainDictionary, FstDictionary, CdbDictionary, HanjaDictionary};

let domain_dict = FstDictionary::open("legal.gukfst")?;
let names_dict = CdbDictionary::open("names.gukcdb")?;
let chain = ChainDictionary::from_iter([
    Box::new(domain_dict) as Box<dyn HanjaDictionary>,
    Box::new(names_dict),
]);

let converter = Builder::with_preset(Preset::KoKr)
    .no_bundled_stdict()
    .push_boxed_dictionary(Box::new(chain))
    .build()?;
~~~~

Alternatively, call `push_dictionary` multiple times; dictionaries are probed
in the order they were pushed, before the bundled dictionary.


Composing Open Korean Dictionary categories
-------------------------------------------

When the `opendict` feature is enabled, Rust callers can load the *Open Korean
Dictionary* general, North Korean, dialect, and archaic categories directly.
The `ko-kp` preset includes the North Korean category automatically, but
dialect and archaic data are opt-in:

~~~~ rust
use gukhanmun::{Builder, ChainDictionary, HanjaDictionary, Preset};

let chain = ChainDictionary::from_iter([
    Box::new(gukhanmun::opendict::dialect()) as Box<dyn HanjaDictionary>,
    Box::new(gukhanmun::opendict::archaic()),
]);

let converter = Builder::with_preset(Preset::KoKr)
    .push_boxed_dictionary(Box::new(chain))
    .build()?;
~~~~


Building a custom dictionary
----------------------------

The *.gukfst* and *.gukcdb* files loaded above are compiled artifacts, built
from a plain text table with the `gukhanmun-mkdict` tool.  See
[*Building a custom dictionary*](../cli/dictionary.md#building-a-custom-dictionary)
in the CLI guide for how to author and compile dictionary sources.


Keeping alternative readings
----------------------------

Use `insert_candidates()` when a key has competing lexical readings. Ordinary
`insert()` and `insert_with_suffix()` still replace the entire key.

~~~~ rust
use gukhanmun::{MapDictionary, MatchMark, ReadingCandidate, ReadingCandidates};

let candidates = ReadingCandidates::new(vec![
    ReadingCandidate {
        reading: "동경".into(), suffix_reading: None,
        mark: MatchMark::default(), preference: 0,
    },
    ReadingCandidate {
        reading: "도쿄".into(), suffix_reading: None,
        mark: MatchMark::default(), preference: 10,
    },
])?;
let mut dictionary = MapDictionary::new();
dictionary.insert_candidates("東京", candidates);
~~~~

The context-free default is `도쿄`. Conversion selects `동경` in `東京을` and
`도쿄` in `東京를`, leaving the particle unchanged. A higher-priority
dictionary replaces the whole set for `東京`. Use `candidate_matches_at()` or
`candidate_entries()` to inspect alternatives; `matches_at()` and `entries()`
expose only the selected default. FST/CDB `lookup().candidates()` exposes the
same sets.

When the readings arrive one per row, `MapDictionary::from_candidates()` groups
repeated keys into the same candidate set, so a table of `(hanja, reading)`
rows needs no manual grouping:

~~~~ rust
use gukhanmun::{MapDictionary, MatchMark, ReadingCandidate};

let dictionary = MapDictionary::from_candidates([
    ("東京", ReadingCandidate {
        reading: "동경".into(), suffix_reading: None,
        mark: MatchMark::default(), preference: 0,
    }),
    ("東京", ReadingCandidate {
        reading: "도쿄".into(), suffix_reading: None,
        mark: MatchMark::default(), preference: 10,
    }),
])?;
~~~~

Preferences are normalised per key exactly as `ReadingCandidates::new()` does,
and an empty hanja key is rejected with `CandidateError::EmptyKey`.

### Particle evidence and limits

The filter runs automatically after dictionary precedence and variant identity
resolution. It keeps the first compatible candidate in preference order;
uncertain context or an empty survivor set uses the default. An override that
leaves a single reading therefore stays authoritative. Candidate marks and
positional spellings travel with the selected reading. Unknown endings remain
eligible. If a custom candidate's initial and suffix spellings disagree, either
compatible or unknown ending keeps that candidate eligible.

Supported particles are `을/를`, `은/는`, `이/가`, `과/와`, and `으로/로`,
including the rieul exception. Supported combinations are `으로는`, `으로도`,
`으로만`, `로는`, `로도`, `로만`, `과는`, `과도`, `과만`, `와는`, `와도`, and
`와만`. A complete suffix must end at whitespace, EOF, a block boundary
reported by the reader, or `.,!?:;)]}。、！？，：；）】」』`. Copulas such as
`이다`, lexical prefixes such as `이름`, and unsupported combinations keep the
default.

Streaming holds four extra source scalars beyond dictionary lookahead.
`finish()` proves EOF; calling `flush()` inside a word may forfeit evidence.
Inline markup, preserved/verbatim transitions, Markdown soft/hard line breaks
and links, HTML `br`, literal entities, and unlisted quotation marks do not
prove a boundary. For example, `[東京을](u)` keeps the default, while a space
after `을` supplies evidence. The engine never connects the particle across
separate inline tokens.

Dictionary-wide homophone detection indexes context-free defaults and queries
the emitted reading. A selected alternate may be marked as a homophone of
another key's default, while unused alternatives alone do not create marks.
Context-local detection compares the readings actually emitted in the active
window.

If the document ends right after a root inline close such as `<b>東京을</b>`,
EOF still supplies evidence. A later token or explicit `flush()` treats that
boundary as uncertain.


Cooperating sources
-------------------

The default `SourcePolicy::ReferenceAuthorized` policy keeps reference and
loanword candidate sets separate. A reference candidate authorizes a foreign
form only when the source dictionary explicitly records the relevant synonym or
formal redirect. The offline compiler intersects the permissions of every
contributing sense; a shared hanja key or reading alone grants no permission.
Missing evidence retains the reference form. Thus `東扶餘` reads `동부여`,
while the reference entry for `香港` authorizes `홍콩`.

The resolver ranks complete forms before applying particle evidence once to the
whole pool. An independent loanword can still match a particle even when it is
not the default: `平` reads `평`, but `平를` can select `다이라를`. Dictionary
preferences apply within each source; they are not comparable across sources.
The lattice objective is unchanged.

| Policy                              | Source behavior                                                                          |
| ----------------------------------- | ---------------------------------------------------------------------------------------- |
| `SourcePolicy::ReferenceAuthorized` | Authorized loanword forms, reference redirect/original forms, then independent loanwords |
| `SourcePolicy::ReferenceFirst`      | Original reference forms before independent loanwords                                    |
| `SourcePolicy::LoanwordFirst`       | Independent loanwords before original reference forms                                    |
| `SourcePolicy::Ordered`             | Legacy dictionary order; ignores roles and authorization                                 |

An authored override is exclusive for the same spelling and consumed length
under every cooperating policy. It does not remove a longer dictionary match.
For cooperating policies, `Builder::reference_only(true)` removes independent
loanwords from eligibility but retains reference-authorized forms, including
forms supplied by a loanword dictionary.

Selected forms carry their own lexical identity, alias, positional form and
annotation marks. First-occurrence handling uses the reference anchor when a
reference authorizes a foreign form; homophone handling compares the reading
actually emitted. Source-aware dictionary matches keep separate annotation
boundaries, including matches next to fallback characters. This can change
parenthetical grouping compared with `SourcePolicy::Ordered`.

Use `Builder::source_policy()` to select a policy:

~~~~ rust
use gukhanmun::{Builder, Preset, SourcePolicy, SourceRole};

let converter = Builder::with_preset(Preset::KoKr)
    .source_policy(SourcePolicy::ReferenceAuthorized)
    .reference_only(true)
    .build()?;
~~~~

`push_dictionary()` and `push_boxed_dictionary()` always register authored
overrides. To add a cooperating source, use
`push_dictionary_with_role(dictionary, SourceRole::Reference)` or
`SourceRole::Loanword`. Direct `ChainDictionary::new()` and
`ChainDictionary::from_iter()` calls retain `SourcePolicy::Ordered`; opt in
with `with_source_policy()` and `push_with_role()` when composing the chain
yourself.

With lattice segmentation, overlapping paths can remain unresolved beyond the
maximum dictionary word length. Streaming retains their shared uncertainty until
a word boundary or EOF settles the path, while already settled prefixes can
still be emitted. Dictionary roles do not change the lattice scoring objective.

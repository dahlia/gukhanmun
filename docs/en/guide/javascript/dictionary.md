---
title: "Dictionaries"
description: |-
  Loading and using dictionaries with the Gukhanmun JavaScript library.
---

Dictionaries
============

Pass an array of `DictionarySource` objects in the `dictionaries` option.
Dictionary sources are either compiled artifacts (`FileDictionarySource`) or
entry lists computed at runtime (`MapDictionarySource`). The default
`reference-authorized` policy ranks candidates by source role and explicit
dictionary relations. Use `sourcePolicy: "ordered"` for legacy array precedence.


Standard Korean Dictionary packages
-----------------------------------

`@gukhanmun/stdict-fst` and `@gukhanmun/stdict-cdb` both ship the bundled
*Standard Korean Dictionary* (標準國語大辭典) in different binary formats.

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
import { stdictCdb } from "@gukhanmun/stdict-cdb";

// FST — preferred; smaller on disk, better for lattice segmentation
const g = await load({ dictionaries: [await stdictFst()] });

// CDB — O(1) lookup; simpler layout
const g = await load({ dictionaries: [await stdictCdb()] });
~~~~

Both `stdictFst()` and `stdictCdb()` return a `Promise<FileDictionarySource>`.


Open Korean Dictionary packages
-------------------------------

`@gukhanmun/opendict-fst` and `@gukhanmun/opendict-cdb` ship the *Open Korean
Dictionary* (우리말샘) as separate general (一般語), North Korean (北韓語),
dialect (方言), and archaic (옛말) category dictionaries.  JavaScript presets
do not auto-load these packages, so pass the categories you want explicitly:

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
import {
  opendictDialectFst,
  opendictNorthKoreanFst,
} from "@gukhanmun/opendict-fst";

const g = await load({
  preset: "ko-kp",
  dictionaries: [
    await opendictNorthKoreanFst(),
    await opendictDialectFst(),
  ],
});
~~~~

Use the CDB package when you want the same categories in CDB format:

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/napi";
import { opendictNorthKoreanCdb } from "@gukhanmun/opendict-cdb";

const g = await load({
  preset: "ko-kp",
  dictionaries: [await opendictNorthKoreanCdb()],
});
~~~~


Loanword spelling examples packages
-----------------------------------

`@gukhanmun/loanwords-fst` and `@gukhanmun/loanwords-cdb` provide the National
Institute of Korean Language's *Loanword spelling examples* dictionary. Both
`"ko-kr"` and `"ko-kp"` require explicit loading; presets never add it for you.
The official loaders identify reference and loanword sources, so their relative
order does not change which source class takes priority under the default
`"reference-authorized"` policy:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { loanwordsFst } from "@gukhanmun/loanwords-fst";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  preset: "ko-kr",
  dictionaries: [await loanwordsFst(), await stdictFst()],
});
g.convert("香港"); // "홍콩"
~~~~

For CDB format, import `loanwordsCdb` from `@gukhanmun/loanwords-cdb` instead.
Both loaders return a `Promise<FileDictionarySource>` and work with
`@gukhanmun/wasm` and `@gukhanmun/napi`. Installing the package alone does not
change conversion; include its loader's result in `dictionaries`.


Custom dictionaries
-------------------

A `FileDictionarySource` specifies the binary format and the data to load from:

~~~~ ts twoslash
interface FileDictionarySource {
  format: "fst" | "cdb";
  data: ArrayBuffer | ArrayBufferView | URL | string;
}
~~~~

The `data` field accepts:

| Type                            | Where              | Notes                                     |
| ------------------------------- | ------------------ | ----------------------------------------- |
| `ArrayBuffer`/`ArrayBufferView` | All environments   | Bytes already in memory                   |
| `URL`                           | All environments   | Remote or local URL; loaded via `fetch()` |
| `string`                        | Node.js, Deno, Bun | Filesystem path                           |


Load from a URL
---------------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  dictionaries: [{
    format: "fst",
    data: new URL("./legal.gukfst", import.meta.url),
  }],
});
~~~~

In a browser the URL is fetched with `fetch()`.  In Node.js, Deno, and Bun a
`file://` URL is read from disk.


Load from bytes
---------------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const response = await fetch("/custom.gukcdb");
const buf = await response.arrayBuffer();

const g = await load({
  dictionaries: [{ format: "cdb", data: buf }],
});
~~~~


Load from a file path (Node.js/Deno/Bun)
----------------------------------------

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
// ---cut-before---
const g = await load({
  dictionaries: [{ format: "fst", data: "/data/domain.gukfst" }],
});
~~~~

Passing a plain string path in a browser throws `GukhanmunError` with code
`"io"`.


Combining multiple dictionaries
-------------------------------

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  dictionaries: [
    { format: "fst", data: "/data/legal.gukfst" },  // checked first
    await stdictFst(),                               // fallback
  ],
});
~~~~


Building a custom dictionary
----------------------------

The *.gukfst* and *.gukcdb* files loaded above are compiled artifacts, built
from a plain text table with the `gukhanmun-mkdict` tool.  See
[*Building a custom dictionary*](../cli/dictionary.md#building-a-custom-dictionary)
in the CLI guide for how to author and compile dictionary sources.  When the
entries are computed at runtime rather than stored on disk, pass them directly
as shown in the next section.


Entries computed at runtime
---------------------------

A `MapDictionarySource` carries entries in memory.  Use it when a caller
derives readings while building a document, in a browser, or in an edge runtime
where neither a TSV file nor `gukhanmun-mkdict` is available:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const converter = await load({
  dictionaries: [
    {
      entries: [
        { hanja: "高橋", reading: "다카하시" },
        { hanja: "東京", reading: "도쿄", preference: 10 },
        { hanja: "年代", reading: "연대", suffixReading: "년대" },
      ],
    },
    await stdictFst(),
  ],
});
~~~~

The fields mirror the Rust `ReadingCandidate` type plus the hanja key:

| Field           | Meaning                                                                |
| --------------- | ---------------------------------------------------------------------- |
| `hanja`         | The dictionary key                                                     |
| `reading`       | The word-initial hangul reading                                        |
| `suffixReading` | The reading used when the match is not word-initial, when it differs   |
| `preference`    | Relative default preference within the key; larger wins, absent is `0` |
| `requireHanja`  | Always show the original hanja alongside the reading                   |
| `requireHangul` | Always show a hangul gloss alongside the original hanja                |

Entry lists default to the `"override"` role. Cooperating policies consider
them before reference and loanword sources for the same spelling and consumed
length; earlier overrides win. With `sourcePolicy: "ordered"`, all sources
follow array precedence, so place the entry list before a compiled dictionary
to override that dictionary's keys in this mode.

Repeating a `hanja` key appends another reading instead of replacing the
earlier one.  Several readings for one headword stay available to the engine,
which then selects one from a following particle when the particle is decisive:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  dictionaries: [
    {
      entries: [
        { hanja: "東京", reading: "동경" },
        { hanja: "東京", reading: "도쿄", preference: 10 },
      ],
    },
  ],
});
g.convert("東京");   // "도쿄": the higher preference
g.convert("東京을"); // "동경을": the particle selects the other reading
g.convert("東京를"); // "도쿄를"
~~~~

`preference` decides the context-free default.  List order never does: when two
readings share a preference, the tie is broken by reading spelling, so set
distinct preferences when the choice matters.

An empty `entries` list contributes nothing, which makes `{ entries: [] }` a
convenient placeholder for a document without overrides.


Cooperating sources
-------------------

The default `reference-authorized` policy keeps reference and loanword
candidate sets separate. A reference candidate authorizes a foreign form only
when the source dictionary explicitly records the relevant synonym or formal
redirect. The offline compiler intersects the permissions of every contributing
sense; a shared hanja key or reading alone grants no permission. Missing
evidence retains the reference form. Thus `東扶餘` reads `동부여`, while the
reference entry for `香港` authorizes `홍콩`.

The resolver ranks complete forms before applying particle evidence once to the
whole pool. An independent loanword can still match a particle even when it is
not the default: `平` reads `평`, but `平를` can select `다이라를`. Dictionary
preferences apply within each source; they are not comparable across sources.
The lattice objective is unchanged.

| Policy                 | Source behavior                                                                          |
| ---------------------- | ---------------------------------------------------------------------------------------- |
| `reference-authorized` | Authorized loanword forms, reference redirect/original forms, then independent loanwords |
| `reference-first`      | Original reference forms before independent loanwords                                    |
| `loanword-first`       | Independent loanwords before original reference forms                                    |
| `ordered`              | Legacy dictionary array order; ignores roles and authorization                           |

An authored override is exclusive for the same spelling and consumed length
under every cooperating policy. It does not remove a longer dictionary match.
For cooperating policies, `referenceOnly` removes independent loanwords from
eligibility but retains reference-authorized forms, including forms supplied by
a loanword dictionary.

Selected forms carry their own lexical identity, alias, positional form and
annotation marks. First-occurrence handling uses the reference anchor when a
reference authorizes a foreign form; homophone handling compares the reading
actually emitted. Source-aware dictionary matches keep separate annotation
boundaries, including matches next to fallback characters. This can change
parenthetical grouping compared with `ordered`.

Set the policy and eligibility on the converter:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
import { loanwordsFst } from "@gukhanmun/loanwords-fst";
// ---cut-before---
const converter = await load({
  sourcePolicy: "reference-authorized",
  referenceOnly: true,
  dictionaries: [await stdictFst(), await loanwordsFst()],
});
~~~~

Both file and entry descriptors accept
`role: "override" | "reference" | "loanword"`. Entry descriptors default to
`"override"`. File descriptors use the binary's embedded role when present,
otherwise `"override"`; an explicit `role` replaces that metadata. Official
loaders declare their roles. Overrides are considered before cooperating
sources regardless of their array position; earlier overrides win. Set
`sourcePolicy: "ordered"` to restore legacy array precedence for all
descriptors.

The standard CDB loader downloads *stdict.cdb.gz* and decompresses it before
returning a dictionary descriptor. A custom URL can serve either gzip or raw
CDB bytes. The npm package retains the raw *stdict.cdb* export.

With lattice segmentation, overlapping paths can remain unresolved beyond the
maximum dictionary word length. Streaming retains their shared uncertainty until
a word boundary or EOF settles the path, while already settled prefixes can
still be emitted. Dictionary roles do not change the lattice scoring objective.

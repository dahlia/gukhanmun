---
title: "Dictionaries"
description: |-
  Loading and composing dictionaries in the Gukhanmun Rust library.
---

Dictionaries
============

Gukhanmun looks up hanja readings from one or more `HanjaDictionary`
implementations.  The `gukhanmun` crate ships with FST and CDB backends and
a bundled Standard Korean Dictionary.


Using the bundled dictionary
----------------------------

The bundled dictionary is included automatically when the `stdict` feature is
enabled (the default).  To disable it:

~~~~ rust
let converter = Builder::with_preset(Preset::KoKr)
    .no_bundled_stdict()
    .build()?;
~~~~

To re-enable it explicitly after calling `no_bundled_stdict`:

~~~~ rust
builder.bundled_stdict();
~~~~


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
dictionary.  The first match across the chain wins.


Zero-copy static dictionaries
-----------------------------

Embed a dictionary directly in your binary with `include_bytes!` and load it
without any file I/O:

~~~~ rust
use gukhanmun::FstDictionary;

static MY_DICT: &[u8] = include_bytes!("../data/custom.gukfst");

let dict = FstDictionary::from_static_bytes(MY_DICT)?;
~~~~

`from_static_bytes` does not copy the data; it creates a zero-copy view
backed by the static slice.


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
use gukhanmun::{ChainDictionary, FstDictionary, CdbDictionary};

let domain_dict = FstDictionary::open("legal.gukfst")?;
let names_dict = CdbDictionary::open("names.gukcdb")?;
let chain = ChainDictionary::new(vec![
    Box::new(domain_dict),
    Box::new(names_dict),
]);

let converter = Builder::with_preset(Preset::KoKr)
    .no_bundled_stdict()
    .push_boxed_dictionary(Box::new(chain))
    .build()?;
~~~~

Alternatively, call `push_dictionary` multiple times; dictionaries are probed
in the order they were pushed, before the bundled dictionary.


Building a custom dictionary
----------------------------

The *.gukfst* and *.gukcdb* files loaded above are compiled artifacts, built
from a plain text table with the `gukhanmun-mkdict` tool.  See
[*Building a custom dictionary*](../cli/dictionary.md#building-a-custom-dictionary)
in the CLI guide for how to author and compile dictionary sources.

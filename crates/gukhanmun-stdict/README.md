gukhanmun-stdict
================

[![crates.io][crates.io badge]][crates.io]
[![License: GPL-3.0-only][GPL badge]][GPL]

Bundled South Korean Standard Dictionary (標準國語大辭典) for Gukhanmun,
compiled into an FST binary and embedded in the crate at build time. No
external file or network access is required at runtime.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun-stdict?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun-stdict
[GPL badge]: https://img.shields.io/crates/l/gukhanmun-stdict
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html


Installation
------------

~~~~ toml
[dependencies]
gukhanmun-stdict = "0.1"
~~~~


Usage
-----

~~~~ rust
use gukhanmun_stdict::ko_kr;

let dict = ko_kr();
// dict: &'static FstDictionary — decoded once, shared for the process lifetime
~~~~

`ko_kr()` decodes the embedded FST bytes on the first call and caches the
result in a `OnceLock`. Subsequent calls return the same reference with no
additional work.

The dictionary is suitable for South Korean orthography (`ko-KR`). North
Korean orthography is not covered because the Standard Dictionary's readings
apply the initial sound law and other South Korean conventions that are
incorrect for North Korean text.


Build-time requirements
-----------------------

The FST binary is generated during `cargo build` from a canonical TSV snapshot
stored in the *data/* directory. The snapshot is committed to the repository,
so a network connection is not required to build the crate. `gukhanmun-mkdict`
is used as a build dependency.


License
-------

GPL-3.0-only. See *LICENSE* at the repository root.

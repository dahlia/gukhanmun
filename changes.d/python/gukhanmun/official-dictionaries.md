---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#49': https://github.com/dahlia/gukhanmun/issues/49
  '#57': https://github.com/dahlia/gukhanmun/pull/57
---
 -  Added the `stdict`, `opendict` and `loanwords` extras, which install the
    FST data of the *Standard Korean Language Dictionary*, the *Open Korean
    Dictionary* and the NIKL loanword spelling examples, and the
    format-specific `stdict-fst`, `stdict-cdb`, `opendict-fst`, `opendict-cdb`,
    `loanwords-fst` and `loanwords-cdb` extras; for example,
    `pip install 'gukhanmun[stdict]'`. Each extra pins its data distribution to
    exactly the version of `gukhanmun`. Installing an extra does not add a
    dictionary to any converter, and neither preset loads official data.
    [[#49], [#15], [#57]]

 -  Added the `gukhanmun.dictionaries` module for activating the installed
    official dictionaries explicitly: `stdict()`, `opendict_general()`,
    `opendict_north_korean()`, `opendict_dialect()`, `opendict_archaic()` and
    `loanwords()` return a `FileDictionarySource` for
    `Converter(dictionaries=[...])` with the `REFERENCE` or `LOANWORD` source
    role, and the matching `*_bytes()` functions return the dictionary bytes
    alone. Each takes a keyword-only `format=DictionaryFormat.FST` or
    `DictionaryFormat.CDB`, reads its data through `importlib.resources` into
    owned bytes and decompresses bundled gzip data. A missing data distribution
    raises `GukhanmunError` with `ErrorCode.DICTIONARY_LOAD` and the
    `pip install` command for its extra. Each resource is checked against the
    size and SHA-256 hash in its distribution's *provenance.json*, so a damaged
    installation, such as a missing, altered or corrupt resource, raises the
    same code with a `pip install --force-reinstall` command and any original
    cause in `chain`. [[#49], [#15], [#57]]

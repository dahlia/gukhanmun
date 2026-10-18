Loanword examples in FST format
===============================

The National Institute of Korean Language's loanword spelling examples,
packaged for `@gukhanmun/wasm` and `@gukhanmun/napi`.

~~~~ ts
import { load } from "@gukhanmun/wasm";
import { loanwordsFst } from "@gukhanmun/loanwords-fst";

const converter = await load({ dictionaries: [await loanwordsFst()] });
console.log(converter.convert("香港")); // 홍콩
~~~~

Put user dictionaries first, then loanwords, then stdict. Loading is explicit
in both Korean presets. The binary is generated offline from the pinned TSV
during build/publish and is not tracked in Git. See *ATTRIBUTION.md* and
*LICENSE* for the separate data and software terms.

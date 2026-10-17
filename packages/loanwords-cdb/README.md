Loanword examples in CDB format
===============================

The National Institute of Korean Language's loanword spelling examples,
packaged for `@gukhanmun/wasm` and `@gukhanmun/napi`.

~~~~ ts
import { load } from "@gukhanmun/wasm";
import { loanwordsCdb } from "@gukhanmun/loanwords-cdb";

const converter = await load({ dictionaries: [await loanwordsCdb()] });
console.log(converter.convert("香港")); // 홍콩
~~~~

Put user dictionaries first, then loanwords, then stdict. Loading is explicit
in both Korean presets. The binary is generated offline from the pinned TSV
during build/publish and is not tracked in Git. See *ATTRIBUTION.md* and
*LICENSE* for the separate data and software terms.

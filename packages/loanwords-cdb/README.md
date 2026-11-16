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

User dictionaries override cooperating sources for the same spelling and
consumed length. The default `reference-authorized` policy combines stdict
readings with loanword forms authorized by source evidence; array order alone
does not give loanwords unconditional priority. Loading is explicit
in both Korean presets. The binary is generated offline from the pinned TSV
during build/publish and is not tracked in Git. See *ATTRIBUTION.md* and
*LICENSE* for the separate data and software terms.

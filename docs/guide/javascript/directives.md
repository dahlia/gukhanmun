---
title: "Directives"
description: |-
  Per-hanja annotation overrides in the Gukhanmun JavaScript library.
---

Directives
==========

The `directives` option lets you override annotation marks for specific hanja
characters.


Directive interface
-------------------

~~~~ ts twoslash
interface Directives {
  requireHanja?:    string[];  // always show hanja in output
  requireHangul?:   string[];  // always show hangul reading (for "original" mode)
  skipAnnotation?:  string[];  // suppress annotation entirely
}
~~~~

Each array contains hanja strings whose marks you want to override:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  dictionaries: [await stdictFst()],
  directives: {
    requireHanja:   ["漢", "字"],
    requireHangul:  ["東"],
    skipAnnotation: ["中"],
  },
});
~~~~


Combining with rendering modes
------------------------------

Directives interact with the active rendering mode:

 -  `requireHanja` is most visible in `"hangul-only"` mode, where it forces
    the hanja to appear alongside the hangul reading.
 -  `requireHangul` is useful in `"original"` mode to force the hangul gloss
    for specific characters.
 -  `skipAnnotation` suppresses any annotation regardless of mode.

---
links:
  '#35': https://github.com/dahlia/gukhanmun/issues/35
  '#36': https://github.com/dahlia/gukhanmun/pull/36
---
 -  Markdown conversion escapes source text through the re-exported adapter, so
    the Rust API no longer drops the escapes and character references in the
    input.
    [[#35], [#36]]
 -  HTML and Markdown conversions now escape dictionary readings end to end
    through the re-exported adapters.  [[GHSA-j87h-8h88-wm6g]]

[GHSA-j87h-8h88-wm6g]: https://github.com/dahlia/gukhanmun/security/advisories/GHSA-j87h-8h88-wm6g

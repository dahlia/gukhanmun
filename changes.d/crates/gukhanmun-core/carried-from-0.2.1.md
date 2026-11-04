 -  Marked annotation-derived text with the new `RenderedToken::Annotated`
    variant so that format writers serialize a dictionary reading as literal
    text instead of treating it as already-valid markup.  The variant is a
    breaking change for code that matches `RenderedToken` exhaustively; a
    `_ =>` arm still compiles but silently drops the text.  Upgrade
    *gukhanmun-core*, *gukhanmun-html*, and *gukhanmun-markdown* together.
    [[GHSA-j87h-8h88-wm6g]]

[GHSA-j87h-8h88-wm6g]: https://github.com/dahlia/gukhanmun/security/advisories/GHSA-j87h-8h88-wm6g

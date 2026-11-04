---
links:
  '#35': https://github.com/dahlia/gukhanmun/issues/35
  '#36': https://github.com/dahlia/gukhanmun/pull/36
---
 -  Escaped source text when writing Markdown, so backslash escapes and
    character references consumed by the parser are restored instead of being
    re-emitted raw.  Interior `<`, `&`, `[`, `]`, `` ` ``, `*`, `_`, `~`, `|`,
    and `\` can no longer reform HTML, links, images, emphasis, code spans, or
    tables, and line-start characters can no longer start a heading, list,
    block quote, or definition list after conversion.  Code blocks, autolinks,
    and HTML blocks remain untouched.  Literal `&` and `<` in prose now appear
    as `&amp;` and `&lt;` in Markdown output; the rendered text is unchanged.
    [[#35], [#36]]
 -  Encoded dictionary readings as literal Markdown text, with U+200B separators
    that stop GFM from forming an autolink across a reading, so annotation text
    can no longer form links, emphasis, code spans, tables, or raw HTML.
    [[GHSA-j87h-8h88-wm6g]]

[GHSA-j87h-8h88-wm6g]: https://github.com/dahlia/gukhanmun/security/advisories/GHSA-j87h-8h88-wm6g

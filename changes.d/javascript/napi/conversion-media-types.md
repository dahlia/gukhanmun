---
links:
  '#87': https://github.com/dahlia/gukhanmun/issues/87
  '#91': https://github.com/dahlia/gukhanmun/pull/91
---
 -  Added media types to `convert()` and `stream()`: `"text/plain"`,
    `"text/html"`, `"text/markdown"`, and `"text/markdown; variant=GFM"`. The
    old format strings and Markdown objects are deprecated and will be removed
    in 1.0. Switch to media types. To find old calls at runtime, enable
    LogTape warnings in `["gukhanmun", "format"]`. [[#87], [#91]]

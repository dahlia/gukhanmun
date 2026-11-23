---
links:
  '#87': https://github.com/dahlia/gukhanmun/issues/87
  '#91': https://github.com/dahlia/gukhanmun/pull/91
---
 -  Added media types to `convert()` and `stream()`: `text/plain`,
    `text/html`, `text/markdown`, and `text/markdown; variant=GFM`. Old
    strings and Markdown objects still work until 1.0 and emit a LogTape
    warning in `["gukhanmun", "format"]` when configured. [[#87], [#91]]

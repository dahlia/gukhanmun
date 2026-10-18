---
links:
  '#27': https://github.com/dahlia/gukhanmun/issues/27
  '#28': https://github.com/dahlia/gukhanmun/pull/28
---
 -  Added `Builder::punctuation()` with `preserve`, `modern`, `traditional`,
    and `traditional-vertical` output profiles and custom role styles. The
    default preserves punctuation across plain text, HTML, and Markdown.
    Technical guards include Unicode Latin letters, and contextual spaces do
    not detach combining marks or format controls.  HTML block, list, and table
    elements trim contextual spaces without changing annotation windows.
    [[#27], [#28]]

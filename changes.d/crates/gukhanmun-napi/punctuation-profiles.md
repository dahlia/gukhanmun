---
links:
  '#27': https://github.com/dahlia/gukhanmun/issues/27
  '#28': https://github.com/dahlia/gukhanmun/pull/28
---
 -  Added the `punctuationJson` option and `punctuationProfileJson()` resolved
    view for output punctuation profiles, including custom role styles.
    Technical guards include Unicode Latin letters, and contextual spaces do
    not detach combining marks or format controls.  HTML block, list, and table
    elements trim contextual spaces without changing annotation windows.
    [[#27], [#28]]

---
links:
  '#27': https://github.com/dahlia/gukhanmun/issues/27
  '#28': https://github.com/dahlia/gukhanmun/pull/28
---
 -  Added `HtmlPunctuationTextCodec` for punctuation normalization across
    literal and HTML character reference spellings, with original spelling
    retained for untouched references. Opt-in reader options protect unmatched
    end tags.  HTML block, list, and table elements trim contextual spaces
    without changing annotation windows.  [[#27], [#28]]

Character selection and sources
===============================

Use the tables as decisions, not replacements
---------------------------------------------

Read both TSV files with tabs as separators and UTF-8 preserved.

 -  *form-preferences.tsv*: the author's explicit choices among character forms.
    `alternatives` is a space-separated set of complete alternative characters.
    Apply `preferred` only inside eligible lexical text after checking names and
    lexical exceptions. Every row is an author decision, not a universal claim
    that all alternatives are wrong in other languages.
 -  *lexical-exceptions.tsv*: Korean surface forms, senses, selected spellings,
    intended Hangul readings, and decisions about whether a loan remains native.
    Match the word and sense, including hybrid morphology. A row is not a rule
    to replace the same character sequence inside arbitrary names.

Both tables use `basis` to distinguish a direct author decision from an
application of a stated author rule. They intentionally exclude unapproved
directions proposed during research. Do not turn observations or candidate
pairs into new personal rules without the user's decision.

For other Korean words, whole-word dictionary spelling is useful evidence.
The Standard Korean Language Dictionary is usually a good starting point for
forms, but its classifications and individual characters do not override
explicit decisions such as 茶, 產, and 眾. For 裏/裡 use the actual lexical
spelling: 裏面 and 祕密裡. For 착/저 preserve 着/著 in Korean vocabulary.

恒/恆 has no single author-wide selection. A Korean dictionary may supply
恒常 while a Kangxi transcription describes 恒 as a vulgar form of 恆. Keep
the distinction between a word's attested Korean spelling and a proposal to
change the author's policy. Ask if a new task requires resolving that conflict.


Gather evidence at the right level
----------------------------------

Distinguish these claims:

 -  A dictionary uses a particular spelling for a particular Korean word.
 -  Two characters are listed as variants.
 -  A particular modern region treats one variant as its standard.
 -  A Kangxi edition uses a particular head character or discusses a vulgar
    form.
 -  The author explicitly prefers one spelling.

One does not establish all the others. In particular, Taiwan's current
standard forms do not determine Korean Kangxi forms or the author's choices.
Unihan variant fields and generated converter tables are leads, not proof of
a required normalization direction. Inspect the actual entry, and an original
scan when a transcription's character mapping cannot settle a glyph question.
Label inaccessible or unverified evidence rather than claiming confirmation.

Do not convert ambiguous simplified characters without lexical evidence:
发 can represent 發 or 髮; 后 may be 后 in 皇后 or correspond to 後; 干 can
represent 干, 乾, or 幹. A character's historical parent does not erase Korean
reading distinctions, and a similar component does not authorize recursively
changing every character containing it.


References
----------

 -  [Standard Korean Language Dictionary]
 -  [Open Korean Dictionary]
 -  [Taiwan MOE Dictionary of Chinese Character Variants]
 -  [MOE 產 entry, including 産]
 -  [MOE 眾 entry, including 衆]
 -  [CUHK 卽 entry]
 -  [Kangxi transcription for 恒]
 -  [NIKL explanation of title marks]

The author's Markdown posts in [dahlia/writings] are useful
examples if available. The repository is optional, not a dependency. Its
contents, including older spellings, foreign-language titles, quotations, and
possible drafts, do not outweigh an explicit decision recorded in this skill.

[Standard Korean Language Dictionary]: https://stdict.korean.go.kr/
[Open Korean Dictionary]: https://opendict.korean.go.kr/
[Taiwan MOE Dictionary of Chinese Character Variants]: https://dict.variants.moe.edu.tw/
[MOE 產 entry, including 産]: https://dict.variants.moe.edu.tw/dictView.jsp?ID=28200&la=0&powerMode=2
[MOE 眾 entry, including 衆]: https://dict.variants.moe.edu.tw/dictView.jsp?ID=29747&la=0
[CUHK 卽 entry]: https://humanum.arts.cuhk.edu.hk/Lexis/lexi-mf/search.php?word=卽
[Kangxi transcription for 恒]: https://zdic.net/hans/恒
[NIKL explanation of title marks]: https://www.korean.go.kr/front/onlineQna/onlineQnaView.do?mn_id=216&pageIndex=1&qna_seq=328219&searchCondition=&searchKeyword=
[dahlia/writings]: https://github.com/dahlia/writings

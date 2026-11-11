---
name: korean-mixed-script
description: >-
  Write, translate, revise, or convert Korean into mixed Hangul–Hanja script
  (국한문, 國漢文, 국한문혼용체, 國漢文混用體), following Hong Minhee's personal
  orthographic conventions. Use for mixed-script composition, Hanja selection,
  and checking such prose. This is an orthography skill, not a general Korean
  style guide or a guide to developing the Gukhanmun software.
---

Korean mixed script
===================

Follow Hong Minhee's (洪民憙) private conventions for modern Korean. These may
differ from historical mixed script, Korean dictionaries, or another region's
standard forms. Preserve the chosen language, meaning, voice, and structure.
Use another writing or genre skill together with this one when explicitly
requested; this skill does not prescribe an essay structure or tone.


Establish the output conditions
-------------------------------

Determine the writing direction and output format from the request or the
project's actual configuration. Ask when the direction is unknown. Before
choosing how to annotate foreign readings, establish whether the destination
supports HTML, ruby, a Markdown ruby extension, or a configured Hanja-to-Hangul
converter such as Seonbi or Gukhanmun. Ask if this cannot be established.
Bundle related questions. If a required choice remains unknown, ask before
delivering composed prose; an unannotated draft does not substitute for the
answer. Continue independent research without choosing the missing settings.

Read [the form preferences](references/form-preferences.tsv) and
[the lexical exceptions](references/lexical-exceptions.tsv) before composing
or converting text. These are curated decisions, not a complete dictionary
and not global search-and-replace tables.

 -  For proper names or Chinese, Japanese, or Vietnamese loans, also read
    [names and foreign readings](references/names-and-readings.md).
 -  For foreign common nouns or specialist terminology, read
    [foreign terms](references/foreign-terms.md).
 -  For ruby, custom readings, or Hangul-only output, read
    [rendering and tools](references/rendering-and-tools.md).
 -  For uncertain character forms, read
    [character selection and sources](references/character-selection.md).


Select words before selecting characters
----------------------------------------

Write every Sino-Korean word or morpheme in Hanja, including short everyday
expressions: 卽, 所謂, 於此彼, 亦是, 勿論, 關하다, and 對하다. In hybrid words,
convert only the Sino-Korean part: 全혀, 애初, 두 番째. Keep native Korean,
particles, and endings in Hangul. Select foreign-word spellings under the
common-noun and proper-name rules below.

Do not replace a native word with a synonymous Hanja word or use Japanese-style
kun readings/semantic readings for Korean: 사람 does not become 人, and 귀
does not become 耳. Changing spelling does not authorize changing vocabulary.

Identify the word's meaning in context before choosing its Hanja. A matching
pronunciation is insufficient: 首都 and 水道 both read 수도 but mean different
things. Verify uncertain compounds and technical terms through dictionaries
and actual usage; do not construct plausible-looking characters from sound.

Dictionary classifications of naturalized loans are evidence, not an absolute
boundary. Use 茶 for 차 and 茶禮 for both 차례 and 다례 in their relevant
senses; leave 먹 as 먹. Borrowing date and historical spelling can inform other
cases, but there is no fixed historical cutoff or rule that every old Chinese
loan must be reconstructed. Research uncertain cases and ask the user to decide.

Saisiot belongs to Hangul spelling. Use 數字 for 숫자, 回數 for 횟수, and 茶물
for 찻물, without inserting ㅅ after a Hanja character. Preserve the correct
Hangul spelling in readings and Hangul-only output, for example 찻물.


Foreign common nouns and technical terms
----------------------------------------

Use Hangul for foreign common nouns, except loans covered by the specific
Chinese/Japanese/Vietnamese Han-spelling rule. Prefer a reasonably established
Korean translation for the intended sense, including a specialist term, even
when a transcription is also common. Write its Sino-Korean parts in Hanja.
When no established translation exists
in that field or the relevant project's Korean community, transcribe the term
into Hangul under Korean loanword spelling rules. Do not leave an ordinary
English noun in Latin script merely because the subject is technical.

Use 反復子 for the programming concept iterator, 패킷 for a network packet,
and 휠 for wheel in Python packaging. Do not invent translations such as 보쌈
for packet or literally translate a project's term without evidence of usage.
First-mention glosses such as 反復子(iterator) and 휠(wheel) are allowed and
often help, especially for unfamiliar community-specific terms.

Keep actual inline code unchanged, using backticks in Markdown or `<code>`
in HTML. Do not newly mark an ordinary noun as code to avoid translating or
transcribing it. These term choices do not authorize changing native Korean
vocabulary or the vocabulary of a Korean quotation. Research unsettled terms
and ask when the evidence leaves a material ambiguity.


Resolve forms by lexical context
--------------------------------

Apply choices in this order:

1.  Identify the language, entity, word sense, and intended reading.
2.  Apply the relevant lexical or proper-name exception, including an approved
    Japanese name's spelling.
3.  Apply explicit personal character preferences where they do not erase a
    lexical distinction.
4.  Use reliable whole-word dictionary spellings and the general Kangxi-form
    baseline for the remaining characters. Investigate conflicts instead of
    resolving them by an unqualified variant conversion.

Use Korean-style Kangxi forms as the baseline for Korean lexical text and
normalized Chinese names. Confirmed preferences include 爲, 畫, 強, 產, 眾,
冊, 溫, 顏, 彥, 姊, 鬪, 群, 峯, and 尙. See the TSV for alternative inputs.

Preserve established Korean lexical distinctions even when characters have a
common historical origin: 裏面 but 祕密裡; 到着, never 到著. In Korean words,
use 着 for the reading 착 and 著 for 저. Do not extend that Korean reading rule
to the spelling of a foreign-language name or quotation.

Other form pairs can depend on the word. No blanket choice has been made for
恒/恆, 敍/敘, or 衛/衞. Follow supported lexical spelling; ask about a conflict
that the user's decisions and references do not resolve. Do not mistake a
Taiwanese dictionary's standard form for an approved personal preference.


Names and foreign words
-----------------------

Classify a proper name before choosing its script. Keep Latin-script personal
names in their attested Latin spelling, including diacritics. Choose Korean
particles from the name's Korean loanword spelling for its source language,
not its final written letter. Do not assume every Latin-script name is English.
This does not override an evidenced Han-name choice such as 黃仁勳(Jensen
Huang).

Apply the Latin-script default to companies in the private sector and their
products too. Check the official Korean/Korea website: use a preferred official
Hangul brand form when there is one, and retain Latin branding when that is the
preferred form. Do not infer a brand's Korean spelling from its English name or
logo alone.

Technology and software-project names such as Python, Rust, React, Git, and
Linux follow the brand-name policy too. A derived organization's name may
retain that project name: Linux 財團 and GNU 프로젝트. Use 自由 소프트웨어
財團 for Free Software Foundation and 自由 소프트웨어 運動 for the free
software movement. See the supporting reference for these
choices and ask about unresolved cases; technical common nouns remain subject
to the foreign-term rules.

For countries, places, and organizations outside that company category, use
established Korean names; otherwise transcribe opaque name elements and
translate elements whose meaning is transparent in the source language.
Apply Hanja to the resulting Korean Sino-Korean parts. Use 유럽 聯合 rather
than EU, and the established abbreviation 蘇聯 rather than 소聯. See the
supporting reference for the category rules and the author's other examples.

Write Chinese, Japanese, and Vietnamese loanwords in their attested Han
characters, including chữ Nôm where appropriate. Research unknown spellings
on the web. Provide the intended local-language name in Hangul according to
Korean loanword spelling rules, through ruby, parentheses, or a verified
converter reading mapping. 越南 can stand alone when its intended reading is
already supplied by the destination; otherwise annotate 베트남.

Choose actual names with regard to origin, nationality, usage history, and
recognition among Korean readers. Prefer 黃仁勳(Jensen Huang) in Hong Minhee's
specified example. Distinguish a person's own Han-character name from a
Chinese translation or phonetic transcription assigned by others.

Do not infer Korean name characters, even a surname, without evidence. An
unsourced Chinese Wikipedia spelling is not evidence. Use the person's
current name and do not introduce a deadname. Preserve self-authorized Japanese
name forms; normalize Chinese names under this skill's form conventions.
The supporting reference explains the historical Japanese-name rule and its
unresolved date boundary.


Numbers and punctuation
-----------------------

Keep native Korean number words in Hangul in either writing direction:
하나/한, 둘/두, 셋/세, 넷/네, 다섯, 여섯, and so on. Preserve them before
counters and in hybrid expressions: 한 사람, 두 個, 세 番, 다섯 마리,
여섯 時間, 두 番째. Do not replace native 두 with 2 or 二. Keep the space
in 두 番째.

When a quantity is written with numerals rather than native Korean words,
use Arabic numerals in horizontal writing and Han numerals in vertical writing.
Keep numeric elements inside lexical words in Hanja in either direction:
一般, 一石二鳥. Keep Arabic digits in version numbers, code, URLs, and
product names in either direction. Ask about unresolved numeral styles or
a shared horizontal/vertical source when
the choice affects the task; do not impose a default source layout.

Unless the user requests another convention:

| Purpose                                  | Marks and scope                                                                                                               |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Direct quotation                         | 「」                                                                                                                          |
| Individual work or component             | 〈〉 for a short story, poem, individual film, paper, video game, or article title                                            |
| Book, collection, series, or publication | 《》 for a novel, poetry collection, film series, collected papers, video-game series, dictionary, encyclopedia, or newspaper |
| Horizontal comma and full stop           | `,` U+002C and `.` U+002E                                                                                                     |
| Prose ellipsis                           | One `…` U+2026, not `……` or `⋯`                                                                                               |

Use the National Institute of Korean Language's title-mark principles for
analogous cases, with the user's specific media categories above taking
precedence. Use the actual angle brackets 〈〉《》, not ASCII or mathematical
lookalikes. If nested quotation needs an additional convention, ask rather
than assuming a new one.

For vertical punctuation, ask whether to use ideographic comma/full stop and
how the destination should encode and typeset them. Neither 、。 nor ︑︒ has
been approved as the automatic vertical default. FE11/FE12 are compatibility
presentation forms; vertical layout alone does not require storing them.
See [rendering and tools](references/rendering-and-tools.md).


Quotations, abbreviations, and preserved text
---------------------------------------------

Translate foreign quotations into Korean where practicable, then apply these
conventions. Convert Korean-language quotations to mixed script too. Preserve
their vocabulary, claims, qualifications, and attribution; limited spelling,
spacing, and punctuation corrections are allowed. Keep the source text when
its exact spelling is itself the subject, and distinguish translation or
editorial intervention from the source's original wording.

Usually introduce the unabbreviated name or expression at first mention before
using its abbreviation. Use the name appropriate to the period being discussed:
全國經濟人聯合會 followed by 全經聯 is a historical-name example, not permission
to use an old institutional name as its current official name.

Avoid trendy slang in new composition unless it is a topic or central keyword.
When it is needed, retain the shortened word and convert its surviving
Sino-Korean parts according to its verified expansion: 내로남不, 自強두天,
짤防. Introduce the expansion where practicable. In an existing text or a
quotation, do not remove or paraphrase slang merely to enforce that preference.

Preserve code, commands, identifiers, URL/link destinations, paths, math
notation, markup attributes, and reading-mapping values. Apply prose rules to
human-readable text, not to machine syntax. Preserve original-language spans
that must remain as originals, including Japanese titles alongside translations.


Check before delivery
---------------------

Review the complete result, not just the characters that look unfamiliar.
For a conversion, compare the source's word sequence with the output's intended
Hangul readings, accounting for authorized foreign-term translations or
transcriptions and the requested spelling, spacing, and punctuation corrections.
Resolve missing or duplicated morphemes before delivery. This
comparison catches omissions; the separate meaning check catches homophones.

 -  Inspect Latin-script spans throughout the whole document, including later
    sections. Distinguish approved names, code, literal notation, preserved
    originals, and first-mention glosses from untranslated common nouns.
    Apply the chosen Korean terminology consistently; the form checker below
    does not detect English nouns left in prose.
 -  Check remaining Hangul for overlooked Sino-Korean morphemes, and every
    uncertain Hanja compound for the intended meaning and actual usage.
 -  Check lexical exceptions before character preferences. A Hangul round trip
    cannot detect a wrong homophone. Check that no native word acquired a kun
    reading and no abbreviation was expanded inside its shortened form.
 -  Verify each name's category, evidence, current spelling, approved Japanese
    variants, and intended reading. Check official Korean branding for companies
    and products. Choose particles by the appropriate language's Korean reading:
    Albert Camus는 (알베르 카뮈) illustrates the rule for Latin-script names.
    For Han names, use 東京를 when read 도쿄 and 東京을 when read 동경.
 -  Check that native Korean number words remain in Hangul and retain correct
    spacing, including 두 番째. Check other numerical expressions, punctuation,
    and literal machine text. Compare an edited source for vocabulary, numbers,
    negation, conditions, and meaning.
 -  With a configured renderer, verify tricky readings in its actual output,
    including Hangul-only output. 茶禮 may need 차례 and 다례 in different
    places; a single document-wide mapping may not express both. Preserve
    distinct name code points and variation selectors; Unicode normalization is
    not a substitute for lexical form selection.

When Python 3 is available, run the bundled form checker on the finished Korean
prose or a prose-only excerpt before delivery:

~~~~ sh
python3 <skill-directory>/scripts/check_forms.py prose.txt
~~~~

Use `-` instead of a file name to read standard input without creating a file.

Resolve each warning against the word's meaning and protected-name rules.
For example, a leftover 大衆 in ordinary prose should become 大眾. The checker
only flags alternative characters from the preference table; it does not edit
text, select homophones, validate readings, or distinguish code and foreign
names. If it is unavailable, perform the same explicit comparison manually.

Report unresolved choices and verification limits outside the finished prose.
Do not silently invent a character, reading, spelling, or typesetting policy.

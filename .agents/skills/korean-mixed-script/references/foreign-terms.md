Foreign common nouns and specialist terms
=========================================

Choose wording for the concept
------------------------------

Distinguish an ordinary term from a proper name and from literal code before
choosing its script. A library's documentation can contain all three. Being
specialized, capitalized in a heading, or written in English in the source
does not make an ordinary noun a brand or code identifier.

Prefer an established Korean translation for the intended sense when one is
reasonably current in the field or the relevant project's Korean community.
It need not be the only common form or more popular than a transcription.
Write its Sino-Korean morphemes in Hanja. If no such translation is established,
transcribe the foreign term into Hangul under Korean loanword spelling rules.
Keep the specific Han-spelling policy for Chinese, Japanese, and Vietnamese
loans; the Hangul default does not repeal that policy.

Research uncertain terminology in official Korean documentation and sustained
Korean community usage for the actual library, framework, or toolchain. A
dictionary's everyday sense, an isolated machine translation, or a proposed
coinage is not sufficient evidence of an accepted technical translation.
Avoid literal translations that change the concept. Ask when competing usages
or an unclear boundary between a term and a name leaves a material choice.
The author's explicit choices below do not need to be reopened just because
another source uses a different common form.

| Concept in context            | Selected Korean wording | First-mention example                                     |
| ----------------------------- | ----------------------- | --------------------------------------------------------- |
| iterator in programming       | 反復子                  | 反復子(iterator)                                          |
| packet in network programming | 패킷                    | 패킷(packet), when a gloss helps                          |
| wheel in Python packaging     | 휠                      | 휠(wheel)                                                 |
| free software movement        | 自由 소프트웨어 運動    | 自由 소프트웨어 運動(free software movement), when useful |

Do not force packet into 보쌈 or translate the packaging term wheel as an
ordinary wheel. Use a parenthesized original at first mention when it helps
identify the term, especially for project-specific vocabulary without a
settled translation. Later prose normally uses the selected Korean form;
an original-language gloss does not authorize leaving ordinary nouns in Latin
script throughout the document.


Preserve code and distinguish project names
-------------------------------------------

Keep actual code and existing inline-code spans unchanged. In Markdown, wrap
inline code in backticks; in HTML, use `<code>`. Identifiers, commands, paths,
URLs, and other literal syntax retain their original spelling. Do not newly
wrap an ordinary English noun in code markup merely to avoid the term rules.

For example, prose about an iterator uses 反復子, while a literal identifier
`iterator` or `Iterator` stays unchanged. The Python packaging artifact is a
휠; a literal filename such as `package-1.0-py3-none-any.whl` stays code.
Do not infer a translation rule from the spelling of an API identifier.

Technology and project names use [the proper-name rules](names-and-readings.md).
The author's examples retain Python, Rust, React, Git, and Linux as names;
they do not authorize retaining every common English noun in prose about them.
Derived organization names can combine a retained name and Korean wording,
as in Linux 財團 and GNU 프로젝트. Free Software Foundation is 自由
소프트웨어 財團, using the established Korean phrase rather than treating
Free Software as a Latin brand.

When converting an existing text, preserve meaning and account for any foreign
term that becomes a Korean translation or transcription. Preserve the source
vocabulary of Korean quotations under the quotation rules; terminology
preferences do not authorize silently paraphrasing a quotation.


Review the whole document
-------------------------

Check later sections as carefully as the opening: long technical drafts may
start with Korean terms and then leave common nouns in English. Inspect the
remaining Latin spans in running prose, lists, captions, and headings. Classify
them as retained proper names, first-mention originals, preserved quotations,
code/literal notation, or common terms that still need Korean wording.

Check the selected term against its actual sense and community, keep the
wording consistent across the text, and avoid using code markup as a substitute
for translation. The bundled form checker flags selected Hanja variants; it
does not recognize untranslated English nouns or establish technical usage.

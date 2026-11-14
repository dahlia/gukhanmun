---
title: Markdown 變換
description: Python에서 CommonMark와 GitHub Flavored Markdown을 變換하며, 코드와 링크 目的地, 原始 HTML 블록을 保存하는 方法을 說明합니다.
---

Markdown 變換
=============

`InputFormat.MARKDOWN`은 CommonMark를 읽고 HTML 代身 Markdown을 씁니다.

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[MapDictionarySource([
    DictionaryEntry("漢字", "한자"),
])])
output = converter.convert("# 漢字\n\n漢字를 읽습니다.", format=InputFormat.MARKDOWN)
~~~~


GitHub Flavored Markdown
------------------------

表·作業 목록·取消線 같은 GFM 擴張을 使用하려면 `MarkdownOptions(gfm=True)`를
指定합니다. `MarkdownOptions()`는 `InputFormat.MARKDOWN`과 같습니다.

~~~~ python
from gukhanmun import MarkdownOptions

assert converter.convert("~~漢字~~", format=MarkdownOptions(gfm=True)) == "~~한자~~"
~~~~


變換 範圍와 保存 範圍
---------------------

段落·標題·목록 項目·引用文과 링크/이미지 標示 文句는 變換합니다. GFM을
켜면 表 셀(cell)도 變換합니다. 코드 블록(block), 인라인(inline) 코드,
링크/이미지의 目的地와 原始 HTML 블록은 保存합니다.

인라인 HTML 태그는 마크업으로 남습니다. 인라인 태그 사이의 텍스트는
變換 對象입니다. 둘러싼 `<code>` 같은 保存 태그나 繼承한 非韓國語 `lang`
屬性은 變換을 막습니다. 原始 HTML 블록은 全體를 保存합니다.
完全한 HTML 조각(fragment)을 處理하려면
[HTML 모드](./html.md)를 使用합니다.


出力과 렌더링
-------------

Markdown 어댑터(adapter)는 文書를 解析하고 直列化합니다. 文書 構造는
保存하지만 空白이나 목록 標識 같은 形式의 細部를 正規化할 수 있으므로, 바이트
單位의 往復 一致를 保證하지는 않습니다. 텍스트가 Markdown 文法을 만들 수
있는 境遇에는 必要에 따라 이스케이프(escape)합니다.

[루비(ruby) 렌더링](./rendering.md#루비-마크업)은 스코프(scope)가 許容하는 곳에
인라인 HTML을 내보냅니다. 結果를 表示하는 애플리케이션이 그 HTML을 許容해야
루비 註解(annotation)가 보입니다. 目的地가 인라인 HTML을 除去한다면 括弧
렌더링 모드를 選擇합니다.

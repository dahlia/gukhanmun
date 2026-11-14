---
title: HTML 處理
description: Python에서 HTML 조각을 變換하고, 選擇한 하위 트리를 保存하며, 루비 마크업과 嚴格하거나 寬大한 復舊를 使用하는 方法을 說明합니다.
---

HTML 處理
=========

HTML 조각(fragment)을 變換하려면 `InputFormat.HTML`을 使用합니다. 태그와
屬性은 出力에 남고, 變換은 對象 텍스트 노드(node)에 適用됩니다.

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[MapDictionarySource([
    DictionaryEntry("漢字", "한자"),
])])
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == "<p>한자</p>"
~~~~


基本 保存
---------

`<pre>`, `<code>`, `<kbd>`, `<script>`, `<style>`, `<textarea>`는 그 內容과
子孫을 保存합니다. 繼承한 `lang` 屬性도 變換을 制御합니다. 韓國語 태그인
`ko`나 `kor`, 또는 `ko-KR`처럼 하위 태그를 더한 形態는 大小文字 區別 없이
變換을 許容하며, 다른 言語
태그는 텍스트를 保存합니다. 內部의 `lang`은 繼承한 言語를 바꿀 수 있습니다.

~~~~ python
assert converter.convert("<code>漢字</code>", format=InputFormat.HTML) == (
    "<code>漢字</code>"
)
~~~~

`translate="no"` 같은 다른 保存 慣例에는 明示的인 規則을 使用합니다.


使用者 定義 保存
----------------

`HtmlOptions`는 클래스(class)와 屬性 規則을 더합니다. 어느 規則이든 要素에
一致하면 그 要素와 모든 子孫을 保存합니다.

~~~~ python
from gukhanmun import AttributeMatcher, HtmlOptions

converter = Converter(
    dictionaries=[MapDictionarySource([DictionaryEntry("漢字", "한자")])],
    html=HtmlOptions(
        preserve_classes=["keep"],
        preserve_attributes=[
            AttributeMatcher("data-keep"),
            AttributeMatcher("translate", "no"),
        ],
    ),
)
assert converter.convert(
    '<p class="keep"><b>漢字</b></p>', format=InputFormat.HTML,
) == '<p class="keep"><b>漢字</b></p>'
~~~~

클래스名은 ASCII 空白으로 나뉜 토큰(token)이며 大小文字를 區別합니다.
`AttributeMatcher(name)`은 存在를, `AttributeMatcher(name, value)`는 디코딩한
값을 檢査합니다. 屬性名은 ASCII 大小文字를 區別하지 않지만 값은 區別합니다.
`AttributeMatcher` 客體를 傳達해야 하며, 屬性名 文字列만은 받지 않습니다.


루비 마크업
-----------

HTML에 `RenderMode.RUBY_ON_HANGUL`, `RUBY_ON_HANJA`, 또는
`OriginalGloss.RUBY`를 指定한 原文 모드를 結合하면 루비(ruby) 註解(annotation)를
만듭니다. `<title>`·`<option>` 같은 텍스트 專用 要素는 括弧 폴백(fallback)을
使用합니다. 完全한 例題는 〈[렌더링 모드](./rendering.md#루비-마크업)〉를
參照하십시오.


誤謬 復舊
---------

基本값인 `Recovery.STRICT`는 HTML 스캐너(scanner)가 處理할 수 없는 잘못된
入力을 만나면 `GukhanmunError`를 發生시킵니다. `Recovery.LENIENT`는 잘못된
範圍를 그대로 保存하고 認識할 수 있는 部分은 變換합니다.

~~~~ python
from gukhanmun import Recovery

converter = Converter(recovery=Recovery.LENIENT)
assert converter.convert(
    "<p>學校 <![CDATA[北京</p>", format=InputFormat.HTML,
) == "<p>학교 <![CDATA[北京</p>"
~~~~

寬大한 復舊는 HTML을 修復하거나 無害化하지 않습니다. 認識하지 못한 範圍를
維持하며, 끝나지 않은 構造의 나머지 全體가 그 範圍에 들어갈 수도 있습니다.
이 옵션은 平文 텍스트나 Markdown에 影響을 주지 않습니다. 코드와 原因 連鎖는
[誤謬 參考 文書](../../api/python.md#誤謬)를 參照하십시오.

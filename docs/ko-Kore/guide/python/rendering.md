---
title: 렌더링 모드
description: Python 렌더링 모드를 比較하고, 루비나 括弧로 讀音을 倂記하며, 出力할 漢字의 異體字 集合을 選擇하는 方法을 說明합니다.
---

렌더링 모드
===========

`rendering`은 認識한 單語와 讀音을 어떻게 表示할지 制御합니다. 다음 表는
同音異義語나 指示 標識 없이, 辭典 讀音이 `한자`인 `漢字`를 보여줍니다.

| `RenderMode`          | 平文 텍스트 出力 |
| --------------------- | ---------------- |
| `HANGUL_ONLY` (基本)  | 한자             |
| `HANGUL_HANJA_PARENS` | 한자(漢字)       |
| `HANJA_HANGUL_PARENS` | 漢字(한자)       |
| `RUBY_ON_HANGUL`      | 한자(漢字)       |
| `RUBY_ON_HANJA`       | 漢字(한자)       |
| `ORIGINAL`            | 漢字             |

~~~~ python
from gukhanmun import Converter, DictionaryEntry, MapDictionarySource, RenderMode

source = MapDictionarySource([DictionaryEntry("漢字", "한자")])
converter = Converter(
    dictionaries=[source],
    rendering=RenderMode.HANGUL_HANJA_PARENS,
)
assert converter.convert("漢字") == "한자(漢字)"
~~~~


루비 마크업
-----------

루비(ruby) 모드는 HTML과 Markdown에서 인라인(inline) 마크업을 許容하는
位置에 `<ruby>` 要素를 만듭니다. 平文 텍스트와 `<title>`·`<option>` 같은
텍스트 專用 HTML 要素에서는 括弧를 代身 使用합니다.

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource, RenderMode

converter = Converter(
    dictionaries=[MapDictionarySource([DictionaryEntry("漢字", "한자")])],
    rendering=RenderMode.RUBY_ON_HANJA,
)
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == (
    "<p><ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby></p>"
)
~~~~

`RUBY_ON_HANGUL`은 反對로 한글을 基底로 삼고 `<rt>` 안에 漢字를 넣습니다.
`<rp>` 要素는 루비를 支援하지 않는 브라우저에 括弧를 提供합니다.


讀音을 倂記하는 原文 모드
-------------------------

`ORIGINAL`은 漢字를 維持하고, 註解(annotation)에 `require_hangul` 標識가
있을 때 讀音을 더합니다. 이 모드에서는 同音異義語 判別만으로 讀音이
追加되지는 않습니다. `original_gloss`는 그 讀音을
括弧나 루비 中 어느 쪽으로 表示할지 選擇합니다.

~~~~ python
from gukhanmun import (
    Converter, DictionaryEntry, InputFormat, MapDictionarySource,
    OriginalGloss, RenderMode,
)

converter = Converter(
    dictionaries=[MapDictionarySource([
        DictionaryEntry("漢字", "한자", require_hangul=True),
    ])],
    rendering=RenderMode.ORIGINAL,
    original_gloss=OriginalGloss.RUBY,
)
assert converter.convert("漢字", format=InputFormat.HTML) == (
    "<ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>"
)
assert converter.convert("漢字") == "漢字(한자)"
~~~~

基本값은 `OriginalGloss.PARENS`입니다. `original_gloss`는
`RenderMode.ORIGINAL`에만 影響을 주며, 主 렌더링 모드를 選擇하지는 않습니다.


異體字 集合
-----------

漢字는 여러 異體字 表記를 가질 수 있습니다. 支援하는 異體字의 認識은 恒常
켜져 있습니다. `hanja_variant_set`는 漢字를 남기는 모드가
認識 後에 내보낼 表記를 制御하며, 辭典이나 讀音을 選擇하지는 않습니다.

~~~~ python
from gukhanmun import Converter, HanjaVariantSet, RenderMode

converter = Converter(
    rendering=RenderMode.ORIGINAL,
    hanja_variant_set=HanjaVariantSet.SHINJITAI,
)
~~~~

| `HanjaVariantSet`      | 出力 表記                                                        |
| ---------------------- | ---------------------------------------------------------------- |
| `AS_DICTIONARY` (基本) | 辭典이 選擇한 表記를 維持함                                      |
| `SHINJITAI`            | 日本의 常用 漢字에 쓰이는 新字體                                 |
| `KANXI`                | 傳統 字形과 互換 文字 對應을 適用한 形態를 優先함                |
| `SIMPLIFIED`           | 使用할 수 있으면 Unicode `kSimplifiedVariant`의 첫 對應을 使用함 |
| `ASAHIMOJI`            | 新字體에 Gukhanmun이 檢證한 追加 字形 對應을 더함                |

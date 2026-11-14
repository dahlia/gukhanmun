---
title: 變換 옵션
description: Python 프리셋, 分割과 數詞 戰略을 選擇하고 同音異義語 倂記, 最初 出現 解除와 文章 符號를 制御하는 方法을 說明합니다.
---

變換 옵션
=========

`Converter(...)`에 키워드 引數로 옵션을 傳達합니다. 生成 後에는 設定을 바꿀
수 없으므로, 다른 設定에는 새 變換器를 만듭니다. 文字列을 直接 넘기는 代身
`Preset.KO_KP` 같은 列擧型(enum) 멤버를 使用합니다. 設定에서 읽은 文字列은
`Preset("ko-kp")`처럼 明示的으로 變換합니다.


프리셋
------

`preset`은 頭音法則과 同音異義語(homophone) 窓의 基本값을 提供합니다.

| 프리셋(preset)        | 頭音法則 | 同音異義語 窓             |
| --------------------- | -------- | ------------------------- |
| `Preset.KO_KR` (基本) | `True`   | `ContextWindow.PER_BLOCK` |
| `Preset.KO_KP`        | `False`  | `ContextWindow.OFF`       |

어느 프리셋도 公式 辭典을 불러오지 않습니다. 明示的으로 設置하고 追加해야
합니다. 〈[辭典](./dictionaries.md)〉을 參照하십시오. 個別 옵션을 指定하면
프리셋의 基本값보다 優先합니다.

~~~~ python
from gukhanmun import ContextWindow, Converter, Preset

converter = Converter(
    preset=Preset.KO_KP,
    initial_sound_law=True,
    homophone_window=ContextWindow.PER_DOCUMENT,
)
assert converter.options.initial_sound_law is True
~~~~

`converter.options`는 完全히 決定된 文章 符號 프로파일(profile)을 包含하여,
實際 適用된 값을 담습니다. 이 값과 內部 레코드는 읽기 專用입니다.


分割
----

`segmentation`은 辭典 一致로 漢字 連續을 單語로 나누는 方式을 選擇합니다.
基本값인 `Segmentation.LATTICE`는 格子(lattice)의 候補 經路를 動的 計劃法으로
比較합니다. `Segmentation.EAGER`는 各 位置에서 가장 긴 一致를 고릅니다.
긴 一致가 뒤에 辭典에 없는 조각을 남기면 더 나쁜 境界를 選擇할 수 있습니다.

~~~~ python
from gukhanmun import Converter, Segmentation

converter = Converter(segmentation=Segmentation.LATTICE)
~~~~

一般的인 文書 變換에는 基本값을 維持합니다. `EAGER`를 選擇하려면 自身의
語彙로 먼저 評價하십시오.


數詞
----

`numerals`는 漢字 數詞 連續의 變換을 制御합니다. 다음 例示는 辭典 없이 文字
單位 폴백(fallback)을 使用합니다.

| `NumeralStrategy`        | 二〇一六年 | 十一月 | 一千二百三十四 |
| ------------------------ | ---------- | ------ | -------------- |
| `HANGUL_PHONETIC` (基本) | 이공일륙년 | 십일월 | 일천이백삼십사 |
| `POSITIONAL_ARABIC`      | 2016년     | 십일월 | 일천이백삼십사 |
| `ADDITIVE_ARABIC`        | 이공일륙년 | 11월   | 1234           |
| `SMART`                  | 2016년     | 11월   | 1234           |

~~~~ python
from gukhanmun import Converter, NumeralStrategy

converter = Converter(numerals=NumeralStrategy.SMART)
assert converter.convert("二〇一六年 十一月") == "2016년 11월"
assert converter.convert("百濟 十長生") == "백제 십장생"
~~~~

`POSITIONAL_ARABIC`은 자리別 數字 連續을, `ADDITIVE_ARABIC`은 十·百 같은
자리 標識가 있는 數量을 處理합니다. `SMART`는 둘을 結合하며, 例示의
十長生처럼 單語로 읽히는 連續은 音讀을 維持합니다.


頭音法則
--------

`initial_sound_law`는 單語 첫머리의 特定 폴백 讀音을 南韓式으로 바꿉니다.
例를 들어 `來日`은 켜면 `내일`, 끄면 `래일`로 읽힙니다. 辭典 項目은 이미
讀音을 提供하므로 이 設定이 그 讀音을 다시 쓰지는 않습니다.

~~~~ python
from gukhanmun import Converter

assert Converter(initial_sound_law=True).convert("來日") == "내일"
assert Converter(initial_sound_law=False).convert("來日") == "래일"
~~~~


括弧 讀音 註解
--------------

`庫間`을 `곳간`으로 읽는 辭典 項目이 있을 때, 基本값인
`collapse_redundant_parens=True`는 `庫間(곳간)`이나 `곳간(庫間)`처럼 어느 쪽
順序로 쓰인 旣存 讀音 註解(annotation)도 認識하여, 重複하는 代身 하나의
註解로 렌더링합니다. 한글 專用 모드의 結果는 `곳간(庫間)`입니다. 이 處理를
끄려면 `False`를 指定합니다. 같은 入力 `庫間(곳간)`은 이때 `곳간(곳간)`이
됩니다. 單語는 變換되고 旣存 括弧는 남습니다.

辭典 讀音과 같은 倂記는 사이시옷이 들어간 讀音까지 認定합니다. 다른 讀音은
漢字마다 한글 音節이 하나씩 對應하고 各 音節이 該當 文字의 有效한 漢字音이어야
합니다. 따라서 `數字(수자)`는 그 出現 位置의 讀音으로 `수자`를 選擇할 수
있습니다. 이 規則을 滿足하지 않는 뜻풀이와 外國語 音譯은 그대로 둡니다.


同音異義語
----------

`homophone_window`는 같은 讀音을 가진 두 認識 單語가 區別用 漢字 倂記를
發生시킬 範圍를 定합니다. 한글 專用 렌더링에서는 다음과 같습니다.

~~~~ python
from gukhanmun import ContextWindow, Converter, DictionaryEntry, MapDictionarySource

converter = Converter(
    dictionaries=[MapDictionarySource([
        DictionaryEntry("甲", "한"),
        DictionaryEntry("乙", "한"),
    ])],
    homophone_window=ContextWindow.PER_DOCUMENT,
)
assert converter.convert("甲 乙") == "한(甲) 한(乙)"
assert converter.convert("甲") == "한"
~~~~

各 `convert()` 呼出은 새 文書와 그 文書만의 追跡 狀態로 始作합니다.

HTML과 Markdown 入力의 窓은 다음 構造的 範圍를 使用합니다.

| 窓                           | 範圍                       |
| ---------------------------- | -------------------------- |
| `ContextWindow.OFF`          | 同音異義語를 追跡하지 않음 |
| `ContextWindow.PER_BLOCK`    | 段落·목록·標題 블록        |
| `ContextWindow.PER_SECTION`  | 다음 標題까지              |
| `ContextWindow.PER_DOCUMENT` | 入力 全體                  |

`InputFormat.TEXT`에서는 `PER_BLOCK`과 `PER_SECTION`이 모두 文書 全體를
範圍로 삼습니다. 빈 줄과 標題처럼 보이는 텍스트는 境界를 만들지 않습니다.
이 入力 形式別 範圍는 `first_occurrence_window`에도 適用됩니다.

`homophone_detection`의 基本값은 `HomophoneDetection.CONTEXT_LOCAL`이며,
衝突하는 單語가 窓 안에 實際로 나타나야 합니다. `DICTIONARY_WIDE`는 불러온 辭典
어디에든 있는 같은 讀音의 다른 表記도 考慮합니다. 큰 辭典에서는 많은 倂記가
생길 수 있습니다. 特定 表記를 恒常 倂記하려면 [指示](./directives.md)를
使用합니다.

同音異義語 判別은 認識된 辭典 單位에 適用됩니다. 文字 單位 폴백으로 읽은
連續이 自動으로 語彙 識別性을 가진 單語가 되지는 않습니다. 用語 全體를
同音異義語 判別에 參與시키려면 [辭典 項目](./dictionaries.md)을 追加합니다.


最初 出現 解除
--------------

`first_occurrence_window`는 選擇한 窓 안에서 反復되는 辭典 項目의
`require_hanja`와 `require_hangul` 要求를 解除합니다. 入力의 明示的 讀音
註解와 指示는 要求를 維持하며, 同音異義語 標識도 남습니다. 두 文字를 恒常
表示하는 렌더링 모드는
그대로 兩쪽을 表示합니다. 같은 네 가지 `ContextWindow` 값을 받으며
基本값은 `OFF`이고, 同音異義語 窓과는 獨立的입니다.

~~~~ python
from gukhanmun import ContextWindow, Converter, DictionaryEntry, MapDictionarySource

converter = Converter(
    dictionaries=[MapDictionarySource([
        DictionaryEntry("漢字", "한자", require_hanja=True),
    ])],
    first_occurrence_window=ContextWindow.PER_DOCUMENT,
)
assert converter.convert("漢字 漢字") == "한자(漢字) 한자"
~~~~


文章 符號와 復舊
----------------

文章 符號 正規化는 辭典과 렌더링 選擇에 獨立的입니다.

~~~~ python
from gukhanmun import Converter, CustomPunctuationProfile, PunctuationPreset

converter = Converter(punctuation=CustomPunctuationProfile(
    base=PunctuationPreset.MODERN,
    period=None,
))
assert converter.convert("가、나。 다") == "가, 나。 다"
~~~~

省略한 使用者 定義 필드는 `UNSET`을 通해 基底 프로파일을 繼承하고, `None`은 該當
役割의 入力 文章 符號를 維持합니다. 認識하는 文章 符號는
〈[文章 符號 프로파일](../punctuation.md)〉, Python 타입은
[API 參考 文書](../../api/python.md#文章-符號-프로파일)를 參照하십시오.

`recovery`는 잘못된 HTML의 處理를 制御하며, 平文 텍스트나 Markdown 變換에는
影響을 주지 않습니다. 〈[HTML 處理](./html.md#誤謬-復舊)〉를 參照하십시오.

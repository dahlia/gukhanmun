---
title: 指示
description: Python의 리터럴 漢字 指示로 漢字나 한글 讀音을 要求하거나, 讀音 選擇을 바꾸지 않고 註解를 抑制하는 方法을 說明합니다.
---

指示
====

`Directives`는 選擇된 讀音을 어떻게 表示할지 調整합니다. 辭典 項目을
追加하거나 다른 讀音을 選擇하지는 않습니다. 세 시퀀스(sequence)에는
글로브(glob) 패턴이나 Python 콜백(callback) 代身 리터럴(literal) 漢字 表記를
넣습니다.

~~~~ python
from gukhanmun import Converter, DictionaryEntry, Directives, MapDictionarySource

converter = Converter(
    dictionaries=[MapDictionarySource([DictionaryEntry("漢字", "한자")])],
    directives=Directives(require_hanja=["漢字"]),
)
assert converter.convert("漢字") == "한자(漢字)"
~~~~

規則은 變換 註解(annotation)에 適用됩니다. 用語가 하나의 單位로 認識되어야
한다면 그 用語 全體를 [辭典](./dictionaries.md)에 追加합니다. 辭典의 識別
메타데이터는 다른 表記를 正準 表記의 檢討된 別名(alias)으로 指定할 수
있습니다. 이때 正準 表記의 리터럴 規則이 그 別名에도 適用됩니다.


렌더링과 指示의 結合
--------------------

| 필드              | 效果                                  |
| ----------------- | ------------------------------------- |
| `require_hanja`   | 한글 專用 모드에서도 漢字를 보이게 함 |
| `require_hangul`  | 原文 모드에서도 한글 讀音을 倂記함    |
| `skip_annotation` | 活性 모드의 主 텍스트만 使用함        |

`ORIGINAL`에서 要求한 한글 讀音은 倂記로 나타납니다.

~~~~ python
from gukhanmun import Converter, DictionaryEntry, Directives, MapDictionarySource, RenderMode

converter = Converter(
    dictionaries=[MapDictionarySource([DictionaryEntry("漢字", "한자")])],
    rendering=RenderMode.ORIGINAL,
    directives=Directives(require_hangul=["漢字"]),
)
assert converter.convert("漢字") == "漢字(한자)"
~~~~

`skip_annotation`은 入力을 그대로 保存하는 代身 括弧와 루비(ruby)를
抑制합니다. 한글을 主로 하는 모드는 讀音을, 漢字를 主로 하는 모드는 漢字를
남깁니다. HTML 하위 트리(subtree)를 變換하지 않고 두려면
[HTML 保存 規則](./html.md#使用者-定義-保存)을 使用합니다.

하나의 表記를 `require_hanja`와 `require_hangul`에 모두 넣을 수 있습니다.
`skip_annotation`이 있으면 다른 要求가 있어도 註解 렌더링을 抑制합니다.
入力 시퀀스는 生成 時 變更할 수 없는 튜플(tuple)로 複寫됩니다.

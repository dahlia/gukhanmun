---
title: Python 빠른 始作
description: 再使用 可能한 Gukhanmun 變換器, 明示的 辭典과 타입이 指定된 設定으로 Python에서 平文 텍스트, HTML, Markdown을 變換합니다.
---

Python 빠른 始作
================

例題를 實行하기 前에 [Gukhanmun을 設置](./install.mdx)합니다.
`Converter(...)`는 Rust 基盤의 再使用 可能한 變換器를 만듭니다. 한 番
設定한 뒤 文書마다 `convert()`를 呼出합니다.


文書 變換
---------

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[
    MapDictionarySource(entries=[DictionaryEntry(hanja="漢字", reading="한자")]),
])

assert converter.convert("漢字를 한글로") == "한자를 한글로"
assert converter.convert("<p>漢字</p>", format=InputFormat.HTML) == "<p>한자</p>"
~~~~

基本 形式은 平文 텍스트입니다. CommonMark에는 `InputFormat.MARKDOWN`을,
GitHub Flavored Markdown에는 `MarkdownOptions(gfm=True)`를 使用합니다.

~~~~ python
from gukhanmun import MarkdownOptions

result = converter.convert("# 漢字", format=MarkdownOptions(gfm=True))
~~~~

設定에는 實際 列擧型 멤버를 傳達합니다. 設定 파일에서 읽은 文字列은
`Preset("ko-kr")`처럼 明示的으로 變換합니다. `Converter()`에
`preset="ko-kr"`를 直接 傳達하면 `TypeError`가 發生합니다.

어느 프리셋도 公式 辭典을 自動으로 불러오지 않습니다. 辭典을 指定하지 않으면
內藏된 漢字 音價로 變換합니다. 單語 單位의 讀音이 必要하면 實行 中에 만든
項目이나 컴파일된 FST/CDB 辭典을 追加하거나,
[公式 辭典](./dictionaries.md)을 設置하여 追加합니다.


變換器 再使用
-------------

辭典 리소스(resource)는 變換器를 生成할 때 불러오므로, 한 變換器를 設定하여
여러 文書에 再使用하십시오. 設定은 變更할 수 없습니다. `converter.options`로
解決된 設定을 確認하거나, 다른 設定이 必要하면 새 變換器를 만듭니다.
〈[文書 竝列 處理](./streaming.md#文書-竝列-處理)〉에서 說明한 것처럼
여러 스레드(thread)가 變換器 하나를 共有할 수 있습니다.


다음 作業 選擇
--------------

| 作業                                   | 案內                           |
| -------------------------------------- | ------------------------------ |
| 公式·메모리 內·컴파일된 辭典 追加      | [辭典](./dictionaries.md)      |
| 프리셋·數詞·註解 範圍 設定             | [變換 옵션](./conversion.md)   |
| 括弧·루비·原文 比較                    | [렌더링 모드](./rendering.md)  |
| 特定 語彙의 註解 強制나 抑制           | [指示](./directives.md)        |
| HTML 하위 트리 保存과 잘못된 入力 處理 | [HTML 處理](./html.md)         |
| Markdown·GFM 變換                      | [Markdown 變換](./markdown.md) |
| 청크 入力이나 文書 竝列 變換           | [스트리밍 API](./streaming.md) |


誤謬 處理
---------

[API 參考 文書](../../api/python.md#誤謬)는 `GukhanmunError`와 `ErrorCode`,
根本 原因부터 나열하는 `chain`을 說明합니다. 引數의 타입이 잘못되면
`TypeError`가, 入力에 짝이 없는 Unicode 서로게이트(surrogate)가 있으면
`UnicodeEncodeError`가 發生합니다.

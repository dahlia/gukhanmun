---
title: 스트리밍 API
description: Python의 버퍼링 變換 스트림과 反復子를 使用하고, 終了를 處理하며, 文書 處理 스레드 사이에서 變換器를 共有하는 方法을 說明합니다.
---

스트리밍 API
============

`stream()`과 `convert_iter()`는 文書를 文字列 청크(chunk)로 받습니다. 現在
Python 具顯은 入力 全體를 버퍼링(buffering)하고 終了 時 變換합니다. 一括
變換과 같은 結果를 提供하지만, 限定된 메모리만 쓰거나 出力을 일찍 내보내는
스트리밍은 아닙니다.

文書를 어떤 청크로 나누어도, 스트림(stream) 出力을 合친 結果는 入力을 合쳐 한 番
`convert()`한 結果와 같습니다. 單語·HTML 태그·Markdown 構造 안의 境界가
變換을 바꾸지 않습니다. 이 保證은 `stream()`과 `convert_iter()` 모두에
適用됩니다.


入力과 終了
-----------

~~~~ python
from gukhanmun import Converter, DictionaryEntry, InputFormat, MapDictionarySource

converter = Converter(dictionaries=[MapDictionarySource([
    DictionaryEntry("漢字", "한자"),
])])
stream = converter.stream(format=InputFormat.HTML)
assert stream.push("<p>漢") == ""
assert stream.push("字</p>") == ""
assert stream.finish() == "<p>한자</p>"
~~~~

成功한 모든 `push()`는 빈 文字列을 返還합니다. `finish()`는 入力을 이어 붙여
한 番 變換하며, 빈 入力에도 適用됩니다. 形式은 `convert()`와 같은
`InputFormat`이나 `MarkdownOptions` 값을 받습니다.

스트림은 變換器에 對한 參照를 維持하므로, 스트림이 있는 동안
變換器도 維持됩니다. 各 文書에 別途의 스트림을 使用하고, 各 스트림에는
消費者를 하나만 둡니다. `finish()`의 成功,
`push()`의 失敗, 또는 `finish()`의 失敗는 스트림을 終了 狀態로 만듭니다.
그 後의 모든 入力과 終了 呼出은 `RuntimeError`를 發生시킵니다. 誤謬 後에
다시 試圖하려면 새 스트림을 만듭니다.


反復 可能한 入力 變換
---------------------

`convert_iter()`는 文字列 청크를 내놓는 反復 可能 客體(iterable)를 빈 값이
아닌 出力 청크의 反復子(iterator)로 바꿉니다.

~~~~ python
chunks = ["<p>漢", "字</p>"]
output = "".join(converter.convert_iter(chunks, format=InputFormat.HTML))
assert output == "<p>한자</p>"
~~~~

形式 檢査는 `convert_iter()`를 呼出할 때 이뤄집니다. 入力은 返還된 反復子가
進行할 때 消費됩니다. 現在 具顯은 入力이 正常的으로 끝난 뒤 結果 하나를
내놓으며, 빈 出力에는 아무것도 내놓지 않습니다. 入力 反復子가 例外를
發生시키거나 出力 反復子를 일찍 닫으면 不完全한 文書를 變換하지 않습니다.


文書 竝列 處理
--------------

獨立된 文書에 하나의 設定된 變換器를 再使用합니다. 네이티브(native) 變換은
全域 인터프리터 잠금(global interpreter lock, GIL)을 解除하고,
파서(parser)·同音異義語·最初 出現 解除 狀態를 各 呼出 안에 둡니다.

~~~~ python
from concurrent.futures import ThreadPoolExecutor

documents = ["漢字", "漢字를 읽습니다."]
with ThreadPoolExecutor(max_workers=2) as pool:
    results = list(pool.map(converter.convert, documents))
assert results == ["한자", "한자를 읽습니다."]
~~~~

이 때문에 變更 可能한 스트림을 여러 消費者가 安全하게 쓸 수 있는 것은
아닙니다. 變換器를 共有하고 各各의 스트림을 만드십시오. Python 信號는
네이티브 作業이 돌아온 뒤 檢査하므로, 긴 變換은 `KeyboardInterrupt`를 늦출 수
있습니다. asyncio 專用 인터페이스(interface)는 없습니다.

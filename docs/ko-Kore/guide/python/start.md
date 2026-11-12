---
title: Python 빠른 始作
description: Gukhanmun Python 패키지를 로컬에서 빌드하고, 明示的 辭典과 타입이 指定된 設定으로 平文 텍스트, HTML, Markdown을 變換하는 方法을 說明합니다.
---

Python 빠른 始作
================

`Converter(...)`는 Rust 基盤의 再使用 可能한 Python 變換器를 만듭니다.
한 番 設定한 뒤 文書마다 `convert()`를 呼出합니다. Python 3.11 以上이
必要하며, 實行에 必須的인 第三者 Python 패키지는 없습니다.


로컬 휠(wheel) 빌드와 設置
--------------------------

貯藏所 루트에서 開發 道具를 設置하고 패키지를 빌드합니다.

~~~~ sh
mise install
mise run py-build
~~~~

휠은 *target/python/wheels/* 아래에 生成됩니다. 實際 파일 經路를 指定하여
使用할 Python 環境에 設置합니다.

~~~~ sh
python -m pip install /path/to/gukhanmun.whl
~~~~

위 命令은 로컬에서 빌드한 파일을 設置합니다. PyPI에 配布된 버전이 없어도
實行할 수 있습니다. 릴리스 휠이 支援하는 플랫폼과 原始 코드 配布版에서
빌드하는 方法은 〈[支援 플랫폼](./platforms.md)〉을 參照하십시오. 貯藏所에서
패키지를 檢査하려면 다음을 實行합니다.

~~~~ sh
mise run py-test
mise run py-typecheck
~~~~

두 作業은 貯藏所 밖의 環境에 設置한 휠을 檢査합니다. `py-typecheck`는
同一한 使用 例題를 mypy, Pyright, basedpyright, ty, Pyrefly로 檢査합니다.


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


컴파일된 辭典 불러오기
----------------------

~~~~ python
from pathlib import Path
from gukhanmun import Converter, DictionaryFormat, FileDictionarySource

converter = Converter(dictionaries=[
    FileDictionarySource(Path("custom.gukfst"), format=DictionaryFormat.FST),
])
~~~~

`FileDictionarySource()`에는 `bytes`, `bytearray`, `memoryview`도 傳達할 수
있습니다. 變更 可能한 버퍼는 複寫됩니다. 파일은 變換器 生成 時 읽으며, 變換
呼出 中에는 辭典 入出力을 遂行하지 않습니다. 文字列은 URL처럼 보여도 恒常
파일 經路로 取扱합니다.

遠隔 辭典은 `UrlDictionarySource()`로 明示합니다.

~~~~ python
from gukhanmun import UrlDictionarySource

source = UrlDictionarySource(
    "https://example.com/custom.gukfst",
    format=DictionaryFormat.FST,
    timeout=30.0,
    max_bytes=64 * 1024 * 1024,
    total_timeout=30.0,
)
# Converter(dictionaries=[source])를 호출하면 이 예시 주소에 접근합니다.
~~~~

HTTP(S) URL만 許容합니다. 리다이렉트는 最初 URL과 스킴, 호스트 이름, 有效
포트가 같아야 하며, 다른 오리진은 要請을 보내기 前에 `INVALID_INPUT`으로
拒否합니다. 다른 오리진에서 提供하는 辭典은 最終 URL을 直接 指定하세요.
`timeout`은 秒 單位의 陽의 有限한 소켓 作業 制限 時間입니다. `max_bytes`는 辭典
本文 크기를, `total_timeout`은 리다이렉트 全體에 걸친 HTTP 應答 읽기를 制限하며
헤더와 청크 構文도 包含합니다. 同期 DNS/連結 設定 때문에 取消가 `total_timeout`
以後까지 遲延될 수 있습니다. 限度를 넘으면 `IO` 原因을 包含한 `DICTIONARY_LOAD`
誤謬가 發生합니다. 프록시 環境 變數는 使用하지 않습니다.

메모리 項目 辭典의 基本 役割은 `SourceRole.OVERRIDE`입니다. 바이너리 辭典은
明示한 役割, 內藏된 根據 資料의 役割, `OVERRIDE` 順序로 決定합니다. 파일
이름으로 役割을 推測하지 않습니다. 辭典 協力 政策은
[API 參考 文書](../../api/python.md)를 參考하세요.


出力과 文章 符號 設定
---------------------

~~~~ python
from gukhanmun import (
    Converter, CustomPunctuationProfile, PunctuationPreset, RenderMode,
)

converter = Converter(
    rendering=RenderMode.HANGUL_HANJA_PARENS,
    punctuation=CustomPunctuationProfile(
        base=PunctuationPreset.MODERN,
        period=None,
    ),
)
~~~~

使用者 文章 符號 프로파일(profile)에서 省略한 필드는 `UNSET`이며 基本
프로파일을 相續합니다. `None`은 該當 種類의 文章 符號를 入力대로 保存합니다.
`StopStyle`이나 `DelimiterStyle`로 出力 樣式을 直接 定할 수도 있습니다. 위
例題는 쉼표에 現代 樣式을 適用하고 마침표는 入力대로 維持합니다.

`converter.options`는 變更할 수 없는 `ResolvedOptions` 값입니다. 文章 符號
프로파일과 그 안의 樣式도 變更할 수 없습니다. 다른 設定에는 새 變換器를
만듭니다.


청크 變換
---------

~~~~ python
converter = Converter()
stream = converter.stream()
assert stream.push("漢") == ""
assert stream.push("字") == ""
assert stream.finish() == converter.convert("漢字")

assert "".join(converter.convert_iter(["漢", "字"])) == converter.convert("漢字")
~~~~

初期 스트림 具顯은 모든 청크를 모아 `finish()`에서만 結果를 돌려줍니다. 청크의
境界는 結果에 影響을 주지 않습니다. 스트림은 變換器를 繼續 參照하며, 하나의
消費者만 使用해야 합니다. 正常 終了 後 또는 入力/終了 失敗 後의 追加 呼出은
`RuntimeError`를 發生시킵니다. `convert_iter()`는 上位 反復子(iterator)의
誤謬를 그대로 傳達하며, 正常的으로 모든 入力을 읽었을 때만 스트림을 終了합니다.


誤謬 處理와 變換器 共有
-----------------------

~~~~ python
from gukhanmun import (
    Converter, DictionaryFormat, ErrorCode, FileDictionarySource, GukhanmunError,
)

try:
    Converter(dictionaries=[
        FileDictionarySource(b"invalid binary", format=DictionaryFormat.FST),
    ])
except GukhanmunError as error:
    assert error.code is ErrorCode.DICTIONARY_LOAD
    print(error.message)
    for cause in error.chain:
        print(cause.code, cause.message)
~~~~

`error.chain`은 가장 깊은 根本 原因부터 羅列한 變更 不可能한 튜플입니다.
引數의 타입이 틀리면 `TypeError`, 設定값이 有效하지 않으면 코드가 있는
`GukhanmunError`가 發生합니다. 入力 本文에 單獨 Unicode 서로게이트가 있으면
`UnicodeEncodeError`가 發生합니다. 辭典 項目, 指示, HTML 規則, 文章 符號,
URL 等 生成子의 텍스트 設定에 有效하지 않은 Unicode가 있으면
`ErrorCode.INVALID_INPUT`인 `GukhanmunError`가 發生합니다. 運營 體制가
인코딩할 수 없는 파일 經路는 `ErrorCode.DICTIONARY_LOAD`인
`GukhanmunError`를 發生시킵니다.

같은 變換器를 여러 스레드에서 使用할 수 있습니다. 네이티브 變換은 GIL을
解除하며 文書마다 別途의 構文 分析 및 註解(annotation) 狀態를 使用합니다.
Python 시그널은 네이티브 作業이 끝난 뒤 確認하므로, 긴 變換 中에는
`KeyboardInterrupt`가 遲延될 수 있습니다. 스트림은 變更 可能한 客體이므로 各各
하나의 消費者만 使用해야 합니다.

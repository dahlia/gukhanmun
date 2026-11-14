---
title: Python API
description: Gukhanmun Python 變換器의 生成子, 變更 不可能한 設定, 辭典 入力, 文章 符號 프로파일, 스트림 壽命과 誤謬 코드의 使用法을 說明합니다.
---

Python API
==========

公開 타입은 `gukhanmun`에서 가져옵니다. 擴張 모듈 `gukhanmun._native`는
非公開입니다. PyPI 設置는 〈[設置](../guide/python/install.mdx)〉를, 基本 例題는
[Python 案內](../guide/python/start.md)를 參考하세요.


變換器
------

`Converter(...)`는 키워드 引數로 設定하며, 生成 後 變更할 수 없는 再使用
可能한 變換器를 만듭니다. 傳達한 시퀀스는 所有權을 갖는 不變 스냅숏으로
複寫됩니다. 辭典 데이터와 파일은 生成 時 불러옵니다.

| 引數                        | 타입                                            | 基本값                              |
| --------------------------- | ----------------------------------------------- | ----------------------------------- |
| `preset`                    | `Preset`                                        | `Preset.KO_KR`                      |
| `rendering`                 | `RenderMode`                                    | `RenderMode.HANGUL_ONLY`            |
| `original_gloss`            | `OriginalGloss`                                 | `OriginalGloss.PARENS`              |
| `hanja_variant_set`         | `HanjaVariantSet`                               | `HanjaVariantSet.AS_DICTIONARY`     |
| `segmentation`              | `Segmentation`                                  | `Segmentation.LATTICE`              |
| `source_policy`             | `SourcePolicy`                                  | `SourcePolicy.REFERENCE_AUTHORIZED` |
| `reference_only`            | `bool`                                          | `False`                             |
| `numerals`                  | `NumeralStrategy`                               | `NumeralStrategy.HANGUL_PHONETIC`   |
| `initial_sound_law`         | `bool \| None`                                  | `None`, 프리셋 相續                 |
| `homophone_window`          | `ContextWindow \| None`                         | `None`, 프리셋 相續                 |
| `homophone_detection`       | `HomophoneDetection`                            | `HomophoneDetection.CONTEXT_LOCAL`  |
| `first_occurrence_window`   | `ContextWindow`                                 | `ContextWindow.OFF`                 |
| `collapse_redundant_parens` | `bool`                                          | `True`                              |
| `recovery`                  | `Recovery`                                      | `Recovery.STRICT`                   |
| `punctuation`               | `PunctuationPreset \| CustomPunctuationProfile` | `PunctuationPreset.PRESERVE`        |
| `dictionaries`              | 파일/URL/메모리 辭典의 시퀀스                   | `()`                                |
| `directives`                | `Directives \| None`                            | `None`                              |
| `html`                      | `HtmlOptions \| None`                           | `None`                              |

`KO_KR`의 相續값은 `initial_sound_law=True`와
`homophone_window=ContextWindow.PER_BLOCK`입니다. `KO_KP`는 各各
`False`와 `ContextWindow.OFF`입니다. 어느 프리셋도 公式 辭典을 自動으로
불러오지 않습니다. `original_gloss`는 `RenderMode.ORIGINAL`에만 適用하며,
漢字 字形은 辭典 認識 後 選擇합니다.

읽기 專用 屬性 `options`는 `ResolvedOptions`를 返還합니다. 필드는 위
키워드에서 `original_gloss`, `dictionaries`, `directives`, `html`을 除外한
열네 個입니다. 各 값은 指定된 列擧型 타입을 使用하며, `punctuation`은
`UNSET`이 남아 있지 않은 `ResolvedPunctuationProfile`입니다. 프로파일(profile)과
內部 레코드도 變更할 수 없습니다.


變換 演算
---------

~~~~ text
convert(source: str, *, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> str
~~~~

`InputFormat.TEXT`는 平文 텍스트, `HTML`은 HTML 조각(fragment), `MARKDOWN`은
CommonMark를 變換합니다. `MarkdownOptions(gfm=False)`는 `MARKDOWN`과
같고, `gfm=True`는 GitHub Flavored Markdown을 活性化합니다. 인라인 마크업을
使用할 수 없는 곳에서는 루비(ruby) 代身 括弧로 表記합니다.

~~~~ text
stream(*, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> ConversionStream
convert_iter(chunks: Iterable[str], *, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> Iterator[str]
~~~~

`ConversionStream.push(source: str) -> str`는 청크를 貯藏하고 `""`를
返還합니다. `finish() -> str`는 모은 入力을 한 番 變換합니다. 스트림은
變換器를 繼續 參照하며 하나의 消費者만 使用해야 합니다. 正常 終了 後 또는
入力/終了 失敗 後에는 終了 狀態가 되어, 追加 呼出 時 `RuntimeError`가
發生합니다.

`convert_iter()`는 呼出 卽時 形式을 檢證하고, 返還한 反復子(iterator)를 進行할
때 청크를 읽습니다. 빈 結果는 省略하며, 上位 反復子가 正常 終了했을 때만 變換을
마칩니다. 上位 反復子의 誤謬나 早期 終了 時에는 一部 文書를 마무리하지
않습니다. 入力을 어느 境界에서 나누어도 스트림/反復子 結果를 합치면 全體 入力에
`convert()`를 適用한 結果와 같습니다.

하나의 變換器를 여러 스레드에서 共有할 수 있으며, 各 스트림의 文書 狀態는
獨立的입니다. 네이티브 作業은 GIL을 解除합니다. 待機 中인 Python 시그널은
네이티브 作業이 끝난 뒤 確認하므로 긴 呼出은 `KeyboardInterrupt`를 遲延할
수 있습니다.


設定 列擧型
-----------

標準 라이브러리의 `enum.Enum` 클래스입니다. 값은 JavaScript 契約의 文字列과
같습니다. 멤버 이름은 大文字이며 하이픈을 밑줄로 바꿉니다. 例를 들어
`RenderMode.HANGUL_ONLY.value == "hangul-only"`입니다. 文字列이나 다른
列擧型 클래스의 멤버를 直接 傳達하면 `TypeError`가 發生합니다. 文字列을
解析하려면 `Preset("ko-kp")`처럼 明示的으로 生成합니다.

| 列擧型               | 文字列 값                                                                                                                                                                    |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Preset`             | `ko-kr`, `ko-kp`                                                                                                                                                             |
| `RenderMode`         | `hangul-only`, `hangul-hanja-parens`, `hanja-hangul-parens`, `ruby-on-hangul`, `ruby-on-hanja`, `original`                                                                   |
| `OriginalGloss`      | `parens`, `ruby`                                                                                                                                                             |
| `HanjaVariantSet`    | `as-dictionary`, `shinjitai`, `kanxi`, `simplified`, `asahimoji`                                                                                                             |
| `Segmentation`       | `lattice`, `eager`                                                                                                                                                           |
| `SourcePolicy`       | `reference-authorized`, `reference-first`, `loanword-first`, `ordered`                                                                                                       |
| `SourceRole`         | `override`, `reference`, `loanword`                                                                                                                                          |
| `NumeralStrategy`    | `hangul-phonetic`, `positional-arabic`, `additive-arabic`, `smart`                                                                                                           |
| `ContextWindow`      | `off`, `per-block`, `per-section`, `per-document`                                                                                                                            |
| `HomophoneDetection` | `context-local`, `dictionary-wide`                                                                                                                                           |
| `Recovery`           | `strict`, `lenient`                                                                                                                                                          |
| `PunctuationPreset`  | `preserve`, `modern`, `traditional`, `traditional-vertical`                                                                                                                  |
| `SpaceAfter`         | `preserve`, `remove`, `single`                                                                                                                                               |
| `DictionaryFormat`   | `fst`, `cdb`                                                                                                                                                                 |
| `InputFormat`        | `text`, `html`, `markdown`                                                                                                                                                   |
| `ErrorCode`          | `dictionary-load`, `segmentation`, `invalid-reading`, `html-scan`, `html-malformed-attr`, `markdown`, `unsupported-content-type`, `invalid-input`, `io`, `internal`, `other` |


辭典 入力과 項目
----------------

`DictionaryEntry(hanja, reading, suffix_reading=None, preference=0, require_hanja=False, require_hangul=False)`는
不變 레코드입니다. 漢字와 讀音은 必須이며 비어 있으면 안 됩니다. 接尾 讀音을
指定한 境遇에도 빈 文字列은 許容하지 않습니다. `preference`는 `bool`을 除外한
0부터 4294967295까지의 整數입니다. 같은 漢字 키의 項目도 競爭하는 讀音으로
維持합니다. 基本的으로 높은 選好度가 優先하며 同率은 讀音의 辭典順으로
決定합니다. 뒤따르는 助詞에 맞는 候補를 選擇할 수도 있습니다.
`suffix_reading`은 語頭가 아닌 곳의 讀音을 指定합니다. 두 表記 플래그는 各各
漢字나 한글 註解(annotation)를 要請합니다.

`MapDictionarySource(entries, role=SourceRole.OVERRIDE)`는 項目 시퀀스를
튜플로 複寫합니다. 빈 辭典도 有效합니다. 使用者 再定義 辭典은 協力하는
參照 辭典과 外來語 辭典보다 優先합니다.

`FileDictionarySource(data, format, role=None)`는 `bytes`, `bytearray`,
`memoryview`, 文字列 經路, `os.PathLike[str]`를 받습니다. 變更 可能한
버퍼와 `memoryview`의 論理的 內容은 複寫합니다. 解除했거나 읽을 수 없는
버퍼는 `INVALID_INPUT` 誤謬를 發生시킵니다. 經路는 Python 파일 시스템 값으로
維持하므로 有效한 `surrogateescape` 經路도 使用할 수 있습니다. 파일은 變換器
生成 時 읽습니다.

`UrlDictionarySource(url, format, role=None, timeout=30.0, max_bytes=67108864, total_timeout=30.0)`는
變換器 生成 時 HTTP(S) 다운로드를 要請합니다. 리다이렉트는 最初 URL과 스킴,
호스트 이름, 有效 포트가 같아야 합니다. DNS 호스트 이름의 大小文字는 區分하지
않지만 IPv6 스코프 識別子는 正確히 比較합니다. 省略한 포트는 HTTP 80, HTTPS
443으로 比較합니다. 다른 오리진으로의 리다이렉트는 對象에 要請을 보내기 前에
`INVALID_INPUT`으로 拒否합니다. 같은 오리진의 相對/絶對 리다이렉트는
支援합니다. 다른 오리진의 辭典은 最終 URL을 直接 指定하세요. 最初 URL은 內部
서비스 住所를 包含해 呼出者가 選擇합니다. 오리진 檢査는 URL 構成 要素를
比較하며, DNS로 照會한 IP 住所는 檢査하지 않습니다. 잘못된 URL/포트, 許容하지
않은 스킴과 리다이렉트 對象은 `INVALID_INPUT`입니다. `timeout`은 各 소켓 作業의
制限 時間입니다. 陽의 整數인 `max_bytes`는 辭典 本文 크기를 基本 64 MiB로
制限합니다. 陽의 有限한 秒 單位 값인 `total_timeout`은 리다이렉트 全體에 걸쳐
HTTP 應答을 읽는 期限이며, 헤더와 청크 構文에도 適用합니다. DNS 照會는 소켓
制限 時間을 設定하기 前에 實行하므로 `timeout`과 `total_timeout`을 모두 超過할
수 있습니다. 連結 試圖에는 소켓 制限 時間을 適用하지만, 여러 住所에 對한
再試圖와 TLS 協商 때문에 `total_timeout` 以後까지 取消가 遲延될 수 있습니다.
陽數가 아닌 限度나 有限하지 않은 制限 時間은 `INVALID_INPUT`, 잘못된 引子
形式은 `TypeError`입니다. 크기나 時間 限度를 넘으면 `IO` 原因을 包含한
`DICTIONARY_LOAD` 誤謬가 發生합니다. 리다이렉트 本文은 消費하지 않고 닫습니다.
프록시 環境 變數는 使用하지 않습니다.

모든 바이너리 入力은 `DictionaryFormat.FST`나 `CDB`를 指定해야 합니다.
役割의 優先順位는 明示한 役割, 內藏된 出處 根據의 役割, `OVERRIDE`입니다.
바이너리의 出處 및 句節 根據는 該當 辭典에 連結된 狀態로 維持됩니다.

`SourcePolicy.REFERENCE_AUTHORIZED`는 參照 根據로 許容한 代替 表記를
使用합니다. `REFERENCE_FIRST`와 `LOANWORD_FIRST`는 各各 參照/外來語를
優先합니다. `ORDERED`는 登錄 順序대로 選擇하며 役割과 `reference_only`를
無視합니다. 協力 政策에서 `reference_only=True`는 獨立的인 外來語 選擇을
除外하되 參照 辭典이 許容한 表記는 維持합니다. 役割 指定만으로 語彙 識別이나
表記 許容 根據가 생기지는 않습니다.


公式 辭典
---------

`gukhanmun.dictionaries`는 選擇的 데이터 配布版에서 公式 辭典을 불러옵니다.
코어 패키지의 엑스트라로 設置합니다.
〈[辭典](../guide/python/dictionaries.md)〉을 參照하십시오.

~~~~ text
stdict(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource
opendict_general(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource
opendict_north_korean(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource
opendict_dialect(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource
opendict_archaic(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource
loanwords(*, format: DictionaryFormat = DictionaryFormat.FST) -> FileDictionarySource
~~~~

| 函數                      | 配布版 (FST/CDB)                                     | 役割        |
| ------------------------- | ---------------------------------------------------- | ----------- |
| `stdict()`                | `gukhanmun-stdict-fst`, `gukhanmun-stdict-cdb`       | `REFERENCE` |
| `opendict_general()`      | `gukhanmun-opendict-fst`, `gukhanmun-opendict-cdb`   | `REFERENCE` |
| `opendict_north_korean()` | `gukhanmun-opendict-fst`, `gukhanmun-opendict-cdb`   | `REFERENCE` |
| `opendict_dialect()`      | `gukhanmun-opendict-fst`, `gukhanmun-opendict-cdb`   | `REFERENCE` |
| `opendict_archaic()`      | `gukhanmun-opendict-fst`, `gukhanmun-opendict-cdb`   | `REFERENCE` |
| `loanwords()`             | `gukhanmun-loanwords-fst`, `gukhanmun-loanwords-cdb` | `LOANWORD`  |

各 函數는 所有權을 갖는 바이트, 要請한 形式, 위의 役割을 담은
`FileDictionarySource`를 返還합니다. `stdict_bytes()`,
`opendict_general_bytes()`, `opendict_north_korean_bytes()`,
`opendict_dialect_bytes()`, `opendict_archaic_bytes()`, `loanwords_bytes()`는
같은 키워드 引數를 받고 바이트만 返還합니다. `format`은 키워드 專用 引數이며,
`DictionaryFormat` 以外의 값을 넘기면 `TypeError`가 發生합니다.

函數는 呼出될 때 `importlib.resources`로 패키지에 包含된 파일을 읽습니다.
`gukhanmun`이나 `gukhanmun.dictionaries`를 가져오는 것만으로는 데이터
패키지를 가져오지 않습니다. gzip으로 壓縮된 파일은 壓縮을 풀므로, 返還된
바이트는 로더(loader)에서 바로 使用할 수 있습니다. 어느 파일이 壓縮되어 있는지는
配布版마다 定해져 있으며, 파일 內容으로 推測하지 않습니다.

失敗하면 `DICTIONARY_LOAD`인 `GukhanmunError`가 發生합니다. 가져오기, 읽기,
壓縮 解除 誤謬처럼 例外가 失敗의 原因이면, 그 例外는 `__cause__`가 되고
`chain`의 `IO` 項目에도 包含됩니다. 데이터 패키지를 가리는 一般 모듈에는 原因
例外가 없으므로 `chain`이 비어 있습니다. 配布版이 없으면 메시지에
`pip install 'gukhanmun[<extra>]'`가 나타납니다. 設置된 配布版을 가져올 수
없거나, 파일이 없거나 읽을 수 없거나, 파일이 *provenance.json*에 記錄된
크기 및 SHA-256 해시와 다르거나, *provenance.json*을 읽을 수 없거나, gzip으로
壓縮된 파일이 비어 있거나 損傷되었으면 `pip install --force-reinstall`을
案內합니다. `KeyboardInterrupt`처럼 `Exception`의 下位 클래스가 아닌 例外는
그대로 傳播됩니다. 記錄과 一致하지만 올바른 辭典이 아닌 內容은 다른 바이너리
出處와 마찬가지로 `Converter(...)`가 誤謬를 報告합니다.


指示와 HTML 保存
----------------

`Directives(require_hanja=(), require_hangul=(), skip_annotation=())`는
漢字 文字列을 세 튜플에 貯藏합니다. 指示는 위 順序로 登錄하며 註解 表記에
作用합니다. Python 콜백은 받지 않습니다.

`HtmlOptions(preserve_classes=(), preserve_attributes=())`는 어느 規則이든
一致하면 該當 要素와 모든 子孫을 保存합니다. 클래스는 大小文字를 區分하며
ASCII 空白으로 나눈 토큰과 比較합니다. 屬性 規則은
`AttributeMatcher(name, value=None)`입니다. `None`은 存在 與否를,
文字列은 디코딩한 값과의 正確한 一致를 檢査합니다. 屬性 이름은 ASCII
大小文字를 區分하지 않고 값은 區分합니다. 重複 屬性은 첫 番째 값으로
判定합니다. 非ASCII 文字를 包含한 이름도 全體를 比較합니다. 값의 數字
文字 參照와 `amp`, `lt`, `gt`, `quot`, `apos` 이름 參照를 디코딩합니다.


文章 符號 프로파일
------------------

`StopStyle(mark, after)`는 하나의 Unicode 스칼라와 뒤따르는 U+0020
空白의 `SpaceAfter` 政策을 指定합니다. `DelimiterStyle(open, close)`는
여는/닫는 文章 符號를 指定합니다. 變換器를 生成할 때 네이티브 코드에서
範疇와 衝突을 檢證합니다. 出力 樣式을 追加해도 入力에서 認識하는 文章
符號의 集合이 늘어나지는 않습니다.

`CustomPunctuationProfile(base=PunctuationPreset.PRESERVE, comma=UNSET, period=UNSET, single=UNSET, double=UNSET)`는
세 가지 狀態를 區分합니다.

| 값                           | 效果                            |
| ---------------------------- | ------------------------------- |
| `UNSET`                      | 該當 役割의 基本 프로파일 相續  |
| `None`                       | 該當 種類의 入力 文章 符號 保存 |
| `StopStyle`/`DelimiterStyle` | 明示한 出力 樣式 使用           |

`UNSET`은 `UnsetType`의 唯一한 멤버이며 複寫나 피클 處理 後에도 같은
客體입니다. `comma`/`period`는 `StopStyle`, `single`/`double`은
`DelimiterStyle`을 받습니다. 解析된 프로파일도 같은 네 役割의 필드를 가지며 各
값은 該當 樣式 또는 `None`입니다.


誤謬
----

`GukhanmunError(code, message, chain=())`는 `code: ErrorCode`,
`message: str`, 不變 `tuple[ErrorCause, ...]`를 提供합니다.
`ErrorCause(code, message)`는 不變 레코드입니다. 原因은 가장 깊은 根本
原因부터 直接的인 原因 順序로 羅列합니다.

| 失敗                                              | Python 結果                                             |
| ------------------------------------------------- | ------------------------------------------------------- |
| 잘못된 引數 타입, 列擧型 種類, 必須 필드 漏落     | `TypeError`                                             |
| 有效하지 않은 設定, 項目, 文章 符號, URL 값       | `INVALID_INPUT`인 `GukhanmunError`                      |
| 파일/URL 入出力 失敗                              | 根本 原因부터 羅列한 `IO` 原因을 가진 `DICTIONARY_LOAD` |
| 有效하지 않은 바이너리 辭典                       | `DICTIONARY_LOAD`                                       |
| 生成子의 텍스트 設定에 있는 有效하지 않은 Unicode | `INVALID_INPUT`                                         |
| 運營 體制가 인코딩할 수 없는 파일 經路            | `DICTIONARY_LOAD`                                       |
| 入力 本文이나 스트림의 單獨 서로게이트            | `UnicodeEncodeError`                                    |
| 豫期치 않은 네이티브 Rust 패닉                    | `INTERNAL`                                              |
| Python 인터럽트                                   | 原來 `KeyboardInterrupt` 傳達                           |

一部 코드가 이 바인딩에서 普通 發生하지 않더라도 열한 가지 값을 모두
維持합니다. 네이티브 變換의 매핑은 Node-API와 같습니다. 코어는
`SEGMENTATION`, HTML은 `HTML_SCAN`, Markdown은 `MARKDOWN`, FST/CDB는
`DICTIONARY_LOAD`, 入出力은 `IO`, 設定은 `INVALID_INPUT`, 分類하지 못한
네이티브 誤謬는 `INTERNAL`로 매핑합니다.


JavaScript 對應表
-----------------

*packages/types/index.ts*가 動作의 基準입니다. Python은 非同期 `load()`
팩토리 代身 生成子의 키워드 引數를 使用하고, 暗默的 文字列 代身 明示的인
列擧型을 使用합니다.

| JavaScript 契約                                                                                                    | Python 對應                                                     |
| ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| `load(options)`                                                                                                    | `Converter(**options)`                                          |
| `preset`, `rendering`, `segmentation`, `numerals`, `recovery`, `punctuation`, `dictionaries`, `directives`, `html` | 同一한 키워드 이름                                              |
| `originalGloss`, `hanjaVariantSet`                                                                                 | `original_gloss`, `hanja_variant_set`                           |
| `sourcePolicy`, `referenceOnly`, `initialSoundLaw`                                                                 | `source_policy`, `reference_only`, `initial_sound_law`          |
| `homophoneWindow`, `homophoneDetection`                                                                            | `homophone_window`, `homophone_detection`                       |
| `firstOccurrenceWindow`, `collapseRedundantParens`                                                                 | `first_occurrence_window`, `collapse_redundant_parens`          |
| `convert(source, format)`                                                                                          | `convert(source, format=...)`                                   |
| `Format` 文字列/Markdown 客體                                                                                      | `InputFormat`/`MarkdownOptions`                                 |
| `TransformStream` 返還값 `stream()`                                                                                | `ConversionStream` 및 `push()`/`finish()`                       |
| `options`                                                                                                          | 不變 `ResolvedOptions`                                          |
| `DictionaryEntry.suffixReading`, `requireHanja`, `requireHangul`                                                   | `suffix_reading`, `require_hanja`, `require_hangul`             |
| 파일 辭典 `data`, `format`, `role`                                                                                 | `FileDictionarySource`; 明示的 URL은 `UrlDictionarySource`      |
| 메모리 辭典 `entries`, `role`                                                                                      | `MapDictionarySource`                                           |
| `requireHanja`, `requireHangul`, `skipAnnotation` 指示                                                             | `Directives.require_hanja`, `require_hangul`, `skip_annotation` |
| `preserveClasses`, `preserveAttributes`                                                                            | `HtmlOptions.preserve_classes`, `preserve_attributes`           |
| 屬性 文字列 또는 `{name, value}`                                                                                   | `AttributeMatcher(name, value=None)`                            |
| 文章 符號 省略/`null`/樣式                                                                                         | `UNSET`/`None`/樣式 레코드                                      |
| `GukhanmunError.code`, `message`, `chain`                                                                          | 타입이 指定된 코드, 메시지와 根本 原因 優先의 不變 튜플         |

Python 便宜 메서드 `convert_iter()`는 文字列 청크의 反復 可能 客體를
버퍼링 스트림 契約에 連結합니다.

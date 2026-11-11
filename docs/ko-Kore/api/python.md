---
title: Python API
description: Gukhanmun Python 변환기의 생성자, 변경 불가능한 설정, 사전 입력, 문장 부호 프로필, 스트림 수명과 오류 코드의 사용법을 설명합니다.
---

Python API
==========

公開 타입은 `gukhanmun`에서 가져옵니다. 확장 모듈 `gukhanmun._native`는
비공개입니다. 로컬 wheel 설치와 기본 예제는
[Python 안내](../guide/python/start.md)를 참고하세요.


變換器
------

`Converter(...)`는 키워드 인수로 설정하며, 생성 후 변경할 수 없는 재사용
가능한 변환기를 만듭니다. 전달한 시퀀스는 소유권을 갖는 불변 스냅샷으로
복사됩니다. 사전 데이터와 파일은 생성 시 불러옵니다.

| 인수                        | 타입                                            | 기본값                              |
| --------------------------- | ----------------------------------------------- | ----------------------------------- |
| `preset`                    | `Preset`                                        | `Preset.KO_KR`                      |
| `rendering`                 | `RenderMode`                                    | `RenderMode.HANGUL_ONLY`            |
| `original_gloss`            | `OriginalGloss`                                 | `OriginalGloss.PARENS`              |
| `hanja_variant_set`         | `HanjaVariantSet`                               | `HanjaVariantSet.AS_DICTIONARY`     |
| `segmentation`              | `Segmentation`                                  | `Segmentation.LATTICE`              |
| `source_policy`             | `SourcePolicy`                                  | `SourcePolicy.REFERENCE_AUTHORIZED` |
| `reference_only`            | `bool`                                          | `False`                             |
| `numerals`                  | `NumeralStrategy`                               | `NumeralStrategy.HANGUL_PHONETIC`   |
| `initial_sound_law`         | `bool \| None`                                  | `None`, 프리셋 상속                 |
| `homophone_window`          | `ContextWindow \| None`                         | `None`, 프리셋 상속                 |
| `homophone_detection`       | `HomophoneDetection`                            | `HomophoneDetection.CONTEXT_LOCAL`  |
| `first_occurrence_window`   | `ContextWindow`                                 | `ContextWindow.OFF`                 |
| `collapse_redundant_parens` | `bool`                                          | `True`                              |
| `recovery`                  | `Recovery`                                      | `Recovery.STRICT`                   |
| `punctuation`               | `PunctuationPreset \| CustomPunctuationProfile` | `PunctuationPreset.PRESERVE`        |
| `dictionaries`              | 파일/URL/메모리 사전의 시퀀스                   | `()`                                |
| `directives`                | `Directives \| None`                            | `None`                              |
| `html`                      | `HtmlOptions \| None`                           | `None`                              |

`KO_KR`의 상속값은 `initial_sound_law=True`와
`homophone_window=ContextWindow.PER_BLOCK`입니다. `KO_KP`는 각각
`False`와 `ContextWindow.OFF`입니다. 어느 프리셋도 공식 사전을 자동으로
불러오지 않습니다. `original_gloss`는 `RenderMode.ORIGINAL`에만 적용하며,
한자 자형은 사전 인식 후 선택합니다.

읽기 전용 속성 `options`는 `ResolvedOptions`를 반환합니다. 필드는 위
키워드에서 `original_gloss`, `dictionaries`, `directives`, `html`을 제외한
열네 개입니다. 각 값은 지정된 enum 타입을 사용하며, `punctuation`은
`UNSET`이 남아 있지 않은 `ResolvedPunctuationProfile`입니다. 프로필과
내부 레코드도 변경할 수 없습니다.


變換 演算
---------

~~~~ text
convert(source: str, *, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> str
~~~~

`InputFormat.TEXT`는 일반 텍스트, `HTML`은 HTML 조각, `MARKDOWN`은
CommonMark를 변환합니다. `MarkdownOptions(gfm=False)`는 `MARKDOWN`과
같고, `gfm=True`는 GitHub Flavored Markdown을 활성화합니다. 인라인 마크업을
사용할 수 없는 곳에서는 루비 대신 괄호로 표기합니다.

~~~~ text
stream(*, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> ConversionStream
convert_iter(chunks: Iterable[str], *, format: InputFormat | MarkdownOptions = InputFormat.TEXT) -> Iterator[str]
~~~~

`ConversionStream.push(source: str) -> str`는 청크를 저장하고 `""`를
반환합니다. `finish() -> str`는 모은 입력을 한 번 변환합니다. 스트림은
변환기를 계속 참조하며 하나의 소비자만 사용해야 합니다. 정상 종료 후 또는
입력/종료 실패 후에는 종료 상태가 되어, 추가 호출 시 `RuntimeError`가
발생합니다.

`convert_iter()`는 호출 즉시 형식을 검증하고, 반환한 반복자를 진행할 때
청크를 읽습니다. 빈 결과는 생략하며, 상위 반복자가 정상 종료했을 때만
변환을 마칩니다. 상위 반복자의 오류나 조기 종료 시에는 일부 문서를
마무리하지 않습니다. 입력을 어느 경계에서 나누어도 스트림/반복자 결과를
합치면 전체 입력에 `convert()`를 적용한 결과와 같습니다.

하나의 변환기를 여러 스레드에서 공유할 수 있으며, 각 스트림의 문서 상태는
독립적입니다. 네이티브 작업은 GIL을 해제합니다. 대기 중인 Python 시그널은
네이티브 작업이 끝난 뒤 확인하므로 긴 호출은 `KeyboardInterrupt`를 지연할
수 있습니다.


設定 enum
---------

표준 라이브러리의 `enum.Enum` 클래스입니다. 값은 JavaScript 계약의 문자열과
같습니다. 멤버 이름은 대문자이며 하이픈을 밑줄로 바꿉니다. 예를 들어
`RenderMode.HANGUL_ONLY.value == "hangul-only"`입니다. 문자열이나 다른
enum 클래스의 멤버를 직접 전달하면 `TypeError`가 발생합니다. 문자열을
해석하려면 `Preset("ko-kp")`처럼 명시적으로 생성합니다.

| enum                 | 문자열 값                                                                                                                                                                    |
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
불변 레코드입니다. 한자와 독음은 필수이며 비어 있으면 안 됩니다. 접미 독음을
지정한 경우에도 빈 문자열은 허용하지 않습니다. `preference`는 bool을 제외한
0부터 4294967295까지의 정수입니다. 같은 한자 키의 항목도 경쟁하는 독음으로
유지합니다. 기본적으로 높은 선호도가 우선하며 동률은 독음의 사전순으로
결정합니다. 뒤따르는 조사에 맞는 후보를 선택할 수도 있습니다.
`suffix_reading`은 어두가 아닌 곳의 독음을 지정합니다. 두 표기 플래그는 각각
한자나 한글 주석을 요청합니다.

`MapDictionarySource(entries, role=SourceRole.OVERRIDE)`는 항목 시퀀스를
튜플로 복사합니다. 빈 사전도 유효합니다. 사용자 재정의 사전은 협력하는
참조 사전과 외래어 사전보다 우선합니다.

`FileDictionarySource(data, format, role=None)`는 bytes, `bytearray`,
`memoryview`, 문자열 경로, `os.PathLike[str]`를 받습니다. 변경 가능한
버퍼와 memoryview의 논리적 내용은 복사합니다. 해제했거나 읽을 수 없는
버퍼는 `INVALID_INPUT` 오류를 발생시킵니다. 경로는 Python 파일 시스템 값으로
유지하므로 유효한 surrogateescape 경로도 사용할 수 있습니다. 파일은 변환기
생성 시 읽습니다.

`UrlDictionarySource(url, format, role=None, timeout=30.0, max_bytes=67108864, total_timeout=30.0)`는
변환기 생성 시 HTTP(S) 다운로드를 요청합니다. 리다이렉트는 최초 URL과 스킴,
호스트 이름, 유효 포트가 같아야 합니다. DNS 호스트 이름의 대소문자는 구분하지
않지만 IPv6 scope 식별자는 정확히 비교합니다. 생략한 포트는 HTTP 80, HTTPS
443으로 비교합니다. 다른 origin으로의 리다이렉트는 대상에 요청을 보내기 전에
`INVALID_INPUT`으로 거부합니다. 같은 origin의 상대/절대 리다이렉트는
지원합니다. 다른 origin의 사전은 최종 URL을 직접 지정하세요. 최초 URL은 내부
서비스 주소를 포함해 호출자가 선택합니다. Origin 검사는 URL 구성 요소를
비교하며, DNS로 조회한 IP 주소는 검사하지 않습니다. 잘못된 URL/포트, 허용하지
않은 스킴과 리다이렉트 대상은 `INVALID_INPUT`입니다. `timeout`은 각 소켓 작업의
제한 시간입니다. 양의 정수인 `max_bytes`는 사전 본문 크기를 기본 64 MiB로
제한합니다. 양의 유한한 초 단위 값인 `total_timeout`은 리다이렉트 전체에 걸쳐
HTTP 응답을 읽는 기한이며, 헤더와 청크 구문에도 적용합니다. DNS 조회는 소켓
제한 시간을 설정하기 전에 실행하므로 `timeout`과 `total_timeout`을 모두 초과할
수 있습니다. 연결 시도에는 소켓 제한 시간을 적용하지만, 여러 주소에 대한
재시도와 TLS 협상 때문에 `total_timeout` 이후까지 취소가 지연될 수 있습니다.
양수가 아닌 한도나 유한하지 않은 제한 시간은 `INVALID_INPUT`, 잘못된 인자
형식은 `TypeError`입니다. 크기나 시간 한도를 넘으면 `IO` 원인을 포함한
`DICTIONARY_LOAD` 오류가 발생합니다. 리다이렉트 본문은 소비하지 않고 닫습니다.
프록시 환경 변수는 사용하지 않습니다.

모든 바이너리 입력은 `DictionaryFormat.FST`나 `CDB`를 지정해야 합니다.
역할의 우선순위는 명시한 역할, 내장된 출처 근거의 역할, `OVERRIDE`입니다.
바이너리의 출처 및 구절 근거는 해당 사전에 연결된 상태로 유지됩니다.

`SourcePolicy.REFERENCE_AUTHORIZED`는 참조 근거로 허용한 대체 표기를
사용합니다. `REFERENCE_FIRST`와 `LOANWORD_FIRST`는 각각 참조/외래어를
우선합니다. `ORDERED`는 등록 순서대로 선택하며 역할과 `reference_only`를
무시합니다. 협력 정책에서 `reference_only=True`는 독립적인 외래어 선택을
제외하되 참조 사전이 허용한 표기는 유지합니다. 역할 지정만으로 어휘 식별이나
표기 허용 근거가 생기지는 않습니다.


公式 辭典
---------

`gukhanmun.dictionaries`는 選擇的 데이터 配布版에서 公式 辭典을 불러옵니다.
코어 패키지의 extra로 設置합니다.
〈[公式 辭典](../guide/python/dictionaries.md)〉을 參照하십시오.

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
바이트는 로더에서 바로 使用할 수 있습니다. 어느 파일이 壓縮되어 있는지는
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
한자 문자열을 세 튜플에 저장합니다. 지시는 위 순서로 등록하며 주석 표기에
작용합니다. Python 콜백은 받지 않습니다.

`HtmlOptions(preserve_classes=(), preserve_attributes=())`는 어느 규칙이든
일치하면 해당 요소와 모든 자손을 보존합니다. 클래스는 대소문자를 구분하며
ASCII 공백으로 나눈 토큰과 비교합니다. 속성 규칙은
`AttributeMatcher(name, value=None)`입니다. `None`은 존재 여부를,
문자열은 디코딩한 값과의 정확한 일치를 검사합니다. 속성 이름은 ASCII
대소문자를 구분하지 않고 값은 구분합니다. 중복 속성은 첫 번째 값으로
판정합니다. 비ASCII 문자를 포함한 이름도 전체를 비교합니다. 값의 숫자
문자 참조와 `amp`, `lt`, `gt`, `quot`, `apos` 이름 참조를 디코딩합니다.


文章 符號 프로필
----------------

`StopStyle(mark, after)`는 하나의 Unicode 스칼라와 뒤따르는 U+0020
공백의 `SpaceAfter` 정책을 지정합니다. `DelimiterStyle(open, close)`는
여는/닫는 문장 부호를 지정합니다. 변환기를 생성할 때 네이티브 코드에서
범주와 충돌을 검증합니다. 출력 양식을 추가해도 입력에서 인식하는 문장
부호의 집합이 늘어나지는 않습니다.

`CustomPunctuationProfile(base=PunctuationPreset.PRESERVE, comma=UNSET, period=UNSET, single=UNSET, double=UNSET)`는
세 가지 상태를 구분합니다.

| 값                           | 효과                            |
| ---------------------------- | ------------------------------- |
| `UNSET`                      | 해당 역할의 기본 프로필 상속    |
| `None`                       | 해당 종류의 입력 문장 부호 보존 |
| `StopStyle`/`DelimiterStyle` | 명시한 출력 양식 사용           |

`UNSET`은 `UnsetType`의 유일한 멤버이며 복사나 pickle 처리 후에도 같은
객체입니다. comma/period는 `StopStyle`, single/double은 `DelimiterStyle`을
받습니다. 해석된 프로필도 같은 네 역할의 필드를 가지며 각 값은 해당 양식
또는 `None`입니다.


誤謬
----

`GukhanmunError(code, message, chain=())`는 `code: ErrorCode`,
`message: str`, 불변 `tuple[ErrorCause, ...]`를 제공합니다.
`ErrorCause(code, message)`는 불변 레코드입니다. 원인은 가장 깊은 근본
원인부터 직접적인 원인 순서로 나열합니다.

| 실패                                              | Python 결과                                             |
| ------------------------------------------------- | ------------------------------------------------------- |
| 잘못된 인수 타입, enum 종류, 필수 필드 누락       | `TypeError`                                             |
| 유효하지 않은 설정, 항목, 문장 부호, URL 값       | `INVALID_INPUT`인 `GukhanmunError`                      |
| 파일/URL 입출력 실패                              | 근본 원인부터 나열한 `IO` 원인을 가진 `DICTIONARY_LOAD` |
| 유효하지 않은 바이너리 사전                       | `DICTIONARY_LOAD`                                       |
| 생성자의 텍스트 설정에 있는 유효하지 않은 Unicode | `INVALID_INPUT`                                         |
| 운영 체제가 인코딩할 수 없는 파일 경로            | `DICTIONARY_LOAD`                                       |
| 입력 본문이나 스트림의 단독 서로게이트            | `UnicodeEncodeError`                                    |
| 예기치 않은 네이티브 Rust 패닉                    | `INTERNAL`                                              |
| Python 인터럽트                                   | 원래 `KeyboardInterrupt` 전달                           |

일부 코드가 이 바인딩에서 보통 발생하지 않더라도 열한 가지 값을 모두
유지합니다. 네이티브 변환의 매핑은 Node-API와 같습니다. 코어는
`SEGMENTATION`, HTML은 `HTML_SCAN`, Markdown은 `MARKDOWN`, FST/CDB는
`DICTIONARY_LOAD`, 입출력은 `IO`, 설정은 `INVALID_INPUT`, 분류하지 못한
네이티브 오류는 `INTERNAL`로 매핑합니다.


JavaScript 對應表
-----------------

*packages/types/index.ts*가 동작의 기준입니다. Python은 비동기 `load()`
팩토리 대신 생성자의 키워드 인수를 사용하고, 암묵적 문자열 대신 명시적인
enum을 사용합니다.

| JavaScript 계약                                                                                                    | Python 대응                                                     |
| ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| `load(options)`                                                                                                    | `Converter(**options)`                                          |
| `preset`, `rendering`, `segmentation`, `numerals`, `recovery`, `punctuation`, `dictionaries`, `directives`, `html` | 동일한 키워드 이름                                              |
| `originalGloss`, `hanjaVariantSet`                                                                                 | `original_gloss`, `hanja_variant_set`                           |
| `sourcePolicy`, `referenceOnly`, `initialSoundLaw`                                                                 | `source_policy`, `reference_only`, `initial_sound_law`          |
| `homophoneWindow`, `homophoneDetection`                                                                            | `homophone_window`, `homophone_detection`                       |
| `firstOccurrenceWindow`, `collapseRedundantParens`                                                                 | `first_occurrence_window`, `collapse_redundant_parens`          |
| `convert(source, format)`                                                                                          | `convert(source, format=...)`                                   |
| `Format` 문자열/Markdown 객체                                                                                      | `InputFormat`/`MarkdownOptions`                                 |
| `TransformStream` 반환값 `stream()`                                                                                | `ConversionStream` 및 `push()`/`finish()`                       |
| `options`                                                                                                          | 불변 `ResolvedOptions`                                          |
| `DictionaryEntry.suffixReading`, `requireHanja`, `requireHangul`                                                   | `suffix_reading`, `require_hanja`, `require_hangul`             |
| 파일 사전 `data`, `format`, `role`                                                                                 | `FileDictionarySource`; 명시적 URL은 `UrlDictionarySource`      |
| 메모리 사전 `entries`, `role`                                                                                      | `MapDictionarySource`                                           |
| `requireHanja`, `requireHangul`, `skipAnnotation` 지시                                                             | `Directives.require_hanja`, `require_hangul`, `skip_annotation` |
| `preserveClasses`, `preserveAttributes`                                                                            | `HtmlOptions.preserve_classes`, `preserve_attributes`           |
| 속성 문자열 또는 `{name, value}`                                                                                   | `AttributeMatcher(name, value=None)`                            |
| 문장 부호 생략/null/양식                                                                                           | `UNSET`/`None`/양식 레코드                                      |
| `GukhanmunError.code`, `message`, `chain`                                                                          | 타입이 지정된 코드, 메시지와 근본 원인 우선의 불변 튜플         |

Python 편의 메서드 `convert_iter()`는 문자열 청크의 반복 가능 객체를
버퍼링 스트림 계약에 연결합니다.

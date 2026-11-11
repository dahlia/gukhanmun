---
title: Python 빠른 始作
description: Gukhanmun Python 패키지를 로컬에서 빌드하고, 명시적 사전과 타입이 지정된 설정으로 일반 텍스트, HTML, Markdown을 변환하는 방법을 설명합니다.
---

Python 빠른 始作
================

`Converter(...)`는 Rust 基盤의 再使用 가능한 Python 變換器를 만듭니다.
한 번 설정한 뒤 문서마다 `convert()`를 호출합니다. Python 3.11 이상이
필요하며, 실행에 필수적인 제삼자 Python 패키지는 없습니다.


로컬 wheel 빌드와 설치
----------------------

저장소 루트에서 개발 도구를 설치하고 패키지를 빌드합니다.

~~~~ sh
mise install
mise run py-build
~~~~

wheel은 *target/python/wheels/* 아래에 생성됩니다. 실제 파일 경로를 지정하여
사용할 Python 환경에 설치합니다.

~~~~ sh
python -m pip install /path/to/gukhanmun.whl
~~~~

위 명령은 로컬에서 빌드한 파일을 설치합니다. PyPI에 배포된 버전이 없어도
실행할 수 있습니다. 저장소에서 패키지를 검사하려면 다음을 실행합니다.

~~~~ sh
mise run py-test
mise run py-typecheck
~~~~

두 작업은 저장소 밖의 환경에 설치한 wheel을 검사합니다. `py-typecheck`는
동일한 사용 예제를 mypy, Pyright, basedpyright, ty, Pyrefly로 검사합니다.


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

기본 형식은 일반 텍스트입니다. CommonMark에는 `InputFormat.MARKDOWN`을,
GitHub Flavored Markdown에는 `MarkdownOptions(gfm=True)`를 사용합니다.

~~~~ python
from gukhanmun import MarkdownOptions

result = converter.convert("# 漢字", format=MarkdownOptions(gfm=True))
~~~~

설정에는 실제 enum 멤버를 전달합니다. 설정 파일에서 읽은 문자열은
`Preset("ko-kr")`처럼 명시적으로 변환합니다. `Converter()`에
`preset="ko-kr"`를 직접 전달하면 `TypeError`가 발생합니다.

어느 프리셋도 공식 사전을 자동으로 불러오지 않습니다. 사전을 지정하지 않으면
내장된 한자 음가로 변환합니다. 單語 單位의 讀音이 必要하면 實行 中에 만든
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

`FileDictionarySource()`에는 `bytes`, `bytearray`, `memoryview`도 전달할 수
있습니다. 변경 가능한 버퍼는 복사됩니다. 파일은 변환기 생성 시 읽으며, 변환
호출 중에는 사전 입출력을 수행하지 않습니다. 문자열은 URL처럼 보여도 항상
파일 경로로 취급합니다.

원격 사전은 `UrlDictionarySource()`로 명시합니다.

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

HTTP(S) URL만 허용합니다. 리다이렉트는 최초 URL과 스킴, 호스트 이름, 유효
포트가 같아야 하며, 다른 origin은 요청을 보내기 전에 `INVALID_INPUT`으로
거부합니다. 다른 origin에서 제공하는 사전은 최종 URL을 직접 지정하세요.
`timeout`은 초 단위의 양의 유한한 소켓 작업 제한 시간입니다. `max_bytes`는 사전
본문 크기를, `total_timeout`은 리다이렉트 전체에 걸친 HTTP 응답 읽기를 제한하며
헤더와 청크 구문도 포함합니다. 동기 DNS/연결 설정 때문에 취소가 `total_timeout`
이후까지 지연될 수 있습니다. 한도를 넘으면 `IO` 원인을 포함한 `DICTIONARY_LOAD`
오류가 발생합니다. 프록시 환경 변수는 사용하지 않습니다.

메모리 항목 사전의 기본 역할은 `SourceRole.OVERRIDE`입니다. 바이너리 사전은
명시한 역할, 내장된 근거 자료의 역할, `OVERRIDE` 순서로 결정합니다. 파일
이름으로 역할을 추측하지 않습니다. 사전 협력 정책은
[API 참고 문서](../../api/python.md)를 참고하세요.


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

사용자 문장 부호 프로필에서 생략한 필드는 `UNSET`이며 기본 프로필을 상속합니다.
`None`은 해당 종류의 문장 부호를 입력대로 보존합니다. `StopStyle`이나
`DelimiterStyle`로 출력 양식을 직접 정할 수도 있습니다. 위 예제는 쉼표에 현대
양식을 적용하고 마침표는 입력대로 유지합니다.

`converter.options`는 변경할 수 없는 `ResolvedOptions` 값입니다. 문장 부호
프로필과 그 안의 양식도 변경할 수 없습니다. 다른 설정에는 새 변환기를 만듭니다.


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

초기 스트림 구현은 모든 청크를 모아 `finish()`에서만 결과를 돌려줍니다. 청크의
경계는 결과에 영향을 주지 않습니다. 스트림은 변환기를 계속 참조하며, 하나의
소비자만 사용해야 합니다. 정상 종료 후 또는 입력/종료 실패 후의 추가 호출은
`RuntimeError`를 발생시킵니다. `convert_iter()`는 상위 반복자의 오류를 그대로
전달하며, 정상적으로 모든 입력을 읽었을 때만 스트림을 종료합니다.


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

`error.chain`은 가장 깊은 근본 원인부터 나열한 변경 불가능한 튜플입니다.
인수의 타입이 틀리면 `TypeError`, 설정값이 유효하지 않으면 코드가 있는
`GukhanmunError`가 발생합니다. 입력 본문에 단독 Unicode 서로게이트가 있으면
`UnicodeEncodeError`가 발생합니다. 사전 항목, 지시, HTML 규칙, 문장 부호,
URL 등 생성자의 텍스트 설정에 유효하지 않은 Unicode가 있으면
`ErrorCode.INVALID_INPUT`인 `GukhanmunError`가 발생합니다. 운영 체제가
인코딩할 수 없는 파일 경로는 `ErrorCode.DICTIONARY_LOAD`인
`GukhanmunError`를 발생시킵니다.

같은 변환기를 여러 스레드에서 사용할 수 있습니다. 네이티브 변환은 GIL을
해제하며 문서마다 별도의 구문 분석 및 주석 상태를 사용합니다. Python 시그널은
네이티브 작업이 끝난 뒤 확인하므로, 긴 변환 중에는 `KeyboardInterrupt`가
지연될 수 있습니다. 스트림은 변경 가능한 객체이므로 각각 하나의 소비자만
사용해야 합니다.

PLAN-2.5: 혼용 표기 사전 항목 처리하기
======================================

이 단계의 목표
--------------

이 단계의 목표는 `汽車길` → `기찻길`, `祭祀날` → `제삿날`, `洗手대야` →
`세숫대야`, `火김` → `홧김`, `色깔論` → `색깔론`처럼 한글과 한자가 섞여 있는
사전 항목을 engine이 하나의 dictionary match로 처리할 수 있게 만드는 것이다.
PLAN-2에서 구현한 라티스 분할기는 연속된 hanja run 안에서는 올바르게 동작하지만,
현재 engine은 먼저 hanja run을 잘라 그 부분만 사전에 묻는다. 따라서 사전에
`汽車길` 항목이 있어도 입력 `汽車길`에서 `汽車`만 조회되고, 뒤의 `길`은 후보에
포함되지 않는다.

완료 시점에는 다음이 가능해야 한다:

 -  현재 커서에서 시작하는 텍스트 suffix를 사전에 물어 mixed-script match를 찾을
    수 있다.
 -  dictionary edge가 한자와 한글을 함께 소비할 수 있다.
 -  fallback edge는 여전히 사전이 덮지 못한 한자 글자에만 생성된다.
 -  `汽車길` 같은 항목이 단일 annotation으로 렌더링된다.
 -  한글만 있는 일반 텍스트는 여전히 변환 대상이 아니다.


왜 이 단계가 중요한가
---------------------

《표준국어대사전》에는 순수 한자어만이 아니라 고유어·한자·한글이 섞인 표기가
상당히 들어 있다. 이런 항목은 글자별 독음이나 fallback으로는 복원할 수 없다.
`汽車`만 변환하면 `기차길`이 되지만, 표제어 `汽車길`의 독음은 사이시옷이 들어간
`기찻길`이다. `火김`도 `화김`이 아니라 `홧김`이고, `洗手대야`도 `세수대야`가
아니라 `세숫대야`이다. 이런 독음과 사전 항목의 mark, 동음이의어 정보,
annotation grouping은 혼용 표기 전체에 붙어야 한다.

이 문제를 PLAN-3의 fallback phoneticizer로 넘기면 책임이 섞인다. fallback은
사전이 모르는 한자 글자를 처리하는 경로이고, 혼용 표기 항목은 사전이 이미 알고
있는 단어를 올바른 검색 범위에 올리지 못하는 문제이다. 따라서 fallback을 붙이기
전에 engine의 cursor/window 모델을 바로잡아야 한다.


주요 작업
---------

1.  engine의 변환 단위를 재정의한다.
     -  입력을 무조건 hanja run으로 먼저 자르지 않는다.
     -  한자가 나타난 위치 주변에서 dictionary lookup window를 만든다.
     -  window는 현재 text token 안에서만 잡고, 보존 scope나 verbatim token을
        넘지 않는다.
     -  window 길이는 dictionary `max_word_chars()`와 현재 text token 길이로
        제한한다.

2.  dictionary match 조건을 명확히 한다.
     -  `matches_at()`은 현재 커서에서 시작하는 텍스트 suffix를 받는다.
     -  반환된 match는 적어도 한 글자 이상의 한자를 포함해야 engine 변환 후보가
        된다.
     -  한글만으로 된 dictionary entry는 이 단계에서는 무시한다.
     -  `Match.byte_len`은 한자와 한글을 모두 포함한 UTF-8 byte length이다.

3.  라티스 edge 모델을 확장한다.
     -  dictionary edge는 한자와 한글이 섞인 slice를 소비할 수 있다.
     -  fallback edge는 현재 글자가 한자인 경우에만 한 글자를 소비한다.
     -  현재 글자가 한글이고 dictionary edge가 없으면 라티스가 그 글자를
        소비하지 않고 ordinary text로 남겨야 한다.
     -  byte boundary와 char boundary table은 기존처럼 명시적으로 유지한다.

4.  출력 생성을 조정한다.
     -  mixed-script dictionary segment는 `OutputToken::Annotated`로 방출한다.
     -  annotation의 `hanja` 필드는 설계상 “원 표기” 역할이므로 `汽車길` 전체를
        담는다. 이름 변경은 public API churn이므로 이 단계에서는 하지 않는다.
     -  renderer는 기존 annotation 렌더링 규칙을 그대로 적용한다.
     -  fallback으로 남은 한자는 PLAN-3 전까지 기존처럼 원문 `Text`로 보존한다.

5.  streaming 경계의 기본 정책을 정한다.
     -  현재 구현은 `Vec` 기반이므로 이 단계에서 streaming을 완성하지 않는다.
     -  다만 chunked engine으로 바꿀 때 필요한 invariant를 코드 주석이나 문서에
        남긴다: trailing buffer는 mixed-script dictionary key가 경계를 넘을 수
        있는 만큼 보존해야 한다.


작업 방식
---------

TDD로 진행한다. 먼저 작은 synthetic dictionary에 다음 항목을 넣는다:

~~~~ text
汽車길 -> 기찻길
祭祀날 -> 제삿날
洗手대야 -> 세숫대야
火김 -> 홧김
色깔論 -> 색깔론
汽車 -> 기차
天地 -> 천지
~~~~

그리고 현재 구현에서 `汽車길`이 `기차(汽車)길` 또는 `기차길`처럼 잘못 처리되는
것을 먼저 확인한다. `色깔論`처럼 앞뒤로 한자와 한글이 섞인 입력은
`RenderMode::HangulHanjaParens`나 `process_tokens()`의 annotation을 직접 검사해
전체가 하나의 segment로 묶이는지 확인한다.

가능하면 proptest를 붙인다. 예를 들어 한자를 하나 이상 포함한 mixed-script key를
작게 생성하고, 그 key가 dictionary에 있을 때 결과 annotation의 원 표기가 입력
slice를 빠짐없이 덮는다는 성질을 확인할 수 있다. 다만 한글/한자 조합 strategy가
테스트를 지나치게 복잡하게 만들면, 이 단계에서는 명시적 단위 테스트를 우선한다.


미리 결정하지 말아야 할 것
--------------------------

 -  한글만으로 된 사전 항목을 변환 대상으로 삼지 않는다.
 -  형태소 분석이나 조사 결합 규칙을 넣지 않는다.
 -  text token 밖으로 lookup window를 확장하지 않는다.
 -  `Annotation.hanja`의 이름을 이 단계에서 바꾸지 않는다.
 -  PLAN-3의 Unihan fallback, 두음법칙, 숫자 처리를 함께 구현하지 않는다.


확인할 점
---------

 -  `汽車길`이 하나의 dictionary annotation으로 처리되는가?
 -  `火김`이 `화김`이 아니라 `홧김`으로 처리되는가?
 -  `色깔論`이 전체 dictionary match로 선택되는가?
 -  `天地` 같은 순수 한자 전체 match가 component split보다 계속 우선되는가?
 -  `行事場所` regression이 계속 통과하는가?
 -  한글만 있는 입력이 변환되거나 annotation을 만들지 않는가?
 -  dictionary entry 순서가 명확한 winner의 결과를 바꾸지 않는가?
 -  UTF-8 byte offset과 char offset이 섞여 slice panic이 나지 않는가?


완료 기준
---------

혼용 표기 dictionary match가 라티스 후보로 올라오고, `汽車길`, `火김`,
`色깔論` 회귀 테스트가 통과하며, PLAN-2의 기존 라티스 회귀 테스트가 모두
유지되면 완료이다.
이 단계가 끝나면 PLAN-3의 fallback phoneticizer는 “사전이 덮지 못한 한자”만
다루면 된다.

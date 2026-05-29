---
title: HTML 處理
description: |-
  Gukhanmun이 HTML 入力을 다루는 方式과 恒常 건너뛰는 要素.
---

HTML 處理
=========

HTML 文書나 斷片을 變換하려면 `-f text/html`을 넘기거나 *.html*/*.htm* 擴張子를
使用합니다.  Gukhanmun은 HTML을 解析하여 텍스트 노드와 屬性 속의 漢字를
變換하고, 모든 태그와 屬性을 保存하면서 結果를 다시 HTML로 直列化합니다.


恒常 保存되는 要素
------------------

Gukhanmun은 다른 어떤 設定과도 無關하게 다음 要素 안의 內容을 결코 修正하지
않습니다:

 -  `<code>`, `<kbd>`, `<pre>`, `<samp>`: 코드와 미리 整形된 텍스트
 -  `<script>`, `<style>`: 스크립트와 스타일시트
 -  `<textarea>`: 使用者 入力 領域
 -  `translate="no"`를 가진 要素: 明示的 除外

`<ruby>` 註釋 안의 內容도 그대로 둡니다.


CSS 클래스로 追加 要素 保存
---------------------------

特定 클래스를 가진 要素 안의 變換을 건너뛰려면 `--html-preserve-class`를
使用합니다.  이 플래그는 反復할 수 있습니다:

~~~~ sh
gukhanmun -f text/html \
  --html-preserve-class math \
  --html-preserve-class no-translate \
  input.html
~~~~

그 클래스 中 하나를 가진 要素(와 그 모든 後孫)는 變更 없이 通過됩니다.


屬性으로 要素 保存
------------------

屬性 이름 하나만으로, 또는 `attribute=value` 雙으로 變換을 건너뛰려면
`--html-preserve-attr`을 使用합니다.  이 플래그는 反復할 수 있습니다:

~~~~ sh
gukhanmun -f text/html \
  --html-preserve-attr data-no-hanja \
  --html-preserve-attr lang=en \
  input.html
~~~~

첫 番째 形態는 그 屬性을 가진 任意의 要素를(값과 無關하게) 一致시킵니다.  두
番째 形態는 屬性이 주어진 값과 같은 要素만 一致시킵니다.


루비 마크업
-----------

變換을 `<ruby>` 要素로 감싸려면 `-f text/html`을
`--rendering ruby-on-hangul`이나 `--rendering ruby-on-hanja`와 結合합니다:

~~~~ sh
echo "<p>漢字</p>" | gukhanmun -f text/html --rendering ruby-on-hangul
# → <p><ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby></p>
~~~~

註釋은 `<rp>`(ruby parenthesis) 要素로 감싸집니다.  `<ruby>`를 理解하는
브라우저는 `<rp>` 內容을 숨기고 讀音을 基底 위에 쌓아 렌더링합니다; `<ruby>`를
支援하지 않는 브라우저는 括弧로 묶인 倂記를 인라인으로(`한자(漢字)`) 보이는
것으로 退化하여, 出力이 어디서나 읽을 수 있게 維持됩니다.

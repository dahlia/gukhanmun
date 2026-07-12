---
title: 렌더링 모드
description: |-
  Gukhanmun이 出力에서 漢字와 그 한글 讀音을 어떻게 表示할지 制御하는 方法.
---

렌더링 모드
===========

`--rendering`은 漢字와 그 한글 讀音이 出力에 어떻게 나타날지 制御합니다.
基本값은 `hangul-only`입니다.


使用 可能한 모드
----------------

### `hangul-only` (基本)

各 漢字語를 그 한글 讀音으로 置換합니다.  原 漢字는 버려집니다.

~~~~ sh
echo "漢字" | gukhanmun --rendering hangul-only
# → 한자
~~~~

### `hangul-hanja-parens`

한글 讀音 다음에 原 漢字를 括弧 안에 넣어 出力합니다.

~~~~ sh
echo "漢字" | gukhanmun --rendering hangul-hanja-parens
# → 한자(漢字)
~~~~

### `hanja-hangul-parens`

原 漢字 다음에 한글 讀音을 括弧 안에 넣어 出力합니다.

~~~~ sh
echo "漢字" | gukhanmun --rendering hanja-hangul-parens
# → 漢字(한자)
~~~~


異體字 集合
-----------

異體字 認識은 이 設定과 無關하게 언제나 活性化됩니다. `--hanja-variant-set`는
렌더링 모드가 漢字를 남길 때 내보낼 表記만 制御합니다.

| 값              | 政策                                   |
| --------------- | -------------------------------------- |
| `as-dictionary` | 一致한 辭典 表記를 保存합니다.         |
| `shinjitai`     | 日本 常用漢字 新字體를 使用합니다.     |
| `kanxi`         | 互換 폴딩한 傳統 字體를 優先합니다.    |
| `simplified`    | Unicode 簡化字를 使用합니다.           |
| `asahimoji`     | 新字體와 檢證한 朝日文字를 使用합니다. |

### `ruby-on-hangul`

한글 讀音을 `<ruby>` 要素로 감싸고 漢字를 그 註釋으로 답니다.  HTML이나 Markdown
出力(`-f text/html` 또는 `-f text/markdown`)이 必要합니다; 平文 텍스트에서는
括弧로 退化합니다.

~~~~ sh
echo "漢字" | gukhanmun -f text/html --rendering ruby-on-hangul
# → <ruby>한자<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>
~~~~

`<rp>` 要素는 括弧로 묶인 fallback 텍스트를 담습니다.  `<ruby>`를 支援하는
브라우저는 이를 숨기고 註釋을 基底 텍스트 위에 쌓아 올리며, `<ruby>`를 支援하지
않는 브라우저는 그 倂記를 基底 텍스트에 섞지 않고 括弧 안에(`한자(漢字)`)
表示합니다.

### `ruby-on-hanja`

原 漢字를 `<ruby>` 要素로 감싸고 한글 讀音을 그 註釋으로 답니다.

~~~~ sh
echo "漢字" | gukhanmun -f text/html --rendering ruby-on-hanja
# → <ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>
~~~~

### `original`

原 漢字를 出力에 그대로 維持하고, 同音異義語를 區別하기 爲해 必要한 곳에만
倂記를 더합니다.  倂記 樣式을 고르려면 `--original-gloss`를 使用합니다:

| `--original-gloss` | 出力                                                 |
| ------------------ | ---------------------------------------------------- |
| `parens`           | 漢字(한자)                                           |
| `ruby`             | `<ruby>漢字<rp>(</rp><rt>한자</rt><rp>)</rp></ruby>` |

~~~~ sh
echo "漢字" | gukhanmun --rendering original --original-gloss parens
# → 漢字(한자)
~~~~

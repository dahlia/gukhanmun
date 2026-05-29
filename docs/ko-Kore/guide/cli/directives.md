---
title: 指示
description: |-
  特定 文字에 對해 倂記를 強制하거나 抑制하는 漢字別 再定義.
---

指示
====

指示를 使用하면 特定 漢字에 對해 辭典의 倂記 標識를 덮어쓸 수 있습니다.  漢字가
恒常 그 讀音을 보이도록, 恒常 原 漢字를 보이도록, 또는 倂記를 全的으로
건너뛰도록 要求할 수 있습니다.


인라인 플래그
-------------

### `--require-hanja`

漢字가 出力에 恒常 나타나도록 強制합니다(그렇지 않으면 漢字가 사라지는
`hangul-only` 모드에서 該當):

~~~~ sh
gukhanmun --require-hanja 漢 --require-hanja 字 input.txt
~~~~

### `--require-hangul`

한글 讀音이 漢字 곁에 나타나도록 強制합니다(`original` 모드에서 該當):

~~~~ sh
gukhanmun --rendering original --require-hangul 東 input.txt
~~~~

### `--skip-annotation`

漢字에 對한 一切의 倂記를 抑制하여, 出力에 그대로 둡니다:

~~~~ sh
gukhanmun --skip-annotation 中 input.txt
~~~~


Glob 패턴
---------

各 指示에는 漢字 키에 對해 셸 스타일 glob을 一致시키는 `-glob` 變種이 있습니다:

~~~~ sh
gukhanmun --require-hanja-glob "東*" input.txt
gukhanmun --require-hangul-glob "北[京津]" input.txt
gukhanmun --skip-annotation-glob "中*" input.txt
~~~~


指示 파일
---------

指示가 많을 때는 `--directives`로 TSV 파일을 使用합니다:

~~~~ sh
gukhanmun --directives overrides.tsv input.txt
~~~~

이 파일은 탭으로 區分된 세 個의 列을 가집니다:

| 列        | 값                                                   |
| --------- | ---------------------------------------------------- |
| `action`  | `require-hanja`, `require-hangul`, `skip-annotation` |
| `pattern` | 一致시킬 漢字 文字列                                 |
| `kind`    | `literal` 또는 `glob`                                |

`#`로 始作하는 줄은 註釋입니다.  빈 줄은 無視됩니다.

*overrides.tsv* 例示:

~~~~ tsv
# 固有名詞의 讀音을 強制
require-hanja	東京	literal
require-hanja	北京	literal
# 가운뎃點으로 區分된 모든 이름의 倂記를 抑制
skip-annotation	*·*	glob
~~~~

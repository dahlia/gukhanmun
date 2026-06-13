---
title: 빠른 始作
description: |-
  Gukhanmun 命令줄 道具의 基本 使用法.
---

빠른 始作
=========

Gukhanmun은 漢字가 包含된 텍스트를 읽어, 漢字를 그 한글 讀音으로 바꾼 텍스트를
씁니다.


파일 變換
---------

파일 經路를 位置 引數로 넘깁니다:

~~~~ sh
gukhanmun input.txt
~~~~

出力은 基本的으로 標準 出力으로 갑니다.  대신 파일로 쓰려면 `-o`를 使用합니다:

~~~~ sh
gukhanmun -o output.txt input.txt
~~~~

파일을 제자리에서 바꾸려면, 入力과 出力에 같은 經路를 넘깁니다.  Gukhanmun은
먼저 臨時 파일에 쓴 뒤 原本을 原子的으로 置換하므로, 原本이 一部만 쓰인 狀態로
남는 일이 결코 없습니다:

~~~~ sh
gukhanmun -o document.txt document.txt
~~~~


標準 入力에서 읽기
------------------

標準 入力에서 읽으려면 파일 引數를 省略합니다:

~~~~ sh
echo "漢字를 한글로" | gukhanmun
# → 한자를 한글로
~~~~


入力 形式 指定
--------------

Gukhanmun은 파일 擴張子로부터 形式을 推論합니다:

| 擴張子             | 形式        |
| ------------------ | ----------- |
| *.txt*, *(없음)*   | 平文 텍스트 |
| *.html*, *.htm*    | HTML        |
| *.md*, *.markdown* | Markdown    |

感知된 形式은 `-f`로 덮어쓸 수 있습니다:

~~~~ sh
gukhanmun -f text/plain input.html      # 平文 텍스트로 다룸
gukhanmun -f text/html  snippet.txt     # HTML로 다룸
gukhanmun -f text/markdown article.txt  # Markdown으로 다룸
~~~~

Markdown 形式은 變種 파라미터도 받습니다:

~~~~ sh
gukhanmun -f "text/markdown; variant=GFM" post.md
~~~~


詳細 로깅
---------

標準 誤謬로 디버그 情報를 出力하려면 `-v`를 더합니다:

~~~~ sh
gukhanmun -v input.txt
~~~~

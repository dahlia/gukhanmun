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

出力은 基本的으로 標準 出力으로 갑니다.  代身 파일로 쓰려면 `-o`를 使用합니다:

~~~~ sh
gukhanmun -o output.txt input.txt
~~~~

파일을 제자리에서 바꾸려면, 入力과 出力에 같은 經路를 넘깁니다.  Gukhanmun은
먼저 臨時 파일에 쓴 뒤 原本을 原子的으로 置換하므로, 原本이 一部만 쓰인 狀態로
남는 일이 決코 없습니다:

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


셸 自動 完成 生成
-----------------

設置된 CLI 自身의 옵션 定義에서 自動 完成(shell completion) 스크립트를
生成합니다:

~~~~ sh
gukhanmun --generate-completion bash > gukhanmun.bash
~~~~

셸 이름은 `bash`, `zsh`, `fish`, `powershell`, `nushell`입니다. 生成은
스크립트만 標準 出力으로 쓴 뒤, 入力을 읽거나 辭典을 載入하지 않고 끝납니다.
파일에 貯藏하려면 셸의 出力 리다이렉션(redirection)을 使用합니다. 變換 옵션이나
入力 파일을 함께 指定하면 使用法 誤謬가 됩니다. `-o`는 變換 出力을 爲한
옵션입니다. npm이나 Python 래퍼(wrapper)가 다른 經路의 實行 파일을 呼出하더라도
스크립트는 公開 命令인 `gukhanmun`을 自動 完成합니다.

옵션, 公開 別名, 列擧型(enum) 값과 파일 引數는 變換 引數를 解析하는 것과 같은
clap 定義에서 나옵니다. PowerShell은 7 버전에서 檢證했습니다. PowerShell의 값
候補도 이 定義에서 生成합니다. 自由 텍스트 값의 自動 完成과 經路 引用 方式은
셸마다 다릅니다.

Nushell 스크립트를 불러오면 `extern` 命令이 宣言됩니다. 이때 Nushell이
`gukhanmun --help`를 自體 處理하며, `-ooutput.txt`처럼 값을 붙인 一部 짧은
옵션을 拒否합니다. `-o output.txt`처럼 띄어 씁니다. 實行 파일 自身의 도움말이나
引數 解析이 必要하면 `^gukhanmun`으로 宣言을 迂回합니다.

### 같은 버전의 自動 完成 묶음 빌드

原始 코드 체크아웃(checkout)에서 다음을 實行합니다:

~~~~ sh
mise run completions
mise run completions-test
~~~~

첫 作業은 잠긴 依存性으로 체크아웃의 호스트(host) 實行 파일을 빌드합니다.
Cargo가 交叉 컴파일(cross-compilation) 對象으로 設定되어 있어도 對象 實行 파일은
實行하지 않습니다. 두 番째 作業은 다섯 셸을 모두 要求하며, 文法과 實際 自動
完成 候補를 檢證합니다. 生成 後 한 셸만 檢査하려면
`mise run completions-test -- --shell nushell`을 實行합니다.

묶음은 *target/completions/* 디렉터리에 生成됩니다:

| 셸         | 파일              |
| ---------- | ----------------- |
| Bash       | *gukhanmun.bash*  |
| zsh        | *\_gukhanmun*     |
| fish       | *gukhanmun.fish*  |
| PowerShell | *\_gukhanmun.ps1* |
| Nushell    | *gukhanmun.nu*    |

*manifest.json* 明細에는 實行 파일 버전, 全體 原始 코드 리비전(revision), 作業
트리의 變更 與否와 各 스크립트의 SHA-256을 記錄합니다. 로컬 修正은 變更된
狀態로 記錄하므로, 리비전 欄만 같다고 깨끗한 빌드인 것은 아닙니다. 다른 묶음
디렉터리를 選擇하려면 `mise run completions --` 뒤에 `--output-dir PATH`를
넘깁니다.

安定 버전과 適格 開發 빌드는 各 플랫폼 바이너리 아카이브(archive)의
*completions/* 디렉터리에 같은 묶음을 包含합니다. CI는 產出物 버전을 設定한 뒤
生成하고, 아카이브에 넣기 前 리비전, 버전과 해시를 檢證합니다. 바이너리
패키지는 Rust 컴파일러 없이 이 파일을 使用할 수 있습니다. 實行 파일과 正確히
같은 配布 버전이나 開發 리비전의 묶음을 使用합니다. 패키지別 設置 經路와
活性化는 패키지에서 處理하며, 生成器는 셸 始作 파일을 修正하지 않습니다.

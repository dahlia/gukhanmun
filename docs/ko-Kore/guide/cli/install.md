---
title: 設置
description: |-
  Gukhanmun 命令줄 道具를 設置하는 方法.
---

設置
====

네이티브 Gukhanmun 實行 파일에는 런타임 依存性이 없습니다. npm을 通해
實行하려면 Node.js 22 以上이 必要합니다.


npm을 通해서
------------

Node.js 22 以上이 있다면, 辭典이 內藏된 變換 道具를 設置합니다:

~~~~ sh
npm install -g @gukhanmun/cli
gukhanmun --help
~~~~

全域 設置 없이 `npx @gukhanmun/cli --help`로 實行할 수도 있습니다. 이
패키지는 Node-API 바인딩이 아니라 旣存 CLI 實行 파일을 包含합니다.
macOS, Windows, Linux의 x64/arm64를 支援합니다. Linux에서는 musl 環境이나
이 實行 파일과 互換되는 glibc 環境에서 같은 靜的 musl 바이너리를 使用합니다.
Rust 컴파일러나 設置 스크립트가 必要하지 않으며, `--ignore-scripts`도
支援합니다.

更新하려면 `npm install -g @gukhanmun/cli@latest`를 使用합니다.
開發 中인 빌드를 選擇하려면 다음과 같이 `@dev`를 明示합니다:

~~~~ sh
npm install -g @gukhanmun/cli@dev
npx @gukhanmun/cli@dev --help
~~~~

플랫폼別 實行 파일이 함께 設置되도록 `--omit=optional`은 使用하지 마십시오.
實行 파일 패키지가 없다는 診斷이 나오면
`npm install -g @gukhanmun/cli --include=optional`로 다시 設置합니다.
支援하지 않는 플랫폼에서는 實行 파일을 내려받거나 컴파일하는 代身
診斷을 出力합니다.

npm 패키지는 `gukhanmun`만 設置하며, `gukhanmun-mkdict`는 設置하지
않습니다. 버전이 一致하는 다섯 셸의 自動 完成(shell completion) 스크립트
經路는 `gukhanmun --print-completion-path SHELL`로 찾을 수 있습니다.
이 옵션은 npm에서만 提供하며 네이티브 實行 파일의 도움말과는 別個입니다.
各 셸에서 불러오는 命令은 [패키지 說明 文書]를 參考하십시오.
npm과 Gukhanmun은 셸의 始作 파일을 自動으로 編輯하지 않습니다.
네이티브 [自動 完成 生成 옵션](start.md#셸-自動-完成-生成)도 그대로
使用할 수 있습니다.

設置된 開發 빌드의 *package.json*과 네이티브 버전 出力에는
`X.Y.Z-dev.N+sha`가 維持되지만, npm에 登錄되는 버전과 태그에는
`+sha` 部分이 빠집니다. `dev` 태그는 `latest`를 代替하지 않습니다.
첫 安定 버전이 나오기 前에는 버전을 指定하지 않은 設置를 開發 빌드用으로
使用할 수 없습니다. `@dev`를 明示하십시오.

[패키지 說明 文書]: https://github.com/dahlia/gukhanmun/blob/main/packages/cli/README.md


mise를 通해서
-------------

[mise]를 使用한다면, 미리 빌드된 바이너리를 命令 하나로 設置할 수 있습니다:

~~~~ sh
mise use -g aqua:dahlia/gukhanmun
~~~~

`-g` 플래그는 全域으로 設置합니다.  이를 省略하면 現在 프로젝트 디렉터리에서만
道具가 活性化됩니다.

[mise]: https://mise.jdx.dev/


Windows Package Manager(`winget`)를 通해서
------------------------------------------

Windows에서는 `winget`으로 設置할 수 있습니다:

~~~~ powershell
winget install HongMinhee.Gukhanmun
~~~~


crates.io에서
-------------

Rust 툴체인이 設置되어 있다면, crates.io에서 設置합니다:

~~~~ sh
cargo install gukhanmun-cli gukhanmun-mkdict
~~~~

이 命令은 바이너리를 컴파일하여 *~/.cargo/bin/* 디렉터리에 둡니다.
그 디렉터리가 `PATH`에 있는지 確認하십시오.


미리 빌드된 바이너리
--------------------

Linux(x86\_64, aarch64), macOS(x86\_64, aarch64), Windows(x86\_64)用으로 미리
빌드된 바이너리가 GitHub의 各 릴리스에 添附되어 있습니다:

<https://github.com/dahlia/gukhanmun/releases>

自身의 플랫폼에 맞는 아카이브를 내려받아 풀고, `gukhanmun` 바이너리를 `PATH`
上의 어딘가에 둡니다.


`gukhanmun-mkdict` 同伴 道具
----------------------------

mise 設置와 미리 빌드된 아카이브에는 [使用者 定義 辭典](/guide/cli/dictionary)을
컴파일하는 道具인 `gukhanmun-mkdict`도 包含되어 있습니다.  別途의 設置 段階는
必要하지 않습니다.


設置 確認
---------

變換 道具를 確認합니다:

~~~~ sh
gukhanmun --help
~~~~

辭典 컴파일러도 함께 設置했다면 別途로 確認합니다. npm으로 設置했다면
다음 命令은 省略하십시오:

~~~~ sh
gukhanmun-mkdict --help
~~~~

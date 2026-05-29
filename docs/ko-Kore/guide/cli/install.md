---
title: 設置
description: |-
  Gukhanmun 命令줄 道具를 設置하는 方法.
---

設置
====

Gukhanmun은 런타임 依存性이 없는 單一 바이너리로 配布됩니다.


mise를 通하여
-------------

[mise]를 使用한다면, 미리 빌드된 바이너리를 命令 하나로 設置할 수 있습니다:

~~~~ sh
mise use -g github:dahlia/gukhanmun
~~~~

`-g` 플래그는 全域으로 設置합니다.  이를 省略하면 現在 프로젝트 디렉터리에서만
道具가 活性化됩니다.

[mise]: https://mise.jdx.dev/


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

~~~~ sh
gukhanmun --help
gukhanmun-mkdict --help
~~~~

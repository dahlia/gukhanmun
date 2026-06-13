---
title: 設置
description: |-
  Gukhanmun을 Rust 依存性으로 追加하기.
---

設置
====

*Cargo.toml*에 `gukhanmun`을 追加합니다:

~~~~ sh
cargo add gukhanmun
~~~~


피처 플래그
-----------

모든 피처는 基本으로 켜져 있습니다.  컴파일 時間과 바이너리 크기를 줄이려면 必要
없는 것들을 끕니다:

| 피처       | 더해 주는 것                     | 基本 |
| ---------- | -------------------------------- | ---- |
| `html`     | HTML 斷片 變換                   | 예   |
| `markdown` | Markdown 變換                    | 예   |
| `fst`      | FST 辭典 백엔드(*.gukfst* 파일)  | 예   |
| `cdb`      | CDB 辭典 백엔드(*.gukcdb* 파일)  | 예   |
| `stdict`   | 內藏 《標準國語大辭典》(約 3 MB) | 예   |
| `opendict` | 內藏 《우리말샘》(約 8 MB)       | 예   |

內藏 辭典 없이 빌드하려면(自身의 辭典을 提供할 때 有用):

~~~~ sh
cargo add gukhanmun --no-default-features -F fst
~~~~

平文 텍스트 專用의 最小 바이너리를 빌드하려면:

~~~~ sh
cargo add gukhanmun --no-default-features -F stdict
~~~~

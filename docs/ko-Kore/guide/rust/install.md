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

아래의 모든 피처는 基本으로 켜져 있습니다. 컴파일 時間과
바이너리 크기를 줄이려면 基本 피처를 끄고 必要한 것만 選擇합니다:

| 피처        | 더해 주는 것                      | 基本 |
| ----------- | --------------------------------- | ---- |
| `html`      | HTML 斷片 變換                    | 예   |
| `markdown`  | Markdown 變換                     | 예   |
| `fst`       | FST 辭典 백엔드(*.gukfst* 파일)   | 예   |
| `cdb`       | CDB 辭典 백엔드(*.gukcdb* 파일)   | 예   |
| `stdict`    | 內藏 《標準國語大辭典》(約 3 MB)  | 예   |
| `opendict`  | 內藏 《우리말샘》(約 8 MB)        | 예   |
| `loanwords` | 內藏 NIKL 外來語 用例(`fst` 包含) | 예   |

`loanwords` 피처는 國立國語院 《外來語 表記 用例》 辭典을 提供하는
`gukhanmun-loanwords`를 包含합니다. `Preset::KoKr`는 이 辭典을 自動으로
選擇하고, `Preset::KoKp`는 基本으로 끕니다. 基本 피처를 껐다면 `loanwords`를
明示的으로 켜서 이 辭典을 使用할 수 있습니다:

~~~~ sh
cargo add gukhanmun --no-default-features -F stdict,loanwords
~~~~

內藏 辭典 없이 빌드하려면(自身의 辭典을 提供할 때 有用):

~~~~ sh
cargo add gukhanmun --no-default-features -F fst
~~~~

平文 텍스트 專用의 最小 바이너리를 빌드하려면:

~~~~ sh
cargo add gukhanmun --no-default-features -F stdict
~~~~

基本 피처를 끌 때는 빌더에서도 對應하는 프리셋 辭典을 끕니다. `fst`만
켰다면 `.no_bundled_dictionaries()`를, `stdict`만 켰다면
`.no_bundled_loanwords()`를 `.build()` 前에 呼出합니다. 피처가 꺼진 內藏
辭典을 要求하면 設定 誤謬가 發生합니다.

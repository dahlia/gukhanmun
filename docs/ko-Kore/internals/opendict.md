---
title: 《우리말샘》
description: 《우리말샘》 分類別 辭典 스냅숏의 出處, 抽出 方針, 實行 時 使用法.
---

《우리말샘》
============

`gukhanmun-opendict`는 國立國語院 《우리말샘》 JSON 全體 내려받기에서 만든
分類別 辭典 스냅숏을 內藏합니다.

原本 덤프는 이 貯藏所에 커밋하지 않습니다. 代身
*crates/gukhanmun-opendict/data/* 아래의 네 TSV 파일을 커밋된 正本으로 둡니다.
네 파일은 原本의 `senseinfo.type` 값인 一般語, 北韓語, 方言, 옛말로 나뉩니다.


스냅숏
------

| 項目           | 값                                                                 |
| -------------- | ------------------------------------------------------------------ |
| 原本 壓縮 파일 | *전체 내려받기\_우리말샘\_json\_20260603.zip*                      |
| 덤프 날짜      | 2026-06-03                                                         |
| SHA-256        | `345cfae71f3710cc483975a9af04773985aaaa4ba7ca4855a4bb93f390f63e8e` |
| 데이터 license | CC BY-SA 2.0 KR                                                    |

| 分類   | 原本 分類 | TSV 파일           | 項目 數 | SHA-256                                                            |
| ------ | --------- | ------------------ | ------: | ------------------------------------------------------------------ |
| 一般語 | 일반어    | *general.tsv*      | 350,330 | `03a744cc783797e09b0e094bf2649718792edcf7e3254a4010994814ad9a16d9` |
| 北韓語 | 북한어    | *north-korean.tsv* |  34,093 | `cdc67c66b3c5febf870a406188e9621dd3a72654b5e878de1e9fb25b40db6256` |
| 方言   | 방언      | *dialect.tsv*      |   5,714 | `24547f29e78e2b3bc9a1709294e0030940b46c862e503f2f337c2f5609a801a8` |
| 옛말   | 옛말      | *archaic.tsv*      |      16 | `0f77342de48317faf10a997b156955bf73cad442e36d150a91ed19265fea0bdc` |


再生成
------

壓縮 파일이나 壓縮을 푼 디렉터리를 놓고 다음 命令을 實行합니다:

~~~~ sh
cargo run --release -p gukhanmun-opendict --bin gukhanmun-opendict-extract -- \
  ~/Downloads/전체\ 내려받기_우리말샘_json_20260603 \
  --general-output crates/gukhanmun-opendict/data/general.tsv \
  --north-korean-output crates/gukhanmun-opendict/data/north-korean.tsv \
  --dialect-output crates/gukhanmun-opendict/data/dialect.tsv \
  --archaic-output crates/gukhanmun-opendict/data/archaic.tsv
~~~~

抽出器는 單一 JSON 파일, 公式 壓縮 파일, JSON 디렉터리를 모두
받습니다. 디렉터리 內 項目과 壓縮 파일 內 項目은 文字列 順序로 處理합니다. 各
分類 TSV는 UTF-8이고 辭典 키 順으로 決定的으로 整列됩니다.


抽出 方針
---------

`word_unit`이 `어휘`인 項目 中 `original_language_info`에서 漢字가 있는
檢索 키를 만들 수 있는 것만 包含합니다. 讀音은 `wordinfo.word`에서
오며, 同形異義語 番號, 하이픈, `^` 區分子는 제거합니다.

키 生成은 `gukhanmun-stdict`와 共有하는 로직을 씁니다. 漢字 分節은
그대로 키가 되고, 固有語 分節은 漢字 周邊에 남을 수 있습니다.
`Beijing[北京]` 같은 괄호 속 外來 漢字 表記는 保存하지만, 漢字가 없는
外來語 分節은 건너뜁니다. 單一 漢字 外來語 讀音은 Unihan 對替의
韓國 漢字音 讀音을 가리지 않도록 건너뜁니다.

重複 키는 分類 안에서만 解決합니다. 같은 分類에서 먼저 만난 讀音이
이깁니다. 그래서 一般語와 北韓語 讀音은 서로 獨立이고, 使用者는
`ChainDictionary`에서 北韓語 辭典을 다른 辭典보다 앞에 놓아 원하는
優先順位를 만들 수 있습니다.


實行 時 使用法
--------------

Rust crate는 分類別 讀入 函數를 提供합니다:

 -  `gukhanmun_opendict::general()`
 -  `gukhanmun_opendict::north_korean()`
 -  `gukhanmun_opendict::dialect()`
 -  `gukhanmun_opendict::archaic()`

Rust와 CLI에서 `Preset::KoKp`는 北韓語 辭典을 기본으로 포함합니다.
JavaScript 바인딩은 어느 프리셋에서도 辭典 데이터를 自動으로 불러오지
않습니다. 必要한 分類를 `@gukhanmun/opendict-fst`나
`@gukhanmun/opendict-cdb`에서 불러와 `load({ dictionaries: [...] })`에
넘기면 됩니다.

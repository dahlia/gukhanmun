---
title: 우리말샘
description: 우리말샘 分類別 辭典 스냅숏의 出處, 抽出 方針, runtime 使用法.
---

우리말샘
========

`gukhanmun-opendict`는 國立國語院 *우리말샘* JSON 全體 내려받기에서 만든
分類別 辭典 스냅숏을 內藏합니다.

原本 덤프는 이 貯藏所에 커밋하지 않습니다. 代身
*crates/gukhanmun-opendict/data/* 아래의 네 TSV 파일을 커밋된 source of
truth로 둡니다. 네 파일은 原本의 `senseinfo.type` 값인 一般語, 北韓語,
方言, 옛말로 나뉩니다.


스냅숏
------

| 項目           | 값                                                                 |
| -------------- | ------------------------------------------------------------------ |
| 原本 archive   | *전체 내려받기\_우리말샘\_json\_20260503.zip*                      |
| 덤프 날짜      | 2026-05-03                                                         |
| SHA-256        | `846547ca6d80e6f8858af287aababb570ec54d591d40ea68b76266c25a0742ae` |
| 데이터 license | CC BY-SA 2.0 KR                                                    |

| 分類   | 原本 label | TSV 파일           | entry 數 | SHA-256                                                            |
| ------ | ---------- | ------------------ | -------: | ------------------------------------------------------------------ |
| 一般語 | 일반어     | *general.tsv*      |  350,383 | `db42f9c3ee160abf2ebb46da7df2f761e1392d19f29af32fbcfdf190c401020e` |
| 北韓語 | 북한어     | *north-korean.tsv* |   34,093 | `cdc67c66b3c5febf870a406188e9621dd3a72654b5e878de1e9fb25b40db6256` |
| 方言   | 방언       | *dialect.tsv*      |    5,715 | `a77ec3981ff0804ac97a59de8b44c262bd964f9031c706ba501126e793f92652` |
| 옛말   | 옛말       | *archaic.tsv*      |       16 | `0f77342de48317faf10a997b156955bf73cad442e36d150a91ed19265fea0bdc` |


再生成
------

zip 파일이나 압축을 푼 shard directory를 놓고 다음 命令을 실행합니다:

~~~~ sh
cargo run --release -p gukhanmun-opendict --bin gukhanmun-opendict-extract -- \
  ~/Downloads/전체\ 내려받기_우리말샘_json_20260503 \
  --general-output crates/gukhanmun-opendict/data/general.tsv \
  --north-korean-output crates/gukhanmun-opendict/data/north-korean.tsv \
  --dialect-output crates/gukhanmun-opendict/data/dialect.tsv \
  --archaic-output crates/gukhanmun-opendict/data/archaic.tsv
~~~~

extractor는 單一 JSON 파일, 公式 zip archive, JSON shard directory를 모두
받습니다. Directory entry와 zip member는 文字列 順序로 처리합니다. 各 分類
TSV는 UTF-8이고 辭典 key 順序로 決定的으로 정렬됩니다.


抽出 方針
---------

`word_unit`이 `어휘`인 entry 중 `original_language_info`에서 hanja가 있는
lookup key를 만들 수 있는 것만 포함합니다. 읽기는 `wordinfo.word`에서
오며, 同形異義語 番號, hyphen, `^` 區分子는 제거합니다.

key 生成은 `gukhanmun-stdict`와 共有하는 logic을 씁니다. 漢字 segment는
그대로 key가 되고, 固有語 segment는 漢字 周邊에 남을 수 있습니다.
`Beijing[北京]` 같은 괄호 속 外來 漢字 表記는 보존하지만, 漢字가 없는
外來語 segment는 건너뜁니다. 單一 漢字 外來語 읽기는 unihan fallback의
Sino-Korean 읽기를 가리지 않도록 건너뜁니다.

重複 key는 分類 안에서만 解決합니다. 같은 分類에서 먼저 만난 읽기가
이깁니다. 그래서 一般語와 北韓語 읽기는 서로 獨立이고, 使用者는
`ChainDictionary`에서 北韓語 辭典을 다른 辭典보다 앞에 놓아 원하는
優先順位를 만들 수 있습니다.


Runtime 使用
------------

Rust crate는 分類別 loader를 提供합니다:

 -  `gukhanmun_opendict::general()`
 -  `gukhanmun_opendict::north_korean()`
 -  `gukhanmun_opendict::dialect()`
 -  `gukhanmun_opendict::archaic()`

Rust와 CLI에서 `Preset::KoKp`는 北韓語 辭典을 기본으로 포함합니다.
JavaScript binding은 어느 preset에서도 辭典 데이터를 自動 load하지
않습니다. 必要한 分類를 `@gukhanmun/opendict-fst`나
`@gukhanmun/opendict-cdb`에서 불러 `load({ dictionaries: [...] })`에
넘기면 됩니다.

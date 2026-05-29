---
title: 辭典
description: |-
  Gukhanmun Rust 라이브러리에서 辭典을 불러오고 結合하기.
---

辭典
====

Gukhanmun은 하나 以上의 `HanjaDictionary` 具顯에서 漢字 讀音을 찾습니다.
`gukhanmun` 크레이트는 FST와 CDB 백엔드, 그리고 內藏 《標準國語大辭典》과 함께
配布됩니다.


內藏 辭典 使用
--------------

`stdict` 피처가 켜져 있으면(基本) 內藏 辭典이 自動으로 包含됩니다.  이를 끄려면:

~~~~ rust
let converter = Builder::with_preset(Preset::KoKr)
    .no_bundled_stdict()
    .build()?;
~~~~

`no_bundled_stdict`를 呼出한 뒤 다시 明示的으로 켜려면:

~~~~ rust
builder.bundled_stdict();
~~~~


파일에서 辭典 불러오기
----------------------

`fst`나 `cdb` 피처가 必要합니다.

~~~~ rust
use gukhanmun::FstDictionary;  // 또는 CdbDictionary

let dict = FstDictionary::open("custom.gukfst")?;
let converter = Builder::with_preset(Preset::KoKr)
    .push_dictionary(dict)
    .build()?;
~~~~

`push_dictionary`로 追加한 辭典은 內藏 辭典보다 먼저 參照됩니다.  連鎖 全體에서
처음 一致한 것이 採擇됩니다.


zero-copy 靜的 辭典
-------------------

`include_bytes!`로 辭典을 바이너리에 直接 內藏하여 파일 入出力 없이 불러옵니다:

~~~~ rust
use gukhanmun::FstDictionary;

static MY_DICT: &[u8] = include_bytes!("../data/custom.gukfst");

let dict = FstDictionary::from_static_bytes(MY_DICT)?;
~~~~

`from_static_bytes`는 데이터를 複寫하지 않습니다; 靜的 슬라이스에 기댄 zero-copy
뷰를 만듭니다.


所有된 바이트에서 불러오기
--------------------------

바이트가 런타임 出處(네트워크, 데이터베이스 等)에서 올 때는 `Arc<[u8]>`로
감쌉니다:

~~~~ rust
use std::sync::Arc;
use gukhanmun::CdbDictionary;

let bytes: Vec<u8> = std::fs::read("custom.gukcdb")?;
let dict = CdbDictionary::from_bytes(Arc::from(bytes.as_slice()))?;
~~~~


여러 辭典 連結
--------------

`ChainDictionary`는 여러 辭典을 明示的 優先順位로 結合하게 해 줍니다.  連鎖에서
一致를 가진 첫 辭典이 採擇됩니다:

~~~~ rust
use gukhanmun::{ChainDictionary, FstDictionary, CdbDictionary};

let domain_dict = FstDictionary::open("legal.gukfst")?;
let names_dict = CdbDictionary::open("names.gukcdb")?;
let chain = ChainDictionary::new(vec![
    Box::new(domain_dict),
    Box::new(names_dict),
]);

let converter = Builder::with_preset(Preset::KoKr)
    .no_bundled_stdict()
    .push_boxed_dictionary(Box::new(chain))
    .build()?;
~~~~

代案으로, `push_dictionary`를 여러 番 呼出합니다; 辭典은 push된 順序대로, 內藏
辭典보다 먼저 探索됩니다.


使用者 定義 辭典 構築
---------------------

위에서 불러온 *.gukfst*와 *.gukcdb* 파일은 컴파일된 産出物이며,
`gukhanmun-mkdict` 道具로 純粹 텍스트 表에서 빌드됩니다.  辭典 出處를 作成하고
컴파일하는 方法은 CLI 案內書의
〈[使用者 定義 辭典 構築](../cli/dictionary.md#使用者-定義-辭典-構築)〉을
參照하십시오.

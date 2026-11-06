---
title: 辭典
description: |-
  Gukhanmun Rust 라이브러리에서 辭典을 불러오고 結合하기.
---

辭典
====

Gukhanmun은 하나 以上의 `HanjaDictionary` 具顯에서 漢字 讀音을 찾습니다.
`gukhanmun` 크레이트는 FST와 CDB 백엔드, 內藏 《標準國語大辭典》, 《外來語 表記
用例》, 그리고 `ko-kp` 프리셋에서 쓰는 《우리말샘》 北韓語 辭典과 함께
配布됩니다.


內藏 辭典 使用
--------------

프리셋이 고른 內藏 辭典은 自動으로 包含됩니다.  모든 內藏 辭典을 끄고 使用者
定義 辭典이나 fallback에만 依存하려면:

~~~~ rust
let converter = Builder::with_preset(Preset::KoKr)
    .no_bundled_dictionaries()
    .build()?;
~~~~

個別 內藏 辭典만 끄려면 `no_bundled_stdict()`나 `no_bundled_opendict()`를
使用합니다.  `no_bundled_stdict()`를 呼出한 뒤 다시 明示的으로 켜려면:

~~~~ rust
builder.bundled_stdict();
~~~~

北韓語 表記를 基本값으로 쓰려면 `ko-kp` 프리셋을 使用합니다.  이 프리셋은
頭音法則을 끄고 《우리말샘》 北韓語 辭典을 內藏 辭典으로 包含합니다:

~~~~ rust
let converter = Builder::with_preset(Preset::KoKp).build()?;
assert_eq!(converter.convert_text_to_string("歷史와 來日")?, "력사와 래일");
~~~~

`ko-kp`의 正書法 옵션은 維持하되 《우리말샘》 北韓語 辭典만 끄려면:

~~~~ rust
let converter = Builder::with_preset(Preset::KoKp)
    .no_bundled_opendict()
    .build()?;
~~~~


《外來語 表記 用例》
--------------------

`gukhanmun-loanwords` 크레이트는 國立國語院 《外來語 表記 用例》를 오프라인 FST
辭典으로 提供합니다. `loanwords` 피처는 基本으로 켜져 있으며, `Preset::KoKr`는
이 辭典을 自動으로 包含합니다:

~~~~ rust
use gukhanmun::{Builder, Preset};

let converter = Builder::with_preset(Preset::KoKr)
    .build()?;
assert_eq!(converter.convert_text_to_string("香港")?, "홍콩");
~~~~

`Preset::KoKp`는 이 辭典을 基本으로 끄며, `.bundled_loanwords()`로 明示的으로
켤 수 있습니다. 使用者 定義 辭典이 優先하며, 內藏 參照 辭典과 外來語 辭典은
아래의 出處 政策에 따라 協力합니다. `no_bundled_loanwords()`는 이 辭典만 끄고,
`no_bundled_dictionaries()`는 다른 內藏 辭典과 함께 끕니다.

`HanjaDictionary`로 直接 使用하려면 `gukhanmun-loanwords` 크레이트를 追加하고
`gukhanmun_loanwords::dictionary()`를 呼出합니다. 우산 크레이트의 `loanwords`
피처를 켜면 `gukhanmun::loanwords::dictionary()`로도 같은 函數를 使用할 수
있습니다.


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

`from_static_bytes`는 데이터를 複寫하지 않습니다; 靜的 슬라이스에 기댄
zero-copy 뷰를 만듭니다.


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
use gukhanmun::{ChainDictionary, FstDictionary, CdbDictionary, HanjaDictionary};

let domain_dict = FstDictionary::open("legal.gukfst")?;
let names_dict = CdbDictionary::open("names.gukcdb")?;
let chain = ChainDictionary::from_iter([
    Box::new(domain_dict) as Box<dyn HanjaDictionary>,
    Box::new(names_dict),
]);

let converter = Builder::with_preset(Preset::KoKr)
    .no_bundled_stdict()
    .push_boxed_dictionary(Box::new(chain))
    .build()?;
~~~~

代案으로, `push_dictionary`를 여러 番 呼出합니다; 辭典은 push된 順序대로, 內藏
辭典보다 먼저 探索됩니다.


《우리말샘》 分類 結合
----------------------

`opendict` 피처가 켜져 있으면 《우리말샘》의 一般語, 北韓語, 方言, 옛말 分類를
Rust에서 直接 불러올 수 있습니다.  `ko-kp` 프리셋은 北韓語 分類를 自動으로
包含하지만, 方言과 옛말은 使用者가 明示的으로 選擇해야 합니다:

~~~~ rust
use gukhanmun::{Builder, ChainDictionary, HanjaDictionary, Preset};

let chain = ChainDictionary::from_iter([
    Box::new(gukhanmun::opendict::dialect()) as Box<dyn HanjaDictionary>,
    Box::new(gukhanmun::opendict::archaic()),
]);

let converter = Builder::with_preset(Preset::KoKr)
    .push_boxed_dictionary(Box::new(chain))
    .build()?;
~~~~


使用者 定義 辭典 構築
---------------------

위에서 불러온 *.gukfst*와 *.gukcdb* 파일은 컴파일된 産出物이며,
`gukhanmun-mkdict` 道具로 平文 텍스트 表에서 빌드됩니다.  辭典 出處를 作成하고
컴파일하는 方法은 CLI 案內書의
〈[使用者 定義 辭典 構築](../cli/dictionary.md#使用者-定義-辭典-構築)〉을
參照하십시오.


出處에 따른 候補 選擇
---------------------

基本 `reference-authorized` 政策은 參照 辭典과 外來語 辭典의 候補를 別途로
保存합니다. 參照 辭典이 同義語나 形式的 轉送 關係로 許容한 表記만 優先하며,
같은 候補에 寄與한 모든 語義의 許容 範圍를 交集合으로 計算합니다. 同一한 漢字
表記나 讀音만으로 關係를 推測하지 않습니다. 根據가 없으면 參照 辭典의 讀音을
維持하므로 `東扶餘`는 `동부여`, `香港`은 `홍콩`이 됩니다.

最終 候補 풀 全體에 助詞 檢査를 한 번 適用합니다. 따라서 基本 讀音이 아닌 獨立
外來語 候補도 助詞에 맞으면 選擇할 수 있습니다. `平`은 `평`이지만 `平를`은
`다이라를`이 될 수 있습니다. 數値 優先度는 各 辭典 內部에서만 比較하며, 격자의
目的 函數는 變更하지 않습니다.

| 政策                   | 動作                                                        |
| ---------------------- | ----------------------------------------------------------- |
| `reference-authorized` | 許容된 外來語 表記, 參照 辭典의 轉送/原讀音, 獨立 外來語 順 |
| `reference-first`      | 參照 辭典의 原讀音을 獨立 外來語보다 優先                   |
| `loanword-first`       | 獨立 外來語를 參照 辭典의 原讀音보다 優先                   |
| `ordered`              | 役割과 許容 關係를 無視하고 從來의 辭典 順序를 使用         |

協力 政策에서 使用者의 `override`는 같은 表記와 消費 길이의 다른 候補를
代替하며, 더 긴 一致를 除去하지 않습니다. 參照 專用 設定은 獨立 外來語를
除外하지만, 參照 辭典이 許容한 外來語 實現은 維持합니다.

選擇한 候補의 識別子·別稱·位置形·注釋 標識을 함께 傳達합니다. 出處를 保存하는
辭典 一致는 隣接한 fallback 漢字와 別途의 注釋으로 남으므로, `ordered`와
比較하여 括弧로 묶이는 範圍가 달라질 수 있습니다.

`Builder::source_policy(SourcePolicy::ReferenceAuthorized)`로 政策을,
`Builder::reference_only(true)`로 參照 專用을 設定합니다. `push_dictionary()`와
`push_boxed_dictionary()`는 항상 使用者 override입니다. 協力 辭典은
`push_dictionary_with_role(dictionary, SourceRole::Reference)`나
`SourceRole::Loanword`로 追加합니다. 直接 作成한 `ChainDictionary`의 基本값은
`Ordered`이며, `with_source_policy()`와 `push_with_role()`로 明示的으로 協力을
啟用할 수 있습니다.

`lattice` 分節에서는 重疊된 經路가 最大 辭典 語長보다 긴 範圍에서 未確定으로
남을 수 있습니다. 스트리밍은 單語 境界나 EOF가 經路를 確定할 때까지 해당
接頭部를 버퍼링하며, 이미 確定한 接頭部는 먼저 出力할 수 있습니다.
辭典 役割은 격자의 點數 目的 函數를 變更하지 않습니다.

---
title: 變換 옵션
description: |-
  Gukhanmun이 漢字를 한글로 變換하는 方式을 制御하는 Builder 메서드.
---

變換 옵션
=========

모든 옵션은 `.build()`를 呼出하기 前에 `Builder`에 設定합니다.


프리셋
------

`Builder::with_preset(preset)`은 一貫된 基本값 集合을 構成합니다:

| 프리셋         | 辭典        | 頭音法則 | 同音異義 窓               |
| -------------- | ----------- | -------- | ------------------------- |
| `Preset::KoKr` | 內藏 stdict | `true`   | `ContextWindow::PerBlock` |
| `Preset::KoKp` | 없음        | `false`  | `ContextWindow::Off`      |

아래의 個別 옵션들은 프리셋을 덮어씁니다.


分割 戰略
---------

~~~~ rust
use gukhanmun::SegmentationStrategy;

builder.segmentation(SegmentationStrategy::Lattice);  // 基本
builder.segmentation(SegmentationStrategy::Eager);
~~~~

`Lattice`는 動的 計劃法으로 全域的으로 最適인 分割을 찾습니다.  `Eager`는
왼쪽에서 오른쪽으로 가장 긴 一致를 取하는 貪慾 方式입니다; 더 빠르지만
合成語에는 덜 正確합니다.


數詞 處理
---------

`NumeralStrategy`는 二〇一六 같은 漢字 數詞 文字를 렌더링하는 方式을 制御합니다.
漢字式 數詞는 文脈에 따라 位置 記數法이나 加算 記數法으로 數를 나타낼 수
있습니다:

| 變種               | 二〇一六年 | 十一月 | 一千二百三十四 |
| ------------------ | ---------- | ------ | -------------- |
| `HangulPhonetic`   | 이공일륙년 | 십일월 | 일천이백삼십사 |
| `PositionalArabic` | 2016년     | —      | —              |
| `AdditiveArabic`   | —          | 11월   | 1234           |
| `Smart`            | 2016년     | 11월   | 1234           |

~~~~ rust
use gukhanmun::NumeralStrategy;

builder.numerals(NumeralStrategy::HangulPhonetic);   // 基本: 이공일륙
builder.numerals(NumeralStrategy::PositionalArabic); // 2016 (年度 形)
builder.numerals(NumeralStrategy::AdditiveArabic);   // 11 (加算)
builder.numerals(NumeralStrategy::Smart);            // 文脈마다 最適을 選擇
~~~~

`Smart`는 年度 같은 네 자리 連續에는 位置 記數法을, 數量에는 加算 記數法을
고릅니다; 汎用 文書에 使用합니다.


頭音法則
--------

~~~~ rust
builder.initial_sound_law(true);   // 켜짐 (Preset::KoKr 基本)
builder.initial_sound_law(false);  // 꺼짐 (Preset::KoKp 基本)
~~~~

어떤 辭典에서도 찾지 못한 文字의 fallback 讀音에 南韓의 音韻 規則(頭音法則)을
適用합니다:

| 入力 | 法則 켜짐 (`KoKr`) | 法則 꺼짐 (`KoKp`) |
| ---- | ------------------ | ------------------ |
| 來日 | 내일               | 래일               |
| 理由 | 이유               | 리유               |
| 女子 | 여자               | 녀자               |


同音異義 區別 窓
----------------

서로 다른 漢字語가 같은 한글 讀音을 共有할 수 있습니다(例를 들어 連霸와 連敗는
둘 다 연패입니다).  `RenderMode::HangulOnly`에서, Gukhanmun은 讀者가 그것들을
區別할 수 있도록 그런 單語의 漢字를 括弧 안에 維持할 수 있습니다.
`homophone_window`는 한 讀音이 模糊하다고 看做되는 範圍를 設定합니다:

| 값                                    | 動作                             |
| ------------------------------------- | -------------------------------- |
| `ContextWindow::Off`                  | 區別 追跡 안 함                  |
| `ContextWindow::PerBlock` (KoKr 基本) | 段落·리스트·헤딩 境界에서 再設定 |
| `ContextWindow::PerSection`           | 헤딩 境界에서만 再設定           |
| `ContextWindow::PerDocument`          | 入力 全體에 걸쳐 追跡            |

~~~~ rust
use gukhanmun::ContextWindow;

builder.homophone_window(ContextWindow::Off);
builder.homophone_window(ContextWindow::PerBlock);    // KoKr의 基本
builder.homophone_window(ContextWindow::PerSection);
builder.homophone_window(ContextWindow::PerDocument);
~~~~

더 넓은 窓은 讀音이 여러 섹션에 걸쳐 反復되는, 漢字 密度가 높은 텍스트에
適合합니다.


同音異義 探知 戰略
------------------

`homophone_detection`은 窓 안에서 *어떤* 讀音을 模糊한 것으로 셀지를 選擇합니다:

| 값                                        | 動作                                                                      |
| ----------------------------------------- | ------------------------------------------------------------------------- |
| `HomophoneDetection::ContextLocal` (基本) | 窓 안에 뜻이 다른 同音異義語가 實際로 나타날 때에만 그 單語를 倂記합니다. |
| `HomophoneDetection::DictionaryWide`      | 辭典 안 어디서든 다른 漢字 形態와 共有되는 讀音도 倂記합니다.             |

~~~~ rust
use gukhanmun::HomophoneDetection;

builder.homophone_detection(HomophoneDetection::ContextLocal);    // 基本
builder.homophone_detection(HomophoneDetection::DictionaryWide);
~~~~

`ContextLocal`은 한글 專用 出力을 깔끔하게 維持합니다: 單語는 周邊 텍스트가
그것을 眞짜로 模糊하게 만들 때에만 倂記됩니다.  `DictionaryWide`는 더 넓지만,
內藏 《標準國語大辭典》 같은 큰 參照 辭典으로는 거의 모든 흔한 讀音에 어떤
同音異義語가 있으므로 大部分의 漢字語를 倂記하게 됩니다.  文脈과 無關하게 特定
單語를 恒常 倂記하려면, 대신 `DirectiveAction::RequireHanja` 指示를
使用합니다(〈[使用者 指示](./directives.md)〉 參照).


認識된 單語만 區別된다
----------------------

同音異義 區別은 辭典이 單位로 認識하는 單語에 對해 動作합니다.  自體 辭典
項目이 없는 漢字 連續은 하나의 單語로 다뤄지지 않으며, 그 fallback(非辭典)
文字는 결코 倂記되지 않습니다; 그 안에 있는, 認識되는 한 글자 項目(例컨대
`紫`)은 여전히 따로 處理됩니다. 例를 들어 `自由`와 `子游`는 둘 다 `자유`로
읽히는 內藏 項目이므로 `自由와 子游`는 `자유(自由)와 자유(子游)`로
렌더링됩니다; 그러나 `紫楡`는 自體 項目이 없으므로, 基本 context-local 戰略에서
`自由와 紫楡`는 倂記 없이 `자유와 자유`로 렌더링되는데, 이는 엔진이 `自由`와
衝突할 두 番째 `자유` 單位를 결코 보지 못하기 때문입니다.  全體 用語를
區別하려면, 그것을 [使用者 定義 辭典](./dictionary.md)에 追加하여 엔진이 그것을
하나의 單位로 다루게 합니다.


最初 出現 解除 窓
-----------------

켜져 있으면, 最初 出現 解除는 窓 안에서 漢字의 첫 出現 以後 그 漢字의 倂記를
멈춥니다.  이는 各 文字를 한 番 紹介한 뒤 自由롭게 使用하는 文書에 有用합니다;
後續 出現은 括弧 漢字 없이 純粹 한글로 남습니다.

| 값                           | 動作                              |
| ---------------------------- | --------------------------------- |
| `ContextWindow::Off` (基本)  | 결코 解除 안 함; 모든 出現을 倂記 |
| `ContextWindow::PerBlock`    | 같은 段落/블록 안에서 解除        |
| `ContextWindow::PerSection`  | 같은 섹션 안에서 解除             |
| `ContextWindow::PerDocument` | 文書 全體에 걸쳐 解除             |

~~~~ rust
builder.first_occurrence_window(ContextWindow::Off);        // 基本
builder.first_occurrence_window(ContextWindow::PerBlock);
builder.first_occurrence_window(ContextWindow::PerSection);
builder.first_occurrence_window(ContextWindow::PerDocument);
~~~~


誤謬 復舊
---------

~~~~ rust
use gukhanmun::Recovery;

builder.recovery(Recovery::Strict);   // 基本: 誤謬 時 中斷
builder.recovery(Recovery::Lenient);  // 問題가 되는 斷片을 건너뜀
~~~~

HTML 變換에서 該當합니다; 純粹 텍스트와 Markdown은 復舊 可能한 誤謬를 내지
않습니다.

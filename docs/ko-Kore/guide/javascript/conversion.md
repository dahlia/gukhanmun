---
title: 變換 옵션
description: |-
  JavaScript에서 Gukhanmun이 漢字를 한글로 變換하는 方式을 制御하는 옵션.
---

變換 옵션
=========

이들을 `GukhanmunOptions` 客體의 屬性으로 `load()`에 넘깁니다.


프리셋
------

`preset`은 미리 構成된 基本값 集合을 選擇합니다:

| 값               | 辭典                                  | 頭音法則 | 同音異義 窓   |
| ---------------- | ------------------------------------- | -------- | ------------- |
| `"ko-kr"` (基本) | 內藏 없음, stdict를 明示的으로 불러옴 | `true`   | `"per-block"` |
| `"ko-kp"`        | 없음                                  | `false`  | `"off"`       |

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ preset: "ko-kp", dictionaries: [] });
~~~~

Rust 크레이트와 달리, JavaScript 패키지는 內藏 辭典을 결코 自動으로 包含하지
않습니다; 恒常 `dictionaries`를 通해 넘깁니다.


分割 戰略
---------

`segmentation`은 Gukhanmun이 漢字 連續 안에서 單語 境界를 찾는 方式을
制御합니다:

 -  `"lattice"` (基本): 每 位置의 모든 辭典 一致를 評價하여 動的 計劃法으로
    全域的으로 最適인 分割을 選擇합니다.  特히 合成語와 模糊한 境界에서 가장
    正確합니다.
 -  `"eager"`: 왼쪽에서 오른쪽으로 가장 긴 一致를 取하는 貪慾 方式.  더 빠르지만
    合成語를 잘못 分割할 수 있습니다.

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  segmentation: "lattice",  // 基本: 最適, 動的 計劃法
  // segmentation: "eager", // 貪慾, 더 빠르지만 덜 正確
});
~~~~

正確度보다 處理量이 더 重要할 때에만 `"eager"`를 選好합니다.


數詞 處理
---------

`numerals`는 二〇一六 같은 漢字 數詞 文字를 렌더링하는 方式을 制御합니다.
漢字式 數詞는 位置를 담느냐 數量을 담느냐에 따라 數를 여러 方式으로 나타낼 수
있습니다:

| 값                         | 二〇一六年 | 十一月 | 一千二百三十四 |
| -------------------------- | ---------- | ------ | -------------- |
| `"hangul-phonetic"` (基本) | 이공일륙년 | 십일월 | 일천이백삼십사 |
| `"positional-arabic"`      | 2016년     | —      | —              |
| `"additive-arabic"`        | —          | 11월   | 1234           |
| `"smart"`                  | 2016년     | 11월   | 1234           |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ numerals: "hangul-phonetic" }); // 이공일륙 (基本)
const g = await load({ numerals: "positional-arabic"}); // 2016
const g = await load({ numerals: "additive-arabic" });  // 11 (月), 1234
const g = await load({ numerals: "smart" });            // 文脈마다 最適을 選擇
~~~~

`"smart"`는 年度 같은 네 자리 連續에는 位置 記數法을, 數量에는 加算 記數法을
고릅니다; 汎用 文書에 좋은 基本값입니다.


頭音法則
--------

頭音法則은 單語 첫머리의 特定 初聲을 바꾸는 南韓의 音韻 規則입니다.  이 規則은
어떤 辭典에서도 찾지 못한 文字의 fallback 讀音에 適用됩니다; 辭典 項目은 이미
그 올바른 讀音을 담고 있습니다.

| 入力 | `initialSoundLaw: true` (ko-kr) | `initialSoundLaw: false` (ko-kp) |
| ---- | ------------------------------- | -------------------------------- |
| 來日 | 내일                            | 래일                             |
| 理由 | 이유                            | 리유                             |
| 女子 | 여자                            | 녀자                             |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ initialSoundLaw: true });   // ko-kr의 基本
const g = await load({ initialSoundLaw: false });  // ko-kp의 基本
~~~~

北韓 正書法(`"ko-kp"` 프리셋)이나 北韓 綴字 慣行을 따르는 텍스트를 處理할 때는
이를 끕니다.


同音異義 區別 窓
----------------

서로 다른 漢字語가 같은 한글 讀音을 共有할 수 있습니다(例를 들어 連霸와 連敗는
둘 다 연패입니다).  `"hangul-only"` 렌더링 모드에서, Gukhanmun은 讀者가
그것들을 區別할 수 있도록 그런 單語의 漢字를 括弧 안에 維持할 수 있습니다.
`homophoneWindow`는 한 讀音이 模糊하다고 看做되는 範圍를 設定합니다:

| 값                             | 動作                             |
| ------------------------------ | -------------------------------- |
| `"off"`                        | 區別 追跡 안 함                  |
| `"per-block"` (`ko-kr`의 基本) | 段落·리스트·헤딩 境界에서 再設定 |
| `"per-section"`                | 헤딩 境界에서만 再設定           |
| `"per-document"`               | 入力 全體에 걸쳐 追跡            |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ homophoneWindow: "off" });
const g = await load({ homophoneWindow: "per-block" });    // ko-kr의 基本
const g = await load({ homophoneWindow: "per-section" });
const g = await load({ homophoneWindow: "per-document" });
~~~~

더 넓은 窓은 讀音이 여러 섹션에 걸쳐 反復되는, 漢字 密度가 높은 텍스트에
適合합니다.


同音異義 探知 戰略
------------------

`homophoneDetection`은 窓 안에서 *어떤* 讀音을 模糊한 것으로 셀지를 選擇합니다:

| 값                       | 動作                                                                      |
| ------------------------ | ------------------------------------------------------------------------- |
| `"context-local"` (基本) | 窓 안에 뜻이 다른 同音異義語가 實際로 나타날 때에만 그 單語를 倂記합니다. |
| `"dictionary-wide"`      | 辭典 안 어디서든 다른 漢字 形態와 共有되는 讀音도 倂記합니다.             |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ homophoneDetection: "context-local" });    // 基本
const g = await load({ homophoneDetection: "dictionary-wide" });
~~~~

`"context-local"`은 한글 專用 出力을 깔끔하게 維持합니다: 單語는 周邊 텍스트가
그것을 眞짜로 模糊하게 만들 때에만 倂記됩니다.  `"dictionary-wide"`는 더 넓지만,
《標準國語大辭典》 같은 큰 參照 辭典으로는 거의 모든 흔한 讀音에 어떤
同音異義語가 있으므로 大部分의 漢字語를 倂記하게 됩니다.  文脈과 無關하게 特定
單語를 恒常 倂記하려면, 대신 `requireHanja` 指示를
使用합니다(〈[使用者 指示](./directives.md)〉 參照).


認識된 單語만 區別된다
----------------------

同音異義 區別은 辭典이 單位로 認識하는 單語에 對해 動作합니다.  自體 辭典
項目이 없는 漢字 連續은 하나의 單語로 다뤄지지 않으며, 그 fallback(非辭典)
文字는 결코 倂記되지 않습니다; 그 안에 있는, 認識되는 한 글자 項目(例컨대
`紫`)은 여전히 따로 處理됩니다. 例를 들어, 《標準國語大辭典》이 불러와진
狀態에서 `自由`와 `子游`는 둘 다 `자유`로 읽히는 項目이므로 `自由와 子游`는
`자유(自由)와 자유(子游)`로 렌더링됩니다; 그러나 `紫楡`는 自體 項目이 없으므로,
基本 context-local 戰略에서 `自由와 紫楡`는 倂記 없이 `자유와 자유`로
렌더링되는데, 이는 엔진이 `自由`와 衝突할 두 番째 `자유` 單位를 결코 보지
못하기 때문입니다.  全體 用語를 區別하려면, 그것을
[使用者 定義 辭典](./dictionary.md)에 追加하여 엔진이 그것을 하나의 單位로
다루게 합니다.


最初 出現 解除 窓
-----------------

켜져 있으면, 最初 出現 解除는 窓 안에서 漢字의 첫 出現 以後 그 漢字의 倂記를
멈춥니다.  이는 各 文字를 한 番 紹介한 뒤 自由롭게 使用하는 文書에 有用합니다;
後續 出現은 括弧 漢字 없이 純粹 한글로 남습니다.

| 값               | 動作                              |
| ---------------- | --------------------------------- |
| `"off"` (基本)   | 결코 解除 안 함; 모든 出現을 倂記 |
| `"per-block"`    | 같은 段落/블록 안에서 解除        |
| `"per-section"`  | 같은 섹션 안에서 解除             |
| `"per-document"` | 文書 全體에 걸쳐 解除             |

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ firstOccurrenceWindow: "off" });        // 基本
const g = await load({ firstOccurrenceWindow: "per-block" });
const g = await load({ firstOccurrenceWindow: "per-section" });
const g = await load({ firstOccurrenceWindow: "per-document" });
~~~~


誤謬 復舊
---------

`recovery`는 HTML 파서가 解釋할 수 없는 마크업을 만났을 때 무슨 일이 일어날지
制御합니다.  純粹 텍스트나 Markdown 入力에는 影響이 없습니다.

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({ recovery: "strict" });   // 基本: 誤謬 時 던짐
const g = await load({ recovery: "lenient" });  // 잘못된 斷片을 건너뜀 (HTML)
~~~~

斷片이나 非標準 마크업을 담을 수 있는 外部 出處의 HTML을 處理할 때는
`"lenient"`를 使用합니다; 이는 `GukhanmunError`를 던지는 대신 問題가 되는
部分을 건너뜁니다.

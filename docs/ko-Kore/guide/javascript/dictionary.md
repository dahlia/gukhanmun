---
title: 辭典
description: |-
  Gukhanmun JavaScript 라이브러리로 辭典을 불러오고 使用하기.
---

辭典
====

`dictionaries` 옵션에 `DictionarySource` 客體의 配列을 넘깁니다.  辭典 出處는
컴파일된 産出物(`FileDictionarySource`)이거나 實行 時點에 計算한 項目 目錄
(`MapDictionarySource`)입니다. 基本 `reference-authorized` 政策은 出處 役割과
明示的 辭典 關係에 따라 候補 順位를 定합니다. `sourcePolicy: "ordered"`를
選擇하면 從來의 配列 優先順位를 使用합니다.


《標準國語大辭典》 패키지
-------------------------

`@gukhanmun/stdict-fst`와 `@gukhanmun/stdict-cdb`는 둘 다 內藏
《標準國語大辭典》을 各各 다른 바이너리 形式으로 提供합니다.

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
import { stdictCdb } from "@gukhanmun/stdict-cdb";

// FST: 勸奬; 디스크에서 더 작고, 래티스(lattice) 分割에 더 適合
const g = await load({ dictionaries: [await stdictFst()] });

// CDB: O(1) 찾기; 더 單純한 配置
const g = await load({ dictionaries: [await stdictCdb()] });
~~~~

`stdictFst()`와 `stdictCdb()`는 모두 `Promise<FileDictionarySource>`를
返還합니다.


《우리말샘》 패키지
-------------------

`@gukhanmun/opendict-fst`와 `@gukhanmun/opendict-cdb`는 《우리말샘》을 一般語,
北韓語, 方言, 옛말 分類別 辭典으로 나누어 提供합니다. JavaScript 프리셋은 이
패키지들을 自動으로 불러오지 않으므로, 必要한 分類를 明示的으로 넘겨야 합니다:

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/wasm";
import {
  opendictDialectFst,
  opendictNorthKoreanFst,
} from "@gukhanmun/opendict-fst";

const g = await load({
  preset: "ko-kp",
  dictionaries: [
    await opendictNorthKoreanFst(),
    await opendictDialectFst(),
  ],
});
~~~~

같은 分類를 CDB 形式으로 쓰려면 CDB 패키지를 使用합니다:

~~~~ ts twoslash
// @noErrors: 2451
import { load } from "@gukhanmun/napi";
import { opendictNorthKoreanCdb } from "@gukhanmun/opendict-cdb";

const g = await load({
  preset: "ko-kp",
  dictionaries: [await opendictNorthKoreanCdb()],
});
~~~~


《外來語 表記 用例》 패키지
---------------------------

`@gukhanmun/loanwords-fst`와 `@gukhanmun/loanwords-cdb`는 國立國語院 《外來語
表記 用例》 辭典을 提供합니다. `"ko-kr"`과 `"ko-kp"` 모두 明示的으로 불러와야
하며, 프리셋은 이 辭典을 自動으로 追加하지 않습니다. 使用者 定義 辭典은
`override`, stdict·opendict는 `reference`, 外來語 用例는 `loanword` 役割로
區別합니다. 公式 로더는 役割을 明示하므로 基本 政策에서는 外來語 用例와 參照
辭典의 相對的인 配列 順序에 依存하지 않습니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { loanwordsFst } from "@gukhanmun/loanwords-fst";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  preset: "ko-kr",
  dictionaries: [await loanwordsFst(), await stdictFst()],
});
g.convert("香港"); // "홍콩"
~~~~

CDB 形式을 쓰려면 `@gukhanmun/loanwords-cdb`에서 `loanwordsCdb`를 가져옵니다.
두 로더 모두 `Promise<FileDictionarySource>`를 返還하고 `@gukhanmun/wasm`과
`@gukhanmun/napi`에서 쓸 수 있습니다. 패키지를 設置하는 것만으로는 變換이
달라지지 않으며, 로더의 結果를 `dictionaries`에 넣어야 합니다.


使用者 定義 辭典
----------------

`FileDictionarySource`는 바이너리 形式과 불러올 데이터를 指定합니다:

~~~~ ts twoslash
interface FileDictionarySource {
  format: "fst" | "cdb";
  data: ArrayBuffer | ArrayBufferView | URL | string;
}
~~~~

`data` 필드는 다음을 받습니다:

| 種類                            | 位置               | 備考                                   |
| ------------------------------- | ------------------ | -------------------------------------- |
| `ArrayBuffer`/`ArrayBufferView` | 모든 環境          | 이미 메모리에 있는 바이트              |
| `URL`                           | 모든 環境          | 遠隔 또는 로컬 URL; `fetch()`로 불러옴 |
| `string`                        | Node.js, Deno, Bun | 파일시스템 經路                        |


URL에서 불러오기
----------------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  dictionaries: [{
    format: "fst",
    data: new URL("./legal.gukfst", import.meta.url),
  }],
});
~~~~

브라우저에서는 URL이 `fetch()`로 가져와집니다.  Node.js·Deno·Bun에서는
`file://` URL이 디스크에서 읽힙니다.


바이트에서 불러오기
-------------------

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const response = await fetch("/custom.gukcdb");
const buf = await response.arrayBuffer();

const g = await load({
  dictionaries: [{ format: "cdb", data: buf }],
});
~~~~


파일 經路에서 불러오기(Node.js/Deno/Bun)
----------------------------------------

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
// ---cut-before---
const g = await load({
  dictionaries: [{ format: "fst", data: "/data/domain.gukfst" }],
});
~~~~

브라우저에서 純粹 文字列 經路를 넘기면 `"io"` 코드와 함께 `GukhanmunError`를
던집니다.


여러 辭典 結合
--------------

~~~~ ts twoslash
import { load } from "@gukhanmun/napi";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const g = await load({
  dictionaries: [
    { format: "fst", data: "/data/legal.gukfst" },  // 먼저 檢査
    await stdictFst(),                               // fallback
  ],
});
~~~~


使用者 定義 辭典 構築
---------------------

위에서 불러온 *.gukfst*와 *.gukcdb* 파일은 컴파일된 産出物이며,
`gukhanmun-mkdict` 道具로 平文 텍스트 表에서 빌드됩니다.  辭典 出處를 作成하고
컴파일하는 方法은 CLI 案內書의
〈[使用者 定義 辭典 構築](../cli/dictionary.md#使用者-定義-辭典-構築)〉을
參照하십시오.  項目을 디스크에 두지 않고 實行 中에 計算한다면, 다음 節처럼 直接
넘기면 됩니다.


實行 時點에 計算한 項目
-----------------------

`MapDictionarySource`는 項目을 메모리에 담습니다.  文書를 만드는 途中에 讀音을
計算하거나, TSV 파일도 `gukhanmun-mkdict`도 쓸 수 없는 브라우저·에지 런타임에서
使用합니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";
// ---cut-before---
const converter = await load({
  dictionaries: [
    {
      entries: [
        { hanja: "高橋", reading: "다카하시" },
        { hanja: "東京", reading: "도쿄", preference: 10 },
        { hanja: "年代", reading: "연대", suffixReading: "년대" },
      ],
    },
    await stdictFst(),
  ],
});
~~~~

各 필드는 한자 키를 除外하면 Rust의 `ReadingCandidate` 타입과 같습니다:

| 필드            | 意味                                                     |
| --------------- | -------------------------------------------------------- |
| `hanja`         | 辭典 키                                                  |
| `reading`       | 語頭 讀音                                                |
| `suffixReading` | 語頭가 아닐 때 쓰는 讀音 (다를 때만)                     |
| `preference`    | 같은 키 안에서의 相對 優先 順位; 클수록 優先, 없으면 `0` |
| `requireHanja`  | 讀音과 함께 原本 漢字를 恒常 보임                        |
| `requireHangul` | 原本 漢字와 함께 한글 註를 恒常 보임                     |

項目 목록의 基本 役割은 `"override"`입니다. 辭典 協力 政策에서는 같은 表記와
消費 길이에 對해 參照·外來語 出處보다 먼저 適用하며, 여러 override가 있으면
먼저 登錄한 것이 優先합니다. `sourcePolicy: "ordered"`에서는 모든 出處가
配列 順序를 따르므로, 이 모드에서 컴파일된 辭典의 키를 덮어쓰려면 項目 목록을
그 辭典보다 앞에 둡니다.

같은 `hanja` 키를 되풀이하면 앞의 讀音을 代替하지 않고 讀音을 하나 더
追加합니다. 엔진은 한 標題語의 여러 讀音을 모두 지니고 있다가, 뒤따르는 助詞가
決定的이면 그 助詞에 맞는 讀音을 고릅니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
// ---cut-before---
const g = await load({
  dictionaries: [
    {
      entries: [
        { hanja: "東京", reading: "동경" },
        { hanja: "東京", reading: "도쿄", preference: 10 },
      ],
    },
  ],
});
g.convert("東京");   // "도쿄": 優先 順位가 높은 讀音
g.convert("東京을"); // "동경을": 助詞가 다른 讀音을 고름
g.convert("東京를"); // "도쿄를"
~~~~

文脈 없이 쓰는 基本 讀音은 `preference`가 決定합니다.  目錄 順序는 決코 基本
讀音을 고르지 않습니다.  두 讀音의 優先 順位가 같으면 讀音 綴字 順序로
決定되므로, 選擇이 重要하다면 `preference`를 서로 다르게 指定하십시오.

빈 `entries` 目錄은 아무것도 寄與하지 않으므로, 덮어쓸 項目이 없는 文書에는
`{ entries: [] }`를 그대로 둘 수 있습니다.


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

`load({ sourcePolicy: "reference-authorized", referenceOnly: true, dictionaries })`로
政策과 參照 專用을 設定합니다. 파일·entry descriptor 모두
`role: "override" | "reference" | "loanword"`를 받을 수 있습니다. entry의
基本값은 `override`이며, 파일은 바이너리의 役割 메타데이터가 있으면 使用하고
없으면 `override`입니다. 明示的 `role`은 메타데이터보다 優先합니다. 公式 로더는
役割을 明示합니다. 協力 政策에서 override는 配列 位置와 無關하게 協力 辭典보다
優先하고, override끼리는 앞의 項目이 優先합니다. `sourcePolicy: "ordered"`는
모든 descriptor의 從來 配列 順序를 復元합니다.

《標準國語大辭典》 CDB 로더는 *stdict.cdb.gz*를 내려받아 解凍합니다. 使用者
URL에는 gzip이나 raw CDB를 提供할 수 있으며, npm의 raw *stdict.cdb* export는
維持합니다.

`lattice` 分節에서는 重疊된 經路가 最大 辭典 語長보다 긴 範圍에서 未確定으로
남을 수 있습니다. 스트리밍은 單語 境界나 EOF가 經路를 確定할 때까지 해당
接頭部를 버퍼링하며, 이미 確定한 接頭部는 먼저 出力할 수 있습니다.
辭典 役割은 격자의 點數 目的 函數를 變更하지 않습니다.

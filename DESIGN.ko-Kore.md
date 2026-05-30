---
title: 設計
---

Gukhanmun 設計 文書
===================

Gukhanmun은 國漢文混用體로 쓰인 韓國語 텍스트를 한글 專用 텍스트로 變換하는
라이브러리이다. [Seonbi]의 後繼 프로젝트로서, 漢字 變換 파이프라인 하나에
集中하면서도 다음과 같은 軸들로 擴張되었다: 스트리밍 入出力, 結合 可能한 辭典,
라티스 基盤 分割, 더 多樣한 出力 形式. 프로젝트는 Rust로 具顯되었고 Rust
라이브러리·命令줄 道具·WebAssembly 바인딩·[Node-API] 바인딩의 形態로 提供된다.

[Seonbi]: https://github.com/dahlia/seonbi
[Node-API]: https://nodejs.org/api/n-api.html


目標와 範圍
-----------

이 라이브러리는 韓國語 텍스트의 漢字語를 한글 讀音으로 變換한다. 必要에 따라
同音異義 區別·루비 마크업·文體上 理由 等을 爲하여 原 漢字를 倂記할 수 있다. 變換
對象 텍스트 바깥의 構造나 內容은 손대지 않으며, 韓國語가 아닌 區域이나 保存
對象으로 標識된 區域(코드 블록 等)은 그대로 通過시킨다. 入出力은 可能한 範圍
內에서 스트리밍으로 處理하며, 버퍼링은 漢字를 包含하는 連續된 變換 候補 範圍의
크기와 選擇的 文脈間 同音異義 區別에 局限된다. 메모리 內 데이터·mmap 基盤·또는
엔진에 不透明한 其他 形態의 結合 可能한 辭典을 받아들인다. 大韓民國
國立國語院의 《標準國語大辭典》에서 抽出한 基本 辭典을 함께 提供한다. Rust에서,
Node-API를 通한 Node.js에서, WebAssembly를 通한 브라우저와 Deno에서, 그리고
命令줄에서 使用 可能하다.

이 라이브러리는 스마트 따옴標·줄標·줄임標·引用 標識 같은 더 廣範圍한 韓國語
組版 補正은 意圖的으로 提供하지 않는다; 그것들은 Seonbi의 役割이다. HTML5 完全
適合 파서를 提供하지 않는다; HTML 스캐너는 段落 中心이고 가벼운 誤謬는
復舊하지만 html5ever의 代替物은 아니다. Markdown을 바이트 單位로 保存하지
않는다; 意味 保存은 契約이지만 原形 保存은 最善의 努力일 뿐이다. 言語 間
飜譯이나 漢字에서 한글로의 寫像을 넘어서는 表記 變換은 하지 않는다.


設計 原則
---------

이 設計를 貫通하는 原則은 셋이다.

엔진은 **形式 中立的**이다. 같은 엔진이 HTML·Markdown·純粹 텍스트를 모두 處理할
수 있는 것은, 各 形式이 同一한 中間 表現으로 解析되어 들어왔다가 다시 그
形式으로 돌아 나가기 때문이다. 形式에 따른 細部 事項, 卽 스캐닝·直列化·該當
形式에서 「保存 對象 區域」이나 「段落 境界」가 무엇인지에 對한 定義는 모두
境界面의 어댑터가 處理한다. 엔진은 HTML 태그名이나 pulldown-cmark 이벤트의
種類, 또는 그 밖에 形式에 結合될 만한 어떤 것도 들여다보지 않는다. 이 原則
덕분에 한 番 만든 테스트 픽스처가 세 形式 모두에서 意味를 가지게 되고, 새
形式을 追加하는 作業이 局所化된다.

**責任은 獨立的으로 交替 可能한 파이프라인 段階들로 分擔**된다.
**리더**(reader)는 入力 形式을 IR로 解析한다. **엔진**(engine)은 漢字를 包含한
語彙 範圍를 찾아 分割하고 **註解**(annotation)를 만든다.
**미들웨어**(middleware)들은 註解의 플래그를 調整해 IR 흐름을 加工한다.
**렌더러**(renderer)는 註解를 具體的 텍스트나 마크업으로 풀어낸다.
**라이터**(writer)는 IR을 目標 形式으로 直列化한다. 어느 한 段階를 交替해도
다른 段階들은 影響을 받지 않으며, 段階 사이의 境界는 메서드 呼出이 아닌
明示的인 데이터 흐름이다.

**正確性이 許容하는 곳에서는 스트리밍이 基本이다.** 本質的으로 先讀이 必要한
動作은 該當 文脈 境界까지 버퍼링한다. HTML과 Markdown에서는 基本 per-block
同音異義 窓이 普通 段落·리스트 項目·헤딩 같은 스코프에서 境界를 만난다. 反面
純粹 텍스트에는 블록 스코프가 없으므로 같은 基本 窓이 文書 全體 窓이 된다:
나중 줄의 註解가 앞 줄의 註解에 漢字 倂記를 要求할 수 있고, 바이트 스트림은 이미
stdout에 쓴 텍스트를 되돌려 고칠 수 없다. 純粹 텍스트에서 즉시 出力이 必要한
呼出者는 同音異義 標識을 끄고, 그에 따른 區別 情報 損失을 받아들여야 한다.

스트리밍 變換은 fallback 註解 範圍도 正確하게 維持한다. 꼬리에 남은 fallback
漢字 連續 範圍는 辭典의 最大 單語 길이보다 길더라도, 後續 非變換 境界나 EOF를
만날 때까지 保留한다. 이 때문에 fallback 漢字만 이어지는 入力은 辭典 先讀만 놓고
보면 必要한 것보다 덜 卽時的일 수 있지만, `hangul-hanja-parens` 같은 렌더링
모드가 one-shot 變換과 같은 註解 묶음을 보게 된다.


構造
----

파이프라인은 다섯 段階로 構成된다.

~~~~ mermaid
flowchart LR
    Bytes -->|Reader| InTok[InputToken stream]
    InTok -->|Engine| OutTok[OutputToken stream]
    OutTok -->|Middlewares| OutTok2[OutputToken stream]
    OutTok2 -->|Renderer| OutTok3[OutputToken stream]
    OutTok3 -->|Writer| Bytes2[Bytes]
~~~~

리더는 바이트를 入力 토큰의 흐름으로 解析한다. 엔진은 入力 토큰을 읽고 出力
토큰을 내놓는데, 出力 토큰에는 「原 漢字」와 「한글 讀音」을 모두 들고 있는
註解 토큰이 包含될 수 있다는 點이 다르다. 미들웨어들은 出力 흐름을 巡廻하면서
註解의 플래그들을 調整한다. 렌더러는 各 註解를 設定된 모드와 플래그에 따라
具體的 텍스트나 마크업으로 풀어낸다. 라이터는 最終 흐름을 目標 形式으로
直列化한다.

### 中間 表現

中間 表現은 平面的인 토큰 흐름이며, **스코프 데이터** 타입에 依해
파라미터化되어 있다. 이 타입은 엔진이 아니라 어댑터에 屬한다. 엔진은 토큰의
形態만 알 뿐, 스코프가 들고 다니는 페이로드(HTML의 날(raw) 屬性 文字列,
pulldown-cmark 이벤트의 變種, 等)의 內容에 對하여는 아무것도 모른다.

엔진에 들어가는 토큰은 다음 中 하나이다:

 -  `Open(Scope<S>)`: HTML 要素나 Markdown 블록 같은 構造的 스코프 進入.
    `Scope<S>`는 어댑터의 不透明 데이터와 함께 아래에서 說明할 세 가지
    미리-計算된 플래그를 들고 다닌다.
 -  `Close`: 가장 最近의 스코프를 닫음. 엔진이 스택을 自體 維持하므로, 어댑터는
    어떤 스코프가 닫히는지 다시 알려줄 必要가 없다.
 -  `Text(Cow<str>)`: 엔진이 變換할 수 있는 텍스트 조각.
 -  `Verbatim(Cow<str>)`: HTML의 `<code>` 內容이나 Markdown 코드 스팬처럼,
    그대로 通過되어야 하는 텍스트. 무엇이 verbatim인지는 엔진이 아니라 어댑터가
    決定한다.

엔진에서 나오는 토큰은 다음 中 하나이다:

 -  `Open(Scope<S>)`, `Close`, `Text(Cow<str>)`, `Verbatim(Cow<str>)`: 위와
    같은 形態로 그대로 通過.
 -  `Annotated(Annotation)`: 엔진이 한 漢字語를 變換한 자리. 註解는 原 漢字,
    한글 讀音, 그리고 이 變換이 왜 일어났고 後續 段階가 그것을 어떻게 다룰지를
    描寫하는 플래그를 들고 있다.

`Annotation`은 政策 플래그들을 들고 있다. `homophone`은 有效 辭典 項目 集合이나
現在 文脈이 같은 한글 讀音을 가진 다른 漢字 單語가 있음을 알려줄 때 設定되며,
이 境遇 出力을 읽는 사람이 한글만으로는 元 漢字를 復元할 수 없다.
`require_hanja`는 出處 辭典이나 使用者 指示에 依해 原 漢字를 한글 옆에 함께
表示할 것이 要求되는 境遇에 設定된다. `require_hangul`은 그 反對 方向이다;
出力에서는 漢字를 그대로 維持하지만 한글 倂記가 必要한 境遇에 設定되며, 國漢文
表記를 그대로 두고 어려운 漢字만 補助 表記하는 **Original** 렌더링 모드에서
使用된다. `skip_annotation`은 렌더러가 註解를 붙이지 않고 主 表記의 純粹
텍스트만 내보내길 원하는 使用者 指示에 依해 設定된다. `first_in_context`는
設定된 文脈 窓 內에서 該當 漢字語의 첫 番째 登場일 때 設定되며, 文脈은 設定에
따라 블록·섹션·文書 中 하나이다. `from_dictionary`는 辭典 一致인지 글字 單位
폴백인지를 區分한다. 디버깅 用途로 렌더러가 이 둘을 다르게 標識하도록 選擇할 수
있다.

`InputToken`과 `OutputToken`을 別個의 타입으로 둔 것은 이 IR에서 가장 큰 形態
上 決定이다. 우리가 檢討했던 代案은 `Annotated` 變種을 처음부터 包含하는 單一
`Token` 列擧型이었다. 어댑터가 `Annotated`를 만들 수 있는 可能性을 열어 두자는
趣旨였다. 이 案을 두 가지 理由로 棄却했다. 첫째, 入力 側의 `Annotated` 變種은
리더와 엔진 全 區間에서 「絶對 일어나지 않는」 變種이 되어, 코드베이스의 모든
`match` 文에 到達 不可能한 가지를 남기게 된다. 둘째, 「엔진이 註解를
**生成**하고 렌더러가 **消費**한다」는 契約을 흐리게 만든다. 두 타입을 分離하면
데이터 흐름이 한눈에 들어오고, 「未처리 註解가 라이터까지 흘러갈 수 없다」는
不變式을 타입 시스템에 맡길 수 있다.

### 스코프 데이터

不透明 스코프 페이로드는 작은 트레이트를 따른다:

~~~~ rust
pub trait ScopeData: Clone + 'static {
    /// 이 스코프 內의 텍스트는 그대로 두어야 하는가?
    fn is_preserve(&self) -> bool;

    /// 렌더러가 이 스코프 內에 `<ruby>` 같은 인라인 마크업을 揷入해도 되는가?
    fn allows_inline_markup(&self) -> bool { true }

    /// 이 스코프가 per-block 미들웨어를 리셋시키는 境界인가?
    fn is_block_boundary(&self) -> bool { false }
}
~~~~

엔진은 現在 스코프의 `is_preserve()`를 「이 `Text` 토큰을 건너뛸 것인가」에
對한 唯一한 判斷 根據로 다룬다. 相續이 必要한 어댑터는 各 열린 스코프에 이미
有效한 答을 담아 보낸다. HTML 어댑터의 `ScopeData` 具顯은 여러 關心事를 그 한
플래그에 모은다: 相續된 `lang` 屬性(韓國語 述語와 比較됨), 現在 태그와 保存
對象 祖上(保存 對象 태그 目錄인 `pre`, `code`, `kbd`, `script`, `style`,
`textarea`와 比較됨), 그리고 使用者가 날 屬性에 對하여 提供한 任意의 述語. 엔진
自體는 `lang` 屬性이 무엇인지조차 모른다.

이는 Seonbi와 다른 點이다. Seonbi는 `LangHtmlEntity`라는 別途의 註解를
파이프라인에 함께 흘려보낸다. 우리는 그 接近을 棄却했는데, 엔진이 두 信號를
따로따로 必要로 하는 境遇가 없기 때문이다; 엔진이 묻는 質問은 언제나 「이
텍스트를 건너뛰는가」 하나뿐이다. `lang` 相續을 (이미 태그名 保存 目錄이 살고
있는) 어댑터 內部로 밀어 넣음으로써, 엔진은 HTML에 對하여 無知할 수 있고 IR에는
엔진이 消費하지 않는 필드가 남지 않는다.


엔진
----

엔진의 任務는 셋이다: 漢字를 包含하는 變換 可能 語彙 範圍를 찾는 일, 그 範圍를
辭典 邊과 폴백 邊으로 分割하는 일, 그리고 그에 따라 註解와 폴백 텍스트를
내놓는 일. 大部分의 範圍는 連續된 漢字 列이지만, 辭典 項目은 `汽車길`이나
`色깔論`처럼 漢字 앞뒤에 固有語·한글 조각을 包含할 수 있다. 또한 漢字 한 글字를
한글로 寫像하고 必要하면 頭音法則을 適用하는 폴백 音譯機와, 漢字 數字를 設定에
따라 한글이나 아라비아 數字로 變換하는 數字 變換器를 함께 가지고 있다.

### 라티스 分割

漢字를 包含하는 範圍를 辭典 單語와 폴백 조각으로 分割하는 일은 엔진의 核心
決定이고, 가장 自然스러워 보이는 算法이 틀린 答을 낸다.

자연스러워 보이는 算法은 **左→右 最長 一致**(eager longest-match)이다: 왼쪽에서
오른쪽으로 走査하며 各 位置에서 그 자리로 始作하는 가장 긴 辭典 項目을 取한 뒤,
그 끝에서 다시 始作한다. Seonbi가 取한 方式이기도 하다. 이 算法의 失敗 모드는
더 긴 接頭辭가 더 自然스러운 分割에 屬해야 할 글字를 집어삼키는 境遇이다. 例를
들어 辭典에 `行事`, `行事場`, `場所`, `入口`가 들어 있다고 하자.
`行事場入口`라는 入力에 對하여 左→右 最長 一致는 `行事場` + `入口`로 分割하는데,
이는 옳다. 그러나 `行事場所`에 對하여 같은 算法은 `行事場` + `所`로 分割하고,
`所`는 글字 單位 폴백으로 넘어간다. `行事` + `場所`로 갈라야 두 部分 모두
辭典으로 덮을 수 있는데도 그렇다.

範圍는 純粹한 漢字만으로 限定되지 않는다. 《標準國語大辭典》에는 `汽車길`이
「기찻길」로, `祭祀날`이 「제삿날」로, `洗手대야`가 「세숫대야」로, `火김`이
「홧김」으로, `色깔論`이 「색깔론」으로 收錄되는 것처럼 固有語와 漢字가 섞인
項目도 있다. 따라서 辭典 邊은 現在 텍스트 커서에서 始作하는 混用 表記 接頭辭를
消費할 수 있어야 한다. 反面 폴백 邊은 辭典 邊이 덮지 못한 漢字 글字에 對하여만
作成되며, 辭典 一致의 一部가 아닌 한글은 普通 텍스트로 通過한다.

올바른 算法은 라티스 위에서 動的 計劃法을 돌리는 것이다. 變換 候補 範圍의 各
글字 位置 `i`에서, 엔진은 辭典에 그 자리로 始作하는 모든 一致를 묻고, 現在
글字가 漢字일 때에는 補助로 글字 한 個짜리 폴백 邊을 함께 考慮한다. 評價函數가
代案들을 比較하고, 우리는 範圍의 끝에서 비터비式 逆追跡으로 最適 分割을
選擇한다.

評價函數는 意圖的으로 單純하다. 먼저 辭典 一致가 덮은 글字 數를 最大化한다.
따라서 `行事` + `場所`는 폴백을 남기지 않으므로 `行事場` + `所`보다 낫다.
辭典으로 덮은 글字 數가 같으면 分割 數가 더 적은 쪽을 選好한다. 그래서 `天地`
같은 全體 單語 一致는 `天` + `地` 같은 構成要素 分割보다 낫다. 그래도 同點이면
같은 點數에 먼저 到達한 候補를 維持해 結果를 決定的으로 만든다.

라티스 分割의 費用은 變換 候補 範圍의 길이와 辭典의 最大 項目 길이의 곱에
비례한다. 普通의 韓國語 텍스트에서 漢字를 包含하는 範圍는 짧다; 大部分 1–4
글字이고, 混用 表記 辭典 項目 같은 드문 境遇를 빼면 10 글字를 넘는 일은 거의
없다. 辭典의 最大 項目 길이도 10 字 內外이다. 따라서 範圍 하나당 數十 番의
辭典 照會면 充分하며, FST와 CDB 백엔드 모두 마이크로秒 單位로 處理한다.

라티스의 正確度가 必要 없고 範圍 當 追加 費用을 줄이고 싶은 呼出者를 爲하여
**eager** 分割 戰略도 옵션으로 提供한다. 基本은 라티스이다.

### 폴백 音譯機

辭典 一致가 漢字 글字를 덮지 못할 때, 폴백 音譯機는 그 글字를 內藏된 Unihan
起源 글字 表에서 標準 讀音을 찾아 한글로 變換한다. 글字 表는 Unicode
`kHangul` 屬性으로부터 빌드되어 *gukhanmun-core*에 生成된 整列 테이블로
內藏되고 二分 探索으로 照會된다. 數千 個의 漢字를 다루며 基本 빌드는
네트워크 接近에 依存하지 않는다.

폴백이 만들어 낸 單語의 첫 글字, 卽 폴백 專用 列의 첫 글字 또는 辭典 一致
直後의 첫 글字에는 選擇的으로 頭音法則이 適用된다. 이 法則은 ㄴ이나 ㄹ로
始作하던 한글 音節 一部를 大韓民國 正書法에 맞는 形態로 變換한다(녀→여, 려→여,
례→예, 等). 對照表는 작고(16 個), *gukhanmun-core*에 內藏되어 있다. 法則을 켜고
끄는 옵션은 폴백에만 適用된다; 辭典 項目은 이미 옳은 讀音을 들고 있다고
假定한다(大韓民國 辭典은 `來日`을 「내일」로, 朝鮮民主主義人民共和國 辭典은
「래일」로 收錄한다).

폴백에는 글字別 寫像으로는 얻을 수 없는 Seonbi의 작은 規則 두 가지가 남아 있다.
첫째, `列`·`律` 規則은 頭音法則의 一部로 다룬다. 頭音法則이 켜져 있고 이
글字들이 앞 音節의 받침이 ㄴ이거나 받침이 없을 때 「렬·률」 代身 「열·율」로
미끄러진다. 頭音法則이 꺼져 있는 朝鮮民主主義人民共和國 正書法에서는 「렬·률」로
남는다. 둘째, 漢字 數字 規則: 두 個 以上의 漢字 數字가 連續되면 한 單語로 읽고,
頭音法則은 첫머리에만 適用하고 內部에는 適用하지 않는다. 두 規則 모두 글字
흐름에 對한 작은 파서로 인코딩되어 있다.

### 數字 變換

漢字 數字는 特殊한 境遇인데, 文脈에 따라 세 가지 表面 形態로 變換될 수 있고
어느 쪽이 옳은지가 文脈에 依해 決定되기 때문이다. 라이브러리는
`NumeralStrategy` 옵션을 네 가지로 提供한다:

| 戰略                | 例: `二〇一六年` | 例: `十一月` | 例: `一千二百三十四` |
| ------------------- | ---------------- | ------------ | -------------------- |
| `hangul-phonetic`   | 이공일륙년       | 십일월       | 일천이백삼십사       |
| `positional-arabic` | 2016년           | (適用 不可)  | (適用 不可)          |
| `additive-arabic`   | (適用 不可)      | 11월         | 1234                 |
| `smart`             | 2016년           | 11월         | 1234                 |

`hangul-phonetic` 戰略은 Seonbi의 動作이고 `ko-kr`·`ko-kp` 두 프리셋의
基本값이다. `positional-arabic` 戰略은 數字만으로 이뤄진
列(`〇一二三四五六七八九`와 그 異體字)을 位置 表記로 보고 아라비아 數字로
變換한다. `additive-arabic` 戰略은 자릿값 標識(`十百千萬億兆京`)가 包含된 列을
스택 基盤 누적으로 解析해 아라비아 數字를 내놓으며, `一`이 `十` 앞에서 省略되는
韓國式 表記(`十一`이 11이지 `一十一`이 아님)를 處理한다. `smart` 戰略은 周邊
文脈을 본다: 單位 漢字(`年月日時分秒號世紀` 等)가 따라오면 `additive-arabic`을
쓰고, 그렇지 않으면서 4 字 以上의 純粹 數字라면 `positional-arabic`을 쓰며(年度
表記 慣行에 맞춤), 그 外에는 `hangul-phonetic`으로 還元한다.

數字 變換은 폴백 經路 안에서, 라티스가 「辭典에 一致하지 않음」으로 判定한
區間에 對하여 動作한다. `hangul-phonetic` 戰略은 폴백 `Annotated` 토큰을
내보내어 原 漢字 數字를 保存하므로, `HangulHanjaParens` 같은 렌더러도 原文을
表示할 수 있다. 아라비아 數字 戰略은 原 漢字의 한글 讀音이 아니라 數字
正規化를 出力하는 것이므로, 純粹 텍스트를 내보낼 수 있다.


辭典
----

辭典은 「주어진 텍스트 位置에서 始作하는 一致가 무엇인지 물어볼 수 있는」 모든
것이다.

~~~~ rust
pub trait HanjaDictionary {
    /// `s`의 첫머리에서 始作하는 모든 一致를 yield 한다.
    fn matches_at<'a>(&'a self, s: &'a str)
        -> Box<dyn Iterator<Item = Match<'a>> + 'a>;

    /// 이 辭典에 들어 있는 項目 中 가장 긴 것의 길이(글字 單位).
    /// 라티스의 終了 條件으로 使用된다.
    fn max_word_chars(&self) -> Option<usize> { None }

    /// 배치 政策 index를 만들 만큼 效率的으로 列擧할 수 있을 때의 完整한 項目들.
    fn entries<'a>(&'a self)
        -> Option<Box<dyn Iterator<Item = DictionaryRecord> + 'a>>
    { None }

    /// 同一한 한글 讀音을 가진 다른 漢字 單語가 있는가?
    /// 便宜 API이다. homophone-marker 미들웨어는 反復 全體 走査를 피하려고
    /// entries()를 使用한다.
    fn has_homophone(&self, hanja: &str, reading: &str) -> bool { false }
}
~~~~

`matches_at`이 異例的인 部分이다. 韓國語 텍스트 處理에 自然스러운 시그니처는
`longest_match`처럼 보이지만, 그 eager longest-match가 바로 우리가 엔진에서
排除한 算法이다. 라티스 分割機가 代案들을 評價하려면 始作 位置에서 가능한
**모든** 一致 集合이 必要하므로, 트레이트는 그 全體를 露出한다. 入力 文字列은
미리 잘라 낸 漢字 列만이 아니라 現在 커서에서 始作하는 텍스트의 나머지이다. 이
形態 덕분에 辭典은 `汽車길` 같은 混用 表記 키도, 그 一致 안에 漢字가 하나 以上
들어 있는 한, 담을 수 있다.

`Match`는 一致한 바이트 길이, 한글 讀音, 그리고 `MatchMark`를 들고 있다.
`byte_len`은 一致한 辭典 키의 UTF-8 길이이며, 그 키에는 漢字와 한글이 함께
들어 있을 수 있다:

~~~~ rust
pub struct Match<'a> {
    pub byte_len: usize,
    pub reading: Cow<'a, str>,
    pub mark: MatchMark,
}

pub struct MatchMark {
    /// 이 項目은 항상 漢字와 함께 表示되어야 한다고 辭典이 主張.
    pub require_hanja: bool,
    /// 이 項目은 항상 한글과 함께 表示되어야 한다고 辭典이 主張.
    pub require_hangul: bool,
}
~~~~

이 標識들은 出處 辭典의 빌드-時點 메타데이터에서 온다. 內藏된
《標準國語大辭典》의 CDB·FST 파일은 어려운 漢字와 曖昧한 讀音 等 漢字 倂記가
바람직한 項目을 列擧하는 規則 파일과 함께 빌드된다.

### 內藏 具顯體

`UnihanCharDict`는 글字 單位 Unihan 讀音 테이블을 `HanjaDictionary`로 露出한
것이다. 呼出者는 다른 辭典 소스와 같은 公開 辭典 인터페이스로 이 讀音을
結合할 수 있다. 具顯은 Unicode `kHangul` 屬性에서 만들어지는 生成 整列
테이블이며, 頭音法則이나 數字 묶음 같은 狀態 있는 폴백 規則은 엔진의 役割로
남긴다.

`MapDictionary`는 테스트, 프로그램 內에서 提供되는 項目, 이미 메모리에 있는
작은 用語集을 爲한 메모리 內 辭典이다. 依存性을 작게 維持하고 `no_std` core
크레이트에서 쓸 수 있도록 順序 맵을 基盤으로 한다. 壓縮된 直列化 데이터,
mmap 親和的 로딩, 큰 靜的 辭典이 必要한 呼出者는 代身 `gukhanmun-fst`
백엔드를 使用해야 한다.

`ChainDictionary`는 優先順位 政策에 따라 辭典들을 連續으로 結合한다. 呼出者는
작은 使用者 辭典(最高 優先), 主題別 辭典, 《標準國語大辭典》을 잇고, 正規
글字 單位 辭典 match가 必要할 때 `UnihanCharDict`(最低 優先)를 더할 수 있다.

### 外部 백엔드

두 個의 外部 辭典 백엔드가 別途 크레이트로 提供된다.

*gukhanmun-cdb*는 CDB 파일을 `HanjaDictionary`로 감싼다. CDB는 djb의 constant
database이다; $O(1)$ 照會를 提供하는 靜的 디스크 해시 테이블이며, 파일 形式이
사소하다. 漢字→한글 寫像을 素朴하게 CDB에 넣으면 失敗한다, CDB는 接頭辭 巡廻
없는 해시 테이블이라 「어떤 키가 이 바이트 列로 始作하는가」를 물을 수 없기
때문이다. 우리는 辭典을 **CDB 키 空間에 埋藏된 trie**로 인코딩해서 이를
回避한다. 빌드 時點에 *gukhanmun-mkdict*가 各 項目의 모든 接頭辭를 列擧해
各各에 對하여 한 個의 레코드를 저장하며, 完全한 單語인지 中間 接頭辭인지를 1
바이트 플래그로 區分한다:

~~~~ text
key   = 接頭辭의 utf-8 바이트
value = { is_complete: u8, mark: u8, reading_len: u16, reading: utf-8 }
~~~~

照會는 커서 位置에서 한 글字씩 進行한다. 失敗(miss)가 나면 이 位置에서 더 긴
一致는 不可能하므로 巡廻를 終了한다. `is_complete = 1`로 一致하면 그 一致를
yield한다; `is_complete = 0`으로 一致하면 더 긴 一致를 찾아 巡廻를 繼續한다.
費用은 位置 當 $O(\text{max_word_chars})$ 回의 CDB 照會이고, 各 照會는
$O(1)$이다. 크기 費用은 實在한다: 各 項目의 모든 接頭辭가 한 레코드를
차지하므로 內藏 stdict CDB는 原本 TSV의 約 2 倍이다. 折衷의 對價는 CDB의
單純함이다(파일 形式 文書가 6 個 syscall 분량이고 公共 領域의 參照 具顯體들이
存在함). 백엔드의 監査가 사소해진다.

*gukhanmun-fst*는 `fst::Map`을 `HanjaDictionary`로 감싼다. FST(finite state
transducer)는 接頭辭 巡廻를 native로 支援하고 CDB-as-trie보다 壓縮이 좋지만,
디스크 形式이 그만큼 普遍的으로 具顯되어 있지는 않다. 둘 다 提供하는 理由는
使用者의 優先順位가 다르기 때문이다; CDB는 코드 監査와 사소한 mmap 支援을 爲한
選擇이고, FST는 작은 WebAssembly 번들을 爲한 選擇이다.

### 辭典 道具

*gukhanmun-mkdict*는 CDB·FST 辭典 파일을 빌드하는 別途의 CLI 道具이다.
TSV·CSV·JSON Lines 入力을 받고, 設定 可能한 衝突 政策으로 여러 入力 파일을
倂合하며, 結果를 라운드트립으로 檢證하고, 헤더에 빌드
메타데이터(出處·라이선스·빌드 日字)를 埋藏한다.

~~~~ text
gukhanmun-mkdict [OPTIONS] -o OUTPUT <INPUT>...

INPUT FORMATS:
    tsv     hanja TAB hangul [TAB flags]
    csv     hanja,hangul[,require_hanja,require_hangul]
    jsonl   {"hanja":..., "hangul":..., "requireHanja":..., ...}

OPTIONS:
    -o, --output PATH               output path
    -f, --format FMT                cdb|fst            (default: fst)
        --merge STRATEGY            first-wins|last-wins|error
        --metadata KEY=VAL          embedded metadata
        --validate                  round-trip verification
        --max-key-bytes N           reject pathologically long entries
        --rules PATH                annotation rules TSV (repeatable)
        --allow-unmatched-rules     accept rules that match no entries
~~~~

規則(rules) 파일은 `kind`, `pattern`, `require_hanja`, `require_hangul`,
`reason` 컬럼을 갖는 TSV이다. `kind`는 세 種類의 셀렉터를 고른다.
`entry`는 漢字 키 全體와 正確히 一致하는 單一 엔트리, `contains`는
`pattern`이 漢字 substring(1字 以上)이면 그것을 包含하는 모든 엔트리,
`reading`은 `pattern`이 한글 reading일 때 같은 reading을 가진 모든 엔트리를
表示한다. `contains` pattern은 漢字만 許容한다. 辭典 키가 mixed-script
(`布告하다` 等)이라 한글이 섞인 substring을 許容하면 無關한 엔트리까지 조용히
表示되기 때문이다. 같은 엔트리에 닿는 여러 規則의 表示 비트는 OR-merge된다. 각
規則은 `require_hanja` 및 `require_hangul` 中 하나 以上을 設定해야 하고
`reason`이 비어 있으면 안 되며, 實際로 매칭되는 엔트리가 0個이면 빌드를
失敗시킨다(規則 파일이 辭典과 어긋나 사라진 stale rule을 방지). 더 작은 辭典과
規則 파일을 共有하는 境遇 `--allow-unmatched-rules`로 迂廻한다.

이 同一한 바이너리를 *gukhanmun-stdict*의 빌드 스크립트도 呼出해서 內藏
《標準國語大辭典》 FST 파일을 만든다. CDB 백엔드는 使用者가 빌드하는 辭典에
對해 같은 正規化 入力과 檢證 經路를 使用한다. 卽 末端 使用者와 라이브러리
自體가 같은 빌드 經路를 共有하는 셈이다. 빌드 經路의 버그는 라이브러리 自身의
統合 테스트에 먼저 걸린다.


미들웨어
--------

렌더러의 入力은 `OutputToken` 흐름이고, 그 안의 各 `Annotated`는 플래그들을
들고 있다. 플래그는 엔진이 該當 註解에 對하여 알고 있는 것을 描寫한다: 辭典에서
왔는가, 同音異義語가 있는가, 이 文脈에서 처음 본 單語인가. 플래그는 「렌더러가
이를 어떻게 처리할 것인가」를 描寫하지는 않는다; 그것은 미들웨어가 設定한다.

「어떤 註解를 漢字와 함께 提示할지, 어떤 것은 한글만 提示할지, 『첫 番째』는
언제 리셋되는지」와 같은 政策을, 「括弧·루비·한글 專用」 같은 形式 決定과
分離하면, Seonbi에 비해 깔끔한 파이프라인이 나온다. Seonbi에서는 렌더링 函數가
「무엇을 提示할지」와 「어떻게 提示할지」를 함께 決定한다. 그 結果, 同音異義
區別 렌더러는 內部에 同音異義 探知 패스를 거의 그대로 다시 具顯해야 하고, 다른
同音異義 휴리스틱으로 갈아끼우려면 렌더러 自體를 다시 써야 한다.
Gukhanmun에서는 미들웨어가 `OutputToken` 흐름에 對한 狀態 維持 필터이고,
렌더러는 (普通) 無狀態의 `Annotated` 飜譯機이다.

### 內藏 미들웨어

`HomophoneMarker`는 흐름을 走査하면서, 有效 辭典 項目 集合이나 設定된 文脈 窓
內에서 다른 漢字 表記와 한글 讀音을 共有하는 註解에 `homophone = true`를
設定한다. 백엔드가 項目 列擧를 露出하면 `HanjaDictionary::entries()`로
reading-to-hanja index를 한 번 만든다. 照會 專用 辭典은 `has_homophone()`으로
fallback하며 文脈 內 표시도 계속 받는다. 窓은
`per-block`(基本값)·`per-document`·`off` 셋 中 하나이다. per-block 窓은 다음
「`is_block_boundary()`가 true를 返還하는 스코프」가 나올 때까지만 버퍼링하며,
普通 한 段落이나 한 리스트 項目 程度이다. per-document 窓은 흐름 全體를
버퍼링하므로 入力이 작거나 完全한 正確度가 應答 지연보다 重要할 때에만 적합하다.
純粹 텍스트에는 블록 스코프가 없으므로 `per-block`은 文書 全體 窓이 된다. 이는
意圖한 契約이다. 1번째 줄에 `漢字`가 나오고 100번째 줄에 `翰字`가 나오면 두 줄
모두 漢字 倂記로 렌더링되어야 하며, 스트리밍 라이터는 100번째 줄을 본 뒤 1번째
줄을 遡及 修正할 수 없다.

`FirstOccurrenceFilter`는 設定된 文脈 內에서 첫 登場 以後의 註解들에 對하여
`require_hanja`·`require_hangul`을 解除한다. 첫 番째 登場은 그대로 두어 讀者가
한 番은 倂記를 보게 한다. 文脈은 `per-block`·`per-section`·`per-document` 中
하나이다. section 變種은 모든 헤딩 境界에서 리셋되며, HTML·Markdown 어댑터가
헤딩 스코프의 `is_section_boundary()`로 그 境界를 露出한다.

`UserDirectives`는 使用者가 提供한 規則 集合을 適用한다. 規則은 「漢字 表記에
對한 述語」 + 「動作」으로, 動作은 `require_hanja` 設定·`require_hangul`
設定·註解 自體를 건너뛰기(이 境遇 活性化된 렌더러에 따라 한글이나 漢字만
남는다) 中 하나이다. 述語는 文字列 集合·glob 패턴·또는 Rust 呼出者의 任意
클로저일 수 있다; JavaScript 側에서는 토큰 當 境界 橫斷 費用을 避하기 爲하여
文字列 集合 形態만 露出한다.

### 使用者 定義 미들웨어

미들웨어는 上流 이터레이터를 받는 `impl Iterator<Item = OutputToken<S>>`이다.
트레이트 表面이 작아 한 자리에서 손쉽게 쓸 수 있다. 例를 들어 自身의 用語集에서
技術 用語 註解를 標識하고 싶다면, 用語集 集合을 들고 一致 時 `require_hanja`를
設定하는 미들웨어를 짜면 된다.


렌더러
------

렌더러는 `Annotated` 토큰을 그 모드와 플래그에 따라 具體的인
`Text`·`Open`·`Close` 토큰으로 풀어낸다. 라이브러리가 提供하는 렌더러는
다섯이다.

`HangulOnly` 렌더러는 한글 讀音만을 내보낸다. `require_hanja`나 `homophone`이
設定되어 있으면 `한글(漢字)` 形態로 내보낸다. `require_hangul`이 設定되어
있어도 結果는 이미 한글이라 變化가 없다.

`HangulHanjaParens` 렌더러는 항상 `한글(漢字)`을 내보낸다. `require_hangul`은
한글 部分이, `require_hanja`는 漢字 部分이 充足한다.

`HanjaHangulParens` 렌더러는 항상 `漢字(한글)`을 내보낸다. 學術·古文書 樣式에서
漢字를 앞세우는 表記에 適合하다.

`Ruby` 렌더러는 `<ruby>` 要素를 만들어 내며, 어느 쪽을 基底로 두는지는 下位
모드로 決定된다: `on-hangul`은
`<ruby>한글<rp>(</rp><rt>漢字</rt><rp>)</rp></ruby>`, `on-hanja`는
`<ruby>漢字<rp>(</rp><rt>한글</rt><rp>)</rp></ruby>`. `<rp>` 要素는 括弧 代替
텍스트를 담아, `<ruby>`를 支援하지 않는 브라우저에서도 讀音이 基底에 붙어
버리지 않고 `基底(讀音)` 形態로(`on-hangul`은 `한글(漢字)`, `on-hanja`는
`漢字(한글)`) 括弧로 나타나도록 한다. 現在 스코프의 `allows_inline_markup()`이
false면 括弧 形式으로 還元한다.

`Original` 렌더러는 原 漢字를 純粹 텍스트로 내보낸다. `require_hangul`이
設定되어 있거나 使用者 指示로 標識된 註解만 倂記를 받으며, 倂記 形態는 下位
옵션에 따라 括弧나 ruby가 된다. 「國漢文 表記는 그대로 두고 어려운 글字만
倂記한다」는 모드이며, 이 設計 文書의 韓國語版이 바로 이 樣式을 採擇하고 있다.

렌더러는 單一 `Annotated` 토큰과 現在 스코프의 `allows_inline_markup()` 값에
對한 純粹 函數이다. 出力은 작고 固定된 크기의 토큰 시퀀스이다(한글이나 漢字
單獨이면 1 個, 括弧 形式이면 3 個, ruby면 5–9 個). 狀態도 없고 버퍼링도 없다.

形式 決定을 렌더러에 둔 理由는, 그 形式이 同一한 스코프 位置에 다른 어떤
토큰들이 흐르고 있는가에 依存하기 때문이다. 例를 들어 `<pre>` 內部의 `<ruby>`는
잘못이며, 그 判斷은 스코프 스택을 必要로 한다. 스코프 스택은 IR을 通해 흐르고
있는 그 데이터 自體이다. 形式 決定을 다른 곳에 두면 스코프 追跡을 二重으로
가지든가, 렌더러가 다른 미들웨어들에 對하여 알고 있어야 하는 結果가 된다.


形式 어댑터
-----------

라이브러리에 包含된 어댑터는 셋이다. 새 어댑터를 追加할 때 必要한 것은 「形式의
토큰과 IR을 飜譯하는」 `Reader`·`Writer` 具顯뿐이다.

### HTML

HTML 어댑터는 손수 쓴 스캐너로 `InputToken<HtmlScopeData>` 이벤트를 만든다. 이
스캐너는 Seonbi의 `Text.Seonbi.Html.Scanner` 모듈을 거의 그대로 옮긴 것으로, 數
年間 實 環境의 韓國語 웹 콘텐츠에서 檢證되어 왔다. 段落 中心 設計: 入力은
完全한 文書·`<body>` 內容·또는 단편일 수 있고, 스캐너는 트리 構成을 試圖하지
않고 본 이벤트를 그대로 내놓는다.

html5ever와 lol\_html도 檢討했다. html5ever는 Rust의 標準 HTML5 파서이고 DOM을
만들며 仕樣의 모든 邊緣 事例를 處理한다. lol\_html은 Cloudflare의 스트리밍 HTML
리라이터로, 셀렉터 中心이며 WebAssembly에 잘 어울린다. 그럼에도 손수 쓴
스캐너를 擇한 두 가지 理由가 있다. 첫째, Seonbi 接近은 屬性 文字列을 **날
文字列로 그대로 保存**하므로, 라이터가 變化 없는 스코프를 入力에 나타난 그대로
直列化할 수 있다. 둘째, WebAssembly 번들에 充分히 작게 들어간다; html5ever를
함께 가져오면 바이너리 크기를 支配해 버린다. 對價는 이 스캐너가 HTML5에 完全히
適合하지는 않다는 點이고, 우리는 그 折衷을 受容한다.

스캐너는 가벼운 誤謬는 復舊한다. 닫히지 않은 태그는 같은 이름의 가장 最近
스코프를 pop하고, 그런 스코프가 없으면 텍스트로 내보낸다. `<`로 始作하지만
有效한 태그·코멘트·CDATA 始作이 아닌 構造는 텍스트 글字로 내보낸다. 完全히
망가진 入力도 토큰 흐름은 만들어 내며, 다만 構造的 異常이 包含될 뿐이다; 엔진은
異常에 強健하고(스코프 一致를 假定하지 않는다), 라이터는 받은 스코프를 그대로
내보낸다.

`HtmlScopeData::is_preserve()`는 現在 要素가
`pre`·`code`·`kbd`·`script`·`style`·`textarea` 中 하나이거나, 相續된 `lang`
屬性이 韓國語가 아니면 true를 返還한다. `lang` 相續은 어댑터 內部에서 計算된다:
各 `Open` 이벤트에서 날 屬性을 評價해 `lang` 값을 찾고, 어댑터는 自身의 `lang`
스택을 維持한다. 코드베이스에서 `lang`이 무엇인지 아는 唯一한 곳이며, 保存 對象
태그 目錄을 아는 唯一한 곳이기도 하다. 엔진은 그 結果를 `is_preserve()`로 받을
뿐, 어느 規則도 다시 따라 하지 않는다.

### Markdown

Markdown 어댑터는 `pulldown-cmark` 위에 얹은 얇은 階層이다. 各
`pulldown-cmark::Event`는 한 個 以上의 IR 토큰이 된다.
`Start(Tag)`·`End(Tag)`는 `Open`·`Close`로, `Text`는 `Text`로, `Code`는
`Verbatim`으로 對應된다. 인라인 HTML(즉 `Event::Html`)은 HTML 스캐너로 한 番 더
通過시켜, 段落 內部의 `<q lang="ja">` 같은 構造도 올바른 `lang` 處理를 받게
한다.

出力은 `pulldown-cmark-to-cmark`를 通해 이뤄지며, 이는 이벤트 흐름을 다시
Markdown으로 直列化한다. 意味 保存은 契約이다: 出力을 다시 解析하면 同一한 論理
構造가 나온다. 바이트 單位 保存은 最善의 努力이다: setext 헤딩이 ATX로 바뀔 수
있고, 링크 參照 定義가 인라인化될 수 있으며, 軟性 줄바꿈이 規則化될 수 있다.
바이트 單位 忠實性이 必要한 使用者는 Markdown보다는 렌더링된 HTML을 處理해야
한다.

`pulldown-cmark`를 擇한 理由는 CommonMark 適合은 큰 作業이고 그것은
`pulldown-cmark`가 잘 處理하기 때문이며, 그 이벤트 흐름 API가 우리의 IR과 거의
完璧히 一致하기 때문이다. 代案은 `markdown-rs`였는데, 그것은 이벤트 흐름이
아니라 AST를 産出한다; 스트리밍 性質을 維持하기 爲하여 이벤트 흐름을 選好했다.

### 純粹 텍스트

純粹 텍스트 어댑터는 全體 入力을 單一 스코프로 감싸 한 個의 `Text` 토큰을
내보낸다. 出力은 `Text` 토큰들의 連結이다. Ruby 렌더링은 純粹 텍스트에서 意味가
없으므로 括弧 形式으로 還元된다. CLI는 이미 렌더링한 出力을 文書 全體
미들웨어가 나중에 바꿀 可能性이 없을 때에만 純粹 텍스트를 EOF 前에 스트리밍할 수
있다. 實際로 `ko-kp` 프리셋은 同音異義 標識이 꺼져 있으므로 스트리밍하지만,
基本 `ko-kr` 프리셋은 줄 사이 同音異義를 正確히 렌더링하기 爲해 純粹 텍스트
出力을 EOF까지 保留한다.


配布
----

### Rust 워크스페이스

Rust 코드는 Cargo 워크스페이스로 다음과 같이 構成된다:

 -  *Cargo.toml*: 워크스페이스 매니페스트
 -  *DESIGN.md*: *DESIGN.en.md*로의 심볼릭 링크
 -  *DESIGN.en.md*, *DESIGN.ko-Kore.md*: 設計 文書
 -  *crates/*
     -  *gukhanmun-core/*: IR 타입·엔진·辭典 트레이트·라티스 分割機·폴백
        音譯機·頭音法則 對照表·內藏 `UnihanCharDict`. I/O 없음, 形式 依存 코드
        없음, 依存性 最小. `alloc`이 있는 `no_std` 環境에도 適合.
     -  *gukhanmun-html/*: HTML 스캐너와 直列化機. `HtmlScopeData` 具顯.
     -  *gukhanmun-markdown/*: `pulldown-cmark` 위에 얹은 Markdown 어댑터.
     -  *gukhanmun-cdb/*: CDB-trie 辭典 백엔드.
     -  *gukhanmun-fst/*: FST 辭典 백엔드.
     -  *gukhanmun-stdict/*: 內藏된 《標準國語大辭典》을 FST 바이트 配列로 提供.
     -  *gukhanmun-mkdict/*: TSV·CSV·JSONL 入力에서 CDB·FST 辭典을 빌드하는 CLI.
     -  *gukhanmun/*: 우산 라이브러리 크레이트. 기능 플래그에 따라 다른
        크레이트들을 再露出하고, 高水準 `Builder` API와 우산 `Error` 列擧型을
        提供.
     -  *gukhanmun-cli/*: `gukhanmun` 命令줄 바이너리.
     -  *gukhanmun-wasm/*: `wasm-bindgen`을 通한 WebAssembly 바인딩.
     -  *gukhanmun-napi/*: `napi-rs`를 通한 Node-API 바인딩.

우산 크레이트의 기능 플래그가 나머지를 組合한다. 基本 기능은 HTML·Markdown·內藏
stdict를 活性化한다. CDB·FST는 個別 選擇 可能하다. 모두 꺼 두면 엔진과
`UnihanCharDict`만 남는 Rust API 專用 빌드가 되며, 임베디드 환경에 適合하다.

### JavaScript 패키지

JavaScript 側은 타입 專用 패키지 하나와, 런타임 具顯體別 패키지들로 分離된다:

 -  *@gukhanmun/types* (npm·JSR): TypeScript 인터페이스·타입 別稱·에러 클래스
    宣言·`GukhanmunFactory` 인터페이스. 런타임 코드 없음; npm에는 宣言만,
    JSR에는 *.ts* 原本을 그대로 配布. API 契約의 正本이며, 두 具顯體 모두
    構造的으로 이를 滿足한다.
 -  *@gukhanmun/wasm* (npm·JSR): WebAssembly 具顯體. 타입을 再露出. *.wasm*
    아티팩트를 `import.meta.url`로 解決해서, Deno·브라우저가 native로, Node 22
    以上이 標準 ESM 로더로 解決할 수 있게 한다.
 -  *@gukhanmun/napi* (npm 專用): Node-API 具顯體. 타입을 再露出. napi-rs의
    optional dependency 패키징으로 플랫폼別 prebuilt 바이너리를 配布.

辭典 데이터는 別途 패키지로 配布해 런타임 번들이 작게 維持되도록 한다:

 -  *@gukhanmun/stdict-fst* (npm·JSR): 內藏 stdict를 FST 形態로 `Uint8Array`
    export.
 -  *@gukhanmun/stdict-cdb* (npm·JSR): 同一한 辭典을 CDB 形態로 提供.
 -  *@gukhanmun/stdict-min* (npm·JSR): 同音異義 項目과 曖昧 讀音만 줄여서 담은
    縮小 FST. 크기 민감한 文脈用.

타입 專用 패키지를 따로 두는 理由는 API 契約의 正本이 한 곳에만 있어야 하기
때문이다. 萬一 契約이 *@gukhanmun/wasm*에 살고 *@gukhanmun/napi*가 그것을
重複하거나 서로에서 가져온다면, 두 具顯體 사이 버전 어긋남이 維持保守 負擔이
된다. *@gukhanmun/types*를 兩 具顯體의 `peerDependency`로 두면, 使用者는 런타임
選擇과 無關하게 唯一한 出處와 唯一한 타입 集合을 얻는다. 棄却한 두 代案은
다음과 같다: 타입을 WASM 패키지에 두고 NAPI가 그것에 直接 依存(非對稱이며,
NAPI가 WASM에 從屬됨); 두 패키지에 타입을 重複(複寫本 사이의 drift, 正本
TSDoc을 둘 곳이 없음).

JavaScript 側 옵션 列擧型은 const-as 對象이 아니라 文字列 union 타입이다.
이것이 *@gukhanmun/types*를 眞正한 의미의 타입 專用으로 만든다: 런타임 코드를 0
바이트 내보내므로 번들에서 重要하고, JSR 패키지의 原本을 옮겨 적기(transpile)
없이 *.ts* 한 個로 끝낼 수 있다. 折衷은 옵션 文字列이 런타임에 「stringly
typed」가 된다는 點이다; 두 具顯體는 境界에서 이를 檢證하고, 認識할 수 없는
값에 對하여는 `invalid-input` 코드의 `GukhanmunError`를 던진다.

JavaScript 側 스트리밍은 플랫폼의 `TransformStream<string, string>`
인터페이스를 利用한다. 이 인터페이스는 브라우저·Deno·Node 18 以上·Bun에서 두루
使用 可能하다. 청크는 JavaScript 文字列이고, 인코딩
處理(`TextDecoderStream`·`TextEncoderStream`)는 Gukhanmun 스트림 바깥에 있다.
엔진 內部에서는 청크가 變換 候補 範圍 中間에서 끝나거나, 漢字 글字에 너무 가까워
混用 表記 辭典 키가 境界를 넘어 이어질 可能性이 있으면, 그 꼬리 範圍를 다음
청크까지 保留하며, 그 앞까지는 즉시 내보낸다. 그 中 辭典 先讀에 必要한 버퍼는
辭典의 `max_word_chars`에 라티스의 出口 狀態를 爲한 작은 常數를 더한 程度로
制限되며, 普通 數 十 字 내외이다. 그러나 fallback 漢字만 이어지는 連續 範圍는
청크 境界에서 나누지 않는다. 原 漢字를 보여 주는 렌더링 모드에서는 註解 묶음이
出力에 드러나므로, 그런 範圍는 後續 非變換 境界나 EOF에서 내보낸다.

狀態 있는 미들웨어는 自身만의 先讀 要件을 追加할 수 있다. 文書 全體 文脈의
同音異義 標識, 그리고 블록 스코프가 없어 文書 全體처럼 動作하는 純粹 텍스트
`per-block`은 正確한 렌더링을 保證하려면 EOF까지 버퍼링해야 한다. 이 미들웨어를
끄면 早期 스트리밍은 回復되지만 줄 사이 同音異義 區別은 함께 사라진다.

스트림 타입으로 `Uint8Array`가 아니라 文字列을 擇한 理由는 엔진이 根本的으로
Unicode 스칼라 단위로 動作하기 때문이다. 바이트 單位 청크화는 어댑터가 모든
境界에서 部分 코드포인트 再組立을 直接 해야 함을 意味하는데, 그것은 플랫폼의
`TextDecoderStream`이 이미 올바르게 해 주는 일이다. 바이트 스트림을 들고 있는
使用者는 `TextDecoderStream` → Gukhanmun 變換 順으로 連結하면 된다.

### 辭典 設定

JavaScript의 辭典 設定은 파일 出處 또는 메모리 內 맵 中 하나를 받는다:

~~~~ ts
export type DictionarySource = FileDictionarySource | MapDictionarySource;

export interface FileDictionarySource {
  readonly data: BufferSource | string | URL;
  readonly format: "cdb" | "fst" | "tsv";
}

export type MapDictionarySource =
  ReadonlyMap<string, Omit<DictionaryEntry, "hanja">>;
~~~~

두 變種은 런타임에서는 `instanceof Map`으로, 컴파일 時點에서는 構造 타입으로
區分된다. `Map` 形態는 코드 안에서 만드는 작은 用語集에 便利하고, 파일 形態는
配布되는 辭典을 爲한 것이다.

### 레지스트리 對照表

| 패키지                  | crates.io | npm         | JSR             |
| ----------------------- | --------- | ----------- | --------------- |
| `gukhanmun-core`        | 예        | 아니오      | 아니오          |
| `gukhanmun-html`        | 예        | 아니오      | 아니오          |
| `gukhanmun-markdown`    | 예        | 아니오      | 아니오          |
| `gukhanmun-cdb`         | 예        | 아니오      | 아니오          |
| `gukhanmun-fst`         | 예        | 아니오      | 아니오          |
| `gukhanmun-stdict`      | 예        | 아니오      | 아니오          |
| `gukhanmun-mkdict`      | 예        | 아니오      | 아니오          |
| `gukhanmun`             | 예        | 아니오      | 아니오          |
| `gukhanmun-cli`         | 예        | 아니오      | 아니오          |
| `gukhanmun-wasm`        | 예        | 아니오      | 아니오          |
| `gukhanmun-napi`        | 예        | 아니오      | 아니오          |
| `@gukhanmun/types`      | 아니오    | 예 (宣言만) | 예 (`.ts` 原本) |
| `@gukhanmun/wasm`       | 아니오    | 예          | 예              |
| `@gukhanmun/napi`       | 아니오    | 예          | 아니오          |
| `@gukhanmun/stdict-fst` | 아니오    | 예          | 예              |
| `@gukhanmun/stdict-cdb` | 아니오    | 예          | 예              |
| `@gukhanmun/stdict-min` | 아니오    | 예          | 예              |

CLI 바이너리는 各 릴리스 GitHub Releases에 플랫폼別(Linux x86\_64·aarch64, macOS
arm64·x86\_64, Windows x86\_64) 添附된다.

버전은 모든 패키지에서 lockstep으로 進行한다. 매 릴리스 태그가 Rust
워크스페이스의 모든 크레이트 버전과 모든 JavaScript 패키지 버전을 同時에
올린다. 어떤 패키지는 該當 릴리스에서 機能的 變化가 없을 수도 있는데, 그래도
버전은 올라간다. 言語 間 이야기를 明確하게 維持하기 爲함이다. 패키지 個別
semver 代身 lockstep을 擇한 理由는 依存 範圍 不一致
費用(*@gukhanmun/wasm@1.2*와 *@gukhanmun/types@1.3*을 함께 設置한 使用者가
混亂스러운 타입 誤謬를 보는 等)이 가끔 일어나는 「實質 變化 없는 버전 올림」
費用보다 크기 때문이다. 태그 푸시에 反應하는 CI 워크플로가 crates.io에
配布하고, NAPI prebuilt를 플랫폼別로 竝列 빌드하고, npm·JSR에 配布한다. 같은
태그에서 다시 配布를 실행해도 重複 덮어쓰기를 拒否하는 레지스트리들 對象으로는
無動作이다.


엔지니어링 政策
---------------

### 에러

各 크레이트는 `thiserror`로 自身만의 에러 列擧型을 定義한다. 우산 *gukhanmun*
크레이트가 `#[from]`으로 그것들을 모아, 呼出者가 크레이트 境界를 넘어 `?`
演算子를 自然스럽게 쓸 수 있게 한다. 패턴:

~~~~ rust
// gukhanmun-core/src/error.rs
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("dictionary load failed: {0}")]
    DictionaryLoad(String),

    #[error("segmentation failed for {hanja:?}: {reason}")]
    Segmentation { hanja: String, reason: String },

    #[error("invalid hangul reading {reading:?} for hanja {hanja:?}")]
    InvalidReading { hanja: String, reading: String },

    #[error("internal invariant violated: {0}")]
    Internal(&'static str),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}
~~~~

`#[non_exhaustive]` 屬性 덕분에 마이너 릴리스에서 새 變種을 追加해도 呼出者의
코드가 깨지지 않는다; 下流의 `match` 文은 wildcard 가지를 두도록 強制된다. 各
變種은 사람이 읽을 수 있는 메시지와 機械가 다룰 수 있는 構造化 데이터를 同時에
들고 있다. `std::error::Error::source()` 사슬은 `#[from]`과 `#[source]`로
維持되므로, 에러를 거슬러 올라가면 全體 痕跡을 얻을 수 있다.

라이브러리 크레이트는 `anyhow`를 쓰지 않는다. CLI는 쓴다, CLI의 任務는 사람에게
에러를 보여주는 것이고, 다른 코드의 檢査 對象이 되는 것이 아니기 때문이다.

스트림 段階의 復舊 政策은 設定 可能하다. 基本값은 `Recovery::Strict`이다:
엔진은 리더 에러를 그대로 傳播하고 中斷한다. `Recovery::Lenient`는 에러를
`tracing`으로 로그하고 該當 區間에 對하여 `Verbatim` 토큰을 내보내, 그 뒤
토큰들이 繼續 흐르게 한다.

JavaScript 側에서는 에러를 區別子 코드를 가진 單一 클래스로 다룬다:

~~~~ ts
export class GukhanmunError extends Error {
  readonly code: ErrorCode;
  readonly chain: readonly { code: ErrorCode; message: string }[];
}

export type ErrorCode =
  | "dictionary-load"
  | "segmentation"
  | "invalid-reading"
  | "html-scan"
  | "html-malformed-attr"
  | "markdown"
  | "unsupported-content-type"
  | "invalid-input"
  | "io"
  | "internal"
  | "other";
~~~~

바인딩은 FFI 境界에서 Rust의 `source()` 사슬을 巡廻하면서, 에러에 `chain`
속성을 構成한다. JavaScript 呼出者는 追加 FFI 없이도 原因을 살펴볼 수 있다.

### 로깅

*gukhanmun-core*와 同伴 크레이트들은 `tracing` 크레이트에 條件 없이 依存한다.
라이브러리 코드는
`tracing::trace!`·`tracing::debug!`·`tracing::info!`·`tracing::warn!`·`tracing::error!`을
直接 使用한다. 購讀者(subscriber)가 登錄되어 있지 않을 때의 오버헤드는 呼出
地點 當 atomic load 한 番과 分岐 하나 程度로, 最適化할 만한 限界 아래이다.

呼出을 完全히 컴파일아웃하고자 하는 바이너리(WebAssembly 빌드가 代表的)는
自身의 *Cargo.toml*에서 `tracing`의 `release_max_level_off` 기능을 켠다. 이
기능은 바이너리 레벨에서 動作한다: 依存性 全體에 걸친 모든 `tracing::*!` 呼出을
컴파일 時點의 no-op으로 置換하므로, 라이브러리 側 設定을 다시 할 必要가 없다.

라이브러리 레벨에서 `tracing` 依存性 自體를 옵션으로 두고 off 經路用 스텁
매크로를 提供하는 案도 檢討했다. `log` 모듈을 各 크레이트에 두고 條件 컴파일로
`tracing` 매크로를 再露出하거나 스텁을 提供하는 追加 複雜度가 該當 바이너리
節約分보다 커서 採擇하지 않았다. WebAssembly 번들 測定 結果가 이를 要求하면
그때 再考한다.

### 테스트

테스트 모음은 네 部分이다.

**回歸 픽스처**는 Seonbi나 Gukhanmun 初期 開發에서 나타났던 特定 버그 樣相을
다룬다. Seonbi *test/data/* 디렉터리에서 適切한 部分이 그대로 移植된다; 各
픽스처는 HTML이나 Markdown 入力·期待 出力 파일 한 雙과 옆의 設定 파일로
構成된다.

**스냅샷 테스트**는 `insta`로 IR 直列化와 저장된 JSON을 比較한다. 入出力이
텍스트가 아니라 토큰 흐름인 엔진·미들웨어 크레이트에 가장 有用하다. 失敗한
스냅샷은 色色의 diff를 出力하고 對話的으로 受容·拒否를 묻는다.

**性質 基盤 테스트**는 `proptest`로 生成된 入力에 對하여 不變式을 主張한다. 가장
重要한 두 가지는: 리더 → 라이터 라운드트립이 토큰 흐름을 損失 없이
維持한다(文書化된 Markdown 「最善의 努力」 條件 除外); 그리고 한글만 든 入力에
對한 엔진 → 렌더러는 無動作이다(漢字가 없는 텍스트에서 註解를 만들어 내면 안
된다).

**適合性 테스트**는 CommonMark 仕樣의 一部 例示에 對하여 Markdown 어댑터를 돌려,
Gukhanmun이 손대지 않으려 하는 構文을 어댑터가 깨뜨리지 않음을 確認한다.

CI는 네 가지 모두를 stable·beta·워크스페이스의 MSRV(最低 支援 Rust 버전)에서
돌린다. WASM 빌드는 크기 退步 對象으로도 監視된다: 各 아티팩트別 固定 크기
豫算이 設定되어 있고, 이를 超過하는 退步는 빌드를 失敗시킨다.


프리셋
------

| 옵션                 | `ko-kr`           | `ko-kp`           |
| -------------------- | ----------------- | ----------------- |
| `rendering`          | `hangul-only`     | `hangul-only`     |
| `disambiguation`     | `per-block`       | `off`             |
| `segmentation`       | `lattice`         | `lattice`         |
| `initialSoundLaw`    | `true`            | `false`           |
| `numerals`           | `hangul-phonetic` | `hangul-phonetic` |
| `dictionary.bundled` | `"stdict-ko-kr"`  | `false`           |
| `firstOccurrence`    | (없음)            | (없음)            |

`ko-kr` 프리셋은 大韓民國의 正書法과 語彙 慣行에 맞춘다: 辭典 主導 讀音, 라티스
分割, 폴백 區間에 適用되는 頭音法則, 段落 內에서 讀音이 曖昧할 때 漢字를 括弧로
倂記하는 per-block 同音異義 區別. `ko-kp` 프리셋은 朝鮮民主主義人民共和國의
慣行, 卽 頭音法則을 適用하지 않고 漢字語를 한글로 表記하는 慣行(래일, 류행,
녀자)을 反映한다. 內藏 辭典을 가져오지 않는다, 大韓民國 《標準國語大辭典》의
讀音은 `ko-KP`에서 不正確하기 때문이다.

CLI는 두 프리셋을 `--preset ko-kr`·`--preset ko-kp`로 露出한다. 個別 옵션은
프리셋 위에서 덮어쓸 수 있다, 例를 들어 `--preset ko-kr --no-stdict`는 다른
大韓民國 基本값은 維持하면서 內藏 辭典만 끈다.


頭音法則 對照表
---------------

大韓民國 한글 맞춤법(第6章 第5節 第52項)은 一部 語頭 音節을 變換한다. 對照表는
Seonbi의 `Text.Seonbi.Hanja` 모듈에서 그대로 옮겨 왔으며, *gukhanmun-core*의
`initial_sound_law_table` 常數의 正本이다.

| 原 音 | 變換 結果 |
| ----- | --------- |
| 녀    | 여        |
| 뇨    | 요        |
| 뉴    | 유        |
| 니    | 이        |
| 랴    | 야        |
| 려    | 여        |
| 례    | 예        |
| 료    | 요        |
| 류    | 유        |
| 리    | 이        |
| 라    | 나        |
| 래    | 내        |
| 로    | 노        |
| 뢰    | 뇌        |
| 루    | 누        |
| 르    | 느        |


漢字 數字 對照表
----------------

폴백 音譯機와 `additive-arabic` 數字 戰略은 同一한 漢字 數字·자릿값 標識
對照表를 共有한다.

| 漢字                               | 값        | 備考   |
| ---------------------------------- | --------- | ------ |
| `零`, `〇`                         | 0         |        |
| `一`, `壹`, `壱`, `弌`, `夁`       | 1         |        |
| `二`, `貳`, `贰`, `弐`, `弍`, `貮` | 2         |        |
| `三`, `參`, `叁`, `参`, `弎`, `叄` | 3         |        |
| `四`, `肆`, `䦉`                   | 4         |        |
| `五`, `伍`                         | 5         |        |
| `六`, `陸`, `陆`                   | 6         |        |
| `七`, `柒`, `漆`                   | 7         |        |
| `八`, `捌`                         | 8         |        |
| `九`, `玖`                         | 9         |        |
| `十`, `拾`                         | 10        |        |
| `百`, `佰`, `陌`                   | 100       |        |
| `千`, `仟`, `阡`                   | 1000      |        |
| `萬`, `万`                         | 10000     |        |
| `億`                               | 100000000 | $10^8$ |
| `兆`                               | $10^{12}$ |        |
| `京`                               | $10^{16}$ |        |
| `垓`                               | $10^{20}$ |        |
| `秭`                               | $10^{24}$ |        |
| `穰`                               | $10^{28}$ |        |
| `溝`                               | $10^{32}$ |        |
| `澗`                               | $10^{36}$ |        |

`additive-arabic` 戰略은 자릿값 標識를 乘數로, 隣接한 數字 漢字를 被乘數로
다루며, `十`·`百`·`千`이 單獨으로 쓰일 때 各各 10·100·1000을 뜻하고
`一十`·`一百`·`一千`이 아니라는 韓國式 省略 規則도 處理한다.


保存 對象 HTML 태그
-------------------

基本 `HtmlScopeData::is_preserve()`는 屬性 內容과 無關하게 다음 태그名에 對하여
true를 返還한다: `pre`, `code`, `kbd`, `script`, `style`, `textarea`.

또한 相續된 `lang` 屬性의 主 태그가 `ko`·`kor`이나 韓國語를 가리키는 下位
태그(`ko-KR`, `ko-Hang`, `ko-Kore`, `kor-KP` 等)가 아닐 때에도 true를 返還한다.
韓國語 述語는 Seonbi의 `isKorean`과 一致한다.

이 目錄을 擴張하려는 使用者(例: `class="no-translate"` 屬性을 追加)는
`HtmlReaderOptions`의 `preserve_when` 述語를 `read_html_fragment_with_options`에
넘긴다. 述語는 各 要素에 對하여 `HtmlElementInfo`(正規 태그 名稱, 始作 태그의
raw 屬性 슬라이스, 相續된 `lang` 값)를 받아서 `true`를 返還하면 該當 스코프와
그 後孫을 保存 對象으로 표시한다. 述語로 標識된 스코프는 旣存 保存 태그처럼
後孫에게 保存 플래그를 相續하므로 子 要素마다 規則을 다시 提示할 必要가 없다.
CLI는 이 훅을 가장 자주 쓰는 두 가지 形態로 노출한다:
`--html-preserve-class CLASS`와 `--html-preserve-attr KEY[=VALUE]`(各各 反復
可能, OR 合成, `--format text/html`에서만 有效). format에 中立인 skip 클로저를
`EngineOptions`에 두는 代案은 後續 릴리즈에서 다시 檢討한다. 現在 同梱
어댑터들은 各自의 `ScopeData` 로 保存 規則을 充分히 表現할 수 있어 이번
範圍에서는 除外했다.


CDB-trie 키 스키마
------------------

CDB-trie 데이터베이스는 各 項目의 接頭辭마다 한 個의 레코드를 둔다. 키는
接頭辭의 UTF-8 바이트이다. 값의 레이아웃은 다음과 같다:

| 오프셋 | 크기          | 필드                                                                       |
| ------ | ------------- | -------------------------------------------------------------------------- |
| 0      | 1             | `is_complete` (該當 接頭辭 自體가 項目이면 1)                              |
| 1      | 1             | `mark` (비트필드: 0번 비트는 `require_hanja`, 1번 비트는 `require_hangul`) |
| 2      | 2             | `reading_len` (little-endian; `is_complete = 0`이면 0)                     |
| 4      | `reading_len` | 讀音 (UTF-8)                                                               |

別途의 잘 알려진 키 `__gukhanmun_meta__`는 빌드 메타데이터(出處·라이선스·빌드
日字·原本 項目 數·接頭辭 數·글字/바이트 單位 最大 項目 길이)를 작은 CBOR 文書로
담는다.


FST 스키마
----------

FST 데이터베이스는 各 辭典 單語마다 한 個의 항목을 둔다. 키는 漢字 表記의 UTF-8
바이트이고, 값은 다음 레이아웃의 64비트 整數이다:

| 비트  | 필드                                                        |
| ----- | ----------------------------------------------------------- |
| 0–15  | UTF-8 讀音 길이 (바이트)                                    |
| 16–23 | `mark` (CDB의 `require_hanja`·`require_hangul` 비트와 同一) |
| 24–63 | 별도 讀音 文字列 테이블 內 오프셋                           |

讀音 文字列 테이블은 FST 자체에 이어지는 連續된 UTF-8 바이트 블록이다. FST 값
타입이 固定 크기이기 때문에 可變 길이 讀音을 인라인으로 둘 수 없어 별도
테이블이 必要하다. `mark` 바이트는 모든 照會에서 確認되어 핫 經路에 屬하므로,
별도 테이블이 아닌 값 안에 함께 둔다.

파일 始作 部分의 메타데이터 헤더(8 바이트 매직, 버전, 레이아웃 오프셋, CDB와
비슷한 CBOR 메타데이터 블롭)가 FST 바이트 앞에 놓인다.


用語集
------

| 한글           | 漢字           | 英語                                |
| -------------- | -------------- | ----------------------------------- |
| 한자           | 漢字           | hanja                               |
| 한자어         | 漢字語         | Sino-Korean word                    |
| 국한문혼용     | 國漢文混用     | mixed-script Korean writing         |
| 두음법칙       | 頭音法則       | initial sound law                   |
| 표준국어대사전 | 標準國語大辭典 | Standard Korean Language Dictionary |
| 한글           | 한글           | hangul, the native Korean alphabet  |
| 한글전용       | 한글專用       | hangul-only writing                 |
| 동음이의       | 同音異義       | homophony                           |

---
title: 句讀 符號 프로파일
description: 漢字 렌더링과 獨立的으로 쉼표, 마침표, 引用 符號를 正規化하는 方法.
---

句讀 符號 프로파일
==================

句讀 符號 正規化는 明示的으로 켜는 機能이며, 漢字가 없는 入力에도 適用됩니다.
讀音 選擇과 註釋 指示가 끝난 뒤, 렌더링 前에 實行됩니다. 言語 프리셋, 辭典,
렌더링 모드와 獨立的입니다.

| 프로파일               | 쉼표/마침표 | 單一/二重 引用 符號 | 뒤의 U+0020 空白 |
| ---------------------- | ----------- | ------------------- | ---------------- |
| `preserve` (基本)      | 維持        | 維持                | 維持             |
| `modern`               | `,` `.`     | `〈〉` `《》`       | 文脈에 따라 하나 |
| `traditional`          | `、` `。`   | `「」` `『』`       | 除去             |
| `traditional-vertical` | `︑` `︒`   | `﹁﹂` `﹃﹄`       | 除去             |

세로쓰기 프로파일은 明示的인 縱書 文字를 出力합니다. CSS의 쓰기 方向이나 폰트는
設定하지 않습니다.


프로파일 選擇
-------------

~~~~ rust
use gukhanmun::{Builder, PunctuationProfile};

let converter = Builder::new()
    .no_bundled_dictionaries()
    .punctuation(PunctuationProfile::modern())
    .build()?;
assert_eq!(converter.convert_text_to_string("첫째、둘째。")?, "첫째, 둘째.");
~~~~

두 JavaScript 바인딩 모두 같은 옵션을 받습니다:

~~~~ typescript
import { load } from "@gukhanmun/wasm"; // 또는 @gukhanmun/napi
const converter = await load({ punctuation: "traditional" });
converter.convert("첫째, 둘째."); // "첫째、둘째。"
~~~~

CLI에서는 `--punctuation modern`을 使用합니다. 커스텀 JSON 設定은
`--punctuation-profile profile.json`으로 읽습니다. 두 플래그는 함께 쓸 수
없습니다.


커스텀 設定
-----------

`base`는 省略하면 `preserve`입니다. 省略한 役割은 基底 프로파일을 相續하고,
明示的인 `null`은 그 役割의 入力을 維持합니다. JavaScript의 `undefined` 屬性은
바인딩에 傳達하기 前에 省略합니다.

~~~~ json
{
  "base": "modern",
  "period": null,
  "comma": { "mark": "、", "after": "remove" },
  "single": { "open": "︿", "close": "﹀" }
}
~~~~

이 객체를 JavaScript의 `punctuation`으로 傳達하거나 CLI用 파일로 貯藏합니다.
Rust에서는 `PunctuationProfile`의 公開 필드를 `StopStyle`, `DelimiterStyle`로
設定합니다. `serde` 機能을 켜면 같은 設定을 `PunctuationConfig`로 읽고
`resolve()`로 檢證할 수 있습니다. 直接 만든 프로파일도 `builder.build()`에서
檢證합니다.

各 符號는 Unicode 스칼라 하나여야 합니다. 여는 符號는 Unicode 17.0의 Ps/Pi,
닫는 符號는 Pe/Pf 分類여야 하며, 쉼표와 마침표는 나머지 句讀點/記號 分類를
받습니다. 서로 다른 役割의 出力이나 다른 役割의 入力 符號와 衝突하면 拒否합니다.
空白, 制御 文字, 글자, 數字, `<`, `>`, `&`, U+FFFD도 拒否합니다. 未知의
프로파일名, 필드, 空白 政策은 오류입니다. 固定된 入力 集合 밖의 커스텀 出力은
다음 變換에서 새 入力 樣式으로 推論하지 않습니다.

`after`는 `preserve`, `remove`, `single`입니다. U+0020만 바꾸며 줄바꿈, 탭,
줄바꿈 없는 空白, 全角 空白은 維持합니다. `single`은 普通 內容과 여는 符號 앞에
空白 하나를 두고, 닫는 符號, 블록/줄 境界, EOF 앞에서는 除去합니다. 曖昧한
記號나 保存 境界 앞에서는 旣存 空白을 維持하고 새 空白을 넣지 않습니다.
透明한 인라인 範圍를 건너뛰어 共通 外部 位置에 空白을 놓되, 構造 이벤트를
256個 넘게 保留하면 旣存 空白만 維持합니다.

`converter.options.punctuation`은 네 役割이 모두 있는 深層 凍結된 複寫本입니다.
保存하는 役割은 `null`입니다.


入力 判別과 保護 範圍
---------------------

어떤 出力 프로파일이든 同一한 쉼표/마침표 入力을 認識합니다. ASCII, 全角,
表意文字用, 縱書 形式을 包含합니다. 引用 符號는 `〈〉`, `《》`, `「」`,
`『』`, 그 縱書 形式과 U+2329/U+232A를 받지만, 數學用 U+27E8/U+27E9는
除外합니다. 各 符號를 獨立的으로 바꾸며 짝을 修復하지 않습니다. `<>` 같은
ASCII 代用 表記, 普通 따옴표, 대시, 줄임표는 推論하지 않습니다.

數字 사이의 ASCII/全角 쉼표와 마침표, 두 個 以上 連續된 마침표, Latin
文字/數字에 隣接한 ASCII/全角 마침표는 保護합니다. ASCII/全角 쉼표도 兩쪽이
英數字이거나, 앞이 英數字이고 뒤가 空白과 Latin 文字이면 保護합니다.
`Hello, world`, `1,000`, `3.14`, `example.com`은 維持하지만
`IMF, 세계은행`은 正規化할 수 있습니다. 大小文字를 區分하지 않는 `http://`,
`https://`, `mailto:`, `www.` 區間은 Unicode 空白이나 構造/註釋 境界까지
保護합니다.


HTML과 Markdown
---------------

HTML 텍스트에서는 WHATWG 이름式/十進/十六進 文字 參照를 認識합니다. 許容된
세미콜론 省略과 數値 置換 規則도 適用합니다. 바꾸지 않는 參照는 原文 表記를
維持합니다. 正規化 對象 符號와 空白만 리터럴과 같이 處理하며, 한 번만
디코딩하므로 `&amp;period;`는 普通 텍스트, `&lt;script&gt;`는 마크업이 아닙니다.
`&lang;`/`&rang;`은 數學用이므로 對象 밖입니다. 屬性, 코드, 外國語 範圍 等의
保存 領域은 維持합니다. 原文의 ruby와 未知의 인라인 要素는 空白 推論의 境界로
다룹니다. 새로 出力하는 `;`, `#`, `!`, `?`, `/`는 隣接 原文과 文字 參照를
再構成하지 않도록 數値 參照로 이스케이프합니다.

엔티티 디코딩은 正規化 단계에서만 使用합니다. 엔티티로 쓴 漢字나 句讀點을
讀音 엔진에 다시 傳達하지 않으므로, 表記만 다른 入力의 讀音 選擇까지 같다고
保證하지 않습니다. 未完成 參照는 텍스트 청크 사이에서 維持합니다. 數値 參照의
先行 0은 길이 制限이 없으므로, 處理 時間은 線形이지만 原文 保存에 必要한
버퍼는 길어질 수 있습니다.

Markdown 텍스트는 리더가 이미 디코딩했으므로 다시 디코딩하지 않습니다.
生 HTML과 이미지는 保護합니다. 正規化가 켜지면 普通 텍스트의 ASCII 句讀點과
soft break 前 末尾 空白을 數値 參照로 出力해, 새 목록/링크/hard break 等을
만들지 않습니다. 强調는 로컬 `<em>`, `<strong>`, `<del>` 태그로 出力해
符號 變換 後에도 意味를 維持합니다. 自動 링크의 라벨이 바뀌면 原來 目的地와
제목을 維持하는 普通 링크로 내보냅니다. 따라서 Markdown 原文 表記는 달라질 수
있지만 렌더링 構造는 保存합니다.

이스케이프한 表記는 호스팅 사이트의 URL·멘션·이슈 參照 自動 링크를 끌 수
있습니다. 目的地를 維持해야 한다면 明示的 Markdown 링크나 CommonMark 自動
링크를 使用합니다. 이러한 호스트 固有 擴張은 파서가 支援하는 GFM 範圍 밖입니다.


低水準 파이프라인
-----------------

`PunctuationNormalizer`는 註釋 政策 後, `Renderer` 前의 `OutputToken`을
받습니다. 上流의 `finish()`를 모두 비운 뒤 正規化의 `finish()`를 呼出합니다.
I/O flush는 EOF가 아닙니다. `Annotated` 內容과 `Verbatim`은 바꾸지 않습니다.

`Converter::convert_tokens()`는 리터럴 텍스트용입니다. HTML 토큰에는
`convert_tokens_with_codec(tokens, HtmlPunctuationTextCodec::new())`와
`converter.html_reader_options()`를 使用해야 짝 없는 닫는 태그도 保護합니다.
루트 없는 HTML 조각도 코덱이 必要합니다. Markdown 이터레이터를 直接
直列化한다면
`write_markdown_with_options(tokens, converter.markdown_write_options())`를
使用합니다. 버퍼式 便宜 메서드는 이 옵션들을 自動으로 選擇합니다.

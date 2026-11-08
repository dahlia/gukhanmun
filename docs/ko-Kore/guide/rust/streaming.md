---
title: 스트리밍 API
description: |-
  Gukhanmun의 이터레이터 API로 큰 文書를 漸進的으로 處理하기.
---

스트리밍 API
============

모든 `convert_*_to_string` 메서드는 返還하기 前에 全體 出力을 버퍼링합니다.
이터레이터 變種은 出力을 토큰 單位로 處理하게 해 주는데, 이는 큰 文書나 싱크에
漸進的으로 써야 할 때 有用합니다.


텍스트 이터레이터
-----------------

~~~~ rust
use gukhanmun::{Builder, Preset, write_plain_text};

let converter = Builder::with_preset(Preset::KoKr).build()?;

let output = write_plain_text(converter.convert_text_iter("漢字를 한글로"));
~~~~

`convert_text_iter`는 `impl Iterator<Item = RenderedToken>`을 返還합니다.
토큰을 손으로 이어 붙이지 말고 `write_plain_text`에 넘기십시오. 토큰 種類가
原文 텍스트와 倂記 텍스트를 區別하며, 그 區別을 안전한 出力으로 바꾸는 것이
라이터의 役割입니다.


HTML과 Markdown 이터레이터
--------------------------

~~~~ rust
use gukhanmun::{MarkdownVariant, html, markdown};

// HTML
let html = html::write_html_fragment(converter.convert_html_fragment_iter("<p>漢字</p>")?);

// Markdown
let source = markdown::write_markdown(
    converter.convert_markdown_iter("# 漢字", MarkdownVariant::Gfm),
)?;
~~~~

各 어댑터의 라이터가 그 포맷의 이스케이프 規則을 담당합니다. 特히 辭典
讀音은 `RenderedToken::Annotated` 토큰으로 到達하며, `write_html_fragment`는
이를 文字 데이터로 이스케이프하므로 讀音이 마크업이 될 수 없습니다.


形式 中立的 토큰 파이프라인
---------------------------

`convert_tokens`는 `InputToken`의 任意 이터레이터를 받아 `RenderedToken`의
이터레이터를 返還합니다.  이는 가장 低水準의 進入點이며, 使用者 定義 리더에서
미리 토큰化된 入力을 提供하게 해 줍니다:

~~~~ rust
use gukhanmun::InputToken;

let tokens: Vec<InputToken<&str>> = /* 使用者의 토크나이저 出力 */;
for rendered in converter.convert_tokens(tokens.into_iter()) {
    // 出力 포맷에 맞는 라이터로 直列化하십시오.
}
~~~~

使用者 定義 라이터는 `RenderedToken::Annotated`를 文字 그대로 直列化해야
합니다. `Text`는 이미 그 포맷에 適合한 原文 데이터이고, `Annotated`는 辭典
等의 倂記 來源에서 生成된 데이터라 任意 文字를 담을 수 있습니다.
`Annotated`를 이스케이프 없이 그대로 내보내면 GHSA-j87h-8h88-wm6g가
되살아납니다.


토큰 種類
---------

| 種類            | 說明                                                                      |
| --------------- | ------------------------------------------------------------------------- |
| `InputToken`    | 入力 單位: 漢字 連續, 한글 텍스트, 섞인 텍스트, 블록 境界, 또는 섹션 境界 |
| `OutputToken`   | 辭典 찾기와 倂記 以後의 單位: 倂記된 漢字, 平文 텍스트, 또는 境界 標識    |
| `RenderedToken` | 렌더링 以後의 出力 토큰: 原文 텍스트, 生成된 倂記 텍스트, 또는 境界       |


句 決定 確認
------------

原本 句에 따른 決定을 確認하려면 低水準 `Engine`을 使用합니다. 報告를
明示的으로 켜고, 完了된 決定은 `take_resolution_report()`로 비우며, EOF에서
처음 確定된 決定은 `finish_with_report()`로 받습니다:

~~~~ rust
use gukhanmun::{Engine, InputToken, PlainScopeData};

fn main() {
    let dictionary = gukhanmun::stdict::ko_kr();
    let mut engine = Engine::<PlainScopeData, _>::new(dictionary)
        .with_resolution_reporting();
    let mut tokens = engine.push_token(InputToken::Text("무조건 ".into()));
    tokens.extend(engine.push_token(InputToken::Text("降服".into())));
    let early_report = engine.take_resolution_report();
    let (tail, final_report) = engine.finish_with_report();
    tokens.extend(tail);
    for event in early_report.events.iter().chain(&final_report.events) {
        println!("{:?}: {:?}", event.outcome, event.chosen.form);
    }
}
~~~~

`ResolutionEvent`는 出處가 있는 規則 識別子, 남은 完全한 候補, 實際 選擇과
`Selected`·`Unresolved`·`Conflict`·`Limit`, 또는 句 制約이 없을 때의
`PolicyDefault` 結果를 記錄합니다. 報告의 選擇은 位置形 實現과 明示的 原文
讀音 處理 前의 決定을 뜻합니다. 有效한 原文 讀音이 나중에 最終 註解를 다시
檢證하여 덮어쓸 수 있습니다.

바이트 오프셋은
連續한 變換 可能 IR 텍스트 基準이며 原來 HTML/Markdown 바이트 位置가 아닙니다.
構造/保存 境界와 明示的 flush마다 0으로 다시 始作합니다.

句 制約 없는 出處
풀에서 助詞에 맞는 候補가 없거나 意味上 서로 다른 候補가 남으면
`PolicyDefault`를 記錄합니다. 이는 從來 政策의 選擇을 表示하며 사람이 檢討한
選好度를 再分類하지 않습니다. 意味上 唯一한 無制約 候補는 報告할 必要가
없습니다. 基本으로 報告를 끄며, 켠 境遇에는 文書 全體의 報告가 쌓이지 않도록
週期的으로 비웁니다.

`Limit`에서는 resolver가 集合을 列擧하지 않았으므로 `remaining`을 비워 둡니다.
`MAX_PHRASE_CANDIDATES`는 政策 基本값 報告를 包含한 追加 決定 作業을 邊마다
256個의 整列 候補로 制限합니다. 풀이 이를 넘으면 resolver는 從來 選擇을
維持하고 影響을 받는 規則/窓 全體의 根據를 保留합니다. 貯藏된 出處 풀을 잘라
내지는 않습니다. 풀 構成과 從來 政策 選擇은 이 追加 作業 限度 밖에 있으며,
엔진 全體의 上限은 아닙니다.

隣接한 普通 `Text` 청크는 句 文脈을 共有합니다. 스코프 여닫힘, 保存/verbatim
轉換이나 明示的 `flush()`는 그 흐름을 끝냅니다. 인라인 마크업, 保存/verbatim
轉換과 明示的 flush 自體는 單語 오른쪽 境界의 根據가 되지 않습니다. 規則에는
흐름 內의 實際 텍스트 根據, 어댑터가 報告한 블록 境界 또는 實際 EOF가
必要합니다. 흐름 內部의 完全한 規則은 適用할 수 있지만 境界를 넘지는 못합니다.
`finish()`와 `finish_with_report()`는
EOF를 確定하고, `flush()`는 EOF 助詞 根據를 提供하지 않습니다. 따라서 句
中間의 `flush()`는 나중 制約의 根據를 버릴 수 있습니다.

句 規則은 첫 漢字 앞의 한글 리터럴을 包含하여, 有限한 追加 꼬리와 이미 出力한
왼쪽 文脈을 保存합니다. 許容量은 最大 句 表面 길이와 境界/助詞 根據에 따라
計算합니다. 單語 最大 길이를 몰라도 經路와 句 文脈이 確定된 오래된 空白 區切
接頭部는 내보낼 수 있습니다. 未完 單語는 區切子나 EOF까지 保留할 수 있습니다.

이는 엔진 全體의 메모리 上限은 아닙니다. 未確定 래티스 經路, 最大 길이를
모르는 辭典, 未完 數字, 폴백 註解 묶음과 미들웨어가 더 많은 入力을 保留할 수
있습니다. `buffered_chars()`는 保留 텍스트만 세며, 이미 出力한 句 文脈은
別途로 保存합니다.

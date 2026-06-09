<picture>
  <source srcset="logo-square-white.svg" media="(prefers-color-scheme: dark)">
  <img src="logo-square.svg" width="75" height="75">
</picture>

Gukhanmun
=========

[![crates.io][crates.io badge]][crates.io]
[![JSR][JSR badge]][JSR]
[![npm][npm badge]][npm]
[![GitHub Actions][GitHub Actions badge]][GitHub Actions]
[![License: GPL-3.0-only][GPL badge]][GPL]
[![GitHub Sponsors][GitHub Sponsors badge]][GitHub Sponsors]

*다른 言語: [English](README.en.md) (英語).*

Gukhanmun은 國漢文混用體로 쓰인 韓國語 텍스트를 한글 專用 텍스트로 變換하는
Rust 라이브러리입니다. [Seonbi]의 後繼 프로젝트로서, 漢字 變換 파이프라인에
集中하면서 스트리밍 入出力·結合 可能한 辭典·라티스 基盤 分割·多樣한 出力
形式 等의 軸으로 擴張되었습니다. Rust 라이브러리·命令줄 道具·WebAssembly
패키지·네이티브 Node-API 애드온 形態로 提供됩니다.

[crates.io badge]: https://img.shields.io/crates/v/gukhanmun?logo=rust
[crates.io]: https://crates.io/crates/gukhanmun
[JSR badge]: https://jsr.io/badges/@gukhanmun/types
[JSR]: https://jsr.io/@gukhanmun
[npm badge]: https://img.shields.io/npm/v/@gukhanmun/types?logo=npm
[npm]: https://www.npmjs.com/package/@gukhanmun/types
[GitHub Actions badge]: https://github.com/dahlia/gukhanmun/actions/workflows/main.yaml/badge.svg
[GitHub Actions]: https://github.com/dahlia/gukhanmun/actions/workflows/main.yaml
[GPL badge]: https://img.shields.io/github/license/dahlia/gukhanmun
[GPL]: https://www.gnu.org/licenses/gpl-3.0.html
[GitHub Sponsors badge]: https://img.shields.io/github/sponsors/dahlia?logo=githubsponsors
[GitHub Sponsors]: https://github.com/sponsors/dahlia
[Seonbi]: https://github.com/dahlia/seonbi


機能
----

 -  라티스 分割로 最適 分割을 찾습니다. 貪慾的 最長 一致와 달리, 行事場所를
    行事場 + 所가 아니라 行事 + 場所로 正確하게 분립니다.
 -  結合 可能한 辭典: 메모리 內 맵·mmap 親和的 FST 파일·CDB 파일을 쓸 수
    있으며, `ChainDictionary`로 合成할 수 있습니다.
 -  《標準國語大辭典》과 《우리말샘》이 컴파일된 FST·CDB 形態로 內藏되어 있어
    別途의 다운로드가 必要 없습니다.
 -  純粹 텍스트·HTML 단편·Markdown 形式 어댑터를 提供합니다. 엔진은 形式
    中立的이며, 解析과 直列化는 어댑터가 담당합니다.
 -  다섯 가지 렌더링 모드: 한글 專用·한글(漢字) 括弧·漢字(한글) 括弧·루비
    마크업·選擇的 倂記를 곁들인 國漢文 原文.
 -  스트리밍 優先 設計: 엔진은 單一 漢字 變換 候補 範圍 內에서만 버퍼링하며,
    文書 全體를 메모리에 올리지 않습니다.
 -  大韓民國 正書法을 爲한 頭音法則을 폴백 讀音에 適用합니다. 辭典 項目은 이미
    正確한 讀音을 들고 있다고 假定합니다.
 -  핵심 크레이트(`gukhanmun-core`)는 `no_std` + `alloc`으로, 임베디드
    環境에서도 使用할 수 있습니다.
 -  JavaScript·TypeScript 바인딩을 두 가지 形態로 提供합니다: 브라우저·
    Deno·Node.js·Bun·에지 런타임에서 動作하는 WebAssembly 패키지와 서버
    사이드 高性能用 네이티브 Node-API 애드온.


設置
----

### 命令줄 道具

#### mise를 通해서

[mise]를 使用한다면, 미리 빌드된 바이너리를 命令 하나로 設置할 수 있습니다:

~~~~ sh
mise use -g aqua:dahlia/gukhanmun
~~~~

`-g` 플래그는 全域으로 設置합니다.  이를 省略하면 現在 프로젝트 디렉터리에서만
道具가 活性化됩니다.

#### Windows Package Manager(`winget`)를 通해서

Windows에서는 `winget`으로 設置할 수 있습니다:

~~~~ pwsh
winget install HongMinhee.Gukhanmun
~~~~

#### crates.io에서

Rust 툴체인이 設置되어 있다면, crates.io에서 設置합니다:

~~~~ sh
cargo install gukhanmun-cli gukhanmun-mkdict
~~~~

이 命令은 바이너리를 컴파일하여 *~/.cargo/bin/* 디렉터리에 둡니다.
그 디렉터리가 `PATH`에 있는지 確認하십시오.

#### 미리 빌드된 바이너리

Linux(x86\_64, aarch64), macOS(x86\_64, aarch64), Windows(x86\_64)用으로 미리
빌드된 바이너리가 GitHub의 各 릴리스에 添附되어 있습니다:

<https://github.com/dahlia/gukhanmun/releases>

自身의 플랫폼에 맞는 아카이브를 내려받아 풀고, `gukhanmun` 및 `gukhanmun-mkdict`
實行 파일을 `PATH` 上의 어딘가에 둡니다.

[mise]: https://mise.jdx.dev/

### Rust 라이브러리

*Cargo.toml*에 追加합니다:

~~~~ sh
cargo add gukhanmun-core
~~~~

必要에 따라 形式 어댑터와 辭典 백엔드를 追加합니다:

~~~~ sh
cargo add gukhanmun-html gukhanmun-markdown
cargo add gukhanmun-stdict gukhanmun-opendict
cargo add gukhanmun-fst gukhanmun-cdb
~~~~

### JavaScript/TypeScript 라이브러리

大部分의 JavaScript 環境의 境遇 WebAssembly 패키지를 設置하면 됩니다:

~~~~ sh
npm  add       @gukhanmun/wasm @gukhanmun/stdict-fst
pnpm add       @gukhanmun/wasm @gukhanmun/stdict-fst
yarn add       @gukhanmun/wasm @gukhanmun/stdict-fst
bun  add       @gukhanmun/wasm @gukhanmun/stdict-fst
deno add --jsr @gukhanmun/wasm @gukhanmun/stdict-fst
~~~~

또는, 더 나은 서버 사이드 性能을 爲해 네이티브 依存關係를 개의치 않는다면,
Node-API 패키지를 設置할 수도 있습니다:

~~~~ sh
npm  add     @gukhanmun/napi     @gukhanmun/stdict-fst
pnpm add     @gukhanmun/napi     @gukhanmun/stdict-fst
yarn add     @gukhanmun/napi     @gukhanmun/stdict-fst
bun  add     @gukhanmun/napi     @gukhanmun/stdict-fst
deno add npm:@gukhanmun/napi jsr:@gukhanmun/stdict-fst
~~~~


使用 例
-------

~~~~ sh
echo "漢字 北京 標識" | gukhanmun
# → 한자 베이징 표지

echo "漢字" | gukhanmun --rendering hangul-hanja-parens
# → 한자(漢字)

echo "<p>漢字</p>" | gukhanmun --format text/html
# → <p>한자</p>
~~~~

~~~~ rust
use gukhanmun_core::{MapDictionary, RenderMode, convert_plain_text};

let mut dict = MapDictionary::new();
dict.insert("漢字", "한자");

let output = convert_plain_text("漢字", &dict, RenderMode::HangulOnly);
assert_eq!(output, "한자");
~~~~

HTML·Markdown 어댑터·렌더링 모드·프리셋·JavaScript API를 包含한 全體 案內는
<https://gukhanmun.org/> 文書 사이트를 參照하십시오.


크레이트 構成
-------------

이 프로젝트는 Cargo 워크스페이스로 構成되며, 모든 크레이트가 同一한 버전을
共有합니다.

| 크레이트                                    | 說明                                                                                     |
| ------------------------------------------- | ---------------------------------------------------------------------------------------- |
| [`gukhanmun-core`][cr-core]                 | 形式 中立的 IR·엔진·辭典 트레이트·라티스 分割機·폴백 音譯機. `no_std` + `alloc`.         |
| [`gukhanmun-html`][cr-html]                 | HTML 단편 리더·라이터. `lang` 相續과 保存 對象 태그 處理를 포함하는 `HtmlScopeData`.     |
| [`gukhanmun-markdown`][cr-md]               | `pulldown-cmark` 基盤 Markdown 어댑터. 인라인 HTML은 `lang` 屬性 處理를 위해 再走査된다. |
| [`gukhanmun-fst`][cr-fst]                   | mmap 親和的 온-디스크 辭典을 爲한 FST 基盤 `HanjaDictionary` 具顯.                       |
| [`gukhanmun-cdb`][cr-cdb]                   | 監査 容易한 온-디스크 形式의 CDB-trie `HanjaDictionary` 具顯.                            |
| [`gukhanmun-stdict`][cr-stdict]             | 內藏 大韓民國 《標準國語大辭典》을 FST 바이트 配列로 提供.                               |
| [`gukhanmun-opendict`][cr-opendict]         | 內藏 《우리말샘》 辭典을 範疇別 FST 바이트 配列로 提供.                                  |
| [`gukhanmun-dict-extract`][cr-dict-extract] | 共有 辭典 덤프 抽出 헬퍼.                                                                |
| [`gukhanmun-mkdict`][cr-mkdict]             | TSV·CSV·JSON Lines 入力에서 FST·CDB 辭典 파일을 빌드하는 CLI 道具.                       |
| [`gukhanmun-cli`][cr-cli]                   | `gukhanmun` 命令줄 바이너리.                                                             |

[cr-core]: https://crates.io/crates/gukhanmun-core
[cr-html]: https://crates.io/crates/gukhanmun-html
[cr-md]: https://crates.io/crates/gukhanmun-markdown
[cr-fst]: https://crates.io/crates/gukhanmun-fst
[cr-cdb]: https://crates.io/crates/gukhanmun-cdb
[cr-stdict]: https://crates.io/crates/gukhanmun-stdict
[cr-opendict]: https://crates.io/crates/gukhanmun-opendict
[cr-dict-extract]: https://crates.io/crates/gukhanmun-dict-extract
[cr-mkdict]: https://crates.io/crates/gukhanmun-mkdict
[cr-cli]: https://crates.io/crates/gukhanmun-cli


npm/JSR 패키지 構成
-------------------

일곱 個의 JavaScript 패키지도 配布하며, 모두 Rust 크레이트와 同一한 버전을
共有합니다.

| 패키지                    | JSR                                | npm                                | 說明                                                                 |
| ------------------------- | ---------------------------------- | ---------------------------------- | -------------------------------------------------------------------- |
| `@gukhanmun/types`        | [JSR][jsr:@gukhanmun/types]        | [npm][npm:@gukhanmun/types]        | WASM·NAPI 패키지가 共有하는 TypeScript 型 宣言. 런타임 코드 없음.    |
| `@gukhanmun/wasm`         | [JSR][jsr:@gukhanmun/wasm]         | [npm][npm:@gukhanmun/wasm]         | WebAssembly 빌드. 브라우저·Deno·Node.js·Bun에서 動作.                |
| `@gukhanmun/napi`         |                                    | [npm][npm:@gukhanmun/napi]         | napi-rs 기반 네이티브 Node.js 애드온. 서버 사이드에서 WASM보다 빠름. |
| `@gukhanmun/stdict-fst`   | [JSR][jsr:@gukhanmun/stdict-fst]   | [npm][npm:@gukhanmun/stdict-fst]   | FST 形式으로 內藏된 《標準國語大辭典》.                              |
| `@gukhanmun/stdict-cdb`   | [JSR][jsr:@gukhanmun/stdict-cdb]   | [npm][npm:@gukhanmun/stdict-cdb]   | CDB 形式으로 內藏된 《標準國語大辭典》.                              |
| `@gukhanmun/opendict-fst` | [JSR][jsr:@gukhanmun/opendict-fst] | [npm][npm:@gukhanmun/opendict-fst] | FST 形式으로 內藏된 《우리말샘》 辭典 範疇.                          |
| `@gukhanmun/opendict-cdb` | [JSR][jsr:@gukhanmun/opendict-cdb] | [npm][npm:@gukhanmun/opendict-cdb] | CDB 形式으로 內藏된 《우리말샘》 辭典 範疇.                          |

[jsr:@gukhanmun/types]: https://jsr.io/@gukhanmun/types
[npm:@gukhanmun/types]: https://www.npmjs.com/package/@gukhanmun/types
[jsr:@gukhanmun/wasm]: https://jsr.io/@gukhanmun/wasm
[npm:@gukhanmun/wasm]: https://www.npmjs.com/package/@gukhanmun/wasm
[npm:@gukhanmun/napi]: https://www.npmjs.com/package/@gukhanmun/napi
[jsr:@gukhanmun/stdict-fst]: https://jsr.io/@gukhanmun/stdict-fst
[npm:@gukhanmun/stdict-fst]: https://www.npmjs.com/package/@gukhanmun/stdict-fst
[jsr:@gukhanmun/stdict-cdb]: https://jsr.io/@gukhanmun/stdict-cdb
[npm:@gukhanmun/stdict-cdb]: https://www.npmjs.com/package/@gukhanmun/stdict-cdb
[jsr:@gukhanmun/opendict-fst]: https://jsr.io/@gukhanmun/opendict-fst
[npm:@gukhanmun/opendict-fst]: https://www.npmjs.com/package/@gukhanmun/opendict-fst
[jsr:@gukhanmun/opendict-cdb]: https://jsr.io/@gukhanmun/opendict-cdb
[npm:@gukhanmun/opendict-cdb]: https://www.npmjs.com/package/@gukhanmun/opendict-cdb


設計 文書
---------

[*DESIGN.md*](./DESIGN.ko-Kore.md)에서 全體 構造를 살펴볼 수 있습니다:
中間 表現·라티스 分割 算法·辭典 트레이트 設計·미들웨어 體系·形式 어댑터 內部
構造.


라이선스
--------

GPL 3.0 하에 配布. [*LICENSE*](./LICENSE) 參照.

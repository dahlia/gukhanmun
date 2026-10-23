---
title: 辭典
description: |-
  內藏 辭典 使用과 CLI에서 使用者 定義 辭典을 불러오기.
---

辭典
====

Gukhanmun은 漢字의 한글 讀音을 찾기 爲하여 辭典을 使用합니다.  `ko-kr`
프리셋은 內藏된 《外來語 表記 用例》와 《標準國語大辭典》을 使用하고,
`ko-kp` 프리셋은
《우리말샘》 北韓語 分類를 使用합니다.


內藏 辭典
---------

프리셋이 고른 內藏 辭典은 自動으로 불러와집니다.  大部分의 韓國語 텍스트에는
追加 플래그가 必要하지 않습니다.

모든 內藏 辭典을 끄려면, 例를 들어 使用者 定義 辭典에 全的으로 依存하고
싶을 때는 `--no-bundled-dictionaries`를 넘깁니다:

~~~~ sh
gukhanmun --no-bundled-dictionaries -d my-dict.gukfst input.txt
~~~~

北韓語 表記를 基本값으로 쓰려면 `ko-kp` 프리셋을 使用합니다.  이 프리셋은
頭音法則을 끄고 《우리말샘》 北韓語 辭典을 內藏 辭典으로 使用합니다:

~~~~ sh
gukhanmun --preset ko-kp input.txt
~~~~


《外來語 表記 用例》
--------------------

`ko-kr` 프리셋은 國立國語院 《外來語 表記 用例》 辭典을 基本으로 包含합니다.
漢字로 적힌 外國 人名·地名 等을 韓國語 外來語 表記로 바꾸며, 例를 들어
`香港`을 `홍콩`으로 變換합니다:

~~~~ sh
echo '香港' | gukhanmun
# 홍콩
~~~~

다른 內藏 辭典을 維持하면서 外來語 表記 用例만 끄려면 `--no-loanwords`를
넘깁니다. `ko-kp` 프리셋은 이 辭典을 基本으로 끄며, `--loanwords`로
明示的으로 켤 수 있습니다. 使用者 定義 辭典, 外來語 表記 用例, 프리셋의 다른
辭典 順으로 優先합니다. `--loanwords`와 `--no-bundled-dictionaries`는 함께 쓸 수
없습니다.


使用者 定義 辭典
----------------

`-d`(또는 `--dictionary`)로 하나 以上의 使用者 定義 辭典을 提供합니다.  이
플래그는 反復할 수 있습니다:

~~~~ sh
gukhanmun -d legal.gukfst input.txt
gukhanmun -d legal.gukfst -d names.gukcdb input.txt
~~~~

Gukhanmun은 두 가지 바이너리 辭典 形式을 支援합니다:

| 形式 | 擴張子    | 찾기                | 備考                                            |
| ---- | --------- | ------------------- | ----------------------------------------------- |
| FST  | *.gukfst* | $O(\text{키 길이})$ | 래티스(lattice) 分割에 適合; 디스크에서 더 작음 |
| CDB  | *.gukcdb* | $O(1)$              | 더 單純한 配置; 손으로 監査하기 쉬움            |

같은 텍스트와 같은 길이로 一致할 때에는 나중에 넘긴 `-d` 引數가 앞서 넘긴
辭典과 모든 內藏 辭典보다 優先합니다. 基本값인 `--segmentation lattice`는
길이가 다른 重疊 一致에 대해 辭典으로 處理되는 文字가 더 많은 分割을
選擇하므로, 긴 內藏 辭典 項目이 짧은 使用者 辭典 項目보다 優先할 수 있습니다.


使用者 定義 辭典 構築
---------------------

*.gukfst*와 *.gukcdb* 파일은 컴파일된 産出物이며, 손으로 編輯하는 것이 아닙니다.
項目은 平文 텍스트 表로 作成하여 `gukhanmun-mkdict`로 컴파일합니다.

`gukhanmun-mkdict` 빌더는 [mise를 通한 設置](./install.md#mise를-通하여)이든
[미리 빌드된 아카이브 내려받기](./install.md#미리-빌드된-바이너리)든
`gukhanmun`과 함께 設置됩니다.  대신 crates.io에서 빌드했다면, 빌더도 같은
方式으로 設置합니다:

~~~~ sh
cargo install gukhanmun-mkdict
~~~~

項目을 `hanja` 키 列과 `hangul` 讀音 列을 가진 탭 區分 파일로 作成합니다:

~~~~ tsv
hanja	hangul
北京	베이징
學校	학교
~~~~

두 個의 選擇的 列이 렌더러가 各 項目을 어떻게 다룰지를 制御합니다:
`require_hanja`를 `true`로 設定하면 (區別이 必要한 同音異義語를 爲하여) 出處
漢字를 보이게 維持하고, `require_hangul`을 `true`로 設定하면 原 表記 렌더링
모드에서 한글 倂記를 強制합니다.

~~~~ tsv
hanja	hangul	require_hanja	require_hangul
北京	베이징	false	false
色깔論	색깔론	false	true
~~~~

表를 FST 辭典(基本 形式)으로 컴파일합니다:

~~~~ sh
gukhanmun-mkdict --output legal.gukfst legal.tsv
~~~~

대신 *.gukcdb* 파일을 만들려면 `--format cdb`를 넘깁니다.  여러 入力 파일을 줄
수 있으며, 順序대로 倂合됩니다; `--merge`는 重複 키를 어떻게 解決할지(`error`,
`first-wins`, `last-wins`)를 選擇합니다.  出力을 다시 열어 모든 項目이
往復(round-trip)됨을 確認하려면 `--validate`를, 出處나 라이선스 같은 來歷을
심으려면 `--metadata KEY=VAL`을 더합니다.

그런 다음 結果를 다른 使用者 定義 辭典과 마찬가지로 불러옵니다:

~~~~ sh
gukhanmun -d legal.gukfst input.txt
~~~~

CSV와 JSON Lines 入力도 받아들이며, 몇 가지 더 進步된 옵션이 있습니다.  全體
辭典 파일 形式 明細은 [內部 構造 섹션](/internals/dictionary-format)을
參照하십시오.

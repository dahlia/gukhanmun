---
title: 指示
description: |-
  Gukhanmun JavaScript 라이브러리에서의 漢字別 倂記 再定義.
---

指示
====

`directives` 옵션을 使用하면 特定 漢字 文字에 對해 倂記 標識를 덮어쓸 수
있습니다.


指示 인터페이스
---------------

~~~~ ts twoslash
interface Directives {
  requireHanja?:    string[];  // 出力에 漢字를 恒常 表示
  requireHangul?:   string[];  // 한글 讀音을 恒常 表示("original" 모드用)
  skipAnnotation?:  string[];  // 倂記를 全的으로 抑制
}
~~~~

各 配列은 標識를 덮어쓰려는 漢字 文字列을 담습니다:

~~~~ ts twoslash
import { load } from "@gukhanmun/wasm";
import { stdictFst } from "@gukhanmun/stdict-fst";

const g = await load({
  dictionaries: [await stdictFst()],
  directives: {
    requireHanja:   ["漢", "字"],
    requireHangul:  ["東"],
    skipAnnotation: ["中"],
  },
});
~~~~


렌더링 모드와의 結合
--------------------

指示는 活性 렌더링 모드와 相互作用합니다:

 -  `requireHanja`는 `"hangul-only"` 모드에서 가장 잘 드러나며, 漢字가 한글 讀音
    곁에 나타나도록 強制합니다.
 -  `requireHangul`은 `"original"` 모드에서 特定 文字의 한글 倂記를 強制하는 데
    有用합니다.
 -  `skipAnnotation`은 모드와 無關하게 一切의 倂記를 抑制합니다.

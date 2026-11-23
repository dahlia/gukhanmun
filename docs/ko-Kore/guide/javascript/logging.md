---
title: 로깅
description: >-
  應用 프로그램에서 LogTape를 設定해 Gukhanmun의 JavaScript 로그를 確認하는
  方法과 로그 分類, 使用 中斷 警告를 說明합니다.
---

두 JavaScript 바인딩 `@gukhanmun/napi`와 `@gukhanmun/wasm`은 [LogTape]를
씁니다. 基本的으로는 使用 中斷 警告(deprecation warning)를 包含해 로그를
出力하지 않습니다.
어떤 로그를 어디로 보낼지는 應用 프로그램에서 定합니다.

[LogTape]: https://logtape.org/


應用 프로그램에서 LogTape 設定하기
----------------------------------

`@logtape/logtape`를 應用 프로그램의 依存性으로 追加합니다. npm을 쓴다면
다음과 같이 設置합니다:

~~~~ sh
npm install @logtape/logtape
~~~~

應用 프로그램의 進入點에서 變換器를 쓰기 前에 `configure()`를 呼出합니다.
이미 LogTape를 設定하고 있다면 `configure()`를 다시 呼出하지 말고,
그 設定에 Gukhanmun의 로그 分類를 追加합니다.

~~~~ typescript
import { configure, getConsoleSink } from "@logtape/logtape";

await configure({
  sinks: { console: getConsoleSink() },
  loggers: [
    { category: ["gukhanmun"], lowestLevel: "warning", sinks: ["console"] },
  ],
});
~~~~

이 設定은 Gukhanmun의 警告 以上 等級의 로그를 콘솔에 出力합니다.
다른 出力先이나 필터를 設定하려면 [LogTape 設定 文書]를 보십시오.

[LogTape 設定 文書]: https://logtape.org/manual/config


로그 分類
---------

LogTape의 로그 分類에는 階層이 있습니다. `["gukhanmun"]`을 設定하면
Gukhanmun의 모든 下位 分類에 適用됩니다. 特定 分類만 設定할 수도 있습니다:

 -  `["gukhanmun", "format"]`: `convert()`나 `stream()`에 더는 使用을 勸하지
    않는 形式 引數를 넘겼을 때 남는 警告입니다. 두 바인딩 모두 現在는
    이 分類에만 로그를 남깁니다.


使用 中斷 警告
--------------

使用 中斷 勸告(deprecation)는 아직 쓸 수 있지만 더는 使用을 勸하지 않는다는
뜻입니다. 以前 形式 文字列과 Markdown 客體는 Gukhanmun 1.0에서 除去합니다.
이 값을 `convert()`나 `stream()`에 넘기면
`["gukhanmun", "format"]` 分類에 `warning` 等級의 로그가 남습니다.
警告마다 바꿔 쓸 미디어 타입을 알려 줍니다. 여러 分類에서 使用 中斷 警告만
골라 보려면 로그의 `kind: "deprecation"` 屬性을 쓰십시오.

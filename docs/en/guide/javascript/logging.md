---
title: Logging
description: >-
  Configure LogTape in your application to see Gukhanmun's JavaScript logs,
  including deprecation warnings for legacy formats.
---

Both JavaScript bindings, `@gukhanmun/napi` and `@gukhanmun/wasm`, use
[LogTape]. By default, they print no logs, including deprecation warnings.
LogTape lets the application decide what to record and where to send it.

[LogTape]: https://logtape.org/


Configure LogTape in your application
-------------------------------------

Add `@logtape/logtape` as an application dependency. For example, with npm:

~~~~ sh
npm install @logtape/logtape
~~~~

In your application's entry point, call `configure()` before using the
converter. If the application already configures LogTape, add the Gukhanmun
category to that configuration instead of calling `configure()` again.

~~~~ typescript
import { configure, getConsoleSink } from "@logtape/logtape";

await configure({
  sinks: { console: getConsoleSink() },
  loggers: [
    { category: ["gukhanmun"], lowestLevel: "warning", sinks: ["console"] },
  ],
});
~~~~

This sends Gukhanmun warnings and higher-severity records to the console.
See the [LogTape configuration guide] for other destinations and filters.

[LogTape configuration guide]: https://logtape.org/manual/config


Log categories
--------------

LogTape categories are hierarchical. Set `["gukhanmun"]` to cover all Gukhanmun
categories, or select a specific category to limit the output:

 -  `["gukhanmun", "format"]`: Warnings about deprecated format arguments to
    `convert()` and `stream()`. This is the only category the bindings
    currently log to.


Deprecation warnings
--------------------

The legacy format strings and Markdown objects are deprecated and will be
removed in Gukhanmun 1.0. When you pass one to `convert()` or `stream()`,
Gukhanmun logs a `warning` in `["gukhanmun", "format"]`. Each warning names
the replacement media type. To filter deprecation warnings across categories,
match the structured property `kind: "deprecation"`.

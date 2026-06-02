---
pageType: home

hero:
  name: Gukhanmun
  tagline: 國漢文混用體 韓國語를 한글 專用 텍스트로 變換하는 Rust/JavaScript 라이브러리
  actions:
  - theme: brand
    text: 紹介
    link: ./guide/intro
  - theme: alt
    text: CLI
    link: ./guide/cli/install
  - theme: alt
    text: Rust
    link: ./guide/rust/install
  - theme: alt
    text: JavaScript
    link: ./guide/javascript/install
  image:
    src:
      light: /logo.svg
      dark: /logo-dark.svg
    alt: Logo
features:
- title: 漢字-한글 變換
  details: 함께 提供되는 《標準國語大辭典》을 利用하여, 漢字가 섞인 國漢文混用體 韓國語 텍스트를 한글 專用 出力으로 變換합니다.
  icon: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M7.5 21 3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5"/></svg>'
  link: ./guide/cli/start
- title: 多樣한 出力 形式
  details: 純粹 텍스트, HTML 斷片, Markdown(CommonMark과 GFM)을 處理합니다. 한글 專用, 括弧 倂記, 루비 마크업 렌더링 모드를 支援합니다.
  icon: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z"/></svg>'
  link: ./guide/cli/rendering
- title: 使用者 定義 辭典
  details: 專門 用語를 다루기 爲하여, 함께 提供되는 辭典과 더불어 FST나 CDB 形式의 分野別 語彙를 불러올 수 있습니다.
  icon: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 6.042A8.967 8.967 0 0 0 6 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 0 1 6 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 0 1 6-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0 0 18 18a8.967 8.967 0 0 0-6 2.292m0-14.25v14.25"/></svg>'
  link: ./guide/cli/dictionary
- title: 漢字別 指示
  details: 特定 文字나 glob 패턴에 對하여, 인라인으로 또는 指示 파일을 通하여 倂記를 強制하거나 抑制합니다.
  icon: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10.5 6h9.75M10.5 6a1.5 1.5 0 1 1-3 0m3 0a1.5 1.5 0 1 0-3 0M3.75 6H7.5m3 12h9.75m-9.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-3.75 0H7.5m9-6h3.75m-3.75 0a1.5 1.5 0 0 1-3 0m3 0a1.5 1.5 0 0 0-3 0m-9.75 0h9.75"/></svg>'
  link: ./guide/cli/directives
- title: CLI, Rust, JavaScript
  details: 命令줄에서 使用하거나, Rust 크레이트에 內藏하거나, WebAssembly 또는 네이티브 Node.js 애드온을 通하여 브라우저·Node.js·Deno·Bun에서 實行합니다.
  icon: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="m6.75 7.5 3 2.25-3 2.25m4.5 0h3m-9 8.25h13.5A2.25 2.25 0 0 0 21 18V6a2.25 2.25 0 0 0-2.25-2.25H5.25A2.25 2.25 0 0 0 3 6v12a2.25 2.25 0 0 0 2.25 2.25Z"/></svg>'
  link: ./guide/javascript/start
- title: 스트리밍 API
  details: JavaScript의 <code>TransformStream</code> 境界面이나 Rust의 이터레이터 API로, 任意의 큰 文書를 청크 單位로 處理합니다.
  icon: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182m0-4.991v4.99"/></svg>'
  link: ./guide/javascript/streaming
---


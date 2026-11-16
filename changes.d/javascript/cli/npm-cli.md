---
links:
  '#71': https://github.com/dahlia/gukhanmun/issues/71
  '#77': https://github.com/dahlia/gukhanmun/pull/77
---
 -  Added the dictionary-bundled `gukhanmun` converter and the
    `gukhanmun-mkdict` FST/CDB dictionary builder through
    `npm install -g @gukhanmun/cli`, without Rust or installation
    scripts. Node.js 22 or later and optional dependencies are required.
    The package includes five-shell completions and supports macOS,
    Windows and Linux on x64/arm64. With two commands installed, select either
    explicitly with `npx --package @gukhanmun/cli gukhanmun` or
    `npx --package @gukhanmun/cli gukhanmun-mkdict`.
    `gukhanmun --print-completion-path SHELL` locates a packaged completion
    script.
    Development builds are available through `@dev`.
    On first publication, npm may also assign `latest` to the initial
    development version; later development publications leave `latest`
    unchanged. [[#71], [#77]]

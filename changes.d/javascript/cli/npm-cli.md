---
links:
  '#71': https://github.com/dahlia/gukhanmun/issues/71
  '#77': https://github.com/dahlia/gukhanmun/pull/77
---
 -  Added the dictionary-bundled CLI through `npm install -g @gukhanmun/cli`
    and `npx --package @gukhanmun/cli gukhanmun`, without Rust or installation
    scripts. The package includes five-shell completions, supports macOS,
    Windows and Linux on x64/arm64, and offers development builds through
    `@dev`. On first publication, npm may also assign `latest` to the initial
    development version; later development publications leave `latest`
    unchanged. [[#71], [#77]]
 -  Added `gukhanmun-mkdict` alongside the converter to build FST and CDB
    dictionaries. With two commands installed, select either explicitly with
    `npx --package @gukhanmun/cli gukhanmun` or
    `npx --package @gukhanmun/cli gukhanmun-mkdict`. [[#71], [#77]]

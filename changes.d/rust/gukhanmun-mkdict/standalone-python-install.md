---
links:
  '#71': https://github.com/dahlia/gukhanmun/issues/71
  '#72': https://github.com/dahlia/gukhanmun/issues/72
  '#76': https://github.com/dahlia/gukhanmun/pull/76
  '#77': https://github.com/dahlia/gukhanmun/pull/77
---
 -  Added `gukhanmun-mkdict` to the standalone PyPI wheels. Install it together
    with `gukhanmun` using `pipx install gukhanmun-cli` or
    `uv tool install gukhanmun-cli`, without compiling Rust. [[#72], [#76]]

 -  Added `gukhanmun-mkdict` to `@gukhanmun/cli` on npm for macOS,
    Windows and Linux x64/arm64. Install with
    `npm install -g @gukhanmun/cli` or run
    `npx --package @gukhanmun/cli gukhanmun-mkdict`, without Rust or
    installation scripts. Node.js 22 or later and optional dependencies are
    required. [[#71], [#77]]

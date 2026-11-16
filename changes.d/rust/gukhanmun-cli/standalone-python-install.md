---
links:
  '#71': https://github.com/dahlia/gukhanmun/issues/71
  '#72': https://github.com/dahlia/gukhanmun/issues/72
  '#76': https://github.com/dahlia/gukhanmun/pull/76
  '#77': https://github.com/dahlia/gukhanmun/pull/77
---
 -  Added standalone PyPI wheels for installing the CLI with
    `pipx install gukhanmun-cli` or `uv tool install gukhanmun-cli`, without
    compiling Rust. The wheels include `gukhanmun-mkdict`, the bundled
    dictionaries and version-matched shell completions. [[#72], [#76]]

 -  Added installation through `npm install -g @gukhanmun/cli` and
    `npx --package @gukhanmun/cli gukhanmun` on macOS, Windows and Linux
    x64/arm64, including `gukhanmun-mkdict`, bundled dictionaries and shell
    completions. Node.js 22 or later and optional dependencies are required;
    Rust and installation scripts are not. [[#71], [#77]]

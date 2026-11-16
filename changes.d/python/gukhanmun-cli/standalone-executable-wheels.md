---
links:
  '#72': https://github.com/dahlia/gukhanmun/issues/72
  '#76': https://github.com/dahlia/gukhanmun/pull/76
---
 -  Added standalone CLI wheels for Python 3.11 or later on Linux with
    glibc 2.28 or musl 1.2 or later, macOS 11 or later, and Windows,
    on x86\_64 and aarch64. Install with `pipx install gukhanmun-cli`
    or `uv tool install gukhanmun-cli`, without Rust or the Python library's
    dictionary packages. The wheels include the `gukhanmun` and
    `gukhanmun-mkdict` executables, bundled dictionaries and shell completion
    resources for `gukhanmun`; no source distribution or runtime executable
    download is used. [[#72], [#76]]
 -  Added opt-in development releases with PEP 440 versions. Install with
    `uv tool install --prerelease allow gukhanmun-cli` or
    `pipx install --pip-args='--pre' gukhanmun-cli`; ordinary installation
    prefers a compatible stable release when one exists. [[#72], [#76]]

---
links:
  '#15': https://github.com/dahlia/gukhanmun/issues/15
  '#51': https://github.com/dahlia/gukhanmun/issues/51
  '#58': https://github.com/dahlia/gukhanmun/pull/58
---
 -  Added a standalone `gukhanmun` source distribution. It builds with a
    Rust 1.95 or later toolchain and a PEP 517 frontend such as pip, without
    the repository checkout, Git metadata or any dictionary source data, and it
    leaves out the unused source data of the optional dictionary crates.
    [[#51], [#15], [#58]]

 -  Added `gukhanmun` wheels, so common platforms install without a Rust
    toolchain. One `abi3` wheel per platform supports CPython 3.11 and later on
    Linux with glibc 2.28 or later and with musl 1.2 or later (x86\_64 and
    aarch64), on macOS 11 or later (x86\_64 and arm64) and on Windows 10 or
    later (AMD64 and ARM64). Separate wheels support PyPy 3.11 and 3.12 (PyPy
    8.0) on the glibc Linux and macOS platforms and on Windows AMD64. Each
    wheel is installed and tested on its own platform and architecture. There
    are no wheels for free-threaded CPython, or for PyPy on musl or Windows
    ARM64. [[#51], [#15], [#58]]

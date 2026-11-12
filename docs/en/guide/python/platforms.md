---
title: Supported platforms
description: The platforms, interpreters and minimum versions that the Gukhanmun Python wheels support, and how to build the package from its source distribution.
---

Supported platforms
===================

The `gukhanmun` package comes as wheels for common platforms and as a source
distribution (sdist). A wheel installs without a Rust toolchain; any other
platform builds the package from its sdist.

The packages are not published on PyPI yet. Until they are, build a wheel
locally as described in
[*Python quick start*](./start.md#build-and-install-a-local-wheel).


Wheels
------

| Platform      | Architectures    | Minimum version               | CPython        | PyPy       |
| ------------- | ---------------- | ----------------------------- | -------------- | ---------- |
| Linux (glibc) | x86\_64, aarch64 | glibc 2.28 (`manylinux_2_28`) | 3.11 and later | 3.11, 3.12 |
| Linux (musl)  | x86\_64, aarch64 | musl 1.2 (`musllinux_1_2`)    | 3.11 and later |            |
| macOS         | x86\_64, arm64   | macOS 11                      | 3.11 and later | 3.11, 3.12 |
| Windows       | AMD64            | Windows 10                    | 3.11 and later | 3.11, 3.12 |
| Windows       | ARM64            | Windows 10                    | 3.11 and later |            |

For CPython, each platform has one wheel built against the stable ABI
(`abi3`) of Python 3.11, which also runs on every later CPython version. Each
wheel is installed and tested on its own operating system and architecture,
with the oldest and newest supported CPython. The glibc wheels are also run on
glibc 2.28 itself, and the musl wheels on a musl system.

Windows 10 is the minimum of the Rust toolchain that builds the extension; the
Python interpreter you use must support your Windows version as well. The
extension uses the Visual C++ runtime that CPython itself ships.

### PyPy

The PyPy wheels are built for PyPy 8.0, in its Python 3.11 and Python 3.12
versions. Unlike the CPython wheels, a PyPy wheel matches only the PyPy ABI it
was built for (`pypy311_pp80` and `pypy312_pp80`). A PyPy release with a
different ABI builds the package from its sdist.

PyPy is not available for musl or for Windows ARM64, so these platforms have no
PyPy wheels.

### Unsupported interpreters

Free-threaded CPython builds, such as `python3.14t`, are not supported. The
`abi3` wheels do not cover them, and no free-threaded wheels are built or
tested. Neither are 32-bit platforms.


Building from the source distribution
-------------------------------------

On a platform without a wheel, or when you ask for one explicitly, pip builds
the package from its sdist. Until the packages are published, build the sdist
in a checkout and install that archive, replacing `0.4.0` with the version you
built:

~~~~ sh
mise run py-sdist
pip install target/python/sdist/gukhanmun-0.4.0.tar.gz
~~~~

Once the packages are on PyPI, `pip install --no-binary gukhanmun gukhanmun`
builds the published sdist instead.

The build needs:

 -  Rust 1.95 or later, for example installed with [rustup];
 -  the platform's C toolchain and linker: the Visual Studio Build Tools on
    Windows, the Xcode Command Line Tools on macOS, or a C compiler on Linux;
 -  network access to crates.io, from which Cargo downloads the Rust
    dependencies at the versions in the archive's lockfile.

pip installs the build backend, maturin, by itself. The sdist contains every
source file the build needs, so it does not need a Gukhanmun checkout, Git or
any dictionary source data. The dictionary data distributions are pure Python
packages with platform-independent wheels, and their sdists already contain
the data.

[rustup]: https://rustup.rs/

---
title: "Installation"
description: |-
  How to install the Gukhanmun command-line tool.
---

Installation
============

Gukhanmun is distributed as a single binary with no runtime dependencies.


Via mise
--------

If you use [mise], install a prebuilt binary with a single command:

~~~~ sh
mise use -g aqua:dahlia/gukhanmun
~~~~

The `-g` flag installs it globally.  Omit it to activate the tool only in the
current project directory.

[mise]: https://mise.jdx.dev/


From crates.io
--------------

If you have a Rust toolchain installed, install from crates.io:

~~~~ sh
cargo install gukhanmun-cli gukhanmun-mkdict
~~~~

This compiles the binaries and places them in *~/.cargo/bin/*.  Make sure that
directory is on your `PATH`.


Prebuilt binaries
-----------------

Prebuilt binaries for Linux (x86\_64, aarch64), macOS (x86\_64, aarch64), and
Windows (x86\_64) are attached to each release on GitHub:

<https://github.com/dahlia/gukhanmun/releases>

Download the archive for your platform, extract it, and place the `gukhanmun`
binary somewhere on your `PATH`.


The `gukhanmun-mkdict` companion
--------------------------------

The mise install and the prebuilt archives also include `gukhanmun-mkdict`, the
tool for compiling [custom dictionaries](/guide/cli/dictionary).  No separate
installation step is needed.


Verify the installation
-----------------------

~~~~ sh
gukhanmun --help
gukhanmun-mkdict --help
~~~~

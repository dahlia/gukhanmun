Gukhanmun commands
==================

`gukhanmun-cli` installs the standalone `gukhanmun` converter, its built-in
Korean dictionaries and the `gukhanmun-mkdict` custom dictionary builder.
It needs Python 3.11 or newer, but does not depend on the
`gukhanmun` Python library or its optional data distributions.

Install a prebuilt wheel with `pip install gukhanmun-cli`, or use an isolated
tool environment with `pipx install gukhanmun-cli` or
`uv tool install gukhanmun-cli`. Installation and execution do not download
native payloads. Source and editable builds are deliberately unavailable.

Run `gukhanmun --help` for conversion options. The command accepts files or
standard input. `gukhanmun --generate-completion bash` emits a completion
script; `zsh`, `fish`, `powershell`, and `nushell` are also supported.
Identical completion scripts and their manifest are installed in the
`gukhanmun_cli/completions` package resource directory. Installing the wheel
does not edit shell startup files or global completion directories.

Run `gukhanmun-mkdict --help` for dictionary build options. For example,
`gukhanmun-mkdict --validate --output custom.gukfst custom.tsv` compiles a
tab-separated table with `hanja` and `hangul` columns. Load the result with
`gukhanmun --dictionary custom.gukfst`. Both commands are installed and removed
together. The bundled completion scripts are for `gukhanmun` only.

The package contains GPL-licensed software and separately licensed dictionary
data. The installed *licenses* directory retains the software license,
dictionary licenses, attribution, and *NOTICE.md*.

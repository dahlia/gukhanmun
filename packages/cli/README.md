Gukhanmun CLI for npm
=====================

Install the dictionary-bundled command-line converter without a Rust compiler:

~~~~ sh
npm install -g @gukhanmun/cli
gukhanmun --help
~~~~

Or run it without a global installation:

~~~~ sh
npx @gukhanmun/cli --disambiguation off
~~~~

Input comes from standard input when no filename is supplied. For example,
`汽車길 色깔論 東京` becomes `기찻길 색깔론 도쿄`. The package runs the existing
native CLI, not the Node-API binding. Node.js 22 or later is required. The six
optional executable packages cover macOS, Windows and Linux on x64/arm64. Linux
uses the same static musl executable on musl and compatible glibc hosts. Other
package managers and architectures are not verified.

Updates use `npm install -g @gukhanmun/cli@latest`. Development builds are
opt-in:

~~~~ sh
npm install -g @gukhanmun/cli@dev
npx @gukhanmun/cli@dev --help
~~~~

Installation works with `--ignore-scripts`. Executables are included in the
registry artifacts; neither installation nor execution downloads a binary. Keep
optional dependencies enabled. If one is missing, reinstall with:

~~~~ sh
npm install -g @gukhanmun/cli --include=optional
~~~~


Shell completions
-----------------

The package includes completion files for Bash, zsh, fish, PowerShell and
Nushell. Locate a file with `gukhanmun --print-completion-path SHELL`; this
launcher-only option is not listed in the native executable's help. It also
works when optional executables were omitted. The invocation must consist of
exactly that option and one shell name. To emit a script instead, use the native
`gukhanmun --generate-completion SHELL`.

Activate completions explicitly in the current shell:

~~~~ bash
source "$(gukhanmun --print-completion-path bash)"
~~~~

~~~~ zsh
autoload -Uz compinit
compinit
source "$(gukhanmun --print-completion-path zsh)"
compdef _gukhanmun gukhanmun
~~~~

~~~~ fish
source (gukhanmun --print-completion-path fish)
~~~~

~~~~ powershell
. (gukhanmun --print-completion-path powershell)
~~~~

Nushell's `source` requires a parse-time path. Run
`gukhanmun --print-completion-path nushell`, then use
`source "/the/printed/path"` with the literal result. Nushell then handles help
through its `extern` declaration; use `^gukhanmun` for the executable's own
argument parsing.

No command above edits startup files or installs shell configuration. To enable
completions in future sessions, add the appropriate command yourself. Run
`gukhanmun --print-completion-path SHELL` again after changing an npm prefix or
moving an installation.


Process behavior and licenses
-----------------------------

The launcher preserves arguments, working directory, environment, standard
streams and numeric exit statuses. On POSIX it forwards `SIGINT`, `SIGTERM` and
`SIGHUP`, and re-raises those signals only if the child actually died from one.
Other child signals use a numeric `128 + signal number` status. On Windows,
console interruption is delivered directly to the executable; POSIX signal
semantics are not available.

Software terms are in _LICENSE_. Executable packages also include _NOTICE.md_,
_ATTRIBUTION.loanwords.md_ and _ATTRIBUTION.opendict.md_. The bundled dictionary
data has separate terms, including CC BY-SA 2.0 KR for the _Standard Korean
Language Dictionary_ and _Open Korean Dictionary_. See those notices for sources
and modifications.

Development tarballs retain `X.Y.Z-dev.N+sha` in installed manifests and
`--version`. npm registry keys and dist-tags omit the `+sha` build metadata. The
packages remain exact-pinned to the same artifact version.

Full documentation: <https://gukhanmun.org/guide/cli/install>.

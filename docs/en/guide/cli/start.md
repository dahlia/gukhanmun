---
title: "Quick start"
description: |-
  Basic usage of the Gukhanmun command-line tool.
---

Quick start
===========

Gukhanmun reads text containing hanja and writes the same text with hanja
replaced by their hangul readings.


Converting a file
-----------------

Pass a file path as the positional argument:

~~~~ sh
gukhanmun input.txt
~~~~

Output goes to standard output by default.  Use `-o` to write to a file
instead:

~~~~ sh
gukhanmun -o output.txt input.txt
~~~~

To replace a file in-place, pass the same path for both input and output.
Gukhanmun writes to a temporary file first and replaces the original
atomically, so the original is never left in a partial state:

~~~~ sh
gukhanmun -o document.txt document.txt
~~~~


Reading from standard input
---------------------------

Omit the file argument to read from standard input:

~~~~ sh
echo "漢字를 한글로" | gukhanmun
# → 한자를 한글로
~~~~


Specifying the input format
---------------------------

Gukhanmun infers the format from the file extension:

| Extension          | Format     |
| ------------------ | ---------- |
| *.txt*, *(none)*   | Plain text |
| *.html*, *.htm*    | HTML       |
| *.md*, *.markdown* | Markdown   |

Override the detected format with `-f`:

~~~~ sh
gukhanmun -f text/plain input.html      # treat as plain text
gukhanmun -f text/html  snippet.txt     # treat as HTML
gukhanmun -f text/markdown article.txt  # treat as Markdown
~~~~

The Markdown format also accepts variant parameters:

~~~~ sh
gukhanmun -f "text/markdown; variant=GFM" post.md
~~~~


Verbose logging
---------------

Add `-v` to print debug information to standard error:

~~~~ sh
gukhanmun -v input.txt
~~~~


Generating shell completions
----------------------------

Generate a completion script from the installed CLI's own option definitions:

~~~~ sh
gukhanmun --generate-completion bash > gukhanmun.bash
~~~~

The shell names are `bash`, `zsh`, `fish`, `powershell` and `nushell`.
Generation writes only the script to standard output and exits without reading
input or loading dictionaries. Combining this option with a conversion option
or input file is a usage error. This includes `-o`, which selects conversion
output; use shell redirection to save the script. The script always completes
the public `gukhanmun` command, even when an npm or Python wrapper runs a
relocated executable.

Options, visible aliases, enum values and file arguments come from the same
clap definition used to parse conversions. PowerShell 7 is the tested
PowerShell baseline. The PowerShell generator also derives value candidates
from that definition. Shells differ in how they complete free-text values and
quote paths.

Loading the Nushell script declares an `extern` command. Nushell then handles
`gukhanmun --help` itself and rejects some attached short-option values such as
`-ooutput.txt`; use `-o output.txt`. Use `^gukhanmun` to bypass the declaration
when you need the executable's own help or argument parsing.

### Building a matching completion bundle

From a source checkout, run:

~~~~ sh
mise run completions
mise run completions-test
~~~~

The first task builds a host executable from the checkout with locked
dependencies, even when Cargo is configured for a cross target. It never runs
a target executable. The second task requires all five shells and checks syntax
and actual completion candidates. To check one shell after generation, run
`mise run completions-test -- --shell nushell`.

Run the full test in a Unix environment, such as Linux, macOS or WSL. The Bash
and zsh checks use Python's `pty` and `termios` modules, which are
unavailable on native Windows.

The bundle goes to *target/completions/*:

| Shell      | File              |
| ---------- | ----------------- |
| Bash       | *gukhanmun.bash*  |
| zsh        | *\_gukhanmun*     |
| fish       | *gukhanmun.fish*  |
| PowerShell | *\_gukhanmun.ps1* |
| Nushell    | *gukhanmun.nu*    |

*manifest.json* records the executable version, full source revision, whether
the working tree was dirty, and each script's SHA-256. Local edits do not
change the revision field, so check the dirty flag to tell whether the build
was clean. Pass `--output-dir PATH` after `mise run completions --` to choose a
different bundle directory.

Stable releases and eligible development builds include the same bundle under
*completions/* in every platform binary archive. CI stamps the artifact version
before generation and verifies the revision, version and hashes before
archiving it. Binary packages can use these files without a Rust compiler.
Use the bundle from the executable's exact release or development revision.
Package-specific installation paths and activation are handled by the package;
the generator does not modify shell startup files.

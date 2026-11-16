# Gukhanmun: Validates generated completions in their five native shells.
# Copyright (C) 2026  Hong Minhee
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

"""Exercise real shell completion engines, without loading user configuration."""

import argparse
import json
import os
from pathlib import Path
import select
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
import uuid

import generate as bundle

ROOT = Path(__file__).resolve().parents[2]
FILES = bundle.SHELL_FILES
EXECUTABLES = {"powershell": "pwsh", "nushell": "nu"}
SHELLS = set(FILES)
FIXTURES = ("sample.txt", "space name.gukfst", "-dash.txt", "ko-file.txt")
TIMEOUT = 20


def run(command, directory, env, *, input=None):
    result = subprocess.run(
        command, cwd=directory, env=env, input=input, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=TIMEOUT,
    )
    if result.returncode:
        raise RuntimeError(
            f"{command[0]} exited {result.returncode}:\n{result.stdout}{result.stderr}"
        )
    return result.stdout


def require(candidates, expected, context):
    missing = set(expected) - set(candidates)
    # An empty expectation probes ValueHint::Other: some engines return the
    # unchanged word, but must not suggest the fixture filenames.
    unwanted_files = not expected and any(
        value.replace("\\ ", " ").strip("'\"").removeprefix("./") in FIXTURES
        for value in candidates
    )
    if missing or unwanted_files:
        raise AssertionError(
            f"{context}: missing {sorted(missing)!r}; candidates={candidates!r}"
        )


def executable(shell):
    name = EXECUTABLES.get(shell, shell)
    found = shutil.which(name)
    if found and Path(found).parent.name == "shims":
        resolved = subprocess.run(
            ["mise", "which", name], text=True, capture_output=True, timeout=TIMEOUT,
        )
        if resolved.returncode == 0:
            found = resolved.stdout.strip()
        else:
            # An inactive mise shim must not hide a system-installed shell.
            search = os.pathsep.join(
                entry for entry in os.get_exec_path()
                if Path(entry).name != "shims"
            )
            found = shutil.which(name, path=search)
    if not found:
        raise RuntimeError(f"Required shell {name!r} is absent or inactive on PATH")
    return str(Path(found).absolute())


class Terminal:
    """A bounded PTY session; records contain controls absent from echoed input."""

    def __init__(self, command, directory, env):
        import pty
        import struct
        import termios
        import fcntl

        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.chdir(directory)
            os.execve(command[0], command, env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 240, 0, 0))
        self.pending = b""
        self.token = uuid.uuid4().hex
        self.ready = b"\x1eREADY" + self.token.encode() + b"\x1f"
        self.record = b"\x1eRESULT" + self.token.encode() + b":"

    def send(self, text):
        os.write(self.fd, text.encode())

    def until(self, marker):
        deadline = time.monotonic() + TIMEOUT
        while marker not in self.pending:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError(f"Shell PTY timed out awaiting {marker!r}: {self.pending!r}")
            if select.select([self.fd], [], [], remaining)[0]:
                try:
                    chunk = os.read(self.fd, 65536)
                except OSError as error:
                    raise RuntimeError(f"Shell PTY ended: {self.pending!r}") from error
                if not chunk:
                    raise RuntimeError(f"Shell PTY closed: {self.pending!r}")
                self.pending += chunk
        end = self.pending.index(marker) + len(marker)
        result, self.pending = self.pending[:end], self.pending[end:]
        return result

    def query(self, line, key="\t"):
        self.send(line + key)
        self.until(self.record)
        result = self.until(b"\x1f")[:-1].decode()
        self.send("\x15\n")
        self.until(self.ready)
        return result

    def close(self):
        try:
            os.kill(self.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        os.close(self.fd)
        os.waitpid(self.pid, 0)


def bash(binary, source, directory, env):
    run([binary, "--noprofile", "--norc", "-n", str(source)], directory, env)
    setup = f"source {shlex.quote(str(source))}\n"
    cases = [
        (["--pres"], ["--preset"]),
        (["--preset", "ko-"], ["ko-kr", "ko-kp"]),
        (["-p", "ko-"], ["ko-kr", "ko-kp"]),
        (["--numeral-strategy", "sma"], ["smart"]),
        (["--generate-completion", ""], SHELLS),
        (["--format", "spac"], []),
    ]
    for words, expected in cases:
        script = setup + "COMP_WORDS=(gukhanmun " + " ".join(map(shlex.quote, words)) + ")\n"
        script += f"COMP_CWORD={len(words)}\n"
        script += '_gukhanmun gukhanmun "${COMP_WORDS[COMP_CWORD]}" "${COMP_WORDS[COMP_CWORD-1]}"\n'
        script += 'printf "%s\\0" "${COMPREPLY[@]}"\n'
        candidates = run([binary, "--noprofile", "--norc"], directory, env, input=script).split("\0")
        require(candidates, expected, f"bash {words}")
    terminal = Terminal([binary, "--noprofile", "--norc", "-i"], directory, env)
    try:
        terminal.send(
            setup + f"PS1=$'\\036READY{terminal.token}\\037'; PS2=''; "
            f"_dump() {{ printf '\\036RESULT{terminal.token}:%s\\037' \"$READLINE_LINE\"; }}; "
            "bind -x '\"\\C-x\\C-g\":_dump'\n"
        )
        terminal.until(terminal.ready)
        for line, filename in [("gukhanmun --dictionary spac", "space name.gukfst"), ("gukhanmun sampl", "sample.txt")]:
            completed = terminal.query(line, "\t\x18\x07")
            require(shlex.split(completed), [filename], f"bash readline {line}")
    finally:
        terminal.close()


def zsh(binary, source, directory, env):
    run([binary, "-f", "-n", str(source)], directory, env)
    terminal = Terminal([binary, "-f", "-i"], directory, env)
    try:
        # compadd -A uses zsh's own prefix matching within the active widget.
        # Calls that request an output array are internal probes, not candidates.
        terminal.send(
            "autoload -Uz compinit; compinit -u -D; "
            f"source {shlex.quote(str(source))}; compdef _gukhanmun gukhanmun; "
            f"PROMPT=$'\\036READY{terminal.token}\\037'; RPROMPT=''; "
            "compadd() { local -a captured; "
            "if (( ${@[(I)-[AO]]} == 0 )); then "
            "builtin compadd -A captured \"$@\"; _captured+=(\"${captured[@]}\"); fi; "
            "builtin compadd \"$@\"; }; "
            "_probe() { _captured=(); zle expand-or-complete; "
            f"printf '\\036RESULT{terminal.token}:%s\\037' \"${{(pj:\\n:)_captured}}\"; }}; "
            "zle -N _probe; bindkey '^I' _probe\n"
        )
        terminal.until(terminal.ready)
        for line, expected in [
            ("gukhanmun --pres", ["--preset"]),
            ("gukhanmun --preset ko-", ["ko-kr", "ko-kp"]),
            ("gukhanmun -p ko-", ["ko-kr", "ko-kp"]),
            ("gukhanmun --numeral-strategy sma", ["smart"]),
            ("gukhanmun --generate-completion ", SHELLS),
            ("gukhanmun --format spac", []),
            ("gukhanmun --dictionary spac", ["space name.gukfst"]),
            ("gukhanmun sampl", ["sample.txt"]),
        ]:
            candidates = terminal.query(line).splitlines()
            require([shlex.split(value)[0] for value in candidates], expected, f"zsh {line}")
    finally:
        terminal.close()


def fish(binary, source, directory, env):
    run([binary, "--no-config", "--no-execute", str(source)], directory, env)
    for line, expected in [
        ("gukhanmun --pres", ["--preset"]),
        ("gukhanmun --preset ko-", ["ko-kr", "ko-kp"]),
        ("gukhanmun -p ko-", ["ko-kr", "ko-kp"]),
        ("gukhanmun --numeral-strategy sma", ["smart"]),
        ("gukhanmun --generate-completion ", SHELLS),
        ("gukhanmun --format spac", []),
        ("gukhanmun --dictionary spac", ["space name.gukfst"]),
        ("gukhanmun sampl", ["sample.txt"]),
    ]:
        output = run(
            [binary, "--no-config", "-c", f"source {shlex.quote(str(source))}; complete -C {shlex.quote(line)}"], directory, env,
        )
        require([value.split("\t")[0] for value in output.splitlines()], expected, f"fish {line}")


def powershell_filename(value):
    """Compare native filename candidates without platform-specific prefixes."""
    value = value.strip("'\"")
    if value.startswith(("./", ".\\")):
        return value[2:]
    return value




def normalize_powershell_candidate(value, *, filename=False):
    """Allow Windows relative prefixes only in native filename cases."""
    return powershell_filename(value) if filename else value.strip("'\"").removeprefix("./")


def powershell(binary, source, directory, env):
    quote = lambda text: "'" + str(text).replace("'", "''") + "'"
    setup = (
        "$ErrorActionPreference='Stop'; $tokens=$null; $errors=$null; "
        f"[void][System.Management.Automation.Language.Parser]::ParseFile({quote(source)},[ref]$tokens,[ref]$errors); "
        "if ($errors.Count) { throw ($errors | Out-String) }; "
        f". {quote(source)}; "
    )
    cases = [
        ("gukhanmun --pres", None, ["--preset"]),
        ("gukhanmun --preset ko-", None, ["ko-kr", "ko-kp"]),
        ("gukhanmun -p ko-", None, ["ko-kr", "ko-kp"]),
        ("gukhanmun --preset=ko-", None, ["--preset=ko-kr", "--preset=ko-kp"]),
        ("gukhanmun --preset ko-f", None, []),
        ("gukhanmun --preset ko-fZZ trailing.txt", len("gukhanmun --preset ko-f"), []),
        ("gukhanmun --preset=ko-f", None, []),
        ("gukhanmun -Spko-f", None, []),
        ("gukhanmun -Sp=ko-f", None, []),
        ("gukhanmun --preset ko- trailing.txt", len("gukhanmun --preset ko-"), ["ko-kr", "ko-kp"]),
        ("gukhanmun -s e", None, ["eager"]),
        ("gukhanmun -Sp ko-", None, ["ko-kr", "ko-kp"]),
        ("gukhanmun -Spko-", None, ["-Spko-kr", "-Spko-kp"]),
        ("gukhanmun -pko-", None, ["-pko-kr", "-pko-kp"]),
        ("gukhanmun -Sp=ko-", None, ["-Sp=ko-kr", "-Sp=ko-kp"]),
        ("gukhanmun -Sp=", None, ["-Sp=ko-kr", "-Sp=ko-kp"]),
        ("gukhanmun -p=ko-", None, ["-p=ko-kr", "-p=ko-kp"]),
        ("gukhanmun -S --pres", None, ["--preset"]),
        ("gukhanmun --numeral-strategy sma", None, ["smart"]),
        ("gukhanmun --generate-completion ", None, SHELLS),
        ("gukhanmun --require-hanja ko-f", None, []),
        ("gukhanmun --require-hanja ", None, []),
        ("gukhanmun --require-hanja=", None, []),
        ("gukhanmun --dictionary spac", None, ["space name.gukfst"]),
        ("gukhanmun --dictionary ko-f", None, ["ko-file.txt"]),
        ("gukhanmun --output=spac", None, ["space name.gukfst"]),
        ("gukhanmun sampl", None, ["sample.txt"]),
        ("gukhanmun ko-f", None, ["ko-file.txt"]),
        ("gukhanmun -- ko-f", None, ["ko-file.txt"]),
    ]
    for line, cursor, expected in cases:
        script = setup + f"$line={quote(line)}; $r=TabExpansion2 $line {cursor if cursor is not None else len(line)}; "
        script += (
            "ConvertTo-Json -Compress -InputObject @{ "
            "candidates=@($r.CompletionMatches | ForEach-Object { $_.CompletionText }); "
            "replacement=$line.Substring($r.ReplacementIndex, $r.ReplacementLength) }"
        )
        result = json.loads(run([binary, "-NoProfile", "-NonInteractive", "-Command", script], directory, env))
        candidates = result["candidates"]
        # Native filename completions preserve quoting and a host-specific prefix.
        filename = bool(expected) and all(value in FIXTURES for value in expected)
        normalized = [normalize_powershell_candidate(value, filename=filename) for value in candidates]
        for value in candidates:
            if "=" in value:
                normalized.append(normalize_powershell_candidate(value.split("=", 1)[1], filename=filename))
        require(normalized, expected, f"PowerShell {line} at {cursor}")
        if not expected:
            unchanged = result["replacement"] or " "
            if candidates != [unchanged]:
                raise AssertionError(f"PowerShell non-path completion changed input: {line!r}: {candidates!r}")
        if line == "gukhanmun -Sp ko-" and set(normalized) != {"ko-kr", "ko-kp"}:
            raise AssertionError(f"PowerShell short cluster offered non-preset candidates: {candidates!r}")


def nushell(binary, source, directory, env):
    prefix = f"use {json.dumps(str(source), ensure_ascii=False)} *\n"
    syntax = run([binary, "--no-config-file", "-c", f"nu-check {json.dumps(str(source), ensure_ascii=False)}"], directory, env).strip()
    if syntax != "true":
        raise AssertionError(f"Nushell nu-check returned {syntax!r}, expected true")
    for line, expected in [
        ("gukhanmun --pres", ["--preset"]),
        ("gukhanmun --preset ko-", ["ko-kr", "ko-kp"]),
        ("gukhanmun -p ko-", ["ko-kr", "ko-kp"]),
        ("gukhanmun --numeral-strategy sma", ["smart"]),
        ("gukhanmun --generate-completion ", SHELLS),
        ("gukhanmun --dictionary spac", ["space name.gukfst"]),
        ("gukhanmun sampl", ["sample.txt"]),
    ]:
        # Non-ASCII text makes a mistaken character cursor differ from byte offset.
        script = prefix + "# 한글 cursor offset\n" + line
        probe = directory / "probe.nu"
        probe.write_text(script, encoding="utf-8")
        output = run([binary, "--no-config-file", "--ide-complete", str(len(script.encode())), str(probe)], directory, env)
        data = json.loads(output)
        candidates = data["completions"]
        normalized = [value.strip("'\"`").removeprefix("./") for value in candidates]
        require(normalized, expected, f"Nushell {line}")
    # An external stub records argv and stdin: the extern must forward both.
    stub = directory / "bin" / "gukhanmun"
    stub.parent.mkdir()
    stub.write_text(
        f"#!{sys.executable}\nimport json,sys\nprint(json.dumps({{'args':sys.argv[1:], 'stdin':sys.stdin.read()}}))\n",
        encoding="utf-8",
    )
    stub.chmod(0o755)
    invocation_env = env | {"PATH": str(stub.parent) + os.pathsep + env["PATH"]}
    script = prefix + "'piped text' | gukhanmun 'space name.gukfst' -d first.gukfst -d second.gukfst --dueum --numeral-strategy smart"
    result = json.loads(run([binary, "--no-config-file", "-c", script], directory, invocation_env))
    if result["args"] != ["space name.gukfst", "--dictionary", "first.gukfst", "--dictionary", "second.gukfst", "--dueum", "--numeral-strategy", "smart"] or result["stdin"].rstrip("\n") != "piped text":
        raise AssertionError(f"Nushell extern changed invocation: {result!r}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", type=Path, default=ROOT / "target" / "completions")
    parser.add_argument("--shell", choices=FILES, action="append")
    parser.add_argument("--binary", type=Path, help="reuse a built CLI when checking regeneration")
    args = parser.parse_args()
    selected = list(dict.fromkeys(args.shell or FILES))
    binaries = {shell: executable(shell) for shell in selected}
    revision = bundle.run(["git", "rev-parse", "HEAD"]).decode("ascii").strip()
    with (ROOT / "Cargo.toml").open("rb") as stream:
        version = tomllib.load(stream)["workspace"]["package"]["version"]
    manifest = bundle.verify(args.dir, revision=revision, version=version)
    with tempfile.TemporaryDirectory(prefix="gukhanmun-regenerate-") as temporary:
        regenerated = Path(temporary)
        bundle.generate(regenerated, binary=args.binary, revision=revision, version=version, excluded_dir=args.dir)
        for filename in [*FILES.values(), "manifest.json"]:
            if (args.dir / filename).read_bytes() != (regenerated / filename).read_bytes():
                raise AssertionError(f"Completion regeneration changed {filename}")
    print(f"bundle: hashes, version {manifest['version']}, revision and deterministic regeneration passed")
    for shell in selected:
        source = (args.dir / FILES[shell]).resolve(strict=True)
        with tempfile.TemporaryDirectory(prefix=f"gukhanmun-{shell}-") as temporary:
            directory = Path(temporary)
            for name in FIXTURES:
                (directory / name).touch()
            # PATH is retained for shell helpers, while startup/config locations
            # and environment-injected startup scripts cannot escape the fixture.
            env = {key: value for key, value in os.environ.items() if key not in {
                "BASH_ENV", "ENV", "ZDOTDIR", "XDG_CONFIG_HOME", "NU_CONFIG_DIR",
                "FISH_FUNCTION_PATH", "FISH_COMPLETE_PATH", "PROMPT_COMMAND", "PSModulePath",
            }}
            env.update(HOME=temporary, ZDOTDIR=temporary, XDG_CONFIG_HOME=temporary,
                       TERM="xterm", LC_ALL="C.UTF-8", NO_COLOR="1")
            globals()[shell](binaries[shell], source, directory, env)
        print(f"{shell}: syntax, load, options, values, aliases and paths passed")


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, OSError, RuntimeError, subprocess.SubprocessError) as error:
        sys.exit(str(error))

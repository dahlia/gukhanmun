# Gukhanmun: Launches the bundled standalone command without Python bindings.
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

"""Launch the wheel's two commands without dictionary initialization in Python."""

import errno
from importlib.resources import files
import os
from pathlib import Path
import signal
import subprocess
import sys


def main():
    """Launch the bundled text converter."""
    return _launch("gukhanmun")


def mkdict_main():
    """Launch the bundled custom dictionary builder."""
    return _launch("gukhanmun-mkdict")


def _launch(command):
    """Replace the POSIX process, or share the Windows console and wait."""
    executable = Path(str(files(__package__).joinpath(
        "_bin", command + (".exe" if os.name == "nt" else "")
    )))
    try:
        # Flush before exec without changing stream encodings or inheritance.
        for stream in (sys.stdout, sys.stderr):
            if stream is not None:
                stream.flush()
        if os.name != "nt":
            # Python ignores these dispositions; a direct native invocation
            # receives the shell defaults. Rust makes its own SIGPIPE choice.
            for name in ("SIGPIPE", "SIGXFZ", "SIGXFSZ"):
                if hasattr(signal, name):
                    signal.signal(getattr(signal, name), signal.SIG_DFL)
            os.execv(executable, [command, *sys.argv[1:]])
        previous = {}
        try:
            # Do not use SIG_IGN: on Windows it can disable Ctrl-C in the
            # child. A Python no-op handler keeps parent spawn/wait/reap alive.
            for name in ("SIGINT", "SIGBREAK"):
                if hasattr(signal, name):
                    number = getattr(signal, name)
                    previous[number] = signal.signal(number, lambda *_: None)
            with subprocess.Popen([str(executable), *sys.argv[1:]]) as child:
                status = child.wait()
            # distlib's console shim passes sys.exit through the CRT's signed
            # integer exit path; preserve all 32 status bits without overflow.
            return status if status < 0x80000000 else status - 0x100000000
        finally:
            for number, handler in previous.items():
                signal.signal(number, handler)
    except OSError as error:
        status = 127 if error.errno == errno.ENOENT else 126
        if sys.stderr is not None:
            print(f"{command}: cannot launch bundled executable: {error}", file=sys.stderr)
        return status

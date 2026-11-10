# Gukhanmun: Builds synthetic FST/CDB fixtures for Python binding regressions.
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

"""Use the existing dictionary compiler; no official datasets are required."""

import argparse
import json
from pathlib import Path
import subprocess


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--inputs", type=Path)
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    inputs = arguments.inputs or root / "tests" / "python" / "fixtures"
    output = arguments.output or root / "target" / "python" / "fixtures"
    output.mkdir(parents=True, exist_ok=True)
    build = subprocess.run(
        ["cargo", "build", "--locked", "--release", "-p", "gukhanmun-mkdict",
         "--bin", "gukhanmun-mkdict", "--message-format=json-render-diagnostics"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    executable = None
    for line in build.stdout.splitlines():
        message = json.loads(line)
        if (message.get("reason") == "compiler-artifact"
                and message["target"]["name"] == "gukhanmun-mkdict"
                and "bin" in message["target"]["kind"]
                and message.get("executable") is not None):
            executable = message["executable"]
    if executable is None:
        raise RuntimeError("Cargo did not report the gukhanmun-mkdict executable")
    for name in ("plain", "reference", "loanword"):
        for dictionary_format in ("fst", "cdb"):
            command = [executable, "--validate", "--format", dictionary_format,
                       "--output", str(output / f"{name}.{dictionary_format}")]
            evidence = inputs / f"{name}.evidence.json"
            phrase = inputs / f"{name}.phrase.json"
            if evidence.exists():
                command += ["--evidence", str(evidence)]
            if phrase.exists():
                command += ["--phrase-evidence", str(phrase)]
            command.append(str(inputs / f"{name}.jsonl"))
            subprocess.run(command, cwd=root, check=True)


if __name__ == "__main__":
    main()

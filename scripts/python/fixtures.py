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
    for name in ("plain", "reference", "loanword"):
        for format in ("fst", "cdb"):
            command = ["cargo", "run", "--locked", "--release", "-p", "gukhanmun-mkdict", "--",
                       "--validate", "--format", format, "--output", str(output / f"{name}.{format}")]
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

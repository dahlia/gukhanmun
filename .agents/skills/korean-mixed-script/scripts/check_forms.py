#!/usr/bin/env python3
"""Report candidate character-form conflicts without changing the input."""

import argparse
import csv
import json
from pathlib import Path
import sys


def inspect(text: str, preferences: Path) -> list[dict]:
    alternatives = {}
    with preferences.open(encoding="utf-8", newline="") as stream:
        for row in csv.DictReader(stream, delimiter="\t"):
            for form in row["alternatives"].split():
                alternatives[form] = row["preferred"]
    findings = []
    for line_number, line in enumerate(text.splitlines(), 1):
        for column, char in enumerate(line, 1):
            if char in alternatives:
                findings.append(
                    {
                        "line": line_number,
                        "column": column,
                        "found": char,
                        "codepoint": f"U+{ord(char):04X}",
                        "preferred": alternatives[char],
                        "context": line[max(0, column - 13) : column + 12],
                    }
                )
    return findings


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", help="UTF-8 prose file, or - for standard input")
    args = parser.parse_args()
    text = (
        sys.stdin.read()
        if args.input == "-"
        else Path(args.input).read_text(encoding="utf-8")
    )
    preferences = Path(__file__).resolve().parents[1] / "references/form-preferences.tsv"
    findings = inspect(text, preferences)
    print(
        json.dumps(
            {
                "potential_conflicts": findings,
                "note": "Review context and lexical/name exceptions. No text was changed; absence of warnings is not a full orthography check.",
            },
            ensure_ascii=False,
            indent=2,
        )
    )


if __name__ == "__main__":
    main()

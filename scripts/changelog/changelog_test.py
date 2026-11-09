# Gukhanmun: Verifies historical carry and reserved changelog sections.
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

"""Run Sacho against disposable releases without touching the checkout."""

from collections import Counter
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest

from check import fragment_errors


PROJECT_ROOT = Path(os.environ.get("GUKHANMUN_PROJECT_ROOT", Path(__file__).resolve().parents[2]))
EMPTY_CHANGELOG = "Changelog\n=========\n\n<!-- sacho:unreleased:begin -->\n\n<!-- sacho:unreleased:end -->\n"
OLD_PATTERNS = '''
[[section-patterns]]
source = "crates/{name}"
id = "{name}"
directory = "crates/{name}"
[[section-patterns]]
source = "packages/{name}"
id = "@gukhanmun/{name}"
directory = "packages/{name}"
'''
NEW_PATTERNS = '''
[[section-patterns]]
source = "crates/{name}"
id = "{name} (Rust)"
title = "{name} (Rust)"
directory = "rust/{name}"
[[section-patterns]]
source = "packages/{name}"
id = "@gukhanmun/{name} (JavaScript/TypeScript)"
title = "@gukhanmun/{name} (JavaScript/TypeScript)"
directory = "javascript/{name}"
[[section-patterns]]
source = "python/{name}"
id = "{name} (Python)"
title = "{name} (Python)"
directory = "python/{name}"
[[section-maps]]
kind = "title"
pattern = true
from = "{name}"
to = "{name} (Rust)"
[[section-maps]]
kind = "title"
pattern = true
from = "@gukhanmun/{name}"
to = "@gukhanmun/{name} (JavaScript/TypeScript)"
'''
CONFIG = '''[changelog]
path = "CHANGES.md"
materialize = true
region-detection = "marker"
[fragments]
directory = "changes.d"
next-file = "next"
[links]
"#" = "https://github.com/dahlia/gukhanmun/issues/{n}"
[vcs]
preset = "none"
[check]
paths = []
'''
PYTHON_DISTRIBUTIONS = sorted([
    "gukhanmun",
    *(f"gukhanmun-{source}-{kind}" for source in ("stdict", "opendict", "loanwords") for kind in ("fst", "cdb")),
])


def headings(markdown: str) -> list[str]:
    """Extract component headings from the generated/released document."""
    return re.findall(r"^### (.+)$", markdown, re.MULTILINE)


def items(markdown: str) -> Counter[str]:
    """Compare complete top-level item text independently of hard wrapping."""
    result = []
    current = []
    for line in markdown.splitlines():
        match = re.match(r"^ -  (.*)", line)
        if match:
            if current:
                result.append(" ".join(current))
            current = [match[1]]
        elif current and line.startswith("    "):
            current.append(line.strip())
        elif current:
            result.append(" ".join(current))
            current = []
    if current:
        result.append(" ".join(current))
    return Counter(result)


def links(markdown: str) -> dict[str, str]:
    """Retain resolved reference destinations, including security advisories."""
    return dict(re.findall(r"^\[([^\]]+)\]: (.+)$", markdown, re.MULTILINE))


def release(markdown: str, version: str) -> str:
    """Read a frozen section without re-rendering its Markdown reference syntax."""
    match = re.search(r"^Version " + re.escape(version) + r"\n-+\n", markdown, re.MULTILINE)
    if match is None:
        raise AssertionError(f"Missing release {version}")
    following = re.search(r"^Version [^\n]+\n-+\n", markdown[match.end():], re.MULTILINE)
    return markdown[match.start():match.end() + following.start()] if following else markdown[match.start():]


def section_bodies(markdown: str) -> dict[str, str]:
    """Retain each section independently so swapped entries cannot pass."""
    result = {}
    found = list(re.finditer(r"^### (.+)$", markdown, re.MULTILINE))
    for index, heading in enumerate(found):
        end = found[index + 1].start() if index + 1 < len(found) else len(markdown)
        result[heading.group(1)] = markdown[heading.end():end]
    return result


def fragment_links(markdown: str) -> dict[str, str]:
    """Read Sacho's generated URL pins and retained Markdown definitions."""
    pins = dict(re.findall(r"^  [\'\"]?([^\'\"\n:]+)[\'\"]?: (https?://\S+)$", markdown, re.MULTILINE))
    return {**pins, **links(markdown)}


def released_tail(raw: bytes) -> bytes:
    """Keep all frozen releases byte-for-byte after the generated marker."""
    return raw.split(b"<!-- sacho:unreleased:end -->", 1)[1]


class FragmentLayoutTests(unittest.TestCase):
    def test_public_families_and_next_file_are_valid(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for path in ("rust/gukhanmun/note.md", "javascript/napi/note.md", "python/gukhanmun/note.md", "next"):
                target = root / "changes.d" / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("0.4.0\n" if path == "next" else " -  Added an example.\n")
            self.assertEqual(fragment_errors(root), [])

    def test_ignored_and_private_locations_fail(self):
        for relative in ("note.md", "crates/gukhanmun/note.md", "packages/napi/note.md", "other/gukhanmun/note.md", "rust/note.md", "rust/gukhanmun/deep/note.md", "rust/gukhanmun-python/note.md"):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                target = root / "changes.d" / relative
                target.parent.mkdir(parents=True)
                target.write_text(" -  Added an example.\n")
                self.assertEqual(len(fragment_errors(root)), 1)


class FragmentLayoutCommandTests(unittest.TestCase):
    def run_check(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, "-B", str(Path(__file__).with_name("check.py")), *args],
            text=True,
            capture_output=True,
        )

    def test_missing_or_nonproject_roots_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plain_file = root / "file"
            plain_file.write_text("not a directory")
            for invalid in (root / "missing", root, plain_file):
                with self.subTest(root=invalid):
                    result = self.run_check(str(invalid))
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("expected a project directory containing sacho.toml", result.stderr)

    def test_extra_arguments_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "sacho.toml").write_text(CONFIG + NEW_PATTERNS)
            result = self.run_check(str(root), "extra")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("usage: check.py [project-root]", result.stderr)

    def test_released_project_without_fragment_directory_is_valid(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "sacho.toml").write_text(CONFIG + NEW_PATTERNS)
            self.assertFalse((root / "changes.d").exists())
            result = self.run_check(str(root))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stderr, "")


class ConfigurationTests(unittest.TestCase):
    def test_repository_patterns_preserve_current_and_historical_titles(self):
        config = tomllib.loads((PROJECT_ROOT / "sacho.toml").read_text())
        expected = tomllib.loads(NEW_PATTERNS)
        self.assertEqual(config["check"]["paths"], [])
        actual_patterns = config["section-patterns"]
        self.assertEqual(len(actual_patterns), 3)
        for actual, wanted in zip(actual_patterns, expected["section-patterns"], strict=True):
            self.assertEqual({key: actual.get(key) for key in wanted}, wanted)
        self.assertEqual(config["section-maps"], expected["section-maps"])
        python_paths = actual_patterns[2].get("paths", ["python/{name}/**"])
        self.assertTrue(all(path.startswith("python/{name}/") for path in python_paths))


class CarryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sacho = shutil.which("sacho")
        if not cls.sacho:
            raise RuntimeError("Run changelog tests through mise so the pinned sacho is available")
        cls.version = subprocess.check_output([cls.sacho, "--version"], text=True).strip()
        print(f"Changelog regression tool: {cls.version}")

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "changes.d").mkdir()
        (self.root / "changes.d/next").write_text("1.0.0\n")
        (self.root / "CHANGES.md").write_text(EMPTY_CHANGELOG)

    def run_sacho(self, *args: str) -> str:
        result = subprocess.run([self.sacho, *args], cwd=self.root, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, f"{self.version}: sacho {' '.join(args)}\n{result.stdout}{result.stderr}")
        return result.stdout

    def configure(self, patterns: str):
        (self.root / "sacho.toml").write_text(CONFIG + patterns)
        self.run_sacho("sync", "--force")

    def document(self) -> str:
        return (self.root / "CHANGES.md").read_text()

    def fragment_bytes(self) -> dict[str, bytes]:
        return {path.relative_to(self.root / "changes.d").as_posix(): path.read_bytes() for path in sorted((self.root / "changes.d").rglob("*.md"))}

    def sample_release(self, old: bool):
        self.configure(OLD_PATTERNS if old else NEW_PATTERNS)
        for component in ("crates/gukhanmun", "packages/napi"):
            (self.root / component).mkdir(parents=True)
            family = component if old else component.replace("crates/", "rust/").replace("packages/", "javascript/")
            for label, priority in (("first", -10), ("second", 10)):
                path = self.root / "changes.d" / family / f"{label}.md"
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(f'''---
priority: {priority}
links:
  "#54": https://github.com/dahlia/gukhanmun/pull/54
---

 -  Added {component} {label}. [[#47], [#54]]
''')
        if not old:
            path = self.root / "changes.d/python/gukhanmun/sample.md"
            path.parent.mkdir(parents=True)
            path.write_text(' -  Added Python sample. [[#47]]\n')
        self.run_sacho("sync", "--force")
        self.run_sacho("fmt")
        self.run_sacho("release", "1.0.0", "--date", "2026-11-10", "--next", "1.0.1")

    def assert_repeated_carry(self, version: str, expected_paths: set[str], expected: str):
        frozen = released_tail((self.root / "CHANGES.md").read_bytes())
        self.run_sacho("carry", version)
        first = self.fragment_bytes()
        self.assertEqual(set(first), expected_paths)
        self.assertTrue(first)
        source_sections = section_bodies(expected)
        rendered = self.document().split("<!-- sacho:unreleased:end -->")[0]
        rendered_sections = section_bodies(rendered)
        for path, value in first.items():
            family, component, _ = Path(path).parts
            base = f"@gukhanmun/{component}" if family == "javascript" else component
            suffix = {"rust": "Rust", "javascript": "JavaScript/TypeScript", "python": "Python"}[family]
            title = f"{base} ({suffix})"
            source = source_sections.get(title, source_sections.get(base))
            self.assertIsNotNone(source, path)
            self.assertEqual(items(value.decode()), items(source), path)
            expected_links = {
                key: url for key, url in links(expected).items()
                if re.search(r"\[" + re.escape(key) + r"\]", source)
            }
            self.assertEqual(fragment_links(value.decode()), expected_links, path)
            self.assertIn(title, rendered_sections)
            self.assertEqual(items(rendered_sections[title]), items(source), title)
            for key, destination in expected_links.items():
                self.assertEqual(links(rendered)[key], destination, title)
            for carried in (value.decode(), rendered_sections[title]):
                if " first." in carried:
                    self.assertLess(carried.index(" first."), carried.index(" second."))
        combined = "\n".join(value.decode() for value in first.values())
        self.assertEqual(items(combined), items(expected))
        carried_links = {}
        for value in first.values():
            carried_links.update(fragment_links(value.decode()))
        self.assertEqual(carried_links, links(expected))
        rendered_first = self.document()
        self.run_sacho("carry", version)
        self.assertEqual(self.fragment_bytes(), first)
        self.assertEqual(self.document(), rendered_first)
        self.assertEqual(released_tail((self.root / "CHANGES.md").read_bytes()), frozen)
        self.assertEqual((self.root / "changes.d/next").read_text(), "1.0.1\n")
        self.assertEqual(fragment_errors(self.root), [])
        self.run_sacho("fmt")
        self.run_sacho("check")

    def test_historical_release_maps_to_new_families(self):
        self.sample_release(old=True)
        frozen = release(self.document(), "1.0.0")
        self.assertEqual(sum(items(frozen).values()), 4)
        for component in ("crates/gukhanmun", "packages/napi"):
            self.assertLess(frozen.index(f"Added {component} first."), frozen.index(f"Added {component} second."))
        self.configure(NEW_PATTERNS)
        self.assert_repeated_carry("1.0.0", {"rust/gukhanmun/carried-from-1.0.0.md", "javascript/napi/carried-from-1.0.0.md"}, frozen)
        self.assertEqual(headings(self.document().split("<!-- sacho:unreleased:end -->")[0]), ["gukhanmun (Rust)", "@gukhanmun/napi (JavaScript/TypeScript)"])

    def test_new_identity_release_is_repeatable(self):
        self.sample_release(old=False)
        frozen = release(self.document(), "1.0.0")
        self.assert_repeated_carry("1.0.0", {"rust/gukhanmun/carried-from-1.0.0.md", "javascript/napi/carried-from-1.0.0.md", "python/gukhanmun/carried-from-1.0.0.md"}, frozen)
        self.assertEqual(headings(self.document().split("<!-- sacho:unreleased:end -->")[0]), ["gukhanmun (Rust)", "@gukhanmun/napi (JavaScript/TypeScript)", "gukhanmun (Python)"])

    def test_current_titles_without_metadata_still_resolve(self):
        self.sample_release(old=False)
        text = re.sub(r"^<!-- sacho:section:v1 [^\n]+ -->\n", "", self.document(), flags=re.MULTILINE)
        (self.root / "CHANGES.md").write_text(text, newline="\n")
        frozen = release(text, "1.0.0")
        self.assert_repeated_carry("1.0.0", {"rust/gukhanmun/carried-from-1.0.0.md", "javascript/napi/carried-from-1.0.0.md", "python/gukhanmun/carried-from-1.0.0.md"}, frozen)
        self.assertEqual(headings(self.document().split("<!-- sacho:unreleased:end -->")[0]), ["gukhanmun (Rust)", "@gukhanmun/napi (JavaScript/TypeScript)", "gukhanmun (Python)"])

    def test_actual_release_retains_every_historical_section(self):
        self.configure(NEW_PATTERNS)
        shutil.copyfile(PROJECT_ROOT / "CHANGES.md", self.root / "CHANGES.md")
        (self.root / "changes.d/next").write_text("1.0.1\n")
        self.run_sacho("sync", "--force")
        frozen = release(self.document(), "0.3.1")
        expected_old = ["gukhanmun", "gukhanmun-cli", "gukhanmun-core", "gukhanmun-html", "gukhanmun-markdown", "gukhanmun-napi", "gukhanmun-wasm", "@gukhanmun/napi", "@gukhanmun/wasm"]
        self.assertEqual(headings(frozen), expected_old)
        expected_paths = {f"{'javascript/' + name.split('/')[1] if name.startswith('@') else 'rust/' + name}/carried-from-0.3.1.md" for name in expected_old}
        self.assert_repeated_carry("0.3.1", expected_paths, frozen)
        expected_new = [name + (" (JavaScript/TypeScript)" if name.startswith("@") else " (Rust)") for name in expected_old]
        self.assertEqual(headings(self.document().split("<!-- sacho:unreleased:end -->")[0]), expected_new)

    def test_python_sections_can_be_reserved_before_sources_exist(self):
        self.configure(NEW_PATTERNS)
        (self.root / "crates/gukhanmun-python").mkdir(parents=True)
        for distribution in reversed(PYTHON_DISTRIBUTIONS):
            self.assertFalse((self.root / "python" / distribution).exists())
            self.run_sacho("add", "--section", f"{distribution} (Python)", "reserved")
            path = self.root / "changes.d/python" / distribution / "reserved.md"
            self.assertTrue(path.is_file())
            path.write_text(f" -  Added {distribution} reservation.\n")
            self.run_sacho("sync", "--force")
            self.run_sacho("fmt")
        self.assertEqual(headings(self.document()), [f"{name} (Python)" for name in PYTHON_DISTRIBUTIONS])
        self.assertFalse((self.root / "changes.d/rust/gukhanmun-python").exists())
        self.assertFalse((self.root / "python").exists())
        self.assertEqual(fragment_errors(self.root), [])
        self.run_sacho("check")


if __name__ == "__main__":
    unittest.main()

# Gukhanmun: Tests dictionary collection without accounts or network access.
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

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request
import zipfile

import collect as c


def button(kind, key="123", month="2026-10"):
    if kind == "stdict":
        return f'''<p>올린 날짜 : {month}</p><a href="javascript:fnDownloadFile('9');">XML 전체 내려받기</a><a href="javascript:fnDownloadFile('{key}');"><img alt="JSON">전체 내려받기</a>'''
    return f'''<div>{month}</div><button onclick="javascript:fnCmdDowndload('{key}');">JSON 전체 내려받기</button>'''


def record(kind, code=1, word="가"):
    value = {"target_code": code, "word_info" if kind == "stdict" else "wordinfo": {"word": word}}
    if kind == "opendict":
        value["senseinfo"] = {"type": "일반어"}
    return value


def archive(path, kind, shards, total=None, date="20261012"):
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for endpoint, items in shards:
            stamp = date + (" 00:00:00" if kind == "stdict" else "000000")
            channel = {"item": items, "total": total or len(items), "lastBuildDate": stamp}
            z.writestr(f"123_{endpoint}.json", json.dumps({"channel": channel}, ensure_ascii=False))
    label = "표준국어대사전" if kind == "stdict" else "우리말샘"
    return f"전체 내려받기_{label}_JSON_20261012.zip"


class CollectorTests(unittest.TestCase):
    def test_semantic_discovery_and_ambiguity(self):
        for kind in c.KINDS:
            self.assertEqual(c.discover(kind, button(kind)), {"link_key": "123", "month": "2026-10"})
            for invalid in ["<html>Login</html>", button(kind) + button(kind, "456"), button(kind).replace("JSON", "XML")]:
                with self.assertRaises(c.InvalidDump):
                    c.discover(kind, invalid)

    def test_rsa_padding_capacity_and_login_shapes(self):
        modulus = "f" * 256
        with patch.object(c.secrets, "token_bytes", return_value=b"\x01" * 128):
            encrypted = c.rsa_password("비밀", modulus, "10001")
            payload = "비밀".encode()
            padded = b"\x00\x02" + b"\x01" * (128 - len(payload) - 3) + b"\x00" + payload
            self.assertEqual(encrypted, format(pow(int.from_bytes(padded, "big"), 65537, int(modulus, 16)), "x").zfill(256))
        with self.assertRaises(c.InvalidDump):
            c.rsa_password("x" * 200, modulus, "10001")
        for kind in c.KINDS:
            calls = []
            class Fake:
                def page(self, url, data=None):
                    calls.append((url, data))
                    if "loginForm" in url:
                        return f'<input name="RSAModulus" value="{modulus}"><input name="RSAExponent" value="10001">'
                    return button(kind)
            self.assertEqual(c.login(kind, "synthetic-user", "synthetic-password", Fake())["link_key"], "123")
            posted = [data for _, data in calls if data is not None][0]
            self.assertNotIn("synthetic-password", posted.values())
            self.assertEqual(posted["j_username" if kind == "stdict" else "user_id"], "synthetic-user")

    def test_redirect_origins_and_error_redaction(self):
        session = c.Session("opendict")
        for url in ["http://opendict.korean.go.kr/main", "https://opendict.korean.go.kr.evil.test/", "https://user@opendict.korean.go.kr/", "https://opendict.korean.go.kr:444/"]:
            with self.assertRaises(c.InvalidDump):
                session.open(url)
        request = urllib.request.Request(c.ENDPOINTS["opendict"], data=b"password=private")
        redirect = c.Redirects(session.hosts)
        with self.assertRaises(c.InvalidDump):
            redirect.redirect_request(request, None, 302, "", {}, "https://kdict.korean.go.kr/main")
        error = urllib.error.HTTPError("https://private-secret.test/", 403, "private-secret", {}, None)
        with patch.object(session.opener, "open", side_effect=error):
            with self.assertRaises(c.InvalidDump) as raised:
                session.open(c.ENDPOINTS["opendict"])
        self.assertNotIn("private-secret", str(raised.exception))

    @unittest.skipIf(c.os.name == "nt", "symlink creation requires elevated Windows privileges")
    def test_trusted_root_alias_and_untrusted_descendant_symlinks(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "root"
            root.mkdir()
            (root / "value.json").write_text("{}")
            alias = Path(temp) / "alias"
            alias.symlink_to(root, target_is_directory=True)
            self.assertEqual(c.read_json(alias / "value.json", root=alias), {})
            (root / "nested").symlink_to(root, target_is_directory=True)
            with self.assertRaises(c.InvalidDump):
                c.read_json(root / "nested/value.json", root=root)

    def test_raw_duplicate_totals_and_conflicting_records(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "source.zip"
            for kind in c.KINDS:
                a = record(kind)
                name = archive(path, kind, [(2, [a, dict(reversed(list(a.items())))])], total=2)
                source = c.validate_archive(path, kind, "123", name)
                self.assertEqual((source["record_count"], source["unique_record_count"], source["duplicate_record_count"]), (2, 1, 1))
                name = archive(path, kind, [(2, [a, record(kind, word="나")])], total=2)
                with self.assertRaisesRegex(c.InvalidDump, "conflicting duplicate"):
                    c.validate_archive(path, kind, "123", name)

    def test_opendict_endpoint_shortfall_and_missing_shards(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "source.zip"
            full = [record("opendict", n) for n in range(1, 50001)]
            # A partial filename endpoint need not equal the raw row count.
            name = archive(path, "opendict", [(50000, full), (50001, [record("opendict", n) for n in range(50001, 50061)])])
            source = c.validate_archive(path, "opendict", "123", name)
            self.assertEqual(source["record_count"] - source["terminal_endpoint"], 59)
            for shards in [[(50000, full)], [(50000, full), (100001, [record("opendict", 50001)])], [(100, [record("opendict")])]]:
                name = archive(path, "opendict", shards)
                with self.assertRaises(c.InvalidDump):
                    c.validate_archive(path, "opendict", "123", name)

    def test_dates_totals_schema_traversal_and_corruption(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "source.zip"
            for kind in c.KINDS:
                for total, date in [(2, "20261012"), (1, "20260912")]:
                    name = archive(path, kind, [(1, [record(kind)])], total=total, date=date)
                    with self.assertRaises(c.InvalidDump):
                        c.validate_archive(path, kind, "123", name)
                name = archive(path, kind, [(1, [record(kind)])])
                with zipfile.ZipFile(path, "a") as z:
                    z.writestr("../escape.json", "{}")
                with self.assertRaises(c.InvalidDump):
                    c.validate_archive(path, kind, "123", name)
                path.write_bytes(b"<html>Login required</html>")
                with self.assertRaises(zipfile.BadZipFile):
                    c.validate_archive(path, kind, "123", name)
            name = archive(path, "stdict", [(1, [record("stdict")])])
            with patch.object(c, "MAX_MEMBER", 1):
                with self.assertRaises(c.InvalidDump):
                    c.validate_archive(path, "stdict", "123", name)

    def test_reissued_download_identifier_and_mixed_archive_generations(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "source.zip"
            name = archive(path, "stdict", [(1, [record("stdict")])])
            source = c.validate_archive(path, "stdict", "456", name)
            self.assertEqual(source["link_key"], "456")
            self.assertEqual(source["archive_member_key"], "123")
            with zipfile.ZipFile(path, "a") as z:
                z.writestr("456_2.json", json.dumps({"channel": {"item": [record("stdict", 2)], "total": 2, "lastBuildDate": "20261012 00:00:00"}}))
            with self.assertRaisesRegex(c.InvalidDump, "mixed archive"):
                c.validate_archive(path, "stdict", "456", name)

    def test_collect_skips_committed_identity_and_removes_partial_failure(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "collection"
            class Fake:
                def __init__(self, kind): self.kind = kind
                def page(self, url, data=None): return button(self.kind)
                def open(self, *args, **kwargs): raise AssertionError("unchanged source must not download")
            prior = {"dictionaries": {k: {"link_key": "123", "archive_date": "2026-10-12"} for k in c.KINDS}}
            with patch.dict(c.os.environ, NIKL_USERNAME="synthetic", NIKL_PASSWORD="synthetic"), patch.object(c, "login", side_effect=lambda kind, u, p, s: c.discover(kind, button(kind))):
                result = c.collect(path, prior, sessions=Fake)
                self.assertEqual([v["status"] for v in result["dictionaries"].values()], ["unchanged", "unchanged"])
            path2 = Path(temp) / "failure"
            with patch.dict(c.os.environ, NIKL_USERNAME="synthetic", NIKL_PASSWORD="synthetic"), patch.object(c, "login", side_effect=c.InvalidDump("login failed")):
                with self.assertRaises(c.InvalidDump): c.collect(path2, {}, sessions=Fake)
            self.assertFalse(path2.exists())

    def test_skip_fingerprint_covers_editorial_and_output_drift(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in ["Cargo.toml", "Cargo.lock", "mise.toml", "scripts/dictionaries/interface.json", "crates/gukhanmun-stdict/src/extract.rs", "crates/gukhanmun-stdict/data/defaults.tsv"]:
                path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text("synthetic")
            base = root / "crates/gukhanmun-stdict/data"
            outputs = {}
            for name in c.FILES["stdict"]:
                (base / name).write_text("hanja\thangul\n甲\t갑\n")
                outputs[name] = {"sha256": c.sha_file(base / name)}
            source = {"version": 1, "dictionary": "stdict", "inputs_sha256": c.input_fingerprint(root), "outputs": outputs, "archive_sha256": "a" * 64, "link_key": "123", "archive_date": "2026-10-12"}
            (base / "source.json").write_bytes(c.json_bytes(source))
            (base / "metadata.json").write_bytes(c.json_bytes({"source": "stdict-json-20261012", "build_date": "2026-10-12T00:00:00Z"}))
            self.assertIn("stdict", c.state(root)["dictionaries"])
            (base / "stdict.tsv").write_text("modified by human")
            self.assertNotIn("stdict", c.state(root)["dictionaries"])
            (base / "stdict.tsv").write_text("hanja\thangul\n甲\t갑\n")
            (base / "defaults.tsv").write_text("changed decision")
            self.assertNotIn("stdict", c.state(root)["dictionaries"])


if __name__ == "__main__":
    unittest.main()

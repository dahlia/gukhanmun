# Gukhanmun: Collects and validates monthly NIKL dictionary dumps.
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

"""Credential-bearing collection is separate from offline extraction/publication."""

import argparse
from datetime import datetime, timedelta
import hashlib
from html.parser import HTMLParser
import http.cookiejar
import json
import os
from pathlib import Path
import re
import secrets
import stat
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile

KINDS = ("stdict", "opendict")
LISTINGS = {
    "stdict": "https://stdict.korean.go.kr/mypage/download/downloadList.do",
    "opendict": "https://opendict.korean.go.kr/member/memberDownloadList",
}
ENDPOINTS = {
    "stdict": "https://stdict.korean.go.kr/common/download.do",
    "opendict": "https://opendict.korean.go.kr/member/fileDownload",
}
FILES = {
    "stdict": ("stdict.tsv", "suffix.tsv"),
    "opendict": ("general.tsv", "north-korean.tsv", "dialect.tsv", "archaic.tsv"),
}
MAX_ZIP = 1024**3
MAX_MEMBER = 128 * 1024**2
MAX_TOTAL = 4 * 1024**3
MAX_HTML = 2 * 1024**2
LANGUAGE = "ko-KR,ko;q=0.9,en-US;q=0.8,en;q=0.7"


class InvalidDump(Exception):
    """A fixed-context failure that can be logged without HTTP/account material."""


def sha_file(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode()


def regular(path, root=None):
    path = Path(path).absolute()
    # The caller's root is trusted. Resolve OS aliases such as macOS /var and
    # /tmp, but refuse symlinks in the untrusted path below that boundary.
    boundary = Path(root).absolute() if root is not None else path.parent
    relative = path.relative_to(boundary)
    boundary = boundary.resolve()
    path = boundary / relative
    current = path
    while current != boundary:
        if current.is_symlink():
            raise InvalidDump("symlink input path")
        current = current.parent
    if not path.is_file():
        raise InvalidDump("non-regular input")
    return path


def read_json(path, limit=128 * 1024, root=None):
    path = regular(path, root)
    if path.stat().st_size > limit:
        raise InvalidDump("oversized metadata")
    return json.loads(path.read_bytes())


def input_fingerprint(root):
    """Fingerprint source/decisions rather than generated artifacts or timestamps."""
    root = Path(root).resolve()
    paths = [root / p for p in ("Cargo.lock", "Cargo.toml", "mise.toml", "scripts/dictionaries/interface.json")]
    for p in (root / "crates").rglob("*"):
        if p.is_file() and (p.suffix == ".rs" or p.name in ("Cargo.toml", "defaults.tsv", "rules.tsv")):
            paths.append(p)
    digest = hashlib.sha256()
    for path in sorted(paths):
        relative = path.relative_to(root).as_posix().encode()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(bytes.fromhex(sha_file(regular(path, root))))
    return digest.hexdigest()


def state(root):
    """Read only committed target paths in the caller's pinned target checkout."""
    root = Path(root).resolve()
    fingerprint = input_fingerprint(root)
    result = {"version": 1, "inputs_sha256": fingerprint, "dictionaries": {}}
    for kind in KINDS:
        base = root / f"crates/gukhanmun-{kind}/data"
        try:
            source = read_json(base / "source.json", root=root)
            outputs = source["outputs"]
            if source["inputs_sha256"] != fingerprint or set(outputs) != set(FILES[kind]):
                continue
            if any(outputs[name]["sha256"] != sha_file(regular(base / name, root)) for name in FILES[kind]):
                continue
            if not re.fullmatch(r"[a-f0-9]{64}", source["archive_sha256"]):
                continue
            if source["dictionary"] != kind or source["version"] != 1:
                continue
            if not re.fullmatch(r"[1-9][0-9]*", source["link_key"]):
                continue
            metadata = read_json(base / "metadata.json", root=root)
            if metadata.get("source") != f"{kind}-json-{source['archive_date'].replace('-', '')}" or metadata.get("build_date") != source["archive_date"] + "T00:00:00Z":
                continue
            result["dictionaries"][kind] = source
        except (OSError, ValueError, KeyError, TypeError, InvalidDump):
            # A missing/stale provenance record cannot authorize skipping collection.
            continue
    return result


class Page(HTMLParser):
    """Match a whole JSON button by label, not the first download identifier."""

    def __init__(self):
        super().__init__()
        self.inputs = {}
        self.text = []
        self.buttons = []
        self.current = None

    def handle_starttag(self, tag, attributes):
        attributes = dict(attributes)
        if tag == "input":
            self.inputs[attributes.get("name", attributes.get("id", ""))] = attributes.get("value", "")
        if tag in ("a", "button"):
            self.current = {"action": attributes.get("onclick", "") + attributes.get("href", ""), "label": "", "before": " ".join(self.text)}
        if tag == "img" and self.current:
            self.current["label"] += " " + attributes.get("alt", "")

    def handle_data(self, text):
        self.text.append(text)
        if self.current:
            self.current["label"] += text

    def handle_endtag(self, tag):
        if tag in ("a", "button") and self.current:
            self.buttons.append(self.current)
            self.current = None


def discover(kind, html):
    page = Page()
    page.feed(html)
    callback = "fnDownloadFile" if kind == "stdict" else "fnCmdDowndload"
    choices = []
    for button in page.buttons:
        label = " ".join(button["label"].split())
        match = re.search(rf"\b{callback}\(['\"]([1-9][0-9]*)['\"]\)", button["action"])
        if match and "JSON" in label.upper() and "전체" in label and "내려받기" in label:
            months = re.findall(r"\b(20[0-9]{2}-(?:0[1-9]|1[0-2]))\b", button["before"])
            if not months:
                raise InvalidDump(f"{kind}: listing month missing")
            choices.append({"link_key": match[1], "month": months[-1]})
    if len(choices) != 1:
        raise InvalidDump(f"{kind}: authenticated whole JSON button missing or ambiguous")
    return choices[0]


def allowed_url(url, hosts):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != "https" or parsed.netloc not in hosts or parsed.username or parsed.password:
        raise InvalidDump("unexpected request/redirect origin")


class Redirects(urllib.request.HTTPRedirectHandler):
    def __init__(self, hosts):
        super().__init__()
        self.hosts = hosts

    def redirect_request(self, request, response, code, message, headers, newurl):
        allowed_url(newurl, self.hosts)
        if request.data is not None and urllib.parse.urlsplit(request.full_url).netloc != urllib.parse.urlsplit(newurl).netloc:
            raise InvalidDump("credential-bearing cross-origin redirect refused")
        return super().redirect_request(request, response, code, message, headers, newurl)


class Session:
    def __init__(self, kind):
        self.hosts = {f"{kind}.korean.go.kr"}
        if kind == "opendict":
            self.hosts.add("kdict.korean.go.kr")
        self.opener = urllib.request.build_opener(Redirects(self.hosts), urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
        self.opener.addheaders = [("User-Agent", "Gukhanmun dictionary updater"), ("Accept-Language", LANGUAGE)]

    def open(self, url, data=None, retry=False):
        allowed_url(url, self.hosts)
        body = urllib.parse.urlencode(data).encode() if data is not None else None
        for attempt in range(3 if retry else 1):
            try:
                return self.opener.open(urllib.request.Request(url, data=body), timeout=60)
            except urllib.error.HTTPError as error:
                if not retry or error.code not in (429, 500, 502, 503, 504) or attempt == 2:
                    raise InvalidDump("NIKL HTTP request failed") from None
                error.close()
            except (urllib.error.URLError, TimeoutError, ConnectionError):
                if not retry or attempt == 2:
                    raise InvalidDump("NIKL connection failed") from None
            time.sleep(attempt + 1)

    def page(self, url, data=None):
        with self.open(url, data, retry=data is None) as response:
            content = response.read(MAX_HTML + 1)
        if len(content) > MAX_HTML:
            raise InvalidDump("oversized NIKL page")
        return content.decode("utf-8")


def rsa_password(password, modulus, exponent):
    if not re.fullmatch(r"[a-fA-F0-9]{256,1024}", modulus) or exponent.lower() != "10001":
        raise InvalidDump("unexpected login public key")
    number = int(modulus, 16)
    size = (number.bit_length() + 7) // 8
    payload = password.encode()
    if len(payload) > size - 11:
        raise InvalidDump("password exceeds login key capacity")
    padding = b""
    while len(padding) < size - len(payload) - 3:
        padding += secrets.token_bytes(size).replace(b"\x00", b"")
    message = b"\x00\x02" + padding[:size - len(payload) - 3] + b"\x00" + payload
    return format(pow(int.from_bytes(message, "big"), 65537, number), "x").zfill(size * 2)


def login(kind, username, password, session):
    if kind == "stdict":
        page = Page()
        page.feed(session.page("https://stdict.korean.go.kr/login/loginForm.do"))
        encrypted = rsa_password(password, page.inputs.get("RSAModulus", ""), page.inputs.get("RSAExponent", ""))
        session.page("https://stdict.korean.go.kr/login/loginLogin.do", {
            "user_agent": "Gukhanmun dictionary updater", "user_targetUrl": "",
            "j_username": username, "j_password": encrypted,
            "j_username_txt": username, "j_password_txt": "",
        })
    else:
        session.page(LISTINGS[kind])
        session.page("https://kdict.korean.go.kr/sign/loginProc", {
            "user_id": username, "user_password": "", "returnUrl": "",
            "hashed_password": hashlib.md5(password.encode(), usedforsecurity=False).hexdigest(),
        })
    return discover(kind, session.page(LISTINGS[kind]))


def positive(value):
    if type(value) is int and value > 0:
        return value
    if isinstance(value, str) and re.fullmatch(r"[1-9][0-9]*", value):
        return int(value)
    raise InvalidDump("invalid source count or identifier")


def validate_archive(path, kind, link_key, filename):
    """Validate observed whole-dump layout, CRC, schema and raw/unique totals."""
    if kind not in KINDS or not re.fullmatch(r"[1-9][0-9]*", link_key):
        raise InvalidDump("invalid dictionary or download identifier")
    label = "표준국어대사전" if kind == "stdict" else "우리말샘"
    match = re.fullmatch(rf"전체 내려받기_{label}_[Jj][Ss][Oo][Nn]_(20[0-9]{{6}})\.zip", filename)
    if not match:
        raise InvalidDump("unexpected download filename")
    date = datetime.strptime(match[1], "%Y%m%d")
    path = regular(path)
    if not 0 < path.stat().st_size <= MAX_ZIP:
        raise InvalidDump("invalid archive size")
    seen = {}
    duplicates = 0
    rows = 0
    dates = []
    totals = set()
    step = 5000 if kind == "stdict" else 50000
    with zipfile.ZipFile(path) as archive:
        members = archive.infolist()
        if not 0 < len(members) <= 1000 or sum(m.file_size for m in members) > MAX_TOTAL:
            raise InvalidDump("invalid archive member count/total size")
        endpoints = {}
        member_key = None
        for member in members:
            name = re.fullmatch(r"([1-9][0-9]*)_([1-9][0-9]*)\.json", member.filename)
            mode = member.external_attr >> 16
            if not name or member.is_dir() or stat.S_IFMT(mode) not in (0, stat.S_IFREG) or mode & 0o111:
                raise InvalidDump("unexpected archive member")
            if member_key is None:
                member_key = name[1]
            elif member_key != name[1]:
                raise InvalidDump("mixed archive member identifiers")
            endpoint = int(name[2])
            if endpoint in endpoints or not 0 < member.file_size <= MAX_MEMBER:
                raise InvalidDump("duplicate or oversized archive member")
            channel = json.loads(archive.read(member))["channel"]
            items = channel["item"]
            if not isinstance(items, list) or not items:
                raise InvalidDump("empty or invalid JSON shard")
            total = positive(channel["total"])
            totals.add(total)
            if kind == "opendict" and total != len(items):
                raise InvalidDump("Open Korean Dictionary shard count mismatch")
            stamp = str(channel["lastBuildDate"])
            built = datetime.strptime(stamp, "%Y%m%d %H:%M:%S" if kind == "stdict" else "%Y%m%d%H%M%S")
            if built.date() not in (date.date(), (date - timedelta(days=1)).date()):
                raise InvalidDump("archive/build date mismatch")
            dates.append(built.date())
            endpoints[endpoint] = len(items)
            rows += len(items)
            word_key = "word_info" if kind == "stdict" else "wordinfo"
            for item in items:
                code = positive(item["target_code"])
                word = item[word_key]
                if not isinstance(word.get("word"), str) or not word["word"]:
                    raise InvalidDump("missing source word")
                if kind == "opendict" and not isinstance(item["senseinfo"]["type"], str):
                    raise InvalidDump("invalid source category")
                digest = hashlib.sha256(json.dumps(item, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()).digest()
                if code in seen:
                    if seen[code] != digest:
                        raise InvalidDump("conflicting duplicate source identifier")
                    duplicates += 1
                else:
                    seen[code] = digest
        ordered = sorted(endpoints)
        final = ordered[-1]
        expected = list(range(step, final, step)) + [final]
        if ordered != expected or any(endpoints[n] != step for n in ordered[:-1]):
            raise InvalidDump("missing or incomplete archive shard")
        if kind == "stdict":
            if totals != {rows} or final != rows or endpoints[final] != final - step * (len(ordered) - 1):
                raise InvalidDump("Standard Korean Dictionary global count mismatch")
        elif final % step == 0 or not final % step <= endpoints[final] < step:
            raise InvalidDump("Open Korean Dictionary terminal shard missing or ambiguous")
        if max(dates) != date.date():
            raise InvalidDump("archive date not represented in source")
    return {
        "version": 1, "dictionary": kind, "source_url": ENDPOINTS[kind],
        "link_key": link_key, "archive_filename": filename,
        "archive_date": date.strftime("%Y-%m-%d"), "archive_sha256": sha_file(path),
        "archive_bytes": path.stat().st_size, "record_count": rows,
        "unique_record_count": len(seen), "duplicate_record_count": duplicates,
        "shard_count": len(endpoints), "terminal_endpoint": final,
        "archive_member_key": member_key,
    }


def collect(destination, prior, sessions=Session):
    username, password = os.environ.get("NIKL_USERNAME"), os.environ.get("NIKL_PASSWORD")
    if not username or not password:
        raise InvalidDump("NIKL account environment variables required")
    destination = Path(destination)
    if destination.exists():
        raise InvalidDump("collection destination must not exist")
    destination.mkdir(mode=0o700, parents=True)
    outcome = {"version": 1, "dictionaries": {}}
    try:
        for kind in KINDS:
            session = sessions(kind)
            identity = login(kind, username, password, session)
            old = prior.get("dictionaries", {}).get(kind, {})
            if old.get("link_key") == identity["link_key"] and old.get("archive_date", "").startswith(identity["month"]):
                outcome["dictionaries"][kind] = {"status": "unchanged", "source": old}
                continue
            fields = {"link_key": identity["link_key"]}
            if kind == "stdict":
                fields.update(pageUnit=10, pageIndex=1)
            path = destination / f"{kind}.zip"
            with session.open(ENDPOINTS[kind], fields, retry=True) as response, path.open("xb") as stream:
                filename = urllib.parse.unquote(response.headers.get_filename() or "")
                size = 0
                while block := response.read(1024 * 1024):
                    size += len(block)
                    if size > MAX_ZIP:
                        raise InvalidDump("download exceeds size limit")
                    stream.write(block)
            source = validate_archive(path, kind, identity["link_key"], filename)
            if not source["archive_date"].startswith(identity["month"]):
                raise InvalidDump("listing/archive month mismatch")
            if discover(kind, session.page(LISTINGS[kind])) != identity:
                raise InvalidDump("source changed during collection; retry next run")
            outcome["dictionaries"][kind] = {"status": "downloaded", "source": source}
        (destination / "collection.json").write_bytes(json_bytes(outcome))
    except BaseException:
        # No completed manifest means no usable collection. Remove partial public inputs.
        import shutil
        shutil.rmtree(destination)
        raise
    return outcome


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    s = sub.add_parser("state")
    s.add_argument("root")
    s.add_argument("output")
    c = sub.add_parser("collect")
    c.add_argument("output")
    c.add_argument("--state")
    v = sub.add_parser("validate")
    v.add_argument("directory")
    args = parser.parse_args()
    try:
        if args.mode == "state":
            Path(args.output).write_bytes(json_bytes(state(args.root)))
        elif args.mode == "collect":
            prior = read_json(args.state) if args.state else {}
            collect(args.output, prior)
            print("Dictionary collection validated")
        else:
            directory = Path(args.directory)
            outcome = read_json(directory / "collection.json")
            if outcome.get("version") != 1 or set(outcome["dictionaries"]) != set(KINDS):
                raise InvalidDump("invalid collection manifest")
            for kind, value in outcome["dictionaries"].items():
                if value["status"] == "downloaded":
                    source = value["source"]
                    actual = validate_archive(directory / f"{kind}.zip", kind, source["link_key"], source["archive_filename"])
                    if actual != source:
                        raise InvalidDump("collection provenance mismatch")
                elif value["status"] != "unchanged":
                    raise InvalidDump("invalid collection status")
            print(json.dumps(outcome, ensure_ascii=False))
    except Exception as error:
        # Values from network errors and pages may contain account material.
        message = str(error) if isinstance(error, InvalidDump) else f"dictionary operation failed ({type(error).__name__})"
        print(message, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

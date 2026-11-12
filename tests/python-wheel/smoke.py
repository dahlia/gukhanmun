# Gukhanmun: Smoke-tests an installed gukhanmun wheel without other tools.
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

"""Smoke checks for an installed gukhanmun wheel.

Run by *scripts/python/wheel_smoke.py* with `python -I` from a directory that
contains no package source.  Only the standard library and the installed
distributions are used, so the checks also run on bare interpreters.  The
final line of output is `SMOKE-RESULT` followed by a JSON record.
"""

import argparse
import ast
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import struct
import sys
import sysconfig

import gukhanmun as g
from gukhanmun import dictionaries as d

CHECKS = []
MACHINES = {"x86_64": "x86_64", "amd64": "x86_64", "x64": "x86_64",
            "aarch64": "aarch64", "arm64": "aarch64"}
SOURCES = {
    "stdict": d.stdict,
    "opendict-general": d.opendict_general,
    "opendict-north-korean": d.opendict_north_korean,
    "opendict-dialect": d.opendict_dialect,
    "opendict-archaic": d.opendict_archaic,
    "loanwords": d.loanwords,
}
# Readings that differ from the per-character fallback, so they prove the
# dictionary was consulted.
ENTRY = g.DictionaryEntry(hanja="樂山樂水", reading="요산요수")


def check(name):
    def decorator(function):
        def wrapper(*arguments):
            result = function(*arguments)
            CHECKS.append(name)
            return result
        return wrapper
    return decorator


def expect(condition, message):
    if not condition:
        raise AssertionError(message)


def libc():
    """The C library; Python before 3.13 cannot name musl, but its SOABI can."""
    name, version = platform.libc_ver()
    if not name and (sysconfig.get_config_var("SOABI") or "").endswith("-musl"):
        return ["musl", version]
    return [name, version]


def installed():
    """Describe the interpreter and the installed native module."""
    distribution = importlib.metadata.distribution("gukhanmun")
    from gukhanmun import _native

    expect(g.__version__ == distribution.version, "__version__ differs from installed metadata")

    native = Path(_native.__file__).resolve()
    tags = [line.removeprefix("Tag: ") for line in (distribution.read_text("WHEEL") or "").splitlines()
            if line.startswith("Tag: ")]
    return {
        "implementation": sys.implementation.name,
        "python": ".".join(map(str, sys.version_info[:3])),
        "implementation_version": ".".join(map(str, sys.implementation.version[:3])),
        "soabi": sysconfig.get_config_var("SOABI"),
        "executable": sys.executable,
        "base_prefix": sys.base_prefix,
        "system": platform.system(),
        "machine": platform.machine(),
        "pointer_bits": struct.calcsize("P") * 8,
        "libc": libc(),
        "distribution": distribution.version,
        "public_version": g.__version__,
        "wheel_tags": tags,
        "native": native.name,
        "native_sha256": hashlib.sha256(native.read_bytes()).hexdigest(),
    }


@check("installed-location")
def check_location(info):
    package = Path(g.__file__).resolve()
    expect(package.is_relative_to(Path(sys.prefix).resolve()), f"gukhanmun was imported from {package}")
    expect(not package.is_relative_to(Path.cwd().resolve()), "gukhanmun was imported from the working directory")
    direct = json.loads(importlib.metadata.distribution("gukhanmun").read_text("direct_url.json") or "{}")
    expect(not direct.get("dir_info", {}).get("editable"), "gukhanmun is an editable install")
    expect(info["wheel_tags"], "the installed wheel records no tags")


@check("interpreter-matches-wheel")
def check_tags(info, arguments):
    version = sys.version_info
    for tag in info["wheel_tags"]:
        python, abi, _ = tag.split("-", 2)
        if sys.implementation.name == "cpython":
            expect(python == "cp311" and abi == "abi3", f"CPython installed the {tag} wheel")
            expect(version >= (3, 11), "the abi3 wheel requires Python 3.11")
        else:
            expect(python == f"pp{version.major}{version.minor}", f"{sys.implementation.name} installed {tag}")
            expect(abi.startswith(f"pypy{version.major}{version.minor}_pp"), f"unexpected PyPy ABI tag {tag}")
    expect(not sysconfig.get_config_var("Py_GIL_DISABLED"), "free-threaded builds are not supported")
    if arguments.expect_implementation:
        expect(sys.implementation.name == arguments.expect_implementation,
               f"expected {arguments.expect_implementation}, running {sys.implementation.name}")
    if arguments.expect_python:
        actual = f"{version.major}.{version.minor}"
        expect(actual == arguments.expect_python, f"expected Python {arguments.expect_python}, running {actual}")
    if arguments.expect_machine:
        actual = MACHINES.get(platform.machine().lower())
        expect(actual == arguments.expect_machine, f"expected {arguments.expect_machine}, running {platform.machine()}")
        expect(struct.calcsize("P") == 8, "expected a 64-bit interpreter")


@check("typing-payload")
def check_typing():
    package = Path(g.__file__).resolve().parent
    expect((package / "py.typed").is_file(), "py.typed is missing")
    stub = package / "_native.pyi"
    expect(stub.is_file(), "_native.pyi is missing")
    ast.parse(stub.read_text(encoding="utf-8"), filename=str(stub))
    for name in g.__all__:
        expect(hasattr(g, name), f"gukhanmun.{name} is listed in __all__ but missing")


def converter():
    return g.Converter(dictionaries=[g.MapDictionarySource([ENTRY])], homophone_window=g.ContextWindow.OFF)


@check("formats")
def check_formats():
    fallback = g.Converter(homophone_window=g.ContextWindow.OFF)
    expect(fallback.convert("漢字") == "한자", "the character fallback is broken")
    expect(fallback.convert(ENTRY.hanja) != ENTRY.reading, "the runtime entry no longer proves dictionary use")
    instance = converter()
    cases = {
        g.InputFormat.TEXT: ("樂山樂水 漢字", "요산요수 한자"),
        g.InputFormat.HTML: ("<p>樂山樂水</p>", "<p>요산요수</p>"),
        g.InputFormat.MARKDOWN: ("# 樂山樂水\n\n漢字\n", "# 요산요수\n\n한자"),
    }
    for format, (source, expected) in cases.items():
        actual = instance.convert(source, format=format)
        expect(actual == expected, f"{format.value}: {source!r} became {actual!r}, not {expected!r}")


@check("stream")
def check_stream():
    instance = converter()
    for format, source in ((g.InputFormat.TEXT, "樂山樂水 漢字"), (g.InputFormat.HTML, "<p>樂山樂水</p>")):
        expected = instance.convert(source, format=format)
        for split in range(len(source) + 1):
            stream = instance.stream(format=format)
            actual = stream.push(source[:split]) + stream.push(source[split:]) + stream.finish()
            expect(actual == expected, f"a stream split at {split} produced {actual!r}")


@check("concurrency")
def check_concurrency():
    shared = converter()
    inputs = [f"樂山樂水 {index} 漢字" for index in range(64)]
    expected = [shared.convert(text) for text in inputs]
    with ThreadPoolExecutor(max_workers=8) as pool:
        same = list(pool.map(shared.convert, inputs * 4))
        independent = list(pool.map(lambda text: converter().convert(text), inputs))
    expect(same == expected * 4, "concurrent use of one converter changed its results")
    expect(independent == expected, "independent converters disagree")


@check("structured-errors")
def check_errors():
    for format in g.DictionaryFormat:
        try:
            g.Converter(dictionaries=[g.FileDictionarySource(b"not a dictionary", format)])
        except g.GukhanmunError as error:
            expect(error.code is g.ErrorCode.DICTIONARY_LOAD, f"{format.value}: got {error.code}")
            expect(error.message, f"{format.value}: the error has no message")
            expect(all(isinstance(cause, g.ErrorCause) for cause in error.chain),
                   f"{format.value}: malformed cause chain")
        else:
            raise AssertionError(f"malformed {format.value} data was accepted")
    try:
        converter().convert("<p", format=g.InputFormat.HTML)
    except g.GukhanmunError as error:
        expect(error.code is g.ErrorCode.SEGMENTATION, f"an unclosed HTML tag raised {error.code}")
        expect("unclosed HTML start tag" in error.message, f"unexpected HTML error: {error.message}")
    else:
        raise AssertionError("an unclosed HTML start tag was accepted")


@check("missing-data")
def check_missing_data():
    for key, helper in SOURCES.items():
        try:
            helper()
        except g.GukhanmunError as error:
            expect(error.code is g.ErrorCode.DICTIONARY_LOAD, f"{key}: got {error.code}")
            expect("pip install 'gukhanmun[" in error.message, f"{key}: no installation hint: {error.message}")
        else:
            raise AssertionError(f"{key} loaded data although no data distribution is installed")


@check("official-corpus")
def check_official(corpus):
    cases = json.loads(Path(corpus).read_text(encoding="utf-8"))["cases"]
    expect(cases, "the official corpus is empty")
    for format in g.DictionaryFormat:
        for case in cases:
            sources = []
            for item in case["dictionaries"]:
                if isinstance(item, dict):
                    entry = g.DictionaryEntry(hanja=item["hanja"], reading=item["reading"])
                    sources.append(g.MapDictionarySource([entry]))
                else:
                    sources.append(SOURCES[item](format=format))
            instance = g.Converter(
                dictionaries=sources,
                source_policy=g.SourcePolicy(case.get("source_policy", "reference-authorized")),
                reference_only=bool(case.get("reference_only", False)),
                homophone_window=g.ContextWindow.OFF,
            )
            actual = instance.convert(case["input"])
            expect(actual == case["expected"],
                   f"{case['name']} ({format.value}): {case['input']!r} became {actual!r}")
            if case.get("differs_from_fallback"):
                fallback = g.Converter(homophone_window=g.ContextWindow.OFF).convert(case["input"])
                expect(fallback != case["expected"], f"{case['name']} does not prove the data was used")


def loaded_modules():
    """List the files of every module loaded into this Windows process."""
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    kernel32.GetCurrentProcess.restype = wintypes.HANDLE
    kernel32.GetModuleFileNameW.argtypes = (wintypes.HMODULE, wintypes.LPWSTR, wintypes.DWORD)
    kernel32.GetModuleFileNameW.restype = wintypes.DWORD
    psapi.EnumProcessModules.argtypes = (wintypes.HANDLE, ctypes.POINTER(wintypes.HMODULE),
                                         wintypes.DWORD, ctypes.POINTER(wintypes.DWORD))
    psapi.EnumProcessModules.restype = wintypes.BOOL
    process = kernel32.GetCurrentProcess()
    count = 256
    while True:
        handles = (wintypes.HMODULE * count)()
        needed = wintypes.DWORD()
        if not psapi.EnumProcessModules(process, handles, ctypes.sizeof(handles), ctypes.byref(needed)):
            raise ctypes.WinError(ctypes.get_last_error())
        if needed.value <= ctypes.sizeof(handles):
            break
        count = needed.value // ctypes.sizeof(wintypes.HMODULE) + 16
    paths = []
    for handle in handles[:needed.value // ctypes.sizeof(wintypes.HMODULE)]:
        size = 260
        while True:
            buffer = ctypes.create_unicode_buffer(size)
            length = kernel32.GetModuleFileNameW(handle, buffer, size)
            if length == 0:
                raise ctypes.WinError(ctypes.get_last_error())
            if length < size - 1:
                break
            size *= 2
        paths.append(buffer.value)
    return paths


def contains(root, path):
    """Whether path lies under root; paths on different drives never do."""
    try:
        return os.path.commonpath([path, root]) == root
    except ValueError:
        return False


def windows_modules():
    """Record where each loaded module came from (all modules at this point)."""
    roots = {"system": os.environ.get("SYSTEMROOT", r"C:\Windows"),
             "interpreter": sys.base_prefix, "environment": sys.prefix}
    canonical = {name: os.path.normcase(os.path.realpath(path)) for name, path in roots.items()}
    modules = []
    for path in loaded_modules():
        resolved = os.path.normcase(os.path.realpath(path))
        origin = next((name for name, root in canonical.items() if contains(root, resolved)), "other")
        modules.append({"path": path, "origin": origin})
    return modules


def resolve_module(name):
    """Load a DLL by name through the Windows loader and return its file."""
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.GetModuleHandleW.argtypes = (wintypes.LPCWSTR,)
    kernel32.GetModuleHandleW.restype = wintypes.HMODULE
    kernel32.GetModuleFileNameW.argtypes = (wintypes.HMODULE, wintypes.LPWSTR, wintypes.DWORD)
    kernel32.GetModuleFileNameW.restype = wintypes.DWORD
    # Loading keeps the library mapped for the rest of the process.
    library = ctypes.WinDLL(name)
    handle = kernel32.GetModuleHandleW(name) or library._handle
    size = 260
    while True:
        buffer = ctypes.create_unicode_buffer(size)
        length = kernel32.GetModuleFileNameW(handle, buffer, size)
        if length == 0:
            raise ctypes.WinError(ctypes.get_last_error())
        if length < size - 1:
            return buffer.value
        size *= 2


@check("windows-import-closure")
def check_import_closure(record_path, wheel, digest):
    """Every DLL the extension imports resolves from an allowed location.

    The audit record lists the extension's normal and delay imports.  Each is
    resolved by the loader in this process, after the extension is imported,
    and must come from Windows itself, the interpreter or the venv; a library
    found only through a developer tool directory would be missing elsewhere.
    The VC runtime is reported separately, because CPython ships its own copy.
    """
    records = json.loads(Path(record_path).read_text(encoding="utf-8"))["records"]
    record = next((item for item in records if item["wheel"] == wheel), None)
    expect(record is not None, f"the audit record does not describe {wheel}")
    expect(record["sha256"] == digest, f"the audit record describes another build of {wheel}")
    roots = {"system": os.environ.get("SYSTEMROOT", r"C:\Windows"),
             "interpreter": sys.base_prefix, "environment": sys.prefix}
    canonical = {name: os.path.normcase(os.path.realpath(path)) for name, path in roots.items()}
    resolved = {}
    for name in record["pe"]["imports"] + record["pe"]["delay_imports"]:
        path = resolve_module(name)
        location = os.path.normcase(os.path.realpath(path))
        origin = next((root for root, prefix in canonical.items() if contains(prefix, location)), None)
        expect(origin is not None, f"{name} resolved to {path}, outside Windows and the interpreter")
        resolved[name] = {"path": path, "origin": origin}
    return resolved


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("base", "official"), required=True)
    parser.add_argument("--corpus", type=Path)
    parser.add_argument("--expect-implementation", choices=("cpython", "pypy"))
    parser.add_argument("--expect-python")
    parser.add_argument("--expect-machine", choices=("x86_64", "aarch64"))
    parser.add_argument("--audit-record", type=Path, help="audit record listing the extension's imports")
    parser.add_argument("--wheel", help="filename of the installed gukhanmun wheel")
    parser.add_argument("--wheel-sha256", help="SHA-256 of the installed gukhanmun wheel")
    arguments = parser.parse_args()
    info = installed()
    check_location(info)
    check_tags(info, arguments)
    check_typing()
    check_formats()
    check_stream()
    check_concurrency()
    check_errors()
    if arguments.mode == "base":
        check_missing_data()
    else:
        if arguments.corpus is None:
            parser.error("--mode official requires --corpus")
        check_official(arguments.corpus)
    result = {"mode": arguments.mode, "interpreter": info, "checks": CHECKS}
    if os.name == "nt":
        if arguments.audit_record is not None:
            if not arguments.wheel or not arguments.wheel_sha256:
                parser.error("--audit-record requires --wheel and --wheel-sha256")
            result["windows_imports"] = check_import_closure(arguments.audit_record, arguments.wheel,
                                                             arguments.wheel_sha256)
        result["windows_modules"] = windows_modules()
    print("SMOKE-RESULT " + json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()

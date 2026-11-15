# Gukhanmun: Tests unchanged npm CLI tarballs through an isolated local registry.
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

"""Native execution evidence is not interchangeable with cross compilation."""

import argparse
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import threading
import time
from urllib.parse import unquote, urlsplit

import artifacts

WITNESS = "汽車길 色깔論 東京"
BUNDLED = "기찻길 색깔론 도쿄"
FALLBACK = "기거길 색깔론 동경"


def run(command, *, directory, env, input=None, ok=True):
    result = subprocess.run(command, cwd=directory, env=env, input=input,
                            capture_output=True, text=True, encoding="utf-8", timeout=180)
    if ok and result.returncode:
        raise RuntimeError(f"command exited {result.returncode}: {command!r}\n{result.stdout}{result.stderr}")
    return result


def native_target(target):
    spec = artifacts.TARGETS[target]
    operating_system = {"Linux": "linux", "Darwin": "darwin", "Windows": "win32"}.get(platform.system())
    cpu = {"aarch64": "arm64", "arm64": "arm64", "x86_64": "x64", "amd64": "x64"}.get(
        platform.machine().lower())
    if operating_system != spec["os"] or cpu != spec["cpu"]:
        raise RuntimeError(f"refusing nonnative execution: {platform.system()}/{platform.machine()} != {target}")
    node = shutil.which("node")
    if not node:
        raise RuntimeError("Node.js is required")
    identity = json.loads(subprocess.check_output(
        [node, "-p", "JSON.stringify({os:process.platform,cpu:process.arch,version:process.versions.node})"],
        text=True,
    ))
    if (identity["os"] != spec["os"] or identity["cpu"] != spec["cpu"]
            or int(identity["version"].split(".")[0]) < 22):
        raise RuntimeError(f"Node.js must be native and >=22: {identity}")
    return identity


def audit_elf(data, cpu):
    """Check ELF64 architecture, PT_INTERP and every DT_NEEDED without readelf."""
    if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01":
        raise RuntimeError("Linux payload must be little-endian ELF64")
    machine = struct.unpack_from("<H", data, 18)[0]
    if machine != {"x64": 62, "arm64": 183}[cpu]:
        raise RuntimeError("ELF architecture differs from target")
    offset = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    if size != 56 or not count or count == 0xffff or offset + size * count > len(data):
        raise RuntimeError("invalid ELF program header table")
    for index in range(count):
        kind, _, start, _, _, length, _, _ = struct.unpack_from("<IIQQQQQQ", data, offset + index * size)
        if start + length > len(data):
            raise RuntimeError("ELF segment exceeds payload")
        if kind == 3:
            raise RuntimeError("Linux payload has PT_INTERP; static musl binary required")
        if kind == 2:
            if length % 16:
                raise RuntimeError("invalid ELF dynamic segment")
            terminated = False
            for position in range(start, start + length, 16):
                tag, _ = struct.unpack_from("<qQ", data, position)
                if tag == 0:
                    terminated = True
                    break
                if tag == 1:
                    raise RuntimeError("Linux payload has DT_NEEDED; static binary required")
            if not terminated:
                raise RuntimeError("unterminated ELF dynamic segment")
    return {"elf_machine": machine, "pt_interp": False, "dt_needed": []}


@contextmanager
def local_registry(directory, manifest):
    """Serve exact bytes with normalized packuments; unknown routes fail locally."""
    directory = Path(directory)
    packages = {}
    tarballs = {}
    for record in manifest["packages"]:
        files, _ = artifacts.archive_files(directory / record["filename"], npm=True)
        packages[record["name"]] = (record, artifacts.read_json(files["package.json"]))
        tarballs["/tarballs/" + record["filename"]] = (directory / record["filename"]).read_bytes()
    requests = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            path = unquote(urlsplit(self.path).path)
            requests.append(path)
            if path in tarballs:
                data, content_type = tarballs[path], "application/octet-stream"
            elif path.lstrip("/") in packages:
                record, metadata = packages[path.lstrip("/")]
                version = manifest["npm_version"]
                metadata = dict(metadata, version=version, dist={
                    "tarball": f"http://127.0.0.1:{self.server.server_port}/tarballs/{record['filename']}",
                    "integrity": record["integrity"],
                    "shasum": __import__("hashlib").sha1(tarballs["/tarballs/" + record["filename"]]).hexdigest(),
                })
                channel = "dev" if "-dev." in version else "latest"
                data = json.dumps({"name": record["name"], "dist-tags": {channel: version},
                                   "versions": {version: metadata}}).encode()
                content_type = "application/json"
            else:
                self.send_error(404, "Not present in immutable local candidate")
                return
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/"
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


def shim_command(path, arguments):
    if os.name != "nt":
        return [str(path), *arguments]
    # Only fixed probe flags are used here. Hostile quoting belongs to direct
    # node-launcher tests, rather than cmd.exe's fundamentally different grammar.
    line = subprocess.list2cmdline([str(path), *arguments])
    return [os.environ.get("COMSPEC", "cmd.exe"), "/d", "/s", "/c", f'"{line}"']


def probe(command, full_version, directory, env):
    def invoke(arguments, **kwargs):
        return run([*command, *arguments], directory=directory, env=env, **kwargs)

    if invoke(["--version"]).stdout.strip() != f"gukhanmun {full_version}":
        raise RuntimeError("installed native CLI version differs from artifact version")
    for flags, expected in (([], BUNDLED), (["--no-bundled-dictionaries"], FALLBACK)):
        result = invoke(["--disambiguation", "off", *flags], input=WITNESS)
        if result.stdout != expected:
            raise RuntimeError(f"dictionary/stdin witness failed: {result.stdout!r} != {expected!r}")
    source = directory / "입력 space.txt"
    destination = directory / "출력 space.txt"
    source.write_text(WITNESS, encoding="utf-8")
    invoke(["--disambiguation", "off", str(source), "-o", str(destination)])
    if destination.read_text(encoding="utf-8") != BUNDLED:
        raise RuntimeError("Unicode/spaced input/output paths did not roundtrip")
    failure = invoke(["--definitely-not-a-gukhanmun-option"], ok=False)
    if failure.returncode == 0 or not failure.stderr:
        raise RuntimeError("CLI failure exit/stderr was not preserved")
    return failure.returncode


def completion_paths(command, directory, env):
    paths = {}
    for shell, filename in artifacts.SHELL_FILES.items():
        result = run([*command, "--print-completion-path", shell], directory=directory, env=env)
        path = Path(result.stdout.strip())
        if not path.is_absolute() or path.name != filename or not path.is_file():
            raise RuntimeError(f"invalid installed completion path for {shell}: {path}")
        paths[shell] = path
    return paths


def resolve_payload(node, parent, target, directory, env, *, required=True):
    """Global npm installs may nest optional dependencies below the parent."""
    result = run([
        node, "-e",
        "process.stdout.write(require.resolve(process.argv[1], {paths: [process.argv[2]]}))",
        f"{artifacts.PARENT}-{target}/package.json", str(parent),
    ], directory=directory, env=env, ok=False)
    if required:
        if result.returncode:
            raise RuntimeError(f"installed platform package cannot be resolved: {result.stderr}")
        return Path(result.stdout).parent
    if result.returncode == 0:
        raise RuntimeError("--omit=optional unexpectedly installed the platform package")
    return None


def shell_probes(paths, selected, directory, env):
    if not selected:
        return
    # shells.py imports generate by its historical sibling name. Loading these
    # functions does not run its main/build/regeneration path.
    generator = artifacts.load_module("npm_shell_generate", artifacts.ROOT / "scripts/completions/generate.py")
    previous = sys.modules.get("generate")
    sys.modules["generate"] = generator
    try:
        shells = artifacts.load_module("npm_completion_shells", artifacts.ROOT / "scripts/completions/shells.py")
    finally:
        if previous is None:
            del sys.modules["generate"]
        else:
            sys.modules["generate"] = previous
    for filename in shells.FIXTURES:
        (directory / filename).touch()
    env = dict(env, ZDOTDIR=str(directory), TERM="xterm", LC_ALL="C.UTF-8")
    for shell in selected:
        getattr(shells, shell)(shells.executable(shell), paths[shell], directory, env)


def _windows_console_worker(native, launcher, node):
    """A private console prevents test events from touching the CI runner."""
    import ctypes
    from ctypes import wintypes

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel.WaitForSingleObject.restype = wintypes.DWORD
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]

    class ProcessEntry(ctypes.Structure):
        _fields_ = [
            ("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
            ("th32ProcessID", wintypes.DWORD), ("th32DefaultHeapID", ctypes.c_size_t),
            ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
            ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", wintypes.LONG),
            ("dwFlags", wintypes.DWORD), ("szExeFile", wintypes.WCHAR * 260),
        ]

    for name in ("Process32FirstW", "Process32NextW"):
        getattr(kernel, name).argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
        getattr(kernel, name).restype = wintypes.BOOL

    def children(pid):
        snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
        if snapshot == ctypes.c_void_p(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            entry = ProcessEntry()
            entry.dwSize = ctypes.sizeof(entry)
            active = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
            result = []
            while active:
                if entry.th32ParentProcessID == pid:
                    result.append((entry.th32ProcessID, entry.szExeFile))
                active = kernel.Process32NextW(snapshot, ctypes.byref(entry))
            return result
        finally:
            kernel.CloseHandle(snapshot)

    kernel.FreeConsole()
    if not kernel.AllocConsole():
        raise RuntimeError(f"cannot allocate private Windows console: {ctypes.WinError(ctypes.get_last_error())}")
    try:
        outcomes = {}
        for label, command in (
            ("direct", [str(native), "--disambiguation", "off"]),
            ("wrapper", [str(node), str(launcher), "--disambiguation", "off"]),
        ):
            process = subprocess.Popen(
                command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
            )
            child_handle = None
            try:
                deadline = time.monotonic() + 15
                if label == "wrapper":
                    while time.monotonic() < deadline:
                        found = children(process.pid)
                        native_children = [pid for pid, name in found if name.lower() == "gukhanmun.exe"]
                        if len(native_children) == 1:
                            child_handle = kernel.OpenProcess(0x100000 | 0x0001, False, native_children[0])
                            if not child_handle:
                                raise ctypes.WinError(ctypes.get_last_error())
                            break
                        if process.poll() is not None:
                            raise RuntimeError("Windows launcher exited before its native child started")
                        time.sleep(0.02)
                    else:
                        raise RuntimeError("cannot observe the Windows launcher's native child")
                # Allow the native console handler to initialize before the
                # real group event. stdin stays open, so EOF cannot end it.
                time.sleep(0.25)
                if process.poll() is not None:
                    raise RuntimeError("Windows console probe exited before interruption")
                os.kill(process.pid, signal.CTRL_BREAK_EVENT)
                status = process.wait(timeout=15)
                if child_handle and kernel.WaitForSingleObject(child_handle, 5000) != 0:
                    raise RuntimeError("native child survived Windows console interruption")
                if children(process.pid):
                    raise RuntimeError("a child survived Windows console interruption")
                outcomes[label] = status & 0xffffffff
            finally:
                # Holding the native handle also makes failure cleanup robust
                # against reparenting and PID reuse.
                for pid, _ in children(process.pid):
                    cleanup = kernel.OpenProcess(0x100000 | 0x0001, False, pid)
                    if cleanup:
                        kernel.TerminateProcess(cleanup, 1)
                        kernel.WaitForSingleObject(cleanup, 5000)
                        kernel.CloseHandle(cleanup)
                if child_handle:
                    if kernel.WaitForSingleObject(child_handle, 0) != 0:
                        kernel.TerminateProcess(child_handle, 1)
                        kernel.WaitForSingleObject(child_handle, 5000)
                    kernel.CloseHandle(child_handle)
                if process.poll() is None:
                    process.kill()
                process.communicate(timeout=10)
        if not outcomes["direct"] or outcomes["wrapper"] != outcomes["direct"]:
            raise RuntimeError(f"Windows console outcomes differ: {outcomes}")
        return {"event": "CTRL_BREAK_EVENT", "statuses": outcomes, "surviving_children": False}
    finally:
        kernel.FreeConsole()


def windows_console_probe(native, launcher, node, directory, env):
    # The helper alone detaches/allocates a console; the smoke driver's original
    # console and standard streams are unaffected.
    script = (
        "import json,sys; sys.path.insert(0,sys.argv[1]); import smoke; "
        "print(json.dumps(smoke._windows_console_worker(*sys.argv[2:])))"
    )
    result = run([sys.executable, "-B", "-c", script, str(Path(__file__).resolve().parent),
                  str(native), str(launcher), str(node)], directory=directory, env=env)
    return artifacts.read_json(result.stdout)


def exercise_registry(registry, version, expected_full_version, target, *, shells=(), spec=None):
    """Clean global + npx + omitted-optional installs, also reusable after publish.

    `version` is the normalized exact registry identity. `spec` may instead be
    '@gukhanmun/cli@dev', '@gukhanmun/cli@latest' or '@gukhanmun/cli'; bytes and
    installed/native versions must still match `expected_full_version`.
    """
    identity = native_target(target)
    if artifacts.npm_version(expected_full_version) != version:
        raise RuntimeError("exact registry identity differs from full artifact version")
    spec = spec or f"{artifacts.PARENT}@{version}"
    npm = artifacts.npm_command()
    evidence = {"node": identity, "npm": subprocess.check_output([*npm, "--version"], text=True).strip(),
                "registry": registry, "spec": spec, "target": target, "version": expected_full_version}
    with tempfile.TemporaryDirectory(prefix="gukhanmun npm 한글 ") as temporary:
        root = Path(temporary)
        for mode in ("global", "npx", "omitted"):
            directory = root / mode
            directory.mkdir()
            env = artifacts.isolated_env(directory / "home", registry)
            prefix = Path(env["NPM_CONFIG_PREFIX"])
            if mode == "npx":
                command = [*npm, "exec", "--yes", "--ignore-scripts",
                           f"--package={spec}", "--", "gukhanmun"]
                # npm exec is npx's implementation; execute its actual npx entry
                # point when available so the public invocation is exercised.
                npx = Path(npm[1]).with_name("npx-cli.js")
                if npx.is_file():
                    command = [npm[0], str(npx), "--yes", "--ignore-scripts",
                               f"--package={spec}", "gukhanmun"]
                evidence["npx_failure_status"] = probe(command, expected_full_version, directory, env)
                paths = completion_paths(command, directory, env)
                parent_candidates = list((directory / "home/cache/_npx").glob("*/node_modules/@gukhanmun/cli/package.json"))
                if len(parent_candidates) != 1:
                    raise RuntimeError("npx did not install exactly one parent package")
                parent = parent_candidates[0].parent
            else:
                if mode == "omitted":
                    # npm 11.20 ignores --omit=optional in global installs.
                    # A fresh local project exercises genuine omission without
                    # deleting installed payloads or rewriting dependencies.
                    artifacts.write_json(directory / "package.json", {"private": True})
                    options = ["--prefix", str(directory), "--omit=optional"]
                    parent = directory / "node_modules/@gukhanmun/cli"
                else:
                    options = ["--global"]
                    parent = prefix / ("node_modules" if os.name == "nt" else "lib/node_modules") / "@gukhanmun/cli"
                run([*npm, "install", "--ignore-scripts", *options, spec],
                    directory=directory, env=env)
                launcher = parent / "bin/gukhanmun.cjs"
                command = [npm[0], str(launcher)]
                paths = completion_paths(command, directory, env)
                if mode == "omitted":
                    result = run([*command, "--version"], directory=directory, env=env, ok=False)
                    if result.returncode == 0 or not result.stderr.strip():
                        raise RuntimeError("omitted optional dependency did not produce a useful failure")
                    for omitted_target in artifacts.TARGETS:
                        resolve_payload(npm[0], parent, omitted_target, directory, env, required=False)
                    evidence["omitted_optional_status"] = result.returncode
                else:
                    evidence["global_failure_status"] = probe(command, expected_full_version, directory, env)
                    shim = prefix / ("gukhanmun.cmd" if os.name == "nt" else "bin/gukhanmun")
                    result = run(shim_command(shim, ["--version"]), directory=directory, env=env)
                    if result.stdout.strip() != f"gukhanmun {expected_full_version}":
                        raise RuntimeError("npm global command shim failed")
                    shell_probes(paths, shells, directory, env)
            metadata = artifacts.read_json((parent / "package.json").read_bytes())
            if metadata["version"] != expected_full_version:
                raise RuntimeError("npm rewrote the full parent package version")
            if mode != "omitted":
                payload = resolve_payload(npm[0], parent, target, directory, env)
                installed = artifacts.read_json((payload / "package.json").read_bytes())
                if installed["version"] != expected_full_version:
                    raise RuntimeError("npm did not preserve the full exact optional package version")
                native = payload / artifacts.TARGETS[target]["binary"]
                result = run([str(native), "--version"], directory=directory, env=env)
                if result.stdout.strip() != f"gukhanmun {expected_full_version}":
                    raise RuntimeError("direct native version mismatch")
                if artifacts.TARGETS[target]["os"] == "linux":
                    evidence["elf"] = audit_elf(native.read_bytes(), artifacts.TARGETS[target]["cpu"])
                elif artifacts.TARGETS[target]["os"] == "win32" and mode == "global":
                    evidence["windows_console"] = windows_console_probe(
                        native, parent / "bin/gukhanmun.cjs", npm[0], directory, env)
        evidence.update(global_install=True, npx=True, ignore_scripts=True, omitted_optional=True,
                        completions=list(artifacts.SHELL_FILES), shell_probes=list(shells),
                        dictionary_witness=True, stdin=True, unicode_paths=True)
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", type=Path, required=True)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--artifact-id", required=True)
    parser.add_argument("--target", choices=artifacts.TARGETS, required=True)
    parser.add_argument("--receipt", type=Path)
    parser.add_argument("--shell", action="append", choices=artifacts.SHELL_FILES, default=[])
    parser.add_argument("--registry", help="Explicit real-registry post-publication check instead of local registry")
    parser.add_argument("--spec", help="Post-publication channel lookup to test")
    args = parser.parse_args()
    manifest = artifacts.validate_bundle(args.dir, args.manifest_sha256, args.artifact_id)
    if args.registry:
        evidence = exercise_registry(args.registry, manifest["npm_version"], manifest["version"],
                                     args.target, shells=args.shell, spec=args.spec)
    else:
        if args.spec:
            parser.error("--spec requires --registry")
        with local_registry(args.dir, manifest) as registry:
            evidence = exercise_registry(registry, manifest["npm_version"], manifest["version"],
                                         args.target, shells=args.shell)
    evidence.update(schema_version=1, artifact_id=str(args.artifact_id),
                    manifest_sha256=args.manifest_sha256, source=manifest["source"],
                    tarballs={item["filename"]: item["sha256"] for item in manifest["packages"]})
    if args.receipt:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        artifacts.write_json(args.receipt, evidence)
    print(json.dumps(evidence, sort_keys=True))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, AssertionError,
            subprocess.SubprocessError) as error:
        sys.exit(str(error))

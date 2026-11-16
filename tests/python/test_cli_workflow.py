# Gukhanmun: Checks the independent CLI workflow graph and credential boundary.
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

import importlib.util
import os
from pathlib import Path
import re
import shutil
import subprocess
from types import SimpleNamespace

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SOURCE = (ROOT / ".github/workflows/main.yaml").read_text()


def job(name):
    match = re.search(r"(?ms)^  " + re.escape(name) + r":\n(.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)", SOURCE)
    assert match is not None
    return match[1]


def needs(name):
    text = job(name)
    match = re.search(r"(?m)^    needs: (.+)$", text)
    if match:
        return set(match[1].strip("[]").replace(",", " ").split())
    match = re.search(r"(?m)^    needs:\n((?:    - .+\n)+)", text)
    return set(re.findall(r"(?m)^    - (.+)$", match[1])) if match else set()


def helper():
    spec = importlib.util.spec_from_file_location("cli_ci_under_test", ROOT / "scripts/python/cli_ci.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_native_graph_has_no_publication_gate_or_cycle():
    assert not needs("cli-context")
    assert "fetch-depth: 0" in job("cli-context")
    assert needs("build-completions") == {"cli-context"}
    assert needs("build-release-binaries") == {"cli-context", "build-completions"}
    for name in ("build-completions", "build-release-binaries"):
        text = job(name)
        assert "should-publish" not in text and "needs.artifact-context" not in text
        assert "stamp-native-version == 'true'" in text
        assert '"$STAMP_NATIVE" != true' in text and "--require-clean" in text
        assert "cli_context.py stamp" in text
        assert "sync_version" not in text and "artifacts.py stamp" not in text
    assert "cli-context" in needs("artifact-context")
    assert not needs("ci") and "cli-" not in job("ci")


def test_python_resolution_failure_cannot_suppress_native_context_or_upload():
    text = job("cli-context")
    assert text.index("id: native") < text.index("id: python")
    python_step = text.split("id: python", 1)[1].split("    - name:", 1)[0]
    assert "continue-on-error: true" in python_step
    assert "python-outcome: ${{ steps.python.outcome }}" in text
    assert "git rev-parse --short" not in SOURCE
    native_source = (ROOT / "scripts/python/cli_context.py").read_text()
    assert native_source.count('"rev-parse", "--short"') == 1
    build = job("build-release-binaries")
    assembly = build.split("id: cli-wheel", 1)[1].split("    - uses:", 1)[0]
    assert "continue-on-error: true" in assembly and 'test "$PYTHON_OUTCOME" = success' in assembly
    assert "--binary \"target/${{ matrix.target }}/release/gukhanmun${{ matrix.exe }}\"" in assembly
    assert "--mkdict-binary \"target/${{ matrix.target }}/release/gukhanmun-mkdict${{ matrix.exe }}\"" in assembly
    assert "rustc -vV" in assembly and "--native-version" in assembly and "--version" in assembly
    assert build.index("Assemble the CLI wheel") < build.index("name: binary-")
    # CLI test's required download fails if isolated assembly emitted no wheel.
    assert "name: cli-wheel-${{ matrix.id }}" in job("cli-wheel-test")
    assert "continue-on-error" not in job("cli-wheel-test")


def test_cli_testing_is_six_native_hosts_not_extension_matrix():
    text = job("cli-wheel-test")
    assert needs("cli-wheel-test") == {"cli-context", "build-release-binaries"}
    assert "fromJSON(needs.cli-context.outputs.matrix)" in text
    assert "runs-on: ${{ matrix.runner }}" in text
    assert "cli_ci.py host" in text and "cli_ci.py containers" in text
    for shell in ("bash", "zsh", "fish", "nushell", "powershell"):
        assert "--shell " + shell in text
    assert "rust-init" not in text and "cargo build" not in text
    ci = helper()
    assert ci.names({"pypy": True}) == ["py311", "py315", "pypy311", "pypy312"]
    assert ci.names({"pypy": False}) == ["py311", "py315"]
    source = (ROOT / "scripts/python/cli_ci.py").read_text()
    assert '"--locked", "--only-group", "installer"' in source
    assert '"--require-hashes"' in source and '"--only-binary=:all:"' in source
    assert '("manylinux_2_28", "musllinux_1_2")' in source and "artifacts.IMAGES" in source
    assert "/opt/python/cp311-cp311/bin/python" in source and "/opt/python/cp315-cp315/bin/python" in source


@pytest.mark.parametrize("ending", ["\n", "\r\n"])
def test_host_pins_split_without_carriage_returns(ending):
    bash = shutil.which("bash")
    if not bash:
        pytest.skip("Bash is required for workflow shell probes")
    split = next(line.strip() for line in job("cli-wheel-test").splitlines() if "read -ra pythons" in line)
    result = subprocess.run([bash, "-euc", split + '\nprintf "%s\\n" "${pythons[@]}"'],
                            env={**os.environ, "pins": "python@3.11.14 python@3.15.2" + ending.rstrip("\n")},
                            capture_output=True, check=True)
    assert result.stdout.splitlines() == [b"python@3.11.14", b"python@3.15.2"]
    assert b"\r" not in result.stdout


@pytest.mark.parametrize("target,count", [("macos-x86_64", 0), ("macos-arm64", 0),
                                        ("windows-arm64", 0), ("windows-amd64", 2), ("linux-x86_64", 8)])
def test_host_shell_arguments_with_nounset(target, count):
    bash = shutil.which("bash")
    if not bash:
        pytest.skip("Bash is required for workflow shell probes")
    text = job("cli-wheel-test").split("- name: Audit and execute on the actual native host", 1)[1]
    script = text.split("      run: |\n", 1)[1].split("    - name:", 1)[0]
    script = "\n".join(line[8:] for line in script.splitlines())
    # Stub only external tools; execute the workflow's actual argument logic.
    prelude = 'mise() { if [[ "$*" == *"which nu"* ]]; then echo /bin/nu; else printf "%s\\n" "$@"; fi; }\n'
    env = {**os.environ, "CLI_TARGET": target, "GUKHANMUN_TOOLS_PYTHON": "/tools/python", "PYTHON_VERSION": "0.4.0"}
    result = subprocess.run([bash, "-c", prelude + script], env=env, capture_output=True, text=True, check=True)
    args = result.stdout.splitlines()
    expected = (["--shell", "powershell"] if count == 2 else
                ["--shell", "bash", "--shell", "zsh", "--shell", "fish", "--shell", "nushell"])
    if count:
        assert args[-count:] == expected
    else:
        assert args[-1] == "0.4.0"
    assert "shells[@]" not in script  # Bash 3.2 treats an empty array as unset.


def test_only_cli_uploader_has_cli_oidc_and_eligible_unsupported_uploads_fail_loudly():
    for name in ("cli-context", "build-completions", "build-release-binaries", "cli-wheel-test",
                 "cli-dists", "cli-publish-check"):
        text = job(name)
        assert "id-token:" not in text and "persist-credentials: false" in text
    text = job("publish-python-cli")
    assert needs("publish-python-cli") == {
        "ci", "python-semantic", "artifact-context", "cli-context", "cli-dists", "cli-publish-check"}
    assert "id-token: write" in text and "persist-credentials: false" in text
    gate = text.split("    if:", 1)[1].split("    timeout-minutes:", 1)[0]
    assert "github.repository == 'dahlia/gukhanmun'" in gate
    assert "github.event_name == 'push'" in gate
    assert "needs.artifact-context.outputs.should-publish == 'true'" in gate
    assert "eligible" not in gate and "python-outcome" not in gate
    assert '"$CLI_ELIGIBLE" != true' in text and '"$PYTHON_OUTCOME" != success' in text
    assert "scripts/python/cli_publish.py publish" in text and "scripts/python/publish.py publish" not in text
    for action in re.findall(r"(?m)^ +(?:- +)?uses: +(\S+)", text):
        if not action.startswith(("actions/", "./")):
            assert re.fullmatch(r"[^@\s]+@[0-9a-f]{40}", action)
    assert "publish-python-cli" not in needs("publish-python")
    assert "publish-python" not in needs("publish-python-cli")


def test_exact_cli_immutable_identity_and_receipts():
    for name in ("cli-publish-check", "publish-python-cli", "release"):
        text = job(name)
        marker = "artifact-ids: ${{ needs.cli-dists.outputs.artifact-id }}"
        assert marker in text
        assert text.index("Require the original CLI artifact identity") < text.index(marker)
        assert "MANIFEST_SHA256: ${{ needs.cli-dists.outputs.manifest-sha256 }}" in text
        assert '--manifest-sha256 "$MANIFEST_SHA256"' in text or "sha256sum --check --strict" in text
        assert "overwrite: true" not in text
    assert "cli-validation-receipt-${{ github.run_attempt }}" in job("cli-publish-check")
    assert "cli-publish-receipt-${{ github.run_attempt }}" in job("publish-python-cli")
    text = job("cli-dists")
    assert "pattern: \"{cli-wheel-*,cli-evidence-*}\"" in text
    assert "retention-days: 90" in text
    assert text.index("cli_artifacts.py verify") < text.index("name: cli-dists")
    # This invokes the same exact-set validator used by the publication adapter;
    # only the later check has a real immutable artifact ID for a receipt.
    assert "cli_publish.py validate" not in text
    assert text.rstrip().endswith("if-no-files-found: error")
    assert "steps.upload.outputs.artifact-id" in text and "steps.manifest.outputs.manifest-sha256" in text


def test_release_keeps_cli_six_and_core31_contracts_separate():
    assert {"publish-python", "publish-python-cli", "python-dists", "cli-dists"} <= needs("release")
    text = job("release")
    assert 'test "${#files[@]}" -eq 31' in text and 'test "${#files[@]}" -eq 6' in text
    assert "cli-manifest.json" in text and "cli-SHA256SUMS" in text
    assert 'pattern: "{python-sdists,python-wheels-*,python-evidence-*}"' in job("python-dists")
    assert "cli-" not in job("python-dists")


@pytest.mark.parametrize("pypy", [False, True])
def test_host_orchestration_uses_exact_pins_and_runs_shells_only_once(tmp_path, monkeypatch, pypy):
    ci = helper()
    tool_python, pipx = ci.executables(tmp_path / "installer")
    tool_python.parent.mkdir(parents=True)
    tool_python.touch()
    pipx.touch()
    pins = {name: "pin-" + name for name in ci.names({"pypy": pypy})}
    selected, commands, calls = [], [], []

    def interpreter(pin):
        selected.append(pin)
        return Path("/interpreters") / pin, Path("/interpreters")

    monkeypatch.setattr(ci, "load", lambda _: SimpleNamespace(pins=lambda: pins, interpreter=interpreter))
    monkeypatch.setattr(ci, "run", lambda *args, **kwargs: commands.append(args))
    monkeypatch.setattr(ci, "smoke", lambda *args, **kwargs: calls.append((args, kwargs)))
    monkeypatch.setattr(ci.shutil, "which", lambda _: str(tmp_path / "uv"))
    item = {"id": "test", "pypy": pypy}
    ci.host(item, tmp_path, "0.4.0", ["bash", "fish"])
    assert selected == list(pins.values())
    assert len(commands) == 1 and "audit" in commands[0]
    assert [call[1]["shells"] for call in calls] == [
        ["bash", "fish"] if name == "py315" else () for name in pins]
    assert all(call[1]["pipx"] == pipx for call in calls)
    assert all(Path(call[1]["uv"]).is_absolute() for call in calls)


def test_linux_container_orchestration_uses_only_existing_digest_pins(tmp_path, monkeypatch):
    ci = helper()
    commands = []
    images = {f"{environment}_aarch64": f"image-{environment}@sha256:" + "a" * 64
              for environment in ("manylinux_2_28", "musllinux_1_2")}
    monkeypatch.setattr(ci, "load", lambda _: SimpleNamespace(
        IMAGES=images, container_command=lambda image, work: ["docker", "run", "--rm", "--volume", str(work)]))
    monkeypatch.setattr(ci, "run", lambda *args, **kwargs: commands.append(args))
    ci.containers({"id": "linux-aarch64", "family": "linux", "machine": "aarch64"}, tmp_path, "0.4.0")
    assert len(commands) == 2
    for command, image in zip(commands, images.values()):
        assert image in command and "container-run" in command
        assert "--platform" not in command and "cargo" not in command
        assert command[-1] == image
    with pytest.raises(ValueError, match="only to Linux"):
        ci.containers({"family": "windows"}, tmp_path, "0.4.0")


def test_floor_images_run_oldest_current_without_building_or_downloading(tmp_path, monkeypatch):
    ci = helper()
    commands, calls = [], []
    monkeypatch.setattr(ci, "run", lambda *args, **kwargs: commands.append(args))
    monkeypatch.setattr(ci, "smoke", lambda *args, **kwargs: calls.append((args, kwargs)))
    item = {"id": "linux-x86_64", "family": "linux"}
    ci.container_run(item, tmp_path, "0.4.0", "musllinux_1_2", "pinned-image")
    assert len(commands) == 2
    install = commands[1]
    assert "--no-index" in install and "--require-hashes" in install and "--find-links" in install
    assert all("cargo" not in command and "build" not in command for command in commands)
    assert [str(call[0][3]) for call in calls] == [
        "/opt/python/cp311-cp311/bin/python", "/opt/python/cp315-cp315/bin/python"]
    assert all(call[1]["environment"] == "musllinux_1_2" and call[1]["image"] == "pinned-image"
               and call[1]["pipx"].name.startswith("pipx") for call in calls)


def test_local_reuses_just_built_binary_without_stamping_source(tmp_path, monkeypatch):
    ci = helper()
    commands, calls = [], []

    def run(*args, **kwargs):
        commands.append((args, kwargs))
        return SimpleNamespace(stdout="a" * 40 if args[:2] == ("git", "rev-parse") else "rustc 1.95\nhost: test\n")

    monkeypatch.setattr(ci, "run", run)
    monkeypatch.setattr(ci, "prepare", lambda directory: calls.append(("prepare", directory)))
    monkeypatch.setattr(ci, "host", lambda *args: calls.append(("host", args)))
    item = {"id": "windows-amd64", "family": "windows", "rust": "x86_64-pc-windows-msvc"}
    ci.local(item, tmp_path / "wheels", "0.4.0")
    build, completion, assembly = commands[1], commands[2], commands[4]
    assert "+crt-static" in build[1]["env"]["RUSTFLAGS"]
    assert "--binary" in completion[0] and "--binary" in assembly[0]
    assert completion[0][completion[0].index("--binary") + 1] == assembly[0][assembly[0].index("--binary") + 1]
    assert "--native-version" in assembly[0] and "--version" in assembly[0]
    assert "gukhanmun-mkdict" in build[0]
    assert assembly[0][assembly[0].index("--mkdict-binary") + 1].name == "gukhanmun-mkdict.exe"
    assert all("stamp" not in command and "sync_version.py" not in command for command, _ in commands)
    assert [call[0] for call in calls] == ["prepare", "host"]


@pytest.mark.parametrize("target", ["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc",
                                  "x86_64-unknown-linux-musl", "aarch64-apple-darwin"])
def test_native_task_and_local_build_share_crt_policy(monkeypatch, target):
    ci = helper()
    calls = []
    monkeypatch.setenv("RUSTFLAGS", "-C debuginfo=1")
    monkeypatch.setattr(ci, "run", lambda *args, **kwargs: calls.append((args, kwargs)))
    ci.build_native(target)
    command, kwargs = calls[0]
    assert command == ("cargo", "build", "--locked", "--release", "--target", target,
                       "-p", "gukhanmun-cli", "-p", "gukhanmun-mkdict")
    expected = "-C debuginfo=1" + (" -C target-feature=+crt-static" if target.endswith("-windows-msvc") else "")
    assert kwargs["env"]["RUSTFLAGS"] == expected
    assert os.environ["RUSTFLAGS"] == "-C debuginfo=1"
    import tomllib
    task = tomllib.loads((ROOT / "mise.toml").read_text())["tasks"]["py-cli-native"]
    assert task["run"] == ["rustup target add {{usage.target}}",
                           "python -I scripts/python/cli_ci.py native --target {{usage.target}}"]


@pytest.mark.parametrize("event", [0, 1])
def test_windows_console_keeps_stdin_open_until_control_exit(monkeypatch, event):
    spec = importlib.util.spec_from_file_location("console_probe", ROOT / "tests/python-cli/windows_console.py")
    probe = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(probe)
    calls = []
    child = SimpleNamespace(returncode=None)

    def wait(timeout):
        calls.append(("wait", timeout))
        child.returncode = 0xC000013A

    def communicate():
        assert child.returncode == 0xC000013A, "EOF must not race control delivery"
        calls.append(("communicate",))

    def handler(callback, enabled):
        calls.append(("handler", enabled))
        return True

    def generate(value, group):
        assert child.returncode is None
        calls.append(("event", value, group))
        return True

    child.poll = lambda: child.returncode
    child.wait = wait
    child.communicate = communicate
    kernel = SimpleNamespace(SetConsoleCtrlHandler=handler, GenerateConsoleCtrlEvent=generate)
    monkeypatch.setattr(probe.ctypes, "WinDLL", lambda *_a, **_k: kernel, raising=False)
    monkeypatch.setattr(probe.ctypes, "WINFUNCTYPE", lambda *_a: lambda function: function, raising=False)
    monkeypatch.setattr(probe.time, "sleep", lambda _: None)
    monkeypatch.setattr(probe.subprocess, "Popen", lambda *_a, **_k: child)
    assert probe.broadcast("gukhanmun.exe", event) == 0xC000013A
    assert calls == [("handler", True), ("event", event, 0), ("wait", 20),
                     ("communicate",), ("handler", False)]

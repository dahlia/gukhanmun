# Gukhanmun: Tests interpreter selection and locked Python test orchestration.
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
import json
import os
from pathlib import Path
import subprocess

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
spec = importlib.util.spec_from_file_location("python_matrix", ROOT / "scripts/python/matrix.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def actual(pin="3.11.18", prefix="/mise/python"):
    return {"implementation": "cpython", "version": [3, 11, 18],
            "implementation_version": [3, 11, 18], "base_prefix": prefix, "free_threaded": False}


def test_complete_matrix_preserves_development_default():
    pins = module.pins()
    assert next(iter(pins)) == "py315"
    assert pins["py311"] == "3.11.18"
    assert pins["pypy312"] == "pypy3.12-8.0.1"


def test_selection_rejects_unknown_empty_and_tox_overrides():
    available = module.pins()
    assert module.selected_names("pypy311,py311,pypy311", available) == ["pypy311", "py311"]
    for selection in ("", "py399", "py311,", "-p", "--skip-uv-sync"):
        with pytest.raises(RuntimeError, match="choose environments"):
            module.selected_names(selection, available)


@pytest.mark.parametrize("field,value", [
    ("implementation", "pypy"), ("version", [3, 11, 17]),
    ("base_prefix", "/usr"), ("free_threaded", True),
])
def test_runtime_rejects_same_minor_system_substitution_and_wrong_abi(field, value):
    data = actual()
    data[field] = value
    with pytest.raises(RuntimeError, match="expected mise Python"):
        module.verify_runtime(data, "3.11.18", "/mise/python")


def test_pypy_checks_language_and_implementation_separately():
    data = actual()
    data.update(implementation="pypy", version=[3, 11, 17], implementation_version=[8, 0, 1])
    module.verify_runtime(data, "pypy3.11-8.0.1", "/mise/python")
    data["implementation_version"] = [8, 0, 0]
    with pytest.raises(RuntimeError, match="expected mise Python"):
        module.verify_runtime(data, "pypy3.11-8.0.1", "/mise/python")


def test_missing_mise_pin_never_falls_back_to_system_python(monkeypatch):
    calls = []

    def missing(command, **kwargs):
        calls.append(command)
        return subprocess.CompletedProcess(command, 1, "", "not installed")

    monkeypatch.setattr(module.subprocess, "run", missing)
    with pytest.raises(RuntimeError, match="mise --env python install"):
        module.interpreter("3.11.18")
    assert calls == [["mise", "where", "python@3.11.18"]]


def test_exact_interpreter_is_probed_before_tox(monkeypatch, tmp_path):
    prefix = tmp_path / "mise"
    executable = prefix / ("python.exe" if os.name == "nt" else "bin/python")
    executable.parent.mkdir(parents=True)
    executable.touch()
    calls = []

    def probe(command, **kwargs):
        calls.append(command)
        return subprocess.CompletedProcess(command, 0, str(prefix) if len(calls) == 1
                                           else json.dumps(actual(prefix=str(prefix))), "")

    monkeypatch.setattr(module.subprocess, "run", probe)
    assert module.interpreter("3.11.18") == (executable, prefix)
    assert calls[1][0] == executable
    assert calls[1][-1] == "--probe"


def test_launcher_controls_frozen_overrides_and_venv_per_serial_invocation(monkeypatch):
    monkeypatch.setattr(module, "interpreter", lambda pin: (Path("/mise") / pin / "python", Path("/mise") / pin))
    for name in ("TOX_OVERRIDE", "TOX_SKIP_ENV", "TOXENV", "PYTHONPATH", "VIRTUAL_ENV"):
        monkeypatch.setenv(name, "unrelated")
    monkeypatch.setenv("UV_FROZEN", "1")
    calls = []
    monkeypatch.setattr(module.subprocess, "run", lambda command, **kwargs: calls.append((command, kwargs)))
    module.matrix("py311,pypy311")
    assert len(calls) == 2
    for (command, kwargs), name in zip(calls, ("py311", "pypy311")):
        assert command[-2:] == ["-e", name]
        environment = kwargs["env"]
        assert environment["UV_FROZEN"] == "false"
        assert environment["UV_PYTHON_DOWNLOADS"] == "never"
        assert Path(environment["GUKHANMUN_PYTHON"]).name == "python"
        assert not any(key in environment for key in ("TOX_OVERRIDE", "TOX_SKIP_ENV", "TOXENV", "PYTHONPATH", "VIRTUAL_ENV"))
        assert kwargs["check"] is True


def test_semantic_requires_launcher_even_for_existing_environment(monkeypatch):
    for name in ("GUKHANMUN_EXPECTED_PIN", "GUKHANMUN_EXPECTED_PREFIX", "GUKHANMUN_PYTHON"):
        monkeypatch.delenv(name, raising=False)
    with pytest.raises(RuntimeError, match="py-test-all"):
        module.semantic()


def test_pinned_lock_runner_ignores_dev_python_and_rejects_stale_lock(tmp_path):
    import shutil
    import sys

    manifest = tmp_path / "pyproject.toml"
    manifest.write_text('[project]\nname="probe"\nversion="0.0.0"\nrequires-python=">=3.11"\n'
                        '[tool.uv]\npackage=false\n[dependency-groups]\ntest=[]\n')
    environment = dict(os.environ, UV_PYTHON=sys.executable, UV_PYTHON_DOWNLOADS="never")
    subprocess.run([shutil.which("uv"), "lock"], cwd=tmp_path, env=environment, check=True,
                   capture_output=True)
    configuration = tmp_path / "tox.ini"
    configuration.write_text(
        '[tox]\nenv_list=probe\nskip_missing_interpreters=false\n'
        '[testenv]\nrunner=uv-venv-lock-runner\npackage=skip\ndependency_groups=test\n'
        'base_python=' + sys.executable + '\nuv_python_preference=only-system\n'
        'set_env=\n UV_FROZEN=false\n UV_PYTHON_DOWNLOADS=never\n'
        'commands=python -I -c "import sys; print(sys.base_prefix)"\n')
    environment.update(UV_PYTHON=str(tmp_path / "missing-python"), UV_FROZEN="1")
    command = [sys.executable, "-m", "tox", "run", "-v", "-c", str(configuration)]
    result = subprocess.run(command, cwd=tmp_path, env=environment, text=True,
                            encoding="utf-8", capture_output=True)
    assert result.returncode == 0, result.stdout + result.stderr
    assert str(Path(sys.base_prefix)) in result.stdout
    assert "--locked" in result.stdout
    manifest.write_text(manifest.read_text().replace('test=[]', 'test=["packaging==26.3"]'))
    stale = subprocess.run(command, cwd=tmp_path, env=environment, text=True,
                           encoding="utf-8", capture_output=True)
    assert stale.returncode != 0
    assert "lockfile" in (stale.stdout + stale.stderr).lower()

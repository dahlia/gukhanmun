# Gukhanmun: Verifies safe replacement of installed-wheel check environments.
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
import sys
import time
from types import SimpleNamespace

import pytest


@pytest.fixture
def lifecycle(tmp_path, monkeypatch):
    source = Path(os.environ['GUKHANMUN_REPO_ROOT']) / 'scripts/python/checks.py'
    spec = importlib.util.spec_from_file_location('wheel_checks_under_test', source)
    checks = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checks)
    root = tmp_path / 'checkout-\uad6d\ud55c\ubb38'
    root.mkdir()
    output = root / 'target/python'
    wheels = output / 'wheels'
    wheels.mkdir(parents=True)
    (wheels / 'example.whl').write_bytes(b'fake wheel; installers are stubbed')
    temporary = tmp_path / 'temporary'
    temporary.mkdir()
    monkeypatch.setattr(checks, 'ROOT', root)
    monkeypatch.setattr(checks, 'OUTPUT', output)
    monkeypatch.setattr(checks, 'STATE', output / 'installed.json')
    monkeypatch.setattr(checks.tempfile, 'gettempdir', lambda: str(temporary))
    monkeypatch.setattr(checks.tempfile, 'tempdir', str(temporary))
    monkeypatch.setattr(sys, 'argv', ['checks.py', 'wheel-env'])
    old = temporary / 'gukhanmun-wheel-old'
    old.mkdir()
    (old / '.gukhanmun-check-env').write_text(str(root), encoding='utf-8')
    (old / 'keep.txt').write_text('old environment contents', encoding='utf-8')
    data = {'directory': str(old), 'python': str(old / 'bin/python'), 'package': str(old / 'package')}
    checks.STATE.write_text(json.dumps(data), encoding='utf-8')
    original = checks.STATE.read_bytes()
    calls = []

    def installer(command, **kwargs):
        calls.append(command)
        directory = Path(kwargs.get('cwd', temporary))
        return subprocess.CompletedProcess(command, 0, json.dumps({
            'package': str(directory / 'package'), 'prefix': str(directory),
        }), '')

    monkeypatch.setattr(checks, 'run', installer)
    return SimpleNamespace(checks=checks, root=root, output=output, temporary=temporary,
                           old=old, original=original, calls=calls, installer=installer,
                           source=source)


def assert_old_preserved(case):
    assert case.checks.STATE.read_bytes() == case.original
    assert (case.old / 'keep.txt').read_text(encoding='utf-8') == 'old environment contents'


def assert_no_failed_artifacts(case):
    assert sorted(case.temporary.iterdir()) == [case.old]
    assert list(case.output.glob('.installed-*.json')) == []


def test_replacement_publishes_complete_new_environment_and_removes_old(lifecycle):
    c = lifecycle
    c.checks.main()
    current = json.loads(c.checks.STATE.read_text(encoding='utf-8'))
    directory = Path(current['directory'])
    assert directory.is_dir()
    assert directory != c.old
    assert current['prefix'] == str(directory)
    assert current['package'] == str(directory / 'package')
    assert (directory / '.gukhanmun-check-env').read_text(encoding='utf-8') == str(c.root)
    assert not c.old.exists()
    assert sorted(c.temporary.iterdir()) == [directory]
    assert not list(c.output.glob('.installed-*.json'))


@pytest.mark.parametrize('stage', ['venv', 'export', 'requirements', 'wheel', 'inspection', 'inspection-status', 'inspection-json'])
def test_failed_installation_preserves_previous_environment(lifecycle, monkeypatch, stage):
    c = lifecycle
    error = RuntimeError(f'injected {stage} failure')
    stage_number = {'venv': 1, 'export': 2, 'requirements': 3, 'wheel': 4,
                    'inspection': 5, 'inspection-status': 5, 'inspection-json': 5}[stage]

    def fail(command, **kwargs):
        result = c.installer(command, **kwargs)
        if len(c.calls) == stage_number:
            if stage == 'inspection-status':
                return subprocess.CompletedProcess(command, 7, result.stdout, 'inspection failed')
            if stage == 'inspection-json':
                return subprocess.CompletedProcess(command, 0, '{invalid json', '')
            raise error
        return result

    monkeypatch.setattr(c.checks, 'run', fail)
    expected = (subprocess.CalledProcessError if stage == 'inspection-status'
                else json.JSONDecodeError if stage == 'inspection-json' else RuntimeError)
    with pytest.raises(expected) as caught:
        c.checks.main()
    if expected is RuntimeError:
        assert caught.value is error
    assert_old_preserved(c)
    assert_no_failed_artifacts(c)


@pytest.mark.parametrize('stage', ['marker-write', 'pending-create', 'pending-write', 'publish'])
def test_failed_state_publication_keeps_previous_state(lifecycle, monkeypatch, stage):
    c = lifecycle
    error = OSError(f'injected {stage} failure')
    original_write = Path.write_text
    original_temporary = c.checks.tempfile.NamedTemporaryFile

    if stage == 'marker-write':
        def write(path, *args, **kwargs):
            if path.name == '.gukhanmun-check-env':
                raise error
            return original_write(path, *args, **kwargs)
        monkeypatch.setattr(Path, 'write_text', write)
    elif stage == 'publish':
        def replace(*args, **kwargs):
            raise error
        monkeypatch.setattr(c.checks.os, 'replace', replace)
    else:
        def temporary(*args, **kwargs):
            if stage == 'pending-create':
                raise error
            real = original_temporary(*args, **kwargs)

            class FailedWriter:
                name = real.name

                def __enter__(self):
                    return self

                def write(self, text):
                    real.write(text[:5])
                    raise error

                def __exit__(self, *exc):
                    real.close()
            return FailedWriter()
        monkeypatch.setattr(c.checks.tempfile, 'NamedTemporaryFile', temporary)
    with pytest.raises(OSError) as caught:
        c.checks.main()
    assert caught.value is error
    assert_old_preserved(c)
    assert_no_failed_artifacts(c)


@pytest.mark.parametrize('kind', [
    'malformed-json', 'non-object', 'missing-directory', 'non-string', 'relative',
    'traversal', 'foreign-location', 'foreign-marker', 'missing-marker',
    'bad-name', 'directory-symlink', 'marker-symlink', 'state-symlink',
])
def test_untrusted_previous_state_never_deletes_its_target(lifecycle, monkeypatch, kind):
    c = lifecycle
    target = c.old
    value = str(target)
    if kind == 'foreign-location':
        target = c.root / 'gukhanmun-wheel-foreign'
        target.mkdir()
        (target / '.gukhanmun-check-env').write_text(str(c.root), encoding='utf-8')
        (target / 'keep.txt').write_text('old environment contents', encoding='utf-8')
        value = str(target)
    elif kind == 'foreign-marker':
        (target / '.gukhanmun-check-env').write_text(str(c.root / 'different-checkout'), encoding='utf-8')
    elif kind == 'missing-marker':
        (target / '.gukhanmun-check-env').unlink()
    elif kind == 'bad-name':
        renamed = target.with_name('unrelated-environment')
        target.rename(renamed)
        target = renamed
        value = str(target)
    elif kind in ('directory-symlink', 'marker-symlink', 'state-symlink'):
        try:
            if kind == 'directory-symlink':
                link = c.temporary / 'gukhanmun-wheel-link'
                link.symlink_to(target, target_is_directory=True)
                value = str(link)
            elif kind == 'marker-symlink':
                marker = target / '.gukhanmun-check-env'
                content = c.root / 'marker-target'
                content.write_text(str(c.root), encoding='utf-8')
                marker.unlink()
                marker.symlink_to(content)
            else:
                external_state = c.root / 'external-state.json'
                c.checks.STATE.rename(external_state)
                c.checks.STATE.symlink_to(external_state)
        except OSError as error:
            pytest.skip(f'symlink creation unavailable: {error}')
    elif kind == 'relative':
        value = target.name
    elif kind == 'traversal':
        value = str(c.temporary / 'unused' / '..' / target.name)
    elif kind == 'non-string':
        value = 42
    if kind != 'state-symlink':
        data = ('{invalid' if kind == 'malformed-json' else '[]' if kind == 'non-object'
                else '{}' if kind == 'missing-directory' else json.dumps({'directory': value}))
        c.checks.STATE.write_text(data, encoding='utf-8')
    c.checks.main()
    assert (target / 'keep.txt').read_text(encoding='utf-8') == 'old environment contents'
    assert Path(json.loads(c.checks.STATE.read_text(encoding='utf-8'))['directory']).is_dir()
    if kind == 'directory-symlink':
        assert link.is_symlink()
    if kind == 'state-symlink':
        assert json.loads(external_state.read_text(encoding='utf-8'))['directory'] == str(c.old)


@pytest.mark.parametrize('installation_fails', [False, True])
def test_cleanup_failure_does_not_replace_primary_result(lifecycle, monkeypatch, capsys, installation_fails):
    c = lifecycle
    original_error = RuntimeError('original installation failure')

    def refuse_cleanup(directory):
        assert Path(directory).parent == c.temporary
        raise PermissionError('cleanup denied')

    monkeypatch.setattr(c.checks.shutil, 'rmtree', refuse_cleanup)
    if installation_fails:
        def fail(command, **kwargs):
            raise original_error
        monkeypatch.setattr(c.checks, 'run', fail)
        with pytest.raises(RuntimeError) as caught:
            c.checks.main()
        assert caught.value is original_error
        assert_old_preserved(c)
    else:
        c.checks.main()
        current = Path(json.loads(c.checks.STATE.read_text(encoding='utf-8'))['directory'])
        assert current.is_dir() and current != c.old
        assert c.old.is_dir()
    assert 'cleanup denied' in capsys.readouterr().err


def wait_for_file(path, process):
    deadline = time.monotonic() + 10
    while not path.exists():
        if process.poll() is not None:
            stdout, stderr = process.communicate()
            pytest.fail(f'child exited before readiness: {stdout}\n{stderr}')
        if time.monotonic() >= deadline:
            pytest.fail(f'child did not signal readiness: {path}')
        time.sleep(0.01)


def test_builder_waits_for_consumer_before_removing_environment(lifecycle):
    c = lifecycle
    # Actual separate interpreters exercise the CLI lock, not an in-process mock.
    setup = '''
import importlib.util, json, pathlib, subprocess, sys
source, root, temporary, mode = sys.argv[1:]
spec = importlib.util.spec_from_file_location('checks_child', source)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
c.ROOT = pathlib.Path(root)
c.OUTPUT = c.ROOT / 'target/python'
c.STATE = c.OUTPUT / 'installed.json'
c.tempfile.gettempdir = lambda: temporary
c.tempfile.tempdir = temporary
if mode == 'consumer':
    def consume():
        directory = pathlib.Path(c.installed()['directory'])
        (c.OUTPUT / 'consumer-ready').write_text(str(directory), encoding='utf-8')
        assert sys.stdin.readline().strip() == 'release'
        assert (directory / 'keep.txt').read_text(encoding='utf-8') == 'old environment contents'
    c.test = consume
    sys.argv = ['checks.py', 'test']
else:
    def install(command, **kwargs):
        directory = pathlib.Path(kwargs.get('cwd', temporary))
        return subprocess.CompletedProcess(command, 0, json.dumps({
            'prefix': str(directory), 'package': str(directory / 'package')}), '')
    c.run = install
    (c.OUTPUT / 'builder-started').write_text('started', encoding='utf-8')
    sys.argv = ['checks.py', 'wheel-env']
c.main()
'''
    processes = []
    try:
        consumer = subprocess.Popen([sys.executable, '-I', '-c', setup, str(c.source),
                                     str(c.root), str(c.temporary), 'consumer'],
                                    stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, text=True)
        processes.append(consumer)
        wait_for_file(c.output / 'consumer-ready', consumer)
        builder = subprocess.Popen([sys.executable, '-I', '-c', setup, str(c.source),
                                    str(c.root), str(c.temporary), 'builder'],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        processes.append(builder)
        wait_for_file(c.output / 'builder-started', builder)
        with pytest.raises(subprocess.TimeoutExpired):
            builder.wait(timeout=0.3)
        assert_old_preserved(c)
        assert_no_failed_artifacts(c)
        stdout, stderr = consumer.communicate('release\n', timeout=10)
        assert consumer.returncode == 0, (stdout, stderr)
        stdout, stderr = builder.communicate(timeout=10)
        assert builder.returncode == 0, (stdout, stderr)
        assert not c.old.exists()
        assert Path(json.loads(c.checks.STATE.read_text(encoding='utf-8'))['directory']).is_dir()
    finally:
        for process in processes:
            if process.poll() is None:
                process.kill()
            process.communicate(timeout=10)


def test_old_environment_ownership_is_rechecked_before_cleanup(lifecycle, monkeypatch):
    c = lifecycle

    def install(command, **kwargs):
        result = c.installer(command, **kwargs)
        if kwargs.get('capture'):
            (c.old / '.gukhanmun-check-env').write_text(str(c.root / 'another-checkout'), encoding='utf-8')
        return result

    monkeypatch.setattr(c.checks, 'run', install)
    c.checks.main()
    assert (c.old / 'keep.txt').read_text(encoding='utf-8') == 'old environment contents'
    current = Path(json.loads(c.checks.STATE.read_text(encoding='utf-8'))['directory'])
    assert current.is_dir() and current != c.old


def test_missing_wheel_does_not_replace_previous_environment(lifecycle):
    c = lifecycle
    for wheel in (c.output / 'wheels').iterdir():
        wheel.unlink()
    with pytest.raises(RuntimeError, match='run py-build'):
        c.checks.main()
    assert_old_preserved(c)
    assert_no_failed_artifacts(c)
    assert c.calls == []


def test_typecheck_reports_package_errors_inside_ignored_environment(lifecycle, monkeypatch):
    c = lifecycle
    package = c.old / 'site-packages' / 'gukhanmun'
    package.mkdir(parents=True)
    (package / '__init__.py').write_text('value: int = "wrong"\n', encoding='utf-8')
    (c.old / '.gitignore').write_text('*\n', encoding='utf-8')
    typing = c.root / 'tests' / 'python' / 'typing'
    typing.mkdir(parents=True)
    for name in ('positive.py', 'negative.py'):
        (typing / name).write_text('', encoding='utf-8')
    monkeypatch.setattr(c.checks, 'installed', lambda: {
        'directory': str(c.old), 'package': str(package), 'python': sys.executable,
    })

    def check(command, **kwargs):
        if command[0] == 'ty':
            return subprocess.run(list(map(str, command)), cwd=kwargs['cwd'],
                                  env=kwargs['env'], capture_output=True, text=True,
                                  encoding='utf-8')
        negative = Path(command[-1]).name == 'negative.py'
        return subprocess.CompletedProcess(command, 1 if negative else 0,
                                           '{"generalDiagnostics": [], "errors": []}', '')

    monkeypatch.setattr(c.checks, 'run', check)
    with pytest.raises(RuntimeError, match='ty positive/package check failed') as caught:
        c.checks.typecheck()
    assert 'invalid-assignment' in str(caught.value)
    assert '__init__.py:1' in str(caught.value)

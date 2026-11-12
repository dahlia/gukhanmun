# Gukhanmun: Executes version-bump task control flow with isolated command doubles.
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
import shlex
import shutil
import subprocess
import sys
import tomllib

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
TASKS = tomllib.loads((ROOT / "mise.toml").read_text())["tasks"]

# External tools are controlled doubles; the task body and Python manifest
# synchronizer are real. No signing, commits, tags or registry access occurs.
DISPATCH = r"""
import json, os
from pathlib import Path
import subprocess, sys, tomllib
root = Path.cwd()
state = json.loads((root / 'state.json').read_text())
command, *args = sys.argv[1:]
state['calls'].append([command, *args])
code = 0
if command == 'git':
    if args == ['rev-parse', 'HEAD']:
        print(state['head'])
    elif args[:3] == ['commit', '-S', '--amend']:
        state['amended'] = True
elif command == 'cargo':
    if '--execute' in args and os.environ.get('FAIL_STAGE') != 'no-bump':
        requested = args[-1]
        version = '0.4.1' if requested == 'patch' else requested
        (root / 'Cargo.toml').write_text('[workspace.package]\nversion = "' + version + '"\n')
        state['head'] = 'bumped'
elif command == 'mise':
    if args == ['run', 'py-sync-version']:
        if os.environ.get('FAIL_STAGE') == 'sync':
            code = 7
        else:
            child = subprocess.run([sys.executable, '-I', os.environ['REAL_SYNC'], str(root)])
            code = child.returncode
            if code == 0:
                version = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
                (root / 'uv.lock').write_text('synchronized ' + version)
                if os.environ.get('FAIL_STAGE') == 'lock':
                    code = 8
elif command == 'sacho':
    if os.environ.get('FAIL_STAGE') == 'sacho':
        code = 9
    else:
        (root / 'changes.d').mkdir(exist_ok=True)
        (root / 'changes.d/next').write_text(args[-1])
        (root / 'CHANGES.md').write_text(args[-1])
(root / 'state.json').write_text(json.dumps(state))
sys.exit(code)
"""


def fixture(tmp_path):
    # Reuse the actual manifest fixture, including all seven extras and data
    # distributions, rather than testing a simplified independent layout.
    spec = importlib.util
    definition = spec.spec_from_file_location('version_fixture', ROOT / 'tests/python/test_version.py')
    module = spec.module_from_spec(definition)
    definition.loader.exec_module(module)
    module.project(tmp_path, '0.4.0')
    (tmp_path / 'state.json').write_text(json.dumps({'head': 'original', 'amended': False, 'calls': []}))
    (tmp_path / 'uv.lock').write_text('original')
    commands = tmp_path / 'commands'
    commands.mkdir()
    dispatcher = commands / 'dispatch.py'
    dispatcher.write_text(DISPATCH)
    for name in ('cargo', 'git', 'mise', 'sacho'):
        if os.name == 'nt':
            (commands / (name + '.cmd')).write_text(f'@"{sys.executable}" "{dispatcher}" {name} %*\r\n')
        else:
            command = commands / name
            command.write_text(f'#!/bin/sh\nexec {shlex.quote(sys.executable)} {shlex.quote(str(dispatcher))} {name} "$@"\n')
            command.chmod(0o755)
    environment = {key: value for key, value in os.environ.items() if key != 'PYPI_VERSION'}
    environment.update(PATH=str(commands) + os.pathsep + os.environ['PATH'],
                       REAL_SYNC=str(ROOT / 'scripts/python/sync_version.py'))
    return environment


def invoke(tmp_path, environment, task='bump-execute', version='patch'):
    environment['usage_version'] = version
    script = tmp_path / 'task.nu'
    script.write_text(TASKS[task]['run'])
    nu = shutil.which('nu')
    assert nu is not None, 'mise must provision the configured Nushell'
    result = subprocess.run([nu, '--no-config-file', str(script)], cwd=tmp_path, env=environment,
                            text=True, capture_output=True)
    state = json.loads((tmp_path / 'state.json').read_text())
    return result, state


@pytest.mark.parametrize('requested,expected', [('patch', '0.4.1'), ('1.2.3', '1.2.3')])
@pytest.mark.parametrize('inherited', [False, True])
def test_bump_uses_applied_version_and_amends_all_python_metadata(tmp_path, requested, expected, inherited):
    environment = fixture(tmp_path)
    if inherited:
        environment['PYPI_VERSION'] = expected + '.dev0'
    result, state = invoke(tmp_path, environment, version=requested)
    assert result.returncode == 0, result.stdout + result.stderr
    assert state['amended']
    versions = [tomllib.loads(path.read_text())['project']['version'] for path in (tmp_path / 'python').glob('*/pyproject.toml')]
    assert len(versions) == 7 and set(versions) == {expected}
    extras = tomllib.loads((tmp_path / 'python/gukhanmun/pyproject.toml').read_text())['project']['optional-dependencies']
    assert all(value[0].endswith('==' + expected) for value in extras.values())
    assert (tmp_path / 'uv.lock').read_text() == 'synchronized ' + expected
    amend = next(call for call in state['calls'] if call[:4] == ['git', 'commit', '-S', '--amend'])
    # Nushell may preserve or expand a glob depending on the external runner;
    # both must name all seven manifests and the lock in the amend operation.
    assert 'uv.lock' in amend
    assert 'python/*/pyproject.toml' in amend or sum(path.endswith('/pyproject.toml') for path in amend) == 7
    staged = next(call for call in state['calls'] if call[:2] == ['git', 'add'])
    assert 'uv.lock' in staged and 'changes.d/next' in staged and 'CHANGES.md' in staged


@pytest.mark.parametrize('stage', ['no-bump', 'sync', 'lock', 'sacho'])
def test_failed_bump_cannot_report_success_or_amend(tmp_path, stage):
    environment = fixture(tmp_path)
    environment['FAIL_STAGE'] = stage
    result, state = invoke(tmp_path, environment)
    assert result.returncode != 0
    assert not state['amended']
    assert not any(call[:2] == ['git', 'add'] for call in state['calls'])


def test_preview_is_nonmutating(tmp_path):
    environment = fixture(tmp_path)
    before = {path.relative_to(tmp_path): path.read_bytes() for path in tmp_path.rglob('*') if path.is_file()}
    result, state = invoke(tmp_path, environment, task='bump')
    assert result.returncode == 0, result.stdout + result.stderr
    assert state['head'] == 'original' and not state['amended']
    assert all(path.read_bytes() == content for name, content in before.items()
               if name != Path('state.json') for path in [tmp_path / name])
    assert state['calls'] == [['cargo', 'release', '--workspace', '--no-publish', '--no-tag', '--no-push', 'patch']]

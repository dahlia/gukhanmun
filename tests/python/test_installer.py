# Gukhanmun: Tests pip selection against controlled stable and development indexes.
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
import zipfile

import pytest

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    sys.modules[name] = value
    spec.loader.exec_module(value)
    return value


smoke = module('installer_smoke', ROOT / 'scripts/python/wheel_smoke.py')
table = module('installer_data', ROOT / 'python/gukhanmun/src/gukhanmun/_dictionary_data.py')


def wheel(directory, name, version):
    normalized = name.replace('-', '_')
    info = f'{normalized}-{version}.dist-info'
    metadata = f'Metadata-Version: 2.4\nName: {name}\nVersion: {version}\nRequires-Python: >=3.11\n'
    if name == 'gukhanmun':
        for extra, distribution in table.EXTRAS.items():
            metadata += f'Provides-Extra: {extra}\nRequires-Dist: {distribution}=={version}; extra == "{extra}"\n'
    path = directory / f'{normalized}-{version}-py3-none-any.whl'
    with zipfile.ZipFile(path, 'w') as archive:
        archive.writestr(f'{normalized}/__init__.py', '')
        archive.writestr(f'{info}/METADATA', metadata)
        archive.writestr(f'{info}/WHEEL', 'Wheel-Version: 1.0\nGenerator: selection-fixture\nRoot-Is-Purelib: true\nTag: py3-none-any\n')
        archive.writestr(f'{info}/RECORD', '')
    return path


@pytest.fixture(scope='session')
def pip_requirements(tmp_path_factory):
    requirements = tmp_path_factory.mktemp('locked-installer') / 'requirements.txt'
    subprocess.run(['uv', 'export', '--locked', '--only-group', 'installer', '--no-emit-workspace',
                    '--output-file', str(requirements)], cwd=ROOT, check=True, capture_output=True)
    assert 'pip==26.0' in requirements.read_text()
    assert '--hash=sha256:' in requirements.read_text()
    return requirements


@pytest.fixture
def installer(tmp_path, pip_requirements):
    venv = tmp_path / 'venv'
    subprocess.run([*smoke.venv_command(sys.executable, str(venv)), '--without-pip'], check=True)
    executable = smoke.venv_python(venv)
    subprocess.run(['uv', '--no-config', 'pip', 'install', '--python', str(executable),
                    '--require-hashes', '-r', str(pip_requirements)], check=True, capture_output=True,
                   env={key: value for key, value in os.environ.items()
                        if not key.startswith(('PIP_', 'UV_PUBLISH_', 'UV_INDEX', 'UV_DEFAULT_INDEX'))})
    environment = smoke.runtime_environment(venv)
    environment['PIP_CONFIG_FILE'] = os.devnull
    version = subprocess.check_output([executable, '-I', '-m', 'pip', '--version'],
                                     env=environment, text=True).split()[1]
    assert version == '26.0'
    work = tmp_path / 'work'
    work.mkdir()
    serial = 0

    def install(index, requirement, *options):
        nonlocal serial
        serial += 1
        report = work / f'install-{serial}.json'
        result = subprocess.run([executable, '-I', '-m', 'pip', '--isolated', '--disable-pip-version-check',
                                 'install', '--no-cache-dir', '--no-input', '--only-binary=:all:',
                                 '--index-url', index.as_uri(), '--report', str(report), *options, requirement],
                                cwd=work, env=environment, text=True, capture_output=True)
        assert result.returncode == 0, result.stdout + result.stderr
        data = json.loads(report.read_text())
        for item in data["install"]:
            path = smoke.report_path(item)
            assert path.is_relative_to(index.resolve()), f"pip escaped the controlled index: {path}"
            assert smoke.sha256(path) == item["download_info"]["archive_info"]["hashes"]["sha256"]
        return data

    def versions():
        result = subprocess.check_output([executable, '-I', '-c',
            'import importlib.metadata,json; print(json.dumps({d.metadata["Name"]:d.version for d in importlib.metadata.distributions()}))'],
            cwd=work, env=environment, text=True)
        return json.loads(result)

    return install, versions


def index(tmp_path, versions):
    dist = tmp_path / 'dist'
    dist.mkdir(parents=True)
    found = {}
    for version in versions:
        for name in ['gukhanmun', *(entry.name for entry in table.DISTRIBUTIONS)]:
            path = wheel(dist, name, version)
            found[path.name] = path
    simple, _ = smoke.build_index(found, tmp_path / 'index')
    return simple


def test_fresh_install_prefers_stable_and_explicit_pre_upgrade_selects_dev(tmp_path, installer):
    simple = index(tmp_path, ['0.3.1', '0.4.0.dev12'])
    install, versions = installer
    install(simple, 'gukhanmun')
    assert versions()['gukhanmun'] == '0.3.1'
    install(simple, 'gukhanmun', '--upgrade', '--pre')
    assert versions()['gukhanmun'] == '0.4.0.dev12'


def test_dev_only_index_falls_back_without_pre(tmp_path, installer):
    simple = index(tmp_path, ['0.4.0.dev12'])
    install, versions = installer
    install(simple, 'gukhanmun')
    assert versions()['gukhanmun'] == '0.4.0.dev12'


def test_incompatible_older_stable_does_not_block_dev_fallback(tmp_path, installer):
    simple = index(tmp_path, ['0.3.1', '0.4.0.dev12'])
    install, versions = installer
    install(simple, 'gukhanmun>0.3.1')
    assert versions()['gukhanmun'] == '0.4.0.dev12'


@pytest.mark.parametrize('version', ['0.3.1', '0.4.0.dev12'])
def test_exact_extra_pins_resolve_every_data_project(tmp_path, installer, version):
    simple = index(tmp_path, ['0.3.1', '0.4.0.dev12'])
    install, versions = installer
    install(simple, f'gukhanmun[{",".join(table.EXTRAS)}]=={version}')
    installed = versions()
    assert {installed[name] for name in ['gukhanmun', *(entry.name for entry in table.DISTRIBUTIONS)]} == {version}


def test_satisfying_installed_dev_remains_without_upgrade(tmp_path, installer):
    simple = index(tmp_path, ['0.3.1', '0.4.0.dev12'])
    install, versions = installer
    install(simple, 'gukhanmun==0.4.0.dev12')
    report = install(simple, 'gukhanmun')
    assert report['install'] == []
    assert versions()['gukhanmun'] == '0.4.0.dev12'

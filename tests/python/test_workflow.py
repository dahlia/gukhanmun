# Gukhanmun: Checks CI publication permission and immutable-artifact wiring contracts.
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

import os
from pathlib import Path
import re

ROOT = Path(os.environ["GUKHANMUN_REPO_ROOT"])
SOURCE = (ROOT / ".github/workflows/main.yaml").read_text()


def job(name):
    match = re.search(r"(?ms)^  " + re.escape(name) + r":\n(.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)", SOURCE)
    assert match is not None
    return match[1]


def test_builds_and_offline_validation_cannot_request_python_oidc():
    for name in ('python-context', 'python-sdist', 'python-wheels', 'python-wheel-test', 'python-dists', 'python-publish-check'):
        assert 'id-token:' not in job(name)
    upload = job('publish-python')
    assert 'id-token: write' in upload and 'persist-credentials: false' in upload
    gate = upload.split('if:', 1)[1].split('timeout-minutes:', 1)[0]
    assert "github.repository == 'dahlia/gukhanmun'" in gate
    assert "github.event_name == 'push'" in gate
    assert "needs.artifact-context.outputs.should-publish == 'true'" in gate
    # Otherwise-required but unsupported Python publication must fail loudly,
    # rather than a green job skip based on Python-specific policy outputs.
    assert 'python-eligible' not in gate and 'PYTHON_ELIGIBLE' not in gate
    for required in ('ci', 'python-semantic', 'artifact-context', 'python-context', 'python-dists', 'python-publish-check'):
        assert required in upload.split('needs:', 1)[1].split('if:', 1)[0]
    assert 'artifacts.py stamp' not in upload and 'maturin' not in upload
    assert 'publish.py publish' in upload


def test_every_consumer_requires_original_identity_before_download():
    for name in ('python-publish-check', 'publish-python', 'release'):
        text = job(name)
        assert 'artifact-ids: ${{ needs.python-dists.outputs.artifact-id }}' in text
        assert text.index('Require the original Python artifact identity') < text.index('artifact-ids:')
        assert 'MANIFEST_SHA256: ${{ needs.python-dists.outputs.manifest-sha256 }}' in text
        assert '--manifest-sha256 "$MANIFEST_SHA256"' in text or 'sha256sum --check --strict' in text
        assert 'overwrite: true' not in text


def test_python_publisher_pins_external_actions():
    # Non-GitHub actions in this OIDC job must not follow movable refs.
    for action in re.findall(r"(?m)^ +(?:- +)?uses: +(\S+)", job('publish-python')):
        if action.startswith(('actions/', './')):
            continue
        assert re.fullmatch(r"[^@\s]+@[0-9a-f]{40}", action), action


def test_bundle_is_fully_validated_before_last_immutable_upload():
    text = job('python-dists')
    assert text.index('artifacts.py verify') < text.index('publish.py validate') < text.index('name: python-dists')
    assert 'retention-days: 90' in text and 'steps.upload.outputs.artifact-id' in text
    assert 'steps.manifest.outputs.manifest-sha256' in text
    assert text.rstrip().endswith('if-no-files-found: error')


def test_attempt_receipts_and_release_use_the_original_files():
    assert 'python-validation-receipt-${{ github.run_attempt }}' in job('python-publish-check')
    assert 'python-publish-receipt-${{ github.run_attempt }}' in job('publish-python')
    text = job('release')
    dependencies = text.split('needs:', 1)[1].split('if:', 1)[0]
    assert '- publish-python' in dependencies and '- python-dists' in dependencies
    assert 'artifacts.py verify --dir python-dists --complete' in text
    assert 'python-manifest.json' in text and 'python-SHA256SUMS' in text
    assert '"${#files[@]}" -eq 31' in text
    assert 'publish.py publish' not in text


def test_shared_context_and_tag_binding_are_in_the_verified_full_depth_job():
    text = job('artifact-context')
    assert 'fetch-depth: 0' in text
    assert 'scripts/python/verify_tag.py' in text and '--ref-object "$tag_object"' in text
    assert 'scripts/python/verify_tag.py' not in job('publish-python')
    assert 'python-version: ${{ steps.python-version.outputs.python-version }}' in text
    assert 'continue-on-error: true' in text and 'scripts/python/release_context.py' in text
    assert text.index('Resolve artifact version') < text.index('Resolve the independent Python publication context')

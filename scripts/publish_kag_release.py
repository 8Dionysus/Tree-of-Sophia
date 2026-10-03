#!/usr/bin/env python3
"""Native local KAG release entry; selected aoa-kag programs remain host adapters.

The prior implementation remains in publish_kag_release_legacy_oracle.py for
independent compatibility assertions. Maintained calls never fall back to it.
"""
from __future__ import annotations
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from corpus_store import CorpusStoreError
ROOT = Path(__file__).resolve().parents[1]
PROGRAM_PATHS = tuple(sorted((
    'scripts/build_repo_local_kag_release.py', 'scripts/query_repo_local_kag.py',
    'scripts/validators/local_kag_subtree.py', 'scripts/validators/repo_local_kag_index.py',
)))

def _native(arguments: list[str]) -> dict:
    selected = os.environ.get('TOS_KAG_RELEASE_BIN') or shutil.which('tos-kag-release')
    if not selected:
        raise CorpusStoreError('install tos-kag-release or set TOS_KAG_RELEASE_BIN')
    completed = subprocess.run(
        [selected, '--repo-root', str(ROOT), '--python', str(Path(sys.executable).resolve()), *arguments],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False, timeout=620,
    )
    if completed.returncode:
        message = completed.stderr.decode('utf-8', 'replace').strip()
        raise CorpusStoreError(message or f'native KAG release exited {completed.returncode}')
    try:
        result = json.loads(completed.stdout)
    except (ValueError, UnicodeError) as error:
        raise CorpusStoreError('native KAG release returned invalid JSON') from error
    if not isinstance(result, dict):
        raise CorpusStoreError('native KAG release returned an invalid object')
    return result

def build_release(store_root: Path, revision: str, kag_root: Path, release_root: Path) -> dict:
    return _native(['build', '--store', str(store_root), '--revision', revision,
                    '--kag-root', str(kag_root), '--release-root', str(release_root)])

def status_release(release_root: Path, expected_revision: str) -> dict:
    return _native(['status', '--release-root', str(release_root), '--expected-revision', expected_revision])

def _verify_integration(root: Path, *, expected_revision: str) -> dict:
    return _native(['verify', '--release', str(root), '--expected-revision', expected_revision])

def main(argv: list[str] | None = None) -> None:
    result = _native(list(sys.argv[1:] if argv is None else argv))
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(',', ':')))

if __name__ == '__main__':
    try:
        main()
    except (CorpusStoreError, OSError, ValueError) as error:
        raise SystemExit(f'KAG release rejected: {error}')

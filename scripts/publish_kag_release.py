#!/usr/bin/env python3
"""Native local KAG release entry; selected aoa-kag programs remain host adapters.

The prior implementation remains in publish_kag_release_legacy_oracle.py for
independent compatibility assertions. Maintained calls never fall back to it.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
from corpus_store import CorpusStoreError
ROOT = Path(__file__).resolve().parents[1]
PROGRAM_PATHS = tuple(sorted((
    'scripts/build_repo_local_kag_release.py', 'scripts/query_repo_local_kag.py',
    'scripts/validators/local_kag_subtree.py', 'scripts/validators/repo_local_kag_index.py',
)))

def _native(arguments: list[str]) -> dict:
    if arguments and arguments[0] in ('export-build', 'export-verify'):
        selected = os.environ.get('TOS_OPS_MECHANICS_EXECUTOR') or shutil.which('tos-ops-mechanics-plan')
        if not selected:
            raise CorpusStoreError('install tos-ops-mechanics-plan or set TOS_OPS_MECHANICS_EXECUTOR')
        if arguments[0] == 'export-build':
            command = [selected, '--repo-root', str(ROOT), '--kag-source-export-build', *arguments[1:]]
        else:
            if len(arguments) != 3 or arguments[1] != '--release':
                raise CorpusStoreError('source export verification requires one exact release path')
            command = [selected, '--repo-root', str(ROOT), '--kag-source-export-verify', '--kag-export', arguments[2]]
    else:
        selected = os.environ.get('TOS_KAG_RELEASE_BIN') or shutil.which('tos-kag-release')
        if not selected:
            raise CorpusStoreError('install tos-kag-release or set TOS_KAG_RELEASE_BIN')
        command = [selected, '--repo-root', str(ROOT), '--python', str(Path(sys.executable).resolve()), *arguments]
    child = subprocess.Popen(
        command,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    try:
        stdout, stderr = child.communicate(timeout=620)
    except subprocess.TimeoutExpired as error:
        # Let the native subreaper perform its owned cleanup before escalating.
        child.send_signal(signal.SIGTERM)
        try:
            child.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.communicate(timeout=5)
            raise CorpusStoreError('native KAG deadline; forced leader exit, descendant custody requires host review') from error
        raise CorpusStoreError('native KAG release deadline expired') from error
    if child.returncode:
        message = stderr.decode('utf-8', 'replace').strip()
        raise CorpusStoreError(message or f'native KAG release exited {child.returncode}')
    if len(stdout) > 16 * 1024 * 1024 or len(stderr) > 16 * 1024 * 1024:
        raise CorpusStoreError('native KAG output exceeds its declared wire bound')
    try:
        result = json.loads(stdout)
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
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    build = commands.add_parser('build')
    build.add_argument('--store', type=Path, required=True)
    build.add_argument('--revision', required=True)
    build.add_argument('--kag-root', type=Path, required=True)
    build.add_argument('--release-root', type=Path, required=True)
    status = commands.add_parser('status')
    status.add_argument('--release-root', type=Path, required=True)
    status.add_argument('--expected-revision', required=True)
    args = parser.parse_args(argv)
    result = (build_release(args.store, args.revision, args.kag_root, args.release_root)
              if args.command == 'build' else status_release(args.release_root, args.expected_revision))
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(',', ':')))

if __name__ == '__main__':
    try:
        main()
    except (CorpusStoreError, OSError, ValueError) as error:
        raise SystemExit(f'KAG release rejected: {error}')

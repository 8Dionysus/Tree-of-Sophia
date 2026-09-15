#!/usr/bin/env python3
"""Prepare and select a verified software/data pair without deploying a site."""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
from pathlib import Path
import sys
import zipfile

ACCESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS_ROOT / 'src'))
sys.path.insert(0, str(ACCESS_ROOT / 'packaging'))
from tos_access.data_snapshot import verify_data_snapshot  # noqa: E402
from tos_access.release_state import ReleaseStore  # noqa: E402
from validate_software_bundle import verify_archive  # noqa: E402


def sha256(path: Path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def _reader_abi(software_archive: Path) -> tuple[str, str]:
    # Read constants from the verified archive without importing its code.
    with zipfile.ZipFile(software_archive) as archive:
        tree = ast.parse(archive.read('access/src/tos_access/query_store.py'))
    values = {}
    for statement in tree.body:
        if isinstance(statement, ast.Assign) and isinstance(statement.value, ast.Constant):
            for target in statement.targets:
                if isinstance(target, ast.Name) and target.id in {'SCHEMA', 'COMPILER_VERSION'}:
                    if target.id in values or not isinstance(statement.value.value, str):
                        raise ValueError('software reader ABI is ambiguous')
                    values[target.id] = statement.value.value
    if set(values) != {'SCHEMA', 'COMPILER_VERSION'}:
        raise ValueError('software archive has no supported reader ABI declaration')
    return values['SCHEMA'], values['COMPILER_VERSION']


def prepare_pair(software_archive: Path, data_root: Path) -> dict:
    software_archive, data_root = Path(software_archive).absolute(), Path(data_root).absolute()
    if (software_archive.is_symlink() or data_root.is_symlink()
            or software_archive.resolve() != software_archive or data_root.resolve() != data_root):
        raise ValueError('release subjects must not be symlinks')
    software = verify_archive(software_archive)
    if software['source_dirty'] is not False:
        raise ValueError('release pair needs a clean source-bound software archive')
    data = verify_data_snapshot(data_root, require_compatible=False)
    schema, version = _reader_abi(software_archive)
    if data['compiler']['schema'] != schema or data['compiler']['compiler_version'] != version:
        raise ValueError('software reader and data snapshot are incompatible')
    return {'schema_version': 'tos_access_release_pair_v1',
            'software_sha256': sha256(software_archive),
            'data_revision': data['data_revision'],
            'data_manifest_sha256': sha256(data_root / 'manifest.json'),
            'corpus_revision': data['corpus_revision'],
            'query_schema': schema, 'compiler_version': version}


def verify_pair(pair: dict, bindings: dict) -> None:
    actual = prepare_pair(Path(bindings['software_archive']), Path(bindings['data_root']))
    if actual != pair:
        raise ValueError('release pair differs from the exact bound artifacts')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    prepare = commands.add_parser('prepare')
    prepare.add_argument('--software', type=Path, required=True)
    prepare.add_argument('--data', type=Path, required=True)
    prepare.add_argument('--output', type=Path, required=True)
    promote = commands.add_parser('promote')
    promote.add_argument('--state', type=Path, required=True)
    promote.add_argument('--pair', type=Path, required=True)
    promote.add_argument('--software', type=Path, required=True)
    promote.add_argument('--data', type=Path, required=True)
    base = promote.add_mutually_exclusive_group(required=True)
    base.add_argument('--initial', action='store_true')
    base.add_argument('--expected-current')
    rollback = commands.add_parser('rollback')
    rollback.add_argument('--state', type=Path, required=True)
    rollback.add_argument('--expected-current', required=True)
    revoke = commands.add_parser('revoke')
    revoke.add_argument('--state', type=Path, required=True)
    revoke.add_argument('--kind', choices=['data', 'corpus', 'software'], required=True)
    revoke.add_argument('--digest', required=True)
    revoke.add_argument('--reason', required=True)
    revoke.add_argument('--owner-ref', required=True)
    status = commands.add_parser('status')
    status.add_argument('--state', type=Path, required=True)
    args = parser.parse_args(argv)
    if args.command == 'prepare':
        result = prepare_pair(args.software, args.data)
        with args.output.open('x') as stream:
            stream.write(json.dumps(result, sort_keys=True, separators=(',', ':')) + '\n')
    elif args.command == 'promote':
        result = ReleaseStore(args.state).promote(json.loads(args.pair.read_bytes()),
            {'data_root': str(args.data.absolute()), 'software_archive': str(args.software.absolute())},
            expected_current=args.expected_current, verify_pair=verify_pair)
    elif args.command == 'rollback':
        result = ReleaseStore(args.state, create=False).rollback(
            expected_current=args.expected_current, verify_pair=verify_pair)
    elif args.command == 'revoke':
        result = ReleaseStore(args.state, create=False).revoke(args.kind, args.digest,
            reason=args.reason, owner_ref=args.owner_ref)
    else:
        result = ReleaseStore(args.state, create=False).read_selection()
    print(json.dumps(result, sort_keys=True, ensure_ascii=False, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

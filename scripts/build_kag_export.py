#!/usr/bin/env python3
"""Build the bounded ToS source-return export from one accepted corpus revision.

This command never imports aoa-kag, builds its indexes, or publishes a service.
Its consumer receives exact source bytes and the corpus revision that owns them.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import tempfile

from corpus_store import (CorpusStore, CorpusStoreError, canonical, digest_file,
                          hex_digest, read_json, regular, _rename_new, _sync_dir)

PROGRAM_ROOT = Path(__file__).resolve().parents[1]
GENERATOR = Path('mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py')
PRIMARY = 'ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json'
SOURCE_PATHS = (
    PRIMARY,
    'ToS/derived-exports/README.md',
    'ToS/public-compatibility/concept_node.example.json',
    'ToS/public-compatibility/source_node.example.json',
    'ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md',
    'ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md',
)
CAPSULE = 'ToS/derived-exports/kag_export.min.json'
SCHEMA = 'tos_kag_source_export_v1'
MAX_SOURCE_BYTES = 8 * 1024 * 1024


def _generator():
    path = PROGRAM_ROOT / GENERATOR
    regular(path)
    spec = importlib.util.spec_from_file_location('tos_owned_kag_export', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _producer_sha256():
    return hashlib.sha256(canonical({
        'scripts/build_kag_export.py': digest_file(Path(__file__)),
        'scripts/corpus_store.py': digest_file(PROGRAM_ROOT / 'scripts/corpus_store.py'),
        GENERATOR.as_posix(): digest_file(PROGRAM_ROOT / GENERATOR),
    })).hexdigest()


def verify_export(root: Path) -> dict:
    root = Path(root).absolute()
    if root.resolve() != root or not root.is_dir():
        raise CorpusStoreError('KAG export must be an explicit regular directory')
    manifest = read_json(root / 'export.json')
    keys = {'schema_version', 'corpus_revision', 'producer_sha256', 'primary_source',
            'files', 'export_revision'}
    if set(manifest) != keys or manifest['schema_version'] != SCHEMA:
        raise CorpusStoreError('unsupported KAG source export')
    for key in ('corpus_revision', 'producer_sha256', 'export_revision'):
        hex_digest(manifest[key])
    body = {key: value for key, value in manifest.items() if key != 'export_revision'}
    if hashlib.sha256(canonical(body)).hexdigest() != manifest['export_revision']:
        raise CorpusStoreError('KAG export identity mismatch')
    if not isinstance(manifest['files'], list):
        raise CorpusStoreError('KAG export files must be an exact list')
    expected = sorted((*SOURCE_PATHS, CAPSULE))
    if [entry.get('path') for entry in manifest['files'] if isinstance(entry, dict)] != expected:
        raise CorpusStoreError('KAG export source membership differs')
    by_path = {}
    total = 0
    for entry in manifest['files']:
        if set(entry) != {'path', 'sha256', 'size_bytes'}:
            raise CorpusStoreError('invalid KAG source binding')
        hex_digest(entry['sha256'])
        if type(entry['size_bytes']) is not int or entry['size_bytes'] < 0:
            raise CorpusStoreError('invalid KAG source byte size')
        total += entry['size_bytes']
        if total > MAX_SOURCE_BYTES:
            raise CorpusStoreError('KAG source export exceeds its bounded capsule budget')
        path = root / 'Tree-of-Sophia' / entry['path']
        if regular(path).st_size != entry['size_bytes'] or digest_file(path) != entry['sha256']:
            raise CorpusStoreError('KAG exported source digest mismatch')
        by_path[entry['path']] = entry
    observed = set()
    for path in root.rglob('*'):
        if path.is_symlink():
            raise CorpusStoreError('KAG export may not contain symlinks')
        if not path.is_dir():
            regular(path)
            observed.add(path.relative_to(root).as_posix())
    if observed != {'export.json', *('Tree-of-Sophia/' + path for path in expected)}:
        raise CorpusStoreError('KAG export contains undeclared files')
    source_root = root / 'Tree-of-Sophia'
    node = json.loads((source_root / PRIMARY).read_bytes())
    capsule = json.loads((source_root / CAPSULE).read_bytes())
    if not isinstance(node, dict) or not isinstance(capsule, dict):
        raise CorpusStoreError('KAG source node and capsule must be objects')
    primary = {'record_id': node.get('node_id'), 'path': PRIMARY,
               'sha256': by_path[PRIMARY]['sha256'], 'corpus_revision': manifest['corpus_revision']}
    if (not isinstance(primary['record_id'], str) or not primary['record_id']
            or manifest['primary_source'] != primary or capsule.get('object_id') != primary['record_id']):
        raise CorpusStoreError('KAG capsule does not return its exact canonical source')
    # The receiver verifies bytes and the stable source-return contract. A new
    # exporter program does not invalidate an older byte-bound export.
    entry = capsule.get('entry_surface')
    mirror = json.loads((source_root / 'ToS/public-compatibility/source_node.example.json').read_bytes())
    expected_entry = {'repo': 'Tree-of-Sophia',
                      'path': 'ToS/public-compatibility/source_node.example.json',
                      'match_key': 'node_id', 'match_value': primary['record_id']}
    if (capsule.get('owner_repo') != 'Tree-of-Sophia'
            or capsule.get('kind') != 'source_node'
            or entry != expected_entry or not isinstance(mirror, dict)
            or mirror.get('node_id') != primary['record_id']
            or not isinstance(mirror.get('interpretation_layers'), list)
            or not mirror['interpretation_layers']
            or any(not isinstance(layer, str) or not layer for layer in mirror['interpretation_layers'])
            or capsule.get('section_handles') != mirror['interpretation_layers']):
        raise CorpusStoreError('invalid KAG source-return capsule')
    relations = [
        {'relation_type': kind, 'target_ref': 'Tree-of-Sophia/' + relative}
        for kind, relative in (
            ('bounded_hop', 'ToS/public-compatibility/concept_node.example.json'),
            ('capsule_surface', 'ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md'),
            ('tiny_entry_route', 'ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md'))]
    if capsule.get('direct_relations') != relations:
        raise CorpusStoreError('KAG relation leaves the exact exported source contract')
    for field in ('primary_question', 'summary_50', 'summary_200', 'provenance_note',
                  'non_identity_boundary'):
        if not isinstance(capsule.get(field), str) or not capsule[field].strip():
            raise CorpusStoreError('KAG capsule is missing its source-return explanation')
    return manifest


def build_export(store_root: Path, revision: str, output: Path) -> dict:
    store_root, output = Path(store_root).absolute(), Path(output).absolute()
    if (not store_root.is_dir() or not (store_root / 'revisions').is_dir()
            or store_root.resolve() != store_root):
        raise CorpusStoreError('select an existing corpus store')
    if output.exists() or output.is_symlink() or output.resolve() != output:
        raise CorpusStoreError('KAG export output must be a new regular path')
    store = CorpusStore(store_root)
    source = store.load(revision)
    entries = {entry['path']: entry for entry in source['files']}
    if not set(SOURCE_PATHS) <= entries.keys():
        raise CorpusStoreError('accepted corpus is missing the bounded KAG source closure')
    if sum(entries[path]['size_bytes'] for path in SOURCE_PATHS) > MAX_SOURCE_BYTES:
        raise CorpusStoreError('KAG source inputs exceed the bounded capsule budget')
    before = _producer_sha256()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.kag-export-', dir=output.parent) as raw:
        stage = Path(raw) / 'export'
        source_root = stage / 'Tree-of-Sophia'
        for relative in SOURCE_PATHS:
            entry = entries[relative]
            store._verify_object(entry)
            target = source_root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(store._object(entry['sha256']), target)
            if digest_file(target) != entry['sha256']:
                raise CorpusStoreError('corpus source changed during export')
        payload = _generator().build_kag_export_payload(source_root)
        (source_root / CAPSULE).write_bytes(canonical(payload))
        files = [{'path': relative, 'sha256': digest_file(source_root / relative),
                  'size_bytes': (source_root / relative).stat().st_size}
                 for relative in sorted((*SOURCE_PATHS, CAPSULE))]
        node = json.loads((source_root / PRIMARY).read_bytes())
        body = {'schema_version': SCHEMA, 'corpus_revision': source['revision'],
                'producer_sha256': before, 'files': files,
                'primary_source': {'record_id': node['node_id'], 'path': PRIMARY,
                    'sha256': entries[PRIMARY]['sha256'], 'corpus_revision': source['revision']}}
        manifest = {**body, 'export_revision': hashlib.sha256(canonical(body)).hexdigest()}
        (stage / 'export.json').write_bytes(canonical(manifest))
        verify_export(stage)
        if _producer_sha256() != before:
            raise CorpusStoreError('KAG exporter changed during construction')
        for path in stage.rglob('*'):
            if path.is_file():
                with path.open('rb') as stream:
                    os.fsync(stream.fileno())
        for directory in sorted((p for p in stage.rglob('*') if p.is_dir()),
                                key=lambda p: len(p.parts), reverse=True):
            _sync_dir(directory)
        _sync_dir(stage)
        _rename_new(stage, output)
        _sync_dir(output.parent)
    return manifest


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    build = commands.add_parser('build')
    build.add_argument('--store', type=Path, required=True)
    build.add_argument('--revision', required=True)
    build.add_argument('--output', type=Path, required=True)
    verify = commands.add_parser('verify')
    verify.add_argument('export', type=Path)
    args = parser.parse_args(argv)
    try:
        result = (verify_export(args.export) if args.command == 'verify' else
                  build_export(args.store, args.revision, args.output))
    except (CorpusStoreError, ValueError, OSError, KeyError, TypeError) as error:
        parser.exit(1, f'KAG export rejected: {error}\n')
    print(json.dumps({'export_revision': result['export_revision'],
                      'corpus_revision': result['corpus_revision'],
                      'primary_source': result['primary_source']}))


if __name__ == '__main__':
    main()

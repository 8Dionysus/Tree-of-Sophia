#!/usr/bin/env python3
"""Admit one exact source batch using the program-owned source validator.

Batch paths are relative to an explicitly selected input root. Nothing in a
batch can select executable code, change an owner judgment, or skip a bad row.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from corpus_store import CorpusStore, CorpusStoreError, canonical, hex_digest, read_json, relative_path
from corpus_source_validation import SourceValidator, is_source_member

BATCH_SCHEMA = 'tos_corpus_batch_v1'
BATCH_KEYS = {'schema_version', 'base_revision', 'validator_sha256', 'updates', 'retirements'}


def read_batch(path: Path, input_root: Path) -> tuple[dict, dict, dict]:
    batch = read_json(path)
    if set(batch) != BATCH_KEYS or batch['schema_version'] != BATCH_SCHEMA:
        raise CorpusStoreError('unsupported source batch')
    if batch['base_revision'] is not None:
        hex_digest(batch['base_revision'])
    hex_digest(batch['validator_sha256'])
    input_root = Path(input_root).absolute()
    if input_root != input_root.resolve() or not input_root.is_dir():
        raise CorpusStoreError('batch input root must be an explicit regular directory')
    if not isinstance(batch['updates'], list) or not isinstance(batch['retirements'], list):
        raise CorpusStoreError('batch updates and retirements must be arrays')
    updates, retirements = {}, {}
    for row in batch['updates']:
        if not isinstance(row, dict) or set(row) != {'path', 'sha256', 'size_bytes', 'mode'}:
            raise CorpusStoreError('source update must bind exact bytes and mode')
        relative = relative_path(row['path'])
        if not is_source_member(relative) or relative in updates:
            raise CorpusStoreError('duplicate or non-source update')
        source = input_root / relative
        if source.is_symlink() or source.resolve() != source.absolute() or not source.is_file():
            raise CorpusStoreError('source update must be an existing regular input file')
        updates[relative] = {key: value for key, value in row.items() if key != 'path'}
        updates[relative]['source'] = source
    for row in batch['retirements']:
        if not isinstance(row, dict) or set(row) != {'path', 'event_ref', 'event_sha256'}:
            raise CorpusStoreError('retirement needs an exact event binding')
        relative = relative_path(row['path'])
        if not is_source_member(relative) or relative in retirements or relative in updates:
            raise CorpusStoreError('duplicate or conflicting retirement')
        event_ref = relative_path(row['event_ref'])
        if not is_source_member(event_ref):
            raise CorpusStoreError('retirement event must be a source member')
        retirements[relative] = {
            'event_ref': event_ref,
            'event_sha256': hex_digest(row['event_sha256']),
        }
    return batch, updates, retirements


def admit_batch(store_root: Path, batch_path: Path, input_root: Path,
                grammar_root: Path, *, payload_source_root: Path | None = None,
                historical_capture: Path | list[Path] | None = None,
                historical_root: Path | list[Path] | None = None) -> dict:
    batch, updates, retirements = read_batch(batch_path, input_root)
    validator_options = {'payload_source_root': payload_source_root}
    if historical_capture is not None or historical_root is not None:
        validator_options.update(historical_capture=historical_capture, historical_root=historical_root)
    validator = SourceValidator(grammar_root, **validator_options)
    if validator.sha256 != batch['validator_sha256']:
        raise CorpusStoreError('batch schema/validator identity does not match the selected program and grammar')
    store = CorpusStore(store_root)
    snapshot = store.admit(base_revision=batch['base_revision'], updates=updates,
        retirements=retirements, validator_sha256=validator.sha256, validate=validator)
    return {'schema_version': 'tos_corpus_admission_receipt_v1',
            'batch_sha256': hashlib.sha256(canonical(batch)).hexdigest(),
            'revision': snapshot['revision'], 'base_revision': snapshot['base_revision'],
            'validator_sha256': validator.sha256, 'members': len(snapshot['files']),
            'identities': len(snapshot['identities']),
            'source_bytes': sum(row['size_bytes'] for row in snapshot['files']),
            'semantic_admission': False, 'rights_change': False}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--store', type=Path, required=True)
    parser.add_argument('--batch', type=Path, required=True)
    parser.add_argument('--input-root', type=Path, required=True)
    parser.add_argument('--grammar-root', type=Path, required=True)
    parser.add_argument('--payload-source-root', type=Path)
    parser.add_argument('--historical-capture', type=Path, action='append')
    parser.add_argument('--historical-root', type=Path, action='append')
    args = parser.parse_args(argv)
    result = admit_batch(args.store, args.batch, args.input_root, args.grammar_root,
                         payload_source_root=args.payload_source_root,
                         historical_capture=args.historical_capture, historical_root=args.historical_root)
    print(canonical(result).decode(), end='')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

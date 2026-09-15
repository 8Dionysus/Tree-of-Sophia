#!/usr/bin/env python3
"""Compile one accepted source revision in an isolated, disposable data view.

This entry point runs in its own process. Existing producer modules are bound
once to that view; source/data files never select or supply executable code.
Generated files are published only by the separate data snapshot builder.
"""
from __future__ import annotations

import argparse
import importlib
import json
from pathlib import Path
import shutil
import sys
import tempfile

from corpus_store import CorpusStore, CorpusStoreError, canonical, digest_file
from corpus_source_validation import is_source_member

SOFTWARE_ROOT = Path(__file__).resolve().parents[1]
OUTPUTS = (
    ('philosophy_atlas_projection_common', 'ToS/derived-exports/philosophy_atlas_projection.min.json'),
    ('philosophy_graph_views_common', 'ToS/derived-exports/philosophy_graph_views.min.json'),
    ('philosophy_graph_projection_common', 'ToS/derived-exports/philosophy_graph_projection.min.json'),
)


def _bind(module_name: str, root: Path):
    """Bind a known legacy producer inside this single-use worker process."""
    module = importlib.import_module(module_name)
    if Path(module.__file__).resolve().parent != SOFTWARE_ROOT / 'scripts':
        raise CorpusStoreError('data producer was not loaded from software')
    for name, value in list(vars(module).items()):
        if isinstance(value, Path) and value.is_absolute() and value.is_relative_to(SOFTWARE_ROOT):
            setattr(module, name, root / value.relative_to(SOFTWARE_ROOT))
    return module


def _write_projection(root: Path, relative: str, payload):
    from partitioned_projection_common import write_partitioned_payload
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        raise CorpusStoreError('producer output overlaps an existing input')
    write_partitioned_payload(path, payload, prune=False)


def compile_revision(store_root: Path, revision: str, output: Path) -> dict:
    store = CorpusStore(store_root)
    manifest = store.load(revision, verify_objects=True)
    output = Path(output).absolute()
    if output.exists() or output.is_symlink():
        raise CorpusStoreError('data output must be new')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='tos-corpus-build-', dir=output.parent) as raw:
        view = Path(raw) / 'source'
        view.mkdir()
        for entry in manifest['files']:
            relative = entry['path']
            if not is_source_member(relative):
                raise CorpusStoreError('accepted source includes a generated producer output')
            path = view / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            # Producer isolation must protect the accepted objects even if a
            # buggy helper writes or chmods what it thought was an output.
            shutil.copyfile(store._object(entry['sha256']), path)
            path.chmod(entry['mode'])
        # API schemas are software-owned; the data packager will not copy them.
        shutil.copytree(SOFTWARE_ROOT / 'access/contracts', view / 'access/contracts')
        from build_source_witness_catalog import render_outputs, write_outputs
        write_outputs(view, render_outputs(view))
        _bind('philosophy_multilingual_common', view)
        for name, relative in OUTPUTS:
            module = _bind(name, view)
            path = view / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            payload = module.build_payload()
            if name == 'philosophy_graph_projection_common':
                _write_projection(view, relative, payload)
            else:
                path.write_text(module.render_payload(payload), encoding='utf-8')
        from partitioned_projection_common import build_storage
        from source_witness_bibliographic_graph_common import build_payload as build_bibliographic
        with build_storage() as storage:
            payload = build_bibliographic(view, storage=storage)
            _write_projection(view, 'ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json', payload)
        corpus = _bind('tos_corpus_index_common', view)
        with build_storage() as storage:
            payload = corpus.build_payload(storage=storage,
                source_paths=[entry['path'] for entry in manifest['files']])
            _write_projection(view, 'ToS/derived-exports/tos_corpus_index.min.json', payload)
        evidence = _bind('epistemic_evidence_projection_common', view)
        (view / 'ToS/derived-exports/epistemic_evidence_projection.min.json').write_text(
            evidence.render_payload(evidence.build_payload()), encoding='utf-8')
        # Producer bugs cannot silently turn a modified source view into a new
        # accepted corpus. Verify the exact original bytes after all producers.
        for entry in manifest['files']:
            if digest_file(view / entry['path']) != entry['sha256']:
                raise CorpusStoreError('producer changed accepted source bytes')
        sys.path.insert(0, str(SOFTWARE_ROOT / 'access/packaging'))
        from build_data_snapshot import build_data_snapshot
        result = build_data_snapshot(SOFTWARE_ROOT, view, output, corpus_revision=revision)
        return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--store', type=Path, required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(argv)
    result = compile_revision(args.store, args.revision, args.output)
    print(canonical(result).decode(), end='')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

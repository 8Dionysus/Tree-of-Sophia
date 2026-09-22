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

from corpus_store import (
    CorpusStore,
    CorpusStoreError,
    _copy_stream_digest,
    canonical,
    digest_file,
    regular,
    stage_timing,
)
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


def _copy_source_view(store: CorpusStore, manifest: dict, view: Path) -> None:
    with stage_timing(
        'build.source_materialize',
        members=len(manifest['files']),
        bytes=sum(entry['size_bytes'] for entry in manifest['files']),
    ):
        view.mkdir()
        for entry in manifest['files']:
            relative = entry['path']
            if not is_source_member(relative):
                raise CorpusStoreError('accepted source includes a generated producer output')
            path = view / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            source = store._object(entry['sha256'])
            before = regular(source)
            with source.open('rb') as stream, path.open('xb') as target:
                copied_size, copied_sha256 = _copy_stream_digest(stream, target)
            after = regular(source)
            if ((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) !=
                    (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
                    or copied_size != entry['size_bytes']
                    or copied_sha256 != entry['sha256']
                    or path.stat().st_size != entry['size_bytes']):
                raise CorpusStoreError('source object changed during build view materialization')
            path.chmod(entry['mode'])


def _compile_view(store: CorpusStore, manifest: dict, revision: str, view: Path, output: Path) -> dict:
    # API schemas are software-owned; the data packager will not copy them.
    with stage_timing('build.copy_software_contracts'):
        shutil.copytree(SOFTWARE_ROOT / 'access/contracts', view / 'access/contracts')
    from build_source_witness_catalog import render_outputs, write_outputs
    with stage_timing('build.catalog_render', members=len(manifest['files'])):
        catalog_outputs = render_outputs(view)
    with stage_timing('build.catalog_write', outputs=len(catalog_outputs)):
        write_outputs(view, catalog_outputs)
    _bind('philosophy_multilingual_common', view)
    for name, relative in OUTPUTS:
        with stage_timing(f'build.projection.{name}'):
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
    with stage_timing('build.bibliographic_graph'):
        with build_storage() as storage:
            payload = build_bibliographic(view, storage=storage)
            _write_projection(view, 'ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json', payload)
    corpus = _bind('tos_corpus_index_common', view)
    with stage_timing('build.corpus_index', members=len(manifest['files'])):
        with build_storage() as storage:
            payload = corpus.build_payload(storage=storage,
                source_paths=[entry['path'] for entry in manifest['files']])
            _write_projection(view, 'ToS/derived-exports/tos_corpus_index.min.json', payload)
    evidence = _bind('epistemic_evidence_projection_common', view)
    with stage_timing('build.epistemic_evidence'):
        (view / 'ToS/derived-exports/epistemic_evidence_projection.min.json').write_text(
            evidence.render_payload(evidence.build_payload()), encoding='utf-8')
    # Producer bugs cannot silently turn a modified source view into a new
    # accepted corpus. Verify the exact original bytes after all producers.
    with stage_timing('build.source_audit', members=len(manifest['files'])):
        for entry in manifest['files']:
            if digest_file(view / entry['path']) != entry['sha256']:
                raise CorpusStoreError('producer changed accepted source bytes')
    sys.path.insert(0, str(SOFTWARE_ROOT / 'access/packaging'))
    from build_data_snapshot import build_data_snapshot
    with stage_timing('build.data_snapshot'):
        return build_data_snapshot(SOFTWARE_ROOT, view, output, corpus_revision=revision)


def compile_revision(store_root: Path, revision: str, output: Path) -> dict:
    """Compile one revision in a private streamed-copy source view."""
    store = CorpusStore(store_root)
    output = Path(output).absolute()
    if output.exists() or output.is_symlink():
        raise CorpusStoreError('data output must be new')
    output.parent.mkdir(parents=True, exist_ok=True)
    # The streamed copy hashes each immutable object as it is read and checks
    # the private destination before producers run. This removes the
    # redundant pre-copy full-object pass while retaining source fixity.
    with stage_timing('build.load_manifest'):
        manifest = store.load(revision, verify_objects=False)
    # Current members are verified as they stream into the private view. A
    # retirement is not copied into that view, so its historical source and
    # event objects need their own fixity pass before a snapshot is published.
    with stage_timing('build.verify_retirements', events=len(manifest['retirements'])):
        store.verify_retirement_objects(manifest)

    with tempfile.TemporaryDirectory(prefix='tos-corpus-build-', dir=output.parent) as raw:
        root = Path(raw)
        view = root / 'source'
        _copy_source_view(store, manifest, view)
        return _compile_view(store, manifest, revision, view, output)


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

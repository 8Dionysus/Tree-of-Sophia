"""Explicit offline compilation of source-bound projections into a query store.

No request handler calls this module. Build scratch state is SQLite-backed;
publication is an atomic replacement after owner validation and input recheck.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import sys
import tempfile

if __package__ != 'tos_access':
    # Keep the manifest-owned lane runnable without a caller-provided
    # PYTHONPATH when this module is invoked as a direct script.
    _ACCESS_SRC = Path(__file__).resolve().parents[1]
    if str(_ACCESS_SRC) not in sys.path:
        sys.path.insert(0, str(_ACCESS_SRC))
    __package__ = 'tos_access'

from .disk_collections import DiskCollections, DiskSequence, DiskMap, canonical_digest, compact
from . import knowledge as k
from .projection_store import ProjectionReader, FORMAT, is_partitioned
from .query_store import SCHEMA, DEFAULT_RELATIVE_PATH, COMPILER_VERSION
from .exploration import EXECUTION_VERSION

INPUTS = {
    'corpus': 'ToS/derived-exports/tos_corpus_index.min.json',
    'philosophy': 'ToS/derived-exports/philosophy_graph_projection.min.json',
    'bibliographic': 'ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json',
    'entities': 'ToS/doctrine/semantic-interchange/entity-types.v1.json',
    'predicates': 'ToS/doctrine/semantic-interchange/relation-types.v1.json',
}
def _digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def _load(path, storage, *, allow_legacy, legacy_bound=4 * 1024 * 1024):
    # Legacy is explicit fixture/migration support, never a silent large fallback.
    if path.stat().st_size > legacy_bound:
        raise ValueError(f'partitioned input required: {path}')
    header = json.loads(path.read_text())
    if header.get('schema_version') != FORMAT:
        if not allow_legacy:
            raise ValueError(f'partitioned input required: {path}')
        return header, None
    reader = ProjectionReader(path)
    result = reader.metadata()
    for name, spec in reader.manifest['collections'].items():
        target = result
        parts = name.split('/')
        for part in parts[:-1]:
            target = target.setdefault(part, {})
        if spec['key_field'] is None:
            target[parts[-1]] = storage.mapping(reader.iter_items(name))
        else:
            rows = storage.sequence(reader.iter_collection(name))
            fields = spec['order_fields'] or (
                spec['key_field'] if isinstance(spec['key_field'], list)
                else [spec['key_field']]
            )
            rows.sort(key=lambda row: tuple(str(row.get(field, '')) for field in fields))
            target[parts[-1]] = rows
    return result, reader


def _schema(db):
    db.executescript('''
    CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
    CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,native_id TEXT,entity_id TEXT,source_graph TEXT,kind_id TEXT,type_id TEXT,label TEXT,search_text TEXT,payload TEXT NOT NULL);
    CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,native_id TEXT,from_id TEXT,to_id TEXT,source_graph TEXT,predicate_id TEXT,relation_type_id TEXT,label TEXT,search_text TEXT,payload TEXT NOT NULL);
    CREATE TABLE source_nodes(id TEXT PRIMARY KEY,kind_id TEXT,packet_id TEXT,payload TEXT NOT NULL);
    CREATE TABLE source_edges(id TEXT PRIMARY KEY,from_id TEXT,to_id TEXT,edge_kind TEXT,predicate_id TEXT,payload TEXT NOT NULL);
    CREATE TABLE source_rights(id TEXT PRIMARY KEY,payload TEXT NOT NULL);
    CREATE TABLE source_rights_scopes(right_id TEXT,scope_id TEXT,PRIMARY KEY(right_id,scope_id));
    CREATE TABLE semantic_diagnostics(kind TEXT,position INTEGER,payload TEXT NOT NULL,PRIMARY KEY(kind,position));
    CREATE TABLE raw_records(collection TEXT,key TEXT,position INTEGER,payload TEXT NOT NULL,PRIMARY KEY(collection,key));
    ''')


def _search_index(db, mode):
    if mode == 'scan':
        return {'mode': 'scan', 'reason': 'explicit-compiler-selection'}
    if mode != 'fts5-trigram':
        raise ValueError('search_accelerator must be fts5-trigram or scan')
    try:
        for table in ('knowledge_nodes', 'knowledge_relations'):
            index = table + '_trigram'
            db.execute(f"CREATE VIRTUAL TABLE {index} USING fts5(search_text, content='', detail=none, columnsize=0, tokenize='trigram case_sensitive 1')")
            db.execute(f'INSERT INTO {index}(rowid,search_text) SELECT rowid,search_text FROM {table}')
            db.execute(f"INSERT INTO {index}({index}) VALUES ('optimize')")
            db.execute(f"INSERT INTO {index}({index}) VALUES ('integrity-check')")
    except sqlite3.OperationalError as error:
        raise ValueError('FTS5 case-sensitive trigram compilation unavailable; use a capable SQLite build or explicitly select search_accelerator=scan') from error
    return {'mode': 'fts5-trigram', 'tokenizer': 'trigram case_sensitive 1',
            'detail': 'none', 'content': 'contentless', 'minimum_query_characters': 3,
            'verification': 'exact-python-lowered-json-substring',
            'fallback': 'disk-scan-for-short-or-nul-query', 'sqlite_version': sqlite3.sqlite_version}


def compile_knowledge_store(root, output=None, *, allow_legacy=False, search_accelerator='fts5-trigram', progress=None):

    def phase(name):
        if progress is not None:
            progress(name)

    phase('load-inputs')
    root = Path(root).resolve()
    output = Path(output) if output else root / DEFAULT_RELATIVE_PATH
    if not output.is_absolute():
        output = root / output
    output.parent.mkdir(parents=True, exist_ok=True)
    paths = {name: root / relative for name, relative in INPUTS.items()}
    # Corpus and bibliography are a coupled source snapshot.  Accepting one
    # partitioned root beside one legacy monolith would make the compiler's
    # source mode depend on which input happened to be loaded first, so reject
    # that state before allocating the private staging database.  Both legacy
    # roots remain available only through the explicit fixture/migration route.
    corpus_partitioned = is_partitioned(paths['corpus'])
    bibliography_partitioned = is_partitioned(paths['bibliographic'])
    if corpus_partitioned != bibliography_partitioned:
        raise ValueError('corpus and bibliographic projections must use the same storage mode')
    bindings = {INPUTS[name]: _digest(path) for name, path in paths.items()}
    fd, temporary = tempfile.mkstemp(prefix=output.name + '.', suffix='.building', dir=output.parent)
    os.close(fd)
    db = sqlite3.connect(temporary)
    token = k.active_cache.set(None)
    storage = None
    reader_closures = []
    try:
        # This unpublished file is discarded in full on every error; no caller
        # can observe or resume a partial transaction. Avoid a second multi-GB
        # rollback copy during VACUUM. The prior published snapshot remains
        # untouched until integrity/input checks pass and atomic replacement.
        db.execute('PRAGMA journal_mode=OFF')
        db.execute('PRAGMA temp_store=FILE')
        db.execute('PRAGMA cache_size=-8192')
        # This database is private compiler scratch state.  Final VACUUM
        # removes retired staging rows before the completed store is published.
        db.execute('PRAGMA secure_delete=OFF')
        storage = DiskCollections(db)
        corpus, cr = _load(paths['corpus'], storage, allow_legacy=allow_legacy)
        # Philosophy retains its existing deduplicated transport; this one explicit
        # offline load is separate from the partitioned corpus/bibliography path.
        philosophy, pr = _load(paths['philosophy'], storage, allow_legacy=True, legacy_bound=64 * 1024 * 1024)
        bibliography, br = _load(paths['bibliographic'], storage, allow_legacy=allow_legacy)
        for reader in (cr, pr, br):
            if reader:
                # Capture the exact referenced manifest closure before the
                # semantic build. The second pass below detects a changed
                # index or data part even when the root file is unchanged.
                reader_closures.append((reader, tuple(reader.closure_paths(verify_data=False))))
        entities = json.loads(paths['entities'].read_text())
        predicates = json.loads(paths['predicates'].read_text())
        revision = canonical_digest({'compiler': COMPILER_VERSION, 'snapshot_bindings': bindings})
        phase('normalize-and-validate')
        graph = k.build_knowledge_graph(corpus, philosophy, bibliography, entities, predicates,
                                       _storage=storage, _source_revision=revision)
        phase('catalog')
        catalog = k.knowledge_catalog(graph, corpus, philosophy, entities, predicates, _storage=storage)
        phase('write-knowledge-rows')
        _schema(db)
        semantic_report = graph['counts'].get('semantic_validation')
        if semantic_report is not None:
            gaps = semantic_report['gaps']
            db.executemany('INSERT INTO semantic_diagnostics VALUES (?,?,?)',
                           (('gap', position, compact(gap)) for position, gap in enumerate(gaps)))
            if len(gaps) <= 1000:
                semantic_report['gaps'] = list(gaps)
            else:
                semantic_report['gaps'] = []
                semantic_report['gap_count'] = len(gaps)
                semantic_report['gap_details_collection'] = 'semantic_diagnostics/gap'
                semantic_report['gaps_inline_complete'] = False
        for row in graph['nodes']:
            db.execute('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)',
                       (row['id'], row.get('native_id'), row.get('entity_id'), row.get('source_graph'),
                        row.get('kind_id'), row.get('type_id'), row['display']['title']['default'],
                        k._searchable(row), compact(row)))
        for row in graph['relations']:
            db.execute('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?)',
                       (row['id'], row.get('native_id'), row.get('from_id'), row.get('to_id'),
                        row.get('source_graph'), row.get('predicate_id'), row.get('relation_type_id'),
                        row['display']['label']['default'], k._searchable(row), compact(row)))
        phase('search-index')
        search_capability = _search_index(db, search_accelerator)
        phase('write-source-rows')
        navigation = corpus.get('source_navigation') or {}
        for row in navigation.get('nodes', []):
            db.execute('INSERT INTO source_nodes VALUES (?,?,?,?)',
                       (row['node_id'], row.get('node_kind'), (row.get('properties') or {}).get('packet_id'), compact(row)))
        for row in navigation.get('edges', []):
            db.execute('INSERT INTO source_edges VALUES (?,?,?,?,?,?)',
                       (row['edge_id'], row.get('from_id'), row.get('to_id'), row.get('edge_kind'), row.get('predicate_id'), compact(row)))
        for row in navigation.get('rights', []):
            db.execute('INSERT INTO source_rights VALUES (?,?)', (row['rights_id'], compact(row)))
            for scope in row.get('scope_refs', []):
                db.execute('INSERT OR IGNORE INTO source_rights_scopes VALUES (?,?)', (row['rights_id'], scope))
        for owner, payload in [('corpus', corpus), ('bibliographic', bibliography)]:
            for name, rows in payload.items():
                if not isinstance(rows, (list,)) and not isinstance(rows, DiskSequence):
                    continue
                if isinstance(rows, dict) or name == 'input_digests':
                    continue
                for position, row in enumerate(rows):
                    if not isinstance(row, dict):
                        continue
                    db.execute('INSERT INTO raw_records VALUES (?,?,?,?)',
                               (owner + '/' + name, str(position), position, compact(row)))
        node_revisions = storage.sequence([row['id'], row['content_revision']] for row in graph['nodes'])
        node_revisions.sort(key=lambda row: tuple(row))
        relation_revisions = storage.sequence([row['id'], row['content_revision']] for row in graph['relations'])
        relation_revisions.sort(key=lambda row: tuple(row))
        exploration_revision = k._stable_digest({'execution_version': EXECUTION_VERSION, 'source_revision': revision,
                                                 'nodes': node_revisions, 'relations': relation_revisions})
        header = {key: value for key, value in graph.items() if key not in ('nodes', 'relations')}
        def metadata_header(payload):
            return {key: value for key, value in payload.items()
                    if not isinstance(value, (DiskSequence, DiskMap)) and key not in
                    ('nodes', 'edges', 'rights', 'resources', 'manifests', 'branches', 'relation_edges',
                     'relation_packs', 'graph_views', 'claim_traces', 'input_digests', 'source_navigation')}
        metadata = {'search_accelerator': search_capability, 'schema': SCHEMA, 'complete': True, 'compiler_version': COMPILER_VERSION,
                    'snapshot_bindings': bindings, 'graph_header': header, 'catalog': catalog,
                    'exploration_revision': exploration_revision, 'corpus_header': metadata_header(corpus),
                    'bibliographic_header': metadata_header(bibliography),
                    'source_navigation_header': metadata_header(navigation)}
        db.executemany('INSERT INTO metadata VALUES (?,?)', ((key, compact(value)) for key, value in metadata.items()))
        for table, fields in {'knowledge_nodes': ['native_id', 'entity_id', 'source_graph', 'type_id', 'kind_id'],
                              'knowledge_relations': ['native_id', 'from_id', 'to_id', 'source_graph', 'predicate_id', 'relation_type_id'],
                              'source_nodes': ['kind_id', 'packet_id'], 'source_edges': ['from_id', 'to_id', 'edge_kind', 'predicate_id'],
                              'source_rights_scopes': ['scope_id']}.items():
            for field in fields:
                db.execute(f'CREATE INDEX {table}_{field} ON {table}({field})')
        phase('compact')
        storage.drop()
        db.commit()
        db.execute('VACUUM')
        phase('verify')
        if db.execute('PRAGMA integrity_check').fetchone()[0] != 'ok':
            raise ValueError('compiled store integrity check failed')
        for reader in (cr, pr, br):
            if reader:
                expected_closure = next(expected for candidate, expected in reader_closures
                                        if candidate is reader)
                # Reopen without the reader cache: a cached part cannot prove
                # that its on-disk bytes stayed unchanged during the build.
                current_reader = ProjectionReader(reader.path, cache_bytes=0)
                current_closure = tuple(current_reader.closure_paths())
                if current_closure != expected_closure:
                    raise ValueError('source projection manifest closure changed during compilation')
                reader.require_current()
        if bindings != {INPUTS[name]: _digest(path) for name, path in paths.items()}:
            raise ValueError('source snapshot changed during compilation')
        db.close()
        with open(temporary, 'rb') as stream:
            os.fsync(stream.fileno())
        os.replace(temporary, output)
        return {'output': str(output), 'source_revision': revision, 'counts': header['counts']}
    finally:
        k.active_cache.reset(token)
        if storage is not None:
            # Mark every retained disk view terminal before the connection is
            # closed; weakref finalizers then become no-ops instead of issuing
            # SQL against a closed scratch database during frame teardown.
            storage.close()
        db.close()
        if os.path.exists(temporary):
            os.unlink(temporary)


def iter_semantic_diagnostics(path, kind='gap'):
    """Export every diagnostic detail explicitly, decoding one row at a time."""
    db = sqlite3.connect(Path(path).resolve().as_uri() + '?mode=ro', uri=True)
    try:
        for (payload,) in db.execute('SELECT payload FROM semantic_diagnostics WHERE kind=? ORDER BY position', (kind,)):
            yield json.loads(payload)
    finally:
        db.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path.cwd())
    parser.add_argument('--output', type=Path)
    parser.add_argument('--search-accelerator', choices=['fts5-trigram', 'scan'], default='fts5-trigram')
    args = parser.parse_args()
    import time
    started = time.monotonic()
    def progress(name):
        print(f'[compile {time.monotonic() - started:.1f}s] {name}', file=sys.stderr, flush=True)
    print(json.dumps(compile_knowledge_store(args.root, args.output, search_accelerator=args.search_accelerator,
                                           progress=progress), ensure_ascii=False))


if __name__ == '__main__':
    main()

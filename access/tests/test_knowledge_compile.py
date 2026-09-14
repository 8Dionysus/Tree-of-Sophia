from __future__ import annotations
import copy
import gc
import hashlib
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch
import sys
from contextlib import closing
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))

from test_access_contract import write_fixture
from tos_access import knowledge as k
from tos_access.disk_collections import DiskCollections, canonical_digest
from tos_access.knowledge_compile import INPUTS, compile_knowledge_store, iter_semantic_diagnostics, _load, _search_index
from tos_access.projection_store import Collection, write_projection
from tos_access.query_store import QueryStore


class KnowledgeCompileTests(unittest.TestCase):
    def fixture(self, root):
        write_fixture(root)
        return {name: json.loads((root / path).read_text()) for name, path in INPUTS.items()}

    def partition(self, root, inputs, owners=('corpus', 'philosophy', 'bibliographic')):
        for owner in owners:
            header = copy.deepcopy(inputs[owner])
            collections = {}
            for name in ('nodes', 'edges', 'resources', 'manifests', 'relation_edges', 'relation_packs', 'claim_traces'):
                if name in header:
                    rows = header.pop(name)
                    field = next((field for field in ('node_id', 'edge_id', 'resource_id', 'pack_id', 'claim_id', 'claim_ref', 'path', 'id') if rows and field in rows[0]), 'id')
                    collections[name] = Collection(rows, field)
            if owner == 'corpus':
                for name in ('nodes', 'edges', 'rights'):
                    collections['source_navigation/' + name] = Collection(header['source_navigation'].pop(name), {'nodes': 'node_id', 'edges': 'edge_id', 'rights': 'rights_id'}[name])
            write_projection(root / INPUTS[owner], header, collections)

    def test_partitioned_compiler_matches_authoritative_graph_and_catalog(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = self.fixture(root)
            expected = k.build_knowledge_graph(inputs['corpus'], inputs['philosophy'], inputs['bibliographic'], inputs['entities'], inputs['predicates'])
            self.partition(root, inputs)
            with patch('tos_access.projection_store.load_projection', side_effect=AssertionError('full reconstruction forbidden')):
                result = compile_knowledge_store(root)
            store = QueryStore(result['output'])
            self.assertEqual(list(store.rows('knowledge_nodes')), expected['nodes'])
            self.assertEqual(list(store.rows('knowledge_relations')), expected['relations'])
            expected['source_revision'] = store.header['source_revision']
            self.assertEqual(store.metadata['catalog'], k.knowledge_catalog(expected, inputs['corpus'], inputs['philosophy'], inputs['entities'], inputs['predicates']))
            self.assertEqual(store.metadata['corpus_header']['authority_order'], inputs['corpus']['authority_order'])
            self.assertEqual(store.count('source_nodes'), len(inputs['corpus']['source_navigation']['nodes']))
            with closing(sqlite3.connect(result['output'])) as db:
                self.assertEqual(db.execute("SELECT count(*) FROM sqlite_master WHERE name LIKE '_build_%'").fetchone()[0], 0)

    def test_mixed_partitioned_and_legacy_source_roots_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = self.fixture(root)
            # Keep the bibliography as its legacy fixture while converting only
            # the corpus root.  Explicit legacy compatibility must not permit a
            # mixed source snapshot.
            self.partition(root, inputs, owners=('corpus',))
            output = root / 'store.sqlite3'
            with self.assertRaisesRegex(ValueError, 'same storage mode'):
                compile_knowledge_store(root, output, allow_legacy=True)
            self.assertFalse(output.exists())
            self.assertFalse(list(root.glob('*.building')))

    def test_catalog_compact_summaries_match_pure_catalog(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = self.fixture(root)
            expected_graph = k.build_knowledge_graph(
                inputs['corpus'], inputs['philosophy'], inputs['bibliographic'],
                inputs['entities'], inputs['predicates'],
            )
            db = sqlite3.connect(':memory:')
            storage = DiskCollections(db)
            try:
                disk_graph = k.build_knowledge_graph(
                    inputs['corpus'], inputs['philosophy'], inputs['bibliographic'],
                    inputs['entities'], inputs['predicates'], _storage=storage,
                )
                expected = k.knowledge_catalog(
                    expected_graph, inputs['corpus'], inputs['philosophy'],
                    inputs['entities'], inputs['predicates'],
                )
                actual = k.knowledge_catalog(
                    disk_graph, inputs['corpus'], inputs['philosophy'],
                    inputs['entities'], inputs['predicates'], _storage=storage,
                )
                # The disk graph intentionally retains large diagnostics as a
                # DiskSequence. Compare the canonical JSON surface so that
                # collection representation does not obscure catalog parity.
                self.assertEqual(canonical_digest(actual), canonical_digest(expected))
            finally:
                storage.close()
                db.close()

    def test_failed_build_preserves_previous_store_and_removes_scratch(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            output = root / 'store.sqlite3'
            compile_knowledge_store(root, output, allow_legacy=True)
            before = output.read_bytes()
            with patch.object(k, 'build_knowledge_graph', side_effect=ValueError('owner invariant rejected')):
                with self.assertRaisesRegex(ValueError, 'owner invariant rejected'):
                    compile_knowledge_store(root, output, allow_legacy=True)
            self.assertEqual(output.read_bytes(), before)
            self.assertFalse(list(root.glob('*.building')))

    def test_changed_input_during_compile_is_not_published(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            original = k.knowledge_catalog
            def changed(*args, **kwargs):
                catalog = original(*args, **kwargs)
                path = root / INPUTS['entities']
                path.write_text(path.read_text() + '\n')
                return catalog
            with patch.object(k, 'knowledge_catalog', side_effect=changed):
                with self.assertRaisesRegex(ValueError, 'source snapshot changed'):
                    compile_knowledge_store(root, root / 'store.sqlite3', allow_legacy=True)
            self.assertFalse((root / 'store.sqlite3').exists())
            self.assertFalse(list(root.glob('*.building')))

    def test_changed_partition_part_is_not_published(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = self.fixture(root)
            self.partition(root, inputs)
            part = next((root / Path(INPUTS['corpus']).with_suffix('.parts')).rglob('*.jsonl.gz'))
            original = part.read_bytes()
            catalog = k.knowledge_catalog

            def changed(*args, **kwargs):
                result = catalog(*args, **kwargs)
                part.write_bytes(original[:-1] + bytes([original[-1] ^ 1]))
                return result

            with patch.object(k, 'knowledge_catalog', side_effect=changed):
                with self.assertRaisesRegex(ValueError, 'digest mismatch|closure changed'):
                    compile_knowledge_store(root, root / 'store.sqlite3')
            self.assertFalse((root / 'store.sqlite3').exists())
            self.assertFalse(list(root.glob('*.building')))

    def test_large_accepted_diagnostics_are_streamed_to_detail_table(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            original = k.build_knowledge_graph
            def diagnostics(*args, **kwargs):
                graph = original(*args, **kwargs)
                graph['counts']['semantic_validation']['gaps'] = kwargs['_storage'].sequence(
                    {'id': str(index), 'reason': 'fixture-unassessed'} for index in range(1001))
                return graph
            with patch.object(k, 'build_knowledge_graph', side_effect=diagnostics):
                result = compile_knowledge_store(root, allow_legacy=True)
            report = QueryStore(result['output']).header['counts']['semantic_validation']
            self.assertEqual(report['gaps'], [])
            self.assertEqual(report['gap_count'], 1001)
            self.assertFalse(report['gaps_inline_complete'])
            self.assertEqual(sum(1 for _ in iter_semantic_diagnostics(result['output'])), 1001)

    def test_trigram_index_is_used_and_scan_is_an_explicit_build_option(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            indexed = compile_knowledge_store(root, root / 'indexed.sqlite3', allow_legacy=True)
            scanned = compile_knowledge_store(root, root / 'scan.sqlite3', allow_legacy=True, search_accelerator='scan')
            store = QueryStore(indexed['output'])
            scan = QueryStore(scanned['output'])
            self.assertEqual(scan.metadata['search_accelerator']['mode'], 'scan')
            clause, params = store.text_candidates('knowledge_nodes', 'fixture')
            with store.connect() as db:
                plan = list(db.execute('EXPLAIN QUERY PLAN SELECT id FROM knowledge_nodes WHERE ' + clause, params))
                self.assertTrue(any('VIRTUAL TABLE INDEX' in row[3] for row in plan), plan)
                self.assertTrue(any('INTEGER PRIMARY KEY' in row[3] for row in plan), plan)
            for query in ('fixture', '', 'fi', 'a' * 256):
                self.assertEqual(store.search(query), scan.search(query))
            self.assertEqual(store.text_candidates('knowledge_nodes', 'ab'), ('1', []))
            self.assertEqual(store.text_candidates('knowledge_nodes', 'abc\0def'), ('1', []))

    def test_missing_trigram_support_fails_with_explicit_fallback_route(self):
        class Unsupported:
            def execute(self, *args):
                raise sqlite3.OperationalError('no such tokenizer: trigram')
        with self.assertRaisesRegex(ValueError, 'explicitly select search_accelerator=scan'):
            _search_index(Unsupported(), 'fts5-trigram')
        self.assertEqual(_search_index(Unsupported(), 'scan')['mode'], 'scan')

    def test_explicit_disk_sort_and_digest_match_json(self):
        with closing(sqlite3.connect(':memory:')) as db:
            disk = DiskCollections(db)
            values = [['z', 'c'], ['a', 'b'], ['aa', 'x'], ['a', 'a']]
            rows = disk.sequence(values)
            rows.sort(key=lambda row: tuple(row))
            self.assertEqual(list(rows), sorted(values))
            rows.append(['zz', 'late'])
            self.assertEqual(list(rows), [*sorted(values), ['zz', 'late']])
            payload = {'rows': sorted(values), 'text': 'ф'}
            self.assertEqual(canonical_digest({'rows': rows[:4], 'text': 'ф'}), hashlib.sha256(json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()).hexdigest())

    def test_partitioned_loader_restores_declared_composite_order(self):
        rows = [
            {'pack_id': 'pack-b', 'edge_id': 'edge-2', 'value': 'second'},
            {'pack_id': 'pack-a', 'edge_id': 'edge-3', 'value': 'third'},
            {'pack_id': 'pack-a', 'edge_id': 'edge-1', 'value': 'first'},
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'projection.min.json'
            write_projection(
                path,
                {'schema_version': 'fixture_projection_v1'},
                {'edges': Collection(rows, ('pack_id', 'edge_id'), ('edge_id', 'pack_id'))},
            )
            with closing(sqlite3.connect(':memory:')) as db:
                storage = DiskCollections(db)
                loaded, reader = _load(path, storage, allow_legacy=False)
                self.assertEqual(
                    list(loaded['edges']),
                    sorted(rows, key=lambda row: (row['edge_id'], row['pack_id'])),
                )
                reader.require_current()

    def test_disk_groups_keep_first_row_and_connection_allocates_distinct_collections(self):
        with closing(sqlite3.connect(':memory:')) as db:
            first = DiskCollections(db)
            first_groups = first.groups()
            first_groups['claim'].append({'b': 2, 'a': 1})
            first_groups['claim'].update([{'a': 1, 'b': 2}])
            self.assertEqual(list(first_groups['claim']), [{'a': 1, 'b': 2}])

            second = DiskCollections(db)
            left = first.sequence(['left'])
            right = second.sequence(['right'])
            self.assertNotEqual(left.collection, right.collection)
            self.assertEqual(list(left), ['left'])
            self.assertEqual(list(right), ['right'])

    def test_unreachable_views_release_only_their_collection_and_keep_views_alive(self):
        with closing(sqlite3.connect(':memory:')) as db:
            storage = DiskCollections(db)
            first = storage.sequence(['first'])
            second = storage.sequence(['second'])
            first_collection = first.collection
            second_collection = second.collection
            iterator = iter(second)
            self.assertEqual(next(iterator), 'second')

            groups = storage.groups()
            group_rows = groups['claim']
            group_rows.append({'id': 'kept-by-view'})
            group_collection = groups.collection
            del first
            gc.collect()

            self.assertEqual(
                db.execute('SELECT count(*) FROM _build_rows WHERE collection=?',
                           (first_collection,)).fetchone()[0],
                0,
            )
            self.assertEqual(
                db.execute('SELECT count(*) FROM _build_rows WHERE collection=?',
                           (second_collection,)).fetchone()[0],
                1,
            )
            # GroupRows owns its DiskGroups parent, so deleting the top-level
            # variable cannot release a collection while that view is live.
            del groups
            gc.collect()
            self.assertEqual(list(group_rows), [{'id': 'kept-by-view'}])
            self.assertEqual(
                db.execute('SELECT count(*) FROM _build_groups WHERE collection=?',
                           (group_collection,)).fetchone()[0],
                1,
            )
            del group_rows
            gc.collect()
            self.assertEqual(
                db.execute('SELECT count(*) FROM _build_groups WHERE collection=?',
                           (group_collection,)).fetchone()[0],
                0,
            )
            # An exhausted/active generator keeps its sequence usable until
            # the cursor owner is gone; the finalizer never closes a live view.
            self.assertEqual(list(iterator), [])
            storage.close()

    def test_owner_close_makes_late_collection_finalizers_noops(self):
        db = sqlite3.connect(':memory:')
        storage = DiskCollections(db)
        retained = storage.sequence(['retained'])
        storage.close()
        db.close()
        del retained
        gc.collect()


if __name__ == '__main__':
    unittest.main()

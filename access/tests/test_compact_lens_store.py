"""Atomic compact publication and exact runtime parity, not source admission."""
import copy
from contextlib import closing
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access import knowledge as k
from tos_access.compact_lens_store import prepare_compact_lens_store_transaction, CompactStoreLimits
from tos_access.prepared_publication import publish_prepared, apply_prepared_delta_transaction, PreparedChange
from tos_access.published_lens import PublishedLensService
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedReadModelError, _Read
from tos_access.published_read_metadata import _compact
from test_prepared_publication import fixture
from test_indexed_lens import lens


class CompactLensStoreTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='compact-store-', dir=os.environ.get('TMPDIR'))
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'snapshot.sqlite'
        self.graph, self.catalog = fixture()
        self.graph['nodes'][0]['attributes']['large'] = 'x' * 20000
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        self.reader = PublishedKnowledgeReadModel(self.path, self.binding)

    def install(self, db):
        db.execute('BEGIN IMMEDIATE')
        report = prepare_compact_lens_store_transaction(db, expected_binding=self.binding)
        db.commit()
        return report

    def spec(self, **extra):
        return lens(detail='compact', language='ru', **extra)

    def delta(self, db, changes):
        header = {key: copy.deepcopy(value) for key, value in self.graph.items() if key not in ('nodes', 'relations')}
        header['source_revision'] = 'c' * 64
        return apply_prepared_delta_transaction(db, expected_binding=self.binding, source_header=header,
            catalog={**self.catalog, 'source_revision': header['source_revision']}, changes=changes)

    def test_real_reader_uses_seeds_but_inspection_stays_complete(self):
        expected = PublishedLensService(self.reader).execute(self.spec())
        with closing(sqlite3.connect(self.path)) as db:
            before = list(db.execute('SELECT * FROM edge_meta ORDER BY key,part'))
            report = self.install(db)
            self.assertEqual(report['rows'], 4)
            self.assertLess(report['seed_bytes'], report['source_bytes'])
            self.assertEqual(list(db.execute('SELECT * FROM edge_meta ORDER BY key,part')), before)
        with patch.object(_Read, 'items', side_effect=AssertionError('full payload read')):
            actual = PublishedLensService(self.reader).execute(self.spec())
        self.assertEqual(_compact(actual), _compact(expected))
        self.assertEqual(self.reader.node('a')['matches'][0]['attributes']['large'], 'x' * 20000)

    def test_addressed_update_insert_delete_and_rollback(self):
        with closing(sqlite3.connect(self.path)) as db:
            self.install(db)
            before = list(db.iterdump())
            changed = copy.deepcopy(self.graph['nodes'][0])
            changed['display']['label'] = 'Исправлено'
            added = copy.deepcopy(self.graph['nodes'][1])
            added.update(id='new', entity_id='new-entity', native_id='new-native')
            changes = [PreparedChange('update', 'node', 'a', changed),
                       PreparedChange('insert', 'node', 'new', added, 100),
                       PreparedChange('delete', 'relation', 'r'), PreparedChange('delete', 'node', 'b')]
            db.execute('BEGIN IMMEDIATE')
            self.delta(db, changes)
            db.rollback()
            self.assertEqual(list(db.iterdump()), before)
            db.execute('BEGIN IMMEDIATE')
            binding = self.delta(db, changes)
            db.commit()
            rows = list(db.execute('SELECT kind,id FROM knowledge_compact_lens ORDER BY kind,id'))
            self.assertEqual(rows, [('node', 'A'), ('node', 'a'), ('node', 'new')])
            expected_graph = {**self.graph, 'source_revision': 'c' * 64,
                              'nodes': [changed, self.graph['nodes'][1], added], 'relations': []}
            reader = PublishedKnowledgeReadModel(self.path, binding)
            self.assertEqual(_compact(PublishedLensService(reader).execute(self.spec())),
                             _compact(k.execute_knowledge_lens(expected_graph, self.spec())))

    def test_old_writer_invalidates_compact_lane_without_breaking_full_reads(self):
        with closing(sqlite3.connect(self.path)) as db:
            self.install(db)
            db.execute("UPDATE knowledge_nodes SET json=json WHERE id='a'")
            db.commit()
            with self.assertRaisesRegex(PublishedReadModelError, 'stale'):
                PublishedLensService(self.reader).execute(self.spec())
            self.assertEqual(self.reader.node('a')['matches'][0]['id'], 'a')
            db.execute('BEGIN IMMEDIATE')
            with self.assertRaisesRegex(PublishedReadModelError, 'stale'):
                self.delta(db, [])
            db.rollback()

    def test_partial_install_and_corrupt_seed_fail_closed(self):
        with closing(sqlite3.connect(self.path)) as db:
            before = list(db.iterdump())
            db.execute('BEGIN IMMEDIATE')
            with self.assertRaisesRegex(PublishedReadModelError, 'input budget'):
                prepare_compact_lens_store_transaction(db, expected_binding=self.binding,
                                                      limits=CompactStoreLimits(max_rows=1))
            db.rollback()
            self.assertEqual(list(db.iterdump()), before)
            self.install(db)
            db.execute("UPDATE knowledge_compact_lens SET json=json||' ' WHERE id='a'")
            db.commit()
            with self.assertRaisesRegex(PublishedReadModelError, 'checksum'):
                PublishedLensService(self.reader).execute(self.spec())

    def test_index_drift_cannot_change_compact_scope_or_identity(self):
        with closing(sqlite3.connect(self.path)) as db:
            self.install(db)
            db.execute("UPDATE knowledge_nodes SET native_id='wrong' WHERE id='a'")
            # Simulate a falsely re-admitted index row: even a current marker
            # must not replace the old reader's per-row index/packet check.
            db.execute('UPDATE knowledge_compact_lens_state SET valid=1')
            db.commit()
            with self.assertRaisesRegex(PublishedReadModelError, 'index columns'):
                PublishedLensService(self.reader).execute(self.spec())

    def test_metadata_successor_and_uncovered_filters_keep_complete_behavior(self):
        spec = self.spec(node_query={'filters':[{'field':'attributes.large','op':'exists','value':True}]})
        expected = PublishedLensService(self.reader).execute(spec)
        with closing(sqlite3.connect(self.path)) as db:
            self.install(db)
            self.assertEqual(PublishedLensService(self.reader).execute(spec), expected)
            before = list(db.execute('SELECT * FROM knowledge_compact_lens ORDER BY kind,id'))
            db.execute('BEGIN IMMEDIATE')
            binding = self.delta(db, [])
            db.commit()
            self.assertEqual(list(db.execute('SELECT * FROM knowledge_compact_lens ORDER BY kind,id')), before)
            reader = PublishedKnowledgeReadModel(self.path, binding)
            graph = {**self.graph, 'source_revision':'c'*64}
            self.assertEqual(PublishedLensService(reader).execute(self.spec()), k.execute_knowledge_lens(graph, self.spec()))

"""Bounded prepared/D1 composition against an independent full SQL producer."""
import copy
import json
from pathlib import Path
import sqlite3
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / 'access/deploy/cloudflare-worker/scripts')]
import prepared_delta_runtime as delta
import build_runtime as full
from incremental_runtime import prepare_search_address_indexes_transaction
from tos_access.prepared_publication import PreparedChange, SOURCE_ORDER_STRIDE
from tos_access.prepared_source_binding import apply_source_bound_prepared_delta_transaction
import test_prepared_source_binding as fixtures


class PreparedD1DeltaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        fixtures.PreparedSourceBindingTests.setUpClass()

    def setUp(self):
        self.source = fixtures.PreparedSourceBindingTests()
        self.source.setUp()
        self.addCleanup(self.source.doCleanups)
        self.source.attach()
        self.f = self.source.f
        self.root = Path(self.f.tmp.name)
        self.before = sqlite3.connect(':memory:')
        self.addCleanup(self.before.close)
        self.f.db.backup(self.before)
        self.d1 = self.full(self.f.graph, self.f.catalog, 'd'*64, 'initial')
        self.d1.execute('BEGIN IMMEDIATE')
        prepare_search_address_indexes_transaction(self.d1, expected_revision='d'*64)
        self.d1.commit()

    def full(self, graph, catalog, revision, name):
        carriers = full.ProducerCarrierSet.admit(corpus={}, philosophy={}, knowledge=graph, knowledge_catalog=catalog,
            evidence={}, philosophy_audit={}, word_analysis_capability={'available': False}, carrier_paths={},
            logical_bindings={'source_revision': graph['source_revision']})
        path = self.root / name / 'read-model.sql'
        full.build_read_model_sql(None, path, revision, carriers, emit_delta_baseline=False)
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        db.executescript(path.read_text())
        db.executescript((ROOT / 'access/deploy/cloudflare-worker/migrations/0001-exploration.sql').read_text())
        return db

    def change(self):
        graph, source = self.source.delta()
        a, upper, b = graph['nodes']
        a['attributes']['opaque'] = {'2': 9007199254740993, '1': -0.0, 'null': None}
        new = copy.deepcopy(a)
        new.update(id='new', entity_id='new-entity', native_id='new-native')
        graph['nodes'] = [upper, b, a, new]
        graph['relations'] = []
        self.f.db.execute('BEGIN IMMEDIATE')
        result = apply_source_bound_prepared_delta_transaction(self.f.db,
            expected_binding=self.f.binding, before_source_inputs=self.source.before, after_source_inputs=source,
            before_inputs=self.f.inputs(self.f.graph), after_inputs=self.f.inputs(graph),
            changes=[PreparedChange('update', 'node', 'a', a, 3*SOURCE_ORDER_STRIDE),
                     PreparedChange('insert', 'node', 'new', new, 4*SOURCE_ORDER_STRIDE),
                     PreparedChange('delete', 'relation', 'r')])
        self.f.db.commit()
        graph.update(result['source_header'])
        return graph, result

    def capture(self, result, name='delta.sql', **options):
        for db in (self.d1, self.before, self.f.db):
            if not db.in_transaction:
                db.execute('BEGIN')
        try:
            return delta.build_prepared_delta_sql(self.d1, self.before, self.f.db, self.root / name,
                expected_d1_revision='d'*64, before_binding=self.f.binding, after_binding=result['binding'], **options)
        finally:
            for db in (self.d1, self.before, self.f.db):
                db.rollback()

    def test_update_add_delete_case_order_all_lanes_match_full_producer_and_replay(self):
        graph, result = self.change()
        before = list(self.d1.iterdump())
        with patch.object(full, 'build_read_model_sql', side_effect=AssertionError('whole producer')):
            receipt = self.capture(result)
        self.assertEqual(list(self.d1.iterdump()), before)
        self.assertFalse(receipt['d1_applied'])
        oracle = self.full(graph, result['catalog'], receipt['target_d1_revision'], 'oracle')
        sql = (self.root / 'delta.sql').read_text()
        publish = 'INSERT OR REPLACE INTO tos_delta_publications SELECT'
        stage, rest = sql.split(publish, 1)
        self.d1.executescript(stage)
        self.assertEqual(self.d1.execute("SELECT json FROM knowledge_nodes WHERE id='a'").fetchone(),
                         self.before.execute("SELECT json FROM knowledge_nodes WHERE id='a'").fetchone())
        self.d1.executescript(publish + rest)
        self.d1.executescript(sql)
        for table in ('knowledge_nodes', 'knowledge_relations', 'edge_meta', 'knowledge_lens_order', 'knowledge_search_gram_stats'):
            order = ','.join(delta.PRIMARY_KEYS[table])
            self.assertEqual(self.d1.execute(f'SELECT * FROM {table} ORDER BY {order}').fetchall(),
                             oracle.execute(f'SELECT * FROM {table} ORDER BY {order}').fetchall(), table)
        columns = ','.join(c for c in delta.DOCUMENT_COLUMNS if c != 'position')
        self.assertEqual(self.d1.execute(f'SELECT {columns} FROM knowledge_search_documents ORDER BY kind,id').fetchall(),
                         oracle.execute(f'SELECT {columns} FROM knowledge_search_documents ORDER BY kind,id').fetchall())
        postings = ('SELECT g.kind,g.n,g.gram,d.id FROM knowledge_search_grams g JOIN knowledge_search_documents d '
                    'ON d.kind=g.kind AND d.position=g.position ORDER BY g.kind,g.n,g.gram,d.id')
        self.assertEqual(self.d1.execute(postings).fetchall(), oracle.execute(postings).fetchall())
        ranking = 'SELECT id FROM knowledge_search_documents ORDER BY kind,id_lower,position'
        self.assertEqual(self.d1.execute(ranking).fetchall(), oracle.execute(ranking).fetchall())

    def test_capture_refuses_stale_rows_and_budget_without_publishing_sql(self):
        _, result = self.change()
        self.d1.execute("UPDATE knowledge_nodes SET json='{}' WHERE id='a'")
        self.d1.commit()
        before = list(self.d1.iterdump())
        with self.assertRaisesRegex(ValueError, 'predecessor row differs'):
            self.capture(result)
        self.assertEqual(list(self.d1.iterdump()), before)
        self.assertFalse((self.root / 'delta.sql').exists())
        with self.assertRaises(ValueError):
            self.capture(result, 'budget.sql', limits=delta.PreparedD1DeltaLimits(max_read_bytes=10))
        self.assertFalse((self.root / 'budget.sql').exists())

    def serving_rows(self):
        return {table: self.d1.execute(f'SELECT * FROM {table} ORDER BY ' + ','.join(delta.PRIMARY_KEYS[table])).fetchall()
                for table in delta.COLUMNS}

    def test_atomic_refusal_recovery_and_exact_reverse_without_source_rollback(self):
        _, result = self.change()
        original = self.serving_rows()
        self.capture(result, rollback_target=self.root / 'rollback.sql')
        sql = (self.root / 'delta.sql').read_text()
        publish = 'INSERT OR REPLACE INTO tos_delta_publications SELECT'
        stage, rest = sql.split(publish, 1)
        self.d1.executescript(stage)
        # A truncated stage cannot publish even though other tables are ready.
        table = self.d1.execute("SELECT name FROM sqlite_master WHERE type='table' "
            "AND name LIKE 'tos_delta_%_knowledge_nodes' AND name NOT LIKE '%_keys'").fetchone()[0]
        self.d1.execute(f'DELETE FROM {table} WHERE rowid=(SELECT min(rowid) FROM {table})')
        self.d1.commit()
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'incomplete delta staging'):
            self.d1.executescript(publish + rest)
        self.assertEqual(self.serving_rows(), original)
        self.d1.executescript(sql)
        published = self.serving_rows()
        rollback = (self.root / 'rollback.sql').read_text()
        self.d1.executescript(rollback)
        self.d1.executescript(rollback)
        self.assertEqual(self.serving_rows(), original)
        self.assertEqual(self.f.db.execute("SELECT json FROM knowledge_nodes WHERE id='new'").fetchone() is not None, True)
        self.d1.executescript(sql)
        self.assertEqual(self.serving_rows(), published)
        # Historic publication receipts do not authorize a stale re-publication.
        self.d1.execute("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'", (json.dumps({'sha256': 'e'*64}),))
        self.d1.commit()
        competing = self.serving_rows()
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'stale delta baseline'):
            self.d1.executescript(rollback)
        self.assertEqual(self.serving_rows(), competing)

    def test_output_and_retention_budgets_keep_both_sql_targets_unpublished(self):
        _, result = self.change()
        for field in ('max_sql_bytes', 'max_retained_bytes'):
            with self.subTest(field=field):
                target, reverse = self.root / (field + '.sql'), self.root / (field + '-reverse.sql')
                with self.assertRaisesRegex(ValueError, 'budget exceeded'):
                    self.capture(result, target.name, rollback_target=reverse,
                                 limits=delta.PreparedD1DeltaLimits(**{field: 10}))
                self.assertFalse(target.exists())
                self.assertFalse(reverse.exists())

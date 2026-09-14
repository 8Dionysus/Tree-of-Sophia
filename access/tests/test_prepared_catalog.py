"""One-file catalog/search/lens publication and caller rollback contracts."""
import copy
from contextlib import closing
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access import knowledge as k
from tos_access.catalog_index import CatalogIndex
from tos_access.catalog_semantics import CatalogInputs, CANONICAL_ORDER, SEQUENCE_ORDER
from tos_access.prepared_catalog import (bootstrap_prepared_catalog_transaction,
    apply_catalogued_prepared_delta_transaction)
from tos_access.prepared_publication import PreparedChange, PublicationLimits, publish_prepared, SOURCE_ORDER_STRIDE
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
from tos_access.published_lens import PublishedLensService
from tos_access.published_search import PublishedSearchService
from tos_access.published_read_metadata import _compact
from test_prepared_publication import fixture
from test_indexed_lens import lens


def inputs(graph, profile=SEQUENCE_ORDER):
    return CatalogInputs({key: value for key, value in graph.items() if key not in ('nodes', 'relations')},
                         source_order_profile=profile)


class PreparedCatalogTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='prepared-catalog-', dir=os.environ.get('TMPDIR'))
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'prepared.sqlite'
        self.graph, _ = fixture()
        self.graph['normalization_binding'] = k._normalization_binding(None, None)
        self.graph['counts'] = {'nodes': 3, 'relations': 1,
            'semantic_validation': {'schema': 'synthetic-owner-report', 'observation': 'before'},
            'unknown': {'number': 9007199254740993, 'false': False}}
        self.catalog = k.knowledge_catalog(self.graph, {}, {})
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)

    def connection(self):
        db = sqlite3.connect(self.path, isolation_level=None)
        self.addCleanup(db.close)
        db.execute('BEGIN IMMEDIATE')
        return db

    def attach(self, profile=SEQUENCE_ORDER):
        db = self.connection()
        result = bootstrap_prepared_catalog_transaction(db, expected_binding=self.binding,
                                                       inputs=inputs(self.graph, profile))
        db.commit()
        return db, result

    def after(self, marker='c'):
        graph = copy.deepcopy(self.graph)
        graph['source_revision'] = marker * 64
        # This is an explicit synthetic owner report, not inferred validation.
        graph['counts']['semantic_validation']['observation'] = marker
        return graph

    def test_attach_preserves_existing_publication_and_requires_no_full_builder(self):
        with closing(sqlite3.connect(self.path)) as read:
            before = {table: read.execute(f'SELECT * FROM {table} ORDER BY 1,2').fetchall()
                      for table in ('knowledge_nodes', 'knowledge_relations', 'edge_meta',
                                    'knowledge_lens_order', 'prepared_documents', 'search_documents')}
        with patch.object(k, 'knowledge_catalog', side_effect=AssertionError('full catalog wrapper')), \
             patch.object(k, 'build_knowledge_graph', side_effect=AssertionError('full graph')):
            db, result = self.attach()
        self.assertEqual(result['binding'], self.binding)
        self.assertFalse(result['publication_changed'])
        self.assertFalse(result['consumer_switched'])
        for table, rows in before.items():
            self.assertEqual(db.execute(f'SELECT * FROM {table} ORDER BY 1,2').fetchall(), rows)
        self.assertEqual(PublishedKnowledgeReadModel(self.path, self.binding).catalog(), self.catalog)
        db.execute('BEGIN')
        self.assertEqual(CatalogIndex(db).render(inputs(self.graph)), self.catalog)
        db.rollback()

    def test_update_joins_catalog_full_rows_lens_search_and_final_header(self):
        db, _ = self.attach()
        old_reader = PublishedKnowledgeReadModel(self.path, self.binding)
        after = self.after()
        after['nodes'][0]['display']['title'] = 'Новая форма'
        after['nodes'][0]['attributes']['exact'] = {'10': 1.0, '2': 9007199254740993, 'zero': -0.0}
        expected = k.knowledge_catalog(after, {}, {})
        db.execute('BEGIN IMMEDIATE')
        with patch.object(k, 'knowledge_catalog', side_effect=AssertionError('full catalog wrapper')), \
             patch.object(k, 'build_knowledge_graph', side_effect=AssertionError('full graph')):
            result = apply_catalogued_prepared_delta_transaction(db, expected_binding=self.binding,
                before_inputs=inputs(self.graph), after_inputs=inputs(after),
                changes=[PreparedChange('update', 'node', 'a', after['nodes'][0])])
        self.assertTrue(db.in_transaction)
        db.commit()
        self.assertEqual(result['catalog'], expected)
        self.assertEqual(result['source_header'], inputs(after).header)
        reader = PublishedKnowledgeReadModel(self.path, result['binding'])
        self.assertEqual(reader.catalog(), expected)
        self.assertEqual(reader.node('a')['matches'][0], after['nodes'][0])
        spec = lens(seed={'node_ids': ['a']})
        self.assertEqual(PublishedLensService(reader).execute(spec), k.execute_knowledge_lens(after, spec))
        actual = PublishedSearchService(reader).search('Новая форма', limit=10)
        self.assertEqual(actual['nodes'], k.search_knowledge_graph(after, 'Новая форма', limit=10)['nodes'])
        with self.assertRaises(PublishedSnapshotConflict):
            old_reader.catalog()
        self.assertFalse(result['source_transition_verified'])
        self.assertFalse(result['semantic_acceptance'])

    def test_insert_delete_and_next_final_header_without_rebuilding_catalog(self):
        db, _ = self.attach()
        after = self.after()
        new = copy.deepcopy(after['nodes'][0]); new.update(id='new', native_id='new', entity_id='new')
        after['nodes'].insert(1, new)
        relation = copy.deepcopy(after['relations'][0]); relation.update(id='new-rel', native_id='new-rel', to_id='new')
        after['relations'].append(relation)
        db.execute('BEGIN IMMEDIATE')
        result = apply_catalogued_prepared_delta_transaction(db, expected_binding=self.binding,
            before_inputs=inputs(self.graph), after_inputs=inputs(after), changes=[
                PreparedChange('insert', 'relation', 'new-rel', relation, SOURCE_ORDER_STRIDE),
                PreparedChange('insert', 'node', 'new', new, SOURCE_ORDER_STRIDE // 2)])
        db.commit()
        self.assertEqual(result['source_header']['counts']['nodes'], 4)
        self.assertEqual(result['source_header']['counts']['relations'], 2)
        self.assertEqual(result['source_header']['counts']['semantic_validation'], after['counts']['semantic_validation'])
        self.assertEqual(result['source_header']['counts']['unknown'], self.graph['counts']['unknown'])
        after.update(result['source_header'])
        self.assertEqual(result['catalog'], k.knowledge_catalog(after, {}, {}))
        db.execute('BEGIN IMMEDIATE')
        self.assertEqual(CatalogIndex(db).render(inputs(after)), result['catalog'])
        final = self.after('d')
        removed = apply_catalogued_prepared_delta_transaction(db, expected_binding=result['binding'],
            before_inputs=inputs(after), after_inputs=inputs(final), changes=[
                PreparedChange('delete', 'node', 'new'), PreparedChange('delete', 'relation', 'new-rel')])
        db.commit()
        self.assertEqual(removed['catalog'], k.knowledge_catalog(final, {}, {}))
        self.assertEqual(PublishedKnowledgeReadModel(self.path, removed['binding']).node('a')['matches'][0],
                         self.graph['nodes'][0])

    def test_canonical_order_uses_exact_item_keys_not_prepared_numeric_addresses(self):
        # The complete source graph's native order, not an arbitrary fixture.
        self.graph['nodes'].sort(key=lambda item: (item['source_graph'], item['id']))
        path = self.path.with_name('canonical.sqlite')
        self.catalog = k.knowledge_catalog(self.graph, {}, {})
        self.binding = publish_prepared(path, graph=self.graph, catalog=self.catalog)
        self.path = path
        db, _ = self.attach(CANONICAL_ORDER)
        after = self.after()
        new = copy.deepcopy(after['nodes'][0]); new.update(id='Ab', native_id='Ab', entity_id='Ab')
        after['nodes'].append(new)
        after['nodes'].sort(key=lambda item: (item['source_graph'], item['id']))
        db.execute('BEGIN IMMEDIATE')
        result = apply_catalogued_prepared_delta_transaction(db, expected_binding=self.binding,
            before_inputs=inputs(self.graph, CANONICAL_ORDER), after_inputs=inputs(after, CANONICAL_ORDER),
            changes=[PreparedChange('insert', 'node', 'Ab', new, SOURCE_ORDER_STRIDE // 2)])
        db.commit()
        after.update(result['source_header'])
        self.assertEqual(result['catalog'], k.knowledge_catalog(after, {}, {}))

    def test_failure_after_catalog_updates_rolls_back_every_lane_and_sentinel(self):
        db, _ = self.attach()
        db.execute('CREATE TABLE sentinel(value TEXT)')
        before = list(db.iterdump())
        after = self.after()
        after['nodes'][0]['display']['title'] = 'changed'
        db.execute('BEGIN IMMEDIATE')
        db.execute("INSERT INTO sentinel VALUES ('not committed')")
        with patch('tos_access.prepared_catalog.apply_prepared_delta_transaction', side_effect=RuntimeError('injected')):
            with self.assertRaisesRegex(RuntimeError, 'injected'):
                apply_catalogued_prepared_delta_transaction(db, expected_binding=self.binding,
                    before_inputs=inputs(self.graph), after_inputs=inputs(after),
                    changes=[PreparedChange('update', 'node', 'a', after['nodes'][0])])
        self.assertTrue(db.in_transaction)
        db.rollback()
        self.assertEqual(list(db.iterdump()), before)
        self.assertEqual(PublishedKnowledgeReadModel(self.path, self.binding).catalog(), self.catalog)

    def test_foreign_header_registry_index_row_and_late_budget_refuse(self):
        db, _ = self.attach()
        after = self.after()
        before_dump = list(db.iterdump())
        for scenario in ('header', 'registry', 'index-row', 'budget'):
            with self.subTest(scenario=scenario):
                before_input = inputs(self.graph)
                db.execute('BEGIN IMMEDIATE')
                limits = PublicationLimits()
                if scenario == 'header':
                    forged = copy.deepcopy(self.graph); forged['counts']['unknown']['number'] += 1
                    before_input = inputs(forged)
                elif scenario == 'registry':
                    before_input = CatalogInputs(inputs(self.graph).header, {'types': []})
                elif scenario == 'index-row':
                    db.execute("UPDATE catalog_contributors SET row_digest=? WHERE kind='node' AND id='a'", ('0' * 64,))
                else:
                    limits = PublicationLimits(max_mutations=1)
                with self.assertRaises((ValueError, RuntimeError)):
                    apply_catalogued_prepared_delta_transaction(db, expected_binding=self.binding,
                        before_inputs=before_input, after_inputs=inputs(after), limits=limits,
                        changes=[PreparedChange('update', 'node', 'a', after['nodes'][0])])
                db.rollback()
                self.assertEqual(list(db.iterdump()), before_dump)

    def test_mutating_input_generator_cannot_change_prior_captured_item(self):
        db, _ = self.attach()
        after = self.after()
        item = copy.deepcopy(after['nodes'][0]); item['display']['title'] = 'captured'
        after['nodes'][0] = copy.deepcopy(item)
        def changes():
            yield PreparedChange('update', 'node', 'a', item)
            item['display']['title'] = 'iterator side effect'
        db.execute('BEGIN IMMEDIATE')
        result = apply_catalogued_prepared_delta_transaction(db, expected_binding=self.binding,
            before_inputs=inputs(self.graph), after_inputs=inputs(after), changes=changes())
        db.commit()
        self.assertEqual(PublishedKnowledgeReadModel(self.path, result['binding']).node('a')['matches'][0]['display']['title'],
                         'captured')
        self.assertEqual(result['catalog'], k.knowledge_catalog(after, {}, {}))

    def test_mutation_budget_covers_catalog_and_prepared_writes_together(self):
        db, _ = self.attach()
        after = self.after()
        after['nodes'][0]['display']['title'] = 'measured replacement'
        before = list(db.iterdump())

        def apply(maximum):
            return apply_catalogued_prepared_delta_transaction(db, expected_binding=self.binding,
                before_inputs=inputs(self.graph), after_inputs=inputs(after),
                changes=[PreparedChange('update', 'node', 'a', after['nodes'][0])],
                limits=PublicationLimits(max_mutations=maximum))

        db.execute('BEGIN IMMEDIATE')
        measured = apply(PublicationLimits().max_mutations)['sql_mutations']
        db.rollback()
        self.assertGreater(measured, 1)
        self.assertEqual(list(db.iterdump()), before)
        db.execute('BEGIN IMMEDIATE')
        with self.assertRaisesRegex(ValueError, 'mutation budget'):
            apply(measured - 1)
        db.rollback()
        self.assertEqual(list(db.iterdump()), before)
        db.execute('BEGIN IMMEDIATE')
        result = apply(measured)
        self.assertEqual(result['sql_mutations'], measured)
        db.commit()
        self.assertEqual(PublishedKnowledgeReadModel(self.path, result['binding']).catalog(),
                         k.knowledge_catalog(after, {}, {}))

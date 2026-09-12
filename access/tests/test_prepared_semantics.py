"""Semantic/catalog/search/lens transaction composition, not source admission."""
import copy
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access import knowledge as k
from tos_access import prepared_semantics as joined
from tos_access.catalog_semantics import CatalogInputs
from tos_access.prepared_publication import PreparedChange, PublicationLimits, publish_prepared, SOURCE_ORDER_STRIDE
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
from tos_access.published_search import PublishedSearchService
from tos_access.published_lens import PublishedLensService
from test_prepared_publication import fixture
from test_indexed_lens import lens


class PreparedSemanticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parents[2]
        cls.entities = json.loads((root / 'ToS/doctrine/semantic-interchange/entity-types.v1.json').read_text())
        cls.relations = json.loads((root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_text())

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='prepared-semantics-', dir=os.environ.get('TMPDIR'))
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'publication.sqlite'
        self.graph, _ = fixture()
        self.graph['normalization_binding'] = k._normalization_binding(self.entities, self.relations)
        self.graph['counts'] = {'nodes': 3, 'relations': 1, 'unknown': {'not_a_number': False},
                               'semantic_validation': self.full(self.graph)}
        self.catalog = self.catalogue(self.graph)
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        self.db = sqlite3.connect(self.path, isolation_level=None)
        self.addCleanup(self.db.close)

    def full(self, graph):
        return k.validate_knowledge_semantics(graph, self.entities, self.relations)

    def catalogue(self, graph):
        return k.knowledge_catalog(graph, {}, {}, self.entities, self.relations)

    def inputs(self, graph):
        return CatalogInputs({key: value for key, value in graph.items() if key not in ('nodes', 'relations')},
                             self.entities, self.relations)

    def attach(self):
        self.db.execute('BEGIN IMMEDIATE')
        result = joined.bootstrap_prepared_maintenance_transaction(self.db,
            expected_binding=self.binding, inputs=self.inputs(self.graph),
            ordered_rows=lambda kind: iter(self.graph[kind + 's']))
        self.db.commit()
        return result

    def after(self):
        graph = copy.deepcopy(self.graph)
        graph['source_revision'] = 'c' * 64
        graph['counts']['semantic_validation'] = {'valid': 'untrusted input report'}
        return graph

    def apply(self, graph, changes, **kwargs):
        return joined.apply_semantic_prepared_delta_transaction(self.db,
            expected_binding=self.binding, before_inputs=self.inputs(self.graph),
            after_inputs=self.inputs(graph), changes=changes, **kwargs)

    def test_bootstrap_preserves_binding_and_recomputes_report_without_full_wrapper(self):
        before = {table: self.db.execute(f'SELECT * FROM {table} ORDER BY 1,2').fetchall()
                  for table in ('edge_meta', 'knowledge_nodes', 'knowledge_relations', 'search_documents')}
        with patch.object(k, 'validate_knowledge_semantics', side_effect=AssertionError('full wrapper')), \
             patch.object(k, 'knowledge_catalog', side_effect=AssertionError('full catalog')):
            result = self.attach()
        self.assertEqual(result['binding'], self.binding)
        self.assertEqual(result['semantic_report'], self.full(self.graph))
        self.assertFalse(result['publication_changed'])
        for table, rows in before.items():
            self.assertEqual(self.db.execute(f'SELECT * FROM {table} ORDER BY 1,2').fetchall(), rows)

    def test_update_delivers_same_report_catalog_rows_search_and_lens(self):
        self.attach()
        old = PublishedKnowledgeReadModel(self.path, self.binding)
        graph = self.after()
        graph['nodes'][0]['display']['title'] = 'Новая мысль'
        graph['nodes'][0]['attributes']['opaque'] = {'2': 9007199254740993, '1': -0.0, 'null': None}
        graph['nodes'][0]['type_id'] = 'unregistered-extension'
        expected_report = self.full(graph)
        self.db.execute('BEGIN IMMEDIATE')
        with patch.object(k, 'validate_knowledge_semantics', side_effect=AssertionError('full wrapper')), \
             patch.object(k, 'knowledge_catalog', side_effect=AssertionError('full catalog')), \
             patch.object(k, 'build_knowledge_graph', side_effect=AssertionError('full graph')):
            result = self.apply(graph, [PreparedChange('update', 'node', 'a', graph['nodes'][0])])
        self.assertTrue(self.db.in_transaction)
        self.db.commit()
        self.assertEqual(result['semantic_report'], expected_report)
        graph.update(result['source_header'])
        self.assertEqual(result['catalog'], self.catalogue(graph))
        self.assertEqual(graph['counts']['unknown'], self.graph['counts']['unknown'])
        reader = PublishedKnowledgeReadModel(self.path, result['binding'])
        self.assertEqual(reader.node('a')['matches'][0], graph['nodes'][0])
        self.assertEqual(reader.catalog(), result['catalog'])
        self.assertEqual(PublishedSearchService(reader).search('Новая мысль', limit=10)['nodes'],
                         k.search_knowledge_graph(graph, 'Новая мысль', limit=10)['nodes'])
        spec = lens(seed={'node_ids': ['a']})
        self.assertEqual(PublishedLensService(reader).execute(spec), k.execute_knowledge_lens(graph, spec))
        with self.assertRaises(PublishedSnapshotConflict):
            old.catalog()
        for flag in ('source_transition_verified', 'semantic_acceptance', 'consumer_switched'):
            self.assertFalse(result[flag])

    def test_insert_delete_with_connected_overlay_and_second_transition(self):
        self.attach()
        graph = self.after()
        node = copy.deepcopy(graph['nodes'][0]); node.update(id='new', entity_id='new', native_id='new')
        relation = copy.deepcopy(graph['relations'][0]); relation.update(id='new-r', native_id='new-r', to_id='new')
        graph['nodes'].insert(1, node); graph['relations'].append(relation)
        self.db.execute('BEGIN IMMEDIATE')
        result = self.apply(graph, [PreparedChange('insert', 'relation', 'new-r', relation, SOURCE_ORDER_STRIDE),
                                   PreparedChange('insert', 'node', 'new', node, SOURCE_ORDER_STRIDE // 2)])
        self.db.commit()
        self.assertEqual(result['semantic_report'], self.full(graph))
        graph.update(result['source_header'])
        self.assertEqual(result['catalog'], self.catalogue(graph))
        self.graph, self.binding = graph, result['binding']
        after = self.after(); after['source_revision'] = 'd' * 64
        after['nodes'] = [row for row in after['nodes'] if row['id'] != 'new']
        after['relations'] = [row for row in after['relations'] if row['id'] != 'new-r']
        self.db.execute('BEGIN IMMEDIATE')
        result = self.apply(after, [PreparedChange('delete', 'node', 'new'),
                                   PreparedChange('delete', 'relation', 'new-r')])
        self.db.commit()
        self.assertEqual(result['semantic_report'], self.full(after))
        after.update(result['source_header'])
        self.assertEqual(result['catalog'], self.catalogue(after))

    def test_failures_in_later_lanes_leave_caller_rollback_responsible_for_everything(self):
        self.attach()
        self.db.execute('CREATE TABLE sentinel(value TEXT)')
        before = list(self.db.iterdump())
        graph = self.after()
        for point in ('apply_catalogued_prepared_delta_transaction', 'verify_semantic_index_binding_transaction'):
            with self.subTest(point=point):
                self.db.execute('BEGIN IMMEDIATE')
                self.db.execute("INSERT INTO sentinel VALUES ('pending')")
                with patch.object(joined, point, side_effect=RuntimeError('injected')):
                    with self.assertRaisesRegex(RuntimeError, 'injected'):
                        self.apply(graph, [PreparedChange('update', 'node', 'a', graph['nodes'][0])])
                self.assertTrue(self.db.in_transaction)
                self.db.rollback()
                self.assertEqual(list(self.db.iterdump()), before)

    def test_combined_exact_mutation_budget_includes_final_binding_verification(self):
        self.attach()
        before = list(self.db.iterdump())
        graph = self.after()
        changes = [PreparedChange('update', 'node', 'a', graph['nodes'][0])]
        self.db.execute('BEGIN IMMEDIATE')
        measured = self.apply(graph, changes)['sql_mutations']
        self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        with self.assertRaisesRegex(ValueError, 'budget'):
            self.apply(graph, changes, limits=PublicationLimits(max_mutations=measured - 1))
        self.db.rollback()
        self.assertEqual(list(self.db.iterdump()), before)
        self.db.execute('BEGIN IMMEDIATE')
        result = self.apply(graph, changes, limits=PublicationLimits(max_mutations=measured))
        self.assertEqual(result['sql_mutations'], measured)
        self.db.commit()

    def test_generator_side_effect_cannot_split_checked_and_published_rows(self):
        self.attach()
        graph = self.after()
        item = copy.deepcopy(graph['nodes'][0]); item['display']['title'] = 'captured'
        graph['nodes'][0] = copy.deepcopy(item)
        def changes():
            yield PreparedChange('update', 'node', 'a', item)
            item['type_id'] = 'changed-after-yield'
            item['display']['title'] = 'changed-after-yield'
        self.db.execute('BEGIN IMMEDIATE')
        result = self.apply(graph, changes())
        self.db.commit()
        self.assertEqual(result['semantic_report'], self.full(graph))
        self.assertEqual(PublishedKnowledgeReadModel(self.path, result['binding']).node('a')['matches'][0],
                         graph['nodes'][0])

    def test_bootstrap_refuses_wrong_report_without_leaving_auxiliary_tables(self):
        # Re-publish a separate fixture with a false report; exact storage is
        # valid but does not make that report a semantic verification result.
        path = self.path.with_name('wrong-report.sqlite')
        graph = self.after(); catalog = self.catalogue(graph)
        binding = publish_prepared(path, graph=graph, catalog=catalog)
        with closing(sqlite3.connect(path, isolation_level=None)) as db:
            before = list(db.iterdump())
            db.execute('BEGIN IMMEDIATE')
            with self.assertRaisesRegex(ValueError, 'report'):
                joined.bootstrap_prepared_maintenance_transaction(db, expected_binding=binding,
                                                                  inputs=self.inputs(graph))
            db.rollback()
            self.assertEqual(list(db.iterdump()), before)

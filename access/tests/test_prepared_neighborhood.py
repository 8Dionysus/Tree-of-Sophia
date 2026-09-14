"""Complete bounded existing incidence for source assembly, not source truth."""
import copy
from contextlib import closing
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access import knowledge as k
from tos_access.prepared_neighborhood import capture_prepared_neighborhood, NeighborhoodLimits
from tos_access.prepared_publication import publish_prepared, apply_prepared_delta, PreparedChange
from tos_access.published_read_model import (PublishedKnowledgeReadModel, PublishedReadLimits,
    PublishedReadBudgetExceeded, PublishedSnapshotConflict)
from tos_access.published_read_metadata import PublishedReadModelError
from test_prepared_publication import fixture


class PreparedNeighborhoodTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='prepared-neighborhood-', dir=os.environ.get('TMPDIR'))
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / 'snapshot.sqlite'
        self.graph, self.catalog = fixture()
        self.graph['nodes'][1]['entity_id'] = self.graph['nodes'][0]['entity_id']
        # Two representations plus outgoing, incoming, self-loop, and a relation
        # of an opposite endpoint which must not be mistaken for seed incidence.
        original = self.graph['relations'][0]
        for identifier, left, right in (('incoming', 'b', 'A'), ('self', 'a', 'a'), ('opposite', 'b', 'b')):
            row = copy.deepcopy(original)
            row.update(id=identifier, native_id=identifier, from_id=left, to_id=right)
            self.graph['relations'].append(row)
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        self.reader = PublishedKnowledgeReadModel(self.path, self.binding)

    def capture(self, **kwargs):
        return capture_prepared_neighborhood(self.reader, **kwargs)

    def test_exact_union_full_rows_order_and_no_full_source_fallback(self):
        with patch.object(k, 'build_knowledge_graph', side_effect=AssertionError('full graph')):
            packet = self.capture(entity_id='a-entity')
        self.assertEqual(packet['seed_node_ids'], ['A', 'a'])
        self.assertEqual(packet['binding'], self.binding)
        self.assertEqual(packet['nodes'], sorted(self.graph['nodes'], key=lambda n: n['id']))
        self.assertEqual(packet['relations'], sorted(self.graph['relations'][:3], key=lambda r: r['id']))
        self.assertEqual(len(packet['relations']), 3)  # Self-loop is not duplicated.
        with closing(sqlite3.connect(self.path)) as db:
            for kind, orders in packet['source_order'].items():
                for identifier, order in orders.items():
                    self.assertEqual(order, db.execute('SELECT source_order FROM prepared_documents '
                        'WHERE kind=? AND id=?', (kind, identifier)).fetchone()[0])
        scope = packet['scope']
        self.assertTrue(scope['complete_existing_seed_incidence_verified'])
        self.assertTrue(scope['returned_endpoint_closure_verified'])
        self.assertFalse(any(value for key, value in scope.items() if key not in (
            'complete_existing_seed_incidence_verified', 'returned_endpoint_closure_verified')))
        packet['nodes'][0]['probe']['false'] = 'not retained across reads'
        self.assertFalse(self.capture(entity_id='a-entity')['nodes'][0]['probe']['false'])

    def test_exact_node_seed_is_not_entity_alias_or_transitive_expansion(self):
        packet = self.capture(node_ids=['a'])
        self.assertEqual(packet['seed_node_ids'], ['a'])
        self.assertEqual([n['id'] for n in packet['nodes']], ['a', 'b'])
        self.assertEqual([r['id'] for r in packet['relations']], ['r', 'self'])
        with self.assertRaises(PublishedReadModelError):
            self.capture(node_ids=['a-entity'])
        with self.assertRaises(ValueError):
            self.capture(entity_id='missing')

    def test_union_limits_refuse_instead_of_returning_incomplete_graph(self):
        for limits in (NeighborhoodLimits(max_seed_nodes=1), NeighborhoodLimits(max_nodes=2),
                       NeighborhoodLimits(max_relations=2)):
            with self.subTest(limits=limits), self.assertRaises(PublishedReadBudgetExceeded):
                self.capture(entity_id='a-entity', limits=limits)
        # Each seed individually fits the relation limit but the union does not.
        with self.assertRaises(PublishedReadBudgetExceeded):
            self.capture(node_ids=['A', 'a'], limits=NeighborhoodLimits(max_relations=2))
        for limits in (PublishedReadLimits(max_vm_steps=100), PublishedReadLimits(max_rows=4),
                       PublishedReadLimits(max_response_bytes=128), PublishedReadLimits(max_row_bytes=128)):
            reader = PublishedKnowledgeReadModel(self.path, self.binding, limits=limits)
            with self.subTest(limits=limits), self.assertRaises(PublishedReadModelError):
                capture_prepared_neighborhood(reader, node_ids=['a'])

    def test_invalid_selector_is_rejected_before_opening(self):
        with patch.object(self.reader, '_connect', side_effect=AssertionError('unexpected database open')):
            for kwargs in ({}, {'node_ids': ['a', 'a']}, {'node_ids': ['a'], 'entity_id': 'a'},
                           {'node_ids': iter(['a'])}, {'node_ids': ['']}, {'entity_id': 1},
                           {'entity_id': '界' * 4096}):
                with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                    self.capture(**kwargs)

    def test_row_digest_endpoint_or_order_drift_refuses(self):
        for statement, args in (
            ('UPDATE knowledge_nodes SET json=? WHERE id=?', ('{}', 'b')),
            ('UPDATE knowledge_relations SET from_id=? WHERE id=?', ('A', 'r')),
            ('DELETE FROM knowledge_nodes WHERE id=?', ('b',)),
            ('DELETE FROM prepared_documents WHERE kind=? AND id=?', ('node', 'a')),
            ('UPDATE prepared_documents SET source_order=? WHERE kind=? AND id=?', (-1, 'relation', 'r')),
        ):
            with self.subTest(statement=statement), tempfile.TemporaryDirectory(dir=self.tmp.name) as folder:
                path = Path(folder) / 'corrupt.sqlite'
                binding = publish_prepared(path, graph=self.graph, catalog=self.catalog)
                with closing(sqlite3.connect(path)) as db:
                    db.execute(statement, args)
                    db.commit()
                reader = PublishedKnowledgeReadModel(path, binding)
                with self.assertRaises(PublishedReadModelError):
                    capture_prepared_neighborhood(reader, entity_id='a-entity')

    def test_named_but_wrong_index_cannot_enable_scan_fallback(self):
        for expression in ('(id,from_id)', '(from_id COLLATE NOCASE,id)',
                           '(from_id,id) WHERE id<>\'r\''):
            with self.subTest(expression=expression), closing(sqlite3.connect(self.path)) as db:
                db.execute('DROP INDEX knowledge_relations_from_seek')
                db.execute('CREATE INDEX knowledge_relations_from_seek ON knowledge_relations' + expression)
                db.commit()
                with self.assertRaises(PublishedReadModelError):
                    self.capture(node_ids=['a'])

    def test_publication_during_read_is_rejected_by_post_transaction_observation(self):
        original = self.reader._snapshot
        calls = 0
        def observe(read):
            nonlocal calls
            calls += 1
            if calls == 2:
                # The first transaction has ended. Change the actual prepared
                # clock, not an injected success/failure result in the reader.
                with closing(sqlite3.connect(self.path)) as db:
                    db.execute('UPDATE knowledge_exploration_clock SET epoch=epoch+1 WHERE singleton=1')
                    db.commit()
            return original(read)
        with patch.object(self.reader, '_snapshot', side_effect=observe), \
                self.assertRaises(PublishedSnapshotConflict):
            self.capture(entity_id='a-entity')
        self.assertEqual(calls, 2)

    def test_successive_publication_and_return_to_prior_content_reject_old_binding(self):
        header = {key: value for key, value in self.graph.items() if key not in ('nodes', 'relations')}
        current = self.binding
        for title in ('changed', self.graph['nodes'][0]['display']['title']):
            node = copy.deepcopy(self.graph['nodes'][0])
            node['display']['title'] = title
            current = apply_prepared_delta(self.path, expected_binding=current,
                source_header=header, catalog=self.catalog, changes=[PreparedChange('update', 'node', 'a', node)])
        with self.assertRaises(PublishedSnapshotConflict):
            self.capture(node_ids=['a'])
        self.assertEqual(capture_prepared_neighborhood(PublishedKnowledgeReadModel(self.path, current),
            node_ids=['a'])['binding'], current)


if __name__ == '__main__':
    unittest.main()

from __future__ import annotations

import copy
import ast
import hashlib
import json
import sqlite3
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

ACCESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS_ROOT / 'src'))

from catalog_fixture import cases, fixture
from tos_access.catalog_index import CatalogIndex, CatalogIndexError, CatalogIndexBudgetError, CatalogLimits
from tos_access.catalog_semantics import (CANONICAL_ORDER, CatalogChange, CatalogInputs,
                                         CatalogRow, catalog_digest, finalized_header, order_key, order_value, owner_json)
from tos_access.knowledge import knowledge_catalog
from tos_access.normalization_cache import normalization_processor_digest

ORACLE = json.loads(Path(__file__).with_name('catalog_oracle.json').read_text(encoding='utf-8'))


def rows(graph):
    return (CatalogRow(kind, item['id'], order, item) for kind, field in
            (('node', 'nodes'), ('relation', 'relations')) for order, item in enumerate(graph[field]))


def inputs(args):
    return CatalogInputs.from_graph(*args)


class CatalogIndexTests(unittest.TestCase):
    def index(self, args=None, **limits):
        args = fixture() if args is None else args
        connection = sqlite3.connect(':memory:')
        self.addCleanup(connection.close)
        connection.execute('BEGIN')
        index = CatalogIndex(connection, CatalogLimits(**limits))
        result = index.bootstrap(inputs(args), rows(args[0]))
        return connection, index, args, result

    def test_frozen_original_oracle_for_both_routes(self):
        for name, args in cases():
            with self.subTest(name=name):
                expected = ORACLE['cases'][name]
                native = knowledge_catalog(*args)
                self.assertEqual(catalog_digest(native), expected)
                self.assertEqual(hashlib.sha256(owner_json(native).encode()).hexdigest(), ORACLE['wire_cases'][name])
                connection, index, _, result = self.index(args)
                self.assertEqual(catalog_digest(result), expected)
                self.assertEqual(hashlib.sha256(owner_json(result).encode()).hexdigest(), ORACLE['wire_cases'][name])
                self.assertEqual(index.render(inputs(args)), result)
                self.assertTrue(connection.in_transaction)

    def test_catalog_wrapper_is_outside_normalization_fingerprint(self):
        path = ACCESS_ROOT / 'src/tos_access/knowledge.py'
        tree = ast.parse(path.read_text(encoding='utf-8'))
        wrapper = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                       and node.name == 'knowledge_catalog')
        wrapper.body = ast.parse('return {"changed_catalog_only": True}').body
        modified = SimpleNamespace(read_text=lambda **_: ast.unparse(tree))
        self.assertEqual(normalization_processor_digest(path), normalization_processor_digest(modified))

    def test_native_small_fixture_with_full_registries_lenses_and_semantic_counts(self):
        import test_knowledge_contract as native
        from tos_access.knowledge import build_knowledge_graph
        native.KnowledgeContractTests.setUpClass()
        owner = native.KnowledgeContractTests()
        corpus, philosophy = owner.fixture()
        graph = build_knowledge_graph(corpus, philosophy,
            entity_type_registry=owner.entity_type_registry,
            relation_type_registry=owner.relation_type_registry)
        args = graph, corpus, philosophy, owner.entity_type_registry, owner.relation_type_registry
        _, index, _, result = self.index(args)
        self.assertEqual(result, knowledge_catalog(*args))
        self.assertEqual(result['counts'], graph['counts'])
        self.assertTrue(result['lenses'])
        self.assertTrue(result['semantic_registries']['entity_types']['entries'])
        self.assertEqual(index.render(inputs(args)), result)

    def test_deletion_promotes_beyond_five_examples_and_representative(self):
        for name in ('delete-first', 'delete-six', 'empty'):
            with self.subTest(name=name):
                connection, index, before, _ = self.index()
                after = dict(cases())[name]
                keep = {kind: {row['id'] for row in after[0][field]}
                        for kind, field in (('node', 'nodes'), ('relation', 'relations'))}
                changes = [CatalogChange('delete', row.kind, row.id, catalog_digest(row.item))
                           for row in rows(before[0]) if row.id not in keep[row.kind]]
                result = index.apply_delta(inputs(before), inputs(after), changes)
                self.assertEqual(catalog_digest(result), ORACLE['cases'][name])
                self.assertEqual(index.render(inputs(after)), result)
                self.assertFalse(connection.execute('SELECT 1 FROM catalog_totals WHERE n<=0').fetchone())

    def test_route_change_rechecks_both_incident_sides_and_self_loop_once(self):
        connection, index, before, _ = self.index()
        after = dict(cases())['route-change']
        changes = [CatalogChange('update', 'node', new['id'], catalog_digest(old), new)
                   for old, new in zip(before[0]['nodes'], after[0]['nodes']) if old != new]
        touched = []
        connection.set_trace_callback(touched.append)
        result = index.apply_delta(inputs(before), inputs(after), changes)
        connection.set_trace_callback(None)
        self.assertEqual(catalog_digest(result), ORACLE['cases']['route-change'])
        author = next(route for route in result['capabilities']['entity_routes'] if route['route_id'] == 'author')
        self.assertEqual(author['semantic_confirming_relation_count'], 0)
        self.assertTrue(any('from_id=' in sql for sql in touched))
        self.assertTrue(any('to_id=' in sql for sql in touched))
        self.assertFalse(any('FROM catalog_contributors' in sql and sql.startswith('SELECT')
                             and 'WHERE kind=' not in sql for sql in touched))

    def test_label_only_change_never_visits_adjacency(self):
        connection, index, before, _ = self.index()
        after = copy.deepcopy(before)
        after[0]['nodes'][0]['display']['kind_label'] = {'en': 'Changed first representative'}
        old, new = before[0]['nodes'][0], after[0]['nodes'][0]
        traced = []
        connection.set_trace_callback(traced.append)
        result = index.apply_delta(inputs(before), inputs(after), [
            CatalogChange('update', 'node', new['id'], catalog_digest(old), new)])
        connection.set_trace_callback(None)
        self.assertEqual(result, knowledge_catalog(*after))
        self.assertFalse(any('SELECT id FROM catalog_contributors' in sql for sql in traced))
        self.assertTrue(all('from_id=' not in sql for sql in traced if sql.startswith('SELECT')))

    def test_relation_endpoint_replacement_and_same_transaction_node_insertion(self):
        _, index, before, _ = self.index()
        after = copy.deepcopy(before)
        node = copy.deepcopy(after[0]['nodes'][0]); node['id'] = 'n9'
        after[0]['nodes'].append(node)
        old = before[0]['relations'][0]
        new = after[0]['relations'][0]
        new['from_id'] = 'n9'; new['to_id'] = 'n9'
        # Caller order intentionally presents the relation before its new node.
        result = index.apply_delta(inputs(before), inputs(after), [
            CatalogChange('update', 'relation', new['id'], catalog_digest(old), new),
            CatalogChange('insert', 'node', 'n9', new_item=node, source_order=9)])
        self.assertEqual(result, knowledge_catalog(*after))

    def test_dangling_endpoint_refuses_and_caller_rollback_restores_everything(self):
        connection, index, before, result = self.index()
        connection.commit(); connection.execute('BEGIN')
        after = copy.deepcopy(before); old = after[0]['nodes'].pop(0)
        with self.assertRaisesRegex(CatalogIndexError, 'endpoint is absent'):
            index.apply_delta(inputs(before), inputs(after), [CatalogChange('delete', 'node', old['id'], catalog_digest(old))])
        self.assertTrue(connection.in_transaction)
        with self.assertRaisesRegex(CatalogIndexError, 'instance failed'):
            index.render(inputs(before))
        connection.rollback(); connection.execute('BEGIN')
        self.assertEqual(CatalogIndex(connection).render(inputs(before)), result)

    def test_missing_index_transaction_and_binding_refusals(self):
        connection = sqlite3.connect(':memory:'); self.addCleanup(connection.close)
        with self.assertRaisesRegex(CatalogIndexError, 'caller-owned transaction'):
            CatalogIndex(connection).bootstrap(inputs(fixture()), rows(fixture()[0]))
        connection.execute('BEGIN')
        with self.assertRaisesRegex(CatalogIndexError, 'absent or incompatible'):
            CatalogIndex(connection).render(inputs(fixture()))
        self.assertFalse(connection.execute("SELECT name FROM sqlite_master WHERE name LIKE 'catalog_%'").fetchone())
        connection, index, before, _ = self.index()
        after = copy.deepcopy(before); after[3]['types'][0]['parent_type_ids'] = ['changed']
        with self.assertRaisesRegex(CatalogIndexError, 'binding mismatch'):
            index.apply_delta(inputs(before), inputs(after), [])

    def test_wrong_old_digest_duplicate_change_and_order_refuse(self):
        for scenario in ('digest', 'duplicate', 'order', 'missing-order'):
            with self.subTest(scenario=scenario):
                connection, index, args, _ = self.index()
                row = args[0]['nodes'][0]
                change = CatalogChange('update', 'node', row['id'], catalog_digest(row), row)
                if scenario == 'digest':
                    changes = [CatalogChange('update', 'node', row['id'], '0' * 64, row)]
                elif scenario == 'duplicate':
                    changes = [change, change]
                else:
                    new = {**row, 'id': 'new'}
                    changes = [CatalogChange('insert', 'node', 'new', new_item=new,
                                             source_order=0 if scenario == 'order' else None)]
                with self.assertRaises((CatalogIndexError, sqlite3.IntegrityError)):
                    index.apply_delta(inputs(args), inputs(args), changes)
                self.assertTrue(connection.in_transaction)

    def test_row_owned_counts_and_explicit_after_semantic_report(self):
        args = fixture()
        args[0]['counts'] = {'nodes': 9, 'relations': 3, 'sources': {'fixture': 9},
            'display_coverage': {'node_titles': 9, 'node_summaries': 9, 'nodes_without_source_summary': 4,
                                 'owner_extra': ['retain']},
            'semantic_mapping': {'mapped_nodes': 9, 'owner_extra': 'retain'},
            'semantic_validation': {'valid': True, 'owner_revision': 'before'},
            'unknown_count': {'nested': True}}
        _, index, before, _ = self.index(args)
        after = copy.deepcopy(before)
        old = after[0]['nodes'].pop(6)
        after[0]['source_revision'] = 'after'
        after[0]['counts']['semantic_validation'] = {'valid': True, 'owner_revision': 'after'}
        result = index.apply_delta(inputs(before), inputs(after), [
            CatalogChange('delete', 'node', old['id'], catalog_digest(old))])
        counts = result['counts']
        self.assertEqual(counts['nodes'], 8)
        self.assertEqual(counts['sources'], {'fixture': 8})
        self.assertEqual(counts['display_coverage']['node_titles'], 8)
        self.assertEqual(counts['display_coverage']['owner_extra'], ['retain'])
        self.assertEqual(counts['semantic_mapping']['owner_extra'], 'retain')
        self.assertEqual(counts['semantic_validation']['owner_revision'], 'after')
        self.assertEqual(counts['unknown_count'], {'nested': True})

    def test_selected_fact_corruption_and_aggregate_corruption_refuse(self):
        for damage in ('fact', 'aggregate', 'head', 'state'):
            with self.subTest(damage=damage):
                connection, index, args, _ = self.index()
                if damage == 'fact':
                    connection.execute('UPDATE catalog_contributors SET facts_digest=? WHERE kind=? AND id=?',
                                       ('0' * 64, 'node', 'n6'))
                    row = args[0]['nodes'][6]
                    operation = lambda: index.apply_delta(inputs(args), inputs(args), [
                        CatalogChange('update', 'node', row['id'], catalog_digest(row), row)])
                elif damage == 'aggregate':
                    connection.execute('UPDATE catalog_totals SET n=n+1 WHERE key=(SELECT key FROM catalog_totals LIMIT 1)')
                    operation = lambda: index.render(inputs(args))
                elif damage == 'head':
                    connection.execute('DELETE FROM catalog_heads WHERE bucket=(SELECT atom FROM catalog_atoms WHERE value=?)',
                                       ('["node","facet-order","view_ids"]',))
                    operation = lambda: index.render(inputs(args))
                else:
                    connection.execute('UPDATE catalog_state SET projector=?', ('0' * 64,))
                    operation = lambda: index.render(inputs(args))
                with self.assertRaises(CatalogIndexError):
                    operation()

    def test_budget_refusals_do_not_commit(self):
        for limits in ({'max_catalog_bytes': 1}, {'max_catalog_entries': 1}, {'max_row_bytes': 1},
                       {'max_index_bytes': 1}):
            with self.subTest(limits=limits):
                connection = sqlite3.connect(':memory:'); self.addCleanup(connection.close)
                connection.execute('BEGIN')
                with self.assertRaises(CatalogIndexBudgetError):
                    CatalogIndex(connection, CatalogLimits(**limits)).bootstrap(inputs(fixture()), rows(fixture()[0]))
                self.assertTrue(connection.in_transaction)
                connection.rollback()
                self.assertFalse(connection.execute("SELECT name FROM sqlite_master WHERE name LIKE 'catalog_%'").fetchone())
        connection, _, args, _ = self.index()
        index = CatalogIndex(connection, CatalogLimits(max_incident_relations=1))
        node = copy.deepcopy(args[0]['nodes'][7]); node['kind_id'] = 'place'; node['type_id'] = 'tos.entity.place'
        with self.assertRaises(CatalogIndexBudgetError):
            index.apply_delta(inputs(args), inputs(args), [
                CatalogChange('update', 'node', node['id'], catalog_digest(args[0]['nodes'][7]), node)])

    def test_copy_isolation_and_no_row_reads_during_render(self):
        connection, index, args, expected = self.index()
        bound = inputs(args)
        bound.header['source_revision'] = 'not-mutated'
        bound.entity_type_registry['types'].clear()
        def authorize(action, table, column, database, trigger):
            if action == sqlite3.SQLITE_READ and table in ('catalog_contributors', 'catalog_occurrences'):
                return sqlite3.SQLITE_DENY
            return sqlite3.SQLITE_OK
        connection.set_authorizer(authorize)
        try:
            actual = index.render(bound)
        finally:
            connection.set_authorizer(None)
        self.assertEqual(actual, expected)
        actual['node_kinds'].clear()
        self.assertEqual(index.render(bound), expected)

    def test_canonical_order_preserves_python_tuple_unicode_and_nul_order(self):
        components = ('', '\x00', '\x00x', 'a', 'a\x00', 'a"', 'a\\', 'aa', 'é', 'Я', '😀')
        values = [(left, right) for left in components for right in reversed(components)]
        self.assertEqual(sorted(values), sorted(values, key=order_key))
        self.assertEqual([order_value(order_key(value)) for value in values], values)
        connection = sqlite3.connect(':memory:'); self.addCleanup(connection.close)
        connection.execute('CREATE TABLE ordering (value BLOB PRIMARY KEY)')
        connection.executemany('INSERT INTO ordering VALUES(?)', ((order_key(value),) for value in values))
        self.assertEqual([order_value(row[0]) for row in connection.execute('SELECT value FROM ordering ORDER BY value')],
                         sorted(values))

    def test_canonical_insertion_between_neighbors_and_source_move_are_addressed(self):
        args = fixture()
        connection = sqlite3.connect(':memory:'); self.addCleanup(connection.close)
        connection.execute('BEGIN')
        index = CatalogIndex(connection)
        def bound(value):
            return CatalogInputs.from_graph(*value, source_order_profile=CANONICAL_ORDER)
        stream = (CatalogRow(row.kind, row.id, (str(row.item['source_graph']), row.id), row.item)
                  for row in rows(args[0]))
        index.bootstrap(bound(args), stream)
        after = copy.deepcopy(args)
        new = copy.deepcopy(args[0]['nodes'][0]); new['id'] = 'n0.5'
        new['attributes']['examples'] = ['inserted']
        after[0]['nodes'].append(new)
        after[0]['nodes'].sort(key=lambda item: (str(item['source_graph']), str(item['id'])))
        before_rows = dict(connection.execute('SELECT id,source_order FROM catalog_contributors WHERE kind="node"'))
        result = index.apply_delta(bound(args), bound(after), [CatalogChange(
            'insert', 'node', new['id'], new_item=new, source_order=('fixture', 'n0.5'))])
        self.assertEqual(result, knowledge_catalog(*after))
        for identifier, order in before_rows.items():
            self.assertEqual(connection.execute('SELECT source_order FROM catalog_contributors WHERE kind="node" AND id=?',
                                                (identifier,)).fetchone()[0], order)
        moved = copy.deepcopy(after)
        old = next(item for item in after[0]['nodes'] if item['id'] == 'n6')
        new = next(item for item in moved[0]['nodes'] if item['id'] == 'n6'); new['source_graph'] = 'earlier'
        moved[0]['nodes'].sort(key=lambda item: (str(item['source_graph']), str(item['id'])))
        result = index.apply_delta(bound(after), bound(moved), [CatalogChange(
            'update', 'node', new['id'], catalog_digest(old), new, ('earlier', 'n6'))])
        self.assertEqual(result, knowledge_catalog(*moved))

    def test_sequence_reorder_swaps_positions_atomically(self):
        _, index, before, _ = self.index()
        after = dict(cases())['reverse-order']
        old = {(row.kind, row.id): row for row in rows(before[0])}
        changes = [CatalogChange('update', row.kind, row.id, catalog_digest(old[row.kind, row.id].item),
                                 row.item, row.source_order) for row in rows(after[0])]
        result = index.apply_delta(inputs(before), inputs(after), changes)
        self.assertEqual(catalog_digest(result), ORACLE['cases']['reverse-order'])

    def test_canonical_profile_rejects_mismatched_row_order(self):
        connection = sqlite3.connect(':memory:'); self.addCleanup(connection.close)
        connection.execute('BEGIN')
        args = fixture()
        with self.assertRaisesRegex(CatalogIndexError, 'canonical catalog order'):
            CatalogIndex(connection).bootstrap(CatalogInputs.from_graph(*args, source_order_profile=CANONICAL_ORDER),
                                                rows(args[0]))

    def test_streaming_delta_stops_at_byte_budget_without_consuming_rest(self):
        connection, _, args, _ = self.index()
        index = CatalogIndex(connection, CatalogLimits(max_delta_bytes=1))
        yielded = []
        def changes():
            for row in rows(args[0]):
                yielded.append(row.id)
                yield CatalogChange('update', row.kind, row.id, catalog_digest(row.item), row.item)
        with self.assertRaises(CatalogIndexBudgetError):
            index.apply_delta(inputs(args), inputs(args), changes())
        self.assertEqual(yielded, ['n0'])

    def test_binding_damage_on_unselected_endpoint_refuses_route_recompute(self):
        connection, index, args, _ = self.index()
        connection.execute('UPDATE catalog_contributors SET summary=? WHERE kind=? AND id=?',
                           ('{}', 'node', 'n0'))
        old = args[0]['relations'][0]
        new = copy.deepcopy(old); new['attributes']['added'] = True
        with self.assertRaisesRegex(CatalogIndexError, 'endpoint summary binding'):
            index.apply_delta(inputs(args), inputs(args), [
                CatalogChange('update', 'relation', new['id'], catalog_digest(old), new)])

    def test_rendered_baseline_damage_cannot_be_republished_by_delta(self):
        connection, index, args, _ = self.index()
        connection.execute('UPDATE catalog_totals SET n=n+1 WHERE key=(SELECT key FROM catalog_totals LIMIT 1)')
        with self.assertRaisesRegex(CatalogIndexError, 'before aggregate'):
            index.apply_delta(inputs(args), inputs(args), [])

    def test_disappearing_last_source_and_count_histograms_are_removed(self):
        args = fixture()
        args[0]['relations'] = []
        args[0]['counts'] = {'nodes': 9, 'relations': 0, 'sources': {'fixture': 9},
                            'display_coverage': {'node_summary_states': {'source': 9}}}
        _, index, _, _ = self.index(args)
        after = copy.deepcopy(args); after[0]['nodes'] = []
        result = index.apply_delta(inputs(args), inputs(after), [
            CatalogChange('delete', row.kind, row.id, catalog_digest(row.item)) for row in rows(args[0])])
        self.assertEqual(result['counts'], {'nodes': 0, 'relations': 0, 'sources': {},
                                            'display_coverage': {'node_summary_states': {}}})

    def test_draft_after_final_before_header_binding_is_explicit_and_exact(self):
        args = fixture()
        args[0]['relations'] = []
        args[0]['counts'] = {'nodes': 9, 'relations': 0, 'sources': {'fixture': 9},
                            'semantic_validation': {'owner': 'before'}, 'unknown': {'keep': True}}
        connection, index, _, _ = self.index(args)
        after = copy.deepcopy(args)
        removed = after[0]['nodes'].pop(6)
        after[0]['source_revision'] = 'after'
        after[0]['counts']['semantic_validation'] = {'owner': 'after'}
        draft = inputs(after)
        catalog = index.apply_delta(inputs(args), draft, [
            CatalogChange('delete', 'node', removed['id'], catalog_digest(removed))])
        final = finalized_header(draft, catalog)
        self.assertEqual(draft.header['counts']['nodes'], 9)
        self.assertEqual(final['counts']['nodes'], 8)
        self.assertEqual(final['counts']['semantic_validation'], {'owner': 'after'})
        self.assertEqual(final['counts']['unknown'], {'keep': True})
        def bind(header):
            return CatalogInputs(header, draft.entity_type_registry, draft.relation_type_registry, draft.lenses)
        self.assertEqual(index.render(bind(final)), catalog)
        self.assertEqual(index.apply_delta(bind(final), bind(final), []), catalog)
        for damaged in ('nodes', 'semantic_validation'):
            header = copy.deepcopy(final)
            header['counts'][damaged] = 999 if damaged == 'nodes' else {'owner': 'tampered'}
            with self.subTest(damaged=damaged), self.assertRaisesRegex(CatalogIndexError, 'header digest'):
                CatalogIndex(connection).render(bind(header))
            with self.assertRaisesRegex(CatalogIndexError, 'before header'):
                CatalogIndex(connection).apply_delta(bind(header), bind(final), [])
        with self.assertRaisesRegex(CatalogIndexError, 'header digest'):
            CatalogIndex(connection).render(draft)

    def test_storage_ceiling_is_preallocation_and_never_raises_owner_limit(self):
        connection = sqlite3.connect(':memory:'); self.addCleanup(connection.close)
        connection.execute('PRAGMA max_page_count=128')
        connection.execute('BEGIN')
        index = CatalogIndex(connection, CatalogLimits(max_index_bytes=8 * 1024 * 1024))
        args = fixture(); index.bootstrap(inputs(args), rows(args[0]))
        self.assertEqual(connection.execute('PRAGMA max_page_count').fetchone()[0], 128)
        connection = sqlite3.connect(':memory:'); self.addCleanup(connection.close)
        connection.execute('BEGIN')
        index = CatalogIndex(connection, CatalogLimits(max_index_bytes=65536))
        with self.assertRaises(CatalogIndexBudgetError):
            index.bootstrap(inputs(args), rows(args[0]))
        self.assertLessEqual(connection.execute('PRAGMA page_count').fetchone()[0] *
                             connection.execute('PRAGMA page_size').fetchone()[0], 65536)

    def test_physical_schema_drift_refuses_and_interrupt_never_commits(self):
        connection, index, args, _ = self.index()
        connection.execute('DROP INDEX catalog_from')
        with self.assertRaisesRegex(CatalogIndexError, 'physical schema'):
            index.render(inputs(args))
        connection = sqlite3.connect(':memory:'); self.addCleanup(connection.close)
        connection.execute('BEGIN')
        index = CatalogIndex(connection)
        def interrupted():
            yield next(rows(args[0]))
            raise KeyboardInterrupt('synthetic owner interruption')
        with self.assertRaises(KeyboardInterrupt):
            index.bootstrap(inputs(args), interrupted())
        self.assertTrue(connection.in_transaction)
        with self.assertRaisesRegex(CatalogIndexError, 'instance failed'):
            index.render(inputs(args))
        connection.rollback()
        self.assertFalse(connection.execute("SELECT name FROM sqlite_master WHERE name GLOB 'catalog_*'").fetchone())


if __name__ == '__main__':
    unittest.main()

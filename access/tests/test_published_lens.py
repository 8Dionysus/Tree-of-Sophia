"""Actual emitted-v9 SQL/native-v7 full-packet parity, without corpus loads."""
from __future__ import annotations

import copy
import asyncio
import http.client
import importlib.util
import json
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import unittest
import threading
from contextlib import closing
from http.server import ThreadingHTTPServer
from pathlib import Path
from unittest.mock import patch

from test_published_exploration import builder, write_fixture, ToSAccessCore
from test_indexed_lens import graph_for, lens, scenarios
from tos_access import knowledge as k
from tos_access.http_server import build_handler
from tos_access.published_lens import PublishedLensService, PublishedLensLimits, _Plan
from tos_access.published_read_metadata import (
    TOP_KEY, LENS_META_KEY, READER_SCHEMA, LOCAL_READ_MODEL_SCHEMA, _compact, emitted_row_digest,
    published_row_digest_key, published_snapshot_binding, lens_order_row,
)
from tos_access.published_read_model import (
    PublishedKnowledgeReadModel, PublishedReadBudgetExceeded, PublishedReadModelError,
    PublishedSnapshotConflict, _Read,
)


class PublishedLensTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(temporary.cleanup)
        cls.root = Path(temporary.name)
        write_fixture(cls.root)
        core = ToSAccessCore.discover(cls.root)
        snapshot = copy.deepcopy(core.knowledge_snapshot())
        graph = snapshot['graph']
        synthetic = graph_for(seed=7)
        remap = {'philosophy:n04': 'philosophy:İ04', 'philosophy:n05': 'philosophy:I05'}
        values = (None, False, 0, 'İßЁ', ['İßЁ', False, 0], 3.5)
        for index, node in enumerate(synthetic['nodes']):
            node['id'] = remap.get(node['id'], node['id'])
            node['attributes']['probe'] = values[index % len(values)]
        for index, relation in enumerate(synthetic['relations']):
            for endpoint in ('from_id', 'to_id'):
                relation[endpoint] = remap.get(relation[endpoint], relation[endpoint])
            relation['id'] = ('Relation:' if index % 3 else 'relation:İ') + str(index)
        for key in ('nodes', 'relations', 'query_properties'):
            graph[key] = synthetic[key]
        cls.graph = builder.normalize_paths(graph, cls.root)
        snapshot['graph'] = cls.graph
        cls.stored = lens(seed={'node_ids': ['philosophy:n00']})
        cls.stored['lens_id'] = 'prepared-test-lens'
        snapshot['catalog']['lenses'] = [cls.stored]
        target = cls.root / 'runtime/read-model.sql'
        with patch.object(builder, 'REPO_ROOT', cls.root), patch.object(ToSAccessCore, 'knowledge_snapshot', return_value=snapshot):
            builder.build_read_model_sql(core, target, 'a' * 64)
        cls.seed = cls.root / 'published.sqlite'
        with closing(sqlite3.connect(cls.seed)) as db:
            db.executescript(target.read_text())
            db.executescript((builder.WORKER_ROOT / 'migrations/0001-exploration.sql').read_text())
            top = json.loads(''.join(row[0] for row in db.execute('SELECT json_chunk FROM edge_meta WHERE key=? ORDER BY part', (TOP_KEY,))))
            epoch = db.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()[0]
            cls.binding = published_snapshot_binding(top, epoch)
        assert sum(path.stat().st_size for path in cls.root.rglob('*') if path.is_file()) < 64 * 1024 * 1024

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.path = Path(temporary.name) / 'published.sqlite'
        shutil.copyfile(self.seed, self.path)
        self.reader = PublishedKnowledgeReadModel(self.path, self.binding)
        self.service = PublishedLensService(self.reader)

    def parity(self, spec):
        expected = k.execute_knowledge_lens(self.graph, spec)
        actual = self.service.execute(spec)
        self.assertEqual(actual, expected)
        return actual

    def prepared_core(self):
        return ToSAccessCore.discover(self.root, published_read_model_path=self.path,
                                     published_read_model_expected=self.binding)

    def test_core_routes_focus_compilation_and_stored_lens_without_source_fallback(self):
        core = self.prepared_core()
        expected_focus = k.focus_knowledge_node(self.graph, 'n00')
        expected_lens = k.execute_knowledge_lens(self.graph, self.stored)
        with patch.object(ToSAccessCore, 'knowledge_graph', side_effect=AssertionError('graph fallback')), patch.object(
                ToSAccessCore, 'knowledge_snapshot', side_effect=AssertionError('snapshot fallback')):
            self.assertEqual(core.knowledge_focus('n00'), expected_focus)
            self.assertEqual(core.compile_knowledge_lens(self.stored), expected_lens)
            self.assertEqual(core.stored_knowledge_lens('prepared-test-lens'), expected_lens)
            with self.assertRaises(KeyError):
                core.stored_knowledge_lens('absent')

    def test_stored_lens_refuses_publication_between_catalog_and_execution(self):
        core = self.prepared_core()
        catalog = core.knowledge_catalog()
        def superseded(_core):
            with closing(sqlite3.connect(self.path)) as db:
                db.execute("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'")
                db.commit()
            return catalog
        with patch.object(ToSAccessCore, 'knowledge_catalog', superseded):
            with self.assertRaises(PublishedSnapshotConflict):
                core.stored_knowledge_lens('prepared-test-lens')

    def test_http_focus_compile_and_stored_lens_use_exact_native_packets(self):
        core = self.prepared_core()
        expected_focus = k.focus_knowledge_node(self.graph, 'n00')
        expected_lens = k.execute_knowledge_lens(self.graph, self.stored)
        server = ThreadingHTTPServer(('127.0.0.1', 0), build_handler(core, self.root))
        thread = threading.Thread(target=server.serve_forever, kwargs={'poll_interval': 0.01}, daemon=True)
        thread.start()
        def request(path, payload=None):
            with closing(http.client.HTTPConnection('127.0.0.1', server.server_port, timeout=5)) as connection:
                connection.request('GET' if payload is None else 'POST', path,
                    body=None if payload is None else json.dumps(payload),
                    headers={} if payload is None else {'Content-Type': 'application/json'})
                response = connection.getresponse()
                return response.status, json.loads(response.read())
        try:
            with patch.object(ToSAccessCore, 'knowledge_graph', side_effect=AssertionError('graph fallback')):
                self.assertEqual(request('/api/knowledge/focus/n00'), (200, expected_focus))
                self.assertEqual(request('/api/knowledge/lenses/compile', self.stored), (200, expected_lens))
                self.assertEqual(request('/api/knowledge/lenses/prepared-test-lens'), (200, expected_lens))
                with closing(sqlite3.connect(self.path)) as db:
                    db.execute("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'")
                    db.commit()
                self.assertEqual(request('/api/knowledge/focus/n00')[0], 409)
                self.assertEqual(request('/api/knowledge/lenses/compile', self.stored)[0], 409)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    @unittest.skipUnless(importlib.util.find_spec('mcp'), 'mcp dependency is not installed')
    def test_mcp_shared_dispatch_focus_compile_and_stored_lens(self):
        from tos_access.mcp_server import build_server
        core = self.prepared_core()
        requests = [
            ('tos_knowledge_focus', {'node_id': 'n00'}, k.focus_knowledge_node(self.graph, 'n00')),
            ('tos_knowledge_lens_compile', {'spec': self.stored}, k.execute_knowledge_lens(self.graph, self.stored)),
            ('tos_knowledge_lens_open', {'lens_id': 'prepared-test-lens'}, k.execute_knowledge_lens(self.graph, self.stored)),
        ]
        with patch.object(ToSAccessCore, 'knowledge_graph', side_effect=AssertionError('graph fallback')):
            server = build_server(core=core)
            for name, arguments, expected in requests:
                with self.subTest(tool=name):
                    result = asyncio.run(server.call_tool(name, arguments))
                    self.assertEqual(result[1], expected)
                    self.assertEqual(json.loads(result[0][0].text), expected)

    def test_full_native_matrix_exact_packets_fingerprints_forms_and_counts(self):
        for number, spec in enumerate(scenarios()):
            with self.subTest(scenario=number):
                self.parity(spec)

    def test_explicit_local_profile_reads_without_edge_search_compatibility_planes(self):
        # This is a reader-profile fixture, not evidence of the local publisher.
        with closing(sqlite3.connect(self.path)) as db:
            for table in ('knowledge_search_documents', 'knowledge_search_grams', 'knowledge_search_gram_stats'):
                db.execute(f'DROP TABLE {table}')
            top = json.loads(db.execute('SELECT json_chunk FROM edge_meta WHERE key=?', (TOP_KEY,)).fetchone()[0])
            top['read_model_schema'] = LOCAL_READ_MODEL_SCHEMA
            db.execute('UPDATE edge_meta SET json_chunk=? WHERE key=?', (_compact(top), TOP_KEY))
            db.commit()
        binding = published_snapshot_binding(top, self.binding['publication_epoch'])
        reader = PublishedKnowledgeReadModel(self.path, binding)
        service = PublishedLensService(reader)
        self.assertEqual(reader.status()['read_model_schema'], LOCAL_READ_MODEL_SCHEMA)
        self.assertEqual(service.capability()['read_model_schema'], LOCAL_READ_MODEL_SCHEMA)
        self.assertEqual(reader.catalog()['lenses'], [self.stored])
        self.assertEqual(service.focus('n00'), k.focus_knowledge_node(self.graph, 'n00'))
        self.assertEqual(service.execute(self.stored), k.execute_knowledge_lens(self.graph, self.stored))
        with self.assertRaises(PublishedSnapshotConflict):
            self.reader.status()  # The edge-v9 binding cannot silently select local-v1.
        top['schema'] = READER_SCHEMA
        del top['lens_sha256']
        with self.assertRaises(PublishedReadModelError):
            published_snapshot_binding(top, self.binding['publication_epoch'])

    def test_focus_exact_entity_native_scopes(self):
        for identifier in ('philosophy:n00', 'tos.synthetic.subject.0', 'n00'):
            for sources in (None, ['philosophy']):
                with self.subTest(identifier=identifier, sources=sources):
                    self.assertEqual(self.service.focus(identifier, sources=sources),
                                     k.focus_knowledge_node(self.graph, identifier, sources=sources))

    def test_pagination_and_replay_use_identical_native_cursors(self):
        spec = lens(composition={'endpoint_policy': 'independent'},
                    limits={'nodes': 14, 'relations': 30}, pagination={'nodes': 3, 'relations': 3})
        seen = set()
        for _ in range(30):
            packet = self.parity(spec)
            self.assertEqual(self.service.execute(spec), packet)
            cursor = packet['page']['next_cursor']
            if cursor is None:
                break
            self.assertNotIn(cursor, seen)
            seen.add(cursor)
            spec = {**spec, 'pagination': {'nodes': 3, 'relations': 3, 'cursor': cursor}}
        else:
            self.fail('bounded lens pagination did not terminate')
        self.assertTrue(seen)

    def test_cold_requests_never_build_index_graph_catalog_or_digest(self):
        statements = []
        original = PublishedKnowledgeReadModel._connect
        def connect(reader):
            db = original(reader)
            db.set_trace_callback(statements.append)
            return db
        with patch.object(PublishedKnowledgeReadModel, '_connect', connect), patch.object(
                k, 'KnowledgeGraphIndex', side_effect=AssertionError('full index')), patch(
                'tos_access.core.build_knowledge_graph', side_effect=AssertionError('full graph')), patch(
                'tos_access.search_read_model.SQLiteKnowledgeSearchReadModel._snapshot_digest', side_effect=AssertionError('whole digest')):
            self.service.focus('n00', node_limit=6, relation_limit=8)
            self.service.execute(lens(limits={'nodes': 5, 'relations': 5}))
            status = self.reader.status()
        self.assertFalse(status['verifies_all_rows'])
        self.assertFalse(status['writes_to_tree'])
        self.assertFalse(any("key='knowledge_catalog'" in sql or "key='knowledge_top'" in sql for sql in statements))
        self.assertFalse(any(' OFFSET ' in sql.upper() for sql in statements))
        self.assertFalse(any(sql.startswith(('INSERT', 'UPDATE', 'CREATE', 'DELETE')) for sql in statements))

    def test_budgets_refuse_without_returning_approximate_counts(self):
        for limits, spec in (
            (PublishedLensLimits(max_candidates=1), lens(seed={'text_query': 'Узел'})),
            (PublishedLensLimits(max_callback_calls=1), lens()),
            (PublishedLensLimits(max_decoded_bytes=1), lens()),
            (PublishedLensLimits(max_sort_bytes=1), lens(composition={'sort_nodes': [{'field': 'id', 'direction': 'desc'}]})),
            (PublishedLensLimits(max_path_steps=1), lens(seed={'node_ids': ['philosophy:n00']},
                path_query=[{'path_id': 'negative', 'quantifier': 'not_exists', 'steps': [
                    {'node_query': {'filters': [{'field': 'id', 'op': 'eq', 'value': 'absent'}]}}]}])),
        ):
            with self.subTest(limits=limits):
                with self.assertRaises(PublishedReadBudgetExceeded):
                    PublishedLensService(self.reader, limits=limits).execute(spec)

    def test_missing_metadata_or_order_index_refuses(self):
        for sql, args in (
                ('DELETE FROM edge_meta WHERE key=?', (LENS_META_KEY,)),
                ('DROP INDEX knowledge_lens_order_sort', ())):
            with self.subTest(sql=sql):
                shutil.copyfile(self.seed, self.path)
                with closing(sqlite3.connect(self.path)) as db:
                    db.execute(sql, args)
                    db.commit()
                with self.assertRaises(PublishedReadModelError):
                    self.service.execute(lens())

    def test_native_unicode_scalar_list_false_null_filters_and_mixed_sorts(self):
        for operation, value in (
                ('eq', False), ('eq', 0), ('eq', 'İßЁ'), ('neq', False), ('in', ['İßЁ', False]),
                ('contains', 'i'), ('contains', [False, 0]), ('prefix', 'İ'),
                ('exists', True), ('exists', False), ('gt', 0), ('gte', 0), ('lt', 4), ('lte', 0)):
            with self.subTest(operation=operation, value=value):
                self.parity(lens(node_query={'filters': [{'field': 'attributes.probe', 'op': operation, 'value': value}]},
                    composition={'endpoint_policy': 'independent',
                        'sort_nodes': [{'field': 'attributes.probe', 'direction': 'desc'}, {'field': 'id', 'direction': 'asc'}],
                        'sort_relations': [{'field': 'predicate_id', 'direction': 'desc'}, {'field': 'id', 'direction': 'desc'}]}))

    def test_selected_order_corruption_refuses_even_focus_depth_zero(self):
        for sql in ("DELETE FROM knowledge_lens_order WHERE kind='node' AND id='philosophy:n00'",
                    "UPDATE knowledge_lens_order SET sort_key='wrong' WHERE kind='node' AND id='philosophy:n00'"):
            with self.subTest(sql=sql):
                shutil.copyfile(self.seed, self.path)
                with closing(sqlite3.connect(self.path)) as db:
                    db.execute(sql)
                    db.commit()
                with self.assertRaises(PublishedReadModelError):
                    self.service.focus('n00', depth=0)

    def test_v8_legacy_inspect_stays_available_but_lens_requires_v9(self):
        with closing(sqlite3.connect(self.path)) as db:
            top = json.loads(db.execute('SELECT json_chunk FROM edge_meta WHERE key=?', (TOP_KEY,)).fetchone()[0])
            top.pop('lens_sha256')
            top.update(schema=READER_SCHEMA, read_model_schema='tos_cloudflare_edge_read_model_v8')
            db.execute('UPDATE edge_meta SET json_chunk=? WHERE key=?', (_compact(top), TOP_KEY))
            db.execute('DROP TABLE knowledge_lens_order')
            db.commit()
        expected = published_snapshot_binding(top, self.binding['publication_epoch'])
        reader = PublishedKnowledgeReadModel(self.path, expected)
        self.assertEqual(reader.status()['read_model_schema'], 'tos_cloudflare_edge_read_model_v8')
        self.assertEqual(reader.node('philosophy:n00'), k.inspect_knowledge_node(self.graph, 'philosophy:n00'))
        self.assertFalse(PublishedLensService(reader).capability()['available'])
        with self.assertRaisesRegex(PublishedReadModelError, 'requires.*v9'):
            PublishedLensService(reader).focus('n00')

    def test_aba_replay_and_concurrent_publication_refuse(self):
        self.service.focus('n00')
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'")
            db.commit()
        with self.assertRaises(PublishedSnapshotConflict):
            self.service.focus('n00')
        shutil.copyfile(self.seed, self.path)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute('PRAGMA journal_mode=WAL')
        original = self.reader._snapshot
        calls = 0
        def snapshot(read):
            nonlocal calls
            top = original(read)
            calls += 1
            if calls == 1:
                with closing(sqlite3.connect(self.path)) as db:
                    db.execute("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'")
                    db.commit()
            return top
        with patch.object(self.reader, '_snapshot', snapshot), self.assertRaises(PublishedSnapshotConflict):
            self.service.focus('n00')

    def test_unrelated_rows_and_high_degree_do_not_scale_default_focus_reads(self):
        observations = []
        seeded_observations = []
        seeded_spec = lens(seed={'node_ids': ['philosophy:n01']},
            node_query={'filters': [{'property_id': 'tos.property.synthetic-score', 'op': 'eq', 'value': 1}]},
            relation_query={'enabled': False},
            path_query=[{'path_id': 'typed-neighbor', 'steps': [{'node_query': {'filters': [
                {'property_id': 'tos.property.synthetic-score', 'op': 'eq', 'value': 2}]}}]}])
        for size in (5000, 10000):
            shutil.copyfile(self.seed, self.path)
            oracle = copy.deepcopy(self.graph)
            with closing(sqlite3.connect(self.path)) as db:
                # Lean synthetic emitted rows isolate index pressure. The
                # compared focus's selected full rows remain the actual builder
                # fixture above; no corpus or whole graph is materialized.
                for index in range(size):
                    identifier = f'zz:node:{index:05d}'
                    node = dict(id=identifier, entity_id=identifier, native_id=identifier,
                                source_graph='philosophy', kind_id='concept', type_id='tos.entity.concept')
                    edge_id = f'zz:edge:{index:05d}'
                    edge = dict(id=edge_id, native_id=edge_id, from_id='philosophy:n00', to_id=identifier,
                                source_graph='philosophy', predicate_id='supports', relation_type_id='tos.relation.supports')
                    oracle['nodes'].append(node)
                    oracle['relations'].append(edge)
                    db.execute('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?,?)',
                               (*[node[key] for key in ('id', 'entity_id', 'native_id', 'source_graph', 'kind_id', 'type_id')], '', '', '', _compact(node)))
                    db.execute('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?,?)',
                               (*[edge[key] for key in ('id', 'native_id', 'source_graph', 'from_id', 'to_id', 'predicate_id', 'relation_type_id')], '', '', '', _compact(edge)))
                    for kind, item in (('node', node), ('relation', edge)):
                        db.execute('INSERT INTO knowledge_lens_order VALUES (?,?,?,?,?)', lens_order_row(kind, item))
                        db.execute('INSERT INTO edge_meta VALUES (?,0,?)',
                                   (published_row_digest_key(kind, item['id']), _compact(emitted_row_digest(_compact(item)))))
                metadata = json.loads(db.execute('SELECT json_chunk FROM edge_meta WHERE key=?', (LENS_META_KEY,)).fetchone()[0])
                for kind, dimensions in (('node', ('source_graph', 'kind_id', 'type_id')),
                                         ('relation', ('source_graph', 'predicate_id', 'relation_type_id'))):
                    item = node if kind == 'node' else edge
                    cells = {tuple(cell[:3]): cell[3] for cell in metadata[kind + '_counts']}
                    key = tuple(item[field] for field in dimensions)
                    cells[key] = cells.get(key, 0) + size
                    metadata[kind + '_counts'] = [[*key, count] for key, count in sorted(cells.items())]
                raw = _compact(metadata)
                db.execute('UPDATE edge_meta SET json_chunk=? WHERE key=?', (raw, LENS_META_KEY))
                top = json.loads(db.execute('SELECT json_chunk FROM edge_meta WHERE key=?', (TOP_KEY,)).fetchone()[0])
                top['lens_sha256'] = emitted_row_digest(raw)['sha256']
                db.execute('UPDATE edge_meta SET json_chunk=? WHERE key=?', (_compact(top), TOP_KEY))
                db.commit()
            self.assertLess(self.path.stat().st_size, 64 * 1024 * 1024)
            reader = PublishedKnowledgeReadModel(self.path, published_snapshot_binding(top, self.binding['publication_epoch']))
            captured = []
            original = _Read.query
            def query(read, sql, args=()):
                result = original(read, sql, args)
                captured.append((read.rows, read.bytes, read.steps))
                return result
            with patch.object(_Read, 'query', query):
                result = PublishedLensService(reader).focus('n00', node_limit=3, relation_limit=5)
            self.assertEqual(result['counts']['available_nodes'], len(self.graph['nodes']) + size)
            self.assertEqual(result, k.focus_knowledge_node(oracle, 'n00', node_limit=3, relation_limit=5))
            observations.append(captured[-1])
            captured.clear()
            with patch.object(_Read, 'query', query):
                seeded = PublishedLensService(reader).execute(seeded_spec)
            self.assertEqual(seeded, k.execute_knowledge_lens(oracle, seeded_spec))
            self.assertEqual([node['id'] for node in seeded['nodes']], ['philosophy:n01'])
            seeded_observations.append(captured[-1])
        self.assertEqual(observations[0][0], observations[1][0])
        self.assertLessEqual(observations[1][1] - observations[0][1], 10)
        self.assertLessEqual(observations[1][2] - observations[0][2], 1000)
        self.assertEqual(seeded_observations[0][0], seeded_observations[1][0])
        self.assertLessEqual(seeded_observations[1][1] - seeded_observations[0][1], 10)
        self.assertLessEqual(seeded_observations[1][2] - seeded_observations[0][2], 1000)

    def test_seed_identity_union_preserves_all_aliases_dedup_and_sources(self):
        for identifiers in (['n13'], ['tos.synthetic.subject.0'],
                            ['philosophy:n01', 'n01', 'tos.synthetic.subject.0'], ['absent']):
            for sources in (['philosophy', 'repository'], ['repository']):
                with self.subTest(identifiers=identifiers, sources=sources):
                    spec = lens(seed={'node_ids': identifiers}, sources=sources, relation_query={'enabled': False})
                    expected = k.execute_knowledge_lens(self.graph, spec)
                    actual = PublishedLensService(self.reader, limits=PublishedLensLimits(block_size=1)).execute(spec)
                    self.assertEqual(actual, expected)

    def test_exact_identity_filters_push_down_without_charging_unrelated_rows(self):
        cases = (
            ('node', 'id', 'eq', 'repository:other', 1),
            ('node', 'id', 'in', ['absent', 'repository:other'], 1),
            ('node', 'native_id', 'eq', 'n12', 1),
            ('node', 'entity_id', 'eq', 'tos.synthetic.subject.6', 3),
            ('relation', 'id', 'eq', 'relation:İ39', 1),
            ('relation', 'native_id', 'eq', 'r039', 1),
        )
        for kind, field, operation, value, max_candidates in cases:
            with self.subTest(kind=kind, field=field, value=value):
                query = {'filters': [{'field': field, 'op': operation, 'value': value}]}
                spec = lens(
                    node_query=query if kind == 'node' else {'enabled': False},
                    relation_query=query if kind == 'relation' else {'enabled': False},
                    composition={'endpoint_policy': 'independent'},
                    limits={'nodes': 20, 'relations': 20},
                )
                expected = k.execute_knowledge_lens(self.graph, spec)
                statements = []
                original = _Read.query

                def query_read(read, sql, args=()):
                    statements.append(sql)
                    return original(read, sql, args)

                with patch.object(_Read, 'query', query_read):
                    actual = PublishedLensService(
                        self.reader,
                        limits=PublishedLensLimits(max_candidates=max_candidates),
                    ).execute(spec)
                self.assertEqual(actual, expected)
                table = 'knowledge_nodes' if kind == 'node' else 'knowledge_relations'
                self.assertTrue(
                    any(table in sql and 'json_each' in sql and f'{field} IN' in sql for sql in statements),
                    statements,
                )

    def test_exact_identity_filter_groups_keep_native_boolean_semantics(self):
        cases = (
            {'match': 'all', 'filters': [
                {'field': 'id', 'op': 'eq', 'value': 'philosophy:n12'},
                {'field': 'native_id', 'op': 'neq', 'value': 'n11'},
            ]},
            {'match': 'any', 'filters': [
                {'field': 'id', 'op': 'eq', 'value': 'repository:other'},
                {'field': 'native_id', 'op': 'eq', 'value': 'n12'},
            ]},
            # The non-identity disjunct must keep the complete bounded scan;
            # exact candidates cannot stand in for an OR branch.
            {'match': 'any', 'filters': [
                {'field': 'id', 'op': 'eq', 'value': 'repository:other'},
                {'field': 'source_graph', 'op': 'eq', 'value': 'philosophy'},
            ]},
            {'match': 'all', 'filters': [
                {'field': 'id', 'op': 'neq', 'value': 'absent'},
            ]},
            {'match': 'all', 'filters': [
                {'field': 'id', 'op': 'in', 'value': []},
            ]},
            {'match': 'any', 'filters': [
                {'field': 'id', 'op': 'in', 'value': []},
            ]},
            {'match': 'all', 'filters': [
                {'field': 'id', 'op': 'eq', 'value': 0},
            ]},
        )
        for number, query in enumerate(cases):
            with self.subTest(node_case=number):
                self.parity(lens(node_query=query, relation_query={'enabled': False}, limits={'nodes': 30, 'relations': 0}))

        for field, value in (('id', 'relation:İ39'), ('native_id', 'r039')):
            with self.subTest(relation_field=field):
                self.parity(lens(
                    node_query={'enabled': False},
                    relation_query={'filters': [{'field': field, 'op': 'eq', 'value': value}]},
                    composition={'endpoint_policy': 'independent'},
                    limits={'nodes': 20, 'relations': 20},
                ))

    def test_non_string_identity_values_keep_native_unknown_semantics(self):
        # Emitted index columns stringify missing fields; they cannot alone
        # answer a predicate against null or another non-string packet value.
        for field in ('id', 'native_id', 'entity_id'):
            for operation, value in (('eq', None), ('eq', 0), ('eq', ['n12']),
                                     ('in', ['n12', None]), ('in', [False])):
                with self.subTest(field=field, operation=operation, value=value):
                    rule = {'field': field, 'op': operation, 'value': value}
                    self.assertIsNone(_Plan._identity_selector('node', rule))
                    spec = lens(node_query={'filters': [rule]},
                                relation_query={'enabled': False},
                                limits={'nodes': 30, 'relations': 0})
                    if operation == 'eq' and isinstance(value, list):
                        with self.assertRaises(ValueError):
                            k.execute_knowledge_lens(self.graph, spec)
                        with self.assertRaises(ValueError):
                            self.service.execute(spec)
                    else:
                        self.parity(spec)

    def test_complete_eligible_stream_cannot_hide_missing_order_rows(self):
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("DELETE FROM knowledge_lens_order WHERE kind='relation' AND id=?", (self.graph['relations'][0]['id'],))
            db.commit()
        with self.assertRaises(PublishedReadModelError):
            self.service.execute(lens(composition={'endpoint_policy': 'independent'}))

    def test_invalid_owner_metadata_framing_refuses_even_with_matching_hash(self):
        for field, value in (('unicode_version', 'incompatible'), ('execution_version', 'obsolete'),
                             ('query_properties', [None]), ('node_counts', [['philosophy', 'concept', 'kind', -1]])):
            with self.subTest(field=field):
                shutil.copyfile(self.seed, self.path)
                with closing(sqlite3.connect(self.path)) as db:
                    metadata = json.loads(db.execute('SELECT json_chunk FROM edge_meta WHERE key=?', (LENS_META_KEY,)).fetchone()[0])
                    metadata[field] = value
                    raw = _compact(metadata)
                    db.execute('UPDATE edge_meta SET json_chunk=? WHERE key=?', (raw, LENS_META_KEY))
                    top = json.loads(db.execute('SELECT json_chunk FROM edge_meta WHERE key=?', (TOP_KEY,)).fetchone()[0])
                    top['lens_sha256'] = emitted_row_digest(raw)['sha256']
                    db.execute('UPDATE edge_meta SET json_chunk=? WHERE key=?', (_compact(top), TOP_KEY))
                    db.commit()
                reader = PublishedKnowledgeReadModel(self.path, published_snapshot_binding(top, self.binding['publication_epoch']))
                with self.assertRaises(PublishedReadModelError):
                    PublishedLensService(reader).focus('n00')

    def test_genuinely_cold_process_preserves_complete_packet(self):
        program = '''
import json, sys
from unittest.mock import patch
from tos_access.published_read_model import PublishedKnowledgeReadModel
from tos_access.published_lens import PublishedLensService
with patch('tos_access.core.build_knowledge_graph', side_effect=AssertionError('graph build')), patch('tos_access.knowledge.KnowledgeGraphIndex', side_effect=AssertionError('full index')):
    reader = PublishedKnowledgeReadModel(sys.argv[1], json.loads(sys.argv[2]))
    print(json.dumps(PublishedLensService(reader).execute(json.loads(sys.argv[3])), ensure_ascii=False))
'''
        spec = lens(seed={'text_query': 'Узел'}, limits={'nodes': 4, 'relations': 5})
        result = subprocess.run([sys.executable, '-c', program, str(self.path), _compact(self.binding), _compact(spec)],
                                check=False, capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), k.execute_knowledge_lens(self.graph, spec))


if __name__ == '__main__':
    unittest.main()

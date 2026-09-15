"""Native source navigation joins the same D1 delta, not a partial product."""
import copy
import json
from pathlib import Path
import sqlite3
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / 'access/deploy/cloudflare-worker/scripts')]
import build_runtime as full
import prepared_delta_runtime as delta
import source_navigation_delta_runtime as navigation
import test_prepared_source_binding as fixtures
from tos_access.projection_store import Collection, write_projection
from tos_access.projection_mutation import ProjectionSnapshotView
from tos_access.prepared_source_binding import PreparedSourceInputs
from incremental_runtime import prepare_search_address_indexes_transaction


class SourceNavigationDeltaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        fixtures.PreparedSourceBindingTests.setUpClass()

    def setUp(self):
        self.source = fixtures.PreparedSourceBindingTests()
        self.source.setUp()
        self.addCleanup(self.source.doCleanups)
        self.f = self.source.f
        self.root = Path(self.f.tmp.name)
        self.raw = {
            'schema_version': 'tos_source_navigation_v1',
            'authority_boundary': 'synthetic read-only navigation',
            'nodes': [{'node_id': 'person', 'node_kind': 'agent', 'source_ref': 'synthetic/person.json',
                       'label': "Человек — λόγος '", 'identity_status': 'provisional',
                       'properties': {'unknown': {'false': False, 'zero': 0.0}}}],
            'edges': [{'edge_id': 'edge', 'from_id': 'person', 'to_id': 'person',
                       'edge_kind': 'version', 'predicate_id': 'has_record_version',
                       'review_status': 'unreviewed', 'source_refs': ['synthetic/person.json']}],
            'rights': [{'rights_id': 'rights', 'scope_refs': ['person'],
                        'assessment_status': 'unknown', 'review_status': 'unreviewed'}],
        }
        self.counts(self.raw)

    @staticmethod
    def counts(raw):
        raw['counts'] = {key: len(raw[key]) for key in ('nodes', 'edges', 'rights')}

    def view(self, name, raw):
        path = self.root / name / 'navigation.json'
        write_projection(path, {k: v for k, v in raw.items() if k not in ('nodes', 'edges', 'rights')},
            {key: Collection(raw[key], field, (field,)) for key, field in
             (('nodes', 'node_id'), ('edges', 'edge_id'), ('rights', 'rights_id'))}, work_dir=self.root)
        return ProjectionSnapshotView(path.read_bytes(), path)

    def full(self, graph, catalog, revision, name, navigation_raw):
        carriers = full.ProducerCarrierSet.admit(corpus={'source_navigation': navigation_raw},
            philosophy={}, knowledge=graph, knowledge_catalog=catalog, evidence={}, philosophy_audit={},
            word_analysis_capability={'available': False}, carrier_paths={},
            logical_bindings={'source_revision': graph['source_revision']})
        path = self.root / name / 'read-model.sql'
        full.build_read_model_sql(None, path, revision, carriers, emit_delta_baseline=False)
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        db.executescript(path.read_text())
        db.executescript((ROOT / 'access/deploy/cloudflare-worker/migrations/0001-exploration.sql').read_text())
        return db

    def prepare(self, *, available=True):
        before_view = self.view('before', self.raw)
        previous = self.source.before
        self.source.before = PreparedSourceInputs(source_revision=previous.value()['source_revision'],
            source_publication=previous.value()['source_publication'], dependencies=previous.value()['dependencies'],
            roots={**previous.roots(), 'source-navigation': before_view})
        self.source.attach()
        self.before = sqlite3.connect(':memory:')
        self.addCleanup(self.before.close)
        self.f.db.backup(self.before)  # Tiny synthetic fixture only.
        self.d1 = self.full(self.f.graph, self.f.catalog, 'd' * 64, 'old', self.raw if available else {})
        self.d1.execute('BEGIN IMMEDIATE')
        prepare_search_address_indexes_transaction(self.d1, expected_revision='d' * 64)
        self.d1.commit()
        raw = copy.deepcopy(self.raw)
        raw['nodes'][0]['label'] = 'Исправлено'
        raw['nodes'].append({'node_id': 'new', 'node_kind': 'work', 'label': 'Новое', 'source_ref': '',
                            'identity_status': 'provisional', 'properties': {}})
        raw['edges'] = []
        raw['rights'][0]['assessment_status'] = 'restricted'
        self.counts(raw)
        after_view = self.view('before', raw)
        graph, after = self.source.delta()
        after = PreparedSourceInputs(source_revision=after.value()['source_revision'],
            source_publication=after.value()['source_publication'], dependencies=after.value()['dependencies'],
            roots={**after.roots(), 'source-navigation': after_view})
        self.f.db.execute('BEGIN IMMEDIATE')
        result = self.source.apply(graph, after)
        self.f.db.commit()
        graph.update(result['source_header'])
        return raw, graph, result

    def capture(self, result, **options):
        for db in (self.d1, self.before, self.f.db):
            db.execute('BEGIN')
        try:
            return delta.build_prepared_delta_sql(self.d1, self.before, self.f.db, self.root / 'delta.sql',
                expected_d1_revision='d' * 64, before_binding=self.f.binding,
                after_binding=result['binding'], rollback_target=self.root / 'rollback.sql', **options)
        finally:
            for db in (self.d1, self.before, self.f.db):
                db.rollback()

    def test_joint_navigation_knowledge_rights_and_exact_rollback(self):
        raw, graph, result = self.prepare()
        before = {table: self.d1.execute(f'SELECT * FROM {table} ORDER BY ' + ','.join(delta.PRIMARY_KEYS[table])).fetchall()
                  for table in delta.COLUMNS}
        with patch.object(full, 'build_read_model_sql', side_effect=AssertionError('full rebuild')):
            receipt = self.capture(result)
        self.assertEqual(receipt['source_navigation_product']['state'], 'maintained')
        sql = (self.root / 'delta.sql').read_text()
        publish = 'INSERT OR REPLACE INTO tos_delta_publications SELECT'
        stage, rest = sql.split(publish, 1)
        self.d1.executescript(stage)
        for table in navigation.projection.COLUMNS:
            order = ','.join(delta.PRIMARY_KEYS[table])
            self.assertEqual(self.d1.execute(f'SELECT * FROM {table} ORDER BY {order}').fetchall(),
                             before[table], table)
        self.d1.executescript(publish + rest)
        self.d1.executescript((self.root / 'delta.sql').read_text())  # Same delta replay.
        oracle = self.full(graph, result['catalog'], receipt['target_d1_revision'], 'oracle', raw)
        # The full emitter retains input key order while immutable delta inputs
        # are canonicalized. Compare semantic rows below, but independently
        # require each physical checksum to match its own exact emitted bytes.
        for db in (self.d1, oracle):
            checksums = {}
            for kind in ('nodes', 'edges', 'rights'):
                table = 'source_navigation_' + kind
                key = navigation.projection.COLUMNS[table][0]
                for identifier, encoded in db.execute(f'SELECT {key},json FROM {table}'):
                    self.assertTrue(encoded)  # This fixture is entirely inline.
                    digest_key = navigation.published_source_navigation_digest_key(kind, identifier)
                    checksums[digest_key] = navigation.emitted_row_digest(encoded)
            actual_checksums = {key: json.loads(encoded) for key, part, encoded in db.execute(
                "SELECT * FROM edge_meta WHERE key GLOB 'source_navigation_row_digest:*'")
                if part == 0}
            self.assertEqual(actual_checksums, checksums)
            header_raw = ''.join(row[0] for row in db.execute(
                "SELECT json_chunk FROM edge_meta WHERE key='source_navigation_top' ORDER BY part",
            ))
            header_digest = db.execute(
                "SELECT json_chunk FROM edge_meta WHERE key=? AND part=0",
                (navigation.SOURCE_NAVIGATION_HEADER_DIGEST_KEY,),
            ).fetchone()
            self.assertIsNotNone(header_digest)
            self.assertEqual(json.loads(header_digest[0]), navigation.emitted_row_digest(header_raw))
        for table, columns in delta.COLUMNS.items():
            # Legacy positional ord is not consumed by native navigation;
            # all owned JSON, selection fields, payloads and other lanes match.
            selected = ','.join(column for column in columns if column != 'ord')
            order = ','.join(delta.PRIMARY_KEYS[table])
            actual = self.d1.execute(f'SELECT {selected} FROM {table} ORDER BY {order}').fetchall()
            expected = oracle.execute(f'SELECT {selected} FROM {table} ORDER BY {order}').fetchall()
            if table == 'edge_meta':
                excluded = lambda row: (
                    row[0].startswith('source_navigation_row_digest:')
                    or row[0] == navigation.SOURCE_NAVIGATION_HEADER_DIGEST_KEY
                )
                actual = [row for row in actual if not excluded(row)]
                expected = [row for row in expected if not excluded(row)]
            def semantic(rows):
                return [[json.dumps(json.loads(value), sort_keys=True, ensure_ascii=False)
                         if table in ('source_navigation_nodes', 'source_navigation_edges',
                                      'source_navigation_rights')
                         and isinstance(value, str) and value[:1] in ('{', '[') else value
                         for value in row] for row in rows]
            self.assertEqual(semantic(actual), semantic(expected), table)
        self.d1.executescript((self.root / 'rollback.sql').read_text())
        for table, expected in before.items():
            order = ','.join(delta.PRIMARY_KEYS[table])
            self.assertEqual(self.d1.execute(f'SELECT * FROM {table} ORDER BY {order}').fetchall(), expected, table)

    def test_unavailable_product_stays_empty_not_partially_published(self):
        _, _, result = self.prepare(available=False)
        receipt = self.capture(result)
        self.assertEqual(receipt['source_navigation_product']['state'], 'unavailable')
        self.d1.executescript((self.root / 'delta.sql').read_text())
        top = self.d1.execute(
            "SELECT json_chunk FROM edge_meta WHERE key='source_navigation_top' ORDER BY part"
        ).fetchone()[0]
        self.assertEqual(top, '{}')
        header_digest = self.d1.execute(
            "SELECT json_chunk FROM edge_meta WHERE key=? AND part=0",
            (navigation.SOURCE_NAVIGATION_HEADER_DIGEST_KEY,),
        ).fetchone()
        self.assertEqual(json.loads(header_digest[0]), navigation.emitted_row_digest(top))
        for table in navigation.projection.COLUMNS:
            self.assertIsNone(self.d1.execute(f'SELECT 1 FROM {table} LIMIT 1').fetchone())

    def test_inconsistent_predecessor_refuses_before_sql(self):
        _, _, result = self.prepare()
        self.d1.execute("UPDATE source_navigation_nodes SET label='tampered' WHERE node_id='person'")
        self.d1.commit()
        with self.assertRaisesRegex(ValueError, 'predecessor serving row differs'):
            self.capture(result)
        self.assertFalse((self.root / 'delta.sql').exists())

    def test_budget_exhaustion_has_no_partial_sql(self):
        _, _, result = self.prepare()
        with self.assertRaises((ValueError, RuntimeError)):
            self.capture(result, limits=delta.PreparedD1DeltaLimits(max_changes=1))
        self.assertFalse((self.root / 'delta.sql').exists())

    def test_missing_or_wrong_header_digest_refuses_before_sql(self):
        _, _, result = self.prepare()
        key = navigation.SOURCE_NAVIGATION_HEADER_DIGEST_KEY
        row = self.d1.execute(
            'SELECT * FROM edge_meta WHERE key=?', (key,)
        ).fetchone()
        self.d1.execute('DELETE FROM edge_meta WHERE key=?', (key,))
        self.d1.commit()
        with self.assertRaisesRegex(ValueError, 'header digest'):
            self.capture(result)
        self.d1.execute('INSERT INTO edge_meta VALUES (?,?,?)', row)
        self.d1.commit()
        self.d1.execute(
            'UPDATE edge_meta SET json_chunk=? WHERE key=? AND part=0', ('{}', key)
        )
        self.d1.commit()
        with self.assertRaisesRegex(ValueError, 'header digest'):
            self.capture(result)
        self.assertFalse((self.root / 'delta.sql').exists())

    def test_missing_or_orphan_digest_refuses_before_sql(self):
        _, _, result = self.prepare()
        key = navigation.published_source_navigation_digest_key('nodes', 'person')
        row = self.d1.execute('SELECT * FROM edge_meta WHERE key=?', (key,)).fetchone()
        self.d1.execute('DELETE FROM edge_meta WHERE key=?', (key,))
        self.d1.commit()
        with self.assertRaisesRegex(ValueError, 'digest requires explicit product migration'):
            self.capture(result)
        self.d1.execute('INSERT INTO edge_meta VALUES (?,?,?)', row)
        orphan = navigation.published_source_navigation_digest_key('nodes', 'new')
        self.d1.execute('INSERT INTO edge_meta VALUES (?,0,?)', (orphan, '{}'))
        self.d1.commit()
        with self.assertRaisesRegex(ValueError, 'orphan digest'):
            self.capture(result)
        self.assertFalse((self.root / 'delta.sql').exists())

    def test_unavailable_header_with_live_rows_is_not_optional(self):
        _, _, result = self.prepare()
        self.d1.execute("UPDATE edge_meta SET json_chunk='{}' WHERE key='source_navigation_top'")
        self.d1.commit()
        with self.assertRaisesRegex(ValueError, 'unavailable native navigation contains'):
            self.capture(result)
        self.assertFalse((self.root / 'delta.sql').exists())

    def test_overflow_payload_survives_update_and_exact_reverse(self):
        self.raw['nodes'][0]['properties']['oversized'] = '🙂' * 2_060_000
        _, _, result = self.prepare()
        before = self.d1.execute('SELECT * FROM source_navigation_node_payload ORDER BY id,part').fetchall()
        self.assertGreater(len(before), 256)
        self.capture(result)
        self.d1.executescript((self.root / 'delta.sql').read_text())
        current = self.d1.execute("SELECT json_chunk FROM source_navigation_node_payload WHERE id='person' ORDER BY part").fetchall()
        self.assertEqual(json.loads(''.join(row[0] for row in current))['label'], 'Исправлено')
        self.d1.executescript((self.root / 'rollback.sql').read_text())
        self.assertEqual(self.d1.execute('SELECT * FROM source_navigation_node_payload ORDER BY id,part').fetchall(), before)


class PayloadCaptureBudgetTests(unittest.TestCase):
    def test_configured_limits_and_exact_boundary(self):
        with sqlite3.connect(':memory:') as db:
            db.execute('CREATE TABLE source_navigation_node_payload '
                       '(id TEXT, part INTEGER, json_chunk TEXT, PRIMARY KEY(id,part))')
            rows = [('selected', 0, '🙂'), ('selected', 1, 'ab')]
            db.executemany('INSERT INTO source_navigation_node_payload VALUES (?,?,?)', rows)
            sizes = [len(json.dumps(row, ensure_ascii=False, separators=(',', ':')).encode())
                     for row in rows]
            exact = dict(max_rows=2, max_row_bytes=max(sizes),
                         max_metadata_bytes=sum(sizes), max_read_bytes=sum(sizes))
            # Unrelated payloads never consume the selected identity's budget.
            db.execute("INSERT INTO source_navigation_node_payload VALUES ('other',0,'ignored')")
            capture = delta.Capture(delta.PreparedD1DeltaLimits(**exact))
            self.assertEqual(navigation._capture_payload(db, capture,
                'source_navigation_node_payload', 'selected'), [list(row) for row in rows])
            self.assertEqual(capture.read_bytes, sum(sizes))
            for field in exact:
                with self.subTest(budget=field), self.assertRaises(ValueError):
                    capture = delta.Capture(delta.PreparedD1DeltaLimits(
                        **{**exact, field: exact[field] - 1}))
                    navigation._capture_payload(db, capture, 'source_navigation_node_payload', 'selected')
            for part in (-1, 2):
                with self.subTest(part=part), self.assertRaisesRegex(ValueError, 'framing'):
                    db.execute('UPDATE source_navigation_node_payload SET part=? '
                               "WHERE id='selected' AND part=0", (part,))
                    try:
                        navigation._capture_payload(db, delta.Capture(delta.PreparedD1DeltaLimits()),
                            'source_navigation_node_payload', 'selected')
                    finally:
                        db.execute('UPDATE source_navigation_node_payload SET part=0 '
                                   "WHERE id='selected' AND part=?", (part,))


if __name__ == '__main__':
    unittest.main()

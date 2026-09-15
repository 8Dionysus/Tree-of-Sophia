"""Cold-reader boundary tests against the actual edge producer's emitted SQL."""
from __future__ import annotations

import copy
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from contextlib import closing
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "deploy/cloudflare-worker/scripts"))
import build_runtime as builder
from source_navigation_rows import chunk_text, project_rows
from test_access_contract import write_fixture
from tos_access.core import ToSAccessCore
from tos_access.knowledge import inspect_knowledge_node, inspect_knowledge_relation
from tos_access.published_read_metadata import published_source_navigation_digest_key
from tos_access.published_read_model import (
    CATALOG_KEY, TOP_KEY, PublishedKnowledgeReadModel, PublishedReadBudgetExceeded,
    PublishedReadLimits, PublishedReadModelError, PublishedSnapshotConflict,
    _Read, emitted_row_digest, published_row_digest_key, published_snapshot_binding,
)


def metadata(db, key):
    return json.loads("".join(row[0] for row in db.execute(
        "SELECT json_chunk FROM edge_meta WHERE key=? ORDER BY part", (key,))))


def source_subprocess_environment():
    source = str(Path(__file__).resolve().parents[1] / "src")
    pythonpath = os.pathsep.join(path for path in (source, os.environ.get("PYTHONPATH")) if path)
    return {**os.environ, "PYTHONPATH": pythonpath, "PYTHONDONTWRITEBYTECODE": "1"}


class PublishedReadModelTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.fixture.cleanup)
        cls.root = Path(cls.fixture.name)
        write_fixture(cls.root)
        core = ToSAccessCore.discover(cls.root)
        graph = copy.deepcopy(core.knowledge_graph())
        # Public synthetic values exercise preservation, not semantic admission.
        graph["nodes"][0]["transport_probe"] = {
            "false": False, "zero": 0, "unknown": None,
            "forms": [{"id": f"synthetic-form-{i}", "text": f"Форма {i}",
                       "source_ref": {"path": "ToS/synthetic.json", "pointer": f"/forms/{i}"}}
                      for i in range(44)],
        }
        template = graph["nodes"][0]
        aliases = [copy.deepcopy(template) for _ in range(2)]
        for i, item in enumerate(aliases):
            item.update(id=f"synthetic:alias-{i}", native_id="synthetic-shared-native",
                        entity_id="tos.synthetic.shared-entity")
        graph["nodes"].extend(aliases)
        graph["nodes"].sort(key=lambda item: item["id"])
        graph["relations"].sort(key=lambda item: item["id"])
        cls.graph = builder.normalize_paths(graph, cls.root)
        target = cls.root / "runtime/read-model.sql"
        with patch.object(builder, "REPO_ROOT", cls.root), patch.object(ToSAccessCore, "knowledge_graph", return_value=graph):
            cls.catalog = builder.normalize_paths(core.knowledge_catalog(), cls.root)
            builder.build_read_model_sql(core, target, "a" * 64)
        cls.seed = cls.root / "published.sqlite"
        with closing(sqlite3.connect(cls.seed)) as db:
            db.executescript(target.read_text())
            db.executescript((builder.WORKER_ROOT / "migrations/0001-exploration.sql").read_text())
            top = metadata(db, TOP_KEY)
            epoch = db.execute("SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1").fetchone()[0]
            cls.binding = published_snapshot_binding(top, epoch)

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.path = Path(self.directory.name) / "published.sqlite"
        shutil.copyfile(self.seed, self.path)
        self.reader = PublishedKnowledgeReadModel(self.path, self.binding)

    def mutate(self, sql, args=()):
        with closing(sqlite3.connect(self.path)) as db:
            db.execute(sql, args)
            db.commit()

    def test_native_navigation_matches_legacy_without_loading_stale_index(self):
        legacy = ToSAccessCore.discover(self.root)
        selected = ToSAccessCore.discover(self.root, published_read_model_path=self.path,
                                         published_read_model_expected=self.binding)
        cases = [('source_descend', 'tos.work.fixture'),
                 ('source_dossier', 'tos.work.fixture'),
                 ('source_dossier', 'tos.link.fixture.download')]
        expected = [getattr(legacy, method)(identifier) for method, identifier in cases]
        with patch.object(ToSAccessCore, 'index', side_effect=AssertionError('stale corpus index')):
            for (method, identifier), packet in zip(cases, expected):
                with self.subTest(method=method, identifier=identifier):
                    self.assertEqual(getattr(selected, method)(identifier), packet)

    def test_native_navigation_missing_product_and_mirror_drift_fail_closed(self):
        self.mutate("UPDATE source_navigation_nodes SET label='stale' WHERE node_id=?",
                    ('tos.work.fixture',))
        with self.assertRaises(PublishedReadModelError):
            self.reader.source_descend('tos.work.fixture', max_depth=8, limit=300)
        self.mutate("DELETE FROM edge_meta WHERE key='source_navigation_top'")
        with self.assertRaises(PublishedReadModelError):
            self.reader.source_dossier('tos.work.fixture', limit=300)

    def test_native_navigation_payload_framing_and_budget(self):
        with closing(sqlite3.connect(self.path)) as db:
            raw = db.execute('SELECT json FROM source_navigation_nodes WHERE node_id=?',
                             ('tos.work.fixture',)).fetchone()[0]
            expected = self.reader.source_descend('tos.work.fixture', max_depth=8, limit=300)
            db.execute("UPDATE source_navigation_nodes SET json='' WHERE node_id=?", ('tos.work.fixture',))
            db.executemany('INSERT INTO source_navigation_node_payload VALUES(?,?,?)',
                           [('tos.work.fixture', part, raw[start:start + 200])
                            for part, start in enumerate(range(0, len(raw), 200))])
            db.commit()
        self.assertEqual(self.reader.source_descend('tos.work.fixture', max_depth=8, limit=300), expected)
        limited = PublishedKnowledgeReadModel(self.path, self.binding,
                                              limits=PublishedReadLimits(max_rows=4))
        with self.assertRaises(PublishedReadBudgetExceeded):
            limited.source_descend('tos.work.fixture', max_depth=8, limit=300)
        self.mutate('DELETE FROM source_navigation_node_payload WHERE id=? AND part=0', ('tos.work.fixture',))
        with self.assertRaises(PublishedReadModelError):
            self.reader.source_descend('tos.work.fixture', max_depth=8, limit=300)

    def test_native_navigation_bad_hints_unknown_ids_and_unsupported_dossier_kind_fail_closed(self):
        with closing(sqlite3.connect(self.path)) as db:
            properties = db.execute(
                'SELECT properties_json FROM source_navigation_nodes WHERE node_id=?',
                ('tos.work.fixture',),
            ).fetchone()[0]
            db.execute(
                'UPDATE source_navigation_nodes SET properties_json=? WHERE node_id=?',
                ('{"access_status":"stale"}', 'tos.work.fixture'),
            )
            db.commit()
        with self.assertRaisesRegex(PublishedReadModelError, 'property hint'):
            self.reader.source_descend('tos.work.fixture', max_depth=8, limit=300)
        self.mutate(
            'UPDATE source_navigation_nodes SET properties_json=? WHERE node_id=?',
            (properties, 'tos.work.fixture'),
        )

        with closing(sqlite3.connect(self.path)) as db:
            source_refs = db.execute(
                'SELECT source_refs_json FROM source_navigation_edges WHERE edge_id=?',
                ('sn1a',),
            ).fetchone()[0]
            db.execute(
                'UPDATE source_navigation_edges SET source_refs_json=? WHERE edge_id=?',
                ('["stale-source-ref"]', 'sn1a'),
            )
            db.commit()
        with self.assertRaisesRegex(PublishedReadModelError, 'reference hint'):
            self.reader.source_descend('tos.work.fixture', max_depth=8, limit=300)
        self.mutate(
            'UPDATE source_navigation_edges SET source_refs_json=? WHERE edge_id=?',
            (source_refs, 'sn1a'),
        )

        with self.assertRaises(KeyError):
            self.reader.source_descend('tos.native-navigation.missing', max_depth=8, limit=300)
        with self.assertRaises(KeyError):
            self.reader.source_dossier('tos.native-navigation.missing', limit=300)
        with self.assertRaises(ValueError):
            self.reader.source_dossier('philosophy.eras.fixture', limit=300)

    def test_native_navigation_chunked_unknown_fields_preserve_exact_values(self):
        big_integer = 1234567890123456789012345678901234567890
        with closing(sqlite3.connect(self.path)) as db:
            raw = db.execute(
                'SELECT json FROM source_navigation_nodes WHERE node_id=?',
                ('tos.work.fixture',),
            ).fetchone()[0]
            value = json.loads(raw)
            value.update({
                'unknown_big_integer': big_integer,
                'unknown_false': False,
                'unknown_zero': 0,
                'unknown_null': None,
            })
            raw = builder.compact_json(value)
            # This fixture intentionally publishes a new complete row. Its
            # integrity companion must move with it; the drift tests below do
            # not update the companion when mutating persisted contents.
            db.execute('UPDATE edge_meta SET json_chunk=? WHERE key=? AND part=0',
                       (builder.compact_json(emitted_row_digest(raw)),
                        published_source_navigation_digest_key('nodes', 'tos.work.fixture')))
            db.execute(
                'UPDATE source_navigation_nodes SET json=? WHERE node_id=?',
                ('', 'tos.work.fixture'),
            )
            db.execute(
                'DELETE FROM source_navigation_node_payload WHERE id=?',
                ('tos.work.fixture',),
            )
            db.executemany(
                'INSERT INTO source_navigation_node_payload VALUES(?,?,?)',
                [
                    ('tos.work.fixture', part, chunk)
                    for part, chunk in enumerate(chunk_text(raw, size=80))
                ],
            )
            db.commit()

        packet = self.reader.source_descend('tos.work.fixture', max_depth=8, limit=300)
        node = next(item for item in packet['nodes'] if item['node_id'] == 'tos.work.fixture')
        self.assertEqual(node['unknown_big_integer'], big_integer)
        self.assertIs(node['unknown_false'], False)
        self.assertEqual(node['unknown_zero'], 0)
        self.assertIsNone(node['unknown_null'])

    def test_native_navigation_full_row_drift_is_not_hidden_by_unchanged_mirrors(self):
        cases = [('nodes', 'node_id', 'tos.work.fixture', {'research_note': 'altered'}),
                 ('edges', 'edge_id', 'sn1a', {'research_note': 'altered'}),
                 ('rights', 'rights_id', 'tos.rights.fixture',
                  {'assessment_status': 'licensed', 'redistribution_posture': 'authorized', 'review_status': 'accepted'})]
        for kind, key, identifier, delta in cases:
            with self.subTest(kind=kind):
                with closing(sqlite3.connect(self.path)) as db:
                    original = db.execute(f'SELECT json FROM source_navigation_{kind} WHERE {key}=?',
                                          (identifier,)).fetchone()[0]
                    changed = builder.compact_json({**json.loads(original), **delta})
                    db.execute(f'UPDATE source_navigation_{kind} SET json=? WHERE {key}=?', (changed, identifier))
                    db.commit()
                try:
                    with self.assertRaisesRegex(PublishedReadModelError, 'checksum differs'):
                        self.reader.source_dossier('tos.work.fixture', limit=300)
                finally:
                    self.mutate(f'UPDATE source_navigation_{kind} SET json=? WHERE {key}=?', (original, identifier))

    def test_native_navigation_missing_checksum_requires_explicit_product_migration(self):
        self.mutate('DELETE FROM edge_meta WHERE key=?',
                    (published_source_navigation_digest_key('rights', 'tos.rights.fixture'),))
        with self.assertRaises(PublishedReadModelError):
            self.reader.source_dossier('tos.work.fixture', limit=300)

    def test_native_navigation_stale_index_does_not_hide_new_environment_edge(self):
        node_id = 'tos.environment.native-fixture'
        edge_id = 'sn-native-environment'
        stale = ToSAccessCore.discover(self.root).source_navigation(bibliographic_only=True)
        self.assertNotIn(node_id, {node['node_id'] for node in stale['nodes']})
        self.assertNotIn(edge_id, {edge['edge_id'] for edge in stale['edges']})

        native_node = {
            'node_id': node_id,
            'node_kind': 'environment',
            'label': 'Native fixture environment',
            'source_ref': 'ToS/access/fixtures/native-environment.json',
            'identity_status': 'not_applicable',
            'properties': {},
        }
        native_edge = {
            'edge_id': edge_id,
            'from_id': node_id,
            'predicate_id': 'grounds',
            'to_id': 'tos.work.fixture',
            'edge_kind': 'authored_source_planting',
            'review_status': 'unreviewed',
            'source_refs': ['ToS/access/fixtures/native-environment-edge.json'],
        }
        with closing(sqlite3.connect(self.path)) as db:
            for kind, ordinal, item in (('nodes', 100, native_node), ('edges', 100, native_edge)):
                for table, rows in project_rows(kind, ordinal, item, self.root).items():
                    for row in rows:
                        db.execute(
                            f"INSERT INTO {table} VALUES ({','.join('?' for _ in row)})",
                            row,
                        )
            db.commit()

        packet = self.reader.source_descend(node_id, max_depth=1, limit=300)
        self.assertEqual(packet['counts'], {'nodes': 2, 'edges': 1})
        self.assertEqual([node['node_id'] for node in packet['nodes']],
                         [node_id, 'tos.work.fixture'])
        self.assertEqual([edge['edge_id'] for edge in packet['edges']], [edge_id])

    def test_full_packet_parity_and_restart_without_graph_or_catalog_read(self):
        original = type(self.reader)._connect
        statements = []
        def connect(reader):
            db = original(reader)
            db.set_trace_callback(statements.append)
            return db
        with patch.object(type(self.reader), "_connect", connect), patch(
                "tos_access.core.build_knowledge_graph", side_effect=AssertionError("hidden graph build")), patch(
                "tos_access.search_read_model.SQLiteKnowledgeSearchReadModel._snapshot_digest", side_effect=AssertionError("hidden graph digest")):
            self.assertEqual(self.reader.catalog(), self.catalog)
            statements.clear()
            for item in self.graph["nodes"]:
                self.assertEqual(self.reader.node(item["id"], 1), inspect_knowledge_node(self.graph, item["id"], 1))
            for item in self.graph["relations"]:
                self.assertEqual(self.reader.relation(item["id"]), inspect_knowledge_relation(self.graph, item["id"]))
            restarted = PublishedKnowledgeReadModel(self.path, self.binding)
            self.assertEqual(restarted.node("synthetic-shared-native"), inspect_knowledge_node(self.graph, "synthetic-shared-native"))
            self.assertEqual(restarted.node("tos.synthetic.shared-entity"), inspect_knowledge_node(self.graph, "tos.synthetic.shared-entity"))
        self.assertFalse(any(f"key='{CATALOG_KEY}'" in sql for sql in statements))
        self.assertFalse(any("knowledge_top" in sql for sql in statements))
        self.assertFalse(any(sql.startswith(("INSERT", "UPDATE", "CREATE", "DELETE")) for sql in statements))

    def test_overflow_carriers_preserve_exact_packets_and_fail_closed(self):
        expected = []
        with closing(sqlite3.connect(self.path)) as db:
            for kind in ('node', 'relation'):
                identifier, raw = db.execute(f'SELECT id,json FROM knowledge_{kind}s ORDER BY id LIMIT 1').fetchone()
                key = f'knowledge_{kind}_payload:{identifier}'
                expected.append((kind, identifier, key))
                db.execute(f"UPDATE knowledge_{kind}s SET json='' WHERE id=?", (identifier,))
                db.executemany('INSERT INTO edge_meta VALUES(?,?,?)',
                    [(key, part, raw[start:start + 400]) for part, start in enumerate(range(0, len(raw), 400))])
            db.commit()
        for kind, identifier, _ in expected:
            with self.subTest(kind=kind):
                actual = (self.reader.node(identifier, 1) if kind == 'node' else self.reader.relation(identifier))
                oracle = (inspect_knowledge_node(self.graph, identifier, 1) if kind == 'node'
                          else inspect_knowledge_relation(self.graph, identifier))
                self.assertEqual(actual, oracle)
        limited = PublishedKnowledgeReadModel(self.path, self.binding, limits=PublishedReadLimits(max_row_bytes=128))
        with self.assertRaises(PublishedReadBudgetExceeded):
            limited.node(expected[0][1], 1)
        # A missing chunk cannot be replaced by the compact seed or accepted
        # merely because a parseable prefix happened to survive.
        self.mutate('DELETE FROM edge_meta WHERE key=? AND part=1', (expected[0][2],))
        with self.assertRaises(PublishedReadModelError):
            self.reader.node(expected[0][1], 1)

    def test_prepared_inspection_projects_retained_raw_source_target(self):
        from tos_access.knowledge import _content_revision, _exact_record_digest, _stable_digest

        item_id = self.graph["nodes"][0]["id"]
        record = {
            "schema_version": "tos_corpus_record_v1", "record_type": "agent",
            "record_id": "tos.agent.prepared-source", "record_version": 1,
            "preferred_label": "Prepared source agent",
        }
        item = copy.deepcopy(self.graph["nodes"][0])
        payload = copy.deepcopy(item["source_record"]["payload"])
        payload["properties"] = {
            **(payload.get("properties") if isinstance(payload.get("properties"), dict) else {}),
            "source_record": record,
        }
        item["source_record"] = {
            **item["source_record"], "payload": payload, "digest": _stable_digest(payload),
        }
        item["content_revision"] = _content_revision(item)
        raw_json = builder.compact_json(item)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE knowledge_nodes SET json=? WHERE id=?", (raw_json, item_id))
            db.execute(
                "UPDATE edge_meta SET json_chunk=? WHERE key=? AND part=0",
                (builder.compact_json(emitted_row_digest(raw_json)), published_row_digest_key("node", item_id)),
            )
            db.commit()

        packet = self.reader.node(item_id, 0)
        targets = packet["source_read_targets"]
        self.assertEqual(set(targets), {item_id})
        target = targets[item_id]
        self.assertEqual(target["source_revision"], self.binding["source_revision"])
        self.assertEqual(target["target"]["layer"], "metadata_record")
        self.assertEqual(target["target"]["record_type"], "agent")
        self.assertEqual(target["target"]["record_ref"]["id"], record["record_id"])
        self.assertEqual(target["target"]["record_ref"]["version"], record["record_version"])
        self.assertEqual(
            target["target"]["record_ref"]["digest"],
            "sha256:" + _exact_record_digest(record),
        )

    def test_genuinely_cold_process_and_explicit_core_opt_in(self):
        program = """
import json, sys
from pathlib import Path
from unittest.mock import patch
from tos_access.core import ToSAccessCore
from tos_access.published_read_model import PublishedReadModelError
with patch('tos_access.core.build_knowledge_graph', side_effect=AssertionError('graph')), \\
     patch('tos_access.search_read_model.SQLiteKnowledgeSearchReadModel._snapshot_digest', side_effect=AssertionError('digest')), \\
     patch('tos_access.normalization_cache.NormalizationCache.__init__', side_effect=AssertionError('cache')):
    core = ToSAccessCore.discover(sys.argv[1], published_read_model_path=sys.argv[2], published_read_model_expected=json.loads(sys.argv[3]))
    assert core.knowledge_catalog()['schema'] == 'tos_knowledge_catalog_v1'
    assert core.knowledge_node(sys.argv[4])['matches']
    assert core.knowledge_relation(sys.argv[5])['endpoints']
    assert core.knowledge_exploration_contracts()['capabilities']['available'] is True
    assert core.knowledge_explore({'focus_node_id': sys.argv[4], 'max_depth': 0})['status'] == 'complete'
    try: core.knowledge_graph()
    except PublishedReadModelError: pass
    else: raise AssertionError('hidden fallback')
    with patch('tos_access.core._read_json', side_effect=AssertionError('source read')):
        detached = ToSAccessCore.discover(Path(sys.argv[2]).parent, published_read_model_path=sys.argv[2], published_read_model_expected=json.loads(sys.argv[3]))
        assert detached.knowledge_catalog()['schema'] == 'tos_knowledge_catalog_v1'
        assert detached.knowledge_node(sys.argv[4])['matches']
        assert detached.knowledge_relation(sys.argv[5])['endpoints']
"""
        outcome = subprocess.run([sys.executable, "-c", program, str(self.root), str(self.path),
                                  json.dumps(self.binding), self.graph["nodes"][0]["id"], self.graph["relations"][0]["id"]],
                                 capture_output=True, text=True, timeout=20, env=source_subprocess_environment())
        self.assertEqual(outcome.returncode, 0, outcome.stderr)
        with self.assertRaises(ValueError):
            ToSAccessCore.discover(self.root, published_read_model_path=self.path)
        with self.assertRaises(ValueError):
            ToSAccessCore.discover(self.root, published_read_model_expected=self.binding)
        self.assertEqual(ToSAccessCore.discover(self.root).knowledge_catalog(),
                         ToSAccessCore.discover(self.root).knowledge_catalog())

    def test_missing_corrupt_and_selected_row_byte_drift_refuse(self):
        with self.assertRaises(PublishedReadModelError):
            PublishedKnowledgeReadModel(self.path.with_name("missing"), self.binding).catalog()
        item_id = self.graph["nodes"][0]["id"]
        self.mutate("UPDATE knowledge_nodes SET json=json || ' ' WHERE id=?", (item_id,))
        with self.assertRaisesRegex(PublishedReadModelError, "checksum"):
            self.reader.node(item_id)
        self.mutate("DELETE FROM edge_meta WHERE key=?", (TOP_KEY,))
        with self.assertRaises(PublishedReadModelError):
            self.reader.catalog()

    def test_catalog_corruption_does_not_force_catalog_loading_on_inspect(self):
        self.mutate("UPDATE edge_meta SET json_chunk='{}' WHERE key=?", (CATALOG_KEY,))
        self.reader.node(self.graph["nodes"][0]["id"])
        with self.assertRaises(PublishedReadModelError):
            self.reader.catalog()

    def test_identity_mirror_missing_digest_and_unsupported_schema_refuse(self):
        item_id = self.graph["nodes"][0]["id"]
        self.mutate("UPDATE knowledge_nodes SET native_id='wrong-index' WHERE id=?", (item_id,))
        with self.assertRaisesRegex(PublishedReadModelError, "identity/index"):
            self.reader.node(item_id)
        self.mutate("DELETE FROM edge_meta WHERE key=?", (published_row_digest_key("node", item_id),))
        with self.assertRaises(PublishedReadModelError):
            self.reader.node(item_id)
        with closing(sqlite3.connect(self.path)) as db:
            top = metadata(db, TOP_KEY)
        top["read_model_schema"] = "tos_cloudflare_edge_read_model_v999"
        self.mutate("UPDATE edge_meta SET json_chunk=? WHERE key=? AND part=0",
                    (builder.compact_json(top), TOP_KEY))
        expected = published_snapshot_binding(top, self.binding["publication_epoch"])
        with self.assertRaisesRegex(PublishedReadModelError, "support.*schema"):
            PublishedKnowledgeReadModel(self.path, expected).catalog()

    def test_invalid_json_with_matching_emitted_digest_still_refuses(self):
        item_id = self.graph["nodes"][0]["id"]
        invalid_unicode = json.dumps({**self.graph["nodes"][0], "invalid": chr(0xD800)})
        for raw in ('{"duplicate":1,"duplicate":2}', '{"number":NaN}', invalid_unicode):
            self.mutate("UPDATE knowledge_nodes SET json=? WHERE id=?", (raw, item_id))
            self.mutate("UPDATE edge_meta SET json_chunk=? WHERE key=? AND part=0",
                        (builder.compact_json(emitted_row_digest(raw)), published_row_digest_key("node", item_id)))
            with self.assertRaises(PublishedReadModelError):
                self.reader.node(item_id)

    def test_clock_aba_and_owner_binding_mismatches_refuse(self):
        # A -> B -> A data bytes still advance the serving generation.
        self.mutate("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'", ('{"sha256":"' + "b" * 64 + '"}',))
        self.mutate("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'", ('{"sha256":"' + "a" * 64 + '"}',))
        with self.assertRaises(PublishedSnapshotConflict):
            self.reader.catalog()
        for field in ("source_revision", "data_revision", "metadata_sha256"):
            expected = {**self.binding, field: "f" * 64}
            with self.assertRaises(PublishedSnapshotConflict):
                PublishedKnowledgeReadModel(self.seed, expected).catalog()
        expected = copy.deepcopy(self.binding)
        expected["normalization_binding"]["processor_digest"] = "f" * 64
        with self.assertRaises(PublishedSnapshotConflict):
            PublishedKnowledgeReadModel(self.seed, expected).catalog()

    def test_concurrent_wal_publication_is_rejected_after_read_snapshot(self):
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("PRAGMA journal_mode=WAL")
        original = self.reader._snapshot
        calls = 0
        def observed(read):
            nonlocal calls
            top = original(read)
            calls += 1
            if calls == 1:
                self.mutate("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'")
            return top
        with patch.object(self.reader, "_snapshot", observed), self.assertRaises(PublishedSnapshotConflict):
            self.reader.catalog()

    def test_concurrent_identical_file_replacement_is_detected(self):
        replacement = self.path.with_name("next.sqlite")
        shutil.copyfile(self.seed, replacement)
        original = self.reader._snapshot
        calls = 0
        def observed(read):
            nonlocal calls
            top = original(read)
            calls += 1
            if calls == 1:
                os.replace(replacement, self.path)
            return top
        with patch.object(self.reader, "_snapshot", observed), self.assertRaises(PublishedSnapshotConflict):
            self.reader.catalog()

    def test_uncommitted_publication_and_rollback_leave_old_snapshot_readable(self):
        with closing(sqlite3.connect(self.path)) as writer:
            writer.execute("PRAGMA journal_mode=WAL")
            writer.execute("BEGIN")
            writer.execute("UPDATE edge_meta SET json_chunk='{}' WHERE key=?", (CATALOG_KEY,))
            writer.execute("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'")
            self.assertEqual(self.reader.catalog(), self.catalog)
            writer.rollback()
            self.assertEqual(self.reader.catalog(), self.catalog)

    def test_budgets_missing_seek_and_symlink_fail_closed(self):
        for limits in (PublishedReadLimits(max_rows=1), PublishedReadLimits(max_vm_steps=1),
                       PublishedReadLimits(max_response_bytes=50), PublishedReadLimits(max_matches=1)):
            reader = PublishedKnowledgeReadModel(self.path, self.binding, limits=limits)
            with self.assertRaises(PublishedReadBudgetExceeded):
                reader.node("synthetic-shared-native")
        link = self.path.with_name("link.sqlite")
        link.symlink_to(self.path)
        with self.assertRaises(PublishedReadModelError):
            PublishedKnowledgeReadModel(link, self.binding).catalog()
        self.mutate("DROP INDEX knowledge_relations_from_seek")
        with self.assertRaises(PublishedReadModelError):
            self.reader.node(self.graph["nodes"][0]["id"])

    def test_unknown_ids_and_limits_do_not_become_partial_success(self):
        with self.assertRaises(KeyError):
            self.reader.node("missing")
        with self.assertRaises(KeyError):
            self.reader.relation("missing")
        for value in (True, -1, 1001, "1"):
            with self.assertRaises(ValueError):
                self.reader.node("synthetic-shared-native", value)
        result = self.reader.node(self.graph["nodes"][0]["id"], 0)
        self.assertEqual(result, inspect_knowledge_node(self.graph, self.graph["nodes"][0]["id"], 0))

    def test_unrelated_population_doubling_does_not_expand_selected_reads(self):
        observations = []
        original = _Read.__init__
        def capture(read, *args, **kwargs):
            original(read, *args, **kwargs)
            observations.append(read)
        with closing(sqlite3.connect(self.path)) as db:
            for start in (0, 5000):
                db.executemany("INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?,?)",
                    ((f"unrelated:{i}", f"tos.unrelated.{i}", f"unrelated-{i}", "synthetic", "kind", "type", "", "", "", "{}")
                     for i in range(start, start + 5000)))
                db.commit()
                with patch.object(_Read, "__init__", capture):
                    packet = self.reader.node("synthetic-shared-native", 0)
                self.assertEqual(packet["counts"]["matches"], 2)
        self.assertEqual(observations[0].rows, observations[1].rows)
        self.assertEqual(observations[0].bytes, observations[1].bytes)
        self.assertLessEqual(observations[1].steps, observations[0].steps + 200)

    def test_high_degree_exact_count_refuses_instead_of_scanning_without_bound(self):
        with closing(sqlite3.connect(self.path)) as db:
            db.executemany("INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?,?)",
                ((f"synthetic:incident-{i}", f"incident-{i}", "synthetic", "synthetic:alias-0",
                  "synthetic:alias-1", "predicate", "relation-type", "", "", "", "{}") for i in range(10000)))
            db.commit()
        reader = PublishedKnowledgeReadModel(self.path, self.binding, limits=PublishedReadLimits(max_vm_steps=1000))
        with self.assertRaises(PublishedReadBudgetExceeded):
            reader.node("synthetic:alias-0", 0)

    def test_byte_budget_stops_cursor_before_materializing_wide_result_set(self):
        with closing(sqlite3.connect(self.path)) as db:
            db.executemany("INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?,?)",
                ((f"wide:{i}", f"tos.wide.{i}", f"wide-{i}", "synthetic", "kind", "type", "", "", "", "x" * 65536)
                 for i in range(50)))
            db.commit()
            db.row_factory = sqlite3.Row
            read = _Read(db, PublishedReadLimits(max_response_bytes=80000))
            with self.assertRaises(PublishedReadBudgetExceeded):
                read.query("SELECT json FROM knowledge_nodes WHERE id >= 'wide:' ORDER BY id LIMIT 50")
            self.assertEqual(read.rows, 2)


if __name__ == "__main__":
    unittest.main()

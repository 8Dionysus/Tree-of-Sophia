from __future__ import annotations

import json
import hashlib
import sqlite3
import sys
import tempfile
import unittest
import os
import shutil
from pathlib import Path
from contextlib import closing
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'deploy/cloudflare-worker/scripts'))
from incremental_runtime import (
    DiskRowBaseline,
    DiskRowIndex,
    DeltaRecorder,
    PRIMARY_KEYS,
    REGISTERED_KEYS,
    ROW_INDEX_MAX_KEY_CHARS,
    prepare_search_address_indexes_transaction,
    plan_search_addresses_transaction,
    MAX_SEARCH_ADDRESS,
    delete_staged_keys_sql,
)


class IncrementalRuntimeTests(unittest.TestCase):
    def test_posting_insert_shape_and_bytes_are_both_bounded_without_row_loss(self):
        import build_runtime as builder
        from incremental_runtime import INSERT, sql_value_rows, sql_value_literals
        rows = [("'nodes'", '3', builder.sql_text(str(i)), str(i)) for i in range(4901)]
        rows += [("'nodes'", '3', builder.sql_text('🌳' * 8000), '4901')]
        output = []
        builder.append_batched_inserts(output, 'knowledge_search_gram_stats_next',
                                      ('kind', 'n', 'gram', 'postings'), rows)
        actual = []
        for statement in output:
            self.assertLessEqual(len(statement.encode('utf-8')), builder.MAX_D1_SQL_STATEMENT_BYTES)
            values = sql_value_rows(INSERT.match(statement).group(3))
            self.assertLessEqual(len(values), builder.MAX_D1_SQL_INSERT_ROWS)
            actual.extend(tuple(sql_value_literals(value)) for value in values)
        self.assertEqual(actual, rows)

    def test_full_only_sql_preserves_publication_and_explicit_posting_budget(self):
        import build_runtime as builder
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_fixture(root)
            core = ToSAccessCore.discover(root)
            with patch.object(builder, 'REPO_ROOT', root):
                carriers = builder.ProducerCarrierSet.from_core(core)
                revision = builder.data_revision(core, carriers)
                legacy = root / 'legacy' / 'read-model.sql'
                full = root / 'full' / 'read-model.sql'
                expected = builder.build_read_model_sql(core, legacy, revision, carriers)
                actual = builder.build_read_model_sql(core, full, revision, carriers,
                    emit_delta_baseline=False,
                    max_search_postings=expected['knowledge_search_postings'])
                self.assertEqual(full.read_bytes(), legacy.read_bytes())
                saved = {path: path.read_bytes() for path in legacy.parent.iterdir() if path.is_file()}
                for budget in ({'max_lens_auxiliary_bytes': 1}, {'max_lens_memberships': 1}):
                    with self.subTest(budget=budget), self.assertRaisesRegex(RuntimeError, 'lens auxiliary production budget'):
                        builder.build_read_model_sql(core, legacy, revision, carriers, **budget)
                    for path, original in saved.items():
                        self.assertEqual(path.read_bytes(), original)
                self.assertEqual({k: v for k, v in actual.items() if k != 'delta'},
                                 {k: v for k, v in expected.items() if k != 'delta'})
                self.assertIsNone(actual['delta'])
                self.assertEqual({p.name for p in full.parent.iterdir()}, {'read-model.sql'})
                with self.assertRaisesRegex(ValueError, 'fresh output'):
                    builder.build_read_model_sql(core, full, revision, carriers, emit_delta_baseline=False)
                self.assertEqual(full.read_bytes(), legacy.read_bytes())
                refused = root / 'refused' / 'read-model.sql'
                with self.assertRaisesRegex(RuntimeError, 'posting budget exceeded'):
                    builder.build_read_model_sql(core, refused, revision, carriers,
                        emit_delta_baseline=False,
                        max_search_postings=expected['knowledge_search_postings'] - 1)
                self.assertFalse(refused.exists())
                self.assertFalse(refused.with_name('read-model.rows.json').exists())
                invalid = root / 'invalid' / 'read-model.sql'
                for budget in (0, -1, True, 1.5):
                    with self.subTest(budget=budget), self.assertRaises(ValueError):
                        builder.build_read_model_sql(core, invalid, revision, carriers,
                            max_search_postings=budget)
                self.assertFalse(invalid.parent.exists())
                conflict = root / 'conflict' / 'read-model.sql'
                conflict.parent.mkdir()
                conflict.with_name('read-model.deployed.rows.json').write_text('existing')
                with self.assertRaisesRegex(ValueError, 'fresh output'):
                    builder.build_read_model_sql(core, conflict, revision, carriers, emit_delta_baseline=False)
                self.assertFalse(conflict.exists())

    def test_auxiliary_baseline_requires_explicit_migration_and_valid_identity(self):
        import copy
        import build_runtime as builder
        import lens_auxiliary_runtime as auxiliary
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_fixture(root)
            core = ToSAccessCore.discover(root)
            target = root / 'runtime' / 'read-model.sql'
            with patch.object(builder, 'REPO_ROOT', root):
                carriers = builder.ProducerCarrierSet.from_core(core)
                revision = builder.data_revision(core, carriers)
                builder.build_read_model_sql(core, target, revision, carriers)
                index = target.with_name('read-model.rows.json')
                baseline = json.loads(index.read_text())
                top = auxiliary.baseline_publication_top(baseline)
                self.assertEqual(top['data_revision'], revision)
                old = copy.deepcopy(baseline)
                del old['auxiliary_publication']
                for table in auxiliary.STORES:
                    del old['rows'][table]
                index.write_text(json.dumps(old))
                result = builder.build_read_model_sql(core, target, revision, carriers)
                self.assertEqual(result['auxiliary_migration'], 'lens-auxiliary-initial-migration-required')
                self.assertFalse(result['delta']['available'])
                self.assertEqual(json.loads(index.read_text()), baseline)
                with closing(sqlite3.connect(':memory:')) as db:
                    db.executescript(target.read_text())
                    first_epoch = db.execute('SELECT epoch FROM knowledge_exploration_clock').fetchone()[0]
                    db.executescript(target.read_text())
                    epoch = db.execute('SELECT epoch FROM knowledge_exploration_clock').fetchone()[0]
                    self.assertGreater(epoch, first_epoch)
                    for table, (state, schema) in auxiliary.STORES.items():
                        self.assertEqual(db.execute(f'SELECT schema,binding,valid FROM {state}').fetchall(),
                            [(schema, auxiliary._compact(auxiliary.published_snapshot_binding(top, epoch)), 1)])
                for field, invalid in (('schema', 'unknown'), ('stores', {}),
                                       ('reader_top', {**top, 'data_revision': 'f' * 64})):
                    broken = copy.deepcopy(baseline)
                    broken['auxiliary_publication'][field] = invalid
                    index.write_text(json.dumps(broken))
                    saved = {path: path.read_bytes() for path in (target, index, target.with_name('read-model.delta.sql'))}
                    with self.subTest(field=field), self.assertRaises(ValueError):
                        builder.build_read_model_sql(core, target, revision, carriers)
                    for path, raw in saved.items():
                        self.assertEqual(path.read_bytes(), raw)

    def test_data_revision_binds_catalog_even_when_graph_is_identical(self):
        import copy
        import build_runtime as builder
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_fixture(root)
            core = ToSAccessCore.discover(root)
            graph = core.knowledge_graph()
            catalog = core.knowledge_catalog()
            changed_catalog = copy.deepcopy(catalog)
            changed_catalog["capabilities"]["catalog_revision_probe"] = "changed"
            changed_graph = copy.deepcopy(graph)
            changed_graph["normalization_binding"]["processor_digest"] = "f" * 64
            with patch.object(builder, "REPO_ROOT", root), patch.object(
                ToSAccessCore,
                "knowledge_snapshot",
                return_value={"graph": graph, "catalog": catalog},
            ):
                baseline = builder.data_revision(core)
            with patch.object(builder, "REPO_ROOT", root), patch.object(
                ToSAccessCore,
                "knowledge_snapshot",
                return_value={"graph": graph, "catalog": changed_catalog},
            ):
                changed = builder.data_revision(core)
            with patch.object(builder, "REPO_ROOT", root), patch.object(
                ToSAccessCore,
                "knowledge_snapshot",
                return_value={"graph": changed_graph, "catalog": catalog},
            ):
                metadata_changed = builder.data_revision(core)
            self.assertEqual(graph, core.knowledge_graph())
            self.assertNotEqual(baseline, changed)
            self.assertNotEqual(baseline, metadata_changed)

    def test_explicit_producer_carrier_set_preserves_legacy_parity_and_bindings(self):
        import build_runtime as builder
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_fixture(root)
            with patch.object(builder, "REPO_ROOT", root):
                core = ToSAccessCore.discover(root)
                legacy_revision = builder.data_revision(core)
                captured = builder.ProducerCarrierSet.from_core(core)
                self.assertEqual(legacy_revision, builder.data_revision(core, captured))

                legacy_sql = root / "legacy" / "read-model.sql"
                captured_sql = root / "captured" / "read-model.sql"
                builder.build_read_model_sql(core, legacy_sql, legacy_revision)
                builder.build_read_model_sql(core, captured_sql, legacy_revision, captured)
                self.assertEqual(legacy_sql.read_bytes(), captured_sql.read_bytes())

                # Physical scratch locations do not participate in the
                # revision; their explicit logical labels and bytes do.
                scratch = root / "external-scratch"
                external_paths = []
                for index, (label, path) in enumerate(captured.carrier_paths):
                    destination = scratch / f"carrier-{index}.json"
                    if path.is_file():
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(path, destination)
                    external_paths.append((label, destination))
                external = builder.ProducerCarrierSet.admit(
                    corpus=captured.corpus,
                    philosophy=captured.philosophy,
                    knowledge=captured.knowledge,
                    knowledge_catalog=captured.knowledge_catalog,
                    evidence=captured.evidence,
                    philosophy_audit=captured.philosophy_audit,
                    word_analysis_capability=captured.word_analysis_capability,
                    carrier_paths=external_paths,
                    logical_bindings={"source_revision": captured.source_revision},
                )
                relocated_paths = []
                for index, (_label, path) in enumerate(external_paths):
                    relocated_path = root / "external-scratch-relocated" / f"carrier-{index}.json"
                    if path.is_file():
                        relocated_path.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copyfile(path, relocated_path)
                    relocated_paths.append((external_paths[index][0], relocated_path))
                relocated = builder.ProducerCarrierSet.admit(
                    corpus=captured.corpus,
                    philosophy=captured.philosophy,
                    knowledge=captured.knowledge,
                    knowledge_catalog=captured.knowledge_catalog,
                    evidence=captured.evidence,
                    philosophy_audit=captured.philosophy_audit,
                    word_analysis_capability=captured.word_analysis_capability,
                    carrier_paths=relocated_paths,
                    logical_bindings={"source_revision": captured.source_revision},
                )
                self.assertNotEqual(legacy_revision, builder.data_revision(core, external))
                self.assertEqual(builder.data_revision(core, external), builder.data_revision(core, relocated))

                bound = builder.ProducerCarrierSet.from_core(
                    core,
                    logical_bindings={
                        "source_revision": captured.source_revision,
                        "source-vector-root": "root-a",
                    },
                )
                changed_binding = builder.ProducerCarrierSet.from_core(
                    core,
                    logical_bindings={
                        "source_revision": captured.source_revision,
                        "source-vector-root": "root-b",
                    },
                )
                self.assertNotEqual(
                    builder.data_revision(core, bound),
                    builder.data_revision(core, changed_binding),
                )

                mismatch = captured.knowledge_catalog["source_revision"]
                target = root / "mismatch" / "read-model.sql"
                captured.knowledge_catalog["source_revision"] = "b" * 64
                try:
                    with self.assertRaisesRegex(ValueError, "source revisions do not match"):
                        builder.build_read_model_sql(core, target, legacy_revision, captured)
                    self.assertFalse(target.exists())
                finally:
                    captured.knowledge_catalog["source_revision"] = mismatch

    def test_cache_budget_cli_and_real_builder_admission(self):
        import build_runtime as builder
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore
        args = builder.parse_args(['--cache-max-mib', '2', '--cache-max-entries', '3', '--cache-keep-runs', '1'])
        self.assertEqual((args.cache_max_mib, args.cache_max_entries, args.cache_keep_runs), (2, 3, 1))
        for option in ('--cache-max-mib', '--cache-max-entries', '--cache-keep-runs'):
            with patch('sys.stderr'), self.assertRaises(SystemExit):
                builder.parse_args([option, '0'])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); write_fixture(root)
            (root/'access/web/dist/index.html').write_text('<script src="/static/assets/tos-graph.js"></script>')
            core = ToSAccessCore.discover(root)
            projection = json.loads(core.philosophy_graph_projection_path.read_text())
            projection['review_packets'].append({'view_id':'direct-only', 'unresolved_diagnostics':[]})
            core.philosophy_graph_projection_path.write_text(json.dumps(projection))
            with patch.object(builder, 'REPO_ROOT', root):
                manifest = builder.build(core, root/'dist', root/'runtime',
                    cache_options={'max_cache_bytes':1024, 'max_cache_entries':3, 'keep_runs':1})
            cache = manifest['processing']['cache']
            self.assertEqual(cache['max_payload_bytes'], 1024)
            self.assertEqual(cache['max_entries'], 3)
            self.assertLessEqual(cache['payload_bytes'], 1024)
            self.assertLessEqual(cache['entries'], 3)

    def test_cli_targets_cannot_replace_sources_or_overlap_build_cache(self):
        import build_runtime as builder
        output, runtime = builder.WORKER_ROOT/'dist', builder.WORKER_ROOT/'runtime'
        builder.validate_output_paths(output, runtime)
        for bad_output, bad_runtime in (
            (Path('/'),runtime), (Path.home(),runtime), (builder.REPO_ROOT/'ToS',runtime),
            (output,output), (output,output/'cache'), (runtime/'dist',runtime),
            (output,builder.REPO_ROOT/'ToS/canon'),
        ):
            with self.assertRaisesRegex(RuntimeError, 'unsafe'):
                builder.validate_output_paths(bad_output,bad_runtime)

    def test_real_fixture_build_noop_and_preserved_mtime_change(self):
        import build_runtime as builder
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); write_fixture(root)
            (root/'access/web/dist/index.html').write_text('<script src="/static/assets/tos-graph.js"></script>')
            core=ToSAccessCore.discover(root); runtime=root/'runtime'; output=root/'dist'
            projection=json.loads(core.philosophy_graph_projection_path.read_text())
            projection['review_packets'].append({'view_id':'direct-only','unresolved_diagnostics':[]})
            core.philosophy_graph_projection_path.write_text(json.dumps(projection))
            with patch.object(builder,'REPO_ROOT',root):
                first=builder.build(core,output,runtime)
                with patch.object(ToSAccessCore,'knowledge_graph',side_effect=AssertionError('no-op rebuilt graph')):
                    second=builder.build(ToSAccessCore.discover(root),output,runtime)
                self.assertEqual(second['build_stages'],{'read-model':'reused','static-responses':'reused'})
                self.assertEqual(first['data_revision'],second['data_revision'])
                self.assertEqual(second['processing']['status'],'not-run')
                self.assertEqual(second['normalization_cache']['computed_steps'],0)
                stamp=core.index_path.stat()
                content=core.index_path.read_text()
                self.assertIn('Alpha',content)
                core.index_path.write_text(content.replace('Alpha','Omega'))
                os.utime(core.index_path,ns=(stamp.st_atime_ns,stamp.st_mtime_ns))
                changed=builder.build(core,output,runtime)
                self.assertNotEqual(changed['data_revision'],first['data_revision'])
                with closing(sqlite3.connect(':memory:')) as database:
                    database.executescript((runtime/'read-model.sql').read_text())
                    node=json.loads(database.execute("SELECT json FROM knowledge_nodes WHERE id='canon:a'").fetchone()[0])
                    self.assertEqual(node['display']['title']['default'],'Omega')
                before=builder.build_inputs(core)
                schema=root/'ToS/contracts/semantic-entity-type-registry.schema.json'
                schema.write_text(schema.read_text()+'\n')
                self.assertNotEqual(builder.build_inputs(core),before)

    def test_real_builder_search_delta_add_edit_delete_replay_and_stale_guard(self):
        import copy
        import build_runtime as builder
        import lens_auxiliary_runtime as auxiliary
        from tos_access.published_read_metadata import _compact, published_snapshot_binding
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore

        def serving_snapshot(database):
            tables = (
                'edge_meta',
                'knowledge_search_documents',
                'knowledge_search_grams',
                'knowledge_search_gram_stats',
                'knowledge_lens_order',
                *auxiliary.STORES,
            )
            snapshot = {}
            for table in tables:
                order = ','.join(REGISTERED_KEYS[table])
                snapshot[table] = database.execute(
                    f'SELECT * FROM {table} ORDER BY {order}'
                ).fetchall()
            top = edge_meta_packet(database, 'knowledge_reader_top')
            epoch = database.execute('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1').fetchone()[0]
            for table, (state, schema) in auxiliary.STORES.items():
                self.assertEqual(database.execute(f'SELECT schema,binding,valid FROM {state}').fetchall(),
                                 [(schema, _compact(published_snapshot_binding(top, epoch)), 1)])
                projected = []
                for kind in ('node', 'relation'):
                    for identifier, raw in database.execute(f'SELECT id,json FROM knowledge_{kind}s'):
                        projected.extend(auxiliary.projected_rows(table, kind, identifier, raw))
                self.assertEqual(sorted(snapshot[table]), sorted(projected))
            return snapshot

        def edge_meta_packet(database, key):
            chunks = database.execute(
                "SELECT json_chunk FROM edge_meta WHERE key=? ORDER BY part", (key,)
            ).fetchall()
            self.assertTrue(chunks, f"missing edge metadata: {key}")
            return json.loads("".join(row[0] for row in chunks))

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_fixture(root)
            runtime = root / 'runtime'
            runtime.mkdir()
            with patch.object(builder, 'REPO_ROOT', root):
                source_core = ToSAccessCore.discover(root)
                source_graph = source_core.knowledge_graph()
                # Keep the fixture build bounded while still using the actual
                # producer, including its batched posting/stat rows.
                graph_v1 = copy.deepcopy(source_graph)
                graph_v1['source_revision'] = 'a' * 64
                graph_v1['nodes'] = copy.deepcopy(source_graph['nodes'][:3])
                graph_v1['relations'] = copy.deepcopy(source_graph['relations'][:1])
                graph_v2 = copy.deepcopy(graph_v1)
                graph_v2['source_revision'] = 'b' * 64
                graph_v2['nodes'][0]['display']['title']['default'] = 'Edited fixture node'
                deleted_id = graph_v2['nodes'][1]['id']
                graph_v2['nodes'] = [graph_v2['nodes'][0], graph_v2['nodes'][2]]
                added = copy.deepcopy(graph_v1['nodes'][1])
                added['id'] = 'fixture:added-node'
                added['native_id'] = 'fixture:added-node'
                added['display']['title']['default'] = 'Added fixture node'
                graph_v2['nodes'].append(added)
                graph_v2['relations'][0]['to_id'] = added['id']

                first_sql = runtime / 'read-model.v1.sql'
                with patch.object(ToSAccessCore, 'knowledge_graph', return_value=graph_v1):
                    revision_v1 = builder.data_revision(source_core)
                    builder.build_read_model_sql(source_core, first_sql, revision_v1)
                shutil.copy2(runtime / 'read-model.rows.json', runtime / 'read-model.deployed.rows.json')

                first_database = root / 'first.sqlite'
                with closing(sqlite3.connect(first_database)) as database:
                    database.executescript(first_sql.read_text(encoding='utf-8'))
                    before = serving_snapshot(database)
                    before_ids = {row[2] for row in before['knowledge_search_documents']}
                    reader_top = edge_meta_packet(database, "knowledge_reader_top")
                    self.assertEqual(reader_top["schema"], "tos_published_knowledge_reader_v2")
                    self.assertEqual(reader_top["read_model_schema"], builder.READ_MODEL_SCHEMA_VERSION)
                    self.assertEqual(reader_top["data_revision"], revision_v1)
                    self.assertEqual(
                        edge_meta_packet(database, "knowledge_catalog")["schema"],
                        "tos_knowledge_catalog_v1",
                    )
                    node_json = database.execute(
                        "SELECT id,json FROM knowledge_nodes ORDER BY id LIMIT 1"
                    ).fetchone()
                    node_digest = edge_meta_packet(
                        database, f"knowledge_node_digest:{node_json[0]}"
                    )
                    self.assertEqual(
                        node_digest["sha256"],
                        hashlib.sha256(node_json[1].encode("utf-8")).hexdigest(),
                    )
                    deleted_json = database.execute(
                        "SELECT id,json FROM knowledge_nodes WHERE id=?", (deleted_id,)
                    ).fetchone()
                    self.assertIsNotNone(
                        edge_meta_packet(database, f"knowledge_node_digest:{deleted_json[0]}")
                    )
                    initial_catalog_sha = reader_top["catalog_sha256"]

                second_sql = runtime / 'read-model.v2.sql'
                with patch.object(ToSAccessCore, 'knowledge_graph', return_value=graph_v2):
                    revision_v2 = builder.data_revision(source_core)
                    result = builder.build_read_model_sql(source_core, second_sql, revision_v2)
                self.assertFalse((runtime / 'read-model.baseline.sqlite').exists())
                delta_sql = runtime / 'read-model.delta.sql'
                delta_text = delta_sql.read_text(encoding='utf-8')
                self.assertIn('knowledge_search_documents', delta_text)
                self.assertIn('knowledge_search_grams', delta_text)
                self.assertIn('knowledge_search_gram_stats', delta_text)
                self.assertNotIn('INSERT INTO knowledge_search_gram_stats(kind,n,gram,postings) SELECT', delta_text)
                self.assertGreater(result['delta']['changed_rows'], 0)
                self.assertGreater(result['delta']['removed_rows'], 0)

                delta_database = root / 'delta.sqlite'
                with closing(sqlite3.connect(delta_database)) as database:
                    database.executescript(first_sql.read_text(encoding='utf-8'))
                    stale_baseline = serving_snapshot(database)
                    database.executescript(delta_text)
                    after = serving_snapshot(database)
                    self.assertEqual(
                        edge_meta_packet(database, "knowledge_reader_top")["data_revision"],
                        revision_v2,
                    )
                    updated_top = edge_meta_packet(database, "knowledge_reader_top")
                    self.assertNotEqual(updated_top["catalog_sha256"], initial_catalog_sha)
                    self.assertIsNone(
                        database.execute(
                            "SELECT 1 FROM edge_meta WHERE key=? LIMIT 1",
                            (f"knowledge_node_digest:{deleted_id}",),
                        ).fetchone()
                    )
                    edited_json = database.execute(
                        "SELECT json FROM knowledge_nodes WHERE id=?", (graph_v2["nodes"][0]["id"],)
                    ).fetchone()[0]
                    self.assertNotEqual(
                        hashlib.sha256(edited_json.encode("utf-8")).hexdigest(),
                        node_digest["sha256"],
                    )
                    self.assertEqual(
                        edge_meta_packet(
                            database, f"knowledge_node_digest:{graph_v2['nodes'][0]['id']}"
                        )["sha256"],
                        hashlib.sha256(edited_json.encode("utf-8")).hexdigest(),
                    )
                    self.assertNotEqual(stale_baseline, after)
                    self.assertIn('fixture:added-node', {row[2] for row in after['knowledge_search_documents']})
                    self.assertNotIn(deleted_id, {row[2] for row in after['knowledge_search_documents']})
                    order_rows = {row[1]: row for row in after['knowledge_lens_order'] if row[0] == 'relation'}
                    self.assertEqual(order_rows[graph_v2['relations'][0]['id']][4], added['id'])
                    replayed = serving_snapshot(database)
                    database.executescript(delta_text)
                    self.assertEqual(replayed, serving_snapshot(database))

                full_database = root / 'full.sqlite'
                with closing(sqlite3.connect(full_database)) as database:
                    database.executescript(second_sql.read_text(encoding='utf-8'))
                    expected = serving_snapshot(database)
                self.assertEqual(after, expected)
                self.assertNotEqual(before, after)
                self.assertNotEqual(before_ids, {row[2] for row in after['knowledge_search_documents']})

                stale_database = root / 'stale.sqlite'
                with closing(sqlite3.connect(stale_database)) as database:
                    database.executescript(first_sql.read_text(encoding='utf-8'))
                    database.execute(
                        "UPDATE edge_meta SET json_chunk=? WHERE key='data_revision' AND part=0",
                        (json.dumps({'sha256': 'f' * 64}),),
                    )
                    database.commit()
                    with self.assertRaisesRegex(sqlite3.IntegrityError, 'stale delta baseline'):
                        database.executescript(delta_text)
                    self.assertEqual(
                        database.execute("SELECT json_chunk FROM edge_meta WHERE key='data_revision' AND part=0").fetchone()[0],
                        json.dumps({'sha256': 'f' * 64}),
                    )

    def test_build_stage_restart_integrity_inputs_and_lock(self):
        from build_stages import BuildStages, build_lock, fingerprint, tree_paths
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, output, receipt = root/'source', root/'output', root/'stages.json'
            source.mkdir(); (source/'one').write_text('first')
            calls = []
            def produce():
                calls.append(1); output.write_text('generated'); return {'ok': True}
            def run():
                stages = BuildStages(receipt)
                result = stages.run('stage', lambda: fingerprint(tree_paths(source, 'input')),
                                    lambda: {'file':output}, produce)
                stages.verify()
                return stages.report
            self.assertEqual(run(), {'stage':'computed'})
            self.assertEqual(run(), {'stage':'reused'})  # A fresh object/process can reuse it.
            stamp = (source/'one').stat()
            (source/'one').write_text('other')  # Same size and restored mtime must still invalidate.
            os.utime(source/'one', ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
            self.assertEqual(run(), {'stage':'computed'})
            output.write_text('corrupted')
            self.assertEqual(run(), {'stage':'computed'})
            packet = json.loads(receipt.read_text()); packet['stages']['stage']['result'] = {'ok':False}
            receipt.write_text(json.dumps(packet))
            self.assertEqual(run(), {'stage':'computed'})
            (source/'two').write_text('new member')
            self.assertEqual(run(), {'stage':'computed'})
            (source/'two').unlink()
            self.assertEqual(run(), {'stage':'computed'})
            output.unlink()
            self.assertEqual(run(), {'stage':'computed'})
            with build_lock(root/'runtime'):
                with self.assertRaisesRegex(RuntimeError, 'another edge build'):
                    with build_lock(root/'runtime'): pass
            with build_lock(root/'runtime'): pass

    def test_changed_inputs_or_failed_stage_never_get_a_success_checkpoint(self):
        from build_stages import BuildStages, fingerprint
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); source=root/'source'; output=root/'output'; receipt=root/'cache.json'
            source.write_text('initial')
            def mutate():
                output.write_text('partial'); source.write_text('changed'); return {}
            stage=BuildStages(receipt)
            with self.assertRaisesRegex(RuntimeError, 'inputs changed'):
                stage.run('one',lambda: fingerprint({'source':source}),lambda: {'out':output},mutate)
            self.assertEqual(BuildStages(receipt).entries,{})
            def good(): output.write_text('complete'); return {}
            stage.run('one',lambda: fingerprint({'source':source}),lambda: {'out':output},good)
            source.write_text('changed again')
            with self.assertRaisesRegex(RuntimeError, 'before completion'): stage.verify()

    def test_builder_reuses_independent_stages_and_recovers_failed_static_build(self):
        import build_runtime as builder
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); runtime=root/'runtime'; output=root/'dist'
            names=('index_path','philosophy_graph_projection_path','bibliographic_graph_path',
                   'entity_type_registry_path','relation_type_registry_path','evidence_projection_path',
                   'philosophy_post_planting_audit_path')
            paths={name:root/(name+'.json') for name in names}
            for path in paths.values(): path.write_text('{}')
            core=SimpleNamespace(tos_root=root,knowledge_graph=lambda:{},**paths)
            web=root/'access/web/dist'; web.mkdir(parents=True); (web/'index.html').write_text('web')
            calls=[]
            def sql(core,target,revision):
                calls.append('sql')
                for name in ('read-model.sql','read-model.rows.json','read-model.delta.sql'):
                    (target.parent/name).write_text(revision)
                return {'sql_statements':1}
            def static(core,target):
                calls.append('static'); target.mkdir(exist_ok=True)
                (target/'index.html').write_text((web/'index.html').read_text())
                return {'corpus':{'graph_views':['one']},'philosophy':{'views':['two']}}
            with patch.object(builder,'REPO_ROOT',root), patch.object(builder,'build_read_model_sql',side_effect=sql), \
                    patch.object(builder,'build_static_assets',side_effect=static), patch.object(builder,'data_revision',return_value='a'*64):
                first=builder.build(core,output,runtime)
                self.assertEqual(first['build_stages'],{'read-model':'computed','static-responses':'computed'})
                second=builder.build(core,output,runtime)
                self.assertEqual(second['build_stages'],{'read-model':'reused','static-responses':'reused'})
                self.assertEqual(calls,['sql','static'])
                (web/'index.html').write_text('new UI')
                ui=builder.build(core,output,runtime)
                self.assertEqual(ui['build_stages'],{'read-model':'reused','static-responses':'computed'})
                (runtime/'read-model.sql').write_text('corrupt')
                repaired=builder.build(core,output,runtime)
                self.assertEqual(repaired['build_stages'],{'read-model':'computed','static-responses':'reused'})
                paths['index_path'].write_text('{"changed":true}')
                with patch.object(builder,'build_static_assets',side_effect=RuntimeError('static failure')):
                    with self.assertRaisesRegex(RuntimeError, 'static failure'): builder.build(core,output,runtime)
                self.assertFalse((runtime/'manifest.json').exists())
                self.assertFalse((output/'__edge/build-manifest.json').exists())
                resumed=builder.build(core,output,runtime)
                self.assertEqual(resumed['build_stages'],{'read-model':'reused','static-responses':'computed'})
                (runtime/'read-model.deployed.rows.json').write_text('{}')
                deployed=builder.build(core,output,runtime)
                self.assertEqual(deployed['build_stages'],{'read-model':'computed','static-responses':'reused'})

    def test_local_bootstrap_is_revision_guarded_and_rolls_back_incomplete_input(self):
        from import_local_sqlite import import_sql, revision
        with tempfile.TemporaryDirectory() as root, closing(self.database()) as source:
            database = Path(root) / 'local.sqlite'
            sql = Path(root) / 'input.sql'
            with closing(sqlite3.connect(database)) as disk:
                source.backup(disk)
            sql.write_text("UPDATE knowledge_nodes SET value='changed' WHERE id='one';\n")
            with self.assertRaisesRegex(RuntimeError, 'target revision'):
                import_sql(database, sql, 'a'*64, 'b'*64)
            with closing(sqlite3.connect(database)) as disk:
                self.assertEqual(disk.execute("SELECT value FROM knowledge_nodes WHERE id='one'").fetchone()[0], 'old')
            sql.write_text(sql.read_text() + "UPDATE edge_meta SET json_chunk='" + json.dumps({'sha256': 'b'*64}) + "' WHERE key='data_revision';\n")
            self.assertEqual(import_sql(database, sql, 'a'*64, 'b'*64), 2)
            with self.assertRaisesRegex(RuntimeError, 'baseline'):
                import_sql(database, sql, 'a'*64, 'b'*64)
            with closing(sqlite3.connect(database)) as disk:
                self.assertEqual(revision(disk), 'b'*64)

    def test_local_bootstrap_preserves_multiline_producer_literals_and_triggers(self):
        from build_runtime import SqlStatementWriter, sql_text
        from import_local_sqlite import import_sql, revision
        from sql_stream import write_sql_chunk
        value = "София's λόγος 🌳\r\n\n  \rnext\n'); COMMIT; --\n\v\f\x85\u2028\u2029last"
        event = 'second;\r\n -- literal, not a SQL comment\n event'
        with tempfile.TemporaryDirectory() as root, closing(self.database()) as source:
            database, sql = Path(root) / 'local.sqlite', Path(root) / 'input.sql'
            with closing(sqlite3.connect(database)) as disk:
                source.backup(disk)
            writer = SqlStatementWriter(sql)
            writer.append('CREATE TABLE events (value TEXT);')
            writer.append('CREATE TRIGGER record_value AFTER UPDATE ON knowledge_nodes\n'
                          'BEGIN\nINSERT INTO events VALUES (NEW.value);\n'
                          f'INSERT INTO events VALUES ({sql_text(event)});\nEND;')
            writer.append(f"UPDATE knowledge_nodes SET value={sql_text(value)} WHERE id='one';")
            writer.append('UPDATE edge_meta SET json_chunk=' + sql_text(json.dumps({'sha256': 'b'*64}))
                          + " WHERE key='data_revision';")
            writer.finish()
            with closing(self.database()) as chunked:
                offset, copied = 0, []
                while True:
                    part = Path(root) / 'chunk.sql'
                    chunk = write_sql_chunk(sql, part, offset, 40)
                    content = part.read_bytes()
                    copied.append(content)
                    # Exercise each independent upload file, not only concat
                    # parity: no trigger or quoted literal may be split.
                    chunked.executescript(content.decode('utf-8'))
                    part.unlink()
                    if chunk['eof']:
                        break
                    self.assertGreater(chunk['next_offset'], offset)
                    offset = chunk['next_offset']
                self.assertEqual(b''.join(copied), sql.read_bytes())
                self.assertEqual(chunked.execute('SELECT value FROM events').fetchall(), [(value,), (event,)])
                self.assertEqual(revision(chunked), 'b'*64)
            self.assertEqual(import_sql(database, sql, 'a'*64, 'b'*64), writer.count)
            with closing(sqlite3.connect(database)) as disk:
                self.assertEqual(disk.execute("SELECT value FROM knowledge_nodes WHERE id='one'").fetchone()[0], value)
                self.assertEqual(disk.execute('SELECT value FROM events').fetchall(), [(value,), (event,)])
                self.assertEqual(revision(disk), 'b'*64)

    def test_local_bootstrap_rejects_invalid_input_and_rolls_back_rows_and_ddl(self):
        from import_local_sqlite import import_sql, revision
        prefix = ("UPDATE knowledge_nodes SET value='changed' WHERE id='one';\n"
                  'CREATE TABLE uncommitted (value TEXT);\n')
        for suffix, error in (
            ("INSERT INTO uncommitted VALUES ('unterminated\n\n", ValueError),
            ("INSERT INTO uncommitted VALUES ('no semicolon')", ValueError),
            ("INSERT INTO absent VALUES ('bad');\n", sqlite3.OperationalError),
            ("SELECT 1; SELECT 2;\n", sqlite3.ProgrammingError),
            ("INSERT INTO uncommitted VALUES ('" + '🌳' * 25_000 + "');\n", ValueError),
            ("INSERT INTO uncommitted VALUES ('" + 'x' * 100_001, ValueError),
        ):
            with self.subTest(suffix=suffix[:50]), tempfile.TemporaryDirectory() as root, closing(self.database()) as source:
                database, sql = Path(root) / 'local.sqlite', Path(root) / 'input.sql'
                with closing(sqlite3.connect(database)) as disk:
                    source.backup(disk)
                sql.write_text(prefix + suffix, encoding='utf-8', newline='')
                with self.assertRaises(error):
                    import_sql(database, sql, 'a'*64, 'b'*64)
                with closing(sqlite3.connect(database)) as disk:
                    self.assertEqual(disk.execute("SELECT value FROM knowledge_nodes WHERE id='one'").fetchone()[0], 'old')
                    self.assertIsNone(disk.execute("SELECT 1 FROM sqlite_master WHERE name='uncommitted'").fetchone())
                    self.assertEqual(revision(disk), 'a'*64)

    def test_local_bootstrap_accepts_exact_producer_byte_limit_and_no_final_newline(self):
        from build_runtime import MAX_D1_SQL_STATEMENT_BYTES, SqlStatementWriter, sql_text
        from import_local_sqlite import import_sql
        prefix, suffix = "UPDATE knowledge_nodes SET value='", "' WHERE id='one';"
        value = 'x' * (MAX_D1_SQL_STATEMENT_BYTES - len(prefix + suffix))
        with tempfile.TemporaryDirectory() as root, closing(self.database()) as source:
            database, sql = Path(root) / 'local.sqlite', Path(root) / 'input.sql'
            with closing(sqlite3.connect(database)) as disk:
                source.backup(disk)
            writer = SqlStatementWriter(sql)
            writer.append(prefix + value + suffix)
            writer.append('UPDATE edge_meta SET json_chunk=' + sql_text(json.dumps({'sha256': 'b'*64}))
                          + " WHERE key='data_revision';")
            writer.finish()
            sql.write_bytes(sql.read_bytes().removesuffix(b'\n'))
            self.assertEqual(import_sql(database, sql, 'a'*64, 'b'*64), 2)
            with closing(sqlite3.connect(database)) as disk:
                self.assertEqual(disk.execute("SELECT value FROM knowledge_nodes WHERE id='one'").fetchone()[0], value)

    def test_local_bootstrap_does_not_create_a_missing_database(self):
        from import_local_sqlite import import_sql
        with tempfile.TemporaryDirectory() as root:
            database, sql = Path(root) / 'missing.sqlite', Path(root) / 'input.sql'
            sql.write_text('SELECT 1;\n')
            with self.assertRaises(sqlite3.OperationalError):
                import_sql(database, sql, None, 'b'*64)
            self.assertFalse(database.exists())

    def test_sql_framing_bounds_unterminated_input_before_buffering_the_file(self):
        from sql_stream import MAX_SQL_STATEMENT_BYTES, sql_statements
        class OversizedStream:
            calls = 0
            def readline(self, limit):
                self.calls += 1
                self.limit = limit
                return b'x' * limit
        stream = OversizedStream()
        with self.assertRaisesRegex(ValueError, 'exceeds 100000 bytes'):
            next(sql_statements(stream))
        self.assertEqual(stream.calls, 1)
        self.assertLessEqual(stream.limit, MAX_SQL_STATEMENT_BYTES + 3)

    def test_utf8_chunks_are_bounded_and_lossless(self):
        from build_runtime import chunk_text
        original = "София ' λόγος 🌳 " * 100
        for size in (4, 7, 31, 32000):
            chunks = chunk_text(original, size)
            self.assertEqual(''.join(chunks), original)
            self.assertTrue(all(len(chunk.encode('utf-8')) <= size for chunk in chunks))
        self.assertEqual(chunk_text(''), [''])

    def test_sql_chunking_cannot_bypass_the_row_budget(self):
        from build_runtime import append_chunkable_insert
        with self.assertRaisesRegex(RuntimeError, 'row exceeds'):
            append_chunkable_insert(None, 'knowledge_nodes_next', ('id', 'json'),
                                    ("'large'", "'" + 'x' * 2_000_000 + "'"),
                                    selector_sql="id='large'", chunked_text={'json': 'x' * 2_000_000})

    def build(self, path, revision, values, previous=None):
        recorder = DeltaRecorder(path, revision, 'test-schema', previous)
        recorder.observe("INSERT INTO edge_meta_next (key, part, json_chunk) VALUES ('data_revision', 0, '" + json.dumps({'sha256': revision}) + "');")
        for key, value in values:
            recorder.observe(f"INSERT INTO knowledge_nodes_next (id, value) VALUES ('{key}', '{value}');")
        index = recorder.finish()
        return index, recorder.summary()

    def database(self):
        db = sqlite3.connect(':memory:')
        for table, keys in PRIMARY_KEYS.items():
            extra = ', json_chunk TEXT' if table == 'edge_meta' else ', value TEXT'
            db.execute(f'CREATE TABLE {table} ({", ".join(key + " TEXT" for key in keys)}{extra}, PRIMARY KEY ({", ".join(keys)}));')
        db.execute("INSERT INTO edge_meta VALUES ('data_revision', '0', ?)", (json.dumps({'sha256': 'a'*64}),))
        db.executemany('INSERT INTO knowledge_nodes VALUES (?, ?)', [('one', 'old'), ('two', 'stable'), ('three', 'removed')])
        db.commit()
        return db

    def test_only_delta_uploaded_and_publication_is_atomic_idempotent_and_guarded(self):
        with tempfile.TemporaryDirectory() as directory, closing(self.database()) as db:
            path = Path(directory) / 'patch.sql'
            base, _ = self.build(path, 'a'*64, [('one', 'old'), ('two', 'stable'), ('three', 'removed')])
            after, counts = self.build(path, 'b'*64, [('one', 'new'), ('two', 'stable')], base)
            self.assertEqual(counts['changed_rows'], 2)  # node + revision metadata
            self.assertEqual(counts['reused_rows'], 1)
            self.assertEqual(counts['removed_rows'], 1)
            sql = path.read_text()
            self.assertNotIn("'stable'", sql)
            publication = "INSERT OR REPLACE INTO tos_delta_publications SELECT"
            before_publish, after_publish = sql.split(publication, 1)
            db.executescript(before_publish)
            self.assertEqual(db.execute("SELECT value FROM knowledge_nodes WHERE id='one'").fetchone()[0], 'old')
            db.executescript(publication + after_publish)
            self.assertEqual(db.execute('SELECT * FROM knowledge_nodes ORDER BY id').fetchall(), [('one', 'new'), ('two', 'stable')])
            db.executescript(sql)  # complete retry does not duplicate or erase rows
            db.execute("UPDATE edge_meta SET json_chunk=?", (json.dumps({'sha256': 'c'*64}),))
            # A different target with a stale base must roll back publication.
            self.build(path, 'd'*64, [('one', 'bad'), ('two', 'stable')], base)
            with self.assertRaisesRegex(sqlite3.IntegrityError, 'stale delta baseline'):
                db.executescript(path.read_text())
            self.assertEqual(db.execute("SELECT value FROM knowledge_nodes WHERE id='one'").fetchone()[0], 'new')

    def test_stage_driven_delete_seeks_composite_keys_and_preserves_null_semantics(self):
        with closing(self.database()) as db:
            db.executemany('INSERT INTO knowledge_search_grams VALUES (?,?,?,?,?)',
                (('nodes', '3', f'g{i}', str(i), 'stable') for i in range(10000)))
            db.execute('INSERT INTO knowledge_search_grams VALUES (NULL,?,?,?,?)', ('3', 'null-key', '0', 'removed'))
            db.execute('CREATE TABLE staged_keys AS SELECT kind,n,gram,position FROM knowledge_search_grams WHERE 0')
            db.executemany('INSERT INTO staged_keys VALUES (?,?,?,?)',
                [('nodes', '3', 'g5000', '5000'), (None, '3', 'null-key', '0'), ('nodes', '3', 'absent', '9')])
            sql = delete_staged_keys_sql('knowledge_search_grams', 'staged_keys')
            plan = [row[3] for row in db.execute('EXPLAIN QUERY PLAN ' + sql)]
            self.assertTrue(any('SEARCH target USING' in row for row in plan), plan)
            self.assertFalse(any('SCAN knowledge_search_grams' in row or 'SCAN target' in row for row in plan), plan)
            steps = 0
            def budget():
                nonlocal steps
                steps += 1
                return int(steps > 2000)
            db.set_progress_handler(budget, 1)
            try:
                db.execute(sql)
            finally:
                db.set_progress_handler(None, 0)
            self.assertLess(steps, 2000)
            self.assertEqual(db.execute('SELECT count(*) FROM knowledge_search_grams').fetchone()[0], 9999)
            self.assertEqual(db.execute("SELECT value FROM knowledge_search_grams WHERE gram='g4999'").fetchone(), ('stable',))
            self.assertIsNone(db.execute("SELECT value FROM knowledge_search_grams WHERE gram='null-key'").fetchone())

    def test_changed_chunked_row_has_complete_value_and_key_parser_handles_quotes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'patch.sql'
            recorder = DeltaRecorder(path, 'a'*64, 'test-schema')
            recorder.observe("INSERT INTO knowledge_nodes_next (id, value) VALUES ('a''b,c', '');")
            recorder.observe("UPDATE knowledge_nodes_next SET value = value || 'part1' WHERE id = 'a''b,c';")
            recorder.observe("UPDATE knowledge_nodes_next SET value = value || 'part2' WHERE id = 'a''b,c';")
            base = recorder.finish()
            row = next(iter(base['rows']['knowledge_nodes'].values()))
            self.assertEqual(row['values'], ["'a''b,c'"])
            self.assertEqual(len(row['digest']), 64)
            self.assertEqual(recorder.summary()['changed_rows'], 1)

    def test_batched_search_rows_are_indexed_individually(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'patch.sql'
            recorder = DeltaRecorder(path, 'a' * 64, 'test-schema')
            recorder.observe(
                "INSERT INTO knowledge_search_grams_next (kind,n,gram,position) VALUES "
                "('nodes',3,'a''b,c',0),('nodes',3,'def',1);"
            )
            recorder.observe(
                "INSERT INTO knowledge_search_gram_stats_next (kind,n,gram,postings) VALUES "
                "('nodes',3,'a''b,c',1),('nodes',3,'def',1);"
            )
            index = recorder.finish()
            self.assertEqual(len(index['rows']['knowledge_search_grams']), 2)
            self.assertEqual(len(index['rows']['knowledge_search_gram_stats']), 2)
            grams = list(index['rows']['knowledge_search_grams'].values())
            self.assertIn(["'a''b,c'", '0'], [row['values'][2:] for row in grams])
            self.assertEqual(recorder.summary()['changed_rows'], 4)

    def test_disk_row_index_preserves_memory_contract_and_is_removed_after_finish(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            statements = [
                "INSERT INTO edge_meta_next (key, part, json_chunk) VALUES ('data_revision', 0, '{\"sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}');",
                "INSERT INTO knowledge_nodes_next (id, value) VALUES ('a''b,c', '');",
                "UPDATE knowledge_nodes_next SET value = value || 'part1' WHERE id = 'a''b,c';",
                "UPDATE knowledge_nodes_next SET value = value || 'part2' WHERE id = 'a''b,c';",
                "INSERT INTO knowledge_search_grams_next (kind,n,gram,position) VALUES ('nodes',3,'a''b,c',0),('nodes',3,'def',1);",
            ]

            memory = DeltaRecorder(root / 'memory.sql', 'a' * 64, 'test-schema')
            for statement in statements:
                memory.observe(statement)
            expected = memory.finish()

            disk = DeltaRecorder(
                root / 'disk.sql',
                'a' * 64,
                'test-schema',
                index_store_path=root / 'disk.rows.index.sqlite',
            )
            for statement in statements:
                disk.observe(statement)
            index_output = root / 'disk.rows.json.next'
            self.assertIsNone(disk.finish(index_output=index_output))
            self.assertEqual(json.loads(index_output.read_text()), expected)
            self.assertFalse((root / 'disk.rows.index.sqlite').exists())

    def test_posting_stats_sidecar_is_removed_on_success_and_failure(self):
        import build_runtime as builder

        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'search-gram-stats.sqlite'
            with builder.PostingStatsStore(path) as store:
                store.add('nodes', 'abc')
            self.assertFalse(path.exists())

            with self.assertRaisesRegex(RuntimeError, 'synthetic failure'):
                with builder.PostingStatsStore(path) as store:
                    store.add('relations', 'xyz')
                    raise RuntimeError('synthetic failure')
            self.assertFalse(path.exists())

    def test_disk_baseline_streams_large_rows_and_preserves_digest_lookup(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'baseline.rows.json'
            source_index = DiskRowIndex(root / 'source.rows.sqlite')
            large_literal = "'" + ('x' * (1024 * 1024 + 17)) + "'"
            source_index.record(
                'knowledge_nodes',
                '["large"]',
                'a' * 64,
                ["'large'", large_literal],
            )
            source_index.write_json(source, 'test-schema', 'b' * 64)
            source_index.close()
            source_index.path.unlink(missing_ok=True)

            baseline = DiskRowBaseline(source, 'test-schema', root / 'baseline.sqlite')
            self.assertEqual((baseline.schema, baseline.revision), ('test-schema', 'b' * 64))
            self.assertEqual(baseline.digest('knowledge_nodes', '["large"]'), 'a' * 64)
            row = next(baseline.iter_table('knowledge_nodes'))
            self.assertEqual(json.loads(row[2]), ["'large'", large_literal])
            baseline.close()
            self.assertFalse((root / 'baseline.sqlite').exists())

    def test_disk_baseline_rejects_overbudget_truncated_value(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'broken.rows.json'
            source.write_text(
                '{"schema":"test-schema","revision":"' + ('b' * 64)
                + '","rows":{"knowledge_nodes":{"[\\"one\\"]":{"digest":"'
                + ('a' * 64) + '","values":["' + ('x' * 1024) + '"',
                encoding='utf-8',
            )
            with self.assertRaisesRegex(ValueError, 'bounded size'):
                DiskRowBaseline(source, 'test-schema', root / 'baseline.sqlite', max_value_chars=128)
            self.assertFalse((root / 'baseline.sqlite').exists())

    def test_disk_baseline_roundtrips_long_escaped_producer_key(self):
        import build_runtime as builder

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            raw_key = 'x' * 8_300 + '"' + '\\' + '\n' + '\r' + '\t'
            key_literal = builder.sql_text(raw_key)
            statement = builder.sql_insert(
                'knowledge_nodes_next',
                ('id', 'value'),
                (key_literal, builder.sql_text('payload')),
            )
            key = json.dumps([key_literal], ensure_ascii=False, separators=(',', ':'))
            self.assertGreater(len(key), 8_192)
            self.assertLess(
                len(json.dumps(key, ensure_ascii=False, separators=(',', ':'))),
                ROW_INDEX_MAX_KEY_CHARS,
            )

            first_target = root / 'first.sql'
            first_recorder = DeltaRecorder(
                first_target,
                'a' * 64,
                'test-schema',
                index_store_path=root / 'first.rows.sqlite',
            )
            first_writer = builder.SqlStatementWriter(first_target, first_recorder)
            first_writer.append(statement)
            first_writer.finish(publish=False)
            first_index = root / 'first.rows.json.next'
            first_recorder.finish(index_output=first_index, publish=False)

            baseline = DiskRowBaseline(
                first_index,
                'test-schema',
                root / 'baseline.sqlite',
            )
            self.assertEqual(
                baseline.digest('knowledge_nodes', key),
                hashlib.sha256(statement.encode()).hexdigest(),
            )

            second_target = root / 'second.sql'
            second_recorder = DeltaRecorder(
                second_target,
                'b' * 64,
                'test-schema',
                baseline,
                index_store_path=root / 'second.rows.sqlite',
            )
            second_writer = builder.SqlStatementWriter(second_target, second_recorder)
            second_writer.append(statement)
            second_writer.finish(publish=False)
            second_index = root / 'second.rows.json.next'
            second_recorder.finish(index_output=second_index, publish=False)
            self.assertEqual(second_recorder.summary()['reused_rows'], 1)
            self.assertEqual(second_recorder.summary()['changed_rows'], 0)
            self.assertFalse((root / 'first.rows.sqlite').exists())
            self.assertFalse((root / 'second.rows.sqlite').exists())

    def test_disk_delta_materialization_failure_does_not_publish_sql_target(self):
        with tempfile.TemporaryDirectory() as directory:
            from unittest.mock import patch

            root = Path(directory)
            target = root / 'delta.sql'
            target.write_text('old delta\n', encoding='utf-8')
            recorder = DeltaRecorder(
                target,
                'a' * 64,
                'test-schema',
                index_store_path=root / 'rows.sqlite',
            )
            recorder.observe("INSERT INTO knowledge_nodes_next (id, value) VALUES ('one', 'new');")
            with patch.object(DiskRowIndex, 'write_json', side_effect=OSError('synthetic disk failure')):
                with self.assertRaisesRegex(OSError, 'synthetic disk failure'):
                    recorder.finish(index_output=root / 'rows.json.next')
            self.assertEqual(target.read_text(encoding='utf-8'), 'old delta\n')
            self.assertFalse((root / 'rows.sqlite').exists())

    def test_prepared_output_rename_rolls_back_partial_publish(self):
        import build_runtime as builder

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pairs = []
            for name in ('sql', 'delta', 'rows'):
                pending = root / f'{name}.next'
                target = root / name
                pending.write_text(f'new {name}\n', encoding='utf-8')
                target.write_text(f'old {name}\n', encoding='utf-8')
                pairs.append((pending, target))
            real_replace = builder.os.replace

            def fail_rows(source, target):
                if Path(source).name == 'rows.next':
                    raise OSError('synthetic rename failure')
                return real_replace(source, target)

            with patch.object(builder.os, 'replace', side_effect=fail_rows):
                with self.assertRaisesRegex(OSError, 'synthetic rename failure'):
                    builder.publish_prepared_files(tuple(pairs))
            for name in ('sql', 'delta', 'rows'):
                self.assertEqual((root / name).read_text(encoding='utf-8'), f'old {name}\n')
                self.assertFalse((root / f'{name}.rollback').exists())

    def test_schema_change_requires_full_baseline(self):
        with tempfile.TemporaryDirectory() as directory:
            recorder = DeltaRecorder(Path(directory)/'patch.sql', 'a'*64, 'new-schema', {'schema': 'old-schema'})
            recorder.finish()
            self.assertFalse(recorder.summary()['available'])


class SearchAddressPlanTests(unittest.TestCase):
    def database(self, members=('Alpha', 'alpha', 'stable')):
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        db.execute('CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part))')
        db.execute('INSERT INTO edge_meta VALUES (?,?,?)', ('data_revision', 0, json.dumps({'sha256': 'a'*64})))
        db.execute('CREATE TABLE knowledge_search_documents(kind TEXT,position INTEGER,id TEXT,id_lower TEXT,PRIMARY KEY(kind,position))')
        db.execute('CREATE TABLE knowledge_search_grams(kind TEXT,gram TEXT,position INTEGER,PRIMARY KEY(kind,gram,position))')
        for position, identity in enumerate(members):
            db.execute('INSERT INTO knowledge_search_documents VALUES (?,?,?,?)', ('nodes', position, identity, identity.lower()))
            db.execute('INSERT INTO knowledge_search_grams VALUES (?,?,?)', ('nodes', 'common', position))
        db.commit()
        return db

    def ready(self, db):
        db.execute('BEGIN IMMEDIATE')
        return prepare_search_address_indexes_transaction(db, expected_revision='a'*64)

    def plan(self, db, groups, **limits):
        return plan_search_addresses_transaction(db, expected_revision='a'*64,
            kind='nodes', successor_groups=groups, **limits)

    def apply_plan(self, db, plan):
        # Test-only source caller: one bounded posting per item. Actual publisher
        # must reconstruct complete documents/postings and all remaining lanes.
        for identity in plan['changed_ids']:
            old = plan['before'].get(identity)
            if old is not None:
                db.execute('DELETE FROM knowledge_search_grams WHERE kind=? AND position=?', ('nodes', old))
                db.execute('DELETE FROM knowledge_search_documents WHERE kind=? AND position=?', ('nodes', old))
        for identity in plan['changed_ids']:
            position = plan['after'].get(identity)
            if position is not None:
                db.execute('INSERT INTO knowledge_search_documents VALUES (?,?,?,?)', ('nodes', position, identity, identity.lower()))
                db.execute('INSERT INTO knowledge_search_grams VALUES (?,?,?)', ('nodes', 'common', position))

    def test_middle_insertion_preserves_unrelated_postings_and_exact_case_tie_order(self):
        db = self.database()
        before = list(db.iterdump())
        receipt = self.ready(db)
        self.assertEqual(receipt['posting_mutations'], 0)
        planned = self.plan(db, {'alpha': ['Alpha', 'aLpha', 'alpha'], 'beta': ['beta']})
        self.assertFalse(planned['source_closure_verified'])
        self.assertEqual(planned['before'], {'Alpha': 0, 'alpha': 1})
        self.assertEqual(planned['after'], {'Alpha': 3, 'aLpha': 4, 'alpha': 5, 'beta': 6})
        self.apply_plan(db, planned)
        # Same public rank/id_lower/source-order result as independent full
        # successor enumeration, although physical posting addresses differ.
        actual = [row[0] for row in db.execute('SELECT d.id FROM knowledge_search_grams g JOIN '
            'knowledge_search_documents d USING(kind,position) WHERE g.gram=? ORDER BY d.id_lower,d.position', ('common',))]
        successor = ['Alpha', 'aLpha', 'alpha', 'beta', 'stable']
        self.assertEqual(actual, sorted(successor, key=lambda identity: (identity.lower(), successor.index(identity))))
        self.assertEqual(db.execute("SELECT position FROM knowledge_search_documents WHERE id='stable'").fetchone(), (2,))
        self.assertEqual(db.execute("SELECT position FROM knowledge_search_grams WHERE position=2").fetchall(), [(2,)])
        db.rollback()
        self.assertEqual(list(db.iterdump()), before)  # optional indexes and test writes roll back together

    def test_content_deletion_reordering_and_distinct_insertions_are_bounded(self):
        db = self.database(tuple(f'old-{i:04}' for i in range(1000)) + ('Alpha', 'alpha'))
        self.ready(db)
        self.assertEqual(self.plan(db, {'alpha': ['Alpha', 'alpha']})['changed_ids'], [])
        removed = self.plan(db, {'alpha': ['alpha']})
        self.assertEqual(removed['after'], {'alpha': 1001})
        self.assertEqual(removed['changed_ids'], ['Alpha'])
        reordered = self.plan(db, {'alpha': ['alpha', 'Alpha']})
        self.assertEqual(list(reordered['after']), ['alpha', 'Alpha'])
        added = self.plan(db, {'middle': ['middle']}, max_groups=1, max_members=1)
        self.assertEqual(added['after'], {'middle': 1002})
        self.assertEqual(added['before'], {})
        empty = self.plan(db, {'alpha': []})
        self.assertEqual(empty['after'], {})
        self.assertEqual(empty['changed_ids'], ['Alpha', 'alpha'])
        query = list(db.execute("EXPLAIN QUERY PLAN SELECT id,position FROM knowledge_search_documents "
            "INDEXED BY knowledge_search_address_tie_idx WHERE kind='nodes' AND id_lower='alpha' ORDER BY position LIMIT 3"))
        self.assertTrue(any('SEARCH' in r[3] and 'knowledge_search_address_tie_idx' in r[3] for r in query))

    def test_refuses_unprepared_stale_oversized_or_corrupt_inputs_without_writes(self):
        db = self.database()
        with self.assertRaisesRegex(ValueError, 'caller transaction'):
            self.plan(db, {})
        db.execute('BEGIN')
        with self.assertRaisesRegex(ValueError, 'index preparation'):
            self.plan(db, {})
        with self.assertRaisesRegex(ValueError, 'preparation budget'):
            prepare_search_address_indexes_transaction(db, expected_revision='a'*64, max_documents=2)
        prepare_search_address_indexes_transaction(db, expected_revision='a'*64)
        before = list(db.iterdump())
        for groups, limits in (({'alpha': ['Alpha', 'Alpha']}, {}), ({'ALPHA': []}, {}),
                ({'alpha': ['not-alpha']}, {}), ({'alpha': []}, {'max_members': 1}),
                ({'alpha': []}, {'max_key_bytes': 6}), ({'a': [], 'b': []}, {'max_groups': 1})):
            with self.subTest(groups=groups, limits=limits), self.assertRaises(ValueError):
                self.plan(db, groups, **limits)
            self.assertEqual(list(db.iterdump()), before)
        with self.assertRaisesRegex(ValueError, 'predecessor differs'):
            plan_search_addresses_transaction(db, expected_revision='b'*64, kind='nodes', successor_groups={})
        db.execute("UPDATE knowledge_search_documents SET id_lower='wrong' WHERE id='Alpha'")
        with self.assertRaisesRegex(ValueError, 'omitted'):
            self.plan(db, {'alpha': ['Alpha', 'alpha']})
        db.execute("UPDATE knowledge_search_documents SET position=? WHERE id='stable'", (MAX_SEARCH_ADDRESS,))
        with self.assertRaisesRegex(ValueError, 'space exhausted'):
            self.plan(db, {'new': ['new']})

    def test_wrong_index_definition_is_not_silently_replaced(self):
        db = self.database()
        db.execute('CREATE INDEX knowledge_search_address_id_idx ON knowledge_search_documents(id)')
        db.execute('BEGIN')
        with self.assertRaisesRegex(ValueError, 'index preparation'):
            prepare_search_address_indexes_transaction(db, expected_revision='a'*64)
        db.rollback()
        self.assertEqual([r[2] for r in db.execute('PRAGMA index_info(knowledge_search_address_id_idx)')], ['id'])

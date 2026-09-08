from __future__ import annotations

import json
import sqlite3
import sys
import tempfile
import unittest
import os
from pathlib import Path
from contextlib import closing
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'deploy/cloudflare-worker/scripts'))
from incremental_runtime import DeltaRecorder, PRIMARY_KEYS


class IncrementalRuntimeTests(unittest.TestCase):
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

    def test_schema_change_requires_full_baseline(self):
        with tempfile.TemporaryDirectory() as directory:
            recorder = DeltaRecorder(Path(directory)/'patch.sql', 'a'*64, 'new-schema', {'schema': 'old-schema'})
            recorder.finish()
            self.assertFalse(recorder.summary()['available'])

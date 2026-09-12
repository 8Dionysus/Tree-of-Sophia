"""Private declaration storage integrity, not source or semantic admission."""
from contextlib import closing
from dataclasses import replace
import hashlib
import sqlite3
import unittest

from tos_access import prepared_source_dependencies as index
from tos_access.projection_mutation import _json_bytes
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
import test_prepared_source_binding as fixtures


class PreparedSourceDependencyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        fixtures.PreparedSourceBindingTests.setUpClass()

    def setUp(self):
        self.f = fixtures.PreparedSourceBindingTests()
        self.f.setUp()
        self.addCleanup(self.f.doCleanups)
        self.f.attach()
        self.db = self.f.db
        self.binding = self.f.f.binding
        self.source = self.f.before
        self.profile = 'a' * 64
        self.owner = index.ProgressHandlerOwner()

    def declaration(self, number, *, refs=(('identity', 'tos.agent.fixture'),), version=1):
        identifier = f'tos.claim.fixture-{number:04d}'
        source_ref = 'ToS/test/source-claims.jsonl'
        digest = hashlib.sha256(_json_bytes({'id': identifier, 'version': version}, 4096)).hexdigest()
        entry = {'claim_id': identifier, 'claim_sha256': digest, 'source_claim_file_ref': source_ref,
                 'source_claim_line': number + 1, 'claim_version': version, 'opaque': [None, False, 'Ω']}
        pairs = {('claim', identifier), ('path', source_ref), *refs}
        dependencies = [{'kind': kind, 'ref': ref, 'field_paths': ['/source_claim/test'],
                         'reasons': ['synthetic-index-fixture']}
                        for kind, ref in sorted(pairs, key=lambda item: (item[0], item[1] or ''))]
        return index.SourceClaimDependencies(claim_id=identifier, source_entry=entry,
                                            input_sha256=digest, dependencies=dependencies)

    def options(self, binding=None, source=None):
        return {'expected_binding': binding or self.binding,
                'source_inputs_sha256': (source or self.source).digest,
                'declaration_profile_sha256': self.profile, 'progress_owner': self.owner}

    def attach(self, declarations):
        self.db.execute('BEGIN IMMEDIATE')
        result = index.bootstrap_source_dependency_index_transaction(self.db,
            claims=iter(declarations), **self.options())
        self.assertTrue(self.db.in_transaction)
        self.db.commit()
        return result

    def lookup(self, kind='identity', ref='tos.agent.fixture', **options):
        return index.lookup_source_dependencies_transaction(self.db, kind=kind, ref=ref,
                                                            **(self.options() | options))

    def stage(self, changes, graph, after, **options):
        return index.apply_source_dependency_delta_transaction(self.db, expected_binding=self.binding,
            before_source_inputs_sha256=self.source.digest, after_source_inputs_sha256=after.digest,
            new_source_revision=graph['source_revision'], declaration_profile_sha256=self.profile,
            changes=changes, progress_owner=self.owner, **options)

    def finalize(self, binding, source, **options):
        return index.verify_source_dependency_binding_transaction(self.db, new_binding=binding,
            source_inputs_sha256=source.digest, declaration_profile_sha256=self.profile,
            progress_owner=self.owner, **options)

    def test_bootstrap_exact_addresses_complete_declarations_and_unchanged_publication(self):
        declarations = [self.declaration(2), self.declaration(0, refs=(('unresolved', None),)), self.declaration(1)]
        result = self.attach(declarations)
        self.assertEqual(result['claim_count'], 3)
        self.assertFalse(result['publication_changed'])
        reader = PublishedKnowledgeReadModel(self.f.f.path, self.binding)
        self.assertEqual(reader.catalog(), self.f.f.catalog)
        self.db.execute('BEGIN')
        found = self.lookup()
        self.assertEqual(found['claim_ids'], [declarations[2].claim_id, declarations[0].claim_id])
        self.assertEqual([row['digest'] for row in found['declarations']], [declarations[2].digest, declarations[0].digest])
        self.assertTrue(found['complete_stored_address_verified'])
        self.assertEqual(self.lookup('unresolved', None)['claim_ids'], [declarations[1].claim_id])
        self.assertEqual(self.lookup('identity', 'not-present')['declarations'], [])
        for key in ('source_completeness_verified', 'source_transition_verified', 'source_verification_performed',
                    'target_closure_verified', 'semantic_acceptance', 'consumer_switched'):
            self.assertFalse(found[key])
        one = index.read_source_claim_dependencies_transaction(self.db, claim_id=declarations[0].claim_id,
                                                               **self.options())
        self.assertEqual(one['declaration'], {'digest': declarations[0].digest, **declarations[0].value()})
        self.assertEqual(one['sql_mutations'], 0)
        self.db.rollback()

    def test_insert_update_delete_join_source_and_rows_in_one_commit(self):
        old, deleted = self.declaration(1), self.declaration(2)
        self.attach([old, deleted])
        updated = self.declaration(1, refs=(('identity', 'tos.place.fixture'), ('unresolved', 'opaque:future')), version=2)
        inserted = self.declaration(3)
        graph, after = self.f.delta()
        changes = [index.SourceDependencyChange('update', old.claim_id, old.digest, updated),
                   index.SourceDependencyChange('delete', deleted.claim_id, deleted.digest),
                   index.SourceDependencyChange('insert', inserted.claim_id, declaration=inserted)]
        self.db.execute('BEGIN IMMEDIATE')
        stage = self.stage(changes, graph, after)
        self.assertTrue(stage['pending'])
        self.assertEqual(stage['finalize_sql_mutations_upper_bound'], 4)
        with self.assertRaisesRegex(ValueError, 'pending/stale'):
            self.lookup()
        self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        self.stage(changes, graph, after)
        published = self.f.apply(graph, after)
        final = self.finalize(published['binding'], after)
        self.assertEqual(final['sql_mutations'], 4)
        self.assertTrue(self.db.in_transaction)
        with closing(sqlite3.connect(self.f.f.path)) as independent:
            self.assertEqual(independent.execute('SELECT digest FROM source_dependency_claims WHERE claim_id=?',
                                                (old.claim_id,)).fetchone()[0], old.digest)
        self.db.commit()
        self.db.execute('BEGIN')
        options = self.options(published['binding'], after)
        self.assertEqual(self.lookup(**options)['claim_ids'], [inserted.claim_id])
        self.assertEqual(self.lookup('identity', 'tos.place.fixture', **options)['claim_ids'], [updated.claim_id])
        self.assertEqual(self.lookup('unresolved', 'opaque:future', **options)['claim_ids'], [updated.claim_id])
        absent = index.read_source_claim_dependencies_transaction(self.db, claim_id=deleted.claim_id, **options)
        self.assertIsNone(absent['declaration'])
        self.db.rollback()
        self.assertEqual(PublishedKnowledgeReadModel(self.f.f.path, published['binding']).node('a')['matches'][0]
                         ['display']['title'], 'After')
        with self.assertRaises(PublishedSnapshotConflict):
            PublishedKnowledgeReadModel(self.f.f.path, self.binding).catalog()

    def test_empty_declaration_delta_advances_binding_without_claim_or_graph_reads(self):
        declaration = self.declaration(1)
        self.attach([declaration])
        graph, after = self.f.delta()
        denied = {'source_dependency_claims', 'source_dependency_refs', 'knowledge_nodes', 'knowledge_relations'}
        def mask(action, table, _column, _database, _trigger):
            return sqlite3.SQLITE_DENY if action == sqlite3.SQLITE_READ and table in denied else sqlite3.SQLITE_OK
        self.db.execute('BEGIN IMMEDIATE')
        self.db.set_authorizer(mask)
        try:
            stage = self.stage([], graph, after)
        finally:
            self.db.set_authorizer(None)
        self.assertEqual(stage['sql_mutations'], 1)
        published = self.f.apply(graph, after)
        self.db.set_authorizer(mask)
        try:
            final = self.finalize(published['binding'], after)
        finally:
            self.db.set_authorizer(None)
        self.assertEqual(final['sql_mutations'], 1)
        self.db.commit()
        self.db.execute('BEGIN')
        found = self.lookup(**self.options(published['binding'], after))
        self.assertEqual(found['declarations'][0]['digest'], declaration.digest)
        self.db.rollback()

    def test_stale_preconditions_refuse_before_lane_mutation(self):
        old = self.declaration(1)
        self.attach([old])
        graph, after = self.f.delta()
        invalid = [
            [index.SourceDependencyChange('update', old.claim_id, '0' * 64, self.declaration(1, version=2))],
            [index.SourceDependencyChange('insert', old.claim_id, declaration=old)],
            [index.SourceDependencyChange('delete', 'tos.claim.absent', '0' * 64)],
            [index.SourceDependencyChange('delete', old.claim_id, old.digest)] * 2,
        ]
        for changes in invalid:
            self.db.execute('BEGIN IMMEDIATE')
            start = self.db.total_changes
            with self.assertRaises(ValueError):
                self.stage(changes, graph, after)
            self.assertEqual(self.db.total_changes, start)
            self.db.rollback()
        for options in ({'source_inputs_sha256': '0' * 64}, {'declaration_profile_sha256': '0' * 64},
                        {'expected_binding': {**self.binding, 'publication_epoch': self.binding['publication_epoch'] + 1}}):
            self.db.execute('BEGIN')
            with self.assertRaises(ValueError):
                self.lookup(**options)
            self.db.rollback()

    def test_selected_corruption_and_missing_index_refuse_without_scan_fallback(self):
        declaration = self.declaration(1)
        self.attach([declaration])
        mutations = [
            ('UPDATE source_dependency_claims SET digest=?', ('0' * 64,)),
            ("DELETE FROM source_dependency_refs WHERE kind='identity'", ()),
            ("UPDATE source_dependency_heads SET xor_sha256=? WHERE kind='identity'", ('0' * 64,)),
            ('UPDATE source_dependency_state SET json=?', ('{}',)),
            ('DROP INDEX source_dependency_claim_refs', ()),
        ]
        for sql, args in mutations:
            self.db.execute('BEGIN IMMEDIATE')
            self.db.execute(sql, args)
            with self.assertRaises(ValueError):
                self.lookup()
            self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        value = declaration.value()
        value['source_entry']['opaque'].append('changed without entry digest')
        raw = _json_bytes(value, 100000)
        self.db.execute('UPDATE source_dependency_claims SET declaration=?,digest=?',
                        (raw.decode(), hashlib.sha256(raw).hexdigest()))
        with self.assertRaises(ValueError):
            self.lookup()
        self.db.rollback()

    def test_late_finalize_failure_rolls_back_every_lane_and_caller_sentinel(self):
        old = self.declaration(1)
        self.attach([old])
        tables = ['source_dependency_state', 'source_dependency_claims', 'source_dependency_refs',
                  'source_dependency_heads', 'prepared_source_state', 'edge_meta', 'knowledge_nodes',
                  'semantic_state', 'catalog_state', 'search_documents']
        before = {name: self.db.execute('SELECT * FROM ' + name).fetchall() for name in tables}
        graph, after = self.f.delta()
        self.db.execute('BEGIN IMMEDIATE')
        self.db.execute('CREATE TABLE caller_sentinel(value TEXT)')
        self.stage([index.SourceDependencyChange('update', old.claim_id, old.digest,
                                               self.declaration(1, version=2))], graph, after)
        published = self.f.apply(graph, after)
        def refuse_final_write(action, table, *_args):
            return sqlite3.SQLITE_DENY if action == sqlite3.SQLITE_UPDATE and table == 'source_dependency_state' else sqlite3.SQLITE_OK
        self.db.set_authorizer(refuse_final_write)
        try:
            with self.assertRaises(sqlite3.DatabaseError):
                self.finalize(published['binding'], after)
        finally:
            self.db.set_authorizer(None)
        self.assertTrue(self.db.in_transaction)
        self.db.rollback()
        for name in tables:
            self.assertEqual(self.db.execute('SELECT * FROM ' + name).fetchall(), before[name], name)
        self.assertIsNone(self.db.execute("SELECT name FROM sqlite_master WHERE name='caller_sentinel'").fetchone())

    def test_explicit_budget_refusal_and_exact_write_retry(self):
        old = self.declaration(1)
        self.attach([old])
        for field in ('max_rows', 'max_queries', 'max_read_bytes', 'max_input_bytes', 'max_output_bytes',
                      'max_row_bytes', 'max_vm_steps', 'max_dependencies_per_claim', 'max_bytes'):
            self.db.execute('BEGIN')
            start = self.db.total_changes
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.lookup(limits=replace(index.SourceDependencyLimits(), **{field: 1}))
            self.assertEqual(self.db.total_changes, start)
            self.db.rollback()
        graph, after = self.f.delta()
        changes = [index.SourceDependencyChange('update', old.claim_id, old.digest, self.declaration(1, version=2))]
        self.db.execute('BEGIN IMMEDIATE')
        actual = self.stage(changes, graph, after)['sql_mutations']
        self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        with self.assertRaises(ValueError):
            self.stage(changes, graph, after, limits=replace(index.SourceDependencyLimits(), max_writes=actual - 1))
        self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        result = self.stage(changes, graph, after, limits=replace(index.SourceDependencyLimits(), max_writes=actual))
        self.assertEqual(result['sql_mutations'], actual)
        self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        start = self.db.total_changes
        with self.assertRaisesRegex(ValueError, 'delta byte budget'):
            self.stage(changes, graph, after, limits=replace(index.SourceDependencyLimits(), max_change_bytes=1))
        self.assertEqual(self.db.total_changes, start)
        self.db.rollback()

    def test_high_fanout_claim_delta_never_reads_the_address_bucket(self):
        declarations = [self.declaration(number) for number in range(200)]
        self.attach(declarations)
        graph, after = self.f.delta()
        self.db.execute('BEGIN IMMEDIATE')
        statements = []
        self.db.set_trace_callback(statements.append)
        try:
            result = self.stage([index.SourceDependencyChange('update', declarations[0].claim_id,
                declarations[0].digest, self.declaration(0, version=2))], graph, after,
                limits=replace(index.SourceDependencyLimits(), max_claims=1, max_rows=160))
        finally:
            self.db.set_trace_callback(None)
        self.assertLess(result['budget_usage']['rows'], 160)
        selected = [sql for sql in statements if sql.startswith('SELECT ') and ' FROM source_dependency_refs ' in sql]
        self.assertTrue(selected)
        self.assertTrue(all('claim_id=' in sql or ' LIMIT 2' in sql or ' LIMIT 1' in sql for sql in selected), selected)
        self.db.rollback()
        self.db.execute('BEGIN')
        with self.assertRaisesRegex(ValueError, 'fanout budget'):
            self.lookup(limits=replace(index.SourceDependencyLimits(), max_claims=1))
        self.db.rollback()

    def test_caller_handler_row_factory_and_transaction_are_preserved_on_success_and_failure(self):
        self.attach([self.declaration(1)])
        calls = []
        def previous():
            calls.append(1)
            return 0
        self.db.set_progress_handler(previous, 1)
        self.db.row_factory = sqlite3.Row
        self.db.execute('BEGIN')
        owner = index.ProgressHandlerOwner(previous, 1)
        self.lookup(progress_owner=owner)
        self.assertIs(self.db.row_factory, sqlite3.Row)
        before = len(calls)
        self.db.execute('SELECT count(*) FROM source_dependency_claims').fetchone()
        self.assertGreater(len(calls), before)
        with self.assertRaises(ValueError):
            self.lookup(progress_owner=owner, limits=replace(index.SourceDependencyLimits(), max_vm_steps=1))
        before = len(calls)
        self.db.execute('SELECT count(*) FROM source_dependency_claims').fetchone()
        self.assertGreater(len(calls), before)
        self.assertTrue(self.db.in_transaction)
        self.db.rollback()
        self.db.set_progress_handler(None, 0)
        self.db.row_factory = None

    def test_declaration_detachment_validation_and_no_implicit_transaction(self):
        declaration = self.declaration(1)
        detached = declaration.value()
        detached['dependencies'].clear()
        self.assertTrue(declaration.value()['dependencies'])
        self.assertEqual(index.SourceClaimDependencies.parse(declaration.raw), declaration)
        for field, value in (('input_sha256', '0' * 64), ('source_entry_sha256', '0' * 64)):
            changed = declaration.value()
            changed[field] = value
            with self.assertRaises(ValueError):
                index.SourceClaimDependencies.parse(_json_bytes(changed, 100000))
        with self.assertRaisesRegex(ValueError, 'caller-owned transaction'):
            index.bootstrap_source_dependency_index_transaction(self.db, claims=[declaration], **self.options())
        self.assertIsNone(self.db.execute("SELECT name FROM sqlite_master WHERE name='source_dependency_state'").fetchone())

    def test_real_enumerator_roundtrip_and_metadata_only_declaration_stability(self):
        # The SQLite/root fixture remains synthetic. This checks the actual
        # enumerator ABI, not whether that root covers these real source bytes.
        import json
        import test_bibliographic_claim_projector as source_fixture
        import source_witness_bibliographic_graph_common as source
        fixture = source_fixture.BibliographicClaimProjectorTest()
        fixture.setUp()
        with fixture.fixture.historical_fixture() as (root, _history, real, claims, rebuild):
            agent_id = real[0]['record_id']
            claims[1]['maker'] = {'maker_type': 'human', 'agent_ref': agent_id}
            claims[2]['evidence_refs'] = [agent_id]
            def declarations():
                _graph, inputs = fixture.capture(rebuild)
                return [index.SourceClaimDependencies(claim_id=value.source_claim['claim_id'],
                    source_entry=value.entry, input_sha256=value.entry['claim_sha256'],
                    dependencies=source.enumerate_bibliographic_claim_dependencies(value)) for value in inputs]
            before = declarations()
            self.attach(before)
            self.db.execute('BEGIN')
            found = self.lookup('identity', agent_id)
            self.assertEqual(found['claim_ids'], sorted(value.claim_id for value in before))
            self.assertEqual(found['declarations'], [{'digest': value.digest, **value.value()} for value in before])
            self.db.rollback()
            path = root / 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json'
            agent = json.loads(path.read_bytes())
            agent.update(preferred_label='Synthetic revised Agent', record_version=agent['record_version'] + 1)
            path.write_text(json.dumps(agent))
            self.assertEqual(declarations(), before)

    def test_missing_head_cannot_be_silently_recreated_by_a_small_insert(self):
        old = self.declaration(1)
        self.attach([old])
        graph, after = self.f.delta()
        self.db.execute('BEGIN IMMEDIATE')
        self.db.execute("DELETE FROM source_dependency_heads WHERE kind='identity'")
        inserted = self.declaration(2)
        with self.assertRaisesRegex(ValueError, 'head missing'):
            self.stage([index.SourceDependencyChange('insert', inserted.claim_id, declaration=inserted)], graph, after)
        self.db.rollback()

    def test_interrupted_bootstrap_leaves_rollback_to_caller_and_can_retry(self):
        declaration = self.declaration(1)
        def interrupted():
            yield declaration
            raise KeyboardInterrupt('synthetic producer interruption')
        self.db.execute('BEGIN IMMEDIATE')
        self.db.execute('CREATE TABLE caller_sentinel(value TEXT)')
        with self.assertRaises(KeyboardInterrupt):
            index.bootstrap_source_dependency_index_transaction(self.db, claims=interrupted(), **self.options())
        self.assertTrue(self.db.in_transaction)
        self.db.rollback()
        for name in ('source_dependency_claims', 'caller_sentinel'):
            self.assertIsNone(self.db.execute('SELECT name FROM sqlite_master WHERE name=?', (name,)).fetchone())
        self.attach([declaration])

    def test_caller_iterator_ending_transaction_is_not_followed_by_autocommit_writes(self):
        old = self.declaration(1)
        self.attach([old])
        graph, after = self.f.delta()
        def invalid_caller():
            yield index.SourceDependencyChange('update', old.claim_id, old.digest, self.declaration(1, version=2))
            self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        with self.assertRaisesRegex(ValueError, 'caller transaction ended'):
            self.stage(invalid_caller(), graph, after)
        self.assertFalse(self.db.in_transaction)
        self.assertEqual(self.db.execute('SELECT digest FROM source_dependency_claims WHERE claim_id=?',
                                        (old.claim_id,)).fetchone()[0], old.digest)


if __name__ == '__main__':
    unittest.main()

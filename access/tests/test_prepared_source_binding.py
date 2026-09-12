"""Atomic private roots and prepared rows, not source-chain admission."""
from dataclasses import replace
from pathlib import Path
import sqlite3
import unittest
from unittest.mock import patch

from tos_access import prepared_source_binding as paired
from tos_access.projection_store import Collection, ProjectionReader, canonical_bytes, write_projection
from tos_access.projection_mutation import ProjectionSnapshotView, ProjectionChange, stage_projection_changes
from tos_access.prepared_publication import PreparedChange, PublicationLimits
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
import test_prepared_semantics as fixtures


class PreparedSourceBindingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        fixtures.PreparedSemanticTests.setUpClass()

    def setUp(self):
        self.f = fixtures.PreparedSemanticTests()
        self.f.setUp()
        self.addCleanup(self.f.doCleanups)
        self.db = self.f.db
        self.f.attach()
        self.root_path = Path(self.f.tmp.name) / 'source-catalog.json'
        write_projection(self.root_path, {'schema_version': 'test_source_catalog_v2'},
                         {'records': Collection([('agent', {'label': 'before'})], None)},
                         work_dir=self.root_path.parent)
        self.root = ProjectionSnapshotView(self.root_path.read_bytes(), self.root_path)
        self.before = self.inputs(self.f.graph['source_revision'], self.root, None)

    def inputs(self, revision, view, publication):
        return paired.PreparedSourceInputs(source_revision=revision, source_publication=publication,
            dependencies={'entity-registry': self.f.graph['normalization_binding']['entity_registry_digest']},
            roots={'source-catalog': view})

    def attach(self):
        self.db.execute('BEGIN IMMEDIATE')
        result = paired.bootstrap_prepared_source_inputs_transaction(self.db,
            expected_binding=self.f.binding, inputs=self.before)
        self.db.commit()
        return result

    def delta(self):
        selected = ProjectionReader(self.root_path)
        import hashlib
        candidate = stage_projection_changes(selected, expected_before_sha256=selected.snapshot_digest,
            trusted_baseline_sha256=selected.snapshot_digest,
            changes=[ProjectionChange('records', 'agent', True,
                     hashlib.sha256(canonical_bytes({'label': 'before'})).hexdigest(), True, {'label': 'after'})])
        graph = self.f.after()
        graph['nodes'][0]['display']['title'] = 'After'
        after = self.inputs(graph['source_revision'], candidate.snapshot(), 'sha256:' + 'a' * 64)
        return graph, after

    def apply(self, graph, after, **options):
        return paired.apply_source_bound_prepared_delta_transaction(self.db,
            expected_binding=self.f.binding, before_source_inputs=self.before, after_source_inputs=after,
            before_inputs=self.f.inputs(self.f.graph), after_inputs=self.f.inputs(graph),
            changes=[PreparedChange('update', 'node', 'a', graph['nodes'][0])], **options)

    def selected(self, binding):
        return paired.read_prepared_source_inputs_transaction(self.db, expected_binding=binding)

    def test_explicit_bootstrap_keeps_reader_binding_and_detached_native_roots(self):
        result = self.attach()
        self.assertEqual(result['binding'], self.f.binding)
        self.db.execute('BEGIN')
        observed = self.selected(self.f.binding)
        self.assertEqual(observed, self.before)
        self.assertEqual(observed.roots()['source-catalog'].lookup('records', 'agent')['value'], {'label': 'before'})
        detached = observed.value()
        detached['roots'].clear()
        self.assertEqual(len(observed.roots()), 1)
        self.assertEqual(result['sql_mutations'], 1)
        self.db.rollback()
        self.assertEqual(PublishedKnowledgeReadModel(self.f.path, self.f.binding).catalog(), self.f.catalog)
        for key in ('source_transition_verified', 'target_closure_verified', 'semantic_acceptance', 'consumer_switched'):
            self.assertFalse(result[key])
        self.db.execute('BEGIN')
        with self.assertRaises(sqlite3.OperationalError):
            paired.bootstrap_prepared_source_inputs_transaction(self.db, expected_binding=self.f.binding, inputs=self.before)
        self.db.rollback()

    def test_one_commit_changes_rows_and_selected_root_without_root_file_replacement(self):
        self.attach()
        graph, after = self.delta()
        old_root = self.root_path.read_bytes()
        old_reader = PublishedKnowledgeReadModel(self.f.path, self.f.binding)
        original_read = Path.read_bytes
        def without_root_read(path):
            self.assertNotEqual(path, self.root_path)
            return original_read(path)
        self.db.execute('BEGIN IMMEDIATE')
        with patch.object(ProjectionReader, 'materialize', side_effect=AssertionError('full source read')), \
             patch.object(Path, 'read_bytes', without_root_read):
            result = self.apply(graph, after)
            self.assertEqual(self.selected(result['binding']), after)
        # Other connections still observe the entire old selection.
        self.assertEqual(old_reader.node('a')['matches'][0]['display']['title'], self.f.graph['nodes'][0]['display']['title'])
        with sqlite3.connect(self.f.path) as independent:
            self.assertEqual(independent.execute('SELECT sha256 FROM prepared_source_state').fetchone()[0], self.before.digest)
        self.db.commit()
        self.assertEqual(self.root_path.read_bytes(), old_root)
        reader = PublishedKnowledgeReadModel(self.f.path, result['binding'])
        self.assertEqual(reader.node('a')['matches'][0]['display']['title'], 'After')
        self.db.execute('BEGIN')
        self.assertEqual(self.selected(result['binding']).roots()['source-catalog'].lookup('records', 'agent')['value'], {'label': 'after'})
        self.db.rollback()
        with self.assertRaises(PublishedSnapshotConflict):
            old_reader.catalog()
        self.assertTrue(result['roots_paired_in_caller_transaction'])

    def test_late_root_write_failure_rolls_back_all_prepared_lanes_and_sentinel(self):
        self.attach()
        graph, after = self.delta()
        self.db.execute("CREATE TRIGGER fail_source BEFORE UPDATE ON prepared_source_state BEGIN SELECT RAISE(ABORT,'injected pairing failure'); END")
        tables = ['prepared_source_state', 'edge_meta', 'knowledge_nodes', 'knowledge_relations', 'search_documents', 'semantic_state', 'catalog_state']
        before = {name: self.db.execute('SELECT * FROM ' + name).fetchall() for name in tables}
        self.db.execute('BEGIN IMMEDIATE')
        self.db.execute('CREATE TABLE caller_sentinel(value TEXT)')
        with self.assertRaises(sqlite3.IntegrityError):
            self.apply(graph, after)
        self.assertTrue(self.db.in_transaction)
        self.db.rollback()
        for name in tables:
            self.assertEqual(self.db.execute('SELECT * FROM ' + name).fetchall(), before[name], name)
        self.assertIsNone(self.db.execute("SELECT name FROM sqlite_master WHERE name='caller_sentinel'").fetchone())

    def test_combined_write_budget_includes_source_selection_and_survives_retry(self):
        self.attach()
        graph, after = self.delta()
        self.db.execute('BEGIN IMMEDIATE')
        result = self.apply(graph, after)
        writes = result['sql_mutations']
        self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        with self.assertRaises(ValueError):
            self.apply(graph, after, limits=replace(PublicationLimits(), max_mutations=writes - 1))
        self.db.rollback()
        self.db.execute('BEGIN IMMEDIATE')
        exact = self.apply(graph, after, limits=replace(PublicationLimits(), max_mutations=writes))
        self.assertEqual(exact['sql_mutations'], writes)
        self.db.commit()
        self.db.execute('BEGIN IMMEDIATE')
        with self.assertRaises(ValueError):
            self.apply(graph, after)
        self.db.rollback()

    def test_wrong_predecessor_revision_and_root_namespace_fail_before_mutation(self):
        self.attach()
        graph, after = self.delta()
        for kind in ('before', 'revision', 'namespace'):
            self.db.execute('BEGIN IMMEDIATE')
            start = self.db.total_changes
            changed = after
            if kind == 'before':
                self.db.execute("UPDATE prepared_source_state SET sha256=?", ('f' * 64,))
                start = self.db.total_changes
            elif kind == 'revision':
                changed = self.inputs('e' * 64, self.root, None)
            else:
                changed = self.inputs(graph['source_revision'], ProjectionSnapshotView(self.root.root_bytes,
                    self.root_path.parent / 'other' / self.root_path.name), None)
            with self.assertRaises(ValueError):
                self.apply(graph, changed)
            self.assertEqual(self.db.total_changes, start)
            self.db.rollback()

    def test_corrupt_or_over_budget_selection_refuses_without_source_file_reads(self):
        self.attach()
        for kind in ('digest', 'root', 'oversize', 'type'):
            self.db.execute('BEGIN IMMEDIATE')
            value = self.before.value()
            if kind == 'digest':
                self.db.execute("UPDATE prepared_source_state SET sha256=?", ('f' * 64,))
            elif kind == 'root':
                value['roots']['source-catalog']['snapshot_sha256'] = 'f' * 64
                self.db.execute('UPDATE prepared_source_state SET inputs=?', (canonical_bytes(value).decode(),))
            else:
                self.db.execute('UPDATE prepared_source_state SET inputs=?',
                                ('x' * (paired.MAX_STATE_BYTES + 1) if kind == 'oversize' else b'{}',))
            with patch.object(Path, 'read_bytes', side_effect=AssertionError('filesystem root fallback')), self.assertRaises(ValueError):
                self.selected(self.f.binding)
            self.db.rollback()

    def test_root_roles_and_collection_identity_cannot_migrate_in_small_delta(self):
        self.attach()
        graph, after = self.delta()
        for kind in ('roles', 'collection'):
            roots = after.roots()
            if kind == 'roles':
                roots['another-role'] = roots.pop('source-catalog')
            else:
                import json
                view = roots['source-catalog']
                manifest = json.loads(view.root_bytes)
                manifest['collections']['renamed'] = manifest['collections'].pop('records')
                roots['source-catalog'] = ProjectionSnapshotView(canonical_bytes(manifest), view.namespace_path)
            changed = paired.PreparedSourceInputs(source_revision=graph['source_revision'],
                source_publication=after.value()['source_publication'],
                dependencies=after.value()['dependencies'], roots=roots)
            self.db.execute('BEGIN IMMEDIATE')
            start = self.db.total_changes
            with self.assertRaises(ValueError):
                self.apply(graph, changed)
            self.assertEqual(self.db.total_changes, start)
            self.db.rollback()

    def test_bootstrap_budget_failure_requires_whole_rollback_and_clean_retry(self):
        self.db.execute('BEGIN IMMEDIATE')
        self.db.execute('CREATE TABLE caller_sentinel(value TEXT)')
        with self.assertRaises(ValueError):
            paired.bootstrap_prepared_source_inputs_transaction(self.db,
                expected_binding=self.f.binding, inputs=self.before,
                limits=replace(PublicationLimits(), max_mutations=0))
        self.assertTrue(self.db.in_transaction)
        self.db.rollback()
        for table in ('caller_sentinel', 'prepared_source_state'):
            self.assertIsNone(self.db.execute('SELECT name FROM sqlite_master WHERE name=?', (table,)).fetchone())
        self.assertEqual(self.attach()['sql_mutations'], 1)

    def test_constructor_and_transactions_are_explicit_without_closure_claim(self):
        with self.assertRaises(ValueError):
            paired.bootstrap_prepared_source_inputs_transaction(self.db, expected_binding=self.f.binding, inputs=self.before)
        for revision, token in (('bad', None), ('a' * 64, 'a' * 64)):
            with self.assertRaises(ValueError):
                self.inputs(revision, self.root, token)
        with self.assertRaises(ValueError):
            paired.PreparedSourceInputs(source_revision='a' * 64, source_publication=None, dependencies={}, roots={})
        # Construction/selection does not certify retained part availability.
        missing_namespace = self.root_path.parent / 'never-created' / self.root_path.name
        supplied = self.inputs(self.f.graph['source_revision'], ProjectionSnapshotView(self.root.root_bytes, missing_namespace), None)
        self.assertFalse(missing_namespace.exists())
        self.db.execute('BEGIN')
        result = paired.bootstrap_prepared_source_inputs_transaction(self.db, expected_binding=self.f.binding, inputs=supplied)
        self.assertFalse(result['target_closure_verified'])
        self.db.rollback()


if __name__ == '__main__':
    unittest.main()

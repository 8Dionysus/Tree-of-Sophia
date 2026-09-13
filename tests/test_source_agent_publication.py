"""Real tiny owner revision and all-lane prepared publication, no live corpus."""
from contextlib import closing
import copy
import json
from pathlib import Path
import sqlite3
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
for directory in ('scripts', 'access/src', 'access/tests', 'tests', 'mechanics/growth-cycle/tests'):
    sys.path.insert(0, str(ROOT / directory))

import test_bibliographic_claim_assembler as fixtures
import source_agent_publication as publication
import bibliographic_claim_assembler as assembly
import build_source_witness_catalog as legacy
import source_witness_bibliographic_graph_common as bibliography
import tos_corpus_index_common as navigation
from tos_access import knowledge as k
from tos_access.catalog_semantics import CatalogInputs, CANONICAL_ORDER, memory_catalog
from tos_access.projection_store import Collection, ProjectionReader, write_projection, canonical_bytes
from tos_access.projection_mutation import ProjectionSnapshotView
from tos_access.prepared_publication import publish_prepared
from tos_access.prepared_semantics import bootstrap_prepared_maintenance_transaction
from tos_access.prepared_source_binding import bootstrap_prepared_source_inputs_transaction
from tos_access.prepared_source_dependencies import (SourceClaimDependencies, ProgressHandlerOwner,
    bootstrap_source_dependency_index_transaction)
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
from tos_access.published_search import PublishedSearchService
from tos_access.published_lens import PublishedLensService
from test_indexed_lens import lens


class SourceAgentPublicationTests(unittest.TestCase):
    def setUp(self):
        self.f = fixtures.BibliographicClaimAssemblerTests()
        self.addCleanup(self.f.doCleanups)
        self.helper, self.claim, ref = self.f.native_claim_fixture()
        self.hidden_claim = {**copy.deepcopy(self.claim), 'claim_id': 'tos.claim.hidden-maker-fixture',
            'subject_ref': self.helper.fixture.other['record_id'], 'object': self.helper.fixture.other['record_id'],
            'maker': {'maker_type': 'software', 'agent_ref': self.helper.fixture.identity}}
        # Only one schema-real social Claim; no invalid authored-by Agent fixture.
        self.helper.fixture.write(self.helper.claim_ref, b'')
        self.helper.fixture.write(ref, canonical_bytes(self.claim) + canonical_bytes(self.hidden_claim))
        self.helper.fixture.rebuild()
        self.root = self.helper.root
        self.snapshot = self.helper.snapshot(self.helper.bootstrap())
        self.owner = ProgressHandlerOwner()
        self.profile = publication.declaration_profile_sha256()
        self.entities, self.relations = [json.loads((self.root / 'ToS/doctrine/semantic-interchange' / name).read_text())
            for name in ('entity-types.v1.json', 'relation-types.v1.json')]
        self.corpus, self.bibliography, self.graph = self.full_build()
        roots = {'source-catalog': self.snapshot.view}
        for role, value, fields in (
                ('source-navigation', self.corpus['source_navigation'], {'nodes': 'node_id', 'edges': 'edge_id'}),
                ('bibliographic-claims', self.bibliography, {'nodes': 'node_id', 'edges': 'edge_id', 'claim_traces': 'claim_ref'})):
            path = self.root / 'derived' / (role + '.json')
            write_projection(path, {'schema_version': publication.RAW_SCHEMAS[role]},
                {name: Collection(value[name], key, (key,)) for name, key in fields.items()},
                work_dir=self.root / 'scratch', target_part_bytes=1024)
            roots[role] = ProjectionSnapshotView(path.read_bytes(), path)
        self.source = publication.source_vector_inputs(roots=roots, dependencies={
            'declaration-profile': self.profile,
            'agent-publication-profile': publication.execution_profile_sha256(),
            'entity-registry': k._stable_digest(self.entities), 'relation-registry': k._stable_digest(self.relations),
            'normalization': k._stable_digest(self.graph['normalization_binding']),
            'nonparticipating-profile': k._stable_digest({'philosophy': {}, 'corpus_without_navigation': {}})},
            source_publication=self.snapshot.header['source_publication']['token'])
        self.root_files = {role: view.namespace_path.read_bytes() for role, view in roots.items()
                           if view.namespace_path.exists()}
        self.graph['source_revision'] = self.source.value()['source_revision']
        self.catalog = memory_catalog(self.graph, self.corpus, {}, self.entities, self.relations)
        self.inputs = CatalogInputs.from_graph(self.graph, self.corpus, {}, self.entities, self.relations,
                                              source_order_profile=CANONICAL_ORDER)
        self.path = self.root / 'derived/prepared.sqlite'
        receipt = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        self.binding = receipt
        self.db = sqlite3.connect(self.path)
        self.addCleanup(self.db.close)
        self.db.execute('PRAGMA journal_mode=WAL')
        self.db.execute('BEGIN IMMEDIATE')
        bootstrap_prepared_maintenance_transaction(self.db, expected_binding=self.binding,
            inputs=self.inputs, ordered_rows=lambda kind: iter(self.graph[kind + 's']))
        bootstrap_prepared_source_inputs_transaction(self.db, expected_binding=self.binding, inputs=self.source)
        reader = assembly.BibliographicClaimAssembler(self.root, catalog_snapshot=self.snapshot)
        declarations = []
        for _, row in self.snapshot.view.iter_items('claims'):
            selected = self.snapshot.get_claim(row['claim_id'])
            projected = reader.assemble(row['claim_id'], expected_row_sha256=selected.row_sha256)
            declarations.append(SourceClaimDependencies(claim_id=row['claim_id'], source_entry=selected.entry,
                input_sha256=selected.entry['claim_sha256'], dependencies=projected.dependencies))
        bootstrap_source_dependency_index_transaction(self.db, expected_binding=self.binding,
            source_inputs_sha256=self.source.digest, declaration_profile_sha256=self.profile,
            claims=declarations, progress_owner=self.owner)
        publication.bootstrap_agent_context_index_transaction(self.db, source_root=self.root, expected_binding=self.binding,
            source_inputs=self.source, catalog_inputs=self.inputs, declaration_profile_sha256=self.profile,
            ordered_nodes=self.graph['nodes'], ordered_relations=self.graph['relations'],
            source_dossier_refs=[], progress_owner=self.owner)
        self.db.commit()

    def full_build(self):
        with patch.object(navigation, 'REPO_ROOT', self.root), patch.object(navigation, 'TOS_ROOT', self.root / 'ToS'):
            corpus = {'source_navigation': navigation.build_source_navigation([], catalog_snapshot=self.snapshot)}
        bib = bibliography.build_payload(self.root)
        return corpus, bib, k.build_knowledge_graph(corpus, {}, bib, self.entities, self.relations)

    def capture(self, **kwargs):
        return publication.capture_agent_correction(self.db, source_root=self.root,
            record_id=self.helper.fixture.identity, expected_binding=self.binding,
            catalog_inputs=self.inputs, declaration_profile_sha256=self.profile, progress_owner=self.owner, **kwargs)

    def test_real_agent_revision_publishes_all_lanes_and_preserves_old_reader_until_commit(self):
        captured = self.capture()
        transaction, token = self.helper.fixture.revise('Новая заметка реальной тестовой команды')
        original = PublishedKnowledgeReadModel(self.path, self.binding)
        old_catalog = original.catalog()
        with publication.agent_correction_publication(captured, transaction_id=transaction,
                expected_source_token=token, progress_owner=self.owner) as candidate:
            self.db.execute('BEGIN IMMEDIATE')
            with patch.object(k, 'build_knowledge_graph', side_effect=AssertionError('full-build fallback')), \
                    patch.object(ProjectionSnapshotView, 'materialize', side_effect=AssertionError('whole-root fallback')):
                result = candidate.apply_transaction(self.db)
            self.assertEqual(original.catalog(), old_catalog)
            with closing(sqlite3.connect(self.path)) as independent:
                self.assertEqual(independent.execute('SELECT sha256 FROM prepared_source_state').fetchone()[0],
                                 self.source.digest)
            self.assertFalse(result['prepared_committed'])
            self.assertEqual(result['reverse_dependent_claims'], 2)
            observation = candidate.result
            observation['binding']['source_revision'] = 'f' * 64
            self.assertEqual(candidate.result, result)
            committed = candidate.commit_transaction(self.db)
        self.assertTrue(committed['prepared_committed'])
        with self.assertRaises(PublishedSnapshotConflict):
            original.catalog()
        # Independent full tiny oracle is outside the incremental execution.
        self.helper.fixture.rebuild()
        self.snapshot = candidate.catalog
        corpus, bib, expected = self.full_build()
        self.assertEqual(result['semantic_report'], expected['counts']['semantic_validation'])
        expected.update(result['source_header'])
        actual = PublishedKnowledgeReadModel(self.path, result['binding'])
        self.assertEqual(actual.catalog(), memory_catalog(expected, corpus, {}, self.entities, self.relations))
        for kind in ('node', 'relation'):
            rows = {identity: json.loads(raw) for identity, raw in self.db.execute('SELECT id,json FROM knowledge_' + kind + 's')}
            self.assertEqual(rows, {row['id']: row for row in expected[kind + 's']})
        from tos_access.prepared_source_binding import PreparedSourceInputs
        after_source = PreparedSourceInputs.parse(self.db.execute('SELECT inputs FROM prepared_source_state').fetchone()[0].encode())
        self.assertEqual(after_source.value()['source_revision'], result['binding']['source_revision'])
        for role, raw in self.root_files.items():
            self.assertEqual(after_source.roots()[role].namespace_path.read_bytes(), raw)
        for role, source in (('source-navigation', corpus['source_navigation']), ('bibliographic-claims', bib)):
            material = after_source.roots()[role].materialize()
            for field, key in publication.RAW_COLLECTIONS[role].items():
                self.assertEqual(sorted(material[field], key=lambda row: row[key]), sorted(source[field], key=lambda row: row[key]))
        self.assertEqual(PublishedSearchService(actual).search('Новая заметка', limit=10)['nodes'],
                         k.search_knowledge_graph(expected, 'Новая заметка', limit=10)['nodes'])
        spec = lens(seed={'node_ids': ['source-claims:identity:' + self.helper.fixture.identity]})
        self.assertEqual(PublishedLensService(actual).execute(spec), k.execute_knowledge_lens(expected, spec))
        untouched = 'source-navigation:' + self.helper.fixture.other['record_id']
        self.assertEqual(next(row for row in expected['nodes'] if row['id'] == untouched),
                         next(row for row in self.graph['nodes'] if row['id'] == untouched))
        print(json.dumps({'agent_source_transition_measurement': {
            'before_nodes': len(self.graph['nodes']), 'before_relations': len(self.graph['relations']),
            'after_nodes': len(expected['nodes']), 'after_relations': len(expected['relations']),
            **{key: result[key] for key in ('reverse_dependent_claims', 'normalized_node_count',
                'normalized_relation_count', 'changed_nodes', 'changed_relations', 'sql_mutations')},
            'semantic_report_valid': result['semantic_report']['valid'], 'all_full_oracle_lanes_match': True}}))

    def test_late_all_lane_failure_rolls_back_prepared_but_source_command_stays_committed(self):
        captured = self.capture()
        transaction, token = self.helper.fixture.revise()
        before = list(self.db.iterdump())
        joined = publication.apply_dependency_bound_prepared_delta_transaction
        def refuse(*args, **kwargs):
            joined(*args, **kwargs)
            raise RuntimeError('injected after all prepared lanes')
        with self.assertRaisesRegex(RuntimeError, 'after all prepared lanes'):
            with publication.agent_correction_publication(captured, transaction_id=transaction,
                    expected_source_token=token, progress_owner=self.owner) as candidate:
                self.db.execute('BEGIN IMMEDIATE')
                with patch.object(publication, 'apply_dependency_bound_prepared_delta_transaction', side_effect=refuse):
                    candidate.apply_transaction(self.db)
        self.assertTrue(self.db.in_transaction)
        self.db.rollback()
        self.assertEqual(list(self.db.iterdump()), before)
        from source_metadata_snapshot import PublicationSnapshot
        self.assertEqual(PublicationSnapshot(self.root).token, token)
        # Same captured transition can be retried; no source command is rerun.
        with publication.agent_correction_publication(captured, transaction_id=transaction,
                expected_source_token=token, progress_owner=self.owner) as candidate:
            self.db.execute('BEGIN IMMEDIATE')
            candidate.apply_transaction(self.db)
            self.assertTrue(candidate.commit_transaction(self.db)['prepared_committed'])

    def test_stale_prepared_and_live_source_guards_refuse_without_partial_commit(self):
        captured = self.capture()
        transaction, token = self.helper.fixture.revise()
        before = list(self.db.iterdump())
        with self.assertRaisesRegex(ValueError, 'publication differs'):
            with publication.agent_correction_publication(captured, transaction_id=transaction,
                    expected_source_token=token, progress_owner=self.owner) as candidate:
                self.db.execute('BEGIN IMMEDIATE')
                self.db.execute('UPDATE knowledge_exploration_clock SET epoch=epoch+1')
                candidate.apply_transaction(self.db)
        self.db.rollback()
        self.assertEqual(list(self.db.iterdump()), before)
        with self.assertRaisesRegex(ValueError, 'caller wrote after verified publication'):
            with publication.agent_correction_publication(captured, transaction_id=transaction,
                    expected_source_token=token, progress_owner=self.owner) as candidate:
                self.db.execute('BEGIN IMMEDIATE')
                candidate.apply_transaction(self.db)
                self.db.execute('UPDATE knowledge_exploration_clock SET epoch=epoch')
                candidate.commit_transaction(self.db)
        self.db.rollback()
        self.assertEqual(list(self.db.iterdump()), before)
        path = self.helper.fixture.fixture.path
        current = path.read_bytes()
        try:
            with self.assertRaises(ValueError):
                with publication.agent_correction_publication(captured, transaction_id=transaction,
                        expected_source_token=token, progress_owner=self.owner) as candidate:
                    self.db.execute('BEGIN IMMEDIATE')
                    candidate.apply_transaction(self.db)
                    path.write_bytes(current + b'\n')
                    candidate.commit_transaction(self.db)
        finally:
            path.write_bytes(current)
            self.db.rollback()
        self.assertEqual(list(self.db.iterdump()), before)

    def test_missing_reverse_declaration_refuses_before_any_source_command(self):
        self.db.execute('DELETE FROM source_dependency_claims WHERE claim_id=?', (self.hidden_claim['claim_id'],))
        self.db.commit()
        with self.assertRaises(ValueError):
            self.capture()
        self.assertFalse(self.db.in_transaction)
        from source_metadata_snapshot import PublicationSnapshot
        self.assertEqual(PublicationSnapshot(self.root).token, self.source.value()['source_publication'])

    def test_late_schema_change_refuses_guarded_commit_and_preserves_predecessor(self):
        captured = self.capture()
        transaction, token = self.helper.fixture.revise()
        before = list(self.db.iterdump())
        with self.assertRaisesRegex(ValueError, 'schema changed after verified publication'):
            with publication.agent_correction_publication(captured, transaction_id=transaction,
                    expected_source_token=token, progress_owner=self.owner) as candidate:
                self.db.execute('BEGIN IMMEDIATE')
                candidate.apply_transaction(self.db)
                mutations = self.db.total_changes
                self.db.execute('DROP INDEX knowledge_relations_to_seek')
                self.assertEqual(self.db.total_changes, mutations)
                candidate.commit_transaction(self.db)
        self.db.rollback()
        self.assertEqual(list(self.db.iterdump()), before)

    def test_limits_and_profile_mismatch_refuse_capture_without_source_revision(self):
        with self.assertRaisesRegex(ValueError, 'fanout budget'):
            self.capture(limits=publication.AgentPublicationLimits(max_claims=1))
        previous = publication.execution_profile_sha256
        with patch.object(publication, 'execution_profile_sha256', return_value='f' * 64):
            with self.assertRaisesRegex(ValueError, 'profile differs'):
                self.capture()
        self.assertNotEqual(previous(), 'f' * 64)

    def test_root_vector_excludes_absolute_namespaces_and_refuses_legacy_revision(self):
        roots = self.source.roots()
        relocated = {role: ProjectionSnapshotView(view.root_bytes, self.root / 'relocated' / view.namespace_path.name)
                     for role, view in roots.items()}
        other = publication.source_vector_inputs(roots=relocated,
            dependencies=self.source.value()['dependencies'], source_publication=self.source.value()['source_publication'])
        self.assertEqual(other.value()['source_revision'], self.source.value()['source_revision'])
        self.assertNotEqual(other.digest, self.source.digest)
        from tos_access.prepared_source_binding import PreparedSourceInputs
        foreign = PreparedSourceInputs(source_revision='a' * 64,
            roots=roots, dependencies=self.source.value()['dependencies'],
            source_publication=self.source.value()['source_publication'])
        with self.assertRaises(ValueError):
            publication._require_vector(foreign)

    def test_missing_context_member_and_physical_seek_index_require_rollback(self):
        captured = self.capture()
        transaction, token = self.helper.fixture.revise()
        before = list(self.db.iterdump())
        for sql in ('DELETE FROM agent_context_refs', 'DROP INDEX knowledge_relations_to_seek',
                    'CREATE TRIGGER bad_context AFTER UPDATE ON agent_context_state BEGIN DELETE FROM knowledge_nodes; END'):
            with self.subTest(sql=sql), self.assertRaises(ValueError):
                with publication.agent_correction_publication(captured, transaction_id=transaction,
                        expected_source_token=token, progress_owner=self.owner) as candidate:
                    self.db.execute('BEGIN IMMEDIATE')
                    self.db.execute(sql)
                    candidate.apply_transaction(self.db)
            self.db.rollback()
            self.assertEqual(list(self.db.iterdump()), before)


if __name__ == '__main__':
    unittest.main()

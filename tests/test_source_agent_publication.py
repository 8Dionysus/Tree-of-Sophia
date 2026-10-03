"""Real tiny owner revision and all-lane prepared publication, no live corpus."""
from contextlib import closing
import copy
from dataclasses import dataclass
import hashlib
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
import source_commands as commands
import bibliographic_claim_assembler as assembly
from source_catalog_projection import SourceSlotLimits
import build_source_witness_catalog as legacy
import source_witness_bibliographic_graph_common as bibliography
import tos_corpus_index_common as navigation
from tos_access import knowledge as k
from tos_access.catalog_semantics import CatalogInputs, CANONICAL_ORDER, memory_catalog
from tos_access.projection_store import Collection, ProjectionReader, write_projection, canonical_bytes
from tos_access.projection_mutation import ProjectionSnapshotView, MutationLimits
from tos_access.prepared_publication import publish_prepared
from tos_access.prepared_semantics import bootstrap_prepared_maintenance_transaction
from tos_access.prepared_source_binding import bootstrap_prepared_source_inputs_transaction
from tos_access.prepared_source_dependencies import (SourceClaimDependencies, ProgressHandlerOwner,
    bootstrap_source_dependency_index_transaction)
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
from tos_access.published_search import PublishedSearchService
from tos_access.published_lens import PublishedLensService
from test_indexed_lens import lens


@dataclass(frozen=True)
class PublicationWorkload:
    """Additional genuine predecessor material; never an admission profile.

    The addressed Claim addition remains unchanged. Extra Claims refer only
    to extra records. The whole receipt must still verify the same affected U;
    source parameter counts do not substitute for that measured assertion.
    History counts real record.revise operations per extra record, not epochs.
    """
    records: int = 0
    claims: int = 0
    history: int = 0
    skew: str = 'uniform'

    def validate(self):
        for name in ('records', 'claims', 'history'):
            value = getattr(self, name)
            if type(value) is not int or value < 0:
                raise ValueError('publication workload exact nonnegative integers')
        # Existing fixture/slot envelopes remain authoritative; these are only
        # early source construction ceilings, including baseline/addition
        # headroom, not a runtime permission.
        if self.records > 8189 or self.claims > 4093 or self.history > 4096:
            raise ValueError('publication workload construction ceiling')
        if self.skew not in ('uniform', 'hub'):
            raise ValueError('publication workload skew')
        if (self.claims and self.records < 2) or (self.history and not self.records):
            raise ValueError('publication workload independent record cohort required')


def extend_publication_predecessor(helper, claim, workload):
    """Run before the maintained rebuild/bootstrap/full normalization.

    Reuses complete Agent metadata, actual selected owner forms and retained
    revision writer. Claims retain the maintained synthetic-evidence posture;
    this creates neither historical authority nor an empty normalized row.
    """
    workload.validate()
    if workload == PublicationWorkload():
        return  # Default fixture bytes and operation schedule are unchanged.
    fixture = helper.fixture
    revision = fixture.fixture
    identities = []
    for index in range(workload.records):
        record = copy.deepcopy(fixture.other)
        identity = f'tos.agent.publication-working-set-{index:06d}'
        record.update(record_id=identity, record_version=1,
                      preferred_label=f'Independent synthetic Agent {index}',
                      notes='Synthetic working-set predecessor; no historical assertion.')
        record['external_identifiers'] = [dict(value, value=f'working-set-{index:06d}-{ordinal}')
                                         for ordinal, value in enumerate(record['external_identifiers'])]
        relative = f'ToS/source-witnesses/agents/publication-working-set-{index:06d}/agent.json'
        fixture.write(relative, canonical_bytes(record))
        selections = [dict(selection, form_id=selection['form_id'] + f'.working-set-{index:06d}')
                      for selection in revision.selections]
        changes = [commands.prepare_metadata_change(record, None, 'test:author', **selection)
                   for selection in selections]
        forms = commands._apply(None, commands.Record.from_payload(identity, 1, record), changes)
        fixture.write(str(Path(relative).with_name('agent.human-forms.json')), canonical_bytes(forms))
        owner = fixture.root / f'working-set-{index:06d}-owner.json'
        config = {**revision.config, 'source_path': relative, 'record_id': identity,
                  'allowed_form_ids': [selection['form_id'] for selection in selections]}
        owner.write_bytes(canonical_bytes(config))
        owner.chmod(0o600)
        predecessors = []
        for version in range(workload.history):
            proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
                        'fields': {'notes': f'Synthetic retained correction {version + 1} of Agent {index}.'},
                        'forms': selections, 'reason': 'Synthetic retained working-set history.'}
            preview = commands.run_legacy_oracle_command(owner, proposal)
            package = {name: (fixture.root / relative).with_name(name).read_bytes()
                       for name in ('agent.json', 'agent.human-forms.json')}
            predecessors.append((preview['source'], package))
            commands.run_legacy_oracle_command(owner, {**proposal, 'operation': 'record.revise',
                'command_id': f'synthetic:working-set-{index:06d}-revision-{version:06d}',
                'expected_source': preview['source'], 'expected_revision': preview['revision'],
                'expected_configuration': preview['owner_configuration'],
                'expected_dependencies': preview['expected_dependencies'],
                'expected_publication': preview['publication_snapshot']})
        # Reopen every exact predecessor after the entire history is written;
        # path presence alone cannot establish intact archives or membership.
        for source, package in predecessors:
            prior = commands.run_legacy_oracle_command(owner, {
                'schema_version': 'tos_local_source_command_v1',
                'operation': 'inspect-version', 'source': source})
            if prior['record']['record_id'] != identity:
                raise ValueError('working-set archived identity differs')
            for name, expected_raw in package.items():
                archived = fixture.root / prior['files'][name]['archive_path']
                if archived.read_bytes() != expected_raw:
                    raise ValueError('working-set archived exact predecessor differs')
        identities.append(identity)
    rows, events = [], []
    for index in range(workload.claims):
        subject = identities[0 if workload.skew == 'hub' else index % len(identities)]
        target = identities[(index + 1) % len(identities)]
        if target == subject:
            target = identities[1]
        row = copy.deepcopy(claim)
        event = copy.deepcopy(helper.event)
        event['event_id'] = f'tos.event.publication-working-set-{index:06d}'
        row.update(claim_id=f'tos.claim.publication-working-set-{index:06d}',
                   subject_ref=subject, object=target, provenance_event_ref=event['event_id'])
        row['qualifiers']['statement'] = f'Synthetic independent predecessor relationship {index}; not history.'
        event['outputs'] = [{'ref': row['claim_id'], 'role': 'synthetic-claim-annotation'}]
        rows.append(canonical_bytes(row))
        events.append(canonical_bytes(event))
    if rows:
        fixture.write('ToS/source-witnesses/relations/publication-working-set/source-claims.jsonl', b''.join(rows))
        fixture.write('ToS/source-witnesses/relations/publication-working-set/provenance.jsonl', b''.join(events))


class SourceAgentPublicationTests(unittest.TestCase):
    workload = PublicationWorkload()

    def setUp(self):
        self.f = fixtures.BibliographicClaimAssemblerTests()
        self.addCleanup(self.f.doCleanups)
        self.helper, self.claim, ref = self.f.native_claim_fixture()
        # The publication fixture consumes a schema-real provenance event.
        # Preserve the slot fixture's uninterpreted marker inside the allowed
        # method configuration, without making it an identity lookup.
        self.helper.event.setdefault('inputs', [])
        self.helper.event.setdefault('outputs', [
            {'ref': self.claim['claim_id'], 'role': 'synthetic-claim-annotation'},
        ])
        configuration = self.helper.event['method'].setdefault('configuration', {})
        if 'unknown' in self.helper.event:
            configuration['uninterpreted_fixture'] = self.helper.event.pop('unknown')
        self.helper.fixture.write(self.helper.event_ref, b'\r\n' + canonical_bytes(self.helper.event))
        self.hidden_claim = {**copy.deepcopy(self.claim), 'claim_id': 'tos.claim.hidden-maker-fixture',
            'subject_ref': self.helper.fixture.other['record_id'], 'object': self.helper.fixture.other['record_id'],
            'maker': {'maker_type': 'software', 'agent_ref': self.helper.fixture.identity}}
        # Only one schema-real social Claim; no invalid authored-by Agent fixture.
        self.helper.fixture.write(self.helper.claim_ref, b'')
        self.helper.fixture.write(ref, canonical_bytes(self.claim) + canonical_bytes(self.hidden_claim))
        extend_publication_predecessor(self.helper, self.claim, self.workload)
        self.helper.fixture.rebuild()
        self.root = self.helper.root
        self.snapshot = self.helper.snapshot(self.helper.bootstrap())
        # Verify actual addressed catalog membership after its real bootstrap.
        for index in range(self.workload.records):
            identity = f'tos.agent.publication-working-set-{index:06d}'
            relative = f'ToS/source-witnesses/agents/publication-working-set-{index:06d}/agent.json'
            selected = self.snapshot.get(identity)
            current_raw = (self.root / relative).read_bytes()
            current_ref = commands.metadata_subject(json.loads(current_raw)).ref
            if (selected.entry['record_id'] != identity
                    or selected.source['record_ref']['version'] != self.workload.history + 1
                    or selected.source['record_ref'] != current_ref
                    or 'sha256:' + selected.entry['record_sha256'] != current_ref['digest']
                    or selected.source['raw_sha256'] != hashlib.sha256(current_raw).hexdigest()
                    or selected.source['raw_bytes'] != len(current_raw)):
                raise ValueError('working-set catalog current record differs')
        for index in range(self.workload.claims):
            identity = f'tos.claim.publication-working-set-{index:06d}'
            if self.snapshot.get_claim(identity).entry['claim_id'] != identity:
                raise ValueError('working-set catalog Claim membership differs')
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

    def test_addressing_extension_keeps_context_and_following_real_agent_correction(self):
        from authored_corpus_source_read import bootstrap_authored_source_read_transaction
        path = self.root / 'derived' / 'authored-addressing.json'
        tables = ('agent_context_nodes', 'agent_context_refs', 'agent_context_heads', 'source_dependency_claims')
        before = {name: self.db.execute('SELECT * FROM ' + name).fetchall() for name in tables}
        self.db.execute('BEGIN IMMEDIATE')
        result = bootstrap_authored_source_read_transaction(self.db, source_root=self.root, output=path,
            corpus_index={'relation_packs': [], 'relation_edges': []},
            expected_binding=self.binding, before_source_inputs=self.source, before_inputs=self.inputs,
            progress_owner=self.owner, work_dir=self.root / 'scratch')
        self.assertTrue(result['agent_context_selection_paired'])
        self.assertTrue(result['execution_profile_current'])
        self.assertTrue(result['source_root_admission_verified'])
        self.assertEqual(result['prepared_csv_rows_verified'], 0)
        extended = publication.read_prepared_source_inputs_transaction(self.db, expected_binding=result['binding'])
        view = extended.roots()['authored-corpus']
        header = self.inputs.header
        header['source_revision'] = extended.value()['source_revision']
        next_inputs = CatalogInputs(header, self.entities, self.relations, self.inputs.lenses,
                                    source_order_profile=CANONICAL_ORDER)
        for name, rows in before.items():
            self.assertEqual(self.db.execute('SELECT * FROM ' + name).fetchall(), rows)
        self.db.commit()
        self.binding, self.source, self.inputs = result['binding'], extended, next_inputs
        captured = self.capture()
        transaction, token = self.helper.fixture.revise('Agent correction after address extension')
        with publication.agent_correction_publication(captured, transaction_id=transaction,
                expected_source_token=token, progress_owner=self.owner) as candidate:
            self.db.execute('BEGIN IMMEDIATE')
            corrected = candidate.apply_transaction(self.db)
            candidate.commit_transaction(self.db)
        self.assertEqual(corrected['reverse_dependent_claims'], 2)
        self.db.execute('BEGIN')
        retained = publication.read_prepared_source_inputs_transaction(self.db, expected_binding=corrected['binding'])
        self.assertEqual(retained.roots()['authored-corpus'], view)
        self.db.rollback()
        self.assertTrue(PublishedKnowledgeReadModel(self.path, corrected['binding']).catalog())

    def test_real_agent_revision_publishes_all_lanes_and_preserves_old_reader_until_commit(self):
        publication_limits = publication.PublicationLimits(max_bytes=128 * 1024 * 1024)
        original_read = publication.read_prepared_source_inputs_transaction
        reader_patch = patch.object(publication, 'read_prepared_source_inputs_transaction', wraps=original_read)
        reader = reader_patch.start()
        self.addCleanup(reader_patch.stop)
        captured = self.capture(publication_limits=publication_limits)
        transaction, token = self.helper.fixture.revise('Новая заметка реальной тестовой команды')
        original = PublishedKnowledgeReadModel(self.path, self.binding)
        old_catalog = original.catalog()
        with publication.agent_correction_publication(captured, transaction_id=transaction,
                expected_source_token=token, progress_owner=self.owner) as candidate:
            self.db.execute('BEGIN IMMEDIATE')
            with patch.object(k, 'build_knowledge_graph', side_effect=AssertionError('full-build fallback')), \
                    patch.object(ProjectionSnapshotView, 'materialize', side_effect=AssertionError('whole-root fallback')):
                result = candidate.apply_transaction(self.db, publication_limits=publication_limits)
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
        self.assertEqual(len(reader.call_args_list), 3)
        self.assertTrue(all(call.kwargs['limits'] == publication_limits for call in reader.call_args_list))
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

    def test_full_context_accepts_explicit_budgets_without_widening_correction_defaults(self):
        self.assertEqual(assembly.ClaimAssemblyLimits().max_claims, 64)
        self.assertEqual(assembly.ClaimAssemblyLimits().max_output_bytes, 16 * 1024 * 1024)
        self.assertEqual(publication.AgentPublicationLimits().max_claims, 64)
        self.assertEqual(publication.AgentPublicationLimits().max_bytes, 16 * 1024 * 1024)

        # The fixture was already bootstrapped once through the omitted-default
        # path in setUp. Rebuild the private context tables and exercise the
        # newly explicit full-only assembler budgets on the same tiny source.
        for table in ('agent_context_heads', 'agent_context_refs',
                      'agent_context_nodes', 'agent_context_state'):
            self.db.execute('DROP TABLE ' + table)
        self.db.commit()
        catalog_limits = MutationLimits(
            max_changes=0, max_input_bytes=64 * 1024 * 1024,
            max_opened_parts=4096, max_stored_read_bytes=64 * 1024 * 1024,
            max_decoded_bytes=64 * 1024 * 1024, max_keys=4096,
            max_written_parts=0, max_written_decoded_bytes=0,
            max_written_stored_bytes=0, max_result_bytes=16 * 1024 * 1024)
        slot_limits = SourceSlotLimits(
            max_source_files=1024, max_read_slots=4096,
            max_read_bytes=64 * 1024 * 1024, max_row_bytes=1024 * 1024,
            max_profile_bytes=8 * 1024 * 1024, max_profile_files=256)
        too_small = assembly.ClaimAssemblyLimits(max_claims=64,
            max_metadata_records=256, max_addressed_lookups=4096,
            max_files=256, max_file_bytes=2 * 1024 * 1024,
            max_read_bytes=64 * 1024 * 1024, max_output_bytes=1)
        self.db.execute('BEGIN IMMEDIATE')
        with self.assertRaises(assembly.ClaimAssemblyBudgetExceeded):
            publication.bootstrap_agent_context_index_transaction(
                self.db, source_root=self.root, expected_binding=self.binding,
                source_inputs=self.source, catalog_inputs=self.inputs,
                declaration_profile_sha256=self.profile,
                ordered_nodes=self.graph['nodes'], ordered_relations=self.graph['relations'],
                source_dossier_refs=[], progress_owner=self.owner,
                catalog_read_limits=catalog_limits, assembly_limits=too_small,
                slot_limits=slot_limits)
        self.db.rollback()

        full_limits = assembly.ClaimAssemblyLimits(max_claims=64,
            max_metadata_records=256, max_addressed_lookups=4096,
            max_files=256, max_file_bytes=2 * 1024 * 1024,
            max_read_bytes=64 * 1024 * 1024, max_output_bytes=32 * 1024 * 1024)
        self.db.execute('BEGIN IMMEDIATE')
        dependency_limits = publication.SourceDependencyLimits(max_bytes=256 * 1024 * 1024)
        with patch.object(publication, 'read_prepared_source_inputs_transaction',
                wraps=publication.read_prepared_source_inputs_transaction) as read_binding:
            publication.bootstrap_agent_context_index_transaction(
                self.db, source_root=self.root, expected_binding=self.binding,
                source_inputs=self.source, catalog_inputs=self.inputs,
                declaration_profile_sha256=self.profile,
                ordered_nodes=self.graph['nodes'], ordered_relations=self.graph['relations'],
                source_dossier_refs=[], progress_owner=self.owner, limits=dependency_limits,
                catalog_read_limits=catalog_limits, assembly_limits=full_limits,
                slot_limits=slot_limits)
        self.assertEqual(read_binding.call_args.kwargs['limits'].max_bytes, dependency_limits.max_bytes)
        self.assertEqual(self.db.execute(
            'SELECT count(*) FROM agent_context_state').fetchone()[0], 1)
        self.db.commit()

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


def test_reviewed_execution_profile_bootstrap_preserves_rows_and_following_command():
    """Storage/command behavior; a fixture review ref is not compatibility proof."""
    from dataclasses import replace
    import pytest
    fixture = SourceAgentPublicationTests()
    old_profile = 'a' * 64
    with patch.object(publication, 'execution_profile_sha256', return_value=old_profile):
        fixture.setUp()
    try:
        current_profile = publication.execution_profile_sha256()
        db = fixture.db
        before = list(db.iterdump())
        def migrate(**overrides):
            options = dict(expected_binding=fixture.binding, before_source_inputs=fixture.source,
                before_inputs=fixture.inputs, reviewed_before_profile_sha256=old_profile,
                reviewed_after_profile_sha256=current_profile, compatibility_review_ref='test-only:reviewed-pair',
                progress_owner=fixture.owner)
            options.update(overrides)
            return publication.bootstrap_reviewed_agent_execution_profile_transaction(db, **options)
        with pytest.raises(ValueError, match='profile'):
            fixture.capture()
        for bad in ({'reviewed_before_profile_sha256': 'b' * 64},
                    {'reviewed_after_profile_sha256': 'b' * 64}, {'compatibility_review_ref': ''}):
            db.execute('BEGIN IMMEDIATE')
            with pytest.raises(ValueError, match='review'):
                migrate(**bad)
            db.rollback()
            assert list(db.iterdump()) == before
        original = PublishedKnowledgeReadModel(fixture.path, fixture.binding)
        original_catalog = original.catalog()
        db.execute('BEGIN IMMEDIATE')
        result = migrate()
        assert original.catalog() == original_catalog
        assert result['execution_profile_current'] and not result['compatibility_verified_by_helper']
        assert result['normalized_row_changes_supplied'] == 0
        required = result['sql_mutations']
        db.rollback()
        assert list(db.iterdump()) == before
        db.execute('BEGIN IMMEDIATE')
        with pytest.raises(ValueError, match='budget'):
            migrate(publication_limits=replace(publication.PublicationLimits(), max_mutations=required - 1))
        db.rollback()
        assert list(db.iterdump()) == before
        tables = ('knowledge_nodes', 'knowledge_relations', 'source_dependency_claims',
                  'agent_context_nodes', 'agent_context_refs', 'agent_context_heads')
        bodies = {name: db.execute('SELECT * FROM ' + name).fetchall() for name in tables}
        db.execute('BEGIN IMMEDIATE')
        result = migrate(publication_limits=replace(publication.PublicationLimits(), max_mutations=required))
        after = publication.read_prepared_source_inputs_transaction(db, expected_binding=result['binding'])
        assert after.roots() == fixture.source.roots()
        assert after.value()['source_publication'] == fixture.source.value()['source_publication']
        assert after.value()['dependencies'] == {**fixture.source.value()['dependencies'],
                                               'agent-publication-profile': current_profile}
        for name, rows in bodies.items():
            assert db.execute('SELECT * FROM ' + name).fetchall() == rows
        db.commit()
        with pytest.raises(PublishedSnapshotConflict):
            original.catalog()
        fixture.binding, fixture.source = result['binding'], after
        fixture.inputs = CatalogInputs(result['source_header'], fixture.entities, fixture.relations,
            fixture.inputs.lenses, source_order_profile=CANONICAL_ORDER)
        captured = fixture.capture()
        transaction, token = fixture.helper.fixture.revise('Correction after reviewed execution bootstrap')
        with publication.agent_correction_publication(captured, transaction_id=transaction,
                expected_source_token=token, progress_owner=fixture.owner) as candidate:
            db.execute('BEGIN IMMEDIATE')
            changed = candidate.apply_transaction(db)
            candidate.commit_transaction(db)
        assert changed['reverse_dependent_claims'] == 2
        assert PublishedKnowledgeReadModel(fixture.path, changed['binding']).catalog()
    finally:
        fixture.doCleanups()


if __name__ == '__main__':
    unittest.main()

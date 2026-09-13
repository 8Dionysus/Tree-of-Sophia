"""Small real-source addressed assembly; synthetic claims are not admissions."""
from dataclasses import replace
import copy
import hashlib
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
for directory in (ROOT / 'scripts', ROOT / 'access/src', ROOT / 'tests',
                  ROOT / 'mechanics/growth-cycle/tests'):
    sys.path.insert(0, str(directory))

import test_source_witness_bibliographic_graph as graph_fixtures
import test_source_catalog_slots as slot_fixtures
import bibliographic_claim_assembler as assembly
import claim_revisions
import source_revisions as claim_packages
import source_catalog_projection as catalog
import build_source_witness_catalog as legacy
import source_witness_bibliographic_graph_common as graph
from source_metadata_snapshot import PublicationSnapshot
from tos_access.projection_store import ProjectionReader, canonical_bytes


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


class BibliographicClaimAssemblerTests(unittest.TestCase):
    def fixture(self):
        helper = graph_fixtures.SourceWitnessBibliographicGraphTest()
        context = helper.historical_fixture()
        root, history, real, claims, rebuild = context.__enter__()
        self.addCleanup(context.__exit__, None, None, None)
        return root, history, real, claims, rebuild

    def bootstrap(self, root):
        scratch = root / 'assembly-scratch'
        scratch.mkdir(exist_ok=True)
        output = root / 'assembled/catalog.json'
        output.parent.mkdir(exist_ok=True)
        candidate = catalog.bootstrap_source_catalog(root, output, catalog_namespace='tos.catalog.synthetic.assembler',
            expected_manifest_sha256=sha((root / graph.CATALOG_MANIFEST_REF).read_bytes()),
            expected_publication_token=PublicationSnapshot(root).token, work_dir=scratch,
            include_claims=True, target_part_bytes=512)
        return catalog.SourceCatalogSnapshot(candidate.snapshot(), expected_root_sha256=candidate.root_sha256,
                                             trusted_baseline_sha256=candidate.root_sha256)

    def capture(self, rebuild):
        inputs = []
        render = graph.project_bibliographic_claim
        def project(value):
            inputs.append(copy.deepcopy(value))
            return render(value)
        with patch.object(graph, 'project_bibliographic_claim', side_effect=project):
            result = rebuild()
        return result, inputs

    def test_exact_raw_cohorts_and_dependencies_match_full_builder_without_global_reads(self):
        root, _history, real, claims, rebuild = self.fixture()
        claims[0]['evidence_refs'] = [real[0]['record_id'], 'https://example.org/synthetic']
        claims[0]['counterevidence_refs'] = claims[1]['evidence_refs']
        claims[0]['alternative_claim_refs'] = [claims[2]['claim_id'], claims[1]['claim_id']]
        claims[0]['supersedes_claim_ref'] = claims[2]['claim_id']
        _, expected = self.capture(rebuild)
        snapshot = self.bootstrap(root)
        rows = [snapshot.get_claim(claim['claim_id']) for claim in claims]
        with patch.object(Path, 'rglob', side_effect=AssertionError('source discovery')), \
                patch.object(Path, 'read_bytes', side_effect=AssertionError('whole file API')), \
                patch.object(ProjectionReader, 'iter_items', side_effect=AssertionError('catalog scan')), \
                patch.object(graph, '_load_source_claim', side_effect=AssertionError('full Claim lookup')), \
                patch.object(graph, '_scan_index', side_effect=AssertionError('whole event scan')):
            reader = assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot)
            actual = [reader.assemble(row.entry['claim_id'], expected_row_sha256=row.row_sha256) for row in rows]
            reader.verify_current()
        for item, expected_input in zip(actual, expected):
            self.assertEqual(item.project(), graph.project_bibliographic_claim(expected_input))
            self.assertEqual(item.dependencies, graph.enumerate_bibliographic_claim_dependencies(expected_input))
            self.assertFalse(item.bindings['reverse_claim_closure_verified'])
            self.assertFalse(item.bindings['source_admission'])
            self.assertFalse(item.bindings['historical_claim_transport'])
        self.assertEqual(actual[0].inputs.source_claim['qualifiers']['x-unknown'], False)
        self.assertEqual(actual[0].inputs.maker_node['source_sha256'],
                         snapshot.header['legacy_baseline']['files'][graph.CLAIM_CATALOG_REF]['sha256'])
        self.assertGreater(actual[0].bindings['accounting']['source_slots']['read_slots'], 0)
        self.assertFalse((root / 'assembled/catalog.json').exists())

    def test_detached_inputs_do_not_poison_reused_assembler(self):
        root, _, _, claims, rebuild = self.fixture()
        rebuild()
        snapshot = self.bootstrap(root)
        reader = assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot)
        row = snapshot.get_claim(claims[0]['claim_id'])
        first = reader.assemble(row.entry['claim_id'], expected_row_sha256=row.row_sha256)
        expected = first.project()
        first.inputs.source_claim['qualifiers']['x-unknown'] = 'caller mutation'
        first.inputs.subject_node['properties']['source_record']['notes'] = 'caller mutation'
        for binding in first.bindings['metadata'].values():
            binding['exact_ref']['version'] = -1
            binding['provenance'].clear()
        second = reader.assemble(row.entry['claim_id'], expected_row_sha256=row.row_sha256)
        self.assertEqual(second.project(), expected)
        self.assertTrue(all(binding['exact_ref']['version'] > 0 and binding['provenance']
                            for binding in second.bindings['metadata'].values()))

    def test_wrong_row_and_selected_source_raw_drift_refuse(self):
        root, _, _, claims, rebuild = self.fixture()
        rebuild()
        snapshot = self.bootstrap(root)
        row = snapshot.get_claim(claims[0]['claim_id'])
        reader = assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot)
        with self.assertRaisesRegex(ValueError, 'row digest'):
            reader.assemble(claims[0]['claim_id'], expected_row_sha256='0' * 64)
        source_path = root / row.entry['source_claim_file_ref']
        source_path.write_bytes(source_path.read_bytes() + b'\n')
        with self.assertRaises(ValueError):
            reader.assemble(claims[0]['claim_id'], expected_row_sha256=row.row_sha256)

    def test_metadata_bytes_and_observed_evidence_change_refuse(self):
        root, _, real, claims, rebuild = self.fixture()
        rebuild()
        snapshot = self.bootstrap(root)
        row = snapshot.get_claim(claims[0]['claim_id'])
        reader = assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot)
        reader.assemble(claims[0]['claim_id'], expected_row_sha256=row.row_sha256)
        path = root / snapshot.get(real[0]['record_id']).entry['source_record_ref']
        path.write_bytes(path.read_bytes() + b'\n')
        with self.assertRaises(ValueError):
            reader.verify_current()
        fresh = assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot)
        with self.assertRaisesRegex(ValueError, 'endpoint unavailable'):
            fresh.assemble(claims[0]['claim_id'], expected_row_sha256=row.row_sha256)

    def test_assembly_bounds_and_unknown_source_reference_fail_without_candidate(self):
        root, _, _, claims, rebuild = self.fixture()
        rebuild()
        snapshot = self.bootstrap(root)
        row = snapshot.get_claim(claims[0]['claim_id'])
        for kwargs in ({'max_claims': 0}, {'max_addressed_lookups': 0},
                       {'max_metadata_records': 0}, {'max_output_bytes': 1}):
            reader = assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot,
                limits=replace(assembly.ClaimAssemblyLimits(), **kwargs))
            with self.assertRaises(assembly.ClaimAssemblyBudgetExceeded):
                reader.assemble(claims[0]['claim_id'], expected_row_sha256=row.row_sha256)
        for kwargs in ({'max_files': 0}, {'max_file_bytes': 1}, {'max_read_bytes': 1}):
            with self.assertRaises(assembly.ClaimAssemblyBudgetExceeded):
                assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot,
                    limits=replace(assembly.ClaimAssemblyLimits(), **kwargs))
        with self.assertRaises(ValueError):
            assembly.ClaimAssemblyLimits(max_claims=True)
        claims[0]['evidence_refs'] = ['tos.anchor.not-present']
        (root / row.entry['source_claim_file_ref']).write_bytes(b''.join(canonical_bytes(claim) for claim in claims))
        legacy.write_outputs(root, legacy.render_outputs(root))
        snapshot = self.bootstrap(root)
        reader = assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot)
        row = snapshot.get_claim(claims[0]['claim_id'])
        with self.assertRaises(ValueError):
            reader.assemble(claims[0]['claim_id'], expected_row_sha256=row.row_sha256)

    def test_public_path_evidence_symlink_is_rejected(self):
        root, _, _, claims, rebuild = self.fixture()
        rebuild()
        snapshot = self.bootstrap(root)
        row = snapshot.get_claim(claims[0]['claim_id'])
        path = root / claims[0]['evidence_refs'][0]
        # This fixture-owned replace has an exact local target, never a host source.
        original = path.with_name('retained-fixture.json')
        path.rename(original)
        path.symlink_to(original.name)
        with self.assertRaises((ValueError, OSError)):
            reader = assembly.BibliographicClaimAssembler(root, catalog_snapshot=snapshot)
            reader.assemble(claims[0]['claim_id'], expected_row_sha256=row.row_sha256)

    def agent_fixture(self):
        helper = slot_fixtures.SourceCatalogSlotTests()
        helper.setUp()
        self.addCleanup(helper.doCleanups)
        helper.event.update(event_id='tos.event.slot-fixture', schema_version='tos_provenance_event_v1', event_type='annotation',
            started_at='2026-09-12T00:00:00Z', ended_at='2026-09-12T00:00:00Z',
            agent_refs=['software:synthetic'], method={'maker_type': 'software', 'name': 'synthetic', 'version': '1'},
            status='completed_with_warnings')
        for claim in (helper.claim, helper.other_claim):
            claim.update(epistemic_status='uncertain', provenance_event_ref=helper.event['event_id'])
            claim['maker']['maker_type'] = 'software'
        helper.fixture.write(helper.claim_ref, b'\n' + canonical_bytes(helper.claim) + canonical_bytes(helper.other_claim))
        helper.fixture.write(helper.event_ref, b'\r\n' + canonical_bytes(helper.event))
        for ref in ('ToS/doctrine/semantic-interchange/relation-types.v1.json',
                    'ToS/contracts/semantic-relation-type-registry.schema.json',
                    'ToS/contracts/claim-packet.schema.json',
                    'ToS/contracts/historical-claim.schema.json',
                    'ToS/contracts/knowledge-assessment.schema.json',
                    'ToS/contracts/human-form.schema.json',
                    'ToS/contracts/human-form-set.schema.json',
                    'ToS/contracts/human-form-template.schema.json',
                    'ToS/contracts/source-witness-bibliographic-graph.schema.json'):
            helper.fixture.write(ref, (ROOT / ref).read_bytes())
        helper.fixture.rebuild()
        return helper

    def test_real_agent_record_forms_and_all_exact_versions_share_one_reader(self):
        helper, source_claim, claim_ref = self.native_claim_fixture()
        helper.fixture.write(claim_ref, canonical_bytes(source_claim))
        helper.fixture.rebuild()
        candidate = helper.bootstrap()
        before = helper.snapshot(candidate)
        row = before.get(helper.fixture.identity)
        reader = assembly.BibliographicClaimAssembler(helper.root, catalog_snapshot=before)
        record = reader.assemble_record(row.entry['record_id'], expected_row_sha256=row.row_sha256)
        claim_row = before.get_claim(source_claim['claim_id'])
        before_claim = reader.assemble(source_claim['claim_id'], expected_row_sha256=claim_row.row_sha256)
        self.assertEqual(record.project_bibliographic(), before_claim.inputs.subject_node)
        self.assertEqual(record.navigation_inputs.source_record, helper.fixture.record)
        self.assertIsNotNone(record.navigation_inputs.forms)
        self.assertEqual(len(record.navigation_inputs.versions), 1)
        with patch.object(Path, 'open', side_effect=AssertionError('pure projection opened a file')):
            navigation, bibliography = record.project_navigation(), record.project_bibliographic()
        self.assertEqual(bibliography['properties']['human_forms'], record.navigation_inputs.forms[2])
        self.assertEqual(navigation.nodes[0]['properties']['source_record'], helper.fixture.record)
        transaction, publication = helper.fixture.revise()
        changed = helper.fixture.transition(before, transaction, publication)
        after = helper.snapshot(changed)
        selected = after.get(row.entry['record_id'])
        current = assembly.BibliographicClaimAssembler(helper.root, catalog_snapshot=after)
        updated = current.assemble_record(row.entry['record_id'], expected_row_sha256=selected.row_sha256)
        after_claim = current.assemble(source_claim['claim_id'], expected_row_sha256=claim_row.row_sha256)
        self.assertEqual(after_claim.inputs.source_claim, before_claim.inputs.source_claim)
        self.assertNotEqual(after_claim.inputs.subject_node, before_claim.inputs.subject_node)
        self.assertEqual(updated.project_bibliographic(), after_claim.inputs.subject_node)
        self.assertEqual([value['version_status'] for _, value in updated.navigation_inputs.versions],
                         ['historical', 'current'])
        self.assertEqual(updated.navigation_inputs.versions[0][1]['record'], helper.fixture.record)
        with self.assertRaises(ValueError):
            reader.verify_current()
        with self.assertRaisesRegex(ValueError, 'row digest'):
            current.assemble_record(row.entry['record_id'], expected_row_sha256=row.row_sha256)

    def test_anchor_exact_source_and_known_maker_material_are_full_native_parity(self):
        helper = self.agent_fixture()
        helper.claim['maker']['agent_ref'] = helper.fixture.identity
        helper.claim['evidence_refs'] = [helper.anchor['anchor_id'], helper.anchor['anchor_id'],
                                       helper.event['event_id'], helper.fixture.identity]
        helper.fixture.write(helper.claim_ref, b'\n' + canonical_bytes(helper.claim))
        helper.fixture.rebuild()
        expected = graph.build_payload(helper.root)
        snapshot = helper.snapshot(helper.bootstrap())
        reader = assembly.BibliographicClaimAssembler(helper.root, catalog_snapshot=snapshot)
        row = snapshot.get_claim(helper.claim['claim_id'])
        result = reader.assemble(helper.claim['claim_id'], expected_row_sha256=row.row_sha256)
        nodes = {value['node_id']: value for value in expected['nodes']}
        for value in result.project().nodes:
            self.assertEqual(value, nodes[value['node_id']])
        self.assertEqual(result.inputs.evidence_nodes[0]['properties']['source_anchor'], helper.anchor)
        self.assertEqual(result.inputs.evidence_nodes[0], result.inputs.evidence_nodes[1])
        self.assertIsNotNone(result.inputs.maker_identity_node)

    def native_claim_fixture(self):
        helper = self.agent_fixture()
        for name in ('source-claim-record', 'social-relation-claim', 'source-member-structure-claim',
                     'source-structured-value', 'scoped-member-structure'):
            ref = 'ToS/contracts/' + name + '.schema.json'
            helper.fixture.write(ref, (ROOT / ref).read_bytes())
        claim = copy.deepcopy(helper.claim)
        claim.pop('reviews')
        unknown = claim.pop('uninterpreted')
        claim.update(schema_version='tos_social_relation_claim_v1', claim_type='relation',
            predicate='learned_from', assertion_layer='scholarly_report', extensions=unknown,
            claim_id='tos.claim.native-assembly-fixture', qualifiers={
                'statement': 'Synthetic relationship, not a historical assertion.',
                'statement_language': 'en', 'statement_script': 'Latn',
                'relation_basis': 'Synthetic unit test only.', 'social_scope': 'Fixture-only relationship.',
                'time_scope_note': 'No historical dates asserted.',
                'uninterpreted': 'tos.agent.not-an-explicit-lookup'})
        ref = 'ToS/source-witnesses/relations/native-assembly/source-claims.jsonl'
        return helper, claim, ref

    def test_native_claim_profile_uses_bounded_shared_schema_reader_and_literal_extensions_are_inert(self):
        helper, claim, ref = self.native_claim_fixture()
        helper.fixture.write(ref, canonical_bytes(claim))
        helper.fixture.rebuild()
        _, expected = self.capture(lambda: graph.build_payload(helper.root))
        snapshot = helper.snapshot(helper.bootstrap())
        row = snapshot.get_claim(claim['claim_id'])
        reader = assembly.BibliographicClaimAssembler(helper.root, catalog_snapshot=snapshot)
        lookups = []
        original = snapshot.lookup
        def lookup(identity):
            lookups.append(identity)
            return original(identity)
        with patch.object(snapshot, 'lookup', side_effect=lookup), \
                patch.object(Path, 'rglob', side_effect=AssertionError('source discovery')):
            result = reader.assemble(claim['claim_id'], expected_row_sha256=row.row_sha256)
        expected_input = next(value for value in expected if value.source_claim['claim_id'] == claim['claim_id'])
        self.assertEqual(result.project(), graph.project_bibliographic_claim(expected_input))
        self.assertNotIn('tos.agent.not-an-explicit-lookup', lookups)
        self.assertIn('ToS/contracts/social-relation-claim.schema.json', result.bindings['files'])
        self.assertEqual(result.inputs.source_claim['extensions'], claim['extensions'])

    def test_collection_order_basis_uses_owner_historical_reader_and_is_projected(self):
        helper, claim, ref = self.native_claim_fixture()
        collection = {**helper.fixture.record, 'record_type': 'collection',
                      'record_id': 'tos.collection.assembly-fixture', 'membership_claim_refs': []}
        work = {**helper.fixture.record, 'record_type': 'work', 'record_id': 'tos.work.assembly-fixture',
                'expression_claim_refs': []}
        helper.fixture.write('ToS/source-witnesses/collections/assembly/collection.json', canonical_bytes(collection))
        helper.fixture.write('ToS/source-witnesses/works/assembly/work.json', canonical_bytes(work))
        claim.update(schema_version='tos_source_member_structure_claim_v1',
            subject_ref=collection['record_id'], predicate='collection_member_order', object={
                'kind': 'collection-member-order', 'members': [work['record_id']],
                'source_wording': {'text': 'Synthetic order only.', 'language': 'en', 'script': 'Latn'},
                'source_scope': 'A test only.', 'coverage': 'partial', 'membership_basis': 'Synthetic fixture.',
                'ordering': {'mode': 'unordered', 'basis': 'No order asserted.', 'precedes': []},
                'limitations': 'No historical or bibliographic admission.',
                'collection_version': assembly.source.metadata_subject(collection).ref,
                'membership_versions': [{'id': 'tos.claim.missing-history-fixture', 'version': 1,
                                         'digest': 'sha256:' + '0' * 64}]})
        helper.fixture.write(ref, canonical_bytes(claim))
        helper.fixture.rebuild()
        snapshot = helper.snapshot(helper.bootstrap())
        row = snapshot.get_claim(claim['claim_id'])
        reader = assembly.BibliographicClaimAssembler(helper.root, catalog_snapshot=snapshot)
        basis = {
            'collection': {'ref': copy.deepcopy(claim['object']['collection_version']),
                           'provenance': {'source': 'fixture'}, 'version_status': 'historical'},
            'memberships': [{'ref': copy.deepcopy(membership_ref), 'provenance': {'source': 'fixture'},
                             'version_status': 'historical'}
                            for membership_ref in claim['object']['membership_versions']],
            'input_digests': {'ToS/test/retained-membership.jsonl': 'a' * 64},
            'establishes_membership': False, 'grants_admission': False,
        }
        with patch.object(assembly.profiles, 'ground_collection_order', return_value=basis) as ground:
            result = reader.assemble(claim['claim_id'], expected_row_sha256=row.row_sha256)
        ground.assert_called_once_with(claim, reader._collection_metadata_reader, reader._claim_version_reader)
        self.assertEqual(result.inputs.collection_order_basis, basis)
        self.assertEqual(result.project(), graph.project_bibliographic_claim(result.inputs))
        self.assertTrue(result.bindings['historical_claim_transport'])
        self.assertEqual(result.bindings['accounting']['claim_versions']['max_read_bytes'],
                         64 * 1024 * 1024)

    def test_collection_order_assembles_a_real_retained_membership_version(self):
        """A controlled correction proves transport, not historical truth."""
        helper = self.agent_fixture()
        for name in ('source-claim-record', 'source-relation-claim', 'source-member-structure-claim',
                     'source-structured-value', 'scoped-member-structure'):
            ref = 'ToS/contracts/' + name + '.schema.json'
            helper.fixture.write(ref, (ROOT / ref).read_bytes())
        collection = {**helper.fixture.record, 'record_type': 'collection',
                      'record_id': 'tos.collection.assembly-history-fixture',
                      'membership_claim_refs': ['tos.claim.collection-order-history-member']}
        work = {**helper.fixture.record, 'record_type': 'work',
                'record_id': 'tos.work.assembly-history-fixture', 'expression_claim_refs': []}
        helper.fixture.write('ToS/source-witnesses/collections/history-fixture/collection.json',
                             canonical_bytes(collection))
        helper.fixture.write('ToS/source-witnesses/works/history-fixture/work.json', canonical_bytes(work))

        membership_id = 'tos.claim.collection-order-history-member'
        membership_ref = 'ToS/source-witnesses/relations/collection-order-history-membership/source-claims.jsonl'
        membership = copy.deepcopy(helper.claim)
        membership.pop('reviews', None)
        unknown = membership.pop('uninterpreted', {})
        membership.update(schema_version='tos_source_relation_claim_v1', claim_type='relation',
                          extensions=unknown, claim_id=membership_id, subject_ref=collection['record_id'],
                          predicate='contains_work', object=work['record_id'], claim_version=1,
                          assertion_layer='scholarly_report', polarity='positive', qualifiers={
                              'statement': 'Initial controlled membership wording.',
                              'statement_language': 'en', 'statement_script': 'Latn',
                              'relation_basis': 'Controlled transport fixture only.',
                              'time_scope_note': 'No historical dates asserted.'})
        helper.fixture.write(membership_ref, canonical_bytes(membership))
        membership_path = helper.root / membership_ref
        form_path = assembly.forms.claim_forms_path(membership_path, membership_id)
        selection = {'form_id': 'tos.form.collection-order-history-member.statement',
                     'field_id': 'claim.statement'}
        initial_change = assembly.source.prepare_claim_change(membership, None, 'test:synthetic', **selection)
        helper.fixture.write(form_path.relative_to(helper.root).as_posix(),
                             claim_packages._encode(assembly.source._apply(
                                 None, claim_revisions._subject(membership), [initial_change])))
        helper.fixture.write((membership_path.parent / 'retained-context.json').relative_to(helper.root).as_posix(),
                             b'{"controlled": true}\n')

        # Build one actual retained archive and current successor with the same
        # byte/package helpers the owner writer uses. This is fixture transport,
        # not a production correction command or an admission decision.
        files = claim_packages._package(membership_path.parent)
        previous_ref = claim_revisions._subject(membership).ref
        previous_revision = claim_packages._revision(files)
        revised = claim_revisions._advance(
            membership, {'qualifiers': {'statement': 'Current controlled membership wording.'}})
        prior_forms = assembly.source._json_object(files[form_path.name])
        revised_change = assembly.source.prepare_claim_change(revised, prior_forms, 'test:synthetic', **selection)
        revised_forms = assembly.source._apply(
            prior_forms, claim_revisions._subject(revised), [revised_change])
        archive_config = {'source_root': str(helper.root), 'source_path': membership_ref,
                          'record_id': membership_id}
        claim_packages._archive(helper.root, archive_config, files,
                                claim_revisions._subject(membership), previous_revision,
                                reader=claim_revisions._read_archive)
        request = {'operation': 'claim.revise', 'command_id': 'test:controlled-history-1',
                   'fields': {'qualifiers': {'statement': 'Current controlled membership wording.'}},
                   'forms': [selection], 'reason': 'Controlled transport fixture only.',
                   'expected_source': previous_ref, 'expected_revision': previous_revision,
                   'expected_configuration': 'sha256:' + '0' * 64,
                   'expected_dependencies': 'sha256:' + '1' * 64, 'expected_inputs': {}}
        receipt = {'command_id': request['command_id'],
                   'request_digest': assembly.source._digest(assembly.source._canonical(request)),
                   'principal_id': 'test:synthetic', 'authority_ref': 'test:not-production-authority',
                   'owner_configuration': request['expected_configuration'],
                   'recorded_at': '2026-01-01T00:00:00Z', 'reason': request['reason'],
                   'previous_source': previous_ref, 'source': claim_revisions._subject(revised).ref,
                   'previous_revision': previous_revision,
                   'archive_path': claim_packages._archive_path(archive_config, previous_revision).as_posix(),
                   'dependencies': request['expected_dependencies'], 'source_bindings': request['expected_inputs'],
                   'changed_fields': ['qualifiers'], 'forms': [assembly.source._form_ref(revised_change['form'])],
                   'grants_admission': False, 'request': request}
        history = {'schema_version': 'tos_claim_revision_history_v1', 'source_path': membership_ref,
                   'receipts': [receipt]}
        membership_path.write_bytes(claim_revisions._replace(membership_path.read_bytes(), revised))
        form_path.write_bytes(claim_packages._encode(revised_forms))
        helper.fixture.write((membership_path.parent / claim_revisions.HISTORY).relative_to(helper.root).as_posix(),
                             claim_packages._encode(history))

        target = copy.deepcopy(helper.claim)
        target.pop('reviews', None)
        unknown = target.pop('uninterpreted', {})
        target.update(schema_version='tos_source_member_structure_claim_v1', claim_type='relation',
                      claim_id='tos.claim.collection-order-history', assertion_layer='scholarly_report',
                      extensions=unknown, qualifiers={
                          'statement': 'Controlled order only, not a historical assertion.',
                          'statement_language': 'en', 'statement_script': 'Latn',
                          'relation_basis': 'Controlled transport fixture only.',
                          'social_scope': 'Fixture-only collection order.',
                          'time_scope_note': 'No historical dates asserted.'})
        target_ref = 'ToS/source-witnesses/relations/native-assembly-history/source-claims.jsonl'
        target.update(schema_version='tos_source_member_structure_claim_v1',
                      subject_ref=collection['record_id'], predicate='collection_member_order',
                      object={'kind': 'collection-member-order', 'members': [work['record_id']],
                              'source_wording': {'text': 'Controlled order only.', 'language': 'en', 'script': 'Latn'},
                              'source_scope': 'A controlled transport fixture.', 'coverage': 'partial',
                              'membership_basis': 'Controlled correction fixture, not historical evidence.',
                              'ordering': {'mode': 'unordered', 'basis': 'No order asserted.', 'precedes': []},
                              'limitations': 'No historical or bibliographic admission.',
                              'collection_version': assembly.source.metadata_subject(collection).ref,
                              'membership_versions': [previous_ref]})
        helper.fixture.write(target_ref, canonical_bytes(target))
        helper.fixture.rebuild()
        snapshot = helper.snapshot(helper.bootstrap())
        target_row = snapshot.get_claim(target['claim_id'])
        reader = assembly.BibliographicClaimAssembler(helper.root, catalog_snapshot=snapshot)
        result = reader.assemble(target['claim_id'], expected_row_sha256=target_row.row_sha256)
        basis = result.inputs.collection_order_basis
        self.assertTrue(result.bindings['historical_claim_transport'])
        self.assertEqual(basis['memberships'][0]['ref'], previous_ref)
        self.assertEqual(basis['memberships'][0]['version_status'], 'historical')
        self.assertEqual(basis['memberships'][0]['provenance']['transition']['source'],
                         claim_revisions._subject(revised).ref)
        self.assertFalse(basis['establishes_membership'])
        self.assertFalse(basis['grants_admission'])
        accounting = result.bindings['accounting']['claim_versions']
        self.assertGreater(accounting['read_bytes'], 0)
        self.assertLessEqual(accounting['read_bytes'], accounting['max_read_bytes'])

        claim_reader = reader._claim_version_reader
        self.assertEqual(claim_reader.resolve(previous_ref)['version_status'], 'historical')
        wrong_digest = {**previous_ref, 'digest': 'sha256:' + 'f' * 64}
        self.assertEqual(claim_reader.resolve(wrong_digest)['reason'], 'exact-version-digest-mismatch')
        missing_version = {**previous_ref, 'version': 9}
        self.assertEqual(claim_reader.resolve(missing_version)['reason'], 'exact-version-not-retained')
        current_ref = claim_revisions._subject(revised).ref
        self.assertEqual(claim_reader.resolve(current_ref)['version_status'], 'current')
        retained_context = helper.root / membership_ref.rsplit('/', 1)[0] / 'retained-context.json'
        retained_context.write_bytes(b'{"controlled": false}\n')
        with self.assertRaises(ValueError):
            reader.verify_current()


if __name__ == '__main__':
    unittest.main()

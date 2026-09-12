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

    def test_collection_order_basis_is_explicitly_unsupported_without_historical_claim_fallback(self):
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
        with patch('claim_version_reader.ClaimVersionReader', side_effect=AssertionError('full Claim history transport')):
            with self.assertRaisesRegex(assembly.ClaimAssemblyUnsupported, 'historical Claim producer'):
                reader.assemble(claim['claim_id'], expected_row_sha256=row.row_sha256)


if __name__ == '__main__':
    unittest.main()

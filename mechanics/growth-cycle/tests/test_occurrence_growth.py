"""Native-bound occurrence growth; every textual example is synthetic.

These tests constrain the public-source/closed-text boundary and the common
owner operations. They do not establish historical, linguistic or rights truth.
"""
from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'tests'))
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))

from test_native_text_binding import NativeTextBindingFixture
from source_record_profiles import SourceRecordProfiles, SourceProfileError
import source_commands as commands


def copy_contracts(root):
    for original in (ROOT / 'ToS/contracts').glob('*.schema.json'):
        path = root / original.relative_to(ROOT)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(original.read_bytes())
    for name in ('entity-types.v1.json', 'relation-types.v1.json'):
        ref = 'ToS/doctrine/semantic-interchange/' + name
        path = root / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes((ROOT / ref).read_bytes())


def occurrence(binding):
    return {'schema_version': 'tos_occurrence_description_record_v1',
        'record_type': 'occurrence', 'record_id': 'tos.occurrence.synthetic.exact-use',
        'record_version': 1, 'preferred_label': 'Условное употребление',
        'notes': 'Synthetic use at a proposed surface token; no linguistic admission.',
        'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                            'notes': {'language': 'en', 'script': 'Latn'}},
        'identity_status': 'provisional', 'source_refs': [binding['packet_ref']],
        'external_identifiers': [], 'same_as_posture': 'no_equivalence_claim',
        'visibility': 'public_metadata_only',
        'semantic_scope': {'scope_note': 'Only this synthetic situated use.',
            'identity_criterion': 'The bound unit, not another occurrence of the same string.',
            'language': 'en', 'script': 'Latn'},
        'semantic_content': {'occurrence_account': 'Synthetic test occurrence only.',
            'context_account': 'The proposed test token, not an accepted lemma or meaning.',
            'language': 'en', 'script': 'Latn', 'unknown_analysis': {'negative': False, 'unknown': None}},
        'native_text_binding': copy.deepcopy(binding),
        'extensions': {'opaque': {'zero': 0, 'empty': '', 'unknown': None}}}


class OccurrenceProfileTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='tos-occurrence-profile-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.native = NativeTextBindingFixture(self.root)
        copy_contracts(self.root)

    def public(self):
        self.native.make_public()
        return occurrence(self.native.binding)

    def test_exact_binding_preserves_distinct_identity_proposed_granularity_and_unknown_fields(self):
        body = self.public()
        profiles = SourceRecordProfiles(self.root)
        profiles.validate('occurrence', body)
        summary = profiles.validate_native_binding('occurrence', body, verify_content=True)
        self.assertTrue(summary['content_verified'])
        self.assertTrue(summary['public_content_declared'])
        self.assertEqual(summary['unit_kind'], 'surface_token')
        self.assertEqual(summary['native_status']['unit_boundary_posture'], 'method_proposed')
        self.assertEqual(summary['native_status']['segmentation_status'], 'proposed')
        self.assertNotEqual(body['record_id'], summary['unit_id'])
        path = 'ToS/source-witnesses/lexical-descriptions/synthetic/occurrence.json'
        self.native.write_json(path, body)
        self.assertEqual(profiles.load('occurrence', path), body)
        entry = profiles.catalog_entry('occurrence', body, path)
        self.assertEqual(profiles.verify_entry('occurrence', entry), body)
        snapshot = commands._profile_input_snapshot(profiles)
        self.assertIn('native_text_binding_snapshot', snapshot)
        self.assertTrue(all(ref.startswith('ToS/contracts/') or ref.endswith('entity-types.v1.json')
                            for ref in snapshot['source_contracts']))
        self.assertNotIn(self.native.content_ref, json.dumps(snapshot))

    def test_metadata_read_without_text_does_not_claim_content_verification(self):
        body = self.public()
        (self.root / self.native.content_ref).unlink()
        profiles = SourceRecordProfiles(self.root)
        profiles.validate('occurrence', body)
        summary = profiles.validate_native_binding('occurrence', body)
        self.assertFalse(summary['content_verified'])
        self.assertFalse(summary['public_content_available'])
        self.assertTrue(summary['public_content_declared'])
        with self.assertRaises(SourceProfileError):
            profiles.validate_native_binding('occurrence', body, verify_content=True)

    def test_private_binding_never_becomes_public_metadata_even_without_a_quote(self):
        body = occurrence(self.native.binding)
        self.assertNotIn(self.native.text[3:8], json.dumps(body))
        profiles = SourceRecordProfiles(self.root)
        with self.assertRaisesRegex(SourceProfileError, 'nonpublic native'):
            profiles.validate('occurrence', body)
        with self.assertRaises(SourceProfileError):
            profiles.validate_native_binding('occurrence', body, verify_content=True)

    def test_incompatible_or_unbound_source_does_not_use_a_description_as_an_address(self):
        body = self.public()
        alternatives = [
            {**body, 'native_text_binding': None},
            {key: value for key, value in body.items() if key != 'native_text_binding'},
            {**body, 'schema_version': 'tos_occurrence_description_record_v99'},
            {**body, 'record_id': body['native_text_binding']['unit_id']},
            {**body, 'semantic_content': {'language': 'en', 'script': 'Latn'}},
            {**body, 'native_text_binding': {**body['native_text_binding'], 'unit_version': 2}},
            {**body, 'native_text_binding': {**body['native_text_binding'], 'ordered_anchor_refs': ['tos.anchor.unknown']}},
        ]
        for invalid in alternatives:
            with self.subTest(invalid=invalid), self.assertRaises(SourceProfileError):
                SourceRecordProfiles(self.root).validate('occurrence', invalid)

    def test_public_flag_does_not_override_current_rights_and_snapshot_drift(self):
        body = self.public()
        profiles = SourceRecordProfiles(self.root)
        profiles.validate('occurrence', body)
        snapshot = profiles.native_text_snapshot()
        path = self.root / self.native.authority_ref
        original_authority = path.read_bytes()
        path.write_bytes(original_authority + b'\n')
        with self.assertRaises(SourceProfileError):
            profiles.native_text_snapshot(read_bytes=commands._read)
        self.assertIsInstance(snapshot, str)
        path.write_bytes(original_authority)
        self.native.rights['redistribution_posture'] = 'not_authorized'
        self.native.refresh()
        body['native_text_binding'] = copy.deepcopy(self.native.binding)
        with self.assertRaises(SourceProfileError):
            SourceRecordProfiles(self.root).validate('occurrence', body)

    def test_adapter_is_not_inferred_from_an_unrecognized_field_or_identity_reader(self):
        body = self.public()
        path = self.root / 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
        registry = json.loads(path.read_bytes())
        entry = next(row for row in registry['types'] if row['type_id'] == 'tos.entity.occurrence')
        entry['source_record_profile'].pop('native_binding_adapter')
        path.write_text(json.dumps(registry))
        with self.assertRaisesRegex(SourceProfileError, 'explicitly declared'):
            SourceRecordProfiles(self.root).validate('occurrence', body)
        entry['source_record_profile']['native_binding_adapter'] = 'source-text-unit-v1'
        entry['source_record_profile']['reader'] = 'corpus-metadata-v1'
        path.write_text(json.dumps(registry))
        with self.assertRaises(SourceProfileError):
            SourceRecordProfiles(self.root)

    def test_lexical_assignments_are_separate_qualified_claims_with_concrete_endpoints(self):
        from source_record_profiles import SourceClaimProfiles
        body = self.public()
        base = json.loads((ROOT / 'ToS/source-witnesses/relations/lexical/source-claims.jsonl')
                          .read_text().splitlines()[1])
        profiles = SourceClaimProfiles(self.root)
        for predicate, kind in (('occurrence_has_form', 'lexical-form'),
                                ('occurrence_of_lexeme', 'lexeme'), ('occurrence_has_sense', 'sense')):
            target = 'tos.' + kind + '.synthetic-use-analysis'
            objects = {body['record_id']: body, target: {'record_id': target, 'record_type': kind}}
            claim = {**copy.deepcopy(base), 'claim_id': 'tos.claim.synthetic.' + predicate.replace('_', '-'),
                'predicate': predicate, 'subject_ref': body['record_id'], 'object': target,
                'evidence_refs': [self.native.policy_ref],
                'provenance_event_ref': 'tos.event.synthetic-use-analysis',
                'qualifiers': {'statement': 'Synthetic assignment only, not historical analysis.',
                    'statement_language': 'en', 'statement_script': 'Latn',
                    'relation_basis': 'Only this synthetic test relation.',
                    'attestation_scope': 'The exact synthetic occurrence, not all spellings.'}}
            with self.subTest(predicate=predicate):
                profiles.validate(claim, objects)
                profiles.validate({**claim, 'claim_id': claim['claim_id'] + '.denied', 'polarity': 'negative'}, objects)
                for invalid in ({**claim, 'subject_ref': target, 'object': body['record_id']},
                                {**claim, 'evidence_refs': []},
                                {**claim, 'qualifiers': {key: value for key, value in claim['qualifiers'].items()
                                                       if key != 'attestation_scope'}}):
                    with self.assertRaises(SourceProfileError):
                        profiles.validate(invalid, objects)
                rule = profiles.relations[predicate]
                self.assertFalse(rule['transitive'])
                self.assertIsNone(rule['cardinality']['per_subject_max'])


class OccurrenceGrowthTests(unittest.TestCase):
    def test_common_creation_revision_forms_assessment_and_reader_keep_the_binding(self):
        from test_source_commands import HistoricalCreationTests
        from assessment_journal import _source_records
        with HistoricalCreationTests().creation() as (root, owner, config, request, rebuild, graph_fixture):
            native = NativeTextBindingFixture(root)
            copy_contracts(root)
            native.make_public()
            (root / 'ToS/source-witnesses/lexical-descriptions').mkdir()
            body = occurrence(native.binding)
            config.pop('allowed_claim_ids')
            config.update(schema_version=commands.PROFILE_CONFIG, profile_type_id='tos.entity.occurrence',
                allowed_operations=['source.create'], record_id=body['record_id'],
                source_path='ToS/source-witnesses/lexical-descriptions/synthetic/occurrence.json',
                provenance_event_id='tos.event.synthetic-occurrence-create')
            owner.write_text(json.dumps(config))
            request.pop('claims')
            request.update(operation='source.create', record=body)
            prepare = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create',
                       'record': body, 'forms': request['forms']}
            preview = commands.run_local_command(owner, prepare)
            request.update(expected_configuration=preview['owner_configuration'],
                           expected_dependencies=preview['expected_dependencies'])
            original = (root / native.content_ref).read_bytes()
            (root / native.content_ref).write_bytes(original + b'corruption')
            with self.assertRaises(SourceProfileError):
                commands.run_local_command(owner, request)
            self.assertFalse((root / config['source_path']).exists())
            (root / native.content_ref).write_bytes(original)
            result = commands.run_local_command(owner, request)
            self.assertFalse(result['grants_admission'])
            self.assertTrue(commands.run_local_command(owner, request)['replayed'])
            source_path = root / config['source_path']
            self.assertEqual(json.loads(source_path.read_bytes()), body)
            graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
            nodes = [node for node in graph['nodes'] if node['entity_id'] == body['record_id']]
            self.assertEqual(len(nodes), 1)
            self.assertEqual(nodes[0]['type_id'], 'tos.entity.occurrence')
            self.assertEqual(nodes[0]['attributes']['source_record'], body)
            self.assertTrue(all(view['state'] == 'ready' and view['admission'] is None
                                for view in nodes[0]['attributes']['human_forms']))
            for view in nodes[0]['attributes']['human_forms']:
                self.assertTrue(any(item['binding']['pointer'] == '/native_text_binding'
                                    and item['value'] == body['native_text_binding'] for item in view['context']))
            snapshots = {}
            selected = [{'path': config['source_path'], 'record_id': body['record_id'],
                         'origin_id': 'synthetic:occurrence-evidence'}]
            records, fixity = _source_records(root, selected, identity_snapshots=snapshots)
            self.assertEqual(records[0]['payload'], body)
            self.assertIn('native_text_binding_snapshot', snapshots)
            self.assertNotIn(native.content_ref, json.dumps(fixity))
            revision = {key: config[key] for key in ('uid', 'principal_id', 'source_root', 'source_path',
                'authority_ref', 'allowed_form_ids', 'expires_at', 'record_id', 'profile_type_id')}
            revision.update(schema_version=commands.PROFILE_REVISION_CONFIG,
                            allowed_operations=['record.revise'], allowed_fields=['notes'])
            writer = root / 'occurrence-revision-owner.json'
            writer.write_text(json.dumps(revision))
            proposal = {'fields': {'notes': 'Corrected description of the same synthetic use; still proposed.'},
                        'forms': request['forms'], 'reason': 'Synthetic description correction only.'}
            prepared = commands.run_local_command(writer, {'schema_version': 'tos_local_source_command_v1',
                                                          'operation': 'prepare-revise', **proposal})
            change = {'schema_version': 'tos_local_source_command_v1', 'operation': 'record.revise',
                'command_id': 'synthetic:occurrence-description-correction',
                'expected_configuration': prepared['owner_configuration'], 'expected_source': prepared['source'],
                'expected_revision': prepared['revision'], 'expected_dependencies': prepared['expected_dependencies'],
                **proposal}
            changed = commands.run_local_command(writer, change)
            self.assertEqual(changed['source']['version'], 2)
            stored = json.loads(source_path.read_bytes())
            self.assertEqual(stored['native_text_binding'], body['native_text_binding'])
            self.assertEqual(stored['extensions'], body['extensions'])
            self.assertEqual(commands.run_local_command(owner, request)['receipt'], result['receipt'])
            writer.write_text(json.dumps({**revision, 'allowed_fields': ['native_text_binding']}))
            with self.assertRaises((ValueError, PermissionError)):
                commands.run_local_command(writer, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})


if __name__ == '__main__':
    unittest.main()

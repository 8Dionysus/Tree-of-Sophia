"""Synthetic public-native mechanics only; no historical or rights evidence."""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(ROOT / 'scripts'), str(ROOT / 'tests'),
    str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]

from test_native_text_binding import NativeTextBindingFixture
from test_occurrence_growth import copy_contracts, occurrence
from native_text_binding import NativeTextBindingResolver
from source_record_profiles import SourceRecordProfiles
import source_commands as source
import source_text_layer_commands as layers
import source_public_native_commands as public


def encoded(value):
    return source._canonical(value) + b'\n'


class PublicNativeCommandTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='tos-public-native-')
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.root, self.recovery = self.base / 'source', self.base / 'recovery'
        self.root.mkdir()
        self.recovery.mkdir(mode=0o700)
        self.fixture = NativeTextBindingFixture(self.root)
        copy_contracts(self.root)
        for ref in public.IMPLEMENTATIONS:
            self.fixture.write_bytes(ref, (ROOT / ref).read_bytes())
        self.original = 'Header\r\nA cafe\u0301 corpus stays literal.\r\nTail'
        self.input_ref = 'ToS/review-ledger/synthetic-project-note.md'
        self.fixture.write_bytes(self.input_ref, self.original.encode('utf-8'))
        self.fixture.write_bytes('LICENSE', b'Synthetic license evidence for a temporary test only.\n')
        start, end = self.original.index('A '), self.original.index('Tail')
        self.content = self.original[start:end]
        sha = source._digest(self.original.encode('utf-8'))[7:]
        self.file_id = 'tos.file.sha256.' + sha
        self.fixture.manifest['payload_files'] = [{'file_id': self.file_id, 'relative_path': 'payload/project.md',
            'original_basename': 'project.md', 'media_type': 'text/markdown', 'byte_size': len(self.original.encode('utf-8')),
            'sha256': sha, 'fixity_verified_at': '2026-01-01T00:00:00Z'}]
        self.fixture.rights['scope_refs'] = [self.fixture.ids['item'], self.file_id]
        self.fixture.write_json(self.fixture.manifest_ref, self.fixture.manifest)
        self.fixture.write_json(self.fixture.rights_ref, self.fixture.rights)
        self.package = 'ToS/source-witnesses/works/synthetic-native-binding/technical-markup/public-range'
        self.path = self.root / self.package / public.BASENAME
        self.path.parent.parent.mkdir(exist_ok=True)
        self.ids = {key: 'tos.' + kind + '.sid-' + format(index, '032x') for index, (key, kind) in enumerate((
            ('layer_id', 'text-layer'), ('anchor_id', 'anchor'), ('passage_id', 'passage'), ('provenance_event_id', 'event'),
            ('packet_id', 'source-text-unit-packet'), ('scheme_id', 'text-unit-scheme'),
            ('segmentation_id', 'text-segmentation'), ('scope_anchor_ref', 'anchor')), 1)}
        self.ids['unit_slots'] = [{'unit_id': 'tos.text-unit.sid-' + 'a' * 32,
            'anchor_ref': 'tos.anchor.sid-' + 'b' * 32, 'unit_kind': 'surface_token'}]
        self.ids['gap_anchor_refs'] = ['tos.anchor.sid-' + char * 32 for char in ('c', 'd')]
        unit_start = self.content.index('corpus')
        unit_end = unit_start + len('corpus')
        scheme = copy.deepcopy(self.fixture.packet['schemes'][0])
        scheme['policies'].update(normalization='no-text-mutation-separate-successor-layer',
                                 unreported_gaps_allowed=False, overlap='forbid')
        method = copy.deepcopy(scheme['method'])
        method.update(agent_ref='software:synthetic-public-native', maker_kind='software',
            configuration_ref=self.package + '/' + public.PLAN_FILE,
            provenance_event_ref=self.ids['provenance_event_id'])
        self.authority_ref = 'ToS/review-ledger/synthetic-public-native-authority.json'
        self.output_rights_ref = self.fixture.native_home + '/public-rights.json'
        source_scope = {**{key + '_ref': value for key, value in self.fixture.ids.items()},
                        'file_ref': self.file_id, 'file_sha256': sha}
        self.config = {'schema_version': public.CONFIG, 'uid': os.getuid(),
            'principal_id': method['agent_ref'], 'authority_ref': 'operator:synthetic-public-native',
            'expires_at': '2099-01-01T00:00:00Z', 'allowed_operations': [public.OPERATION],
            'source_root': str(self.root), 'recovery_root': str(self.recovery),
            'source_path': self.package + '/' + public.BASENAME,
            'source': {'ref': self.input_ref, 'sha256': sha, 'byte_size': len(self.original.encode('utf-8')),
                'media_type': 'text/markdown', 'selector': {'start': start, 'end': end}, 'source_posture': 'project_authored'},
            'source_scope': source_scope, 'source_record_refs': dict(self.fixture.refs),
            'source_record_sha256': {key: self.fixture.file_digest(ref) for key, ref in self.fixture.refs.items()},
            'manifest_sha256': self.fixture.file_digest(self.fixture.manifest_ref),
            'rights_record_refs': [], 'publication_authority': {},
            'license_bindings': [{'ref': 'LICENSE', 'sha256': self.fixture.file_digest('LICENSE')}],
            'language': 'und', 'identities': self.ids,
            'unit_proposal': {'scheme': {key: scheme[key] for key in ('scheme_name', 'analysis_role', 'boundary_basis', 'policies')},
                'method': method,
                'spans': [{'unit_id': self.ids['unit_slots'][0]['unit_id'], 'start': unit_start, 'end': unit_end,
                    'certainty': copy.deepcopy(self.fixture.packet['units'][0]['certainty']), 'status_reason': 'Synthetic supplied interval only.'}],
                'excluded_gaps': [{'anchor_ref': self.ids['gap_anchor_refs'][0], 'start': 0, 'end': unit_start},
                    {'anchor_ref': self.ids['gap_anchor_refs'][1], 'start': unit_end, 'end': len(self.content)}]},
            'limits': {'max_source_bytes': 131072, 'max_output_bytes': 131072, 'max_seconds': 60}}
        self.authority = {'schema_version': 'tos_public_native_text_authority_v1',
            'authority_id': self.config['authority_ref'], 'record_version': 1,
            'operator_scope': {'operator_ref': 'operator:synthetic-test', 'goal_sha256': '0' * 64,
                'holder_task_ref': 'synthetic-test-task', 'delegated_by': 'agent:synthetic-test'},
            'granted_to': self.config['principal_id'], 'issued_at': '2026-01-01T00:00:00Z',
            'expires_at': self.config['expires_at'], 'source': copy.deepcopy(self.config['source']),
            'source_scope': copy.deepcopy(source_scope), 'license_bindings': copy.deepcopy(self.config['license_bindings']),
            'output_scope': {'package_ref': self.package, 'native_identities': public._native_ids(self.config),
                'content_file_id': 'tos.file.sha256.' + source._digest(self.content.encode('utf-8'))[7:],
                'content_sha256': source._digest(self.content.encode('utf-8'))[7:]},
            'permissions': ['native_public_representation', 'native_public_segmentation', 'source_branch_local_reader'],
            'external_publication_authorized': False, 'human_review_performed': False,
            'scope_statement': 'Synthetic authority fixture only; not a real license or grant.'}
        self.output_rights = {**copy.deepcopy(self.fixture.rights),
            'rights_id': 'tos.rights.synthetic.public-output',
            'scope_refs': [self.ids['layer_id'], self.authority['output_scope']['content_file_id']],
            'assessment_status': 'licensed', 'visibility': 'public_payload',
            'redistribution_posture': 'authorized_with_conditions', 'derivative_posture': 'allowed_with_conditions',
            'source_refs': [self.authority_ref, 'LICENSE']}
        self.owner = self.base / 'protected-public-grant.json'
        self.refresh()

    def refresh(self):
        self.fixture.write_json(self.authority_ref, self.authority)
        self.fixture.write_json(self.output_rights_ref, self.output_rights)
        self.config['publication_authority'] = {'ref': self.authority_ref, 'sha256': self.fixture.file_digest(self.authority_ref)}
        self.config['rights_record_refs'] = [{'ref': ref, 'sha256': self.fixture.file_digest(ref)}
            for ref in (self.fixture.rights_ref, self.output_rights_ref)]
        self.write_owner()

    def write_owner(self):
        self.owner.write_bytes(encoded(self.config))
        self.owner.chmod(0o600)

    def invoke(self, operation, **fields):
        return source.run_local_command(self.owner, {'schema_version': public.contract.REQUEST, 'operation': operation, **fields})

    def prepare(self):
        prepared = self.invoke('prepare-create')
        self.request = {'schema_version': public.contract.REQUEST, 'operation': public.OPERATION,
            'command_id': 'synthetic-public-1', 'expected_configuration': prepared['configuration'],
            'expected_dependencies': prepared['dependencies'], 'expected_source': None, 'expected_revision': None}
        return prepared

    def create(self):
        self.prepare()
        return source.run_local_command(self.owner, self.request)

    def files(self):
        return {path.name: path.read_bytes() for path in self.path.parent.iterdir()}

    def test_real_mechanics_public_native_binding_occurrence_and_byte_exact_replay(self):
        result = self.create()
        files = self.files()
        self.assertEqual(files['content.txt'], self.content.encode('utf-8'))
        self.assertEqual((self.root / self.input_ref).read_bytes(), self.original.encode('utf-8'))
        binding = result['native_bindings'][0]
        summary = NativeTextBindingResolver(self.root).resolve(binding, verify_content=True)
        self.assertTrue(summary['public_content_available'])
        body = occurrence(binding)
        SourceRecordProfiles(self.root).validate('occurrence', body)
        self.assertNotEqual(body['record_id'], binding['unit_id'])
        self.assertEqual(json.loads(files['source-text-layer.v1.json'])['admission']['accepted_uses'], [])
        self.assertEqual(json.loads(files[public.BASENAME])['reviews'], [])
        event = json.loads(files['source-create-provenance.jsonl'])
        self.assertEqual(event['activity']['event_type'], 'native_extraction')
        self.assertEqual(event['method']['model_invocations'], [])
        self.assertEqual(event['rights_and_visibility']['content_visibility'], 'public_content')
        self.assertFalse(event['review_and_authority']['promotion_authorized'])
        replay = source.run_local_command(self.owner, self.request)
        self.assertEqual(replay['status'], 'replayed')
        self.assertEqual(replay['receipt_digest'], result['receipt_digest'])
        self.assertEqual(self.files(), files)
        self.assertEqual(self.invoke('inspect-recovery', command_id=self.request['command_id'])['status'], 'committed')

    def test_discovery_never_reads_grants_and_public_receipts_do_not_disclose_local_paths(self):
        with patch.object(public, '_schemas', side_effect=AssertionError('discovery read a grant')):
            descriptor = next(row for row in source.discover_commands()['handlers']
                              if row['handler_id'] == 'public-project-native-text-create')
        self.assertFalse(descriptor['grants_admission'])
        with patch.object(public, '_prepare', side_effect=AssertionError('describe opened text')):
            self.invoke('describe')
        result = self.create()
        public_bytes = b'\n'.join(self.files().values()) + encoded(result)
        for value in (str(self.base), str(self.owner), str(self.recovery), 'recovery_root', 'source_root'):
            self.assertNotIn(value.encode(), public_bytes)

    def test_recovery_does_not_call_a_corrupted_installed_package_committed(self):
        self.create()
        content = self.path.parent / 'content.txt'
        content.write_bytes(b'altered after commit')
        with self.assertRaises(source.JournalConflict):
            self.invoke('inspect-recovery', command_id=self.request['command_id'])
        self.assertEqual(content.read_bytes(), b'altered after commit')

    def test_exact_retry_preserves_initial_inventory_after_unrelated_native_growth(self):
        self.create()
        before = self.files()
        ref = self.fixture.native_home + '/later-provenance.jsonl'
        self.fixture.write_bytes(ref, encoded({'schema_version': 'tos_provenance_event_v1', 'event_id': 'tos.event.later-unrelated'}))
        self.assertEqual(source.run_local_command(self.owner, self.request)['status'], 'replayed')
        self.assertEqual(self.files(), before)
        self.fixture.write_bytes(ref, encoded({'schema_version': 'tos_provenance_event_v1', 'event_id': self.ids['provenance_event_id']}))
        with self.assertRaises(source.JournalConflict):
            source.run_local_command(self.owner, self.request)

    def test_rights_and_authority_fail_before_source_text_io(self):
        original_read = source._read
        def read(path, limit):
            self.assertNotEqual(path, self.root / self.input_ref, 'denied operation opened source text')
            return original_read(path, limit)
        for field, value in (('visibility', 'local_only'), ('assessment_status', 'permission_denied'),
                ('redistribution_posture', 'not_authorized'), ('derivative_posture', 'permission_required')):
            old = self.output_rights[field]
            self.output_rights[field] = value
            self.refresh()
            with self.subTest(field=field), patch.object(source, '_read', side_effect=read), self.assertRaises((ValueError, PermissionError)):
                self.invoke('prepare-create')
            self.output_rights[field] = old
        self.authority['granted_to'] = 'someone-else'
        self.refresh()
        with patch.object(source, '_read', side_effect=read), self.assertRaises(PermissionError):
            self.invoke('prepare-create')
        self.assertFalse(self.path.parent.exists())

    def test_expired_unsafe_or_wrong_profile_grants_and_undeclared_requests_fail(self):
        original = copy.deepcopy(self.config)
        for changed in ({**original, 'expires_at': '2000-01-01T00:00:00Z'},
                {**original, 'allowed_operations': ['text-layer.create']},
                {**original, 'schema_version': layers.CONFIG},
                {**original, 'source_root': str(self.root) + '/../source'},
                {**original, 'source': {**original['source'], 'ref': 'ToS/source-witnesses/owner-local/private.txt'}}):
            self.config = changed
            self.write_owner()
            with self.subTest(changed=changed['schema_version']), self.assertRaises((ValueError, PermissionError)):
                self.invoke('prepare-create')
        self.config = original
        self.write_owner()
        self.owner.chmod(0o644)
        with self.assertRaises((ValueError, PermissionError)):
            self.invoke('prepare-create')
        self.write_owner()
        with self.assertRaises(ValueError):
            self.invoke('prepare-create', publication_authorized=True)

    def test_exact_bindings_prevent_input_rights_license_and_partition_drift(self):
        prepared = self.prepare()
        source_path = self.root / self.input_ref
        original = source_path.read_bytes()
        source_path.write_bytes(original + b'!')
        with self.assertRaises((ValueError, PermissionError)):
            source.run_local_command(self.owner, self.request)
        source_path.write_bytes(original)
        self.fixture.write_bytes('LICENSE', b'Changed synthetic license evidence.')
        with self.assertRaises((ValueError, PermissionError)):
            self.invoke('prepare-create')
        self.assertFalse(self.path.parent.exists())
        self.assertFalse(prepared['grants_admission'])

    def test_identity_collision_and_wrong_unit_partition_do_not_publish(self):
        ref = self.fixture.native_home + '/collision.source-text-layer.json'
        self.fixture.write_json(ref, {'schema_version': 'tos_source_text_layer_v1', 'layer_id': self.ids['layer_id']})
        with self.assertRaises(source.JournalConflict):
            self.prepare()
        (self.root / ref).unlink()
        self.config['unit_proposal']['excluded_gaps'] = []
        self.write_owner()
        with self.assertRaises(ValueError):
            self.prepare()
        self.assertFalse(self.path.parent.exists())

    def test_interrupted_stage_resumes_exact_bytes_and_does_not_overwrite(self):
        self.prepare()
        original = layers._write_new
        count = 0
        def interrupted(path, raw):
            nonlocal count
            if path.parent.name == 'output':
                count += 1
                if count == 3:
                    raise OSError('synthetic interruption before third staged file')
            return original(path, raw)
        with patch.object(layers, '_write_new', side_effect=interrupted), self.assertRaises(OSError):
            source.run_local_command(self.owner, self.request)
        state = self.invoke('inspect-recovery', command_id=self.request['command_id'])
        self.assertEqual(state['status'], 'retained_plan')
        result = source.run_local_command(self.owner, self.request)
        self.assertEqual(result['status'], 'created')
        before = self.files()
        with self.assertRaises(source.JournalConflict):
            source.run_local_command(self.owner, {**self.request, 'command_id': 'another-command'})
        self.assertEqual(self.files(), before)

    def test_torn_or_foreign_staged_evidence_is_preserved_for_owner_review(self):
        self.prepare()
        with (patch.object(source, '_publish_new_directory', side_effect=OSError('synthetic before atomic publication')),
                self.assertRaises(OSError)):
            source.run_local_command(self.owner, self.request)
        control = public._Store(self.config).control(self.path.parent, self.request)
        content = control / 'output' / 'content.txt'
        content.write_bytes(b'torn')
        with self.assertRaises((source.JournalCorruption, source.JournalConflict)):
            source.run_local_command(self.owner, self.request)
        self.assertEqual(content.read_bytes(), b'torn')
        self.assertFalse(self.path.parent.exists())

    def test_precommit_rights_mutation_and_expiry_preserve_original_and_fail_closed(self):
        self.prepare()
        original = public._Store.plan
        changed = False
        def mutate(store, control):
            nonlocal changed
            result = original(store, control)
            if not changed:
                changed = True
                self.output_rights['derivative_posture'] = 'permission_required'
                self.fixture.write_json(self.output_rights_ref, self.output_rights)
            return result
        with patch.object(public._Store, 'plan', new=mutate), self.assertRaises((ValueError, PermissionError)):
            source.run_local_command(self.owner, self.request)
        self.assertFalse(self.path.parent.exists())
        self.assertEqual((self.root / self.input_ref).read_bytes(), self.original.encode())


if __name__ == '__main__':
    unittest.main()

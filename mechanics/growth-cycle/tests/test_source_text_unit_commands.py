"""Synthetic exact-source private writer checks, never linguistic acceptance."""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'tests'))
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))

from test_native_text_binding import NativeTextBindingFixture
from source_owner_context import OwnerLocalSourceContext
from native_text_binding import NativeTextBindingResolver
import source_commands as source
import source_text_unit_commands as native


def encode(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode()


class NativeUnitCommandTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='tos-native-unit-command-')
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.public, self.private = self.base / 'public', self.base / 'private'
        self.public.mkdir()
        self.private.mkdir(mode=0o700)
        self.fixture = NativeTextBindingFixture(self.public)
        for ref in ('ToS/contracts/owner-local-source-context.schema.json', native.PROVENANCE_SCHEMA):
            self.fixture.write_bytes(ref, (ROOT / ref).read_bytes())
        self.prefix = 'ToS/source-witnesses/owner-local/sid-' + '9' * 32 + '/'
        self.source_ref = self.prefix + 'native/new-unit/source-text-unit.v1.json'
        self.path = self.private / self.source_ref
        current = self.private
        for part in Path(self.source_ref).parent.parent.parts:
            current /= part
            current.mkdir(mode=0o700, exist_ok=True)
        self.context_path = self.base / 'source-context.json'
        self.context_config = {'schema_version': 'tos_owner_local_source_context_v1',
            'store_id': 'sid-' + '9' * 32, 'public_root': str(self.public),
            'private_root': str(self.private), 'private_prefix': self.prefix}
        self.context_path.write_bytes(encode(self.context_config))
        self.context_path.chmod(0o600)
        self.owner = self.base / 'owner.json'
        packet = self.fixture.packet
        method = copy.deepcopy(packet['schemes'][0]['method'])
        method.update(agent_ref='software:synthetic-unit-writer',
            configuration_ref=str(Path(self.source_ref).parent / native.CONFIG_FILE),
            provenance_event_ref='tos.event.synthetic.native-unit-create')
        self.config = {'schema_version': native.CONFIG, 'uid': os.getuid(),
            'principal_id': method['agent_ref'], 'authority_ref': 'operator:synthetic-write-delegation',
            'expires_at': '2099-01-01T00:00:00Z', 'source_context_ref': str(self.context_path),
            'source_path': self.source_ref, 'allowed_operations': [native.OPERATION],
            'source_binding': copy.deepcopy(self.fixture.binding),
            'source_access': {'read_scope': 'exact_owner_local', 'access_allowed': True,
                              'authority_ref': 'operator:synthetic-exact-source-read'},
            'allowed_text_scope': {'start': 3, 'end': 8},
            'packet_id': 'tos.source-text-unit-packet.sid-' + '9' * 32,
            'scheme_id': 'tos.text-unit-scheme.sid-' + '9' * 32,
            'segmentation_id': 'tos.text-segmentation.sid-' + '9' * 32,
            'scope_anchor_ref': 'tos.anchor.synthetic.new.scope',
            'unit_slots': [
                {'unit_id': 'tos.text-unit.sid-' + '9' * 32, 'anchor_ref': 'tos.anchor.synthetic.new.one', 'unit_kind': 'surface_token'},
                {'unit_id': 'tos.text-unit.sid-' + '8' * 32, 'anchor_ref': 'tos.anchor.synthetic.new.two', 'unit_kind': 'surface_token'}],
            'gap_anchor_refs': ['tos.anchor.synthetic.new.gap'],
            'scheme': {key: copy.deepcopy(packet['schemes'][0][key])
                       for key in ('scheme_name', 'analysis_role', 'boundary_basis', 'policies')},
            'method': method, 'provenance_event_id': method['provenance_event_ref']}
        certainty = copy.deepcopy(packet['units'][0]['certainty'])
        self.proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create',
            'spans': [
                {'unit_id': self.config['unit_slots'][0]['unit_id'], 'start': 3, 'end': 5,
                 'certainty': certainty, 'status_reason': 'Synthetic proposed first interval only.'},
                {'unit_id': self.config['unit_slots'][1]['unit_id'], 'start': 6, 'end': 8,
                 'certainty': certainty, 'status_reason': 'Synthetic proposed second interval only.'}],
            'excluded_gaps': [{'anchor_ref': self.config['gap_anchor_refs'][0], 'start': 5, 'end': 6}]}
        self.write_owner()

    def write_owner(self):
        self.owner.write_bytes(encode(self.config))
        self.owner.chmod(0o600)

    def run_command(self, request):
        return source.run_local_command(self.owner, request)

    def prepare(self):
        prepared = self.run_command(self.proposal)
        self.request = {**self.proposal, 'operation': native.OPERATION, 'command_id': 'synthetic-native-command-1',
            'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'],
            'expected_source': None, 'expected_revision': None}
        return prepared

    def files(self):
        return {path.name: path.read_bytes() for path in self.path.parent.iterdir()}

    def assert_not_published(self):
        self.assertFalse(os.path.lexists(self.path.parent))
        self.assertEqual(list(self.private.glob('.native-create-*.pending')), [])

    def test_describe_needs_delegation_but_never_reads_the_representation(self):
        (self.public / self.fixture.content_ref).unlink()
        described = self.run_command({'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
        self.assertEqual(described['supported_operations'], ['text-unit.create'])
        self.assertFalse(described['grants_admission'])
        self.assertFalse(described['target_exists'])
        self.assert_not_published()

    def test_private_creation_builds_exact_native_packet_and_auditable_unadmitted_receipt(self):
        original = (self.public / self.fixture.packet_ref).read_bytes()
        prepared = self.prepare()
        result = self.run_command(self.request)
        packet = json.loads(self.path.read_bytes())
        self.assertEqual(packet['source_scope'], self.fixture.packet['source_scope'])
        self.assertEqual(packet['source_layer'], self.fixture.packet['source_layer'])
        self.assertEqual(result['receipt']['source'], prepared['prepared_source'])
        self.assertEqual(packet['rights_and_visibility']['packet_visibility'], 'local_only')
        self.assertFalse(packet['rights_and_visibility']['publication_authorized'])
        self.assertEqual(packet['segmentations'][0]['status'], 'proposed')
        self.assertEqual(packet['reviews'], [])
        self.assertFalse(result['grants_admission'])
        self.assertEqual((self.public / self.fixture.packet_ref).read_bytes(), original)
        self.assertEqual((self.public / self.fixture.content_ref).read_bytes(), self.fixture.content)
        self.assertFalse((self.public / self.source_ref).exists())
        self.assertEqual(self.path.parent.stat().st_mode & 0o777, 0o700)
        self.assertTrue(all(path.stat().st_mode & 0o777 == 0o600 for path in self.path.parent.iterdir()))
        event = json.loads((self.path.parent / 'source-create-provenance.jsonl').read_bytes())
        self.assertEqual(event['activity']['event_type'], 'segmentation')
        self.assertEqual(event['rights_and_visibility']['content_visibility'], 'local_only')
        inputs = event['entities']['inputs']
        self.assertTrue(any(item['entity_ref'] == self.fixture.content_ref and item['fixity_verified'] for item in inputs))
        self.assertTrue(all(item['content_disclosure'] == 'private_content' for group in event['entities'].values() for item in group))
        # New packet is usable by the same native resolver, not a second grammar.
        binding = copy.deepcopy(self.fixture.binding)
        binding.update(packet_ref=self.source_ref, packet_id=packet['packet_id'],
            packet_sha256=source._digest(self.path.read_bytes())[7:], unit_id=packet['units'][0]['unit_id'],
            ordered_anchor_refs=packet['units'][0]['ordered_anchor_refs'],
            segmentation_id=packet['segmentations'][0]['segmentation_id'])
        resolved = NativeTextBindingResolver(self.public, owner_context=OwnerLocalSourceContext.load(self.context_path)).resolve(
            binding, verify_content=True, allow_private_content=True)
        self.assertTrue(resolved['content_verified'])
        self.assertFalse(resolved['public_content_declared'])

    def test_historical_retry_preserves_receipt_and_checks_current_source_topology(self):
        self.prepare()
        first = self.run_command(self.request)
        files = self.files()
        replay = self.run_command(self.request)
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], first['receipt'])
        self.assertEqual(self.files(), files)
        self.fixture.manifest['manifest_version'] = 2
        self.fixture.write_json(self.fixture.manifest_ref, self.fixture.manifest)
        # The historical receipt is not a promise that every unpinned source
        # metadata byte still equals the original prepare-time inventory.
        replay = self.run_command(self.request)
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], first['receipt'])
        self.assertEqual(self.files(), files)
        self.fixture.manifest['payload_files'][0]['sha256'] = '0' * 64
        self.fixture.write_json(self.fixture.manifest_ref, self.fixture.manifest)
        with self.assertRaises((ValueError, source.JournalConflict)):
            self.run_command(self.request)
        self.assertEqual(self.files(), files)

    def test_historical_retry_does_not_require_original_implementation_bytes(self):
        self.prepare()
        first = self.run_command(self.request)
        files = self.files()
        original_read = source._read
        implementation = source.ROOT / native.IMPLEMENTATIONS[0]
        def changed(path, limit):
            raw = original_read(path, limit)
            return raw + b'\n' if path == implementation else raw
        with patch.object(source, '_read', side_effect=changed):
            replay = self.run_command(self.request)
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], first['receipt'])
        self.assertEqual(self.files(), files)

    def test_historical_retry_still_refuses_current_identity_and_grammar_conflicts(self):
        self.prepare()
        self.run_command(self.request)
        files = self.files()
        collision = self.private / self.prefix / 'native/competing-provenance.jsonl'
        collision.write_bytes(source._canonical({'event_id': self.config['provenance_event_id']}) + b'\n')
        collision.chmod(0o600)
        try:
            with self.assertRaises(source.JournalConflict):
                self.run_command(self.request)
        finally:
            collision.unlink()
        contract = self.public / 'ToS/contracts/corpus-record.schema.json'
        raw = contract.read_bytes()
        contract.write_bytes(raw + b'\n')
        self.assertTrue(self.run_command(self.request)['replayed'])
        schema = json.loads(raw)
        schema.setdefault('allOf', []).append({'required': ['synthetic_missing_current_field']})
        contract.write_bytes(encode(schema))
        with self.assertRaises((ValueError, source.JournalConflict)):
            self.run_command(self.request)
        self.assertEqual(self.files(), files)

    def test_historical_retry_refuses_late_source_delegation_and_package_drift(self):
        self.prepare()
        self.run_command(self.request)
        files = self.files()
        cases = [(self.public / self.fixture.manifest_ref, 1)]
        cases.extend((target, phase) for target in (self.owner, self.context_path,
            self.path.parent / 'source-create-environment.json') for phase in (1, 2))
        for target, phase in cases:
            with self.subTest(target=target.name, validation_pass=phase):
                retained = target.read_bytes()
                original_prepare = native._prepare
                calls = 0
                def drift(*args, **kwargs):
                    nonlocal calls
                    result = original_prepare(*args, **kwargs)
                    calls += 1
                    if calls == phase:
                        if target == self.owner:
                            revoked = json.loads(retained)
                            revoked['source_access']['access_allowed'] = False
                            target.write_bytes(encode(revoked))
                        else:
                            target.write_bytes(retained + b'\n')
                    return result
                try:
                    with patch.object(native, '_prepare', side_effect=drift):
                        with self.assertRaises((ValueError, PermissionError, source.JournalConflict, source.JournalCorruption)):
                            self.run_command(self.request)
                finally:
                    target.write_bytes(retained)
                self.assertEqual(self.files(), files)

    def test_missing_source_access_refuses_before_content_read(self):
        self.config['source_access']['access_allowed'] = False
        self.write_owner()
        with patch.object(NativeTextBindingResolver, 'resolve', side_effect=AssertionError('must not read')):
            with self.assertRaises(PermissionError):
                self.run_command(self.proposal)
        self.assert_not_published()

    def test_owner_configuration_must_itself_be_confidential(self):
        self.owner.chmod(0o644)
        with self.assertRaises(ValueError):
            self.run_command(self.proposal)
        self.assert_not_published()

    def test_read_access_does_not_clear_unknown_or_conditional_derivation_rights(self):
        for posture in ('unknown', 'permission_required', 'allowed_with_conditions'):
            with self.subTest(posture=posture):
                self.fixture.rights['derivative_posture'] = posture
                self.fixture.refresh()
                self.config['source_binding'] = copy.deepcopy(self.fixture.binding)
                self.write_owner()
                seen = []
                reader = source._read
                def observe(path, limit):
                    seen.append(path)
                    return reader(path, limit)
                with patch.object(source, '_read', side_effect=observe):
                    with self.assertRaises(PermissionError):
                        self.run_command(self.proposal)
                self.assertNotIn(self.public / self.fixture.content_ref, seen)
                self.assert_not_published()

    def _exact_rights_layer(self, derivative_posture='allowed'):
        return {
            'layer_id': 'tos.rights.synthetic.native-binding.layer.text',
            'layer_role': 'embedded_text',
            'scope_refs': [self.fixture.layer['layer_id'], self.fixture.layer['representation']['content_file_id']],
            'assessment_status': 'licensed', 'assessment_basis': 'operator_statement',
            'jurisdictions_reviewed': [], 'source_refs': [self.fixture.policy_ref],
            'rights_holder_refs': [], 'permissions': [], 'restrictions': ['Synthetic only.'],
            'redistribution_posture': 'not_authorized', 'derivative_posture': derivative_posture,
            'server_processing_posture': 'local_research_only',
            'term': {'calculation_status': 'not_applicable', 'basis': 'Synthetic only.',
                     'starts_on': None, 'ends_on': None, 'uncertainty': 'Synthetic only.'},
            'uncertainty': 'Synthetic only.', 'assessed_at': '2026-09-08T00:00:00Z',
            'review_status': 'unreviewed', 'rationale': 'Synthetic only.'}

    def test_exact_child_does_not_revive_a_superseded_or_denied_parent_rights_record(self):
        original = copy.deepcopy(self.fixture.rights)
        for fields in ({'review_status': 'superseded'}, {'assessment_status': 'permission_denied'},
                       {'assessment_status': 'conflicting_evidence'}):
            with self.subTest(parent_state=fields):
                self.fixture.rights = {**copy.deepcopy(original), **fields,
                                       'layer_assessments': [self._exact_rights_layer()]}
                self.fixture.refresh()
                self.config['source_binding'] = copy.deepcopy(self.fixture.binding)
                self.write_owner()
                seen, reader = [], source._read
                def observe(path, limit):
                    seen.append(path)
                    return reader(path, limit)
                with patch.object(source, '_read', side_effect=observe):
                    with self.assertRaises(PermissionError):
                        self.run_command(self.proposal)
                self.assertNotIn(self.public / self.fixture.content_ref, seen)
                self.assert_not_published()

    def test_direct_exact_allow_does_not_mask_competing_nested_exact_restrictions(self):
        original = copy.deepcopy(self.fixture.rights)
        extra_ref = self.fixture.native_home + '/rights.synthetic-exact-layer.json'
        direct = {**copy.deepcopy(original),
                  'rights_id': 'tos.rights.synthetic.exact-layer',
                  'scope_refs': [self.fixture.layer['layer_id'], self.fixture.layer['representation']['content_file_id']],
                  'assessment_status': 'licensed', 'derivative_posture': 'allowed'}
        self.fixture.write_json(extra_ref, direct)
        for posture in ('unknown', 'permission_required', 'allowed_with_conditions'):
            with self.subTest(nested_derivative_posture=posture):
                self.fixture.rights = {**copy.deepcopy(original),
                                       'layer_assessments': [self._exact_rights_layer(posture)]}
                self.fixture.packet['rights_and_visibility']['rights_record_refs'] = [self.fixture.rights_ref]
                self.fixture.refresh()
                self.fixture.layer['representation']['rights_record_refs'].append(
                    {'ref': extra_ref, 'sha256': self.fixture.file_digest(extra_ref)})
                self.fixture.packet['rights_and_visibility']['rights_record_refs'].append(extra_ref)
                self.fixture.write_json(self.fixture.layer_ref, self.fixture.layer)
                self.fixture.write_json(self.fixture.packet_ref, self.fixture.packet)
                self.fixture.binding['text_layer']['record_sha256'] = self.fixture.file_digest(self.fixture.layer_ref)
                self.fixture.binding['packet_sha256'] = self.fixture.file_digest(self.fixture.packet_ref)
                self.config['source_binding'] = copy.deepcopy(self.fixture.binding)
                self.write_owner()
                seen, reader = [], source._read
                def observe(path, limit):
                    seen.append(path)
                    return reader(path, limit)
                with patch.object(source, '_read', side_effect=observe):
                    with self.assertRaises(PermissionError):
                        self.run_command(self.proposal)
                self.assertNotIn(self.public / self.fixture.content_ref, seen)
                self.assert_not_published()

    def test_request_cannot_select_packet_source_identity_or_authority(self):
        for field, value in (('packet', self.fixture.packet), ('source_binding', self.fixture.binding),
                             ('authority_ref', 'injected'), ('source_path', self.fixture.packet_ref)):
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.run_command({**self.proposal, field: value})
        self.assert_not_published()

    def test_scope_cannot_expand_to_other_parts_of_the_same_representation(self):
        self.config['allowed_text_scope'] = {'start': 3, 'end': 10}
        self.write_owner()
        with self.assertRaises(PermissionError):
            self.run_command(self.proposal)
        self.assert_not_published()

    def test_prepared_dependency_drift_refuses_without_publication(self):
        self.prepare()
        self.fixture.manifest['manifest_version'] = 2
        self.fixture.write_json(self.fixture.manifest_ref, self.fixture.manifest)
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.request)
        self.assert_not_published()

    def test_configuration_change_refuses_a_prepared_request(self):
        self.prepare()
        self.config['authority_ref'] = 'operator:changed-delegation'
        self.write_owner()
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.request)
        self.assert_not_published()

    def test_public_and_private_native_identity_collisions_are_not_new_objects(self):
        self.config['packet_id'] = self.fixture.packet['packet_id']
        self.write_owner()
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.proposal)
        self.config['packet_id'] = 'tos.source-text-unit-packet.sid-' + '9' * 32
        self.write_owner()
        collision = self.private / self.prefix / 'native/source-anchor-v2.collision.json'
        body = copy.deepcopy(self.fixture.anchor)
        body['anchor_id'] = self.config['scope_anchor_ref']
        collision.write_bytes(encode(body))
        collision.chmod(0o600)
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.proposal)
        self.assert_not_published()

    def test_existing_suffix_named_packet_and_jsonl_anchor_ids_are_reserved(self):
        for basename, body in (
                ('part-1.source-text-unit.v1.json', {**self.fixture.packet, 'packet_id': self.config['packet_id']}),
                ('structure-anchors.jsonl', {**self.fixture.anchor, 'anchor_id': self.config['scope_anchor_ref']})):
            with self.subTest(basename=basename):
                collision = self.public / self.fixture.native_home / basename
                collision.write_bytes(source._canonical(body) + b'\n')
                try:
                    with self.assertRaises(source.JournalConflict):
                        self.run_command(self.proposal)
                finally:
                    collision.unlink()
                self.assert_not_published()

    def test_staging_corruption_never_becomes_an_authored_packet(self):
        self.prepare()
        publish = source._publish
        def corrupt(path, raw):
            publish(path, raw + b' ' if path.name == 'source-text-unit.v1.json' else raw)
        with patch.object(source, '_publish', side_effect=corrupt):
            with self.assertRaises(ValueError):
                self.run_command(self.request)
        self.assert_not_published()

    def test_real_cli_uses_the_same_creation_and_retry_contract_without_text_output(self):
        script = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py'
        def cli(request):
            process = subprocess.run([sys.executable, str(script), '--owner-config', str(self.owner)],
                input=encode(request), capture_output=True, timeout=30)
            self.assertEqual(process.returncode, 0, process.stderr.decode())
            self.assertNotIn(self.fixture.content, process.stdout)
            return json.loads(process.stdout)
        prepared = cli(self.proposal)
        request = {**self.proposal, 'operation': native.OPERATION, 'command_id': 'synthetic-native-cli',
            'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'], 'expected_source': None, 'expected_revision': None}
        created = cli(request)
        replay = cli(request)
        self.assertFalse(created['replayed'])
        self.assertTrue(replay['replayed'])
        self.assertEqual(created['receipt'], replay['receipt'])

    def test_drift_during_staging_aborts_and_preserves_exact_source(self):
        self.prepare()
        original_publish = source._publish
        changed = False
        def publish(path, raw):
            nonlocal changed
            original_publish(path, raw)
            if not changed:
                changed = True
                self.fixture.manifest['manifest_version'] = 2
                self.fixture.write_json(self.fixture.manifest_ref, self.fixture.manifest)
        with patch.object(source, '_publish', side_effect=publish):
            with self.assertRaises(source.JournalConflict):
                self.run_command(self.request)
        self.assert_not_published()
        self.assertEqual(self.fixture.read_json(self.fixture.manifest_ref)['manifest_version'], 2)

    def test_corrupt_replay_package_is_never_excluded_or_overwritten(self):
        self.prepare()
        self.run_command(self.request)
        altered = self.path.parent / 'source-create-environment.json'
        altered.write_bytes(altered.read_bytes() + b' ')
        retained = self.files()
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.request)
        self.assertEqual(self.files(), retained)

    def test_existing_empty_target_is_not_overwritten(self):
        self.prepare()
        self.path.parent.mkdir(mode=0o700)
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.request)
        self.assertEqual(list(self.path.parent.iterdir()), [])

    def test_identity_inventory_is_bounded_and_does_not_read_payload(self):
        self.assertFalse((self.public / self.fixture.original_ref).exists())
        with patch.object(native, 'MAX_INVENTORY_ENTRIES', 1):
            with self.assertRaises(ValueError):
                self.run_command(self.proposal)
        self.assert_not_published()

    def test_legacy_large_native_json_is_reserved_with_an_explicit_file_budget(self):
        legacy = self.public / self.fixture.native_home / 'part-1.source-text-unit.v1.json'
        packet = {**self.fixture.packet, 'packet_id': self.config['packet_id']}
        legacy.write_bytes(source._canonical(packet) + b' ' * source.MAX_COMMAND_BYTES)
        self.assertGreater(legacy.stat().st_size, source.MAX_COMMAND_BYTES)
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.proposal)
        with patch.object(native, 'MAX_INVENTORY_FILE_BYTES', source.MAX_COMMAND_BYTES):
            with self.assertRaises(ValueError):
                self.run_command(self.proposal)
        self.assert_not_published()

    def test_process_loss_before_rename_leaves_no_source_and_retry_is_complete(self):
        self.prepare()
        module_home = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
        program = (
            'import os,sys,json; from pathlib import Path; '
            'sys.path.insert(0,sys.argv[1]); import source_commands as source; '
            'source._publish_new_directory=lambda staging,target: os._exit(71); '
            'source.run_local_command(Path(sys.argv[2]),json.loads(sys.stdin.buffer.read()))')
        process = subprocess.run([sys.executable, '-c', program, str(module_home), str(self.owner)],
            input=encode(self.request), capture_output=True, timeout=30)
        self.assertEqual(process.returncode, 71, process.stderr.decode())
        self.assertFalse(self.path.parent.exists())
        abandoned = list(self.private.glob('.native-create-*.pending'))
        self.assertEqual(len(abandoned), 1)
        retained = {path.name: path.read_bytes() for path in abandoned[0].iterdir()}
        result = self.run_command(self.request)
        self.assertFalse(result['replayed'])
        self.assertTrue(self.path.exists())
        # A new invocation neither accepts nor deletes another invocation's stage.
        self.assertEqual({path.name: path.read_bytes() for path in abandoned[0].iterdir()}, retained)
        self.assertTrue(self.run_command(self.request)['replayed'])


if __name__ == '__main__':
    unittest.main()

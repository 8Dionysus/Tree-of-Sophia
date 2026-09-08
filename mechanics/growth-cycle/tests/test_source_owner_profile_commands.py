"""Synthetic private source transactions, not real semantic or rights evidence."""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'tests'))
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
from test_native_text_binding import NativeTextBindingFixture
from test_occurrence_growth import copy_contracts, occurrence
from source_owner_context import OwnerLocalSourceContext
import source_commands as source
import source_owner_profile_commands as private


def encode(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode()


class OwnerLocalProfileCommandTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='tos-private-source-command-')
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.public, self.store = self.base / 'public', self.base / 'private'
        self.public.mkdir()
        self.store.mkdir(mode=0o700)
        self.native = NativeTextBindingFixture(self.public)
        copy_contracts(self.public)
        self.prefix = 'ToS/source-witnesses/owner-local/sid-' + '8' * 32 + '/'
        self.source_ref = self.prefix + 'descriptions/synthetic/occurrence.json'
        self.path = self.store / self.source_ref
        current = self.store
        for part in Path(self.source_ref).parent.parent.parts:
            current /= part
            current.mkdir(mode=0o700, exist_ok=True)
        self.context_path = self.base / 'context.json'
        self.context_config = {'schema_version': 'tos_owner_local_source_context_v1', 'store_id': 'sid-' + '8' * 32,
            'public_root': str(self.public), 'private_root': str(self.store), 'private_prefix': self.prefix}
        self.context_path.write_bytes(encode(self.context_config))
        self.context_path.chmod(0o600)
        self.record = occurrence(self.native.binding)
        self.record['visibility'] = 'local_only'
        self.config = {'schema_version': private.CONFIG, 'uid': os.getuid(),
            'principal_id': 'agent:synthetic-private-writer', 'authority_ref': 'operator:synthetic-scope',
            'expires_at': '2099-01-01T00:00:00Z', 'source_context_ref': str(self.context_path),
            'source_path': self.source_ref, 'source_access': {'read_scope': 'exact_owner_local',
                'access_allowed': True, 'authority_ref': 'operator:synthetic-source-read'},
            'source_binding': copy.deepcopy(self.native.binding), 'profile_type_id': 'tos.entity.occurrence',
            'record_id': self.record['record_id'], 'allowed_operations': list(private.OPERATIONS),
            'allowed_fields': ['preferred_label', 'notes', 'field_languages', 'semantic_content'],
            'allowed_form_ids': ['tos.form.synthetic.private-name', 'tos.form.synthetic.private-note'],
            'provenance_event_id': 'tos.event.synthetic.private-creation'}
        self.forms = [{'form_id': self.config['allowed_form_ids'][0], 'field_id': 'metadata.preferred-name'}]
        self.owner = self.base / 'owner.json'
        self.write_owner()

    def write_owner(self):
        self.owner.write_bytes(encode(self.config))
        self.owner.chmod(0o600)

    def run_command(self, request):
        return source.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1', **request})

    def prepare_create(self):
        prepared = self.run_command({'operation': 'prepare-create', 'record': self.record, 'forms': self.forms})
        self.creation = {'operation': 'source.create', 'command_id': 'synthetic-create', 'record': self.record,
            'forms': self.forms, 'expected_configuration': prepared['owner_configuration'],
            'expected_source': None, 'expected_revision': None, 'expected_dependencies': prepared['expected_dependencies']}
        return prepared

    def create(self):
        self.prepare_create()
        return self.run_command(self.creation)

    def revise(self, *, apply=True):
        proposal = {'fields': {'notes': 'Corrected synthetic account; the native use is unchanged.'},
                    'forms': self.forms, 'reason': 'Synthetic descriptive correction, not identity replacement.'}
        prepared = self.run_command({'operation': 'prepare-revise', **proposal})
        self.revision = {'operation': 'record.revise', 'command_id': 'synthetic-revise', **proposal,
            'expected_configuration': prepared['owner_configuration'], 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_dependencies': prepared['expected_dependencies']}
        return self.run_command(self.revision) if apply else prepared

    def prepare_form(self, form_index=1):
        prepared = self.run_command({'operation': 'prepare', 'form_id': self.config['allowed_form_ids'][form_index],
                                    'field_id': 'metadata.source-note' if form_index else 'metadata.preferred-name'})
        self.form_request = {'operation': 'apply', 'command_id': 'synthetic-form-' + str(form_index),
            'expected_configuration': prepared['owner_configuration'], 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_dependencies': prepared['expected_dependencies'],
            'changes': [prepared['prepared_change']]}
        return prepared

    def files(self):
        return {path.name: path.read_bytes() for path in self.path.parent.iterdir()}

    def test_create_private_occurrence_forms_provenance_and_retry_share_exact_grammar(self):
        prepared = self.prepare_create()
        public_before = (self.public / self.native.packet_ref).read_bytes()
        result = self.run_command(self.creation)
        self.assertEqual(result['source'], prepared['prepared_source'])
        self.assertEqual(json.loads(self.path.read_bytes()), self.record)
        self.assertFalse(result['grants_admission'])
        self.assertFalse(result['publication_authorized'])
        self.assertEqual(result['materializations'][0]['display_text'], self.record['preferred_label'])
        self.assertTrue(all(row['admission'] is None for row in result['materializations']))
        self.assertEqual((self.public / self.native.packet_ref).read_bytes(), public_before)
        self.assertFalse((self.public / self.source_ref).exists())
        self.assertEqual(self.path.parent.stat().st_mode & 0o777, 0o700)
        self.assertTrue(all(path.stat().st_mode & 0o777 == 0o600 for path in self.path.parent.iterdir()))
        event = json.loads((self.path.parent / 'source-create-provenance.jsonl').read_bytes())
        self.assertEqual(event['activity']['event_type'], 'annotation')
        self.assertEqual(event['rights_and_visibility']['content_visibility'], 'local_only')
        self.assertEqual(event['rights_and_visibility']['intended_uses'], ['local_research'])
        self.assertTrue(all(item['content_disclosure'] == 'private_content' for group in event['entities'].values() for item in group))
        self.assertTrue(self.run_command(self.creation)['replayed'])
        self.assertEqual(self.run_command(self.creation)['receipt'], result['receipt'])

    def test_revision_keeps_exact_original_archive_native_identity_unknowns_and_forms(self):
        created = self.create()
        original = self.files()
        revised = self.revise()
        self.assertEqual(revised['source']['version'], 2)
        current = json.loads(self.path.read_bytes())
        self.assertEqual(current['native_text_binding'], self.record['native_text_binding'])
        self.assertEqual(current['semantic_scope'], self.record['semantic_scope'])
        self.assertEqual(current['semantic_content']['unknown_analysis'], self.record['semantic_content']['unknown_analysis'])
        self.assertEqual(current['extensions'], self.record['extensions'])
        self.assertTrue(revised['receipt']['archive_path'].startswith(self.prefix + '.record-revisions/'))
        inspected = self.run_command({'operation': 'inspect-version', 'source': created['source']})
        self.assertEqual(inspected['record'], self.record)
        self.assertEqual(set(inspected['files']), set(original))
        for name, ref in inspected['files'].items():
            self.assertEqual((self.store / ref['archive_path']).read_bytes(), original[name])
        payload = json.loads((self.path.parent / 'occurrence.human-forms.json').read_bytes())
        self.assertEqual(payload['prior_forms'][0]['subject'], created['source'])
        self.assertEqual(payload['forms'][0]['subject'], revised['source'])
        self.assertTrue(self.run_command(self.revision)['replayed'])
        self.assertTrue(self.run_command(self.creation)['replayed'])

    def test_form_create_and_revision_keep_package_source_and_creation_replay(self):
        self.create()
        original = self.path.read_bytes()
        self.prepare_form()
        first = self.run_command(self.form_request)
        self.assertEqual(self.path.read_bytes(), original)
        self.assertEqual(len(first['forms']), 2)
        self.assertTrue(self.run_command(self.form_request)['replayed'])
        self.prepare_form(0)
        second = self.run_command(self.form_request)
        self.assertEqual(second['forms'][0]['version'], 2)
        self.assertTrue(self.run_command(self.creation)['replayed'])
        self.forms.append({'form_id': self.config['allowed_form_ids'][1], 'field_id': 'metadata.source-note'})
        revised = self.revise()
        self.assertEqual(revised['source']['version'], 2)
        self.assertEqual(len(revised['materializations']), 2)

    def test_public_profile_renderer_catalog_and_ordinary_command_refuse_private_source(self):
        from source_record_profiles import SourceRecordProfiles, SourceProfileError
        from build_source_witness_catalog import collect_records, CatalogBuildError
        from source_witness_human_forms import materialize_metadata_forms
        from source_owner_record_profiles import OwnerLocalSourceRecordProfiles
        self.create()
        payload = json.loads((self.path.parent / 'occurrence.human-forms.json').read_bytes())
        views = materialize_metadata_forms(self.record, payload, access_allowed=True)
        self.assertTrue(all(row['display_text'] is None for row in views))
        with self.assertRaises(SourceProfileError):
            SourceRecordProfiles(self.public).validate('occurrence', self.record)
        profiles = OwnerLocalSourceRecordProfiles(OwnerLocalSourceContext.load(self.context_path), self.config['source_access'], self.config['source_binding'])
        with self.assertRaises(CatalogBuildError):
            collect_records(self.public, profiles=profiles)
        with self.assertRaises(CatalogBuildError):
            collect_records(self.store, profiles=SourceRecordProfiles(self.public))
        self.config = {key: self.config[key] for key in ('uid', 'principal_id', 'authority_ref', 'expires_at', 'source_path', 'allowed_form_ids')}
        self.config.update(schema_version='tos_local_source_command_owner_v1', source_root=str(self.store), allowed_operations=['form.create'])
        self.write_owner()
        with self.assertRaises(PermissionError):
            self.run_command({'operation': 'describe'})

    def test_owner_configuration_and_private_permissions_are_enforced_before_use(self):
        for mode in (0o644, 0o640):
            with self.subTest(mode=mode):
                self.owner.chmod(mode)
                with self.assertRaises((ValueError, PermissionError)):
                    self.run_command({'operation': 'describe'})
        self.owner.chmod(0o600)
        self.create()
        self.path.chmod(0o644)
        with self.assertRaises((ValueError, PermissionError)):
            self.run_command({'operation': 'describe'})
        self.path.chmod(0o600)
        self.path.parent.chmod(0o755)
        with self.assertRaises((ValueError, PermissionError)):
            self.run_command({'operation': 'describe'})

    def test_exact_read_scope_and_current_rights_precede_content_reads(self):
        observed = []
        original = source._read
        def read(path, limit):
            observed.append(path)
            return original(path, limit)
        self.native.rights['derivative_posture'] = 'not_authorized'
        self.native.refresh()
        self.record['native_text_binding'] = copy.deepcopy(self.native.binding)
        self.config['source_binding'] = copy.deepcopy(self.native.binding)
        self.write_owner()
        with patch.object(source, '_read', side_effect=read), self.assertRaises((ValueError, PermissionError)):
            self.prepare_create()
        self.assertNotIn(self.public / self.native.content_ref, observed)
        self.assertFalse(self.path.parent.exists())

    def test_native_binding_visibility_scope_and_identity_are_immutable_in_revision(self):
        self.create()
        before = self.files()
        for fields in ({'native_text_binding': None}, {'visibility': 'public'},
                       {'semantic_scope': {**self.record['semantic_scope'], 'identity_criterion': 'Different use'}},
                       {'record_id': 'tos.occurrence.other'}):
            with self.subTest(fields=fields), self.assertRaises(PermissionError):
                self.run_command({'operation': 'prepare-revise', 'fields': fields, 'forms': self.forms, 'reason': 'Synthetic illicit scope.'})
        self.assertEqual(self.files(), before)

    def test_lost_context_or_binding_invalidates_prepared_creation(self):
        self.prepare_create()
        original = self.context_path.read_bytes()
        self.context_path.write_bytes(original + b'\n')
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.creation)
        self.context_path.write_bytes(original)
        path = self.public / self.native.rights_ref
        path.write_bytes(path.read_bytes() + b'\n')
        with self.assertRaises((source.JournalConflict, ValueError)):
            self.run_command(self.creation)
        self.assertFalse(self.path.parent.exists())

    def test_form_mandatory_context_cannot_be_removed_and_all_current_forms_rebind(self):
        self.create()
        self.prepare_form()
        self.run_command(self.form_request)
        with self.assertRaises(ValueError):
            self.revise(apply=False)
        self.prepare_form(0)
        change = self.form_request['changes'][0]
        change['form']['bindings'] = {'wording': change['form']['bindings']['wording']}
        with self.assertRaises(ValueError):
            self.run_command(self.form_request)

    def test_concurrent_package_and_duplicate_identity_are_not_overwritten(self):
        self.prepare_create()
        self.path.parent.mkdir(mode=0o700)
        (self.path.parent / 'unrelated.txt').write_bytes(b'owned by another writer')
        (self.path.parent / 'unrelated.txt').chmod(0o600)
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.creation)
        self.assertEqual((self.path.parent / 'unrelated.txt').read_bytes(), b'owned by another writer')

    def test_changed_package_and_corrupt_archive_refuse_replay(self):
        created = self.create()
        self.revise()
        inspected = self.run_command({'operation': 'inspect-version', 'source': created['source']})
        blob = self.store / inspected['files']['occurrence.json']['archive_path']
        blob.write_bytes(blob.read_bytes() + b'\n')
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.revision)
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.creation)

    def test_record_replay_rejects_modified_output_form_even_with_coherent_form_history(self):
        self.create()
        self.revise()
        form_path = self.path.parent / 'occurrence.human-forms.json'
        forms = json.loads(form_path.read_bytes())
        forms['forms'][0]['creator_id'] = 'agent:synthetic-different-creator'
        form_path.write_bytes(encode(forms))
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.revision)
        history_path = self.path.parent / private.revisions.HISTORY
        history = json.loads(history_path.read_bytes())
        history['receipts'][0]['forms'] = [source._form_ref(forms['forms'][0])]
        history_path.write_bytes(encode(history))
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.revision)
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.creation)

    def test_form_replay_rejects_modified_form_and_receipt_that_disagree_with_request(self):
        self.create()
        self.prepare_form(0)
        self.run_command(self.form_request)
        form_path = self.path.parent / 'occurrence.human-forms.json'
        forms = json.loads(form_path.read_bytes())
        forms['forms'][0]['creator_id'] = 'agent:synthetic-different-creator'
        forms['growth_history'][0]['results'] = [source._form_ref(forms['forms'][0])]
        form_path.write_bytes(encode(forms))
        with self.assertRaises(source.JournalCorruption):
            self.run_command(self.form_request)

    def test_failing_exchange_keeps_current_bytes_and_retry_uses_retained_archive(self):
        self.create()
        before = self.files()
        self.revise(apply=False)
        with patch.object(private.revisions, '_exchange', side_effect=OSError('synthetic publication failure')):
            with self.assertRaises(OSError):
                self.run_command(self.revision)
        self.assertEqual(self.files(), before)
        self.assertEqual(list(self.store.glob('.owner-source-*.pending')), [])
        self.assertEqual(self.run_command(self.revision)['source']['version'], 2)

    def test_full_file_budget_still_leaves_room_for_archive_manifest(self):
        self.create()
        files = self.files()
        for index in range(private.revisions.MAX_FILES - len(files)):
            files[f'synthetic-{index}.json'] = encode({'synthetic': index})
        context = OwnerLocalSourceContext.load(self.context_path)
        revision = private.revisions._revision(files)
        subject = source.metadata_subject(self.record)
        relative = private._archive(context, self.config, files, subject, revision)
        receipt = {'archive_path': relative.as_posix(), 'previous_source': subject.ref, 'previous_revision': revision}
        self.assertEqual(private._read_archive(context, self.config, receipt)[0], files)

    def test_metadata_only_other_semantic_profile_needs_no_fabricated_native_binding(self):
        self.record.pop('native_text_binding')
        self.record.update(schema_version='tos_semantic_description_record_v1', record_type='crosscutting-concept',
                           record_id='tos.crosscutting-concept.synthetic.private')
        self.record.pop('semantic_content')
        self.config.update(profile_type_id='tos.entity.crosscutting-concept', record_id=self.record['record_id'],
                           source_binding=None, source_path=self.source_ref.replace('occurrence.json', 'crosscutting-concept.json'))
        self.config['source_access']['read_scope'] = 'metadata_only'
        self.source_ref = self.config['source_path']
        self.path = self.store / self.source_ref
        self.write_owner()
        (self.public / self.native.content_ref).unlink()
        result = self.create()
        self.assertEqual(result['source']['id'], self.record['record_id'])
        self.assertEqual(json.loads(self.path.read_bytes()), self.record)

    def test_native_semantic_entity_and_other_private_subject_reserve_their_identities(self):
        native_ref = 'ToS/source-witnesses/native/semantic-annotation.synthetic.json'
        self.native.write_json(native_ref, {'schema_version': 'tos_semantic_annotation_packet_v2',
            'entities': [{'entity_id': self.record['record_id']}]})
        with self.assertRaises(source.JournalConflict):
            self.prepare_create()
        (self.public / native_ref).unlink()
        other = self.store / (self.prefix + 'descriptions/other/occurrence.json')
        other.parent.mkdir(mode=0o700)
        other.write_bytes(encode(self.record))
        other.chmod(0o600)
        with self.assertRaises(source.JournalConflict):
            self.prepare_create()
        self.assertFalse(self.path.parent.exists())

    def test_command_identity_cannot_be_reused_across_creation_forms_and_record_revision(self):
        self.create()
        self.prepare_form()
        self.form_request['command_id'] = self.creation['command_id']
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.form_request)
        self.prepare_form(0)
        self.run_command(self.form_request)
        self.revise(apply=False)
        self.revision['command_id'] = self.form_request['command_id']
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.revision)

    def test_record_operation_names_cannot_bypass_form_delegation_inside_apply(self):
        self.create()
        self.prepare_form(0)
        for operation in ('source.create', 'record.revise'):
            self.config['allowed_operations'] = [operation]
            self.write_owner()
            described = self.run_command({'operation': 'describe'})
            request = copy.deepcopy(self.form_request)
            request.update(expected_configuration=described['owner_configuration'],
                           expected_dependencies=described['expected_dependencies'])
            request['changes'][0]['operation'] = operation
            before = self.files()
            with self.subTest(operation=operation), self.assertRaises(PermissionError):
                self.run_command(request)
            self.assertEqual(self.files(), before)

    def test_private_input_cannot_escalate_operation_or_choose_a_public_path(self):
        for key, value in (('allowed_operations', ['source.create', 'publish']),
                           ('allowed_fields', ['visibility']),
                           ('source_path', 'ToS/source-witnesses/semantic/new/occurrence.json'),
                           ('source_path', self.source_ref.replace('sid-' + '8' * 32, 'sid-' + '7' * 32))):
            original = copy.deepcopy(self.config)
            self.config[key] = value
            self.write_owner()
            try:
                with self.assertRaises((ValueError, PermissionError)):
                    self.run_command({'operation': 'describe'})
            finally:
                self.config = original
                self.write_owner()

    def test_commit_rechecks_staged_bytes_and_mode_not_only_source_dependencies(self):
        self.prepare_create()
        original = private._stage
        def stage(context, files):
            result = original(context, files)
            (result / 'occurrence.json').chmod(0o644)
            return result
        with patch.object(private, '_stage', side_effect=stage), self.assertRaises((ValueError, PermissionError)):
            self.run_command(self.creation)
        self.assertFalse(self.path.parent.exists())
        self.assertEqual(list(self.store.glob('.owner-source-*.pending')), [])

    def test_process_exit_before_publish_leaves_unpublished_package_and_retry_creates_once(self):
        self.prepare_create()
        request_path = self.base / 'request.json'
        request_path.write_bytes(encode({'schema_version': 'tos_local_source_command_v1', **self.creation}))
        request_path.chmod(0o600)
        code = """import json, os, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import source_commands as source
source._publish_new_directory = lambda *args: os._exit(79)
source.run_local_command(Path(sys.argv[2]), json.loads(Path(sys.argv[3]).read_bytes()))
"""
        run = subprocess.run([sys.executable, '-c', code,
            str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'), str(self.owner), str(request_path)],
            capture_output=True, timeout=60)
        self.assertEqual(run.returncode, 79, run.stderr.decode())
        self.assertFalse(self.path.parent.exists())
        stages = list(self.store.glob('.owner-source-*.pending'))
        self.assertEqual(len(stages), 1)
        before = {path.name: path.read_bytes() for path in stages[0].iterdir()}
        result = self.run_command(self.creation)
        self.assertFalse(result['replayed'])
        self.assertTrue(self.run_command(self.creation)['replayed'])
        self.assertEqual({path.name: path.read_bytes() for path in stages[0].iterdir()}, before)

    def test_source_schema_change_and_revoked_access_block_exact_retry(self):
        self.create()
        before = self.files()
        contract = self.public / 'ToS/contracts/occurrence-description-record.schema.json'
        contract.write_bytes(contract.read_bytes() + b'\n')
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.creation)
        self.config['source_access']['access_allowed'] = False
        self.write_owner()
        with self.assertRaises((ValueError, PermissionError)):
            self.run_command(self.creation)
        self.assertEqual(self.files(), before)

    def test_replay_rechecks_late_rights_and_package_drift_before_current_materializations(self):
        self.create()
        self.prepare_form(0)
        self.run_command(self.form_request)
        original = private._dependencies
        calls = 0
        def dependencies(*args, **kwargs):
            nonlocal calls
            value = original(*args, **kwargs)
            calls += 1
            if calls == 2:
                path = self.public / self.native.rights_ref
                path.write_bytes(path.read_bytes() + b'\n')
            return value
        before = self.files()
        with patch.object(private, '_dependencies', side_effect=dependencies), self.assertRaises((ValueError, source.JournalConflict)):
            self.run_command(self.form_request)
        self.assertEqual(self.files(), before)

    def test_creation_replay_rechecks_config_after_original_bytes_are_verified(self):
        self.create()
        original = private._creation_replay
        def replay(*args, **kwargs):
            value = original(*args, **kwargs)
            self.owner.write_bytes(self.owner.read_bytes() + b'\n')
            return value
        with patch.object(private, '_creation_replay', side_effect=replay), self.assertRaises(source.JournalConflict):
            self.run_command(self.creation)

    def test_new_private_native_packet_flows_into_occurrence_creation_forms_and_revision(self):
        from test_source_text_unit_commands import NativeUnitCommandTests
        native_writer = NativeUnitCommandTests()
        native_writer.setUp()
        self.addCleanup(native_writer.doCleanups)
        copy_contracts(native_writer.public)
        native_writer.prepare()
        native_writer.run_command(native_writer.request)
        packet_bytes = native_writer.path.read_bytes()
        packet = json.loads(packet_bytes)
        binding = copy.deepcopy(native_writer.fixture.binding)
        unit, segmentation = packet['units'][0], packet['segmentations'][0]
        binding.update(packet_ref=native_writer.source_ref, packet_sha256=source._digest(packet_bytes)[7:],
            packet_id=packet['packet_id'], packet_version=packet['packet_version'],
            segmentation_id=segmentation['segmentation_id'], segmentation_version=segmentation['segmentation_version'],
            unit_id=unit['unit_id'], unit_version=unit['unit_version'], ordered_anchor_refs=unit['ordered_anchor_refs'])
        self.public, self.store, self.native = native_writer.public, native_writer.private, native_writer.fixture
        self.prefix, self.context_path = native_writer.prefix, native_writer.context_path
        self.source_ref = self.prefix + 'descriptions/synthetic/occurrence.json'
        self.path = self.store / self.source_ref
        self.path.parent.parent.mkdir(mode=0o700)
        self.record = occurrence(binding)
        self.record['visibility'] = 'local_only'
        self.config.update(source_context_ref=str(self.context_path), source_path=self.source_ref,
                           source_binding=binding, record_id=self.record['record_id'])
        self.write_owner()
        created = self.create()
        self.assertEqual(created['source']['id'], self.record['record_id'])
        self.assertEqual(created['materializations'][0]['state'], 'ready')
        revised = self.revise()
        self.assertEqual(revised['source']['version'], 2)
        self.assertEqual(json.loads(self.path.read_bytes())['native_text_binding'], binding)
        self.assertEqual(native_writer.path.read_bytes(), packet_bytes)
        self.assertEqual(packet['reviews'], [])
        self.assertFalse(revised['grants_admission'])


if __name__ == '__main__':
    unittest.main()

"""Source commands against real metadata copies; no historical review verdicts."""
from __future__ import annotations

import copy
import hashlib
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
sys.path.insert(0, str(MECHANIC))
import source_commands as commands
from knowledge_assessment import Record


class SourceCommandTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.relative = 'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json'
        self.source = self.root / self.relative
        self.source.parent.mkdir(parents=True)
        self.source.write_bytes((ROOT / self.relative).read_bytes())
        self.target = self.source.with_name('work.human-forms.json')
        self.target.write_bytes((ROOT / self.relative).with_name('work.human-forms.json').read_bytes())
        self.original_source = self.source.read_bytes()
        self.original_set = json.loads(self.target.read_bytes())
        self.creator = 'source-command-test-account'
        self.config = {'schema_version': 'tos_local_source_command_owner_v1', 'uid': os.getuid(),
            'principal_id': self.creator, 'source_root': str(self.root), 'source_path': self.relative,
            'authority_ref': 'test-only:operator-delegated-form-writing-not-assessment',
            'allowed_form_ids': [form['form_id'] for form in self.original_set['forms']] + ['tos.form.test.new'],
            'allowed_operations': list(commands.OPERATIONS), 'expires_at': '2099-01-01T00:00:00Z'}
        self.owner = self.root / 'owner.json'
        self.save_config()

    def save_config(self):
        self.owner.write_text(json.dumps(self.config))

    def describe(self):
        return commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})

    def request(self, changes=None, command_id='test:1'):
        context = self.describe()
        old = json.loads(self.target.read_bytes())['forms'][0] if self.target.exists() else self.original_set['forms'][0]
        form = {**copy.deepcopy(old), 'form_version': old['form_version'] + 1,
                'creator_id': self.creator, 'revises': commands._form_ref(old)}
        return {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply', 'command_id': command_id,
            'expected_source': context['source'], 'expected_revision': context['revision'],
            'expected_configuration': context['owner_configuration'],
            'changes': changes or [{'operation': 'form.revise', 'expected_form': commands._form_ref(old), 'form': form}]}

    def run_request(self, request):
        return commands.run_local_command(self.owner, request)

    def test_real_metadata_revision_is_atomic_retains_history_and_replays_after_restart(self):
        request = self.request()
        changed = self.run_request(request)
        self.assertFalse(changed['replayed'])
        self.assertFalse(changed['grants_admission'])
        stored = json.loads(self.target.read_bytes())
        self.assertEqual(stored['prior_forms'], [self.original_set['forms'][0]])
        self.assertEqual(stored['forms'][1:], self.original_set['forms'][1:])
        self.assertEqual(stored['forms'][0], request['changes'][0]['form'])
        self.assertEqual(stored['growth_history'], [changed['receipt']])
        self.assertEqual(changed['materializations'][0]['state'], 'ready')
        self.assertEqual(changed['materializations'][0]['display_text'], json.loads(self.original_source)['preferred_label'])
        self.assertIsNone(changed['materializations'][0]['admission'])
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stderr)
        replay = json.loads(process.stdout)
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['revision'], changed['revision'])
        self.assertEqual(replay['receipt'], changed['receipt'])
        self.assertEqual(self.source.read_bytes(), self.original_source)

    def test_create_and_revise_together_or_neither_and_no_implicit_acceptance(self):
        request = self.request()
        proposed = {**copy.deepcopy(request['changes'][0]['form']), 'form_id': 'tos.form.test.new',
            'form_version': 1, 'revises': None, 'role': 'statement',
            'content': {'kind': 'freeform', 'text': 'Test proposal, not a verified historical assertion.'}}
        request['changes'].append({'operation': 'form.create', 'expected_form': None, 'form': proposed})
        before = self.target.read_bytes()
        invalid = copy.deepcopy(request)
        invalid['changes'][1]['form']['subject']['digest'] = 'sha256:' + '0' * 64
        with self.assertRaises(commands.JournalConflict):
            self.run_request(invalid)
        self.assertEqual(self.target.read_bytes(), before)
        result = self.run_request(request)
        self.assertEqual(len(result['receipt']['results']), 2)
        self.assertEqual(len(result['forms']), 4)
        view = result['materializations'][-1]
        self.assertEqual(view['state'], 'unavailable')
        self.assertIsNone(view['display_text'])
        self.assertFalse(view['performs_semantic_assessment'])
        self.assertEqual(json.loads(self.target.read_bytes())['forms'][-1], proposed)

    def test_new_set_uses_the_existing_source_adapter_without_a_second_store(self):
        self.target.unlink()  # Only a temporary fixture, never the source repo.
        form = {**copy.deepcopy(self.original_set['forms'][0]), 'creator_id': self.creator}
        request = self.request([{'operation': 'form.create', 'expected_form': None, 'form': form}])
        result = self.run_request(request)
        self.assertIsNone(result['receipt']['previous_revision'])
        self.assertEqual(result['materializations'][0]['state'], 'ready')
        self.assertEqual(json.loads(self.target.read_bytes())['prior_forms'], [])

    def test_discovered_field_prepares_a_source_bound_change_without_json_path_guessing(self):
        context = self.describe()
        field = next(field for field in context['source_fields'] if field['field_id'] == 'metadata.variant-name:0')
        self.assertEqual(field['language'], 'ru')
        self.assertNotIn('pointer', field)
        before = self.target.read_bytes()
        prepared = commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare', 'form_id': 'tos.form.test.new', 'field_id': field['field_id']})
        self.assertEqual(self.target.read_bytes(), before)
        self.assertEqual(prepared['prepared_change']['operation'], 'form.create')
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply', 'command_id': 'discovered-copy',
            'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
            'expected_configuration': prepared['owner_configuration'], 'changes': [prepared['prepared_change']]}
        result = self.run_request(request)
        view = result['materializations'][-1]
        self.assertEqual(view['state'], 'ready')
        self.assertEqual(view['language'], 'ru')
        self.assertEqual(view['display_text'], json.loads(self.original_source)['variant_labels'][0]['value'])
        self.assertFalse(view['standalone_reading'])
        self.assertEqual({item['binding']['pointer'] for item in view['context']},
            {'/identity_status', '/same_as_posture', '/variant_labels/0/language',
             '/variant_labels/0/source_ref', '/variant_labels/0/status'})
        revision = commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare', 'form_id': 'tos.form.test.new', 'field_id': field['field_id']})
        self.assertEqual(revision['prepared_change']['operation'], 'form.revise')
        self.assertEqual(revision['prepared_change']['form']['revises'], result['forms'][-1])

    def test_source_correction_rebinds_selected_forms_and_preserves_unmodified_stale_forms(self):
        source = json.loads(self.original_source)
        source['record_version'] += 1
        source['variant_labels'][0]['future/a~b'] = {'unknown': None, 'negative': False}
        self.source.write_text(json.dumps(source))
        identifier = self.original_set['forms'][1]['form_id']
        prepared = commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare', 'form_id': identifier, 'field_id': 'metadata.variant-name:0'})
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply', 'command_id': 'source-corrected',
            'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
            'expected_configuration': prepared['owner_configuration'], 'changes': [prepared['prepared_change']]}
        result = self.run_request(request)
        self.assertEqual([view['state'] for view in result['materializations']], ['stale', 'ready', 'stale'])
        context = result['materializations'][1]['context']
        unknown = next(item for item in context if item['binding']['pointer'].endswith('/future~1a~0b'))
        self.assertEqual(unknown['value'], {'unknown': None, 'negative': False})
        stored = json.loads(self.target.read_bytes())
        self.assertEqual(stored['prior_forms'], [self.original_set['forms'][1]])
        self.assertEqual(stored['forms'][0], self.original_set['forms'][0])

    def test_current_catalogue_metadata_preparation_covers_each_supported_record_without_writes(self):
        manifest = json.loads((ROOT / 'ToS/source-witnesses/catalog/catalog.manifest.json').read_bytes())
        supported = {'agent', 'place', 'organization', 'work', 'expression', 'edition', 'collection', 'item'}
        seen = set()
        for kind, path in manifest['record_files'].items():
            if kind not in supported:
                continue  # Object links have a separate schema and semantic owner.
            for line in (ROOT / path).read_text().splitlines():
                row = json.loads(line)
                source_path = ROOT / row['source_record_ref']
                original = source_path.read_bytes()
                source = json.loads(original)
                with self.subTest(record=row['record_id']):
                    self.assertEqual(hashlib.sha256(commands._canonical(source)).hexdigest(), row['record_sha256'])
                    fields = commands.metadata_field_catalog(source)
                    self.assertTrue(fields)
                    changes = [commands.prepare_metadata_change(source, None, 'test:source-copy-not-assessment',
                        'tos.form.test.catalog.' + str(index), field['field_id']) for index, field in enumerate(fields)]
                    record = Record.from_payload(source['record_id'], source['record_version'], source)
                    forms = commands._apply(None, record, changes)
                    views = commands.materialize_metadata_forms(source, forms, access_allowed=True)
                    self.assertEqual(len(views), len(fields))
                    for field, view in zip(fields, views):
                        self.assertEqual(view['state'], 'ready')
                        self.assertFalse(view['performs_semantic_assessment'])
                        self.assertIsNone(view['admission'])
                        if field['field_id'] == 'metadata.preferred-name':
                            self.assertEqual(view['display_text'], source['preferred_label'])
                        elif field['field_id'] == 'metadata.source-note':
                            self.assertEqual(view['display_text'], source['notes'])
                        else:
                            self.assertEqual(view['display_text'], source['variant_labels'][int(field['field_id'].split(':')[1])]['value'])
                        guards = {item['binding']['pointer']: item['value'] for item in view['context']}
                        self.assertEqual(guards['/identity_status'], source['identity_status'])
                    self.assertEqual(source_path.read_bytes(), original)
                seen.add(kind)
        self.assertEqual(seen, supported)

    def test_live_writer_busy_and_midcommand_configuration_change_do_not_publish(self):
        request = self.request()
        before = self.target.read_bytes()
        with commands._locked(self.target):
            with self.assertRaises(commands.JournalBusy):
                with commands._locked(self.target, timeout=0):
                    self.fail('a competing lock must not be acquired')
        original = commands._apply
        def changed_config(*args):
            value = original(*args)
            self.config['allowed_operations'] = []
            self.save_config()
            return value
        with patch.object(commands, '_apply', side_effect=changed_config):
            with self.assertRaises(commands.JournalConflict):
                self.run_request(request)
        self.assertEqual(self.target.read_bytes(), before)

    def test_stale_versions_identity_reuse_and_scope_cannot_write(self):
        before = self.target.read_bytes()
        for field in ('expected_configuration', 'expected_revision'):
            request = self.request()
            request[field] = 'sha256:' + '0' * 64
            with self.subTest(field=field), self.assertRaises(commands.JournalConflict):
                self.run_request(request)
        for field, value in [('creator_id', 'impostor'), ('form_id', 'tos.form.outside-scope')]:
            request = self.request()
            request['changes'][0]['form'][field] = value
            with self.subTest(field=field), self.assertRaises(PermissionError):
                self.run_request(request)
        request = self.request()
        request['changes'][0]['form']['form_version'] += 1
        with self.assertRaises(commands.JournalConflict):
            self.run_request(request)
        self.assertEqual(self.target.read_bytes(), before)
        request = self.request()
        result = self.run_request(request)
        another = self.request()
        with self.assertRaises(commands.JournalConflict):
            self.run_request(another)
        self.assertEqual(self.describe()['revision'], result['revision'])

    def test_revocation_applies_to_replay_and_source_change_does_not_resurrect(self):
        request = self.request()
        self.run_request(request)
        source = json.loads(self.source.read_bytes())
        source['notes'] += ' Temporary source correction.'
        source['record_version'] += 1
        self.source.write_text(json.dumps(source))
        replay = self.run_request(request)
        self.assertTrue(replay['replayed'])
        self.assertTrue(all(view['state'] == 'stale' for view in replay['materializations']))
        self.assertNotEqual(replay['source'], replay['receipt']['source'])
        self.config['allowed_operations'] = []
        self.save_config()
        before = self.target.read_bytes()
        with self.assertRaises(PermissionError):
            self.run_request(request)
        self.assertEqual(self.target.read_bytes(), before)

    def test_source_copy_cannot_omit_guards_or_turn_unknown_language_into_original(self):
        for mutate in (lambda form: form['bindings'].pop('identity_status'),
                       lambda form: form.update(language='de'),
                       lambda form: form['bindings']['wording'].update(pointer='/absent')):
            request = self.request()
            mutate(request['changes'][0]['form'])
            before = self.target.read_bytes()
            with self.assertRaises(ValueError):
                self.run_request(request)
            self.assertEqual(self.target.read_bytes(), before)

    def test_competing_writers_and_failures_before_or_after_publication(self):
        first = self.request(command_id='first')
        second = self.request(command_id='second')
        def run(request):
            try:
                return self.run_request(request)
            except commands.JournalConflict:
                return None
        with ThreadPoolExecutor(2) as pool:
            results = list(pool.map(run, [first, second]))
        self.assertEqual(sum(result is not None for result in results), 1)
        before = self.target.read_bytes()
        request = self.request(command_id='next')
        with patch.object(commands.os, 'replace', side_effect=OSError('before publish')):
            with self.assertRaises(OSError):
                self.run_request(request)
        self.assertEqual(self.target.read_bytes(), before)
        self.assertEqual(list(self.target.parent.glob('*.pending')), [])
        real_publish = commands._publish
        def publish_then_fail(path, raw):
            real_publish(path, raw)
            raise OSError('response lost after publication')
        with patch.object(commands, '_publish', side_effect=publish_then_fail):
            with self.assertRaises(OSError):
                self.run_request(request)
        self.assertTrue(self.run_request(request)['replayed'])
        self.assertEqual(len(json.loads(self.target.read_bytes())['growth_history']), 2)

    def test_corrupt_history_cannot_be_hidden_and_cli_errors_do_not_echo_input(self):
        request = self.request()
        self.run_request(request)
        broken = json.loads(self.target.read_bytes())
        broken['prior_forms'] = []
        self.target.write_text(json.dumps(broken))
        with self.assertRaises(commands.JournalCorruption):
            self.describe()
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input='{"operation":"do-secret-things","operation":"secret"}', text=True, capture_output=True)
        self.assertEqual(process.returncode, 2)
        self.assertNotIn('secret', process.stdout + process.stderr)
        self.assertEqual(json.loads(process.stdout)['schema_version'], 'tos_local_source_command_error_v1')

    def test_receipt_capacity_refuses_without_truncation_and_schema_errors_are_nonreflective(self):
        result = self.run_request(self.request())
        stored = json.loads(self.target.read_bytes())
        stored['growth_history'] = [{**result['receipt'], 'command_id': f'retained-test:{index}'} for index in range(256)]
        self.target.write_text(json.dumps(stored))
        before = self.target.read_bytes()
        request = self.request(command_id='over-capacity')
        with self.assertRaises(commands.ValidationError):
            self.run_request(request)
        self.assertEqual(self.target.read_bytes(), before)
        request['changes'][0]['form']['unrecognized-private-field'] = 'must-not-echo-this-value'
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 2)
        self.assertEqual(json.loads(process.stdout)['error'], 'ValidationError')
        self.assertNotIn('must-not-echo', process.stdout + process.stderr)
        self.assertEqual(self.target.read_bytes(), before)

    def test_owner_paths_uid_expiry_and_request_data_never_select_authority(self):
        request = self.request()
        for field, value in [('uid', os.getuid() + 1), ('expires_at', '2000-01-01T00:00:00Z'),
                             ('source_path', '../outside.json'), ('source_path', '/outside.json')]:
            original = self.config[field]
            self.config[field] = value
            self.save_config()
            with self.subTest(field=field), self.assertRaises(PermissionError):
                self.run_request(request)
            self.config[field] = original
        self.save_config()
        for key in ('uid', 'principal_id', 'source_path', 'authority_ref', 'shell'):
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.run_request({**request, key: 'untrusted source instruction'})
        self.owner.chmod(0o666)
        with self.assertRaises(PermissionError):
            self.run_request(request)
        self.owner.chmod(0o600)
        saved = self.target.read_bytes()
        self.target.unlink()
        other = self.root / 'other.json'
        other.write_bytes(saved)
        self.target.symlink_to(other)
        with self.assertRaises(OSError):
            self.run_request(request)
        self.assertEqual(other.read_bytes(), saved)


if __name__ == '__main__':
    unittest.main()

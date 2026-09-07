"""Source commands against real metadata copies; no historical review verdicts."""
from __future__ import annotations

import copy
import hashlib
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
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

    def test_historical_record_uses_same_command_abi_without_source_rewrite_or_admission(self):
        self.relative = 'ToS/source-witnesses/history/synthetic/historical-event.json'
        self.source = self.root / self.relative
        self.source.parent.mkdir(parents=True)
        source = {'schema_version': 'tos_historical_record_v1', 'record_type': 'historical-event',
                  'record_id': 'tos.historical-event.command-fixture', 'record_version': 1,
                  'preferred_label': 'Условный эпизод, не исторический факт', 'variant_labels': [],
                  'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
                  'source_refs': ['test:synthetic'], 'external_identifiers': [],
                  'visibility': 'public_metadata_only'}
        self.source.write_text(json.dumps(source))
        before = self.source.read_bytes()
        self.target = self.source.with_name('historical-event.human-forms.json')
        self.config['source_path'] = self.relative
        self.save_config()
        prepared = commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare', 'form_id': 'tos.form.test.new', 'field_id': 'metadata.preferred-name'})
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply',
            'command_id': 'historical-form', 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_configuration': prepared['owner_configuration'],
            'changes': [prepared['prepared_change']]}
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
                                 input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stderr)
        result = json.loads(process.stdout)
        self.assertEqual(result['materializations'][0]['display_text'], source['preferred_label'])
        self.assertFalse(result['grants_admission'])
        self.assertEqual(self.source.read_bytes(), before)
        stored = self.target.read_bytes()
        self.source.write_text(json.dumps({**source, 'visibility': 'local_only'}))
        with self.assertRaisesRegex(PermissionError, 'visibility'):
            self.run_request(request)
        self.assertEqual(self.target.read_bytes(), stored)

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

    def test_explicit_field_language_survives_preparation_and_revision_without_becoming_original(self):
        source = json.loads(self.original_source)
        source['record_version'] += 1
        metadata = {'language': 'ru', 'script': 'Cyrl',
                    'source_ref': 'test:synthetic-language-declaration',
                    'qualification': {'independently_assessed': False, 'future': None}}
        source['field_languages'] = {'notes': metadata}
        self.source.write_text(json.dumps(source))
        before = self.source.read_bytes()
        identifier = self.original_set['forms'][2]['form_id']
        prepared = commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare', 'form_id': identifier, 'field_id': 'metadata.source-note'})
        form = prepared['prepared_change']['form']
        self.assertEqual((form['language'], form['script']), ('ru', 'Cyrl'))
        self.assertNotIn('language_context', form)  # A language tag does not declare an original or translation.
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply',
            'command_id': 'declared-field-language', 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_configuration': prepared['owner_configuration'],
            'changes': [prepared['prepared_change']]}
        missing = copy.deepcopy(request)
        missing['changes'][0]['form']['bindings'] = {key: value for key, value in form['bindings'].items()
            if value['pointer'] != '/field_languages/notes'}
        with self.assertRaises(ValueError):
            self.run_request(missing)
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stderr)
        result = json.loads(process.stdout)
        view = result['materializations'][2]
        self.assertEqual(view['state'], 'ready')
        self.assertEqual((view['language'], view['script']), ('ru', 'Cyrl'))
        self.assertEqual(view['display_text'], source['notes'])
        self.assertIsNone(view['admission'])
        self.assertNotIn('language_context', view)
        self.assertEqual(next(item['value'] for item in view['context']
                             if item['binding']['pointer'] == '/field_languages/notes'), metadata)
        stored = json.loads(self.target.read_bytes())
        self.assertEqual(stored['prior_forms'], [self.original_set['forms'][2]])
        self.assertEqual(self.source.read_bytes(), before)

    def test_field_language_metadata_is_explicit_and_bounded_not_inferred_from_expression_or_ui(self):
        source = json.loads(self.original_source)
        source['language'] = 'de'  # Language of an Expression is not language of its catalog notes.
        fields = commands.metadata_field_catalog(source)
        self.assertIsNone(next(field for field in fields if field['field_id'] == 'metadata.source-note')['language'])
        for metadata in ({'language': 'x-test', 'script': None},
                         {'language': None, 'script': 'Latn'},
                         {'language': 'i-enochian', 'script': None}):
            with self.subTest(metadata=metadata):
                source['field_languages'] = {'preferred_label': metadata}
                field = commands.metadata_field_catalog(source)[0]
                self.assertEqual((field['language'], field['script']), (metadata['language'], metadata['script']))
        for metadata in (None, {'notes': {'language': 'ru'}},
                         {'notes': {'language': 'ru\n', 'script': None}},
                         {'notes': {'language': 12, 'script': None}},
                         {'notes': {'language': 'ru', 'script': 'Cyrillic'}},
                         {'unowned-field': {'language': 'ru', 'script': None}}):
            with self.subTest(invalid=metadata):
                source['field_languages'] = metadata
                with self.assertRaises(ValueError):
                    commands.metadata_field_catalog(source)

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
                    # The current metadata corpus is connected, not merely
                    # preparable in a demonstration. Full source-copy coverage
                    # remains separate from any judgment of the copied prose.
                    persisted_path = source_path.with_name(source_path.stem + '.human-forms.json')
                    persisted = json.loads(persisted_path.read_bytes())
                    commands._validate_history(persisted)
                    persisted_views = commands.materialize_metadata_forms(source, persisted, access_allowed=True)
                    self.assertTrue(all(view['state'] == 'ready' and view['admission'] is None
                                        for view in persisted_views))
                    copied_fields = {form['bindings'][form['content']['slot']]['pointer']
                                     for form in persisted['forms'] if form['content']['kind'] == 'source-copy'}
                    self.assertTrue({field['pointer'] for field in fields}.issubset(copied_fields))
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


class HistoricalCreationTests(unittest.TestCase):
    def test_v2_creation_captures_own_provenance_atomically_and_replays_exact_bytes(self):
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            schema_ref = 'ToS/contracts/provenance-event-v2.schema.json'
            (root / schema_ref).write_bytes((ROOT / schema_ref).read_bytes())
            config.update(schema_version='tos_local_historical_create_owner_v2',
                          provenance_event_id='tos.event.creation-fixture')
            owner.write_text(json.dumps(config))
            for claim in request['claims']:
                claim['provenance_event_ref'] = config['provenance_event_id']
            discovery = commands.run_local_command(owner, {
                'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
            request['expected_configuration'] = discovery['owner_configuration']
            preview = commands.run_local_command(owner, {
                'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create',
                **{key: request[key] for key in ('record', 'claims', 'forms')}})
            request['expected_dependencies'] = preview['expected_dependencies']
            target = (root / config['source_path']).parent
            self.assertFalse(target.exists())
            invalid = copy.deepcopy(request)
            invalid['claims'][0]['provenance_event_ref'] = 'tos.event.not-delegated'
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, invalid)
            self.assertFalse(target.exists())
            with patch.object(commands, '_publish_new_directory', side_effect=OSError('interrupted')):
                with self.assertRaises(OSError):
                    commands.run_local_command(owner, request)
            self.assertFalse(target.exists())
            result = commands.run_local_command(owner, request)
            before = {path.name: path.read_bytes() for path in target.iterdir()}
            event = json.loads(before['source-create-provenance.jsonl'])
            commands._validator_for_provenance(root).validate(event)
            from validate_source_witness_foundation import _provenance_v2_semantic_issues
            self.assertEqual(_provenance_v2_semantic_issues(event), [])
            self.assertEqual(event['event_id'], config['provenance_event_id'])
            self.assertEqual(event['responsibility'][0]['agent_kind'], 'software')
            self.assertEqual(event['method']['model_invocations'], [])
            self.assertFalse(event['review_and_authority']['promotion_authorized'])
            self.assertEqual(json.loads(before['source-create-request.json']), request)
            for binding in [event['method']['configuration_binding'],
                            event['method']['environment']['environment_profile_binding']]:
                self.assertEqual(hashlib.sha256((root / binding['ref']).read_bytes()).hexdigest(), binding['sha256'])
            for entity in [*event['entities']['inputs'], *event['entities']['outputs']]:
                raw = (root / entity['entity_ref']).read_bytes()
                self.assertEqual(hashlib.sha256(raw).hexdigest(), entity['sha256'])
                self.assertEqual(len(raw), entity['size_bytes'])
            for name, spec in result['receipt']['files'].items():
                self.assertEqual(spec['sha256'], commands._digest(before[name]))
            projection = rebuild()
            self.assertTrue(any(node['properties'].get('source_event') == event for node in projection['nodes']))
            retry = commands.run_local_command(owner, request)
            self.assertTrue(retry['replayed'])
            self.assertEqual(retry['receipt'], result['receipt'])
            self.assertEqual({path.name: path.read_bytes() for path in target.iterdir()}, before)
            config['provenance_event_id'] = 'tos.event.reassigned'
            owner.write_text(json.dumps(config))
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, request)
            self.assertEqual({path.name: path.read_bytes() for path in target.iterdir()}, before)

    @contextmanager
    def creation(self):
        sys.path.insert(0, str(ROOT / 'tests'))
        from test_source_witness_bibliographic_graph import SourceWitnessBibliographicGraphTest
        fixture = SourceWitnessBibliographicGraphTest()
        with fixture.historical_fixture() as (root, history, real, old_claims, rebuild):
            rebuild()
            source = {**copy.deepcopy(history[0][1]), 'record_id': 'tos.historical-event.creation-fixture',
                      'extensions': {'unknown': {'negative': False, 'missing': None}}}
            claims = [{**copy.deepcopy(claim), 'claim_id': f'tos.claim.creation-fixture-{index}',
                       'subject_ref': source['record_id']} for index, claim in enumerate(old_claims)]
            relative = 'ToS/source-witnesses/history/new-subject/historical-event.json'
            config = {'schema_version': 'tos_local_historical_create_owner_v1', 'uid': os.getuid(),
                'principal_id': 'software:test-fixture', 'maker_type': 'software',
                'source_root': str(root), 'source_path': relative, 'record_id': source['record_id'],
                'authority_ref': 'synthetic-test-only:creation-not-assessment',
                'allowed_form_ids': ['tos.form.creation-name', 'tos.form.creation-hover'],
                'allowed_claim_ids': [claim['claim_id'] for claim in claims],
                'allowed_operations': [commands.CREATION_OPERATION], 'expires_at': '2099-01-01T00:00:00Z'}
            owner = root / 'owner.json'
            owner.write_text(json.dumps(config))
            context = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
            self.assertFalse(context['target_exists'])
            self.assertEqual(context['allowed_operations'], ['historical.create'])
            request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'historical.create',
                'command_id': 'synthetic:create-first', 'expected_configuration': context['owner_configuration'],
                'expected_source': None, 'expected_revision': None, 'record': source, 'claims': claims,
                'forms': [{'form_id': 'tos.form.creation-name', 'field_id': 'metadata.preferred-name'},
                          {'form_id': 'tos.form.creation-hover', 'field_id': 'metadata.source-note'}]}
            prepared = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare', 'record': source})
            self.assertFalse((root / relative).parent.exists())
            self.assertEqual(prepared['prepared_source'], Record.from_payload(source['record_id'], 1, source).ref)
            self.assertEqual({field['field_id'] for field in prepared['source_fields']},
                             {'metadata.preferred-name', 'metadata.source-note'})
            preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', **{key: request[key] for key in ('record', 'claims', 'forms')}})
            request['expected_dependencies'] = preview['expected_dependencies']
            self.assertFalse((root / relative).parent.exists())
            self.assertFalse((root / 'ToS/source-witnesses/.historical-create.writer.lock').exists())
            yield root, owner, config, request, rebuild, fixture

    def test_complete_creation_reaches_existing_catalog_graph_and_restart_without_admission(self):
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', **{key: request[key] for key in ('record', 'claims', 'forms')}})
            result = commands.run_local_command(owner, request)
            target = (root / config['source_path']).parent
            self.assertFalse(result['replayed'])
            self.assertFalse(result['grants_admission'])
            self.assertEqual(result['receipt']['files'], preview['prepared_files'])
            self.assertEqual(json.loads((root / config['source_path']).read_bytes()), request['record'])
            self.assertEqual([json.loads(line) for line in (target / 'historical-claims.jsonl').read_text().splitlines()], request['claims'])
            before = {path.name: path.read_bytes() for path in target.iterdir()}
            for name, spec in result['receipt']['files'].items():
                self.assertEqual(spec, {'sha256': commands._digest(before[name]), 'bytes': len(before[name])})
            projection = rebuild()
            graph, _, _ = fixture.historical_knowledge(root, projection)
            subject = next(node for node in graph['nodes'] if node.get('entity_id') == config['record_id'])
            self.assertEqual(subject['type_id'], 'tos.entity.historical-event')
            self.assertEqual(subject['attributes']['source_record'], request['record'])
            from tos_access.knowledge import focus_knowledge_node, select_human_forms
            selected = select_human_forms(subject, 'auto')['roles']['hover']['packet']
            self.assertEqual(selected['display_text'], request['record']['notes'])
            self.assertIsNone(selected['admission'])
            neighbors = focus_knowledge_node(graph, config['record_id'], depth=2)
            self.assertTrue({claim['object'] for claim in request['claims']}.issubset(
                {node['entity_id'] for node in neighbors['nodes']}))
            forms = commands._snapshot(root / config['source_path'])[-1]
            views = commands.materialize_metadata_forms(request['record'], forms, access_allowed=True)
            self.assertTrue(all(view['state'] == 'ready' and view['admission'] is None for view in views))
            for claim in request['claims']:
                self.assertTrue(any(node.get('entity_id') == claim['claim_id'] for node in graph['nodes']))
            process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(owner)],
                input=json.dumps(request), text=True, capture_output=True)
            self.assertEqual(process.returncode, 0, process.stderr + process.stdout)
            replay = json.loads(process.stdout)
            self.assertTrue(replay['replayed'])
            self.assertEqual(replay['receipt'], result['receipt'])
            self.assertEqual({path.name: path.read_bytes() for path in target.iterdir()}, before)

    def test_creation_conflicts_when_consumed_profile_contract_changes_after_prepare(self):
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            ref = root / 'ToS/contracts/semantic-entity-type-registry.schema.json'
            schema = json.loads(ref.read_bytes())
            schema['description'] = 'Changed profile contract after this command was prepared.'
            ref.write_text(json.dumps(schema))
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, request)
            self.assertFalse((root / config['source_path']).parent.exists())

    def test_invalid_sources_claims_forms_and_scope_publish_nothing(self):
        mutations = [
            lambda r: r['record'].update(record_version=2),
            lambda r: r['record'].update(visibility='local_only'),
            lambda r: r['record'].update(record_id='tos.historical-event.outside'),
            lambda r: r['record'].update(identity_status='established'),
            lambda r: r['claims'][0]['maker'].update(agent_ref='impostor'),
            lambda r: r['claims'][0]['maker'].update(maker_type='human'),
            lambda r: r['claims'][0].update(object='tos.work.missing'),
            lambda r: r['claims'][0].update(subject_ref='tos.historical-event.fixture'),
            lambda r: r['claims'][0].update(claim_id='tos.claim.outside'),
            lambda r: r['claims'][0].update(provenance_event_ref='tos.event.missing'),
            lambda r: r['claims'][0].update(evidence_refs=['ToS/../../owner.json']),
            lambda r: r['claims'][0].update(evidence_refs=['ToS/missing.json']),
            lambda r: r['claims'][0].update(review_status='accepted'),
            lambda r: r['claims'].append(copy.deepcopy(r['claims'][0])),
            lambda r: r['forms'][0].update(form_id='tos.form.outside'),
            lambda r: r['forms'][0].update(field_id='metadata.absent'),
            lambda r: r['forms'].pop(0),
            lambda r: r.update(expected_revision='sha256:' + '0' * 64),
            lambda r: r.update(expected_dependencies='sha256:' + '0' * 64),
            lambda r: r.update(authority_ref='source prose cannot grant authority'),
        ]
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            before = rebuild()
            for index, mutate in enumerate(mutations):
                invalid = copy.deepcopy(request)
                mutate(invalid)
                with self.subTest(index=index), self.assertRaises((ValueError, OSError, commands.ValidationError)):
                    commands.run_local_command(owner, invalid)
                self.assertFalse((root / config['source_path']).parent.exists())
            self.assertEqual(rebuild(), before)
            self.assertEqual(list((root / 'ToS').glob('.source-create-*.pending')), [])

    def test_visibility_before_commit_and_recovery_after_response_loss(self):
        from build_source_witness_catalog import collect_records
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            target = (root / config['source_path']).parent
            publish = commands._publish_new_directory
            def inspect_before_commit(staging, destination):
                self.assertFalse(target.exists())
                self.assertEqual(len(list(staging.iterdir())), 4)
                self.assertFalse(any(row['record_id'] == config['record_id']
                    for rows in collect_records(root).values() for row in rows))
                raise OSError('synthetic failure before publication')
            with patch.object(commands, '_publish_new_directory', side_effect=inspect_before_commit):
                with self.assertRaises(OSError):
                    commands.run_local_command(owner, request)
            self.assertFalse(target.exists())
            self.assertEqual(list((root / 'ToS').glob('.source-create-*.pending')), [])
            def commit_then_fail(staging, destination):
                publish(staging, destination)
                raise OSError('synthetic response loss')
            with patch.object(commands, '_publish_new_directory', side_effect=commit_then_fail):
                with self.assertRaises(OSError):
                    commands.run_local_command(owner, request)
            self.assertTrue(commands.run_local_command(owner, request)['replayed'])
            self.assertEqual(len(list(target.iterdir())), 4)

    def test_concurrency_no_replace_and_current_revocation(self):
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            target = (root / config['source_path']).parent
            rename = commands._publish_new_directory
            def create_empty_competitor(staging, destination):
                destination.mkdir()
                rename(staging, destination)
            with patch.object(commands, '_publish_new_directory', side_effect=create_empty_competitor):
                with self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
            self.assertEqual(list(target.iterdir()), [])
            target.rmdir()  # Exact empty synthetic competing directory only.
            with ThreadPoolExecutor(2) as pool:
                results = list(pool.map(lambda _: commands.run_local_command(owner, request), range(2)))
            self.assertEqual(sorted(result['replayed'] for result in results), [False, True])
            different = {**request, 'command_id': 'different-command'}
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, different)
            for field in ('allowed_claim_ids', 'allowed_form_ids'):
                limited = {**config, field: []}
                owner.write_text(json.dumps(limited))
                with self.subTest(field=field), self.assertRaises(PermissionError):
                    commands.run_local_command(owner, request)
            config['allowed_operations'] = []
            owner.write_text(json.dumps(config))
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, request)

    def test_abrupt_process_loss_leaves_only_invisible_staging_and_retry_does_not_delete_it(self):
        from build_source_witness_catalog import collect_records
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            code = ('import json, os, pathlib, sys; sys.path.insert(0, sys.argv[1]); '
                    'import source_commands as c; '
                    'c._publish_new_directory = lambda *args: os._exit(73); '
                    'c.run_local_command(pathlib.Path(sys.argv[2]), json.load(sys.stdin))')
            process = subprocess.run([sys.executable, '-c', code, str(MECHANIC), str(owner)],
                input=json.dumps(request), text=True, capture_output=True)
            self.assertEqual(process.returncode, 73, process.stderr + process.stdout)
            abandoned = list((root / 'ToS').glob('.source-create-*.pending'))
            self.assertEqual(len(abandoned), 1)
            saved = {p.name: p.read_bytes() for p in abandoned[0].iterdir()}
            self.assertFalse(any(row['record_id'] == config['record_id']
                for rows in collect_records(root).values() for row in rows))
            self.assertFalse(commands.run_local_command(owner, request)['replayed'])
            self.assertEqual({p.name: p.read_bytes() for p in abandoned[0].iterdir()}, saved)
            self.assertTrue(commands.run_local_command(owner, request)['replayed'])

    def test_dependency_drift_and_revocation_during_staging_refuse_publication(self):
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            original = commands._historical_creation
            calls = []
            def change_configuration(*args):
                output = original(*args)
                calls.append(True)
                if len(calls) == 2:
                    owner.write_text(json.dumps({**config, 'allowed_operations': []}))
                return output
            with patch.object(commands, '_historical_creation', side_effect=change_configuration):
                with self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
            self.assertFalse((root / config['source_path']).parent.exists())
            owner.write_text(json.dumps(config))
            calls.clear()
            def change_dependency(*args):
                if calls:
                    path = root / 'ToS/source-witnesses/places/chemnitz/place.json'
                    source = json.loads(path.read_bytes())
                    source['notes'] = 'Synthetic concurrent correction.'
                    source['record_version'] += 1
                    path.write_text(json.dumps(source))
                calls.append(True)
                return original(*args)
            with patch.object(commands, '_historical_creation', side_effect=change_dependency):
                with self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
            self.assertFalse((root / config['source_path']).parent.exists())
            self.assertEqual(list((root / 'ToS').glob('.source-create-*.pending')), [])
            with self.assertRaisesRegex(commands.JournalConflict, 'dependencies are stale'):
                commands.run_local_command(owner, request)

    def test_allocated_identity_collisions_and_symlinks_are_not_overwritten(self):
        with self.creation() as (root, owner, config, request, rebuild, fixture):
            source = root / config['source_path']
            other = root / 'ToS/source-witnesses/history/other-subject'
            other.mkdir()
            (other / source.name).write_text(json.dumps(request['record']))
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, request)
            (other / source.name).unlink()  # Only the deliberate synthetic duplicate.
            old_path = root / 'ToS/source-witnesses/history/fixture/historical-event.json'
            old = json.loads(old_path.read_bytes())
            subject = Record.from_payload(old['record_id'], old['record_version'], old)
            change = commands.prepare_metadata_change(old, None, config['principal_id'],
                request['forms'][0]['form_id'], 'metadata.preferred-name')
            old_forms = old_path.with_name('historical-event.human-forms.json')
            old_forms.write_text(json.dumps(commands._apply(None, subject, [change])))
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, request)
            old_forms.unlink()  # Only the deliberate synthetic colliding form.
            source.parent.symlink_to(other, target_is_directory=True)
            with self.assertRaises(OSError):
                commands.run_local_command(owner, request)
            self.assertEqual(list(other.iterdir()), [])


if __name__ == '__main__':
    unittest.main()

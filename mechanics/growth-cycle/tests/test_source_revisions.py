"""Versioned source corrections: synthetic semantics, real filesystem/CLI."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
sys.path.insert(0, str(MECHANIC))
import source_commands as commands
import source_revisions as revisions


class SourceRevisionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.relative = 'ToS/source-witnesses/history/synthetic/episode/historical-event.json'
        self.path = self.root / self.relative
        self.path.parent.mkdir(parents=True)
        (self.root / 'ToS/contracts').mkdir()
        registry = 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
        (self.root / registry).parent.mkdir(parents=True)
        (self.root / registry).write_bytes((ROOT / registry).read_bytes())
        contract = 'ToS/contracts/semantic-entity-type-registry.schema.json'
        (self.root / contract).write_bytes((ROOT / contract).read_bytes())
        for name in ('historical-record.schema.json', 'corpus-record.schema.json'):
            (self.root / 'ToS/contracts' / name).write_bytes((ROOT / 'ToS/contracts' / name).read_bytes())
        self.record = {'schema_version': 'tos_historical_record_v1', 'record_type': 'historical-event',
            'record_id': 'tos.historical-event.revision-fixture', 'record_version': 1,
            'preferred_label': 'Условный эпизод', 'notes': 'Не историческое свидетельство.',
            'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
            'source_refs': ['test:synthetic'], 'external_identifiers': [],
            'visibility': 'public_metadata_only', 'extensions': {'uninterpreted': [False, None, {'x': 3}]}}
        self.path.write_text(json.dumps(self.record, indent=3))
        self.selections = [{'form_id': 'tos.form.revision-name', 'field_id': 'metadata.preferred-name'},
                           {'form_id': 'tos.form.revision-note', 'field_id': 'metadata.source-note'}]
        changes = [commands.prepare_metadata_change(self.record, None, 'test:author', **item) for item in self.selections]
        forms = commands._apply(None, commands.Record.from_payload(self.record['record_id'], 1, self.record), changes)
        self.formpath = self.path.with_name('historical-event.human-forms.json')
        self.formpath.write_text(json.dumps(forms, indent=3))
        # Unknown companion bytes and historical provenance must not be rewritten.
        (self.path.parent / 'unrecognized.json').write_bytes(b'{ "unknown": true }\n')
        (self.path.parent / 'historical-claims.jsonl').write_bytes(b'')
        self.original = self.package()
        self.config = {'schema_version': 'tos_local_source_revision_owner_v1', 'uid': os.getuid(),
            'principal_id': 'test:reviser', 'source_root': str(self.root), 'source_path': self.relative,
            'record_id': self.record['record_id'], 'authority_ref': 'test:explicit-source-correction',
            'allowed_form_ids': [item['form_id'] for item in self.selections],
            'allowed_fields': ['notes', 'field_languages'], 'allowed_operations': ['record.revise'],
            'expires_at': '2099-01-01T00:00:00Z'}
        self.owner = self.root / 'owner.json'
        self.owner.write_text(json.dumps(self.config))

    def package(self):
        return {path.name: path.read_bytes() for path in self.path.parent.iterdir() if path.is_file()}

    def run_command(self, operation, **fields):
        return commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
                                                      'operation': operation, **fields})

    def request(self, command_id='synthetic:revision-1'):
        proposal = {'fields': {'notes': 'Уточнённая условная запись, всё ещё не свидетельство.',
                              'field_languages': {'notes': {'language': 'ru', 'script': 'Cyrl'}}},
                    'forms': self.selections, 'reason': 'Synthetic correction and explicit field language.'}
        preview = self.run_command('prepare-revise', **proposal)
        return {'schema_version': 'tos_local_source_command_v1', 'operation': 'record.revise',
            'command_id': command_id, 'expected_configuration': preview['owner_configuration'],
            'expected_source': preview['source'], 'expected_revision': preview['revision'],
            'expected_dependencies': preview['expected_dependencies'], **proposal}

    def test_revision_keeps_exact_old_package_forms_and_unknown_fields_and_replays_in_cli(self):
        request = self.request()
        self.assertEqual(self.package(), self.original)  # Preparation is read-only.
        result = commands.run_local_command(self.owner, request)
        current = json.loads(self.path.read_bytes())
        self.assertEqual(current, {**self.record, **request['fields'], 'record_version': 2})
        self.assertFalse(result['grants_admission'])
        self.assertFalse(result['replayed'])
        forms = json.loads(self.formpath.read_bytes())
        self.assertEqual(forms['prior_forms'], json.loads(self.original[self.formpath.name])['forms'])
        self.assertTrue(all(view['state'] == 'ready' for view in result['materializations']))
        self.assertTrue(all(view['admission'] is None for view in result['materializations']))
        prior = self.run_command('inspect-version', source=request['expected_source'])
        self.assertEqual(prior['record'], self.record)
        for name, binding in prior['files'].items():
            self.assertEqual((self.root / binding['archive_path']).read_bytes(), self.original[name])
        self.assertEqual((self.path.parent / 'unrecognized.json').read_bytes(), self.original['unrecognized.json'])
        after = self.package()
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stderr + process.stdout)
        retry = json.loads(process.stdout)
        self.assertTrue(retry['replayed'])
        self.assertEqual(retry['receipt'], result['receipt'])
        self.assertEqual(self.package(), after)

    def test_stale_input_scope_and_dropped_form_are_rejected_without_source_change(self):
        request = self.request()
        for modify in (
            lambda r: r['fields'].update(record_id='tos.historical-event.other'),
            lambda r: r.update(expected_revision='sha256:' + '0' * 64),
            lambda r: r.update(forms=r['forms'][:1]),
            lambda r: r['fields'].update(field_languages={'notes': {'language': 'ru'}}),
        ):
            invalid = copy.deepcopy(request)
            modify(invalid)
            with self.subTest(request=invalid), self.assertRaises((ValueError, PermissionError, commands.ValidationError)):
                commands.run_local_command(self.owner, invalid)
            self.assertEqual(self.package(), self.original)

    def test_interruption_before_commit_and_response_loss_after_commit_keep_one_history(self):
        request = self.request()
        with patch.object(revisions, '_exchange', side_effect=RuntimeError('interruption before commit')):
            with self.assertRaises(RuntimeError):
                commands.run_local_command(self.owner, request)
        self.assertEqual(self.package(), self.original)
        # Archive was durable first, but an uncommitted copy is not revision history.
        with self.assertRaises(commands.JournalConflict):
            self.run_command('inspect-version', source=request['expected_source'])
        exchange = revisions._exchange
        def response_loss(staging, target):
            exchange(staging, target)
            raise RuntimeError('response lost after atomic commit')
        with patch.object(revisions, '_exchange', side_effect=response_loss):
            with self.assertRaises(RuntimeError):
                commands.run_local_command(self.owner, request)
        self.assertEqual(json.loads(self.path.read_bytes())['record_version'], 2)
        retry = commands.run_local_command(self.owner, request)
        self.assertTrue(retry['replayed'])
        self.assertEqual(len(json.loads((self.path.parent / revisions.HISTORY).read_bytes())['receipts']), 1)
        self.assertEqual(self.run_command('inspect-version', source=request['expected_source'])['record'], self.record)

    def test_competing_revisions_cannot_overwrite_or_duplicate_the_winner(self):
        first = self.request()
        second = copy.deepcopy(first)
        second['command_id'] = 'synthetic:competing-revision'
        second['fields']['notes'] = 'Competing synthetic wording.'
        def attempt(request):
            try:
                return commands.run_local_command(self.owner, request)
            except commands.JournalConflict:
                return None
        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(attempt, [first, second]))
        self.assertEqual(sum(result is not None for result in results), 1)
        winner = next(result for result in results if result is not None)
        self.assertEqual(json.loads(self.path.read_bytes())['record_version'], 2)
        self.assertEqual(len(json.loads((self.path.parent / revisions.HISTORY).read_bytes())['receipts']), 1)
        self.assertEqual(self.run_command('inspect-version', source=winner['receipt']['previous_source'])['record'], self.record)

    def test_revocation_on_retry_and_corrupt_archive_fail_closed(self):
        request = self.request()
        commands.run_local_command(self.owner, request)
        after = self.package()
        self.config['allowed_operations'] = []
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, request)
        self.assertEqual(self.package(), after)
        self.config['allowed_operations'] = ['record.revise']
        self.owner.write_text(json.dumps(self.config))
        prior = self.run_command('inspect-version', source=request['expected_source'])
        archived = self.root / prior['files'][self.path.name]['archive_path']
        archived.write_bytes(b'corrupt synthetic archive')
        with self.assertRaises(commands.JournalCorruption):
            commands.run_local_command(self.owner, request)
        with self.assertRaises(commands.JournalCorruption):
            self.run_command('inspect-version', source=request['expected_source'])
        self.assertEqual(self.package(), after)

    def test_multiple_revisions_return_exact_versions_and_do_not_duplicate_source_catalog(self):
        first = self.request()
        commands.run_local_command(self.owner, first)
        second = self.request('synthetic:revision-2')
        second['fields']['notes'] = 'Third source version, still synthetic.'
        commands.run_local_command(self.owner, second)
        self.assertEqual(json.loads(self.path.read_bytes())['record_version'], 3)
        self.assertEqual(self.run_command('inspect-version', source=first['expected_source'])['record'], self.record)
        self.assertEqual(self.run_command('inspect-version', source=second['expected_source'])['record']['record_version'], 2)
        from build_source_witness_catalog import collect_records
        records = collect_records(self.root)
        self.assertEqual(len(records[self.record['record_type']]), 1)
        self.assertEqual(records[self.record['record_type']][0]['record_id'], self.record['record_id'])

    def test_changed_companion_symlink_nested_package_and_budget_are_not_silently_discarded(self):
        request = self.request()
        companion = self.path.parent / 'unrecognized.json'
        companion.write_bytes(b'{"new": "concurrent editor"}')
        changed = self.package()
        with self.assertRaises(commands.JournalConflict):
            commands.run_local_command(self.owner, request)
        self.assertEqual(self.package(), changed)
        companion.unlink()
        companion.symlink_to(self.owner)
        with self.assertRaises(PermissionError):
            self.run_command('describe')
        companion.unlink()
        companion.mkdir()
        with self.assertRaises(PermissionError):
            self.run_command('describe')
        companion.rmdir()
        companion.write_bytes(self.original['unrecognized.json'])
        with patch.object(revisions, 'MAX_PACKAGE_BYTES', 1), self.assertRaises(ValueError):
            self.run_command('describe')
        self.assertEqual(self.package(), self.original)

    def test_real_process_loss_leaves_only_inactive_staging_and_retry_preserves_original(self):
        request = self.request()
        program = '''import json, os, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import source_commands, source_revisions
source_revisions._exchange = lambda *args: os._exit(73)
source_commands.run_local_command(Path(sys.argv[2]), json.load(sys.stdin))
'''
        process = subprocess.run([sys.executable, '-c', program, str(MECHANIC), str(self.owner)],
                                 input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 73, process.stderr)
        self.assertEqual(self.package(), self.original)
        abandoned = set((self.root / 'ToS').glob('.source-revision-*.pending'))
        self.assertEqual(len(abandoned), 1)
        commands.run_local_command(self.owner, request)
        self.assertEqual(set((self.root / 'ToS').glob('.source-revision-*.pending')), abandoned)
        self.assertEqual(self.run_command('inspect-version', source=request['expected_source'])['record'], self.record)

    def test_process_loss_after_exchange_retains_both_history_and_abandoned_old_copy(self):
        request = self.request()
        program = '''import json, os, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import source_commands, source_revisions
exchange = source_revisions._exchange
def lose_response(*args):
    exchange(*args)
    os._exit(74)
source_revisions._exchange = lose_response
source_commands.run_local_command(Path(sys.argv[2]), json.load(sys.stdin))
'''
        process = subprocess.run([sys.executable, '-c', program, str(MECHANIC), str(self.owner)],
                                 input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 74, process.stderr)
        self.assertEqual(json.loads(self.path.read_bytes())['record_version'], 2)
        abandoned = list((self.root / 'ToS').glob('.source-revision-*.pending'))
        self.assertEqual(len(abandoned), 1)
        self.assertEqual({p.name: p.read_bytes() for p in abandoned[0].iterdir()}, self.original)
        result = commands.run_local_command(self.owner, request)
        self.assertTrue(result['replayed'])
        self.assertTrue(abandoned[0].exists())
        self.assertEqual(self.run_command('inspect-version', source=request['expected_source'])['record'], self.record)

    def test_form_only_writer_and_record_revision_share_one_stable_writer_boundary(self):
        revision_request = self.request()
        form_config = {key: value for key, value in self.config.items() if key not in {'record_id', 'allowed_fields', 'profile_type_id', 'record_type'}}
        form_config.update(schema_version='tos_local_source_command_owner_v1', allowed_operations=['form.revise'])
        form_owner = self.root / 'form-owner.json'
        form_owner.write_text(json.dumps(form_config))
        form_context = commands.run_local_command(form_owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare', **self.selections[0]})
        form_request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply',
            'command_id': 'synthetic:form-writer', 'expected_source': form_context['source'],
            'expected_revision': form_context['revision'], 'expected_configuration': form_context['owner_configuration'],
            'changes': [form_context['prepared_change']]}
        def attempt(item):
            owner, request = item
            try:
                return commands.run_local_command(owner, request)
            except commands.JournalConflict:
                return None
        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(attempt, [(form_owner, form_request), (self.owner, revision_request)]))
        self.assertEqual(sum(result is not None for result in results), 1)
        self.assertTrue(all(view['state'] == 'ready' for view in self.run_command('describe')['materializations']))

    def test_corrected_created_subject_reaches_existing_catalog_graph_and_form_reader(self):
        from test_source_commands import HistoricalCreationTests
        fixture = HistoricalCreationTests()
        with fixture.creation() as (root, owner, config, initial, rebuild, _):
            created = commands.run_local_command(owner, initial)
            path = root / config['source_path']
            original = {p.name: p.read_bytes() for p in path.parent.iterdir()}
            delegated = {key: value for key, value in config.items() if key not in {'maker_type', 'allowed_claim_ids'}}
            delegated.update(schema_version=commands.REVISION_CONFIG, allowed_operations=['record.revise'],
                             allowed_fields=['notes', 'field_languages'])
            owner.write_text(json.dumps(delegated))
            proposal = {'fields': {'notes': 'Уточнение синтетической записи, не оценка истории.',
                'field_languages': {'notes': {'language': 'ru', 'script': 'Cyrl'}}},
                'forms': initial['forms'], 'reason': 'Synthetic end-to-end source correction.'}
            preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-revise', **proposal})
            applied = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'record.revise', 'command_id': 'synthetic:graph-source-correction',
                'expected_source': preview['source'], 'expected_revision': preview['revision'],
                'expected_configuration': preview['owner_configuration'],
                'expected_dependencies': preview['expected_dependencies'], **proposal})
            graph = rebuild()
            matching = [node for node in graph['nodes'] if node['properties'].get('record_id') == config['record_id']]
            self.assertEqual(len(matching), 1)
            self.assertEqual(matching[0]['properties']['source_record'], json.loads(path.read_bytes()))
            self.assertEqual(applied['source']['version'], created['receipt']['source']['version'] + 1)
            self.assertEqual((path.parent / 'historical-claims.jsonl').read_bytes(), original['historical-claims.jsonl'])
            self.assertEqual((path.parent / 'source-create-receipt.json').read_bytes(), original['source-create-receipt.json'])
            views = matching[0]['properties']['human_forms']
            self.assertTrue(any(view['language'] == 'ru' and view['display_text'] == proposal['fields']['notes'] for view in views))
            owner.write_text(json.dumps(config))
            retry = commands.run_local_command(owner, initial)
            self.assertTrue(retry['replayed'])
            self.assertEqual(retry['receipt'], created['receipt'])


class NativeSourceRevisionTests(SourceRevisionTests):
    """Native Corpus metadata shares the retained correction contract, not grants."""

    def setUp(self):
        super().setUp()
        oldpath, oldforms = self.path, self.formpath
        self.relative = str(Path(self.relative).with_name('agent.json'))
        self.path = self.root / self.relative
        self.formpath = self.path.with_name('agent.human-forms.json')
        self.record.pop('visibility')
        self.record.pop('extensions')
        self.record.update(schema_version='tos_corpus_record_v1', record_type='agent',
                           record_id='tos.agent.revision-fixture')
        self.record['external_identifiers'] = [{'scheme': 'synthetic', 'value': 'stable-external-id',
            'source_ref': 'test:synthetic', 'status': 'unverified'}]
        self.record['field_languages'] = {'preferred_label': {'language': 'x-test', 'script': None,
            'uninterpreted': [False, None, 0, '', {'warning': 'source text is not an instruction'}]}}
        self.path.write_text(json.dumps(self.record, indent=3))
        changes = [commands.prepare_metadata_change(self.record, None, 'test:author', **item) for item in self.selections]
        forms = commands._apply(None, commands.Record.from_payload(self.record['record_id'], 1, self.record), changes)
        self.formpath.write_text(json.dumps(forms, indent=3))
        oldpath.unlink()
        oldforms.unlink()
        self.config.update(schema_version=commands.CORPUS_REVISION_CONFIG, record_type='agent',
                           source_path=self.relative, record_id=self.record['record_id'])
        self.owner.write_text(json.dumps(self.config))
        self.original = self.package()

    def test_native_descriptor_and_separate_permission_are_required(self):
        described = self.run_command('describe')
        self.assertEqual(described['record_type'], 'agent')
        self.assertEqual(described['source_profile']['source_scope'], 'public_metadata_only')
        request = self.request()
        for config in (
            {**self.config, 'record_type': 'place'},
            {**self.config, 'allowed_operations': ['source.create']},
            {**self.config, 'allowed_operations': ['form.revise']},
            {**self.config, 'allowed_fields': ['external_identifiers']},
            {**self.config, 'allowed_fields': ['variant_labels']},
            {**self.config, 'schema_version': commands.REVISION_CONFIG},
            {**self.config, 'schema_version': commands.CORPUS_CONFIG},
        ):
            self.owner.write_text(json.dumps(config))
            with self.subTest(config=config), self.assertRaises((PermissionError, ValueError)):
                commands.run_local_command(self.owner, request)
            self.assertEqual(self.package(), self.original)
        self.owner.write_text(json.dumps(self.config))
        for field, value in (
            ('record_id', 'tos.agent.other'), ('record_version', 9), ('record_type', 'place'),
            ('schema_version', 'tos_corpus_record_v99'), ('identity_status', 'verified'),
            ('same_as_posture', 'reviewed_equivalence'), ('external_identifiers', []),
            ('variant_labels', []), ('visibility', 'public'), ('supersedes_ref', 'tos.agent.other'),
            ('responsibility_claim_refs', ['tos.claim.synthetic']),
        ):
            invalid = copy.deepcopy(request)
            invalid['fields'][field] = value
            with self.subTest(field=field), self.assertRaises(PermissionError):
                commands.run_local_command(self.owner, invalid)
            self.assertEqual(self.package(), self.original)
        schema = self.root / 'ToS/contracts/corpus-record.schema.json'
        schema.write_bytes(schema.read_bytes() + b'\n')
        with self.assertRaises(commands.JournalConflict):
            commands.run_local_command(self.owner, request)
        self.assertEqual(self.package(), self.original)

    def test_native_correction_refuses_foreign_schema_or_visibility_without_reinterpreting_it(self):
        for fields in ({'visibility': 'public'}, {'visibility': 'local_only'},
                       {'schema_version': 'tos_historical_record_v1'}, {'record_type': 'place'}):
            self.path.write_text(json.dumps({**self.record, **fields}))
            before = self.package()
            with self.subTest(fields=fields), self.assertRaises((PermissionError, ValueError, commands.ValidationError)):
                self.run_command('describe')
            self.assertEqual(self.package(), before)

    def test_created_native_families_correct_names_without_changing_identity_claims_or_creation_history(self):
        from test_source_commands import HistoricalCreationTests
        fixture = HistoricalCreationTests()
        for kind in ('agent', 'place', 'organization', 'work'):
            with self.subTest(kind=kind), fixture.native_creation(kind) as (root, owner, config, initial, rebuild, graph_fixture):
                created = commands.run_local_command(owner, initial)
                path = root / config['source_path']
                original = {p.name: p.read_bytes() for p in path.parent.iterdir()}
                old_graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
                old = next(n for n in old_graph['nodes'] if n['entity_id'] == config['record_id'])
                delegated = {key: value for key, value in config.items() if key not in {'maker_type', 'provenance_event_id'}}
                delegated.update(schema_version=commands.CORPUS_REVISION_CONFIG, allowed_operations=['record.revise'],
                                 allowed_fields=['preferred_label', 'notes', 'field_languages'])
                owner.write_text(json.dumps(delegated))
                proposal = {'fields': {'preferred_label': 'Уточнённое синтетическое имя',
                    'notes': 'Описание исправлено, предмет и его связи остаются теми же.',
                    'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                        'notes': {'language': 'ru', 'script': 'Cyrl'}}},
                    'forms': initial['forms'], 'reason': 'Synthetic native descriptive correction.'}
                preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                    'operation': 'prepare-revise', **proposal})
                applied = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                    'operation': 'record.revise', 'command_id': 'synthetic:native-' + kind,
                    'expected_source': preview['source'], 'expected_revision': preview['revision'],
                    'expected_configuration': preview['owner_configuration'],
                    'expected_dependencies': preview['expected_dependencies'], **proposal})
                self.assertEqual(applied['source']['id'], created['receipt']['source']['id'])
                self.assertEqual(applied['source']['version'], 2)
                current = json.loads(path.read_bytes())
                self.assertEqual(current, {**initial['record'], **proposal['fields'], 'record_version': 2})
                graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
                node = next(n for n in graph['nodes'] if n['entity_id'] == config['record_id'])
                self.assertEqual(node['id'], old['id'])
                self.assertNotEqual(node['content_revision'], old['content_revision'])
                self.assertEqual(node['attributes']['source_record'], current)
                self.assertEqual(graph['relations'], old_graph['relations'])
                self.assertEqual({f['state'] for f in node['attributes']['human_forms']}, {'ready'})
                self.assertTrue(any(f['display_text'] == proposal['fields']['preferred_label']
                                    for f in node['attributes']['human_forms']))
                for name, raw in original.items():
                    if name not in {path.name, path.stem + '.human-forms.json'}:
                        self.assertEqual((path.parent / name).read_bytes(), raw)
                owner.write_text(json.dumps(config))
                retry = commands.run_local_command(owner, initial)
                self.assertTrue(retry['replayed'])
                self.assertEqual(retry['receipt'], created['receipt'])

    def test_connected_agent_keeps_claim_endpoints_and_context_when_its_copied_description_changes(self):
        sys.path.insert(0, str(ROOT / 'tests'))
        from test_source_witness_bibliographic_graph import SourceWitnessBibliographicGraphTest
        fixture = SourceWitnessBibliographicGraphTest()
        with fixture.historical_fixture() as (root, history, real, claims, rebuild):
            # The named identity is a temporary copy. Its surrounding Claims
            # are explicitly synthetic associations, not historical assertions.
            relative = 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json'
            path = root / relative
            original = path.read_bytes()
            record = json.loads(original)
            before, _, _ = fixture.historical_knowledge(root, rebuild())
            claim_path = root / 'ToS/source-witnesses/history/fixture/historical-claims.jsonl'
            original_claim_stream = claim_path.read_bytes()
            prior = next(n for n in before['nodes'] if n['entity_id'] == record['record_id'])
            self.assertTrue(any(e['to_id'] == prior['id'] or e['from_id'] == prior['id'] for e in before['relations']))
            forms = [{'form_id': 'tos.form.synthetic-copied-agent', 'field_id': 'metadata.preferred-name'}]
            owner = root / 'synthetic-native-owner.json'
            owner.write_text(json.dumps({**self.config, 'source_root': str(root), 'source_path': relative,
                'record_id': record['record_id'], 'allowed_fields': ['preferred_label', 'field_languages'],
                'allowed_form_ids': [forms[0]['form_id']]}))
            proposal = {'fields': {'preferred_label': 'Синтетическое имя временной копии',
                'field_languages': {**record.get('field_languages', {}),
                                    'preferred_label': {'language': 'ru', 'script': 'Cyrl'}}},
                'forms': forms, 'reason': 'Only exercise copied-record identity; not a historical correction.'}
            preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-revise', **proposal})
            commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'record.revise', 'command_id': 'synthetic:connected-agent-description',
                'expected_source': preview['source'], 'expected_revision': preview['revision'],
                'expected_configuration': preview['owner_configuration'],
                'expected_dependencies': preview['expected_dependencies'], **proposal})
            # Check before the fixture's catalog rebuild, which serializes its
            # captured Claims again and would otherwise mask an accidental write.
            self.assertEqual(claim_path.read_bytes(), original_claim_stream)
            after, _, _ = fixture.historical_knowledge(root, rebuild())
            current = next(n for n in after['nodes'] if n['entity_id'] == record['record_id'])
            self.assertEqual(current['id'], prior['id'])
            self.assertEqual(current['attributes']['source_record']['external_identifiers'], record['external_identifiers'])
            for kind in ('identity_status', 'same_as_posture'):
                self.assertEqual(current['attributes']['source_record'][kind], record[kind])
            def endpoint_identity(graph):
                return [(e['id'], e['from_id'], e['to_id'], e['predicate_id']) for e in graph['relations']]
            self.assertEqual(endpoint_identity(after), endpoint_identity(before))
            source_claims = lambda graph: {n['entity_id']: n['attributes']['source_claim'] for n in graph['nodes']
                                           if n.get('semantics', {}).get('claim')}
            self.assertEqual(source_claims(after), source_claims(before))
            self.assertTrue(source_claims(after))
            archived = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'inspect-version', 'source': preview['source']})
            self.assertEqual((root / archived['files']['agent.json']['archive_path']).read_bytes(), original)


class ProfileSourceRevisionTests(SourceRevisionTests):
    """The same transactional contract must hold for a declared Letter profile."""

    def setUp(self):
        super().setUp()
        for name in ('document-record.schema.json', 'source-metadata-record.schema.json'):
            (self.root / 'ToS/contracts' / name).write_bytes((ROOT / 'ToS/contracts' / name).read_bytes())
        oldpath, oldforms = self.path, self.formpath
        self.relative = str(Path(self.relative).with_name('letter.json'))
        self.path = self.root / self.relative
        self.formpath = self.path.with_name('letter.human-forms.json')
        self.record.update(schema_version='tos_document_record_v1', record_type='letter',
                           record_id='tos.letter.revision-fixture')
        self.path.write_text(json.dumps(self.record, indent=3))
        changes = [commands.prepare_metadata_change(self.record, None, 'test:author', **item) for item in self.selections]
        forms = commands._apply(None, commands.Record.from_payload(self.record['record_id'], 1, self.record), changes)
        self.formpath.write_text(json.dumps(forms, indent=3))
        oldpath.unlink()
        oldforms.unlink()
        self.config.update(schema_version='tos_local_profile_revision_owner_v1',
            profile_type_id='tos.entity.letter', source_path=self.relative, record_id=self.record['record_id'])
        self.owner.write_text(json.dumps(self.config))
        self.original = self.package()

    def test_profile_authority_and_schema_drift_fail_without_writes(self):
        request = self.request()
        registry_path = self.root / 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
        registry = json.loads(registry_path.read_bytes())
        registry['registry_version'] += 1
        registry_path.write_text(json.dumps(registry))
        with self.assertRaises(commands.JournalConflict):
            commands.run_local_command(self.owner, request)
        self.assertEqual(self.package(), self.original)
        self.config['profile_type_id'] = 'tos.entity.document'
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            self.run_command('describe')
        self.assertEqual(self.package(), self.original)
        self.config['profile_type_id'] = 'tos.entity.letter'
        self.owner.write_text(json.dumps(self.config))
        request = self.request()
        schema = self.root / 'ToS/contracts/document-record.schema.json'
        schema.write_bytes(schema.read_bytes() + b'\n')
        with self.assertRaises(commands.JournalConflict):
            commands.run_local_command(self.owner, request)
        self.assertEqual(self.package(), self.original)

    def test_legacy_revision_grant_does_not_gain_profile_authority(self):
        self.config.pop('profile_type_id')
        self.config['schema_version'] = commands.REVISION_CONFIG
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(ValueError):
            self.run_command('describe')
        self.assertEqual(self.package(), self.original)

    def test_profile_revision_does_not_grant_identity_visibility_or_schema_transitions(self):
        request = self.request()
        for field, value in (('record_type', 'document'), ('schema_version', 'tos_document_record_v99'),
                             ('record_version', 12), ('identity_status', 'verified'),
                             ('visibility', 'local_only'), ('supersedes_ref', 'tos.letter.other')):
            invalid = copy.deepcopy(request)
            invalid['fields'][field] = value
            with self.subTest(field=field), self.assertRaises(PermissionError):
                commands.run_local_command(self.owner, invalid)
            self.assertEqual(self.package(), self.original)
        # An existing unsupported record is refused, not silently rewritten
        # using the nearest schema version or a looser metadata envelope.
        self.path.write_text(json.dumps({**self.record, 'schema_version': 'tos_document_record_v99'}))
        before = self.package()
        with self.assertRaises(ValueError):
            self.run_command('describe')
        self.assertEqual(self.package(), before)


if __name__ == '__main__':
    unittest.main()

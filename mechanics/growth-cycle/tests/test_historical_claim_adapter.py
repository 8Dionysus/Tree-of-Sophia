"""Captured historical Claim growth; synthetic assertions, real owner commands."""
import copy
from contextlib import contextmanager
import json
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
import source_commands as source
import source_historical_claims as legacy
import claim_revisions as claims
import source_revisions as packages
from claim_version_reader import ClaimVersionReader
import claim_version_reader as version_reader
import test_source_commands as fixtures


class HistoricalClaimAdapterTests(unittest.TestCase):
    @contextmanager
    def fixture(self):
        with fixtures.HistoricalCreationTests().creation() as (root, owner, config, request, rebuild, fixture):
            for ref in (*legacy.CONTRACT_REFS, 'ToS/contracts/provenance-event-v2.schema.json'):
                (root / ref).parent.mkdir(parents=True, exist_ok=True)
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            config.update(schema_version='tos_local_historical_create_owner_v2',
                          provenance_event_id='tos.event.creation-fixture')
            date = {**copy.deepcopy(request['claims'][0]), 'claim_id': 'tos.claim.creation-fixture-date',
                'predicate': 'historical_dating', 'object': {'kind': 'date-assertion', 'role': 'historical-time',
                    'calendar': None, 'year_numbering': None, 'certainty': 'uncertain', 'value': '1900-01-01',
                    'source_wording': {'text': 'Synthetic date only', 'language': 'en'}},
                'qualifiers': {'synthetic_evidence_limit': 'No historical claim; temporal reader fixture only.'}}
            request['claims'].append(date)
            config['allowed_claim_ids'].append(date['claim_id'])
            owner.write_text(json.dumps(config))
            for claim in request['claims']:
                claim['provenance_event_ref'] = config['provenance_event_id']
            preview = source.run_local_command(owner, {'schema_version': contract_request,
                'operation': 'prepare-create', **{key: request[key] for key in ('record', 'claims', 'forms')}})
            request.update(expected_configuration=preview['owner_configuration'],
                           expected_dependencies=preview['expected_dependencies'])
            result = source.run_local_command(owner, request)
            self.root, self.creation_owner, self.creation_config, self.creation_request = root, owner, config, request
            self.record_path = root / config['source_path']
            self.path = self.record_path.with_name(legacy.BASENAME)
            self.original = packages._package(self.path.parent)
            self.rebuild, self.graph_fixture = rebuild, fixture
            self.assertEqual(result['receipt']['files'][legacy.BASENAME]['sha256'], source._digest(self.original[legacy.BASENAME]))
            yield

    def grant(self, index):
        identity = self.creation_request['claims'][index]['claim_id']
        config = {'schema_version': legacy.REVISION_CONFIG, 'uid': os.getuid(),
            'principal_id': 'software:synthetic-reviser', 'source_root': str(self.root),
            'source_path': self.path.relative_to(self.root).as_posix(), 'claim_id': identity,
            'historical_record_id': self.creation_config['record_id'],
            'creation_receipt_sha256': source._digest(self.original['source-create-receipt.json']),
            'authority_ref': 'test:separate-descriptive-legacy-grant', 'expires_at': '2099-01-01T00:00:00Z',
            'allowed_operations': ['claim.revise'], 'allowed_fields': ['qualifiers'],
            'allowed_qualifier_fields': sorted(legacy.QUALIFIER_FIELDS), 'allowed_evidence_refs': [],
            'allowed_form_ids': [f'tos.form.synthetic-legacy-{index}-{role}' for role in ('statement', 'name', 'caption', 'hover')],
            'allowed_form_field_ids': list(source.CLAIM_FORM_FIELDS)}
        owner = self.root / f'claim-owner-{index}.json'
        owner.write_text(json.dumps(config))
        return owner, config

    def proposal(self, index, text='Синтетическое утверждение; не исторический факт.'):
        return {'fields': {'qualifiers': {'statement': text, 'statement_language': 'ru', 'statement_script': 'Cyrl',
            'display_fields': {'schema_version': 'tos_claim_display_fields_v1', **{role:
                {'text': f'Условное описание {role}', 'language': 'ru', 'script': 'Cyrl'}
                for role in ('name', 'caption', 'hover')}}}},
            'forms': [{'form_id': f'tos.form.synthetic-legacy-{index}-{role}', 'field_id': 'claim.' + role}
                      for role in ('statement', 'name', 'caption', 'hover')],
            'reason': 'Synthetic descriptive evolution; no historical acceptance.'}

    def prepared(self, index, text='Синтетическое утверждение; не исторический факт.'):
        owner, config = self.grant(index)
        proposal = self.proposal(index, text)
        preview = source.run_local_command(owner, {'schema_version': contract_request,
            'operation': 'prepare-revise', **proposal})
        return owner, config, {'schema_version': contract_request, 'operation': 'claim.revise',
            'command_id': f'test:legacy-{index}-{preview["source"]["version"]}',
            'expected_source': preview['source'], 'expected_revision': preview['revision'],
            'expected_configuration': preview['owner_configuration'],
            'expected_dependencies': preview['expected_dependencies'], 'expected_inputs': preview['source_bindings'], **proposal}

    def event_revision(self, text):
        config = {key: self.creation_config[key] for key in ('uid', 'source_root', 'source_path', 'record_id', 'expires_at')}
        config.update(schema_version=source.REVISION_CONFIG, principal_id='test:record-reviser',
            authority_ref='test:separate-record-grant', allowed_operations=['record.revise'],
            allowed_fields=['notes'], allowed_form_ids=self.creation_config['allowed_form_ids'])
        owner = self.root / 'event-owner.json'; owner.write_text(json.dumps(config))
        proposal = {'fields': {'notes': text}, 'forms': self.creation_request['forms'], 'reason': 'Synthetic record correction only.'}
        preview = source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'prepare-revise', **proposal})
        request = {'schema_version': contract_request, 'operation': 'record.revise',
            'command_id': f'test:event-{preview["source"]["version"]}', 'expected_source': preview['source'],
            'expected_revision': preview['revision'], 'expected_configuration': preview['owner_configuration'],
            'expected_dependencies': preview['expected_dependencies'], **proposal}
        return source.run_local_command(owner, request)

    def test_interleaved_record_and_claim_versions_preserve_origin_and_reach_exact_reader(self):
        with self.fixture():
            self.event_revision('First independent event correction.')
            event_v2 = self.record_path.read_bytes()
            first_owner, _, first = self.prepared(0)
            before = packages._package(self.path.parent)
            first_result = source.run_local_command(first_owner, first)
            self.assertFalse(first_result['grants_admission'])
            self.assertEqual(self.record_path.read_bytes(), event_v2)
            sibling_lines = self.original[legacy.BASENAME].splitlines(keepends=True)[1:]
            self.assertEqual(self.path.read_bytes().splitlines(keepends=True)[1:], sibling_lines)
            self.assertEqual({view['state'] for view in first_result['materializations']}, {'ready'})
            owner, _, second = self.prepared(1)
            source.run_local_command(owner, second)
            self.event_revision('Second independent event correction between Claims.')
            owner, _, third = self.prepared(0, 'Вторая синтетическая формулировка; не новое событие.')
            result = source.run_local_command(owner, third)
            after = packages._package(self.path.parent)
            for name in legacy.CAPTURE:
                self.assertEqual(after[name], self.original[name])
            self.assertEqual(json.loads(self.record_path.read_bytes())['record_version'], 3)
            self.assertEqual(len(json.loads(after[claims.HISTORY])['receipts']), 3)
            self.assertTrue(source.run_local_command(first_owner, first)['replayed'])
            self.assertTrue(source.run_local_command(self.creation_owner, self.creation_request)['replayed'])
            self.assertEqual(packages._package(self.path.parent), after)
            self.assertEqual(legacy.verify_creation(self.root, self.path.relative_to(self.root).as_posix())['status'],
                             'verified-captured-historical-origin')
            graph, _, _ = self.graph_fixture.historical_knowledge(self.root, self.rebuild())
            node = next(node for node in graph['nodes'] if node['entity_id'] == first['expected_source']['id'])
            self.assertEqual({form['state'] for form in node['attributes']['human_forms']}, {'ready'})
            reader = ClaimVersionReader(self.root)
            for ref in (first['expected_source'], first_result['source'], result['source']):
                with self.subTest(ref=ref):
                    resolved = reader.resolve(ref)
                    self.assertEqual(resolved['status'], 'available', resolved)
                    self.assertEqual(claims._subject(resolved['record']).ref, ref)
            reader.verify_current()

    def test_old_grants_and_structural_rewrites_are_refused_without_mutation(self):
        with self.fixture():
            owner, config = self.grant(0)
            before = packages._package(self.path.parent)
            proposal = self.proposal(0)
            for field, value in (('object', 'tos.agent.foreign'), ('assertion_layer', 'bibliographic_assertion'),
                                 ('schema_version', 'tos_historical_context_claim_v1'),
                                 ('evidence_refs', ['test:new']), ('review_status', 'accepted')):
                with self.subTest(field=field):
                    invalid = {**proposal, 'fields': {field: value}}
                    with self.assertRaises((PermissionError, ValueError)):
                        source.run_local_command(owner, {'schema_version': contract_request,
                            'operation': 'prepare-revise', **invalid})
            for qualifiers in ({'participation_role': 'different'}, {'primary_letter_inspected': True},
                               {'nested_uninterpreted_source_field': None}):
                with self.subTest(qualifiers=qualifiers), self.assertRaises(PermissionError):
                    source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'prepare-revise',
                        **{**proposal, 'fields': {'qualifiers': qualifiers}}})
            with self.assertRaises(PermissionError):
                source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'prepare-revise',
                    **{**proposal, 'fields': None}})
            missing = {key: value for key, value in config.items() if key != 'allowed_form_field_ids'}
            owner.write_text(json.dumps(missing))
            with self.assertRaises(ValueError):
                source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'describe'})
            owner.write_text(json.dumps(config))
            for schema in (source.CLAIM_REVISION_CONFIG, source.CLAIM_VALUE_REVISION_CONFIG,
                           source.CLAIM_LAYER_REVISION_CONFIG):
                with self.subTest(schema=schema):
                    old = {key: value for key, value in config.items() if key not in legacy.CONFIG_FIELDS}
                    old['schema_version'] = schema
                    owner.write_text(json.dumps(old))
                    with self.assertRaises((PermissionError, ValueError)):
                        source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'describe'})
            for changed in ({'creation_receipt_sha256': 'sha256:' + '0' * 64},
                            {'historical_record_id': 'tos.historical-event.foreign'},
                            {'claim_id': 'tos.claim.foreign'}, {'expires_at': '2000-01-01T00:00:00Z'}):
                with self.subTest(changed=changed):
                    owner.write_text(json.dumps({**config, **changed}))
                    with self.assertRaises((PermissionError, ValueError)):
                        source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'describe'})
            self.assertEqual(packages._package(self.path.parent), before)

    def test_separate_form_writer_replay_and_whole_context_preserve_historical_source(self):
        with self.fixture():
            owner, config, request = self.prepared(0)
            source.run_local_command(owner, request)
            config = {key: value for key, value in config.items()
                      if key not in {'allowed_fields', 'allowed_qualifier_fields', 'allowed_evidence_refs'}}
            config.update(schema_version=legacy.FORM_CONFIG, allowed_operations=['form.create'],
                          allowed_form_ids=['tos.form.synthetic-historical-extra-name'], allowed_form_field_ids=['claim.name'])
            owner = self.root / 'form-owner.json'; owner.write_text(json.dumps(config))
            source_bytes = self.path.read_bytes()
            preview = source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'prepare',
                'form_id': config['allowed_form_ids'][0], 'field_id': 'claim.name'})
            request = {'schema_version': contract_request, 'operation': 'apply', 'command_id': 'test:legacy-form',
                'expected_source': preview['source'], 'expected_revision': preview['revision'],
                'expected_configuration': preview['owner_configuration'], 'changes': [preview['prepared_change']]}
            bad = copy.deepcopy(request)
            bad['changes'][0]['form']['bindings'].pop('context-0')
            with self.assertRaises((ValueError, source.JournalConflict)):
                source.run_local_command(owner, bad)
            self.assertEqual(self.path.read_bytes(), source_bytes)
            result = source.run_local_command(owner, request)
            view = next(view for view in result['materializations'] if view['role'] == 'name'
                        and view['form']['id'] == config['allowed_form_ids'][0])
            self.assertEqual(view['state'], 'ready')
            self.assertIsNone(view['admission'])
            self.assertEqual(self.path.read_bytes(), source_bytes)
            after = packages._package(self.path.parent)
            self.assertTrue(source.run_local_command(owner, request)['replayed'])
            self.assertTrue(source.run_local_command(self.creation_owner, self.creation_request)['replayed'])
            self.assertEqual(packages._package(self.path.parent), after)
            # Neither an old native form grant nor the new family can select a
            # narrower context, another field, or a foreign globally owned ID.
            for changed in ({'schema_version': source.CLAIM_DISPLAY_FORM_CONFIG},
                            {'allowed_form_field_ids': ['claim.caption']},
                            {'allowed_form_ids': self.creation_config['allowed_form_ids']}):
                with self.subTest(changed=changed):
                    owner.write_text(json.dumps({**config, **changed}))
                    with self.assertRaises((PermissionError, ValueError, source.JournalConflict)):
                        source.run_local_command(owner, request)
            self.assertEqual(self.path.read_bytes(), source_bytes)

    def test_stale_package_and_corrupt_retained_capture_fail_closed(self):
        with self.fixture():
            owner, _, stale = self.prepared(0)
            self.event_revision('Independent record change makes the entire prepared Claim package stale.')
            before = packages._package(self.path.parent)
            with self.assertRaises(source.JournalConflict):
                source.run_local_command(owner, stale)
            self.assertEqual(packages._package(self.path.parent), before)
            owner, _, request = self.prepared(0)
            with patch.object(packages, '_exchange', side_effect=OSError('synthetic prepublication loss')):
                with self.assertRaises(OSError):
                    source.run_local_command(owner, request)
            self.assertEqual(packages._package(self.path.parent), before)
            source.run_local_command(owner, request)
            current = packages._package(self.path.parent)
            history = json.loads(current[claims.HISTORY])
            archive = self.root / history['receipts'][0]['archive_path']
            manifest = json.loads((archive / 'manifest.json').read_bytes())
            blob = archive / manifest['files'][legacy.BASENAME]['blob']
            original_blob = blob.read_bytes()
            blob.write_bytes(original_blob + b' ')
            with self.assertRaises((source.JournalCorruption, ValueError)):
                source.run_local_command(owner, request)
            with self.assertRaises((source.JournalCorruption, ValueError)):
                source.run_local_command(self.creation_owner, self.creation_request)
            blob.write_bytes(original_blob)
            self.assertTrue(source.run_local_command(owner, request)['replayed'])
            capture = self.path.parent / 'source-create-environment.json'
            capture.write_bytes(current[capture.name] + b' ')
            with self.assertRaises((source.JournalCorruption, ValueError)):
                source.run_local_command(owner, request)
            capture.write_bytes(current[capture.name])
            self.assertEqual(packages._package(self.path.parent), current)

    def test_forged_retained_field_scope_and_missing_capture_do_not_become_legacy_authority(self):
        with self.fixture():
            index = next(index for index, claim in enumerate(self.creation_request['claims'])
                         if claim['predicate'] == 'historical_dating')
            owner, _, request = self.prepared(index)
            source.run_local_command(owner, request)
            self.rebuild()
            history_path = self.path.parent / claims.HISTORY
            original_history = history_path.read_bytes()
            history = json.loads(original_history)
            receipt = history['receipts'][0]
            # Keep the successor, archive and exact forms coherent. Merely
            # granting the old object again is still outside this family.
            receipt['request']['fields']['object'] = self.creation_request['claims'][index]['object']
            receipt['changed_fields'] = sorted(receipt['request']['fields'])
            receipt['request_digest'] = source._digest(source._canonical(receipt['request']))
            history_path.write_bytes(packages._encode(history))
            with self.assertRaises((source.JournalCorruption, ValueError)):
                source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'describe'})
            result = ClaimVersionReader(self.root).resolve(request['expected_source'])
            self.assertEqual(result['status'], 'corrupt', result)
            history_path.write_bytes(original_history)
            capture = self.path.parent / 'source-create-environment.json'
            capture_bytes = capture.read_bytes(); capture.unlink()
            with self.assertRaises((source.JournalCorruption, ValueError)):
                source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'describe'})
            result = ClaimVersionReader(self.root).resolve(request['expected_source'])
            self.assertEqual(result['status'], 'corrupt', result)
            capture.write_bytes(capture_bytes)
            self.assertTrue(source.run_local_command(owner, request)['replayed'])

    def test_exact_claim_reader_has_its_own_budget_and_does_not_certify_event_archives(self):
        with self.fixture():
            self.event_revision('A separate record history exists but is not the Claim reader subject.')
            owner, _, request = self.prepared(0)
            source.run_local_command(owner, request)
            self.rebuild()
            with patch.object(version_reader, 'MAX_TOTAL_BYTES', 1):
                result = ClaimVersionReader(self.root).resolve(request['expected_source'])
            self.assertEqual(result['status'], 'over-budget', result)
            self.assertIsNone(result['record'])
            event_history = json.loads((self.path.parent / packages.HISTORY).read_bytes())
            event_archive = self.root / event_history['receipts'][0]['archive_path']
            manifest = json.loads((event_archive / 'manifest.json').read_bytes())
            blob = event_archive / manifest['files'][self.record_path.name]['blob']
            old = blob.read_bytes(); blob.write_bytes(old + b' ')
            result = ClaimVersionReader(self.root).resolve(request['expected_source'])
            self.assertEqual(result['status'], 'available', result)
            self.assertFalse(result['provenance']['history']['record_history_verified'])
            with self.assertRaises((source.JournalCorruption, ValueError)):
                source.run_local_command(owner, {'schema_version': contract_request, 'operation': 'describe'})
            blob.write_bytes(old)


contract_request = 'tos_local_source_command_v1'

if __name__ == '__main__':
    unittest.main()

from __future__ import annotations

import copy
from contextlib import contextmanager
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import subprocess
import sys
import unittest
from unittest.mock import patch

import test_source_commands as fixtures
import source_commands as commands

ROOT = fixtures.ROOT


class SourceClaimCreationTests(unittest.TestCase):
    def test_temporal_profile_value_grammar_and_new_predicate_are_data_driven(self):
        from build_source_witness_catalog import collect_records
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        import assessment_journal
        with self.creation() as (root, owner, config, baseline, _, rebuild, fixture):
            ref = 'ToS/contracts/source-temporal-claim.schema.json'
            (root / ref).write_bytes((ROOT / ref).read_bytes())
            path = root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            registry = json.loads(path.read_bytes())
            extension = copy.deepcopy(next(r for r in registry['relations'] if r['relation_type_id'] == 'tos.relation.historical-dating'))
            extension.update(relation_type_id='tos.relation.synthetic-dating',
                source_mappings=[{'source_graph': 'source-claims', 'source_predicate_id': 'synthetic_dating', 'scope': 'claim-predicate'}])
            registry['relations'].append(extension)
            path.write_text(json.dumps(registry))
            for field, invalid in (('range_type_ids', ['tos.entity.historical-situation']),
                                   ('range_type_ids', ['tos.entity.work']),
                                   ('domain_type_ids', ['tos.entity.temporal-assertion'])):
                original = extension[field]
                extension[field] = invalid
                path.write_text(json.dumps(registry))
                with self.subTest(field=field, invalid=invalid), self.assertRaises(SourceProfileError):
                    SourceClaimProfiles(root)
                extension[field] = original
            path.write_text(json.dumps(registry))
            profiles = SourceClaimProfiles(root)
            objects = {record['record_id']: record for rows in collect_records(root).values() for record in rows}
            context = {'role': 'historical-time', 'calendar': None, 'year_numbering': None, 'certainty': 'uncertain',
                       'source_wording': {'text': 'Условная датировка; только проверка контракта.', 'language': 'ru'},
                       'extensions': {'unknown': [False, None, '', 0]}}
            values = [{**context, 'kind': 'date-assertion', 'value': '1886'},
                      {**context, 'kind': 'interval-assertion', 'interval': {'end': '1886'}},
                      {**context, 'kind': 'unknown-date', 'certainty': 'unknown'},
                      {**context, 'kind': 'relative-order', 'relative': {'relation': 'during', 'anchor_ref': 'tos.historical-process.fixture'}}]
            claims = [{**baseline, 'claim_id': f'tos.claim.synthetic-value-{i}', 'schema_version': 'tos_source_temporal_claim_v1',
                       'subject_ref': 'tos.historical-event.fixture', 'predicate': 'synthetic_dating', 'object': value,
                       'qualifiers': {'statement': 'Предложена условная датировка; не исторический факт.',
                                      'statement_language': 'ru', 'statement_script': 'Cyrl'}} for i, value in enumerate(values)]
            for claim in claims:
                profiles.validate(claim, objects)
            for mutation in ({'object': baseline['object']}, {'subject_ref': baseline['subject_ref']},
                             {'object': {**values[0], 'role': 'data-capture-time'}},
                             {'object': {**values[2], 'certainty': 'exact'}},
                             {'object': {**values[0], 'interval': {'start': '1886'}}},
                             {'object': {**values[-1], 'relative': {'relation': 'after', 'anchor_ref': 'tos.historical-state.absent'}}},
                             {'object': {**values[-1], 'relative': {'relation': 'after', 'anchor_ref': baseline['object']}}}):
                with self.subTest(mutation=mutation), self.assertRaises(SourceProfileError):
                    profiles.validate({**claims[0], **mutation}, objects)
            # A permissive profile schema cannot weaken the shared value grammar.
            extension['source_claim_profile']['schemas'][0]['schema_ref'] = 'ToS/contracts/source-claim-record.schema.json'
            path.write_text(json.dumps(registry))
            with self.assertRaisesRegex(SourceProfileError, 'temporal value contract'):
                SourceClaimProfiles(root).validate({**claims[0], 'object': {'kind': 'unknown-date'}}, objects)
            extension['source_claim_profile']['schemas'][0]['schema_ref'] = ref
            path.write_text(json.dumps(registry))
            config.update(schema_version='tos_local_claim_create_owner_v2', allowed_object_values=values,
                allowed_claim_ids=[c['claim_id'] for c in claims], allowed_subject_refs=[claims[0]['subject_ref']],
                allowed_object_refs=['tos.historical-process.fixture'], allowed_predicates=['synthetic_dating'])
            owner.write_text(json.dumps(config))
            prepare = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create', 'claims': claims}
            preview = commands.run_local_command(owner, prepare)
            request = {**prepare, 'operation': 'claims.create', 'command_id': 'synthetic:new-dating-profile',
                'expected_configuration': preview['owner_configuration'], 'expected_revision': None,
                'expected_dependencies': preview['expected_dependencies'], 'expected_inputs': preview['source_bindings']}
            commands.run_local_command(owner, request)
            selected = [claims[-1]['subject_ref'], values[-1]['relative']['anchor_ref']]
            bindings = [{'path': config['source_path'], 'record_id': claims[-1]['claim_id'], 'origin_id': 'test:claim'}]
            bindings += [{'path': objects[key]['source_record_ref'], 'record_id': key, 'origin_id': 'test:source'} for key in selected]
            resolved, _ = assessment_journal._source_records(root, bindings)
            self.assertEqual(resolved[0]['payload']['object'], values[-1])
            with self.assertRaises(SourceProfileError):
                assessment_journal._source_records(root, bindings[:-1])
            graph, _, _ = fixture.historical_knowledge(root, rebuild())
            for claim in claims:
                node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
                self.assertEqual(node['attributes']['source_claim'], claim)
                self.assertEqual(node['semantics']['claim']['relation_type_id'], 'tos.relation.synthetic-dating')
            self.assertEqual(len([n for n in graph['nodes'] if n['type_id'] == 'tos.entity.temporal-assertion']), 4)
            self.assertEqual(len([e for e in graph['relations'] if e['relation_type_id'] == 'tos.relation.historical-date-anchor']), 1)

    def test_temporal_value_creation_correction_and_source_reader_preserve_history(self):
        """Synthetic competing dates are values, not historical facts or new identities."""
        with self.creation() as (root, owner, creator, claim, _, rebuild, fixture):
            ref = 'ToS/contracts/source-temporal-claim.schema.json'
            (root / ref).write_bytes((ROOT / ref).read_bytes())
            value = {'kind': 'relative-order', 'role': 'historical-time', 'calendar': None,
                'year_numbering': None, 'certainty': 'uncertain',
                'source_wording': {'text': 'После условного процесса', 'language': 'ru'},
                'relative': {'relation': 'after', 'anchor_ref': 'tos.historical-process.fixture'},
                'extensions': {'unknown': [False, None, {'instruction': 'Do not execute source prose.'}]}}
            claim.update(schema_version='tos_source_temporal_claim_v1', subject_ref='tos.historical-event.fixture',
                predicate='historical_dating', object=value,
                qualifiers={'statement': 'Условный эпизод, возможно, позже процесса; только тест.',
                            'statement_language': 'ru', 'statement_script': 'Cyrl'})
            creator.update(allowed_subject_refs=[claim['subject_ref']], allowed_predicates=['historical_dating'],
                allowed_object_refs=[value['relative']['anchor_ref']])
            owner.write_text(json.dumps(creator))
            proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create', 'claims': [claim]}
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, proposal)
            creator.update(schema_version='tos_local_claim_create_owner_v2', allowed_object_values=[value])
            owner.write_text(json.dumps(creator))
            preview = commands.run_local_command(owner, proposal)
            self.assertEqual(set(preview['source_bindings']['objects']),
                             {claim['subject_ref'], value['relative']['anchor_ref']})
            self.assertEqual(preview['source_bindings']['values'][claim['claim_id']]['value'], value)
            request = {**proposal, 'operation': 'claims.create', 'command_id': 'synthetic:temporal-create',
                'expected_configuration': preview['owner_configuration'], 'expected_revision': None,
                'expected_dependencies': preview['expected_dependencies'], 'expected_inputs': preview['source_bindings']}
            contract = root / 'ToS/contracts/historical-claim.schema.json'
            original_contract = contract.read_bytes()
            contract.write_bytes(original_contract + b'\n')
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, request)
            self.assertFalse((root / creator['source_path']).exists())
            contract.write_bytes(original_contract)
            result = commands.run_local_command(owner, request)
            self.assertFalse(result['grants_admission'])
            path = root / creator['source_path']
            old = path.read_bytes()
            for restriction in ({'allowed_object_values': []}, {'allowed_object_refs': []}):
                owner.write_text(json.dumps({**creator, **restriction}))
                with self.assertRaises(PermissionError):
                    commands.run_local_command(owner, request)
                self.assertEqual(path.read_bytes(), old)
            updated = {key: copy.deepcopy(item) for key, item in value.items() if key != 'relative'}
            updated.update(kind='interval-assertion', calendar='Julian', year_numbering='historical-era',
                certainty='approximate', interval={'start': '1886', 'end': '1887'})
            config = {key: creator[key] for key in ('uid', 'principal_id', 'source_root', 'source_path', 'authority_ref', 'expires_at')}
            config.update(schema_version='tos_local_claim_revision_owner_v1', claim_id=claim['claim_id'],
                allowed_operations=['claim.revise'], allowed_fields=['object', 'qualifiers'],
                allowed_evidence_refs=creator['allowed_evidence_refs'], allowed_form_ids=['tos.form.synthetic-date'])
            owner.write_text(json.dumps(config))
            change = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
                'fields': {'object': updated, 'qualifiers': {'statement': 'Условный интервал, приблизительно 1886–1887; только тест.'}},
                'forms': [{'form_id': 'tos.form.synthetic-date', 'field_id': 'claim.statement'}], 'reason': 'Synthetic date correction.'}
            with self.assertRaises((PermissionError, ValueError)):
                commands.run_local_command(owner, change)
            config.update(schema_version='tos_local_claim_revision_owner_v2', allowed_object_values=[updated],
                          allowed_object_refs=[])
            owner.write_text(json.dumps(config))
            for invalid in (value, claim['subject_ref'], {**updated, 'certainty': 'exact'}):
                with self.subTest(invalid=invalid), self.assertRaises(PermissionError):
                    commands.run_local_command(owner, {**change, 'fields': {'object': invalid}})
                self.assertEqual(path.read_bytes(), old)
            prepared = commands.run_local_command(owner, change)
            correction = {**change, 'operation': 'claim.revise', 'command_id': 'synthetic:temporal-correct',
                'expected_configuration': prepared['owner_configuration'], 'expected_source': prepared['source'],
                'expected_revision': prepared['revision'], 'expected_dependencies': prepared['expected_dependencies'],
                'expected_inputs': prepared['source_bindings']}
            corrected = commands.run_local_command(owner, correction)
            self.assertEqual(corrected['source']['version'], 2)
            self.assertEqual(json.loads(path.read_bytes())['object'], updated)
            prior = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'inspect-version', 'source': prepared['source']})
            self.assertEqual(prior['record'], claim)
            self.assertEqual((root / prior['files'][path.name]['archive_path']).read_bytes(), old)
            self.assertTrue(commands.run_local_command(owner, correction)['replayed'])
            owner.write_text(json.dumps({**config, 'allowed_object_values': []}))
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, correction)
            owner.write_text(json.dumps(config))
            graph, _, _ = fixture.historical_knowledge(root, rebuild())
            node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
            self.assertEqual(node['attributes']['source_claim']['object'], updated)
            self.assertEqual(node['semantics']['claim']['relation_type_id'], 'tos.relation.historical-dating')
            self.assertFalse(any(n['entity_id'] == str(updated) for n in graph['nodes']))
            self.assertEqual({form['state'] for form in corrected['materializations']}, {'ready'})
            owner.write_text(json.dumps(creator))
            self.assertTrue(commands.run_local_command(owner, request)['replayed'])

    def test_value_correction_grant_cannot_retarget_an_identity_relation(self):
        with self.correction() as (root, owner, config, claim, request):
            value = {'kind': 'unknown-date', 'role': 'historical-time', 'calendar': None,
                'year_numbering': None, 'certainty': 'unknown', 'source_wording': {'text': 'Неизвестно', 'language': 'ru'}}
            config.update(schema_version='tos_local_claim_revision_owner_v2', allowed_fields=['object'],
                allowed_object_values=[value], allowed_object_refs=[])
            owner.write_text(json.dumps(config))
            path = root / config['source_path']
            original = path.read_bytes()
            with self.assertRaisesRegex(PermissionError, 'identity endpoint'):
                commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                    'operation': 'prepare-revise', 'fields': {'object': value},
                    'forms': request['forms'], 'reason': 'Synthetic prohibited endpoint retarget.'})
            self.assertEqual(path.read_bytes(), original)

    @contextmanager
    def correction(self):
        """An ordinary created synthetic Claim and a separate correction grant."""
        with self.creation() as (root, owner, creator, claim, creation, *_):
            claim['qualifiers'].update(statement='Условная тестовая атрибуция; не исторический факт.',
                statement_language='ru', statement_script='Cyrl')
            prepared = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', 'claims': [claim]})
            creation.update(expected_dependencies=prepared['expected_dependencies'], expected_inputs=prepared['source_bindings'])
            commands.run_local_command(owner, creation)
            config = {key: creator[key] for key in ('uid', 'source_root', 'source_path', 'expires_at')}
            config.update(schema_version='tos_local_claim_revision_owner_v1', principal_id='test:claim-corrector',
                authority_ref='test:explicit-correction-not-assessment', claim_id=claim['claim_id'],
                allowed_operations=['claim.revise'], allowed_fields=['qualifiers', 'evidence_refs', 'alternative_claim_refs'],
                allowed_evidence_refs=creator['allowed_evidence_refs'], allowed_form_ids=['tos.form.test.corrected'])
            owner.write_text(json.dumps(config))
            proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
                'fields': {'qualifiers': {'statement': 'Уточнённая условная атрибуция; не исторический факт.'}},
                'forms': [{'form_id': 'tos.form.test.corrected', 'field_id': 'claim.statement'}],
                'reason': 'Synthetic correction for writer boundary validation.'}
            preview = commands.run_local_command(owner, proposal)
            request = {**proposal, 'operation': 'claim.revise', 'command_id': 'synthetic:correction',
                'expected_configuration': preview['owner_configuration'], 'expected_source': preview['source'],
                'expected_revision': preview['revision'], 'expected_dependencies': preview['expected_dependencies'],
                'expected_inputs': preview['source_bindings']}
            yield root, owner, config, claim, request

    def test_claim_correction_crash_before_and_after_exchange_recovers_exactly_once(self):
        for after in (False, True):
            with self.subTest(after=after), self.correction() as (root, owner, config, claim, request):
                path = root / config['source_path']
                original = {p.name: p.read_bytes() for p in path.parent.iterdir()}
                program = '''import json, os, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import source_commands, source_revisions
exchange = source_revisions._exchange
def lose_process(*args):
    if sys.argv[3] == 'after':
        exchange(*args)
    os._exit(73)
source_revisions._exchange = lose_process
source_commands.run_local_command(Path(sys.argv[2]), json.load(sys.stdin))
'''
                stopped = subprocess.run([sys.executable, '-c', program, str(fixtures.MECHANIC),
                    str(owner), 'after' if after else 'before'], input=json.dumps(request),
                    text=True, capture_output=True, timeout=30)
                self.assertEqual(stopped.returncode, 73, stopped.stderr)
                abandoned = list((root / 'ToS').glob('.claim-revision-*.pending'))
                self.assertEqual(len(abandoned), 1)
                retained = {p.name: p.read_bytes() for p in abandoned[0].iterdir()}
                if after:
                    self.assertEqual(retained, original)
                else:
                    self.assertEqual({p.name: p.read_bytes() for p in path.parent.iterdir()}, original)
                recovered = commands.run_local_command(owner, request)
                self.assertEqual(recovered['replayed'], after)
                self.assertEqual(recovered['source']['version'], 2)
                self.assertEqual({p.name: p.read_bytes() for p in abandoned[0].iterdir()}, retained)
                previous = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                    'operation': 'inspect-version', 'source': request['expected_source']})
                self.assertEqual(previous['record'], claim)
                self.assertEqual(len(json.loads(path.with_name('claim-revision-history.json').read_bytes())['receipts']), 1)
                self.assertEqual(commands.run_local_command(owner, request)['receipt'], recovered['receipt'])

    def test_competing_claim_corrections_have_one_winner_and_idempotent_retries(self):
        with self.correction() as (root, owner, config, claim, request):
            other = copy.deepcopy(request)
            other['command_id'] = 'synthetic:competing-correction'
            other['fields']['qualifiers']['statement'] = 'Конкурирующее условное уточнение.'
            def attempt(value):
                try:
                    return commands.run_local_command(owner, value)
                except commands.JournalConflict:
                    return None
            with ThreadPoolExecutor(max_workers=2) as pool:
                results = list(pool.map(attempt, [request, other]))
            self.assertEqual(sum(r is not None for r in results), 1)
            winner = next(r for r in results if r is not None)
            winning_request = winner['receipt']['request']
            with ThreadPoolExecutor(max_workers=2) as pool:
                retries = list(pool.map(lambda _: commands.run_local_command(owner, winning_request), range(2)))
            self.assertTrue(all(r['replayed'] and r['receipt'] == winner['receipt'] for r in retries))
            path = root / config['source_path']
            self.assertEqual(json.loads(path.read_bytes())['claim_version'], 2)
            self.assertEqual(len(json.loads(path.with_name('claim-revision-history.json').read_bytes())['receipts']), 1)

    def test_claim_correction_source_contract_and_reference_drift_refuse_without_writes(self):
        with self.correction() as (root, owner, config, claim, request):
            path = root / config['source_path']
            original = {p.name: p.read_bytes() for p in path.parent.iterdir()}
            for ref in ('ToS/contracts/source-relation-claim.schema.json', claim['evidence_refs'][0],
                        'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json'):
                target = root / ref
                raw = target.read_bytes()
                target.write_bytes(raw + b'\n')
                with self.subTest(ref=ref), self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
                target.write_bytes(raw)
                self.assertEqual({p.name: p.read_bytes() for p in path.parent.iterdir()}, original)
            for invalid in ({'expected_inputs': {}}, {'fields': {'alternative_claim_refs': ['tos.claim.absent']}},
                            {'fields': {'qualifiers': {'statement_language': 'ru\n'}}}):
                with self.subTest(invalid=invalid), self.assertRaises((ValueError, commands.ValidationError)):
                    commands.run_local_command(owner, {**request, **invalid})
                self.assertEqual({p.name: p.read_bytes() for p in path.parent.iterdir()}, original)

    def test_claim_correction_revocation_and_missing_or_corrupt_history_fail_closed(self):
        with self.correction() as (root, owner, config, claim, request):
            commands.run_local_command(owner, request)
            path = root / config['source_path']
            committed = {p.name: p.read_bytes() for p in path.parent.iterdir()}
            owner.write_text(json.dumps({**config, 'allowed_operations': []}))
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, request)
            owner.write_text(json.dumps(config))
            prior = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'inspect-version', 'source': request['expected_source']})
            archived = root / prior['files'][path.name]['archive_path']
            raw = archived.read_bytes()
            archived.write_bytes(b'corrupt synthetic archive')
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, request)
            archived.write_bytes(raw)
            moved = archived.with_suffix('.temporarily-absent')
            archived.rename(moved)
            with self.assertRaises((commands.JournalCorruption, FileNotFoundError)):
                commands.run_local_command(owner, request)
            moved.rename(archived)
            history = path.with_name('claim-revision-history.json')
            history_raw = history.read_bytes()
            hidden_history = history.with_suffix('.temporarily-absent')
            history.rename(hidden_history)
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, request)
            hidden_history.rename(history)
            history.write_text(json.dumps({**json.loads(history_raw), 'receipts': []}))
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, request)
            history.write_bytes(history_raw)
            self.assertTrue(commands.run_local_command(owner, request)['replayed'])
            self.assertEqual({p.name: p.read_bytes() for p in path.parent.iterdir()}, committed)

    def test_claim_form_writer_and_correction_cannot_publish_mixed_source_versions(self):
        with self.correction() as (root, owner, config, claim, request):
            form_config = {key: value for key, value in config.items()
                           if key not in {'allowed_fields', 'allowed_evidence_refs'}}
            form_config.update(schema_version='tos_local_claim_form_owner_v1',
                allowed_operations=['form.create', 'form.revise'])
            form_owner = root / 'form-owner.json'
            form_owner.write_text(json.dumps(form_config))
            prepared = commands.run_local_command(form_owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare', **request['forms'][0]})
            form_request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply',
                'command_id': 'synthetic:concurrent-form', 'expected_source': prepared['source'],
                'expected_configuration': prepared['owner_configuration'], 'expected_revision': prepared['revision'],
                'changes': [prepared['prepared_change']]}
            def attempt(item):
                try:
                    return commands.run_local_command(*item)
                except commands.JournalConflict:
                    return None
            with ThreadPoolExecutor(max_workers=2) as pool:
                results = list(pool.map(attempt, [(owner, request), (form_owner, form_request)]))
            self.assertEqual(sum(r is not None for r in results), 1)
            context = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
            self.assertTrue(all(v['state'] == 'ready' for v in context['materializations']))
            path = root / config['source_path']
            form_path = commands.claim_forms_path(path, claim['claim_id'])
            old = json.loads(form_path.read_bytes())
            proposal = {key: request[key] for key in ('schema_version', 'fields', 'forms', 'reason')}
            proposal.update(operation='prepare-revise', fields={'qualifiers': {'statement': 'Следующее условное уточнение.'}})
            # A different form ID is not permission to silently retire an existing form.
            config['allowed_form_ids'].append('tos.form.test.replacement')
            owner.write_text(json.dumps(config))
            with self.assertRaisesRegex(ValueError, 'every current form'):
                commands.run_local_command(owner, {**proposal,
                    'forms': [{'form_id': 'tos.form.test.replacement', 'field_id': 'claim.statement'}]})
            preview = commands.run_local_command(owner, proposal)
            commands.run_local_command(owner, {**proposal, 'operation': 'claim.revise', 'command_id': 'synthetic:next',
                'expected_configuration': preview['owner_configuration'], 'expected_source': preview['source'],
                'expected_revision': preview['revision'], 'expected_dependencies': preview['expected_dependencies'],
                'expected_inputs': preview['source_bindings']})
            retained = json.loads(form_path.read_bytes())
            self.assertEqual(retained['prior_forms'], [*old['prior_forms'], *old['forms']])
            self.assertEqual(retained['forms'][0]['form_version'], old['forms'][0]['form_version'] + 1)

    def test_claim_correction_midstage_scope_and_dependency_changes_refuse_publication(self):
        import source_revisions as packages
        for revoke in (False, True):
            with self.subTest(revoke=revoke), self.correction() as (root, owner, config, claim, request):
                path = root / config['source_path']
                original = {p.name: p.read_bytes() for p in path.parent.iterdir()}
                stage = packages._stage
                def drift(*args):
                    staging = stage(*args)
                    if args[-1] == '.claim-revision-':
                        if revoke:
                            owner.write_text(json.dumps({**config, 'allowed_operations': []}))
                        else:
                            schema = root / 'ToS/contracts/source-relation-claim.schema.json'
                            schema.write_bytes(schema.read_bytes() + b'\n')
                    return staging
                with patch.object(packages, '_stage', side_effect=drift), self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
                self.assertEqual({p.name: p.read_bytes() for p in path.parent.iterdir()}, original)

    def test_claim_revision_preserves_shared_stream_siblings_history_and_creation_replay(self):
        """Synthetic correction: no historical judgment or research admission."""
        with self.creation() as (root, owner, creator, claim, creation, rebuild, graph_fixture):
            claim['qualifiers'].update(statement='Условное исходное утверждение, не исторический факт.',
                statement_language='ru', statement_script='Cyrl', uninterpreted={'values': [None, False, 'Ω']})
            sibling = {**copy.deepcopy(claim), 'claim_id': claim['claim_id'] + '-sibling'}
            creator['allowed_claim_ids'].append(sibling['claim_id'])
            owner.write_text(json.dumps(creator))
            prepared = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', 'claims': [claim, sibling]})
            creation.update(claims=[claim, sibling], expected_configuration=prepared['owner_configuration'],
                expected_dependencies=prepared['expected_dependencies'], expected_inputs=prepared['source_bindings'])
            created = commands.run_local_command(owner, creation)
            path = root / creator['source_path']
            original = {p.name: p.read_bytes() for p in path.parent.iterdir()}
            config = {key: creator[key] for key in ('uid', 'source_root', 'source_path', 'expires_at')}
            config.update(schema_version='tos_local_claim_revision_owner_v1', principal_id='test:claim-corrector',
                authority_ref='test:explicit-claim-correction-not-assessment', claim_id=claim['claim_id'],
                allowed_operations=['claim.revise'], allowed_fields=['qualifiers', 'counterevidence_refs'],
                allowed_evidence_refs=creator['allowed_evidence_refs'], allowed_form_ids=['tos.form.test.revised-claim'])
            owner.write_text(json.dumps(config))
            proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
                'fields': {'qualifiers': {'statement': 'Уточнённое условное утверждение с явной оговоркой.'}},
                'forms': [{'form_id': 'tos.form.test.revised-claim', 'field_id': 'claim.statement'}],
                'reason': 'Synthetic source correction; preserve unknown qualifiers and all siblings.'}
            preview = commands.run_local_command(owner, proposal)
            self.assertEqual(original, {p.name: p.read_bytes() for p in path.parent.iterdir()})
            request = {**proposal, 'operation': 'claim.revise', 'command_id': 'test:claim-revision-1',
                'expected_configuration': preview['owner_configuration'], 'expected_source': preview['source'],
                'expected_revision': preview['revision'], 'expected_dependencies': preview['expected_dependencies'],
                'expected_inputs': preview['source_bindings']}
            for changes in ({'fields': {'object': sibling['object']}}, {'fields': {'review_status': 'accepted'}},
                    {'fields': {'claim_version': 100}}, {'fields': {'counterevidence_refs': ['ToS/not-delegated.md']}},
                    {'forms': []}, {'expected_source': {**preview['source'], 'digest': 'sha256:' + '0' * 64}},
                    {'expected_dependencies': 'sha256:' + '0' * 64}):
                with self.subTest(changes=changes), self.assertRaises((ValueError, PermissionError)):
                    commands.run_local_command(owner, {**request, **changes})
                self.assertEqual(original, {p.name: p.read_bytes() for p in path.parent.iterdir()})
            revised = commands.run_local_command(owner, request)
            self.assertFalse(revised['grants_admission'])
            self.assertEqual(revised['receipt']['source_bindings'], preview['source_bindings'])
            self.assertFalse(revised['replayed'])
            rows = path.read_bytes().splitlines(keepends=True)
            self.assertEqual(rows[1], original[path.name].splitlines(keepends=True)[1])
            current = json.loads(rows[0])
            self.assertEqual(current, {**claim, 'claim_version': 2,
                'qualifiers': {**claim['qualifiers'], **proposal['fields']['qualifiers']}})
            for name in original.keys() - {path.name}:
                self.assertEqual((path.parent / name).read_bytes(), original[name])
            self.assertEqual(revised['source']['id'], claim['claim_id'])
            self.assertEqual(revised['source']['version'], 2)
            self.assertEqual(revised['materializations'][0]['display_text'], current['qualifiers']['statement'])
            self.assertEqual(revised['materializations'][0]['context'][0]['value'], current)
            self.assertIsNone(revised['materializations'][0]['admission'])
            previous = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'inspect-version', 'source': request['expected_source']})
            self.assertEqual(previous['record'], claim)
            for name, binding in previous['files'].items():
                self.assertEqual((root / binding['archive_path']).read_bytes(), original[name])
            after = {p.name: p.read_bytes() for p in path.parent.iterdir()}
            process = subprocess.run([sys.executable, str(fixtures.MECHANIC / 'source_commands.py'),
                '--owner-config', str(owner)], input=json.dumps(request), text=True, capture_output=True, timeout=30)
            self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
            replay = json.loads(process.stdout)
            self.assertTrue(replay['replayed'])
            self.assertEqual(replay['receipt'], revised['receipt'])
            self.assertEqual(after, {p.name: p.read_bytes() for p in path.parent.iterdir()})
            graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
            node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
            self.assertEqual(node['attributes']['source_claim'], current)
            self.assertEqual(node['attributes']['human_forms'], revised['materializations'])
            owner.write_text(json.dumps(creator))
            replay_creation = commands.run_local_command(owner, creation)
            self.assertTrue(replay_creation['replayed'])
            self.assertEqual(replay_creation['receipt'], created['receipt'])
            self.assertEqual(path.read_bytes(), b''.join(rows))
            # One shared history must cover interleaved revisions of different
            # Claims, while each Claim keeps its own version and form lineage.
            for index, selected in enumerate((sibling, claim), start=2):
                config['claim_id'] = selected['claim_id']
                form_id = 'tos.form.test.revised-sibling' if selected is sibling else 'tos.form.test.revised-claim'
                config['allowed_form_ids'] = [form_id]
                owner.write_text(json.dumps(config))
                fields = {'qualifiers': {'statement': f'Условное уточнение {index}; не историческое свидетельство.'}}
                next_proposal = {**proposal, 'fields': fields,
                    'forms': [{'form_id': form_id, 'field_id': 'claim.statement'}]}
                prepared = commands.run_local_command(owner, next_proposal)
                next_request = {**next_proposal, 'operation': 'claim.revise', 'command_id': f'test:claim-revision-{index}',
                    'expected_configuration': prepared['owner_configuration'], 'expected_source': prepared['source'],
                    'expected_revision': prepared['revision'], 'expected_dependencies': prepared['expected_dependencies'],
                    'expected_inputs': prepared['source_bindings']}
                unchanged_index = 0 if selected is sibling else 1
                sibling_bytes = path.read_bytes().splitlines(keepends=True)[unchanged_index]
                commands.run_local_command(owner, next_request)
                self.assertEqual(path.read_bytes().splitlines(keepends=True)[unchanged_index], sibling_bytes)
            replay = commands.run_local_command(owner, request)
            self.assertTrue(replay['replayed'])
            self.assertEqual(replay['receipt'], revised['receipt'])
            self.assertEqual(replay['source']['version'], 3)
            self.assertEqual(len(json.loads(path.with_name('claim-revision-history.json').read_bytes())['receipts']), 3)
            history_path = path.with_name('claim-revision-history.json')
            history_raw = history_path.read_bytes()
            history = json.loads(history_raw)
            history_path.write_text(json.dumps({**history, 'receipts': history['receipts'][1:]}))
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, request)
            history_path.write_bytes(history_raw)
            committed_stream = path.read_bytes()
            corrupt = [json.loads(line) for line in committed_stream.splitlines()]
            corrupt[1]['qualifiers']['statement'] = 'Unrecorded sibling corruption, synthetic only.'
            path.write_bytes(b''.join(commands._canonical(c) + b'\n' for c in corrupt))
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
            path.write_bytes(committed_stream)  # Restore only this temporary fixture.
            owner.write_text(json.dumps(creator))
            self.assertEqual(commands.run_local_command(owner, creation)['receipt'], created['receipt'])
            self.assertEqual(path.read_bytes(), committed_stream)

    def test_argument_chain_creation_is_atomic_scoped_and_source_bound(self):
        with self.creation() as (root, owner, config, base, request, rebuild, fixture):
            for name in ('source-metadata-record', 'semantic-description-record', 'thought-description-record',
                         'semantic-relation-claim', 'thought-relation-claim'):
                ref = 'ToS/contracts/' + name + '.schema.json'
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            content = {
                'thesis': {'proposition': 'Synthetic hypothesis only.', 'assertion_force': 'hypothetical'},
                'argument': {'reconstruction_note': 'Synthetic reconstruction only.', 'coverage': 'partial'},
                'inference-step': {'transition_account': 'Synthetic transition only.', 'reasoning_mode': 'reductio'},
                'objection': {'challenge_account': 'This synthetic transition is disputed.'},
            }
            subjects = {}
            for kind, detail in content.items():
                source = {'schema_version': 'tos_thought_description_record_v1', 'record_type': kind,
                    'record_id': 'tos.' + kind + '.synthetic-command', 'record_version': 1,
                    'preferred_label': 'Условный предмет мысли', 'notes': 'Синтетическое описание, не исторический факт.',
                    'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                        'notes': {'language': 'ru', 'script': 'Cyrl'}},
                    'semantic_scope': {'scope_note': 'Synthetic test only.', 'identity_criterion': 'Same test referent.',
                                       'language': 'en', 'script': 'Latn'},
                    'semantic_content': {**detail, 'language': 'en', 'script': 'Latn'},
                    'identity_status': 'provisional', 'source_refs': base['evidence_refs'],
                    'external_identifiers': [], 'same_as_posture': 'no_equivalence_claim',
                    'visibility': 'public_metadata_only'}
                path = root / f'ToS/source-witnesses/semantic-descriptions/synthetic-{kind}/{kind}.json'
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps(source))
                subjects[kind] = source
            claims = []
            for predicate, left, right in (('argument_has_step', 'argument', 'inference-step'),
                    ('step_has_premise', 'inference-step', 'thesis'),
                    ('step_has_conclusion', 'inference-step', 'thesis'),
                    ('objection_to_step', 'objection', 'inference-step')):
                claims.append({**copy.deepcopy(base), 'schema_version': 'tos_semantic_relation_claim_v1',
                    'claim_id': 'tos.claim.synthetic-command-' + predicate.replace('_', '-'),
                    'subject_ref': subjects[left]['record_id'], 'object': subjects[right]['record_id'],
                    'predicate': predicate, 'assertion_layer': 'semantic_interpretation',
                    'qualifiers': {'statement': 'Условная спорная реконструкция, не признанный вывод.',
                        'statement_language': 'ru', 'statement_script': 'Cyrl',
                        'relation_basis': 'Synthetic execution test; no logical validity asserted.',
                        **({'step_position': 0} if predicate == 'argument_has_step' else {})}})
            config.update(allowed_claim_ids=[c['claim_id'] for c in claims],
                allowed_predicates=[c['predicate'] for c in claims],
                allowed_subject_refs=sorted({c['subject_ref'] for c in claims}),
                allowed_object_refs=sorted({c['object'] for c in claims}))
            owner.write_text(json.dumps(config))
            preview_request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create', 'claims': claims}
            bad = copy.deepcopy(preview_request)
            bad['claims'][0]['qualifiers'].pop('step_position')
            with self.assertRaises(ValueError):
                commands.run_local_command(owner, bad)
            self.assertFalse((root / config['source_path']).parent.exists())
            preview = commands.run_local_command(owner, preview_request)
            request.update(claims=claims, expected_configuration=preview['owner_configuration'],
                expected_dependencies=preview['expected_dependencies'], expected_inputs=preview['source_bindings'])
            # One invalid member cannot leave the other three published.
            bad = copy.deepcopy(request)
            bad['claims'][-1]['object'] = subjects['thesis']['record_id']
            with self.assertRaises((ValueError, commands.JournalConflict)):
                commands.run_local_command(owner, bad)
            self.assertFalse((root / config['source_path']).parent.exists())
            result = commands.run_local_command(owner, request)
            self.assertFalse(result['grants_admission'])
            self.assertEqual(commands.run_local_command(owner, request)['receipt'], result['receipt'])
            graph, _, _ = fixture.historical_knowledge(root, rebuild())
            nodes = {node['entity_id']: node for node in graph['nodes']}
            for claim in claims:
                self.assertEqual(nodes[claim['claim_id']]['attributes']['source_claim'], claim)
            for source in subjects.values():
                self.assertEqual(nodes[source['record_id']]['attributes']['source_record'], source)

    def test_semantic_claim_creation_uses_shared_writer_and_keeps_exact_source_scope(self):
        with self.creation() as (root, owner, config, claim, request, rebuild, graph_fixture):
            for name in ('source-metadata-record', 'semantic-description-record', 'semantic-relation-claim'):
                ref = 'ToS/contracts/' + name + '.schema.json'
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            source = {'schema_version': 'tos_semantic_description_record_v1', 'record_type': 'conception',
                'record_id': 'tos.conception.synthetic-command', 'record_version': 1,
                'preferred_label': 'Synthetic account', 'notes': 'Synthetic account, not attributed historical thought.',
                'field_languages': {'preferred_label': {'language': 'en', 'script': 'Latn'},
                                    'notes': {'language': 'en', 'script': 'Latn'}},
                'semantic_scope': {'scope_note': 'A test fixture only.', 'identity_criterion': 'This exact test referent.',
                                   'language': 'en', 'script': 'Latn'},
                'identity_status': 'provisional', 'source_refs': claim['evidence_refs'],
                'external_identifiers': [], 'same_as_posture': 'no_equivalence_claim', 'visibility': 'public_metadata_only'}
            path = root / 'ToS/source-witnesses/semantic-descriptions/synthetic/conception.json'
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(source))
            claim.update(schema_version='tos_semantic_relation_claim_v1', subject_ref=source['record_id'],
                predicate='conception_attributed_to', assertion_layer='semantic_interpretation',
                qualifiers={'statement': 'Synthetic disputed attribution, not a historical conclusion.',
                    'statement_language': 'en', 'statement_script': 'Latn', 'negated': True,
                    'relation_basis': 'Synthetic comparison for execution checks only.'})
            config.update(allowed_subject_refs=[source['record_id']], allowed_predicates=[claim['predicate']])
            owner.write_text(json.dumps(config))
            preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', 'claims': [claim]})
            request.update(expected_configuration=preview['owner_configuration'],
                expected_dependencies=preview['expected_dependencies'], expected_inputs=preview['source_bindings'])
            source['semantic_scope']['scope_note'] += ' Changed during preparation.'
            path.write_text(json.dumps(source))
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, request)
            preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', 'claims': [claim]})
            request.update(expected_dependencies=preview['expected_dependencies'], expected_inputs=preview['source_bindings'])
            result = commands.run_local_command(owner, request)
            self.assertFalse(result['grants_admission'])
            self.assertEqual(commands.run_local_command(owner, request)['receipt'], result['receipt'])
            graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
            node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
            self.assertEqual(node['attributes']['source_claim'], claim)
            self.assertEqual(node['semantics']['claim']['relation_type_id'], 'tos.relation.conception-attributed-to')

    def test_claim_forms_use_shared_commands_and_keep_exact_claim_context(self):
        with self.creation() as (root, owner, config, claim, request, rebuild, graph_fixture):
            claim['qualifiers'].update(statement='Не подтверждено; только условная тестовая атрибуция.',
                                       statement_language='ru', statement_script='Cyrl')
            preview = commands.run_local_command(owner, {
                'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create', 'claims': [claim]})
            request.update(expected_dependencies=preview['expected_dependencies'], expected_inputs=preview['source_bindings'])
            commands.run_local_command(owner, request)
            source = root / config['source_path']
            original = source.read_bytes()
            form_config = {key: config[key] for key in ('uid', 'principal_id', 'source_root', 'source_path',
                                                       'authority_ref', 'expires_at')}
            form_config.update(schema_version='tos_local_claim_form_owner_v1', claim_id=claim['claim_id'],
                allowed_operations=['form.create', 'form.revise'], allowed_form_ids=['tos.form.test.claim-statement'])
            owner.write_text(json.dumps(form_config))
            prepared = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare', 'field_id': 'claim.statement', 'form_id': 'tos.form.test.claim-statement'})
            self.assertEqual(prepared['source']['id'], claim['claim_id'])
            self.assertNotEqual(prepared['source']['id'], claim['subject_ref'])
            self.assertEqual(prepared['source_fields'], [{'field_id': 'claim.statement', 'role': 'statement',
                                                         'language': 'ru', 'script': 'Cyrl'}])
            form_request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply',
                'command_id': 'synthetic-claim-form', 'expected_source': prepared['source'],
                'expected_configuration': prepared['owner_configuration'], 'expected_revision': prepared['revision'],
                'changes': [prepared['prepared_change']]}
            result = commands.run_local_command(owner, form_request)
            view = result['materializations'][0]
            self.assertEqual(view['state'], 'ready')
            self.assertEqual(view['display_text'], claim['qualifiers']['statement'])
            self.assertEqual(view['context'][0]['value'], claim)
            self.assertFalse(view['standalone_reading'])
            self.assertIsNone(view['admission'])
            self.assertFalse(result['grants_admission'])
            replay = commands.run_local_command(owner, form_request)
            self.assertTrue(replay['replayed'])
            self.assertEqual(replay['receipt'], result['receipt'])
            self.assertEqual(source.read_bytes(), original)
            graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
            node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
            self.assertEqual(node['attributes']['human_forms'], result['materializations'])
            self.assertEqual(node['attributes']['source_claim'], claim)
            from tos_access.knowledge import select_human_forms
            self.assertEqual(select_human_forms(node, 'ru')['roles']['statement']['packet'], view)
            # Forms are separately owned descendants, not a mutation of the
            # immutable creation receipt or a reason to repeat source creation.
            owner.write_text(json.dumps(config))
            replay_creation = commands.run_local_command(owner, request)
            self.assertTrue(replay_creation['replayed'])
            owner.write_text(json.dumps(form_config))
            prepared_again = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare', 'field_id': 'claim.statement', 'form_id': 'tos.form.test.claim-statement'})
            self.assertEqual(prepared_again['prepared_change']['operation'], 'form.revise')
            revised_request = {**form_request, 'command_id': 'synthetic-claim-form-revision',
                'expected_revision': prepared_again['revision'], 'changes': [prepared_again['prepared_change']]}
            revised = commands.run_local_command(owner, revised_request)
            retained = json.loads((root / revised['target_path']).read_bytes())
            self.assertEqual(retained['prior_forms'], [form_request['changes'][0]['form']])
            self.assertEqual(retained['forms'][0]['form_version'], 2)
            self.assertEqual(revised['receipt']['source_contracts'], prepared_again['source_contracts'])
            cli = subprocess.run([sys.executable, str(fixtures.MECHANIC / 'source_commands.py'), '--owner-config', str(owner)],
                input=json.dumps(revised_request), text=True, capture_output=True, timeout=30)
            self.assertEqual(cli.returncode, 0, cli.stdout)
            self.assertEqual(json.loads(cli.stdout)['receipt'], revised['receipt'])
            # Mechanical reading does not invent a language/script or admit a
            # statement after changes in qualifiers, assessment or source bytes.
            altered = copy.deepcopy(claim)
            altered['qualifiers']['negated'] = False
            source.write_text(json.dumps(altered))
            stale = commands.run_local_command(owner, form_request)
            self.assertTrue(stale['replayed'])
            self.assertEqual(stale['materializations'][0]['state'], 'stale')
            self.assertIsNone(stale['materializations'][0]['display_text'])
            self.assertIsNone(stale['materializations'][0]['admission'])

    def test_claim_form_refusals_preserve_source_and_do_not_publish(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            claim['qualifiers'].update(statement='Не доказано.', statement_language='x-fixture')
            preview = commands.run_local_command(owner, {
                'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create', 'claims': [claim]})
            request.update(expected_dependencies=preview['expected_dependencies'], expected_inputs=preview['source_bindings'])
            commands.run_local_command(owner, request)
            source = root / config['source_path']
            original = source.read_bytes()
            form_config = {key: config[key] for key in ('uid', 'principal_id', 'source_root', 'source_path',
                                                       'authority_ref', 'expires_at')}
            form_config.update(schema_version='tos_local_claim_form_owner_v1', claim_id=claim['claim_id'],
                allowed_operations=['form.create', 'form.revise'], allowed_form_ids=['tos.form.test.claim'])
            owner.write_text(json.dumps(form_config))
            prepare = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare',
                       'field_id': 'claim.statement', 'form_id': 'tos.form.test.claim'}
            prepared = commands.run_local_command(owner, prepare)
            form_request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'apply',
                'command_id': 'synthetic-negative-claim-form', 'expected_source': prepared['source'],
                'expected_configuration': prepared['owner_configuration'], 'expected_revision': None,
                'changes': [prepared['prepared_change']]}
            target = root / prepared['target_path']
            for mutation in ('context', 'language', 'subject', 'role', 'maker'):
                bad = copy.deepcopy(form_request)
                form = bad['changes'][0]['form']
                if mutation == 'context':
                    del form['bindings']['context-0']
                elif mutation == 'language':
                    form['language'] = 'ru'
                elif mutation == 'subject':
                    form['subject']['id'] = claim['subject_ref']
                elif mutation == 'role':
                    form['role'] = 'caption'
                else:
                    form['creator_id'] = 'model:other'
                with self.subTest(mutation=mutation), self.assertRaises((ValueError, PermissionError)):
                    commands.run_local_command(owner, bad)
                self.assertFalse(target.exists())
            schema = root / 'ToS/contracts/source-relation-claim.schema.json'
            schema.write_bytes(schema.read_bytes() + b'\n')
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, form_request)
            self.assertFalse(target.exists())
            for mutation in ({'claim_id': 'tos.claim.absent'}, {'claim_id': '../../escape'},
                             {'source_path': 'ToS/source-witnesses/catalog/source-claims.jsonl'}):
                owner.write_text(json.dumps({**form_config, **mutation}))
                with self.subTest(config=mutation), self.assertRaises((ValueError, PermissionError, FileNotFoundError)):
                    commands.run_local_command(owner, prepare)
            owner.write_text(json.dumps(form_config))
            for replacement in ({**claim, 'visibility': 'local_only'}, {**claim, 'schema_version': 'unknown'},
                                {**claim, 'qualifiers': {'statement': 'test', 'statement_language': 'ru\n'}},
                                {**claim, 'qualifiers': {'statement': 'test', 'statement_script': 'Cyrillic'}}):
                source.write_text(json.dumps(replacement))
                with self.assertRaises((ValueError, PermissionError)):
                    commands.run_local_command(owner, prepare)
            source.write_bytes(original + original)
            with self.assertRaisesRegex(ValueError, 'exactly once'):
                commands.run_local_command(owner, prepare)
            source.write_bytes(original)
            self.assertFalse(target.exists())


    @contextmanager
    def creation(self):
        fixture = fixtures.HistoricalCreationTests()
        with fixture.creation() as (root, owner, original, old_request, rebuild, graph_fixture):
            for name in ('semantic-relation-type-registry', 'source-claim-record',
                         'source-relation-claim', 'provenance-event-v2'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            subject = 'tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese'
            target = 'tos.agent.friedrich-nietzsche'
            relative = 'ToS/source-witnesses/relations/new-claim-batch/source-claims.jsonl'
            (root / relative).parent.parent.mkdir(parents=True, exist_ok=True)
            claim = {**copy.deepcopy(old_request['claims'][0]),
                     'schema_version': 'tos_source_relation_claim_v1',
                     'claim_id': 'tos.claim.synthetic-shared-writer',
                     'subject_ref': subject, 'predicate': 'authored_by', 'object': target,
                     'provenance_event_ref': 'tos.event.synthetic-shared-claim-writer',
                     'extensions': {'unknown': [None, False, 'Ω']}}
            config = {key: original[key] for key in
                      ('uid', 'principal_id', 'maker_type', 'source_root', 'authority_ref', 'expires_at')}
            config.update(schema_version='tos_local_claim_create_owner_v1', source_path=relative,
                allowed_operations=['claims.create'], allowed_claim_ids=[claim['claim_id']],
                allowed_subject_refs=[subject], allowed_object_refs=[target],
                allowed_predicates=['authored_by'], allowed_evidence_refs=claim['evidence_refs'],
                provenance_event_id=claim['provenance_event_ref'])
            owner.write_text(json.dumps(config))
            description = commands.run_local_command(owner, {
                'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
            self.assertEqual(description['supported_operations'], ['claims.create'])
            self.assertFalse(description['target_exists'])
            preview = commands.run_local_command(owner, {
                'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create', 'claims': [claim]})
            self.assertFalse((root / relative).parent.exists())
            request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'claims.create',
                'command_id': 'synthetic-claim-create', 'expected_configuration': description['owner_configuration'],
                'expected_revision': None, 'expected_dependencies': preview['expected_dependencies'],
                'expected_inputs': preview['source_bindings'], 'claims': [claim]}
            yield root, owner, config, claim, request, rebuild, graph_fixture

    def test_declared_claim_batch_publishes_atomically_and_replays(self):
        """Synthetic authorship, not another historical attribution to Nietzsche."""
        with self.creation() as (root, owner, config, claim, request, rebuild, graph_fixture):
            relative = config['source_path']
            result = commands.run_local_command(owner, request)
            self.assertFalse(result['grants_admission'])
            self.assertFalse(result['replayed'])
            self.assertEqual(result['receipt']['source_bindings'], request['expected_inputs'])
            self.assertEqual(set(request['expected_inputs']['objects']), {claim['subject_ref'], claim['object']})
            for binding in request['expected_inputs']['objects'].values():
                self.assertEqual(binding['source_sha256'], commands._digest((root / binding['source_ref']).read_bytes()))
            files = {p.name: p.read_bytes() for p in (root / relative).parent.iterdir()}
            self.assertEqual(set(files), {'source-claims.jsonl', 'source-create-request.json',
                'source-create-environment.json', 'source-create-provenance.jsonl', 'source-create-receipt.json'})
            self.assertEqual(json.loads(files['source-claims.jsonl']), claim)
            replay = commands.run_local_command(owner, request)
            self.assertTrue(replay['replayed'])
            self.assertEqual(replay['receipt'], result['receipt'])
            self.assertEqual(files, {p.name: p.read_bytes() for p in (root / relative).parent.iterdir()})
            graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
            node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
            self.assertEqual(node['attributes']['source_claim'], claim)
            self.assertEqual(node['semantics']['claim']['relation_type_id'], 'tos.relation.authored-by')
            event = json.loads(files['source-create-provenance.jsonl'])
            self.assertEqual(event['method']['procedure']['name'], 'source-claim-serialization')
            import source_claim_commands as writer
            component = next(row for row in event['method']['software_components']
                             if row['artifact_ref'] == writer.MODULE_REF)
            self.assertEqual(component['artifact_sha256'], hashlib.sha256((ROOT / writer.MODULE_REF).read_bytes()).hexdigest())
            cli = subprocess.run([sys.executable, str(fixtures.MECHANIC / 'source_commands.py'), '--owner-config', str(owner)],
                input=json.dumps(request), text=True, capture_output=True, timeout=30)
            self.assertEqual(cli.returncode, 0, cli.stdout)
            self.assertEqual(json.loads(cli.stdout)['receipt'], result['receipt'])
            config['allowed_operations'] = []
            owner.write_text(json.dumps(config))
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, request)

    def test_scope_schema_and_initial_state_refusals_publish_nothing(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            for changes in (
                {'claim_id': 'tos.claim.not-delegated'}, {'subject_ref': claim['object']},
                {'object': claim['subject_ref']}, {'predicate': 'historical_work'},
                {'maker': {'maker_type': 'human', 'agent_ref': config['principal_id']}},
                {'maker': {'maker_type': 'software', 'agent_ref': 'software:other'}},
                {'provenance_event_ref': 'tos.event.not-delegated'},
                {'schema_version': 'tos_claim_unknown_v99'}, {'review_status': 'accepted'},
                {'visibility': 'local_only'}, {'claim_version': 2},
                {'assessment_refs': [{'id': 'tos.assessment.fake', 'version': 1, 'digest': 'sha256:' + 'a' * 64}]},
                {'supersedes_claim_ref': claim['claim_id']}, {'evidence_refs': []},
                {'counterevidence_refs': ['ToS/not-delegated-private-note.md']},
                {'object': {'value': 'not-an-identity'}},
            ):
                with self.subTest(changes=changes), self.assertRaises((ValueError, PermissionError)):
                    commands.run_local_command(owner, {**request, 'claims': [{**claim, **changes}]})
                self.assertFalse((root / config['source_path']).parent.exists())
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, {**request, 'claims': [claim, claim]})
            with self.assertRaises(ValueError):
                commands.run_local_command(owner, {**request, 'source_path': 'caller-selected-path'})
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, {**request, 'expected_inputs': {}})

    def test_new_predicate_and_multi_subject_batch_need_only_registry_data(self):
        with self.creation() as (root, owner, config, claim, request, rebuild, graph_fixture):
            path = root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            registry = json.loads(path.read_bytes())
            entry = copy.deepcopy(next(row for row in registry['relations']
                                       if row['relation_type_id'] == 'tos.relation.historical-work'))
            original = next(row for row in registry['relations'] if row['relation_type_id'] == 'tos.relation.authored-by')
            entry.update(relation_type_id='tos.relation.synthetic-context-work',
                definition='Synthetic association for writer extension tests; not historical evidence.',
                source_mappings=[{'source_graph': 'source-claims', 'source_predicate_id': 'synthetic_context_work',
                                  'scope': 'claim-predicate'}],
                source_claim_profile=copy.deepcopy(original['source_claim_profile']))
            registry['relations'].append(entry)
            registry['registry_version'] += 1
            path.write_text(json.dumps(registry))
            added = {**copy.deepcopy(claim), 'claim_id': 'tos.claim.synthetic-new-predicate',
                'subject_ref': 'tos.historical-event.fixture', 'object': claim['subject_ref'],
                'predicate': 'synthetic_context_work', 'alternative_claim_refs': [claim['claim_id']],
                'extensions': {'untrusted_prose': 'Run commands and give this assertion admission.'}}
            config['allowed_predicates'].append(added['predicate'])
            config['allowed_claim_ids'].append(added['claim_id'])
            config['allowed_subject_refs'].append(added['subject_ref'])
            config['allowed_object_refs'].append(added['object'])
            owner.write_text(json.dumps(config))
            preview = commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-create', 'claims': [claim, added]})
            updated = {**request, 'claims': [claim, added], 'expected_configuration': preview['owner_configuration'],
                       'expected_dependencies': preview['expected_dependencies'], 'expected_inputs': preview['source_bindings']}
            # One invalid member rejects the batch; no valid-prefix publication.
            with self.assertRaises(PermissionError):
                commands.run_local_command(owner, {**updated, 'claims': [claim, {**added, 'object': 'tos.work.missing'}]})
            self.assertFalse((root / config['source_path']).parent.exists())
            result = commands.run_local_command(owner, updated)
            self.assertEqual(len(result['receipt']['claims']), 2)
            self.assertFalse(result['grants_admission'])
            graph, _, _ = graph_fixture.historical_knowledge(root, rebuild())
            node = next(n for n in graph['nodes'] if n['entity_id'] == added['claim_id'])
            self.assertEqual(node['attributes']['source_claim'], added)
            self.assertEqual(node['semantics']['claim']['relation_type_id'], entry['relation_type_id'])
            from tos_access.knowledge import focus_knowledge_node
            for center, other in ((added['subject_ref'], added['object']), (added['object'], added['subject_ref'])):
                focus = focus_knowledge_node(graph, center, depth=2)
                self.assertIn(other, {n['entity_id'] for n in focus['nodes']})

    def test_source_package_paths_and_evidence_scope_are_not_bypassable(self):
        for package in ('catalog', 'payload', 'local-content', '..'):
            with self.subTest(package=package), self.creation() as (root, owner, config, claim, request, *_):
                config['source_path'] = f'ToS/source-witnesses/relations/{package}/source-claims.jsonl'
                owner.write_text(json.dumps(config))
                with self.assertRaises(PermissionError):
                    commands.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
        with self.creation() as (root, owner, config, claim, request, *_):
            target = (root / config['source_path']).parent
            other = target.parent / 'unrelated'
            other.mkdir()
            target.symlink_to(other, target_is_directory=True)
            with self.assertRaises(OSError):
                commands.run_local_command(owner, request)
            self.assertEqual(list(other.iterdir()), [])

    def test_selected_source_and_schema_changes_conflict_after_preparation(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            refs = ['ToS/contracts/source-relation-claim.schema.json', claim['evidence_refs'][0],
                    'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json']
            for ref in refs:
                path = root / ref
                original = path.read_bytes()
                payload = json.loads(original)
                key = 'description' if ref.endswith('.schema.json') else 'notes'
                payload[key] = str(payload.get(key, '')) + ' changed synthetic input'
                path.write_text(json.dumps(payload))
                with self.subTest(ref=ref), self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
                self.assertFalse((root / config['source_path']).parent.exists())
                path.write_bytes(original)

    def test_concurrent_creation_and_occupied_paths_are_not_overwritten(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            with ThreadPoolExecutor(max_workers=2) as pool:
                results = list(pool.map(lambda _: commands.run_local_command(owner, request), range(2)))
            self.assertEqual(sorted(result['replayed'] for result in results), [False, True])
            self.assertEqual(results[0]['receipt'], results[1]['receipt'])
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, {**request, 'command_id': 'different-command'})
        with self.creation() as (root, owner, config, claim, request, *_):
            target = (root / config['source_path']).parent
            target.mkdir()
            with self.assertRaises(commands.JournalConflict):
                commands.run_local_command(owner, request)
            self.assertTrue(target.exists())
            self.assertEqual(list(target.iterdir()), [])

    def test_abrupt_loss_and_response_loss_recover_without_deleting_other_staging(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            code = ('import json, os, sys; sys.path.insert(0, sys.argv[1]); '
                    'from pathlib import Path; import source_commands as c; '
                    'c._publish_new_directory=lambda *args: os._exit(73); '
                    'c.run_local_command(Path(sys.argv[2]),json.load(sys.stdin))')
            stopped = subprocess.run([sys.executable, '-c', code, str(fixtures.MECHANIC), str(owner)],
                input=json.dumps(request), text=True, capture_output=True, timeout=30)
            self.assertEqual(stopped.returncode, 73, stopped.stderr)
            self.assertFalse((root / config['source_path']).parent.exists())
            orphan = list((root / 'ToS').glob('.source-claims-create-*.pending'))
            self.assertEqual(len(orphan), 1)
            old_bytes = {p.name: p.read_bytes() for p in orphan[0].iterdir()}
            original_publish = commands._publish_new_directory
            def lose_response(*args):
                original_publish(*args)
                raise RuntimeError('synthetic response loss after commit')
            with patch.object(commands, '_publish_new_directory', lose_response), self.assertRaises(RuntimeError):
                commands.run_local_command(owner, request)
            replay = commands.run_local_command(owner, request)
            self.assertTrue(replay['replayed'])
            self.assertEqual(old_bytes, {p.name: p.read_bytes() for p in orphan[0].iterdir()})

    def test_current_scope_and_dependency_changes_during_staging_refuse_commit(self):
        for revoke in (False, True):
            with self.subTest(revoke=revoke), self.creation() as (root, owner, config, claim, request, *_):
                original_publish = commands._publish
                changed = False
                def change_during_stage(path, raw):
                    nonlocal changed
                    original_publish(path, raw)
                    if not changed:
                        changed = True
                        if revoke:
                            config['allowed_operations'] = []
                            owner.write_text(json.dumps(config))
                        else:
                            schema = root / 'ToS/contracts/source-relation-claim.schema.json'
                            value = json.loads(schema.read_bytes())
                            value['description'] = 'changed during staged write'
                            schema.write_text(json.dumps(value))
                with patch.object(commands, '_publish', change_during_stage), self.assertRaises(commands.JournalConflict):
                    commands.run_local_command(owner, request)
                self.assertFalse((root / config['source_path']).parent.exists())

    def test_replay_rejects_source_or_receipt_corruption(self):
        with self.creation() as (root, owner, config, claim, request, *_):
            commands.run_local_command(owner, request)
            path = root / config['source_path']
            original = path.read_bytes()
            path.write_text(json.dumps({**claim, 'epistemic_status': 'reported'}))
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, request)
            path.write_bytes(original)
            receipt_path = path.with_name('source-create-receipt.json')
            receipt = json.loads(receipt_path.read_bytes())
            receipt['grants_admission'] = True
            receipt_path.write_text(json.dumps(receipt))
            with self.assertRaises(commands.JournalCorruption):
                commands.run_local_command(owner, request)


if __name__ == '__main__':
    unittest.main()

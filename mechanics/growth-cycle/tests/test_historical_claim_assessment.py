"""Synthetic legacy source/form assessment closure, not historical competence."""
import copy
from contextlib import contextmanager
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import test_source_commands as fixtures
import test_knowledge_assessment as assessment_fixtures
import assessment_journal as journal
import source_commands as source
import source_historical_claims as historical
import source_witness_bibliographic_graph_common as graph
from knowledge_assessment import Record

ROOT = fixtures.ROOT


class HistoricalClaimAssessmentTests(unittest.TestCase):
    @contextmanager
    def fixture(self, *, relative_date=False):
        from build_source_witness_catalog import collect_records, collect_claims
        with fixtures.HistoricalCreationTests().creation() as (root, *_):
            ref = 'ToS/contracts/claim-display-fields.schema.json'
            (root / ref).write_bytes((ROOT / ref).read_bytes())
            entry = next(row for row in collect_claims(root) if historical.is_path(row['source_claim_file_ref']))
            stream = root / entry['source_claim_file_ref']
            rows = [json.loads(line) for line in stream.read_bytes().splitlines() if line.strip()]
            claim = next(row for row in rows if row['claim_id'] == entry['claim_id'])
            claim.setdefault('qualifiers', {}).update(
                statement='Условная историческая связь; только механическая фикстура.',
                statement_language='ru', statement_script='Cyrl',
                display_fields={'schema_version': 'tos_claim_display_fields_v1', **{role: {
                    'text': f'Условное описание {role}; не историческое заключение.', 'language': 'ru', 'script': 'Cyrl'}
                    for role in ('name', 'caption', 'hover')}},
                unchanged_limit={'primary_inspection': False, 'unknown': [0, None, 'Ω'],
                                 'opaque_ref': 'tos.agent.not-a-selected-dependency'})
            objects = {row['record_id']: row for values in collect_records(root).values() for row in values}
            if relative_date:
                anchor = next(identity for identity, row in objects.items() if row['record_type'] == 'historical-process')
                claim.update(predicate='historical_dating', object={
                    'kind': 'relative-order', 'role': 'historical-time', 'calendar': None,
                    'year_numbering': None, 'certainty': 'uncertain',
                    'source_wording': {'text': 'Условно после процесса', 'language': 'ru'},
                    'relative': {'relation': 'after', 'anchor_ref': anchor}})
                endpoint_ids = {claim['subject_ref'], anchor}
            else:
                endpoint_ids = {claim['subject_ref'], claim['object']}
            stream.write_bytes(b''.join(source._canonical(row) + b'\n' for row in rows))
            parent = Record.from_payload(claim['claim_id'], claim['claim_version'], claim)
            changes = [source.prepare_claim_change(claim, None, 'test:historical-form-maker',
                f'tos.form.historical-assessment-{role}', 'claim.' + role,
                allowed_field_ids=source.CLAIM_FORM_FIELDS) for role in ('statement', 'name', 'caption', 'hover')]
            forms = source._apply(None, parent, changes)
            form_path = source.claim_forms_path(stream, parent.id)
            form_path.write_text(json.dumps(forms))
            bindings = [{'path': stream.relative_to(root).as_posix(), 'record_id': parent.id,
                         'origin_id': 'synthetic-historical-source'}]
            bindings += [{'path': objects[identity]['source_record_ref'], 'record_id': identity,
                          'origin_id': 'synthetic-endpoint-' + identity} for identity in sorted(endpoint_ids)]
            bindings += [{'path': form_path.relative_to(root).as_posix(), 'record_id': form['form_id'],
                          'origin_id': 'synthetic-form-source'} for form in forms['forms']]
            assessor = assessment_fixtures.AssessmentPolicyTests()
            assessor.setUp()
            self.addCleanup(assessor.doCleanups)
            owner, config, _, _ = assessor.assessed_form_fixture()
            configured_ids = {binding['record_id'] for binding in bindings}
            config.update(source_root=str(root), source_records=bindings,
                records=[row for row in config['records'] if row['id'] not in configured_ids],
                subjects={form['form_id']: {'record': source._form_ref(form),
                    'assertion_layer': 'human_projection', 'risk': 'low', 'languages': ['ru', 'de'],
                    'maker_id': form['creator_id'], 'requested_use': 'research', 'access_allowed': True}
                    for form in forms['forms']})
            owner.write_text(json.dumps(config))
            self.assertFalse((stream.parent / 'source-create-receipt.json').exists())
            yield SimpleNamespace(root=root, stream=stream, claim=claim, parent=parent, forms=forms,
                form_path=form_path, bindings=bindings, endpoint_ids=endpoint_ids,
                owner=owner, config=config, assessor=assessor)

    @contextmanager
    def no_discovery(self):
        with patch('build_source_witness_catalog.collect_records', side_effect=AssertionError('no catalog discovery')), \
             patch('build_source_witness_catalog.collect_claims', side_effect=AssertionError('no catalog discovery')), \
             patch.object(Path, 'rglob', side_effect=AssertionError('no instance discovery')):
            yield

    def test_uncaptured_historical_forms_use_actual_v2_selection_and_materialization(self):
        with self.fixture() as fx, self.no_discovery():
            before = {path: path.read_bytes() for path in (fx.stream, fx.form_path)}
            dependencies, sets = {}, {}
            records, fixity = journal._source_records(fx.root, fx.bindings, claim_dependencies=dependencies, form_sets=sets)
            selected = {row['id']: Record.from_payload(**row) for row in records}
            self.assertEqual({ref['id'] for ref in dependencies[fx.parent.id]}, fx.endpoint_ids)
            contracts = {row['path'] for row in fixity}
            self.assertTrue({'ToS/contracts/historical-claim.schema.json',
                'ToS/contracts/claim-packet.schema.json', 'ToS/contracts/claim-display-fields.schema.json',
                'ToS/doctrine/semantic-interchange/relation-types.v1.json'} <= contracts)
            for form in fx.forms['forms']:
                with self.subTest(role=form['role']):
                    fx.assessor.subject = Record.from_payload(form['form_id'], form['form_version'], form)
                    fx.assessor.source = selected[fx.parent.id]
                    describe = {'schema_version': 'tos_local_assessment_command_v1',
                        'operation': 'describe', 'subject_id': form['form_id']}
                    result = fx.assessor.run_local(fx.owner, describe)
                    required = result['result']['command_context']['required_sources']
                    self.assertEqual({ref['id'] for ref in required}, {fx.parent.id, *fx.endpoint_ids})
                    request = {**describe, 'operation': 'materialize-form',
                        'expected_subject': fx.assessor.subject.ref, 'expected_snapshot': result['owner_snapshot']}
                    pending = fx.assessor.run_local(fx.owner, request)['result']['materialization']
                    self.assertEqual(pending['state'], 'needs-assessment')
                    self.assertIsNone(pending['display_text'])
                    # Independently configured synthetic authority/competence;
                    # this exercises the consumer, not real semantic judgment.
                    review = fx.assessor.review(profile='interpretation', name='tos.review.historical-' + form['role']).assessment
                    review['evidence'] = [{'record': ref, 'stance': 'supports' if ref['id'] == fx.parent.id else 'context',
                        'locator': 'Synthetic source-selected closure; not independent historical corroboration.'}
                        for ref in required]
                    append = {**request, 'operation': 'append', 'command_id': 'test:historical-' + form['role'],
                        'expected_revision': None, 'assessments': [review]}
                    try:
                        admitted = fx.assessor.run_local(fx.owner, append)['result']
                    except journal.AssessmentRejected as error:
                        self.fail(str(error.invalid_assessments))
                    self.assertTrue(admitted['current_admission']['can_use'])
                    ready = fx.assessor.run_local(fx.owner, request)['result']['materialization']
                    self.assertEqual(ready['state'], 'ready')
                    self.assertEqual(ready['context'][0]['value'], fx.claim)
                    expected = (fx.claim['qualifiers']['statement'] if form['role'] == 'statement' else
                                fx.claim['qualifiers']['display_fields'][form['role']]['text'])
                    self.assertEqual(ready['display_text'], expected)
            self.assertEqual(before, {path: path.read_bytes() for path in before})

    def test_historical_forms_refuse_missing_or_inline_parent_endpoints_and_stale_subject(self):
        with self.fixture() as fx:
            for omitted in fx.bindings[:3]:
                with self.subTest(omitted=omitted['record_id']), self.no_discovery(), self.assertRaises(ValueError):
                    journal._source_records(fx.root, [row for row in fx.bindings if row != omitted])
            original = copy.deepcopy(fx.config)
            fx.config['source_records'] = fx.bindings[1:]
            fx.config['records'].append({'id': fx.parent.id, 'version': fx.parent.version,
                'payload': fx.claim, 'origin_id': 'synthetic-inline-shadow'})
            fx.owner.write_text(json.dumps(fx.config))
            with self.assertRaises(ValueError):
                fx.assessor.run_local(fx.owner, {'schema_version': 'tos_local_assessment_command_v1',
                    'operation': 'describe', 'subject_id': fx.forms['forms'][0]['form_id']})
            fx.owner.write_text(json.dumps(original))
            saved = fx.form_path.read_bytes()
            for change in ({'version': True}, {'version': fx.parent.version + 1}, {'digest': 'sha256:' + '0' * 64}):
                with self.subTest(change=change):
                    forms = copy.deepcopy(fx.forms)
                    forms['forms'][0]['subject'].update(change)
                    fx.form_path.write_text(json.dumps(forms))
                    with self.assertRaises(journal.JournalConflict):
                        journal._source_records(fx.root, fx.bindings)
            fx.form_path.write_bytes(saved)

    def test_historical_family_schema_domain_evidence_and_opt_in_display_stay_exact(self):
        with self.fixture() as fx:
            raw = fx.stream.read_bytes()
            rows = [json.loads(line) for line in raw.splitlines() if line.strip()]
            index = next(i for i, row in enumerate(rows) if row['claim_id'] == fx.parent.id)
            sources = fx.bindings[:3]
            for change in ({'schema_version': 'tos_claim_packet_v1'}, {'predicate': 'authored_by'},
                           {'visibility': 'local_only'}, {'evidence_refs': []}, {'counterevidence_refs': [True]},
                           {'assertion_layer': 'bibliographic_assertion'},
                           {'object': fx.claim['subject_ref']}, {'subject_ref': fx.claim['object']},
                           {'qualifiers': {**fx.claim['qualifiers'], 'display_fields': {
                               'schema_version': 'tos_claim_display_fields_v1', 'name': {'text': 5}}}}):
                with self.subTest(change=change):
                    changed = copy.deepcopy(rows)
                    changed[index].update(change)
                    fx.stream.write_bytes(b''.join(source._canonical(row) + b'\n' for row in changed))
                    with self.assertRaises((ValueError, PermissionError, journal.ValidationError)):
                        journal._source_records(fx.root, sources)
            unknown = copy.deepcopy(rows)
            unknown[index]['qualifiers']['display_fields'] = {'schema_version': 'unimplemented-display-v99', 'opaque': [False, 0, None]}
            fx.stream.write_bytes(b''.join(source._canonical(row) + b'\n' for row in unknown))
            self.assertEqual(journal._source_records(fx.root, sources)[0][0]['payload'], unknown[index])
            fx.stream.write_bytes(raw)
            registry_path = fx.root / graph.HISTORICAL_REGISTRY_REFS[1]
            registry_raw = registry_path.read_bytes()
            registry = json.loads(registry_raw)
            relation = next(row for row in registry['relations'] if any(
                mapping.get('source_predicate_id') == fx.claim['predicate'] for mapping in row['source_mappings']))
            relation['domain_type_ids'] = ['tos.entity.place']
            registry_path.write_text(json.dumps(registry))
            with self.assertRaisesRegex(ValueError, 'registry domain/range'):
                journal._source_records(fx.root, sources)
            registry_path.write_bytes(registry_raw)
            for relative in ('ToS/source-witnesses/relations/unsupported/historical-claims.jsonl',
                             'ToS/source-witnesses/history/fixture/source-claims.jsonl'):
                with self.subTest(path=relative):
                    target = fx.root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(raw)
                    selected = [{**sources[0], 'path': relative}, *sources[1:]]
                    with self.assertRaises(ValueError):
                        journal._source_records(fx.root, selected)
        with self.fixture(relative_date=True) as fx:
            dependencies = {}
            journal._source_records(fx.root, fx.bindings, claim_dependencies=dependencies)
            self.assertEqual({ref['id'] for ref in dependencies[fx.parent.id]}, fx.endpoint_ids)
            anchor = fx.claim['object']['relative']['anchor_ref']
            with self.assertRaises(ValueError):
                journal._source_records(fx.root, [row for row in fx.bindings if row['record_id'] != anchor])

    def test_historical_contract_fixity_and_unique_eight_mib_input_budget(self):
        with self.fixture() as fx:
            _, fixity = journal._source_records(fx.root, fx.bindings)
            refs = [row['path'] for row in fixity]
            self.assertEqual(len(refs), len(set(refs)))
            used = sum((fx.root / ref).stat().st_size for ref in refs)
            raw = fx.stream.read_bytes()
            padded = raw + b' ' * (8 * journal.MAX_RECORD_BYTES - used)
            fx.stream.write_bytes(padded)
            with self.no_discovery():
                self.assertEqual(len(journal._source_records(fx.root, fx.bindings)[0]), len(fx.bindings))
                fx.stream.write_bytes(padded + b' ')
                with self.assertRaisesRegex(ValueError, '8 MiB'):
                    journal._source_records(fx.root, fx.bindings)
            fx.stream.write_bytes(raw)
            contract_path = fx.root / 'ToS/contracts/claim-display-fields.schema.json'
            contract = contract_path.read_bytes()
            real_validate = graph._validate_historical_claim
            def drift(*args, **kwargs):
                result = real_validate(*args, **kwargs)
                contract_path.write_bytes(contract + b'\n')
                return result
            with patch.object(graph, '_validate_historical_claim', side_effect=drift), \
                 self.assertRaisesRegex(journal.JournalConflict, 'dependency changed'):
                journal._source_records(fx.root, fx.bindings)
            contract_path.write_bytes(contract)
            describe = {'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe',
                'subject_id': fx.forms['forms'][0]['form_id']}
            described = fx.assessor.run_local(fx.owner, describe)
            contract_path.write_bytes(contract + b'\n')
            with self.assertRaisesRegex(journal.JournalConflict, 'owner snapshot is stale'):
                fx.assessor.run_local(fx.owner, {**describe, 'operation': 'materialize-form',
                    'expected_subject': source._form_ref(fx.forms['forms'][0]),
                    'expected_snapshot': described['owner_snapshot']})
            contract_path.write_bytes(contract)


if __name__ == '__main__':
    unittest.main()

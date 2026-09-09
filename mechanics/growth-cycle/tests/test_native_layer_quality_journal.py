"""Synthetic quality dependencies: historical evidence is not current use.

The fixtures do not attest source rights, model competence or historical text.
They exercise exact closure, per-purpose admission and the private command path.
"""
from __future__ import annotations

import copy
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(ROOT / 'scripts'), str(ROOT / 'tests'),
    str(ROOT / 'mechanics/growth-cycle/tests'),
    str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]

from assessment_journal import (_quality_requirements, _quality_basis,
                                _validate_quality_dependencies, JournalConflict)
from knowledge_assessment import Record
import assessment_journal
import test_knowledge_assessment as policy_tests
from test_native_text_layer_assessment import NativeLayerAssessmentFixture
from test_occurrence_growth import copy_contracts, occurrence
from native_text_binding import NativeTextBindingResolver
from source_owner_context import OwnerLocalSourceContext
from source_text_unit_proposal import build_text_unit_proposal
import source_text_unit_commands
import source_commands
import source_owner_claim_commands
from source_witness_human_forms import claim_forms_path
from test_source_owner_record_profiles import lexeme


def encode(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True) + '\n').encode()


def envelope(record):
    return {'id': record.id, 'version': record.version, 'payload': record.payload,
            'origin_id': record.origin_id}


class QualityJournalFixture:
    """Real private adapters over synthetic source, policy and execution inputs."""
    def __init__(self, test, *, source=False, claim=False):
        self.fx = NativeLayerAssessmentFixture(test)
        self.policy_fixture = policy_tests.AssessmentPolicyTests(methodName='runTest')
        self.policy_fixture.setUp()
        test.addCleanup(self.policy_fixture.doCleanups)
        _, self.config, _ = self.policy_fixture.local_command_fixture()
        self.owner = self.fx.base / 'quality-assessment-owner.json'
        self.journal = self.fx.store / '.quality-assessment-journal'
        self.journal.mkdir(mode=0o700)
        copy_contracts(self.fx.public)
        for name in ('native-text-layer-quality-basis', 'native-text-unit-assessment-subject'):
            target = self.fx.public / 'ToS/contracts' / (name + '.schema.json')
            target.write_bytes((ROOT / 'ToS/contracts' / target.name).read_bytes())
        self.policy = Record.from_payload('tos.policy.knowledge-assessment', 3, json.loads(
            (ROOT / 'ToS/doctrine/semantic-interchange/assessment-policy.v3.json').read_text()))
        self.use = 'text-layer:semantic-analysis' if source or claim else 'text-layer:citation'
        self.fx.subjects[self.fx.layer_id]['requested_use'] = self.use
        self.config.update(schema_version='tos_local_assessment_owner_v5', policy=envelope(self.policy),
            source_context_ref=str(self.fx.context_path), source_records=[], owner_local_source_records=[],
            native_text_units=[], native_text_layers=self.fx.selections,
            subjects=copy.deepcopy(self.fx.subjects), quality_dependencies={}, journal_directory=str(self.journal))
        self.config['records'] = [row for row in self.config['records']
                                  if row['id'] != self.policy_fixture.subject.id]
        profile_ids = [row['profile_id'] for row in self.policy.payload['profiles']]
        for index, competence in enumerate(self.config['competencies']):
            competence['payload'].update(assertion_layers=['textual_observation', 'semantic_interpretation',
                'human_projection'], languages=['und', 'en', 'ru'], profile_ids=profile_ids)
            self.config['authorities'][index]['payload'].update(policy=self.policy.ref,
                competence_refs=[Record.from_payload(**competence).ref],
                assertion_layers=competence['payload']['assertion_layers'], languages=['und', 'en', 'ru'],
                profile_ids=profile_ids, uses=['research', self.use],
                subject_prefixes=['tos.text-layer.', 'tos.text-unit.', 'tos.occurrence.', 'tos.claim.', 'tos.form.'])
        self.layer_record = Record.from_payload(self.fx.layer_id, 1, self.fx.layer,
                                               origin_id=self.fx.selections[0]['origin_id'])
        self._unit()
        self.target = self.unit
        if source or claim:
            self._source()
        if claim:
            self._create_claim()
        self.save()

    def _unit(self):
        cfg = copy.deepcopy(self.fx.seed.seed.config)
        cfg['unit_slots'] = [cfg['unit_slots'][0]]
        cfg['unit_slots'][0]['unit_kind'] = 'document'
        cfg['scheme'].update(analysis_role='source_structure', boundary_basis='source_markup')
        cfg['scheme']['policies'].update(whitespace='included_in_neighbor', line_break='included_in_neighbor')
        span = copy.deepcopy(self.fx.seed.seed.proposal['spans'][0])
        span.update(start=0, end=len(self.fx.content.decode()))
        packet = build_text_unit_proposal(verified_layer=self.fx.layer, verified_layer_binding=self.fx.binding,
            exact_text=self.fx.content.decode(), scope={'start': 0, 'end': span['end']},
            identities=source_text_unit_commands._identities(cfg), spans=[span], excluded_gaps=[],
            scheme=cfg['scheme'], method=cfg['method'])
        self.packet_ref = cfg['source_path']
        path = self.fx.store / self.packet_ref
        path.parent.mkdir(mode=0o700, parents=True)
        path.write_bytes(encode(packet))
        path.chmod(0o600)
        unit, segmentation = packet['units'][0], packet['segmentations'][0]
        self.native = {'schema_version': 'tos_native_text_unit_binding_v1', 'packet_ref': self.packet_ref,
            'packet_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'packet_id': packet['packet_id'], 'packet_version': 1,
            'segmentation_id': segmentation['segmentation_id'], 'segmentation_version': 1,
            'unit_id': unit['unit_id'], 'unit_version': 1, 'ordered_anchor_refs': unit['ordered_anchor_refs'],
            'text_layer': self.fx.binding['text_layer'], 'source_record_refs': self.fx.binding['source_record_refs']}
        self.access = {'read_scope': 'exact_owner_local', 'access_allowed': True,
                       'authority_ref': 'operator:synthetic-native-read'}
        adapted = NativeTextBindingResolver(self.fx.public,
            owner_context=OwnerLocalSourceContext.load(self.fx.context_path)).assessment_records(
                self.native, origin_id=self.layer_record.origin_id, verify_content=True, allow_private_content=True)
        self.unit = Record.from_payload(**adapted['records'][0])
        self.config['native_text_units'] = [{'binding': self.native, 'origin_id': self.layer_record.origin_id,
                                             'source_access': self.access}]
        self.config['subjects'][self.unit.id] = {'record': self.unit.ref, 'assertion_layer': 'textual_observation',
            'risk': 'low', 'languages': ['und'], 'maker_id': segmentation['maker']['agent_ref'],
            'requested_use': 'research', 'access_allowed': True}
        self.config['quality_dependencies'][self.unit.id] = [
            {'layer_id': self.layer_record.id, 'use': 'text-layer:citation'}]

    def _source(self):
        body = occurrence(self.native)
        body['visibility'] = 'local_only'
        self.source_ref = self.fx.seed.prefix + 'descriptions/quality/occurrence.json'
        self.source_path = self.fx.store / self.source_ref
        self.source_path.parent.parent.mkdir(mode=0o700)
        self.source_path.parent.mkdir(mode=0o700)
        self.source_path.write_bytes(encode(body))
        self.source_path.chmod(0o600)
        self.target = Record.from_payload(body['record_id'], body['record_version'], body,
                                          origin_id=self.layer_record.origin_id)
        self.config['owner_local_source_records'] = [{'path': self.source_ref, 'record_id': self.target.id,
            'profile_type_id': 'tos.entity.occurrence', 'origin_id': self.target.origin_id,
            'source_access': self.access, 'source_binding': self.native, 'form_ids': []}]
        self.config['subjects'][self.target.id] = {'record': self.target.ref, 'assertion_layer': 'semantic_interpretation',
            'risk': 'low', 'languages': ['ru', 'en', 'und'], 'maker_id': 'agent:synthetic-description-maker',
            'requested_use': 'research', 'access_allowed': True}
        self.config['quality_dependencies'][self.target.id] = [{'layer_id': self.layer_record.id, 'use': self.use}]

    def save(self):
        self.owner.write_bytes(encode(self.config))
        self.owner.chmod(0o600)

    def _create_claim(self):
        """Use the actual private Claim writer, including its initial source-copy."""
        conception = lexeme()
        conception.pop('semantic_content')
        conception.update(schema_version='tos_semantic_description_record_v1', record_type='conception',
            record_id='tos.conception.synthetic.quality', visibility='public_metadata_only')
        conception_ref = 'ToS/source-witnesses/semantic-descriptions/quality/conception.json'
        path = self.fx.public / conception_ref
        path.parent.mkdir(parents=True)
        path.write_bytes(encode(conception))
        expression_ref = self.native['source_record_refs']['expression']
        expression = json.loads((self.fx.public / expression_ref).read_bytes())
        body = {'schema_version': 'tos_semantic_relation_claim_v1', 'claim_id': 'tos.claim.synthetic.quality',
            'claim_version': 1, 'claim_type': 'relation', 'assertion_layer': 'semantic_interpretation',
            'subject_ref': conception['record_id'], 'predicate': 'conception_expressed_in',
            'object': expression['record_id'], 'evidence_refs': [self.source_ref],
            'maker': {'maker_type': 'model', 'agent_ref': 'model:synthetic-claim-writer'},
            'provenance_event_ref': 'tos.event.synthetic.quality-claim-creation',
            'epistemic_status': 'uncertain', 'review_status': 'unreviewed', 'visibility': 'local_only',
            'polarity': 'unknown', 'qualifiers': {'statement': 'Синтетическая возможность, не установленный вывод.',
                'statement_language': 'ru', 'statement_script': 'Cyrl',
                'relation_basis': 'Synthetic selected occurrence, not a historical or semantic judgment.',
                'unknown': {'zero': 0, 'negative': False}}}
        sources = [{'path': conception_ref, 'record_id': conception['record_id'],
            'profile_type_id': 'tos.entity.conception', 'origin_id': 'synthetic-conception',
            'source_access': {**self.access, 'read_scope': 'metadata_only'}, 'source_binding': None},
            {'path': expression_ref, 'record_id': expression['record_id'], 'profile_type_id': 'tos.entity.expression',
             'origin_id': 'synthetic-expression', 'source_access': {**self.access, 'read_scope': 'metadata_only'},
             'source_binding': None},
            {key: value for key, value in self.config['owner_local_source_records'][0].items() if key != 'form_ids'}]
        selected = {'claim_id': body['claim_id'], 'relation_type_id': 'tos.relation.conception-expressed-in',
            'origin_id': self.layer_record.origin_id, 'source_access': self.access, 'source_records': sources,
            'native_bindings': [], 'verify_content': True}
        claim_ref = self.fx.seed.prefix + 'claims/quality/source-claims.jsonl'
        (self.fx.store / claim_ref).parent.parent.mkdir(mode=0o700)
        form_id = 'tos.form.synthetic.created-quality-statement'
        config = {'schema_version': source_owner_claim_commands.CONFIG, 'uid': os.getuid(),
            'principal_id': body['maker']['agent_ref'], 'maker_type': 'model',
            'authority_ref': 'operator:synthetic-quality-claim-creation', 'expires_at': '2099-01-01T00:00:00Z',
            'source_context_ref': str(self.fx.context_path), 'source_path': claim_ref,
            'provenance_event_id': body['provenance_event_ref'], 'allowed_operations': ['claims.create'],
            'allowed_claim_ids': [body['claim_id']], 'allowed_subject_refs': [body['subject_ref']],
            'allowed_object_refs': [body['object']], 'allowed_predicates': [body['predicate']],
            'allowed_evidence_refs': body['evidence_refs'], 'allowed_form_ids': [form_id], 'allowed_fields': [],
            'claim_selections': [selected]}
        owner = self.fx.base / 'claim-source-owner.json'
        owner.write_bytes(encode(config))
        owner.chmod(0o600)
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create',
            'claims': [body], 'forms': [{'claim_id': body['claim_id'], 'form_id': form_id, 'field_id': 'claim.statement'}]}
        prepared = source_commands.run_local_command(owner, request)
        self.created = source_commands.run_local_command(owner, {**request, 'operation': 'claims.create',
            'command_id': 'synthetic-quality-claim-created', 'expected_source': None, 'expected_revision': None,
            'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'], 'expected_inputs': prepared['source_bindings']})
        self.config['subjects'].pop(self.target.id)
        self.config['quality_dependencies'].pop(self.target.id)
        self.config['subjects'].pop(self.unit.id)
        self.config['quality_dependencies'].pop(self.unit.id)
        self.target = Record.from_payload(body['claim_id'], 1, body, origin_id=self.layer_record.origin_id)
        self.source_path = self.fx.store / claim_ref
        self.form_path = claim_forms_path(self.source_path, self.target.id)
        form = json.loads(self.form_path.read_bytes())['forms'][0]
        self.form = Record.from_payload(form_id, 1, form, origin_id=self.layer_record.origin_id)
        self.config.update(owner_local_source_records=[], native_text_units=[],
            owner_local_source_claims=[{**selected, 'path': claim_ref, 'form_ids': [form_id]}])
        for record, layer, maker in ((self.target, 'semantic_interpretation', body['maker']['agent_ref']),
                                     (self.form, 'human_projection', form['creator_id'])):
            self.config['subjects'][record.id] = {'record': record.ref, 'assertion_layer': layer,
                'risk': 'low', 'languages': ['ru', 'en', 'und'], 'maker_id': maker,
                'requested_use': 'research', 'access_allowed': True}
            self.config['quality_dependencies'][record.id] = [{'layer_id': self.layer_record.id, 'use': self.use}]

    def actor(self, index):
        self.config['principal_id'] = self.config['authorities'][index]['payload']['actor_id']
        self.save()

    def run(self, request):
        return self.policy_fixture.run_local(self.owner, request)

    def describe(self, record=None):
        return self.run({'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe',
                         'subject_id': (record or self.target).id})

    def request(self, operation='append', *, record=None, name='synthetic-quality-review', decision='admit',
                supersedes=(), limits=()):
        record = record or self.target
        described = self.describe(record)
        result = {'schema_version': 'tos_local_assessment_command_v1', 'operation': operation,
            'subject_id': record.id, 'expected_subject': record.ref, 'expected_snapshot': described['owner_snapshot']}
        if operation == 'append':
            command = described['result']['command_context']
            index = next(index for index, grant in enumerate(self.config['authorities'])
                         if grant['payload']['actor_id'] == self.config['principal_id'])
            review = copy.deepcopy(self.policy_fixture.review(index).assessment)
            evidence = [*command.get('required_sources', ()),
                        *(row['basis'] for row in command.get('required_admissions', ()))]
            review.update(assessment_id='tos.review.' + name, subject=record.ref, policy=self.policy.ref,
                profile_id='text-layer-quality' if record.id == self.layer_record.id else
                    'source-observation' if record.id == self.unit.id else 'interpretation',
                decision=decision, limits=list(limits), supersedes=list(supersedes),
                authority=Record.from_payload(**self.config['authorities'][index]).ref,
                competence=Record.from_payload(**self.config['competencies'][index]).ref,
                evidence=[{'record': ref, 'stance': 'supports', 'locator': 'Synthetic explicit comparison or quality context.'}
                          for ref in evidence])
            result.update(command_id=name, expected_revision=described['result']['revision'], assessments=[review])
        return result

    def admit_quality(self, name='synthetic-layer-admit', **kwargs):
        return self.run(self.request(record=self.layer_record, name=name, **kwargs))

    def add_form(self, *, bind_quality=True):
        reader = self.fx.reader()
        admission = self.describe(self.layer_record)['result']['current_admission']
        validator = Draft202012Validator(json.loads(
            (ROOT / 'ToS/contracts/native-text-layer-quality-basis.schema.json').read_text()))
        self.basis = _quality_basis(reader.layers[self.layer_record.id], admission, self.use, validator)
        body = {'schema_version': 'tos_human_form_v1', 'form_id': 'tos.form.synthetic.quality-reading',
            'form_version': 1, 'subject': self.target.ref, 'role': 'hover', 'language': 'ru', 'script': 'Cyrl',
            'creator_id': 'agent:synthetic-form-maker', 'revises': None,
            'bindings': {'context': {'record': self.target.ref, 'pointer': ''}},
            'content': {'kind': 'freeform', 'text': 'Синтетическая форма с ограниченным основанием качества.'}}
        if bind_quality:
            body['bindings']['quality'] = {'record': self.basis.ref, 'pointer': ''}
        self.form = Record.from_payload(body['form_id'], 1, body, origin_id=self.target.origin_id)
        self.form_path = self.source_path.with_name('occurrence.human-forms.json')
        self.form_path.write_bytes(encode({'schema_version': 'tos_human_form_set_v1',
            'subject': self.target.ref, 'forms': [body], 'prior_forms': []}))
        self.form_path.chmod(0o600)
        self.config['owner_local_source_records'][0]['form_ids'] = [self.form.id]
        self.config['subjects'][self.form.id] = {'record': self.form.ref, 'assertion_layer': 'human_projection',
            'risk': 'low', 'languages': ['ru', 'en', 'und'], 'maker_id': body['creator_id'],
            'requested_use': 'research', 'access_allowed': True}
        self.config['quality_dependencies'][self.form.id] = [{'layer_id': self.layer_record.id, 'use': self.use}]
        self.save()

    def history_bytes(self):
        return {str(path.relative_to(self.journal)): path.read_bytes()
                for path in self.journal.rglob('*') if path.is_file()}


class QualityJournalTests(unittest.TestCase):
    def test_source_created_claim_copy_uses_current_quality_review_without_source_rewrite(self):
        fixture = QualityJournalFixture(self, claim=True)
        before = {path: path.read_bytes() for path in (fixture.source_path, fixture.form_path,
            fixture.fx.store / fixture.packet_ref, fixture.fx.store / fixture.fx.source_ref)}
        self.assertEqual(fixture.form.payload['content']['kind'], 'source-copy')
        self.assertTrue(all(binding['record'] == fixture.target.ref
                            for binding in fixture.form.payload['bindings'].values()))
        source_view = fixture.created['materializations'][0]
        self.assertEqual(source_view['state'], 'ready')
        self.assertIsNone(source_view['admission'])
        self.assertIn('materialize-form', fixture.describe(fixture.form)['result']['command_context']['supported_operations'])
        closed = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(closed['state'], 'needs-assessment')
        self.assertFalse(closed['admission']['can_use'])
        quality = fixture.admit_quality(decision='admit-with-limits', limits=['synthetic selected layer only'])
        unreviewed = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(unreviewed['state'], 'needs-assessment')
        self.assertFalse(unreviewed['admission']['can_use'])
        review = fixture.request(record=fixture.form, name='source-copy-reviewed',
            decision='admit-with-limits', limits=['synthetic form reading only'])
        fixture.run(review)
        ready = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(ready['state'], 'ready')
        self.assertEqual(ready['derivation'], 'source-copy')
        self.assertEqual(ready['display_text'], fixture.target.payload['qualifiers']['statement'])
        self.assertEqual((ready['language'], ready['script']), ('ru', 'Cyrl'))
        self.assertTrue(ready['admission']['can_use'])
        self.assertEqual(ready['admission']['subject'], fixture.form.ref)
        self.assertEqual(ready['subject_assessment']['subject'], fixture.target.ref)
        self.assertEqual(ready['subject_assessment']['admission']['status'], 'unreviewed')
        self.assertFalse(ready['subject_assessment']['admission']['can_use'])
        self.assertFalse(ready['subject_assessment']['form_admission_is_parent_endorsement'])
        self.assertEqual(ready['admission']['limits'], ['synthetic form reading only', 'synthetic selected layer only'])
        self.assertEqual(ready['context'][0]['value'], fixture.target.payload)
        self.assertEqual(ready['context'][-1]['slot'], 'owner:quality:0')
        self.assertFalse(ready['standalone_reading'])
        self.assertIn(ready['context'][-1]['binding']['record'], ready['dependencies'])
        history = fixture.history_bytes()
        request = fixture.request('materialize-form', record=fixture.form)
        self.assertEqual(fixture.run(request)['result']['materialization'], ready)
        self.assertEqual(fixture.history_bytes(), history)
        fixture.admit_quality(name='copy-quality-withdrawn', decision='withdraw',
            supersedes=quality['result']['current_admission']['assessment_refs'])
        with self.assertRaises(JournalConflict):
            fixture.run(request)
        stopped = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(stopped['state'], 'needs-assessment')
        self.assertFalse(stopped['admission']['can_use'])
        self.assertIsNone(stopped['display_text'])
        fixture.admit_quality(name='copy-quality-renewed')
        stale_review = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(stale_review['state'], 'needs-assessment')
        self.assertFalse(stale_review['admission']['can_use'])
        fixture.run(fixture.request(record=fixture.form, name='same-copy-renewed-review'))
        renewed = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(renewed['state'], 'ready')
        self.assertEqual(renewed['form'], ready['form'])
        self.assertEqual(renewed['display_text'], ready['display_text'])
        self.assertNotEqual(renewed['context'][-1]['binding'], ready['context'][-1]['binding'])
        self.assertEqual({path: path.read_bytes() for path in before}, before)
        self.assertTrue(all(fixture.history_bytes()[name] == raw for name, raw in history.items()
                            if not name.endswith('/head')))

    def test_assessed_copy_access_expiry_and_lock_time_drift_never_emit_wording(self):
        fixture = QualityJournalFixture(self, claim=True)
        fixture.admit_quality()
        fixture.run(fixture.request(record=fixture.form, name='copy-before-refusal'))
        request = fixture.request('materialize-form', record=fixture.form)
        before, history = fixture.form_path.read_bytes(), fixture.history_bytes()
        future = '2028-01-01T00:00:00Z'
        described = fixture.policy_fixture.run_local(fixture.owner,
            {'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe',
             'subject_id': fixture.form.id}, now=future)
        expired = fixture.policy_fixture.run_local(fixture.owner,
            {**request, 'expected_snapshot': described['owner_snapshot']}, now=future)
        self.assertNotEqual(expired['result']['materialization']['state'], 'ready')
        self.assertIsNone(expired['result']['materialization']['display_text'])
        self.assertFalse(expired['result']['materialization']['admission']['can_use'])
        original = assessment_journal.AssessmentJournal.locked_subjects
        observed = []
        @contextmanager
        def changed(journal, identifiers):
            observed.extend(identifiers)
            with original(journal, identifiers):
                fixture.config['subjects'][fixture.form.id]['access_allowed'] = False
                fixture.save()
                yield
        with patch.object(assessment_journal.AssessmentJournal, 'locked_subjects', changed), self.assertRaises(JournalConflict):
            fixture.run(request)
        self.assertEqual(set(observed), {fixture.form.id, fixture.target.id, fixture.layer_record.id})
        with self.assertRaises(PermissionError):
            fixture.run(request)
        self.assertEqual(fixture.form_path.read_bytes(), before)
        self.assertEqual(fixture.history_bytes(), history)

    def test_current_parent_withdrawal_dispute_and_limits_are_visible_without_endorsing_the_claim(self):
        fixture = QualityJournalFixture(self, claim=True)
        fixture.admit_quality()
        form_before, claim_before = fixture.form_path.read_bytes(), fixture.source_path.read_bytes()
        fixture.actor(1)
        admitted = fixture.run(fixture.request(name='parent-reviewed', decision='admit-with-limits',
            limits=['parent synthetic interpretation only']))
        fixture.actor(0)
        unreviewed = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(unreviewed['state'], 'needs-assessment')
        self.assertFalse(unreviewed['admission']['can_use'])
        self.assertTrue(unreviewed['subject_assessment']['admission']['can_use'])
        self.assertIsNone(unreviewed['display_text'])
        fixture.run(fixture.request(record=fixture.form, name='qualified-copy-reading'))
        request = fixture.request('materialize-form', record=fixture.form)
        packet = fixture.run(request)['result']['materialization']
        self.assertTrue(packet['admission']['can_use'])
        self.assertTrue(packet['subject_assessment']['admission']['can_use'])
        self.assertEqual(packet['subject_assessment']['admission']['limits'], ['parent synthetic interpretation only'])
        fixture.actor(1)
        withdrawn = fixture.run(fixture.request(name='parent-withdrawn', decision='withdraw',
            supersedes=admitted['result']['current_admission']['assessment_refs']))
        withdrawal_ref = withdrawn['result']['current_admission']['assessment_refs'][0]
        fixture.actor(0)
        with self.assertRaises(JournalConflict):
            fixture.run(request)
        packet = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(packet['state'], 'ready')
        self.assertTrue(packet['admission']['can_use'])
        self.assertFalse(packet['subject_assessment']['admission']['can_use'])
        self.assertEqual(packet['subject_assessment']['historical_withdrawals'], [withdrawal_ref])
        for decision, status in (('reject', 'rejected'), ('dispute', 'disputed')):
            fixture.actor(1)
            fixture.run(fixture.request(name='parent-' + decision, decision=decision, limits=['parent ' + status]))
            fixture.actor(0)
            packet = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
            self.assertEqual(packet['state'], 'ready')
            self.assertTrue(packet['admission']['can_use'])
            self.assertFalse(packet['subject_assessment']['admission']['can_use'])
            self.assertEqual(packet['subject_assessment']['admission']['status'], status)
            self.assertIn('parent ' + status, packet['subject_assessment']['admission']['limits'])
            self.assertEqual(packet['subject_assessment']['historical_withdrawals'], [withdrawal_ref])
            self.assertFalse(packet['standalone_reading'])
            self.assertFalse(packet['subject_assessment']['form_admission_is_parent_endorsement'])
        request = fixture.request('materialize-form', record=fixture.form)
        history = fixture.history_bytes()
        original_load, parent_reads = assessment_journal.AssessmentJournal._load, []
        def changed_parent_head(journal, identity):
            revision, chain = original_load(journal, identity)
            if identity == fixture.target.id:
                parent_reads.append(revision)
                if len(parent_reads) == 2:
                    return 'f' * 64, chain
            return revision, chain
        with patch.object(assessment_journal.AssessmentJournal, '_load', changed_parent_head), self.assertRaises(JournalConflict):
            fixture.run(request)
        self.assertEqual(len(parent_reads), 2)
        self.assertEqual(fixture.history_bytes(), history)
        original = copy.deepcopy(fixture.config)
        for mutation in ('missing', 'access', 'use', 'maker', 'source', 'languages'):
            with self.subTest(parent_scope=mutation):
                fixture.config = copy.deepcopy(original)
                scope = fixture.config['subjects'][fixture.target.id]
                if mutation == 'missing':
                    fixture.config['subjects'].pop(fixture.target.id)
                    fixture.config['quality_dependencies'].pop(fixture.target.id)
                elif mutation == 'access':
                    scope['access_allowed'] = False
                elif mutation == 'use':
                    scope['requested_use'] = 'another-use'
                elif mutation == 'maker':
                    scope['maker_id'] = 'another-maker'
                elif mutation == 'source':
                    scope['record']['digest'] = 'sha256:' + 'a' * 64
                else:
                    scope['languages'] = ['en']
                fixture.save()
                with self.assertRaises((PermissionError, JournalConflict)):
                    fixture.describe(fixture.form)
        self.assertEqual(fixture.form_path.read_bytes(), form_before)
        self.assertEqual(fixture.source_path.read_bytes(), claim_before)

    def test_assessed_copy_requires_owned_field_language_and_whole_claim_context(self):
        fixture = QualityJournalFixture(self, claim=True)
        fixture.admit_quality()
        original = json.loads(fixture.form_path.read_bytes())
        for mutation in ('language', 'role', 'context', 'pointer'):
            with self.subTest(mutation=mutation):
                form_set = copy.deepcopy(original)
                body = form_set['forms'][0]
                if mutation == 'language':
                    body['language'] = 'en'
                elif mutation == 'role':
                    body['role'] = 'hover'
                elif mutation == 'context':
                    body['bindings'] = {key: value for key, value in body['bindings'].items() if value['pointer']}
                else:
                    body['bindings'][body['content']['slot']]['pointer'] = '/predicate'
                fixture.form = Record.from_payload(body['form_id'], 1, body, origin_id=fixture.target.origin_id)
                fixture.config['subjects'][fixture.form.id]['record'] = fixture.form.ref
                fixture.form_path.write_bytes(encode(form_set))
                fixture.save()
                if mutation in ('role', 'pointer'):
                    with self.assertRaises(PermissionError):
                        fixture.run(fixture.request('materialize-form', record=fixture.form))
                else:
                    result = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
                    self.assertEqual(result['state'], 'invalid')
                    self.assertIsNone(result['display_text'])
                    self.assertEqual(result['issues'], ['context.omitted'] if mutation == 'context'
                        else ['source-copy.language-not-bound-to-source'])

    def test_exact_comparison_and_quality_append_preserve_native_source(self):
        fixture = QualityJournalFixture(self)
        before = {str(path): path.read_bytes() for path in (
            fixture.fx.store / fixture.fx.source_ref, fixture.fx.store / fixture.packet_ref, fixture.fx.payload)}
        described = fixture.describe(fixture.layer_record)
        self.assertIn('read-layer-comparison', described['result']['command_context']['supported_operations'])
        self.assertNotIn(fixture.fx.content.decode(), json.dumps(described))
        comparison = fixture.run(fixture.request('read-layer-comparison', record=fixture.layer_record))
        self.assertEqual(comparison['result']['source_comparison']['payload']['source_member_utf8'], fixture.fx.member.decode())
        self.assertFalse(comparison['publication_authorized'])
        request = fixture.request(record=fixture.layer_record)
        first = fixture.run(request)
        self.assertTrue(first['result']['current_admission']['can_use'])
        self.assertTrue(fixture.run(request)['result']['replayed'])
        self.assertEqual({name: Path(name).read_bytes() for name in before}, before)
        self.assertFalse(fixture.fx.layer['admission']['human_review_performed'])

    def test_unit_requires_current_quality_and_explicit_basis_evidence(self):
        fixture = QualityJournalFixture(self)
        request = fixture.request(name='unit-without-quality')
        with self.assertRaises(assessment_journal.AssessmentRejected):
            fixture.run(request)
        fixture.admit_quality()
        request = fixture.request(name='unit-with-quality')
        omitted = copy.deepcopy(request)
        omitted['assessments'][0]['evidence'] = []
        with self.assertRaises(assessment_journal.AssessmentRejected):
            fixture.run(omitted)
        admitted = fixture.run(request)
        self.assertTrue(admitted['result']['current_admission']['can_use'])
        self.assertTrue(fixture.run(request)['result']['replayed'])

    def test_withdrawal_and_new_positive_quality_do_not_resurrect_old_dependent_evidence(self):
        fixture = QualityJournalFixture(self, source=True)
        first = fixture.admit_quality()['result']
        quality_ref = first['current_admission']['assessment_refs'][0]
        dependent = fixture.run(fixture.request(name='description-admit'))
        self.assertTrue(dependent['result']['current_admission']['can_use'])
        old_history = fixture.history_bytes()
        withdrawal = fixture.admit_quality(name='quality-withdraw', decision='withdraw', supersedes=[quality_ref])
        self.assertFalse(withdrawal['result']['current_admission']['can_use'])
        self.assertFalse(fixture.describe()['result']['current_admission']['can_use'])
        fixture.admit_quality(name='quality-renewed')
        self.assertFalse(fixture.describe()['result']['current_admission']['can_use'])
        renewed = fixture.run(fixture.request(name='description-reassessed'))
        self.assertTrue(renewed['result']['current_admission']['can_use'])
        self.assertTrue(all(fixture.history_bytes()[name] == raw for name, raw in old_history.items()
                            if not name.endswith('/head')))

    def test_public_adapter_and_invalid_grants_fail_before_private_context(self):
        fixture = QualityJournalFixture(self)
        request = {'schema_version': 'tos_local_assessment_command_v1', 'operation': 'describe',
                   'subject_id': fixture.layer_record.id}
        with patch.object(OwnerLocalSourceContext, 'load', side_effect=AssertionError('private context opened')):
            with self.assertRaises(PermissionError):
                assessment_journal.run_public_source_command(fixture.owner, request)
            fixture.config['native_text_layers'][0]['payload_access']['access_allowed'] = False
            fixture.save()
            with self.assertRaises(PermissionError):
                fixture.run(request)

    def test_quality_reviewer_revocation_closes_other_reviewers_dependent_use(self):
        fixture = QualityJournalFixture(self)
        fixture.admit_quality()
        fixture.actor(1)
        self.assertTrue(fixture.run(fixture.request(name='independent-unit-admit'))['result']['current_admission']['can_use'])
        history = fixture.history_bytes()
        grant = fixture.config['authorities'][0]
        grant['version'] += 1
        grant['payload'].update(authority_version=grant['version'], state='revoked')
        fixture.save()
        described = fixture.describe()['result']
        self.assertFalse(described['command_context']['required_admissions'][0]['can_use'])
        self.assertFalse(described['current_admission']['can_use'])
        self.assertEqual(fixture.config['authorities'][1]['payload']['state'], 'active')
        self.assertEqual(fixture.history_bytes(), history)

    def test_source_configuration_change_while_waiting_for_locks_prevents_append(self):
        fixture = QualityJournalFixture(self)
        fixture.admit_quality()
        request = fixture.request(name='stale-at-lock')
        history = fixture.history_bytes()
        original = assessment_journal.AssessmentJournal.locked_subjects
        observed = []

        @contextmanager
        def changed(journal, identifiers):
            observed.extend(identifiers)
            with original(journal, identifiers):
                fixture.config['native_text_layers'][0]['payload_access']['access_allowed'] = False
                fixture.save()
                yield

        with patch.object(assessment_journal.AssessmentJournal, 'locked_subjects', changed), \
                self.assertRaises(JournalConflict):
            fixture.run(request)
        self.assertEqual(set(observed), {fixture.unit.id, fixture.layer_record.id})
        self.assertEqual(fixture.history_bytes(), history)

    def test_freeform_retains_current_basis_and_stops_on_withdrawal_without_source_rewrite(self):
        fixture = QualityJournalFixture(self, source=True)
        quality = fixture.admit_quality(decision='admit-with-limits', limits=['synthetic source slice only'])
        fixture.add_form()
        admitted = fixture.run(fixture.request(record=fixture.form, name='form-reviewed'))
        self.assertTrue(admitted['result']['current_admission']['can_use'])
        result = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(result['state'], 'ready')
        self.assertIn(fixture.basis.ref, result['dependencies'])
        self.assertIn('synthetic source slice only', result['admission']['limits'])
        source_before = fixture.form_path.read_bytes()
        fixture.admit_quality(name='form-quality-withdrawn', decision='withdraw',
            supersedes=quality['result']['current_admission']['assessment_refs'])
        unavailable = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertNotEqual(unavailable['state'], 'ready')
        self.assertIsNone(unavailable['display_text'])
        self.assertEqual(fixture.form_path.read_bytes(), source_before)

    def test_freeform_gets_owner_quality_context_without_frozen_authored_binding(self):
        fixture = QualityJournalFixture(self, source=True)
        fixture.admit_quality()
        fixture.add_form(bind_quality=False)
        fixture.run(fixture.request(record=fixture.form, name='form-review-without-context'))
        result = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(result['state'], 'ready')
        self.assertEqual(result['context'][-1]['slot'], 'owner:quality:0')
        self.assertEqual(result['context'][-1]['binding']['record'], fixture.basis.ref)
        self.assertNotIn('quality', fixture.form.payload['bindings'])

    def test_explicit_authored_quality_binding_stays_stale_after_owner_basis_renewal(self):
        fixture = QualityJournalFixture(self, source=True)
        quality = fixture.admit_quality()
        fixture.add_form()
        before = fixture.form_path.read_bytes()
        fixture.run(fixture.request(record=fixture.form, name='explicit-basis-form-reviewed'))
        fixture.admit_quality(name='explicit-basis-quality-withdrawn', decision='withdraw',
            supersedes=quality['result']['current_admission']['assessment_refs'])
        fixture.admit_quality(name='explicit-basis-quality-renewed')
        fixture.run(fixture.request(record=fixture.form, name='explicit-basis-form-reassessed'))
        current = fixture.describe(fixture.form)['result']['current_admission']
        self.assertTrue(current['can_use'])
        stale = fixture.run(fixture.request('materialize-form', record=fixture.form))['result']['materialization']
        self.assertEqual(stale['state'], 'stale')
        self.assertEqual(stale['issues'], ['binding.changed:quality'])
        self.assertIsNone(stale['display_text'])
        self.assertEqual(fixture.form_path.read_bytes(), before)

    def test_closed_quality_still_allows_negative_review_and_explicit_withdrawal(self):
        fixture = QualityJournalFixture(self)
        rejected = fixture.run(fixture.request(name='unqualified-source-rejected', decision='reject'))
        self.assertEqual(rejected['result']['current_admission']['status'], 'rejected')
        self.assertFalse(rejected['result']['current_admission']['can_use'])
        quality = fixture.admit_quality()
        positive = fixture.run(fixture.request(name='unit-positive',
            supersedes=rejected['result']['current_admission']['assessment_refs']))
        fixture.admit_quality(name='quality-gone', decision='withdraw',
            supersedes=quality['result']['current_admission']['assessment_refs'])
        withdrawn = fixture.run(fixture.request(name='unit-explicit-withdrawal', decision='withdraw',
            supersedes=positive['result']['current_admission']['assessment_refs']))
        self.assertFalse(withdrawn['result']['current_admission']['can_use'])
        self.assertEqual(withdrawn['result']['receipt']['events'][0]['assessment']['decision'], 'withdraw')
        fixture.config['native_text_layers'][0].update(payload_access=None,
            source_access={'read_scope': 'metadata_only', 'access_allowed': True,
                           'authority_ref': 'operator:synthetic-metadata-only'})
        fixture.save()
        self.assertNotIn('append', fixture.describe()['result']['command_context']['supported_operations'])
        with self.assertRaises(PermissionError):
            fixture.run(fixture.request(name='unread-quality-rejected', decision='reject'))

    def test_available_comparison_mismatch_can_be_rejected_but_never_admitted(self):
        fixture = QualityJournalFixture(self)
        fixture.fx.rebuild(text='Synthetic wrong extraction.')
        fixture.layer_record = Record.from_payload(fixture.fx.layer_id, 1, fixture.fx.layer,
            origin_id=fixture.fx.selections[0]['origin_id'])
        fixture.config.update(native_text_units=[], native_text_layers=fixture.fx.selections,
            subjects=fixture.fx.subjects, quality_dependencies={})
        fixture.save()
        negative = fixture.request(record=fixture.layer_record, name='mismatch-rejected', decision='reject')
        rejected = fixture.run(negative)
        self.assertEqual(rejected['result']['current_admission']['status'], 'rejected')
        self.assertFalse(rejected['result']['current_admission']['can_use'])
        with self.assertRaises(assessment_journal.AssessmentRejected):
            fixture.run(fixture.request(record=fixture.layer_record, name='mismatch-admitted'))
        fixture.config['native_text_layers'][0].update(payload_access=None,
            source_access={'read_scope': 'metadata_only', 'access_allowed': True,
                           'authority_ref': 'operator:synthetic-metadata-only'})
        fixture.save()
        with self.assertRaises(PermissionError):
            fixture.run(fixture.request(record=fixture.layer_record, name='metadata-only-rejection', decision='reject'))


class QualityDependencyTests(unittest.TestCase):
    def setUp(self):
        self.layer = Record.from_payload('tos.text-layer.synthetic.quality', 1,
            {'schema_version': 'tos_source_text_layer_v1'}, origin_id='one-synthetic-origin')
        self.comparison = Record.from_payload('tos.text-comparison.synthetic', 1,
            {'synthetic': True}, origin_id=self.layer.origin_id)
        self.binding = {'text_layer': {'layer_id': self.layer.id, 'record_sha256': 'a' * 64},
            'source_record_refs': {'item': 'ToS/source-witnesses/synthetic/item.json'}}
        self.layer_row = {'record': self.layer, 'comparison': self.comparison,
            'binding': self.binding, 'read_ready': True,
            'scope': {'content_file_id': 'tos.file.synthetic', 'content_sha256': 'b' * 64,
                'text_scope': {'position_unit': 'unicode_code_point', 'interval': 'half_open',
                               'start': 0, 'end': 5}}}
        self.layers = {self.layer.id: self.layer_row}
        self.unit = Record.from_payload('tos.text-unit.synthetic', 1,
            {'schema_version': 'tos_native_text_unit_assessment_subject_v1',
             'native_binding': self.binding})
        self.claim = Record.from_payload('tos.claim.synthetic', 1,
            {'assertion_layer': 'semantic_interpretation'})
        self.records = {record.id: record for record in (self.layer, self.unit, self.claim)}
        self.validator = Draft202012Validator(json.loads(
            (ROOT / 'ToS/contracts/native-text-layer-quality-basis.schema.json').read_text()))
        self.policy = Record.from_payload('tos.policy.synthetic', 3, {'synthetic': True})
        self.assessment = Record.from_payload('tos.review.synthetic', 1, {'synthetic': True})
        self.admission = {'policy': self.policy.ref, 'assessment_refs': [self.assessment.ref],
            'status': 'admitted-with-limits', 'can_use': True, 'limits': ['synthetic scope only']}

    def requirements(self, subject=None, sources=None, entries=None, assertion='semantic_interpretation'):
        subject = subject or self.claim
        if entries is None:
            entries = [{'layer_id': self.layer.id, 'use': 'text-layer:semantic-analysis'}]
        return _quality_requirements(subject, (self.unit,) if sources is None else sources,
            self.records, {subject.id: entries}, self.layers, assertion)

    def test_actual_source_closure_requires_every_exact_layer_and_one_purpose(self):
        self.assertEqual(len(self.requirements()), 1)
        for entries in ([], [{'layer_id': self.layer.id, 'use': 'text-layer:search-projection'}],
                [{'layer_id': 'tos.text-layer.unrelated', 'use': 'text-layer:semantic-analysis'}]):
            with self.subTest(entries=entries), self.assertRaises(PermissionError):
                self.requirements(entries=entries)
        for field, replacement in (('text_layer', {'layer_id': self.layer.id, 'record_sha256': 'c' * 64}),
                                   ('source_record_refs', {'item': 'another-source'})):
            altered = copy.deepcopy(self.binding)
            altered[field] = replacement
            source = Record.from_payload(self.unit.id, 1, {'native_binding': altered})
            with self.subTest(field=field), self.assertRaises(PermissionError):
                self.requirements(sources=(source,))

    def test_layer_target_has_no_self_dependency_and_units_require_citation_quality(self):
        self.assertEqual(self.requirements(self.layer, sources=(self.comparison,), entries=[]), [])
        with self.assertRaises(PermissionError):
            self.requirements(self.layer, sources=(self.comparison,))
        self.assertEqual(self.requirements(self.unit, sources=(), assertion='textual_observation',
            entries=[{'layer_id': self.layer.id, 'use': 'text-layer:citation'}])[0]['use'],
            'text-layer:citation')
        with self.assertRaises(PermissionError):
            self.requirements(self.unit, sources=(), assertion='textual_observation')

    def test_form_follows_parent_sources_and_defers_only_runtime_quality_refs(self):
        basis = _quality_basis(self.layer_row, self.admission, 'text-layer:semantic-analysis', self.validator)
        form = Record.from_payload('tos.form.synthetic', 1, {'schema_version': 'tos_human_form_v1',
            'subject': self.claim.ref, 'bindings': {'source': {'record': self.unit.ref, 'pointer': ''},
                'quality': {'record': basis.ref, 'pointer': ''}}})
        self.assertEqual(len(self.requirements(form, sources=(), assertion='human_projection')), 1)
        with self.assertRaises(PermissionError):
            self.requirements(form, sources=(), assertion='human_projection', entries=[])
        stale = Record.from_payload(self.unit.id, 2, self.unit.payload)
        self.records[self.unit.id] = stale
        with self.assertRaises(JournalConflict):
            self.requirements(form, sources=(), assertion='human_projection')

    def test_quality_basis_changes_with_relevant_evidence_not_unrelated_head(self):
        first = _quality_basis(self.layer_row, self.admission, 'text-layer:semantic-analysis', self.validator)
        unrelated = {**self.admission, 'journal_head': 'unrelated synthetic append'}
        same = _quality_basis(self.layer_row, unrelated, 'text-layer:semantic-analysis', self.validator)
        self.assertEqual(first.ref, same.ref)
        for field, value in (('assessment_refs', []), ('policy', Record.from_payload(self.policy.id, 4, {}).ref),
                             ('limits', ['narrower synthetic scope']), ('can_use', False)):
            changed = _quality_basis(self.layer_row, {**self.admission, field: value},
                'text-layer:semantic-analysis', self.validator)
            self.assertEqual(first.id, changed.id)
            self.assertNotEqual(first.ref, changed.ref)
        forbidden = _quality_basis({**self.layer_row, 'read_ready': False}, self.admission,
            'text-layer:semantic-analysis', self.validator)
        self.assertFalse(forbidden.payload['can_use'])
        self.assertEqual(first.origin_id, self.layer.origin_id)

    def test_dependency_mapping_is_bounded_and_cannot_repeat_or_select_unconfigured_target(self):
        good = [{'layer_id': self.layer.id, 'use': 'text-layer:citation'}]
        _validate_quality_dependencies({self.unit.id: good}, {self.unit.id: {}})
        for dependencies in ({self.unit.id: good * 2}, {self.unit.id: good * 9},
                {'unconfigured': good}, {self.unit.id: [{'layer_id': self.layer.id, 'use': 'research'}]}):
            with self.subTest(dependencies=dependencies), self.assertRaises(ValueError):
                _validate_quality_dependencies(dependencies, {self.unit.id: {}})


if __name__ == '__main__':
    unittest.main()

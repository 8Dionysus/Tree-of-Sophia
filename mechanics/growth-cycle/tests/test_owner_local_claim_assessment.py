"""Private Claim -> common assessment/form integration over synthetic sources.

Temporary source bytes, grants, competence and judgments are artificial. These
checks prove source-selection mechanics, never real research admission.
"""
from __future__ import annotations

import copy
from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(ROOT / 'scripts'), str(ROOT / 'mechanics/growth-cycle/tests'),
               str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]

from tests.test_source_owner_claim_profiles import OwnerLocalClaimFixture, CLAIM_REF, FORM_REF, SOURCE_REF
from assessment_journal import AssessmentRejected, JournalConflict
from knowledge_assessment import Record
from source_owner_context import OwnerLocalSourceContext
from native_text_binding import NativeTextBindingResolver
from source_witness_human_forms import claim_forms_path
import test_knowledge_assessment as policy_fixtures
import assessment_journal
from test_occurrence_growth import copy_contracts


COMMAND_VERSION = 'tos_local_assessment_command_v1'
FORM_ID = 'tos.form.synthetic.private-claim-hover'
FORM_MAKER = 'model:synthetic-claim-form-maker'
LANGUAGES = ['en', 'ru', 'und', 'x-test']
REFUSAL = (PermissionError, ValueError, OSError)


class PrivateClaimAssessmentFixture:
    def __init__(self, test, *, exact=True, form=False):
        temporary = tempfile.TemporaryDirectory(prefix='tos-private-claim-assessment-')
        test.addCleanup(temporary.cleanup)
        self.local = OwnerLocalClaimFixture(Path(temporary.name))
        copy_contracts(self.local.public)
        self.policy = policy_fixtures.AssessmentPolicyTests(methodName='runTest')
        self.policy.setUp()
        test.addCleanup(self.policy.doCleanups)
        self.owner, self.config, _ = self.policy.local_command_fixture()
        self.journal = self.local.private / '.assessment-journal'
        self.journal.mkdir(mode=0o700)
        self.config.update(schema_version='tos_local_assessment_owner_v4',
            source_context_ref=str(self.local.config_path), source_records=[],
            owner_local_source_records=[], native_text_units=[],
            owner_local_source_claims=[self.local.claim_selection(exact)],
            journal_directory=str(self.journal))
        self.config['subjects'] = {}
        self.claim = Record.from_payload(self.local.claim['claim_id'], self.local.claim['claim_version'],
                                        self.local.claim, origin_id='origin:synthetic-claim')
        self.identifier = self.claim.id
        self.config['subjects'][self.claim.id] = self.scope(self.claim, 'linguistic_analysis',
                                                         self.local.claim['maker']['agent_ref'])
        self.form, self.form_path = None, None
        if form:
            body = {'schema_version': 'tos_human_form_v1', 'form_id': FORM_ID, 'form_version': 1,
                'subject': self.claim.ref, 'role': 'hover', 'language': 'ru', 'script': 'Cyrl',
                'creator_id': FORM_MAKER, 'revises': None,
                'bindings': {'context': {'record': self.claim.ref, 'pointer': ''}},
                'content': {'kind': 'freeform', 'text': 'Синтетический спорный разбор, не реальный вывод.'}}
            self.form = Record.from_payload(FORM_ID, 1, body, origin_id='origin:synthetic-claim')
            self.form_path = self.local.write_private(claim_forms_path(Path(CLAIM_REF), self.claim.id).as_posix(),
                self.local.encode({'schema_version': 'tos_human_form_set_v1', 'subject': self.claim.ref,
                                   'forms': [body], 'prior_forms': []}))
            self.config['owner_local_source_claims'][0]['form_ids'] = [FORM_ID]
            self.config['subjects'][FORM_ID] = self.scope(self.form, 'human_projection', FORM_MAKER)
            self.identifier = FORM_ID
        for index, competency in enumerate(self.config['competencies']):
            competency['payload'].update(assertion_layers=['linguistic_analysis', 'human_projection'],
                                         languages=LANGUAGES)
            self.config['authorities'][index]['payload'].update(
                assertion_layers=['linguistic_analysis', 'human_projection'], languages=LANGUAGES,
                subject_prefixes=['tos.claim.', 'tos.form.'],
                competence_refs=[Record.from_payload(**competency).ref])
        self.save()

    @staticmethod
    def scope(record, layer, maker):
        return {'record': record.ref, 'assertion_layer': layer, 'risk': 'low', 'languages': LANGUAGES[:],
                'maker_id': maker, 'requested_use': 'research', 'access_allowed': True}

    def save(self):
        self.owner.write_bytes(self.local.encode(self.config))
        self.owner.chmod(0o600)

    def record(self, identifier=None):
        return self.claim if (identifier or self.identifier) == self.claim.id else self.form

    def run(self, request):
        return self.policy.run_local(self.owner, request)

    def describe(self, identifier=None):
        return self.run({'schema_version': COMMAND_VERSION, 'operation': 'describe',
                         'subject_id': identifier or self.identifier})

    def request(self, operation='append', *, identifier=None, command_id='synthetic-private-claim-one'):
        identifier = identifier or self.identifier
        described = self.describe(identifier)
        result = {'schema_version': COMMAND_VERSION, 'operation': operation, 'subject_id': identifier,
                  'expected_subject': self.record(identifier).ref,
                  'expected_snapshot': described['owner_snapshot']}
        if operation == 'append':
            required = described['result']['command_context']['required_sources']
            review = copy.deepcopy(self.policy.review(profile='interpretation').assessment)
            review.update(subject=self.record(identifier).ref,
                authority=Record.from_payload(**self.config['authorities'][0]).ref,
                competence=Record.from_payload(**self.config['competencies'][0]).ref,
                evidence=[{'record': ref,
                           'stance': 'supports' if ref['id'] == self.local.binding['text_layer']['layer_id'] else 'context',
                           'locator': 'Synthetic exact owner-selected dependency; no private locator.'}
                          for ref in required])
            result.update(command_id=command_id, expected_revision=None, assessments=[review])
        return result

    def history(self):
        return {path.relative_to(self.journal): path.read_bytes()
                for path in self.journal.rglob('*') if path.is_file()}


class OwnerLocalClaimAssessmentTests(unittest.TestCase):
    def fixture(self, **kwargs):
        return PrivateClaimAssessmentFixture(self, **kwargs)

    def assert_private(self, fixture, response):
        self.assertEqual(response['visibility'], 'local_only')
        self.assertIs(response['publication_authorized'], False)
        serialized = json.dumps(response, ensure_ascii=False)
        for value in (CLAIM_REF, SOURCE_REF, fixture.local.binding['packet_ref'],
                      fixture.local.native.content_ref, fixture.local.native.text[3:8],
                      fixture.local.claim['qualifiers']['statement'],
                      fixture.local.binding['packet_sha256']):
            self.assertNotIn(value, serialized)

    def test_exact_claim_describe_append_replay_preserves_sources_and_grounding(self):
        f = self.fixture()
        original = {path: path.read_bytes() for path in
                    (f.local.private / CLAIM_REF, f.local.private / SOURCE_REF, f.local.public / FORM_REF)}
        described = f.describe()
        self.assert_private(f, described)
        context = described['result']['command_context']
        self.assertEqual({row['id'] for row in context['required_sources']},
            {f.local.source['record_id'], f.local.form['record_id'], f.local.binding['unit_id'],
             f.local.binding['text_layer']['layer_id']})
        self.assertTrue(context['source_read']['ready'])
        self.assertFalse(described['result']['current_admission']['can_use'])
        request = f.request()
        committed = f.run(request)
        self.assert_private(f, committed)
        self.assertTrue(committed['result']['current_admission']['can_use'])
        event = committed['result']['receipt']['events'][0]['assessment']
        self.assertEqual(event['reviewer']['kind'], 'agent')
        self.assertEqual(event['evidence'], request['assessments'][0]['evidence'])
        replay = f.run(request)['result']
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], committed['result']['receipt'])
        self.assertEqual({path: path.read_bytes() for path in original}, original)
        self.assertEqual(f.local.native.packet['reviews'], [])

    def test_omitting_loaded_endpoint_from_review_cannot_qualify(self):
        f = self.fixture()
        request = f.request()
        request['assessments'][0]['evidence'] = [row for row in request['assessments'][0]['evidence']
            if row['record']['id'] != f.local.form['record_id']]
        with self.assertRaises(AssessmentRejected):
            f.run(request)
        self.assertEqual(list(f.journal.rglob('head')), [])

    def test_endpoint_correction_invalidates_old_review_without_rewriting_claim_or_history(self):
        f = self.fixture()
        request = f.request()
        committed = f.run(request)['result']
        original_claim, history = (f.local.private / CLAIM_REF).read_bytes(), f.history()
        f.local.form['record_version'] += 1
        f.local.form['semantic_content']['form_account'] += ' Corrected synthetic description.'
        f.local.native.write_json(FORM_REF, f.local.form)
        inspected = f.run(f.request('inspect'))['result']
        self.assertFalse(inspected['current_admission']['can_use'])
        request['expected_snapshot'] = f.describe()['owner_snapshot']
        replay = f.run(request)['result']
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], committed['receipt'])
        self.assertTrue(replay['receipt']['admission_at_commit']['can_use'])
        self.assertFalse(replay['current_admission']['can_use'])
        self.assertEqual((f.local.private / CLAIM_REF).read_bytes(), original_claim)
        self.assertEqual(f.history(), history)

    def test_metadata_only_claim_never_reads_text_and_cannot_append(self):
        f = self.fixture(exact=False)
        content = f.local.public / f.local.native.content_ref
        content.unlink()
        original = OwnerLocalSourceContext.read_bytes
        def no_text(context, path, *args, **kwargs):
            self.assertNotEqual(path, content)
            return original(context, path, *args, **kwargs)
        with patch.object(OwnerLocalSourceContext, 'read_bytes', no_text):
            described = f.describe()
            self.assert_private(f, described)
            self.assertFalse(described['result']['command_context']['source_read']['ready'])
            self.assertEqual(described['result']['command_context']['supported_operations'], ['describe', 'inspect'])
            inspected = f.run(f.request('inspect'))
            self.assertFalse(inspected['result']['current_admission']['can_use'])
            with self.assertRaises(REFUSAL):
                f.run(f.request())
        self.assertEqual(list(f.journal.rglob('head')), [])

    def test_claim_freeform_requires_grounding_and_loses_current_display_after_endpoint_change(self):
        f = self.fixture(form=True)
        unreviewed = f.run(f.request('materialize-form'))['result']['materialization']
        self.assertEqual(unreviewed['state'], 'needs-assessment')
        required = f.describe()['result']['command_context']['required_sources']
        self.assertIn(f.claim.ref, required)
        omitted = f.request()
        omitted['assessments'][0]['evidence'] = [row for row in omitted['assessments'][0]['evidence']
            if row['record']['id'] == f.local.binding['text_layer']['layer_id']]
        with self.assertRaises(AssessmentRejected):
            f.run(omitted)
        f.run(f.request())
        materialized = f.run(f.request('materialize-form'))
        self.assertEqual(materialized['visibility'], 'local_only')
        ready = materialized['result']['materialization']
        self.assertEqual(ready['state'], 'ready')
        self.assertEqual(ready['display_text'], f.form.payload['content']['text'])
        self.assertTrue(all(ref in ready['dependencies'] for ref in required))
        before, history = f.form_path.read_bytes(), f.history()
        f.local.form['semantic_content']['form_account'] += ' Same-version byte correction is still a change.'
        f.local.native.write_json(FORM_REF, f.local.form)
        stale = f.run(f.request('materialize-form'))['result']['materialization']
        self.assertEqual(stale['state'], 'needs-assessment')
        self.assertIsNone(stale['display_text'])
        self.assertEqual(f.form_path.read_bytes(), before)
        self.assertEqual(f.history(), history)

    def test_languages_include_claim_endpoint_and_native_but_ignore_tag_case(self):
        f = self.fixture()
        original = list(f.config['subjects'][f.identifier]['languages'])
        for omitted in ('ru', 'en', 'und', 'x-test'):
            with self.subTest(language=omitted):
                f.config['subjects'][f.identifier]['languages'] = [value for value in original if value != omitted]
                f.save()
                with self.assertRaises(PermissionError):
                    f.describe()
        f.config['subjects'][f.identifier]['languages'] = original
        f.local.form['form_identity']['language'] = 'X-TEST'
        f.local.native.write_json(FORM_REF, f.local.form)
        f.save()
        self.assertTrue(f.describe()['result']['command_context']['source_read']['ready'])

    def test_claim_supporting_native_records_cannot_become_targets_by_an_invented_scope(self):
        f = self.fixture()
        reader = f.local.reader(exact=True)
        f.local.load(reader)
        for row in reader.records:
            if row['id'] not in {f.local.binding['unit_id'], f.local.binding['text_layer']['layer_id']}:
                continue
            with self.subTest(identity=row['id']):
                target = Record.from_payload(**row)
                f.config['subjects'][target.id] = f.scope(target, 'linguistic_analysis', 'invented-maker')
                f.save()
                with self.assertRaises(PermissionError):
                    f.describe(target.id)
        self.assertEqual(list(f.journal.rglob('head')), [])

    def test_later_claim_exact_access_mismatch_refuses_before_any_private_source_read(self):
        f = self.fixture()
        second = copy.deepcopy(f.local.claim)
        second['claim_id'] += '.second'
        f.local.write_claims(f.local.claim, second)
        baseline = copy.deepcopy(f.config['owner_local_source_claims'][0])
        later = copy.deepcopy(baseline)
        later['claim_id'] = second['claim_id']
        original = OwnerLocalSourceContext.read_bytes
        def no_private(context, path, *args, **kwargs):
            self.assertFalse(path.is_relative_to(f.local.private), 'private source read before all grants passed')
            self.assertNotEqual(path, f.local.public / f.local.native.content_ref)
            return original(context, path, *args, **kwargs)
        for case in ('claim', 'bound-source', 'additional-native', 'non-native-exact'):
            selection = copy.deepcopy(later)
            if case == 'claim':
                selection['source_access'] = f.local.access()
            elif case == 'bound-source':
                selection['source_records'][0]['source_access'] = f.local.access()
            elif case == 'additional-native':
                selection['native_bindings'] = [{'binding': copy.deepcopy(f.local.binding),
                    'origin_id': 'origin:synthetic-native', 'source_access': f.local.access()}]
            else:
                selection['source_records'][1]['source_access'] = f.local.access(True)
            f.config['owner_local_source_claims'] = [baseline, selection]
            f.save()
            with self.subTest(case=case), patch.object(OwnerLocalSourceContext, 'read_bytes', no_private):
                with self.assertRaises(REFUSAL):
                    f.describe()
        self.assertEqual(list(f.journal.rglob('head')), [])

    def test_public_endpoint_can_be_shared_only_with_identical_body_and_origin(self):
        f = self.fixture()
        f.config['source_records'] = [{'path': FORM_REF, 'record_id': f.local.form['record_id'],
                                       'origin_id': 'origin:synthetic-lexical-form'}]
        f.save()
        self.assertTrue(f.run(f.request())['result']['current_admission']['can_use'])
        f.config['source_records'][0]['origin_id'] = 'invented-independent-origin'
        f.save()
        with self.assertRaises(ValueError):
            f.describe()

    def test_endpoint_drift_between_describe_and_append_or_during_lock_never_publishes(self):
        f = self.fixture()
        request = f.request()
        path = f.local.public / FORM_REF
        original_bytes = path.read_bytes()
        f.local.form['semantic_content']['form_account'] += ' drift'
        path.write_bytes(f.local.encode(f.local.form))
        with self.assertRaises(JournalConflict):
            f.run(request)
        path.write_bytes(original_bytes)
        original_lock = assessment_journal.AssessmentJournal._locked
        @contextmanager
        def changed_source(journal, identifier):
            with original_lock(journal, identifier):
                path.write_bytes(f.local.encode(f.local.form))
                yield
        with patch.object(assessment_journal.AssessmentJournal, '_locked', changed_source):
            with self.assertRaises(REFUSAL):
                f.run(request)
        self.assertEqual(list(f.journal.rglob('head')), [])

    def test_unrelated_metadata_only_native_selection_does_not_invalidate_claim_scope(self):
        f = self.fixture()
        f.run(f.request())
        before = f.run(f.request('inspect'))['result']['current_admission']
        layer = copy.deepcopy(f.local.native.layer)
        layer['layer_id'] += '.de'
        layer['representation']['language'] = 'de'
        layer_ref = f.local.native.layer_ref.replace('.v1.json', '.de.v1.json')
        layer_raw = f.local.encode(layer)
        f.local.native.write_bytes(layer_ref, layer_raw)
        packet = copy.deepcopy(f.local.native.packet)
        def new_id(value):
            if '.sid-' in value:
                return value.split('.sid-')[0] + '.sid-' + hashlib.sha256((value + ':de').encode()).hexdigest()[:32]
            return value + '.de'
        mapping = {packet['packet_id']: new_id(packet['packet_id'])}
        for collection, key in (('schemes', 'scheme_id'), ('units', 'unit_id'),
                                ('segmentations', 'segmentation_id'), ('anchors', 'anchor_ref')):
            mapping.update({row[key]: new_id(row[key]) for row in packet[collection]})
        def rewrite(value):
            if isinstance(value, str):
                return mapping.get(value, value)
            if isinstance(value, list):
                return [rewrite(item) for item in value]
            if isinstance(value, dict):
                return {key: rewrite(item) for key, item in value.items()}
            return value
        packet = rewrite(packet)
        packet['source_layer'].update(text_layer_ref=layer_ref, language='de')
        for anchor in packet['anchors']:
            anchor['text_layer_ref'] = layer_ref
        packet_ref = f.local.binding['packet_ref'].replace('.v1.json', '.de.v1.json')
        packet_raw = f.local.encode(packet)
        f.local.write_private(packet_ref, packet_raw)
        binding = rewrite(copy.deepcopy(f.local.binding))
        binding.update(packet_ref=packet_ref, packet_sha256=hashlib.sha256(packet_raw).hexdigest())
        binding['text_layer'].update(record_ref=layer_ref, layer_id=layer['layer_id'],
                                    record_sha256=hashlib.sha256(layer_raw).hexdigest())
        adapted = NativeTextBindingResolver(f.local.public, owner_context=f.local.context).assessment_records(
            binding, origin_id='origin:synthetic-unrelated-native', verify_content=False)
        unit = Record.from_payload(**adapted['records'][0])
        scope = f.scope(unit, 'textual_observation', packet['segmentations'][0]['maker']['agent_ref'])
        scope['languages'] = ['de']
        f.config['subjects'][unit.id] = scope
        f.config['native_text_units'] = [{'binding': binding, 'origin_id': 'origin:synthetic-unrelated-native',
                                          'source_access': f.local.access()}]
        f.save()
        described = f.describe()
        self.assertTrue(described['result']['command_context']['source_read']['ready'])
        self.assertIn('append', described['result']['command_context']['supported_operations'])
        self.assertNotIn(unit.id, {row['id'] for row in described['result']['command_context']['required_sources']})
        after = f.run(f.request('inspect'))['result']['current_admission']
        self.assertEqual(after, before)


if __name__ == '__main__':
    unittest.main()

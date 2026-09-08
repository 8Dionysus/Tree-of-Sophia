"""Synthetic v4 assessment, never real private source or competence evidence.

The existing fixtures supply all text, owner grants and source metadata in
temporary directories. Tests exercise the common assessment grammar and
journal without creating a durable private store or admitting real knowledge.
"""
from __future__ import annotations

import copy
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import unittest
from unittest.mock import patch

from jsonschema import ValidationError


ROOT = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(ROOT / 'scripts'), str(ROOT / 'tests'),
               str(ROOT / 'mechanics/growth-cycle/tests'),
               str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]

import assessment_journal
import source_commands
import tests.test_source_owner_record_profiles as owner_fixtures
from assessment_journal import AssessmentJournal, JournalConflict
from knowledge_assessment import AssessmentEngine, Record
from native_text_binding import NativeTextBindingResolver
from source_owner_context import OwnerLocalSourceContext
from source_witness_human_forms import AssessedFormSnapshot
from test_native_text_assessment import NativeAssessmentFixture, ORIGIN
from test_occurrence_growth import copy_contracts


CONFIG_VERSION = 'tos_local_assessment_owner_v4'
COMMAND_VERSION = 'tos_local_assessment_command_v1'
FORM_ID = 'tos.form.synthetic.owner-local-reading'
OTHER_FORM_ID = 'tos.form.synthetic.owner-local-unselected'
DESCRIPTION_MAKER = 'agent:synthetic-description-author'
FORM_MAKER = 'agent:synthetic-form-author'
REFUSAL = (PermissionError, ValueError, OSError)


def encode(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode('utf-8')


class OwnerLocalAssessmentFixture:
    """Compose existing native/policy and confidential source fixture owners."""

    def __init__(self, test, *, read_scope='exact_owner_local', source=False,
                 form=None, lexical=False):
        self.assessment = NativeAssessmentFixture(test)
        self.local = owner_fixtures.OwnerLocalSourceRecordProfilesTests(methodName='runTest')
        self.local.setUp()
        test.addCleanup(self.local.doCleanups)
        self.public, self.private = self.local.public, self.local.private
        self.native = self.local.native
        copy_contracts(self.public)
        # The private authored packet and private exact text share the same
        # transport; original Item payload bytes are deliberately absent.
        self.content_ref = owner_fixtures.PREFIX + 'native/synthetic/local-content/exact.txt'
        self.content_path = self.local.write_private(self.content_ref, self.native.content)
        (self.public / self.native.content_ref).unlink()
        self.native.layer['representation']['content_ref'] = self.content_ref
        for anchor in self.native.packet['anchors']:
            anchor['source_return']['locator_ref'] = self.content_ref
        self.native.refresh()
        self.binding = copy.deepcopy(self.native.binding)
        self.binding['packet_ref'] = owner_fixtures.PACKET_REF
        self.local.write_private(owner_fixtures.PACKET_REF,
                                 (self.public / self.native.packet_ref).read_bytes())
        (self.public / self.native.packet_ref).unlink()
        self.packet_path = self.private / owner_fixtures.PACKET_REF
        self.context_path = self.local.config_path
        self.owner = self.local.base / 'assessment-owner.json'
        self.journal = self.private / '.assessment-journal'
        self.journal.mkdir(mode=0o700)
        self.config = copy.deepcopy(self.assessment.config)
        self.config.pop('source_root')
        self.config.update(schema_version=CONFIG_VERSION,
            source_context_ref=str(self.context_path), source_records=[],
            owner_local_source_records=[], journal_directory=str(self.journal),
            native_text_units=[{'binding': copy.deepcopy(self.binding), 'origin_id': ORIGIN,
                                'source_access': self.local.access(read_scope)}])
        self.config['subjects'] = {}
        self.source_ref = (owner_fixtures.SOURCE_REF.replace('occurrence.json', 'lexeme.json')
                           if lexical else owner_fixtures.SOURCE_REF)
        self.source_body = (owner_fixtures.lexeme() if lexical else owner_fixtures.occurrence(self.binding))
        self.source = Record.from_payload(self.source_body['record_id'], 1, self.source_body)
        self.form_ref = str(Path(self.source_ref).with_name(Path(self.source_ref).stem + '.human-forms.json'))
        self.form_set, self.form = None, None
        self.identifier = self.binding['unit_id']
        self.rebind_native(read_scope)
        if source or form is not None or lexical:
            self.local.write_private(self.source_ref, encode(self.source_body))
            self.config['owner_local_source_records'] = [{
                'path': self.source_ref, 'record_id': self.source.id,
                'profile_type_id': 'tos.entity.lexeme' if lexical else 'tos.entity.occurrence',
                'origin_id': ORIGIN, 'source_access': self.local.access(),
                'source_binding': None if lexical else copy.deepcopy(self.binding), 'form_ids': []}]
            self.config['subjects'][self.source.id] = self.scope(
                self.source, 'semantic_interpretation', DESCRIPTION_MAKER, ['en', 'und'])
            self.identifier = self.source.id
        if form is not None:
            self.form_set = {'schema_version': 'tos_human_form_set_v1', 'subject': self.source.ref,
                             'forms': [], 'prior_forms': []}
            if form == 'source-copy':
                body = source_commands.prepare_metadata_change(self.source_body, None, FORM_MAKER,
                    FORM_ID, 'metadata.preferred-name')['form']
            else:
                body = self.freeform(FORM_ID)
            self.form_set['forms'] = [body, self.freeform(OTHER_FORM_ID)]
            self.form = Record.from_payload(body['form_id'], body['form_version'], body)
            self.config['owner_local_source_records'][0]['form_ids'] = [self.form.id]
            self.config['subjects'][self.form.id] = self.scope(
                self.form, 'human_projection', FORM_MAKER, ['ru', 'en', 'und'])
            self.local.write_private(self.form_ref, encode(self.form_set))
            self.identifier = self.form.id
        self._grants()
        self.save()

    @staticmethod
    def scope(record, layer, maker, languages):
        return {'record': record.ref, 'assertion_layer': layer, 'risk': 'low',
                'languages': languages, 'maker_id': maker, 'requested_use': 'research',
                'access_allowed': True}

    def freeform(self, identity):
        return {'schema_version': 'tos_human_form_v1', 'form_id': identity, 'form_version': 1,
            'subject': self.source.ref, 'role': 'hover', 'language': 'ru', 'script': 'Cyrl',
            'creator_id': FORM_MAKER, 'revises': None,
            'bindings': {'context': {'record': self.source.ref, 'pointer': ''}},
            'content': {'kind': 'freeform', 'text': 'Синтетическое частное описание, не реальное суждение.'}}

    def _grants(self):
        for index, competency in enumerate(self.config['competencies']):
            competency['payload'].update(
                assertion_layers=['textual_observation', 'linguistic_analysis',
                                  'semantic_interpretation', 'human_projection'],
                languages=['ru', 'en', 'und'])
            self.config['authorities'][index]['payload'].update(
                assertion_layers=list(competency['payload']['assertion_layers']),
                languages=['ru', 'en', 'und'],
                subject_prefixes=['tos.text-unit.', 'tos.occurrence.', 'tos.lexeme.', 'tos.form.'],
                competence_refs=[Record.from_payload(**competency).ref])

    def save(self):
        self.owner.write_bytes(encode(self.config))
        self.owner.chmod(0o600)

    def rebind_native(self, read_scope='exact_owner_local'):
        selection = self.config['native_text_units'][0]
        selection['source_access'] = self.local.access(read_scope)
        context = OwnerLocalSourceContext.load(self.context_path)
        adapted = NativeTextBindingResolver(self.public, owner_context=context).assessment_records(
            selection['binding'], origin_id=ORIGIN, verify_content=read_scope != 'metadata_only',
            allow_private_content=read_scope == 'exact_owner_local')
        self.unit, self.layer = [Record.from_payload(**row) for row in adapted['records']]
        self.config['subjects'][self.unit.id] = self.scope(self.unit, 'textual_observation',
            self.native.packet['segmentations'][0]['maker']['agent_ref'], ['und'])
        self.save()

    def record(self, identifier=None):
        identifier = identifier or self.identifier
        if identifier == self.unit.id:
            return self.unit
        if identifier == self.source.id:
            return self.source
        if self.form is not None and identifier == self.form.id:
            return self.form
        raise ValueError('fixture has no selected record')

    def run(self, request):
        return self.assessment.policy.run_local(self.owner, request)

    def describe(self, identifier=None):
        return self.run({'schema_version': COMMAND_VERSION, 'operation': 'describe',
                         'subject_id': identifier or self.identifier})

    def request(self, operation='append', *, identifier=None, command_id='synthetic-v4-one'):
        identifier = identifier or self.identifier
        described = self.describe(identifier)
        target = self.record(identifier)
        result = {'schema_version': COMMAND_VERSION, 'operation': operation,
            'subject_id': identifier, 'expected_subject': target.ref,
            'expected_snapshot': described['owner_snapshot']}
        if operation == 'append':
            review = copy.deepcopy(self.assessment.policy.review().assessment)
            review.update(subject=target.ref,
                profile_id='source-observation' if identifier == self.unit.id else 'interpretation',
                authority=Record.from_payload(**self.config['authorities'][0]).ref,
                competence=Record.from_payload(**self.config['competencies'][0]).ref,
                evidence=[{'record': self.layer.ref, 'stance': 'supports',
                           'locator': 'Synthetic exact layer; no quoted text or private locator.'}])
            result.update(command_id=command_id, expected_revision=None, assessments=[review])
        return result

    def heads(self):
        return list(self.journal.rglob('head'))

    def history_bytes(self):
        return {path.relative_to(self.journal): path.read_bytes()
                for path in self.journal.rglob('*') if path.is_file()}

    def revoke(self):
        grant = self.config['authorities'][0]
        grant['version'] += 1
        grant['payload']['authority_version'] += 1
        grant['payload']['state'] = 'revoked'
        self.save()


class OwnerLocalAssessmentTests(unittest.TestCase):
    def fixture(self, **kwargs):
        return OwnerLocalAssessmentFixture(self, **kwargs)

    def assert_local(self, response):
        self.assertEqual(response['visibility'], 'local_only')
        self.assertIs(response['publication_authorized'], False)

    def assert_no_native_disclosure(self, fixture, response):
        rendered = json.dumps(response, ensure_ascii=False)
        for forbidden in (fixture.native.text[3:8], fixture.content_ref,
                          fixture.binding['packet_ref'], fixture.binding['packet_sha256'],
                          fixture.binding['text_layer']['record_sha256'],
                          'ordered_anchor_refs', 'exact_sha256', 'source_record_refs'):
            self.assertNotIn(forbidden, rendered)

    def test_native_only_append_replay_and_current_revocation_keep_native_bytes(self):
        fixture = self.fixture()
        before = {path: path.read_bytes() for path in
                  (fixture.packet_path, fixture.content_path, fixture.public / fixture.native.layer_ref)}
        described = fixture.describe()
        self.assert_local(described)
        self.assert_no_native_disclosure(fixture, described)
        request = fixture.request()
        committed = fixture.run(request)
        self.assert_local(committed)
        self.assert_no_native_disclosure(fixture, committed)
        result = committed['result']
        self.assertTrue(result['current_admission']['can_use'])
        self.assertEqual(result['receipt']['events'][0]['assessment']['reviewer']['kind'], 'agent')
        replay = fixture.run(request)['result']
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], result['receipt'])
        fixture.revoke()
        request['expected_snapshot'] = fixture.describe()['owner_snapshot']
        replay = fixture.run(request)['result']
        self.assertTrue(replay['replayed'])
        self.assertTrue(replay['receipt']['admission_at_commit']['can_use'])
        self.assertFalse(replay['current_admission']['can_use'])
        self.assertEqual({path: path.read_bytes() for path in before}, before)
        self.assertFalse((fixture.public / fixture.native.original_ref).exists())
        self.assertEqual(fixture.native.packet['reviews'], [])

    def test_independent_native_unit_language_scopes_survive_joint_selection(self):
        fixture = self.fixture()
        # A second synthetic layer declares another language over the same
        # test bytes. This tests scope isolation, not real linguistic truth.
        layer = copy.deepcopy(fixture.native.layer)
        layer['layer_id'] += '.de'
        layer['representation']['language'] = 'de'
        layer_ref = fixture.native.layer_ref.replace('.v1.json', '.de.v1.json')
        layer_raw = encode(layer)
        fixture.native.write_bytes(layer_ref, layer_raw)
        packet = json.loads(fixture.packet_path.read_bytes())

        def independent_id(value):
            if '.sid-' in value:
                return value.split('.sid-')[0] + '.sid-' + hashlib.sha256(
                    (value + ':de').encode('utf-8')).hexdigest()[:32]
            return value + '.de'

        mapping = {packet['packet_id']: independent_id(packet['packet_id'])}
        for collection, key in (('schemes', 'scheme_id'), ('units', 'unit_id'),
                                ('segmentations', 'segmentation_id'), ('anchors', 'anchor_ref')):
            for row in packet[collection]:
                mapping[row[key]] = independent_id(row[key])

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
        packet_ref = owner_fixtures.PREFIX + 'native/synthetic/source-text-unit.de.v1.json'
        packet_raw = encode(packet)
        fixture.local.write_private(packet_ref, packet_raw)
        binding = rewrite(copy.deepcopy(fixture.binding))
        binding.update(packet_ref=packet_ref, packet_sha256=hashlib.sha256(packet_raw).hexdigest())
        binding['text_layer'].update(record_ref=layer_ref, layer_id=layer['layer_id'],
                                    record_sha256=hashlib.sha256(layer_raw).hexdigest())
        adapted = NativeTextBindingResolver(fixture.public,
            owner_context=OwnerLocalSourceContext.load(fixture.context_path)).assessment_records(
                binding, origin_id=ORIGIN, verify_content=True, allow_private_content=True)
        unit = Record.from_payload(**adapted['records'][0])
        fixture.config['subjects'][unit.id] = fixture.scope(unit, 'textual_observation',
            packet['segmentations'][0]['maker']['agent_ref'], ['de'])
        first = copy.deepcopy(fixture.config['native_text_units'][0])
        second = {'binding': binding, 'origin_id': ORIGIN,
                  'source_access': fixture.local.access('exact_owner_local')}
        for selections, identities in (([first], [fixture.unit.id]), ([second], [unit.id]),
                                       ([first, second], [fixture.unit.id, unit.id])):
            fixture.config['native_text_units'] = selections
            fixture.save()
            for identifier in identities:
                with self.subTest(units=len(selections), subject=identifier):
                    result = fixture.describe(identifier)
                    self.assert_local(result)
                    self.assertEqual(result['result']['command_context']['subject']['id'], identifier)
        self.assertEqual(fixture.config['subjects'][fixture.unit.id]['languages'], ['und'])
        self.assertEqual(fixture.config['subjects'][unit.id]['languages'], ['de'])
        self.assertEqual(fixture.heads(), [])

    def test_occurrence_and_source_copy_form_are_distinct_assessment_subjects(self):
        fixture = self.fixture(form='source-copy')
        paths = (fixture.private / fixture.source_ref, fixture.private / fixture.form_ref)
        before = {path: path.read_bytes() for path in paths}
        self.assertNotEqual(DESCRIPTION_MAKER, fixture.config['subjects'][fixture.unit.id]['maker_id'])
        for identifier in (fixture.source.id, fixture.form.id):
            with self.subTest(subject=identifier):
                described = fixture.describe(identifier)
                self.assert_local(described)
                self.assertEqual(described['result']['command_context']['subject'], fixture.record(identifier).ref)
                self.assertFalse(described['result']['current_admission']['can_use'])
                request = fixture.request(identifier=identifier, command_id='synthetic-' + identifier)
                result = fixture.run(request)['result']
                self.assertTrue(result['current_admission']['can_use'])
                self.assertEqual(result['current_admission']['subject'], fixture.record(identifier).ref)
                self.assertTrue(fixture.run(request)['result']['replayed'])
        self.assertEqual({path: path.read_bytes() for path in paths}, before)
        self.assertEqual(len(fixture.heads()), 2)

    def test_freeform_materialization_requires_current_assessment_and_keeps_local_ceiling(self):
        fixture = self.fixture(form='freeform')
        unreviewed = fixture.run(fixture.request('materialize-form'))
        self.assert_local(unreviewed)
        self.assertEqual(unreviewed['result']['materialization']['state'], 'needs-assessment')
        fixture.run(fixture.request())
        materialized = fixture.run(fixture.request('materialize-form'))
        self.assert_local(materialized)
        self.assertEqual(materialized['result']['materialization']['state'], 'ready')
        self.assertEqual(materialized['result']['materialization']['display_text'], fixture.form.payload['content']['text'])
        before = fixture.history_bytes()
        fixture.revoke()
        unavailable = fixture.run(fixture.request('materialize-form'))['result']['materialization']
        self.assertEqual(unavailable['state'], 'needs-assessment')
        self.assertIsNone(unavailable['display_text'])
        self.assertEqual(fixture.history_bytes(), before)

    def test_metadata_only_never_opens_text_and_cannot_append(self):
        fixture = self.fixture(source=True, read_scope='metadata_only')
        fixture.content_path.unlink()
        original = OwnerLocalSourceContext.read_bytes

        def without_content(context, path, *args, **kwargs):
            self.assertNotEqual(path, fixture.content_path)
            return original(context, path, *args, **kwargs)

        with patch.object(OwnerLocalSourceContext, 'read_bytes', without_content):
            for identifier in (fixture.unit.id, fixture.source.id):
                described = fixture.describe(identifier)
                self.assertFalse(described['result']['current_admission']['can_use'])
                self.assertEqual(described['result']['command_context']['supported_operations'], ['describe', 'inspect'])
                self.assertFalse(fixture.run(fixture.request('inspect', identifier=identifier))['result']['current_admission']['can_use'])
                with self.assertRaises(REFUSAL):
                    fixture.run(fixture.request(identifier=identifier))
        self.assertEqual(fixture.heads(), [])

    def test_metadata_only_native_evidence_cannot_revive_unrelated_description_admission(self):
        fixture = self.fixture(lexical=True)
        committed = fixture.run(fixture.request())['result']
        self.assertTrue(committed['current_admission']['can_use'])
        before = fixture.history_bytes()
        source_ref, layer_ref = fixture.source.ref, fixture.layer.ref
        fixture.rebind_native('metadata_only')
        fixture.content_path.unlink()
        self.assertEqual(fixture.source.ref, source_ref)
        self.assertEqual(fixture.layer.ref, layer_ref)
        inspected = fixture.run(fixture.request('inspect'))['result']
        self.assertFalse(inspected['current_admission']['can_use'])
        self.assertEqual(inspected['revision'], committed['revision'])
        reasons = {reason for row in inspected['current_admission']['invalid_assessments'] for reason in row['reasons']}
        self.assertIn('subject.exact-source-unverified', reasons)
        self.assertEqual(fixture.history_bytes(), before)

    def test_private_forms_select_current_exact_ids_not_neighbors_or_predecessors(self):
        fixture = self.fixture(form='freeform')
        other = Record.from_payload(OTHER_FORM_ID, 1, fixture.form_set['forms'][1])
        fixture.config['subjects'][OTHER_FORM_ID] = fixture.scope(other, 'human_projection', FORM_MAKER, ['ru', 'en', 'und'])
        fixture.save()
        with self.assertRaises(REFUSAL):
            fixture.describe(OTHER_FORM_ID)
        fixture.form_set['prior_forms'] = [fixture.form_set['forms'].pop()]
        fixture.local.write_private(fixture.form_ref, encode(fixture.form_set))
        fixture.config['owner_local_source_records'][0]['form_ids'] = [OTHER_FORM_ID]
        fixture.save()
        with self.assertRaises(REFUSAL):
            fixture.describe(OTHER_FORM_ID)
        self.assertEqual(fixture.heads(), [])

    def test_private_form_set_must_bind_its_exact_adjacent_source(self):
        fixture = self.fixture(form='freeform')
        fixture.form_set['subject']['digest'] = 'sha256:' + '0' * 64
        fixture.local.write_private(fixture.form_ref, encode(fixture.form_set))
        with self.assertRaises(REFUSAL):
            fixture.describe()
        self.assertEqual(fixture.heads(), [])

    def test_private_subject_scope_cannot_omit_authored_or_native_source_languages(self):
        for target in ('occurrence', 'freeform'):
            with self.subTest(subject=target):
                fixture = self.fixture(source=True, form='freeform' if target == 'freeform' else None)
                # A Russian review or projection cannot erase the English
                # description and explicitly undetermined native language.
                fixture.config['subjects'][fixture.identifier]['languages'] = ['ru']
                fixture.save()
                with self.assertRaises(REFUSAL):
                    fixture.describe()
                self.assertEqual(fixture.heads(), [])

    def test_v4_config_and_context_require_confidential_modes(self):
        fixture = self.fixture()
        for path in (fixture.owner, fixture.context_path):
            with self.subTest(path=path.name):
                path.chmod(0o644)
                try:
                    with self.assertRaises(REFUSAL):
                        fixture.describe()
                finally:
                    path.chmod(0o600)
        self.assertEqual(fixture.heads(), [])

    def test_private_journal_modes_hold_under_permissive_umask_and_on_replay(self):
        fixture = self.fixture()
        request = fixture.request()
        previous_umask = os.umask(0)
        try:
            fixture.run(request)
        finally:
            os.umask(previous_umask)
        paths = [fixture.journal, *fixture.journal.rglob('*')]
        for path in paths:
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o700 if path.is_dir() else 0o600)
        before = fixture.history_bytes()
        for path in paths:
            with self.subTest(path=path.name):
                original = stat.S_IMODE(path.stat().st_mode)
                path.chmod(0o755 if path.is_dir() else 0o644)
                try:
                    with self.assertRaises(REFUSAL):
                        fixture.run(request)
                finally:
                    path.chmod(original)
        self.assertEqual(fixture.history_bytes(), before)

    def test_journal_cannot_be_selected_outside_its_private_owner_root(self):
        fixture = self.fixture()
        for path in (fixture.public, fixture.local.base, fixture.private):
            with self.subTest(path=path.name):
                fixture.config['journal_directory'] = str(path)
                fixture.save()
                with self.assertRaises(REFUSAL):
                    fixture.describe()
        self.assertEqual(fixture.heads(), [])

    def test_public_command_and_assessed_snapshot_refuse_v4_before_private_io(self):
        fixture = self.fixture(form='freeform')
        request = {'schema_version': COMMAND_VERSION, 'operation': 'describe', 'subject_id': fixture.form.id}
        snapshot = AssessedFormSnapshot(fixture.owner, [fixture.form.id])
        snapshot._observed[fixture.form.id] = {'request': request, 'reply': {}}
        with patch.object(OwnerLocalSourceContext, 'load', side_effect=AssertionError('private context opened')) as context, \
                patch.object(NativeTextBindingResolver, 'assessment_records', side_effect=AssertionError('native content opened')) as native, \
                patch.object(AssessmentJournal, '_load', side_effect=AssertionError('private journal opened')) as journal:
            with self.assertRaises(PermissionError):
                assessment_journal.run_public_source_command(fixture.owner, request)
            with self.assertRaises(PermissionError):
                snapshot._resolve(fixture.source, fixture.form.ref, fixture.source_ref, fixture.form_ref)
            with self.assertRaises(PermissionError):
                snapshot.verify_current()
            context.assert_not_called()
            native.assert_not_called()
            journal.assert_not_called()

    def test_legacy_public_selector_does_not_read_private_namespace(self):
        fixture = self.fixture(source=True)
        fixture.config['source_records'] = [{'path': fixture.source_ref, 'record_id': fixture.source.id, 'origin_id': ORIGIN}]
        fixture.config['owner_local_source_records'] = []
        fixture.save()
        original = assessment_journal._owned_path

        def public_only(path, *args, **kwargs):
            self.assertNotEqual(path, fixture.public / fixture.source_ref)
            self.assertNotEqual(path, fixture.private / fixture.source_ref)
            return original(path, *args, **kwargs)

        with patch.object(assessment_journal, '_owned_path', public_only):
            with self.assertRaises(REFUSAL):
                fixture.describe()

    def test_v4_transport_and_access_fields_cannot_come_from_request(self):
        fixture = self.fixture()
        request = {'schema_version': COMMAND_VERSION, 'operation': 'describe', 'subject_id': fixture.identifier}
        for key, value in (('source_context_ref', str(fixture.context_path)),
                           ('source_access', fixture.local.access('exact_owner_local')),
                           ('owner_local_source_records', []), ('native_text_units', []),
                           ('journal_directory', str(fixture.journal))):
            with self.subTest(field=key), self.assertRaises(REFUSAL):
                fixture.run({**request, key: value})
        self.assertEqual(fixture.heads(), [])

    def test_missing_or_false_private_read_authority_refuses_exact_native_io(self):
        fixture = self.fixture()
        original = copy.deepcopy(fixture.config['native_text_units'][0]['source_access'])
        for value in ({**original, 'access_allowed': False}, {**original, 'access_allowed': 1},
                      {**original, 'authority_ref': ''}, {**original, 'read_scope': 'exact_public'}):
            with self.subTest(access=value):
                fixture.config['native_text_units'][0]['source_access'] = value
                fixture.save()
                with patch.object(NativeTextBindingResolver, 'assessment_records', side_effect=AssertionError('unauthorized native read')) as native:
                    with self.assertRaises(REFUSAL):
                        fixture.describe()
                    native.assert_not_called()
        self.assertEqual(fixture.heads(), [])

    def test_exact_source_context_native_and_form_drift_invalidates_expected_snapshot(self):
        fixture = self.fixture(form='freeform')
        request = fixture.request()
        paths = (fixture.owner, fixture.context_path, fixture.private / fixture.source_ref,
                 fixture.private / fixture.form_ref, fixture.packet_path,
                 fixture.public / fixture.native.rights_ref, fixture.content_path,
                 fixture.public / 'ToS/contracts/native-text-unit-assessment-subject.schema.json')
        for path in paths:
            with self.subTest(path=path.name):
                raw = path.read_bytes()
                path.write_bytes(raw + b'\n')
                try:
                    with self.assertRaises(REFUSAL):
                        fixture.run(request)
                finally:
                    path.write_bytes(raw)
        self.assertEqual(fixture.heads(), [])

    def test_private_form_uses_exact_context_grammar_and_binds_its_schema_bytes(self):
        fixture = self.fixture(form='freeform')
        request = fixture.request()
        path = fixture.public / 'ToS/contracts/human-form.schema.json'
        raw = path.read_bytes()
        # Equivalent JSON still changes the selected grammar's exact bytes.
        fixture.native.write_bytes(path.relative_to(fixture.public).as_posix(), raw + b'\n')
        with self.assertRaises(REFUSAL):
            fixture.run(request)
        schema = json.loads(raw)
        schema['required'].append('synthetic_unknown_required_field')
        fixture.native.write_bytes(path.relative_to(fixture.public).as_posix(), encode(schema))
        # A fresh discovery must validate the actual context grammar, not an
        # older cached/global checkout schema with the same public identity.
        with self.assertRaises(ValidationError):
            fixture.describe()
        self.assertEqual(fixture.heads(), [])

    def test_private_materialization_keeps_context_grammar_after_renderer_warmup(self):
        fixture = self.fixture(form='freeform')
        warm = fixture.request('materialize-form')
        self.assertEqual(fixture.run(warm)['result']['materialization']['state'],
                         'needs-assessment')
        # Evolve only this synthetic source context's grammar. A cached
        # validator from another checkout must not become a second owner.
        ref = 'ToS/contracts/human-form.schema.json'
        schema = json.loads((fixture.public / ref).read_bytes())
        schema['properties']['synthetic_context_marker'] = {'type': 'string'}
        schema['required'].append('synthetic_context_marker')
        fixture.native.write_bytes(ref, encode(schema))
        for body in fixture.form_set['forms']:
            body['synthetic_context_marker'] = 'source-context-grammar'
        body = next(row for row in fixture.form_set['forms'] if row['form_id'] == fixture.form.id)
        fixture.form = Record.from_payload(body['form_id'], body['form_version'], body)
        fixture.config['subjects'][fixture.form.id]['record'] = fixture.form.ref
        fixture.local.write_private(fixture.form_ref, encode(fixture.form_set))
        fixture.save()
        fresh = fixture.describe()
        self.assertNotEqual(fresh['owner_snapshot'], warm['expected_snapshot'])
        committed = fixture.run(fixture.request())
        self.assertTrue(committed['result']['current_admission']['can_use'])
        result = fixture.run(fixture.request('materialize-form'))
        self.assert_local(result)
        self.assertEqual(result['result']['materialization']['state'], 'ready')
        self.assertEqual(result['result']['materialization']['display_text'], body['content']['text'])

    def test_private_form_scope_covers_explicit_public_binding_languages(self):
        fixture = self.fixture(form='freeform')
        source = owner_fixtures.lexeme()
        source.update(record_id='tos.lexeme.synthetic.public-german',
                      visibility='public_metadata_only', preferred_label='synthetisch')
        source['semantic_scope']['language'] = 'de'
        source['semantic_content']['language'] = 'de'
        for language in source['field_languages'].values():
            language['language'] = 'de'
        ref = 'ToS/source-witnesses/semantic-descriptions/lexemes/synthetic-de/lexeme.json'
        fixture.native.write_bytes(ref, encode(source))
        selected = Record.from_payload(source['record_id'], source['record_version'], source)
        fixture.config['source_records'] = [{'path': ref, 'record_id': selected.id, 'origin_id': ORIGIN}]
        body = next(row for row in fixture.form_set['forms'] if row['form_id'] == fixture.form.id)
        body['bindings']['german_context'] = {'record': selected.ref, 'pointer': '/preferred_label'}
        fixture.form = Record.from_payload(body['form_id'], body['form_version'], body)
        fixture.config['subjects'][fixture.form.id]['record'] = fixture.form.ref
        fixture.local.write_private(fixture.form_ref, encode(fixture.form_set))
        fixture.save()
        # Public delivery does not erase the language of an exact source
        # binding or grant the assessor competence in that language.
        with self.assertRaises(PermissionError):
            fixture.describe()
        fixture.config['subjects'][fixture.form.id]['languages'].append('de')
        fixture.save()
        with self.assertRaises(assessment_journal.AssessmentRejected) as rejected:
            fixture.run(fixture.request())
        reasons = rejected.exception.invalid_assessments[0]['reasons']
        self.assertIn('authority.outside-scope', reasons)
        self.assertIn('competence.outside-scope', reasons)
        self.assertEqual(fixture.heads(), [])

    def test_changed_private_inputs_cannot_publish_or_return_stale_replay(self):
        for edge in ('after-lock', 'after-blob', 'replay-after-evaluation'):
            with self.subTest(edge=edge):
                fixture = self.fixture()
                request = fixture.request()
                if edge == 'replay-after-evaluation':
                    fixture.run(request)
                before = {path: path.read_bytes() for path in fixture.heads()}

                def mutate():
                    fixture.packet_path.write_bytes(fixture.packet_path.read_bytes() + b'\n')

                if edge == 'after-lock':
                    original = AssessmentJournal._locked

                    @contextmanager
                    def changed_lock(journal, *args, **kwargs):
                        with original(journal, *args, **kwargs):
                            mutate()
                            yield

                    target = patch.object(AssessmentJournal, '_locked', changed_lock)
                elif edge == 'after-blob':
                    original = AssessmentJournal._write_blob

                    def changed_blob(journal, *args, **kwargs):
                        result = original(journal, *args, **kwargs)
                        mutate()
                        return result

                    target = patch.object(AssessmentJournal, '_write_blob', changed_blob)
                else:
                    original = AssessmentEngine.evaluate

                    def changed_evaluation(engine, *args, **kwargs):
                        result = original(engine, *args, **kwargs)
                        mutate()
                        return result

                    target = patch.object(AssessmentEngine, 'evaluate', changed_evaluation)
                with target, self.assertRaises(REFUSAL):
                    fixture.run(request)
                self.assertEqual({path: path.read_bytes() for path in fixture.heads()}, before)


if __name__ == '__main__':
    unittest.main()

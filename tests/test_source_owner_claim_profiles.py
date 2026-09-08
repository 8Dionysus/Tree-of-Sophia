"""Private Claim reader checks over synthetic source bytes, never real stores.

The reusable fixture constructs no historical evidence, rights assessment or
accepted linguistic relation. Its original Item payload is never created.
"""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tests.test_native_text_binding import NativeTextBindingFixture, REPO_ROOT, digest
from tests.test_source_owner_record_profiles import (
    PACKET_REF, PREFIX, PROFILE_SCHEMAS, SOURCE_REF, STORE_ID, lexeme, occurrence,
)
from source_owner_context import CONTEXT_SCHEMA_REF, OWNER_LOCAL_HOME, OwnerLocalSourceContext
from source_owner_claim_profiles import OwnerLocalSourceClaimProfiles
from source_record_profiles import (
    CLAIM_CONTRACT_REF, CLAIM_REGISTRY_REF, CONTRACT_REF, REGISTRY_REF,
    SourceClaimProfiles, SourceProfileError,
)


CLAIM_REF = PREFIX + 'claims/synthetic/source-claims.jsonl'
FORM_REF = 'ToS/source-witnesses/lexical-descriptions/synthetic/lexical-form.json'
RELATION_TYPE_ID = 'tos.relation.occurrence-has-form'
CLAIM_SCHEMAS = (
    'claim-packet.schema.json', 'knowledge-assessment.schema.json',
    'source-claim-record.schema.json', 'semantic-relation-claim.schema.json',
    'linguistic-relation-claim.schema.json', 'native-text-unit-assessment-subject.schema.json',
)


class OwnerLocalClaimFixture:
    """Portable synthetic private Claim closure under a caller-owned base."""

    def __init__(self, base: Path):
        self.base = base
        self.public, self.private = base / 'public', base / 'private'
        self.public.mkdir(mode=0o755)
        self.private.mkdir(mode=0o700)
        self.native = NativeTextBindingFixture(self.public)
        for ref in (REGISTRY_REF, CONTRACT_REF, CLAIM_REGISTRY_REF, CLAIM_CONTRACT_REF,
                    CONTEXT_SCHEMA_REF, *('ToS/contracts/' + name for name in (*PROFILE_SCHEMAS, *CLAIM_SCHEMAS))):
            self.native.write_bytes(ref, (REPO_ROOT / ref).read_bytes())
        self.native.packet['rights_and_visibility']['packet_visibility'] = 'local_only'
        self.sync_native()
        self.source = occurrence(self.binding)
        self.write_private(SOURCE_REF, self.encode(self.source))
        self.form = lexeme()
        self.form.update(record_type='lexical-form', record_id='tos.lexical-form.synthetic.claim',
                         visibility='public_metadata_only')
        self.form['semantic_content'] = {'form_account': 'Synthetic represented spelling; not an occurrence.',
                                        'language': 'en', 'script': 'Latn'}
        self.form['form_identity'] = {'written_representation': 'cafe\u0301',
            'language': 'x-test', 'script': 'Latn', 'representation_kind': 'orthographic',
            'notation_scope': 'Only this artificial notation.', 'unicode_posture': 'preserved_as_supplied'}
        self.native.write_json(FORM_REF, self.form)
        self.claim = {'schema_version': 'tos_semantic_relation_claim_v1',
            'claim_id': 'tos.claim.synthetic.owner-local-form', 'claim_version': 1,
            'claim_type': 'relation', 'assertion_layer': 'linguistic_analysis',
            'subject_ref': self.source['record_id'], 'predicate': 'occurrence_has_form',
            'object': self.form['record_id'], 'evidence_refs': [PACKET_REF],
            'maker': {'maker_type': 'model', 'agent_ref': 'model:synthetic-claim-fixture'},
            'provenance_event_ref': 'tos.event.synthetic.unresolved-claim-origin',
            'epistemic_status': 'uncertain', 'review_status': 'unreviewed', 'visibility': 'local_only',
            'polarity': 'unknown', 'qualifiers': {
                'statement': 'Это лишь синтетическая возможность, не установленный разбор.\r\n',
                'statement_language': 'ru', 'statement_script': 'Cyrl',
                'relation_basis': 'Synthetic test grounding, not string equality.',
                'attestation_scope': 'This selected artificial source use only.',
                'unknown': {'negative': False, 'empty': '', 'missing': None, 'zero': 0}},
            'extensions': {'uninterpreted': ['cafe\u0301', '\r\n', None, False]}}
        self.write_claims(self.claim)
        self.config_path = base / 'context.json'
        self.config = {'schema_version': 'tos_owner_local_source_context_v1', 'store_id': STORE_ID,
            'public_root': str(self.public), 'private_root': str(self.private), 'private_prefix': PREFIX}
        self.config_path.write_bytes(self.encode(self.config))
        self.config_path.chmod(0o600)
        self.context = OwnerLocalSourceContext.load(self.config_path)

    @staticmethod
    def encode(value):
        return (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode('utf-8')

    @staticmethod
    def access(exact=False):
        return {'read_scope': 'exact_owner_local' if exact else 'metadata_only',
                'access_allowed': True, 'authority_ref': 'authority:synthetic-claim-fixture'}

    def write_private(self, ref, raw):
        path = self.private / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        current = path.parent
        while current.is_relative_to(self.private):
            current.chmod(0o700)
            if current == self.private:
                break
            current = current.parent
        path.write_bytes(raw)
        path.chmod(0o600)
        return path

    def write_claims(self, *rows):
        raw = ''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
        return self.write_private(CLAIM_REF, raw)

    def sync_native(self):
        self.native.refresh()
        self.binding = copy.deepcopy(self.native.binding)
        self.binding['packet_ref'] = PACKET_REF
        self.write_private(PACKET_REF, (self.public / self.native.packet_ref).read_bytes())
        (self.public / self.native.packet_ref).unlink()
        if hasattr(self, 'source'):
            self.source['native_text_binding'] = copy.deepcopy(self.binding)
            self.write_private(SOURCE_REF, self.encode(self.source))

    def source_selections(self, exact=False):
        return [{'path': SOURCE_REF, 'record_id': self.source['record_id'],
                 'profile_type_id': 'tos.entity.occurrence', 'origin_id': 'origin:synthetic-native',
                 'source_access': self.access(exact), 'source_binding': copy.deepcopy(self.binding)},
                {'path': FORM_REF, 'record_id': self.form['record_id'],
                 'profile_type_id': 'tos.entity.lexical-form', 'origin_id': 'origin:synthetic-lexical-form',
                 'source_access': self.access(), 'source_binding': None}]

    def claim_selection(self, exact=False, form_ids=()):
        return {'path': CLAIM_REF, 'claim_id': self.claim['claim_id'], 'relation_type_id': RELATION_TYPE_ID,
            'origin_id': 'origin:synthetic-claim', 'source_access': self.access(exact),
            'source_records': self.source_selections(exact), 'native_bindings': [],
            'verify_content': exact, 'form_ids': list(form_ids)}

    def reader(self, exact=False, **overrides):
        kwargs = {'source_access': self.access(exact), 'source_records': self.source_selections(exact),
                  'verify_content': exact, **overrides}
        return OwnerLocalSourceClaimProfiles(self.context, **kwargs)

    def load(self, reader=None):
        return (reader or self.reader()).load(CLAIM_REF, self.claim['claim_id'],
            origin_id='origin:synthetic-claim', relation_type_id=RELATION_TYPE_ID)


class OwnerLocalSourceClaimProfilesTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='tos-owner-claim-profile-')
        self.addCleanup(temporary.cleanup)
        self.fixture = OwnerLocalClaimFixture(Path(temporary.name))

    def test_candidate_without_source_package_uses_real_selected_dependencies(self):
        f, seen = self.fixture, []
        stored = f.reader()
        expected = f.load(stored)
        expected_refs = stored.dependency_refs(f.claim['claim_id'])
        expected_languages = stored.required_languages(f.claim['claim_id'])
        expected_summaries = stored.native_summaries
        target = f.private / CLAIM_REF
        target.unlink()
        target.parent.rmdir()
        before = set(f.private.rglob('*'))
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()  # No virtual source file or inline endpoint.
        reader = f.reader(source_reader=read)
        actual = reader.prepare_candidate(f.claim, origin_id='origin:synthetic-claim',
                                           relation_type_id=RELATION_TYPE_ID)
        self.assertEqual(actual, expected)
        self.assertEqual(reader.dependency_refs(actual['id']), expected_refs)
        self.assertEqual(reader.required_languages(actual['id']), expected_languages)
        self.assertEqual(reader.native_summaries, expected_summaries)
        self.assertEqual({row['id'] for row in reader.records}, {row['id'] for row in expected_refs})
        self.assertIsNone(reader._claim_input)
        self.assertNotIn(target, seen)
        self.assertIn(f.private / SOURCE_REF, seen)
        self.assertIn(f.public / FORM_REF, seen)
        self.assertEqual(set(f.private.rglob('*')), before)
        snapshot = reader.snapshot()
        f.write_claims({**f.claim, 'claim_id': 'tos.claim.synthetic.unrelated'})
        self.assertEqual(reader.snapshot(), snapshot)

    def test_candidate_payload_origin_relation_and_mode_are_frozen(self):
        f, reader = self.fixture, self.fixture.reader()
        candidate = copy.deepcopy(f.claim)
        loaded = reader.prepare_candidate(candidate, origin_id='origin:synthetic-claim',
                                           relation_type_id=RELATION_TYPE_ID)
        snapshot = reader.snapshot()
        candidate['qualifiers']['statement'] = 'Caller mutation is not a new source.'
        loaded['payload']['qualifiers']['statement'] = 'Returned envelopes are detached.'
        self.assertEqual(reader.snapshot(), snapshot)
        self.assertEqual(reader.prepare_candidate(f.claim, origin_id='origin:synthetic-claim',
            relation_type_id=RELATION_TYPE_ID)['payload'], f.claim)
        different = f.reader()
        different.prepare_candidate(candidate, origin_id='origin:synthetic-claim', relation_type_id=RELATION_TYPE_ID)
        self.assertNotEqual(different.snapshot(), snapshot)
        for value, origin, relation in ((candidate, 'origin:synthetic-claim', RELATION_TYPE_ID),
                (f.claim, 'origin:another', RELATION_TYPE_ID), (f.claim, 'origin:synthetic-claim', None)):
            with self.subTest(origin=origin, relation=relation), self.assertRaises(SourceProfileError):
                reader.prepare_candidate(value, origin_id=origin, relation_type_id=relation)
        with self.assertRaises(SourceProfileError):
            f.load(reader)
        stored = f.reader()
        f.load(stored)
        self.assertNotEqual(stored.snapshot(), snapshot)
        with self.assertRaises(SourceProfileError):
            stored.prepare_candidate(f.claim, origin_id='origin:synthetic-claim', relation_type_id=RELATION_TYPE_ID)

    def test_candidate_failure_cannot_be_reused_as_candidate_or_stored_source(self):
        f = self.fixture
        reader = f.reader()
        invalid = {**f.claim, 'object': f.source['record_id']}
        with self.assertRaises(SourceProfileError):
            reader.prepare_candidate(invalid, origin_id='origin:synthetic-claim')
        with self.assertRaises(SourceProfileError):
            reader.prepare_candidate(f.claim, origin_id='origin:synthetic-claim')
        with self.assertRaises(SourceProfileError):
            f.load(reader)
        (f.private / CLAIM_REF).unlink()
        stored = f.reader()
        with self.assertRaises(SourceProfileError):
            f.load(stored)
        with self.assertRaises(SourceProfileError):
            stored.prepare_candidate(f.claim, origin_id='origin:synthetic-claim')

    def test_candidate_rejects_shape_visibility_and_budget_before_source_reads(self):
        f, seen = self.fixture, []
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        for candidate in ([], {**f.claim, 'visibility': 'public'}, {**f.claim, 'polarity': False},
                {**f.claim, 'claim_id': None}, {**f.claim, 'confidence': float('nan')},
                {**f.claim, 'qualifiers': {**f.claim['qualifiers'], 'statement': 'x' * 1_048_577}}):
            reader = f.reader(source_reader=read)
            seen.clear()
            with self.subTest(candidate_type=type(candidate).__name__), self.assertRaises(SourceProfileError):
                reader.prepare_candidate(candidate, origin_id='origin:synthetic-claim')
            self.assertFalse(any(path.is_relative_to(f.private) for path in seen))

    def test_candidate_keeps_exact_rights_and_current_dependency_checks(self):
        f, seen = self.fixture, []
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        (f.private / CLAIM_REF).unlink()
        reader = f.reader(exact=True, source_reader=read)
        reader.prepare_candidate(f.claim, origin_id='origin:synthetic-claim')
        self.assertTrue(reader.native_summaries[0]['content_verified'])
        self.assertIn(f.public / f.native.content_ref, seen)
        self.assertNotIn(f.public / f.native.original_ref, seen)
        f.native.rights['derivative_posture'] = 'permission_required'
        f.sync_native()
        with self.assertRaises(SourceProfileError):
            reader.snapshot()
        seen.clear()
        denied = f.reader(exact=True, source_reader=read)
        with self.assertRaises(SourceProfileError):
            denied.prepare_candidate(f.claim, origin_id='origin:synthetic-claim')
        self.assertNotIn(f.public / f.native.content_ref, seen)

    def test_private_claim_and_full_distinct_source_envelopes(self):
        f, reader = self.fixture, self.fixture.reader()
        original = (f.private / CLAIM_REF).read_bytes()
        claim = f.load(reader)
        self.assertEqual(claim, {'id': f.claim['claim_id'], 'version': 1,
            'payload': f.claim, 'origin_id': 'origin:synthetic-claim'})
        records = {row['id']: row for row in reader.records}
        self.assertEqual(set(records), {f.source['record_id'], f.form['record_id'],
            f.binding['unit_id'], f.binding['text_layer']['layer_id']})
        self.assertNotIn(claim['id'], records)
        self.assertEqual(records[f.source['record_id']]['payload'], f.source)
        self.assertEqual(records[f.form['record_id']]['payload'], f.form)
        unit = records[f.binding['unit_id']]
        layer = records[f.binding['text_layer']['layer_id']]
        self.assertEqual(unit['origin_id'], layer['origin_id'])
        self.assertEqual(unit['payload']['packet'], f.native.packet)
        self.assertEqual(layer['payload'], f.native.layer)
        self.assertEqual({ref['id'] for ref in reader.dependency_refs(claim['id'])}, set(records))
        for row in records.values():
            raw = json.dumps(row['payload'], ensure_ascii=False, sort_keys=True,
                             separators=(',', ':'), allow_nan=False).encode('utf-8')
            self.assertIn({'id': row['id'], 'version': row['version'], 'digest': 'sha256:' + digest(raw)},
                          reader.dependency_refs(claim['id']))
        summary, = reader.native_summaries
        self.assertTrue(summary['supporting_only'])
        self.assertFalse(summary['content_verified'])
        self.assertFalse(summary['assessment_applied'])
        self.assertEqual({ref['id'] for ref in summary['record_refs']}, {unit['id'], layer['id']})
        self.assertEqual(reader.required_languages(claim['id']), ('en', 'ru', 'und', 'x-test'))
        self.assertRegex(reader.snapshot(), r'^sha256:[a-f0-9]{64}$')
        self.assertIn(CLAIM_REGISTRY_REF, reader.contract_digests)
        self.assertNotIn(CLAIM_REF, reader.contract_digests)
        self.assertEqual((f.private / CLAIM_REF).read_bytes(), original)
        claim['payload']['qualifiers']['statement'] = 'mutated returned copy'
        records[f.source['record_id']]['payload']['notes'] = 'mutated returned copy'
        self.assertEqual(f.load(reader)['payload'], f.claim)
        self.assertEqual(next(row for row in reader.records if row['id'] == f.source['record_id'])['payload'], f.source)
        for name in ('catalog_entry', 'export', 'write', 'native_resolver'):
            self.assertFalse(hasattr(reader, name), name)

    def test_metadata_mode_does_not_require_or_open_representation(self):
        f, seen = self.fixture, []
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        reader = f.reader(source_access=f.access(True), source_records=f.source_selections(True), source_reader=read)
        (f.public / f.native.content_ref).unlink()
        f.load(reader)
        reader.snapshot()
        self.assertFalse(reader.native_summaries[0]['content_verified'])
        self.assertNotIn(f.public / f.native.content_ref, seen)
        self.assertNotIn(f.public / f.native.original_ref, seen)
        self.assertFalse((f.public / f.native.original_ref).exists())

    def test_exact_mode_is_a_separate_frozen_native_view(self):
        f, seen = self.fixture, []
        metadata = f.reader()
        f.load(metadata)
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        exact = f.reader(exact=True, source_reader=read)
        f.load(exact)
        self.assertIn(f.public / f.native.content_ref, seen)
        self.assertNotIn(f.public / f.native.original_ref, seen)
        summary, = exact.native_summaries
        self.assertTrue(summary['metadata_verified'])
        self.assertTrue(summary['content_verified'])
        self.assertFalse(summary['original_payload_verified'])
        self.assertFalse(summary['public_content_available'])
        self.assertEqual((f.public / f.native.content_ref).read_bytes(), f.native.content)
        metadata_ref = next(row for row in metadata.dependency_refs(f.claim['claim_id']) if row['id'] == f.binding['unit_id'])
        exact_ref = next(row for row in exact.dependency_refs(f.claim['claim_id']) if row['id'] == f.binding['unit_id'])
        self.assertEqual(metadata_ref['id'], exact_ref['id'])
        self.assertEqual(metadata_ref['version'], exact_ref['version'])
        self.assertNotEqual(metadata_ref['digest'], exact_ref['digest'])
        self.assertFalse(metadata.native_summaries[0]['content_verified'])

    def test_access_is_preflighted_for_every_selector_before_private_reads(self):
        f, seen = self.fixture, []
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        denied_sources = f.source_selections()
        denied_sources[-1]['source_access']['access_allowed'] = False
        denied_native = {'binding': f.binding, 'origin_id': 'origin:synthetic-native',
                         'source_access': {**f.access(), 'access_allowed': False}}
        for overrides in (
            {'source_access': {**f.access(), 'access_allowed': False}},
            {'source_records': denied_sources}, {'native_bindings': [denied_native]},
            {'verify_content': True},
            {'source_access': f.access(True), 'verify_content': True},
        ):
            with self.subTest(overrides=list(overrides)), self.assertRaises(SourceProfileError):
                f.reader(source_reader=read, **overrides)
            self.assertFalse(any(path.is_relative_to(f.private) for path in seen))

    def test_selector_keys_and_access_shapes_are_strict(self):
        f = self.fixture
        invalid_access = [None, [], {}, {**f.access(), 'read_scope': []},
            {**f.access(), 'read_scope': {}}, {**f.access(), 'read_scope': 1},
            {**f.access(), 'access_allowed': 1}, {**f.access(), 'authority_ref': ''},
            {**f.access(), 'extra_permission': True}]
        for access in invalid_access:
            with self.subTest(access=access), self.assertRaises(SourceProfileError):
                f.reader(source_access=access)
        for value in (1, 'true', None):
            with self.subTest(verify_content=value), self.assertRaises(SourceProfileError):
                f.reader(verify_content=value)
        for change in ({'payload': f.source}, {'kind': 'occurrence'}, {'record_type': 'occurrence'},
                       {'profile_type_id': 'tos.entity.form'}, {'source_binding': []}, {'origin_id': ''}):
            selectors = f.source_selections()
            selectors[0].update(change)
            with self.subTest(change=list(change)), self.assertRaises(SourceProfileError):
                f.reader(source_records=selectors)
        for selection in ({'binding': f.binding},
                          {'binding': f.binding, 'origin_id': 'origin:native', 'source_access': f.access(), 'payload': {}}):
            with self.subTest(native_keys=list(selection)), self.assertRaises(SourceProfileError):
                f.reader(native_bindings=[selection])

    def test_selection_and_access_inputs_are_copied_not_live_grants(self):
        f = self.fixture
        access, selectors = f.access(), f.source_selections()
        reader = f.reader(source_access=access, source_records=selectors)
        access.update(read_scope='exact_owner_local', access_allowed=False)
        selectors[0]['source_binding']['unit_version'] = 99
        selectors[0]['source_access']['access_allowed'] = False
        f.load(reader)
        self.assertFalse(reader.native_summaries[0]['content_verified'])
        with self.assertRaises(SourceProfileError):
            reader.load(CLAIM_REF, 'tos.claim.another', origin_id='origin:synthetic-claim')
        with self.assertRaises(SourceProfileError):
            reader.dependency_refs('tos.claim.another')

    def test_private_and_public_claims_share_shape_without_sharing_visibility(self):
        f = self.fixture
        public = SourceClaimProfiles(f.public)
        published = {**copy.deepcopy(f.claim), 'visibility': 'public_metadata_only'}
        objects = {row['record_id']: row for row in (f.source, f.form)}
        public.validate(published, objects)
        with self.assertRaises(SourceProfileError):
            public.validate(f.claim, objects)
        invalids = [dict(f.claim, polarity=False), dict(f.claim, claim_version=True),
                    dict(f.claim, evidence_refs=[]), dict(f.claim, review_status='accepted'),
                    dict(f.claim, assertion_layer='canon_judgment'),
                    dict(f.claim, predicate='not_a_declared_predicate')]
        invalid = copy.deepcopy(f.claim)
        invalid['qualifiers'].pop('attestation_scope')
        invalids.append(invalid)
        for index, claim in enumerate(invalids):
            f.write_claims(claim)
            with self.subTest(index=index):
                with self.assertRaises(SourceProfileError):
                    public.validate({**claim, 'visibility': 'public'}, objects)
                with self.assertRaises(SourceProfileError):
                    f.load()
        f.write_claims(published)
        with self.assertRaises(SourceProfileError):
            f.load()

    def test_expected_relation_and_real_endpoint_types_are_not_inferred(self):
        f = self.fixture
        with self.assertRaises(SourceProfileError):
            f.reader().load(CLAIM_REF, f.claim['claim_id'], origin_id='origin:claim',
                            relation_type_id='tos.relation.occurrence-has-sense')
        selectors = f.source_selections()
        selectors[0]['record_id'] += '.wrong'
        f.write_claims({**f.claim, 'subject_ref': selectors[0]['record_id']})
        with self.assertRaises(SourceProfileError):
            f.load(f.reader(source_records=selectors))
        # A real Occurrence cannot serve as the LexicalForm range even when
        # the request names its identity and the predicate is understood.
        f.write_claims({**f.claim, 'object': f.source['record_id']})
        with self.assertRaises(SourceProfileError):
            f.load(f.reader(source_records=f.source_selections()[:1]))
        selectors = f.source_selections()
        selectors[1]['profile_type_id'] = 'tos.entity.lexeme'
        with self.assertRaises(SourceProfileError):
            f.reader(source_records=selectors)

    def test_private_path_guards_precede_claim_or_source_payload_reads(self):
        f, seen = self.fixture, []
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        for ref in (str(f.private / CLAIM_REF),
                    CLAIM_REF.replace(STORE_ID, 'sid-' + '2' * 32),
                    PREFIX + '.hidden/source-claims.jsonl', PREFIX + 'payload/source-claims.jsonl',
                    PREFIX + 'local-content/source-claims.jsonl', PREFIX + 'catalog/source-claims.jsonl',
                    PREFIX + '../source-claims.jsonl', FORM_REF, PREFIX + 'claims/source.json'):
            with self.subTest(ref=ref), self.assertRaises(SourceProfileError):
                f.reader(source_reader=read).load(ref, f.claim['claim_id'], origin_id='origin:claim')
            self.assertFalse(any(path.is_relative_to(f.private) for path in seen))
        selectors = f.source_selections()
        selectors[0]['path'] = PREFIX + 'payload/occurrence.json'
        with self.assertRaises(SourceProfileError):
            f.reader(source_records=selectors, source_reader=read)
        self.assertFalse(any(path.is_relative_to(f.private) for path in seen))

    def test_unsafe_claim_modes_and_aliases_never_reach_the_callback(self):
        f, seen = self.fixture, []
        target = f.private / CLAIM_REF
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        for unsafe in ('file-mode', 'directory-mode', 'symlink'):
            reader = f.reader(source_reader=read)
            if unsafe == 'file-mode':
                target.chmod(0o644)
            elif unsafe == 'directory-mode':
                target.parent.chmod(0o755)
            else:
                target.unlink()
                target.symlink_to(f.private / SOURCE_REF)
            seen.clear()
            try:
                with self.subTest(unsafe=unsafe), self.assertRaises(SourceProfileError):
                    f.load(reader)
                self.assertNotIn(target, seen)
            finally:
                if unsafe == 'symlink':
                    target.unlink()
                    f.write_claims(f.claim)
                elif unsafe == 'file-mode':
                    target.chmod(0o600)
                else:
                    target.parent.chmod(0o700)

    def test_strict_json_and_duplicate_stream_identities_fail_closed(self):
        f = self.fixture
        valid = json.dumps(f.claim)
        for raw in (b'[]\n', valid[:-1].encode() + b', "claim_id": "tos.claim.shadow"}\n',
                    valid[:-1].encode() + b', "confidence": NaN}\n',
                    b'{"claim_id": "tos.claim.invalid", "unknown": 1e999}\n', b'\xff\n'):
            f.write_private(CLAIM_REF, raw)
            with self.subTest(raw=raw[:30]), self.assertRaises(SourceProfileError):
                f.load()
        for rows in ((f.claim, f.claim),
                     (f.claim, {'claim_id': 'tos.claim.neighbor'}, {'claim_id': 'tos.claim.neighbor'})):
            f.write_claims(*rows)
            reader = f.reader()
            with self.assertRaises(SourceProfileError):
                f.load(reader)
            f.write_claims(f.claim)
            with self.assertRaises(SourceProfileError):
                f.load(reader)

    def test_row_order_is_not_claim_identity_and_neighbors_are_not_followed(self):
        f = self.fixture
        neighbor = {**copy.deepcopy(f.claim), 'claim_id': 'tos.claim.synthetic.unselected',
                    'evidence_refs': [PREFIX + 'payload/do-not-open.txt']}
        f.write_claims(neighbor, f.claim)
        first = f.reader()
        claim = f.load(first)
        snapshot, refs = first.snapshot(), first.dependency_refs(f.claim['claim_id'])
        f.write_claims(f.claim, neighbor)
        with self.assertRaises(SourceProfileError):
            first.snapshot()
        second = f.reader()
        self.assertEqual(f.load(second), claim)
        self.assertEqual(second.dependency_refs(f.claim['claim_id']), refs)
        self.assertNotEqual(second.snapshot(), snapshot)

    def test_only_selected_source_and_native_evidence_are_followed(self):
        f, seen = self.fixture, []
        never = PREFIX + 'payload/private-not-an-adapter.txt'
        f.source['extensions']['uninterpreted_ref'] = never
        f.source['source_refs'].append(never)
        f.write_private(SOURCE_REF, f.encode(f.source))
        claim = copy.deepcopy(f.claim)
        claim['evidence_refs'] = [f.source['record_id']]
        claim['counterevidence_refs'] = [FORM_REF]
        claim['alternative_claim_refs'] = ['tos.claim.not-selected']
        claim['supersedes_claim_ref'] = 'tos.claim.not-resolved-here'
        claim['qualifiers']['unknown']['file_ref'] = never
        f.write_claims(claim)
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        loaded = f.load(f.reader(source_reader=read))
        self.assertEqual(loaded['payload'], claim)
        self.assertNotIn(f.private / never, seen)
        # A real file, provenance ID, native original payload or URL does not
        # acquire a typed evidence adapter merely by appearing in evidence.
        for ref in (never, f.native.policy_ref, f.claim['provenance_event_ref'],
                    f.native.content_ref, f.native.original_ref, 'https://example.invalid/evidence'):
            f.write_claims({**f.claim, 'evidence_refs': [ref]})
            seen.clear()
            with self.subTest(ref=ref), self.assertRaises(SourceProfileError):
                f.load(f.reader(exact=True, source_reader=read))
            self.assertNotIn(f.public / f.native.content_ref, seen)
            self.assertNotIn(f.public / f.native.original_ref, seen)

    def test_native_aliases_and_quote_anchors_stay_with_the_selected_unit(self):
        f = self.fixture
        for ref in (f.binding['unit_id'], f.binding['ordered_anchor_refs'][0]):
            claim = {**copy.deepcopy(f.claim), 'evidence_refs': [ref],
                'supporting_quotes': [{'anchor_ref': f.binding['ordered_anchor_refs'][0],
                                       'exact': 'cafe\u0301', 'translation_posture': 'source'}]}
            f.write_claims(claim)
            reader = f.reader()
            self.assertEqual(f.load(reader)['payload'], claim)
            self.assertEqual(len(reader.native_summaries), 1)
        for anchor in (f.native.gap_id, f.native.scope_id, 'tos.anchor.unselected'):
            f.write_claims({**f.claim, 'supporting_quotes': [{'anchor_ref': anchor, 'exact': 'not selected'}]})
            with self.subTest(anchor=anchor), self.assertRaises(SourceProfileError):
                f.load()

    def test_additional_native_evidence_is_not_a_fake_typed_endpoint(self):
        f = self.fixture
        lexical = lexeme()
        lexical_ref = PREFIX + 'lexical/synthetic/lexeme.json'
        f.write_private(lexical_ref, f.encode(lexical))
        claim = {**copy.deepcopy(f.claim), 'predicate': 'lexical_form_of',
                 'subject_ref': f.form['record_id'], 'object': lexical['record_id']}
        f.write_claims(claim)
        sources = [f.source_selections()[1], {'path': lexical_ref, 'record_id': lexical['record_id'],
            'profile_type_id': 'tos.entity.lexeme', 'origin_id': 'origin:synthetic-lexeme',
            'source_access': f.access(), 'source_binding': None}]
        native = {'binding': f.binding, 'origin_id': 'origin:synthetic-direct-native',
                  'source_access': f.access()}
        reader = f.reader(source_records=sources, native_bindings=[native])
        self.assertEqual(reader.load(CLAIM_REF, claim['claim_id'], origin_id='origin:claim',
            relation_type_id='tos.relation.lexical-form-of')['payload'], claim)
        self.assertEqual({row['id'] for row in reader.records}, {f.form['record_id'],
            lexical['record_id'], f.binding['unit_id'], f.binding['text_layer']['layer_id']})
        self.assertTrue(reader.native_summaries[0]['supporting_only'])
        f.write_claims({**claim, 'object': f.binding['unit_id']})
        with self.assertRaises(SourceProfileError):
            f.reader(source_records=sources[:1], native_bindings=[native]).load(
                CLAIM_REF, claim['claim_id'], origin_id='origin:claim')

    def test_duplicate_inputs_require_same_body_version_and_origin(self):
        f = self.fixture
        sources = f.source_selections()
        duplicate = {'binding': f.binding, 'origin_id': sources[0]['origin_id'], 'source_access': f.access()}
        reader = f.reader(source_records=[*sources, copy.deepcopy(sources[1])], native_bindings=[duplicate])
        f.load(reader)
        self.assertEqual(len(reader.records), 4)
        self.assertEqual(len(reader.native_summaries), 1)
        conflicting = copy.deepcopy(sources[1])
        conflicting['origin_id'] = 'origin:different-source-family'
        with self.assertRaises(SourceProfileError):
            f.load(f.reader(source_records=[*sources, conflicting]))
        with self.assertRaises(SourceProfileError):
            f.reader(native_bindings=[{**duplicate, 'origin_id': 'origin:different-native-family'}])
        shadow = copy.deepcopy(f.form)
        shadow['record_version'] = 2
        shadow['semantic_content']['form_account'] = 'Another current body, not the same source.'
        ref = FORM_REF.replace('/synthetic/', '/another-synthetic/')
        f.native.write_json(ref, shadow)
        extra = {**sources[1], 'path': ref}
        with self.assertRaises(SourceProfileError):
            f.load(f.reader(source_records=[*sources, extra]))

    def test_native_research_rights_gate_precedes_exact_bytes(self):
        f, seen = self.fixture, []
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        for posture in ('permission_required', 'allowed_with_conditions'):
            f.native.rights['derivative_posture'] = posture
            f.sync_native()
            f.load(f.reader(source_reader=read))
            seen.clear()
            with self.subTest(posture=posture), self.assertRaises(SourceProfileError):
                f.load(f.reader(exact=True, source_reader=read))
            self.assertNotIn(f.public / f.native.content_ref, seen)
            self.assertNotIn(f.public / f.native.original_ref, seen)

    def test_exact_binding_fixity_failure_does_not_open_representation(self):
        f, seen = self.fixture, []
        def read(path, limit):
            seen.append(path)
            return path.read_bytes()
        selectors = f.source_selections(True)
        selectors[0]['source_binding']['packet_sha256'] = '0' * 64
        f.source['native_text_binding'] = copy.deepcopy(selectors[0]['source_binding'])
        f.write_private(SOURCE_REF, f.encode(f.source))
        with self.assertRaises(SourceProfileError):
            f.load(f.reader(exact=True, source_records=selectors, source_reader=read))
        self.assertNotIn(f.public / f.native.content_ref, seen)

    def test_consumed_source_and_contract_changes_invalidate_cached_reader(self):
        f = self.fixture
        for path in (f.private / CLAIM_REF, f.private / SOURCE_REF, f.public / FORM_REF,
                     f.public / CLAIM_REGISTRY_REF, f.public / CONTEXT_SCHEMA_REF,
                     f.public / 'ToS/contracts/linguistic-relation-claim.schema.json',
                     f.private / PACKET_REF, f.public / f.native.rights_ref):
            reader = f.reader()
            f.load(reader)
            original = path.read_bytes()
            path.write_bytes(original + b'\n')
            try:
                with self.subTest(path=path.name):
                    with self.assertRaises(SourceProfileError):
                        reader.snapshot()
                    with self.assertRaises(SourceProfileError):
                        reader.dependency_refs(f.claim['claim_id'])
            finally:
                path.write_bytes(original)

    def test_changed_endpoint_changes_dependency_ref_without_rewriting_claim(self):
        f = self.fixture
        first = f.reader()
        claim = f.load(first)
        old = first.dependency_refs(f.claim['claim_id'])
        f.form['record_version'] += 1
        f.form['semantic_content']['form_account'] = 'A revised synthetic source-owned account.'
        f.native.write_json(FORM_REF, f.form)
        with self.assertRaises(SourceProfileError):
            first.records
        second = f.reader()
        self.assertEqual(f.load(second), claim)
        new = second.dependency_refs(f.claim['claim_id'])
        self.assertNotEqual(next(row for row in old if row['id'] == f.form['record_id']),
                            next(row for row in new if row['id'] == f.form['record_id']))
        self.assertEqual(next(row for row in old if row['id'] == f.source['record_id']),
                         next(row for row in new if row['id'] == f.source['record_id']))

    def test_required_languages_only_read_owned_field_dictionaries(self):
        f = self.fixture
        f.claim['extensions']['language'] = 'zz-UNKNOWN'
        f.claim['qualifiers']['unknown']['language'] = 'not-a-requirement'
        f.source['semantic_content']['unknown_analysis']['language'] = 'not-a-source-language'
        f.source['extensions']['language'] = {'nested': 'not-language'}
        f.source['field_languages']['notes']['language'] = 'de'
        f.form['form_identity']['language'] = 'fr'
        f.write_private(SOURCE_REF, f.encode(f.source))
        f.native.write_json(FORM_REF, f.form)
        f.write_claims(f.claim)
        reader = f.reader()
        f.load(reader)
        self.assertEqual(reader.required_languages(f.claim['claim_id']), ('de', 'en', 'fr', 'ru', 'und'))

    def test_public_claim_readers_refuse_reserved_home_before_io(self):
        from build_source_witness_catalog import CatalogBuildError, collect_claims
        f = self.fixture
        public = SourceClaimProfiles(f.public)
        with patch.object(Path, 'open', side_effect=AssertionError('must not open reserved Claim')):
            with self.assertRaises(SourceProfileError):
                list(public.read_rows(CLAIM_REF))
        reserved = f.public / OWNER_LOCAL_HOME
        for alias in (False, True):
            if alias:
                reserved.symlink_to(f.base / 'absent')
            else:
                reserved.mkdir()
            try:
                with patch.object(Path, 'rglob', side_effect=AssertionError('must refuse before catalog scan')):
                    with self.subTest(alias=alias), self.assertRaises(CatalogBuildError):
                        collect_claims(f.public)
                with self.assertRaises(SourceProfileError):
                    f.reader()
            finally:
                reserved.unlink() if alias else reserved.rmdir()

    def test_adapter_budgets_refuse_instead_of_truncating(self):
        f = self.fixture
        with patch('source_owner_claim_profiles.MAX_SOURCE_RECORDS', 1):
            with self.assertRaises(SourceProfileError):
                f.reader()
        with patch('source_owner_claim_profiles.MAX_NATIVE_BINDINGS', 0):
            with self.assertRaises(SourceProfileError):
                f.reader()
        with patch('source_owner_claim_profiles.MAX_CLAIM_FILE_BYTES', 128):
            with self.assertRaises(SourceProfileError):
                f.load()
        f.write_claims(f.claim, {**f.claim, 'claim_id': 'tos.claim.another'})
        with patch('source_owner_claim_profiles.MAX_CLAIM_ROWS', 1):
            with self.assertRaises(SourceProfileError):
                f.load()
        f.write_claims({**f.claim, 'counterevidence_refs': [FORM_REF]})
        with patch('source_owner_claim_profiles.MAX_EVIDENCE_REFS', 1):
            with self.assertRaises(SourceProfileError):
                f.load()
        with patch('source_owner_claim_profiles.MAX_SNAPSHOT_FILES', 1):
            with self.assertRaises(SourceProfileError):
                f.reader()
        # Raw whitespace still costs bytes even when canonical JSON is tiny.
        f.write_private(CLAIM_REF, b' ' * 1_048_577 + json.dumps(f.claim).encode() + b'\n')
        with self.assertRaises(SourceProfileError):
            f.load()

    def test_public_endpoint_keeps_native_original_identity_inventory(self):
        f = self.fixture
        packet_ref = 'ToS/source-witnesses/semantic-descriptions/synthetic/semantic-annotation.v2.json'
        packet = f.native.skeleton('semantic-annotation-v2-abc/variant-a-occurrences-only.json')
        f.native.write_bytes('ToS/contracts/semantic-annotation-packet-v2.schema.json',
                            (REPO_ROOT / 'ToS/contracts/semantic-annotation-packet-v2.schema.json').read_bytes())
        f.native.make_public()
        public_ref = 'ToS/source-witnesses/semantic-descriptions/synthetic/occurrence.json'
        source = occurrence(f.native.binding)
        source.update(record_id=packet['entities'][0]['entity_id'], visibility='public_metadata_only')
        f.native.write_json(public_ref, source)
        f.write_claims({**f.claim, 'subject_ref': source['record_id'], 'evidence_refs': [public_ref]})
        selectors = f.source_selections()
        selectors[0].update(path=public_ref, record_id=source['record_id'], source_binding=f.native.binding)
        reader = f.reader(source_records=selectors)
        f.load(reader)
        f.native.write_json(packet_ref, packet)
        with self.assertRaises(SourceProfileError):
            reader.snapshot()
        with self.assertRaises(SourceProfileError):
            f.load(f.reader(source_records=selectors))


if __name__ == '__main__':
    unittest.main()

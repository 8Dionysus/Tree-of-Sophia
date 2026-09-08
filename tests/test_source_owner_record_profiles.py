"""Private semantic reading against synthetic source bytes and public grammar.

These tests do not open retained private material, create a durable store,
judge rights, reserve identities, perform assessment or publish a catalog.
"""
from __future__ import annotations

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tests.test_native_text_binding import NativeTextBindingFixture, REPO_ROOT, digest
from source_owner_context import CONTEXT_SCHEMA_REF, OwnerLocalSourceContext, OWNER_LOCAL_HOME
from source_owner_record_profiles import OwnerLocalSourceRecordProfiles
from source_record_profiles import (
    CONTRACT_REF, MAX_RECORD_BYTES, REGISTRY_REF, SourceProfileError, SourceRecordProfiles,
)


STORE_ID = 'sid-' + '1' * 32
PREFIX = 'ToS/source-witnesses/owner-local/' + STORE_ID + '/'
PACKET_REF = PREFIX + 'native/synthetic/source-text-unit.v1.json'
SOURCE_REF = PREFIX + 'semantic/synthetic/occurrence.json'
PROFILE_SCHEMAS = (
    'semantic-entity-type-registry.schema.json', 'source-metadata-record.schema.json',
    'semantic-description-record.schema.json', 'occurrence-description-record.schema.json',
    'lexical-description-record.schema.json',
)


def occurrence(binding):
    return {'schema_version': 'tos_occurrence_description_record_v1',
        'record_type': 'occurrence', 'record_id': 'tos.occurrence.synthetic.owner-local',
        'record_version': 1, 'preferred_label': 'cafe\u0301',
        'notes': 'Synthetic situated use only; no linguistic admission.\r\nUncertainty retained.',
        'field_languages': {'preferred_label': {'language': 'und', 'script': 'Latn'},
                           'notes': {'language': 'en', 'script': 'Latn'}},
        'identity_status': 'provisional', 'source_refs': [binding['packet_ref']],
        'external_identifiers': [], 'same_as_posture': 'no_equivalence_claim',
        'visibility': 'local_only',
        'semantic_scope': {'scope_note': 'This synthetic use only.',
            'identity_criterion': 'This bound unit, not another use of its spelling.',
            'language': 'en', 'script': 'Latn'},
        'semantic_content': {'occurrence_account': 'Proposed test token; interpretation unknown.',
            'context_account': 'Only the supplied synthetic source context.',
            'language': 'en', 'script': 'Latn',
            'unknown_analysis': {'zero': 0, 'negative': False, 'unknown': None, 'empty': ''}},
        'extensions': {'uninterpreted': ['cafe\u0301', '\r\n', None, False]},
        'native_text_binding': copy.deepcopy(binding)}


def lexeme():
    source = occurrence({'packet_ref': 'tos.work.synthetic'})
    source.pop('native_text_binding')
    source.update(schema_version='tos_lexical_description_record_v1',
                  record_type='lexeme', record_id='tos.lexeme.synthetic.owner-local')
    source['semantic_content'] = {'lexical_account': 'Synthetic grouping, not a word occurrence.',
        'grammatical_account': 'Unknown grammar; no native binding invented.',
        'language': 'en', 'script': 'Latn', 'unknown_analysis': {'unknown': None}}
    return source


class OwnerLocalSourceRecordProfilesTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='tos-owner-record-profile-')
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.public, self.private = self.base / 'public', self.base / 'private'
        self.public.mkdir(mode=0o755)
        self.private.mkdir(mode=0o700)
        self.native = NativeTextBindingFixture(self.public)
        for ref in (REGISTRY_REF, CONTRACT_REF, CONTEXT_SCHEMA_REF,
                    *('ToS/contracts/' + name for name in PROFILE_SCHEMAS)):
            self.native.write_bytes(ref, (REPO_ROOT / ref).read_bytes())
        self.native.packet['rights_and_visibility']['packet_visibility'] = 'local_only'
        self.native.refresh()
        self.binding = copy.deepcopy(self.native.binding)
        self.binding['packet_ref'] = PACKET_REF
        self.write_private(PACKET_REF, (self.public / self.native.packet_ref).read_bytes())
        (self.public / self.native.packet_ref).unlink()
        self.source = occurrence(self.binding)
        self.write_private(SOURCE_REF, self.encode(self.source))
        self.config_path = self.base / 'context.json'
        self.config = {'schema_version': 'tos_owner_local_source_context_v1',
            'store_id': STORE_ID, 'public_root': str(self.public),
            'private_root': str(self.private), 'private_prefix': PREFIX}
        self.config_path.write_bytes(self.encode(self.config))
        self.config_path.chmod(0o600)
        self.context = OwnerLocalSourceContext.load(self.config_path)

    @staticmethod
    def encode(value):
        return (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode('utf-8')

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

    @staticmethod
    def access(scope='metadata_only'):
        return {'read_scope': scope, 'access_allowed': True,
                'authority_ref': 'authority:synthetic-test-only'}

    def reader(self, *, exact=False, native=True, source_reader=None):
        return OwnerLocalSourceRecordProfiles(self.context,
            self.access('exact_owner_local' if exact else 'metadata_only'),
            self.binding if native else None, source_reader)

    def test_private_occurrence_uses_shared_grammar_and_preserves_unknown_fields(self):
        reader = self.reader()
        original = copy.deepcopy(self.source)
        self.assertIsNone(reader.validate('occurrence', self.source))
        self.assertEqual(reader.load('occurrence', SOURCE_REF), original)
        self.assertEqual(self.source, original)
        self.assertEqual((self.private / SOURCE_REF).read_bytes(), self.encode(original))
        self.assertEqual(reader.source_basenames['occurrence'], 'occurrence.json')
        self.assertNotIn('document', reader.profiles)
        self.assertTrue(all(profile['reader'] == 'semantic-metadata-v1'
                            for profile in reader.profiles.values()))
        self.assertEqual(reader.profiles['occurrence'], SourceRecordProfiles(self.public).profiles['occurrence'])
        self.assertRegex(reader.snapshot(), r'^sha256:[a-f0-9]{64}$')
        summary = reader.validate_native_binding('occurrence', self.source)
        self.assertTrue(summary['metadata_verified'])
        self.assertFalse(summary['content_verified'])
        self.assertFalse(summary['public_content_declared'])
        self.assertFalse(summary['assessment_applied'])

    def test_validate_and_load_never_read_content_even_with_exact_access(self):
        seen = []

        def read(path, limit):
            seen.append(path)
            return path.read_bytes()

        reader = self.reader(exact=True, source_reader=read)
        (self.public / self.native.content_ref).unlink()
        reader.validate('occurrence', self.source)
        reader.load('occurrence', SOURCE_REF)
        self.assertFalse(reader.validate_native_binding('occurrence', self.source)['content_verified'])
        self.assertNotIn(self.public / self.native.content_ref, seen)
        self.assertNotIn(self.public / self.native.original_ref, seen)
        with self.assertRaises(SourceProfileError):
            reader.validate_native_binding('occurrence', self.source, verify_content=True)

    def test_exact_verification_requires_distinct_call_and_preserves_source_bytes(self):
        reader = self.reader(exact=True)
        reader.validate('occurrence', self.source)
        metadata_snapshot = reader.snapshot()
        summary = reader.validate_native_binding('occurrence', self.source, verify_content=True)
        self.assertTrue(summary['content_verified'])
        self.assertFalse(summary['public_content_available'])
        self.assertFalse(summary['original_payload_verified'])
        self.assertEqual(summary['unit_kind'], 'surface_token')
        self.assertEqual(summary['native_status']['unit_boundary_posture'], 'method_proposed')
        self.assertEqual(summary['native_status']['segmentation_status'], 'proposed')
        self.assertNotEqual(metadata_snapshot, reader.snapshot())
        self.assertEqual((self.public / self.native.content_ref).read_bytes(), self.native.content)
        self.assertIn(b'\r\n', self.native.content)
        self.assertIn('cafe\u0301'.encode(), self.native.content)
        self.assertFalse((self.public / self.native.original_ref).exists())
        with self.assertRaises(SourceProfileError):
            self.reader().validate_native_binding('occurrence', self.source, verify_content=True)
        for value in (1, 'true', None):
            with self.subTest(value=value), self.assertRaises(SourceProfileError):
                self.reader(exact=True).validate_native_binding('occurrence', self.source, verify_content=value)

    def test_plain_semantic_profile_has_no_fabricated_native_binding(self):
        reader, source = self.reader(native=False), lexeme()
        ref = PREFIX + 'semantic/another/lexeme.json'
        self.write_private(ref, self.encode(source))
        self.assertIsNone(reader.validate_native_binding('lexeme', source))
        self.assertEqual(reader.load('lexeme', ref), source)
        for selected, body in (
            (self.reader(), source),
            (self.reader(native=False, exact=True), source),
            (self.reader(native=False), {**source, 'native_text_binding': self.binding}),
        ):
            with self.subTest(body=body['record_type']), self.assertRaises(SourceProfileError):
                selected.validate('lexeme', body)
        with self.assertRaises(SourceProfileError):
            self.reader(native=False).validate('occurrence', self.source)
        with self.assertRaises(SourceProfileError):
            self.reader(native=False).validate('work', self.native.read_json(self.native.refs['work']))

    def test_future_native_profile_is_not_silently_treated_as_occurrence(self):
        registry = self.native.read_json(REGISTRY_REF)
        entry = next(entry for entry in registry['types'] if entry['type_id'] == 'tos.entity.lexeme')
        entry['source_record_profile']['native_binding_adapter'] = 'source-text-unit-v1'
        self.native.write_json(REGISTRY_REF, registry)
        with self.assertRaisesRegex(SourceProfileError, 'separately understood adapter'):
            self.reader(native=False).validate('lexeme', lexeme())

    def test_complete_binding_is_typed_canonical_and_delegation_inputs_are_frozen(self):
        access, binding = self.access(), copy.deepcopy(self.binding)
        reader = OwnerLocalSourceRecordProfiles(self.context, access, binding)
        access.update(read_scope='exact_owner_local', access_allowed=False)
        binding['unit_version'] = 7
        reader.validate('occurrence', self.source)
        reordered = copy.deepcopy(self.source)
        reordered['native_text_binding'] = dict(reversed(list(self.binding.items())))
        reader.validate('occurrence', reordered)
        for key, value in (('unit_version', True), ('packet_version', 2),
                           ('ordered_anchor_refs', ['tos.anchor.other']),
                           ('packet_ref', self.native.packet_ref)):
            changed = copy.deepcopy(self.source)
            changed['native_text_binding'][key] = value
            with self.subTest(key=key), self.assertRaises(SourceProfileError):
                reader.validate('occurrence', changed)
        with self.assertRaises(SourceProfileError):
            reader.validate_native_binding('occurrence', self.source, verify_content=True)

    def test_context_is_not_a_read_grant(self):
        for access in ({}, {**self.access(), 'access_allowed': False},
                       {**self.access(), 'access_allowed': 1},
                       {**self.access(), 'read_scope': 'public'},
                       {**self.access(), 'authority_ref': ''},
                       {**self.access(), 'extra_permission': True}):
            with self.subTest(access=access), self.assertRaises(SourceProfileError):
                OwnerLocalSourceRecordProfiles(self.context, access, self.binding)
        with self.assertRaises(SourceProfileError):
            OwnerLocalSourceRecordProfiles(self.config, self.access(), self.binding)

    def test_private_ref_does_not_escape_or_choose_another_store(self):
        reader = self.reader()
        refs = [SOURCE_REF.replace(STORE_ID, 'sid-' + '2' * 32),
                'ToS/source-witnesses/semantic/synthetic/occurrence.json',
                PREFIX + 'occurrence.json', PREFIX + 'semantic/synthetic/lexeme.json',
                SOURCE_REF.replace('/semantic/', '/payload/'),
                SOURCE_REF.replace('/semantic/', '/local-content/'),
                SOURCE_REF.replace('/semantic/', '/catalog/'),
                SOURCE_REF.replace('/semantic/', '/.hidden/'),
                SOURCE_REF.replace('/semantic/', '/semantic/../'),
                SOURCE_REF.replace('/semantic/', '/semantic//'),
                str(self.private / SOURCE_REF), None]
        for ref in refs:
            with self.subTest(ref=ref), self.assertRaises(SourceProfileError):
                reader.validate_path('occurrence', ref)

    def test_default_public_reader_cannot_consume_private_record_or_export_its_ref(self):
        public = SourceRecordProfiles(self.public)
        with self.assertRaises(SourceProfileError):
            public.validate('occurrence', self.source)
        with self.assertRaises(SourceProfileError):
            public.validate_path('occurrence', SOURCE_REF)
        public_flag = {**self.source, 'visibility': 'public_metadata_only'}
        with self.assertRaises(SourceProfileError):
            public.validate('occurrence', public_flag)
        with self.assertRaises(SourceProfileError):
            public.catalog_entry('occurrence', public_flag, SOURCE_REF)
        reader = self.reader()
        self.assertNotIsInstance(reader, SourceRecordProfiles)
        for name in ('catalog_files', 'catalog_entry', 'verify_entry', 'export', 'native_resolver'):
            self.assertFalse(hasattr(reader, name), name)

    def test_public_native_inventory_rejects_reserved_directory_and_broken_alias_before_read(self):
        reserved = self.public / OWNER_LOCAL_HOME
        for alias in (False, True):
            with self.subTest(alias=alias):
                cached = SourceRecordProfiles(self.public)
                self.assertEqual(cached.native_semantic_identities(), {})
                if alias:
                    reserved.symlink_to(self.base / 'absent')
                else:
                    reserved.mkdir()
                try:
                    public = SourceRecordProfiles(self.public)
                    with patch('source_record_profiles._read_json', side_effect=AssertionError('private read')):
                        with self.assertRaises(SourceProfileError):
                            public.native_semantic_identities()
                        with self.assertRaises(SourceProfileError):
                            cached.native_semantic_identities()
                        with self.assertRaises(SourceProfileError):
                            cached.native_identity_snapshot()
                finally:
                    reserved.unlink() if alias else reserved.rmdir()

    def test_owner_reader_rejects_public_alias_and_never_uses_public_fallback(self):
        reader = self.reader()
        private = self.private / SOURCE_REF
        private.unlink()
        with self.assertRaises(SourceProfileError):
            reader.load('occurrence', SOURCE_REF)
        self.native.write_json(SOURCE_REF, self.source)
        with self.assertRaises(SourceProfileError):
            reader.load('occurrence', SOURCE_REF)
        with self.assertRaises(SourceProfileError):
            reader.snapshot()

    def test_unsafe_source_modes_and_symlinks_are_rejected_before_source_reader(self):
        target = self.private / SOURCE_REF
        seen = []

        def read(path, limit):
            seen.append(path)
            return path.read_bytes()

        for unsafe in ('file-mode', 'directory-mode', 'symlink'):
            with self.subTest(unsafe=unsafe):
                reader = self.reader(source_reader=read)
                if unsafe == 'file-mode':
                    target.chmod(0o644)
                elif unsafe == 'directory-mode':
                    target.parent.chmod(0o755)
                else:
                    target.unlink()
                    target.symlink_to(self.private / PACKET_REF)
                seen.clear()
                try:
                    with self.assertRaises(SourceProfileError):
                        reader.load('occurrence', SOURCE_REF)
                    self.assertNotIn(target, seen)
                finally:
                    if unsafe == 'symlink':
                        target.unlink()
                        self.write_private(SOURCE_REF, self.encode(self.source))
                    elif unsafe == 'file-mode':
                        target.chmod(0o600)
                    else:
                        target.parent.chmod(0o700)

    def test_snapshot_and_subsequent_validation_reject_exact_dependency_drift(self):
        for path in (self.config_path, self.public / CONTEXT_SCHEMA_REF,
                     self.public / REGISTRY_REF,
                     self.public / 'ToS/contracts/occurrence-description-record.schema.json',
                     self.private / PACKET_REF, self.public / self.native.layer_ref,
                     self.private / SOURCE_REF):
            with self.subTest(path=path.name):
                reader = self.reader()
                reader.load('occurrence', SOURCE_REF)
                original = path.read_bytes()
                path.write_bytes(original + b'\n')
                try:
                    with self.assertRaises(SourceProfileError):
                        reader.snapshot()
                    with self.assertRaises(SourceProfileError):
                        reader.validate('occurrence', self.source)
                finally:
                    path.write_bytes(original)

    def test_verified_content_remains_in_snapshot_and_is_not_refreshed(self):
        seen = []

        def read(path, limit):
            seen.append(path)
            return path.read_bytes()

        reader = self.reader(exact=True, source_reader=read)
        reader.validate_native_binding('occurrence', self.source, verify_content=True)
        target = self.public / self.native.content_ref
        target.write_bytes(self.native.content + b'!')
        seen.clear()
        reader.validate('occurrence', self.source)
        reader.load('occurrence', SOURCE_REF)
        self.assertFalse(reader.validate_native_binding('occurrence', self.source)['content_verified'])
        self.assertNotIn(target, seen)
        with self.assertRaises(SourceProfileError):
            reader.snapshot()
        self.assertIn(target, seen)
        with self.assertRaises(SourceProfileError):
            reader.validate_native_binding('occurrence', self.source, verify_content=True)

    def test_private_schema_lookalike_never_overrides_public_contract(self):
        ref = 'ToS/contracts/occurrence-description-record.schema.json'
        # Malicious synthetic lookalike, not a private schema installation.
        self.write_private(ref, b'{"type": "object"}\n')
        reader = self.reader()
        invalid = {**self.source, 'semantic_content': {}}
        with self.assertRaises(SourceProfileError):
            reader.validate('occurrence', invalid)
        reader.validate('occurrence', self.source)
        self.assertEqual(reader.input_digests[ref], digest((self.public / ref).read_bytes()))
        self.assertEqual(self.context.path(ref), self.public / ref)

    def test_properties_and_returned_records_do_not_mutate_reader_state(self):
        reader = self.reader()
        loaded = reader.load('occurrence', SOURCE_REF)
        before = reader.snapshot()
        loaded['semantic_content']['unknown_analysis']['zero'] = 9
        reader.profiles['occurrence']['native_binding_adapter'] = 'made-up'
        reader.registry['types'].clear()
        reader.input_digests.clear()
        reader.source_basenames.clear()
        self.assertEqual(reader.load('occurrence', SOURCE_REF), self.source)
        self.assertEqual(reader.snapshot(), before)
        self.assertNotIn(PACKET_REF, json.dumps(reader.input_digests))
        self.assertNotIn(SOURCE_REF, json.dumps(reader.input_digests))
        self.assertNotIn(self.native.content_ref, json.dumps(reader.input_digests))
        self.assertTrue(all(ref.startswith('ToS/contracts/') or ref == REGISTRY_REF
                            for ref in reader.input_digests))

    def test_visibility_invalid_schema_and_record_budgets_fail_closed(self):
        for visibility in ('public', 'public_metadata_only', 'research_group', 'permission_requested', None):
            with self.subTest(visibility=visibility), self.assertRaises(SourceProfileError):
                self.reader().validate('occurrence', {**self.source, 'visibility': visibility})
        for invalid in ({**self.source, 'record_version': True},
                        {**self.source, 'schema_version': 'unknown'},
                        {**self.source, 'semantic_content': {}},
                        {**self.source, 'extensions': {'bad': float('nan')}},
                        {**self.source, 'notes': 'x' * (MAX_RECORD_BYTES + 1)}):
            with self.subTest(kind=invalid['schema_version']), self.assertRaises(SourceProfileError):
                self.reader().validate('occurrence', invalid)
        self.write_private(SOURCE_REF, b' ' * (MAX_RECORD_BYTES + 1))
        with self.assertRaises(SourceProfileError):
            self.reader().load('occurrence', SOURCE_REF)
        with self.assertRaises(SourceProfileError):
            self.reader(source_reader=lambda path, limit: b'x' * (limit + 1))
        with patch('source_owner_record_profiles.MAX_SNAPSHOT_FILES', 1), self.assertRaises(SourceProfileError):
            self.reader()
        with patch('source_owner_record_profiles.MAX_SNAPSHOT_BYTES', 1), self.assertRaises(SourceProfileError):
            self.reader()

    def test_duplicate_json_fields_are_not_silently_rebound(self):
        raw = self.encode(self.source)
        raw = raw.replace(b'"record_version": 1,', b'"record_version": 1, "record_version": 2,')
        self.write_private(SOURCE_REF, raw)
        with self.assertRaises(SourceProfileError):
            self.reader().load('occurrence', SOURCE_REF)


if __name__ == '__main__':
    unittest.main()

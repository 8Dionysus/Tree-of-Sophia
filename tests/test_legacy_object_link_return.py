"""Retained legacy object-Link context, not source migration or admission."""
from __future__ import annotations

import copy
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
for directory in (ROOT / 'scripts', ROOT / 'access/src'):
    if str(directory) not in sys.path:
        sys.path.insert(0, str(directory))

from source_object_link_read import (
    LegacyObjectLinkReader, LegacyObjectLinkError, SOURCE_REF, SCHEMA_REF, canonical_digest,
)


class LegacyObjectLinkReturnTests(unittest.TestCase):
    @contextmanager
    def source_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for ref in (SCHEMA_REF, SOURCE_REF):
                (root / ref).parent.mkdir(parents=True, exist_ok=True)
                (root / ref).write_bytes((ROOT / ref).read_bytes())
            yield root

    def test_exact_legacy_rows_return_without_fabricated_fields_or_source_writes(self):
        with self.source_fixture() as root:
            before = (root / SOURCE_REF).read_bytes()
            reader = LegacyObjectLinkReader(root)
            rows = reader.rows()
            self.assertEqual(len(rows), 5)
            for line, claim in rows:
                with self.subTest(claim=claim['claim_id']):
                    entry = {'claim_id': claim['claim_id'], 'source_claim_file_ref': SOURCE_REF,
                        'source_claim_line': line, 'claim_sha256': canonical_digest(claim)}
                    self.assertEqual(reader.from_catalog(entry), claim)
                    self.assertEqual(reader.context(line, claim)['source_claim'], claim)
                    self.assertEqual(claim['reviews'], [])
                    self.assertFalse(claim['qualifiers']['availability_is_rights_conclusion'])
                    for field in ('statement', 'statement_language', 'assessment_refs', 'human_forms'):
                        self.assertNotIn(field, claim)
                    reader.from_catalog(entry)['maker']['agent_ref'] = 'no-mutation'
                    self.assertEqual(reader.from_catalog(entry), claim)
            reader.verify_current()
            self.assertEqual((root / SOURCE_REF).read_bytes(), before)

    def test_exact_path_line_digest_schema_and_endpoint_domains_fail_closed(self):
        with self.source_fixture() as root:
            reader = LegacyObjectLinkReader(root)
            line, claim = reader.rows()[0]
            entry = {'claim_id': claim['claim_id'], 'source_claim_file_ref': SOURCE_REF,
                'source_claim_line': line, 'claim_sha256': canonical_digest(claim)}
            for alteration in ({'source_claim_file_ref': SOURCE_REF.replace('object-link/', 'other/')},
                    {'source_claim_line': True}, {'source_claim_line': 999}, {'claim_id': 'tos.claim.other'},
                    {'claim_sha256': '0' * 64}, {'source_schema_ref': 'ToS/contracts/claim-packet.schema.json'}):
                with self.subTest(alteration=alteration), self.assertRaises(LegacyObjectLinkError):
                    reader.from_catalog({**entry, **alteration})
            objects = {claim['subject_ref']: {'record_type': 'work'}, claim['object']: {'record_type': 'link'}}
            reader.validate(claim, objects)
            for field, kind in (('subject_ref', 'artifact'), ('subject_ref', 'agent'), ('object', 'work')):
                altered = copy.deepcopy(objects)
                altered[claim[field]]['record_type'] = kind
                with self.subTest(field=field, kind=kind), self.assertRaises(LegacyObjectLinkError):
                    reader.validate(claim, altered)
            with self.assertRaises(LegacyObjectLinkError):
                reader.context(line, {**claim, 'claim_version': 2})
            (root / SOURCE_REF).write_bytes((root / SOURCE_REF).read_bytes() + b'\n')
            with self.assertRaises(LegacyObjectLinkError):
                reader.verify_current()

    def test_legacy_schema_scope_and_unknown_qualifiers_remain_distinct(self):
        with self.source_fixture() as root:
            reader = LegacyObjectLinkReader(root)
            _, claim = reader.rows()[0]
            extended = copy.deepcopy(claim)
            extended['qualifiers']['uninterpreted_source_limit'] = {'scope': None, 'explicit': False, 'gaps': []}
            (root / SOURCE_REF).write_text(json.dumps(extended) + '\n')
            reread = LegacyObjectLinkReader(root)
            self.assertEqual(reread.context(1, reread.rows()[0][1])['source_claim'], extended)
            for alteration in ({'subject_ref': 'tos.artifact.fixture'}, {'schema_version': 'tos_object_link_claim_v2'},
                    {'review_status': 'accepted'}, {'reviews': [{'invented': True}]},
                    {'visibility': 'private'}, {'statement': 'Fabricated wording.'}, {'predicate': []}):
                with self.subTest(alteration=alteration), self.assertRaises(LegacyObjectLinkError):
                    reader.validate({**claim, **alteration})

    def test_duplicate_identity_duplicate_json_key_and_symlink_are_not_legacy_sources(self):
        with self.source_fixture() as root:
            first = (root / SOURCE_REF).read_bytes().splitlines()[0]
            for raw in (first + b'\n' + first + b'\n', first[:-1] + b',"reviews":[]}\n'):
                (root / SOURCE_REF).write_bytes(raw)
                with self.assertRaises(LegacyObjectLinkError):
                    LegacyObjectLinkReader(root)
            path = root / SOURCE_REF
            retained = path.with_name('retained.jsonl')
            path.rename(retained)
            path.symlink_to(retained.name)
            with self.assertRaises(LegacyObjectLinkError):
                LegacyObjectLinkReader(root)

    def test_undeclared_schema_dependency_is_rejected_without_network_retrieval(self):
        with self.source_fixture() as root:
            path = root / SCHEMA_REF
            schema = json.loads(path.read_bytes())
            schema['allOf'] = [{'$ref': 'https://unregistered.invalid/schema.json'}]
            path.write_text(json.dumps(schema))
            with patch('urllib.request.urlopen', side_effect=AssertionError('network retrieval')):
                with self.assertRaisesRegex(LegacyObjectLinkError, 'undeclared dependency'):
                    LegacyObjectLinkReader(root)

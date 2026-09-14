"""Current addressed source slots, with original bytes and native line numbers."""
from dataclasses import replace
import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
for directory in (ROOT / 'scripts', ROOT / 'access/src', ROOT / 'tests',
                  ROOT / 'mechanics/growth-cycle/tests'):
    sys.path.insert(0, str(directory))

import test_source_catalog_projection as fixtures
import source_catalog_projection as catalog
import build_source_witness_catalog as legacy
from tos_access.projection_store import ProjectionReader, canonical_bytes
from tos_access.projection_mutation import ProjectionSnapshotView, ProjectionChange, stage_projection_snapshot_changes


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


class SourceCatalogSlotTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.SourceCatalogProjectionTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.claim_ref = 'ToS/source-witnesses/relations/slot-fixture/responsibility-claims.jsonl'
        self.event_ref = 'ToS/source-witnesses/relations/slot-fixture/provenance.jsonl'
        self.anchor_ref = 'ToS/source-witnesses/relations/slot-fixture/anchors.jsonl'
        self.claim = {'schema_version': 'tos_claim_packet_v1', 'claim_id': 'tos.claim.slot-fixture',
            'claim_type': 'bibliographic', 'assertion_layer': 'bibliographic_assertion',
            'subject_ref': self.fixture.identity, 'predicate': 'authored_by',
            'object': self.fixture.other['record_id'], 'evidence_refs': ['tos.anchor.slot-fixture'],
            'maker': {'maker_type': 'agent', 'agent_ref': 'software:synthetic'},
            'provenance_event_ref': 'tos.provenance.slot-fixture',
            'epistemic_status': 'documented', 'review_status': 'unreviewed',
            'visibility': 'public_metadata_only', 'reviews': [], 'claim_version': 1,
            'uninterpreted': {'empty': '', 'zero': 0, 'false': False, 'null': None, 'unicode': 'Ω'}}
        self.other_claim = {**copy.deepcopy(self.claim), 'claim_id': 'tos.claim.slot-fixture-other'}
        self.event = {'event_id': 'tos.provenance.slot-fixture', 'event_version': 1,
                      'unknown': [None, False, 'source only']}
        self.anchor = {'anchor_id': 'tos.anchor.slot-fixture', 'unknown': {'literal': 'tos.agent.not-a-lookup'}}
        self.claim_raw = json.dumps(self.claim, ensure_ascii=False, indent=None).encode()
        self.other_raw = canonical_bytes(self.other_claim).rstrip(b'\n')
        self.fixture.write(self.claim_ref, b'\n' + self.claim_raw + b'\r\n \t\r\n' + self.other_raw)
        self.fixture.write(self.event_ref, b'\r\n' + canonical_bytes(self.event))
        self.fixture.write(self.anchor_ref, canonical_bytes(self.anchor))
        self.fixture.rebuild()

    def bootstrap(self, **kwargs):
        return self.fixture.bootstrap(include_claims=True, **kwargs)

    def snapshot(self, candidate):
        return self.fixture.reader(candidate)

    def reader(self, candidate, **kwargs):
        return catalog.SourceCatalogSourceReader(self.root, catalog_snapshot=self.snapshot(candidate), **kwargs)

    def read_claim(self, candidate, identity=None, **kwargs):
        identity = identity or self.claim['claim_id']
        reader = self.reader(candidate, **kwargs)
        row = reader.catalog_snapshot.get_claim(identity)
        return reader.read_claim(identity, expected_row_sha256=row.row_sha256)

    def test_bootstrap_produces_exact_claim_entries_and_typed_original_source_slots(self):
        candidate = self.bootstrap()
        snapshot = self.snapshot(candidate)
        material = candidate.snapshot().materialize()
        self.assertEqual(set(material) & {'records', 'claims', 'source_slots'}, {'records', 'claims', 'source_slots'})
        self.assertTrue(snapshot.header['claims_addressed'])
        self.assertEqual(snapshot.header['claim_count'], 2)
        self.assertEqual(snapshot.header['source_slot_count'], 4)
        self.assertEqual([row['entry'] for row in material['claims']], legacy.collect_claims(self.root))
        selected = snapshot.get_claim(self.claim['claim_id'])
        slot = snapshot.get_slot('claim', self.claim['claim_id'])
        self.assertEqual(selected.source_slot_key, slot.source_slot_key)
        self.assertEqual(slot.source['source_line'], 2)
        self.assertEqual(slot.source['byte_offset'], 1)
        self.assertEqual(slot.source['raw_row_sha256'], sha(self.claim_raw))
        self.assertEqual(slot.source['file_sha256'], sha((self.root / self.claim_ref).read_bytes()))
        self.assertEqual(slot.source['delimiter'], 'crlf')
        self.assertEqual(snapshot.get_slot('claim', self.other_claim['claim_id']).source['source_line'], 4)
        self.assertEqual(snapshot.get_slot('claim', self.other_claim['claim_id']).source['delimiter'], 'eof')
        self.assertEqual(snapshot.get_slot('provenance_event', self.event['event_id']).source['source_line'], 2)
        for address in (selected.provenance, slot.provenance):
            self.assertNotIn('root_sha256', address)
            self.assertNotIn('publication_token', address)
        self.assertFalse(self.fixture.output.exists())
        self.assertFalse(candidate.published)

    def test_range_reader_never_loads_full_jsonl_and_preserves_unknown_payload(self):
        candidate = self.bootstrap()
        snapshot = self.snapshot(candidate)
        selected = snapshot.get_claim(self.claim['claim_id'])
        calls = []
        real = catalog.os.pread
        def read(fd, length, offset):
            calls.append((length, offset))
            return real(fd, length, offset)
        with patch.object(Path, 'rglob', side_effect=AssertionError('source scan')), \
                patch.object(Path, 'read_bytes', side_effect=AssertionError('whole-file read')), \
                patch.object(ProjectionReader, 'iter_items', side_effect=AssertionError('projection scan')), \
                patch.object(catalog.os, 'pread', side_effect=read):
            reader = catalog.SourceCatalogSourceReader(self.root, catalog_snapshot=snapshot)
            result = reader.read_claim(self.claim['claim_id'], expected_row_sha256=selected.row_sha256)
            event = reader.read_slot('provenance_event', self.event['event_id'])
            anchor = reader.read_slot('anchor', self.anchor['anchor_id'])
            reader.verify_current()
        self.assertEqual(result.payload, self.claim)
        self.assertEqual(result.raw_bytes, self.claim_raw)
        self.assertEqual(event.payload, self.event)
        self.assertEqual(anchor.payload, self.anchor)
        self.assertEqual(calls[0], (len(self.claim_raw) + 3, 0))
        self.assertFalse(result.whole_file_rehashed)
        self.assertFalse(result.historical_claim_verified)
        self.assertEqual(reader.accounting['whole_files_rehashed'], 0)
        changed = result.payload
        changed['uninterpreted'].clear()
        self.assertEqual(result.payload, self.claim)

    def test_slot_keys_are_typed_reversible_and_records_only_requires_explicit_bootstrap(self):
        values = [('anchor', 'id:with:delimiter'), ('provenance_event', 'id:with:delimiter'),
                  ('anchor', 'id","anchor"'), ('anchor', 'id/Ω')]
        keys = [catalog._slot_key(*value) for value in values]
        self.assertEqual(len(set(keys)), len(values))
        self.assertEqual([json.loads(key) for key in keys], [list(value) for value in values])
        baseline = self.fixture.bootstrap()
        reader = self.snapshot(baseline)
        self.assertEqual(reader.get(self.fixture.identity).entry['record_id'], self.fixture.identity)
        with self.assertRaises(catalog.SourceCatalogRequiresBootstrap):
            reader.lookup_claim(self.claim['claim_id'])
        with self.assertRaises(catalog.SourceCatalogRequiresBootstrap):
            catalog.SourceCatalogSourceReader(self.root, catalog_snapshot=reader)

    def test_bootstrap_rejects_duplicate_slots_and_event_membership_drift(self):
        duplicate = 'ToS/source-witnesses/relations/slot-fixture/other-provenance.jsonl'
        self.fixture.write(duplicate, canonical_bytes(self.event))
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'duplicate typed'):
            self.bootstrap()
        (self.root / duplicate).unlink()
        real = catalog._source_slot_rows
        def changed(*args, **kwargs):
            result = real(*args, **kwargs)
            self.fixture.write(duplicate, canonical_bytes({**self.event, 'event_id': 'tos.provenance.extra'}))
            return result
        with patch.object(catalog, '_source_slot_rows', side_effect=changed), patch.object(catalog, '_install') as install:
            with self.assertRaisesRegex(catalog.SourceCatalogError, 'membership changed'):
                self.bootstrap()
            install.assert_not_called()

    def test_range_reader_refuses_wrong_digest_missing_source_symlink_and_observed_drift(self):
        candidate = self.bootstrap()
        reader = self.reader(candidate)
        row = reader.catalog_snapshot.get_claim(self.claim['claim_id'])
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'row digest differs'):
            reader.read_claim(self.claim['claim_id'], expected_row_sha256='0' * 64)
        reader.read_claim(self.claim['claim_id'], expected_row_sha256=row.row_sha256)
        target = self.root / self.claim_ref
        original = target.read_bytes()
        target.write_bytes(original.replace(b'source only', b'SOURCE only'))
        with self.assertRaises(catalog.source.JournalConflict):
            reader.verify_current()
        target.unlink()
        with self.assertRaises(FileNotFoundError):
            self.read_claim(candidate)
        retained = target.with_name('retained.jsonl')
        retained.write_bytes(original)
        target.symlink_to(retained)
        with self.assertRaises(OSError):
            self.read_claim(candidate)

    def test_range_reader_refuses_selected_raw_mutation_and_all_source_budgets(self):
        candidate = self.bootstrap()
        for values in ({'max_source_files': 0}, {'max_read_slots': 0}, {'max_read_bytes': 0}, {'max_row_bytes': 0},
                       {'max_profile_bytes': 0}, {'max_profile_files': 0}):
            with self.subTest(values=values), patch.object(catalog.os, 'pread') as read:
                with self.assertRaises(catalog.SourceCatalogBudgetExceeded):
                    self.read_claim(candidate, limits=replace(catalog.SourceSlotLimits(), **values))
                read.assert_not_called()
        target = self.root / self.claim_ref
        original = target.read_bytes()
        target.write_bytes(original.replace(b'"zero": 0', b'"zero": 1', 1))
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'raw digest differs'):
            self.read_claim(candidate)

    def test_selected_agent_transition_preserves_claim_and_slot_descriptors_and_local_addresses(self):
        before = self.bootstrap()
        original = self.snapshot(before)
        claim = original.get_claim(self.claim['claim_id'])
        slot = original.get_slot('claim', self.claim['claim_id'])
        transaction, token = self.fixture.revise()
        with patch.object(legacy, 'collect_claims', side_effect=AssertionError('whole Claim collection')):
            after = self.fixture.transition(original, transaction, token)
        successor = self.snapshot(after)
        for collection in ('claims', 'source_slots'):
            self.assertEqual(json.loads(before.root_bytes)['collections'][collection],
                             json.loads(after.root_bytes)['collections'][collection])
        self.assertEqual(claim.row_bytes, successor.get_claim(self.claim['claim_id']).row_bytes)
        self.assertEqual(slot.provenance, successor.get_slot('claim', self.claim['claim_id']).provenance)
        self.assertEqual(self.read_claim(after).payload, self.claim)
        with self.assertRaises(catalog.source.JournalConflict):
            self.reader(before)

    def test_pure_claim_renderer_is_full_collector_parity_and_does_no_io(self):
        with patch.object(Path, 'open', side_effect=AssertionError('pure Claim renderer opened a file')):
            row = legacy.render_claim_catalog_entry(self.claim, self.claim_ref, 2)
        self.assertEqual(row, next(row for row in legacy.collect_claims(self.root)
                                   if row['claim_id'] == self.claim['claim_id']))

    def test_original_line_and_range_parity_for_cr_lf_crlf_blank_and_eof(self):
        target = self.root / self.claim_ref
        for separator in (b'\n', b'\r\n', b'\r'):
            for final in (separator, b''):
                with self.subTest(separator=separator, final=final):
                    target.write_bytes(separator + self.claim_raw + separator + b' ' + separator + self.other_raw + final)
                    self.fixture.rebuild()
                    candidate = self.bootstrap()
                    for identity, expected_line in ((self.claim['claim_id'], 2), (self.other_claim['claim_id'], 4)):
                        result = self.read_claim(candidate, identity)
                        self.assertEqual(result.slot.source['source_line'], expected_line)
                        self.assertEqual(result.payload['claim_id'], identity)

    def test_malformed_range_and_profile_drift_fail_without_scanning(self):
        candidate = self.bootstrap()
        target = self.root / self.claim_ref
        original = target.read_bytes()
        target.write_bytes(b'x' + original[1:])
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'boundary'):
            self.read_claim(candidate)
        target.write_bytes(original)
        snapshot = self.snapshot(candidate)
        slot = snapshot.get_slot('claim', self.claim['claim_id'])
        value = json.loads(slot.row_bytes)
        value['source']['byte_offset'] = value['source']['file_bytes']
        changed = stage_projection_snapshot_changes(candidate.snapshot(), expected_before_sha256=candidate.root_sha256,
            trusted_baseline_sha256=candidate.root_sha256,
            changes=[ProjectionChange('source_slots', slot.source_slot_key, True, slot.row_sha256, True, value)])
        broken = catalog.SourceCatalogSnapshot(changed.snapshot(), expected_root_sha256=changed.after_sha256,
                                               trusted_baseline_sha256=changed.after_sha256)
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'range'):
            broken.get_slot('claim', self.claim['claim_id'])
        schema = self.root / catalog.CORPUS_REF
        schema.write_bytes(schema.read_bytes() + b'\n')
        with self.assertRaises(catalog.SourceCatalogRequiresBootstrap):
            self.reader(candidate)

    def test_declared_nonpublic_provenance_slot_and_bootstrap_count_budget_refuse(self):
        with patch.object(catalog, '_install') as install:
            with self.assertRaises(catalog.SourceCatalogBudgetExceeded):
                self.bootstrap(limits=replace(catalog.CatalogLimits(), max_source_slots=0))
            install.assert_not_called()
        self.fixture.write(self.event_ref, canonical_bytes({**self.event,
            'schema_version': 'tos_provenance_event_v2',
            'rights_and_visibility': {'content_visibility': 'owner_local'}}))
        with patch.object(catalog, '_install') as install:
            with self.assertRaisesRegex(catalog.SourceCatalogError, 'not public metadata'):
                self.bootstrap()
            install.assert_not_called()


if __name__ == '__main__':
    unittest.main()

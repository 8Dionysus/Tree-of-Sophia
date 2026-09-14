"""Real source-command packages through addressed catalog addition, synthetic facts."""
from dataclasses import replace
import copy
import hashlib
import json
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
for directory in ('scripts', 'access/src', 'tests', 'mechanics/growth-cycle/tests'):
    sys.path.insert(0, str(ROOT / directory))

import test_bibliographic_claim_assembler as fixtures
import bibliographic_claim_assembler as assembly
import source_catalog_projection as catalog
import source_claim_catalog as addition
import source_commands as commands
import build_source_witness_catalog as legacy
from tos_access.projection_store import ProjectionReader, canonical_bytes
from tos_access.projection_mutation import MutationLimits, ProjectionMutationError


class SourceClaimCatalogTests(unittest.TestCase):
    def setUp(self):
        fixture = fixtures.BibliographicClaimAssemblerTests()
        self.addCleanup(fixture.doCleanups)
        self.helper, self.claim, _ = fixture.native_claim_fixture()
        self.root = self.helper.root
        self.evidence = 'ToS/review-ledger/synthetic-addition.md'
        self.helper.fixture.write(self.evidence, b'Synthetic test evidence, not history.\n')
        ref = 'ToS/contracts/provenance-event-v2.schema.json'
        self.helper.fixture.write(ref, (ROOT / ref).read_bytes())
        self.helper.fixture.rebuild()
        self.before = self.helper.snapshot(self.helper.bootstrap())
        self.claim.update(claim_id='tos.claim.synthetic-catalog-addition',
            evidence_refs=[self.evidence], counterevidence_refs=[], alternative_claim_refs=[],
            provenance_event_ref='tos.event.synthetic-catalog-addition',
            maker={'maker_type': 'software', 'agent_ref': 'software:synthetic'})
        self.relative = 'ToS/source-witnesses/relations/synthetic-catalog-addition/source-claims.jsonl'
        self.owner = self.root / 'claim-addition-owner.json'
        self.config = {'schema_version': commands.CLAIM_CONFIG, 'uid': os.getuid(),
            'principal_id': 'software:synthetic', 'maker_type': 'software', 'source_root': str(self.root),
            'source_path': self.relative, 'authority_ref': 'test:explicit-source-addition',
            'expires_at': '2099-01-01T00:00:00Z', 'provenance_event_id': self.claim['provenance_event_ref'],
            'allowed_operations': ['claims.create'], 'allowed_claim_ids': [self.claim['claim_id']],
            'allowed_subject_refs': [self.claim['subject_ref']], 'allowed_object_refs': [self.claim['object']],
            'allowed_predicates': [self.claim['predicate']], 'allowed_evidence_refs': [self.evidence]}
        self.owner.write_bytes(canonical_bytes(self.config))
        prepared = commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare-create', 'claims': [self.claim]})
        self.request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'claims.create',
            'command_id': 'synthetic-addressed-claim-addition', 'claims': [self.claim],
            'expected_revision': None, 'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'], 'expected_inputs': prepared['source_bindings']}
        result = commands.run_local_command(self.owner, self.request)
        self.receipt_path = (self.root / self.relative).with_name('source-create-receipt.json')
        self.expected = {'expected_receipt_sha256': hashlib.sha256(self.receipt_path.read_bytes()).hexdigest(),
                         'expected_request_digest': result['receipt']['request_digest']}

    def stage(self, before=None, **kwargs):
        return addition.claim_catalog_addition(self.owner, before or self.before, **self.expected,
                                               target_part_bytes=512, **kwargs)

    def test_addition_is_addressed_detached_and_matches_full_source_collector(self):
        source_bytes = (self.root / self.relative).read_bytes()
        old_root = self.before.view.root_bytes
        with patch.object(Path, 'rglob', side_effect=AssertionError('global source scan')), \
                patch.object(ProjectionReader, 'iter_items', side_effect=AssertionError('full catalog scan')), \
                patch.object(legacy, 'collect_claims', side_effect=AssertionError('whole Claim collection')), \
                patch.object(commands, 'run_local_command', side_effect=AssertionError('observer executed source command')):
            with self.stage() as operation:
                candidate = operation.candidate
                after = self.helper.snapshot(candidate)
                new = after.get_claim(self.claim['claim_id'])
                assembled = assembly.BibliographicClaimAssembler(self.root, catalog_snapshot=after)
                result = assembled.assemble(self.claim['claim_id'], expected_row_sha256=new.row_sha256)
                self.assertEqual(result.inputs.source_claim, self.claim)
                self.assertEqual(result.inputs.source_claim['extensions'], self.claim['extensions'])
                assembled.verify_current()
                operation.verify_current()
                self.assertEqual(after.get(self.claim['subject_ref']), self.before.get(self.claim['subject_ref']))
                self.assertEqual(after.header['claim_count'], self.before.header['claim_count'] + 1)
                self.assertEqual(after.header['source_slot_count'], self.before.header['source_slot_count'] + 2)
                self.assertFalse(candidate.verification['prepared_reader_updated'])
        with self.assertRaisesRegex(ValueError, 'closed'):
            operation.verify_current()
        expected = next(row for row in legacy.collect_claims(self.root) if row['claim_id'] == self.claim['claim_id'])
        self.assertEqual(new.entry, expected)
        self.assertEqual(self.before.view.root_bytes, old_root)
        self.assertEqual((self.root / self.relative).read_bytes(), source_bytes)
        self.assertIsNone(self.before.lookup_claim(self.claim['claim_id']))
        self.assertFalse(candidate.published)
        with self.assertRaisesRegex(ValueError, 'already belongs'):
            with self.stage(after):
                self.fail('duplicate identity accepted')

    def test_changed_endpoint_or_evidence_cannot_reuse_creation_receipt(self):
        for ref in (self.evidence, self.request['expected_inputs']['objects'][self.claim['subject_ref']]['source_ref']):
            path = self.root / ref
            original = path.read_bytes()
            try:
                path.write_bytes(original + b' ')
                with self.subTest(ref=ref), self.assertRaisesRegex(ValueError, 'bytes differ|version differs'):
                    with self.stage():
                        self.fail('stale source accepted')
            finally:
                path.write_bytes(original)

    def test_scope_revocation_and_late_source_drift_refuse_without_source_rollback(self):
        with self.stage() as operation:
            self.owner.write_bytes(canonical_bytes({**self.config, 'allowed_operations': []}))
            with self.assertRaises(PermissionError):
                operation.verify_current()
        self.owner.write_bytes(canonical_bytes(self.config))
        with self.stage() as operation:
            (self.root / self.evidence).write_bytes(b'Changed synthetic evidence.\n')
            with self.assertRaises(ValueError):
                operation.verify_current()
        self.assertTrue(self.receipt_path.exists())
        self.assertTrue((self.root / self.relative).exists())

    def test_receipt_corruption_symlink_and_projection_budget_refuse(self):
        with self.assertRaisesRegex(ValueError, 'count budget'):
            with self.stage(limits=replace(catalog.CatalogLimits(), max_claims=0)):
                self.fail('Claim count budget ignored')
        with self.assertRaises(ProjectionMutationError):
            with self.stage(mutation_limits=replace(MutationLimits(), max_written_parts=0)):
                self.fail('write budget ignored')
        original = self.receipt_path.read_bytes()
        self.receipt_path.write_bytes(original + b' ')
        with self.assertRaisesRegex(ValueError, 'receipt bytes differ'):
            with self.stage():
                self.fail('changed receipt accepted')
        self.receipt_path.write_bytes(original)
        evidence = self.root / self.evidence
        retained = evidence.with_name('retained-synthetic.md')
        evidence.rename(retained)
        evidence.symlink_to(retained.name)
        with self.assertRaises((ValueError, OSError)):
            with self.stage():
                self.fail('symlink evidence accepted')


if __name__ == '__main__':
    unittest.main()

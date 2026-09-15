"""Focused detached-catalog checks for one initial identity Claim package."""
import copy
from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
for directory in (
    'scripts',
    'access/src',
    'access/tests',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts',
):
    sys.path.insert(0, str(ROOT / directory))

import source_agent_publication_fixture as fixtures
import source_catalog_projection as catalog
import source_claim_catalog as addition
import source_claim_commands as claims
import source_commands as commands
from tos_access.projection_store import canonical_bytes


class SourceClaimCatalogTests(unittest.TestCase):
    def setUp(self):
        self.base = fixtures.SourceAgentPublicationTests()
        self.base.setUp()
        self.addCleanup(self.base.doCleanups)
        self.root, self.before = self.base.root, self.base.snapshot

        provenance_ref = 'ToS/contracts/provenance-event-v2.schema.json'
        self.base.helper.fixture.write(provenance_ref, (ROOT / provenance_ref).read_bytes())
        self.claim = copy.deepcopy(self.base.claim)
        self.claim.update(
            claim_id='tos.claim.synthetic-expiry-catalog',
            evidence_refs=['ToS/source-witnesses/agents/friedrich-nietzsche/agent.json'],
            provenance_event_ref='tos.event.synthetic-expiry-catalog',
        )
        self.relative = 'ToS/source-witnesses/relations/synthetic-expiry-catalog/source-claims.jsonl'
        self.owner = self.root / 'claim-catalog-owner.json'
        self.config = {
            'schema_version': commands.CLAIM_CONFIG,
            'uid': os.getuid(),
            'principal_id': 'software:synthetic',
            'maker_type': 'software',
            'source_root': str(self.root),
            'source_path': self.relative,
            'authority_ref': 'test:explicit-claim-catalog',
            'expires_at': '2099-01-01T00:00:00Z',
            'provenance_event_id': self.claim['provenance_event_ref'],
            'allowed_operations': ['claims.create'],
            'allowed_claim_ids': [self.claim['claim_id']],
            'allowed_subject_refs': [self.claim['subject_ref']],
            'allowed_object_refs': [self.claim['object']],
            'allowed_predicates': [self.claim['predicate']],
            'allowed_evidence_refs': self.claim['evidence_refs'],
        }
        self.owner.write_bytes(canonical_bytes(self.config))
        request = {
            'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare-create',
            'claims': [self.claim],
        }
        preview = commands.run_local_command(self.owner, request)
        request.update(
            operation='claims.create',
            command_id='synthetic-expiry-catalog',
            expected_configuration=preview['owner_configuration'],
            expected_dependencies=preview['expected_dependencies'],
            expected_inputs=preview['source_bindings'],
            expected_revision=None,
        )
        self.creation_request = copy.deepcopy(request)
        created = commands.run_local_command(self.owner, request)
        receipt = (self.root / self.relative).with_name('source-create-receipt.json')
        self.expected = {
            'expected_receipt_sha256': hashlib.sha256(receipt.read_bytes()).hexdigest(),
            'expected_request_digest': created['receipt']['request_digest'],
        }

    def receipt(self):
        path = (self.root / self.relative).with_name('source-create-receipt.json')
        return commands._json_object(path.read_bytes())

    def stage(self, before=None, **kwargs):
        return addition.claim_catalog_addition(
            self.owner,
            before or self.before,
            **self.expected,
            target_part_bytes=512,
            **kwargs,
        )

    def test_natural_expiry_does_not_block_committed_claim_stage_but_blocks_claim_create(self):
        owner_bytes = self.owner.read_bytes()
        receipt_path = (self.root / self.relative).with_name('source-create-receipt.json')
        receipt_bytes = receipt_path.read_bytes()
        future = datetime(2100, 1, 1, tzinfo=timezone.utc)

        # The detached catalog reader evaluates this already-committed package
        # at its receipt instant, while the writer wrapper still evaluates now.
        with patch.object(claims, 'datetime') as stage_clock:
            stage_clock.now.return_value = future
            with self.stage() as operation:
                operation.verify_current()

        with patch.object(claims, 'datetime') as command_clock:
            command_clock.now.return_value = future
            with self.assertRaisesRegex(PermissionError, 'invalid or expired'):
                commands.run_local_command(self.owner, self.creation_request)

        self.assertEqual(self.owner.read_bytes(), owner_bytes)
        self.assertEqual(receipt_path.read_bytes(), receipt_bytes)

    def test_committed_claim_creation_rejects_expired_future_or_mismatched_receipts(self):
        receipt = self.receipt()
        with patch.object(claims, 'datetime') as clock:
            clock.now.return_value = datetime(2101, 1, 1, tzinfo=timezone.utc)
            after_expiry = copy.deepcopy(receipt)
            after_expiry['recorded_at'] = '2099-01-02T00:00:00Z'
            with self.assertRaisesRegex(PermissionError, 'invalid or expired'):
                commands.inspect_committed_creation_owner(self.owner, after_expiry)

            clock.now.return_value = datetime(2090, 1, 1, tzinfo=timezone.utc)
            future = copy.deepcopy(receipt)
            future['recorded_at'] = '2090-01-01T00:00:01Z'
            with self.assertRaisesRegex(PermissionError, 'future'):
                commands.inspect_committed_creation_owner(self.owner, future)

            clock.now.return_value = datetime(2100, 1, 1, tzinfo=timezone.utc)
            bad_configuration = copy.deepcopy(receipt)
            bad_configuration['owner_configuration'] = 'sha256:' + '0' * 64
            with self.assertRaisesRegex(PermissionError, 'authority evidence differs'):
                commands.inspect_committed_creation_owner(self.owner, bad_configuration)

            bad_principal = copy.deepcopy(receipt)
            bad_principal['principal_id'] = 'software:other'
            with self.assertRaisesRegex(PermissionError, 'authority evidence differs'):
                commands.inspect_committed_creation_owner(self.owner, bad_principal)

    def test_owner_scope_drift_is_detected_after_detached_staging(self):
        with self.stage() as operation:
            self.owner.write_bytes(canonical_bytes({**self.config, 'allowed_operations': []}))
            with self.assertRaises(PermissionError):
                operation.verify_current()
        self.owner.write_bytes(canonical_bytes(self.config))


if __name__ == '__main__':
    unittest.main()

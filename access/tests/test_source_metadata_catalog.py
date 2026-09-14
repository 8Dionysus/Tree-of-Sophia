"""Focused detached-catalog checks for one initial metadata package."""
import copy
import hashlib
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
for directory in ('scripts', 'access/src', 'tests', 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'):
    sys.path.insert(0, str(ROOT / directory))

import test_source_agent_publication as fixtures
import source_catalog_projection as catalog
import source_commands as commands
import source_metadata_catalog as addition
from tos_access.projection_mutation import MutationLimits
from tos_access.projection_store import ProjectionReader, canonical_bytes


class SourceMetadataCatalogTests(unittest.TestCase):
    def setUp(self):
        self.base = fixtures.SourceAgentPublicationTests()
        self.base.setUp()
        self.addCleanup(self.base.doCleanups)
        self.root, self.before = self.base.root, self.base.snapshot
        record = copy.deepcopy(self.base.helper.fixture.record)
        record.update(
            record_id='tos.agent.synthetic-metadata-catalog',
            record_version=1,
            identity_status='provisional',
            same_as_posture='no_equivalence_claim',
            preferred_label='Synthetic metadata catalog record',
            notes='Synthetic metadata, not a historical person.',
            external_identifiers=[],
            source_refs=['https://example.invalid/synthetic-metadata-catalog'],
        )
        record.pop('variant_labels', None)
        record.pop('supersedes_ref', None)
        self.relative = 'ToS/source-witnesses/agents/synthetic-metadata-catalog/agent.json'
        self.owner = self.root / 'metadata-catalog-owner.json'
        self.config = {
            'schema_version': commands.CORPUS_CONFIG,
            'uid': os.getuid(),
            'principal_id': 'software:synthetic',
            'maker_type': 'software',
            'source_root': str(self.root),
            'source_path': self.relative,
            'record_id': record['record_id'],
            'record_type': 'agent',
            'authority_ref': 'test:explicit-metadata-catalog',
            'expires_at': '2099-01-01T00:00:00Z',
            'provenance_event_id': 'tos.event.synthetic-metadata-catalog',
            'allowed_operations': ['source.create'],
            'allowed_form_ids': ['tos.form.synthetic-metadata-catalog.name'],
        }
        provenance_ref = 'ToS/contracts/provenance-event-v2.schema.json'
        self.base.helper.fixture.write(provenance_ref, (ROOT / provenance_ref).read_bytes())
        self.base.helper.fixture.rebuild()
        self.owner.write_bytes(canonical_bytes(self.config))
        request = {
            'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare-create',
            'record': record,
            'forms': [{'field_id': 'metadata.preferred-name',
                       'form_id': self.config['allowed_form_ids'][0]}],
        }
        preview = commands.run_local_command(self.owner, request)
        request.update(
            operation='source.create',
            command_id='synthetic-metadata-catalog',
            expected_configuration=preview['owner_configuration'],
            expected_dependencies=preview['expected_dependencies'],
            expected_revision=None,
            expected_source=None,
        )
        created = commands.run_local_command(self.owner, request)
        receipt = (self.root / self.relative).with_name('source-create-receipt.json')
        self.expected = {
            'expected_receipt_sha256': hashlib.sha256(receipt.read_bytes()).hexdigest(),
            'expected_request_digest': created['receipt']['request_digest'],
        }

    def stage(self, before=None, **kwargs):
        return addition.metadata_catalog_addition(
            self.owner,
            before or self.before,
            **self.expected,
            target_part_bytes=512,
            **kwargs,
        )

    def test_stages_one_record_and_provenance_without_source_command_or_scan(self):
        source_bytes = (self.root / self.relative).read_bytes()
        with patch.object(Path, 'rglob', side_effect=AssertionError('global source scan')), \
                patch.object(ProjectionReader, 'iter_items', side_effect=AssertionError('catalog scan')), \
                patch.object(commands, 'run_local_command', side_effect=AssertionError('source command')):
            with self.stage() as operation:
                after = catalog.SourceCatalogSnapshot(
                    operation.candidate.snapshot(),
                    expected_root_sha256=operation.candidate.root_sha256,
                    trusted_baseline_sha256=operation.candidate.root_sha256,
                    limits=MutationLimits(),
                )
                row = after.get(self.config['record_id'])
                self.assertEqual(row.source['source_ref'], self.relative)
                self.assertIsNone(self.before.lookup(self.config['record_id']))
                self.assertEqual(after.header['record_count'], self.before.header['record_count'] + 1)
                self.assertEqual(after.header['claim_count'], self.before.header['claim_count'])
                self.assertEqual(after.header['source_slot_count'], self.before.header['source_slot_count'] + 1)
                self.assertFalse(operation.candidate.verification['prepared_reader_updated'])
                operation.verify_current()
        self.assertEqual((self.root / self.relative).read_bytes(), source_bytes)
        self.assertIsNone(self.before.lookup(self.config['record_id']))

    def test_existing_identity_and_retained_history_are_not_reinterpreted(self):
        with self.stage() as operation:
            after = catalog.SourceCatalogSnapshot(
                operation.candidate.snapshot(),
                expected_root_sha256=operation.candidate.root_sha256,
                trusted_baseline_sha256=operation.candidate.root_sha256,
                limits=MutationLimits(),
            )
        with self.assertRaisesRegex(ValueError, 'already belongs'):
            with self.stage(after):
                self.fail('existing catalog identity was accepted')

        package = (self.root / self.relative).parent
        history = package / addition.HISTORY
        history.write_bytes(b'{"schema_version":"tos_source_revision_history_v1","record_id":"x","receipts":[]}\n')
        self.addCleanup(history.unlink, missing_ok=True)
        with self.assertRaisesRegex(ValueError, 'initial package'):
            with self.stage():
                self.fail('retained history was accepted as an initial package')

    def test_owner_scope_drift_is_detected_after_detached_staging(self):
        with self.stage() as operation:
            self.owner.write_bytes(canonical_bytes({**self.config, 'allowed_operations': []}))
            with self.assertRaises(PermissionError):
                operation.verify_current()
        self.owner.write_bytes(canonical_bytes(self.config))


if __name__ == '__main__':
    unittest.main()

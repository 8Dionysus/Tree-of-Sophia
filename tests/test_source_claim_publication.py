"""Source-created Claims: complete prepared transition and recovery, synthetic facts."""
from contextlib import closing
import copy
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
for directory in ('scripts', 'access/src', 'access/tests', 'tests', 'mechanics/growth-cycle/tests'):
    sys.path.insert(0, str(ROOT / directory))

import test_source_agent_publication as fixtures
import source_claim_publication as publication
import source_commands as commands
from tos_access.projection_store import canonical_bytes
from tos_access.projection_mutation import ProjectionSnapshotView
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
from tos_access.published_search import PublishedSearchService
from tos_access.published_lens import PublishedLensService
from tos_access.prepared_source_binding import read_prepared_source_inputs_transaction
from tos_access.prepared_source_dependencies import lookup_source_dependencies_transaction
from tos_access.catalog_semantics import memory_catalog
from tos_access import knowledge as k
from test_indexed_lens import lens


class SourceClaimPublicationTests(unittest.TestCase):
    def setUp(self):
        self.base = fixtures.SourceAgentPublicationTests()
        self.base.setUp()
        self.addCleanup(self.base.doCleanups)
        self.root, self.db = self.base.root, self.base.db
        self.claim = copy.deepcopy(self.base.claim)
        self.evidence = 'ToS/review-ledger/new-claim-publication.md'
        self.base.helper.fixture.write(self.evidence, b'Synthetic integration evidence only.\n')
        ref = 'ToS/contracts/provenance-event-v2.schema.json'
        self.base.helper.fixture.write(ref, (ROOT / ref).read_bytes())
        self.claim.update(claim_id='tos.claim.synthetic-publication-addition',
            evidence_refs=[self.evidence], counterevidence_refs=[], alternative_claim_refs=[],
            provenance_event_ref='tos.event.synthetic-publication-addition')
        self.claim['qualifiers']['statement'] = 'AddressedNewClaim: synthetic scoped relationship, not history.'
        self.relative = 'ToS/source-witnesses/relations/synthetic-publication-addition/source-claims.jsonl'
        self.owner = self.root / 'addition-owner.json'
        self.config = {'schema_version': commands.CLAIM_CONFIG, 'uid': os.getuid(),
            'principal_id': self.claim['maker']['agent_ref'], 'maker_type': self.claim['maker']['maker_type'],
            'source_root': str(self.root), 'source_path': self.relative, 'authority_ref': 'test:source-publication',
            'expires_at': '2099-01-01T00:00:00Z', 'provenance_event_id': self.claim['provenance_event_ref'],
            'allowed_operations': ['claims.create'], 'allowed_claim_ids': [self.claim['claim_id']],
            'allowed_subject_refs': [self.claim['subject_ref']], 'allowed_object_refs': [self.claim['object']],
            'allowed_predicates': [self.claim['predicate']], 'allowed_evidence_refs': [self.evidence]}
        self.owner.write_bytes(canonical_bytes(self.config))
        preview = commands.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
            'operation': 'prepare-create', 'claims': [self.claim]})
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'claims.create',
            'command_id': 'synthetic-claim-publication', 'claims': [self.claim], 'expected_revision': None,
            'expected_configuration': preview['owner_configuration'],
            'expected_dependencies': preview['expected_dependencies'], 'expected_inputs': preview['source_bindings']}
        created = commands.run_local_command(self.owner, request)
        receipt_path = (self.root / self.relative).with_name('source-create-receipt.json')
        self.expected = {'expected_receipt_sha256': hashlib.sha256(receipt_path.read_bytes()).hexdigest(),
                         'expected_request_digest': created['receipt']['request_digest']}

    def operation(self):
        return publication.claim_addition_publication(self.owner, source_inputs=self.base.source,
            expected_binding=self.base.binding, catalog_inputs=self.base.inputs,
            progress_owner=self.base.owner, **self.expected)

    def oracle(self, operation):
        # Full union normalization of the exact admitted raw predecessor plus
        # separately source-verified addition; not a second source admission.
        bib = copy.deepcopy(self.base.bibliography)
        for field, key in (('nodes', 'node_id'), ('edges', 'edge_id')):
            rows = {row[key]: row for row in bib[field]}
            rows.update({row[key]: row for row in operation.raw[field].values()})
            bib[field] = [rows[name] for name in sorted(rows)]
        traces = {row['claim_ref']: row for row in bib['claim_traces']}
        traces.update(operation.raw['traces'])
        bib['claim_traces'] = [traces[name] for name in sorted(traces)]
        return k.build_knowledge_graph(self.base.corpus, {}, bib, self.base.entities, self.base.relations)

    def test_atomic_all_lane_addition_matches_full_union_oracle_and_old_reader_conflicts(self):
        before = PublishedKnowledgeReadModel(self.base.path, self.base.binding)
        old_catalog = before.catalog()
        with self.operation() as operation:
            observed = operation.raw
            observed['nodes'].clear()
            operation.binding['source_revision'] = 'f' * 64
            self.assertTrue(operation.raw['nodes'])
            self.assertEqual(operation.binding, self.base.binding)
            self.db.execute('BEGIN IMMEDIATE')
            with (patch.object(k, 'build_knowledge_graph', side_effect=AssertionError('full graph build')),
                  patch.object(ProjectionSnapshotView, 'materialize', side_effect=AssertionError('whole raw roots'))):
                result = operation.apply_transaction(self.db)
            self.assertFalse(result['prepared_committed'])
            self.assertEqual(before.catalog(), old_catalog)
            with closing(sqlite3.connect(self.base.path)) as observer:
                self.assertEqual(observer.execute('SELECT sha256 FROM prepared_source_state').fetchone()[0], self.base.source.digest)
            detached = operation.result
            detached['binding']['source_revision'] = 'f' * 64
            self.assertEqual(operation.result, result)
            result = operation.commit_transaction(self.db)
            expected = self.oracle(operation)
        self.assertTrue(result['prepared_committed'])
        self.assertGreater(result['raw_projection_reads']['cache_hits'], 0)
        expected.update(result['source_header'])
        for kind in ('node', 'relation'):
            stored = {identity: json.loads(raw) for identity, raw in self.db.execute('SELECT id,json FROM knowledge_' + kind + 's')}
            self.assertEqual(stored, {row['id']: row for row in expected[kind + 's']})
        self.assertEqual(result['semantic_report'], expected['counts']['semantic_validation'])
        after = PublishedKnowledgeReadModel(self.base.path, result['binding'])
        self.assertEqual(after.catalog(), memory_catalog(expected, self.base.corpus, {}, self.base.entities, self.base.relations))
        self.assertEqual(PublishedSearchService(after).search('AddressedNewClaim', limit=10)['nodes'],
                         k.search_knowledge_graph(expected, 'AddressedNewClaim', limit=10)['nodes'])
        spec = lens(seed={'node_ids': ['source-claims:identity:' + self.claim['subject_ref']]})
        self.assertEqual(PublishedLensService(after).execute(spec), k.execute_knowledge_lens(expected, spec))
        with self.assertRaises(PublishedSnapshotConflict):
            before.catalog()
        self.db.execute('BEGIN')
        paired = read_prepared_source_inputs_transaction(self.db, expected_binding=result['binding'])
        dependencies = lookup_source_dependencies_transaction(self.db, kind='identity', ref=self.claim['subject_ref'],
            expected_binding=result['binding'], source_inputs_sha256=paired.digest,
            declaration_profile_sha256=self.base.profile, progress_owner=self.base.owner)
        self.assertIn(self.claim['claim_id'], dependencies['claim_ids'])
        self.db.rollback()
        # The existing correction path must consume the new dependency/context
        # index, not only the newly added graph rows.
        self.base.binding, self.base.source = result['binding'], paired
        self.base.inputs = publication.CatalogInputs(result['source_header'], self.base.entities,
            self.base.relations, self.base.inputs.lenses, source_order_profile=publication.CANONICAL_ORDER)
        captured = self.base.capture()
        self.assertIn(self.claim['claim_id'], [row['claim_id'] for row in json.loads(captured.declarations_raw)])

    def test_late_failure_rolls_back_every_lane_and_retry_preserves_source(self):
        before = list(self.db.iterdump())
        joined = publication.apply_dependency_bound_prepared_delta_transaction
        def fail(*args, **kwargs):
            joined(*args, **kwargs)
            raise RuntimeError('injected after joined Claim lanes')
        with self.assertRaisesRegex(RuntimeError, 'after joined Claim lanes'):
            with self.operation() as operation:
                self.db.execute('BEGIN IMMEDIATE')
                with patch.object(publication, 'apply_dependency_bound_prepared_delta_transaction', side_effect=fail):
                    operation.apply_transaction(self.db)
        self.db.rollback()
        self.assertEqual(list(self.db.iterdump()), before)
        self.assertTrue((self.root / self.relative).exists())
        with self.operation() as operation:
            self.db.execute('BEGIN IMMEDIATE')
            operation.apply_transaction(self.db)
            self.assertTrue(operation.commit_transaction(self.db)['prepared_committed'])

    def test_late_source_and_sql_changes_fail_closed(self):
        before = list(self.db.iterdump())
        for tamper in ('source', 'sql'):
            with self.subTest(tamper=tamper), self.operation() as operation:
                self.db.execute('BEGIN IMMEDIATE')
                operation.apply_transaction(self.db)
                if tamper == 'source':
                    self.owner.write_bytes(canonical_bytes({**self.config, 'allowed_operations': []}))
                else:
                    self.db.execute('CREATE TABLE unexpected_caller_ddl(value TEXT)')
                with self.assertRaises((ValueError, PermissionError)):
                    operation.commit_transaction(self.db)
            self.db.rollback()
            self.owner.write_bytes(canonical_bytes(self.config))
            self.assertEqual(list(self.db.iterdump()), before)


if __name__ == '__main__':
    unittest.main()

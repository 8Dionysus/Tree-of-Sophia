"""Source-command metadata growth through the prepared reader; synthetic facts."""
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

ROOT = Path(__file__).resolve().parents[2]
for directory in ('scripts', 'access/src', 'access/tests', 'tests', 'mechanics/growth-cycle/tests'):
    sys.path.insert(0, str(ROOT / directory))

import test_source_agent_publication as fixtures
import source_metadata_publication as publication
import source_commands as commands
from tos_access import knowledge as k
from tos_access.catalog_semantics import memory_catalog
from tos_access.projection_store import canonical_bytes
from tos_access.projection_mutation import ProjectionSnapshotView
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
from tos_access.published_search import PublishedSearchService
from tos_access.prepared_source_binding import read_prepared_source_inputs_transaction


class SourceMetadataPublicationTests(unittest.TestCase):
    def setUp(self):
        self.base = fixtures.SourceAgentPublicationTests()
        self.base.setUp()
        self.addCleanup(self.base.doCleanups)
        self.root, self.db = self.base.root, self.base.db
        ref = 'ToS/contracts/provenance-event-v2.schema.json'
        self.base.helper.fixture.write(ref, (ROOT / ref).read_bytes())
        self.record = copy.deepcopy(self.base.helper.fixture.record)
        self.record.update(record_id='tos.agent.synthetic-metadata-addition', record_version=1,
            identity_status='provisional', same_as_posture='no_equivalence_claim',
            preferred_label='AddressedMetadataGrowth', notes='Synthetic metadata, not a historical person.',
            external_identifiers=[], source_refs=['https://example.invalid/synthetic-metadata'])
        self.record.pop('variant_labels', None)
        self.record.pop('supersedes_ref', None)
        self.relative = 'ToS/source-witnesses/agents/synthetic-metadata-addition/agent.json'
        self.owner = self.root / 'metadata-addition-owner.json'
        self.config = {'schema_version': commands.CORPUS_CONFIG, 'uid': os.getuid(),
            'principal_id': 'software:synthetic', 'maker_type': 'software', 'source_root': str(self.root),
            'source_path': self.relative, 'record_id': self.record['record_id'], 'record_type': 'agent',
            'authority_ref': 'test:metadata-publication', 'expires_at': '2099-01-01T00:00:00Z',
            'provenance_event_id': 'tos.event.synthetic-metadata-addition',
            'allowed_operations': ['source.create'],
            'allowed_form_ids': ['tos.form.synthetic-metadata-addition.name']}
        self.owner.write_bytes(canonical_bytes(self.config))
        request = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-create',
            'record': self.record, 'forms': [{'field_id': 'metadata.preferred-name',
                'form_id': self.config['allowed_form_ids'][0]}]}
        preview = commands.run_local_command(self.owner, request)
        request.update(operation='source.create', command_id='synthetic-metadata-addition',
            expected_configuration=preview['owner_configuration'], expected_dependencies=preview['expected_dependencies'],
            expected_revision=None, expected_source=None)
        created = commands.run_local_command(self.owner, request)
        receipt = (self.root / self.relative).with_name('source-create-receipt.json')
        self.expected = {'expected_receipt_sha256': hashlib.sha256(receipt.read_bytes()).hexdigest(),
                         'expected_request_digest': created['receipt']['request_digest']}

    def operation(self):
        return publication.metadata_addition_publication(self.owner, source_inputs=self.base.source,
            expected_binding=self.base.binding, catalog_inputs=self.base.inputs,
            progress_owner=self.base.owner, **self.expected)

    def oracle(self, operation):
        corpus, bibliography = copy.deepcopy(self.base.corpus), copy.deepcopy(self.base.bibliography)
        for (graph, _), row in operation.raw['nodes'].items():
            (corpus['source_navigation'] if graph == 'source-navigation' else bibliography)['nodes'].append(row)
        for (graph, _), row in operation.raw['edges'].items():
            (corpus['source_navigation'] if graph == 'source-navigation' else bibliography)['edges'].append(row)
        return k.build_knowledge_graph(corpus, {}, bibliography, self.base.entities, self.base.relations), corpus

    def test_all_lanes_match_full_union_without_whole_corpus_work(self):
        before = PublishedKnowledgeReadModel(self.base.path, self.base.binding)
        old_catalog = before.catalog()
        with self.operation() as operation:
            expected, corpus = self.oracle(operation)
            self.db.execute('BEGIN IMMEDIATE')
            kernel = k.build_knowledge_graph
            def bounded_kernel(selected_corpus, philosophy, bibliography, *args):
                self.assertEqual(set(selected_corpus), {'source_navigation'})
                self.assertEqual(philosophy, {})
                self.assertEqual(len(selected_corpus['source_navigation']['nodes']), 2)
                self.assertEqual(len(bibliography['nodes']), 1)
                self.assertEqual(bibliography['claim_traces'], [])
                return kernel(selected_corpus, philosophy, bibliography, *args)
            with (patch.object(k, 'build_knowledge_graph', side_effect=bounded_kernel),
                  patch.object(ProjectionSnapshotView, 'materialize', side_effect=AssertionError('full materialization')),
                  patch.object(Path, 'rglob', side_effect=AssertionError('full source scan'))):
                result = operation.apply_transaction(self.db)
                self.assertEqual(before.catalog(), old_catalog)
                with closing(sqlite3.connect(self.base.path)) as observer:
                    self.assertEqual(observer.execute('SELECT sha256 FROM prepared_source_state').fetchone()[0], self.base.source.digest)
                result = operation.commit_transaction(self.db)
        expected.update(result['source_header'])
        for kind in ('node', 'relation'):
            stored = {identity: json.loads(raw) for identity, raw in self.db.execute('SELECT id,json FROM knowledge_' + kind + 's')}
            oracle = {row['id']: row for row in expected[kind + 's']}
            differences = {identity: sorted(key for key in stored[identity].keys() | oracle[identity].keys()
                                           if stored[identity].get(key) != oracle[identity].get(key))
                           for identity in stored.keys() & oracle.keys() if stored[identity] != oracle[identity]}
            self.assertTrue(stored == oracle, {'kind': kind, 'missing': sorted(oracle.keys() - stored.keys()),
                'extra': sorted(stored.keys() - oracle.keys()), 'changed_fields': differences})
        after = PublishedKnowledgeReadModel(self.base.path, result['binding'])
        self.assertEqual(after.catalog(), memory_catalog(expected, corpus, {}, self.base.entities, self.base.relations))
        self.assertEqual(PublishedSearchService(after).search('AddressedMetadataGrowth', limit=10)['nodes'],
                         k.search_knowledge_graph(expected, 'AddressedMetadataGrowth', limit=10)['nodes'])
        self.assertEqual(result['changed_nodes'], 3)
        self.assertEqual(result['changed_relations'], 2)
        self.assertFalse(result['is_semantic_acceptance'])
        self.assertFalse(result['consumer_switched'])
        self.assertNotIn('catalog', result)
        self.assertNotIn('semantic_report', result)
        self.assertIn('semantic_validation', result['source_header']['counts'])
        with self.assertRaises(PublishedSnapshotConflict):
            before.catalog()
        self.db.execute('BEGIN')
        paired = read_prepared_source_inputs_transaction(self.db, expected_binding=result['binding'])
        self.assertNotEqual(paired.digest, self.base.source.digest)
        self.db.rollback()

    def test_late_failure_rolls_back_and_keeps_source_for_retry(self):
        before = list(self.db.iterdump())
        joined = publication.apply_dependency_bound_prepared_delta_transaction
        def fail(*args, **kwargs):
            joined(*args, **kwargs)
            raise RuntimeError('injected after joined metadata lanes')
        with self.assertRaisesRegex(RuntimeError, 'after joined metadata lanes'):
            with self.operation() as operation:
                self.db.execute('BEGIN IMMEDIATE')
                with patch.object(publication, 'apply_dependency_bound_prepared_delta_transaction', side_effect=fail):
                    operation.apply_transaction(self.db)
        self.db.rollback()
        self.assertEqual(list(self.db.iterdump()), before)
        self.assertTrue((self.root / self.relative).is_file())
        with self.operation() as operation:
            self.db.execute('BEGIN IMMEDIATE')
            operation.apply_transaction(self.db)
            self.assertTrue(operation.commit_transaction(self.db)['prepared_committed'])

    def test_explicit_rollback_is_terminal_without_forcing_a_commit(self):
        before = list(self.db.iterdump())
        with self.operation() as operation:
            self.db.execute('BEGIN IMMEDIATE')
            operation.apply_transaction(self.db)
            result = operation.rollback_transaction(self.db)
            self.assertTrue(result['prepared_rolled_back'])
            self.assertFalse(result['prepared_committed'])
            with self.assertRaises(ValueError):
                operation.commit_transaction(self.db)
            with self.assertRaises(ValueError):
                operation.rollback_transaction(self.db)
        self.assertEqual(list(self.db.iterdump()), before)
        self.assertTrue((self.root / self.relative).is_file())

    def test_post_apply_delegation_or_schema_change_refuses_commit(self):
        before = list(self.db.iterdump())
        for tamper in ('delegation', 'schema'):
            with self.subTest(tamper=tamper):
                with self.assertRaises((ValueError, PermissionError)):
                    with self.operation() as operation:
                        self.db.execute('BEGIN IMMEDIATE')
                        operation.apply_transaction(self.db)
                        if tamper == 'delegation':
                            self.owner.write_bytes(canonical_bytes({**self.config, 'allowed_operations': []}))
                        else:
                            self.db.execute('CREATE TABLE forbidden_caller_ddl(value TEXT)')
                        operation.commit_transaction(self.db)
                self.db.rollback()
                self.owner.write_bytes(canonical_bytes(self.config))
                self.assertEqual(list(self.db.iterdump()), before)


if __name__ == '__main__':
    unittest.main()

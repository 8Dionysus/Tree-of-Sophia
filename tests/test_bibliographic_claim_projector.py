"""Pure Claim assembly and real-source metadata dependency boundaries.

Fixture associations are synthetic, never historical evidence or admission.
"""
from __future__ import annotations

import copy
from dataclasses import replace
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import test_source_witness_bibliographic_graph as fixtures
import source_witness_bibliographic_graph_common as owner


class BibliographicClaimProjectorTest(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.SourceWitnessBibliographicGraphTest()

    def capture(self, rebuild):
        inputs = []
        project = owner.project_bibliographic_claim

        def render(value):
            inputs.append(copy.deepcopy(value))
            return project(value)

        with patch.object(owner, 'project_bibliographic_claim', side_effect=render):
            payload = rebuild()
        return payload, inputs

    def test_full_builder_uses_pure_exact_cohorts_without_io_or_input_mutation(self):
        with self.fixture.historical_fixture() as (_root, _history, _real, claims, rebuild):
            claims[0]['counterevidence_refs'] = claims[1]['evidence_refs']
            claims[0]['alternative_claim_refs'] = [claims[2]['claim_id'], claims[1]['claim_id']]
            claims[0]['supersedes_claim_ref'] = claims[2]['claim_id']
            payload, inputs = self.capture(rebuild)
            nodes = {row['node_id']: row for row in payload['nodes']}
            edges = {row['edge_id']: row for row in payload['edges']}
            traces = {row['claim_ref']: row for row in payload['claim_traces']}
            before = copy.deepcopy(inputs)
            with patch.object(Path, 'open', side_effect=AssertionError('pure projector opened a file')), \
                    patch('builtins.open', side_effect=AssertionError('pure projector opened a file')):
                for value in inputs:
                    projected = owner.project_bibliographic_claim(value)
                    self.assertFalse(projected.source_verification_performed)
                    self.assertFalse(projected.incident_closure_verified)
                    self.assertEqual(projected.trace, traces[value.source_claim['claim_id']])
                    for node in projected.nodes:
                        self.assertEqual(node, nodes[node['node_id']])
                    for edge in projected.edges:
                        self.assertEqual(edge, edges[edge['edge_id']])
            self.assertEqual(inputs, before)

    def test_source_agent_label_and_version_reaches_claim_and_non_agent_relations(self):
        with self.fixture.historical_fixture() as (root, _history, real, claims, rebuild):
            # Identity evidence is a hidden dependency without an Agent-edge.
            claims[0]['evidence_refs'] = [real[0]['record_id']]
            source_claims = copy.deepcopy(claims)
            before, before_inputs = self.capture(rebuild)
            old_graph, _, _ = self.fixture.historical_knowledge(root, before)
            ref = 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json'
            agent = json.loads((root / ref).read_bytes())
            agent.update(preferred_label='Synthetic revised Agent label', record_version=agent['record_version'] + 1)
            (root / ref).write_text(json.dumps(agent, ensure_ascii=False))
            after, after_inputs = self.capture(rebuild)
            new_graph, _, _ = self.fixture.historical_knowledge(root, after)
            self.assertEqual(source_claims, claims)
            self.assertEqual(before_inputs[0].source_claim, after_inputs[0].source_claim)
            old_raw = owner.project_bibliographic_claim(before_inputs[0])
            new_raw = owner.project_bibliographic_claim(after_inputs[0])
            self.assertEqual(old_raw.trace, new_raw.trace)
            self.assertEqual(old_raw.edges, new_raw.edges)
            self.assertNotEqual(before_inputs[0].evidence_nodes, after_inputs[0].evidence_nodes)
            claim_ref = claims[0]['claim_id']
            old_claim = next(row for row in old_graph['nodes'] if row['entity_id'] == claim_ref)
            new_claim = next(row for row in new_graph['nodes'] if row['entity_id'] == claim_ref)
            self.assertNotEqual(old_claim['display']['title'], new_claim['display']['title'])
            self.assertNotEqual(old_claim['attributes']['navigation_descriptor'],
                                new_claim['attributes']['navigation_descriptor'])
            agent_id = next(row['id'] for row in new_graph['nodes'] if row['entity_id'] == real[0]['record_id'])
            relations = {row['id']: row for row in new_graph['relations']}
            claim_relations = [row for row in old_graph['relations'] if row['from_id'] == old_claim['id']]
            self.assertGreaterEqual(len(claim_relations), 5)
            beyond_agent = [row for row in claim_relations if row['to_id'] != agent_id]
            self.assertGreaterEqual(len(beyond_agent), 4)
            for row in claim_relations:
                self.assertNotEqual(row['display'], relations[row['id']]['display'])
            # Pure cohort replacement retains complete raw nodes/edges/traces;
            # full normalization remains the oracle, not an incremental reducer.
            nodes = {row['node_id']: row for row in before['nodes']}
            nodes.update((row['node_id'], row) for row in new_raw.nodes)
            reconstructed = {**after, 'nodes': sorted(nodes.values(), key=lambda row: row['node_id'])}
            rebuilt_graph, _, _ = self.fixture.historical_knowledge(root, reconstructed)
            self.assertEqual(rebuilt_graph['nodes'], new_graph['nodes'])
            self.assertEqual(rebuilt_graph['relations'], new_graph['relations'])

    def test_version_only_updates_descriptor_binding_without_source_claim_change(self):
        with self.fixture.historical_fixture() as (root, _history, _real, _claims, rebuild):
            _, before = self.capture(rebuild)
            ref = 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json'
            agent = json.loads((root / ref).read_bytes())
            agent.update(record_version=agent['record_version'] + 1, notes='Synthetic note-only correction.')
            (root / ref).write_text(json.dumps(agent, ensure_ascii=False))
            _, after = self.capture(rebuild)
            old = owner.project_bibliographic_claim(before[0])
            new = owner.project_bibliographic_claim(after[0])
            old_claim = next(row for row in old.nodes if row['node_kind'] == 'claim')
            new_claim = next(row for row in new.nodes if row['node_kind'] == 'claim')
            self.assertNotEqual(old_claim['properties']['navigation_descriptor'],
                                new_claim['properties']['navigation_descriptor'])
            self.assertEqual(old.trace, new.trace)
            self.assertEqual(before[0].source_claim, after[0].source_claim)

    def test_source_agent_as_maker_is_a_dependency_without_identity_incident_edges(self):
        with self.fixture.historical_fixture() as (root, _history, real, claims, rebuild):
            claims[1]['maker'] = {'maker_type': 'human', 'agent_ref': real[0]['record_id']}
            _, before = self.capture(rebuild)
            ref = 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json'
            agent = json.loads((root / ref).read_bytes())
            agent.update(record_version=agent['record_version'] + 1, notes='Synthetic maker metadata correction.')
            (root / ref).write_text(json.dumps(agent, ensure_ascii=False))
            _, after = self.capture(rebuild)
            old = owner.project_bibliographic_claim(before[1])
            new = owner.project_bibliographic_claim(after[1])
            self.assertEqual(before[1].source_claim, after[1].source_claim)
            self.assertNotEqual(before[1].maker_node['source_sha256'], after[1].maker_node['source_sha256'])
            agent_id = before[1].maker_identity_node['node_id']
            self.assertTrue(all(agent_id not in (edge['from_id'], edge['to_id']) for edge in old.edges))
            self.assertEqual(old.edges, new.edges)
            self.assertEqual(old.trace, new.trace)

    def test_exact_evidence_and_referenced_claim_coverage_are_required(self):
        with self.fixture.historical_fixture() as (_root, _history, _real, _claims, rebuild):
            _, inputs = self.capture(rebuild)
            value = inputs[0]
            for changed in (replace(value, evidence_nodes=()),
                            replace(value, evidence_nodes=inputs[1].evidence_nodes)):
                with self.assertRaisesRegex(owner.BibliographicGraphBuildError, 'exact ordered evidence'):
                    owner.project_bibliographic_claim(changed)
            claim = copy.deepcopy(value.source_claim)
            claim['alternative_claim_refs'] = ['tos.claim.not-selected']
            with self.assertRaisesRegex(owner.BibliographicGraphBuildError, 'outside the projection'):
                owner.project_bibliographic_claim(replace(value, source_claim=claim))

    def test_owner_order_duplicates_and_optional_context_are_preserved(self):
        with self.fixture.historical_fixture() as (_root, _history, _real, _claims, rebuild):
            _, inputs = self.capture(rebuild)
            value = inputs[0]
            claim = copy.deepcopy(value.source_claim)
            claim['evidence_refs'] *= 2
            claim['reviews'] = [{'review_id': 'test:review', 'reviewer_kind': 'agent',
                                'reviewer_ref': 'software:test', 'decision': 'needs_revision',
                                'reviewed_at': '2026-09-12T00:00:00Z'}] * 2
            members = (value.object_node, value.subject_node, value.object_node)
            value = replace(value, source_claim=claim, evidence_nodes=value.evidence_nodes * 2,
                member_nodes=members, normalized_identity_edges=(('has_date_anchor', value.subject_node),),
                forms=('test:forms', b'{}', [{'test': [None, False, 'Ω']}]),
                collection_order_basis={'test': 'owner-resolved'},
                legacy_object_link_context={'test_legacy': False})
            output = owner.project_bibliographic_claim(value)
            kinds = [edge['edge_kind'] for edge in output.edges]
            self.assertEqual(kinds, ['has_subject', 'has_object', 'made_by', 'generated_by',
                                    'supported_by', 'reviewed_by', 'reviewed_by',
                                    'has_value_member', 'has_value_member', 'has_value_member', 'has_date_anchor'])
            self.assertEqual(output.trace['value_member_node_ids'], sorted(row['node_id'] for row in members))
            self.assertEqual(len(output.trace['review_node_ids']), 2)
            self.assertEqual(len(output.trace['evidence_node_ids']), 1)
            properties = next(row['properties'] for row in output.nodes if row['node_kind'] == 'claim')
            self.assertEqual(properties['human_forms'], value.forms[2])
            self.assertEqual(properties['collection_order_basis'], value.collection_order_basis)
            self.assertIs(properties['test_legacy'], False)


if __name__ == '__main__':
    unittest.main()

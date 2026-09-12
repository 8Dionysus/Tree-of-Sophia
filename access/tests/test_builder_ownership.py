"""Tiny-fixture ownership and full-value parity for the one-shot builder."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ACCESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS / 'src'))
from tos_access import knowledge
from tos_access.normalization_cache import NormalizationCache


def mutable_ids(value):
    """Identity, not equality, detects nested mutable borrowing from input."""
    found = set()
    def visit(item):
        if not isinstance(item, (dict, list)) or id(item) in found:
            return
        found.add(id(item))
        for child in item.values() if isinstance(item, dict) else item:
            visit(child)
    visit(value)
    return found


class BuilderOwnershipTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        registry = ACCESS.parent / 'ToS/doctrine/semantic-interchange'
        cls.entities = json.loads((registry / 'entity-types.v1.json').read_text())
        cls.relations = json.loads((registry / 'relation-types.v1.json').read_text())

    def fixture(self):
        from test_knowledge_contract import KnowledgeContractTests
        corpus, philosophy = KnowledgeContractTests().fixture()
        philosophy['nodes'][0].update(
            unknown_extension={'nested': [None, False, 0, []]},
            display={'provenance': {'unknown': {'nested': ['source']}}})
        philosophy['nodes'][0]['properties']['nested'] = {'values': ['raw']}
        philosophy['nodes'][0]['properties']['source_record'] = {
            'record_id': 'tos.record.synthetic-owned',
            'unknown_extension': {'values': [False, None, 0]},
        }
        philosophy['edges'][0]['view_ids'].append('inherited-only')
        return corpus, philosophy

    def claim_fixture(self):
        # No corpus payload is loaded. Exercise claim updates, literal contexts,
        # unknown source extensions, and temporal/place values on four tiny rows.
        claim_ref = 'tos.claim.synthetic-owned'
        bibliographic = {'nodes': [
            {'node_id': 'subject', 'node_kind': 'identity',
             'properties': {'identity_kind': 'work'}},
            {'node_id': 'claim', 'node_kind': 'claim',
             'properties': {'claim_ref': claim_ref, 'polarity': 'negative',
                            'qualifiers': {'scope': ['synthetic']}}},
            {'node_id': 'literal', 'node_kind': 'literal',
             'properties': {'claim_ref': claim_ref, 'value': {
                 'interval': {'start': '1883', 'end': '1885'},
                 'places': [{'label': 'Synthetic place', 'extension': {'v': [1]}}]}}},
        ], 'edges': [
            {'edge_id': 'subject', 'edge_kind': 'has_subject', 'from_id': 'claim',
             'to_id': 'subject', 'claim_ref': claim_ref, 'view_ids': ['claim-view']},
            {'edge_id': 'object', 'edge_kind': 'has_object', 'from_id': 'claim',
             'to_id': 'literal', 'claim_ref': claim_ref},
        ], 'claim_traces': [{'claim_ref': claim_ref, 'claim_node_id': 'claim',
            'subject_node_id': 'subject', 'object_node_id': 'literal',
            'predicate': 'synthetic', 'evidence_node_ids': [],
            'unknown_trace': {'v': [False]}}]}
        return {}, {}, bibliographic

    def test_owned_target_detaches_borrowed_subtrees_but_reuses_isolated_envelope(self):
        raw = {'node_id': 'annotation', 'node_kind': 'annotation-claim',
            'display': {'provenance': {'extension': {'v': ['source']}}},
            'properties': {'packet_id': 'synthetic', 'proposition': {'v': ['source']},
                           'unknown_extension': {'v': [False]}}, 'view_ids': ['base']}
        node = knowledge._normalize_node(raw, 'source-navigation')
        # These are real normalizer boundaries, not assumed ownership.
        self.assertIs(node['attributes']['proposition'], raw['properties']['proposition'])
        self.assertIs(node['semantics']['claim']['proposition'], raw['properties']['proposition'])
        self.assertIs(node['display']['provenance']['extension'], raw['display']['provenance']['extension'])
        self.assertFalse(mutable_ids(node['source_record']) & mutable_ids(raw))
        envelope = node['source_record']
        original = copy.deepcopy(raw)
        contexts = [{'fields': {'scope': {'value': ['new-context']}}}]
        update = ({'subject': {'id': ['s']}}, {'claim_ref': {'v': ['c']}})
        update_before, contexts_before = copy.deepcopy(update), copy.deepcopy(contexts)
        expected = knowledge._final_node_value(node, update, ['inherited'], contexts)
        actual = knowledge._finalize_knowledge_node(node, update, ['inherited'], contexts, _owned=True)
        self.assertIs(actual, node)
        self.assertIs(actual['source_record'], envelope)
        self.assertEqual(actual, expected)
        self.assertEqual(actual['content_revision'], knowledge._content_revision(actual))
        self.assertFalse(mutable_ids(actual) & mutable_ids(raw))
        self.assertFalse(mutable_ids(actual) & (mutable_ids(list(update)) | mutable_ids(contexts)))
        actual['attributes']['proposition']['v'].append('consumer')
        actual['display']['provenance']['extension']['v'].append('consumer')
        actual['semantics']['claim']['subject']['id'].append('consumer')
        actual['semantics']['assertion_contexts'][-1]['fields']['scope']['value'].append('consumer')
        self.assertEqual(raw, original)
        self.assertEqual(update, update_before)
        self.assertEqual(contexts, contexts_before)

    def test_full_builder_owned_and_ordinary_values_and_input_isolation_match(self):
        corpus, philosophy = self.fixture()
        for inputs in [(corpus, philosophy, {}, self.entities, self.relations),
                       (*self.claim_fixture(), None, None), ({}, {}, {}, None, None)]:
            with self.subTest(nodes=len(inputs[1].get('nodes', []))):
                before = copy.deepcopy(inputs)
                with patch.object(knowledge, '_owned_final_node_value',
                                  side_effect=knowledge._final_node_value):
                    ordinary = knowledge.build_knowledge_graph(*inputs)
                owned = knowledge.build_knowledge_graph(*inputs)
                self.assertEqual(owned, ordinary)
                self.assertEqual(inputs, before)
                if inputs[3]:
                    self.assertTrue(any('readable_context' in node for node in owned['nodes']))
                    self.assertIn('semantic_validation', owned['counts'])
                # Public node delivery remains detached from source AND registry.
                borrowed = mutable_ids(list(inputs))
                for node in owned['nodes']:
                    self.assertFalse(mutable_ids(node) & borrowed, node['id'])
                    self.assertEqual(node['content_revision'], knowledge._content_revision(node))
                self.assertEqual(knowledge.knowledge_catalog(owned, inputs[0], inputs[1], inputs[3], inputs[4]),
                                 knowledge.knowledge_catalog(ordinary, inputs[0], inputs[1], inputs[3], inputs[4]))
        node = next(node for node in owned['nodes'] if node['source_graph'] == 'repository')
        self.assertTrue(node['source_record']['payload'])

    def test_unchanged_owned_node_does_not_restamp_and_keeps_source_isolation(self):
        raw = {'node_id': 'n', 'properties': {'nested': {'v': ['source']}}, 'view_ids': ['a']}
        node = knowledge._normalize_node(raw, 'philosophy')
        expected = copy.deepcopy(node)
        with patch.object(knowledge, '_stamp_content_revision', side_effect=AssertionError('unchanged revision')):
            result = knowledge._finalize_knowledge_node(node, None, ['a'], _owned=True)
        self.assertIs(result, node)
        self.assertEqual(result, expected)
        raw['properties']['nested']['v'].append('later input edit')
        self.assertEqual(result, expected)

    def test_cache_path_never_consumes_owned_nodes_and_warm_snapshot_is_isolated(self):
        corpus, philosophy = self.fixture()
        expected = knowledge.build_knowledge_graph(corpus, philosophy, {}, self.entities, self.relations)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'steps.sqlite'
            with patch.object(knowledge, '_owned_final_node_value', side_effect=AssertionError('cache ownership')):
                with NormalizationCache(path, 'owned-fixture'):
                    first = knowledge.build_knowledge_graph(corpus, philosophy, {}, self.entities, self.relations)
                self.assertEqual(first, expected)
                target = next(node for node in first['nodes'] if node['id'] == 'philosophy:a')
                target['attributes']['nested']['values'].append('consumer mutation')
                with NormalizationCache(path, 'owned-fixture') as warm:
                    second = knowledge.build_knowledge_graph(corpus, philosophy, {}, self.entities, self.relations)
                self.assertEqual(second, expected)
                self.assertEqual(warm.misses, 0)

    def test_addressed_update_keeps_previous_and_replacement_isolated(self):
        corpus, philosophy = self.fixture()
        first = knowledge.build_knowledge_graph(corpus, philosophy, {}, self.entities, self.relations)
        previous = copy.deepcopy(first)
        replacement = copy.deepcopy(philosophy['nodes'][0])
        replacement['label'] = 'Synthetic addressed edit'
        changed = copy.deepcopy(philosophy)
        changed['nodes'][0] = replacement
        expected = knowledge.build_knowledge_graph(corpus, changed, {}, self.entities, self.relations)
        with patch.object(knowledge, '_owned_final_node_value', side_effect=AssertionError('addressed ownership')):
            updated = knowledge.addressed_update_knowledge_graph(first, 'philosophy', 'a', replacement,
                self.entities, self.relations, source_revision=expected['source_revision'])
        self.assertEqual(updated, expected)
        self.assertEqual(first, previous)
        node = next(node for node in updated['nodes'] if node['id'] == 'philosophy:a')
        self.assertFalse(mutable_ids(node) & mutable_ids(replacement))
        node['attributes']['nested']['values'].append('consumer edit')
        self.assertEqual(first, previous)
        self.assertEqual(replacement['properties']['nested']['values'], ['raw'])


if __name__ == '__main__':
    unittest.main()

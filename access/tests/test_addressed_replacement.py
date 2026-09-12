"""Durable boundary: local candidate computation is not incidence admission."""
import copy
from dataclasses import replace
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ACCESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS / 'src'))
from tos_access import knowledge
from tos_access.addressed_replacement import ReplacementLimits, replace_direct_carrier_candidate
from tos_access.normalization_cache import active_cache


class AddressedReplacementTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        folder = ACCESS.parent / 'ToS/doctrine/semantic-interchange'
        cls.entities = json.loads((folder / 'entity-types.v1.json').read_text())
        cls.relations = json.loads((folder / 'relation-types.v1.json').read_text())

    def fixture(self):
        from test_builder_ownership import BuilderOwnershipTests
        corpus, philosophy = BuilderOwnershipTests().fixture()
        graph = knowledge.build_knowledge_graph(corpus, philosophy, {}, self.entities, self.relations)
        prior = next(row for row in graph['nodes'] if row['id'] == 'philosophy:a')
        incident = [row for row in graph['relations'] if prior['id'] in (row['from_id'], row['to_id'])]
        ids = {row[field] for row in incident for field in ('from_id', 'to_id')} - {prior['id']}
        endpoints = [row for row in graph['nodes'] if row['id'] in ids]
        args = dict(incident_relations=incident, endpoint_nodes=endpoints,
                    normalization_binding=graph['normalization_binding'],
                    entity_registry=copy.deepcopy(self.entities), relation_registry=copy.deepcopy(self.relations))
        return corpus, philosophy, graph, prior, copy.deepcopy(philosophy['nodes'][0]), args

    def test_candidate_wrapper_and_full_builder_parity(self):
        for field, value in [('label', 'Изменённое имя'), ('summary', 'Новая аннотация')]:
            with self.subTest(field=field):
                corpus, philosophy, graph, prior, raw, args = self.fixture()
                raw[field] = value
                changed = copy.deepcopy(philosophy)
                changed['nodes'][0] = raw
                full = knowledge.build_knowledge_graph(corpus, changed, {}, self.entities, self.relations)
                candidate = replace_direct_carrier_candidate(prior, raw, **args)
                wrapped = knowledge.addressed_update_knowledge_graph(
                    graph, 'philosophy', 'a', raw, self.entities, self.relations,
                    source_revision=full['source_revision'])
                self.assertEqual(wrapped, full)
                self.assertEqual(candidate['node'], next(row for row in full['nodes'] if row['id'] == prior['id']))
                self.assertEqual(candidate['relations'], [row for row in full['relations']
                    if row['id'] in candidate['scope']['supplied_relation_ids']])
                self.assertIn('inherited-only', candidate['node']['view_ids'])
                self.assertIn('readable_context', candidate['node'])
                for row in [candidate['node'], *candidate['relations']]:
                    self.assertEqual(row['content_revision'], knowledge._content_revision(row))

    def test_inputs_and_registry_values_are_not_borrowed_or_mutated(self):
        from test_builder_ownership import mutable_ids
        _, _, _, prior, raw, args = self.fixture()
        all_inputs = [prior, raw, args]
        before = copy.deepcopy(all_inputs)
        result = replace_direct_carrier_candidate(prior, raw, **args)
        self.assertEqual(all_inputs, before)
        self.assertFalse(mutable_ids(result) & mutable_ids(all_inputs))
        result['node']['source_record']['payload']['properties']['nested']['values'].append('consumer')
        self.assertEqual(all_inputs, before)

    def test_absent_incidence_is_explicitly_not_verified(self):
        _, _, _, prior, raw, args = self.fixture()
        args.update(incident_relations=[], endpoint_nodes=[])
        candidate = replace_direct_carrier_candidate(prior, raw, **args)
        self.assertEqual(candidate['relations'], [])
        self.assertNotIn('inherited-only', candidate['node']['view_ids'])
        self.assertEqual(candidate['scope']['kind'], 'supplied-neighborhood-local-only')
        for name in ('complete_incidence_verified', 'source_transition_verified',
                     'global_semantics_validated', 'catalog_updated', 'publication_current', 'published'):
            self.assertIs(candidate['scope'][name], False)
        self.assertNotIn('source_revision', candidate)
        self.assertNotIn('graph', candidate)

    def test_corrupt_content_and_source_digests_are_refused(self):
        for role in ('prior', 'relation', 'endpoint'):
            for corrupt in ('content', 'source'):
                with self.subTest(role=role, corrupt=corrupt):
                    _, _, _, prior, raw, args = self.fixture()
                    row = {'prior': prior, 'relation': args['incident_relations'][0],
                           'endpoint': args['endpoint_nodes'][0]}[role]
                    if corrupt == 'content':
                        row['display']['title' if role != 'relation' else 'label']['default'] = 'corrupt'
                    else:
                        row['source_record']['payload']['unknown'] = False
                        knowledge._stamp_content_revision(row)
                    with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'corrupt'):
                        replace_direct_carrier_candidate(prior, raw, **args)

    def test_identity_and_missing_required_endpoint_fail(self):
        _, _, _, prior, raw, args = self.fixture()
        for changes in ({'endpoint_nodes': []}, {'endpoint_nodes': args['endpoint_nodes'] * 2},
                        {'incident_relations': args['incident_relations'] * 2}):
            with self.subTest(changes=list(changes)):
                with self.assertRaises(knowledge.AddressedUpdateError):
                    replace_direct_carrier_candidate(prior, raw, **{**args, **changes})
        raw['node_id'] = 'other'
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'native id'):
            replace_direct_carrier_candidate(prior, raw, **args)

    def test_nonincident_row_and_retargeted_payload_fail(self):
        _, _, _, prior, raw, args = self.fixture()
        relation = args['incident_relations'][0]
        relation['from_id'] = relation['to_id']
        knowledge._stamp_content_revision(relation)
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'not incident'):
            replace_direct_carrier_candidate(prior, raw, **args)
        _, _, _, prior, raw, args = self.fixture()
        relation = args['incident_relations'][0]
        relation['source_record']['payload']['to_id'] = 'missing'
        relation['source_record']['digest'] = knowledge._stable_digest(relation['source_record']['payload'])
        knowledge._stamp_content_revision(relation)
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'changed endpoints'):
            replace_direct_carrier_candidate(prior, raw, **args)

    def test_registry_binding_and_unsupported_source_refuse(self):
        _, _, _, prior, raw, args = self.fixture()
        args['entity_registry']['types'][0]['labels']['en'] = 'changed'
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'binding changed'):
            replace_direct_carrier_candidate(prior, raw, **args)
        _, _, _, prior, raw, args = self.fixture()
        prior.update(source_graph='repository', id='repository:a')
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'direct source'):
            replace_direct_carrier_candidate(prior, raw, **args)

    def test_claim_and_referenced_context_guards_remain(self):
        _, _, _, prior, raw, args = self.fixture()
        prior['semantics']['claim'] = {'claim_ref': 'external'}
        knowledge._stamp_content_revision(prior)
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'claim-enriched'):
            replace_direct_carrier_candidate(prior, raw, **args)
        _, _, _, prior, raw, args = self.fixture()
        relation = args['incident_relations'][0]
        relation['semantics']['assertion_contexts'] = [{'external': 'context'}]
        knowledge._stamp_content_revision(relation)
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'referenced claim context'):
            replace_direct_carrier_candidate(prior, raw, **args)

    def test_all_limits_and_exact_byte_boundaries(self):
        _, _, _, prior, raw, args = self.fixture()
        result = replace_direct_carrier_candidate(prior, raw, **args)
        accounting = result['accounting']
        exact = ReplacementLimits(accounting['relations'], accounting['endpoints'],
                                  accounting['input_bytes'], accounting['output_row_bytes'])
        self.assertEqual(replace_direct_carrier_candidate(prior, raw, **args, limits=exact), result)
        for name in ('max_relations', 'max_endpoints', 'max_input_bytes', 'max_output_bytes'):
            with self.subTest(name=name):
                with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'budget'):
                    replace_direct_carrier_candidate(prior, raw, **args,
                        limits=replace(exact, **{name: getattr(exact, name) - 1}))
                for bad in (True, -1, 1.5):
                    with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'nonnegative integers'):
                        replace_direct_carrier_candidate(prior, raw, **args, limits=replace(exact, **{name: bad}))

    def test_invalid_json_and_preflight_before_hash_or_normalize(self):
        _, _, _, prior, raw, args = self.fixture()
        for value in (float('nan'), ('tuple',), {1: 'coerced'}, '\ud800'):
            with self.subTest(value=repr(value)):
                with self.assertRaises(knowledge.AddressedUpdateError):
                    replace_direct_carrier_candidate(prior, {**raw, 'extension': value}, **args)
        with patch.object(knowledge, '_normalize_node', side_effect=AssertionError('must not normalize')):
            with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'byte budget'):
                replace_direct_carrier_candidate(prior, raw, **args, limits=ReplacementLimits(max_input_bytes=1))
        cycle = []
        cycle.append(cycle)
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'depth budget'):
            replace_direct_carrier_candidate(prior, {**raw, 'extension': cycle}, **args)

    def test_candidate_does_not_touch_ambient_processing_cache(self):
        _, _, _, prior, raw, args = self.fixture()
        sentinel = object()
        token = active_cache.set(sentinel)
        try:
            candidate = replace_direct_carrier_candidate(prior, raw, **args)
            self.assertIs(active_cache.get(), sentinel)
            self.assertFalse(candidate['scope']['published'])
            with self.assertRaises(knowledge.AddressedUpdateError):
                replace_direct_carrier_candidate(prior, raw, **{**args, 'endpoint_nodes': []})
            self.assertIs(active_cache.get(), sentinel)
        finally:
            active_cache.reset(token)

    def test_self_loop_and_custom_relation_identity_preserve_exact_rows(self):
        _, _, _, prior, raw, args = self.fixture()
        edge = copy.deepcopy(args['incident_relations'][0]['source_record']['payload'])
        edge.update(from_id='a', to_id='a')
        entries, mappings, fallback = knowledge._relation_registry_indexes(self.relations)
        loop = knowledge._normalize_relation(edge, 'philosophy', {prior['id']: prior},
            identity_id='pack:edge', relation_type_entries=entries,
            relation_type_mappings=mappings, fallback_relation_type_id=fallback)
        args.update(incident_relations=[loop], endpoint_nodes=[])
        result = replace_direct_carrier_candidate(prior, raw, **args)
        self.assertEqual(len(result['relations']), 1)
        self.assertEqual(result['relations'][0]['id'], 'philosophy:pack:edge')
        self.assertEqual(result['relations'][0]['from_id'], result['relations'][0]['to_id'])
        self.assertEqual(result['node']['view_ids'].count('inherited-only'), 1)

    def test_dossier_and_claim_kinds_require_stronger_owner(self):
        for source in ('source-navigation', 'source-claims'):
            raw = {'node_id': 'work', 'node_kind': 'work', 'properties': {'object_ref': 'tos.work.example'}}
            prior = knowledge._normalize_node(raw, source, source_dossier_ref='tos.work.example')
            args = dict(incident_relations=[], endpoint_nodes=[], entity_registry={}, relation_registry={},
                        normalization_binding=knowledge._normalization_binding({}, {}))
            with self.subTest(source=source):
                changed = copy.deepcopy(raw)
                changed['properties']['object_ref'] = 'tos.work.other'
                with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'dossier binding changed'):
                    replace_direct_carrier_candidate(prior, changed, **args)
                # Match the existing source-owner candidate extraction, then
                # exercise both protected kinds without a full source build.
                prior['source_dossier_ref'] = knowledge._source_dossier_candidate(raw, source)
                for kind in ('claim', 'record-version'):
                    changed = {**raw, 'node_kind': kind}
                    with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'claim and record-version'):
                        replace_direct_carrier_candidate(prior, changed, **args)

    def test_self_consistent_native_identity_drift_is_not_relocated(self):
        _, _, _, prior, raw, args = self.fixture()
        relation = args['incident_relations'][0]
        relation['native_id'] = 'other'
        knowledge._stamp_content_revision(relation)
        with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'native identity differs'):
            replace_direct_carrier_candidate(prior, raw, **args)

    def test_candidate_never_substitutes_for_wrapper_global_validation(self):
        _, _, graph, prior, raw, args = self.fixture()
        raw['properties']['original_node_type'] = 'a-new-source-kind'
        with patch.object(knowledge, 'validate_knowledge_semantics',
                          return_value={'valid': False, 'violations': ['global owner constraint']}) as check:
            result = replace_direct_carrier_candidate(prior, raw, **args)
            check.assert_not_called()
            self.assertEqual(result['node']['kind_id'], 'a-new-source-kind')
            self.assertFalse(result['scope']['global_semantics_validated'])
            with self.assertRaisesRegex(knowledge.AddressedUpdateError, 'global semantic validation'):
                knowledge.addressed_update_knowledge_graph(graph, 'philosophy', 'a', raw,
                    self.entities, self.relations, source_revision='a' * 64)
            check.assert_called_once()


if __name__ == '__main__':
    unittest.main()

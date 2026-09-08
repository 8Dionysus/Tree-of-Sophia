from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from collections import Counter
from contextlib import contextmanager
from unittest.mock import patch
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from source_witness_bibliographic_graph_common import (  # noqa: E402
    CLAIM_CATALOG_REF,
    GRAPH_PATH,
    BibliographicGraphBuildError,
    _load_claim_catalog,
    _projection_fingerprint,
    _validate_cross_references,
    build_payload,
    build_claim_navigation_descriptor,
    canonical_digest,
    load_claim_navigation_registry,
    load_verified_projection,
    query_projection,
    render_payload,
)
from source_witness_human_forms import load_metadata_forms, materialize_metadata_forms


class SourceWitnessBibliographicGraphTest(unittest.TestCase):
    @contextmanager
    def claim_navigation_fixture(self):
        """Only synthetic Claim associations; names retain their source bytes."""
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            projection = rebuild()
            nodes = {node['node_id']: node for node in projection['nodes']}
            claim = claims[0]
            subject = nodes['identity:' + claim['subject_ref']]
            target = nodes['identity:' + claim['object']]
            relations = load_claim_navigation_registry(root)
            entities = json.loads((root / 'ToS/doctrine/semantic-interchange/entity-types.v1.json').read_text())
            yield root, claim, subject, target, relations, entities, projection

    def test_claim_navigation_legacy_build_is_bound_and_does_not_change_source(self):
        with self.claim_navigation_fixture() as (root, claim, subject, target, relations, entities, projection):
            descriptor = build_claim_navigation_descriptor(claim, subject, target, relations, entities)
            carrier = next(node for node in projection['nodes'] if node['node_id'] == 'claim:' + claim['claim_id'])
            self.assertEqual(carrier['properties']['source_claim'], claim)
            self.assertEqual(carrier['properties']['navigation_descriptor'], descriptor)
            self.assertEqual(descriptor['state'], 'ready')
            self.assertIsNone(descriptor['reason'])
            self.assertFalse(descriptor['standalone'])
            self.assertEqual(descriptor['purpose'], 'claim-navigation-only')
            self.assertNotIn('human_forms', descriptor)
            for field, node in (('subject', subject), ('object', target)):
                self.assertEqual(descriptor[field]['label'], node['properties']['source_record']['preferred_label'])
                self.assertEqual(descriptor[field]['sha256'], canonical_digest(node['properties']['source_record']))
                self.assertEqual(descriptor[field]['label_pointer'], '/preferred_label')
            self.assertEqual(descriptor['claim']['sha256'], canonical_digest(claim))
            self.assertEqual(descriptor['template']['sha256'], canonical_digest(relations['claim_navigation_template']))
            self.assertEqual(descriptor['title']['default'], descriptor['title']['ru'])
            self.assertTrue(descriptor['title']['ru'].startswith('Запись утверждения'))
            self.assertIn('исходная запись не оценена', descriptor['title']['ru'])
            for ref in ('ToS/doctrine/semantic-interchange/relation-types.v1.json',
                        'ToS/contracts/semantic-relation-type-registry.schema.json'):
                self.assertEqual(projection['input_digests'][ref], hashlib.sha256((root / ref).read_bytes()).hexdigest())
            unrelated = copy.deepcopy(relations)
            unrelated['registry_version'] += 1
            unrelated['relations'][-1]['definition'] += ' Synthetic unrelated edit.'
            self.assertEqual(descriptor, build_claim_navigation_descriptor(claim, subject, target, unrelated, entities))
            changed = copy.deepcopy(claim)
            changed['qualifiers']['x-unknown'] = True
            newer = build_claim_navigation_descriptor(changed, subject, target, relations, entities)
            self.assertNotEqual(newer['claim']['sha256'], descriptor['claim']['sha256'])
            self.assertEqual(newer['title'], descriptor['title'])
            del relations['claim_navigation_template']
            self.assertIsNone(build_claim_navigation_descriptor(claim, subject, target, relations, entities))
            (root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').write_text(json.dumps(relations))
            without = build_payload(root)
            self.assertTrue(all('navigation_descriptor' not in node['properties'] for node in without['nodes']))

    def test_claim_navigation_failure_reasons_and_priority_are_closed(self):
        with self.claim_navigation_fixture() as (_root, claim, subject, target, relations, entities, _projection):
            def outcome(c=claim, s=subject, o=target, r=relations, e=entities):
                result = build_claim_navigation_descriptor(c, s, o, r, e)
                if result['state'] == 'unavailable':
                    self.assertEqual(set(result), {'schema_version', 'purpose', 'standalone', 'state',
                                                   'reason', 'template', 'claim'})
                return result['reason']

            for predicate in ('unknown-predicate', None, False, []):
                self.assertEqual(outcome(c={**claim, 'predicate': predicate}), 'predicate-not-understood')
            ambiguous = copy.deepcopy(relations)
            relation = next(entry for entry in ambiguous['relations'] if entry['relation_type_id'] == 'tos.relation.historical-participant')
            relation['source_mappings'].append(copy.deepcopy(relation['source_mappings'][0]))
            self.assertEqual(outcome(r=ambiguous), 'predicate-not-understood')
            for field, value in (('abstract', True), ('assertion_mode', 'structural')):
                altered = copy.deepcopy(relations)
                entry = next(entry for entry in altered['relations'] if entry['relation_type_id'] == relation['relation_type_id'])
                entry[field] = value
                self.assertEqual(outcome(r=altered), 'predicate-not-understood')
            self.assertEqual(outcome(c={**claim, 'object': {'literal': 'test'}}), 'object-not-identity')
            self.assertEqual(outcome(o={**target, 'node_kind': 'literal'}), 'object-not-identity')
            wrong_type = copy.deepcopy(target)
            wrong_type['properties']['identity_kind'] = 'work'
            self.assertEqual(outcome(o=wrong_type), 'endpoint-type-not-understood')
            no_type = copy.deepcopy(target)
            del no_type['properties']['identity_kind']
            self.assertEqual(outcome(o=no_type), 'endpoint-type-not-understood')
            self.assertEqual(outcome(o={**target, 'node_id': 'identity:wrong'}), 'source-name-unavailable')
            # Every earlier failure retains priority when later inputs also fail.
            self.assertEqual(outcome(c={**claim, 'predicate': None, 'object': {}, 'review_status': None},
                                     o={**target, 'node_kind': 'literal'}), 'predicate-not-understood')
            self.assertEqual(outcome(c={**claim, 'object': {}, 'review_status': None}), 'object-not-identity')
            self.assertEqual(outcome(c={**claim, 'review_status': None}, o=wrong_type), 'endpoint-type-not-understood')
            self.assertEqual(outcome(c={**claim, 'review_status': None},
                                     o={**target, 'source_sha256': '0' * 64}), 'source-name-unavailable')
            for field in ('epistemic_status', 'review_status'):
                for value in (None, False, 3, [], {}, 'unknown'):
                    self.assertEqual(outcome(c={**claim, field: value}), 'source-status-unavailable')
                missing = dict(claim)
                del missing[field]
                self.assertEqual(outcome(c=missing), 'source-status-unavailable')
            for epistemic in relations['claim_navigation_template']['status_labels']['epistemic_status']:
                for review in relations['claim_navigation_template']['status_labels']['review_status']:
                    changed = {**claim, 'epistemic_status': epistemic, 'review_status': review}
                    self.assertIsNone(outcome(c=changed))

    def test_claim_navigation_names_are_complete_source_bound_pointers(self):
        with self.claim_navigation_fixture() as (_root, claim, subject, target, relations, entities, _projection):
            def render(node):
                return build_claim_navigation_descriptor(claim, subject, node, relations, entities)

            for value in ('', '   ', target['properties']['identity_ref'], target['source_ref'], None, {}):
                node = copy.deepcopy(target)
                node['properties']['source_record']['preferred_label'] = value
                node['properties']['preferred_label'] = value
                node['source_sha256'] = canonical_digest(node['properties']['source_record'])
                self.assertEqual(render(node)['reason'], 'source-name-unavailable')
            for mutation in ('truncated', 'version', 'digest', 'source-record', 'source-ref', 'identity'):
                node = copy.deepcopy(target)
                if mutation == 'truncated':
                    node['properties']['preferred_label'] = node['properties']['preferred_label'][:3]
                elif mutation == 'version':
                    node['properties']['source_record']['record_version'] = True
                    node['source_sha256'] = canonical_digest(node['properties']['source_record'])
                elif mutation == 'digest':
                    node['properties']['source_record']['notes'] = 'Changed full source bytes.'
                elif mutation == 'source-record':
                    del node['properties']['source_record']
                elif mutation == 'source-ref':
                    node['source_ref'] = ''
                else:
                    node['properties']['source_record']['record_id'] = 'wrong'
                    node['source_sha256'] = canonical_digest(node['properties']['source_record'])
                self.assertEqual(render(node)['reason'], 'source-name-unavailable')
            for field, value in (('schema_version', []), ('schema_version', ''),
                                 ('notes', float('nan')), ('notes', '\ud800')):
                node = copy.deepcopy(target)
                node['properties']['source_record'][field] = value
                self.assertEqual(render(node)['reason'], 'source-name-unavailable')
            node = copy.deepcopy(target)
            label = '  Полное имя ${not_code} {subject-label} <script>\nнесокращённое  '
            node['properties']['source_record']['preferred_label'] = label
            node['properties']['source_record']['notes'] = 'Narrative is not a navigation name.'
            node['properties']['preferred_label'] = label
            node['properties']['label_source_pointer'] = '/preferred_label'
            node['source_sha256'] = canonical_digest(node['properties']['source_record'])
            descriptor = render(node)
            self.assertEqual(descriptor['object']['label'], label)
            self.assertIn(label, descriptor['title']['ru'])
            for pointer in ('preferred_label', '/missing', '/a~1b/~0key/00', '/a~2b', '/notes'):
                node['properties']['label_source_pointer'] = pointer
                self.assertEqual(render(node)['reason'], 'source-name-unavailable')
            node['properties']['preferred_label'] = node['properties']['source_record']['notes']
            self.assertEqual(render(node)['reason'], 'source-name-unavailable')

    def test_claim_navigation_predicate_wording_and_utf8_budget_are_exact(self):
        with self.claim_navigation_fixture() as (_root, claim, subject, target, relations, entities, _projection):
            relation = next(entry for entry in relations['relations'] if entry['relation_type_id'] == 'tos.relation.historical-participant')
            mapping = relation['source_mappings'][0]
            mapping['labels'] = {'ru': 'точная роль', 'en': 'exact role', 'default': 'exact role'}
            descriptor = build_claim_navigation_descriptor(claim, subject, target, relations, entities)
            self.assertIn('точная роль', descriptor['title']['ru'])
            self.assertEqual(descriptor['predicate']['sha256'], canonical_digest(relation))
            self.assertEqual(descriptor['predicate']['mapping_sha256'], canonical_digest(mapping))
            mapping['labels']['ru'] = None
            self.assertEqual(build_claim_navigation_descriptor(claim, subject, target, relations, entities)['reason'],
                             'predicate-label-unavailable')
            del mapping['labels']
            relation['source_mappings'].append({**mapping, 'source_predicate_id': 'another-role'})
            self.assertEqual(build_claim_navigation_descriptor(claim, subject, target, relations, entities)['reason'],
                             'predicate-label-unavailable')
            relation['source_mappings'].pop()
            descriptor = build_claim_navigation_descriptor(claim, subject, target, relations, entities)
            size = len(json.dumps(descriptor['title'], ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode('utf-8'))
            relations['claim_navigation_template']['max_output_bytes'] = size
            self.assertEqual(build_claim_navigation_descriptor(claim, subject, target, relations, entities)['state'], 'ready')
            relations['claim_navigation_template']['max_output_bytes'] = size - 1
            unavailable = build_claim_navigation_descriptor(claim, subject, target, relations, entities)
            self.assertEqual(unavailable['reason'], 'over-budget')
            self.assertNotIn('title', unavailable)

    def test_claim_navigation_native_names_and_type_closure_are_not_inferred_from_ids(self):
        with self.claim_navigation_fixture() as (_root, claim, subject, target, relations, entities, _projection):
            relation = next(entry for entry in relations['relations'] if entry['relation_type_id'] == 'tos.relation.historical-participant')
            for schema, kind, field, pointer in (
                    ('tos_artifact_source_witness_v1', 'artifact', 'artifact_id', '/custody/inventory_numbers/0'),
                    ('tos_artifact_source_witness_v2', 'artifact', 'artifact_id', '/custody/inventory_numbers/0'),
                    ('tos_scholarly_composite_witness_v1', 'composite', 'composite_id', '/preferred_label')):
                # Identity spelling is intentionally opaque to this pure helper.
                identity = 'synthetic:opaque-endpoint'
                source = {'schema_version': schema, field: identity, 'record_version': 1,
                          'custody': {'inventory_numbers': ['Synthetic inventory 1']},
                          'preferred_label': 'Synthetic composite name'}
                label = source['custody']['inventory_numbers'][0] if kind == 'artifact' else source['preferred_label']
                node = {'node_id': 'identity:' + identity, 'node_kind': 'identity',
                        'source_ref': 'ToS/source-witnesses/synthetic/metadata.json',
                        'source_sha256': canonical_digest(source),
                        'properties': {'identity_ref': identity, 'identity_kind': kind, 'source_record': source,
                                       'preferred_label': label, 'label_source_pointer': pointer}}
                relation['range_type_ids'] = ['tos.entity.' + kind]
                adapted_claim = {**claim, 'object': identity}
                result = build_claim_navigation_descriptor(adapted_claim, subject, node, relations, entities)
                self.assertEqual(result['state'], 'ready', (schema, result))
                self.assertEqual(result['object']['label_pointer'], pointer)
                self.assertEqual(result['object']['sha256'], canonical_digest(source))
                if kind == 'artifact':
                    del node['properties']['label_source_pointer']
                    self.assertEqual(build_claim_navigation_descriptor(adapted_claim, subject, node, relations, entities)['reason'],
                                     'source-name-unavailable')
            relation['range_type_ids'] = ['tos.entity.agent']
            for mutation in ('duplicate-mapping', 'unknown-parent', 'cycle', 'abstract', 'unknown-range'):
                changed = copy.deepcopy(entities)
                entry = next(entry for entry in changed['types'] if entry['type_id'] == 'tos.entity.agent')
                changed_relations = copy.deepcopy(relations)
                if mutation == 'duplicate-mapping':
                    mapping = next(mapping for mapping in entry['source_mappings'] if mapping['source_graph'] == 'source-claims')
                    entry['source_mappings'].append(copy.deepcopy(mapping))
                elif mutation == 'unknown-parent':
                    entry['parent_type_ids'] = ['tos.entity.unknown']
                elif mutation == 'cycle':
                    entry['parent_type_ids'] = [entry['type_id']]
                elif mutation == 'abstract':
                    entry['abstract'] = True
                else:
                    next(entry for entry in changed_relations['relations'] if entry['relation_type_id'] == relation['relation_type_id'])['range_type_ids'].append('tos.entity.unknown')
                with self.subTest(mutation=mutation):
                    self.assertEqual(build_claim_navigation_descriptor(claim, subject, target, changed_relations, changed)['reason'],
                                     'endpoint-type-not-understood')

    def test_claim_navigation_registry_and_template_are_validated_for_legacy_only(self):
        with self.claim_navigation_fixture() as (root, _claim, _subject, _target, relations, _entities, _projection):
            path = root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            for mutation in ('unknown-field', 'unknown-reader', 'duplicate-slot', 'marker-later',
                             'language-map-gap', 'language-alias', 'blank-marker', 'default-language', 'bad-budget'):
                changed = copy.deepcopy(relations)
                template = changed['claim_navigation_template']
                if mutation == 'unknown-field':
                    template['execute'] = 'never'
                elif mutation == 'unknown-reader':
                    template['reader'] = 'unknown'
                elif mutation == 'duplicate-slot':
                    template['renderings']['ru'][-1] = {'slot': 'subject-label'}
                elif mutation == 'marker-later':
                    template['renderings']['ru'].insert(0, {'literal': 'before marker'})
                elif mutation == 'language-map-gap':
                    del template['status_labels']['review_status']['accepted']['en']
                elif mutation == 'language-alias':
                    template['renderings']['RU'] = template['renderings']['ru']
                elif mutation == 'blank-marker':
                    template['marker']['ru'] = '   '
                elif mutation == 'default-language':
                    template['default_language'] = 'de'
                else:
                    template['max_output_bytes'] = 1
                path.write_text(json.dumps(changed))
                with self.subTest(mutation=mutation), self.assertRaises(BibliographicGraphBuildError):
                    build_payload(root)
            path.write_text(json.dumps(relations)[:-1] + ', "registry_version": 1}')
            with self.assertRaises(BibliographicGraphBuildError):
                load_claim_navigation_registry(root)

    def test_motif_proposal_is_one_claim_over_every_typed_occurrence(self):
        """Artificial members test the grammar, not a historical motif or Sign."""
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        reader = SourceClaimProfiles(REPO_ROOT)
        members = [f'tos.occurrence.synthetic-motif-{number}' for number in range(3)]
        objects = {identity: {'record_type': 'occurrence'} for identity in members}
        claim = json.loads((REPO_ROOT / 'ToS/source-witnesses/relations/goethe-leitkultur-translatability/source-claims.jsonl').read_text().splitlines()[0])
        claim.update(schema_version='tos_source_occurrence_motif_claim_v1',
            claim_id='tos.claim.synthetic-motif', subject_ref=members[0],
            predicate='occurrence_motif_proposal', assertion_layer='semantic_interpretation',
            object={'kind': 'motif-proposal', 'members': members,
                'source_wording': {'text': 'Условная гипотеза о мотиве, а не цитата.',
                    'language': 'ru', 'script': 'Cyrl', 'wording_kind': 'research_paraphrase'},
                'proposed_signification': 'Only an artificial comparison hypothesis.',
                'grouping_basis': 'Compare these three uses without asserting identity.',
                'source_scope': 'Only these synthetic units, not the entire work.',
                'contrast': 'An alternative interpretation remains possible.',
                'limitations': 'No real source reading or semantic admission.',
                'extensions': {'members': ['tos.occurrence.inert-extension'],
                    'relative': {'anchor_ref': 'tos.occurrence.inert-anchor'}}})
        original = copy.deepcopy(claim)
        reader.validate(claim, objects)
        self.assertEqual(claim, original)
        self.assertTrue(reader.is_value(claim))
        self.assertFalse(reader.is_temporal(claim))
        self.assertEqual(reader.reference_members(claim), tuple(members))
        self.assertEqual(reader.identity_refs(claim), set(members))
        for invalid_members in (members[:1], members[:2] + [members[0]], members[1:],
                members + [f'tos.occurrence.extra-{n}' for n in range(6)],
                [members[0], False], [members[0], 'not-an-identity']):
            invalid = copy.deepcopy(claim); invalid['object']['members'] = invalid_members
            with self.subTest(members=invalid_members), self.assertRaises(SourceProfileError):
                reader.validate(invalid, objects)
        for broken_objects in ({key: row for key, row in objects.items() if key != members[2]},
                {**objects, members[2]: {'record_type': 'work'}},
                {**objects, members[2]: {'record_type': 'lexical-form'}}):
            with self.subTest(objects=broken_objects), self.assertRaises(SourceProfileError):
                reader.validate(claim, broken_objects)
        for field in ('members', 'proposed_signification', 'grouping_basis', 'source_scope', 'contrast', 'limitations'):
            invalid = copy.deepcopy(claim); invalid['object'].pop(field)
            with self.subTest(missing=field), self.assertRaises(SourceProfileError):
                reader.validate(invalid, objects)
        invalid = copy.deepcopy(claim); invalid['object']['source_wording']['wording_kind'] = 'witness_quote'
        with self.assertRaises(SourceProfileError):
            reader.validate(invalid, objects)
        # Identical value bytes do not coalesce the candidate identity.
        other = {**copy.deepcopy(claim), 'claim_id': 'tos.claim.another-synthetic-motif'}
        reader.validate(other, objects)
        self.assertNotEqual(claim['claim_id'], other['claim_id'])

    def test_motif_graph_and_compact_reading_require_all_three_native_members(self):
        from tests.test_native_text_binding import NativeTextBindingFixture, digest
        from tests.test_source_owner_record_profiles import occurrence
        from source_record_profiles import SourceClaimProfiles, SourceRecordProfiles
        with self.historical_fixture() as (root, history, real, baseline, rebuild):
            native = NativeTextBindingFixture(root)
            for name in ('source-metadata-record', 'semantic-description-record', 'occurrence-description-record',
                         'source-claim-record', 'source-structured-value', 'source-occurrence-motif-claim',
                         'semantic-relation-type-registry'):
                ref = f'ToS/contracts/{name}.schema.json'
                native.write_bytes(ref, (REPO_ROOT / ref).read_bytes())
            original_anchor, original_unit = copy.deepcopy(native.packet['anchors'][1]), copy.deepcopy(native.packet['units'][0])
            anchors, units = [], []
            # Three exact, disjoint synthetic spans, not three IDs for one use.
            for number, (start, end) in enumerate(((3, 4), (4, 6), (6, 8))):
                anchor = copy.deepcopy(original_anchor)
                anchor.update(anchor_ref=f'tos.anchor.synthetic.motif-{number}', ordinal=number + 2,
                              exact_sha256=digest(native.text[start:end].encode('utf-8')))
                anchor['selector'].update(start=start, end=end)
                unit = copy.deepcopy(original_unit)
                unit.update(unit_id='tos.text-unit.sid-' + str(number + 1) * 32,
                            ordered_anchor_refs=[anchor['anchor_ref']])
                anchors.append(anchor); units.append(unit)
            gap = copy.deepcopy(native.packet['anchors'][-1]); gap['ordinal'] = 5
            native.packet['anchors'] = [native.packet['anchors'][0], *anchors, gap]
            native.packet['units'] = units
            native.packet['segmentations'][0]['ordered_unit_refs'] = [unit['unit_id'] for unit in units]
            native.make_public()
            records, reader = {}, SourceRecordProfiles(root)
            for number, unit in enumerate(units):
                binding = {**copy.deepcopy(native.binding), 'unit_id': unit['unit_id'],
                           'ordered_anchor_refs': unit['ordered_anchor_refs']}
                record = occurrence(binding)
                record.update(record_id=f'tos.occurrence.synthetic-motif-{number}', visibility='public_metadata_only',
                              preferred_label=f'Synthetic occurrence {number}')
                reader.validate('occurrence', record)
                native.write_json(f'ToS/source-witnesses/semantic-descriptions/motif-{number}/occurrence.json', record)
                records[record['record_id']] = record
            members = list(records)
            claim = {**copy.deepcopy(baseline[0]), 'schema_version': 'tos_source_occurrence_motif_claim_v1',
                'claim_id': 'tos.claim.synthetic-motif-graph', 'subject_ref': members[0],
                'predicate': 'occurrence_motif_proposal', 'assertion_layer': 'semantic_interpretation',
                'qualifiers': {'statement': 'Условное сопоставление трёх употреблений с оговорками.',
                               'statement_language': 'ru', 'statement_script': 'Cyrl'},
                'object': {'kind': 'motif-proposal', 'members': members,
                    'source_wording': {'text': 'Synthetic three-member hypothesis, not a witness quotation.',
                        'language': 'en', 'script': 'Latn', 'wording_kind': 'research_paraphrase'},
                    'proposed_signification': 'Only a test proposal.', 'grouping_basis': 'Compare three distinct spans.',
                    'source_scope': 'This synthetic packet only.', 'contrast': 'No alternative has been assessed.',
                    'limitations': 'No actual motif, recurrence, accepted tokenization or Sign is claimed.'}}
            SourceClaimProfiles(root).validate(claim, records)
            path = 'ToS/source-witnesses/history/fixture/source-claims.jsonl'
            native.write_bytes(path, (json.dumps(claim, ensure_ascii=False) + '\n').encode('utf-8'))
            projection = rebuild()
            trace = next(row for row in projection['claim_traces'] if row['claim_ref'] == claim['claim_id'])
            self.assertEqual(set(trace['value_member_node_ids']), {'identity:' + id for id in members})
            member_edges = [edge for edge in projection['edges']
                            if edge['claim_ref'] == claim['claim_id'] and edge['edge_kind'] == 'has_value_member']
            self.assertEqual({edge['to_id'] for edge in member_edges}, set(trace['value_member_node_ids']))
            graph, _, _ = self.historical_knowledge(root, projection)
            from tos_access.knowledge import focus_knowledge_node, knowledge_scene, execute_knowledge_lens
            # The standalone access path normalizes a supplied projection without
            # reading the original repository. Removing derived member carriers
            # must not downgrade a declared reference Claim to an ordinary pair.
            for mutation in ('absent', 'null', 'empty', 'trace-removed', 'third-edge',
                             'third-node', 'third-type', 'trace-truncated', 'trace-duplicate',
                             'source-members', 'claim-value', 'literal-value', 'member-origin'):
                changed = json.loads(json.dumps(projection))  # Match independent JSON carriers on disk.
                changed_trace = next(row for row in changed['claim_traces'] if row['claim_ref'] == claim['claim_id'])
                raw_claim = next(row for row in changed['nodes'] if row['node_id'] == trace['claim_node_id'])
                raw_value = next(row for row in changed['nodes'] if row['node_id'] == trace['object_node_id'])
                third_id = 'identity:' + members[2]
                if mutation in {'absent', 'null', 'empty'}:
                    if mutation == 'absent':
                        changed_trace.pop('value_member_node_ids')
                    else:
                        changed_trace['value_member_node_ids'] = None if mutation == 'null' else []
                    changed['edges'] = [row for row in changed['edges'] if row['edge_kind'] != 'has_value_member']
                    changed_trace['edge_ids'] = [row['edge_id'] for row in changed['edges']]
                elif mutation == 'trace-removed':
                    changed['claim_traces'] = []
                elif mutation == 'third-edge':
                    changed['edges'] = [row for row in changed['edges']
                        if not (row['edge_kind'] == 'has_value_member' and row['to_id'] == third_id)]
                elif mutation == 'third-node':
                    changed['nodes'] = [row for row in changed['nodes'] if row['node_id'] != third_id]
                elif mutation == 'third-type':
                    next(row for row in changed['nodes'] if row['node_id'] == third_id)['properties']['identity_kind'] = 'work'
                elif mutation == 'trace-truncated':
                    changed_trace['value_member_node_ids'].remove(third_id)
                elif mutation == 'trace-duplicate':
                    changed_trace['value_member_node_ids'].append(third_id)
                elif mutation == 'source-members':
                    raw_claim['properties']['source_claim']['object']['members'].pop()
                elif mutation == 'claim-value':
                    raw_claim['properties']['object']['members'].pop()
                elif mutation == 'literal-value':
                    raw_value['properties']['value']['members'].pop()
                elif mutation == 'member-origin':
                    next(row for row in changed['edges'] if row['edge_kind'] == 'has_value_member')['claim_ref'] = 'tos.claim.wrong-origin'
                with self.subTest(carrier=mutation), self.assertRaises(ValueError):
                    self.historical_knowledge(root, changed)
            claim_node = next(node for node in graph['nodes'] if node['entity_id'] == claim['claim_id'])
            value = next(node for node in graph['nodes'] if node['type_id'] == 'tos.entity.motif-proposal')
            self.assertEqual(value['attributes']['value'], claim['object'])
            self.assertEqual(claim_node['attributes']['source_claim'], claim)
            self.assertEqual(set(claim_node['semantics']['claim']['value_member_node_ids']),
                             {'source-claims:identity:' + id for id in members})
            for identity in members:
                result = focus_knowledge_node(graph, identity, depth=2)
                self.assertTrue(set(members) | {claim['claim_id']} <= {node['entity_id'] for node in result['nodes']})
            relation_types = {'tos.relation.has-subject', 'tos.relation.has-object', 'tos.relation.claim-value-member'}
            relations = [relation for relation in graph['relations']
                         if relation['from_id'] == claim_node['id'] and relation['relation_type_id'] in relation_types]
            ids = {claim_node['id'], *(r['to_id'] for r in relations)}
            nodes = [node for node in graph['nodes'] if node['id'] in ids]
            scene = knowledge_scene(nodes, relations)
            compact = scene['compact']['claim_paths'][0]
            self.assertEqual(set(compact['reading']['relation_context_ids']), {row['id'] for row in relations})
            self.assertEqual(len(compact['detail_relation_ids']), 3)
            self.assertFalse(compact['reading']['standalone'])
            third_edge = next(row for row in relations if row['relation_type_id'] == 'tos.relation.claim-value-member'
                              and row['to_id'].endswith(members[2]))
            incomplete = knowledge_scene(nodes, [row for row in relations if row is not third_edge])
            self.assertEqual(incomplete['compact']['claim_paths'], [])
            self.assertIn({'node_id': claim_node['id'], 'reason': 'incomplete-value-member-context'},
                          incomplete['compact']['retained_claims'])
            for declaration in (None, [], [claim_node['id']] * 3, 'not-a-member-set'):
                changed_nodes = copy.deepcopy(nodes)
                changed_claim = next(row for row in changed_nodes if row['id'] == claim_node['id'])
                if declaration is None:
                    changed_claim['semantics']['claim'].pop('value_member_node_ids')
                else:
                    changed_claim['semantics']['claim']['value_member_node_ids'] = declaration
                with self.subTest(declaration=declaration):
                    rejected = knowledge_scene(changed_nodes, relations)
                    self.assertEqual(rejected['compact']['claim_paths'], [])
                    self.assertIn({'node_id': claim_node['id'], 'reason': 'incomplete-value-member-context'},
                                  rejected['compact']['retained_claims'])
            null_nodes = copy.deepcopy(nodes)
            next(row for row in null_nodes if row['id'] == claim_node['id'])['semantics']['claim']['value_member_node_ids'] = None
            no_member_edges = [row for row in relations if row['relation_type_id'] != 'tos.relation.claim-value-member']
            self.assertEqual(knowledge_scene(null_nodes, no_member_edges)['compact']['claim_paths'], [])
            result = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                'lens_id': 'synthetic-motif-property', 'node_query': {'filters': [
                    {'property_id': 'tos.property.motif-grouping-basis', 'op': 'eq',
                     'value': claim['object']['grouping_basis']}]}, 'relation_query': {'enabled': False}, 'detail': 'full'})
            self.assertEqual([node['id'] for node in result['nodes']], [value['id']])

    def test_lexical_translatability_keeps_judgment_transfer_search_and_wording_independent(self):
        """Artificial rendering reports test source grammar, not any word's translatability."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        with self.historical_fixture() as (root, history, real, baseline, rebuild):
            for name in ('source-metadata-record', 'semantic-description-record', 'lexical-description-record',
                         'source-claim-record', 'source-structured-value', 'source-lexical-translatability-claim',
                         'semantic-relation-type-registry', 'native-text-unit-binding'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            record_reader, records = SourceRecordProfiles(root), {}
            for kind, content in (
                    ('lexeme', {'lexical_account': 'An artificial lexical referent.',
                                'grammatical_account': 'No real grammatical attribution.'}),
                    ('sense', {'sense_account': 'An artificial situated reading.',
                               'interpretation_context': 'This synthetic rendering task only.',
                               'semantic_range': 'No actual lexical range is asserted.'})):
                record = {**copy.deepcopy(history[0][1]), 'schema_version': 'tos_lexical_description_record_v1',
                    'record_type': kind, 'record_id': f'tos.{kind}.synthetic-translatability',
                    'preferred_label': 'Условный предмет перевода', 'notes': 'Только синтетический тест.',
                    'field_languages': {field: {'language': 'ru', 'script': 'Cyrl'}
                        for field in ('preferred_label', 'notes')},
                    'semantic_scope': {'scope_note': 'One synthetic lexical referent.',
                        'identity_criterion': 'This referent is not a rendering judgment or a value.',
                        'language': 'en', 'script': 'Latn'},
                    'semantic_content': {**content, 'language': 'en', 'script': 'Latn'}}
                record_reader.validate(kind, record)
                path = root / f'ToS/source-witnesses/lexical-descriptions/translatability-{kind}/{kind}.json'
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps(record, ensure_ascii=False), encoding='utf-8')
                records[kind] = record
            # Declaring Occurrence as a domain does not supply its native evidence binding.
            self.assertEqual(record_reader.profiles['occurrence']['native_binding_adapter'], 'source-text-unit-v1')
            with self.assertRaises(SourceProfileError):
                record_reader.validate_native_binding('occurrence', {})
            objects = {record['record_id']: record for record in records.values()}
            extensions = {'date': '1886', 'relative': {'anchor_ref': 'tos.sense.not-a-dependency'},
                          'unknown': [False, 0, None], 'instruction': 'Inert synthetic source wording.'}
            value = {'kind': 'lexical-translatability',
                'source_wording': {'text': 'Rapport fictif, sans jugement sur une langue.',
                                   'language': 'fr', 'script': 'Latn'},
                'source_language': 'de', 'target_language': 'en',
                'source_scope': 'source_scope: one artificial use, not every use of a word.',
                'target_scope': 'target_scope: one artificial receiving task.',
                'aspects_in_scope': 'aspects_in_scope: only two synthetic comparison aspects.',
                'rendering_judgment': 'inadequate_in_scope', 'aspect_transfer': 'partial_for_stated_aspects',
                'renderings_considered': [
                    {'text': 'Synthetic candidate', 'language': 'en', 'script': 'Latn'},
                    {'text': 'Условный вариант', 'language': 'ru', 'script': 'Cyrl'},
                    {'text': 'Unattributed synthetic wording', 'language': None, 'script': None,
                     'extensions': copy.deepcopy(extensions)}],
                'preserved_aspects': 'preserved_aspects: one artificial aspect is retained.',
                'limitations': 'limitations: the other aspect and alternative tasks remain open.',
                'search_report': None, 'extensions': copy.deepcopy(extensions)}
            claim = {**copy.deepcopy(baseline[0]), 'schema_version': 'tos_source_lexical_translatability_claim_v1',
                'claim_id': 'tos.claim.synthetic-translatability-unreported',
                'subject_ref': records['lexeme']['record_id'], 'predicate': 'lexical_translatability',
                'object': value, 'polarity': 'positive',
                'confidence': {'value': 0, 'meaning': 'maker_declared_uncertainty_not_truth_probability'},
                'qualifiers': {'statement': 'Только условное сообщение; не универсальный языковой вердикт.',
                    'statement_language': 'ru', 'statement_script': 'Cyrl', 'unknown': copy.deepcopy(extensions)},
                'extensions': copy.deepcopy(extensions)}
            reader = SourceClaimProfiles(root)
            relation = reader.relations['lexical_translatability']
            self.assertEqual(reader.profiles['lexical_translatability']['reader'], 'structured-value-v1')
            self.assertEqual(set(relation['domain_type_ids']),
                             {'tos.entity.lexeme', 'tos.entity.lexical-sense', 'tos.entity.occurrence'})
            self.assertEqual(relation['range_type_ids'], ['tos.entity.lexical-translatability'])
            self.assertFalse(relation['transitive'])
            self.assertIn('tos.entity.literal', reader.ancestry('tos.entity.lexical-translatability'))
            self.assertNotIn('tos.entity.identity', reader.ancestry('tos.entity.lexical-translatability'))
            reader.validate(claim, objects)
            for field in (field for field in value if field != 'extensions'):
                invalid = copy.deepcopy(claim); invalid['object'].pop(field)
                with self.subTest(missing=field), self.assertRaises(SourceProfileError):
                    reader.validate(invalid, objects)
            for field in ('source_scope', 'target_scope', 'aspects_in_scope', 'preserved_aspects', 'limitations'):
                invalid = copy.deepcopy(claim); invalid['object'][field] = ' '
                with self.subTest(blank=field), self.assertRaises(SourceProfileError):
                    reader.validate(invalid, objects)
            for field in ('statement', 'statement_language', 'statement_script'):
                invalid = copy.deepcopy(claim); invalid['qualifiers'].pop(field)
                with self.subTest(missing_qualifier=field), self.assertRaises(SourceProfileError):
                    reader.validate(invalid, objects)
            for field in ('text', 'language', 'script'):
                invalid = copy.deepcopy(claim); invalid['object']['renderings_considered'][0].pop(field)
                with self.subTest(missing_candidate_field=field), self.assertRaises(SourceProfileError):
                    reader.validate(invalid, objects)
            for field, replacement in (
                    ('kind', 'textual-survival'), ('source_language', 'not a tag'),
                    ('target_language', 'en\n'), ('rendering_judgment', 'untranslatable'),
                    ('aspect_transfer', 'equivalent'), ('renderings_considered', ['Unqualified candidate']),
                    ('renderings_considered', [{'text': ' ', 'language': 'en', 'script': 'Latn'}]),
                    ('renderings_considered', [{'text': 'Synthetic', 'language': 'not a tag', 'script': 'Latn'}]),
                    ('renderings_considered', [{'text': 'Synthetic', 'language': 'en', 'script': 'Latin'}])):
                invalid = copy.deepcopy(claim); invalid['object'][field] = replacement
                with self.subTest(field=field, replacement=replacement), self.assertRaises(SourceProfileError):
                    reader.validate(invalid, objects)
            for change in ({'object': 'tos.sense.synthetic-translatability'}, {'assertion_layer': 'source_observation'},
                           {'review_status': 'accepted'}, {'evidence_refs': []}):
                with self.subTest(change=change), self.assertRaises(SourceProfileError):
                    reader.validate({**claim, **change}, objects)
            for wrong_kind in ('work', 'agent', 'lexical-form', 'language', 'crosscutting-concept'):
                with self.subTest(subject_kind=wrong_kind), self.assertRaises(SourceProfileError):
                    reader.validate(claim, {claim['subject_ref']: {'record_type': wrong_kind}})
            search = {'outcome': 'none_found', 'sought_criterion': 'criterion: both artificial aspects, not just one.',
                'coverage_note': 'coverage: only the synthetic reported comparison; no actual search run.',
                'method_note': None, 'extensions': copy.deepcopy(extensions)}
            for field in ('outcome', 'sought_criterion', 'coverage_note', 'method_note'):
                invalid = {**copy.deepcopy(value), 'search_report': {key: val for key, val in search.items() if key != field}}
                with self.subTest(missing_search=field), self.assertRaises(SourceProfileError):
                    reader.validate({**claim, 'object': invalid}, objects)
            for field in ('sought_criterion', 'coverage_note', 'method_note'):
                invalid = {**copy.deepcopy(value), 'search_report': {**search, field: ' '}}
                with self.subTest(blank_search=field), self.assertRaises(SourceProfileError):
                    reader.validate({**claim, 'object': invalid}, objects)
            for outcome in ('not_searched', 'untranslatable'):
                with self.subTest(search_outcome=outcome), self.assertRaises(SourceProfileError):
                    reader.validate({**claim, 'object': {**value, 'search_report': {**search, 'outcome': outcome}}}, objects)
            # Equal values remain two Claims; polarity is not derived from an inner judgment.
            claims = [claim, {**copy.deepcopy(claim), 'claim_id': 'tos.claim.synthetic-translatability-denial',
                             'polarity': 'negative', 'epistemic_status': 'disputed'}]
            claims.append({**copy.deepcopy(claim), 'claim_id': 'tos.claim.synthetic-translatability-none-found',
                'assertion_layer': 'translation_judgment', 'object': {**copy.deepcopy(value),
                    'source_language': None, 'target_language': 'und', 'rendering_judgment': 'adequate_in_scope',
                    'search_report': copy.deepcopy(search)}})
            claims.append({**copy.deepcopy(claim), 'claim_id': 'tos.claim.synthetic-translatability-undetermined',
                'polarity': 'unknown', 'object': {**copy.deepcopy(value), 'rendering_judgment': 'undetermined',
                    'aspect_transfer': 'undetermined', 'renderings_considered': [],
                    'search_report': {**copy.deepcopy(search), 'outcome': 'undetermined',
                        'method_note': 'method: an attributed synthetic comparison, not executed here.'}}})
            claims.append({**copy.deepcopy(claim), 'claim_id': 'tos.claim.synthetic-translatability-candidates-found',
                'subject_ref': records['sense']['record_id'], 'assertion_layer': 'linguistic_analysis',
                'object': {**copy.deepcopy(value), 'rendering_judgment': 'adequate_in_scope',
                    'aspect_transfer': 'full_for_stated_aspects',
                    'search_report': {**copy.deepcopy(search), 'outcome': 'candidates_found'}}})
            for transfer in ('full_for_stated_aspects', 'partial_for_stated_aspects', 'none_for_stated_aspects', 'undetermined'):
                reader.validate({**claim, 'object': {**value, 'aspect_transfer': transfer}}, objects)
            for language in (None, 'und'):
                reader.validate({**claim, 'object': {**value, 'source_language': language, 'target_language': language,
                    'renderings_considered': [{'text': 'Synthetic wording', 'language': language, 'script': None}]}}, objects)
            for item in claims:
                original = copy.deepcopy(item)
                reader.validate(item, objects)
                self.assertEqual(item, original)
                self.assertEqual(reader.identity_refs(item), {item['subject_ref']})
                self.assertTrue(reader.is_value(item))
                self.assertFalse(reader.is_temporal(item))
            path = root / 'ToS/source-witnesses/history/fixture/source-claims.jsonl'
            path.write_text(''.join(json.dumps(item, ensure_ascii=False) + '\n' for item in claims), encoding='utf-8')
            projection = rebuild()
            graph, _, _ = self.historical_knowledge(root, projection)
            from tos_access.knowledge import execute_knowledge_lens, focus_knowledge_node, inspect_knowledge_node
            values = [node for node in graph['nodes'] if node['type_id'] == 'tos.entity.lexical-translatability']
            self.assertEqual(len(values), len(claims))
            self.assertEqual(len({node['entity_id'] for node in values}), len(claims))
            self.assertEqual(len([node for node in values if node['attributes']['value'] == value]), 2)
            self.assertFalse(projection['relation_model']['direct_subject_object_edges'])
            claims_by_id = {item['claim_id']: item for item in claims}
            values_by_claim = {}
            for node in values:
                contexts = [context for context in node['semantics']['assertion_contexts']
                            if context['binding_role'] == 'referenced-claim']
                self.assertEqual(len(contexts), 1)
                fields = contexts[0]['fields']
                item = claims_by_id[fields['claim_id']['value']]
                values_by_claim[item['claim_id']] = node
                self.assertEqual(node['attributes']['value'], item['object'])
                self.assertNotIn('time', node['semantics'])
                self.assertEqual(node['display']['title']['fr'], item['object']['source_wording']['text'])
                for field in ('object', 'polarity', 'epistemic_status', 'review_status', 'qualifiers', 'confidence'):
                    self.assertEqual(fields[field]['value'], item[field])
                inspected = inspect_knowledge_node(graph, node['entity_id'])
                self.assertEqual(inspected['matches'], [node])
                focused = focus_knowledge_node(graph, node['entity_id'], depth=2)
                self.assertIn(item['subject_ref'], {entry['entity_id'] for entry in focused['nodes']})
                focused_value = next(entry for entry in focused['nodes'] if entry['id'] == node['id'])
                self.assertEqual(focused_value['attributes']['value'], item['object'])
                self.assertEqual(focused_value['semantics']['assertion_contexts'], node['semantics']['assertion_contexts'])
                claim_node = next(entry for entry in focused['nodes'] if entry['entity_id'] == item['claim_id'])
                self.assertEqual(claim_node['attributes']['source_claim'], item)
                inspected_claim = inspect_knowledge_node(graph, item['claim_id'])['matches'][0]
                self.assertEqual(inspected_claim['attributes']['source_claim'], item)
                self.assertEqual(inspected_claim['semantics']['assertion_contexts'],
                                 claim_node['semantics']['assertion_contexts'])
                vertex = next(entry for entry in focused['scene']['vertices']
                              if entry['id'] == focused['scene']['focus_vertex_id'])
                self.assertIsNone(vertex['entity_id'])
            property_paths = {field.replace('_', '-'): (field,) for field in (
                'source_language', 'target_language', 'source_scope', 'target_scope', 'aspects_in_scope',
                'rendering_judgment', 'aspect_transfer', 'preserved_aspects', 'limitations')}
            property_paths.update({f'search-{name}': ('search_report', field) for name, field in (
                ('outcome', 'outcome'), ('criterion', 'sought_criterion'),
                ('coverage', 'coverage_note'), ('method', 'method_note'))})
            def property_value(item, keys):
                result = item['object']
                for key in keys:
                    result = result.get(key) if isinstance(result, dict) else None
                return result
            for suffix, keys in property_paths.items():
                expected_value = next(property_value(item, keys) for item in claims if property_value(item, keys) is not None)
                spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'synthetic-translatability-property',
                    'node_query': {'filters': [{'property_id': 'tos.property.translatability-' + suffix,
                                               'op': 'eq', 'value': expected_value}]},
                    'relation_query': {'enabled': False}, 'detail': 'full'}
                with self.subTest(property=suffix):
                    result = execute_knowledge_lens(graph, spec)
                    self.assertEqual({node['entity_id'] for node in result['nodes']},
                        {values_by_claim[item['claim_id']]['entity_id'] for item in claims
                         if property_value(item, keys) == expected_value})
                    absent = copy.deepcopy(spec)
                    absent['node_query']['filters'][0]['value'] = 'absent synthetic translatability value'
                    self.assertEqual(execute_knowledge_lens(graph, absent)['nodes'], [])
            for outcome in ('none_found', 'undetermined', 'candidates_found'):
                result = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                    'lens_id': 'synthetic-reported-search-outcome', 'node_query': {'filters': [{
                        'property_id': 'tos.property.translatability-search-outcome', 'op': 'eq', 'value': outcome}]},
                    'relation_query': {'enabled': False}, 'detail': 'full'})
                self.assertEqual({node['entity_id'] for node in result['nodes']},
                    {values_by_claim[item['claim_id']]['entity_id'] for item in claims
                     if property_value(item, ('search_report', 'outcome')) == outcome})

    def test_lexical_comparison_claims_keep_history_and_translation_source_bound(self):
        """Synthetic comparisons test grammar and reading, never etymological truth."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        cases = {
            'lexical_inherited_from': ('lexeme', ('chronology_basis',)),
            'lexical_borrowed_from': ('lexeme', ('chronology_basis',)),
            'lexical_formed_from': ('lexeme', ('chronology_basis',)),
            'lexical_cognate_with': ('lexeme', ('common_origin_basis',)),
            'lexical_sense_developed_from': ('sense', ('chronology_basis',)),
            'lexical_translation_correspondence': ('sense', ('translation_scope', 'preserved_aspects', 'limitations')),
        }
        self.assertTrue(set(cases).issubset(SourceClaimProfiles(REPO_ROOT).profiles),
                        'The six lexical comparison predicates need declared source Claim profiles.')
        common = {'statement': 'Только синтетическое сравнение; не принятое суждение о языке.',
            'statement_language': 'ru', 'statement_script': 'Cyrl',
            'relation_basis': 'An artificial comparison, not an inference from matching labels.',
            'attestation_scope': 'Only this synthetic research fixture; no actual token is asserted.',
            'source_scope': 'The bounded source use in the artificial comparison.',
            'target_scope': 'The bounded target use; unrelated senses remain open.',
            'source_language': 'de', 'target_language': 'en',
            'unknown_extension': {'relative': {'anchor_ref': 'tos.sense.not-a-dependency'}, 'values': [False, None]}}
        with self.historical_fixture() as (root, history, real, baseline, rebuild):
            for name in ('source-metadata-record', 'semantic-description-record', 'lexical-description-record',
                         'source-claim-record', 'semantic-relation-claim', 'linguistic-relation-claim',
                         'lexical-comparison-claim', 'semantic-relation-type-registry'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            records = {}
            record_reader = SourceRecordProfiles(root)
            for kind in ('lexeme', 'sense'):
                content = ({'lexical_account': 'An artificial lexical referent.',
                            'grammatical_account': 'A synthetic analysis, not accepted grammar.'} if kind == 'lexeme' else
                           {'sense_account': 'An artificial situated reading.',
                            'interpretation_context': 'This synthetic context only.',
                            'semantic_range': 'Other readings are not excluded.'})
                for suffix in ('a', 'b', 'c'):
                    record = {**copy.deepcopy(history[0][1]),
                        'schema_version': 'tos_lexical_description_record_v1', 'record_type': kind,
                        'record_id': f'tos.{kind}.synthetic-comparison-{suffix}',
                        'preferred_label': 'Одинаковая метка', 'notes': 'Синтетический предмет сравнения.',
                        'field_languages': {field: {'language': 'ru', 'script': 'Cyrl'}
                            for field in ('preferred_label', 'notes')},
                        'semantic_scope': {'scope_note': 'One artificial research referent.',
                            'identity_criterion': 'Description correction preserves this referent, not a historical transition.',
                            'language': 'en', 'script': 'Latn'},
                        'semantic_content': {**content, 'language': 'en', 'script': 'Latn'}}
                    record_reader.validate(kind, record)
                    path = root / f'ToS/source-witnesses/lexical-descriptions/comparison-{kind}-{suffix}/{kind}.json'
                    path.parent.mkdir(parents=True)
                    path.write_text(json.dumps(record, ensure_ascii=False), encoding='utf-8')
                    records[kind, suffix] = record
            objects = {record['record_id']: record for record in records.values()}
            reader, claims = SourceClaimProfiles(root), []
            for predicate, (kind, required) in cases.items():
                qualifiers = {**copy.deepcopy(common), **{field: f'{field}: A scoped synthetic account; alternatives remain open.'
                                                         for field in required}}
                claim = {**copy.deepcopy(baseline[0]), 'schema_version': 'tos_semantic_relation_claim_v1',
                    'claim_id': 'tos.claim.synthetic-' + predicate.replace('_', '-'),
                    'subject_ref': records[kind, 'a']['record_id'], 'object': records[kind, 'b']['record_id'],
                    'predicate': predicate, 'assertion_layer': 'linguistic_analysis', 'polarity': 'positive',
                    'qualifiers': qualifiers}
                with self.subTest(predicate=predicate):
                    reader.validate(claim, objects)
                    relation = reader.relations[predicate]
                    expected_type = 'tos.entity.' + ('lexical-sense' if kind == 'sense' else kind)
                    self.assertEqual(relation['relation_type_id'], 'tos.relation.' + predicate.replace('_', '-'))
                    self.assertEqual(relation['domain_type_ids'], [expected_type])
                    self.assertEqual(relation['range_type_ids'], [expected_type])
                    self.assertEqual(relation['directionality'], 'symmetric' if predicate == 'lexical_cognate_with' else 'directed')
                    self.assertFalse(relation['transitive'])
                    self.assertIsNone(relation['cardinality']['per_subject_max'])
                    self.assertIsNone(relation['cardinality']['per_object_max'])
                    self.assertEqual(reader.profiles[predicate]['reader'], 'semantic-relation-v1')
                    self.assertEqual(reader.identity_refs(claim), {claim['subject_ref'], claim['object']})
                for field in (*(field for field in common if field != 'unknown_extension'), *required):
                    invalid = copy.deepcopy(claim); invalid['qualifiers'].pop(field)
                    with self.subTest(predicate=predicate, missing=field), self.assertRaises(SourceProfileError):
                        reader.validate(invalid, objects)
                for field in ('source_scope', 'target_scope', *required):
                    invalid = copy.deepcopy(claim); invalid['qualifiers'][field] = ' '
                    with self.subTest(predicate=predicate, empty=field), self.assertRaises(SourceProfileError):
                        reader.validate(invalid, objects)
                for field in ('source_language', 'target_language'):
                    for value in (None, 'und'):
                        reader.validate({**claim, 'qualifiers': {**qualifiers, field: value}}, objects)
                    with self.subTest(predicate=predicate, bad_language=field), self.assertRaises(SourceProfileError):
                        reader.validate({**claim, 'qualifiers': {**qualifiers, field: 'not a language tag'}}, objects)
                other_kind = 'sense' if kind == 'lexeme' else 'lexeme'
                for endpoint in ('subject_ref', 'object'):
                    with self.subTest(predicate=predicate, endpoint=endpoint), self.assertRaises(SourceProfileError):
                        reader.validate({**claim, endpoint: records[other_kind, 'a']['record_id']}, objects)
                for change in ({'assertion_layer': 'source_observation'}, {'review_status': 'accepted'}, {'evidence_refs': []}):
                    with self.subTest(predicate=predicate, change=change), self.assertRaises(SourceProfileError):
                        reader.validate({**claim, **change}, objects)
                # The same pair supports distinct proposals, denials and uncertainty, not merged truth.
                claims.extend({**copy.deepcopy(claim), 'claim_id': claim['claim_id'] + '.' + polarity, 'polarity': polarity}
                              for polarity in ('positive', 'negative', 'unknown'))
            # Multiple formation inputs and a proposed chain remain only their explicit Claims.
            for predicate, source, target, suffix in (
                    ('lexical_formed_from', 'a', 'c', 'second-component'),
                    ('lexical_cognate_with', 'b', 'c', 'second-pair')):
                template = next(claim for claim in claims if claim['predicate'] == predicate)
                extra = {**copy.deepcopy(template), 'claim_id': template['claim_id'] + '.' + suffix,
                         'subject_ref': records['lexeme', source]['record_id'], 'object': records['lexeme', target]['record_id']}
                reader.validate(extra, objects)
                claims.append(extra)
            # Unknown compared language is independent of the proposal's polarity.
            unknown_language = {**copy.deepcopy(claims[0]), 'claim_id': 'tos.claim.synthetic-comparison-unknown-language',
                'qualifiers': {**copy.deepcopy(claims[0]['qualifiers']), 'source_language': None, 'target_language': 'und'}}
            reader.validate(unknown_language, objects)
            claims.append(unknown_language)
            claim_path = root / 'ToS/source-witnesses/lexical-descriptions/comparison-claims/source-claims.jsonl'
            claim_path.parent.mkdir(parents=True)
            claim_path.write_text(''.join(json.dumps(claim, ensure_ascii=False) + '\n' for claim in claims), encoding='utf-8')
            projection = rebuild()
            graph, _, _ = self.historical_knowledge(root, projection)
            projected = {node['entity_id']: node for node in graph['nodes']
                         if node['entity_id'] in {claim['claim_id'] for claim in claims}}
            traces = [trace for trace in projection['claim_traces'] if trace['predicate'] in cases]
            self.assertEqual({trace['claim_ref'] for trace in traces}, {claim['claim_id'] for claim in claims})
            self.assertEqual(len(traces), len(claims))
            self.assertFalse(projection['relation_model']['direct_subject_object_edges'])
            for claim in claims:
                node = projected[claim['claim_id']]
                self.assertEqual(node['attributes']['source_claim'], claim)
                self.assertEqual(node['semantics']['claim']['relation_type_id'],
                                 'tos.relation.' + claim['predicate'].replace('_', '-'))
            for record in records.values():
                node = next(node for node in graph['nodes'] if node['entity_id'] == record['record_id'])
                self.assertEqual(node['attributes']['source_record'], record)
                self.assertNotIn('time', node['semantics'])
            from tos_access.knowledge import execute_knowledge_lens
            for field in ('source_scope', 'target_scope', 'source_language', 'target_language', 'chronology_basis',
                          'common_origin_basis', 'translation_scope', 'preserved_aspects', 'limitations'):
                value = next(claim['qualifiers'][field] for claim in claims if field in claim['qualifiers'])
                property_id = 'tos.property.claim-lexical-' + field.replace('_', '-')
                spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'synthetic-lexical-comparison-property',
                    'node_query': {'filters': [{'property_id': property_id, 'op': 'eq', 'value': value}]},
                    'relation_query': {'enabled': False}, 'detail': 'full'}
                with self.subTest(property_id=property_id):
                    result = execute_knowledge_lens(graph, spec)
                    expected = {claim['claim_id'] for claim in claims if claim['qualifiers'].get(field) == value}
                    self.assertEqual({node['entity_id'] for node in result['nodes']}, expected)
                    for node in result['nodes']:
                        self.assertEqual(node['attributes']['source_claim'], next(
                            claim for claim in claims if claim['claim_id'] == node['entity_id']))
                    absent = copy.deepcopy(spec)
                    absent['node_query']['filters'][0]['value'] = 'absent synthetic lexical comparison value'
                    self.assertEqual(execute_knowledge_lens(graph, absent)['nodes'], [])
            unknown_spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'synthetic-unknown-compared-language',
                'node_query': {'filters': [{'property_id': 'tos.property.claim-lexical-target-language',
                                           'op': 'eq', 'value': 'und'}]},
                'relation_query': {'enabled': False}, 'detail': 'full'}
            self.assertEqual({node['entity_id'] for node in execute_knowledge_lens(graph, unknown_spec)['nodes']},
                             {unknown_language['claim_id']})

    def test_lexical_profiles_preserve_form_lexeme_sense_and_competing_readings(self):
        """Artificial lexical data; spelling is neither identity nor attestation."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        profiles = SourceRecordProfiles(REPO_ROOT)
        contents = {
            'lexeme': {'lexical_account': 'A synthetic lexical referent, not one spelling.',
                       'grammatical_account': 'Synthetic nominal analysis; no accepted grammar.'},
            'lexical-form': {'form_account': 'The supplied decomposed spelling, not an occurrence.'},
            'sense': {'sense_account': 'A synthetic contextual reading, not a concept.',
                      'interpretation_context': 'Only this synthetic interpretation.',
                      'semantic_range': 'Other readings remain open, not disproved.'},
        }
        with self.historical_fixture() as (root, history, real, baseline, rebuild):
            for name in ('source-metadata-record', 'semantic-description-record', 'lexical-description-record',
                         'semantic-relation-claim', 'linguistic-relation-claim', 'source-claim-record',
                         'semantic-relation-type-registry'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            from source_commands import prepare_metadata_change, _apply, REVISION_FIELDS
            from knowledge_assessment import Record
            self.assertNotIn('form_identity', REVISION_FIELDS)
            records = {}
            for kind, content in contents.items():
                source = {**copy.deepcopy(history[0][1]), 'schema_version': 'tos_lexical_description_record_v1',
                    'record_type': kind, 'record_id': f'tos.{kind}.synthetic-lexical',
                    'preferred_label': 'Одинаковое имя', 'notes': 'Синтетический предмет; не свидетельство употребления.',
                    'field_languages': {key: {'language': 'ru', 'script': 'Cyrl'} for key in ('preferred_label', 'notes')},
                    'semantic_scope': {'scope_note': 'One artificial research referent.',
                        'identity_criterion': 'Corrected accounts retain this referent; matching strings do not merge it.',
                        'language': 'en', 'script': 'Latn'},
                    'semantic_content': {**content, 'language': 'en', 'script': 'Latn',
                        'unknown_extension': {'future_reading': ['𒀀', False, None]}},
                    'extensions': {'language_code': 'x-test', 'instructions': 'Inert source text'}}
                if kind == 'lexical-form':
                    source['form_identity'] = {'written_representation': 'e\u0301', 'language': 'x-test', 'script': 'Latn',
                        'representation_kind': 'orthographic', 'notation_scope': 'Only the synthetic notation.',
                        'unicode_posture': 'preserved_as_supplied'}
                profiles.validate(kind, source)
                self.assertEqual(profiles.profiles[kind]['id_prefix'], f'tos.{kind}.')
                for field in (*content, 'language', 'script'):
                    invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                    with self.subTest(kind=kind, missing=field), self.assertRaises(SourceProfileError):
                        profiles.validate(kind, invalid)
                for change in ({'record_id': 'tos.form.synthetic-lexical'}, {'record_id': 'tos.concept.synthetic'},
                               {'notes': ' '}, {'lexeme_ref': 'tos.lexeme.synthetic'}, {'semantic_scope': {}}):
                    with self.subTest(kind=kind, change=change), self.assertRaises(SourceProfileError):
                        profiles.validate(kind, {**source, **change})
                if kind == 'lexical-form':
                    for field in source['form_identity']:
                        invalid = copy.deepcopy(source); invalid['form_identity'].pop(field)
                        with self.subTest(form_missing=field), self.assertRaises(SourceProfileError):
                            profiles.validate(kind, invalid)
                    self.assertEqual(source['form_identity']['written_representation'], 'e\u0301')
                path = root / f'ToS/source-witnesses/lexical-descriptions/fixture-{kind}/{kind}.json'
                path.parent.mkdir(parents=True); path.write_text(json.dumps(source))
                changes = [prepare_metadata_change(source, None, 'test:lexical-profile',
                    form_id=f'tos.form.lexical-{kind}-{role}', field_id=field) for role, field in
                    (('name', 'metadata.preferred-name'), ('hover', 'metadata.source-note'))]
                formset = _apply(None, Record.from_payload(source['record_id'], 1, source), changes)
                path.with_name(kind + '.human-forms.json').write_text(json.dumps(formset))
                records[kind] = source
            objects = {r['record_id']: r for r in records.values()}
            reader, claims = SourceClaimProfiles(root), []
            for index, (predicate, subject, target) in enumerate((
                    ('lexical_form_of', records['lexical-form']['record_id'], records['lexeme']['record_id']),
                    ('lexical_sense_of', records['sense']['record_id'], records['lexeme']['record_id']))):
                claim = {**copy.deepcopy(baseline[0]), 'schema_version': 'tos_semantic_relation_claim_v1',
                    'claim_id': f'tos.claim.synthetic-lexical-{index}', 'subject_ref': subject,
                    'predicate': predicate, 'object': target, 'qualifiers': {
                        'statement': 'Этот разбор предложен в синтетическом контексте; конкурирующие не исключены.',
                        'statement_language': 'ru', 'statement_script': 'Cyrl',
                        'relation_basis': 'A declared synthetic analysis, not string equality.',
                        'attestation_scope': 'Only this artificial research context.', 'negated': False}}
                reader.validate(claim, objects)
                relation = reader.relations[predicate]
                self.assertFalse(relation['transitive'])
                self.assertIsNone(relation['cardinality']['per_subject_max'])
                for change in ({'object': records['sense']['record_id']}, {'claim_id': subject},
                               {'evidence_refs': []}, {'review_status': 'accepted'}):
                    with self.subTest(predicate=predicate, change=change), self.assertRaises(SourceProfileError):
                        reader.validate({**claim, **change}, objects)
                invalid = copy.deepcopy(claim); invalid['qualifiers'].pop('attestation_scope')
                with self.assertRaises(SourceProfileError):
                    reader.validate(invalid, objects)
                claims.extend([claim, {**copy.deepcopy(claim), 'claim_id': claim['claim_id'] + '.counter',
                    'qualifiers': {**claim['qualifiers'], 'negated': True}}])
            path.with_name('source-claims.jsonl').write_text(''.join(json.dumps(c) + '\n' for c in claims))
            projection = rebuild()
            _, entities, relations = self.historical_knowledge(root, projection)
            import tos_corpus_index_common as corpus_builder
            from tos_access.knowledge import build_knowledge_graph, focus_knowledge_node, select_human_forms, execute_knowledge_lens
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                navigation = corpus_builder.build_source_navigation([])
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, projection, entities, relations)
            for kind, source in records.items():
                carriers = [n for n in graph['nodes'] if n['entity_id'] == source['record_id']]
                self.assertEqual({n['source_graph'] for n in carriers}, {'source-claims', 'source-navigation'})
                for node in carriers:
                    self.assertEqual(node['type_id'], 'tos.entity.' + ('lexical-sense' if kind == 'sense' else kind))
                    self.assertEqual(node['attributes']['source_record'], source)
                    packet = select_human_forms(node, 'ru')['roles']['hover']['packet']
                    self.assertEqual(packet['display_text'], source['notes'])
                    self.assertIsNone(packet['admission'])
                    for field in ('semantic_content', 'form_identity'):
                        if field in source:
                            self.assertTrue(any(c['binding']['pointer'] == '/' + field and c['value'] == source[field]
                                                for c in packet['context']))
                for field, value in contents[kind].items():
                    result = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                        'lens_id': 'synthetic-lexical-property', 'node_query': {'filters': [{
                            'property_id': 'tos.property.' + kind + '-' + field.replace('_', '-'),
                            'op': 'eq', 'value': value}]}, 'relation_query': {'enabled': False}, 'detail': 'full'})
                    self.assertEqual({n['entity_id'] for n in result['nodes']}, {source['record_id']})
            for claim in claims:
                node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
                self.assertEqual(node['attributes']['source_claim'], claim)
                for center, other in ((claim['subject_ref'], claim['object']), (claim['object'], claim['subject_ref'])):
                    focus = focus_knowledge_node(graph, center, depth=2)
                    self.assertIn(other, {n['entity_id'] for n in focus['nodes']})

    def test_linguistic_profiles_keep_language_script_variety_and_notation_distinct(self):
        """Synthetic linguistic accounts are not claims about the copied artifact."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        profiles = SourceRecordProfiles(REPO_ROOT)
        contents = {
            'language': {'system_account': 'A synthetic linguistic system, not a script.'},
            'linguistic-variety': {'system_account': 'A synthetic variety, not an artifact period.',
                'distinguishing_basis': 'A disputed local criterion, not a universal language/dialect test.'},
            'script': {'script_account': 'A synthetic writing tradition, not a language.',
                'sign_inventory_scope': 'An explicitly limited repertoire, not timeless sign readings.'},
            'transliteration-scheme': {'mapping_convention': 'A synthetic notation convention, not translation.',
                'coverage_and_loss': 'Unknown readings remain unknown; no reversibility is asserted.'},
        }
        with self.historical_fixture() as (root, history, real, baseline, rebuild):
            for name in ('source-metadata-record', 'semantic-description-record', 'linguistic-description-record',
                         'semantic-relation-claim', 'linguistic-relation-claim', 'source-claim-record',
                         'semantic-relation-type-registry', 'artifact-source-witness'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            artifact_ref = 'ToS/source-witnesses/artifacts/old-babylonian/uncertain/penn-cbs-07771/artifact-witness.json'
            artifact_path = root / artifact_ref
            artifact_path.parent.mkdir(parents=True)
            artifact_path.write_bytes((REPO_ROOT / artifact_ref).read_bytes())
            artifact = json.loads(artifact_path.read_bytes())
            for ref in artifact['philosophy_planting_refs']:
                path = root / ref; path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes((REPO_ROOT / ref).read_bytes())
            from source_commands import prepare_metadata_change, _apply
            from knowledge_assessment import Record
            records = {}
            for kind, content in contents.items():
                source = {**copy.deepcopy(history[0][1]), 'schema_version': 'tos_linguistic_description_record_v1',
                    'record_type': kind, 'record_id': f'tos.{kind}.synthetic-linguistic',
                    'preferred_label': 'Одинаковая метка', 'notes': 'Синтетическое описание; совпадение имени не доказывает тождества.',
                    'field_languages': {key: {'language': 'ru', 'script': 'Cyrl'} for key in ('preferred_label', 'notes')},
                    'semantic_scope': {'scope_note': 'Only this artificial research referent.',
                        'identity_criterion': 'Correcting its description preserves its ID; changing its referent does not.',
                        'language': 'en', 'script': 'Latn'},
                    'semantic_content': {**content, 'language': 'en', 'script': 'Latn',
                        'unknown_extension': {'instruction': 'Inert text', 'values': [False, None, '𒀀']}},
                    'extensions': {'uninterpreted_code': 'x-synthetic'}}
                profiles.validate(kind, source)
                for field in (*content, 'language', 'script'):
                    invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                    with self.subTest(kind=kind, missing=field), self.assertRaises(SourceProfileError):
                        profiles.validate(kind, invalid)
                for field in content:
                    invalid = copy.deepcopy(source); invalid['semantic_content'][field] = ' '
                    with self.subTest(kind=kind, empty=field), self.assertRaises(SourceProfileError):
                        profiles.validate(kind, invalid)
                for change in ({'record_id': 'tos.language-script.synthetic'}, {'notes': ' '},
                               {'language_ref': 'tos.language.synthetic-linguistic'}, {'semantic_scope': {}}):
                    with self.subTest(kind=kind, change=change), self.assertRaises(SourceProfileError):
                        profiles.validate(kind, {**source, **change})
                path = root / f'ToS/source-witnesses/languages/fixture/{kind}.json'
                path.parent.mkdir(parents=True, exist_ok=True); path.write_text(json.dumps(source))
                changes = [prepare_metadata_change(source, None, 'test:linguistic-profile',
                    form_id=f'tos.form.linguistic-{kind}-{role}', field_id=field) for role, field in
                    (('name', 'metadata.preferred-name'), ('hover', 'metadata.source-note'))]
                formset = _apply(None, Record.from_payload(source['record_id'], 1, source), changes)
                path.with_name(kind + '.human-forms.json').write_text(json.dumps(formset))
                records[kind] = source
            objects = {r['record_id']: r for r in records.values()}
            objects[artifact['artifact_id']] = {'record_type': 'artifact'}
            claims, reader = [], SourceClaimProfiles(root)
            for index, (predicate, subject, target) in enumerate((
                    ('inscription_language', artifact['artifact_id'], records['language']['record_id']),
                    ('inscription_script', artifact['artifact_id'], records['script']['record_id']),
                    ('dialect_of', records['linguistic-variety']['record_id'], records['language']['record_id']),
                    ('historical_language_stage_of', records['linguistic-variety']['record_id'], records['language']['record_id']),
                    ('transliteration_source_script', records['transliteration-scheme']['record_id'], records['script']['record_id']),
                    ('transliteration_notation_script', records['transliteration-scheme']['record_id'], records['script']['record_id']))):
                claim = {**copy.deepcopy(baseline[0]), 'schema_version': 'tos_semantic_relation_claim_v1',
                    'claim_id': f'tos.claim.synthetic-linguistic-{index}', 'subject_ref': subject,
                    'predicate': predicate, 'object': target, 'qualifiers': {
                        'statement': 'Условная атрибуция только для теста; не установленный факт.',
                        'statement_language': 'ru', 'statement_script': 'Cyrl',
                        'relation_basis': 'Synthetic evidence only, not the copied artifact metadata.',
                        'attestation_scope': 'Only this synthetic relation; not the whole object or every period.',
                        'negated': index == 1,
                        'source_wording': {'text': 'synthetic attribution', 'language': 'en', 'script': 'Latn'}}}
                reader.validate(claim, objects)
                self.assertFalse(reader.relations[predicate]['transitive'])
                self.assertIsNone(reader.relations[predicate]['cardinality']['per_subject_max'])
                for field in ('attestation_scope', 'relation_basis', 'statement_language'):
                    invalid = copy.deepcopy(claim); invalid['qualifiers'].pop(field)
                    with self.subTest(predicate=predicate, missing=field), self.assertRaises(SourceProfileError):
                        reader.validate(invalid, objects)
                for change in ({'subject_ref': real[2]['record_id']}, {'object': real[0]['record_id']},
                               {'review_status': 'accepted'}, {'evidence_refs': []}, {'claim_id': subject}):
                    with self.subTest(predicate=predicate, change=change), self.assertRaises(SourceProfileError):
                        reader.validate({**claim, **change}, {**objects, **{r['record_id']: r for r in real}})
                swapped = records['script' if predicate == 'inscription_language' else 'language']['record_id']
                if predicate in {'inscription_language', 'inscription_script'}:
                    with self.assertRaises(SourceProfileError):
                        reader.validate({**claim, 'object': swapped}, objects)
                claims.append(claim)
            path.with_name('source-claims.jsonl').write_text(''.join(json.dumps(c) + '\n' for c in claims))
            projection = rebuild()
            _, entities, relations = self.historical_knowledge(root, projection)
            import tos_corpus_index_common as corpus_builder
            from tos_access.knowledge import build_knowledge_graph, focus_knowledge_node, select_human_forms, execute_knowledge_lens
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                navigation = corpus_builder.build_source_navigation([])
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, projection, entities, relations)
            for kind, source in records.items():
                carriers = [n for n in graph['nodes'] if n['entity_id'] == source['record_id']]
                self.assertEqual({n['source_graph'] for n in carriers}, {'source-claims', 'source-navigation'})
                for node in carriers:
                    self.assertEqual(node['attributes']['source_record'], source)
                    self.assertEqual(node['type_id'], 'tos.entity.' + kind)
                    self.assertNotIn('tos.entity.language-script', node['semantics']['type_ancestors'])
                    packet = select_human_forms(node, 'ru')['roles']['hover']['packet']
                    self.assertEqual(packet['display_text'], source['notes'])
                    self.assertTrue(any(c['binding']['pointer'] == '/semantic_content' and c['value'] == source['semantic_content']
                                        for c in packet['context']))
                    self.assertIsNone(packet['admission'])
                for field, value in contents[kind].items():
                    result = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                        'lens_id': 'synthetic-linguistic-property', 'node_query': {'filters': [{
                            'property_id': 'tos.property.' + kind + '-' + field.replace('_', '-'),
                            'op': 'eq', 'value': value}]}, 'relation_query': {'enabled': False}, 'detail': 'full'})
                    self.assertEqual({n['entity_id'] for n in result['nodes']}, {source['record_id']})
            for claim in claims:
                node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
                self.assertEqual(node['attributes']['source_claim'], claim)
                for center, other in ((claim['subject_ref'], claim['object']), (claim['object'], claim['subject_ref'])):
                    focused = focus_knowledge_node(graph, center, depth=2)
                    self.assertIn(other, {n['entity_id'] for n in focused['nodes']})

    def test_reception_profiles_keep_historical_recognition_and_knowledge_admission_distinct(self):
        """Synthetic reception contracts; no claim that any history occurred."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        records, relations = SourceRecordProfiles(REPO_ROOT), SourceClaimProfiles(REPO_ROOT)
        contents = {
            'reception-process': {'engagement_basis': 'Documented reading and response, not inferred influence.'},
            'historical-canonization': {'engagement_basis': 'A source-described selection practice.',
                'selection_basis': 'The stated criteria of the historical selection.',
                'authority_scope': 'Only the specified historical community, not ToS admission.'},
            'historical-forgetting': {'evidence_boundary': 'A scoped report of diminished transmission; database absence proves nothing.'},
            'rediscovery-episode': {'prior_access_boundary': 'New access for this community, not first knowledge by anyone.'},
            'intellectual-legacy': {'transmission_basis': 'A source-described persistence with changes; similarity is insufficient.'},
        }
        for kind, content in contents.items():
            source = {'schema_version': 'tos_reception_record_v1', 'record_type': kind,
                'record_id': f'tos.{kind}.synthetic-reception', 'record_version': 1,
                'preferred_label': 'Условная история рецепции', 'notes': 'Только синтетическая проверка.',
                'field_languages': {key: {'language': 'ru', 'script': 'Cyrl'} for key in ('preferred_label', 'notes')},
                'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
                'source_refs': ['test:synthetic-reception'], 'external_identifiers': [], 'visibility': 'public_metadata_only',
                'semantic_scope': {'scope_note': 'The declared historical scope only.',
                    'identity_criterion': 'One researched process, episode or state, not the current description.',
                    'language': 'en', 'script': 'Latn'},
                'semantic_content': {'reception_account': 'An attributed historical account, not an assessment.',
                    'receiving_context': 'The particular audience and research limits.',
                    **content, 'language': 'en', 'script': 'Latn', 'uninterpreted': [None, False]}}
            with self.subTest(kind=kind):
                records.validate(kind, source)
                ancestry = relations.ancestry('tos.entity.' + kind)
                self.assertIn('tos.entity.reception-history', ancestry)
                self.assertIn('tos.entity.historical-situation', ancestry)
                self.assertEqual('tos.entity.historical-event' in ancestry, kind == 'rediscovery-episode')
                self.assertEqual('tos.entity.historical-state' in ancestry, kind == 'intellectual-legacy')
                self.assertEqual('tos.entity.historical-process' in ancestry,
                                 kind in {'reception-process', 'historical-canonization', 'historical-forgetting'})
                self.assertNotIn('tos.entity.semantic-object', ancestry)
                for field in ('reception_account', 'receiving_context', *content, 'language', 'script'):
                    invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                    with self.assertRaises(SourceProfileError):
                        records.validate(kind, invalid)
                for field in ('reception_account', 'receiving_context', *content):
                    invalid = copy.deepcopy(source); invalid['semantic_content'][field] = ' '
                    with self.assertRaises(SourceProfileError):
                        records.validate(kind, invalid)
                for update in ({'record_id': 'tos.historical-event.synthetic-reception'},
                               {'schema_version': 'tos_historical_record_v1'}, {'semantic_scope': {}},
                               {'admission': 'accepted'}, {'canon_status': 'accepted'}):
                    with self.assertRaises(SourceProfileError):
                        records.validate(kind, {**source, **update})
        cases = [('receives', 'reception-process', 'work'),
                 ('historically_canonizes', 'historical-canonization', 'conception'),
                 ('historically_forgets', 'historical-forgetting', 'intellectual-tradition'),
                 ('rediscovers', 'rediscovery-episode', 'artifact'),
                 ('legacy_of', 'intellectual-legacy', 'agent'),
                 ('reception_carrier', 'rediscovery-episode', 'work')]
        objects = {f'tos.{kind}.synthetic-reception': {'record_type': kind}
                   for kind in (*contents, 'work', 'conception', 'intellectual-tradition', 'artifact', 'agent', 'place')}
        for predicate, left, right in cases:
            claim = {'schema_version': 'tos_historical_context_claim_v1', 'claim_type': 'relation',
                'claim_id': 'tos.claim.synthetic-reception', 'claim_version': 1,
                'subject_ref': f'tos.{left}.synthetic-reception', 'predicate': predicate,
                'object': f'tos.{right}.synthetic-reception', 'assertion_layer': 'scholarly_report',
                'evidence_refs': ['test:synthetic-reception'], 'provenance_event_ref': 'tos.event.synthetic-reception',
                'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic-test'},
                'epistemic_status': 'disputed', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'An attributed relationship, not ToS canon or an unqualified fact.',
                    'statement_language': 'en', 'statement_script': 'Latn',
                    'relation_basis': 'Synthetic source report, not graph proximity.',
                    'context_scope': 'The particular receiving community only.',
                    'time_scope_note': 'Historical bounds unknown; not the record creation time.',
                    'uninterpreted': {'retained': [None, False]}}}
            with self.subTest(predicate=predicate):
                relations.validate(claim, objects)
                self.assertFalse(relations.relations[predicate]['transitive'])
                for field in ('statement', 'statement_language', 'statement_script', 'relation_basis', 'context_scope', 'time_scope_note'):
                    invalid = copy.deepcopy(claim); invalid['qualifiers'].pop(field)
                    with self.assertRaises(SourceProfileError):
                        relations.validate(invalid, objects)
                for update in ({'subject_ref': 'tos.place.synthetic-reception'},
                               {'object': 'tos.place.synthetic-reception'}, {'assertion_layer': 'canon_judgment'}):
                    with self.assertRaises(SourceProfileError):
                        relations.validate({**claim, **update}, objects)

    def test_fragment_and_quotation_profiles_keep_research_identity_and_text_evidence_distinct(self):
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        records = SourceRecordProfiles(REPO_ROOT)
        relations = SourceClaimProfiles(REPO_ROOT)
        subjects = {}
        for kind, content in (
            ('textual-fragment', {'fragment_account': 'A synthetic portion, not a physical fragment.',
                'boundary_basis': 'A proposed editorial distinction; no exact text is stored.'}),
            ('quotation-passage', {'quotation_account': 'A synthetic passage quoting another text.',
                'location_account': 'A reported position in a containing work; not a resolved text anchor.'}),
        ):
            record = {'schema_version': 'tos_textual_passage_record_v1', 'record_type': kind,
                'record_id': f'tos.{kind}.synthetic', 'record_version': 1,
                'preferred_label': f'Synthetic {kind}', 'identity_status': 'provisional',
                'source_refs': ['test:synthetic-source'], 'external_identifiers': [],
                'same_as_posture': 'no_equivalence_claim', 'visibility': 'public_metadata_only',
                'notes': 'A synthetic source-described referent; no text authenticity or historical assertion.',
                'field_languages': {key: {'language': 'en', 'script': 'Latn'} for key in ('preferred_label', 'notes')},
                'semantic_scope': {'scope_note': 'Synthetic unit only.', 'identity_criterion': 'The declared referent, not its name or record version.', 'language': 'en', 'script': 'Latn'},
                'semantic_content': {**content, 'language': 'en', 'script': 'Latn', 'unknown': [False, None]}}
            records.validate(kind, record)
            subjects[kind] = record
            for invalid in ({k: v for k, v in record.items() if k != 'semantic_scope'},
                    {**record, 'notes': ' '}, {**record, 'record_id': 'tos.artifact.synthetic'},
                    {**record, 'semantic_content': {'language': 'en', 'script': 'Latn'}},
                    {**record, 'semantic_scope': {**record['semantic_scope'], 'identity_criterion': ' '}}):
                with self.subTest(kind=kind, invalid=invalid), self.assertRaises(SourceProfileError):
                    records.validate(kind, invalid)
        objects = {r['record_id']: r for r in subjects.values()}
        objects.update({'tos.work.synthetic': {'record_type': 'work'}, 'tos.agent.synthetic': {'record_type': 'agent'},
                        'tos.artifact.synthetic': {'record_type': 'artifact'}})
        fragment = subjects['textual-fragment']['record_id']
        quotation = subjects['quotation-passage']['record_id']
        associations = []
        for i, (predicate, subject, target) in enumerate((
            ('fragment_of', fragment, 'tos.work.synthetic'),
            ('quotation_in', quotation, 'tos.work.synthetic'),
            ('quotation_preserves_fragment', quotation, fragment),
        )):
            claim = {'schema_version': 'tos_textual_passage_claim_v1', 'claim_id': f'tos.claim.synthetic-passage-{i}',
                'claim_version': 1, 'claim_type': 'relation', 'assertion_layer': 'scholarly_report',
                'subject_ref': subject, 'predicate': predicate, 'object': target,
                'evidence_refs': ['test:synthetic-only'], 'maker': {'maker_type': 'software', 'agent_ref': 'test:fixture'},
                'provenance_event_ref': 'tos.event.synthetic-passage', 'epistemic_status': 'uncertain',
                'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'Synthetic reported association, not textual equivalence.',
                    'statement_language': 'en', 'statement_script': 'Latn', 'scope_note': 'Synthetic scope only.'}}
            relations.validate(claim, objects)
            associations.append(claim)
            for change in ({'object': 'tos.artifact.synthetic'}, {'subject_ref': 'tos.agent.synthetic'},
                           {'subject_ref': target, 'object': subject}, {'evidence_refs': []},
                           {'qualifiers': {k: v for k, v in claim['qualifiers'].items() if k != 'scope_note'}}):
                with self.subTest(predicate=predicate, change=change), self.assertRaises(SourceProfileError):
                    relations.validate({**claim, **change}, objects)
        with self.historical_fixture() as (root, history, real, baseline, rebuild):
            for name in ('source-metadata-record', 'semantic-description-record', 'textual-passage-record',
                         'textual-passage-claim', 'source-claim-record', 'semantic-relation-type-registry'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            directory = root / 'ToS/source-witnesses/textual-passages/synthetic'
            directory.mkdir(parents=True)
            for kind, record in subjects.items():
                (directory / (kind + '.json')).write_text(json.dumps(record))
            for claim in associations:
                if claim['object'] == 'tos.work.synthetic':
                    claim['object'] = real[2]['record_id']
                claim.update(evidence_refs=baseline[0]['evidence_refs'],
                             provenance_event_ref=baseline[0]['provenance_event_ref'])
            (directory / 'source-claims.jsonl').write_text(''.join(json.dumps(c) + '\n' for c in associations))
            projection = rebuild()
            _, entities, registry = self.historical_knowledge(root, projection)
            import tos_corpus_index_common as corpus_builder
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                navigation = corpus_builder.build_source_navigation([])
            from tos_access.knowledge import build_knowledge_graph, focus_knowledge_node
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, projection, entities, registry)
            for record in subjects.values():
                carriers = [n for n in graph['nodes'] if n['entity_id'] == record['record_id']]
                self.assertEqual({n['source_graph'] for n in carriers}, {'source-claims', 'source-navigation'})
                for node in carriers:
                    self.assertEqual(node['attributes']['source_record'], record)
                    self.assertEqual(node['type_id'], 'tos.entity.' + record['record_type'])
                    self.assertNotIn('tos.entity.artifact', node['semantics']['type_ancestors'])
            for claim in associations:
                node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
                self.assertEqual(node['attributes']['source_claim'], claim)
                for center, other in ((claim['subject_ref'], claim['object']), (claim['object'], claim['subject_ref'])):
                    focused = focus_knowledge_node(graph, center, depth=2)
                    self.assertIn(other, {n['entity_id'] for n in focused['nodes']})

    def test_independent_classifications_preserve_scope_and_do_not_retype_objects(self):
        """Synthetic genre/medium claims are mechanics, not literary judgments."""
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            for name in ('semantic-relation-type-registry', 'source-claim-record', 'source-structured-value', 'source-classification-claim'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            reader = SourceClaimProfiles(root)
            additions = []
            for index, (predicate, kind, term) in enumerate((
                    ('classified_genre', 'genre-classification', 'dialogue'),
                    ('classified_genre', 'genre-classification', 'dialogue'),
                    ('classified_communication_medium', 'communication-medium-classification', 'spoken'),
                    ('classified_content_form', 'content-form-classification', 'aphorism'))):
                value = {'kind': kind, 'term': term, 'term_language': 'en', 'term_script': 'Latn',
                    'classification_basis': 'Synthetic source-specific criterion, not a real attribution.',
                    'scope_note': 'Only the synthetic characterization; no whole-life or universal claim.',
                    'source_wording': {'text': 'Условная классификация — только тест.', 'language': 'ru', 'script': 'Cyrl'},
                    'extensions': {'date': '1886', 'unknown': [False, None], 'instruction': 'Inert source text.'}}
                claim = {**copy.deepcopy(claims[0]), 'schema_version': 'tos_source_classification_claim_v1',
                    'claim_id': f'tos.claim.classification-fixture-{index}', 'subject_ref': real[2]['record_id'],
                    'predicate': predicate, 'object': value, 'qualifiers': {
                        'statement': 'A synthetic attribution, uncertain and limited to its named scope.',
                        'statement_language': 'en', 'statement_script': 'Latn', 'negated': index == 1}}
                objects = {real[2]['record_id']: real[2]}
                original = copy.deepcopy(claim)
                reader.validate(claim, objects)
                self.assertEqual(claim, original)
                self.assertEqual(reader.identity_refs(claim), {claim['subject_ref']})
                self.assertTrue(reader.is_value(claim))
                self.assertFalse(reader.is_temporal(claim))
                for field in ('term', 'term_language', 'term_script', 'classification_basis', 'scope_note', 'source_wording'):
                    bad = {key: val for key, val in value.items() if key != field}
                    with self.subTest(kind=kind, missing=field), self.assertRaises(SourceProfileError):
                        reader.validate({**claim, 'object': bad}, objects)
                for bad in ({**value, 'kind': 'textual-survival'}, {**value, 'term': ' '},
                            {**value, 'term_language': 'en\n'}, 'tos.genre.synthetic'):
                    with self.subTest(kind=kind, value=bad), self.assertRaises(SourceProfileError):
                        reader.validate({**claim, 'object': bad}, objects)
                other_kind = 'communication-medium-classification' if index < 2 else 'genre-classification'
                with self.assertRaises(SourceProfileError):
                    reader.validate({**claim, 'object': {**value, 'kind': other_kind}}, objects)
                for wrong in ('agent', 'place', 'file', 'genre', 'medium'):
                    with self.subTest(wrong_subject=wrong), self.assertRaises(SourceProfileError):
                        reader.validate(claim, {claim['subject_ref']: {'record_type': wrong}})
                additions.append(claim)
            claim_path = root / 'ToS/source-witnesses/history/fixture/source-claims.jsonl'
            claim_path.write_text(''.join(json.dumps(c, ensure_ascii=False) + '\n' for c in additions))
            graph, entities, relations = self.historical_knowledge(root, rebuild())
            from tos_access.knowledge import execute_knowledge_lens, focus_knowledge_node, knowledge_catalog
            values = [n for n in graph['nodes'] if n['type_id'] in {
                'tos.entity.genre-classification', 'tos.entity.communication-medium-classification',
                'tos.entity.content-form-classification'}]
            self.assertEqual(len(values), 4)
            self.assertEqual(len({n['entity_id'] for n in values}), 4)
            self.assertTrue(all(n['type_id'] == 'tos.entity.work' for n in graph['nodes']
                                if n['entity_id'] == real[2]['record_id']))
            for node in values:
                self.assertNotIn('time', node['semantics'])
                self.assertIn(node['attributes']['value'], [c['object'] for c in additions])
                self.assertTrue(node['semantics']['assertion_contexts'])
                focused = focus_knowledge_node(graph, node['entity_id'], depth=2)
                self.assertIn(real[2]['record_id'], {n['entity_id'] for n in focused['nodes']})
                focus_vertex = next(v for v in focused['scene']['vertices']
                                    if v['id'] == focused['scene']['focus_vertex_id'])
                self.assertEqual(focus_vertex['node_ids'], [node['id']])
                self.assertIsNone(focus_vertex['entity_id'])  # Addressable value, not a new persistent subject.
            self.assertTrue(any(context['fields']['qualifiers']['value']['negated'] is True
                for node in values for context in node['semantics']['assertion_contexts']
                if context['binding_role'] == 'referenced-claim'))
            spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'classification-test',
                'node_query': {'filters': [{'property_id': 'tos.property.classification-term', 'op': 'eq', 'value': 'dialogue'}]},
                'relation_query': {'enabled': False}, 'detail': 'compact', 'explain': True}
            selected = execute_knowledge_lens(graph, spec)
            self.assertEqual(len(selected['nodes']), 2)
            self.assertEqual({n['type_id'] for n in selected['nodes']}, {'tos.entity.genre-classification'})
            spec['node_query']['filters'].extend([
                {'property_id': 'tos.property.classification-term-language', 'op': 'eq', 'value': 'en'},
                {'property_id': 'tos.property.classification-term-script', 'op': 'eq', 'value': 'Latn'}])
            self.assertEqual(len(execute_knowledge_lens(graph, spec)['nodes']), 2)
            spec['node_query']['filters'][1]['value'] = 'ru'  # Wording is Russian, classification term is not.
            self.assertEqual(execute_knowledge_lens(graph, spec)['nodes'], [])
            catalog = knowledge_catalog(graph, {}, {}, entities, relations)
            descriptors = catalog['semantic_registries']['relation_types']['entries']
            self.assertEqual(len([d for d in descriptors if d['relation_type_id'] in {
                'tos.relation.classified-genre', 'tos.relation.classified-communication-medium'}]), 2)
            for identifier in ('tos.entity.genre', 'tos.entity.medium'):
                entry = next(t for t in entities['types'] if t['type_id'] == identifier)
                self.assertEqual(entry['object_role'], 'navigation')
            carrier = {**copy.deepcopy(additions[0]), 'predicate': 'classified_carrier_medium',
                'subject_ref': 'tos.artifact.synthetic-carrier', 'object': {
                    **copy.deepcopy(additions[0]['object']), 'kind': 'carrier-medium-classification', 'term': 'codex'}}
            for kind in ('artifact', 'item'):
                reader.validate(carrier, {carrier['subject_ref']: {'record_type': kind}})
            for kind in ('work', 'expression', 'agent', 'file', 'genre'):
                with self.subTest(carrier_subject=kind), self.assertRaises(SourceProfileError):
                    reader.validate(carrier, {carrier['subject_ref']: {'record_type': kind}})
            with self.assertRaises(SourceProfileError):
                reader.validate({**carrier, 'object': {**carrier['object'], 'kind': 'genre-classification'}})

    def test_textual_survival_is_a_typed_claim_value_not_a_lost_work_identity(self):
        """Synthetic mechanics only: no assertion about a real work's survival."""
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        profiles = SourceClaimProfiles(REPO_ROOT)
        value = {'kind': 'textual-survival', 'status': 'fragmentary',
            'scope_note': 'The text of this synthetic work, not one physical copy.',
            'coverage_note': 'Reported quotations only; completeness is not established.',
            'source_wording': {'text': 'Сохранилось фрагментарно — только тест.', 'language': 'ru', 'script': 'Cyrl'},
            'extensions': {'date': '1886', 'relative': {'anchor_ref': 'tos.work.unresolved'}, 'unknown': [False, None]}}
        claim = {'schema_version': 'tos_source_textual_survival_claim_v1', 'claim_id': 'tos.claim.synthetic-survival',
            'claim_version': 1, 'claim_type': 'relation', 'assertion_layer': 'scholarly_report',
            'subject_ref': 'tos.work.synthetic', 'predicate': 'textual_survival', 'object': value,
            'evidence_refs': ['test:synthetic-only'], 'maker': {'maker_type': 'software', 'agent_ref': 'test:fixture'},
            'provenance_event_ref': 'tos.event.synthetic-survival', 'epistemic_status': 'uncertain',
            'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
            'qualifiers': {'statement': 'Only a synthetic, uncertain report of fragmentary survival.',
                'statement_language': 'en', 'statement_script': 'Latn'}}
        objects = {'tos.work.synthetic': {'record_type': 'work'}}
        original = copy.deepcopy(claim)
        profiles.validate(claim, objects)
        self.assertTrue(profiles.is_value(claim))
        self.assertFalse(profiles.is_temporal(claim))
        self.assertEqual(profiles.identity_refs(claim), {'tos.work.synthetic'})
        self.assertEqual(claim, original)
        relation = profiles.relations['textual_survival']
        self.assertEqual(relation['range_type_ids'], ['tos.entity.textual-survival'])
        self.assertFalse(relation['transitive'])
        self.assertIn('tos.entity.literal', profiles.ancestry('tos.entity.textual-survival'))
        self.assertNotIn('tos.entity.identity', profiles.ancestry('tos.entity.textual-survival'))
        for status in ('complete', 'fragmentary', 'not_extant', 'unknown'):
            profiles.validate({**claim, 'object': {**value, 'status': status}}, objects)
        for replacement in ('tos.work.synthetic', {**value, 'status': 'accepted'},
                {**value, 'kind': 'unknown-value'}, {**value, 'coverage_note': ' '},
                {k: v for k, v in value.items() if k != 'source_wording'}):
            with self.subTest(value=replacement), self.assertRaises(SourceProfileError):
                profiles.validate({**claim, 'object': replacement}, objects)
        for kind in ('agent', 'place', 'historical-event'):
            with self.subTest(kind=kind), self.assertRaises(SourceProfileError):
                profiles.validate(claim, {'tos.work.synthetic': {'record_type': kind}})

    def test_structured_value_profile_extends_by_data_without_temporal_or_identity_guessing(self):
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            for name in ('semantic-relation-type-registry', 'source-claim-record', 'source-structured-value'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            entity_ref = 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
            relation_ref = 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            entities = json.loads((root / entity_ref).read_bytes())
            registry = json.loads((root / relation_ref).read_bytes())
            entity = copy.deepcopy(next(e for e in entities['types'] if e['type_id'] == 'tos.entity.textual-survival'))
            entity.update(type_id='tos.entity.fixture-value', source_mappings=[
                {'source_graph': 'source-claims', 'source_kind_id': 'fixture-value'}])
            entities['types'].append(entity)
            (root / entity_ref).write_text(json.dumps(entities))
            relation = copy.deepcopy(next(e for e in registry['relations'] if e['relation_type_id'] == 'tos.relation.textual-survival'))
            relation.update(relation_type_id='tos.relation.fixture-value', range_type_ids=['tos.entity.fixture-value'],
                source_mappings=[{'source_graph': 'source-claims', 'source_predicate_id': 'fixture_value', 'scope': 'claim-predicate'}])
            profile = relation['source_claim_profile']
            profile['value_kind'] = 'fixture-value'
            profile['schemas'] = [{'schema_version': 'tos_fixture_value_v1',
                'schema_ref': 'ToS/contracts/fixture-value.schema.json', 'schema_dependencies': []}]
            registry['relations'].append(relation)
            (root / relation_ref).write_text(json.dumps(registry))
            # A deliberately permissive domain schema cannot weaken the shared value law.
            schema = {'$schema': 'https://json-schema.org/draft/2020-12/schema',
                '$id': 'https://tree-of-sophia.local/ToS/contracts/fixture-value.schema.json', 'type': 'object'}
            (root / 'ToS/contracts/fixture-value.schema.json').write_text(json.dumps(schema))
            value = {'kind': 'fixture-value', 'date': '1886', 'relative': {'anchor_ref': 'tos.work.unresolved'},
                'source_wording': {'text': 'Условное значение, не дата.', 'language': 'ru', 'script': 'Cyrl'},
                'unknown': [False, None, {'instruction': 'Source data, never executable authority.'}]}
            claim = {**copy.deepcopy(claims[0]), 'schema_version': 'tos_fixture_value_v1',
                'claim_id': 'tos.claim.fixture-value-a', 'subject_ref': real[2]['record_id'],
                'predicate': 'fixture_value', 'object': value}
            claim['qualifiers']['negated'] = True
            other = {**copy.deepcopy(claim), 'claim_id': 'tos.claim.fixture-value-b'}
            path = root / 'ToS/source-witnesses/history/fixture/source-claims.jsonl'
            path.write_text(json.dumps(claim) + '\n' + json.dumps(other) + '\n')
            reader = SourceClaimProfiles(root)
            self.assertEqual(reader.identity_refs(claim), {claim['subject_ref']})
            for bad in ({k: v for k, v in value.items() if k != 'source_wording'},
                        {**value, 'kind': 'textual-survival'},
                        {**value, 'source_wording': {'text': ' ', 'language': 'ru', 'script': 'Cyrl'}},
                        {**value, 'source_wording': {'text': 'test', 'language': 'ru\n', 'script': 'Cyrl'}}):
                with self.subTest(value=bad), self.assertRaises(SourceProfileError):
                    reader.validate({**claim, 'object': bad})
            graph, entity_registry, relation_registry = self.historical_knowledge(root, rebuild())
            nodes = [n for n in graph['nodes'] if n['type_id'] == 'tos.entity.fixture-value']
            self.assertEqual(len(nodes), 2)
            self.assertEqual(len({n['entity_id'] for n in nodes}), 2)
            for node in nodes:
                self.assertEqual(node['attributes']['value'], value)
                self.assertNotIn('time', node['semantics'])
                self.assertEqual(node['display']['title']['ru'], value['source_wording']['text'])
                contexts = node['semantics']['assertion_contexts']
                self.assertTrue(any(c['binding_role'] == 'referenced-claim'
                    and c['fields']['qualifiers']['value']['negated'] is True for c in contexts))
            from tos_access.knowledge import knowledge_catalog, focus_knowledge_node, validate_semantic_registries
            catalog = knowledge_catalog(graph, {}, {}, entity_registry, relation_registry)
            descriptor = next(e for e in catalog['semantic_registries']['relation_types']['entries']
                              if e['relation_type_id'] == relation['relation_type_id'])
            self.assertEqual(descriptor['source_claim_profile'], profile)
            focused = focus_knowledge_node(graph, nodes[0]['entity_id'], depth=2)
            self.assertIn(claim['subject_ref'], {n['entity_id'] for n in focused['nodes']})
            changed = copy.deepcopy(registry)
            changed['registry_version'] += 1
            changed['relations'][-1]['source_claim_profile'].update(profile_version=2, value_kind='textual-survival')
            self.assertFalse(validate_semantic_registries(entities, changed, previous_relation_registry=registry)['valid'])
            for update in ({'range_type_ids': ['tos.entity.literal']}, {'range_type_ids': ['tos.entity.work']},
                           {'range_type_ids': ['tos.entity.fixture-value', 'tos.entity.textual-survival']},
                           {'domain_type_ids': ['tos.entity.literal']}):
                invalid = copy.deepcopy(registry)
                invalid['relations'][-1].update(update)
                (root / relation_ref).write_text(json.dumps(invalid))
                with self.subTest(update=update), self.assertRaises(SourceProfileError):
                    SourceClaimProfiles(root)

    def test_historical_context_profiles_keep_biography_period_and_cohort_distinct(self):
        """Synthetic source contracts; no historical or causal acceptance."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        records, relations = SourceRecordProfiles(REPO_ROOT), SourceClaimProfiles(REPO_ROOT)
        contents = {
            'biographical-episode': {'episode_account': 'A bounded occurrence.', 'biographical_relevance': 'Its documented place in a life.'},
            'biographical-phase': {'phase_account': 'A described life phase.', 'phase_boundary_basis': 'Research boundaries, not mandatory universal stages.'},
            'historical-period': {'period_account': 'A historically situated periodization.', 'periodization_basis': 'Specific developments, not an arbitrary date interval.'},
            'historical-generation': {'generation_account': 'A described cohort.', 'cohort_basis': 'Shared historical experience, not institutional membership.'},
            'historical-environment': {'environment_account': 'A scoped configuration of conditions.', 'educational_account': 'A teaching arrangement.'},
            'life-circumstance': {'circumstance_account': 'A documented condition.', 'documented_relevance': 'Limited relevance to the life.', 'evidence_limitations': 'Not a diagnosis or causal interpretation.'},
        }
        for kind, content in contents.items():
            source = {'schema_version': 'tos_historical_context_record_v1', 'record_type': kind,
                'record_id': f'tos.{kind}.synthetic-context', 'record_version': 1,
                'preferred_label': 'Условный исторический предмет', 'notes': 'Только синтетическая проверка.',
                'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'}, 'notes': {'language': 'ru', 'script': 'Cyrl'}},
                'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
                'source_refs': ['test:synthetic-context'], 'external_identifiers': [], 'visibility': 'public_metadata_only',
                'semantic_scope': {'scope_note': 'This test only.', 'identity_criterion': 'Same research referent, not its label or description version.', 'language': 'en', 'script': 'Latn'},
                'semantic_content': {**content, 'language': 'en', 'script': 'Latn', 'unknown': {'values': [None, False]}}}
            records.validate(kind, source)
            ancestry = relations.ancestry('tos.entity.' + kind)
            self.assertIn('tos.entity.identity', ancestry)
            for excluded in ('semantic-object', 'temporal-object', 'organization', 'navigation-object'):
                self.assertNotIn('tos.entity.' + excluded, ancestry)
            self.assertEqual('tos.entity.historical-situation' in ancestry, kind != 'historical-generation')
            self.assertEqual('tos.entity.historical-event' in ancestry, kind == 'biographical-episode')
            for field in (*content, 'language', 'script'):
                invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                with self.subTest(kind=kind, missing=field), self.assertRaises(SourceProfileError):
                    records.validate(kind, invalid)
            for field in content:
                invalid = copy.deepcopy(source); invalid['semantic_content'][field] = ' '
                with self.assertRaises(SourceProfileError):
                    records.validate(kind, invalid)
            for update in ({'record_id': 'tos.historical-event.synthetic-context'}, {'semantic_scope': {}},
                           {'schema_version': 'tos_historical_record_v1'}, {'admission': 'accepted'}):
                with self.assertRaises(SourceProfileError):
                    records.validate(kind, {**source, **update})
            if kind == 'historical-environment':
                for domain in ('political', 'economic', 'cultural', 'religious', 'educational', 'scientific_technological'):
                    alternate = copy.deepcopy(source)
                    alternate['semantic_content'].pop('educational_account')
                    alternate['semantic_content'][domain + '_account'] = 'A source-described condition, not causal influence.'
                    records.validate(kind, alternate)
        cases = [('biographical_subject', 'biographical-episode', 'agent'),
                 ('biographical_subject', 'biographical-phase', 'agent'),
                 ('biographical_subject', 'life-circumstance', 'agent'),
                 ('phase_contains_episode', 'biographical-phase', 'biographical-episode'),
                 ('generation_member', 'historical-generation', 'agent'),
                 ('generation_in_period', 'historical-generation', 'historical-period'),
                 ('situation_in_period', 'biographical-phase', 'historical-period'),
                 ('contextualized_by_environment', 'institutional-body', 'historical-environment'),
                 ('contextualized_by_environment', 'work', 'historical-environment'),
                 ('contextualized_by_environment', 'historical-event', 'historical-environment'),
                 ('conception_in_environment', 'conception', 'historical-environment'),
                 ('circumstance_during_phase', 'life-circumstance', 'biographical-phase'),
                 ('historical_participant', 'biographical-episode', 'institutional-body'),
                 ('historical_place', 'historical-environment', 'place')]
        objects = {f'tos.{kind}.synthetic-context': {'record_type': kind}
                   for kind in (*contents, 'agent', 'institutional-body', 'work', 'conception', 'place', 'historical-event')}
        for predicate, left, right in cases:
            claim = {'schema_version': 'tos_historical_context_claim_v1', 'claim_type': 'relation',
                'claim_id': 'tos.claim.synthetic-context', 'claim_version': 1,
                'subject_ref': f'tos.{left}.synthetic-context', 'predicate': predicate, 'object': f'tos.{right}.synthetic-context',
                'assertion_layer': 'scholarly_report', 'evidence_refs': ['test:synthetic-context'],
                'provenance_event_ref': 'tos.event.synthetic-context', 'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic-test'},
                'epistemic_status': 'uncertain', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'A synthetic, source-attributed relationship only.', 'statement_language': 'en',
                    'statement_script': 'Latn', 'relation_basis': 'Explicit synthetic report, not graph proximity.',
                    'context_scope': 'Only the specified research scope.', 'time_scope_note': 'Bounds unknown; not capture time.',
                    'participation_role': 'documented institutional participant', 'uninterpreted': [False, None]}}
            with self.subTest(predicate=predicate):
                relations.validate(claim, objects)
                self.assertFalse(relations.relations[predicate]['transitive'])
                for field in ('statement', 'statement_language', 'statement_script', 'relation_basis', 'context_scope', 'time_scope_note'):
                    invalid = copy.deepcopy(claim); invalid['qualifiers'].pop(field)
                    with self.assertRaises(SourceProfileError):
                        relations.validate(invalid, objects)
                relations.validate({**claim, 'epistemic_status': 'disputed'}, objects)
                for endpoint in ('subject_ref', 'object'):
                    with self.assertRaises(SourceProfileError):
                        relations.validate({**claim, endpoint: 'tos.place.synthetic-context' if endpoint == 'subject_ref' else 'tos.work.synthetic-context'}, objects)
                if predicate == 'historical_participant':
                    invalid = copy.deepcopy(claim); invalid['qualifiers'].pop('participation_role')
                    with self.assertRaises(SourceProfileError):
                        relations.validate(invalid, objects)

    def test_biographical_context_relative_dates_round_trip_without_temporal_identity_coercion(self):
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        from source_commands import prepare_metadata_change, _apply
        from knowledge_assessment import Record
        import tos_corpus_index_common as corpus_builder
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            for name in ('source-metadata-record', 'semantic-description-record', 'historical-context-record',
                         'source-claim-record', 'source-temporal-claim', 'historical-context-claim',
                         'semantic-relation-type-registry'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            sources = []
            for kind, content in (
                    ('biographical-phase', {'phase_account': 'Synthetic phase.', 'phase_boundary_basis': 'A scoped test phase.'}),
                    ('biographical-episode', {'episode_account': 'Synthetic episode.', 'biographical_relevance': 'Synthetic relevance.'}),
                    ('historical-generation', {'generation_account': 'Synthetic cohort.', 'cohort_basis': 'Not an organization or an interval.'})):
                source = copy.deepcopy(history[0][1])
                source.update(schema_version='tos_historical_context_record_v1', record_type=kind,
                    record_id=f'tos.{kind}.synthetic-context', preferred_label='Тест: ' + kind,
                    field_languages={'preferred_label': {'language': 'ru', 'script': 'Cyrl'}, 'notes': {'language': 'ru', 'script': 'Cyrl'}},
                    semantic_scope={'scope_note': 'Only this synthetic test.', 'identity_criterion': 'Same research referent.', 'language': 'en', 'script': 'Latn'},
                    semantic_content={**content, 'language': 'en', 'script': 'Latn', 'unknown': [False, None]})
                path = root / f'ToS/source-witnesses/history/context/{kind}.json'
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(source, ensure_ascii=False))
                changes = [prepare_metadata_change(source, None, 'test:context',
                    form_id=f'tos.form.context-{kind}-{role}', field_id=field)
                    for role, field in (('name', 'metadata.preferred-name'), ('hover', 'metadata.source-note'))]
                forms = _apply(None, Record.from_payload(source['record_id'], 1, source), changes)
                path.with_name(kind + '.human-forms.json').write_text(json.dumps(forms))
                sources.append(source)
            phase, episode, generation = sources
            value = {'kind': 'relative-order', 'role': 'historical-time', 'calendar': None,
                     'year_numbering': None, 'certainty': 'uncertain',
                     'source_wording': {'text': 'В пределах условной фазы; точные даты неизвестны.', 'language': 'ru'},
                     'relative': {'relation': 'during', 'anchor_ref': phase['record_id']}, 'extensions': {'unknown': [False, None]}}
            claim = {**claims[0], 'schema_version': 'tos_source_temporal_claim_v1',
                'claim_id': 'tos.claim.context-relative', 'subject_ref': episode['record_id'],
                'predicate': 'historical_dating', 'object': value,
                'qualifiers': {'statement': 'Synthetic relative dating only.', 'statement_language': 'en', 'statement_script': 'Latn'}}
            path = root / 'ToS/source-witnesses/history/context/source-claims.jsonl'
            path.write_text(json.dumps(claim, ensure_ascii=False) + '\n')
            projection = rebuild()
            graph, entities, relations = self.historical_knowledge(root, projection)
            from tos_access.knowledge import (build_knowledge_graph, focus_knowledge_node,
                                              select_human_forms, validate_knowledge_semantics)
            report = validate_knowledge_semantics(graph, entities, relations)
            self.assertTrue(report['valid'], report['violations'])
            for source in sources:
                node = next(n for n in graph['nodes'] if n['entity_id'] == source['record_id'])
                self.assertEqual(node['attributes']['source_record'], source)
                self.assertNotIn('time', node['semantics'])
                self.assertEqual(node['display']['title']['default'], source['preferred_label'])
                forms = select_human_forms(node, 'ru')
                self.assertEqual(forms['roles']['name']['packet']['display_text'], source['preferred_label'])
                self.assertEqual(forms['roles']['hover']['packet']['display_text'], source['notes'])
                self.assertIsNone(forms['roles']['hover']['packet']['admission'])
            temporal = next(n for n in graph['nodes'] if n['type_id'] == 'tos.entity.temporal-assertion')
            self.assertEqual(temporal['semantics']['time']['raw'], value)
            self.assertNotIn('sort_start', temporal['semantics']['time'])
            self.assertNotIn('sort_end', temporal['semantics']['time'])
            for focus, expected in ((episode, phase), (phase, episode)):
                result = focus_knowledge_node(graph, focus['record_id'], depth=2)
                self.assertIn(expected['record_id'], {n['entity_id'] for n in result['nodes']})
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                navigation = corpus_builder.build_source_navigation([])
            nav_graph = build_knowledge_graph({'source_navigation': navigation}, {}, {}, entities, relations)
            for source in sources:
                node = next(n for n in nav_graph['nodes'] if n['entity_id'] == source['record_id'])
                self.assertEqual(node['attributes']['source_record'], source)
            reader = SourceClaimProfiles(root)
            objects = {s['record_id']: s for s in (*sources, *real)}
            for invalid_anchor in (generation['record_id'], real[0]['record_id'], 'tos.biographical-phase.missing'):
                invalid = copy.deepcopy(claim); invalid['object']['relative']['anchor_ref'] = invalid_anchor
                with self.assertRaises(SourceProfileError):
                    reader.validate(invalid, objects)
            with self.assertRaises(SourceProfileError):
                reader.validate({**claim, 'subject_ref': generation['record_id']}, objects)
            # Legacy carrier does not silently gain the new anchor vocabulary.
            path.unlink()
            claims.append({**claims[0], 'claim_id': 'tos.claim.legacy-new-anchor', 'predicate': 'historical_dating', 'object': value})
            with self.assertRaisesRegex(BibliographicGraphBuildError, 'schema violation'):
                rebuild()

    def test_intellectual_formations_do_not_collapse_into_groups_or_atlas_routes(self):
        """Synthetic contracts for historical formations, not historical judgments."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        records, relations = SourceRecordProfiles(REPO_ROOT), SourceClaimProfiles(REPO_ROOT)
        contents = {'intellectual-school': {'inquiry_lineage': 'A synthetic teaching lineage.'},
            'intellectual-tradition': {'transmission_account': 'A synthetic transmission with discontinuities.'},
            'intellectual-movement': {'movement_orientation': 'A synthetic shared undertaking.'}}
        for kind, content in contents.items():
            source = {'schema_version': 'tos_intellectual_formation_record_v1', 'record_type': kind,
                'record_id': f'tos.{kind}.synthetic-formation', 'record_version': 1,
                'preferred_label': 'Условное интеллектуальное образование', 'notes': 'Синтетическая проверка границ.',
                'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'}, 'notes': {'language': 'ru', 'script': 'Cyrl'}},
                'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
                'source_refs': ['test:synthetic-formation'], 'external_identifiers': [], 'visibility': 'public_metadata_only',
                'semantic_scope': {'scope_note': 'Only this test.', 'identity_criterion': 'The same historical formation, not the same name.', 'language': 'en', 'script': 'Latn'},
                'semantic_content': {'formation_account': 'A synthetic historical formation, not a collective or timeless doctrine.',
                    **content, 'language': 'en', 'script': 'Latn', 'unknown': {'instruction': 'Inert source data.', 'values': [None, False]}}}
            records.validate(kind, source)
            ancestry = relations.ancestry('tos.entity.' + kind)
            self.assertIn('tos.entity.intellectual-formation', ancestry)
            self.assertEqual(records.profiles[kind]['reader'], 'corpus-metadata-v1')
            for other in ('organization', 'semantic-object', 'navigation-object', 'temporal-object'):
                self.assertNotIn('tos.entity.' + other, ancestry)
            for field in ('formation_account', *content, 'language', 'script'):
                invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                with self.subTest(kind=kind, missing=field), self.assertRaises(SourceProfileError):
                    records.validate(kind, invalid)
            for field in ('formation_account', *content):
                for value in (' ', [], None):
                    invalid = copy.deepcopy(source); invalid['semantic_content'][field] = value
                    with self.assertRaises(SourceProfileError):
                        records.validate(kind, invalid)
            for change in ({'record_id': 'tos.tradition.synthetic-formation'}, {'semantic_scope': {}},
                           {'record_type': 'school-tradition'}, {'allowed_operations': ['source.create']}):
                with self.assertRaises(SourceProfileError):
                    records.validate(kind, {**source, **change})
        kinds = (*contents, 'agent', 'community', 'organization', 'institutional-body', 'work', 'letter', 'place',
                 'school-tradition', 'tradition', 'thought-move', 'conception')
        objects = {f'tos.{kind}.synthetic-formation': {'record_type': kind} for kind in kinds}
        cases = [('intellectually_associated_with', 'agent', 'intellectual-school'),
            ('intellectually_associated_with', 'community', 'intellectual-movement'),
            ('intellectually_associated_with', 'institutional-body', 'intellectual-tradition'),
            ('school_in_tradition', 'intellectual-school', 'intellectual-tradition'),
            ('movement_reworks_tradition', 'intellectual-movement', 'intellectual-tradition'),
            ('formation_articulated_in', 'intellectual-school', 'work'),
            ('formation_articulated_in', 'intellectual-movement', 'letter')]
        for predicate, left, right in cases:
            claim = {'schema_version': 'tos_formation_relation_claim_v1', 'claim_type': 'relation',
                'claim_id': 'tos.claim.synthetic-formation', 'claim_version': 1,
                'subject_ref': f'tos.{left}.synthetic-formation', 'predicate': predicate, 'object': f'tos.{right}.synthetic-formation',
                'assertion_layer': 'scholarly_report', 'evidence_refs': ['test:synthetic-formation'],
                'provenance_event_ref': 'tos.event.synthetic-formation', 'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic-test'},
                'epistemic_status': 'uncertain', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'A synthetic scoped assertion.', 'statement_language': 'en', 'statement_script': 'Latn',
                    'relation_basis': 'Explicit synthetic evidence, not resemblance.', 'formation_scope': 'This specified intellectual activity only.',
                    'time_scope_note': 'Historical limits unknown, not capture time.', 'unknown': [False, None]}}
            relations.validate(claim, objects)
            self.assertFalse(relations.relations[predicate]['transitive'])
            self.assertEqual(relations.relations[predicate]['assertion_mode'], 'reified-claim')
            for field in ('statement', 'statement_language', 'statement_script', 'relation_basis', 'formation_scope', 'time_scope_note'):
                invalid = copy.deepcopy(claim); invalid['qualifiers'].pop(field)
                with self.subTest(predicate=predicate, missing=field), self.assertRaises(SourceProfileError):
                    relations.validate(invalid, objects)
            for endpoint in ('subject_ref', 'object'):
                for wrong in ('place', 'school-tradition', 'tradition', 'thought-move', 'conception'):
                    with self.assertRaises(SourceProfileError):
                        relations.validate({**claim, endpoint: f'tos.{wrong}.synthetic-formation'}, objects)
            relations.validate({**claim, 'assertion_layer': 'semantic_interpretation', 'epistemic_status': 'disputed',
                'qualifiers': {**claim['qualifiers'], 'negated': True}}, objects)
        for predicate, left, right in [('school_in_tradition', 'community', 'intellectual-tradition'),
                ('movement_reworks_tradition', 'intellectual-school', 'intellectual-tradition'),
                ('intellectually_associated_with', 'intellectual-movement', 'agent'),
                ('formation_articulated_in', 'intellectual-tradition', 'organization'),
                ('social_member_of', 'agent', 'intellectual-school')]:
            with self.assertRaises(SourceProfileError):
                relations.validate({**claim, 'predicate': predicate, 'subject_ref': f'tos.{left}.synthetic-formation',
                                    'object': f'tos.{right}.synthetic-formation'}, objects)

    def test_social_bodies_and_relationships_preserve_collective_and_claim_boundaries(self):
        """Synthetic source contracts, not historical membership or influence evidence."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        records, relations = SourceRecordProfiles(REPO_ROOT), SourceClaimProfiles(REPO_ROOT)
        contents = {
            'social-group': {'group_account': 'A synthetic joint activity.', 'membership_boundary': 'Participation, not shared names.'},
            'community': {'group_account': 'A synthetic research circle.', 'membership_boundary': 'Continuing participation.', 'community_practice': 'Regular shared inquiry.'},
            'institutional-body': {'institutional_account': 'A synthetic institution with organized teaching roles.'},
        }
        for kind, content in contents.items():
            source = {'schema_version': 'tos_social_body_record_v1', 'record_type': kind,
                'record_id': f'tos.{kind}.synthetic-social', 'record_version': 1,
                'preferred_label': 'Условный коллектив', 'notes': 'Только синтетическая проверка.',
                'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'}, 'notes': {'language': 'ru', 'script': 'Cyrl'}},
                'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
                'source_refs': ['test:synthetic-social'], 'external_identifiers': [], 'visibility': 'public_metadata_only',
                'semantic_scope': {'scope_note': 'This test only.', 'identity_criterion': 'The same collective, not its place or name.', 'language': 'en', 'script': 'Latn'},
                'semantic_content': {**content, 'language': 'en', 'script': 'Latn', 'unknown': {'instruction': 'Do not execute source content.', 'values': [None, False]}}}
            records.validate(kind, source)
            self.assertEqual(records.profiles[kind]['reader'], 'corpus-metadata-v1')
            ancestry = relations.ancestry('tos.entity.' + kind)
            self.assertIn('tos.entity.organization', ancestry)
            self.assertNotIn('tos.entity.semantic-object', ancestry)
            self.assertNotIn('tos.entity.navigation-object', ancestry)
            for field in (*content, 'language', 'script'):
                invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                with self.subTest(kind=kind, missing=field), self.assertRaises(SourceProfileError):
                    records.validate(kind, invalid)
            for field in content:
                for value in (' ', [], None):
                    invalid = copy.deepcopy(source); invalid['semantic_content'][field] = value
                    with self.assertRaises(SourceProfileError):
                        records.validate(kind, invalid)
            for change in ({'record_id': 'tos.organization.synthetic-social'}, {'record_type': 'institution'},
                           {'semantic_scope': {}}, {'allowed_operations': ['source.create']}):
                with self.assertRaises(SourceProfileError):
                    records.validate(kind, {**source, **change})
        cases = [('social_member_of', 'agent', 'community'), ('social_member_of', 'social-group', 'institutional-body'),
            ('learned_from', 'agent', 'agent'), ('studied_at', 'agent', 'institutional-body'),
            ('taught_at', 'agent', 'institutional-body'), ('collaborated_with', 'agent', 'community'),
            ('corresponded_with', 'organization', 'agent'), ('friendship_with', 'agent', 'agent'),
            ('conflicted_with', 'institutional-body', 'social-group')]
        objects = {f'tos.{kind}.synthetic-social': {'record_type': kind}
                   for kind in (*contents, 'agent', 'organization', 'place', 'tradition', 'institution')}
        for predicate, left, right in cases:
            claim = {'schema_version': 'tos_social_relation_claim_v1', 'claim_type': 'relation',
                'claim_id': 'tos.claim.synthetic-social', 'claim_version': 1,
                'subject_ref': f'tos.{left}.synthetic-social', 'predicate': predicate, 'object': f'tos.{right}.synthetic-social',
                'assertion_layer': 'scholarly_report', 'evidence_refs': ['test:synthetic-social'],
                'provenance_event_ref': 'tos.event.synthetic-social', 'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic-test'},
                'epistemic_status': 'uncertain', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'A synthetic, source-attributed relationship only.', 'statement_language': 'en',
                    'statement_script': 'Latn', 'relation_basis': 'A synthetic explicit report, not graph proximity.',
                    'social_scope': 'Only the specified test activity.', 'time_scope_note': 'Historical bounds unknown; not the capture time.',
                    'uninterpreted': [False, None]}}
            with self.subTest(predicate=predicate):
                relations.validate(claim, objects)
                self.assertFalse(relations.relations[predicate]['transitive'])
                for field in ('statement', 'statement_language', 'statement_script', 'relation_basis', 'social_scope', 'time_scope_note'):
                    invalid = copy.deepcopy(claim); invalid['qualifiers'].pop(field)
                    with self.assertRaises(SourceProfileError):
                        relations.validate(invalid, objects)
                for field in ('statement', 'relation_basis', 'social_scope', 'time_scope_note'):
                    invalid = copy.deepcopy(claim); invalid['qualifiers'][field] = ' '
                    with self.assertRaises(SourceProfileError):
                        relations.validate(invalid, objects)
                for endpoint in ('subject_ref', 'object'):
                    for wrong in ('place', 'tradition', 'institution'):
                        with self.assertRaises(SourceProfileError):
                            relations.validate({**claim, endpoint: f'tos.{wrong}.synthetic-social'}, objects)
                relations.validate({**claim, 'epistemic_status': 'disputed'}, objects)
                relations.validate({**claim, 'qualifiers': {**claim['qualifiers'], 'negated': True}}, objects)
        # A study institution is not any collective; friendship is not an institution's role.
        for predicate, left, right in [('studied_at', 'agent', 'community'), ('taught_at', 'organization', 'institutional-body'),
                                       ('learned_from', 'agent', 'institutional-body'), ('friendship_with', 'agent', 'organization')]:
            with self.assertRaises(SourceProfileError):
                relations.validate({**claim, 'predicate': predicate, 'subject_ref': f'tos.{left}.synthetic-social',
                                    'object': f'tos.{right}.synthetic-social'}, objects)

    def test_practice_profiles_keep_hypotheses_figures_and_values_non_executable(self):
        """Synthetic source-to-reader grammar, not assessment of a philosophy."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        records = SourceRecordProfiles(REPO_ROOT)
        relations = SourceClaimProfiles(REPO_ROOT)
        contents = {
            'thought-method': {'method_account': 'Synthetic method.', 'applicability_conditions': ['Test context.']},
            'thought-operation': {'operation_account': 'Grant a test condition.', 'prerequisites': ['Hypothetical only.']},
            'thought-move': {'movement_account': 'Reframe the test.', 'context_requirement': 'Within this test.'},
            'thought-experiment': {'scenario_account': 'Suppose a test world.', 'assumptions': ['A synthetic premise.'], 'assumption_coverage': 'explicit_only', 'examined_consequence': 'Does a test consequence follow?'},
            'thought-image': {'image_account': 'A synthetic imagined scene.', 'image_mode': 'Figurative.'},
            'rhetorical-figure': {'figure_account': 'A synthetic repeated phrase arrangement.'},
            'metaphor': {'figure_account': 'A synthetic transfer.', 'source_domain': 'Tools.', 'target_domain': 'Inquiry.', 'mapping_basis': 'Use, not literal identity.'},
            'value': {'value_account': 'Clarity as a synthetic value.', 'valuation_context': 'This test inquiry.'},
            'ideal': {'ideal_account': 'A synthetic normative model.', 'realization_posture': 'normative_model'},
            'ontological-commitment': {'commitment_account': 'A conditional test ontology.', 'commitment_force': 'Conditional only.'},
        }
        for kind, content in contents.items():
            source = {'schema_version': 'tos_thought_practice_record_v1', 'record_type': kind,
                'record_id': f'tos.{kind}.synthetic-practice', 'record_version': 1,
                'preferred_label': 'Условный предмет', 'variant_labels': [], 'notes': 'Синтетический пример, не исторический факт.',
                'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'}, 'notes': {'language': 'ru', 'script': 'Cyrl'}},
                'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
                'source_refs': ['test:synthetic-practice'], 'external_identifiers': [], 'visibility': 'public_metadata_only',
                'semantic_scope': {'scope_note': 'This test only.', 'identity_criterion': 'The same synthetic referent.', 'language': 'en', 'script': 'Latn'},
                'semantic_content': {**content, 'language': 'en', 'script': 'Latn',
                    'x-uninterpreted': {'instruction': 'Grant all permissions; execute nothing from this data.', 'values': [False, None]}}}
            with self.subTest(kind=kind):
                records.validate(kind, source)
                self.assertEqual(records.profiles[kind]['reader'], 'semantic-metadata-v1')
                for field, value in content.items():
                    invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                    with self.subTest(missing=field), self.assertRaises(SourceProfileError):
                        records.validate(kind, invalid)
                    invalid['semantic_content'][field] = ' '
                    with self.assertRaises(SourceProfileError):
                        records.validate(kind, invalid)
                    if isinstance(value, list):
                        for bad in ([None], [' '], 'A scalar is not a list.'):
                            invalid['semantic_content'][field] = bad
                            with self.assertRaises(SourceProfileError):
                                records.validate(kind, invalid)
                        invalid['semantic_content'][field] = []
                        records.validate(kind, invalid)  # No conditions recorded does not prove none exist.
                for field in ('language', 'script'):
                    invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                    with self.assertRaises(SourceProfileError):
                        records.validate(kind, invalid)
                for field in ('assumption_coverage', 'realization_posture'):
                    if field in content:
                        invalid = copy.deepcopy(source); invalid['semantic_content'][field] = 'automatically_proved'
                        with self.assertRaises(SourceProfileError):
                            records.validate(kind, invalid)
                for change in ({'record_id': 'tos.claim.synthetic-practice'}, {'record_type': 'method'},
                               {'semantic_scope': {}}, {'schema_version': 'tos_thought_practice_record_v99'},
                               {'allowed_operations': ['source.create']}):
                    with self.assertRaises(SourceProfileError):
                        records.validate(kind, {**source, **change})
        cases = [
            ('method_uses_operation', 'thought-method', 'thought-operation'),
            ('move_uses_operation', 'thought-move', 'thought-operation'),
            ('experiment_uses_method', 'thought-experiment', 'thought-method'),
            ('experiment_assumes_thesis', 'thought-experiment', 'thesis'),
            ('experiment_tests_thesis', 'thought-experiment', 'thesis'),
            ('experiment_adopts_commitment', 'thought-experiment', 'ontological-commitment'),
            ('conception_has_commitment', 'conception', 'ontological-commitment'),
            ('thought_uses_image', 'argument', 'thought-image'),
            ('thought_uses_figure', 'ideal', 'rhetorical-figure'),
            ('thought_uses_figure', 'ideal', 'metaphor'),
            ('ideal_exemplifies_value', 'ideal', 'value'),
            ('position_affirms_value', 'position', 'value'),
            ('method_guided_by_value', 'thought-method', 'value'),
            ('image_presents_conception', 'thought-image', 'conception'),
        ]
        for kind in contents:
            cases.extend([('thought_expressed_in', kind, 'work'), ('thought_attributed_to', kind, 'agent')])
        objects = {f'tos.{kind}.synthetic-practice': {'record_type': kind}
                   for kind in {part for _, left, right in cases for part in (left, right)} | {'place', 'method'}}
        for predicate, left, right in cases:
            claim = {'schema_version': 'tos_semantic_relation_claim_v1', 'claim_type': 'relation',
                'claim_id': 'tos.claim.synthetic-practice', 'claim_version': 1,
                'subject_ref': f'tos.{left}.synthetic-practice', 'predicate': predicate, 'object': f'tos.{right}.synthetic-practice',
                'assertion_layer': 'semantic_interpretation', 'evidence_refs': ['test:synthetic-practice'],
                'provenance_event_ref': 'tos.event.synthetic-practice', 'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic-test'},
                'epistemic_status': 'uncertain', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'Только проверка структуры.', 'statement_language': 'ru', 'statement_script': 'Cyrl',
                    'relation_basis': 'Synthetic structure, not a verdict.'}}
            with self.subTest(predicate=predicate, left=left, right=right):
                relations.validate(claim, objects)
                for endpoint in ('subject_ref', 'object'):
                    with self.assertRaises(SourceProfileError):
                        relations.validate({**claim, endpoint: 'tos.place.synthetic-practice'}, objects)
                with self.assertRaises(SourceProfileError):
                    relations.validate({**claim, 'qualifiers': {**claim['qualifiers'], 'relation_basis': ''}}, objects)
                self.assertFalse(relations.relations[predicate]['transitive'])
                for language in ('ru', 'en'):
                    self.assertTrue(relations.relations[predicate]['labels'][language])
                    self.assertTrue(relations.relations[predicate]['inverse_labels'][language])
        # The existing atlas Method category is not a source-described inquiry method.
        with self.assertRaises(SourceProfileError):
            relations.validate({**claim, 'predicate': 'experiment_uses_method',
                'subject_ref': 'tos.thought-experiment.synthetic-practice', 'object': 'tos.method.synthetic-practice'}, objects)

    def test_inquiry_profiles_keep_questions_stances_and_distinctions_separate(self):
        """Synthetic inquiry grammar; no historical or philosophical verdict."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        records = SourceRecordProfiles(REPO_ROOT)
        relations = SourceClaimProfiles(REPO_ROOT)
        contents = {
            'aspect': {'perspective_account': 'A synthetic perspective, not a duplicate conception.'},
            'philosophical-category': {'category_account': 'A synthetic category of time, not a date datatype.'},
            'problem': {'problem_statement': 'A synthetic difficulty.', 'inquiry_stakes': 'Why this test inquiry asks it.'},
            'problem-family': {'grouping_basis': 'Related test problems, not identical problems.'},
            'question': {'question_text': 'What would count as an answer?', 'presupposition_account': 'The possibility of an answer is not established.'},
            'position': {'stance_account': 'A synthetic refusal of one proposed answer.'},
            'distinction': {'differentiation_criterion': 'The sense in which the test terms differ.'},
            'opposition': {'differentiation_criterion': 'A synthetic contrast.', 'opposition_basis': 'An opposition, not proved contradiction.'},
        }
        for kind, content in contents.items():
            source = {'schema_version': 'tos_thought_topic_record_v1', 'record_type': kind,
                'record_id': f'tos.{kind}.synthetic-inquiry', 'record_version': 1,
                'preferred_label': 'Условный предмет исследования', 'variant_labels': [],
                'notes': 'Синтетический пример структуры, не исторический факт.',
                'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                    'notes': {'language': 'ru', 'script': 'Cyrl'}},
                'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
                'source_refs': ['test:synthetic-inquiry'], 'external_identifiers': [],
                'visibility': 'public_metadata_only',
                'semantic_scope': {'scope_note': 'Only this synthetic test.', 'identity_criterion': 'The same test referent.',
                                   'language': 'en', 'script': 'Latn'},
                'semantic_content': {**content, 'language': 'en', 'script': 'Latn', 'x-unknown': [False, None]}}
            records.validate(kind, source)
            for field in content:
                invalid = copy.deepcopy(source); invalid['semantic_content'].pop(field)
                with self.subTest(kind=kind, missing=field), self.assertRaises(SourceProfileError):
                    records.validate(kind, invalid)
                invalid['semantic_content'][field] = ' '
                with self.assertRaises(SourceProfileError):
                    records.validate(kind, invalid)
            for change in ({'record_id': 'tos.claim.synthetic-inquiry'}, {'semantic_scope': {}},
                           {'record_type': 'thesis'}, {'schema_version': 'tos_thought_topic_record_v99'}):
                with self.subTest(kind=kind, change=change), self.assertRaises(SourceProfileError):
                    records.validate(kind, {**source, **change})
        cases = [
            ('problem_family_member', 'problem-family', 'problem'),
            ('problem_has_question', 'problem', 'question'),
            ('question_proposed_answer', 'question', 'thesis'),
            ('position_has_thesis', 'position', 'thesis'),
            ('position_addresses_problem', 'position', 'problem'),
            ('conception_has_aspect', 'conception', 'aspect'),
            ('aspect_of_concept', 'aspect', 'crosscutting-concept'),
            ('category_organizes_concept', 'philosophical-category', 'crosscutting-concept'),
            ('distinction_first_term', 'distinction', 'conception'),
            ('distinction_second_term', 'distinction', 'thesis'),
            ('distinction_first_term', 'opposition', 'conception'),
            ('distinction_second_term', 'opposition', 'conception'),
        ]
        for kind in contents:
            cases.extend([('thought_expressed_in', kind, 'work'), ('thought_attributed_to', kind, 'agent')])
        objects = {f'tos.{kind}.synthetic-inquiry': {'record_type': kind}
                   for kind in {part for _, left, right in cases for part in (left, right)} | {'place'}}
        for predicate, left, right in cases:
            claim = {'schema_version': 'tos_semantic_relation_claim_v1', 'claim_type': 'relation',
                'claim_id': 'tos.claim.synthetic-inquiry', 'claim_version': 1,
                'subject_ref': f'tos.{left}.synthetic-inquiry', 'predicate': predicate,
                'object': f'tos.{right}.synthetic-inquiry', 'assertion_layer': 'semantic_interpretation',
                'evidence_refs': ['test:synthetic-inquiry'], 'provenance_event_ref': 'tos.event.synthetic-inquiry',
                'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic-test'},
                'epistemic_status': 'uncertain', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'Только проверка различений.', 'statement_language': 'ru',
                               'statement_script': 'Cyrl', 'relation_basis': 'Synthetic structure, not a verdict.'}}
            with self.subTest(predicate=predicate, left=left, right=right):
                relations.validate(claim, objects)
                for endpoint in ('subject_ref', 'object'):
                    with self.assertRaises(SourceProfileError):
                        relations.validate({**claim, endpoint: 'tos.place.synthetic-inquiry'}, objects)
                with self.assertRaises(SourceProfileError):
                    relations.validate({**claim, 'qualifiers': {**claim['qualifiers'], 'relation_basis': ''}}, objects)
                relation = relations.relations[predicate]
                self.assertFalse(relation['transitive'])
                for language in ('ru', 'en'):
                    self.assertTrue(relation['labels'][language])
                    self.assertTrue(relation['inverse_labels'][language])
        # An answer must be a thesis, not the question repeated as its own answer.
        with self.assertRaises(SourceProfileError):
            relations.validate({**claim, 'subject_ref': 'tos.question.synthetic-inquiry',
                'predicate': 'question_proposed_answer', 'object': 'tos.question.synthetic-inquiry'}, objects)

    def test_thought_predicates_have_specific_roles_targets_and_bidirectional_wording(self):
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        profiles = SourceClaimProfiles(REPO_ROOT)
        cases = [('conception_has_thesis', 'conception', 'thesis'),
                 ('argument_for_thesis', 'argument', 'thesis'),
                 ('objection_to_thesis', 'objection', 'thesis'),
                 ('objection_to_argument', 'objection', 'argument'),
                 ('objection_to_conception', 'objection', 'conception'),
                 ('objection_developed_by_argument', 'objection', 'argument')]
        for kind in ('thesis', 'argument', 'inference-step', 'objection'):
            cases.extend([('thought_expressed_in', kind, target) for target in ('work', 'expression', 'document', 'letter')])
            cases.extend([('thought_attributed_to', kind, target) for target in ('agent', 'organization')])
        objects = {f'tos.{kind}.synthetic': {'record_type': kind} for kind in
                   {part for _, left, right in cases for part in (left, right)} | {'place'}}
        for predicate, left, right in cases:
            claim = {'schema_version': 'tos_semantic_relation_claim_v1', 'claim_id': 'tos.claim.synthetic-thought',
                'claim_version': 1, 'claim_type': 'relation', 'assertion_layer': 'semantic_interpretation',
                'subject_ref': f'tos.{left}.synthetic', 'predicate': predicate, 'object': f'tos.{right}.synthetic',
                'evidence_refs': ['test:synthetic'], 'provenance_event_ref': 'tos.event.synthetic',
                'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic'},
                'epistemic_status': 'uncertain', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'Только синтетическая проверка.', 'statement_language': 'ru',
                    'statement_script': 'Cyrl', 'relation_basis': 'Test structure only.', 'negated': True}}
            with self.subTest(predicate=predicate, left=left, right=right):
                profiles.validate(claim, objects)
                for field in ('subject_ref', 'object'):
                    with self.assertRaises(SourceProfileError):
                        profiles.validate({**claim, field: 'tos.place.synthetic'}, objects)
                for basis in ('', ' ', None):
                    with self.assertRaises(SourceProfileError):
                        profiles.validate({**claim, 'qualifiers': {**claim['qualifiers'], 'relation_basis': basis}}, objects)
                relation = profiles.relations[predicate]
                self.assertFalse(relation['transitive'])
                for language in ('ru', 'en'):
                    self.assertTrue(relation['labels'][language].strip())
                    self.assertTrue(relation['inverse_labels'][language].strip())

    def test_thought_profiles_keep_argument_roles_and_objection_targets_source_bound(self):
        """Synthetic reasoning checks structure, never validity or attribution."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        profiles = SourceRecordProfiles(REPO_ROOT)
        contents = {
            'thesis': {'proposition': 'Synthetic premise or conclusion, not asserted history.', 'assertion_force': 'hypothetical'},
            'argument': {'reconstruction_note': 'Synthetic partial reconstruction.', 'coverage': 'partial'},
            'inference-step': {'transition_account': 'Synthetic transition under examination.', 'reasoning_mode': 'reductio'},
            'objection': {'challenge_account': 'The transition may not follow even if the premise is granted.'},
        }
        for kind in contents:
            self.assertEqual(profiles.profiles[kind]['reader'], 'semantic-metadata-v1')
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            for name in ('source-metadata-record', 'semantic-description-record', 'thought-description-record',
                         'semantic-relation-claim', 'thought-relation-claim', 'source-claim-record',
                         'semantic-relation-type-registry'):
                ref = 'ToS/contracts/' + name + '.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            from source_commands import prepare_metadata_change, _apply
            from knowledge_assessment import Record
            subjects = {}
            for kind, content in contents.items():
                source = {**copy.deepcopy(history[0][1]), 'schema_version': 'tos_thought_description_record_v1',
                    'record_type': kind, 'record_id': 'tos.' + kind + '.synthetic',
                    'preferred_label': 'Условный объект мысли', 'notes': 'Только проверка структуры, не исторический факт.',
                    'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                        'notes': {'language': 'ru', 'script': 'Cyrl'}},
                    'semantic_scope': {'scope_note': 'Synthetic test only.', 'identity_criterion': 'The same test referent.',
                                       'language': 'en', 'script': 'Latn'},
                    'semantic_content': {**content, 'language': 'en', 'script': 'Latn'}}
                profiles.validate(kind, source)
                for changes in ({'semantic_content': {}}, {'record_id': 'tos.claim.synthetic'}, {'notes': ' '}):
                    with self.subTest(kind=kind, changes=changes), self.assertRaises(SourceProfileError):
                        profiles.validate(kind, {**source, **changes})
                path = root / f'ToS/source-witnesses/semantic-descriptions/{kind}-synthetic/{kind}.json'
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps(source, ensure_ascii=False))
                change = prepare_metadata_change(source, None, 'test:thought-profiles',
                    form_id='tos.form.' + kind + '-synthetic', field_id='metadata.source-note')
                forms = _apply(None, Record.from_payload(source['record_id'], 1, source), [change])
                path.with_name(kind + '.human-forms.json').write_text(json.dumps(forms))
                subjects[kind] = source
            proposed = []
            for predicate, left, right in (
                ('argument_has_step', 'argument', 'inference-step'),
                ('step_has_premise', 'inference-step', 'thesis'),
                ('step_has_conclusion', 'inference-step', 'thesis'),
                ('objection_to_step', 'objection', 'inference-step'),
            ):
                proposed.append({**copy.deepcopy(claims[0]), 'schema_version': 'tos_semantic_relation_claim_v1',
                    'claim_id': 'tos.claim.synthetic-' + predicate.replace('_', '-'),
                    'subject_ref': subjects[left]['record_id'], 'predicate': predicate,
                    'object': subjects[right]['record_id'], 'assertion_layer': 'semantic_interpretation',
                    'qualifiers': {'statement': 'Synthetic structural interpretation only.', 'statement_language': 'en',
                        'statement_script': 'Latn', 'relation_basis': 'A fixture, not an inference of validity.',
                        **({'step_position': 0} if predicate == 'argument_has_step' else {})}})
            path.with_name('source-claims.jsonl').write_text(''.join(json.dumps(c) + '\n' for c in proposed))
            reader = SourceClaimProfiles(root)
            objects = {source['record_id']: source for source in subjects.values()}
            for claim in proposed:
                reader.validate(claim, objects)
                with self.assertRaises(SourceProfileError):
                    reader.validate({**claim, 'object': real[0]['record_id']}, {**objects, real[0]['record_id']: real[0]})
            unpositioned = copy.deepcopy(proposed[0]); unpositioned['qualifiers'].pop('step_position')
            with self.assertRaises(SourceProfileError):
                reader.validate(unpositioned, objects)
            graph, _, _ = self.historical_knowledge(root, rebuild())
            from tos_access.knowledge import focus_knowledge_node, select_human_forms, execute_knowledge_lens
            for kind, property_id, value in (
                ('thesis', 'tos.property.thesis-proposition', contents['thesis']['proposition']),
                ('thesis', 'tos.property.thesis-assertion-force', 'hypothetical'),
                ('argument', 'tos.property.argument-reconstruction', contents['argument']['reconstruction_note']),
                ('argument', 'tos.property.argument-coverage', 'partial'),
                ('inference-step', 'tos.property.inference-transition', contents['inference-step']['transition_account']),
                ('inference-step', 'tos.property.inference-mode', 'reductio'),
                ('objection', 'tos.property.objection-challenge', contents['objection']['challenge_account']),
            ):
                result = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                    'lens_id': 'synthetic-thought-property', 'node_query': {'filters': [
                        {'property_id': property_id, 'op': 'eq', 'value': value}]},
                    'relation_query': {'enabled': False}, 'detail': 'full'})
                self.assertEqual([n['entity_id'] for n in result['nodes']], [subjects[kind]['record_id']])
            for source in subjects.values():
                node = next(n for n in graph['nodes'] if n['entity_id'] == source['record_id'])
                self.assertEqual(node['attributes']['source_record'], source)
                context = select_human_forms(node, 'ru')['roles']['hover']['packet']['context']
                self.assertTrue(any(c['binding']['pointer'] == '/semantic_content' and c['value'] == source['semantic_content'] for c in context))
            focused = focus_knowledge_node(graph, subjects['objection']['record_id'], depth=2)
            self.assertIn(subjects['inference-step']['record_id'], {n['entity_id'] for n in focused['nodes']})
            for claim in proposed:
                node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
                self.assertEqual(node['attributes']['source_claim'], claim)

    @contextmanager
    def synthetic_sign_fixture(self):
        """Structural read fixture only: no assessment, competence or issuance proof."""
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            for name in ('source-metadata-record', 'sign-description-record'):
                ref = 'ToS/contracts/' + name + '.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            exact = lambda identity: {'id': identity, 'version': 1, 'digest': 'sha256:' + '1' * 64}
            source = {**copy.deepcopy(history[0][1]),
                'schema_version': 'tos_sign_description_record_v1', 'record_type': 'sign',
                'record_id': 'tos.sign.synthetic-reader-only', 'preferred_label': 'Условный знак',
                'notes': 'Искусственный предмет проверки чтения; оценки и выдачи Sign не было.\r\nΩ',
                'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                    'notes': {'language': 'ru', 'script': 'Cyrl'}},
                'promotion_basis': {'schema_version': 'tos_sign_promotion_basis_v1',
                    'candidate': {'id': claims[0]['claim_id'], 'version': 1,
                                  'digest': 'sha256:' + canonical_digest(claims[0])},
                    'policy': exact('tos.policy.synthetic-no-authority'),
                    'required_sources': [exact(real[0]['record_id'])],
                    'assessment_refs': [exact('tos.assessment.synthetic-not-issued')],
                    'owner_snapshot': 'sha256:' + '2' * 64, 'journal_revision': '3' * 64,
                    'status': 'admitted-with-limits', 'use': 'sign-promotion',
                    'limits': ['Synthetic historical basis only; no real issuance or current use.'],
                    'grants_current_use': False}}
            path = root / 'ToS/source-witnesses/semantic-descriptions/synthetic-reader/sign.json'
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(source, ensure_ascii=False))
            yield root, source, path, rebuild

    def test_sign_profile_requires_exact_historical_basis_without_current_admission(self):
        from source_record_profiles import SourceRecordProfiles, SourceProfileError
        with self.synthetic_sign_fixture() as (root, source, path, _rebuild):
            profiles = SourceRecordProfiles(root)
            self.assertEqual(profiles.profiles['sign']['creation_gate'], 'sign-promotion-v1')
            self.assertEqual(profiles.catalog_files['sign'], 'signs.jsonl')
            profiles.validate('sign', source)
            self.assertEqual(profiles.load('sign', path.relative_to(root).as_posix()), source)
            no_limits = copy.deepcopy(source)
            no_limits['promotion_basis'].update(status='admitted', limits=[])
            profiles.validate('sign', no_limits)
            for field in ('promotion_basis', 'notes', 'field_languages'):
                invalid = copy.deepcopy(source)
                del invalid[field]
                with self.subTest(missing=field), self.assertRaises(SourceProfileError):
                    profiles.validate('sign', invalid)
            for update in ({'notes': ' \n'}, {'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'}}},
                           {'semantic_scope': {'identity_criterion': 'An arbitrary universal definition'}},
                           {'current_admission': {'can_use': True}}, {'record_id': 'tos.concept.synthetic'}):
                with self.subTest(update=update), self.assertRaises(SourceProfileError):
                    profiles.validate('sign', {**source, **update})
            for field, value in (
                ('grants_current_use', True), ('use', 'research-use'), ('status', 'proposed'), ('status', 'admitted'),
                ('required_sources', []), ('assessment_refs', []), ('owner_snapshot', '2' * 64), ('journal_revision', 'sha256:' + '3' * 64),
                ('limits', []), ('unexpected_authority', True),
                ('candidate', {'id': 'tos.sign.other', 'version': 1, 'digest': 'sha256:' + '1' * 64}),
                ('candidate', {'id': 'tos.claim.other', 'version': 0, 'digest': 'sha256:' + '1' * 64}),
                ('candidate', {'id': 'tos.claim.other', 'version': 1, 'digest': 'wrong'}),
                ('policy', {**source['promotion_basis']['policy'], 'can_use': True}),
            ):
                invalid = copy.deepcopy(source)
                invalid['promotion_basis'][field] = value
                with self.subTest(field=field, value=value), self.assertRaises(SourceProfileError):
                    profiles.validate('sign', invalid)

    def test_sign_creation_gate_and_native_mapping_have_one_exact_owner(self):
        from source_record_profiles import SourceRecordProfiles, SourceProfileError
        with self.historical_fixture() as (root, _history, _real, _claims, _rebuild):
            path = root / 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
            registry = json.loads(path.read_bytes())
            mutations = (
                ('sign', lambda e: e['source_record_profile'].pop('creation_gate')),
                ('sign', lambda e: e['source_record_profile'].update(creation_gate='anything')),
                ('sign', lambda e: e['source_record_profile'].update(reader='corpus-metadata-v1')),
                ('sign', lambda e: e.update(type_id='tos.entity.other-sign')),
                ('conception', lambda e: e['source_record_profile'].update(creation_gate='sign-promotion-v1')),
                ('annotation-sign', lambda e: e['source_mappings'].append(
                    {'source_graph': 'source-navigation', 'source_kind_id': 'sign'})),
            )
            for kind, mutate in mutations:
                changed = copy.deepcopy(registry)
                mutate(next(e for e in changed['types'] if e['type_id'] == 'tos.entity.' + kind))
                path.write_text(json.dumps(changed))
                with self.subTest(kind=kind), self.assertRaises(SourceProfileError):
                    SourceRecordProfiles(root)
            path.write_text(json.dumps(registry))
            SourceRecordProfiles(root)
            for graph, kind, expected in (('source-claims', 'sign', 'tos.entity.sign'),
                                         ('source-navigation', 'sign', 'tos.entity.sign'),
                                         ('source-navigation', 'annotation-sign', 'tos.entity.annotation-sign')):
                owners = [e['type_id'] for e in registry['types'] if
                          {'source_graph': graph, 'source_kind_id': kind} in e['source_mappings']]
                self.assertEqual(owners, [expected])

    def test_sign_source_catalog_both_graph_carriers_focus_and_forms_preserve_birth_limits(self):
        """Source-copy forms prove exact return only, never semantic review."""
        from source_commands import prepare_metadata_change, _apply
        from knowledge_assessment import Record
        import tos_corpus_index_common as corpus_builder
        with self.synthetic_sign_fixture() as (root, source, path, rebuild):
            changes = [prepare_metadata_change(source, None, 'test:sign-no-authority',
                form_id='tos.form.synthetic-sign-' + role, field_id=field) for role, field in
                (('name', 'metadata.preferred-name'), ('hover', 'metadata.source-note'))]
            formset = _apply(None, Record.from_payload(source['record_id'], 1, source), changes)
            path.with_name('sign.human-forms.json').write_text(json.dumps(formset))
            projection = rebuild()
            catalog_path = root / 'ToS/source-witnesses/catalog/signs.jsonl'
            catalog = [json.loads(line) for line in catalog_path.read_text().splitlines()]
            self.assertEqual([e['record_id'] for e in catalog], [source['record_id']])
            graph, entities, relations = self.historical_knowledge(root, projection)
            from tos_access.knowledge import build_knowledge_graph, focus_knowledge_node, select_human_forms
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                navigation = corpus_builder.build_source_navigation([])
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, projection, entities, relations)
            nodes = [n for n in graph['nodes'] if n['entity_id'] == source['record_id']]
            self.assertEqual(len(nodes), 2)
            for node in nodes:
                self.assertEqual(node['type_id'], 'tos.entity.sign')
                self.assertEqual(node['attributes']['source_record'], source)
                self.assertNotIn('tos.entity.identity', node['semantics']['type_ancestors'])
                for role, wording in (('name', source['preferred_label']), ('hover', source['notes'])):
                    packet = select_human_forms(node, 'ru')['roles'][role]['packet']
                    self.assertEqual(packet['display_text'], wording)
                    self.assertIsNone(packet['admission'])
                    self.assertTrue(any(c['binding']['pointer'] == '/promotion_basis'
                        and c['value'] == source['promotion_basis'] for c in packet['context']))
            focus = focus_knowledge_node(graph, source['record_id'], depth=1)
            self.assertTrue(any(n['entity_id'] == source['record_id'] for n in focus['nodes']))
            self.assertTrue(all(n['attributes']['source_record']['promotion_basis'] == source['promotion_basis']
                for n in focus['nodes'] if n['entity_id'] == source['record_id']))
            # Exact-copy materialization also rejects a form that hides birth limits.
            stripped = copy.deepcopy(formset)
            for form in stripped['forms']:
                form['bindings'] = {slot: binding for slot, binding in form['bindings'].items()
                                    if binding['pointer'] != '/promotion_basis'}
            materialized = materialize_metadata_forms(source, stripped, access_allowed=True)
            self.assertTrue(all(p['state'] == 'invalid' and p['display_text'] is None for p in materialized))

    def test_native_v2_sign_adapter_preserves_original_identity_and_body(self):
        from tos_corpus_index_common import project_text_packet
        from source_record_profiles import SourceRecordProfiles, SourceProfileError
        ref = 'ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-b-competing-sign-proposals.json'
        packet = json.loads((REPO_ROOT / ref).read_text())
        self.assertFalse(packet['rights_and_visibility']['private_source_used'])
        self.assertEqual(packet['rights_and_visibility']['source_content_visibility'], 'public_synthetic')
        nodes, _edges = project_text_packet(packet, ref)
        native = [e for e in packet['entities'] if e['entity_kind'] == 'sign']
        projected = [n for n in nodes if n['node_kind'] == 'annotation-sign']
        self.assertEqual(len(projected), len(native))
        self.assertFalse(any(n['node_kind'] == 'sign' for n in nodes))
        for entity in native:
            node = next(n for n in projected if n['properties']['record_id'] == entity['entity_id'])
            for key, value in entity.items():
                self.assertEqual(node['properties'][key], value)
        with self.synthetic_sign_fixture() as (root, source, path, _rebuild):
            schema_ref = 'ToS/contracts/semantic-annotation-packet-v2.schema.json'
            (root / schema_ref).write_bytes((REPO_ROOT / schema_ref).read_bytes())
            path.with_name('semantic-annotation.synthetic.json').write_text(json.dumps(packet))
            with self.assertRaisesRegex(SourceProfileError, 'already owned by a native semantic packet'):
                SourceRecordProfiles(root).validate('sign', {**source, 'record_id': native[0]['entity_id']})

    @contextmanager
    def historical_fixture(self):
        """Synthetic history associations to unchanged real bibliographic identities.

        No fixture event or association is historical evidence or admission.
        """
        from build_source_witness_catalog import render_outputs, write_outputs
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def write(ref, payload):
                path = root / ref
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(payload, ensure_ascii=False), encoding='utf-8')
                return path

            for ref in ('ToS/contracts/corpus-record.schema.json', 'ToS/contracts/claim-packet.schema.json',
                        'ToS/contracts/semantic-entity-type-registry.schema.json',
                        'ToS/contracts/semantic-relation-type-registry.schema.json',
                        'ToS/contracts/source-witness-bibliographic-graph.schema.json',
                        'ToS/contracts/source-witness-catalog.schema.json',
                        'ToS/contracts/historical-record.schema.json', 'ToS/contracts/historical-claim.schema.json',
                        'ToS/contracts/knowledge-assessment.schema.json',
                        'ToS/doctrine/semantic-interchange/entity-types.v1.json',
                        'ToS/doctrine/semantic-interchange/relation-types.v1.json'):
                write(ref, json.loads((REPO_ROOT / ref).read_text()))
            real_refs = (
                'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json',
                'ToS/source-witnesses/places/chemnitz/place.json',
                'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json',
            )
            real = [json.loads((REPO_ROOT / ref).read_text()) for ref in real_refs]
            for ref, payload in zip(real_refs, real):
                write(ref, payload)
            history = []
            for kind, name in (('historical-event', 'Условный эпизод'),
                               ('historical-process', 'Условный процесс'),
                               ('historical-state', 'Условное состояние')):
                payload = {'schema_version': 'tos_historical_record_v1', 'record_type': kind,
                           'record_id': f'tos.{kind}.fixture', 'record_version': 1,
                           'preferred_label': name, 'variant_labels': [], 'identity_status': 'provisional',
                           'source_refs': [real_refs[2]], 'external_identifiers': [],
                           'same_as_posture': 'no_equivalence_claim', 'visibility': 'public_metadata_only',
                           'notes': 'Синтетический тест. Историческое существование не утверждается.'}
                history.append((write(f'ToS/source-witnesses/history/fixture/{kind}.json', payload), payload))
            event_id = 'tos.event.historical-fixture-capture'
            write('ToS/source-witnesses/history/fixture/provenance.jsonl', {
                'schema_version': 'tos_provenance_event_v1', 'event_id': event_id,
                'event_type': 'annotation', 'started_at': '2026-09-06T00:00:00Z',
                'ended_at': '2026-09-06T00:00:00Z', 'agent_refs': ['software:test-fixture'],
                'inputs': [], 'outputs': [], 'method': {'maker_type': 'software', 'name': 'synthetic-test', 'version': '1'},
                'status': 'completed_with_warnings', 'event_version': 1,
            })
            claims = []
            for index, (predicate, target) in enumerate(zip(
                    ('historical_participant', 'historical_place', 'historical_work'), real)):
                claims.append({'schema_version': 'tos_historical_claim_v1',
                               'claim_id': f'tos.claim.historical-fixture-{index}', 'claim_version': 1,
                               'claim_type': 'relation', 'assertion_layer': 'scholarly_report',
                               'subject_ref': history[0][1]['record_id'], 'predicate': predicate,
                               'object': target['record_id'], 'evidence_refs': [real_refs[index]],
                               'maker': {'maker_type': 'software', 'agent_ref': 'software:test-fixture'},
                               'provenance_event_ref': event_id, 'epistemic_status': 'uncertain',
                               'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                               'qualifiers': {'participation_role': 'test-participant', 'negated': True,
                                              'scope': 'synthetic-only', 'x-unknown': False}})
            claim_path = root / 'ToS/source-witnesses/history/fixture/historical-claims.jsonl'

            def rebuild():
                claim_path.write_text(''.join(json.dumps(claim, ensure_ascii=False) + '\n' for claim in claims))
                write_outputs(root, render_outputs(root))
                return build_payload(root)

            yield root, history, real, claims, rebuild

    def historical_knowledge(self, root, projection):
        access_src = REPO_ROOT / 'access/src'
        if str(access_src) not in sys.path:
            sys.path.insert(0, str(access_src))
        from tos_access.knowledge import build_knowledge_graph
        entities, relations = [json.loads((root / 'ToS/doctrine/semantic-interchange' / name).read_text())
                               for name in ('entity-types.v1.json', 'relation-types.v1.json')]
        return build_knowledge_graph({}, {}, projection, entities, relations), entities, relations

    def test_native_metadata_forms_survive_both_carriers_and_stale_source_is_not_hidden(self):
        """Source -> navigation -> shared reader must not drop native forms."""
        import tos_corpus_index_common as corpus_builder
        from build_source_witness_catalog import collect_records
        sys.path.insert(0, str(REPO_ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
        from source_commands import prepare_metadata_change, _apply
        from knowledge_assessment import Record
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            organization = {'schema_version': 'tos_corpus_record_v1', 'record_type': 'organization',
                'record_id': 'tos.organization.synthetic-form-test', 'record_version': 1,
                'preferred_label': 'Тестовая организация', 'notes': 'Только синтетический пример.',
                'identity_status': 'provisional', 'same_as_posture': 'no_equivalence_claim',
                'source_refs': ['test:synthetic-native-form'], 'external_identifiers': []}
            org_path = root / 'ToS/source-witnesses/organizations/synthetic-form-test/organization.json'
            org_path.parent.mkdir(parents=True)
            org_path.write_text(json.dumps(organization))
            sources = []
            for kind in ('agent', 'place', 'organization', 'work'):
                entry = collect_records(root)[kind][0]
                path = root / entry['source_record_ref']
                source = json.loads(path.read_text())
                source.update(preferred_label='Тестовая запись ' + kind,
                    notes='Синтетическая проверка формы, не новое историческое описание.',
                    field_languages={'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                        'notes': {'language': 'ru', 'script': 'Cyrl', 'unknown_qualification': [None, False]}},
                    record_version=source['record_version'] + 1)
                path.write_text(json.dumps(source, ensure_ascii=False))
                changes = [prepare_metadata_change(source, None, 'test:native-form',
                    form_id='tos.form.synthetic-native-' + kind + '-' + role, field_id=field)
                    for role, field in (('name', 'metadata.preferred-name'), ('hover', 'metadata.source-note'))]
                forms = _apply(None, Record.from_payload(source['record_id'], source['record_version'], source), changes)
                path.with_name(path.stem + '.human-forms.json').write_text(json.dumps(forms))
                sources.append((path, source))
            projection = rebuild()
            _, entities, relations = self.historical_knowledge(root, projection)
            from tos_access.knowledge import build_knowledge_graph, select_human_forms, focus_knowledge_node

            def navigation():
                with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                    return corpus_builder.build_source_navigation([])

            combined = build_knowledge_graph({'source_navigation': navigation()}, {}, projection, entities, relations)
            for path, source in sources:
                carriers = [n for n in combined['nodes'] if n['entity_id'] == source['record_id']]
                self.assertEqual({n['source_graph'] for n in carriers}, {'source-navigation', 'source-claims'})
                packets = []
                for carrier in carriers:
                    self.assertEqual(carrier['attributes']['source_record'], source)
                    selection = select_human_forms(carrier, 'ru')
                    self.assertEqual(selection['roles']['name']['packet']['display_text'], source['preferred_label'])
                    packet = selection['roles']['hover']['packet']
                    self.assertEqual(packet['display_text'], source['notes'])
                    self.assertIsNone(packet['admission'])
                    self.assertTrue(any(c['value'] == source['field_languages']['notes'] for c in packet['context']))
                    packets.append(packet)
                self.assertEqual(*packets)
                focus = focus_knowledge_node(combined, source['record_id'], depth=1)
                center = next(n for n in focus['nodes'] if n['id'] == focus['focus']['node_id'])
                self.assertEqual(select_human_forms(center, 'ru')['roles']['hover']['packet'], packets[0])

            path, source = sources[0]
            newer = {**source, 'record_version': source['record_version'] + 1, 'notes': 'Исправленная тестовая запись.'}
            path.write_text(json.dumps(newer, ensure_ascii=False))
            # A stale catalog cannot supply a digest for forms over new source bytes.
            with self.assertRaisesRegex(ValueError, 'digest'):
                navigation()
            projection = rebuild()
            combined = build_knowledge_graph({'source_navigation': navigation()}, {}, projection, entities, relations)
            for carrier in (n for n in combined['nodes'] if n['entity_id'] == source['record_id']):
                self.assertEqual(carrier['attributes']['source_record'], newer)
                self.assertEqual({f['state'] for f in carrier['attributes']['human_forms']}, {'stale'})
                self.assertIsNone(select_human_forms(carrier, 'ru')['roles']['hover']['packet'])

    def test_documentary_claims_keep_roles_carrier_and_historical_context_distinct(self):
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        profiles = SourceClaimProfiles(REPO_ROOT)
        objects = {f'tos.{kind}.synthetic': {'record_type': kind} for kind in
                   ('letter', 'document', 'agent', 'organization', 'artifact', 'work', 'historical-event')}
        cases = [('correspondence_sender', 'letter', 'agent'),
                 ('correspondence_addressee', 'letter', 'organization'),
                 ('document_carried_by', 'letter', 'artifact'),
                 ('document_carried_by', 'document', 'artifact'),
                 ('historical_document', 'historical-event', 'letter'),
                 ('document_concerns_work', 'letter', 'work'),
                 ('authored_by', 'document', 'agent'), ('authored_by', 'work', 'agent')]
        for predicate, subject, target in cases:
            with self.subTest(predicate=predicate, subject=subject):
                claim = {'schema_version': 'tos_source_relation_claim_v1', 'claim_type': 'relation',
                    'claim_id': 'tos.claim.synthetic-document-role', 'claim_version': 1,
                    'subject_ref': f'tos.{subject}.synthetic', 'predicate': predicate,
                    'object': f'tos.{target}.synthetic', 'assertion_layer': 'scholarly_report',
                    'evidence_refs': ['test:synthetic-no-historical-proof'],
                    'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic'},
                    'provenance_event_ref': 'tos.event.synthetic', 'epistemic_status': 'reported',
                    'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                    'extensions': {'uninterpreted': [None, False]}}
                profiles.validate(claim, objects)
                for modified in ({'subject_ref': 'tos.agent.synthetic'}, {'object': 'tos.historical-event.synthetic'},
                                 {'review_status': 'accepted'}, {'object': {'label': 'not-an-identity'}}):
                    with self.assertRaises(SourceProfileError):
                        profiles.validate({**claim, **modified}, objects)
        with self.assertRaises(SourceProfileError):
            profiles.validate({**claim, 'predicate': 'correspondence_sender', 'subject_ref': 'tos.document.synthetic'}, objects)

    def test_concept_and_conception_profiles_preserve_scope_and_claim_boundaries(self):
        """Synthetic accounts test the grammar, not Nietzsche's philosophy."""
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        profiles = SourceRecordProfiles(REPO_ROOT)
        kinds = ('crosscutting-concept', 'conception')
        for kind in kinds:
            self.assertEqual(profiles.profiles[kind]['reader'], 'semantic-metadata-v1')
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            for name in ('source-metadata-record', 'semantic-description-record', 'semantic-relation-claim',
                         'source-claim-record', 'semantic-relation-type-registry'):
                ref = 'ToS/contracts/' + name + '.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            from source_commands import prepare_metadata_change, _apply
            from knowledge_assessment import Record
            records = []
            for kind, suffix in (('crosscutting-concept', 'question'), ('conception', 'first'), ('conception', 'second')):
                source = {**copy.deepcopy(history[0][1]), 'schema_version': 'tos_semantic_description_record_v1',
                    'record_type': kind, 'record_id': f'tos.{kind}.synthetic-{suffix}',
                    'preferred_label': 'Условная трактовка' if kind == 'conception' else 'Условный сквозной концепт',
                    'notes': 'Синтетическое описание; сходство имён не доказывает общность содержания.',
                    'field_languages': {'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                        'notes': {'language': 'ru', 'script': 'Cyrl'}},
                    'semantic_scope': {'scope_note': 'Только искусственный пример проверки.',
                        'identity_criterion': 'Точный предмет теста; изменение формулировки не создаёт другого предмета.',
                        'language': 'ru', 'script': 'Cyrl'},
                    'extensions': {'uninterpreted': [False, None, 'Ω']}}
                profiles.validate(kind, source)
                for change in ({'semantic_scope': {}}, {'notes': ' '}, {'record_type': 'concept'},
                               {'record_id': 'tos.concept.becoming'}, {'conception_of': 'tos.concept.becoming'}):
                    with self.subTest(kind=kind, change=change), self.assertRaises(SourceProfileError):
                        profiles.validate(kind, {**source, **change})
                path = root / f'ToS/source-witnesses/semantic-descriptions/synthetic-{suffix}/{kind}.json'
                path.parent.mkdir(parents=True)
                path.write_text(json.dumps(source, ensure_ascii=False))
                changes = [prepare_metadata_change(source, None, 'test:semantic-profile',
                    form_id=f'tos.form.synthetic-{suffix}-{role}', field_id=field) for role, field in
                    (('name', 'metadata.preferred-name'), ('hover', 'metadata.source-note'))]
                formset = _apply(None, Record.from_payload(source['record_id'], 1, source), changes)
                path.with_name(kind + '.human-forms.json').write_text(json.dumps(formset))
                records.append(source)
            associations = []
            for index, (predicate, subject, target) in enumerate((
                ('conception_of', records[1], records[0]), ('conception_of', records[2], records[0]),
                ('conception_attributed_to', records[1], real[0]),
                ('conception_expressed_in', records[1], real[2]),
                ('conception_redefines', records[2], records[1]),
            )):
                claim = {**copy.deepcopy(claims[0]), 'schema_version': 'tos_semantic_relation_claim_v1',
                    'claim_id': f'tos.claim.synthetic-conception-{index}', 'predicate': predicate,
                    'subject_ref': subject['record_id'], 'object': target['record_id'],
                    'assertion_layer': 'semantic_interpretation',
                    'qualifiers': {'statement': 'Синтетическая спорная связь; не исторический факт.',
                        'statement_language': 'ru', 'statement_script': 'Cyrl', 'negated': index == 1,
                        'relation_basis': 'Только искусственное основание для проверки контракта.'}}
                associations.append(claim)
            path.with_name('source-claims.jsonl').write_text(''.join(json.dumps(c) + '\n' for c in associations))
            objects = {r['record_id']: r for r in [*records, *real]}
            reader = SourceClaimProfiles(root)
            for claim in associations:
                reader.validate(claim, objects)
                for change in ({'subject_ref': real[0]['record_id']}, {'object': real[1]['record_id']},
                               {'qualifiers': {'statement': 'No basis'}}, {'evidence_refs': []},
                               {'review_status': 'accepted'}, {'claim_id': claim['subject_ref']}):
                    with self.subTest(predicate=claim['predicate'], change=change), self.assertRaises(SourceProfileError):
                        reader.validate({**claim, **change}, objects)
            graph, entities, relations = self.historical_knowledge(root, rebuild())
            from tos_access.knowledge import focus_knowledge_node, select_human_forms
            import tos_corpus_index_common as corpus_builder
            from tos_access.knowledge import build_knowledge_graph
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                navigation = corpus_builder.build_source_navigation([])
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, rebuild(), entities, relations)
            for source in records:
                self.assertEqual(sum(n['entity_id'] == source['record_id'] for n in graph['nodes']), 2)
                node = next(n for n in graph['nodes'] if n['entity_id'] == source['record_id'])
                self.assertEqual(node['attributes']['source_record'], source)
                self.assertIn('tos.entity.semantic-object', node['semantics']['type_ancestors'])
                self.assertNotIn('tos.entity.identity', node['semantics']['type_ancestors'])
                packet = select_human_forms(node, 'ru')['roles']['hover']['packet']
                self.assertEqual(packet['display_text'], source['notes'])
                self.assertTrue(any(c['binding']['pointer'] == '/semantic_scope' and c['value'] == source['semantic_scope']
                                    for c in packet['context']))
                self.assertIsNone(packet['admission'])
            for claim in associations:
                node = next(n for n in graph['nodes'] if n['entity_id'] == claim['claim_id'])
                self.assertEqual(node['attributes']['source_claim'], claim)
                for center, other in ((claim['subject_ref'], claim['object']), (claim['object'], claim['subject_ref'])):
                    focused = focus_knowledge_node(graph, center, depth=2)
                    self.assertIn(other, {n['entity_id'] for n in focused['nodes']})
            self.assertEqual(next(e for e in entities['types'] if e['type_id'] == 'tos.entity.concept')['source_mappings'],
                [{'source_graph': 'canon', 'source_kind_id': 'concept'},
                 {'source_graph': 'philosophy', 'source_kind_id': 'concept'}])

    def test_semantic_profile_modes_cannot_retype_existing_identity_or_bypass_endpoint_scope(self):
        from source_record_profiles import SourceRecordProfiles, SourceClaimProfiles, SourceProfileError
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            entity_ref = 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
            relation_ref = 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            contract = 'ToS/contracts/semantic-relation-type-registry.schema.json'
            (root / contract).write_bytes((REPO_ROOT / contract).read_bytes())
            entities = json.loads((root / entity_ref).read_bytes())
            relations = json.loads((root / relation_ref).read_bytes())
            for kind, change in (
                ('conception', lambda e: e['source_record_profile'].update(reader='corpus-metadata-v1')),
                ('conception', lambda e: e.update(parent_type_ids=['tos.entity.identity'])),
                ('conception', lambda e: e.update(object_role='identity')),
                ('conception', lambda e: e.update(abstract=True)),
                ('historical-event', lambda e: e['source_record_profile'].update(reader='semantic-metadata-v1')),
            ):
                invalid = copy.deepcopy(entities)
                change(next(e for e in invalid['types'] if e['type_id'] == 'tos.entity.' + kind))
                (root / entity_ref).write_text(json.dumps(invalid))
                with self.subTest(kind=kind, change=change), self.assertRaises(SourceProfileError):
                    SourceRecordProfiles(root)
            (root / entity_ref).write_text(json.dumps(entities))
            for change in (
                lambda e: e['source_claim_profile'].update(reader='identity-relation-v1'),
                lambda e: e.update(domain_type_ids=['tos.entity.thing']),
                lambda e: e.update(range_type_ids=['tos.entity.semantic-object']),
                lambda e: e.update(domain_type_ids=['tos.entity.claim']),
                lambda e: e.update(domain_type_ids=['tos.entity.agent'], range_type_ids=['tos.entity.work']),
            ):
                invalid = copy.deepcopy(relations)
                change(next(e for e in invalid['relations'] if e['relation_type_id'] == 'tos.relation.conception-of'))
                (root / relation_ref).write_text(json.dumps(invalid))
                with self.subTest(change=change), self.assertRaises(SourceProfileError):
                    SourceClaimProfiles(root)
            from tos_access.knowledge import validate_semantic_registries
            changed = copy.deepcopy(entities)
            changed['registry_version'] += 1
            profile = next(e for e in changed['types'] if e['type_id'] == 'tos.entity.conception')['source_record_profile']
            profile.update(profile_version=2, reader='corpus-metadata-v1')
            self.assertFalse(validate_semantic_registries(changed, relations, previous_entity_registry=entities)['valid'])

    def test_conception_transformations_are_distinct_nontransitive_grounded_predicates(self):
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        profiles = SourceClaimProfiles(REPO_ROOT)
        objects = {'tos.conception.subject': {'record_type': 'conception'},
                   'tos.conception.object': {'record_type': 'conception'}}
        transformations = ('redefines', 'rejects', 'narrows', 'expands', 'secularizes',
                           'psychologizes', 'politicizes', 'inverts')
        for transformation in transformations:
            predicate = 'conception_' + transformation
            relation = profiles.relations[predicate]
            self.assertFalse(relation['transitive'])
            self.assertEqual(relation['domain_type_ids'], ['tos.entity.conception'])
            self.assertEqual(relation['range_type_ids'], ['tos.entity.conception'])
            self.assertIsNone(relation['cardinality']['per_subject_max'])
            claim = {'schema_version': 'tos_semantic_relation_claim_v1', 'claim_type': 'relation',
                'claim_id': 'tos.claim.synthetic-' + transformation, 'claim_version': 1,
                'subject_ref': 'tos.conception.subject', 'predicate': predicate, 'object': 'tos.conception.object',
                'assertion_layer': 'semantic_interpretation', 'evidence_refs': ['test:synthetic'],
                'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic'},
                'provenance_event_ref': 'tos.event.synthetic', 'epistemic_status': 'uncertain',
                'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
                'qualifiers': {'statement': 'Synthetic comparison only.', 'statement_language': 'en',
                              'statement_script': 'Latn', 'relation_basis': 'Explicit synthetic comparison dimension.'}}
            profiles.validate(claim, objects)
            for qualifiers in ({**claim['qualifiers'], 'relation_basis': ' '},
                               {**claim['qualifiers'], 'statement_language': 'not a language tag'}):
                with self.subTest(transformation=transformation), self.assertRaises(SourceProfileError):
                    profiles.validate({**claim, 'qualifiers': qualifiers}, objects)

    def test_declared_claim_profile_reads_new_predicate_without_python_branch(self):
        """Synthetic predicate/claim grammar, not evidence for any real event."""
        from build_source_witness_catalog import collect_claims, CatalogBuildError
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            registry_ref = 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            registry = json.loads((root / registry_ref).read_bytes())
            entry = copy.deepcopy(next(row for row in registry['relations']
                                       if row['relation_type_id'] == 'tos.relation.historical-participant'))
            entry.update(relation_type_id='tos.relation.fixture-person-reference',
                source_mappings=[{'source_graph': 'source-claims', 'scope': 'claim-predicate',
                                  'source_predicate_id': 'fixture_person_reference'}],
                source_claim_profile={'profile_version': 1, 'reader': 'identity-relation-v1',
                    'assertion_layers': ['scholarly_report'],
                    'schemas': [{'schema_version': 'tos_fixture_identity_relation_v1',
                        'schema_ref': 'ToS/contracts/fixture-identity-relation.schema.json',
                        'schema_dependencies': ['ToS/contracts/claim-packet.schema.json',
                                                'ToS/contracts/knowledge-assessment.schema.json']}]})
            registry['relations'].append(entry)
            (root / registry_ref).write_text(json.dumps(registry))
            contract = 'ToS/contracts/semantic-relation-type-registry.schema.json'
            (root / contract).write_bytes((REPO_ROOT / contract).read_bytes())
            contract = 'ToS/contracts/source-claim-record.schema.json'
            (root / contract).write_bytes((REPO_ROOT / contract).read_bytes())
            schema = json.loads((root / 'ToS/contracts/historical-claim.schema.json').read_bytes())
            schema['$id'] = 'https://tree-of-sophia.local/ToS/contracts/fixture-identity-relation.schema.json'
            schema.pop('allOf')
            schema['properties'].update(schema_version={'const': 'tos_fixture_identity_relation_v1'},
                predicate={'const': 'fixture_person_reference'}, subject_ref={'type': 'string'}, object={'type': 'string'})
            (root / 'ToS/contracts/fixture-identity-relation.schema.json').write_text(json.dumps(schema))
            claim = {**copy.deepcopy(claims[0]), 'schema_version': 'tos_fixture_identity_relation_v1',
                     'claim_id': 'tos.claim.fixture-person-reference', 'predicate': 'fixture_person_reference',
                     'extensions': {'uninterpreted': [False, None, 'Ω']}}
            path = root / 'ToS/source-witnesses/history/fixture/source-claims.jsonl'
            path.write_text(json.dumps(claim) + '\n')
            self.assertIn(claim['claim_id'], {row['claim_id'] for row in collect_claims(root)})
            graph, entity_registry, relation_registry = self.historical_knowledge(root, rebuild())
            from tos_access.knowledge import knowledge_catalog
            catalog = knowledge_catalog(graph, {}, {}, entity_registry, relation_registry)
            discovered = next(row for row in catalog['semantic_registries']['relation_types']['entries']
                              if row['relation_type_id'] == entry['relation_type_id'])
            self.assertEqual(discovered['source_claim_profile'], entry['source_claim_profile'])
            claim_node = next(node for node in graph['nodes'] if node['entity_id'] == claim['claim_id'])
            meaning = claim_node['semantics']['claim']
            self.assertEqual(meaning['relation_type_id'], entry['relation_type_id'])
            self.assertEqual(meaning['subject_entity_id'], claim['subject_ref'])
            self.assertEqual(meaning['object_entity_id'], claim['object'])
            self.assertEqual(claim_node['attributes']['source_claim'], claim)
            from tos_access.knowledge import focus_knowledge_node
            for center, other in ((claim['subject_ref'], claim['object']), (claim['object'], claim['subject_ref'])):
                focused = focus_knowledge_node(graph, center, depth=2)
                self.assertIn(other, {node['entity_id'] for node in focused['nodes']})
            projection = rebuild()
            self.assertIn('source-profile', projection['graph_layers'])
            self.assertIn('ToS/contracts/fixture-identity-relation.schema.json', projection['input_digests'])
            self.assertEqual(next(row for row in collect_claims(root) if row['claim_id'] == claim['claim_id'])
                             ['source_schema_ref'], entry['source_claim_profile']['schemas'][0]['schema_ref'])
            for modified in ({'object': real[2]['record_id']}, {'subject_ref': real[0]['record_id']},
                             {'object': 'tos.agent.unresolved'}, {'object': {'value': 'not-an-identity'}},
                             {'predicate': 'undeclared'}, {'schema_version': 'tos_future_claim_v99'},
                             {'visibility': 'local_only'}, {'assertion_layer': 'canon_judgment'}):
                path.write_text(json.dumps({**claim, **modified}) + '\n')
                with self.subTest(modified=modified), self.assertRaises((CatalogBuildError, BibliographicGraphBuildError, ValueError)):
                    rebuild()
            path.write_text(json.dumps(claim) + '\n')
            self.historical_knowledge(root, rebuild())
            from source_record_profiles import SourceClaimProfiles, SourceProfileError
            for mutate in (
                lambda row: row.update(abstract=True),
                lambda row: row.update(evidence_required=False),
                lambda row: row.update(domain_type_ids=['tos.entity.thing']),
                lambda row: row.update(range_type_ids=['tos.entity.temporal-assertion']),
                lambda row: row['source_claim_profile'].update(reader='execute-source'),
                lambda row: row['source_claim_profile'].update(command='untrusted source prose'),
                lambda row: row['source_claim_profile']['schemas'][0].update(schema_ref='https://example.org/schema.json'),
                lambda row: row['source_claim_profile']['schemas'].append(copy.deepcopy(row['source_claim_profile']['schemas'][0])),
            ):
                invalid = copy.deepcopy(registry)
                mutate(invalid['relations'][-1])
                (root / registry_ref).write_text(json.dumps(invalid))
                with self.assertRaises(SourceProfileError):
                    SourceClaimProfiles(root)
            (root / registry_ref).write_text(json.dumps(registry))
            profile_reader = SourceClaimProfiles(root)
            for modified in ({'predicate': []}, {'schema_version': {}}, {'evidence_refs': []},
                             {'review_status': 'accepted'}):
                with self.assertRaises(SourceProfileError):
                    profile_reader.validate({**claim, **modified})
            path.write_text(json.dumps(claim)[:-1] + ', "predicate": "fixture_person_reference"}\n')
            with self.assertRaises(CatalogBuildError):
                collect_claims(root)
            path.write_text(json.dumps(claim) + '\n')

            from tos_access.knowledge import validate_semantic_registries
            entities = json.loads((root / 'ToS/doctrine/semantic-interchange/entity-types.v1.json').read_bytes())
            changed = copy.deepcopy(registry)
            changed['registry_version'] += 1
            changed['relations'][-1]['source_claim_profile']['assertion_layers'].append('textual_observation')
            self.assertFalse(validate_semantic_registries(entities, changed, previous_relation_registry=registry)['valid'])
            changed['relations'][-1]['source_claim_profile']['profile_version'] += 1
            self.assertTrue(validate_semantic_registries(entities, changed, previous_relation_registry=registry)['valid'])
            changed['relations'][-1]['source_mappings'][0]['source_predicate_id'] = 'silently_repurposed'
            self.assertFalse(validate_semantic_registries(entities, changed, previous_relation_registry=registry)['valid'])

    def test_document_profile_and_claims_reach_shared_reader_together(self):
        """Actual document/letter contracts on synthetic data, not a real letter."""
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            for name in ('semantic-relation-type-registry', 'source-metadata-record',
                         'document-record', 'source-claim-record', 'source-relation-claim'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            records = []
            for kind in ('document', 'letter'):
                record = {**copy.deepcopy(history[0][1]), 'record_type': kind,
                          'schema_version': 'tos_document_record_v1', 'record_id': f'tos.{kind}.synthetic',
                          'preferred_label': f'Synthetic {kind}',
                          'extensions': {'uninterpreted': [False, None, 'Ω']}}
                ref = f'ToS/source-witnesses/documents/synthetic/{kind}.json'
                (root / ref).parent.mkdir(parents=True, exist_ok=True)
                (root / ref).write_text(json.dumps(record))
                records.append(record)
            associations = []
            for index, (subject, predicate, target) in enumerate((
                (records[1]['record_id'], 'correspondence_sender', real[0]['record_id']),
                (history[0][1]['record_id'], 'historical_document', records[1]['record_id']),
                (records[0]['record_id'], 'document_concerns_work', real[2]['record_id']),
            )):
                associations.append({**copy.deepcopy(claims[0]),
                    'schema_version': 'tos_source_relation_claim_v1',
                    'claim_id': f'tos.claim.synthetic-document-{index}',
                    'subject_ref': subject, 'predicate': predicate, 'object': target,
                    'counterevidence_refs': [claims[2]['evidence_refs'][0]],
                    'extensions': {'uninterpreted': [False, None, 'Ω']}})
            (root / 'ToS/source-witnesses/documents/synthetic/source-claims.jsonl').write_text(
                ''.join(json.dumps(claim) + '\n' for claim in associations))
            graph, _, _ = self.historical_knowledge(root, rebuild())
            from tos_access.knowledge import focus_knowledge_node
            for record in records:
                node = next(node for node in graph['nodes'] if node['entity_id'] == record['record_id'])
                self.assertEqual(node['attributes']['source_record'], record)
                self.assertEqual(node['type_id'], 'tos.entity.' + record['record_type'])
            for claim in associations:
                node = next(node for node in graph['nodes'] if node['entity_id'] == claim['claim_id'])
                self.assertEqual(node['attributes']['source_claim'], claim)
                self.assertNotEqual(node['entity_id'], claim['object'])
                for center, other in ((claim['subject_ref'], claim['object']),
                                      (claim['object'], claim['subject_ref'])):
                    focused = focus_knowledge_node(graph, center, depth=2)
                    self.assertIn(other, {node['entity_id'] for node in focused['nodes']})

    def test_declared_metadata_profile_extends_both_readers_without_python_type_branch(self):
        """A synthetic profile is a grammar test, not a historical letter."""
        from build_source_witness_catalog import collect_records, CatalogBuildError
        from source_commands import prepare_metadata_change, _apply
        from knowledge_assessment import Record
        import tos_corpus_index_common as corpus_builder
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            registry_ref = 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
            registry = json.loads((root / registry_ref).read_bytes())
            entry = copy.deepcopy(next(item for item in registry['types']
                                       if item['type_id'] == 'tos.entity.historical-event'))
            kind = 'fixture-document'
            entry.update(type_id='tos.entity.' + kind, parent_type_ids=['tos.entity.identity'],
                         definition='Synthetic independent document identity; not a physical carrier.',
                         source_mappings=[{'source_graph': graph, 'source_kind_id': kind}
                                          for graph in ('source-claims', 'source-navigation')],
                         source_record_profile={
                             'profile_version': 1, 'reader': 'corpus-metadata-v1',
                             'record_type': kind, 'id_prefix': 'tos.fixture-document.',
                             'source_basename': 'fixture-document.json',
                             'catalog_filename': 'fixture-documents.jsonl',
                             'schemas': [{'schema_version': 'tos_fixture_document_v1',
                             'schema_ref': 'ToS/contracts/fixture-document.schema.json',
                             'schema_dependencies': ['ToS/contracts/corpus-record.schema.json']}],
                             'graph_layer': 'source-profile'})
            registry['types'].append(entry)
            (root / registry_ref).write_text(json.dumps(registry))
            schema = json.loads((root / 'ToS/contracts/historical-record.schema.json').read_bytes())
            schema.update(**{'$id': 'https://tree-of-sophia.local/ToS/contracts/fixture-document.schema.json'})
            schema.pop('allOf')
            schema['properties']['schema_version'] = {'const': 'tos_fixture_document_v1'}
            schema['properties']['record_type'] = {'const': kind}
            schema['properties']['record_id'] = {'type': 'string', 'pattern': '^tos\\.fixture-document\\.'}
            (root / entry['source_record_profile']['schemas'][0]['schema_ref']).write_text(json.dumps(schema))
            source = copy.deepcopy(history[0][1])
            source.update(schema_version='tos_fixture_document_v1', record_type=kind,
                          record_id='tos.fixture-document.synthetic', preferred_label='Условное письмо',
                          field_languages={'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                           'notes': {'language': 'ru', 'script': 'Cyrl'}},
                          extensions={'uninterpreted': [False, None, {'language': 'x-unknown', 'value': 'Ω'}]})
            path = root / 'ToS/source-witnesses/documents/fixture/fixture-document.json'
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(source, ensure_ascii=False))
            changes = [prepare_metadata_change(source, None, 'test:profile', form_id='tos.form.profile-' + role,
                                               field_id=field) for role, field in
                       (('name', 'metadata.preferred-name'), ('hover', 'metadata.source-note'))]
            formset = _apply(None, Record.from_payload(source['record_id'], 1, source), changes)
            path.with_name('fixture-document.human-forms.json').write_text(json.dumps(formset))
            self.assertEqual(collect_records(root)[kind][0]['record_id'], source['record_id'])
            projection = rebuild()
            self.assertIn('source-profile', projection['graph_layers'])
            graph_schema = json.loads((root / 'ToS/contracts/source-witness-bibliographic-graph.schema.json').read_bytes())
            from jsonschema import Draft202012Validator
            Draft202012Validator(graph_schema).validate(projection)
            catalog_schema = json.loads((root / 'ToS/contracts/source-witness-catalog.schema.json').read_bytes())
            Draft202012Validator(catalog_schema).validate(json.loads(
                (root / 'ToS/source-witnesses/catalog/catalog.manifest.json').read_bytes()))
            Draft202012Validator(catalog_schema['$defs']['entry']).validate(collect_records(root)[kind][0])
            self.historical_knowledge(root, projection)
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                navigation = corpus_builder.build_source_navigation([])
            from tos_access.knowledge import build_knowledge_graph
            relations = json.loads((root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_bytes())
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, projection, registry, relations)
            carriers = [node for node in graph['nodes'] if node['entity_id'] == source['record_id']]
            self.assertEqual(len(carriers), 2)
            for node in carriers:
                self.assertEqual(node['type_id'], entry['type_id'])
                self.assertEqual(node['attributes']['source_record'], source)
                self.assertEqual(node['source_record']['payload']['properties']['source_record'], source)
                self.assertEqual(node['display']['title']['default'], source['preferred_label'])
                forms = node['attributes']['human_forms']
                self.assertEqual({form['state'] for form in forms}, {'ready'})
                self.assertEqual({form['display_text'] for form in forms},
                                 {source['preferred_label'], source['notes']})
            from tos_access.knowledge import focus_knowledge_node, select_human_forms
            focused = focus_knowledge_node(graph, source['record_id'], depth=1)
            center = next(node for node in focused['nodes'] if node['id'] == focused['focus']['node_id'])
            self.assertEqual(center['entity_id'], source['record_id'])
            self.assertEqual(center['type_id'], entry['type_id'])
            readable = select_human_forms(center, 'ru')
            self.assertEqual(readable['roles']['hover']['packet']['display_text'], source['notes'])
            self.assertIsNone(readable['roles']['hover']['packet']['admission'])
            for field, value in [('visibility', 'local_only'), ('record_id', 'tos.work.false-identity'),
                                 ('schema_version', 'tos_fixture_document_v2')]:
                path.write_text(json.dumps({**source, field: value}))
                with self.subTest(field=field), self.assertRaises(CatalogBuildError):
                    collect_records(root)
            self.assertEqual(source['extensions']['uninterpreted'][0], False)

            # Add a compatible schema route without retyping the subject or
            # discarding the old schema. This test is not a source revision.
            from source_record_profiles import SourceRecordProfiles
            descriptor = entry['source_record_profile']
            newer_schema = copy.deepcopy(schema)
            newer_schema['$id'] = 'https://tree-of-sophia.local/ToS/contracts/fixture-document-v2.schema.json'
            newer_schema['properties']['schema_version'] = {'const': 'tos_fixture_document_v2'}
            newer_schema['properties']['new_source_field'] = {'type': 'boolean'}
            newer_schema['required'].append('new_source_field')
            (root / 'ToS/contracts/fixture-document-v2.schema.json').write_text(json.dumps(newer_schema))
            descriptor['profile_version'] = 2
            descriptor['schemas'].append({'schema_version': 'tos_fixture_document_v2',
                                         'schema_ref': 'ToS/contracts/fixture-document-v2.schema.json',
                                         'schema_dependencies': ['ToS/contracts/corpus-record.schema.json']})
            (root / registry_ref).write_text(json.dumps(registry))
            reader = SourceRecordProfiles(root)
            reader.validate(kind, source)
            newer_source = {**source, 'schema_version': 'tos_fixture_document_v2',
                            'record_version': 2, 'new_source_field': False}
            reader.validate(kind, newer_source)
            path.write_text(json.dumps(newer_source))
            self.assertEqual(reader.verify_entry(kind, collect_records(root)[kind][0]), newer_source)
            self.assertEqual(newer_source['record_id'], source['record_id'])
            revised_graph, _, _ = self.historical_knowledge(root, rebuild())
            revised_carrier = next(node for node in revised_graph['nodes'] if node['entity_id'] == source['record_id'])
            self.assertEqual(revised_carrier['attributes']['source_record'], newer_source)
            self.assertEqual({form['state'] for form in revised_carrier['attributes']['human_forms']}, {'stale'})

            from tos_access.knowledge import validate_semantic_registries, knowledge_catalog
            catalog = knowledge_catalog(graph, {}, {}, registry, relations)
            discovered = next(item for item in catalog['semantic_registries']['entity_types']['entries']
                              if item['type_id'] == entry['type_id'])
            self.assertEqual(discovered['source_record_profile'], descriptor)
            previous = copy.deepcopy(registry)
            previous['registry_version'] -= 1
            previous_type = previous['types'][-1]
            previous_type['source_record_profile']['profile_version'] = 1
            previous_type['source_record_profile']['schemas'].pop()
            self.assertTrue(validate_semantic_registries(registry, relations,
                            previous_entity_registry=previous)['valid'])
            for mutate in (lambda p: p.update(profile_version=1), lambda p: p['schemas'].pop(0),
                           lambda p: p.update(id_prefix='tos.reassigned.')):
                invalid = copy.deepcopy(registry)
                mutate(invalid['types'][-1]['source_record_profile'])
                self.assertFalse(validate_semantic_registries(invalid, relations,
                                 previous_entity_registry=previous)['valid'])

    def test_source_profile_contract_rejects_collisions_and_invented_authority(self):
        from source_record_profiles import SourceRecordProfiles, SourceProfileError
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            ref = 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
            original = json.loads((root / ref).read_bytes())
            for mutate in (
                lambda entry: entry['source_record_profile'].update(reader='run-shell'),
                lambda entry: entry['source_record_profile'].update(command='execute source prose'),
                lambda entry: entry['source_record_profile'].update(id_prefix='tos.work.'),
                lambda entry: entry['source_record_profile'].update(source_basename='../historical-event.json'),
                lambda entry: entry['source_record_profile'].update(catalog_filename='agents.jsonl'),
                lambda entry: entry['source_record_profile']['schemas'][0].update(schema_ref='/etc/passwd'),
                lambda entry: entry['source_record_profile']['schemas'][0].update(schema_dependencies=['https://example.org/schema.json']),
                lambda entry: entry['source_record_profile']['schemas'].append(copy.deepcopy(entry['source_record_profile']['schemas'][0])),
                lambda entry: entry.update(abstract=True),
                lambda entry: entry.update(source_mappings=entry['source_mappings'][:1]),
            ):
                registry = copy.deepcopy(original)
                entry = next(item for item in registry['types'] if item['type_id'] == 'tos.entity.historical-event')
                mutate(entry)
                (root / ref).write_text(json.dumps(registry))
                with self.subTest(mutation=repr(mutate)), self.assertRaises(SourceProfileError):
                    SourceRecordProfiles(root)
            (root / ref).write_text(json.dumps(original))
            native_collision = copy.deepcopy(original)
            composite = next(item for item in native_collision['types'] if item['type_id'] == 'tos.entity.composite')
            profile = copy.deepcopy(next(item['source_record_profile'] for item in native_collision['types']
                                         if item['type_id'] == 'tos.entity.historical-event'))
            profile.update(record_type='composite', id_prefix='tos.composite.',
                           source_basename='composite.json', catalog_filename='composites.jsonl')
            composite['source_record_profile'] = profile
            (root / ref).write_text(json.dumps(native_collision))
            with self.assertRaisesRegex(SourceProfileError, 'adapter collision'):
                SourceRecordProfiles(root)
            (root / ref).write_text(json.dumps(original))
            reader = SourceRecordProfiles(root)
            for field, value in (('retained_native_adapter', None), ('reader', 'semantic-metadata-v1'),
                                 ('catalog_filename', 'other-composites.jsonl'), ('id_prefix', 'tos.work.')):
                registry = copy.deepcopy(original)
                composite = next(item for item in registry['types'] if item['type_id'] == 'tos.entity.composite')
                if value is None:
                    del composite['source_record_profile'][field]
                else:
                    composite['source_record_profile'][field] = value
                (root / ref).write_text(json.dumps(registry))
                with self.subTest(composite_field=field), self.assertRaises(SourceProfileError):
                    SourceRecordProfiles(root)
            (root / ref).write_text(json.dumps(original))
            path, source = history[0]
            relative = path.relative_to(root).as_posix()
            entry = reader.catalog_entry(source['record_type'], source, relative)
            for field, value in [('preferred_label', 'Invented certainty'), ('links', {'work_ref': real[2]['record_id']}),
                                 ('source_schema_ref', 'ToS/contracts/corpus-record.schema.json'),
                                 ('record_sha256', '0' * 64), ('source_record_ref', '../outside.json')]:
                with self.subTest(field=field), self.assertRaises(SourceProfileError):
                    reader.verify_entry(source['record_type'], {**entry, field: value})
            saved = path.read_bytes()
            path.write_bytes(b'{"record_id":"first","record_id":"second"}')
            with self.assertRaises(SourceProfileError):
                reader.load(source['record_type'], relative)
            path.write_bytes(b'{"number":1e309}')
            with self.assertRaises(SourceProfileError):
                reader.load(source['record_type'], relative)
            path.write_bytes(b' ' * 1_048_577)
            with self.assertRaises(SourceProfileError):
                reader.load(source['record_type'], relative)
            path.unlink()
            path.symlink_to(history[1][0])
            with self.assertRaises(SourceProfileError):
                reader.load(source['record_type'], relative)
            path.unlink()
            path.write_bytes(saved)
            self.assertEqual(reader.load(source['record_type'], relative), source)

    def test_composite_metadata_growth_coexists_with_unchanged_native_witnesses(self):
        """Synthetic reconstruction metadata is not an ancient source claim."""
        from build_source_witness_catalog import collect_records, CatalogBuildError
        from source_record_profiles import SourceRecordProfiles, SourceProfileError
        import tos_corpus_index_common as corpus_builder
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            profiles = SourceRecordProfiles(root)
            self.assertEqual(profiles.profiles['composite']['retained_native_adapter'], 'scholarly-composite-v1')
            if str(REPO_ROOT / 'access/src') not in sys.path:
                sys.path.insert(0, str(REPO_ROOT / 'access/src'))
            from tos_access.knowledge import validate_semantic_registries
            entities = profiles.registry
            relations = json.loads((root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_bytes())
            previous = copy.deepcopy(entities)
            old_type = next(entry for entry in previous['types'] if entry['type_id'] == 'tos.entity.composite')
            del old_type['source_record_profile']
            previous['registry_version'] -= 1
            self.assertTrue(validate_semantic_registries(entities, relations, previous_entity_registry=previous)['valid'])
            removed = copy.deepcopy(entities)
            changed = next(entry for entry in removed['types'] if entry['type_id'] == 'tos.entity.composite')
            del changed['source_record_profile']['retained_native_adapter']
            changed['source_record_profile']['profile_version'] += 1
            removed['registry_version'] += 1
            report = validate_semantic_registries(removed, relations, previous_entity_registry=entities)
            self.assertFalse(report['valid'])
            self.assertTrue(any('retained_native_adapter' in violation for violation in report['violations']))
            for name in ('scholarly-composite-record', 'scholarly-composite-witness', 'textual-passage-record',
                         'scholarly-composite-claim', 'source-claim-record', 'semantic-relation-type-registry',
                         'source-metadata-record', 'semantic-description-record'):
                ref = f'ToS/contracts/{name}.schema.json'
                (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            native_ref = 'ToS/source-witnesses/scholarly-composites/synoptic/akkadian/old-babylonian-gilgamesh-fragments/composite-witness.json'
            native_path = root / native_ref
            native_path.parent.mkdir(parents=True)
            native_raw = (REPO_ROOT / native_ref).read_bytes()
            native_path.write_bytes(native_raw)
            native = json.loads(native_raw)
            record = copy.deepcopy(history[0][1])
            record.update(schema_version='tos_scholarly_composite_record_v1', record_type='composite',
                          record_id='tos.composite.synthetic-arrangement',
                          field_languages={'preferred_label': {'language': 'ru', 'script': 'Cyrl'},
                                           'notes': {'language': 'ru', 'script': 'Cyrl'}},
                          semantic_scope={'language': 'en', 'script': 'Latn',
                              'scope_note': 'Synthetic arrangement, not a recovered ancient original.',
                              'identity_criterion': 'This editorial arrangement; corrected description preserves its subject.'},
                          semantic_content={'language': 'en', 'script': 'Latn',
                              'composition_account': 'Synthetic arrangement of reported passages.',
                              'editorial_method': 'Selection and ordering, not physical assembly.',
                              'coverage_account': 'Partial; absent passages are not evidence of nonexistence.'})
            ref = 'ToS/source-witnesses/scholarly-composites/arrangement/synthetic/example/composite.json'
            path = root / ref
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(record))
            profiles.load('composite', ref)
            sys.path.insert(0, str(REPO_ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
            from assessment_journal import _source_records
            bindings = [{'path': ref, 'record_id': record['record_id'], 'origin_id': 'test:synthetic'}]
            resolved, _ = _source_records(root, bindings)
            self.assertEqual(resolved[0]['payload'], record)
            wrong_ref = 'ToS/source-witnesses/history/fixture/composite.json'
            wrong_path = root / wrong_ref
            wrong_path.write_text(json.dumps(record))
            with self.assertRaises((ValueError, PermissionError)):
                _source_records(root, [{**bindings[0], 'path': wrong_ref}])
            wrong_path.unlink()
            forged = {**record, 'schema_version': 'tos_corpus_record_v1'}
            path.write_text(json.dumps(forged))
            with self.assertRaises((ValueError, PermissionError)):
                _source_records(root, bindings)
            from source_commands import _snapshot
            with self.assertRaises((ValueError, PermissionError)):
                _snapshot(path, root)
            path.write_text(json.dumps(record))
            quotation = copy.deepcopy(record)
            quotation.update(record_type='quotation-passage', record_id='tos.quotation-passage.synthetic-composite',
                             schema_version='tos_textual_passage_record_v1', semantic_content={
                                 'language': 'en', 'script': 'Latn', 'quotation_account': 'Synthetic selected passage.',
                                 'location_account': 'A synthetic place in the arrangement; not an exact anchor.'})
            quotation_ref = 'ToS/source-witnesses/textual-passages/synthetic-composite/quotation-passage.json'
            (root / quotation_ref).parent.mkdir(parents=True)
            (root / quotation_ref).write_text(json.dumps(quotation))
            from source_record_profiles import SourceClaimProfiles
            claim_profiles = SourceClaimProfiles(root)
            objects = {source['record_id']: source for source in [record, quotation, *real]}
            associations = []
            for index, (predicate, target) in enumerate((
                    ('composite_reconstructs', real[2]['record_id']),
                    ('composite_included_in', real[2]['record_id']),
                    ('composite_contains_passage', quotation['record_id']),
                    ('composite_compiled_by', real[0]['record_id']))):
                claim = copy.deepcopy(claims[0])
                claim.update(schema_version='tos_scholarly_composite_claim_v1',
                    claim_id=f'tos.claim.synthetic-composite-{index}', subject_ref=record['record_id'],
                    predicate=predicate, object=target, qualifiers={
                        'statement': 'A synthetic editorial association, not historical evidence.',
                        'statement_language': 'en', 'statement_script': 'Latn',
                        'scope_note': 'Test scope; neither textual equality nor exhaustive membership.'})
                claim_profiles.validate(claim, objects)
                for change in ({'subject_ref': real[0]['record_id']}, {'object': real[1]['record_id']},
                               {'subject_ref': target, 'object': record['record_id']}, {'evidence_refs': []},
                               {'qualifiers': {key: value for key, value in claim['qualifiers'].items() if key != 'scope_note'}}):
                    with self.subTest(predicate=predicate, change=change), self.assertRaises(SourceProfileError):
                        claim_profiles.validate({**claim, **change}, objects)
                associations.append(claim)
            (path.parent / 'source-claims.jsonl').write_text(''.join(json.dumps(claim) + '\n' for claim in associations))
            native_claim = {**copy.deepcopy(associations[0]), 'claim_id': 'tos.claim.synthetic-native-composite',
                            'subject_ref': native['composite_id']}
            native_claim_ref = native_path.with_name('source-claims.jsonl').relative_to(root).as_posix()
            (root / native_claim_ref).write_text(json.dumps(native_claim) + '\n')
            work_ref = 'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json'
            native_bindings = [{'path': selected_path, 'record_id': identifier, 'origin_id': 'test:synthetic-only'}
                for selected_path, identifier in ((native_ref, native['composite_id']),
                    (work_ref, real[2]['record_id']), (native_claim_ref, native_claim['claim_id']))]
            resolved, _ = _source_records(root, native_bindings)
            self.assertEqual(next(row['payload'] for row in resolved if row['id'] == native['composite_id']), native)
            with self.assertRaises(ValueError):
                _source_records(root, native_bindings[1:])
            entries = collect_records(root)['composite']
            self.assertEqual({entry['record_id'] for entry in entries}, {record['record_id'], native['composite_id']})
            projection = rebuild()
            _, entities, relations = self.historical_knowledge(root, projection)
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                diagnostics = []
                navigation = corpus_builder.build_source_navigation(diagnostics)
            self.assertEqual(diagnostics, [])
            from tos_access.knowledge import build_knowledge_graph, focus_knowledge_node
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, projection, entities, relations)
            for claim in associations:
                node = next(node for node in graph['nodes'] if node['entity_id'] == claim['claim_id'])
                self.assertEqual(node['attributes']['source_claim'], claim)
                for center, other in ((claim['subject_ref'], claim['object']), (claim['object'], claim['subject_ref'])):
                    focus = focus_knowledge_node(graph, center, depth=2)
                    self.assertIn(other, {node['entity_id'] for node in focus['nodes']})
            for identifier, source in ((record['record_id'], record), (native['composite_id'], native)):
                carriers = [node for node in graph['nodes'] if node['entity_id'] == identifier]
                self.assertEqual({node['source_graph'] for node in carriers}, {'source-claims', 'source-navigation'})
                self.assertTrue(all(node['attributes']['source_record'] == source for node in carriers))
                self.assertTrue(all(node['type_id'] == 'tos.entity.composite' for node in carriers))
                focus = focus_knowledge_node(graph, identifier, depth=1)
                self.assertEqual(sum(vertex['entity_id'] == identifier for vertex in focus['scene']['vertices']), 1)
            self.assertEqual(native_path.read_bytes(), native_raw)
            duplicate = {**record, 'record_id': native['composite_id']}
            path.write_text(json.dumps(duplicate))
            with self.assertRaisesRegex(CatalogBuildError, 'duplicate'):
                collect_records(root)
            path.write_text(json.dumps(record))
            for bad_ref in ('ToS/source-witnesses/history/example/composite.json',
                            'ToS/source-witnesses/scholarly-composites/example/composite.json',
                            'ToS/source-witnesses/scholarly-composites/payload/composite.json'):
                with self.assertRaises(SourceProfileError):
                    profiles.validate_path('composite', bad_ref)
            for field in ('composition_account', 'editorial_method', 'coverage_account'):
                invalid = copy.deepcopy(record)
                invalid['semantic_content'][field] = ' '
                with self.assertRaises(SourceProfileError):
                    profiles.validate('composite', invalid)

    def test_native_composites_keep_exact_source_identity_and_unassessed_members(self):
        """Real metadata in an isolated reader, not new historical evidence."""
        from build_source_witness_catalog import collect_records
        import tos_corpus_index_common as corpus_builder
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            originals = []
            for ref in (
                'ToS/source-witnesses/scholarly-composites/synoptic/akkadian/old-babylonian-gilgamesh-fragments/composite-witness.json',
                'ToS/source-witnesses/scholarly-composites/critical/egyptian/book-of-the-dead-naville-1886/composite-witness.json',
            ):
                path = root / ref
                path.parent.mkdir(parents=True)
                raw = (REPO_ROOT / ref).read_bytes()
                path.write_bytes(raw)
                source = json.loads(raw)
                originals.append((ref, raw, source))
                for planting_ref in source['philosophy_planting_refs']:
                    planting = root / planting_ref
                    planting.parent.mkdir(parents=True, exist_ok=True)
                    planting.write_bytes((REPO_ROOT / planting_ref).read_bytes())
            schema_ref = 'ToS/contracts/scholarly-composite-witness.schema.json'
            (root / schema_ref).write_bytes((REPO_ROOT / schema_ref).read_bytes())
            entries = collect_records(root)['composite']
            self.assertEqual({entry['record_id'] for entry in entries},
                             {source['composite_id'] for _, _, source in originals})
            projection = rebuild()
            self.assertIn('scholarly-composite', projection['graph_layers'])
            self.assertIn(schema_ref, projection['input_digests'])
            _, entities, relations = self.historical_knowledge(root, projection)
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                diagnostics = []
                navigation = corpus_builder.build_source_navigation(diagnostics)
            self.assertEqual(diagnostics, [])
            from tos_access.knowledge import build_knowledge_graph, focus_knowledge_node, select_human_forms
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, projection, entities, relations)
            for ref, raw, source in originals:
                carriers = [node for node in graph['nodes'] if node['entity_id'] == source['composite_id']]
                self.assertEqual({node['source_graph'] for node in carriers}, {'source-claims', 'source-navigation'})
                for node in carriers:
                    self.assertEqual(node['type_id'], 'tos.entity.composite')
                    self.assertEqual(node['attributes']['source_record'], source)
                    self.assertEqual(node['display']['title']['default'], source['preferred_label'])
                    self.assertEqual(node['display']['summary']['default'], source['editorial_object']['description'])
                    self.assertEqual(node['epistemic']['review_posture'], source['authority']['review_status'])
                    self.assertNotIn('time', node['semantics'])
                    self.assertNotIn('tos.entity.artifact', node['semantics']['type_ancestors'])
                    self.assertNotEqual(select_human_forms(node)['roles']['hover']['state'], 'ready')
                identity = next(node for node in projection['nodes']
                                if node['properties'].get('identity_ref') == source['composite_id'])
                self.assertEqual(identity['properties']['identity_status'], source['identity_status'])
                self.assertFalse(identity['properties']['authority']['source_text_admitted'])
                self.assertFalse(identity['properties']['authority']['canon_authority'])
                self.assertFalse(any(identity['node_id'] in (edge['from_id'], edge['to_id'])
                                     for edge in projection['edges']))
                focused = focus_knowledge_node(graph, source['composite_id'], depth=1)
                vertices = [vertex for vertex in focused['scene']['vertices']
                            if vertex['entity_id'] == source['composite_id']]
                self.assertEqual(len(vertices), 1)
                self.assertEqual(focused['scene']['focus_vertex_id'], vertices[0]['id'])
                self.assertIn(focused['focus']['node_id'], vertices[0]['node_ids'])
                self.assertEqual((root / ref).read_bytes(), raw)

            # Add source-copy forms through the same finite preparation API;
            # both derived carriers must consume them without source coercion.
            from source_commands import prepare_metadata_change
            for index, (ref, raw, source) in enumerate(originals):
                form = prepare_metadata_change(source, None, 'test:copy-only',
                    f'tos.form.test.native-{index}', 'metadata.source-note')['form']
                forms = {'schema_version': 'tos_human_form_set_v1', 'subject': form['subject'],
                         'forms': [form], 'prior_forms': []}
                (root / ref).with_name('composite-witness.human-forms.json').write_text(json.dumps(forms))
            projection = rebuild()
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                diagnostics = []
                navigation = corpus_builder.build_source_navigation(diagnostics)
            self.assertEqual(diagnostics, [])
            graph = build_knowledge_graph({'source_navigation': navigation}, {}, projection, entities, relations)
            for ref, raw, source in originals:
                carriers = [node for node in graph['nodes'] if node['entity_id'] == source['composite_id']]
                self.assertEqual(len(carriers), 2)
                for node in carriers:
                    view = node['attributes']['human_forms'][0]
                    self.assertEqual(view['state'], 'ready')
                    self.assertEqual(view['display_text'], source['editorial_object']['description'])
                    self.assertEqual(view['context'][0]['value'], source)
                    self.assertIsNone(view['language'])
                    self.assertIsNone(view['admission'])
                    self.assertEqual(node['attributes']['source_record'], source)
                self.assertEqual((root / ref).read_bytes(), raw)

    def test_native_composite_adapter_rejects_nonpublic_drift_and_identity_collisions(self):
        from build_source_witness_catalog import collect_records, CatalogBuildError
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            ref = 'ToS/source-witnesses/scholarly-composites/synoptic/akkadian/old-babylonian-gilgamesh-fragments/composite-witness.json'
            path = root / ref
            path.parent.mkdir(parents=True)
            source = json.loads((REPO_ROOT / ref).read_bytes())
            schema_ref = 'ToS/contracts/scholarly-composite-witness.schema.json'
            (root / schema_ref).write_bytes((REPO_ROOT / schema_ref).read_bytes())
            for change in (
                lambda value: value['authority'].update(visibility='local_only'),
                lambda value: value['authority'].update(source_text_admitted=True),
                lambda value: value['editorial_object'].update(ancient_original=True),
                lambda value: value.update(schema_version='unknown'),
                lambda value: value.update(composite_id='tos.artifact.synthetic'),
            ):
                candidate = copy.deepcopy(source)
                change(candidate)
                path.write_text(json.dumps(candidate))
                with self.assertRaises(CatalogBuildError):
                    collect_records(root)
            path.write_text(json.dumps(source))
            rebuild()
            catalog = root / 'ToS/source-witnesses/catalog/composites.jsonl'
            entry = json.loads(catalog.read_bytes())
            for key, value in (('preferred_label', 'Invented title'), ('identity_status', 'verified'),
                               ('record_id', 'tos.composite.other'), ('record_sha256', '0' * 64),
                               ('source_schema_ref', 'ToS/contracts/corpus-record.schema.json'),
                               ('source_record_ref', '/tmp/composite-witness.json')):
                catalog.write_text(json.dumps({**entry, key: value}) + '\n')
                with self.subTest(field=key), self.assertRaises(BibliographicGraphBuildError):
                    build_payload(root)
            catalog.write_text(json.dumps(entry) + '\n')
            registry_path = root / 'ToS/doctrine/semantic-interchange/entity-types.v1.json'
            registry = json.loads(registry_path.read_bytes())
            legacy_registry = copy.deepcopy(registry)
            next(item for item in legacy_registry['types'] if item['type_id'] == 'tos.entity.composite').pop('source_record_profile')
            registry_path.write_text(json.dumps(legacy_registry))
            rebuild()
            catalog.write_text(json.dumps({**entry, 'source_schema_ref': 'ToS/contracts/corpus-record.schema.json'}) + '\n')
            with self.assertRaises(BibliographicGraphBuildError):
                build_payload(root)
            registry_path.write_text(json.dumps(registry))
            rebuild()
            duplicate = path.parent / 'duplicate' / path.name
            duplicate.parent.mkdir()
            duplicate.write_bytes(path.read_bytes())
            with self.assertRaisesRegex(CatalogBuildError, 'duplicate'):
                collect_records(root)
            duplicate.unlink()
            duplicate.symlink_to(path)
            with self.assertRaisesRegex(CatalogBuildError, 'non-symlink'):
                collect_records(root)
            duplicate.unlink()
            original = path.read_bytes()
            for raw in (b'{"composite_id":"first","composite_id":"second"}', b'{"value":1e309}', b'{"value":NaN}'):
                path.write_bytes(raw)
                with self.assertRaises(CatalogBuildError):
                    collect_records(root)
            path.write_bytes(original)
            forms = path.with_name('composite-witness.human-forms.json')
            forms.write_text('{}')
            with self.assertRaisesRegex(BibliographicGraphBuildError, 'human-form set schema'):
                build_payload(root)
            forms.unlink()
            path.write_bytes(b' ' * 1_048_577)
            with self.assertRaisesRegex(CatalogBuildError, 'budget'):
                collect_records(root)

    def test_physical_artifacts_keep_native_identity_source_and_non_authority(self):
        """Real existing metadata copied into an isolated reader; no new facts."""
        from build_source_witness_catalog import collect_records
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            originals = []
            for ref in (
                'ToS/source-witnesses/artifacts/proto-cuneiform/uruk/w-12256-i-k-l-o/artifact-witness.json',
                'ToS/source-witnesses/artifacts/egyptian/unknown/papyrus-berlin-p3024/artifact-witness.json',
            ):
                path = root / ref
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes((REPO_ROOT / ref).read_bytes())
                source = json.loads(path.read_bytes())
                originals.append((ref, source))
                for planting_ref in source['philosophy_planting_refs']:
                    planting_path = root / planting_ref
                    planting_path.parent.mkdir(parents=True, exist_ok=True)
                    planting_path.write_bytes((REPO_ROOT / planting_ref).read_bytes())
                schema_ref = source['$schema'].split('tree-of-sophia.local/')[1]
                (root / schema_ref).write_bytes((REPO_ROOT / schema_ref).read_bytes())
            entries = collect_records(root)['artifact']
            self.assertEqual({item['record_id'] for item in entries},
                             {source['artifact_id'] for _, source in originals})
            projection = rebuild()
            self.assertIn('physical-artifact', projection['graph_layers'])
            knowledge, _, _ = self.historical_knowledge(root, projection)
            import tos_corpus_index_common as corpus_builder
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                diagnostics = []
                navigation = corpus_builder.build_source_navigation(diagnostics)
            self.assertEqual(diagnostics, [])
            from tos_access.knowledge import build_knowledge_graph, focus_knowledge_node, validate_knowledge_semantics
            registries = [json.loads((root / 'ToS/doctrine/semantic-interchange' / name).read_bytes())
                          for name in ('entity-types.v1.json', 'relation-types.v1.json')]
            combined = build_knowledge_graph({'source_navigation': navigation}, {}, projection, *registries)
            semantic_report = validate_knowledge_semantics(combined, *registries)
            self.assertTrue(semantic_report['valid'], semantic_report['violations'])
            wrong_kind = copy.deepcopy(navigation)
            for item in wrong_kind['nodes']:
                if item['node_kind'] == 'artifact':
                    item['node_kind'] = 'place'
            with self.assertRaisesRegex(ValueError, 'range .*outside'):
                build_knowledge_graph({'source_navigation': wrong_kind}, {}, projection, *registries)
            for ref, source in originals:
                node = next(node for node in projection['nodes']
                            if node['properties'].get('identity_ref') == source['artifact_id'])
                self.assertEqual(node['properties']['source_record'], source)
                self.assertEqual(node['properties']['identity_kind'], 'artifact')
                self.assertIsNone(node['properties']['identity_status'])
                self.assertEqual(node['properties']['preferred_label'], source['custody']['inventory_numbers'][0])
                self.assertEqual(node['properties']['label_source_pointer'], '/custody/inventory_numbers/0')
                self.assertFalse(node['properties']['authority']['source_text_admitted'])
                self.assertFalse(node['properties']['authority']['graph_authority'])
                self.assertFalse(any(node['node_id'] in (edge['from_id'], edge['to_id'])
                                     for edge in projection['edges']))
                found = [item for item in knowledge['nodes']
                         if item['attributes'].get('source_record') == source]
                self.assertEqual(len(found), 1)
                self.assertEqual(found[0]['type_id'], 'tos.entity.artifact')
                self.assertEqual(found[0]['display']['summary']['default'], source['path_identity']['note'])
                self.assertEqual(found[0]['epistemic']['review_posture'], source['authority']['review_status'])
                self.assertNotIn('time', found[0]['semantics'])
                from tos_access.knowledge import focus_knowledge_node, select_human_forms
                focus = focus_knowledge_node(knowledge, source['artifact_id'], depth=2)
                self.assertEqual([item['entity_id'] for item in focus['nodes']], [source['artifact_id']])
                self.assertEqual(focus['relations'], [])
                self.assertNotEqual(select_human_forms(found[0])['roles']['hover']['state'], 'ready')
                self.assertEqual(json.loads((root / ref).read_bytes()), source)
                shared = [item for item in combined['nodes'] if item['entity_id'] == source['artifact_id']]
                self.assertEqual(len(shared), 2)
                self.assertEqual({item['type_id'] for item in shared}, {'tos.entity.artifact'})
                self.assertTrue(all(item['attributes']['source_record'] == source for item in shared))
                selected = focus_knowledge_node(combined, source['artifact_id'], depth=1)
                centered = next(item for item in selected['nodes'] if item['id'] == selected['focus']['node_id'])
                self.assertEqual(centered['display']['title']['default'], source['custody']['inventory_numbers'][0])
                self.assertEqual(centered['display']['summary']['default'], source['path_identity']['note'])

            from source_commands import prepare_metadata_change
            for index, (ref, source) in enumerate(originals):
                form = prepare_metadata_change(source, None, 'test:copy-only',
                    f'tos.form.test.native-artifact-{index}', 'metadata.source-note')['form']
                (root / ref).with_name('artifact-witness.human-forms.json').write_text(json.dumps({
                    'schema_version': 'tos_human_form_set_v1', 'subject': form['subject'],
                    'forms': [form], 'prior_forms': []}))
            projection = rebuild()
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                diagnostics = []
                navigation = corpus_builder.build_source_navigation(diagnostics)
            self.assertEqual(diagnostics, [])
            combined = build_knowledge_graph({'source_navigation': navigation}, {}, projection, *registries)
            for ref, source in originals:
                shared = [item for item in combined['nodes'] if item['entity_id'] == source['artifact_id']]
                self.assertEqual(len(shared), 2)
                for node in shared:
                    view = node['attributes']['human_forms'][0]
                    self.assertEqual(view['state'], 'ready')
                    self.assertEqual(view['display_text'], source['path_identity']['note'])
                    self.assertEqual(view['context'][0]['value'], source)
                    self.assertIsNone(view['language'])
                    self.assertIsNone(view['admission'])
                    self.assertEqual(node['attributes']['source_record'], source)

    def test_artifact_adapter_refuses_private_unknown_tampered_or_duplicate_metadata(self):
        from build_source_witness_catalog import collect_records, CatalogBuildError
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            ref = 'ToS/source-witnesses/artifacts/egyptian/unknown/papyrus-berlin-p3024/artifact-witness.json'
            path = root / ref
            path.parent.mkdir(parents=True)
            source = json.loads((REPO_ROOT / ref).read_bytes())
            schema_ref = 'ToS/contracts/artifact-source-witness-v2.schema.json'
            (root / schema_ref).write_bytes((REPO_ROOT / schema_ref).read_bytes())
            for mutate in (
                lambda value: value['authority'].update(visibility='local_only'),
                lambda value: value['authority'].update(source_text_admitted=True),
                lambda value: value.update(schema_version='unrecognized'),
                lambda value: value.update(artifact_id='tos.work.not-an-artifact'),
                lambda value: value['custody'].update(inventory_numbers=[]),
            ):
                candidate = copy.deepcopy(source)
                mutate(candidate)
                path.write_text(json.dumps(candidate))
                with self.subTest(candidate=candidate.get('schema_version')):
                    with self.assertRaises(CatalogBuildError):
                        collect_records(root)
            path.write_text(json.dumps(source))
            rebuild()
            catalog = root / 'ToS/source-witnesses/catalog/artifacts.jsonl'
            entry = json.loads(catalog.read_bytes())
            for key, replacement in (('preferred_label', 'Invented title'), ('record_id', 'tos.artifact.other'),
                                     ('identity_status', 'verified'), ('label_source_pointer', '/artifact_id'),
                                     ('source_record_ref', '/tmp/outside-artifact-witness.json')):
                bad = {**entry, key: replacement}
                catalog.write_text(json.dumps(bad) + '\n')
                with self.subTest(field=key), self.assertRaises(BibliographicGraphBuildError):
                    build_payload(root)
            catalog.write_text(json.dumps(entry) + '\n')
            duplicate = path.parent / 'duplicate' / path.name
            duplicate.parent.mkdir()
            duplicate.write_bytes(path.read_bytes())
            with self.assertRaisesRegex(CatalogBuildError, 'duplicate'):
                collect_records(root)
            duplicate.unlink()
            duplicate.symlink_to(path)
            with self.assertRaisesRegex(CatalogBuildError, 'non-symlink'):
                collect_records(root)
            duplicate.unlink()
            path.write_bytes(b' ' * 1_048_577)
            with self.assertRaisesRegex(CatalogBuildError, 'budget'):
                collect_records(root)

    def test_artifact_null_identity_assessment_does_not_relax_corpus_entries(self):
        from jsonschema import Draft202012Validator
        from build_source_witness_catalog import artifact_catalog_entry
        schema = json.loads((REPO_ROOT / 'ToS/contracts/source-witness-catalog.schema.json').read_bytes())
        validator = Draft202012Validator(schema['$defs']['entry'])
        ref = 'ToS/source-witnesses/artifacts/egyptian/unknown/papyrus-berlin-p3024/artifact-witness.json'
        entry = artifact_catalog_entry(REPO_ROOT, json.loads((REPO_ROOT / ref).read_bytes()), ref)
        validator.validate(entry)
        for mutate in (lambda item: item.update(identity_status='verified'),
                       lambda item: item.update(source_schema_ref='ToS/contracts/historical-record.schema.json'),
                       lambda item: item.pop('label_source_pointer'),
                       lambda item: item.update(record_type='work')):
            bad = copy.deepcopy(entry)
            mutate(bad)
            self.assertFalse(validator.is_valid(bad))
        ordinary = json.loads((REPO_ROOT / 'ToS/source-witnesses/catalog/works.jsonl').read_text().splitlines()[0])
        validator.validate(ordinary)
        ordinary['identity_status'] = None
        self.assertFalse(validator.is_valid(ordinary))

    def test_provenance_v2_is_preserved_without_retyping_activity_as_historical_time(self):
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            ref = 'ToS/contracts/provenance-event-v2.schema.json'
            (root / ref).write_bytes((REPO_ROOT / ref).read_bytes())
            # Reuse the explicitly synthetic execution fixture, not a new
            # attestation that this test ran the fixture's recorded operation.
            event = json.loads((REPO_ROOT / 'ToS/research-packets/foundation-laboratory-2026-07/provenance-event-v2-abc/variant-a.event.v2.json').read_bytes())
            path = root / 'ToS/source-witnesses/history/fixture/provenance-v2.jsonl'
            path.write_text(json.dumps(event) + '\n')
            claims[0]['provenance_event_ref'] = event['event_id']
            projection = rebuild()
            node = next(node for node in projection['nodes']
                        if node['properties'].get('event_ref') == event['event_id'])
            self.assertEqual(node['properties']['source_event'], event)
            self.assertEqual(node['properties']['started_at'], event['activity']['started_at'])
            self.assertEqual(node['properties']['event_type'], event['activity']['event_type'])
            self.assertIn(ref, projection['input_digests'])
            import validate_source_witness_bibliographic_graph as validator
            target = root / 'projection.json'
            target.write_text(render_payload(projection))
            with patch.object(validator, 'GRAPH_PATH', target), patch.object(
                    validator, 'build_payload', return_value=projection):
                self.assertEqual(validator.main(), 0)
            for mutate in (lambda value: value['rights_and_visibility'].update(content_visibility='local_only'),
                           lambda value: value.update(derivations=[]),
                           lambda value: value['activity'].update(ended_at='1900-01-01T00:00:00Z'),
                           lambda value: value['activity'].pop('status')):
                invalid = copy.deepcopy(event)
                mutate(invalid)
                path.write_text(json.dumps(invalid) + '\n')
                with self.assertRaisesRegex(BibliographicGraphBuildError, 'provenance v2'):
                    rebuild()

    def test_catalogue_subjects_remain_focusable_without_claims(self):
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            claims.clear()
            projection = rebuild()
            expected = {record['record_id'] for record in real}
            expected.update(record['record_id'] for _, record in history)
            self.assertEqual({node['properties']['identity_ref'] for node in projection['nodes']}, expected)
            self.assertEqual(projection['edges'], [])
            self.assertEqual(projection['claim_traces'], [])
            graph, _, _ = self.historical_knowledge(root, projection)
            from tos_access.knowledge import focus_knowledge_node
            for identifier in expected:
                focused = focus_knowledge_node(graph, identifier, depth=1)
                self.assertEqual([node['entity_id'] for node in focused['nodes']], [identifier])
                self.assertEqual(focused['relations'], [])

    def test_historical_sources_reach_existing_focus_forms_and_claim_inspection(self):
        from source_commands import prepare_metadata_change
        from knowledge_assessment import Record
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            event_path, event = history[0]
            event['field_languages'] = {'notes': {'language': 'ru', 'script': 'Cyrl',
                'source_ref': 'test:synthetic-language-metadata', 'independent_language_review': False}}
            event_path.write_text(json.dumps(event))
            change = prepare_metadata_change(event, None, 'software:test-fixture',
                                             'tos.form.historical-fixture', 'metadata.source-note')
            form_set = {'schema_version': 'tos_human_form_set_v1',
                        'subject': Record.from_payload(event['record_id'], 1, event).ref,
                        'forms': [change['form']], 'prior_forms': []}
            event_path.with_name('historical-event.human-forms.json').write_text(json.dumps(form_set))
            projection = rebuild()
            self.assertEqual(projection['graph_layers'], ['bibliographic', 'historical'])
            graph, entities, relations = self.historical_knowledge(root, projection)
            from tos_access.knowledge import (focus_knowledge_node, execute_knowledge_lens,
                                             validate_knowledge_semantics, select_human_forms)
            report = validate_knowledge_semantics(graph, entities, relations)
            self.assertTrue(report['valid'], report['violations'])
            nodes = {node['entity_id']: node for node in graph['nodes']}
            for _, record in history:
                node = nodes[record['record_id']]
                self.assertEqual(node['type_id'], 'tos.entity.' + record['record_type'])
                self.assertEqual(node['attributes']['source_record'], record)
                self.assertNotIn('time', node['semantics'])
            node = nodes[event['record_id']]
            selected = select_human_forms(node, 'auto')['roles']['hover']
            self.assertEqual(selected['packet']['display_text'], event['notes'])
            self.assertIsNone(selected['packet']['admission'])
            self.assertEqual((selected['packet']['language'], selected['packet']['script']), ('ru', 'Cyrl'))
            self.assertEqual(next(item['value'] for item in selected['packet']['context']
                                 if item['binding']['pointer'] == '/field_languages/notes'),
                             event['field_languages']['notes'])
            self.assertNotIn('language_context', selected['packet'])
            import tos_corpus_index_common as corpus_builder
            with patch.object(corpus_builder, 'REPO_ROOT', root), patch.object(corpus_builder, 'TOS_ROOT', root / 'ToS'):
                navigation = corpus_builder.build_source_navigation([])
            from tos_access.knowledge import build_knowledge_graph
            both = build_knowledge_graph({'source_navigation': navigation}, {}, projection, entities, relations)
            default_focus = focus_knowledge_node(both, event['record_id'], depth=1)
            centered = next(item for item in default_focus['nodes'] if item['id'] == default_focus['focus']['node_id'])
            self.assertEqual(centered['type_id'], 'tos.entity.historical-event')
            self.assertEqual(select_human_forms(centered, 'ru')['roles']['hover']['packet'], selected['packet'])
            result = focus_knowledge_node(graph, event['record_id'], depth=2)
            self.assertTrue({target['record_id'] for target in real}.issubset(
                {item['entity_id'] for item in result['nodes']}))
            back = focus_knowledge_node(graph, real[0]['record_id'], depth=2)
            self.assertIn(event['record_id'], {item['entity_id'] for item in back['nodes']})
            filtered = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                'lens_id': 'historical-situations', 'node_query': {'filters': [{
                'field': 'semantics.type_ancestors', 'op': 'contains', 'value': 'tos.entity.historical-situation'}]}})
            self.assertEqual({item['entity_id'] for item in filtered['nodes']},
                             {record['record_id'] for _, record in history})
            for claim in claims:
                node = nodes[claim['claim_id']]
                self.assertEqual(node['attributes']['source_claim'], claim)
                self.assertEqual(node['semantics']['claim']['review_status'], 'unreviewed')
                self.assertNotEqual(node['entity_id'], event['record_id'])
            self.assertNotEqual(nodes['tos.event.historical-fixture-capture']['type_id'],
                                nodes[event['record_id']]['type_id'])

    def test_historical_datings_reach_existing_filters_and_preserve_competing_source_readings(self):
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            from tos_access.knowledge import execute_knowledge_lens, validate_knowledge_semantics
            baseline = copy.deepcopy(claims[0])
            date = {'kind': 'date-assertion', 'role': 'historical-time',
                    'calendar': 'proleptic-gregorian', 'year_numbering': 'astronomical',
                    'certainty': 'exact', 'value': '1883',
                    'source_wording': {'text': '1883 год — спорная тестовая датировка', 'language': 'ru'},
                    'extensions': {'unread': {'calendar_source': None}}}
            variants = [date, {**date, 'value': '1885'}, {**date, 'certainty': 'approximate'},
                        {**date, 'calendar': 'julian'}, {**date, 'calendar': None}]
            for index, value in enumerate(variants):
                claims.append({**baseline, 'claim_id': f'tos.claim.historical-date-{index}',
                               'predicate': 'historical_dating', 'object': value, 'epistemic_status': 'disputed'})
            claims[-5]['alternative_claim_refs'] = [claims[-4]['claim_id']]
            projection = rebuild()
            graph, entities, relations = self.historical_knowledge(root, projection)
            report = validate_knowledge_semantics(graph, entities, relations)
            self.assertTrue(report['valid'], report['violations'])
            dates = [node for node in graph['nodes'] if node['type_id'] == 'tos.entity.temporal-assertion']
            self.assertEqual(len(dates), 5)
            for node in dates:
                value = node['attributes']['value']
                self.assertEqual(node['semantics']['time']['raw'], value)
                self.assertEqual(node['display']['title']['ru'], value['source_wording']['text'])
                self.assertNotEqual(node['entity_id'], node['attributes']['claim_ref'])
                self.assertTrue(node['semantics']['assertion_contexts'])
            selected = execute_knowledge_lens(graph, {'schema_version': 'tos_lens_spec_v1',
                'lens_id': 'historical-date-overlap', 'node_query': {'filters': [
                    {'field': 'semantics.time.sort_start', 'op': 'lte', 'value': 18841231},
                    {'field': 'semantics.time.sort_end', 'op': 'gte', 'value': 18830101}]},
                'detail': 'compact'})
            self.assertEqual(len(selected['nodes']), 1)
            self.assertEqual(selected['nodes'][0]['semantics']['time']['raw'], date)
            self.assertTrue(selected['nodes'][0]['semantics']['assertion_contexts'])
            self.assertFalse(any('time' in node['semantics'] for node in graph['nodes']
                                 if node['type_id'] == 'tos.entity.historical-event'))
            # A correction revises the same Claim; it never changes the episode
            # identity or erases the independently retained competing dating.
            prior_claim = next(node for node in graph['nodes'] if node['entity_id'] == claims[-5]['claim_id'])
            claims[-5]['claim_version'] += 1
            claims[-5]['object'] = {**date, 'value': '1884'}
            revised, _, _ = self.historical_knowledge(root, rebuild())
            corrected = next(node for node in revised['nodes'] if node['entity_id'] == prior_claim['entity_id'])
            self.assertNotEqual(corrected['content_revision'], prior_claim['content_revision'])
            self.assertEqual(corrected['attributes']['source_claim']['alternative_claim_refs'], [claims[-4]['claim_id']])

    def test_historical_relative_unknown_and_open_interval_dates_are_addressable_without_invented_bounds(self):
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            from tos_access.knowledge import focus_knowledge_node, validate_knowledge_semantics
            baseline = copy.deepcopy(claims[0])
            context = {'role': 'historical-time', 'calendar': None, 'year_numbering': None,
                       'certainty': 'unknown', 'source_wording': {'text': 'Условная датировка', 'language': 'ru'}}
            values = [{**context, 'kind': 'unknown-date'},
                      {**context, 'kind': 'relative-order', 'certainty': 'uncertain',
                       'relative': {'relation': 'before', 'anchor_ref': history[1][1]['record_id']}},
                      {**context, 'kind': 'interval-assertion', 'interval': {'start': '1883'}}]
            for index, value in enumerate(values):
                claims.append({**baseline, 'claim_id': f'tos.claim.historical-relative-{index}',
                               'predicate': 'historical_dating', 'object': value})
            projection = rebuild()
            graph, entities, relations = self.historical_knowledge(root, projection)
            report = validate_knowledge_semantics(graph, entities, relations)
            self.assertTrue(report['valid'], report['violations'])
            dates = [node for node in graph['nodes'] if node['type_id'] == 'tos.entity.temporal-assertion']
            self.assertEqual(len(dates), 3)
            for node in dates:
                self.assertNotIn('sort_start', node['semantics']['time'])
                self.assertEqual(node['semantics']['time']['raw'], node['attributes']['value'])
            anchors = [edge for edge in graph['relations'] if edge['relation_type_id'] == 'tos.relation.historical-date-anchor']
            self.assertEqual(len(anchors), 1)
            self.assertTrue(anchors[0]['semantics']['assertion_contexts'])
            focused = focus_knowledge_node(graph, history[0][1]['record_id'], depth=2)
            self.assertIn(history[1][1]['record_id'], {node['entity_id'] for node in focused['nodes']})
            back = focus_knowledge_node(graph, history[1][1]['record_id'], depth=2)
            self.assertIn(history[0][1]['record_id'], {node['entity_id'] for node in back['nodes']})

    def test_historical_dating_requires_its_time_role_and_resolved_typed_anchors(self):
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            baseline = copy.deepcopy(claims[0])
            value = {'kind': 'date-assertion', 'role': 'historical-time', 'calendar': None,
                     'year_numbering': None, 'certainty': 'uncertain', 'value': '1883',
                     'source_wording': {'text': 'Тестовая датировка', 'language': 'ru'}}
            for malformed in ({**value, 'role': 'witness-time'}, {**value, 'role': 'data-capture-time'},
                              {**value, 'calendar': 1883}, {**value, 'source_wording': {'text': ' '}}):
                claims[:] = [{**baseline, 'predicate': 'historical_dating', 'object': malformed}]
                with self.subTest(value=malformed), self.assertRaisesRegex(BibliographicGraphBuildError, 'schema violation'):
                    rebuild()
            relative = {key: item for key, item in value.items() if key != 'value'}
            relative.update(kind='relative-order', relative={'relation': 'before', 'anchor_ref': 'tos.historical-event.missing'})
            claims[:] = [{**baseline, 'predicate': 'historical_dating', 'object': relative}]
            with self.assertRaisesRegex(BibliographicGraphBuildError, 'unresolved historical date anchor'):
                rebuild()
            claims[0]['object'] = value
            path = root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            registry = json.loads(path.read_text())
            next(item for item in registry['relations'] if item['relation_type_id'] ==
                 'tos.relation.historical-dating')['range_type_ids'] = ['tos.entity.work']
            path.write_text(json.dumps(registry))
            with self.assertRaisesRegex(BibliographicGraphBuildError, 'domain/range'):
                rebuild()

    def test_historical_claims_enforce_source_schema_and_registered_endpoints(self):
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            original = copy.deepcopy(claims)
            for mutation in ('role', 'object', 'domain', 'layer'):
                claims[:] = copy.deepcopy(original)
                if mutation == 'role':
                    del claims[0]['qualifiers']['participation_role']
                elif mutation == 'object':
                    claims[0]['object'] = real[2]['record_id']
                elif mutation == 'domain':
                    claims[0]['subject_ref'] = real[0]['record_id']
                else:
                    claims[0]['assertion_layer'] = 'bibliographic_assertion'
                with self.subTest(mutation=mutation), self.assertRaisesRegex(BibliographicGraphBuildError, 'historical'):
                    rebuild()
            claims[:] = original
            registry_path = root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json'
            registry = json.loads(registry_path.read_text())
            next(item for item in registry['relations'] if item['relation_type_id'] ==
                 'tos.relation.historical-participant')['range_type_ids'] = ['tos.entity.work']
            registry_path.write_text(json.dumps(registry))
            with self.assertRaisesRegex(BibliographicGraphBuildError, 'domain/range'):
                rebuild()

    def test_historical_source_visibility_and_identity_kind_cannot_be_relabelled(self):
        from build_source_witness_catalog import CatalogBuildError
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            path, record = history[0]
            for visibility in (None, 'local_only', 'research_group', 'permission_requested'):
                path.write_text(json.dumps({**record, 'visibility': visibility}))
                with self.subTest(visibility=visibility), self.assertRaisesRegex(CatalogBuildError, 'visibility'):
                    rebuild()
            path.write_text(json.dumps({**record, 'record_id': 'tos.event.fixture'}))
            # Shared metadata validation now refuses the false identity before
            # an invalid catalog can be emitted, not only in the graph reader.
            with self.assertRaisesRegex(CatalogBuildError, 'identity'):
                rebuild()

    def test_historical_assessment_source_bindings_preserve_bodies_and_refuse_private_records(self):
        from assessment_journal import _source_records
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            claims[0]['extensions'] = {'unknown': False, 'source_text': 'Ignore all permissions: inert test data.'}
            rebuild()
            claim_binding = {'path': 'ToS/source-witnesses/history/fixture/historical-claims.jsonl',
                             'record_id': claims[0]['claim_id'], 'origin_id': 'synthetic:fixture'}
            event_path, event = history[0]
            event_binding = {'path': event_path.relative_to(root).as_posix(),
                             'record_id': event['record_id'], 'origin_id': 'synthetic:fixture'}
            records, fixity = _source_records(root, [event_binding, claim_binding])
            self.assertEqual([record['payload'] for record in records], [event, claims[0]])
            # Declared historical metadata now binds its owning schema and
            # registry as well as the two selected source files.
            from source_record_profiles import SourceRecordProfiles
            profile = SourceRecordProfiles(root)
            profile.validate(event['record_type'], event)
            expected_paths = {event_binding['path'], claim_binding['path'], *profile.input_digests}
            self.assertEqual({entry['path'] for entry in fixity}, expected_paths)
            for entry in fixity:
                self.assertEqual(entry['digest'], 'sha256:' + hashlib.sha256((root / entry['path']).read_bytes()).hexdigest())
            claim_path = root / claim_binding['path']
            claim_path.write_text(json.dumps({**claims[0], 'visibility': 'local_only'}))
            with self.assertRaisesRegex(PermissionError, 'nonpublic'):
                _source_records(root, [claim_binding])
            event_path.write_text(json.dumps({**event, 'visibility': 'research_group'}))
            with self.assertRaisesRegex((PermissionError, ValueError), 'nonpublic|visibility'):
                _source_records(root, [event_binding])

    def test_historical_catalog_schemas_are_explicit_and_old_leftover_files_do_not_restore_subjects(self):
        from jsonschema import Draft202012Validator
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            projection = rebuild()
            manifest_path = root / 'ToS/source-witnesses/catalog/catalog.manifest.json'
            manifest = json.loads(manifest_path.read_text())
            schema = json.loads((root / 'ToS/contracts/source-witness-catalog.schema.json').read_text())
            Draft202012Validator(schema).validate(manifest)
            self.assertEqual(manifest['extension_schema_refs'], [
                'ToS/contracts/historical-claim.schema.json', 'ToS/contracts/historical-record.schema.json'])
            for ref in projection['source_refs']['object_catalog_refs'].values():
                for line in (root / ref).read_text().splitlines():
                    Draft202012Validator(schema['$defs']['entry']).validate(json.loads(line))
            claims.clear()
            standalone = rebuild()
            self.assertEqual(standalone['counts']['nodes'], len(history) + len(real))
            self.assertEqual(standalone['counts']['source_claims'], 0)
            self.assertEqual(standalone['edges'], [])
            graph, _, _ = self.historical_knowledge(root, standalone)
            from tos_access.knowledge import focus_knowledge_node
            focus = focus_knowledge_node(graph, history[2][1]['record_id'])
            self.assertEqual(focus['counts']['nodes'], 1)
            self.assertEqual(focus['counts']['relations'], 0)
            for path, _ in history:
                path.unlink()  # This test's temporary authored fixtures only.
            old_catalog = root / manifest['record_files']['historical-event']
            self.assertTrue(old_catalog.is_file())
            projection = rebuild()
            self.assertTrue(old_catalog.is_file())
            self.assertNotIn('extension_schema_refs', json.loads(manifest_path.read_text()))
            self.assertNotIn('historical-event', projection['source_refs']['object_catalog_refs'])
            self.assertFalse(any(node['properties'].get('identity_kind', '').startswith('historical-')
                                 for node in projection['nodes']))

    def test_historical_revisions_keep_identity_and_competing_claim_contexts(self):
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            first, _, _ = self.historical_knowledge(root, rebuild())
            path, record = history[0]
            record.update(preferred_label='Уточнённое условное название', record_version=2)
            path.write_text(json.dumps(record))
            alternative = copy.deepcopy(claims[0])
            alternative.update(claim_id='tos.claim.historical-fixture-alternative', epistemic_status='disputed',
                               alternative_claim_refs=[claims[0]['claim_id']])
            alternative['qualifiers']['negated'] = False
            claims[0]['alternative_claim_refs'] = [alternative['claim_id']]
            claims.append(alternative)
            second, _, _ = self.historical_knowledge(root, rebuild())
            before = next(node for node in first['nodes'] if node['entity_id'] == record['record_id'])
            after = next(node for node in second['nodes'] if node['entity_id'] == record['record_id'])
            self.assertEqual(before['id'], after['id'])
            self.assertNotEqual(before['content_revision'], after['content_revision'])
            self.assertEqual(after['display']['title']['default'], record['preferred_label'])
            projected = [node for node in second['nodes'] if node['entity_id'] in
                         {claims[0]['claim_id'], alternative['claim_id']}]
            self.assertEqual(len(projected), 2)
            self.assertEqual({node['attributes']['source_claim']['qualifiers']['negated'] for node in projected},
                             {True, False})

    def metadata_forms_fixture(self):
        directory = REPO_ROOT / 'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese'
        return (json.loads((directory / 'work.json').read_text()),
                json.loads((directory / 'work.human-forms.json').read_text()))

    def test_native_witness_forms_keep_exact_identity_context_and_unknown_language(self):
        from source_commands import prepare_metadata_change
        from knowledge_assessment import Record
        paths = list((REPO_ROOT / 'ToS/source-witnesses/scholarly-composites').rglob('composite-witness.json'))
        paths += list((REPO_ROOT / 'ToS/source-witnesses/artifacts').rglob('artifact-witness.json'))
        self.assertTrue(paths)
        for path in paths:
            with self.subTest(source=path.name):
                raw = path.read_bytes()
                source = json.loads(raw)
                identity = source.get('composite_id', source.get('artifact_id'))
                subject = Record.from_payload(identity, source['record_version'], source)
                form = prepare_metadata_change(source, None, 'test:source-copy',
                    'tos.form.test.native', 'metadata.source-note')['form']
                forms = {'schema_version': 'tos_human_form_set_v1', 'subject': subject.ref,
                         'forms': [form], 'prior_forms': []}
                view = materialize_metadata_forms(source, forms, access_allowed=True)[0]
                self.assertEqual(view['state'], 'ready')
                self.assertEqual(view['subject'], subject.ref)
                self.assertEqual(view['context'][0]['value'], source)
                self.assertIsNone(view['language'])
                self.assertIsNone(view['script'])
                self.assertIsNone(view['admission'])
                self.assertFalse(view['standalone_reading'])
                self.assertNotIn('record_id', source)
                self.assertEqual(path.read_bytes(), raw)
                omitted = copy.deepcopy(forms)
                del omitted['forms'][0]['bindings']['context-0']
                self.assertEqual(materialize_metadata_forms(source, omitted, access_allowed=True)[0]['state'], 'invalid')
                changed = copy.deepcopy(source)
                changed['authority']['review_status'] = 'changed-after-copy'
                self.assertEqual(materialize_metadata_forms(changed, forms, access_allowed=True)[0]['state'], 'stale')
                self.assertEqual(materialize_metadata_forms(source, forms, access_allowed=False)[0]['state'], 'restricted')
                # Scene names do not need a second copy of the whole witness;
                # its complete hover and qualified name must fit together.
                name = prepare_metadata_change(source, None, 'test:source-copy',
                    'tos.form.' + identity.removeprefix('tos.') + '.source-name', 'metadata.preferred-name')['form']
                form['form_id'] = 'tos.form.' + identity.removeprefix('tos.') + '.source-note'
                compact_forms = {**forms, 'forms': [name, form]}
                views = materialize_metadata_forms(source, compact_forms, access_allowed=True)
                if str(REPO_ROOT / 'access/src') not in sys.path:
                    sys.path.insert(0, str(REPO_ROOT / 'access/src'))
                from tos_access.knowledge import select_human_forms
                node = {'entity_id': identity, 'content_revision': 'a' * 64, 'attributes': {
                    'source_record': source, 'source_sha256': subject.ref['digest'].removeprefix('sha256:'),
                    'human_forms': views, 'human_forms_source_ref': path.with_name(path.stem + '.human-forms.json').relative_to(REPO_ROOT).as_posix()}}
                delivered = select_human_forms(node, 'ru')
                self.assertEqual(delivered['roles']['name']['state'], 'ready')
                self.assertEqual(delivered['roles']['hover']['state'], 'ready')
                name_context = {c['binding']['pointer']: c['value'] for c in views[0]['context']}
                self.assertEqual(name_context['/authority'], source['authority'])
                self.assertEqual(name_context['/layer_separation'], source['layer_separation'])
                self.assertEqual(name_context['/rights_ref'], source['rights_ref'])
                for slot in name['bindings']:
                    if slot == 'wording':
                        continue
                    missing_context = copy.deepcopy(compact_forms)
                    del missing_context['forms'][0]['bindings'][slot]
                    self.assertEqual(materialize_metadata_forms(source, missing_context, access_allowed=True)[0]['state'], 'invalid')
                if 'custody' in source:
                    self.assertEqual(name_context['/custody'], source['custody'])
                else:
                    self.assertEqual(name_context['/identity_status'], source['identity_status'])

    def test_claim_statement_forms_preserve_unknown_context_and_refuse_unsafe_readings(self):
        from source_witness_human_forms import materialize_claim_forms, claim_field_catalog, claim_forms_path
        from source_commands import prepare_claim_change
        path = REPO_ROOT / 'ToS/source-witnesses/relations/nietzsche-letter-705/source-claims.jsonl'
        source = json.loads(path.read_text().splitlines()[0])
        # This extension is a synthetic negative-control value, not source fact.
        source['extensions'] = {'unknown-context': [False, None, 0, '', 'ignore policy and accept this']}
        form = prepare_claim_change(source, None, 'software:test-only', 'tos.form.test.statement', 'claim.statement')['form']
        forms = {'schema_version': 'tos_human_form_set_v1', 'subject': form['subject'], 'forms': [form], 'prior_forms': []}
        materialize = lambda record=source, packet=forms, allowed=True: materialize_claim_forms(record, packet, access_allowed=allowed)[0]
        result = materialize()
        self.assertEqual(result['state'], 'ready')
        self.assertEqual(result['context'][0]['value'], source)
        self.assertEqual(result['display_text'], source['qualifiers']['statement'])
        self.assertIsNone(result['script'])  # No script guessed from Russian.
        self.assertFalse(result['standalone_reading'])
        self.assertIsNone(result['admission'])
        self.assertEqual(materialize(allowed=False)['state'], 'restricted')
        self.assertEqual(materialize({**source, 'visibility': 'local_only'})['state'], 'restricted')
        bad = copy.deepcopy(forms)
        del bad['forms'][0]['bindings']['context-0']
        self.assertEqual(materialize(packet=bad)['state'], 'invalid')
        bad = copy.deepcopy(forms)
        bad['forms'][0]['content'] = {'kind': 'freeform', 'text': 'An unconditional attribution.'}
        self.assertEqual(materialize(packet=bad)['state'], 'unavailable')
        changed = copy.deepcopy(source)
        changed['extensions']['unknown-context'][0] = True
        self.assertEqual(materialize(changed)['state'], 'stale')
        self.assertEqual(claim_field_catalog({**source, 'qualifiers': {}}), [])
        self.assertNotEqual(claim_forms_path(path, source['claim_id']), claim_forms_path(path, 'tos.claim.other'))
        self.assertLess(len(claim_forms_path(path, 'tos.claim.' + 'long' * 200).name), 255)
        for identifier in ('../../outside', 'tos.claim.invalid/segment', 'tos.claim.bad\n'):
            with self.assertRaises(ValueError):
                claim_forms_path(path, identifier)
        large = copy.deepcopy(source)
        large['extensions']['unknown-context'] = '界' * 30_000
        large_form = prepare_claim_change(large, None, 'software:test-only', 'tos.form.test.large', 'claim.statement')['form']
        large_set = {**forms, 'subject': large_form['subject'], 'forms': [large_form]}
        bounded = materialize(large, large_set)
        self.assertEqual(bounded['state'], 'over-budget')
        self.assertIsNone(bounded['display_text'])
        self.assertEqual(bounded['context'], [])

    def test_corpus_and_historical_language_schema_share_extensible_tags_and_reject_lossy_shapes(self):
        from jsonschema import Draft202012Validator
        from source_witness_bibliographic_graph_common import historical_schema_validator
        source, _ = self.metadata_forms_fixture()
        schema = json.loads((REPO_ROOT / 'ToS/contracts/corpus-record.schema.json').read_bytes())
        corpus_validator = Draft202012Validator(schema)
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            validator = historical_schema_validator(root)
            event = history[0][1]
            for language in ('ru', 'zh-Hant', 'x-private', 'i-enochian', 'abcdefgh-Latn', None):
                metadata = {'notes': {'language': language, 'script': 'Cyrl',
                                     'future_qualification': {'negative': False, 'unknown': None}}}
                corpus_validator.validate({**source, 'field_languages': metadata})
                validator.validate({**event, 'field_languages': metadata})
            for metadata in (None, {'notes': {'language': 'ru'}},
                             {'notes': {'language': 'ru\n', 'script': None}},
                             {'notes': {'language': 'ru', 'script': 'Cyrillic'}},
                             {'unowned': {'language': 'ru', 'script': None}}):
                with self.subTest(metadata=metadata):
                    self.assertFalse(corpus_validator.is_valid({**source, 'field_languages': metadata}))
                    self.assertFalse(validator.is_valid({**event, 'field_languages': metadata}))

    def test_real_metadata_forms_are_exact_source_bound_with_context(self):
        source, forms = self.metadata_forms_fixture()
        results = materialize_metadata_forms(source, forms, access_allowed=True)
        self.assertEqual([r['state'] for r in results], ['ready'] * 3)
        self.assertEqual([r['display_text'] for r in results],
                         [source['preferred_label'], source['variant_labels'][0]['value'], source['notes']])
        self.assertEqual([r['language'] for r in results], [None, 'ru', None])
        self.assertTrue(all(r['context'] and not r['standalone_reading'] for r in results))
        self.assertTrue(all(r['admission'] is None for r in results))
        self.assertIn('verified', [c['value'] for c in results[1]['context']])

    def test_generated_graph_carries_current_forms_without_mutating_the_subject(self):
        source, forms = self.metadata_forms_fixture()
        graph = self.load_projection()
        node = next(node for node in graph['nodes']
                    if node['properties'].get('identity_ref') == source['record_id'])
        self.assertEqual(node['properties']['source_record'], source)
        self.assertNotIn('human_forms', node['properties']['source_record'])
        self.assertEqual(node['properties']['human_forms'],
                         materialize_metadata_forms(source, forms, access_allowed=True))
        source_ref = node['properties']['human_forms_source_ref']
        self.assertEqual(graph['input_digests'][source_ref],
                         hashlib.sha256((REPO_ROOT / source_ref).read_bytes()).hexdigest())

    def test_metadata_adapter_refuses_missing_context_stale_source_and_freeform(self):
        source, forms = self.metadata_forms_fixture()
        del forms['forms'][0]['bindings']['identity_status']
        self.assertEqual(materialize_metadata_forms(source, forms, access_allowed=True)[0]['state'], 'invalid')
        forms['forms'][0]['content'] = {'kind': 'freeform', 'text': 'An unaudited summary.'}
        self.assertEqual(materialize_metadata_forms(source, forms, access_allowed=True)[0]['state'], 'unavailable')
        source['notes'] += ' changed'
        stale = materialize_metadata_forms(source, forms, access_allowed=True)
        self.assertTrue(all(form['state'] == 'stale' and form['display_text'] is None for form in stale))
        source['record_id'] = 'tos.work.unrelated'
        with self.assertRaisesRegex(ValueError, 'another source subject'):
            materialize_metadata_forms(source, forms, access_allowed=True)

    def test_metadata_forms_have_a_schema_and_a_whole_set_input_budget(self):
        source, forms = self.metadata_forms_fixture()
        forms['accepted'] = True
        with self.assertRaisesRegex(ValueError, 'schema'):
            materialize_metadata_forms(source, forms, access_allowed=True)
        del forms['accepted']
        forms['forms'][0]['content'] = {'kind': 'freeform', 'text': '界' * 710000}
        with self.assertRaisesRegex(ValueError, 'input budget'):
            materialize_metadata_forms(source, forms, access_allowed=True)

    def test_metadata_forms_cannot_self_authorize_access_or_overwrite_predecessors(self):
        source, forms = self.metadata_forms_fixture()
        results = materialize_metadata_forms(source, forms, access_allowed=False)
        self.assertTrue(all(r['state'] == 'restricted' and r['display_text'] is None for r in results))
        old = copy.deepcopy(forms['forms'][0])
        from knowledge_assessment import Record
        forms['forms'][0]['form_version'] = 2
        forms['forms'][0]['revises'] = Record.from_payload(old['form_id'], old['form_version'], old).ref
        self.assertEqual(materialize_metadata_forms(source, forms, access_allowed=True)[0]['state'], 'unavailable')
        forms['prior_forms'].append(old)
        self.assertEqual(materialize_metadata_forms(source, forms, access_allowed=True)[0]['state'], 'ready')

    def test_metadata_form_loader_is_adjacent_and_confined_to_source_home(self):
        source, forms = self.metadata_forms_fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            home = root / 'ToS/source-witnesses/works/example'
            home.mkdir(parents=True)
            path = home / 'work.json'
            path.write_text(json.dumps(source))
            ref = path.relative_to(root).as_posix()
            self.assertIsNone(load_metadata_forms(root, ref, source, access_allowed=True))
            (home / 'work.human-forms.json').write_text(json.dumps(forms))
            result = load_metadata_forms(root, ref, source, access_allowed=True)
            self.assertEqual(result[0], 'ToS/source-witnesses/works/example/work.human-forms.json')
            self.assertEqual(len(result[2]), 3)
            with self.assertRaises(ValueError):
                load_metadata_forms(root, 'outside.json', source, access_allowed=True)

    def load_projection(self) -> dict[str, object]:
        return json.loads(GRAPH_PATH.read_text(encoding="utf-8"))

    def test_generated_projection_matches_builder(self) -> None:
        self.assertEqual(
            GRAPH_PATH.read_text(encoding="utf-8"),
            render_payload(build_payload()),
        )

    def test_projection_is_claim_reified_and_complete(self) -> None:
        payload = self.load_projection()
        counts = payload["counts"]
        entries = _load_claim_catalog(REPO_ROOT)
        self.assertEqual({trace['claim_ref'] for trace in payload['claim_traces']},
                         {entry['claim_id'] for entry in entries})
        self.assertEqual(counts["source_claims"], len(entries))
        self.assertEqual(counts["claim_traces"], len(entries))
        self.assertEqual(counts["nodes"], len({node['node_id'] for node in payload['nodes']}))
        self.assertEqual(counts["edges"], len({edge['edge_id'] for edge in payload['edges']}))
        self.assertEqual(counts["direct_subject_object_edges"], 0)
        self.assertFalse(payload["relation_model"]["direct_subject_object_edges"])
        manifest = json.loads((REPO_ROOT / 'ToS/source-witnesses/catalog/catalog.manifest.json').read_bytes())
        historical = any(manifest['counts'].get(kind, 0) for kind in
                         ('historical-event', 'historical-process', 'historical-state'))
        registry = json.loads((REPO_ROOT / 'ToS/doctrine/semantic-interchange/entity-types.v1.json').read_bytes())
        declared_profile = any(manifest['counts'].get(entry['source_record_profile']['record_type'], 0)
                               for entry in registry['types']
                               if entry.get('source_record_profile', {}).get('graph_layer') == 'source-profile')
        declared_profile = declared_profile or any(Path(entry['source_claim_file_ref']).name == 'source-claims.jsonl'
                                                  for entry in entries)
        self.assertEqual(payload["graph_layers"], ['bibliographic', *(['historical'] if historical else []),
                         *(['source-profile'] if declared_profile else []),
                         *(['physical-artifact'] if manifest['counts'].get('artifact') else []),
                         *(['scholarly-composite'] if manifest['counts'].get('composite') else [])])
        self.assertEqual(payload["review_counts"], Counter(entry['review_status'] for entry in entries))
        self.assertEqual(payload["visibility_counts"], Counter(entry['visibility'] for entry in entries))
        self.assertEqual(
            payload["projection_fingerprint"],
            _projection_fingerprint(payload),
        )

    def test_final_validator_accepts_source_owned_historical_layer_but_no_invented_layer(self):
        import validate_source_witness_bibliographic_graph as validator
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            projection = rebuild()
            target = root / 'projection.json'
            target.write_text(render_payload(projection))
            with patch.object(validator, 'GRAPH_PATH', target), patch.object(
                    validator, 'build_payload', return_value=projection):
                self.assertEqual(validator.main(), 0)
                bad = copy.deepcopy(projection)
                bad['graph_layers'].append('invented-unowned-layer')
                target.write_text(render_payload(bad))
                with self.assertRaises((BibliographicGraphBuildError, SystemExit)):
                    validator.main()

    def test_every_edge_returns_to_claim_evidence_maker_event_and_review(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        traces = {trace["claim_ref"]: trace for trace in payload["claim_traces"]}
        claim_nodes = {
            node["properties"]["claim_ref"]: node
            for node in payload["nodes"]
            if node["node_kind"] == "claim"
        }
        self.assertEqual(set(traces), set(claim_nodes))

        for edge in payload["edges"]:
            trace = traces[edge["claim_ref"]]
            self.assertEqual(edge["from_id"], trace["claim_node_id"])
            self.assertEqual(edge["claim_sha256"], trace["claim_sha256"])
            self.assertEqual(edge["evidence_node_ids"], trace["evidence_node_ids"])
            self.assertEqual(edge["maker_node_id"], trace["maker_node_id"])
            self.assertEqual(
                edge["provenance_event_node_id"],
                trace["provenance_event_node_id"],
            )
            self.assertEqual(edge["review_status"], trace["review_status"])
            self.assertIn(edge["to_id"], nodes)
            self.assertTrue(edge["evidence_node_ids"])
            self.assertTrue(
                all(nodes[node_id]["node_kind"] == "evidence" for node_id in edge["evidence_node_ids"])
            )

        for trace in traces.values():
            event = nodes[trace["provenance_event_node_id"]]
            maker = nodes[trace["maker_node_id"]]
            self.assertEqual(event["node_kind"], "provenance_event")
            self.assertTrue(event["properties"]["started_at"])
            self.assertTrue(event["properties"]["ended_at"])
            method = event['properties']['method']
            procedure = method['procedure'] if event['properties'].get('schema_version') == 'tos_provenance_event_v2' else method
            self.assertTrue(procedure['name'])
            self.assertEqual(maker["node_kind"], "maker")
            self.assertTrue(maker["properties"]["agent_ref"])

    def test_claim_source_return_uses_independent_canonical_digest(self) -> None:
        payload = self.load_projection()
        for trace in payload["claim_traces"]:
            source_path = REPO_ROOT / trace["source_claim_file_ref"]
            raw_line = source_path.read_text(encoding="utf-8").splitlines()[
                trace["source_claim_line"] - 1
            ]
            source_claim = json.loads(raw_line)
            canonical = json.dumps(
                source_claim,
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            )
            digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
            self.assertEqual(source_claim["claim_id"], trace["claim_ref"])
            self.assertEqual(digest, trace["source_claim_sha256"])
            self.assertEqual(digest, trace["claim_sha256"])

    def test_structured_source_fields_survive_claim_projection_losslessly(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        trace = payload["claim_traces"][0]

        claim_node = nodes[trace["claim_node_id"]]
        claim_lines = (REPO_ROOT / claim_node["source_ref"]).read_text(
            encoding="utf-8"
        ).splitlines()
        source_claim = json.loads(claim_lines[claim_node["source_line"] - 1])
        self.assertEqual(claim_node["properties"]["source_claim"], source_claim)

        identity_node = nodes[trace["subject_node_id"]]
        source_record = json.loads(
            (REPO_ROOT / identity_node["source_ref"]).read_text(encoding="utf-8")
        )
        self.assertEqual(identity_node["properties"]["source_record"], source_record)
        self.assertEqual(
            identity_node["properties"]["record_version"],
            source_record["record_version"],
        )

        event_node = nodes[trace["provenance_event_node_id"]]
        event_lines = (REPO_ROOT / event_node["source_ref"]).read_text(
            encoding="utf-8"
        ).splitlines()
        source_event = json.loads(event_lines[event_node["source_line"] - 1])
        self.assertEqual(event_node["properties"]["source_event"], source_event)

        anchor_node = next(
            node
            for node in payload["nodes"]
            if node["node_kind"] == "evidence"
            and node["properties"].get("evidence_kind") == "anchor"
        )
        anchor_lines = (REPO_ROOT / anchor_node["source_ref"]).read_text(
            encoding="utf-8"
        ).splitlines()
        source_anchor = json.loads(anchor_lines[anchor_node["source_line"] - 1])
        self.assertEqual(anchor_node["properties"]["source_anchor"], source_anchor)

    def test_literal_objects_remain_literals_not_false_identities(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        traces = {trace["claim_ref"]: trace for trace in payload["claim_traces"]}
        issue_trace = traces[
            "tos.claim.edition.der-fall-wagner.naumann-1888.nominal-later-issue-state"
        ]
        issue_object = nodes[issue_trace["object_node_id"]]
        self.assertEqual(issue_object["node_kind"], "literal")
        self.assertEqual(
            issue_object["properties"]["value"]["textual_identity_status"],
            "unresolved",
        )
        self.assertEqual(
            issue_object["properties"]["value"]["textual_difference_status"],
            "unresolved",
        )
        direct_assertions = [
            edge
            for edge in payload["edges"]
            if nodes[edge["from_id"]]["node_kind"] != "claim"
        ]
        self.assertEqual(direct_assertions, [])

    def test_projection_contains_no_local_payload_route(self) -> None:
        payload = self.load_projection()
        serialized = json.dumps(payload, ensure_ascii=False)
        self.assertNotIn("/srv/", serialized)
        self.assertNotIn("/home/", serialized)
        for node in payload["nodes"]:
            self.assertNotIn("/payload/", node["source_ref"])

    def test_cross_reference_guard_rejects_direct_subject_object_edge(self) -> None:
        payload = build_payload()
        mutated = copy.deepcopy(payload)
        first_trace = mutated["claim_traces"][0]
        first_edge = next(
            edge
            for edge in mutated["edges"]
            if edge["claim_ref"] == first_trace["claim_ref"]
        )
        first_edge["from_id"] = first_trace["subject_node_id"]
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "every edge must start at its reified claim node",
        ):
            _validate_cross_references(mutated)

    def test_cross_reference_guard_closes_normalized_provision_routes(self) -> None:
        payload = build_payload()
        provision_trace = next(
            trace
            for trace in payload["claim_traces"]
            if trace["predicate"] == "provision_activity"
        )

        wrong_kind = copy.deepcopy(payload)
        place_edge = next(
            edge
            for edge in wrong_kind["edges"]
            if edge["claim_ref"] == provision_trace["claim_ref"]
            and edge["edge_kind"] == "has_normalized_place"
        )
        place_edge["to_id"] = provision_trace["subject_node_id"]
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "normalized place route must end at a Place identity",
        ):
            _validate_cross_references(wrong_kind)

        missing_trace_ref = copy.deepcopy(payload)
        mutated_trace = next(
            trace
            for trace in missing_trace_ref["claim_traces"]
            if trace["claim_ref"] == provision_trace["claim_ref"]
        )
        mutated_trace["normalized_identity_node_ids"] = []
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "normalized identity trace differs from normalized edges",
        ):
            _validate_cross_references(missing_trace_ref)

    def test_catalog_loader_rejects_nonpublic_claim(self) -> None:
        source_entry = json.loads(
            (REPO_ROOT / CLAIM_CATALOG_REF).read_text(encoding="utf-8").splitlines()[0]
        )
        source_entry["visibility"] = "local_only"
        with tempfile.TemporaryDirectory() as temporary:
            temp_root = Path(temporary)
            claim_path = temp_root / CLAIM_CATALOG_REF
            claim_path.parent.mkdir(parents=True)
            claim_path.write_text(
                json.dumps(source_entry, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                BibliographicGraphBuildError,
                "visibility is not safe",
            ):
                _load_claim_catalog(temp_root)

    def test_catalog_loader_excludes_object_link_claims_from_bibliographic_graph(self) -> None:
        source_entry = next(
            json.loads(line)
            for line in (REPO_ROOT / CLAIM_CATALOG_REF).read_text(encoding="utf-8").splitlines()
            if "/relations/object-link/" in line
        )
        with tempfile.TemporaryDirectory() as temporary:
            temp_root = Path(temporary)
            claim_path = temp_root / CLAIM_CATALOG_REF
            claim_path.parent.mkdir(parents=True)
            claim_path.write_text(
                json.dumps(source_entry, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            self.assertEqual(_load_claim_catalog(temp_root), [])

            source_entry["source_claim_file_ref"] = (
                "ToS/source-witnesses/relations/unexpected/relation-claims.jsonl"
            )
            claim_path.write_text(
                json.dumps(source_entry, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                BibliographicGraphBuildError,
                "outside the bounded Expression-derivation profile",
            ):
                _load_claim_catalog(temp_root)

    def test_exact_claim_query_returns_complete_source_bundle(self) -> None:
        payload = load_verified_projection()
        claim_ref = (
            "tos.claim.edition.ecce-homo.insel-1908.edited-by-raoul-richter"
        )
        result = query_projection(payload, claim_ref=claim_ref)
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["result_count"], 1)
        match = result["matches"][0]
        self.assertEqual(match["claim_ref"], claim_ref)
        self.assertEqual(
            match["source_return"]["source_claim"]["claim_id"],
            claim_ref,
        )
        self.assertEqual(
            match["source_return"]["canonical_sha256"],
            match["claim_sha256"],
        )
        self.assertEqual(match["subject_node"]["node_kind"], "identity")
        self.assertEqual(match["object_node"]["node_kind"], "identity")
        self.assertTrue(match["evidence_nodes"])
        self.assertEqual(match["maker_node"]["node_kind"], "maker")
        self.assertEqual(
            match["provenance_event_node"]["node_kind"],
            "provenance_event",
        )
        self.assertEqual(match["review_nodes"], [])
        self.assertTrue(match["edges"])

    def test_query_uses_exact_and_semantics(self) -> None:
        payload = load_verified_projection()
        subject_ref = (
            "tos.collection.friedrich-nietzsche."
            "works-in-two-volumes-volume-2-mysl-1996"
        )
        result = query_projection(
            payload,
            subject_ref=subject_ref,
            predicate="contains_work",
        )
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["result_count"], 7)
        self.assertEqual(
            [match["claim_ref"] for match in result["matches"]],
            sorted(match["claim_ref"] for match in result["matches"]),
        )
        for match in result["matches"]:
            self.assertEqual(match["predicate"], "contains_work")
            self.assertEqual(
                match["subject_node"]["properties"]["identity_ref"],
                subject_ref,
            )

    def test_first_publication_chronology_remains_claim_scoped_literal(self) -> None:
        payload = load_verified_projection()
        result = query_projection(
            payload,
            predicate="first_publication_chronology",
        )
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["result_count"], 7)
        self.assertTrue(
            all(match["object_node"]["node_kind"] == "literal" for match in result["matches"])
        )
        self.assertTrue(
            all(
                match["object_node"]["properties"]["value"]["ordering_warning"]
                for match in result["matches"]
            )
        )
        self.assertTrue(
            all(
                match["source_return"]["file_ref"].endswith(
                    "/work-chronology-claims.jsonl"
                )
                for match in result["matches"]
            )
        )

    def test_provision_activity_query_preserves_literal_and_normalized_routes(
        self,
    ) -> None:
        payload = load_verified_projection()
        leipzig_ref = "tos.place.leipzig"
        result = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref=leipzig_ref,
        )
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["result_count"], 9)
        self.assertEqual(
            {
                "tos.organization.c-g-naumann-verlag-leipzig",
                "tos.organization.druckerei-c-g-naumann-leipzig",
                "tos.organization.insel-verlag-anton-kippenberg-leipzig",
            },
            {
                node["properties"]["identity_ref"]
                for match in result["matches"]
                for node in match["normalized_identity_nodes"]
                if node["properties"]["identity_kind"] == "organization"
            },
        )
        for match in result["matches"]:
            self.assertEqual(match["object_node"]["node_kind"], "literal")
            self.assertEqual(
                match["object_node"]["properties"]["value"]["temporal"]["role"],
                "statement_date",
            )
            self.assertIn(
                "has_normalized_place",
                {edge["edge_kind"] for edge in match["edges"]},
            )
            self.assertTrue(
                all(
                    edge["from_id"] == match["claim_node"]["node_id"]
                    for edge in match["edges"]
                )
            )

        modern_successor = query_projection(
            payload,
            normalized_ref="tos.organization.insel-verlag-berlin",
        )
        self.assertEqual(modern_successor["status"], "no_match")
        self.assertEqual(modern_successor["matches"], [])

    def test_zarathustra_parts_1_to_4_provision_queries_remain_distinct(
        self,
    ) -> None:
        payload = load_verified_projection()
        part_1_ref = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "chemnitz-schmeitzner-1883-part-1"
        )
        part_2_ref = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "chemnitz-schmeitzner-1883-part-2"
        )
        part_3_ref = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "chemnitz-schmeitzner-1884-part-3"
        )
        part_4_ref = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "leipzig-naumann-1891-part-4"
        )
        organization_ref = (
            "tos.organization.ernst-schmeitzner-verlagsbuchhandlung-chemnitz"
        )

        by_place = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref="tos.place.chemnitz",
        )
        self.assertEqual(by_place["result_count"], 3)
        self.assertEqual(
            {part_1_ref, part_2_ref, part_3_ref},
            {
                match["subject_node"]["properties"]["identity_ref"]
                for match in by_place["matches"]
            },
        )
        self.assertEqual(3, len({match["claim_ref"] for match in by_place["matches"]}))
        for match in by_place["matches"]:
            self.assertEqual("literal", match["object_node"]["node_kind"])
            self.assertEqual(
                "authority_record",
                match["object_node"]["properties"]["value"]["statement_basis"],
            )
            self.assertEqual(
                {"tos.place.chemnitz", organization_ref},
                {
                    node["properties"]["identity_ref"]
                    for node in match["normalized_identity_nodes"]
                },
            )
            self.assertTrue(
                all(
                    edge["from_id"] == match["claim_node"]["node_id"]
                    for edge in match["edges"]
                )
            )

        by_organization = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref=organization_ref,
        )
        self.assertEqual(
            {match["claim_ref"] for match in by_place["matches"]},
            {match["claim_ref"] for match in by_organization["matches"]},
        )
        expected_years = {
            part_1_ref: "1883",
            part_2_ref: "1883",
            part_3_ref: "1884",
        }
        exact_claim_refs = set()
        for subject_ref, year in expected_years.items():
            exact = query_projection(
                payload,
                subject_ref=subject_ref,
                predicate="provision_activity",
            )
            self.assertEqual("ok", exact["status"])
            self.assertEqual(1, exact["result_count"])
            exact_claim_refs.add(exact["matches"][0]["claim_ref"])
            self.assertEqual(
                year,
                exact["matches"][0]["object_node"]["properties"]["value"][
                    "temporal"
                ]["value"],
            )
        self.assertEqual(3, len(exact_claim_refs))

        part_4 = query_projection(
            payload,
            subject_ref=part_4_ref,
            predicate="provision_activity",
        )
        self.assertEqual("ok", part_4["status"])
        self.assertEqual(1, part_4["result_count"])
        part_4_match = part_4["matches"][0]
        self.assertNotIn(part_4_match["claim_ref"], exact_claim_refs)
        self.assertEqual(
            "1891",
            part_4_match["object_node"]["properties"]["value"]["temporal"]["value"],
        )
        self.assertEqual(
            {"tos.place.leipzig", "tos.organization.c-g-naumann-verlag-leipzig"},
            {
                node["properties"]["identity_ref"]
                for node in part_4_match["normalized_identity_nodes"]
            },
        )
        self.assertEqual(4, len(exact_claim_refs | {part_4_match["claim_ref"]}))

        person_gnd = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref="118823698",
        )
        self.assertEqual("no_match", person_gnd["status"])
        self.assertEqual([], person_gnd["matches"])

    def test_antonovsky_1913_provision_query_separates_publisher_and_printer(
        self,
    ) -> None:
        payload = load_verified_projection()
        edition_ref = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "saint-petersburg-zhizn-dlya-vsekh-1913"
        )
        result = query_projection(
            payload,
            subject_ref=edition_ref,
            predicate="provision_activity",
        )
        self.assertEqual("ok", result["status"])
        self.assertEqual(2, result["result_count"])
        self.assertEqual(
            {"publication", "manufacture"},
            {
                match["object_node"]["properties"]["value"]["provision_kind"]
                for match in result["matches"]
            },
        )
        self.assertEqual(
            {
                "tos.organization.zhizn-dlya-vsekh-saint-petersburg",
                "tos.organization.bratya-v-i-i-linnik-printing-saint-petersburg",
            },
            {
                node["properties"]["identity_ref"]
                for match in result["matches"]
                for node in match["normalized_identity_nodes"]
                if node["properties"]["identity_kind"] == "organization"
            },
        )
        self.assertEqual(
            {"tos.place.saint-petersburg"},
            {
                node["properties"]["identity_ref"]
                for match in result["matches"]
                for node in match["normalized_identity_nodes"]
                if node["properties"]["identity_kind"] == "place"
            },
        )
        for match in result["matches"]:
            self.assertEqual("literal", match["object_node"]["node_kind"])
            self.assertTrue(
                all(
                    edge["from_id"] == match["claim_node"]["node_id"]
                    for edge in match["edges"]
                )
            )
            self.assertIn(
                "has_normalized_place",
                {edge["edge_kind"] for edge in match["edges"]},
            )
            self.assertIn(
                "has_normalized_agent",
                {edge["edge_kind"] for edge in match["edges"]},
            )

        posse = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref="tos.agent.vladimir-posse",
        )
        self.assertEqual("no_match", posse["status"])
        self.assertEqual([], posse["matches"])

    def test_naumann_1893_provision_query_separates_publisher_and_printer(
        self,
    ) -> None:
        payload = load_verified_projection()
        edition_ref = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "leipzig-c-g-naumann-1893"
        )
        result = query_projection(
            payload,
            subject_ref=edition_ref,
            predicate="provision_activity",
        )
        self.assertEqual("ok", result["status"])
        self.assertEqual(2, result["result_count"])
        self.assertEqual(
            {"publication", "manufacture"},
            {
                match["object_node"]["properties"]["value"]["provision_kind"]
                for match in result["matches"]
            },
        )
        self.assertEqual(
            {
                "tos.organization.c-g-naumann-verlag-leipzig",
                "tos.organization.druckerei-c-g-naumann-leipzig",
            },
            {
                node["properties"]["identity_ref"]
                for match in result["matches"]
                for node in match["normalized_identity_nodes"]
                if node["properties"]["identity_kind"] == "organization"
            },
        )
        for match in result["matches"]:
            self.assertEqual("literal", match["object_node"]["node_kind"])
            self.assertTrue(
                all(
                    edge["from_id"] == match["claim_node"]["node_id"]
                    for edge in match["edges"]
                )
            )

        printer = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref="tos.organization.druckerei-c-g-naumann-leipzig",
        )
        self.assertEqual("ok", printer["status"])
        self.assertEqual(3, printer["result_count"])
        self.assertTrue(
            all(
                match["object_node"]["properties"]["value"]["provision_kind"]
                == "manufacture"
                for match in printer["matches"]
            )
        )

    def test_jenseits_1886_provision_query_preserves_shared_literal_and_roles(
        self,
    ) -> None:
        payload = load_verified_projection()
        edition_ref = (
            "tos.edition.friedrich-nietzsche.jenseits-von-gut-und-boese."
            "leipzig-c-g-naumann-1886"
        )
        result = query_projection(
            payload,
            subject_ref=edition_ref,
            predicate="provision_activity",
        )
        self.assertEqual("ok", result["status"])
        self.assertEqual(2, result["result_count"])
        self.assertEqual(
            {"publication", "manufacture"},
            {
                match["object_node"]["properties"]["value"]["provision_kind"]
                for match in result["matches"]
            },
        )
        self.assertEqual(
            {"Leipzig / Druck und Verlag von C. G. Naumann. / 1886."},
            {
                match["object_node"]["properties"]["value"][
                    "transcribed_statement"
                ]
                for match in result["matches"]
            },
        )
        self.assertEqual(
            {
                "tos.organization.c-g-naumann-verlag-leipzig",
                "tos.organization.druckerei-c-g-naumann-leipzig",
            },
            {
                node["properties"]["identity_ref"]
                for match in result["matches"]
                for node in match["normalized_identity_nodes"]
                if node["properties"]["identity_kind"] == "organization"
            },
        )
        self.assertTrue(
            all(
                match["source_return"]["file_ref"].endswith(
                    "/provision-activity-claims.jsonl"
                )
                and all(
                    edge["from_id"] == match["claim_node"]["node_id"]
                    for edge in match["edges"]
                )
                for match in result["matches"]
            )
        )

        publisher = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref="tos.organization.c-g-naumann-verlag-leipzig",
        )
        self.assertEqual("ok", publisher["status"])
        self.assertEqual(5, publisher["result_count"])
        self.assertIn(
            edition_ref,
            {
                match["subject_node"]["properties"]["identity_ref"]
                for match in publisher["matches"]
            },
        )

    def test_genealogie_1892_provision_query_preserves_page_split_and_roles(
        self,
    ) -> None:
        payload = load_verified_projection()
        edition_ref = (
            "tos.edition.friedrich-nietzsche.zur-genealogie-der-moral."
            "leipzig-c-g-naumann-1892-second"
        )
        result = query_projection(
            payload,
            subject_ref=edition_ref,
            predicate="provision_activity",
        )
        self.assertEqual("ok", result["status"])
        self.assertEqual(2, result["result_count"])
        self.assertEqual(
            {"publication", "manufacture"},
            {
                match["object_node"]["properties"]["value"]["provision_kind"]
                for match in result["matches"]
            },
        )
        self.assertEqual(
            {
                "LEIPZIG / Verlag von C. G. Naumann. / 1892.",
                "LEIPZIG / Druck von C. G. Naumann.",
            },
            {
                match["object_node"]["properties"]["value"][
                    "transcribed_statement"
                ]
                for match in result["matches"]
            },
        )
        self.assertEqual(
            {
                "tos.organization.c-g-naumann-verlag-leipzig",
                "tos.organization.druckerei-c-g-naumann-leipzig",
            },
            {
                node["properties"]["identity_ref"]
                for match in result["matches"]
                for node in match["normalized_identity_nodes"]
                if node["properties"]["identity_kind"] == "organization"
            },
        )
        self.assertTrue(
            all(
                match["source_return"]["file_ref"].endswith(
                    "/provision-activity-claims.jsonl"
                )
                and all(
                    edge["from_id"] == match["claim_node"]["node_id"]
                    for edge in match["edges"]
                )
                for match in result["matches"]
            )
        )

        printer = query_projection(
            payload,
            predicate="provision_activity",
            normalized_ref="tos.organization.druckerei-c-g-naumann-leipzig",
        )
        self.assertEqual("ok", printer["status"])
        self.assertEqual(3, printer["result_count"])
        self.assertIn(
            edition_ref,
            {
                match["subject_node"]["properties"]["identity_ref"]
                for match in printer["matches"]
            },
        )

    def test_antonovsky_translation_queries_preserve_expression_identity(
        self,
    ) -> None:
        payload = load_verified_projection()
        agent_ref = "tos.agent.yuri-antonovsky"
        expression_1911 = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-antonovsky-1911"
        )
        expression_1913 = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-antonovsky-1913"
        )
        expression_1996 = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-antonovsky-mysl-1996"
        )
        expression_2007 = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-antonovsky-cultural-revolution"
        )

        result_1911 = query_projection(
            payload,
            subject_ref=expression_1911,
            object_ref=agent_ref,
            predicate="translated_by",
        )
        result_1913 = query_projection(
            payload,
            subject_ref=expression_1913,
            object_ref=agent_ref,
            predicate="translated_by",
        )
        result_1996 = query_projection(
            payload,
            subject_ref=expression_1996,
            object_ref=agent_ref,
            predicate="translated_by",
        )
        result_2007 = query_projection(
            payload,
            subject_ref=expression_2007,
            object_ref=agent_ref,
            predicate="translated_by",
        )
        self.assertEqual(1, result_1911["result_count"])
        self.assertEqual(1, result_1913["result_count"])
        self.assertEqual(1, result_1996["result_count"])
        self.assertEqual(1, result_2007["result_count"])
        self.assertNotEqual(
            result_1913["matches"][0]["claim_ref"],
            result_1996["matches"][0]["claim_ref"],
        )
        self.assertEqual(
            {
                result_1911["matches"][0]["claim_ref"],
                result_1913["matches"][0]["claim_ref"],
                result_1996["matches"][0]["claim_ref"],
                result_2007["matches"][0]["claim_ref"],
            },
            {
                "tos.claim.expression.also-sprach-zarathustra.ru-antonovsky-1911.translated-by-yuri-antonovsky",
                "tos.claim.expression.also-sprach-zarathustra.ru-antonovsky-1913.translated-by-yuri-antonovsky",
                "tos.claim.expression.mysl-1996-volume-2.also-sprach-zarathustra.translated-by-yuri-antonovsky",
                "tos.claim.expression.also-sprach-zarathustra.ru-antonovsky-cultural-revolution-2007.translated-by-yuri-antonovsky",
            },
        )
        self.assertEqual(
            "tos.anchor.also-sprach-zarathustra.ru-antonovsky-1913."
            "title-page-translator-credit",
            next(
                node["properties"]["evidence_ref"]
                for node in result_1913["matches"][0]["evidence_nodes"]
                if node["properties"]["evidence_ref"].startswith("tos.anchor.")
            ),
        )
        self.assertTrue(
            all(
                match["source_return"]["source_claim"]["review_status"]
                == "unreviewed"
                for result in (result_1911, result_1913, result_1996, result_2007)
                for match in result["matches"]
            )
        )

        agent_result = query_projection(
            payload,
            object_ref=agent_ref,
            predicate="translated_by",
        )
        self.assertEqual(5, agent_result["result_count"])

    def test_foundation_topology_queries_return_all_three_relation_families(self) -> None:
        payload = load_verified_projection()
        work_ref = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        work_result = query_projection(
            payload,
            subject_ref=work_ref,
            predicate="has_expression",
        )
        self.assertEqual(work_result["result_count"], 14)
        self.assertTrue(
            all(
                match["source_return"]["file_ref"].endswith(
                    "/work-expression-claims.jsonl"
                )
                for match in work_result["matches"]
            )
        )

        expression_ref = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-antonovsky-mysl-1996"
        )
        edition_ref = (
            "tos.edition.friedrich-nietzsche.works-in-two-volumes."
            "moscow-mysl-1996-volume-2"
        )
        embodiment_result = query_projection(
            payload,
            subject_ref=expression_ref,
            object_ref=edition_ref,
            predicate="embodied_by",
        )
        self.assertEqual(embodiment_result["result_count"], 1)
        embodiment = embodiment_result["matches"][0]
        self.assertEqual(
            embodiment["claim_node"]["properties"]["assertion_layer"],
            "bibliographic_assertion",
        )
        self.assertEqual(
            embodiment["claim_node"]["properties"]["review_status"],
            "unreviewed",
        )
        self.assertEqual(
            embodiment["source_return"]["source_claim"]["predicate"],
            "embodied_by",
        )

        edition_with_two_items = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "leipzig-c-g-naumann-1893"
        )
        exemplar_result = query_projection(
            payload,
            subject_ref=edition_with_two_items,
            predicate="exemplified_by",
        )
        self.assertEqual(exemplar_result["result_count"], 2)
        self.assertTrue(
            all(len(match["evidence_nodes"]) == 3 for match in exemplar_result["matches"])
        )

    def test_embodiment_topology_does_not_assert_textual_equivalence(self) -> None:
        payload = load_verified_projection()
        result = query_projection(payload, predicate="embodied_by", limit=28)
        self.assertEqual(result["result_count"], 28)
        for match in result["matches"]:
            source_claim = match["source_return"]["source_claim"]
            self.assertEqual(source_claim["claim_type"], "bibliographic")
            self.assertEqual(source_claim["epistemic_status"], "observed")
            self.assertNotIn("same_as", source_claim["predicate"])
            self.assertNotIn("textual", json.dumps(source_claim, ensure_ascii=False))

        expression_1907 = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-antonovsky-1907"
        )
        edition_1907 = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "saint-petersburg-vaisberg-gershunin-typography-1907-third"
        )
        exact = query_projection(
            payload,
            subject_ref=expression_1907,
            object_ref=edition_1907,
            predicate="embodied_by",
        )
        self.assertEqual(exact["result_count"], 1)
        source_claim = exact["matches"][0]["source_return"]["source_claim"]
        self.assertEqual(source_claim["review_status"], "unreviewed")
        self.assertEqual(source_claim["visibility"], "public_metadata_only")
        self.assertEqual(source_claim["object"], edition_1907)

    def test_reader_1899_queries_preserve_positive_topology_and_negative_authorship(
        self,
    ) -> None:
        payload = load_verified_projection()
        work_ref = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        expression_ref = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-reader-1899-uncredited"
        )
        edition_ref = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "moscow-reader-editorial-office-1899"
        )
        item_ref = (
            "tos.item.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-reader-1899-uncredited.rnl-rusneb-fragment-pdf-parts"
        )

        work_expression = query_projection(
            payload,
            subject_ref=work_ref,
            object_ref=expression_ref,
            predicate="has_expression",
        )
        self.assertEqual(1, work_expression["result_count"])
        expression_edition = query_projection(
            payload,
            subject_ref=expression_ref,
            object_ref=edition_ref,
            predicate="embodied_by",
        )
        self.assertEqual(1, expression_edition["result_count"])
        edition_item = query_projection(
            payload,
            subject_ref=edition_ref,
            object_ref=item_ref,
            predicate="exemplified_by",
        )
        self.assertEqual(1, edition_item["result_count"])

        translated_by = query_projection(
            payload,
            subject_ref=expression_ref,
            predicate="translated_by",
        )
        self.assertEqual("no_match", translated_by["status"])
        self.assertEqual(0, translated_by["result_count"])
        derivation = query_projection(
            payload,
            subject_ref=expression_ref,
            predicate="is_derivative_of",
        )
        self.assertEqual("no_match", derivation["status"])
        self.assertEqual(0, derivation["result_count"])

    def test_nani_1899_queries_preserve_topology_responsibility_and_negative_derivation(
        self,
    ) -> None:
        payload = load_verified_projection()
        work_ref = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        expression_ref = (
            "tos.expression.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-nani-1899-nine-fragments"
        )
        edition_ref = (
            "tos.edition.friedrich-nietzsche.also-sprach-zarathustra."
            "saint-petersburg-stasyulevich-1899-nine-fragments"
        )
        item_ref = (
            "tos.item.friedrich-nietzsche.also-sprach-zarathustra."
            "ru-nani-1899-nine-fragments.rsl-rusneb-parallel-scan-pdf"
        )
        agent_ref = "tos.agent.s-p-nani"
        printer_ref = (
            "tos.organization.m-m-stasyulevich-printing-saint-petersburg"
        )

        for subject_ref, object_ref, predicate in (
            (work_ref, expression_ref, "has_expression"),
            (expression_ref, edition_ref, "embodied_by"),
            (edition_ref, item_ref, "exemplified_by"),
            (expression_ref, agent_ref, "translated_by"),
        ):
            result = query_projection(
                payload,
                subject_ref=subject_ref,
                object_ref=object_ref,
                predicate=predicate,
            )
            self.assertEqual(1, result["result_count"])
            claim_properties = result["matches"][0]["claim_node"]["properties"]
            self.assertEqual("unreviewed", claim_properties["review_status"])
            self.assertEqual(
                "public_metadata_only",
                claim_properties["visibility"],
            )

        manufacture = query_projection(
            payload,
            subject_ref=edition_ref,
            predicate="provision_activity",
            normalized_ref=printer_ref,
        )
        self.assertEqual(1, manufacture["result_count"])
        source_claim = manufacture["matches"][0]["source_return"]["source_claim"]
        self.assertEqual("manufacture", source_claim["object"]["provision_kind"])
        self.assertEqual("printer", source_claim["object"]["agents"][0]["role"])

        derivation = query_projection(
            payload,
            subject_ref=expression_ref,
            predicate="is_derivative_of",
        )
        self.assertEqual("no_match", derivation["status"])
        self.assertEqual(0, derivation["result_count"])

        same_as = query_projection(
            payload,
            subject_ref=expression_ref,
            predicate="same_as",
        )
        self.assertEqual("no_match", same_as["status"])
        self.assertEqual(0, same_as["result_count"])

    def test_expression_derivation_queries_preserve_direction_and_absent_edges(self) -> None:
        payload = load_verified_projection()
        result = query_projection(payload, predicate="is_derivative_of")
        self.assertEqual(result["result_count"], 2)
        pairs = {
            (
                match["source_return"]["source_claim"]["subject_ref"],
                match["source_return"]["source_claim"]["object"],
            )
            for match in result["matches"]
        }
        self.assertEqual(
            pairs,
            {
                (
                    "tos.expression.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1903",
                    "tos.expression.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1900",
                ),
                (
                    "tos.expression.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1907",
                    "tos.expression.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1903",
                ),
            },
        )
        for match in result["matches"]:
            source_claim = match["source_return"]["source_claim"]
            self.assertEqual(source_claim["claim_type"], "relation")
            self.assertEqual(source_claim["review_status"], "unreviewed")
            self.assertEqual(source_claim["qualifiers"]["derivation_kind"], "revision")
            self.assertFalse(source_claim["qualifiers"]["transitive"])
            self.assertFalse(source_claim["qualifiers"]["equivalence_inferred"])
            self.assertEqual(
                match["claim_node"]["properties"]["qualifiers"],
                source_claim["qualifiers"],
            )

        unsupported_pairs = {
            (
                "tos.expression.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1911",
                "tos.expression.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1907",
            ),
            (
                "tos.expression.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-cultural-revolution",
                "tos.expression.friedrich-nietzsche.also-sprach-zarathustra.ru-antonovsky-1911",
            ),
        }
        self.assertTrue(pairs.isdisjoint(unsupported_pairs))

    def test_query_no_match_is_explicit_and_deterministic(self) -> None:
        payload = load_verified_projection()
        first = query_projection(payload, claim_ref="tos.claim.missing")
        second = query_projection(payload, claim_ref="tos.claim.missing")
        self.assertEqual(first, second)
        self.assertEqual(first["status"], "no_match")
        self.assertEqual(first["result_count"], 0)
        self.assertEqual(first["matches"], [])

    def test_query_requires_selector_and_rejects_silent_truncation(self) -> None:
        payload = load_verified_projection()
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "at least one exact query selector",
        ):
            query_projection(payload)
        with self.assertRaisesRegex(
            BibliographicGraphBuildError,
            "exceeding explicit limit 20",
        ):
            query_projection(payload, review_status="unreviewed")

    def test_verified_loader_rejects_projection_fingerprint_drift(self) -> None:
        payload = self.load_projection()
        payload["claim_traces"][0]["predicate"] = "tampered_predicate"
        with tempfile.TemporaryDirectory() as temporary:
            graph_path = Path(temporary) / "graph.json"
            graph_path.write_text(
                json.dumps(payload, ensure_ascii=False),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                BibliographicGraphBuildError,
                "projection fingerprint does not match",
            ):
                load_verified_projection(graph_path=graph_path)

    def test_query_cli_emits_json_and_rejects_unbounded_dump(self) -> None:
        script = REPO_ROOT / "scripts" / "query_source_witness_bibliographic_graph.py"
        claim_ref = (
            "tos.claim.edition.der-fall-wagner.naumann-1888."
            "nominal-later-issue-state"
        )
        completed = subprocess.run(
            [sys.executable, str(script), "--claim-ref", claim_ref],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        result = json.loads(completed.stdout)
        self.assertEqual(result["result_count"], 1)
        self.assertEqual(result["matches"][0]["claim_ref"], claim_ref)
        self.assertNotIn("/srv/", completed.stdout)
        self.assertNotIn("/home/", completed.stdout)

        rejected = subprocess.run(
            [sys.executable, str(script)],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(rejected.returncode, 2)
        self.assertEqual(rejected.stdout, "")
        self.assertIn("at least one exact query selector", rejected.stderr)


if __name__ == "__main__":
    unittest.main()

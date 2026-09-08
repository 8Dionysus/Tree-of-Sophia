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
    load_verified_projection,
    query_projection,
    render_payload,
)
from source_witness_human_forms import load_metadata_forms, materialize_metadata_forms


class SourceWitnessBibliographicGraphTest(unittest.TestCase):
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

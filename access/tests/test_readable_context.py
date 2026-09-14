"""Readable context preserves governing source data; coverage is not judgment."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'access/src'))
sys.path.insert(0, str(ROOT / 'scripts'))
from tos_access.knowledge import (_normalize_node, _stable_digest, knowledge_catalog,
    validate_semantic_registries, build_knowledge_graph, _lens_carrier, _attach_readable_context)
from tos_access.readable_context import (ReadableContextCompiler, ReadableContextError,
    build_readable_context, presentation_catalog, validate_sidecar, validate_vocabulary, vocabulary_digest)
from source_witness_human_forms import materialize_metadata_forms
from fixture_support import canonical_node_fixture, knowledge_fixture_path


def real_freedom():
    path = knowledge_fixture_path(
        'ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.json')
    record = json.loads(path.read_text())
    forms = json.loads(knowledge_fixture_path(
        'ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.human-forms.json')
        .read_text())
    return _normalize_node({'node_id': 'identity:' + record['record_id'], 'node_type': record['record_type'],
        'properties': {'source_record': record,
                       'human_forms': materialize_metadata_forms(record, forms, access_allowed=True)}}, 'source-navigation')


def real_freedom_graph(*, numeric_control=False):
    """Bounded public material; optional synthetic numeric extension is test-only."""
    entities = json.loads((ROOT / 'ToS/doctrine/semantic-interchange/entity-types.v1.json').read_text())
    relations = json.loads((ROOT / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_text())
    raw = real_freedom()['source_record']['payload']
    if numeric_control:
        raw = copy.deepcopy(raw)
        raw['properties'].pop('human_forms')
        raw['properties']['source_record']['unknown_numeric_extension'] = [1, 1.0, 9007199254740993, -0.0, 1e-7, 1e21, False]
    corpus = {'source_navigation': {'nodes': [raw], 'edges': []}}
    graph = build_knowledge_graph(corpus, {}, entity_type_registry=entities, relation_type_registry=relations)
    return graph, knowledge_catalog(graph, corpus, {}, entities, relations)


def real_canonical_graph():
    """The two opted-in canonical source nodes; never a whole-corpus build."""
    from tos_corpus_index_common import build_nodes
    with canonical_node_fixture() as (fixture_root, paths):
        diagnostics = []
        # The real index builder reports repository-relative source refs.  Keep
        # that contract while pointing its test-local root at exact snapshots,
        # rather than making sparse CI depend on the authored canon checkout.
        import tos_corpus_index_common as corpus_index
        with patch.object(corpus_index, 'REPO_ROOT', fixture_root):
            nodes = build_nodes(diagnostics, paths)
        if diagnostics or len(nodes) != 2:
            raise AssertionError('exact canonical source fixtures failed to build: ' + repr(diagnostics))
    corpus = {'nodes': nodes}
    entities = json.loads((ROOT / 'ToS/doctrine/semantic-interchange/entity-types.v1.json').read_text())
    relations = json.loads((ROOT / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_text())
    graph = build_knowledge_graph(corpus, {}, entity_type_registry=entities, relation_type_registry=relations)
    return graph, knowledge_catalog(graph, corpus, {}, entities, relations)


class ReadableContextTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry = json.loads((ROOT / 'ToS/doctrine/semantic-interchange/entity-types.v1.json').read_text())
        cls.relations = json.loads((ROOT / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_text())
        cls.schema = json.loads((ROOT / 'access/contracts/knowledge-graph.v1.schema.json').read_text())['$defs']['readableContext']

    def build(self, item, registry=None):
        result = build_readable_context(item, registry or self.registry, digest=_stable_digest)
        Draft202012Validator(self.schema).validate(result)
        return result

    def test_real_native_canonical_context_keeps_source_forms_and_no_admission(self):
        graph, _ = real_canonical_graph()
        nodes = [node for node in graph['nodes']
                 if node['attributes'].get('schema_version') == 'tos_canonical_node_v1']
        self.assertEqual(len(nodes), 2)
        for node in nodes:
            with self.subTest(node=node['id']):
                before = copy.deepcopy(node)
                sidecar = node['readable_context']
                # Full real HumanForms exceed the unchanged presentation budget.
                # Their exact roots remain available, never a partial ready view.
                self.assertEqual(sidecar['state'], 'requires-exact-context')
                self.assertEqual(sidecar['reason'], 'context-presentation-budget')
                self.assertEqual(sidecar['exact_context_pointers'],
                                 ['/attributes/human_forms', '/attributes/source_record'])
                self.assertEqual(sidecar['contexts'], [])
                self.assertEqual(sidecar, self.build(node))
                source = node['attributes']['source_record']
                source_hash = hashlib.sha256(json.dumps(source, ensure_ascii=False, sort_keys=True,
                    separators=(',', ':'), allow_nan=False).encode()).hexdigest()
                self.assertEqual(node['attributes']['source_sha256'], source_hash)
                direct = copy.deepcopy(node)
                direct['attributes'].pop('human_forms')
                direct_context = self.build(direct)
                self.assertEqual(direct_context['state'], 'complete')
                material = next(value for value in direct_context['exact_materials']
                                if '/attributes/source_record' in value['origin_pointers'])
                self.assertEqual(json.loads(material['canonical_json']), source)
                self.assertEqual(material['digest'], 'sha256:' + source_hash)
                self.assertFalse(sidecar['performs_semantic_assessment'])
                self.assertFalse(sidecar['performs_translation'])
                for form in node['attributes']['human_forms']:
                    if form['state'] != 'ready':
                        continue
                    self.assertEqual(form['subject'], {'id': source['node_id'],
                        'version': source['record_version'], 'digest': 'sha256:' + source_hash})
                    self.assertTrue(form['context'])
                    self.assertIsNone(form.get('admission'))
                    # One unchanged real form fits the budget and independently
                    # exercises native record binding inside HumanForm context.
                    selected = self.build({'attributes': {'human_forms': [form]}})
                    self.assertEqual(selected['state'], 'complete')
                    self.assertTrue(any(json.loads(value['canonical_json']) == source
                                        for value in selected['exact_materials']))
                self.assertEqual(node, before)

    def test_native_canonical_context_requires_exact_identity_and_preserves_extensions(self):
        graph, _ = real_canonical_graph()
        original = next(node for node in graph['nodes']
                        if node['attributes'].get('schema_version') == 'tos_canonical_node_v1')
        source = original['attributes']['source_record']
        mutations = [
            {'schema_version': 'tos_canonical_node_v999'}, {'schema_version': []},
            {'schema_version': None}, {'node_id': source['node_id'] + '\n'},
            {'node_type': ['support']}, {'node_type': 'foreign'},
            {'node_id': 'tos.event.test-wrong-kind', 'node_type': 'support'},
            {'record_version': True}, {'record_version': 0}, {'record_version': 9007199254740992},
            {'record_id': source['node_id']},
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                item = copy.deepcopy(original)
                item['attributes']['source_record'].update(mutation)
                item['attributes'].pop('source_sha256')
                result = self.build(item)
                self.assertEqual(result['state'], 'unavailable')
                self.assertEqual(result['contexts'], [])
                self.assertEqual(result['coverage']['returned_contexts'], 0)
        item = copy.deepcopy(original)
        item['attributes'].pop('human_forms')
        item['attributes'].pop('source_sha256')
        item['attributes']['source_record'].update(claim_id='source-declared-extension',
                                                  unknown_extension={'negation': False})
        result = self.build(item)
        self.assertEqual(result['state'], 'complete')
        entries = {entry['key']: entry for context in result['contexts'] for entry in context['entries']}
        self.assertEqual(entries['claim_id']['value'], 'source-declared-extension')
        self.assertEqual(entries['unknown_extension']['value'], {'negation': False})
        self.assertEqual(entries['unknown_extension']['category'], 'unclassified')

    def test_real_forms_are_unchanged_and_languages_remain_source_declared(self):
        item = real_freedom()
        before = copy.deepcopy(item)
        result = self.build(item)
        self.assertEqual(item, before)
        self.assertEqual(result['state'], 'complete')
        self.assertEqual(result['coverage']['input_contexts'], 3)
        entries = [e for c in result['contexts'] for e in c['entries']]
        scope = next(e for e in entries if e['key'] == 'semantic_scope')
        self.assertEqual(scope['language'], 'en')
        self.assertEqual(scope['value'], item['attributes']['source_record']['semantic_scope'])
        self.assertTrue(all(e['value'] == 'no_equivalence_claim' for e in entries if e['key'] == 'same_as_posture'))
        self.assertFalse(result['performs_semantic_assessment'])
        self.assertFalse(result['performs_translation'])

    def test_unknown_members_enums_false_zero_null_and_empty_are_not_hidden(self):
        item = real_freedom()
        item['attributes'].pop('human_forms')
        record = item['attributes']['source_record']
        record.update(identity_status='future-owner-value', negation=False, conditions=[],
                      scope=None, unknown_extension={'zero': 0, 'empty': '', 'boolean': False})
        entries = {e['key']: e for e in self.build(item)['contexts'][0]['entries']}
        self.assertEqual(entries['identity_status']['category'], 'unclassified')
        self.assertIsNone(entries['identity_status']['value_label'])
        for key in ('negation', 'conditions', 'scope', 'unknown_extension'):
            self.assertNotEqual(entries[key]['category'], 'technical')
            self.assertEqual(entries[key]['value'], record[key])
        self.assertNotIn('value', entries['record_id'])

    def test_exact_form_pointer_digest_version_and_bool_number_tampering_refused(self):
        for mutation in ('pointer', 'digest', 'version', 'value'):
            with self.subTest(mutation=mutation):
                item = real_freedom()
                entry = item['attributes']['human_forms'][0]['context'][0]
                if mutation == 'pointer': entry['binding']['pointer'] = '/no~2such'
                if mutation == 'digest': entry['binding']['record']['digest'] = 'sha256:' + '0' * 64
                if mutation == 'version': entry['binding']['record']['version'] = True
                if mutation == 'value': entry['value'] = False
                self.assertEqual(self.build(item)['state'], 'unavailable')
        item = real_freedom()
        item['readable_context'] = self.build(item)
        item['readable_context']['performs_translation'] = 0
        with self.assertRaises(ReadableContextError): validate_sidecar(item, self.registry, digest=_stable_digest)

    def test_unrecognized_schema_never_inherits_technical_or_governing_rules(self):
        item = real_freedom()
        item['attributes'].pop('human_forms')
        item['attributes']['source_record']['schema_version'] = 'tos_future_source_v999'
        self.assertTrue(all(e['category'] == 'unclassified' for e in self.build(item)['contexts'][0]['entries']))

    def test_retained_historical_claim_gets_known_labels_without_reinterpreting_values(self):
        source = knowledge_fixture_path(
            'ToS/source-witnesses/history/friedrich-nietzsche/jenseits-1886-commission/historical-claims.jsonl')
        record = next(json.loads(line) for line in source.read_text().splitlines()
                      if json.loads(line)['claim_id'] == 'tos.claim.jenseits-1886-commission.date')
        item = _normalize_node({'node_id': 'claim:' + record['claim_id'], 'node_type': 'claim',
                                'properties': {'source_claim': record}}, 'source-claims')
        before = copy.deepcopy(item)
        result = self.build(item)
        self.assertEqual(result['state'], 'complete')
        direct = next(c for c in result['contexts'] if c['origin_pointer'] == '/attributes/source_claim')
        entries = {e['key']: e for e in direct['entries']}
        for field in ('schema_version', 'claim_id', 'claim_version'):
            self.assertEqual(entries[field]['category'], 'technical')
        for field in ('object', 'qualifiers', 'epistemic_status', 'review_status', 'evidence_refs'):
            self.assertEqual(entries[field]['category'], 'governing')
            self.assertEqual(entries[field]['value'], record[field])
        self.assertEqual(entries['object']['value']['source_wording'], record['object']['source_wording'])
        self.assertIsNone(entries['object']['language'])  # No whole-object language is declared.
        self.assertFalse(entries['qualifiers']['value']['primary_letter_inspected'])
        self.assertEqual(item, before)

        # Synthetic unknown extension and schema controls are not historical facts.
        item['attributes']['source_claim']['extensions'] = {'negation': False, 'calendar': None}
        extensions = next(e for c in self.build(item)['contexts']
                          if c['origin_pointer'] == '/attributes/source_claim'
                          for e in c['entries'] if e['key'] == 'extensions')
        self.assertEqual(extensions['category'], 'unclassified')
        self.assertEqual(extensions['value'], {'negation': False, 'calendar': None})
        item['attributes']['source_claim']['schema_version'] = 'tos_historical_claim_v999'
        unknown_before = copy.deepcopy(item)
        unknown = self.build(item)
        # Unknown-schema presentation of the grown source exceeds the unchanged
        # owner budget. It must refuse the whole sidecar, not omit qualifiers.
        self.assertEqual(unknown['state'], 'requires-exact-context')
        self.assertEqual(unknown['reason'], 'context-presentation-budget')
        self.assertEqual(unknown['exact_context_pointers'],
                         ['/semantics/assertion_contexts', '/attributes/source_claim'])
        self.assertEqual(unknown['contexts'], [])
        self.assertEqual(unknown['exact_materials'], [])
        self.assertEqual(unknown['coverage'], {'input_contexts': 2, 'returned_contexts': 0,
                                             'entries': 0, 'unclassified_entries': 0})
        self.assertFalse(unknown['performs_semantic_assessment'])
        self.assertFalse(unknown['performs_translation'])
        self.assertEqual(item, unknown_before)

    def test_small_unknown_claim_schema_does_not_inherit_known_field_classification(self):
        # A bounded synthetic record isolates schema dispatch from source growth.
        record = {'schema_version': 'tos_historical_claim_v999',
                  'claim_id': 'tos.claim.test.unknown-context', 'claim_version': 1,
                  'review_status': 'unreviewed', 'object': {'calendar': None},
                  'qualifiers': {'negation': False, 'zero': 0, 'empty': ''},
                  'evidence_refs': []}
        item = _normalize_node({'node_id': 'claim:' + record['claim_id'], 'node_type': 'claim',
                                'properties': {'source_claim': record}}, 'source-claims')
        before = copy.deepcopy(item)
        result = self.build(item)
        self.assertEqual(result['state'], 'complete')
        context = next(context for context in result['contexts']
                       if context['origin_pointer'] == '/attributes/source_claim')
        self.assertEqual(context['origin_pointer'], '/attributes/source_claim')
        self.assertEqual({entry['key']: entry['value'] for entry in context['entries']}, record)
        for entry in context['entries']:
            self.assertEqual(entry['category'], 'unclassified')
            self.assertEqual(entry['value_mode'], 'source-value')
            self.assertIsNone(entry['value_label'])
            self.assertEqual(entry['value_pointer'], '/attributes/source_claim/' + entry['key'])
            self.assertEqual(entry['binding']['source_pointer'], '/' + entry['key'])
        self.assertEqual(item, before)

    def test_claim_context_reference_is_visible_and_not_an_identity_or_admission(self):
        payload = {'claim_ref': 'tos.claim.test.context-subject'}
        item = {'attributes': {}, 'source_record': {'payload': payload, 'digest': _stable_digest(payload)},
                'semantics': {'assertion_contexts': [{'schema_version': 'tos_assertion_context_v1',
                    'binding_role': 'carrier', 'source_record_digest': _stable_digest(payload),
                    'source_refs': ['test:synthetic-claim-reference'],
                    'interpretation': 'source-declared-not-semantic-assessment',
                    'fields': {'claim_ref': {'value': payload['claim_ref'], 'source_pointer': '/claim_ref'}},
                    'conflicts': []}]}}
        result = self.build(item)
        entry, = result['contexts'][0]['entries']
        self.assertEqual(entry['category'], 'governing')
        self.assertEqual(entry['value'], payload['claim_ref'])
        self.assertEqual(entry['binding']['source_pointer'], '/claim_ref')
        self.assertFalse(result['performs_semantic_assessment'])

    def test_document_catalogue_context_preserves_null_calendar_and_field_selection(self):
        # Synthetic metadata exercises the declared profile, not a catalogue observation.
        record = {'schema_version': 'tos_document_catalogue_claim_v1',
                  'claim_id': 'tos.claim.test.catalogue-context', 'claim_version': 1,
                  'claim_type': 'relation', 'assertion_layer': 'bibliographic_assertion',
                  'predicate': 'document_catalogue_date', 'subject_ref': 'tos.letter.test.catalogue',
                  'object': {'kind': 'date-assertion', 'role': 'catalogue-assigned-document-date',
                             'value': '1886-06-03', 'calendar': None, 'year_numbering': None,
                             'certainty': 'exact', 'source_wording': {'text': '3.6.1886', 'language': None}},
                  'qualifiers': {'catalogue_attribution': {'evidence_ref': 'test:synthetic-catalogue',
                                 'source_field': 'Eintrag', 'field_role': 'assigned-date',
                                 'source_wording': {'text': '3.6.1886', 'language': None}},
                                 'dispatch_established': False},
                  'epistemic_status': 'reported', 'review_status': 'unreviewed',
                  'visibility': 'public_metadata_only', 'evidence_refs': ['test:synthetic-catalogue']}
        item = {'attributes': {'source_claim': record}, 'semantics': {}}
        before = copy.deepcopy(item)
        result = self.build(item)
        self.assertEqual(result['state'], 'complete')
        entries = {e['key']: e for e in result['contexts'][0]['entries']}
        for field in ('object', 'qualifiers', 'epistemic_status', 'review_status', 'evidence_refs'):
            self.assertEqual(entries[field]['category'], 'governing')
            self.assertEqual(entries[field]['value'], record[field])
        self.assertIsNone(entries['object']['value']['calendar'])
        self.assertIsNone(entries['object']['value']['year_numbering'])
        self.assertFalse(entries['qualifiers']['value']['dispatch_established'])
        self.assertIsNone(entries['object']['language'])
        self.assertFalse(result['performs_semantic_assessment'])
        self.assertEqual(item, before)

    def test_budget_returns_exact_roots_and_no_partial_ready(self):
        item = real_freedom()
        registry = copy.deepcopy(self.registry)
        registry['context_presentation']['max_entries'] = 1
        result = self.build(item, registry)
        self.assertEqual(result['state'], 'requires-exact-context')
        self.assertEqual(result['contexts'], [])
        self.assertEqual(result['exact_materials'], [])
        self.assertIn('/attributes/human_forms', result['exact_context_pointers'])
        self.assertEqual(result['coverage']['input_contexts'], 3)

    def test_vocabulary_evolution_dependency_and_catalog_exactness(self):
        old = copy.deepcopy(self.registry)
        changed = copy.deepcopy(old)
        changed['context_presentation']['field_rules'][0]['label']['ru'] += ' уточнение'
        self.assertTrue(validate_vocabulary(changed, old))
        self.assertFalse(validate_semantic_registries(changed, self.relations, previous_entity_registry=old)['valid'])
        changed['context_presentation']['presentation_version'] += 1
        self.assertEqual(validate_vocabulary(changed, old), [])
        self.assertNotEqual(ReadableContextCompiler(old, digest=_stable_digest).dependency,
                            ReadableContextCompiler(changed, digest=_stable_digest).dependency)
        item = real_freedom()
        item['readable_context'] = self.build(item)
        with self.assertRaises(ReadableContextError): validate_sidecar(item, changed, digest=_stable_digest)
        catalog = presentation_catalog(old)
        self.assertEqual(catalog['payload'], old['context_presentation'])
        self.assertEqual(catalog['digest'], vocabulary_digest(catalog['payload']))
        catalog['payload']['presentation_version'] = 999
        self.assertEqual(old, self.registry)

    def test_vocabulary_cannot_hide_unknown_or_governing_context(self):
        for field in ('negation', 'conditions', 'semantic_scope', 'mysterious_qualifier'):
            changed = copy.deepcopy(self.registry)
            rule = changed['context_presentation']['field_rules'][0]
            rule.update(field=field, category='technical')
            self.assertTrue(validate_vocabulary(changed))
        removed = copy.deepcopy(self.registry)
        removed.pop('context_presentation')
        self.assertTrue(validate_vocabulary(removed, self.registry))

    def test_assertion_context_conflicts_and_boolean_values_keep_exact_bindings(self):
        payload = {'polarity': False, 'conditions': None, 'unknown': {'negated': True},
                   'numeric_control': [1, 1.0, 9007199254740993, -0.0, 1e-7, 1e21]}
        digest = _stable_digest(payload)
        item = {'attributes': {}, 'source_record': {'payload': payload, 'digest': digest},
                'semantics': {'assertion_contexts': [{'schema_version': 'tos_assertion_context_v1',
                    'binding_role': 'carrier', 'source_record_digest': digest,
                    'source_refs': ['test:synthetic-conflict'],
                    'interpretation': 'source-declared-not-semantic-assessment',
                    'fields': {key: {'value': value, 'source_pointer': '/' + key} for key, value in payload.items()},
                    'conflicts': [{'field': 'polarity',
                        'lower_priority': {'value': False, 'source_pointer': '/polarity'},
                        'higher_priority': {'value': True, 'source_pointer': '/unknown/negated'}}]}]}}
        graph_schema = json.loads((ROOT / 'access/contracts/knowledge-graph.v1.schema.json').read_text())
        Draft202012Validator({'$ref': '#/$defs/assertionContext', '$defs': graph_schema['$defs']}).validate(
            item['semantics']['assertion_contexts'][0])
        result = self.build(item)
        self.assertEqual(result['state'], 'complete')
        material, = result['exact_materials']
        self.assertEqual(material['origin_pointers'], ['/semantics/assertion_contexts/0'])
        self.assertIn('[1,1.0,9007199254740993,-0.0,1e-07,1e+21]', material['canonical_json'])
        entries = {e['key']: e for e in result['contexts'][0]['entries']}
        self.assertEqual(entries['polarity']['category'], 'unclassified')
        self.assertIs(entries['polarity']['value'], False)
        self.assertEqual(entries['conflicts']['value'], item['semantics']['assertion_contexts'][0]['conflicts'])
        item['semantics']['assertion_contexts'][0]['fields']['polarity']['value'] = 0
        self.assertEqual(self.build(item)['state'], 'unavailable')

    def test_full_build_catalog_and_compact_transport_share_exact_contract(self):
        graph, catalog = real_freedom_graph()
        Draft202012Validator(json.loads((ROOT / 'access/contracts/knowledge-graph.v1.schema.json').read_text())).validate(graph)
        node = next(n for n in graph['nodes'] if n['native_id'].startswith('identity:'))
        self.assertEqual(catalog['source_revision'], graph['source_revision'])
        self.assertEqual(catalog['context_presentation'], presentation_catalog(self.registry))
        self.assertEqual(node['readable_context']['vocabulary'], {
            key: catalog['context_presentation'][key] for key in ('id', 'version', 'source_ref', 'digest')})
        validate_sidecar(node, self.registry, digest=_stable_digest)
        for language in ('ru', 'en'):
            full = _lens_carrier(node, 'full', language=language)
            compact = _lens_carrier(node, 'compact', language=language)
            self.assertEqual(full['readable_context'], node['readable_context'])
            self.assertNotIn('readable_context', compact)
            self.assertEqual(full['human_form_selection'], compact['human_form_selection'])
            self.assertEqual(compact['attributes'], {})
            forged = {**compact, 'readable_context': full['readable_context']}
            with self.assertRaises(ReadableContextError):
                validate_sidecar(forged, self.registry, digest=_stable_digest)

    def test_contextless_carriers_do_not_copy_or_fill_normalization_cache(self):
        from unittest.mock import Mock
        from tos_access.normalization_cache import active_cache
        item = _normalize_node({'node_id': 'test:contextless', 'properties': {}}, 'philosophy')
        compiler = ReadableContextCompiler(self.registry, digest=_stable_digest)
        cache = Mock()
        token = active_cache.set(cache)
        try:
            self.assertIs(_attach_readable_context(item, compiler, 'node'), item)
            cache.memo.assert_not_called()
        finally:
            active_cache.reset(token)

    def test_cached_context_tracks_vocabulary_changes_without_changing_raw_forms(self):
        from tos_access.normalization_cache import NormalizationCache
        item = real_freedom()
        changed = copy.deepcopy(self.registry)
        changed['context_presentation']['presentation_version'] += 1
        changed['context_presentation']['field_rules'][0]['label']['ru'] += ' (уточнение)'
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'context.sqlite'
            compiler = ReadableContextCompiler(self.registry, digest=_stable_digest)
            with NormalizationCache(path, 'test:bounded-context-dependency'):
                first = _attach_readable_context(item, compiler, 'node')
            with NormalizationCache(path, 'test:bounded-context-dependency') as cache:
                repeated = _attach_readable_context(item, compiler, 'node')
                self.assertEqual(first, repeated)
                self.assertGreater(cache.hits, 0)
            with NormalizationCache(path, 'test:bounded-context-dependency') as cache:
                successor = _attach_readable_context(item, ReadableContextCompiler(changed, digest=_stable_digest), 'node')
                self.assertGreater(cache.misses, 0)
            self.assertNotEqual(first['content_revision'], successor['content_revision'])
            self.assertNotEqual(first['readable_context']['vocabulary'], successor['readable_context']['vocabulary'])
            self.assertEqual(first['attributes'], successor['attributes'])
            self.assertEqual(item['attributes'], first['attributes'])

    def test_exact_material_preserves_numbers_and_deduplicates_record_bindings(self):
        raw = copy.deepcopy(real_freedom()['source_record']['payload'])
        raw['properties'].pop('human_forms')
        values = [1, 1.0, 9007199254740993, -0.0, 1e-7, 1e21, False]
        raw['properties']['source_record']['unknown_numeric_extension'] = values
        item = _normalize_node(raw, 'source-navigation')
        sidecar = self.build(item)
        self.assertEqual(sidecar['state'], 'complete')
        material, = sidecar['exact_materials']
        self.assertIn('[1,1.0,9007199254740993,-0.0,1e-07,1e+21,false]', material['canonical_json'])
        self.assertEqual(material['digest'], 'sha256:' + hashlib.sha256(material['canonical_json'].encode()).hexdigest())
        decoded = json.loads(material['canonical_json'])
        self.assertIs(type(decoded['unknown_numeric_extension'][0]), int)
        self.assertIs(type(decoded['unknown_numeric_extension'][1]), float)
        self.assertEqual(material['origin_pointers'], ['/attributes/source_record'])
        self.assertEqual(next(e for e in sidecar['contexts'][0]['entries']
            if e['key'] == 'unknown_numeric_extension')['binding']['record']['digest'], material['digest'])
        item['readable_context'] = copy.deepcopy(sidecar)
        item['readable_context']['exact_materials'][0]['canonical_json'] = material['canonical_json'].replace('9007199254740993', '9007199254740992')
        with self.assertRaises(ReadableContextError):
            validate_sidecar(item, self.registry, digest=_stable_digest)
        original = real_freedom()
        result = self.build(original)
        references = [e['binding']['record']['digest'] for c in result['contexts'] for e in c['entries'] if e['binding']['kind'] == 'record']
        self.assertGreater(len(references), len(set(references)))
        for reference in set(references):
            self.assertEqual(sum(m['digest'] == reference for m in result['exact_materials']), 1)


if __name__ == '__main__':
    unittest.main()

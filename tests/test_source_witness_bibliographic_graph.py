from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from contextlib import contextmanager
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

    def test_historical_sources_reach_existing_focus_forms_and_claim_inspection(self):
        from source_commands import prepare_metadata_change
        from knowledge_assessment import Record
        with self.historical_fixture() as (root, history, real, claims, rebuild):
            event_path, event = history[0]
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
            with self.assertRaisesRegex(BibliographicGraphBuildError, 'historical record'):
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
            self.assertEqual(len(fixity), 2)
            claim_path = root / claim_binding['path']
            claim_path.write_text(json.dumps({**claims[0], 'visibility': 'local_only'}))
            with self.assertRaisesRegex(PermissionError, 'nonpublic'):
                _source_records(root, [claim_binding])
            event_path.write_text(json.dumps({**event, 'visibility': 'research_group'}))
            with self.assertRaisesRegex(PermissionError, 'nonpublic'):
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
            self.assertEqual(standalone['counts']['nodes'], 3)
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
        self.assertEqual(counts["source_claims"], 193)
        self.assertEqual(counts["claim_traces"], 193)
        self.assertEqual(counts["nodes"], 680)
        self.assertEqual(counts["edges"], 1312)
        self.assertEqual(counts["direct_subject_object_edges"], 0)
        self.assertFalse(payload["relation_model"]["direct_subject_object_edges"])
        self.assertEqual(payload["graph_layers"], ["bibliographic"])
        self.assertEqual(payload["review_counts"], {"unreviewed": 193})
        self.assertEqual(payload["visibility_counts"], {"public_metadata_only": 193})
        self.assertEqual(
            payload["projection_fingerprint"],
            _projection_fingerprint(payload),
        )

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
            self.assertTrue(event["properties"]["method"]["name"])
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

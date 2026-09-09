"""Synthetic contract cases plus unchanged real source-date envelopes.

Synthetic date relations are arithmetic fixtures, not historical evidence.
"""
import copy
import asyncio
import io
import json
import sys
import unittest
from contextlib import redirect_stdout
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
from pathlib import Path
from threading import Thread
from unittest.mock import patch

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ACCESS = Path(__file__).resolve().parents[1]
ROOT = ACCESS.parent
sys.path.insert(0, str(ACCESS / 'src'))

from tos_access.knowledge import KnowledgeGraphIndex, build_knowledge_graph
from tos_access.core import ToSAccessCore
from tos_access.lens_pagination import KnowledgeRevisionConflict
from tos_access.temporal_comparison import (
    compare_temporal_claims, compare_temporal_operands, normalize_temporal_comparison_request,
    TemporalReadModelInvalid,
)


def date(value, **changes):
    return {'kind': 'date-assertion', 'value': value, 'calendar': 'proleptic-gregorian',
            'year_numbering': 'astronomical', 'certainty': 'exact', 'role': 'historical-time',
            'source_wording': {'text': 'Synthetic date only', 'language': 'en'}, **changes}


def interval(start, end, **changes):
    result = date(None, kind='interval-assertion', interval={'start': start, 'end': end}, **changes)
    result.pop('value')
    return result


class TemporalComparisonTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        base = ROOT / 'ToS/doctrine/semantic-interchange'
        cls.entities = json.loads((base / 'entity-types.v1.json').read_text())
        cls.relations = json.loads((base / 'relation-types.v1.json').read_text())
        cls.schemas = {name: json.loads((ACCESS / 'contracts' / name).read_text()) for name in (
            'knowledge-graph.v1.schema.json', 'temporal-comparison-request.v1.schema.json',
            'temporal-comparison-result.v1.schema.json')}
        cls.registry = Registry().with_resources((schema['$id'], Resource.from_contents(schema)) for schema in cls.schemas.values())

    def fixture(self, left=None, right=None, *, source_claims=None, source_subjects=None, predicate='historical_dating', registry=None):
        nodes, traces, edges = [], [], []
        for pos, value in enumerate((left or date('1869'), right or date('1879'))):
            claim = copy.deepcopy(source_claims[pos]) if source_claims else {
                'claim_id': f'tos.claim.synthetic-temporal-{pos}', 'claim_version': 1,
                'predicate': predicate, 'object': value, 'review_status': 'unreviewed',
                'epistemic_status': 'uncertain', 'polarity': 'negative',
                'qualifiers': {'statement': 'Synthetic disputed dating, not a source assertion.',
                               'statement_language': 'en', 'unknown_extension': [False, None, []]},
            }
            identifier, literal = 'claim:' + claim['claim_id'], f'literal:synthetic-{pos}'
            subject = claim.get('subject_ref', f'tos.historical-event.synthetic-{pos}')
            subject_id = 'identity:' + subject
            source_ref = 'test:synthetic-date' if source_claims is None else 'ToS/source-witnesses/relations/basel-biography-research/source-claims.jsonl'
            nodes.extend([
                {'node_id': identifier, 'node_kind': 'claim', 'source_ref': source_ref,
                 'properties': {**claim, 'claim_ref': claim['claim_id'], 'source_claim': claim}},
                {'node_id': literal, 'node_kind': 'literal', 'source_ref': source_ref,
                 'properties': {'value': claim['object'], 'claim_ref': claim['claim_id']}},
                {'node_id': subject_id, 'node_kind': 'identity', 'source_ref': source_ref,
                 'properties': {**(source_subjects or {}).get(subject, {}),
                                **({'source_record': source_subjects[subject]} if source_subjects and subject in source_subjects else {}),
                                'identity_ref': subject, 'identity_kind': subject.split('.')[1]}},
            ])
            edges.extend({'edge_id': identifier + ':' + kind, 'edge_kind': kind, 'from_id': identifier,
                          'to_id': endpoint, 'claim_ref': claim['claim_id'], 'review_status': claim['review_status'],
                          'source_claim_file_ref': source_ref}
                         for kind, endpoint in (('has_subject', subject_id), ('has_object', literal)))
            traces.append({'claim_ref': claim['claim_id'], 'claim_node_id': identifier,
                           'object_node_id': literal, 'subject_node_id': subject_id,
                           'predicate': claim['predicate'], 'review_status': claim['review_status'],
                           'epistemic_status': claim['epistemic_status']})
        graph = build_knowledge_graph({}, {}, {'nodes': nodes, 'edges': edges, 'claim_traces': traces}, self.entities, registry or self.relations)
        index = KnowledgeGraphIndex(graph)
        selected = [index.node_ids['source-claims:' + trace['claim_node_id']][0] for trace in traces]
        request = {'schema_version': 'tos_temporal_comparison_request_v1', 'source_revision': graph['source_revision'],
                   **{side: {'node_id': node['id'], 'content_revision': node['content_revision']}
                      for side, node in zip(('left', 'right'), selected)}}
        return graph, index, request

    def compare(self, left, right):
        graph, index, request = self.fixture(left, right)
        original = copy.deepcopy(graph)
        result = compare_temporal_claims(graph, request, graph_index=index)
        self.assertEqual(graph, original)
        Draft202012Validator(self.schemas['temporal-comparison-result.v1.schema.json'], registry=self.registry).validate(result)
        return result

    def test_partition_inclusive_bounds_and_precision_envelopes(self):
        cases = [
            (date('1869'), date('1870'), 'before'), (date('1870'), date('1869'), 'after'),
            (date('1869'), date('1869'), 'equal'),
            (date('1869'), date('1869-03'), 'contains'), (date('1869-03'), date('1869'), 'contained-by'),
            (interval('1869', '1879'), interval('1875', '1880'), 'overlaps'),
            (interval('1869-01-01', '1869-03-01'), interval('1869-03-01', '1870'), 'overlaps'),
            (interval('1869', '1879'), interval('1869', '1878'), 'contains'),
            (date('-0001'), date('0000'), 'before'), (date('0000-02-29'), date('0000-03'), 'before'),
        ]
        for left, right, relation in cases:
            with self.subTest(relation=relation, left=left, right=right):
                result = self.compare(left, right)
                self.assertEqual(result['comparison']['status'], 'comparable')
                self.assertEqual(result['comparison']['relation'], relation)
                self.assertFalse(result['authority_boundary']['creates_inferred_claim'])

    def test_unknown_uncertain_relative_and_invalid_dates_do_not_become_false(self):
        cases = [date('1869', calendar=None), date('1869', year_numbering=None),
                 date('1869', certainty=None),
                 date('1869', certainty='approximate'), date('1869', certainty='uncertain'),
                 date('1869', role=None), date('1869', precision='unknown'),
                 interval(None, '1879'),
                 {'kind': 'relative-order', 'relative': {'relation': 'during', 'anchor_ref': 'tos.phase.synthetic'},
                  'calendar': None, 'year_numbering': None, 'certainty': 'uncertain', 'role': 'historical-time'}]
        missing_certainty = date('1869'); missing_certainty.pop('certainty'); cases.append(missing_certainty)
        for value in cases:
            with self.subTest(value=value):
                comparison = self.compare(value, date('1870'))['comparison']
                self.assertEqual(comparison['status'], 'undetermined')
                self.assertIsNone(comparison['relation'])
                self.assertTrue(comparison['reasons'])

    def test_unsupported_systems_conflicts_and_invalid_shapes_are_explicit(self):
        cases = [date('1869', calendar='julian'), date('1869', year_numbering='regnal'),
                 date('1869-02-30'), date('c. 1869'), interval('1879', '1869'),
                 date('1869', interval={'start': '1869', 'end': '1879', 'calendar': 'julian'}),
                 date('1869', kind='uninterpreted-future-temporal-shape')]
        for value in cases:
            with self.subTest(value=value):
                comparison = self.compare(value, date('1870'))['comparison']
                self.assertEqual(comparison['status'], 'unsupported')
                self.assertIsNone(comparison['relation'])
                self.assertTrue(comparison['reasons'])

    def test_time_roles_are_not_silently_substituted(self):
        result = self.compare(date('1869'), date('1870', role='witness-time'))
        self.assertEqual(result['comparison']['status'], 'unsupported')
        self.assertIn({'side': 'pair', 'code': 'different-time-roles'}, result['comparison']['reasons'])
        result = self.compare(date('1869', role='future-role'), date('1870', role='future-role'))
        self.assertIn({'side': 'pair', 'code': 'unsupported-time-role'}, result['comparison']['reasons'])

    def test_new_declared_temporal_predicate_needs_no_handler_change(self):
        registry = copy.deepcopy(self.relations)
        entries = registry['relations']
        entry = next(item for item in entries if item['relation_type_id'] == 'tos.relation.historical-dating')
        entry['source_mappings'].append({'source_graph': 'source-claims', 'source_predicate_id': 'synthetic_new_dating', 'scope': 'claim-predicate'})
        graph, index, request = self.fixture(predicate='synthetic_new_dating', registry=registry)
        result = compare_temporal_claims(graph, request, graph_index=index)
        self.assertEqual(result['comparison']['relation'], 'before')
        self.assertEqual(result['left']['claim']['semantics']['claim']['source_predicate_id'], 'synthetic_new_dating')

    def test_self_comparison_does_not_create_independent_evidence(self):
        graph, index, request = self.fixture()
        request['right'] = dict(request['left'])
        result = compare_temporal_claims(graph, request, graph_index=index)
        self.assertEqual(result['comparison']['relation'], 'equal')
        self.assertEqual(result['left']['claim']['id'], result['right']['claim']['id'])
        self.assertFalse(result['authority_boundary']['creates_inferred_claim'])

    def test_raw_qualifiers_polarity_and_versions_survive_without_acceptance(self):
        result = self.compare(date('1869'), date('1879'))
        claim = result['left']['claim']['attributes']['source_claim']
        self.assertEqual(claim['qualifiers']['unknown_extension'], [False, None, []])
        self.assertEqual(claim['polarity'], 'negative')
        self.assertEqual(claim['review_status'], 'unreviewed')
        self.assertEqual(claim['claim_version'], 1)
        self.assertEqual(result['left']['normalized_time']['raw'], claim['object'])
        self.assertRegex(result['left']['value']['content_revision'], '^[a-f0-9]{64}$')
        self.assertFalse(result['authority_boundary']['performs_assessment'])

    def test_exact_revision_and_no_alias_resolution(self):
        for case in ('source', 'content', 'alias', 'duplicate'):
            graph, index, request = self.fixture()
            if case == 'source': request['source_revision'] = '0' * 64
            elif case == 'content': request['left']['content_revision'] = '0' * 64
            elif case == 'alias': request['left']['node_id'] = index.node_ids[request['left']['node_id']][0]['native_id']
            else: index.node_ids[request['left']['node_id']].append(index.node_ids[request['left']['node_id']][0])
            with self.subTest(case=case), self.assertRaises(KnowledgeRevisionConflict if case in ('source', 'content') else KeyError):
                compare_temporal_claims(graph, request, graph_index=index)

    def test_foreign_object_trace_and_json_boolean_alias_are_rejected(self):
        for case in ('claim-id', 'version', 'predicate', 'claim-ref', 'value', 'raw', 'bool-number', 'foreign-object'):
            graph, index, request = self.fixture()
            claim = index.node_ids[request['left']['node_id']][0]
            value = index.node_ids[claim['semantics']['claim']['object_node_id']][0]
            if case == 'claim-id': claim['semantics']['claim']['claim_id'] = 'tos.claim.foreign'
            elif case == 'version': claim['semantics']['claim']['claim_version'] = True
            elif case == 'predicate': claim['semantics']['claim']['source_predicate_id'] = 'unknown'
            elif case == 'claim-ref': value['attributes']['claim_ref'] = 'tos.claim.foreign'
            elif case == 'value': value['attributes']['value']['value'] = '1900'
            elif case == 'raw': value['semantics']['time']['raw']['value'] = '1900'
            elif case == 'bool-number':
                claim['attributes']['source_claim']['object']['extension'] = True
                value['attributes']['value']['extension'] = 1
            else:
                claim['semantics']['claim']['object_node_id'] = index.node_ids[request['right']['node_id']][0]['semantics']['claim']['object_node_id']
            with self.subTest(case=case):
                result = compare_temporal_claims(graph, request, graph_index=index)
                self.assertEqual(result['comparison']['status'], 'undetermined')
                self.assertIsNone(result['comparison']['relation'])

    def test_at_most_four_lookups_after_shared_index_preparation(self):
        graph, index, request = self.fixture()
        calls = []
        def lookup(identifier):
            calls.append(identifier)
            return index.node_ids.get(identifier, ())
        result = compare_temporal_operands(graph['source_revision'], request, lookup)
        self.assertEqual(len(calls), 4)
        self.assertEqual(result['comparison']['relation'], 'before')
        with self.assertRaisesRegex(ValueError, 'different snapshot'):
            compare_temporal_claims(dict(graph), request, graph_index=index)

    def transport_cases(self):
        """Shared Python/Worker examples; all synthetic except the named Basel pair.

        Malformed-carrier controls model a corrupt derived input, not valid
        owner source. Do not silently repair them to satisfy graph validation.
        """
        pairs = [
            (date('1869'), date('1870')), (date('1870'), date('1869')),
            (date('1869'), date('1869')), (date('1869'), date('1869-03')),
            (date('1869-03'), date('1869')),
            (interval('1869', '1879'), interval('1875', '1880')),
            (interval('1869-01-01', '1869-03-01'), interval('1869-03-01', '1870')),
            (date('-0001'), date('0000')), (date('0000-02-29'), date('0000-03')),
        ]
        pairs.extend((value, date('1870')) for value in (
            date('1869', calendar=None), date('1869', year_numbering=None),
            date('1869', certainty=None), date('1869', certainty='uncertain'),
            date('1869', role=None), date('1869', role='witness-time'),
            date('1869', calendar='julian'), date('1869', year_numbering='regnal'),
            date('1869-02-30'), date('c. 1869'), interval('1879', '1869'), interval(None, '1879'),
            {'kind': 'unknown-date', 'certainty': 'unknown'},
            {'kind': 'relative-order', 'relative': {'relation': 'during', 'anchor_ref': 'tos.phase.synthetic'},
             'calendar': None, 'year_numbering': None, 'certainty': 'uncertain', 'role': 'historical-time'},
        ))
        fixtures = [(f'date-{pos}', self.fixture(left, right)) for pos, (left, right) in enumerate(pairs)]
        for case in ('missing-value', 'duplicate-value', 'non-claim', 'non-temporal', 'foreign-binding',
                     'missing-normalization', 'issues-null', 'issues-string', 'issues-empty-code',
                     'issues-mixed', 'bool-number', 'number-spelling', 'fractional-bound', 'unsafe-bound',
                     'kind-array', 'calendar-array', 'calendar-null-with-keys', 'calendar-missing-with-keys',
                     'numbering-null-with-keys', 'numbering-missing-with-keys',
                     'relative-with-keys', 'unknown-with-keys', 'uncertain-precision-with-keys',
                     'claim-semantics-null', 'claim-attributes-null', 'value-mapping-null',
                     'claim-mapping-list', 'claim-semantics-list', 'claim-packet-list',
                     'claim-attributes-list', 'value-attributes-list', 'value-semantics-list', 'time-container-null'):
            graph, index, request = self.fixture()
            claim = index.node_ids[request['left']['node_id']][0]
            value = index.node_ids[claim['semantics']['claim']['object_node_id']][0]
            if case == 'missing-value': graph['nodes'].remove(value)
            elif case == 'duplicate-value': graph['nodes'].append(copy.deepcopy(value))
            elif case == 'non-claim': claim['kind_id'] = 'identity'
            elif case == 'non-temporal': value['type_id'] = 'tos.entity.literal'
            elif case == 'foreign-binding': value['attributes']['claim_ref'] = 'tos.claim.foreign'
            elif case == 'missing-normalization': value['semantics'].pop('time')
            elif case.startswith('issues-'):
                value['semantics']['time']['issues'] = {
                    'issues-null': None, 'issues-string': 'unknown-calendar',
                    'issues-empty-code': [''], 'issues-mixed': ['unknown-calendar', False],
                }[case]
            elif case in ('bool-number', 'number-spelling'):
                claim['attributes']['source_claim']['object']['extension'] = 1 if case == 'number-spelling' else True
                value['attributes']['value']['extension'] = 1.0
                value['semantics']['time']['raw']['extension'] = 1.0
            elif case == 'fractional-bound': value['semantics']['time']['sort_start'] = 18690101.5
            elif case == 'unsafe-bound': value['semantics']['time']['sort_start'] = -9007199254740992
            elif case == 'kind-array': value['semantics']['time']['kind'] = ['date-assertion']
            elif case == 'calendar-array': value['semantics']['time']['calendar'] = ['gregorian']
            elif case == 'calendar-null-with-keys': value['semantics']['time']['calendar'] = None
            elif case == 'calendar-missing-with-keys': value['semantics']['time'].pop('calendar')
            elif case == 'numbering-null-with-keys': value['semantics']['time']['declared_year_numbering'] = None
            elif case == 'numbering-missing-with-keys': value['semantics']['time'].pop('declared_year_numbering')
            elif case == 'relative-with-keys': value['semantics']['time']['kind'] = 'relative-order'
            elif case == 'unknown-with-keys': value['semantics']['time']['kind'] = 'unknown-date'
            elif case == 'uncertain-precision-with-keys': value['semantics']['time']['precision'] = 'uncertain'
            elif case == 'claim-semantics-null': claim['semantics'] = None
            elif case == 'claim-attributes-null': claim['attributes'] = None
            elif case == 'value-mapping-null': value['type_mapping'] = None
            elif case == 'claim-mapping-list': claim['type_mapping'] = []
            elif case == 'claim-semantics-list': claim['semantics'] = []
            elif case == 'claim-packet-list': claim['semantics']['claim'] = []
            elif case == 'claim-attributes-list': claim['attributes'] = []
            elif case == 'value-attributes-list': value['attributes'] = []
            elif case == 'value-semantics-list': value['semantics'] = []
            elif case == 'time-container-null': value['semantics']['time'] = None
            fixtures.append((case, (graph, KnowledgeGraphIndex(graph), request)))
        fixtures.append(('real-basel-no-calendar', self.basel_fixture()))
        cases = []
        for name, (graph, index, request) in fixtures:
            case = {'name': name, 'graph': graph, 'request': request}
            try:
                case['expected'] = compare_temporal_claims(graph, request, graph_index=index)
            except TemporalReadModelInvalid as error:
                case.update(error_status=503, error=str(error))
            cases.append(case)
        return cases

    def test_transport_controls_preserve_json_types_and_invalid_issue_states(self):
        cases = {case['name']: case for case in self.transport_cases()}
        self.assertEqual(cases['number-spelling']['expected']['comparison']['status'], 'comparable')
        self.assertEqual(cases['bool-number']['expected']['comparison']['status'], 'undetermined')
        validator = Draft202012Validator(self.schemas['temporal-comparison-result.v1.schema.json'], registry=self.registry)
        for name, case in cases.items():
            with self.subTest(name=name):
                if 'error_status' in case:
                    self.assertEqual(case['error_status'], 503)
                    self.assertNotIn('expected', case)
                    self.assertIn('invalid structural containers', case['error'])
                    continue
                validator.validate(case['expected'])
                self.assertFalse(case['expected']['authority_boundary']['creates_inferred_claim'])
                if name.startswith('issues-'):
                    self.assertEqual(case['expected']['comparison']['status'], 'unsupported')
                    self.assertIn({'side': 'left', 'code': 'temporal-normalization-issues-invalid'},
                                  case['expected']['comparison']['reasons'])
                if name.endswith('-with-keys') or name in ('kind-array', 'calendar-array'):
                    self.assertNotEqual(case['expected']['comparison']['status'], 'comparable')
                    self.assertIsNone(case['expected']['comparison']['relation'])

    def test_core_cli_and_loopback_http_use_the_same_packet_and_errors(self):
        from tos_access.cli import main
        from tos_access.http_server import build_handler
        graph, _, request = self.fixture()
        core = ToSAccessCore.discover(tos_root=ROOT)
        with patch.object(ToSAccessCore, 'knowledge_graph', return_value=graph), \
                patch.object(ToSAccessCore, 'discover', return_value=core):
            expected = core.knowledge_temporal_compare(request)
            prepared_index = core._graph_index
            output = io.StringIO()
            with patch('sys.stdin', io.StringIO(json.dumps(request))), redirect_stdout(output):
                main(['--root', str(ROOT), 'knowledge', 'temporal-compare', '-'])
            self.assertEqual(json.loads(output.getvalue()), expected)
            self.assertIs(core._graph_index, prepared_index)
            server = ThreadingHTTPServer(('127.0.0.1', 0), build_handler(core, ACCESS / 'web/dist'))
            thread = Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                cases = [(request, 200), ({**request, 'source_revision': '0' * 64}, 409),
                         ({**request, 'left': {**request['left'], 'content_revision': '0' * 64}}, 409),
                         ({**request, 'left': {**request['left'], 'node_id': 'not-an-alias'}}, 404),
                         ({**request, 'calendar': 'gregorian'}, 400), ([], 400)]
                for value, status in cases:
                    with self.subTest(status=status, value=value):
                        connection = HTTPConnection('127.0.0.1', server.server_port, timeout=5)
                        try:
                            connection.request('POST', '/api/knowledge/temporal/compare', json.dumps(value),
                                               {'Content-Type': 'application/json'})
                            response = connection.getresponse()
                            packet = json.loads(response.read())
                            self.assertEqual(response.status, status)
                            if status == 200: self.assertEqual(packet, expected)
                        finally:
                            connection.close()
                self.assertIs(core._graph_index, prepared_index)
                next(node for node in graph['nodes'] if node['id'] == request['left']['node_id'])['attributes'] = None
                connection = HTTPConnection('127.0.0.1', server.server_port, timeout=5)
                try:
                    connection.request('POST', '/api/knowledge/temporal/compare', json.dumps(request),
                                       {'Content-Type': 'application/json'})
                    response = connection.getresponse()
                    self.assertEqual(response.status, 503)
                    self.assertIn('invalid structural containers', json.loads(response.read())['error'])
                finally:
                    connection.close()
            finally:
                server.shutdown(); server.server_close(); thread.join(timeout=5)

    def test_native_mcp_reuses_one_path_bound_core_and_observes_changed_snapshot(self):
        from tos_access.mcp_server import build_server
        graph, _, request = self.fixture()
        next_graph = copy.deepcopy(graph)
        next_graph['source_revision'] = 'e' * 64
        first = ToSAccessCore.discover(tos_root=ROOT)
        same_paths = ToSAccessCore.discover(tos_root=ROOT)
        different_paths = ToSAccessCore.discover(tos_root=ROOT, index_path=ROOT / 'test-only-other-index.json')
        with patch.object(ToSAccessCore, 'discover', side_effect=[first, same_paths, same_paths, different_paths]), \
                patch.object(ToSAccessCore, 'knowledge_graph', return_value=graph) as read_graph:
            server = build_server(tos_root=ROOT)
            async def exercise():
                tools = await server.list_tools()
                self.assertIn('tos_knowledge_temporal_compare', [tool.name for tool in tools])
                content, structured = await server.call_tool('tos_knowledge_temporal_compare', {'request': request})
                self.assertEqual(structured, first.knowledge_temporal_compare(request))
                index = first._graph_index
                await server.call_tool('tos_knowledge_temporal_compare', {'request': request})
                self.assertIs(first._graph_index, index)
                self.assertIsNone(same_paths._graph_index)
                read_graph.return_value = next_graph
                with self.assertRaisesRegex(Exception, 'snapshot changed'):
                    await server.call_tool('tos_knowledge_temporal_compare', {'request': request})
                self.assertIs(first._graph_index.graph, next_graph)
                await server.call_tool('tos_knowledge_temporal_compare', {'request': {**request, 'source_revision': next_graph['source_revision']}})
                self.assertIs(different_paths._graph_index.graph, next_graph)
            asyncio.run(exercise())

    def test_strict_request_matches_discovered_schema(self):
        _, _, request = self.fixture()
        validator = Draft202012Validator(self.schemas['temporal-comparison-request.v1.schema.json'])
        self.assertTrue(validator.is_valid(request))
        cases = [None, [], {}, {**request, 'calendar': 'gregorian'}, {**request, 'schema_version': 'v2'}]
        for key, value in [('node_id', ''), ('node_id', ' x'), ('node_id', 'x '), ('node_id', 'x' * 1025),
                           ('content_revision', False), ('content_revision', 'unknown')]:
            cases.append({**request, 'left': {**request['left'], key: value}})
        cases.append({**request, 'left': {**request['left'], 'version': 1}})
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                normalize_temporal_comparison_request(value)
            self.assertFalse(validator.is_valid(value))

    def basel_fixture(self):
        path = ROOT / 'ToS/source-witnesses/relations/basel-biography-research/source-claims.jsonl'
        claims = {claim['claim_id']: claim for line in path.read_text().splitlines() if line.strip() for claim in [json.loads(line)]}
        selected = [claims['tos.claim.basel-research.' + suffix] for suffix in ('phase-dating', 'period-dating')]
        base = ROOT / 'ToS/source-witnesses/history/basel-research'
        subjects = [json.loads((base / path).read_text()) for path in (
            'basel-teaching/biographical-phase.json', 'basel-chair-turnover-period/historical-period.json')]
        return self.fixture(source_claims=selected, source_subjects={record['record_id']: record for record in subjects})

    def test_real_basel_claims_remain_unknown_without_invented_calendar(self):
        graph, index, request = self.basel_fixture()
        result = compare_temporal_claims(graph, request, graph_index=index)
        self.assertEqual(result['comparison']['status'], 'undetermined')
        self.assertIsNone(result['comparison']['relation'])
        for side in ('left', 'right'):
            source = index.node_ids[request[side]['node_id']][0]['attributes']['source_claim']
            self.assertEqual(result[side]['claim']['attributes']['source_claim'], source)
            self.assertIsNone(result[side]['normalized_time']['calendar'])
            self.assertNotIn('sort_start', result[side]['normalized_time'])


if __name__ == '__main__':
    unittest.main()

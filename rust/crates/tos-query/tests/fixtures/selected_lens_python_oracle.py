"""Independent maintained Python lens oracle over CMP's emitted normalized input.

No production corpus and no expected Rust output participates in this oracle.
The selected descriptor supplies the fixture's vocabulary policy constants.
"""
import base64
import copy
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[5] / 'access' / 'src'))
from tos_access import knowledge as k

graph, descriptor = json.load(sys.stdin)
k.KNOWLEDGE_SOURCES = tuple(s['source_graph_id'] for s in descriptor['sources'])
k._CARRIER_SOURCE_PRIORITY = {s['source_graph_id']: s['representative_priority'] for s in descriptor['sources']}
k.OVERVIEW_EXCLUDED_PREDICATES = set(descriptor['overview']['excluded_predicate_ids'])
k.OVERVIEW_EXCLUDED_RELATION_TYPES = set(descriptor['overview']['excluded_relation_type_ids'])
nodes = graph['nodes']
relations = graph['relations']
assert nodes and relations, 'must exercise genuine normalized producer carriers'
first = nodes[0]
cases = []

def case(name, **parts):
    spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': name, **parts}
    try:
        packet = k.execute_knowledge_lens(graph, spec)
    except ValueError as error:
        cases.append({'name': name, 'spec': spec, 'error': 'invalid', 'message': str(error)})
    else:
        cases.append({'name': name, 'spec': spec, 'packet': packet})
    return spec

def rule(field, op, value):
    return {'field': field, 'op': op, 'value': value}

case('default-lens')
case('coerced-bounds', limits={'nodes': ' +1_0 ', 'relations': 2.9, 'groups': '03'}, traversal={'depth': ' 0 '})
case('localized-title', title={'zz': 'Z-last', 'de': 'A-first'}, description={'original': 'Original'})
case('strict-strings', seed={'node_ids': [' ' + first['id'] + ' ']})
case('focus-exact', seed={'focus_node_id': first['id']}, node_query={'enabled': False}, explain=True)
case('focus-entity', seed={'focus_node_id': first['entity_id']}, node_query={'filters': [rule('id', 'eq', 'absent')]}, explain=True)
native = next((n for n in nodes if sum(other['native_id'] == n['native_id'] for other in nodes) == 1), None)
if native:
    case('focus-native', seed={'focus_node_id': native['native_id']}, node_query={'enabled': False})
for source in k.KNOWLEDGE_SOURCES:
    case('source-' + source, sources=[source])
for profile in ('all', 'overview'):
    for direction in ('outgoing', 'incoming', 'either'):
        case('traversal-' + profile + '-' + direction,
             seed={'focus_node_id': relations[0]['from_id']}, node_query={'enabled': False},
             traversal={'depth': 3, 'direction': direction, 'profile': profile}, explain=True)
for policy in ('both', 'either', 'independent'):
    case('endpoints-' + policy, seed={'node_ids': [relations[0]['from_id']]},
         composition={'endpoint_policy': policy}, limits={'nodes': 4, 'relations': 3}, explain=True)
for op, value in [('eq', None), ('neq', None), ('exists', False), ('in', [None, True, 1]),
                  ('contains', 'I'), ('prefix', 'I'), ('gt', 1), ('gte', 1), ('lt', 1), ('lte', 1)]:
    case('generic-missing-' + op, node_query={'filters': [rule('attributes.missing', op, value)]})
case('array-membership', node_query={'filters': [rule('source_refs', 'contains', first['source_refs'][:1])]})
case('array-equality', node_query={'filters': [rule('source_refs', 'eq', first['source_refs'][0])]})
case('any-identities', node_query={'match': 'any', 'filters': [rule('id', 'eq', first['id']), rule('native_id', 'eq', nodes[-1]['native_id'])]}, explain=True)
case('all-identities', node_query={'filters': [rule('id', 'in', [first['id'], nodes[-1]['id']]), rule('entity_id', 'exists', True)]})
case('seed-text', seed={'text_query': first['native_id']})
case('multi-sort-groups', composition={'sort_nodes': [{'field': 'source_graph', 'direction': 'desc'}, {'field': 'display.title.default', 'direction': 'asc'}],
                                     'sort_relations': [{'field': 'predicate_id', 'direction': 'desc'}],
                                     'group_by': ['source_graph', 'kind_id', 'graph_layers', 'attributes.missing']},
     limits={'nodes': 6, 'relations': 4, 'groups': 4}, explain=True)
case('compact-language', detail='compact', language='ru', explain=True)
case('original-language', language='original')
for direction in ('incoming', 'outgoing', 'either'):
    for quantifier in ('exists', 'not_exists'):
        case('path-' + direction + '-' + quantifier, path_query=[{
            'path_id': 'one-hop', 'quantifier': quantifier,
            'steps': [{'direction': direction, 'relation_query': {'filters': [rule('predicate_id', 'eq', relations[0]['predicate_id'])]}}]}], explain=True)
case('path-revisit', path_query=[{'path_id': 'return', 'steps': [{'direction': 'either'}, {'direction': 'either'}]}], explain=True)
for definition in graph.get('query_properties', []):
    applicable = next((n for n in nodes if n.get('type_id') in definition['applies_to'] or
                       definition['inherited'] and set(n.get('semantics', {}).get('type_ancestors', [])) & set(definition['applies_to'])), None)
    if applicable is None:
        continue
    actual = k._field(applicable, definition['field'])
    value = actual[0] if isinstance(actual, list) and actual else actual
    for op in definition['operators']:
        operand = True if op == 'exists' else value
        if operand is None or isinstance(operand, (dict, list)):
            continue
        if op == 'in':
            operand = [operand]
        if op == 'prefix' and not isinstance(operand, str):
            continue
        case('property-' + definition['property_id'] + '-' + op, node_query={'filters': [{'property_id': definition['property_id'], 'op': op, 'value': operand}]})
    case('property-unknown-' + definition['property_id'], node_query={'filters': [{'property_id': definition['property_id'], 'op': 'neq', 'value': value}]})

paged = case('paged-context', seed={'focus_node_id': first['id']}, composition={'group_by': ['source_graph']}, pagination={'nodes': 1, 'relations': 1}, explain=True)
packet = cases[-1]['packet']
page = 0
while packet['page']['has_more']:
    page += 1
    continuation = copy.deepcopy(paged)
    continuation['pagination']['cursor'] = packet['page']['next_cursor']
    packet = k.execute_knowledge_lens(graph, continuation)
    cases.append({'name': 'continuation-' + str(page), 'spec': continuation, 'packet': packet})
assert page > 0
bad_cursor = copy.deepcopy(paged)
token = {'v': 1, 'fingerprint': '0' * 64, 'n': 0, 'r': 0}
bad_cursor['pagination']['cursor'] = base64.urlsafe_b64encode(json.dumps(token).encode()).decode().rstrip('=')
cases.append({'name': 'stale-cursor', 'spec': bad_cursor, 'error': 'stale'})

for key in ('sources', 'detail', 'explain'):
    case('null-' + key, **{key: None})
for key in ('enabled', 'match', 'filters'):
    case('query-null-' + key, node_query={key: None})
case('page-null-size', pagination={'nodes': None})
case('unknown-property', node_query={'filters': [{'property_id': 'tos.property.fixture-absent', 'op': 'exists', 'value': True}]})
case('path-null-direction', path_query=[{'path_id': 'bad', 'steps': [{'direction': None}]}])
case('path-space-direction', path_query=[{'path_id': 'bad', 'steps': [{'direction': ' outgoing '}]}])
case('path-null-quantifier', path_query=[{'path_id': 'bad', 'quantifier': None, 'steps': [{}]}])
case('registered-space-selector', node_query={'filters': [{'property_id': ' tos.property.fixture ', 'op': 'exists', 'value': True}]})
json.dump({'cases': cases, 'oracle': 'tos_access.knowledge.execute_knowledge_lens',
           'python_source_sha256': hashlib.sha256(Path(k.__file__).read_bytes()).hexdigest()}, sys.stdout, ensure_ascii=False)

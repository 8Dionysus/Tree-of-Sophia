"""Small explicit catalog carriers; no corpus build or generated repository reads."""
import copy


def fixture():
    entities = {'registry_id': 'fixture-entity', 'types': [
        {'type_id': 'tos.entity.agent', 'parent_type_ids': []},
        {'type_id': 'fixture.writer', 'parent_type_ids': ['tos.entity.agent']},
        {'type_id': 'tos.entity.work', 'parent_type_ids': ['tos.entity.intellectual-object']},
        {'type_id': 'tos.entity.place', 'parent_type_ids': []}], 'property_definitions': []}
    relations = {'registry_id': 'fixture-relation', 'relations': [
        {'relation_type_id': 'tos.relation.authored-by', 'labels': {'en': 'By'},
         'definition': 'Fixture', 'domain_type_ids': ['tos.entity.work'],
         'range_type_ids': ['tos.entity.agent']}]}
    nodes = []
    for i in range(9):
        kind = 'agent' if i < 7 else 'work' if i == 7 else 'place'
        nodes.append({'id': f'n{i}', 'source_graph': 'fixture', 'kind_id': kind,
            'type_id': 'fixture.writer' if i < 7 else 'tos.entity.' + kind,
            'type_mapping': {'status': 'mapped'},
            'display': {'kind_label': {'en': f'Kind {i}'}, 'title': {'ru': f'Имя {i}', 'x_bad': 'No'},
                        'summary': {'en': ''}, 'summary_state': 'source',
                        'provenance': {'source_summary_available': i % 2 == 0}},
            'attributes': {'examples': [i, i, None, {'nested': True}, False],
                           'nested': {'a': 1.0, 'deep': {'value': 'é' * 181}},
                           'numeric': [1, 1.0, False, -0.0, 'é' * 177, 'sixth'],
                           'scalar': 'A' if i % 2 else 'a'},
            'epistemic': {'authority_layer': 'fixture'}, 'view_ids': ['A', 'a', 'A'] if i % 2 else ['a', 'A'],
            'graph_layers': ['main', 'main'],
            'semantics': {'claim': {'relation_type_id': 'tos.relation.authored-by'}} if i == 0 else {}})
    edges = []
    for i, ends in enumerate((('n7', 'n0'), ('n1', 'n1'), ('n8', 'n7'))):
        edges.append({'id': f'r{i}', 'source_graph': 'fixture', 'from_id': ends[0], 'to_id': ends[1],
            'predicate_id': 'authored_by', 'relation_type_id': 'tos.relation.authored-by',
            'predicate_mapping': {'status': None if i == 1 else 'mapped'},
            'display': {'label': {'en': f'By {i}'}, 'statement': {'ru': 'Текст'},
                        'explanation_state': 'fallback', 'provenance': {'source_explanation_available': False}},
            'attributes': {'value': [False, 0, 0.0, None, [], {}]},
            'view_ids': ['A', 'a'], 'graph_layers': ['main'], 'epistemic': {}})
    graph = {'schema': 'tos_knowledge_graph_v1', 'source_revision': 'fixture-before',
             'normalization_binding': {'fixture': True}, 'counts': {'owner_extension': {'keep': 7}},
             'authority_boundary': {'is_source': False}, 'nodes': nodes, 'relations': edges}
    return graph, {}, {}, entities, relations


def cases():
    base = fixture()
    yield 'base', base
    missing = copy.deepcopy(base)
    missing[0]['nodes'] = missing[0]['nodes'][1:]
    missing[0]['relations'] = missing[0]['relations'][1:]
    yield 'delete-first', missing
    promoted = copy.deepcopy(base)
    promoted[0]['nodes'] = promoted[0]['nodes'][6:]
    promoted[0]['relations'] = promoted[0]['relations'][2:]
    yield 'delete-six', promoted
    empty = copy.deepcopy(base)
    empty[0]['nodes'] = []
    empty[0]['relations'] = []
    yield 'empty', empty
    legacy = copy.deepcopy(base)
    yield 'legacy', (*legacy[:3], None, None)
    changed = copy.deepcopy(base)
    changed[0]['nodes'][0]['kind_id'] = 'place'
    changed[0]['nodes'][0]['type_id'] = 'tos.entity.place'
    changed[0]['nodes'][1]['kind_id'] = 'place'
    changed[0]['nodes'][1]['type_id'] = 'tos.entity.place'
    yield 'route-change', changed
    reorder = copy.deepcopy(base)
    reorder[0]['nodes'].reverse()
    reorder[0]['relations'].reverse()
    yield 'reverse-order', reorder
    ordered = copy.deepcopy(base)
    ordered[0]['counts']['ordered_extension'] = {'zeta': 1, 'alpha': {'omega': 2, 'beta': 3}}
    ordered[0]['authority_boundary']['ordered_extension'] = {'zeta': 1, 'alpha': 2}
    ordered[3]['types'][0]['labels'] = {'ru': 'Агент', 'en': 'Agent'}
    ordered[4]['relations'][0]['labels'] = {'ru': 'Авторство', 'en': 'Authorship'}
    ordered[0]['nodes'][0]['display']['kind_label'] = {'zz': 'Z', 'aa': 'A', 'en': 'E'}
    ordered[0]['relations'][0]['display']['label'] = {'ru': 'Автор', 'en': 'By'}
    ordered[1]['graph_views'] = [{'view_id': 'route-graph', 'title': 'Route'}]
    ordered[2]['views'] = [{'view_id': 'chronology', 'title': 'Chronology', 'layout_hint': 'timeline'}]
    yield 'ordered-owner-values', ordered

"""Stateless, revision-bound delivery pages of an already bounded LensResult.

This is not a corpus traversal scheduler: each page re-executes the bounded
lens. The public token is a validated seek position, not an authorization token.
"""
from __future__ import annotations

import base64
import binascii
import json
import re


class KnowledgeRevisionConflict(ValueError):
    pass


def normalize_pagination(value):
    if value is None:
        return None
    if not isinstance(value, dict) or set(value) - {'nodes', 'relations', 'cursor'}:
        raise ValueError('pagination must contain only nodes, relations, cursor')
    sizes = {}
    for key, default in (('nodes', 40), ('relations', 80)):
        size = value.get(key, default)
        if type(size) is not int or not 1 <= size <= 100:
            raise ValueError(f'pagination.{key} must be between 1 and 100')
        sizes[key] = size
    cursor = value.get('cursor')
    if cursor is not None and (not isinstance(cursor, str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,512}', cursor)):
        raise ValueError('invalid lens cursor')
    return {**sizes, 'cursor': cursor}


def paginate_lens(result):
    options = result['lens']['pagination']
    if options is None:
        return result
    node_offset = relation_offset = 0
    if options['cursor'] is not None:
        try:
            token = json.loads(base64.urlsafe_b64decode(options['cursor'] + '=' * (-len(options['cursor']) % 4)))
        except (ValueError, UnicodeError, binascii.Error) as exc:
            raise ValueError('invalid lens cursor') from exc
        if not isinstance(token, dict) or set(token) != {'v', 'fingerprint', 'n', 'r'} or type(token.get('v')) is not int or token['v'] != 1:
            raise ValueError('invalid lens cursor')
        if not isinstance(token['fingerprint'], str) or not re.fullmatch('[0-9a-f]{64}', token['fingerprint']):
            raise ValueError('invalid lens cursor')
        for key in ('n', 'r'):
            if type(token[key]) is not int or not 0 <= token[key] <= 2000:
                raise ValueError('invalid lens cursor position')
        if token['fingerprint'] != result['fingerprint']:
            raise KnowledgeRevisionConflict('lens query or snapshot changed; restart pagination')
        node_offset, relation_offset = token['n'], token['r']
        if node_offset > len(result['nodes']) or relation_offset > len(result['relations']):
            raise ValueError('invalid lens cursor position')
    nodes, relations = result['nodes'], result['relations']
    primary = nodes[node_offset:node_offset + options['nodes']]
    selected_relations = relations[relation_offset:relation_offset + options['relations']]
    primary_ids = {node['id'] for node in primary}
    selected_ids = primary_ids | {r[key] for r in selected_relations for key in ('from_id', 'to_id')}
    if result['focus'] is not None:
        selected_ids.add(result['focus']['node_id'])
    selected_nodes = [node for node in nodes if node['id'] in selected_ids]
    next_n, next_r = node_offset + len(primary), relation_offset + len(selected_relations)
    has_more = next_n < len(nodes) or next_r < len(relations)
    token = {'v': 1, 'fingerprint': result['fingerprint'], 'n': next_n, 'r': next_r}
    next_cursor = base64.urlsafe_b64encode(json.dumps(token, separators=(',', ':')).encode()).decode().rstrip('=') if has_more else None
    relation_ids = {r['id'] for r in selected_relations}
    groups = []
    for group in result['groups']:
        group_nodes = [id for id in group['node_ids'] if id in selected_ids]
        group_relations = [id for id in group['relation_ids'] if id in relation_ids]
        if group_nodes or group_relations:
            groups.append({**group, 'node_ids': group_nodes, 'relation_ids': group_relations,
                           'node_count': len(group_nodes), 'relation_count': len(group_relations)})
    result = {**result, 'nodes': selected_nodes, 'relations': selected_relations, 'groups': groups,
              'page': {'next_cursor': next_cursor, 'has_more': has_more,
                       'primary_node_ids': [n['id'] for n in primary],
                       'context_node_ids': [n['id'] for n in selected_nodes if n['id'] not in primary_ids],
                       'returned_nodes': len(selected_nodes), 'returned_relations': len(selected_relations),
                       'scope': 'bounded-lens-result', 'counts_scope': 'complete-bounded-result'}}
    if 'inclusion' in result:
        result['inclusion'] = {**result['inclusion'],
                               'nodes': {k: v for k, v in result['inclusion']['nodes'].items() if k in selected_ids},
                               'relations': {k: v for k, v in result['inclusion']['relations'].items() if k in relation_ids}}
    return result

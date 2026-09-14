"""Exact, typed exploration origins. An origin is not a new graph identity."""
from __future__ import annotations

import re

from .lens_pagination import KnowledgeRevisionConflict

REQUEST_V2 = 'tos_exploration_request_v2'
RESULT_V2 = 'tos_exploration_result_v2'


class ExplorationReadModelInvalid(RuntimeError):
    """An origin cannot be read intact from the prepared knowledge snapshot."""


def _identifier(value):
    return isinstance(value, str) and 0 < len(value) <= 1024 and value == value.strip()


def _revision(value):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None


def normalize_origin(request):
    if request.get('schema_version') != REQUEST_V2 or not _revision(request.get('source_revision')):
        raise ValueError('exploration v2 requires its schema version and exact source_revision')
    origin = request.get('origin')
    if (not isinstance(origin, dict) or set(origin) != {'kind', 'id', 'content_revision'}
            or not isinstance(origin.get('kind'), str) or origin['kind'] not in ('node', 'relation')
            or not _identifier(origin.get('id')) or not _revision(origin.get('content_revision'))):
        raise ValueError('exploration origin requires kind, exact id and content_revision')
    return dict(origin)


def require_origin_carrier(item, kind):
    """Reject corrupt containers, never repair a prepared carrier for delivery.

    The graph builder owns complete normalized-node/relation validation. These
    bounded checks protect origin binding and the containers consumed here;
    they do not replace that builder or adjudicate unknown semantic contents.
    """
    if not isinstance(item, dict):
        raise ExplorationReadModelInvalid('exploration origin carrier is not an object')
    fields = ('id', 'native_id', 'source_graph') + (
        ('entity_id', 'kind_id', 'type_id') if kind == 'node'
        else ('from_id', 'to_id', 'predicate_id', 'relation_type_id'))
    for name in fields:
        if not isinstance(item.get(name), str) or not item[name]:
            raise ExplorationReadModelInvalid(f'exploration origin carrier has invalid {name}')
    if not _revision(item.get('content_revision')):
        raise ExplorationReadModelInvalid('exploration origin carrier has invalid content_revision')
    for name in ('attributes', 'semantics', 'display', 'epistemic',
                 'type_mapping' if kind == 'node' else 'predicate_mapping'):
        if not isinstance(item.get(name), dict):
            raise ExplorationReadModelInvalid(f'exploration origin carrier has invalid {name} container')
    for name in ('source_refs', 'graph_layers', 'view_ids'):
        value = item.get(name)
        if not isinstance(value, list) or any(not isinstance(v, str) or not v for v in value):
            raise ExplorationReadModelInvalid(f'exploration origin carrier has invalid {name}')
        if name == 'source_refs' and not value:
            raise ExplorationReadModelInvalid('exploration origin carrier has no source_refs')
    for name in ('claim', 'time', 'space', 'responsibility', 'annotation', 'language_context', 'record_version'):
        if name in item['semantics'] and not isinstance(item['semantics'][name], dict):
            raise ExplorationReadModelInvalid(f'exploration origin carrier has invalid semantics.{name}')


def bind_origin(query, source_revision, lookup_node, lookup_relation):
    """Read at most one selected relation and its two exact endpoint carriers.

    Lookup functions return an exact unambiguous carrier or None; they never
    resolve entity/native aliases. The owner retains immutable snapshots.
    """
    if query['source_revision'] != source_revision:
        raise KnowledgeRevisionConflict('exploration source revision changed; select the origin again')
    requested = query['origin']
    item = (lookup_node if requested['kind'] == 'node' else lookup_relation)(requested['id'])
    if item is None:
        raise KeyError('unknown or ambiguous exact exploration origin')
    require_origin_carrier(item, requested['kind'])
    if item['id'] != requested['id']:
        raise ExplorationReadModelInvalid('exploration origin lookup returned a different id')
    if item['content_revision'] != requested['content_revision']:
        raise KnowledgeRevisionConflict('exploration origin content revision changed')
    if item['source_graph'] not in query['sources']:
        raise ValueError('exploration sources exclude the selected origin')
    resolved = dict(requested)
    if requested['kind'] == 'node':
        return resolved, [item['id']], []
    endpoints = {}
    roots = []
    cached = {}
    for role, field in (('from', 'from_id'), ('to', 'to_id')):
        identifier = item[field]
        if identifier not in cached:
            cached[identifier] = lookup_node(identifier)
        node = cached[identifier]
        if node is None:
            raise ExplorationReadModelInvalid('exploration relation endpoint is missing or ambiguous')
        require_origin_carrier(node, 'node')
        if node['id'] != identifier:
            raise ExplorationReadModelInvalid('exploration endpoint lookup returned a different id')
        if node['source_graph'] not in query['sources']:
            raise ValueError('exploration sources exclude an origin endpoint')
        endpoints[role] = {name: node[name] for name in ('content_revision', 'entity_id')}
        endpoints[role]['node_id'] = identifier
        if identifier not in roots:
            roots.append(identifier)
    resolved['endpoints'] = endpoints
    return resolved, roots, [item['id']]

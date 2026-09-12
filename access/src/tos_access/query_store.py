"""Read-only, snapshot-bound queries over the explicitly compiled SQLite store.

Opening a store never creates a file or rebuilds projections. JSON substring
search preserves the legacy grammar; arbitrary metadata filters can scan disk
rows, but returned payloads and traversal state remain bounded in memory.
"""
from __future__ import annotations

import heapq
import json
import sqlite3
from collections import Counter
from collections.abc import Mapping, Sequence
from contextlib import contextmanager
from pathlib import Path

from . import knowledge as k
from .lens_pagination import paginate_lens

SCHEMA = 'tos_query_store_v1'
DEFAULT_RELATIVE_PATH = Path('ToS/derived-exports/runtime/knowledge.sqlite3')
# This is an explicit compatibility boundary for the row-model ABI.  A store
# that has the right tables but was compiled by a different semantic compiler
# must be rebuilt before it is served.
COMPILER_VERSION = 'tos_offline_knowledge_v1'


class QueryStoreRequired(RuntimeError):
    """A complete explicit offline build is required before serving queries."""


class QueryStore:
    def __init__(self, path, *, snapshot_bindings=None):
        self.path = Path(path).resolve()
        if not self.path.is_file():
            raise QueryStoreRequired(f'query store build required: missing {self.path}; run the explicit offline query-store builder')
        self.expected_bindings = snapshot_bindings
        stat = self.path.stat()
        self.file_identity = (stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns)
        with self.connect() as db:
            try:
                self.metadata = {key: json.loads(value) for key, value in db.execute('SELECT key,value FROM metadata')}
            except (sqlite3.Error, ValueError) as exc:
                raise QueryStoreRequired('query store build required: invalid metadata') from exc
        if (self.metadata.get('schema') != SCHEMA
                or self.metadata.get('compiler_version') != COMPILER_VERSION
                or self.metadata.get('complete') is not True):
            raise QueryStoreRequired('query store build required: unsupported or incomplete snapshot')
        if snapshot_bindings is not None and self.metadata.get('snapshot_bindings') != snapshot_bindings:
            raise QueryStoreRequired('query store build required: stale snapshot bindings')
        if not isinstance(self.metadata.get('graph_header'), dict) or not isinstance(self.metadata.get('catalog'), dict) or not isinstance(self.metadata.get('exploration_revision'), str):
            raise QueryStoreRequired('query store build required: missing completed snapshot headers')
        self.header = self.metadata['graph_header']
        self.revision = self.metadata['exploration_revision']

    @contextmanager
    def connect(self):
        try:
            stat = self.path.stat()
            if (stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns) != self.file_identity:
                raise QueryStoreRequired('query store snapshot changed during request; retry against current snapshot')
            if any(Path(str(self.path) + suffix).exists() for suffix in ('-wal', '-journal')):
                raise QueryStoreRequired('query store must be a completed immutable snapshot, not a live journaled database')
            db = sqlite3.connect(self.path.as_uri() + '?mode=ro&immutable=1', uri=True)
            # Opening by pathname is a second filesystem operation after the
            # initial identity check.  An atomic compiler publish can replace
            # the path in that window, so force SQLite to read the opened
            # database and then bind the connection to the same snapshot
            # identity before handing it to a query.
            db.execute('PRAGMA schema_version').fetchone()
            db.execute('PRAGMA query_only=ON')
            db.execute('PRAGMA cache_size=-8192')
            db.execute('PRAGMA temp_store=FILE')
            db.create_function('tos_contains', 2, lambda payload, needle: int(_contains(json.loads(payload), needle)), deterministic=True)
            db.create_function('tos_lower', 1, lambda value: str(value or '').lower(), deterministic=True)
            db.create_function('tos_match', 2, lambda payload, group: int(k._matches_group(json.loads(payload), json.loads(group))), deterministic=True)
            db.create_function('tos_sort', 2, lambda payload, field: str(k._field(json.loads(payload), field) or '').lower(), deterministic=True)
            opened_stat = self.path.stat()
            opened_identity = (opened_stat.st_ino, opened_stat.st_size,
                               opened_stat.st_mtime_ns, opened_stat.st_ctime_ns)
            if opened_identity != self.file_identity:
                raise QueryStoreRequired('query store snapshot changed during request; retry against current snapshot')
            yield db
        except (sqlite3.Error, OSError) as exc:
            raise QueryStoreRequired(f'query store cannot serve this snapshot: {exc}; explicit offline rebuild required') from exc
        finally:
            if 'db' in locals():
                db.close()

    def rows(self, table, where='1', params=(), *, order='id', limit=None, offset=0, column='payload'):
        # Every SQL fragment is internal; caller values always use parameters.
        sql = f'SELECT {column} FROM {table} WHERE {where} ORDER BY {order}'
        if limit is not None:
            sql += ' LIMIT ? OFFSET ?'
            params = (*params, limit, offset)
        with self.connect() as db:
            for row in db.execute(sql, params):
                yield json.loads(row[0]) if column == 'payload' else row[0]

    def count(self, table, where='1', params=()):
        with self.connect() as db:
            return db.execute(f'SELECT count(*) FROM {table} WHERE {where}', params).fetchone()[0]

    @staticmethod
    def membership(field, values):
        values = list(values)
        return (f'{field} IN ({",".join("?" for _ in values)})' if values else '0', values)

    def incident(self, ids, *, table='knowledge_relations', extra='1', params=(), limit=None):
        left, ids = self.membership('from_id', ids)
        right, _ = self.membership('to_id', ids)
        return self.rows(table, f'({left} OR {right}) AND ({extra})', (*ids, *ids, *params), limit=limit)

    def resolve(self, identifier, *, sources=None, relation=False):
        table = 'knowledge_relations' if relation else 'knowledge_nodes'
        source_sql, source_values = self.membership('source_graph', sources) if sources is not None else ('1', [])
        for field in (('id', 'native_id') if relation else ('id', 'entity_id', 'native_id')):
            matches = list(self.rows(table, f'{field}=? AND ({source_sql})', [identifier, *source_values]))
            if matches:
                return field, matches
        return None, []

    def focus_node(self, identifier, sources):
        if identifier is None:
            return None
        _, matches = self.resolve(identifier, sources=sources)
        return k._resolve_focus_node(matches, identifier)

    def text_candidates(self, table, needle):
        """Indexed superset only; exact substring verification always follows.

        Already Python-lowered JSON preserves Unicode behavior independently of
        SQLite's case folding. Individual quoted trigrams work with detail=none
        and treat punctuation/quotes as data, never FTS query grammar.
        """
        if (self.metadata.get('search_accelerator', {}).get('mode') != 'fts5-trigram'
                or len(needle) < 3 or '\0' in needle):
            return '1', []
        if table not in ('knowledge_nodes', 'knowledge_relations'):
            raise ValueError('unsupported text candidate table')
        grams = sorted({needle[index:index + 3] for index in range(len(needle) - 2)})
        expression = ' AND '.join('"' + gram.replace('"', '""') + '"' for gram in grams)
        index = table + '_trigram'
        return f'rowid IN (SELECT rowid FROM {index} WHERE {index} MATCH ?)', [expression]

    def search(self, query='', *, sources=None, kind_ids=None, predicate_ids=None, offset=0, limit=40):
        query = str(query).strip()
        if len(query) > 256:
            raise ValueError('knowledge search query exceeds 256 characters')
        offset = k._bounded_integer(offset, 'offset', 0, 0, 100_000)
        limit = k._bounded_integer(limit, 'limit', 40, 1, 100)
        sources = k._normalized_source_filter(sources)
        kinds, predicates = set(k._strings(kind_ids)), set(k._strings(predicate_ids))
        if len(kinds) > 100 or len(predicates) > 100:
            raise ValueError('knowledge search kind and predicate filters must contain at most 100 values')
        needle = query.lower()
        selected, counts = {}, {}
        for kind, filters, field in [('nodes', kinds, 'kind_id'), ('relations', predicates, 'predicate_id')]:
            where, params = self.membership('source_graph', sorted(sources))
            if filters:
                clause, values = self.membership(field, sorted(filters))
                where += ' AND ' + clause
                params += values
            if needle:
                candidate_sql, candidate_params = self.text_candidates('knowledge_' + kind, needle)
                where += ' AND (' + candidate_sql + ')'
                params += candidate_params
                where += ' AND instr(search_text, ?) > 0'
                params.append(needle)
            # SQLite lower is ASCII-only. The registered function deliberately
            # follows Python Unicode lower, including the established tie order.
            rank = "CASE WHEN ? IN (tos_lower(id),tos_lower(native_id),tos_lower(label)) THEN 0 WHEN substr(tos_lower(id),1,length(?))=? OR substr(tos_lower(native_id),1,length(?))=? OR substr(tos_lower(label),1,length(?))=? THEN 1 ELSE 2 END"
            order = rank + ',tos_lower(id),id' if needle else 'tos_lower(id),id'
            counts[kind] = self.count('knowledge_' + kind, where, params)
            selected[kind] = list(self.rows('knowledge_' + kind, where, [*params, *([needle] * 7 if needle else [])], order=order, limit=limit, offset=offset))
        return {'schema': 'tos_knowledge_search_v1', 'source_revision': self.header['source_revision'],
                'query': query, 'filters': {'sources': sorted(sources), 'kind_ids': sorted(kinds), 'predicate_ids': sorted(predicates)},
                'page': {'offset': offset, 'limit_per_kind': limit},
                'counts': {'matching_nodes': counts['nodes'], 'matching_relations': counts['relations'],
                           'returned_nodes': len(selected['nodes']), 'returned_relations': len(selected['relations'])},
                **selected, 'authority_boundary': self.header.get('authority_boundary', {})}

    def inspect_node(self, identifier, relation_limit=200):
        identifier = str(identifier).strip()
        if not identifier:
            raise ValueError('knowledge node id is required')
        field, matches = self.resolve(identifier)
        if not matches:
            raise KeyError(f'unknown ToS knowledge node: {identifier}')
        limit = k._bounded_integer(relation_limit, 'relation_limit', 200, 0, 1000)
        ids = [item['id'] for item in matches]
        left, values = self.membership('from_id', ids)
        right, _ = self.membership('to_id', ids)
        count = self.count('knowledge_relations', f'({left} OR {right})', values * 2)
        relations = list(self.incident(ids, limit=limit))
        return {'schema': 'tos_knowledge_node_packet_v1', 'source_revision': self.header['source_revision'],
                'requested_id': identifier, 'ambiguous_native_id': field == 'native_id' and len(matches) > 1,
                'shared_entity_id': field == 'entity_id' and len(matches) > 1, 'matches': matches,
                'related_relations': relations, 'counts': {'matches': len(matches), 'related_relations': count, 'returned_relations': len(relations)},
                'source_refs': sorted({ref for item in [*matches, *relations] for ref in k._strings(item.get('source_refs'))}),
                'authority_boundary': self.header.get('authority_boundary', {})}

    def inspect_relation(self, identifier):
        identifier = str(identifier).strip()
        if not identifier:
            raise ValueError('knowledge relation id is required')
        field, matches = self.resolve(identifier, relation=True)
        if not matches:
            raise KeyError(f'unknown ToS knowledge relation: {identifier}')
        where, values = self.membership('id', {endpoint for item in matches for endpoint in (item['from_id'], item['to_id'])})
        endpoints = list(self.rows('knowledge_nodes', where, values))
        return {'schema': 'tos_knowledge_relation_packet_v1', 'source_revision': self.header['source_revision'],
                'requested_id': identifier, 'ambiguous_native_id': field == 'native_id' and len(matches) > 1,
                'matches': matches, 'endpoints': endpoints, 'counts': {'matches': len(matches), 'endpoints': len(endpoints)},
                'source_refs': sorted({ref for item in [*matches, *endpoints] for ref in k._strings(item.get('source_refs'))}),
                'authority_boundary': self.header.get('authority_boundary', {})}

    def selection(self, table, sources, group, *, sorts=None, extra='1', params=()):
        source_sql, source_values = self.membership('source_graph', sources)
        where = f'({source_sql}) AND ({extra})'
        values = [*source_values, *params]
        if not group['enabled']:
            where += ' AND 0'
        elif group['filters']:
            clause, args = filter_sql(table, group)
            where += ' AND (' + clause + ')'
            values.extend(args)
        return SQLSelection(self, table, where, values, sorts)

    def execute_lens(self, spec):
        return execute_store_lens(self, spec)

    def focus(self, node_id, **kwargs):
        # Reuse the existing authoritative focus-spec construction, substituting
        # only its executor through a small explicit callable adapter.
        return self.execute_lens(focus_spec(node_id, **kwargs))

    def raw(self, collection, *, where='1', params=(), limit=None):
        return self.rows('raw_records', f'collection=? AND ({where})', [collection, *params], order='position', limit=limit)

    def corpus_payload(self, collections=(), *, limits=None):
        payload = dict(self.metadata['corpus_header'])
        for name in collections:
            payload[name] = list(self.raw('corpus/' + name, limit=(limits or {}).get(name)))
        return payload

    def corpus_search(self, query, limit, resource_kind=None):
        results = []
        for collection in ('nodes', 'resources', 'manifests', 'branches', 'graph_views'):
            where, values = '1', []
            if query.lower().strip():
                where += ' AND tos_contains(payload, ?)'
                values.append(query.lower().strip())
            if resource_kind:
                where += " AND json_extract(payload,'$.resource_kind')=?"
                values.append(resource_kind)
            results.extend({'collection': collection, 'item': item} for item in self.raw('corpus/' + collection, where=where, params=values, limit=limit - len(results)))
            if len(results) >= limit:
                break
        return results

    def source_nodes(self):
        return RowMapping(self, 'source_nodes', where="coalesce(packet_id,'')='' ")

    def source_edges(self, direction='either', semantic=False):
        return Adjacency(self, table='source_edges', direction=direction, semantic=semantic)

    def rights(self, ids):
        where, values = self.membership('scope_id', ids)
        return list(self.rows('source_rights', f'id IN (SELECT right_id FROM source_rights_scopes WHERE {where})', values))


class RowMapping(Mapping):
    def __init__(self, store, table='knowledge_nodes', *, where='1', params=()):
        self.store, self.table, self.where, self.params = store, table, where, params

    def __getitem__(self, identifier):
        item = next(self.store.rows(self.table, f'id=? AND ({self.where})', (identifier, *self.params), limit=1), None)
        if item is None:
            raise KeyError(identifier)
        return item

    def __iter__(self):
        return self.store.rows(self.table, self.where, self.params, column='id')

    def __len__(self):
        return self.store.count(self.table, self.where, self.params)


class SQLSequence(Sequence):
    """An indexed adjacency list; only requested offsets enter Python memory."""
    def __init__(self, store, table, where, params, *, column='payload', order='id'):
        self.store, self.table, self.where, self.params, self.column, self.order = store, table, where, params, column, order

    def __len__(self):
        return self.store.count(self.table, self.where, self.params)

    def __getitem__(self, index):
        if isinstance(index, slice):
            start, stop, step = index.indices(len(self))
            return [self[i] for i in range(start, stop, step)]
        if index < 0:
            index += len(self)
        if index < 0:
            raise IndexError(index)
        value = next(self.store.rows(self.table, self.where, self.params, column=self.column, order=self.order, limit=1, offset=index), None)
        if value is None:
            raise IndexError(index)
        return value

    def __iter__(self):
        return self.store.rows(self.table, self.where, self.params, column=self.column, order=self.order)


class Adjacency:
    def __init__(self, store, *, table='knowledge_relations', direction='either', semantic=False, ids=False, sources=None):
        self.store, self.table, self.direction, self.semantic, self.ids, self.sources = store, table, direction, semantic, ids, sources

    def __getitem__(self, identifier):
        clauses = {'either': '(from_id=? OR to_id=?)', 'outgoing': 'from_id=?', 'incoming': 'to_id=?'}
        where = clauses[self.direction]
        params = [identifier] * (2 if self.direction == 'either' else 1)
        if self.semantic:
            where += " AND (edge_kind='authored_item_manifest' OR (edge_kind='evidence_claim' AND predicate_id IN ('has_expression','embodied_by','exemplified_by','described_by','metadata_at','downloadable_at','rights_statement_at')))"
        if self.sources is not None:
            clause, values = self.store.membership('source_graph', self.sources)
            where += ' AND ' + clause
            params += values
        return SQLSequence(self.store, self.table, where, params, column='id' if self.ids else 'payload')

    def get(self, identifier, default=None):
        return self[identifier]


class CarrierGroups:
    def __init__(self, store, sources=None):
        self.store, self.sources = store, sources

    def get(self, entity, default=None):
        if not isinstance(entity, str) or not entity.startswith('tos.'):
            return default
        where, params = 'entity_id=?', [entity]
        if self.sources is not None:
            clause, values = self.store.membership('source_graph', self.sources)
            where += ' AND ' + clause
            params += values
        return SQLSequence(self.store, 'knowledge_nodes', where, params, column='id')

    def __contains__(self, entity):
        return bool(self.get(entity, []))

    def __getitem__(self, entity):
        return self.get(entity, [])


class SQLSelection:
    def __init__(self, store, table, where, params, sorts=None):
        self.store, self.table, self.where, self.params = store, table, where, list(params)
        order, self.order_params = [], []
        columns = {'id','native_id','source_graph','kind_id','type_id','entity_id'} if table == 'knowledge_nodes' else {'id','native_id','source_graph','from_id','to_id','predicate_id','relation_type_id'}
        label_field = 'display.title.default' if table == 'knowledge_nodes' else 'display.label.default'
        for rule in sorts or []:
            if rule['field'] in columns:
                expression = 'tos_lower(' + rule['field'] + ')'
            elif rule['field'] == label_field:
                expression = 'tos_lower(label)'
            else:
                expression = 'tos_sort(payload, ?)'
                self.order_params.append(rule['field'])
            order.append(expression + ' ' + rule['direction'])
        self.order = ','.join([*order, 'id'])

    def __iter__(self):
        return self.store.rows(self.table, self.where, [*self.params, *self.order_params], order=self.order)

    def __len__(self):
        return self.store.count(self.table, self.where, self.params)

    def incident(self, ids):
        left, values = self.store.membership('from_id', ids)
        right, _ = self.store.membership('to_id', ids)
        return self.store.rows(self.table, f'({self.where}) AND ({left} OR {right})', [*self.params, *values, *values, *self.order_params], order=self.order)


def execute_store_lens(store, spec_value) -> dict[str, Any]:
    graph = store.header
    public_spec = k.normalize_lens_spec(spec_value)
    spec = k._bind_query_properties(graph, public_spec)
    sources = set(spec['sources'])
    source_where, source_params = store.membership('source_graph', sorted(sources))
    all_nodes_by_id = RowMapping(store, where=source_where, params=source_params)
    carrier_groups = CarrierGroups(store, sources) if spec['traversal']['profile'] == 'overview' else {}
    focus_node = store.focus_node(spec['seed']['focus_node_id'], sources)
    selected_ids = set(spec['seed']['node_ids'])
    extra, values = '1', []
    if selected_ids:
        clauses = []
        for field in ('id','native_id','entity_id'):
            clause, args = store.membership(field, sorted(selected_ids))
            clauses.append(clause)
            values.extend(args)
        extra = '(' + ' OR '.join(clauses) + ')'
    if spec['seed']['text_query']:
        candidate_sql, candidate_params = store.text_candidates('knowledge_nodes', str(spec['seed']['text_query']).lower())
        extra += ' AND (' + candidate_sql + ')'
        values += candidate_params
        extra += ' AND instr(search_text,?)>0'
        values.append(str(spec['seed']['text_query']).lower())
    candidates = store.selection('knowledge_nodes', sources, spec['node_query'],
                                 sorts=spec['composition']['sort_nodes'], extra=extra, params=values)
    adjacency = Adjacency(store, sources=sources)
    path_budget = [100_000]
    path_proofs = {}
    matched_node_count = 0
    focus_matched = False
    sorted_nodes = []
    candidate_rows = candidates if spec['path_query'] else store.rows(candidates.table, candidates.where, [*candidates.params, *candidates.order_params], order=candidates.order, limit=spec['limits']['nodes'])
    for node in candidate_rows:
        proofs = []
        for condition in spec['path_query']:
            witness = k._path_witness(node['id'], condition, all_nodes_by_id, adjacency, path_budget)
            if (witness is not None) != (condition['quantifier'] == 'exists'):
                break
            proofs.append(witness or {'path_id': condition['path_id'], 'absence_in_scope': True})
        else:
            matched_node_count += 1
            focus_matched = focus_matched or (focus_node is not None and node['id'] == focus_node['id'])
            if len(sorted_nodes) < spec['limits']['nodes']:
                sorted_nodes.append(node)
                path_proofs[node['id']] = proofs
    if not spec['path_query']:
        matched_node_count = len(candidates)
        if focus_node is not None:
            focus_matched = bool(store.count(candidates.table, '(' + candidates.where + ') AND id=?', [*candidates.params, focus_node['id']]))
    selected_nodes: dict[str, dict[str, Any]] = {}
    inclusion = {}
    if focus_node is not None:
        selected_nodes[str(focus_node["id"])] = focus_node
        inclusion[focus_node['id']] = {'kind': 'focus'}
    for item in sorted_nodes:
        if len(selected_nodes) >= spec["limits"]["nodes"]:
            break
        selected_nodes.setdefault(str(item["id"]), item)
        inclusion.setdefault(item['id'], {'kind': 'selector', 'path_witnesses': path_proofs.get(item['id'], [])})

    extra, values = '1', []
    if spec['traversal']['profile'] == 'overview':
        for field, excluded in [('predicate_id', k.OVERVIEW_EXCLUDED_PREDICATES),
                                ('relation_type_id', k.OVERVIEW_EXCLUDED_RELATION_TYPES)]:
            clause, args = store.membership("coalesce(" + field + ",'')", sorted(excluded))
            extra += ' AND NOT (' + clause + ')'
            values.extend(args)
    if spec['traversal']['predicate_ids']:
        clause, args = store.membership('predicate_id', spec['traversal']['predicate_ids'])
        extra += ' AND ' + clause
        values.extend(args)
    relation_candidates = store.selection('knowledge_relations', sources, spec['relation_query'],
                                          sorts=spec['composition']['sort_relations'], extra=extra, params=values)
    frontier = list(selected_nodes)
    identity_expansion_limited = False
    traversed_relation_ids: set[str] = set()
    for depth in range(spec["traversal"]["depth"]):
        origins = {}
        for node_id in sorted(frontier):
            entity = all_nodes_by_id[node_id].get('entity_id')
            if entity in carrier_groups:
                origins.setdefault(entity, node_id)
        aliases = heapq.nsmallest(spec['limits']['nodes'] - len(selected_nodes) + 1,
                                  (id for entity in origins for id in carrier_groups[entity] if id not in selected_nodes))
        for node_id in aliases:
            if len(selected_nodes) >= spec['limits']['nodes']:
                identity_expansion_limited = True
                break
            node = all_nodes_by_id[node_id]
            selected_nodes[node_id] = node
            inclusion[node_id] = {'kind': 'identity-carrier', 'via_node_id': origins[node['entity_id']],
                                  'entity_id': node['entity_id'], 'depth': depth}
            frontier.append(node_id)
        next_frontier: list[str] = []
        for relation in relation_candidates.incident(frontier):
            relation_id = str(relation["id"])
            touched = False
            for node_id in frontier:
                for neighbor_id in k._relation_neighbors(relation, node_id, spec["traversal"]["direction"]):
                    touched = True
                    if neighbor_id not in selected_nodes and neighbor_id in all_nodes_by_id and len(selected_nodes) < spec["limits"]["nodes"]:
                        selected_nodes[neighbor_id] = all_nodes_by_id[neighbor_id]
                        inclusion[neighbor_id] = {'kind': 'traversal', 'via_node_id': node_id,
                                                  'via_relation_id': relation_id, 'depth': depth + 1}
                        next_frontier.append(neighbor_id)
            if touched and len(traversed_relation_ids) < spec["limits"]["relations"]:
                traversed_relation_ids.add(relation_id)
        frontier = list(dict.fromkeys(next_frontier))
        if not frontier:
            break

    endpoint_policy = spec["composition"]["endpoint_policy"]
    selection_basis = set(selected_nodes)
    selected_relations: list[dict[str, Any]] = []
    eligible_relation_count = 0
    eligible_candidates = relation_candidates if spec['composition']['endpoint_policy'] == 'independent' else relation_candidates.incident(selection_basis)
    for relation in eligible_candidates:
        left = str(relation["from_id"])
        right = str(relation["to_id"])
        left_selected = left in selection_basis
        right_selected = right in selection_basis
        allowed = (
            (endpoint_policy == "both" and left_selected and right_selected)
            or (endpoint_policy == "either" and (left_selected or right_selected))
            or endpoint_policy == "independent"
            or str(relation["id"]) in traversed_relation_ids
        )
        if not allowed:
            continue
        eligible_relation_count += 1
        if len(selected_relations) >= spec["limits"]["relations"]:
            continue
        missing = list(dict.fromkeys(node_id for node_id in (left, right) if node_id not in selected_nodes))
        if len(selected_nodes) + len(missing) > spec["limits"]["nodes"]:
            continue
        for node_id in missing:
            if node_id in all_nodes_by_id:
                selected_nodes[node_id] = all_nodes_by_id[node_id]
                inclusion[node_id] = {'kind': 'endpoint', 'via_relation_id': relation['id']}
        if left in selected_nodes and right in selected_nodes:
            selected_relations.append(relation)

    final_nodes = k._sort_items(selected_nodes.values(), spec["composition"]["sort_nodes"])
    final_relations = k._sort_items(selected_relations, spec["composition"]["sort_relations"])
    focus = k._focus_payload(spec, final_nodes, focus_node)
    groups = k._groups(final_nodes, final_relations, spec["composition"]["group_by"], spec["limits"]["groups"])
    refs = sorted({ref for item in [*final_nodes, *final_relations] for ref in k._strings(item.get("source_refs"))})
    missing_node_summaries = sum(item["display"]["summary_state"] == "missing" for item in final_nodes)
    missing_relation_explanations = sum(item["display"]["explanation_state"] == "missing" for item in final_relations)
    nodes_without_source_summary = sum(
        item["display"]["provenance"].get("source_summary_available") is False
        for item in final_nodes
    )
    relations_without_source_explanation = sum(
        item["display"]["provenance"].get("source_explanation_available") is False
        for item in final_relations
    )
    if focus_node is not None and not focus_matched:
        matched_node_count += 1
    truncated_nodes = max(0, matched_node_count - spec['limits']['nodes'])
    truncated_relations = max(0, eligible_relation_count - len(final_relations))
    fingerprint_material = {
        "execution_version": "tos-lens-execution-v6",
        "source_revision": graph.get("source_revision"),
        "lens": {k: v for k, v in public_spec.items() if k != 'pagination'},
        "nodes": [[item["id"], item["content_revision"]] for item in final_nodes],
        "relations": [[item["id"], item["content_revision"]] for item in final_relations],
        "groups": groups,
    }
    result = paginate_lens({
        "schema": "tos_lens_result_v1",
        "source_revision": str(graph.get("source_revision") or ""),
        "lens": public_spec,
        "fingerprint": k._stable_digest(fingerprint_material),
        "presentation": spec["presentation"],
        "focus": focus,
        **({'inclusion': {'nodes': inclusion,
                          'relations': {r['id']: {'kind': 'traversal' if r['id'] in traversed_relation_ids else 'endpoint-policy',
                                                  'endpoint_policy': endpoint_policy} for r in final_relations},
                          'authority': 'query-execution-not-semantic-proof'}} if spec['explain'] else {}),
        "nodes": [k._lens_carrier(item, spec['detail'], language=spec['language']) for item in final_nodes],
        "relations": [k._lens_carrier(item, spec['detail'], language=spec['language']) for item in final_relations],
        "groups": groups,
        "facets": {
            "node_kinds": dict(sorted(Counter(str(item["kind_id"]) for item in final_nodes).items())),
            "predicates": dict(sorted(Counter(str(item["predicate_id"]) for item in final_relations).items())),
            "sources": dict(sorted(Counter(str(item["source_graph"]) for item in final_nodes).items())),
        },
        "counts": {
            "available_nodes": store.count('knowledge_nodes', source_where, source_params),
            "available_relations": store.count('knowledge_relations', source_where, source_params),
            "matched_nodes": matched_node_count,
            "matched_relations": len(relation_candidates),
            "eligible_relations": eligible_relation_count,
            "nodes": len(final_nodes),
            "relations": len(final_relations),
            "groups": len(groups),
            "truncated_nodes": truncated_nodes,
            "truncated_relations": truncated_relations,
            "identity_expansion_limited": identity_expansion_limited,
            "missing_node_summaries": missing_node_summaries,
            "missing_relation_explanations": missing_relation_explanations,
            "nodes_without_source_summary": nodes_without_source_summary,
            "relations_without_source_explanation": relations_without_source_explanation,
        },
        "source_refs": refs,
        "warnings": [
            *(['identity carrier expansion reached the node budget; use resumable exploration or narrower sources'] if identity_expansion_limited else []),
            *( [f"{missing_node_summaries} nodes expose an explicit missing-summary state"] if missing_node_summaries else [] ),
            *( [f"{missing_relation_explanations} relations expose an explicit missing-explanation state"] if missing_relation_explanations else [] ),
            *( [f"{nodes_without_source_summary} nodes use transparent metadata synthesis because no source summary is projected"] if nodes_without_source_summary else [] ),
            *( [f"{relations_without_source_explanation} relations use transparent metadata synthesis because no source explanation is projected"] if relations_without_source_explanation else [] ),
            *( [f"node selector exceeded its bounded result by {truncated_nodes} nodes"] if truncated_nodes else [] ),
            *( [f"relation selector exceeded its bounded result by {truncated_relations} relations"] if truncated_relations else [] ),
        ],
        "authority_boundary": dict(graph.get("authority_boundary") or {}),
        "agent_summary": {
            "lens_id": spec["lens_id"],
            "focus_node_id": focus["node_id"] if focus is not None else None,
            "node_count": len(final_nodes),
            "relation_count": len(final_relations),
            "group_count": len(groups),
            "source_ref_count": len(refs),
            "is_source": False,
            "writes_to_tree": False,
        },
    })
    result['scene'] = k.knowledge_scene(result['nodes'], result['relations'],
                                      result['focus']['node_id'] if result['focus'] else None)
    return result



def focus_spec(
    node_id: str,
    *,
    sources: list[str] | None = None,
    depth: int = 1,
    direction: str = "either",
    predicate_ids: list[str] | None = None,
    node_limit: int = 200,
    relation_limit: int = 400,
    profile: str = "overview",
) -> dict[str, Any]:
    """Build one radial LensSpec around an exact or uniquely namespaced node identity."""
    identifier = k._string(node_id)
    if identifier is None:
        raise ValueError("knowledge focus node id is required")
    source_set = k._normalized_source_filter(sources)
    selected_sources = [source for source in k.KNOWLEDGE_SOURCES if source in source_set]
    return {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "focus-neighborhood",
            "title": {"default": f"Focus: {identifier}"},
            "description": {
                "default": f"Bounded knowledge neighborhood centered on {identifier}."
            },
            "sources": selected_sources,
            "seed": {"focus_node_id": identifier},
            "node_query": {"enabled": False},
            "relation_query": {"enabled": True},
            "traversal": {
                "depth": depth,
                "direction": direction,
                "predicate_ids": predicate_ids or [],
                "profile": profile,
            },
            "composition": {
                "endpoint_policy": "both",
                "group_by": [],
                "sort_nodes": [{"field": "id", "direction": "asc"}],
                "sort_relations": [{"field": "id", "direction": "asc"}],
            },
            "presentation": {
                "layout": "radial",
                "color_by": "kind_id",
                "lane_by": "epistemic.authority_layer",
                "size_by": None,
                "inspector_fields": ["display", "epistemic", "source_refs", "attributes"],
            },
            "limits": {"nodes": node_limit, "relations": relation_limit, "groups": 100},
        }


def _contains(value, needle):
    if isinstance(value, str):
        return needle in value.lower()
    if isinstance(value, dict):
        return any(_contains(item, needle) for item in value.values())
    if isinstance(value, list):
        return any(_contains(item, needle) for item in value)
    return False


def filter_sql(table, group):
    """Use identity/type indexes for common selectors; preserve the complete
    metadata/property grammar through the pure legacy predicate otherwise."""
    columns = {'id','native_id','source_graph','kind_id','type_id','entity_id'} if table == 'knowledge_nodes' else {'id','native_id','source_graph','from_id','to_id','predicate_id','relation_type_id'}
    clauses, values = [], []
    for rule in group['filters']:
        field, op, value = rule.get('field'), rule['op'], rule['value']
        if field in columns and '_property_binding' not in rule and op == 'eq' and isinstance(value,str):
            clauses.append(field + '=?')
            values.append(value)
        elif field in columns and '_property_binding' not in rule and op == 'in' and isinstance(value,list) and all(isinstance(v,str) for v in value):
            clause, args = QueryStore.membership(field,value)
            clauses.append(clause)
            values.extend(args)
        else:
            clauses.append('tos_match(payload, ?)')
            values.append(json.dumps({'enabled':True,'match':'all','filters':[rule]}))
    return (' AND ' if group['match'] == 'all' else ' OR ').join('(' + clause + ')' for clause in clauses), values

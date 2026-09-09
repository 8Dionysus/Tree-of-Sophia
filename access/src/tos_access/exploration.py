"""Resumable read-only BFS with disposable, bounded process-local checkpoints.

The graph provider must return immutable snapshots (as ToSAccessCore does).
Index construction is once per snapshot, not once per page. Checkpoints are
query execution state, never corpus records or a historical snapshot service.
"""
from __future__ import annotations

import json
import re
import secrets
import threading
import time
from collections import Counter, OrderedDict, defaultdict

from .knowledge import (
    KNOWLEDGE_SOURCES, OVERVIEW_EXCLUDED_PREDICATES, OVERVIEW_EXCLUDED_RELATION_TYPES, _lens_carrier,
    _resolve_focus_node, _stable_digest, knowledge_scene, _identity_carrier_groups,
)
from .lens_pagination import KnowledgeRevisionConflict
from .exploration_origin import REQUEST_V2, RESULT_V2, ExplorationReadModelInvalid, bind_origin, normalize_origin

EXECUTION_VERSION = "tos-exploration-execution-v6"


class ExplorationExpired(ValueError):
    """Checkpoint expired, was evicted, or belongs to another process."""


def exploration_capabilities():
    return {
        "schema": "tos_exploration_capabilities_v1", "available": True,
        "execution_version": EXECUTION_VERSION,
        "http": {"method": "POST", "path": "/api/knowledge/explore"},
        "storage": "bounded-process-memory", "ttl_seconds": 900,
        "max_checkpoints": 128, "max_checkpoint_bytes": 32 * 1024 * 1024,
        "limits": {"depth": 10, "page_nodes": 100, "page_relations": 100,
                   "work_per_page": 512, "session_nodes": 10000, "session_relations": 20000},
        "restart_survival": False, "writes_to_tree": False,
        "continuation": "opaque-cursor-only; fixed query and page sizes",
        "ordering": "zero-distance declared identity carriers before relation-id ordered edges; all profile uses carrier BFS",
        "identity_expansion": "overview only; source-filtered declared tos.* IDs; page and session node budgets apply",
        "runtime": "local", "other_runtimes": "discover-on-target",
        "request_versions": ['tos_exploration_request_v1', REQUEST_V2],
        "result_versions": ['tos_exploration_result_v1', RESULT_V2],
        "v2_origin_kinds": ['node', 'relation'],
        "v2_origin_context": {"max_nodes": 2, "max_relations": 1,
                              "page_budgets": 'incremental; mandatory origin closure is additional'},
    }


def normalize_exploration(request):
    if not isinstance(request, dict):
        raise ValueError("exploration request must be an object")
    if request.get('schema_version') == REQUEST_V2:
        origin = normalize_origin(request)
        options = {k: v for k, v in request.items() if k not in ('schema_version', 'source_revision', 'origin')}
        if 'focus_node_id' in options:
            raise ValueError('exploration v2 uses origin, not legacy focus_node_id')
        query = normalize_exploration({'focus_node_id': origin['id'], **options})
        del query['focus_node_id']
        return {'schema_version': REQUEST_V2, 'source_revision': request['source_revision'], 'origin': origin, **query}
    allowed = {"focus_node_id", "sources", "direction", "predicate_ids", "profile",
               "max_depth", "page_nodes", "page_relations"}
    if set(request) - allowed:
        raise ValueError("unknown exploration fields; continue with cursor only")
    focus = request.get("focus_node_id")
    if not isinstance(focus, str) or not focus.strip() or len(focus) > 1024:
        raise ValueError("focus_node_id must be a nonempty string of at most 1024 characters")
    query = {"focus_node_id": focus}
    for name, default, maximum in (("max_depth", 3, 10), ("page_nodes", 40, 100),
                                    ("page_relations", 80, 100)):
        value = request.get(name, default)
        if type(value) is not int or not (0 if name == "max_depth" else 1) <= value <= maximum:
            raise ValueError(f"invalid exploration {name}")
        query[name] = value
    for name, default, values in (("direction", "either", {"either", "outgoing", "incoming"}),
                                   ("profile", "overview", {"overview", "all"})):
        value = request.get(name, default)
        if not isinstance(value, str) or value not in values:
            raise ValueError(f"invalid exploration {name}")
        query[name] = value
    for name, default, maximum in (("sources", list(KNOWLEDGE_SOURCES), len(KNOWLEDGE_SOURCES)),
                                    ("predicate_ids", [], 100)):
        value = request.get(name, default)
        if (not isinstance(value, list) or len(value) > maximum
                or any(not isinstance(v, str) or not v or len(v) > 1024 for v in value)):
            raise ValueError(f"invalid exploration {name}")
        if name == "sources" and (not value or set(value) - set(KNOWLEDGE_SOURCES)):
            raise ValueError("exploration sources must be nonempty registered sources")
        query[name] = sorted(set(value))
    return query


class ExplorationService:
    """One service per core/server, serialized for deterministic concurrent replay.

    JSON bytes bound stored checkpoint size independently of Python container
    overhead. The current immutable graph and its adjacency index are separate
    from this cache. No source payloads or graph copies enter checkpoints.
    """

    def __init__(self, graph_provider, *, clock=time.monotonic, ttl=900,
                 max_checkpoints=128, max_bytes=32 * 1024 * 1024,
                 work_limit=512, node_limit=10000, relation_limit=20000):
        if min(ttl, max_checkpoints, max_bytes, work_limit, node_limit, relation_limit) <= 0:
            raise ValueError("exploration service limits must be positive")
        self.graph_provider, self.clock, self.ttl = graph_provider, clock, ttl
        self.max_checkpoints, self.max_bytes = max_checkpoints, max_bytes
        self.work_limit, self.node_limit, self.relation_limit = work_limit, node_limit, relation_limit
        self.lock = threading.RLock()
        self.records = OrderedDict()
        self.stored_bytes = 0
        self.graph = None

    def _index(self, graph):
        if graph is self.graph:
            return
        # Prepare atomically: an invalid replacement must not poison the last
        # immutable snapshot's index. This checks indexing structure only.
        try:
            nodes = {n["id"]: n for n in graph["nodes"]}
            duplicate_nodes = {id for id, count in Counter(n['id'] for n in graph['nodes']).items() if count > 1}
            carrier_groups = _identity_carrier_groups(graph['nodes'])
            relations = {r["id"]: r for r in graph["relations"]}
            duplicate_relations = {id for id, count in Counter(r['id'] for r in graph['relations']).items() if count > 1}
            adjacency = defaultdict(list)
            for relation in sorted(graph["relations"], key=lambda r: r["id"]):
                for endpoint in dict.fromkeys((relation["from_id"], relation["to_id"])):
                    adjacency[endpoint].append(relation["id"])
            revision = _stable_digest({
                "execution_version": EXECUTION_VERSION,
                "source_revision": graph.get("source_revision"),
                "nodes": sorted([n["id"], n["content_revision"]] for n in graph["nodes"]),
                "relations": sorted([r["id"], r["content_revision"]] for r in graph["relations"]),
            })
        except (KeyError, TypeError, AttributeError) as exc:
            raise ExplorationReadModelInvalid('exploration snapshot cannot be indexed intact') from exc
        self.nodes, self.relations, self.carrier_groups = nodes, relations, carrier_groups
        self.duplicate_nodes, self.duplicate_relations = duplicate_nodes, duplicate_relations
        self.adjacency, self.revision = adjacency, revision
        self.graph = graph

    def _remove(self, token):
        _, raw = self.records.pop(token)
        self.stored_bytes -= len(raw)

    def _put(self, token, expires, record):
        raw = json.dumps(record, ensure_ascii=False, separators=(",", ":")).encode()
        if len(raw) > self.max_bytes:
            raise ExplorationExpired("checkpoint exceeds cache capacity; narrow exploration")
        if token in self.records:
            self._remove(token)
        while self.records and (len(self.records) >= self.max_checkpoints
                                or self.stored_bytes + len(raw) > self.max_bytes):
            self._remove(next(iter(self.records)))
        self.records[token] = (expires, raw)
        self.stored_bytes += len(raw)

    def explore(self, request):
        with self.lock:
            if not isinstance(request, dict):
                raise ValueError("exploration request must be an object")
            cursor = request.get("cursor")
            continuing = "cursor" in request
            if continuing and (set(request) != {"cursor"} or not isinstance(cursor, str)
                               or not re.fullmatch("[0-9a-f]{64}", cursor)):
                raise ValueError("continue exploration with one opaque cursor only")
            query = None if continuing else normalize_exploration(request)
            now = self.clock()
            for token, (expires, _) in list(self.records.items()):
                if expires <= now:
                    self._remove(token)
            record = None
            if continuing:
                if cursor not in self.records:
                    raise ExplorationExpired("exploration expired or was evicted; restart from focus")
                expires, raw = self.records[cursor]
                record = json.loads(raw)
            self._index(self.graph_provider())
            if continuing:
                if record["revision"] != self.revision:
                    raise KnowledgeRevisionConflict("exploration snapshot changed; restart from focus")
                if record.get("result") is not None:
                    return record["result"]
                state = record["state"]
            else:
                if query.get('schema_version') == REQUEST_V2:
                    origin, roots, seed_relations = bind_origin(query, self.graph.get('source_revision'),
                        lambda id: None if id in self.duplicate_nodes else self.nodes.get(id),
                        lambda id: None if id in self.duplicate_relations else self.relations.get(id))
                    if len(roots) > self.node_limit or len(seed_relations) > self.relation_limit:
                        raise ValueError('exploration origin closure exceeds session limits')
                else:
                    focus = _resolve_focus_node([n for n in self.nodes.values()
                                                 if n["source_graph"] in query["sources"]], query["focus_node_id"])
                    query["focus_node_id"] = focus["id"]
                    roots, seed_relations, origin = [focus['id']], [], None
                expires = now + self.ttl
                state = {"query": query, "queue": [[id, 0] for id in roots], "head": 0, "offset": 0,
                         "identity_offset": 0, "identity_added": 0,
                         "expanded_entities": [],
                         "seen_nodes": list(roots), "seen_relations": list(seed_relations), "page_number": 0}
                if origin is not None:
                    state.update(origin=origin, roots=roots, seed_relations=seed_relations)
            result, state = self._advance(state)
            next_cursor = secrets.token_hex(32) if result["status"] == "paused" else None
            result["page"]["next_cursor"] = next_cursor
            next_record = {"revision": self.revision, "state": state} if next_cursor else None
            # Check capacity before replacing the input checkpoint with a replay
            # response. A rejected oversized successor must not leave a cached
            # successful page pointing to a cursor that was never admitted.
            if next_record is not None and len(json.dumps(next_record, ensure_ascii=False, separators=(",", ":")).encode()) > self.max_bytes:
                raise ExplorationExpired("checkpoint exceeds cache capacity; narrow exploration")
            # Cache replay separately from continuation state; neither contains
            # graph payloads beyond the bounded returned page.
            if continuing:
                self._put(cursor, expires, {"revision": self.revision, "result": result})
            if next_cursor:
                self._put(next_cursor, expires, next_record)
            return result

    def _advance(self, state):
        query = state["query"]
        origin = state.get('origin')
        focus = query.get('focus_node_id') if origin is None else origin['id'] if origin['kind'] == 'node' else None
        roots = state['roots'] if origin is not None else [focus]
        seed_relations = state.get('seed_relations', [])
        primary = [focus] if origin is None and state["page_number"] == 0 else []
        emitted = []
        seen_nodes, seen_relations = set(state["seen_nodes"]), set(state["seen_relations"])
        node_reasons = {id: {'kind': 'origin' if origin and origin['kind'] == 'node'
                            else 'origin-endpoint' if origin else 'focus'} for id in roots}
        promoted = set()
        expanded_entities = set(state['expanded_entities'])
        edge_reasons = {id: {'kind': 'origin'} for id in seed_relations}
        work = 0
        limit_reason = None
        while state["head"] < len(state["queue"]) and work < self.work_limit:
            current, depth = state["queue"][state["head"]]
            adjacent = self.adjacency[current]
            # Identity expansion is a source-filtered zero-distance operation,
            # not an invented relation. Its pending position is checkpointed.
            entity = self.nodes[current].get('entity_id')
            aliases = self.carrier_groups.get(entity, []) if query['profile'] == 'overview' and entity not in expanded_entities else []
            if depth < query['max_depth'] and state['identity_offset'] < len(aliases):
                alias = aliases[state['identity_offset']]
                work += 1
                if alias in seen_nodes or self.nodes[alias]['source_graph'] not in query['sources']:
                    if alias in seen_nodes:
                        position = next(i for i, (id, _) in enumerate(state['queue']) if id == alias)
                        if state['queue'][position][1] > depth:
                            if alias not in promoted and len(primary) + len(promoted) >= query['page_nodes']:
                                break
                            state['queue'].pop(position)
                            state['queue'].insert(state['head'] + 1 + state['identity_added'], [alias, depth])
                            state['identity_added'] += 1
                            promoted.add(alias)
                            node_reasons[alias] = {'kind': 'identity-carrier', 'via_node_id': current,
                                                   'entity_id': entity, 'depth': depth}
                    state['identity_offset'] += 1
                    continue
                if len(seen_nodes) >= self.node_limit:
                    limit_reason = 'session_nodes'
                    break
                if len(primary) + len(promoted) >= query['page_nodes']:
                    break
                state['identity_offset'] += 1
                seen_nodes.add(alias)
                state['seen_nodes'].append(alias)
                state['queue'].insert(state['head'] + 1 + state['identity_added'], [alias, depth])
                state['identity_added'] += 1
                primary.append(alias)
                node_reasons[alias] = {'kind': 'identity-carrier', 'via_node_id': current,
                                       'entity_id': entity, 'depth': depth}
                continue
            if aliases and depth < query['max_depth'] and entity not in expanded_entities:
                expanded_entities.add(entity)
                state['expanded_entities'].append(entity)
            if depth >= query["max_depth"] or state["offset"] == len(adjacent):
                state["head"] += 1
                state["offset"] = 0
                state['identity_offset'] = state['identity_added'] = 0
                work += 1
                continue
            edge = self.relations[adjacent[state["offset"]]]
            work += 1
            target = edge["to_id"] if edge["from_id"] == current else edge["from_id"]
            eligible = (edge["id"] not in seen_relations
                        and edge["source_graph"] in query["sources"]
                        and target in self.nodes and self.nodes[target]["source_graph"] in query["sources"]
                        and (query["direction"] != "outgoing" or edge["from_id"] == current)
                        and (query["direction"] != "incoming" or edge["to_id"] == current)
                        and (not query["predicate_ids"] or edge["predicate_id"] in query["predicate_ids"])
                        and (query["profile"] != "overview" or (edge["predicate_id"] not in OVERVIEW_EXCLUDED_PREDICATES
                             and edge.get("relation_type_id") not in OVERVIEW_EXCLUDED_RELATION_TYPES)))
            if not eligible:
                state["offset"] += 1
                continue
            new_node = target not in seen_nodes
            if len(seen_relations) >= self.relation_limit or (new_node and len(seen_nodes) >= self.node_limit):
                limit_reason = "session_relations" if len(seen_relations) >= self.relation_limit else "session_nodes"
                break
            if len(emitted) >= query["page_relations"] or (new_node and len(primary) + len(promoted) >= query["page_nodes"]):
                break  # Keep this edge pending, including when the node page is full.
            state["offset"] += 1
            seen_relations.add(edge["id"])
            state["seen_relations"].append(edge["id"])
            emitted.append(edge["id"])
            edge_reasons[edge["id"]] = {"kind": "traversal", "via_node_id": current, "depth": depth + 1}
            if new_node:
                seen_nodes.add(target)
                state["seen_nodes"].append(target)
                state["queue"].append([target, depth + 1])
                primary.append(target)
                node_reasons[target] = {"kind": "traversal", "via_node_id": current,
                                        "via_relation_id": edge["id"], "depth": depth + 1}
        state["page_number"] += 1
        selected = set(primary) | promoted | set(roots)
        for id in emitted:
            selected.update((self.relations[id]["from_id"], self.relations[id]["to_id"]))
        for id in selected - node_reasons.keys():
            node_reasons[id] = {"kind": "context-endpoint"}
        status = ("limit_reached" if limit_reason else
                  "complete" if state["head"] == len(state["queue"]) else "paused")
        delivered_nodes = [_lens_carrier(self.nodes[id], "compact", language="auto") for id in sorted(selected)]
        delivered_relations = [_lens_carrier(self.relations[id], "compact", language="auto") for id in [*seed_relations, *emitted]]
        result = {
            "schema": RESULT_V2 if origin is not None else "tos_exploration_result_v1", "execution_version": EXECUTION_VERSION,
            "snapshot_revision": self.revision, "source_revision": self.graph.get("source_revision", ""),
            "query": query, "focus": {"node_id": focus}, "status": status, "limit_reason": limit_reason,
            "nodes": delivered_nodes, "relations": delivered_relations,
            "scene": knowledge_scene(delivered_nodes, delivered_relations, focus,
                                      origin['id'] if origin and origin['kind'] == 'relation' else None),
            "page": {"number": state["page_number"], "primary_node_ids": primary,
                     "context_node_ids": sorted(selected - set(primary)), "next_cursor": None,
                     "returned_nodes": len(selected), "returned_relations": len(delivered_relations),
                     "work_units": work, "scope": "resumable-neighborhood"},
            "counts": {"discovered_nodes": len(seen_nodes), "emitted_relations": len(seen_relations) - len(seed_relations),
                       "scope": "cumulative-discovered-not-global-total"},
            "inclusion": {"nodes": node_reasons, "relations": edge_reasons,
                          "authority": "query-execution-not-semantic-proof"},
            "authority_boundary": self.graph.get("authority_boundary", {}),
            "writes_to_tree": False,
        }
        if origin is not None:
            del result['focus']
            result['origin'] = origin
            result['page'].update(primary_relation_ids=emitted, context_relation_ids=list(seed_relations))
        return result, state

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
from collections import OrderedDict, defaultdict

from .knowledge import (
    KNOWLEDGE_SOURCES, OVERVIEW_EXCLUDED_PREDICATES, OVERVIEW_EXCLUDED_RELATION_TYPES, _lens_carrier,
    _resolve_focus_node, _stable_digest, knowledge_scene,
)
from .lens_pagination import KnowledgeRevisionConflict

EXECUTION_VERSION = "tos-exploration-execution-v3"


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
        "ordering": "breadth-first, relation-id ascending per expanded node",
        "runtime": "local", "other_runtimes": "discover-on-target",
    }


def normalize_exploration(request):
    if not isinstance(request, dict):
        raise ValueError("exploration request must be an object")
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
        self.nodes = {n["id"]: n for n in graph["nodes"]}
        self.relations = {r["id"]: r for r in graph["relations"]}
        self.adjacency = defaultdict(list)
        for relation in sorted(graph["relations"], key=lambda r: r["id"]):
            for endpoint in dict.fromkeys((relation["from_id"], relation["to_id"])):
                self.adjacency[endpoint].append(relation["id"])
        self.revision = _stable_digest({
            "execution_version": EXECUTION_VERSION,
            "source_revision": graph.get("source_revision"),
            "nodes": sorted([n["id"], n["content_revision"]] for n in graph["nodes"]),
            "relations": sorted([r["id"], r["content_revision"]] for r in graph["relations"]),
        })
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
                focus = _resolve_focus_node([n for n in self.nodes.values()
                                             if n["source_graph"] in query["sources"]], query["focus_node_id"])
                query["focus_node_id"] = focus["id"]
                expires = now + self.ttl
                state = {"query": query, "queue": [[focus["id"], 0]], "head": 0, "offset": 0,
                         "seen_nodes": [focus["id"]], "seen_relations": [], "page_number": 0}
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
        focus = query["focus_node_id"]
        primary = [focus] if state["page_number"] == 0 else []
        emitted = []
        seen_nodes, seen_relations = set(state["seen_nodes"]), set(state["seen_relations"])
        node_reasons = {focus: {"kind": "focus"}}
        edge_reasons = {}
        work = 0
        limit_reason = None
        while state["head"] < len(state["queue"]) and work < self.work_limit:
            current, depth = state["queue"][state["head"]]
            adjacent = self.adjacency[current]
            if depth >= query["max_depth"] or state["offset"] == len(adjacent):
                state["head"] += 1
                state["offset"] = 0
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
            if len(emitted) >= query["page_relations"] or (new_node and len(primary) >= query["page_nodes"]):
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
        selected = set(primary) | {focus}
        for id in emitted:
            selected.update((self.relations[id]["from_id"], self.relations[id]["to_id"]))
        for id in selected - node_reasons.keys():
            node_reasons[id] = {"kind": "context-endpoint"}
        status = ("limit_reached" if limit_reason else
                  "complete" if state["head"] == len(state["queue"]) else "paused")
        return {
            "schema": "tos_exploration_result_v1", "execution_version": EXECUTION_VERSION,
            "snapshot_revision": self.revision, "source_revision": self.graph.get("source_revision", ""),
            "query": query, "focus": {"node_id": focus}, "status": status, "limit_reason": limit_reason,
            "nodes": [_lens_carrier(self.nodes[id], "compact") for id in sorted(selected)],
            "relations": [_lens_carrier(self.relations[id], "compact") for id in emitted],
            "scene": knowledge_scene([self.nodes[id] for id in selected],
                                     [self.relations[id] for id in emitted], focus),
            "page": {"number": state["page_number"], "primary_node_ids": primary,
                     "context_node_ids": sorted(selected - set(primary)), "next_cursor": None,
                     "returned_nodes": len(selected), "returned_relations": len(emitted),
                     "work_units": work, "scope": "resumable-neighborhood"},
            "counts": {"discovered_nodes": len(seen_nodes), "emitted_relations": len(seen_relations),
                       "scope": "cumulative-discovered-not-global-total"},
            "inclusion": {"nodes": node_reasons, "relations": edge_reasons,
                          "authority": "query-execution-not-semantic-proof"},
            "authority_boundary": self.graph.get("authority_boundary", {}),
            "writes_to_tree": False,
        }, state

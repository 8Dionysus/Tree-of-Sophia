"""Native-v6 exploration over pinned published rows, never a whole graph.

The snapshot hash is SHA256 of compact emitted JSON containing this framing
schema, native execution version and the complete owner-selected publication
binding (including epoch). It is not the native in-memory full-graph hash.
Only that opaque hash and random cursor tokens differ in semantic A/B parity.
Memory checkpoints retain the native restart=false boundary. An explicitly
selected private checkpoint_path enables restart survival; read-model/source
files are never used for disposable query writes.
"""
from __future__ import annotations

import copy
import json
import re
import secrets
import time

from .exploration import EXECUTION_VERSION, ExplorationExpired, ExplorationService, exploration_capabilities, normalize_exploration
from .exploration_origin import REQUEST_V2, RESULT_V2, bind_origin
from .knowledge import (
    OVERVIEW_EXCLUDED_PREDICATES, OVERVIEW_EXCLUDED_RELATION_TYPES,
    _CARRIER_SOURCE_PRIORITY, _lens_carrier, _resolve_focus_node, knowledge_scene,
)
from .lens_pagination import KnowledgeRevisionConflict
from .published_read_metadata import _compact, emitted_row_digest
from .published_read_model import PublishedKnowledgeReadModel, PublishedReadModelError, PublishedSnapshotConflict
from .published_checkpoints import PublishedCheckpointStore, PublishedCheckpointError

SNAPSHOT_SCHEMA = "tos_published_exploration_snapshot_v1"


class _Rows:
    """One transaction-local payload cache and bounded keyset windows."""
    def __init__(self, read, block_size):
        self.read, self.block_size = read, block_size
        self.nodes, self.relations = {}, {}
        self.edge_window = (None, [])
        self.alias_window = (None, [])
        found = read.query("SELECT name FROM sqlite_master WHERE type='index' AND name='knowledge_nodes_identity_seek'")
        if not found:
            raise PublishedReadModelError("prepared identity seek migration is unavailable")

    def load(self, kind, ids):
        cache = self.nodes if kind == "node" else self.relations
        missing = sorted(set(ids) - cache.keys())
        if missing:
            items = self.read.items(kind, "id IN (SELECT value FROM json_each(?))", (_compact(missing),), len(missing))
            cache.update((identifier, None) for identifier in missing)
            cache.update((item["id"], item) for item in items)
        return cache

    def node(self, identifier):
        return self.load("node", [identifier])[identifier]

    def relation(self, identifier):
        return self.load("relation", [identifier])[identifier]

    def focus(self, query):
        identifier, sources = query["focus_node_id"], _compact(query["sources"])
        exact = self.read.query("SELECT id FROM knowledge_nodes WHERE id=? AND source_graph IN (SELECT value FROM json_each(?)) LIMIT 1",
                                (identifier, sources))
        if exact:
            return self.node(exact[0]["id"])
        # Find the native source-priority representative without loading every
        # wide alias packet. This narrow range still has the reader VM budget.
        priority = "CASE source_graph " + " ".join(
            "WHEN '" + source + "' THEN " + str(rank) for source, rank in _CARRIER_SOURCE_PRIORITY.items()) + " ELSE 99 END"
        entity = self.read.query(
            "SELECT id FROM knowledge_nodes INDEXED BY knowledge_nodes_identity_seek "
            "WHERE entity_id=? AND source_graph IN (SELECT value FROM json_each(?)) "
            f"ORDER BY {priority},id LIMIT 1", (identifier, sources))
        if entity:
            return self.node(entity[0]["id"])
        native = self.read.query(
            "SELECT id FROM knowledge_nodes WHERE native_id=? AND source_graph IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT 2",
            (identifier, sources))
        # Two matches suffice to refuse ambiguity; never enumerate an unbounded
        # alias list merely to put its complete contents into an error message.
        return _resolve_focus_node([self.node(row["id"]) for row in native], identifier)

    def alias(self, entity, after):
        key, window = self.alias_window
        if key != entity or not window or window[-1] <= after:
            window = [row["id"] for row in self.read.query(
                "SELECT id FROM knowledge_nodes INDEXED BY knowledge_nodes_identity_seek "
                "WHERE entity_id=? AND id>? ORDER BY id LIMIT ?", (entity, after, self.block_size))]
            self.alias_window = entity, window
            self.load("node", window)
        return next((identifier for identifier in window if identifier > after), None)

    def adjacent(self, current, after):
        key, window = self.edge_window
        if key != current or not window or window[-1] <= after:
            # Limit both covering seeks before their bounded merge. No OFFSET,
            # degree COUNT, unbounded UNION, or graph-wide relationship scan.
            windows = [self.read.query(
                f"SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_{side}_seek "
                f"WHERE {side}_id=? AND id>? ORDER BY id LIMIT ?", (current, after, self.block_size))
                for side in ("from", "to")]
            window = sorted({row["id"] for packet in windows for row in packet})[:self.block_size]
            self.edge_window = current, window
            self.load("relation", window)
            endpoints = {item[field] for identifier in window
                         for item in [self.relations[identifier]] if item is not None
                         for field in ("from_id", "to_id")}
            self.load("node", endpoints)
        identifier = next((identifier for identifier in window if identifier > after), None)
        return self.relations[identifier] if identifier is not None else None


class PublishedExplorationService(ExplorationService):
    """Reuse the native bounded replay-cache mechanics, not its graph/index.

    All public queries override the native graph path. Each successful page is
    admitted to the replay cache only after the published reader's final
    snapshot check. A failed read leaves the input cursor/state untouched.
    """
    def __init__(self, reader: PublishedKnowledgeReadModel, *, clock=None, checkpoint_path=None,
                 ttl=900, max_checkpoints=128, max_bytes=32 * 1024 * 1024,
                 work_limit=512, node_limit=10000, relation_limit=20000, block_size=16):
        for value, maximum in ((work_limit, 512), (node_limit, 10000), (relation_limit, 20000), (block_size, 64)):
            if type(value) is not int or not 1 <= value <= maximum:
                raise ValueError("prepared exploration limits exceed the native contract")
        if (type(ttl) is not int or not 1 <= ttl <= 86400
                or type(max_checkpoints) is not int or not 1 <= max_checkpoints <= 128
                or type(max_bytes) is not int or not 1 <= max_bytes <= 32 * 1024 * 1024):
            raise ValueError("prepared checkpoint limits exceed the bounded contract")
        clock = clock or (time.time if checkpoint_path is not None else time.monotonic)
        super().__init__(None, clock=clock, ttl=ttl, max_checkpoints=max_checkpoints, max_bytes=max_bytes,
                         work_limit=work_limit, node_limit=node_limit, relation_limit=relation_limit)
        self.reader, self.block_size = reader, block_size
        self.revision = emitted_row_digest(_compact({
            "schema": SNAPSHOT_SCHEMA, "execution_version": EXECUTION_VERSION,
            "publication_binding": reader.snapshot_binding,
        }))["sha256"]
        self.checkpoints = None if checkpoint_path is None else PublishedCheckpointStore(
            checkpoint_path, reader.path, max_checkpoints=max_checkpoints, max_bytes=max_bytes,
            execution_config={"version": EXECUTION_VERSION, "snapshot_schema": SNAPSHOT_SCHEMA,
                              "ttl": ttl, "work_limit": work_limit, "node_limit": node_limit,
                              "relation_limit": relation_limit, "block_size": block_size})

    def capability(self):
        result = exploration_capabilities()
        result.update(ttl_seconds=self.ttl, max_checkpoints=self.max_checkpoints, max_checkpoint_bytes=self.max_bytes)
        result["limits"].update(work_per_page=self.work_limit, session_nodes=self.node_limit, session_relations=self.relation_limit)
        result["snapshot_binding"] = "sha256 compact published-exploration-v1 framing of execution version and complete owner publication binding"
        result["read_model"] = "explicit-pinned-published-sqlite"
        if self.checkpoints is not None:
            result.update(storage="owner-selected-private-sqlite", restart_survival=True,
                          clock="UTC epoch seconds; backwards movement fails closed without state change",
                          database_byte_cap=self.checkpoints.max_pages * 4096,
                          storage_overhead="DELETE rollback journal may transiently add one database cap plus SQLite headers; free database pages are reused",
                          cleanup="expired/evicted records pruned transactionally on successful queries; explicit owner reset only for incompatible state")
        return result

    def explore(self, request):
        with self.lock:
            if self.checkpoints is not None:
                with self.checkpoints.transaction(self.clock) as checkpoint:
                    return self._explore(request, checkpoint.now, checkpoint)
            return self._explore(request, self.clock(), None)

    def _explore(self, request, now, checkpoint):
        with self.lock:
            if not isinstance(request, dict):
                raise ValueError("exploration request must be an object")
            cursor, continuing = request.get("cursor"), "cursor" in request
            if continuing and (set(request) != {"cursor"} or not isinstance(cursor, str)
                               or not re.fullmatch("[0-9a-f]{64}", cursor)):
                raise ValueError("continue exploration with one opaque cursor only")
            query = None if continuing else normalize_exploration(request)
            if checkpoint is None:
                for token, (expires, _) in list(self.records.items()):
                    if expires <= now:
                        self._remove(token)
            record = None
            if continuing:
                if checkpoint is not None:
                    expires, raw = checkpoint.get(cursor)
                else:
                    if cursor not in self.records:
                        raise ExplorationExpired("exploration expired or was evicted; restart from focus")
                    expires, raw = self.records[cursor]
                try:
                    record = json.loads(raw)
                    if (not isinstance(record, dict) or set(record) not in ({"revision", "state"}, {"revision", "result"})
                            or not isinstance(record.get("revision"), str)
                            or not isinstance(record.get("state", record.get("result")), dict)):
                        raise ValueError("invalid checkpoint framing")
                except (ValueError, UnicodeError, RecursionError) as error:
                    raise PublishedCheckpointError("checkpoint payload is invalid; no implicit reset") from error
                if record["revision"] != self.revision:
                    raise KnowledgeRevisionConflict("exploration publication binding changed")
            else:
                expires = now + self.ttl
            successor = None

            def operation(read, top):
                nonlocal successor
                if continuing and record.get("result") is not None:
                    return record["result"]
                rows = _Rows(read, self.block_size)
                if continuing:
                    state = record["state"]
                else:
                    if query.get("schema_version") == REQUEST_V2:
                        origin, roots, seed_relations = bind_origin(query, top["source_revision"], rows.node, rows.relation)
                        if len(roots) > self.node_limit or len(seed_relations) > self.relation_limit:
                            raise ValueError("exploration origin closure exceeds session limits")
                    else:
                        focus = rows.focus(query)
                        query["focus_node_id"] = focus["id"]
                        roots, seed_relations, origin = [focus["id"]], [], None
                    state = {"query": query, "queue": [[identifier, 0] for identifier in roots], "head": 0,
                             "after": "", "identity_after": "", "identity_added": 0,
                             "expanded_entities": [], "seen_nodes": list(roots),
                             "seen_relations": list(seed_relations), "page_number": 0}
                    if origin is not None:
                        state.update(origin=origin, roots=roots, seed_relations=seed_relations)
                result = self._advance_rows(state, rows, top)
                next_cursor = secrets.token_hex(32) if result["status"] == "paused" else None
                result["page"]["next_cursor"] = next_cursor
                if next_cursor:
                    successor = next_cursor, {"revision": self.revision, "state": state}
                    if len(_compact(successor[1]).encode()) > self.max_bytes:
                        raise ExplorationExpired("checkpoint exceeds cache capacity; narrow exploration")
                return result

            try:
                result = self.reader._read(operation)
            except PublishedSnapshotConflict as error:
                raise KnowledgeRevisionConflict("exploration snapshot changed; select the publication again") from error
            # No mutable query state is stored before the complete read succeeds.
            pending = []
            if continuing and record.get("result") is None:
                pending.append((cursor, expires, {"revision": self.revision, "result": result}))
            if successor is not None:
                pending.append((successor[0], expires, successor[1]))
            if checkpoint is None:
                self._admit_memory(pending)
            else:
                for arguments in pending:
                    checkpoint.put(*arguments)
            return copy.deepcopy(result)

    def _admit_memory(self, pending):
        # Stage immutable serialized records without changing the live cache.
        # Both replay and successor are protected from the same admission's
        # eviction. A failed pair leaves the input cursor/state intact.
        records, stored_bytes, protected = self.records.copy(), self.stored_bytes, set()
        for token, expires, record in pending:
            raw = _compact(record).encode("utf-8")
            if len(raw) > self.max_bytes:
                raise ExplorationExpired("checkpoint exceeds cache capacity; narrow exploration")
            if token in records:
                stored_bytes -= len(records.pop(token)[1])
            while records and (len(records) >= self.max_checkpoints or stored_bytes + len(raw) > self.max_bytes):
                oldest = next((key for key in records if key not in protected), None)
                if oldest is None:
                    raise ExplorationExpired("replay and successor together exceed checkpoint capacity; narrow exploration")
                stored_bytes -= len(records.pop(oldest)[1])
            records[token] = expires, raw
            stored_bytes += len(raw)
            protected.add(token)
        self.records, self.stored_bytes = records, stored_bytes

    def _advance_rows(self, state, rows, top):
        query, origin = state["query"], state.get("origin")
        focus = query.get("focus_node_id") if origin is None else origin["id"] if origin["kind"] == "node" else None
        roots = state["roots"] if origin is not None else [focus]
        seed_relations = state.get("seed_relations", [])
        primary = [focus] if origin is None and state["page_number"] == 0 else []
        emitted = []
        seen_nodes, seen_relations = set(state["seen_nodes"]), set(state["seen_relations"])
        node_reasons = {identifier: {"kind": "origin" if origin and origin["kind"] == "node"
                                    else "origin-endpoint" if origin else "focus"} for identifier in roots}
        edge_reasons = {identifier: {"kind": "origin"} for identifier in seed_relations}
        promoted, expanded_entities = set(), set(state["expanded_entities"])
        work, limit_reason = 0, None
        while state["head"] < len(state["queue"]) and work < self.work_limit:
            current, depth = state["queue"][state["head"]]
            if depth < query["max_depth"]:
                node = rows.node(current)
                if node is None:
                    raise PublishedReadModelError("exploration frontier node is missing")
                entity = node.get("entity_id")
                aliases = query["profile"] == "overview" and isinstance(entity, str) and entity.startswith("tos.") and entity not in expanded_entities
                alias = rows.alias(entity, state["identity_after"]) if aliases else None
                if alias is not None:
                    work += 1
                    if alias in seen_nodes or rows.node(alias)["source_graph"] not in query["sources"]:
                        if alias in seen_nodes:
                            position = next(i for i, (identifier, _) in enumerate(state["queue"]) if identifier == alias)
                            if state["queue"][position][1] > depth:
                                if alias not in promoted and len(primary) + len(promoted) >= query["page_nodes"]:
                                    break
                                state["queue"].pop(position)
                                state["queue"].insert(state["head"] + 1 + state["identity_added"], [alias, depth])
                                state["identity_added"] += 1
                                promoted.add(alias)
                                node_reasons[alias] = {"kind": "identity-carrier", "via_node_id": current, "entity_id": entity, "depth": depth}
                        state["identity_after"] = alias
                        continue
                    if len(seen_nodes) >= self.node_limit:
                        limit_reason = "session_nodes"
                        break
                    if len(primary) + len(promoted) >= query["page_nodes"]:
                        break
                    state["identity_after"] = alias
                    seen_nodes.add(alias)
                    state["seen_nodes"].append(alias)
                    state["queue"].insert(state["head"] + 1 + state["identity_added"], [alias, depth])
                    state["identity_added"] += 1
                    primary.append(alias)
                    node_reasons[alias] = {"kind": "identity-carrier", "via_node_id": current, "entity_id": entity, "depth": depth}
                    continue
                if aliases:
                    expanded_entities.add(entity)
                    state["expanded_entities"].append(entity)
                edge = rows.adjacent(current, state["after"])
            else:
                edge = None
            if edge is None:
                state["head"] += 1
                state["after"] = state["identity_after"] = ""
                state["identity_added"] = 0
                work += 1
                continue
            work += 1
            target = edge["to_id"] if edge["from_id"] == current else edge["from_id"]
            eligible = (edge["id"] not in seen_relations and edge["source_graph"] in query["sources"]
                        and rows.node(target) is not None and rows.node(target)["source_graph"] in query["sources"]
                        and (query["direction"] != "outgoing" or edge["from_id"] == current)
                        and (query["direction"] != "incoming" or edge["to_id"] == current)
                        and (not query["predicate_ids"] or edge["predicate_id"] in query["predicate_ids"])
                        and (query["profile"] != "overview" or (edge["predicate_id"] not in OVERVIEW_EXCLUDED_PREDICATES
                             and edge.get("relation_type_id") not in OVERVIEW_EXCLUDED_RELATION_TYPES)))
            if not eligible:
                state["after"] = edge["id"]
                continue
            new_node = target not in seen_nodes
            if len(seen_relations) >= self.relation_limit or (new_node and len(seen_nodes) >= self.node_limit):
                limit_reason = "session_relations" if len(seen_relations) >= self.relation_limit else "session_nodes"
                break
            if len(emitted) >= query["page_relations"] or (new_node and len(primary) + len(promoted) >= query["page_nodes"]):
                break
            state["after"] = edge["id"]
            seen_relations.add(edge["id"])
            state["seen_relations"].append(edge["id"])
            emitted.append(edge["id"])
            edge_reasons[edge["id"]] = {"kind": "traversal", "via_node_id": current, "depth": depth + 1}
            if new_node:
                seen_nodes.add(target)
                state["seen_nodes"].append(target)
                state["queue"].append([target, depth + 1])
                primary.append(target)
                node_reasons[target] = {"kind": "traversal", "via_node_id": current, "via_relation_id": edge["id"], "depth": depth + 1}
        state["page_number"] += 1
        selected = set(primary) | promoted | set(roots)
        for identifier in emitted:
            selected.update((rows.relation(identifier)["from_id"], rows.relation(identifier)["to_id"]))
        for identifier in selected - node_reasons.keys():
            node_reasons[identifier] = {"kind": "context-endpoint"}
        rows.load("node", selected)
        rows.load("relation", [*seed_relations, *emitted])
        if any(rows.nodes[identifier] is None for identifier in selected) or any(rows.relations[identifier] is None for identifier in [*seed_relations, *emitted]):
            raise PublishedReadModelError("exploration result endpoint/row closure is incomplete")
        status = "limit_reached" if limit_reason else "complete" if state["head"] == len(state["queue"]) else "paused"
        nodes = [_lens_carrier(rows.nodes[identifier], "compact", language="auto") for identifier in sorted(selected)]
        relations = [_lens_carrier(rows.relations[identifier], "compact", language="auto") for identifier in [*seed_relations, *emitted]]
        result = {
            "schema": RESULT_V2 if origin is not None else "tos_exploration_result_v1", "execution_version": EXECUTION_VERSION,
            "snapshot_revision": self.revision, "source_revision": top["source_revision"], "query": query,
            "focus": {"node_id": focus}, "status": status, "limit_reason": limit_reason,
            "nodes": nodes, "relations": relations,
            "scene": knowledge_scene(nodes, relations, focus, origin["id"] if origin and origin["kind"] == "relation" else None),
            "page": {"number": state["page_number"], "primary_node_ids": primary,
                     "context_node_ids": sorted(selected - set(primary)), "next_cursor": None,
                     "returned_nodes": len(selected), "returned_relations": len(relations), "work_units": work, "scope": "resumable-neighborhood"},
            "counts": {"discovered_nodes": len(seen_nodes), "emitted_relations": len(seen_relations) - len(seed_relations), "scope": "cumulative-discovered-not-global-total"},
            "inclusion": {"nodes": node_reasons, "relations": edge_reasons, "authority": "query-execution-not-semantic-proof"},
            "authority_boundary": top["authority_boundary"], "writes_to_tree": False,
        }
        if origin is not None:
            del result["focus"]
            result["origin"] = origin
            result["page"].update(primary_relation_ids=emitted, context_relation_ids=list(seed_relations))
        return result

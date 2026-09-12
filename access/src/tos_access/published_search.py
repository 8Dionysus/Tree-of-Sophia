"""Bounded search and exact carrier joins within one selected publication read.

This module never assembles, normalizes, publishes or loads a graph. The caller
selects the publication; checksums and cursor MACs do not grant source authority.
"""
from __future__ import annotations

import base64
import binascii
import hashlib
import hmac
import json
import time
from dataclasses import dataclass
from typing import Any

from .compressed_search_store import (
    MAX_ADDRESS, MIN_METADATA_BYTES, SearchStore, SearchInvalidRequest,
    SearchStaleBinding, SearchCursorError, SearchCursorExpired,
    SearchUnavailable, SearchBudgetExceeded,
)
from .knowledge import KNOWLEDGE_SOURCES
from .published_read_metadata import (
    LOCAL_READ_MODEL_SCHEMA, LENS_READER_SCHEMA, _compact, published_row_digest_key,
)
from .published_read_model import (
    PublishedKnowledgeReadModel, PublishedReadModelError, PublishedReadBudgetExceeded,
    PublishedSnapshotConflict, _json,
)
from .search_read_model import normalize_search_query, SearchReadModelError

SCHEMA = "tos_knowledge_search_compressed_v3"
CURSOR_SCHEMA = "tos_published_search_cursor_v1"
MAX_CURSOR_BYTES = 65_536
MAX_FILTER_BYTES = 65_536
OUTPUT_RESERVE_BYTES = 524_288
FINAL_READ_BYTES = 65_536 + 1024
FINAL_READ_ROWS = 513  # Top/revision metadata: at most 256 chunks each + clock.
_MAC_DOMAIN = b"tos-published-search-v1\0"
_COLUMNS = {
    "node": ("id", "entity_id", "native_id", "source_graph", "kind_id", "type_id"),
    "relation": ("id", "native_id", "source_graph", "from_id", "to_id", "predicate_id", "relation_type_id"),
}
_RANKS = ("exact-identity", "identity-prefix", "visible-text", "serialized-text")
_SEARCH_OBJECTS = {
    "prepared_documents": "table", "search_header": "table", "search_documents": "table",
    "search_values": "table", "search_text_chunks": "table", "search_terms": "table",
    "search_blocks": "table", "search_document_terms": "table", "search_blocks_nonempty": "index",
    "sqlite_autoindex_prepared_documents_1": "index", "sqlite_autoindex_prepared_documents_2": "index",
}


@dataclass(frozen=True)
class PublishedSearchLimits:
    candidate_budget: int = 256
    verification_bytes: int = 65_536
    metadata_bytes: int = MIN_METADATA_BYTES
    body_bytes: int = 4 * 1024 * 1024

    def __post_init__(self):
        for key, low, high in (("candidate_budget", 2, 4096),
                               ("verification_bytes", 8192, 8 * 1024 * 1024),
                               ("metadata_bytes", MIN_METADATA_BYTES, 64 * 1024 * 1024),
                               ("body_bytes", 1, 64 * 1024 * 1024)):
            value = getattr(self, key)
            if type(value) is not int or not low <= value <= high:
                raise SearchInvalidRequest(f"{key} must be an integer in {low}..{high}")


def _hash(value: Any) -> str:
    # Binding identity follows dictionary equality across owner config reloads;
    # emitted carrier JSON framing intentionally remains a separate contract.
    raw = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


def _id_hash(value: Any) -> str:
    # Exact SearchStore JSON-ID framing, including JSON string escaping.
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True)
                          .encode("utf-8", "surrogatepass")).hexdigest()


def _hex(value):
    return isinstance(value, str) and len(value) == 64 and all(c in "0123456789abcdef" for c in value)


def _filters(values, name, *, known=None):
    if values is None:
        return sorted(known) if known is not None else []
    if (not isinstance(values, list) or len(values) > 100
            or any(not isinstance(value, str) for value in values)):
        raise SearchInvalidRequest(f"{name} must contain at most 100 strings")
    normalized = {value for value in values if value}
    if known is not None and normalized - set(known):
        raise SearchInvalidRequest("unsupported knowledge sources")
    return sorted(normalized) if normalized or known is None else sorted(known)


def _decode(cursor):
    if not isinstance(cursor, str) or not 1 <= len(cursor) <= MAX_CURSOR_BYTES:
        raise SearchCursorError("invalid published search cursor length")
    try:
        if any(c not in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_" for c in cursor):
            raise ValueError("cursor alphabet")
        raw = base64.b64decode(cursor + "=" * (-len(cursor) % 4), altchars=b"-_", validate=True)
        envelope = _json(raw.decode("utf-8"))
        if (not isinstance(envelope, dict) or set(envelope) != {"state", "mac"}
                or not isinstance(envelope["state"], dict) or not _hex(envelope["mac"])):
            raise ValueError("cursor envelope")
        state = envelope["state"]
        if (set(state) != {"schema", "binding", "query", "expires", "nodes", "relations"}
                or state["schema"] != CURSOR_SCHEMA or not _hex(state["binding"])
                or not _hex(state["query"]) or type(state["expires"]) is not int
                or not 1 <= state["expires"] <= MAX_ADDRESS):
            raise ValueError("cursor state")
        for name in ("nodes", "relations"):
            kind = state[name]
            if (not isinstance(kind, dict) or set(kind) != {"cursor", "exhausted", "pending"}
                    or type(kind["exhausted"]) is not bool or not isinstance(kind["pending"], list)
                    or len(kind["pending"]) > 100
                    or (kind["cursor"] is not None and not isinstance(kind["cursor"], dict))
                    or (kind["exhausted"] and kind["cursor"] is not None)):
                raise ValueError("cursor kind")
            addresses = set()
            for pending in kind["pending"]:
                if (not isinstance(pending, list) or len(pending) != 3
                        or type(pending[0]) is not int or not 1 <= pending[0] <= MAX_ADDRESS
                        or type(pending[1]) is not int or not 0 <= pending[1] <= 3
                        or not _hex(pending[2]) or pending[0] in addresses):
                    raise ValueError("cursor pending match")
                addresses.add(pending[0])
        return state, envelope["mac"]
    except (ValueError, TypeError, UnicodeError, binascii.Error, PublishedReadModelError) as exc:
        raise SearchCursorError("invalid published search cursor") from exc


def _encode(state, key):
    raw = _compact(state).encode("utf-8")
    mac = hmac.new(key, _MAC_DOMAIN + raw, hashlib.sha256).hexdigest()
    cursor = base64.urlsafe_b64encode(_compact({"state": state, "mac": mac}).encode("utf-8")).decode("ascii").rstrip("=")
    if len(cursor) > MAX_CURSOR_BYTES:
        raise SearchBudgetExceeded("published search cursor exceeds its framing budget")
    return cursor


class PublishedSearchService:
    def __init__(self, reader: PublishedKnowledgeReadModel, *, limits: PublishedSearchLimits | None = None):
        self.reader = reader
        self.limits = limits or PublishedSearchLimits()
        available_body = min(self.limits.body_bytes, reader.limits.max_response_bytes - OUTPUT_RESERVE_BYTES)
        if available_body < 2 * reader.limits.max_row_bytes:
            raise SearchInvalidRequest("search body budget must admit one maximal carrier per kind")
        self.body_per_kind = available_body // 2

    def capability(self):
        schema = self.reader.snapshot_binding["read_model_schema"]
        available = schema == LOCAL_READ_MODEL_SCHEMA
        packet = {"available": available, "schema": SCHEMA, "read_model_schema": schema,
                **({} if available else {"reason": "local-prepared-publication-required"}),
                "ordering_scope": "global-rank", "cursor": "authenticated-stateless-15-minute",
                "counts": "exact-only-initial-complete-kind", "writes_to_tree": False,
                "limits": {**vars(self.limits), "body_bytes_per_kind": self.body_per_kind,
                           "cursor_bytes": MAX_CURSOR_BYTES, "inner_pages_per_kind": 1}}
        if not available:
            return packet
        def operation(read, top):
            self._key(read, top, self.reader.snapshot_binding)
            return {**packet, "source_revision": top["source_revision"],
                    "authority_boundary": top["authority_boundary"]}
        return self._run(operation)

    def _run(self, operation):
        try:
            return self.reader._read(operation)
        except PublishedSnapshotConflict as exc:
            raise SearchStaleBinding(str(exc)) from exc
        except PublishedReadBudgetExceeded as exc:
            raise SearchBudgetExceeded(str(exc)) from exc
        except PublishedReadModelError as exc:
            raise SearchUnavailable(str(exc)) from exc

    @staticmethod
    def _key(read, top, binding):
        if top["read_model_schema"] != LOCAL_READ_MODEL_SCHEMA or top["schema"] != LENS_READER_SCHEMA:
            raise SearchUnavailable("published search requires a local prepared reader header")
        objects = read.query("SELECT name,type FROM sqlite_master WHERE name IN (SELECT value FROM json_each(?))", (_compact(sorted(_SEARCH_OBJECTS)),))
        if {row["name"]: row["type"] for row in objects} != _SEARCH_OBJECTS:
            raise SearchUnavailable("prepared search tables or address indexes are unavailable")
        work = {"metadata_probes": 0, "metadata_rows": 0, "metadata_bytes": 0}
        key = SearchStore._check_header(read.db, SearchStore._header(binding), work=work)
        read.bytes += work["metadata_bytes"]
        return key, work["metadata_bytes"]

    def search(self, query="", *, sources=None, kind_ids=None, predicate_ids=None,
               cursor: str | None = None, limit=40):
        try:
            needle = normalize_search_query(query)
        except SearchReadModelError as exc:
            raise SearchInvalidRequest(str(exc)) from exc
        if type(limit) is not int or not 1 <= limit <= 100:
            raise SearchInvalidRequest("limit must be an integer in 1..100")
        filters = {"sources": _filters(sources, "sources", known=KNOWLEDGE_SOURCES),
                   "kind_ids": _filters(kind_ids, "kind_ids"),
                   "predicate_ids": _filters(predicate_ids, "predicate_ids")}
        try:
            frame = _compact([needle, filters]).encode("utf-8")
        except (UnicodeError, ValueError) as exc:
            raise SearchInvalidRequest("invalid search request framing") from exc
        if len(frame) > MAX_FILTER_BYTES:
            raise SearchInvalidRequest("search query/filter frame exceeds 65536 bytes")
        binding = self.reader.snapshot_binding
        if binding["read_model_schema"] != LOCAL_READ_MODEL_SCHEMA:
            raise SearchUnavailable("published search requires the explicit local prepared profile")
        binding_hash, query_hash = _hash(binding), _hash([binding, needle, filters])
        incoming = _decode(cursor) if cursor is not None else None
        if incoming is not None and incoming[0]["binding"] != binding_hash:
            raise SearchStaleBinding("search cursor selects another publication; restart query")

        def operation(read, top):
            key, header_bytes = self._key(read, top, binding)
            if incoming is None:
                state = {"schema": CURSOR_SCHEMA, "binding": binding_hash, "query": query_hash,
                         "expires": int(time.time()) + 900,
                         "nodes": {"cursor": None, "exhausted": False, "pending": []},
                         "relations": {"cursor": None, "exhausted": False, "pending": []}}
            else:
                state, mac = incoming
                expected = hmac.new(key, _MAC_DOMAIN + _compact(state).encode("utf-8"), hashlib.sha256).hexdigest()
                if not hmac.compare_digest(expected, mac):
                    raise SearchCursorError("invalid published search cursor integrity; restart query")
                if state["query"] != query_hash:
                    raise SearchCursorError("published search cursor query/filter mismatch")
                if state["expires"] <= time.time():
                    raise SearchCursorExpired("published search cursor expired")
            byte_share = (read.limits.max_response_bytes - read.bytes - FINAL_READ_BYTES) // 2
            row_share = (read.limits.max_rows - read.rows - FINAL_READ_ROWS) // 2
            if byte_share < max(self.limits.metadata_bytes + self.limits.verification_bytes,
                                4 * read.limits.max_row_bytes + 1024) or row_share < 270:
                raise SearchBudgetExceeded("reader budget cannot admit bounded search and carrier continuation")
            nodes, node_ranks, node_work = self._kind(read, state["nodes"], "node", binding, needle,
                {"source_graph": filters["sources"], "kind_id": filters["kind_ids"]}, limit, byte_share, row_share)
            relations, relation_ranks, relation_work = self._kind(read, state["relations"], "relation", binding, needle,
                {"source_graph": filters["sources"], "predicate_id": filters["predicate_ids"]}, limit, byte_share, row_share)
            more = any(not state[name]["exhausted"] or state[name]["pending"] for name in ("nodes", "relations"))
            next_cursor = _encode(state, key) if more else None
            counts = {"returned_nodes": len(nodes), "returned_relations": len(relations),
                      "scope": "exact-only-initial-complete-kind"}
            for name, items in (("nodes", nodes), ("relations", relations)):
                counts["matching_" + name] = len(items) if cursor is None and state[name]["exhausted"] and not state[name]["pending"] else None
            return {"schema": SCHEMA, "source_revision": top["source_revision"], "query": query.strip(), "filters": filters,
                    "page": {"cursor": cursor, "next_cursor": next_cursor, "limit_per_kind": limit,
                             "ordering_scope": "global-rank", "has_more": more},
                    "counts": counts, "nodes": nodes, "relations": relations,
                    "ranks": {"nodes": node_ranks, "relations": relation_ranks},
                    "authority_boundary": top["authority_boundary"],
                    "work": {"nodes": node_work, "relations": relation_work,
                             "read_rows": read.rows, "read_bytes": read.bytes,
                             "read_vm_steps": read.steps, "search_header_bytes": header_bytes}}
        return self._run(operation)

    def _kind(self, read, state, kind, binding, needle, filters, limit, byte_share, row_share):
        byte_end, row_end = read.bytes + byte_share, read.rows + row_share
        work = {"inner_pages": 0, "body_rows": 0, "body_bytes": 0, "deferred": False, "search": None}
        if not state["pending"] and not state["exhausted"]:
            page = SearchStore.query_transaction(read.db, binding=binding, kind=kind, query=needle,
                filters=filters, page_size=limit, cursor=state["cursor"],
                candidate_budget=self.limits.candidate_budget, verification_bytes=self.limits.verification_bytes,
                max_metadata_bytes=self.limits.metadata_bytes)
            # Direct search BLOB/chunk fetches do not pass through _Read.query.
            # Charge them here as well as reporting the search owner's counters.
            read.bytes += page["work"]["metadata_bytes"] + page["work"]["verification_bytes"]
            state["pending"] = [[row["doc_id"], row["rank"], _id_hash(row["id"])] for row in page["matches"]]
            state["cursor"], state["exhausted"] = page["next_cursor"], not page["has_more"]
            work.update(inner_pages=1, search=page["work"])
        items, ranks = [], []
        while state["pending"] and len(items) < limit:
            if read.rows + 270 > row_end:
                work["deferred"] = True
                break
            address, rank, expected_id = state["pending"][0]
            probe = read.query("SELECT CASE WHEN typeof(id)='text' THEN length(CAST(id AS BLOB)) ELSE -1 END AS bytes FROM prepared_documents WHERE doc_id=? AND kind=? LIMIT 2", (address, kind))
            if len(probe) != 1 or type(probe[0]["bytes"]) is not int or probe[0]["bytes"] < 1:
                raise SearchUnavailable("missing or invalid prepared search address mapping")
            if probe[0]["bytes"] > read.limits.max_row_bytes:
                raise SearchBudgetExceeded("prepared search identity exceeds the reader row budget")
            id_bytes = probe[0]["bytes"]
            if read.bytes + id_bytes > byte_end:
                work["deferred"] = True
                break
            mapped = read.query("SELECT id FROM prepared_documents WHERE doc_id=? AND kind=? LIMIT 2", (address, kind))
            if len(mapped) != 1 or _id_hash(mapped[0]["id"]) != expected_id:
                raise SearchUnavailable("prepared search address differs from exact search identity")
            identifier = mapped[0]["id"]
            table = "knowledge_nodes" if kind == "node" else "knowledge_relations"
            column_bytes = "+".join(f"length(CAST({column} AS BLOB))" for column in _COLUMNS[kind])
            text_columns = " AND ".join(f"typeof({column})='text'" for column in (*_COLUMNS[kind], "json"))
            probe = read.query(f"SELECT CASE WHEN {text_columns} THEN length(CAST(json AS BLOB)) ELSE -1 END AS body, ({column_bytes}) AS indexed FROM {table} WHERE id=? LIMIT 2", (identifier,))
            if len(probe) != 1 or probe[0]["body"] < 1 or type(probe[0]["indexed"]) is not int:
                raise SearchUnavailable("missing or invalid prepared search carrier")
            if probe[0]["body"] > read.limits.max_row_bytes:
                raise SearchBudgetExceeded("prepared search carrier exceeds the reader row budget")
            body_bytes, indexed_bytes = probe[0]["body"], probe[0]["indexed"]
            if body_bytes + indexed_bytes + 1024 > max(read.limits.max_row_bytes, 131072) + 4096:
                raise SearchBudgetExceeded("prepared search carrier exceeds the reader record budget")
            digest = read.query("SELECT count(*) AS chunks,coalesce(sum(length(CAST(json_chunk AS BLOB))),0) AS bytes FROM edge_meta WHERE key=?", (published_row_digest_key(kind, identifier),))[0]
            if not 1 <= digest["chunks"] <= 256 or not 1 <= digest["bytes"] <= 1024:
                raise SearchUnavailable("invalid selected carrier checksum framing")
            fetch_bytes = id_bytes + body_bytes + indexed_bytes + digest["bytes"]
            if (work["body_bytes"] + body_bytes > self.body_per_kind or read.bytes + fetch_bytes > byte_end
                    or read.rows + 2 + digest["chunks"] > row_end):
                work["deferred"] = True
                break
            selected = read.items(kind, "id=?", (identifier,), 1)
            if len(selected) != 1 or _id_hash(selected[0].get("id")) != expected_id:
                raise SearchUnavailable("selected prepared search identity closure differs")
            items.append(selected[0])
            ranks.append({"doc_id": address, "rank": rank, "explanation": _RANKS[rank] if needle else "all-items"})
            work["body_rows"] += 1
            work["body_bytes"] += body_bytes
            state["pending"].pop(0)
        return items, ranks, work

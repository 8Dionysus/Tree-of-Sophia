"""Opt-in cold reads of the existing published SQLite/D1 knowledge schema.

This module never builds, migrates, normalizes, publishes, or opens the offline
normalization cache. An owner supplies an exact expected snapshot binding;
database metadata alone is not permission to select a new source revision.
Checksums detect accidental byte drift, not a malicious writer with the same
filesystem/producer authority. Full semantic validation remains a build step.
"""
from __future__ import annotations

import copy
import json
import math
import sqlite3
import stat
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from .published_read_metadata import (
    BINDING_SCHEMA, CATALOG_KEY, TOP_KEY, LENS_READER_SCHEMA, LENS_READ_MODEL_SCHEMAS, PublishedReadModelError,
    _BINDING_KEYS, _HASH, _compact, _normalization, _validate_top,
    emitted_row_digest, published_reader_metadata, published_row_digest_key,
    published_snapshot_binding,
)
from .source_read_projection import source_read_targets
_INDEXES = (
    "knowledge_nodes_native_idx", "knowledge_nodes_entity_idx",
    "knowledge_relations_native_idx", "knowledge_relations_from_seek",
    "knowledge_relations_to_seek",
)
SUPPORTED_READ_MODEL_SCHEMAS = LENS_READ_MODEL_SCHEMAS | {"tos_cloudflare_edge_read_model_v8"}


class PublishedSnapshotConflict(PublishedReadModelError):
    """The selected publication is stale, replaced, or changed during a read."""


class PublishedReadBudgetExceeded(PublishedReadModelError):
    """An exact response cannot be produced within the declared read budget."""


@dataclass(frozen=True)
class PublishedReadLimits:
    max_matches: int = 128
    max_row_bytes: int = 1_048_576
    max_metadata_bytes: int = 8_388_608
    max_response_bytes: int = 16_777_216
    max_rows: int = 4096
    max_vm_steps: int = 200_000

    def __post_init__(self):
        if any(type(value) is not int or value < 1 for value in vars(self).values()):
            raise ValueError("prepared reader limits must be positive integers")


def _json(raw: str) -> Any:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate JSON member")
            result[key] = value
        return result

    def finite(text):
        value = float(text)
        if not math.isfinite(value):
            raise ValueError("nonfinite JSON number")
        return value

    try:
        value = json.loads(raw, object_pairs_hook=pairs, parse_float=finite,
                           parse_constant=lambda _: (_ for _ in ()).throw(ValueError("nonfinite JSON")))
        pending = [(value, 0)]
        members = 0
        while pending:
            item, depth = pending.pop()
            members += 1
            if depth > 64 or members > 300_000:
                raise ValueError("JSON structural budget")
            if isinstance(item, dict):
                pending.extend((child, depth + 1) for child in item.values())
            elif isinstance(item, list):
                pending.extend((child, depth + 1) for child in item)
        return value
    except (ValueError, TypeError, RecursionError, OverflowError) as error:
        raise PublishedReadModelError("prepared reader contains invalid or over-complex JSON") from error


def _file_state(path: Path) -> tuple:
    try:
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode):
            raise PublishedReadModelError("prepared read model must be a regular non-symlink file")
        return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
    except OSError as error:
        raise PublishedReadModelError("configured prepared read model is unavailable") from error


class _Read:
    def __init__(self, connection: sqlite3.Connection, limits: PublishedReadLimits):
        self.db, self.limits = connection, limits
        self.rows = self.bytes = self.steps = 0
        self.exhausted = False
        connection.setlimit(sqlite3.SQLITE_LIMIT_LENGTH, max(limits.max_row_bytes, 131_072) + 4096)
        connection.set_progress_handler(self._progress, 100)

    def _progress(self):
        self.steps += 100
        self.exhausted = self.steps > self.limits.max_vm_steps
        return int(self.exhausted)

    def query(self, sql: str, args=()) -> list[sqlite3.Row]:
        # Account for the sub-100-op tail of each statement as well as progress
        # callbacks. Many short digest lookups must not escape the work budget.
        self.steps += 100
        if self.steps > self.limits.max_vm_steps:
            raise PublishedReadBudgetExceeded("prepared inspection exceeds its SQLite work budget")
        result = []
        for row in self.db.execute(sql, args):
            self.rows += 1
            self.bytes += sum(len(value.encode("utf-8")) for value in row if isinstance(value, str))
            if self.rows > self.limits.max_rows:
                raise PublishedReadBudgetExceeded("prepared inspection exceeds its row budget")
            if self.bytes > self.limits.max_response_bytes:
                raise PublishedReadBudgetExceeded("prepared inspection exceeds its byte budget")
            result.append(row)
        return result

    def text(self, raw: Any, maximum: int) -> str:
        if not isinstance(raw, str):
            raise PublishedReadModelError("prepared JSON column is not text")
        size = len(raw.encode("utf-8"))
        if size > maximum:
            raise PublishedReadBudgetExceeded("prepared inspection exceeds its byte budget")
        return raw

    def metadata(self, key: str, maximum: int | None = None) -> tuple[str, Any]:
        chunks = self.query("SELECT part,json_chunk FROM edge_meta WHERE key=? ORDER BY part LIMIT 257", (key,))
        if not chunks:
            raise PublishedReadModelError(f"prepared read model is missing metadata: {key}")
        if len(chunks) > 256:
            raise PublishedReadBudgetExceeded("prepared metadata exceeds its chunk budget")
        if [row["part"] for row in chunks] != list(range(len(chunks))):
            raise PublishedReadModelError("prepared metadata chunks are incomplete")
        raw = "".join(self.text(row["json_chunk"], 131_072) for row in chunks)
        if len(raw.encode("utf-8")) > (maximum or self.limits.max_metadata_bytes):
            raise PublishedReadBudgetExceeded("prepared metadata exceeds its byte budget")
        return raw, _json(raw)

    def items(self, kind: str, selector: str, args: tuple, limit: int, *, before_parse=None) -> list[dict[str, Any]]:
        columns = ("id", "entity_id", "native_id", "source_graph", "kind_id", "type_id") if kind == "node" else (
            "id", "native_id", "source_graph", "from_id", "to_id", "predicate_id", "relation_type_id")
        table = "knowledge_nodes" if kind == "node" else "knowledge_relations"
        # First bound identities; never sort/materialize wide full packets to
        # resolve an alias. Payload fetching then uses only exact primary keys.
        identities = self.query(f"SELECT id FROM {table} WHERE {selector} ORDER BY id LIMIT ?", (*args, limit + 1))
        if len(identities) > limit:
            raise PublishedReadBudgetExceeded("prepared inspection has too many identity matches")
        selected = self.query(
            f"SELECT {','.join(columns)},json FROM {table} WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?",
            (_compact([row["id"] for row in identities]), limit + 1)) if identities else []
        if len(selected) != len(identities):
            raise PublishedReadModelError("prepared identity selection has incomplete row closure")
        result = []
        for row in selected:
            raw = self.text(row["json"], self.limits.max_row_bytes)
            _, expected = self.metadata(published_row_digest_key(kind, row["id"]), 1024)
            if (not isinstance(expected, dict) or set(expected) != {"sha256"}
                    or expected != emitted_row_digest(raw)):
                raise PublishedReadModelError("emitted knowledge row checksum differs")
            if before_parse is not None:
                before_parse(raw)
            item = _json(raw)
            if not isinstance(item, dict) or any(str(item.get(key) or "") != row[key] for key in columns):
                raise PublishedReadModelError("knowledge row identity/index columns differ from its full packet")
            result.append(item)
        return result


class PublishedKnowledgeReadModel:
    """Cold, read-only reader pinned to an explicitly selected publication.

    Each operation opens a read-only connection and one read transaction. A
    fresh post-transaction observation rejects concurrent publication and ABA;
    no connection, graph, mutable cursor, or cache crosses requests/restarts.
    Exact counts may hit a VM budget and refuse; they are never approximated.
    """
    def __init__(self, path: str | Path, expected: dict[str, Any], *, limits: PublishedReadLimits | None = None):
        self.path = Path(path).expanduser().absolute()
        if (not isinstance(expected, dict) or set(expected) != _BINDING_KEYS
                or expected.get("schema") != BINDING_SCHEMA
                or type(expected.get("publication_epoch")) is not int
                or not 0 <= expected["publication_epoch"] <= 9_007_199_254_740_991
                or not _normalization(expected.get("normalization_binding"))
                or any(not isinstance(expected.get(key), str) or not _HASH.fullmatch(expected[key])
                       for key in ("metadata_sha256", "source_revision", "data_revision"))):
            raise ValueError("an exact owner-selected prepared snapshot binding is required")
        self._expected = copy.deepcopy(expected)
        self.limits = limits or PublishedReadLimits()

    @property
    def snapshot_binding(self):
        return copy.deepcopy(self._expected)

    def _connect(self):
        connection = sqlite3.connect(self.path.as_uri() + "?mode=ro", uri=True, isolation_level=None, timeout=0.1)
        connection.row_factory = sqlite3.Row
        connection.execute("PRAGMA query_only=ON")
        return connection

    def _snapshot(self, read: _Read):
        raw, top = read.metadata(TOP_KEY, 65_536)
        _validate_top(top)
        if top["read_model_schema"] not in SUPPORTED_READ_MODEL_SCHEMAS:
            raise PublishedReadModelError("prepared reader does not support this read-model schema")
        if (top["read_model_schema"] in LENS_READ_MODEL_SCHEMAS
                and top['schema'] != LENS_READER_SCHEMA):
            raise PublishedReadModelError("prepared lens reader metadata requires the lens binding")
        _, revision = read.metadata("data_revision", 1024)
        clocks = read.query("SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1 LIMIT 2")
        if (len(clocks) != 1 or type(clocks[0]["epoch"]) is not int
                or not isinstance(revision, dict) or revision != {"sha256": top["data_revision"]}):
            raise PublishedReadModelError("prepared reader has no coherent publication clock/revision")
        binding = published_snapshot_binding(top, clocks[0]["epoch"])
        if emitted_row_digest(raw)["sha256"] != binding["metadata_sha256"]:
            raise PublishedReadModelError("prepared header is not in its declared emitted JSON framing")
        if binding != self._expected:
            raise PublishedSnapshotConflict("prepared knowledge snapshot differs; select the current publication explicitly")
        return top

    def _read(self, operation: Callable[[_Read, dict], Any]):
        before = _file_state(self.path)
        read = None
        operation_running = False
        try:
            with closing(self._connect()) as connection:
                if _file_state(self.path) != before:
                    raise PublishedSnapshotConflict("prepared read-model file changed while opening")
                read = _Read(connection, self.limits)
                connection.execute("BEGIN")
                top = self._snapshot(read)
                required = set(_INDEXES)
                if top['read_model_schema'] in LENS_READ_MODEL_SCHEMAS:
                    required.update(('knowledge_nodes_identity_seek', 'knowledge_lens_order_sort',
                                     'knowledge_lens_order_from', 'knowledge_lens_order_to',
                                     'knowledge_lens_order_pair'))
                indexes = read.query("SELECT name FROM sqlite_master WHERE type='index' AND name IN (SELECT value FROM json_each(?))", (_compact(sorted(required)),))
                if {row["name"] for row in indexes} != required:
                    raise PublishedReadModelError("prepared reader adjacency/identity migration is unavailable")
                operation_running = True
                result = operation(read, top)
                operation_running = False
                try:
                    response_bytes = len(_compact(result).encode("utf-8"))
                except (UnicodeError, ValueError, RecursionError) as error:
                    raise PublishedReadModelError("prepared response contains invalid JSON values") from error
                if response_bytes > self.limits.max_response_bytes:
                    raise PublishedReadBudgetExceeded("prepared response exceeds its byte budget")
                connection.execute("COMMIT")
                # New read transaction: the previous transaction could legally
                # have read an older WAL snapshot throughout the operation.
                connection.execute("BEGIN")
                self._snapshot(read)
                connection.execute("COMMIT")
                if _file_state(self.path) != before:
                    raise PublishedSnapshotConflict("prepared read-model file changed during query")
                return result
        except PublishedReadModelError:
            raise
        except (UnicodeError, RecursionError) as error:
            raise PublishedReadModelError("prepared response contains invalid JSON values") from error
        except ValueError as error:
            if operation_running:
                raise  # Preserve the query API's typed validation/conflict errors.
            raise PublishedReadModelError("prepared metadata contains invalid JSON values") from error
        except sqlite3.Error as error:
            if read is not None and read.exhausted:
                raise PublishedReadBudgetExceeded("prepared inspection exceeds its SQLite work budget") from error
            raise PublishedReadModelError("configured prepared read model is unavailable or invalid") from error

    def catalog(self) -> dict[str, Any]:
        def operation(read, top):
            raw, catalog = read.metadata(CATALOG_KEY)
            if (not isinstance(catalog, dict) or catalog.get("schema") != "tos_knowledge_catalog_v1"
                    or catalog.get("source_revision") != top["source_revision"]
                    or emitted_row_digest(raw)["sha256"] != top["catalog_sha256"]):
                raise PublishedReadModelError("prepared catalog differs from the selected snapshot")
            return catalog
        return self._read(operation)

    def status(self) -> dict[str, Any]:
        """Check only the selected publication header/indices, never every row."""
        return self._read(lambda read, top: {
            "schema": "tos_published_read_status_v1",
            "read_model_schema": top["read_model_schema"], "graph_schema": top["graph_schema"],
            "source_revision": top["source_revision"], "data_revision": top["data_revision"],
            "publication_epoch": self._expected["publication_epoch"],
            "scope": "selected-publication-metadata-and-required-indices",
            "verifies_all_rows": False, "writes_to_tree": False,
        })

    def temporal_compare(self, request: dict[str, Any]) -> dict[str, Any]:
        """Compare exact Claim/value/subject rows in one selected snapshot."""
        from .temporal_comparison import compare_temporal_operands

        def operation(read, top):
            # The shared evaluator needs at most two Claims, two values and
            # two documentary subjects. Never use inspect's alias fallback.
            return compare_temporal_operands(
                top["source_revision"], request,
                lambda identifier: read.items("node", "id=?", (identifier,), 1),
            )
        return self._read(operation)

    @staticmethod
    def _identifier(value):
        identifier = str(value).strip()
        if not identifier or len(identifier) > 4096:
            raise ValueError("knowledge identifier must contain 1 to 4096 characters")
        return identifier

    @staticmethod
    def _refs(items):
        return sorted({ref for item in items
                       for ref in (item.get("source_refs") if isinstance(item.get("source_refs"), list) else [])
                       if isinstance(ref, str) and ref})

    def node(self, identifier: str, relation_limit: int = 200) -> dict[str, Any]:
        identifier = self._identifier(identifier)
        if type(relation_limit) is not int or not 0 <= relation_limit <= 1000:
            raise ValueError("relation_limit must be an integer between 0 and 1000")
        def operation(read, top):
            exact = read.items("node", "id=?", (identifier,), 1)
            entity = [] if exact else read.items("node", "entity_id=?", (identifier,), self.limits.max_matches)
            matches = exact or entity or read.items("node", "native_id=?", (identifier,), self.limits.max_matches)
            if not matches:
                raise KeyError(f"unknown ToS knowledge node: {identifier}")
            ids = _compact([item["id"] for item in matches])
            incident = ("SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id IN (SELECT value FROM json_each(?)) "
                        "UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id IN (SELECT value FROM json_each(?))")
            related_count = read.query(f"SELECT count(*) AS total FROM ({incident})", (ids, ids))[0]["total"]
            # Explicit ID range limit precedes full packet loading/digest reads.
            selected_ids = read.query(f"SELECT id FROM ({incident}) ORDER BY id LIMIT ?", (ids, ids, relation_limit))
            selected = read.items("relation", "id IN (SELECT value FROM json_each(?))", (_compact([row["id"] for row in selected_ids]),), relation_limit)
            return {
                "schema": "tos_knowledge_node_packet_v1", "source_revision": top["source_revision"],
                "requested_id": identifier, "ambiguous_native_id": not exact and not entity and len(matches) > 1,
                "shared_entity_id": len(entity) > 1, "matches": matches, "related_relations": selected,
                "counts": {"matches": len(matches), "related_relations": related_count, "returned_relations": len(selected)},
                "source_refs": self._refs([*matches, *selected]), "authority_boundary": top["authority_boundary"],
                "source_read_targets": source_read_targets([*matches, *selected], top["source_revision"]),
            }
        return self._read(operation)

    def relation(self, identifier: str) -> dict[str, Any]:
        identifier = self._identifier(identifier)
        def operation(read, top):
            exact = read.items("relation", "id=?", (identifier,), 1)
            matches = exact or read.items("relation", "native_id=?", (identifier,), self.limits.max_matches)
            if not matches:
                raise KeyError(f"unknown ToS knowledge relation: {identifier}")
            ids = sorted({item[field] for item in matches for field in ("from_id", "to_id")})
            endpoints = read.items("node", "id IN (SELECT value FROM json_each(?))", (_compact(ids),), 2 * self.limits.max_matches)
            if {item["id"] for item in endpoints} != set(ids):
                raise PublishedReadModelError("prepared relation endpoint closure is incomplete")
            return {
                "schema": "tos_knowledge_relation_packet_v1", "source_revision": top["source_revision"],
                "requested_id": identifier, "ambiguous_native_id": not exact and len(matches) > 1,
                "matches": matches, "endpoints": endpoints,
                "counts": {"matches": len(matches), "endpoints": len(endpoints)},
                "source_refs": self._refs([*matches, *endpoints]), "authority_boundary": top["authority_boundary"],
                "source_read_targets": source_read_targets([*matches, *endpoints], top["source_revision"]),
            }
        return self._read(operation)

"""Bounded auxiliary semantic validation in an existing caller transaction.

This is not an admission, source assembler, publisher, or full-validator bypass.
Any refusal requires caller rollback of ALL lanes. A delta remains pending until
the caller publishes its rows/header/search and explicitly verifies the binding.
"""
from __future__ import annotations

import hashlib
import copy
import json
import math
from dataclasses import dataclass
from pathlib import Path

from . import knowledge as k
from .prepared_publication import PreparedChange, SCHEMA
from .published_read_metadata import (
    TOP_KEY, _compact, emitted_row_digest, published_row_digest_key,
    published_snapshot_binding,
)

VERSION = "tos_auxiliary_semantic_index_v1"
KINDS = ("node", "relation")
COUNT_KEYS = ("registered_node_count", "unmapped_node_count", "registered_relation_count",
              "unmapped_relation_count", "claim_contract_count", "cross_layer_relation_count")
MAX_ORDER = 9_007_199_254_740_991


@dataclass(frozen=True)
class SemanticIndexLimits:
    max_changes: int = 4096
    max_rows: int = 2_000_000
    max_queries: int = 2_000_000
    max_writes: int = 2_000_000
    max_read_bytes: int = 256 * 1024 * 1024
    max_input_bytes: int = 32 * 1024 * 1024
    max_input_values: int = 2_000_000
    max_row_bytes: int = 1_048_576
    max_output_bytes: int = 8 * 1024 * 1024
    max_output_items: int = 65536
    max_bytes: int = 256 * 1024 * 1024

    def __post_init__(self):
        if any(type(v) is not int or v < 1 for v in vars(self).values()):
            raise ValueError("semantic index limits must be positive integers")
        if self.max_bytes > 2**40:
            raise ValueError("semantic index database cap exceeds portable bound")


class _Budget:
    def __init__(self, db, limits):
        if not db.in_transaction:
            raise ValueError("semantic index requires caller-owned transaction")
        self.db, self.limits = db, limits
        self.rows = self.queries = self.read_bytes = self.input_bytes = self.input_values = 0
        self.output_items = self.output_bytes = 0
        self.initial_writes = db.total_changes
        page_size = self.one("PRAGMA page_size")[0]
        pages = min(self.one("PRAGMA max_page_count")[0], limits.max_bytes // page_size)
        if pages < 1 or self.one("PRAGMA page_count")[0] > pages:
            raise ValueError("semantic index whole database byte budget exceeded")
        self.execute(f"PRAGMA max_page_count={pages}")

    def execute(self, sql, args=()):
        if (sql.startswith(("INSERT ", "UPDATE "))
                and self.db.total_changes - self.initial_writes >= self.limits.max_writes):
            raise ValueError("semantic index operation write budget exceeded before mutation")
        if sql.startswith("DELETE FROM "):
            remaining = self.limits.max_writes - (self.db.total_changes - self.initial_writes)
            # Existing dependency fanout may have been created under a larger
            # budget. Refuse BEFORE executing one broad addressed deletion.
            probe = "SELECT 1 FROM " + sql[len("DELETE FROM "):] + " LIMIT ?"
            count = 0
            for _ in self.rows_from(probe, (*args, remaining + 1)):
                count += 1
            if count > remaining:
                raise ValueError("semantic index operation write budget exceeded before deletion")
        self.queries += 1
        if self.queries > self.limits.max_queries:
            raise ValueError("semantic index operation query budget exceeded")
        cursor = self.db.execute(sql, args)
        if self.db.total_changes - self.initial_writes > self.limits.max_writes:
            raise ValueError("semantic index operation write budget exceeded; rollback required")
        return cursor

    def rows_from(self, sql, args=()):
        # Every query is either a primary/indexed lookup or an explicitly capped
        # bootstrap/report stream. No full row decoding occurs in SQLite.
        for row in self.execute(sql, args):
            self.rows += 1
            if self.rows > self.limits.max_rows:
                raise ValueError("semantic index operation row budget exceeded")
            for value in row:
                if isinstance(value, (str, bytes)):
                    self.read_bytes += len(value.encode("utf-8") if isinstance(value, str) else value)
            if self.read_bytes > self.limits.max_read_bytes:
                raise ValueError("semantic index operation read byte budget exceeded")
            yield row

    def one(self, sql, args=()):
        return next(self.rows_from(sql, args), None)

    def input(self, value):
        # Preflight before JSON encoding or registry indexing; no recursive
        # deep copy and no large container flattening. Limits are operation-wide.
        def visit(item, depth):
            self.input_values += 1
            if self.input_values > self.limits.max_input_values or depth > 64:
                raise ValueError("semantic index input value/depth budget exceeded")
            if isinstance(item, str):
                # JSON escaping is at most six bytes per Unicode codepoint;
                # count that upper bound without allocating a large encoding.
                self.input_bytes += 6 * len(item) + 2
            elif isinstance(item, dict):
                self.input_bytes += 2 + 2 * len(item)
                for key, child in item.items():
                    if not isinstance(key, str):
                        raise ValueError("semantic index requires JSON string keys")
                    visit(key, depth + 1)
                    visit(child, depth + 1)
            elif isinstance(item, list):
                self.input_bytes += 2 + len(item)
                for child in item:
                    visit(child, depth + 1)
            elif item is None or type(item) in (bool, int, float):
                if type(item) is float and not math.isfinite(item):
                    raise ValueError("semantic index requires finite JSON scalars")
                self.input_bytes += len(_compact(item))
            else:
                raise ValueError("semantic index requires JSON values")
            if self.input_bytes > self.limits.max_input_bytes:
                raise ValueError("semantic index input byte budget exceeded")
        visit(value, 0)

    def text(self, table, column, where, args, maximum):
        # Length is checked BEFORE fetching the potentially broad payload.
        size = self.one(f"SELECT length(CAST({column} AS BLOB)) FROM {table} WHERE {where}", args)
        if size is None:
            return None
        if type(size[0]) is not int or size[0] > maximum or self.read_bytes + size[0] > self.limits.max_read_bytes:
            raise ValueError("semantic index stored payload byte budget exceeded")
        return self.one(f"SELECT {column} FROM {table} WHERE {where}", args)[0]

    def diagnostic(self, value):
        self.output_items += 1
        self.output_bytes += len(_compact(value).encode("utf-8"))
        if self.output_items > self.limits.max_output_items or self.output_bytes > self.limits.max_output_bytes:
            raise ValueError("semantic kernel operation output budget exceeded")


class _Diagnostics(list):
    def __init__(self, budget):
        self.budget = budget

    def append(self, value):
        self.budget.diagnostic(value)
        super().append(value)

    def extend(self, values):
        for value in values:
            self.append(value)


def _metadata(b, key):
    rows = list(b.rows_from("SELECT part,length(CAST(json_chunk AS BLOB)) FROM edge_meta WHERE key=? ORDER BY part LIMIT 257", (key,)))
    if not rows or len(rows) > 256 or [r[0] for r in rows] != list(range(len(rows))):
        raise ValueError("semantic index missing exact prepared metadata")
    total = sum(r[1] for r in rows)
    if any(r[1] > 131072 for r in rows) or total > b.limits.max_output_bytes or total + b.read_bytes > b.limits.max_read_bytes:
        raise ValueError("semantic index metadata byte budget exceeded")
    raw = "".join(r[0] for r in b.rows_from("SELECT json_chunk FROM edge_meta WHERE key=? ORDER BY part LIMIT 257", (key,)))
    value = json.loads(raw)
    if _compact(value) != raw:
        raise ValueError("semantic index metadata framing differs")
    return value


def _binding(b, expected):
    b.input(expected)
    top = _metadata(b, TOP_KEY)
    clock = b.one("SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1")
    if (not clock or top.get("read_model_schema") != SCHEMA
            or published_snapshot_binding(top, clock[0]) != expected
            or _metadata(b, "data_revision") != {"sha256": top["data_revision"]}):
        raise ValueError("semantic index stale or foreign prepared binding")
    return published_snapshot_binding(top, clock[0])


def _dependencies(b, binding, entities, relations):
    b.input(entities)
    b.input(relations)
    entities, relations = copy.deepcopy(entities), copy.deepcopy(relations)
    normalization = binding["normalization_binding"]
    processor = k.normalization_processor_digest(Path(k.__file__).resolve())
    if (normalization["processor_digest"] != processor
            or normalization["entity_registry_digest"] != k._stable_digest(entities)
            or normalization["relation_registry_digest"] != k._stable_digest(relations)):
        raise ValueError("semantic index normalization/registry drift requires explicit bootstrap")
    return ({"version": VERSION, "normalization": normalization,
             "projector": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}, entities, relations)


def _stored_row(b, kind, identifier):
    raw = b.text("knowledge_" + kind + "s", "json", "id=?", (identifier,), b.limits.max_row_bytes)
    if raw is None:
        return None
    digest = emitted_row_digest(raw)
    if _metadata(b, published_row_digest_key(kind, identifier)) != digest:
        raise ValueError("semantic index exact prepared row digest differs")
    item = json.loads(raw)
    if not isinstance(item, dict) or item.get("id") != identifier or _compact(item) != raw:
        raise ValueError("semantic index prepared carrier framing differs")
    if k._string(identifier) != identifier:
        raise ValueError("semantic index profile requires canonical untrimmed identifiers")
    return item, digest["sha256"]


def _scope(value):
    # Counter's Python scalar equality includes True == 1 == 1.0 and -0 == 0.
    # Lists/dicts are unhashable in the full wrapper and are refused here too.
    if value is None or isinstance(value, str):
        return _compact(["string" if isinstance(value, str) else "none", value])
    if type(value) in (int, float, bool):
        if isinstance(value, float) and not math.isfinite(value):
            raise ValueError("non-finite semantic cardinality scope")
        return _compact(["number", str(int(value)) if int(value) == value else value.hex()])
    raise ValueError("unhashable semantic cardinality scope")


_DDL = (
    "CREATE TABLE semantic_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),json TEXT NOT NULL)",
    "CREATE TABLE semantic_rows(kind TEXT NOT NULL,id TEXT NOT NULL,source_order INTEGER NOT NULL,digest TEXT NOT NULL,type_id TEXT,entity TEXT,counts TEXT NOT NULL,PRIMARY KEY(kind,id),UNIQUE(kind,source_order))",
    "CREATE INDEX semantic_claim_winner ON semantic_rows(kind,type_id,entity,source_order DESC)",
    "CREATE TABLE semantic_edges(id TEXT PRIMARY KEY,from_id TEXT NOT NULL,to_id TEXT NOT NULL,relation_type TEXT NOT NULL,scope TEXT NOT NULL)",
    "CREATE INDEX semantic_outgoing ON semantic_edges(from_id,relation_type,id)",
    "CREATE TABLE semantic_deps(kind TEXT NOT NULL,id TEXT NOT NULL,dependency_kind TEXT NOT NULL,dependency_id TEXT NOT NULL,PRIMARY KEY(kind,id,dependency_kind,dependency_id)) WITHOUT ROWID",
    "CREATE INDEX semantic_dependents ON semantic_deps(dependency_kind,dependency_id,kind,id)",
    "CREATE TABLE semantic_cardinality(axis TEXT NOT NULL,endpoint TEXT NOT NULL,relation_type TEXT NOT NULL,scope TEXT NOT NULL,n INTEGER NOT NULL,PRIMARY KEY(axis,endpoint,relation_type,scope)) WITHOUT ROWID",
    "CREATE INDEX semantic_cardinality_peak ON semantic_cardinality(axis,endpoint,relation_type,n DESC)",
    "CREATE TABLE semantic_cardinality_errors(axis TEXT NOT NULL,endpoint TEXT NOT NULL,relation_type TEXT NOT NULL,error TEXT NOT NULL,PRIMARY KEY(axis,endpoint,relation_type)) WITHOUT ROWID",
    "CREATE TABLE semantic_diagnostics(kind TEXT NOT NULL,id TEXT NOT NULL,source_order INTEGER NOT NULL,errors TEXT NOT NULL,gaps TEXT NOT NULL,PRIMARY KEY(kind,id)) WITHOUT ROWID",
    "CREATE INDEX semantic_diagnostic_order ON semantic_diagnostics(kind,source_order)",
    "CREATE TABLE semantic_pending(kind TEXT NOT NULL,id TEXT NOT NULL,digest TEXT,source_order INTEGER,PRIMARY KEY(kind,id)) WITHOUT ROWID",
)


class _Lookup:
    def __init__(self, context, kind):
        self.context, self.kind = context, kind

    def get(self, key, default=None):
        c = self.context
        dep = _compact(list(key)) if self.kind == "outgoing" else key
        if not isinstance(dep, str):
            return default
        c.deps.add((self.kind, dep))
        if self.kind == "node":
            value = c.row("node", key)
        elif self.kind == "claim":
            winner = c.b.one("SELECT id FROM semantic_rows INDEXED BY semantic_claim_winner WHERE kind='node' AND type_id='tos.entity.claim' AND entity=? ORDER BY source_order DESC LIMIT 1", (key,))
            value = c.row("node", winner[0]) if winner else None
        else:
            value = [{"to_id": row[0]} for row in c.b.rows_from(
                "SELECT to_id FROM semantic_edges INDEXED BY semantic_outgoing WHERE from_id=? AND relation_type=? ORDER BY id LIMIT ?",
                (*key, c.b.limits.max_rows + 1))]
        return default if value is None else value

    def __contains__(self, key):
        return self.get(key) is not None

    def __getitem__(self, key):
        value = self.get(key)
        if value is None:
            raise KeyError(key)
        return value


class _Context:
    def __init__(self, b, entities, relations, overlay=None):
        self.b, self.overlay = b, overlay or {}
        self.entities, _, self.fallback_node = k._entity_registry_indexes(entities)
        self.relations, _, self.fallback_relation = k._relation_registry_indexes(relations)
        self.deps = set()
        self.checked_rows = {}
        self.kernels = k._semantic_validation_kernels(entities, relations, _Lookup(self, "node"),
                                                     _Lookup(self, "claim"), _Lookup(self, "outgoing"),
                                                     diagnostic_list=lambda: _Diagnostics(b))

    def row(self, kind, identifier):
        key = (kind, identifier)
        if key in self.overlay:
            return self.overlay[key]
        stored = _stored_row(self.b, kind, identifier)
        indexed = self.b.one("SELECT digest,source_order FROM semantic_rows WHERE kind=? AND id=?", key)
        prepared = self.b.one("SELECT source_order FROM prepared_documents WHERE kind=? AND id=?", key)
        if stored is None:
            if indexed is not None or prepared is not None:
                raise ValueError("semantic index carrier disappeared")
            self.checked_rows[key] = (None, None)
            return None
        if indexed is None or stored[1] != indexed[0] or prepared is None or prepared[0] != indexed[1]:
            raise ValueError("semantic index current row dependency drift")
        self.checked_rows[key] = (stored[1], indexed[1])
        return stored[0]

    def summarize(self, kind, item):
        if kind == "node":
            return [int(k._string(item.get("type_id")) in self.entities),
                    int(item.get("type_id") == self.fallback_node), 0, 0, 0, 0]
        return [0, 0, int(k._string(item.get("relation_type_id")) in self.relations),
                int(item.get("relation_type_id") == self.fallback_relation), 0,
                int(item.get("source_graph") == "semantic-interchange")]

    def add(self, kind, item, order, digest):
        identifier = item["id"]
        if type(order) is not int or not 0 <= order <= MAX_ORDER:
            raise ValueError("semantic index requires an explicit valid source order")
        counts = self.summarize(kind, item)
        self.b.execute("INSERT INTO semantic_rows VALUES (?,?,?,?,?,?,?)",
                       (kind, identifier, order, digest, item.get("type_id") if kind == "node" else None,
                        str(item.get("entity_id")) if kind == "node" else None, _compact(counts)))
        if kind == "relation":
            relation_type = k._string(item.get("relation_type_id"))
            if relation_type in self.relations:
                scope = _scope((item.get("attributes") or {}).get("claim_ref")
                               if self.relations[relation_type].get("assertion_mode") == "reified-claim" else None)
                self.b.execute("INSERT INTO semantic_edges VALUES (?,?,?,?,?)",
                               (identifier, str(item.get("from_id")), str(item.get("to_id")), relation_type, scope))
                self.adjust_edges(identifier, 1)
        return counts

    def adjust_edges(self, identifier, adjustment):
        edge = self.b.one("SELECT from_id,to_id,relation_type,scope FROM semantic_edges WHERE id=?", (identifier,))
        if edge is None:
            return
        for axis, endpoint in zip(("per_subject_max", "per_object_max"), edge[:2]):
            key = (axis, endpoint, edge[2], edge[3])
            found = self.b.one("SELECT n FROM semantic_cardinality WHERE axis=? AND endpoint=? AND relation_type=? AND scope=?", key)
            n = (found[0] if found else 0) + adjustment
            if n < 0:
                raise ValueError("semantic cardinality index drift")
            if n:
                self.b.execute("INSERT INTO semantic_cardinality VALUES (?,?,?,?,?) ON CONFLICT(axis,endpoint,relation_type,scope) DO UPDATE SET n=excluded.n", (*key, n))
            else:
                self.b.execute("DELETE FROM semantic_cardinality WHERE axis=? AND endpoint=? AND relation_type=? AND scope=?", key)
            group = key[:3]
            peak = self.b.one("SELECT n FROM semantic_cardinality INDEXED BY semantic_cardinality_peak WHERE axis=? AND endpoint=? AND relation_type=? ORDER BY n DESC LIMIT 1", group)
            error = k._semantic_cardinality_violation(endpoint, edge[2], axis, peak[0] if peak else 0, self.relations)
            if error is None:
                self.b.execute("DELETE FROM semantic_cardinality_errors WHERE axis=? AND endpoint=? AND relation_type=?", group)
            else:
                self.b.execute("INSERT INTO semantic_cardinality_errors VALUES (?,?,?,?) ON CONFLICT(axis,endpoint,relation_type) DO UPDATE SET error=excluded.error", (*group, error))

    def evaluate(self, kind, identifier):
        item = self.row(kind, identifier)
        if item is None:
            return [0] * len(COUNT_KEYS)
        self.deps = set()
        counts = self.summarize(kind, item)
        if kind == "node":
            errors, gaps = self.kernels[0](item), []
            if item.get("type_id") == "tos.entity.claim":
                more, gaps, counts[4] = self.kernels[2](item)
                errors.extend(more)
        else:
            errors, gaps = self.kernels[1](item)
            if (identifier == "<missing relation id>"
                    and k._string(item.get("relation_type_id")) in self.relations):
                errors.append("duplicate or missing relation id <missing relation id>")
        self.b.execute("DELETE FROM semantic_deps WHERE kind=? AND id=?", (kind, identifier))
        for dep in sorted(self.deps):
            self.b.execute("INSERT INTO semantic_deps VALUES (?,?,?,?)", (kind, identifier, *dep))
        self.b.execute("DELETE FROM semantic_diagnostics WHERE kind=? AND id=?", (kind, identifier))
        if errors or gaps:
            raw_errors, raw_gaps = _compact(errors), _compact(gaps)
            if len(raw_errors.encode()) + len(raw_gaps.encode()) > self.b.limits.max_output_bytes:
                raise ValueError("semantic diagnostic output byte budget exceeded")
            order = self.b.one("SELECT source_order FROM semantic_rows WHERE kind=? AND id=?", (kind, identifier))[0]
            self.b.execute("INSERT INTO semantic_diagnostics VALUES (?,?,?,?,?)", (kind, identifier, order, raw_errors, raw_gaps))
        self.b.execute("UPDATE semantic_rows SET counts=? WHERE kind=? AND id=?", (_compact(counts), kind, identifier))
        return counts


def _report(b, counts, registry_violations):
    violations, gaps = list(registry_violations), []
    output_bytes = len(_compact(violations).encode())
    items = len(violations)
    if items > b.limits.max_output_items or output_bytes > b.limits.max_output_bytes:
        raise ValueError("semantic report registry output budget exceeded")
    # Full wrapper emits relation gaps first, then Claim gaps in node order.
    for kind in ("relation", "node"):
        rows = b.rows_from("SELECT id,length(CAST(errors AS BLOB)),length(CAST(gaps AS BLOB)) FROM semantic_diagnostics INDEXED BY semantic_diagnostic_order WHERE kind=? ORDER BY source_order LIMIT ?", (kind, b.limits.max_output_items + 1))
        for identifier, errors_size, gaps_size in rows:
            output_bytes += errors_size + gaps_size
            if output_bytes > b.limits.max_output_bytes:
                raise ValueError("semantic report operation output byte budget exceeded")
            errors, missing = b.one("SELECT errors,gaps FROM semantic_diagnostics WHERE kind=? AND id=?", (kind, identifier))
            errors, missing = json.loads(errors), json.loads(missing)
            items += len(errors) + len(missing)
            if items > b.limits.max_output_items:
                raise ValueError("semantic report operation output item budget exceeded")
            violations.extend(errors)
            gaps.extend(missing)
    for (error,) in b.rows_from("SELECT error FROM semantic_cardinality_errors LIMIT ?", (b.limits.max_output_items + 1,)):
        violations.append(error)
        items += 1
        output_bytes += len(error.encode())
        if items > b.limits.max_output_items or output_bytes > b.limits.max_output_bytes:
            raise ValueError("semantic report operation output budget exceeded")
    report = {"valid": not violations, "violations": sorted(set(violations)),
              **dict(zip(COUNT_KEYS, counts)), "gaps": gaps}
    if len(_compact(report).encode()) > b.limits.max_output_bytes:
        raise ValueError("semantic report operation output byte budget exceeded")
    return report


def _state(b):
    raw = b.text("semantic_state", "json", "singleton=1", (), b.limits.max_output_bytes)
    if raw is None:
        raise ValueError("semantic index is absent; explicit bootstrap required")
    return json.loads(raw)


def _put_state(b, state):
    raw = _compact(state)
    if len(raw.encode()) > b.limits.max_output_bytes:
        raise ValueError("semantic index state byte budget exceeded")
    b.execute("INSERT INTO semantic_state VALUES (1,?) ON CONFLICT(singleton) DO UPDATE SET json=excluded.json", (raw,))


def bootstrap_semantic_index_transaction(db, *, binding, entity_registry, relation_registry,
                                         ordered_rows=None, limits=None):
    """Compute (never inherit) a complete report, using existing prepared bytes.

    Optional ordered_rows(kind) is a repeatable owner row stream. Each supplied
    item must match the exact existing row digest AND increasing source_order.
    With no stream, the bounded bootstrap scans the existing prepared map.
    Existing semantic tables are refused, never silently replaced.
    """
    b = _Budget(db, limits or SemanticIndexLimits())
    binding = _binding(b, binding)
    dependencies, entity_registry, relation_registry = _dependencies(b, binding, entity_registry, relation_registry)
    registry = k.validate_semantic_registries(entity_registry, relation_registry)
    for sql in _DDL:
        b.execute(sql)
    c = _Context(b, entity_registry, relation_registry)
    for kind in KINDS:
        last = -1
        if ordered_rows is None:
            # The full bootstrap is the only non-addressed carrier route. Its
            # bounded scan also gives SQLite a capped input to the sort.
            ids = list(b.rows_from("SELECT id,source_order FROM prepared_documents WHERE kind=? ORDER BY id LIMIT ?", (kind, b.limits.max_rows + 1)))
            ids.sort(key=lambda row: row[1])
            stream = ((identifier, order, None) for identifier, order in ids)
        else:
            def supplied():
                for item in ordered_rows(kind):
                    b.input(item)
                    if not isinstance(item, dict) or not isinstance(item.get("id"), str):
                        raise ValueError("semantic bootstrap requires exact supplied carrier")
                    found = b.one("SELECT source_order FROM prepared_documents WHERE kind=? AND id=?", (kind, item["id"]))
                    if found is None:
                        raise ValueError("semantic bootstrap supplied foreign carrier")
                    yield item["id"], found[0], item
            stream = supplied()
        for identifier, order, supplied_item in stream:
            if type(order) is not int or order <= last:
                raise ValueError("semantic bootstrap source order must be unique and increasing")
            last = order
            stored = _stored_row(b, kind, identifier)
            if stored is None or (supplied_item is not None and emitted_row_digest(_compact(supplied_item))["sha256"] != stored[1]):
                raise ValueError("semantic bootstrap supplied row differs from exact prepared digest")
            c.add(kind, stored[0], order, stored[1])
        # A bounded indexed anti-lookup catches omitted supplied rows, including
        # a missing suffix; no caller-supplied valid bit authorizes completeness.
        for (identifier,) in b.rows_from("SELECT id FROM prepared_documents WHERE kind=? ORDER BY id LIMIT ?", (kind, b.limits.max_rows + 1)):
            if b.one("SELECT 1 FROM semantic_rows WHERE kind=? AND id=?", (kind, identifier)) is None:
                raise ValueError("semantic bootstrap omitted prepared carrier")
        for (identifier,) in b.rows_from(f"SELECT id FROM knowledge_{kind}s ORDER BY id LIMIT ?", (b.limits.max_rows + 1,)):
            if b.one("SELECT 1 FROM semantic_rows WHERE kind=? AND id=?", (kind, identifier)) is None:
                raise ValueError("semantic bootstrap carrier is absent from prepared map")
    counts = [0] * len(COUNT_KEYS)
    for kind in KINDS:
        for (identifier,) in b.rows_from("SELECT id FROM semantic_rows WHERE kind=? ORDER BY id LIMIT ?", (kind, b.limits.max_rows + 1)):
            counts = [a + v for a, v in zip(counts, c.evaluate(kind, identifier))]
    report = _report(b, counts, registry["violations"])
    report_digest = emitted_row_digest(_compact(report))["sha256"]
    _verify_descriptor(b, binding, report_digest)
    _put_state(b, {"dependencies": dependencies, "binding": binding, "pending": False,
                   "counts": counts, "registry_violations": registry["violations"],
                   "report_digest": report_digest})
    return report


def apply_semantic_delta_transaction(db, *, expected_binding, new_source_revision,
                                     changes, entity_registry, relation_registry, limits=None):
    """Validate a bounded candidate overlay BEFORE the prepared row delta.

    Returns the full-wrapper report; updates only auxiliary semantic tables.
    Owner must publish that report in header.counts.semantic_validation, apply
    the SAME PreparedChange sequence, then call verify below in this transaction.
    """
    b = _Budget(db, limits or SemanticIndexLimits())
    expected_binding = _binding(b, expected_binding)
    state = _state(b)
    dependencies, entity_registry, relation_registry = _dependencies(b, expected_binding, entity_registry, relation_registry)
    if state["pending"] or state["binding"] != expected_binding or state["dependencies"] != dependencies:
        raise ValueError("semantic index pending/stale dependency state requires rollback or bootstrap")
    if (not isinstance(new_source_revision, str) or len(new_source_revision) != 64
            or any(ch not in "0123456789abcdef" for ch in new_source_revision)
            or expected_binding["publication_epoch"] >= MAX_ORDER):
        raise ValueError("semantic delta requires explicit source revision and available next epoch")
    c = _Context(b, entity_registry, relation_registry)
    pending, affected, signals, frames = [], set(), set(), []
    seen = set()
    for change in changes:
        if len(seen) >= b.limits.max_changes:
            raise ValueError("semantic delta change count budget exceeded")
        if (not isinstance(change, PreparedChange) or change.kind not in KINDS
                or change.operation not in {"insert", "update", "delete"}
                or not isinstance(change.identifier, str) or not 1 <= len(change.identifier) <= 4096):
            raise ValueError("semantic delta requires exact PreparedChange targets")
        if k._string(change.identifier) != change.identifier:
            raise ValueError("semantic index profile requires canonical untrimmed identifiers")
        key = (change.kind, change.identifier)
        if key in seen:
            raise ValueError("semantic delta duplicate target")
        seen.add(key)
        found = b.one("SELECT source_order FROM prepared_documents WHERE kind=? AND id=?", key)
        if (found is None) != (change.operation == "insert"):
            raise ValueError("semantic delta operation differs from exact prepared target")
        old = c.row(*key)
        if (old is None) != (change.operation == "insert"):
            raise ValueError("semantic delta prepared map/carrier differs")
        if change.operation == "delete":
            if change.item is not None or change.source_order is not None:
                raise ValueError("semantic deletion cannot supply replacement/order")
            item, order, digest = None, None, None
        else:
            b.input(change.item)
            if not isinstance(change.item, dict) or change.item.get("id") != change.identifier:
                raise ValueError("semantic replacement identity differs")
            raw = _compact(change.item)
            if len(raw.encode()) > b.limits.max_row_bytes:
                raise ValueError("semantic replacement row byte budget exceeded")
            # Keep a private exact JSON replacement, not a caller-mutable dict.
            item, digest = json.loads(raw), emitted_row_digest(raw)["sha256"]
            order = found[0] if found and change.source_order is None else change.source_order
            if type(order) is not int or not 0 <= order <= MAX_ORDER:
                raise ValueError("semantic insertion/reorder requires explicit valid token")
        pending.append((change.kind, change.identifier, item, order, digest))
        frames.append([change.operation, change.kind, change.identifier,
                       found[0] if change.operation == "delete" else order, digest])
        affected.add(key)
        if change.kind == "node":
            signals.add(("node", change.identifier))
            for row in (old, item):
                if row is not None and row.get("type_id") == "tos.entity.claim":
                    signals.add(("claim", str(row.get("entity_id"))))
        else:
            for row in (old, item):
                if row is not None:
                    signals.add(("outgoing", _compact([str(row.get("from_id")), k._string(row.get("relation_type_id"))])))
    for signal in sorted(signals):
        for kind, identifier in b.rows_from("SELECT kind,id FROM semantic_deps INDEXED BY semantic_dependents WHERE dependency_kind=? AND dependency_id=? ORDER BY kind,id LIMIT ?", (*signal, b.limits.max_rows + 1)):
            affected.add((kind, identifier))
            if len(affected) > b.limits.max_rows:
                raise ValueError("semantic delta affected closure budget exceeded")
    counts = list(state["counts"])
    # Remove every changed ordering row first so a valid simultaneous swap is
    # not rejected by an intermediate UNIQUE constraint.
    for key in sorted(affected):
        found = b.one("SELECT counts FROM semantic_rows WHERE kind=? AND id=?", key)
        if found:
            counts = [a - v for a, v in zip(counts, json.loads(found[0]))]
    for kind, identifier, item, order, digest in pending:
        if kind == "relation":
            c.adjust_edges(identifier, -1)
            b.execute("DELETE FROM semantic_edges WHERE id=?", (identifier,))
        b.execute("DELETE FROM semantic_rows WHERE kind=? AND id=?", (kind, identifier))
        b.execute("DELETE FROM semantic_deps WHERE kind=? AND id=?", (kind, identifier))
        b.execute("DELETE FROM semantic_diagnostics WHERE kind=? AND id=?", (kind, identifier))
        c.overlay[(kind, identifier)] = item
    for kind, identifier, item, order, digest in pending:
        if item is not None:
            c.add(kind, item, order, digest)
    for key in sorted(affected):
        counts = [a + v for a, v in zip(counts, c.evaluate(*key))]
    report = _report(b, counts, state["registry_violations"])
    # Persist exact loaded dependencies and changed candidates for final check.
    checks = dict(c.checked_rows)
    checks.update({(kind, identifier): (digest, order) for kind, identifier, _, order, digest in pending})
    for (kind, identifier), (digest, order) in sorted(checks.items()):
        b.execute("INSERT INTO semantic_pending VALUES (?,?,?,?)", (kind, identifier, digest, order))
    state.update(pending=True, counts=counts, next_source_revision=new_source_revision,
                 next_epoch=expected_binding["publication_epoch"] + 1,
                 changes_digest=emitted_row_digest(_compact(frames))["sha256"],
                 report_digest=emitted_row_digest(_compact(report))["sha256"])
    _put_state(b, state)
    return report


def _verify_descriptor(b, binding, report_digest):
    raw = b.text("prepared_state", "descriptor", "singleton=1", (), b.limits.max_input_bytes)
    if raw is None or emitted_row_digest(raw)["sha256"] != binding["data_revision"]:
        raise ValueError("semantic final prepared descriptor digest differs")
    descriptor = json.loads(raw)
    header = descriptor.get("header", {})
    report = header.get("counts", {}).get("semantic_validation")
    if (header.get("source_revision") != binding["source_revision"]
            or header.get("normalization_binding") != binding["normalization_binding"]
            or emitted_row_digest(_compact(report))["sha256"] != report_digest):
        raise ValueError("semantic computed report differs from final source header")
    return descriptor


def verify_semantic_index_binding_transaction(db, new_binding, *, limits=None):
    """Finalize only a coherent caller-published binding; never commit/select it.

    Checks exact pending rows/order, normalization, source revision, epoch, and
    the computed report in the prepared descriptor's source header. Caller owns
    the search/catalog lanes and all commit/rollback/publication decisions.
    """
    b = _Budget(db, limits or SemanticIndexLimits())
    new_binding = _binding(b, new_binding)
    state = _state(b)
    expected = state["binding"]
    if (state["dependencies"]["projector"] != hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
            or state["dependencies"]["normalization"]["processor_digest"]
            != k.normalization_processor_digest(Path(k.__file__).resolve())):
        raise ValueError("semantic projector changed; explicit bootstrap required")
    if state["pending"]:
        if (new_binding["publication_epoch"] != state["next_epoch"]
                or new_binding["source_revision"] != state["next_source_revision"]
                or new_binding["normalization_binding"] != expected["normalization_binding"]):
            raise ValueError("semantic pending transition final binding differs")
    elif new_binding != expected:
        raise ValueError("semantic index final binding differs")
    for kind, identifier, digest, order in b.rows_from("SELECT kind,id,digest,source_order FROM semantic_pending ORDER BY kind,id LIMIT ?", (b.limits.max_rows + 1,)):
        stored = _stored_row(b, kind, identifier)
        found = b.one("SELECT source_order FROM prepared_documents WHERE kind=? AND id=?", (kind, identifier))
        if ((stored[1] if stored else None) != digest
                or (found[0] if found else None) != order):
            raise ValueError("semantic pending final carrier/order dependency differs")
    descriptor = _verify_descriptor(b, new_binding, state["report_digest"])
    if state["pending"]:
        frames = descriptor.get("changes")
        if (descriptor.get("mode") != "delta-history"
                or descriptor.get("parent_data_revision") != expected["data_revision"]
                or not isinstance(frames, list)
                or any(not isinstance(frame, list) or len(frame) != 6 for frame in frames)
                or emitted_row_digest(_compact([[f[0], f[1], f[2], f[4], f[5]] for f in frames]))["sha256"] != state["changes_digest"]):
            raise ValueError("semantic final prepared change set differs from validated overlay")
    b.execute("DELETE FROM semantic_pending")
    state.update(binding=new_binding, pending=False)
    state.pop("next_source_revision", None)
    state.pop("next_epoch", None)
    state.pop("changes_digest", None)
    _put_state(b, state)
    return {"binding": new_binding, "semantic_report_sha256": state["report_digest"]}

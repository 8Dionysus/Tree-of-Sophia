"""Explicit offline publication of an already normalized, owner-selected snapshot.

No source assembly, normalization, semantic admission, consumer switching or
request-time building occurs here. See LOCAL_PREPARED_PUBLICATION.md.
"""
from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat
from dataclasses import dataclass
from typing import Any, Iterable

from .compressed_search_store import ALGORITHM, MAX_ADDRESS, PreparedSearchDocument, SearchChange, SearchStore
from .published_read_metadata import (
    CATALOG_KEY, LENS_META_KEY, TOP_KEY, _compact, emitted_row_digest,
    lens_order_row, published_lens_metadata, published_reader_metadata,
    published_row_digest_key, published_snapshot_binding, validate_lens_metadata,
)

SCHEMA = "tos_local_prepared_read_model_v1"
DESCRIPTOR_SCHEMA = "tos_local_prepared_revision_v1"
SOURCE_ORDER_STRIDE = 2**32
CAPABILITIES = {"full_rows": True, "catalog": True, "lens": True,
                "compressed_search_v3": True, "legacy_search": False,
                "search_v2": False, "edge_runtime": False}
_COLUMNS = {
    "node": ("id", "entity_id", "native_id", "source_graph", "kind_id", "type_id"),
    "relation": ("id", "native_id", "source_graph", "from_id", "to_id", "predicate_id", "relation_type_id"),
}
_DIMENSIONS = {"node": ("source_graph", "kind_id", "type_id"),
               "relation": ("source_graph", "predicate_id", "relation_type_id")}


@dataclass(frozen=True)
class PublicationLimits:
    max_bytes: int = 64 * 1024 * 1024
    max_mutations: int = 2_000_000
    max_row_bytes: int = 1_048_576
    max_metadata_bytes: int = 8_388_608
    max_changes: int = 4096
    max_change_bytes: int = 16_777_216

    def __post_init__(self):
        if any(type(v) is not int or v < 1 for v in vars(self).values()):
            raise ValueError("publication limits must be positive integers")


@dataclass(frozen=True)
class PreparedChange:
    operation: str
    kind: str
    identifier: str
    item: dict[str, Any] | None = None
    source_order: int | None = None


def _hash(value):
    return emitted_row_digest(_compact(value))["sha256"]


def _header(graph, catalog):
    if not isinstance(graph, dict) or not isinstance(catalog, dict):
        raise ValueError("explicit normalized graph/header and catalog required")
    # Validate the complete coherence/normalization boundary before file creation.
    published_reader_metadata(graph, catalog, SCHEMA, "0" * 64,
                              lens_metadata=published_lens_metadata({**graph, "nodes": [], "relations": []}))
    if "normalization_binding" in catalog and catalog["normalization_binding"] != graph["normalization_binding"]:
        raise ValueError("catalog normalization binding differs from graph")
    return {key: copy.deepcopy(value) for key, value in graph.items() if key not in ("nodes", "relations")}


def _row(kind, item, limits):
    if kind not in _COLUMNS or not isinstance(item, dict):
        raise ValueError("exact normalized node/relation required")
    if not isinstance(item.get("id"), str) or not 1 <= len(item["id"]) <= 4096:
        raise ValueError("exact string identifier of 1..4096 characters required")
    raw = _compact(item)
    raw_bytes = len(raw.encode("utf-8"))
    if raw_bytes > limits.max_row_bytes:
        raise ValueError("publication row byte budget exceeded")
    # Reader SQLITE_LIMIT_LENGTH also covers the complete record, including
    # duplicated index columns. Keep conservative record-header headroom.
    indexed_bytes = sum(len(str(item.get(key) or "").encode("utf-8")) for key in _COLUMNS[kind])
    if raw_bytes + indexed_bytes + 1024 > max(limits.max_row_bytes, 131072) + 4096:
        raise ValueError("publication carrier exceeds compatible reader record budget")
    return raw


def _metadata(db, key):
    chunks = db.execute("SELECT part,json_chunk FROM edge_meta WHERE key=? ORDER BY part LIMIT 257", (key,)).fetchall()
    if not chunks or len(chunks) > 256 or [row[0] for row in chunks] != list(range(len(chunks))):
        raise ValueError("missing or incomplete publication metadata")
    if any(not isinstance(row[1], str) or len(row[1].encode("utf-8")) > 131072 for row in chunks):
        raise ValueError("invalid publication metadata chunk")
    raw = "".join(row[1] for row in chunks)
    value = json.loads(raw)
    if _compact(value) != raw:
        raise ValueError("publication metadata emitted framing differs")
    return value


def _put_metadata(db, key, value, limits):
    raw = _compact(value)
    maximum = min(limits.max_metadata_bytes, 65_536) if key == TOP_KEY else limits.max_metadata_bytes
    if len(raw.encode("utf-8")) > maximum:
        raise ValueError("publication metadata byte budget exceeded")
    db.execute("DELETE FROM edge_meta WHERE key=?", (key,))
    # 32K codepoints remain below the reader's 128K UTF-8 chunk bound.
    chunks = [raw[i:i + 32768] for i in range(0, len(raw), 32768)]
    if len(chunks) > 256:
        raise ValueError("publication metadata chunk budget exceeded")
    db.executemany("INSERT INTO edge_meta VALUES (?,?,?)", ((key, i, chunk) for i, chunk in enumerate(chunks)))


def _ddl(db):
    statements = [
        "CREATE TABLE edge_meta(key TEXT NOT NULL,part INTEGER NOT NULL,json_chunk TEXT NOT NULL,PRIMARY KEY(key,part))",
        "CREATE TABLE knowledge_exploration_clock(singleton INTEGER PRIMARY KEY CHECK(singleton=1),epoch INTEGER NOT NULL)",
        "CREATE TABLE knowledge_lens_order(kind TEXT NOT NULL,id TEXT NOT NULL,sort_key TEXT NOT NULL,from_id TEXT NOT NULL,to_id TEXT NOT NULL,PRIMARY KEY(kind,id))",
        "CREATE TABLE prepared_documents(kind TEXT NOT NULL,id TEXT NOT NULL,doc_id INTEGER NOT NULL UNIQUE,source_order INTEGER NOT NULL,PRIMARY KEY(kind,id))",
        "CREATE TABLE prepared_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),high_water INTEGER NOT NULL,max_pages INTEGER NOT NULL,descriptor TEXT NOT NULL)",
    ]
    for kind, columns in _COLUMNS.items():
        statements.append(f"CREATE TABLE knowledge_{kind}s(" + ",".join(
            f"{column} TEXT " + ("PRIMARY KEY" if column == "id" else "NOT NULL") for column in columns) + ",json TEXT NOT NULL)")
    indices = {
        "knowledge_nodes_native_idx": "knowledge_nodes(native_id)",
        "knowledge_nodes_entity_idx": "knowledge_nodes(entity_id)",
        "knowledge_nodes_identity_seek": "knowledge_nodes(entity_id,id)",
        "knowledge_relations_native_idx": "knowledge_relations(native_id)",
        "knowledge_relations_from_seek": "knowledge_relations(from_id,id)",
        "knowledge_relations_to_seek": "knowledge_relations(to_id,id)",
        "knowledge_lens_order_sort": "knowledge_lens_order(kind,sort_key,id)",
        "knowledge_lens_order_from": "knowledge_lens_order(kind,from_id,sort_key,id)",
        "knowledge_lens_order_to": "knowledge_lens_order(kind,to_id,sort_key,id)",
        "knowledge_lens_order_pair": "knowledge_lens_order(kind,from_id,to_id,id)",
    }
    statements.extend(f"CREATE INDEX {name} ON {body}" for name, body in indices.items())
    for sql in statements:
        db.execute(sql)


def _put_row(db, kind, item, raw, limits):
    columns = _COLUMNS[kind]
    db.execute(f"INSERT OR REPLACE INTO knowledge_{kind}s VALUES (" + ",".join("?" for _ in range(len(columns) + 1)) + ")",
               (*(str(item.get(key) or "") for key in columns), raw))
    db.execute("INSERT OR REPLACE INTO knowledge_lens_order VALUES (?,?,?,?,?)", lens_order_row(kind, item))
    _put_metadata(db, published_row_digest_key(kind, item["id"]), emitted_row_digest(raw), limits)


def _endpoint(db, identifier):
    if not isinstance(identifier, str) or not db.execute("SELECT 1 FROM knowledge_nodes WHERE id=?", (identifier,)).fetchone():
        raise ValueError("relation endpoint is missing from this publication")


def _cap(db, limits, retained=None):
    page_size = db.execute("PRAGMA page_size").fetchone()[0]
    maximum = min(limits.max_bytes // page_size, db.execute("PRAGMA max_page_count").fetchone()[0])
    if retained is not None:
        maximum = min(maximum, retained)
    if maximum < 1 or db.execute("PRAGMA page_count").fetchone()[0] > maximum:
        raise ValueError("whole publication exceeds owner byte cap")
    db.execute(f"PRAGMA max_page_count={maximum}")
    return maximum


def _publish_header(db, header, catalog, lens, descriptor, epoch, limits):
    revision = _hash(descriptor)
    metadata = published_reader_metadata(header, catalog, SCHEMA, revision, lens_metadata=lens)
    for key, value in metadata.items():
        _put_metadata(db, key, value, limits)
    _put_metadata(db, "data_revision", {"sha256": revision}, limits)
    db.execute("INSERT OR REPLACE INTO knowledge_exploration_clock VALUES (1,?)", (epoch,))
    return published_snapshot_binding(metadata[TOP_KEY], epoch)


def publish_prepared(path: str | Path, *, graph: dict, catalog: dict,
                     limits: PublicationLimits | None = None) -> dict:
    """Create one exclusive 0600 file; failure removes only that new inode.

    Explicit repeatable normalized row lists retain their native source order.
    Rows are streamed into SQLite once; the descriptor pass retains no row copy.
    """
    limits = limits or PublicationLimits()
    header = _header(graph, catalog)
    digest = hashlib.sha256()
    count = 0
    for kind in _COLUMNS:
        if not isinstance(graph.get(kind + "s"), list):
            raise ValueError("explicit repeatable normalized row lists required")
        for position, item in enumerate(graph[kind + "s"]):
            raw = _row(kind, item, limits)
            count += 1
            token = position * SOURCE_ORDER_STRIDE
            if token > MAX_ADDRESS:
                raise ValueError("source order capacity exhausted")
            digest.update((_compact([kind, item["id"], count, token, emitted_row_digest(raw)["sha256"]]) + "\n").encode("utf-8"))
    descriptor = {"schema": DESCRIPTOR_SCHEMA, "mode": "bootstrap", "profile": SCHEMA,
                  "algorithm": ALGORITHM, "capabilities": CAPABILITIES,
                  "header": header, "catalog_sha256": _hash(catalog), "rows_sha256": digest.hexdigest()}
    lens = published_lens_metadata(graph)
    path = Path(path).absolute()
    fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    created = os.fstat(fd)
    os.close(fd)
    db = None
    try:
        db = sqlite3.connect(path, isolation_level=None)
        db.execute("PRAGMA journal_mode=DELETE")
        maximum = _cap(db, limits)
        db.execute("BEGIN IMMEDIATE")
        _ddl(db)
        binding = _publish_header(db, header, catalog, lens, descriptor, 1, limits)

        def documents():
            address = 0
            actual = hashlib.sha256()
            for kind in _COLUMNS:
                for position, item in enumerate(graph[kind + "s"]):
                    address += 1
                    token = position * SOURCE_ORDER_STRIDE
                    raw = _row(kind, item, limits)
                    actual.update((_compact([kind, item["id"], address, token, emitted_row_digest(raw)["sha256"]]) + "\n").encode("utf-8"))
                    _put_row(db, kind, item, raw, limits)
                    db.execute("INSERT INTO prepared_documents VALUES (?,?,?,?)", (kind, item["id"], address, token))
                    if kind == "relation":
                        _endpoint(db, item.get("from_id"))
                        _endpoint(db, item.get("to_id"))
                    yield PreparedSearchDocument.from_item(address, kind, item, token)
            if actual.hexdigest() != digest.hexdigest():
                raise ValueError("normalized input changed during publication")

        SearchStore.initialize_transaction(db, binding=binding, documents=documents(),
                                           max_mutations=limits.max_mutations, max_bytes=maximum * db.execute("PRAGMA page_size").fetchone()[0])
        if db.execute("SELECT high_water FROM search_header WHERE singleton=1").fetchone()[0] != count:
            raise ValueError("prepared/search address high-water differs")
        db.execute("INSERT INTO prepared_state VALUES (1,?,?,?)", (count, maximum, _compact(descriptor)))
        _cap(db, limits, maximum)
        if db.total_changes > limits.max_mutations:
            raise ValueError("whole publication mutation budget exceeded")
        db.execute("COMMIT")
        return binding
    except BaseException:
        if db is not None and db.in_transaction:
            db.execute("ROLLBACK")
        try:
            current = path.lstat()
            if (current.st_dev, current.st_ino) == (created.st_dev, created.st_ino):
                path.unlink()
        except FileNotFoundError:
            pass
        raise
    finally:
        if db is not None:
            db.close()


def apply_prepared_delta_transaction(db: sqlite3.Connection, *, expected_binding: dict,
                                     source_header: dict, catalog: dict,
                                     changes: Iterable[PreparedChange],
                                     limits: PublicationLimits | None = None) -> dict:
    """Apply addressed storage changes in the caller's transaction, without commit.

    On any exception caller MUST roll back the complete transaction. Caller owns
    semantic validation and the dependency-complete normalized change set.
    """
    if not db.in_transaction:
        raise ValueError("caller-owned transaction required")
    limits = limits or PublicationLimits()
    header = _header(source_header, catalog)
    if "nodes" in source_header or "relations" in source_header:
        raise ValueError("delta requires a header, not a full graph")
    top = _metadata(db, TOP_KEY)
    epoch = db.execute("SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1").fetchone()[0]
    if top.get("read_model_schema") != SCHEMA or published_snapshot_binding(top, epoch) != expected_binding:
        raise ValueError("stale or foreign prepared publication binding")
    if _metadata(db, "data_revision") != {"sha256": top["data_revision"]}:
        raise ValueError("publication data revision differs from header")
    if header["normalization_binding"] != top["normalization_binding"]:
        raise ValueError("normalization drift requires explicit bootstrap")
    if epoch >= MAX_ADDRESS:
        raise ValueError("publication epoch exhausted")
    high_water, retained, _ = db.execute("SELECT high_water,max_pages,descriptor FROM prepared_state WHERE singleton=1").fetchone()
    if db.execute("SELECT high_water FROM search_header WHERE singleton=1").fetchone()[0] != high_water:
        raise ValueError("prepared/search address high-water differs")
    maximum = _cap(db, limits, retained)
    before = db.total_changes
    lens = _metadata(db, LENS_META_KEY)
    validate_lens_metadata(lens, top["source_revision"])
    if _hash(lens) != top["lens_sha256"]:
        raise ValueError("publication lens digest differs from header")
    histograms = {kind: {tuple(cell[:3]): cell[3] for cell in lens[kind + "_counts"]} for kind in _COLUMNS}
    search_changes, frames, seen, deleted_nodes, relations = [], [], set(), [], []
    changed_bytes = 0
    for change in changes:
        if len(seen) >= limits.max_changes:
            raise ValueError("delta change count budget exceeded")
        if not isinstance(change, PreparedChange) or change.kind not in _COLUMNS or change.operation not in {"insert", "update", "delete"}:
            raise ValueError("explicit addressed insert/update/delete required")
        key = (change.kind, change.identifier)
        if not isinstance(change.identifier, str) or not change.identifier or key in seen:
            raise ValueError("duplicate or invalid delta target")
        seen.add(key)
        found = db.execute("SELECT doc_id,source_order FROM prepared_documents WHERE kind=? AND id=?", key).fetchone()
        if (found is None) != (change.operation == "insert"):
            raise ValueError("delta operation does not match existing exact target")
        old = None
        if found:
            stored = db.execute(f"SELECT json FROM knowledge_{change.kind}s WHERE id=?", (change.identifier,)).fetchone()
            if stored is None or _metadata(db, published_row_digest_key(*key)) != emitted_row_digest(stored[0]):
                raise ValueError("addressed old carrier checksum differs")
            old = json.loads(stored[0])
            if old.get("id") != change.identifier:
                raise ValueError("addressed carrier identity differs")
            search_identity = db.execute("SELECT kind,identifier FROM search_documents WHERE doc_id=?", (found[0],)).fetchone()
            expected_identity = (change.kind, json.dumps(change.identifier, ensure_ascii=False, sort_keys=True).encode("utf-8", "surrogatepass"))
            if search_identity is None or tuple(search_identity) != expected_identity:
                raise ValueError("prepared/search exact address identity differs")
        if change.operation == "delete":
            if change.item is not None or change.source_order is not None:
                raise ValueError("delete must not contain a replacement or order")
            address, token = found
            db.execute(f"DELETE FROM knowledge_{change.kind}s WHERE id=?", (change.identifier,))
            db.execute("DELETE FROM prepared_documents WHERE kind=? AND id=?", key)
            db.execute("DELETE FROM knowledge_lens_order WHERE kind=? AND id=?", key)
            db.execute("DELETE FROM edge_meta WHERE key=?", (published_row_digest_key(*key),))
            search_changes.append(SearchChange("delete", address))
            if change.kind == "node":
                deleted_nodes.append(change.identifier)
            raw = None
        else:
            raw = _row(change.kind, change.item, limits)
            changed_bytes += len(raw.encode("utf-8"))
            if changed_bytes > limits.max_change_bytes:
                raise ValueError("delta retained change byte budget exceeded")
            if change.item["id"] != change.identifier:
                raise ValueError("replacement identity differs from addressed target")
            if found:
                address, token = found
                token = token if change.source_order is None else change.source_order
            else:
                high_water += 1
                address, token = high_water, change.source_order
            if type(token) is not int or not 0 <= token <= MAX_ADDRESS or address > MAX_ADDRESS:
                raise ValueError("explicit valid insertion source order/address required")
            _put_row(db, change.kind, change.item, raw, limits)
            db.execute("INSERT INTO prepared_documents VALUES (?,?,?,?) ON CONFLICT(kind,id) DO UPDATE SET source_order=excluded.source_order", (*key, address, token))
            search_changes.append(SearchChange(change.operation, address, PreparedSearchDocument.from_item(address, change.kind, change.item, token)))
            if change.kind == "relation":
                relations.append(change.item)
        for item, adjustment in ((old, -1), (change.item, 1)):
            if item is not None:
                cell = tuple(str(item.get(field) or "") for field in _DIMENSIONS[change.kind])
                histograms[change.kind][cell] = histograms[change.kind].get(cell, 0) + adjustment
                if histograms[change.kind][cell] == 0:
                    del histograms[change.kind][cell]
        frames.append([change.operation, *key, address, token, emitted_row_digest(raw)["sha256"] if raw is not None else None])
    for item in relations:
        _endpoint(db, item.get("from_id"))
        _endpoint(db, item.get("to_id"))
    for identifier in deleted_nodes:
        if db.execute("SELECT 1 FROM knowledge_relations WHERE from_id=? UNION ALL SELECT 1 FROM knowledge_relations WHERE to_id=? LIMIT 1", (identifier, identifier)).fetchone():
            raise ValueError("node deletion leaves incident relations")
    for kind, histogram in histograms.items():
        lens[kind + "_counts"] = [[*cell, count] for cell, count in sorted(histogram.items())]
    lens.update(source_revision=header["source_revision"], query_properties=copy.deepcopy(header.get("query_properties", [])))
    descriptor = {"schema": DESCRIPTOR_SCHEMA, "mode": "delta-history", "profile": SCHEMA,
                  "algorithm": ALGORITHM, "capabilities": CAPABILITIES,
                  "parent_data_revision": top["data_revision"], "header": header,
                  "catalog_sha256": _hash(catalog), "changes": frames}
    binding = _publish_header(db, header, catalog, lens, descriptor, epoch + 1, limits)
    SearchStore.apply_delta_transaction(db, expected_binding=expected_binding, new_binding=binding,
                                       changes=search_changes, max_mutations=limits.max_mutations)
    if db.execute("SELECT high_water FROM search_header WHERE singleton=1").fetchone()[0] != high_water:
        raise ValueError("prepared/search address high-water differs")
    db.execute("UPDATE prepared_state SET high_water=?,max_pages=?,descriptor=? WHERE singleton=1", (high_water, maximum, _compact(descriptor)))
    _cap(db, limits, maximum)
    if db.total_changes - before > limits.max_mutations:
        raise ValueError("whole publication mutation budget exceeded")
    return binding


def apply_prepared_delta(path: str | Path, **kwargs) -> dict:
    """Offline file owner wrapper; one transaction commits or rolls back all lanes."""
    path = Path(path).absolute()
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError("prepared publication must be a regular non-symlink file")
    db = sqlite3.connect(path.as_uri() + "?mode=rw", uri=True, isolation_level=None)
    try:
        db.execute("BEGIN IMMEDIATE")
        binding = apply_prepared_delta_transaction(db, **kwargs)
        db.execute("COMMIT")
        return binding
    except BaseException:
        if db.in_transaction:
            db.execute("ROLLBACK")
        raise
    finally:
        db.close()

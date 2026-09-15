"""Pure source-navigation row projection shared by the full edge producer."""

from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
from typing import Any, Callable, Literal, Mapping

from incremental_runtime import MAX_D1_SQL_ROW_VALUE_BYTES
from tos_access.portable_paths import normalize_paths
from tos_access.published_read_metadata import (
    emitted_row_digest,
    published_source_navigation_digest_key,
)


SQL_CHUNK_BYTES = 32_000
SourceNavigationKind = Literal["nodes", "edges", "rights"]
SQLiteValue = str | int | None


def compact_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def sql_text(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def _sql_value(value: SQLiteValue) -> str:
    if value is None:
        return "NULL"
    if type(value) is int:
        return str(value)
    if type(value) is str:
        return sql_text(value)
    raise TypeError("source-navigation row values must be strings, integers, or NULL")


def chunk_text(value: str, size: int = SQL_CHUNK_BYTES) -> list[str]:
    """Split text without breaking UTF-8 and keep escaped INSERTs bounded."""
    chunks: list[str] = []
    encoded = value.encode("utf-8")
    start = 0
    while start < len(encoded):
        end = min(start + size, len(encoded))
        while end < len(encoded) and end > start and encoded[end] & 0xC0 == 0x80:
            end -= 1
        if end == start:
            raise ValueError("chunk size cannot hold one UTF-8 character")
        chunks.append(encoded[start:end].decode("utf-8"))
        start = end
    if not chunks:
        chunks.append("")
    return chunks


def source_navigation_selection_properties(item: Mapping[str, Any]) -> dict[str, Any]:
    """Return only the source-navigation fields used by edge selection."""
    properties = item.get("properties")
    if not isinstance(properties, dict):
        return {}
    return {
        key: properties[key]
        for key in ("packet_id", "access_status")
        if key in properties
    }


def prepare_source_navigation_row(
    values: tuple[str, ...],
    item_json: str,
    *,
    json_position: int,
    properties_position: int,
) -> tuple[tuple[str, ...], str | None]:
    """Keep selection fields bounded while retaining the full source JSON."""
    return _bound_source_navigation_values(
        values,
        item_json,
        json_position=json_position,
        properties_position=properties_position,
        encode=lambda value: value,
        empty_json=sql_text(""),
        empty_properties=sql_text(""),
    )


def _bound_source_navigation_values(
    values: tuple[SQLiteValue, ...],
    item_json: str,
    *,
    json_position: int,
    properties_position: int,
    encode: Callable[[SQLiteValue], str],
    empty_json: SQLiteValue,
    empty_properties: SQLiteValue,
) -> tuple[tuple[SQLiteValue, ...], str | None]:
    """Apply the producer row budget once, at either typed or SQL boundary."""
    row_size = lambda candidate: sum(len(encode(value).encode("utf-8")) for value in candidate) + 1024
    if row_size(values) <= MAX_D1_SQL_ROW_VALUE_BYTES:
        return values, None

    compact_values = list(values)
    compact_values[json_position] = empty_json
    # A large properties object is a selection hint only. If retaining it
    # would breach the row budget, leave it empty and keep the object in the
    # one lossless source JSON payload.
    if row_size(tuple(compact_values)) > MAX_D1_SQL_ROW_VALUE_BYTES:
        compact_values[properties_position] = empty_properties
    if row_size(tuple(compact_values)) > MAX_D1_SQL_ROW_VALUE_BYTES:
        raise RuntimeError("source-navigation selection fields exceed the D1 row budget")
    return tuple(compact_values), item_json


@dataclass(frozen=True)
class SourceNavigationRow:
    """One typed serving row plus its optional lossless payload rows."""

    kind: SourceNavigationKind
    ordinal: int
    table: str
    payload_table: str
    id_column: str
    item_id: str
    columns: tuple[str, ...]
    values: tuple[SQLiteValue, ...]
    sql_values: tuple[str, ...]
    chunked_text: tuple[tuple[str, str], ...]
    payload_rows: tuple[tuple[str, int, str], ...]


_TABLES: dict[SourceNavigationKind, tuple[str, str, str]] = {
    "nodes": (
        "source_navigation_nodes",
        "source_navigation_node_payload",
        "node_id",
    ),
    "edges": (
        "source_navigation_edges",
        "source_navigation_edge_payload",
        "edge_id",
    ),
    "rights": (
        "source_navigation_rights",
        "source_navigation_rights_payload",
        "rights_id",
    ),
}


COLUMNS: dict[str, tuple[str, ...]] = {
    "source_navigation_nodes": (
        "node_id",
        "ord",
        "node_kind",
        "source_ref",
        "label",
        "identity_status",
        "properties_json",
        "json",
    ),
    "source_navigation_node_payload": ("id", "part", "json_chunk"),
    "source_navigation_edges": (
        "edge_id",
        "ord",
        "from_id",
        "to_id",
        "edge_kind",
        "predicate_id",
        "review_status",
        "source_refs_json",
        "json",
    ),
    "source_navigation_edge_payload": ("id", "part", "json_chunk"),
    "source_navigation_rights": ("rights_id", "ord", "scope_refs_json", "json"),
    "source_navigation_rights_payload": ("id", "part", "json_chunk"),
}


def project_source_navigation_row(
    kind: SourceNavigationKind,
    ordinal: int,
    item: Mapping[str, Any],
    repo_root: Path,
) -> SourceNavigationRow:
    """Project one native source-navigation item into serving SQL tuples.

    ``repo_root`` is explicit so callers using a temporary or external
    producer root normalize paths exactly as the full builder does. The
    returned values are typed SQLite tuples for consumers that need a direct
    row projection. The full builder uses the sibling ``sql_values`` tuple at
    its explicit SQL quoting boundary; payload rows remain typed until then.
    """
    if kind not in _TABLES:
        raise ValueError(f"unknown source-navigation kind: {kind!r}")
    if type(ordinal) is not int or ordinal < 0:
        raise ValueError("source-navigation ordinal must be a non-negative integer")
    if not isinstance(item, Mapping):
        raise TypeError("source-navigation item must be an object")

    # ``normalize_paths`` intentionally works on ordinary dictionaries. Make
    # that conversion explicit while retaining source key insertion order.
    normalized = normalize_paths(dict(item), repo_root)
    item_json = compact_json(normalized)
    table, payload_table, id_column = _TABLES[kind]

    if kind == "nodes":
        properties_json = compact_json(source_navigation_selection_properties(normalized))
        item_id = str(normalized.get("node_id") or "")
        columns = COLUMNS[table]
        values = (
            item_id,
            ordinal,
            str(normalized.get("node_kind") or ""),
            str(normalized.get("source_ref") or ""),
            str(normalized.get("label") or ""),
            str(normalized.get("identity_status") or ""),
            properties_json,
            item_json,
        )
        json_position = 7
        properties_position = 6
        chunked_text = (("properties_json", properties_json), ("json", item_json))
    elif kind == "edges":
        source_refs = normalized.get("source_refs")
        source_refs_json = compact_json(source_refs if isinstance(source_refs, list) else [])
        item_id = str(normalized.get("edge_id") or "")
        columns = COLUMNS[table]
        values = (
            item_id,
            ordinal,
            str(normalized.get("from_id") or ""),
            str(normalized.get("to_id") or ""),
            str(normalized.get("edge_kind") or ""),
            str(normalized.get("predicate_id") or ""),
            str(normalized.get("review_status") or ""),
            source_refs_json,
            item_json,
        )
        json_position = 8
        properties_position = 7
        chunked_text = (("source_refs_json", source_refs_json), ("json", item_json))
    else:
        scope_refs = normalized.get("scope_refs")
        scope_refs_json = compact_json(scope_refs if isinstance(scope_refs, list) else [])
        item_id = str(normalized.get("rights_id") or "")
        columns = COLUMNS[table]
        values = (
            item_id,
            ordinal,
            scope_refs_json,
            item_json,
        )
        json_position = 3
        properties_position = 2
        chunked_text = (("scope_refs_json", scope_refs_json), ("json", item_json))

    row_values, payload_json = _bound_source_navigation_values(
        values,
        item_json,
        json_position=json_position,
        properties_position=properties_position,
        encode=_sql_value,
        empty_json="",
        empty_properties="",
    )
    if payload_json is None:
        payload_rows: tuple[tuple[str, int, str], ...] = ()
        inline_chunks = chunked_text
    else:
        payload_rows = tuple(
            (item_id, part, chunk)
            for part, chunk in enumerate(chunk_text(payload_json))
        )
        inline_chunks = ()
    return SourceNavigationRow(
        kind=kind,
        ordinal=ordinal,
        table=table,
        payload_table=payload_table,
        id_column=id_column,
        item_id=item_id,
        columns=columns,
        values=row_values,
        sql_values=tuple(_sql_value(value) for value in row_values),
        chunked_text=inline_chunks,
        payload_rows=payload_rows,
    )


def source_navigation_digest_row(
    projected: SourceNavigationRow,
) -> tuple[str, int, str]:
    """Return the bounded edge-meta checksum for one emitted navigation row.

    The digest is over the exact JSON bytes emitted by the producer.  For a
    bounded inline row those bytes remain in the serving row; for a payload
    row they are the ordered concatenation of its lossless chunks.  Do not
    parse and re-serialize here: the checksum protects the physical carrier.
    """
    json_position = projected.columns.index("json")
    if projected.payload_rows:
        item_json = "".join(
            chunk for _item_id, _part, chunk in projected.payload_rows
        )
    else:
        item_json = projected.values[json_position]
    if not isinstance(item_json, str) or not item_json:
        raise ValueError("source-navigation emitted JSON is missing")
    return (
        published_source_navigation_digest_key(projected.kind, projected.item_id),
        0,
        compact_json(emitted_row_digest(item_json)),
    )


def project_rows(
    collection: SourceNavigationKind,
    ordinal: int,
    item: Mapping[str, Any],
    repo_root: Path,
) -> dict[str, list[tuple[SQLiteValue, ...]]]:
    """Return canonical serving-table rows for one native source item.

    The primary row is always present. If its lossless JSON exceeds the D1
    row budget, the second entry is the corresponding payload table with
    already bounded ``(id, part, json_chunk)`` tuples. Table names are the
    published names (without the full-build ``_next`` staging suffix), so the
    same projection can be consumed by future delta code.
    """
    projection = project_source_navigation_row(collection, ordinal, item, repo_root)
    result: dict[str, list[tuple[SQLiteValue, ...]]] = {
        projection.table: [projection.values],
    }
    if projection.payload_rows:
        result[projection.payload_table] = [
            (item_id, part, chunk)
            for item_id, part, chunk in projection.payload_rows
        ]
    # ``edge_meta`` is a global metadata carrier, deliberately kept outside
    # ``COLUMNS`` so navigation-product absence checks do not treat it as a
    # serving table.
    result["edge_meta"] = [source_navigation_digest_row(projection)]
    return result

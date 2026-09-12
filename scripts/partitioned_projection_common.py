"""Source-builder binding for the portable partitioned projection contract."""
from __future__ import annotations

from contextlib import contextmanager
from pathlib import Path
import sqlite3
import sys
import tempfile

from jsonschema import Draft202012Validator, ValidationError, validators

_ACCESS = Path(__file__).resolve().parents[1] / "access/src"
if str(_ACCESS) not in sys.path:
    sys.path.insert(0, str(_ACCESS))
from tos_access.disk_collections import DiskCollections, DiskMap, DiskSequence, canonical_digest, json_chunks
from tos_access.projection_store import Collection, ProjectionReader, ProjectionStoreError, write_projection, is_partitioned, load_projection


PROJECTION_PART_ROOTS = (
    "ToS/derived-exports/tos_corpus_index.min.parts",
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.parts",
)
CORPUS_COLLECTIONS = {
    "nodes": ("node_id", ("source_path",)),
    "resources": ("path", ("path",)),
    "manifests": ("path", ("path",)),
    "relation_packs": ("pack_id", ("path",)),
    "relation_edges": (("pack_id", "edge_id"), ("pack_id", "edge_id")),
    "source_navigation/nodes": ("node_id", ("node_id",)),
    "source_navigation/edges": ("edge_id", ("edge_id",)),
    "source_navigation/rights": ("rights_id", ("rights_id",)),
}
BIBLIOGRAPHIC_COLLECTIONS = {
    "nodes": ("node_id", ("node_id",)),
    "edges": ("edge_id", ("edge_id",)),
    "claim_traces": ("claim_ref", ("claim_ref",)),
    "input_digests": (None, ()),
}


def collection_policy(payload):
    if payload["schema_version"] == "tos_corpus_index_v1":
        return CORPUS_COLLECTIONS
    if payload["schema_version"] == "tos_source_witness_bibliographic_graph_v1":
        return BIBLIOGRAPHIC_COLLECTIONS
    raise ProjectionStoreError("no owner collection policy for this logical schema")


@contextmanager
def build_storage():
    """Disposable build-owned disk staging; no persistent query side effects."""
    with tempfile.TemporaryDirectory(prefix="tos-source-build-") as directory:
        connection = sqlite3.connect(Path(directory) / "stage.sqlite")
        connection.execute("PRAGMA temp_store=FILE")
        connection.execute("PRAGMA cache_size=-8192")
        try:
            yield DiskCollections(connection)
        finally:
            connection.close()


def _streaming_items(validator, items, instance, schema):
    if not isinstance(instance, DiskSequence):
        yield from Draft202012Validator.VALIDATORS["items"](validator, items, instance, schema)
        return
    prefix = len(schema.get("prefixItems", []))
    extra = len(instance) - prefix
    if extra <= 0:
        return
    if items is False:
        yield ValidationError(f"Expected at most {prefix} items but found {extra} extra")
        return
    # Draft 2020-12 applies the same schema to each item after prefixItems.
    # Its default indexed loop turns a sorted disk sequence into repeated
    # OFFSET scans. Traverse once while preserving the exact assertion/path.
    for index, item in enumerate(instance):
        if index >= prefix:
            yield from validator.descend(instance=item, schema=items, path=index)


def schema_validator(schema):
    # Same JSON Schema assertions; only these concrete build-owned collection
    # implementations represent JSON arrays/maps before serialization.
    checker = Draft202012Validator.TYPE_CHECKER.redefine(
        "array", lambda checker, value: isinstance(value, (list, DiskSequence)))
    checker = checker.redefine("object", lambda checker, value: isinstance(value, (dict, DiskMap)))
    return validators.extend(Draft202012Validator, validators={"items": _streaming_items},
                             type_checker=checker)(schema)


def ordered_rows(storage, values, field):
    if storage is None:
        return sorted(values, key=lambda row: str(row[field]))
    result = storage.sequence(values)
    result.sort(keyfield=field)
    return result


def write_partitioned_payload(path, payload, *, prune=False):
    header = dict(payload)
    if "source_navigation" in header:
        header["source_navigation"] = dict(header["source_navigation"])
    collections = {}
    for name, (key, ordering) in collection_policy(payload).items():
        owner = header
        parts = name.split("/")
        for part in parts[:-1]:
            owner = owner[part]
        rows = owner.pop(parts[-1])
        collections[name] = Collection(rows.items() if key is None else rows, key, ordering)
    return write_projection(Path(path), header, collections, prune=prune)


def check_partitioned_payload(path, expected):
    """Verify closure integrity and exact decoded source parity across codecs."""
    reader = ProjectionReader(path)
    policy = collection_policy(expected)
    if set(reader.manifest["collections"]) != set(policy):
        raise ProjectionStoreError(f"{path}: collection policy differs from the logical owner")
    for name, (key_field, ordering) in policy.items():
        spec = reader.manifest["collections"][name]
        key_field = list(key_field) if isinstance(key_field, tuple) else key_field
        if spec["key_field"] != key_field or spec["order_fields"] != list(ordering):
            raise ProjectionStoreError(f"{path}: identity or ordering policy differs for {name}")
    for _ in reader.closure_paths():
        pass
    with build_storage() as storage:
        actual = disk_payload(reader, storage)
        if canonical_digest(actual) != canonical_digest(expected):
            raise ProjectionStoreError(f"{path}: logical projection differs from the canonical source rebuild")
    reader.require_current()
    return reader


def disk_payload(reader, storage):
    """Explicit complete validation/export view whose collections stay on disk."""
    result = reader.metadata()
    for name, spec in reader.manifest["collections"].items():
        if spec["key_field"] is None:
            value = storage.mapping(reader.iter_items(name))
        else:
            value = storage.sequence(reader.iter_collection(name))
            fields = spec["order_fields"] or (spec["key_field"] if isinstance(spec["key_field"], list) else [spec["key_field"]])
            value.sort(key=lambda row: tuple(str(row.get(field, "")) for field in fields))
        owner = result
        parts = name.split("/")
        for part in parts[:-1]:
            owner = owner[part]
        owner[parts[-1]] = value
    reader.require_current()
    return result

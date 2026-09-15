"""Pure emitted-byte metadata ABI shared by the producer and cold reader.

These helpers do no I/O and select no publication. The source owner supplies
the complete normalized graph/catalog and independently selects the expected
serving binding. Checksums detect accidental drift, not a malicious co-owner.
"""
from __future__ import annotations

import copy
import hashlib
import json
import re
import unicodedata
from collections import Counter
from typing import Any

READER_SCHEMA = "tos_published_knowledge_reader_v1"
LENS_READER_SCHEMA = "tos_published_knowledge_reader_v2"
LOCAL_READ_MODEL_SCHEMA = "tos_local_prepared_read_model_v1"
LENS_READ_MODEL_SCHEMAS = frozenset({"tos_cloudflare_edge_read_model_v9", LOCAL_READ_MODEL_SCHEMA})
LENS_META_KEY = "knowledge_lens_top"
LENS_META_SCHEMA = "tos_published_lens_metadata_v1"
LENS_EXECUTION_VERSION = "tos-lens-execution-v7"
LENS_METADATA_MAX_BYTES = 1_048_576
LENS_SORT_KEY = "python-str-or-empty-lower-v1"
BINDING_SCHEMA = "tos_published_knowledge_snapshot_v1"
TOP_KEY = "knowledge_reader_top"
CATALOG_KEY = "knowledge_catalog"
ROW_INTEGRITY = "sha256-emitted-json-v1"
_HASH = re.compile(r"[0-9a-f]{64}")
_NORMALIZATION_KEYS = {
    "schema", "processor_digest", "entity_registry_digest",
    "relation_registry_digest", "configuration_digest",
}
_TOP_KEYS = {
    "schema", "read_model_schema", "source_revision", "data_revision",
    "graph_schema", "normalization_binding", "catalog_sha256",
    "row_integrity", "authority_boundary",
}
_BINDING_KEYS = {
    "schema", "publication_epoch", "metadata_sha256", "read_model_schema",
    "source_revision", "data_revision", "graph_schema", "normalization_binding",
}


class PublishedReadModelError(RuntimeError):
    """The configured prepared reader is unavailable; never fall back to a build."""


def _compact(value: Any) -> str:
    # Match the existing edge producer's emitted JSON byte framing.
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), allow_nan=False)


def emitted_row_digest(raw_json: str) -> dict[str, str]:
    """Checksum exact emitted JSON, separately from semantic content_revision."""
    return {"sha256": hashlib.sha256(raw_json.encode("utf-8")).hexdigest()}


def published_row_digest_key(kind: str, identifier: str) -> str:
    if kind not in {"node", "relation"} or not isinstance(identifier, str) or not identifier:
        raise ValueError("a node/relation and exact identifier are required")
    return f"knowledge_{kind}_digest:{identifier}"


def published_source_navigation_digest_key(kind: str, identifier: str) -> str:
    """Bounded key for an emitted native-navigation row, not rights admission."""
    if kind not in {'nodes', 'edges', 'rights'} or not isinstance(identifier, str) or not identifier:
        raise ValueError('a native navigation kind and exact identifier are required')
    identity = hashlib.sha256(identifier.encode('utf-8')).hexdigest()
    return f'source_navigation_row_digest:{kind}:{identity}'


def _normalization(value: Any) -> bool:
    return (isinstance(value, dict) and set(value) == _NORMALIZATION_KEYS
            and value.get("schema") == "tos_knowledge_graph_normalization_binding_v1"
            and all(isinstance(value.get(key), str) and _HASH.fullmatch(value[key])
                    for key in _NORMALIZATION_KEYS - {"schema"}))


def _validate_top(top: Any) -> None:
    lens = isinstance(top, dict) and top.get("schema") == LENS_READER_SCHEMA
    if (not isinstance(top, dict) or set(top) != _TOP_KEYS | ({"lens_sha256"} if lens else set())
            or top.get("schema") not in {READER_SCHEMA, LENS_READER_SCHEMA}
            or top.get("graph_schema") != "tos_knowledge_graph_v1"
            or not isinstance(top.get("read_model_schema"), str)
            or not (re.fullmatch(r"tos_cloudflare_edge_read_model_v[1-9][0-9]*", top["read_model_schema"])
                    or top["read_model_schema"] == LOCAL_READ_MODEL_SCHEMA)
            or (top["read_model_schema"] == LOCAL_READ_MODEL_SCHEMA and not lens)
            or top.get("row_integrity") != ROW_INTEGRITY
            or not _normalization(top.get("normalization_binding"))
            or any(not isinstance(top.get(key), str) or not _HASH.fullmatch(top[key])
                   for key in ("source_revision", "data_revision", "catalog_sha256", *(["lens_sha256"] if lens else [])))):
        raise PublishedReadModelError("invalid prepared knowledge reader metadata")
    boundary = top["authority_boundary"]
    if (not isinstance(boundary, dict) or boundary.get("source_owner") != "Tree-of-Sophia"
            or any(boundary.get(key) is not False for key in ("is_source", "is_canon", "writes_to_tree"))):
        raise PublishedReadModelError("prepared reader does not preserve the source authority boundary")


def published_reader_metadata(graph: dict[str, Any], catalog: dict[str, Any],
                              read_model_schema: str, revision: str, *,
                              lens_metadata: dict[str, Any] | None = None) -> dict[str, Any]:
    """Frame normalized inputs for the existing atomic chunked edge_meta lane.

    Serving clock, not this helper, owns the publication epoch. Graph/catalog
    validation and path normalization are the producer's preceding obligations.
    """
    if catalog.get("schema") != "tos_knowledge_catalog_v1" or catalog.get("source_revision") != graph.get("source_revision"):
        raise ValueError("catalog and graph must belong to the same source snapshot")
    top = {
        "schema": LENS_READER_SCHEMA if lens_metadata is not None else READER_SCHEMA,
        "read_model_schema": read_model_schema,
        "source_revision": graph.get("source_revision"), "data_revision": revision,
        "graph_schema": graph.get("schema"),
        "normalization_binding": copy.deepcopy(graph.get("normalization_binding")),
        "catalog_sha256": emitted_row_digest(_compact(catalog))["sha256"],
        "row_integrity": ROW_INTEGRITY,
        "authority_boundary": copy.deepcopy(graph.get("authority_boundary")),
    }
    if lens_metadata is not None:
        validate_lens_metadata(lens_metadata, graph.get("source_revision"))
        top["lens_sha256"] = emitted_row_digest(_compact(lens_metadata))["sha256"]
    _validate_top(top)
    return {TOP_KEY: top, CATALOG_KEY: copy.deepcopy(catalog),
            **({LENS_META_KEY: copy.deepcopy(lens_metadata)} if lens_metadata is not None else {})}


def lens_order_row(kind: str, item: dict[str, Any]) -> tuple[str, str, str, str, str]:
    """Pure query-order carrier; the normalized source row remains authoritative."""
    identifier = item.get("id")
    if kind not in {"node", "relation"} or not isinstance(identifier, str) or not identifier:
        raise PublishedReadModelError("prepared lens ordering requires an exact node/relation identifier")
    return (kind, identifier, identifier.lower(),
            str(item.get("from_id") or "") if kind == "relation" else "",
            str(item.get("to_id") or "") if kind == "relation" else "")


def published_lens_metadata(graph: dict[str, Any]) -> dict[str, Any]:
    """Build exact small-dimensional counts from already normalized owner rows."""
    nodes = Counter(tuple(str(item.get(key) or "") for key in ("source_graph", "kind_id", "type_id"))
                    for item in graph.get("nodes", []))
    relations = Counter(tuple(str(item.get(key) or "") for key in ("source_graph", "predicate_id", "relation_type_id"))
                        for item in graph.get("relations", []))
    result = {"schema": LENS_META_SCHEMA, "execution_version": LENS_EXECUTION_VERSION,
              "source_revision": graph.get("source_revision"), "sort_key": LENS_SORT_KEY,
              "unicode_version": unicodedata.unidata_version,
              "query_properties": copy.deepcopy(graph.get("query_properties", [])),
              "node_counts": [[*key, nodes[key]] for key in sorted(nodes)],
              "relation_counts": [[*key, relations[key]] for key in sorted(relations)]}
    validate_lens_metadata(result, graph.get("source_revision"))
    return result


def validate_lens_metadata(value: Any, source_revision: str) -> None:
    fields = {"schema", "execution_version", "source_revision", "sort_key", "unicode_version",
              "query_properties", "node_counts", "relation_counts"}
    if (not isinstance(value, dict) or set(value) != fields
            or value.get("schema") != LENS_META_SCHEMA
            or value.get("execution_version") != LENS_EXECUTION_VERSION
            or value.get("source_revision") != source_revision
            or value.get("sort_key") != LENS_SORT_KEY
            or value.get("unicode_version") != unicodedata.unidata_version
            or not isinstance(value.get("query_properties"), list)
            or len(value["query_properties"]) > 4096):
        raise PublishedReadModelError("prepared lens metadata is unavailable or incompatible")
    for definition in value['query_properties']:
        if (not isinstance(definition, dict)
                or any(not isinstance(definition.get(key), str) or not definition[key]
                       for key in ('property_id', 'field', 'value_type'))
                or type(definition.get('inherited')) is not bool
                or any(not isinstance(definition.get(key), list)
                       or any(not isinstance(item, str) or not item for item in definition[key])
                       for key in ('applies_to', 'operators'))):
            raise PublishedReadModelError("prepared lens property definition framing is invalid")
    for name in ("node_counts", "relation_counts"):
        cells = value[name]
        if (not isinstance(cells, list) or len(cells) > 16384
                or any(not isinstance(cell, list) or len(cell) != 4
                       or any(not isinstance(part, str) or not part for part in cell[:3])
                       or type(cell[3]) is not int or not 1 <= cell[3] <= 9_007_199_254_740_991
                       for cell in cells)):
            raise PublishedReadModelError("prepared lens count histogram is invalid")
        keys = [tuple(cell[:3]) for cell in cells]
        if keys != sorted(set(keys)):
            raise PublishedReadModelError("prepared lens count cells are duplicated or unordered")
    if len(_compact(value).encode("utf-8")) > LENS_METADATA_MAX_BYTES:
        raise PublishedReadModelError("prepared lens metadata exceeds its bounded publication size")


def published_snapshot_binding(top: dict[str, Any], publication_epoch: int) -> dict[str, Any]:
    """Frame an owner-selected binding, including the actual serving generation.

    Reading a DB header and calling this helper does not independently verify
    current source/policy authority. The caller must select the publication.
    """
    _validate_top(top)
    if type(publication_epoch) is not int or not 0 <= publication_epoch <= 9_007_199_254_740_991:
        raise ValueError("publication epoch must be a nonnegative safe integer")
    return {
        "schema": BINDING_SCHEMA, "publication_epoch": publication_epoch,
        "metadata_sha256": emitted_row_digest(_compact(top))["sha256"],
        **{key: copy.deepcopy(top[key]) for key in (
            "read_model_schema", "source_revision", "data_revision", "graph_schema", "normalization_binding")},
    }

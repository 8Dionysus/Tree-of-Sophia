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
from typing import Any

READER_SCHEMA = "tos_published_knowledge_reader_v1"
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


def _normalization(value: Any) -> bool:
    return (isinstance(value, dict) and set(value) == _NORMALIZATION_KEYS
            and value.get("schema") == "tos_knowledge_graph_normalization_binding_v1"
            and all(isinstance(value.get(key), str) and _HASH.fullmatch(value[key])
                    for key in _NORMALIZATION_KEYS - {"schema"}))


def _validate_top(top: Any) -> None:
    if (not isinstance(top, dict) or set(top) != _TOP_KEYS
            or top.get("schema") != READER_SCHEMA
            or top.get("graph_schema") != "tos_knowledge_graph_v1"
            or not isinstance(top.get("read_model_schema"), str)
            or not re.fullmatch(r"tos_cloudflare_edge_read_model_v[1-9][0-9]*", top["read_model_schema"])
            or top.get("row_integrity") != ROW_INTEGRITY
            or not _normalization(top.get("normalization_binding"))
            or any(not isinstance(top.get(key), str) or not _HASH.fullmatch(top[key])
                   for key in ("source_revision", "data_revision", "catalog_sha256"))):
        raise PublishedReadModelError("invalid prepared knowledge reader metadata")
    boundary = top["authority_boundary"]
    if (not isinstance(boundary, dict) or boundary.get("source_owner") != "Tree-of-Sophia"
            or any(boundary.get(key) is not False for key in ("is_source", "is_canon", "writes_to_tree"))):
        raise PublishedReadModelError("prepared reader does not preserve the source authority boundary")


def published_reader_metadata(graph: dict[str, Any], catalog: dict[str, Any],
                              read_model_schema: str, revision: str) -> dict[str, Any]:
    """Frame normalized inputs for the existing atomic chunked edge_meta lane.

    Serving clock, not this helper, owns the publication epoch. Graph/catalog
    validation and path normalization are the producer's preceding obligations.
    """
    if catalog.get("schema") != "tos_knowledge_catalog_v1" or catalog.get("source_revision") != graph.get("source_revision"):
        raise ValueError("catalog and graph must belong to the same source snapshot")
    top = {
        "schema": READER_SCHEMA, "read_model_schema": read_model_schema,
        "source_revision": graph.get("source_revision"), "data_revision": revision,
        "graph_schema": graph.get("schema"),
        "normalization_binding": copy.deepcopy(graph.get("normalization_binding")),
        "catalog_sha256": emitted_row_digest(_compact(catalog))["sha256"],
        "row_integrity": ROW_INTEGRITY,
        "authority_boundary": copy.deepcopy(graph.get("authority_boundary")),
    }
    _validate_top(top)
    return {TOP_KEY: top, CATALOG_KEY: copy.deepcopy(catalog)}


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

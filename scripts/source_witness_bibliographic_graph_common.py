#!/usr/bin/env python3
"""Build the source-returnable bibliographic claim graph read model."""

from __future__ import annotations

import hashlib
import json
from collections import Counter
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
CATALOG_ROOT = SOURCE_ROOT / "catalog"
CLAIM_CATALOG_REF = (CATALOG_ROOT / "claims.jsonl").as_posix()
CATALOG_MANIFEST_REF = (CATALOG_ROOT / "catalog.manifest.json").as_posix()
OBJECT_CATALOG_REFS = {
    record_type: (CATALOG_ROOT / f"{record_type}s.jsonl").as_posix()
    for record_type in ("agent", "work", "expression", "edition", "collection", "item")
}
SCHEMA_REF = "ToS/contracts/source-witness-bibliographic-graph.schema.json"
GRAPH_PATH = (
    REPO_ROOT
    / "ToS"
    / "derived-exports"
    / "graph"
    / "source-witness-bibliographic-claims.min.json"
)
GRAPH_REF = "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json"
VALIDATION_REFS = (
    "scripts/build_source_witness_bibliographic_graph.py",
    "scripts/query_source_witness_bibliographic_graph.py",
    "scripts/validate_source_witness_bibliographic_graph.py",
    "tests/test_source_witness_bibliographic_graph.py",
)


class BibliographicGraphBuildError(ValueError):
    """Raised when a source claim cannot be projected without weakening return."""


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def canonical_digest(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def file_digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def repo_ref(path: Path, repo_root: Path = REPO_ROOT) -> str:
    return path.relative_to(repo_root).as_posix()


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise BibliographicGraphBuildError(f"{repo_ref(path)}: cannot load JSON: {exc}") from exc
    if not isinstance(payload, dict):
        raise BibliographicGraphBuildError(f"{repo_ref(path)}: JSON root must be an object")
    return payload


def iter_jsonl(path: Path, repo_root: Path = REPO_ROOT) -> Iterable[tuple[int, dict[str, Any]]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise BibliographicGraphBuildError(f"{repo_ref(path, repo_root)}: cannot read JSONL: {exc}") from exc
    for line_number, raw_line in enumerate(lines, start=1):
        if not raw_line.strip():
            continue
        try:
            payload = json.loads(raw_line)
        except json.JSONDecodeError as exc:
            raise BibliographicGraphBuildError(
                f"{repo_ref(path, repo_root)}:{line_number}: cannot parse JSON: {exc}"
            ) from exc
        if not isinstance(payload, dict):
            raise BibliographicGraphBuildError(
                f"{repo_ref(path, repo_root)}:{line_number}: JSONL row must be an object"
            )
        yield line_number, payload


def load_schema(repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    return load_json(repo_root / SCHEMA_REF)


def validate_payload_schema(payload: dict[str, Any], repo_root: Path = REPO_ROOT) -> None:
    validator = Draft202012Validator(load_schema(repo_root))
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.absolute_path))
    if errors:
        error = errors[0]
        location = "".join(
            f"[{part}]" if isinstance(part, int) else f".{part}"
            for part in error.absolute_path
        )
        raise BibliographicGraphBuildError(
            f"schema violation at {location.lstrip('.') or '<root>'}: {error.message}"
        )


def _catalog_input_digests(repo_root: Path) -> dict[str, str]:
    refs = [CATALOG_MANIFEST_REF, CLAIM_CATALOG_REF, *OBJECT_CATALOG_REFS.values()]
    digests: dict[str, str] = {}
    for ref in refs:
        path = repo_root / ref
        if not path.is_file():
            raise BibliographicGraphBuildError(f"{ref}: required catalog input is missing")
        digests[ref] = file_digest(path)
    return dict(sorted(digests.items()))


def _load_object_catalog(repo_root: Path) -> dict[str, dict[str, Any]]:
    objects: dict[str, dict[str, Any]] = {}
    for expected_type, relative in OBJECT_CATALOG_REFS.items():
        path = repo_root / relative
        for line_number, entry in iter_jsonl(path, repo_root):
            location = f"{relative}:{line_number}"
            record_id = entry.get("record_id")
            if not isinstance(record_id, str) or not record_id:
                raise BibliographicGraphBuildError(f"{location}: record_id is missing")
            if entry.get("record_type") != expected_type:
                raise BibliographicGraphBuildError(
                    f"{location}: record_type differs from catalog family {expected_type!r}"
                )
            if record_id in objects:
                raise BibliographicGraphBuildError(f"{location}: duplicate record_id {record_id!r}")
            source_ref = entry.get("source_record_ref")
            record_sha256 = entry.get("record_sha256")
            if not isinstance(source_ref, str) or not isinstance(record_sha256, str):
                raise BibliographicGraphBuildError(
                    f"{location}: source_record_ref and record_sha256 are required"
                )
            source_path = repo_root / source_ref
            source_payload = load_json(source_path)
            if canonical_digest(source_payload) != record_sha256:
                raise BibliographicGraphBuildError(
                    f"{location}: source record digest differs from {source_ref}"
                )
            objects[record_id] = entry
    return objects


def _load_claim_catalog(repo_root: Path) -> list[dict[str, Any]]:
    path = repo_root / CLAIM_CATALOG_REF
    claims: list[dict[str, Any]] = []
    seen: set[str] = set()
    for line_number, entry in iter_jsonl(path, repo_root):
        location = f"{CLAIM_CATALOG_REF}:{line_number}"
        claim_id = entry.get("claim_id")
        if not isinstance(claim_id, str) or not claim_id:
            raise BibliographicGraphBuildError(f"{location}: claim_id is missing")
        if claim_id in seen:
            raise BibliographicGraphBuildError(f"{location}: duplicate claim_id {claim_id!r}")
        seen.add(claim_id)
        if entry.get("claim_type") != "bibliographic":
            raise BibliographicGraphBuildError(
                f"{location}: bibliographic projection cannot admit {entry.get('claim_type')!r}"
            )
        if entry.get("assertion_layer") not in {
            "bibliographic_assertion",
            "scholarly_report",
        }:
            raise BibliographicGraphBuildError(
                f"{location}: assertion_layer is outside the bibliographic graph profile"
            )
        if entry.get("visibility") not in {"public", "public_metadata_only"}:
            raise BibliographicGraphBuildError(
                f"{location}: visibility is not safe for the tracked graph"
            )
        claims.append(entry)
    claims.sort(key=lambda entry: str(entry["claim_id"]))
    return claims


def _load_source_claim(
    entry: dict[str, Any],
    *,
    repo_root: Path,
) -> dict[str, Any]:
    source_ref = entry.get("source_claim_file_ref")
    source_line = entry.get("source_claim_line")
    if not isinstance(source_ref, str) or not isinstance(source_line, int):
        raise BibliographicGraphBuildError(
            f"{entry.get('claim_id')}: source claim file and line are required"
        )
    source_path = repo_root / source_ref
    rows = dict(iter_jsonl(source_path, repo_root))
    claim = rows.get(source_line)
    if claim is None:
        raise BibliographicGraphBuildError(
            f"{entry.get('claim_id')}: {source_ref}:{source_line} does not exist"
        )
    if claim.get("claim_id") != entry.get("claim_id"):
        raise BibliographicGraphBuildError(
            f"{source_ref}:{source_line}: claim identity differs from catalog"
        )
    if canonical_digest(claim) != entry.get("claim_sha256"):
        raise BibliographicGraphBuildError(
            f"{source_ref}:{source_line}: canonical claim digest differs from catalog"
        )
    projected_fields = (
        "claim_type",
        "assertion_layer",
        "subject_ref",
        "predicate",
        "object",
        "evidence_refs",
        "maker",
        "provenance_event_ref",
        "epistemic_status",
        "review_status",
        "visibility",
        "claim_version",
    )
    for field in projected_fields:
        if claim.get(field) != entry.get(field):
            raise BibliographicGraphBuildError(
                f"{source_ref}:{source_line}: catalog field {field!r} drifted"
            )
    review_refs = [
        review.get("review_id")
        for review in claim.get("reviews", [])
        if isinstance(review, dict)
    ]
    if review_refs != entry.get("review_refs"):
        raise BibliographicGraphBuildError(
            f"{source_ref}:{source_line}: review_refs differ from source claim"
        )
    return claim


def _scan_index(
    repo_root: Path,
    *,
    filename_pattern: str,
    id_field: str,
) -> dict[str, dict[str, Any]]:
    source_root = repo_root / SOURCE_ROOT
    indexed: dict[str, dict[str, Any]] = {}
    for path in sorted(source_root.rglob(filename_pattern)):
        if CATALOG_ROOT in path.relative_to(repo_root).parents:
            continue
        relative = repo_ref(path, repo_root)
        for line_number, payload in iter_jsonl(path, repo_root):
            record_id = payload.get(id_field)
            if record_id is None:
                continue
            if not isinstance(record_id, str) or not record_id:
                raise BibliographicGraphBuildError(
                    f"{relative}:{line_number}: invalid {id_field}"
                )
            if record_id in indexed:
                prior = indexed[record_id]
                raise BibliographicGraphBuildError(
                    f"{relative}:{line_number}: duplicate {id_field} {record_id!r}; "
                    f"first seen at {prior['source_ref']}:{prior['source_line']}"
                )
            indexed[record_id] = {
                "payload": payload,
                "source_ref": relative,
                "source_line": line_number,
                "source_sha256": canonical_digest(payload),
            }
    return indexed


def _node_id(kind: str, ref: str) -> str:
    if kind in {"identity", "claim", "provenance_event", "review"}:
        return f"{kind}:{ref}"
    digest = hashlib.sha256(ref.encode("utf-8")).hexdigest()
    return f"{kind}:sha256:{digest}"


def _literal_node(value: Any, claim: dict[str, Any], entry: dict[str, Any]) -> dict[str, Any]:
    value_digest = canonical_digest(value)
    claim_literal_digest = canonical_digest(
        {"claim_ref": claim["claim_id"], "value": value}
    )
    return {
        "node_id": f"literal:sha256:{claim_literal_digest}",
        "node_kind": "literal",
        "source_ref": entry["source_claim_file_ref"],
        "source_line": entry["source_claim_line"],
        "source_sha256": entry["claim_sha256"],
        "properties": {
            "value": value,
            "value_sha256": value_digest,
            "value_type": (
                "object"
                if isinstance(value, dict)
                else "array"
                if isinstance(value, list)
                else "boolean"
                if isinstance(value, bool)
                else "number"
                if isinstance(value, (int, float))
                else "string"
            ),
            "claim_ref": claim["claim_id"],
        },
    }


def _identity_node(entry: dict[str, Any]) -> dict[str, Any]:
    return {
        "node_id": _node_id("identity", str(entry["record_id"])),
        "node_kind": "identity",
        "source_ref": entry["source_record_ref"],
        "source_sha256": entry["record_sha256"],
        "properties": {
            "identity_ref": entry["record_id"],
            "identity_kind": entry["record_type"],
            "preferred_label": entry["preferred_label"],
            "identity_status": entry["identity_status"],
        },
    }


def _claim_node(entry: dict[str, Any], claim: dict[str, Any]) -> dict[str, Any]:
    return {
        "node_id": _node_id("claim", str(entry["claim_id"])),
        "node_kind": "claim",
        "source_ref": entry["source_claim_file_ref"],
        "source_line": entry["source_claim_line"],
        "source_sha256": entry["claim_sha256"],
        "properties": {
            "claim_ref": entry["claim_id"],
            "claim_type": entry["claim_type"],
            "assertion_layer": entry["assertion_layer"],
            "predicate": entry["predicate"],
            "epistemic_status": entry["epistemic_status"],
            "review_status": entry["review_status"],
            "visibility": entry["visibility"],
            "claim_version": entry["claim_version"],
            "confidence": claim.get("confidence"),
        },
    }


def _event_node(indexed: dict[str, Any]) -> dict[str, Any]:
    event = indexed["payload"]
    return {
        "node_id": _node_id("provenance_event", str(event["event_id"])),
        "node_kind": "provenance_event",
        "source_ref": indexed["source_ref"],
        "source_line": indexed["source_line"],
        "source_sha256": indexed["source_sha256"],
        "properties": {
            "event_ref": event["event_id"],
            "event_type": event["event_type"],
            "started_at": event["started_at"],
            "ended_at": event["ended_at"],
            "agent_refs": event["agent_refs"],
            "method": event["method"],
            "status": event["status"],
            "event_version": event["event_version"],
        },
    }


def _maker_node(
    maker: dict[str, Any],
    *,
    objects: dict[str, dict[str, Any]],
    claim_catalog_sha256: str,
) -> dict[str, Any]:
    agent_ref = str(maker["agent_ref"])
    properties: dict[str, Any] = {
        "agent_ref": agent_ref,
        "maker_type": maker["maker_type"],
    }
    if agent_ref in objects:
        identity = objects[agent_ref]
        properties["identity_node_id"] = _node_id("identity", agent_ref)
        source_ref = identity["source_record_ref"]
        source_sha256 = identity["record_sha256"]
    else:
        properties["identity_node_id"] = None
        source_ref = CLAIM_CATALOG_REF
        source_sha256 = claim_catalog_sha256
    return {
        "node_id": _node_id("maker", agent_ref),
        "node_kind": "maker",
        "source_ref": source_ref,
        "source_sha256": source_sha256,
        "properties": properties,
    }


def _evidence_node(
    evidence_ref: str,
    *,
    repo_root: Path,
    anchors: dict[str, dict[str, Any]],
    objects: dict[str, dict[str, Any]],
    events: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    node_id = _node_id("evidence", evidence_ref)
    if evidence_ref.startswith("ToS/"):
        path = repo_root / evidence_ref
        if not path.is_file():
            raise BibliographicGraphBuildError(
                f"{evidence_ref}: claim evidence path does not exist"
            )
        return {
            "node_id": node_id,
            "node_kind": "evidence",
            "source_ref": evidence_ref,
            "source_sha256": file_digest(path),
            "properties": {
                "evidence_ref": evidence_ref,
                "evidence_kind": "repo_path",
                "resolved": True,
            },
        }
    if evidence_ref in anchors:
        indexed = anchors[evidence_ref]
        anchor = indexed["payload"]
        return {
            "node_id": node_id,
            "node_kind": "evidence",
            "source_ref": indexed["source_ref"],
            "source_line": indexed["source_line"],
            "source_sha256": indexed["source_sha256"],
            "properties": {
                "evidence_ref": evidence_ref,
                "evidence_kind": "anchor",
                "resolved": True,
                "anchor_status": anchor.get("status"),
                "item_ref": anchor.get("item_id"),
                "file_ref": anchor.get("file_id"),
            },
        }
    if evidence_ref in objects:
        identity = objects[evidence_ref]
        return {
            "node_id": node_id,
            "node_kind": "evidence",
            "source_ref": identity["source_record_ref"],
            "source_sha256": identity["record_sha256"],
            "properties": {
                "evidence_ref": evidence_ref,
                "evidence_kind": "identity",
                "resolved": True,
                "identity_node_id": _node_id("identity", evidence_ref),
            },
        }
    if evidence_ref in events:
        indexed = events[evidence_ref]
        return {
            "node_id": node_id,
            "node_kind": "evidence",
            "source_ref": indexed["source_ref"],
            "source_line": indexed["source_line"],
            "source_sha256": indexed["source_sha256"],
            "properties": {
                "evidence_ref": evidence_ref,
                "evidence_kind": "provenance_event",
                "resolved": True,
                "provenance_event_node_id": _node_id("provenance_event", evidence_ref),
            },
        }
    raise BibliographicGraphBuildError(
        f"{evidence_ref}: claim evidence does not resolve to a tracked source surface"
    )


def _review_node(
    review: dict[str, Any],
    *,
    entry: dict[str, Any],
) -> dict[str, Any]:
    return {
        "node_id": _node_id("review", str(review["review_id"])),
        "node_kind": "review",
        "source_ref": entry["source_claim_file_ref"],
        "source_line": entry["source_claim_line"],
        "source_sha256": entry["claim_sha256"],
        "properties": {
            "review_ref": review["review_id"],
            "reviewer_kind": review["reviewer_kind"],
            "reviewer_ref": review["reviewer_ref"],
            "decision": review["decision"],
            "reviewed_at": review["reviewed_at"],
        },
    }


def _add_node(nodes: dict[str, dict[str, Any]], node: dict[str, Any]) -> None:
    node_id = str(node["node_id"])
    prior = nodes.get(node_id)
    if prior is not None and prior != node:
        raise BibliographicGraphBuildError(
            f"{node_id}: the same projected node was derived with conflicting content"
        )
    nodes[node_id] = node


def _edge(
    *,
    claim_entry: dict[str, Any],
    edge_kind: str,
    ordinal: int,
    from_id: str,
    to_id: str,
    evidence_node_ids: list[str],
    maker_node_id: str,
    provenance_event_node_id: str,
) -> dict[str, Any]:
    claim_id = str(claim_entry["claim_id"])
    edge_id = f"edge:{claim_id}:{edge_kind}:{ordinal:03d}"
    return {
        "edge_id": edge_id,
        "edge_kind": edge_kind,
        "from_id": from_id,
        "to_id": to_id,
        "claim_ref": claim_id,
        "claim_sha256": claim_entry["claim_sha256"],
        "evidence_node_ids": evidence_node_ids,
        "maker_node_id": maker_node_id,
        "provenance_event_node_id": provenance_event_node_id,
        "review_status": claim_entry["review_status"],
        "source_claim_file_ref": claim_entry["source_claim_file_ref"],
        "source_claim_line": claim_entry["source_claim_line"],
    }


def _validate_cross_references(payload: dict[str, Any]) -> None:
    nodes = {
        node["node_id"]: node
        for node in payload["nodes"]
        if isinstance(node, dict) and isinstance(node.get("node_id"), str)
    }
    if len(nodes) != len(payload["nodes"]):
        raise BibliographicGraphBuildError("projected node IDs must be unique")
    edges = {
        edge["edge_id"]: edge
        for edge in payload["edges"]
        if isinstance(edge, dict) and isinstance(edge.get("edge_id"), str)
    }
    if len(edges) != len(payload["edges"]):
        raise BibliographicGraphBuildError("projected edge IDs must be unique")
    claim_nodes = {
        node["properties"]["claim_ref"]: node["node_id"]
        for node in payload["nodes"]
        if node["node_kind"] == "claim"
    }
    for edge in payload["edges"]:
        for field in (
            "from_id",
            "to_id",
            "maker_node_id",
            "provenance_event_node_id",
        ):
            if edge[field] not in nodes:
                raise BibliographicGraphBuildError(
                    f"{edge['edge_id']}: {field} does not resolve"
                )
        if edge["from_id"] != claim_nodes.get(edge["claim_ref"]):
            raise BibliographicGraphBuildError(
                f"{edge['edge_id']}: every edge must start at its reified claim node"
            )
        if not edge["evidence_node_ids"]:
            raise BibliographicGraphBuildError(
                f"{edge['edge_id']}: every edge must preserve claim evidence"
            )
        for evidence_node_id in edge["evidence_node_ids"]:
            if evidence_node_id not in nodes:
                raise BibliographicGraphBuildError(
                    f"{edge['edge_id']}: evidence node does not resolve"
                )
            if nodes[evidence_node_id]["node_kind"] != "evidence":
                raise BibliographicGraphBuildError(
                    f"{edge['edge_id']}: evidence route does not end at an evidence node"
                )
        if nodes[edge["maker_node_id"]]["node_kind"] != "maker":
            raise BibliographicGraphBuildError(
                f"{edge['edge_id']}: maker route does not end at a maker node"
            )
        if nodes[edge["provenance_event_node_id"]]["node_kind"] != "provenance_event":
            raise BibliographicGraphBuildError(
                f"{edge['edge_id']}: provenance route does not end at an event node"
            )
        if edge["claim_sha256"] != nodes[edge["from_id"]]["source_sha256"]:
            raise BibliographicGraphBuildError(
                f"{edge['edge_id']}: claim digest differs from its claim node"
            )
    trace_claims: set[str] = set()
    for trace in payload["claim_traces"]:
        claim_ref = trace["claim_ref"]
        if claim_ref in trace_claims:
            raise BibliographicGraphBuildError(f"{claim_ref}: duplicate claim trace")
        trace_claims.add(claim_ref)
        for field in (
            "claim_node_id",
            "subject_node_id",
            "object_node_id",
            "maker_node_id",
            "provenance_event_node_id",
        ):
            if trace[field] not in nodes:
                raise BibliographicGraphBuildError(
                    f"{claim_ref}: {field} does not resolve"
                )
        if any(edge_id not in edges for edge_id in trace["edge_ids"]):
            raise BibliographicGraphBuildError(
                f"{claim_ref}: trace edge does not resolve"
            )
        expected_edge_ids = {
            edge_id
            for edge_id, edge in edges.items()
            if edge["claim_ref"] == claim_ref
        }
        if set(trace["edge_ids"]) != expected_edge_ids:
            raise BibliographicGraphBuildError(
                f"{claim_ref}: trace edge set differs from projected claim edges"
            )
        for field in ("evidence_node_ids", "counterevidence_node_ids"):
            for node_id in trace[field]:
                if node_id not in nodes or nodes[node_id]["node_kind"] != "evidence":
                    raise BibliographicGraphBuildError(
                        f"{claim_ref}: {field} does not resolve to evidence nodes"
                    )
        for node_id in trace["review_node_ids"]:
            if node_id not in nodes or nodes[node_id]["node_kind"] != "review":
                raise BibliographicGraphBuildError(
                    f"{claim_ref}: review_node_ids do not resolve to review nodes"
                )
    if trace_claims != set(claim_nodes):
        raise BibliographicGraphBuildError(
            "claim traces must cover every projected claim node exactly once"
        )


def _projection_fingerprint(payload: dict[str, Any]) -> str:
    material = dict(payload)
    material.pop("projection_fingerprint", None)
    return canonical_digest(material)


def build_payload(repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    manifest = load_json(repo_root / CATALOG_MANIFEST_REF)
    if manifest.get("schema_version") != "tos_source_witness_catalog_v2":
        raise BibliographicGraphBuildError(
            f"{CATALOG_MANIFEST_REF}: source-witness catalog v2 is required"
        )
    if manifest.get("claim_file") != CLAIM_CATALOG_REF:
        raise BibliographicGraphBuildError(
            f"{CATALOG_MANIFEST_REF}: claim_file differs from {CLAIM_CATALOG_REF}"
        )

    input_digests = _catalog_input_digests(repo_root)
    objects = _load_object_catalog(repo_root)
    claim_entries = _load_claim_catalog(repo_root)
    if manifest.get("counts", {}).get("claim") != len(claim_entries):
        raise BibliographicGraphBuildError(
            f"{CATALOG_MANIFEST_REF}: claim count differs from the claim catalog"
        )

    events = _scan_index(
        repo_root,
        filename_pattern="*provenance*.jsonl",
        id_field="event_id",
    )
    anchors = _scan_index(
        repo_root,
        filename_pattern="*anchor*.jsonl",
        id_field="anchor_id",
    )

    nodes: dict[str, dict[str, Any]] = {}
    edges: list[dict[str, Any]] = []
    traces: list[dict[str, Any]] = []
    review_counts: Counter[str] = Counter()
    visibility_counts: Counter[str] = Counter()
    claim_ids = {str(entry["claim_id"]) for entry in claim_entries}

    for entry in claim_entries:
        claim = _load_source_claim(entry, repo_root=repo_root)
        claim_id = str(claim["claim_id"])
        subject_ref = str(claim["subject_ref"])
        subject_entry = objects.get(subject_ref)
        if subject_entry is None:
            raise BibliographicGraphBuildError(
                f"{claim_id}: subject does not resolve through the object catalog"
            )
        subject_node = _identity_node(subject_entry)
        _add_node(nodes, subject_node)

        object_value = claim["object"]
        if isinstance(object_value, str) and object_value in objects:
            object_node = _identity_node(objects[object_value])
        elif isinstance(object_value, str) and object_value.startswith("tos."):
            raise BibliographicGraphBuildError(
                f"{claim_id}: identity-like object {object_value!r} is unresolved"
            )
        else:
            object_node = _literal_node(object_value, claim, entry)
        _add_node(nodes, object_node)

        claim_node = _claim_node(entry, claim)
        _add_node(nodes, claim_node)

        event_ref = str(claim["provenance_event_ref"])
        indexed_event = events.get(event_ref)
        if indexed_event is None:
            raise BibliographicGraphBuildError(
                f"{claim_id}: provenance event {event_ref!r} does not resolve"
            )
        event_node = _event_node(indexed_event)
        _add_node(nodes, event_node)

        maker = claim["maker"]
        if not isinstance(maker, dict):
            raise BibliographicGraphBuildError(f"{claim_id}: maker must be an object")
        maker_node = _maker_node(
            maker,
            objects=objects,
            claim_catalog_sha256=input_digests[CLAIM_CATALOG_REF],
        )
        _add_node(nodes, maker_node)
        identity_node_id = maker_node["properties"].get("identity_node_id")
        if isinstance(identity_node_id, str):
            _add_node(nodes, _identity_node(objects[str(maker["agent_ref"])]))

        evidence_node_ids: list[str] = []
        for evidence_ref in claim["evidence_refs"]:
            evidence_node = _evidence_node(
                str(evidence_ref),
                repo_root=repo_root,
                anchors=anchors,
                objects=objects,
                events=events,
            )
            _add_node(nodes, evidence_node)
            evidence_node_ids.append(str(evidence_node["node_id"]))
        evidence_node_ids = sorted(set(evidence_node_ids))
        if not evidence_node_ids:
            raise BibliographicGraphBuildError(
                f"{claim_id}: a projected claim must carry evidence"
            )

        counterevidence_refs = claim.get("counterevidence_refs", [])
        if not isinstance(counterevidence_refs, list):
            raise BibliographicGraphBuildError(
                f"{claim_id}: counterevidence_refs must be a list"
            )
        counterevidence_node_ids: list[str] = []
        for counterevidence_ref in counterevidence_refs:
            counterevidence_node = _evidence_node(
                str(counterevidence_ref),
                repo_root=repo_root,
                anchors=anchors,
                objects=objects,
                events=events,
            )
            _add_node(nodes, counterevidence_node)
            counterevidence_node_ids.append(str(counterevidence_node["node_id"]))
        counterevidence_node_ids = sorted(set(counterevidence_node_ids))

        review_node_ids: list[str] = []
        reviews = claim.get("reviews", [])
        if not isinstance(reviews, list):
            raise BibliographicGraphBuildError(f"{claim_id}: reviews must be a list")
        for review in reviews:
            if not isinstance(review, dict):
                raise BibliographicGraphBuildError(
                    f"{claim_id}: review entries must be objects"
                )
            review_node = _review_node(review, entry=entry)
            _add_node(nodes, review_node)
            review_node_ids.append(str(review_node["node_id"]))

        claim_node_id = str(claim_node["node_id"])
        edge_specs: list[tuple[str, str]] = [
            ("has_subject", str(subject_node["node_id"])),
            ("has_object", str(object_node["node_id"])),
            ("made_by", str(maker_node["node_id"])),
            ("generated_by", str(event_node["node_id"])),
        ]
        edge_specs.extend(("supported_by", node_id) for node_id in evidence_node_ids)
        edge_specs.extend(
            ("counterevidenced_by", node_id)
            for node_id in counterevidence_node_ids
        )
        edge_specs.extend(("reviewed_by", node_id) for node_id in sorted(review_node_ids))

        alternative_claim_refs = claim.get("alternative_claim_refs", [])
        if not isinstance(alternative_claim_refs, list):
            raise BibliographicGraphBuildError(
                f"{claim_id}: alternative_claim_refs must be a list"
            )
        for alternative_ref in sorted(str(ref) for ref in alternative_claim_refs):
            if alternative_ref not in claim_ids:
                raise BibliographicGraphBuildError(
                    f"{claim_id}: alternative claim {alternative_ref!r} is outside the projection"
                )
            edge_specs.append(
                ("alternative_to", _node_id("claim", alternative_ref))
            )

        supersedes_ref = claim.get("supersedes_claim_ref")
        if supersedes_ref is not None:
            if supersedes_ref not in claim_ids:
                raise BibliographicGraphBuildError(
                    f"{claim_id}: superseded claim {supersedes_ref!r} is outside the projection"
                )
            edge_specs.append(("supersedes", _node_id("claim", str(supersedes_ref))))

        claim_edge_ids: list[str] = []
        for ordinal, (edge_kind, to_id) in enumerate(edge_specs, start=1):
            projected_edge = _edge(
                claim_entry=entry,
                edge_kind=edge_kind,
                ordinal=ordinal,
                from_id=claim_node_id,
                to_id=to_id,
                evidence_node_ids=evidence_node_ids,
                maker_node_id=str(maker_node["node_id"]),
                provenance_event_node_id=str(event_node["node_id"]),
            )
            edges.append(projected_edge)
            claim_edge_ids.append(str(projected_edge["edge_id"]))

        traces.append(
            {
                "claim_ref": claim_id,
                "claim_sha256": entry["claim_sha256"],
                "claim_node_id": claim_node_id,
                "subject_node_id": subject_node["node_id"],
                "object_node_id": object_node["node_id"],
                "predicate": claim["predicate"],
                "assertion_layer": claim["assertion_layer"],
                "epistemic_status": claim["epistemic_status"],
                "confidence": claim.get("confidence"),
                "maker_node_id": maker_node["node_id"],
                "provenance_event_node_id": event_node["node_id"],
                "evidence_node_ids": evidence_node_ids,
                "counterevidence_node_ids": counterevidence_node_ids,
                "review_node_ids": sorted(review_node_ids),
                "review_status": claim["review_status"],
                "visibility": claim["visibility"],
                "alternative_claim_refs": sorted(
                    str(ref) for ref in alternative_claim_refs
                ),
                "counterevidence_refs": sorted(
                    str(ref) for ref in counterevidence_refs
                ),
                "supersedes_claim_ref": supersedes_ref,
                "source_claim_file_ref": entry["source_claim_file_ref"],
                "source_claim_line": entry["source_claim_line"],
                "source_claim_sha256": entry["claim_sha256"],
                "edge_ids": sorted(claim_edge_ids),
            }
        )
        review_counts[str(claim["review_status"])] += 1
        visibility_counts[str(claim["visibility"])] += 1

    nodes_list = sorted(nodes.values(), key=lambda node: str(node["node_id"]))
    edges.sort(key=lambda edge: str(edge["edge_id"]))
    traces.sort(key=lambda trace: str(trace["claim_ref"]))
    node_kind_counts = Counter(str(node["node_kind"]) for node in nodes_list)
    edge_kind_counts = Counter(str(edge["edge_kind"]) for edge in edges)

    payload: dict[str, Any] = {
        "schema_version": "tos_source_witness_bibliographic_graph_v1",
        "schema_ref": SCHEMA_REF,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_source_witness_bibliographic_claim_graph",
        "generated_by": "scripts/build_source_witness_bibliographic_graph.py",
        "source_refs": {
            "catalog_manifest_ref": CATALOG_MANIFEST_REF,
            "claim_catalog_ref": CLAIM_CATALOG_REF,
            "object_catalog_refs": OBJECT_CATALOG_REFS,
        },
        "input_digests": input_digests,
        "graph_layers": ["bibliographic"],
        "relation_model": {
            "assertion_form": "reified_claim_node",
            "direct_subject_object_edges": False,
            "edge_trace_rule": (
                "every edge starts at its claim node and carries the claim digest, "
                "evidence nodes, maker node, provenance event node, and review status"
            ),
            "runtime_owner": "abyss-stack",
        },
        "counts": {
            "source_claims": len(claim_entries),
            "nodes": len(nodes_list),
            "edges": len(edges),
            "claim_traces": len(traces),
            "direct_subject_object_edges": 0,
            "node_kinds": dict(sorted(node_kind_counts.items())),
            "edge_kinds": dict(sorted(edge_kind_counts.items())),
        },
        "review_counts": dict(sorted(review_counts.items())),
        "visibility_counts": dict(sorted(visibility_counts.items())),
        "nodes": nodes_list,
        "edges": edges,
        "claim_traces": traces,
        "authority_boundary": {
            "authoritative_claims": (
                "source claim packets named by source_claim_file_ref, "
                "source_claim_line, and source_claim_sha256"
            ),
            "projection_role": (
                "generated public-safe navigation over the tracked bibliographic "
                "claim catalog; deletable and rebuildable"
            ),
            "does_not_establish": [
                "bibliographic truth or claim acceptance",
                "generic creator equivalence",
                "textual identity, translation, semantics, sign, concept, or canon",
                "rights to publish source payload bytes",
                "Neo4j, RDF, KAG, UI, or runtime authority",
            ],
        },
        "validation_refs": list(VALIDATION_REFS),
    }
    payload["projection_fingerprint"] = _projection_fingerprint(payload)
    validate_payload_schema(payload, repo_root)
    _validate_cross_references(payload)
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return canonical_json(payload) + "\n"


def load_verified_projection(
    repo_root: Path = REPO_ROOT,
    *,
    graph_path: Path | None = None,
) -> dict[str, Any]:
    """Load the tracked reader only after exact source-parity verification."""

    path = graph_path or repo_root / GRAPH_REF
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        try:
            location = repo_ref(path, repo_root)
        except ValueError:
            location = str(path)
        raise BibliographicGraphBuildError(
            f"{location}: cannot load bibliographic graph: {exc}"
        ) from exc
    if not isinstance(payload, dict):
        raise BibliographicGraphBuildError(
            f"{GRAPH_REF}: bibliographic graph root must be an object"
        )

    validate_payload_schema(payload, repo_root)
    _validate_cross_references(payload)
    if payload.get("projection_fingerprint") != _projection_fingerprint(payload):
        raise BibliographicGraphBuildError(
            f"{GRAPH_REF}: projection fingerprint does not match graph content"
        )

    expected = build_payload(repo_root)
    if render_payload(payload) != render_payload(expected):
        raise BibliographicGraphBuildError(
            f"{GRAPH_REF}: graph differs from the exact source-backed rebuild"
        )
    return payload


def _source_claim_for_trace(
    trace: dict[str, Any],
    *,
    repo_root: Path,
) -> dict[str, Any]:
    source_ref = trace["source_claim_file_ref"]
    source_line = trace["source_claim_line"]
    rows = dict(iter_jsonl(repo_root / source_ref, repo_root))
    claim = rows.get(source_line)
    if claim is None:
        raise BibliographicGraphBuildError(
            f"{trace['claim_ref']}: {source_ref}:{source_line} no longer exists"
        )
    digest = canonical_digest(claim)
    if claim.get("claim_id") != trace["claim_ref"]:
        raise BibliographicGraphBuildError(
            f"{source_ref}:{source_line}: source-return claim identity differs"
        )
    if digest != trace["source_claim_sha256"]:
        raise BibliographicGraphBuildError(
            f"{source_ref}:{source_line}: source-return claim digest differs"
        )
    return claim


def query_projection(
    payload: dict[str, Any],
    *,
    repo_root: Path = REPO_ROOT,
    claim_ref: str | None = None,
    subject_ref: str | None = None,
    object_ref: str | None = None,
    predicate: str | None = None,
    review_status: str | None = None,
    visibility: str | None = None,
    limit: int = 20,
) -> dict[str, Any]:
    """Return deterministic, source-backed claim bundles using AND semantics."""

    selectors = {
        key: value
        for key, value in (
            ("claim_ref", claim_ref),
            ("subject_ref", subject_ref),
            ("object_ref", object_ref),
            ("predicate", predicate),
            ("review_status", review_status),
            ("visibility", visibility),
        )
        if value is not None
    }
    if not selectors:
        raise BibliographicGraphBuildError(
            "at least one exact query selector is required"
        )
    if isinstance(limit, bool) or not isinstance(limit, int) or not 1 <= limit <= 100:
        raise BibliographicGraphBuildError("query limit must be an integer from 1 to 100")

    validate_payload_schema(payload, repo_root)
    _validate_cross_references(payload)
    if payload.get("projection_fingerprint") != _projection_fingerprint(payload):
        raise BibliographicGraphBuildError(
            "projection fingerprint does not match graph content"
        )

    nodes = {node["node_id"]: node for node in payload["nodes"]}
    edges_by_claim: dict[str, list[dict[str, Any]]] = {}
    for edge in payload["edges"]:
        edges_by_claim.setdefault(str(edge["claim_ref"]), []).append(edge)
    for edges in edges_by_claim.values():
        edges.sort(key=lambda edge: str(edge["edge_id"]))

    selected: list[dict[str, Any]] = []
    for trace in payload["claim_traces"]:
        subject_node = nodes[trace["subject_node_id"]]
        object_node = nodes[trace["object_node_id"]]
        trace_subject_ref = subject_node["properties"].get("identity_ref")
        trace_object_ref = (
            object_node["properties"].get("identity_ref")
            if object_node["node_kind"] == "identity"
            else None
        )
        values = {
            "claim_ref": trace["claim_ref"],
            "subject_ref": trace_subject_ref,
            "object_ref": trace_object_ref,
            "predicate": trace["predicate"],
            "review_status": trace["review_status"],
            "visibility": trace["visibility"],
        }
        if any(values[key] != value for key, value in selectors.items()):
            continue

        source_claim = _source_claim_for_trace(trace, repo_root=repo_root)
        selected.append(
            {
                "claim_ref": trace["claim_ref"],
                "predicate": trace["predicate"],
                "claim_sha256": trace["claim_sha256"],
                "source_return": {
                    "file_ref": trace["source_claim_file_ref"],
                    "line": trace["source_claim_line"],
                    "canonical_sha256": trace["source_claim_sha256"],
                    "source_claim": source_claim,
                },
                "trace": trace,
                "claim_node": nodes[trace["claim_node_id"]],
                "subject_node": subject_node,
                "object_node": object_node,
                "evidence_nodes": [
                    nodes[node_id] for node_id in trace["evidence_node_ids"]
                ],
                "counterevidence_nodes": [
                    nodes[node_id] for node_id in trace["counterevidence_node_ids"]
                ],
                "maker_node": nodes[trace["maker_node_id"]],
                "provenance_event_node": nodes[
                    trace["provenance_event_node_id"]
                ],
                "review_nodes": [
                    nodes[node_id] for node_id in trace["review_node_ids"]
                ],
                "edges": edges_by_claim[trace["claim_ref"]],
            }
        )

    selected.sort(key=lambda match: str(match["claim_ref"]))
    if len(selected) > limit:
        raise BibliographicGraphBuildError(
            f"query matched {len(selected)} claims, exceeding explicit limit {limit}"
        )

    query = {
        "match_semantics": "all_selectors",
        "selectors": dict(sorted(selectors.items())),
        "limit": limit,
    }
    query_material = {
        "source_graph_fingerprint": payload["projection_fingerprint"],
        "query": query,
        "claim_refs": [match["claim_ref"] for match in selected],
    }
    return {
        "schema_version": "tos_source_witness_bibliographic_query_result_v1",
        "status": "ok" if selected else "no_match",
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "ephemeral_source_witness_bibliographic_query_result",
        "source_graph_ref": GRAPH_REF,
        "source_graph_sha256": hashlib.sha256(
            render_payload(payload).encode("utf-8")
        ).hexdigest(),
        "source_graph_fingerprint": payload["projection_fingerprint"],
        "query": query,
        "query_fingerprint": canonical_digest(query_material),
        "result_count": len(selected),
        "matches": selected,
        "authority_boundary": {
            "role": (
                "deterministic read-only source return over the generated "
                "bibliographic graph"
            ),
            "does_not_establish": [
                "claim truth or acceptance",
                "textual identity, translation, semantics, sign, concept, or canon",
                "rights to publish source payload bytes",
                "runtime, service, MCP, UI, Neo4j, RDF, or KAG authority",
            ],
        },
    }

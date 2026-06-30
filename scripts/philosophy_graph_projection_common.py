#!/usr/bin/env python3
"""Shared helpers for the ToS philosophy graph projection export."""

from __future__ import annotations

import json
import hashlib
from collections import Counter
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

from philosophy_multilingual_common import content_language_contract, multilingual_label


REPO_ROOT = Path(__file__).resolve().parents[1]
TOS_ROOT = REPO_ROOT / "ToS"
ATLAS_PROJECTION_REF = "ToS/derived-exports/philosophy_atlas_projection.min.json"
GRAPH_VIEW_CATALOG_REF = "ToS/derived-exports/philosophy_graph_views.min.json"
SOURCE_VIEW_CONTRACT_REF = "ToS/philosophy/graph-workbench/views/view-contracts.json"
LENS_REVIEW_CONTRACT_REF = "ToS/philosophy/graph-workbench/views/lens-review-contracts.json"
CLUSTER_CONTRACT_REF = "ToS/philosophy/graph-workbench/clusters/cluster-contracts.json"
REVIEW_PACKET_CONTRACT_REF = "ToS/philosophy/graph-workbench/review-packets/review-packet-contract.json"
GRAPH_PROJECTION_PATH = TOS_ROOT / "derived-exports" / "philosophy_graph_projection.min.json"
SCHEMA_REF = "ToS/contracts/philosophy-graph-projection.schema.json"
VALIDATION_REFS = (
    "scripts/build_philosophy_graph_projection.py",
    "scripts/validate_philosophy_graph_projection.py",
    "tests/test_philosophy_graph_projection.py",
)
PREDICATE_GRAPH_LAYERS = {
    "belongs_to_genre": {"conceptual-relation"},
    "canonized_by": {"canonical-relation", "transmission-relation"},
    "commented_by": {"transmission-relation"},
    "contested_by": {"conceptual-relation", "evidence-relation"},
    "contains_dossier": {"source-relation"},
    "contains_row": {"source-relation"},
    "contains_table": {"source-relation"},
    "contains_view": {"source-relation"},
    "develops_concept": {"conceptual-relation"},
    "figure_anchor": {"historical-relation", "source-relation"},
    "fragments_preserved_by": {"evidence-relation", "transmission-relation"},
    "has_atlas": {"source-relation"},
    "has_candidate_node": {"candidate-relation"},
    "has_node_type_pressure": {"candidate-relation", "evidence-relation"},
    "has_prepared_dossier": {"evidence-relation", "source-relation"},
    "has_relation_pressure": {"candidate-relation", "evidence-relation"},
    "has_section": {"source-relation"},
    "has_view_section": {"source-relation"},
    "influences": {"conceptual-relation", "historical-relation"},
    "institutionalized_in": {"historical-relation"},
    "polemicizes_with": {"conceptual-relation", "evidence-relation"},
    "preserved_in": {"evidence-relation", "transmission-relation"},
    "preserves_in": {"evidence-relation", "transmission-relation"},
    "receives_from": {"transmission-relation"},
    "survives_as": {"evidence-relation", "historical-relation", "transmission-relation"},
    "translated_into": {"transmission-relation"},
    "transforms_concept": {"conceptual-relation"},
    "transmits_to": {"transmission-relation"},
    "uncertain_relation": {"evidence-relation"},
    "uses_language": {"source-relation"},
    "uses_script": {"source-relation"},
}
NODE_TYPE_GRAPH_LAYERS = {
    "atlas": {"source-relation"},
    "atlas-node-type": {"source-relation"},
    "atlas-relation-kind": {"source-relation"},
    "atlas-section": {"source-relation"},
    "candidate-endpoint": {"candidate-relation", "evidence-relation"},
    "candidate-node": {"candidate-relation"},
    "domain-root": {"source-relation"},
    "graph-view": {"source-relation"},
    "master-table": {"source-relation"},
    "master-table-row": {"historical-relation", "source-relation"},
    "prepared-dossier": {"evidence-relation", "source-relation"},
    "view-section": {"source-relation"},
}
KNOWN_GRAPH_LAYERS = {
    "source-relation",
    "historical-relation",
    "conceptual-relation",
    "transmission-relation",
    "evidence-relation",
    "candidate-relation",
    "canonical-relation",
}


def repo_ref(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{repo_ref(path)} must contain a JSON object")
    return payload


def load_schema() -> dict[str, Any]:
    return load_json(REPO_ROOT / SCHEMA_REF)


def validate_payload_schema(payload: dict[str, Any]) -> None:
    validator = Draft202012Validator(load_schema())
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.absolute_path))
    if errors:
        error = errors[0]
        path = "".join(f"[{item}]" if isinstance(item, int) else f".{item}" for item in error.absolute_path)
        raise ValueError(f"schema violation at {path.lstrip('.') or '<root>'}: {error.message}")


def _as_string_set(values: Any) -> set[str]:
    if not isinstance(values, list):
        return set()
    return {str(item) for item in values if isinstance(item, str) and item}


def _node_semantic_layers(node: dict[str, Any]) -> set[str]:
    layers = set(NODE_TYPE_GRAPH_LAYERS.get(str(node.get("node_type") or ""), {"source-relation"}))
    properties = node.get("properties")
    if isinstance(properties, dict):
        if str(properties.get("canon_status") or "") == "pre-canon":
            layers.add("candidate-relation")
        if str(properties.get("authority_posture") or "").endswith("_candidate"):
            layers.add("candidate-relation")
        if str(properties.get("priority") or "").strip():
            layers.add("evidence-relation")
    return layers & KNOWN_GRAPH_LAYERS


def _edge_semantic_layers(edge: dict[str, Any]) -> set[str]:
    predicate = str(edge.get("predicate_id") or "")
    layers = set(PREDICATE_GRAPH_LAYERS.get(predicate, {"source-relation"}))
    properties = edge.get("properties")
    if isinstance(properties, dict):
        if str(properties.get("canon_status") or "") == "pre-canon":
            layers.add("candidate-relation")
        if str(properties.get("authority_posture") or "").endswith("_candidate"):
            layers.add("candidate-relation")
        if str(properties.get("endpoint_resolution") or "") == "unresolved":
            layers.add("evidence-relation")
        if str(properties.get("confidence") or "") in {"низкий", "low"}:
            layers.add("evidence-relation")
    return layers & KNOWN_GRAPH_LAYERS


def _node_matches_filters(node: dict[str, Any], filters: dict[str, Any]) -> bool:
    node_type = str(node.get("node_type") or "")
    label = str(node.get("label") or "")
    properties = node.get("properties")
    if not isinstance(properties, dict):
        properties = {}
    if node_type in _as_string_set(filters.get("node_types")):
        return True
    if node_type == "atlas-node-type" and label in _as_string_set(filters.get("node_type_keys")):
        return True
    if node_type == "atlas-relation-kind" and label in _as_string_set(filters.get("relation_kind_keys")):
        return True
    if node_type == "candidate-node":
        node_type_keys = _as_string_set(filters.get("node_type_keys"))
        if str(properties.get("original_node_type") or "") in node_type_keys:
            return True
    if node_type == "candidate-endpoint":
        return "candidate-endpoint" in _as_string_set(filters.get("node_types"))
    return False


def _edge_matches_filters(edge: dict[str, Any], selected_node_ids: set[str], filters: dict[str, Any]) -> bool:
    predicate = str(edge.get("predicate_id") or "")
    allowed_predicates = _as_string_set(filters.get("predicates")) | _as_string_set(filters.get("relation_kind_keys"))
    if predicate not in allowed_predicates:
        return False
    return str(edge.get("from_id") or "") in selected_node_ids or str(edge.get("to_id") or "") in selected_node_ids


def _pick_view_material(
    view: dict[str, Any],
    atlas_nodes_by_id: dict[str, dict[str, Any]],
    atlas_edges: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, str]]]:
    diagnostics: list[dict[str, str]] = []
    filters = view.get("current_projection_filters")
    if not isinstance(filters, dict):
        raise ValueError(f"{view.get('view_id')}: current_projection_filters must be an object")

    selected_node_ids = {
        str(node.get("node_id"))
        for node in atlas_nodes_by_id.values()
        if _node_matches_filters(node, filters)
    }
    view_edges = [
        edge
        for edge in atlas_edges
        if isinstance(edge, dict) and _edge_matches_filters(edge, selected_node_ids, filters)
    ]
    endpoint_ids = {
        str(edge.get("from_id") or "")
        for edge in view_edges
    } | {
        str(edge.get("to_id") or "")
        for edge in view_edges
    }
    view_node_ids = selected_node_ids | endpoint_ids
    view_nodes = [atlas_nodes_by_id[node_id] for node_id in sorted(view_node_ids) if node_id in atlas_nodes_by_id]
    if not view_nodes:
        diagnostics.append(
            {
                "level": "warning",
                "path": str(view.get("source_ref") or view.get("route_card") or SOURCE_VIEW_CONTRACT_REF),
                "message": "view filters currently select no atlas projection nodes",
            }
        )
    return view_nodes, sorted(view_edges, key=lambda edge: str(edge.get("edge_id") or "")), diagnostics


def _projection_node(
    atlas_node: dict[str, Any],
    *,
    view_ids: set[str],
    graph_layers: set[str],
) -> dict[str, Any]:
    properties = atlas_node.get("properties") if isinstance(atlas_node.get("properties"), dict) else {}
    return {
        "node_id": atlas_node["node_id"],
        "label": atlas_node["label"],
        "multilingual": atlas_node.get("multilingual")
        if isinstance(atlas_node.get("multilingual"), dict)
        else multilingual_label(
            str(atlas_node["label"]),
            str(atlas_node["source_ref"]),
            {"node_type": atlas_node.get("node_type"), **properties},
        ),
        "node_type": atlas_node["node_type"],
        "graph_layers": sorted(graph_layers),
        "view_ids": sorted(view_ids),
        "source_ref": atlas_node["source_ref"],
        "properties": properties,
    }


def _projection_edge(
    atlas_edge: dict[str, Any],
    *,
    view_ids: set[str],
    graph_layers: set[str],
) -> dict[str, Any]:
    return {
        "edge_id": atlas_edge["edge_id"],
        "from_id": atlas_edge["from_id"],
        "to_id": atlas_edge["to_id"],
        "predicate_id": atlas_edge["predicate_id"],
        "graph_layers": sorted(graph_layers),
        "view_ids": sorted(view_ids),
        "source_ref": atlas_edge["source_ref"],
        "properties": atlas_edge.get("properties") if isinstance(atlas_edge.get("properties"), dict) else {},
    }


def _source_refs(nodes: list[dict[str, Any]], edges: list[dict[str, Any]]) -> list[str]:
    refs = {
        str(item.get("source_ref"))
        for item in (*nodes, *edges)
        if isinstance(item.get("source_ref"), str) and item.get("source_ref")
    }
    return sorted(refs)


def _source_refs_from_items(items: list[dict[str, Any]]) -> list[str]:
    refs = {
        str(item.get("source_ref"))
        for item in items
        if isinstance(item.get("source_ref"), str) and item.get("source_ref")
    }
    for item in items:
        if isinstance(item.get("source_refs"), list):
            refs.update(str(ref) for ref in item["source_refs"] if isinstance(ref, str) and ref)
    return sorted(refs)


def _stringify_value(value: Any) -> str | None:
    if value is None:
        return None
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (str, int, float)):
        text = str(value).strip()
        return text or None
    return None


def _stable_digest(value: Any) -> str:
    rendered = json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    return hashlib.sha256(rendered.encode("utf-8")).hexdigest()


def _field_value(node: dict[str, Any], field: str) -> str | None:
    if field == "label":
        return _stringify_value(node.get("label"))
    if field == "node_type":
        return _stringify_value(node.get("node_type"))
    if field == "source_ref":
        return _stringify_value(node.get("source_ref"))
    if field.startswith("properties."):
        properties = node.get("properties")
        if isinstance(properties, dict):
            return _stringify_value(properties.get(field.removeprefix("properties.")))
    return None


def _node_matches_member_key(node: dict[str, Any], member_key: dict[str, Any]) -> bool:
    node_types = member_key.get("node_types")
    if isinstance(node_types, list) and node_types:
        if str(node.get("node_type") or "") not in {str(item) for item in node_types}:
            return False
    value = _field_value(node, str(member_key.get("field") or ""))
    if value is None:
        return False
    allowed_values = member_key.get("allowed_values")
    if isinstance(allowed_values, list) and allowed_values:
        return value in {str(item) for item in allowed_values}
    return True


def _stable_cluster_id(cluster_kind: str, member_key: str, member_value: str) -> str:
    digest = hashlib.sha1(f"{cluster_kind}|{member_key}|{member_value}".encode("utf-8")).hexdigest()[:12]
    return f"cluster:{cluster_kind}:{digest}"


def _build_clusters(
    cluster_contract: dict[str, Any],
    *,
    nodes: list[dict[str, Any]],
    edges: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    if cluster_contract.get("schema_version") != "tos_philosophy_graph_cluster_contracts_v1":
        raise ValueError("cluster-contracts.json must use tos_philosophy_graph_cluster_contracts_v1")
    if cluster_contract.get("projection_ref") != "ToS/derived-exports/philosophy_graph_projection.min.json":
        raise ValueError("cluster-contracts.json must point to the philosophy graph projection export")
    if cluster_contract.get("downstream_consumer") != "abyss-stack":
        raise ValueError("cluster-contracts.json downstream_consumer must be abyss-stack")

    node_by_id = {str(node["node_id"]): node for node in nodes}
    clusters: list[dict[str, Any]] = []
    unresolved: list[dict[str, Any]] = []
    families = cluster_contract.get("cluster_families")
    if not isinstance(families, list):
        raise ValueError("cluster-contracts.json cluster_families must be a list")

    for family in families:
        if not isinstance(family, dict):
            raise ValueError("cluster family entries must be objects")
        cluster_kind = str(family.get("cluster_kind") or "")
        label = str(family.get("label") or "")
        current_member_keys = family.get("current_member_keys")
        future_member_keys = family.get("future_member_keys")
        if not cluster_kind or not label:
            raise ValueError("cluster family must carry cluster_kind and label")
        if not isinstance(current_member_keys, list) or not isinstance(future_member_keys, list):
            raise ValueError(f"{cluster_kind}: current_member_keys and future_member_keys must be lists")

        family_cluster_count = 0
        for member_key in current_member_keys:
            if not isinstance(member_key, dict) or not isinstance(member_key.get("field"), str):
                raise ValueError(f"{cluster_kind}: member key must declare a field")
            field = member_key["field"]
            groups: dict[str, list[dict[str, Any]]] = {}
            for node in nodes:
                if not _node_matches_member_key(node, member_key):
                    continue
                value = _field_value(node, field)
                if value is None:
                    continue
                groups.setdefault(value, []).append(node)

            for member_value, member_nodes in sorted(groups.items()):
                member_node_ids = sorted(str(node["node_id"]) for node in member_nodes)
                member_node_set = set(member_node_ids)
                member_edges = [
                    edge
                    for edge in edges
                    if str(edge.get("from_id")) in member_node_set or str(edge.get("to_id")) in member_node_set
                ]
                cluster_source_refs = _source_refs_from_items([*member_nodes, *member_edges])
                graph_layers = sorted(
                    {
                        str(layer)
                        for item in [*member_nodes, *member_edges]
                        for layer in item.get("graph_layers", [])
                        if isinstance(layer, str) and layer
                    }
                )
                view_ids = sorted(
                    {
                        str(view_id)
                        for item in [*member_nodes, *member_edges]
                        for view_id in item.get("view_ids", [])
                        if isinstance(view_id, str) and view_id
                    }
                )
                cluster_label = f"{label}: {member_value}"
                clusters.append(
                    {
                        "cluster_id": _stable_cluster_id(cluster_kind, field, member_value),
                        "cluster_kind": cluster_kind,
                        "label": cluster_label,
                        "multilingual": multilingual_label(
                            cluster_label,
                            CLUSTER_CONTRACT_REF,
                            {"cluster_kind": cluster_kind, "member_key": field, "member_value": member_value},
                        ),
                        "member_key": field,
                        "member_value": member_value,
                        "member_node_ids": member_node_ids,
                        "member_edge_ids": sorted(str(edge["edge_id"]) for edge in member_edges),
                        "view_ids": view_ids,
                        "graph_layers": graph_layers,
                        "source_ref": CLUSTER_CONTRACT_REF,
                        "source_refs": cluster_source_refs,
                        "properties": {
                            "family_label": label,
                            "review_use": str(family.get("review_use") or ""),
                            "future_member_keys": [str(item) for item in future_member_keys],
                            "member_count": len(member_node_ids),
                            "edge_count": len(member_edges),
                        },
                    }
                )
                family_cluster_count += 1

        if family_cluster_count == 0:
            unresolved.append(
                {
                    "surface_id": f"unresolved-cluster-family:{cluster_kind}",
                    "kind": "unresolved-cluster-family",
                    "cluster_kind": cluster_kind,
                    "source_ref": CLUSTER_CONTRACT_REF,
                    "message": "current projection has no source-owned member key coverage for this cluster family yet",
                    "future_member_keys": [str(item) for item in future_member_keys],
                }
            )

    return sorted(clusters, key=lambda cluster: cluster["cluster_id"]), unresolved


def _layer_counts(
    graph_layers: list[dict[str, Any]],
    *,
    views: list[dict[str, Any]],
    nodes: list[dict[str, Any]],
    edges: list[dict[str, Any]],
    clusters: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    counts: list[dict[str, Any]] = []
    for layer in graph_layers:
        layer_id = str(layer.get("layer_id") or "")
        layer_nodes = [node for node in nodes if layer_id in node.get("graph_layers", [])]
        layer_edges = [edge for edge in edges if layer_id in edge.get("graph_layers", [])]
        layer_views = [view for view in views if layer_id in view.get("graph_layers", [])]
        layer_clusters = [cluster for cluster in clusters if layer_id in cluster.get("graph_layers", [])]
        counts.append(
            {
                "layer_id": layer_id,
                "node_count": len(layer_nodes),
                "edge_count": len(layer_edges),
                "view_count": len(layer_views),
                "cluster_count": len(layer_clusters),
                "source_ref_count": len(_source_refs_from_items([*layer_nodes, *layer_edges, *layer_clusters])),
            }
        )
    return counts


def _view_layer_counts(view: dict[str, Any], graph_layers: list[dict[str, Any]], clusters: list[dict[str, Any]]) -> list[dict[str, Any]]:
    view_id = str(view.get("view_id") or "")
    rows: list[dict[str, Any]] = []
    for layer in graph_layers:
        layer_id = str(layer.get("layer_id") or "")
        view_nodes = [node for node in view.get("nodes", []) if layer_id in node.get("graph_layers", [])]
        view_edges = [edge for edge in view.get("edges", []) if layer_id in edge.get("graph_layers", [])]
        view_clusters = [
            cluster
            for cluster in clusters
            if view_id in cluster.get("view_ids", []) and layer_id in cluster.get("graph_layers", [])
        ]
        rows.append(
            {
                "layer_id": layer_id,
                "node_count": len(view_nodes),
                "edge_count": len(view_edges),
                "cluster_count": len(view_clusters),
                "source_ref_count": len(_source_refs_from_items([*view_nodes, *view_edges, *view_clusters])),
            }
        )
    return rows


def _candidate_to_canon_pressure(nodes: list[dict[str, Any]]) -> dict[str, int]:
    counter: Counter[str] = Counter()
    for node in nodes:
        properties = node.get("properties")
        if node.get("node_type") == "master-table-row" and isinstance(properties, dict):
            status = _stringify_value(properties.get("status")) or "missing"
            counter[status] += 1
        if node.get("node_type") == "candidate-node" and isinstance(properties, dict):
            status = _stringify_value(properties.get("canon_status")) or "pre-canon"
            counter[status] += 1
    return dict(sorted(counter.items()))


def _degree_counts(edges: list[dict[str, Any]]) -> Counter[str]:
    degrees: Counter[str] = Counter()
    for edge in edges:
        degrees[str(edge.get("from_id") or "")] += 1
        degrees[str(edge.get("to_id") or "")] += 1
    degrees.pop("", None)
    return degrees


def _build_review_packets(
    *,
    views: list[dict[str, Any]],
    graph_layers: list[dict[str, Any]],
    clusters: list[dict[str, Any]],
    unresolved_review_surfaces: list[dict[str, Any]],
    review_packet_contract: dict[str, Any],
) -> list[dict[str, Any]]:
    if review_packet_contract.get("schema_version") != "tos_philosophy_graph_review_packet_contract_v1":
        raise ValueError("review-packet-contract.json must use tos_philosophy_graph_review_packet_contract_v1")
    if review_packet_contract.get("projection_ref") != "ToS/derived-exports/philosophy_graph_projection.min.json":
        raise ValueError("review-packet-contract.json must point to the philosophy graph projection export")
    if review_packet_contract.get("downstream_consumer") != "abyss-stack":
        raise ValueError("review-packet-contract.json downstream_consumer must be abyss-stack")

    limits = review_packet_contract.get("default_limits")
    if not isinstance(limits, dict):
        raise ValueError("review-packet-contract.json default_limits must be an object")
    dense_threshold = int(review_packet_contract.get("dense_hub_degree_threshold") or 24)
    changed_subgraph = review_packet_contract.get("changed_subgraph")
    if not isinstance(changed_subgraph, dict):
        changed_subgraph = {"available": False, "reason": "not declared"}

    packets: list[dict[str, Any]] = []
    clusters_by_view: dict[str, list[dict[str, Any]]] = {}
    for cluster in clusters:
        for view_id in cluster.get("view_ids", []):
            clusters_by_view.setdefault(str(view_id), []).append(cluster)

    for view in views:
        view_id = str(view["view_id"])
        view_nodes = view.get("nodes", [])
        view_edges = view.get("edges", [])
        view_clusters = clusters_by_view.get(view_id, [])
        degrees = _degree_counts(view_edges)
        node_by_id = {str(node["node_id"]): node for node in view_nodes}
        dense_hubs = [
            {
                "node_id": node_id,
                "label": str(node_by_id.get(node_id, {}).get("label") or node_id),
                "degree": degree,
                "source_ref": str(node_by_id.get(node_id, {}).get("source_ref") or ""),
            }
            for node_id, degree in degrees.most_common()
            if degree >= dense_threshold and node_id in node_by_id
        ][: int(limits.get("dense_hubs") or 12)]
        isolated_nodes = [
            {
                "node_id": str(node["node_id"]),
                "label": str(node.get("label") or node["node_id"]),
                "source_ref": str(node.get("source_ref") or ""),
            }
            for node in view_nodes
            if degrees.get(str(node["node_id"]), 0) == 0
        ][: int(limits.get("isolated_nodes") or 20)]
        default_cluster_kinds = set(view.get("collapse_rule", {}).get("default_cluster_kinds", []))
        unresolved_for_view = [
            surface
            for surface in unresolved_review_surfaces
            if surface.get("cluster_kind") in default_cluster_kinds
        ][: int(limits.get("unresolved_diagnostics") or 20)]
        weak_source_refs = [
            {"item_id": str(item.get("node_id") or item.get("edge_id") or item.get("cluster_id")), "item_type": kind}
            for kind, material in (("node", view_nodes), ("edge", view_edges), ("cluster", view_clusters))
            for item in material
            if not item.get("source_ref") and not item.get("source_refs")
        ][: int(limits.get("weak_source_refs") or 20)]
        cluster_summaries = [
            {
                "cluster_id": str(cluster["cluster_id"]),
                "cluster_kind": str(cluster["cluster_kind"]),
                "label": str(cluster["label"]),
                "node_count": len(cluster.get("member_node_ids", [])),
                "edge_count": len(cluster.get("member_edge_ids", [])),
                "source_ref_count": len(cluster.get("source_refs", [])),
            }
            for cluster in sorted(
                view_clusters,
                key=lambda cluster: (
                    str(cluster.get("cluster_kind") or ""),
                    -len(cluster.get("member_node_ids", [])),
                    str(cluster.get("label") or ""),
                ),
            )
        ][: int(limits.get("cluster_summaries") or 12)]
        current_view_fingerprint = _stable_digest(
            {
                "view_id": view_id,
                "node_ids": sorted(str(node["node_id"]) for node in view_nodes),
                "edge_ids": sorted(str(edge["edge_id"]) for edge in view_edges),
                "cluster_ids": sorted(str(cluster["cluster_id"]) for cluster in view_clusters),
                "graph_layers": view.get("graph_layers", []),
                "source_refs": view.get("source_refs", []),
            }
        )
        changed_subgraph_packet = dict(changed_subgraph)
        changed_subgraph_packet.update(
            {
                "snapshot_mode": "current-view-fingerprint",
                "current_view_fingerprint": current_view_fingerprint,
            }
        )
        packets.append(
            {
                "packet_id": f"review-packet:{view_id}",
                "view_id": view_id,
                "review_intent": str(view.get("review_intent") or ""),
                "active_filters": view["filters_applied"],
                "counts": {
                    "nodes": len(view_nodes),
                    "edges": len(view_edges),
                    "source_refs": len(view.get("source_refs", [])),
                    "clusters": len(view_clusters),
                    "weak_source_refs": len(weak_source_refs),
                    "unresolved_diagnostics": len(unresolved_for_view),
                    "suspicious_dense_hubs": len(dense_hubs),
                    "isolated_nodes": len(isolated_nodes),
                },
                "layer_counts": _view_layer_counts(view, graph_layers, view_clusters),
                "cluster_summaries": cluster_summaries,
                "weak_source_refs": weak_source_refs,
                "unresolved_diagnostics": unresolved_for_view,
                "suspicious_dense_hubs": dense_hubs,
                "isolated_nodes": isolated_nodes,
                "candidate_to_canon_pressure": _candidate_to_canon_pressure(view_nodes),
                "changed_subgraph": changed_subgraph_packet,
                "recommended_human_review_route": str(view.get("route_card") or ""),
                "source_refs": view.get("source_refs", []),
            }
        )
    return packets


def _build_snapshot_review(
    *,
    views: list[dict[str, Any]],
    nodes: list[dict[str, Any]],
    edges: list[dict[str, Any]],
    clusters: list[dict[str, Any]],
) -> dict[str, Any]:
    clusters_by_view: dict[str, list[dict[str, Any]]] = {}
    for cluster in clusters:
        for view_id in cluster.get("view_ids", []):
            clusters_by_view.setdefault(str(view_id), []).append(cluster)

    view_fingerprints: list[dict[str, Any]] = []
    for view in views:
        view_id = str(view.get("view_id") or "")
        view_nodes = view.get("nodes", [])
        view_edges = view.get("edges", [])
        view_clusters = clusters_by_view.get(view_id, [])
        view_fingerprint_material = {
            "view_id": view_id,
            "node_ids": sorted(str(node["node_id"]) for node in view_nodes),
            "edge_ids": sorted(str(edge["edge_id"]) for edge in view_edges),
            "cluster_ids": sorted(str(cluster["cluster_id"]) for cluster in view_clusters),
            "graph_layers": view.get("graph_layers", []),
            "source_refs": view.get("source_refs", []),
        }
        view_fingerprints.append(
            {
                "view_id": view_id,
                "fingerprint": _stable_digest(view_fingerprint_material),
                "node_count": len(view_nodes),
                "edge_count": len(view_edges),
                "cluster_count": len(view_clusters),
                "source_ref_count": len(view.get("source_refs", [])),
            }
        )

    projection_material = {
        "node_ids": sorted(str(node["node_id"]) for node in nodes),
        "edge_ids": sorted(str(edge["edge_id"]) for edge in edges),
        "cluster_ids": sorted(str(cluster["cluster_id"]) for cluster in clusters),
        "view_fingerprints": view_fingerprints,
    }
    count_material = {
        "views": len(views),
        "nodes": len(nodes),
        "edges": len(edges),
        "clusters": len(clusters),
    }
    return {
        "snapshot_schema_version": "tos_philosophy_graph_projection_snapshot_v1",
        "current_snapshot": {
            "projection_fingerprint": _stable_digest(projection_material),
            "count_fingerprint": _stable_digest(count_material),
            "view_fingerprints": view_fingerprints,
        },
        "diff_route": {
            "mode": "fingerprint-ready",
            "changed_subgraph_available": False,
            "previous_snapshot_ref": None,
            "next_route": "compare current_snapshot against a previous reviewed philosophy graph projection snapshot",
        },
    }


def _validate_cross_references(payload: dict[str, Any]) -> None:
    graph_layer_ids = {str(layer.get("layer_id")) for layer in payload.get("graph_layers", []) if isinstance(layer, dict)}
    view_ids = {str(view.get("view_id")) for view in payload.get("views", []) if isinstance(view, dict)}
    node_ids = {str(node.get("node_id")) for node in payload.get("nodes", []) if isinstance(node, dict)}
    edge_ids = {str(edge.get("edge_id")) for edge in payload.get("edges", []) if isinstance(edge, dict)}
    for node in payload.get("nodes", []):
        if not isinstance(node, dict):
            continue
        if not node.get("source_ref"):
            raise ValueError(f"{node.get('node_id')}: node must preserve source_ref")
        unknown_layers = set(node.get("graph_layers", [])) - graph_layer_ids
        if unknown_layers:
            raise ValueError(f"{node.get('node_id')}: node references unknown graph layers: {sorted(unknown_layers)}")
    for edge in payload.get("edges", []):
        if not isinstance(edge, dict):
            continue
        if edge.get("from_id") not in node_ids or edge.get("to_id") not in node_ids:
            raise ValueError(f"{edge.get('edge_id')}: edge endpoints must exist in projection nodes")
        if not edge.get("source_ref"):
            raise ValueError(f"{edge.get('edge_id')}: edge must preserve source_ref")
        unknown_layers = set(edge.get("graph_layers", [])) - graph_layer_ids
        if unknown_layers:
            raise ValueError(f"{edge.get('edge_id')}: edge references unknown graph layers: {sorted(unknown_layers)}")
    for view in payload.get("views", []):
        if not isinstance(view, dict):
            continue
        unknown_layers = set(view.get("graph_layers", [])) - graph_layer_ids
        if unknown_layers:
            raise ValueError(f"{view.get('view_id')}: view references unknown graph layers: {sorted(unknown_layers)}")
        if not view.get("nodes") and not view.get("diagnostics"):
            raise ValueError(f"{view.get('view_id')}: empty view must carry a diagnostic")
    for cluster in payload.get("clusters", []):
        if not isinstance(cluster, dict):
            continue
        unknown_nodes = set(cluster.get("member_node_ids", [])) - node_ids
        unknown_edges = set(cluster.get("member_edge_ids", [])) - edge_ids
        unknown_layers = set(cluster.get("graph_layers", [])) - graph_layer_ids
        unknown_views = set(cluster.get("view_ids", [])) - view_ids
        if unknown_nodes or unknown_edges or unknown_layers or unknown_views:
            raise ValueError(f"{cluster.get('cluster_id')}: cluster references unknown graph material")
        if not cluster.get("source_refs"):
            raise ValueError(f"{cluster.get('cluster_id')}: cluster must preserve source_refs")
    for packet in payload.get("review_packets", []):
        if not isinstance(packet, dict):
            continue
        if packet.get("view_id") not in view_ids:
            raise ValueError(f"{packet.get('packet_id')}: review packet references an unknown view")
    if payload.get("runtime_projection_boundary", {}).get("runtime_owner") != "abyss-stack":
        raise ValueError("philosophy graph projection must keep runtime_owner in abyss-stack")


def build_payload() -> dict[str, Any]:
    atlas_projection = load_json(REPO_ROOT / ATLAS_PROJECTION_REF)
    graph_view_catalog = load_json(REPO_ROOT / GRAPH_VIEW_CATALOG_REF)
    view_contract = load_json(REPO_ROOT / SOURCE_VIEW_CONTRACT_REF)
    cluster_contract = load_json(REPO_ROOT / CLUSTER_CONTRACT_REF)
    review_packet_contract = load_json(REPO_ROOT / REVIEW_PACKET_CONTRACT_REF)
    if graph_view_catalog.get("atlas_projection_ref") != ATLAS_PROJECTION_REF:
        raise ValueError(f"{GRAPH_VIEW_CATALOG_REF} must point to {ATLAS_PROJECTION_REF}")
    if view_contract.get("atlas_projection_ref") != ATLAS_PROJECTION_REF:
        raise ValueError(f"{SOURCE_VIEW_CONTRACT_REF} must point to {ATLAS_PROJECTION_REF}")
    if graph_view_catalog.get("lens_review_contract_ref") != LENS_REVIEW_CONTRACT_REF:
        raise ValueError(f"{GRAPH_VIEW_CATALOG_REF} must point to {LENS_REVIEW_CONTRACT_REF}")

    atlas_nodes = atlas_projection.get("nodes")
    atlas_edges = atlas_projection.get("edges")
    if not isinstance(atlas_nodes, list) or not isinstance(atlas_edges, list):
        raise ValueError(f"{ATLAS_PROJECTION_REF} must expose nodes and edges")
    atlas_nodes_by_id = {
        str(node.get("node_id")): node
        for node in atlas_nodes
        if isinstance(node, dict) and isinstance(node.get("node_id"), str)
    }

    node_membership: dict[str, dict[str, set[str]]] = {}
    edge_membership: dict[str, dict[str, set[str]]] = {}
    views: list[dict[str, Any]] = []
    diagnostics: list[dict[str, str]] = []

    raw_views = graph_view_catalog.get("views")
    if not isinstance(raw_views, list):
        raise ValueError(f"{GRAPH_VIEW_CATALOG_REF} views must be a list")

    for view in raw_views:
        if not isinstance(view, dict):
            raise ValueError(f"{GRAPH_VIEW_CATALOG_REF} views entries must be objects")
        view_id = str(view.get("view_id") or "")
        graph_layers = _as_string_set(view.get("graph_layers"))
        view_nodes, view_edges, view_diagnostics = _pick_view_material(view, atlas_nodes_by_id, atlas_edges)
        view_node_layers: dict[str, set[str]] = {
            str(node["node_id"]): _node_semantic_layers(node)
            for node in view_nodes
        }
        view_edge_layers: dict[str, set[str]] = {}
        for edge in view_edges:
            edge_id = str(edge["edge_id"])
            edge_layers = _edge_semantic_layers(edge)
            view_edge_layers[edge_id] = edge_layers
            for endpoint_key in ("from_id", "to_id"):
                endpoint_id = str(edge.get(endpoint_key) or "")
                if endpoint_id in view_node_layers:
                    view_node_layers[endpoint_id].update(edge_layers)
        for node in view_nodes:
            node_id = str(node["node_id"])
            membership = node_membership.setdefault(node_id, {"view_ids": set(), "graph_layers": set()})
            membership["view_ids"].add(view_id)
            membership["graph_layers"].update(view_node_layers.get(node_id) or _node_semantic_layers(node))
        for edge in view_edges:
            edge_id = str(edge["edge_id"])
            membership = edge_membership.setdefault(edge_id, {"view_ids": set(), "graph_layers": set()})
            membership["view_ids"].add(view_id)
            membership["graph_layers"].update(view_edge_layers.get(edge_id) or _edge_semantic_layers(edge))
        diagnostics.extend(view_diagnostics)
        views.append(
            {
                "view_id": view_id,
                "title": view["title"],
                "source_ref": view["source_ref"],
                "route_card": view["route_card"],
                "order": view["order"],
                "layout_hint": view["layout_hint"],
                "graph_layers": sorted(graph_layers),
                "filters_applied": view["current_projection_filters"],
                "future_branch_filters": view["future_branch_filters"],
                "review_intent": view["review_intent"],
                "source_posture": view["source_posture"],
                "evidence_posture": view["evidence_posture"],
                "collapse_rule": view["collapse_rule"],
                "ordering_hints": view["ordering_hints"],
                "agent_packet_hint": view["agent_packet_hint"],
                "nodes": [
                    _projection_node(
                        node,
                        view_ids={view_id},
                        graph_layers=view_node_layers.get(str(node["node_id"])) or _node_semantic_layers(node),
                    )
                    for node in view_nodes
                ],
                "edges": [
                    _projection_edge(
                        edge,
                        view_ids={view_id},
                        graph_layers=view_edge_layers.get(str(edge["edge_id"])) or _edge_semantic_layers(edge),
                    )
                    for edge in view_edges
                ],
                "source_refs": _source_refs(view_nodes, view_edges),
                "diagnostics": view_diagnostics,
            }
        )

    edge_by_id = {
        str(edge.get("edge_id")): edge
        for edge in atlas_edges
        if isinstance(edge, dict) and isinstance(edge.get("edge_id"), str)
    }
    nodes = [
        _projection_node(
            atlas_nodes_by_id[node_id],
            view_ids=membership["view_ids"],
            graph_layers=membership["graph_layers"],
        )
        for node_id, membership in sorted(node_membership.items())
    ]
    edges = [
        _projection_edge(
            edge_by_id[edge_id],
            view_ids=membership["view_ids"],
            graph_layers=membership["graph_layers"],
        )
        for edge_id, membership in sorted(edge_membership.items())
    ]
    clusters, unresolved_review_surfaces = _build_clusters(cluster_contract, nodes=nodes, edges=edges)
    graph_layers = graph_view_catalog.get("graph_layers", [])
    if not isinstance(graph_layers, list):
        raise ValueError(f"{GRAPH_VIEW_CATALOG_REF} graph_layers must be a list")
    layer_counts = _layer_counts(graph_layers, views=views, nodes=nodes, edges=edges, clusters=clusters)
    review_packets = _build_review_packets(
        views=views,
        graph_layers=graph_layers,
        clusters=clusters,
        unresolved_review_surfaces=unresolved_review_surfaces,
        review_packet_contract=review_packet_contract,
    )
    snapshot_review = _build_snapshot_review(views=views, nodes=nodes, edges=edges, clusters=clusters)
    cluster_contract_rules = cluster_contract.get("collapse_rules")
    if not isinstance(cluster_contract_rules, dict):
        raise ValueError("cluster-contracts.json collapse_rules must be an object")

    payload: dict[str, Any] = {
        "schema_version": "tos_philosophy_graph_projection_v1",
        "schema_ref": SCHEMA_REF,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_philosophy_graph_projection",
        "source_refs": {
            "atlas_projection_ref": ATLAS_PROJECTION_REF,
            "graph_view_catalog_ref": GRAPH_VIEW_CATALOG_REF,
            "source_view_contract_ref": SOURCE_VIEW_CONTRACT_REF,
            "lens_review_contract_ref": LENS_REVIEW_CONTRACT_REF,
            "cluster_contract_ref": CLUSTER_CONTRACT_REF,
            "review_packet_contract_ref": REVIEW_PACKET_CONTRACT_REF,
        },
        "content_language_contract": content_language_contract(),
        "runtime_projection_boundary": {
            "runtime_owner": "abyss-stack",
            "runtime_scope": [
                "serve this projection through API, MCP, UI, layout, and cache behavior",
                "materialize this projection into Neo4j as a rebuildable cache",
                "render switchable graph lenses without writing runtime state back into ToS",
            ],
            "tos_authority_scope": [
                "atlas projection and graph-view catalog remain the source-owned inputs",
                "this projection is generated and reproducible, not canon",
                "source_ref fields route every projected node and edge back to ToS-owned surfaces",
            ],
        },
        "validation_refs": list(VALIDATION_REFS),
        "counts": {
            "views": len(views),
            "graph_layers": len(graph_view_catalog.get("graph_layers", [])),
            "nodes": len(nodes),
            "edges": len(edges),
            "source_refs": len(_source_refs(nodes, edges)),
            "diagnostics": len(diagnostics),
            "clusters": len(clusters),
            "review_packets": len(review_packets),
            "unresolved_review_surfaces": len(unresolved_review_surfaces),
        },
        "visibility_model": {
            "default_payload_mode": "cluster-first",
            "default_depth": 1,
            "default_limit": int(cluster_contract_rules.get("runtime_payload_limit") or 200),
            "layer_ids": [str(layer["layer_id"]) for layer in graph_layers if isinstance(layer, dict)],
            "expand_returns": cluster_contract_rules.get("expand_returns", []),
            "cluster_contract_ref": CLUSTER_CONTRACT_REF,
            "review_packet_contract_ref": REVIEW_PACKET_CONTRACT_REF,
            "lens_review_contract_ref": LENS_REVIEW_CONTRACT_REF,
        },
        "snapshot_review": snapshot_review,
        "graph_layers": graph_layers,
        "layer_counts": layer_counts,
        "views": views,
        "nodes": nodes,
        "edges": edges,
        "clusters": clusters,
        "review_packets": review_packets,
        "unresolved_review_surfaces": unresolved_review_surfaces,
        "diagnostics": diagnostics,
    }
    validate_payload_schema(payload)
    _validate_cross_references(payload)
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"

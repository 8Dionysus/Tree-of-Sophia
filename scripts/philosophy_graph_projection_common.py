#!/usr/bin/env python3
"""Shared helpers for the ToS philosophy graph projection export."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
TOS_ROOT = REPO_ROOT / "ToS"
ATLAS_PROJECTION_REF = "ToS/derived-exports/philosophy_atlas_projection.min.json"
GRAPH_VIEW_CATALOG_REF = "ToS/derived-exports/philosophy_graph_views.min.json"
SOURCE_VIEW_CONTRACT_REF = "ToS/philosophy/graph-workbench/views/view-contracts.json"
GRAPH_PROJECTION_PATH = TOS_ROOT / "derived-exports" / "philosophy_graph_projection.min.json"
SCHEMA_REF = "ToS/contracts/philosophy-graph-projection.schema.json"
VALIDATION_REFS = (
    "scripts/build_philosophy_graph_projection.py",
    "scripts/validate_philosophy_graph_projection.py",
    "tests/test_philosophy_graph_projection.py",
)


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


def _node_matches_filters(node: dict[str, Any], filters: dict[str, Any]) -> bool:
    node_type = str(node.get("node_type") or "")
    label = str(node.get("label") or "")
    if node_type in _as_string_set(filters.get("node_types")):
        return True
    if node_type == "atlas-node-type" and label in _as_string_set(filters.get("node_type_keys")):
        return True
    if node_type == "atlas-relation-kind" and label in _as_string_set(filters.get("relation_kind_keys")):
        return True
    return False


def _edge_matches_filters(edge: dict[str, Any], selected_node_ids: set[str], filters: dict[str, Any]) -> bool:
    predicate = str(edge.get("predicate_id") or "")
    if predicate not in _as_string_set(filters.get("predicates")):
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
    return {
        "node_id": atlas_node["node_id"],
        "label": atlas_node["label"],
        "node_type": atlas_node["node_type"],
        "graph_layers": sorted(graph_layers),
        "view_ids": sorted(view_ids),
        "source_ref": atlas_node["source_ref"],
        "properties": atlas_node.get("properties") if isinstance(atlas_node.get("properties"), dict) else {},
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


def _validate_cross_references(payload: dict[str, Any]) -> None:
    graph_layer_ids = {str(layer.get("layer_id")) for layer in payload.get("graph_layers", []) if isinstance(layer, dict)}
    node_ids = {str(node.get("node_id")) for node in payload.get("nodes", []) if isinstance(node, dict)}
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
    if payload.get("runtime_projection_boundary", {}).get("runtime_owner") != "abyss-stack":
        raise ValueError("philosophy graph projection must keep runtime_owner in abyss-stack")


def build_payload() -> dict[str, Any]:
    atlas_projection = load_json(REPO_ROOT / ATLAS_PROJECTION_REF)
    graph_view_catalog = load_json(REPO_ROOT / GRAPH_VIEW_CATALOG_REF)
    view_contract = load_json(REPO_ROOT / SOURCE_VIEW_CONTRACT_REF)
    if graph_view_catalog.get("atlas_projection_ref") != ATLAS_PROJECTION_REF:
        raise ValueError(f"{GRAPH_VIEW_CATALOG_REF} must point to {ATLAS_PROJECTION_REF}")
    if view_contract.get("atlas_projection_ref") != ATLAS_PROJECTION_REF:
        raise ValueError(f"{SOURCE_VIEW_CONTRACT_REF} must point to {ATLAS_PROJECTION_REF}")

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
        for node in view_nodes:
            node_id = str(node["node_id"])
            membership = node_membership.setdefault(node_id, {"view_ids": set(), "graph_layers": set()})
            membership["view_ids"].add(view_id)
            membership["graph_layers"].update(graph_layers)
        for edge in view_edges:
            edge_id = str(edge["edge_id"])
            membership = edge_membership.setdefault(edge_id, {"view_ids": set(), "graph_layers": set()})
            membership["view_ids"].add(view_id)
            membership["graph_layers"].update(graph_layers)
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
                "nodes": [
                    _projection_node(
                        node,
                        view_ids={view_id},
                        graph_layers=graph_layers,
                    )
                    for node in view_nodes
                ],
                "edges": [
                    _projection_edge(
                        edge,
                        view_ids={view_id},
                        graph_layers=graph_layers,
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

    payload: dict[str, Any] = {
        "schema_version": "tos_philosophy_graph_projection_v1",
        "schema_ref": SCHEMA_REF,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_philosophy_graph_projection",
        "source_refs": {
            "atlas_projection_ref": ATLAS_PROJECTION_REF,
            "graph_view_catalog_ref": GRAPH_VIEW_CATALOG_REF,
            "source_view_contract_ref": SOURCE_VIEW_CONTRACT_REF,
        },
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
        },
        "graph_layers": graph_view_catalog.get("graph_layers", []),
        "views": views,
        "nodes": nodes,
        "edges": edges,
        "diagnostics": diagnostics,
    }
    validate_payload_schema(payload)
    _validate_cross_references(payload)
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"

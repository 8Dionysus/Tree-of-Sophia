#!/usr/bin/env python3
"""Shared helpers for the ToS philosophy graph view catalog export."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
TOS_ROOT = REPO_ROOT / "ToS"
SOURCE_VIEW_CONTRACT_REF = "ToS/philosophy/graph-workbench/views/view-contracts.json"
LENS_REVIEW_CONTRACT_REF = "ToS/philosophy/graph-workbench/views/lens-review-contracts.json"
SOURCE_VIEW_ROOT = "ToS/philosophy/graph-workbench/views"
GRAPH_LAYER_SOURCE_REF = "ToS/philosophy/trunk/graph-layers/README.md"
ATLAS_PROJECTION_REF = "ToS/derived-exports/philosophy_atlas_projection.min.json"
GRAPH_VIEW_CATALOG_PATH = TOS_ROOT / "derived-exports" / "philosophy_graph_views.min.json"
SCHEMA_REF = "ToS/contracts/philosophy-graph-views.schema.json"
VALIDATION_REFS = (
    "scripts/build_philosophy_graph_views.py",
    "scripts/validate_philosophy_graph_views.py",
    "tests/test_philosophy_graph_views.py",
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


def section_lines(markdown: str, heading: str) -> list[str]:
    lines = markdown.splitlines()
    start: int | None = None
    for index, line in enumerate(lines):
        if line.strip() == f"## {heading}":
            start = index + 1
            break
    if start is None:
        raise ValueError(f"markdown card is missing ## {heading}")
    section: list[str] = []
    for line in lines[start:]:
        if line.startswith("## "):
            break
        section.append(line)
    return section


def paragraph_from_section(lines: list[str]) -> str:
    parts = [line.strip() for line in lines if line.strip()]
    return " ".join(parts)


def bullets_from_section(lines: list[str]) -> list[str]:
    bullets: list[str] = []
    current: list[str] = []
    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("- "):
            if current:
                bullets.append(" ".join(current))
            current = [stripped[2:].strip()]
            continue
        if current:
            current.append(stripped)
    if current:
        bullets.append(" ".join(current))
    return bullets


def parse_view_card(path: Path) -> dict[str, Any]:
    markdown = path.read_text(encoding="utf-8")
    title_match = re.search(r"^#\s+(.+)$", markdown, flags=re.MULTILINE)
    if not title_match:
        raise ValueError(f"{repo_ref(path)} is missing an H1 title")
    lens = paragraph_from_section(section_lines(markdown, "Lens"))
    future_inputs = bullets_from_section(section_lines(markdown, "Future Inputs"))
    boundary = paragraph_from_section(section_lines(markdown, "Boundary"))
    if not future_inputs:
        raise ValueError(f"{repo_ref(path)} must list future inputs")
    return {
        "title": title_match.group(1).strip(),
        "lens": lens,
        "future_inputs": future_inputs,
        "boundary": boundary,
    }


def parse_graph_layers(path: Path) -> list[dict[str, str]]:
    layers: list[dict[str, str]] = []
    pattern = re.compile(r"^\|\s*`([^`]+)`\s*\|\s*(.*?)\s*\|\s*$")
    for line in path.read_text(encoding="utf-8").splitlines():
        match = pattern.match(line)
        if not match:
            continue
        layers.append(
            {
                "layer_id": match.group(1),
                "use": match.group(2).strip(),
                "source_ref": repo_ref(path),
            }
        )
    if not layers:
        raise ValueError(f"{repo_ref(path)} must declare graph layers")
    return layers


def projection_sets(payload: dict[str, Any]) -> dict[str, set[str]]:
    nodes = payload.get("nodes", [])
    edges = payload.get("edges", [])
    if not isinstance(nodes, list) or not isinstance(edges, list):
        raise ValueError(f"{ATLAS_PROJECTION_REF} must expose nodes and edges")

    row_fields: set[str] = set()
    for node in nodes:
        if not isinstance(node, dict) or node.get("node_type") != "master-table-row":
            continue
        properties = node.get("properties")
        if isinstance(properties, dict):
            row_fields.update(str(key) for key in properties)

    return {
        "node_types": {
            str(node.get("node_type"))
            for node in nodes
            if isinstance(node, dict) and isinstance(node.get("node_type"), str)
        },
        "predicates": {
            str(edge.get("predicate_id"))
            for edge in edges
            if isinstance(edge, dict) and isinstance(edge.get("predicate_id"), str)
        },
        "row_fields": row_fields,
        "node_type_keys": {
            str(node.get("label"))
            for node in nodes
            if isinstance(node, dict)
            and node.get("node_type") == "atlas-node-type"
            and isinstance(node.get("label"), str)
        },
        "relation_kind_keys": {
            str(node.get("label"))
            for node in nodes
            if isinstance(node, dict)
            and node.get("node_type") == "atlas-relation-kind"
            and isinstance(node.get("label"), str)
        },
        "graph_view_ids": {
            str(node.get("node_id")).removeprefix("graph-view:")
            for node in nodes
            if isinstance(node, dict)
            and node.get("node_type") == "graph-view"
            and isinstance(node.get("node_id"), str)
        },
    }


def require_string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        raise ValueError(f"{label} must be a list of non-empty strings")
    return value


def require_subset(values: list[str], allowed: set[str], label: str) -> None:
    missing = sorted(set(values) - allowed)
    if missing:
        raise ValueError(f"{label} references unknown values: {', '.join(missing)}")


def validate_view_contract(
    view: dict[str, Any],
    *,
    graph_layer_ids: set[str],
    projection: dict[str, set[str]],
    manifest_view_routes: set[str],
) -> None:
    view_id = view.get("view_id")
    route_card = view.get("route_card")
    if not isinstance(view_id, str) or not view_id:
        raise ValueError("view_id must be a non-empty string")
    if not isinstance(route_card, str) or not route_card:
        raise ValueError(f"{view_id}: route_card must be a non-empty string")
    if route_card not in manifest_view_routes:
        raise ValueError(f"{view_id}: route_card is not listed in ToS/philosophy/philosophy.manifest.json")
    expected_route = f"{SOURCE_VIEW_ROOT}/{view_id}.graph.md"
    if route_card != expected_route:
        raise ValueError(f"{view_id}: route_card must be {expected_route}")
    if not (REPO_ROOT / route_card).is_file():
        raise ValueError(f"{view_id}: route_card is missing")
    if not isinstance(view.get("order"), int):
        raise ValueError(f"{view_id}: order must be an integer")

    layers = require_string_list(view.get("graph_layers"), f"{view_id}.graph_layers")
    require_subset(layers, graph_layer_ids, f"{view_id}.graph_layers")

    filters = view.get("current_projection_filters")
    if not isinstance(filters, dict):
        raise ValueError(f"{view_id}.current_projection_filters must be an object")
    for key in ("node_types", "predicates", "row_fields", "node_type_keys", "relation_kind_keys"):
        values = require_string_list(filters.get(key), f"{view_id}.current_projection_filters.{key}")
        require_subset(values, projection[key], f"{view_id}.current_projection_filters.{key}")

    future_filters = view.get("future_branch_filters")
    if not isinstance(future_filters, dict):
        raise ValueError(f"{view_id}.future_branch_filters must be an object")
    for key in ("node_kinds", "relation_kinds"):
        require_string_list(future_filters.get(key), f"{view_id}.future_branch_filters.{key}")

    require_string_list(view.get("group_by"), f"{view_id}.group_by")
    sort_fields = require_string_list(view.get("sort_fields"), f"{view_id}.sort_fields")
    allowed_sort_fields = projection["row_fields"] | {"relation_kind", "node_type_key"}
    require_subset(sort_fields, allowed_sort_fields, f"{view_id}.sort_fields")
    if not isinstance(view.get("layout_hint"), str) or not view["layout_hint"]:
        raise ValueError(f"{view_id}.layout_hint must be a non-empty string")


def require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be an object")
    return value


def validate_lens_review_contract(contract: dict[str, Any], view_ids: set[str]) -> dict[str, dict[str, Any]]:
    if contract.get("schema_version") != "tos_philosophy_lens_review_contracts_v1":
        raise ValueError("lens-review-contracts.json must use tos_philosophy_lens_review_contracts_v1")
    if contract.get("view_contract_ref") != SOURCE_VIEW_CONTRACT_REF:
        raise ValueError(f"lens-review-contracts.json must point to {SOURCE_VIEW_CONTRACT_REF}")
    if contract.get("downstream_consumer") != "abyss-stack":
        raise ValueError("lens-review-contracts.json downstream_consumer must be abyss-stack")

    default_requirements = require_object(contract.get("default_requirements"), "lens-review default_requirements")
    require_string_list(
        default_requirements.get("minimum_source_ref_expectations"),
        "lens-review default_requirements.minimum_source_ref_expectations",
    )
    require_string_list(
        default_requirements.get("diagnostics_expected"),
        "lens-review default_requirements.diagnostics_expected",
    )
    if default_requirements.get("ui_mcp_payload_mode") != "cluster-first":
        raise ValueError("lens-review ui_mcp_payload_mode must be cluster-first")

    raw_reviews = contract.get("views")
    if not isinstance(raw_reviews, list):
        raise ValueError("lens-review-contracts.json views must be a list")
    reviews: dict[str, dict[str, Any]] = {}
    for raw_review in raw_reviews:
        review = require_object(raw_review, "lens-review view")
        view_id = review.get("view_id")
        if not isinstance(view_id, str) or not view_id:
            raise ValueError("lens-review view_id must be a non-empty string")
        if view_id in reviews:
            raise ValueError(f"duplicate lens-review view id: {view_id}")
        for field in ("review_intent", "source_posture", "evidence_posture", "agent_packet_hint"):
            if not isinstance(review.get(field), str) or not review[field]:
                raise ValueError(f"{view_id}.{field} must be a non-empty string")
        collapse_rule = require_object(review.get("collapse_rule"), f"{view_id}.collapse_rule")
        require_string_list(collapse_rule.get("default_cluster_kinds"), f"{view_id}.collapse_rule.default_cluster_kinds")
        require_string_list(collapse_rule.get("expand_to"), f"{view_id}.collapse_rule.expand_to")
        require_string_list(review.get("ordering_hints"), f"{view_id}.ordering_hints")
        reviews[view_id] = review

    missing_reviews = sorted(view_ids - set(reviews))
    extra_reviews = sorted(set(reviews) - view_ids)
    if missing_reviews:
        raise ValueError(f"lens-review-contracts.json is missing views: {', '.join(missing_reviews)}")
    if extra_reviews:
        raise ValueError(f"lens-review-contracts.json references unknown views: {', '.join(extra_reviews)}")
    return reviews


def build_payload() -> dict[str, Any]:
    diagnostics: list[dict[str, str]] = []
    source_contract = load_json(REPO_ROOT / SOURCE_VIEW_CONTRACT_REF)
    lens_review_contract = load_json(REPO_ROOT / LENS_REVIEW_CONTRACT_REF)
    atlas_projection = load_json(REPO_ROOT / ATLAS_PROJECTION_REF)
    philosophy_manifest = load_json(REPO_ROOT / "ToS/philosophy/philosophy.manifest.json")
    graph_layers = parse_graph_layers(REPO_ROOT / GRAPH_LAYER_SOURCE_REF)

    if source_contract.get("schema_version") != "tos_philosophy_graph_view_contracts_v1":
        raise ValueError("view-contracts.json must use tos_philosophy_graph_view_contracts_v1")
    if source_contract.get("atlas_projection_ref") != ATLAS_PROJECTION_REF:
        raise ValueError(f"view-contracts.json must point to {ATLAS_PROJECTION_REF}")
    if source_contract.get("downstream_consumer") != "abyss-stack":
        raise ValueError("view-contracts.json downstream_consumer must be abyss-stack")

    manifest_view_routes = set(require_string_list(philosophy_manifest.get("graph_view_routes"), "graph_view_routes"))
    graph_layer_ids = {layer["layer_id"] for layer in graph_layers}
    projection = projection_sets(atlas_projection)
    raw_views = source_contract.get("views")
    if not isinstance(raw_views, list):
        raise ValueError("view-contracts.json views must be a list")
    raw_view_ids = {
        str(raw_view.get("view_id"))
        for raw_view in raw_views
        if isinstance(raw_view, dict) and isinstance(raw_view.get("view_id"), str)
    }
    lens_reviews = validate_lens_review_contract(lens_review_contract, raw_view_ids)

    seen_view_ids: set[str] = set()
    views: list[dict[str, Any]] = []
    for raw_view in sorted(raw_views, key=lambda item: item.get("order") if isinstance(item, dict) else 0):
        if not isinstance(raw_view, dict):
            raise ValueError("view-contracts.json views entries must be objects")
        validate_view_contract(
            raw_view,
            graph_layer_ids=graph_layer_ids,
            projection=projection,
            manifest_view_routes=manifest_view_routes,
        )
        view_id = str(raw_view["view_id"])
        if view_id in seen_view_ids:
            raise ValueError(f"duplicate graph view id: {view_id}")
        seen_view_ids.add(view_id)
        card = parse_view_card(REPO_ROOT / raw_view["route_card"])
        lens_review = lens_reviews[view_id]
        views.append(
            {
                "view_id": view_id,
                "title": card["title"],
                "route_card": raw_view["route_card"],
                "source_ref": raw_view["route_card"],
                "order": raw_view["order"],
                "lens": card["lens"],
                "future_inputs": card["future_inputs"],
                "boundary": card["boundary"],
                "graph_layers": raw_view["graph_layers"],
                "current_projection_filters": raw_view["current_projection_filters"],
                "future_branch_filters": raw_view["future_branch_filters"],
                "layout_hint": raw_view["layout_hint"],
                "group_by": raw_view["group_by"],
                "sort_fields": raw_view["sort_fields"],
                "review_intent": lens_review["review_intent"],
                "source_posture": lens_review["source_posture"],
                "evidence_posture": lens_review["evidence_posture"],
                "collapse_rule": lens_review["collapse_rule"],
                "ordering_hints": lens_review["ordering_hints"],
                "agent_packet_hint": lens_review["agent_packet_hint"],
            }
        )

    missing_contracts = sorted(projection["graph_view_ids"] - seen_view_ids)
    extra_contracts = sorted(seen_view_ids - projection["graph_view_ids"])
    if missing_contracts:
        diagnostics.append(
            {
                "level": "error",
                "path": SOURCE_VIEW_CONTRACT_REF,
                "message": f"missing graph view contracts: {', '.join(missing_contracts)}",
            }
        )
    if extra_contracts:
        diagnostics.append(
            {
                "level": "error",
                "path": SOURCE_VIEW_CONTRACT_REF,
                "message": f"contracts without atlas projection graph-view nodes: {', '.join(extra_contracts)}",
            }
        )

    payload: dict[str, Any] = {
        "schema_version": "tos_philosophy_graph_views_v1",
        "schema_ref": SCHEMA_REF,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_philosophy_graph_view_catalog",
        "source_view_contract_ref": SOURCE_VIEW_CONTRACT_REF,
        "lens_review_contract_ref": LENS_REVIEW_CONTRACT_REF,
        "source_view_root": SOURCE_VIEW_ROOT,
        "atlas_projection_ref": ATLAS_PROJECTION_REF,
        "runtime_projection_boundary": {
            "runtime_owner": "abyss-stack",
            "runtime_scope": [
                "read graph-view filters as ToS-owned display contracts",
                "map view ids to API, MCP, UI, layout, and cache behavior downstream",
                "render and switch graph lenses without writing runtime state back into ToS",
            ],
            "tos_authority_scope": [
                "view cards and view-contracts.json own ToS graph lens meaning",
                "atlas projection remains the current graph input, not canon",
                "future branch filters guide growth without proving future nodes already exist",
            ],
        },
        "validation_refs": list(VALIDATION_REFS),
        "counts": {
            "views": len(views),
            "graph_layers": len(graph_layers),
            "atlas_projection_graph_views": len(projection["graph_view_ids"]),
            "lens_review_contracts": len(lens_reviews),
            "diagnostics": len(diagnostics),
        },
        "graph_layers": graph_layers,
        "default_lens_review_requirements": lens_review_contract["default_requirements"],
        "views": views,
        "diagnostics": diagnostics,
    }
    validate_payload_schema(payload)
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"

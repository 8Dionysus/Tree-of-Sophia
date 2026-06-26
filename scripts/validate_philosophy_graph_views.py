#!/usr/bin/env python3
"""Validate the ToS philosophy graph view catalog export."""

from __future__ import annotations

import json

from philosophy_graph_views_common import GRAPH_VIEW_CATALOG_PATH, build_payload, render_payload, validate_payload_schema


def main() -> int:
    expected_payload = build_payload()
    current_payload = json.loads(GRAPH_VIEW_CATALOG_PATH.read_text(encoding="utf-8"))
    validate_payload_schema(current_payload)
    if render_payload(current_payload) != render_payload(expected_payload):
        raise SystemExit("ToS/derived-exports/philosophy_graph_views.min.json does not match the canonical rebuild")

    counts = current_payload.get("counts", {})
    if counts.get("views") != 11:
        raise SystemExit("philosophy graph view catalog must expose 11 graph views")
    if counts.get("graph_layers") != 7:
        raise SystemExit("philosophy graph view catalog must expose 7 graph layers")
    if counts.get("lens_review_contracts") != 11:
        raise SystemExit("philosophy graph view catalog must expose 11 lens review contracts")
    if counts.get("diagnostics") != 0:
        raise SystemExit("philosophy graph view catalog must not contain diagnostics")

    boundary = current_payload.get("runtime_projection_boundary", {})
    if boundary.get("runtime_owner") != "abyss-stack":
        raise SystemExit("philosophy graph view catalog must keep runtime projection ownership in abyss-stack")
    requirements = current_payload.get("default_lens_review_requirements", {})
    if requirements.get("ui_mcp_payload_mode") != "cluster-first":
        raise SystemExit("philosophy graph views must default to cluster-first UI/MCP packets")

    views = {
        view.get("view_id"): view
        for view in current_payload.get("views", [])
        if isinstance(view, dict)
    }
    chronology_filters = views["chronology"]["current_projection_filters"]
    if "formation" not in chronology_filters.get("row_fields", []):
        raise SystemExit("chronology view must expose formation row fields")
    source_layers = set(views["source-evidence"].get("graph_layers", []))
    if "evidence-relation" not in source_layers:
        raise SystemExit("source-evidence view must include the evidence-relation layer")
    if "research packets remain preparation" not in views["source-evidence"].get("source_posture", ""):
        raise SystemExit("source-evidence view must preserve research-packet/source-witness boundary")
    canon_layers = set(views["canon-promotion"].get("graph_layers", []))
    if not {"candidate-relation", "canonical-relation"} <= canon_layers:
        raise SystemExit("canon-promotion view must bridge candidate and canonical graph layers")
    canon_collapse = set(views["canon-promotion"].get("collapse_rule", {}).get("default_cluster_kinds", []))
    if "canon-candidate-status" not in canon_collapse:
        raise SystemExit("canon-promotion view must collapse by canon/candidate status")

    print("[ok] validated ToS/derived-exports/philosophy_graph_views.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

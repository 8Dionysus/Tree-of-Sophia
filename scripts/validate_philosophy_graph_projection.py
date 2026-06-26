#!/usr/bin/env python3
"""Validate the ToS philosophy graph projection export."""

from __future__ import annotations

import json

from philosophy_graph_projection_common import GRAPH_PROJECTION_PATH, build_payload, render_payload, validate_payload_schema


def main() -> int:
    expected_payload = build_payload()
    current_payload = json.loads(GRAPH_PROJECTION_PATH.read_text(encoding="utf-8"))
    validate_payload_schema(current_payload)
    if render_payload(current_payload) != render_payload(expected_payload):
        raise SystemExit("ToS/derived-exports/philosophy_graph_projection.min.json does not match the canonical rebuild")

    counts = current_payload.get("counts", {})
    if counts.get("views") != 11:
        raise SystemExit("philosophy graph projection must expose 11 graph views")
    if counts.get("graph_layers") != 7:
        raise SystemExit("philosophy graph projection must expose 7 graph layers")
    if int(counts.get("nodes") or 0) == 0:
        raise SystemExit("philosophy graph projection must expose projected nodes")
    if int(counts.get("edges") or 0) == 0:
        raise SystemExit("philosophy graph projection must expose projected edges")
    if counts.get("diagnostics") != 0:
        raise SystemExit("philosophy graph projection must not contain diagnostics in the current atlas slice")

    boundary = current_payload.get("runtime_projection_boundary", {})
    if boundary.get("runtime_owner") != "abyss-stack":
        raise SystemExit("philosophy graph projection must keep runtime projection ownership in abyss-stack")

    node_ids = {node.get("node_id") for node in current_payload.get("nodes", []) if isinstance(node, dict)}
    for edge in current_payload.get("edges", []):
        if edge.get("from_id") not in node_ids or edge.get("to_id") not in node_ids:
            raise SystemExit(f"{edge.get('edge_id')} has an endpoint outside the projection")
        if not edge.get("source_ref"):
            raise SystemExit(f"{edge.get('edge_id')} must preserve source_ref")

    views = {
        view.get("view_id"): view
        for view in current_payload.get("views", [])
        if isinstance(view, dict)
    }
    chronology = views.get("chronology", {})
    if chronology.get("layout_hint") != "timeline-lanes":
        raise SystemExit("chronology view must preserve the timeline-lanes layout hint")
    if not chronology.get("nodes") or not chronology.get("edges"):
        raise SystemExit("chronology view must contain materialized nodes and edges")
    source_evidence_layers = set(views.get("source-evidence", {}).get("graph_layers", []))
    if "evidence-relation" not in source_evidence_layers:
        raise SystemExit("source-evidence view must include the evidence-relation layer")
    for view_id, view in views.items():
        if not view.get("source_refs"):
            raise SystemExit(f"{view_id} view must expose source_refs")

    print("[ok] validated ToS/derived-exports/philosophy_graph_projection.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

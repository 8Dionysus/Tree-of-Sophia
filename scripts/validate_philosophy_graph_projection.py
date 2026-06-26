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
    if int(counts.get("clusters") or 0) == 0:
        raise SystemExit("philosophy graph projection must expose source-owned clusters")
    if counts.get("review_packets") != 11:
        raise SystemExit("philosophy graph projection must expose one review packet per graph view")
    if counts.get("diagnostics") != 0:
        raise SystemExit("philosophy graph projection must not contain diagnostics in the current atlas slice")

    boundary = current_payload.get("runtime_projection_boundary", {})
    if boundary.get("runtime_owner") != "abyss-stack":
        raise SystemExit("philosophy graph projection must keep runtime projection ownership in abyss-stack")
    visibility = current_payload.get("visibility_model", {})
    if visibility.get("default_payload_mode") != "cluster-first":
        raise SystemExit("philosophy graph projection must default to cluster-first payloads")
    if set(visibility.get("layer_ids", [])) != {layer.get("layer_id") for layer in current_payload.get("graph_layers", [])}:
        raise SystemExit("visibility_model layer_ids must match graph_layers")
    snapshot = current_payload.get("snapshot_review", {})
    current_snapshot = snapshot.get("current_snapshot", {}) if isinstance(snapshot, dict) else {}
    if snapshot.get("snapshot_schema_version") != "tos_philosophy_graph_projection_snapshot_v1":
        raise SystemExit("philosophy graph projection must expose snapshot review metadata")
    if not isinstance(current_snapshot.get("projection_fingerprint"), str) or len(current_snapshot["projection_fingerprint"]) != 64:
        raise SystemExit("snapshot review must expose a stable projection fingerprint")
    if len(current_snapshot.get("view_fingerprints", [])) != counts.get("views"):
        raise SystemExit("snapshot review must expose one fingerprint per graph view")
    if snapshot.get("diff_route", {}).get("mode") != "fingerprint-ready":
        raise SystemExit("snapshot review diff_route must be fingerprint-ready")

    node_ids = {node.get("node_id") for node in current_payload.get("nodes", []) if isinstance(node, dict)}
    for edge in current_payload.get("edges", []):
        if edge.get("from_id") not in node_ids or edge.get("to_id") not in node_ids:
            raise SystemExit(f"{edge.get('edge_id')} has an endpoint outside the projection")
        if not edge.get("source_ref"):
            raise SystemExit(f"{edge.get('edge_id')} must preserve source_ref")
    edge_ids = {edge.get("edge_id") for edge in current_payload.get("edges", []) if isinstance(edge, dict)}
    for cluster in current_payload.get("clusters", []):
        if not cluster.get("source_refs"):
            raise SystemExit(f"{cluster.get('cluster_id')} must preserve source_refs")
        if set(cluster.get("member_node_ids", [])) - node_ids:
            raise SystemExit(f"{cluster.get('cluster_id')} has member nodes outside the projection")
        if set(cluster.get("member_edge_ids", [])) - edge_ids:
            raise SystemExit(f"{cluster.get('cluster_id')} has member edges outside the projection")

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
        if not view.get("review_intent"):
            raise SystemExit(f"{view_id} view must expose review_intent")

    packets = {
        packet.get("view_id"): packet
        for packet in current_payload.get("review_packets", [])
        if isinstance(packet, dict)
    }
    if set(packets) != set(views):
        raise SystemExit("review packets must match graph view ids")
    canon_packet = packets.get("canon-promotion", {})
    if "candidate_to_canon_pressure" not in canon_packet:
        raise SystemExit("canon-promotion packet must expose candidate_to_canon_pressure")
    if len(str(canon_packet.get("changed_subgraph", {}).get("current_view_fingerprint") or "")) != 64:
        raise SystemExit("review packets must carry current view fingerprints for later changed-subgraph review")
    if not canon_packet.get("recommended_human_review_route"):
        raise SystemExit("review packets must expose a human review route")

    print("[ok] validated ToS/derived-exports/philosophy_graph_projection.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

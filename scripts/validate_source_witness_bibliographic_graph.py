#!/usr/bin/env python3
"""Validate the source-witness bibliographic claim graph projection."""

from __future__ import annotations

import json

from source_witness_bibliographic_graph_common import (
    GRAPH_PATH,
    build_payload,
    render_payload,
    validate_payload_schema,
)


def main() -> int:
    expected = build_payload()
    try:
        current = json.loads(GRAPH_PATH.read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError) as exc:
        raise SystemExit(f"cannot read bibliographic claim graph: {exc}") from exc
    validate_payload_schema(current)
    if render_payload(current) != render_payload(expected):
        raise SystemExit(
            "source-witness bibliographic claim graph differs from the canonical rebuild"
        )

    counts = current["counts"]
    if counts["source_claims"] != counts["claim_traces"]:
        raise SystemExit("every source claim must have exactly one graph trace")
    if counts["direct_subject_object_edges"] != 0:
        raise SystemExit("the bibliographic graph must not emit direct subject-object edges")
    if current["relation_model"]["direct_subject_object_edges"] is not False:
        raise SystemExit("the bibliographic graph relation model must remain claim-reified")
    if set(current["graph_layers"]) != {"bibliographic"}:
        raise SystemExit("the projection must remain limited to the bibliographic layer")
    if sum(current["review_counts"].values()) != counts["source_claims"]:
        raise SystemExit("review counts must cover every source claim exactly once")
    if current["relation_model"]["runtime_owner"] != "abyss-stack":
        raise SystemExit("runtime graph ownership must remain in abyss-stack")

    nodes = {node["node_id"]: node for node in current["nodes"]}
    claim_nodes = {
        node["properties"]["claim_ref"]: node["node_id"]
        for node in current["nodes"]
        if node["node_kind"] == "claim"
    }
    for edge in current["edges"]:
        if edge["from_id"] != claim_nodes.get(edge["claim_ref"]):
            raise SystemExit(f"{edge['edge_id']}: edge does not start at its claim node")
        if not edge["evidence_node_ids"]:
            raise SystemExit(f"{edge['edge_id']}: edge lost its evidence route")
        for ref in (
            edge["to_id"],
            edge["maker_node_id"],
            edge["provenance_event_node_id"],
            *edge["evidence_node_ids"],
        ):
            if ref not in nodes:
                raise SystemExit(f"{edge['edge_id']}: node reference {ref!r} is unresolved")
    for trace in current["claim_traces"]:
        event = nodes[trace["provenance_event_node_id"]]["properties"]
        if not event.get("started_at") or not event.get("ended_at"):
            raise SystemExit(f"{trace['claim_ref']}: provenance time is unresolved")
        if not isinstance(event.get("method"), dict) or not event["method"].get("name"):
            raise SystemExit(f"{trace['claim_ref']}: provenance method is unresolved")
        if nodes[trace["maker_node_id"]]["node_kind"] != "maker":
            raise SystemExit(f"{trace['claim_ref']}: maker route is unresolved")
        if not trace["evidence_node_ids"]:
            raise SystemExit(f"{trace['claim_ref']}: evidence route is empty")

    print("[ok] validated source-witness bibliographic claim graph")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

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
from partitioned_projection_common import build_storage, check_partitioned_payload


def main() -> int:
    with build_storage() as storage:
        expected = build_payload(storage=storage)
        check_partitioned_payload(GRAPH_PATH, expected)
        return validate_graph(expected, expected, storage)


def validate_graph(current, expected, storage=None):
    # Parity checks bind the committed parts to the exact source rebuild;
    # these independent relation/time assertions still inspect every row.
    validate_payload_schema(current)

    counts = current["counts"]
    if counts["source_claims"] != counts["claim_traces"]:
        raise SystemExit("every source claim must have exactly one graph trace")
    if counts["direct_subject_object_edges"] != 0:
        raise SystemExit("the bibliographic graph must not emit direct subject-object edges")
    if current["relation_model"]["direct_subject_object_edges"] is not False:
        raise SystemExit("the bibliographic graph relation model must remain claim-reified")
    if current["graph_layers"] != expected["graph_layers"]:
        raise SystemExit("projection layers must match the source-owned bibliographic and historical profiles")
    if sum(current["review_counts"].values()) != counts["source_claims"]:
        raise SystemExit("review counts must cover every source claim exactly once")
    if current["relation_model"]["runtime_owner"] != "abyss-stack":
        raise SystemExit("runtime graph ownership must remain in abyss-stack")

    make_map = storage.mapping if storage is not None else dict
    def validation_facts(node):
        if storage is None:
            return node
        properties = node.get("properties", {})
        method = properties.get("method")
        version = properties.get("schema_version")
        procedure = method.get("procedure") if version == "tos_provenance_event_v2" and isinstance(method, dict) else method
        procedure = {"name": procedure.get("name")} if isinstance(procedure, dict) else None
        return {"node_kind": node.get("node_kind"), "properties": {
            "started_at": properties.get("started_at"), "ended_at": properties.get("ended_at"),
            "schema_version": version,
            "method": {"procedure": procedure} if version == "tos_provenance_event_v2" else procedure}}

    nodes = make_map((node["node_id"], validation_facts(node)) for node in current["nodes"])
    claim_nodes = make_map(
        (node["properties"]["claim_ref"], node["node_id"])
        for node in current["nodes"]
        if node["node_kind"] == "claim"
    )
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
        method = event.get("method")
        procedure = (method.get('procedure') if event.get('schema_version') == 'tos_provenance_event_v2'
                     and isinstance(method, dict) else method)
        if not isinstance(procedure, dict) or not procedure.get("name"):
            raise SystemExit(f"{trace['claim_ref']}: provenance method is unresolved")
        if nodes[trace["maker_node_id"]]["node_kind"] != "maker":
            raise SystemExit(f"{trace['claim_ref']}: maker route is unresolved")
        if not trace["evidence_node_ids"]:
            raise SystemExit(f"{trace['claim_ref']}: evidence route is empty")

    print("[ok] validated source-witness bibliographic claim graph")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

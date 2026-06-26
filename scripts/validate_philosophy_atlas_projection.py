#!/usr/bin/env python3
"""Validate the ToS philosophy atlas projection export."""

from __future__ import annotations

import json

from philosophy_atlas_projection_common import PROJECTION_PATH, build_payload, render_payload, validate_payload_schema


def main() -> int:
    expected_payload = build_payload()
    current_payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
    validate_payload_schema(current_payload)
    if render_payload(current_payload) != render_payload(expected_payload):
        raise SystemExit("ToS/derived-exports/philosophy_atlas_projection.min.json does not match the canonical rebuild")

    counts = current_payload.get("counts", {})
    required_counts = {
        "master_tables": 3,
        "master_rows": 190,
        "dossiers": 30,
        "dossier_node_rows": 1040,
        "dossier_relation_rows": 986,
        "candidate_nodes": 1040,
        "candidate_relations": 986,
    }
    for key, expected in required_counts.items():
        if counts.get(key) != expected:
            raise SystemExit(f"philosophy atlas projection must keep counts.{key}={expected}")
    if counts.get("graph_views", 0) < 1:
        raise SystemExit("philosophy atlas projection must include graph view route nodes")
    if counts.get("nodes", 0) <= counts.get("master_rows", 0):
        raise SystemExit("philosophy atlas projection must include structural nodes beyond master rows")
    if counts.get("edges", 0) <= counts.get("nodes", 0):
        raise SystemExit("philosophy atlas projection must include graph edges")

    boundary = current_payload.get("runtime_projection_boundary", {})
    if boundary.get("runtime_owner") != "abyss-stack":
        raise SystemExit("philosophy atlas projection must keep runtime projection ownership in abyss-stack")

    diagnostics = current_payload.get("diagnostics", [])
    errors = [item for item in diagnostics if isinstance(item, dict) and item.get("level") == "error"]
    if errors:
        raise SystemExit(f"philosophy atlas projection contains error diagnostics: {errors[:3]}")

    print("[ok] validated ToS/derived-exports/philosophy_atlas_projection.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

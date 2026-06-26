#!/usr/bin/env python3
"""Validate the ToS philosophy post-planting audit packet."""

from __future__ import annotations

import json

from philosophy_post_planting_audit_common import AUDIT_JSON_PATH, AUDIT_MD_PATH, build_payload, render_markdown, render_payload


def main() -> int:
    expected = build_payload()
    current = json.loads(AUDIT_JSON_PATH.read_text(encoding="utf-8"))
    if render_payload(current) != render_payload(expected):
        raise SystemExit("post-planting audit JSON does not match the canonical rebuild")
    if AUDIT_MD_PATH.read_text(encoding="utf-8") != render_markdown(expected):
        raise SystemExit("post-planting audit Markdown does not match the canonical rebuild")

    if current.get("schema_version") != "tos_philosophy_post_planting_audit_v1":
        raise SystemExit("post-planting audit has an unexpected schema_version")
    if current.get("runtime_projection_boundary", {}).get("runtime_owner") != "abyss-stack":
        raise SystemExit("post-planting audit must keep runtime ownership in abyss-stack")
    counts = current.get("counts", {})
    if counts.get("prepared_dossiers") != 30:
        raise SystemExit("post-planting audit must account for the 30 prepared Table I dossiers")
    if counts.get("errors") != 0:
        raise SystemExit("post-planting audit must not contain error diagnostics")
    if current.get("review_readiness", {}).get("status") != "ready_for_first_graph_review":
        raise SystemExit("post-planting audit must be ready for the first graph review")
    graph = current.get("graph_workbench_audit", {})
    if graph.get("canon_status") != "pre-canon":
        raise SystemExit("post-planting audit must keep prepared graph material pre-canon")
    projection = current.get("graph_projection_audit", {})
    if projection.get("views") != 11 or projection.get("review_packets") != 11:
        raise SystemExit("post-planting audit must see all 11 graph views and review packets")

    print("[ok] validated ToS philosophy post-planting audit")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Shared builder for the ToS philosophy post-planting audit."""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
TOS_ROOT = REPO_ROOT / "ToS"
PHILOSOPHY_ROOT = TOS_ROOT / "philosophy"
AUDIT_JSON_PATH = PHILOSOPHY_ROOT / "graph-workbench/review-packets/table-i-post-planting-audit.json"
AUDIT_MD_PATH = PHILOSOPHY_ROOT / "graph-workbench/review-packets/table-i-post-planting-audit.md"
TABLE_ROOT = PHILOSOPHY_ROOT / "atlas/master-tables"
DOSSIER_INDEX_PATH = PHILOSOPHY_ROOT / "atlas/dossiers/index.jsonl"
DOSSIER_SUMMARY_PATH = PHILOSOPHY_ROOT / "atlas/dossiers/graph-shape-summary.json"
SOURCE_ANCHOR_BACKLOG_PATH = PHILOSOPHY_ROOT / "atlas/dossiers/source-anchor-backlog.jsonl"
TERM_INDEX_PATH = PHILOSOPHY_ROOT / "atlas/dossiers/term-index.jsonl"
TRANSMISSION_BACKLOG_PATH = PHILOSOPHY_ROOT / "atlas/dossiers/transmission-backlog.jsonl"
PROPOSED_NODES_PATH = PHILOSOPHY_ROOT / "graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl"
PROPOSED_RELATIONS_PATH = PHILOSOPHY_ROOT / "graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl"
LANGUAGE_PACKETS_PATH = PHILOSOPHY_ROOT / "graph-workbench/language-packets/table-i-text-bearing-nodes.jsonl"
BRANCH_FRAGMENTS_PATH = PHILOSOPHY_ROOT / "graph-workbench/branch-fragments/table-i-prepared-dossier-branches.json"
GRAPH_PROJECTION_PATH = TOS_ROOT / "derived-exports/philosophy_graph_projection.min.json"
GRAPH_VIEWS_PATH = TOS_ROOT / "derived-exports/philosophy_graph_views.min.json"
ATLAS_PROJECTION_PATH = TOS_ROOT / "derived-exports/philosophy_atlas_projection.min.json"

def repo_ref(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{repo_ref(path)} must contain a JSON object")
    return payload


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            row = json.loads(line)
            if not isinstance(row, dict):
                raise ValueError(f"{repo_ref(path)} must contain JSON objects")
            rows.append(row)
    return rows


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def render_markdown(payload: dict[str, Any]) -> str:
    counts = payload["counts"]
    table_i = payload["master_tables"]["table-i"]
    branch = payload["branch_audit"]
    graph = payload["graph_workbench_audit"]
    projection = payload["graph_projection_audit"]
    readiness = payload["review_readiness"]
    diagnostics = payload["diagnostics"]
    diagnostic_lines = "\n".join(
        f"- {item['level']}: `{item['path']}` - {item['message']}" for item in diagnostics
    ) or "- clear"
    return (
        "# Table I Post-Planting Audit\n\n"
        "This generated review packet checks the first prepared Table I planting "
        "against the ToS philosophy topology before runtime graph review.\n\n"
        "## Readiness\n\n"
        f"- Status: `{readiness['status']}`\n"
        f"- Prepared dossiers: {table_i['dossier_available_count']} / {table_i['row_count']}\n"
        f"- Prepared branches: {branch['prepared_branch_count']}\n"
        f"- Proposed nodes: {graph['proposed_node_count']}\n"
        f"- Proposed relations: {graph['proposed_relation_count']}\n"
        f"- Text-bearing language packets: {graph['language_packet_count']}\n"
        f"- Graph views: {projection['views']}\n"
        f"- Review packets: {projection['review_packets']}\n\n"
        "## Counts\n\n"
        "| Surface | Count |\n"
        "| --- | ---: |\n"
        f"| master rows | {counts['master_rows']} |\n"
        f"| dossier rows | {counts['prepared_dossiers']} |\n"
        f"| source anchors | {counts['source_anchor_rows']} |\n"
        f"| terms | {counts['term_rows']} |\n"
        f"| transmissions | {counts['transmission_rows']} |\n"
        f"| projection nodes | {projection['nodes']} |\n"
        f"| projection edges | {projection['edges']} |\n"
        f"| clusters | {projection['clusters']} |\n\n"
        "## Diagnostics\n\n"
        f"{diagnostic_lines}\n\n"
        "## Next Routes\n\n"
        + "\n".join(f"- `{route}`" for route in readiness["next_routes"])
        + "\n"
    )


def _table_audit(table_id: str) -> dict[str, Any]:
    rows = load_jsonl(TABLE_ROOT / table_id / "rows.jsonl")
    available = [row for row in rows if row.get("dossier_available") is True]
    missing = [str(row.get("row_id") or "") for row in rows if row.get("dossier_available") is not True]
    return {
        "row_count": len(rows),
        "dossier_available_count": len(available),
        "dossier_missing_count": len(missing),
        "available_row_ids": [str(row.get("row_id") or "") for row in available],
        "missing_row_ids": missing,
        "source_ref": repo_ref(TABLE_ROOT / table_id / "rows.jsonl"),
    }


def _branch_audit(dossier_rows: list[dict[str, Any]]) -> dict[str, Any]:
    branch_paths = sorted(
        {
            str(row.get("branch_path") or "")
            for row in dossier_rows
            if isinstance(row.get("branch_path"), str) and row.get("branch_path")
        }
    )
    missing_branch_paths: list[str] = []
    missing_support: list[dict[str, str]] = []
    for branch_path in branch_paths:
        path = REPO_ROOT / branch_path
        if not path.exists():
            missing_branch_paths.append(branch_path)
            continue
        required = [
            "README.md",
            "branch.manifest.json",
            "sources/source-anchor-backlog.jsonl",
            "graph-workbench/pre-canon-summary.json",
        ]
        for child in required:
            if not (path / child).exists():
                missing_support.append({"branch_path": branch_path, "missing": child})

    local_summaries = {
        repo_ref(path)
        for path in PHILOSOPHY_ROOT.glob("eras/**/graph-workbench/pre-canon-summary.json")
    }
    expected_summaries = {f"{branch}/graph-workbench/pre-canon-summary.json" for branch in branch_paths}
    return {
        "prepared_branch_count": len(branch_paths),
        "prepared_branch_paths": branch_paths,
        "missing_branch_paths": missing_branch_paths,
        "missing_branch_support": missing_support,
        "orphan_local_graph_summaries": sorted(local_summaries - expected_summaries),
    }


def _graph_workbench_audit(
    proposed_nodes: list[dict[str, Any]],
    proposed_relations: list[dict[str, Any]],
    language_packets: list[dict[str, Any]],
) -> dict[str, Any]:
    endpoint_resolution_counts = Counter(
        str(row.get("endpoint_resolution") or "missing") for row in proposed_relations
    )
    node_kind_counts = Counter(str(row.get("node_kind") or "unspecified") for row in proposed_nodes)
    relation_kind_counts = Counter(str(row.get("relation_kind") or "related_to") for row in proposed_relations)
    branch_fragments = load_json(BRANCH_FRAGMENTS_PATH)
    return {
        "proposed_node_count": len(proposed_nodes),
        "proposed_relation_count": len(proposed_relations),
        "language_packet_count": len(language_packets),
        "text_bearing_node_count": node_kind_counts.get("text_corpus", 0),
        "endpoint_resolution_counts": dict(sorted(endpoint_resolution_counts.items())),
        "node_kind_top": dict(node_kind_counts.most_common(12)),
        "relation_kind_top": dict(relation_kind_counts.most_common(12)),
        "branch_fragment_count": int(branch_fragments.get("branch_count") or 0),
        "canon_status": str(branch_fragments.get("canon_status") or ""),
        "source_refs": [
            repo_ref(PROPOSED_NODES_PATH),
            repo_ref(PROPOSED_RELATIONS_PATH),
            repo_ref(LANGUAGE_PACKETS_PATH),
            repo_ref(BRANCH_FRAGMENTS_PATH),
        ],
    }


def _projection_audit() -> dict[str, Any]:
    projection = load_json(GRAPH_PROJECTION_PATH)
    counts = projection.get("counts")
    if not isinstance(counts, dict):
        counts = {}
    snapshot = projection.get("snapshot_review")
    return {
        "views": int(counts.get("views") or 0),
        "graph_layers": int(counts.get("graph_layers") or 0),
        "nodes": int(counts.get("nodes") or 0),
        "edges": int(counts.get("edges") or 0),
        "clusters": int(counts.get("clusters") or 0),
        "review_packets": int(counts.get("review_packets") or 0),
        "unresolved_review_surfaces": int(counts.get("unresolved_review_surfaces") or 0),
        "diagnostics": int(counts.get("diagnostics") or 0),
        "snapshot_ready": isinstance(snapshot, dict),
        "source_ref": repo_ref(GRAPH_PROJECTION_PATH),
    }


def build_payload() -> dict[str, Any]:
    table_audits = {table_id: _table_audit(table_id) for table_id in ("table-i", "table-ii", "table-iii")}
    dossier_rows = load_jsonl(DOSSIER_INDEX_PATH)
    dossier_summary = load_json(DOSSIER_SUMMARY_PATH)
    source_anchor_rows = load_jsonl(SOURCE_ANCHOR_BACKLOG_PATH)
    term_rows = load_jsonl(TERM_INDEX_PATH)
    transmission_rows = load_jsonl(TRANSMISSION_BACKLOG_PATH)
    proposed_nodes = load_jsonl(PROPOSED_NODES_PATH)
    proposed_relations = load_jsonl(PROPOSED_RELATIONS_PATH)
    language_packets = load_jsonl(LANGUAGE_PACKETS_PATH)
    branch_audit = _branch_audit(dossier_rows)
    graph_audit = _graph_workbench_audit(proposed_nodes, proposed_relations, language_packets)
    projection_audit = _projection_audit()

    diagnostics: list[dict[str, str]] = []
    if table_audits["table-i"]["dossier_available_count"] != len(dossier_rows):
        diagnostics.append(
            {
                "level": "error",
                "path": repo_ref(DOSSIER_INDEX_PATH),
                "message": "Table I available dossier rows do not match the dossier index",
            }
        )
    for path in branch_audit["missing_branch_paths"]:
        diagnostics.append({"level": "error", "path": path, "message": "prepared branch path is missing"})
    for item in branch_audit["missing_branch_support"]:
        diagnostics.append(
            {
                "level": "error",
                "path": f"{item['branch_path']}/{item['missing']}",
                "message": "prepared branch support surface is missing",
            }
        )
    for path in branch_audit["orphan_local_graph_summaries"]:
        diagnostics.append({"level": "warning", "path": path, "message": "local graph summary is outside the prepared dossier index"})
    if projection_audit["diagnostics"] != 0:
        diagnostics.append(
            {
                "level": "error",
                "path": repo_ref(GRAPH_PROJECTION_PATH),
                "message": "graph projection contains diagnostics",
            }
        )
    if graph_audit["language_packet_count"] != graph_audit["text_bearing_node_count"]:
        diagnostics.append(
            {
                "level": "error",
                "path": repo_ref(LANGUAGE_PACKETS_PATH),
                "message": "text-bearing language packets do not match text-corpus proposed node count",
            }
        )

    error_count = sum(1 for item in diagnostics if item["level"] == "error")
    status = "ready_for_first_graph_review" if error_count == 0 else "blocked_by_audit_errors"
    return {
        "schema_version": "tos_philosophy_post_planting_audit_v1",
        "surface_ref": repo_ref(AUDIT_JSON_PATH),
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": "ToS/philosophy/graph-workbench/review-packets/README.md",
        "source_refs": {
            "atlas_projection": repo_ref(ATLAS_PROJECTION_PATH),
            "graph_views": repo_ref(GRAPH_VIEWS_PATH),
            "graph_projection": repo_ref(GRAPH_PROJECTION_PATH),
            "dossier_index": repo_ref(DOSSIER_INDEX_PATH),
            "dossier_summary": repo_ref(DOSSIER_SUMMARY_PATH),
            "proposed_nodes": repo_ref(PROPOSED_NODES_PATH),
            "proposed_relations": repo_ref(PROPOSED_RELATIONS_PATH),
            "language_packets": repo_ref(LANGUAGE_PACKETS_PATH),
            "branch_fragments": repo_ref(BRANCH_FRAGMENTS_PATH),
        },
        "runtime_projection_boundary": {
            "runtime_owner": "abyss-stack",
            "runtime_role": "consume the generated graph projection, review packets, and audit as runtime/API/UI inputs",
            "tos_authority": "author and regenerate atlas, graph, dossier, branch, and audit surfaces",
        },
        "counts": {
            "master_tables": 3,
            "master_rows": sum(table["row_count"] for table in table_audits.values()),
            "prepared_dossiers": len(dossier_rows),
            "source_anchor_rows": len(source_anchor_rows),
            "term_rows": len(term_rows),
            "transmission_rows": len(transmission_rows),
            "diagnostics": len(diagnostics),
            "errors": error_count,
            "warnings": len(diagnostics) - error_count,
        },
        "master_tables": table_audits,
        "dossier_shape": dossier_summary,
        "branch_audit": branch_audit,
        "graph_workbench_audit": graph_audit,
        "source_anchor_audit": {
            "source_anchor_count": len(source_anchor_rows),
            "term_count": len(term_rows),
            "transmission_count": len(transmission_rows),
        },
        "graph_projection_audit": projection_audit,
        "review_readiness": {
            "status": status,
            "next_routes": [
                "ToS/philosophy/graph-workbench/views/",
                "ToS/philosophy/graph-workbench/review-packets/",
                "ToS/derived-exports/philosophy_graph_projection.min.json",
                "abyss-stack tos-graph runtime bridge",
            ],
        },
        "diagnostics": diagnostics,
    }

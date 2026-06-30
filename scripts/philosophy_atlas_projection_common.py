#!/usr/bin/env python3
"""Shared helpers for the ToS philosophy atlas projection export."""

from __future__ import annotations

import json
import hashlib
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

from philosophy_multilingual_common import content_language_contract, multilingual_label


REPO_ROOT = Path(__file__).resolve().parents[1]
TOS_ROOT = REPO_ROOT / "ToS"
PROJECTION_PATH = TOS_ROOT / "derived-exports" / "philosophy_atlas_projection.min.json"
SCHEMA_REF = "ToS/contracts/philosophy-atlas-projection.schema.json"
SOURCE_ATLAS_REF = "ToS/philosophy/atlas/atlas.manifest.json"
CANDIDATE_NODES_REF = "ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl"
CANDIDATE_RELATIONS_REF = "ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl"
VALIDATION_REFS = (
    "scripts/build_philosophy_atlas_projection.py",
    "scripts/validate_philosophy_atlas_projection.py",
    "tests/test_philosophy_atlas_projection.py",
)


def repo_ref(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{repo_ref(path)} must contain a JSON object")
    return payload


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        payload = json.loads(line)
        if not isinstance(payload, dict):
            raise ValueError(f"{repo_ref(path)}:{line_number} must contain a JSON object")
        rows.append(payload)
    return rows


def load_optional_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    return load_jsonl(path)


def add_node(
    nodes: list[dict[str, Any]],
    node_id: str,
    node_type: str,
    label: str,
    source_ref: str,
    **properties: Any,
) -> None:
    clean_properties = {key: value for key, value in properties.items() if value is not None}
    nodes.append(
        {
            "node_id": node_id,
            "node_type": node_type,
            "label": label,
            "multilingual": multilingual_label(label, source_ref, {"node_type": node_type, **clean_properties}),
            "source_ref": source_ref,
            "properties": clean_properties,
        }
    )


def add_edge(
    edges: list[dict[str, Any]],
    edge_id: str,
    from_id: str,
    predicate_id: str,
    to_id: str,
    source_ref: str,
    **properties: Any,
) -> None:
    edges.append(
        {
            "edge_id": edge_id,
            "from_id": from_id,
            "predicate_id": predicate_id,
            "to_id": to_id,
            "source_ref": source_ref,
            "properties": {key: value for key, value in properties.items() if value is not None},
        }
    )


def row_projection_fields(row: dict[str, Any]) -> dict[str, Any]:
    normalized = row.get("normalized")
    if not isinstance(normalized, dict):
        normalized = {}
    research_node = (
        normalized.get("macroregion_research_node")
        or normalized.get("research_node")
        or normalized.get("node_and_task")
        or row.get("row_id")
    )
    return {
        "table_id": row.get("table_id"),
        "table_label": row.get("table_label"),
        "row_order": row.get("row_order"),
        "source_document": row.get("source_document"),
        "source_section": row.get("source_section"),
        "launch_order": normalized.get("launch_order"),
        "status": normalized.get("status"),
        "confidence": normalized.get("confidence"),
        "formation": normalized.get("formation"),
        "fixation": (
            normalized.get("written_fixation")
            or normalized.get("fixation_translation")
            or normalized.get("fixation_print_institutional_entry")
        ),
        "canonization": (
            normalized.get("canonization_redaction_commentary")
            or normalized.get("canonization_commentary")
            or normalized.get("canonization_academization")
        ),
        "research_node": research_node,
        "dossier_id": row.get("dossier_id"),
        "dossier_available": row.get("dossier_available"),
    }


def candidate_node_ref(candidate_id: str) -> str:
    return f"candidate-node:{candidate_id}"


def endpoint_ref(dossier_id: str, label: str) -> str:
    digest = hashlib.sha1(f"{dossier_id}|{label}".encode("utf-8")).hexdigest()[:12]
    return f"candidate-endpoint:{dossier_id}:{digest}"


def load_schema() -> dict[str, Any]:
    return load_json(REPO_ROOT / SCHEMA_REF)


def validate_payload_schema(payload: dict[str, Any]) -> None:
    validator = Draft202012Validator(load_schema())
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.absolute_path))
    if errors:
        error = errors[0]
        path = "".join(f"[{item}]" if isinstance(item, int) else f".{item}" for item in error.absolute_path)
        raise ValueError(f"schema violation at {path.lstrip('.') or '<root>'}: {error.message}")


def build_payload() -> dict[str, Any]:
    diagnostics: list[dict[str, str]] = []
    atlas = load_json(REPO_ROOT / SOURCE_ATLAS_REF)
    dossier_index_path = REPO_ROOT / "ToS/philosophy/atlas/dossiers/index.jsonl"
    graph_shape_path = REPO_ROOT / "ToS/philosophy/atlas/dossiers/graph-shape-summary.json"
    dossier_rows = load_jsonl(dossier_index_path)
    graph_shape = load_json(graph_shape_path)
    candidate_nodes = load_optional_jsonl(REPO_ROOT / CANDIDATE_NODES_REF)
    candidate_relations = load_optional_jsonl(REPO_ROOT / CANDIDATE_RELATIONS_REF)

    nodes: list[dict[str, Any]] = []
    edges: list[dict[str, Any]] = []
    row_count = 0

    add_node(nodes, "philosophy", "domain-root", "Philosophy", "ToS/philosophy/philosophy.manifest.json")
    add_node(nodes, "philosophy.atlas", "atlas", "Philosophy Atlas", SOURCE_ATLAS_REF)
    add_node(
        nodes,
        "philosophy.atlas.master-tables",
        "atlas-section",
        "Master Tables",
        "ToS/philosophy/atlas/master-tables/branch.manifest.json",
    )
    add_node(
        nodes,
        "philosophy.atlas.dossiers",
        "atlas-section",
        "Dossiers",
        "ToS/philosophy/atlas/dossiers/branch.manifest.json",
    )
    add_node(
        nodes,
        "philosophy.graph-views",
        "view-section",
        "Graph Views",
        "ToS/philosophy/graph-workbench/views/README.md",
    )

    add_edge(edges, "edge:philosophy:has-atlas", "philosophy", "has_atlas", "philosophy.atlas", SOURCE_ATLAS_REF)
    add_edge(
        edges,
        "edge:atlas:has-master-tables",
        "philosophy.atlas",
        "has_section",
        "philosophy.atlas.master-tables",
        SOURCE_ATLAS_REF,
    )
    add_edge(
        edges,
        "edge:atlas:has-dossiers",
        "philosophy.atlas",
        "has_section",
        "philosophy.atlas.dossiers",
        SOURCE_ATLAS_REF,
    )
    add_edge(
        edges,
        "edge:atlas:has-graph-views",
        "philosophy.atlas",
        "has_view_section",
        "philosophy.graph-views",
        SOURCE_ATLAS_REF,
    )

    for table in atlas.get("master_tables", []):
        if not isinstance(table, dict):
            diagnostics.append(
                {
                    "level": "error",
                    "path": SOURCE_ATLAS_REF,
                    "message": "master_tables entry is not an object",
                }
            )
            continue
        table_id = str(table.get("table_id") or "")
        table_node_id = f"atlas-table:{table_id}"
        table_manifest_ref = str(table.get("manifest") or "")
        rows_ref = str(table.get("rows") or "")
        rows_path = REPO_ROOT / rows_ref
        rows = load_jsonl(rows_path)
        row_count += len(rows)
        add_node(
            nodes,
            table_node_id,
            "master-table",
            str(table.get("table_label") or table_id),
            table_manifest_ref,
            table_id=table_id,
            row_count=len(rows),
            source_document=table.get("source_document"),
            rows_ref=rows_ref,
        )
        add_edge(
            edges,
            f"edge:master-tables:contains:{table_id}",
            "philosophy.atlas.master-tables",
            "contains_table",
            table_node_id,
            table_manifest_ref,
            row_count=len(rows),
        )
        for row in rows:
            row_id = str(row.get("row_id") or "")
            node_id = f"atlas-row:{row_id}"
            fields = row_projection_fields(row)
            add_node(nodes, node_id, "master-table-row", row_id, rows_ref, **fields)
            add_edge(
                edges,
                f"edge:{table_id}:contains-row:{row_id}",
                table_node_id,
                "contains_row",
                node_id,
                rows_ref,
                row_order=row.get("row_order"),
            )
            dossier_id = row.get("dossier_id")
            if isinstance(dossier_id, str) and dossier_id:
                add_edge(
                    edges,
                    f"edge:row:{row_id}:has-dossier:{dossier_id}",
                    node_id,
                    "has_prepared_dossier",
                    f"atlas-dossier:{dossier_id}",
                    rows_ref,
                )

    for dossier in dossier_rows:
        dossier_id = str(dossier.get("dossier_id") or "")
        node_id = f"atlas-dossier:{dossier_id}"
        add_node(
            nodes,
            node_id,
            "prepared-dossier",
            str(dossier.get("title") or dossier_id),
            repo_ref(dossier_index_path),
            dossier_id=dossier_id,
            source_document=dossier.get("source_document"),
            node_row_count=dossier.get("node_row_count"),
            relation_row_count=dossier.get("relation_row_count"),
            table_count=dossier.get("table_count"),
        )
        add_edge(
            edges,
            f"edge:dossiers:contains:{dossier_id}",
            "philosophy.atlas.dossiers",
            "contains_dossier",
            node_id,
            repo_ref(dossier_index_path),
        )
        node_type_counts = dossier.get("node_type_counts")
        if isinstance(node_type_counts, dict):
            for node_type, count in sorted(node_type_counts.items()):
                type_node_id = f"atlas-node-type:{node_type}"
                add_edge(
                    edges,
                    f"edge:dossier:{dossier_id}:node-type:{node_type}",
                    node_id,
                    "has_node_type_pressure",
                    type_node_id,
                    repo_ref(dossier_index_path),
                    count=count,
                )
        relation_counts = dossier.get("relation_counts")
        if isinstance(relation_counts, dict):
            for relation, count in sorted(relation_counts.items()):
                relation_node_id = f"atlas-relation-kind:{relation}"
                add_edge(
                    edges,
                    f"edge:dossier:{dossier_id}:relation-kind:{relation}",
                    node_id,
                    "has_relation_pressure",
                    relation_node_id,
                    repo_ref(dossier_index_path),
                    count=count,
                )

    for node_type, count in sorted((graph_shape.get("node_type_counts") or {}).items()):
        add_node(
            nodes,
            f"atlas-node-type:{node_type}",
            "atlas-node-type",
            str(node_type),
            repo_ref(graph_shape_path),
            count=count,
        )
        add_edge(
            edges,
            f"edge:atlas:node-type:{node_type}",
            "philosophy.atlas",
            "has_node_type_pressure",
            f"atlas-node-type:{node_type}",
            repo_ref(graph_shape_path),
            count=count,
        )

    for relation, count in sorted((graph_shape.get("relation_counts") or {}).items()):
        add_node(
            nodes,
            f"atlas-relation-kind:{relation}",
            "atlas-relation-kind",
            str(relation),
            repo_ref(graph_shape_path),
            count=count,
        )
        add_edge(
            edges,
            f"edge:atlas:relation-kind:{relation}",
            "philosophy.atlas",
            "has_relation_pressure",
            f"atlas-relation-kind:{relation}",
            repo_ref(graph_shape_path),
            count=count,
        )

    for candidate in candidate_nodes:
        candidate_id = str(candidate.get("candidate_id") or "")
        dossier_id = str(candidate.get("dossier_id") or "")
        if not candidate_id:
            diagnostics.append(
                {
                    "level": "warning",
                    "path": CANDIDATE_NODES_REF,
                    "message": "candidate node row without candidate_id was skipped",
                }
            )
            continue
        node_id = candidate_node_ref(candidate_id)
        add_node(
            nodes,
            node_id,
            "candidate-node",
            str(candidate.get("label") or candidate_id),
            CANDIDATE_NODES_REF,
            candidate_id=candidate_id,
            dossier_id=dossier_id,
            atlas_row_id=candidate.get("atlas_row_id"),
            branch_path=candidate.get("branch_path"),
            original_node_id=candidate.get("original_node_id"),
            original_node_type=candidate.get("node_kind"),
            period=candidate.get("period"),
            priority=candidate.get("priority"),
            canon_status=candidate.get("canon_status"),
            authority_posture=candidate.get("authority_posture"),
            source_document=candidate.get("source_document"),
        )
        if dossier_id:
            add_edge(
                edges,
                f"edge:dossier:{dossier_id}:candidate-node:{candidate_id}",
                f"atlas-dossier:{dossier_id}",
                "has_candidate_node",
                node_id,
                CANDIDATE_NODES_REF,
            )

    endpoint_nodes: set[str] = set()
    for relation in candidate_relations:
        candidate_id = str(relation.get("candidate_id") or "")
        dossier_id = str(relation.get("dossier_id") or "")
        relation_kind = str(relation.get("relation_kind") or "related_to")
        source_candidate_id = relation.get("source_candidate_id")
        target_candidate_id = relation.get("target_candidate_id")
        source_label = str(relation.get("source_endpoint_label") or "source endpoint")
        target_label = str(relation.get("target_endpoint_label") or "target endpoint")
        if not candidate_id:
            diagnostics.append(
                {
                    "level": "warning",
                    "path": CANDIDATE_RELATIONS_REF,
                    "message": "candidate relation row without candidate_id was skipped",
                }
            )
            continue
        if isinstance(source_candidate_id, str) and source_candidate_id:
            from_id = candidate_node_ref(source_candidate_id)
        else:
            from_id = endpoint_ref(dossier_id, source_label)
            if from_id not in endpoint_nodes:
                endpoint_nodes.add(from_id)
                add_node(
                    nodes,
                    from_id,
                    "candidate-endpoint",
                    source_label,
                    CANDIDATE_RELATIONS_REF,
                    dossier_id=dossier_id,
                    branch_path=relation.get("branch_path"),
                    endpoint_role="source",
                    canon_status="pre-canon",
                )
        if isinstance(target_candidate_id, str) and target_candidate_id:
            to_id = candidate_node_ref(target_candidate_id)
        else:
            to_id = endpoint_ref(dossier_id, target_label)
            if to_id not in endpoint_nodes:
                endpoint_nodes.add(to_id)
                add_node(
                    nodes,
                    to_id,
                    "candidate-endpoint",
                    target_label,
                    CANDIDATE_RELATIONS_REF,
                    dossier_id=dossier_id,
                    branch_path=relation.get("branch_path"),
                    endpoint_role="target",
                    canon_status="pre-canon",
                )
        add_edge(
            edges,
            f"edge:candidate-relation:{candidate_id}",
            from_id,
            relation_kind,
            to_id,
            CANDIDATE_RELATIONS_REF,
            candidate_id=candidate_id,
            dossier_id=dossier_id,
            atlas_row_id=relation.get("atlas_row_id"),
            branch_path=relation.get("branch_path"),
            relation_label=relation.get("relation_label"),
            confidence=relation.get("confidence"),
            canon_status=relation.get("canon_status"),
            authority_posture=relation.get("authority_posture"),
            endpoint_resolution=relation.get("endpoint_resolution"),
            comment=relation.get("comment"),
        )

    views_root = REPO_ROOT / "ToS/philosophy/graph-workbench/views"
    for path in sorted(views_root.glob("*.graph.md")):
        view_id = path.stem.removesuffix(".graph")
        node_id = f"graph-view:{view_id}"
        add_node(nodes, node_id, "graph-view", view_id, repo_ref(path), view_file=repo_ref(path))
        add_edge(
            edges,
            f"edge:graph-views:contains:{view_id}",
            "philosophy.graph-views",
            "contains_view",
            node_id,
            repo_ref(path),
        )

    payload: dict[str, Any] = {
        "schema_version": "tos_philosophy_atlas_projection_v1",
        "schema_ref": SCHEMA_REF,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_philosophy_atlas_projection",
        "source_atlas_ref": SOURCE_ATLAS_REF,
        "content_language_contract": content_language_contract(),
        "runtime_projection_boundary": {
            "runtime_owner": "abyss-stack",
            "runtime_scope": [
                "read this projection as a ToS-owned graph input",
                "serve MCP/API resources that point back to ToS surfaces",
                "render UI, graph layout, and local caches downstream",
            ],
            "tos_authority_scope": [
                "canon remains in ToS canon and source-owned atlas surfaces",
                "source witnesses and atlas rows remain the authored evidence route",
                "runtime graph state returns through an explicit ToS change route",
            ],
        },
        "validation_refs": list(VALIDATION_REFS),
        "counts": {
            "master_tables": len(atlas.get("master_tables", [])),
            "master_rows": row_count,
            "dossiers": len(dossier_rows),
            "dossier_node_rows": int(graph_shape.get("node_row_count") or 0),
            "dossier_relation_rows": int(graph_shape.get("relation_row_count") or 0),
            "candidate_nodes": len(candidate_nodes),
            "candidate_relations": len(candidate_relations),
            "candidate_endpoint_placeholders": len(endpoint_nodes),
            "graph_views": len(list(views_root.glob("*.graph.md"))),
            "nodes": len(nodes),
            "edges": len(edges),
            "diagnostics": len(diagnostics),
        },
        "nodes": nodes,
        "edges": edges,
        "diagnostics": diagnostics,
    }
    validate_payload_schema(payload)
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"

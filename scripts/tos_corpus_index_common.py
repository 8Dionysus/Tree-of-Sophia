#!/usr/bin/env python3
"""Shared builder helpers for the ToS whole-corpus index."""

from __future__ import annotations

import csv
import hashlib
import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
TOS_ROOT = REPO_ROOT / "ToS"
TOS_CORPUS_INDEX_PATH = TOS_ROOT / "derived-exports" / "tos_corpus_index.min.json"
SCHEMA_REF = "ToS/contracts/tos-corpus-index.schema.json"
SELF_REF = "ToS/derived-exports/tos_corpus_index.min.json"
VALIDATION_REFS = (
    "scripts/build_tos_corpus_index.py",
    "scripts/validate_tos_corpus_index.py",
    "tests/test_tos_corpus_index.py",
)

BRANCH_AUTHORITY = {
    "ToS": "source_home",
    "candidate-intake": "candidate_intake",
    "canon": "canon",
    "contracts": "contract",
    "derived-exports": "derived_export",
    "doctrine": "doctrine",
    "philosophy": "domain_topology",
    "public-compatibility": "public_compatibility",
    "research-packets": "research_packet",
    "review-ledger": "review_evidence",
    "source-witnesses": "source_witness",
    "zarathustra": "golden_route_orientation",
}

AUTHORITY_ORDER = (
    {
        "layer": "source_home",
        "owner_branch": "ToS",
        "meaning": "Tree of Sophia home surface, source-home manifest, and top-level home route cards",
    },
    {
        "layer": "source_witness",
        "owner_branch": "ToS/source-witnesses",
        "meaning": "source-facing witness and provenance surfaces",
    },
    {
        "layer": "golden_route_orientation",
        "owner_branch": "ToS/zarathustra",
        "meaning": "golden Zarathustra orientation route for the project's current living entry",
    },
    {
        "layer": "canon",
        "owner_branch": "ToS/canon",
        "meaning": "reviewed authored nodes, relation packs, and registries",
    },
    {
        "layer": "doctrine",
        "owner_branch": "ToS/doctrine",
        "meaning": "current ToS knowledge law, node contracts, templates, and interpretation discipline",
    },
    {
        "layer": "contract",
        "owner_branch": "ToS/contracts",
        "meaning": "public structural contracts for ToS-owned surfaces",
    },
    {
        "layer": "domain_topology",
        "owner_branch": "ToS/philosophy",
        "meaning": "branch-shaped philosophy topology and local graph workbench routes",
    },
    {
        "layer": "candidate_intake",
        "owner_branch": "ToS/candidate-intake",
        "meaning": "provisional extraction and promotion residue",
    },
    {
        "layer": "research_packet",
        "owner_branch": "ToS/research-packets",
        "meaning": "non-authoritative research scaffolds for later review",
    },
    {
        "layer": "review_evidence",
        "owner_branch": "ToS/review-ledger",
        "meaning": "dated inspection notes and review evidence for corpus growth",
    },
    {
        "layer": "public_compatibility",
        "owner_branch": "ToS/public-compatibility",
        "meaning": "public-safe mirrors and compatibility examples",
    },
    {
        "layer": "derived_export",
        "owner_branch": "ToS/derived-exports",
        "meaning": "generated downstream read models subordinate to ToS authority",
    },
    {
        "layer": "runtime_projection",
        "owner_branch": "abyss-stack",
        "meaning": "runtime access, visualization, MCP, UI, and projection stores only",
    },
)

GRAPH_VIEWS = (
    {
        "view_id": "corpus-topology",
        "purpose": "show the whole ToS home as a branch-shaped tree",
        "layout_hint": "elk-layered-or-graphviz-dot",
        "entry_surface": "ToS/source_home.manifest.json",
    },
    {
        "view_id": "authority-layers",
        "purpose": "switch corpus visibility by witness, research, candidate, canon, compatibility, and export layers",
        "layout_hint": "layered-filter",
        "entry_surface": "ToS/source_home.manifest.json",
    },
    {
        "view_id": "route-graph",
        "purpose": "inspect a concrete relation pack without losing its owner branch and provenance",
        "layout_hint": "directed-route-graph",
        "entry_surface": "ToS/canon/relations",
    },
    {
        "view_id": "node-neighborhood",
        "purpose": "expand around one node by bounded hops over the full corpus substrate",
        "layout_hint": "sigma-graphology-webgl",
        "entry_surface": "ToS/canon",
    },
    {
        "view_id": "provenance-dag",
        "purpose": "trace source witness or research packet pressure into candidate, canon, and export surfaces",
        "layout_hint": "dag",
        "entry_surface": "ToS/source-witnesses",
    },
    {
        "view_id": "promotion-flow",
        "purpose": "review candidate-intake material against canon promotion status",
        "layout_hint": "elk-layered-flow",
        "entry_surface": "ToS/candidate-intake",
    },
    {
        "view_id": "diff-snapshot",
        "purpose": "compare two corpus index snapshots for review",
        "layout_hint": "changed-subgraph",
        "entry_surface": "ToS/derived-exports/tos_corpus_index.min.json",
    },
)


def repo_ref(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{repo_ref(path)} must contain a JSON object")
    return payload


def owner_branch(path_ref: str) -> str:
    parts = Path(path_ref).parts
    if not parts or parts[0] != "ToS":
        return "repository"
    if len(parts) == 1:
        return "ToS"
    if len(parts) == 2 and (REPO_ROOT / path_ref).is_file():
        return "ToS"
    return f"ToS/{parts[1]}"


def authority_layer(path_ref: str) -> str:
    branch = owner_branch(path_ref)
    branch_name = branch.removeprefix("ToS/")
    return BRANCH_AUTHORITY.get(branch_name, "repository")


def resource_kind(path: Path) -> str:
    path_ref = repo_ref(path)
    name = path.name
    suffix = path.suffix.lower()
    if name == "AGENTS.md":
        return "route_card"
    if name == "source_home.manifest.json":
        return "source_home_manifest"
    if name == "philosophy.manifest.json":
        return "philosophy_manifest"
    if name == "branch.manifest.json":
        return "branch_manifest"
    if name == "node.json":
        return "node_payload"
    if name == "edges.csv":
        return "relation_pack"
    if path_ref.startswith("ToS/source-witnesses/"):
        return "source_witness"
    if path_ref.startswith("ToS/research-packets/"):
        return "research_packet"
    if path_ref.startswith("ToS/review-ledger/"):
        return "review_note"
    if path_ref.startswith("ToS/contracts/") and suffix == ".json":
        return "contract_schema"
    if path_ref.startswith("ToS/derived-exports/"):
        return "derived_export"
    if suffix == ".md":
        return "markdown"
    if suffix == ".csv":
        return "tabular"
    if suffix == ".json":
        return "json"
    return "binary" if suffix in {".xlsx", ".xls"} else suffix.lstrip(".") or "file"


def canonical_label(payload: dict[str, Any]) -> str:
    for key in ("canonical_label", "source_anchor", "distilled_thesis", "node_id"):
        value = payload.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return "unnamed-node"


def route_hint_for_node(path_ref: str) -> str | None:
    marker = "/friedrich-nietzsche/"
    if marker not in path_ref:
        return None
    return path_ref.split(marker, 1)[1].rsplit("/node.json", 1)[0]


def route_hint_for_edges(path_ref: str) -> str:
    return path_ref.rsplit("/edges.csv", 1)[0].split("/", 2)[-1]


def build_branches(source_home: dict[str, Any], diagnostics: list[dict[str, str]]) -> list[dict[str, str]]:
    branches: list[dict[str, str]] = []
    for branch in source_home.get("branches", []):
        if not isinstance(branch, dict):
            diagnostics.append({"level": "error", "path": "ToS/source_home.manifest.json", "message": "branch entry is not an object"})
            continue
        path_ref = str(branch.get("path") or "")
        owner_surface = str(branch.get("owner_surface") or "")
        if path_ref and not (REPO_ROOT / path_ref).is_dir():
            diagnostics.append({"level": "error", "path": path_ref, "message": "branch path declared by source_home manifest is missing"})
        if owner_surface and not (REPO_ROOT / owner_surface).is_file():
            diagnostics.append({"level": "error", "path": owner_surface, "message": "branch owner surface declared by source_home manifest is missing"})
        branches.append(
            {
                "id": str(branch.get("id") or ""),
                "path": path_ref,
                "owner_surface": owner_surface,
                "authority_layer": authority_layer(path_ref),
                "role": str(branch.get("role") or ""),
            }
        )
    return branches


def build_manifests(diagnostics: list[dict[str, str]]) -> list[dict[str, Any]]:
    manifests: list[dict[str, Any]] = []
    source_home_seen = False
    for path in sorted(TOS_ROOT.rglob("*.manifest.json")):
        path_ref = repo_ref(path)
        if path_ref == "ToS/source_home.manifest.json":
            source_home_seen = True
        try:
            payload = load_json(path)
        except (json.JSONDecodeError, ValueError) as exc:
            diagnostics.append({"level": "error", "path": path_ref, "message": str(exc)})
            continue
        declared_path = payload.get("path") if isinstance(payload.get("path"), str) else None
        if path_ref == "ToS/source_home.manifest.json" and isinstance(payload.get("home"), str):
            declared_path = payload.get("home")
        manifests.append(
            {
                "path": path_ref,
                "manifest_kind": resource_kind(path),
                "owner_branch": owner_branch(path_ref),
                "authority_layer": authority_layer(path_ref),
                "schema_version": str(payload.get("schema_version") or ""),
                "branch_id": payload.get("branch_id") if isinstance(payload.get("branch_id"), str) else None,
                "declared_path": declared_path,
                "sha256": sha256(path),
            }
        )
    source_home_path = TOS_ROOT / "source_home.manifest.json"
    source_home_ref = repo_ref(source_home_path)
    if not source_home_path.is_file():
        diagnostics.append({"level": "error", "path": source_home_ref, "message": "missing source-home manifest"})
    elif not source_home_seen:
        payload = load_json(source_home_path)
        manifests.insert(
            0,
            {
                "path": source_home_ref,
                "manifest_kind": "source_home_manifest",
                "owner_branch": "ToS",
                "authority_layer": "source_home",
                "schema_version": str(payload.get("schema_version") or ""),
                "branch_id": None,
                "declared_path": payload.get("home") if isinstance(payload.get("home"), str) else None,
                "sha256": sha256(source_home_path),
            },
        )
    return manifests


def build_nodes(diagnostics: list[dict[str, str]]) -> list[dict[str, Any]]:
    nodes: list[dict[str, Any]] = []
    for path in sorted(TOS_ROOT.rglob("node.json")):
        path_ref = repo_ref(path)
        try:
            payload = load_json(path)
        except (json.JSONDecodeError, ValueError) as exc:
            diagnostics.append({"level": "error", "path": path_ref, "message": str(exc)})
            continue
        node_id = payload.get("node_id")
        node_type = payload.get("node_type")
        if not isinstance(node_id, str) or not node_id:
            diagnostics.append({"level": "error", "path": path_ref, "message": "node payload is missing node_id"})
            continue
        if not isinstance(node_type, str) or not node_type:
            diagnostics.append({"level": "error", "path": path_ref, "message": "node payload is missing node_type"})
            continue
        nodes.append(
            {
                "node_id": node_id,
                "node_type": node_type,
                "label": canonical_label(payload),
                "owner_branch": owner_branch(path_ref),
                "authority_layer": authority_layer(path_ref),
                "source_path": path_ref,
                "source_sha256": sha256(path),
                "route_hint": route_hint_for_node(path_ref),
            }
        )
    return nodes


def read_edge_rows(path: Path) -> tuple[list[str], list[dict[str, str]]]:
    with path.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        return list(reader.fieldnames or []), list(reader)


def build_relations(diagnostics: list[dict[str, str]]) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    relation_packs: list[dict[str, Any]] = []
    relation_edges: list[dict[str, str]] = []
    for path in sorted(TOS_ROOT.rglob("edges.csv")):
        path_ref = repo_ref(path)
        try:
            columns, rows = read_edge_rows(path)
        except csv.Error as exc:
            diagnostics.append({"level": "error", "path": path_ref, "message": f"invalid CSV: {exc}"})
            continue
        pack_id = path_ref.rsplit("/edges.csv", 1)[0].removeprefix("ToS/")
        relation_packs.append(
            {
                "pack_id": pack_id,
                "path": path_ref,
                "route_hint": route_hint_for_edges(path_ref),
                "owner_branch": owner_branch(path_ref),
                "authority_layer": authority_layer(path_ref),
                "edge_count": len(rows),
                "columns": columns,
                "sha256": sha256(path),
            }
        )
        for row in rows:
            edge_id = str(row.get("edge_id") or f"{pack_id}:{len(relation_edges) + 1}")
            relation_edges.append(
                {
                    "edge_id": edge_id,
                    "pack_id": pack_id,
                    "from_id": str(row.get("from_id") or ""),
                    "predicate_id": str(row.get("predicate_id") or ""),
                    "to_id": str(row.get("to_id") or ""),
                    "owner_branch": owner_branch(path_ref),
                    "authority_layer": authority_layer(path_ref),
                    "layer": str(row.get("layer") or ""),
                    "status": str(row.get("status") or ("canon" if path_ref.startswith("ToS/canon/") else "unmarked")),
                }
            )
    return relation_packs, relation_edges


def build_resources() -> list[dict[str, Any]]:
    resources: list[dict[str, Any]] = []
    for path in sorted(TOS_ROOT.rglob("*")):
        if not path.is_file():
            continue
        path_ref = repo_ref(path)
        if path_ref == SELF_REF:
            continue
        resources.append(
            {
                "path": path_ref,
                "resource_kind": resource_kind(path),
                "owner_branch": owner_branch(path_ref),
                "authority_layer": authority_layer(path_ref),
                "sha256": sha256(path),
                "size_bytes": path.stat().st_size,
            }
        )
    return resources


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
    source_home = load_json(TOS_ROOT / "source_home.manifest.json")
    branches = build_branches(source_home, diagnostics)
    manifests = build_manifests(diagnostics)
    nodes = build_nodes(diagnostics)
    relation_packs, relation_edges = build_relations(diagnostics)
    resources = build_resources()
    payload: dict[str, Any] = {
        "schema_version": "tos_corpus_index_v1",
        "schema_ref": SCHEMA_REF,
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_corpus_index",
        "authority_order": list(AUTHORITY_ORDER),
        "runtime_projection_boundary": {
            "runtime_owner": "abyss-stack",
            "allowed": [
                "read ToS-owned corpus surfaces",
                "serve MCP resources and tools that point back to ToS",
                "build runtime graph projections, UI views, and Neo4j caches",
                "emit review diagnostics without changing ToS authority",
            ],
            "not_allowed": [
                "move canonical ToS meaning into abyss-stack",
                "treat Neo4j, MCP, UI, or runtime cache as source truth",
                "write ToS canon without ToS validators and explicit operator route",
            ],
        },
        "validation_refs": list(VALIDATION_REFS),
        "counts": {
            "branches": len(branches),
            "manifests": len(manifests),
            "nodes": len(nodes),
            "relation_packs": len(relation_packs),
            "relation_edges": len(relation_edges),
            "resources": len(resources),
            "diagnostics": len(diagnostics),
        },
        "graph_views": list(GRAPH_VIEWS),
        "branches": branches,
        "manifests": manifests,
        "nodes": nodes,
        "relation_packs": relation_packs,
        "relation_edges": relation_edges,
        "resources": resources,
        "diagnostics": diagnostics,
    }
    validate_payload_schema(payload)
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n"

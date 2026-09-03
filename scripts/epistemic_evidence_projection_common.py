#!/usr/bin/env python3
"""Build and validate the public-safe ToS Evidence Lens projection."""

from __future__ import annotations

import csv
import hashlib
import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_REF = "ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json"
SCHEMA_REF = "ToS/contracts/epistemic-evidence-projection.schema.json"
PROJECTION_PATH = REPO_ROOT / "ToS/derived-exports/epistemic_evidence_projection.min.json"
CANON_RELATION_REF = "ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv"


def load_json(ref: str) -> dict[str, Any]:
    payload = json.loads((REPO_ROOT / ref).read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{ref} must contain a JSON object")
    return payload


def digest(ref: str) -> str:
    return hashlib.sha256((REPO_ROOT / ref).read_bytes()).hexdigest()


def projection_ids() -> dict[tuple[str, str], set[str]]:
    philosophy = load_json("ToS/derived-exports/philosophy_graph_projection.min.json")
    philosophy_views: dict[str, set[str]] = {}
    for view in philosophy.get("views", []):
        if not isinstance(view, dict) or not isinstance(view.get("view_id"), str):
            continue
        ids = {
            str(item)
            for key in ("node_ids", "edge_ids")
            for item in view.get(key, [])
            if isinstance(item, str)
        }
        for key in ("nodes", "edges"):
            for item in view.get(key, []):
                if isinstance(item, dict):
                    identity = item.get("node_id") or item.get("edge_id")
                    if isinstance(identity, str):
                        ids.add(identity)
        philosophy_views[str(view["view_id"])] = ids

    corpus = load_json("ToS/derived-exports/tos_corpus_index.min.json")
    corpus_ids = {
        str(item.get("node_id"))
        for item in corpus.get("nodes", [])
        if isinstance(item, dict) and isinstance(item.get("node_id"), str)
    }
    corpus_ids.update(
        str(item.get("edge_id"))
        for item in corpus.get("relation_edges", [])
        if isinstance(item, dict)
        and item.get("owner_branch") == "ToS/canon"
        and isinstance(item.get("edge_id"), str)
    )
    result = {("corpus", "route-graph"): corpus_ids}
    result.update({("philosophy", view_id): ids for view_id, ids in philosophy_views.items()})
    return result


def canon_anchor_rows() -> dict[str, dict[str, str]]:
    with (REPO_ROOT / CANON_RELATION_REF).open(encoding="utf-8", newline="") as handle:
        return {str(row["edge_id"]): row for row in csv.DictReader(handle)}


def build_payload() -> dict[str, Any]:
    source = load_json(SOURCE_REF)
    known_ids = projection_ids()
    anchor_rows = canon_anchor_rows()
    scenes: list[dict[str, Any]] = []
    for raw_scene in source.get("scenes", []):
        if not isinstance(raw_scene, dict):
            raise ValueError("Evidence Lens scenes must be objects")
        selections = raw_scene.get("selections", [])
        selection_ids: list[str] = []
        for selection in selections:
            if not isinstance(selection, dict):
                raise ValueError("Evidence Lens selections must be objects")
            key = (str(selection.get("mode") or ""), str(selection.get("view_id") or ""))
            if key not in known_ids:
                raise ValueError(f"unknown Evidence Lens view: {key[0]}/{key[1]}")
            item_ids = [str(item) for item in selection.get("item_ids", [])]
            missing = sorted(set(item_ids) - known_ids[key])
            if missing:
                raise ValueError(f"{raw_scene.get('scene_id')}: selection IDs not in {key}: {missing}")
            selection_ids.extend(item_ids)

        routes: list[dict[str, Any]] = []
        for raw_route in raw_scene.get("routes", []):
            route = dict(raw_route)
            ref = str(route.get("ref") or "")
            if not ref or ref.startswith("/") or "/payload/" in ref:
                raise ValueError(f"unsafe Evidence Lens route ref: {ref!r}")
            path = REPO_ROOT / ref
            if not path.is_file():
                raise ValueError(f"missing Evidence Lens route ref: {ref}")
            route["exists"] = True
            route["sha256"] = digest(ref)
            routes.append(route)

        source_anchors: list[dict[str, Any]] = []
        for edge_id in raw_scene.get("anchor_edge_ids", []):
            row = anchor_rows.get(str(edge_id))
            if row is None:
                raise ValueError(f"unknown canonical anchor edge: {edge_id}")
            segments = [item for item in str(row.get("anchor_segment_ids") or "").split("|") if item]
            if not segments:
                raise ValueError(f"canonical edge has no segment anchors: {edge_id}")
            source_anchors.append(
                {
                    "edge_id": str(edge_id),
                    "anchor_segment_ids": segments,
                    "witness_scope": str(row.get("witness_scope") or ""),
                    "relation_ref": CANON_RELATION_REF,
                }
            )

        scene = {key: value for key, value in raw_scene.items() if key != "anchor_edge_ids"}
        scene["selection_ids"] = list(dict.fromkeys(selection_ids))
        scene["source_anchors"] = source_anchors
        scene["routes"] = routes
        scene["source_refs"] = sorted({SOURCE_REF, *(route["ref"] for route in routes)})
        scenes.append(scene)

    payload = {
        "schema_version": "tos_epistemic_evidence_projection_v1",
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived_public_evidence_navigation",
        "source_definition_ref": SOURCE_REF,
        "source_definition_sha256": digest(SOURCE_REF),
        "scenes": scenes,
        "authority_boundary": {
            "is_source": False,
            "is_canon": False,
            "is_semantic_truth": False,
            "is_rights_clearance": False,
            "note": "This projection joins explicit owner routes for inspection. The referenced source, review, canon, and rights surfaces retain authority.",
        },
    }
    validate_payload(payload)
    return payload


def validate_payload(payload: dict[str, Any]) -> None:
    validator = Draft202012Validator(load_json(SCHEMA_REF))
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.absolute_path))
    if errors:
        error = errors[0]
        path = ".".join(str(item) for item in error.absolute_path) or "<root>"
        raise ValueError(f"Evidence Lens schema violation at {path}: {error.message}")
    ids: list[str] = []
    for scene in payload.get("scenes", []):
        ids.extend(str(item) for item in scene.get("selection_ids", []))
        if scene.get("posture") == "contested-pre-canon" and scene.get("conclusion", {}).get("can_conclude"):
            raise ValueError("contested pre-canon scenes cannot be conclusive")
        if scene.get("conclusion", {}).get("claim_evidence_closed"):
            raise ValueError("v1 Evidence Lens scenes must not assert claim/evidence closure")
    duplicates = sorted({item for item in ids if ids.count(item) > 1})
    if duplicates:
        raise ValueError(f"selection IDs must bind to one scene only: {duplicates}")


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n"

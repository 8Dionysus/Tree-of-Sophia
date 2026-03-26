#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_NODE_PATH = REPO_ROOT / "examples" / "source_node.example.json"
CONCEPT_NODE_PATH = REPO_ROOT / "examples" / "concept_node.example.json"
TINY_ENTRY_ROUTE_PATH = REPO_ROOT / "docs" / "TINY_ENTRY_ROUTE.md"
CAPSULE_PATH = REPO_ROOT / "docs" / "ZARATHUSTRA_TRILINGUAL_ENTRY.md"
OUTPUT_PATH = REPO_ROOT / "generated" / "kag_export.json"
MIN_OUTPUT_PATH = REPO_ROOT / "generated" / "kag_export.min.json"

PRIMARY_QUESTION = (
    "What source-owned tiny export keeps the current Zarathustra prologue route "
    "legible for downstream KAG consumers without replacing ToS authority?"
)
SUMMARY_50 = "Source-owned tiny export for the current Zarathustra prologue authority route."
SUMMARY_200 = (
    "Source-owned tiny export capsule for the current Zarathustra prologue route, "
    "keeping the public compatibility entry surface aligned with the canonical "
    "tree while preserving the capsule and tiny-entry docs as supporting "
    "ToS-owned orientation surfaces."
)
PROVENANCE_NOTE = (
    "Guide to the current canonical tree node, its public compatibility mirror, "
    "and the supporting capsule and tiny-entry slice; it does not replace the "
    "authored source node."
)
NON_IDENTITY_BOUNDARY = (
    "Derived export capsule for downstream KAG consumers; ToS-authored authority "
    "remains in Tree-of-Sophia tree, source, and capsule surfaces."
)


def read_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def encode_json(payload: object, *, compact: bool) -> str:
    if compact:
        return json.dumps(payload, ensure_ascii=True, separators=(",", ":")) + "\n"
    return json.dumps(payload, ensure_ascii=True, indent=2) + "\n"


def build_kag_export_payload() -> dict[str, object]:
    source_payload = read_json(SOURCE_NODE_PATH)
    if not isinstance(source_payload, dict):
        raise RuntimeError("examples/source_node.example.json must be a JSON object")

    node_id = source_payload.get("node_id")
    interpretation_layers = source_payload.get("interpretation_layers")
    relations = source_payload.get("relations")
    if not isinstance(node_id, str) or not node_id:
        raise RuntimeError("examples/source_node.example.json must keep node_id")
    if not isinstance(interpretation_layers, list) or not interpretation_layers:
        raise RuntimeError("examples/source_node.example.json must keep interpretation_layers")
    if not isinstance(relations, list):
        raise RuntimeError("examples/source_node.example.json must keep relations")

    direct_relations = [
        {
            "relation_type": "bounded_hop",
            "target_ref": "Tree-of-Sophia/examples/concept_node.example.json",
        },
        {
            "relation_type": "capsule_surface",
            "target_ref": "Tree-of-Sophia/docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md",
        },
        {
            "relation_type": "tiny_entry_route",
            "target_ref": "Tree-of-Sophia/docs/TINY_ENTRY_ROUTE.md",
        },
    ]
    if not relations:
        raise RuntimeError("examples/source_node.example.json must keep at least one relation")

    section_handles = []
    for layer in interpretation_layers:
        if not isinstance(layer, str) or not layer:
            raise RuntimeError("interpretation_layers must contain non-empty strings")
        section_handles.append(layer)

    return {
        "owner_repo": "Tree-of-Sophia",
        "kind": "source_node",
        "object_id": node_id,
        "primary_question": PRIMARY_QUESTION,
        "summary_50": SUMMARY_50,
        "summary_200": SUMMARY_200,
        "source_inputs": [
            {
                "repo": "Tree-of-Sophia",
                "source_class": "tos_text",
                "role": "primary",
            },
            {
                "repo": "Tree-of-Sophia",
                "source_class": "review_surface",
                "role": "supporting",
            },
        ],
        "entry_surface": {
            "repo": "Tree-of-Sophia",
            "path": "examples/source_node.example.json",
            "match_key": "node_id",
            "match_value": node_id,
        },
        "section_handles": section_handles,
        "direct_relations": direct_relations,
        "provenance_note": PROVENANCE_NOTE,
        "non_identity_boundary": NON_IDENTITY_BOUNDARY,
    }


def write_outputs() -> list[Path]:
    payload = build_kag_export_payload()
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_text(encode_json(payload, compact=False), encoding="utf-8")
    MIN_OUTPUT_PATH.write_text(encode_json(payload, compact=True), encoding="utf-8")
    return [OUTPUT_PATH, MIN_OUTPUT_PATH]


def main() -> int:
    written = write_outputs()
    for path in written:
        print(f"[ok] wrote {path.relative_to(REPO_ROOT).as_posix()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

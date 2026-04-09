#!/usr/bin/env python3
"""Shared builder helpers for the ToS root entry capsule."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
ROOT_ENTRY_MAP_PATH = REPO_ROOT / "generated" / "root_entry_map.min.json"
SCHEMA_REF = "schemas/root-entry-map.schema.json"
VALIDATION_REFS = (
    "scripts/build_root_entry_map.py",
    "scripts/validate_root_entry_map.py",
    "tests/test_root_entry_map.py",
)
FORBIDDEN_LOW_CONTEXT_PREFIXES = ("src/", "scripts/")

SURFACE_PAYLOAD = {
    "schema_version": "tos_root_entry_map_v1",
    "schema_ref": SCHEMA_REF,
    "owner_repo": "Tree-of-Sophia",
    "surface_kind": "root_entry_map",
    "authority_ref": "CHARTER.md",
    "public_root_ref": "README.md",
    "current_tiny_entry_ref": "examples/tos_tiny_entry_route.example.json",
    "export_ref": "generated/kag_export.min.json",
    "validation_refs": list(VALIDATION_REFS),
}

ROUTES = (
    {
        "route_id": "current-tiny-entry",
        "need": "enter the current bounded ToS route through the source-owned tiny-entry seam",
        "surface_ref": "examples/tos_tiny_entry_route.example.json",
        "verification_refs": [
            "docs/TINY_ENTRY_ROUTE.md",
            "docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md",
        ],
    },
    {
        "route_id": "tree-first-model",
        "need": "restore the tree-first model and repository boundary before following derived downstream layers",
        "surface_ref": "docs/KNOWLEDGE_MODEL.md",
        "verification_refs": ["CHARTER.md", "BOUNDARIES.md"],
    },
    {
        "route_id": "bounded-export",
        "need": "inspect the current bounded downstream export seam without mistaking it for ToS authority",
        "surface_ref": "generated/kag_export.min.json",
        "verification_refs": ["docs/KAG_EXPORT.md", "examples/source_node.example.json"],
    },
)


def resolve_local_ref(value: str) -> Path:
    target_path = REPO_ROOT / value
    if not target_path.exists():
        raise ValueError(f"missing ref target '{value}'")
    return target_path


def validate_low_context_local_ref(value: str, location: str) -> Path:
    path_text, _, anchor = value.partition("#")
    for prefix in FORBIDDEN_LOW_CONTEXT_PREFIXES:
        if path_text.startswith(prefix):
            raise ValueError(f"{location} must not point to implementation path '{value}'")
    target_path = resolve_local_ref(path_text)
    if anchor and target_path.suffix.lower() != ".md":
        raise ValueError(f"{location} may only use anchors for markdown refs")
    return target_path


def load_schema() -> dict[str, object]:
    return json.loads(resolve_local_ref(SCHEMA_REF).read_text(encoding="utf-8"))


def validate_payload_schema(payload: dict[str, object]) -> None:
    validator = Draft202012Validator(load_schema())
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.absolute_path))
    if not errors:
        return
    error = errors[0]
    path = "".join(f"[{item}]" if isinstance(item, int) else f".{item}" for item in error.absolute_path)
    if path.startswith("."):
        path = path[1:]
    if path:
        raise ValueError(f"schema violation at '{path}': {error.message}")
    raise ValueError(f"schema violation: {error.message}")


def build_payload() -> dict[str, object]:
    for key in (
        "schema_ref",
        "authority_ref",
        "public_root_ref",
        "current_tiny_entry_ref",
        "export_ref",
    ):
        validate_low_context_local_ref(str(SURFACE_PAYLOAD[key]), f"surface.{key}")
    for ref in SURFACE_PAYLOAD["validation_refs"]:
        resolve_local_ref(ref)

    routes: list[dict[str, object]] = []
    for route in ROUTES:
        validate_low_context_local_ref(route["surface_ref"], f"route:{route['route_id']}.surface_ref")
        for ref in route["verification_refs"]:
            validate_low_context_local_ref(ref, f"route:{route['route_id']}.verification_refs")
        routes.append(
            {
                "route_id": route["route_id"],
                "need": route["need"],
                "surface_ref": route["surface_ref"],
                "verification_refs": list(route["verification_refs"]),
            }
        )

    payload = {
        **SURFACE_PAYLOAD,
        "routes": routes,
    }
    validate_payload_schema(payload)
    return payload


def render_payload(payload: dict[str, object]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"

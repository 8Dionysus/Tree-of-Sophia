#!/usr/bin/env python3
"""Shared builder helpers for the ToS root entry capsule."""

from __future__ import annotations

import json
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
ROOT_ENTRY_MAP_PATH = REPO_ROOT / "ToS" / "derived-exports" / "root_entry_map.min.json"
SCHEMA_REF = "ToS/contracts/root-entry-map.schema.json"
SOURCE_PATH = REPO_ROOT / "scripts/root_entry_map.source.json"
FORBIDDEN_LOW_CONTEXT_PREFIXES = ("src/", "scripts/")


def source_payload() -> dict[str, object]:
    """The one authored generator input, shared with the native held-root reader."""
    return json.loads(SOURCE_PATH.read_text(encoding="utf-8"))


# Retained comparison API names expose the same owner input; build_payload reloads
# it on every call so an edited declaration cannot hide behind an imported cache.
_source = source_payload()
ARTIFACT_IDENTITY = _source["artifact_identity"]
SURFACE_PAYLOAD = {key: value for key, value in _source.items() if key not in ("artifact_identity", "routes")}
VALIDATION_REFS = tuple(SURFACE_PAYLOAD["validation_refs"])
ROUTES = tuple(_source["routes"])
CORE_ROUTE_IDS = frozenset(route["route_id"] for route in ROUTES)


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
    payload = source_payload()
    validate_payload_schema(payload)
    for key in ("schema_ref", "authority_ref", "public_root_ref", "current_tiny_entry_ref", "export_ref"):
        validate_low_context_local_ref(str(payload[key]), f"surface.{key}")
    for ref in payload["validation_refs"]:
        resolve_local_ref(ref)
    for route in payload["routes"]:
        validate_low_context_local_ref(route["surface_ref"], f"route:{route['route_id']}.surface_ref")
        for ref in route["verification_refs"]:
            validate_low_context_local_ref(ref, f"route:{route['route_id']}.verification_refs")
    return payload


def render_payload(payload: dict[str, object]) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"

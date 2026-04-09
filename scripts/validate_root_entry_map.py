#!/usr/bin/env python3
"""Validate the ToS root entry capsule against live route posture."""

from __future__ import annotations

import json

from root_entry_map_common import (
    ROOT_ENTRY_MAP_PATH,
    ROUTES,
    SURFACE_PAYLOAD,
    build_payload,
    resolve_local_ref,
    validate_low_context_local_ref,
    validate_payload_schema,
)


def main() -> int:
    expected_payload = build_payload()
    current_payload = json.loads(ROOT_ENTRY_MAP_PATH.read_text(encoding="utf-8"))
    validate_payload_schema(current_payload)
    if current_payload != expected_payload:
        raise SystemExit("generated/root_entry_map.min.json does not match the canonical rebuild")

    for key, expected in SURFACE_PAYLOAD.items():
        if current_payload.get(key) != expected:
            raise SystemExit(f"generated/root_entry_map.min.json must keep {key}={expected!r}")
        if key == "validation_refs":
            for ref in expected:
                resolve_local_ref(ref)
        elif key in {
            "schema_ref",
            "authority_ref",
            "public_root_ref",
            "current_tiny_entry_ref",
            "export_ref",
        }:
            resolve_local_ref(str(expected))

    routes = current_payload.get("routes")
    if not isinstance(routes, list) or len(routes) != len(ROUTES):
        raise SystemExit("generated/root_entry_map.min.json must publish exactly three root-entry routes")

    for route in routes:
        if not isinstance(route, dict):
            raise SystemExit("generated/root_entry_map.min.json routes must be objects")
        for key in ("route_id", "need", "surface_ref", "verification_refs"):
            value = route.get(key)
            if not value:
                raise SystemExit(f"generated/root_entry_map.min.json is missing route field '{key}'")
        validate_low_context_local_ref(route["surface_ref"], f"route:{route['route_id']}.surface_ref")
        verification_refs = route["verification_refs"]
        if not isinstance(verification_refs, list) or not verification_refs:
            raise SystemExit("generated/root_entry_map.min.json verification_refs must be a non-empty list")
        for ref in verification_refs:
            if not isinstance(ref, str) or not ref.strip():
                raise SystemExit("generated/root_entry_map.min.json verification_refs must contain strings")
            validate_low_context_local_ref(ref, f"route:{route['route_id']}.verification_refs")

    print("[ok] validated generated/root_entry_map.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

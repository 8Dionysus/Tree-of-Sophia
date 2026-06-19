#!/usr/bin/env python3
"""Validate the ToS root entry capsule against live route posture."""

from __future__ import annotations

import json
import shlex

from root_entry_map_common import (
    ARTIFACT_IDENTITY,
    CORE_ROUTE_IDS,
    ROOT_ENTRY_MAP_PATH,
    SURFACE_PAYLOAD,
    build_payload,
    resolve_local_ref,
    validate_low_context_local_ref,
    validate_payload_schema,
)


def validate_artifact_identity(identity: object) -> None:
    if not isinstance(identity, dict):
        raise SystemExit("ToS/derived-exports/root_entry_map.min.json artifact_identity must be an object")
    if identity != ARTIFACT_IDENTITY:
        raise SystemExit("ToS/derived-exports/root_entry_map.min.json artifact_identity must match the canonical contract")

    for key in ("authority_ref", "contract_version"):
        validate_low_context_local_ref(str(identity[key]), f"artifact_identity.{key}")

    verification = identity.get("verification")
    if not isinstance(verification, list) or not verification:
        raise SystemExit("ToS/derived-exports/root_entry_map.min.json artifact_identity.verification must be non-empty")

    for command in verification:
        if not isinstance(command, str) or not command.strip():
            raise SystemExit("ToS/derived-exports/root_entry_map.min.json artifact_identity.verification must contain strings")
        try:
            parts = shlex.split(command)
        except ValueError as exc:
            raise SystemExit(
                "ToS/derived-exports/root_entry_map.min.json artifact_identity.verification has invalid shell syntax"
            ) from exc
        for part in parts[1:]:
            if part.endswith(".py") and not part.startswith("-"):
                resolve_local_ref(part)


def main() -> int:
    expected_payload = build_payload()
    current_payload = json.loads(ROOT_ENTRY_MAP_PATH.read_text(encoding="utf-8"))
    validate_payload_schema(current_payload)
    if current_payload != expected_payload:
        raise SystemExit("ToS/derived-exports/root_entry_map.min.json does not match the canonical rebuild")

    for key, expected in SURFACE_PAYLOAD.items():
        if current_payload.get(key) != expected:
            raise SystemExit(f"ToS/derived-exports/root_entry_map.min.json must keep {key}={expected!r}")
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

    validate_artifact_identity(current_payload.get("artifact_identity"))

    routes = current_payload.get("routes")
    if not isinstance(routes, list) or not routes:
        raise SystemExit("ToS/derived-exports/root_entry_map.min.json must publish root-entry routes")

    route_ids: set[str] = set()
    for route in routes:
        if not isinstance(route, dict):
            raise SystemExit("ToS/derived-exports/root_entry_map.min.json routes must be objects")
        route_id = route.get("route_id")
        if not isinstance(route_id, str) or not route_id.strip():
            raise SystemExit("ToS/derived-exports/root_entry_map.min.json routes must keep non-empty route_id values")
        if route_id in route_ids:
            raise SystemExit(f"ToS/derived-exports/root_entry_map.min.json has duplicate route_id {route_id!r}")
        route_ids.add(route_id)
        for key in ("need", "surface_ref", "verification_refs"):
            value = route.get(key)
            if not value:
                raise SystemExit(f"ToS/derived-exports/root_entry_map.min.json is missing route field '{key}'")
        validate_low_context_local_ref(route["surface_ref"], f"route:{route_id}.surface_ref")
        verification_refs = route["verification_refs"]
        if not isinstance(verification_refs, list) or not verification_refs:
            raise SystemExit("ToS/derived-exports/root_entry_map.min.json verification_refs must be a non-empty list")
        for ref in verification_refs:
            if not isinstance(ref, str) or not ref.strip():
                raise SystemExit("ToS/derived-exports/root_entry_map.min.json verification_refs must contain strings")
            validate_low_context_local_ref(ref, f"route:{route_id}.verification_refs")

    missing_core_routes = sorted(CORE_ROUTE_IDS - route_ids)
    if missing_core_routes:
        joined = ", ".join(missing_core_routes)
        raise SystemExit(f"ToS/derived-exports/root_entry_map.min.json is missing core root-entry routes: {joined}")

    print("[ok] validated ToS/derived-exports/root_entry_map.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

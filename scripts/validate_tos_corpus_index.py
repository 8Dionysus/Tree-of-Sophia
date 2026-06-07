#!/usr/bin/env python3
"""Validate the ToS whole-corpus index derived export."""

from __future__ import annotations

import json

from tos_corpus_index_common import TOS_CORPUS_INDEX_PATH, build_payload, render_payload, validate_payload_schema


def error_diagnostics(payload: dict[str, object]) -> list[dict[str, object]]:
    errors: list[dict[str, object]] = []
    for diagnostic in payload.get("diagnostics", []):
        if isinstance(diagnostic, dict) and diagnostic.get("level") == "error":
            errors.append(diagnostic)
    return errors


def require_no_error_diagnostics(payload: dict[str, object], label: str) -> None:
    errors = error_diagnostics(payload)
    if errors:
        preview = "; ".join(
            f"{error.get('path', '<unknown>')}: {error.get('message', '<missing message>')}"
            for error in errors[:5]
        )
        suffix = "" if len(errors) <= 5 else f"; +{len(errors) - 5} more"
        raise SystemExit(f"{label} contains error diagnostics: {preview}{suffix}")


def require_declared_authority_layers(payload: dict[str, object]) -> None:
    authority_order = payload.get("authority_order", [])
    declared = {
        entry.get("layer")
        for entry in authority_order
        if isinstance(entry, dict) and isinstance(entry.get("layer"), str)
    }
    emitted: set[str] = set()
    for collection_name in (
        "branches",
        "manifests",
        "nodes",
        "relation_packs",
        "relation_edges",
        "resources",
    ):
        collection = payload.get(collection_name, [])
        if not isinstance(collection, list):
            continue
        for item in collection:
            if isinstance(item, dict) and isinstance(item.get("authority_layer"), str):
                emitted.add(item["authority_layer"])
    missing = sorted(emitted - declared)
    if missing:
        raise SystemExit(f"ToS corpus index emits undeclared authority layers: {', '.join(missing)}")


def main() -> int:
    expected_payload = build_payload()
    current_payload = json.loads(TOS_CORPUS_INDEX_PATH.read_text(encoding="utf-8"))
    validate_payload_schema(current_payload)
    require_no_error_diagnostics(expected_payload, "rebuilt ToS corpus index")
    require_no_error_diagnostics(current_payload, "committed ToS corpus index")
    require_declared_authority_layers(current_payload)
    if render_payload(current_payload) != render_payload(expected_payload):
        raise SystemExit("ToS/derived-exports/tos_corpus_index.min.json does not match the canonical rebuild")

    counts = current_payload.get("counts", {})
    if counts.get("branches", 0) < 10:
        raise SystemExit("ToS corpus index must include the full source-home branch set")
    if counts.get("nodes", 0) == 0:
        raise SystemExit("ToS corpus index must include canonical node payloads")
    if counts.get("relation_packs", 0) == 0:
        raise SystemExit("ToS corpus index must include relation packs")
    if counts.get("resources", 0) == 0:
        raise SystemExit("ToS corpus index must include resource file entries")

    runtime_boundary = current_payload.get("runtime_projection_boundary", {})
    if runtime_boundary.get("runtime_owner") != "abyss-stack":
        raise SystemExit("ToS corpus index must keep runtime projection ownership in abyss-stack")

    print("[ok] validated ToS/derived-exports/tos_corpus_index.min.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

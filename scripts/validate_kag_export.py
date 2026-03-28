#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path

from generate_kag_export import (
    CAPSULE_PATH,
    CONCEPT_NODE_PATH,
    MIN_OUTPUT_PATH,
    OUTPUT_PATH,
    REPO_ROOT,
    SOURCE_NODE_PATH,
    TINY_ENTRY_ROUTE_PATH,
    build_kag_export_payload,
    encode_json,
)
from validate_intake_pack import run_validation as run_intake_pack_validation
from validate_nested_agents import run_validation as run_nested_agents_validation
from validate_tree_example_sync import run_validation as run_tree_example_sync_validation
from validate_tree_node_contracts import run_validation as run_tree_node_contracts_validation
from validate_tree_relation_pack import run_validation as run_tree_relation_pack_validation


class ValidationError(RuntimeError):
    pass


def fail(message: str) -> None:
    raise ValidationError(message)


def read_json(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"missing required file: {path.relative_to(REPO_ROOT).as_posix()}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path.relative_to(REPO_ROOT).as_posix()}: {exc}")


def validate_generated_text(path: Path, expected_text: str, *, label: str) -> None:
    try:
        actual_text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        fail(f"{label} is missing at {path.relative_to(REPO_ROOT).as_posix()}")
    if actual_text != expected_text:
        fail(f"{label} is out of date; run python scripts/generate_kag_export.py")


def validate_nested_agents_docs() -> None:
    issues = run_nested_agents_validation(REPO_ROOT)
    if issues:
        details = "\n".join(f"- {location}: {message}" for location, message in issues)
        fail(f"nested AGENTS docs check failed:\n{details}")


def validate_tree_example_sync() -> None:
    issues = run_tree_example_sync_validation(REPO_ROOT)
    if issues:
        details = "\n".join(f"- {location}: {message}" for location, message in issues)
        fail(f"tree/example sync check failed:\n{details}")


def validate_tree_node_contracts() -> None:
    issues = run_tree_node_contracts_validation(REPO_ROOT)
    if issues:
        details = "\n".join(f"- {location}: {message}" for location, message in issues)
        fail(f"tree node contract check failed:\n{details}")


def validate_tree_relation_pack() -> None:
    issues = run_tree_relation_pack_validation(REPO_ROOT)
    if issues:
        details = "\n".join(f"- {location}: {message}" for location, message in issues)
        fail(f"tree relation-pack check failed:\n{details}")


def validate_intake_pack() -> None:
    issues = run_intake_pack_validation(REPO_ROOT)
    if issues:
        details = "\n".join(f"- {location}: {message}" for location, message in issues)
        fail(f"intake pack check failed:\n{details}")


def validate_export_payload(payload: object) -> None:
    if not isinstance(payload, dict):
        fail("generated KAG export must be a JSON object")

    for key in (
        "owner_repo",
        "kind",
        "object_id",
        "primary_question",
        "summary_50",
        "summary_200",
        "source_inputs",
        "entry_surface",
        "section_handles",
        "direct_relations",
        "provenance_note",
        "non_identity_boundary",
    ):
        if key not in payload:
            fail(f"generated KAG export is missing required key '{key}'")

    if payload["owner_repo"] != "Tree-of-Sophia":
        fail("generated KAG export owner_repo must equal 'Tree-of-Sophia'")
    if payload["kind"] != "source_node":
        fail("generated KAG export kind must equal 'source_node'")

    source_payload = read_json(SOURCE_NODE_PATH)
    if not isinstance(source_payload, dict):
        fail("examples/source_node.example.json must be a JSON object")
    node_id = source_payload.get("node_id")
    if payload["object_id"] != node_id:
        fail("generated KAG export object_id must stay aligned with examples/source_node.example.json")

    entry_surface = payload["entry_surface"]
    if not isinstance(entry_surface, dict):
        fail("generated KAG export entry_surface must be an object")
    expected_entry_surface = {
        "repo": "Tree-of-Sophia",
        "path": "examples/source_node.example.json",
        "match_key": "node_id",
        "match_value": node_id,
    }
    if entry_surface != expected_entry_surface:
        fail("generated KAG export entry_surface must stay aligned with the current authority surface")

    for path in (SOURCE_NODE_PATH, CONCEPT_NODE_PATH, TINY_ENTRY_ROUTE_PATH, CAPSULE_PATH):
        if not path.exists():
            fail(f"required source-owned KAG export surface is missing: {path.relative_to(REPO_ROOT).as_posix()}")

    source_inputs = payload["source_inputs"]
    if not isinstance(source_inputs, list) or len(source_inputs) != 2:
        fail("generated KAG export source_inputs must contain one primary and one supporting input")
    primary_count = 0
    for index, source_input in enumerate(source_inputs):
        location = f"generated KAG export source_inputs[{index}]"
        if not isinstance(source_input, dict):
            fail(f"{location} must be an object")
        role = source_input.get("role")
        if role == "primary":
            primary_count += 1
        if source_input.get("repo") != "Tree-of-Sophia":
            fail(f"{location}.repo must equal 'Tree-of-Sophia'")
    if primary_count != 1:
        fail("generated KAG export source_inputs must contain exactly one primary input")

    section_handles = payload["section_handles"]
    expected_section_handles = source_payload.get("interpretation_layers")
    if section_handles != expected_section_handles:
        fail("generated KAG export section_handles must mirror source_node interpretation_layers")

    direct_relations = payload["direct_relations"]
    if not isinstance(direct_relations, list) or len(direct_relations) != 3:
        fail("generated KAG export direct_relations must contain the bounded authority-supporting route set")
    expected_refs = {
        "Tree-of-Sophia/examples/concept_node.example.json",
        "Tree-of-Sophia/docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md",
        "Tree-of-Sophia/docs/TINY_ENTRY_ROUTE.md",
    }
    actual_refs = set()
    for index, relation in enumerate(direct_relations):
        location = f"generated KAG export direct_relations[{index}]"
        if not isinstance(relation, dict):
            fail(f"{location} must be an object")
        relation_type = relation.get("relation_type")
        target_ref = relation.get("target_ref")
        if not isinstance(relation_type, str) or not relation_type:
            fail(f"{location}.relation_type must be a non-empty string")
        if not isinstance(target_ref, str) or not target_ref:
            fail(f"{location}.target_ref must be a non-empty string")
        actual_refs.add(target_ref)
    if actual_refs != expected_refs:
        fail("generated KAG export direct_relations must stay aligned with the current bounded hop and supporting doctrine refs")


def main() -> int:
    try:
        validate_nested_agents_docs()
        validate_intake_pack()
        validate_tree_node_contracts()
        validate_tree_relation_pack()
        validate_tree_example_sync()
        expected_payload = build_kag_export_payload()
        validate_generated_text(
            OUTPUT_PATH,
            encode_json(expected_payload, compact=False),
            label="generated KAG export",
        )
        validate_generated_text(
            MIN_OUTPUT_PATH,
            encode_json(expected_payload, compact=True),
            label="generated compact KAG export",
        )
        validate_export_payload(read_json(MIN_OUTPUT_PATH))
    except ValidationError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    print("[ok] validated nested AGENTS docs surfaces")
    print("[ok] validated v6.1 tabular base intake pack")
    print("[ok] validated canonical tree node payloads against the node contract")
    print("[ok] validated route-local canonical relation pack")
    print("[ok] validated tree/example compatibility mirrors")
    print("[ok] validated generated KAG export outputs are up to date")
    print("[ok] validated generated KAG export structure")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

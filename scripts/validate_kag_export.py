#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, Sequence

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
import yaml


class ValidationError(RuntimeError):
    pass


QUESTBOOK_PATH = Path("QUESTBOOK.md")
QUESTBOOK_INTEGRATION_PATH = Path("docs") / "QUESTBOOK_TOS_INTEGRATION.md"
QUEST_SCHEMA_PATH = Path("schemas") / "quest.schema.json"
QUEST_DISPATCH_SCHEMA_PATH = Path("schemas") / "quest_dispatch.schema.json"
QUEST_CATALOG_EXAMPLE_PATH = Path("examples") / "quest_catalog.min.example.json"
QUEST_DISPATCH_EXAMPLE_PATH = Path("examples") / "quest_dispatch.min.example.json"
QUEST_IDS = (
    "TOS-Q-0001",
    "TOS-Q-0002",
    "TOS-Q-0003",
    "TOS-Q-0004",
)
QUESTBOOK_REQUIRED_TOKENS = (
    "operational obligations that belong to `Tree-of-Sophia`",
    "philosophical truth claims as backlog items",
    "collapsing ToS meaning into AoA operational language",
    "examples/quest_catalog.min.example.json",
    "reviewable examples",
)
QUESTBOOK_FORBIDDEN_TOKENS = ("ATM10-Agent", "aoa-sdk")
QUESTBOOK_INTEGRATION_REQUIRED_TOKENS = (
    "operational obligations in the source-first knowledge architecture",
    "philosophical interpretation, authored knowledge, or source meaning becomes a task list",
    "source, identifier, and node-template hardening",
    "bounded ToS to `aoa-kag` bridge requests",
    "compatibility artifacts only",
    "do not replace authored doctrine",
)
QUESTBOOK_INTEGRATION_FORBIDDEN_TOKENS = ("ATM10-Agent", "aoa-sdk")
CLOSED_QUEST_STATES = {"done", "dropped"}
QUEST_SCHEMA_REQUIRED_FIELDS = (
    "schema_version",
    "id",
    "title",
    "repo",
    "owner_surface",
    "kind",
    "state",
    "band",
    "difficulty",
    "risk",
    "control_mode",
    "delegate_tier",
    "write_scope",
    "activation",
    "anchor_ref",
    "evidence",
    "opened_at",
    "touched_at",
    "public_safe",
)
QUEST_DISPATCH_REQUIRED_FIELDS = (
    "schema_version",
    "id",
    "repo",
    "state",
    "band",
    "difficulty",
    "risk",
    "control_mode",
    "delegate_tier",
    "split_required",
    "write_scope",
    "activation_mode",
    "public_safe",
)


def fail(message: str) -> None:
    raise ValidationError(message)


def read_json(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"missing required file: {path.relative_to(REPO_ROOT).as_posix()}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path.relative_to(REPO_ROOT).as_posix()}: {exc}")


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        fail(f"missing required file: {path.relative_to(REPO_ROOT).as_posix()}")


def read_yaml(path: Path) -> object:
    try:
        return yaml.safe_load(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"missing required file: {path.relative_to(REPO_ROOT).as_posix()}")
    except yaml.YAMLError as exc:
        fail(f"invalid YAML in {path.relative_to(REPO_ROOT).as_posix()}: {exc}")


def validate_generated_text(path: Path, expected_text: str, *, label: str) -> None:
    try:
        actual_text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        fail(f"{label} is missing at {path.relative_to(REPO_ROOT).as_posix()}")
    if actual_text != expected_text:
        fail(f"{label} is out of date; run python scripts/generate_kag_export.py")


def validate_quest_schema_envelope(
    payload: object,
    *,
    title: str,
    required_fields: Sequence[str],
    schema_version: str,
    label: str,
) -> None:
    if not isinstance(payload, dict):
        fail(f"{label} must be a JSON object")
    if payload.get("title") != title:
        fail(f"{label} title must equal '{title}'")
    if payload.get("type") != "object":
        fail(f"{label} type must equal 'object'")
    if payload.get("additionalProperties") is not False:
        fail(f"{label} must set additionalProperties to false")

    required = payload.get("required")
    if required != list(required_fields):
        fail(f"{label} required fields must stay aligned with the local quest contract")

    properties = payload.get("properties")
    if not isinstance(properties, dict):
        fail(f"{label} properties must be an object")

    version_payload = properties.get("schema_version")
    if not isinstance(version_payload, dict) or version_payload.get("const") != schema_version:
        fail(f"{label} schema_version.const must equal '{schema_version}'")


def build_expected_quest_catalog_entry(quest_id: str, payload: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": quest_id,
        "title": payload["title"],
        "repo": payload["repo"],
        "theme_ref": payload.get("theme_ref", ""),
        "milestone_ref": payload.get("milestone_ref", ""),
        "state": payload["state"],
        "band": payload["band"],
        "kind": payload["kind"],
        "difficulty": payload["difficulty"],
        "risk": payload["risk"],
        "owner_surface": payload["owner_surface"],
        "source_path": f"quests/{quest_id}.yaml",
        "public_safe": payload["public_safe"],
    }


def build_expected_quest_dispatch_entry(quest_id: str, payload: dict[str, Any]) -> dict[str, Any]:
    if quest_id == "TOS-Q-0002":
        requires_artifacts = [
            "bounded_plan",
            "guardrail_check",
            "verification_result",
        ]
    else:
        requires_artifacts = [
            "bounded_plan",
            "work_result",
            "verification_result",
        ]

    activation = payload.get("activation")
    if not isinstance(activation, dict):
        fail(f"quest {quest_id} activation must be an object")

    return {
        "schema_version": "quest_dispatch_v1",
        "id": quest_id,
        "repo": payload["repo"],
        "state": payload["state"],
        "band": payload["band"],
        "difficulty": payload["difficulty"],
        "risk": payload["risk"],
        "control_mode": payload["control_mode"],
        "delegate_tier": payload["delegate_tier"],
        "split_required": payload["split_required"],
        "write_scope": payload["write_scope"],
        "requires_artifacts": requires_artifacts,
        "activation_mode": activation["mode"],
        "source_path": f"quests/{quest_id}.yaml",
        "public_safe": payload["public_safe"],
        "fallback_tier": payload["fallback_tier"],
        "wrapper_class": payload["wrapper_class"],
    }


def validate_questbook_surface() -> None:
    required_paths = (
        QUESTBOOK_PATH,
        QUESTBOOK_INTEGRATION_PATH,
        QUEST_SCHEMA_PATH,
        QUEST_DISPATCH_SCHEMA_PATH,
        QUEST_CATALOG_EXAMPLE_PATH,
        QUEST_DISPATCH_EXAMPLE_PATH,
    ) + tuple(Path("quests") / f"{quest_id}.yaml" for quest_id in QUEST_IDS)

    for relative_path in required_paths:
        path = REPO_ROOT / relative_path
        if not path.exists():
            fail(f"missing required file: {relative_path.as_posix()}")

    questbook_text = read_text(REPO_ROOT / QUESTBOOK_PATH)
    for token in QUESTBOOK_REQUIRED_TOKENS:
        if token not in questbook_text:
            fail(f"QUESTBOOK.md must contain '{token}'")
    for token in QUESTBOOK_FORBIDDEN_TOKENS:
        if token in questbook_text:
            fail(f"QUESTBOOK.md must not mention '{token}'")

    integration_text = read_text(REPO_ROOT / QUESTBOOK_INTEGRATION_PATH)
    for token in QUESTBOOK_INTEGRATION_REQUIRED_TOKENS:
        if token not in integration_text:
            fail(f"{QUESTBOOK_INTEGRATION_PATH.as_posix()} must contain '{token}'")
    for token in QUESTBOOK_INTEGRATION_FORBIDDEN_TOKENS:
        if token in integration_text:
            fail(f"{QUESTBOOK_INTEGRATION_PATH.as_posix()} must not mention '{token}'")

    quest_schema_payload = read_json(REPO_ROOT / QUEST_SCHEMA_PATH)
    validate_quest_schema_envelope(
        quest_schema_payload,
        title="Tree-of-Sophia work_quest_v1",
        required_fields=QUEST_SCHEMA_REQUIRED_FIELDS,
        schema_version="work_quest_v1",
        label=QUEST_SCHEMA_PATH.as_posix(),
    )

    dispatch_schema_payload = read_json(REPO_ROOT / QUEST_DISPATCH_SCHEMA_PATH)
    validate_quest_schema_envelope(
        dispatch_schema_payload,
        title="Tree-of-Sophia quest_dispatch_v1",
        required_fields=QUEST_DISPATCH_REQUIRED_FIELDS,
        schema_version="quest_dispatch_v1",
        label=QUEST_DISPATCH_SCHEMA_PATH.as_posix(),
    )

    expected_catalog = []
    expected_dispatch = []
    active_quest_ids: list[str] = []
    closed_quest_ids: list[str] = []
    for quest_id in QUEST_IDS:
        quest_path = REPO_ROOT / "quests" / f"{quest_id}.yaml"
        quest_payload = read_yaml(quest_path)
        if not isinstance(quest_payload, dict):
            fail(f"{quest_path.relative_to(REPO_ROOT).as_posix()} must be a YAML object")
        if quest_payload.get("schema_version") != "work_quest_v1":
            fail(f"{quest_id} schema_version must equal 'work_quest_v1'")
        if quest_payload.get("id") != quest_id:
            fail(f"{quest_path.relative_to(REPO_ROOT).as_posix()} id must equal '{quest_id}'")
        if quest_payload.get("repo") != "Tree-of-Sophia":
            fail(f"{quest_id} repo must equal 'Tree-of-Sophia'")
        if quest_payload.get("public_safe") is not True:
            fail(f"{quest_id} public_safe must be true")
        if quest_payload.get("state") in CLOSED_QUEST_STATES:
            closed_quest_ids.append(quest_id)
        else:
            active_quest_ids.append(quest_id)
        notes = quest_payload.get("notes", "")
        if not isinstance(notes, str):
            fail(f"{quest_id} notes must be a string")
        if "ATM10-Agent" in notes or "aoa-sdk" in notes:
            fail(f"{quest_id} notes must stay in scope for the current contour")

        expected_catalog.append(build_expected_quest_catalog_entry(quest_id, quest_payload))
        expected_dispatch.append(build_expected_quest_dispatch_entry(quest_id, quest_payload))

    for quest_id in active_quest_ids:
        if quest_id not in questbook_text:
            fail(f"QUESTBOOK.md must reference active quest id '{quest_id}'")
    for quest_id in closed_quest_ids:
        if quest_id in questbook_text:
            fail(f"QUESTBOOK.md must not list closed quest id '{quest_id}'")

    catalog_payload = read_json(REPO_ROOT / QUEST_CATALOG_EXAMPLE_PATH)
    if catalog_payload != expected_catalog:
        fail("examples/quest_catalog.min.example.json must stay aligned with quests/*.yaml")

    dispatch_payload = read_json(REPO_ROOT / QUEST_DISPATCH_EXAMPLE_PATH)
    if dispatch_payload != expected_dispatch:
        fail("examples/quest_dispatch.min.example.json must stay aligned with quests/*.yaml")


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
        validate_questbook_surface()
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
    print("[ok] validated questbook boundary-runtime surfaces")
    print("[ok] validated generated KAG export outputs are up to date")
    print("[ok] validated generated KAG export structure")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

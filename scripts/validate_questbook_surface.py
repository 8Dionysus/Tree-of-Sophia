#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, Sequence

import yaml
from jsonschema import Draft202012Validator, exceptions


REPO_ROOT = Path(__file__).resolve().parents[1]

QUESTBOOK_PATH = Path("QUESTBOOK.md")
QUESTBOOK_INTEGRATION_PATH = (
    Path("mechanics") / "questbook" / "parts" / "obligation-boundary" / "docs" / "QUESTBOOK_TOS_INTEGRATION.md"
)
QUEST_SCHEMA_PATH = (
    Path("mechanics") / "questbook" / "parts" / "dispatch-contracts" / "schemas" / "quest.schema.json"
)
QUEST_DISPATCH_SCHEMA_PATH = (
    Path("mechanics") / "questbook" / "parts" / "dispatch-contracts" / "schemas" / "quest_dispatch.schema.json"
)
QUEST_CATALOG_EXAMPLE_PATH = (
    Path("mechanics") / "questbook" / "parts" / "dispatch-contracts" / "examples" / "quest_catalog.min.example.json"
)
QUEST_DISPATCH_EXAMPLE_PATH = (
    Path("mechanics") / "questbook" / "parts" / "dispatch-contracts" / "examples" / "quest_dispatch.min.example.json"
)
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
    "mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json",
    "reviewable compatibility examples",
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


class ValidationError(RuntimeError):
    pass


def fail(message: str) -> None:
    raise ValidationError(message)


def repo_relative(path: Path, repo_root: Path) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return path.as_posix()


def read_json(path: Path, *, repo_root: Path | None = None) -> object:
    root = repo_root or REPO_ROOT
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"missing required file: {repo_relative(path, root)}")
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {repo_relative(path, root)}: {exc}")


def read_text(path: Path, *, repo_root: Path | None = None) -> str:
    root = repo_root or REPO_ROOT
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        fail(f"missing required file: {repo_relative(path, root)}")


def read_yaml(path: Path, *, repo_root: Path | None = None) -> object:
    root = repo_root or REPO_ROOT
    try:
        return yaml.safe_load(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        fail(f"missing required file: {repo_relative(path, root)}")
    except yaml.YAMLError as exc:
        fail(f"invalid YAML in {repo_relative(path, root)}: {exc}")


def schema_path(error: exceptions.ValidationError) -> str:
    parts = [str(part) for part in error.path]
    return ".".join(parts) if parts else "$"


def validate_with_schema(
    payload: object,
    schema: object,
    *,
    payload_label: str,
    schema_label: str,
) -> None:
    if not isinstance(schema, dict):
        fail(f"{schema_label} must be a JSON object")
    try:
        Draft202012Validator.check_schema(schema)
    except exceptions.SchemaError as exc:
        fail(f"{schema_label} is not a valid JSON Schema: {exc.message}")

    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: list(error.path),
    )
    if errors:
        error = errors[0]
        fail(
            f"{payload_label} violates {schema_label} at {schema_path(error)}: "
            f"{error.message}"
        )


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
    activation_mode = activation.get("mode")
    if not isinstance(activation_mode, str) or not activation_mode:
        fail(f"quest {quest_id} activation.mode must be a non-empty string")

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
        "activation_mode": activation_mode,
        "source_path": f"quests/{quest_id}.yaml",
        "public_safe": payload["public_safe"],
        "fallback_tier": payload["fallback_tier"],
        "wrapper_class": payload["wrapper_class"],
    }


def validate_questbook_surface(repo_root: Path | None = None) -> None:
    root = repo_root or REPO_ROOT
    required_paths = (
        QUESTBOOK_PATH,
        QUESTBOOK_INTEGRATION_PATH,
        QUEST_SCHEMA_PATH,
        QUEST_DISPATCH_SCHEMA_PATH,
        QUEST_CATALOG_EXAMPLE_PATH,
        QUEST_DISPATCH_EXAMPLE_PATH,
    ) + tuple(Path("quests") / f"{quest_id}.yaml" for quest_id in QUEST_IDS)

    for relative_path in required_paths:
        path = root / relative_path
        if not path.exists():
            fail(f"missing required file: {relative_path.as_posix()}")

    questbook_text = read_text(root / QUESTBOOK_PATH, repo_root=root)
    for token in QUESTBOOK_REQUIRED_TOKENS:
        if token not in questbook_text:
            fail(f"QUESTBOOK.md must contain '{token}'")
    for token in QUESTBOOK_FORBIDDEN_TOKENS:
        if token in questbook_text:
            fail(f"QUESTBOOK.md must not mention '{token}'")

    integration_text = read_text(root / QUESTBOOK_INTEGRATION_PATH, repo_root=root)
    for token in QUESTBOOK_INTEGRATION_REQUIRED_TOKENS:
        if token not in integration_text:
            fail(f"{QUESTBOOK_INTEGRATION_PATH.as_posix()} must contain '{token}'")
    for token in QUESTBOOK_INTEGRATION_FORBIDDEN_TOKENS:
        if token in integration_text:
            fail(f"{QUESTBOOK_INTEGRATION_PATH.as_posix()} must not mention '{token}'")

    quest_schema_payload = read_json(root / QUEST_SCHEMA_PATH, repo_root=root)
    if not isinstance(quest_schema_payload, dict):
        fail(f"{QUEST_SCHEMA_PATH.as_posix()} must be a JSON object")
    validate_quest_schema_envelope(
        quest_schema_payload,
        title="Tree-of-Sophia work_quest_v1",
        required_fields=QUEST_SCHEMA_REQUIRED_FIELDS,
        schema_version="work_quest_v1",
        label=QUEST_SCHEMA_PATH.as_posix(),
    )

    dispatch_schema_payload = read_json(root / QUEST_DISPATCH_SCHEMA_PATH, repo_root=root)
    if not isinstance(dispatch_schema_payload, dict):
        fail(f"{QUEST_DISPATCH_SCHEMA_PATH.as_posix()} must be a JSON object")
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
        quest_path = root / "quests" / f"{quest_id}.yaml"
        quest_payload = read_yaml(quest_path, repo_root=root)
        if not isinstance(quest_payload, dict):
            fail(f"{repo_relative(quest_path, root)} must be a YAML object")
        validate_with_schema(
            quest_payload,
            quest_schema_payload,
            payload_label=repo_relative(quest_path, root),
            schema_label=QUEST_SCHEMA_PATH.as_posix(),
        )
        if quest_payload.get("schema_version") != "work_quest_v1":
            fail(f"{quest_id} schema_version must equal 'work_quest_v1'")
        if quest_payload.get("id") != quest_id:
            fail(f"{repo_relative(quest_path, root)} id must equal '{quest_id}'")
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
        dispatch_entry = build_expected_quest_dispatch_entry(quest_id, quest_payload)
        validate_with_schema(
            dispatch_entry,
            dispatch_schema_payload,
            payload_label=f"derived dispatch entry for {quest_id}",
            schema_label=QUEST_DISPATCH_SCHEMA_PATH.as_posix(),
        )
        expected_dispatch.append(dispatch_entry)

    for quest_id in active_quest_ids:
        if quest_id not in questbook_text:
            fail(f"QUESTBOOK.md must reference active quest id '{quest_id}'")
    for quest_id in closed_quest_ids:
        if quest_id in questbook_text:
            fail(f"QUESTBOOK.md must not list closed quest id '{quest_id}'")

    catalog_payload = read_json(root / QUEST_CATALOG_EXAMPLE_PATH, repo_root=root)
    if catalog_payload != expected_catalog:
        fail("mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json must stay aligned with quests/*.yaml")

    dispatch_payload = read_json(root / QUEST_DISPATCH_EXAMPLE_PATH, repo_root=root)
    if not isinstance(dispatch_payload, list):
        fail(f"{QUEST_DISPATCH_EXAMPLE_PATH.as_posix()} must be a JSON array")
    for index, dispatch_entry in enumerate(dispatch_payload):
        validate_with_schema(
            dispatch_entry,
            dispatch_schema_payload,
            payload_label=f"{QUEST_DISPATCH_EXAMPLE_PATH.as_posix()}[{index}]",
            schema_label=QUEST_DISPATCH_SCHEMA_PATH.as_posix(),
        )
    if dispatch_payload != expected_dispatch:
        fail("mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json must stay aligned with quests/*.yaml")


def main() -> int:
    try:
        validate_questbook_surface()
    except ValidationError as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1

    print("[ok] validated questbook boundary-runtime surfaces")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

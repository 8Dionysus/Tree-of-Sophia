#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any, TypeAlias

from jsonschema import Draft202012Validator, FormatChecker
from jsonschema.exceptions import SchemaError


REPO_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = Path("ToS/contracts/lived-witness-packet.schema.json")
ROUTE_ROOT = Path("ToS/zarathustra/lived-witness")

Issue: TypeAlias = tuple[str, str]

REQUIRED_FILES = (
    ROUTE_ROOT / "AGENTS.md",
    ROUTE_ROOT / "README.md",
    ROUTE_ROOT / "CAPTURE_PROTOCOL.md",
    ROUTE_ROOT / "CAPTURE_FORM.md",
    ROUTE_ROOT / "local-content/README.md",
    SCHEMA_PATH,
)

REQUIRED_TOKENS: dict[Path, tuple[str, ...]] = {
    ROUTE_ROOT / "AGENTS.md": (
        "explicit author request",
        "AI output",
        "never lived witness",
        "separate permissions",
        "cannot confirm",
    ),
    ROUTE_ROOT / "README.md": (
        "private-by-default",
        "I am ready to record a lived witness",
        "No record is created",
    ),
    ROUTE_ROOT / "CAPTURE_PROTOCOL.md": (
        "no testimony captured",
        "the time or interval of the remembered experience",
        "the recording time of this version",
        "All except local storage begin `not-granted`",
        "separate `claim-packet`",
        "Structural validation is insufficient",
    ),
    ROUTE_ROOT / "CAPTURE_FORM.md": (
        "one block at a time",
        "Keep the raw answer",
        "Without explicit confirmation",
        "Default answer for every item below is `not-granted`",
    ),
    ROUTE_ROOT / "local-content/README.md": (
        "ignored by Git",
        "mode `0600`",
        "No testimony has been created",
    ),
}

PRIVATE_PROBE = ROUTE_ROOT / "local-content/tos.lived-witness.validator-probe/packet.json"
TRACKED_BOUNDARY_PROBE = ROUTE_ROOT / "local-content/README.md"
ROUTE_DOC_PROBE = ROUTE_ROOT / "CAPTURE_FORM.md"


def _load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("JSON root must be an object")
    return payload


def synthetic_draft_packet() -> dict[str, Any]:
    """Return a non-authoritative schema fixture, never a testimony record."""

    body = "Synthetic schema-validation fixture; this is not lived testimony."
    body_sha256 = hashlib.sha256(body.encode("utf-8")).hexdigest()

    def permission(status: str, decided_at: str | None = None) -> dict[str, Any]:
        return {"status": status, "decided_at": decided_at, "scope_note": None}

    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/lived-witness-packet.schema.json",
        "schema_version": "tos_lived_witness_packet_v1",
        "packet_id": "tos.lived-witness.validator.synthetic-draft-v1",
        "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
        "author": {
            "agent_ref": "human:synthetic-validator-fixture",
            "agent_kind": "human",
            "first_person_human_authorship_asserted": True,
            "public_name": None,
        },
        "record_status": "draft",
        "testimony": {
            "body": body,
            "body_sha256": body_sha256,
            "language": "en",
            "media_type": "text/plain",
            "authorship_posture": "human-authored-text",
            "human_wording_asserted": True,
        },
        "targets": [
            {
                "target_kind": "whole-work",
                "target_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
                "anchor_refs": [],
                "relationship": "lived-with",
                "note": None,
            }
        ],
        "experience_context": {
            "temporal_scopes": [
                {"precision": "withheld", "started": None, "ended": None, "author_note": None}
            ],
            "author_context_terms": [],
            "context_note": None,
            "recollection_posture": "first-person-recollection-not-objective-event-proof",
        },
        "capture": {
            "mode": "self-authored-text",
            "recorded_at": "2026-08-08T00:00:00Z",
            "recorder_ref": "human:synthetic-validator-fixture",
            "raw_artifacts": [],
            "ai_involvement": {
                "involved": False,
                "roles": [],
                "model_refs": [],
                "prompt_or_instruction_refs": [],
            },
            "transformations": [],
            "silent_transformation_allowed": False,
        },
        "third_party_posture": {
            "mentions_or_implicates_others": "no",
            "risk_review_status": "not-needed",
            "redaction_status": "not-needed",
            "public_derivative_blocked_until_review": True,
        },
        "rights": {
            "copyright_holder_ref": "human:synthetic-validator-fixture",
            "copyright_posture": "author-retained",
            "license_ref": None,
            "permission_model": "purpose-specific-no-inheritance",
        },
        "use_permissions": {
            "store_local": permission("granted", "2026-08-08T00:00:00Z"),
            "track_metadata_in_git": permission("not-granted"),
            "quote": permission("not-granted"),
            "lexical_index": permission("not-granted"),
            "semantic_index": permission("not-granted"),
            "graph_projection": permission("not-granted"),
            "public_release": permission("not-granted"),
            "model_training": permission("not-granted"),
            "external_service_processing": permission("not-granted"),
        },
        "author_review": {
            "author_confirmed": False,
            "confirmed_at": None,
            "confirmation_method": "not-yet-confirmed",
            "reviewed_body_sha256": None,
            "review_note": None,
        },
        "revision": {
            "packet_version": 1,
            "supersedes_packet_ref": None,
            "superseded_by_packet_ref": None,
            "change_summary": None,
            "withdrawal_note": None,
        },
        "provenance_event_refs": ["tos.event.lived-witness.validator.synthetic-capture-v1"],
        "authority_boundary": {
            "is_first_person_authored_record": True,
            "is_primary_source_witness": False,
            "establishes_textual_fact": False,
            "establishes_bibliographic_fact": False,
            "establishes_philological_fact": False,
            "establishes_translation_truth": False,
            "establishes_semantic_truth": False,
            "promotes_canon": False,
            "ai_output_alone_can_be_lived_witness": False,
            "separate_claim_and_review_required_for_promotion": True,
        },
    }


def validate_packet_mechanics(
    packet: dict[str, Any], schema: dict[str, Any]
) -> list[str]:
    """Validate public packet mechanics without claiming human truth."""

    issues: list[str] = []
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    for error in sorted(
        validator.iter_errors(packet), key=lambda item: list(item.absolute_path)
    ):
        location = ".".join(str(part) for part in error.absolute_path) or "root"
        issues.append(f"{location}: {error.message}")

    testimony = packet.get("testimony")
    if isinstance(testimony, dict):
        body = testimony.get("body")
        declared_digest = testimony.get("body_sha256")
        if isinstance(body, str) and isinstance(declared_digest, str):
            actual_digest = hashlib.sha256(body.encode("utf-8")).hexdigest()
            if declared_digest != actual_digest:
                issues.append("testimony.body_sha256 does not match the exact UTF-8 body")

    if packet.get("record_status") == "author-confirmed":
        author_review = packet.get("author_review")
        if isinstance(testimony, dict) and isinstance(author_review, dict):
            body_digest = testimony.get("body_sha256")
            reviewed_digest = author_review.get("reviewed_body_sha256")
            if isinstance(body_digest, str) and isinstance(reviewed_digest, str):
                if reviewed_digest != body_digest:
                    issues.append(
                        "author_review.reviewed_body_sha256 does not bind the exact testimony body"
                    )

    return issues


def _git_ignored(repo_root: Path, relative_path: Path) -> bool:
    completed = subprocess.run(
        ("git", "check-ignore", "--no-index", "--quiet", relative_path.as_posix()),
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode not in (0, 1):
        raise RuntimeError(completed.stderr.strip() or "git check-ignore failed")
    return completed.returncode == 0


def _tracked_local_content(repo_root: Path) -> set[str]:
    completed = subprocess.run(
        (
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            (ROUTE_ROOT / "local-content").as_posix(),
        ),
        cwd=repo_root,
        check=True,
        capture_output=True,
        text=True,
    )
    return {line for line in completed.stdout.splitlines() if line}


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    for relative_path in REQUIRED_FILES:
        if not (root / relative_path).is_file():
            issues.append((relative_path.as_posix(), "required lived-witness route file is missing"))

    if issues:
        return issues

    for relative_path, tokens in REQUIRED_TOKENS.items():
        text = (root / relative_path).read_text(encoding="utf-8")
        for token in tokens:
            if token not in text:
                issues.append((relative_path.as_posix(), f"missing boundary token: {token}"))

    try:
        schema = _load_json(root / SCHEMA_PATH)
        Draft202012Validator.check_schema(schema)
    except (OSError, ValueError, json.JSONDecodeError, SchemaError) as exc:
        issues.append((SCHEMA_PATH.as_posix(), f"schema load/check failed: {exc}"))
        return issues

    fixture = synthetic_draft_packet()
    for message in validate_packet_mechanics(fixture, schema):
        issues.append((SCHEMA_PATH.as_posix(), f"synthetic fixture {message}"))

    try:
        if not _git_ignored(root, PRIVATE_PROBE):
            issues.append((".gitignore", "private lived-witness packet path is not ignored"))
        if _git_ignored(root, TRACKED_BOUNDARY_PROBE):
            issues.append((".gitignore", "local-content boundary README is unexpectedly ignored"))
        if _git_ignored(root, ROUTE_DOC_PROBE):
            issues.append((".gitignore", "lived-witness route form is unexpectedly ignored"))
        tracked = _tracked_local_content(root)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as exc:
        issues.append((".gitignore", f"ignore/tracked-boundary check failed: {exc}"))
    else:
        expected = {(ROUTE_ROOT / "local-content/README.md").as_posix()}
        if tracked != expected:
            issues.append(
                (
                    (ROUTE_ROOT / "local-content").as_posix(),
                    f"tracked private-content surface must contain only README.md; found {sorted(tracked)}",
                )
            )

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Lived-witness route check failed.")
        for location, message in issues:
            print(f"- {location}: {message}")
        return 1

    print("Lived-witness route check passed for structure and private boundary only.")
    print("Human authorship, consent, memory, context, and meaning remain unvalidated.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

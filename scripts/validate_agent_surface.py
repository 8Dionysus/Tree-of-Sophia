#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any, Mapping

from build_agent_surface_currentness import (
    COMPANION_FAMILIES,
    CURRENTNESS_PATH,
    MANIFEST_PATH,
    REPO_ROOT,
    SKILLS_ROOT,
    build_currentness,
    parse_skill,
    rendered_currentness,
)


Issue = tuple[str, str]
EXPECTED_SKILLS = {
    "aoa-adr-write",
    "aoa-approval-gate-check",
    "aoa-automation-opportunity-scan",
    "aoa-bounded-context-map",
    "aoa-change-protocol",
    "aoa-checkpoint-closeout-bridge",
    "aoa-commit-growth-seam",
    "aoa-contract-test",
    "aoa-core-logic-boundary",
    "aoa-dry-run-first",
    "aoa-invariant-coverage-audit",
    "aoa-local-stack-bringup",
    "aoa-port-adapter-refactor",
    "aoa-property-invariants",
    "aoa-quest-harvest",
    "aoa-safe-infra-change",
    "aoa-sanitized-share",
    "aoa-session-donor-harvest",
    "aoa-session-progression-lift",
    "aoa-session-route-forks",
    "aoa-session-self-diagnose",
    "aoa-session-self-repair",
    "aoa-source-of-truth-check",
    "aoa-summon",
    "aoa-tdd-slice",
}
EXPECTED_PORTS = {"eval_port", "stats_port", "kag_provider", "memo_port"}
EXPECTED_PROBES = {
    "source_authority",
    "bounded_context_mapping",
    "repo_local_change",
    "session_diagnosis_repair",
    "approval_or_dry_run",
    "eval_intake",
    "owner_local_stats",
    "kag_currentness",
    "memo_candidate_routing",
}
PATH_REFERENCE_RE = re.compile(r"\b(?:references|examples|checks|scripts|assets)/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*\.[A-Za-z0-9_-]+\b")
PUBLIC_FORBIDDEN = (
    "/home/",
    "/srv/",
    "/tmp/",
    "BEGIN PRIVATE KEY",
    "OPENAI_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "file://",
)
KAG_BUDGET_RECEIPT_SCHEMA_VERSION = "aoa-repo-local-kag-budget-receipt-v1"
KAG_BUDGET_RECEIPT_SCHEMA_V2_VERSION = "aoa-repo-local-kag-budget-receipt-v2"
KAG_BUDGET_RECEIPT_SCHEMA_VERSIONS = frozenset(
    {
        KAG_BUDGET_RECEIPT_SCHEMA_VERSION,
        KAG_BUDGET_RECEIPT_SCHEMA_V2_VERSION,
    }
)
KAG_BUDGET_RECEIPT_SCOPES = {
    "generated_delta",
    "tracked_size",
    "generated_delta_and_tracked_size",
    "v2_to_v3_migration",
}
KAG_BUDGET_EXCEEDANCE_RELATIONS: dict[frozenset[str], str | None] = {
    frozenset(): None,
    frozenset({"generated_delta"}): "generated_delta",
    frozenset({"tracked_size"}): "tracked_size",
    frozenset({"generated_delta", "tracked_size"}): "generated_delta_and_tracked_size",
}
KAG_BUDGET_DECISION_REF = (
    "aoa-kag:docs/decisions/"
    "AOA-KAG-D-0017-portable-content-addressed-repository-family.md"
)
KAG_TIERED_DECISION_REF = (
    "aoa-kag:docs/decisions/"
    "AOA-KAG-D-0039-tiered-content-addressed-kag-distribution.md"
)
KAG_BUDGET_RECEIPT_REQUIRED_FIELDS = {
    "schema_version",
    "repo",
    "scope",
    "base_ref",
    "head_family_digest",
    "changed_generated_bytes",
    "changed_generated_files",
    "default_limit_bytes",
    "allowed_bytes",
    "tracked_bytes",
    "tracked_bytes_max",
    "allowed_tracked_bytes",
    "reason",
    "approved_by",
    "decision_ref",
}
KAG_BUDGET_RECEIPT_V2_REQUIRED_FIELDS = KAG_BUDGET_RECEIPT_REQUIRED_FIELDS | {
    "head_source_snapshot",
    "candidate_identity",
    "producer_identity",
}
KAG_BUDGET_CANDIDATE_IDENTITY_FIELDS = {
    "contract_version",
    "algorithm",
    "seal",
    "file_count",
    "excluded_path",
    "base_ref",
    "family_digest",
    "source_snapshot",
    "source_epoch",
}
KAG_BUDGET_PRODUCER_IDENTITY_FIELDS = {
    "contract_version",
    "owner",
    "revision_binding",
    "source_digest",
    "procedure_manifest",
    "files",
    "action",
    "execution_inputs",
    "identity_digest",
}
KAG_BUDGET_PROCEDURE_MANIFEST_FIELDS = {
    "manifest_path",
    "manifest_digest",
    "schema_path",
    "closure_mode",
    "dynamic_import_policy",
    "python_entrypoints",
    "python_import_closure",
    "schema_inputs",
    "action_path",
    "action_inputs",
    "environment",
    "dependencies",
}
KAG_BUDGET_PRODUCER_PROFILES = {
    "aoa-kag:budget-receipt-producer-identity-v3": {
        "revision_binding": "content-addressed-procedure-import-closure-runtime-inputs-and-descriptor-io-v1",
        "runtime_inputs": "aoa-kag:budget-receipt-producer-runtime-inputs-v1",
    },
    "aoa-kag:budget-receipt-producer-identity-v4": {
        "revision_binding": "content-addressed-procedure-import-closure-portable-runtime-contract-and-descriptor-io-v1",
        "runtime_inputs": "aoa-kag:budget-receipt-producer-runtime-inputs-v2",
    },
}
KAG_BUDGET_CANDIDATE_ZERO_DIGEST = "0" * 64
KAG_BUDGET_SOURCE_EPOCH_VERSION = "aoa-kag:budget-receipt-source-epoch-v1"
KAG_BUDGET_OWNER_ROOT_VALUE = "<owner-root>"
KAG_BUDGET_UNSET_ENVIRONMENT_VALUE = "<unset>"
KAG_BUDGET_APPROVED_PYTHON_INTERPRETER_PATH = "<approved-python-interpreter>"
KAG_BUDGET_RUNTIME_CONTRACT_PREFIX = "aoa-kag:budget-producer-runtime-contract:"
KAG_BUDGET_PROCEDURE_MANIFEST_PATH = "config/repo-local-kag-budget-producer.json"
KAG_BUDGET_PROCEDURE_SCHEMA_PATH = "schemas/repo-local-kag-budget-producer-manifest.schema.json"
KAG_BUDGET_PROCEDURE_ACTION_PATH = ".github/actions/repo-local-kag-index/action.yml"


def activation_policy_issues(
    invocation_mode: str | None,
    implicit_policy: str | None,
    allow_implicit: bool | None,
    skill_id: str | None = None,
) -> list[str]:
    expected: tuple[str, bool] | None = {
        "explicit-only": ("manual", False),
        "explicit-preferred": ("invoke", True),
    }.get(invocation_mode or "")
    if skill_id == "aoa-checkpoint-closeout-bridge" and invocation_mode == "explicit-preferred":
        expected = ("suggest", False)
    if expected is None:
        return [f"unknown aoa_invocation_mode {invocation_mode!r}"]
    expected_policy, expected_allow = expected
    issues = []
    if implicit_policy != expected_policy:
        issues.append(f"implicit_activation_policy={implicit_policy!r}, expected {expected_policy!r}")
    if allow_implicit != expected_allow:
        issues.append(f"allow_implicit_invocation={allow_implicit!r}, expected {expected_allow!r}")
    return issues


def public_safety_issues(root: Path = REPO_ROOT) -> list[Issue]:
    issues: list[Issue] = []
    for relative in (".agents/README.md", MANIFEST_PATH.as_posix()):
        path = root / relative
        if not path.is_file():
            issues.append((relative, "public authored surface is missing"))
            continue
        text = path.read_text(encoding="utf-8")
        for forbidden in PUBLIC_FORBIDDEN:
            if forbidden in text:
                issues.append((relative, f"contains forbidden public-safety marker {forbidden!r}"))
    return issues


def _relative_label(root: Path, path: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return str(path)


def _is_integer(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def budget_exceedance_relation(
    *,
    changed_generated_bytes: Any,
    default_limit_bytes: Any,
    tracked_bytes: Any,
    tracked_bytes_max: Any,
) -> str | None:
    """Map measured budget booleans to the canonical aoa-kag scope vocabulary."""
    values = {
        "changed_generated_bytes": changed_generated_bytes,
        "default_limit_bytes": default_limit_bytes,
        "tracked_bytes": tracked_bytes,
        "tracked_bytes_max": tracked_bytes_max,
    }
    malformed = [field for field, value in values.items() if not _is_integer(value)]
    if malformed:
        raise ValueError(f"budget relation fields must be integers: {', '.join(sorted(malformed))}")
    negative = [field for field, value in values.items() if value < 0]
    if negative:
        raise ValueError(f"budget relation fields must not be negative: {', '.join(sorted(negative))}")
    exceeded = frozenset(
        dimension
        for dimension, is_exceeded in (
            ("generated_delta", changed_generated_bytes > default_limit_bytes),
            ("tracked_size", tracked_bytes > tracked_bytes_max),
        )
        if is_exceeded
    )
    return KAG_BUDGET_EXCEEDANCE_RELATIONS[exceeded]


def _base_has_v3_manifest(root: Path, base_ref: str) -> bool | None:
    """Return the owner-source migration discriminator for an exact base ref."""
    try:
        resolved = subprocess.run(
            ("git", "rev-parse", "--verify", f"{base_ref}^{{commit}}"),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if resolved.returncode != 0:
            return None
        manifest = subprocess.run(
            (
                "git",
                "cat-file",
                "-e",
                f"{resolved.stdout.strip()}:kag/indexes/index_family.manifest.json",
            ),
            cwd=root,
            check=False,
            capture_output=True,
        )
    except OSError:
        return None
    return manifest.returncode == 0


def _fetch_exact_base_ref(root: Path, base_ref: str) -> bool:
    """Fetch one receipt-bound commit when an explicit validation preflight allows it."""
    if not re.fullmatch(r"[0-9a-f]{40,64}", base_ref):
        return False
    try:
        fetched = subprocess.run(
            (
                "git",
                "fetch",
                "--no-tags",
                "--no-recurse-submodules",
                "origin",
                base_ref,
            ),
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return fetched.returncode == 0


def _verified_base_has_v3_manifest(
    root: Path,
    base_ref: str,
    *,
    fetch_missing_base: bool,
) -> bool | None:
    """Resolve the base locally, optionally through an explicit exact-fetch preflight."""
    result = _base_has_v3_manifest(root, base_ref)
    if result is None and fetch_missing_base and _fetch_exact_base_ref(root, base_ref):
        result = _base_has_v3_manifest(root, base_ref)
    return result


def canonical_budget_scope(
    relation: str | None,
    *,
    base_has_v3: bool,
) -> str | None:
    """Mirror aoa-kag's narrow pre-v3 migration branch before normal mapping."""
    if relation is None:
        return None
    if not base_has_v3:
        return "v2_to_v3_migration"
    return relation


def _v2_digest_issue(label: str, field: str, value: object, *, prefixed: bool = False) -> Issue | None:
    pattern = r"sha256:[0-9a-f]{64}" if prefixed else r"[0-9a-f]{64}"
    if not isinstance(value, str) or not re.fullmatch(pattern, value):
        expected = "sha256:<64 lowercase hex>" if prefixed else "64 lowercase hex"
        return (label, f"budget receipt field {field} must be {expected}")
    return None


def _v2_action_ref_issues(
    label: str,
    value: object,
    field: str,
    base_ref: str,
) -> list[Issue]:
    """Bind a producer action's hashed Git ref input to the receipt base."""
    expected_fields = {"state", "kind", "value_digest", "bytes"}
    issues = _v2_object_shape_issues(label, value, expected_fields, field)
    if not isinstance(value, dict):
        return issues
    short_field = field.rsplit(".", 1)[-1]
    if value.get("state") != "set":
        issues.append((label, f"budget receipt producer action input {short_field} state must be 'set'"))
    if value.get("kind") != "git-ref":
        issues.append((label, f"budget receipt producer action input {short_field} kind must be 'git-ref'"))
    digest_issue = _v2_digest_issue(label, f"{field}.value_digest", value.get("value_digest"))
    if digest_issue:
        issues.append(digest_issue)
    expected_digest = hashlib.sha256(base_ref.encode("utf-8")).hexdigest()
    if value.get("value_digest") != expected_digest:
        issues.append(
            (
                label,
                f"budget receipt producer action input {short_field} value_digest does not match receipt base_ref",
            )
        )
    expected_bytes = len(base_ref.encode("utf-8"))
    if value.get("bytes") != expected_bytes:
        issues.append(
            (
                label,
                f"budget receipt producer action input {short_field} bytes does not match receipt base_ref",
            )
        )
    return issues


def _v2_relative_path_input_issues(
    label: str,
    value: object,
    field: str,
    expected_path: str,
) -> list[Issue]:
    """Bind a producer's relative-path input to the generated family output."""
    expected_fields = {"state", "kind", "value_digest", "bytes"}
    issues = _v2_object_shape_issues(label, value, expected_fields, field)
    if not isinstance(value, dict):
        return issues
    short_field = field.rsplit(".", 1)[-1]
    if value.get("state") != "set":
        issues.append((label, f"budget receipt producer action input {short_field} state must be 'set'"))
    if value.get("kind") != "relative-path":
        issues.append(
            (label, f"budget receipt producer action input {short_field} kind must be 'relative-path'"),
        )
    digest_issue = _v2_digest_issue(label, f"{field}.value_digest", value.get("value_digest"))
    if digest_issue:
        issues.append(digest_issue)
    expected_digest = hashlib.sha256(expected_path.encode("utf-8")).hexdigest()
    if value.get("value_digest") != expected_digest:
        issues.append(
            (
                label,
                f"budget receipt producer action input {short_field} value_digest does not match canonical family output",
            )
        )
    expected_bytes = len(expected_path.encode("utf-8"))
    if value.get("bytes") != expected_bytes:
        issues.append(
            (
                label,
                f"budget receipt producer action input {short_field} bytes does not match canonical family output",
            )
        )
    return issues


def _v2_owner_root_input_issues(label: str, value: object, field: str) -> list[Issue]:
    """Bind the producer's stable owner-root input to its canonical value."""
    expected_fields = {"state", "kind", "value_digest", "bytes"}
    issues = _v2_object_shape_issues(label, value, expected_fields, field)
    if not isinstance(value, dict):
        return issues
    if value.get("state") != "set":
        issues.append((label, "budget receipt producer action input repo-root state must be 'set'"))
    if value.get("kind") != "path":
        issues.append((label, "budget receipt producer action input repo-root kind must be 'path'"))
    digest_issue = _v2_digest_issue(label, f"{field}.value_digest", value.get("value_digest"))
    if digest_issue:
        issues.append(digest_issue)
    expected_digest = hashlib.sha256(KAG_BUDGET_OWNER_ROOT_VALUE.encode("utf-8")).hexdigest()
    if value.get("value_digest") != expected_digest:
        issues.append((label, "budget receipt producer action input repo-root value_digest does not match canonical owner root"))
    expected_bytes = len(KAG_BUDGET_OWNER_ROOT_VALUE.encode("utf-8"))
    if value.get("bytes") != expected_bytes:
        issues.append((label, "budget receipt producer action input repo-root bytes does not match canonical owner root"))
    return issues


def _v2_repo_root_target_issues(label: str, value: object, field: str) -> list[Issue]:
    """Validate the command target's stable owner-root path digests."""
    expected_fields = {"path_digest", "resolved_path_digest"}
    issues = _v2_object_shape_issues(label, value, expected_fields, field)
    if not isinstance(value, dict):
        return issues
    expected_digest = hashlib.sha256(KAG_BUDGET_OWNER_ROOT_VALUE.encode("utf-8")).hexdigest()
    for name in expected_fields:
        digest_issue = _v2_digest_issue(label, f"{field}.{name}", value.get(name))
        if digest_issue:
            issues.append(digest_issue)
        if value.get(name) != expected_digest:
            issues.append((label, f"budget receipt producer command target repo_root {name} does not match canonical owner root"))
    return issues


def _v2_runtime_environment_issues(
    label: str,
    value: object,
    procedure_manifest: object,
) -> list[Issue]:
    """Bind captured environment records to the declared producer inputs."""
    if not isinstance(value, list) or not value:
        return [
            (
                label,
                "budget receipt producer runtime environment must be a non-empty array",
            )
        ]
    declarations = (
        procedure_manifest.get("environment")
        if isinstance(procedure_manifest, Mapping)
        else None
    )
    expected: dict[str, str] = {}
    if isinstance(declarations, list):
        for item in declarations:
            if isinstance(item, Mapping):
                name = item.get("name")
                role = item.get("role")
                if isinstance(name, str) and isinstance(role, str):
                    expected[name] = role

    issues: list[Issue] = []
    actual_names: list[str] = []
    expected_fields = {"name", "role", "state", "kind", "value_digest", "bytes"}
    for index, item in enumerate(value):
        field = f"producer_identity.execution_inputs.environment[{index}]"
        issues.extend(_v2_object_shape_issues(label, item, expected_fields, field))
        if not isinstance(item, Mapping):
            continue
        name = item.get("name")
        role = item.get("role")
        if not isinstance(name, str) or not name:
            issues.append((label, f"budget receipt field {field}.name must be a non-empty string"))
        else:
            actual_names.append(name)
            if name not in expected:
                issues.append((label, f"budget receipt runtime environment {name} is not declared by the producer procedure"))
            elif role != expected[name]:
                issues.append((label, f"budget receipt runtime environment {name} role does not match the producer procedure"))
        if not isinstance(role, str) or not role:
            issues.append((label, f"budget receipt field {field}.role must be a non-empty string"))
        state = item.get("state")
        if not isinstance(state, str) or state not in {"set", "unset"}:
            issues.append((label, f"budget receipt field {field}.state must be 'set' or 'unset'"))
        if item.get("kind") != "environment":
            issues.append((label, f"budget receipt field {field}.kind must be 'environment'"))
        digest_issue = _v2_digest_issue(label, f"{field}.value_digest", item.get("value_digest"))
        if digest_issue:
            issues.append(digest_issue)
        if state == "unset":
            expected_digest = hashlib.sha256(
                KAG_BUDGET_UNSET_ENVIRONMENT_VALUE.encode("utf-8")
            ).hexdigest()
            if item.get("value_digest") != expected_digest:
                issues.append(
                    (
                        label,
                        f"budget receipt unset runtime environment {name} value_digest does not match the unset sentinel",
                    )
                )
            if item.get("bytes") != 0:
                issues.append(
                    (
                        label,
                        f"budget receipt unset runtime environment {name} bytes must be zero",
                    )
                )
        elif state == "set":
            actual_value = os.environ.get(name) if isinstance(name, str) else None
            if actual_value is None:
                issues.append(
                    (
                        label,
                        f"budget receipt set runtime environment {name} cannot be verified against the current action environment",
                    )
                )
            else:
                actual_bytes = actual_value.encode("utf-8", errors="surrogateescape")
                actual_digest = hashlib.sha256(actual_bytes).hexdigest()
                if item.get("value_digest") != actual_digest:
                    issues.append(
                        (
                            label,
                            f"budget receipt set runtime environment {name} value_digest does not match the current action environment",
                        )
                    )
                if item.get("bytes") != len(actual_bytes):
                    issues.append(
                        (
                            label,
                            f"budget receipt set runtime environment {name} bytes do not match the current action environment",
                        )
                    )
        if not _is_integer(item.get("bytes")) or item["bytes"] < 0:
            issues.append((label, f"budget receipt field {field}.bytes must be a non-negative integer"))
    if (
        sorted(actual_names) != sorted(expected)
        or len(actual_names) != len(set(actual_names))
    ):
        issues.append((label, "budget receipt runtime environment does not match the producer procedure"))
    return issues


def _v2_runtime_dependency_issues(
    label: str,
    value: object,
    procedure_manifest: object,
) -> list[Issue]:
    """Bind captured dependency records to the declared producer inputs."""
    if not isinstance(value, list) or not value:
        return [
            (
                label,
                "budget receipt producer runtime dependencies must be a non-empty array",
            )
        ]
    declarations = (
        procedure_manifest.get("dependencies")
        if isinstance(procedure_manifest, Mapping)
        else None
    )
    expected: dict[str, tuple[object, object]] = {}
    if isinstance(declarations, list):
        for item in declarations:
            if isinstance(item, Mapping):
                name = item.get("name")
                version = item.get("version")
                if isinstance(name, str) and isinstance(version, str):
                    expected[name] = (version, item.get("required", True))

    issues: list[Issue] = []
    actual_names: list[str] = []
    expected_fields = {
        "name",
        "declared_version",
        "required",
        "state",
        "resolved_version",
        "path_digest",
        "artifact_digest",
        "artifact_bytes",
        "artifact_files",
    }
    for index, item in enumerate(value):
        field = f"producer_identity.execution_inputs.dependencies[{index}]"
        issues.extend(_v2_object_shape_issues(label, item, expected_fields, field))
        if not isinstance(item, Mapping):
            continue
        name = item.get("name")
        if not isinstance(name, str) or not name:
            issues.append((label, f"budget receipt field {field}.name must be a non-empty string"))
        else:
            actual_names.append(name)
            declaration = expected.get(name)
            if declaration is None:
                issues.append((label, f"budget receipt runtime dependency {name} is not declared by the producer procedure"))
            else:
                declared_version, required = declaration
                if item.get("declared_version") != declared_version:
                    issues.append((label, f"budget receipt runtime dependency {name} version does not match the producer procedure"))
                if item.get("required") != required:
                    issues.append((label, f"budget receipt runtime dependency {name} required flag does not match the producer procedure"))
        if not isinstance(item.get("declared_version"), str) or not item["declared_version"]:
            issues.append((label, f"budget receipt field {field}.declared_version must be a non-empty string"))
        required = item.get("required")
        if not isinstance(required, bool):
            issues.append((label, f"budget receipt field {field}.required must be a boolean"))
        state = item.get("state")
        if not isinstance(state, str) or state not in {"available", "unavailable", "declared"}:
            issues.append((label, f"budget receipt field {field}.state is unsupported"))
        if required is True and state != "available":
            issues.append((label, f"budget receipt required runtime dependency {name} must be available"))
        resolved_version = item.get("resolved_version")
        if resolved_version is not None and not isinstance(resolved_version, str):
            issues.append((label, f"budget receipt field {field}.resolved_version must be a string or null"))
        if state == "available" and (not isinstance(resolved_version, str) or not resolved_version):
            issues.append((label, f"budget receipt available runtime dependency {name} must keep resolved_version"))
        for name_field in ("path_digest", "artifact_digest"):
            digest = item.get(name_field)
            if digest is not None:
                digest_issue = _v2_digest_issue(label, f"{field}.{name_field}", digest)
                if digest_issue:
                    issues.append(digest_issue)
            elif state == "available":
                issues.append((label, f"budget receipt available runtime dependency {name} must keep {name_field}"))
        for name_field in ("artifact_bytes", "artifact_files"):
            if not _is_integer(item.get(name_field)) or item[name_field] < 0:
                issues.append((label, f"budget receipt field {field}.{name_field} must be a non-negative integer"))
    if (
        sorted(actual_names) != sorted(expected)
        or len(actual_names) != len(set(actual_names))
    ):
        issues.append((label, "budget receipt runtime dependencies do not match the producer procedure"))
    return issues


def _v2_runtime_interpreter_issues(
    label: str,
    value: object,
    procedure_manifest: object,
) -> list[Issue]:
    """Validate the interpreter record and bind its version to Python input."""
    expected_fields = {
        "implementation",
        "version",
        "invoked_path_digest",
        "resolved_path_digest",
        "artifact_digest",
    }
    issues = _v2_object_shape_issues(
        label,
        value,
        expected_fields,
        "producer_identity.execution_inputs.interpreter",
    )
    if not isinstance(value, Mapping):
        return issues
    field = "producer_identity.execution_inputs.interpreter"
    implementation = value.get("implementation")
    version = value.get("version")
    for name, current in (("implementation", implementation), ("version", version)):
        if not isinstance(current, str) or not current:
            issues.append((label, f"budget receipt field {field}.{name} must be a non-empty string"))
    if isinstance(implementation, str) and implementation != sys.implementation.name:
        issues.append((label, "budget receipt interpreter implementation does not match the current Python runtime"))
    for name in ("invoked_path_digest", "resolved_path_digest", "artifact_digest"):
        digest_issue = _v2_digest_issue(label, f"{field}.{name}", value.get(name))
        if digest_issue:
            issues.append(digest_issue)
    declarations = (
        procedure_manifest.get("dependencies")
        if isinstance(procedure_manifest, Mapping)
        else None
    )
    python_version = None
    if isinstance(declarations, list):
        for item in declarations:
            if isinstance(item, Mapping) and item.get("name") == "python":
                python_version = item.get("version")
                break
    if isinstance(python_version, str) and value.get("version") != python_version:
        issues.append((label, "budget receipt interpreter version does not match the declared python dependency"))
    if isinstance(implementation, str) and isinstance(version, str):
        expected_path_digest = hashlib.sha256(
            KAG_BUDGET_APPROVED_PYTHON_INTERPRETER_PATH.encode("utf-8")
        ).hexdigest()
        for name in ("invoked_path_digest", "resolved_path_digest"):
            if value.get(name) != expected_path_digest:
                issues.append(
                    (
                        label,
                        f"budget receipt interpreter {name} does not match the approved Python interpreter",
                    )
                )
        expected_artifact_digest = hashlib.sha256(
            (
                KAG_BUDGET_RUNTIME_CONTRACT_PREFIX
                + "python:"
                + sys.implementation.name
                + ":"
                + version
            ).encode("utf-8")
        ).hexdigest()
        if value.get("artifact_digest") != expected_artifact_digest:
            issues.append(
                (
                    label,
                    "budget receipt interpreter artifact_digest does not match the captured Python runtime contract",
                )
            )
    return issues


def _v2_jobs_input_issues(
    label: str,
    value: object,
    field: str,
    expected_jobs: object,
) -> list[Issue]:
    """Bind the hashed jobs input to the bounded command target."""
    expected_fields = {"state", "kind", "value_digest", "bytes"}
    issues = _v2_object_shape_issues(label, value, expected_fields, field)
    if not isinstance(value, Mapping):
        return issues
    if value.get("state") != "set":
        issues.append((label, f"budget receipt producer action input jobs state must be 'set'"))
    if value.get("kind") != "bounded-integer":
        issues.append((label, "budget receipt producer action input jobs kind must be 'bounded-integer'"))
    digest_issue = _v2_digest_issue(label, f"{field}.value_digest", value.get("value_digest"))
    if digest_issue:
        issues.append(digest_issue)
    if not isinstance(expected_jobs, str) or not re.fullmatch(r"[1-3]", expected_jobs):
        issues.append((label, "budget receipt producer command target jobs must be a bounded integer in [1, 3]"))
    else:
        expected_digest = hashlib.sha256(expected_jobs.encode("utf-8")).hexdigest()
        if value.get("value_digest") != expected_digest:
            issues.append((label, "budget receipt producer action input jobs value_digest does not match command target jobs"))
        if value.get("bytes") != len(expected_jobs.encode("utf-8")):
            issues.append((label, "budget receipt producer action input jobs bytes do not match command target jobs"))
    if not _is_integer(value.get("bytes")) or value["bytes"] < 0:
        issues.append((label, f"budget receipt field {field}.bytes must be a non-negative integer"))
    return issues


def _v2_canonical_digest(value: object) -> str:
    """Match aoa-kag's canonical JSON digest for identity material."""
    return hashlib.sha256(
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    ).hexdigest()


def _v2_object_shape_issues(
    label: str,
    value: object,
    expected_fields: set[str],
    field: str,
) -> list[Issue]:
    if not isinstance(value, dict):
        return [(label, f"budget receipt field {field} must be an object")]
    issues: list[Issue] = []
    missing = sorted(expected_fields - set(value))
    extra = sorted(set(value) - expected_fields)
    for name in missing:
        issues.append((label, f"budget receipt field {field} missing {name}"))
    for name in extra:
        issues.append((label, f"budget receipt field {field} has unexpected {name}"))
    return issues


def _v2_producer_file_issues(label: str, value: object, field: str) -> list[Issue]:
    expected_fields = {"path", "state", "content_digest", "bytes", "git_blob"}
    issues = _v2_object_shape_issues(label, value, expected_fields, field)
    if not isinstance(value, dict):
        return issues
    if value.get("state") != "present":
        issues.append((label, f"budget receipt field {field}.state must be 'present'"))
    path = value.get("path")
    if not isinstance(path, str) or not path or Path(path).is_absolute() or ".." in Path(path).parts:
        issues.append((label, f"budget receipt field {field}.path must be a relative path"))
    elif any(ord(character) < 32 or ord(character) == 127 for character in path):
        issues.append((label, f"budget receipt field {field}.path must not contain control characters"))
    digest_issue = _v2_digest_issue(label, f"{field}.content_digest", value.get("content_digest"))
    if digest_issue:
        issues.append(digest_issue)
    git_blob = value.get("git_blob")
    if not isinstance(git_blob, str) or not re.fullmatch(r"sha1:[0-9a-f]{40}", git_blob):
        issues.append((label, f"budget receipt field {field}.git_blob must be sha1:<40 lowercase hex>"))
    if not _is_integer(value.get("bytes")) or value["bytes"] < 0:
        issues.append((label, f"budget receipt field {field}.bytes must be a non-negative integer"))
    return issues


def _v2_candidate_inventory(root: Path, excluded_path: Path) -> list[dict[str, Any]]:
    """Rebuild the v2 candidate inventory used by aoa-kag's portable receipt."""
    result = subprocess.run(
        ("git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"),
        cwd=root,
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        raise ValueError("candidate identity requires a readable Git worktree")

    relative_paths: set[Path] = set()
    for raw in result.stdout.split(b"\0"):
        if not raw:
            continue
        try:
            relative = Path(raw.decode("utf-8", errors="strict"))
        except UnicodeDecodeError as exc:
            raise ValueError("candidate identity encountered a non-UTF-8 Git path") from exc
        if relative.is_absolute() or not relative.parts or ".." in relative.parts:
            raise ValueError("candidate identity encountered an unsafe path")
        if relative != excluded_path:
            relative_paths.add(relative)

    inventory: list[dict[str, Any]] = []
    for relative in sorted(relative_paths, key=lambda path: path.as_posix()):
        path = root / relative
        try:
            metadata = path.lstat()
        except FileNotFoundError:
            inventory.append(
                {
                    "path": relative.as_posix(),
                    "state": "missing",
                    "kind": "missing",
                    "mode": "missing",
                    "bytes": 0,
                    "content_digest": KAG_BUDGET_CANDIDATE_ZERO_DIGEST,
                }
            )
            continue
        if stat.S_ISREG(metadata.st_mode):
            content = path.read_bytes()
            kind = "file"
            mode = "0755" if metadata.st_mode & stat.S_IXUSR else "0644"
        elif stat.S_ISLNK(metadata.st_mode):
            content = os.readlink(path).encode("utf-8", errors="surrogateescape")
            kind = "symlink"
            mode = "0777"
        else:
            raise ValueError(f"candidate identity cannot inventory non-file path {relative}")
        inventory.append(
            {
                "path": relative.as_posix(),
                "state": "present",
                "kind": kind,
                "mode": mode,
                "bytes": len(content),
                "content_digest": hashlib.sha256(content).hexdigest(),
            }
        )
    return inventory


def _v2_candidate_seal(root: Path, digest: str) -> tuple[str, int]:
    excluded_path = Path("kag/receipts/index_family_budget") / f"{digest}.json"
    inventory = _v2_candidate_inventory(root, excluded_path)
    material = {
        "contract_version": "aoa-kag:budget-receipt-candidate-identity-v2",
        "algorithm": "sha256:canonical-json-file-inventory-v2",
        "excluded_path": excluded_path.as_posix(),
        "files": inventory,
    }
    encoded = json.dumps(
        material,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest(), len(inventory)


def _v2_source_epoch_control_path(relative: Path) -> bool:
    """Match aoa-kag's generated/index and receipt exclusions for source epochs."""
    return (
        Path("kag/indexes") in (relative, *relative.parents)
        or Path("kag/receipts/index_family_budget") in (relative, *relative.parents)
    )


def _v2_git_nul_paths(root: Path, *arguments: str) -> set[Path]:
    result = subprocess.run(
        ("git", *arguments, "-z"),
        cwd=root,
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        raise ValueError("source epoch requires a readable Git worktree")
    paths: set[Path] = set()
    for raw in result.stdout.split(b"\0"):
        if not raw:
            continue
        try:
            relative = Path(raw.decode("utf-8", errors="strict"))
        except UnicodeDecodeError as exc:
            raise ValueError("source epoch encountered a non-UTF-8 path") from exc
        if relative.is_absolute() or not relative.parts or ".." in relative.parts:
            raise ValueError("source epoch encountered an unsafe path")
        paths.add(relative)
    return paths


def _v2_source_epoch(root: Path) -> str:
    """Recompute aoa-kag's source epoch from the current clean Git index."""
    head = subprocess.run(
        ("git", "rev-parse", "HEAD"),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if head.returncode != 0:
        raise ValueError("source epoch requires a readable Git worktree")

    staged = _v2_git_nul_paths(root, "diff", "--name-only", "--cached")
    unstaged = _v2_git_nul_paths(root, "diff", "--name-only")
    untracked = _v2_git_nul_paths(root, "ls-files", "--others", "--exclude-standard")
    dirty = {
        path
        for path in staged | unstaged | untracked
        if not _v2_source_epoch_control_path(path)
    }
    if dirty:
        raise ValueError(
            "source epoch is not clean; source drift is present at: "
            + ", ".join(path.as_posix() for path in sorted(dirty))
        )

    result = subprocess.run(
        ("git", "ls-files", "-s", "--cached", "-z"),
        cwd=root,
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        raise ValueError("source epoch cannot inspect the Git worktree")
    entries: list[dict[str, Any]] = []
    for item in result.stdout.split(b"\0"):
        if not item:
            continue
        metadata, separator, path_bytes = item.partition(b"\t")
        fields = metadata.split(b" ")
        if not separator or len(fields) != 3:
            raise ValueError("source epoch found a malformed Git index entry")
        mode, blob_id, stage = (field.decode("ascii") for field in fields)
        try:
            relative = Path(path_bytes.decode("utf-8", errors="strict"))
        except UnicodeDecodeError as exc:
            raise ValueError("source epoch encountered a non-UTF-8 path") from exc
        if stage != "0" or relative.is_absolute() or ".." in relative.parts:
            raise ValueError("source epoch found an unstable Git index entry")
        if _v2_source_epoch_control_path(relative):
            continue
        path = root / relative
        try:
            metadata = path.lstat()
        except FileNotFoundError as exc:
            raise ValueError(f"source epoch source path is missing: {relative.as_posix()}") from exc
        if mode == "160000":
            entries.append(
                {
                    "path": relative.as_posix(),
                    "mode": mode,
                    "blob_id": blob_id,
                    "kind": "gitlink",
                }
            )
            continue
        if stat.S_ISREG(metadata.st_mode):
            content = path.read_bytes()
            kind = "file"
        elif stat.S_ISLNK(metadata.st_mode):
            content = os.readlink(path).encode("utf-8", errors="surrogateescape")
            kind = "symlink"
        else:
            raise ValueError(f"source epoch found a non-file source path: {relative.as_posix()}")
        expected_kind = "symlink" if mode == "120000" else "file"
        if kind != expected_kind:
            raise ValueError(f"source epoch mode changed for {relative.as_posix()}")
        entries.append(
            {
                "path": relative.as_posix(),
                "mode": mode,
                "kind": kind,
                "bytes": len(content),
                "content_digest": hashlib.sha256(content).hexdigest(),
            }
        )
    material = {
        "contract_version": KAG_BUDGET_SOURCE_EPOCH_VERSION,
        "files": sorted(entries, key=lambda item: item["path"]),
    }
    encoded = json.dumps(
        material,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return "sha256:" + hashlib.sha256(encoded).hexdigest()


def _v2_canonical_family_output_path(manifest: Mapping[str, Any]) -> str | None:
    compatibility = manifest.get("compatibility")
    files = compatibility.get("files") if isinstance(compatibility, Mapping) else None
    if not isinstance(files, list):
        return None
    source_paths: list[str] = []
    for item in files:
        if not isinstance(item, Mapping) or item.get("kind") != "source":
            continue
        path = item.get("path")
        if not isinstance(path, str) or not path:
            return None
        relative = Path(path)
        if relative.is_absolute() or not relative.parts or ".." in relative.parts:
            return None
        source_paths.append(relative.as_posix())
    return source_paths[0] if len(source_paths) == 1 else None


_V2_MANIFEST_PATH = Path("kag/indexes/index_family.manifest.json")
_V2_LEGACY_INDEX_FILENAMES = (
    "source_surface_index.json",
    "repo_artifact_index.json",
    "repo_anchor_index.json",
    "repo_entity_index.json",
    "repo_event_index.json",
    "repo_assertion_index.json",
    "repo_relation_index.json",
)


def _v2_git_bytes(root: Path, ref: str, path: Path) -> bytes | None:
    result = subprocess.run(
        ("git", "show", f"{ref}:{path.as_posix()}"),
        cwd=root,
        check=False,
        capture_output=True,
    )
    return result.stdout if result.returncode == 0 else None


def _v2_expected_generated_paths(
    root: Path,
    manifest: Mapping[str, Any],
    base_ref: str,
) -> tuple[set[Path], set[Path]]:
    """Mirror aoa-kag's portable generated path set for delta recomputation."""
    head_paths = {_V2_MANIFEST_PATH}
    descriptors = manifest.get("shards")
    if not isinstance(descriptors, list):
        raise ValueError("current family manifest shards must be an array")
    for descriptor in descriptors:
        if not isinstance(descriptor, Mapping) or not isinstance(descriptor.get("path"), str):
            raise ValueError("current family manifest contains a malformed shard descriptor")
        head_paths.add(Path(descriptor["path"]))

    base_manifest_bytes = _v2_git_bytes(root, base_ref, _V2_MANIFEST_PATH)
    if base_manifest_bytes is None:
        base_paths = {
            Path("kag/indexes") / filename
            for filename in _V2_LEGACY_INDEX_FILENAMES
            if _v2_git_bytes(root, base_ref, Path("kag/indexes") / filename) is not None
        }
        return head_paths, base_paths
    try:
        base_manifest = json.loads(base_manifest_bytes)
    except json.JSONDecodeError as exc:
        raise ValueError("base family manifest is invalid") from exc
    if not isinstance(base_manifest, Mapping):
        raise ValueError("base family manifest must be an object")
    if base_manifest.get("schema_version") == "aoa-repo-local-kag-distribution-manifest-v1":
        corpus = _v2_git_bytes(root, base_ref, Path("kag/indexes/corpus.manifest.json"))
        hot_profile = _v2_git_bytes(root, base_ref, Path("kag/indexes/hot_profile.json"))
        if corpus is None or hot_profile is None:
            raise ValueError("base tiered family control manifests are incomplete")
        try:
            corpus_payload = json.loads(corpus)
            hot_payload = json.loads(hot_profile)
        except json.JSONDecodeError as exc:
            raise ValueError("base tiered family control manifest is invalid") from exc
        objects = corpus_payload.get("objects") if isinstance(corpus_payload, Mapping) else None
        selection = hot_payload.get("selection") if isinstance(hot_payload, Mapping) else None
        hot_kinds = selection.get("include_record_kinds") if isinstance(selection, Mapping) else None
        placement = base_manifest.get("placement")
        placement_state = placement.get("state") if isinstance(placement, Mapping) else None
        if (
            not isinstance(objects, list)
            or not isinstance(hot_kinds, list)
            or not isinstance(placement_state, str)
            or placement_state not in {"shadow", "externalized"}
        ):
            raise ValueError("base tiered family placement is malformed")
        base_paths = {
            _V2_MANIFEST_PATH,
            Path("kag/indexes/corpus.manifest.json"),
            Path("kag/indexes/hot_profile.json"),
            Path("kag/indexes/artifact_locators.json"),
        }
        for descriptor in objects:
            if not isinstance(descriptor, Mapping):
                raise ValueError("base tiered object descriptor is malformed")
            kind = descriptor.get("kind")
            range_value = descriptor.get("range")
            if not isinstance(kind, str) or not isinstance(range_value, str):
                raise ValueError("base tiered object path is malformed")
            if placement_state == "shadow" or kind in hot_kinds:
                base_paths.add(Path("kag/indexes/shards") / kind / f"{range_value}.jsonl")
        return head_paths, base_paths
    base_paths = {_V2_MANIFEST_PATH}
    base_shards = base_manifest.get("shards")
    if not isinstance(base_shards, list):
        raise ValueError("base family manifest shards must be an array")
    for descriptor in base_shards:
        if not isinstance(descriptor, Mapping) or not isinstance(descriptor.get("path"), str):
            raise ValueError("base family manifest contains a malformed shard descriptor")
        base_paths.add(Path(descriptor["path"]))
    return head_paths, base_paths


def _v2_changed_generated_measurements(
    root: Path,
    manifest: Mapping[str, Any],
    base_ref: str,
) -> tuple[int, int]:
    """Recompute the generated delta from the bound base and current family."""
    resolved = subprocess.run(
        ("git", "rev-parse", "--verify", f"{base_ref}^{{commit}}"),
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if resolved.returncode != 0:
        raise ValueError("budget receipt base_ref cannot be resolved")
    resolved_ref = resolved.stdout.strip()
    if resolved_ref != base_ref:
        raise ValueError("budget receipt base_ref must be the resolved commit identity")
    head_paths, base_paths = _v2_expected_generated_paths(root, manifest, resolved_ref)
    changed_bytes = 0
    changed_files = 0
    for path in sorted(head_paths | base_paths):
        old = _v2_git_bytes(root, resolved_ref, path)
        current = root / path
        new = current.read_bytes() if current.is_file() else None
        if old == new:
            continue
        changed_files += 1
        changed_bytes += max(len(old or b""), len(new or b""))
    return changed_bytes, changed_files


def _v2_non_python_input_issues(
    label: str,
    value: object,
    procedure_manifest: object,
    producer_files: object,
) -> list[Issue]:
    """Validate and bind non-Python runtime files to producer source records."""
    if not isinstance(value, list) or not value:
        return [(label, "budget receipt producer non-Python inputs must be a non-empty array")]
    issues: list[Issue] = []
    expected_paths: set[str] = set()
    if isinstance(procedure_manifest, Mapping):
        for field in ("action_path", "manifest_path", "schema_path"):
            path = procedure_manifest.get(field)
            if isinstance(path, str) and path:
                expected_paths.add(path)
        schema_inputs = procedure_manifest.get("schema_inputs")
        if isinstance(schema_inputs, list):
            expected_paths.update(path for path in schema_inputs if isinstance(path, str) and path)

    producer_paths: dict[str, list[Mapping[str, Any]]] = {}
    if isinstance(producer_files, list):
        for item in producer_files:
            if isinstance(item, Mapping) and isinstance(item.get("path"), str):
                producer_paths.setdefault(item["path"], []).append(item)

    actual_paths: list[str] = []
    for index, item in enumerate(value):
        field = f"producer_identity.execution_inputs.non_python_inputs[{index}]"
        issues.extend(_v2_object_shape_issues(label, item, {"path", "content_digest", "bytes"}, field))
        if not isinstance(item, Mapping):
            continue
        path = item.get("path")
        if not isinstance(path, str) or not path or Path(path).is_absolute() or ".." in Path(path).parts:
            issues.append((label, f"budget receipt field {field}.path must be a relative path"))
            continue
        actual_paths.append(path)
        if path not in expected_paths:
            issues.append((label, f"budget receipt non-Python input path {path} is not declared by the producer procedure"))
        digest_issue = _v2_digest_issue(label, f"{field}.content_digest", item.get("content_digest"))
        if digest_issue:
            issues.append(digest_issue)
        if not _is_integer(item.get("bytes")) or item["bytes"] < 0:
            issues.append((label, f"budget receipt field {field}.bytes must be a non-negative integer"))
        records = producer_paths.get(path, [])
        if len(records) != 1:
            issues.append((label, f"budget receipt non-Python input path {path} must identify exactly one producer file"))
        else:
            producer_file = records[0]
            if item.get("content_digest") != producer_file.get("content_digest"):
                issues.append((label, f"budget receipt non-Python input {path} digest does not match producer file"))
            if item.get("bytes") != producer_file.get("bytes"):
                issues.append((label, f"budget receipt non-Python input {path} bytes do not match producer file"))

    canonical_paths = sorted(expected_paths)
    if sorted(actual_paths) != canonical_paths or len(actual_paths) != len(set(actual_paths)):
        issues.append((label, "budget receipt non-Python inputs do not match the producer procedure and file inventory"))
    return issues


def _v2_pinned_owner_file_issues(label: str, producer_files: object) -> list[Issue]:
    """Resolve producer file records against the pinned aoa-kag action source."""
    owner_root_text = os.environ.get("AOA_KAG_ROOT")
    if not owner_root_text:
        return []
    owner_root = Path(owner_root_text)
    revision = os.environ.get("AOA_KAG_ACTION_REVISION") or os.environ.get("AOA_KAG_REVISION")
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40,64}", revision):
        return [(label, "budget receipt producer files require a pinned aoa-kag revision")]
    if not owner_root.is_dir():
        return [(label, "budget receipt producer files cannot access the pinned aoa-kag checkout")]
    git_check = subprocess.run(
        ("git", "rev-parse", "--git-dir"),
        cwd=owner_root,
        check=False,
        capture_output=True,
    )
    if git_check.returncode != 0:
        return [(label, "budget receipt producer files cannot read the pinned aoa-kag checkout")]
    if not isinstance(producer_files, list):
        return []

    issues: list[Issue] = []
    for index, item in enumerate(producer_files):
        if not isinstance(item, Mapping):
            continue
        path_text = item.get("path")
        if not isinstance(path_text, str) or not path_text:
            continue
        if any(ord(character) < 32 or ord(character) == 127 for character in path_text):
            issues.append(
                (
                    label,
                    f"budget receipt producer file {path_text!r} path contains control characters",
                )
            )
            continue
        relative = Path(path_text)
        if relative.is_absolute() or ".." in relative.parts:
            continue
        spec = f"{revision}:{relative.as_posix()}"
        blob_result = subprocess.run(
            ("git", "rev-parse", "--verify", spec),
            cwd=owner_root,
            check=False,
            capture_output=True,
            text=True,
        )
        source_result = subprocess.run(
            ("git", "show", spec),
            cwd=owner_root,
            check=False,
            capture_output=True,
        )
        if blob_result.returncode != 0 or source_result.returncode != 0:
            issues.append(
                (
                    label,
                    f"budget receipt producer file {path_text} is missing from pinned aoa-kag revision",
                )
            )
            continue
        source_bytes = source_result.stdout
        expected_blob = blob_result.stdout.strip()
        expected_digest = hashlib.sha256(source_bytes).hexdigest()
        expected_bytes = len(source_bytes)
        if item.get("content_digest") != expected_digest:
            issues.append(
                (
                    label,
                    f"budget receipt producer file {path_text} content does not match pinned aoa-kag source",
                )
            )
        if item.get("bytes") != expected_bytes:
            issues.append(
                (
                    label,
                    f"budget receipt producer file {path_text} bytes do not match pinned aoa-kag source",
                )
            )
        if item.get("git_blob") != f"sha1:{expected_blob}":
            issues.append(
                (
                    label,
                    f"budget receipt producer file {path_text} git_blob does not match pinned aoa-kag source",
                )
            )
    return issues


def _v2_procedure_manifest_issues(
    label: str,
    procedure_manifest: object,
) -> list[Issue]:
    """Bind the projected procedure manifest to its canonical owner payload."""
    if not isinstance(procedure_manifest, Mapping):
        return []
    issues: list[Issue] = []
    for field, expected in (
        ("manifest_path", KAG_BUDGET_PROCEDURE_MANIFEST_PATH),
        ("schema_path", KAG_BUDGET_PROCEDURE_SCHEMA_PATH),
        ("action_path", KAG_BUDGET_PROCEDURE_ACTION_PATH),
    ):
        if procedure_manifest.get(field) != expected:
            issues.append(
                (
                    label,
                    f"budget receipt producer procedure manifest {field} must be {expected}",
                )
            )

    owner_root_text = os.environ.get("AOA_KAG_ROOT")
    revision = os.environ.get("AOA_KAG_ACTION_REVISION") or os.environ.get("AOA_KAG_REVISION")
    if not owner_root_text or not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40,64}", revision):
        return issues
    owner_root = Path(owner_root_text)
    if not owner_root.is_dir():
        issues.append((label, "budget receipt procedure manifest cannot access the pinned aoa-kag checkout"))
        return issues
    source = subprocess.run(
        ("git", "show", f"{revision}:{KAG_BUDGET_PROCEDURE_MANIFEST_PATH}"),
        cwd=owner_root,
        check=False,
        capture_output=True,
    )
    if source.returncode != 0:
        issues.append((label, "budget receipt procedure manifest is missing from the pinned aoa-kag revision"))
        return issues
    raw = source.stdout
    try:
        payload = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        issues.append((label, "budget receipt procedure manifest cannot be decoded from the pinned aoa-kag revision"))
        return issues
    if not isinstance(payload, Mapping):
        issues.append((label, "budget receipt procedure manifest payload must be an object"))
        return issues
    if procedure_manifest.get("manifest_digest") != hashlib.sha256(raw).hexdigest():
        issues.append((label, "budget receipt producer procedure manifest digest does not match pinned owner payload"))
    for field in (
        "closure_mode",
        "dynamic_import_policy",
        "python_entrypoints",
        "python_import_closure",
        "schema_inputs",
        "action_inputs",
        "environment",
        "dependencies",
    ):
        if procedure_manifest.get(field) != payload.get(field):
            issues.append((label, f"budget receipt producer procedure manifest {field} does not match pinned owner payload"))
    return issues


def _v2_producer_file_inventory_issues(
    label: str,
    producer_files: object,
    procedure_manifest: object,
) -> list[Issue]:
    """Require the producer inventory to cover the declared procedure exactly."""
    if not isinstance(producer_files, list) or not isinstance(procedure_manifest, Mapping):
        return []
    declared_paths: set[str] = set()
    for field in (
        "python_entrypoints",
        "python_import_closure",
        "schema_inputs",
    ):
        values = procedure_manifest.get(field)
        if isinstance(values, list):
            declared_paths.update(value for value in values if isinstance(value, str))
    for field in ("manifest_path", "schema_path", "action_path"):
        value = procedure_manifest.get(field)
        if isinstance(value, str):
            declared_paths.add(value)
    actual_paths = [
        item.get("path")
        for item in producer_files
        if isinstance(item, Mapping) and isinstance(item.get("path"), str)
    ]
    actual_set = set(actual_paths)
    missing = sorted(declared_paths - actual_set)
    extra = sorted(actual_set - declared_paths)
    if missing or extra or len(actual_paths) != len(actual_set):
        details: list[str] = []
        if missing:
            details.append("missing=" + ",".join(missing))
        if extra:
            details.append("extra=" + ",".join(extra))
        if len(actual_paths) != len(actual_set):
            details.append("duplicate=present")
        return [
            (
                label,
                "budget receipt producer file inventory does not cover the declared procedure exactly"
                + (f" ({'; '.join(details)})" if details else ""),
            )
        ]
    return []


def v2_budget_receipt_identity_issues(
    root: Path,
    manifest: Mapping[str, Any],
    receipt: dict[str, Any],
    digest: str,
    receipt_path: Path,
) -> list[Issue]:
    """Validate the portable v2 identity shape before aoa-kag replay validation.

    Tree publishes the provider receipt but does not own the aoa-kag producer
    procedure.  This local check therefore admits only a complete, typed,
    path-safe v2 shape; the canonical aoa-kag validator remains responsible
    for recomputing the candidate and producer identities.
    """

    label = _relative_label(root, receipt_path)
    issues: list[Issue] = []
    receipt_base_ref = receipt.get("base_ref")
    base_ref = receipt_base_ref
    candidate = receipt.get("candidate_identity")
    issues.extend(
        _v2_object_shape_issues(
            label,
            candidate,
            KAG_BUDGET_CANDIDATE_IDENTITY_FIELDS,
            "candidate_identity",
        )
    )
    if isinstance(candidate, dict):
        if candidate.get("contract_version") != "aoa-kag:budget-receipt-candidate-identity-v2":
            issues.append((label, "budget receipt candidate identity contract is not v2"))
        if candidate.get("algorithm") != "sha256:canonical-json-file-inventory-v2":
            issues.append((label, "budget receipt candidate identity algorithm is not v2"))
        if candidate.get("base_ref") != receipt.get("base_ref"):
            issues.append((label, "budget receipt candidate identity base_ref does not match receipt"))
        if candidate.get("family_digest") != digest:
            issues.append((label, "budget receipt candidate identity family digest does not match receipt"))
        if candidate.get("source_snapshot") != receipt.get("head_source_snapshot"):
            issues.append((label, "budget receipt candidate identity source snapshot does not match receipt"))
        excluded_path = f"kag/receipts/index_family_budget/{digest}.json"
        if candidate.get("excluded_path") != excluded_path:
            issues.append((label, "budget receipt candidate identity excluded path is not canonical"))
        if not _is_integer(candidate.get("file_count")) or candidate["file_count"] < 0:
            issues.append((label, "budget receipt candidate identity file_count must be non-negative"))
        for field in ("seal", "family_digest"):
            digest_issue = _v2_digest_issue(label, f"candidate_identity.{field}", candidate.get(field))
            if digest_issue:
                issues.append(digest_issue)
        for field in ("source_snapshot", "source_epoch"):
            digest_issue = _v2_digest_issue(
                label,
                f"candidate_identity.{field}",
                candidate.get(field),
                prefixed=True,
            )
            if digest_issue:
                issues.append(digest_issue)
        base_ref = candidate.get("base_ref")
        if not isinstance(base_ref, str) or not re.fullmatch(r"[0-9a-f]{40,64}", base_ref):
            issues.append((label, "budget receipt candidate identity base_ref must be lowercase Git commit ref"))

        try:
            actual_seal, actual_file_count = _v2_candidate_seal(root, digest)
        except (OSError, ValueError) as exc:
            issues.append((label, f"budget receipt candidate seal cannot be recomputed: {exc}"))
        else:
            if candidate.get("seal") != actual_seal:
                issues.append((label, "budget receipt candidate identity seal does not match the current candidate"))
            if candidate.get("file_count") != actual_file_count:
                issues.append(
                    (
                        label,
                        "budget receipt candidate identity file_count does not match the current candidate",
                    )
                )
        source_epoch = candidate.get("source_epoch")
        if isinstance(source_epoch, str) and re.fullmatch(r"sha256:[0-9a-f]{64}", source_epoch):
            try:
                actual_source_epoch = _v2_source_epoch(root)
            except (OSError, ValueError) as exc:
                issues.append((label, f"budget receipt candidate source epoch cannot be recomputed: {exc}"))
            else:
                if source_epoch != actual_source_epoch:
                    issues.append(
                        (
                            label,
                            "budget receipt candidate identity source epoch does not match the current source",
                        )
                    )

    family_identity = manifest.get("family_identity")
    if not isinstance(family_identity, Mapping):
        issues.append((label, "family manifest family_identity must be an object"))
    else:
        manifest_source_snapshot = family_identity.get("source_snapshot")
        if not isinstance(manifest_source_snapshot, str):
            issues.append((label, "family manifest source snapshot must be a string"))
        elif receipt.get("head_source_snapshot") != manifest_source_snapshot:
            issues.append((label, "budget receipt source snapshot does not match the family manifest"))

    producer = receipt.get("producer_identity")
    issues.extend(
        _v2_object_shape_issues(
            label,
            producer,
            KAG_BUDGET_PRODUCER_IDENTITY_FIELDS,
            "producer_identity",
        )
    )
    if isinstance(producer, dict):
        if producer.get("owner") != "aoa-kag":
            issues.append((label, "budget receipt producer identity owner must be aoa-kag"))
        contract_version = producer.get("contract_version")
        producer_profile = (
            KAG_BUDGET_PRODUCER_PROFILES.get(contract_version)
            if isinstance(contract_version, str)
            else None
        )
        if producer_profile is None:
            issues.append((label, "budget receipt producer identity contract is unsupported"))
        elif producer.get("revision_binding") != producer_profile["revision_binding"]:
            issues.append((label, "budget receipt producer identity revision binding does not match its contract version"))
        for field in ("source_digest", "identity_digest"):
            digest_issue = _v2_digest_issue(label, f"producer_identity.{field}", producer.get(field))
            if digest_issue:
                issues.append(digest_issue)
        action = producer.get("action")
        issues.extend(_v2_producer_file_issues(label, action, "producer_identity.action"))
        files = producer.get("files")
        if not isinstance(files, list) or not files:
            issues.append((label, "budget receipt producer identity files must be a non-empty array"))
        else:
            for index, item in enumerate(files):
                issues.extend(_v2_producer_file_issues(label, item, f"producer_identity.files[{index}]"))
            expected_source_digest = _v2_canonical_digest(files)
            if producer.get("source_digest") != expected_source_digest:
                issues.append((label, "budget receipt producer source digest does not match its files"))
            issues.extend(_v2_pinned_owner_file_issues(label, files))
            if isinstance(action, dict):
                action_files = [
                    item
                    for item in files
                    if isinstance(item, dict) and item.get("path") == action.get("path")
                ]
                if len(action_files) != 1:
                    issues.append(
                        (
                            label,
                            "budget receipt producer action path must identify exactly one producer file",
                        )
                    )
                elif action_files[0] != action:
                    issues.append(
                        (
                            label,
                            "budget receipt producer action does not match its file record",
                        )
                    )

        identity_material_fields = (
            "contract_version",
            "owner",
            "revision_binding",
            "source_digest",
            "procedure_manifest",
            "action",
            "execution_inputs",
        )
        if all(field in producer for field in identity_material_fields):
            expected_identity_digest = _v2_canonical_digest(
                {field: producer[field] for field in identity_material_fields}
            )
            if producer.get("identity_digest") != expected_identity_digest:
                issues.append((label, "budget receipt producer identity digest does not match its identity material"))
        procedure_manifest = producer.get("procedure_manifest")
        if not isinstance(procedure_manifest, dict):
            issues.append((label, "budget receipt producer identity procedure_manifest must be an object"))
        else:
            issues.extend(
                _v2_object_shape_issues(
                    label,
                    procedure_manifest,
                    KAG_BUDGET_PROCEDURE_MANIFEST_FIELDS,
                    "producer_identity.procedure_manifest",
                )
            )
            for field in (
                "manifest_path",
                "schema_path",
                "closure_mode",
                "dynamic_import_policy",
                "action_path",
            ):
                if not isinstance(procedure_manifest.get(field), str) or not procedure_manifest[field]:
                    issues.append(
                        (
                            label,
                            f"budget receipt producer procedure manifest field {field} must be a non-empty string",
                        )
                    )
            digest_issue = _v2_digest_issue(
                label,
                "producer_identity.procedure_manifest.manifest_digest",
                procedure_manifest.get("manifest_digest"),
            )
            if digest_issue:
                issues.append(digest_issue)
            if isinstance(files, list):
                manifest_path = procedure_manifest.get("manifest_path")
                manifest_files = [
                    item
                    for item in files
                    if isinstance(item, dict) and item.get("path") == manifest_path
                ]
                if len(manifest_files) != 1:
                    issues.append(
                        (
                            label,
                            "budget receipt producer procedure manifest path must identify exactly one producer file",
                        )
                    )
                elif manifest_files[0].get("content_digest") != procedure_manifest.get("manifest_digest"):
                    issues.append(
                        (
                            label,
                            "budget receipt producer procedure manifest digest does not match its file record",
                        )
                    )
                action_path = procedure_manifest.get("action_path")
                if isinstance(action, dict) and action.get("path") != action_path:
                    issues.append(
                        (
                            label,
                            "budget receipt producer action path does not match procedure manifest",
                        )
                    )
            for field in (
                "python_entrypoints",
                "python_import_closure",
                "schema_inputs",
                "action_inputs",
                "environment",
                "dependencies",
            ):
                if not isinstance(procedure_manifest.get(field), list) or not procedure_manifest[field]:
                    issues.append(
                        (
                            label,
                            f"budget receipt producer procedure manifest field {field} must be a non-empty array",
                        )
                    )

        issues.extend(_v2_procedure_manifest_issues(label, procedure_manifest))
        issues.extend(_v2_producer_file_inventory_issues(label, files, procedure_manifest))
        execution = producer.get("execution_inputs")
        execution_fields = {
            "schema_version",
            "action_inputs",
            "environment",
            "dependencies",
            "interpreter",
            "non_python_inputs",
            "dynamic_imports",
            "command_targets",
            "manifest_digest",
        }
        issues.extend(_v2_object_shape_issues(label, execution, execution_fields, "producer_identity.execution_inputs"))
        if isinstance(execution, dict):
            expected_runtime_inputs = (
                producer_profile["runtime_inputs"]
                if producer_profile is not None
                else None
            )
            if execution.get("schema_version") != expected_runtime_inputs:
                issues.append((label, "budget receipt producer runtime-input schema does not match its contract version"))
            digest_issue = _v2_digest_issue(
                label,
                "producer_identity.execution_inputs.manifest_digest",
                execution.get("manifest_digest"),
            )
            if digest_issue:
                issues.append(digest_issue)
            if execution.get("dynamic_imports") != []:
                issues.append((label, "budget receipt producer runtime-input dynamic_imports must be empty"))
            action_inputs = execution.get("action_inputs")
            issues.extend(
                _v2_object_shape_issues(
                    label,
                    action_inputs,
                    {"repo-root", "output", "history-ref", "event-history-ref", "jobs"},
                    "producer_identity.execution_inputs.action_inputs",
                )
            )
            if isinstance(action_inputs, dict):
                issues.extend(
                    _v2_owner_root_input_issues(
                        label,
                        action_inputs.get("repo-root"),
                        "producer_identity.execution_inputs.action_inputs.repo-root",
                    )
                )
            canonical_output_path = _v2_canonical_family_output_path(manifest)
            if canonical_output_path is None:
                issues.append((label, "budget receipt canonical family output path is missing"))
            elif isinstance(action_inputs, dict):
                issues.extend(
                    _v2_relative_path_input_issues(
                        label,
                        action_inputs.get("output"),
                        "producer_identity.execution_inputs.action_inputs.output",
                        canonical_output_path,
                    )
                )
            if isinstance(receipt_base_ref, str) and isinstance(action_inputs, dict):
                for field in ("history-ref", "event-history-ref"):
                    issues.extend(
                        _v2_action_ref_issues(
                            label,
                            action_inputs.get(field),
                            f"producer_identity.execution_inputs.action_inputs.{field}",
                            receipt_base_ref,
                        )
                    )
            command_targets = execution.get("command_targets")
            issues.extend(
                _v2_object_shape_issues(
                    label,
                    command_targets,
                    {
                        "repo_root",
                        "base_ref",
                        "history_ref",
                        "event_history_ref",
                        "output",
                        "family_mode",
                        "artifact_root",
                        "externalized",
                        "jobs",
                    },
                    "producer_identity.execution_inputs.command_targets",
                )
            )
            if isinstance(action_inputs, dict):
                issues.extend(
                    _v2_jobs_input_issues(
                        label,
                        action_inputs.get("jobs"),
                        "producer_identity.execution_inputs.action_inputs.jobs",
                        command_targets.get("jobs")
                        if isinstance(command_targets, dict)
                        else None,
                    )
                )
            if isinstance(command_targets, dict):
                issues.extend(
                    _v2_repo_root_target_issues(
                        label,
                        command_targets.get("repo_root"),
                        "producer_identity.execution_inputs.command_targets.repo_root",
                    )
                )
                if command_targets.get("family_mode") != "portable":
                    issues.append((label, "budget receipt producer command target family_mode must be 'portable'"))
                if command_targets.get("artifact_root") is not None:
                    issues.append((label, "budget receipt producer command target artifact_root must be null for portable family"))
                if command_targets.get("externalized") is not False:
                    issues.append((label, "budget receipt producer command target externalized must be false for portable family"))
                if not isinstance(command_targets.get("jobs"), str) or not re.fullmatch(
                    r"[1-3]", command_targets["jobs"]
                ):
                    issues.append(
                        (
                            label,
                            "budget receipt producer command target jobs must be a bounded integer in [1, 3]",
                        )
                    )
                if isinstance(action_inputs, dict):
                    repo_root_input = action_inputs.get("repo-root")
                    repo_root_target = command_targets.get("repo_root")
                    if isinstance(repo_root_input, dict) and isinstance(repo_root_target, dict):
                        input_digest = repo_root_input.get("value_digest")
                        for field in ("path_digest", "resolved_path_digest"):
                            if repo_root_target.get(field) != input_digest:
                                issues.append(
                                    (
                                        label,
                                        f"budget receipt producer command target repo_root {field} does not match action input repo-root",
                                    )
                                )
            if isinstance(command_targets, dict) and isinstance(receipt_base_ref, str):
                for field in ("base_ref", "history_ref", "event_history_ref"):
                    if command_targets.get(field) != receipt_base_ref:
                        issues.append(
                            (
                                label,
                                f"budget receipt producer command target {field} does not match receipt base_ref",
                            )
                        )
            if isinstance(command_targets, dict) and canonical_output_path is not None:
                if command_targets.get("output") != canonical_output_path:
                    issues.append(
                        (
                            label,
                            "budget receipt producer command target output does not match canonical family output",
                        )
                    )
            if (
                isinstance(procedure_manifest, dict)
                and isinstance(execution.get("manifest_digest"), str)
                and procedure_manifest.get("manifest_digest") != execution.get("manifest_digest")
            ):
                issues.append(
                    (
                        label,
                        "budget receipt producer procedure manifest digest does not match execution inputs",
                    )
                )
            issues.extend(
                _v2_runtime_environment_issues(
                    label,
                    execution.get("environment"),
                    procedure_manifest,
                )
            )
            issues.extend(
                _v2_runtime_dependency_issues(
                    label,
                    execution.get("dependencies"),
                    procedure_manifest,
                )
            )
            issues.extend(
                _v2_runtime_interpreter_issues(
                    label,
                    execution.get("interpreter"),
                    procedure_manifest,
                )
            )
            issues.extend(
                _v2_non_python_input_issues(
                    label,
                    execution.get("non_python_inputs"),
                    procedure_manifest,
                    files,
                )
            )
    return issues


def budget_receipt_contract_issues(
    root: Path,
    manifest: dict[str, Any],
    receipt: Any,
    digest: str,
    receipt_path: Path,
    *,
    base_has_v3: bool | None = None,
    fetch_missing_base: bool = False,
    require_v2: bool = False,
) -> list[Issue]:
    """Admit historical v1 and current identity-bound v2 receipt shapes."""
    label = _relative_label(root, receipt_path)
    if not isinstance(receipt, dict):
        return [(label, "budget receipt must be a JSON object")]

    issues: list[Issue] = []
    schema_version = receipt.get("schema_version")
    if require_v2 and schema_version != KAG_BUDGET_RECEIPT_SCHEMA_V2_VERSION:
        issues.append((label, "current generated KAG budget receipt must use the identity-bound v2 schema"))
    required_fields = (
        KAG_BUDGET_RECEIPT_V2_REQUIRED_FIELDS
        if schema_version == KAG_BUDGET_RECEIPT_SCHEMA_V2_VERSION
        else KAG_BUDGET_RECEIPT_REQUIRED_FIELDS
    )
    for field in sorted(required_fields):
        if field not in receipt:
            issues.append((label, f"budget receipt missing required field {field}"))

    string_fields = (
        "schema_version",
        "repo",
        "scope",
        "base_ref",
        "head_family_digest",
        "reason",
        "approved_by",
        "decision_ref",
    )
    if schema_version == KAG_BUDGET_RECEIPT_SCHEMA_V2_VERSION:
        string_fields = (*string_fields, "head_source_snapshot")
    for field in string_fields:
        if field in receipt and not isinstance(receipt[field], str):
            issues.append((label, f"budget receipt field {field} must be a string"))

    integer_fields = (
        "changed_generated_bytes",
        "changed_generated_files",
        "default_limit_bytes",
        "allowed_bytes",
        "tracked_bytes",
        "tracked_bytes_max",
        "allowed_tracked_bytes",
    )
    for field in integer_fields:
        if field in receipt and not _is_integer(receipt[field]):
            issues.append((label, f"budget receipt field {field} must be an integer"))
        elif field in receipt and receipt[field] < 0:
            issues.append((label, f"budget receipt field {field} must not be negative"))

    if isinstance(schema_version, str) and schema_version not in KAG_BUDGET_RECEIPT_SCHEMA_VERSIONS:
        issues.append(
            (
                label,
                "budget receipt schema_version must be one of "
                f"{sorted(KAG_BUDGET_RECEIPT_SCHEMA_VERSIONS)!r}",
            )
        )

    if schema_version == KAG_BUDGET_RECEIPT_SCHEMA_V2_VERSION:
        expected_fields = KAG_BUDGET_RECEIPT_V2_REQUIRED_FIELDS
        missing = sorted(expected_fields - set(receipt))
        extra = sorted(set(receipt) - expected_fields)
        for field in missing:
            issues.append((label, f"budget receipt missing required field {field}"))
        for field in extra:
            issues.append((label, f"budget receipt has unexpected field {field}"))
        digest_issue = _v2_digest_issue(label, "head_source_snapshot", receipt.get("head_source_snapshot"), prefixed=True)
        if digest_issue:
            issues.append(digest_issue)
        issues.extend(v2_budget_receipt_identity_issues(root, manifest, receipt, digest, receipt_path))

    repo = manifest.get("repo")
    expected_repo = repo.get("name") if isinstance(repo, dict) else None
    if isinstance(expected_repo, str) and receipt.get("repo") != expected_repo:
        issues.append((label, "budget receipt repo must match the family repository"))
    if isinstance(repo, dict):
        history_ref = repo.get("git_ref")
        if not isinstance(history_ref, str) or not history_ref.strip():
            issues.append((label, "family repository must keep a non-empty history ref"))
    else:
        issues.append((label, "family manifest repo must be an object with a history ref"))

    scope = receipt.get("scope")
    if isinstance(scope, str) and scope not in KAG_BUDGET_RECEIPT_SCOPES:
        issues.append((label, "budget receipt scope is not recognized by the aoa-kag v1 contract"))

    base_ref = receipt.get("base_ref")
    if isinstance(base_ref, str) and not re.fullmatch(r"[0-9a-f]{40,64}", base_ref):
        issues.append((label, "budget receipt base_ref must be a lowercase Git commit ref"))
    if receipt.get("head_family_digest") != digest:
        issues.append((label, "budget receipt is not bound to the current family digest"))

    budgets = manifest.get("budgets")
    summary = manifest.get("summary")
    expected_values: dict[str, Any] = {}
    if isinstance(budgets, dict):
        expected_values["default_limit_bytes"] = budgets.get("changed_generated_bytes_max")
        expected_values["tracked_bytes_max"] = budgets.get("tracked_bytes_max")
    if isinstance(summary, dict):
        expected_values["tracked_bytes"] = summary.get("tracked_bytes")
    expected_values["decision_ref"] = (
        KAG_TIERED_DECISION_REF
        if manifest.get("schema_version") == "aoa-repo-local-kag-distribution-manifest-v1"
        else KAG_BUDGET_DECISION_REF
    )
    for field, expected in expected_values.items():
        if expected is not None and receipt.get(field) != expected:
            issues.append((label, f"budget receipt field {field} does not match the family contract"))

    if (
        schema_version == KAG_BUDGET_RECEIPT_SCHEMA_V2_VERSION
        and isinstance(base_ref, str)
        and re.fullmatch(r"[0-9a-f]{40,64}", base_ref)
    ):
        try:
            actual_changed_bytes, actual_changed_files = _v2_changed_generated_measurements(
                root,
                manifest,
                base_ref,
            )
        except (OSError, ValueError) as exc:
            issues.append((label, f"budget receipt generated delta cannot be recomputed: {exc}"))
        else:
            for field, actual in (
                ("changed_generated_bytes", actual_changed_bytes),
                ("changed_generated_files", actual_changed_files),
                ("allowed_bytes", actual_changed_bytes),
            ):
                if receipt.get(field) != actual:
                    issues.append((label, f"budget receipt field {field} does not match current generated delta"))

    relation: str | None = None
    relation_inputs = (
        receipt.get("changed_generated_bytes"),
        budgets.get("changed_generated_bytes_max") if isinstance(budgets, dict) else None,
        summary.get("tracked_bytes") if isinstance(summary, dict) else None,
        budgets.get("tracked_bytes_max") if isinstance(budgets, dict) else None,
    )
    if all(_is_integer(value) for value in relation_inputs):
        try:
            relation = budget_exceedance_relation(
                changed_generated_bytes=relation_inputs[0],
                default_limit_bytes=relation_inputs[1],
                tracked_bytes=relation_inputs[2],
                tracked_bytes_max=relation_inputs[3],
            )
        except ValueError as exc:
            issues.append((label, f"budget receipt exceedance relation is malformed: {exc}"))

    def verified_base_has_v3() -> bool | None:
        if base_has_v3 is not None:
            return base_has_v3
        if not isinstance(base_ref, str):
            return None
        return _verified_base_has_v3_manifest(
            root,
            base_ref,
            fetch_missing_base=fetch_missing_base,
        )

    if isinstance(scope, str) and scope in KAG_BUDGET_RECEIPT_SCOPES:
        # Base state, measured relation, and requested scope are independent
        # admission inputs. Resolve the exact base before applying either the
        # pre-v3 exception or the v3 measured-scope mapping so an unavailable
        # base remains fail-closed for every recognized scope.
        base_has_v3_result = verified_base_has_v3()
        if base_has_v3_result is None:
            if scope == "v2_to_v3_migration":
                issues.append((label, "cannot verify v2_to_v3_migration base family"))
            else:
                issues.append((label, "cannot verify budget receipt base family"))
        elif relation is None:
            issues.append((label, "budget receipt scope cannot authorize a family with no exceeded budget dimension"))
        elif scope == "v2_to_v3_migration":
            if base_has_v3_result:
                issues.append((label, "v2_to_v3_migration requires a base without a v3 family manifest"))
        elif base_has_v3_result is False:
            expected_scope = canonical_budget_scope(relation, base_has_v3=False)
            issues.append(
                (
                    label,
                    f"budget receipt scope must be {expected_scope!r} for a pre-v3 base family",
                )
            )
        else:
            expected_scope = canonical_budget_scope(relation, base_has_v3=True)
            if scope != expected_scope:
                issues.append(
                    (
                        label,
                        f"budget receipt scope does not match the current exceedance: expected {expected_scope!r}",
                    )
                )

    changed_bytes = receipt.get("changed_generated_bytes")
    allowed_bytes = receipt.get("allowed_bytes")
    if _is_integer(changed_bytes) and _is_integer(allowed_bytes) and allowed_bytes < changed_bytes:
        issues.append((label, "budget receipt allowed_bytes must cover changed_generated_bytes"))
    tracked_bytes = receipt.get("tracked_bytes")
    allowed_tracked_bytes = receipt.get("allowed_tracked_bytes")
    if (
        _is_integer(tracked_bytes)
        and _is_integer(allowed_tracked_bytes)
        and allowed_tracked_bytes < tracked_bytes
    ):
        issues.append((label, "budget receipt allowed_tracked_bytes must cover tracked_bytes"))
    for field in ("reason", "approved_by"):
        if isinstance(receipt.get(field), str) and not receipt[field].strip():
            issues.append((label, f"budget receipt {field} must not be empty"))
    return issues


def generated_family_issues(
    root: Path,
    port: dict[str, Any],
    *,
    fetch_missing_base: bool = False,
) -> list[Issue]:
    """Check the KAG generated carrier without coupling it into currentness."""
    issues: list[Issue] = []
    family = port.get("generated_family")
    if not isinstance(family, dict):
        return [("kag_provider", "generated_family carrier declaration is required")]
    manifest_text = family.get("manifest")
    shards_text = family.get("shards")
    receipt_root_text = family.get("receipt_root")
    for label, relative in (
        ("manifest", manifest_text),
        ("shards", shards_text),
        ("receipt_root", receipt_root_text),
    ):
        if not isinstance(relative, str) or not relative:
            issues.append(("kag_provider", f"generated_family.{label} is required"))
            continue
        path = root / relative
        if label == "manifest" and not path.is_file():
            issues.append((relative, "generated KAG family manifest is missing"))
        if label != "manifest" and not path.is_dir():
            issues.append((relative, f"generated KAG family {label} route is missing"))
    builder = family.get("builder")
    validator = family.get("validator")
    if not isinstance(builder, str) or not builder.startswith("aoa-kag:"):
        issues.append(("kag_provider", "generated_family.builder must keep the aoa-kag owner handle"))
    if not isinstance(validator, str) or not validator.startswith("aoa-kag:"):
        issues.append(("kag_provider", "generated_family.validator must keep the aoa-kag owner handle"))
    if not isinstance(manifest_text, str) or not isinstance(receipt_root_text, str):
        return issues
    manifest_path = root / manifest_text
    if not manifest_path.is_file():
        return issues
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        digest = manifest["family_identity"]["content_digest"]
    except (OSError, json.JSONDecodeError, KeyError, TypeError):
        issues.append((manifest_text, "generated KAG family manifest lacks family_identity.content_digest"))
        return issues
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
        issues.append((manifest_text, "generated KAG family digest must be 64 lowercase hex characters"))
        return issues
    receipt_path = root / receipt_root_text / f"{digest}.json"
    if not receipt_path.is_file():
        issues.append((receipt_path.relative_to(root).as_posix(), "matching generated KAG budget receipt is missing"))
        return issues
    try:
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        issues.append((_relative_label(root, receipt_path), "matching generated KAG budget receipt is invalid"))
        return issues
    issues.extend(
        budget_receipt_contract_issues(
            root,
            manifest,
            receipt,
            digest,
            receipt_path,
            fetch_missing_base=fetch_missing_base,
            require_v2=True,
        )
    )
    return issues


def currentness_parity_issues(root: Path = REPO_ROOT) -> list[Issue]:
    path = root / CURRENTNESS_PATH
    if not path.is_file():
        return [(CURRENTNESS_PATH.as_posix(), "generated currentness is missing")]
    if path.read_bytes() != rendered_currentness(root):
        return [(CURRENTNESS_PATH.as_posix(), "generated currentness is stale")]
    return []


def _check_local_reference_routes(root: Path, package_path: Path) -> list[Issue]:
    issues: list[Issue] = []
    for relative_file in ("SKILL.md", "agents/openai.yaml"):
        path = package_path / relative_file
        text = path.read_text(encoding="utf-8")
        for match_object in PATH_REFERENCE_RE.finditer(text):
            match = match_object.group(0)
            if "repo:" in text[max(0, match_object.start() - 20):match_object.start()]:
                continue
            target = package_path / match
            if not target.is_file():
                issues.append((path.relative_to(root).as_posix(), f"broken local companion route {match}"))
        for match in re.findall(r"\./(assets/[A-Za-z0-9_.-]+)", text):
            target = package_path / match
            if not target.is_file():
                issues.append((path.relative_to(root).as_posix(), f"broken asset route {match}"))
    return issues


def validate_manifest(
    root: Path = REPO_ROOT,
    *,
    fetch_missing_budget_base: bool = False,
) -> list[Issue]:
    issues: list[Issue] = []
    manifest_path = root / MANIFEST_PATH
    try:
        import json
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return [(MANIFEST_PATH.as_posix(), "missing agent-surface manifest")]
    except Exception as exc:
        return [(MANIFEST_PATH.as_posix(), f"invalid JSON: {exc}")]

    if not isinstance(manifest, dict):
        return [(MANIFEST_PATH.as_posix(), "manifest root must be an object")]
    required = {
        "schema_version": "tos_agent_tool_owner_port_documentation_v1",
        "owner_repo": "Tree-of-Sophia",
        "owner_surface": ".agents/AGENTS.md",
        "human_entrypoint": ".agents/README.md",
        "generated_currentness": CURRENTNESS_PATH.as_posix(),
        "builder": "scripts/build_agent_surface_currentness.py",
        "validator": "scripts/validate_agent_surface.py",
    }
    for key, expected in required.items():
        if manifest.get(key) != expected:
            issues.append((MANIFEST_PATH.as_posix(), f"{key} must be {expected!r}"))
    for key in ("owner_surface", "human_entrypoint", "builder", "validator"):
        value = manifest.get(key)
        if isinstance(value, str) and not (root / value).is_file():
            issues.append((value, "declared route is missing"))

    budget = manifest.get("context_budget")
    if not isinstance(budget, dict):
        issues.append((MANIFEST_PATH.as_posix(), "context_budget must be an object"))
        budget = {}
    for key in (
        "discovery_description_max_words",
        "triggered_body_max_words",
        "mandatory_task_probe_depth_max",
    ):
        if not isinstance(budget.get(key), int) or budget[key] <= 0:
            issues.append((MANIFEST_PATH.as_posix(), f"context_budget.{key} must be positive"))

    packages = manifest.get("skills")
    package_by_id: dict[str, dict[str, Any]] = {}
    if not isinstance(packages, list):
        issues.append((MANIFEST_PATH.as_posix(), "skills must be a list"))
        packages = []
    for package in packages:
        if not isinstance(package, dict):
            issues.append((MANIFEST_PATH.as_posix(), "each skill record must be an object"))
            continue
        skill_id = package.get("id")
        if not isinstance(skill_id, str) or not skill_id:
            issues.append((MANIFEST_PATH.as_posix(), "skill record needs a non-empty id"))
            continue
        if skill_id in package_by_id:
            issues.append((MANIFEST_PATH.as_posix(), f"duplicate skill record {skill_id}"))
        package_by_id[skill_id] = package
        if not isinstance(package.get("family"), str) or not package["family"]:
            issues.append((MANIFEST_PATH.as_posix(), f"{skill_id} needs a family"))
        entrypoint = package.get("entrypoint")
        if not isinstance(entrypoint, str):
            issues.append((MANIFEST_PATH.as_posix(), f"{skill_id} needs an entrypoint"))
            continue
        entrypoint_path = root / entrypoint
        if not entrypoint_path.is_file():
            issues.append((entrypoint, f"{skill_id} entrypoint is missing"))
            continue
        package_path = entrypoint_path.parent
        try:
            current = parse_skill(entrypoint_path)
        except Exception as exc:
            issues.append((entrypoint, str(exc)))
            continue
        if current["id"] != skill_id:
            issues.append((entrypoint, f"frontmatter name {current['id']!r} does not match {skill_id!r}"))
        frontmatter = current["frontmatter"]
        if frontmatter["aoa_source_repo"] != "8Dionysus/aoa-skills":
            issues.append((entrypoint, "aoa_source_repo must be 8Dionysus/aoa-skills"))
        source_path = frontmatter["aoa_source_skill_path"]
        if not isinstance(source_path, str) or not source_path.startswith("skills/"):
            issues.append((entrypoint, "aoa_source_skill_path must identify an aoa-skills source path"))
        if current["frontmatter"]["description_words"] > budget.get("discovery_description_max_words", 0):
            issues.append((entrypoint, "discovery description exceeds context budget"))
        if current["triggered_body_words"] > budget.get("triggered_body_max_words", 0):
            issues.append((entrypoint, "triggered SKILL.md body exceeds context budget"))
        issues.extend(
            (location, f"{skill_id}: {message}")
            for location, message in _check_local_reference_routes(root, package_path)
        )
        issues.extend(
            (entrypoint, f"{skill_id}: {message}")
            for message in activation_policy_issues(
                frontmatter["aoa_invocation_mode"],
                current["activation"]["implicit_activation_policy"],
                current["activation"]["allow_implicit_invocation"],
                skill_id,
            )
        )

    discovered = {
        path.parent.name
        for path in sorted((root / SKILLS_ROOT).glob("*/SKILL.md"))
    }
    declared = set(package_by_id)
    if discovered != EXPECTED_SKILLS:
        issues.append((SKILLS_ROOT.as_posix(), f"discovered skills differ from expected 25: {sorted(discovered)}"))
    if declared != EXPECTED_SKILLS:
        issues.append((MANIFEST_PATH.as_posix(), f"declared skills differ from expected 25: {sorted(declared)}"))
    if discovered != declared:
        issues.append((MANIFEST_PATH.as_posix(), "manifest skill ids do not match local package discovery"))

    families = manifest.get("skill_families")
    if not isinstance(families, dict):
        issues.append((MANIFEST_PATH.as_posix(), "skill_families must be an object"))
        families = {}
    family_members: set[str] = set()
    for family_id, family in families.items():
        if not isinstance(family, dict):
            issues.append((MANIFEST_PATH.as_posix(), f"{family_id} family must be an object"))
            continue
        members = family.get("skills")
        if not isinstance(members, list) or not members:
            issues.append((MANIFEST_PATH.as_posix(), f"{family_id}.skills must be non-empty"))
            continue
        family_members.update(members)
        for required_field in ("consumer", "load_moment", "canonical_owner", "freshness", "next_organ", "negative_controls"):
            if not family.get(required_field):
                issues.append((MANIFEST_PATH.as_posix(), f"{family_id}.{required_field} is required"))
        for member in members:
            if member not in declared:
                issues.append((MANIFEST_PATH.as_posix(), f"{family_id} names unknown skill {member}"))
            elif package_by_id[member].get("family") != family_id:
                issues.append((MANIFEST_PATH.as_posix(), f"{member} family assignment disagrees"))
    if family_members != declared:
        issues.append((MANIFEST_PATH.as_posix(), "skill families do not cover exactly the declared skills"))

    ports = manifest.get("owner_ports")
    if not isinstance(ports, dict):
        issues.append((MANIFEST_PATH.as_posix(), "owner_ports must be an object"))
        ports = {}
    if set(ports) != EXPECTED_PORTS:
        issues.append((MANIFEST_PATH.as_posix(), f"owner ports must be {sorted(EXPECTED_PORTS)}"))
    for port_id, port in ports.items():
        if not isinstance(port, dict):
            issues.append((MANIFEST_PATH.as_posix(), f"{port_id} must be an object"))
            continue
        for field in ("local_owner", "manifest", "consumer", "load_moment", "canonical_owner", "freshness", "next_organ", "currentness_inputs", "carrier_classes"):
            if not port.get(field):
                issues.append((MANIFEST_PATH.as_posix(), f"{port_id}.{field} is required"))
        for route in (port.get("local_owner"), port.get("manifest")):
            if isinstance(route, str) and not (root / route).is_file():
                issues.append((route, f"{port_id} route is missing"))
        for relative in port.get("currentness_inputs", []):
            if not isinstance(relative, str) or not (root / relative).is_file():
                issues.append((str(relative), f"{port_id} currentness input is missing"))
        local_owner = port.get("local_owner")
        if isinstance(local_owner, str) and local_owner not in port.get("currentness_inputs", []):
            issues.append((MANIFEST_PATH.as_posix(), f"{port_id} local_owner must be a currentness input"))
        manifest_route = port.get("manifest")
        if isinstance(manifest_route, str) and manifest_route not in port.get("currentness_inputs", []):
            issues.append((MANIFEST_PATH.as_posix(), f"{port_id} manifest must be a currentness input"))
        if port_id == "kag_provider":
            issues.extend(
                generated_family_issues(
                    root,
                    port,
                    fetch_missing_base=fetch_missing_budget_base,
                )
            )

    probes = manifest.get("task_probes")
    if not isinstance(probes, list):
        issues.append((MANIFEST_PATH.as_posix(), "task_probes must be a list"))
        probes = []
    probe_ids = set()
    maximum_depth = budget.get("mandatory_task_probe_depth_max", 0)
    for probe in probes:
        if not isinstance(probe, dict):
            issues.append((MANIFEST_PATH.as_posix(), "each task probe must be an object"))
            continue
        probe_id = probe.get("id")
        if not isinstance(probe_id, str) or not probe_id:
            issues.append((MANIFEST_PATH.as_posix(), "task probe needs an id"))
            continue
        if probe_id in probe_ids:
            issues.append((MANIFEST_PATH.as_posix(), f"duplicate task probe {probe_id}"))
        probe_ids.add(probe_id)
        if not probe.get("first_route") or not probe.get("required_chain"):
            issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} needs first_route and required_chain"))
        if not isinstance(probe.get("negative_controls"), list) or not probe["negative_controls"]:
            issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} needs negative controls"))
        depth = probe.get("mandatory_reading_depth")
        if not isinstance(depth, int) or depth <= 0 or depth > maximum_depth:
            issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} exceeds mandatory reading depth budget"))
        for skill_id in probe.get("skill_ids", []):
            if skill_id not in declared:
                issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} names unknown skill {skill_id}"))
        port_id = probe.get("port_id")
        if port_id is not None and port_id not in ports:
            issues.append((MANIFEST_PATH.as_posix(), f"{probe_id} names unknown port {port_id}"))
    if probe_ids != EXPECTED_PROBES:
        issues.append((MANIFEST_PATH.as_posix(), f"task probes must be {sorted(EXPECTED_PROBES)}"))

    inventory = manifest.get("package_inventory")
    if not isinstance(inventory, dict):
        issues.append((MANIFEST_PATH.as_posix(), "package_inventory must be an object"))

    try:
        issues.extend(currentness_parity_issues(root))
        current = build_currentness(root)
        if isinstance(inventory, dict) and current["package_inventory"] != inventory:
            issues.append((MANIFEST_PATH.as_posix(), "package_inventory counts disagree with discovered packages"))
        current_ports = {record["id"] for record in current["owner_ports"]}
        if current_ports != set(ports):
            issues.append((CURRENTNESS_PATH.as_posix(), "owner-port currentness ids disagree with manifest"))
    except Exception as exc:
        issues.append((CURRENTNESS_PATH.as_posix(), f"cannot build currentness: {exc}"))

    issues.extend(public_safety_issues(root))
    return issues


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Validate the ToS model-facing skill and owner-port surface.")
    parser.add_argument("--check", action="store_true", help="validate the authored and generated surface")
    parser.add_argument(
        "--fetch-budget-bases",
        action="store_true",
        help="fetch each missing receipt-bound budget base commit before validation",
    )
    args = parser.parse_args(argv)
    if not args.check:
        parser.print_help()
        return 0
    issues = validate_manifest(
        REPO_ROOT,
        fetch_missing_budget_base=args.fetch_budget_bases,
    )
    if issues:
        print("Agent surface validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1
    print("[ok] validated ToS agent surface")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

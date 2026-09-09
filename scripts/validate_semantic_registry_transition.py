#!/usr/bin/env python3
"""Check registry evolution against an explicitly selected immutable Git baseline.

This owner change gate is not called by snapshot readers or runtime adapters.
It reads baseline JSON, never baseline Python, and performs no fetch or writes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

from jsonschema import Draft202012Validator
from jsonschema.exceptions import SchemaError
from referencing import Registry
from referencing.exceptions import Unresolvable


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "access" / "src"))
from tos_access.knowledge import validate_semantic_registries


BASELINE_ENV = "TOS_SEMANTIC_REGISTRY_BASELINE_COMMIT"
INTRODUCTION_ENV = "TOS_SEMANTIC_REGISTRY_ALLOW_INITIAL_INTRODUCTION"
REGISTRY_REFS = (
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
)
SCHEMA_REFS = (
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
)
DECLARED_READER_REF = "scripts/source_record_profiles.py"
MAX_JSON_BYTES = 1024 * 1024
BASELINE_HELP = (
    "choose the exact pre-change commit, ensure its object is available locally, "
    "then pass --baseline-commit FULL_COMMIT_OID or set "
    f"{BASELINE_ENV}=FULL_COMMIT_OID before "
    "python scripts/validation_lanes.py --run semantic_registry_transition "
    "(or python scripts/release_check.py)"
)


def _git(root: Path, *args: str) -> bytes:
    result = subprocess.run(
        ["git", "--no-replace-objects", "-C", str(root), *args],
        capture_output=True, check=False,
    )
    if result.returncode:
        raise ValueError(f"cannot read exact Git baseline: {result.stderr.decode('utf-8', 'replace').strip()}")
    return result.stdout


def _baseline(root: Path, commit: str | None) -> str:
    if not isinstance(commit, str) or not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", commit) or not int(commit, 16):
        raise ValueError(f"missing or invalid baseline: a nonzero full commit OID is required; {BASELINE_HELP}")
    if Path(_git(root, "rev-parse", "--show-toplevel").decode().strip()).resolve() != root:
        raise ValueError("repo root must be the exact Git worktree root")
    try:
        object_type = _git(root, "cat-file", "-t", commit).strip()
    except ValueError as exc:
        raise ValueError(f"{exc}; {BASELINE_HELP}") from exc
    if object_type != b"commit":
        raise ValueError(f"baseline {commit} is not a commit object; {BASELINE_HELP}")
    return commit


def _read(root: Path, ref: str, commit: str | None = None) -> bytes:
    if commit is not None:
        object_ref = f"{commit}:{ref}"
        if int(_git(root, "cat-file", "-s", object_ref)) > MAX_JSON_BYTES:
            raise ValueError(f"baseline {ref} exceeds the 1 MiB metadata limit")
        data = _git(root, "cat-file", "blob", object_ref)
    else:
        path = root / ref
        if any(part.is_symlink() for part in (path, *path.parents)):
            raise ValueError(f"current {ref} must not use a symlink")
        with path.open("rb") as stream:
            data = stream.read(MAX_JSON_BYTES + 1)
    if len(data) > MAX_JSON_BYTES:
        raise ValueError(f"{ref} exceeds the 1 MiB metadata limit")
    return data


def _json(data: bytes, label: str) -> dict[str, Any]:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result

    def nonfinite(value: str) -> None:
        raise ValueError(f"nonfinite JSON value {value}")

    try:
        value = json.loads(data.decode("utf-8"), object_pairs_hook=pairs, parse_constant=nonfinite)
        if not isinstance(value, dict):
            raise ValueError("JSON root must be an object")
        return value
    except (UnicodeError, ValueError) as exc:
        raise ValueError(f"{label}: {exc}") from exc


def _snapshot(root: Path, commit: str | None) -> tuple[list[dict[str, Any]], dict[str, str]]:
    label = f"baseline {commit}" if commit else "current working tree"
    records: list[dict[str, Any]] = []
    digests: dict[str, str] = {}
    for ref, schema_ref in zip(REGISTRY_REFS, SCHEMA_REFS):
        raw, schema_raw = _read(root, ref, commit), _read(root, schema_ref, commit)
        value, schema = _json(raw, f"{label}:{ref}"), _json(schema_raw, f"{label}:{schema_ref}")
        Draft202012Validator.check_schema(schema)
        # An empty explicit registry prevents implicit network schema retrieval.
        errors = Draft202012Validator(schema, registry=Registry()).iter_errors(value)
        error = next(errors, None)
        if error is not None:
            location = "/".join(str(part) for part in error.absolute_path) or "<root>"
            raise ValueError(f"{label}:{ref}:{location}: {error.message}")
        records.append(value)
        digests.update({ref: hashlib.sha256(raw).hexdigest(), schema_ref: hashlib.sha256(schema_raw).hexdigest()})
    return records, digests


def validate_transition(root: Path, baseline_commit: str | None, *, allow_initial_introduction: bool = False) -> dict[str, Any]:
    root = root.resolve()
    baseline_commit = _baseline(root, baseline_commit)
    required_refs = (*REGISTRY_REFS, *SCHEMA_REFS)
    present = set(_git(root, "ls-tree", "--name-only", baseline_commit, "--", *required_refs).decode().splitlines())
    introduction = not present
    if present and present != set(required_refs):
        raise ValueError("partial baseline registry/contract snapshot; absent objects cannot be treated as an initial introduction")
    previous_digests: dict[str, str] = {}
    if introduction:
        if not allow_initial_introduction:
            raise ValueError("baseline has no registries/contracts; initial introduction requires explicit "
                             f"--allow-initial-introduction or {INTRODUCTION_ENV}=1")
        if _git(root, "rev-parse", "--is-shallow-repository").strip() != b"false":
            raise ValueError("initial introduction requires complete local baseline ancestry, not a shallow history")
        graft_path = root / _git(root, "rev-parse", "--git-path", "info/grafts").decode().strip()
        if "GIT_GRAFT_FILE" in os.environ or graft_path.exists() or graft_path.is_symlink():
            raise ValueError("initial introduction requires unmodified baseline ancestry; Git grafts are not allowed")
        # A deleted prior registry/reader is not a new ontology introduction.
        history = _git(root, "rev-list", "--max-count=1", baseline_commit, "--", *required_refs, DECLARED_READER_REF)
        if history.strip():
            raise ValueError("baseline history already contains a registry, contract or declared-profile reader; initial introduction denied")
    else:
        previous, previous_digests = _snapshot(root, baseline_commit)
        previous_validation = validate_semantic_registries(*previous)
        if not previous_validation["valid"]:
            raise ValueError("baseline registry invariants failed: " + "; ".join(previous_validation["violations"]))
    current, current_digests = _snapshot(root, None)
    result = validate_semantic_registries(*current) if introduction else validate_semantic_registries(
        *current, previous_entity_registry=previous[0], previous_relation_registry=previous[1],
    )
    return {
        **result,
        "transition_kind": "initial-introduction" if introduction else "registry-evolution",
        "compared_previous_registry": not introduction,
        "initial_introduction_explicitly_allowed": introduction and allow_initial_introduction,
        "baseline_commit": baseline_commit,
        "baseline_sha256": previous_digests,
        "baseline_absent_refs": [*required_refs, DECLARED_READER_REF] if introduction else [],
        "current_source": "working-tree",
        "current_sha256": current_digests,
        "semantic_acceptance": False,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, epilog=BASELINE_HELP)
    parser.add_argument("--baseline-commit", help=f"exact full commit OID; otherwise explicitly set {BASELINE_ENV}")
    parser.add_argument("--allow-initial-introduction", action="store_true",
                        help=f"allow first-ever registry introduction only; alternatively set {INTRODUCTION_ENV}=1")
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT, help="exact Git worktree root")
    parser.add_argument("--json", action="store_true", help="print compared source digests and mechanical result")
    args = parser.parse_args(argv)
    try:
        introduction_env = os.environ.get(INTRODUCTION_ENV, "0")
        if introduction_env not in ("0", "1"):
            raise ValueError(f"{INTRODUCTION_ENV} must be exactly 0 or 1")
        result = validate_transition(
            args.repo_root, args.baseline_commit if args.baseline_commit is not None else os.environ.get(BASELINE_ENV),
            allow_initial_introduction=args.allow_initial_introduction or introduction_env == "1",
        )
    except (OSError, ValueError, RecursionError, SchemaError, Unresolvable) as exc:
        print(f"[error] semantic registry transition: {exc}", file=sys.stderr)
        return 1
    if args.json:
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    else:
        print(f"[{'ok' if result['valid'] else 'error'}] semantic registry {result['transition_kind']}: exact baseline {result['baseline_commit']}, current working-tree bytes")
        for violation in result["violations"]:
            print(f"- {violation}", file=sys.stderr)
    return 0 if result["valid"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

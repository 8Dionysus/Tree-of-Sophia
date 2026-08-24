#!/usr/bin/env python3
"""Prepare a Tree-owned witness for a future aoa-kag semantic pair.

This is deliberately a read-only, owner-local adapter.  It binds the current
Tree source/export/family surfaces and records what the existing builder and
local index can prove.  The current Tree builder materializes fields from the
public compatibility mirror; the canonical node is observed separately and
must remain in byte parity, but this adapter does not promote that observation
to an external producer-dependency claim.  It does not emit an aoa-kag receipt, semantic evidence
packet, admission decision, or any other downstream authority.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable, Mapping


REPO_ROOT = Path(__file__).resolve().parents[5]
CANONICAL_SOURCE_PATH = Path(
    "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json"
)
MIRROR_SOURCE_PATH = Path("ToS/public-compatibility/source_node.example.json")
BUILDER_PATH = Path("mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py")
VALIDATOR_PATH = Path("mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py")
FAMILY_VALIDATOR_PATH = Path("scripts/validate_local_kag_provider.py")
FAMILY_MANIFEST_PATH = Path("kag/indexes/index_family.manifest.json")

# These are the exact inputs named by the current Tree export builder.  The
# distinction between materialized and existence-only dependencies is part of
# the witness: existence checks are not semantic dependency coverage.
AUTHORITATIVE_SOURCE_PATHS = (CANONICAL_SOURCE_PATH,)
MATERIALIZED_SOURCE_PATHS = (MIRROR_SOURCE_PATH,)
EXISTENCE_ONLY_SOURCE_PATHS = (
    Path("ToS/public-compatibility/concept_node.example.json"),
    Path("ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md"),
    Path("ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md"),
)
DERIVED_EXPORT_PATHS = (
    Path("ToS/derived-exports/kag_export.json"),
    Path("ToS/derived-exports/kag_export.min.json"),
)

RELEVANT_ENVIRONMENT_KEYS = (
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "PYTHONHASHSEED",
    "PYTHONDONTWRITEBYTECODE",
    "PYTHONPATH",
    "PYTHONWARNINGS",
    "SOURCE_DATE_EPOCH",
    "TZ",
)
EXPECTED_SEAL_KEYS = (
    "head",
    "tree",
    "parent",
    "builder_sha256",
    "validator_sha256",
    "family_validator_sha256",
    "environment_sha256",
)
BLOCKED_EXTERNAL_EXIT = 3
REGULAR_GIT_MODES = {"100644", "100755"}


class PreparationError(RuntimeError):
    """Raised when the owner-only witness cannot be formed safely."""


def _git(repo_root: Path, *args: str) -> str:
    result = subprocess.run(
        ("git", *args),
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        raise PreparationError(
            f"git {' '.join(args)} failed with {result.returncode}: {result.stderr.strip()}"
        )
    return result.stdout.strip()


def _git_optional(repo_root: Path, *args: str) -> str | None:
    result = subprocess.run(
        ("git", *args),
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        return None
    return result.stdout.strip() or None


def _digest_object(value: object) -> str:
    payload = json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def _relative(path: Path, repo_root: Path) -> str:
    return path.relative_to(repo_root).as_posix()


def classify_dependency_paths(
    materialized: Iterable[str], existence_only: Iterable[str]
) -> dict[str, list[str]]:
    """Return stable dependency classes without making an authority claim."""

    materialized_paths = sorted(set(materialized))
    existence_only_paths = sorted(set(existence_only) - set(materialized_paths))
    return {
        "materialized": materialized_paths,
        "existence_only": existence_only_paths,
    }


def _same_stat(left: os.stat_result, right: os.stat_result) -> bool:
    return (
        left.st_dev,
        left.st_ino,
        left.st_size,
        left.st_mtime_ns,
        left.st_ctime_ns,
        left.st_mode,
    ) == (
        right.st_dev,
        right.st_ino,
        right.st_size,
        right.st_mtime_ns,
        right.st_ctime_ns,
        right.st_mode,
    )


def _assert_regular_file(path: Path, file_stat: os.stat_result, role: str) -> None:
    if stat.S_ISLNK(file_stat.st_mode):
        raise PreparationError(f"required {role} is a symlink, not an owner file: {path}")
    if not stat.S_ISREG(file_stat.st_mode):
        raise PreparationError(f"required {role} has unsupported file kind: {path}")


def _validate_relative_path(relative_path: Path, role: str) -> None:
    if relative_path.is_absolute() or ".." in relative_path.parts:
        raise PreparationError(f"{role} path must stay inside the candidate: {relative_path}")
    if not relative_path.parts:
        raise PreparationError(f"{role} path must name a file")


def _open_confined(repo_root: Path, relative_path: Path, role: str) -> int:
    """Open a tracked path through no-follow directory descriptors."""

    _validate_relative_path(relative_path, role)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    directory = getattr(os, "O_DIRECTORY", 0)
    if not nofollow or not directory:
        raise PreparationError(f"{role} requires no-follow directory-descriptor support")
    root = repo_root.resolve()
    try:
        root_fd = os.open(root, os.O_RDONLY | nofollow | directory)
    except OSError as exc:
        raise PreparationError(f"cannot open candidate root for {role}: {root}") from exc
    current_fd = root_fd
    try:
        for component in relative_path.parts[:-1]:
            next_fd = os.open(
                component,
                os.O_RDONLY | nofollow | directory,
                dir_fd=current_fd,
            )
            if current_fd != root_fd:
                os.close(current_fd)
            current_fd = next_fd
        return os.open(
            relative_path.parts[-1],
            os.O_RDONLY | nofollow,
            dir_fd=current_fd,
        )
    except OSError as exc:
        raise PreparationError(f"cannot open confined {role}: {relative_path}") from exc
    finally:
        if current_fd != root_fd:
            os.close(current_fd)
        os.close(root_fd)


def _fd_resolved_path(file_descriptor: int, repo_root: Path, relative_path: Path, role: str) -> Path:
    try:
        resolved = Path(os.readlink(f"/proc/self/fd/{file_descriptor}")).resolve(strict=True)
        expected = (repo_root / relative_path).resolve(strict=True)
        resolved.relative_to(repo_root.resolve())
    except (OSError, ValueError) as exc:
        raise PreparationError(f"opened {role} escaped the candidate: {relative_path}") from exc
    if resolved != expected:
        raise PreparationError(
            f"opened {role} does not name the expected candidate path: {relative_path}"
        )
    return resolved


def _read_confined_file(
    repo_root: Path, relative_path: Path, role: str
) -> tuple[bytes, os.stat_result, Path]:
    _validate_relative_path(relative_path, role)
    absolute_path = repo_root.resolve() / relative_path
    try:
        initial_stat = absolute_path.lstat()
    except OSError as exc:
        raise PreparationError(f"required {role} is missing: {relative_path.as_posix()}") from exc
    _assert_regular_file(absolute_path, initial_stat, role)

    file_descriptor = _open_confined(repo_root, relative_path, role)
    try:
        opened_stat = os.fstat(file_descriptor)
        _assert_regular_file(absolute_path, opened_stat, role)
        if not _same_stat(initial_stat, opened_stat):
            raise PreparationError(f"{role} was replaced while opening: {relative_path}")
        resolved_path = _fd_resolved_path(file_descriptor, repo_root, relative_path, role)
        with os.fdopen(file_descriptor, "rb") as handle:
            file_descriptor = -1
            content = handle.read()
            final_fd_stat = os.fstat(handle.fileno())
    except PreparationError:
        raise
    except OSError as exc:
        raise PreparationError(f"cannot read {role}: {relative_path}") from exc
    finally:
        if file_descriptor >= 0:
            os.close(file_descriptor)

    try:
        final_path_stat = absolute_path.lstat()
    except OSError as exc:
        raise PreparationError(f"{role} disappeared while being read: {relative_path}") from exc
    _assert_regular_file(absolute_path, final_path_stat, role)
    if not _same_stat(initial_stat, final_fd_stat) or not _same_stat(initial_stat, final_path_stat):
        raise PreparationError(f"{role} was replaced while being read: {relative_path}")
    return content, final_path_stat, resolved_path


def _git_hash_bytes(repo_root: Path, content: bytes, role: str) -> str:
    result = subprocess.run(
        ("git", "hash-object", "--stdin"),
        cwd=repo_root,
        input=content,
        capture_output=True,
        check=False,
        text=False,
    )
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise PreparationError(f"cannot hash read {role} bytes: {detail}")
    return result.stdout.decode("ascii", errors="strict").strip()


def _json(repo_root: Path, relative_path: Path) -> dict[str, Any]:
    try:
        content, _, _ = _read_confined_file(repo_root, relative_path, "JSON source")
        value = json.loads(content.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PreparationError(f"cannot read JSON {relative_path}: {exc}") from exc
    if not isinstance(value, dict):
        raise PreparationError(f"expected JSON object at {relative_path}")
    return value


def _file_identity(repo_root: Path, relative_path: Path, role: str) -> dict[str, Any]:
    _validate_relative_path(relative_path, role)
    content, final_path_stat, resolved_path = _read_confined_file(repo_root, relative_path, role)

    staged = _git(repo_root, "ls-files", "--stage", "--", relative_path.as_posix()).splitlines()
    if len(staged) != 1:
        raise PreparationError(f"required {role} is not one tracked candidate file: {relative_path}")
    staged_fields = staged[0].split(maxsplit=3)
    if len(staged_fields) != 4 or staged_fields[0] not in REGULAR_GIT_MODES:
        raise PreparationError(f"required {role} has unsupported Git file kind: {relative_path}")
    git_mode, staged_blob = staged_fields[0], staged_fields[1]

    head_blob = _git(repo_root, "rev-parse", f"HEAD:{relative_path.as_posix()}")
    git_blob = _git_hash_bytes(repo_root, content, role)
    if git_blob != head_blob or staged_blob != head_blob:
        raise PreparationError(
            f"{role} bytes/index are not bound to the exact HEAD candidate: {relative_path}"
        )

    return {
        "path": relative_path.as_posix(),
        "role": role,
        "bytes": final_path_stat.st_size,
        "sha256": hashlib.sha256(content).hexdigest(),
        "git_blob": git_blob,
        "head_blob": head_blob,
        "staged_blob": staged_blob,
        "git_mode": git_mode,
        "file_kind": "regular_file",
        "is_symlink": False,
        "resolved_path": resolved_path.relative_to(repo_root.resolve()).as_posix(),
        "device": final_path_stat.st_dev,
        "inode": final_path_stat.st_ino,
        "mtime_ns": final_path_stat.st_mtime_ns,
        "ctime_ns": final_path_stat.st_ctime_ns,
    }

def _assert_no_aliases(records: Iterable[dict[str, Any]]) -> None:
    seen: dict[tuple[int, int], str] = {}
    for record in records:
        key = (record["device"], record["inode"])
        previous = seen.get(key)
        if previous is not None and previous != record["path"]:
            raise PreparationError(
                "hard-link/alias provenance is not admissible for owner inputs: "
                f"{previous} and {record['path']} share one inode"
            )
        seen[key] = record["path"]


def _canonical_mirror_parity(
    canonical: dict[str, Any], mirror: dict[str, Any]
) -> dict[str, Any]:
    same_inode = (canonical["device"], canonical["inode"]) == (
        mirror["device"],
        mirror["inode"],
    )
    parity = {
        "sha256_equal": canonical["sha256"] == mirror["sha256"],
        "git_blob_equal": canonical["git_blob"] == mirror["git_blob"],
        "head_blob_equal": canonical["head_blob"] == mirror["head_blob"],
        "file_kind_equal": canonical["file_kind"] == mirror["file_kind"],
        "resolved_paths_distinct": canonical["resolved_path"] != mirror["resolved_path"],
        "same_inode": same_inode,
    }
    if not all(
        parity[key]
        for key in (
            "sha256_equal",
            "git_blob_equal",
            "head_blob_equal",
            "file_kind_equal",
            "resolved_paths_distinct",
        )
    ) or same_inode:
        raise PreparationError(
            "canonical source and public mirror failed observed parity or alias checks: "
            + json.dumps(parity, sort_keys=True)
        )
    parity["status"] = "observed_equal_regular_files_distinct_objects"
    return parity


def _repo_identity(repo_root: Path) -> dict[str, Any]:
    repo_root = repo_root.resolve()
    actual_root = Path(_git(repo_root, "rev-parse", "--show-toplevel")).resolve()
    if actual_root != repo_root:
        raise PreparationError(
            f"repo-root is not the isolated worktree root: {repo_root} != {actual_root}"
        )
    status = _git(repo_root, "status", "--porcelain=v1")
    if status:
        raise PreparationError(
            "Tree preparation requires a clean isolated worktree; refusing dirty state: "
            + status.splitlines()[0]
        )
    head = _git(repo_root, "rev-parse", "HEAD")
    parent = _git_optional(repo_root, "rev-parse", "HEAD^")
    changed_files = (
        _git(repo_root, "diff", "--name-only", f"{parent}..{head}").splitlines()
        if parent
        else _git(repo_root, "ls-files").splitlines()
    )
    return {
        "root": str(repo_root),
        "branch": _git(repo_root, "branch", "--show-current") or "(detached)",
        "head": head,
        "tree": _git(repo_root, "rev-parse", "HEAD^{tree}"),
        "parent": parent,
        "changed_files": changed_files,
        "status": "clean",
        "status_porcelain": "",
    }


def _family_identity(repo_root: Path) -> dict[str, Any]:
    manifest = _json(repo_root, FAMILY_MANIFEST_PATH)
    if manifest.get("schema_version") != "aoa-repo-local-kag-family-manifest-v3":
        raise PreparationError("unexpected local KAG family manifest schema")
    identity = manifest.get("family_identity")
    if not isinstance(identity, dict):
        raise PreparationError("local KAG family manifest has no family_identity object")
    required = ("content_digest", "source_snapshot")
    missing = [key for key in required if not identity.get(key)]
    if missing:
        raise PreparationError("local KAG family identity is incomplete: " + ", ".join(missing))
    compatibility_files = manifest.get("compatibility", {}).get("files", [])
    missing_compatibility_files = [
        item.get("path")
        for item in compatibility_files
        if isinstance(item, dict)
        and isinstance(item.get("path"), str)
        and not (repo_root / item["path"]).is_file()
    ]
    binding_state = "snapshot_only" if not missing_compatibility_files else "partial_snapshot"
    return {
        "manifest": _file_identity(repo_root, FAMILY_MANIFEST_PATH, "local_family_manifest"),
        "schema_version": manifest["schema_version"],
        "content_digest": identity["content_digest"],
        "source_snapshot": identity["source_snapshot"],
        "summary": manifest.get("summary", {}),
        "compatibility": manifest.get("compatibility", {}),
        "identity_binding": {
            "state": binding_state,
            "missing_compatibility_files": missing_compatibility_files,
            "producer_dependency_fields_present": all(
                key in manifest for key in ("producer_identity", "source_dependency")
            ),
            "candidate_fields_present": all(
                key in manifest for key in ("head", "tree", "parent")
            ),
            "claim_limit": (
                "family manifest is a source snapshot and does not bind the Tree producer, "
                "dependency seal, or exact candidate"
            ),
        },
    }


def _run_local_family_validator(repo_root: Path) -> dict[str, Any]:
    command = (sys.executable, str(repo_root / FAMILY_VALIDATOR_PATH))
    environment = dict(os.environ)
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    result = subprocess.run(
        command,
        cwd=repo_root,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
    )
    stdout_lines = result.stdout.strip().splitlines()
    stderr_lines = result.stderr.strip().splitlines()
    return {
        "command": [str(item) for item in command],
        "exit_code": result.returncode,
        "state": "current_validated" if result.returncode == 0 else "stale_or_invalid",
        "stdout_tail": stdout_lines[-3:],
        "stderr_tail": stderr_lines[-3:],
        "claim_limit": "local provider validation does not establish external aoa-kag admission",
    }

def _source_index_rows(
    repo_root: Path,
    paths: set[str],
    records_by_path: Mapping[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    """Find source-index witnesses with current bytes and bound shard identity."""

    source_dir = repo_root / "kag/indexes/shards/source"
    if source_dir.is_symlink() or not source_dir.is_dir():
        raise PreparationError(f"local source index is missing or not a real directory: {source_dir}")
    found: dict[str, dict[str, Any]] = {}
    for shard in sorted(source_dir.glob("*.jsonl")):
        shard_relative = shard.relative_to(repo_root)
        shard_identity = _file_identity(repo_root, shard_relative, "source_index_shard")
        shard_bytes, _, _ = _read_confined_file(repo_root, shard_relative, "source_index_shard")
        if hashlib.sha256(shard_bytes).hexdigest() != shard_identity["sha256"]:
            raise PreparationError(f"source-index shard changed during bounded read: {shard_relative}")
        try:
            lines = shard_bytes.decode("utf-8").splitlines()
        except UnicodeDecodeError as exc:
            raise PreparationError(f"source-index shard is not UTF-8: {shard_relative}") from exc
        for line_number, raw_line in enumerate(lines, start=1):
            try:
                row = json.loads(raw_line)
            except json.JSONDecodeError as exc:
                raise PreparationError(f"invalid JSON in {shard}:{line_number}: {exc}") from exc
            if not isinstance(row, dict):
                continue
            identity = row.get("identity")
            if not isinstance(identity, dict):
                continue
            row_path = identity.get("path")
            if row_path not in paths:
                continue
            if row_path in found:
                raise PreparationError(f"duplicate source-index row for selected path: {row_path}")
            if identity.get("repo") != "Tree-of-Sophia":
                raise PreparationError(f"source index selected a foreign repository row: {row_path}")
            current = records_by_path.get(row_path)
            if current is None:
                raise PreparationError(f"source index selected a foreign path: {row_path}")
            freshness = row.get("freshness")
            signs = row.get("signs")
            if not isinstance(signs, dict) or not isinstance(signs.get("digest"), str):
                raise PreparationError(f"source-index row has no required signs.digest: {row_path}")
            content_match = identity.get("content_hash") == current["sha256"]
            blob_match = identity.get("git_blob_id") == current["head_blob"]
            digest_match = signs["digest"] == current["sha256"]
            freshness_current = isinstance(freshness, dict) and freshness.get("state") == "current"
            if not (content_match and blob_match and digest_match and freshness_current):
                raise PreparationError(
                    "source-index row is stale for exact current candidate: "
                    f"{row_path} content={content_match} blob={blob_match} "
                    f"digest={digest_match} freshness={freshness_current}"
                )
            found[row_path] = {
                "path": row_path,
                "shard": shard_relative.as_posix(),
                "shard_identity": shard_identity,
                "line": line_number,
                "key": row.get("_key"),
                "content_hash": identity.get("content_hash"),
                "git_blob": identity.get("git_blob_id"),
                "git_ref": identity.get("git_ref"),
                "freshness": freshness,
                "verification_state": signs.get("verification_state"),
                "current_source_sha256": current["sha256"],
                "current_head_blob": current["head_blob"],
                "content_hash_matches": content_match,
                "git_blob_matches_head": blob_match,
                "digest_matches": digest_match,
                "freshness_is_current": freshness_current,
                "exact_candidate_binding": True,
                "shard_identity_bound": True,
            }
    missing = sorted(paths - set(found))
    if missing:
        raise PreparationError("local source-index rows are missing: " + ", ".join(missing))
    return [found[path] for path in sorted(found)]

def _stable_record(record: dict[str, Any]) -> dict[str, Any]:
    return {
        key: record[key]
        for key in (
            "path",
            "role",
            "bytes",
            "sha256",
            "git_blob",
            "head_blob",
            "staged_blob",
            "git_mode",
            "file_kind",
            "is_symlink",
            "resolved_path",
        )
    }


def _dependency_seal(
    classes: dict[str, list[str]],
    authoritative: list[dict[str, Any]],
    source_records: list[dict[str, Any]],
    export_records: list[dict[str, Any]],
    builder: dict[str, Any],
    validator: dict[str, Any],
    family_validator: dict[str, Any],
) -> str:
    return _digest_object(
        {
            "classes": classes,
            "authoritative": [_stable_record(record) for record in authoritative],
            "source": [_stable_record(record) for record in source_records],
            "exports": [_stable_record(record) for record in export_records],
            "producer": {
                "builder": _stable_record(builder),
                "validator": _stable_record(validator),
                "family_validator": _stable_record(family_validator),
            },
        }
    )


def _tool_identity(command: str) -> dict[str, Any]:
    executable = shutil.which(command)
    if not executable:
        raise PreparationError(f"required tool is not on PATH: {command}")
    path = Path(executable).resolve(strict=True)
    try:
        tool_stat = path.stat()
        if not stat.S_ISREG(tool_stat.st_mode):
            raise PreparationError(f"required tool is not a regular file: {path}")
        content = path.read_bytes()
    except OSError as exc:
        raise PreparationError(f"cannot bind required tool {command}: {path}") from exc
    return {
        "command": command,
        "path": str(path),
        "bytes": len(content),
        "sha256": hashlib.sha256(content).hexdigest(),
        "mode": stat.S_IMODE(tool_stat.st_mode),
        "version": subprocess.run(
            (str(path), "--version"),
            capture_output=True,
            check=False,
            text=True,
        ).stdout.splitlines()[:1],
    }


def _environment_identity() -> dict[str, Any]:
    binding = {
        "python": sys.version,
        "platform": platform.platform(),
        "executable": str(Path(sys.executable).resolve()),
        "variables": {
            key: os.environ.get(key)
            for key in (
                *RELEVANT_ENVIRONMENT_KEYS,
                "PATH",
                "GIT_CONFIG_NOSYSTEM",
                "GIT_CONFIG_GLOBAL",
                "GIT_CONFIG_SYSTEM",
                "GIT_SSH_COMMAND",
                "GIT_SSH",
                "GIT_EXEC_PATH",
            )
        },
        "git_variables": {
            key: value
            for key, value in sorted(os.environ.items())
            if key.startswith("GIT_")
        },
        "tools": {
            "git": _tool_identity("git"),
            "python": _tool_identity("python"),
        },
    }
    return {
        "binding_status": "sealed_local_toolchain_environment_observation",
        **binding,
        "sha256": _digest_object(binding),
    }

def _capture_observation(repo_root: Path, *, run_family_validator: bool = True) -> dict[str, Any]:
    repo = _repo_identity(repo_root)
    environment = _environment_identity()
    authoritative_records = [
        _file_identity(repo_root, path, "authored_source_authority")
        for path in AUTHORITATIVE_SOURCE_PATHS
    ]
    materialized_records = [
        _file_identity(repo_root, path, "authored_source_materialized")
        for path in MATERIALIZED_SOURCE_PATHS
    ]
    existence_records = [
        _file_identity(repo_root, path, "authored_source_existence_only")
        for path in EXISTENCE_ONLY_SOURCE_PATHS
    ]
    export_records = [
        _file_identity(repo_root, path, "derived_export") for path in DERIVED_EXPORT_PATHS
    ]
    family = _family_identity(repo_root)
    builder = _file_identity(repo_root, BUILDER_PATH, "Tree_export_builder")
    validator = _file_identity(repo_root, VALIDATOR_PATH, "Tree_export_validator")
    family_validator = _file_identity(repo_root, FAMILY_VALIDATOR_PATH, "local_family_validator")
    all_records = (
        authoritative_records
        + materialized_records
        + existence_records
        + export_records
        + [family["manifest"], builder, validator, family_validator]
    )
    _assert_no_aliases(all_records)
    records_by_path = {record["path"]: record for record in all_records}
    source_paths = {
        record["path"]
        for record in authoritative_records + materialized_records + existence_records + export_records
    }
    index_rows = _source_index_rows(repo_root, source_paths, records_by_path)
    dependency_classes = classify_dependency_paths(
        (record["path"] for record in materialized_records),
        (record["path"] for record in existence_records),
    )
    parity = _canonical_mirror_parity(authoritative_records[0], materialized_records[0])
    if run_family_validator:
        family["local_validator"] = _run_local_family_validator(repo_root)
    producer_dependency_seal = _dependency_seal(
        dependency_classes,
        authoritative_records,
        materialized_records + existence_records,
        export_records,
        builder,
        validator,
        family_validator,
    )
    return {
        "candidate": repo,
        "environment": environment,
        "authoritative_records": authoritative_records,
        "source_records": materialized_records + existence_records,
        "export_records": export_records,
        "family": family,
        "builder": builder,
        "validator": validator,
        "family_validator": family_validator,
        "dependency_classes": dependency_classes,
        "index_rows": index_rows,
        "parity": parity,
        "producer_dependency_seal": producer_dependency_seal,
    }


def _revalidation_projection(observation: dict[str, Any]) -> dict[str, Any]:
    family = {key: value for key, value in observation["family"].items() if key != "local_validator"}
    return {**observation, "family": family}


def build_index_fixed_point_reuse(repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    """Validate the bounded selected-index fixed point without regenerating outputs."""

    repo_root = repo_root.resolve()
    candidate = _repo_identity(repo_root)
    selected_paths = set(
        path.as_posix()
        for path in AUTHORITATIVE_SOURCE_PATHS
        + MATERIALIZED_SOURCE_PATHS
        + EXISTENCE_ONLY_SOURCE_PATHS
        + DERIVED_EXPORT_PATHS
    )
    records = [
        _file_identity(repo_root, Path(path), "selected_index_input") for path in sorted(selected_paths)
    ]
    family = _family_identity(repo_root)
    records.append(family["manifest"])
    _assert_no_aliases(records)
    records_by_path = {record["path"]: record for record in records}
    rows = _source_index_rows(repo_root, selected_paths, records_by_path)
    return {
        "method": "validated_index_fixed_point_reuse",
        "proof_class": "structural/source-index-currentness",
        "side_effect": "read_only",
        "candidate": candidate,
        "family": family,
        "rows": rows,
        "claim_limit": "selected source-index currentness only; no producer regeneration or semantic admission",
    }


def _validate_expected_seal(
    observation: dict[str, Any], expected: Mapping[str, str] | None
) -> dict[str, Any]:
    actual = {
        "head": observation["candidate"]["head"],
        "tree": observation["candidate"]["tree"],
        "parent": observation["candidate"]["parent"],
        "builder_sha256": observation["builder"]["sha256"],
        "validator_sha256": observation["validator"]["sha256"],
        "family_validator_sha256": observation["family_validator"]["sha256"],
        "environment_sha256": observation["environment"]["sha256"],
    }
    if not expected:
        return {
            "state": "observed_only",
            "missing_expected_fields": list(EXPECTED_SEAL_KEYS),
            "actual": actual,
        }
    missing = [key for key in EXPECTED_SEAL_KEYS if not expected.get(key)]
    if missing:
        raise PreparationError(
            "expected exact candidate/producer/environment seal is incomplete: "
            + ", ".join(missing)
        )
    mismatches = {
        key: {"expected": expected[key], "observed": actual[key]}
        for key in EXPECTED_SEAL_KEYS
        if expected[key] != actual[key]
    }
    if mismatches:
        raise PreparationError(
            "exact candidate/producer/environment seal mismatch: "
            + json.dumps(mismatches, sort_keys=True)
        )
    return {
        "state": "sealed",
        "expected": {key: expected[key] for key in EXPECTED_SEAL_KEYS},
        "actual": actual,
    }


def _assert_revalidated(before: dict[str, Any], after: dict[str, Any]) -> None:
    if before != after:
        raise PreparationError(
            "candidate, producer, source, family, environment, or index input changed "
            "during final TOCTOU revalidation"
        )


def _local_negative_evidence(family: dict[str, Any]) -> dict[str, Any]:
    manifest = family["identity_binding"]
    return {
        "state": "observed_negative",
        "regeneration_or_generated_currentness_sufficient": False,
        "reason": "local family/index surfaces do not bind external producer, source dependency, or semantic pair admission",
        "family_schema": family["schema_version"],
        "family_identity_binding": manifest,
        "local_pair_candidates": [],
    }


def _packet(observation: dict[str, Any], seal: dict[str, Any]) -> dict[str, Any]:
    family = observation["family"]
    local_family_validation = family["local_validator"]
    family_state = (
        "validated_current"
        if local_family_validation["exit_code"] == 0
        else "blocked_stale_or_invalid"
    )
    return {
        "schema_version": "tos-kag-owner-pair-preparation-v2",
        "status": "blocked_external_kag_contract",
        "authority": "Tree-of-Sophia owner preparation only",
        "claim_boundary": {
            "semantic_pair_emitted": False,
            "kag_receipt_emitted": False,
            "kag_admission_attempted": False,
            "owner_or_human_acceptance": False,
            "runtime_or_deployment_claim": False,
        },
        "candidate": {
            **observation["candidate"],
            "identity_binding": seal,
            "final_revalidation": "passed",
        },
        "environment": observation["environment"],
        "tree_source": {
            "authoritative_source_records": observation["authoritative_records"],
            "source_records": observation["source_records"],
            "derived_export_records": observation["export_records"],
            "family": family,
        },
        "producer": {
            "builder": observation["builder"],
            "validator": observation["validator"],
            "family_validator": observation["family_validator"],
            "dependency_seal": observation["producer_dependency_seal"],
            "procedure": {
                "tree_route": "mechanics/boundary-bridge/parts/derived-kag-seam",
                "external_owner": "aoa-kag",
                "external_contract_status": "required_but_not_available_and_not_loaded",
                "external_contract_identity": None,
            },
        },
        "dependency_witness": {
            "state": "tree_local_sealed" if seal["state"] == "sealed" else "partial",
            "classes": observation["dependency_classes"],
            "authority_anchor": [record["path"] for record in observation["authoritative_records"]],
            "source_index_rows": observation["index_rows"],
            "generated_paths": [record["path"] for record in observation["export_records"]],
            "unrelated_generated_paths": [],
            "source_to_export_binding": ("the current Tree builder materializes fields from the public compatibility mirror; this adapter separately requires observed byte/blob parity with the canonical authored node, treats the three other inputs as existence-only, and keeps direct relation targets as builder literals"),
            "builder_reads_authority_anchor": False,
            "canonical_mirror_parity": observation["parity"],
            "authority_mirror_parity": observation["parity"]["status"],
            "producer_dependency_seal": observation["producer_dependency_seal"],
            "selected_index_currentness": "validated_current",
            "family_currentness": family_state,
            "complete_for_external_semantic_pair": False,
        },
        "tree_currentness": {
            "selected_source_index": "validated_current",
            "family": family_state,
            "producer_dependency": "sealed" if seal["state"] == "sealed" else "observed_only",
            "claim_limit": "local Tree evidence does not establish external KAG semantic admission",
        },
        "negative_evidence": _local_negative_evidence(family),
        "external_kag": {
            "binding_status": "blocked_binding_unavailable",
            "callable_operations": [],
            "required_owner": "aoa-kag",
            "required_inputs": [
                "current admitted pair procedure and schema identity",
                "exact candidate/environment binding",
                "independent review outcome",
                "replay-safe admission receipt contract",
            ],
            "contract_identity": None,
            "fallback_used": False,
        },
    }


def build_owner_preparation(
    repo_root: Path = REPO_ROOT,
    expected: Mapping[str, str] | None = None,
) -> dict[str, Any]:
    """Build and final-revalidate a deterministic, candidate-only Tree packet."""

    repo_root = repo_root.resolve()
    before = _capture_observation(repo_root, run_family_validator=True)
    seal = _validate_expected_seal(before, expected)
    after = _capture_observation(repo_root, run_family_validator=False)
    _validate_expected_seal(after, expected)
    _assert_revalidated(_revalidation_projection(before), after)
    after["family"]["local_validator"] = before["family"]["local_validator"]
    return _packet(after, seal)


def _expected_from_args(args: argparse.Namespace) -> dict[str, str]:
    values = {
        "head": args.expected_head,
        "tree": args.expected_tree,
        "parent": args.expected_parent,
        "builder_sha256": args.expected_builder_sha256,
        "validator_sha256": args.expected_validator_sha256,
        "family_validator_sha256": args.expected_family_validator_sha256,
        "environment_sha256": args.expected_environment_sha256,
    }
    return {key: value for key, value in values.items() if value is not None}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--check", action="store_true", help="validate an explicit exact seal")
    parser.add_argument("--json", action="store_true", help="print the full preparation packet")
    parser.add_argument(
        "--index-reuse",
        action="store_true",
        help="validate the bounded selected-index fixed point without regeneration",
    )
    parser.add_argument("--expected-head")
    parser.add_argument("--expected-tree")
    parser.add_argument("--expected-parent")
    parser.add_argument("--expected-builder-sha256")
    parser.add_argument("--expected-validator-sha256")
    parser.add_argument("--expected-family-validator-sha256")
    parser.add_argument("--expected-environment-sha256")
    args = parser.parse_args(argv)
    if args.index_reuse:
        try:
            print(json.dumps(build_index_fixed_point_reuse(args.repo_root), ensure_ascii=False, sort_keys=True))
        except PreparationError as exc:
            print(f"owner-preparation: blocked: {exc}", file=sys.stderr)
            return 2
        return 0
    expected = _expected_from_args(args)
    if args.check:
        missing = [key for key in EXPECTED_SEAL_KEYS if key not in expected]
        if missing:
            print(
                "owner-preparation: blocked: --check requires an explicit exact "
                "candidate/producer/environment seal: " + ", ".join(missing),
                file=sys.stderr,
            )
            return 2
    try:
        packet = build_owner_preparation(args.repo_root, expected or None)
    except PreparationError as exc:
        print(f"owner-preparation: blocked: {exc}", file=sys.stderr)
        return 2
    if args.json or not args.check:
        print(json.dumps(packet, ensure_ascii=False, indent=2, sort_keys=True))
    if packet["status"] == "blocked_external_kag_contract":
        print(
            "owner-preparation: blocked: status=blocked_external_kag_contract "
            "(external aoa-kag contract is unavailable; no semantic pair or admission)",
            file=sys.stderr,
        )
        return BLOCKED_EXTERNAL_EXIT
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

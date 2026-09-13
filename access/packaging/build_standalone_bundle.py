#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import importlib
import json
import os
import stat
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import zipfile
from collections.abc import Mapping
from pathlib import Path
from typing import Any

FIXED_ZIP_TIME = (2020, 1, 1, 0, 0, 0)
BLOCKED_HOST_MARKERS = (b"/srv/" + b"AbyssOS", b"/srv/" + b"abyss-machine")
PARTITIONED_PROJECTION_ROOT_BYTES = 256 * 1024
QUERY_STORE_ARTIFACT_SCHEMA = "tos_query_store_ci_artifact_v1"
QUERY_STORE_COMPILER_PATHS = (
    "access/contracts/runtime-data.v1.json",
    "access/src/tos_access/__init__.py",
    "access/src/tos_access/core.py",
    "access/src/tos_access/disk_collections.py",
    "access/src/tos_access/exploration.py",
    "access/src/tos_access/knowledge.py",
    "access/src/tos_access/knowledge_compile.py",
    "access/src/tos_access/lens_pagination.py",
    "access/src/tos_access/normalization_cache.py",
    "access/src/tos_access/processing.py",
    "access/src/tos_access/projection_store.py",
    "access/src/tos_access/query_store.py",
)
COMPILED_SUBJECT_KEYS = frozenset(
    {
        "subject_id",
        "output_path",
        "builder_module",
        "required_when",
        "input_subject_ids",
        "consumer_roles",
        "identity_rule",
        "authority",
    }
)
SUPPORTED_COMPILED_SUBJECT_CONDITIONS = frozenset({"partitioned_projection_inputs"})
PARTITIONED_SUBJECT_POLICY_KEYS = frozenset(
    {"format", "inclusion", "discovery_glob_allowed", "source_authority"}
)
QUERY_STORE_ARTIFACT_KEYS = frozenset(
    {
        "schema_version",
        "artifact_kind",
        "artifact_name",
        "source_ref",
        "source_head",
        "source_dirty",
        "query_store_path",
        "query_store_filename",
        "query_store_sha256",
        "query_store_size_bytes",
        "schema",
        "compiler_version",
        "compiler_sha256",
        "compiler_paths",
        "input_bindings",
        "snapshot_bindings",
        "complete",
    }
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _regular_file_identity(path: Path, *, label: str) -> tuple[int, int, int, int]:
    """Return a stable identity for one owned regular file, rejecting links."""
    try:
        file_stat = path.stat()
        link_stat = path.lstat()
    except OSError as exc:
        raise RuntimeError(f"{label} is not readable: {path}") from exc
    if path.is_symlink() or not stat.S_ISREG(link_stat.st_mode):
        raise RuntimeError(f"{label} must be a regular file, not a symlink: {path}")
    return (file_stat.st_dev, file_stat.st_ino, file_stat.st_size, file_stat.st_mtime_ns)


def _read_query_store_metadata(path: Path, *, label: str = "query-store artifact") -> dict[str, Any]:
    """Read and integrity-check a completed SQLite snapshot without opening it writable."""
    before = _regular_file_identity(path, label=label)
    if any(Path(str(path) + suffix).exists() for suffix in ("-wal", "-journal")):
        raise RuntimeError(f"{label} has a mutable SQLite journal beside the snapshot: {path}")
    try:
        with sqlite3.connect(path.as_uri() + "?mode=ro&immutable=1", uri=True) as db:
            metadata = {
                key: json.loads(value)
                for key, value in db.execute("SELECT key,value FROM metadata")
            }
            integrity = db.execute("PRAGMA integrity_check").fetchone()
    except (OSError, sqlite3.Error, ValueError, TypeError) as exc:
        raise RuntimeError(f"{label} is not a readable SQLite snapshot: {exc}") from exc
    after = _regular_file_identity(path, label=label)
    if before != after:
        raise RuntimeError(f"{label} changed during integrity validation: {path}")
    if integrity != ("ok",):
        raise RuntimeError(f"{label} SQLite integrity check failed: {path}")
    return metadata


def _git_head(repo_root: Path) -> str | None:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    head = result.stdout.strip()
    return head if result.returncode == 0 and head else None


def _contract_relative_path(value: Any, *, field: str) -> str:
    """Validate one exact repository-relative path from a runtime contract."""
    if not isinstance(value, str) or not value:
        raise RuntimeError(f"compiled subject {field} must be a non-empty string")
    path = Path(value)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise RuntimeError(f"compiled subject {field} must be repository-relative: {value}")
    if any(marker in value for marker in ("*", "?", "[", "]")):
        raise RuntimeError(f"compiled subject {field} cannot contain a glob: {value}")
    return path.as_posix()


def _source_subject_index(allowlist: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    raw_subjects = allowlist.get("subjects", [])
    if not isinstance(raw_subjects, list):
        raise RuntimeError("runtime allowlist subjects must be a list")
    result: dict[str, Mapping[str, Any]] = {}
    for item in raw_subjects:
        if not isinstance(item, Mapping):
            raise RuntimeError("runtime allowlist subjects must be objects")
        subject_id = item.get("subject_id")
        if not isinstance(subject_id, str) or not subject_id:
            raise RuntimeError("runtime allowlist subjects must have non-empty subject_id strings")
        if subject_id in result:
            raise RuntimeError(f"runtime allowlist repeats subject_id: {subject_id}")
        _contract_relative_path(item.get("source_path"), field=f"{subject_id}.source_path")
        result[subject_id] = item
    return result


def _validate_compiled_subjects(allowlist: Mapping[str, Any]) -> list[dict[str, Any]]:
    """Validate the generated-output declarations owned by runtime-data.v1."""
    raw_specs = allowlist.get("compiled_subjects", [])
    if not isinstance(raw_specs, list):
        raise RuntimeError("runtime allowlist compiled_subjects must be a list")
    source_subjects = _source_subject_index(allowlist)
    result: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    seen_outputs: set[str] = set()
    source_ids = set(source_subjects)
    for raw in raw_specs:
        if not isinstance(raw, Mapping):
            raise RuntimeError("runtime allowlist compiled_subjects must contain objects")
        if set(raw) != COMPILED_SUBJECT_KEYS:
            raise RuntimeError(
                "compiled subject field shape drift: "
                f"expected {sorted(COMPILED_SUBJECT_KEYS)}, got {sorted(raw)}"
            )
        spec = dict(raw)
        subject_id = spec["subject_id"]
        if not isinstance(subject_id, str) or not subject_id:
            raise RuntimeError("compiled subject subject_id must be a non-empty string")
        if subject_id in seen_ids or subject_id in source_ids:
            raise RuntimeError(f"compiled subject id is duplicated or collides with a source subject: {subject_id}")
        seen_ids.add(subject_id)

        output_path = _contract_relative_path(spec["output_path"], field=f"{subject_id}.output_path")
        if output_path in seen_outputs or output_path in {
            item["source_path"] for item in source_subjects.values()
        }:
            raise RuntimeError(f"compiled subject output path is duplicated or source-owned: {output_path}")
        seen_outputs.add(output_path)
        spec["output_path"] = output_path

        builder_module = spec["builder_module"]
        if (
            not isinstance(builder_module, str)
            or not builder_module
            or any(not part.isidentifier() for part in builder_module.split("."))
        ):
            raise RuntimeError(f"compiled subject builder_module is invalid: {builder_module!r}")
        condition = spec["required_when"]
        if condition not in SUPPORTED_COMPILED_SUBJECT_CONDITIONS:
            raise RuntimeError(f"unsupported compiled subject required_when: {condition!r}")

        input_ids = spec["input_subject_ids"]
        if (
            not isinstance(input_ids, list)
            or not input_ids
            or any(not isinstance(item, str) or not item for item in input_ids)
            or len(set(input_ids)) != len(input_ids)
        ):
            raise RuntimeError(f"compiled subject input_subject_ids are invalid: {subject_id}")
        missing = sorted(set(input_ids) - source_ids)
        if missing:
            raise RuntimeError(f"compiled subject references unknown input subjects: {missing}")
        spec["input_subject_ids"] = list(input_ids)

        for field in ("consumer_roles",):
            values = spec[field]
            if (
                not isinstance(values, list)
                or not values
                or any(not isinstance(item, str) or not item for item in values)
                or len(set(values)) != len(values)
            ):
                raise RuntimeError(f"compiled subject {field} is invalid: {subject_id}")
            spec[field] = list(values)
        for field in ("identity_rule", "authority"):
            if not isinstance(spec[field], str) or not spec[field].strip():
                raise RuntimeError(f"compiled subject {field} is invalid: {subject_id}")
        result.append(spec)
    return result


def _validate_partitioned_subject_policy(allowlist: Mapping[str, Any]) -> None:
    policy = allowlist.get("partitioned_subject_policy")
    if not isinstance(policy, Mapping) or set(policy) != PARTITIONED_SUBJECT_POLICY_KEYS:
        raise RuntimeError(
            "partitioned projection inputs require an exact partitioned_subject_policy record"
        )
    if policy.get("format") != "tos_partitioned_projection_v1":
        raise RuntimeError("partitioned subject policy format is unsupported")
    if policy.get("inclusion") != "exact-verified-manifest-closure":
        raise RuntimeError("partitioned subject policy must require exact verified manifest closure")
    if policy.get("discovery_glob_allowed") is not False:
        raise RuntimeError("partitioned subject policy must disable discovery globs")
    if not isinstance(policy.get("source_authority"), str) or not policy["source_authority"].strip():
        raise RuntimeError("partitioned subject policy source_authority is invalid")


def _partitioned_subject_ids(repo_root: Path, allowlist: Mapping[str, Any]) -> set[str]:
    source_subjects = _source_subject_index(allowlist)
    result: set[str] = set()
    for subject_id, item in source_subjects.items():
        relative = _contract_relative_path(item["source_path"], field=f"{subject_id}.source_path")
        source = repo_root / relative
        if source.is_file() and _is_partitioned_projection(source):
            result.add(subject_id)
    return result


def _active_compiled_subject(
    repo_root: Path,
    allowlist: Mapping[str, Any],
) -> tuple[dict[str, Any] | None, set[str]]:
    specs = _validate_compiled_subjects(allowlist)
    partitioned_ids = _partitioned_subject_ids(repo_root, allowlist)
    if partitioned_ids:
        _validate_partitioned_subject_policy(allowlist)
    active = [
        spec
        for spec in specs
        if spec["required_when"] == "partitioned_projection_inputs"
        and partitioned_ids.intersection(spec["input_subject_ids"])
    ]
    if partitioned_ids and len(active) != 1:
        raise RuntimeError(
            "partitioned projection inputs require exactly one active compiled subject; "
            f"found {len(active)}"
        )
    return (active[0] if active else None), partitioned_ids


def _relative_path(repo_root: Path, value: str | Path) -> str:
    """Return one normalized repository relative path or reject traversal."""
    candidate = Path(value)
    if candidate.is_absolute():
        try:
            candidate = candidate.resolve().relative_to(repo_root.resolve())
        except ValueError as exc:
            raise RuntimeError(f"runtime subject escapes repository root: {value}") from exc
    normalized = candidate.as_posix()
    if not normalized or normalized == "." or normalized.startswith("../") or "/../" in normalized:
        raise RuntimeError(f"runtime subject is not repository-relative: {value}")
    if any(part in {"", ".", ".."} for part in Path(normalized).parts):
        raise RuntimeError(f"runtime subject has unsafe path components: {value}")
    return normalized


def _is_partitioned_projection(path: Path) -> bool:
    """Identify the versioned manifest without treating arbitrary JSON as one."""
    try:
        if path.is_symlink() or not path.is_file() or path.stat().st_size > PARTITIONED_PROJECTION_ROOT_BYTES:
            return False
        with path.open("rb") as stream:
            raw = stream.read(PARTITIONED_PROJECTION_ROOT_BYTES + 1)
        if len(raw) > PARTITIONED_PROJECTION_ROOT_BYTES:
            return False
        value = json.loads(raw.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        return False
    return isinstance(value, Mapping) and value.get("schema_version") == "tos_partitioned_projection_v1"


def _projection_closure(repo_root: Path, root: Path) -> tuple[list[Path], dict[str, Any]]:
    """Resolve an exact partition manifest closure through the owner reader.

    The projection reader is the source owner for manifest structure. Packaging
    only consumes its exact closure; it never expands a wildcard or walks a
    sibling directory. Hash and byte-size verification is performed by the
    reader and again here before copy/archive admission.
    """
    access_src = repo_root / "access" / "src"
    if access_src.as_posix() not in sys.path:
        sys.path.insert(0, access_src.as_posix())
    try:
        from tos_access.projection_store import ProjectionReader
    except ImportError as exc:
        raise RuntimeError("partitioned projection support is unavailable in this access source") from exc
    # No cache is retained here: closure traversal is the integrity pass and
    # every referenced data/index object must be checked against its declared
    # content address on disk.
    reader = ProjectionReader(root, cache_bytes=0)
    manifest = getattr(reader, "manifest", None)
    if not isinstance(manifest, Mapping):
        raise RuntimeError("partitioned projection reader did not expose its validated manifest")
    raw_paths = list(reader.closure_paths())
    reader.require_current()
    if not raw_paths:
        raise RuntimeError(f"partitioned projection closure is empty: {root}")
    root_resolved = root.resolve()
    closure: dict[str, Path] = {_relative_path(repo_root, root_resolved): root_resolved}
    for raw in raw_paths:
        path = Path(raw)
        if not path.is_absolute():
            path = root.parent / path
        path = path.resolve()
        relative = _relative_path(repo_root, path)
        if not path.is_file():
            raise RuntimeError(f"partitioned projection closure member is missing: {relative}")
        closure[relative] = path
    if _relative_path(repo_root, root_resolved) not in closure:
        raise RuntimeError("partitioned projection reader omitted its root manifest from closure")
    # The root is self-authenticated by the reader's exact bytes. Part
    # descriptors are carried by the validated manifest document, while the
    # public metadata method intentionally omits the growing collection tree.
    metadata = dict(manifest)
    return [closure[key] for key in sorted(closure)], metadata


def _file_identities(repo_root: Path, paths: list[Path]) -> dict[str, tuple[str, int]]:
    """Capture exact source bytes after the owner reader verifies a closure."""
    return {
        _relative_path(repo_root, path): (sha256_file(path), path.stat().st_size)
        for path in paths
    }


def _verify_projection_closure(
    repo_root: Path,
    root: Path,
) -> tuple[list[Path], dict[str, Any], dict[str, tuple[str, int]]]:
    closure, metadata = _projection_closure(repo_root, root)
    identities = _file_identities(repo_root, closure)
    return closure, metadata, identities


def _runtime_subject_paths(repo_root: Path, allowlist: Mapping[str, Any]) -> list[Path]:
    """Return the exact allowlisted files, expanding only owner manifests."""
    result: dict[str, Path] = {}
    for item in allowlist.get("subjects", []):
        if not isinstance(item, Mapping) or not isinstance(item.get("source_path"), str):
            raise RuntimeError("runtime allowlist subjects must have exact source_path strings")
        source_path = item["source_path"]
        if any(marker in source_path for marker in ("*", "?", "[", "]")):
            raise RuntimeError(f"runtime allowlist cannot contain glob patterns: {source_path}")
        relative = _relative_path(repo_root, source_path)
        source = repo_root / relative
        if not source.is_file():
            if item.get("required"):
                raise RuntimeError(f"missing required runtime subject: {relative}")
            continue
        if _is_partitioned_projection(source):
            closure, _, _ = _verify_projection_closure(repo_root, source)
            for member in closure:
                result[_relative_path(repo_root, member)] = member
        else:
            result[relative] = source
    return [result[key] for key in sorted(result)]


def _query_store_compiler_fingerprint(repo_root: Path) -> str:
    """Fingerprint compiler code and its transitive local query helpers.

    The generated SQLite file is deliberately absent from this digest. It is
    a checked output whose own hash is recorded in the bundle manifest; source
    identity remains bound to the exact inputs and compiler implementation.
    """
    digest = hashlib.sha256()
    for relative in QUERY_STORE_COMPILER_PATHS:
        path = repo_root / relative
        if not path.is_file():
            raise RuntimeError(f"missing query-store compiler input: {relative}")
        digest.update(relative.encode("utf-8") + b"\0")
        digest.update(sha256_file(path).encode("ascii") + b"\n")
    return digest.hexdigest()


def _query_store_input_bindings(
    repo_root: Path,
    allowlist: Mapping[str, Any],
    compiled_subject: Mapping[str, Any],
) -> dict[str, str]:
    """Bind the compiler to source roots selected by subject IDs."""
    source_subjects = _source_subject_index(allowlist)
    bindings: dict[str, str] = {}
    for subject_id in compiled_subject["input_subject_ids"]:
        item = source_subjects[subject_id]
        relative = _contract_relative_path(item["source_path"], field=f"{subject_id}.source_path")
        path = repo_root / relative
        if not path.is_file():
            raise RuntimeError(f"missing compiled query-store input: {subject_id} ({relative})")
        if relative in bindings:
            raise RuntimeError(f"compiled query-store input subjects share a source path: {relative}")
        bindings[relative] = sha256_file(path)
    return bindings


def _query_store_compiler_contract(
    repo_root: Path,
    allowlist: Mapping[str, Any],
    compiled_subject: Mapping[str, Any],
) -> tuple[str, str, dict[str, str], str]:
    """Resolve the compiler ABI and all source bindings for one compiled subject."""
    input_bindings = _query_store_input_bindings(repo_root, allowlist, compiled_subject)
    probe = """
import importlib
import json
import sys
from pathlib import Path

request = json.load(sys.stdin)
root = Path(request['root'])
sys.path.insert(0, str(root / 'access' / 'src'))
module = importlib.import_module(request['module'])
print(json.dumps({
    'inputs': getattr(module, 'INPUTS', None),
    'default_relative_path': str(getattr(module, 'DEFAULT_RELATIVE_PATH', '')),
    'schema': getattr(module, 'SCHEMA', None),
    'compiler_version': getattr(module, 'COMPILER_VERSION', None),
}))
"""
    result = subprocess.run(
        [sys.executable, "-I", "-c", probe],
        input=json.dumps(
            {"root": str(repo_root.resolve()), "module": str(compiled_subject["builder_module"])}
        ),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(f"standalone partitioned bundle requires the query-store compiler: {result.stderr}")
    try:
        compiler_contract = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError("query-store compiler contract probe returned invalid JSON") from exc
    compiler_inputs = compiler_contract.get("inputs")
    if not isinstance(compiler_inputs, Mapping) or not compiler_inputs:
        raise RuntimeError("compiled query-store builder module has no INPUTS mapping")
    if set(compiler_inputs.values()) != set(input_bindings):
        raise RuntimeError(
            "compiled query-store input subjects do not match the builder module INPUTS mapping"
        )
    output_path = _contract_relative_path(
        compiled_subject["output_path"], field=f"{compiled_subject['subject_id']}.output_path"
    )
    compiler_default = compiler_contract.get("default_relative_path")
    if not isinstance(compiler_default, str) or Path(compiler_default).as_posix() != output_path:
        raise RuntimeError(
            "compiled query-store output_path does not match the builder module default: "
            f"{output_path}"
        )
    schema = compiler_contract.get("schema")
    compiler_version = compiler_contract.get("compiler_version")
    if not isinstance(schema, str) or not isinstance(compiler_version, str):
        raise RuntimeError("compiled query-store builder module has no schema/compiler version")
    return schema, compiler_version, input_bindings, _query_store_compiler_fingerprint(repo_root)


def write_query_store_artifact_manifest(
    repo_root: Path,
    query_store: Path,
    output: Path,
    explicit_source_ref: str | None,
    artifact_name: str,
) -> dict[str, Any]:
    """Record a same-run, source-bound query-store handoff for CI only."""
    if not isinstance(artifact_name, str) or not artifact_name.strip():
        raise RuntimeError("query-store artifact name must be a non-empty string")
    if query_store.is_symlink():
        raise RuntimeError("query-store artifact handoff rejects a symlink producer")
    allowlist_path = repo_root / "access/contracts/runtime-data.v1.json"
    allowlist = json.loads(allowlist_path.read_text(encoding="utf-8"))
    compiled_subject, partitioned_ids = _active_compiled_subject(repo_root, allowlist)
    if not partitioned_ids or compiled_subject is None:
        raise RuntimeError("query-store artifact handoff requires an active compiled subject")
    resolved_ref, source_dirty = source_identity(repo_root, explicit_source_ref, allow_dirty=False)
    if source_dirty:
        raise RuntimeError("query-store artifact handoff refuses a dirty producer checkout")
    source_head = _git_head(repo_root) or resolved_ref
    if source_head != resolved_ref:
        raise RuntimeError(
            "query-store artifact handoff source_ref must equal the producer checkout HEAD: "
            f"{resolved_ref} != {source_head}"
        )
    relative = _relative_path(repo_root, query_store)
    expected_path = _contract_relative_path(
        compiled_subject["output_path"], field=f"{compiled_subject['subject_id']}.output_path"
    )
    if relative != expected_path:
        raise RuntimeError(
            "query-store artifact handoff path does not match the compiled subject: "
            f"{relative} != {expected_path}"
        )
    schema, compiler_version, input_bindings, compiler_sha256 = _query_store_compiler_contract(
        repo_root, allowlist, compiled_subject
    )
    metadata = _read_query_store_metadata(query_store, label="producer query-store")
    if metadata.get("schema") != schema or metadata.get("compiler_version") != compiler_version:
        raise RuntimeError("producer query-store has an unexpected schema/compiler version")
    if metadata.get("snapshot_bindings") != input_bindings or metadata.get("complete") is not True:
        raise RuntimeError("producer query-store is not bound to the declared source subjects")
    stat_identity = _regular_file_identity(query_store, label="producer query-store")
    query_store_sha256 = sha256_file(query_store)
    if stat_identity != _regular_file_identity(query_store, label="producer query-store"):
        raise RuntimeError("producer query-store changed while recording its handoff")
    manifest = {
        "schema_version": QUERY_STORE_ARTIFACT_SCHEMA,
        "artifact_kind": "same-run-ci-query-store-handoff",
        "artifact_name": artifact_name,
        "source_ref": resolved_ref,
        "source_head": source_head,
        "source_dirty": False,
        "query_store_path": expected_path,
        "query_store_filename": query_store.name,
        "query_store_sha256": query_store_sha256,
        "query_store_size_bytes": stat_identity[2],
        "schema": schema,
        "compiler_version": compiler_version,
        "compiler_sha256": compiler_sha256,
        "compiler_paths": list(QUERY_STORE_COMPILER_PATHS),
        "input_bindings": input_bindings,
        "snapshot_bindings": metadata["snapshot_bindings"],
        "complete": True,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return manifest


def _compile_query_store(
    repo_root: Path,
    output: Path,
    allowlist: Mapping[str, Any],
    compiled_subject: Mapping[str, Any],
    *,
    _isolated: bool = False,
) -> tuple[Path, dict[str, Any]]:
    """Compile one immutable standalone query snapshot from its contract record."""
    if not _isolated:
        # A caller may already have another checkout's tos_access in sys.modules.
        # A fresh interpreter binds execution to the requested compiler source.
        probe = """
import json, runpy, sys
from pathlib import Path
request = json.load(sys.stdin)
root = Path(request['root'])
sys.path.insert(0, str(root / 'access' / 'src'))
builder = runpy.run_path(request['builder'])
path, metadata = builder['_compile_query_store'](
    root, Path(request['output']), request['allowlist'], request['subject'], _isolated=True)
print(json.dumps({'path': str(path), 'metadata': metadata}))
"""
        result = subprocess.run(
            [sys.executable, "-I", "-c", probe],
            input=json.dumps({"root": str(repo_root.resolve()), "output": str(output.resolve()),
                              "builder": str(Path(__file__).resolve()),
                              "allowlist": dict(allowlist), "subject": dict(compiled_subject)}),
            text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        if result.returncode:
            raise RuntimeError(f"isolated query-store compilation failed: {result.stderr}")
        payload = json.loads(result.stdout)
        return Path(payload["path"]), payload["metadata"]
    compiler_digest_before = _query_store_compiler_fingerprint(repo_root)
    access_src = repo_root / "access" / "src"
    if access_src.as_posix() not in sys.path:
        sys.path.insert(0, access_src.as_posix())
    try:
        compiler_module = importlib.import_module(str(compiled_subject["builder_module"]))
    except ImportError as exc:
        raise RuntimeError("standalone partitioned bundle requires the query-store compiler") from exc
    compile_knowledge_store = getattr(compiler_module, "compile_knowledge_store", None)
    if not callable(compile_knowledge_store):
        raise RuntimeError(
            "compiled query-store builder module has no compile_knowledge_store callable: "
            f"{compiled_subject['builder_module']}"
        )
    compiler_inputs = getattr(compiler_module, "INPUTS", None)
    if not isinstance(compiler_inputs, Mapping) or not compiler_inputs:
        raise RuntimeError("compiled query-store builder module has no INPUTS mapping")
    input_bindings = _query_store_input_bindings(repo_root, allowlist, compiled_subject)
    if set(compiler_inputs.values()) != set(input_bindings):
        raise RuntimeError(
            "compiled query-store input subjects do not match the builder module INPUTS mapping"
        )
    compiler_default = getattr(compiler_module, "DEFAULT_RELATIVE_PATH", None)
    output_path = _contract_relative_path(
        compiled_subject["output_path"], field=f"{compiled_subject['subject_id']}.output_path"
    )
    if compiler_default is None or Path(compiler_default).as_posix() != output_path:
        raise RuntimeError(
            "compiled query-store output_path does not match the builder module default: "
            f"{output_path}"
        )
    schema = getattr(compiler_module, "SCHEMA", None)
    compiler_version = getattr(compiler_module, "COMPILER_VERSION", None)
    if not isinstance(schema, str) or not isinstance(compiler_version, str):
        raise RuntimeError("compiled query-store builder module has no schema/compiler version")

    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    result = compile_knowledge_store(repo_root, output, allow_legacy=True)
    candidate = output
    if isinstance(result, Mapping) and isinstance(result.get("output"), str):
        candidate = Path(result["output"]).resolve()
    elif isinstance(result, (str, Path)):
        candidate = Path(result).resolve()
    if candidate != output or not candidate.is_file():
        raise RuntimeError(f"query-store compiler did not publish the requested output: {output}")
    if any(Path(str(candidate) + suffix).exists() for suffix in ("-wal", "-journal")):
        raise RuntimeError("query-store compiler left a mutable SQLite journal beside the snapshot")
    try:
        with sqlite3.connect(candidate.as_uri() + "?mode=ro&immutable=1", uri=True) as db:
            metadata = {
                key: json.loads(value)
                for key, value in db.execute("SELECT key,value FROM metadata")
            }
    except (OSError, sqlite3.Error, ValueError, TypeError) as exc:
        raise RuntimeError(f"compiled query-store output is not readable: {exc}") from exc
    if metadata.get("schema") != schema or metadata.get("compiler_version") != compiler_version:
        raise RuntimeError("compiled query-store output has an unexpected schema/compiler version")
    if metadata.get("snapshot_bindings") != input_bindings or metadata.get("complete") is not True:
        raise RuntimeError("compiled query-store output is not bound to the declared source subjects")
    compiler_digest = _query_store_compiler_fingerprint(repo_root)
    if compiler_digest != compiler_digest_before:
        raise RuntimeError("query-store compiler changed during compilation")
    return candidate, {
        "schema": schema,
        "compiler_version": compiler_version,
        "builder_module": compiled_subject["builder_module"],
        "compiler_sha256": compiler_digest,
        "compiler_paths": list(QUERY_STORE_COMPILER_PATHS),
        "input_bindings": input_bindings,
    }


def _validate_prebuilt_query_store(
    repo_root: Path,
    query_store: Path,
    manifest_path: Path,
    artifact_name: str,
    resolved_source_ref: str,
    allowlist: Mapping[str, Any],
    compiled_subject: Mapping[str, Any],
) -> tuple[dict[str, Any], str, int]:
    """Validate a same-run handoff before it can enter the standalone stage."""
    if query_store.is_symlink() or manifest_path.is_symlink():
        raise RuntimeError("prebuilt query-store handoff rejects symlink inputs")
    try:
        handoff = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"prebuilt query-store handoff manifest is unreadable: {manifest_path}") from exc
    if not isinstance(handoff, Mapping) or set(handoff) != QUERY_STORE_ARTIFACT_KEYS:
        raise RuntimeError("prebuilt query-store handoff manifest has an unsupported field shape")
    if handoff["schema_version"] != QUERY_STORE_ARTIFACT_SCHEMA:
        raise RuntimeError("prebuilt query-store handoff manifest schema is unsupported")
    if handoff["artifact_kind"] != "same-run-ci-query-store-handoff":
        raise RuntimeError("prebuilt query-store handoff artifact kind is unsupported")
    if handoff["artifact_name"] != artifact_name:
        raise RuntimeError("prebuilt query-store handoff artifact name does not match the current run")
    if handoff["source_ref"] != resolved_source_ref or handoff["source_head"] != resolved_source_ref:
        raise RuntimeError("prebuilt query-store handoff source ref does not match the current checkout")
    if handoff["source_dirty"] is not False:
        raise RuntimeError("prebuilt query-store handoff was produced from a dirty checkout")
    expected_path = _contract_relative_path(
        compiled_subject["output_path"], field=f"{compiled_subject['subject_id']}.output_path"
    )
    if handoff["query_store_path"] != expected_path or handoff["query_store_filename"] != query_store.name:
        raise RuntimeError("prebuilt query-store handoff path does not match the current contract")
    if (
        not isinstance(handoff["query_store_sha256"], str)
        or len(handoff["query_store_sha256"]) != 64
        or any(character not in "0123456789abcdef" for character in handoff["query_store_sha256"])
        or not isinstance(handoff["query_store_size_bytes"], int)
        or handoff["query_store_size_bytes"] <= 0
    ):
        raise RuntimeError("prebuilt query-store handoff file identity is invalid")
    if not isinstance(handoff["compiler_paths"], list) or handoff["compiler_paths"] != list(QUERY_STORE_COMPILER_PATHS):
        raise RuntimeError("prebuilt query-store handoff compiler dependency set is stale")
    if (
        not isinstance(handoff["compiler_sha256"], str)
        or len(handoff["compiler_sha256"]) != 64
        or any(character not in "0123456789abcdef" for character in handoff["compiler_sha256"])
    ):
        raise RuntimeError("prebuilt query-store handoff compiler identity is invalid")
    schema, compiler_version, input_bindings, compiler_sha256 = _query_store_compiler_contract(
        repo_root, allowlist, compiled_subject
    )
    if handoff["schema"] != schema or handoff["compiler_version"] != compiler_version:
        raise RuntimeError("prebuilt query-store handoff schema/compiler version is stale")
    if handoff["compiler_sha256"] != compiler_sha256:
        raise RuntimeError("prebuilt query-store handoff compiler bytes are stale")
    if handoff["input_bindings"] != input_bindings or handoff["snapshot_bindings"] != input_bindings:
        raise RuntimeError("prebuilt query-store handoff source inputs are stale")
    if handoff["complete"] is not True:
        raise RuntimeError("prebuilt query-store handoff is incomplete")
    identity = _regular_file_identity(query_store, label="prebuilt query-store")
    if identity[2] != handoff["query_store_size_bytes"]:
        raise RuntimeError("prebuilt query-store handoff size does not match its manifest")
    metadata = _read_query_store_metadata(query_store, label="prebuilt query-store")
    if metadata.get("schema") != schema or metadata.get("compiler_version") != compiler_version:
        raise RuntimeError("prebuilt query-store has an unexpected schema/compiler version")
    if metadata.get("snapshot_bindings") != input_bindings or metadata.get("complete") is not True:
        raise RuntimeError("prebuilt query-store is stale or incomplete")
    return dict(metadata), handoff["query_store_sha256"], handoff["query_store_size_bytes"]


def _materialize_prebuilt_query_store(
    source: Path,
    target: Path,
    *,
    expected_sha256: str,
    expected_size: int,
    require_hardlink: bool,
) -> Path:
    """Materialize one verified input without a second full copy when CI permits linking."""
    source_before = _regular_file_identity(source, label="prebuilt query-store")
    if target.exists() or target.is_symlink():
        raise RuntimeError(f"refusing to overwrite staged query-store path: {target}")
    target.parent.mkdir(parents=True, exist_ok=True)
    try:
        if require_hardlink:
            os.link(source, target, follow_symlinks=False)
        else:
            before = source_before
            with source.open("rb") as source_stream, target.open("xb") as target_stream:
                opened = os.fstat(source_stream.fileno())
                if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns) != before:
                    raise RuntimeError("prebuilt query-store changed before materialization")
                digest = hashlib.sha256()
                copied = 0
                for chunk in iter(lambda: source_stream.read(1024 * 1024), b""):
                    digest.update(chunk)
                    target_stream.write(chunk)
                    copied += len(chunk)
                target_stream.flush()
                os.fsync(target_stream.fileno())
            after = _regular_file_identity(source, label="prebuilt query-store")
            if before != after:
                raise RuntimeError("prebuilt query-store changed during materialization")
            if copied != expected_size or digest.hexdigest() != expected_sha256:
                raise RuntimeError("prebuilt query-store bytes do not match the handoff manifest")
    except OSError as exc:
        if require_hardlink:
            raise RuntimeError(
                "prebuilt query-store requires same-filesystem hard-link materialization; "
                f"the runner cannot safely stage it without a second full copy: {exc}"
            ) from exc
        raise RuntimeError(f"unable to materialize prebuilt query-store: {exc}") from exc
    source_after = _regular_file_identity(source, label="prebuilt query-store")
    target_identity = _regular_file_identity(target, label="staged query-store")
    if source_after != source_before:
        raise RuntimeError("prebuilt query-store changed during hard-link materialization")
    if require_hardlink and target_identity[:3] != source_after[:3]:
        raise RuntimeError("staged query-store is not the verified same-filesystem hard link")
    target_sha256 = sha256_file(target)
    source_final = _regular_file_identity(source, label="prebuilt query-store")
    target_final = _regular_file_identity(target, label="staged query-store")
    if source_final != source_after or target_final != target_identity:
        raise RuntimeError("prebuilt query-store changed during staged byte validation")
    if target_final[2] != expected_size or target_sha256 != expected_sha256:
        raise RuntimeError("staged query-store bytes do not match the handoff manifest")
    return target


def source_identity(repo_root: Path, explicit: str | None, allow_dirty: bool) -> tuple[str, bool]:
    if explicit and not (repo_root / ".git").exists():
        return explicit, False
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    resolved = result.stdout.strip()
    if result.returncode or not resolved:
        if explicit:
            return explicit, False
        raise RuntimeError("--source-ref is required when building outside a Git checkout")
    status = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    dirty = bool(status.stdout.strip())
    if dirty and not allow_dirty:
        raise RuntimeError("refusing to identify a dirty source only by HEAD; pass --allow-dirty for a fingerprinted development candidate")
    return explicit or resolved, dirty


def source_fingerprint(repo_root: Path, allowlist: dict[str, Any]) -> str:
    digest = hashlib.sha256()
    compiled_subject, partitioned_ids = _active_compiled_subject(repo_root, allowlist)
    if partitioned_ids:
        # Keep the offline compiler in the identity boundary only when the
        # contract actually requires a generated read model. The SQLite output
        # itself remains a separately hashed manifest subject.
        if compiled_subject is None:  # guarded by _active_compiled_subject
            raise RuntimeError("partitioned projection inputs have no compiled subject")
        digest.update(b"compiled-subject\0")
        digest.update(compiled_subject["subject_id"].encode("utf-8") + b"\0")
        digest.update(compiled_subject["builder_module"].encode("utf-8") + b"\0")
        digest.update(_query_store_compiler_fingerprint(repo_root).encode("ascii") + b"\n")
    access_root = repo_root / "access"
    ignored_parts = {"node_modules", "__pycache__", ".pytest_cache", "runtime_data", "web_dist", "runtime", ".wrangler", "build"}
    paths = [
        path
        for path in access_root.rglob("*")
        if path.is_file() and not (set(path.relative_to(access_root).parts) & ignored_parts)
        and not any(part.endswith((".pyc", ".egg-info")) for part in path.relative_to(access_root).parts)
    ]
    paths.extend(_runtime_subject_paths(repo_root, allowlist))
    for path in sorted(set(paths), key=lambda item: item.relative_to(repo_root).as_posix()):
        relative = path.relative_to(repo_root).as_posix()
        digest.update(relative.encode("utf-8") + b"\0")
        digest.update(sha256_file(path).encode("ascii") + b"\n")
    return digest.hexdigest()


def _ignored(_: str, names: list[str]) -> set[str]:
    blocked = {"node_modules", "__pycache__", ".pytest_cache", "runtime_data", "web_dist", "runtime", ".wrangler", "build"}
    return {name for name in names if name in blocked or name.endswith((".pyc", ".egg-info"))}


def _scan_portable_code(access_root: Path) -> None:
    checked_suffixes = {".py", ".json", ".toml", ".ts", ".js", ".md", ".html"}
    for path in sorted(access_root.rglob("*")):
        if not path.is_file() or path.suffix not in checked_suffixes or "runtime_data" in path.parts:
            continue
        payload = path.read_bytes()
        for marker in BLOCKED_HOST_MARKERS:
            if marker in payload:
                raise RuntimeError(f"hard-coded host path in bundle source: {path.relative_to(access_root)}")


def _write_deterministic_zip(stage_root: Path, output: Path) -> None:
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(item for item in stage_root.rglob("*") if item.is_file()):
            relative = path.relative_to(stage_root).as_posix()
            info = zipfile.ZipInfo(relative, FIXED_ZIP_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (0o755 if path.name.endswith(".py") else 0o644) << 16
            info.file_size = path.stat().st_size
            info._compresslevel = 9
            with path.open("rb") as source, archive.open(info, "w", force_zip64=info.file_size >= zipfile.ZIP64_LIMIT) as target:
                shutil.copyfileobj(source, target, length=1024 * 1024)


def build_bundle(
    repo_root: Path,
    output: Path,
    explicit_source_ref: str | None = None,
    *,
    allow_dirty: bool = False,
    prebuilt_query_store: Path | None = None,
    prebuilt_query_store_manifest: Path | None = None,
    prebuilt_query_store_artifact_name: str | None = None,
    require_prebuilt_query_store_hardlink: bool = False,
    consume_prebuilt_query_store: bool = False,
) -> dict[str, Any]:
    prebuilt_values = (
        prebuilt_query_store,
        prebuilt_query_store_manifest,
        prebuilt_query_store_artifact_name,
    )
    if any(value is not None for value in prebuilt_values) and not all(
        value is not None for value in prebuilt_values
    ):
        raise RuntimeError(
            "prebuilt query-store reuse requires the store, manifest, and artifact name together"
        )
    if (require_prebuilt_query_store_hardlink or consume_prebuilt_query_store) and prebuilt_query_store is None:
        raise RuntimeError(
            "prebuilt query-store hard-link/consume options require a prebuilt query-store handoff"
        )
    access_root = repo_root / "access"
    allowlist_path = access_root / "contracts/runtime-data.v1.json"
    allowlist = json.loads(allowlist_path.read_text(encoding="utf-8"))
    resolved_ref, source_dirty = source_identity(repo_root, explicit_source_ref, allow_dirty)
    if prebuilt_query_store is not None:
        current_head = _git_head(repo_root)
        if current_head is not None and (source_dirty or current_head != resolved_ref):
            raise RuntimeError(
                "prebuilt query-store reuse requires a clean checkout at the declared source ref"
            )
    resolved_fingerprint = source_fingerprint(repo_root, allowlist)
    if output.exists():
        raise RuntimeError(f"refusing to overwrite existing bundle: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    prebuilt_query_store_path = prebuilt_query_store.absolute() if prebuilt_query_store else None
    prebuilt_query_store_manifest_path = (
        prebuilt_query_store_manifest.absolute() if prebuilt_query_store_manifest else None
    )
    prebuilt_query_store_to_consume: Path | None = None

    with tempfile.TemporaryDirectory(prefix="tos-standalone-build-", dir=output.parent) as raw_temp:
        stage = Path(raw_temp) / "tree-of-sophia-standalone"
        staged_access = stage / "access"
        shutil.copytree(access_root, staged_access, ignore=_ignored)
        runtime_data = staged_access / "src/tos_access/runtime_data"
        runtime_data.mkdir(parents=True)
        shutil.copytree(staged_access / "contracts", runtime_data / "access/contracts")
        shutil.copytree(staged_access / "profiles", runtime_data / "access/profiles")
        web_dist = staged_access / "web/dist"
        if not (web_dist / "assets/tos-graph.js").is_file():
            raise RuntimeError("web assets are missing; run npm --prefix access/web run build")
        shutil.copytree(web_dist, staged_access / "src/tos_access/web_dist")

        subjects: list[dict[str, Any]] = []
        for item in allowlist.get("subjects", []):
            source_path = Path(_relative_path(repo_root, str(item["source_path"])))
            source = repo_root / source_path
            if not source.is_file():
                if item.get("required"):
                    raise RuntimeError(f"missing required runtime subject: {source_path.as_posix()}")
                continue
            closure = [source]
            projection_metadata: dict[str, Any] | None = None
            projection_identities: dict[str, tuple[str, int]] | None = None
            if _is_partitioned_projection(source):
                closure, projection_metadata, projection_identities = _verify_projection_closure(repo_root, source)
            copied: list[dict[str, Any]] = []
            for member in closure:
                member_relative = Path(_relative_path(repo_root, member))
                target = runtime_data / member_relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(member, target)
                if projection_identities is not None:
                    expected = projection_identities[member_relative.as_posix()]
                    actual = (sha256_file(target), target.stat().st_size)
                    if actual != expected:
                        raise RuntimeError(
                            "partitioned projection member changed while packaging: "
                            f"{member_relative.as_posix()}"
                        )
                copied.append(
                    {
                        "source_path": member_relative.as_posix(),
                        "bundle_path": target.relative_to(stage).as_posix(),
                        "sha256": sha256_file(target),
                        "size_bytes": target.stat().st_size,
                    }
                )
            if projection_identities is not None:
                # Reopen and traverse the source after copying. This catches a
                # root/part replacement that happened between the first
                # verification pass and the last copied member.
                current_closure, current_metadata, current_identities = _verify_projection_closure(
                    repo_root, source
                )
                if (
                    [_relative_path(repo_root, path) for path in current_closure]
                    != [_relative_path(repo_root, path) for path in closure]
                    or current_metadata != projection_metadata
                    or current_identities != projection_identities
                ):
                    raise RuntimeError(
                        "partitioned projection snapshot changed while packaging: "
                        f"{source_path.as_posix()}"
                    )
            subject = {
                "subject_id": item["subject_id"],
                "source_path": source_path.as_posix(),
                "bundle_path": (runtime_data / source_path).relative_to(stage).as_posix(),
                "sha256": sha256_file(runtime_data / source_path),
                "size_bytes": (runtime_data / source_path).stat().st_size,
                "required": bool(item.get("required")),
            }
            if projection_metadata is not None:
                subject["projection_schema"] = projection_metadata.get("schema_version")
                subject["closure"] = copied
            subjects.append(subject)

        compiled_subject, partitioned_ids = _active_compiled_subject(repo_root, allowlist)
        if prebuilt_query_store is not None and not partitioned_ids:
            raise RuntimeError("prebuilt query-store handoff has no active compiled subject to consume")
        if partitioned_ids:
            if compiled_subject is None:  # guarded by _active_compiled_subject
                raise RuntimeError("partitioned projection inputs have no compiled subject")
            query_store_relative = Path(compiled_subject["output_path"])
            query_store_target = runtime_data / query_store_relative
            if prebuilt_query_store is None:
                query_store, query_store_metadata = _compile_query_store(
                    repo_root,
                    query_store_target,
                    allowlist,
                    compiled_subject,
                )
            else:
                handoff_metadata, expected_sha256, expected_size = _validate_prebuilt_query_store(
                    repo_root,
                    prebuilt_query_store_path,
                    prebuilt_query_store_manifest_path,
                    prebuilt_query_store_artifact_name,
                    resolved_ref,
                    allowlist,
                    compiled_subject,
                )
                query_store = _materialize_prebuilt_query_store(
                    prebuilt_query_store_path,
                    query_store_target,
                    expected_sha256=expected_sha256,
                    expected_size=expected_size,
                    require_hardlink=require_prebuilt_query_store_hardlink,
                )
                staged_metadata = _read_query_store_metadata(
                    query_store, label="staged prebuilt query-store"
                )
                if staged_metadata != handoff_metadata:
                    raise RuntimeError("staged prebuilt query-store metadata changed during materialization")
                schema, compiler_version, input_bindings, compiler_sha256 = _query_store_compiler_contract(
                    repo_root, allowlist, compiled_subject
                )
                query_store_metadata = {
                    "schema": schema,
                    "compiler_version": compiler_version,
                    "builder_module": compiled_subject["builder_module"],
                    "compiler_sha256": compiler_sha256,
                    "compiler_paths": list(QUERY_STORE_COMPILER_PATHS),
                    "input_bindings": input_bindings,
                }
            # The source checkout may change while the offline compiler runs.
            # Bind the compiled bytes to the inputs and code actually shipped,
            # not merely to a later snapshot observed in the live checkout.
            staged_bindings = _query_store_input_bindings(runtime_data, allowlist, compiled_subject)
            if query_store_metadata["input_bindings"] != staged_bindings:
                raise RuntimeError("compiled query-store inputs differ from staged bundle subjects")
            if query_store_metadata["compiler_sha256"] != _query_store_compiler_fingerprint(stage):
                raise RuntimeError("compiled query-store compiler differs from staged bundle code")
            generated_subject = {
                "subject_id": compiled_subject["subject_id"],
                "source_path": query_store_relative.as_posix(),
                "bundle_path": query_store.relative_to(stage).as_posix(),
                "sha256": sha256_file(query_store),
                "size_bytes": query_store.stat().st_size,
                "required": True,
                "generated": True,
                "compiler": query_store_metadata,
            }
            subjects.append(generated_subject)

            if prebuilt_query_store is not None and consume_prebuilt_query_store:
                prebuilt_query_store_to_consume = prebuilt_query_store_path

        manifest = {
            "schema_version": "tos_standalone_bundle_manifest_v1",
            "artifact_class": "unknown",
            "artifact_class_candidate": "portable_export",
            "artifact_policy_posture": "unknown_class_not_yet_admitted_by_abyss_machine",
            "artifact_id": "tree-of-sophia-standalone",
            "source_owner": "Tree-of-Sophia",
            "source_ref": resolved_ref,
            "source_dirty": source_dirty,
            "source_fingerprint": resolved_fingerprint,
            "source_fingerprint_scope": "access-source-plus-runtime-subject-closures-plus-query-compiler-v4-excludes-generated-query-store",
            "consumer_intent": "installer",
            "access_policy": "read-only-allowlisted-projections",
            "subjects": subjects,
            "controls": {
                "present": ["source-ref", "subject-sha256", "subject-size", "runtime-data-allowlist", "archive-smoke"],
                "deferred": ["signature", "sbom", "durable-registry", "consumer-trust-gate"],
            },
            "authority_limit": "A built bundle is a producer candidate, not publication, promotion, or consumer admission.",
        }
        rendered = json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
        (stage / "bundle.manifest.json").write_text(rendered, encoding="utf-8")
        (staged_access / "src/tos_access/bundle.manifest.json").write_text(rendered, encoding="utf-8")
        _scan_portable_code(staged_access)
        _write_deterministic_zip(stage, output)
        if prebuilt_query_store_to_consume is not None:
            if (
                prebuilt_query_store_to_consume.is_symlink()
                or not prebuilt_query_store_to_consume.is_file()
            ):
                raise RuntimeError("refusing to consume a missing or linked prebuilt query-store")
            prebuilt_query_store_to_consume.unlink()

    sidecar = output.with_suffix(output.suffix + ".manifest.json")
    sidecar_payload = {
        **manifest,
        "archive_sha256": sha256_file(output),
        "archive_size_bytes": output.stat().st_size,
    }
    sidecar.write_text(
        json.dumps(sidecar_payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return sidecar_payload


def main() -> None:
    parser = argparse.ArgumentParser(description="Build a portable Tree of Sophia standalone bundle")
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path)
    parser.add_argument("--source-ref")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--write-query-store-artifact-manifest", action="store_true")
    parser.add_argument("--query-store", type=Path)
    parser.add_argument("--query-store-manifest", type=Path)
    parser.add_argument("--artifact-name")
    parser.add_argument("--prebuilt-query-store", type=Path)
    parser.add_argument("--prebuilt-query-store-manifest", type=Path)
    parser.add_argument("--prebuilt-query-store-artifact-name")
    parser.add_argument("--require-prebuilt-query-store-hardlink", action="store_true")
    parser.add_argument("--consume-prebuilt-query-store", action="store_true")
    args = parser.parse_args()
    repo_root = args.repo_root.resolve()
    if args.write_query_store_artifact_manifest:
        if args.output is not None:
            parser.error("--output is not used with --write-query-store-artifact-manifest")
        if not all((args.query_store, args.query_store_manifest, args.artifact_name)):
            parser.error(
                "--write-query-store-artifact-manifest requires --query-store, "
                "--query-store-manifest, and --artifact-name"
            )
        manifest = write_query_store_artifact_manifest(
            repo_root,
            args.query_store.absolute(),
            args.query_store_manifest.absolute(),
            args.source_ref,
            args.artifact_name,
        )
        print(
            json.dumps(
                {
                    "ok": True,
                    "manifest": args.query_store_manifest.absolute().as_posix(),
                    "sha256": manifest["query_store_sha256"],
                    "size_bytes": manifest["query_store_size_bytes"],
                },
                indent=2,
            )
        )
        return
    if args.output is None:
        parser.error("--output is required when building a standalone bundle")
    manifest = build_bundle(
        repo_root,
        args.output.resolve(),
        args.source_ref,
        allow_dirty=args.allow_dirty,
        prebuilt_query_store=args.prebuilt_query_store,
        prebuilt_query_store_manifest=args.prebuilt_query_store_manifest,
        prebuilt_query_store_artifact_name=args.prebuilt_query_store_artifact_name,
        require_prebuilt_query_store_hardlink=args.require_prebuilt_query_store_hardlink,
        consume_prebuilt_query_store=args.consume_prebuilt_query_store,
    )
    print(
        json.dumps(
            {"ok": True, "bundle": args.output.resolve().as_posix(), "sha256": manifest["archive_sha256"]},
            indent=2,
        )
    )


if __name__ == "__main__":
    main()

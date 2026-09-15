#!/usr/bin/env python3
"""Portable validation for one Tree of Sophia data snapshot.

The verifier consumes only the snapshot tree and the installed query-store ABI.
It does not import build helpers, source checkouts, or release state.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import sqlite3
import stat
from pathlib import Path, PurePosixPath
from typing import Any

from .query_store import COMPILER_VERSION, SCHEMA


CHUNK_SIZE = 1024 * 1024
DATA_SNAPSHOT_SCHEMA = "tos_access_data_snapshot_v1"
QUERY_STORE_RELATIVE_PATH = "ToS/derived-exports/runtime/knowledge.sqlite3"
HEX64 = re.compile(r"[0-9a-f]{64}\Z")

MANIFEST_KEYS = frozenset(
    {
        "schema_version",
        "corpus_revision",
        "input_bindings",
        "compiler",
        "members",
        "data_revision",
    }
)
COMPILER_KEYS = frozenset(
    {
        "schema",
        "compiler_version",
        "compiler_sha256",
        "compiler_paths",
        "input_bindings",
    }
)
MEMBER_KEYS = frozenset({"path", "size_bytes", "sha256"})


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

def _canonical_bytes(value: Any) -> bytes:
    """Render one canonical JSON value with the repository newline rule."""
    try:
        rendered = json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError, UnicodeError) as exc:
        raise RuntimeError(f"cannot render canonical JSON: {exc}") from exc
    return (rendered + "\n").encode("utf-8")

def _strict_object(raw: bytes, *, label: str) -> dict[str, Any]:
    """Decode an object while rejecting duplicate keys at every level."""

    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            if key in result:
                raise RuntimeError(f"{label} contains duplicate JSON member {key!r}")
            result[key] = value
        return result

    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs)
    except RuntimeError:
        raise
    except (UnicodeError, ValueError, TypeError) as exc:
        raise RuntimeError(f"{label} is not valid UTF-8 JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise RuntimeError(f"{label} must contain a JSON object")
    return value

def _safe_relative(value: Any, *, label: str) -> str:
    """Validate one normalized repository-relative POSIX path."""
    if not isinstance(value, str) or not value:
        raise RuntimeError(f"{label} must be a non-empty string")
    if "\x00" in value or "\\" in value or value.startswith("/"):
        raise RuntimeError(f"{label} must be a safe relative path: {value!r}")
    if any(ord(character) < 0x20 for character in value):
        raise RuntimeError(f"{label} contains a control character: {value!r}")
    if any("\udc80" <= character <= "\udcff" for character in value):
        raise RuntimeError(f"{label} is not valid UTF-8: {value!r}")
    parts = value.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        raise RuntimeError(f"{label} must be a normalized relative path: {value!r}")
    try:
        normalized = PurePosixPath(value).as_posix()
    except (TypeError, ValueError) as exc:
        raise RuntimeError(f"{label} is not a valid POSIX path: {value!r}") from exc
    if normalized != value or PurePosixPath(value).is_absolute():
        raise RuntimeError(f"{label} is not normalized: {value!r}")
    return value

def _hex64(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or HEX64.fullmatch(value) is None:
        raise RuntimeError(f"{label} must be exactly 64 lowercase hexadecimal characters")
    return value

def _ensure_regular(path: Path, *, label: str) -> os.stat_result:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise RuntimeError(f"{label} is not readable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise RuntimeError(f"{label} must be a regular file: {path}")
    return metadata

def _ensure_directory(path: Path, *, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise RuntimeError(f"{label} is not readable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise RuntimeError(f"{label} must be a real directory: {path}")

def _relative_under(root: Path, path: str | Path, *, label: str) -> str:
    """Resolve an owner-returned path under a selected root safely."""
    root = Path(root).resolve()
    if not isinstance(path, (str, Path)):
        raise RuntimeError(f"{label} must be a path string")
    candidate = Path(path)
    if candidate.is_absolute():
        candidate = candidate.resolve()
    else:
        candidate = (root / candidate).resolve()
    try:
        relative = candidate.relative_to(root).as_posix()
    except ValueError as exc:
        raise RuntimeError(f"{label} escapes its selected root: {path}") from exc
    return _safe_relative(relative, label=label)

def _sha256_with_size(path: Path, *, label: str) -> tuple[str, int]:
    """Hash one stable regular file without materializing its bytes."""
    before = _ensure_regular(path, label=label)
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(CHUNK_SIZE), b""):
                digest.update(chunk)
                size += len(chunk)
    except OSError as exc:
        raise RuntimeError(f"{label} is not readable: {path}") from exc
    after = _ensure_regular(path, label=label)
    if (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
    ) != (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    ):
        raise RuntimeError(f"{label} changed while hashing: {path}")
    if size != before.st_size:
        raise RuntimeError(f"{label} size changed while hashing: {path}")
    return digest.hexdigest(), size

def _walk_tree(root: Path, *, label: str) -> tuple[dict[str, Path], set[str]]:
    """Return regular files and directories while rejecting links/specials."""
    _ensure_directory(root, label=label)
    files: dict[str, Path] = {}
    directories: set[str] = set()

    def visit(directory: Path, relative_directory: str) -> None:
        try:
            children = sorted(directory.iterdir(), key=lambda item: item.name)
        except OSError as exc:
            raise RuntimeError(f"{label} cannot be listed: {directory}") from exc
        for child in children:
            relative = f"{relative_directory}/{child.name}" if relative_directory else child.name
            _safe_relative(relative, label=f"{label} path")
            try:
                metadata = child.lstat()
            except OSError as exc:
                raise RuntimeError(f"{label} entry is not readable: {child}") from exc
            if stat.S_ISLNK(metadata.st_mode):
                raise RuntimeError(f"{label} contains a symlink: {relative}")
            if stat.S_ISDIR(metadata.st_mode):
                directories.add(relative)
                visit(child, relative)
            elif stat.S_ISREG(metadata.st_mode):
                files[relative] = child
            else:
                raise RuntimeError(f"{label} contains a non-regular entry: {relative}")

    visit(root, "")
    return files, directories

def _validate_compiler_manifest(compiler: Any) -> dict[str, Any]:
    if not isinstance(compiler, dict) or set(compiler) != COMPILER_KEYS:
        raise RuntimeError("data snapshot compiler manifest has an unexpected field set")
    schema = compiler.get("schema")
    version = compiler.get("compiler_version")
    if not isinstance(schema, str) or not schema:
        raise RuntimeError("data snapshot compiler schema is invalid")
    if not isinstance(version, str) or not version:
        raise RuntimeError("data snapshot compiler version is invalid")
    compiler_sha = _hex64(compiler.get("compiler_sha256"), label="compiler_sha256")
    paths = compiler.get("compiler_paths")
    if not isinstance(paths, list) or not paths:
        raise RuntimeError("data snapshot compiler_paths are invalid")
    normalized_paths = [
        _safe_relative(path, label="compiler path") for path in paths
    ]
    if normalized_paths != paths or len(set(normalized_paths)) != len(normalized_paths):
        raise RuntimeError("data snapshot compiler_paths are not sorted and unique")
    bindings = compiler.get("input_bindings")
    if not isinstance(bindings, dict) or not bindings:
        raise RuntimeError("data snapshot compiler input_bindings are invalid")
    normalized_bindings: dict[str, str] = {}
    for relative, digest in bindings.items():
        key = _safe_relative(relative, label="compiler input path")
        normalized_bindings[key] = _hex64(
            digest, label=f"compiler input {key} digest"
        )
    if list(normalized_bindings) != sorted(normalized_bindings):
        raise RuntimeError("data snapshot compiler input_bindings are not sorted")
    return {
        "schema": schema,
        "compiler_version": version,
        "compiler_sha256": compiler_sha,
        "compiler_paths": normalized_paths,
        "input_bindings": normalized_bindings,
    }

def _validate_member_list(members: Any) -> list[dict[str, Any]]:
    if not isinstance(members, list) or not members:
        raise RuntimeError("data snapshot members must be a non-empty list")
    result: list[dict[str, Any]] = []
    previous: str | None = None
    for index, raw in enumerate(members, start=1):
        if not isinstance(raw, dict) or set(raw) != MEMBER_KEYS:
            raise RuntimeError(f"data snapshot member {index} has an unexpected field set")
        path = _safe_relative(raw.get("path"), label=f"data snapshot member {index} path")
        if not path.startswith("data/") or path == "data/":
            raise RuntimeError(f"data snapshot member {index} is outside data/: {path}")
        _safe_relative(path[5:], label=f"data snapshot member {index} relative path")
        if previous is not None and path <= previous:
            raise RuntimeError("data snapshot members are not sorted and unique")
        previous = path
        size = raw.get("size_bytes")
        if isinstance(size, bool) or not isinstance(size, int) or size < 0:
            raise RuntimeError(f"data snapshot member {index} size_bytes is invalid")
        digest = _hex64(raw.get("sha256"), label=f"data snapshot member {index} sha256")
        result.append({"path": path, "size_bytes": size, "sha256": digest})
    return result

def verify_data_snapshot(
    root: Path,
    *,
    require_compatible: bool = True,
) -> dict[str, Any]:
    """Verify a complete data snapshot without extracting or compiling it."""
    if not isinstance(require_compatible, bool):
        raise RuntimeError("require_compatible must be a boolean")
    root_input = Path(root)
    _ensure_directory(root_input, label="data snapshot root")
    root = root_input.resolve()
    try:
        names = {entry.name for entry in root.iterdir()}
    except OSError as exc:
        raise RuntimeError(f"data snapshot root cannot be listed: {root}") from exc
    if names != {"data", "manifest.json"}:
        raise RuntimeError("data snapshot root must contain exactly data and manifest.json")

    manifest_path = root / "manifest.json"
    _ensure_regular(manifest_path, label="data snapshot manifest")
    try:
        raw_manifest = manifest_path.read_bytes()
    except OSError as exc:
        raise RuntimeError(f"data snapshot manifest is not readable: {manifest_path}") from exc
    manifest = _strict_object(raw_manifest, label="data snapshot manifest")
    if _canonical_bytes(manifest) != raw_manifest:
        raise RuntimeError("data snapshot manifest is not canonical JSON")
    if set(manifest) != MANIFEST_KEYS:
        raise RuntimeError("data snapshot manifest has an unexpected field set")
    if manifest.get("schema_version") != DATA_SNAPSHOT_SCHEMA:
        raise RuntimeError("data snapshot manifest has an unsupported schema version")
    _hex64(manifest.get("corpus_revision"), label="corpus_revision")
    _hex64(manifest.get("data_revision"), label="data_revision")

    input_bindings = manifest.get("input_bindings")
    if not isinstance(input_bindings, dict) or not input_bindings:
        raise RuntimeError("data snapshot input_bindings are invalid")
    normalized_input_bindings: dict[str, str] = {}
    for relative, digest in input_bindings.items():
        key = _safe_relative(relative, label="data snapshot input path")
        normalized_input_bindings[key] = _hex64(
            digest, label=f"data snapshot input {key} digest"
        )
    if list(normalized_input_bindings) != sorted(normalized_input_bindings):
        raise RuntimeError("data snapshot input_bindings are not sorted")

    compiler = _validate_compiler_manifest(manifest.get("compiler"))
    members = _validate_member_list(manifest.get("members"))

    # Compatibility is checked before opening SQLite.  A caller that needs to
    # inspect an older/newer ABI can explicitly request integrity-only mode;
    # the database metadata is still checked against the manifest below.
    if require_compatible and (
        compiler["schema"] != SCHEMA
        or compiler["compiler_version"] != COMPILER_VERSION
    ):
        raise RuntimeError("data snapshot query-store ABI is incompatible")

    body = {key: manifest[key] for key in MANIFEST_KEYS if key != "data_revision"}
    expected_revision = hashlib.sha256(_canonical_bytes(body)).hexdigest()
    if manifest["data_revision"] != expected_revision:
        raise RuntimeError("data snapshot data_revision does not match its manifest body")

    data_root = root / "data"
    actual_files, actual_directories = _walk_tree(data_root, label="data snapshot data")
    expected_member_paths = {member["path"] for member in members}
    actual_member_paths = {f"data/{relative}" for relative in actual_files}
    if actual_member_paths != expected_member_paths:
        raise RuntimeError("data snapshot files differ from its declared members")
    expected_directories: set[str] = set()
    for path in expected_member_paths:
        parts = path.split("/")[1:]
        for index in range(1, len(parts)):
            expected_directories.add("/".join(parts[:index]))
    if actual_directories != expected_directories:
        raise RuntimeError("data snapshot data directories differ from its declared members")

    member_by_path = {member["path"]: member for member in members}
    for path in sorted(actual_files):
        member = member_by_path[f"data/{path}"]
        digest, size = _sha256_with_size(
            actual_files[path], label=f"data snapshot member {path}"
        )
        if size != member["size_bytes"] or digest != member["sha256"]:
            raise RuntimeError(f"data snapshot member integrity mismatch: {path}")

    source_paths = set(input_bindings)
    expected_source_members = {f"data/{path}" for path in source_paths}
    query_member_path = f"data/{QUERY_STORE_RELATIVE_PATH}"
    if query_member_path not in expected_member_paths:
        raise RuntimeError("data snapshot compiled query-store member is missing")
    if expected_member_paths - {query_member_path} != expected_source_members:
        raise RuntimeError("data snapshot members do not match input_bindings")
    for relative, digest in input_bindings.items():
        member = member_by_path[f"data/{relative}"]
        if member["sha256"] != digest:
            raise RuntimeError(f"data snapshot input binding differs from member: {relative}")
    for relative, digest in compiler["input_bindings"].items():
        if input_bindings.get(relative) != digest:
            raise RuntimeError(f"compiler input binding is not bound to snapshot input: {relative}")

    metadata = _read_query_store_metadata(
        root / "data" / QUERY_STORE_RELATIVE_PATH,
        label="data snapshot query-store",
    )
    if metadata.get("schema") != compiler["schema"]:
        raise RuntimeError("data snapshot query-store schema differs from compiler manifest")
    if metadata.get("compiler_version") != compiler["compiler_version"]:
        raise RuntimeError("data snapshot query-store compiler version differs from compiler manifest")
    if metadata.get("snapshot_bindings") != compiler["input_bindings"]:
        raise RuntimeError("data snapshot query-store input bindings differ from compiler manifest")
    if metadata.get("complete") is not True:
        raise RuntimeError("data snapshot query-store is incomplete")
    return manifest

__all__ = ["verify_data_snapshot"]

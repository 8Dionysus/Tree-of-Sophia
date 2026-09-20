#!/usr/bin/env python3
"""Publish one verified, local KAG consumer result for a corpus revision.

The command owns only the bounded local publication contour.  It builds the
source export with the ToS exporter, gives a private copy to the explicitly
selected KAG consumer, validates the consumer's source return, and publishes
the resulting files as one immutable release.  It never imports or discovers
an external KAG checkout implicitly.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any, Iterable


_SCRIPTS = Path(__file__).resolve().parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

from build_kag_export import PRIMARY, build_export, verify_export  # noqa: E402
from corpus_store import (  # noqa: E402
    CorpusStoreError,
    _rename_new,
    _sync_dir,
    canonical,
    digest_file,
    hex_digest,
)
from downstream_status import DownstreamStatus, DownstreamStatusError  # noqa: E402


SCHEMA = "tos_kag_integration_v1"
PROGRAM_PATHS = tuple(sorted((
    "scripts/build_repo_local_kag_release.py",
    "scripts/query_repo_local_kag.py",
    "scripts/validators/local_kag_subtree.py",
    "scripts/validators/repo_local_kag_index.py",
)))
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
MAX_ERROR_CHARS = 4096


def _error(message: str) -> CorpusStoreError:
    return CorpusStoreError(message)


def _safe_absolute(value: Path | str, *, label: str) -> Path:
    try:
        path = Path(value).expanduser()
        if any(part in {".", ".."} for part in path.parts):
            raise _error(f"{label} contains traversal segments")
        if not path.is_absolute():
            path = Path.cwd() / path
        path = path.absolute()
        if "\x00" in str(path):
            raise _error(f"{label} contains an unsafe path")
    except CorpusStoreError:
        raise
    except (OSError, TypeError, ValueError) as exc:
        raise _error(f"{label} is not a safe path") from exc

    current = Path(path.anchor)
    for component in path.parts[1:]:
        current /= component
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            break
        except OSError as exc:
            raise _error(f"cannot inspect {label}: {current}") from exc
        if stat.S_ISLNK(metadata.st_mode):
            raise _error(f"{label} may not contain symlinks: {current}")
    return path


def _ensure_directory(path: Path, *, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise _error(f"{label} is unavailable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode):
        raise _error(f"{label} may not be a symlink: {path}")
    if not stat.S_ISDIR(metadata.st_mode):
        raise _error(f"{label} must be a directory: {path}")


def _ensure_regular(path: Path, *, label: str) -> os.stat_result:
    try:
        if path.absolute() != path.resolve():
            raise _error(f"{label} may not have symlink ancestors: {path}")
    except CorpusStoreError:
        raise
    except OSError as exc:
        raise _error(f"cannot inspect {label}: {path}") from exc
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise _error(f"{label} is unavailable: {path}") from exc
    if stat.S_ISLNK(metadata.st_mode):
        raise _error(f"{label} may not be a symlink: {path}")
    if not stat.S_ISREG(metadata.st_mode):
        raise _error(f"{label} must be a regular file: {path}")
    return metadata


def _prepare_release_root(value: Path | str) -> Path:
    root = _safe_absolute(value, label="release root")
    try:
        root.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        raise _error(f"cannot create release root: {root}") from exc
    _safe_absolute(root, label="release root")
    _ensure_directory(root, label="release root")
    releases = root / "releases"
    try:
        releases.mkdir(exist_ok=True)
    except OSError as exc:
        raise _error(f"cannot create release directory: {releases}") from exc
    _safe_absolute(releases, label="release directory")
    _ensure_directory(releases, label="release directory")
    return root


def _selected_kag_root(value: Path | str) -> Path:
    root = _safe_absolute(value, label="selected KAG root")
    _ensure_directory(root, label="selected KAG root")
    for relative in PROGRAM_PATHS:
        _ensure_regular(root / relative, label=f"selected KAG program {relative}")
    return root


def _safe_relative(value: Any, *, label: str) -> str:
    if not isinstance(value, str) or not value or "\\" in value or "\x00" in value:
        raise _error(f"{label} is not a normalized relative path")
    if value.startswith("/") or any(ord(character) < 32 for character in value):
        raise _error(f"{label} is not a normalized relative path")
    path = Path(value)
    if path.is_absolute() or path.as_posix() != value:
        raise _error(f"{label} is not a normalized relative path")
    if any(part in {".", "..", ".git"} for part in path.parts):
        raise _error(f"{label} is not a normalized relative path")
    return value


def _iter_regular_files(root: Path) -> list[tuple[str, Path]]:
    _ensure_directory(root, label="artifact root")
    files: list[tuple[str, Path]] = []
    try:
        for current, dirnames, filenames in os.walk(root, topdown=True, followlinks=False):
            current_path = Path(current)
            checked_dirs: list[str] = []
            for name in sorted(dirnames):
                child = current_path / name
                metadata = child.lstat()
                if stat.S_ISLNK(metadata.st_mode):
                    raise _error(f"artifact tree may not contain symlinks: {child}")
                if not stat.S_ISDIR(metadata.st_mode):
                    raise _error(f"artifact tree contains a non-directory: {child}")
                checked_dirs.append(name)
            dirnames[:] = checked_dirs
            for name in sorted(filenames):
                child = current_path / name
                _ensure_regular(child, label=f"artifact file {child}")
                relative = child.relative_to(root).as_posix()
                _safe_relative(relative, label="artifact member path")
                files.append((relative, child))
    except OSError as exc:
        raise _error(f"cannot inspect artifact tree: {root}") from exc
    files.sort(key=lambda item: item[0])
    return files


def _directory_paths(root: Path) -> set[str]:
    directories: set[str] = set()
    _ensure_directory(root, label="artifact root")
    try:
        for current, dirnames, _filenames in os.walk(root, topdown=True, followlinks=False):
            current_path = Path(current)
            for name in sorted(dirnames):
                child = current_path / name
                metadata = child.lstat()
                if stat.S_ISLNK(metadata.st_mode):
                    raise _error(f"artifact tree may not contain symlinks: {child}")
                if not stat.S_ISDIR(metadata.st_mode):
                    raise _error(f"artifact tree contains a non-directory: {child}")
                directories.add(child.relative_to(root).as_posix())
    except OSError as exc:
        raise _error(f"cannot inspect artifact directories: {root}") from exc
    return directories


def _expected_parent_directories(paths: Iterable[str]) -> set[str]:
    expected: set[str] = set()
    for relative in paths:
        parts = relative.split("/")[:-1]
        for index in range(1, len(parts) + 1):
            expected.add("/".join(parts[:index]))
    return expected


def _validate_tree_layout(
    root: Path,
    expected_files: Iterable[str],
    *,
    required_directories: Iterable[str] = (),
    label: str,
) -> list[tuple[str, Path]]:
    expected = {_safe_relative(path, label=f"{label} manifest path") for path in expected_files}
    observed = _iter_regular_files(root)
    observed_paths = {relative for relative, _path in observed}
    if observed_paths != expected:
        missing = sorted(expected - observed_paths)
        extra = sorted(observed_paths - expected)
        raise _error(f"{label} membership differs (missing={missing!r}, extra={extra!r})")
    expected_dirs = _expected_parent_directories(expected)
    expected_dirs.update(required_directories)
    for directory in expected_dirs:
        _safe_relative(directory, label=f"{label} directory")
    observed_dirs = _directory_paths(root)
    if observed_dirs != expected_dirs:
        missing = sorted(expected_dirs - observed_dirs)
        extra = sorted(observed_dirs - expected_dirs)
        raise _error(f"{label} directory membership differs (missing={missing!r}, extra={extra!r})")
    return observed


def _validate_tree_safety(root: Path, *, label: str) -> None:
    """Walk a consumer tree while permitting owner-generated extra carriers."""
    _iter_regular_files(root)
    _directory_paths(root)


def _manifest_files(root: Path) -> list[dict[str, Any]]:
    observed = _iter_regular_files(root)
    return [
        {
            "path": relative,
            "sha256": digest_file(path),
            "size_bytes": path.stat().st_size,
        }
        for relative, path in observed
    ]


def _verify_members(
    root: Path,
    entries: Any,
    *,
    integration_name: str | None = None,
    required_directories: Iterable[str] = (),
    label: str,
) -> None:
    if not isinstance(entries, list):
        raise _error(f"{label} must be a list")
    paths: list[str] = []
    previous: str | None = None
    expected_metadata: dict[str, tuple[int, str]] = {}
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict) or set(entry) != {"path", "sha256", "size_bytes"}:
            raise _error(f"{label} member {index} has an unexpected field set")
        relative = _safe_relative(entry["path"], label=f"{label} member path")
        if integration_name is not None and relative == integration_name:
            raise _error(f"{label} may not include its integration manifest")
        if previous is not None and relative <= previous:
            raise _error(f"{label} members must be sorted and unique")
        previous = relative
        digest = entry["sha256"]
        if not isinstance(digest, str) or HEX64.fullmatch(digest) is None:
            raise _error(f"{label} member digest is invalid")
        size = entry["size_bytes"]
        if type(size) is not int or size < 0:
            raise _error(f"{label} member size is invalid")
        paths.append(relative)
        expected_metadata[relative] = (size, digest)

    expected_files = [*paths]
    if integration_name is not None:
        expected_files.append(integration_name)
    observed = _validate_tree_layout(
        root,
        expected_files,
        required_directories=required_directories,
        label=label,
    )
    for relative, path in observed:
        if relative == integration_name:
            continue
        size, digest = expected_metadata[relative]
        metadata = _ensure_regular(path, label=f"{label} member {relative}")
        if metadata.st_size != size or digest_file(path) != digest:
            raise _error(f"{label} member digest or size mismatch: {relative}")


def _verify_export_copy(export_root: Path, provider_root: Path, export_manifest: dict[str, Any]) -> None:
    source_entries = export_manifest.get("files")
    if not isinstance(source_entries, list):
        raise _error("KAG export does not contain a source file list")
    expected: list[str] = []
    for entry in source_entries:
        if not isinstance(entry, dict) or set(entry) != {"path", "sha256", "size_bytes"}:
            raise _error("KAG export contains an invalid source member")
        expected.append(_safe_relative(entry["path"], label="KAG source member path"))
    # The selected consumer may materialize additional generated indexes or
    # transport carriers under its private root.  The source closure itself is
    # still checked below; the final integration manifest binds every such
    # regular byte, and its verifier rejects later undeclared additions.
    _validate_tree_safety(provider_root, label="private KAG source copy")
    by_path = {entry["path"]: entry for entry in source_entries}
    for relative in expected:
        source = export_root / "Tree-of-Sophia" / relative
        target = provider_root / relative
        source_metadata = _ensure_regular(source, label=f"KAG export source {relative}")
        target_metadata = _ensure_regular(target, label=f"private KAG source {relative}")
        entry = by_path[relative]
        if (source_metadata.st_size != entry["size_bytes"]
                or target_metadata.st_size != entry["size_bytes"]
                or digest_file(source) != entry["sha256"]
                or digest_file(target) != entry["sha256"]):
            raise _error(f"private KAG source copy differs: {relative}")


def _program_manifest(kag_root: Path) -> list[dict[str, str]]:
    result = []
    for relative in PROGRAM_PATHS:
        path = kag_root / relative
        _ensure_regular(path, label=f"selected KAG program {relative}")
        result.append({"path": relative, "sha256": digest_file(path)})
    return result


def _strict_json(raw: bytes, *, label: str) -> Any:
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in items:
            if key in result:
                raise _error(f"{label} contains duplicate field {key!r}")
            result[key] = value
        return result

    def reject_constant(value: str) -> None:
        raise ValueError(f"non-finite JSON value {value}")

    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=pairs,
            parse_constant=reject_constant,
        )
    except CorpusStoreError:
        raise
    except (TypeError, UnicodeError, ValueError) as exc:
        raise _error(f"{label} is not valid finite UTF-8 JSON: {exc}") from exc


def _identity_safe(value: Any, *, label: str) -> None:
    """Reject host/runtime provenance that cannot be a portable identity."""
    if isinstance(value, dict):
        for key, child in value.items():
            if not isinstance(key, str):
                raise _error(f"{label} contains a non-string field name")
            lowered = key.lower()
            if ("attempt" in lowered or "timestamp" in lowered
                    or lowered in {"time", "created_at", "updated_at", "started_at", "completed_at"}):
                raise _error(f"{label} contains a runtime field: {key}")
            _identity_safe(child, label=f"{label}.{key}")
        return
    if isinstance(value, list):
        for index, child in enumerate(value):
            _identity_safe(child, label=f"{label}[{index}]")
        return
    if isinstance(value, str):
        if value.startswith("/") or re.match(r"^[A-Za-z]:[\\/]", value) or value.startswith("\\\\"):
            raise _error(f"{label} contains a host absolute path")


def _verify_integration(
    root: Path,
    *,
    expected_revision: str,
) -> dict[str, Any]:
    _ensure_directory(root, label="published KAG release")
    integration_path = root / "integration.json"
    _ensure_regular(integration_path, label="KAG integration manifest")
    raw = integration_path.read_bytes()
    integration = _strict_json(raw, label="KAG integration manifest")
    if raw != canonical(integration):
        raise _error("KAG integration manifest is not canonical JSON")
    keys = {
        "schema_version",
        "corpus_revision",
        "export_revision",
        "distribution_identity",
        "primary_source",
        "files",
        "programs",
        "integration_revision",
    }
    if not isinstance(integration, dict) or set(integration) != keys:
        raise _error("KAG integration manifest has an unexpected field set")
    if integration["schema_version"] != SCHEMA:
        raise _error("unsupported KAG integration manifest")
    if integration["corpus_revision"] != expected_revision:
        raise _error("KAG integration corpus revision differs")
    hex_digest(integration["corpus_revision"])
    hex_digest(integration["export_revision"])
    hex_digest(integration["integration_revision"])
    body = {key: value for key, value in integration.items() if key != "integration_revision"}
    if hashlib.sha256(canonical(body)).hexdigest() != integration["integration_revision"]:
        raise _error("KAG integration identity mismatch")
    distribution = integration["distribution_identity"]
    if not isinstance(distribution, dict):
        raise _error("KAG distribution identity must be an object")
    _identity_safe(distribution, label="distribution identity")
    primary = integration["primary_source"]
    if (not isinstance(primary, dict) or set(primary) != {"identity", "owner_return_route"}
            or not isinstance(primary["identity"], dict)
            or set(primary["identity"]) != {"path", "content_hash"}
            or not isinstance(primary["owner_return_route"], dict)):
        raise _error("KAG primary source has an unexpected shape")
    _identity_safe(primary, label="primary source")
    if (primary["identity"]["path"] != PRIMARY
            or not isinstance(primary["identity"]["content_hash"], str)
            or HEX64.fullmatch(primary["identity"]["content_hash"]) is None):
        raise _error("KAG primary source identity is invalid")
    _verify_members(
        root,
        integration["files"],
        integration_name="integration.json",
        required_directories=(
            "export",
            "export/Tree-of-Sophia",
            "provider",
            "provider/Tree-of-Sophia",
            "artifacts",
        ),
        label="KAG integration files",
    )
    programs = integration["programs"]
    if not isinstance(programs, list) or [
        entry.get("path") for entry in programs if isinstance(entry, dict)
    ] != list(PROGRAM_PATHS):
        raise _error("KAG program bindings differ")
    for entry, expected_path in zip(programs, PROGRAM_PATHS):
        if (not isinstance(entry, dict) or set(entry) != {"path", "sha256"}
                or entry["path"] != expected_path
                or not isinstance(entry["sha256"], str)
                or HEX64.fullmatch(entry["sha256"]) is None):
            raise _error("KAG program binding has an unexpected shape")
    # These hashes bind the consumer bytes that produced this historical
    # release.  A later checkout may legitimately have a different identity.
    export_root = root / "export"
    export_manifest = verify_export(export_root)
    if (export_manifest["corpus_revision"] != expected_revision
            or export_manifest["export_revision"] != integration["export_revision"]):
        raise _error("published KAG export binding differs")
    provider_root = root / "provider" / "Tree-of-Sophia"
    _verify_export_copy(export_root, provider_root, export_manifest)
    return integration


def _copy_source_tree(export_root: Path, provider_root: Path, export_manifest: dict[str, Any]) -> None:
    source_root = export_root / "Tree-of-Sophia"
    _ensure_directory(source_root, label="KAG export source tree")
    if os.path.lexists(provider_root):
        raise _error("private KAG provider output must be new")
    provider_root.mkdir(parents=True)
    _ensure_directory(provider_root.parent, label="private KAG provider root")
    for entry in export_manifest["files"]:
        relative = _safe_relative(entry["path"], label="KAG source member path")
        source = source_root / relative
        target = provider_root / relative
        metadata = _ensure_regular(source, label=f"KAG export source {relative}")
        if metadata.st_size != entry["size_bytes"] or digest_file(source) != entry["sha256"]:
            raise _error(f"KAG export source changed before consumer copy: {relative}")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        copied = _ensure_regular(target, label=f"private KAG source {relative}")
        if copied.st_size != entry["size_bytes"] or digest_file(target) != entry["sha256"]:
            raise _error(f"private KAG source copy failed: {relative}")
    _verify_export_copy(export_root, provider_root, export_manifest)


def _materialize_provider_controls(provider_root: Path) -> list[dict[str, Any]]:
    from kag_provider_controls import materialize_provider_controls

    entries = materialize_provider_controls(provider_root)
    if not isinstance(entries, list):
        raise _error("provider controls materializer returned an invalid list")
    return entries


def _verify_provider_controls(provider_root: Path, entries: list[dict[str, Any]]) -> None:
    from kag_provider_controls import verify_provider_controls

    verify_provider_controls(provider_root, entries)


def _run_program(kag_root: Path, provider_root: Path, artifact_root: Path) -> None:
    producer = kag_root / PROGRAM_PATHS[0]
    try:
        completed = subprocess.run(
            [
                sys.executable,
                str(producer),
                "--repo-root",
                str(provider_root),
                "--artifact-root",
                str(artifact_root),
            ],
            cwd=kag_root,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as exc:
        raise _error(f"KAG owner producer could not start: {exc}") from exc
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", "replace").strip()
        if not detail:
            detail = f"exit status {completed.returncode}"
        raise _error(f"KAG owner producer failed: {_bounded_error(detail)}")
    _ensure_directory(artifact_root, label="KAG owner artifact root")


_PROBE = r'''
import json
import sys
from pathlib import Path

kag_root = Path(sys.argv[1])
provider_root = Path(sys.argv[2])
artifact_root = Path(sys.argv[3])
primary_path = sys.argv[4]
sys.path.insert(0, str(kag_root))
from scripts.validators.repo_local_kag_index import (  # noqa: E402
    load_repo_local_kag_repository_index_family_with_manifest,
)

source, _family, manifest = load_repo_local_kag_repository_index_family_with_manifest(
    provider_root,
    artifact_root=artifact_root,
    allow_shadow_git=False,
)
from scripts.validators.local_kag_subtree import _validate_provider_home  # noqa: E402
_validate_provider_home("Tree-of-Sophia", provider_root, prebuild=False)
records = source.get("records") if isinstance(source, dict) else None
if not isinstance(records, list):
    raise ValueError("source index has no records list")
matches = [record for record in records
           if isinstance(record, dict)
           and isinstance(record.get("identity"), dict)
           and record["identity"].get("path") == primary_path]
if len(matches) != 1:
    raise ValueError("source index did not return exactly one primary record")
record = matches[0]
identity = record["identity"]
if "content_hash" not in identity or "owner_return_route" not in record:
    raise ValueError("primary source record is incomplete")
if not isinstance(manifest, dict) or "distribution_identity" not in manifest:
    raise ValueError("portable KAG manifest has no distribution identity")
result = {
    "primary_source": {
        "identity": {
            "path": identity["path"],
            "content_hash": identity["content_hash"],
        },
        "owner_return_route": record["owner_return_route"],
    },
    "distribution_identity": manifest["distribution_identity"],
}
print(json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
'''


def _run_probe(kag_root: Path, provider_root: Path, artifact_root: Path) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            [
                sys.executable,
                "-I",
                "-c",
                _PROBE,
                str(kag_root),
                str(provider_root),
                str(artifact_root),
                PRIMARY,
            ],
            cwd=kag_root,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as exc:
        raise _error(f"KAG validator probe could not start: {exc}") from exc
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", "replace").strip()
        if not detail:
            detail = f"exit status {completed.returncode}"
        raise _error(f"KAG validator probe failed: {_bounded_error(detail)}")
    result = _strict_json(completed.stdout, label="KAG validator probe output")
    if not isinstance(result, dict) or set(result) != {"primary_source", "distribution_identity"}:
        raise _error("KAG validator probe returned an unexpected result")
    return result


def _verify_probe(
    probe: dict[str, Any],
    export_manifest: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    primary = probe.get("primary_source")
    distribution = probe.get("distribution_identity")
    if not isinstance(primary, dict) or not isinstance(distribution, dict):
        raise _error("KAG validator probe did not return complete identity")
    if (not isinstance(primary.get("identity"), dict)
            or set(primary["identity"]) != {"path", "content_hash"}
            or "owner_return_route" not in primary
            or not isinstance(primary["owner_return_route"], dict)):
        raise _error("KAG validator probe primary source has an unexpected shape")
    identity = primary["identity"]
    if (identity.get("path") != PRIMARY
            or identity.get("content_hash") != export_manifest["primary_source"]["sha256"]):
        raise _error("KAG validator returned a mismatched primary source digest")
    _identity_safe(probe, label="KAG validator identity")
    return primary, distribution


def _sync_tree(root: Path) -> None:
    observed = _iter_regular_files(root)
    for _relative, path in observed:
        descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    directories = _directory_paths(root)
    for relative in sorted(directories, key=lambda value: (value.count("/"), value), reverse=True):
        _sync_dir(root / relative)
    _sync_dir(root)


def _bounded_error(error: Any) -> str:
    message = str(error).replace("\x00", "\\x00").replace("\x7f", "\\x7f")
    return message[:MAX_ERROR_CHARS] or error.__class__.__name__


def build_release(
    store_root: Path,
    revision: str,
    kag_root: Path,
    release_root: Path,
) -> dict[str, Any]:
    """Build and atomically publish one local KAG release."""
    revision = hex_digest(revision)
    store_root = _safe_absolute(store_root, label="corpus store")
    kag_root = _selected_kag_root(kag_root)
    release_root = _prepare_release_root(release_root)
    status = DownstreamStatus(release_root / "status", "kag")
    attempt = status.begin(revision)
    try:
        with tempfile.TemporaryDirectory(prefix=".kag-release-", dir=release_root) as raw:
            stage = Path(raw) / "stage"
            stage.mkdir()
            # The consumer program identity is part of the release identity.
            # Capture it before any build work, then check it at both later
            # boundaries so a moving selected checkout cannot be published.
            program_before = _program_manifest(kag_root)
            export_root = stage / "export"
            export_manifest = build_export(store_root, revision, export_root)
            provider_root = stage / "provider" / "Tree-of-Sophia"
            provider_root.parent.mkdir(parents=True)
            _copy_source_tree(export_root, provider_root, export_manifest)
            provider_controls = _materialize_provider_controls(provider_root)
            artifact_root = stage / "artifacts"
            artifact_root.mkdir()
            _run_program(kag_root, provider_root, artifact_root)
            probe = _run_probe(kag_root, provider_root, artifact_root)
            _verify_provider_controls(provider_root, provider_controls)
            primary, distribution = _verify_probe(probe, export_manifest)
            program_after_probe = _program_manifest(kag_root)
            if program_before != program_after_probe:
                raise _error("selected KAG programs changed during publication")
            rerun_export = verify_export(export_root)
            if rerun_export != export_manifest:
                raise _error("KAG source export changed during publication")
            _verify_export_copy(export_root, provider_root, rerun_export)
            program_before_manifest = _program_manifest(kag_root)
            if program_after_probe != program_before_manifest:
                raise _error("selected KAG programs changed before manifest")
            files = _manifest_files(stage)
            body = {
                "schema_version": SCHEMA,
                "corpus_revision": revision,
                "export_revision": rerun_export["export_revision"],
                "distribution_identity": distribution,
                "primary_source": primary,
                "files": files,
                "programs": program_before_manifest,
            }
            _identity_safe(
                {
                    "distribution_identity": distribution,
                    "primary_source": primary,
                },
                label="KAG integration identity",
            )
            integration = {
                **body,
                "integration_revision": hashlib.sha256(canonical(body)).hexdigest(),
            }
            integration_path = stage / "integration.json"
            if os.path.lexists(integration_path):
                raise _error("KAG integration manifest unexpectedly exists")
            integration_path.write_bytes(canonical(integration))
            _ensure_regular(integration_path, label="KAG integration manifest")
            _sync_tree(stage)
            _sync_dir(Path(raw))
            destination = release_root / "releases" / integration["integration_revision"]
            if os.path.lexists(destination):
                existing = _verify_integration(destination, expected_revision=revision)
                if existing != integration:
                    raise _error("existing KAG integration identity differs")
                published = existing
            else:
                try:
                    _rename_new(stage, destination)
                except Exception as rename_error:
                    # A concurrent publisher may have won the no-replace
                    # race.  Reuse it only after the same complete check.
                    if not os.path.lexists(destination):
                        raise
                    existing = _verify_integration(destination, expected_revision=revision)
                    if existing != integration:
                        raise rename_error
                    published = existing
                else:
                    _sync_dir(destination.parent)
                    _sync_dir(release_root)
                    published = _verify_integration(
                        destination,
                        expected_revision=revision,
                    )
                    if published != integration:
                        raise _error("published KAG integration identity differs")

        try:
            status.succeed(
                attempt,
                artifact_revision=published["integration_revision"],
                artifact_manifest_sha256=digest_file(destination / "integration.json"),
            )
        except DownstreamStatusError as exc:
            if "stale" not in str(exc):
                raise
        return published
    except Exception as error:
        try:
            status.fail(attempt, _bounded_error(error))
        except Exception:
            # A newer attempt may own the status by the time this operation
            # fails.  Preserve the original owner/build error in that case.
            pass
        raise


def status_release(release_root: Path, expected_revision: str) -> dict[str, Any]:
    """Read status and verify the immutable artifact named by last_success."""
    release_root = _safe_absolute(release_root, label="release root")
    status = DownstreamStatus(release_root / "status", "kag").status(expected_revision)
    result = {
        **status,
        "source_kind": "corpus_revision",
        "integration_revision": None,
        "primary_source": None,
    }
    state = status["state"]
    last_success = state["last_success"] if isinstance(state, dict) else None
    if last_success is None:
        return result

    artifact_revision = last_success["artifact_revision"]
    destination = _safe_absolute(
        release_root / "releases" / artifact_revision,
        label="selected KAG release",
    )
    integration = _verify_integration(
        destination,
        expected_revision=last_success["source_revision"],
    )
    if integration["integration_revision"] != destination.name:
        raise _error("selected KAG release name differs from its integration identity")
    manifest_digest = digest_file(destination / "integration.json")
    if manifest_digest != last_success["artifact_manifest_sha256"]:
        raise _error("selected KAG integration manifest digest differs from status")
    return {
        **result,
        "integration_revision": integration["integration_revision"],
        "primary_source": integration["primary_source"],
    }


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("build")
    build.add_argument("--store", type=Path, required=True)
    build.add_argument("--revision", required=True)
    build.add_argument("--kag-root", type=Path, required=True)
    build.add_argument("--release-root", type=Path, required=True)
    status_parser = commands.add_parser("status")
    status_parser.add_argument("--release-root", type=Path, required=True)
    status_parser.add_argument("--expected-revision", required=True)
    args = parser.parse_args(argv)
    try:
        if args.command == "build":
            result = build_release(args.store, args.revision, args.kag_root, args.release_root)
        else:
            result = status_release(args.release_root, args.expected_revision)
    except (CorpusStoreError, DownstreamStatusError, ValueError, OSError, KeyError, TypeError) as error:
        parser.exit(1, f"KAG release rejected: {_bounded_error(error)}\n")
    print(canonical(result).decode("utf-8"), end="")


if __name__ == "__main__":
    main()

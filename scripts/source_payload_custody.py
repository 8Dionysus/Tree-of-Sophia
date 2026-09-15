#!/usr/bin/env python3
"""Bounded, manifest-driven custody for ignored source payload bytes.

This module deliberately keeps metadata and payload roots separate.  A caller
names the metadata manifest (or a frozen inventory) and an explicit directory
which mirrors ``ToS/source-witnesses``.  The helper only moves bytes described
by those records; it does not discover works, assess rights, or publish data.

The copy primitive uses a same-directory temporary file followed by an
exclusive hard-link publication.  ``os.replace`` is intentionally not used:
an interrupted or racing transfer must never replace an existing witness.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import tempfile
from typing import Any, Iterable


SOURCE_PREFIX = PurePosixPath("ToS/source-witnesses")
_CHUNK_SIZE = 1024 * 1024


class CustodyError(ValueError):
    """A manifest, path, fixity, or custody operation failed closed."""


@dataclass(frozen=True)
class PayloadEntry:
    """One immutable File in one Item and one source-root custody route."""

    item_id: str
    file_id: str | None
    item_root_ref: str
    relative_path: str
    byte_size: int
    sha256: str | None
    source_root: Path
    manifest_ref: str | None = None
    source_label: str = "source"
    git_blob_sha1: str | None = None

    @property
    def destination_ref(self) -> str:
        return f"{self.item_root_ref}/{self.relative_path}"


@dataclass(frozen=True)
class FileDigest:
    byte_size: int
    sha256: str
    git_blob_sha1: str


def _safe_posix_ref(ref: str, *, label: str) -> PurePosixPath:
    if not isinstance(ref, str) or not ref or "\\" in ref or "\x00" in ref:
        raise CustodyError(f"invalid {label}")
    parsed = PurePosixPath(ref)
    if (
        parsed.is_absolute()
        or str(parsed) != ref
        or any(part in {"", ".", ".."} for part in parsed.parts)
    ):
        raise CustodyError(f"unsafe {label}: {ref}")
    return parsed


def checked_root(root: Path | str, *, must_exist: bool = True) -> Path:
    """Return an absolute non-symlink directory suitable for custody."""

    path = Path(root).expanduser()
    if not path.is_absolute():
        raise CustodyError("custody roots must be absolute")
    if path.is_symlink():
        raise CustodyError(f"custody root must not be a symlink: {path}")
    if must_exist and not path.is_dir():
        raise CustodyError(f"custody root is not a directory: {path}")
    # Do not permit a root whose existing ancestor silently redirects through a
    # symlink.  Mount points are fine; only path symlinks are rejected.
    current = path
    while current != current.parent:
        if current.is_symlink():
            raise CustodyError(f"symlink in custody root: {current}")
        current = current.parent
    return path.resolve(strict=must_exist)


def _checked_child(root: Path, parts: Iterable[str]) -> Path:
    """Join path components while rejecting symlink escapes."""

    root = checked_root(root)
    parts_tuple = tuple(parts)
    candidate = root
    for index, part in enumerate(parts_tuple):
        candidate = candidate / part
        if candidate.is_symlink():
            raise CustodyError(f"symlink in custody path: {candidate}")
        if candidate.exists() and not candidate.is_dir() and index < len(parts_tuple) - 1:
            raise CustodyError(f"non-directory custody ancestor: {candidate}")
    try:
        candidate.resolve(strict=False).relative_to(root)
    except ValueError as exc:
        raise CustodyError("custody path escapes its root") from exc
    return candidate


def payload_path(
    payload_source_root: Path | str,
    item_root_ref: str,
    relative_path: str,
) -> Path:
    """Resolve ``ToS/source-witnesses/.../payload/file`` under an external root."""

    item_ref = _safe_posix_ref(item_root_ref, label="Item root reference")
    relative = _safe_posix_ref(relative_path, label="payload relative path")
    if tuple(item_ref.parts[:2]) != tuple(SOURCE_PREFIX.parts):
        raise CustodyError(f"Item root is outside {SOURCE_PREFIX}: {item_root_ref}")
    if not relative.parts or relative.parts[0] != "payload":
        raise CustodyError(f"payload path must begin with payload/: {relative_path}")
    root = checked_root(payload_source_root)
    return _checked_child(root, (*item_ref.parts[2:], *relative.parts))


def _metadata_path(metadata_root: Path | str, manifest_ref: str) -> Path:
    root = checked_root(metadata_root)
    ref = _safe_posix_ref(manifest_ref, label="manifest reference")
    return _checked_child(root, ref.parts)


def _item_root_from_manifest_ref(manifest_ref: str) -> str:
    ref = _safe_posix_ref(manifest_ref, label="manifest reference")
    if ref.name != "item.manifest.json":
        raise CustodyError("manifest reference must end in item.manifest.json")
    item_ref = PurePosixPath(*ref.parts[:-1])
    if tuple(item_ref.parts[:2]) != tuple(SOURCE_PREFIX.parts):
        raise CustodyError("Item manifest is outside ToS/source-witnesses")
    return item_ref.as_posix()


def _validate_digest_shape(value: str | None, label: str) -> None:
    if value is not None and (not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value)):
        raise CustodyError(f"invalid {label}")


def _entry_from_manifest_payload(
    *,
    item_id: str,
    item_root_ref: str,
    manifest_ref: str,
    payload_source_root: Path,
    payload: dict[str, Any],
    source_label: str,
) -> PayloadEntry:
    relative_path = payload.get("relative_path")
    if not isinstance(relative_path, str):
        raise CustodyError(f"payload entry has no relative_path: {manifest_ref}")
    _safe_posix_ref(relative_path, label="payload relative path")
    if not isinstance(payload.get("file_id"), str):
        raise CustodyError(f"payload entry has no file_id: {manifest_ref}")
    if not isinstance(payload.get("byte_size"), int) or isinstance(payload.get("byte_size"), bool) or payload["byte_size"] < 0:
        raise CustodyError(f"payload entry has invalid byte_size: {manifest_ref}")
    digest = payload.get("sha256")
    _validate_digest_shape(digest, "payload sha256")
    if payload["file_id"] != f"tos.file.sha256.{digest}":
        raise CustodyError(f"payload file_id is not bound to sha256: {manifest_ref}")
    return PayloadEntry(
        item_id=item_id,
        file_id=payload["file_id"],
        item_root_ref=item_root_ref,
        relative_path=relative_path,
        byte_size=payload["byte_size"],
        sha256=digest,
        source_root=payload_source_root,
        manifest_ref=manifest_ref,
        source_label=source_label,
    )


def entries_from_item_manifest(
    metadata_root: Path | str,
    manifest_ref: str,
    payload_source_root: Path | str,
    *,
    source_label: str = "item-manifest",
) -> list[PayloadEntry]:
    """Load one tracked Item manifest without scanning its surrounding tree."""

    manifest_path = _metadata_path(metadata_root, manifest_ref)
    try:
        manifest = json.loads(manifest_path.read_bytes())
    except (OSError, json.JSONDecodeError) as exc:
        raise CustodyError(f"cannot load Item manifest: {manifest_ref}") from exc
    item_id = manifest.get("item_id") if isinstance(manifest, dict) else None
    if (
        not isinstance(item_id, str)
        or not item_id.startswith("tos.item.")
        or len(item_id) <= len("tos.item.")
        or any(character.isspace() for character in item_id)
        or any(character in item_id for character in ("/", "\\", "\x00"))
    ):
        raise CustodyError(f"invalid Item manifest: {manifest_ref}")
    payload_files = manifest.get("payload_files")
    if not isinstance(payload_files, list) or not payload_files or any(not isinstance(payload, dict) for payload in payload_files):
        raise CustodyError(f"Item manifest payload_files is not a list: {manifest_ref}")
    item_root_ref = _item_root_from_manifest_ref(manifest_ref)
    root = checked_root(payload_source_root)
    return [
        _entry_from_manifest_payload(
            item_id=item_id,
            item_root_ref=item_root_ref,
            manifest_ref=manifest_ref,
            payload_source_root=root,
            payload=payload,
            source_label=source_label,
        )
        for payload in payload_files
    ]


def entries_from_inventory(
    inventory_path: Path | str,
    *,
    rows_key: str = "files",
    source_label: str = "inventory",
    only_present: bool = False,
) -> list[PayloadEntry]:
    """Convert a bounded retained-payload inventory into exact File entries."""

    path = Path(inventory_path).expanduser()
    if not path.is_absolute() or not path.is_file() or path.is_symlink():
        raise CustodyError(f"invalid custody inventory: {path}")
    try:
        inventory = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as exc:
        raise CustodyError(f"cannot load custody inventory: {path}") from exc
    rows = inventory.get(rows_key)
    if not isinstance(rows, list):
        raise CustodyError(f"inventory does not contain a list at {rows_key}: {path}")
    entries: list[PayloadEntry] = []
    for row in rows:
        if not isinstance(row, dict):
            raise CustodyError(f"inventory row is not an object: {path}")
        if only_present and row.get("source_present") is not True:
            continue
        source_root = row.get("source_root") or inventory.get("metadata_root")
        for key in ("manifest_ref", "item_ref", "file_ref", "relative_ref", "byte_size", "sha256"):
            if key not in row:
                raise CustodyError(f"inventory row lacks {key}: {path}")
        if not isinstance(source_root, str) or not source_root.startswith("/"):
            raise CustodyError("inventory source_root must be an absolute path")
        if not isinstance(row["item_ref"], str) or not row["item_ref"].startswith("tos.item."):
            raise CustodyError("inventory item_ref is invalid")
        if not isinstance(row["file_ref"], str) or not row["file_ref"].startswith("tos.file.sha256."):
            raise CustodyError("inventory file_ref is invalid")
        _validate_digest_shape(row["sha256"], "inventory sha256")
        if row["file_ref"] != f"tos.file.sha256.{row['sha256']}":
            raise CustodyError("inventory file_ref is not bound to sha256")
        ref = _safe_posix_ref(row["relative_ref"], label="inventory relative_ref")
        if len(ref.parts) < 4 or tuple(ref.parts[:2]) != tuple(SOURCE_PREFIX.parts) or "payload" not in ref.parts:
            raise CustodyError("inventory relative_ref is outside an Item payload")
        payload_index = ref.parts.index("payload")
        item_root_ref = PurePosixPath(*ref.parts[:payload_index]).as_posix()
        relative_path = PurePosixPath(*ref.parts[payload_index:]).as_posix()
        source_base = checked_root(source_root)
        if (source_base / SOURCE_PREFIX).is_dir():
            source_base = checked_root(source_base / SOURCE_PREFIX)
        entries.append(
            PayloadEntry(
                item_id=row["item_ref"],
                file_id=row["file_ref"],
                item_root_ref=item_root_ref,
                relative_path=relative_path,
                byte_size=row["byte_size"],
                sha256=row["sha256"],
                source_root=source_base,
                manifest_ref=row["manifest_ref"],
                source_label=source_label,
            )
        )
    return entries


def entries_from_registry_manifest(
    manifest_path: Path | str,
    source_root: Path | str,
    *,
    metadata_root: Path | str | None = None,
    source_label: str = "registry-manifest",
) -> tuple[list[PayloadEntry], list[dict[str, Any]]]:
    """Read a frozen preparation manifest and return present/missing payloads.

    Registry preparation manifests carry Git blob identity and size.  When an
    Item manifest is available beside the source payload, its SHA-256/File ID
    is cross-checked.  If it is absent, the exact pinned bytes are still
    independently hashed and the content-addressed File ID is derived; the
    receipt marks that source metadata was not present.
    """

    path = Path(manifest_path).expanduser()
    root = checked_root(source_root)
    metadata_base = checked_root(metadata_root) if metadata_root is not None else root
    if not path.is_absolute() or not path.is_file() or path.is_symlink():
        raise CustodyError(f"invalid registry manifest: {path}")
    try:
        manifest = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as exc:
        raise CustodyError(f"cannot load registry manifest: {path}") from exc
    if not isinstance(manifest, dict) or not isinstance(manifest.get("targets"), list):
        raise CustodyError(f"registry manifest lacks targets: {path}")
    try:
        manifest_ref = path.relative_to(metadata_base).as_posix()
    except ValueError:
        manifest_ref = path.name
    entries: list[PayloadEntry] = []
    missing: list[dict[str, Any]] = []
    for target in manifest["targets"]:
        if not isinstance(target, dict):
            raise CustodyError(f"registry target is not an object: {path}")
        item_id = target.get("ids", {}).get("item")
        item_root_ref = target.get("paths", {}).get("item_root")
        files = target.get("files")
        if not isinstance(item_id, str) or not isinstance(item_root_ref, str) or not isinstance(files, list):
            raise CustodyError(f"registry target identity/files are incomplete: {path}")
        for file in files:
            if not isinstance(file, dict) or not isinstance(file.get("basename"), str):
                raise CustodyError(f"registry file identity is incomplete: {path}")
            basename = file["basename"]
            _safe_posix_ref(basename, label="registry basename")
            relative_path = f"payload/{basename}"
            candidate = payload_path(root, item_root_ref, relative_path)
            row = {
                "source_label": source_label,
                "manifest_ref": manifest_ref,
                "item_id": item_id,
                "item_root_ref": item_root_ref,
                "relative_path": relative_path,
                "byte_size": file.get("byte_size"),
                "git_blob_sha1": file.get("git_blob_sha1"),
                "status": "missing" if not candidate.is_file() or candidate.is_symlink() else "source_present",
            }
            if row["status"] == "missing":
                missing.append(row)
                continue
            if not isinstance(row["byte_size"], int) or row["byte_size"] < 0:
                raise CustodyError(f"registry file has invalid byte_size: {path}")
            if not isinstance(row["git_blob_sha1"], str) or len(row["git_blob_sha1"]) != 40:
                raise CustodyError(f"registry file has invalid Git blob SHA-1: {path}")
            file_id: str | None = None
            digest: str | None = None
            item_manifest_ref = f"{item_root_ref}/item.manifest.json"
            item_manifest_path = metadata_base / PurePosixPath(item_manifest_ref)
            if item_manifest_path.is_file() and not item_manifest_path.is_symlink():
                bound = entries_from_item_manifest(
                    metadata_base,
                    item_manifest_ref,
                    root,
                    source_label=source_label,
                )
                matching = [candidate for candidate in bound if candidate.relative_path == relative_path]
                if len(matching) != 1:
                    raise CustodyError(f"registry file is not uniquely present in its Item manifest: {item_manifest_ref}")
                if matching[0].byte_size != row["byte_size"]:
                    raise CustodyError(f"registry and Item manifest sizes differ: {item_manifest_ref}")
                file_id, digest = matching[0].file_id, matching[0].sha256
            entries.append(
                PayloadEntry(
                    item_id=item_id,
                    file_id=file_id,
                    item_root_ref=item_root_ref,
                    relative_path=relative_path,
                    byte_size=row["byte_size"],
                    sha256=digest,
                    source_root=root,
                    manifest_ref=manifest_ref,
                    source_label=source_label,
                    git_blob_sha1=row["git_blob_sha1"],
                )
            )
    return entries, missing


def digest_file(path: Path) -> FileDigest:
    """Hash one regular file through an O_NOFOLLOW descriptor."""

    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags)
    except OSError as exc:
        raise CustodyError(f"cannot open payload without following symlink: {path}") from exc
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode):
            raise CustodyError(f"payload is not a regular file: {path}")
        size = 0
        sha = hashlib.sha256()
        blob = hashlib.sha1()
        blob.update(b"blob ")
        blob.update(str(info.st_size).encode("ascii"))
        blob.update(b"\0")
        with os.fdopen(fd, "rb", closefd=True) as stream:
            fd = -1
            while True:
                chunk = stream.read(_CHUNK_SIZE)
                if not chunk:
                    break
                size += len(chunk)
                sha.update(chunk)
                blob.update(chunk)
        return FileDigest(size, sha.hexdigest(), blob.hexdigest())
    finally:
        if fd != -1:
            os.close(fd)


def _open_payload(path: Path) -> tuple[int, os.stat_result]:
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(path, flags)
    except OSError as exc:
        raise CustodyError(f"cannot open payload without following symlink: {path}") from exc
    info = os.fstat(fd)
    if not stat.S_ISREG(info.st_mode):
        os.close(fd)
        raise CustodyError(f"payload is not a regular file: {path}")
    return fd, info


def verify_entry(entry: PayloadEntry, *, source_path: Path | None = None) -> FileDigest:
    """Verify source size/SHA/Git blob and return the independently read digest."""

    path = source_path or payload_path(entry.source_root, entry.item_root_ref, entry.relative_path)
    if path.is_symlink():
        raise CustodyError(f"source payload is a symlink: {path}")
    digest = digest_file(path)
    if digest.byte_size != entry.byte_size:
        raise CustodyError(f"source byte size differs for {entry.destination_ref}")
    if entry.sha256 is not None and digest.sha256 != entry.sha256:
        raise CustodyError(f"source SHA-256 differs for {entry.destination_ref}")
    if entry.git_blob_sha1 is not None and digest.git_blob_sha1 != entry.git_blob_sha1:
        raise CustodyError(f"source Git blob SHA-1 differs for {entry.destination_ref}")
    if entry.file_id is not None and entry.file_id != f"tos.file.sha256.{digest.sha256}":
        raise CustodyError(f"source File ID differs for {entry.destination_ref}")
    return digest


def _same_destination_digest(destination: Path, expected: FileDigest) -> bool:
    try:
        actual = digest_file(destination)
    except CustodyError:
        return False
    return actual == expected


def _publish_no_clobber(source: Path, destination: Path, expected: FileDigest) -> str:
    """Copy source to destination with an exclusive atomic publication."""

    if destination.exists() or destination.is_symlink():
        if destination.is_symlink():
            return "conflict"
        return "already_present" if _same_destination_digest(destination, expected) else "conflict"
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Validate newly-created ancestors too; mkdir must never reuse a symlink.
    _checked_child(destination.parent.parent, (destination.parent.name,))
    fd, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.custody-", suffix=".tmp", dir=destination.parent
    )
    temporary = Path(temporary_name)
    try:
        source_fd, _ = _open_payload(source)
        with os.fdopen(fd, "wb", closefd=True) as output:
            with os.fdopen(source_fd, "rb", closefd=True) as input_stream:
                while True:
                    chunk = input_stream.read(_CHUNK_SIZE)
                    if not chunk:
                        break
                    output.write(chunk)
            output.flush()
            os.fsync(output.fileno())
            os.fchmod(output.fileno(), 0o444)
        try:
            # link(2) is atomic and fails with EEXIST; it cannot clobber a
            # concurrent destination the way rename/replace can.
            temporary_digest = digest_file(temporary)
            if temporary_digest != expected:
                raise CustodyError(f"temporary payload readback differs before publish: {destination}")
            os.link(temporary, destination)
        except FileExistsError:
            return "already_present" if _same_destination_digest(destination, expected) else "conflict"
        readback = digest_file(destination)
        if readback != expected:
            try:
                if os.stat(destination).st_ino == os.stat(temporary).st_ino:
                    os.unlink(destination)
            except FileNotFoundError:
                pass
            raise CustodyError(f"destination readback differs for {destination}")
        try:
            os.chmod(destination, 0o444, follow_symlinks=False)
        except TypeError:
            os.chmod(destination, 0o444)
        try:
            directory_fd = os.open(destination.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
        except OSError:
            # File and readback fixity are the required custody proof; a
            # directory fsync may be unavailable on a mounted filesystem.
            pass
        return "copied"
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def publish_bytes_no_clobber(destination: Path, body: bytes, expected: FileDigest) -> str:
    """Publish already-verified downloaded bytes without replacing a witness."""

    if destination.exists() or destination.is_symlink():
        if destination.is_symlink():
            return "conflict"
        return "already_present" if _same_destination_digest(destination, expected) else "conflict"
    destination.parent.mkdir(parents=True, exist_ok=True)
    _checked_child(destination.parent.parent, (destination.parent.name,))
    fd, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.custody-", suffix=".tmp", dir=destination.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(fd, "wb", closefd=True) as output:
            output.write(body)
            output.flush()
            os.fsync(output.fileno())
            os.fchmod(output.fileno(), 0o444)
        try:
            temporary_digest = digest_file(temporary)
            if temporary_digest != expected:
                raise CustodyError(f"temporary payload readback differs before publish: {destination}")
            os.link(temporary, destination)
        except FileExistsError:
            return "already_present" if _same_destination_digest(destination, expected) else "conflict"
        readback = digest_file(destination)
        if readback != expected:
            try:
                if os.stat(destination).st_ino == os.stat(temporary).st_ino:
                    os.unlink(destination)
            except FileNotFoundError:
                pass
            raise CustodyError(f"destination readback differs for {destination}")
        try:
            os.chmod(destination, 0o444, follow_symlinks=False)
        except TypeError:
            os.chmod(destination, 0o444)
        return "copied"
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def destination_path(destination_payload_root: Path | str, entry: PayloadEntry) -> Path:
    return payload_path(destination_payload_root, entry.item_root_ref, entry.relative_path)


def destination_git_posture(repo_root: Path | str, entry: PayloadEntry) -> tuple[bool, bool]:
    """Return (ignored, tracked) for a stable Item/File reference."""

    root = checked_root(repo_root)
    relative = entry.destination_ref

    ignored = subprocess.run(
        ("git", "check-ignore", "--quiet", "--", relative),
        cwd=root,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode == 0
    tracked = subprocess.run(
        ("git", "ls-files", "--error-unmatch", "--", relative),
        cwd=root,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode == 0
    return ignored, tracked


def custody_row(entry: PayloadEntry, *, status: str, digest: FileDigest | None = None, reason: str | None = None) -> dict[str, Any]:
    row: dict[str, Any] = {
        "item_id": entry.item_id,
        "file_id": entry.file_id or (f"tos.file.sha256.{digest.sha256}" if digest else None),
        "manifest_ref": entry.manifest_ref,
        "item_root_ref": entry.item_root_ref,
        "relative_path": entry.relative_path,
        "destination_ref": entry.destination_ref,
        "source_label": entry.source_label,
        "expected_byte_size": entry.byte_size,
        "expected_sha256": entry.sha256 or (digest.sha256 if digest else None),
        "status": status,
    }
    if entry.git_blob_sha1 is not None:
        row["expected_git_blob_sha1"] = entry.git_blob_sha1
    if digest is not None:
        row.update({"byte_size": digest.byte_size, "sha256": digest.sha256, "git_blob_sha1": digest.git_blob_sha1})
    if reason:
        row["reason"] = reason
    return row


def deduplicate_entries(entries: Iterable[PayloadEntry]) -> tuple[list[PayloadEntry], list[dict[str, Any]]]:
    """Deduplicate only identical Item/File identities, never by bytes alone."""

    unique: list[PayloadEntry] = []
    duplicates: list[dict[str, Any]] = []
    seen: dict[tuple[str, str], PayloadEntry] = {}
    for entry in entries:
        key = (entry.item_id, entry.file_id or f"path:{entry.destination_ref}")
        previous = seen.get(key)
        if previous is None:
            seen[key] = entry
            unique.append(entry)
            continue
        if previous.destination_ref != entry.destination_ref or previous.byte_size != entry.byte_size or previous.sha256 != entry.sha256:
            raise CustodyError(f"Item/File duplicate has conflicting identity: {entry.item_id} / {entry.file_id}")
        duplicates.append({"item_id": entry.item_id, "file_id": entry.file_id, "kept_source": previous.source_label, "duplicate_source": entry.source_label, "destination_ref": entry.destination_ref, "status": "deduplicated"})
    return unique, duplicates


def plan_entries(
    entries: Iterable[PayloadEntry],
    *,
    destination_payload_root: Path | str | None = None,
    destination_repo_root: Path | str | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Read and verify all selected source files before any destination writes."""

    unique, duplicates = deduplicate_entries(entries)
    destination_root = checked_root(destination_payload_root) if destination_payload_root is not None else None
    if destination_repo_root is not None and destination_root is None:
        raise CustodyError("destination_repo_root requires destination_payload_root")
    destination_repo = checked_root(destination_repo_root) if destination_repo_root is not None else None
    rows: list[dict[str, Any]] = []
    for entry in unique:
        try:
            digest = verify_entry(entry)
        except (CustodyError, OSError) as exc:
            rows.append(custody_row(entry, status="source_invalid", reason=str(exc)))
            continue
        status = "source_verified"
        reason = None
        if destination_root is not None:
            if destination_repo is not None:
                ignored, tracked = destination_git_posture(destination_repo, entry)
                if not ignored or tracked:
                    rows.append(custody_row(entry, status="destination_policy_error", digest=digest, reason="payload path is not ignored or is Git-tracked"))
                    continue
            destination = destination_path(destination_root, entry)
            if destination.exists() or destination.is_symlink():
                if destination.is_symlink():
                    status, reason = "conflict", "destination is a symlink"
                elif _same_destination_digest(destination, digest):
                    status = "already_present"
                else:
                    status, reason = "conflict", "destination has different bytes"
            else:
                status = "planned_copy"
        rows.append(custody_row(entry, status=status, digest=digest, reason=reason))
    return rows, duplicates


def copy_entries(
    entries: Iterable[PayloadEntry],
    destination_payload_root: Path | str,
    *,
    destination_repo_root: Path | str | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Verify then copy selected entries, returning per-file custody rows."""

    destination_root = checked_root(destination_payload_root)
    unique, duplicates = deduplicate_entries(entries)
    rows: list[dict[str, Any]] = []
    for entry in unique:
        try:
            digest = verify_entry(entry)
            if destination_repo_root is not None:
                ignored, tracked = destination_git_posture(destination_repo_root, entry)
                if not ignored or tracked:
                    raise CustodyError("payload path is not ignored or is Git-tracked")
            destination = destination_path(destination_root, entry)
            status = _publish_no_clobber(payload_path(entry.source_root, entry.item_root_ref, entry.relative_path), destination, digest)
            rows.append(custody_row(entry, status=status, digest=digest))
        except (CustodyError, OSError) as exc:
            rows.append(custody_row(entry, status="failed", reason=str(exc)))
    return rows, duplicates


def write_receipt(path: Path | str, *, operation: str, rows: list[dict[str, Any]], duplicates: list[dict[str, Any]], missing: list[dict[str, Any]] = (), inputs: list[dict[str, Any]] = ()) -> Path:
    """Write a private/durable custody receipt without absolute host paths."""

    destination = Path(path).expanduser()
    if not destination.is_absolute():
        raise CustodyError("receipt path must be absolute")
    payload = {
        "schema_version": "tos.source_payload_custody_receipt.v1",
        "operation": operation,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "inputs": list(inputs),
        "counts": {
            "unique": len(rows),
            "copied": sum(row["status"] == "copied" for row in rows),
            "already_present": sum(row["status"] == "already_present" for row in rows),
            "planned_copy": sum(row["status"] == "planned_copy" for row in rows),
            "source_verified": sum(row["status"] == "source_verified" for row in rows),
            "conflict": sum(row["status"] == "conflict" for row in rows),
            "destination_policy_error": sum(row["status"] == "destination_policy_error" for row in rows),
            "failed": sum(row["status"] in {"failed", "source_invalid", "destination_policy_error"} for row in rows),
            "missing": len(missing),
            "deduplicated": len(duplicates),
        },
        "bytes": {
            "expected_unique": sum(row.get("expected_byte_size", 0) for row in rows),
            "copied_or_present": sum(row.get("byte_size", 0) for row in rows if row["status"] in {"copied", "already_present"}),
        },
        "rows": rows,
        "duplicates": duplicates,
        "missing": list(missing),
        "authority_boundary": "mechanical custody/fixity only; no rights, semantic, canon, publication, or human approval",
    }
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists() or destination.is_symlink():
        raise CustodyError(f"receipt already exists; choose a new immutable receipt path: {destination}")
    fd, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.", suffix=".tmp", dir=destination.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", closefd=True) as stream:
            stream.write(json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temporary, 0o600)
        try:
            os.link(temporary, destination)
        except FileExistsError as exc:
            raise CustodyError(f"receipt appeared during publication; choose a new immutable receipt path: {destination}") from exc
        os.chmod(destination, 0o600)
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass
    return destination


def _cli() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("verify", "copy"))
    parser.add_argument("--metadata-root", type=Path)
    parser.add_argument("--item-manifest", action="append", default=[])
    parser.add_argument("--inventory", action="append", default=[])
    parser.add_argument("--inventory-rows-key", default="files")
    parser.add_argument("--payload-source-root", type=Path, required=True)
    parser.add_argument("--destination-payload-root", type=Path)
    parser.add_argument("--receipt", type=Path, required=True)
    args = parser.parse_args()
    entries: list[PayloadEntry] = []
    if args.item_manifest:
        if args.metadata_root is None:
            parser.error("--metadata-root is required with --item-manifest")
        for ref in args.item_manifest:
            entries.extend(entries_from_item_manifest(args.metadata_root, ref, args.payload_source_root))
    for inventory in args.inventory:
        entries.extend(entries_from_inventory(inventory, rows_key=args.inventory_rows_key))
    if not entries:
        parser.error("at least one --item-manifest or --inventory is required")
    if args.command == "verify":
        rows, duplicates = plan_entries(entries, destination_payload_root=args.destination_payload_root)
    else:
        if args.destination_payload_root is None:
            parser.error("--destination-payload-root is required for copy")
        rows, duplicates = copy_entries(entries, args.destination_payload_root)
    write_receipt(args.receipt, operation=args.command, rows=rows, duplicates=duplicates)
    print(json.dumps({"status": "completed", "rows": len(rows), "duplicates": len(duplicates), "receipt": str(args.receipt)}))
    return 1 if any(row["status"] in {"failed", "source_invalid", "destination_policy_error", "conflict"} for row in rows) else 0


if __name__ == "__main__":
    try:
        raise SystemExit(_cli())
    except CustodyError as exc:
        raise SystemExit(f"source payload custody stopped: {exc}")

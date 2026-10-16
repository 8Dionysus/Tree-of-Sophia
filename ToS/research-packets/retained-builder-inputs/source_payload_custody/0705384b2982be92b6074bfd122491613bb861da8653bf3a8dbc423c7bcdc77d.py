#!/usr/bin/env python3
"""Compatibility API for the native, manifest-driven payload custody owner.

This module keeps the historical Python call shapes for source-witness
consumers. Parsing, path checks, fixity, publication, planning and receipts
are performed by ``tos-command`` through the bounded acquisition wire.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
from pathlib import Path, PurePosixPath
import sys
from typing import Any, Iterable

try:
    from acquisition_native import invoke, NativeAcquisitionError
except ModuleNotFoundError:  # pragma: no cover - package import
    from scripts.acquisition_native import invoke, NativeAcquisitionError


SOURCE_PREFIX = PurePosixPath("ToS/source-witnesses")
_CHUNK_SIZE = 1024 * 1024


class CustodyError(ValueError):
    """The native custody owner refused a manifest, path, or transfer."""


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


def _entry_json(entry: PayloadEntry) -> dict[str, Any]:
    return {
        "item_id": entry.item_id,
        "file_id": entry.file_id,
        "item_root_ref": entry.item_root_ref,
        "relative_path": entry.relative_path,
        "byte_size": entry.byte_size,
        "sha256": entry.sha256,
        "source_root": str(entry.source_root),
        "manifest_ref": entry.manifest_ref,
        "source_label": entry.source_label,
        "git_blob_sha1": entry.git_blob_sha1,
    }


def _entry_from_json(value: dict[str, Any]) -> PayloadEntry:
    return PayloadEntry(
        item_id=value["item_id"],
        file_id=value.get("file_id"),
        item_root_ref=value["item_root_ref"],
        relative_path=value["relative_path"],
        byte_size=value["byte_size"],
        sha256=value.get("sha256"),
        source_root=Path(value["source_root"]),
        manifest_ref=value.get("manifest_ref"),
        source_label=value.get("source_label", "source"),
        git_blob_sha1=value.get("git_blob_sha1"),
    )


def _digest_from_json(value: dict[str, Any]) -> FileDigest:
    return FileDigest(
        byte_size=value["byte_size"],
        sha256=value["sha256"],
        git_blob_sha1=value["git_blob_sha1"],
    )


def _wire_value(value: Any) -> Any:
    if isinstance(value, Path):
        return str(value.expanduser())
    if isinstance(value, PayloadEntry):
        return _entry_json(value)
    if isinstance(value, FileDigest):
        return {
            "byte_size": value.byte_size,
            "sha256": value.sha256,
            "git_blob_sha1": value.git_blob_sha1,
        }
    if isinstance(value, (list, tuple)):
        return [_wire_value(item) for item in value]
    if isinstance(value, dict):
        return {str(key): _wire_value(item) for key, item in value.items()}
    return value


def _request(operation: str, *, fetcher: Any = None, **fields: Any) -> dict[str, Any]:
    request = {
        "family": "custody",
        "operation": operation,
        **{name: _wire_value(value) for name, value in fields.items()},
    }
    try:
        return invoke(request, fetcher=fetcher)
    except NativeAcquisitionError as exc:
        raise CustodyError(exc.message) from exc


def _safe_posix_ref(ref: str, *, label: str) -> PurePosixPath:
    result = _request("safe_ref", ref=ref, label=label)
    return PurePosixPath(result["ref"])


def checked_root(root: Path | str, *, must_exist: bool = True) -> Path:
    result = _request("checked_root", root=Path(root).expanduser(), must_exist=must_exist)
    return Path(result["path"])


def _checked_child(root: Path, parts: Iterable[str]) -> Path:
    result = _request("checked_child", root=Path(root), parts=list(parts))
    return Path(result["path"])


def payload_path(
    payload_source_root: Path | str,
    item_root_ref: str,
    relative_path: str,
) -> Path:
    result = _request(
        "payload_path",
        payload_source_root=Path(payload_source_root).expanduser(),
        item_root_ref=item_root_ref,
        relative_path=relative_path,
    )
    return Path(result["path"])


def _metadata_path(metadata_root: Path | str, manifest_ref: str) -> Path:
    result = _request(
        "metadata_path",
        metadata_root=Path(metadata_root).expanduser(),
        manifest_ref=manifest_ref,
    )
    return Path(result["path"])


def _item_root_from_manifest_ref(manifest_ref: str) -> str:
    return _request("item_root_from_manifest_ref", manifest_ref=manifest_ref)["item_root_ref"]


def _validate_digest_shape(value: str | None, label: str) -> None:
    _request("validate_digest_shape", value=value, label=label)


def _entry_from_manifest_payload(
    *,
    item_id: str,
    item_root_ref: str,
    manifest_ref: str,
    payload_source_root: Path,
    payload: dict[str, Any],
    source_label: str,
) -> PayloadEntry:
    value = _request(
        "entry_from_manifest_payload",
        item_id=item_id,
        item_root_ref=item_root_ref,
        manifest_ref=manifest_ref,
        payload_source_root=payload_source_root,
        payload=payload,
        source_label=source_label,
    )
    return _entry_from_json(value["entry"])


def entries_from_item_manifest(
    metadata_root: Path | str,
    manifest_ref: str,
    payload_source_root: Path | str,
    *,
    source_label: str = "item-manifest",
) -> list[PayloadEntry]:
    value = _request(
        "entries_from_item_manifest",
        metadata_root=Path(metadata_root).expanduser(),
        manifest_ref=manifest_ref,
        payload_source_root=Path(payload_source_root).expanduser(),
        source_label=source_label,
    )
    return [_entry_from_json(row) for row in value["entries"]]


def entries_from_inventory(
    inventory_path: Path | str,
    *,
    rows_key: str = "files",
    source_label: str = "inventory",
    only_present: bool = False,
) -> list[PayloadEntry]:
    value = _request(
        "entries_from_inventory",
        inventory_path=Path(inventory_path).expanduser(),
        rows_key=rows_key,
        source_label=source_label,
        only_present=only_present,
    )
    return [_entry_from_json(row) for row in value["entries"]]


def entries_from_registry_manifest(
    manifest_path: Path | str,
    source_root: Path | str,
    *,
    metadata_root: Path | str | None = None,
    source_label: str = "registry-manifest",
) -> tuple[list[PayloadEntry], list[dict[str, Any]]]:
    value = _request(
        "entries_from_registry_manifest",
        manifest_path=Path(manifest_path).expanduser(),
        source_root=Path(source_root).expanduser(),
        metadata_root=Path(metadata_root).expanduser() if metadata_root is not None else None,
        source_label=source_label,
    )
    return [_entry_from_json(row) for row in value["entries"]], value["missing"]


def digest_file(
    path: Path,
    *,
    expected_mode: int | None = None,
    expected_owner_uid: int | None = None,
    require_single_link: bool = False,
    custody_root: Path | str | None = None,
) -> FileDigest:
    value = _request(
        "digest_file",
        path=Path(path),
        expected_mode=expected_mode,
        expected_owner_uid=expected_owner_uid,
        require_single_link=require_single_link,
        custody_root=Path(custody_root).expanduser() if custody_root is not None else None,
    )
    return _digest_from_json(value["digest"])


def verify_entry(entry: PayloadEntry, *, source_path: Path | None = None) -> FileDigest:
    value = _request(
        "verify_entry",
        entry=entry,
        source_path=Path(source_path) if source_path is not None else None,
    )
    return _digest_from_json(value["digest"])


def publish_bytes_no_clobber(
    destination: Path,
    body: bytes,
    expected: FileDigest,
    *,
    custody_root: Path | str | None = None,
) -> str:
    value = _request(
        "publish_bytes_no_clobber",
        destination=Path(destination),
        body_size=len(body),
        expected=expected,
        custody_root=Path(custody_root).expanduser() if custody_root is not None else None,
        fetcher=lambda _payload: body,
    )
    return value["status"]


def destination_path(destination_payload_root: Path | str, entry: PayloadEntry) -> Path:
    value = _request(
        "destination_path",
        destination_payload_root=Path(destination_payload_root).expanduser(),
        entry=entry,
    )
    return Path(value["path"])


def destination_git_posture(repo_root: Path | str, entry: PayloadEntry) -> tuple[bool, bool]:
    value = _request("destination_git_posture", repo_root=Path(repo_root).expanduser(), entry=entry)
    return value["ignored"], value["tracked"]


def custody_row(
    entry: PayloadEntry,
    *,
    status: str,
    digest: FileDigest | None = None,
    reason: str | None = None,
) -> dict[str, Any]:
    value = _request("custody_row", entry=entry, status=status, digest=digest, reason=reason)
    return value["row"]


def deduplicate_entries(
    entries: Iterable[PayloadEntry],
) -> tuple[list[PayloadEntry], list[dict[str, Any]]]:
    value = _request("deduplicate_entries", entries=list(entries))
    return [_entry_from_json(row) for row in value["entries"]], value["duplicates"]


def plan_entries(
    entries: Iterable[PayloadEntry],
    *,
    destination_payload_root: Path | str | None = None,
    destination_repo_root: Path | str | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    value = _request(
        "plan_entries",
        entries=list(entries),
        destination_payload_root=Path(destination_payload_root).expanduser() if destination_payload_root is not None else None,
        destination_repo_root=Path(destination_repo_root).expanduser() if destination_repo_root is not None else None,
    )
    return value["rows"], value["duplicates"]


def copy_entries(
    entries: Iterable[PayloadEntry],
    destination_payload_root: Path | str,
    *,
    destination_repo_root: Path | str | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    value = _request(
        "copy_entries",
        entries=list(entries),
        destination_payload_root=Path(destination_payload_root).expanduser(),
        destination_repo_root=Path(destination_repo_root).expanduser() if destination_repo_root is not None else None,
    )
    return value["rows"], value["duplicates"]


def write_receipt(
    path: Path | str,
    *,
    operation: str,
    rows: list[dict[str, Any]],
    duplicates: list[dict[str, Any]],
    missing: list[dict[str, Any]] = (),
    inputs: list[dict[str, Any]] = (),
) -> Path:
    value = _request(
        "write_receipt",
        path=Path(path).expanduser(),
        operation=operation,
        rows=rows,
        duplicates=duplicates,
        missing=list(missing),
        inputs=list(inputs),
    )
    return Path(value["path"])


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
    try:
        value = _request(
            "cli",
            command=args.command,
            metadata_root=args.metadata_root,
            item_manifests=args.item_manifest,
            inventories=args.inventory,
            inventory_rows_key=args.inventory_rows_key,
            payload_source_root=args.payload_source_root,
            destination_payload_root=args.destination_payload_root,
            receipt=args.receipt,
        )
    except CustodyError as exc:
        print(f"source payload custody stopped: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(value, ensure_ascii=False, sort_keys=True))
    return int(value.get("exit_code", 0))


if __name__ == "__main__":
    raise SystemExit(_cli())

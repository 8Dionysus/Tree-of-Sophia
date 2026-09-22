#!/usr/bin/env python3
"""Prepare and acquire one bounded ToS source batch.

The input manifest is the immutable selection boundary.  It names only the
reviewed records and payload files for one batch, including provider revision,
source IDs, rights references, and expected bytes.  The route copies that
small record closure into a handoff directory and acquires payload bytes into
an explicit custody root.  It never scans or rewrites the growing corpus
topology and never calls corpus admission, R2 transfer, publication, or a
Worker.

Each payload has an independent journal row.  A failed source does not stop
the other files; a later invocation verifies successful destinations and
retries only missing or failed files.  ``source_payload_custody`` supplies the
no-clobber publication and independent readback primitive.  The final handoff
always says ``admission_status: not-admitted``.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tempfile
from typing import Any, Callable
from urllib.request import Request, urlopen

from jsonschema import Draft202012Validator, FormatChecker

try:
    import source_payload_custody as custody
except ModuleNotFoundError as exc:  # pragma: no cover - direct package import
    if exc.name != "source_payload_custody":
        raise
    from scripts import source_payload_custody as custody


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_SCHEMA = Path("ToS/contracts/acquisition-batch.schema.json")
PROVENANCE_DELTA_SCHEMA = Path(
    "ToS/contracts/acquisition-provenance-delta.schema.json"
)
MAX_PAYLOAD_BYTES = 300 * 1024 * 1024
SHA256 = re.compile(r"^[a-f0-9]{64}$")
SHA1 = re.compile(r"^[a-f0-9]{40}$")
TOS_ITEM = re.compile(r"^tos\.item\.[a-z0-9]+(?:[.-][a-z0-9]+)*$")


class AcquisitionBatchError(ValueError):
    """The frozen selection, custody root, or batch evidence is invalid."""


class SourceFetchError(AcquisitionBatchError):
    """One provider fetch failed; the rest of the batch may continue."""


class SourceIntegrityError(AcquisitionBatchError):
    """Fetched or retained bytes do not match the frozen selection."""


@dataclass(frozen=True)
class BatchContext:
    repo_root: Path
    manifest_path: Path
    manifest_ref: str
    manifest_sha256: str
    raw_manifest: bytes
    manifest: dict[str, Any]


@dataclass(frozen=True)
class PayloadSelection:
    selection: dict[str, Any]
    payload: dict[str, Any]

    @property
    def item_ref(self) -> str:
        return self.payload["item_ref"]

    @property
    def file_ref(self) -> str:
        return self.payload["file_ref"]

    @property
    def destination_ref(self) -> str:
        return f"{self.payload['item_root_ref']}/{self.payload['relative_path']}"


Fetcher = Callable[[dict[str, Any]], bytes]


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00", "Z"
    )


def _sha256(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as exc:
        raise AcquisitionBatchError(f"cannot read file: {path}") from exc
    return digest.hexdigest()


def _git_blob_sha1(body: bytes) -> str:
    header = f"blob {len(body)}\0".encode("ascii")
    return hashlib.sha1(header + body).hexdigest()


def _strict_pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in items:
        if key in result:
            raise AcquisitionBatchError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _load_json_bytes(raw: bytes, *, label: str) -> dict[str, Any]:
    try:
        value = json.loads(raw, object_pairs_hook=_strict_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise AcquisitionBatchError(f"{label} is not valid JSON") from exc
    if not isinstance(value, dict):
        raise AcquisitionBatchError(f"{label} must be a JSON object")
    return value


def _canonical(value: Any) -> bytes:
    try:
        return (
            json.dumps(
                value,
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            )
            + "\n"
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise AcquisitionBatchError(f"cannot render canonical JSON: {exc}") from exc


def _safe_ref(ref: str, *, label: str, prefix: str | None = None) -> PurePosixPath:
    if not isinstance(ref, str) or not ref or "\\" in ref or "\x00" in ref:
        raise AcquisitionBatchError(f"invalid {label}")
    parsed = PurePosixPath(ref)
    if parsed.is_absolute() or str(parsed) != ref or any(
        part in {"", ".", ".."} for part in parsed.parts
    ):
        raise AcquisitionBatchError(f"unsafe {label}: {ref}")
    if prefix is not None and not ref.startswith(prefix):
        raise AcquisitionBatchError(f"{label} leaves {prefix}: {ref}")
    return parsed


def _checked_root(root: Path | str, *, create: bool = False) -> Path:
    path = Path(root).expanduser()
    if not path.is_absolute():
        raise AcquisitionBatchError(f"root must be absolute: {path}")
    if create:
        path.mkdir(parents=True, exist_ok=True)
    try:
        return custody.checked_root(path)
    except custody.CustodyError as exc:
        raise AcquisitionBatchError(str(exc)) from exc


def _regular_file(path: Path, *, label: str) -> os.stat_result:
    try:
        if path.is_symlink() or path.resolve(strict=True) != path.absolute():
            raise AcquisitionBatchError(f"{label} may not be a symlink: {path}")
        info = path.stat()
    except AcquisitionBatchError:
        raise
    except OSError as exc:
        raise AcquisitionBatchError(f"{label} is not readable: {path}") from exc
    if not stat.S_ISREG(info.st_mode):
        raise AcquisitionBatchError(f"{label} is not a regular file: {path}")
    return info


def _path_under(root: Path, ref: str, *, label: str) -> Path:
    parts = _safe_ref(ref, label=label)
    candidate = root.joinpath(*parts.parts)
    try:
        candidate.resolve(strict=False).relative_to(root.resolve())
    except ValueError as exc:
        raise AcquisitionBatchError(f"{label} escapes its root: {ref}") from exc
    for ancestor in (candidate, *candidate.parents):
        if ancestor == root.parent:
            break
        if ancestor.is_symlink():
            raise AcquisitionBatchError(f"symlink in {label}: {ancestor}")
    return candidate


def _write_immutable(path: Path, body: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        if not path.is_file() or path.read_bytes() != body:
            raise AcquisitionBatchError(f"immutable output conflict: {path}")
        return
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb", closefd=True) as stream:
            stream.write(body)
            stream.flush()
            os.fsync(stream.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError:
            if path.read_bytes() != body:
                raise AcquisitionBatchError(f"immutable output conflict: {path}")
        os.chmod(path, 0o644)
        try:
            directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
        except OSError:
            pass
    finally:
        temporary.unlink(missing_ok=True)


def _copy_metadata_no_clobber(source: Path, destination: Path, expected_sha256: str) -> str:
    info = _regular_file(source, label="selected source record")
    body = source.read_bytes()
    if _sha256(body) != expected_sha256:
        raise SourceIntegrityError(f"selected record digest differs: {source}")
    if destination.exists() or destination.is_symlink():
        if destination.is_symlink() or not destination.is_file():
            raise AcquisitionBatchError(f"selected record destination conflicts: {destination}")
        if _sha256_file(destination) != expected_sha256:
            raise AcquisitionBatchError(f"selected record destination bytes differ: {destination}")
        return "already_present"
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.", suffix=".tmp", dir=destination.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb", closefd=True) as stream:
            stream.write(body)
            stream.flush()
            os.fsync(stream.fileno())
        if _sha256_file(temporary) != expected_sha256:
            raise SourceIntegrityError(f"selected record readback differs: {source}")
        try:
            os.link(temporary, destination)
        except FileExistsError:
            if destination.is_symlink() or _sha256_file(destination) != expected_sha256:
                raise AcquisitionBatchError(f"selected record destination race: {destination}")
            return "already_present"
        os.chmod(destination, stat.S_IMODE(info.st_mode) or 0o644)
        return "copied"
    finally:
        temporary.unlink(missing_ok=True)


def _validate_semantics(manifest: dict[str, Any]) -> None:
    base_revision = manifest["base_revision"]
    delta = manifest["provenance_delta"]
    if delta["base_revision"] != base_revision:
        raise AcquisitionBatchError("provenance delta base revision differs from batch")
    selections = manifest["selection"]
    seen_items: set[str] = set()
    seen_files: set[str] = set()
    seen_destinations: set[str] = set()
    record_refs: set[str] = set()
    payload_refs: set[str] = set()
    for selection in selections:
        item_ref = selection["item_ref"]
        if not TOS_ITEM.fullmatch(item_ref) or item_ref in seen_items:
            raise AcquisitionBatchError(f"duplicate or invalid Item selection: {item_ref}")
        seen_items.add(item_ref)
        item_root = selection["item_root_ref"]
        _safe_ref(item_root, label="Item root", prefix="ToS/source-witnesses/")
        provider = selection["provider"]
        record_by_ref: dict[str, dict[str, Any]] = {}
        for record in selection["records"]:
            ref = record["ref"]
            _safe_ref(ref, label="record reference", prefix="ToS/")
            if "/payload/" in f"/{ref}/":
                raise AcquisitionBatchError(f"payload cannot be selected as metadata: {ref}")
            if ref in record_by_ref:
                raise AcquisitionBatchError(f"duplicate record reference: {ref}")
            record_by_ref[ref] = record
            record_refs.add(ref)
        rights = selection["rights"]
        if rights["ref"] not in record_by_ref or rights["ref"] not in {
            value["ref"] for value in selection["records"] if value["kind"] == "rights"
        }:
            raise AcquisitionBatchError(f"rights record is not selected for {item_ref}")
        rights_record = record_by_ref[rights["ref"]]
        if rights_record["sha256"] != rights["sha256"]:
            raise AcquisitionBatchError(f"rights record digest differs for {item_ref}")
        item_records = [
            value for value in selection["records"] if value["kind"] == "item"
        ]
        if len(item_records) != 1 or not item_records[0]["ref"].endswith("/item.json"):
            raise AcquisitionBatchError(f"selection must contain one Item record: {item_ref}")
        for payload in selection["payload_files"]:
            if payload["item_ref"] != item_ref or payload["item_root_ref"] != item_root:
                raise AcquisitionBatchError(f"payload Item binding differs: {item_ref}")
            if payload["file_ref"] != f"tos.file.sha256.{payload['sha256']}":
                raise AcquisitionBatchError(f"payload File ID is not SHA-bound: {payload['file_ref']}")
            if payload["provider_revision"] != provider["revision"]:
                raise AcquisitionBatchError(f"provider revision differs for {payload['file_ref']}")
            if payload["provider_source_id"] != provider["source_id"]:
                raise AcquisitionBatchError(f"provider source ID differs for {payload['file_ref']}")
            if payload["byte_size"] > MAX_PAYLOAD_BYTES:
                raise AcquisitionBatchError(f"payload exceeds bounded transfer limit: {payload['file_ref']}")
            destination = f"{item_root}/{payload['relative_path']}"
            if payload["file_ref"] in seen_files or destination in seen_destinations:
                raise AcquisitionBatchError(f"duplicate payload destination: {destination}")
            seen_files.add(payload["file_ref"])
            seen_destinations.add(destination)
            payload_refs.add(payload["file_ref"])
    if set(delta["record_refs"]) != record_refs:
        raise AcquisitionBatchError("provenance delta does not close over selected records")
    if set(delta["payload_file_refs"]) != payload_refs:
        raise AcquisitionBatchError("provenance delta does not close over selected payloads")


def load_manifest(
    manifest_path: Path | str,
    *,
    repo_root: Path | str = REPO_ROOT,
    expected_sha256: str | None = None,
) -> BatchContext:
    root = _checked_root(repo_root)
    path = Path(manifest_path).expanduser()
    if not path.is_absolute():
        path = root / path
    path = path.resolve()
    _regular_file(path, label="acquisition batch manifest")
    raw = path.read_bytes()
    digest = _sha256(raw)
    if not SHA256.fullmatch(digest):  # pragma: no cover - hashlib invariant
        raise AcquisitionBatchError("manifest digest calculation failed")
    if expected_sha256 is not None:
        if not SHA256.fullmatch(expected_sha256) or digest != expected_sha256:
            raise AcquisitionBatchError("acquisition batch manifest SHA-256 differs")
    manifest = _load_json_bytes(raw, label="acquisition batch manifest")
    schema_path = root / MANIFEST_SCHEMA
    try:
        schema = _load_json_bytes(schema_path.read_bytes(), label="acquisition batch schema")
        Draft202012Validator(schema, format_checker=FormatChecker()).validate(manifest)
    except OSError as exc:
        raise AcquisitionBatchError(f"cannot read acquisition batch schema: {schema_path}") from exc
    except Exception as exc:
        if isinstance(exc, AcquisitionBatchError):
            raise
        raise AcquisitionBatchError(f"acquisition batch schema validation failed: {exc}") from exc
    _validate_semantics(manifest)
    try:
        manifest_ref = path.relative_to(root).as_posix()
    except ValueError:
        manifest_ref = manifest["batch_id"]
    return BatchContext(root, path, manifest_ref, digest, raw, manifest)


def _payloads(context: BatchContext) -> list[PayloadSelection]:
    result: list[PayloadSelection] = []
    for selection in context.manifest["selection"]:
        result.extend(
            PayloadSelection(selection, payload)
            for payload in selection["payload_files"]
        )
    return sorted(result, key=lambda value: (value.item_ref, value.file_ref))


def _records(context: BatchContext) -> list[tuple[dict[str, Any], dict[str, Any]]]:
    result: list[tuple[dict[str, Any], dict[str, Any]]] = []
    for selection in context.manifest["selection"]:
        result.extend((selection, record) for record in selection["records"])
    by_ref: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
    for selection, record in result:
        prior = by_ref.get(record["ref"])
        if prior is not None and prior[1]["sha256"] != record["sha256"]:
            raise AcquisitionBatchError(f"record reference has divergent digests: {record['ref']}")
        by_ref.setdefault(record["ref"], (selection, record))
    return [by_ref[key] for key in sorted(by_ref)]


def _batch_slug(batch_id: str) -> str:
    return batch_id.removeprefix("tos.acquisition-batch.")


def _output_manifest_path(output_root: Path) -> Path:
    return output_root / "manifest.json"


def _provenance_delta(context: BatchContext) -> dict[str, Any]:
    delta = context.manifest["provenance_delta"]
    selected_items = [selection["item_ref"] for selection in context.manifest["selection"]]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/acquisition-provenance-delta.schema.json",
        "schema_version": "tos_acquisition_provenance_delta_v1",
        "event_ref": delta["event_ref"],
        "event_version": delta["event_version"],
        "change_kind": delta["change_kind"],
        "batch_id": context.manifest["batch_id"],
        "batch_revision": context.manifest["batch_revision"],
        "base_revision": context.manifest["base_revision"],
        "selection_manifest_ref": "manifest.json",
        "selection_manifest_sha256": context.manifest_sha256,
        "record_refs": delta["record_refs"],
        "payload_file_refs": delta["payload_file_refs"],
        "item_refs": selected_items,
        "supersedes_event_ref": delta.get("supersedes_event_ref"),
        "materialization": "apply-selected-records-and-custody-to-the-bound-base",
        "admission_status": "not-admitted",
    }


def _preparation_receipt(context: BatchContext, record_rows: list[dict[str, Any]], delta_ref: str, delta_sha256: str) -> dict[str, Any]:
    return {
        "schema_version": "tos_acquisition_preparation_receipt_v1",
        "batch_id": context.manifest["batch_id"],
        "batch_revision": context.manifest["batch_revision"],
        "manifest_ref": "manifest.json",
        "manifest_sha256": context.manifest_sha256,
        "base_revision": context.manifest["base_revision"],
        "record_count": len(record_rows),
        "selected_record_bytes": sum(row["byte_size"] for row in record_rows),
        "records": record_rows,
        "provenance_delta_ref": delta_ref,
        "provenance_delta_sha256": delta_sha256,
        "topology_preimages": 0,
        "storage_model": "batch_delta_without_per_target_topology_preimages",
        "payload_acquisition_performed": False,
        "admission_status": "not-admitted",
        "authority_boundary": "selection and metadata custody only; no semantic, rights, canon, corpus admission, R2, publication, or deployment acceptance",
    }


def prepare_batch(
    *,
    manifest_path: Path | str,
    metadata_root: Path | str,
    output_root: Path | str,
    repo_root: Path | str = REPO_ROOT,
    expected_manifest_sha256: str | None = None,
) -> dict[str, Any]:
    """Seal selected metadata and one batch-level provenance delta."""

    if expected_manifest_sha256 is None:
        raise AcquisitionBatchError(
            "prepare requires the expected frozen manifest SHA-256"
        )
    context = load_manifest(
        manifest_path,
        repo_root=repo_root,
        expected_sha256=expected_manifest_sha256,
    )
    metadata = _checked_root(metadata_root)
    output = Path(output_root).expanduser()
    if not output.is_absolute():
        raise AcquisitionBatchError("output root must be absolute")
    if output.exists() or output.is_symlink():
        raise AcquisitionBatchError(f"preparation output must be new: {output}")
    output.mkdir(parents=True)
    source_root = output / "source"
    payload_root = output / "payload"
    receipts_root = output / "receipts"
    source_root.mkdir()
    payload_root.mkdir()
    receipts_root.mkdir()

    _write_immutable(_output_manifest_path(output), context.raw_manifest)
    record_rows: list[dict[str, Any]] = []
    for _selection, record in _records(context):
        source = _path_under(metadata, record["ref"], label="selected metadata path")
        info = _regular_file(source, label="selected metadata record")
        status = _copy_metadata_no_clobber(
            source,
            _path_under(source_root, record["ref"], label="handoff metadata path"),
            record["sha256"],
        )
        record_rows.append(
            {
                "ref": record["ref"],
                "handoff_ref": f"source/{record['ref']}",
                "kind": record["kind"],
                "sha256": record["sha256"],
                "byte_size": info.st_size,
                "status": status,
            }
        )
    record_rows.sort(key=lambda row: row["ref"])
    delta = _provenance_delta(context)
    delta_ref = (
        f"source/ToS/source-witnesses/discovery/acquisition-batches/"
        f"{_batch_slug(context.manifest['batch_id'])}/provenance-delta.json"
    )
    delta_path = output / delta_ref
    delta_bytes = _canonical(delta)
    delta_schema = _load_json_bytes(
        (context.repo_root / PROVENANCE_DELTA_SCHEMA).read_bytes(),
        label="acquisition provenance delta schema",
    )
    try:
        Draft202012Validator(delta_schema).validate(delta)
    except Exception as exc:
        raise AcquisitionBatchError(
            f"acquisition provenance delta schema validation failed: {exc}"
        ) from exc
    _write_immutable(delta_path, delta_bytes)
    preparation = _preparation_receipt(
        context, record_rows, delta_ref, _sha256(delta_bytes)
    )
    _write_immutable(receipts_root / "preparation.json", _canonical(preparation))
    return {
        "status": "prepared-not-acquired",
        "batch_id": context.manifest["batch_id"],
        "manifest_sha256": context.manifest_sha256,
        "output_root": str(output),
        "record_count": len(record_rows),
        "selected_record_bytes": preparation["selected_record_bytes"],
        "provenance_delta": delta_ref,
    }


def _verify_prepared_output(context: BatchContext, output: Path) -> None:
    output = _checked_root(output)
    manifest_path = _output_manifest_path(output)
    _regular_file(manifest_path, label="prepared batch manifest")
    if manifest_path.read_bytes() != context.raw_manifest:
        raise AcquisitionBatchError("prepared output manifest differs from frozen selection")
    preparation_path = output / "receipts/preparation.json"
    _regular_file(preparation_path, label="preparation receipt")
    receipt = _load_json_bytes(preparation_path.read_bytes(), label="preparation receipt")
    if receipt.get("manifest_sha256") != context.manifest_sha256:
        raise AcquisitionBatchError("preparation receipt does not bind frozen selection")
    if receipt.get("topology_preimages") != 0:
        raise AcquisitionBatchError("prepared output contains a topology preimage claim")


def _fetch_url(payload: dict[str, Any]) -> bytes:
    request = Request(
        payload["provider_url"],
        headers={"User-Agent": "Tree-of-Sophia-bounded-acquisition/1"},
    )
    try:
        with urlopen(request, timeout=45) as response:
            body = bytearray()
            while True:
                block = response.read(min(1024 * 1024, payload["byte_size"] + 1 - len(body)))
                if not block:
                    break
                body.extend(block)
                if len(body) > payload["byte_size"]:
                    break
    except OSError as exc:
        raise SourceFetchError(f"provider fetch failed for {payload['file_ref']}: {exc}") from exc
    return bytes(body)


def _expected_file_digest(payload: dict[str, Any], body: bytes) -> custody.FileDigest:
    expected_git = payload.get("git_blob_sha1") or _git_blob_sha1(body)
    return custody.FileDigest(payload["byte_size"], payload["sha256"], expected_git)


def _verify_destination(path: Path, payload: dict[str, Any]) -> custody.FileDigest:
    try:
        digest = custody.digest_file(path)
    except custody.CustodyError as exc:
        raise SourceIntegrityError(str(exc)) from exc
    if digest.byte_size != payload["byte_size"] or digest.sha256 != payload["sha256"]:
        raise SourceIntegrityError(f"destination fixity differs: {payload['file_ref']}")
    expected_git = payload.get("git_blob_sha1")
    if expected_git is not None and digest.git_blob_sha1 != expected_git:
        raise SourceIntegrityError(f"destination Git blob differs: {payload['file_ref']}")
    return digest


def _append_journal(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a+", encoding="utf-8") as stream:
        fcntl.flock(stream.fileno(), fcntl.LOCK_EX)
        stream.seek(0, os.SEEK_END)
        stream.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")
        stream.flush()
        os.fsync(stream.fileno())
        fcntl.flock(stream.fileno(), fcntl.LOCK_UN)


def _journal_rows(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    rows: list[dict[str, Any]] = []
    try:
        with path.open("r", encoding="utf-8") as stream:
            for line_number, line in enumerate(stream, 1):
                if not line.strip():
                    continue
                try:
                    value = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise AcquisitionBatchError(f"acquisition journal is malformed at line {line_number}") from exc
                if not isinstance(value, dict):
                    raise AcquisitionBatchError("acquisition journal rows must be objects")
                rows.append(value)
    except OSError as exc:
        raise AcquisitionBatchError(f"cannot read acquisition journal: {path}") from exc
    return rows


def _run_id(receipts_root: Path) -> str:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    candidate = stamp
    suffix = 0
    while (receipts_root / f"handoff-{candidate}.json").exists():
        suffix += 1
        candidate = f"{stamp}-{suffix}"
    return candidate


def _fixity_receipt(
    context: BatchContext,
    output: Path,
    run_id: str,
) -> tuple[list[dict[str, Any]], str, str]:
    rows: list[dict[str, Any]] = []
    for item in _payloads(context):
        payload = item.payload
        destination = custody.payload_path(
            output / "payload", payload["item_root_ref"], payload["relative_path"]
        )
        row: dict[str, Any] = {
            "item_ref": item.item_ref,
            "file_ref": item.file_ref,
            "destination_ref": item.destination_ref,
            "relative_path": payload["relative_path"],
            "provider_revision": payload["provider_revision"],
            "provider_source_id": payload["provider_source_id"],
            "expected_byte_size": payload["byte_size"],
            "expected_sha256": payload["sha256"],
        }
        try:
            digest = _verify_destination(destination, payload)
        except (SourceIntegrityError, custody.CustodyError, OSError) as exc:
            row.update({"status": "missing-or-invalid", "error": str(exc)})
        else:
            row.update(
                {
                    "status": "verified",
                    "byte_size": digest.byte_size,
                    "sha256": digest.sha256,
                    "git_blob_sha1": digest.git_blob_sha1,
                }
            )
        rows.append(row)
    jsonl_body = b"".join(
        (json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n").encode("utf-8")
        for row in rows
    )
    jsonl_ref = f"receipts/fixity-{run_id}.jsonl"
    _write_immutable(output / jsonl_ref, jsonl_body)
    summary = {
        "schema_version": "tos_acquisition_independent_fixity_v1",
        "batch_id": context.manifest["batch_id"],
        "manifest_sha256": context.manifest_sha256,
        "run_id": run_id,
        "fixity_jsonl_ref": jsonl_ref,
        "fixity_jsonl_sha256": _sha256(jsonl_body),
        "rows": len(rows),
        "verified": sum(row["status"] == "verified" for row in rows),
        "invalid": sum(row["status"] != "verified" for row in rows),
        "independent_pass": True,
        "authority_boundary": "independent byte/readback fixity only; no admission or semantic acceptance",
    }
    summary_ref = f"receipts/fixity-{run_id}.json"
    _write_immutable(output / summary_ref, _canonical(summary))
    return rows, jsonl_ref, summary_ref


def _handoff_receipt(
    context: BatchContext,
    output: Path,
    run_id: str,
    payload_rows: list[dict[str, Any]],
    fixity_rows: list[dict[str, Any]],
    fixity_ref: str,
    fixity_summary_ref: str,
) -> dict[str, Any]:
    verified = {row["file_ref"] for row in fixity_rows if row["status"] == "verified"}
    expected = {item.file_ref for item in _payloads(context)}
    if verified == expected:
        acquisition_status = "acquired-not-admitted"
    elif verified:
        acquisition_status = "partially-acquired-not-admitted"
    else:
        acquisition_status = "prepared-not-acquired"
    records: list[dict[str, Any]] = []
    for selection, record in _records(context):
        source = output / "source" / record["ref"]
        records.append(
            {
                "item_ref": selection["item_ref"],
                "ref": record["ref"],
                "handoff_ref": f"source/{record['ref']}",
                "kind": record["kind"],
                "sha256": record["sha256"],
                "byte_size": source.stat().st_size,
                "rights_ref": selection["rights"]["ref"],
                "rights_sha256": selection["rights"]["sha256"],
            }
        )
    return {
        "schema_version": "tos_acquisition_handoff_v1",
        "batch_id": context.manifest["batch_id"],
        "batch_revision": context.manifest["batch_revision"],
        "run_id": run_id,
        "input_selection": {"ref": "manifest.json", "sha256": context.manifest_sha256},
        "base_revision": context.manifest["base_revision"],
        "source_records": sorted(records, key=lambda row: row["ref"]),
        "payload_custody": payload_rows,
        "independent_fixity": {
            "ref": fixity_ref,
            "summary_ref": fixity_summary_ref,
            "sha256": _sha256_file(output / fixity_ref),
        },
        "provenance_delta": {
            "ref": next(
                path.relative_to(output).as_posix()
                for path in (output / "source").rglob("provenance-delta.json")
            ),
            "event_ref": context.manifest["provenance_delta"]["event_ref"],
            "base_revision": context.manifest["base_revision"],
        },
        "acquisition_status": acquisition_status,
        "admission_status": "not-admitted",
        "publication_status": "not-published",
        "rights_posture": "preserved per selected Item rights record",
        "topology_preimages": 0,
        "source_failure_isolation": True,
        "restartable": True,
        "authority_boundary": "exact reviewed record and local payload custody handoff; no corpus admission, R2 transfer, publication, canon, semantic, or deployment acceptance",
    }


def acquire_batch(
    *,
    manifest_path: Path | str,
    metadata_root: Path | str,
    output_root: Path | str,
    repo_root: Path | str = REPO_ROOT,
    expected_manifest_sha256: str | None = None,
    fetcher: Fetcher | None = None,
    max_attempts: int = 2,
) -> dict[str, Any]:
    """Acquire payloads with per-file isolation and restartable receipts."""

    if max_attempts < 1:
        raise AcquisitionBatchError("max_attempts must be positive")
    if expected_manifest_sha256 is None:
        raise AcquisitionBatchError(
            "acquire requires the expected frozen manifest SHA-256"
        )
    context = load_manifest(
        manifest_path,
        repo_root=repo_root,
        expected_sha256=expected_manifest_sha256,
    )
    output = Path(output_root).expanduser()
    if not output.is_absolute():
        raise AcquisitionBatchError("output root must be absolute")
    if not output.exists():
        prepare_batch(
            manifest_path=manifest_path,
            metadata_root=metadata_root,
            output_root=output,
            repo_root=repo_root,
            expected_manifest_sha256=context.manifest_sha256,
        )
    _verify_prepared_output(context, output)
    receipts_root = _checked_root(output / "receipts")
    journal_path = receipts_root / "acquisition.jsonl"
    existing = _journal_rows(journal_path)
    previous_attempts: dict[str, int] = {}
    for row in existing:
        file_ref = row.get("file_ref")
        if isinstance(file_ref, str):
            previous_attempts[file_ref] = max(previous_attempts.get(file_ref, 0), int(row.get("attempt", 0)))
    fetch = fetcher or _fetch_url
    run_id = _run_id(receipts_root)
    payload_rows: list[dict[str, Any]] = []
    for item in _payloads(context):
        payload = item.payload
        destination = custody.payload_path(
            output / "payload", payload["item_root_ref"], payload["relative_path"]
        )
        base_row = {
            "run_id": run_id,
            "batch_id": context.manifest["batch_id"],
            "manifest_sha256": context.manifest_sha256,
            "item_ref": item.item_ref,
            "file_ref": item.file_ref,
            "destination_ref": item.destination_ref,
            "provider_url": payload["provider_url"],
            "provider_revision": payload["provider_revision"],
            "provider_source_id": payload["provider_source_id"],
            "expected_byte_size": payload["byte_size"],
            "expected_sha256": payload["sha256"],
        }
        try:
            if destination.exists() or destination.is_symlink():
                if destination.is_symlink():
                    raise SourceIntegrityError(f"destination is a symlink: {destination}")
                _verify_destination(destination, payload)
                row = {**base_row, "attempt": previous_attempts.get(item.file_ref, 0), "status": "already_present", "completed_at": utc_now()}
                _append_journal(journal_path, row)
                payload_rows.append(row)
                continue
        except SourceIntegrityError as exc:
            row = {**base_row, "attempt": previous_attempts.get(item.file_ref, 0), "status": "conflict", "error": str(exc), "completed_at": utc_now()}
            _append_journal(journal_path, row)
            payload_rows.append(row)
            continue

        completed: dict[str, Any] | None = None
        for local_attempt in range(1, max_attempts + 1):
            attempt = previous_attempts.get(item.file_ref, 0) + local_attempt
            try:
                body = fetch(payload)
                if not isinstance(body, bytes):
                    raise SourceFetchError("fetcher did not return bytes")
                if len(body) != payload["byte_size"] or _sha256(body) != payload["sha256"]:
                    raise SourceIntegrityError(f"provider bytes differ for {item.file_ref}")
                expected = _expected_file_digest(payload, body)
                status = custody.publish_bytes_no_clobber(destination, body, expected)
                _verify_destination(destination, payload)
                completed = {
                    **base_row,
                    "attempt": attempt,
                    "status": "acquired" if status == "copied" else "already_present",
                    "completed_at": utc_now(),
                    "readback_verified": True,
                }
                _append_journal(journal_path, completed)
                break
            except (SourceFetchError, SourceIntegrityError, custody.CustodyError, OSError) as exc:
                failed = {
                    **base_row,
                    "attempt": attempt,
                    "status": "failed",
                    "error": str(exc),
                    "failure_type": type(exc).__name__,
                    "completed_at": utc_now(),
                }
                _append_journal(journal_path, failed)
                if local_attempt == max_attempts:
                    completed = failed
        assert completed is not None
        payload_rows.append(completed)

    fixity_rows, fixity_ref, fixity_summary_ref = _fixity_receipt(context, output, run_id)
    handoff = _handoff_receipt(
        context,
        output,
        run_id,
        payload_rows,
        fixity_rows,
        fixity_ref,
        fixity_summary_ref,
    )
    handoff_ref = f"receipts/handoff-{run_id}.json"
    _write_immutable(output / handoff_ref, _canonical(handoff))
    return {
        "status": handoff["acquisition_status"],
        "batch_id": context.manifest["batch_id"],
        "manifest_sha256": context.manifest_sha256,
        "handoff_ref": handoff_ref,
        "fixity_ref": fixity_ref,
        "payload_count": len(payload_rows),
        "verified_payload_count": sum(row["status"] == "verified" for row in fixity_rows),
        "failed_payload_count": sum(row["status"] != "verified" for row in fixity_rows),
        "admission_status": "not-admitted",
    }


def verify_local(*, output_root: Path | str, repo_root: Path | str = REPO_ROOT) -> dict[str, Any]:
    """Perform an independent local fixity pass without changing custody."""

    output = _checked_root(output_root)
    manifest_path = _output_manifest_path(output)
    context = load_manifest(manifest_path, repo_root=repo_root)
    _verify_prepared_output(context, output)
    rows: list[dict[str, Any]] = []
    for item in _payloads(context):
        payload = item.payload
        destination = custody.payload_path(
            output / "payload", payload["item_root_ref"], payload["relative_path"]
        )
        row = {"file_ref": item.file_ref, "destination_ref": item.destination_ref}
        try:
            digest = _verify_destination(destination, payload)
        except (SourceIntegrityError, custody.CustodyError, OSError) as exc:
            row.update({"status": "missing-or-invalid", "error": str(exc)})
        else:
            row.update({"status": "verified", "byte_size": digest.byte_size, "sha256": digest.sha256})
        rows.append(row)
    return {
        "status": "verified" if all(row["status"] == "verified" for row in rows) else "incomplete",
        "batch_id": context.manifest["batch_id"],
        "manifest_sha256": context.manifest_sha256,
        "rows": rows,
        "topology_preimages": 0,
        "admission_status": "not-admitted",
    }


def measure_storage(output_root: Path | str) -> dict[str, Any]:
    """Measure selected handoff bytes and prove the old preimage path is absent."""

    output = _checked_root(output_root)
    files: list[Path] = []
    for path in output.rglob("*"):
        if path.is_file() and not path.is_symlink():
            files.append(path)
    metadata_files = [path for path in files if path.is_relative_to(output / "source")]
    payload_files = [path for path in files if path.is_relative_to(output / "payload")]
    topology_preimages = [
        path
        for path in files
        if "topology-before" in path.parts or path.name.startswith("topology-before")
    ]
    return {
        "schema_version": "tos_acquisition_storage_measurement_v1",
        "metadata_file_count": len(metadata_files),
        "metadata_bytes": sum(path.stat().st_size for path in metadata_files),
        "payload_file_count": len(payload_files),
        "payload_bytes": sum(path.stat().st_size for path in payload_files),
        "topology_preimage_count": len(topology_preimages),
        "topology_preimage_bytes": sum(path.stat().st_size for path in topology_preimages),
        "storage_model": "selected-records-and-batch-delta",
    }


def _common_parser(parser: argparse.ArgumentParser, *, output_required: bool = True) -> None:
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--metadata-root", type=Path, required=output_required)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--expected-manifest-sha256", required=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    prepare_parser = subparsers.add_parser("prepare")
    _common_parser(prepare_parser)
    acquire_parser = subparsers.add_parser("acquire")
    _common_parser(acquire_parser)
    acquire_parser.add_argument("--max-attempts", type=int, default=2)
    verify_parser = subparsers.add_parser("verify-local")
    verify_parser.add_argument("--output-root", type=Path, required=True)
    verify_parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(argv)
    try:
        if args.command == "prepare":
            result = prepare_batch(
                manifest_path=args.manifest,
                metadata_root=args.metadata_root,
                output_root=args.output_root,
                repo_root=args.repo_root,
                expected_manifest_sha256=args.expected_manifest_sha256,
            )
        elif args.command == "acquire":
            result = acquire_batch(
                manifest_path=args.manifest,
                metadata_root=args.metadata_root,
                output_root=args.output_root,
                repo_root=args.repo_root,
                expected_manifest_sha256=args.expected_manifest_sha256,
                max_attempts=args.max_attempts,
            )
        else:
            result = verify_local(output_root=args.output_root, repo_root=args.repo_root)
    except AcquisitionBatchError as exc:
        print(f"acquisition-batch: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    if args.command == "acquire" and result.get("status") != "acquired-not-admitted":
        return 1
    if args.command == "verify-local" and result.get("status") != "verified":
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

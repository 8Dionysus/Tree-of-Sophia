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
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
import fcntl
import hashlib
from http.client import HTTPException
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import sys
import tempfile
from typing import Any, Callable
from urllib.parse import urlsplit
from urllib.request import Request, urlopen

from jsonschema import Draft202012Validator, FormatChecker

try:
    import source_payload_custody as custody
    from corpus_source_validation import is_source_member
except ModuleNotFoundError as exc:  # pragma: no cover - direct package import
    if exc.name not in {"source_payload_custody", "corpus_source_validation"}:
        raise
    from scripts import source_payload_custody as custody
    from scripts.corpus_source_validation import is_source_member


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_SCHEMA = Path("ToS/contracts/acquisition-batch.schema.json")
PROVENANCE_DELTA_SCHEMA = Path(
    "ToS/contracts/acquisition-provenance-delta.schema.json"
)
RESOURCE_INVENTORY_SCHEMA = Path(
    "ToS/contracts/source-resource-inventory.schema.json"
)
CORPUS_RECORD_SCHEMA = Path("ToS/contracts/corpus-record.schema.json")
ITEM_MANIFEST_SCHEMA = Path("ToS/contracts/source-item-manifest.schema.json")
RIGHTS_RECORD_SCHEMA = Path("ToS/contracts/rights-record.schema.json")
PROVENANCE_EVENT_SCHEMA = Path("ToS/contracts/provenance-event.schema.json")
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

    @property
    def custody_key(self) -> tuple[str, str, str]:
        """Identity of this Item/File binding at its selected destination."""

        return self.item_ref, self.file_ref, self.destination_ref


def _payload_custody_key(row: dict[str, Any]) -> tuple[str, str, str] | None:
    item_ref = row.get("item_ref")
    file_ref = row.get("file_ref")
    destination_ref = row.get("destination_ref")
    if not all(isinstance(value, str) for value in (item_ref, file_ref, destination_ref)):
        return None
    return item_ref, file_ref, destination_ref


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


def _private_directory(path: Path, *, label: str) -> os.stat_result:
    """Require an owner-only custody directory before placing evidence in it."""

    try:
        if path.is_symlink():
            raise AcquisitionBatchError(f"{label} may not be a symlink: {path}")
        info = path.stat()
    except AcquisitionBatchError:
        raise
    except OSError as exc:
        raise AcquisitionBatchError(f"{label} is not readable: {path}") from exc
    if not stat.S_ISDIR(info.st_mode):
        raise AcquisitionBatchError(f"{label} is not a directory: {path}")
    if stat.S_IMODE(info.st_mode) != 0o700:
        raise AcquisitionBatchError(f"{label} must be owner-only (0700): {path}")
    return info


def _verify_declared_rights_posture(
    selection: dict[str, Any], rights_value: dict[str, Any], *, item_ref: str
) -> None:
    """Prevent a transfer selection from widening its selected rights record.

    Acquisition preserves the rights decision supplied by the selected Item;
    it does not decide whether bytes may be published.  The one mechanical
    boundary needed here is that a selection declaring ``public_payload`` has
    an explicit public visibility and redistribution posture in that same
    rights record.  Other postures remain owner-defined and are not narrowed
    by this transport check.
    """

    declared = selection["rights"].get("posture")
    if declared != "public_payload":
        return
    visibility = rights_value.get("visibility")
    redistribution = rights_value.get("redistribution_posture")
    if visibility != "public_payload" or redistribution not in {
        "authorized",
        "authorized_with_conditions",
    }:
        raise AcquisitionBatchError(
            "declared rights posture public_payload is not supported by selected rights record: "
            f"{item_ref}"
        )


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


def _selected_metadata_info(source: Path, *, source_ref: str) -> os.stat_result:
    info = _regular_file(source, label="selected source record")
    if stat.S_IMODE(info.st_mode) != 0o644:
        raise AcquisitionBatchError(
            f"selected source mode is unsupported for candidate update: {source_ref}"
        )
    return info


def _copy_metadata_no_clobber(
    source: Path,
    destination: Path,
    expected_sha256: str,
    *,
    source_ref: str,
) -> str:
    _selected_metadata_info(source, source_ref=source_ref)
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
        os.chmod(destination, 0o644)
        return "copied"
    finally:
        temporary.unlink(missing_ok=True)


def _validate_public_provider_url(value: str, *, label: str) -> None:
    try:
        parsed = urlsplit(value)
    except ValueError as exc:
        raise AcquisitionBatchError(f"{label} is malformed") from exc
    if (
        parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
    ):
        raise AcquisitionBatchError(
            f"{label} must not contain userinfo, a query, or a fragment"
        )


def _validate_semantics(manifest: dict[str, Any]) -> None:
    base_revision = manifest["base_revision"]
    delta = manifest["provenance_delta"]
    if delta["base_revision"] != base_revision:
        raise AcquisitionBatchError("provenance delta base revision differs from batch")
    selections = manifest["selection"]
    seen_items: set[str] = set()
    seen_destinations: set[str] = set()
    record_refs: set[str] = set()
    record_classifications: dict[str, tuple[str, str]] = {}
    payload_refs: set[str] = set()
    payload_descriptors: dict[str, tuple[str, int, str]] = {}
    for selection in selections:
        item_ref = selection["item_ref"]
        if not TOS_ITEM.fullmatch(item_ref) or item_ref in seen_items:
            raise AcquisitionBatchError(f"duplicate or invalid Item selection: {item_ref}")
        seen_items.add(item_ref)
        item_root = selection["item_root_ref"]
        _safe_ref(item_root, label="Item root", prefix="ToS/source-witnesses/")
        provider = selection["provider"]
        _validate_public_provider_url(
            provider["source_url"], label="provider source_url"
        )
        record_by_ref: dict[str, dict[str, Any]] = {}
        for record in selection["records"]:
            ref = record["ref"]
            _safe_ref(ref, label="record reference", prefix="ToS/")
            try:
                source_member = is_source_member(ref)
            except Exception as exc:
                raise AcquisitionBatchError(
                    f"record reference is not a valid corpus source member: {ref}"
                ) from exc
            if not source_member:
                raise AcquisitionBatchError(
                    f"record is outside the corpus source admission boundary: {ref}"
                )
            if "/payload/" in f"/{ref}/":
                raise AcquisitionBatchError(f"payload cannot be selected as metadata: {ref}")
            if ref in record_by_ref:
                raise AcquisitionBatchError(f"duplicate record reference: {ref}")
            record_by_ref[ref] = record
            classification = (record["kind"], record["sha256"])
            prior_classification = record_classifications.get(ref)
            if prior_classification is not None and prior_classification != classification:
                raise AcquisitionBatchError(
                    f"record reference has conflicting kind or digest: {ref}"
                )
            record_classifications[ref] = classification
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
        item_manifest_records = [
            value for value in selection["records"] if value["kind"] == "manifest"
        ]
        provenance_records = [
            value for value in selection["records"] if value["kind"] == "provenance"
        ]
        expected_item_manifest_ref = f"{item_root}/item.manifest.json"
        if (
            len(item_records) != 1
            or item_records[0]["ref"] != f"{item_root}/item.json"
            or not any(
                record["ref"] == expected_item_manifest_ref
                for record in item_manifest_records
            )
            or not provenance_records
        ):
            raise AcquisitionBatchError(f"selection must contain one Item record: {item_ref}")
        item_file_refs: set[str] = set()
        for payload in selection["payload_files"]:
            _validate_public_provider_url(
                payload["provider_url"], label="payload provider_url"
            )
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
            if payload["file_ref"] in item_file_refs:
                raise AcquisitionBatchError(
                    f"duplicate payload File ID within Item: {payload['file_ref']}"
                )
            item_file_refs.add(payload["file_ref"])
            descriptor = (
                payload["sha256"],
                payload["byte_size"],
                payload["media_type"],
            )
            prior_descriptor = payload_descriptors.get(payload["file_ref"])
            if prior_descriptor is not None and prior_descriptor != descriptor:
                raise AcquisitionBatchError(
                    f"payload File descriptor differs across Items: {payload['file_ref']}"
                )
            payload_descriptors[payload["file_ref"]] = descriptor
            destination = f"{item_root}/{payload['relative_path']}"
            if destination in seen_destinations:
                raise AcquisitionBatchError(f"duplicate payload destination: {destination}")
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
    return sorted(result, key=lambda value: value.custody_key)


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


def _provenance_delta_ref(context: BatchContext) -> str:
    """Return the one deterministic delta path for this prepared batch."""

    return (
        "source/ToS/source-witnesses/discovery/acquisition-batches/"
        f"{_batch_slug(context.manifest['batch_id'])}/provenance-delta.json"
    )


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
    output = Path(output_root).expanduser()
    if not output.is_absolute():
        raise AcquisitionBatchError("output root must be absolute")
    with _batch_execution_lock(output):
        return _prepare_batch_unlocked(
            context=context,
            metadata_root=metadata_root,
            output_root=output,
        )


def _prepare_batch_unlocked(
    *,
    context: BatchContext,
    metadata_root: Path | str,
    output_root: Path | str,
) -> dict[str, Any]:
    """Prepare one selection while the caller owns the batch execution lock."""

    metadata = _checked_root(metadata_root)
    output = Path(output_root).expanduser()
    if not output.is_absolute():
        raise AcquisitionBatchError("output root must be absolute")
    if output.exists() or output.is_symlink():
        raise AcquisitionBatchError(f"preparation output must be new: {output}")
    if not output.parent.is_dir() or output.parent.is_symlink():
        raise AcquisitionBatchError(f"preparation output parent must be a regular directory: {output.parent}")
    selected_records = _records(context)
    for _selection, record in selected_records:
        source = _path_under(metadata, record["ref"], label="selected metadata path")
        _selected_metadata_info(source, source_ref=record["ref"])
    output.mkdir(mode=0o700, parents=True)
    output.chmod(0o700)
    _private_directory(output, label="preparation output")
    source_root = output / "source"
    payload_root = output / "payload"
    receipts_root = output / "receipts"
    source_root.mkdir(mode=0o700)
    payload_root.mkdir(mode=0o700)
    receipts_root.mkdir(mode=0o700)
    source_root.chmod(0o700)
    payload_root.chmod(0o700)
    receipts_root.chmod(0o700)
    _private_directory(source_root, label="prepared source root")
    _private_directory(payload_root, label="prepared payload root")
    _private_directory(receipts_root, label="prepared receipts root")

    _write_immutable(_output_manifest_path(output), context.raw_manifest)
    record_rows: list[dict[str, Any]] = []
    for _selection, record in selected_records:
        source = _path_under(metadata, record["ref"], label="selected metadata path")
        info = _selected_metadata_info(source, source_ref=record["ref"])
        status = _copy_metadata_no_clobber(
            source,
            _path_under(source_root, record["ref"], label="handoff metadata path"),
            record["sha256"],
            source_ref=record["ref"],
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
    delta_ref = _provenance_delta_ref(context)
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
    _verify_prepared_output(context, output, require_private_roots=True)
    return {
        "status": "prepared-not-acquired",
        "batch_id": context.manifest["batch_id"],
        "manifest_sha256": context.manifest_sha256,
        "output_root": str(output),
        "record_count": len(record_rows),
        "selected_record_bytes": preparation["selected_record_bytes"],
        "provenance_delta": delta_ref,
    }


def _verify_prepared_item_bindings(context: BatchContext, output: Path) -> None:
    """Verify Item/manifest/rights/provenance/File closure before transfer.

    This is shared by preparation and the direct handoff adapter.  It checks
    custody identity and selected-record closure only; it does not run the
    source foundation validator or make an admission decision.
    """

    source_root = output / "source"
    schema_validators: dict[Path, Draft202012Validator] = {}

    def schema_validator(schema_ref: Path, *, label: str) -> Draft202012Validator:
        validator = schema_validators.get(schema_ref)
        if validator is not None:
            return validator
        schema_path = context.repo_root / schema_ref
        try:
            _regular_file(schema_path, label=f"{label} schema")
            schema = _load_json_bytes(
                schema_path.read_bytes(), label=f"{label} schema"
            )
            Draft202012Validator.check_schema(schema)
            validator = Draft202012Validator(
                schema, format_checker=FormatChecker()
            )
        except Exception as exc:
            raise AcquisitionBatchError(
                f"{label} schema is unavailable or invalid"
            ) from exc
        schema_validators[schema_ref] = validator
        return validator

    def validate_contract(
        value: Any, schema_ref: Path, *, label: str, item_ref: str
    ) -> None:
        validator = schema_validator(schema_ref, label=label)
        try:
            validator.validate(value)
        except Exception as exc:
            raise AcquisitionBatchError(
                f"prepared Item {label} does not satisfy its schema: {item_ref}"
            ) from exc

    for selection in context.manifest["selection"]:
        item_ref = selection["item_ref"]
        item_root = selection["item_root_ref"]
        records = {record["ref"]: record for record in selection["records"]}
        item_manifest_ref = f"{item_root}/item.manifest.json"
        item_manifest_record = records.get(item_manifest_ref)
        if (
            not isinstance(item_manifest_record, dict)
            or item_manifest_record.get("kind") != "manifest"
        ):
            raise AcquisitionBatchError(
                f"selection has no selected Item manifest record: {item_ref}"
            )
        item_manifest_path = _path_under(
            source_root, item_manifest_ref, label="prepared Item manifest"
        )
        _regular_file(item_manifest_path, label="prepared Item manifest")
        if _sha256_file(item_manifest_path) != item_manifest_record["sha256"]:
            raise SourceIntegrityError("prepared Item manifest bytes differ")

        item_record_path = _path_under(
            source_root, f"{item_root}/item.json", label="prepared Item record"
        )
        _regular_file(item_record_path, label="prepared Item record")
        item_record = _load_json_bytes(
            item_record_path.read_bytes(), label="prepared Item record"
        )
        if item_record.get("record_id") != item_ref:
            raise AcquisitionBatchError(
                f"prepared Item record identity differs from selection: {item_ref}"
            )
        item_record_manifest_ref = item_record.get("item_manifest_ref")
        if item_record_manifest_ref != item_manifest_ref:
            raise AcquisitionBatchError(
                f"prepared Item record does not bind the selected manifest: {item_ref}"
            )
        validate_contract(
            item_record,
            CORPUS_RECORD_SCHEMA,
            label="record",
            item_ref=item_ref,
        )

        item_manifest = _load_json_bytes(
            item_manifest_path.read_bytes(), label="prepared Item manifest"
        )
        validate_contract(
            item_manifest,
            ITEM_MANIFEST_SCHEMA,
            label="manifest",
            item_ref=item_ref,
        )
        provenance_ref = item_manifest.get("provenance_ref")
        provenance_record = (
            records.get(provenance_ref) if isinstance(provenance_ref, str) else None
        )
        if (
            item_manifest.get("schema_version") != "tos_source_item_manifest_v1"
            or item_manifest.get("item_id") != item_ref
            or item_manifest.get("rights_ref") != selection["rights"]["ref"]
            or not isinstance(provenance_record, dict)
            or provenance_record.get("kind") != "provenance"
            or not isinstance(item_manifest.get("payload_files"), list)
        ):
            raise AcquisitionBatchError(
                f"Item manifest identity or rights/provenance binding differs: {item_ref}"
            )

        for field in ("forensic_report_ref", "resource_inventory_ref"):
            companion_ref = item_manifest.get(field)
            companion_record = (
                records.get(companion_ref) if isinstance(companion_ref, str) else None
            )
            if not isinstance(companion_record, dict):
                raise AcquisitionBatchError(
                    f"Item manifest companion is not selected: {item_ref}: {field}"
                )
            companion_path = _path_under(
                source_root, companion_ref, label=f"prepared Item {field}"
            )
            _regular_file(companion_path, label=f"prepared Item {field}")
            if _sha256_file(companion_path) != companion_record["sha256"]:
                raise SourceIntegrityError(
                    f"prepared Item {field} bytes differ: {item_ref}"
                )
        fixity_ref = f"{item_root}/fixity.sha256"
        fixity_record = records.get(fixity_ref)
        if not isinstance(fixity_record, dict):
            raise AcquisitionBatchError(
                f"Item fixity companion is not selected: {item_ref}"
            )
        fixity_path = _path_under(
            source_root, fixity_ref, label="prepared Item fixity companion"
        )
        _regular_file(fixity_path, label="prepared Item fixity companion")
        if _sha256_file(fixity_path) != fixity_record["sha256"]:
            raise SourceIntegrityError(f"prepared Item fixity bytes differ: {item_ref}")

        manifest_by_file: dict[str, dict[str, Any]] = {}
        for payload in item_manifest["payload_files"]:
            if not isinstance(payload, dict) or payload.get("file_id") in manifest_by_file:
                raise AcquisitionBatchError("Item manifest payload file IDs are not unique")
            file_id = payload.get("file_id")
            if not isinstance(file_id, str):
                raise AcquisitionBatchError("Item manifest payload File ID is missing")
            manifest_by_file[file_id] = payload
        selected_payloads = {
            payload["file_ref"]: payload for payload in selection["payload_files"]
        }
        if set(manifest_by_file) != set(selected_payloads):
            raise AcquisitionBatchError(
                f"Item manifest payload closure differs: {item_ref}"
            )

        inventory_ref = item_manifest["resource_inventory_ref"]
        inventory_path = _path_under(
            source_root, inventory_ref, label="prepared Item resource inventory"
        )
        inventory_value = _load_json_bytes(
            inventory_path.read_bytes(), label="prepared Item resource inventory"
        )
        validate_contract(
            inventory_value,
            RESOURCE_INVENTORY_SCHEMA,
            label="resource inventory",
            item_ref=item_ref,
        )
        inventory_files = inventory_value["files"]
        for file_inventory in inventory_files:
            resources = file_inventory["resources"]
            if file_inventory["summary"].get("resource_count") != len(resources):
                raise AcquisitionBatchError(
                    f"Item resource inventory resource_count differs from resources: {item_ref}"
                )
            resource_ids = [resource["resource_id"] for resource in resources]
            if len(resource_ids) != len(set(resource_ids)):
                raise AcquisitionBatchError(
                    f"Item resource inventory has duplicate resource_id: {file_inventory['file_id']}"
                )
        expected_inventory_files = [
            {
                "file_id": payload["file_id"],
                "file_sha256": payload["sha256"],
                "media_type": payload["media_type"],
            }
            for payload in item_manifest["payload_files"]
        ]
        inventory_rows = (
            inventory_value.get("files") if isinstance(inventory_value, dict) else None
        )
        actual_inventory_files = [
            {
                "file_id": payload.get("file_id"),
                "file_sha256": payload.get("file_sha256"),
                "media_type": payload.get("media_type"),
            }
            for payload in inventory_rows
            if isinstance(payload, dict)
        ] if isinstance(inventory_rows, list) else []
        if (
            not isinstance(inventory_value, dict)
            or inventory_value.get("item_id") != item_ref
            or inventory_value.get("generated_from_manifest_ref") != item_manifest_ref
            or actual_inventory_files != expected_inventory_files
        ):
            raise AcquisitionBatchError(
                f"Item resource inventory does not close over manifest payloads: {item_ref}"
            )

        expected_fixity_lines = [
            f"{payload['sha256']}  {payload['relative_path']}"
            for payload in item_manifest["payload_files"]
        ]
        expected_fixity = "\n".join(expected_fixity_lines) + "\n"
        try:
            actual_fixity = fixity_path.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as exc:
            raise AcquisitionBatchError(
                f"Item fixity companion is not readable: {item_ref}"
            ) from exc
        if actual_fixity != expected_fixity:
            raise AcquisitionBatchError(
                f"Item fixity companion differs from manifest payloads: {item_ref}"
            )

        rights_ref = selection["rights"]["ref"]
        rights_record = records.get(rights_ref)
        if not isinstance(rights_record, dict) or rights_record.get("kind") != "rights":
            raise AcquisitionBatchError(f"selected rights record is missing: {item_ref}")
        rights_path = _path_under(
            source_root, rights_ref, label="prepared Item rights"
        )
        _regular_file(rights_path, label="prepared Item rights")
        rights_sha256 = _sha256_file(rights_path)
        if rights_sha256 != selection["rights"]["sha256"] or rights_sha256 != rights_record["sha256"]:
            raise SourceIntegrityError(f"prepared rights bytes differ: {item_ref}")
        rights_value = _load_json_bytes(
            rights_path.read_bytes(), label="prepared Item rights"
        )
        validate_contract(
            rights_value,
            RIGHTS_RECORD_SCHEMA,
            label="rights record",
            item_ref=item_ref,
        )
        _verify_declared_rights_posture(selection, rights_value, item_ref=item_ref)
        if rights_value.get("visibility") != item_manifest.get("visibility"):
            raise AcquisitionBatchError(
                f"rights visibility differs from Item manifest visibility: {item_ref}"
            )
        scope_refs = rights_value.get("scope_refs")
        required_scopes = {item_ref, *selected_payloads}
        if not isinstance(scope_refs, list) or not required_scopes <= set(scope_refs):
            raise AcquisitionBatchError(
                f"Item rights scope does not cover selected Item and payloads: {item_ref}"
            )

        provenance_path = _path_under(
            source_root, provenance_ref, label="prepared Item provenance"
        )
        _regular_file(provenance_path, label="prepared Item provenance")
        try:
            provenance_rows = [
                json.loads(line, object_pairs_hook=_strict_pairs)
                for line in provenance_path.read_text(encoding="utf-8").splitlines()
                if line.strip()
            ]
        except (AcquisitionBatchError, OSError, UnicodeError, json.JSONDecodeError) as exc:
            raise AcquisitionBatchError("prepared Item provenance is not valid JSONL") from exc
        if not provenance_rows:
            raise AcquisitionBatchError("prepared Item provenance is empty")
        for index, event in enumerate(provenance_rows, start=1):
            validate_contract(
                event,
                PROVENANCE_EVENT_SCHEMA,
                label=f"provenance event {index}",
                item_ref=item_ref,
            )
        event_ids = [event["event_id"] for event in provenance_rows]
        if len(event_ids) != len(set(event_ids)):
            raise AcquisitionBatchError(
                f"Item provenance contains duplicate event_id: {item_ref}"
            )
        inventory_event_ref = inventory_value.get("provenance_event_ref")
        inventory_events = [
            event
            for event in provenance_rows
            if isinstance(event, dict) and event.get("event_id") == inventory_event_ref
        ]
        if not isinstance(inventory_event_ref, str) or len(inventory_events) != 1:
            raise AcquisitionBatchError(
                f"Item resource inventory provenance event is missing or ambiguous: {item_ref}"
            )
        expected_inventory_output = {
            "ref": inventory_ref,
            "role": "tracked_text_free_resource_inventory",
            "sha256": _sha256_file(inventory_path),
        }
        inventory_outputs = inventory_events[0].get("outputs")
        if (
            not isinstance(inventory_outputs, list)
            or expected_inventory_output not in inventory_outputs
        ):
            raise AcquisitionBatchError(
                f"Item resource inventory provenance output is not digest-bound: {item_ref}"
            )
        acquisition_event_ref = item_manifest.get("acquisition_event_ref")
        selected_events = [
            event
            for event in provenance_rows
            if isinstance(event, dict) and event.get("event_id") == acquisition_event_ref
        ]
        if (
            not isinstance(acquisition_event_ref, str)
            or len(selected_events) != 1
            or selected_events[0].get("event_type") != "acquisition"
            or selected_events[0].get("status")
            not in {"completed", "completed_with_warnings"}
            or selected_events[0].get("rights_basis_ref") != rights_ref
            or not isinstance(selected_events[0].get("outputs"), list)
        ):
            raise AcquisitionBatchError(
                f"Item provenance does not name the selected acquisition event: {item_ref}"
            )
        acquisition_event = selected_events[0]
        for file_ref, payload in selected_payloads.items():
            manifest_payload = manifest_by_file[file_ref]
            for key, expected in {
                "file_id": file_ref,
                "relative_path": payload["relative_path"],
                "byte_size": payload["byte_size"],
                "sha256": payload["sha256"],
                "media_type": payload["media_type"],
            }.items():
                if manifest_payload.get(key) != expected:
                    raise AcquisitionBatchError(
                        f"Item manifest payload binding differs: {file_ref}"
                    )
            destination_ref = f"{item_root}/{payload['relative_path']}"
            if not any(
                isinstance(output_row, dict)
                and isinstance(output_row.get("ref"), str)
                and output_row.get("ref") in {file_ref, destination_ref}
                and output_row.get("sha256") == payload["sha256"]
                for output_row in acquisition_event["outputs"]
            ):
                raise AcquisitionBatchError(
                    f"Item provenance does not bind acquired payload: {file_ref}"
                )


def _verify_prepared_output(
    context: BatchContext,
    output: Path,
    *,
    require_private_roots: bool = True,
) -> None:
    """Recheck the complete sealed selection before resume or handoff.

    Preparation is a local custody boundary, so a receipt flag alone cannot
    establish that its selected rights/metadata and delta bytes still match
    the frozen manifest.  This check deliberately re-reads every selected
    record and the deterministic delta path on each resume.
    """

    output = _checked_root(output)
    if require_private_roots:
        _private_directory(output, label="prepared output root")
        for directory, label in (
            (output / "source", "prepared source root"),
            (output / "payload", "prepared payload root"),
            (output / "receipts", "prepared receipts root"),
        ):
            _private_directory(directory, label=label)
    manifest_path = _output_manifest_path(output)
    _regular_file(manifest_path, label="prepared batch manifest")
    if manifest_path.read_bytes() != context.raw_manifest:
        raise AcquisitionBatchError("prepared output manifest differs from frozen selection")
    preparation_path = output / "receipts/preparation.json"
    _regular_file(preparation_path, label="preparation receipt")
    receipt = _load_json_bytes(preparation_path.read_bytes(), label="preparation receipt")
    if receipt.get("schema_version") != "tos_acquisition_preparation_receipt_v1":
        raise AcquisitionBatchError("preparation receipt has an unexpected schema")
    if receipt.get("batch_id") != context.manifest["batch_id"]:
        raise AcquisitionBatchError("preparation receipt batch differs from frozen selection")
    if receipt.get("batch_revision") != context.manifest["batch_revision"]:
        raise AcquisitionBatchError("preparation receipt revision differs from frozen selection")
    if receipt.get("base_revision") != context.manifest["base_revision"]:
        raise AcquisitionBatchError("preparation receipt base revision differs from frozen selection")
    if receipt.get("manifest_ref") != "manifest.json" or receipt.get("manifest_sha256") != context.manifest_sha256:
        raise AcquisitionBatchError("preparation receipt does not bind frozen selection")
    if receipt.get("topology_preimages") != 0:
        raise AcquisitionBatchError("prepared output contains a topology preimage claim")

    expected_delta_ref = _provenance_delta_ref(context)
    if receipt.get("provenance_delta_ref") != expected_delta_ref:
        raise AcquisitionBatchError("preparation receipt delta reference is not deterministic")
    if not isinstance(receipt.get("provenance_delta_sha256"), str):
        raise AcquisitionBatchError("preparation receipt does not bind provenance delta fixity")

    receipt_records = receipt.get("records")
    if not isinstance(receipt_records, list):
        raise AcquisitionBatchError("preparation receipt records are missing")
    receipt_by_ref: dict[str, dict[str, Any]] = {}
    for row in receipt_records:
        if not isinstance(row, dict) or not isinstance(row.get("ref"), str):
            raise AcquisitionBatchError("preparation receipt has an invalid record row")
        if row["ref"] in receipt_by_ref:
            raise AcquisitionBatchError(f"preparation receipt repeats record: {row['ref']}")
        receipt_by_ref[row["ref"]] = row
    selected_refs = {record["ref"] for _selection, record in _records(context)}
    if set(receipt_by_ref) != selected_refs:
        raise AcquisitionBatchError("preparation receipt does not close over selected records")
    if receipt.get("record_count") != len(selected_refs):
        raise AcquisitionBatchError("preparation receipt record count differs from selection")

    expected_source_refs = set(selected_refs)
    expected_source_refs.add(expected_delta_ref.removeprefix("source/"))
    source_root = output / "source"
    _regular_file(output / "source" / expected_delta_ref.removeprefix("source/"), label="prepared provenance delta")
    actual_source_refs: set[str] = set()
    if source_root.exists() and not source_root.is_symlink():
        for candidate in source_root.rglob("*"):
            info = candidate.lstat()
            if stat.S_ISLNK(info.st_mode):
                raise AcquisitionBatchError(f"prepared source contains a symlink: {candidate}")
            if stat.S_ISDIR(info.st_mode):
                continue
            if stat.S_ISREG(info.st_mode):
                actual_source_refs.add(candidate.relative_to(source_root).as_posix())
            else:
                raise AcquisitionBatchError(
                    f"prepared source contains a special file: {candidate}"
                )
    if actual_source_refs != expected_source_refs:
        extra = sorted(actual_source_refs - expected_source_refs)
        missing = sorted(expected_source_refs - actual_source_refs)
        raise AcquisitionBatchError(
            "prepared source closure differs from frozen selection"
            + (f"; extra={extra[:3]}" if extra else "")
            + (f"; missing={missing[:3]}" if missing else "")
        )

    _verify_prepared_item_bindings(context, output)

    expected_bytes = 0
    for selection, record in _records(context):
        source = _path_under(source_root, record["ref"], label="prepared selected metadata path")
        info = _regular_file(source, label="prepared selected metadata record")
        if stat.S_IMODE(info.st_mode) != 0o644:
            raise AcquisitionBatchError(
                f"prepared selected source mode is unsupported for candidate update: {record['ref']}"
            )
        actual_sha = _sha256_file(source)
        if actual_sha != record["sha256"]:
            raise SourceIntegrityError(f"prepared selected record digest differs: {record['ref']}")
        row = receipt_by_ref[record["ref"]]
        expected_row = {
            "ref": record["ref"],
            "handoff_ref": f"source/{record['ref']}",
            "kind": record["kind"],
            "sha256": record["sha256"],
            "byte_size": info.st_size,
        }
        for key, expected in expected_row.items():
            if row.get(key) != expected:
                raise AcquisitionBatchError(
                    f"preparation receipt {key} differs for selected record: {record['ref']}"
                )
        expected_bytes += info.st_size
        if selection["rights"]["ref"] == record["ref"] and selection["rights"]["sha256"] != actual_sha:
            raise SourceIntegrityError(f"selected rights record digest differs: {record['ref']}")
    if receipt.get("selected_record_bytes") != expected_bytes:
        raise AcquisitionBatchError("preparation receipt byte total differs from selected records")

    delta_path = _path_under(output, expected_delta_ref, label="provenance delta reference")
    delta_bytes = delta_path.read_bytes()
    if _sha256(delta_bytes) != receipt["provenance_delta_sha256"]:
        raise SourceIntegrityError("prepared provenance delta digest differs from receipt")
    expected_delta_bytes = _canonical(_provenance_delta(context))
    if delta_bytes != expected_delta_bytes:
        raise SourceIntegrityError("prepared provenance delta differs from frozen selection")
    delta_schema_path = context.repo_root / PROVENANCE_DELTA_SCHEMA
    delta_schema = _load_json_bytes(delta_schema_path.read_bytes(), label="acquisition provenance delta schema")
    try:
        delta_value = _load_json_bytes(delta_bytes, label="prepared provenance delta")
        Draft202012Validator(delta_schema).validate(delta_value)
    except Exception as exc:
        if isinstance(exc, AcquisitionBatchError):
            raise
        raise AcquisitionBatchError(f"prepared provenance delta schema validation failed: {exc}") from exc


def _incomplete_prepare_is_route_owned(output: Path, context: BatchContext) -> bool:
    """Recognize only a manifest-owned partial preparation.

    A missing preparation receipt is not enough to prove ownership.  Recovery
    may remove only a directory whose immutable manifest matches this exact
    invocation, whose source tree contains only selected record names (or the
    deterministic delta), and whose payload/receipt trees are still empty.
    Existing payload or receipt evidence therefore fails closed instead of
    being recursively deleted.
    """

    if not output.is_dir() or output.is_symlink():
        return False
    allowed = {"manifest.json", "source", "payload", "receipts"}
    for child in output.iterdir():
        if child.name not in allowed or child.is_symlink():
            return False
        if child.is_file() and child.name != "manifest.json":
            return False
        if child.is_dir() and child.name not in {"source", "payload", "receipts"}:
            return False
    try:
        manifest_path = _output_manifest_path(output)
        _regular_file(manifest_path, label="partial preparation manifest")
        if manifest_path.read_bytes() != context.raw_manifest:
            return False
        expected_source_refs = {
            record["ref"] for _selection, record in _records(context)
        }
        expected_source_refs.add(
            _provenance_delta_ref(context).removeprefix("source/")
        )
        source_root = output / "source"
        payload_root = output / "payload"
        receipts_root = output / "receipts"
        for root in (source_root, payload_root, receipts_root):
            if not root.is_dir() or root.is_symlink():
                return False
        source_refs: set[str] = set()
        for candidate in source_root.rglob("*"):
            if candidate.is_symlink():
                return False
            if candidate.is_file():
                source_refs.add(candidate.relative_to(source_root).as_posix())
            elif not candidate.is_dir():
                return False
        if not source_refs <= expected_source_refs:
            return False
        if any(payload_root.rglob("*")) or any(receipts_root.rglob("*")):
            return False
    except (AcquisitionBatchError, OSError):
        return False
    return True


def _recover_incomplete_prepare(output: Path, context: BatchContext) -> None:
    """Remove one clearly route-owned partial preparation for exact rebuild."""

    if not _incomplete_prepare_is_route_owned(output, context):
        raise AcquisitionBatchError(
            "output exists without preparation receipt and is not a recoverable interrupted preparation"
        )
    try:
        shutil.rmtree(output)
    except OSError as exc:
        raise AcquisitionBatchError(f"cannot recover interrupted preparation: {output}") from exc


@contextmanager
def _batch_execution_lock(output: Path):
    """Serialize all prepare/acquire mutations for one output root."""

    parent = output.parent
    if not parent.is_dir() or parent.is_symlink():
        raise AcquisitionBatchError(f"batch lock parent must be a regular directory: {parent}")
    lock_path = parent / f".{output.name}.acquisition.lock"
    if lock_path.is_symlink():
        raise AcquisitionBatchError(f"batch lock may not be a symlink: {lock_path}")
    try:
        stream = lock_path.open("a+", encoding="utf-8")
    except OSError as exc:
        raise AcquisitionBatchError(f"cannot open batch lock: {lock_path}") from exc
    try:
        fcntl.flock(stream.fileno(), fcntl.LOCK_EX)
        yield
    finally:
        fcntl.flock(stream.fileno(), fcntl.LOCK_UN)
        stream.close()


def _fetch_url(payload: dict[str, Any]) -> bytes:
    try:
        request = Request(
            payload["provider_url"],
            headers={"User-Agent": "Tree-of-Sophia-bounded-acquisition/1"},
        )
        with urlopen(request, timeout=45) as response:
            body = bytearray()
            while True:
                block = response.read(min(1024 * 1024, payload["byte_size"] + 1 - len(body)))
                if not block:
                    break
                body.extend(block)
                if len(body) > payload["byte_size"]:
                    break
    except (OSError, HTTPException, ValueError) as exc:
        raise SourceFetchError(f"provider fetch failed for {payload['file_ref']}: {exc}") from exc
    return bytes(body)


def _expected_file_digest(payload: dict[str, Any], body: bytes) -> custody.FileDigest:
    expected_git = payload.get("git_blob_sha1") or _git_blob_sha1(body)
    return custody.FileDigest(payload["byte_size"], payload["sha256"], expected_git)


def _verify_destination(
    path: Path,
    payload: dict[str, Any],
    *,
    expected_owner_uid: int | None = None,
) -> custody.FileDigest:
    try:
        digest = custody.digest_file(
            path,
            expected_mode=0o444,
            expected_owner_uid=expected_owner_uid,
            require_single_link=True,
        )
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
    flags = os.O_WRONLY | os.O_CREAT | os.O_APPEND | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    flags |= getattr(os, "O_NONBLOCK", 0)
    try:
        descriptor = os.open(path, flags, 0o644)
    except OSError as exc:
        if path.is_symlink():
            raise AcquisitionBatchError(f"acquisition journal may not be a symlink: {path}") from exc
        raise AcquisitionBatchError(f"cannot open acquisition journal: {path}") from exc
    try:
        with os.fdopen(descriptor, "a", encoding="utf-8", closefd=True) as stream:
            journal_stat = os.fstat(stream.fileno())
            if not stat.S_ISREG(journal_stat.st_mode):
                raise AcquisitionBatchError(
                    f"acquisition journal is not a regular file: {path}"
                )
            if journal_stat.st_uid != os.geteuid():
                raise AcquisitionBatchError(
                    f"acquisition journal owner differs from current user: {path}"
                )
            if journal_stat.st_nlink != 1:
                raise AcquisitionBatchError(
                    f"acquisition journal must have one hard link: {path}"
                )
            fcntl.flock(stream.fileno(), fcntl.LOCK_EX)
            stream.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")
            stream.flush()
            os.fsync(stream.fileno())
            fcntl.flock(stream.fileno(), fcntl.LOCK_UN)
    except OSError as exc:
        raise AcquisitionBatchError(f"cannot append acquisition journal: {path}") from exc


def _read_jsonl_rows(
    path: Path,
    *,
    label: str,
    require_current_owner: bool,
    require_single_link: bool,
) -> list[dict[str, Any]]:
    if path.is_symlink():
        raise AcquisitionBatchError(f"{label} may not be a symlink: {path}")
    if not path.exists():
        return []
    rows: list[dict[str, Any]] = []
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0)
    flags |= getattr(os, "O_NONBLOCK", 0)
    try:
        descriptor = os.open(path, flags)
        with os.fdopen(descriptor, "r", encoding="utf-8", closefd=True) as stream:
            journal_stat = os.fstat(stream.fileno())
            if not stat.S_ISREG(journal_stat.st_mode):
                raise AcquisitionBatchError(
                    f"{label} is not a regular file: {path}"
                )
            if require_current_owner and journal_stat.st_uid != os.geteuid():
                raise AcquisitionBatchError(
                    f"{label} owner differs from current user: {path}"
                )
            if require_single_link and journal_stat.st_nlink != 1:
                raise AcquisitionBatchError(
                    f"{label} must have one hard link: {path}"
                )
            for line_number, line in enumerate(stream, 1):
                if not line.strip():
                    continue
                try:
                    value = json.loads(line, object_pairs_hook=_strict_pairs)
                except (AcquisitionBatchError, json.JSONDecodeError) as exc:
                    raise AcquisitionBatchError(f"{label} is malformed at line {line_number}") from exc
                if not isinstance(value, dict):
                    raise AcquisitionBatchError(f"{label} rows must be objects")
                rows.append(value)
    except OSError as exc:
        if path.is_symlink():
            raise AcquisitionBatchError(f"{label} may not be a symlink: {path}") from exc
        raise AcquisitionBatchError(f"cannot read {label}: {path}") from exc
    return rows


def _journal_rows(path: Path) -> list[dict[str, Any]]:
    return _read_jsonl_rows(
        path,
        label="acquisition journal",
        require_current_owner=True,
        require_single_link=True,
    )


def _portable_jsonl_rows(path: Path, *, label: str) -> list[dict[str, Any]]:
    """Read immutable handoff JSONL without local-journal owner constraints."""

    return _read_jsonl_rows(
        path,
        label=label,
        require_current_owner=False,
        require_single_link=False,
    )


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
            destination = custody.payload_path(
                output / "payload", payload["item_root_ref"], payload["relative_path"]
            )
            digest = _verify_destination(
                destination, payload, expected_owner_uid=os.geteuid()
            )
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
    verified = {
        key
        for row in fixity_rows
        if row.get("status") == "verified"
        if (key := _payload_custody_key(row)) is not None
    }
    expected = {item.custody_key for item in _payloads(context)}
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
            "jsonl_sha256": _sha256_file(output / fixity_ref),
            "summary_sha256": _sha256_file(output / fixity_summary_ref),
        },
        "provenance_delta": {
            "ref": _provenance_delta_ref(context),
            "sha256": _sha256_file(
                _path_under(output, _provenance_delta_ref(context), label="provenance delta reference")
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


def _acquire_batch_unlocked(
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
    if output.exists() and output.is_symlink():
        raise AcquisitionBatchError(f"output root may not be a symlink: {output}")
    preparation_path = output / "receipts/preparation.json"
    if output.exists() and not preparation_path.is_file():
        _recover_incomplete_prepare(output, context)
    if not output.exists():
        _prepare_batch_unlocked(
            context=context,
            metadata_root=metadata_root,
            output_root=output,
        )
    _verify_prepared_output(context, output, require_private_roots=True)
    receipts_root = _checked_root(output / "receipts")
    journal_path = receipts_root / "acquisition.jsonl"
    existing = _journal_rows(journal_path)
    previous_attempts: dict[tuple[str, str, str], int] = {}
    for row in existing:
        attempt = row.get("attempt")
        if type(attempt) is not int or attempt < 0:
            raise AcquisitionBatchError(
                "acquisition journal attempt must be a nonnegative integer"
            )
        key = _payload_custody_key(row)
        if key is not None:
            previous_attempts[key] = max(previous_attempts.get(key, 0), attempt)
    fetch = fetcher or _fetch_url
    run_id = _run_id(receipts_root)
    payload_rows: list[dict[str, Any]] = []
    for item in _payloads(context):
        payload = item.payload
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
            destination = custody.payload_path(
                output / "payload", payload["item_root_ref"], payload["relative_path"]
            )
        except custody.CustodyError as exc:
            row = {
                **base_row,
                "attempt": previous_attempts.get(item.custody_key, 0),
                "status": "conflict",
                "error": str(exc),
                "failure_type": type(exc).__name__,
                "completed_at": utc_now(),
            }
            _append_journal(journal_path, row)
            payload_rows.append(row)
            continue
        try:
            if destination.exists() or destination.is_symlink():
                if destination.is_symlink():
                    raise SourceIntegrityError(f"destination is a symlink: {destination}")
                _verify_destination(
                    destination, payload, expected_owner_uid=os.geteuid()
                )
                row = {**base_row, "attempt": previous_attempts.get(item.custody_key, 0), "status": "already_present", "completed_at": utc_now()}
                _append_journal(journal_path, row)
                payload_rows.append(row)
                continue
        except SourceIntegrityError as exc:
            row = {**base_row, "attempt": previous_attempts.get(item.custody_key, 0), "status": "conflict", "error": str(exc), "completed_at": utc_now()}
            _append_journal(journal_path, row)
            payload_rows.append(row)
            continue

        completed: dict[str, Any] | None = None
        for local_attempt in range(1, max_attempts + 1):
            attempt = previous_attempts.get(item.custody_key, 0) + local_attempt
            try:
                body = fetch(payload)
                if not isinstance(body, bytes):
                    raise SourceFetchError("fetcher did not return bytes")
                if len(body) != payload["byte_size"] or _sha256(body) != payload["sha256"]:
                    raise SourceIntegrityError(f"provider bytes differ for {item.file_ref}")
                expected = _expected_file_digest(payload, body)
                status = custody.publish_bytes_no_clobber(destination, body, expected)
                _verify_destination(
                    destination, payload, expected_owner_uid=os.geteuid()
                )
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

    # The source closure may be edited while a provider call is in flight.
    # Recheck it immediately before creating any intake-facing receipt.
    _verify_prepared_output(context, output, require_private_roots=True)
    fixity_rows, fixity_ref, fixity_summary_ref = _fixity_receipt(context, output, run_id)
    _verify_prepared_output(context, output, require_private_roots=True)
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
    """Serialize one batch's preparation, transfer, and handoff."""

    output = Path(output_root).expanduser()
    if not output.is_absolute():
        raise AcquisitionBatchError("output root must be absolute")
    with _batch_execution_lock(output):
        return _acquire_batch_unlocked(
            manifest_path=manifest_path,
            metadata_root=metadata_root,
            output_root=output,
            repo_root=repo_root,
            expected_manifest_sha256=expected_manifest_sha256,
            fetcher=fetcher,
            max_attempts=max_attempts,
        )


def verify_local(*, output_root: Path | str, repo_root: Path | str = REPO_ROOT) -> dict[str, Any]:
    """Perform an independent local fixity pass without changing custody."""

    output = _checked_root(output_root)
    manifest_path = _output_manifest_path(output)
    context = load_manifest(manifest_path, repo_root=repo_root)
    _verify_prepared_output(context, output, require_private_roots=True)
    rows: list[dict[str, Any]] = []
    for item in _payloads(context):
        payload = item.payload
        row = {
            "item_ref": item.item_ref,
            "file_ref": item.file_ref,
            "destination_ref": item.destination_ref,
        }
        try:
            destination = custody.payload_path(
                output / "payload", payload["item_root_ref"], payload["relative_path"]
            )
            digest = _verify_destination(
                destination, payload, expected_owner_uid=os.geteuid()
            )
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

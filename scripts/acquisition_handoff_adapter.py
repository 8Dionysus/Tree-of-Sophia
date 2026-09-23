#!/usr/bin/env python3
"""Turn one bounded acquisition handoff into an explicit corpus-batch input.

The adapter is the narrow consumer seam for ``receipts/handoff-*.json``.  It
rechecks the handoff's manifest, selected record closure, provenance delta,
payload fixity, and status before it creates a private ``tos_corpus_batch_v1``
input root.  It does not admit a corpus revision or mutate an accepted store.
The caller also supplies the full validation context used for the selected
validator identity; that context is verified and carried into the receipt for
the existing corpus-admission command.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import tempfile
from typing import Any

try:
    import acquisition_batch as acquisition
    import corpus_admit
    import corpus_store
    import source_payload_custody as custody
except ModuleNotFoundError as exc:  # pragma: no cover - direct package import
    if exc.name not in {"acquisition_batch", "corpus_admit", "corpus_store", "source_payload_custody"}:
        raise
    from scripts import acquisition_batch as acquisition
    from scripts import corpus_admit
    from scripts import corpus_store
    from scripts import source_payload_custody as custody


HEX64 = re.compile(r"^[a-f0-9]{64}$")


class HandoffAdapterError(ValueError):
    """The selected acquisition handoff cannot form a closed batch input."""


@dataclass(frozen=True)
class VerifiedHandoff:
    """Verified acquired handoff data for the intake closure owner.

    This result intentionally contains no accepted-store object or corpus
    batch.  Intake can use the verified source/payload rows with its one
    streamed accepted index before constructing additive topology successors.
    """

    root: Path
    handoff_path: Path
    handoff: dict[str, Any]
    context: acquisition.BatchContext
    selected_source_rows: list[dict[str, Any]]
    payloads: list[dict[str, Any]]


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
        raise HandoffAdapterError(f"cannot render canonical JSON: {exc}") from exc


def _sha256(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as exc:
        raise HandoffAdapterError(f"cannot read file: {path}") from exc
    return digest.hexdigest()


def _regular(path: Path, *, label: str) -> os.stat_result:
    try:
        if path.is_symlink() or path.resolve(strict=True) != path.absolute():
            raise HandoffAdapterError(f"{label} may not be a symlink: {path}")
        info = path.stat()
    except HandoffAdapterError:
        raise
    except OSError as exc:
        raise HandoffAdapterError(f"{label} is not readable: {path}") from exc
    if not stat.S_ISREG(info.st_mode):
        raise HandoffAdapterError(f"{label} is not a regular file: {path}")
    return info


def _path_under(root: Path, ref: str, *, label: str) -> Path:
    try:
        parts = acquisition._safe_ref(ref, label=label)
    except acquisition.AcquisitionBatchError as exc:
        raise HandoffAdapterError(str(exc)) from exc
    candidate = root.joinpath(*parts.parts)
    try:
        candidate.resolve(strict=False).relative_to(root.resolve())
    except ValueError as exc:
        raise HandoffAdapterError(f"{label} escapes its root: {ref}") from exc
    for ancestor in (candidate, *candidate.parents):
        if ancestor == root.parent:
            break
        if ancestor.is_symlink():
            raise HandoffAdapterError(f"symlink in {label}: {ancestor}")
    return candidate


def _copy_no_clobber(source: Path, destination: Path, *, sha256: str, byte_size: int) -> None:
    """Copy one already-verified source member with an exclusive final link."""

    _regular(source, label="handoff source")
    if _sha256_file(source) != sha256 or source.stat().st_size != byte_size:
        raise HandoffAdapterError(f"handoff source fixity differs: {source}")
    if destination.exists() or destination.is_symlink():
        if destination.is_symlink() or not destination.is_file():
            raise HandoffAdapterError(f"candidate destination conflicts: {destination}")
        if destination.stat().st_size != byte_size or _sha256_file(destination) != sha256:
            raise HandoffAdapterError(f"candidate destination fixity differs: {destination}")
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.", suffix=".tmp", dir=destination.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb", closefd=True) as stream, source.open("rb") as input_stream:
            shutil.copyfileobj(input_stream, stream, 1024 * 1024)
            stream.flush()
            os.fsync(stream.fileno())
        if temporary.stat().st_size != byte_size or _sha256_file(temporary) != sha256:
            raise HandoffAdapterError(f"candidate temporary copy differs: {source}")
        try:
            os.link(temporary, destination)
        except FileExistsError:
            if destination.is_symlink() or _sha256_file(destination) != sha256:
                raise HandoffAdapterError(f"candidate destination race: {destination}")
            return
        os.chmod(destination, 0o644)
    finally:
        temporary.unlink(missing_ok=True)


def _provenance_delta_ref(context: acquisition.BatchContext) -> str:
    return acquisition._provenance_delta_ref(context)


def _load_handoff(root: Path, handoff_ref: str) -> tuple[Path, dict[str, Any]]:
    path = _path_under(root, handoff_ref, label="handoff reference")
    _regular(path, label="handoff receipt")
    try:
        value = acquisition._load_json_bytes(path.read_bytes(), label="handoff receipt")
    except acquisition.AcquisitionBatchError as exc:
        raise HandoffAdapterError(str(exc)) from exc
    if value.get("schema_version") != "tos_acquisition_handoff_v1":
        raise HandoffAdapterError("handoff has an unexpected schema")
    return path, value


def _verify_accepted_pointer(store_root: Path, expected_revision: str) -> None:
    pointer_path = _path_under(store_root, "current.json", label="accepted corpus pointer")
    _regular(pointer_path, label="accepted corpus pointer")
    try:
        pointer = corpus_store.read_json(pointer_path)
        if not isinstance(pointer, dict):
            raise HandoffAdapterError("accepted corpus pointer is not an object")
        if pointer.get("previous") is not None:
            corpus_store.hex_digest(pointer["previous"])
    except (HandoffAdapterError, corpus_store.CorpusStoreError, OSError) as exc:
        raise HandoffAdapterError(str(exc)) from exc
    if (
        set(pointer) != {"schema_version", "current", "previous"}
        or pointer.get("schema_version") != "tos_corpus_pointer_v1"
        or pointer.get("current") != expected_revision
        or not isinstance(pointer.get("previous"), (str, type(None)))
    ):
        raise HandoffAdapterError("accepted corpus pointer is not the selected base revision")


def _load_accepted_manifest(store_root: Path, expected_revision: str) -> dict[str, Any]:
    """Load the selected accepted snapshot and bind it to its revision.

    The accepted source view is only meaningful with the exact immutable
    snapshot that the pointer names.  Reading this manifest validates its
    canonical body, revision digest, member ordering and indexes without
    rehashing the unrelated object store.
    """

    try:
        # ``CorpusStore`` normally creates its backing directories.  The
        # adapter is a read-only consumer of the selected base, so load the
        # existing store without invoking that mutating constructor path.
        store = object.__new__(corpus_store.CorpusStore)
        store.root = store_root
        manifest = store.load(expected_revision)
    except (corpus_store.CorpusStoreError, OSError) as exc:
        raise HandoffAdapterError(
            f"accepted corpus manifest is not the selected base revision: {exc}"
        ) from exc
    if manifest.get("revision") != expected_revision:
        raise HandoffAdapterError("accepted corpus manifest revision differs from pointer")
    return manifest


def _validation_context_binding(
    value: dict[str, Any] | None,
    *,
    validator_sha256: str,
) -> dict[str, Any]:
    """Verify and freeze the caller-selected admission context.

    A validator digest is not enough to reconstruct an admission invocation:
    historical captures and their restored roots are part of
    ``SourceValidator.sha256``.  Keep those exact inputs with the adapter
    receipt so intake can invoke the existing ``corpus_admit`` flags without
    rediscovering or silently replacing the validation context.
    """

    if not isinstance(value, dict):
        raise HandoffAdapterError("explicit validation context is required")
    expected_keys = {"schema_version", "validator_sha256", "grammar_root_ref", "historical_evidence"}
    if set(value) != expected_keys or value.get("schema_version") != "tos_corpus_validation_context_v1":
        raise HandoffAdapterError("validation context has an unexpected schema")
    if value.get("validator_sha256") != validator_sha256:
        raise HandoffAdapterError("validation context validator identity differs from selected batch")
    grammar_ref = value.get("grammar_root_ref")
    if not isinstance(grammar_ref, str):
        raise HandoffAdapterError("validation context grammar root is missing")
    grammar_root = Path(grammar_ref).expanduser()
    try:
        grammar_root = acquisition._checked_root(grammar_root)
    except acquisition.AcquisitionBatchError as exc:
        raise HandoffAdapterError(f"validation context grammar root is invalid: {exc}") from exc

    evidence = value.get("historical_evidence")
    if not isinstance(evidence, list):
        raise HandoffAdapterError("validation context historical evidence must be a list")
    binding_rows: list[dict[str, Any]] = []
    seen_captures: set[str] = set()
    seen_roots: set[str] = set()
    try:
        from corpus_archive import verify_capture
    except ModuleNotFoundError as exc:  # pragma: no cover - package import fallback
        if exc.name != "corpus_archive":
            raise
        from scripts.corpus_archive import verify_capture
    for row in evidence:
        if not isinstance(row, dict) or set(row) != {"capture_ref", "restored_root_ref"}:
            raise HandoffAdapterError("historical validation evidence needs capture and restored root")
        capture_ref = row["capture_ref"]
        restored_ref = row["restored_root_ref"]
        if not isinstance(capture_ref, str) or not isinstance(restored_ref, str):
            raise HandoffAdapterError("historical validation evidence references are invalid")
        capture = Path(capture_ref).expanduser()
        restored = Path(restored_ref).expanduser()
        try:
            capture = acquisition._checked_root(capture)
            restored = acquisition._checked_root(restored)
        except acquisition.AcquisitionBatchError as exc:
            raise HandoffAdapterError(f"historical validation evidence root is invalid: {exc}") from exc
        capture_key = str(capture)
        restored_key = str(restored)
        if capture_key in seen_captures or restored_key in seen_roots:
            raise HandoffAdapterError("historical validation evidence overlaps another pack")
        seen_captures.add(capture_key)
        seen_roots.add(restored_key)
        try:
            capture_manifest = verify_capture(capture)
        except Exception as exc:
            raise HandoffAdapterError(f"historical capture verification failed: {capture}") from exc
        capture_manifest_path = capture / "capture.json"
        members_path = capture / "members.jsonl"
        archive_path = capture / "source.tar.gz"
        restore_receipt_path = restored / "restore-receipt.json"
        _regular(capture_manifest_path, label="historical capture manifest")
        _regular(members_path, label="historical capture members")
        _regular(archive_path, label="historical capture archive")
        _regular(restore_receipt_path, label="historical restore receipt")
        try:
            restore_receipt = corpus_store.read_json(restore_receipt_path)
        except (corpus_store.CorpusStoreError, OSError) as exc:
            raise HandoffAdapterError("historical restore receipt is not canonical") from exc
        if (
            not isinstance(restore_receipt, dict)
            or set(restore_receipt)
            != {
                "schema_version",
                "source_git_commit",
                "member_count",
                "source_bytes",
                "manifest_sha256",
            }
            or restore_receipt.get("schema_version") != "tos_corpus_restore_receipt_v1"
            or restore_receipt.get("source_git_commit") != capture_manifest.get("source_git_commit")
            or restore_receipt.get("member_count") != capture_manifest.get("member_count")
            or restore_receipt.get("source_bytes") != capture_manifest.get("source_bytes")
            or restore_receipt.get("manifest_sha256") != _sha256_file(capture_manifest_path)
        ):
            raise HandoffAdapterError("historical restore receipt does not bind its capture")
        binding_rows.append(
            {
                "capture_ref": capture_key,
                "capture_manifest_sha256": _sha256_file(capture_manifest_path),
                "members_sha256": _sha256_file(members_path),
                "archive_sha256": capture_manifest["archive_sha256"],
                "restored_root_ref": restored_key,
                "restore_receipt_ref": str(restore_receipt_path),
                "restore_receipt_sha256": _sha256_file(restore_receipt_path),
            }
        )
    admission_flags: dict[str, Any] = {"grammar_root": str(grammar_root)}
    if binding_rows:
        admission_flags.update(
            historical_capture=[row["capture_ref"] for row in binding_rows],
            historical_root=[row["restored_root_ref"] for row in binding_rows],
        )
    binding = {
        "schema_version": "tos_corpus_validation_context_v1",
        "validator_sha256": validator_sha256,
        "grammar_root_ref": str(grammar_root),
        "historical_evidence": binding_rows,
        "admission_flags": admission_flags,
    }
    return binding


def verify_validation_context(
    value: dict[str, Any] | None,
    *,
    validator_sha256: str,
) -> dict[str, Any]:
    """Verify the caller-selected grammar and historical admission context."""

    return _validation_context_binding(value, validator_sha256=validator_sha256)


def _expected_records(context: acquisition.BatchContext) -> dict[str, tuple[dict[str, Any], dict[str, Any]]]:
    return {record["ref"]: (selection, record) for selection, record in acquisition._records(context)}


def _expected_payloads(
    context: acquisition.BatchContext,
) -> dict[tuple[str, str, str], acquisition.PayloadSelection]:
    return {item.custody_key: item for item in acquisition._payloads(context)}


def _verify_item_payload_bindings(context: acquisition.BatchContext, acquisition_root: Path) -> None:
    """Expose the producer's Item/File closure under adapter error semantics."""

    try:
        acquisition._verify_prepared_item_bindings(context, acquisition_root)
    except (acquisition.AcquisitionBatchError, acquisition.SourceIntegrityError) as exc:
        raise HandoffAdapterError(str(exc)) from exc


def _load_caller_selected_context(
    *,
    acquisition_root: Path,
    handoff: dict[str, Any],
    expected_manifest_sha256: str,
    repo_root: Path | str,
) -> acquisition.BatchContext:
    """Load only the manifest selected by a caller-held digest.

    The handoff and the acquisition root travel together and therefore cannot
    authenticate one another.  A caller must retain the manifest digest from
    selection/preparation and pass it independently to every consumer.
    """

    if not isinstance(expected_manifest_sha256, str) or not HEX64.fullmatch(
        expected_manifest_sha256
    ):
        raise HandoffAdapterError(
            "caller-selected manifest digest must be lowercase SHA-256"
        )
    input_selection = handoff.get("input_selection")
    if not isinstance(input_selection, dict) or input_selection.get("ref") != "manifest.json":
        raise HandoffAdapterError("handoff input selection must name manifest.json")
    if input_selection.get("sha256") != expected_manifest_sha256:
        raise HandoffAdapterError(
            "handoff manifest digest differs from caller-selected digest"
        )
    manifest_path = _path_under(
        acquisition_root, "manifest.json", label="handoff manifest"
    )
    try:
        return acquisition.load_manifest(
            manifest_path,
            repo_root=repo_root,
            expected_sha256=expected_manifest_sha256,
        )
    except acquisition.AcquisitionBatchError as exc:
        raise HandoffAdapterError(str(exc)) from exc


def _verify_handoff(
    *,
    acquisition_root: Path,
    handoff_path: Path,
    handoff: dict[str, Any],
    context: acquisition.BatchContext,
    expected_base_revision: str,
    accepted_source_root: Path | None,
    accepted_manifest: dict[str, Any] | None,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    if handoff.get("batch_id") != context.manifest["batch_id"]:
        raise HandoffAdapterError("handoff batch differs from its manifest")
    if handoff.get("batch_revision") != context.manifest["batch_revision"]:
        raise HandoffAdapterError("handoff revision differs from its manifest")
    if handoff.get("base_revision") != expected_base_revision:
        raise HandoffAdapterError("handoff base revision differs from selected accepted base")
    if handoff.get("base_revision") != context.manifest["base_revision"]:
        raise HandoffAdapterError("handoff base revision differs from batch manifest")
    if handoff.get("acquisition_status") != "acquired-not-admitted":
        raise HandoffAdapterError("handoff is not a complete acquired-not-admitted transfer")
    if handoff.get("admission_status") != "not-admitted" or handoff.get("publication_status") != "not-published":
        raise HandoffAdapterError("handoff crosses the acquisition authority boundary")
    if handoff.get("topology_preimages") != 0:
        raise HandoffAdapterError("handoff contains a topology preimage claim")
    run_id = handoff.get("run_id")
    if not isinstance(run_id, str) or not re.fullmatch(
        r"[0-9]{8}T[0-9]{6}Z(?:-[0-9]+)?", run_id
    ):
        raise HandoffAdapterError("handoff run identity is invalid")
    if handoff_path.name != f"handoff-{run_id}.json":
        raise HandoffAdapterError("handoff path is not bound to its run identity")
    input_selection = handoff.get("input_selection")
    if not isinstance(input_selection, dict) or input_selection.get("ref") != "manifest.json":
        raise HandoffAdapterError("handoff does not select manifest.json")
    if input_selection.get("sha256") != context.manifest_sha256:
        raise HandoffAdapterError("handoff manifest digest differs")

    if (accepted_source_root is None) != (accepted_manifest is None):
        raise HandoffAdapterError("accepted source view and manifest must be supplied together")
    accepted_files = (
        {entry["path"]: entry for entry in accepted_manifest.get("files", [])}
        if accepted_manifest is not None
        else None
    )
    expected_records = _expected_records(context)
    source_rows = handoff.get("source_records")
    if (
        not isinstance(source_rows, list)
        or len(source_rows) != len(expected_records)
        or {row.get("ref") for row in source_rows if isinstance(row, dict)} != set(expected_records)
    ):
        raise HandoffAdapterError("handoff source record closure differs from manifest")
    selected_source_rows: list[dict[str, Any]] = []
    for row in source_rows:
        if not isinstance(row, dict):
            raise HandoffAdapterError("handoff source record row is not an object")
        ref = row.get("ref")
        if ref not in expected_records:
            raise HandoffAdapterError(f"handoff contains an unselected source record: {ref}")
        selection, record = expected_records[ref]
        if row.get("item_ref") != selection["item_ref"] or row.get("kind") != record["kind"]:
            raise HandoffAdapterError(f"handoff source identity differs: {ref}")
        if row.get("handoff_ref") != f"source/{ref}" or row.get("sha256") != record["sha256"]:
            raise HandoffAdapterError(f"handoff source digest binding differs: {ref}")
        if row.get("rights_ref") != selection["rights"]["ref"] or row.get("rights_sha256") != selection["rights"]["sha256"]:
            raise HandoffAdapterError(f"handoff rights binding differs: {ref}")
        source = _path_under(acquisition_root, row["handoff_ref"], label="handoff source reference")
        info = _regular(source, label="handoff selected source")
        if stat.S_IMODE(info.st_mode) != 0o644:
            raise HandoffAdapterError(
                f"selected source mode is unsupported for candidate update: {ref}"
            )
        if info.st_size != row.get("byte_size"):
            raise HandoffAdapterError(f"handoff source size differs: {ref}")
        if _sha256_file(source) != record["sha256"]:
            raise HandoffAdapterError(f"handoff source bytes differ: {ref}")
        if accepted_files is not None:
            accepted_entry = accepted_files.get(ref)
            accepted = _path_under(accepted_source_root, ref, label="accepted source reference")
            if accepted_entry is None:
                if accepted.exists() or accepted.is_symlink():
                    raise HandoffAdapterError(
                        f"accepted source view contains a path absent from selected base manifest: {ref}"
                    )
            else:
                if accepted_entry["sha256"] != record["sha256"]:
                    raise HandoffAdapterError(f"handoff would replace accepted source bytes: {ref}")
                if accepted_entry.get("mode") != 0o644:
                    raise HandoffAdapterError(
                        f"accepted source mode is unsupported for candidate update: {ref}"
                    )
                if not accepted.exists() or accepted.is_symlink():
                    raise HandoffAdapterError(
                        f"accepted source view is missing selected base member: {ref}"
                    )
                info = _regular(accepted, label="accepted base source")
                if info.st_size != accepted_entry["size_bytes"] or _sha256_file(accepted) != accepted_entry["sha256"]:
                    raise HandoffAdapterError(f"accepted source view differs from base manifest: {ref}")
        selected_source_rows.append(row)

    provenance = handoff.get("provenance_delta")
    expected_delta_ref = _provenance_delta_ref(context)
    if not isinstance(provenance, dict) or provenance.get("ref") != expected_delta_ref:
        raise HandoffAdapterError("handoff provenance delta reference is not deterministic")
    delta = _path_under(acquisition_root, expected_delta_ref, label="handoff provenance delta")
    _regular(delta, label="handoff provenance delta")
    if provenance.get("sha256") != _sha256_file(delta):
        raise HandoffAdapterError("handoff provenance delta digest differs")
    if provenance.get("event_ref") != context.manifest["provenance_delta"]["event_ref"]:
        raise HandoffAdapterError("handoff provenance event differs")
    if provenance.get("base_revision") != context.manifest["base_revision"]:
        raise HandoffAdapterError("handoff provenance delta base differs from batch")
    if provenance.get("base_revision") != expected_base_revision:
        raise HandoffAdapterError("handoff provenance delta base differs from selected accepted base")

    fixity = handoff.get("independent_fixity")
    if not isinstance(fixity, dict):
        raise HandoffAdapterError("handoff independent fixity is missing")
    fixity_ref = fixity.get("ref")
    summary_ref = fixity.get("summary_ref")
    if not isinstance(fixity_ref, str) or not isinstance(summary_ref, str):
        raise HandoffAdapterError("handoff fixity references are missing")
    expected_fixity_ref = f"receipts/fixity-{run_id}.jsonl"
    expected_summary_ref = f"receipts/fixity-{run_id}.json"
    if fixity_ref != expected_fixity_ref or summary_ref != expected_summary_ref:
        raise HandoffAdapterError("handoff fixity references are not bound to its run")
    fixity_path = _path_under(acquisition_root, fixity_ref, label="fixity JSONL reference")
    summary_path = _path_under(acquisition_root, summary_ref, label="fixity summary reference")
    _regular(fixity_path, label="fixity JSONL")
    _regular(summary_path, label="fixity summary")
    jsonl_sha = _sha256_file(fixity_path)
    summary_sha = _sha256_file(summary_path)
    if fixity.get("jsonl_sha256", fixity.get("sha256")) != jsonl_sha:
        raise HandoffAdapterError("handoff fixity JSONL digest differs")
    if fixity.get("sha256") != jsonl_sha or fixity.get("summary_sha256") != summary_sha:
        raise HandoffAdapterError("handoff fixity summary digest is missing or differs")
    try:
        summary = acquisition._load_json_bytes(summary_path.read_bytes(), label="fixity summary")
    except acquisition.AcquisitionBatchError as exc:
        raise HandoffAdapterError(str(exc)) from exc
    expected_payloads = _expected_payloads(context)
    if (
        summary.get("batch_id") != context.manifest["batch_id"]
        or summary.get("run_id") != run_id
        or summary.get("manifest_sha256") != context.manifest_sha256
        or summary.get("fixity_jsonl_ref") != fixity_ref
        or summary.get("fixity_jsonl_sha256") != jsonl_sha
        or summary.get("independent_pass") is not True
        or summary.get("rows") != len(expected_payloads)
        or summary.get("verified") != len(expected_payloads)
        or summary.get("invalid") != 0
    ):
        raise HandoffAdapterError("fixity summary does not bind complete handoff")
    try:
        fixity_rows = acquisition._journal_rows(fixity_path)
    except acquisition.AcquisitionBatchError as exc:
        raise HandoffAdapterError(f"fixity JSONL is malformed: {exc}") from exc
    if (
        len(fixity_rows) != len(expected_payloads)
        or {acquisition._payload_custody_key(row) for row in fixity_rows}
        != set(expected_payloads)
    ):
        raise HandoffAdapterError("fixity rows do not close over selected payloads")
    for row in fixity_rows:
        payload = expected_payloads.get(acquisition._payload_custody_key(row))
        if payload is None or row.get("status") != "verified":
            raise HandoffAdapterError("fixity contains an unverified payload row")
        expected = payload.payload
        for key, value in {
            "item_ref": payload.item_ref,
            "destination_ref": payload.destination_ref,
            "relative_path": expected["relative_path"],
            "provider_revision": expected["provider_revision"],
            "provider_source_id": expected["provider_source_id"],
            "expected_byte_size": expected["byte_size"],
            "expected_sha256": expected["sha256"],
            "byte_size": expected["byte_size"],
            "sha256": expected["sha256"],
        }.items():
            if row.get(key) != value:
                raise HandoffAdapterError(f"fixity row binding differs: {payload.file_ref}")
        destination = custody.payload_path(
            acquisition_root / "payload", expected["item_root_ref"], expected["relative_path"]
        )
        try:
            digest = acquisition._verify_destination(destination, expected)
        except (acquisition.SourceIntegrityError, custody.CustodyError, OSError) as exc:
            raise HandoffAdapterError(f"payload custody differs: {payload.file_ref}") from exc
        if row.get("git_blob_sha1") != digest.git_blob_sha1:
            raise HandoffAdapterError(f"fixity Git blob digest differs: {payload.file_ref}")

    custody_rows = handoff.get("payload_custody")
    if (
        not isinstance(custody_rows, list)
        or len(custody_rows) != len(expected_payloads)
        or {
            acquisition._payload_custody_key(row)
            for row in custody_rows
            if isinstance(row, dict)
        }
        != set(expected_payloads)
    ):
        raise HandoffAdapterError("handoff payload custody closure differs from manifest")
    for row in custody_rows:
        if not isinstance(row, dict):
            raise HandoffAdapterError("handoff payload custody row is not an object")
        if row.get("status") not in {"acquired", "already_present"}:
            raise HandoffAdapterError("handoff payload row is not acquired custody")
        payload_key = acquisition._payload_custody_key(row)
        if payload_key not in expected_payloads:
            raise HandoffAdapterError("handoff payload custody contains an unselected file")
        payload = expected_payloads[payload_key].payload
        expected_custody = {
            "run_id": run_id,
            "batch_id": context.manifest["batch_id"],
            "manifest_sha256": context.manifest_sha256,
            "item_ref": payload["item_ref"],
            "file_ref": payload["file_ref"],
            "destination_ref": f"{payload['item_root_ref']}/{payload['relative_path']}",
            "provider_url": payload["provider_url"],
            "provider_revision": payload["provider_revision"],
            "provider_source_id": payload["provider_source_id"],
            "expected_byte_size": payload["byte_size"],
            "expected_sha256": payload["sha256"],
        }
        if any(row.get(key) != value for key, value in expected_custody.items()):
            raise HandoffAdapterError(f"handoff payload digest binding differs: {row['file_ref']}")
    return selected_source_rows, [expected_payloads[key].payload for key in sorted(expected_payloads)]


def verify_handoff_for_intake(
    *,
    acquisition_root: Path | str,
    handoff_ref: str,
    expected_base_revision: str,
    expected_manifest_sha256: str,
    repo_root: Path | str = acquisition.REPO_ROOT,
) -> VerifiedHandoff:
    """Verify one original handoff for direct intake closure.

    This is the shared acquisition manifest/record/Item/File/rights/fixity/
    custody verifier.  It deliberately does not load ``CorpusStore`` or an
    accepted snapshot: intake supplies its one streamed accepted index and
    applies additive claim/identity closure after this function returns.
    """

    root = acquisition._checked_root(acquisition_root)
    handoff_path, handoff = _load_handoff(root, handoff_ref)
    context = _load_caller_selected_context(
        acquisition_root=root,
        handoff=handoff,
        expected_manifest_sha256=expected_manifest_sha256,
        repo_root=repo_root,
    )
    try:
        # Immutable legacy handoffs retain their historical directory modes;
        # the producer enforces private roots for every new preparation.
        acquisition._verify_prepared_output(
            context, root, require_private_roots=False
        )
    except (acquisition.AcquisitionBatchError, acquisition.SourceIntegrityError) as exc:
        raise HandoffAdapterError(str(exc)) from exc
    _verify_item_payload_bindings(context, root)
    selected_source_rows, payloads = _verify_handoff(
        acquisition_root=root,
        handoff_path=handoff_path,
        handoff=handoff,
        context=context,
        expected_base_revision=expected_base_revision,
        accepted_source_root=None,
        accepted_manifest=None,
    )
    return VerifiedHandoff(
        root=root,
        handoff_path=handoff_path,
        handoff=handoff,
        context=context,
        selected_source_rows=selected_source_rows,
        payloads=payloads,
    )


def adapt_handoff(
    *,
    acquisition_root: Path | str,
    handoff_ref: str,
    expected_manifest_sha256: str,
    output_root: Path | str,
    accepted_store_root: Path | str,
    accepted_source_root: Path | str,
    base_revision: str,
    validator_sha256: str,
    validation_context: dict[str, Any] | None,
    repo_root: Path | str = acquisition.REPO_ROOT,
) -> dict[str, Any]:
    """Create one private ``tos_corpus_batch_v1`` input from one handoff.

    ``validation_context`` is required even though this adapter does not run
    admission.  The intake owner needs the exact grammar root and paired
    historical evidence used to derive ``validator_sha256`` when it invokes
    the existing corpus admission command.
    """

    if not HEX64.fullmatch(base_revision) or not HEX64.fullmatch(validator_sha256):
        raise HandoffAdapterError("base_revision and validator_sha256 must be lowercase SHA-256 values")
    validation_binding = verify_validation_context(
        validation_context,
        validator_sha256=validator_sha256,
    )
    root = acquisition._checked_root(acquisition_root)
    accepted_store = acquisition._checked_root(accepted_store_root)
    accepted = acquisition._checked_root(accepted_source_root)
    output = Path(output_root).expanduser()
    if not output.is_absolute() or output.exists() or output.is_symlink():
        raise HandoffAdapterError(f"adapter output must be a new absolute path: {output}")
    if not output.parent.is_dir() or output.parent.is_symlink():
        raise HandoffAdapterError(f"adapter output parent must be a regular directory: {output.parent}")
    _verify_accepted_pointer(accepted_store, base_revision)
    accepted_manifest = _load_accepted_manifest(accepted_store, base_revision)
    accepted_validator_sha256 = accepted_manifest["validator_sha256"]
    validator_transition = (
        "aligned"
        if accepted_validator_sha256 == validator_sha256
        else "explicit-grammar-update-required"
    )
    admission_preflight = (
        "not-run-transport-only"
        if validator_transition == "aligned"
        else validator_transition
    )
    handoff_path, handoff = _load_handoff(root, handoff_ref)
    context = _load_caller_selected_context(
        acquisition_root=root,
        handoff=handoff,
        expected_manifest_sha256=expected_manifest_sha256,
        repo_root=repo_root,
    )
    manifest_path = _path_under(root, "manifest.json", label="handoff manifest")
    try:
        # Immutable legacy handoffs retain their historical directory modes;
        # the producer enforces private roots for every new preparation.
        acquisition._verify_prepared_output(
            context, root, require_private_roots=False
        )
    except (acquisition.AcquisitionBatchError, acquisition.SourceIntegrityError) as exc:
        raise HandoffAdapterError(str(exc)) from exc
    _verify_item_payload_bindings(context, root)
    selected_source_rows, payloads = _verify_handoff(
        acquisition_root=root,
        handoff_path=handoff_path,
        handoff=handoff,
        context=context,
        expected_base_revision=base_revision,
        accepted_source_root=accepted,
        accepted_manifest=accepted_manifest,
    )

    staging = Path(tempfile.mkdtemp(prefix=f".{output.name}.adapter-", dir=output.parent))
    try:
        source_root = staging / "source"
        payload_root = staging / "payload"
        receipts_root = staging / "receipts"
        evidence_root = receipts_root / "acquisition-evidence"
        source_root.mkdir()
        payload_root.mkdir()
        receipts_root.mkdir()
        evidence_root.mkdir()
        updates: list[dict[str, Any]] = []
        for row in sorted(selected_source_rows, key=lambda value: value["ref"]):
            source = _path_under(root, row["handoff_ref"], label="handoff source")
            destination = _path_under(source_root, row["ref"], label="candidate source")
            _copy_no_clobber(
                source,
                destination,
                sha256=row["sha256"],
                byte_size=row["byte_size"],
            )
            updates.append(
                {
                    "path": row["ref"],
                    "sha256": row["sha256"],
                    "size_bytes": row["byte_size"],
                    "mode": 0o644,
                }
            )
        for payload in payloads:
            source = custody.payload_path(
                root / "payload", payload["item_root_ref"], payload["relative_path"]
            )
            destination = custody.payload_path(
                payload_root, payload["item_root_ref"], payload["relative_path"]
            )
            source_digest = custody.digest_file(source)
            expected = custody.FileDigest(
                payload["byte_size"],
                payload["sha256"],
                payload.get("git_blob_sha1") or source_digest.git_blob_sha1,
            )
            try:
                status = custody._publish_no_clobber(source, destination, expected)
            except custody.CustodyError as exc:
                raise HandoffAdapterError(f"cannot materialize payload custody: {payload['file_ref']}") from exc
            if status not in {"copied", "already_present"}:
                raise HandoffAdapterError(f"payload custody conflict: {payload['file_ref']}")
            try:
                acquisition._verify_destination(destination, payload)
            except (acquisition.SourceIntegrityError, custody.CustodyError, OSError) as exc:
                raise HandoffAdapterError(f"candidate payload fixity differs: {payload['file_ref']}") from exc

        # Keep the exact producer evidence beside the converted candidate so
        # every receipt reference remains resolvable after the handoff root is
        # moved.  These are operational evidence copies, not source updates:
        # the batch delta remains explicitly not-admitted.
        evidence_sources = (
            ("manifest.json", manifest_path, context.manifest_sha256),
            ("handoff.json", handoff_path, _sha256_file(handoff_path)),
            (
                "provenance-delta.json",
                _path_under(root, _provenance_delta_ref(context), label="handoff provenance delta"),
                handoff["provenance_delta"]["sha256"],
            ),
            (
                "fixity.jsonl",
                _path_under(root, handoff["independent_fixity"]["ref"], label="fixity JSONL reference"),
                handoff["independent_fixity"]["jsonl_sha256"],
            ),
            (
                "fixity-summary.json",
                _path_under(root, handoff["independent_fixity"]["summary_ref"], label="fixity summary reference"),
                handoff["independent_fixity"]["summary_sha256"],
            ),
        )
        evidence_refs: dict[str, dict[str, str]] = {}
        for name, source, digest in evidence_sources:
            destination = evidence_root / name
            info = _regular(source, label=f"handoff evidence: {name}")
            _copy_no_clobber(
                source,
                destination,
                sha256=digest,
                byte_size=info.st_size,
            )
            evidence_refs[name] = {
                "ref": destination.relative_to(staging).as_posix(),
                "source_ref": source.relative_to(root).as_posix(),
                "sha256": digest,
            }

        batch = {
            "schema_version": "tos_corpus_batch_v1",
            "base_revision": base_revision,
            "validator_sha256": validator_sha256,
            "updates": updates,
            "retirements": [],
        }
        slug = context.manifest["batch_id"].removeprefix("tos.acquisition-batch.")
        if not re.fullmatch(r"[a-z0-9][a-z0-9.-]*", slug):
            raise HandoffAdapterError("batch id cannot form a safe candidate manifest name")
        batch_ref = f"manifests/tos-corpus-batch-{slug}.json"
        batch_path = staging / batch_ref
        batch_path.parent.mkdir(parents=True, exist_ok=True)
        batch_path.write_bytes(_canonical(batch))
        os.chmod(batch_path, 0o644)
        try:
            corpus_admit.read_batch(batch_path, source_root)
        except Exception as exc:
            raise HandoffAdapterError(f"candidate tos_corpus_batch_v1 rejected by corpus consumer: {exc}") from exc

        handoff_relative = handoff_path.relative_to(root).as_posix()
        validation_context_bytes = _canonical(validation_binding)
        validation_context_sha256 = _sha256(validation_context_bytes)
        adapter_receipt = {
            "schema_version": "tos_acquisition_handoff_adapter_receipt_v1",
            "handoff_ref": evidence_refs["handoff.json"]["ref"],
            "handoff_source_ref": handoff_relative,
            "handoff_sha256": _sha256_file(handoff_path),
            "manifest_ref": evidence_refs["manifest.json"]["ref"],
            "manifest_source_ref": "manifest.json",
            "manifest_sha256": context.manifest_sha256,
            "caller_expected_manifest_sha256": expected_manifest_sha256,
            "provenance_delta_ref": evidence_refs["provenance-delta.json"]["ref"],
            "provenance_delta_source_ref": handoff["provenance_delta"]["ref"],
            "provenance_delta_sha256": handoff["provenance_delta"]["sha256"],
            "fixity_ref": evidence_refs["fixity.jsonl"]["ref"],
            "fixity_source_ref": handoff["independent_fixity"]["ref"],
            "fixity_jsonl_sha256": handoff["independent_fixity"]["jsonl_sha256"],
            "fixity_summary_ref": evidence_refs["fixity-summary.json"]["ref"],
            "fixity_summary_source_ref": handoff["independent_fixity"]["summary_ref"],
            "fixity_summary_sha256": handoff["independent_fixity"]["summary_sha256"],
            "base_revision": base_revision,
            "accepted_manifest_ref": f"revisions/{base_revision}/snapshot.json",
            "accepted_manifest_sha256": _sha256_file(
                accepted_store / "revisions" / base_revision / "snapshot.json"
            ),
            "accepted_validator_sha256": accepted_validator_sha256,
            "validator_sha256": validator_sha256,
            "validator_transition": validator_transition,
            "validation_context_ref": "receipts/validation-context.json",
            "validation_context_sha256": validation_context_sha256,
            "evidence": evidence_refs,
            "candidate_batch_ref": batch_ref,
            "candidate_batch_sha256": _sha256_file(batch_path),
            "input_root": "source",
            "payload_source_root": "payload",
            "admission_status": "not-admitted",
            "admission_preflight": admission_preflight,
            "validation_context_posture": "transport-bound; downstream-source-validator-required",
            "publication_status": "not-published",
            "topology_preimages": 0,
            "authority_boundary": "validated private batch input only; corpus admission remains with corpus_admit and its selected store",
        }
        (receipts_root / "validation-context.json").write_bytes(validation_context_bytes)
        os.chmod(receipts_root / "validation-context.json", 0o644)
        (receipts_root / "acquisition-handoff-adapter.json").write_bytes(_canonical(adapter_receipt))
        os.chmod(receipts_root / "acquisition-handoff-adapter.json", 0o644)
        os.replace(staging, output)
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise
    return {
        "status": "candidate-not-admitted",
        "output_root": str(output),
        "candidate_batch_ref": batch_ref,
        "candidate_batch_sha256": adapter_receipt["candidate_batch_sha256"],
        "admission_status": "not-admitted",
        "admission_preflight": admission_preflight,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--acquisition-root", type=Path, required=True)
    parser.add_argument("--handoff", required=True)
    parser.add_argument(
        "--expected-manifest-sha256",
        required=True,
        help="caller-held SHA-256 captured when this acquisition manifest was selected",
    )
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--accepted-store-root", type=Path, required=True)
    parser.add_argument("--accepted-source-root", type=Path, required=True)
    parser.add_argument("--base-revision", required=True)
    parser.add_argument("--validator-sha256", required=True)
    parser.add_argument("--validation-context", type=Path, required=True)
    parser.add_argument("--repo-root", type=Path, default=acquisition.REPO_ROOT)
    args = parser.parse_args(argv)
    try:
        validation_context_path = args.validation_context.expanduser()
        _regular(validation_context_path, label="validation context")
        validation_context = acquisition._load_json_bytes(
            validation_context_path.read_bytes(), label="validation context"
        )
        result = adapt_handoff(
            acquisition_root=args.acquisition_root,
            handoff_ref=args.handoff,
            expected_manifest_sha256=args.expected_manifest_sha256,
            output_root=args.output_root,
            accepted_store_root=args.accepted_store_root,
            accepted_source_root=args.accepted_source_root,
            base_revision=args.base_revision,
            validator_sha256=args.validator_sha256,
            validation_context=validation_context,
            repo_root=args.repo_root,
        )
    except (HandoffAdapterError, acquisition.AcquisitionBatchError) as exc:
        print(f"acquisition-handoff-adapter: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

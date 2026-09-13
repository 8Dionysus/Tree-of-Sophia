#!/usr/bin/env python3
"""Import an explicitly planned ToS payload without making it a site or D1 concern.

The importer deliberately has a narrow boundary:

* it consumes a frozen ``server-import-contract`` plan;
* it verifies the manifest, exact rights-record digest, and local bytes;
* it uploads through a small transport adapter only after the historical v1
  human/legal route or the additive v3 controlled agent-review route passes;
* it reads the object back and writes an immutable, path-free receipt; and
* it keeps publication enable/disable state in a separate local registry.

This module never discovers payloads from a checkout, writes D1, edits a plan,
publishes a Worker route, or treats a server copy as source authority.  The
Wrangler adapter uses the current ``r2 object put/get`` commands.  Wrangler's
single-object CLI is intentionally bounded at 315 MiB; a multipart adapter is
future work and is not silently substituted here.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tempfile
from typing import Any, Iterable, Protocol
import fcntl

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
SERVER_IMPORT_SCHEMA = Path("ToS/contracts/server-import-contract.schema.json")
RECEIPT_SCHEMA = Path("ToS/contracts/server-import-receipt.schema.json")
RIGHTS_REVIEW_SCHEMA = Path("ToS/contracts/source-payload-rights-review.schema.json")
WRANGLER_MAX_UPLOAD_BYTES = 315 * 1024 * 1024
PUBLIC_PAYLOAD_ASSESSMENTS = {
    "public-domain-reviewed",
    "open-licensed",
    "permission-granted",
}
REVIEWED_RIGHTS = {"human-reviewed", "legal-reviewed"}
TRANSFER_STATUSES = {"approved-not-uploaded", "imported"}
ACCESS_CLASSES = {"controlled-research", "public-payload"}
BLOCKED_RIGHTS_STATUSES = {
    "denied",
    "rejected",
    "restricted",
    "rights_unknown",
    "unknown",
    "not_authorized",
    "unauthorized",
    "prohibited",
    "forbidden",
}
KNOWN_ASSESSMENTS = {
    "licensed",
    "open_licensed",
    "public_domain_reviewed",
    "public_domain",
    "permission_granted",
    "research_only",
}
KNOWN_SERVER_POSTURES = {"authorized", "allowed", "open", "local_only"}
KNOWN_REDISTRIBUTION_POSTURES = {"authorized", "allowed", "open", "public", "local_only"}
KNOWN_VISIBILITY = {"local_only", "private", "controlled", "public", "metadata_only"}
MISSING_OBJECT_MARKERS = (
    "not found",
    "no such object",
    "does not exist",
    "object not found",
)
SAFE_OBJECT_KEY = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/-]*$")
SAFE_TOS_ID = re.compile(r"^tos\.[a-z0-9][a-z0-9.-]*$")


class SourcePayloadImportError(RuntimeError):
    """Base class for fail-closed importer errors."""


class PlanError(SourcePayloadImportError):
    """The frozen plan or its source references are not valid."""


class RightsGateError(SourcePayloadImportError):
    """The plan does not authorize this transfer."""


class LocalIntegrityError(SourcePayloadImportError):
    """The selected local payload is not the planned byte sequence."""


class RemoteIntegrityError(SourcePayloadImportError):
    """The remote object is missing or is not the planned byte sequence."""


class TransportError(SourcePayloadImportError):
    """A transport operation failed without exposing command credentials."""


class ReceiptConflict(SourcePayloadImportError):
    """An immutable receipt path already contains a different receipt."""


class PublicationError(SourcePayloadImportError):
    """A local publication registry operation is denied or malformed."""


@dataclass(frozen=True)
class ImportOutcome:
    receipt_path: Path
    remote_status: str
    upload_attempted: bool
    receipt_reused: bool


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace(
        "+00:00", "Z"
    )


def _strict_pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in items:
        if key in result:
            raise PlanError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"), object_pairs_hook=_strict_pairs
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PlanError(f"cannot read JSON {path.name}: {exc}") from exc
    if not isinstance(value, dict):
        raise PlanError(f"JSON object required: {path.name}")
    return value


def write_json(path: Path, value: dict[str, Any], *, immutable: bool = False) -> None:
    data = (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode(
        "utf-8"
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    if immutable and path.exists():
        try:
            if path.read_bytes() == data:
                return
        except OSError as exc:
            raise ReceiptConflict(f"cannot compare existing receipt: {path.name}") from exc
        raise ReceiptConflict(f"immutable receipt conflict: {path.name}")
    if immutable:
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
        )
        temporary = Path(temporary_name)
        try:
            with os.fdopen(descriptor, "wb") as stream:
                stream.write(data)
                stream.flush()
                os.fsync(stream.fileno())
            try:
                # link(2) is an atomic no-clobber publication on the same
                # filesystem.  A crash can leave only the hidden temp file,
                # never a partially written final receipt.
                os.link(temporary, path)
            except FileExistsError:
                if path.read_bytes() != data:
                    raise ReceiptConflict(f"immutable receipt conflict: {path.name}")
            try:
                directory_fd = os.open(path.parent, os.O_DIRECTORY)
                try:
                    os.fsync(directory_fd)
                finally:
                    os.close(directory_fd)
            except OSError:
                # Receipt contents are durable; directory fsync is an
                # availability improvement on filesystems without O_DIRECTORY.
                pass
        finally:
            try:
                temporary.unlink()
            except FileNotFoundError:
                pass
        return
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    temporary.write_bytes(data)
    os.replace(temporary, path)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as exc:
        raise LocalIntegrityError(f"cannot read payload {path.name}") from exc
    return digest.hexdigest()


def _safe_posix_ref(ref: str, *, prefix: str | None = None) -> tuple[str, ...]:
    if not isinstance(ref, str) or not ref or "\\" in ref or "\x00" in ref:
        raise PlanError(f"invalid path reference: {ref!r}")
    parts = PurePosixPath(ref)
    if parts.is_absolute() or ".." in parts.parts or str(parts) != ref:
        raise PlanError(f"unsafe path reference: {ref!r}")
    if prefix is not None and not ref.startswith(prefix):
        raise PlanError(f"path is outside {prefix}: {ref!r}")
    return parts.parts


def safe_repo_path(repo_root: Path, ref: str) -> Path:
    parts = _safe_posix_ref(ref, prefix="ToS/")
    root = repo_root.resolve()
    candidate = root.joinpath(*parts)
    try:
        candidate.resolve(strict=False).relative_to(root)
    except ValueError as exc:
        raise PlanError(f"repository path escapes root: {ref}") from exc
    current = candidate
    while current != root:
        if current.is_symlink():
            raise PlanError(f"symlink in repository reference: {ref}")
        current = current.parent
    return candidate


def safe_payload_relative(ref: str) -> tuple[str, ...]:
    parts = _safe_posix_ref(ref, prefix="payload/")
    if len(parts) != 2 or parts[0] != "payload" or parts[1] in {"", ".", ".."}:
        raise PlanError(f"payload path must contain one filename: {ref!r}")
    return parts


def _schema_path(repo_root: Path, relative: Path) -> Path:
    candidate = repo_root / relative
    if candidate.is_file():
        return candidate
    raise PlanError(f"missing schema: {relative.as_posix()}")


def validate_json_schema(value: dict[str, Any], path: Path, *, error_type: type[Exception] = PlanError) -> None:
    try:
        schema = json.loads(path.read_text(encoding="utf-8"))
        validator = Draft202012Validator(schema, format_checker=FormatChecker())
        errors = sorted(validator.iter_errors(value), key=lambda error: list(error.path))
    except Exception as exc:  # jsonschema's schema errors must be actionable too.
        if isinstance(exc, error_type):
            raise
        raise error_type(f"cannot validate against {path.name}: {exc}") from exc
    if errors:
        first = errors[0]
        location = ".".join(str(part) for part in first.path) or "$"
        raise error_type(f"{path.name} {location}: {first.message}")


@dataclass(frozen=True)
class PlanContext:
    repo_root: Path
    path: Path
    plan_ref: str
    plan_sha256: str
    plan: dict[str, Any]


@dataclass(frozen=True)
class VerifiedFile:
    item_ref: str
    file_ref: str
    relative_path: str
    local_path: Path
    byte_size: int
    sha256: str
    media_type: str
    manifest_ref: str
    manifest_sha256: str
    rights_ref: str
    rights_sha256: str

    @property
    def object_key(self) -> str:
        return f"blobs/sha256/{self.sha256[:2]}/{self.sha256}"


def relative_plan_ref(repo_root: Path, plan_path: Path, server_import_id: str) -> str:
    try:
        return plan_path.resolve().relative_to(repo_root.resolve()).as_posix()
    except ValueError:
        # The absolute command/config path is private operator input.  Do not
        # copy it into an immutable/public receipt.
        return server_import_id


def load_plan(repo_root: Path, plan_path: Path) -> PlanContext:
    repo_root = repo_root.resolve()
    resolved_plan = plan_path if plan_path.is_absolute() else repo_root / plan_path
    resolved_plan = resolved_plan.resolve()
    plan = load_json(resolved_plan)
    validate_json_schema(plan, _schema_path(repo_root, SERVER_IMPORT_SCHEMA))
    if plan["rights_policy"]["review_status"] == "agent-reviewed":
        review_info = plan["rights_policy"].get("rights_review")
        if plan["contract_version"] < 3 or not isinstance(review_info, dict):
            raise PlanError("agent-reviewed plans require contract_version >= 3 and rights_review")
        review_path = safe_repo_path(repo_root, review_info["ref"])
        if not review_path.is_file():
            raise PlanError(f"rights review is missing: {review_info['ref']}")
        if sha256_file(review_path) != review_info["sha256"]:
            raise PlanError("rights review SHA-256 does not match frozen plan")
        review = load_json(review_path)
        validate_json_schema(review, _schema_path(repo_root, RIGHTS_REVIEW_SCHEMA))
    plan_sha = sha256_file(resolved_plan)
    return PlanContext(
        repo_root=repo_root,
        path=resolved_plan,
        plan_ref=relative_plan_ref(repo_root, resolved_plan, plan["server_import_id"]),
        plan_sha256=plan_sha,
        plan=plan,
    )


def _manifest_and_rights(context: PlanContext) -> tuple[Path, dict[str, Any], Path, dict[str, Any]]:
    plan = context.plan
    manifest_info = plan["manifest"]
    manifest_path = safe_repo_path(context.repo_root, manifest_info["ref"])
    if not manifest_path.is_file():
        raise PlanError(f"manifest is missing: {manifest_info['ref']}")
    actual_manifest_sha = sha256_file(manifest_path)
    if actual_manifest_sha != manifest_info["sha256"]:
        raise PlanError("manifest SHA-256 does not match frozen plan")
    manifest = load_json(manifest_path)
    if not manifest_info["verified"]:
        raise PlanError("frozen plan marks manifest unverified")
    if manifest.get("item_id") != plan["item_ref"]:
        raise PlanError("plan item_ref does not match item manifest item_id")
    _validate_plan_manifest_inventory(context, manifest)

    rights_info = plan["rights_policy"]
    rights_path = safe_repo_path(context.repo_root, rights_info["rights_record_ref"])
    if not rights_path.is_file():
        raise PlanError(f"rights record is missing: {rights_info['rights_record_ref']}")
    actual_rights_sha = sha256_file(rights_path)
    if actual_rights_sha != rights_info["rights_record_sha256"]:
        raise PlanError("rights-record SHA-256 does not match frozen plan")
    rights = load_json(rights_path)
    manifest_rights = manifest.get("rights_ref")
    if manifest_rights and manifest_rights != rights_info["rights_record_ref"]:
        raise PlanError("plan rights record is not the item manifest rights_ref")
    return manifest_path, manifest, rights_path, rights


def _validate_plan_manifest_inventory(context: PlanContext, manifest: dict[str, Any]) -> None:
    """Require the frozen plan and manifest to describe one exact inventory."""

    plan_entries = context.plan.get("payload_files")
    manifest_entries = manifest.get("payload_files")
    if not isinstance(plan_entries, list) or not isinstance(manifest_entries, list):
        raise PlanError("plan and manifest payload_files must both be arrays")
    plan_ids = [entry.get("file_ref") for entry in plan_entries if isinstance(entry, dict)]
    manifest_ids = [entry.get("file_id") for entry in manifest_entries if isinstance(entry, dict)]
    if len(plan_ids) != len(plan_entries) or len(manifest_ids) != len(manifest_entries):
        raise PlanError("plan and manifest payload inventories contain non-object entries")
    if len(set(plan_ids)) != len(plan_ids) or len(set(manifest_ids)) != len(manifest_ids):
        raise PlanError("plan or manifest payload inventory contains duplicate File IDs")
    if len(plan_ids) != len(set(entry.get("relative_path") for entry in plan_entries)):
        raise PlanError("plan payload inventory contains duplicate relative paths")
    if len(manifest_ids) != len(set(entry.get("relative_path") for entry in manifest_entries)):
        raise PlanError("manifest payload inventory contains duplicate relative paths")
    if set(plan_ids) != set(manifest_ids):
        raise PlanError("plan and manifest payload inventories do not contain the same File IDs")
    manifest_by_id = {entry["file_id"]: entry for entry in manifest_entries}
    for planned in plan_entries:
        manifest_entry = manifest_by_id[planned["file_ref"]]
        for field in ("relative_path", "byte_size", "sha256"):
            if manifest_entry.get(field) != planned.get(field):
                raise PlanError(
                    f"plan/manifest inventory mismatch for {planned['file_ref']}: {field}"
                )


def _validate_agent_review(
    context: PlanContext,
    *,
    manifest_path: Path,
    rights_path: Path,
) -> tuple[str, str] | None:
    """Validate v3 agent review evidence against the exact frozen plan scope.

    ``agent-reviewed`` is an additive controlled-research route.  It never
    upgrades historical v1 records and never permits public-payload plans.
    """

    plan = context.plan
    rights_policy = plan["rights_policy"]
    if rights_policy["review_status"] != "agent-reviewed":
        return None
    if plan["contract_version"] < 3:
        raise RightsGateError("agent-reviewed requires server-import contract_version >= 3")
    review_info = rights_policy.get("rights_review")
    if not isinstance(review_info, dict):
        raise RightsGateError("agent-reviewed plan lacks rights_review ref/digest")
    review_path = safe_repo_path(context.repo_root, review_info["ref"])
    if not review_path.is_file():
        raise RightsGateError("agent rights review evidence is missing")
    actual_review_sha = sha256_file(review_path)
    if actual_review_sha != review_info["sha256"]:
        raise RightsGateError("agent rights review evidence digest mismatch")
    review = load_json(review_path)
    validate_json_schema(review, _schema_path(context.repo_root, RIGHTS_REVIEW_SCHEMA), error_type=RightsGateError)

    scope = review["scope"]
    plan_file_refs = {entry["file_ref"] for entry in plan["payload_files"]}
    review_file_refs = set(scope["file_refs"])
    if scope["item_ref"] != plan["item_ref"] or review_file_refs != plan_file_refs:
        raise RightsGateError("agent rights review scope does not cover exactly the planned Item and all Files")
    if scope["manifest_ref"] != plan["manifest"]["ref"] or scope["manifest_sha256"] != plan["manifest"]["sha256"]:
        raise RightsGateError("agent rights review manifest scope does not match the frozen plan")
    if scope["rights_record_ref"] != rights_policy["rights_record_ref"] or scope["rights_record_sha256"] != rights_policy["rights_record_sha256"]:
        raise RightsGateError("agent rights review rights-record scope does not match the frozen plan")
    if review["actor"]["kind"] != "model":
        raise RightsGateError("agent rights review actor kind is not model")
    decision = review["decision"]
    if (
        decision["access_class"] != "controlled-research"
        or decision["raw_cloud_retention"] != "controlled-research"
        or decision["server_processing"] != "authorized"
        or decision["publication"] != "not-published"
    ):
        raise RightsGateError("agent rights review is not limited to controlled research")
    if plan["access_class"] != "controlled-research" or plan["publication_status"] != "not-published":
        raise RightsGateError("agent-reviewed plans cannot authorize public payload publication")
    if decision["expires_at"] != rights_policy.get("expires_at"):
        raise RightsGateError("agent rights review expiry does not match the plan")
    if not decision["revocation_check_ref"].startswith("ToS/"):
        raise RightsGateError("agent rights review revocation check must be a tracked ToS ref")
    evidence = review["license_evidence"]
    if not any(entry["kind"] == "primary-license" for entry in evidence):
        raise RightsGateError("agent rights review lacks primary license evidence")
    planned_license_refs = set(rights_policy["permission_or_license_refs"])
    for item in evidence:
        evidence_path = safe_repo_path(context.repo_root, item["evidence_ref"])
        if not evidence_path.is_file():
            raise RightsGateError(f"tracked license evidence is missing: {item['evidence_ref']}")
        if sha256_file(evidence_path) != item["evidence_sha256"]:
            raise RightsGateError(f"tracked license evidence digest mismatch: {item['evidence_ref']}")
        if item["source_ref"] not in planned_license_refs and item["evidence_ref"] not in planned_license_refs:
            raise RightsGateError("license evidence is not bound to a plan permission_or_license_ref")
    if not review["decision"]["conditions"]:
        raise RightsGateError("agent rights review must preserve explicit processing conditions")
    if sha256_file(manifest_path) != scope["manifest_sha256"] or sha256_file(rights_path) != scope["rights_record_sha256"]:
        raise RightsGateError("agent rights review scope no longer matches source bytes")
    return review_info["ref"], review_info["sha256"]


def validate_source_review(context: PlanContext) -> None:
    """Validate the current plan's rights-review evidence without transfer."""

    manifest_path, _manifest, rights_path, rights = _manifest_and_rights(context)
    _assert_rights_record_compatible(context, rights)
    status = context.plan["rights_policy"]["review_status"]
    if status == "agent-reviewed":
        _validate_agent_review(context, manifest_path=manifest_path, rights_path=rights_path)
    elif status not in REVIEWED_RIGHTS:
        raise RightsGateError("source review is not human/legal-reviewed or agent-reviewed")


def _normalise_rights_token(value: str) -> str:
    return re.sub(r"[-\s]+", "_", value.strip().lower())


def _rights_posture_values(rights: dict[str, Any], field: str) -> list[str]:
    values: list[str] = []
    layers = rights.get("layer_assessments", [])
    if not isinstance(layers, list):
        layers = []
    for container in [rights, *layers]:
        value = container.get(field) if isinstance(container, dict) else None
        if isinstance(value, str):
            values.append(_normalise_rights_token(value))
    return values


def _assert_rights_record_compatible(context: PlanContext, rights: dict[str, Any]) -> None:
    """Reject source-rights records that contradict the frozen transfer claim.

    The v3 actor-labelled review adds an exact, bounded controlled-research
    decision. It cannot turn an actual denied/restricted/prohibited source
    posture into permission. A legacy ``local_only`` visibility marker is
    allowed only on that explicit v3 controlled route; it remains a blocker
    for public payload and for the historical human/legal route.
    """

    plan = context.plan
    scope_refs = rights.get("scope_refs")
    if isinstance(scope_refs, list) and scope_refs:
        required_refs = {plan["item_ref"], *(entry["file_ref"] for entry in plan["payload_files"])}
        if not required_refs.issubset(set(scope_refs)):
            raise RightsGateError("rights record scope does not cover the planned Item and Files")

    assessments = _rights_posture_values(rights, "assessment_status")
    if not assessments or any(value not in KNOWN_ASSESSMENTS and value not in BLOCKED_RIGHTS_STATUSES for value in assessments):
        raise RightsGateError("rights record has an unsupported assessment status")
    if any(value in BLOCKED_RIGHTS_STATUSES for value in assessments):
        raise RightsGateError("rights record contains a denied or restricted assessment")
    plan_assessment = _normalise_rights_token(plan["rights_policy"]["assessment_status"])
    positive_assessments = {"licensed", "open_licensed", "public_domain_reviewed", "public_domain", "permission_granted"}
    if plan_assessment in {"open_licensed", "public_domain_reviewed", "permission_granted"} and not any(
        value in positive_assessments for value in assessments
    ):
        raise RightsGateError("rights record does not support the plan's positive license assessment")

    layers = rights.get("layer_assessments", [])
    if not isinstance(layers, list):
        layers = []
    restrictions = [
        item
        for container in [rights, *layers]
        if isinstance(container, dict)
        for item in container.get("restrictions", [])
        if isinstance(item, str)
    ]
    if restrictions:
        raise RightsGateError(
            "rights record has non-empty restrictions; an exact restricted-rights route is required"
        )

    server_postures = _rights_posture_values(rights, "server_processing_posture")
    unsupported_server = set(server_postures) - KNOWN_SERVER_POSTURES - BLOCKED_RIGHTS_STATUSES
    if unsupported_server:
        raise RightsGateError("rights record has an unsupported server-processing posture")
    if any(value in BLOCKED_RIGHTS_STATUSES for value in server_postures):
        raise RightsGateError("rights record does not authorize server processing")

    redistribution = _rights_posture_values(rights, "redistribution_posture")
    unsupported_redistribution = set(redistribution) - KNOWN_REDISTRIBUTION_POSTURES - BLOCKED_RIGHTS_STATUSES
    if unsupported_redistribution:
        raise RightsGateError("rights record has an unsupported redistribution posture")
    if any(value in BLOCKED_RIGHTS_STATUSES for value in redistribution):
        raise RightsGateError("rights record does not authorize redistribution")
    if plan["access_class"] == "public-payload" and any(
        value not in {"authorized", "allowed", "open", "public"} for value in redistribution
    ):
        raise RightsGateError("rights record does not authorize redistribution")

    visibility = _rights_posture_values(rights, "visibility")
    unsupported_visibility = set(visibility) - KNOWN_VISIBILITY
    if unsupported_visibility:
        raise RightsGateError("rights record has an unsupported visibility posture")
    if plan["access_class"] == "public-payload" and any(value in {"local_only", "private", "restricted"} for value in visibility):
        raise RightsGateError("local-only rights cannot authorize public payload")
    if (
        plan["access_class"] == "controlled-research"
        and any(value == "local_only" for value in visibility)
        and plan["rights_policy"]["review_status"] != "agent-reviewed"
    ):
        raise RightsGateError("legacy local-only rights require the explicit v3 controlled review route")


def _manifest_file_entry(manifest: dict[str, Any], file_ref: str) -> dict[str, Any]:
    candidates = [entry for entry in manifest.get("payload_files", []) if entry.get("file_id") == file_ref]
    if len(candidates) != 1:
        raise PlanError(f"file_ref is not uniquely present in item manifest: {file_ref}")
    return candidates[0]


def _payload_path(
    *,
    payload_source_root: Path,
    payload_source_layout: str,
    manifest_path: Path,
    repo_root: Path,
    relative_path: str,
) -> Path:
    relative_parts = safe_payload_relative(relative_path)
    root = payload_source_root.expanduser()
    if not root.is_absolute():
        raise LocalIntegrityError("payload source root must be an absolute path")
    current_root = root
    while current_root != current_root.parent:
        if current_root.is_symlink():
            raise LocalIntegrityError("payload source root or its ancestors contain a symlink")
        current_root = current_root.parent
    if not root.is_dir():
        raise LocalIntegrityError("payload source root is not a directory")
    if payload_source_layout == "repo":
        try:
            manifest_parent = manifest_path.resolve().relative_to(repo_root.resolve()).parent
        except ValueError as exc:
            raise LocalIntegrityError("repo payload layout requires a repository plan") from exc
        candidate = root.joinpath(*manifest_parent.parts, *relative_parts)
    elif payload_source_layout == "source-witness":
        try:
            witness_parent = manifest_path.resolve().relative_to(
                (repo_root / "ToS/source-witnesses").resolve()
            ).parent
        except ValueError as exc:
            raise LocalIntegrityError(
                "source-witness payload layout requires a ToS/source-witnesses manifest"
            ) from exc
        candidate = root.joinpath(*witness_parent.parts, *relative_parts)
    elif payload_source_layout == "item":
        candidate = root.joinpath(*relative_parts)
    else:
        raise LocalIntegrityError(f"unknown payload source layout: {payload_source_layout}")
    try:
        candidate.resolve(strict=False).relative_to(root)
    except ValueError as exc:
        raise LocalIntegrityError("payload path escapes payload source root") from exc
    current = candidate
    while current != root:
        if current.is_symlink():
            raise LocalIntegrityError(f"symlink in payload path: {relative_path}")
        current = current.parent
    return candidate


def _verified_files(
    context: PlanContext,
    *,
    payload_source_root: Path,
    payload_source_layout: str,
    file_ref: str | None = None,
) -> list[VerifiedFile]:
    manifest_path, manifest, _rights_path, _rights = _manifest_and_rights(context)
    plan = context.plan
    files = plan["payload_files"]
    if file_ref is not None:
        files = [entry for entry in files if entry["file_ref"] == file_ref]
        if not files:
            raise PlanError(f"file_ref is not in the frozen plan: {file_ref}")
    results: list[VerifiedFile] = []
    for planned in files:
        if not planned["verified"]:
            raise PlanError(f"frozen plan marks file unverified: {planned['file_ref']}")
        manifest_entry = _manifest_file_entry(manifest, planned["file_ref"])
        for field in ("relative_path", "byte_size", "sha256"):
            if manifest_entry.get(field) != planned[field]:
                raise PlanError(f"manifest/file mismatch for {planned['file_ref']}: {field}")
        local_path = _payload_path(
            payload_source_root=payload_source_root,
            payload_source_layout=payload_source_layout,
            manifest_path=manifest_path,
            repo_root=context.repo_root,
            relative_path=planned["relative_path"],
        )
        if not local_path.exists() or not local_path.is_file() or local_path.is_symlink():
            raise LocalIntegrityError(f"payload file is unavailable or unsafe: {planned['relative_path']}")
        actual_size = local_path.stat().st_size
        if actual_size != planned["byte_size"]:
            raise LocalIntegrityError(f"byte-size mismatch for {planned['file_ref']}")
        actual_sha = sha256_file(local_path)
        if actual_sha != planned["sha256"]:
            raise LocalIntegrityError(f"SHA-256 mismatch for {planned['file_ref']}")
        results.append(
            VerifiedFile(
                item_ref=plan["item_ref"],
                file_ref=planned["file_ref"],
                relative_path=planned["relative_path"],
                local_path=local_path,
                byte_size=planned["byte_size"],
                sha256=planned["sha256"],
                media_type=manifest_entry.get("media_type", "application/octet-stream"),
                manifest_ref=plan["manifest"]["ref"],
                manifest_sha256=plan["manifest"]["sha256"],
                rights_ref=plan["rights_policy"]["rights_record_ref"],
                rights_sha256=plan["rights_policy"]["rights_record_sha256"],
            )
        )
    return results


def verify_local(
    context: PlanContext,
    *,
    payload_source_root: Path,
    payload_source_layout: str = "source-witness",
    file_ref: str | None = None,
) -> list[VerifiedFile]:
    """Verify frozen plan, manifest, rights digest, and exact local payload bytes."""

    return _verified_files(
        context,
        payload_source_root=payload_source_root,
        payload_source_layout=payload_source_layout,
        file_ref=file_ref,
    )


def _parse_expiry(value: str | None, now: datetime) -> None:
    if not value:
        return
    try:
        expiry = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise RightsGateError("rights expires_at is not an ISO timestamp") from exc
    if expiry.tzinfo is None:
        expiry = expiry.replace(tzinfo=timezone.utc)
    if expiry <= now:
        raise RightsGateError("rights policy has expired")


def enforce_transfer_gate(
    plan: dict[str, Any],
    *,
    context: PlanContext | None = None,
    now: datetime | None = None,
) -> None:
    """Apply the existing v1 rights and operator gates without promotion."""

    access_class = plan["access_class"]
    if access_class not in ACCESS_CLASSES:
        raise RightsGateError(f"payload transfer denied for access_class={access_class}")
    if not plan["payload_transfer_authorized"]:
        raise RightsGateError("payload_transfer_authorized is false")
    approval = plan["operator_transfer_approval"]
    if not (
        approval["approved"]
        and approval["approved_by_real_human"]
        and isinstance(approval.get("approval_ref"), str)
        and approval["approval_ref"]
        and isinstance(approval.get("approved_at"), str)
    ):
        raise RightsGateError("real-human operator transfer approval is missing")
    if plan["server_import_status"] not in TRANSFER_STATUSES:
        raise RightsGateError(
            f"server_import_status does not authorize transfer: {plan['server_import_status']}"
        )
    if plan["publication_status"] in {"withdrawn", "metadata-only", "not-published"} and access_class == "public-payload":
        # A fresh approved-not-uploaded plan may still say not-published.  The
        # transfer is allowed then; only a withdrawn or metadata-only plan is
        # contradictory.  Publication remains separately disabled after import.
        if plan["publication_status"] == "not-published":
            pass
        else:
            raise RightsGateError("public payload plan is not publication-capable")
    rights = plan["rights_policy"]
    rights_record: dict[str, Any] | None = None
    if rights["review_status"] == "agent-reviewed":
        if context is None:
            raise RightsGateError("agent-reviewed transfer requires source-bound review context")
        manifest_path, _manifest, rights_path, rights_record = _manifest_and_rights(context)
        _validate_agent_review(context, manifest_path=manifest_path, rights_path=rights_path)
    elif rights["review_status"] not in REVIEWED_RIGHTS:
        raise RightsGateError(
            "legacy server-import v1 accepts only human-reviewed or legal-reviewed rights"
        )
    else:
        if context is not None:
            _manifest_path, _manifest, _rights_path, rights_record = _manifest_and_rights(context)
    if context is not None and rights_record is not None:
        _assert_rights_record_compatible(context, rights_record)
    if not rights["recheck_before_transfer"]:
        raise RightsGateError("rights policy disabled mandatory pre-transfer recheck")
    _parse_expiry(rights.get("expires_at"), now or datetime.now(timezone.utc))
    if not rights["permission_or_license_refs"]:
        raise RightsGateError("rights policy has no permission or license evidence")
    if access_class == "public-payload":
        if rights["assessment_status"] not in PUBLIC_PAYLOAD_ASSESSMENTS:
            raise RightsGateError(
                "public-payload requires an exact public-domain/open-license/permission assessment"
            )
    elif rights["assessment_status"] in {"rights-unknown", "restricted", "rejected"}:
        raise RightsGateError(
            "controlled-research cannot transfer a restricted or unknown-rights item"
        )


class TransportAdapter(Protocol):
    """Small object transport contract used by the importer and tests."""

    def fetch(self, object_key: str, destination: Path) -> bool:
        """Download object into destination; return False only for not-found."""

    def put(
        self,
        object_key: str,
        source: Path,
        *,
        byte_size: int,
        media_type: str,
        storage_class: str,
    ) -> None:
        """Upload one object; this protocol does not promise remote CAS."""


def _safe_object_key(key: str) -> str:
    if (
        not isinstance(key, str)
        or not SAFE_OBJECT_KEY.fullmatch(key)
        or key.startswith("/")
        or ".." in PurePosixPath(key).parts
    ):
        raise TransportError(f"unsafe object key: {key!r}")
    return key


class WranglerR2Transport:
    """Native Wrangler R2 adapter with private/no-store object metadata."""

    def __init__(
        self,
        *,
        bucket: str,
        executable: Path,
        max_upload_bytes: int = WRANGLER_MAX_UPLOAD_BYTES,
        cwd: Path | None = None,
    ) -> None:
        if not bucket or "/" in bucket or "\\" in bucket:
            raise TransportError("invalid R2 bucket name")
        if not executable.is_absolute():
            raise TransportError("Wrangler executable must be an absolute path")
        self.bucket = bucket
        self.executable = executable
        self.max_upload_bytes = max_upload_bytes
        self.cwd = cwd or REPO_ROOT

    def _object_path(self, object_key: str) -> str:
        return f"{self.bucket}/{_safe_object_key(object_key)}"

    def _run(self, args: list[str]) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment.setdefault("WRANGLER_SEND_METRICS", "false")
        try:
            result = subprocess.run(
                [str(self.executable), *args],
                cwd=self.cwd,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
        except OSError as exc:
            raise TransportError("cannot execute Wrangler transport") from exc
        return result

    @staticmethod
    def _is_missing(result: subprocess.CompletedProcess[str]) -> bool:
        text = f"{result.stdout}\n{result.stderr}".lower()
        return any(marker in text for marker in MISSING_OBJECT_MARKERS)

    def fetch(self, object_key: str, destination: Path) -> bool:
        _safe_object_key(object_key)
        if destination.exists():
            raise TransportError("transport destination already exists")
        result = self._run(
            [
                "r2",
                "object",
                "get",
                self._object_path(object_key),
                "--remote",
                "--file",
                str(destination),
            ]
        )
        if result.returncode != 0:
            if destination.exists():
                destination.unlink()
            if self._is_missing(result):
                return False
            raise TransportError(f"Wrangler R2 read failed with exit {result.returncode}")
        if not destination.is_file():
            raise TransportError("Wrangler reported success without an object file")
        return True

    def put(
        self,
        object_key: str,
        source: Path,
        *,
        byte_size: int,
        media_type: str,
        storage_class: str,
    ) -> None:
        _safe_object_key(object_key)
        if storage_class != "Standard":
            raise TransportError("only R2 Standard storage is enabled by this adapter")
        if byte_size > self.max_upload_bytes:
            raise TransportError(
                f"Wrangler single-object upload limit is {self.max_upload_bytes} bytes; "
                "multipart transport is required for this file"
            )
        result = self._run(
            [
                "r2",
                "object",
                "put",
                self._object_path(object_key),
                "--remote",
                "--file",
                str(source),
                "--content-type",
                media_type,
                "--cache-control",
                "private,no-store",
                "--storage-class",
                "Standard",
            ]
        )
        if result.returncode != 0:
            raise TransportError(f"Wrangler R2 upload failed with exit {result.returncode}")


def _verify_download(path: Path, expected_size: int, expected_sha256: str) -> None:
    if not path.is_file():
        raise RemoteIntegrityError("transport did not produce a regular readback file")
    actual_size = path.stat().st_size
    if actual_size != expected_size:
        raise RemoteIntegrityError("remote readback byte-size mismatch")
    actual_sha = sha256_file(path)
    if actual_sha != expected_sha256:
        raise RemoteIntegrityError("remote readback SHA-256 mismatch")


def _checked_scratch_root(path: Path) -> Path:
    path = path.expanduser()
    if not path.is_absolute():
        raise LocalIntegrityError("managed scratch root must be an absolute path")
    path.mkdir(parents=True, exist_ok=True)
    current = path
    while current != current.parent:
        if current.is_symlink():
            raise LocalIntegrityError("managed scratch root or its ancestors contain a symlink")
        current = current.parent
    if not path.is_dir():
        raise LocalIntegrityError("managed scratch root is not a directory")
    return path


def _snapshot_verified_payload(verified: VerifiedFile, scratch_root: Path) -> Path:
    """Copy and re-verify a source file before exposing it to a transport."""

    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f"tos-source-snapshot-{verified.sha256[:12]}-",
        suffix=".bin",
        dir=scratch_root,
    )
    snapshot = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as target, verified.local_path.open("rb") as source:
            shutil.copyfileobj(source, target, length=1024 * 1024)
            target.flush()
            os.fsync(target.fileno())
        try:
            _verify_download(snapshot, verified.byte_size, verified.sha256)
        except RemoteIntegrityError as exc:
            raise LocalIntegrityError("payload changed while creating the managed snapshot") from exc
        return snapshot
    except LocalIntegrityError:
        snapshot.unlink(missing_ok=True)
        raise
    except OSError as exc:
        snapshot.unlink(missing_ok=True)
        raise LocalIntegrityError("cannot create a managed payload snapshot") from exc


@contextmanager
def _import_lock(receipt_dir: Path) -> Iterable[None]:
    """Serialize import writers sharing one receipt root.

    This is an operator-local coordination lock. It cannot provide a remote
    compare-and-swap against foreign hosts or writers that do not take it.
    """

    receipt_dir.mkdir(parents=True, exist_ok=True)
    lock_path = receipt_dir / ".source-payload-import.lock"
    with lock_path.open("a+") as stream:
        fcntl.flock(stream.fileno(), fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(stream.fileno(), fcntl.LOCK_UN)


def _publish_verified_output(source: Path, output: Path, *, byte_size: int, sha256: str) -> None:
    """Verify a private readback before atomically publishing a new output."""

    if output.exists():
        raise PublicationError("read output already exists; refusing overwrite")
    output.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{output.name}.", suffix=".tmp", dir=output.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as target, source.open("rb") as incoming:
            shutil.copyfileobj(incoming, target, length=1024 * 1024)
            target.flush()
            os.fsync(target.fileno())
        _verify_download(temporary, byte_size, sha256)
        try:
            os.link(temporary, output)
        except FileExistsError as exc:
            raise PublicationError("read output appeared during no-clobber publication") from exc
        try:
            directory_fd = os.open(output.parent, os.O_DIRECTORY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
        except OSError:
            pass
    finally:
        try:
            temporary.unlink()
        except FileNotFoundError:
            pass


def publication_id(item_ref: str, file_ref: str, rights_revision: str) -> str:
    token = hashlib.sha256(f"{item_ref}\0{file_ref}\0{rights_revision}".encode()).hexdigest()
    return f"pub-{token[:32]}"


def receipt_filename(server_import_id: str, file_sha256: str, plan_sha256: str) -> str:
    safe_id = re.sub(r"[^A-Za-z0-9.-]+", "-", server_import_id)
    return f"{safe_id}.{file_sha256[:16]}.{plan_sha256[:16]}.receipt.json"


def _receipt(
    context: PlanContext,
    verified: VerifiedFile,
    *,
    bucket_alias: str,
    remote_status: str,
    upload_attempted: bool,
    readback_verified_at: str,
) -> dict[str, Any]:
    plan = context.plan
    rights_revision = verified.rights_sha256
    receipt_id = (
        f"tos.receipt.server-import.{re.sub(r'[^a-z0-9.-]+', '-', plan['server_import_id'].lower())}."
        f"{verified.sha256[:16]}.{context.plan_sha256[:16]}"
    )
    receipt = {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/server-import-receipt.schema.json",
        "schema_version": "tos_server_import_receipt_v1",
        "receipt_id": receipt_id,
        "server_import_id": plan["server_import_id"],
        "plan": {"ref": context.plan_ref, "sha256": context.plan_sha256},
        "identity": {
            "item_ref": verified.item_ref,
            "file_ref": verified.file_ref,
            "item_revision_sha256": verified.manifest_sha256,
            "file_revision_sha256": verified.sha256,
            "rights_record_ref": verified.rights_ref,
            "rights_revision_sha256": rights_revision,
        },
        "source": {
            "relative_path": verified.relative_path,
            "byte_size": verified.byte_size,
            "sha256": verified.sha256,
            "media_type": verified.media_type,
        },
        "storage": {
            "provider": "cloudflare-r2",
            "bucket_alias": bucket_alias,
            "object_key": verified.object_key,
            "storage_class": "Standard",
            "remote_status": remote_status,
        },
        "verification": {
            "local_verified": True,
            "upload_attempted": upload_attempted,
            "readback_verified": True,
            "readback_sha256": verified.sha256,
            "readback_byte_size": verified.byte_size,
            "verified_at": readback_verified_at,
        },
        "publication": {
            "publication_id": publication_id(verified.item_ref, verified.file_ref, rights_revision),
            "status": "disabled",
            "access_class": plan["access_class"],
            "rights_revision_sha256": rights_revision,
        },
    }
    _add_agent_review_receipt_fields(receipt, plan)
    return receipt


def _add_agent_review_receipt_fields(
    receipt: dict[str, Any],
    plan: dict[str, Any],
) -> None:
    if plan["rights_policy"]["review_status"] == "agent-reviewed":
        review_info = plan["rights_policy"]["rights_review"]
        receipt["identity"]["rights_review_ref"] = review_info["ref"]
        receipt["identity"]["rights_review_sha256"] = review_info["sha256"]
        receipt["identity"]["rights_review_actor_kind"] = "model"


def validate_receipt(receipt: dict[str, Any], *, repo_root: Path = REPO_ROOT) -> None:
    validate_json_schema(receipt, _schema_path(repo_root, RECEIPT_SCHEMA), error_type=PlanError)
    if receipt["storage"]["object_key"] != (
        f"blobs/sha256/{receipt['source']['sha256'][:2]}/{receipt['source']['sha256']}"
    ):
        raise PlanError("receipt object key is not content-addressed by source SHA-256")
    if receipt["verification"]["readback_sha256"] != receipt["source"]["sha256"]:
        raise PlanError("receipt readback digest does not match source digest")
    if receipt["publication"]["rights_revision_sha256"] != receipt["identity"]["rights_revision_sha256"]:
        raise PlanError("receipt publication rights revision mismatch")
    if receipt["identity"]["file_revision_sha256"] != receipt["source"]["sha256"]:
        raise PlanError("receipt file revision mismatch")
    if receipt["identity"]["item_revision_sha256"] == "":
        raise PlanError("receipt item revision is empty")
    review_fields = {
        "rights_review_ref",
        "rights_review_sha256",
        "rights_review_actor_kind",
    }
    present_review_fields = review_fields.intersection(receipt["identity"])
    if present_review_fields and present_review_fields != review_fields:
        raise PlanError("receipt has incomplete agent rights-review identity")


def _receipt_binding_matches(receipt: dict[str, Any], context: PlanContext, verified: VerifiedFile) -> bool:
    identity = receipt["identity"]
    review_binding = True
    if context.plan["rights_policy"]["review_status"] == "agent-reviewed":
        review_info = context.plan["rights_policy"]["rights_review"]
        review_binding = (
            identity.get("rights_review_ref") == review_info["ref"]
            and identity.get("rights_review_sha256") == review_info["sha256"]
            and identity.get("rights_review_actor_kind") == "model"
        )
    return (
        receipt["server_import_id"] == context.plan["server_import_id"]
        and receipt["plan"]["sha256"] == context.plan_sha256
        and identity["item_ref"] == verified.item_ref
        and identity["file_ref"] == verified.file_ref
        and identity["item_revision_sha256"] == verified.manifest_sha256
        and identity["file_revision_sha256"] == verified.sha256
        and identity["rights_revision_sha256"] == verified.rights_sha256
        and receipt["storage"]["object_key"] == verified.object_key
        and receipt["verification"]["readback_verified"] is True
        and review_binding
    )


def validate_receipt_against_plan(context: PlanContext, receipt: dict[str, Any]) -> None:
    """Recheck current Item/File/rights revisions before a registry read."""

    if receipt["plan"]["sha256"] != context.plan_sha256:
        raise PublicationError("current plan digest differs from imported receipt")
    manifest_path, manifest, rights_path, _rights = _manifest_and_rights(context)
    if manifest.get("item_id") != receipt["identity"]["item_ref"]:
        raise PublicationError("current Item differs from receipt")
    if context.plan["access_class"] != receipt["publication"]["access_class"]:
        raise PublicationError("current access class differs from receipt")
    if context.plan["manifest"]["sha256"] != receipt["identity"]["item_revision_sha256"]:
        raise PublicationError("current manifest revision differs from receipt")
    if context.plan["rights_policy"]["rights_record_sha256"] != receipt["identity"]["rights_revision_sha256"]:
        raise PublicationError("current rights revision differs from receipt")
    planned_file_refs = {entry["file_ref"] for entry in context.plan["payload_files"]}
    if receipt["identity"]["file_ref"] not in planned_file_refs:
        raise PublicationError("receipt File is outside the current plan")
    if context.plan["server_import_status"] not in TRANSFER_STATUSES:
        raise PublicationError("current server-import status no longer permits source reads")
    if context.plan["publication_status"] in {"withdrawn", "metadata-only"}:
        raise PublicationError("current publication status disables source reads")
    if context.plan["rights_policy"]["review_status"] == "agent-reviewed":
        review_identity = receipt["identity"]
        review_info = context.plan["rights_policy"]["rights_review"]
        if (
            review_identity.get("rights_review_ref") != review_info["ref"]
            or review_identity.get("rights_review_sha256") != review_info["sha256"]
            or review_identity.get("rights_review_actor_kind") != "model"
        ):
            raise PublicationError("current agent rights-review revision differs from receipt")
        _validate_agent_review(context, manifest_path=manifest_path, rights_path=rights_path)
    elif context.plan["rights_policy"]["review_status"] not in REVIEWED_RIGHTS:
        raise PublicationError("current legacy rights record is not reviewed")
    elif any(
        field in receipt["identity"]
        for field in ("rights_review_ref", "rights_review_sha256", "rights_review_actor_kind")
    ):
        raise PublicationError("legacy receipt carries an agent rights-review identity")
    _assert_rights_record_compatible(context, _rights)
    _parse_expiry(context.plan["rights_policy"].get("expires_at"), datetime.now(timezone.utc))


def import_plan(
    context: PlanContext,
    *,
    payload_source_root: Path,
    transport: TransportAdapter,
    receipt_dir: Path,
    bucket_alias: str,
    payload_source_layout: str = "source-witness",
    file_ref: str | None = None,
    now: datetime | None = None,
    scratch_root: Path | None = None,
) -> list[ImportOutcome]:
    """Transfer every selected file and return immutable receipt paths."""

    if scratch_root is None:
        raise LocalIntegrityError("import requires an explicit managed scratch_root")
    scratch_root = _checked_scratch_root(scratch_root)
    enforce_transfer_gate(context.plan, context=context, now=now)
    verified_files = verify_local(
        context,
        payload_source_root=payload_source_root,
        payload_source_layout=payload_source_layout,
        file_ref=file_ref,
    )
    outcomes: list[ImportOutcome] = []
    receipt_dir.mkdir(parents=True, exist_ok=True)
    for verified in verified_files:
        with _import_lock(receipt_dir):
            with tempfile.TemporaryDirectory(prefix="tos-r2-readback-", dir=str(scratch_root)) as temp_dir:
                snapshot = _snapshot_verified_payload(verified, Path(temp_dir))
                probe_path = Path(temp_dir) / "probe.bin"
                # The remote existence check and the upload/readback are one
                # operator-local critical section. Wrangler itself has no
                # conditional-create primitive, so this does not claim a
                # cross-host compare-and-swap.
                existing = transport.fetch(verified.object_key, probe_path)
                if existing:
                    _verify_download(probe_path, verified.byte_size, verified.sha256)
                    remote_status = "already-matched"
                    upload_attempted = False
                else:
                    transport.put(
                        verified.object_key,
                        snapshot,
                        byte_size=verified.byte_size,
                        media_type=verified.media_type,
                        storage_class="Standard",
                    )
                    upload_attempted = True
                    readback_path = Path(temp_dir) / "readback.bin"
                    if not transport.fetch(verified.object_key, readback_path):
                        raise RemoteIntegrityError("uploaded object was unavailable on readback")
                    _verify_download(readback_path, verified.byte_size, verified.sha256)
                    remote_status = "uploaded"
                receipt = _receipt(
                    context,
                    verified,
                    bucket_alias=bucket_alias,
                    remote_status=remote_status,
                    upload_attempted=upload_attempted,
                    readback_verified_at=utc_now(),
                )
                validate_receipt(receipt, repo_root=context.repo_root)
                receipt_path = receipt_dir / receipt_filename(
                    context.plan["server_import_id"], verified.sha256, context.plan_sha256
                )
                if receipt_path.exists():
                    existing_receipt = load_receipt(receipt_path, repo_root=context.repo_root)
                    if not _receipt_binding_matches(existing_receipt, context, verified):
                        raise ReceiptConflict(f"receipt binding conflict: {receipt_path.name}")
                    outcomes.append(
                        ImportOutcome(
                            receipt_path=receipt_path,
                            remote_status=remote_status,
                            upload_attempted=upload_attempted,
                            receipt_reused=True,
                        )
                    )
                else:
                    try:
                        write_json(receipt_path, receipt, immutable=True)
                    except ReceiptConflict:
                        # A foreign writer may have published the same exact
                        # receipt after our existence check. Re-read and
                        # compare the stable binding; timestamps are per-run
                        # outcomes and must not create a false conflict.
                        if not receipt_path.exists():
                            raise
                        existing_receipt = load_receipt(receipt_path, repo_root=context.repo_root)
                        if not _receipt_binding_matches(existing_receipt, context, verified):
                            raise
                        outcomes.append(
                            ImportOutcome(
                                receipt_path=receipt_path,
                                remote_status=remote_status,
                                upload_attempted=upload_attempted,
                                receipt_reused=True,
                            )
                        )
                    else:
                        outcomes.append(
                            ImportOutcome(
                                receipt_path=receipt_path,
                                remote_status=remote_status,
                                upload_attempted=upload_attempted,
                                receipt_reused=False,
                            )
                        )
    return outcomes


def load_receipt(path: Path, *, repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    receipt = load_json(path)
    validate_receipt(receipt, repo_root=repo_root)
    return receipt


def _registry_default() -> dict[str, Any]:
    return {"schema_version": "tos_source_publication_registry_v1", "entries": []}


def _validate_registry(value: dict[str, Any]) -> dict[str, Any]:
    if value.get("schema_version") != "tos_source_publication_registry_v1":
        raise PublicationError("unsupported publication registry schema")
    entries = value.get("entries")
    if not isinstance(entries, list):
        raise PublicationError("publication registry entries must be an array")
    for entry in entries:
        if not isinstance(entry, dict):
            raise PublicationError("publication registry entry must be an object")
        if entry.get("status") not in {"enabled", "disabled", "revoked"}:
            raise PublicationError("publication registry has an invalid status")
        for field in ("publication_id", "item_ref", "file_ref", "rights_revision_sha256", "receipt_id"):
            if not isinstance(entry.get(field), str) or not entry[field]:
                raise PublicationError(f"publication registry entry lacks {field}")
    return value


def _load_registry_unlocked(path: Path) -> dict[str, Any]:
    if not path.exists():
        return _registry_default()
    return _validate_registry(load_json(path))


@contextmanager
def _registry_lock(path: Path, *, exclusive: bool) -> Iterable[None]:
    """Serialize registry writers and give readers a coherent snapshot."""

    path.parent.mkdir(parents=True, exist_ok=True)
    lock_path = path.with_name(f".{path.name}.lock")
    with lock_path.open("a+") as stream:
        fcntl.flock(stream.fileno(), fcntl.LOCK_EX if exclusive else fcntl.LOCK_SH)
        try:
            yield
        finally:
            fcntl.flock(stream.fileno(), fcntl.LOCK_UN)


def load_registry(path: Path) -> dict[str, Any]:
    with _registry_lock(path, exclusive=False):
        return _load_registry_unlocked(path)


def _find_registry_entry(registry: dict[str, Any], publication: str) -> dict[str, Any]:
    for entry in registry["entries"]:
        if entry["publication_id"] == publication or entry["receipt_id"] == publication:
            return entry
    raise PublicationError(f"publication is not registered: {publication}")


def enable_publication(
    receipt: dict[str, Any],
    *,
    registry_path: Path,
    mode: str,
    now: str | None = None,
) -> dict[str, Any]:
    if mode not in {"public-payload", "controlled-research"}:
        raise PublicationError("publication mode must be public-payload or controlled-research")
    if receipt["publication"]["status"] != "disabled":
        raise PublicationError("only a disabled receipt can be enabled")
    if receipt["identity"]["rights_revision_sha256"] != receipt["publication"]["rights_revision_sha256"]:
        raise PublicationError("publication rights revision is not bound to receipt identity")
    if receipt["identity"]["file_revision_sha256"] != receipt["source"]["sha256"]:
        raise PublicationError("publication file revision is not bound to receipt identity")
    if receipt["publication"]["access_class"] != mode:
        raise PublicationError("requested publication mode does not match exact plan access class")
    pub_id = receipt["publication"]["publication_id"]
    with _registry_lock(registry_path, exclusive=True):
        registry = _load_registry_unlocked(registry_path)
        existing = [entry for entry in registry["entries"] if entry["publication_id"] == pub_id]
        if existing:
            entry = existing[0]
            if any(
                entry.get(field) != expected
                for field, expected in {
                    "receipt_id": receipt["receipt_id"],
                    "item_ref": receipt["identity"]["item_ref"],
                    "file_ref": receipt["identity"]["file_ref"],
                    "file_revision_sha256": receipt["identity"]["file_revision_sha256"],
                    "rights_revision_sha256": receipt["identity"]["rights_revision_sha256"],
                    "mode": mode,
                }.items()
            ):
                raise PublicationError("publication registry has a conflicting immutable identity")
            if entry["status"] == "enabled":
                return dict(entry)
            entry.update({"status": "enabled", "enabled_at": now or utc_now()})
        else:
            entry = {
                "publication_id": pub_id,
                "receipt_id": receipt["receipt_id"],
                "item_ref": receipt["identity"]["item_ref"],
                "file_ref": receipt["identity"]["file_ref"],
                "file_revision_sha256": receipt["identity"]["file_revision_sha256"],
                "rights_revision_sha256": receipt["identity"]["rights_revision_sha256"],
                "mode": mode,
                "status": "enabled",
                "enabled_at": now or utc_now(),
            }
            registry["entries"].append(entry)
        write_json(registry_path, registry)
        return dict(entry)


def disable_publication(
    *,
    registry_path: Path,
    publication: str,
    reason: str,
    revoked: bool = False,
    now: str | None = None,
) -> dict[str, Any]:
    if not reason.strip():
        raise PublicationError("disable/revoke requires a reason")
    with _registry_lock(registry_path, exclusive=True):
        registry = _load_registry_unlocked(registry_path)
        entry = _find_registry_entry(registry, publication)
        entry["status"] = "revoked" if revoked else "disabled"
        entry["disabled_at"] = now or utc_now()
        entry["reason"] = reason
        write_json(registry_path, registry)
        return dict(entry)


def _require_enabled(registry_path: Path, publication: str, receipt: dict[str, Any]) -> None:
    registry = load_registry(registry_path)
    entry = _find_registry_entry(registry, publication)
    if entry["status"] != "enabled":
        raise PublicationError(f"publication is {entry['status']}")
    if entry["receipt_id"] != receipt["receipt_id"]:
        raise PublicationError("registry receipt does not match requested receipt")
    if entry["item_ref"] != receipt["identity"]["item_ref"] or entry["file_ref"] != receipt["identity"]["file_ref"]:
        raise PublicationError("registry Item/File identity mismatch")
    if entry["rights_revision_sha256"] != receipt["identity"]["rights_revision_sha256"]:
        raise PublicationError("registry rights revision mismatch")


def read_local(
    context: PlanContext,
    receipt: dict[str, Any],
    *,
    registry_path: Path,
    payload_source_root: Path,
    payload_source_layout: str = "source-witness",
    output: Path,
) -> None:
    _require_enabled(registry_path, receipt["publication"]["publication_id"], receipt)
    validate_receipt_against_plan(context, receipt)
    files = verify_local(
        context,
        payload_source_root=payload_source_root,
        payload_source_layout=payload_source_layout,
        file_ref=receipt["identity"]["file_ref"],
    )
    matches = [item for item in files if item.sha256 == receipt["source"]["sha256"]]
    if len(matches) != 1:
        raise LocalIntegrityError("enabled publication does not resolve to the receipt bytes")
    _publish_verified_output(
        matches[0].local_path,
        output,
        byte_size=receipt["source"]["byte_size"],
        sha256=receipt["source"]["sha256"],
    )


def read_remote(
    receipt: dict[str, Any],
    *,
    transport: TransportAdapter,
    output: Path,
    registry_path: Path | None = None,
    publication: str | None = None,
    context: PlanContext | None = None,
    scratch_root: Path | None = None,
) -> None:
    if registry_path is not None:
        if context is None:
            raise PublicationError("registry-protected remote reads require the current plan")
        validate_receipt_against_plan(context, receipt)
        _require_enabled(
            registry_path,
            publication or receipt["publication"]["publication_id"],
            receipt,
        )
    if scratch_root is not None:
        scratch_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="tos-r2-cli-readback-",
        dir=str(scratch_root) if scratch_root is not None else None,
    ) as temp_dir:
        temporary = Path(temp_dir) / "remote.bin"
        if not transport.fetch(receipt["storage"]["object_key"], temporary):
            raise RemoteIntegrityError("receipt object is unavailable")
        _verify_download(temporary, receipt["source"]["byte_size"], receipt["source"]["sha256"])
        _publish_verified_output(
            temporary,
            output,
            byte_size=receipt["source"]["byte_size"],
            sha256=receipt["source"]["sha256"],
        )


def _default_wrangle_path() -> Path:
    return REPO_ROOT / "access/deploy/cloudflare-worker/node_modules/.bin/wrangler"


def _common_parser(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--plan", type=Path, required=True)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    verify = commands.add_parser("verify-local", help="verify a frozen plan and local bytes")
    _common_parser(verify)
    verify.add_argument("--payload-source-root", type=Path, required=True)
    verify.add_argument("--payload-source-layout", choices=("repo", "source-witness", "item"), default="source-witness")
    verify.add_argument("--file-ref")

    transfer = commands.add_parser("import", help="rights-gated R2 transfer and readback")
    _common_parser(transfer)
    transfer.add_argument("--payload-source-root", type=Path, required=True)
    transfer.add_argument("--payload-source-layout", choices=("repo", "source-witness", "item"), default="source-witness")
    transfer.add_argument("--file-ref")
    transfer.add_argument("--receipt-dir", type=Path, required=True)
    transfer.add_argument("--bucket", required=True)
    transfer.add_argument("--bucket-alias", default="private-source-payloads")
    transfer.add_argument("--wrangler", type=Path, default=_default_wrangle_path())
    transfer.add_argument("--max-upload-bytes", type=int, default=WRANGLER_MAX_UPLOAD_BYTES)
    transfer.add_argument("--scratch-root", type=Path, required=True, help="managed temporary directory for verified snapshots and readbacks")
    transfer.add_argument("--confirm-transfer", action="store_true", help="required explicit operator invocation")

    read_local_parser = commands.add_parser("read-local", help="read an enabled publication from local custody")
    _common_parser(read_local_parser)
    read_local_parser.add_argument("--receipt", type=Path, required=True)
    read_local_parser.add_argument("--registry", type=Path, required=True)
    read_local_parser.add_argument("--payload-source-root", type=Path, required=True)
    read_local_parser.add_argument("--payload-source-layout", choices=("repo", "source-witness", "item"), default="source-witness")
    read_local_parser.add_argument("--output", type=Path, required=True)

    read_remote_parser = commands.add_parser("read-remote", help="read and verify one uploaded object without the site")
    read_remote_parser.add_argument("--receipt", type=Path, required=True)
    read_remote_parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    read_remote_parser.add_argument("--plan", type=Path, help="required with --registry for currentness checks")
    read_remote_parser.add_argument("--bucket", required=True)
    read_remote_parser.add_argument("--wrangler", type=Path, default=_default_wrangle_path())
    read_remote_parser.add_argument("--output", type=Path, required=True)
    read_remote_parser.add_argument("--registry", type=Path)
    read_remote_parser.add_argument("--publication")
    read_remote_parser.add_argument("--scratch-root", type=Path)

    enable = commands.add_parser("registry-enable", help="enable one exact imported publication")
    enable.add_argument("--receipt", type=Path, required=True)
    enable.add_argument("--registry", type=Path, required=True)
    enable.add_argument("--mode", choices=("public-payload", "controlled-research"), required=True)
    enable.add_argument("--repo-root", type=Path, default=REPO_ROOT)

    disable = commands.add_parser("registry-disable", help="disable or revoke a publication")
    disable.add_argument("--registry", type=Path, required=True)
    disable.add_argument("--publication", required=True)
    disable.add_argument("--reason", required=True)
    disable.add_argument("--revoke", action="store_true")

    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "verify-local":
            context = load_plan(args.repo_root, args.plan)
            files = verify_local(
                context,
                payload_source_root=args.payload_source_root,
                payload_source_layout=args.payload_source_layout,
                file_ref=args.file_ref,
            )
            print(
                json.dumps(
                    {
                        "status": "verified-local",
                        "server_import_id": context.plan["server_import_id"],
                        "plan_sha256": context.plan_sha256,
                        "files": [
                            {
                                "item_ref": item.item_ref,
                                "file_ref": item.file_ref,
                                "relative_path": item.relative_path,
                                "byte_size": item.byte_size,
                                "sha256": item.sha256,
                            }
                            for item in files
                        ],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0

        if args.command == "import":
            if not args.confirm_transfer:
                raise RightsGateError("import requires --confirm-transfer; verify-local is read-only")
            context = load_plan(args.repo_root, args.plan)
            transport = WranglerR2Transport(
                bucket=args.bucket,
                executable=args.wrangler,
                max_upload_bytes=args.max_upload_bytes,
                cwd=args.repo_root,
            )
            receipts = import_plan(
                context,
                payload_source_root=args.payload_source_root,
                payload_source_layout=args.payload_source_layout,
                file_ref=args.file_ref,
                transport=transport,
                receipt_dir=args.receipt_dir,
                bucket_alias=args.bucket_alias,
                scratch_root=args.scratch_root,
            )
            print(
                json.dumps(
                    {
                        "status": "imported",
                        "receipts": [
                            {
                                "path": outcome.receipt_path.name,
                                "remote_status": outcome.remote_status,
                                "upload_attempted": outcome.upload_attempted,
                                "receipt_reused": outcome.receipt_reused,
                            }
                            for outcome in receipts
                        ],
                    },
                    indent=2,
                )
            )
            return 0

        if args.command == "read-local":
            context = load_plan(args.repo_root, args.plan)
            receipt = load_receipt(args.receipt, repo_root=args.repo_root)
            read_local(
                context,
                receipt,
                registry_path=args.registry,
                payload_source_root=args.payload_source_root,
                payload_source_layout=args.payload_source_layout,
                output=args.output,
            )
            print(json.dumps({"status": "read-local-verified", "receipt_id": receipt["receipt_id"]}, indent=2))
            return 0

        if args.command == "read-remote":
            receipt = load_receipt(args.receipt, repo_root=args.repo_root)
            context = None
            if args.registry is not None:
                if args.plan is None:
                    raise PublicationError("--plan is required with registry-protected remote reads")
                context = load_plan(args.repo_root, args.plan)
            transport = WranglerR2Transport(bucket=args.bucket, executable=args.wrangler, cwd=args.repo_root)
            read_remote(
                receipt,
                transport=transport,
                output=args.output,
                registry_path=args.registry,
                publication=args.publication,
                context=context,
                scratch_root=args.scratch_root,
            )
            print(json.dumps({"status": "read-remote-verified", "receipt_id": receipt["receipt_id"]}, indent=2))
            return 0

        if args.command == "registry-enable":
            receipt = load_receipt(args.receipt, repo_root=args.repo_root)
            entry = enable_publication(receipt, registry_path=args.registry, mode=args.mode)
            print(json.dumps(entry, ensure_ascii=False, indent=2))
            return 0

        if args.command == "registry-disable":
            entry = disable_publication(
                registry_path=args.registry,
                publication=args.publication,
                reason=args.reason,
                revoked=args.revoke,
            )
            print(json.dumps(entry, ensure_ascii=False, indent=2))
            return 0
    except SourcePayloadImportError as exc:
        print(f"source-payload-import: {exc}", file=sys.stderr)
        return 2
    return 2


if __name__ == "__main__":
    raise SystemExit(main())

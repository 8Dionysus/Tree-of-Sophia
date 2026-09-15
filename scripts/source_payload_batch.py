#!/usr/bin/env python3
"""Run a frozen source-payload plan set through one persistent R2 transport.

The batch command is a small command-plane wrapper around
``source_payload_import.import_plan``.  It verifies the plan-index digest,
loads every exact plan and local payload before opening a transport, and then
serializes plan completion through a durable JSONL journal.  The default mode
is verification only; ``--transfer`` is the explicit transfer boundary.

This module never discovers credentials or reads an account cache.  Transfer
mode receives an explicit account ID and Wrangler executable and lets the
REST adapter capture the existing OAuth token in process memory.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import time
from typing import Any, Callable, Iterable, Mapping, Protocol

try:
    import source_payload_import as importer
    from source_payload_r2 import R2RestTransport, R2TransportError
except ModuleNotFoundError as exc:
    if exc.name not in {"source_payload_import", "source_payload_r2"}:
        raise
    from scripts import source_payload_import as importer
    from scripts.source_payload_r2 import R2RestTransport, R2TransportError


_SHA256 = re.compile(r"^[0-9a-f]{64}$")
_ACCOUNT_ID = re.compile(r"^[0-9a-fA-F]{32}$")
_BUCKET = re.compile(r"^[a-z0-9](?:[a-z0-9-]{1,61}[a-z0-9])$")


class BatchError(importer.SourcePayloadImportError):
    """A frozen batch input or orchestration failure."""


class BatchTransport(Protocol):
    def fetch(self, object_key: str, destination: Path) -> bool:
        """Fetch one object, returning false only for a missing object."""

    def put(
        self,
        object_key: str,
        source: Path,
        *,
        byte_size: int,
        media_type: str,
        storage_class: str,
    ) -> None:
        """Upload one object under the importer transport contract."""

    def close(self) -> None:
        """Close the persistent connection."""


TransportFactory = Callable[[], BatchTransport]


@dataclass(frozen=True)
class FrozenPlan:
    ref: str
    sha256: str


@dataclass(frozen=True)
class PreflightPlan:
    context: importer.PlanContext
    file_count: int
    byte_count: int


@dataclass(frozen=True)
class BatchResult:
    summary_path: Path
    journal_path: Path
    summary: dict[str, Any]


def _strict_pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in items:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result


def _digest(value: str, *, label: str) -> str:
    if not isinstance(value, str) or not _SHA256.fullmatch(value):
        raise BatchError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _read_frozen_plans(
    plans_file: Path,
    *,
    expected_plans_sha256: str,
) -> tuple[list[FrozenPlan], str]:
    expected = _digest(expected_plans_sha256, label="--expected-plans-sha256")
    try:
        raw = plans_file.read_bytes()
    except OSError as exc:
        raise BatchError("cannot read frozen plans file") from exc
    actual = hashlib.sha256(raw).hexdigest()
    if actual != expected:
        raise BatchError("frozen plans file SHA-256 does not match expected digest")
    try:
        value = json.loads(raw, object_pairs_hook=_strict_pairs)
    except (TypeError, ValueError):
        raise BatchError("frozen plans file is not valid JSON") from None
    if not isinstance(value, dict) or set(value) != {"plans"}:
        raise BatchError("frozen plans file must contain exactly the plans field")
    entries = value["plans"]
    if not isinstance(entries, list) or not entries:
        raise BatchError("frozen plans file must contain a non-empty plans array")
    seen: set[str] = set()
    plans: list[FrozenPlan] = []
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != {"ref", "sha256"}:
            raise BatchError("each frozen plan entry must contain exactly ref and sha256")
        ref = entry["ref"]
        if not isinstance(ref, str) or not ref:
            raise BatchError("frozen plan ref must be a non-empty string")
        if ref in seen:
            raise BatchError("frozen plans file contains a duplicate plan ref")
        seen.add(ref)
        plans.append(FrozenPlan(ref=ref, sha256=_digest(entry["sha256"], label="plan sha256")))
    return plans, actual


def _validate_runtime_inputs(
    *,
    repo_root: Path,
    plans_file: Path,
    payload_source_root: Path,
    scratch_root: Path,
    receipt_dir: Path,
    output_dir: Path,
    bucket: str,
    account_id: str,
    wrangler: Path,
) -> None:
    if not isinstance(repo_root, Path) or not repo_root.is_absolute():
        raise BatchError("--repo-root must be an absolute path")
    if not isinstance(plans_file, Path) or not plans_file.is_absolute():
        raise BatchError("--plans-file must be an absolute path")
    if not isinstance(payload_source_root, Path) or not payload_source_root.is_absolute():
        raise BatchError("--payload-source-root must be an absolute path")
    if not isinstance(scratch_root, Path) or not scratch_root.is_absolute():
        raise BatchError("--scratch-root must be an absolute path")
    if not isinstance(receipt_dir, Path) or not receipt_dir.is_absolute():
        raise BatchError("--receipt-dir must be an absolute path")
    if not isinstance(output_dir, Path) or not output_dir.is_absolute():
        raise BatchError("--output must be an absolute path")
    if not isinstance(account_id, str) or not _ACCOUNT_ID.fullmatch(account_id):
        raise BatchError("--account-id must be 32 hexadecimal characters")
    if not isinstance(bucket, str) or not _BUCKET.fullmatch(bucket):
        raise BatchError("--bucket is not a valid R2 bucket name")
    if not isinstance(wrangler, Path) or not wrangler.is_absolute():
        raise BatchError("--wrangler must be an absolute path")


def _preflight(
    *,
    repo_root: Path,
    frozen_plans: Iterable[FrozenPlan],
    payload_source_root: Path,
    payload_source_layout: str,
) -> tuple[list[PreflightPlan], int, int]:
    prepared: list[PreflightPlan] = []
    total_files = 0
    total_bytes = 0
    for frozen in frozen_plans:
        plan_path = importer.safe_repo_path(repo_root, frozen.ref)
        if not plan_path.is_file():
            raise BatchError(f"frozen plan is missing: {frozen.ref}")
        if importer.sha256_file(plan_path) != frozen.sha256:
            raise BatchError(f"frozen plan digest mismatch: {frozen.ref}")
        context = importer.load_plan(repo_root, plan_path)
        if context.plan_ref != frozen.ref or context.plan_sha256 != frozen.sha256:
            raise BatchError(f"loaded plan identity differs from frozen index: {frozen.ref}")
        importer.enforce_transfer_gate(context.plan, context=context)
        verified = importer.verify_local(
            context,
            payload_source_root=payload_source_root,
            payload_source_layout=payload_source_layout,
        )
        file_count = len(verified)
        byte_count = sum(item.byte_size for item in verified)
        prepared.append(
            PreflightPlan(
                context=context,
                file_count=file_count,
                byte_count=byte_count,
            )
        )
        total_files += file_count
        total_bytes += byte_count
    return prepared, total_files, total_bytes


def _atomic_json_write(path: Path, value: Mapping[str, Any]) -> None:
    data = (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode(
        "utf-8"
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        try:
            directory_fd = os.open(path.parent, os.O_DIRECTORY)
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


def _append_journal(stream: Any, entry: Mapping[str, Any]) -> None:
    stream.write(json.dumps(entry, ensure_ascii=False, sort_keys=True) + "\n")
    stream.flush()
    os.fsync(stream.fileno())


def _new_output_paths(output_dir: Path) -> tuple[Path, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    for _ in range(5):
        run_id = str(time.time_ns())
        journal = output_dir / f"batch-{run_id}.jsonl"
        summary = output_dir / f"batch-{run_id}.json"
        if not journal.exists() and not summary.exists():
            return summary, journal
        time.sleep(0.001)
    raise BatchError("could not allocate a unique batch output id")


def _default_transport_factory(
    *,
    repo_root: Path,
    bucket: str,
    account_id: str,
    wrangler: Path,
) -> TransportFactory:
    def create() -> BatchTransport:
        return R2RestTransport(
            account_id=account_id,
            bucket=bucket,
            executable=wrangler,
            cwd=repo_root,
        )

    return create


def run_batch(
    *,
    repo_root: Path,
    plans_file: Path,
    expected_plans_sha256: str,
    payload_source_root: Path,
    scratch_root: Path,
    receipt_dir: Path,
    bucket: str,
    account_id: str,
    wrangler: Path,
    output_dir: Path,
    transfer: bool = False,
    payload_source_layout: str = "source-witness",
    transport_factory: TransportFactory | None = None,
) -> BatchResult:
    """Preflight an exact plan closure and optionally transfer it.

    All plans and local payloads are verified before a transport is created.
    Transfer mode keeps one transport for the complete plan set. Receipt
    reuse and remote readback remain owned by :func:`importer.import_plan`.
    """

    _validate_runtime_inputs(
        repo_root=repo_root,
        plans_file=plans_file,
        payload_source_root=payload_source_root,
        scratch_root=scratch_root,
        receipt_dir=receipt_dir,
        output_dir=output_dir,
        bucket=bucket,
        account_id=account_id,
        wrangler=wrangler,
    )
    frozen_plans, plans_digest = _read_frozen_plans(
        plans_file,
        expected_plans_sha256=expected_plans_sha256,
    )
    prepared, total_files, total_bytes = _preflight(
        repo_root=repo_root,
        frozen_plans=frozen_plans,
        payload_source_root=payload_source_root,
        payload_source_layout=payload_source_layout,
    )
    summary_path, journal_path = _new_output_paths(output_dir)
    summary: dict[str, Any] = {
        "schema": "tos_source_payload_batch_summary_v1",
        "started_at": importer.utc_now(),
        "plans_file_sha256": plans_digest,
        "plans": len(prepared),
        "files": total_files,
        "bytes": total_bytes,
        "transfer_requested": transfer,
        "phase": "transfer" if transfer else "verify-only",
        "complete": not transfer,
        "completed_plans": 0,
        "completed_files": 0,
        "local_verified_files": total_files,
        "uploaded_files": 0,
        "reused_receipts": 0,
        "journal": journal_path.name,
    }

    transport: BatchTransport | None = None
    journal_stream: Any | None = None
    try:
        journal_stream = journal_path.open("x", encoding="utf-8")
        _atomic_json_write(summary_path, summary)
        if transfer:
            factory = transport_factory or _default_transport_factory(
                repo_root=repo_root,
                bucket=bucket,
                account_id=account_id,
                wrangler=wrangler,
            )
            transport = factory()
            for item in prepared:
                if importer.sha256_file(item.context.path) != item.context.plan_sha256:
                    raise BatchError(
                        f"plan changed during batch: {item.context.plan_ref}"
                    )
                outcomes = importer.import_plan(
                    item.context,
                    payload_source_root=payload_source_root,
                    payload_source_layout=payload_source_layout,
                    transport=transport,
                    receipt_dir=receipt_dir,
                    bucket_alias=bucket,
                    scratch_root=scratch_root,
                )
                journal_entry = {
                    "plan_ref": item.context.plan_ref,
                    "plan_sha256": item.context.plan_sha256,
                    "completed_at": importer.utc_now(),
                    "files": len(outcomes),
                    "uploaded_files": sum(outcome.upload_attempted for outcome in outcomes),
                    "reused_receipts": sum(outcome.receipt_reused for outcome in outcomes),
                    "outcomes": [
                        {
                            "receipt": outcome.receipt_path.name,
                            "remote_status": outcome.remote_status,
                            "upload_attempted": outcome.upload_attempted,
                            "receipt_reused": outcome.receipt_reused,
                        }
                        for outcome in outcomes
                    ],
                }
                _append_journal(journal_stream, journal_entry)
                summary["completed_plans"] += 1
                summary["completed_files"] += len(outcomes)
                summary["uploaded_files"] += journal_entry["uploaded_files"]
                summary["reused_receipts"] += journal_entry["reused_receipts"]
                _atomic_json_write(summary_path, summary)
            summary["complete"] = True
        _atomic_json_write(summary_path, summary)
    except Exception as exc:
        summary["complete"] = False
        summary["phase"] = "failed"
        summary["failure_type"] = type(exc).__name__
        if isinstance(exc, (BatchError, importer.SourcePayloadImportError, R2TransportError)):
            summary["failure_message"] = str(exc)
        raise
    finally:
        if transport is not None:
            transport.close()
        if journal_stream is not None:
            journal_stream.close()
        summary["finished_at"] = importer.utc_now()
        _atomic_json_write(summary_path, summary)
    return BatchResult(summary_path=summary_path, journal_path=journal_path, summary=summary)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, required=True)
    parser.add_argument("--plans-file", type=Path, required=True)
    parser.add_argument("--expected-plans-sha256", required=True)
    parser.add_argument("--payload-source-root", type=Path, required=True)
    parser.add_argument("--payload-source-layout", choices=("repo", "source-witness", "item"), default="source-witness")
    parser.add_argument("--scratch-root", type=Path, required=True)
    parser.add_argument("--receipt-dir", type=Path, required=True)
    parser.add_argument("--bucket", required=True)
    parser.add_argument("--account-id", required=True)
    parser.add_argument("--wrangler", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--transfer", action="store_true", help="perform the approved transfer after full preflight")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        result = run_batch(
            repo_root=args.repo_root,
            plans_file=args.plans_file,
            expected_plans_sha256=args.expected_plans_sha256,
            payload_source_root=args.payload_source_root,
            scratch_root=args.scratch_root,
            receipt_dir=args.receipt_dir,
            bucket=args.bucket,
            account_id=args.account_id,
            wrangler=args.wrangler,
            output_dir=args.output,
            transfer=args.transfer,
            payload_source_layout=args.payload_source_layout,
        )
    except (BatchError, importer.SourcePayloadImportError, R2TransportError) as exc:
        print(f"source-payload-batch: {exc}", file=sys.stderr)
        return 2
    print(
        json.dumps(
            {
                "status": "transferred" if args.transfer else "verified-only",
                "summary": result.summary_path.name,
                "journal": result.journal_path.name,
                "plans": result.summary["plans"],
                "files": result.summary["files"],
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

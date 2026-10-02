#!/usr/bin/env python3
"""Thin compatibility facade for native acquisition handoff verification."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import sys
from typing import Any

try:
    import acquisition_batch as acquisition
    import acquisition_native
except ModuleNotFoundError as exc:  # pragma: no cover - package import fallback
    if exc.name not in {"acquisition_batch", "acquisition_native"}:
        raise
    from scripts import acquisition_batch as acquisition
    from scripts import acquisition_native


class HandoffAdapterError(ValueError):
    """The selected acquisition handoff cannot form a closed batch input."""


@dataclass(frozen=True)
class VerifiedHandoff:
    """Legacy public result shape for a verified intake handoff."""

    root: Path
    handoff_path: Path
    handoff: dict[str, Any]
    context: acquisition.BatchContext
    selected_source_rows: list[dict[str, Any]]
    payloads: list[dict[str, Any]]


def _invoke(operation: str, **fields: Any) -> Any:
    request = {"family": "handoff", "operation": operation, **fields}
    try:
        return acquisition_native.invoke(request)
    except acquisition_native.NativeAcquisitionError as exc:
        raise HandoffAdapterError(exc.message) from exc


def verify_validation_context(
    value: dict[str, Any] | None,
    *,
    validator_sha256: str,
) -> dict[str, Any]:
    """Verify and freeze the caller-selected grammar and historical context."""

    return _invoke(
        "validation-context",
        validation_context=value,
        validator_sha256=validator_sha256,
    )


def _batch_context(value: dict[str, Any]) -> acquisition.BatchContext:
    """Rehydrate only the legacy data carrier returned by the native owner."""

    import base64

    raw = base64.b64decode(value["raw_manifest_base64"], validate=True)
    return acquisition.BatchContext(
        Path(value["repo_root"]),
        Path(value["manifest_path"]),
        value["manifest_ref"],
        value["manifest_sha256"],
        raw,
        value["manifest"],
    )


def verify_handoff_for_intake(
    *,
    acquisition_root: Path | str,
    handoff_ref: str,
    expected_base_revision: str,
    expected_manifest_sha256: str,
    repo_root: Path | str = acquisition.REPO_ROOT,
) -> VerifiedHandoff:
    """Verify one handoff for direct intake closure without loading a store."""

    value = _invoke(
        "verify",
        acquisition_root=str(acquisition_root),
        handoff_ref=handoff_ref,
        expected_base_revision=expected_base_revision,
        expected_manifest_sha256=expected_manifest_sha256,
        repo_root=str(repo_root),
    )
    return VerifiedHandoff(
        root=Path(value["root"]),
        handoff_path=Path(value["handoff_path"]),
        handoff=value["handoff"],
        context=_batch_context(value["context"]),
        selected_source_rows=value["selected_source_rows"],
        payloads=value["payloads"],
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
    """Create one native-verified, not-admitted ``tos_corpus_batch_v1`` input."""

    return _invoke(
        "adapt",
        acquisition_root=str(acquisition_root),
        handoff_ref=handoff_ref,
        expected_manifest_sha256=expected_manifest_sha256,
        output_root=str(output_root),
        accepted_store_root=str(accepted_store_root),
        accepted_source_root=str(accepted_source_root),
        base_revision=base_revision,
        validator_sha256=validator_sha256,
        validation_context=validation_context,
        repo_root=str(repo_root),
    )


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
        result = _invoke(
            "adapt",
            acquisition_root=str(args.acquisition_root),
            handoff_ref=args.handoff,
            expected_manifest_sha256=args.expected_manifest_sha256,
            output_root=str(args.output_root),
            accepted_store_root=str(args.accepted_store_root),
            accepted_source_root=str(args.accepted_source_root),
            base_revision=args.base_revision,
            validator_sha256=args.validator_sha256,
            validation_context_path=str(args.validation_context),
            repo_root=str(args.repo_root),
        )
    except (HandoffAdapterError, acquisition.AcquisitionBatchError) as exc:
        print(f"acquisition-handoff-adapter: {exc}", file=sys.stderr)
        return 2
    except (OSError, UnicodeError, ValueError) as exc:
        print(f"acquisition-handoff-adapter: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

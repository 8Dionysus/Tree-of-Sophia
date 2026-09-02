#!/usr/bin/env python3
"""Validate reviewed open-work candidates, terminal receipts, and queue parity."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

from jsonschema import FormatChecker
from jsonschema.validators import validator_for

from open_work_candidate_queue_common import (
    LEDGER_PATH,
    QUEUE_RELATIVE_PATH,
    RECEIPT_ROOT,
    REPO_ROOT,
    TIMING_ROOT,
    QueueBuildError,
    _queue_sha256,
    build_payload,
    render_payload,
)


CONTRACT_ROOT = REPO_ROOT / "ToS/contracts"
CANDIDATE_SCHEMA = CONTRACT_ROOT / "open-work-candidate.schema.json"
RECEIPT_SCHEMA = CONTRACT_ROOT / "open-work-candidate-receipt.schema.json"
QUEUE_SCHEMA = CONTRACT_ROOT / "open-work-candidate-queue.schema.json"
TIMING_SCHEMA = CONTRACT_ROOT / "open-work-channel-timing-receipt.schema.json"


def _load_json(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("JSON root must be an object")
    return payload


def _validator(path: Path):
    schema = _load_json(path)
    validator_class = validator_for(schema)
    validator_class.check_schema(schema)
    return validator_class(schema, format_checker=FormatChecker())


def _format_error(error) -> str:
    location = "/".join(str(part) for part in error.absolute_path)
    return f"{location or '<root>'}: {error.message}"


def _validate_jsonl(path: Path, validator, issues: list[str]) -> None:
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not raw_line.strip():
            continue
        location = f"{path.relative_to(REPO_ROOT).as_posix()}:{line_number}"
        try:
            payload = json.loads(raw_line)
        except json.JSONDecodeError as exc:
            issues.append(f"{location}: invalid JSON: {exc}")
            continue
        for error in sorted(validator.iter_errors(payload), key=lambda item: list(item.absolute_path)):
            issues.append(f"{location}: {_format_error(error)}")


def main() -> int:
    issues: list[str] = []
    try:
        candidate_validator = _validator(CANDIDATE_SCHEMA)
        receipt_validator = _validator(RECEIPT_SCHEMA)
        queue_validator = _validator(QUEUE_SCHEMA)
        timing_validator = _validator(TIMING_SCHEMA)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"Open-work queue contract load failed: {exc}", file=sys.stderr)
        return 1

    ledger = REPO_ROOT / LEDGER_PATH
    try:
        _validate_jsonl(ledger, candidate_validator, issues)
    except OSError as exc:
        issues.append(f"{LEDGER_PATH.as_posix()}: cannot read candidate ledger: {exc}")

    receipt_root = REPO_ROOT / RECEIPT_ROOT
    if receipt_root.is_dir():
        for path in sorted(receipt_root.glob("*.json")):
            try:
                receipt = _load_json(path)
            except (OSError, ValueError, json.JSONDecodeError) as exc:
                issues.append(f"{path.relative_to(REPO_ROOT).as_posix()}: cannot read receipt: {exc}")
                continue
            for error in sorted(
                receipt_validator.iter_errors(receipt),
                key=lambda item: list(item.absolute_path),
            ):
                issues.append(
                    f"{path.relative_to(REPO_ROOT).as_posix()}: {_format_error(error)}"
                )

    timing_root = REPO_ROOT / TIMING_ROOT
    if timing_root.is_dir():
        for path in sorted(timing_root.glob("*.json")):
            try:
                timing = _load_json(path)
            except (OSError, ValueError, json.JSONDecodeError) as exc:
                issues.append(f"{path.relative_to(REPO_ROOT).as_posix()}: cannot read timing receipt: {exc}")
                continue
            for error in sorted(
                timing_validator.iter_errors(timing),
                key=lambda item: list(item.absolute_path),
            ):
                issues.append(
                    f"{path.relative_to(REPO_ROOT).as_posix()}: {_format_error(error)}"
                )

    queue_path = REPO_ROOT / QUEUE_RELATIVE_PATH
    try:
        queue = _load_json(queue_path)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        issues.append(f"{QUEUE_RELATIVE_PATH.as_posix()}: cannot read generated queue: {exc}")
        queue = None
    if queue is not None:
        for error in sorted(queue_validator.iter_errors(queue), key=lambda item: list(item.absolute_path)):
            issues.append(f"{QUEUE_RELATIVE_PATH.as_posix()}: {_format_error(error)}")
        if queue.get("queue_sha256") != _queue_sha256(queue):
            issues.append(f"{QUEUE_RELATIVE_PATH.as_posix()}: queue_sha256 does not match payload")

    try:
        expected = render_payload(build_payload(REPO_ROOT))
    except (QueueBuildError, KeyError, TypeError, AttributeError, IndexError, ValueError) as exc:
        issues.append(f"queue build failed closed: {type(exc).__name__}: {exc}")
    else:
        if queue_path.is_file() and queue_path.read_text(encoding="utf-8") != expected:
            issues.append(f"{QUEUE_RELATIVE_PATH.as_posix()}: generated queue is stale")

    if issues:
        print("Open-work candidate queue validation failed.", file=sys.stderr)
        for issue in issues:
            print(f"- {issue}", file=sys.stderr)
        return 1

    print("[ok] validated reviewed open-work candidate queue and generated parity")
    print("[boundary] mechanics only: candidate review does not accept identity, chronology, rights, text, semantics, canon, or publication")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

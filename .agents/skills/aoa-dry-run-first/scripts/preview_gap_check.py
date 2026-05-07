#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def _load_payload(path: str | None) -> dict[str, Any]:
    if path:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    raw = sys.stdin.read().strip()
    if not raw:
        raise SystemExit("Expected JSON input on stdin or via a file path.")
    return json.loads(raw)


def check_gaps(payload: dict[str, Any]) -> dict[str, Any]:
    raw_preview_steps = payload.get("preview_steps") or []
    preview_steps = [step for step in raw_preview_steps if isinstance(step, dict)]
    apply_step = payload.get("apply_step") or {}
    limitations = payload.get("limitations") or payload.get("honest_boundaries") or []

    gaps: list[str] = []
    notes: list[str] = []

    if not isinstance(raw_preview_steps, list):
        gaps.append("preview-steps-not-list")
        raw_preview_steps = []
        preview_steps = []
    malformed_preview_count = len(raw_preview_steps) - len(preview_steps)
    if malformed_preview_count:
        gaps.append("preview-step-not-object")
        notes.append(f"malformed preview steps: {malformed_preview_count}")
    if not isinstance(apply_step, dict):
        gaps.append("apply-step-not-object")
        apply_step = {}

    apply_command = str(apply_step.get("command", "")).strip()
    if not preview_steps:
        gaps.append("missing-preview-step")
    if not apply_command:
        gaps.append("missing-apply-command")

    preview_commands = [str(step.get("command", "")).strip() for step in preview_steps]
    if apply_command and apply_command in preview_commands:
        gaps.append("preview-and-apply-are-the-same-command")

    if not limitations:
        gaps.append("missing-limitations")
    elif len(limitations) == 1:
        notes.append("only-one-limitation-recorded")

    mutating_preview_steps = [
        str(step.get("label", "preview")).strip()
        for step in preview_steps
        if step.get("touches_state") is True
    ]
    if mutating_preview_steps:
        gaps.append("preview-step-marked-mutating")
        notes.append(f"mutating preview labels: {', '.join(mutating_preview_steps)}")

    confirmation_required = payload.get("confirmation_required")
    if confirmation_required is False:
        notes.append("confirmation seam has been weakened")

    if gaps:
        status = "fail"
    elif notes:
        status = "warn"
    else:
        status = "ok"

    return {
        "skill": "aoa-dry-run-first",
        "status": status,
        "gaps": gaps,
        "notes": notes,
        "checked_preview_steps": len(preview_steps),
        "checked_limitations": len(limitations),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Check preview-vs-apply separation.")
    parser.add_argument("input_json", nargs="?", help="JSON file path. Reads stdin when omitted.")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print output JSON.")
    args = parser.parse_args()

    result = check_gaps(_load_payload(args.input_json))
    if args.pretty:
        print(json.dumps(result, indent=2))
    else:
        print(json.dumps(result))
    return 0 if result["status"] != "fail" else 2


if __name__ == "__main__":
    raise SystemExit(main())

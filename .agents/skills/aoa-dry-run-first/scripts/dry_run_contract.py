#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Mapping


TEMPLATE = {
    "requested_action": "Switch application symlink from current to previous release",
    "preview_steps": [
        {
            "label": "inspect current symlink target",
            "command": "readlink current",
            "touches_state": False,
        },
        {
            "label": "show candidate target",
            "command": "readlink previous",
            "touches_state": False,
        },
    ],
    "apply_step": {
        "label": "switch symlink",
        "command": "ln -sfn previous current",
        "touches_state": True,
    },
    "touched_surfaces": ["application symlink", "release pointer"],
    "limitations": [
        "preview does not prove runtime health after the switch",
        "preview does not validate permissions for the final write",
    ],
    "confirmation_required": True,
}


def _load_payload(path: str | None) -> dict[str, Any]:
    if path:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    raw = sys.stdin.read().strip()
    if not raw:
        raise SystemExit("Expected JSON input on stdin or via a file path.")
    return json.loads(raw)


def _command(step: dict[str, Any]) -> str:
    return str(step.get("command", "")).strip()


def build_contract(payload: dict[str, Any]) -> dict[str, Any]:
    raw_preview_steps = payload.get("preview_steps") or []
    preview_steps = raw_preview_steps if isinstance(raw_preview_steps, list) else []
    raw_apply_step = payload.get("apply_step") or {}
    apply_step = raw_apply_step if isinstance(raw_apply_step, Mapping) else {}
    limitations = payload.get("limitations") or []
    warnings: list[str] = []
    errors: list[str] = []

    if not isinstance(raw_preview_steps, list):
        errors.append("preview_steps must be a list.")
    if raw_apply_step and not isinstance(raw_apply_step, Mapping):
        errors.append("apply_step must be an object.")
    if not preview_steps:
        errors.append("No preview_steps were provided.")
    if not apply_step:
        errors.append("No apply_step was provided.")

    preview_commands = []
    for idx, step in enumerate(preview_steps, start=1):
        if not isinstance(step, Mapping):
            errors.append(f"preview step {idx} must be an object.")
            continue
        preview_commands.append(_command(step))
        if step.get("touches_state") is True:
            errors.append(f"preview step {idx} is marked as mutating.")
        if not _command(step):
            errors.append(f"preview step {idx} is missing a command.")

    apply_command = _command(apply_step)
    if not apply_command:
        errors.append("apply_step.command is missing.")
    if apply_step.get("touches_state") is False:
        warnings.append("apply_step is marked as non-mutating; confirm that this is intentional.")

    if apply_command and apply_command in preview_commands:
        errors.append("The apply command matches one of the preview commands.")

    if not limitations:
        warnings.append("No limitations were recorded. The preview boundary is probably underspecified.")

    confirmation_required = bool(payload.get("confirmation_required", True))
    if not confirmation_required:
        warnings.append("confirmation_required is false. This weakens the explicit seam the skill expects.")

    if errors:
        workflow_state = "hold"
        preview_verdict = "fail"
        next_step = "repair-the-preview-contract-before-mutation"
        confirmation_prompt = None
    elif warnings:
        workflow_state = "ready_for_confirmation" if confirmation_required else "preview-complete"
        preview_verdict = "warn"
        next_step = "ask-for-confirmation" if confirmation_required else "review-warnings-before-apply"
        confirmation_prompt = (
            f"Confirm the exact mutating step: {apply_step.get('label', 'apply')} -> {apply_command}"
            if confirmation_required
            else None
        )
    else:
        workflow_state = "ready_for_confirmation"
        preview_verdict = "ok"
        next_step = "ask-for-confirmation"
        confirmation_prompt = f"Confirm the exact mutating step: {apply_step.get('label', 'apply')} -> {apply_command}"

    return {
        "skill": "aoa-dry-run-first",
        "requested_action": payload.get("requested_action", ""),
        "preview_verdict": preview_verdict,
        "workflow_state": workflow_state,
        "preview_steps": preview_steps,
        "apply_step": apply_step,
        "touched_surfaces": payload.get("touched_surfaces") or [],
        "honest_boundaries": limitations,
        "warnings": warnings,
        "errors": errors,
        "confirmation_prompt": confirmation_prompt,
        "next_step": next_step,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Build a deterministic dry-run contract.")
    parser.add_argument("input_json", nargs="?", help="JSON file path. Reads stdin when omitted.")
    parser.add_argument("--template", action="store_true", help="Print a starter JSON payload and exit.")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print the output JSON.")
    args = parser.parse_args()

    if args.template:
        print(json.dumps(TEMPLATE, indent=2))
        return 0

    payload = _load_payload(args.input_json)
    result = build_contract(payload)
    if args.pretty:
        print(json.dumps(result, indent=2))
    else:
        print(json.dumps(result))
    return 0 if not result["errors"] else 2


if __name__ == "__main__":
    raise SystemExit(main())

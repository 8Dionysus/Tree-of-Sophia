#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Mapping


TEMPLATE = {
    "runtime_name": "compose",
    "selector": "dev",
    "rendered_services": ["api", "worker", "postgres", "redis"],
    "readiness_items": [
        {"severity": "ok", "label": "docker daemon reachable"},
        {"severity": "ok", "label": "dev profile render completed"},
        {"severity": "warn", "label": "postgres volume will be reused"},
    ],
    "launch_command": "docker compose --profile dev up -d",
    "stop_command": "docker compose --profile dev down",
    "confirmation_required": True,
}


def _load_payload(path: str | None) -> dict[str, Any]:
    if path:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    raw = sys.stdin.read().strip()
    if not raw:
        raise SystemExit("Expected JSON input on stdin or via a file path.")
    return json.loads(raw)


def build_report(payload: dict[str, Any]) -> dict[str, Any]:
    services = [str(x) for x in payload.get("rendered_services") or []]
    readiness_items = payload.get("readiness_items") or []
    warnings: list[str] = []
    errors: list[str] = []

    if not payload.get("runtime_name"):
        errors.append("runtime_name is missing.")
    if not payload.get("selector"):
        warnings.append("selector is missing; selected runtime truth may be ambiguous.")
    if not services:
        errors.append("rendered_services is empty.")
    if not payload.get("launch_command"):
        errors.append("launch_command is missing.")
    if not payload.get("stop_command"):
        warnings.append("stop_command is missing.")

    fail_items: list[Mapping[str, Any]] = []
    warn_items: list[Mapping[str, Any]] = []
    for index, item in enumerate(readiness_items, start=1):
        if not isinstance(item, Mapping):
            errors.append(f"readiness_items[{index}] must be an object.")
            continue
        severity = str(item.get("severity", "")).lower()
        if severity == "fail":
            fail_items.append(item)
        elif severity == "warn":
            warn_items.append(item)
        elif severity in {"", "ok"}:
            continue
        else:
            fail_items.append(item)
            warnings.append(
                f"Unknown readiness severity {severity!r} at item {index}; treated as a blocker."
            )

    if fail_items:
        verdict = "hold"
        next_step = "fix-blockers-before-launch"
    elif payload.get("confirmation_required", True):
        verdict = "ready_for_confirmation"
        next_step = "ask-for-launch-confirmation"
    else:
        verdict = "go"
        next_step = "launch"

    if warn_items:
        warnings.append(f"{len(warn_items)} readiness warning(s) remain visible.")

    return {
        "skill": "aoa-local-stack-bringup",
        "runtime_name": payload.get("runtime_name", ""),
        "selector": payload.get("selector", ""),
        "rendered_services": services,
        "readiness_items": readiness_items,
        "blocker_count": len(fail_items),
        "warning_count": len(warn_items),
        "launch_command": payload.get("launch_command", ""),
        "stop_command": payload.get("stop_command", ""),
        "verdict": verdict if not errors else "hold",
        "next_step": next_step if not errors else "repair-contract-before-launch",
        "warnings": warnings,
        "errors": errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Build a deterministic local-stack bringup contract.")
    parser.add_argument("input_json", nargs="?", help="JSON file path. Reads stdin when omitted.")
    parser.add_argument("--template", action="store_true", help="Print a starter JSON payload and exit.")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print output JSON.")
    args = parser.parse_args()

    if args.template:
        print(json.dumps(TEMPLATE, indent=2))
        return 0

    result = build_report(_load_payload(args.input_json))
    if args.pretty:
        print(json.dumps(result, indent=2))
    else:
        print(json.dumps(result))
    return 0 if not result["errors"] else 2


if __name__ == "__main__":
    raise SystemExit(main())

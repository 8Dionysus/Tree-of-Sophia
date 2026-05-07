#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def _load_payload(path: str | None) -> Any:
    if path:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    raw = sys.stdin.read().strip()
    if not raw:
        raise SystemExit("Expected JSON or text input on stdin or via a file path.")
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return raw.splitlines()


def _normalize_line(line: str) -> dict[str, str]:
    stripped = line.strip()
    lower = stripped.lower()
    if lower.startswith("ok:"):
        return {"severity": "ok", "label": stripped[3:].strip()}
    if lower.startswith("warn:"):
        return {"severity": "warn", "label": stripped[5:].strip()}
    if lower.startswith("fail:"):
        return {"severity": "fail", "label": stripped[5:].strip()}
    return {"severity": "warn", "label": stripped}


def summarize(payload: Any) -> dict[str, Any]:
    if isinstance(payload, dict) and "readiness_items" in payload:
        items = payload.get("readiness_items") or []
    elif isinstance(payload, list):
        if payload and isinstance(payload[0], dict):
            items = payload
        else:
            items = [_normalize_line(str(line)) for line in payload if str(line).strip()]
    else:
        items = [_normalize_line(str(payload))]

    normalized = []
    counts = {"ok": 0, "warn": 0, "fail": 0}
    for item in items:
        if not isinstance(item, dict):
            item = _normalize_line(str(item))
        severity = str(item.get("severity", "warn")).lower()
        if severity not in counts:
            severity = "warn"
        counts[severity] += 1
        normalized.append({"severity": severity, "label": str(item.get("label", "")).strip()})

    overall = "fail" if counts["fail"] else ("warn" if counts["warn"] else "ok")
    return {
        "skill": "aoa-local-stack-bringup",
        "overall": overall,
        "counts": counts,
        "items": normalized,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Summarize selector-aware readiness items.")
    parser.add_argument("input_json", nargs="?", help="JSON file path. Reads stdin when omitted.")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print output JSON.")
    args = parser.parse_args()

    result = summarize(_load_payload(args.input_json))
    if args.pretty:
        print(json.dumps(result, indent=2))
    else:
        print(json.dumps(result))
    return 0 if result["overall"] != "fail" else 2


if __name__ == "__main__":
    raise SystemExit(main())

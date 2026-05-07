#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


RULES: list[tuple[str, str, str, str]] = [
    (r"terraform|tfvars|terragrunt", "infra_as_code", "run plan before apply", "keep previous variable set or state-safe rollback path"),
    (r"kubernetes|helm|docker-compose|compose|deployment\.ya?ml", "orchestration", "render effective config and inspect rollout status", "keep undo or stop command visible"),
    (r"migration|schema|database|postgres|mysql|redis", "stateful_data", "capture dry-run or migration preview plus health check", "name restore or rollback before mutation"),
    (r"secret|vault|token|oauth|oidc|iam|credential", "identity_or_secret", "verify access posture and secret source before apply", "prepare revoke or rotate path"),
    (r"nginx|haproxy|envoy|ingress|gateway|load[-_ ]balancer", "traffic_routing", "verify config syntax and bounded health check", "keep previous config or undo route change ready"),
    (r"firewall|security group|dns|network policy|iptables", "network_boundary", "stage bounded connectivity checks", "prepare precise revert command"),
]


def _load_payload(path: str | None) -> Any:
    if path:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    raw = sys.stdin.read().strip()
    if not raw:
        raise SystemExit("Expected JSON or newline text on stdin or via a file path.")
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return raw.splitlines()


def _flatten(payload: Any) -> list[str]:
    if isinstance(payload, dict):
        parts: list[str] = []
        for value in payload.values():
            parts.extend(_flatten(value))
        return parts
    if isinstance(payload, list):
        parts: list[str] = []
        for value in payload:
            parts.extend(_flatten(value))
        return parts
    return [str(payload)]


def scan(payload: Any) -> dict[str, Any]:
    texts = _flatten(payload)
    blob = " \n ".join(texts).lower()
    detections = []
    for pattern, label, verification_hint, rollback_hint in RULES:
        if re.search(pattern, blob):
            detections.append(
                {
                    "surface": label,
                    "verification_hint": verification_hint,
                    "rollback_hint": rollback_hint,
                }
            )
    return {
        "skill": "aoa-safe-infra-change",
        "status": "ok" if detections else "warn",
        "detected": detections,
        "input_items": texts,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Scan paths, commands, or notes for risky infra surfaces.")
    parser.add_argument("input_json", nargs="?", help="JSON file path. Reads stdin when omitted.")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print output JSON.")
    args = parser.parse_args()

    result = scan(_load_payload(args.input_json))
    if args.pretty:
        print(json.dumps(result, indent=2))
    else:
        print(json.dumps(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

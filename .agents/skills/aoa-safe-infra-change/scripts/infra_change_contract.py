#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


TEMPLATE = {
    "change_summary": "Adjust Terraform variables and service deployment configuration for the API tier",
    "touched_surfaces": [
        "infra/terraform/api.tfvars",
        "deploy/kubernetes/api-deployment.yaml",
    ],
    "mutating_commands": [
        "terraform apply -target=module.api",
        "kubectl apply -f deploy/kubernetes/api-deployment.yaml",
    ],
    "verification_steps": [
        "terraform plan -target=module.api",
        "kubectl rollout status deployment/api",
        "curl -fsS http://localhost:8080/healthz",
    ],
    "rollback_steps": [
        "terraform apply -target=module.api -var-file=previous.tfvars",
        "kubectl rollout undo deployment/api",
    ],
    "authority_state": "approved",
}

SURFACE_RULES: list[tuple[str, str, int]] = [
    (r"terraform|tfvars|iac|terragrunt", "infra_as_code", 3),
    (r"kubernetes|helm|docker-compose|compose|deployment\.ya?ml", "orchestration", 3),
    (r"nginx|haproxy|envoy|ingress|gateway|lb|load[-_ ]balancer", "traffic_routing", 3),
    (r"secret|vault|token|oauth|oidc|iam|identity|credential", "identity_or_secret", 4),
    (r"migration|sql|database|schema|postgres|mysql|redis", "stateful_data", 4),
    (r"systemd|service|unit|process manager", "runtime_service", 2),
    (r"firewall|security group|network policy|iptables|dns", "network_boundary", 4),
]


def _load_payload(path: str | None) -> dict[str, Any]:
    if path:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    raw = sys.stdin.read().strip()
    if not raw:
        raise SystemExit("Expected JSON input on stdin or via a file path.")
    return json.loads(raw)


def _iter_strings(values: list[Any]) -> list[str]:
    out: list[str] = []
    for value in values:
        if isinstance(value, str):
            out.append(value)
        else:
            out.append(json.dumps(value, ensure_ascii=False))
    return out


def classify_surfaces(texts: list[str]) -> tuple[list[str], int]:
    detected: list[str] = []
    score = 0
    blob = " \n ".join(texts).lower()
    for pattern, label, weight in SURFACE_RULES:
        if re.search(pattern, blob):
            detected.append(label)
            score += weight
    return detected, score


def verification_strength(steps: list[str]) -> str:
    joined = " ".join(steps).lower()
    strength = 0
    if any(token in joined for token in ("plan", "diff", "preview", "lint", "validate")):
        strength += 1
    if any(token in joined for token in ("test", "health", "status", "rollout", "smoke")):
        strength += 1
    if len(steps) >= 3:
        strength += 1
    if strength >= 3:
        return "strong"
    if strength == 2:
        return "moderate"
    if strength == 1:
        return "weak"
    return "missing"


def build_report(payload: dict[str, Any]) -> dict[str, Any]:
    touched_surfaces = _iter_strings(payload.get("touched_surfaces") or [])
    commands = _iter_strings(payload.get("mutating_commands") or [])
    verification_steps = _iter_strings(payload.get("verification_steps") or [])
    rollback_steps = _iter_strings(payload.get("rollback_steps") or [])
    detected_surfaces, score = classify_surfaces(touched_surfaces + commands)
    warnings: list[str] = []
    errors: list[str] = []

    if not touched_surfaces:
        errors.append("No touched_surfaces were provided.")
    if not commands:
        errors.append("No mutating_commands were provided.")
    if not verification_steps:
        warnings.append("No verification_steps were provided.")
    if not rollback_steps:
        warnings.append("No rollback_steps were provided.")

    authority_state = str(payload.get("authority_state", "unspecified")).strip().lower()
    if authority_state not in {"approved", "planned", "unspecified"}:
        warnings.append(f"Unrecognized authority_state value: {authority_state}")

    risk_band = "low"
    if score >= 9:
        risk_band = "high"
    elif score >= 5:
        risk_band = "medium"

    if "identity_or_secret" in detected_surfaces or "stateful_data" in detected_surfaces:
        risk_band = "high"

    if not detected_surfaces:
        warnings.append("No operational surface patterns were detected. Confirm that the scope is described concretely.")

    report_state = "hold" if errors else "ready"
    if risk_band == "high" and authority_state != "approved":
        warnings.append("High-risk change is not marked approved.")
        report_state = "confirm-or-hold"

    if verification_strength(verification_steps) == "missing":
        warnings.append("Verification posture is missing or too thin.")

    if not rollback_steps:
        report_state = "confirm-or-hold"

    return {
        "skill": "aoa-safe-infra-change",
        "change_summary": payload.get("change_summary", ""),
        "detected_surfaces": detected_surfaces,
        "risk_band": risk_band,
        "report_state": report_state,
        "authority_state": authority_state,
        "verification_strength": verification_strength(verification_steps),
        "rollback_ready": bool(rollback_steps),
        "touched_surfaces": touched_surfaces,
        "mutating_commands": commands,
        "verification_steps": verification_steps,
        "rollback_steps": rollback_steps,
        "warnings": warnings,
        "errors": errors,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Build a deterministic infrastructure-change contract.")
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

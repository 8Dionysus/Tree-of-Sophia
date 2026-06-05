#!/usr/bin/env python3
"""Validate ToS decision records without rewriting generated indexes."""

from __future__ import annotations

from pathlib import Path

from generate_decision_indexes import REPO_ROOT, collect_decision_records, validate_decision_lane_surfaces, validate_index_contract


def validate_decision_records(repo_root: Path = REPO_ROOT) -> list[tuple[str, str]]:
    records, issues = collect_decision_records(repo_root)
    issues.extend(validate_index_contract(repo_root))
    issues.extend(validate_decision_lane_surfaces(repo_root))
    if not records:
        issues.append(("docs/decisions", "no decision records available for validation"))
    return issues


def main() -> int:
    issues = validate_decision_records(REPO_ROOT)
    if issues:
        print("Decision record validation failed.")
        for location, message in issues:
            print(f"- {location}: {message}")
        return 1

    print("[ok] decision records validated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

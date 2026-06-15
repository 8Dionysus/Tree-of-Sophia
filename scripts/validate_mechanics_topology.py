#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
TOPOLOGY_PATH = Path("mechanics/topology.json")

Issue: TypeAlias = tuple[str, str]

EXPECTED_PACKAGES: dict[str, dict[str, object]] = {
    "agon": {
        "class": "head-fed/local",
        "status": "active",
        "parts": ("threshold-intake", "canon-restraint", "threshold-registry", "landing-handoff"),
        "legacy_required": True,
    },
    "antifragility": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("source-resilience",),
        "legacy_required": False,
    },
    "audit": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("review-ledger-route",),
        "legacy_required": True,
    },
    "boundary-bridge": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("derived-kag-seam",),
        "legacy_required": True,
    },
    "canon-formation": {
        "class": "local",
        "status": "planted",
        "parts": ("promotion-gate",),
        "legacy_required": False,
    },
    "checkpoint": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("review-return",),
        "legacy_required": False,
    },
    "distillation": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("source-compost",),
        "legacy_required": True,
    },
    "experience": {
        "class": "head-fed/local",
        "status": "active",
        "parts": (
            "candidate-review",
            "adoption-boundary",
            "governance-boundary",
            "installation-boundary",
            "service-office-boundary",
            "pattern-review",
            "write-guards",
        ),
        "legacy_required": True,
    },
    "growth-cycle": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("branch-growth-cycle",),
        "legacy_required": True,
    },
    "method-growth": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("node-method-spine",),
        "legacy_required": False,
    },
    "questbook": {
        "class": "head-fed/local",
        "status": "active",
        "parts": ("obligation-boundary", "dispatch-contracts"),
        "legacy_required": True,
    },
    "recurrence": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("calibration-return",),
        "legacy_required": False,
    },
    "relation-weaving": {
        "class": "local",
        "status": "planted",
        "parts": ("graph-promotion",),
        "legacy_required": False,
    },
    "release-support": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("source-release-gate",),
        "legacy_required": False,
    },
    "rpg": {
        "class": "head-fed/local",
        "status": "planted",
        "parts": ("reading-progression",),
        "legacy_required": False,
    },
    "source-witnessing": {
        "class": "local",
        "status": "planted",
        "parts": ("witness-route",),
        "legacy_required": True,
    },
}

EXPECTED_ORDER = tuple(EXPECTED_PACKAGES)


def load_topology(repo_root: Path, issues: list[Issue]) -> dict[str, object] | None:
    path = repo_root / TOPOLOGY_PATH
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        issues.append((TOPOLOGY_PATH.as_posix(), "missing mechanics topology"))
        return None
    except json.JSONDecodeError as exc:
        issues.append((TOPOLOGY_PATH.as_posix(), f"invalid JSON: {exc}"))
        return None
    if not isinstance(payload, dict):
        issues.append((TOPOLOGY_PATH.as_posix(), "topology root must be a JSON object"))
        return None
    return payload


def require_file(repo_root: Path, issues: list[Issue], relative_path: str) -> None:
    if not (repo_root / relative_path).is_file():
        issues.append((relative_path, "missing required file"))


def require_absent(repo_root: Path, issues: list[Issue], relative_path: str, message: str) -> None:
    if (repo_root / relative_path).exists():
        issues.append((relative_path, message))


def validate_package(repo_root: Path, issues: list[Issue], slug: str, expected: dict[str, object]) -> None:
    for filename in ("AGENTS.md", "README.md", "PARTS.md", "PROVENANCE.md", "ROADMAP.md"):
        require_file(repo_root, issues, f"mechanics/{slug}/{filename}")

    parts = expected["parts"]
    assert isinstance(parts, tuple)
    for part in parts:
        require_file(repo_root, issues, f"mechanics/{slug}/parts/{part}/README.md")

    legacy_required = expected["legacy_required"]
    assert isinstance(legacy_required, bool)
    if legacy_required:
        require_file(repo_root, issues, f"mechanics/{slug}/legacy/README.md")
        require_file(repo_root, issues, f"mechanics/{slug}/legacy/INDEX.md")
    else:
        require_absent(
            repo_root,
            issues,
            f"mechanics/{slug}/legacy",
            "package-local legacy is allowed only for moved-path or raw-receipt accounting",
        )


def validate_moved_targets(repo_root: Path, issues: list[Issue], topology: dict[str, object]) -> None:
    moved_accounting = topology.get("moved_path_accounting")
    if not isinstance(moved_accounting, dict):
        issues.append((TOPOLOGY_PATH.as_posix(), "moved_path_accounting must be an object"))
        return

    moved_targets = topology.get("moved_path_targets")
    if not isinstance(moved_targets, dict):
        issues.append((TOPOLOGY_PATH.as_posix(), "moved_path_targets must be an object"))
        return

    accounted_old_paths: set[str] = set()
    for package, parts in moved_accounting.items():
        if not isinstance(parts, dict):
            issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package} must be an object"))
            continue
        if package not in EXPECTED_PACKAGES:
            issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package} is not a known package"))
        for part, old_paths in parts.items():
            if not isinstance(old_paths, list) or not all(isinstance(item, str) and item for item in old_paths):
                issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package}.{part} must be a string list"))
                continue
            for old_path in old_paths:
                accounted_old_paths.add(old_path)
                new_path = moved_targets.get(old_path)
                if not isinstance(new_path, str) or not new_path:
                    issues.append((TOPOLOGY_PATH.as_posix(), f"{old_path} is missing a moved_path_targets entry"))
                    continue
                require_absent(
                    repo_root,
                    issues,
                    old_path,
                    "mechanic-owned payload must stay in mechanics/, not the old ToS/root path",
                )
                require_file(repo_root, issues, new_path)

    for old_path, new_path in moved_targets.items():
        if not isinstance(old_path, str) or not isinstance(new_path, str):
            issues.append((TOPOLOGY_PATH.as_posix(), "moved_path_targets keys and values must be strings"))
            continue
        if old_path not in accounted_old_paths:
            issues.append((TOPOLOGY_PATH.as_posix(), f"{old_path} target is not listed in moved_path_accounting"))


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    require_file(root, issues, "mechanics/AGENTS.md")
    require_file(root, issues, "mechanics/README.md")
    require_absent(root, issues, "mechanics/legacy", "root mechanics legacy is forbidden")

    topology = load_topology(root, issues)
    if topology is None:
        return issues

    if topology.get("schema_version") != "tos_mechanics_topology_v2":
        issues.append((TOPOLOGY_PATH.as_posix(), "schema_version must be tos_mechanics_topology_v2"))
    if topology.get("owner_repo") != "Tree-of-Sophia":
        issues.append((TOPOLOGY_PATH.as_posix(), "owner_repo must be Tree-of-Sophia"))
    if topology.get("root") != "mechanics/":
        issues.append((TOPOLOGY_PATH.as_posix(), "root must be mechanics/"))
    if (
        topology.get("legacy_policy")
        != "package-local-only-when-active-route-has-moved-path-or-raw-receipt-accounting"
    ):
        issues.append((TOPOLOGY_PATH.as_posix(), "legacy_policy drifted"))

    packages = topology.get("packages")
    if not isinstance(packages, list):
        issues.append((TOPOLOGY_PATH.as_posix(), "packages must be a list"))
        return issues

    seen: dict[str, dict[str, object]] = {}
    order: list[str] = []
    for entry in packages:
        if not isinstance(entry, dict):
            issues.append((TOPOLOGY_PATH.as_posix(), "each package entry must be an object"))
            continue
        slug = entry.get("slug")
        if not isinstance(slug, str) or not slug:
            issues.append((TOPOLOGY_PATH.as_posix(), "package slug must be a non-empty string"))
            continue
        if slug in seen:
            issues.append((TOPOLOGY_PATH.as_posix(), f"duplicate package slug {slug}"))
        seen[slug] = entry
        order.append(slug)

        expected = EXPECTED_PACKAGES.get(slug)
        if expected is None:
            issues.append((TOPOLOGY_PATH.as_posix(), f"unexpected top-level mechanic {slug}"))
            continue
        if entry.get("class") != expected["class"]:
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.class must be {expected['class']}"))
        if entry.get("status") != expected["status"]:
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.status must be {expected['status']}"))
        if entry.get("active_parts") != list(expected["parts"]):
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.active_parts must be {list(expected['parts'])!r}"))
        if entry.get("legacy_required") != expected["legacy_required"]:
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.legacy_required drifted"))
        validate_package(root, issues, slug, expected)

    if tuple(order) != EXPECTED_ORDER:
        issues.append((TOPOLOGY_PATH.as_posix(), "package order must stay shared-first and ToS-local-aware"))
    for missing in sorted(set(EXPECTED_PACKAGES) - set(seen)):
        issues.append((TOPOLOGY_PATH.as_posix(), f"missing package {missing}"))

    validate_moved_targets(root, issues, topology)

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Mechanics topology validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print("[ok] validated ToS mechanics topology")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

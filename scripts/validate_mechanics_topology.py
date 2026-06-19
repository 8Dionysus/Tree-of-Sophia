#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
TOPOLOGY_PATH = Path("mechanics/topology.json")

Issue: TypeAlias = tuple[str, str]

PACKAGE_SLUG_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
ALLOWED_CLASSES = {"head-fed/local", "local"}
ALLOWED_STATUSES = {"active", "planted"}
IGNORED_MECHANICS_DIRS = {"__pycache__", "legacy"}


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


def discover_package_dirs(repo_root: Path) -> set[str]:
    mechanics_root = repo_root / "mechanics"
    if not mechanics_root.is_dir():
        return set()
    return {
        path.name
        for path in mechanics_root.iterdir()
        if path.is_dir() and path.name not in IGNORED_MECHANICS_DIRS and (path / "AGENTS.md").is_file()
    }


def validate_package(repo_root: Path, issues: list[Issue], slug: str, entry: dict[str, object]) -> tuple[str, ...]:
    if not PACKAGE_SLUG_RE.fullmatch(slug):
        issues.append((TOPOLOGY_PATH.as_posix(), f"package slug is not normalized: {slug}"))

    for filename in ("AGENTS.md", "README.md", "PARTS.md", "PROVENANCE.md", "ROADMAP.md"):
        require_file(repo_root, issues, f"mechanics/{slug}/{filename}")

    package_class = entry.get("class")
    if package_class not in ALLOWED_CLASSES:
        issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.class must be one of {sorted(ALLOWED_CLASSES)!r}"))

    status = entry.get("status")
    if status not in ALLOWED_STATUSES:
        issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.status must be one of {sorted(ALLOWED_STATUSES)!r}"))

    active_parts = entry.get("active_parts")
    if (
        not isinstance(active_parts, list)
        or not active_parts
        or not all(isinstance(part, str) and PACKAGE_SLUG_RE.fullmatch(part) for part in active_parts)
    ):
        issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.active_parts must be a non-empty normalized string list"))
        parts: tuple[str, ...] = ()
    else:
        parts = tuple(active_parts)
        if len(parts) != len(set(parts)):
            issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.active_parts contains duplicates"))

    for part in parts:
        require_file(repo_root, issues, f"mechanics/{slug}/parts/{part}/README.md")

    legacy_required = entry.get("legacy_required")
    if not isinstance(legacy_required, bool):
        issues.append((TOPOLOGY_PATH.as_posix(), f"{slug}.legacy_required must be boolean"))
        return parts
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

    return parts


def validate_moved_targets(
    repo_root: Path,
    issues: list[Issue],
    topology: dict[str, object],
    packages: dict[str, dict[str, object]],
    package_parts: dict[str, set[str]],
) -> None:
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
        if package not in packages:
            issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package} is not a known package"))
            known_parts: set[str] = set()
        else:
            known_parts = package_parts.get(package, set())
            if packages[package].get("legacy_required") is not True:
                issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package} requires legacy_required true"))
        for part, old_paths in parts.items():
            if part not in known_parts:
                issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package}.{part} is not an active part"))
            if not isinstance(old_paths, list) or not all(isinstance(item, str) and item for item in old_paths):
                issues.append((TOPOLOGY_PATH.as_posix(), f"moved_path_accounting.{package}.{part} must be a string list"))
                continue
            for old_path in old_paths:
                accounted_old_paths.add(old_path)
                new_path = moved_targets.get(old_path)
                if not isinstance(new_path, str) or not new_path:
                    issues.append((TOPOLOGY_PATH.as_posix(), f"{old_path} is missing a moved_path_targets entry"))
                    continue
                expected_prefix = f"mechanics/{package}/parts/{part}/"
                if part in known_parts and not new_path.startswith(expected_prefix):
                    issues.append((TOPOLOGY_PATH.as_posix(), f"{old_path} target must stay under {expected_prefix}"))
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
    package_parts: dict[str, set[str]] = {}
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
        package_parts[slug] = set(validate_package(root, issues, slug, entry))

    package_dirs = discover_package_dirs(root)
    for missing in sorted(package_dirs - set(seen)):
        issues.append((TOPOLOGY_PATH.as_posix(), f"mechanics package directory missing from topology: {missing}"))
    for extra in sorted(set(seen) - package_dirs):
        issues.append((TOPOLOGY_PATH.as_posix(), f"topology package missing mechanics directory: {extra}"))

    if len(order) != len(set(order)):
        issues.append((TOPOLOGY_PATH.as_posix(), "packages must be unique"))

    validate_moved_targets(root, issues, topology, seen, package_parts)

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

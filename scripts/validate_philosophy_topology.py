#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = Path("ToS/philosophy/philosophy.manifest.json")
WITNESS_MANIFEST_PATH = Path("ToS/source-witnesses/notion/philosophy/witness.manifest.json")

Issue: TypeAlias = tuple[str, str]


def load_json(repo_root: Path, relative_path: Path, issues: list[Issue]) -> dict[str, object] | None:
    path = repo_root / relative_path
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        issues.append((relative_path.as_posix(), "missing JSON file"))
        return None
    except json.JSONDecodeError as exc:
        issues.append((relative_path.as_posix(), f"invalid JSON: {exc}"))
        return None
    if not isinstance(payload, dict):
        issues.append((relative_path.as_posix(), "JSON root must be an object"))
        return None
    return payload


def check_path_components(relative_path: Path, forbidden: set[str], issues: list[Issue]) -> None:
    lowered_parts = {part.lower() for part in relative_path.parts}
    for component in sorted(forbidden):
        if component.lower() in lowered_parts:
            issues.append((relative_path.as_posix(), f"forbidden path component: {component}"))


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    required_files = [
        Path("ToS/philosophy/AGENTS.md"),
        Path("ToS/philosophy/README.md"),
        MANIFEST_PATH,
        Path("ToS/source-witnesses/notion/philosophy/AGENTS.md"),
        WITNESS_MANIFEST_PATH,
    ]
    for relative_path in required_files:
        if not (root / relative_path).is_file():
            issues.append((relative_path.as_posix(), "missing required philosophy topology file"))

    manifest = load_json(root, MANIFEST_PATH, issues)
    if manifest is None:
        return issues

    if manifest.get("schema_version") != "tos_philosophy_topology_v1":
        issues.append((MANIFEST_PATH.as_posix(), "schema_version must be tos_philosophy_topology_v1"))
    if manifest.get("branch_id") != "philosophy":
        issues.append((MANIFEST_PATH.as_posix(), "branch_id must be philosophy"))
    if manifest.get("path") != "ToS/philosophy":
        issues.append((MANIFEST_PATH.as_posix(), "path must be ToS/philosophy"))

    forbidden_raw = manifest.get("forbidden_path_components")
    if not isinstance(forbidden_raw, list) or not all(isinstance(item, str) for item in forbidden_raw):
        issues.append((MANIFEST_PATH.as_posix(), "forbidden_path_components must be a string list"))
        forbidden: set[str] = set()
    else:
        forbidden = set(forbidden_raw)

    branch_manifests = manifest.get("branch_manifests")
    if not isinstance(branch_manifests, list) or not branch_manifests:
        issues.append((MANIFEST_PATH.as_posix(), "branch_manifests must be a non-empty list"))
    else:
        seen: set[str] = set()
        for entry in branch_manifests:
            if not isinstance(entry, str) or not entry:
                issues.append((MANIFEST_PATH.as_posix(), "branch_manifests entries must be non-empty strings"))
                continue
            if entry in seen:
                issues.append((MANIFEST_PATH.as_posix(), f"duplicate branch manifest {entry}"))
            seen.add(entry)
            relative_path = Path(entry)
            check_path_components(relative_path, forbidden, issues)
            payload = load_json(root, relative_path, issues)
            if payload is None:
                continue
            if payload.get("path") != relative_path.parent.as_posix():
                issues.append((entry, "branch manifest path must match its parent directory"))
            branch_id = payload.get("branch_id")
            role = payload.get("role")
            if not isinstance(branch_id, str) or not branch_id.startswith("philosophy."):
                issues.append((entry, "branch_id must start with philosophy."))
            if not isinstance(role, str) or not role:
                issues.append((entry, "role must be a non-empty string"))

    source_witness_routes = manifest.get("source_witness_routes")
    if source_witness_routes != [WITNESS_MANIFEST_PATH.as_posix()]:
        issues.append((MANIFEST_PATH.as_posix(), "source_witness_routes must point to the Notion philosophy witness manifest"))

    witness_manifest = load_json(root, WITNESS_MANIFEST_PATH, issues)
    if witness_manifest is not None:
        if witness_manifest.get("schema_version") != "tos_notion_witness_v1":
            issues.append((WITNESS_MANIFEST_PATH.as_posix(), "schema_version must be tos_notion_witness_v1"))
        if witness_manifest.get("domain_branch") != "ToS/philosophy":
            issues.append((WITNESS_MANIFEST_PATH.as_posix(), "domain_branch must be ToS/philosophy"))
        source_page = witness_manifest.get("notion_source_page")
        if not isinstance(source_page, dict) or not source_page.get("id"):
            issues.append((WITNESS_MANIFEST_PATH.as_posix(), "notion_source_page.id is required"))
        child_pages = witness_manifest.get("seed_child_pages")
        if not isinstance(child_pages, list) or not child_pages:
            issues.append((WITNESS_MANIFEST_PATH.as_posix(), "seed_child_pages must be a non-empty list"))

    for relative_path in Path("ToS").glob("**/*"):
        check_path_components(relative_path, forbidden, issues)

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Philosophy topology validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print("[ok] validated ToS philosophy topology")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

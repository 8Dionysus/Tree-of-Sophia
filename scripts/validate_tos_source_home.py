#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import TypeAlias

from validation_lanes import load_manifest as load_validation_lanes


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = Path("ToS/source_home.manifest.json")
HOME_PATH = Path("ToS")
LEGACY_ROOT_HOMES = (
    Path("sources"),
    Path("intake"),
    Path("tree"),
    Path("examples"),
    Path("generated"),
    Path("schemas"),
)
EXPECTED_BRANCHES = {
    "doctrine": "ToS/doctrine",
    "source_witnesses": "ToS/source-witnesses",
    "zarathustra": "ToS/zarathustra",
    "research_packets": "ToS/research-packets",
    "philosophy": "ToS/philosophy",
    "candidate_intake": "ToS/candidate-intake",
    "canon": "ToS/canon",
    "public_compatibility": "ToS/public-compatibility",
    "derived_exports": "ToS/derived-exports",
    "contracts": "ToS/contracts",
    "review_ledger": "ToS/review-ledger",
}
LANES_PATH = Path("docs/validation/validation_lanes.json")
REQUIRED_HOME_README_FRAGMENTS = (
    "## Operating Card",
    "## Boundary Routes",
    "| role | source-home entrypoint for ToS-authored philosophical work |",
    "| next route | witness or research packet -> zarathustra, philosophy, or candidate intake -> canon -> public compatibility -> derived export |",
)
BANNED_HOME_README_MARKERS = (
    "## Stop Lines",
    "## Hard no",
)

Issue: TypeAlias = tuple[str, str]


def load_manifest(repo_root: Path, issues: list[Issue]) -> dict[str, object] | None:
    path = repo_root / MANIFEST_PATH
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        issues.append((MANIFEST_PATH.as_posix(), "missing ToS source-home manifest"))
        return None
    except json.JSONDecodeError as exc:
        issues.append((MANIFEST_PATH.as_posix(), f"invalid JSON: {exc}"))
        return None
    if not isinstance(payload, dict):
        issues.append((MANIFEST_PATH.as_posix(), "manifest root must be a JSON object"))
        return None
    return payload


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    if not (root / HOME_PATH / "AGENTS.md").is_file():
        issues.append(("ToS/AGENTS.md", "missing ToS home route card"))
    if not (root / HOME_PATH / "README.md").is_file():
        issues.append(("ToS/README.md", "missing ToS home map"))
    else:
        readme_text = (root / HOME_PATH / "README.md").read_text(encoding="utf-8")
        normalized_readme = " ".join(readme_text.lower().split())
        for fragment in REQUIRED_HOME_README_FRAGMENTS:
            if " ".join(fragment.lower().split()) not in normalized_readme:
                issues.append(("ToS/README.md", f"missing source-home route fragment: {fragment}"))
        for marker in BANNED_HOME_README_MARKERS:
            if marker in readme_text:
                issues.append(("ToS/README.md", f"use Operating Card/Boundary Routes instead of {marker}"))

    for legacy_path in LEGACY_ROOT_HOMES:
        if (root / legacy_path).exists():
            issues.append((legacy_path.as_posix(), "legacy root home must not exist as an active ToS surface"))

    manifest = load_manifest(root, issues)
    if manifest is None:
        return issues

    try:
        lane_manifest = load_validation_lanes(root)
    except (FileNotFoundError, json.JSONDecodeError) as exc:
        issues.append((LANES_PATH.as_posix(), f"unable to load validation lanes: {exc}"))
        lane_ids: set[str] = set()
    else:
        lanes = lane_manifest.get("lanes")
        if isinstance(lanes, dict):
            lane_ids = set(lanes)
        else:
            lane_ids = set()
            issues.append((LANES_PATH.as_posix(), "lanes must be an object"))

    if manifest.get("schema_version") != "tos_source_home_v1":
        issues.append((MANIFEST_PATH.as_posix(), "schema_version must be tos_source_home_v1"))
    if manifest.get("owner_repo") != "Tree-of-Sophia":
        issues.append((MANIFEST_PATH.as_posix(), "owner_repo must be Tree-of-Sophia"))
    if manifest.get("home") != "ToS/":
        issues.append((MANIFEST_PATH.as_posix(), "home must be ToS/"))

    branches = manifest.get("branches")
    if not isinstance(branches, list):
        issues.append((MANIFEST_PATH.as_posix(), "branches must be a list"))
        return issues

    seen: dict[str, str] = {}
    for branch in branches:
        if not isinstance(branch, dict):
            issues.append((MANIFEST_PATH.as_posix(), "each branch must be an object"))
            continue
        branch_id = branch.get("id")
        branch_path = branch.get("path")
        owner_surface = branch.get("owner_surface")
        validation_lanes = branch.get("validation_lanes")
        if not isinstance(branch_id, str) or not branch_id:
            issues.append((MANIFEST_PATH.as_posix(), "branch id must be a non-empty string"))
            continue
        if branch_id in seen:
            issues.append((MANIFEST_PATH.as_posix(), f"duplicate branch id {branch_id}"))
        seen[branch_id] = str(branch_path)
        expected_path = EXPECTED_BRANCHES.get(branch_id)
        if expected_path is None:
            issues.append((MANIFEST_PATH.as_posix(), f"unexpected branch id {branch_id}"))
            continue
        if branch_path != expected_path:
            issues.append((MANIFEST_PATH.as_posix(), f"{branch_id}.path must be {expected_path}"))
        if not (root / expected_path).is_dir():
            issues.append((expected_path, "branch directory is missing"))
        if not isinstance(owner_surface, str) or not owner_surface:
            issues.append((MANIFEST_PATH.as_posix(), f"{branch_id}.owner_surface must be a non-empty string"))
        elif not (root / owner_surface).is_file():
            issues.append((owner_surface, f"{branch_id}.owner_surface is missing"))
        if "validators" in branch:
            issues.append((MANIFEST_PATH.as_posix(), f"{branch_id}.validators must move to validation_lanes"))
        if not isinstance(validation_lanes, list) or not validation_lanes:
            issues.append((MANIFEST_PATH.as_posix(), f"{branch_id}.validation_lanes must be a non-empty list"))
        else:
            for lane_id in validation_lanes:
                if not isinstance(lane_id, str) or not lane_id:
                    issues.append((MANIFEST_PATH.as_posix(), f"{branch_id}.validation_lanes must contain strings"))
                elif lane_id not in lane_ids:
                    issues.append((MANIFEST_PATH.as_posix(), f"{branch_id}.validation_lanes references missing lane {lane_id}"))

    missing = sorted(set(EXPECTED_BRANCHES) - set(seen))
    for branch_id in missing:
        issues.append((MANIFEST_PATH.as_posix(), f"missing branch id {branch_id}"))

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("ToS source-home validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print("[ok] validated ToS source-home manifest and branch topology")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

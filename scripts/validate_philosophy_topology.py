#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = Path("ToS/philosophy/philosophy.manifest.json")
RESEARCH_PACKET_ROOT = Path("ToS/research-packets")
FALSE_AUTHORITY_PATHS = (
    Path("ToS/source-witnesses/notion"),
    Path("ToS/source-witnesses/notion/philosophy"),
)

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


def slugify_label(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")


def collect_metadata_only_path_labels(payload: dict[str, object]) -> set[str]:
    labels: set[str] = set()
    capture_container = payload.get("capture_container")
    if isinstance(capture_container, dict):
        title = capture_container.get("original_title") or capture_container.get("title")
        if isinstance(title, str) and title:
            labels.add(slugify_label(title))
    child_pages = payload.get("branch_child_pages")
    if isinstance(child_pages, list):
        for child_page in child_pages:
            if isinstance(child_page, str):
                labels.add(slugify_label(child_page))
            elif isinstance(child_page, dict):
                title = child_page.get("original_title") or child_page.get("title")
                if isinstance(title, str) and title:
                    labels.add(slugify_label(title))
    return {label for label in labels if label}


def check_path_components(relative_path: Path, metadata_only_labels: set[str], issues: list[Issue]) -> None:
    lowered_parts = {part.lower() for part in relative_path.parts}
    for label in sorted(metadata_only_labels):
        if label in lowered_parts:
            issues.append((relative_path.as_posix(), f"metadata-only source label used as path component: {label}"))


def repo_relative_resolved(repo_root: Path, relative_path: Path) -> Path | None:
    root = repo_root.resolve()
    resolved = (root / relative_path).resolve()
    try:
        return resolved.relative_to(root)
    except ValueError:
        return None


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    required_files = [
        Path("ToS/philosophy/AGENTS.md"),
        Path("ToS/philosophy/README.md"),
        MANIFEST_PATH,
        Path("ToS/research-packets/AGENTS.md"),
    ]
    for relative_path in required_files:
        if not (root / relative_path).is_file():
            issues.append((relative_path.as_posix(), "missing required philosophy topology file"))

    manifest = load_json(root, MANIFEST_PATH, issues)
    if manifest is None:
        return issues

    for relative_path in FALSE_AUTHORITY_PATHS:
        if (root / relative_path).exists():
            issues.append((relative_path.as_posix(), "AI/Notion research packets must not live under source-witnesses"))

    if manifest.get("schema_version") != "tos_philosophy_topology_v1":
        issues.append((MANIFEST_PATH.as_posix(), "schema_version must be tos_philosophy_topology_v1"))
    if manifest.get("branch_id") != "philosophy":
        issues.append((MANIFEST_PATH.as_posix(), "branch_id must be philosophy"))
    if manifest.get("path") != "ToS/philosophy":
        issues.append((MANIFEST_PATH.as_posix(), "path must be ToS/philosophy"))

    boundary_routes = manifest.get("boundary_routes")
    if not isinstance(boundary_routes, dict):
        issues.append((MANIFEST_PATH.as_posix(), "boundary_routes must be an object"))
    else:
        expected_routes = {
            "provisional_extraction": "ToS/candidate-intake",
            "research_packets": "ToS/research-packets",
            "source_witnesses": "ToS/source-witnesses",
            "canon_promotion": "ToS/canon",
        }
        if "source_page_witnesses" in boundary_routes:
            issues.append((MANIFEST_PATH.as_posix(), "boundary_routes.source_page_witnesses must not route AI/Notion packets as source witnesses"))
        for key, expected_value in expected_routes.items():
            if boundary_routes.get(key) != expected_value:
                issues.append((MANIFEST_PATH.as_posix(), f"boundary_routes.{key} must be {expected_value}"))

    path_policy = manifest.get("path_component_policy")
    if not isinstance(path_policy, dict):
        issues.append((MANIFEST_PATH.as_posix(), "path_component_policy must be an object"))
    else:
        for key in ("repository_paths_describe", "metadata_only_inputs"):
            value = path_policy.get(key)
            if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
                issues.append((MANIFEST_PATH.as_posix(), f"path_component_policy.{key} must be a string list"))

    if "source_witness_routes" in manifest:
        issues.append((MANIFEST_PATH.as_posix(), "source_witness_routes must not point to AI/Notion research packets"))

    metadata_only_labels: set[str] = set()
    research_packet_routes = manifest.get("research_packet_routes")
    if not isinstance(research_packet_routes, list):
        issues.append((MANIFEST_PATH.as_posix(), "research_packet_routes must be a list when research packets are present"))
        research_packet_routes = []
    for entry in research_packet_routes:
        if not isinstance(entry, str) or not entry:
            issues.append((MANIFEST_PATH.as_posix(), "research_packet_routes entries must be non-empty strings"))
            continue
        packet_path = Path(entry)
        normalized_packet_path = repo_relative_resolved(root, packet_path)
        if packet_path.is_absolute() or ".." in packet_path.parts or normalized_packet_path is None:
            issues.append((entry, "research packet routes must be normalized repo-relative paths under ToS/research-packets"))
            continue
        packet_path = normalized_packet_path
        research_packet_root = (root.resolve() / RESEARCH_PACKET_ROOT).resolve()
        resolved_packet_path = (root.resolve() / packet_path).resolve()
        if not resolved_packet_path.is_relative_to(research_packet_root):
            issues.append((entry, "research packet routes must stay under ToS/research-packets"))
            continue
        if "source-witnesses" in packet_path.parts:
            issues.append((entry, "research packet routes must not point into source-witnesses"))
            continue
        packet_agents = packet_path.parent / "AGENTS.md"
        if not (root / packet_agents).is_file():
            issues.append((packet_agents.as_posix(), "research packet route must have a local AGENTS.md"))
        research_packet = load_json(root, packet_path, issues)
        if research_packet is None:
            continue
        metadata_only_labels.update(collect_metadata_only_path_labels(research_packet))
        if research_packet.get("schema_version") != "tos_research_packet_v1":
            issues.append((entry, "schema_version must be tos_research_packet_v1"))
        if research_packet.get("path") != packet_path.parent.as_posix():
            issues.append((entry, "path must match the research packet directory"))
        if research_packet.get("domain_branch") != "ToS/philosophy":
            issues.append((entry, "domain_branch must be ToS/philosophy"))
        authority = research_packet.get("authority")
        if not isinstance(authority, dict):
            issues.append((entry, "authority must be an object"))
        else:
            if authority.get("source_status") != "not_source_witness":
                issues.append((entry, "authority.source_status must be not_source_witness"))
            if authority.get("canon_status") != "not_canon":
                issues.append((entry, "authority.canon_status must be not_canon"))
        capture_container = research_packet.get("capture_container")
        if not isinstance(capture_container, dict) or not capture_container.get("page_id"):
            issues.append((entry, "capture_container.page_id is required"))
        child_pages = research_packet.get("branch_child_pages")
        if not isinstance(child_pages, list) or not child_pages:
            issues.append((entry, "branch_child_pages must be a non-empty list"))

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
            check_path_components(relative_path, metadata_only_labels, issues)
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

    for relative_path in Path("ToS").glob("**/*"):
        check_path_components(relative_path, metadata_only_labels, issues)

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

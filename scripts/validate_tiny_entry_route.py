#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]
Issue: TypeAlias = tuple[str, str]

ROUTE_PATH = Path("examples") / "tos_tiny_entry_route.example.json"
README_PATH = Path("README.md")
CHARTER_PATH = Path("CHARTER.md")
ROUTE_DOC_PATH = Path("docs") / "TINY_ENTRY_ROUTE.md"
CAPSULE_PATH = Path("docs") / "ZARATHUSTRA_TRILINGUAL_ENTRY.md"
KNOWLEDGE_MODEL_PATH = Path("docs") / "KNOWLEDGE_MODEL.md"
REVIEW_CHECKLIST_PATH = Path("docs") / "REVIEW_CHECKLIST.md"
SOURCE_NODE_PATH = Path("examples") / "source_node.example.json"
CONCEPT_NODE_PATH = Path("examples") / "concept_node.example.json"

EXPECTED_ROUTE_ID = "tos-tiny-entry.zarathustra-prologue"
EXPECTED_NODE_KIND = "source_node"
EXPECTED_ROOT_SURFACE = README_PATH.as_posix()
EXPECTED_CAPSULE_SURFACE = CAPSULE_PATH.as_posix()
EXPECTED_AUTHORITY_SURFACE = SOURCE_NODE_PATH.as_posix()
EXPECTED_BOUNDED_HOP = CONCEPT_NODE_PATH.as_posix()
EXPECTED_FALLBACK = KNOWLEDGE_MODEL_PATH.as_posix()
LEGACY_HOP_FIELD = "lineage_or_context_hop"

README_REQUIRED_PHRASES = (
    "keeps `README.md` as the public `tos-root`",
    "routes through a source-owned tiny-entry seam",
    "python scripts/validate_tiny_entry_route.py",
)
ROUTE_DOC_REQUIRED_PHRASES = (
    "public compatibility authority surface",
    "## Source-first re-entry",
    "`README.md -> examples/tos_tiny_entry_route.example.json -> examples/source_node.example.json`",
    "`aoa-routing` may restore this re-entry hop as bounded navigation",
    "python scripts/validate_tiny_entry_route.py",
)
REVIEW_CHECKLIST_REQUIRED_PHRASES = (
    "python scripts/validate_tiny_entry_route.py",
    "python scripts/validate_kag_export.py",
)
BOUNDARY_REQUIRED_PHRASES = (
    "does not replace ToS-authored authority",
    "does not delegate authority to aoa-kag, aoa-routing, or any other downstream derived system",
)
REQUIRED_FILES = (
    README_PATH,
    CHARTER_PATH,
    ROUTE_DOC_PATH,
    CAPSULE_PATH,
    KNOWLEDGE_MODEL_PATH,
    REVIEW_CHECKLIST_PATH,
    SOURCE_NODE_PATH,
    CONCEPT_NODE_PATH,
    ROUTE_PATH,
)


def normalize(text: str) -> str:
    return " ".join(text.lower().split())


def read_json(path: Path, *, repo_root: Path, issues: list[Issue]) -> object | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        issues.append((path.relative_to(repo_root).as_posix(), "missing required file"))
    except json.JSONDecodeError as exc:
        issues.append((path.relative_to(repo_root).as_posix(), f"invalid JSON: {exc}"))
    return None


def ensure_repo_relative_surface(
    value: object,
    *,
    location: str,
    repo_root: Path,
    issues: list[Issue],
) -> str | None:
    if not isinstance(value, str) or not value:
        issues.append((ROUTE_PATH.as_posix(), f"{location} must be a non-empty repo-relative path"))
        return None
    if ":" in value or value.startswith(("aoa-routing/", "aoa-kag/")):
        issues.append(
            (
                ROUTE_PATH.as_posix(),
                f"{location} must stay inside Tree-of-Sophia and must not point at downstream repos",
            )
        )
        return None
    if ".." in Path(value).parts:
        issues.append((ROUTE_PATH.as_posix(), f"{location} must not escape the repository root"))
        return None
    target = repo_root / value
    if not target.is_file():
        issues.append((ROUTE_PATH.as_posix(), f"{location} target '{value}' is missing"))
        return None
    return value


def require_phrases(
    *,
    relative_path: Path,
    repo_root: Path,
    required_phrases: tuple[str, ...],
    issues: list[Issue],
) -> None:
    path = repo_root / relative_path
    if not path.is_file():
        return
    text = normalize(path.read_text(encoding="utf-8"))
    for phrase in required_phrases:
        if normalize(phrase) not in text:
            issues.append((relative_path.as_posix(), f"missing required phrase: {phrase}"))


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    repo_root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    for relative_path in REQUIRED_FILES:
        if not (repo_root / relative_path).is_file():
            issues.append((relative_path.as_posix(), "missing required file"))

    route_payload = read_json(repo_root / ROUTE_PATH, repo_root=repo_root, issues=issues)
    source_payload = read_json(repo_root / SOURCE_NODE_PATH, repo_root=repo_root, issues=issues)

    if isinstance(route_payload, dict):
        route_id = route_payload.get("route_id")
        if route_id != EXPECTED_ROUTE_ID:
            issues.append(
                (ROUTE_PATH.as_posix(), f"route_id must stay '{EXPECTED_ROUTE_ID}' in the current bounded route")
            )

        root_surface = ensure_repo_relative_surface(
            route_payload.get("root_surface"),
            location="root_surface",
            repo_root=repo_root,
            issues=issues,
        )
        if root_surface is not None and root_surface != EXPECTED_ROOT_SURFACE:
            issues.append((ROUTE_PATH.as_posix(), f"root_surface must stay '{EXPECTED_ROOT_SURFACE}'"))

        node_kind = route_payload.get("node_kind")
        if node_kind != EXPECTED_NODE_KIND:
            issues.append((ROUTE_PATH.as_posix(), f"node_kind must stay '{EXPECTED_NODE_KIND}'"))

        node_id = route_payload.get("node_id")
        if not isinstance(node_id, str) or not node_id:
            issues.append((ROUTE_PATH.as_posix(), "node_id must stay a non-empty string"))
        elif isinstance(source_payload, dict) and source_payload.get("node_id") != node_id:
            issues.append(
                (ROUTE_PATH.as_posix(), "node_id must stay aligned with examples/source_node.example.json")
            )

        capsule_surface = ensure_repo_relative_surface(
            route_payload.get("capsule_surface"),
            location="capsule_surface",
            repo_root=repo_root,
            issues=issues,
        )
        if capsule_surface is not None and capsule_surface != EXPECTED_CAPSULE_SURFACE:
            issues.append(
                (ROUTE_PATH.as_posix(), f"capsule_surface must stay '{EXPECTED_CAPSULE_SURFACE}'")
            )

        authority_surface = ensure_repo_relative_surface(
            route_payload.get("authority_surface"),
            location="authority_surface",
            repo_root=repo_root,
            issues=issues,
        )
        if authority_surface is not None and authority_surface != EXPECTED_AUTHORITY_SURFACE:
            issues.append(
                (ROUTE_PATH.as_posix(), f"authority_surface must stay '{EXPECTED_AUTHORITY_SURFACE}'")
            )

        bounded_hop = ensure_repo_relative_surface(
            route_payload.get("bounded_hop"),
            location="bounded_hop",
            repo_root=repo_root,
            issues=issues,
        )
        if bounded_hop is not None and bounded_hop != EXPECTED_BOUNDED_HOP:
            issues.append((ROUTE_PATH.as_posix(), f"bounded_hop must stay '{EXPECTED_BOUNDED_HOP}'"))

        legacy_hop = route_payload.get(LEGACY_HOP_FIELD)
        if legacy_hop is not None:
            legacy_hop_path = ensure_repo_relative_surface(
                legacy_hop,
                location=LEGACY_HOP_FIELD,
                repo_root=repo_root,
                issues=issues,
            )
            if bounded_hop is not None and legacy_hop_path is not None and legacy_hop_path != bounded_hop:
                issues.append(
                    (
                        ROUTE_PATH.as_posix(),
                        f"{LEGACY_HOP_FIELD} must match bounded_hop during the compatibility transition",
                    )
                )

        fallback = ensure_repo_relative_surface(
            route_payload.get("fallback"),
            location="fallback",
            repo_root=repo_root,
            issues=issues,
        )
        if fallback is not None and fallback != EXPECTED_FALLBACK:
            issues.append((ROUTE_PATH.as_posix(), f"fallback must stay '{EXPECTED_FALLBACK}'"))

        if capsule_surface is not None and authority_surface is not None and capsule_surface == authority_surface:
            issues.append((ROUTE_PATH.as_posix(), "capsule_surface and authority_surface must remain distinct"))

        boundary = route_payload.get("non_identity_boundary")
        if not isinstance(boundary, str) or not boundary:
            issues.append((ROUTE_PATH.as_posix(), "non_identity_boundary must stay a non-empty string"))
        else:
            normalized_boundary = normalize(boundary)
            for phrase in BOUNDARY_REQUIRED_PHRASES:
                if normalize(phrase) not in normalized_boundary:
                    issues.append((ROUTE_PATH.as_posix(), f"non_identity_boundary must contain '{phrase}'"))

    require_phrases(
        relative_path=README_PATH,
        repo_root=repo_root,
        required_phrases=README_REQUIRED_PHRASES,
        issues=issues,
    )
    require_phrases(
        relative_path=ROUTE_DOC_PATH,
        repo_root=repo_root,
        required_phrases=ROUTE_DOC_REQUIRED_PHRASES,
        issues=issues,
    )
    require_phrases(
        relative_path=REVIEW_CHECKLIST_PATH,
        repo_root=repo_root,
        required_phrases=REVIEW_CHECKLIST_REQUIRED_PHRASES,
        issues=issues,
    )

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Tiny-entry route check failed.")
        for location, message in issues:
            print(f"- {location}: {message}")
        return 1

    print("Tiny-entry route check passed for the current bounded route.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

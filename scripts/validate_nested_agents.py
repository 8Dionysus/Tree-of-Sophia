#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
from typing import TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]

Issue: TypeAlias = tuple[str, str]

ROUTE_ROOTS = (
    "ToS",
    "docs",
    "mechanics",
    "scripts",
    "tests",
    "evals",
)
ROOT_ROUTE_CARDS = (
    Path("AGENTS.md"),
)
BANNED_ROUTE_SECTION_MARKERS = (
    "## Stop Lines",
    "## Hard no",
)
ROLE_HEADINGS = (
    "## Role",
    "## Local role",
    "## What lives here",
)
ORIENTATION_HEADINGS = (
    "## Read First",
    "## Read first",
    "## Read Before Editing",
    "## Read before editing",
    "## Applies To",
    "## Applies to",
    "This file applies",
    "This card applies",
)
ROUTE_HEADINGS = (
    "## Operating Card",
    "## Boundary Routes",
    "## Boundary Law",
    "## Boundaries",
    "## Boundary",
    "## Authority",
    "## Editing posture",
    "## Validation",
    "## Verify",
)
OPERATING_CARD_FIELDS = (
    "input",
    "output",
    "owner",
    "next route",
)
OPERATING_CARD_CHECK_FIELDS = (
    "check",
    "validation",
    "tools",
)
REQUIRED_REFERENCES: dict[str, tuple[str, ...]] = {
    "AGENTS.md": (
        "README.md",
        "ROADMAP.md",
        "BOUNDARIES.md",
        "ToS/",
        "mechanics/",
    ),
    "ToS/AGENTS.md": (
        "ToS/source_home.manifest.json",
        "ToS/doctrine/KNOWLEDGE_MODEL.md",
        "ToS/doctrine/NODE_CONTRACT.md",
    ),
    "ToS/philosophy/AGENTS.md": (
        "ToS/philosophy/",
        "ToS/research-packets/deep-research/philosophy/",
        "ToS/candidate-intake/",
        "ToS/canon/",
    ),
    "docs/AGENTS.md": (
        "docs/decisions/",
        "docs/RELEASING.md",
        "docs/AGENTS_ROOT_REFERENCE.md",
    ),
    "docs/decisions/AGENTS.md": (
        "TOS-D-####",
        "## Index Metadata",
        "docs/decisions/indexes/index_contract.yaml",
    ),
    "mechanics/AGENTS.md": (
        "mechanics/topology.json",
        "PARTS.md",
        "PROVENANCE.md",
        "ROADMAP.md",
    ),
    "scripts/AGENTS.md": (
        "docs/validation/validation_lanes.json",
        "scripts/release_check.py",
    ),
    "tests/AGENTS.md": (
        "tests/test_inventory.json",
        "python -m unittest discover -s tests",
    ),
    "evals/AGENTS.md": ("aoa-evals",),
}


def discover_route_cards(repo_root: Path) -> list[Path]:
    paths = [repo_root / path for path in ROOT_ROUTE_CARDS]
    for root_name in ROUTE_ROOTS:
        root = repo_root / root_name
        if root.exists():
            paths.extend(sorted(root.rglob("AGENTS.md")))
    return sorted({path for path in paths if path.is_file()})


def normalize(text: str) -> str:
    return " ".join(text.lower().split())


def has_any(text: str, needles: tuple[str, ...]) -> bool:
    return any(needle in text for needle in needles)


def table_fields(text: str) -> set[str]:
    fields: set[str] = set()
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped.startswith("|") or "---" in stripped:
            continue
        cells = [cell.strip().strip("`").lower() for cell in stripped.strip("|").split("|")]
        if len(cells) >= 2 and cells[0] != "field":
            fields.add(cells[0])
    return fields


def validate_operating_card(relative_path: str, text: str, issues: list[Issue]) -> None:
    if "## Operating Card" not in text:
        return
    fields = table_fields(text)
    for field in OPERATING_CARD_FIELDS:
        if field not in fields:
            issues.append((relative_path, f"Operating Card missing field: {field}"))
    if not any(field in fields for field in OPERATING_CARD_CHECK_FIELDS):
        issues.append((relative_path, "Operating Card needs check, validation, or tools field"))


def validate_references(repo_root: Path, relative_path: str, text: str, issues: list[Issue]) -> None:
    for reference in REQUIRED_REFERENCES.get(relative_path, ()):
        if reference not in text:
            issues.append((relative_path, f"missing stable route reference: {reference}"))
            continue
        if reference.endswith("/") or reference == "TOS-D-####" or reference == "## Index Metadata":
            continue
        if "/" not in reference and not reference.startswith("."):
            continue
        if reference.startswith("python "):
            continue
        if reference == "aoa-evals":
            continue
        if not (repo_root / reference).exists():
            issues.append((relative_path, f"stable route reference points to missing path: {reference}"))


def is_compact_stub(text: str) -> bool:
    headings = [line for line in text.splitlines() if line.startswith("## ")]
    return not headings and ("This file applies" in text or "This card applies" in text)


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    repo_root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    route_cards = discover_route_cards(repo_root)
    for path in route_cards:
        relative_path = path.relative_to(repo_root).as_posix()
        raw_text = path.read_text(encoding="utf-8")
        stripped = raw_text.strip()
        if not stripped.startswith("# AGENTS.md"):
            issues.append((relative_path, "missing '# AGENTS.md' heading"))

        compact_stub = is_compact_stub(raw_text)
        if not compact_stub:
            if not has_any(raw_text, ROLE_HEADINGS):
                issues.append((relative_path, "missing role or local scope heading"))
            if not has_any(raw_text, ORIENTATION_HEADINGS):
                issues.append((relative_path, "missing read/apply orientation heading"))
            if not has_any(raw_text, ROUTE_HEADINGS):
                issues.append((relative_path, "missing route, boundary, or validation heading"))

        for marker in BANNED_ROUTE_SECTION_MARKERS:
            if marker in raw_text:
                issues.append((relative_path, f"use Operating Card/Boundary Routes instead of {marker}"))

        validate_operating_card(relative_path, raw_text, issues)
        validate_references(repo_root, relative_path, raw_text, issues)

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Nested AGENTS route-card check failed.")
        for location, message in issues:
            print(f"- {location}: {message}")
        return 1

    print(f"Nested AGENTS route-card check passed for {len(discover_route_cards(REPO_ROOT))} files.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

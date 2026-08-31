#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path
from typing import Any, TypeAlias


REPO_ROOT = Path(__file__).resolve().parents[1]

Issue: TypeAlias = tuple[str, str]

ROUTE_ROOTS = (
    ".github",
    "ToS",
    "docs",
    "mechanics",
    "scripts",
    "tests",
    "evals",
    "memo",
    "kag",
    "stats",
    "manifests",
    ".agents",
)
ROOT_ROUTE_CARDS = (
    Path("AGENTS.md"),
)
INVENTORY_RELATIVE = Path("docs/validation/agents_route_inventory.json")
CURRENTNESS_RELATIVE = Path(".agents/agents-route.current.json")
LOCAL_SCRIPT_REF_RE = re.compile(r"(?<![\w/.])((?:scripts|mechanics|\.agents)/[^\s`)'\"]+\.py)")
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
        "VALIDATION.md",
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
    "stats/AGENTS.md": (
        "stats/port.manifest.json",
        "aoa-stats",
    ),
}


def discover_route_cards(repo_root: Path) -> list[Path]:
    paths = [repo_root / path for path in ROOT_ROUTE_CARDS]
    roots = ROUTE_ROOTS
    inventory_path = repo_root / INVENTORY_RELATIVE
    if inventory_path.is_file():
        try:
            roots = tuple(json.loads(inventory_path.read_text(encoding="utf-8"))["route_card_discovery"]["route_roots"])
        except (OSError, KeyError, TypeError, ValueError):
            roots = ROUTE_ROOTS
    for root_name in roots:
        root = repo_root / root_name
        if root.exists():
            paths.extend(sorted(root.rglob("AGENTS.md")))
    return sorted({path for path in paths if path.is_file()})


def load_route_inventory(repo_root: Path) -> dict[str, Any] | None:
    path = repo_root / INVENTORY_RELATIVE
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, TypeError, ValueError):
        return None


def tracked_route_cards(repo_root: Path) -> set[str] | None:
    try:
        result = subprocess.run(
            ("git", "ls-files", "--", "*.md"),
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return {
        value
        for value in result.stdout.splitlines()
        if Path(value).name == "AGENTS.md"
    }


def inheritance_stack(repo_root: Path, card: Path, discovered: set[Path]) -> list[Path]:
    ancestors: list[Path] = []
    cursor = card.parent
    while True:
        candidate = repo_root / "AGENTS.md" if cursor == repo_root else cursor / "AGENTS.md"
        if candidate in discovered:
            ancestors.append(candidate)
        if cursor == repo_root:
            break
        cursor = cursor.parent
    ancestors.reverse()
    if card not in ancestors:
        ancestors.append(card)
    return ancestors


def duplicate_inherited_card_issues(repo_root: Path, route_cards: list[Path]) -> list[Issue]:
    issues: list[Issue] = []
    discovered = set(route_cards)
    for card in route_cards:
        stack = inheritance_stack(repo_root, card, discovered)
        for ancestor in stack[:-1]:
            if card.read_bytes() == ancestor.read_bytes():
                issues.append((card.relative_to(repo_root).as_posix(), f"byte-identical nested card duplicates {ancestor.relative_to(repo_root).as_posix()}"))
    return issues


def validate_route_inventory(repo_root: Path, route_cards: list[Path], issues: list[Issue]) -> None:
    inventory = load_route_inventory(repo_root)
    if inventory is None:
        path = repo_root / INVENTORY_RELATIVE
        if path.is_file():
            issues.append((INVENTORY_RELATIVE.as_posix(), "route inventory is unreadable or malformed"))
        else:
            issues.append((INVENTORY_RELATIVE.as_posix(), "route inventory is missing"))
        return
    required_top_level = {
        "schema_version",
        "owner_repo",
        "owner_surface",
        "validator",
        "currentness",
        "harness",
        "route_card_discovery",
        "influencing_surfaces",
        "context_budget",
        "scope_duplication_policy",
        "task_routes",
    }
    missing = sorted(required_top_level - set(inventory))
    if missing:
        issues.append((INVENTORY_RELATIVE.as_posix(), f"inventory missing fields: {', '.join(missing)}"))
        return
    if inventory["schema_version"] != "tos_agents_route_inventory_v1":
        issues.append((INVENTORY_RELATIVE.as_posix(), "unsupported route inventory schema"))
    for reference in (inventory["owner_surface"], inventory["validator"], inventory["currentness"], inventory["harness"]):
        if not (repo_root / reference).is_file():
            issues.append((INVENTORY_RELATIVE.as_posix(), f"inventory reference points to missing path: {reference}"))

    discovery = inventory["route_card_discovery"]
    discovered_names = {path.relative_to(repo_root).as_posix() for path in route_cards}
    tracked_names = tracked_route_cards(repo_root)
    if tracked_names is not None:
        tracked_exact = {value for value in tracked_names if Path(value).name == "AGENTS.md"}
        for value in sorted(tracked_exact - discovered_names):
            issues.append((value, "tracked AGENTS.md is outside route-card inventory roots"))
        for value in sorted(discovered_names - tracked_exact):
            issues.append((value, "discovered AGENTS.md is not tracked"))
    if "AGENTS.md" not in discovery.get("root_cards", []):
        issues.append((INVENTORY_RELATIVE.as_posix(), "route inventory must name the root AGENTS.md"))
    if ".github" not in discovery.get("route_roots", []):
        issues.append((INVENTORY_RELATIVE.as_posix(), "route inventory must include .github for tracked card parity"))

    discovered = set(route_cards)
    for card in route_cards:
        stack = inheritance_stack(repo_root, card, discovered)
        if not stack or stack[0] != repo_root / "AGENTS.md":
            issues.append((card.relative_to(repo_root).as_posix(), "inheritance stack must begin at root AGENTS.md"))
    issues.extend(duplicate_inherited_card_issues(repo_root, route_cards))

    preserved = discovery.get("preserved_non_cards", [])
    for entry in preserved:
        value = entry.get("path", "")
        path = repo_root / value
        if not path.is_file():
            issues.append((INVENTORY_RELATIVE.as_posix(), f"preserved non-card is missing: {value}"))
            continue
        text = path.read_text(encoding="utf-8")
        if value == "docs/AGENTS_ROOT_REFERENCE.md":
            if "../AGENTS.md" not in text or "competing root" not in text:
                issues.append((value, "preserved root reference must point to live root and reject competing-root authority"))
        if value == "DESIGN.AGENTS.md" and Path(value).name == "AGENTS.md":
            issues.append((value, "DESIGN.AGENTS.md must remain a shape reference, not an inherited route card"))

    for entry in inventory["influencing_surfaces"]:
        value = entry.get("path", "")
        if not (repo_root / value).exists():
            issues.append((INVENTORY_RELATIVE.as_posix(), f"influencing surface is missing: {value}"))

    task_ids: set[str] = set()
    for task in inventory["task_routes"]:
        task_id = task.get("id", "")
        if not task_id or task_id in task_ids:
            issues.append((INVENTORY_RELATIVE.as_posix(), f"task route id is missing or duplicated: {task_id or '<empty>'}"))
        task_ids.add(task_id)
        for key in ("target", "owner_route", "on_demand_surfaces", "validation_paths", "completion_evidence", "handoff_routes"):
            values = task.get(key, []) if key in {"on_demand_surfaces", "validation_paths", "completion_evidence", "handoff_routes"} else [task.get(key, "")]
            if isinstance(values, str):
                values = [values]
            for value in values:
                if value and not (repo_root / value).exists() and not value.startswith("aoa-"):
                    issues.append((INVENTORY_RELATIVE.as_posix(), f"task {task_id} {key} points to missing path: {value}"))

    currentness_path = repo_root / inventory["currentness"]
    if currentness_path.is_file():
        try:
            from build_agents_route_currentness import build_currentness, render_currentness

            expected = render_currentness(build_currentness(repo_root))
            actual = currentness_path.read_text(encoding="utf-8")
            if actual != expected:
                issues.append((currentness_path.relative_to(repo_root).as_posix(), "generated AGENTS route currentness is stale"))
        except (OSError, ImportError, KeyError, TypeError, ValueError, subprocess.CalledProcessError) as exc:
            issues.append((currentness_path.relative_to(repo_root).as_posix(), f"cannot verify generated currentness: {exc}"))
    else:
        issues.append((inventory["currentness"], "generated AGENTS route currentness is missing"))


def validate_local_script_references(repo_root: Path, route_cards: list[Path], issues: list[Issue]) -> None:
    for card in route_cards:
        text = card.read_text(encoding="utf-8")
        for match in LOCAL_SCRIPT_REF_RE.finditer(text):
            reference = match.group(1).rstrip(".,:;")
            if not (repo_root / reference).is_file():
                issues.append((card.relative_to(repo_root).as_posix(), f"local executable reference points to missing path: {reference}"))


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

    validate_route_inventory(repo_root, route_cards, issues)
    validate_local_script_references(repo_root, route_cards, issues)
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

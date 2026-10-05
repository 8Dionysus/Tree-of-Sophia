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

# Active AGENTS cards are prompt-light semantic deltas. Executable procedure
# belongs to VALIDATION.md, the lane manifest, or the named owner document.
# Keep these guards syntax-focused so ordinary source paths and prose remain
# valid guidance.
COMMAND_TOKEN_PATTERN = (
    r"(?:python(?:\d+(?:\.\d+)*)?|pytest|uv(?:[ \t]+run)?|pip(?:\d+)?|"
    r"npm|npx|node|cargo|go|git|make|bash|sh|zsh|fish|powershell|pwsh|"
    r"systemctl|journalctl|curl|wget|jq|rg|grep|sed|awk|find|chmod|mkdir|"
    r"cp|mv|rm|docker|podman|env|source(?=[ \t]+[./~$\"']))"
)
ENV_ASSIGNMENT_PATTERN = (
    r"(?:[A-Z_][A-Z0-9_]*=(?:\"[^\"]*\"|'[^']*'|[^\s]+)[ \t]+)"
)
SHELL_FENCE_PATTERN = re.compile(r"^ {0,3}```")
COMMAND_START_PATTERN = re.compile(
    r"^[ \t]*(?:(?:[-*+][ \t]+|\d+[.)][ \t]+))?(?:`)?(?:\$[ \t]+)?"
    + ENV_ASSIGNMENT_PATTERN
    + r"*"
    + COMMAND_TOKEN_PATTERN
    + r"(?=[ \t]|$|`)",
    re.IGNORECASE,
)
ENV_ASSIGNMENT_COMMAND_PATTERN = re.compile(
    r"^[ \t]*(?:(?:[-*+][ \t]+|\d+[.)][ \t]+))?(?:`)?"
    + ENV_ASSIGNMENT_PATTERN
    + r"+"
    + COMMAND_TOKEN_PATTERN
    + r"(?=[ \t]|$|`)",
    re.IGNORECASE,
)
INLINE_COMMAND_PATTERN = re.compile(
    r"`(?:\$[ \t]+)?"
    + ENV_ASSIGNMENT_PATTERN
    + r"*"
    + COMMAND_TOKEN_PATTERN
    + r"(?=[ \t]|$|`)[^`]*`",
    re.IGNORECASE,
)
README_ROUTE_PATTERN = re.compile(
    r"\b(?:read|open|review|start\s+with|use)\b.*\bREADME(?:\.md)?\b",
    re.IGNORECASE,
)
README_TASK_CONDITION_PATTERN = re.compile(
    r"(?:\b(?:when|if|where|after|only|touched)\b|"
    r"\b(?:as|when|if)\s+needed\b|"
    r"\b(?:relevant|selected|known|target|named)\b)",
    re.IGNORECASE,
)
PROCEDURAL_HEADING_PATTERN = re.compile(
    r"^#{1,6}\s+(?:validation|verify|verification|checks?|testing|"
    r"procedure|smoke)(?:\s|$)",
    re.IGNORECASE,
)
HEADING_PATTERN = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
BULLET_PATTERN = re.compile(r"^(?P<indent>\s*)(?P<marker>[-*+]|\d+[.)])\s+")
README_INVENTORY_HEADINGS = {
    "start here",
    "read first",
    "read before editing",
    "reading order",
    "route stack",
    "route inventory",
    "required reading",
}


def _has_readme_condition(text: str) -> bool:
    return README_TASK_CONDITION_PATTERN.search(text) is not None
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
        "tests/VALIDATION.md",
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


def _next_nonblank_index(lines: list[str], index: int) -> int:
    next_index = index + 1
    while next_index < len(lines) and not lines[next_index].strip():
        next_index += 1
    return next_index


def _unconditional_readme_inventory_issues(relative_path: str, text: str) -> list[Issue]:
    issues: list[Issue] = []
    lines = text.splitlines()
    for line_number, line in enumerate(lines, start=1):
        if not README_ROUTE_PATTERN.search(line):
            continue
        if not _has_readme_condition(line):
            issues.append((relative_path, f"line {line_number}: unconditional README inventory"))

    for index, line in enumerate(lines):
        match = HEADING_PATTERN.match(line)
        if not match or match.group(2).strip().lower() not in README_INVENTORY_HEADINGS:
            continue
        end = index + 1
        while end < len(lines) and not HEADING_PATTERN.match(lines[end]):
            end += 1
        section = lines[index + 1 : end]
        if any("readme" in item.lower() for item in section) and not any(
            _has_readme_condition(item) for item in section
        ):
            issues.append((relative_path, f"line {index + 1}: unconditional README inventory section"))
    return issues


def _procedural_structure_issues(
    relative_path: str,
    text: str,
    *,
    has_validation_route: bool | None = None,
) -> list[Issue]:
    """Reject executable and extraction residue in active cards only."""

    issues: list[Issue] = []
    lines = text.splitlines()
    seen_headings: set[tuple[int, str]] = set()
    validation_route_present = (
        "VALIDATION.md" in text
        if has_validation_route is None
        else has_validation_route or "VALIDATION.md" in text
    )

    for line_number, line in enumerate(lines, start=1):
        if SHELL_FENCE_PATTERN.match(line):
            issues.append((relative_path, f"line {line_number}: fenced procedure/example is not allowed"))
        if ENV_ASSIGNMENT_COMMAND_PATTERN.match(line):
            issues.append((relative_path, f"line {line_number}: environment-assignment command is not allowed"))
        elif COMMAND_START_PATTERN.match(line):
            issues.append((relative_path, f"line {line_number}: runnable command line is not allowed"))
        if INLINE_COMMAND_PATTERN.search(line):
            issues.append((relative_path, f"line {line_number}: inline runnable command is not allowed"))

        heading = HEADING_PATTERN.match(line)
        if heading:
            key = (len(heading.group(1)), normalize(heading.group(2)))
            if key in seen_headings:
                issues.append((relative_path, f"line {line_number}: repeated heading {heading.group(2)!r}"))
            seen_headings.add(key)

    for index, line in enumerate(lines):
        if not PROCEDURAL_HEADING_PATTERN.match(line):
            continue
        end = index + 1
        while end < len(lines) and not HEADING_PATTERN.match(lines[end]):
            end += 1
        if not any(item.strip() for item in lines[index + 1 : end]):
            title = line.lstrip("#").strip()
            issues.append((relative_path, f"line {index + 1}: empty procedural section {title!r}"))

    for index, line in enumerate(lines):
        stripped = line.strip()
        if not stripped.endswith(":"):
            continue
        next_index = _next_nonblank_index(lines, index)
        if next_index == len(lines):
            issues.append((relative_path, f"line {index + 1}: orphan extraction lead-in before EOF"))
            continue

        next_line = lines[next_index]
        if HEADING_PATTERN.match(next_line) or SHELL_FENCE_PATTERN.match(next_line):
            issues.append((relative_path, f"line {index + 1}: orphan extraction lead-in before structure"))
        if next_line.strip().endswith(":"):
            issues.append((relative_path, f"line {index + 1}: stacked colon lead-ins are not allowed"))

        current_bullet = BULLET_PATTERN.match(line)
        next_bullet = BULLET_PATTERN.match(next_line)
        if current_bullet and next_bullet:
            current_indent = len(current_bullet.group("indent"))
            next_indent = len(next_bullet.group("indent"))
            if current_indent == next_indent:
                issues.append((relative_path, f"line {index + 1}: same-level bullet lead-in has no body"))

    issues.extend(_unconditional_readme_inventory_issues(relative_path, text))
    if not validation_route_present:
        issues.append((relative_path, "missing nearest validation route"))
    return issues


def validate_active_agent_structure(repo_root: Path) -> list[Issue]:
    """Validate prompt-light structure for every active route card."""

    issues: list[Issue] = []
    route_cards = discover_route_cards(repo_root)
    discovered = set(route_cards)
    for path in route_cards:
        relative_path = path.relative_to(repo_root).as_posix()
        stack = inheritance_stack(repo_root, path, discovered)
        inherited_route = any(
            "VALIDATION.md" in ancestor.read_text(encoding="utf-8")
            for ancestor in stack
        )
        issues.extend(
            _procedural_structure_issues(
                relative_path,
                path.read_text(encoding="utf-8"),
                has_validation_route=inherited_route,
            )
        )
    return issues


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
    issues.extend(validate_active_agent_structure(repo_root))
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

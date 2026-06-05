#!/usr/bin/env python3
"""Generate docs/decisions lookup indexes from decision-note metadata."""

from __future__ import annotations

import argparse
from collections import defaultdict
from dataclasses import dataclass
from datetime import date
from pathlib import Path
import re


REPO_ROOT = Path(__file__).resolve().parents[1]
DECISIONS_DIR = Path("docs/decisions")
INDEX_DIR = DECISIONS_DIR / "indexes"
DECISION_PREFIX = "TOS-D"
DECISION_RE = re.compile(r"^TOS-D-(\d{4})-[a-z0-9][a-z0-9-]*\.md$")
DECISION_ID_RE = re.compile(r"^TOS-D-\d{4}$")
STATIC_MARKDOWN = {"AGENTS.md", "README.md", "TEMPLATE.md"}

INDEX_PATHS = (
    INDEX_DIR / "README.md",
    INDEX_DIR / "by-number.md",
    INDEX_DIR / "by-date.md",
    INDEX_DIR / "by-surface.md",
    INDEX_DIR / "by-tos-layer.md",
    INDEX_DIR / "by-tree-class.md",
    INDEX_DIR / "by-guard.md",
)


@dataclass(frozen=True)
class DecisionRecord:
    decision_id: str
    number: str
    title: str
    path: str
    original_date: str
    surface_classes: tuple[str, ...]
    tos_layers: tuple[str, ...]
    tree_classes: tuple[str, ...]
    guard_families: tuple[str, ...]
    posture: str


def _split_values(value: str) -> tuple[str, ...]:
    items = tuple(item.strip() for item in value.split(",") if item.strip())
    return items or ("none",)


def _metadata_value(metadata: dict[str, str], key: str, location: str, issues: list[tuple[str, str]]) -> str:
    value = metadata.get(key, "").strip()
    if not value:
        issues.append((location, f"missing Index Metadata field '{key}'"))
    return value


def _parse_metadata(text: str, location: str, issues: list[tuple[str, str]]) -> dict[str, str]:
    lines = text.splitlines()
    try:
        start = lines.index("## Index Metadata") + 1
    except ValueError:
        issues.append((location, "missing '## Index Metadata' block"))
        return {}

    metadata: dict[str, str] = {}
    for line in lines[start:]:
        if line.startswith("## "):
            break
        if not line.startswith("- ") or ":" not in line:
            continue
        key, value = line[2:].split(":", 1)
        metadata[key.strip()] = value.strip()
    return metadata


def collect_decision_records(repo_root: Path = REPO_ROOT) -> tuple[list[DecisionRecord], list[tuple[str, str]]]:
    repo_root = repo_root.resolve()
    decision_root = repo_root / DECISIONS_DIR
    issues: list[tuple[str, str]] = []
    records: list[DecisionRecord] = []

    if not decision_root.is_dir():
        return records, [(DECISIONS_DIR.as_posix(), "decision directory is missing")]

    for path in sorted(decision_root.glob("*.md")):
        rel = path.relative_to(repo_root).as_posix()
        if path.name in STATIC_MARKDOWN:
            continue
        if not path.name.startswith(f"{DECISION_PREFIX}-"):
            issues.append((rel, "top-level decision markdown must use TOS-D-####-slug.md"))

    for path in sorted(decision_root.glob(f"{DECISION_PREFIX}-*.md")):
        rel = path.relative_to(repo_root).as_posix()
        match = DECISION_RE.match(path.name)
        if not match:
            issues.append((rel, "filename must match TOS-D-####-slug.md"))
            continue

        text = path.read_text(encoding="utf-8")
        title = next((line[2:].strip() for line in text.splitlines() if line.startswith("# ")), "")
        if not title:
            issues.append((rel, "missing H1 title"))

        metadata = _parse_metadata(text, rel, issues)
        decision_id = _metadata_value(metadata, "Decision ID", rel, issues)
        original_date = _metadata_value(metadata, "Original date", rel, issues)
        surface_classes = _metadata_value(metadata, "Surface classes", rel, issues)
        tos_layers = _metadata_value(metadata, "ToS layers", rel, issues)
        tree_classes = _metadata_value(metadata, "Tree classes", rel, issues)
        guard_families = _metadata_value(metadata, "Guard families", rel, issues)
        posture = _metadata_value(metadata, "Posture", rel, issues)

        expected_id = f"{DECISION_PREFIX}-{match.group(1)}"
        if decision_id and decision_id != expected_id:
            issues.append((rel, f"Decision ID must match filename prefix '{expected_id}'"))
        if decision_id and not DECISION_ID_RE.match(decision_id):
            issues.append((rel, "Decision ID must match TOS-D-####"))
        if original_date:
            try:
                date.fromisoformat(original_date)
            except ValueError:
                issues.append((rel, "Original date must use YYYY-MM-DD"))

        if decision_id and original_date and title:
            records.append(
                DecisionRecord(
                    decision_id=decision_id,
                    number=match.group(1),
                    title=title,
                    path=rel,
                    original_date=original_date,
                    surface_classes=_split_values(surface_classes),
                    tos_layers=_split_values(tos_layers),
                    tree_classes=_split_values(tree_classes),
                    guard_families=_split_values(guard_families),
                    posture=posture or "unknown",
                )
            )

    if not records:
        issues.append((DECISIONS_DIR.as_posix(), "no canonical decision records found"))

    seen: dict[str, str] = {}
    for record in records:
        if record.decision_id in seen:
            issues.append((record.path, f"duplicate Decision ID also used by {seen[record.decision_id]}"))
        seen[record.decision_id] = record.path

    numbers = sorted(int(record.number) for record in records)
    if numbers:
        expected_numbers = list(range(1, numbers[-1] + 1))
        if numbers != expected_numbers:
            issues.append((DECISIONS_DIR.as_posix(), "decision numbers must be contiguous from TOS-D-0001"))

    return records, issues


def validate_index_contract(repo_root: Path = REPO_ROOT) -> list[tuple[str, str]]:
    repo_root = repo_root.resolve()
    path = repo_root / INDEX_DIR / "index_contract.yaml"
    issues: list[tuple[str, str]] = []
    if not path.is_file():
        return [(path.relative_to(repo_root).as_posix(), "index contract is missing")]
    text = path.read_text(encoding="utf-8")
    required = [
        "schema_version: tos_decision_index_contract_v1",
        "authority: docs/decisions/AGENTS.md",
        "source_records: docs/decisions/TOS-D-*.md",
        "generated_by: scripts/generate_decision_indexes.py",
        "pattern: TOS-D-####",
        "path_mode: full_canonical_id_filename",
    ]
    rel = path.relative_to(repo_root).as_posix()
    for snippet in required:
        if snippet not in text:
            issues.append((rel, f"missing contract snippet {snippet!r}"))
    for index_path in INDEX_PATHS:
        snippet = f"- {index_path.as_posix()}"
        if snippet not in text:
            issues.append((rel, f"missing generated index path {index_path.as_posix()}"))
    return issues


def _contract_list_values(repo_root: Path, key: str) -> set[str]:
    path = repo_root / INDEX_DIR / "index_contract.yaml"
    if not path.is_file():
        return set()
    values: set[str] = set()
    in_list = False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line == f"{key}:":
            in_list = True
            continue
        if in_list and line.startswith("  - "):
            values.add(line[4:].strip().strip("'\""))
            continue
        if in_list and line and not line.startswith(" "):
            break
    return values


def _modeled_contract_surfaces(repo_root: Path, issues: list[tuple[str, str]]) -> set[str]:
    allowed: set[str] = set()
    for item in _contract_list_values(repo_root, "modeled_surfaces"):
        relative = Path(item)
        if relative.is_absolute() or ".." in relative.parts:
            issues.append(((INDEX_DIR / "index_contract.yaml").as_posix(), f"modeled_surfaces entry must be a normalized repo-relative path under {DECISIONS_DIR.as_posix()}: {item}"))
            continue
        try:
            relative.relative_to(DECISIONS_DIR)
        except ValueError:
            issues.append(((INDEX_DIR / "index_contract.yaml").as_posix(), f"modeled_surfaces entry must live under {DECISIONS_DIR.as_posix()}: {item}"))
            continue
        if relative.parent == DECISIONS_DIR and relative.suffix == ".md" and relative.name not in STATIC_MARKDOWN and not DECISION_RE.match(relative.name):
            issues.append(((INDEX_DIR / "index_contract.yaml").as_posix(), f"modeled_surfaces must not include root non-record Markdown: {item}"))
            continue
        if not (repo_root / relative).is_file():
            issues.append(((INDEX_DIR / "index_contract.yaml").as_posix(), f"modeled_surfaces entry does not exist: {item}"))
            continue
        allowed.add(item)
    return allowed


def validate_decision_lane_surfaces(repo_root: Path = REPO_ROOT) -> list[tuple[str, str]]:
    repo_root = repo_root.resolve()
    decision_root = repo_root / DECISIONS_DIR
    if not decision_root.is_dir():
        return [(DECISIONS_DIR.as_posix(), "decision directory is missing")]

    issues: list[tuple[str, str]] = []
    allowed_paths = {
        (DECISIONS_DIR / "AGENTS.md").as_posix(),
        (DECISIONS_DIR / "README.md").as_posix(),
        (DECISIONS_DIR / "TEMPLATE.md").as_posix(),
        (INDEX_DIR / "index_contract.yaml").as_posix(),
        *(path.as_posix() for path in INDEX_PATHS),
        *_modeled_contract_surfaces(repo_root, issues),
    }
    for path in sorted(decision_root.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(repo_root)
        relative_text = relative.as_posix()
        if relative_text in allowed_paths:
            continue
        decision_relative = path.relative_to(decision_root)
        if len(decision_relative.parts) == 1 and DECISION_RE.match(path.name):
            continue
        issues.append(
            (
                relative_text,
                "unmodeled decision-lane surface; add it to modeled_surfaces in docs/decisions/indexes/index_contract.yaml or move it outside docs/decisions",
            )
        )
    return issues


def _record_link(record: DecisionRecord) -> str:
    return f"[{record.decision_id} - {record.title}](../{Path(record.path).name})"


def _bullet_line(record: DecisionRecord) -> str:
    return f"- {_record_link(record)} (`{record.path}`)"


def _table(records: list[DecisionRecord], *, columns: tuple[str, ...]) -> str:
    header = "| " + " | ".join(columns) + " |\n"
    divider = "| " + " | ".join("---" for _ in columns) + " |\n"
    rows: list[str] = []
    for record in records:
        values: list[str] = []
        for column in columns:
            if column == "Decision ID":
                values.append(record.decision_id)
            elif column == "Decision":
                values.append(_record_link(record))
            elif column == "Date":
                values.append(record.original_date)
            elif column == "Path":
                values.append(f"`{record.path}`")
            elif column == "Posture":
                values.append(record.posture)
            elif column == "Surface classes":
                values.append(", ".join(record.surface_classes))
            elif column == "ToS layers":
                values.append(", ".join(record.tos_layers))
            elif column == "Tree classes":
                values.append(", ".join(record.tree_classes))
            elif column == "Guard families":
                values.append(", ".join(record.guard_families))
            else:
                raise ValueError(f"unknown column {column}")
        rows.append("| " + " | ".join(values) + " |\n")
    return header + divider + "".join(rows)


def _grouped_index(title: str, groups: dict[str, list[DecisionRecord]]) -> str:
    lines = [
        f"# {title}",
        "",
        "<!-- Generated by scripts/generate_decision_indexes.py; do not edit by hand. -->",
    ]
    for group_name in sorted(groups, key=str.lower):
        lines.extend(["", f"## {group_name}", ""])
        for record in sorted(groups[group_name], key=lambda item: item.decision_id):
            lines.append(_bullet_line(record))
    return "\n".join(lines).rstrip() + "\n"


def _group_by(records: list[DecisionRecord], attribute: str) -> dict[str, list[DecisionRecord]]:
    grouped: dict[str, list[DecisionRecord]] = defaultdict(list)
    for record in records:
        values = getattr(record, attribute)
        for value in values:
            grouped[value].append(record)
    return dict(grouped)


def render_index_files(records: list[DecisionRecord]) -> dict[Path, str]:
    by_number = sorted(records, key=lambda item: item.number)
    by_date = sorted(records, key=lambda item: (item.original_date, item.decision_id))
    generated_note = "<!-- Generated by scripts/generate_decision_indexes.py; do not edit by hand. -->\n"

    return {
        INDEX_DIR / "README.md": (
            "# Decision Lookup Indexes\n\n"
            f"{generated_note}\n"
            "These files are generated read models from decision-note `Index Metadata`.\n"
            "Decision notes own rationale; these indexes only make lookup cheaper for agents.\n\n"
            "## Indexes\n\n"
            "- [By number](by-number.md)\n"
            "- [By date](by-date.md)\n"
            "- [By surface class](by-surface.md)\n"
            "- [By ToS layer](by-tos-layer.md)\n"
            "- [By tree class](by-tree-class.md)\n"
            "- [By validation or guard family](by-guard.md)\n"
        ),
        INDEX_DIR / "by-number.md": (
            "# Decisions By Number\n\n"
            f"{generated_note}\n"
            + _table(
                by_number,
                columns=(
                    "Decision ID",
                    "Date",
                    "Decision",
                    "Path",
                    "Surface classes",
                    "ToS layers",
                    "Tree classes",
                    "Guard families",
                    "Posture",
                ),
            )
        ),
        INDEX_DIR / "by-date.md": (
            "# Decisions By Date\n\n"
            f"{generated_note}\n"
            + "".join(
                f"## {day}\n\n"
                + "".join(
                    _bullet_line(record) + "\n"
                    for record in by_date
                    if record.original_date == day
                )
                + "\n"
                for day in sorted({record.original_date for record in by_date})
            ).rstrip()
            + "\n"
        ),
        INDEX_DIR / "by-surface.md": _grouped_index(
            "Decisions By Surface Class",
            _group_by(records, "surface_classes"),
        ),
        INDEX_DIR / "by-tos-layer.md": _grouped_index(
            "Decisions By ToS Layer",
            _group_by(records, "tos_layers"),
        ),
        INDEX_DIR / "by-tree-class.md": _grouped_index(
            "Decisions By Tree Class",
            _group_by(records, "tree_classes"),
        ),
        INDEX_DIR / "by-guard.md": _grouped_index(
            "Decisions By Validation Or Guard Family",
            _group_by(records, "guard_families"),
        ),
    }


def validate_decision_indexes(repo_root: Path = REPO_ROOT) -> list[tuple[str, str]]:
    records, issues = collect_decision_records(repo_root)
    issues.extend(validate_decision_lane_surfaces(repo_root))
    issues.extend(validate_index_contract(repo_root))
    if issues:
        return issues
    rendered = render_index_files(records)
    for relative_path, expected_text in rendered.items():
        path = repo_root / relative_path
        rel = relative_path.as_posix()
        if not path.is_file():
            issues.append((rel, "generated index is missing"))
            continue
        if path.read_text(encoding="utf-8") != expected_text:
            issues.append((rel, "generated index is stale"))
    return issues


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if generated indexes are stale")
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT, help="repository root")
    args = parser.parse_args(argv)

    repo_root = args.repo_root.resolve()
    records, issues = collect_decision_records(repo_root)
    issues.extend(validate_decision_lane_surfaces(repo_root))
    issues.extend(validate_index_contract(repo_root))
    if issues:
        for location, message in issues:
            print(f"- {location}: {message}")
        return 1

    rendered = render_index_files(records)
    stale: list[str] = []
    for relative_path, expected_text in rendered.items():
        path = repo_root / relative_path
        if args.check:
            if not path.is_file() or path.read_text(encoding="utf-8") != expected_text:
                stale.append(relative_path.as_posix())
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(expected_text, encoding="utf-8")

    if stale:
        print("Stale decision indexes:")
        for path_name in stale:
            print(f"- {path_name}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

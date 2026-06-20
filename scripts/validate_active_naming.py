#!/usr/bin/env python3
from __future__ import annotations

import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
TEXT_SUFFIXES = {
    ".csv",
    ".json",
    ".md",
    ".py",
    ".txt",
    ".yaml",
    ".yml",
}
EXCLUDED_PARTS = {
    ".git",
    ".agents",
    ".pytest_cache",
    "__pycache__",
    "legacy",
}
EXCLUDED_FILES = {
    "CHANGELOG.md",
    "scripts/validate_active_naming.py",
}
RETIRED_TOKENS = (
    "w" + "ave",
    "w" + "aves",
    "s" + "eed",
    "s" + "eeds",
    "s" + "eeded",
    "s" + "eed-pack",
    "s" + "eed_pack",
)
RETIRED_TOKEN_PATTERN = r"(?:" + "|".join(re.escape(token) for token in RETIRED_TOKENS) + r")"
PATH_REFERENCE_MARKER_PATTERN = r"(?:[-_/]|\d|\.(?=[A-Za-z0-9]))"
PATH_TOKEN_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])(?:"
    + RETIRED_TOKEN_PATTERN
    + r")(?![A-Za-z0-9])",
    re.IGNORECASE,
)
ACTIVE_REFERENCE_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])"
    r"(?=[A-Za-z0-9._/-]*" + PATH_REFERENCE_MARKER_PATTERN + r")"
    r"[A-Za-z0-9._/-]*"
    + RETIRED_TOKEN_PATTERN
    + r"[A-Za-z0-9._/-]*"
    r"(?![A-Za-z0-9])",
    re.IGNORECASE,
)
OLD_ROUTE_PREFIX = "z" + "v"
RETIRED_ROUTE_LABEL_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])" + OLD_ROUTE_PREFIX + r"\d+(?:[-_][A-Za-z0-9]+)+",
    re.IGNORECASE,
)
RETIRED_VERSION_PASS_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])" + "v" + r"0\.[6-9](?:\.\d+)?(?![A-Za-z0-9])",
    re.IGNORECASE,
)
RETIRED_EXPERIENCE_VERSION_REF_PATTERN = re.compile(
    r"(?<![A-Za-z0-9])experience\." + "v" + r"0\.",
    re.IGNORECASE,
)
EXPERIENCE_ROUTE_PREFIX = "mechanics/experience/"
RETIRED_NORMALIZED_LABELS = (
    "deployment" + "-" + "watchtower",
    "federation" + "-" + "harvest",
    "adoption" + "-" + "forge",
    "constitution" + "-" + "runtime",
)
NORMALIZED_SEPARATOR_PATTERN = re.compile(r"[_\s]+")


def relative(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def is_excluded(path: Path) -> bool:
    rel = path.relative_to(REPO_ROOT)
    return rel.as_posix() in EXCLUDED_FILES or any(part in EXCLUDED_PARTS for part in rel.parts)


def normalize_label_surface(value: str) -> str:
    return NORMALIZED_SEPARATOR_PATTERN.sub("-", value).lower()


def retired_normalized_label(value: str) -> str | None:
    normalized = normalize_label_surface(value)
    for label in RETIRED_NORMALIZED_LABELS:
        if label in normalized:
            return label
    return None


def retired_path_issue(value: str) -> str | None:
    match = PATH_TOKEN_PATTERN.search(value)
    if match:
        return match.group(0)
    match = RETIRED_ROUTE_LABEL_PATTERN.search(value)
    if match:
        return match.group(0)
    marker = retired_normalized_label(value)
    if marker:
        return marker
    return None


def retired_content_issue(text: str) -> str | None:
    match = ACTIVE_REFERENCE_PATTERN.search(text)
    if match:
        return match.group(0)
    match = RETIRED_ROUTE_LABEL_PATTERN.search(text)
    if match:
        return match.group(0)
    match = RETIRED_EXPERIENCE_VERSION_REF_PATTERN.search(text)
    if match:
        return match.group(0)
    marker = retired_normalized_label(text)
    if marker:
        return marker
    return None


def retired_experience_pass_issue(value: str) -> str | None:
    match = RETIRED_VERSION_PASS_PATTERN.search(value)
    return match.group(0) if match else None


def validate() -> list[str]:
    issues: list[str] = []
    for path in sorted(REPO_ROOT.rglob("*")):
        if is_excluded(path):
            continue
        rel = relative(path)
        path_issue = retired_path_issue(rel)
        if path_issue:
            issues.append(f"{rel}: retired active name in path: {path_issue}")
        if rel.startswith(EXPERIENCE_ROUTE_PREFIX):
            path_issue = retired_experience_pass_issue(rel)
            if path_issue:
                issues.append(f"{rel}: retired experience pass marker in path: {path_issue}")
        if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        content_issue = retired_content_issue(text)
        if content_issue:
            issues.append(f"{rel}: retired active path/id reference in content: {content_issue}")
        if rel.startswith(EXPERIENCE_ROUTE_PREFIX):
            content_issue = retired_experience_pass_issue(text)
            if content_issue:
                issues.append(f"{rel}: retired experience pass marker in content: {content_issue}")
    return issues


def main() -> int:
    issues = validate()
    if issues:
        print("Active naming validation failed.", file=sys.stderr)
        for issue in issues:
            print(f"- {issue}", file=sys.stderr)
        return 1
    print("[ok] validated active naming")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

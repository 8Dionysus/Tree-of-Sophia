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


def relative(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def is_excluded(path: Path) -> bool:
    rel = path.relative_to(REPO_ROOT)
    return rel.as_posix() in EXCLUDED_FILES or any(part in EXCLUDED_PARTS for part in rel.parts)


def validate() -> list[str]:
    issues: list[str] = []
    for path in sorted(REPO_ROOT.rglob("*")):
        if is_excluded(path):
            continue
        rel = relative(path)
        if PATH_TOKEN_PATTERN.search(rel):
            issues.append(f"{rel}: retired active name in path")
        if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        match = ACTIVE_REFERENCE_PATTERN.search(text)
        if match:
            issues.append(f"{rel}: retired active path/id reference in content: {match.group(0)}")
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

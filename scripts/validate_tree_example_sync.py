#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

from tree_example_sync import REPO_ROOT, build_expected_example_payloads, encode_json

Issue = tuple[str, str]


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    for example_path, payload in build_expected_example_payloads(root):
        rel = example_path.relative_to(root).as_posix()
        try:
            actual_text = example_path.read_text(encoding="utf-8")
        except FileNotFoundError:
            issues.append((rel, "missing compatibility mirror"))
            continue

        expected_text = encode_json(payload)
        if actual_text != expected_text:
            issues.append(
                (
                    rel,
                    "out of sync with canonical tree; run python scripts/sync_tree_examples.py",
                )
            )

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Tree/example sync check failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print("[ok] validated tree/example compatibility mirrors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

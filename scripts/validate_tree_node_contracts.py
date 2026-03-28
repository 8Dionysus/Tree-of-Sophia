#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]

Issue = tuple[str, str]


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def node_paths(repo_root: Path | None = None) -> list[Path]:
    root = repo_root or REPO_ROOT
    return sorted((root / "tree").rglob("node.json"))


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    schema = load_json(root / "schemas" / "tos-node-contract.schema.json")
    if not isinstance(schema, dict):
        return [("schemas/tos-node-contract.schema.json", "schema root must be a JSON object")]

    validator = Draft202012Validator(schema)
    paths = node_paths(root)
    if not paths:
        issues.append(("tree/", "no canonical tree node.json files found"))
        return issues

    for path in paths:
        rel = path.relative_to(root).as_posix()
        try:
            payload = load_json(path)
        except json.JSONDecodeError as exc:
            issues.append((rel, f"invalid JSON: {exc}"))
            continue

        for error in validator.iter_errors(payload):
            issues.append((rel, error.message))

    return issues


def main() -> int:
    issues = run_validation(REPO_ROOT)
    if issues:
        print("Tree node contract validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    print(f"[ok] validated {len(node_paths(REPO_ROOT))} canonical tree node payloads against the node contract")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

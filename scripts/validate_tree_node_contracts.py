#!/usr/bin/env python3
from __future__ import annotations

import json
import math
import sys
from decimal import Decimal, InvalidOperation
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]

Issue = tuple[str, str]


def parse_node_json(data: str | bytes) -> object:
    """Parse one exact node/schema input without ambiguous fields or numbers."""
    def unique_fields(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f'duplicate JSON field {key!r}')
            result[key] = value
        return result

    def finite_number(value):
        raise ValueError(f'nonfinite JSON number {value}')

    def finite_float(value):
        number = float(value)
        if not math.isfinite(number):
            finite_number(value)
        try:
            unchanged = Decimal(value) == Decimal(repr(number))
        except InvalidOperation as exc:
            raise ValueError('node decimal exceeds its exact numeric representation') from exc
        if not unchanged:
            raise ValueError('node decimal loses precision in its numeric representation')
        return number

    text = data.decode('utf-8') if isinstance(data, bytes) else data
    return json.loads(text, object_pairs_hook=unique_fields,
                      parse_constant=finite_number, parse_float=finite_float)


def load_json(path: Path) -> object:
    return parse_node_json(path.read_text(encoding="utf-8"))


def node_paths(repo_root: Path | None = None) -> list[Path]:
    root = repo_root or REPO_ROOT
    return sorted((root / "ToS" / "canon").rglob("node.json"))


def duplicate_language_witness_issues(payload: object, *, location: str) -> list[Issue]:
    if not isinstance(payload, dict):
        return []
    witnesses = payload.get("language_witnesses")
    if not isinstance(witnesses, list):
        return []

    seen_languages: set[str] = set()
    issues: list[Issue] = []
    for witness in witnesses:
        if not isinstance(witness, dict):
            continue
        language = witness.get("language")
        if not isinstance(language, str):
            continue
        if language in seen_languages:
            issues.append((location, f"language_witnesses contains a duplicate language: {language}"))
            continue
        seen_languages.add(language)
    return issues


def node_consistency_issues(payload: object, *, location: str) -> list[Issue]:
    """Check relationships between fields of a schema-valid native node.

    Identities and segment spines come from the node itself. No corpus names,
    language list, segment count or expected wording belongs in this check.
    """
    if not isinstance(payload, dict):
        return []
    issues = duplicate_language_witness_issues(payload, location=location)
    identifier, kind = payload.get('node_id'), payload.get('node_type')
    if isinstance(identifier, str) and isinstance(kind, str) and not identifier.startswith(f'tos.{kind}.'):
        issues.append((location, 'node_id family does not match node_type'))
    if ('relations' in payload and 'lineage_relations' in payload
            and payload['relations'] != payload['lineage_relations']):
        issues.append((location, 'relations and its legacy lineage_relations alias disagree'))

    spine = None
    for witness in payload.get('language_witnesses', []):
        segments = [segment['segment_id'] for segment in witness['segments']]
        if len(set(segments)) != len(segments):
            issues.append((location, f"witness {witness['language']} repeats a segment_id"))
        if spine is None:
            spine = segments
        elif segments != spine:
            issues.append((location, f"witness {witness['language']} differs from the shared ordered segment spine"))
    segment_ids = set(spine or [])
    for tension in payload.get('translation_tensions', []):
        if tension['segment_id'] not in segment_ids:
            issues.append((location, f"translation tension references unknown segment_id {tension['segment_id']}"))
    return issues


def run_validation(repo_root: Path | None = None) -> list[Issue]:
    root = repo_root or REPO_ROOT
    issues: list[Issue] = []

    schema = load_json(root / "ToS" / "contracts" / "tos-node-contract.schema.json")
    if not isinstance(schema, dict):
        return [("ToS/contracts/tos-node-contract.schema.json", "schema root must be a JSON object")]

    validator = Draft202012Validator(schema)
    paths = node_paths(root)
    if not paths:
        issues.append(("ToS/canon/", "no canonical tree node.json files found"))
        return issues

    identities: dict[str, str] = {}
    for path in paths:
        rel = path.relative_to(root).as_posix()
        try:
            payload = load_json(path)
        except (ValueError, UnicodeError) as exc:
            issues.append((rel, f"invalid JSON: {exc}"))
            continue

        errors = list(validator.iter_errors(payload))
        for error in errors:
            issues.append((rel, error.message))
        if errors:
            issues.extend(duplicate_language_witness_issues(payload, location=rel))
            continue
        issues.extend(node_consistency_issues(payload, location=rel))
        identifier = payload['node_id']
        if identifier in identities:
            issues.append((rel, f"duplicate canonical node_id {identifier}; already owned by {identities[identifier]}"))
        else:
            identities[identifier] = rel

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

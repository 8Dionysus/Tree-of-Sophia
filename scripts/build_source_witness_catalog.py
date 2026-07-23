#!/usr/bin/env python3
"""Build the tracked source-witness catalog from authored object records."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
CATALOG_ROOT = SOURCE_ROOT / "catalog"
MANIFEST_PATH = CATALOG_ROOT / "catalog.manifest.json"

RECORD_FILES = {
    "agent": "agents.jsonl",
    "work": "works.jsonl",
    "expression": "expressions.jsonl",
    "edition": "editions.jsonl",
    "collection": "collections.jsonl",
    "item": "items.jsonl",
}
SOURCE_BASENAMES = {record_type: f"{record_type}.json" for record_type in RECORD_FILES}
LINK_FIELDS = (
    "work_ref",
    "responsibility_claim_refs",
    "embodies_expression_refs",
    "collection_ref",
    "membership_claim_refs",
    "item_manifest_ref",
)


class CatalogBuildError(RuntimeError):
    pass


def canonical_json(payload: object) -> str:
    return json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def collect_records(repo_root: Path = REPO_ROOT) -> dict[str, list[dict[str, Any]]]:
    source_root = repo_root / SOURCE_ROOT
    records: dict[str, list[dict[str, Any]]] = {record_type: [] for record_type in RECORD_FILES}
    seen_ids: dict[str, str] = {}

    for record_type, basename in SOURCE_BASENAMES.items():
        for path in sorted(source_root.rglob(basename)):
            if CATALOG_ROOT in path.relative_to(repo_root).parents:
                continue
            relative = path.relative_to(repo_root).as_posix()
            try:
                payload = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError) as exc:
                raise CatalogBuildError(f"{relative}: cannot read corpus record: {exc}") from exc
            if not isinstance(payload, dict):
                raise CatalogBuildError(f"{relative}: corpus record root must be an object")
            if payload.get("record_type") != record_type:
                raise CatalogBuildError(
                    f"{relative}: record_type must be {record_type!r} for {basename}"
                )
            record_id = payload.get("record_id")
            if not isinstance(record_id, str) or not record_id:
                raise CatalogBuildError(f"{relative}: missing record_id")
            if record_id in seen_ids:
                raise CatalogBuildError(
                    f"{relative}: duplicate record_id {record_id!r}; first seen at {seen_ids[record_id]}"
                )
            seen_ids[record_id] = relative

            digest = hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()
            links = {field: payload[field] for field in LINK_FIELDS if field in payload}
            records[record_type].append(
                {
                    "schema_version": "tos_source_witness_catalog_entry_v1",
                    "record_id": record_id,
                    "record_type": record_type,
                    "preferred_label": payload.get("preferred_label", ""),
                    "identity_status": payload.get("identity_status", ""),
                    "source_record_ref": relative,
                    "record_sha256": digest,
                    "links": links,
                }
            )

    for entries in records.values():
        entries.sort(key=lambda entry: entry["record_id"])
    return records


def render_outputs(repo_root: Path = REPO_ROOT) -> dict[Path, str]:
    records = collect_records(repo_root)
    outputs: dict[Path, str] = {}
    digest_parts: list[str] = []

    for record_type, filename in RECORD_FILES.items():
        lines = [canonical_json(entry) for entry in records[record_type]]
        text = "\n".join(lines) + ("\n" if lines else "")
        relative = CATALOG_ROOT / filename
        outputs[relative] = text
        digest_parts.append(f"{record_type}\0{text}")

    counts = {record_type: len(entries) for record_type, entries in records.items()}
    counts["total"] = sum(counts.values())
    manifest = {
        "schema_version": "tos_source_witness_catalog_v1",
        "owner_repo": "Tree-of-Sophia",
        "source_root": SOURCE_ROOT.as_posix(),
        "generated_by": "scripts/build_source_witness_catalog.py",
        "record_schema_ref": "ToS/contracts/corpus-record.schema.json",
        "record_files": {
            record_type: (CATALOG_ROOT / filename).as_posix()
            for record_type, filename in RECORD_FILES.items()
        },
        "counts": counts,
        "catalog_sha256": hashlib.sha256("".join(digest_parts).encode("utf-8")).hexdigest(),
        "authority_boundary": "generated navigation over tracked object records; not bibliographic, textual, rights, or semantic authority",
    }
    outputs[MANIFEST_PATH] = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    return outputs


def write_outputs(repo_root: Path, outputs: dict[Path, str]) -> None:
    for relative, expected in outputs.items():
        path = repo_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(expected, encoding="utf-8")


def check_outputs(repo_root: Path, outputs: dict[Path, str]) -> list[str]:
    issues: list[str] = []
    for relative, expected in outputs.items():
        path = repo_root / relative
        try:
            actual = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            issues.append(f"{relative.as_posix()}: generated catalog file is missing")
            continue
        if actual != expected:
            issues.append(f"{relative.as_posix()}: generated catalog is stale")
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check generated parity without writing")
    args = parser.parse_args()

    try:
        outputs = render_outputs(REPO_ROOT)
    except CatalogBuildError as exc:
        print(f"Source-witness catalog build failed: {exc}", file=sys.stderr)
        return 1

    if args.check:
        issues = check_outputs(REPO_ROOT, outputs)
        if issues:
            print("Source-witness catalog parity failed.", file=sys.stderr)
            for issue in issues:
                print(f"- {issue}", file=sys.stderr)
            return 1
        print("[ok] source-witness catalog matches authored object records")
        return 0

    write_outputs(REPO_ROOT, outputs)
    print("[ok] generated source-witness catalog")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Build the tracked source-witness catalog from authored objects and claims."""

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
CLAIM_CATALOG_PATH = CATALOG_ROOT / "claims.jsonl"

RECORD_FILES = {
    "agent": "agents.jsonl",
    "work": "works.jsonl",
    "expression": "expressions.jsonl",
    "edition": "editions.jsonl",
    "collection": "collections.jsonl",
    "item": "items.jsonl",
}
SOURCE_BASENAMES = {record_type: f"{record_type}.json" for record_type in RECORD_FILES}
CLAIM_SOURCE_BASENAMES = (
    "membership-claims.jsonl",
    "responsibility-claims.jsonl",
    "publication-claims.jsonl",
    "work-expression-claims.jsonl",
    "expression-edition-claims.jsonl",
    "edition-item-claims.jsonl",
)
TRACKED_CLAIM_VISIBILITIES = {"public_metadata_only", "public"}
LINK_FIELDS = (
    "work_ref",
    "expression_claim_refs",
    "responsibility_claim_refs",
    "embodiment_claim_refs",
    "embodies_expression_refs",
    "exemplar_claim_refs",
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


def collect_claims(repo_root: Path = REPO_ROOT) -> list[dict[str, Any]]:
    source_root = repo_root / SOURCE_ROOT
    claims: list[dict[str, Any]] = []
    seen_ids: dict[str, str] = {}

    for basename in CLAIM_SOURCE_BASENAMES:
        for path in sorted(source_root.rglob(basename)):
            relative = path.relative_to(repo_root).as_posix()
            try:
                lines = path.read_text(encoding="utf-8").splitlines()
            except OSError as exc:
                raise CatalogBuildError(
                    f"{relative}: cannot read claim packets: {exc}"
                ) from exc
            for line_number, raw_line in enumerate(lines, start=1):
                if not raw_line.strip():
                    continue
                location = f"{relative}:{line_number}"
                try:
                    payload = json.loads(raw_line)
                except json.JSONDecodeError as exc:
                    raise CatalogBuildError(
                        f"{location}: cannot parse claim packet: {exc}"
                    ) from exc
                if not isinstance(payload, dict):
                    raise CatalogBuildError(
                        f"{location}: claim packet root must be an object"
                    )
                claim_id = payload.get("claim_id")
                if not isinstance(claim_id, str) or not claim_id:
                    raise CatalogBuildError(f"{location}: missing claim_id")
                if claim_id in seen_ids:
                    raise CatalogBuildError(
                        f"{location}: duplicate claim_id {claim_id!r}; "
                        f"first seen at {seen_ids[claim_id]}"
                    )
                seen_ids[claim_id] = location
                visibility = payload.get("visibility")
                if visibility not in TRACKED_CLAIM_VISIBILITIES:
                    raise CatalogBuildError(
                        f"{location}: visibility {visibility!r} is not safe "
                        "for the tracked claim catalog"
                    )

                digest = hashlib.sha256(
                    canonical_json(payload).encode("utf-8")
                ).hexdigest()
                entry = {
                    "schema_version": "tos_source_witness_claim_catalog_entry_v1",
                    "claim_id": claim_id,
                    "claim_type": payload.get("claim_type"),
                    "assertion_layer": payload.get("assertion_layer"),
                    "subject_ref": payload.get("subject_ref"),
                    "predicate": payload.get("predicate"),
                    "object": payload.get("object"),
                    "evidence_refs": payload.get("evidence_refs"),
                    "maker": payload.get("maker"),
                    "provenance_event_ref": payload.get("provenance_event_ref"),
                    "epistemic_status": payload.get("epistemic_status"),
                    "review_status": payload.get("review_status"),
                    "visibility": visibility,
                    "review_refs": [
                        review.get("review_id")
                        for review in payload.get("reviews", [])
                        if isinstance(review, dict)
                        and isinstance(review.get("review_id"), str)
                    ],
                    "claim_version": payload.get("claim_version"),
                    "source_claim_file_ref": relative,
                    "source_claim_line": line_number,
                    "claim_sha256": digest,
                }
                if "supersedes_claim_ref" in payload:
                    entry["supersedes_claim_ref"] = payload[
                        "supersedes_claim_ref"
                    ]
                claims.append(entry)

    claims.sort(key=lambda entry: entry["claim_id"])
    return claims


def render_outputs(repo_root: Path = REPO_ROOT) -> dict[Path, str]:
    records = collect_records(repo_root)
    claims = collect_claims(repo_root)
    outputs: dict[Path, str] = {}
    digest_parts: list[str] = []

    for record_type, filename in RECORD_FILES.items():
        lines = [canonical_json(entry) for entry in records[record_type]]
        text = "\n".join(lines) + ("\n" if lines else "")
        relative = CATALOG_ROOT / filename
        outputs[relative] = text
        digest_parts.append(f"{record_type}\0{text}")

    claim_text = "\n".join(canonical_json(entry) for entry in claims)
    claim_text += "\n" if claim_text else ""
    outputs[CLAIM_CATALOG_PATH] = claim_text
    digest_parts.append(f"claim\0{claim_text}")

    counts = {record_type: len(entries) for record_type, entries in records.items()}
    counts["object_total"] = sum(counts.values())
    counts["claim"] = len(claims)
    counts["total"] = counts["object_total"] + counts["claim"]
    manifest = {
        "schema_version": "tos_source_witness_catalog_v2",
        "owner_repo": "Tree-of-Sophia",
        "source_root": SOURCE_ROOT.as_posix(),
        "generated_by": "scripts/build_source_witness_catalog.py",
        "record_schema_ref": "ToS/contracts/corpus-record.schema.json",
        "claim_schema_ref": "ToS/contracts/claim-packet.schema.json",
        "record_files": {
            record_type: (CATALOG_ROOT / filename).as_posix()
            for record_type, filename in RECORD_FILES.items()
        },
        "claim_file": CLAIM_CATALOG_PATH.as_posix(),
        "counts": counts,
        "catalog_sha256": hashlib.sha256("".join(digest_parts).encode("utf-8")).hexdigest(),
        "authority_boundary": (
            "generated navigation over tracked object and claim records; not "
            "bibliographic, textual, rights, review, or semantic authority"
        ),
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
        print("[ok] source-witness catalog matches authored object and claim records")
        return 0

    write_outputs(REPO_ROOT, outputs)
    print("[ok] generated source-witness object and claim catalog")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

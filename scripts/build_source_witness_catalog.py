#!/usr/bin/env python3
"""Build the tracked source-witness catalog from authored objects and claims."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker
from source_record_profiles import SourceRecordProfiles, SourceProfileError, METADATA_LINK_FIELDS


REPO_ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOT = Path("ToS/source-witnesses")
CATALOG_ROOT = SOURCE_ROOT / "catalog"
MANIFEST_PATH = CATALOG_ROOT / "catalog.manifest.json"
CLAIM_CATALOG_PATH = CATALOG_ROOT / "claims.jsonl"

RECORD_FILES = {
    "agent": "agents.jsonl",
    "place": "places.jsonl",
    "organization": "organizations.jsonl",
    "work": "works.jsonl",
    "expression": "expressions.jsonl",
    "edition": "editions.jsonl",
    "collection": "collections.jsonl",
    "item": "items.jsonl",
    "link": "links.jsonl",
}
OPTIONAL_RECORD_FILES = {
    "historical-event": "historical-events.jsonl",
    "historical-process": "historical-processes.jsonl",
    "historical-state": "historical-states.jsonl",
}
ADAPTED_RECORD_FILES = {"artifact": "artifacts.jsonl"}
ARTIFACT_SCHEMAS = {
    'tos_artifact_source_witness_v1': 'ToS/contracts/artifact-source-witness.schema.json',
    'tos_artifact_source_witness_v2': 'ToS/contracts/artifact-source-witness-v2.schema.json',
}
SOURCE_BASENAMES = {
    record_type: f"{record_type}.json"
    for record_type in (*RECORD_FILES, *OPTIONAL_RECORD_FILES)
}
CLAIM_SOURCE_BASENAMES = (
    "membership-claims.jsonl",
    "responsibility-claims.jsonl",
    "publication-claims.jsonl",
    "provision-activity-claims.jsonl",
    "work-chronology-claims.jsonl",
    "work-expression-claims.jsonl",
    "expression-edition-claims.jsonl",
    "edition-item-claims.jsonl",
    "expression-derivation-claims.jsonl",
    "object-link-claims.jsonl",
    "historical-claims.jsonl",
)
TRACKED_CLAIM_VISIBILITIES = {"public_metadata_only", "public"}
LINK_FIELDS = METADATA_LINK_FIELDS


class CatalogBuildError(RuntimeError):
    pass


def canonical_json(payload: object) -> str:
    return json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def artifact_catalog_entry(repo_root: Path, payload: dict, relative: str,
                           validators: dict | None = None) -> dict:
    """Project native physical identity; never manufacture a Corpus record.

    Inventory wording is an attributed navigation label, not an assessed title.
    A missing identity assessment remains null rather than inferred from the
    legacy metadata review state. All native fields stay in the source record.
    """
    schema_ref = ARTIFACT_SCHEMAS.get(payload.get('schema_version'))
    if schema_ref is None:
        raise CatalogBuildError(f'{relative}: unsupported physical artifact source schema')
    validators = {} if validators is None else validators
    if schema_ref not in validators:
        schema = json.loads((repo_root / schema_ref).read_text(encoding='utf-8'))
        validators[schema_ref] = Draft202012Validator(schema, format_checker=FormatChecker())
    if not validators[schema_ref].is_valid(payload):
        raise CatalogBuildError(f'{relative}: invalid or nonpublic physical artifact metadata')
    return {
        'schema_version': 'tos_source_witness_catalog_entry_v1',
        'source_schema_ref': schema_ref,
        'record_id': payload['artifact_id'],
        'record_type': 'artifact',
        'preferred_label': payload['custody']['inventory_numbers'][0],
        'label_source_pointer': '/custody/inventory_numbers/0',
        'identity_status': None,
        'source_record_ref': relative,
        'record_sha256': hashlib.sha256(canonical_json(payload).encode('utf-8')).hexdigest(),
        'links': {},
    }


def load_artifact_record(repo_root: Path, relative: str) -> dict:
    """Bound a public metadata read to the physical-artifact owner subtree."""
    ref = Path(relative)
    if (ref.is_absolute() or '..' in ref.parts or ref.name != 'artifact-witness.json'
            or not ref.is_relative_to(SOURCE_ROOT / 'artifacts')):
        raise CatalogBuildError('physical artifact source path is outside its owner subtree')
    path = repo_root / ref
    if path.is_symlink() or not path.is_file() or path.resolve() != path.absolute():
        raise CatalogBuildError(f'{relative}: artifact source must be a regular non-symlink path')
    with path.open('rb') as handle:
        raw = handle.read(1_048_577)
    if len(raw) > 1_048_576:
        raise CatalogBuildError(f'{relative}: artifact metadata exceeds 1 MiB budget')
    try:
        payload = json.loads(raw)
    except (ValueError, UnicodeError) as exc:
        raise CatalogBuildError(f'{relative}: invalid physical artifact JSON') from exc
    if not isinstance(payload, dict):
        raise CatalogBuildError(f'{relative}: physical artifact record must be an object')
    return payload


def artifact_display_fields(payload: dict) -> dict:
    """Exact field copies shared by the two readers of a validated artifact."""
    return {
        'description': payload['path_identity']['note'],
        'review_status': payload['authority']['review_status'],
        'visibility': payload['authority']['visibility'],
        'label_source_pointer': '/custody/inventory_numbers/0',
        'metadata_field_sources': {
            'preferred_label': '/custody/inventory_numbers/0',
            'description': '/path_identity/note',
            'review_status': '/authority/review_status',
            'visibility': '/authority/visibility',
        },
    }


def collect_records(repo_root: Path = REPO_ROOT, *, profiles: SourceRecordProfiles | None = None) -> dict[str, list[dict[str, Any]]]:
    try:
        return _collect_records(repo_root, profiles=profiles)
    except SourceProfileError as exc:
        raise CatalogBuildError(str(exc)) from exc


def _collect_records(repo_root: Path, *, profiles: SourceRecordProfiles | None) -> dict[str, list[dict[str, Any]]]:
    source_root = repo_root / SOURCE_ROOT
    profiles = profiles or SourceRecordProfiles(repo_root)
    basenames = {**{kind: kind + '.json' for kind in RECORD_FILES}, **profiles.source_basenames}
    records: dict[str, list[dict[str, Any]]] = {record_type: [] for record_type in basenames}
    seen_ids: dict[str, str] = {}

    for record_type, basename in basenames.items():
        for path in sorted(source_root.rglob(basename)):
            if CATALOG_ROOT in path.relative_to(repo_root).parents:
                continue
            relative = path.relative_to(repo_root).as_posix()
            try:
                payload = (profiles.load(record_type, relative) if record_type in profiles.profiles
                           else json.loads(path.read_text(encoding="utf-8")))
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
                profiles.catalog_entry(record_type, payload, relative) if record_type in profiles.profiles else
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

    artifacts, validators = [], {}
    for path in sorted((source_root / 'artifacts').rglob('artifact-witness.json')):
        relative = path.relative_to(repo_root).as_posix()
        payload = load_artifact_record(repo_root, relative)
        entry = artifact_catalog_entry(repo_root, payload, relative, validators)
        if entry['record_id'] in seen_ids:
            raise CatalogBuildError(f"{relative}: duplicate record_id {entry['record_id']!r}")
        seen_ids[entry['record_id']] = relative
        artifacts.append(entry)
    if artifacts:
        records['artifact'] = artifacts
    for entries in records.values():
        entries.sort(key=lambda entry: entry["record_id"])
    return {kind: entries for kind, entries in records.items()
            if kind in RECORD_FILES or entries}


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
                    **({'source_schema_ref': 'ToS/contracts/historical-claim.schema.json'}
                       if payload.get('schema_version') == 'tos_historical_claim_v1' else {}),
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
                if "qualifiers" in payload:
                    entry["qualifiers"] = payload["qualifiers"]
                claims.append(entry)

    claims.sort(key=lambda entry: entry["claim_id"])
    return claims


def render_outputs(repo_root: Path = REPO_ROOT) -> dict[Path, str]:
    profiles = SourceRecordProfiles(repo_root)
    records = collect_records(repo_root, profiles=profiles)
    claims = collect_claims(repo_root)
    outputs: dict[Path, str] = {}
    digest_parts: list[str] = []

    profile_files = profiles.catalog_files
    record_files = {**RECORD_FILES, **{kind: filename for kind, filename in
                                    {**profile_files, **ADAPTED_RECORD_FILES}.items()
                                    if kind in records}}
    for record_type, filename in record_files.items():
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
        "schema_version": "tos_source_witness_catalog_v3",
        "owner_repo": "Tree-of-Sophia",
        "source_root": SOURCE_ROOT.as_posix(),
        "generated_by": "scripts/build_source_witness_catalog.py",
        "record_schema_ref": "ToS/contracts/corpus-record.schema.json",
        "claim_schema_ref": "ToS/contracts/claim-packet.schema.json",
        **({'extension_schema_refs': sorted({entry['source_schema_ref']
                                            for entries in [*records.values(), claims] for entry in entries
                                            if 'source_schema_ref' in entry})}
           if any(kind in records for kind in (*profile_files, *ADAPTED_RECORD_FILES))
           or any('source_schema_ref' in entry for entry in claims) else {}),
        "record_files": {
            record_type: (CATALOG_ROOT / filename).as_posix()
            for record_type, filename in record_files.items()
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
    except (CatalogBuildError, SourceProfileError) as exc:
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

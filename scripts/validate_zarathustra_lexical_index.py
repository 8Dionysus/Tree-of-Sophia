#!/usr/bin/env python3
"""Validate the tracked Zarathustra lexical projection without accepting text."""

from __future__ import annotations

import argparse
import hashlib
import json
import sqlite3
from collections import Counter
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
PLAN_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/index-plan.v1.json"
)
PROJECTION_REF = (
    "ToS/derived-exports/lexical-search/"
    "zarathustra-dta-first-editions-parts-1-4-v1.min.json"
)
GENERATOR_REF = "scripts/build_zarathustra_lexical_index.py"
BASE_PROVENANCE_EVENT_REF = (
    "tos.event.export.zarathustra-lexical-index-v1.2026-07-29"
)
AUTHORITY_BOUNDARY = (
    "mechanical source-observation and rebuildable local search only; no "
    "accepted German, rights clearance, lexeme, lemma, translation, sign, "
    "concept, claim, relation, graph, canon, or publication authority"
)


class LexicalIndexValidationError(RuntimeError):
    """Raised when the tracked projection loses source or authority closure."""


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise LexicalIndexValidationError(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise LexicalIndexValidationError(f"{path} must contain a JSON object")
    return payload


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as exc:
        raise LexicalIndexValidationError(f"cannot hash {path}: {exc}") from exc
    return digest.hexdigest()


def _validate_schema(payload: object, schema_path: Path, *, label: str) -> None:
    schema = _load_json(schema_path)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{'/'.join(map(str, error.absolute_path)) or '<root>'}: {error.message}"
            for error in errors[:8]
        )
        raise LexicalIndexValidationError(f"{label} schema failed: {details}")


def _load_provenance(path: Path) -> list[dict[str, Any]]:
    try:
        lines = [
            line
            for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
    except OSError as exc:
        raise LexicalIndexValidationError(
            f"cannot read provenance {path}: {exc}"
        ) from exc
    if not lines:
        raise LexicalIndexValidationError(
            "lexical provenance must contain at least one event"
        )
    events = []
    for line_number, line in enumerate(lines, start=1):
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            raise LexicalIndexValidationError(
                f"invalid provenance JSON at line {line_number}: {exc}"
            ) from exc
        if not isinstance(payload, dict):
            raise LexicalIndexValidationError(
                f"provenance row {line_number} must be an object"
            )
        events.append(payload)
    return events


def _latest_provenance_event(events: list[dict[str, Any]]) -> dict[str, Any]:
    events_by_id: dict[str, dict[str, Any]] = {}
    for event in events:
        event_id = event.get("event_id")
        if not isinstance(event_id, str) or not event_id:
            raise LexicalIndexValidationError("provenance event_id is absent")
        if event_id in events_by_id:
            raise LexicalIndexValidationError(
                f"duplicate provenance event_id: {event_id}"
            )
        events_by_id[event_id] = event
    current = events_by_id.get(BASE_PROVENANCE_EVENT_REF)
    if current is None:
        raise LexicalIndexValidationError(
            "lexical base provenance event is absent"
        )
    seen = {BASE_PROVENANCE_EVENT_REF}
    while True:
        current_id = current["event_id"]
        successors = [
            event
            for event in events
            if event.get("supersedes_event_ref") == current_id
        ]
        if not successors:
            if seen != set(events_by_id):
                raise LexicalIndexValidationError(
                    "lexical provenance contains an orphan event"
                )
            return current
        if len(successors) != 1:
            raise LexicalIndexValidationError(
                f"ambiguous provenance supersession from {current_id}"
            )
        current = successors[0]
        next_id = current["event_id"]
        if next_id in seen:
            raise LexicalIndexValidationError(
                "cyclic lexical provenance supersession lineage"
            )
        seen.add(next_id)


def _resource_maps(inventory: dict[str, Any], file_id: str) -> tuple[set[str], set[str]]:
    files = [
        entry
        for entry in inventory.get("files", [])
        if isinstance(entry, dict) and entry.get("file_id") == file_id
    ]
    if len(files) != 1:
        raise LexicalIndexValidationError(
            f"inventory does not resolve one file: {file_id}"
        )
    pages = {
        resource["resource_id"]
        for resource in files[0].get("resources", [])
        if isinstance(resource, dict)
        and resource.get("resource_kind") == "tei_page_break"
        and isinstance(resource.get("resource_id"), str)
    }
    sections = {
        resource["resource_id"]
        for resource in files[0].get("resources", [])
        if isinstance(resource, dict)
        and resource.get("resource_kind") == "tei_division"
        and isinstance(resource.get("resource_id"), str)
    }
    if not pages or not sections:
        raise LexicalIndexValidationError("inventory lacks TEI page/division resources")
    return pages, sections


def _validate_local_database(
    local_root: Path,
    projection: dict[str, Any],
) -> None:
    receipt = projection["local_projection_receipt"]
    database_path = (local_root / receipt["relative_path"]).resolve()
    try:
        database_path.relative_to(local_root.resolve())
    except ValueError as exc:
        raise LexicalIndexValidationError(
            "local database path escapes explicit root"
        ) from exc
    if "local-content" not in database_path.parts:
        raise LexicalIndexValidationError("local database is outside local-content")
    if not database_path.is_file():
        raise LexicalIndexValidationError(
            f"local lexical database is absent: {database_path}"
        )
    if database_path.stat().st_size != receipt["database_bytes"]:
        raise LexicalIndexValidationError("local database byte-size drift")
    if _sha256_file(database_path) != receipt["database_sha256"]:
        raise LexicalIndexValidationError("local database digest drift")
    try:
        connection = sqlite3.connect(f"file:{database_path}?mode=ro", uri=True)
        try:
            metadata = dict(connection.execute("SELECT key, value FROM metadata"))
            if metadata.get("plan_id") != projection["plan_id"]:
                raise LexicalIndexValidationError("local database plan identity drift")
            if metadata.get("plan_sha256") != projection["plan_sha256"]:
                raise LexicalIndexValidationError("local database plan digest drift")
            for table, expected in receipt["table_counts"].items():
                actual = connection.execute(
                    f"SELECT count(*) FROM {table}"
                ).fetchone()[0]
                if actual != expected:
                    raise LexicalIndexValidationError(
                        f"local database {table} count drift: {actual} != {expected}"
                    )
        finally:
            connection.close()
    except sqlite3.Error as exc:
        raise LexicalIndexValidationError(
            f"cannot inspect local lexical database: {exc}"
        ) from exc


def validate(*, local_output_root: Path | None = None) -> dict[str, Any]:
    plan_path = REPO_ROOT / PLAN_REF
    projection_path = REPO_ROOT / PROJECTION_REF
    plan = _load_json(plan_path)
    projection = _load_json(projection_path)
    _validate_schema(
        plan,
        REPO_ROOT / "ToS/contracts/lexical-index-plan.schema.json",
        label="lexical plan",
    )
    _validate_schema(
        projection,
        REPO_ROOT / "ToS/contracts/lexical-index-projection.schema.json",
        label="lexical projection",
    )
    if projection.get("generated_or_authored") != "generated_from_source":
        raise LexicalIndexValidationError(
            "lexical projection must declare generated_from_source"
        )
    if plan["authority_boundary"] != AUTHORITY_BOUNDARY:
        raise LexicalIndexValidationError("plan authority boundary drift")
    if projection["authority_boundary"] != AUTHORITY_BOUNDARY:
        raise LexicalIndexValidationError("projection authority boundary drift")
    if projection["plan_id"] != plan["plan_id"]:
        raise LexicalIndexValidationError("plan identity drift")
    if projection["plan_ref"] != PLAN_REF:
        raise LexicalIndexValidationError("plan ref drift")
    if projection["plan_sha256"] != _sha256_file(plan_path):
        raise LexicalIndexValidationError("plan digest drift")
    if projection["generator_ref"] != GENERATOR_REF:
        raise LexicalIndexValidationError("generator ref drift")
    if projection["builder"]["surface"] != GENERATOR_REF:
        raise LexicalIndexValidationError("builder surface drift")
    if projection["generator_sha256"] != _sha256_file(REPO_ROOT / GENERATOR_REF):
        raise LexicalIndexValidationError("generator digest drift")
    if plan["tracked_projection"]["relative_path"] != PROJECTION_REF:
        raise LexicalIndexValidationError("tracked projection route drift")
    if (
        projection["local_projection_receipt"]["relative_path"]
        != plan["local_projection"]["relative_path"]
    ):
        raise LexicalIndexValidationError("local projection route drift")
    if projection["field_posture"] != plan["field_posture"]:
        raise LexicalIndexValidationError("field posture drift")
    if projection["rights_and_visibility"] != plan["rights_and_visibility"]:
        raise LexicalIndexValidationError("rights/visibility posture drift")
    if projection["semantic_boundary"] != plan["semantic_boundary"]:
        raise LexicalIndexValidationError("semantic boundary drift")

    plan_items = {
        item["item_ref"]: item
        for item in plan["source_items"]
    }
    receipts = {
        item["item_ref"]: item
        for item in projection["source_items"]
    }
    if set(plan_items) != set(receipts):
        raise LexicalIndexValidationError("projection source-item closure drift")
    if [item["part_order"] for item in projection["source_items"]] != list(
        range(1, len(receipts) + 1)
    ):
        raise LexicalIndexValidationError("projection part order is not contiguous")

    item_resources: dict[str, tuple[set[str], set[str]]] = {}
    for item_ref, plan_item in plan_items.items():
        receipt = receipts[item_ref]
        for field in (
            "part_order",
            "file_id",
            "file_sha256",
            "manifest_ref",
            "resource_inventory_ref",
            "rights_ref",
            "language",
        ):
            if receipt[field] != plan_item[field]:
                raise LexicalIndexValidationError(
                    f"{item_ref} receipt drift in {field}"
                )
        manifest = _load_json(REPO_ROOT / plan_item["manifest_ref"])
        inventory = _load_json(REPO_ROOT / plan_item["resource_inventory_ref"])
        rights = _load_json(REPO_ROOT / plan_item["rights_ref"])
        if manifest.get("item_id") != item_ref:
            raise LexicalIndexValidationError(f"{item_ref} manifest identity drift")
        matching_files = [
            entry
            for entry in manifest.get("payload_files", [])
            if isinstance(entry, dict)
            and entry.get("file_id") == plan_item["file_id"]
            and entry.get("sha256") == plan_item["file_sha256"]
        ]
        if len(matching_files) != 1:
            raise LexicalIndexValidationError(
                f"{item_ref} exact file does not close through manifest"
            )
        if receipt["edition_ref"] != manifest.get("embodiment_ref"):
            raise LexicalIndexValidationError(f"{item_ref} edition ref drift")
        if receipt["rights_assessment_status"] != rights.get("assessment_status"):
            raise LexicalIndexValidationError(
                f"{item_ref} rights assessment drift"
            )
        if receipt["rights_review_status"] != rights.get("review_status"):
            raise LexicalIndexValidationError(f"{item_ref} rights review drift")
        pages, sections = _resource_maps(inventory, plan_item["file_id"])
        if receipt["body_page_count"] > len(pages):
            raise LexicalIndexValidationError(
                f"{item_ref} body page count exceeds inventory"
            )
        if receipt["section_count"] > len(sections):
            raise LexicalIndexValidationError(
                f"{item_ref} section count exceeds inventory"
            )
        item_resources[item_ref] = (pages, sections)

    form_rows = projection["form_rows"]
    if len(form_rows) != projection["summary"]["exact_form_row_count"]:
        raise LexicalIndexValidationError("exact form row count drift")
    form_keys = [row["form_key"] for row in form_rows]
    exact_hashes = [row["exact_form_sha256"] for row in form_rows]
    if len(form_keys) != len(set(form_keys)):
        raise LexicalIndexValidationError("duplicate tracked form key")
    if len(exact_hashes) != len(set(exact_hashes)):
        raise LexicalIndexValidationError("duplicate tracked exact-form digest")
    if any(
        row["form_key"] != f"lexical-form:sha256:{row['exact_form_sha256']}"
        for row in form_rows
    ):
        raise LexicalIndexValidationError("form key does not bind exact digest")
    if exact_hashes != sorted(exact_hashes):
        raise LexicalIndexValidationError("tracked form rows are not deterministic")
    if (
        len({row["normalized_form_sha256"] for row in form_rows})
        != projection["summary"]["normalized_form_hash_count"]
    ):
        raise LexicalIndexValidationError("normalized form hash count drift")

    token_total = 0
    editorial_total = 0
    unsectioned_total = 0
    per_item_totals: Counter[str] = Counter()
    for row in form_rows:
        if row["source_editorial_occurrence_count"] > row["occurrence_count"]:
            raise LexicalIndexValidationError("editorial count exceeds form count")
        if row["unsectioned_occurrence_count"] > row["occurrence_count"]:
            raise LexicalIndexValidationError("unsectioned count exceeds form count")
        row_total = 0
        row_section_total = 0
        seen_items: set[str] = set()
        for item_hit in row["source_items"]:
            item_ref = item_hit["item_ref"]
            if item_ref in seen_items or item_ref not in plan_items:
                raise LexicalIndexValidationError(
                    "form row has duplicate or unknown source item"
                )
            seen_items.add(item_ref)
            pages, sections = item_resources[item_ref]
            page_total = 0
            seen_pages: set[str] = set()
            for hit in item_hit["page_hits"]:
                if hit["resource_id"] in seen_pages or hit["resource_id"] not in pages:
                    raise LexicalIndexValidationError(
                        f"{item_ref} form row has invalid page resource"
                    )
                seen_pages.add(hit["resource_id"])
                page_total += hit["occurrence_count"]
            if page_total != item_hit["occurrence_count"]:
                raise LexicalIndexValidationError(
                    f"{item_ref} page-hit counts do not close"
                )
            section_total = 0
            seen_sections: set[str] = set()
            for hit in item_hit["section_hits"]:
                if (
                    hit["resource_id"] in seen_sections
                    or hit["resource_id"] not in sections
                ):
                    raise LexicalIndexValidationError(
                        f"{item_ref} form row has invalid section resource"
                    )
                seen_sections.add(hit["resource_id"])
                section_total += hit["occurrence_count"]
            if section_total > item_hit["occurrence_count"]:
                raise LexicalIndexValidationError(
                    f"{item_ref} section-hit counts exceed occurrences"
                )
            row_total += item_hit["occurrence_count"]
            row_section_total += section_total
            per_item_totals[item_ref] += item_hit["occurrence_count"]
        if row_total != row["occurrence_count"]:
            raise LexicalIndexValidationError("form source-item counts do not close")
        if row_total - row_section_total != row["unsectioned_occurrence_count"]:
            raise LexicalIndexValidationError(
                "form unsectioned count does not close"
            )
        token_total += row_total
        editorial_total += row["source_editorial_occurrence_count"]
        unsectioned_total += row["unsectioned_occurrence_count"]

    summary = projection["summary"]
    if token_total != summary["token_occurrence_count"]:
        raise LexicalIndexValidationError("projection token total drift")
    if editorial_total != summary["source_editorial_occurrence_count"]:
        raise LexicalIndexValidationError("projection editorial total drift")
    if unsectioned_total != summary["unsectioned_occurrence_count"]:
        raise LexicalIndexValidationError("projection unsectioned total drift")
    if summary["source_item_count"] != len(plan_items):
        raise LexicalIndexValidationError("source item summary drift")
    if summary["body_page_count"] != sum(
        receipt["body_page_count"] for receipt in receipts.values()
    ):
        raise LexicalIndexValidationError("body page summary drift")
    if summary["section_count"] != sum(
        receipt["section_count"] for receipt in receipts.values()
    ):
        raise LexicalIndexValidationError("section summary drift")
    for item_ref, receipt in receipts.items():
        if per_item_totals[item_ref] != receipt["token_occurrence_count"]:
            raise LexicalIndexValidationError(f"{item_ref} token total drift")

    local_counts = projection["local_projection_receipt"]["table_counts"]
    expected_local_counts = {
        "source_items": summary["source_item_count"],
        "pages": summary["body_page_count"],
        "sections": summary["section_count"],
        "occurrences": summary["token_occurrence_count"],
        "forms": summary["exact_form_row_count"],
    }
    if local_counts != expected_local_counts:
        raise LexicalIndexValidationError("local projection receipt count drift")
    if any(
        projection["local_projection_receipt"]["query_probes"][field]["status"]
        != "passed"
        for field in (
            "exact_form",
            "normalized_form",
            "prefix",
            "phrase",
            "section",
            "page",
            "language",
            "edition",
        )
    ):
        raise LexicalIndexValidationError("materialized query probe did not pass")
    if (
        projection["local_projection_receipt"]["query_probes"]["lemma"]["status"]
        != "blocked-not-materialized"
        or projection["local_projection_receipt"]["query_probes"]["sign_candidate"][
            "status"
        ]
        != "blocked-not-materialized"
    ):
        raise LexicalIndexValidationError("linguistic/semantic blockers drift")

    provenance_path = REPO_ROOT / plan["provenance_ref"]
    provenance_events = _load_provenance(provenance_path)
    for event_number, event in enumerate(provenance_events, start=1):
        _validate_schema(
            event,
            REPO_ROOT / "ToS/contracts/provenance-event.schema.json",
            label=f"lexical provenance event {event_number}",
        )
    provenance = _latest_provenance_event(provenance_events)
    if provenance.get("event_type") != "export":
        raise LexicalIndexValidationError("lexical provenance is not an export event")
    if provenance.get("method", {}).get("artifact_digest") != projection[
        "generator_sha256"
    ]:
        raise LexicalIndexValidationError("provenance generator digest drift")
    output_refs = {
        (entry.get("ref"), entry.get("sha256"))
        for entry in provenance.get("outputs", [])
        if isinstance(entry, dict)
    }
    if (PROJECTION_REF, _sha256_file(projection_path)) not in output_refs:
        raise LexicalIndexValidationError(
            "provenance does not bind tracked projection digest"
        )
    local_pair = (
        plan["local_projection"]["relative_path"],
        projection["local_projection_receipt"]["database_sha256"],
    )
    if local_pair not in output_refs:
        raise LexicalIndexValidationError(
            "provenance does not bind private local database digest"
        )
    if provenance.get("status") != "completed_with_warnings":
        raise LexicalIndexValidationError(
            "lexical export must preserve unresolved source/rights warnings"
        )

    if local_output_root is not None:
        _validate_local_database(local_output_root.resolve(), projection)
    return {
        "status": "ok",
        "projection_ref": PROJECTION_REF,
        "projection_sha256": _sha256_file(projection_path),
        "local_database_sha256": projection["local_projection_receipt"][
            "database_sha256"
        ],
        "local_database_verified": local_output_root is not None,
        "summary": summary,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--local-output-root",
        type=Path,
        help="optional explicit repository root for verifying ignored SQLite bytes",
    )
    args = parser.parse_args()
    print(
        json.dumps(
            validate(local_output_root=args.local_output_root),
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except LexicalIndexValidationError as exc:
        raise SystemExit(f"lexical index validation failed: {exc}") from exc

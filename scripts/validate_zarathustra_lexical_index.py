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
USAGE_CONTEXT_PLAN_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/usage-context-plan.v1.json"
)
USAGE_CONTEXT_RECEIPT_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/usage-context-receipt.v1.json"
)
USAGE_CONTEXT_PROVENANCE_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/usage-context-provenance.jsonl"
)
USAGE_CONTEXT_GENERATOR_REF = (
    "scripts/build_zarathustra_usage_context_bundle.py"
)
BASE_PROVENANCE_EVENT_REF = (
    "tos.event.export.zarathustra-lexical-index-v1.2026-07-29"
)
AUTHORITY_BOUNDARY = (
    "mechanical source-observation and rebuildable local search only; no "
    "accepted German, rights clearance, lexeme, lemma, translation, sign, "
    "concept, claim, relation, graph, canon, or publication authority"
)
USAGE_CONTEXT_AUTHORITY_BOUNDARY = (
    "private complete exact-form usage-context materialization for one "
    "preselected method control plus a tracked source-withholding receipt; "
    "no accepted German, sentence boundary, morphology, lemma, lexeme, "
    "translation correspondence, sign candidate, sign, concept, claim, "
    "relation, graph, canon, public route, or human backlog"
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


def _nested_keys(value: object) -> set[str]:
    keys: set[str] = set()
    if isinstance(value, dict):
        keys.update(str(key) for key in value)
        for child in value.values():
            keys.update(_nested_keys(child))
    elif isinstance(value, list):
        for child in value:
            keys.update(_nested_keys(child))
    return keys


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


def _validate_usage_context_layer() -> dict[str, Any]:
    plan_path = REPO_ROOT / USAGE_CONTEXT_PLAN_REF
    receipt_path = REPO_ROOT / USAGE_CONTEXT_RECEIPT_REF
    provenance_path = REPO_ROOT / USAGE_CONTEXT_PROVENANCE_REF
    plan = _load_json(plan_path)
    receipt = _load_json(receipt_path)
    _validate_schema(
        plan,
        REPO_ROOT / "ToS/contracts/lexical-usage-context-plan.schema.json",
        label="usage-context plan",
    )
    _validate_schema(
        receipt,
        REPO_ROOT / "ToS/contracts/lexical-usage-context-receipt.schema.json",
        label="usage-context receipt",
    )
    provenance_events = _load_provenance(provenance_path)
    if len(provenance_events) != 1:
        raise LexicalIndexValidationError(
            "usage-context provenance must contain exactly one event"
        )
    provenance = provenance_events[0]
    _validate_schema(
        provenance,
        REPO_ROOT / "ToS/contracts/provenance-event.schema.json",
        label="usage-context provenance event",
    )
    prohibited_tracked_row_keys = {
        "target_exact_form",
        "left_exact_tokens",
        "right_exact_tokens",
        "occurrence_id",
        "token_ordinal",
        "text_node_path",
        "start_offset",
        "end_offset",
    }
    leaked_keys = prohibited_tracked_row_keys.intersection(
        _nested_keys(receipt) | _nested_keys(provenance)
    )
    if leaked_keys:
        raise LexicalIndexValidationError(
            "usage-context tracked row data leaked through keys: "
            + ", ".join(sorted(leaked_keys))
        )

    if plan["status"] != "frozen-before-output" or not plan[
        "frozen_before_output"
    ]:
        raise LexicalIndexValidationError(
            "usage-context selection was not frozen before output"
        )
    if plan["authority_boundary"] != USAGE_CONTEXT_AUTHORITY_BOUNDARY:
        raise LexicalIndexValidationError("usage-context plan authority drift")
    if receipt["authority_boundary"] != USAGE_CONTEXT_AUTHORITY_BOUNDARY:
        raise LexicalIndexValidationError("usage-context receipt authority drift")
    if receipt["generated_or_authored"] != "generated_from_local_lexical_projection":
        raise LexicalIndexValidationError("usage-context generation posture drift")

    plan_digest = _sha256_file(plan_path)
    generator_digest = _sha256_file(REPO_ROOT / USAGE_CONTEXT_GENERATOR_REF)
    if receipt["plan"] != {"ref": USAGE_CONTEXT_PLAN_REF, "sha256": plan_digest}:
        raise LexicalIndexValidationError("usage-context plan receipt drift")
    if receipt["generator"] != {
        "ref": USAGE_CONTEXT_GENERATOR_REF,
        "sha256": generator_digest,
    }:
        raise LexicalIndexValidationError("usage-context generator receipt drift")
    if plan["tracked_receipt_ref"] != USAGE_CONTEXT_RECEIPT_REF:
        raise LexicalIndexValidationError("usage-context tracked receipt route drift")
    if plan["provenance_ref"] != USAGE_CONTEXT_PROVENANCE_REF:
        raise LexicalIndexValidationError("usage-context provenance route drift")

    source = plan["source_lexical_index"]
    source_refs = {
        "index_plan": (source["index_plan_ref"], source["index_plan_sha256"]),
        "lexical_projection": (
            source["tracked_projection_ref"],
            source["tracked_projection_sha256"],
        ),
        "recurrence_plan": (
            plan["recurrence_control"]["plan_ref"],
            plan["recurrence_control"]["plan_sha256"],
        ),
        "recurrence_projection": (
            plan["recurrence_control"]["projection_ref"],
            plan["recurrence_control"]["projection_sha256"],
        ),
    }
    if source_refs["index_plan"][0] != PLAN_REF:
        raise LexicalIndexValidationError("usage-context index-plan route drift")
    if source_refs["lexical_projection"][0] != PROJECTION_REF:
        raise LexicalIndexValidationError(
            "usage-context lexical-projection route drift"
        )
    for label, (ref, expected_digest) in source_refs.items():
        if _sha256_file(REPO_ROOT / ref) != expected_digest:
            raise LexicalIndexValidationError(
                f"usage-context {label} digest drift"
            )
        if receipt["source_projections"][label] != {
            "ref": ref,
            "sha256": expected_digest,
        }:
            raise LexicalIndexValidationError(
                f"usage-context {label} receipt drift"
            )

    database = receipt["source_database"]
    if database["relative_path"] != source["local_database_relative_path"]:
        raise LexicalIndexValidationError("usage-context database route drift")
    if database["sha256"] != source["local_database_sha256"]:
        raise LexicalIndexValidationError("usage-context database digest drift")
    if database["bytes"] != source["local_database_bytes"]:
        raise LexicalIndexValidationError("usage-context database byte drift")
    if database["quick_check"] != "ok":
        raise LexicalIndexValidationError("usage-context database check drift")

    control = plan["recurrence_control"]
    observed_control = receipt["recurrence_control"]
    if observed_control != {
        "form_key": control["form_key"],
        "exact_form_sha256": control["exact_form_sha256"],
        "selection_basis": control["selection_basis"],
        "observed_tuple": control["expected_tuple"],
    }:
        raise LexicalIndexValidationError("usage-context recurrence control drift")
    if not control["selection_frozen_before_context_output"]:
        raise LexicalIndexValidationError("usage-context control selection drift")
    if control["tracked_source_surface"]:
        raise LexicalIndexValidationError(
            "usage-context control unexpectedly exposes source surface"
        )

    policy = plan["context_policy"]
    expected_policy = {
        field: policy[field]
        for field in (
            "policy_id",
            "window_tokens_each_side",
            "boundary",
            "sampling",
            "row_order",
            "sentence_boundary_claimed",
        )
    }
    if receipt["context_policy"] != expected_policy:
        raise LexicalIndexValidationError("usage-context policy receipt drift")
    if policy["sampling"] != "none-complete-occurrence-census":
        raise LexicalIndexValidationError("usage-context census posture drift")
    if policy["sentence_boundary_claimed"]:
        raise LexicalIndexValidationError(
            "usage-context baseline claims a sentence boundary"
        )
    if policy["future_challengers_scheduled"]:
        raise LexicalIndexValidationError(
            "usage-context plan schedules unadmitted challengers"
        )

    local_plan = plan["local_bundle"]
    local_receipt = receipt["local_bundle"]
    for plan_field, receipt_field in (
        ("relative_path", "relative_path"),
        ("format", "format"),
        ("schema_ref", "schema_ref"),
        ("schema_version", "schema_version"),
        ("mode", "mode"),
        ("expected_row_count", "row_count"),
        ("required_fields", "required_fields"),
    ):
        if local_receipt[receipt_field] != local_plan[plan_field]:
            raise LexicalIndexValidationError(
                f"usage-context local bundle {receipt_field} drift"
            )
    if local_plan["storage_posture"] != "gitignored-local-only":
        raise LexicalIndexValidationError("usage-context storage posture drift")
    if not local_plan["source_bearing"]:
        raise LexicalIndexValidationError(
            "usage-context local bundle source posture drift"
        )

    expected_tuple = control["expected_tuple"]
    summary = receipt["summary"]
    summary_expectations = {
        "row_count": expected_tuple["occurrence_count"],
        "target_occurrence_count": expected_tuple["occurrence_count"],
        "source_item_count": expected_tuple["part_range"],
        "page_count": expected_tuple["page_range"],
        "section_count": expected_tuple["section_range"],
        "unsectioned_occurrence_count": expected_tuple[
            "unsectioned_occurrence_count"
        ],
        "source_editorial_occurrence_count": expected_tuple[
            "source_editorial_occurrence_count"
        ],
        "semantic_fields_populated": 0,
    }
    for field, expected in summary_expectations.items():
        if summary[field] != expected:
            raise LexicalIndexValidationError(
                f"usage-context summary {field} drift"
            )
    parts = receipt["parts"]
    if [part["part_order"] for part in parts] != list(
        range(1, summary["source_item_count"] + 1)
    ):
        raise LexicalIndexValidationError("usage-context part order drift")
    if len({part["item_ref"] for part in parts}) != summary["source_item_count"]:
        raise LexicalIndexValidationError("usage-context source-item closure drift")
    for field, summary_field in (
        ("occurrence_count", "target_occurrence_count"),
        ("page_count", "page_count"),
        ("section_count", "section_count"),
        ("unsectioned_occurrence_count", "unsectioned_occurrence_count"),
    ):
        if sum(part[field] for part in parts) != summary[summary_field]:
            raise LexicalIndexValidationError(
                f"usage-context part {field} closure drift"
            )

    identity = receipt["identity_closure"]
    target_count = summary["target_occurrence_count"]
    for field in (
        "unique_context_id_count",
        "unique_occurrence_id_count",
        "target_digest_match_count",
        "page_selector_resolution_count",
        "source_file_digest_resolution_count",
    ):
        if identity[field] != target_count:
            raise LexicalIndexValidationError(
                f"usage-context identity {field} drift"
            )
    if identity["section_selector_resolution_count"] != (
        target_count - summary["unsectioned_occurrence_count"]
    ):
        raise LexicalIndexValidationError(
            "usage-context section-selector closure drift"
        )
    if not identity["complete_occurrence_census"]:
        raise LexicalIndexValidationError("usage-context census is incomplete")

    if receipt["content_exposure"] != plan["content_exposure"]:
        raise LexicalIndexValidationError("usage-context exposure posture drift")
    exposure = receipt["content_exposure"]
    for field in (
        "tracked_exact_strings",
        "tracked_sequence",
        "tracked_context",
        "tracked_occurrence_positions",
        "confidentiality_claimed",
    ):
        if exposure[field]:
            raise LexicalIndexValidationError(
                f"usage-context tracked exposure opened: {field}"
            )
    if receipt["rights_and_visibility"] != plan["rights_and_visibility"]:
        raise LexicalIndexValidationError("usage-context rights posture drift")
    rights = receipt["rights_and_visibility"]
    if rights["future_site_route"] != "blocked" or not rights[
        "fresh_public_acquisition_and_rights_gate_required"
    ]:
        raise LexicalIndexValidationError("usage-context public route opened")
    if receipt["semantic_boundary"] != plan["semantic_boundary"]:
        raise LexicalIndexValidationError("usage-context semantic boundary drift")
    if any(receipt["semantic_boundary"].values()):
        raise LexicalIndexValidationError("usage-context semantic authority opened")

    if provenance["event_id"] != plan["provenance_event_ref"]:
        raise LexicalIndexValidationError("usage-context provenance identity drift")
    if provenance["event_type"] != "export":
        raise LexicalIndexValidationError("usage-context provenance type drift")
    if provenance["status"] != "completed_with_warnings":
        raise LexicalIndexValidationError("usage-context provenance status drift")
    if provenance["method"]["artifact_digest"] != generator_digest:
        raise LexicalIndexValidationError("usage-context provenance method drift")
    configuration = provenance["method"]["configuration"]
    if (
        configuration.get("target_occurrences") != target_count
        or configuration.get("tracked_source_strings") is not False
        or configuration.get("source_strings_local_only") is not True
        or configuration.get("sentence_boundary_claimed") is not False
        or configuration.get("future_challengers_scheduled") is not False
        or configuration.get("human_work_scheduled") is not False
    ):
        raise LexicalIndexValidationError(
            "usage-context provenance configuration drift"
        )

    input_refs = {
        (entry.get("ref"), entry.get("sha256"))
        for entry in provenance["inputs"]
    }
    expected_inputs = {
        (USAGE_CONTEXT_PLAN_REF, plan_digest),
        (source["local_database_relative_path"], source["local_database_sha256"]),
        source_refs["lexical_projection"],
        source_refs["recurrence_projection"],
        (plan["research_ref"], _sha256_file(REPO_ROOT / plan["research_ref"])),
    }
    if input_refs != expected_inputs:
        raise LexicalIndexValidationError("usage-context provenance input drift")
    receipt_digest = _sha256_file(receipt_path)
    output_refs = {
        (entry.get("ref"), entry.get("sha256"))
        for entry in provenance["outputs"]
    }
    expected_outputs = {
        (local_plan["relative_path"], local_receipt["sha256"]),
        (USAGE_CONTEXT_RECEIPT_REF, receipt_digest),
    }
    if output_refs != expected_outputs:
        raise LexicalIndexValidationError("usage-context provenance output drift")
    if provenance["rights_basis_ref"] is not None:
        raise LexicalIndexValidationError(
            "usage-context provenance unexpectedly claims a rights basis"
        )

    return {
        "plan_ref": USAGE_CONTEXT_PLAN_REF,
        "plan_sha256": plan_digest,
        "receipt_ref": USAGE_CONTEXT_RECEIPT_REF,
        "receipt_sha256": receipt_digest,
        "local_bundle_sha256": local_receipt["sha256"],
        "local_bundle_verified": False,
        "summary": summary,
        "authority_boundary": USAGE_CONTEXT_AUTHORITY_BOUNDARY,
    }
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
    usage_context = _validate_usage_context_layer()
    return {
        "status": "ok",
        "projection_ref": PROJECTION_REF,
        "projection_sha256": _sha256_file(projection_path),
        "local_database_sha256": projection["local_projection_receipt"][
            "database_sha256"
        ],
        "local_database_verified": local_output_root is not None,
        "summary": summary,
        "usage_context": usage_context,
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

#!/usr/bin/env python3
"""Validate the tracked source-witness evidence spine and optional local payloads.

This validator proves contract shape, reference closure, generated catalog parity,
and byte fixity. It does not prove bibliographic truth, OCR quality, translation
quality, semantic correctness, rights clearance, or human review.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable

from jsonschema import FormatChecker
from jsonschema.validators import validator_for

from build_source_witness_catalog import (
    CATALOG_ROOT,
    RECORD_FILES,
    SOURCE_BASENAMES,
    SOURCE_ROOT,
    CatalogBuildError,
    check_outputs,
    collect_records,
    render_outputs,
)


REPO_ROOT = Path(__file__).resolve().parents[1]
CONTRACT_ROOT = Path("ToS/contracts")

CORPUS_SCHEMA = CONTRACT_ROOT / "corpus-record.schema.json"
ITEM_MANIFEST_SCHEMA = CONTRACT_ROOT / "source-item-manifest.schema.json"
RESOURCE_INVENTORY_SCHEMA = CONTRACT_ROOT / "source-resource-inventory.schema.json"
RIGHTS_SCHEMA = CONTRACT_ROOT / "rights-record.schema.json"
PROVENANCE_SCHEMA = CONTRACT_ROOT / "provenance-event.schema.json"
CLAIM_SCHEMA = CONTRACT_ROOT / "claim-packet.schema.json"
CATALOG_SCHEMA = CONTRACT_ROOT / "source-witness-catalog.schema.json"
ANCHOR_SCHEMA = CONTRACT_ROOT / "source-anchor.schema.json"
COLLECTION_WORK_BOUNDARY_MAP_SCHEMA = (
    CONTRACT_ROOT / "collection-work-boundary-map.schema.json"
)
SAMPLE_PLAN_SCHEMA = CONTRACT_ROOT / "laboratory-sample-plan.schema.json"
OCR_VISUAL_SAMPLE_PLAN_SCHEMA = CONTRACT_ROOT / "ocr-visual-sample-plan.schema.json"
GOLD_STATUS_SCHEMA = CONTRACT_ROOT / "manual-gold-status.schema.json"
GOLD_ASSURANCE_SCHEMA = CONTRACT_ROOT / "manual-gold-assurance.schema.json"
TRANSLATION_SAMPLE_SCHEMA = CONTRACT_ROOT / "translation-sample-plan.schema.json"
TRANSLATION_SOURCE_REVIEW_SCHEMA = (
    CONTRACT_ROOT / "translation-source-review-plan.schema.json"
)
TRANSLATION_LABORATORY_PLAN_SCHEMA = (
    CONTRACT_ROOT / "translation-laboratory-plan.schema.json"
)
TRANSLATION_REFERENCE_REGISTER_SCHEMA = (
    CONTRACT_ROOT / "translation-reference-register.schema.json"
)
TRANSLATION_PRE_DRAFT_ANALYSIS_SCHEMA = (
    CONTRACT_ROOT / "translation-pre-draft-analysis.schema.json"
)
TRANSLATION_PACKET_SCHEMA = CONTRACT_ROOT / "translation-packet.schema.json"
SEMANTIC_LADDER_PACKET_SCHEMA = CONTRACT_ROOT / "semantic-ladder-packet.schema.json"
GOLDEN_KERNEL_TRANSFER_PLAN_SCHEMA = (
    CONTRACT_ROOT / "golden-kernel-transfer-plan.schema.json"
)
SOURCE_GATED_EVALUATION_PLAN_SCHEMA = (
    CONTRACT_ROOT / "source-gated-evaluation-plan.schema.json"
)
SOURCE_GATED_SEMANTIC_EVALUATION_PLAN_SCHEMA = (
    CONTRACT_ROOT / "source-gated-semantic-evaluation-plan.schema.json"
)
SOURCE_GATED_LLM_EVALUATION_PLAN_SCHEMA = (
    CONTRACT_ROOT / "source-gated-llm-evaluation-plan.schema.json"
)
MATERIAL_DISCOVERY_SCHEMA = CONTRACT_ROOT / "material-discovery-record.schema.json"
ACCESS_REQUEST_SCHEMA = CONTRACT_ROOT / "access-request.schema.json"
SERVER_IMPORT_SCHEMA = CONTRACT_ROOT / "server-import-contract.schema.json"
RETRIEVAL_QUERY_SCHEMA = CONTRACT_ROOT / "retrieval-query-plan.schema.json"
VISUAL_RETRIEVAL_PLAN_SCHEMA = CONTRACT_ROOT / "visual-retrieval-plan.schema.json"
GRAPH_QUERY_SCHEMA = CONTRACT_ROOT / "graph-query-plan.schema.json"
PRIVATE_EVIDENCE_HANDOFF_SCHEMA = (
    CONTRACT_ROOT / "private-laboratory-evidence-handoff.schema.json"
)
GERMAN_ASSISTED_SOURCE_REVIEW_SCHEMA = (
    CONTRACT_ROOT / "german-assisted-source-review.schema.json"
)
CRITICAL_EDITION_WITNESS_ADMISSION_SCHEMA = (
    CONTRACT_ROOT / "critical-edition-witness-admission.schema.json"
)
GERMAN_SOURCE_TRIANGULATION_SCHEMA = (
    CONTRACT_ROOT / "german-source-triangulation.schema.json"
)
BOUNDED_TRANSLATION_RESEARCH_INPUT_SCHEMA = (
    CONTRACT_ROOT / "bounded-translation-research-input.schema.json"
)
PRIVATE_EVIDENCE_HANDOFF_PROFILE = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "private-evidence-handoff.v1.json"
)

Issue = tuple[str, str]

PRIVATE_HANDOFF_REQUIRED_FORBIDDEN_CLASSES = {
    "source_page_bytes",
    "source_text_or_transcription",
    "restricted_translation_text",
    "unit_level_judgments",
    "screen_capture",
    "personal_interpretation",
    "operator_identity",
    "reviewer_identity",
    "absolute_or_home_path",
    "hostname_or_network_topology",
    "token_or_credential",
    "browser_url_with_token",
    "session_or_process_identifier",
    "private_timestamp",
    "raw_command_or_environment",
    "resolvable_private_locator",
}


def _load_json(path: Path, repo_root: Path, issues: list[Issue]) -> dict[str, Any] | None:
    relative = _relative(path, repo_root)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        issues.append((relative, "file is missing"))
        return None
    except (OSError, json.JSONDecodeError) as exc:
        issues.append((relative, f"cannot read JSON: {exc}"))
        return None
    if not isinstance(payload, dict):
        issues.append((relative, "JSON root must be an object"))
        return None
    return payload


def _load_jsonl(path: Path, repo_root: Path, issues: list[Issue]) -> list[dict[str, Any]]:
    relative = _relative(path, repo_root)
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        issues.append((relative, "file is missing"))
        return []
    except OSError as exc:
        issues.append((relative, f"cannot read JSONL: {exc}"))
        return []

    payloads: list[dict[str, Any]] = []
    for number, line in enumerate(lines, start=1):
        if not line.strip():
            issues.append((f"{relative}:{number}", "blank JSONL line is not allowed"))
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            issues.append((f"{relative}:{number}", f"invalid JSON: {exc}"))
            continue
        if not isinstance(payload, dict):
            issues.append((f"{relative}:{number}", "JSONL record must be an object"))
            continue
        payloads.append(payload)
    return payloads


def _relative(path: Path, repo_root: Path) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return path.as_posix()


def _schema_validator(schema_path: Path, repo_root: Path):
    schema = json.loads((repo_root / schema_path).read_text(encoding="utf-8"))
    validator_class = validator_for(schema)
    validator_class.check_schema(schema)
    return validator_class(schema, format_checker=FormatChecker()), schema


def _validate_payload(
    payload: object,
    validator: Any,
    location: str,
    issues: list[Issue],
) -> None:
    for error in sorted(validator.iter_errors(payload), key=lambda item: list(item.absolute_path)):
        suffix = "".join(f"[{part!r}]" for part in error.absolute_path)
        issues.append((f"{location}{suffix}", error.message))


def _repo_ref_exists(repo_root: Path, ref: object) -> bool:
    return isinstance(ref, str) and (not ref.startswith("ToS/") or (repo_root / ref).exists())


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _digest_bound_ref_issue(
    digest_bound_ref: object,
    *,
    expected_path: Path,
    repo_root: Path,
    field: str,
) -> str | None:
    if not isinstance(digest_bound_ref, dict):
        return f"{field} is not a digest-bound reference"
    if digest_bound_ref.get("ref") != _relative(expected_path, repo_root):
        return f"{field} does not cite the current owner artifact"
    if digest_bound_ref.get("sha256") != _sha256(expected_path):
        return f"{field} digest drifted"
    return None


def _solo_recheck_delay_issue(unit: dict[str, Any], minimum_delay: object) -> str | None:
    if unit.get("current_assurance") != "solo_human_delayed_rechecked":
        return None
    observed_delay = unit.get("review_evidence", {}).get("observed_delay_hours")
    valid_minimum = (
        isinstance(minimum_delay, (int, float))
        and not isinstance(minimum_delay, bool)
    )
    valid_observed = (
        isinstance(observed_delay, (int, float))
        and not isinstance(observed_delay, bool)
    )
    if valid_minimum and (not valid_observed or observed_delay < minimum_delay):
        return f"{unit.get('sample_id')} solo recheck is below the declared delay floor"
    return None


def _language_scope_overlap_issue(scope: dict[str, Any]) -> str | None:
    allowed = {
        claim for claim in scope.get("allowed_claims", []) if isinstance(claim, str)
    }
    blocked = {
        claim for claim in scope.get("blocked_claims", []) if isinstance(claim, str)
    }
    overlap = sorted(allowed & blocked)
    if overlap:
        return (
            f"{scope.get('language')} language scope both allows and blocks: "
            + ", ".join(overlap)
        )
    return None


def _assurance_reference_use_issue(unit: dict[str, Any]) -> str | None:
    assurance = unit.get("current_assurance")
    expected_by_assurance = {
        "unreviewed": "none",
        "language_competence_blocked": "none",
        "rejected": "none",
        "uncertain": "none",
        "single_human_source_visible": "criteria_evidence_only",
        "solo_human_delayed_rechecked": "calibration_metrics_with_disclosure",
        "independent_multi_human_adjudicated": "independent_multi_human_gold",
    }
    expected = expected_by_assurance.get(assurance)
    if expected is not None and unit.get("reference_use") != expected:
        return (
            f"{unit.get('sample_id')} reference use "
            f"{unit.get('reference_use')} is invalid for {assurance}"
        )
    return None


def _human_work_schedule_issues(
    assurance_units: dict[str, dict[str, Any]],
    schedule: object,
) -> list[str]:
    if not isinstance(schedule, dict):
        return ["human-work schedule is not an object"]
    selected = set(schedule.get("selected_calibration_unit_ids", []))
    unscheduled = set(schedule.get("unscheduled_unit_ids", []))
    issues: list[str] = []
    if selected & unscheduled:
        issues.append("selected calibration and unscheduled units overlap")
    if selected | unscheduled != set(assurance_units):
        issues.append("human-work schedule does not partition the frozen packet")
    if schedule.get("human_debt_units") != 0:
        issues.append("closed sparse calibration must report zero human debt")
    for sample_id in selected:
        unit = assurance_units.get(sample_id, {})
        if (
            unit.get("current_assurance") != "unreviewed"
            or unit.get("reference_use") != "none"
            or unit.get("next_route") != "none"
        ):
            issues.append(f"{sample_id} sparse calibration observation was promoted")
    return issues


def _owner_local_evidence_path(local_ref: object) -> Path | None:
    if not isinstance(local_ref, dict):
        return None
    if (
        local_ref.get("owner") != "abyss-stack"
        or local_ref.get("artifact_root") != "abyss_machine_artifact_store"
    ):
        return None
    relative_path = local_ref.get("relative_path")
    if not isinstance(relative_path, str):
        return None
    relative = Path(relative_path)
    if relative.is_absolute() or ".." in relative.parts:
        return None
    artifact_root = Path(
        os.environ.get(
            "ABYSS_MACHINE_ARTIFACT_ROOT",
            "/srv/abyss-machine/storage/artifacts",
        )
    )
    return artifact_root / relative


def _string_values(payload: object) -> Iterable[str]:
    if isinstance(payload, str):
        yield payload
    elif isinstance(payload, dict):
        for value in payload.values():
            yield from _string_values(value)
    elif isinstance(payload, list):
        for value in payload:
            yield from _string_values(value)


def _private_evidence_handoff_issues(
    handoff: object,
    *,
    repo_root: Path,
) -> list[str]:
    if not isinstance(handoff, dict):
        return ["private-evidence handoff is not an object"]
    issues: list[str] = []
    forbidden = set(
        handoff.get("disclosure_policy", {}).get("forbidden_classes", [])
    )
    missing_forbidden = sorted(
        PRIVATE_HANDOFF_REQUIRED_FORBIDDEN_CLASSES - forbidden
    )
    if missing_forbidden:
        issues.append(
            f"private-evidence handoff omits forbidden classes: {missing_forbidden}"
        )
    for value in _string_values(handoff):
        if value.startswith(("/", "~/")) or "/home/" in value or "/srv/" in value:
            issues.append("private-evidence handoff contains an absolute local path")
            break
    destination = handoff.get("destination", {})
    destination_ref = destination.get("artifact_path")
    if isinstance(destination_ref, str):
        destination_path = repo_root / destination_ref
        if (
            handoff.get("status") == "contract_frozen_raw_unopened"
            and destination_path.exists()
        ):
            issues.append(
                "contract-only private-evidence handoff already has a materialized derivative"
            )
    return issues


def _critical_edition_local_structural_context_issues(
    packet: object,
    *,
    target_unit: object,
    source_review_plan: object,
    ocr_sample_plan: object,
) -> list[str]:
    if not isinstance(packet, dict):
        return ["critical-edition witness is not an object"]
    if not isinstance(target_unit, dict):
        return ["critical-edition local structure has no source-review target"]
    if not isinstance(source_review_plan, dict):
        return ["critical-edition local structure has no source-review plan"]
    if not isinstance(ocr_sample_plan, dict):
        return ["critical-edition local structure has no visual sample plan"]

    issues: list[str] = []
    context = packet.get("local_structural_context", {})
    if not isinstance(context, dict):
        return ["critical-edition local_structural_context is not an object"]

    if context.get("source_expression_ref") != source_review_plan.get(
        "source_expression_ref"
    ):
        issues.append(
            "critical-edition local structure drifted from the source expression"
        )

    epub_witness = context.get("epub_witness", {})
    expected_epub = source_review_plan.get("source_witness", {})
    if not isinstance(epub_witness, dict) or not isinstance(expected_epub, dict):
        issues.append("critical-edition local EPUB witness is malformed")
    else:
        for field in ("item_ref", "file_ref", "file_sha256"):
            if epub_witness.get(field) != expected_epub.get(field):
                issues.append(
                    f"critical-edition local EPUB {field} drifted from the source-review plan"
                )
        start_member = epub_witness.get("section_start_member", {})
        if not isinstance(start_member, dict) or (
            start_member.get("path") != target_unit.get("container_member")
            or start_member.get("sha256") != target_unit.get("member_sha256")
        ):
            issues.append(
                "critical-edition local section start drifted from the target source unit"
            )
        boundary_member = epub_witness.get("next_section_boundary_member", {})
        start_path = (
            start_member.get("path") if isinstance(start_member, dict) else None
        )
        boundary_path = (
            boundary_member.get("path")
            if isinstance(boundary_member, dict)
            else None
        )
        start_match = (
            re.fullmatch(r"EPUB/page_([0-9]+)\.html", start_path)
            if isinstance(start_path, str)
            else None
        )
        boundary_match = (
            re.fullmatch(r"EPUB/page_([0-9]+)\.html", boundary_path)
            if isinstance(boundary_path, str)
            else None
        )
        if (
            start_match is None
            or boundary_match is None
            or int(boundary_match.group(1)) != int(start_match.group(1)) + 1
        ):
            issues.append(
                "critical-edition local section boundary is not the next EPUB member"
            )

    visual_witness = context.get("visual_witness", {})
    expected_visual = source_review_plan.get("visual_witness", {})
    visual_context = target_unit.get("visual_context", {})
    if (
        not isinstance(visual_witness, dict)
        or not isinstance(expected_visual, dict)
        or not isinstance(visual_context, dict)
    ):
        issues.append("critical-edition local visual witness is malformed")
    else:
        for field in ("item_ref", "file_ref", "file_sha256"):
            if visual_witness.get(field) != expected_visual.get(field):
                issues.append(
                    f"critical-edition local visual {field} drifted from the source-review plan"
                )
        if visual_witness.get("section_start_pdf_page") != visual_context.get(
            "current_pdf_page"
        ):
            issues.append(
                "critical-edition local section start page drifted from the target source unit"
            )
        if visual_witness.get("next_section_pdf_page") != visual_context.get(
            "next_pdf_page"
        ):
            issues.append(
                "critical-edition local next-section page drifted from the target source unit"
            )
        matching_visual_units = [
            unit
            for group in ocr_sample_plan.get("source_groups", [])
            if isinstance(group, dict)
            for unit in group.get("samples", [])
            if isinstance(unit, dict)
            and unit.get("page") == visual_witness.get("section_start_pdf_page")
            and unit.get("anchor_ref")
            == visual_witness.get("section_start_anchor_ref")
        ]
        if len(matching_visual_units) != 1:
            issues.append(
                "critical-edition local section start anchor is absent or ambiguous in the visual sample plan"
            )

    return issues


def _discovery_decision_issues(payload: object) -> list[str]:
    if not isinstance(payload, dict):
        return ["discovery record is not an object"]

    issues: list[str] = []
    result_ids: set[str] = set()
    expected_selected: set[str] = set()
    expected_rejected: set[str] = set()
    for channel in payload.get("channels", []):
        if not isinstance(channel, dict):
            continue
        for result in channel.get("results", []):
            if not isinstance(result, dict):
                continue
            result_id = result.get("result_id")
            if not isinstance(result_id, str):
                continue
            if result_id in result_ids:
                issues.append(f"duplicate discovery result_id: {result_id}")
            result_ids.add(result_id)
            if result.get("decision") == "select":
                expected_selected.add(result_id)
            elif result.get("decision") == "reject":
                expected_rejected.add(result_id)

    selected = {
        item for item in payload.get("selected_result_ids", []) if isinstance(item, str)
    }
    rejected = {
        item for item in payload.get("rejected_result_ids", []) if isinstance(item, str)
    }
    if selected != expected_selected:
        issues.append(
            "selected_result_ids do not match results whose decision is select"
        )
    if rejected != expected_rejected:
        issues.append(
            "rejected_result_ids do not match results whose decision is reject"
        )
    return issues


def _semantic_ladder_identity_issues(payload: object) -> list[str]:
    if not isinstance(payload, dict):
        return ["semantic ladder packet is not an object"]

    issues: list[str] = []
    stages = {
        stage.get("stage"): stage
        for stage in payload.get("stages", [])
        if isinstance(stage, dict) and isinstance(stage.get("stage"), str)
    }
    result = payload.get("result", {})
    if not isinstance(result, dict):
        result = {}

    candidate_stage = stages.get("stable_sign_candidate", {})
    candidate_body = candidate_stage.get("body", {})
    if (
        isinstance(candidate_body, dict)
        and candidate_stage.get("status") not in {"blocked", "not-started"}
    ):
        if candidate_body.get("candidate_ref") != payload.get("candidate_ref"):
            issues.append(
                "stable-sign candidate identity differs from packet candidate_ref"
            )
        earlier_occurrences = {
            ref
            for stage_name in (
                "exact_form",
                "frequency_and_concordance",
                "context",
                "morphology",
                "lemma",
                "recurrence_within_section",
                "recurrence_within_work",
                "recurrence_within_author_corpus",
            )
            for ref in stages.get(stage_name, {}).get("body", {}).get(
                "occurrence_refs",
                [],
            )
            if isinstance(ref, str)
        }
        candidate_occurrences = {
            ref
            for ref in candidate_body.get("occurrence_refs", [])
            if isinstance(ref, str)
        }
        if not candidate_occurrences or not candidate_occurrences.issubset(
            earlier_occurrences
        ):
            issues.append(
                "stable-sign candidate does not resolve to earlier occurrence evidence"
            )

    manual_stage = stages.get("manual_confirmation_or_rejection", {})
    manual_body = manual_stage.get("body", {})
    if manual_stage.get("status") == "human-accepted":
        if not isinstance(manual_body, dict) or manual_body.get(
            "accepted_sign_ref"
        ) != payload.get("accepted_sign_ref"):
            issues.append(
                "human sign decision identity differs from packet accepted_sign_ref"
            )
        if isinstance(manual_body, dict) and manual_body.get(
            "review_receipt_ref"
        ) not in result.get("human_decision_refs", []):
            issues.append(
                "human sign decision receipt is absent from packet result"
            )

    relation_stage = stages.get("relations_between_signs", {})
    relation_body = relation_stage.get("body", {})
    relation_records = (
        relation_body.get("relation_records", [])
        if isinstance(relation_body, dict)
        else []
    )
    relation_refs = {
        record.get("relation_ref")
        for record in relation_records
        if isinstance(record, dict) and isinstance(record.get("relation_ref"), str)
    }
    relation_claim_refs = {
        record.get("claim_ref")
        for record in relation_records
        if isinstance(record, dict) and isinstance(record.get("claim_ref"), str)
    }
    relation_sign_values = (
        relation_body.get("sign_refs", [])
        if isinstance(relation_body, dict)
        else []
    )
    declared_sign_refs = {
        ref for ref in relation_sign_values if isinstance(ref, str)
    }
    if relation_records:
        record_sign_refs = {
            ref
            for record in relation_records
            if isinstance(record, dict)
            for ref in (
                record.get("subject_sign_ref"),
                record.get("object_sign_ref"),
            )
            if isinstance(ref, str)
        }
        if record_sign_refs != declared_sign_refs:
            issues.append(
                "relation record sign endpoints differ from declared sign_refs"
            )
        if not relation_refs.issubset(set(result.get("relation_refs", []))):
            issues.append("relation identities are absent from packet result")
        if not relation_claim_refs.issubset(set(result.get("claim_refs", []))):
            issues.append("relation claims are absent from packet result")

    concept_stage = stages.get("conceptual_interpretations", {})
    concept_body = concept_stage.get("body", {})
    if (
        isinstance(concept_body, dict)
        and concept_stage.get("status") not in {"blocked", "not-started"}
    ):
        if concept_body.get("accepted_sign_ref") != payload.get(
            "accepted_sign_ref"
        ):
            issues.append(
                "concept interpretation sign differs from packet accepted_sign_ref"
            )
        if not set(concept_body.get("concept_refs", [])).issubset(
            set(result.get("concept_refs", []))
        ):
            issues.append("concept identities are absent from packet result")
        if not set(concept_body.get("claim_refs", [])).issubset(
            set(result.get("claim_refs", []))
        ):
            issues.append("concept claims are absent from packet result")

    counter_stage = stages.get("competing_readings", {})
    counter_body = counter_stage.get("body", {})
    if (
        isinstance(counter_body, dict)
        and counter_stage.get("status") not in {"blocked", "not-started"}
    ):
        counter_claim_refs = set(counter_body.get("primary_claim_refs", []))
        counter_claim_refs.update(counter_body.get("competing_claim_refs", []))
        if not counter_claim_refs.issubset(set(result.get("claim_refs", []))):
            issues.append("competing-reading claims are absent from packet result")

    graph_stage = stages.get("graph_projection", {})
    graph_body = graph_stage.get("body", {})
    if graph_stage.get("status") == "projected" and isinstance(graph_body, dict):
        if not set(graph_body.get("relation_refs", [])).issubset(relation_refs):
            issues.append(
                "graph relation identities do not resolve to relation records"
            )
        if not set(graph_body.get("claim_refs", [])).issubset(
            set(result.get("claim_refs", []))
        ):
            issues.append("graph claims are absent from packet result")
        if graph_body.get("projection_ref") not in result.get(
            "graph_projection_refs",
            [],
        ):
            issues.append("graph projection is absent from packet result")

    return issues


def _visual_retrieval_plan_issues(
    payload: object,
    *,
    source_query_plan: object,
    source_sample_plan: object,
    visual_sample_plan: object,
) -> list[str]:
    if not isinstance(payload, dict):
        return ["visual retrieval plan is not an object"]
    if not isinstance(source_query_plan, dict):
        return ["visual retrieval plan has no source query plan"]
    if not isinstance(source_sample_plan, dict):
        return ["visual retrieval plan has no source sample plan"]
    if not isinstance(visual_sample_plan, dict):
        return ["visual retrieval plan has no visual sample plan"]

    issues: list[str] = []
    source_anchor_to_sample: dict[str, str] = {}
    for group in source_sample_plan.get("source_groups", []):
        if not isinstance(group, dict):
            continue
        for sample in group.get("samples", []):
            if not isinstance(sample, dict):
                continue
            anchor_ref = sample.get("anchor_ref")
            sample_id = sample.get("sample_id")
            if isinstance(anchor_ref, str) and isinstance(sample_id, str):
                if anchor_ref in source_anchor_to_sample:
                    issues.append(
                        f"source anchor has multiple sample identities: {anchor_ref}"
                    )
                source_anchor_to_sample[anchor_ref] = sample_id

    source_sample_to_visual_anchor: dict[str, str] = {}
    visual_anchors: set[str] = set()
    for group in visual_sample_plan.get("source_groups", []):
        if not isinstance(group, dict):
            continue
        for sample in group.get("samples", []):
            if not isinstance(sample, dict):
                continue
            source_sample_id = sample.get("source_sample_id")
            visual_anchor_ref = sample.get("anchor_ref")
            if isinstance(visual_anchor_ref, str):
                visual_anchors.add(visual_anchor_ref)
            if isinstance(source_sample_id, str) and isinstance(
                visual_anchor_ref, str
            ):
                if source_sample_id in source_sample_to_visual_anchor:
                    issues.append(
                        f"source sample has multiple visual anchors: {source_sample_id}"
                    )
                source_sample_to_visual_anchor[source_sample_id] = (
                    visual_anchor_ref
                )

    unresolved_query_ids: list[str] = []
    query_ids: set[str] = set()
    for query in source_query_plan.get("queries", []):
        if not isinstance(query, dict):
            continue
        query_id = query.get("query_id")
        if not isinstance(query_id, str):
            continue
        query_ids.add(query_id)
        query_anchors = [
            anchor_ref
            for anchor_ref in (
                list(query.get("expected_source_anchor_refs", []))
                + list(query.get("hard_negative_anchor_refs", []))
            )
            if isinstance(anchor_ref, str)
        ]
        query_unresolved = False
        for anchor_ref in query_anchors:
            source_sample_id = source_anchor_to_sample.get(anchor_ref)
            visual_anchor_ref = (
                source_sample_to_visual_anchor.get(source_sample_id)
                if source_sample_id is not None
                else None
            )
            if visual_anchor_ref not in visual_anchors:
                query_unresolved = True
        if query_unresolved:
            unresolved_query_ids.append(query_id)

    projection = payload.get("query_projection", {})
    if not isinstance(projection, dict):
        return issues + ["visual retrieval query_projection is not an object"]
    if projection.get("query_count") != len(query_ids):
        issues.append("visual retrieval query count differs from source plan")
    if projection.get("resolved_query_count") != len(query_ids) - len(
        unresolved_query_ids
    ):
        issues.append("visual retrieval resolved query count drifted")
    if projection.get("unresolved_query_ids") != unresolved_query_ids:
        issues.append("visual retrieval unresolved query IDs drifted")

    crosswalk = projection.get("source_to_visual_anchor_crosswalk", {})
    if not isinstance(crosswalk, dict):
        issues.append("visual retrieval anchor crosswalk is not an object")
    else:
        if crosswalk.get("source_anchor_count") != len(source_anchor_to_sample):
            issues.append("visual retrieval source anchor count drifted")
        if crosswalk.get("visual_anchor_count") != len(visual_anchors):
            issues.append("visual retrieval visual anchor count drifted")
        if crosswalk.get("one_to_one_for_frozen_queries") is not (
            not unresolved_query_ids
        ):
            issues.append("visual retrieval one-to-one query crosswalk drifted")

    return issues


def _git_ignored(repo_root: Path, path: Path) -> bool | None:
    if not (repo_root / ".git").exists():
        return None
    result = subprocess.run(
        ("git", "check-ignore", "--quiet", "--", _relative(path, repo_root)),
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return True
    if result.returncode == 1:
        return False
    return None


def validate_payload_file(
    repo_root: Path,
    item_directory: Path,
    payload_entry: dict[str, Any],
    *,
    require_local_payloads: bool,
) -> list[Issue]:
    """Validate one manifest payload entry; public clones may omit local bytes."""

    issues: list[Issue] = []
    location = _relative(item_directory / "item.manifest.json", repo_root)
    relative_path = payload_entry.get("relative_path")
    if not isinstance(relative_path, str):
        return [(location, "payload relative_path is not a string")]

    payload_path = item_directory / relative_path
    if not payload_path.is_file():
        if require_local_payloads:
            issues.append((_relative(payload_path, repo_root), "required local payload is missing"))
        return issues

    expected_size = payload_entry.get("byte_size")
    actual_size = payload_path.stat().st_size
    if actual_size != expected_size:
        issues.append(
            (_relative(payload_path, repo_root), f"byte size {actual_size} != manifest {expected_size}")
        )

    expected_digest = payload_entry.get("sha256")
    actual_digest = _sha256(payload_path)
    if actual_digest != expected_digest:
        issues.append(
            (_relative(payload_path, repo_root), f"sha256 {actual_digest} != manifest {expected_digest}")
        )

    ignored = _git_ignored(repo_root, payload_path)
    if ignored is False:
        issues.append((_relative(payload_path, repo_root), "local payload is not ignored by Git"))
    elif ignored is None and (repo_root / ".git").exists():
        issues.append((_relative(payload_path, repo_root), "could not determine Git ignore posture"))
    return issues


def _validate_source_refs(repo_root: Path, payload: dict[str, Any], location: str, issues: list[Issue]) -> None:
    refs: list[object] = []
    for field in ("source_refs", "source_record_refs", "receipt_refs"):
        value = payload.get(field, [])
        if isinstance(value, list):
            refs.extend(value)
    for field in (
        "rights_ref",
        "provenance_ref",
        "forensic_report_ref",
        "resource_inventory_ref",
        "generated_from_manifest_ref",
        "item_manifest_ref",
    ):
        if field in payload:
            refs.append(payload[field])
    for ref in refs:
        if not _repo_ref_exists(repo_root, ref):
            issues.append((location, f"repository reference does not exist: {ref}"))


def _record_paths(repo_root: Path) -> Iterable[Path]:
    source_root = repo_root / SOURCE_ROOT
    for basename in SOURCE_BASENAMES.values():
        for path in sorted(source_root.rglob(basename)):
            if CATALOG_ROOT in path.relative_to(repo_root).parents:
                continue
            yield path


def validate_foundation(repo_root: Path, *, require_local_payloads: bool = False) -> list[Issue]:
    repo_root = repo_root.resolve()
    issues: list[Issue] = []

    try:
        corpus_validator, _ = _schema_validator(CORPUS_SCHEMA, repo_root)
        manifest_validator, _ = _schema_validator(ITEM_MANIFEST_SCHEMA, repo_root)
        resource_inventory_validator, _ = _schema_validator(
            RESOURCE_INVENTORY_SCHEMA,
            repo_root,
        )
        rights_validator, _ = _schema_validator(RIGHTS_SCHEMA, repo_root)
        provenance_validator, _ = _schema_validator(PROVENANCE_SCHEMA, repo_root)
        claim_validator, _ = _schema_validator(CLAIM_SCHEMA, repo_root)
        anchor_validator, _ = _schema_validator(ANCHOR_SCHEMA, repo_root)
        collection_work_boundary_map_validator, _ = _schema_validator(
            COLLECTION_WORK_BOUNDARY_MAP_SCHEMA,
            repo_root,
        )
        sample_plan_validator, _ = _schema_validator(SAMPLE_PLAN_SCHEMA, repo_root)
        ocr_visual_sample_plan_validator, _ = _schema_validator(
            OCR_VISUAL_SAMPLE_PLAN_SCHEMA,
            repo_root,
        )
        gold_status_validator, _ = _schema_validator(GOLD_STATUS_SCHEMA, repo_root)
        gold_assurance_validator, _ = _schema_validator(
            GOLD_ASSURANCE_SCHEMA,
            repo_root,
        )
        translation_sample_validator, _ = _schema_validator(TRANSLATION_SAMPLE_SCHEMA, repo_root)
        translation_source_review_validator, _ = _schema_validator(
            TRANSLATION_SOURCE_REVIEW_SCHEMA,
            repo_root,
        )
        translation_laboratory_plan_validator, _ = _schema_validator(
            TRANSLATION_LABORATORY_PLAN_SCHEMA,
            repo_root,
        )
        translation_reference_register_validator, _ = _schema_validator(
            TRANSLATION_REFERENCE_REGISTER_SCHEMA,
            repo_root,
        )
        _schema_validator(TRANSLATION_PRE_DRAFT_ANALYSIS_SCHEMA, repo_root)
        _schema_validator(TRANSLATION_PACKET_SCHEMA, repo_root)
        semantic_ladder_packet_validator, _ = _schema_validator(
            SEMANTIC_LADDER_PACKET_SCHEMA,
            repo_root,
        )
        golden_kernel_transfer_plan_validator, _ = _schema_validator(
            GOLDEN_KERNEL_TRANSFER_PLAN_SCHEMA,
            repo_root,
        )
        _schema_validator(
            SOURCE_GATED_EVALUATION_PLAN_SCHEMA,
            repo_root,
        )
        source_gated_semantic_evaluation_plan_validator, _ = _schema_validator(
            SOURCE_GATED_SEMANTIC_EVALUATION_PLAN_SCHEMA,
            repo_root,
        )
        source_gated_llm_evaluation_plan_validator, _ = _schema_validator(
            SOURCE_GATED_LLM_EVALUATION_PLAN_SCHEMA,
            repo_root,
        )
        material_discovery_validator, _ = _schema_validator(
            MATERIAL_DISCOVERY_SCHEMA,
            repo_root,
        )
        access_request_validator, _ = _schema_validator(ACCESS_REQUEST_SCHEMA, repo_root)
        server_import_validator, _ = _schema_validator(SERVER_IMPORT_SCHEMA, repo_root)
        retrieval_query_validator, _ = _schema_validator(RETRIEVAL_QUERY_SCHEMA, repo_root)
        visual_retrieval_plan_validator, _ = _schema_validator(
            VISUAL_RETRIEVAL_PLAN_SCHEMA,
            repo_root,
        )
        graph_query_validator, _ = _schema_validator(GRAPH_QUERY_SCHEMA, repo_root)
        private_evidence_handoff_validator, _ = _schema_validator(
            PRIVATE_EVIDENCE_HANDOFF_SCHEMA,
            repo_root,
        )
        german_assisted_review_validator, _ = _schema_validator(
            GERMAN_ASSISTED_SOURCE_REVIEW_SCHEMA,
            repo_root,
        )
        critical_edition_witness_validator, _ = _schema_validator(
            CRITICAL_EDITION_WITNESS_ADMISSION_SCHEMA,
            repo_root,
        )
        german_source_triangulation_validator, _ = _schema_validator(
            GERMAN_SOURCE_TRIANGULATION_SCHEMA,
            repo_root,
        )
        bounded_translation_research_input_validator, _ = _schema_validator(
            BOUNDED_TRANSLATION_RESEARCH_INPUT_SCHEMA,
            repo_root,
        )
        catalog_validator, catalog_schema = _schema_validator(CATALOG_SCHEMA, repo_root)
    except (FileNotFoundError, OSError, json.JSONDecodeError) as exc:
        return [(CONTRACT_ROOT.as_posix(), f"cannot load contract schemas: {exc}")]

    private_handoff_path = repo_root / PRIVATE_EVIDENCE_HANDOFF_PROFILE
    private_handoff = _load_json(private_handoff_path, repo_root, issues)
    if private_handoff is not None:
        private_handoff_location = _relative(private_handoff_path, repo_root)
        _validate_payload(
            private_handoff,
            private_evidence_handoff_validator,
            private_handoff_location,
            issues,
        )
        for message in _private_evidence_handoff_issues(
            private_handoff,
            repo_root=repo_root,
        ):
            issues.append((private_handoff_location, message))

    records_by_id: dict[str, tuple[dict[str, Any], Path]] = {}
    item_records: dict[str, tuple[dict[str, Any], Path]] = {}
    for path in _record_paths(repo_root):
        payload = _load_json(path, repo_root, issues)
        if payload is None:
            continue
        location = _relative(path, repo_root)
        _validate_payload(payload, corpus_validator, location, issues)
        _validate_source_refs(repo_root, payload, location, issues)
        record_id = payload.get("record_id")
        if isinstance(record_id, str):
            if record_id in records_by_id:
                issues.append((location, f"duplicate record_id: {record_id}"))
            else:
                records_by_id[record_id] = (payload, path)
            if payload.get("record_type") == "item":
                item_records[record_id] = (payload, path)

    def require_record(ref: object, expected_type: str, location: str) -> None:
        if not isinstance(ref, str):
            return
        target = records_by_id.get(ref)
        if target is None:
            issues.append((location, f"unresolved {expected_type} reference: {ref}"))
        elif target[0].get("record_type") != expected_type:
            issues.append((location, f"{ref} resolves to {target[0].get('record_type')}, expected {expected_type}"))

    for record_id, (payload, path) in records_by_id.items():
        location = _relative(path, repo_root)
        if payload.get("record_type") == "expression":
            require_record(payload.get("work_ref"), "work", location)
        elif payload.get("record_type") == "edition":
            for ref in payload.get("embodies_expression_refs", []):
                require_record(ref, "expression", location)
            if "collection_ref" in payload:
                require_record(payload.get("collection_ref"), "collection", location)

    event_ids: set[str] = set()
    claim_ids: set[str] = set()
    rights_ids: set[str] = set()
    manifest_item_ids: set[str] = set()
    item_edition_by_id: dict[str, str] = {}
    file_owner_by_id: dict[str, str] = {}
    file_digest_by_id: dict[str, str] = {}

    for manifest_path in sorted((repo_root / SOURCE_ROOT).rglob("item.manifest.json")):
        manifest = _load_json(manifest_path, repo_root, issues)
        if manifest is None:
            continue
        location = _relative(manifest_path, repo_root)
        _validate_payload(manifest, manifest_validator, location, issues)
        _validate_source_refs(repo_root, manifest, location, issues)
        item_id = manifest.get("item_id")
        if isinstance(item_id, str):
            manifest_item_ids.add(item_id)
            require_record(item_id, "item", location)
            embodiment_ref = manifest.get("embodiment_ref")
            if isinstance(embodiment_ref, str):
                item_edition_by_id[item_id] = embodiment_ref
        require_record(manifest.get("embodiment_ref"), "edition", location)

        item_directory = manifest_path.parent
        inventory_path = repo_root / str(manifest.get("resource_inventory_ref", ""))
        inventory = _load_json(inventory_path, repo_root, issues)
        if inventory is not None:
            inventory_location = _relative(inventory_path, repo_root)
            _validate_payload(
                inventory,
                resource_inventory_validator,
                inventory_location,
                issues,
            )
            _validate_source_refs(repo_root, inventory, inventory_location, issues)
            if inventory.get("item_id") != item_id:
                issues.append(
                    (inventory_location, "resource inventory item_id differs from manifest")
                )
            if inventory.get("generated_from_manifest_ref") != location:
                issues.append(
                    (
                        inventory_location,
                        "resource inventory does not cite its current item manifest",
                    )
                )
            expected_inventory_files = [
                {
                    "file_id": entry.get("file_id"),
                    "file_sha256": entry.get("sha256"),
                    "media_type": entry.get("media_type"),
                }
                for entry in manifest.get("payload_files", [])
                if isinstance(entry, dict)
            ]
            actual_inventory_files = [
                {
                    "file_id": entry.get("file_id"),
                    "file_sha256": entry.get("file_sha256"),
                    "media_type": entry.get("media_type"),
                }
                for entry in inventory.get("files", [])
                if isinstance(entry, dict)
            ]
            if actual_inventory_files != expected_inventory_files:
                issues.append(
                    (
                        inventory_location,
                        "resource inventory file identity differs from manifest payload files",
                    )
                )
            for file_inventory in inventory.get("files", []):
                if not isinstance(file_inventory, dict):
                    continue
                resources = file_inventory.get("resources", [])
                resource_ids = [
                    resource.get("resource_id")
                    for resource in resources
                    if isinstance(resource, dict)
                ]
                if len(resource_ids) != len(set(resource_ids)):
                    issues.append(
                        (
                            inventory_location,
                            f"duplicate resource_id in {file_inventory.get('file_id')}",
                        )
                    )
                summary = file_inventory.get("summary", {})
                if isinstance(summary, dict) and summary.get("resource_count") != len(
                    resources
                ):
                    issues.append(
                        (
                            inventory_location,
                            f"resource_count differs from resources for {file_inventory.get('file_id')}",
                        )
                    )

        rights_path = repo_root / str(manifest.get("rights_ref", ""))
        rights = _load_json(rights_path, repo_root, issues)
        if rights is not None:
            rights_location = _relative(rights_path, repo_root)
            _validate_payload(rights, rights_validator, rights_location, issues)
            _validate_source_refs(repo_root, rights, rights_location, issues)
            rights_id = rights.get("rights_id")
            if isinstance(rights_id, str):
                rights_ids.add(rights_id)
            scopes = rights.get("scope_refs", [])
            if item_id not in scopes:
                issues.append((rights_location, f"scope_refs does not include item_id {item_id}"))
            if rights.get("visibility") != manifest.get("visibility"):
                issues.append((rights_location, "rights visibility differs from item manifest visibility"))

        provenance_path = repo_root / str(manifest.get("provenance_ref", ""))
        local_event_ids: set[str] = set()
        local_events_by_id: dict[str, dict[str, Any]] = {}
        for index, event in enumerate(_load_jsonl(provenance_path, repo_root, issues), start=1):
            event_location = f"{_relative(provenance_path, repo_root)}:{index}"
            _validate_payload(event, provenance_validator, event_location, issues)
            _validate_source_refs(repo_root, event, event_location, issues)
            event_id = event.get("event_id")
            if isinstance(event_id, str):
                if event_id in event_ids:
                    issues.append((event_location, f"duplicate event_id: {event_id}"))
                event_ids.add(event_id)
                local_event_ids.add(event_id)
                local_events_by_id[event_id] = event
        acquisition_ref = manifest.get("acquisition_event_ref")
        if acquisition_ref not in local_event_ids:
            issues.append((location, f"acquisition_event_ref is absent from provenance: {acquisition_ref}"))
        if inventory is not None:
            inventory_location = _relative(inventory_path, repo_root)
            inventory_event_ref = inventory.get("provenance_event_ref")
            inventory_event = local_events_by_id.get(inventory_event_ref)
            if inventory_event is None:
                issues.append(
                    (
                        inventory_location,
                        f"provenance_event_ref is absent from item provenance: {inventory_event_ref}",
                    )
                )
            else:
                inventory_digest = _sha256(inventory_path)
                expected_output = {
                    "ref": inventory_location,
                    "role": "tracked_text_free_resource_inventory",
                    "sha256": inventory_digest,
                }
                if expected_output not in inventory_event.get("outputs", []):
                    issues.append(
                        (
                            inventory_location,
                            "resource inventory provenance event lacks its digest-bound output",
                        )
                    )

        fixity_path = item_directory / "fixity.sha256"
        try:
            actual_fixity = fixity_path.read_text(encoding="utf-8")
        except FileNotFoundError:
            issues.append((_relative(fixity_path, repo_root), "fixity companion is missing"))
            actual_fixity = ""
        expected_lines: list[str] = []
        file_ids: set[str] = set()
        for payload_entry in manifest.get("payload_files", []):
            if not isinstance(payload_entry, dict):
                continue
            expected_lines.append(f"{payload_entry.get('sha256')}  {payload_entry.get('relative_path')}")
            file_id = payload_entry.get("file_id")
            if isinstance(file_id, str):
                file_ids.add(file_id)
                if isinstance(item_id, str):
                    existing_owner = file_owner_by_id.get(file_id)
                    if existing_owner is not None and existing_owner != item_id:
                        issues.append((location, f"file_id {file_id} belongs to multiple items"))
                    file_owner_by_id[file_id] = item_id
                file_digest = payload_entry.get("sha256")
                if isinstance(file_digest, str):
                    file_digest_by_id[file_id] = file_digest
            issues.extend(
                validate_payload_file(
                    repo_root,
                    item_directory,
                    payload_entry,
                    require_local_payloads=require_local_payloads,
                )
            )
        expected_fixity = "\n".join(expected_lines) + ("\n" if expected_lines else "")
        if actual_fixity and actual_fixity != expected_fixity:
            issues.append((_relative(fixity_path, repo_root), "fixity companion differs from item manifest"))
        if rights is not None:
            rights_scopes = set(rights.get("scope_refs", []))
            missing_file_scopes = sorted(file_ids - rights_scopes)
            if missing_file_scopes:
                issues.append((_relative(rights_path, repo_root), f"scope_refs omits payload files: {missing_file_scopes}"))

    for item_id, (item, path) in item_records.items():
        location = _relative(path, repo_root)
        if item_id not in manifest_item_ids:
            issues.append((location, "item record has no validated item.manifest.json"))
        manifest_ref = item.get("item_manifest_ref")
        if isinstance(manifest_ref, str):
            manifest = _load_json(repo_root / manifest_ref, repo_root, issues)
            if manifest is not None and manifest.get("item_id") != item_id:
                issues.append((manifest_ref, f"manifest item_id does not match {item_id}"))

    gold_roots = sorted(
        path
        for path in (repo_root / SOURCE_ROOT).rglob("gold-sets/*")
        if path.is_dir()
    )
    seen_anchor_ids: set[str] = set()
    for gold_root in gold_roots:
        gold_location = _relative(gold_root, repo_root)
        sample_path = gold_root / "sample-plan.json"
        ocr_sample_path = gold_root / "ocr-visual-samples.json"
        ocr_anchor_path = gold_root / "ocr-anchors.jsonl"
        gold_status_path = gold_root / "gold-status.json"
        gold_assurance_path = gold_root / "gold-assurance.v2.json"
        translation_path = gold_root / "translation-samples.json"
        translation_source_review_path = gold_root / "translation-source-review-plan.v2.json"
        translation_laboratory_path = gold_root / "translation-laboratory-plan.v1.json"
        translation_reference_register_path = (
            gold_root / "translation-reference-register.v1.json"
        )
        german_assisted_review_path = (
            gold_root / "german-assisted-source-review.v1.json"
        )
        critical_edition_witness_paths = sorted(
            gold_root.glob("critical-edition-witness.*.json")
        )
        german_source_triangulation_path = (
            gold_root
            / "german-source-triangulation."
            "ekgwb-dta-naumann.za-i-vorrede-1.v1.json"
        )
        bounded_translation_research_input_path = (
            gold_root
            / "bounded-translation-research-input."
            "za-i-vorrede-1-opening-sentence.v1.json"
        )
        initial_sign_packet_path = gold_root / "initial-sign-packet.v3.json"
        transfer_path = gold_root / "transfer-samples.json"
        semantic_samples_path = gold_root / "semantic-samples.json"
        llm_tasks_path = gold_root / "llm-tasks.json"
        retrieval_path = gold_root / "retrieval-queries.json"
        visual_retrieval_path = gold_root / "visual-retrieval-plan.v1.json"
        graph_claims_path = gold_root / "graph-claims.jsonl"
        graph_queries_path = gold_root / "graph-queries.json"
        provenance_path = gold_root / "provenance.jsonl"
        graph_provenance_path = gold_root / "graph-provenance.jsonl"
        transfer_provenance_path = gold_root / "transfer-provenance.jsonl"
        evaluation_provenance_path = gold_root / "evaluation-provenance.jsonl"
        german_source_triangulation_provenance_path = (
            gold_root / "provenance.german-source-triangulation.jsonl"
        )
        bounded_translation_research_input_provenance_path = (
            gold_root
            / "provenance.bounded-translation-research-input.jsonl"
        )
        anchor_paths = [gold_root / "anchors.jsonl", gold_root / "translation-anchors.jsonl"]
        if ocr_anchor_path.is_file():
            anchor_paths.append(ocr_anchor_path)

        sample_plan = _load_json(sample_path, repo_root, issues)
        ocr_sample_plan = (
            _load_json(ocr_sample_path, repo_root, issues)
            if ocr_sample_path.is_file()
            else None
        )
        gold_status = _load_json(gold_status_path, repo_root, issues)
        gold_assurance = _load_json(gold_assurance_path, repo_root, issues)
        translation_plan = _load_json(translation_path, repo_root, issues)
        translation_source_review_plan = (
            _load_json(translation_source_review_path, repo_root, issues)
            if translation_source_review_path.is_file()
            else None
        )
        translation_laboratory_plan = (
            _load_json(translation_laboratory_path, repo_root, issues)
            if translation_laboratory_path.is_file()
            else None
        )
        translation_reference_register = (
            _load_json(translation_reference_register_path, repo_root, issues)
            if translation_reference_register_path.is_file()
            else None
        )
        german_assisted_review = (
            _load_json(german_assisted_review_path, repo_root, issues)
            if german_assisted_review_path.is_file()
            else None
        )
        critical_edition_witnesses = [
            (path, payload)
            for path in critical_edition_witness_paths
            if (payload := _load_json(path, repo_root, issues)) is not None
        ]
        german_source_triangulation = (
            _load_json(
                german_source_triangulation_path,
                repo_root,
                issues,
            )
            if german_source_triangulation_path.is_file()
            else None
        )
        bounded_translation_research_input = (
            _load_json(
                bounded_translation_research_input_path,
                repo_root,
                issues,
            )
            if bounded_translation_research_input_path.is_file()
            else None
        )
        initial_sign_packet = _load_json(
            initial_sign_packet_path,
            repo_root,
            issues,
        )
        transfer_plan = _load_json(transfer_path, repo_root, issues)
        semantic_samples_plan = _load_json(semantic_samples_path, repo_root, issues)
        llm_tasks_plan = _load_json(llm_tasks_path, repo_root, issues)
        retrieval_plan = _load_json(retrieval_path, repo_root, issues)
        visual_retrieval_plan = (
            _load_json(visual_retrieval_path, repo_root, issues)
            if visual_retrieval_path.is_file()
            else None
        )
        graph_query_plan = _load_json(graph_queries_path, repo_root, issues)
        if sample_plan is not None:
            _validate_payload(sample_plan, sample_plan_validator, _relative(sample_path, repo_root), issues)
        if ocr_sample_plan is not None:
            _validate_payload(
                ocr_sample_plan,
                ocr_visual_sample_plan_validator,
                _relative(ocr_sample_path, repo_root),
                issues,
            )
        if ocr_sample_path.is_file() != ocr_anchor_path.is_file():
            issues.append(
                (
                    gold_location,
                    "OCR visual sample plan and OCR anchor companion must either both exist or both be absent",
                )
            )
        if gold_status is not None:
            _validate_payload(gold_status, gold_status_validator, _relative(gold_status_path, repo_root), issues)
        if gold_assurance is not None:
            _validate_payload(
                gold_assurance,
                gold_assurance_validator,
                _relative(gold_assurance_path, repo_root),
                issues,
            )
        if translation_plan is not None:
            _validate_payload(
                translation_plan,
                translation_sample_validator,
                _relative(translation_path, repo_root),
                issues,
            )
        if transfer_plan is not None:
            _validate_payload(
                transfer_plan,
                golden_kernel_transfer_plan_validator,
                _relative(transfer_path, repo_root),
                issues,
            )
        if initial_sign_packet is not None:
            initial_sign_location = _relative(
                initial_sign_packet_path,
                repo_root,
            )
            _validate_payload(
                initial_sign_packet,
                semantic_ladder_packet_validator,
                initial_sign_location,
                issues,
            )
            issues.extend(
                (initial_sign_location, message)
                for message in _semantic_ladder_identity_issues(
                    initial_sign_packet
                )
            )
            if initial_sign_packet.get("packet_status") != "blocked-not-materialized":
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet must remain blocked until content is materialized",
                    )
                )
            source_gate = initial_sign_packet.get(
                "task_specific_source_gate",
                {},
            )
            if (
                source_gate.get("gate_status") != "blocked"
                or source_gate.get("source_anchor_refs") != []
                or source_gate.get("language_competence_status") != "blocked"
                or source_gate.get("language_competence_evidence_refs") != []
            ):
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet source and competence gate drifted",
                    )
                )
            if (
                initial_sign_packet.get("source_forms") is not None
                or initial_sign_packet.get("candidate_ref") is not None
                or initial_sign_packet.get("accepted_sign_ref") is not None
                or initial_sign_packet.get("translation_evidence") != []
            ):
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet manufactured source or semantic content",
                    )
                )
            stages = initial_sign_packet.get("stages", [])
            if (
                len(stages) != 15
                or any(
                    not isinstance(stage, dict)
                    or stage.get("status") != "blocked"
                    or stage.get("source_anchor_refs") != []
                    or stage.get("body") != {}
                    or stage.get("maker") is not None
                    or stage.get("provenance_event_ref") is not None
                    for stage in stages
                )
            ):
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet stages must be content-free and blocked",
                    )
                )
            result = initial_sign_packet.get("result", {})
            if (
                initial_sign_packet.get("assurance_policy", {}).get(
                    "human_work_scheduled"
                )
                is not False
                or result.get("human_decision_refs") != []
                or result.get("relation_refs") != []
                or result.get("concept_refs") != []
                or result.get("claim_refs") != []
                or result.get("graph_projection_refs") != []
                or result.get("promotion_authorized") is not False
            ):
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet created human debt or promotion output",
                    )
                )
        for evaluation_path, evaluation_plan, evaluation_validator in (
            (
                semantic_samples_path,
                semantic_samples_plan,
                source_gated_semantic_evaluation_plan_validator,
            ),
            (
                llm_tasks_path,
                llm_tasks_plan,
                source_gated_llm_evaluation_plan_validator,
            ),
        ):
            if evaluation_plan is not None:
                _validate_payload(
                    evaluation_plan,
                    evaluation_validator,
                    _relative(evaluation_path, repo_root),
                    issues,
                )
        if translation_source_review_plan is not None:
            review_location = _relative(translation_source_review_path, repo_root)
            _validate_payload(
                translation_source_review_plan,
                translation_source_review_validator,
                review_location,
                issues,
            )
            supersedes = translation_source_review_plan.get("supersedes", {})
            expected_plan_ref = _relative(translation_path, repo_root)
            if supersedes.get("v1_plan_ref") != expected_plan_ref:
                issues.append((review_location, "v2 source review plan does not cite its v1 plan"))
            if translation_path.is_file() and supersedes.get("v1_plan_sha256") != _sha256(
                translation_path
            ):
                issues.append((review_location, "v1 translation plan digest drifted"))
            inspection_ref = supersedes.get("v1_inspection_ref")
            inspection_path = repo_root / str(inspection_ref or "")
            inspection = None
            if not isinstance(inspection_ref, str) or not inspection_path.is_file():
                issues.append((review_location, "v1 selector inspection reference is unresolved"))
            else:
                if supersedes.get("v1_inspection_sha256") != _sha256(inspection_path):
                    issues.append((review_location, "v1 selector inspection digest drifted"))
                inspection = _load_json(inspection_path, repo_root, issues)

            units = translation_source_review_plan.get("units", [])
            fragments = translation_plan.get("fragments", []) if translation_plan else []
            inspection_records = inspection.get("records", []) if inspection else []
            if len(units) != len(fragments) or len(units) != len(inspection_records):
                issues.append(
                    (review_location, "v2 units do not close over all v1 fragments and inspections")
                )
            strategy_counts = {
                "confirm_visible_prose": 0,
                "first_new_unit_after_tail": 0,
                "first_prose_after_heading": 0,
                "resolve_cross_page_boundary": 0,
            }
            strategy_keys = {
                "confirm-visible-complete-prose-unit-without-reusing-v1-text": "confirm_visible_prose",
                "identify-first-new-complete-prose-unit-using-page-triplet": "first_new_unit_after_tail",
                "identify-first-prose-unit-after-visible-heading": "first_prose_after_heading",
                "resolve-cross-page-boundary-before-unit-selection": "resolve_cross_page_boundary",
            }
            seen_review_ids: set[object] = set()
            for index, (unit, fragment, inspection_record) in enumerate(
                zip(units, fragments, inspection_records, strict=False),
                start=1,
            ):
                if not all(isinstance(row, dict) for row in (unit, fragment, inspection_record)):
                    continue
                unit_id = unit.get("review_unit_id")
                if unit_id in seen_review_ids:
                    issues.append((review_location, f"duplicate review_unit_id: {unit_id}"))
                seen_review_ids.add(unit_id)
                expected_unit_id = f"tos-translation-source-review-v2-{index:03d}"
                if unit_id != expected_unit_id:
                    issues.append((review_location, f"unit {index} must be {expected_unit_id}"))
                expected_fields = {
                    "supersedes_fragment_id": fragment.get("fragment_id"),
                    "context_anchor_ref": fragment.get("source_anchor_ref"),
                    "container_member": fragment.get("container_member"),
                    "member_sha256": fragment.get("member_sha256"),
                    "v1_inspection_decision": inspection_record.get("decision"),
                }
                for key, expected in expected_fields.items():
                    if unit.get(key) != expected:
                        issues.append((review_location, f"{unit_id}.{key} drifted from v1 evidence"))
                context = unit.get("visual_context", {})
                current_page = int(fragment.get("page_member_index", -2)) + 1
                if context != {
                    "previous_pdf_page": current_page - 1,
                    "current_pdf_page": current_page,
                    "next_pdf_page": current_page + 1,
                }:
                    issues.append((review_location, f"{unit_id}.visual_context is not the exact page triplet"))

                relevant_signals = [
                    signal
                    for signal in inspection_record.get("signals", [])
                    if signal
                    in {
                        "heading-selected",
                        "page-start-tail",
                        "ocr-contamination",
                        "page-start-boundary-unverified",
                        "transcription-uncertain",
                    }
                ]
                if inspection_record.get("decision") == "accept-with-limits":
                    relevant_signals.insert(0, "candidate-usable-with-limits")
                if unit.get("v1_failure_signals") != relevant_signals:
                    issues.append((review_location, f"{unit_id}.v1_failure_signals drifted"))

                strategy_key = strategy_keys.get(unit.get("selection_instruction"))
                if strategy_key is not None:
                    strategy_counts[strategy_key] += 1
            if translation_source_review_plan.get("selection_strategy_counts") != strategy_counts:
                issues.append((review_location, "selection_strategy_counts do not match v2 units"))

            if translation_plan is not None:
                if translation_source_review_plan.get("source_witness") != {
                    "item_ref": translation_plan.get("source_item_ref"),
                    "file_ref": translation_plan.get("source_file_ref"),
                    "file_sha256": translation_plan.get("source_file_sha256"),
                    "role": "automatic-ocr-derivative-not-source-truth",
                }:
                    issues.append((review_location, "v2 source witness drifted from v1 source identity"))
                comparator = translation_source_review_plan.get("recognized_comparator", {})
                v1_comparator = translation_plan.get("recognized_comparator", {})
                for key in ("expression_ref", "item_ref", "visibility", "reveal_stage"):
                    if comparator.get(key) != v1_comparator.get(key):
                        issues.append((review_location, f"recognized comparator {key} drifted"))
        if german_assisted_review is not None:
            assisted_location = _relative(german_assisted_review_path, repo_root)
            _validate_payload(
                german_assisted_review,
                german_assisted_review_validator,
                assisted_location,
                issues,
            )
            assisted_research_path = (
                repo_root
                / "ToS/research-packets/foundation-laboratory-2026-07/"
                "GERMAN_ASSISTED_REVIEW_RESEARCH.md"
            )
            for field, expected_path in (
                ("method_research", assisted_research_path),
                ("source_review_plan", translation_source_review_path),
            ):
                digest_issue = _digest_bound_ref_issue(
                    german_assisted_review.get(field),
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=field,
                )
                if digest_issue is not None:
                    issues.append((assisted_location, digest_issue))
            source_review_units = (
                translation_source_review_plan.get("units", [])
                if isinstance(translation_source_review_plan, dict)
                else []
            )
            if german_assisted_review.get("current_state", {}).get(
                "prepared_units"
            ) != len(source_review_units):
                issues.append(
                    (
                        assisted_location,
                        "German assisted-review prepared_units differ from source-review units",
                    )
                )
            witness_packet_refs = german_assisted_review.get(
                "critical_edition_witness_packets",
                [],
            )
            for index, packet_ref in enumerate(witness_packet_refs):
                packet_repo_ref = (
                    packet_ref.get("ref")
                    if isinstance(packet_ref, dict)
                    else None
                )
                expected_path = (
                    repo_root / packet_repo_ref
                    if isinstance(packet_repo_ref, str)
                    else None
                )
                if (
                    expected_path is None
                    or expected_path not in critical_edition_witness_paths
                ):
                    issues.append(
                        (
                            assisted_location,
                            "German assisted-review names an unresolved critical-edition packet",
                        )
                    )
                    continue
                digest_issue = _digest_bound_ref_issue(
                    packet_ref,
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=f"critical_edition_witness_packets[{index}]",
                )
                if digest_issue is not None:
                    issues.append((assisted_location, digest_issue))
            prepared_packet_count = german_assisted_review.get(
                "current_state",
                {},
            ).get("prepared_critical_edition_witness_packets")
            if prepared_packet_count != len(witness_packet_refs):
                issues.append(
                    (
                        assisted_location,
                        "German assisted-review prepared critical-edition packet count drifted",
                    )
                )
        if german_source_triangulation is not None:
            triangulation_location = _relative(
                german_source_triangulation_path,
                repo_root,
            )
            _validate_payload(
                german_source_triangulation,
                german_source_triangulation_validator,
                triangulation_location,
                issues,
            )
            bindings = german_source_triangulation.get("bindings", {})
            critical_packet_path = (
                critical_edition_witness_paths[0]
                if len(critical_edition_witness_paths) == 1
                else None
            )
            for field, expected_path in (
                ("assisted_review_plan", german_assisted_review_path),
                ("source_review_plan", translation_source_review_path),
                ("metadata_critical_witness_packet", critical_packet_path),
            ):
                if expected_path is None:
                    issues.append(
                        (
                            triangulation_location,
                            f"{field} has no unique owner artifact",
                        )
                    )
                    continue
                digest_issue = _digest_bound_ref_issue(
                    bindings.get(field),
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=f"bindings.{field}",
                )
                if digest_issue is not None:
                    issues.append((triangulation_location, digest_issue))

            target = german_source_triangulation.get("target", {})
            source_review_units = (
                translation_source_review_plan.get("units", [])
                if isinstance(translation_source_review_plan, dict)
                else []
            )
            target_unit = next(
                (
                    unit
                    for unit in source_review_units
                    if isinstance(unit, dict)
                    and unit.get("review_unit_id")
                    == target.get("review_unit_id")
                ),
                None,
            )
            if target_unit is None:
                issues.append(
                    (
                        triangulation_location,
                        "triangulation target is absent from source-review plan",
                    )
                )
            elif target_unit.get("context_anchor_ref") != target.get(
                "context_anchor_ref"
            ):
                issues.append(
                    (
                        triangulation_location,
                        "triangulation target anchor drifted",
                    )
                )
            if len(critical_edition_witnesses) == 1:
                critical_target = critical_edition_witnesses[0][1].get(
                    "target",
                    {},
                )
                for field in (
                    "review_unit_id",
                    "context_anchor_ref",
                    "critical_locator_siglum",
                ):
                    if target.get(field) != critical_target.get(field):
                        issues.append(
                            (
                                triangulation_location,
                                f"triangulation target {field} drifted from metadata witness",
                            )
                        )

            inputs = german_source_triangulation.get("inputs", {})
            expected_ekgwb_local_ref = (
                gold_root
                / "local-content/translation/source-review/"
                "critical-edition-candidates/ekgwb/za-i/"
                "static-html-include.response.html"
            )
            if inputs.get("ekgwb", {}).get(
                "local_payload_ref"
            ) != _relative(expected_ekgwb_local_ref, repo_root):
                issues.append(
                    (
                        triangulation_location,
                        "triangulation eKGWB local-only payload route drifted",
                    )
                )
            for witness_name in ("dta_tei", "naumann_auto_epub"):
                witness = inputs.get(witness_name, {})
                if not isinstance(witness, dict):
                    continue
                manifest_binding = witness.get("item_manifest", {})
                rights_binding = witness.get("rights_record", {})
                manifest_ref = (
                    manifest_binding.get("ref")
                    if isinstance(manifest_binding, dict)
                    else None
                )
                rights_ref = (
                    rights_binding.get("ref")
                    if isinstance(rights_binding, dict)
                    else None
                )
                if not isinstance(manifest_ref, str):
                    issues.append(
                        (
                            triangulation_location,
                            f"{witness_name} item manifest reference is invalid",
                        )
                    )
                    continue
                manifest_path = repo_root / manifest_ref
                if not manifest_path.is_file():
                    issues.append(
                        (
                            triangulation_location,
                            f"{witness_name} item manifest is unresolved",
                        )
                    )
                    continue
                digest_issue = _digest_bound_ref_issue(
                    manifest_binding,
                    expected_path=manifest_path,
                    repo_root=repo_root,
                    field=f"inputs.{witness_name}.item_manifest",
                )
                if digest_issue is not None:
                    issues.append((triangulation_location, digest_issue))
                manifest = _load_json(manifest_path, repo_root, issues)
                if isinstance(manifest, dict):
                    try:
                        manifest_item_root = Path(
                            manifest_ref
                        ).parent.relative_to(
                            Path("ToS/source-witnesses")
                        )
                        manifest_payload_relative = Path(
                            str(
                                witness.get(
                                    "payload_relative_path",
                                    "",
                                )
                            )
                        ).relative_to(manifest_item_root).as_posix()
                    except ValueError:
                        manifest_payload_relative = None
                    matching_payloads = [
                        row
                        for row in manifest.get("payload_files", [])
                        if isinstance(row, dict)
                        and row.get("file_id") == witness.get("file_ref")
                        and row.get("sha256") == witness.get("file_sha256")
                        and row.get("relative_path")
                        == manifest_payload_relative
                    ]
                    if (
                        manifest.get("item_id") != witness.get("item_ref")
                        or len(matching_payloads) != 1
                    ):
                        issues.append(
                            (
                                triangulation_location,
                                f"{witness_name} does not close over its item manifest",
                            )
                        )
                if not isinstance(rights_ref, str):
                    issues.append(
                        (
                            triangulation_location,
                            f"{witness_name} rights reference is invalid",
                        )
                    )
                else:
                    rights_path = repo_root / rights_ref
                    if not rights_path.is_file():
                        issues.append(
                            (
                                triangulation_location,
                                f"{witness_name} rights record is unresolved",
                            )
                        )
                    else:
                        digest_issue = _digest_bound_ref_issue(
                            rights_binding,
                            expected_path=rights_path,
                            repo_root=repo_root,
                            field=f"inputs.{witness_name}.rights_record",
                        )
                        if digest_issue is not None:
                            issues.append(
                                (triangulation_location, digest_issue)
                            )
            if isinstance(german_assisted_review, dict):
                current_state = german_assisted_review.get(
                    "current_state",
                    {},
                )
                expected_unit_id = target.get("review_unit_id")
                if german_assisted_review.get("status") != "in_progress":
                    issues.append(
                        (
                            triangulation_location,
                            "machine triangulation requires in-progress assisted review",
                        )
                    )
                if current_state.get("selected_unit_ids") != [
                    expected_unit_id
                ]:
                    issues.append(
                        (
                            triangulation_location,
                            "assisted-review selected unit drifted from triangulation",
                        )
                    )
                if (
                    current_state.get("runs") != 1
                    or current_state.get("machine_triangulated_units") != 1
                ):
                    issues.append(
                        (
                            triangulation_location,
                            "assisted-review machine-run counters drifted",
                        )
                    )
                if (
                    current_state.get("accepted_german_units") != 0
                    or current_state.get("admitted_critical_edition_units") != 0
                    or current_state.get("translation_lanes_opened")
                    or current_state.get("promotion_authorized") is not False
                ):
                    issues.append(
                        (
                            triangulation_location,
                            "machine triangulation crossed a human or translation gate",
                        )
                    )
        if bounded_translation_research_input is not None:
            bounded_location = _relative(
                bounded_translation_research_input_path,
                repo_root,
            )
            _validate_payload(
                bounded_translation_research_input,
                bounded_translation_research_input_validator,
                bounded_location,
                issues,
            )
            admission_research_path = (
                repo_root
                / "ToS/research-packets/foundation-laboratory-2026-07/"
                "BOUNDED_TRANSLATION_SOURCE_ADMISSION_RESEARCH.md"
            )
            corpus_decision_path = (
                repo_root
                / "docs/decisions/"
                "TOS-D-0020-corpus-evidence-spine-and-witness-storage.md"
            )
            preexisting_canon_node_path = (
                repo_root
                / "ToS/canon/source/friedrich-nietzsche/"
                "thus-spoke-zarathustra/prologue-1/node.json"
            )
            preexisting_public_mirror_path = (
                repo_root
                / "ToS/public-compatibility/source_node.example.json"
            )
            dta_item_root = (
                repo_root
                / "ToS/source-witnesses/works/friedrich-nietzsche/"
                "also-sprach-zarathustra/expressions/"
                "de-schmeitzner-1883-part-1/editions/"
                "chemnitz-schmeitzner-1883-part-1/items/"
                "dta-sbb-corrected-tei-p5"
            )
            bindings = bounded_translation_research_input.get(
                "bindings",
                {},
            )
            for field, expected_path in (
                ("corpus_decision", corpus_decision_path),
                ("admission_research", admission_research_path),
                ("source_review_plan", translation_source_review_path),
                ("accepted_translation_plan", translation_laboratory_path),
                (
                    "german_source_triangulation",
                    german_source_triangulation_path,
                ),
                (
                    "preexisting_authored_canon_node",
                    preexisting_canon_node_path,
                ),
                (
                    "preexisting_public_compatibility_mirror",
                    preexisting_public_mirror_path,
                ),
                ("dta_item_manifest", dta_item_root / "item.manifest.json"),
                ("dta_rights_record", dta_item_root / "rights.json"),
            ):
                digest_issue = _digest_bound_ref_issue(
                    bindings.get(field),
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=f"bindings.{field}",
                )
                if digest_issue is not None:
                    issues.append((bounded_location, digest_issue))

            target = bounded_translation_research_input.get("target", {})
            source_review_units = (
                translation_source_review_plan.get("units", [])
                if isinstance(translation_source_review_plan, dict)
                else []
            )
            target_unit = next(
                (
                    unit
                    for unit in source_review_units
                    if isinstance(unit, dict)
                    and unit.get("review_unit_id")
                    == target.get("review_unit_id")
                ),
                None,
            )
            if target_unit is None:
                issues.append(
                    (
                        bounded_location,
                        "bounded input target is absent from source-review plan",
                    )
                )
            elif target_unit.get("context_anchor_ref") != target.get(
                "context_anchor_ref"
            ):
                issues.append(
                    (
                        bounded_location,
                        "bounded input target anchor drifted",
                    )
                )

            local_ref = bounded_translation_research_input.get(
                "local_artifact",
                {},
            ).get("ref")
            expected_local_path = (
                gold_root
                / "local-content/translation/research-inputs/"
                "za-i-vorrede-1-opening-sentence.v1.json"
            )
            if local_ref != _relative(expected_local_path, repo_root):
                issues.append(
                    (
                        bounded_location,
                        "bounded input local-only artifact route drifted",
                    )
                )
            elif _git_ignored(repo_root, expected_local_path) is not True:
                issues.append(
                    (
                        bounded_location,
                        "bounded input source artifact is not protected by Git ignore",
                    )
                )

            if isinstance(translation_laboratory_plan, dict):
                source_gate = translation_laboratory_plan.get(
                    "source_review_gate",
                    {},
                )
                blind_lanes = translation_laboratory_plan.get(
                    "blind_lanes",
                    {},
                )
                if (
                    source_gate.get("current_human_accepted_units") != 0
                    or source_gate.get("gate_state")
                    != "blocked-awaiting-real-human-source-review"
                    or any(
                        isinstance(lane, dict)
                        and lane.get("state") != "blocked-on-source-acceptance"
                        for lane in blind_lanes.values()
                    )
                ):
                    issues.append(
                        (
                            bounded_location,
                            "bounded calibration input altered the accepted translation plan",
                        )
                    )

            gate_effects = bounded_translation_research_input.get(
                "gate_effects",
                {},
            )
            if (
                gate_effects.get("human_debt_units") != 0
                or gate_effects.get("accepted_german_units") != 0
                or gate_effects.get("accepted_translation_lanes_opened")
                or gate_effects.get("semantic_tasks_opened") != 0
                or gate_effects.get(
                    "canon_or_graph_promotion_authorized"
                )
                is not False
            ):
                issues.append(
                    (
                        bounded_location,
                        "bounded calibration input crossed an authority gate",
                    )
                )
        for (
            critical_edition_witness_path,
            critical_edition_witness,
        ) in critical_edition_witnesses:
            witness_location = _relative(
                critical_edition_witness_path,
                repo_root,
            )
            _validate_payload(
                critical_edition_witness,
                critical_edition_witness_validator,
                witness_location,
                issues,
            )
            source_plan_issue = _digest_bound_ref_issue(
                critical_edition_witness.get("source_review_plan"),
                expected_path=translation_source_review_path,
                repo_root=repo_root,
                field="source_review_plan",
            )
            if source_plan_issue is not None:
                issues.append((witness_location, source_plan_issue))
            if critical_edition_witness.get(
                "assisted_review_plan_ref"
            ) != _relative(german_assisted_review_path, repo_root):
                issues.append(
                    (
                        witness_location,
                        "critical-edition witness does not cite its assisted-review plan",
                    )
                )
            register_binding = critical_edition_witness.get(
                "reference_register",
                {},
            )
            if register_binding.get("ref") != _relative(
                translation_reference_register_path,
                repo_root,
            ):
                issues.append(
                    (
                        witness_location,
                        "critical-edition witness does not cite its translation reference register",
                    )
                )
            registered_entries = (
                translation_reference_register.get("entries", [])
                if isinstance(translation_reference_register, dict)
                else []
            )
            registered_entry = next(
                (
                    entry
                    for entry in registered_entries
                    if isinstance(entry, dict)
                    and entry.get("reference_id")
                    == register_binding.get("reference_id")
                ),
                None,
            )
            if registered_entry is None:
                issues.append(
                    (
                        witness_location,
                        "critical-edition witness reference_id is absent from the register",
                    )
                )
            elif _relative(critical_edition_witness_path, repo_root) not in (
                registered_entry.get("tos_refs", {}).get("path_refs", [])
            ):
                issues.append(
                    (
                        witness_location,
                        "translation reference entry does not return to the critical-edition witness packet",
                    )
                )
            target = critical_edition_witness.get("target", {})
            target_unit = next(
                (
                    unit
                    for unit in (
                        translation_source_review_plan.get("units", [])
                        if isinstance(translation_source_review_plan, dict)
                        else []
                    )
                    if isinstance(unit, dict)
                    and unit.get("review_unit_id")
                    == target.get("review_unit_id")
                ),
                None,
            )
            if target_unit is None:
                issues.append(
                    (
                        witness_location,
                        "critical-edition witness target unit is absent from the source-review plan",
                    )
                )
            elif target_unit.get("context_anchor_ref") != target.get(
                "context_anchor_ref"
            ):
                issues.append(
                    (
                        witness_location,
                        "critical-edition witness context anchor drifted from its source-review unit",
                    )
                )
            for message in _critical_edition_local_structural_context_issues(
                critical_edition_witness,
                target_unit=target_unit,
                source_review_plan=translation_source_review_plan,
                ocr_sample_plan=ocr_sample_plan,
            ):
                issues.append((witness_location, message))
            if critical_edition_witness.get("status") != (
                "citation_witness_admitted"
            ) and isinstance(german_assisted_review, dict):
                state = german_assisted_review.get("current_state", {})
                if state.get("admitted_critical_edition_units") != 0:
                    issues.append(
                        (
                            witness_location,
                            "pending critical-edition witness inflated the admitted unit count",
                        )
                    )
                if state.get("translation_lanes_opened"):
                    issues.append(
                        (
                            witness_location,
                            "pending critical-edition witness opened a translation lane",
                        )
                    )
        if translation_laboratory_plan is not None:
            laboratory_location = _relative(translation_laboratory_path, repo_root)
            _validate_payload(
                translation_laboratory_plan,
                translation_laboratory_plan_validator,
                laboratory_location,
                issues,
            )
            source_gate = translation_laboratory_plan.get("source_review_gate", {})
            if source_gate.get("review_plan_ref") != _relative(
                translation_source_review_path,
                repo_root,
            ):
                issues.append(
                    (laboratory_location, "translation laboratory does not cite its v2 source-review plan")
                )
            if translation_source_review_path.is_file() and source_gate.get(
                "review_plan_sha256"
            ) != _sha256(translation_source_review_path):
                issues.append((laboratory_location, "translation source-review plan digest drifted"))
            if translation_plan is not None:
                if translation_laboratory_plan.get("work_ref") != translation_plan.get("work_ref"):
                    issues.append((laboratory_location, "translation laboratory work_ref drifted"))
                if translation_laboratory_plan.get(
                    "source_expression_ref"
                ) != translation_plan.get("source_expression_ref"):
                    issues.append(
                        (laboratory_location, "translation laboratory source_expression_ref drifted")
                    )
                laboratory_comparator = translation_laboratory_plan.get(
                    "recognized_comparator", {}
                )
                sample_comparator = translation_plan.get("recognized_comparator", {})
                for key in ("expression_ref", "item_ref", "visibility"):
                    if laboratory_comparator.get(key) != sample_comparator.get(key):
                        issues.append(
                            (laboratory_location, f"translation laboratory comparator {key} drifted")
                        )
            for ref_field in (
                "translation_packet_schema_ref",
                "semantic_ladder_schema_ref",
            ):
                ref = translation_laboratory_plan.get(ref_field)
                if not isinstance(ref, str) or not (repo_root / ref).is_file():
                    issues.append(
                        (laboratory_location, f"translation laboratory {ref_field} is unresolved")
                    )
            accepted_units = source_gate.get("current_human_accepted_units")
            if isinstance(accepted_units, int) and accepted_units < 30:
                if translation_laboratory_plan.get("status") != (
                    "frozen-blocked-on-human-source-acceptance"
                ):
                    issues.append(
                        (laboratory_location, "pre-acceptance translation laboratory is not blocked")
                    )
                for lane_name, lane in translation_laboratory_plan.get(
                    "blind_lanes", {}
                ).items():
                    if not isinstance(lane, dict) or lane.get("state") != (
                        "blocked-on-source-acceptance"
                    ):
                        issues.append(
                            (laboratory_location, f"pre-acceptance lane {lane_name} is not blocked")
                        )
                comparator = translation_laboratory_plan.get("recognized_comparator", {})
                if comparator.get("visibility") != "sealed" or any(
                    comparator.get(key) is not False
                    for key in ("content_consulted", "content_emitted")
                ):
                    issues.append(
                        (laboratory_location, "pre-acceptance comparator is not completely sealed")
                    )
        if translation_laboratory_plan is not None and translation_reference_register is None:
            issues.append(
                (
                    gold_location,
                    "translation laboratory has no translation-reference-register.v1.json",
                )
            )
        if translation_reference_register is not None:
            register_location = _relative(translation_reference_register_path, repo_root)
            _validate_payload(
                translation_reference_register,
                translation_reference_register_validator,
                register_location,
                issues,
            )

            expected_laboratory_ref = _relative(translation_laboratory_path, repo_root)
            if (
                translation_reference_register.get("laboratory_plan_ref")
                != expected_laboratory_ref
            ):
                issues.append(
                    (register_location, "translation reference register does not cite its laboratory plan")
                )
            if translation_laboratory_plan is not None and (
                translation_reference_register.get("work_ref")
                != translation_laboratory_plan.get("work_ref")
            ):
                issues.append((register_location, "translation reference register work_ref drifted"))

            entries = translation_reference_register.get("entries", [])
            required_categories = set(
                translation_reference_register.get("required_categories", [])
            )
            actual_categories: set[object] = set()
            reference_ids: set[object] = set()
            human_bibliographic_reviews = 0
            human_rights_reviews = 0
            for entry in entries:
                if not isinstance(entry, dict):
                    continue
                reference_id = entry.get("reference_id")
                if reference_id in reference_ids:
                    issues.append(
                        (register_location, f"duplicate translation reference_id: {reference_id}")
                    )
                reference_ids.add(reference_id)
                actual_categories.add(entry.get("category"))

                access = entry.get("access", {})
                if isinstance(access, dict):
                    access_state = access.get("access_state")
                    request_required = access.get(
                        "access_request_required_before_content_use"
                    )
                    contact_routes = access.get("contact_routes", [])
                    request_state = access.get("request_state")
                    if request_required is True and not contact_routes:
                        issues.append(
                            (register_location, f"{reference_id} requires access but has no contact route")
                        )
                    if request_required is True and request_state == (
                        "not-required-for-web-consultation"
                    ):
                        issues.append(
                            (register_location, f"{reference_id} suppresses its required access route")
                        )
                    if access_state in {"open-web", "open-download-candidate"} and (
                        request_required is not False
                    ):
                        issues.append(
                            (register_location, f"{reference_id} marks open consultation as request-gated")
                        )

                rights = entry.get("rights", {})
                if isinstance(rights, dict):
                    assessment = rights.get("assessment")
                    if assessment == "declared-license" and not rights.get("license_uri"):
                        issues.append(
                            (register_location, f"{reference_id} declares a license without its URI")
                        )
                    if assessment in {"in-copyright", "copyright-not-evaluated"} and (
                        rights.get("redistribution") == "authorized-with-conditions"
                        or rights.get("derivative_use") == "allowed-with-conditions"
                    ):
                        issues.append(
                            (register_location, f"{reference_id} grants reuse without a reviewed license")
                        )

                admission = entry.get("admission", {})
                if isinstance(admission, dict):
                    human_bibliographic_reviews += int(
                        admission.get("human_bibliographic_review") is True
                    )
                    human_rights_reviews += int(admission.get("human_rights_review") is True)

                tos_refs = entry.get("tos_refs", {})
                if isinstance(tos_refs, dict):
                    for record_ref in tos_refs.get("record_refs", []):
                        if not isinstance(record_ref, str):
                            continue
                        known = (
                            record_ref in rights_ids
                            if record_ref.startswith("tos.rights.")
                            else record_ref in records_by_id
                        )
                        if not known:
                            issues.append(
                                (register_location, f"{reference_id} has unresolved record ref: {record_ref}")
                            )
                    for path_ref in tos_refs.get("path_refs", []):
                        if not isinstance(path_ref, str) or not (repo_root / path_ref).is_file():
                            issues.append(
                                (register_location, f"{reference_id} has unresolved path ref: {path_ref}")
                            )

            missing_categories = sorted(required_categories - actual_categories)
            if missing_categories:
                issues.append(
                    (register_location, f"translation reference categories are missing: {missing_categories}")
                )
            coverage = translation_reference_register.get("coverage", {})
            if isinstance(coverage, dict):
                expected_coverage = {
                    "required_category_count": len(required_categories),
                    "entry_count": len(entries),
                    "all_required_categories_present": not missing_categories,
                    "content_admitted_entries": 0,
                    "human_bibliographic_reviews": human_bibliographic_reviews,
                    "human_rights_reviews": human_rights_reviews,
                    "permission_requests_sent": 0,
                }
                if coverage != expected_coverage:
                    issues.append(
                        (register_location, "translation reference coverage summary drifted")
                    )

            if translation_laboratory_plan is not None:
                comparator = translation_laboratory_plan.get("recognized_comparator", {})
                comparator_expression = comparator.get("expression_ref")
                comparator_item = comparator.get("item_ref")
                comparator_entries = [
                    entry
                    for entry in entries
                    if isinstance(entry, dict)
                    and entry.get("category") == "recognized_ru_translation_candidate"
                    and {
                        comparator_expression,
                        comparator_item,
                    }.issubset(set(entry.get("tos_refs", {}).get("record_refs", [])))
                ]
                if len(comparator_entries) != 1:
                    issues.append(
                        (
                            register_location,
                            "sealed recognized comparator must resolve to exactly one reference entry",
                        )
                    )
        if retrieval_plan is not None:
            _validate_payload(
                retrieval_plan,
                retrieval_query_validator,
                _relative(retrieval_path, repo_root),
                issues,
            )
        if visual_retrieval_plan is not None:
            visual_retrieval_location = _relative(
                visual_retrieval_path,
                repo_root,
            )
            _validate_payload(
                visual_retrieval_plan,
                visual_retrieval_plan_validator,
                visual_retrieval_location,
                issues,
            )
            issues.extend(
                (visual_retrieval_location, message)
                for message in _visual_retrieval_plan_issues(
                    visual_retrieval_plan,
                    source_query_plan=retrieval_plan,
                    source_sample_plan=sample_plan,
                    visual_sample_plan=ocr_sample_plan,
                )
            )
        if graph_query_plan is not None:
            _validate_payload(
                graph_query_plan,
                graph_query_validator,
                _relative(graph_queries_path, repo_root),
                issues,
            )

        local_event_ids: set[str] = set()
        local_events_by_id: dict[str, dict[str, Any]] = {}
        graph_events: list[dict[str, Any]] = []
        local_provenance_paths = [
            provenance_path,
            graph_provenance_path,
            transfer_provenance_path,
            evaluation_provenance_path,
        ]
        if german_source_triangulation_provenance_path.is_file():
            local_provenance_paths.append(
                german_source_triangulation_provenance_path
            )
        if bounded_translation_research_input_provenance_path.is_file():
            local_provenance_paths.append(
                bounded_translation_research_input_provenance_path
            )
        for local_provenance_path in local_provenance_paths:
            for index, event in enumerate(
                _load_jsonl(local_provenance_path, repo_root, issues), start=1
            ):
                event_location = f"{_relative(local_provenance_path, repo_root)}:{index}"
                _validate_payload(event, provenance_validator, event_location, issues)
                _validate_source_refs(repo_root, event, event_location, issues)
                event_id = event.get("event_id")
                if isinstance(event_id, str):
                    if event_id in event_ids:
                        issues.append((event_location, f"duplicate event_id: {event_id}"))
                    event_ids.add(event_id)
                    local_event_ids.add(event_id)
                    local_events_by_id[event_id] = event
                if local_provenance_path == graph_provenance_path:
                    graph_events.append(event)
        for (
            critical_edition_witness_path,
            critical_edition_witness,
        ) in critical_edition_witnesses:
            if (
                critical_edition_witness.get("provenance", {}).get(
                    "event_ref"
                )
                not in local_event_ids
            ):
                issues.append(
                    (
                        _relative(
                            critical_edition_witness_path,
                            repo_root,
                        ),
                        "critical-edition witness provenance event is absent from gold-set provenance",
                    )
                )
        if german_source_triangulation is not None:
            triangulation_event_id = (
                "tos.event.alignment.zarathustra-german-source-triangulation."
                "za-i-vorrede-1.2026-07-29"
            )
            triangulation_event = local_events_by_id.get(
                triangulation_event_id
            )
            triangulation_location = _relative(
                german_source_triangulation_path,
                repo_root,
            )
            if triangulation_event is None:
                issues.append(
                    (
                        triangulation_location,
                        "German source triangulation provenance event is absent",
                    )
                )
            else:
                expected_output = {
                    "ref": triangulation_location,
                    "role": "text-free-machine-triangulation-candidate",
                    "sha256": _sha256(german_source_triangulation_path),
                }
                if expected_output not in triangulation_event.get(
                    "outputs",
                    [],
                ):
                    issues.append(
                        (
                            triangulation_location,
                            "German source triangulation provenance output drifted",
                        )
                    )
        if bounded_translation_research_input is not None:
            bounded_event_id = (
                "tos.event.segmentation."
                "zarathustra-bounded-translation-input."
                "za-i-vorrede-1-opening-sentence.2026-07-29"
            )
            bounded_event = local_events_by_id.get(bounded_event_id)
            bounded_location = _relative(
                bounded_translation_research_input_path,
                repo_root,
            )
            if bounded_event is None:
                issues.append(
                    (
                        bounded_location,
                        "bounded translation research-input provenance event is absent",
                    )
                )
            else:
                expected_output = {
                    "ref": bounded_location,
                    "role": "text-free-bounded-local-calibration-admission",
                    "sha256": _sha256(
                        bounded_translation_research_input_path
                    ),
                }
                if expected_output not in bounded_event.get("outputs", []):
                    issues.append(
                        (
                            bounded_location,
                            "bounded translation research-input provenance output drifted",
                        )
                    )
        if initial_sign_packet is not None:
            initial_sign_location = _relative(
                initial_sign_packet_path,
                repo_root,
            )
            initial_sign_event_refs = initial_sign_packet.get(
                "provenance_event_refs",
                [],
            )
            if initial_sign_event_refs != [
                "tos.event.annotation.zarathustra-initial-sign-packet-v3.2026-07-29"
            ]:
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet provenance event identity drifted",
                    )
                )
            else:
                initial_sign_event = local_events_by_id.get(
                    initial_sign_event_refs[0]
                )
                expected_output = {
                    "ref": initial_sign_location,
                    "role": "content-free-blocked-initial-sign-packet",
                    "sha256": _sha256(initial_sign_packet_path),
                }
                if initial_sign_event is None:
                    issues.append(
                        (
                            initial_sign_location,
                            "initial sign packet provenance event is absent",
                        )
                    )
                elif expected_output not in initial_sign_event.get("outputs", []):
                    issues.append(
                        (
                            initial_sign_location,
                            "initial sign packet provenance output drifted",
                        )
                    )

        anchors_by_id: dict[str, dict[str, Any]] = {}
        for anchor_path in anchor_paths:
            for index, anchor in enumerate(_load_jsonl(anchor_path, repo_root, issues), start=1):
                anchor_location = f"{_relative(anchor_path, repo_root)}:{index}"
                _validate_payload(anchor, anchor_validator, anchor_location, issues)
                anchor_id = anchor.get("anchor_id")
                if isinstance(anchor_id, str):
                    if anchor_id in seen_anchor_ids:
                        issues.append((anchor_location, f"duplicate anchor_id: {anchor_id}"))
                    seen_anchor_ids.add(anchor_id)
                    anchors_by_id[anchor_id] = anchor
                item_ref = anchor.get("item_id")
                file_ref = anchor.get("file_id")
                if item_ref not in records_by_id:
                    issues.append((anchor_location, f"unresolved item_id: {item_ref}"))
                if file_owner_by_id.get(str(file_ref)) != item_ref:
                    issues.append((anchor_location, f"file_id {file_ref} does not belong to {item_ref}"))
                if file_digest_by_id.get(str(file_ref)) != anchor.get("file_sha256"):
                    issues.append((anchor_location, f"file_sha256 does not match manifest file {file_ref}"))
                if anchor.get("provenance_event_ref") not in local_event_ids:
                    issues.append((anchor_location, "anchor provenance_event_ref is absent from gold-set provenance"))

        sample_ids: set[str] = set()
        sample_bindings: dict[str, dict[str, Any]] = {}
        gold_sample_ids: set[str] = set()
        sample_anchor_ids: set[str] = set()
        if sample_plan is not None:
            groups = sample_plan.get("source_groups", [])
            if len(groups) != 3:
                issues.append((_relative(sample_path, repo_root), "sample plan must have exactly three source groups"))
            for group in groups:
                if not isinstance(group, dict):
                    continue
                item_ref = group.get("item_ref")
                file_ref = group.get("file_ref")
                samples = group.get("samples", [])
                edition_ref = item_edition_by_id.get(str(item_ref))
                edition_record = records_by_id.get(str(edition_ref), ({}, Path()))[0]
                group_languages = {
                    records_by_id.get(str(expression_ref), ({}, Path()))[0].get(
                        "language"
                    )
                    for expression_ref in edition_record.get(
                        "embodies_expression_refs",
                        [],
                    )
                }
                group_languages.discard(None)
                group_language = (
                    next(iter(group_languages)) if len(group_languages) == 1 else None
                )
                if file_owner_by_id.get(str(file_ref)) != item_ref:
                    issues.append((_relative(sample_path, repo_root), f"file_ref {file_ref} does not belong to {item_ref}"))
                if file_digest_by_id.get(str(file_ref)) != group.get("file_sha256"):
                    issues.append((_relative(sample_path, repo_root), f"file_sha256 differs from manifest file {file_ref}"))
                if len(samples) != 12:
                    issues.append((_relative(sample_path, repo_root), f"{item_ref} must have exactly 12 samples"))
                group_gold = 0
                for sample in samples:
                    if not isinstance(sample, dict):
                        continue
                    sample_id = sample.get("sample_id")
                    anchor_ref = sample.get("anchor_ref")
                    if not isinstance(sample_id, str) or sample_id in sample_ids:
                        issues.append((_relative(sample_path, repo_root), f"duplicate or invalid sample_id: {sample_id}"))
                    elif isinstance(sample_id, str):
                        sample_ids.add(sample_id)
                        sample_bindings[sample_id] = {
                            "sample": sample,
                            "item_ref": item_ref,
                            "file_ref": file_ref,
                            "anchor_ref": anchor_ref,
                            "language": group_language,
                        }
                    if isinstance(anchor_ref, str):
                        sample_anchor_ids.add(anchor_ref)
                    anchor = anchors_by_id.get(str(anchor_ref))
                    if anchor is None:
                        issues.append((_relative(sample_path, repo_root), f"unresolved sample anchor: {anchor_ref}"))
                    elif anchor.get("item_id") != item_ref or anchor.get("file_id") != file_ref:
                        issues.append((_relative(sample_path, repo_root), f"sample anchor {anchor_ref} crosses its source group"))
                    if sample.get("gold_candidate") is True:
                        group_gold += 1
                        if isinstance(sample_id, str):
                            gold_sample_ids.add(sample_id)
                if group_gold != 5:
                    issues.append((_relative(sample_path, repo_root), f"{item_ref} must have exactly five gold candidates"))

        if transfer_plan is not None:
            transfer_location = _relative(transfer_path, repo_root)
            source_sample_plan = transfer_plan.get("source_sample_plan", {})
            if source_sample_plan.get("ref") != _relative(sample_path, repo_root):
                issues.append(
                    (transfer_location, "transfer plan does not cite the frozen source sample plan")
                )
            elif source_sample_plan.get("sha256") != _sha256(sample_path):
                issues.append((transfer_location, "transfer source sample-plan digest drifted"))

            target_source = transfer_plan.get("target_source", {})
            target_item_ref = target_source.get("collection_item_ref")
            target_file_ref = target_source.get("file_ref")
            if file_owner_by_id.get(str(target_file_ref)) != target_item_ref:
                issues.append(
                    (transfer_location, "transfer target file does not belong to its collection item")
                )
            if file_digest_by_id.get(str(target_file_ref)) != target_source.get(
                "file_sha256"
            ):
                issues.append((transfer_location, "transfer target file digest drifted"))
            rights_record_ref = target_source.get("rights_record_ref")
            rights_record = (
                _load_json(repo_root / rights_record_ref, repo_root, issues)
                if isinstance(rights_record_ref, str)
                else None
            )
            if rights_record is not None:
                if rights_record.get("assessment_status") != target_source.get(
                    "rights_assessment_status"
                ):
                    issues.append(
                        (transfer_location, "transfer target rights assessment drifted")
                    )
                if target_item_ref not in rights_record.get("scope_refs", []):
                    issues.append(
                        (transfer_location, "transfer target rights record omits its item")
                    )

            expected_transfer_ids = {
                sample_id
                for sample_id, binding in sample_bindings.items()
                if "cross-work-transfer" in binding["sample"].get("strata", [])
            }
            actual_transfer_ids: set[str] = set()
            for unit in transfer_plan.get("scouting_units", []):
                if not isinstance(unit, dict):
                    continue
                sample_id = unit.get("sample_id")
                if not isinstance(sample_id, str) or sample_id in actual_transfer_ids:
                    issues.append(
                        (transfer_location, f"duplicate or invalid transfer sample_id: {sample_id}")
                    )
                    continue
                actual_transfer_ids.add(sample_id)
                binding = sample_bindings.get(sample_id)
                if binding is None:
                    issues.append(
                        (transfer_location, f"transfer scouting unit is not in sample plan: {sample_id}")
                    )
                    continue
                expected_fields = {
                    "anchor_ref": binding["anchor_ref"],
                    "item_ref": binding["item_ref"],
                    "file_ref": binding["file_ref"],
                    "source_review_status": binding["sample"].get(
                        "source_review_status"
                    ),
                }
                for field, expected in expected_fields.items():
                    if unit.get(field) != expected:
                        issues.append(
                            (transfer_location, f"{sample_id}.{field} drifted from sample plan")
                        )
                anchor = anchors_by_id.get(str(binding["anchor_ref"]))
                page_selectors = [
                    selector
                    for selector in (anchor or {}).get("selectors", [])
                    if isinstance(selector, dict) and selector.get("type") == "page_region"
                ]
                if len(page_selectors) != 1 or unit.get("page") != page_selectors[0].get(
                    "page"
                ):
                    issues.append(
                        (transfer_location, f"{sample_id}.page drifted from its exact anchor")
                    )
            if actual_transfer_ids != expected_transfer_ids:
                issues.append(
                    (
                        transfer_location,
                        "transfer scouting units do not close over the exact cross-work sample stratum",
                    )
                )

            variant_labels = [
                variant.get("label")
                for variant in transfer_plan.get("variants", [])
                if isinstance(variant, dict)
            ]
            if variant_labels != ["A", "B", "C"]:
                issues.append((transfer_location, "transfer variants are not ordered A/B/C"))
            expected_metrics = {
                "annotation-accuracy",
                "annotation-speed",
                "manual-correction-minutes",
                "traceable-proposal-rate",
                "hallucinated-relation-rate",
                "reusable-sign-utility",
                "zarathustra-ontology-imposition-rate",
                "machine-cost",
            }
            if set(transfer_plan.get("metrics", [])) != expected_metrics:
                issues.append((transfer_location, "transfer metric set is incomplete"))

            actual_gold_count = sum(
                unit.get("gold_status") == "human_double_checked"
                for unit in (gold_status or {}).get("units", [])
                if isinstance(unit, dict)
            )
            kernel_gate = transfer_plan.get("kernel_evidence_gate", {})
            if kernel_gate.get("human_double_checked_gold_units") != actual_gold_count:
                issues.append((transfer_location, "transfer kernel gold count drifted"))
            target_units = [
                unit
                for unit in transfer_plan.get("target_units", [])
                if isinstance(unit, dict)
            ]
            target_unit_ids = [unit.get("unit_id") for unit in target_units]
            if len(target_unit_ids) != len(set(target_unit_ids)):
                issues.append((transfer_location, "transfer target-unit identities are not unique"))
            actual_target_gold_count = sum(
                unit.get("target_gold_status") == "human_double_checked"
                for unit in target_units
            )
            if (
                kernel_gate.get("human_double_checked_target_units")
                != actual_target_gold_count
            ):
                issues.append((transfer_location, "transfer target-gold count drifted"))
            source_gate = (translation_laboratory_plan or {}).get(
                "source_review_gate", {}
            )
            if kernel_gate.get("accepted_source_units") != source_gate.get(
                "current_human_accepted_units"
            ):
                issues.append((transfer_location, "transfer accepted-source count drifted"))

            transfer_event_ref = transfer_plan.get("provenance_event_ref")
            transfer_event = local_events_by_id.get(str(transfer_event_ref))
            if transfer_event is None:
                issues.append(
                    (transfer_location, "transfer provenance_event_ref is absent from gold-set provenance")
                )
            else:
                transfer_output = next(
                    (
                        output
                        for output in transfer_event.get("outputs", [])
                        if isinstance(output, dict)
                        and output.get("ref") == transfer_location
                    ),
                    None,
                )
                if transfer_output is None:
                    issues.append((transfer_location, "transfer provenance omits its plan"))
                elif transfer_output.get("sha256") != _sha256(transfer_path):
                    issues.append((transfer_location, "transfer provenance digest drifted"))

        expected_evaluation_plans = {
            semantic_samples_path: (
                semantic_samples_plan,
                "semantic-annotation",
                "tos-semantic-annotation-v1",
            ),
            llm_tasks_path: (
                llm_tasks_plan,
                "llm-assistance",
                "tos-llm-assistance-v1",
            ),
        }
        for evaluation_path, (
            evaluation_plan,
            expected_kind,
            expected_experiment,
        ) in expected_evaluation_plans.items():
            if evaluation_plan is None:
                continue
            evaluation_location = _relative(evaluation_path, repo_root)
            if evaluation_plan.get("plan_kind") != expected_kind:
                issues.append((evaluation_location, "source-gated plan kind drifted"))
            if evaluation_plan.get("experiment_id") != expected_experiment:
                issues.append((evaluation_location, "source-gated experiment_id drifted"))

            current_accepted = (translation_laboratory_plan or {}).get(
                "source_review_gate", {}
            ).get("current_human_accepted_units")
            actual_gold_count = sum(
                unit.get("gold_status") == "human_double_checked"
                for unit in (gold_status or {}).get("units", [])
                if isinstance(unit, dict)
            )

            if (
                expected_kind == "semantic-annotation"
                and evaluation_plan.get("schema_version")
                == "tos_source_gated_evaluation_plan_v1"
            ):
                source_gate = evaluation_plan.get("source_gate", {})
                expected_source_refs = {
                    "review_plan_ref": _relative(
                        translation_source_review_path,
                        repo_root,
                    ),
                    "review_plan_sha256": _sha256(
                        translation_source_review_path
                    ),
                    "laboratory_plan_ref": _relative(
                        translation_laboratory_path,
                        repo_root,
                    ),
                    "laboratory_plan_sha256": _sha256(
                        translation_laboratory_path
                    ),
                }
                for field, expected in expected_source_refs.items():
                    if source_gate.get(field) != expected:
                        issues.append(
                            (evaluation_location, f"source gate {field} drifted")
                        )
                if (
                    source_gate.get("current_human_accepted_units")
                    != current_accepted
                ):
                    issues.append(
                        (evaluation_location, "source-gate accepted count drifted")
                    )

                human_gold_gate = evaluation_plan.get("human_gold_gate", {})
                if human_gold_gate.get("gold_status_ref") != _relative(
                    gold_status_path,
                    repo_root,
                ):
                    issues.append(
                        (evaluation_location, "human-gold status ref drifted")
                    )
                if human_gold_gate.get("gold_status_sha256") != _sha256(
                    gold_status_path
                ):
                    issues.append(
                        (evaluation_location, "human-gold status digest drifted")
                    )
                if (
                    human_gold_gate.get("current_human_double_checked_units")
                    != actual_gold_count
                ):
                    issues.append(
                        (evaluation_location, "human-gold count drifted")
                    )
            elif expected_kind == "semantic-annotation":
                tasks_value = evaluation_plan.get("tasks", [])
                tasks = tasks_value if isinstance(tasks_value, list) else []
                source_ready_count = sum(
                    isinstance(task, dict)
                    and bool(task.get("source_anchor_refs"))
                    and isinstance(task.get("accepted_source_sha256"), str)
                    and isinstance(task.get("source_review_event_ref"), str)
                    and isinstance(task.get("local_content_ref"), str)
                    and isinstance(task.get("local_content_sha256"), str)
                    for task in tasks
                )
                task_specific_source_gate = evaluation_plan.get(
                    "task_specific_source_gate",
                    {},
                )
                if (
                    task_specific_source_gate.get("current_eligible_tasks")
                    != source_ready_count
                ):
                    issues.append(
                        (
                            evaluation_location,
                            "task-specific semantic source-evidence count drifted",
                        )
                    )

                selection_contract = evaluation_plan.get(
                    "selection_contract",
                    {},
                )
                if tasks:
                    family_counts: dict[str, int] = {}
                    stratum_counts: dict[str, int] = {}
                    task_anchor_refs: set[str] = set()
                    for task in tasks:
                        if not isinstance(task, dict):
                            continue
                        family = task.get("task_family")
                        stratum = task.get("stratum")
                        if isinstance(family, str):
                            family_counts[family] = family_counts.get(family, 0) + 1
                        if isinstance(stratum, str):
                            stratum_counts[stratum] = stratum_counts.get(stratum, 0) + 1
                        task_anchor_refs.update(
                            ref
                            for ref in task.get("source_anchor_refs", [])
                            if isinstance(ref, str)
                        )
                    if family_counts != selection_contract.get(
                        "required_tasks_per_family"
                    ):
                        issues.append(
                            (
                                evaluation_location,
                                "semantic task-family counts drifted from frozen selection",
                            )
                        )
                    expected_strata = {
                        "random": selection_contract.get("random_tasks"),
                        "hard": selection_contract.get("hard_tasks"),
                    }
                    if stratum_counts != expected_strata:
                        issues.append(
                            (
                                evaluation_location,
                                "semantic task strata drifted from frozen selection",
                            )
                        )
                    prepared_anchor_refs = {
                        ref
                        for ref in evaluation_plan.get(
                            "prepared_task_contract",
                            {},
                        ).get("source_anchor_refs", [])
                        if isinstance(ref, str)
                    }
                    if prepared_anchor_refs != task_anchor_refs:
                        issues.append(
                            (
                                evaluation_location,
                                "semantic prepared anchor set differs from task anchors",
                            )
                        )

                historical_gate = evaluation_plan.get(
                    "historical_gate_snapshot",
                    {},
                )
                expected_historical_refs = {
                    "source_review_plan_ref": _relative(
                        translation_source_review_path,
                        repo_root,
                    ),
                    "source_review_plan_sha256": _sha256(
                        translation_source_review_path
                    ),
                    "laboratory_plan_ref": _relative(
                        translation_laboratory_path,
                        repo_root,
                    ),
                    "laboratory_plan_sha256": _sha256(
                        translation_laboratory_path
                    ),
                    "gold_status_ref": _relative(gold_status_path, repo_root),
                    "gold_status_sha256": _sha256(gold_status_path),
                    "observed_human_accepted_source_units": current_accepted,
                    "observed_human_double_checked_gold_units": actual_gold_count,
                    "scheduling_authority": False,
                    "execution_authority": False,
                }
                for field, expected in expected_historical_refs.items():
                    if historical_gate.get(field) != expected:
                        issues.append(
                            (
                                evaluation_location,
                                f"historical gate snapshot {field} drifted",
                            )
                        )
            else:
                tasks_value = evaluation_plan.get("tasks", [])
                tasks = tasks_value if isinstance(tasks_value, list) else []
                source_ready_count = sum(
                    isinstance(task, dict)
                    and bool(task.get("source_anchor_refs"))
                    and isinstance(task.get("accepted_source_sha256"), str)
                    and isinstance(task.get("source_review_event_ref"), str)
                    and isinstance(task.get("local_content_ref"), str)
                    and isinstance(task.get("local_content_sha256"), str)
                    for task in tasks
                )
                baseline_ready_count = sum(
                    isinstance(task, dict)
                    and isinstance(task.get("human_baseline_ref"), str)
                    and bool(task.get("human_baseline_ref"))
                    and isinstance(task.get("human_baseline_sha256"), str)
                    for task in tasks
                )
                source_evidence_gate = evaluation_plan.get(
                    "source_evidence_gate",
                    {},
                )
                if (
                    source_evidence_gate.get("current_eligible_source_units")
                    != source_ready_count
                ):
                    issues.append(
                        (
                            evaluation_location,
                            "task-specific source-evidence count drifted",
                        )
                    )
                human_baseline_gate = evaluation_plan.get(
                    "human_baseline_gate",
                    {},
                )
                if (
                    human_baseline_gate.get("current_frozen_baseline_units")
                    != baseline_ready_count
                ):
                    issues.append(
                        (
                            evaluation_location,
                            "task-specific human-baseline count drifted",
                        )
                    )

                historical_gate = evaluation_plan.get(
                    "historical_gate_snapshot",
                    {},
                )
                expected_historical_refs = {
                    "source_review_plan_ref": _relative(
                        translation_source_review_path,
                        repo_root,
                    ),
                    "source_review_plan_sha256": _sha256(
                        translation_source_review_path
                    ),
                    "laboratory_plan_ref": _relative(
                        translation_laboratory_path,
                        repo_root,
                    ),
                    "laboratory_plan_sha256": _sha256(
                        translation_laboratory_path
                    ),
                    "gold_status_ref": _relative(gold_status_path, repo_root),
                    "gold_status_sha256": _sha256(gold_status_path),
                    "observed_human_accepted_source_units": current_accepted,
                    "observed_human_double_checked_gold_units": actual_gold_count,
                    "scheduling_authority": False,
                    "execution_authority": False,
                }
                for field, expected in expected_historical_refs.items():
                    if historical_gate.get(field) != expected:
                        issues.append(
                            (
                                evaluation_location,
                                f"historical gate snapshot {field} drifted",
                            )
                        )

            event_ref = evaluation_plan.get("provenance_event_ref")
            evaluation_event = local_events_by_id.get(str(event_ref))
            if evaluation_event is None:
                issues.append(
                    (evaluation_location, "evaluation provenance_event_ref is absent")
                )
            else:
                output = next(
                    (
                        candidate
                        for candidate in evaluation_event.get("outputs", [])
                        if isinstance(candidate, dict)
                        and candidate.get("ref") == evaluation_location
                    ),
                    None,
                )
                if output is None:
                    issues.append((evaluation_location, "evaluation provenance omits plan"))
                elif output.get("sha256") != _sha256(evaluation_path):
                    issues.append((evaluation_location, "evaluation provenance digest drifted"))

        ocr_sample_ids: set[str] = set()
        ocr_plan_anchor_ids: set[str] = set()
        if ocr_sample_plan is not None:
            ocr_location = _relative(ocr_sample_path, repo_root)
            expected_projection_ref = _relative(sample_path, repo_root)
            if ocr_sample_plan.get("projection_of_plan_ref") != expected_projection_ref:
                issues.append((ocr_location, "OCR visual plan does not project the frozen general sample plan"))

            discovery_event_ref = ocr_sample_plan.get("source_discovery_event_ref")
            if discovery_event_ref not in event_ids:
                issues.append((ocr_location, "OCR source_discovery_event_ref is unresolved"))

            projection_event_ref = ocr_sample_plan.get("provenance_event_ref")
            projection_event = local_events_by_id.get(str(projection_event_ref))
            if projection_event is None:
                issues.append((ocr_location, "OCR provenance_event_ref is absent from gold-set provenance"))
            else:
                outputs_by_ref = {
                    output.get("ref"): output
                    for output in projection_event.get("outputs", [])
                    if isinstance(output, dict) and isinstance(output.get("ref"), str)
                }
                for output_path in (ocr_sample_path, ocr_anchor_path):
                    output_ref = _relative(output_path, repo_root)
                    output = outputs_by_ref.get(output_ref)
                    if output is None:
                        issues.append((ocr_location, f"OCR provenance omits output: {output_ref}"))
                    elif output.get("sha256") != _sha256(output_path):
                        issues.append((ocr_location, f"OCR provenance digest differs: {output_ref}"))

            render_ref = ocr_sample_plan.get("render_specification", {}).get("render_manifest_ref")
            render_path = repo_root / str(render_ref)
            try:
                render_path.resolve().relative_to((gold_root / "local-content").resolve())
            except ValueError:
                issues.append((ocr_location, "OCR render manifest must stay under gold-set local-content"))
            if _git_ignored(repo_root, render_path) is not True:
                issues.append((ocr_location, "planned OCR render manifest is not inside the ignored local-content lane"))

            for reference_item_ref in ocr_sample_plan.get(
                "reference_witness_reveal_law", {}
            ).get("reference_item_refs", []):
                target = records_by_id.get(str(reference_item_ref))
                if target is None or target[0].get("record_type") != "item":
                    issues.append((ocr_location, f"unresolved OCR reference witness item: {reference_item_ref}"))

            ocr_groups = ocr_sample_plan.get("source_groups", [])
            if len(ocr_groups) != 3:
                issues.append((ocr_location, "OCR visual plan must have exactly three source groups"))
            ocr_group_ids: set[str] = set()
            ocr_group_items: set[str] = set()
            replacement_count = 0
            for group in ocr_groups:
                if not isinstance(group, dict):
                    continue
                group_id = group.get("group_id")
                item_ref = group.get("item_ref")
                file_ref = group.get("file_ref")
                samples = group.get("samples", [])
                if not isinstance(group_id, str) or group_id in ocr_group_ids:
                    issues.append((ocr_location, f"duplicate or invalid OCR group_id: {group_id}"))
                else:
                    ocr_group_ids.add(group_id)
                if not isinstance(item_ref, str) or item_ref in ocr_group_items:
                    issues.append((ocr_location, f"duplicate or invalid OCR source item: {item_ref}"))
                else:
                    ocr_group_items.add(item_ref)
                if file_owner_by_id.get(str(file_ref)) != item_ref:
                    issues.append((ocr_location, f"OCR file_ref {file_ref} does not belong to {item_ref}"))
                if file_digest_by_id.get(str(file_ref)) != group.get("file_sha256"):
                    issues.append((ocr_location, f"OCR file_sha256 differs from manifest file {file_ref}"))
                if len(samples) != 12:
                    issues.append((ocr_location, f"{item_ref} must have exactly 12 OCR samples"))

                group_gold = 0
                for sample in samples:
                    if not isinstance(sample, dict):
                        continue
                    sample_id = sample.get("sample_id")
                    source_sample_id = sample.get("source_sample_id")
                    anchor_ref = sample.get("anchor_ref")
                    page = sample.get("page")
                    projection_change = sample.get("projection_change")
                    if not isinstance(sample_id, str) or sample_id in ocr_sample_ids:
                        issues.append((ocr_location, f"duplicate or invalid OCR sample_id: {sample_id}"))
                    else:
                        ocr_sample_ids.add(sample_id)
                    if isinstance(anchor_ref, str):
                        ocr_plan_anchor_ids.add(anchor_ref)
                    if sample.get("gold_candidate") is True:
                        group_gold += 1

                    anchor = anchors_by_id.get(str(anchor_ref))
                    if anchor is None:
                        issues.append((ocr_location, f"unresolved OCR anchor: {anchor_ref}"))
                    else:
                        if anchor.get("item_id") != item_ref or anchor.get("file_id") != file_ref:
                            issues.append((ocr_location, f"OCR anchor {anchor_ref} crosses its source group"))
                        page_selectors = [
                            selector
                            for selector in anchor.get("selectors", [])
                            if isinstance(selector, dict) and selector.get("type") == "page_region"
                        ]
                        if len(page_selectors) != 1:
                            issues.append((ocr_location, f"OCR anchor {anchor_ref} must have one page selector"))
                        else:
                            page_selector = page_selectors[0]
                            if page_selector.get("page") != page:
                                issues.append((ocr_location, f"OCR sample page differs from anchor: {anchor_ref}"))
                            whole_page_shape = (
                                page_selector.get("x"),
                                page_selector.get("y"),
                                page_selector.get("width"),
                                page_selector.get("height"),
                                page_selector.get("coordinate_space"),
                            )
                            if whole_page_shape != (0, 0, 1, 1, "normalized_0_1"):
                                issues.append((ocr_location, f"OCR anchor is not a full normalized page: {anchor_ref}"))

                    source_binding = sample_bindings.get(str(source_sample_id))
                    if source_binding is None:
                        issues.append((ocr_location, f"OCR sample does not resolve to a frozen source sample: {source_sample_id}"))
                        continue
                    source_anchor = anchors_by_id.get(str(source_binding.get("anchor_ref")))
                    if source_anchor is None:
                        issues.append((ocr_location, f"OCR source sample anchor is unresolved: {source_sample_id}"))
                        continue

                    if projection_change == "same_visual_unit":
                        if (
                            source_binding.get("item_ref") != item_ref
                            or source_binding.get("file_ref") != file_ref
                            or source_binding.get("anchor_ref") != anchor_ref
                        ):
                            issues.append((ocr_location, f"same_visual_unit changed source binding: {sample_id}"))
                    elif projection_change == "same_scan_page_for_epub_member":
                        if source_binding.get("item_ref") == item_ref:
                            issues.append((ocr_location, f"scan projection did not move to a distinct source item: {sample_id}"))
                        if item_edition_by_id.get(str(source_binding.get("item_ref"))) != item_edition_by_id.get(str(item_ref)):
                            issues.append((ocr_location, f"scan projection crosses edition identity: {sample_id}"))
                        member_selector = next(
                            (
                                selector
                                for selector in source_anchor.get("selectors", [])
                                if isinstance(selector, dict) and selector.get("type") == "container_member"
                            ),
                            None,
                        )
                        member_path = member_selector.get("member_path") if isinstance(member_selector, dict) else None
                        match = re.search(r"(?:^|/)page_(\d+)\.html$", str(member_path))
                        if match is None or page != int(match.group(1)) + 1:
                            issues.append((ocr_location, f"EPUB member to PDF page mapping differs: {sample_id}"))
                    elif projection_change == "replacement_for_nonvisual_unit":
                        replacement_count += 1
                        if item_edition_by_id.get(str(source_binding.get("item_ref"))) != item_edition_by_id.get(str(item_ref)):
                            issues.append((ocr_location, f"OCR replacement crosses edition identity: {sample_id}"))
                        source_members = [
                            selector.get("member_path")
                            for selector in source_anchor.get("selectors", [])
                            if isinstance(selector, dict) and selector.get("type") == "container_member"
                        ]
                        source_pages = [
                            selector
                            for selector in source_anchor.get("selectors", [])
                            if isinstance(selector, dict) and selector.get("type") == "page_region"
                        ]
                        if source_members != ["EPUB/notice.html"] or source_pages or page != 2:
                            issues.append((ocr_location, f"OCR replacement is not the declared notice-to-page-2 substitution: {sample_id}"))
                    else:
                        issues.append((ocr_location, f"unsupported OCR projection change: {projection_change}"))

                if group_gold != 5:
                    issues.append((ocr_location, f"{item_ref} must have exactly five OCR gold candidates"))

            if len(ocr_sample_ids) != 36:
                issues.append((ocr_location, "OCR visual plan must contain exactly 36 unique sample IDs"))
            declared_replacements = ocr_sample_plan.get("projection_law", {}).get("replacement_count")
            if replacement_count != declared_replacements:
                issues.append((ocr_location, "actual OCR replacement count differs from projection law"))

        if gold_status is not None:
            status_ids = {
                unit.get("sample_id")
                for unit in gold_status.get("units", [])
                if isinstance(unit, dict)
            }
            if status_ids != gold_sample_ids:
                issues.append((_relative(gold_status_path, repo_root), "gold-status units differ from frozen gold candidates"))
            for unit in gold_status.get("units", []):
                if not isinstance(unit, dict):
                    continue
                if unit.get("gold_status") == "human_double_checked":
                    if not isinstance(unit.get("content_sha256"), str):
                        issues.append((_relative(gold_status_path, repo_root), f"{unit.get('sample_id')} claims gold without a content digest"))
                    for field in ("human_pass_1", "human_pass_2"):
                        review_pass = unit.get(field, {})
                        if review_pass.get("status") != "complete":
                            issues.append((_relative(gold_status_path, repo_root), f"{unit.get('sample_id')} claims gold without complete {field}"))
                        for evidence_field in ("maker_ref", "completed_at", "receipt_ref"):
                            if not isinstance(review_pass.get(evidence_field), str):
                                issues.append((_relative(gold_status_path, repo_root), f"{unit.get('sample_id')} claims gold without {field}.{evidence_field}"))

        if gold_assurance is not None:
            assurance_location = _relative(gold_assurance_path, repo_root)
            method_research_path = (
                repo_root
                / "ToS/research-packets/foundation-laboratory-2026-07/HUMAN_ASSURANCE_RESEARCH.md"
            )
            for field, expected_path in (
                ("legacy_gold_status", gold_status_path),
                ("sample_plan", sample_path),
                ("method_research", method_research_path),
            ):
                digest_issue = _digest_bound_ref_issue(
                    gold_assurance.get(field),
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=field,
                )
                if digest_issue is not None:
                    issues.append((assurance_location, digest_issue))

            assurance_units = {
                unit.get("sample_id"): unit
                for unit in gold_assurance.get("units", [])
                if isinstance(unit, dict) and isinstance(unit.get("sample_id"), str)
            }
            if set(assurance_units) != gold_sample_ids:
                issues.append(
                    (
                        assurance_location,
                        "gold-assurance units differ from frozen gold candidates",
                    )
                )

            human_work_schedule = gold_assurance.get("human_work_schedule", {})
            for message in _human_work_schedule_issues(
                assurance_units, human_work_schedule
            ):
                issues.append((assurance_location, message))
            for field in ("runtime_closure", "runtime_receipt", "source_autosave"):
                local_ref = human_work_schedule.get(field, {})
                local_path = _owner_local_evidence_path(local_ref)
                if local_path is not None and local_path.is_file():
                    expected_digest = local_ref.get("sha256")
                    if _sha256(local_path) != expected_digest:
                        issues.append(
                            (
                                assurance_location,
                                f"{field} local evidence digest drifted",
                            )
                        )

            language_scopes = {
                scope.get("language"): scope
                for scope in gold_assurance.get("language_scopes", [])
                if isinstance(scope, dict) and isinstance(scope.get("language"), str)
            }
            if len(language_scopes) != len(gold_assurance.get("language_scopes", [])):
                issues.append((assurance_location, "language scopes must be unique"))
            for scope in language_scopes.values():
                overlap_issue = _language_scope_overlap_issue(scope)
                if overlap_issue is not None:
                    issues.append((assurance_location, overlap_issue))

            minimum_delay = gold_assurance.get("solo_recheck_policy", {}).get(
                "minimum_delay_hours"
            )
            legacy_units = {
                unit.get("sample_id"): unit
                for unit in (gold_status or {}).get("units", [])
                if isinstance(unit, dict) and isinstance(unit.get("sample_id"), str)
            }
            observed_languages: set[str] = set()
            for sample_id, unit in assurance_units.items():
                binding = sample_bindings.get(sample_id, {})
                expected_anchor = binding.get("anchor_ref")
                expected_language = binding.get("language")
                if unit.get("anchor_ref") != expected_anchor:
                    issues.append(
                        (
                            assurance_location,
                            f"{sample_id} assurance anchor differs from the frozen sample",
                        )
                    )
                if unit.get("language") != expected_language:
                    issues.append(
                        (
                            assurance_location,
                            f"{sample_id} assurance language differs from its source group",
                        )
                    )
                if isinstance(expected_language, str):
                    observed_languages.add(expected_language)
                language_scope = language_scopes.get(unit.get("language"))
                if language_scope is None:
                    issues.append(
                        (
                            assurance_location,
                            f"{sample_id} has no language competence scope",
                        )
                    )
                elif unit.get("competence_level") != language_scope.get(
                    "planned_competence"
                ):
                    issues.append(
                        (
                            assurance_location,
                            f"{sample_id} competence differs from its language scope",
                        )
                    )

                assurance = unit.get("current_assurance")
                evidence = unit.get("review_evidence", {})
                if assurance in {"unreviewed", "language_competence_blocked"}:
                    if any(value is not None for value in evidence.values()):
                        issues.append(
                            (
                                assurance_location,
                                f"{sample_id} has review evidence before review",
                            )
                        )
                if assurance == "language_competence_blocked":
                    if (
                        language_scope is None
                        or language_scope.get("text_review_status")
                        != "language_competence_blocked"
                    ):
                        issues.append(
                            (
                                assurance_location,
                                f"{sample_id} language block is absent from its scope",
                            )
                        )
                if assurance == "solo_human_delayed_rechecked":
                    delay_issue = _solo_recheck_delay_issue(unit, minimum_delay)
                    if delay_issue is not None:
                        issues.append((assurance_location, delay_issue))
                    if evidence.get("same_reviewer") is not True:
                        issues.append(
                            (
                                assurance_location,
                                f"{sample_id} solo recheck must disclose the same reviewer",
                            )
                        )
                if (
                    unit.get("reference_use") == "independent_multi_human_gold"
                    and assurance != "independent_multi_human_adjudicated"
                ):
                    issues.append(
                        (
                            assurance_location,
                            f"{sample_id} claims independent multi-human gold without adjudication",
                        )
                    )
                reference_use_issue = _assurance_reference_use_issue(unit)
                if reference_use_issue is not None:
                    issues.append((assurance_location, reference_use_issue))
                if (
                    legacy_units.get(sample_id, {}).get("gold_status")
                    == "human_double_checked"
                    and assurance in {"unreviewed", "language_competence_blocked"}
                ):
                    issues.append(
                        (
                            assurance_location,
                            f"{sample_id} assurance does not carry forward legacy human review",
                        )
                    )

            if set(language_scopes) != observed_languages:
                issues.append(
                    (
                        assurance_location,
                        "language scopes differ from the frozen gold-unit languages",
                    )
                )

        if translation_plan is not None:
            fragment_ids: set[str] = set()
            fragment_anchor_ids: set[str] = set()
            fragments = translation_plan.get("fragments", [])
            if len(fragments) != 30:
                issues.append((_relative(translation_path, repo_root), "translation plan must freeze exactly 30 fragments"))
            for fragment in fragments:
                if not isinstance(fragment, dict):
                    continue
                fragment_id = fragment.get("fragment_id")
                anchor_ref = fragment.get("source_anchor_ref")
                if not isinstance(fragment_id, str) or fragment_id in fragment_ids:
                    issues.append((_relative(translation_path, repo_root), f"duplicate or invalid fragment_id: {fragment_id}"))
                elif isinstance(fragment_id, str):
                    fragment_ids.add(fragment_id)
                if isinstance(anchor_ref, str):
                    fragment_anchor_ids.add(anchor_ref)
                anchor = anchors_by_id.get(str(anchor_ref))
                if anchor is None:
                    issues.append((_relative(translation_path, repo_root), f"unresolved translation anchor: {anchor_ref}"))
                    continue
                member_selector = next(
                    (
                        selector
                        for selector in anchor.get("selectors", [])
                        if isinstance(selector, dict) and selector.get("type") == "container_member"
                    ),
                    None,
                )
                if not isinstance(member_selector, dict):
                    issues.append((_relative(translation_path, repo_root), f"translation anchor lacks member selector: {anchor_ref}"))
                elif (
                    member_selector.get("member_path") != fragment.get("container_member")
                    or member_selector.get("member_sha256") != fragment.get("member_sha256")
                ):
                    issues.append((_relative(translation_path, repo_root), f"fragment metadata differs from anchor: {anchor_ref}"))

        retrieval_anchor_ids: set[str] = set()
        if retrieval_plan is not None:
            retrieval_location = _relative(retrieval_path, repo_root)
            if retrieval_plan.get("provenance_event_ref") not in local_event_ids:
                issues.append((retrieval_location, "retrieval provenance_event_ref is absent from gold-set provenance"))
            query_ids: set[str] = set()
            for query in retrieval_plan.get("queries", []):
                if not isinstance(query, dict):
                    continue
                query_id = query.get("query_id")
                if not isinstance(query_id, str) or query_id in query_ids:
                    issues.append((retrieval_location, f"duplicate or invalid query_id: {query_id}"))
                elif isinstance(query_id, str):
                    query_ids.add(query_id)
                for field in ("expected_source_anchor_refs", "hard_negative_anchor_refs"):
                    for anchor_ref in query.get(field, []):
                        if isinstance(anchor_ref, str):
                            retrieval_anchor_ids.add(anchor_ref)
                        if anchor_ref not in anchors_by_id:
                            issues.append((retrieval_location, f"unresolved retrieval anchor: {anchor_ref}"))
            if len(query_ids) != 20:
                issues.append((retrieval_location, "retrieval query plan must contain exactly 20 unique IDs"))

            content_ref = retrieval_plan.get("query_content_ref")
            content_path = gold_root / str(content_ref)
            try:
                content_path.resolve().relative_to((gold_root / "local-content").resolve())
            except ValueError:
                issues.append((retrieval_location, "query content must stay under gold-set local-content"))
            query_content = (
                _load_json(content_path, repo_root, issues)
                if content_path.is_file()
                else None
            )
            if query_content is None and require_local_payloads and not content_path.is_file():
                issues.append((_relative(content_path, repo_root), "required local retrieval query content is missing"))
            if query_content is not None:
                if _sha256(content_path) != retrieval_plan.get("query_content_sha256"):
                    issues.append((_relative(content_path, repo_root), "query content digest differs from retrieval plan"))
                if query_content.get("frozen_before_variant_outputs") is not True:
                    issues.append((_relative(content_path, repo_root), "query content was not frozen before outputs"))
                content_ids = {
                    query.get("query_id")
                    for query in query_content.get("queries", [])
                    if isinstance(query, dict)
                }
                if content_ids != query_ids or len(query_content.get("queries", [])) != 20:
                    issues.append((_relative(content_path, repo_root), "local query IDs differ from tracked retrieval plan"))
            if _git_ignored(repo_root, content_path) is not True:
                issues.append((_relative(content_path, repo_root), "local retrieval query content is not ignored"))

        if visual_retrieval_plan is not None:
            visual_retrieval_location = _relative(
                visual_retrieval_path,
                repo_root,
            )
            query_projection = visual_retrieval_plan.get(
                "query_projection",
                {},
            )
            expected_query_plan_ref = _relative(retrieval_path, repo_root)
            expected_query_content_ref = _relative(
                gold_root / "local-content/retrieval/queries.v1.json",
                repo_root,
            )
            if query_projection.get(
                "source_query_plan_ref"
            ) != expected_query_plan_ref:
                issues.append(
                    (
                        visual_retrieval_location,
                        "visual retrieval source query-plan reference drifted",
                    )
                )
            elif query_projection.get(
                "source_query_plan_sha256"
            ) != _sha256(retrieval_path):
                issues.append(
                    (
                        visual_retrieval_location,
                        "visual retrieval source query-plan digest drifted",
                    )
                )
            if query_projection.get(
                "query_content_ref"
            ) != expected_query_content_ref:
                issues.append(
                    (
                        visual_retrieval_location,
                        "visual retrieval local query-content reference drifted",
                    )
                )
            if retrieval_plan is not None and query_projection.get(
                "query_content_sha256"
            ) != retrieval_plan.get("query_content_sha256"):
                issues.append(
                    (
                        visual_retrieval_location,
                        "visual retrieval query-content digest differs from the text retrieval plan",
                    )
                )

            crosswalk = query_projection.get(
                "source_to_visual_anchor_crosswalk",
                {},
            )
            expected_sample_ref = _relative(sample_path, repo_root)
            expected_visual_sample_ref = _relative(
                ocr_sample_path,
                repo_root,
            )
            expected_visual_sample_digest = (
                _sha256(ocr_sample_path)
                if ocr_sample_path.is_file()
                else None
            )
            expected_crosswalk_bindings = {
                "source_sample_plan_ref": expected_sample_ref,
                "visual_sample_plan_ref": expected_visual_sample_ref,
                "visual_sample_plan_sha256": expected_visual_sample_digest,
            }
            for field, expected in expected_crosswalk_bindings.items():
                if crosswalk.get(field) != expected:
                    issues.append(
                        (
                            visual_retrieval_location,
                            f"visual retrieval crosswalk {field} drifted",
                        )
                    )

            page_image_corpus = visual_retrieval_plan.get(
                "page_image_corpus",
                {},
            )
            expected_render_ref = (
                ocr_sample_plan.get("render_specification", {}).get(
                    "render_manifest_ref"
                )
                if isinstance(ocr_sample_plan, dict)
                else None
            )
            expected_page_bindings = {
                "visual_sample_plan_ref": expected_visual_sample_ref,
                "visual_sample_plan_sha256": expected_visual_sample_digest,
                "render_manifest_ref": expected_render_ref,
            }
            for field, expected in expected_page_bindings.items():
                if page_image_corpus.get(field) != expected:
                    issues.append(
                        (
                            visual_retrieval_location,
                            f"visual retrieval page-image corpus {field} drifted",
                        )
                    )

            render_manifest_path = repo_root / str(expected_render_ref or "")
            if render_manifest_path.is_file() and page_image_corpus.get(
                "render_manifest_sha256"
            ) != _sha256(render_manifest_path):
                issues.append(
                    (
                        visual_retrieval_location,
                        "visual retrieval ignored render-manifest digest drifted",
                    )
                )

            artifact_root = Path(
                os.environ.get(
                    "ABYSS_MACHINE_ARTIFACT_ROOT",
                    "/srv/abyss-machine/storage/artifacts",
                )
            )
            render_artifact_ref = page_image_corpus.get(
                "render_artifact_ref"
            )
            render_artifact_relative = (
                Path(render_artifact_ref)
                if isinstance(render_artifact_ref, str)
                else None
            )
            if (
                render_artifact_relative is None
                or render_artifact_relative.is_absolute()
                or ".." in render_artifact_relative.parts
            ):
                issues.append(
                    (
                        visual_retrieval_location,
                        "visual retrieval owner artifact reference is unsafe",
                    )
                )
            else:
                render_artifact_path = (
                    artifact_root / render_artifact_relative
                )
                if render_artifact_path.is_file() and page_image_corpus.get(
                    "render_manifest_sha256"
                ) != _sha256(render_artifact_path):
                    issues.append(
                        (
                            visual_retrieval_location,
                            "visual retrieval owner render-manifest digest drifted",
                        )
                    )

            for control in visual_retrieval_plan.get("fixed_controls", []):
                if not isinstance(control, dict):
                    continue
                run_ref = control.get("run_ref")
                run_relative = Path(run_ref) if isinstance(run_ref, str) else None
                if (
                    run_relative is None
                    or run_relative.is_absolute()
                    or ".." in run_relative.parts
                ):
                    issues.append(
                        (
                            visual_retrieval_location,
                            f"visual retrieval control {control.get('label')} artifact reference is unsafe",
                        )
                    )
                    continue
                run_path = artifact_root / run_relative
                receipt_path = run_path / "run.receipt.json"
                if run_path.exists() and not receipt_path.is_file():
                    issues.append(
                        (
                            visual_retrieval_location,
                            f"visual retrieval control {control.get('label')} run receipt is missing",
                        )
                    )
                elif receipt_path.is_file() and control.get(
                    "run_receipt_sha256"
                ) != _sha256(receipt_path):
                    issues.append(
                        (
                            visual_retrieval_location,
                            f"visual retrieval control {control.get('label')} receipt digest drifted",
                        )
                    )

            visual_event_ref = visual_retrieval_plan.get(
                "provenance_event_ref"
            )
            visual_event = local_events_by_id.get(str(visual_event_ref))
            expected_visual_inputs = {
                (
                    expected_query_plan_ref,
                    "frozen-text-retrieval-query-and-anchor-plan",
                    _sha256(retrieval_path),
                ),
                (
                    expected_sample_ref,
                    "frozen-source-sample-plan",
                    _sha256(sample_path),
                ),
                (
                    expected_visual_sample_ref,
                    "frozen-page-image-sample-projection",
                    expected_visual_sample_digest,
                ),
            }
            expected_visual_outputs = {
                (
                    _relative(
                        repo_root / VISUAL_RETRIEVAL_PLAN_SCHEMA,
                        repo_root,
                    ),
                    "direct-page-image-retrieval-plan-contract",
                    _sha256(repo_root / VISUAL_RETRIEVAL_PLAN_SCHEMA),
                ),
                (
                    visual_retrieval_location,
                    "frozen-direct-page-image-retrieval-plan",
                    _sha256(visual_retrieval_path),
                ),
            }
            if visual_event is None:
                issues.append(
                    (
                        visual_retrieval_location,
                        "visual retrieval provenance_event_ref is absent",
                    )
                )
            else:
                actual_visual_inputs = {
                    (
                        input_record.get("ref"),
                        input_record.get("role"),
                        input_record.get("sha256"),
                    )
                    for input_record in visual_event.get("inputs", [])
                    if isinstance(input_record, dict)
                }
                actual_visual_outputs = {
                    (
                        output.get("ref"),
                        output.get("role"),
                        output.get("sha256"),
                    )
                    for output in visual_event.get("outputs", [])
                    if isinstance(output, dict)
                }
                if not expected_visual_inputs.issubset(actual_visual_inputs):
                    issues.append(
                        (
                            visual_retrieval_location,
                            "visual retrieval provenance inputs drifted",
                        )
                    )
                if not expected_visual_outputs.issubset(
                    actual_visual_outputs
                ):
                    issues.append(
                        (
                            visual_retrieval_location,
                            "visual retrieval provenance outputs drifted",
                        )
                    )

        graph_claim_records = _load_jsonl(graph_claims_path, repo_root, issues)
        graph_claim_ids: set[str] = set()
        graph_predicates: set[str] = set()
        graph_anchor_ids: set[str] = set()
        for index, claim in enumerate(graph_claim_records, start=1):
            claim_location = f"{_relative(graph_claims_path, repo_root)}:{index}"
            _validate_payload(claim, claim_validator, claim_location, issues)
            claim_id = claim.get("claim_id")
            if not isinstance(claim_id, str) or claim_id in graph_claim_ids:
                issues.append((claim_location, f"duplicate or invalid graph claim_id: {claim_id}"))
            else:
                graph_claim_ids.add(claim_id)
                if claim_id in claim_ids:
                    issues.append((claim_location, f"duplicate claim_id: {claim_id}"))
                claim_ids.add(claim_id)
            predicate = claim.get("predicate")
            if isinstance(predicate, str):
                graph_predicates.add(predicate)
            for ref in [claim.get("subject_ref"), claim.get("object"), *claim.get("evidence_refs", [])]:
                if isinstance(ref, str) and ref.startswith("tos.anchor."):
                    graph_anchor_ids.add(ref)
            if claim.get("provenance_event_ref") not in local_event_ids:
                issues.append((claim_location, "graph claim provenance event is absent from the gold set"))
            for evidence_ref in claim.get("evidence_refs", []):
                if isinstance(evidence_ref, str) and evidence_ref.startswith("ToS/"):
                    if not (repo_root / evidence_ref).exists():
                        issues.append((claim_location, f"graph claim evidence path is missing: {evidence_ref}"))

        known_graph_refs = (
            set(records_by_id)
            | set(anchors_by_id)
            | event_ids
            | rights_ids
            | graph_claim_ids
        )
        for index, claim in enumerate(graph_claim_records, start=1):
            claim_location = f"{_relative(graph_claims_path, repo_root)}:{index}"
            subject_ref = claim.get("subject_ref")
            if isinstance(subject_ref, str) and subject_ref.startswith("tos."):
                if subject_ref not in known_graph_refs:
                    issues.append((claim_location, f"unresolved graph claim subject_ref: {subject_ref}"))
            object_ref = claim.get("object")
            if isinstance(object_ref, str) and object_ref.startswith("tos."):
                if object_ref not in known_graph_refs:
                    issues.append((claim_location, f"unresolved graph claim object: {object_ref}"))
            for alternative_ref in claim.get("alternative_claim_refs", []):
                if alternative_ref not in graph_claim_ids:
                    issues.append((claim_location, f"unresolved graph alternative claim: {alternative_ref}"))

        graph_query_ids: set[str] = set()
        if graph_query_plan is not None:
            graph_location = _relative(graph_queries_path, repo_root)
            expected_claim_set_ref = _relative(graph_claims_path, repo_root)
            if graph_query_plan.get("claim_set_ref") != expected_claim_set_ref:
                issues.append((graph_location, "graph query plan claim_set_ref does not own graph-claims.jsonl"))
            if _sha256(graph_claims_path) != graph_query_plan.get("claim_set_sha256"):
                issues.append((graph_location, "graph claim-set digest differs from graph query plan"))
            if set(graph_query_plan.get("graph_layers", [])) != {
                "bibliographic",
                "textual",
                "provenance",
                "interpretive",
            }:
                issues.append((graph_location, "graph query plan must keep exactly four logical layers"))
            if set(graph_query_plan.get("allowed_predicates", [])) != graph_predicates:
                issues.append((graph_location, "allowed graph predicates differ from frozen claims"))
            for query in graph_query_plan.get("queries", []):
                if not isinstance(query, dict):
                    continue
                query_id = query.get("query_id")
                if not isinstance(query_id, str) or query_id in graph_query_ids:
                    issues.append((graph_location, f"duplicate or invalid graph query_id: {query_id}"))
                else:
                    graph_query_ids.add(query_id)
                missing_claims = sorted(set(query.get("expected_claim_refs", [])) - graph_claim_ids)
                if missing_claims:
                    issues.append((graph_location, f"graph query references claims outside frozen set: {missing_claims}"))
                operation = query.get("operation")
                parameters = query.get("parameters", {})
                if operation == "claim_family":
                    family_root_ref = parameters.get("seed_claim_ref") if isinstance(parameters, dict) else None
                    if family_root_ref not in graph_claim_ids:
                        issues.append((graph_location, f"graph claim-family root is unresolved: {family_root_ref}"))
            if len(graph_query_ids) != 10:
                issues.append((graph_location, "graph query plan must contain exactly 10 unique IDs"))

        for event in graph_events:
            event_location = _relative(graph_provenance_path, repo_root)
            for output in event.get("outputs", []):
                if not isinstance(output, dict):
                    continue
                ref = output.get("ref")
                digest = output.get("sha256")
                if isinstance(ref, str) and ref.startswith("ToS/"):
                    output_path = repo_root / ref
                    if not output_path.is_file():
                        issues.append((event_location, f"graph provenance output is missing: {ref}"))
                    elif isinstance(digest, str) and _sha256(output_path) != digest:
                        issues.append((event_location, f"graph provenance output digest differs: {ref}"))

        referenced_anchor_ids = sample_anchor_ids | ocr_plan_anchor_ids | retrieval_anchor_ids | graph_anchor_ids | {
            fragment.get("source_anchor_ref")
            for fragment in (translation_plan or {}).get("fragments", [])
            if isinstance(fragment, dict)
        }
        unreferenced = sorted(set(anchors_by_id) - referenced_anchor_ids)
        if unreferenced:
            issues.append((gold_location, f"unreferenced anchors: {unreferenced}"))

        local_content = gold_root / "local-content"
        route_card = local_content / "README.md"
        if not route_card.is_file():
            issues.append((_relative(route_card, repo_root), "local-content route card is missing"))
        elif _git_ignored(repo_root, route_card) is True:
            issues.append((_relative(route_card, repo_root), "local-content route card must remain tracked"))
        if local_content.is_dir():
            for local_path in local_content.rglob("*"):
                if local_path.is_file() and local_path != route_card:
                    if _git_ignored(repo_root, local_path) is not True:
                        issues.append((_relative(local_path, repo_root), "restricted local-content file is not ignored"))

    boundary_event_paths = [
        repo_root / SOURCE_ROOT / "access-requests/provenance.jsonl",
        repo_root / SOURCE_ROOT / "server-import/provenance.jsonl",
    ]
    boundary_events_by_id: dict[str, dict[str, Any]] = {}
    for provenance_path in boundary_event_paths:
        for index, event in enumerate(
            _load_jsonl(provenance_path, repo_root, issues),
            start=1,
        ):
            location = f"{_relative(provenance_path, repo_root)}:{index}"
            _validate_payload(event, provenance_validator, location, issues)
            _validate_source_refs(repo_root, event, location, issues)
            event_id = event.get("event_id")
            if isinstance(event_id, str):
                if event_id in event_ids:
                    issues.append((location, f"duplicate event_id: {event_id}"))
                event_ids.add(event_id)
                boundary_events_by_id[event_id] = event

    discovery_root = repo_root / SOURCE_ROOT / "discovery/runs"
    for path in sorted(discovery_root.glob("*.json")):
        payload = _load_json(path, repo_root, issues)
        if payload is None:
            continue
        location = _relative(path, repo_root)
        _validate_payload(payload, material_discovery_validator, location, issues)
        for message in _discovery_decision_issues(payload):
            issues.append((location, message))
        channels = [item for item in payload.get("channels", []) if isinstance(item, dict)]
        channel_ids = [item.get("channel_id") for item in channels]
        sequences = [item.get("sequence") for item in channels]
        if len(channel_ids) != len(set(channel_ids)):
            issues.append((location, "discovery channel IDs are not unique"))
        if len(sequences) != len(set(sequences)):
            issues.append((location, "discovery channel sequence values are not unique"))
        general_web_sequences = [
            item.get("sequence")
            for item in channels
            if item.get("channel_type") == "general-web-search"
        ]
        if general_web_sequences and max(sequences, default=0) != max(general_web_sequences):
            issues.append((location, "general web search is not the final discovery channel"))
        result_ids: set[str] = set()
        for channel in channels:
            ranks = [
                result.get("rank")
                for result in channel.get("results", [])
                if isinstance(result, dict)
            ]
            if ranks != list(range(1, len(ranks) + 1)):
                issues.append(
                    (
                        location,
                        f"discovery result order for {channel.get('channel_id')} is not contiguous from rank 1",
                    )
                )
            result_ids.update(
                result.get("result_id")
                for result in channel.get("results", [])
                if isinstance(result, dict) and isinstance(result.get("result_id"), str)
            )
        selected = set(payload.get("selected_result_ids", []))
        rejected = set(payload.get("rejected_result_ids", []))
        if selected & rejected:
            issues.append((location, "discovery result is both selected and rejected"))
        unresolved_results = (selected | rejected) - result_ids
        if unresolved_results:
            issues.append((location, f"discovery decision references unknown results: {sorted(unresolved_results)}"))
        comparison_ids = {
            item.get("channel_id")
            for item in payload.get("channel_comparison", [])
            if isinstance(item, dict)
        }
        if comparison_ids != set(channel_ids):
            issues.append((location, "discovery channel comparison does not cover the exact channel set"))

    access_root = repo_root / SOURCE_ROOT / "access-requests"
    for path in sorted((access_root / "public-ledger").glob("*.json")):
        payload = _load_json(path, repo_root, issues)
        if payload is None:
            continue
        location = _relative(path, repo_root)
        _validate_payload(payload, access_request_validator, location, issues)
        for event_ref in payload.get("provenance_event_refs", []):
            if event_ref not in event_ids:
                issues.append((location, f"unresolved access-request provenance event: {event_ref}"))
    private_root = access_root / "private"
    private_route_card = private_root / "README.md"
    if not private_route_card.is_file():
        issues.append((_relative(private_route_card, repo_root), "private correspondence route card is missing"))
    elif _git_ignored(repo_root, private_route_card) is True:
        issues.append((_relative(private_route_card, repo_root), "private correspondence route card must remain tracked"))
    if private_root.is_dir():
        for private_path in private_root.rglob("*"):
            if private_path.is_file() and private_path != private_route_card:
                if _git_ignored(repo_root, private_path) is not True:
                    issues.append((_relative(private_path, repo_root), "private correspondence file is not ignored"))

    manifest_paths = sorted((repo_root / SOURCE_ROOT).rglob("item.manifest.json"))
    expected_manifest_refs = {_relative(path, repo_root) for path in manifest_paths}
    planned_manifest_refs: set[str] = set()
    for path in sorted((repo_root / SOURCE_ROOT / "server-import/plans").glob("*.json")):
        payload = _load_json(path, repo_root, issues)
        if payload is None:
            continue
        location = _relative(path, repo_root)
        _validate_payload(payload, server_import_validator, location, issues)
        manifest_evidence = payload.get("manifest", {})
        manifest_ref = manifest_evidence.get("ref") if isinstance(manifest_evidence, dict) else None
        if isinstance(manifest_ref, str):
            planned_manifest_refs.add(manifest_ref)
            manifest_path = repo_root / manifest_ref
            manifest = _load_json(manifest_path, repo_root, issues)
            if manifest is not None:
                if _sha256(manifest_path) != manifest_evidence.get("sha256"):
                    issues.append((location, "server plan manifest digest drifted"))
                if manifest.get("item_id") != payload.get("item_ref"):
                    issues.append((location, "server plan item_ref differs from item manifest"))
                expected_payload_files = [
                    {
                        "file_ref": entry.get("file_id"),
                        "relative_path": entry.get("relative_path"),
                        "byte_size": entry.get("byte_size"),
                        "sha256": entry.get("sha256"),
                        "verified": True,
                    }
                    for entry in manifest.get("payload_files", [])
                    if isinstance(entry, dict)
                ]
                if payload.get("payload_files") != expected_payload_files:
                    issues.append((location, "server plan payload inventory differs from item manifest"))
                rights_policy = payload.get("rights_policy", {})
                rights_ref = rights_policy.get("rights_record_ref") if isinstance(rights_policy, dict) else None
                if rights_ref != manifest.get("rights_ref"):
                    issues.append((location, "server plan rights record differs from item manifest"))
                elif isinstance(rights_ref, str):
                    rights_path = repo_root / rights_ref
                    if not rights_path.is_file():
                        issues.append((location, f"server plan rights record is missing: {rights_ref}"))
                    elif _sha256(rights_path) != rights_policy.get("rights_record_sha256"):
                        issues.append((location, "server plan rights-record digest drifted"))
        for event_ref in payload.get("provenance_event_refs", []):
            event = boundary_events_by_id.get(event_ref)
            if event is None:
                issues.append((location, f"unresolved server-plan provenance event: {event_ref}"))
                continue
            if payload.get("contract_version", 1) < 2:
                continue
            actual_outputs = {
                (entry.get("ref"), entry.get("sha256"))
                for entry in event.get("outputs", [])
                if isinstance(entry, dict)
            }
            expected_output = (location, _sha256(path))
            if expected_output not in actual_outputs:
                issues.append(
                    (
                        location,
                        f"server-plan provenance output digest drifted: {event_ref}",
                    )
                )
            rights_policy = payload.get("rights_policy", {})
            expected_inputs = {
                (
                    manifest_evidence.get("ref"),
                    manifest_evidence.get("sha256"),
                ),
                (
                    rights_policy.get("rights_record_ref"),
                    rights_policy.get("rights_record_sha256"),
                ),
            }
            actual_inputs = {
                (entry.get("ref"), entry.get("sha256"))
                for entry in event.get("inputs", [])
                if isinstance(entry, dict)
            }
            if not expected_inputs.issubset(actual_inputs):
                issues.append(
                    (
                        location,
                        f"server-plan provenance input digests drifted: {event_ref}",
                    )
                )
    if planned_manifest_refs != expected_manifest_refs:
        issues.append(
            (
                _relative(repo_root / SOURCE_ROOT / "server-import/plans", repo_root),
                "server plan coverage differs from the exact current item-manifest set",
            )
        )

    boundary_anchor_ids: set[str] = set()
    boundary_maps: list[tuple[dict[str, Any], Path]] = []
    boundary_map_membership_refs: set[str] = set()
    boundary_map_responsibility_refs: set[str] = set()
    for map_path in sorted((repo_root / SOURCE_ROOT).rglob("work-boundary-map.json")):
        boundary_map = _load_json(map_path, repo_root, issues)
        if boundary_map is None:
            continue
        location = _relative(map_path, repo_root)
        _validate_payload(
            boundary_map,
            collection_work_boundary_map_validator,
            location,
            issues,
        )
        boundary_maps.append((boundary_map, map_path))
        require_record(boundary_map.get("collection_ref"), "collection", location)
        require_record(boundary_map.get("edition_ref"), "edition", location)
        require_record(boundary_map.get("item_ref"), "item", location)

        inventory_ref = boundary_map.get("resource_inventory_ref")
        inventory_path = repo_root / str(inventory_ref)
        if not inventory_path.is_file():
            issues.append((location, f"work-boundary resource inventory is missing: {inventory_ref}"))
        elif _sha256(inventory_path) != boundary_map.get("resource_inventory_sha256"):
            issues.append((location, "work-boundary resource inventory digest drifted"))

        item_ref = boundary_map.get("item_ref")
        file_id = boundary_map.get("file_id")
        if file_owner_by_id.get(file_id) != item_ref:
            issues.append((location, "work-boundary file does not belong to its item"))
        if file_digest_by_id.get(file_id) != boundary_map.get("file_sha256"):
            issues.append((location, "work-boundary file digest differs from the item manifest"))
        if boundary_map.get("provenance_event_ref") not in event_ids:
            issues.append((location, "work-boundary provenance event is unresolved"))

        anchor_path = map_path.with_name("anchors.jsonl")
        local_anchor_ids: set[str] = set()
        anchor_page_by_id: dict[str, int] = {}
        for index, anchor in enumerate(_load_jsonl(anchor_path, repo_root, issues), start=1):
            anchor_location = f"{_relative(anchor_path, repo_root)}:{index}"
            _validate_payload(anchor, anchor_validator, anchor_location, issues)
            anchor_id = anchor.get("anchor_id")
            if isinstance(anchor_id, str):
                if anchor_id in boundary_anchor_ids:
                    issues.append((anchor_location, f"duplicate boundary anchor_id: {anchor_id}"))
                boundary_anchor_ids.add(anchor_id)
                local_anchor_ids.add(anchor_id)
                selectors = anchor.get("selectors", [])
                page_selectors = [
                    selector
                    for selector in selectors
                    if isinstance(selector, dict) and selector.get("type") == "page_region"
                ]
                if len(page_selectors) == 1:
                    page = page_selectors[0].get("page")
                    if isinstance(page, int):
                        anchor_page_by_id[anchor_id] = page
                        if page > boundary_map.get("page_count", 0):
                            issues.append((anchor_location, "boundary anchor page exceeds page_count"))
                else:
                    issues.append((anchor_location, "boundary anchor must have exactly one page selector"))
            if anchor.get("item_id") != item_ref:
                issues.append((anchor_location, "boundary anchor item_id differs from map"))
            if anchor.get("file_id") != file_id:
                issues.append((anchor_location, "boundary anchor file_id differs from map"))
            if anchor.get("file_sha256") != boundary_map.get("file_sha256"):
                issues.append((anchor_location, "boundary anchor file digest differs from map"))
            if anchor.get("provenance_event_ref") != boundary_map.get("provenance_event_ref"):
                issues.append((anchor_location, "boundary anchor provenance differs from map"))

        members = boundary_map.get("members", [])
        sequences = [
            member.get("sequence")
            for member in members
            if isinstance(member, dict)
        ]
        if sequences != list(range(1, len(members) + 1)):
            issues.append((location, "work-boundary member sequence is not contiguous from 1"))
        previous_end: int | None = None
        for member in members:
            if not isinstance(member, dict):
                continue
            require_record(member.get("work_ref"), "work", location)
            require_record(member.get("expression_ref"), "expression", location)
            expression = records_by_id.get(member.get("expression_ref"))
            if expression is not None and expression[0].get("work_ref") != member.get("work_ref"):
                issues.append((location, f"work-boundary expression belongs to another work: {member.get('expression_ref')}"))
            start_page = member.get("start_page")
            end_page = member.get("end_page")
            if isinstance(start_page, int) and isinstance(end_page, int):
                if start_page > end_page:
                    issues.append((location, f"work-boundary start exceeds end for sequence {member.get('sequence')}"))
                if previous_end is not None and start_page != previous_end + 1:
                    issues.append((location, f"work-boundary members are not contiguous at sequence {member.get('sequence')}"))
                previous_end = end_page
            title_anchor_ref = member.get("title_page_anchor_ref")
            if title_anchor_ref not in local_anchor_ids:
                issues.append((location, f"unresolved title-page anchor: {title_anchor_ref}"))
            elif anchor_page_by_id.get(title_anchor_ref) != start_page:
                issues.append((location, f"title-page anchor does not match start_page: {title_anchor_ref}"))
            for anchor_ref in member.get("boundary_evidence_anchor_refs", []):
                if anchor_ref not in local_anchor_ids:
                    issues.append((location, f"unresolved member boundary anchor: {anchor_ref}"))
            membership_ref = member.get("membership_claim_ref")
            if isinstance(membership_ref, str):
                boundary_map_membership_refs.add(membership_ref)
            responsibility_ref = member.get("translation_responsibility_claim_ref")
            if isinstance(responsibility_ref, str):
                boundary_map_responsibility_refs.add(responsibility_ref)

        non_member_sections = boundary_map.get("non_member_sections", [])
        for section in non_member_sections:
            if not isinstance(section, dict):
                continue
            start_page = section.get("start_page")
            end_page = section.get("end_page")
            if isinstance(start_page, int) and previous_end is not None and start_page != previous_end + 1:
                issues.append((location, f"non-member section is not contiguous: {section.get('label')}"))
            if isinstance(start_page, int) and isinstance(end_page, int):
                if start_page > end_page:
                    issues.append((location, f"non-member section start exceeds end: {section.get('label')}"))
                previous_end = end_page
            anchor_ref = section.get("boundary_anchor_ref")
            if anchor_ref not in local_anchor_ids:
                issues.append((location, f"unresolved non-member boundary anchor: {anchor_ref}"))
            elif anchor_page_by_id.get(anchor_ref) != start_page:
                issues.append((location, f"non-member boundary anchor does not match start_page: {anchor_ref}"))
        if previous_end != boundary_map.get("page_count"):
            issues.append((location, "work and non-member boundaries do not cover the exact page_count"))
        for anchor_ref in boundary_map.get("crosscheck_anchor_refs", []):
            if anchor_ref not in local_anchor_ids:
                issues.append((location, f"unresolved boundary crosscheck anchor: {anchor_ref}"))

    membership_claim_ids: set[str] = set()
    responsibility_claim_ids: set[str] = set()
    claim_routes = (
        ("membership-claims.jsonl", "contains_work", "collection", "work", membership_claim_ids),
        ("responsibility-claims.jsonl", "translated_by", "expression", "agent", responsibility_claim_ids),
    )
    for filename, predicate, subject_type, object_type, route_ids in claim_routes:
        for claim_path in sorted((repo_root / SOURCE_ROOT).rglob(filename)):
            for index, claim in enumerate(_load_jsonl(claim_path, repo_root, issues), start=1):
                location = f"{_relative(claim_path, repo_root)}:{index}"
                _validate_payload(claim, claim_validator, location, issues)
                claim_id = claim.get("claim_id")
                if isinstance(claim_id, str):
                    if claim_id in claim_ids:
                        issues.append((location, f"duplicate claim_id: {claim_id}"))
                    claim_ids.add(claim_id)
                    route_ids.add(claim_id)
                if claim.get("predicate") != predicate:
                    issues.append((location, f"{filename} claim predicate must be {predicate}"))
                require_record(claim.get("subject_ref"), subject_type, location)
                require_record(claim.get("object"), object_type, location)
                event_ref = claim.get("provenance_event_ref")
                if isinstance(event_ref, str) and event_ref not in event_ids:
                    issues.append((location, f"unresolved provenance_event_ref: {event_ref}"))
                for evidence_ref in claim.get("evidence_refs", []):
                    if isinstance(evidence_ref, str) and evidence_ref.startswith("tos.anchor."):
                        if evidence_ref not in boundary_anchor_ids:
                            issues.append((location, f"unresolved boundary evidence anchor: {evidence_ref}"))
                    elif isinstance(evidence_ref, str) and evidence_ref.startswith("ToS/"):
                        if not (repo_root / evidence_ref).is_file():
                            issues.append((location, f"unresolved repository evidence ref: {evidence_ref}"))

    for payload, path in (value for value in records_by_id.values() if value[0].get("record_type") == "collection"):
        location = _relative(path, repo_root)
        missing = sorted(set(payload.get("membership_claim_refs", [])) - membership_claim_ids)
        if missing:
            issues.append((location, f"unresolved membership claims: {missing}"))

    for payload, path in (value for value in records_by_id.values() if value[0].get("record_type") == "expression"):
        location = _relative(path, repo_root)
        missing = sorted(
            set(payload.get("responsibility_claim_refs", [])) - responsibility_claim_ids
        )
        if missing:
            issues.append((location, f"unresolved responsibility claims: {missing}"))

    missing_boundary_memberships = sorted(
        boundary_map_membership_refs - membership_claim_ids
    )
    if missing_boundary_memberships:
        issues.append(
            (
                SOURCE_ROOT.as_posix(),
                f"work-boundary maps reference missing membership claims: {missing_boundary_memberships}",
            )
        )
    missing_boundary_responsibilities = sorted(
        boundary_map_responsibility_refs - responsibility_claim_ids
    )
    if missing_boundary_responsibilities:
        issues.append(
            (
                SOURCE_ROOT.as_posix(),
                f"work-boundary maps reference missing responsibility claims: {missing_boundary_responsibilities}",
            )
        )

    try:
        collect_records(repo_root)
        expected_outputs = render_outputs(repo_root)
        for message in check_outputs(repo_root, expected_outputs):
            issues.append((CATALOG_ROOT.as_posix(), message))
    except CatalogBuildError as exc:
        issues.append((CATALOG_ROOT.as_posix(), str(exc)))

    catalog_manifest_path = repo_root / CATALOG_ROOT / "catalog.manifest.json"
    catalog_manifest = _load_json(catalog_manifest_path, repo_root, issues)
    if catalog_manifest is not None:
        _validate_payload(
            catalog_manifest,
            catalog_validator,
            _relative(catalog_manifest_path, repo_root),
            issues,
        )
    entry_schema = catalog_schema.get("$defs", {}).get("entry")
    if isinstance(entry_schema, dict):
        entry_class = validator_for(entry_schema)
        entry_class.check_schema(entry_schema)
        entry_validator = entry_class(entry_schema, format_checker=FormatChecker())
        for filename in RECORD_FILES.values():
            catalog_path = repo_root / CATALOG_ROOT / filename
            for index, entry in enumerate(_load_jsonl(catalog_path, repo_root, issues), start=1):
                _validate_payload(
                    entry,
                    entry_validator,
                    f"{_relative(catalog_path, repo_root)}:{index}",
                    issues,
                )

    return issues


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--require-local-payloads",
        action="store_true",
        help="fail when local gitignored source bytes are absent",
    )
    args = parser.parse_args(argv)

    issues = validate_foundation(
        args.repo_root,
        require_local_payloads=args.require_local_payloads,
    )
    if issues:
        print("Source-witness foundation validation failed.", file=sys.stderr)
        for location, message in issues:
            print(f"- {location}: {message}", file=sys.stderr)
        return 1

    payload_posture = "required and fixity-checked" if args.require_local_payloads else "optional; present bytes fixity-checked"
    print(f"[ok] validated source-witness evidence spine ({payload_posture})")
    print("[boundary] mechanics only: bibliographic, textual, rights, translation, semantic, and review truth remain human-evidence questions")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

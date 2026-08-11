#!/usr/bin/env python3
"""Validate the tracked source-witness evidence spine and optional local payloads.

This validator proves contract shape, reference closure, generated catalog parity,
and byte fixity. It does not prove bibliographic truth, OCR quality, translation
quality, semantic correctness, rights clearance, or human review.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import re
import subprocess
import sys
import unicodedata
import xml.etree.ElementTree as ET
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable

from jsonschema import FormatChecker
from jsonschema.exceptions import SchemaError
from jsonschema.validators import validator_for

from build_source_witness_catalog import (
    CATALOG_ROOT,
    CLAIM_CATALOG_PATH,
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
EXPRESSION_DERIVATION_SCHEMA = CONTRACT_ROOT / "expression-derivation.schema.json"
PROVISION_ACTIVITY_SCHEMA = CONTRACT_ROOT / "provision-activity.schema.json"
FIRST_PUBLICATION_CHRONOLOGY_SCHEMA = (
    CONTRACT_ROOT / "first-publication-chronology.schema.json"
)
CATALOG_SCHEMA = CONTRACT_ROOT / "source-witness-catalog.schema.json"
ANCHOR_SCHEMA = CONTRACT_ROOT / "source-anchor.schema.json"
ANCHOR_V2_SCHEMA = CONTRACT_ROOT / "source-anchor-v2.schema.json"
ANCHOR_V2_LAB_ROOT = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc"
)
ANCHOR_V2_LAB_MANIFEST = ANCHOR_V2_LAB_ROOT / "lab.manifest.json"
SOURCE_TEXT_LAYER_SCHEMA = CONTRACT_ROOT / "source-text-layer.schema.json"
SOURCE_TEXT_LAYER_LAB_ROOT = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc"
)
SOURCE_TEXT_LAYER_LAB_MANIFEST = SOURCE_TEXT_LAYER_LAB_ROOT / "lab.manifest.json"
PROVENANCE_V2_SCHEMA = CONTRACT_ROOT / "provenance-event-v2.schema.json"
PROVENANCE_V2_LAB_ROOT = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/provenance-event-v2-abc"
)
PROVENANCE_V2_LAB_MANIFEST = PROVENANCE_V2_LAB_ROOT / "lab.manifest.json"
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
PUBLIC_EVIDENCE_DERIVATIVE_SCHEMA = (
    CONTRACT_ROOT / "public-laboratory-evidence-derivative.schema.json"
)
MANUAL_ERROR_LEDGER_SCHEMA = (
    CONTRACT_ROOT / "manual-error-ledger-record.schema.json"
)
TRANSFER_CANDIDATE_CROSSWALK_SCHEMA = (
    CONTRACT_ROOT / "transfer-candidate-structural-crosswalk.schema.json"
)
HIERARCHICAL_TARGET_NUMBERED_UNIT_MAP_SCHEMA = (
    CONTRACT_ROOT / "hierarchical-target-numbered-unit-page-map.schema.json"
)
TARGET_STRUCTURAL_CROSSWALK_SCHEMA = (
    CONTRACT_ROOT / "transfer-candidate-target-structural-crosswalk.schema.json"
)
GERMAN_ASSISTED_SOURCE_REVIEW_SCHEMA = (
    CONTRACT_ROOT / "german-assisted-source-review.schema.json"
)
PRIVATE_TRANSFER_SOURCE_VISIBLE_REVIEW_BUNDLE_SCHEMA = (
    CONTRACT_ROOT / "private-transfer-source-visible-review-bundle.schema.json"
)
TRANSFER_SOURCE_VISIBLE_REVIEW_RECEIPT_SCHEMA = (
    CONTRACT_ROOT / "transfer-source-visible-review-receipt.schema.json"
)
CRITICAL_EDITION_WITNESS_ADMISSION_SCHEMA = (
    CONTRACT_ROOT / "critical-edition-witness-admission.schema.json"
)
CRITICAL_EDITION_CITATION_WITNESS_DECISION_SCHEMA = (
    CONTRACT_ROOT / "critical-edition-citation-witness-decision.schema.json"
)
EDITION_READING_ADMISSION_SCHEMA = (
    CONTRACT_ROOT / "edition-reading-admission.schema.json"
)
GERMAN_SOURCE_TRIANGULATION_SCHEMA = (
    CONTRACT_ROOT / "german-source-triangulation.schema.json"
)
BOUNDED_TRANSLATION_RESEARCH_INPUT_SCHEMA = (
    CONTRACT_ROOT / "bounded-translation-research-input.schema.json"
)
EXPERIMENTAL_TRANSLATION_CANDIDATE_SCHEMA = (
    CONTRACT_ROOT / "experimental-translation-candidate.schema.json"
)
EXPERIMENTAL_TRANSLATION_EPISODE_SCHEMA = (
    CONTRACT_ROOT / "experimental-translation-episode.schema.json"
)
PRIVATE_EVIDENCE_HANDOFF_PROFILE = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "private-evidence-handoff.v1.json"
)
MANUAL_ERROR_LEDGER_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "manual-error-ledger.jsonl"
)
MANUAL_ERROR_LEDGER_PROVENANCE_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "provenance.manual-error-ledger."
    "ocr-candidate-review-foundation-v1.jsonl"
)
PRIOR_EMPTY_MANUAL_ERROR_LEDGER_SHA256 = (
    "b33a8b535e65f1e431c6324607c395a69c90e56c22be6b2f402f88bcc54bf761"
)
TRANSFER_CANDIDATE_CROSSWALK_PATH = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/alignments/structure/"
    "naumann-1886-polilov-mysl-1996/"
    "transfer-candidate-page-crosswalk.v1.json"
)
HIERARCHICAL_TARGET_STRUCTURE_PATHS = (
    Path(
        "ToS/source-witnesses/works/friedrich-nietzsche/"
        "zur-genealogie-der-moral/expressions/ru-svasyan-mysl-1996/"
        "structure/mysl-1996-volume-2-operator-pdf"
    ),
    Path(
        "ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/"
        "expressions/ru-flerova-mysl-1996/structure/"
        "mysl-1996-volume-2-operator-pdf"
    ),
)
BIBLIOGRAPHIC_TOPOLOGY_ROOT = SOURCE_ROOT / "relations"
BIBLIOGRAPHIC_TOPOLOGY_PROVENANCE = (
    BIBLIOGRAPHIC_TOPOLOGY_ROOT / "provenance.jsonl"
)
BIBLIOGRAPHIC_TOPOLOGY_EVENT_REF = (
    "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31"
)
WORK_CHRONOLOGY_ROOT = (
    SOURCE_ROOT / "chronology/friedrich-nietzsche/first-publication"
)
WORK_CHRONOLOGY_CLAIMS = WORK_CHRONOLOGY_ROOT / "work-chronology-claims.jsonl"
WORK_CHRONOLOGY_PROVENANCE = WORK_CHRONOLOGY_ROOT / "provenance.jsonl"
WORK_CHRONOLOGY_EVENT_REF = (
    "tos.event.annotation.friedrich-nietzsche.first-publication-chronology.2026-07-31"
)
PROVISION_ACTIVITY_PROVENANCE_BASENAME = "provision-activity-provenance.jsonl"
BIBLIOGRAPHIC_TOPOLOGY_ROUTES = (
    (
        BIBLIOGRAPHIC_TOPOLOGY_ROOT
        / "work-expression"
        / "work-expression-claims.jsonl",
        "has_expression",
        "work",
        "expression",
        "expression_claim_refs",
        "unreviewed-work-expression-topology-claims",
    ),
    (
        BIBLIOGRAPHIC_TOPOLOGY_ROOT
        / "expression-edition"
        / "expression-edition-claims.jsonl",
        "embodied_by",
        "expression",
        "edition",
        "embodiment_claim_refs",
        "unreviewed-expression-edition-topology-claims",
    ),
    (
        BIBLIOGRAPHIC_TOPOLOGY_ROOT
        / "edition-item"
        / "edition-item-claims.jsonl",
        "exemplified_by",
        "edition",
        "item",
        "exemplar_claim_refs",
        "unreviewed-edition-item-topology-claims",
    ),
)
EXPRESSION_DERIVATION_ROOT = (
    BIBLIOGRAPHIC_TOPOLOGY_ROOT / "expression-derivation"
)
EXPRESSION_DERIVATION_CLAIMS = (
    EXPRESSION_DERIVATION_ROOT / "expression-derivation-claims.jsonl"
)
EXPRESSION_DERIVATION_PROVENANCE = (
    EXPRESSION_DERIVATION_ROOT / "provenance.jsonl"
)
EXPRESSION_DERIVATION_EVENT_REF = (
    "tos.event.annotation.expression-derivation."
    "antonovsky-revision-lineage.2026-08-01"
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


def _provision_temporal_issues(activity: dict[str, Any]) -> list[str]:
    temporal = activity.get("temporal")
    if not isinstance(temporal, dict) or temporal.get("kind") != "interval":
        return []
    start = temporal.get("start")
    end = temporal.get("end")
    if isinstance(start, str) and isinstance(end, str) and start > end:
        return ["provision-activity interval starts after it ends"]
    return []


def _repo_ref_exists(repo_root: Path, ref: object) -> bool:
    return isinstance(ref, str) and (not ref.startswith("ToS/") or (repo_root / ref).exists())


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _anchor_v2_expression_envelopes(expression: object) -> list[dict[str, Any]]:
    if not isinstance(expression, dict):
        return []
    mode = expression.get("mode")
    if mode == "single":
        selector = expression.get("selector")
        return [selector] if isinstance(selector, dict) else []
    field = "alternatives" if mode == "alternatives" else "steps"
    candidates = expression.get(field, [])
    if not isinstance(candidates, list):
        return []
    return [
        item
        for item in candidates
        if isinstance(item, dict)
    ]


def _anchor_v2_semantic_issues(anchor: dict[str, Any]) -> list[str]:
    """Check semantics JSON Schema cannot express without claiming source truth."""

    messages: list[str] = []
    if (
        anchor.get("supersedes_anchor_ref") is not None
        and anchor.get("supersedes_anchor_ref") == anchor.get("anchor_id")
    ):
        messages.append("source anchor cannot supersede itself")
    selector_payload = anchor.get("selector_payload", {})
    publication = anchor.get("publication_boundary", {})
    if not isinstance(selector_payload, dict):
        return messages
    if not isinstance(publication, dict):
        publication = {}
    if selector_payload.get("kind") == "withheld_selector_receipt":
        if publication.get("source_text_in_record") is True:
            messages.append("withheld selector receipt cannot claim source text in record")
        return messages

    expression = selector_payload.get("expression", {})
    if not isinstance(expression, dict):
        return messages
    envelopes = _anchor_v2_expression_envelopes(expression)
    selectors = [
        envelope.get("selector", {})
        for envelope in envelopes
        if isinstance(envelope.get("selector"), dict)
    ]
    selector_types = {selector.get("type") for selector in selectors}

    target = anchor.get("target", {})
    target_digest = target.get("file_sha256") if isinstance(target, dict) else None
    if expression.get("mode") in {"single", "refinement_chain"} and envelopes:
        first_state = envelopes[0].get("state", {})
        first_digest = (
            first_state.get("representation_sha256")
            if isinstance(first_state, dict)
            else None
        )
        if first_digest != target_digest:
            messages.append("first selector state is not the exact target file state")
    if expression.get("mode") == "alternatives":
        first_digests = {
            envelope.get("state", {}).get("representation_sha256")
            for envelope in envelopes
            if isinstance(envelope.get("state"), dict)
        }
        if first_digests != {target_digest}:
            messages.append("alternatives do not independently begin from the exact target state")

    for envelope in envelopes:
        selector = envelope.get("selector", {})
        if not isinstance(selector, dict):
            continue
        selector_type = selector.get("type")
        state = envelope.get("state", {})
        if not isinstance(state, dict):
            continue
        if selector_type in {"text_quote", "text_position"}:
            if "character_normalization" not in state:
                messages.append(f"{selector_type} lacks explicit character normalization")
        if selector_type in {"text_position", "byte_position"}:
            start = selector.get("start")
            end = selector.get("end")
            if isinstance(start, int) and isinstance(end, int) and start >= end:
                messages.append(f"{selector_type} interval is empty or reversed")
        if selector_type == "page_region":
            coordinate_space = selector.get("coordinate_space")
            if coordinate_space == "normalized_0_1":
                x = selector.get("x")
                y = selector.get("y")
                width = selector.get("width")
                height = selector.get("height")
                if all(isinstance(value, (int, float)) for value in (x, y, width, height)):
                    if x + width > 1 or y + height > 1:
                        messages.append("normalized page region exceeds the unit square")
            elif coordinate_space in {"pixels", "points"}:
                x = selector.get("x")
                y = selector.get("y")
                width = selector.get("width")
                height = selector.get("height")
                source_width = selector.get("source_width")
                source_height = selector.get("source_height")
                if all(
                    isinstance(value, (int, float))
                    for value in (x, y, width, height, source_width, source_height)
                ) and (x + width > source_width or y + height > source_height):
                    messages.append("page region exceeds the declared representation extent")

    tracked_nonpublic = (
        publication.get("record_storage") == "tracked"
        and publication.get("source_content_visibility") != "public"
    )
    if tracked_nonpublic and "text_quote" in selector_types:
        messages.append("tracked nonpublic anchor cannot carry a text quote")
    if publication.get("source_text_in_record") is False and "text_quote" in selector_types:
        messages.append("text quote contradicts source_text_in_record=false")
    if publication.get("source_text_in_record") is True and "text_quote" not in selector_types:
        messages.append("source_text_in_record=true has no text-bearing selector")
    if (
        anchor.get("resolution_status") == "mechanically_resolved"
        and not envelopes
    ):
        messages.append("mechanically resolved anchor has no selector expression")
    return messages


def _anchor_v2_json_pointer(payload: object, pointer: str) -> object:
    if not pointer.startswith("/"):
        raise ValueError("JSON Pointer must begin with /")
    current = payload
    for raw_part in pointer[1:].split("/"):
        part = raw_part.replace("~1", "/").replace("~0", "~")
        if isinstance(current, list):
            current = current[int(part)]
        elif isinstance(current, dict):
            current = current[part]
        else:
            raise ValueError("JSON Pointer traverses a scalar")
    return current


def _anchor_v2_text(scope: dict[str, Any], state: dict[str, Any]) -> str:
    value = scope.get("value")
    if isinstance(value, bytes):
        value = value.decode("utf-8")
    if not isinstance(value, str):
        value = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    normalization = state.get("character_normalization", "none")
    if normalization != "none":
        value = unicodedata.normalize(normalization, value)
    return value


def _anchor_v2_apply_selector(
    envelope: dict[str, Any],
    *,
    scope: dict[str, Any],
    resources_by_path: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    state = envelope["state"]
    selector = envelope["selector"]
    selector_type = selector["type"]

    if selector_type == "container_member":
        container_value = scope.get("value")
        if isinstance(container_value, bytes):
            container_value = container_value.decode("utf-8")
        container_payload = (
            json.loads(container_value)
            if isinstance(container_value, str)
            else container_value
        )
        declared_members = (
            container_payload.get("members")
            if isinstance(container_payload, dict)
            else None
        )
        if (
            not isinstance(declared_members, list)
            or selector["member_path"] not in declared_members
        ):
            raise ValueError("container member is not declared by the selected container")
        member = resources_by_path.get(selector["member_path"])
        if member is None:
            raise ValueError("container member is absent from the laboratory manifest")
        if member["sha256"] != selector["member_sha256"]:
            raise ValueError("container member digest differs from selector")
        if member["media_type"] != selector["member_media_type"]:
            raise ValueError("container member media type differs from selector")
        return {
            "value": member["absolute_path"].read_bytes(),
            "representation_ref": member["representation_ref"],
            "representation_sha256": member["sha256"],
        }

    if selector_type == "structural":
        scheme = selector["scheme"]
        if scheme == "json_pointer":
            raw = scope["value"]
            if isinstance(raw, bytes):
                raw = raw.decode("utf-8")
            payload = json.loads(raw) if isinstance(raw, str) else raw
            selected = _anchor_v2_json_pointer(payload, selector["value"])
            return {**scope, "value": selected}
        if scheme == "xml_id":
            raw = scope["value"]
            if isinstance(raw, str):
                raw = raw.encode("utf-8")
            root = ET.fromstring(raw)
            matches = [
                element
                for element in root.iter()
                if element.get("id") == selector["value"]
                or element.get("{http://www.w3.org/XML/1998/namespace}id")
                == selector["value"]
            ]
            if len(matches) != 1:
                raise ValueError("xml_id selector does not resolve exactly one element")
            return {**scope, "value": "".join(matches[0].itertext())}
        raise ValueError(f"unsupported synthetic structural scheme: {scheme}")

    if selector_type == "text_quote":
        text = _anchor_v2_text(scope, state)
        exact = selector["exact"]
        starts: list[int] = []
        cursor = 0
        while True:
            position = text.find(exact, cursor)
            if position < 0:
                break
            prefix = selector.get("prefix")
            suffix = selector.get("suffix")
            prefix_ok = prefix is None or text[max(0, position - len(prefix)):position] == prefix
            end = position + len(exact)
            suffix_ok = suffix is None or text[end:end + len(suffix)] == suffix
            if prefix_ok and suffix_ok:
                starts.append(position)
            cursor = position + 1
        if len(starts) != 1:
            raise ValueError("text quote does not resolve exactly once with its context")
        start = starts[0]
        end = start + len(exact)
        if unicodedata.combining(text[start]):
            raise ValueError("text quote start splits a combining sequence")
        if end < len(text) and unicodedata.combining(text[end]):
            raise ValueError("text quote end splits a combining sequence")
        return {**scope, "value": exact}

    if selector_type == "text_position":
        text = _anchor_v2_text(scope, state)
        start = selector["start"]
        end = selector["end"]
        if not 0 <= start < end <= len(text):
            raise ValueError("text-position interval is outside the selected representation")
        if unicodedata.combining(text[start]):
            raise ValueError("text-position start splits a combining sequence")
        if end < len(text) and unicodedata.combining(text[end]):
            raise ValueError("text-position end splits a combining sequence")
        return {**scope, "value": text[start:end]}

    if selector_type == "byte_position":
        value = scope["value"]
        if isinstance(value, str):
            value = value.encode("utf-8")
        if not isinstance(value, bytes):
            raise ValueError("byte-position selector requires a byte representation")
        start = selector["start"]
        end = selector["end"]
        if not 0 <= start < end <= len(value):
            raise ValueError("byte-position interval is outside the selected representation")
        return {**scope, "value": value[start:end]}

    if selector_type == "page_region":
        return {
            **scope,
            "value": {
                key: selector[key]
                for key in (
                    "page_identity",
                    "x",
                    "y",
                    "width",
                    "height",
                    "coordinate_space",
                )
            },
        }
    if selector_type == "fragment":
        raise ValueError("synthetic laboratory does not implement arbitrary fragment standards")
    raise ValueError(f"unsupported selector type: {selector_type}")


def _anchor_v2_resolve_expression(
    anchor: dict[str, Any],
    *,
    resources_by_ref: dict[str, dict[str, Any]],
    resources_by_path: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    expression = anchor["selector_payload"]["expression"]
    mode = expression["mode"]

    def state_scope(envelope: dict[str, Any]) -> dict[str, Any]:
        state = envelope["state"]
        resource = resources_by_ref.get(state["representation_ref"])
        if resource is None:
            raise ValueError("selector state representation is absent from the laboratory manifest")
        if state["representation_sha256"] != resource["sha256"]:
            raise ValueError("selector state digest differs from the represented bytes")
        if state["media_type"] != resource["media_type"]:
            raise ValueError("selector state media type differs from the represented bytes")
        return {
            "value": resource["absolute_path"].read_bytes(),
            "representation_ref": resource["representation_ref"],
            "representation_sha256": resource["sha256"],
        }

    if mode == "single":
        envelope = expression["selector"]
        result = _anchor_v2_apply_selector(
            envelope,
            scope=state_scope(envelope),
            resources_by_path=resources_by_path,
        )
        return {"mode": mode, "result": result["value"], "branch_results": [result["value"]]}

    if mode == "alternatives":
        results: list[object] = []
        for envelope in expression["alternatives"]:
            result = _anchor_v2_apply_selector(
                envelope,
                scope=state_scope(envelope),
                resources_by_path=resources_by_path,
            )["value"]
            results.append(result)
        canonical = [
            value if isinstance(value, str) else json.dumps(value, ensure_ascii=False, sort_keys=True)
            for value in results
        ]
        if len(set(canonical)) != 1:
            raise ValueError("alternative selectors resolve to different segments")
        return {"mode": mode, "result": results[0], "branch_results": results}

    if mode == "refinement_chain":
        current: dict[str, Any] | None = None
        step_results: list[object] = []
        for index, envelope in enumerate(expression["steps"]):
            state = envelope["state"]
            base = state_scope(envelope)
            if current is None:
                scope = base
            else:
                if current.get("representation_sha256") != state["representation_sha256"]:
                    raise ValueError("refinement step state differs from the prior selected representation")
                scope = current
            current = _anchor_v2_apply_selector(
                envelope,
                scope=scope,
                resources_by_path=resources_by_path,
            )
            step_results.append(current["value"])
            if index == 0 and envelope["selector"]["type"] == "container_member":
                next_state = expression["steps"][1]["state"]
                if current["representation_sha256"] != next_state["representation_sha256"]:
                    raise ValueError("container refinement does not bind the selected member state")
        if current is None:
            raise ValueError("refinement chain is empty")
        return {"mode": mode, "result": current["value"], "step_results": step_results}
    raise ValueError(f"unsupported selector expression mode: {mode}")


def validate_source_anchor_v2_lab(repo_root: Path) -> tuple[list[Issue], dict[str, Any]]:
    """Resolve public synthetic A/B/C and exercise semantic negative controls."""

    repo_root = repo_root.resolve()
    issues: list[Issue] = []
    report: dict[str, Any] = {"variants": [], "negative_controls": {}}
    try:
        anchor_validator, _ = _schema_validator(ANCHOR_V2_SCHEMA, repo_root)
    except (OSError, json.JSONDecodeError, SchemaError) as exc:
        return [(ANCHOR_V2_SCHEMA.as_posix(), f"cannot load v2 schema: {exc}")], report

    manifest_path = repo_root / ANCHOR_V2_LAB_MANIFEST
    manifest = _load_json(manifest_path, repo_root, issues)
    if manifest is None:
        return issues, report
    if manifest.get("schema_version") != "tos_source_anchor_v2_lab_v1":
        issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), "unexpected laboratory manifest version"))
    if manifest.get("contract_ref") != ANCHOR_V2_SCHEMA.as_posix():
        issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), "laboratory contract reference drifted"))
    if manifest.get("authority_posture") != "synthetic_mechanical_fixture_only":
        issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), "laboratory authority posture widened"))
    expected_authority_limits = {
        "source_payload_used": False,
        "human_review_performed": False,
        "source_text_accepted": False,
        "translation_created": False,
        "semantic_claim_created": False,
        "canon_effect": False,
    }
    if manifest.get("authority_limits") != expected_authority_limits:
        issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), "laboratory authority limits widened"))

    resources_by_ref: dict[str, dict[str, Any]] = {}
    resources_by_path: dict[str, dict[str, Any]] = {}
    for index, resource in enumerate(manifest.get("resources", []), start=1):
        location = f"{ANCHOR_V2_LAB_MANIFEST.as_posix()} resources[{index}]"
        if not isinstance(resource, dict):
            issues.append((location, "resource is not an object"))
            continue
        relative_path = resource.get("path")
        if not isinstance(relative_path, str) or Path(relative_path).is_absolute() or ".." in Path(relative_path).parts:
            issues.append((location, "resource path is not a bounded relative path"))
            continue
        absolute_path = repo_root / ANCHOR_V2_LAB_ROOT / relative_path
        if absolute_path.is_symlink():
            issues.append((location, f"resource is a symlink: {relative_path}"))
            continue
        if not absolute_path.is_file():
            issues.append((location, f"resource is missing: {relative_path}"))
            continue
        actual_digest = _sha256(absolute_path)
        if actual_digest != resource.get("sha256"):
            issues.append((location, f"resource digest drifted: {relative_path}"))
        representation_ref = resource.get("representation_ref")
        if not isinstance(representation_ref, str) or representation_ref in resources_by_ref:
            issues.append((location, "representation_ref is missing or duplicated"))
            continue
        if relative_path in resources_by_path:
            issues.append((location, "resource path is duplicated"))
            continue
        enriched = {**resource, "absolute_path": absolute_path}
        resources_by_ref[representation_ref] = enriched
        resources_by_path[relative_path] = enriched

    anchors_by_variant: dict[str, dict[str, Any]] = {}
    anchor_ids: set[str] = set()
    for variant in manifest.get("variants", []):
        if not isinstance(variant, dict):
            continue
        variant_id = variant.get("variant_id")
        anchor_ref = variant.get("anchor_ref")
        if not isinstance(variant_id, str) or not isinstance(anchor_ref, str):
            issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), "variant identity or anchor_ref is invalid"))
            continue
        anchor_relative = Path(anchor_ref)
        if anchor_relative.is_absolute() or ".." in anchor_relative.parts:
            issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), "variant anchor_ref leaves the laboratory root"))
            continue
        if variant_id in anchors_by_variant:
            issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), f"variant identity is duplicated: {variant_id}"))
            continue
        anchor_path = repo_root / ANCHOR_V2_LAB_ROOT / anchor_ref
        if anchor_path.is_symlink():
            issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), f"variant anchor is a symlink: {anchor_ref}"))
            continue
        anchor = _load_json(anchor_path, repo_root, issues)
        if anchor is None:
            continue
        location = _relative(anchor_path, repo_root)
        _validate_payload(anchor, anchor_validator, location, issues)
        anchor_id = anchor.get("anchor_id")
        if isinstance(anchor_id, str):
            if anchor_id in anchor_ids:
                issues.append((location, f"anchor identity is duplicated: {anchor_id}"))
            anchor_ids.add(anchor_id)
        for message in _anchor_v2_semantic_issues(anchor):
            issues.append((location, message))
        selector_method = anchor.get("selector_method", {})
        if selector_method.get("configuration_ref") != ANCHOR_V2_LAB_MANIFEST.as_posix():
            issues.append((location, "selector method does not cite the laboratory manifest"))
        if selector_method.get("configuration_digest") != _sha256(manifest_path):
            issues.append((location, "selector method configuration digest drifted"))
        try:
            resolved = _anchor_v2_resolve_expression(
                anchor,
                resources_by_ref=resources_by_ref,
                resources_by_path=resources_by_path,
            )
        except (KeyError, TypeError, ValueError, UnicodeError, ET.ParseError, json.JSONDecodeError) as exc:
            issues.append((location, f"synthetic resolution failed: {exc}"))
            continue
        result = resolved["result"]
        result_text = result if isinstance(result, str) else json.dumps(result, ensure_ascii=False, sort_keys=True)
        result_digest = _sha256_text(result_text)
        if result_text != variant.get("expected_selection"):
            issues.append((location, "resolved selection differs from the frozen expected selection"))
        if result_digest != variant.get("expected_selection_sha256"):
            issues.append((location, "resolved selection digest differs from the frozen expectation"))
        report["variants"].append(
            {
                "variant_id": variant_id,
                "mode": resolved["mode"],
                "selection": result_text,
                "selection_sha256": result_digest,
                "resolution_status": anchor.get("resolution_status"),
                "review_status": anchor.get("review_status"),
            }
        )
        anchors_by_variant[variant_id] = anchor

    def negative_rejected(name: str, action: Any) -> None:
        try:
            action()
        except (KeyError, TypeError, ValueError, UnicodeError, ET.ParseError, json.JSONDecodeError):
            report["negative_controls"][name] = "rejected"
        else:
            issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), f"negative control was not rejected: {name}"))
            report["negative_controls"][name] = "not_rejected"

    if set(anchors_by_variant) == {"A", "B", "C"}:
        def utf16_mismatch() -> None:
            mutated = copy.deepcopy(anchors_by_variant["B"])
            selector = mutated["selector_payload"]["expression"]["selector"]["selector"]
            selector["start"] = 5
            selector["end"] = 10
            result = _anchor_v2_resolve_expression(
                mutated,
                resources_by_ref=resources_by_ref,
                resources_by_path=resources_by_path,
            )["result"]
            expected = next(item["expected_selection"] for item in manifest["variants"] if item["variant_id"] == "B")
            if result != expected:
                raise ValueError("UTF-16 offsets do not select the frozen Unicode-code-point span")

        def divergent_alternatives() -> None:
            mutated = copy.deepcopy(anchors_by_variant["A"])
            quote = mutated["selector_payload"]["expression"]["alternatives"][1]["selector"]
            quote["exact"] = "A tree begins in evidence."
            _anchor_v2_resolve_expression(
                mutated,
                resources_by_ref=resources_by_ref,
                resources_by_path=resources_by_path,
            )

        def digest_drift() -> None:
            mutated = copy.deepcopy(anchors_by_variant["B"])
            mutated["selector_payload"]["expression"]["selector"]["state"]["representation_sha256"] = "0" * 64
            _anchor_v2_resolve_expression(
                mutated,
                resources_by_ref=resources_by_ref,
                resources_by_path=resources_by_path,
            )

        def reversed_refinement() -> None:
            mutated = copy.deepcopy(anchors_by_variant["C"])
            mutated["selector_payload"]["expression"]["steps"].reverse()
            _anchor_v2_resolve_expression(
                mutated,
                resources_by_ref=resources_by_ref,
                resources_by_path=resources_by_path,
            )

        def tracked_nonpublic_quote() -> None:
            mutated = copy.deepcopy(anchors_by_variant["A"])
            mutated["publication_boundary"]["source_content_visibility"] = "local_only"
            messages = _anchor_v2_semantic_issues(mutated)
            if "tracked nonpublic anchor cannot carry a text quote" in messages:
                raise ValueError("tracked nonpublic quote rejected")

        def normalized_region_overflow() -> None:
            mutated = copy.deepcopy(anchors_by_variant["B"])
            envelope = mutated["selector_payload"]["expression"]["selector"]
            envelope["selector"] = {
                "type": "page_region",
                "page_identity": {"page_number": 1},
                "x": 0.8,
                "y": 0.1,
                "width": 0.4,
                "height": 0.2,
                "coordinate_space": "normalized_0_1",
            }
            messages = _anchor_v2_semantic_issues(mutated)
            if "normalized page region exceeds the unit square" in messages:
                raise ValueError("normalized region overflow rejected")

        negative_rejected("utf16_offset_mislabeled_as_unicode_code_point", utf16_mismatch)
        negative_rejected("alternatives_resolve_to_different_passages", divergent_alternatives)
        negative_rejected("representation_digest_drift", digest_drift)
        negative_rejected("refinement_steps_reversed", reversed_refinement)
        negative_rejected("tracked_nonpublic_text_quote", tracked_nonpublic_quote)
        negative_rejected("normalized_page_region_overflow", normalized_region_overflow)

    declared_controls = set(manifest.get("negative_controls", []))
    if declared_controls != set(report["negative_controls"]):
        issues.append((ANCHOR_V2_LAB_MANIFEST.as_posix(), "negative-control coverage differs from manifest"))
    return issues, report


def _source_text_layer_semantic_issues(layer: dict[str, Any]) -> list[str]:
    """Return cross-field source-text-layer defects JSON Schema cannot express."""

    messages: list[str] = []
    layer_id = layer.get("layer_id")
    if layer.get("supersedes_layer_ref") == layer_id:
        messages.append("source text layer cannot supersede itself")

    representation = layer.get("representation", {})
    scope = representation.get("text_scope", {})
    if (
        isinstance(scope.get("start"), int)
        and isinstance(scope.get("end"), int)
        and scope["end"] < scope["start"]
    ):
        messages.append("representation text scope is reversed")

    storage = representation.get("storage")
    tracked_content = representation.get("tracked_content")
    visibility = representation.get("content_visibility")
    if (storage == "tracked") != (tracked_content is True):
        messages.append("representation storage and tracked-content posture disagree")
    if tracked_content is True and visibility != "public":
        messages.append("tracked source text layer must be public content")
    publication_authorized = representation.get("publication_authorized")
    publication_authority_refs = representation.get(
        "publication_authority_refs", []
    )
    if publication_authorized is True and visibility != "public":
        messages.append("nonpublic source text layer cannot authorize publication")
    if publication_authorized is True and not publication_authority_refs:
        messages.append("publication authorization has no authority reference")
    if publication_authorized is not True and publication_authority_refs:
        messages.append("publication authority references contradict the closed gate")

    anchor_ids = {
        binding.get("anchor_id")
        for binding in layer.get("source_binding", {}).get("anchors", [])
        if isinstance(binding, dict)
    }
    derivation = layer.get("derivation", {})
    inputs = derivation.get("input_layers", [])
    if any(
        isinstance(item, dict) and item.get("layer_id") == layer_id
        for item in inputs
    ):
        messages.append("source text layer cannot derive from itself")

    change_payload = derivation.get("change_payload", {})
    operations = (
        change_payload.get("operations", [])
        if change_payload.get("kind") == "explicit_operations"
        else []
    )
    last_input_end = -1
    last_output_end = -1
    for operation in operations:
        if not isinstance(operation, dict):
            continue
        input_span = operation.get("input_span", {})
        output_span = operation.get("output_span", {})
        input_start = input_span.get("start")
        input_end = input_span.get("end")
        output_start = output_span.get("start")
        output_end = output_span.get("end")
        if all(isinstance(value, int) for value in (input_start, input_end)):
            if input_end < input_start:
                messages.append(f"{operation.get('edit_id')} input span is reversed")
            if input_start < last_input_end:
                messages.append("explicit input edit spans overlap or are out of order")
            last_input_end = max(last_input_end, input_end)
        if all(isinstance(value, int) for value in (output_start, output_end)):
            if output_end < output_start:
                messages.append(f"{operation.get('edit_id')} output span is reversed")
            if output_start < last_output_end:
                messages.append("explicit output edit spans overlap or are out of order")
            last_output_end = max(last_output_end, output_end)
        input_exact = operation.get("input_exact")
        output_exact = operation.get("output_exact")
        if isinstance(input_exact, str):
            if operation.get("input_sha256") != _sha256_text(input_exact):
                messages.append(f"{operation.get('edit_id')} input text digest drifted")
            if isinstance(input_start, int) and isinstance(input_end, int):
                if len(input_exact) != input_end - input_start:
                    messages.append(f"{operation.get('edit_id')} input span length differs from text")
        if isinstance(output_exact, str):
            if operation.get("output_sha256") != _sha256_text(output_exact):
                messages.append(f"{operation.get('edit_id')} output text digest drifted")
            if isinstance(output_start, int) and isinstance(output_end, int):
                if len(output_exact) != output_end - output_start:
                    messages.append(f"{operation.get('edit_id')} output span length differs from text")
        evidence = set(operation.get("evidence_anchor_refs", []))
        if not evidence or not evidence.issubset(anchor_ids):
            messages.append(f"{operation.get('edit_id')} evidence anchors leave the source binding")

    uncertainty = layer.get("uncertainty", {})
    for annotation in uncertainty.get("annotations", []):
        if not isinstance(annotation, dict):
            continue
        if annotation.get("anchor_ref") not in anchor_ids:
            messages.append("uncertainty annotation leaves the source binding")
        for alternative in annotation.get("alternatives", []):
            if not isinstance(alternative, dict):
                continue
            value = alternative.get("value")
            in_record = alternative.get("value_in_record")
            if in_record is True and not isinstance(value, str):
                messages.append("in-record uncertainty alternative omits its value")
            if in_record is False and value is not None:
                messages.append("withheld uncertainty alternative exposes a value")
            if isinstance(value, str) and alternative.get("value_sha256") != _sha256_text(value):
                messages.append("uncertainty alternative digest drifted")
    return messages


def _source_text_layer_selected_text(
    layer: dict[str, Any],
    *,
    repo_root: Path,
) -> tuple[str, Path]:
    representation = layer["representation"]
    content_ref = representation["content_ref"]
    content_path = repo_root / content_ref
    if content_path.is_symlink() or not content_path.is_file():
        raise ValueError("text-layer representation is absent or symlinked")
    if _sha256(content_path) != representation["content_sha256"]:
        raise ValueError("text-layer representation digest drifted")
    try:
        text = content_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        raise ValueError(f"text-layer representation is not UTF-8 text: {exc}") from exc
    scope = representation["text_scope"]
    start = scope["start"]
    end = scope["end"]
    if end < start or end > len(text):
        raise ValueError("text-layer representation scope leaves the content")
    selected = text[start:end]
    normalization = representation["character_normalization"]
    if normalization in {"NFC", "NFD", "NFKC", "NFKD"}:
        if unicodedata.normalize(normalization, selected) != selected:
            raise ValueError("text-layer representation contradicts its Unicode normalization")
    return selected, content_path


def _replay_source_text_layer_edits(
    input_text: str,
    output_text: str,
    operations: list[dict[str, Any]],
) -> None:
    """Independently replay ordered half-open Unicode edits against real bytes."""

    pieces: list[str] = []
    input_cursor = 0
    output_cursor = 0
    for operation in operations:
        input_span = operation["input_span"]
        output_span = operation["output_span"]
        start = input_span["start"]
        end = input_span["end"]
        if start < input_cursor or end < start or end > len(input_text):
            raise ValueError("explicit input edits overlap, reverse, or leave the input")
        unchanged = input_text[input_cursor:start]
        pieces.append(unchanged)
        output_cursor += len(unchanged)
        if input_text[start:end] != operation["input_exact"]:
            raise ValueError("explicit input edit text does not match the input bytes")
        if _sha256_text(operation["input_exact"]) != operation["input_sha256"]:
            raise ValueError("explicit input edit digest drifted")
        if output_span["start"] != output_cursor:
            raise ValueError("explicit output edit start is not replay-aligned")
        if output_span["end"] != output_cursor + len(operation["output_exact"]):
            raise ValueError("explicit output edit end is not replay-aligned")
        if _sha256_text(operation["output_exact"]) != operation["output_sha256"]:
            raise ValueError("explicit output edit digest drifted")
        pieces.append(operation["output_exact"])
        output_cursor = output_span["end"]
        input_cursor = end
    pieces.append(input_text[input_cursor:])
    if "".join(pieces) != output_text:
        raise ValueError("explicit edit replay does not reproduce the output layer")


def validate_source_text_layer_lab(repo_root: Path) -> tuple[list[Issue], dict[str, Any]]:
    """Validate public synthetic OCR -> diplomatic -> normalized layer lineage."""

    repo_root = repo_root.resolve()
    issues: list[Issue] = []
    report: dict[str, Any] = {
        "source_anchor_selection": None,
        "variants": [],
        "negative_controls": {},
    }
    try:
        layer_validator, _ = _schema_validator(SOURCE_TEXT_LAYER_SCHEMA, repo_root)
    except (OSError, json.JSONDecodeError, SchemaError) as exc:
        return [(SOURCE_TEXT_LAYER_SCHEMA.as_posix(), f"cannot load text-layer schema: {exc}")], report

    manifest_path = repo_root / SOURCE_TEXT_LAYER_LAB_MANIFEST
    manifest = _load_json(manifest_path, repo_root, issues)
    if manifest is None:
        return issues, report
    if manifest.get("schema_version") != "tos_source_text_layer_lab_v1":
        issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "unexpected laboratory manifest version"))
    if manifest.get("contract_ref") != SOURCE_TEXT_LAYER_SCHEMA.as_posix():
        issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "laboratory contract reference drifted"))
    if manifest.get("authority_posture") != "public_synthetic_mechanics_only":
        issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "laboratory authority posture widened"))
    expected_limits = {
        "real_source_payload_used": False,
        "human_review_performed": False,
        "source_text_accepted": False,
        "translation_created": False,
        "linguistic_claim_created": False,
        "semantic_claim_created": False,
        "graph_effect_created": False,
        "canon_effect": False,
        "routine_human_task_created": False,
    }
    if manifest.get("authority_limits") != expected_limits:
        issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "laboratory authority limits widened"))

    policy = manifest.get("editorial_policy", {})
    policy_ref = policy.get("ref")
    policy_path = repo_root / policy_ref if isinstance(policy_ref, str) else manifest_path
    if not policy_path.is_file() or _sha256(policy_path) != policy.get("sha256"):
        issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "editorial policy closure drifted"))

    anchor_issues, anchor_report = validate_source_anchor_v2_lab(repo_root)
    for location, message in anchor_issues:
        issues.append((location, f"source-text-layer anchor dependency: {message}"))
    anchor_expectation = manifest.get("source_anchor", {})
    anchor_row = next(
        (row for row in anchor_report.get("variants", []) if row.get("variant_id") == "B"),
        None,
    )
    if anchor_row is None:
        issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "source anchor B did not resolve"))
    else:
        report["source_anchor_selection"] = anchor_row.get("selection")
        if (
            anchor_row.get("selection") != anchor_expectation.get("expected_selection")
            or anchor_row.get("selection_sha256") != anchor_expectation.get("expected_selection_sha256")
            or anchor_row.get("review_status") != anchor_expectation.get("review_status")
        ):
            issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "source anchor expectation drifted"))
    anchor_record_ref = anchor_expectation.get("anchor_record_ref")
    anchor_record_path = repo_root / anchor_record_ref if isinstance(anchor_record_ref, str) else manifest_path
    if not anchor_record_path.is_file() or _sha256(anchor_record_path) != anchor_expectation.get("anchor_record_sha256"):
        issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "source anchor record closure drifted"))

    layers_by_id: dict[str, dict[str, Any]] = {}
    record_ref_by_id: dict[str, str] = {}
    selected_text_by_id: dict[str, str] = {}
    layers_by_variant: dict[str, dict[str, Any]] = {}
    for variant in manifest.get("variants", []):
        if not isinstance(variant, dict):
            issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "variant is not an object"))
            continue
        variant_id = variant.get("variant_id")
        layer_ref = variant.get("layer_ref")
        content_ref = variant.get("content_ref")
        if not all(isinstance(value, str) for value in (variant_id, layer_ref, content_ref)):
            issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "variant references are invalid"))
            continue
        layer_path = repo_root / layer_ref
        content_path = repo_root / content_ref
        if layer_path.is_symlink() or not layer_path.is_file() or _sha256(layer_path) != variant.get("layer_sha256"):
            issues.append((layer_ref, "layer record fixity drifted"))
            continue
        if content_path.is_symlink() or not content_path.is_file() or _sha256(content_path) != variant.get("content_sha256"):
            issues.append((content_ref, "layer content fixity drifted"))
            continue
        layer = _load_json(layer_path, repo_root, issues)
        if layer is None:
            continue
        _validate_payload(layer, layer_validator, layer_ref, issues)
        for message in _source_text_layer_semantic_issues(layer):
            issues.append((layer_ref, message))
        try:
            selected_text, selected_path = _source_text_layer_selected_text(layer, repo_root=repo_root)
        except (KeyError, TypeError, ValueError) as exc:
            issues.append((layer_ref, str(exc)))
            continue
        if selected_path != content_path:
            issues.append((layer_ref, "layer representation and manifest content paths differ"))
        if selected_text != variant.get("expected_text"):
            issues.append((layer_ref, "selected text differs from the frozen expectation"))
        if _sha256_text(selected_text) != variant.get("expected_text_sha256"):
            issues.append((layer_ref, "selected text digest differs from the frozen expectation"))
        representation = layer.get("representation", {})
        if representation.get("content_sha256") != variant.get("content_sha256"):
            issues.append((layer_ref, "layer and manifest content digests differ"))
        editorial = layer.get("editorial_policy", {})
        if editorial.get("policy_ref") != policy_ref or editorial.get("policy_sha256") != policy.get("sha256"):
            issues.append((layer_ref, "layer editorial policy closure drifted"))
        source_binding = layer.get("source_binding", {})
        if source_binding.get("source_file_sha256") != "0e80d33931ca968af97942d1a91c82d4ab4c2ed9c417b21a05186ef85493dfce":
            issues.append((layer_ref, "layer source file digest left the exact anchor target"))
        anchors = source_binding.get("anchors", [])
        if anchors != [
            {
                "anchor_id": anchor_expectation.get("anchor_id"),
                "anchor_record_ref": anchor_record_ref,
                "anchor_record_sha256": anchor_expectation.get("anchor_record_sha256"),
            }
        ]:
            issues.append((layer_ref, "layer source-anchor binding drifted"))

        layer_id = layer.get("layer_id")
        if not isinstance(layer_id, str) or layer_id in layers_by_id:
            issues.append((layer_ref, "layer identity is invalid or duplicated"))
            continue
        input_layers = layer.get("derivation", {}).get("input_layers", [])
        for input_binding in input_layers:
            input_id = input_binding.get("layer_id")
            predecessor = layers_by_id.get(input_id)
            if predecessor is None:
                issues.append((layer_ref, "input layer is absent or occurs after its successor"))
                continue
            predecessor_ref = record_ref_by_id[input_id]
            predecessor_path = repo_root / predecessor_ref
            if (
                input_binding.get("record_ref") != predecessor_ref
                or input_binding.get("record_sha256") != _sha256(predecessor_path)
                or input_binding.get("content_sha256") != predecessor["representation"]["content_sha256"]
            ):
                issues.append((layer_ref, "input layer digest-bound closure drifted"))
                continue
            payload = layer["derivation"]["change_payload"]
            if payload.get("kind") == "explicit_operations":
                try:
                    _replay_source_text_layer_edits(
                        selected_text_by_id[input_id],
                        selected_text,
                        payload["operations"],
                    )
                except (KeyError, TypeError, ValueError) as exc:
                    issues.append((layer_ref, f"explicit change replay failed: {exc}"))
        if input_layers and layer.get("supersedes_layer_ref") != input_layers[-1].get("layer_id"):
            issues.append((layer_ref, "successor does not name its immediate input as superseded"))
        if layer.get("derivation", {}).get("method") == "unicode_normalization" and input_layers:
            predecessor_text = selected_text_by_id.get(input_layers[-1].get("layer_id"))
            if predecessor_text is not None and unicodedata.normalize("NFC", predecessor_text) != selected_text:
                issues.append((layer_ref, "declared NFC derivation does not reproduce the output"))
        if variant_id == "B" and selected_text != report.get("source_anchor_selection"):
            issues.append((layer_ref, "diplomatic candidate differs from the exact source-anchor selection"))

        layers_by_id[layer_id] = layer
        record_ref_by_id[layer_id] = layer_ref
        selected_text_by_id[layer_id] = selected_text
        layers_by_variant[variant_id] = layer
        report["variants"].append(
            {
                "variant_id": variant_id,
                "layer_id": layer_id,
                "layer_role": layer.get("layer_role"),
                "text": selected_text,
                "text_sha256": _sha256_text(selected_text),
                "review_status": layer.get("admission", {}).get("review_status"),
                "accepted_uses": layer.get("admission", {}).get("accepted_uses"),
                "publication_authorized": layer.get("representation", {}).get(
                    "publication_authorized"
                ),
                "rights_record_refs": layer.get("representation", {}).get(
                    "rights_record_refs"
                ),
            }
        )

    def negative_rejected(name: str, action: Any) -> None:
        try:
            action()
        except (KeyError, TypeError, ValueError):
            report["negative_controls"][name] = "rejected"
        else:
            issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), f"negative control was not rejected: {name}"))
            report["negative_controls"][name] = "not_rejected"

    def schema_reject(payload: dict[str, Any]) -> None:
        if list(layer_validator.iter_errors(payload)):
            raise ValueError("schema rejected negative control")

    if set(layers_by_variant) == {"A", "B", "C"}:
        def silent_correction() -> None:
            mutated = copy.deepcopy(layers_by_variant["B"])
            mutated["derivation"]["change_payload"] = {"kind": "none"}
            schema_reject(mutated)

        def input_record_drift() -> None:
            mutated = copy.deepcopy(layers_by_variant["B"])
            binding = mutated["derivation"]["input_layers"][0]
            if binding["record_sha256"] != _sha256(repo_root / binding["record_ref"]):
                raise ValueError("input record digest drift rejected")
            binding["record_sha256"] = "0" * 64
            if binding["record_sha256"] != _sha256(repo_root / binding["record_ref"]):
                raise ValueError("input record digest drift rejected")

        def edit_mismatch() -> None:
            mutated = copy.deepcopy(layers_by_variant["B"])
            operation = mutated["derivation"]["change_payload"]["operations"][0]
            operation["output_sha256"] = "0" * 64
            messages = _source_text_layer_semantic_issues(mutated)
            if any("output text digest drifted" in message for message in messages):
                raise ValueError("edit digest mismatch rejected")

        def unreviewed_accepted_use() -> None:
            mutated = copy.deepcopy(layers_by_variant["B"])
            mutated["admission"]["accepted_uses"] = ["citation"]
            schema_reject(mutated)

        def normalized_diplomatic_use() -> None:
            mutated = copy.deepcopy(layers_by_variant["C"])
            mutated["admission"].update(
                {
                    "review_status": "accepted",
                    "review_ref": "tos.review.synthetic.source-text-layer",
                    "human_review_performed": True,
                    "accepted_uses": ["diplomatic_transcription"],
                }
            )
            schema_reject(mutated)

        def language_use_without_competence() -> None:
            mutated = copy.deepcopy(layers_by_variant["B"])
            mutated["admission"].update(
                {
                    "review_status": "accepted_with_limits",
                    "review_ref": "tos.review.synthetic.source-text-layer",
                    "human_review_performed": True,
                    "human_language_competence": "blocked",
                    "accepted_uses": ["translation_source"],
                }
            )
            schema_reject(mutated)

        def tracked_nonpublic_text() -> None:
            mutated = copy.deepcopy(layers_by_variant["B"])
            mutated["representation"]["content_visibility"] = "local_only"
            mutated["representation"]["publication_authorized"] = False
            messages = _source_text_layer_semantic_issues(mutated)
            if "tracked source text layer must be public content" in messages:
                raise ValueError("tracked nonpublic source text rejected")

        def publication_without_authority() -> None:
            mutated = copy.deepcopy(layers_by_variant["B"])
            mutated["representation"]["publication_authorized"] = False
            mutated["admission"].update(
                {
                    "review_status": "accepted",
                    "review_ref": "tos.review.synthetic.source-text-layer",
                    "human_review_performed": True,
                    "accepted_uses": ["publication"],
                }
            )
            schema_reject(mutated)

        def self_supersession() -> None:
            mutated = copy.deepcopy(layers_by_variant["B"])
            mutated["supersedes_layer_ref"] = mutated["layer_id"]
            if "source text layer cannot supersede itself" in _source_text_layer_semantic_issues(mutated):
                raise ValueError("self supersession rejected")

        negative_rejected("silent_correction_without_operations", silent_correction)
        negative_rejected("input_record_digest_drift", input_record_drift)
        negative_rejected("edit_span_or_digest_mismatch", edit_mismatch)
        negative_rejected("unreviewed_layer_claims_accepted_use", unreviewed_accepted_use)
        negative_rejected("normalized_layer_claims_diplomatic_use", normalized_diplomatic_use)
        negative_rejected("language_sensitive_use_without_competence", language_use_without_competence)
        negative_rejected("tracked_nonpublic_explicit_text", tracked_nonpublic_text)
        negative_rejected("publication_use_without_publication_authority", publication_without_authority)
        negative_rejected("self_supersession", self_supersession)

    if set(manifest.get("negative_controls", [])) != set(report["negative_controls"]):
        issues.append((SOURCE_TEXT_LAYER_LAB_MANIFEST.as_posix(), "negative-control coverage differs from manifest"))
    return issues, report


def _canonical_json_value_sha256(value: Any) -> str:
    """Hash the lab's declared compact UTF-8 JSON value representation."""

    return hashlib.sha256(
        json.dumps(
            value,
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode("utf-8")
    ).hexdigest()


def _provenance_v2_semantic_issues(event: dict[str, Any]) -> list[str]:
    """Return v2 cross-field defects that JSON Schema cannot express."""

    messages: list[str] = []
    event_id = event.get("event_id")
    if event.get("supersedes_event_ref") == event_id:
        messages.append("provenance event cannot supersede itself")

    activity = event.get("activity", {})
    try:
        started = datetime.fromisoformat(activity["started_at"])
        ended = datetime.fromisoformat(activity["ended_at"])
        if ended < started:
            messages.append("provenance activity ends before it starts")
    except (KeyError, TypeError, ValueError):
        messages.append("provenance activity timestamps are not comparable")

    status = activity.get("status")
    exit_code = activity.get("exit_code")
    terminal_reason = activity.get("terminal_reason")
    if status in {"completed", "completed_with_warnings"}:
        if exit_code != 0:
            messages.append("completed provenance activity has a non-zero or absent exit code")
        if terminal_reason is not None:
            messages.append("completed provenance activity carries a terminal failure reason")
    if status in {"failed", "stopped"}:
        if exit_code in {None, 0}:
            messages.append("failed or stopped provenance activity lacks a non-zero exit code")
        if not isinstance(terminal_reason, str) or not terminal_reason:
            messages.append("failed or stopped provenance activity lacks a terminal reason")

    entities = event.get("entities", {})
    groups = {
        name: entries if isinstance(entries, list) else []
        for name, entries in (
            ("inputs", entities.get("inputs", [])),
            ("outputs", entities.get("outputs", [])),
            ("byproducts", entities.get("byproducts", [])),
        )
    }
    refs_by_group: dict[str, set[str]] = {}
    entity_by_ref: dict[str, dict[str, Any]] = {}
    for group_name, entries in groups.items():
        refs: list[str] = []
        for entity in entries:
            if not isinstance(entity, dict):
                continue
            ref = entity.get("entity_ref")
            if isinstance(ref, str):
                refs.append(ref)
                entity_by_ref[ref] = entity
            if entity.get("fixity_verified") is True and entity.get("fixity_verified_at") is None:
                messages.append(f"{group_name} entity claims fixity without verification time")
            if entity.get("fixity_verified") is False and entity.get("fixity_verified_at") is not None:
                messages.append(f"{group_name} entity has verification time while fixity is false")
        if len(refs) != len(set(refs)):
            messages.append(f"duplicate entity reference inside {group_name}")
        refs_by_group[group_name] = set(refs)
    if refs_by_group["inputs"] & refs_by_group["outputs"]:
        messages.append("input and output entity identities collapse")
    if refs_by_group["outputs"] & refs_by_group["byproducts"]:
        messages.append("authoritative output and byproduct identities collapse")

    covered_outputs: set[str] = set()
    for derivation in event.get("derivations", []):
        if not isinstance(derivation, dict):
            continue
        input_ref = derivation.get("input_entity_ref")
        output_ref = derivation.get("output_entity_ref")
        if input_ref not in refs_by_group["inputs"]:
            messages.append("derivation input leaves the declared input entities")
        if output_ref not in refs_by_group["outputs"]:
            messages.append("derivation output leaves the authoritative output entities")
        else:
            covered_outputs.add(output_ref)
        if input_ref == output_ref:
            messages.append("derivation collapses input and output identity")
        if derivation.get("relation") == "identity_copy_of":
            input_entity = entity_by_ref.get(str(input_ref), {})
            output_entity = entity_by_ref.get(str(output_ref), {})
            if (
                input_entity.get("sha256") != output_entity.get("sha256")
                or input_entity.get("size_bytes") != output_entity.get("size_bytes")
            ):
                messages.append("identity copy does not preserve exact bytes and size")
    missing_derivations = refs_by_group["outputs"] - covered_outputs
    if missing_derivations:
        messages.append("authoritative output lacks an explicit derivation")
    if not refs_by_group["outputs"] and event.get("derivations"):
        messages.append("event without authoritative output still asserts derivation")

    method = event.get("method", {})
    command = method.get("command_capture", {})
    argv = command.get("argv")
    if command.get("disclosure") == "inline" and isinstance(argv, list):
        if command.get("argv_sha256") != _canonical_json_value_sha256(argv):
            messages.append("inline command argv digest drifted")

    output_digests = {
        entity.get("sha256")
        for entity in groups["outputs"]
        if isinstance(entity, dict)
    }
    for invocation in method.get("model_invocations", []):
        if not isinstance(invocation, dict):
            continue
        prompt = invocation.get("prompt_capture", {})
        if prompt.get("disclosure") == "inline" and isinstance(prompt.get("text"), str):
            if prompt.get("sha256") != _sha256_text(prompt["text"]):
                messages.append("inline model prompt digest drifted")
        if invocation.get("response_status") == "completed":
            if invocation.get("output_sha256") not in output_digests:
                messages.append("completed model invocation output is not an event output")

    authentication = event.get("evidence_authentication", {})
    signature_status = authentication.get("signature_status")
    signature_bindings = authentication.get("signature_bindings", [])
    verification_status = authentication.get("verification_status")
    if signature_status == "unsigned":
        if signature_bindings:
            messages.append("unsigned provenance event carries signature bindings")
        if verification_status == "signature_verified":
            messages.append("unsigned provenance event claims signature verification")
    if signature_status in {"signed_unverified", "signed_verified"} and not signature_bindings:
        messages.append("signed provenance event lacks signature bindings")

    reproducibility = event.get("reproducibility", {})
    if reproducibility.get("classification") in {
        "replay_ready",
        "replay_ready_negative_control",
    }:
        if command.get("disclosure") != "inline":
            messages.append("replay-ready provenance withholds its command")
        if event.get("manual_changes", {}).get("status") == "unknown":
            messages.append("replay-ready provenance has unknown manual changes")
        if any(
            component.get("verification_status") != "verified"
            for component in method.get("software_components", [])
            if isinstance(component, dict)
        ):
            messages.append("replay-ready provenance has an unverified software component")
    return messages


def validate_provenance_v2_lab(repo_root: Path) -> tuple[list[Issue], dict[str, Any]]:
    """Validate exact A/B/C execution receipts and their real synthetic bytes."""

    repo_root = repo_root.resolve()
    issues: list[Issue] = []
    report: dict[str, Any] = {"variants": [], "negative_controls": {}}
    try:
        event_validator, _ = _schema_validator(PROVENANCE_V2_SCHEMA, repo_root)
    except (OSError, json.JSONDecodeError, SchemaError) as exc:
        return [(PROVENANCE_V2_SCHEMA.as_posix(), f"cannot load provenance v2 schema: {exc}")], report

    manifest_path = repo_root / PROVENANCE_V2_LAB_MANIFEST
    manifest = _load_json(manifest_path, repo_root, issues)
    if manifest is None:
        return issues, report
    if manifest.get("schema_version") != "tos_provenance_event_v2_lab_v1":
        issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "unexpected provenance v2 lab manifest version"))
    if manifest.get("authority_posture") != "public_synthetic_mechanics_only":
        issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "provenance v2 lab authority posture widened"))
    expected_limits = {
        "private_source_used": False,
        "model_invoked": False,
        "human_evidence_created": False,
        "execution_truth_established": False,
        "content_truth_established": False,
        "rights_clearance_established": False,
        "semantic_claim_created": False,
        "canon_effect": False,
    }
    if manifest.get("authority_limits") != expected_limits:
        issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "provenance v2 lab authority limits widened"))

    def binding_closes(binding: Any, label: str) -> bool:
        if not isinstance(binding, dict):
            issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), f"{label} binding is absent"))
            return False
        ref = binding.get("ref")
        digest = binding.get("sha256")
        path = repo_root / ref if isinstance(ref, str) else manifest_path
        if path.is_symlink() or not path.is_file() or _sha256(path) != digest:
            issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), f"{label} binding drifted"))
            return False
        return True

    for field in ("contract", "plan", "builder", "environment_profile", "input_fixture"):
        binding_closes(manifest.get(field), field)
    if manifest.get("contract", {}).get("ref") != PROVENANCE_V2_SCHEMA.as_posix():
        issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "provenance v2 contract reference drifted"))

    events_by_variant: dict[str, dict[str, Any]] = {}
    manifest_variants = manifest.get("variants", [])
    if not isinstance(manifest_variants, list):
        issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "provenance variants are not a list"))
        manifest_variants = []
    variant_manifest_by_id = {
        variant.get("variant_id"): variant
        for variant in manifest_variants
        if isinstance(variant, dict) and isinstance(variant.get("variant_id"), str)
    }
    if [variant.get("variant_id") for variant in manifest_variants if isinstance(variant, dict)] != ["A", "B", "C"]:
        issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "provenance variants must be ordered exactly A, B, C"))
    if len(variant_manifest_by_id) != len(manifest_variants):
        issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "provenance variant identity is duplicated or missing"))
    for variant in manifest_variants:
        if not isinstance(variant, dict):
            issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "provenance variant is not an object"))
            continue
        variant_id = variant.get("variant_id")
        event_ref = variant.get("event_ref")
        if variant_id not in {"A", "B", "C"} or not isinstance(event_ref, str):
            issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "provenance variant identity is invalid"))
            continue
        event_path = repo_root / event_ref
        if event_path.is_symlink() or not event_path.is_file() or _sha256(event_path) != variant.get("event_sha256"):
            issues.append((event_ref, "provenance event record fixity drifted"))
            continue
        event = _load_json(event_path, repo_root, issues)
        if event is None:
            continue
        _validate_payload(event, event_validator, event_ref, issues)
        for message in _provenance_v2_semantic_issues(event):
            issues.append((event_ref, message))
        if event.get("record_binding") != {
            "manifest_ref": PROVENANCE_V2_LAB_MANIFEST.as_posix(),
            "digest_algorithm": "sha256",
            "digest_scope": "exact_event_record_bytes",
        }:
            issues.append((event_ref, "event-to-manifest record binding drifted"))
        activity = event.get("activity", {})
        if (
            activity.get("status") != variant.get("expected_status")
            or activity.get("exit_code") != variant.get("expected_exit_code")
        ):
            issues.append((event_ref, "event terminal state differs from the frozen expectation"))
        command = event.get("method", {}).get("command_capture", {})
        expected_command = [
            "python",
            "scripts/build_provenance_event_v2_lab.py",
            "--variant",
            variant_id,
        ]
        if command.get("argv") != expected_command or variant.get("captured_command") != expected_command:
            issues.append((event_ref, "captured argv differs from the executed laboratory command"))

        input_ref = variant.get("input_ref")
        input_path = repo_root / input_ref if isinstance(input_ref, str) else manifest_path
        if not input_path.is_file() or _sha256(input_path) != variant.get("input_sha256"):
            issues.append((event_ref, "variant input fixity drifted"))
        input_entities = event.get("entities", {}).get("inputs", [])
        if len(input_entities) != 1 or input_entities[0].get("entity_ref") != input_ref or input_entities[0].get("sha256") != variant.get("input_sha256"):
            issues.append((event_ref, "event input binding differs from the manifest"))

        output_ref = variant.get("output_ref")
        byproduct_ref = variant.get("byproduct_ref")
        outputs = event.get("entities", {}).get("outputs", [])
        byproducts = event.get("entities", {}).get("byproducts", [])
        if isinstance(output_ref, str):
            output_path = repo_root / output_ref
            if not output_path.is_file() or _sha256(output_path) != variant.get("output_sha256"):
                issues.append((event_ref, "variant output fixity drifted"))
            if len(outputs) != 1 or outputs[0].get("entity_ref") != output_ref or outputs[0].get("sha256") != variant.get("output_sha256"):
                issues.append((event_ref, "event output binding differs from the manifest"))
        elif outputs:
            issues.append((event_ref, "failed variant exposes an authoritative output"))
        if isinstance(byproduct_ref, str):
            byproduct_path = repo_root / byproduct_ref
            if not byproduct_path.is_file() or _sha256(byproduct_path) != variant.get("byproduct_sha256"):
                issues.append((event_ref, "variant byproduct fixity drifted"))
            if len(byproducts) != 1 or byproducts[0].get("entity_ref") != byproduct_ref or byproducts[0].get("sha256") != variant.get("byproduct_sha256"):
                issues.append((event_ref, "event byproduct binding differs from the manifest"))
        elif byproducts:
            issues.append((event_ref, "successful variant unexpectedly carries a failure byproduct"))

        relation = variant.get("expected_relation")
        relations = [row.get("relation") for row in event.get("derivations", [])]
        if relations != ([] if relation is None else [relation]):
            issues.append((event_ref, "event derivation relation differs from the frozen expectation"))
        if event.get("method", {}).get("model_invocations") != []:
            issues.append((event_ref, "public synthetic provenance lab invoked a model"))
        if any(actor.get("agent_kind") == "human" for actor in event.get("responsibility", [])):
            issues.append((event_ref, "public synthetic provenance lab fabricated human evidence"))
        if event.get("review_and_authority", {}).get("human_review_status") != "not_performed":
            issues.append((event_ref, "public synthetic provenance lab claims human review"))
        if event.get("rights_and_visibility", {}).get("publication_authorized") is not False:
            issues.append((event_ref, "public synthetic provenance lab widened publication authority"))
        events_by_variant[variant_id] = event
        report["variants"].append(
            {
                "variant_id": variant_id,
                "status": activity.get("status"),
                "exit_code": activity.get("exit_code"),
                "input_sha256": variant.get("input_sha256"),
                "output_sha256": variant.get("output_sha256"),
                "byproduct_sha256": variant.get("byproduct_sha256"),
                "relation": relation,
                "signature_status": event.get("evidence_authentication", {}).get("signature_status"),
                "human_review_status": event.get("review_and_authority", {}).get("human_review_status"),
            }
        )

    if set(events_by_variant) == {"A", "B", "C"} and set(variant_manifest_by_id) == {"A", "B", "C"}:
        input_bytes = (repo_root / manifest["input_fixture"]["ref"]).read_bytes()
        a_bytes = (repo_root / variant_manifest_by_id["A"]["output_ref"]).read_bytes()
        b_bytes = (repo_root / variant_manifest_by_id["B"]["output_ref"]).read_bytes()
        c_bytes = (repo_root / variant_manifest_by_id["C"]["byproduct_ref"]).read_bytes()
        if a_bytes != input_bytes:
            issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "variant A is not a byte-identical copy"))
        try:
            source_text = input_bytes.decode("utf-8")
            b_text = b_bytes.decode("utf-8")
        except UnicodeError as exc:
            issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), f"synthetic text is not UTF-8: {exc}"))
        else:
            if b_text != unicodedata.normalize("NFC", source_text) or b_bytes == input_bytes:
                issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "variant B does not preserve the exact NFC transformation"))
        if c_bytes != b"status=failed\nreason=non_ascii_input\nexit_code=7\n":
            issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "variant C failure byproduct drifted"))

    def negative_rejected(name: str, action: Any) -> None:
        try:
            action()
        except (KeyError, TypeError, ValueError):
            report["negative_controls"][name] = "rejected"
        else:
            report["negative_controls"][name] = "not_rejected"
            issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), f"negative control was not rejected: {name}"))

    def schema_reject(payload: dict[str, Any]) -> None:
        if list(event_validator.iter_errors(payload)):
            raise ValueError("schema rejected negative control")

    if set(events_by_variant) == {"A", "B", "C"}:
        def event_record_digest_drift() -> None:
            first = variant_manifest_by_id["A"]
            if _sha256(repo_root / first["event_ref"]) != "0" * 64:
                raise ValueError("event record digest drift rejected")

        def input_fixity_drift() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["entities"]["inputs"][0]["sha256"] = "0" * 64
            input_path = repo_root / mutated["entities"]["inputs"][0]["entity_ref"]
            if mutated["entities"]["inputs"][0]["sha256"] != _sha256(input_path):
                raise ValueError("input fixity drift rejected")

        def command_digest_drift() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["method"]["command_capture"]["argv_sha256"] = "0" * 64
            if "inline command argv digest drifted" in _provenance_v2_semantic_issues(mutated):
                raise ValueError("command digest drift rejected")

        def completed_without_output() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["entities"]["outputs"] = []
            mutated["derivations"] = []
            schema_reject(mutated)

        def failed_with_output() -> None:
            mutated = copy.deepcopy(events_by_variant["C"])
            mutated["entities"]["outputs"] = copy.deepcopy(events_by_variant["A"]["entities"]["outputs"])
            schema_reject(mutated)

        def identity_copy_drift() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["entities"]["outputs"][0]["sha256"] = "0" * 64
            if "identity copy does not preserve exact bytes and size" in _provenance_v2_semantic_issues(mutated):
                raise ValueError("identity-copy content drift rejected")

        def derivation_escape() -> None:
            mutated = copy.deepcopy(events_by_variant["B"])
            mutated["derivations"][0]["output_entity_ref"] = "outside:event-output"
            if "derivation output leaves the authoritative output entities" in _provenance_v2_semantic_issues(mutated):
                raise ValueError("derivation endpoint escape rejected")

        def model_without_invocation() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["activity"]["event_type"] = "model_inference"
            schema_reject(mutated)

        def replay_withheld_command() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["method"]["command_capture"].update(
                {"disclosure": "withheld_digest_only", "argv": None, "withholding_reason": "synthetic negative"}
            )
            if "replay-ready provenance withholds its command" in _provenance_v2_semantic_issues(mutated):
                raise ValueError("withheld replay command rejected")

        def unsigned_signature_claim() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["evidence_authentication"]["verification_status"] = "signature_verified"
            if "unsigned provenance event claims signature verification" in _provenance_v2_semantic_issues(mutated):
                raise ValueError("unsigned signature claim rejected")

        def publication_without_authority() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["rights_and_visibility"]["publication_authorized"] = True
            schema_reject(mutated)

        def unattested_human_review() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["review_and_authority"]["human_review_status"] = "performed"
            mutated["review_and_authority"]["review_bindings"] = [manifest["plan"]]
            schema_reject(mutated)

        def manual_change_without_receipt() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["manual_changes"]["status"] = "recorded"
            schema_reject(mutated)

        def self_supersession() -> None:
            mutated = copy.deepcopy(events_by_variant["A"])
            mutated["supersedes_event_ref"] = mutated["event_id"]
            if "provenance event cannot supersede itself" in _provenance_v2_semantic_issues(mutated):
                raise ValueError("self supersession rejected")

        negative_rejected("event_record_digest_drift", event_record_digest_drift)
        negative_rejected("input_fixity_drift", input_fixity_drift)
        negative_rejected("command_digest_drift", command_digest_drift)
        negative_rejected("completed_without_output", completed_without_output)
        negative_rejected("failed_with_authoritative_output", failed_with_output)
        negative_rejected("identity_copy_content_drift", identity_copy_drift)
        negative_rejected("derivation_endpoint_escape", derivation_escape)
        negative_rejected("model_event_without_invocation", model_without_invocation)
        negative_rejected("replay_ready_with_withheld_command", replay_withheld_command)
        negative_rejected("unsigned_claimed_signature_verification", unsigned_signature_claim)
        negative_rejected("publication_without_authority", publication_without_authority)
        negative_rejected("unattested_human_review", unattested_human_review)
        negative_rejected("manual_change_without_receipt", manual_change_without_receipt)
        negative_rejected("self_supersession", self_supersession)

    if set(manifest.get("negative_controls", [])) != set(report["negative_controls"]):
        issues.append((PROVENANCE_V2_LAB_MANIFEST.as_posix(), "negative-control coverage differs from manifest"))
    return issues, report


def _latest_event_in_supersession_lineage(
    events_by_id: dict[str, dict[str, Any]],
    base_event_id: str,
    *,
    location: str,
    issues: list[Issue],
) -> dict[str, Any] | None:
    """Resolve one unambiguous provenance lineage without rewriting history."""

    current = events_by_id.get(base_event_id)
    if current is None:
        return None
    seen = {base_event_id}
    while True:
        current_id = current.get("event_id")
        successors = [
            event
            for event in events_by_id.values()
            if event.get("supersedes_event_ref") == current_id
        ]
        if not successors:
            return current
        if len(successors) != 1:
            issues.append(
                (
                    location,
                    f"ambiguous provenance supersession from {current_id}",
                )
            )
            return None
        current = successors[0]
        next_id = current.get("event_id")
        if not isinstance(next_id, str) or next_id in seen:
            issues.append((location, "cyclic provenance supersession lineage"))
            return None
        seen.add(next_id)


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


def _public_evidence_derivative_issues(
    derivative: object,
    *,
    handoff: object,
) -> list[str]:
    if not isinstance(derivative, dict):
        return ["public evidence derivative is not an object"]
    if not isinstance(handoff, dict):
        return ["public evidence derivative has no handoff object"]
    issues: list[str] = []
    for value in _string_values(derivative):
        if value.startswith(("/", "~/")) or "/home/" in value or "/srv/" in value:
            issues.append("public evidence derivative contains an absolute local path")
            break

    source_boundary = handoff.get("source_boundary", {})
    derivative_boundary = derivative.get("source_boundary", {})
    comparisons = (
        ("handoff_id", derivative.get("handoff_id"), handoff.get("handoff_id")),
        (
            "evidence_set_id",
            derivative_boundary.get("evidence_set_id"),
            source_boundary.get("evidence_set_id"),
        ),
        (
            "public_return_handle",
            derivative_boundary.get("public_return_handle"),
            source_boundary.get("public_return_handle"),
        ),
    )
    for label, actual, expected in comparisons:
        if actual != expected:
            issues.append(
                f"public evidence derivative {label} does not match its handoff"
            )

    policy = handoff.get("disclosure_policy", {})
    disclosed = set(derivative.get("disclosed_classes", []))
    allowed = set(policy.get("allowed_classes", []))
    unexpected = sorted(disclosed - allowed)
    if unexpected:
        issues.append(
            f"public evidence derivative discloses non-allowed classes: {unexpected}"
        )
    aggregation = derivative.get("aggregation", {})
    derivative_floor = aggregation.get("minimum_group_size_applied")
    handoff_floor = policy.get("minimum_aggregation_group_size")
    if (
        isinstance(derivative_floor, int)
        and isinstance(handoff_floor, int)
        and derivative_floor < handoff_floor
    ):
        issues.append(
            "public evidence derivative aggregation floor is below its handoff"
        )
    forbidden = set(policy.get("forbidden_classes", []))
    leaked_forbidden_labels = sorted(
        value
        for value in _string_values(derivative)
        if value in forbidden
    )
    if leaked_forbidden_labels:
        issues.append(
            "public evidence derivative names forbidden disclosure classes: "
            f"{leaked_forbidden_labels}"
        )
    return issues


def _manual_error_ledger_issues(
    records: object,
    *,
    handoff: object,
    derivative: object,
    provenance_events: object,
    repo_root: Path,
) -> list[str]:
    """Cross-close the bounded aggregate human-review episode.

    These checks preserve declared evidence and reference closure. They do not
    independently prove who performed the review or that any source text is
    correct.
    """

    if not isinstance(records, list):
        return ["manual error ledger records are not a list"]
    if not isinstance(handoff, dict) or not isinstance(derivative, dict):
        return ["manual error ledger has no handoff or derivative object"]
    if not isinstance(provenance_events, list):
        return ["manual error ledger provenance events are not a list"]

    issues: list[str] = []
    if len(records) != 2:
        issues.append("manual error ledger must preserve one state and one review episode")
        return issues
    if records[0].get("record_type") != "ledger_state":
        issues.append("manual error ledger first record is not the historical state")
    if records[1].get("record_type") != "review_episode":
        issues.append("manual error ledger second record is not the review episode")
        return issues

    episode = records[1]
    for value in _string_values([episode, provenance_events]):
        if value.startswith(("/", "~/")) or "/home/" in value or "/srv/" in value:
            issues.append("manual error ledger evidence contains an absolute local path")
            break

    handoff_ref = PRIVATE_EVIDENCE_HANDOFF_PROFILE.as_posix()
    derivative_ref = handoff.get("destination", {}).get("artifact_path")
    evidence_boundary = episode.get("evidence_boundary", {})
    expected_refs = (
        (
            "handoff",
            evidence_boundary.get("handoff"),
            handoff_ref,
            _sha256(repo_root / PRIVATE_EVIDENCE_HANDOFF_PROFILE),
        ),
        (
            "aggregate derivative",
            evidence_boundary.get("aggregate_derivative"),
            derivative_ref,
            _sha256(repo_root / derivative_ref)
            if isinstance(derivative_ref, str)
            else None,
        ),
    )
    for label, digest_ref, expected_ref, expected_sha in expected_refs:
        if not isinstance(digest_ref, dict):
            issues.append(f"manual error ledger {label} reference is missing")
            continue
        if digest_ref.get("ref") != expected_ref or digest_ref.get("sha256") != expected_sha:
            issues.append(f"manual error ledger {label} reference or digest drifted")

    source_boundary = handoff.get("source_boundary", {})
    boundary_pairs = (
        (
            "private evidence set",
            evidence_boundary.get("private_evidence_set_id"),
            source_boundary.get("evidence_set_id"),
        ),
        (
            "private return handle",
            evidence_boundary.get("private_return_handle"),
            source_boundary.get("public_return_handle"),
        ),
    )
    for label, actual, expected in boundary_pairs:
        if actual != expected:
            issues.append(f"manual error ledger {label} does not match handoff")

    review_scope = episode.get("review_scope", {})
    aggregation = derivative.get("aggregation", {})
    if review_scope.get("source_unit_count") != aggregation.get("source_unit_count"):
        issues.append("manual error ledger source-unit count does not match derivative")
    if review_scope.get("candidate_observation_count") != aggregation.get(
        "candidate_observation_count"
    ):
        issues.append(
            "manual error ledger candidate-observation count does not match derivative"
        )
    if review_scope.get("source_visible") != derivative.get("evidence_posture", {}).get(
        "human_source_visible_review_observed"
    ):
        issues.append("manual error ledger source-visible posture does not match derivative")
    if episode.get("experiment_id") != derivative.get("public_experiment_id"):
        issues.append("manual error ledger experiment does not match derivative")
    if episode.get("aggregate_outcome_counts") != derivative.get(
        "aggregate_outcome_counts"
    ):
        issues.append("manual error ledger aggregate outcomes do not match derivative")
    if episode.get("aggregate_error_taxonomy") != derivative.get(
        "aggregate_error_taxonomy"
    ):
        issues.append("manual error ledger error taxonomy does not match derivative")
    if episode.get("human_time") != derivative.get(
        "aggregate_human_time_with_confounds"
    ):
        issues.append("manual error ledger human time does not match derivative")

    decision_total = sum(
        row.get("count", 0)
        for row in episode.get("aggregate_outcome_counts", [])
        if isinstance(row, dict)
        and isinstance(row.get("code"), str)
        and row["code"].startswith("decision-")
        and isinstance(row.get("count"), int)
    )
    if decision_total != review_scope.get("candidate_observation_count"):
        issues.append("manual error ledger decision counts do not close to observations")

    adjudication = episode.get("adjudication", {})
    if adjudication.get("candidate_dispositions_recorded") != review_scope.get(
        "candidate_observation_count"
    ):
        issues.append("manual error ledger dispositions do not close to observations")
    closed_adjudication = {
        "source_transcriptions_accepted": 0,
        "independent_gold_units": 0,
        "general_method_winner": False,
        "content_authority": False,
        "routine_human_backlog_created": False,
    }
    for field, expected in closed_adjudication.items():
        if adjudication.get(field) != expected:
            issues.append(f"manual error ledger improperly opens {field}")

    if len(provenance_events) != 1:
        issues.append("manual error ledger must have exactly one provenance event")
        return issues
    event = provenance_events[0]
    if event.get("event_id") != episode.get("provenance_event_ref"):
        issues.append("manual error ledger provenance event reference drifted")
    if event.get("event_type") != "export":
        issues.append("manual error ledger provenance event is not an export")
    if event.get("status") != "completed_with_warnings":
        issues.append("manual error ledger provenance must retain warnings")
    if event.get("rights_basis_ref") is not None:
        issues.append("manual error ledger provenance claims a rights basis")

    input_pairs = {
        (item.get("ref"), item.get("sha256"))
        for item in event.get("inputs", [])
        if isinstance(item, dict)
    }
    expected_input_pairs = {
        (handoff_ref, _sha256(repo_root / PRIVATE_EVIDENCE_HANDOFF_PROFILE)),
        (derivative_ref, _sha256(repo_root / derivative_ref))
        if isinstance(derivative_ref, str)
        else (None, None),
        (MANUAL_ERROR_LEDGER_PATH.as_posix(), PRIOR_EMPTY_MANUAL_ERROR_LEDGER_SHA256),
    }
    if input_pairs != expected_input_pairs:
        issues.append("manual error ledger provenance inputs or digests drifted")

    output_pairs = {
        (item.get("ref"), item.get("sha256"))
        for item in event.get("outputs", [])
        if isinstance(item, dict)
    }
    expected_output_pairs = {
        (
            MANUAL_ERROR_LEDGER_PATH.as_posix(),
            _sha256(repo_root / MANUAL_ERROR_LEDGER_PATH),
        )
    }
    if output_pairs != expected_output_pairs:
        issues.append("manual error ledger provenance output digest drifted")

    configuration = event.get("method", {}).get("configuration", {})
    expected_configuration = {
        "aggregate_only": True,
        "candidate_observation_count": 30,
        "human_review_pass_count": 1,
        "private_raw_reopened": False,
        "source_unit_count": 10,
        "unit_level_judgments_embedded": False,
    }
    if configuration != expected_configuration:
        issues.append("manual error ledger provenance configuration drifted")
    return issues


def _transfer_candidate_crosswalk_issues(
    crosswalk: object,
    *,
    transfer_plan: object,
    target_map: object,
    label_map: object,
) -> list[str]:
    if not all(
        isinstance(value, dict)
        for value in (crosswalk, transfer_plan, target_map, label_map)
    ):
        return ["transfer candidate crosswalk inputs are not objects"]
    issues: list[str] = []
    work_ref = crosswalk.get("work_ref")
    expected_candidates = {
        candidate.get("unit_id"): candidate
        for candidate in transfer_plan.get("candidate_target_units", [])
        if isinstance(candidate, dict) and candidate.get("work_ref") == work_ref
    }
    pairings = {
        pairing.get("unit_key"): pairing
        for pairing in label_map.get("pairings", [])
        if isinstance(pairing, dict)
    }
    starts = [
        start
        for start in target_map.get("unit_starts", [])
        if isinstance(start, dict)
        and isinstance(start.get("pdf_page"), int)
        and isinstance(start.get("unit_key"), str)
    ]
    actual_candidates = crosswalk.get("candidates", [])
    if not isinstance(actual_candidates, list):
        return ["transfer candidate crosswalk candidates are not a list"]
    actual_ids = [
        candidate.get("candidate_unit_id")
        for candidate in actual_candidates
        if isinstance(candidate, dict)
    ]
    if len(actual_ids) != len(set(actual_ids)):
        issues.append("transfer candidate crosswalk repeats a candidate unit")
    if set(actual_ids) != set(expected_candidates):
        issues.append("transfer candidate crosswalk does not close the work quota")

    possible_pairing_count = 0
    pages_with_starts = 0
    for candidate in actual_candidates:
        if not isinstance(candidate, dict):
            issues.append("transfer candidate crosswalk contains a non-object row")
            continue
        unit_id = candidate.get("candidate_unit_id")
        expected = expected_candidates.get(unit_id)
        if expected is None:
            continue
        page = expected.get("page")
        if candidate.get("candidate_anchor_ref") != expected.get("anchor_ref"):
            issues.append(f"{unit_id} candidate anchor drifted")
        if candidate.get("target_pdf_page") != page:
            issues.append(f"{unit_id} candidate page drifted")
        if candidate.get("stratum") != expected.get("stratum"):
            issues.append(f"{unit_id} candidate stratum drifted")
        if not isinstance(page, int):
            issues.append(f"{unit_id} has no integer target page")
            continue
        prior = [start for start in starts if start.get("pdf_page", 0) < page]
        on_page = [start for start in starts if start.get("pdf_page") == page]
        following = [start for start in starts if start.get("pdf_page", 0) > page]
        expected_keys = (
            ([] if not prior else [prior[-1].get("unit_key")])
            + [start.get("unit_key") for start in on_page]
        )
        expected_start_keys = [start.get("unit_key") for start in on_page]
        if candidate.get("possible_unit_keys") != expected_keys:
            issues.append(f"{unit_id} possible unit keys drifted")
        if candidate.get("starts_on_page_unit_keys") != expected_start_keys:
            issues.append(f"{unit_id} on-page unit starts drifted")
        expected_relation = (
            "within-one-proposed-numbered-unit"
            if not on_page
            else "prior-unit-spill-plus-unit-starts"
        )
        if candidate.get("page_relation") != expected_relation:
            issues.append(f"{unit_id} page relation drifted")
        if not following:
            issues.append(f"{unit_id} has no following proposed start")
        else:
            expected_next = {
                "unit_key": following[0].get("unit_key"),
                "target_pdf_page": following[0].get("pdf_page"),
            }
            if candidate.get("next_proposed_start") != expected_next:
                issues.append(f"{unit_id} following proposed start drifted")
        for unit_key in expected_keys:
            pairing = pairings.get(unit_key)
            if pairing is None:
                issues.append(f"{unit_id} unit {unit_key} has no shared-label pairing")
            elif pairing.get("translation_alignment_claimed") is not False:
                issues.append(f"{unit_id} unit {unit_key} claims translation alignment")
        possible_pairing_count += len(expected_keys)
        pages_with_starts += bool(on_page)

    summary = crosswalk.get("summary", {})
    if not isinstance(summary, dict):
        issues.append("transfer candidate crosswalk summary is not an object")
        return issues
    expected_summary = {
        "candidate_page_count": len(expected_candidates),
        "random_page_count": sum(
            candidate.get("stratum") == "random"
            for candidate in expected_candidates.values()
        ),
        "hard_page_count": sum(
            candidate.get("stratum") == "hard"
            for candidate in expected_candidates.values()
        ),
        "page_with_unit_start_count": int(pages_with_starts),
        "page_without_unit_start_count": len(expected_candidates)
        - int(pages_with_starts),
        "possible_pairing_count": possible_pairing_count,
    }
    for field, expected_value in expected_summary.items():
        if summary.get(field) != expected_value:
            issues.append(f"transfer candidate crosswalk summary {field} drifted")
    return issues


def _hierarchical_target_numbered_unit_map_issues(payload: object) -> list[str]:
    if not isinstance(payload, dict):
        return ["hierarchical target numbered-unit map is not an object"]
    series_rows = payload.get("series", [])
    if not isinstance(series_rows, list):
        return ["hierarchical target numbered-unit series are not a list"]
    issues: list[str] = []
    series_keys: list[str] = []
    series_sequences: list[object] = []
    all_pages: list[int] = []
    all_anchor_refs: list[str] = []
    expected_override_refs: list[str] = []
    expected_override_pages: set[int] = set()
    expected_machine_count = 0
    method = payload.get("method", {})
    if not isinstance(method, dict):
        issues.append("hierarchical target map method is not an object")
        method = {}
    series_trials = method.get("series_trials", [])
    if not isinstance(series_trials, list):
        issues.append("hierarchical target map method trials are not a list")
        series_trials = []
    trial_by_key: dict[str, dict[str, Any]] = {}
    for trial in series_trials:
        if not isinstance(trial, dict):
            issues.append("hierarchical target map contains a non-object method trial")
            continue
        trial_key = trial.get("series_key")
        if not isinstance(trial_key, str):
            issues.append("hierarchical target map method trial has no string series key")
            continue
        if trial_key in trial_by_key:
            issues.append(f"hierarchical target map repeats method trial {trial_key}")
        trial_by_key[trial_key] = trial

    for expected_series_sequence, series in enumerate(series_rows, start=1):
        if not isinstance(series, dict):
            issues.append("hierarchical target map contains a non-object series")
            continue
        series_key = series.get("series_key")
        if not isinstance(series_key, str):
            issues.append("hierarchical target series key is not a string")
            continue
        series_keys.append(series_key)
        series_sequences.append(series.get("series_sequence"))
        if series.get("series_sequence") != expected_series_sequence:
            issues.append(f"hierarchical target series {series_key} sequence drifted")
        expected_count = series.get("expected_unit_count")
        starts = series.get("unit_starts", [])
        if not isinstance(starts, list):
            issues.append(f"hierarchical target series {series_key} starts are not a list")
            continue
        if not isinstance(expected_count, int) or len(starts) != expected_count:
            issues.append(f"hierarchical target series {series_key} count drifted")
            continue
        expected_keys = [str(number) for number in range(1, expected_count + 1)]
        actual_keys = [
            start.get("unit_key") if isinstance(start, dict) else None
            for start in starts
        ]
        actual_sequences = [
            start.get("sequence") if isinstance(start, dict) else None
            for start in starts
        ]
        if actual_keys != expected_keys:
            issues.append(f"hierarchical target series {series_key} unit keys drifted")
        if actual_sequences != list(range(1, expected_count + 1)):
            issues.append(f"hierarchical target series {series_key} unit sequences drifted")

        start_page = series.get("start_page")
        end_page = series.get("end_page")
        series_pages: list[int] = []
        series_override_keys: list[str] = []
        for start in starts:
            if not isinstance(start, dict):
                continue
            unit_key = start.get("unit_key")
            page = start.get("pdf_page")
            if not isinstance(page, int):
                continue
            series_pages.append(page)
            all_pages.append(page)
            anchor_ref = start.get("anchor_ref")
            if isinstance(anchor_ref, str):
                all_anchor_refs.append(anchor_ref)
            else:
                issues.append(
                    f"hierarchical target unit {series_key}:{unit_key} has no anchor ref"
                )
            if (
                not isinstance(start_page, int)
                or not isinstance(end_page, int)
                or not start_page <= page <= end_page
            ):
                issues.append(
                    f"hierarchical target unit {series_key}:{unit_key} leaves its series"
                )
            if start.get("resource_id") != f"pdf-page-{page:04d}":
                issues.append(
                    f"hierarchical target unit {series_key}:{unit_key} resource drifted"
                )
            if start.get("basis") == "source_visible_gap_review":
                series_override_keys.append(str(unit_key))
                expected_override_refs.append(f"{series_key}:{unit_key}")
                expected_override_pages.add(page)
        if series_pages != sorted(series_pages):
            issues.append(f"hierarchical target series {series_key} pages are not monotonic")

        trial = trial_by_key.get(series_key)
        if not isinstance(trial, dict):
            issues.append(f"hierarchical target series {series_key} has no method trial")
        else:
            expected_machine = expected_count - len(series_override_keys)
            if trial.get("expected_unit_count") != expected_count:
                issues.append(f"hierarchical target series {series_key} trial count drifted")
            if trial.get("ordered_bbox_candidate_match_count") != expected_machine:
                issues.append(
                    f"hierarchical target series {series_key} machine count drifted"
                )
            if trial.get("source_visible_override_unit_keys") != series_override_keys:
                issues.append(
                    f"hierarchical target series {series_key} override keys drifted"
                )
            expected_machine_count += expected_machine

    if len(series_keys) != len(set(series_keys)):
        issues.append("hierarchical target map repeats a series key")
    if series_sequences != list(range(1, len(series_rows) + 1)):
        issues.append("hierarchical target map series sequence is not contiguous")
    if all_pages != sorted(all_pages):
        issues.append("hierarchical target map pages are not globally monotonic")
    if len(all_anchor_refs) != len(set(all_anchor_refs)):
        issues.append("hierarchical target map repeats an anchor ref")
    if set(trial_by_key) != set(series_keys):
        issues.append("hierarchical target map method trials do not close the series")

    source_visible_review = method.get("source_visible_review", {})
    if not isinstance(source_visible_review, dict):
        issues.append("hierarchical target source-visible review is not an object")
    else:
        if source_visible_review.get("override_unit_refs") != expected_override_refs:
            issues.append("hierarchical target override refs drifted")
        if source_visible_review.get("reviewed_target_pdf_pages") != sorted(
            expected_override_pages
        ):
            issues.append("hierarchical target reviewed pages drifted")

    summary = payload.get("summary", {})
    if not isinstance(summary, dict):
        issues.append("hierarchical target map summary is not an object")
        return issues
    expected_summary = {
        "series_count": len(series_rows),
        "numbered_unit_count": len(all_pages),
        "ordered_bbox_candidate_match_count": expected_machine_count,
        "source_visible_override_unit_count": len(expected_override_refs),
        "source_visible_reviewed_page_count": len(expected_override_pages),
        "exact_start_page_candidates_materialized": len(all_pages),
    }
    for field, expected_value in expected_summary.items():
        if summary.get(field) != expected_value:
            issues.append(f"hierarchical target map summary {field} drifted")
    return issues


def _target_structural_crosswalk_issues(
    crosswalk: object,
    *,
    transfer_plan: object,
    target_map: object,
) -> list[str]:
    if not all(
        isinstance(value, dict)
        for value in (crosswalk, transfer_plan, target_map)
    ):
        return ["target structural crosswalk inputs are not objects"]
    issues: list[str] = []
    work_ref = crosswalk.get("work_ref")
    target_series = target_map.get("series", [])
    if not isinstance(target_series, list):
        return ["target structural crosswalk target series are not a list"]
    transfer_candidates = transfer_plan.get("candidate_target_units", [])
    if not isinstance(transfer_candidates, list):
        return ["target structural crosswalk transfer candidates are not a list"]
    expected_candidates = {
        candidate.get("unit_id"): candidate
        for candidate in transfer_candidates
        if (
            isinstance(candidate, dict)
            and isinstance(candidate.get("unit_id"), str)
            and candidate.get("work_ref") == work_ref
        )
    }
    starts: list[dict[str, object]] = []
    for series in target_series:
        if not isinstance(series, dict):
            issues.append("target structural crosswalk map contains a non-object series")
            continue
        series_key = series.get("series_key")
        unit_starts = series.get("unit_starts", [])
        if not isinstance(series_key, str) or not isinstance(unit_starts, list):
            issues.append("target structural crosswalk map series is malformed")
            continue
        for start in unit_starts:
            if not isinstance(start, dict):
                issues.append("target structural crosswalk map has a non-object start")
                continue
            unit_key = start.get("unit_key")
            pdf_page = start.get("pdf_page")
            if not isinstance(unit_key, str) or not isinstance(pdf_page, int):
                issues.append("target structural crosswalk map start is malformed")
                continue
            starts.append(
                {
                    "target_unit_ref": f"{series_key}:{unit_key}",
                    "pdf_page": pdf_page,
                }
            )
    actual_candidates = crosswalk.get("candidates", [])
    if not isinstance(actual_candidates, list):
        return ["target structural crosswalk candidates are not a list"]
    actual_ids: list[str] = []
    for candidate in actual_candidates:
        if not isinstance(candidate, dict):
            continue
        candidate_id = candidate.get("candidate_unit_id")
        if isinstance(candidate_id, str):
            actual_ids.append(candidate_id)
        else:
            issues.append("target structural crosswalk candidate has no string id")
    if len(actual_ids) != len(set(actual_ids)):
        issues.append("target structural crosswalk repeats a candidate unit")
    if set(actual_ids) != set(expected_candidates):
        issues.append("target structural crosswalk does not close the work quota")
    if crosswalk.get("target_expression_ref") != target_map.get("expression_ref"):
        issues.append("target structural crosswalk expression drifted")
    if crosswalk.get("target_item_ref") != target_map.get("item_ref"):
        issues.append("target structural crosswalk item drifted")

    possible_route_count = 0
    pages_with_starts = 0
    for candidate in actual_candidates:
        if not isinstance(candidate, dict):
            issues.append("target structural crosswalk contains a non-object row")
            continue
        unit_id = candidate.get("candidate_unit_id")
        expected = expected_candidates.get(unit_id)
        if expected is None:
            continue
        page = expected.get("page")
        if candidate.get("candidate_anchor_ref") != expected.get("anchor_ref"):
            issues.append(f"{unit_id} target candidate anchor drifted")
        if candidate.get("target_pdf_page") != page:
            issues.append(f"{unit_id} target candidate page drifted")
        if candidate.get("stratum") != expected.get("stratum"):
            issues.append(f"{unit_id} target candidate stratum drifted")
        if not isinstance(page, int):
            issues.append(f"{unit_id} has no integer target page")
            continue
        prior = [start for start in starts if start["pdf_page"] < page]
        on_page = [start for start in starts if start["pdf_page"] == page]
        following = [start for start in starts if start["pdf_page"] > page]
        if not prior or not following:
            issues.append(f"{unit_id} lacks surrounding proposed starts")
            continue
        expected_refs = [prior[-1]["target_unit_ref"]] + [
            start["target_unit_ref"] for start in on_page
        ]
        expected_start_refs = [
            start["target_unit_ref"] for start in on_page
        ]
        if candidate.get("possible_target_unit_refs") != expected_refs:
            issues.append(f"{unit_id} possible target unit refs drifted")
        if candidate.get("starts_on_page_target_unit_refs") != expected_start_refs:
            issues.append(f"{unit_id} on-page target unit starts drifted")
        expected_relation = (
            "prior-target-unit-spill-plus-unit-starts"
            if on_page
            else "within-one-proposed-target-numbered-unit"
        )
        if candidate.get("page_relation") != expected_relation:
            issues.append(f"{unit_id} target page relation drifted")
        expected_next = {
            "target_unit_ref": following[0]["target_unit_ref"],
            "target_pdf_page": following[0]["pdf_page"],
        }
        if candidate.get("next_proposed_start") != expected_next:
            issues.append(f"{unit_id} following target start drifted")
        possible_route_count += len(expected_refs)
        pages_with_starts += bool(on_page)

    summary = crosswalk.get("summary", {})
    if not isinstance(summary, dict):
        issues.append("target structural crosswalk summary is not an object")
        return issues
    expected_summary = {
        "candidate_page_count": len(expected_candidates),
        "random_page_count": sum(
            candidate.get("stratum") == "random"
            for candidate in expected_candidates.values()
        ),
        "hard_page_count": sum(
            candidate.get("stratum") == "hard"
            for candidate in expected_candidates.values()
        ),
        "page_with_unit_start_count": int(pages_with_starts),
        "page_without_unit_start_count": (
            len(expected_candidates) - int(pages_with_starts)
        ),
        "possible_target_unit_route_count": possible_route_count,
    }
    for field, expected_value in expected_summary.items():
        if summary.get(field) != expected_value:
            issues.append(f"target structural crosswalk summary {field} drifted")
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

    anchor_v2_lab_issues, _ = validate_source_anchor_v2_lab(repo_root)
    issues.extend(anchor_v2_lab_issues)

    source_text_layer_lab_issues, _ = validate_source_text_layer_lab(repo_root)
    issues.extend(source_text_layer_lab_issues)

    provenance_v2_lab_issues, _ = validate_provenance_v2_lab(repo_root)
    issues.extend(provenance_v2_lab_issues)

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
        expression_derivation_validator, _ = _schema_validator(
            EXPRESSION_DERIVATION_SCHEMA,
            repo_root,
        )
        provision_activity_validator, _ = _schema_validator(
            PROVISION_ACTIVITY_SCHEMA,
            repo_root,
        )
        first_publication_chronology_validator, _ = _schema_validator(
            FIRST_PUBLICATION_CHRONOLOGY_SCHEMA,
            repo_root,
        )
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
        public_evidence_derivative_validator, _ = _schema_validator(
            PUBLIC_EVIDENCE_DERIVATIVE_SCHEMA,
            repo_root,
        )
        manual_error_ledger_validator, _ = _schema_validator(
            MANUAL_ERROR_LEDGER_SCHEMA,
            repo_root,
        )
        transfer_candidate_crosswalk_validator, _ = _schema_validator(
            TRANSFER_CANDIDATE_CROSSWALK_SCHEMA,
            repo_root,
        )
        hierarchical_target_map_validator, _ = _schema_validator(
            HIERARCHICAL_TARGET_NUMBERED_UNIT_MAP_SCHEMA,
            repo_root,
        )
        target_structural_crosswalk_validator, _ = _schema_validator(
            TARGET_STRUCTURAL_CROSSWALK_SCHEMA,
            repo_root,
        )
        german_assisted_review_validator, _ = _schema_validator(
            GERMAN_ASSISTED_SOURCE_REVIEW_SCHEMA,
            repo_root,
        )
        _schema_validator(
            PRIVATE_TRANSFER_SOURCE_VISIBLE_REVIEW_BUNDLE_SCHEMA,
            repo_root,
        )
        transfer_source_visible_review_receipt_validator, _ = _schema_validator(
            TRANSFER_SOURCE_VISIBLE_REVIEW_RECEIPT_SCHEMA,
            repo_root,
        )
        critical_edition_witness_validator, _ = _schema_validator(
            CRITICAL_EDITION_WITNESS_ADMISSION_SCHEMA,
            repo_root,
        )
        critical_edition_citation_decision_validator, _ = _schema_validator(
            CRITICAL_EDITION_CITATION_WITNESS_DECISION_SCHEMA,
            repo_root,
        )
        edition_reading_admission_validator, _ = _schema_validator(
            EDITION_READING_ADMISSION_SCHEMA,
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
        experimental_translation_candidate_validator, _ = _schema_validator(
            EXPERIMENTAL_TRANSLATION_CANDIDATE_SCHEMA,
            repo_root,
        )
        experimental_translation_episode_validator, _ = _schema_validator(
            EXPERIMENTAL_TRANSLATION_EPISODE_SCHEMA,
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
        destination_ref = private_handoff.get("destination", {}).get("artifact_path")
        if isinstance(destination_ref, str):
            derivative_path = repo_root / destination_ref
            if derivative_path.exists():
                derivative = _load_json(derivative_path, repo_root, issues)
                if derivative is not None:
                    derivative_location = _relative(derivative_path, repo_root)
                    _validate_payload(
                        derivative,
                        public_evidence_derivative_validator,
                        derivative_location,
                        issues,
                    )
                    for message in _public_evidence_derivative_issues(
                        derivative,
                        handoff=private_handoff,
                    ):
                        issues.append((derivative_location, message))

                    ledger_path = repo_root / MANUAL_ERROR_LEDGER_PATH
                    ledger_location = _relative(ledger_path, repo_root)
                    ledger_records = _load_jsonl(ledger_path, repo_root, issues)
                    for number, record in enumerate(ledger_records, start=1):
                        _validate_payload(
                            record,
                            manual_error_ledger_validator,
                            f"{ledger_location}:{number}",
                            issues,
                        )
                    ledger_provenance_path = (
                        repo_root / MANUAL_ERROR_LEDGER_PROVENANCE_PATH
                    )
                    ledger_provenance_location = _relative(
                        ledger_provenance_path,
                        repo_root,
                    )
                    ledger_provenance_events = _load_jsonl(
                        ledger_provenance_path,
                        repo_root,
                        issues,
                    )
                    for number, event in enumerate(
                        ledger_provenance_events,
                        start=1,
                    ):
                        _validate_payload(
                            event,
                            provenance_validator,
                            f"{ledger_provenance_location}:{number}",
                            issues,
                        )
                    for message in _manual_error_ledger_issues(
                        ledger_records,
                        handoff=private_handoff,
                        derivative=derivative,
                        provenance_events=ledger_provenance_events,
                        repo_root=repo_root,
                    ):
                        issues.append((ledger_location, message))

    transfer_crosswalk_path = repo_root / TRANSFER_CANDIDATE_CROSSWALK_PATH
    transfer_crosswalk = _load_json(transfer_crosswalk_path, repo_root, issues)
    if transfer_crosswalk is not None:
        transfer_crosswalk_location = _relative(transfer_crosswalk_path, repo_root)
        _validate_payload(
            transfer_crosswalk,
            transfer_candidate_crosswalk_validator,
            transfer_crosswalk_location,
            issues,
        )
        loaded_crosswalk_inputs: dict[str, object] = {}
        for input_name, digest_bound_ref in transfer_crosswalk.get("inputs", {}).items():
            if not isinstance(digest_bound_ref, dict):
                continue
            input_ref = digest_bound_ref.get("ref")
            if not isinstance(input_ref, str):
                continue
            input_path = repo_root / input_ref
            digest_issue = _digest_bound_ref_issue(
                digest_bound_ref,
                expected_path=input_path,
                repo_root=repo_root,
                field=f"inputs.{input_name}",
            )
            if digest_issue is not None:
                issues.append((transfer_crosswalk_location, digest_issue))
            if input_name in {
                "transfer_plan",
                "target_numbered_unit_map",
                "shared_label_correspondence",
            }:
                loaded_crosswalk_inputs[input_name] = _load_json(
                    input_path,
                    repo_root,
                    issues,
                )
        required_crosswalk_inputs = (
            "transfer_plan",
            "target_numbered_unit_map",
            "shared_label_correspondence",
        )
        if all(
            loaded_crosswalk_inputs.get(name) is not None
            for name in required_crosswalk_inputs
        ):
            for message in _transfer_candidate_crosswalk_issues(
                transfer_crosswalk,
                transfer_plan=loaded_crosswalk_inputs["transfer_plan"],
                target_map=loaded_crosswalk_inputs["target_numbered_unit_map"],
                label_map=loaded_crosswalk_inputs["shared_label_correspondence"],
            ):
                issues.append((transfer_crosswalk_location, message))

    for target_structure_root in HIERARCHICAL_TARGET_STRUCTURE_PATHS:
        target_map_path = (
            repo_root
            / target_structure_root
            / "hierarchical-numbered-unit-page-map.json"
        )
        target_map = _load_json(target_map_path, repo_root, issues)
        if target_map is None:
            continue
        target_map_location = _relative(target_map_path, repo_root)
        _validate_payload(
            target_map,
            hierarchical_target_map_validator,
            target_map_location,
            issues,
        )
        for binding_name in ("inventory", "work_boundary"):
            binding = target_map.get(binding_name)
            if not isinstance(binding, dict):
                continue
            binding_ref = binding.get("ref")
            if not isinstance(binding_ref, str):
                continue
            digest_issue = _digest_bound_ref_issue(
                binding,
                expected_path=repo_root / binding_ref,
                repo_root=repo_root,
                field=binding_name,
            )
            if digest_issue is not None:
                issues.append((target_map_location, digest_issue))
        for message in _hierarchical_target_numbered_unit_map_issues(target_map):
            issues.append((target_map_location, message))

        target_crosswalk_path = (
            repo_root
            / target_structure_root
            / "transfer-candidate-page-crosswalk.v1.json"
        )
        target_crosswalk = _load_json(
            target_crosswalk_path,
            repo_root,
            issues,
        )
        if target_crosswalk is None:
            continue
        target_crosswalk_location = _relative(target_crosswalk_path, repo_root)
        _validate_payload(
            target_crosswalk,
            target_structural_crosswalk_validator,
            target_crosswalk_location,
            issues,
        )
        loaded_target_crosswalk_inputs: dict[str, object] = {}
        for input_name, digest_bound_ref in target_crosswalk.get(
            "inputs", {}
        ).items():
            if not isinstance(digest_bound_ref, dict):
                continue
            input_ref = digest_bound_ref.get("ref")
            if not isinstance(input_ref, str):
                continue
            input_path = repo_root / input_ref
            digest_issue = _digest_bound_ref_issue(
                digest_bound_ref,
                expected_path=input_path,
                repo_root=repo_root,
                field=f"inputs.{input_name}",
            )
            if digest_issue is not None:
                issues.append((target_crosswalk_location, digest_issue))
            if input_name == "transfer_plan":
                loaded_target_crosswalk_inputs[input_name] = _load_json(
                    input_path,
                    repo_root,
                    issues,
                )
        transfer_plan_input = loaded_target_crosswalk_inputs.get("transfer_plan")
        if transfer_plan_input is not None:
            for message in _target_structural_crosswalk_issues(
                target_crosswalk,
                transfer_plan=transfer_plan_input,
                target_map=target_map,
            ):
                issues.append((target_crosswalk_location, message))

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
    events_by_id: dict[str, dict[str, Any]] = {}
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
            layer_ids: set[str] = set()
            for layer_index, layer_assessment in enumerate(
                rights.get("layer_assessments", []),
                start=1,
            ):
                if not isinstance(layer_assessment, dict):
                    continue
                layer_location = f"{rights_location}#layer_assessments/{layer_index}"
                layer_id = layer_assessment.get("layer_id")
                if isinstance(layer_id, str):
                    if layer_id in layer_ids:
                        issues.append((layer_location, f"duplicate layer_id: {layer_id}"))
                    layer_ids.add(layer_id)
                _validate_source_refs(
                    repo_root,
                    layer_assessment,
                    layer_location,
                    issues,
                )

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
                events_by_id[event_id] = event
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
        transfer_target_anchor_path = (
            gold_root / "transfer-target-anchors.v1.jsonl"
        )
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
        transfer_source_visible_review_receipt_path = (
            gold_root
            / "transfer-source-visible-review.jenseits-187.v1.json"
        )
        transfer_route_readiness_path = (
            gold_root / "transfer-route-readiness.v1.json"
        )
        critical_edition_witness_paths = sorted(
            gold_root.glob("critical-edition-witness.*.json")
        )
        critical_edition_citation_decision_path = (
            gold_root
            / "critical-edition-citation-witness-decision."
            "ekgwb.za-i-vorrede-1.v1.json"
        )
        edition_reading_admission_path = (
            gold_root
            / "edition-reading-admission."
            "dta-ekgwb.za-i-vorrede-1.v1.json"
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
        experimental_translation_candidate_path = (
            gold_root
            / "experimental-translation-candidate."
            "admitted-ekgwb.za-i-vorrede-1-opening.variant-a.v1.json"
        )
        experimental_translation_episode_paths = sorted(
            gold_root.glob("experimental-translation-episode.*.json")
        )
        initial_sign_packet_path = gold_root / "initial-sign-packet.v5.json"
        initial_semantic_source_observation_plan_path = (
            gold_root / "initial-semantic-source-observation-plan.v1.json"
        )
        semantic_source_recurrence_plan_path = (
            gold_root / "semantic-source-recurrence-plan.v1.json"
        )
        semantic_source_recurrence_receipt_path = (
            gold_root / "semantic-source-recurrence-receipt.v1.json"
        )
        semantic_source_observation_anchor_path = (
            gold_root / "semantic-source-observation-anchors.v1.jsonl"
        )
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
        critical_edition_citation_decision_provenance_path = (
            gold_root
            / "provenance.critical-edition-citation-decision.jsonl"
        )
        edition_reading_admission_provenance_path = (
            gold_root / "provenance.edition-reading-admission.jsonl"
        )
        semantic_ladder_v4_provenance_path = (
            gold_root / "provenance.semantic-ladder-v4.jsonl"
        )
        semantic_source_observation_provenance_path = (
            gold_root / "provenance.semantic-source-observation-v1.jsonl"
        )
        semantic_source_recurrence_provenance_path = (
            gold_root / "provenance.semantic-source-recurrence-v1.jsonl"
        )
        experimental_translation_episode_provenance_paths = sorted(
            gold_root.glob("provenance.experimental-translation-episodes*.jsonl")
        )
        anchor_paths = [gold_root / "anchors.jsonl", gold_root / "translation-anchors.jsonl"]
        if ocr_anchor_path.is_file():
            anchor_paths.append(ocr_anchor_path)
        if transfer_target_anchor_path.is_file():
            anchor_paths.append(transfer_target_anchor_path)
        if semantic_source_observation_anchor_path.is_file():
            anchor_paths.append(semantic_source_observation_anchor_path)

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
        transfer_source_visible_review_receipt = (
            _load_json(
                transfer_source_visible_review_receipt_path,
                repo_root,
                issues,
            )
            if transfer_source_visible_review_receipt_path.is_file()
            else None
        )
        critical_edition_witnesses = [
            (path, payload)
            for path in critical_edition_witness_paths
            if (payload := _load_json(path, repo_root, issues)) is not None
        ]
        critical_edition_citation_decision = (
            _load_json(
                critical_edition_citation_decision_path,
                repo_root,
                issues,
            )
            if critical_edition_citation_decision_path.is_file()
            else None
        )
        edition_reading_admission = (
            _load_json(
                edition_reading_admission_path,
                repo_root,
                issues,
            )
            if edition_reading_admission_path.is_file()
            else None
        )
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
        experimental_translation_candidate = (
            _load_json(
                experimental_translation_candidate_path,
                repo_root,
                issues,
            )
            if experimental_translation_candidate_path.is_file()
            else None
        )
        experimental_translation_episodes = [
            (path, payload)
            for path in experimental_translation_episode_paths
            if (payload := _load_json(path, repo_root, issues)) is not None
        ]
        initial_sign_packet = _load_json(
            initial_sign_packet_path,
            repo_root,
            issues,
        )
        initial_semantic_source_observation_plan = _load_json(
            initial_semantic_source_observation_plan_path,
            repo_root,
            issues,
        )
        semantic_source_recurrence_plan = _load_json(
            semantic_source_recurrence_plan_path,
            repo_root,
            issues,
        )
        semantic_source_recurrence_receipt = _load_json(
            semantic_source_recurrence_receipt_path,
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
        if transfer_source_visible_review_receipt is not None:
            receipt_location = _relative(
                transfer_source_visible_review_receipt_path,
                repo_root,
            )
            _validate_payload(
                transfer_source_visible_review_receipt,
                transfer_source_visible_review_receipt_validator,
                receipt_location,
                issues,
            )
            generator = transfer_source_visible_review_receipt.get(
                "generator",
                {},
            )
            generator_ref = generator.get("ref")
            generator_path = repo_root / str(generator_ref or "")
            if (
                not isinstance(generator_ref, str)
                or not generator_path.is_file()
                or generator.get("sha256") != _sha256(generator_path)
            ):
                issues.append(
                    (
                        receipt_location,
                        "source-visible review generator reference or digest drifted",
                    )
                )
            evidence_inputs = transfer_source_visible_review_receipt.get(
                "evidence_inputs",
                {},
            )
            for side in ("source", "target"):
                witness = evidence_inputs.get(side, {})
                rights_ref = witness.get("rights_ref")
                rights_path = repo_root / str(rights_ref or "")
                if (
                    not isinstance(rights_ref, str)
                    or not rights_path.is_file()
                    or witness.get("rights_sha256") != _sha256(rights_path)
                ):
                    issues.append(
                        (
                            receipt_location,
                            f"source-visible review {side} rights reference or digest drifted",
                        )
                    )
            readiness = (
                _load_json(transfer_route_readiness_path, repo_root, issues)
                if transfer_route_readiness_path.is_file()
                else None
            )
            matching_routes = [
                route
                for route in (readiness or {}).get("routes", [])
                if isinstance(route, dict)
                and route.get("route_readiness_id")
                == transfer_source_visible_review_receipt.get(
                    "route_readiness_id"
                )
            ]
            if len(matching_routes) != 1:
                issues.append(
                    (
                        receipt_location,
                        "source-visible review route does not resolve exactly once in readiness projection",
                    )
                )
            else:
                route = matching_routes[0]
                for field in ("work_ref", "qualified_unit_key"):
                    if route.get(field) != transfer_source_visible_review_receipt.get(
                        field
                    ):
                        issues.append(
                            (
                                receipt_location,
                                f"source-visible review {field} drifted from readiness route",
                            )
                        )
                if any(
                    route.get(field) is not False
                    for field in (
                        "accepted_source_or_target_text",
                        "source_to_target_passage_alignment",
                        "eligible_for_variant_execution",
                        "target_gold",
                        "human_review_performed",
                    )
                ):
                    issues.append(
                        (
                            receipt_location,
                            "source-visible review readiness route has an open authority gate",
                        )
                    )
            encoded_receipt = json.dumps(
                transfer_source_visible_review_receipt,
                ensure_ascii=False,
                sort_keys=True,
            )
            if any(
                forbidden in encoded_receipt
                for forbidden in (
                    "/srv/",
                    "/home/",
                    '"automatic_candidate_text"',
                    '"diplomatic_lines"',
                    '"text"',
                    '"finding"',
                )
            ):
                issues.append(
                    (
                        receipt_location,
                        "tracked source-visible review leaks text or an absolute private path",
                    )
                )
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
            if initial_sign_packet.get("packet_status") != "observational-analysis":
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet must remain observational without semantic promotion",
                    )
                )
            observation_event_ref = (
                "tos.event.annotation."
                "zarathustra-semantic-source-observation-v1.2026-08-10"
            )
            observation_anchor_refs = [
                "tos.anchor.zarathustra-semantic-source-observation-v1.o001",
                "tos.anchor.zarathustra-semantic-source-observation-v1.o002",
                "tos.anchor.zarathustra-semantic-source-observation-v1.o003",
                "tos.anchor.zarathustra-semantic-source-observation-v1.o004",
            ]
            observation_occurrence_refs = [
                "tos.occurrence.lexical-zarathustra-dta-v1."
                "edad72babd2957782d70834e42cf5cd8baa861ace4ee183a4a0d8c779c087c01",
                "tos.occurrence.lexical-zarathustra-dta-v1."
                "6e1e0006ee44752bf6c33bc00af3cc48ffdcd30771f1ca0da49edb67ffb772b4",
                "tos.occurrence.lexical-zarathustra-dta-v1."
                "3395c4e11df51cfe5ccd7633e2f05077dcdf9f03fc1d94bdb84df79fbb3ff794",
                "tos.occurrence.lexical-zarathustra-dta-v1."
                "06fe0ae445facb6351810717570d11c83131d2f22a710b571bd09c03cb738cc6",
            ]
            source_gate = initial_sign_packet.get(
                "task_specific_source_gate",
                {},
            )
            edition_admission = (
                edition_reading_admission
                if isinstance(edition_reading_admission, dict)
                else {}
            )
            expected_source_anchor_refs = [
                edition_admission.get("target", {}).get("context_anchor_ref")
            ] + observation_anchor_refs
            if (
                source_gate.get("gate_status") != "satisfied"
                or source_gate.get("source_reading_status") != "edition-attested"
                or source_gate.get("source_observation_allowed") is not True
                or source_gate.get("edition_reading_admission_ref")
                != _relative(edition_reading_admission_path, repo_root)
                or source_gate.get("edition_reading_admission_sha256")
                != _sha256(edition_reading_admission_path)
                or source_gate.get("source_review_event_ref")
                != edition_admission.get("provenance_event_ref")
                or source_gate.get("source_anchor_refs")
                != expected_source_anchor_refs
                or source_gate.get("local_source_sha256")
                != edition_admission.get("source_identity", {}).get("file_sha256")
                or source_gate.get("local_source_tracked") is not False
                or source_gate.get("language_competence_status") != "blocked"
                or source_gate.get("language_competence_evidence_refs") != []
                or source_gate.get("linguistic_claim_review_allowed") is not False
            ):
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet edition-reading and competence separation drifted",
                    )
                )
            if (
                initial_sign_packet.get("candidate_ref") is not None
                or initial_sign_packet.get("accepted_sign_ref") is not None
                or initial_sign_packet.get("translation_evidence") != []
            ):
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet manufactured semantic or translation content",
                    )
                )
            source_forms = initial_sign_packet.get("source_forms", {})
            expected_local_root = (
                "ToS/source-witnesses/works/friedrich-nietzsche/"
                "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
                "local-content/semantic-source-observation/initial-v1/"
            )
            if source_forms != {
                "diplomatic_local_ref": (
                    expected_local_root + "diplomatic-form.txt"
                ),
                "diplomatic_sha256": (
                    "7524ff4cef9ab57270250af698a31e2c50f5d6de92e3aaa"
                    "1377f7ca3659c1ec1"
                ),
                "normalized_local_ref": (
                    expected_local_root + "normalized-form.txt"
                ),
                "normalized_sha256": (
                    "7524ff4cef9ab57270250af698a31e2c50f5d6de92e3aaa"
                    "1377f7ca3659c1ec1"
                ),
                "source_values_tracked": False,
            }:
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet source-withholding form binding drifted",
                    )
                )
            stages = initial_sign_packet.get("stages", [])
            active_stages = stages[:3]
            if (
                len(stages) != 15
                or any(
                    not isinstance(stage, dict)
                    or stage.get("status") != "source-observed"
                    or stage.get("source_anchor_refs") != observation_anchor_refs
                    or stage.get("source_return_verified") is not True
                    or stage.get("provenance_event_ref") != observation_event_ref
                    or stage.get("review_status") != "unreviewed"
                    or stage.get("blocker_refs") != []
                    or stage.get("maker", {}).get("performed_by_real_human")
                    is not False
                    for stage in active_stages
                )
                or any(
                    not isinstance(stage, dict)
                    or stage.get("status") != "blocked"
                    or stage.get("source_anchor_refs") != []
                    or stage.get("body") != {}
                    or stage.get("maker") is not None
                    or stage.get("provenance_event_ref") is not None
                    for stage in stages[3:]
                )
            ):
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet observational and blocked stage boundary drifted",
                    )
                )
            elif (
                active_stages[0].get("body", {}).get("occurrence_refs")
                != observation_occurrence_refs
                or active_stages[1].get("body", {}).get("occurrence_refs")
                != observation_occurrence_refs
                or active_stages[2].get("body", {}).get("occurrence_refs")
                != observation_occurrence_refs
                or active_stages[0].get("body", {}).get("exact_form_sha256")
                != (
                    "0007489cd4b0a84b926a341d3540ae1e8a2ff9cfc2062069"
                    "dbf5a4e994f6ef37"
                )
                or active_stages[1].get("body", {}).get("count") != 4
                or active_stages[1].get("body", {}).get(
                    "frequency_only_basis"
                )
                is not False
                or active_stages[2].get("body", {}).get("context_sha256")
                != (
                    "fe1cd840554ab13e2fed5fea9f46b6e5f5e730cd1d100b4a"
                    "57b010894d16c7a1"
                )
            ):
                issues.append(
                    (
                        initial_sign_location,
                        "initial exact-form, frequency, or context observation drifted",
                    )
                )
            plan = initial_semantic_source_observation_plan
            if (
                not isinstance(plan, dict)
                or plan.get("status") != "frozen-before-selection"
                or plan.get("source_scope", {}).get("section_resource_id")
                != "tei-div-0003"
                or plan.get("selection_policy", {}).get("ordering")
                != [
                    "section-occurrence-count-descending",
                    "first-source-token-ordinal-ascending",
                    "exact-form-utf8-bytes-ascending",
                ]
                or plan.get("selection_policy", {}).get(
                    "semantic_filter_used"
                )
                is not False
                or plan.get("selection_policy", {}).get(
                    "selection_is_sign_nomination"
                )
                is not False
            ):
                issues.append(
                    (
                        _relative(
                            initial_semantic_source_observation_plan_path,
                            repo_root,
                        ),
                        "initial semantic source-observation selection plan drifted",
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
        if (
            semantic_source_recurrence_plan is not None
            and semantic_source_recurrence_receipt is not None
        ):
            recurrence_location = _relative(
                semantic_source_recurrence_receipt_path,
                repo_root,
            )
            recurrence_plan_location = _relative(
                semantic_source_recurrence_plan_path,
                repo_root,
            )
            expected_hash = (
                "0007489cd4b0a84b926a341d3540ae1e8a2ff9cfc2062069"
                "dbf5a4e994f6ef37"
            )
            expected_tuple = {
                "occurrence_count": 145,
                "part_range": 4,
                "section_range": 59,
                "page_range": 96,
                "part_dp_millionths": 111588,
                "maximum_part_share_millionths": 351724,
                "source_editorial_occurrence_count": 0,
                "unsectioned_occurrence_count": 0,
            }
            expected_parts = [
                (1, 17, 14, 12),
                (2, 32, 20, 12),
                (3, 45, 26, 13),
                (4, 51, 36, 22),
            ]
            plan_selected = semantic_source_recurrence_plan.get(
                "selected_source_observation",
                {},
            )
            plan_recurrence = semantic_source_recurrence_plan.get(
                "recurrence_input",
                {},
            )
            expected_local_ref = (
                "ToS/source-witnesses/works/friedrich-nietzsche/"
                "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
                "local-content/semantic-source-observation/initial-v1/"
                "work-recurrence-bundle.json"
            )
            if (
                semantic_source_recurrence_plan.get("status")
                != "frozen-before-output"
                or plan_selected.get("exact_form_sha256") != expected_hash
                or plan_selected.get("packet_ref")
                != _relative(initial_sign_packet_path, repo_root)
                or plan_selected.get("packet_sha256")
                != _sha256(initial_sign_packet_path)
                or plan_selected.get("selection_reopened") is not False
                or semantic_source_recurrence_plan.get(
                    "expected_tracked_recurrence_tuple"
                )
                != expected_tuple
                or semantic_source_recurrence_plan.get("local_output", {}).get(
                    "ref"
                )
                != expected_local_ref
                or semantic_source_recurrence_plan.get(
                    "verification_policy",
                    {},
                ).get("raw_source_return_sampling")
                != "none-verify-all-occurrences"
                or semantic_source_recurrence_plan.get(
                    "verification_policy",
                    {},
                ).get("tracked_occurrence_positions")
                is not False
            ):
                issues.append(
                    (
                        recurrence_plan_location,
                        "selected-form source-recurrence plan drifted",
                    )
                )
            for ref_field, digest_field, label in (
                (
                    "recurrence_plan_ref",
                    "recurrence_plan_sha256",
                    "recurrence plan",
                ),
                (
                    "recurrence_projection_ref",
                    "recurrence_projection_sha256",
                    "recurrence projection",
                ),
                (
                    "lexical_projection_ref",
                    "lexical_projection_sha256",
                    "lexical projection",
                ),
            ):
                ref = plan_recurrence.get(ref_field)
                path = repo_root / str(ref or "")
                if (
                    not isinstance(ref, str)
                    or not path.is_file()
                    or plan_recurrence.get(digest_field) != _sha256(path)
                ):
                    issues.append(
                        (
                            recurrence_plan_location,
                            f"selected-form {label} binding drifted",
                        )
                    )
            if _git_ignored(repo_root, repo_root / expected_local_ref) is not True:
                issues.append(
                    (
                        recurrence_plan_location,
                        "private source-recurrence bundle is not Git-ignored",
                    )
                )

            receipt = semantic_source_recurrence_receipt
            generator = receipt.get("generator", {})
            generator_path = repo_root / str(generator.get("ref") or "")
            receipt_selected = receipt.get("selected_source_observation", {})
            receipt_recurrence = receipt.get("recurrence_sources", {})
            if (
                receipt.get("status")
                != "completed-source-observation-no-promotion"
                or receipt.get("plan")
                != {
                    "ref": recurrence_plan_location,
                    "sha256": _sha256(semantic_source_recurrence_plan_path),
                }
                or not generator_path.is_file()
                or generator.get("sha256") != _sha256(generator_path)
                or receipt_selected.get("exact_form_sha256") != expected_hash
                or receipt_selected.get("packet_ref")
                != _relative(initial_sign_packet_path, repo_root)
                or receipt_selected.get("packet_sha256")
                != _sha256(initial_sign_packet_path)
                or receipt_selected.get("selection_reopened") is not False
                or receipt_selected.get("source_value_tracked") is not False
                or receipt_recurrence.get("recurrence_projection_ref")
                != plan_recurrence.get("recurrence_projection_ref")
                or receipt_recurrence.get("recurrence_projection_sha256")
                != plan_recurrence.get("recurrence_projection_sha256")
                or receipt.get("observed_tuple") != expected_tuple
                or receipt.get("local_bundle", {}).get("ref")
                != expected_local_ref
                or receipt.get("local_bundle", {}).get("mode") != "0600"
                or receipt.get("local_bundle", {}).get("occurrence_count") != 145
            ):
                issues.append(
                    (
                        recurrence_location,
                        "selected-form source-recurrence receipt drifted",
                    )
                )
            observed_parts = [
                (
                    row.get("part_order"),
                    row.get("occurrence_count"),
                    row.get("page_count"),
                    row.get("section_count"),
                )
                for row in receipt.get("parts", [])
                if isinstance(row, dict)
            ]
            verification = receipt.get("verification", {})
            packet_effect = receipt.get("packet_effect", {})
            boundary = receipt.get("authority_boundary", {})
            if (
                observed_parts != expected_parts
                or verification.get("complete_occurrence_census") is not True
                or verification.get("raw_offset_return_count") != 145
                or verification.get("raw_offset_return_match_count") != 145
                or verification.get("source_payload_fixity_match_count") != 4
                or verification.get("tracked_recurrence_tuple_match") is not True
                or verification.get(
                    "independent_part_size_aware_recalculation"
                )
                is not True
                or packet_effect.get("packet_changed") is not False
                or packet_effect.get("ladder_stage_changed") is not False
                or packet_effect.get("human_work_scheduled") is not False
                or packet_effect.get("promotion_authorized") is not False
                or boundary.get("source_observation_only") is not True
                or boundary.get("packet_stage_change_authorized") is not False
                or any(
                    boundary.get(field) is not False
                    for field in (
                        "accepted_german",
                        "morphology",
                        "lemma",
                        "sense",
                        "motif",
                        "philosophical_importance",
                        "translation",
                        "sign_candidate",
                        "human_task",
                        "semantic_claim",
                        "graph_effect",
                        "canon_effect",
                        "transfer",
                        "publication",
                    )
                )
            ):
                issues.append(
                    (
                        recurrence_location,
                        "selected-form recurrence verification or authority boundary drifted",
                    )
                )
            encoded_receipt = json.dumps(
                receipt,
                ensure_ascii=False,
                sort_keys=True,
            )
            if any(
                forbidden in encoded_receipt
                for forbidden in (
                    "/srv/",
                    "/home/",
                    '"exact_form"',
                    '"normalized_form"',
                    '"occurrence_id"',
                    '"text_node_path"',
                    '"token_ordinal"',
                    '"start_offset"',
                    '"end_offset"',
                )
            ):
                issues.append(
                    (
                        recurrence_location,
                        "tracked source-recurrence receipt leaks source values, positions, or an absolute private path",
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
                    or current_state.get("promotion_authorized") is not False
                ):
                    issues.append(
                        (
                            triangulation_location,
                            "machine triangulation crossed a German-acceptance or promotion gate",
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
            additive_decision_admitted = (
                isinstance(critical_edition_citation_decision, dict)
                and critical_edition_citation_decision.get("status")
                == "human_admitted_with_limits"
            )
            if (
                critical_edition_witness.get("status")
                != "citation_witness_admitted"
                and not additive_decision_admitted
                and isinstance(german_assisted_review, dict)
            ):
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
        if critical_edition_citation_decision is not None:
            decision_location = _relative(
                critical_edition_citation_decision_path,
                repo_root,
            )
            _validate_payload(
                critical_edition_citation_decision,
                critical_edition_citation_decision_validator,
                decision_location,
                issues,
            )
            historical_witness_path = (
                critical_edition_witness_paths[0]
                if len(critical_edition_witness_paths) == 1
                else None
            )
            institutional_corroboration_path = (
                repo_root
                / SOURCE_ROOT
                / "discovery/runs/"
                "ekgwb-za-i-vorrede-1-institutional-corroboration."
                "2026-07-30.v3.json"
            )
            rights_path = gold_root / "rights.ekgwb.za-i-vorrede-1.v1.json"
            decision_research_path = (
                repo_root
                / "ToS/research-packets/foundation-laboratory-2026-07/"
                "EKGWB_CITATION_WITNESS_DECISION_REFRESH_2026-08-08.md"
            )
            expected_evidence = (
                ("historical_metadata_packet", historical_witness_path),
                ("machine_triangulation_packet", german_source_triangulation_path),
                ("institutional_corroboration_record", institutional_corroboration_path),
                ("rights_record", rights_path),
                ("ordered_research_refresh", decision_research_path),
            )
            for field, expected_path in expected_evidence:
                if expected_path is None or not expected_path.is_file():
                    issues.append(
                        (
                            decision_location,
                            f"{field} has no current owner artifact",
                        )
                    )
                    continue
                digest_issue = _digest_bound_ref_issue(
                    critical_edition_citation_decision.get("evidence", {}).get(field),
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=f"evidence.{field}",
                )
                if digest_issue is not None:
                    issues.append((decision_location, digest_issue))

            decision_target = critical_edition_citation_decision.get("target", {})
            triangulation_target = (
                german_source_triangulation.get("target", {})
                if isinstance(german_source_triangulation, dict)
                else {}
            )
            for field in (
                "review_unit_id",
                "context_anchor_ref",
                "critical_locator_siglum",
            ):
                if decision_target.get(field) != triangulation_target.get(field):
                    issues.append(
                        (
                            decision_location,
                            f"citation-witness decision target {field} drifted from triangulation",
                        )
                    )

            transport_fixity = critical_edition_citation_decision.get(
                "transport_and_fixity",
                {},
            )
            triangulation_results = (
                german_source_triangulation.get("results", {})
                if isinstance(german_source_triangulation, dict)
                else {}
            )
            normalized_digest = triangulation_results.get(
                "ekgwb_reference",
                {},
            ).get("normalized_sequence_sha256")
            if transport_fixity.get("normalized_sequence_sha256") != normalized_digest:
                issues.append(
                    (
                        decision_location,
                        "citation-witness normalized sequence digest drifted from triangulation",
                    )
                )

            institutional_record = (
                _load_json(
                    institutional_corroboration_path,
                    repo_root,
                    issues,
                )
                if institutional_corroboration_path.is_file()
                else None
            )
            target_block_digests: set[str] = set()
            if isinstance(institutional_record, dict):
                selected_ids = set(institutional_record.get("selected_result_ids", []))
                for channel in institutional_record.get("channels", []):
                    if not isinstance(channel, dict):
                        continue
                    for result in channel.get("results", []):
                        if (
                            not isinstance(result, dict)
                            or result.get("result_id") not in selected_ids
                        ):
                            continue
                        for identifier in result.get("identifiers", []):
                            if (
                                isinstance(identifier, dict)
                                and identifier.get("scheme")
                                == "exact target block SHA-256"
                                and isinstance(identifier.get("value"), str)
                            ):
                                target_block_digests.add(identifier["value"])
            if target_block_digests != {
                transport_fixity.get("exact_target_block_sha256")
            }:
                issues.append(
                    (
                        decision_location,
                        "citation-witness target-block digest lacks exact selected-channel closure",
                    )
                )

            decision_admitted = (
                critical_edition_citation_decision.get("status")
                == "human_admitted_with_limits"
            )
            if isinstance(german_assisted_review, dict):
                assisted_state = german_assisted_review.get("current_state", {})
                expected_admitted_units = 1 if decision_admitted else 0
                expected_lanes = ["ai_only", "ai_human"] if decision_admitted else []
                if assisted_state.get("admitted_critical_edition_units") != (
                    expected_admitted_units
                ):
                    issues.append(
                        (
                            decision_location,
                            "citation-witness decision and assisted-review admitted count disagree",
                        )
                    )
                if assisted_state.get("translation_lanes_opened") != expected_lanes:
                    issues.append(
                        (
                            decision_location,
                            "citation-witness decision and assisted-review translation lanes disagree",
                        )
                    )
        if edition_reading_admission is not None:
            admission_location = _relative(
                edition_reading_admission_path,
                repo_root,
            )
            _validate_payload(
                edition_reading_admission,
                edition_reading_admission_validator,
                admission_location,
                issues,
            )
            dta_item_root = (
                repo_root
                / SOURCE_ROOT
                / "works/friedrich-nietzsche/also-sprach-zarathustra/"
                "expressions/de-schmeitzner-1883-part-1/"
                "editions/chemnitz-schmeitzner-1883-part-1/"
                "items/dta-sbb-corrected-tei-p5"
            )
            admission_research_path = (
                repo_root
                / "ToS/research-packets/foundation-laboratory-2026-07/"
                "GERMAN_EDITION_READING_ADMISSION_RESEARCH_2026-08-10.md"
            )
            expected_evidence = (
                ("item_manifest", dta_item_root / "item.manifest.json"),
                (
                    "source_metadata_snapshot",
                    dta_item_root / "source-metadata-snapshot.json",
                ),
                ("rights_record", dta_item_root / "rights.json"),
                ("triangulation_packet", german_source_triangulation_path),
                (
                    "critical_witness_decision",
                    critical_edition_citation_decision_path,
                ),
                ("ordered_research", admission_research_path),
            )
            for field, expected_path in expected_evidence:
                if not expected_path.is_file():
                    issues.append(
                        (
                            admission_location,
                            f"evidence.{field} has no current owner artifact",
                        )
                    )
                    continue
                digest_issue = _digest_bound_ref_issue(
                    edition_reading_admission.get("evidence", {}).get(field),
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=f"evidence.{field}",
                )
                if digest_issue is not None:
                    issues.append((admission_location, digest_issue))

            item_manifest = _load_json(
                dta_item_root / "item.manifest.json",
                repo_root,
                issues,
            )
            source_identity = edition_reading_admission.get("source_identity", {})
            if isinstance(item_manifest, dict):
                if source_identity.get("item_ref") != item_manifest.get("item_id"):
                    issues.append(
                        (
                            admission_location,
                            "edition-reading Item identity drifted from the DTA manifest",
                        )
                    )
                if source_identity.get("edition_ref") != item_manifest.get(
                    "embodiment_ref"
                ):
                    issues.append(
                        (
                            admission_location,
                            "edition-reading Edition identity drifted from the DTA manifest",
                        )
                    )
                payload_files = item_manifest.get("payload_files", [])
                payload = payload_files[0] if len(payload_files) == 1 else {}
                for field, manifest_field in (
                    ("file_ref", "file_id"),
                    ("file_sha256", "sha256"),
                ):
                    if source_identity.get(field) != payload.get(manifest_field):
                        issues.append(
                            (
                                admission_location,
                                f"edition-reading {field} drifted from the DTA manifest",
                            )
                        )

            triangulation_target = (
                german_source_triangulation.get("target", {})
                if isinstance(german_source_triangulation, dict)
                else {}
            )
            admission_target = edition_reading_admission.get("target", {})
            for field in (
                "review_unit_id",
                "context_anchor_ref",
                "critical_locator_siglum",
            ):
                if admission_target.get(field) != triangulation_target.get(field):
                    issues.append(
                        (
                            admission_location,
                            f"edition-reading target {field} drifted from triangulation",
                        )
                    )
            dta_result = (
                german_source_triangulation.get("results", {}).get(
                    "dta_exact_comparison",
                    {},
                )
                if isinstance(german_source_triangulation, dict)
                else {}
            )
            reading_evidence = edition_reading_admission.get(
                "reading_evidence",
                {},
            )
            expected_reading_fields = {
                "source_selector": dta_result.get("section_selector"),
                "paragraph_count": dta_result.get("paragraphs"),
                "normalized_token_count": dta_result.get("normalized_tokens"),
                "normalized_sequence_sha256": dta_result.get(
                    "normalized_sequence_sha256"
                ),
                "exact_after_source_aware_normalization": dta_result.get(
                    "exact_after_source_aware_normalization"
                ),
            }
            for field, expected in expected_reading_fields.items():
                if reading_evidence.get(field) != expected:
                    issues.append(
                        (
                            admission_location,
                            f"edition-reading {field} drifted from exact DTA comparison",
                        )
                    )
            if admission_target.get("source_selector") != reading_evidence.get(
                "source_selector"
            ):
                issues.append(
                    (
                        admission_location,
                        "edition-reading target selector and reading selector disagree",
                    )
                )
            decision_digest = (
                critical_edition_citation_decision.get(
                    "transport_and_fixity",
                    {},
                ).get("normalized_sequence_sha256")
                if isinstance(critical_edition_citation_decision, dict)
                else None
            )
            if reading_evidence.get("normalized_sequence_sha256") != decision_digest:
                issues.append(
                    (
                        admission_location,
                        "edition-reading sequence is not closed by the admitted critical witness",
                    )
                )

            rights_record = _load_json(
                dta_item_root / "rights.json",
                repo_root,
                issues,
            )
            rights_and_visibility = edition_reading_admission.get(
                "rights_and_visibility",
                {},
            )
            if isinstance(rights_record, dict):
                if rights_and_visibility.get("rights_ref") != rights_record.get(
                    "rights_id"
                ):
                    issues.append(
                        (
                            admission_location,
                            "edition-reading rights identity drifted from the DTA record",
                        )
                    )
                if rights_and_visibility.get(
                    "rights_assessment_status"
                ) != rights_record.get("assessment_status"):
                    issues.append(
                        (
                            admission_location,
                            "edition-reading rights posture drifted from the DTA record",
                        )
                    )
        if experimental_translation_candidate is not None:
            candidate_location = _relative(
                experimental_translation_candidate_path,
                repo_root,
            )
            _validate_payload(
                experimental_translation_candidate,
                experimental_translation_candidate_validator,
                candidate_location,
                issues,
            )
            admission = experimental_translation_candidate.get("admission", {})
            expected_bindings = [
                (
                    "citation_witness_decision",
                    critical_edition_citation_decision_path,
                ),
                (
                    "current_source_return_overlay",
                    bounded_translation_research_input_path,
                ),
            ]
            for field, expected_path in expected_bindings:
                binding = admission.get(field)
                if not isinstance(binding, dict):
                    issues.append(
                        (candidate_location, f"admission.{field} is absent")
                    )
                    continue
                digest_issue = _digest_bound_ref_issue(
                    binding,
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=f"admission.{field}",
                )
                if digest_issue is not None:
                    issues.append((candidate_location, digest_issue))
            if (
                not isinstance(critical_edition_citation_decision, dict)
                or critical_edition_citation_decision.get("status")
                != "human_admitted_with_limits"
                or "ai_only"
                not in critical_edition_citation_decision.get(
                    "gate_effects", {}
                ).get("translation_lanes_opened", [])
            ):
                issues.append(
                    (
                        candidate_location,
                        "experimental translation candidate lacks an admitted AI-only citation-witness route",
                    )
                )
            elif experimental_translation_candidate.get("target") != {
                "review_unit_id": critical_edition_citation_decision.get(
                    "target", {}
                ).get("review_unit_id"),
                "context_anchor_ref": critical_edition_citation_decision.get(
                    "target", {}
                ).get("context_anchor_ref"),
                "critical_locator_siglum": critical_edition_citation_decision.get(
                    "target", {}
                ).get("critical_locator_siglum"),
            }:
                issues.append(
                    (
                        candidate_location,
                        "experimental translation candidate target drifted from the admitted citation witness",
                    )
                )
        for episode_path, episode in experimental_translation_episodes:
            episode_location = _relative(episode_path, repo_root)
            _validate_payload(
                episode,
                experimental_translation_episode_validator,
                episode_location,
                issues,
            )
            admission = episode.get("admission", {})
            episode_model_id = (
                episode.get("method_freeze", {})
                .get("model", {})
                .get("model_id")
            )
            if episode_model_id == "OpenVINO/Qwen3-8B-int4-ov":
                research_refresh_name = (
                    "LOCAL_LLM_TRANSLATION_QWEN3_8B_CPU_REFRESH_2026-08-08.md"
                )
            elif episode_model_id == "google/madlad400-3b-mt":
                research_refresh_name = "LOCAL_LLM_ADMISSION.md"
            else:
                research_refresh_name = (
                    "LOCAL_LLM_TRANSLATION_CANDIDATE_REFRESH_2026-08-08.md"
                )
            expected_bindings = [
                (
                    "citation_witness_decision",
                    critical_edition_citation_decision_path,
                ),
                (
                    "current_source_return_overlay",
                    bounded_translation_research_input_path,
                ),
                (
                    "ordered_research_refresh",
                    repo_root
                    / "ToS/research-packets/foundation-laboratory-2026-07/"
                    / research_refresh_name,
                ),
            ]
            if episode_model_id == "google/madlad400-3b-mt":
                expected_bindings.append(
                    (
                        "specialized_mt_challenger_admission",
                        repo_root
                        / "ToS/research-packets/foundation-laboratory-2026-07/"
                        / "SPECIALIZED_MT_CHALLENGER_ADMISSION_2026-08-10.md",
                    )
                )
            for field, expected_path in expected_bindings:
                binding = admission.get(field)
                if not isinstance(binding, dict):
                    issues.append(
                        (episode_location, f"admission.{field} is absent")
                    )
                    continue
                digest_issue = _digest_bound_ref_issue(
                    binding,
                    expected_path=expected_path,
                    repo_root=repo_root,
                    field=f"admission.{field}",
                )
                if digest_issue is not None:
                    issues.append((episode_location, digest_issue))
            if (
                not isinstance(critical_edition_citation_decision, dict)
                or critical_edition_citation_decision.get("status")
                != "human_admitted_with_limits"
                or "ai_only"
                not in critical_edition_citation_decision.get(
                    "gate_effects", {}
                ).get("translation_lanes_opened", [])
            ):
                issues.append(
                    (
                        episode_location,
                        "experimental translation episode lacks an admitted AI-only citation-witness route",
                    )
                )
            elif episode.get("target") != {
                "review_unit_id": critical_edition_citation_decision.get(
                    "target", {}
                ).get("review_unit_id"),
                "context_anchor_ref": critical_edition_citation_decision.get(
                    "target", {}
                ).get("context_anchor_ref"),
                "critical_locator_siglum": critical_edition_citation_decision.get(
                    "target", {}
                ).get("critical_locator_siglum"),
            }:
                issues.append(
                    (
                        episode_location,
                        "experimental translation episode target drifted from the admitted citation witness",
                    )
                )
            if isinstance(bounded_translation_research_input, dict):
                local_artifact = bounded_translation_research_input.get(
                    "local_artifact", {}
                )
                private_run = episode.get("private_run", {})
                if private_run.get("local_source_artifact_sha256") != (
                    local_artifact.get("artifact_sha256")
                ):
                    issues.append(
                        (
                            episode_location,
                            "experimental translation episode local source artifact digest drifted",
                        )
                    )
                if private_run.get("source_text_sha256") != local_artifact.get(
                    "source_text_sha256"
                ):
                    issues.append(
                        (
                            episode_location,
                            "experimental translation episode source-text digest drifted",
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
        if critical_edition_citation_decision_provenance_path.is_file():
            local_provenance_paths.append(
                critical_edition_citation_decision_provenance_path
            )
        if edition_reading_admission_provenance_path.is_file():
            local_provenance_paths.append(
                edition_reading_admission_provenance_path
            )
        if semantic_ladder_v4_provenance_path.is_file():
            local_provenance_paths.append(
                semantic_ladder_v4_provenance_path
            )
        if semantic_source_observation_provenance_path.is_file():
            local_provenance_paths.append(
                semantic_source_observation_provenance_path
            )
        if semantic_source_recurrence_provenance_path.is_file():
            local_provenance_paths.append(
                semantic_source_recurrence_provenance_path
            )
        local_provenance_paths.extend(
            experimental_translation_episode_provenance_paths
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
                    events_by_id[event_id] = event
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
        if critical_edition_citation_decision is not None:
            decision_event_ref = critical_edition_citation_decision.get(
                "provenance_event_ref"
            )
            if decision_event_ref not in local_event_ids:
                issues.append(
                    (
                        _relative(
                            critical_edition_citation_decision_path,
                            repo_root,
                        ),
                        "citation-witness decision provenance event is absent from gold-set provenance",
                    )
                )
        if edition_reading_admission is not None:
            admission_event_ref = edition_reading_admission.get(
                "provenance_event_ref"
            )
            if admission_event_ref not in local_event_ids:
                issues.append(
                    (
                        _relative(
                            edition_reading_admission_path,
                            repo_root,
                        ),
                        "edition-reading admission provenance event is absent from gold-set provenance",
                    )
                )
        if experimental_translation_candidate is not None:
            candidate_event_ref = experimental_translation_candidate.get(
                "provenance_event_ref"
            )
            if candidate_event_ref not in local_event_ids:
                issues.append(
                    (
                        _relative(
                            experimental_translation_candidate_path,
                            repo_root,
                        ),
                        "experimental translation candidate provenance event is absent from gold-set provenance",
                    )
                )
        for episode_path, episode in experimental_translation_episodes:
            episode_location = _relative(episode_path, repo_root)
            episode_event_ref = episode.get("provenance_event_ref")
            episode_event = local_events_by_id.get(episode_event_ref)
            if episode_event is None:
                issues.append(
                    (
                        episode_location,
                        "experimental translation episode provenance event is absent from gold-set provenance",
                    )
                )
                continue
            episode_outputs = {
                (output.get("ref"), output.get("sha256"))
                for output in episode_event.get("outputs", [])
                if isinstance(output, dict)
            }
            if (episode_location, _sha256(episode_path)) not in episode_outputs:
                issues.append(
                    (
                        episode_location,
                        "experimental translation episode provenance output drifted",
                    )
                )
        if german_source_triangulation is not None:
            triangulation_event_id = (
                "tos.event.alignment.zarathustra-german-source-triangulation."
                "za-i-vorrede-1.2026-07-29"
            )
            triangulation_location = _relative(
                german_source_triangulation_path,
                repo_root,
            )
            triangulation_event = _latest_event_in_supersession_lineage(
                local_events_by_id,
                triangulation_event_id,
                location=triangulation_location,
                issues=issues,
            )
            if triangulation_event is None:
                issues.append(
                    (
                        triangulation_location,
                        "German source triangulation provenance event is absent",
                    )
                )
            else:
                actual_outputs = {
                    (
                        output.get("ref"),
                        output.get("sha256"),
                    )
                    for output in triangulation_event.get("outputs", [])
                    if isinstance(output, dict)
                }
                if (
                    triangulation_location,
                    _sha256(german_source_triangulation_path),
                ) not in actual_outputs:
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
            bounded_location = _relative(
                bounded_translation_research_input_path,
                repo_root,
            )
            bounded_event = _latest_event_in_supersession_lineage(
                local_events_by_id,
                bounded_event_id,
                location=bounded_location,
                issues=issues,
            )
            if bounded_event is None:
                issues.append(
                    (
                        bounded_location,
                        "bounded translation research-input provenance event is absent",
                    )
                )
            else:
                actual_outputs = {
                    (
                        output.get("ref"),
                        output.get("sha256"),
                    )
                    for output in bounded_event.get("outputs", [])
                    if isinstance(output, dict)
                }
                if (
                    bounded_location,
                    _sha256(bounded_translation_research_input_path),
                ) not in actual_outputs:
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
            expected_initial_sign_event_refs = [
                "tos.event.annotation."
                "zarathustra-initial-sign-packet-v4.2026-08-10",
                "tos.event.annotation."
                "zarathustra-semantic-source-observation-v1.2026-08-10",
            ]
            if initial_sign_event_refs != expected_initial_sign_event_refs:
                issues.append(
                    (
                        initial_sign_location,
                        "initial sign packet provenance event identity drifted",
                    )
                )
            else:
                initial_sign_event = local_events_by_id.get(
                    initial_sign_event_refs[1]
                )
                expected_output = {
                    "ref": initial_sign_location,
                    "role": "source-observed-no-promotion-semantic-ladder-packet",
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
                expected_anchor_output = {
                    "ref": _relative(
                        semantic_source_observation_anchor_path,
                        repo_root,
                    ),
                    "role": "text-free-exact-occurrence-source-return-anchors",
                    "sha256": _sha256(
                        semantic_source_observation_anchor_path
                    ),
                }
                if (
                    initial_sign_event is not None
                    and expected_anchor_output
                    not in initial_sign_event.get("outputs", [])
                ):
                    issues.append(
                        (
                            initial_sign_location,
                            "initial source-observation anchor provenance drifted",
                        )
                    )
        if semantic_source_recurrence_receipt is not None:
            recurrence_location = _relative(
                semantic_source_recurrence_receipt_path,
                repo_root,
            )
            recurrence_event_id = (
                "tos.event.annotation."
                "zarathustra-semantic-source-recurrence-v1.2026-08-10"
            )
            recurrence_event = local_events_by_id.get(recurrence_event_id)
            expected_receipt_output = {
                "ref": recurrence_location,
                "role": "tracked-source-withholding-recurrence-receipt",
                "sha256": _sha256(semantic_source_recurrence_receipt_path),
            }
            local_bundle = semantic_source_recurrence_receipt.get(
                "local_bundle",
                {},
            )
            expected_private_output = {
                "ref": local_bundle.get("ref"),
                "role": "ignored-private-complete-raw-witness-recurrence-bundle",
                "sha256": local_bundle.get("sha256"),
            }
            if recurrence_event is None:
                issues.append(
                    (
                        recurrence_location,
                        "selected-form recurrence provenance event is absent",
                    )
                )
            elif (
                expected_receipt_output not in recurrence_event.get("outputs", [])
                or expected_private_output
                not in recurrence_event.get("outputs", [])
                or semantic_source_recurrence_receipt.get(
                    "provenance_event_ref"
                )
                != recurrence_event_id
            ):
                issues.append(
                    (
                        recurrence_location,
                        "selected-form recurrence provenance output drifted",
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

            candidate_units = [
                unit
                for unit in transfer_plan.get("candidate_target_units", [])
                if isinstance(unit, dict)
            ]
            candidate_unit_ids = [
                unit.get("unit_id") for unit in candidate_units
            ]
            candidate_pages = [
                (unit.get("file_ref"), unit.get("page"))
                for unit in candidate_units
            ]
            candidate_content_refs = [
                unit.get("source_content_ref") for unit in candidate_units
            ]
            if len(candidate_unit_ids) != len(set(candidate_unit_ids)):
                issues.append(
                    (
                        transfer_location,
                        "transfer candidate-unit identities are not unique",
                    )
                )
            if len(candidate_pages) != len(set(candidate_pages)):
                issues.append(
                    (
                        transfer_location,
                        "transfer candidate pages are not unique",
                    )
                )
            if len(candidate_content_refs) != len(set(candidate_content_refs)):
                issues.append(
                    (
                        transfer_location,
                        "transfer candidate local-content refs are not unique",
                    )
                )
            if set(candidate_unit_ids) & set(target_unit_ids):
                issues.append(
                    (
                        transfer_location,
                        "prepared candidate and eligible target identities overlap",
                    )
                )

            candidate_preparation = transfer_plan.get(
                "candidate_preparation",
                {},
            )
            if candidate_units:
                builder_binding = candidate_preparation.get("builder", {})
                builder_ref = builder_binding.get("ref")
                builder_path = repo_root / str(builder_ref)
                if (
                    builder_ref
                    != "scripts/build_golden_kernel_transfer_candidates.py"
                    or not builder_path.is_file()
                ):
                    issues.append(
                        (
                            transfer_location,
                            "transfer candidate builder ref is unresolved",
                        )
                    )
                elif builder_binding.get("sha256") != _sha256(builder_path):
                    issues.append(
                        (
                            transfer_location,
                            "transfer candidate builder digest drifted",
                        )
                    )

                actual_strata = {
                    stratum: sum(
                        unit.get("stratum") == stratum
                        for unit in candidate_units
                    )
                    for stratum in ("random", "hard")
                }
                if actual_strata != {"random": 10, "hard": 10}:
                    issues.append(
                        (
                            transfer_location,
                            "transfer candidate strata are not exactly 10 random and 10 hard",
                        )
                    )
                expected_work_quotas = {
                    quota.get("work_ref"): {
                        "random": quota.get("random"),
                        "hard": quota.get("hard"),
                    }
                    for quota in candidate_preparation.get(
                        "work_quotas",
                        [],
                    )
                    if isinstance(quota, dict)
                }
                actual_work_quotas = {
                    work_ref: {
                        stratum: sum(
                            unit.get("work_ref") == work_ref
                            and unit.get("stratum") == stratum
                            for unit in candidate_units
                        )
                        for stratum in ("random", "hard")
                    }
                    for work_ref in expected_work_quotas
                }
                if actual_work_quotas != expected_work_quotas:
                    issues.append(
                        (
                            transfer_location,
                            "transfer candidate work quotas drifted",
                        )
                    )

            for candidate in candidate_units:
                unit_id = candidate.get("unit_id")
                anchor_ref = candidate.get("anchor_ref")
                anchor = anchors_by_id.get(str(anchor_ref))
                page_selectors = [
                    selector
                    for selector in (anchor or {}).get("selectors", [])
                    if isinstance(selector, dict)
                    and selector.get("type") == "page_region"
                ]
                if (
                    len(page_selectors) != 1
                    or page_selectors[0].get("page")
                    != candidate.get("page")
                ):
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} does not resolve to its exact whole-page anchor",
                        )
                    )
                elif (
                    page_selectors[0].get("x"),
                    page_selectors[0].get("y"),
                    page_selectors[0].get("width"),
                    page_selectors[0].get("height"),
                    page_selectors[0].get("coordinate_space"),
                ) != (0, 0, 1, 1, "normalized_0_1"):
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} anchor is not the exact whole page",
                        )
                    )
                if (anchor or {}).get("item_id") != candidate.get("item_ref"):
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} item differs from its anchor",
                        )
                    )
                if (anchor or {}).get("file_id") != candidate.get("file_ref"):
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} file differs from its anchor",
                        )
                    )

                work_record = records_by_id.get(
                    str(candidate.get("work_ref")),
                    ({}, Path()),
                )[0]
                expression_record = records_by_id.get(
                    str(candidate.get("expression_ref")),
                    ({}, Path()),
                )[0]
                if work_record.get("record_type") != "work":
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} work_ref is unresolved",
                        )
                    )
                if (
                    expression_record.get("record_type") != "expression"
                    or expression_record.get("work_ref")
                    != candidate.get("work_ref")
                ):
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} expression does not belong to its work",
                        )
                    )
                if (
                    file_owner_by_id.get(str(candidate.get("file_ref")))
                    != candidate.get("item_ref")
                ):
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} file does not belong to its item",
                        )
                    )
                if candidate.get("page_resource_id") != (
                    f"pdf-page-{int(candidate.get('page', 0)):04d}"
                ):
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} page resource identity drifted",
                        )
                    )

                content_ref = candidate.get("source_content_ref")
                content_path = repo_root / str(content_ref)
                try:
                    content_path.resolve().relative_to(
                        (gold_root / "local-content/transfer-targets/v1").resolve()
                    )
                except ValueError:
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} content escaped the private transfer lane",
                        )
                    )
                if _git_ignored(repo_root, content_path) is not True:
                    issues.append(
                        (
                            transfer_location,
                            f"{unit_id} local content is not protected by Git ignore",
                        )
                    )
                if content_path.is_file():
                    if content_path.stat().st_size != candidate.get(
                        "source_content_bytes"
                    ):
                        issues.append(
                            (
                                _relative(content_path, repo_root),
                                "transfer candidate byte count drifted",
                            )
                        )
                    if _sha256(content_path) != candidate.get(
                        "source_content_sha256"
                    ):
                        issues.append(
                            (
                                _relative(content_path, repo_root),
                                "transfer candidate digest drifted",
                            )
                        )
                elif require_local_payloads:
                    issues.append(
                        (
                            _relative(content_path, repo_root),
                            "required local transfer candidate is missing",
                        )
                    )

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
                if candidate_units:
                    expected_anchor_output = {
                        "ref": _relative(
                            transfer_target_anchor_path,
                            repo_root,
                        ),
                        "role": "proposed-whole-page-transfer-candidate-anchors",
                        "sha256": _sha256(transfer_target_anchor_path),
                    }
                    if expected_anchor_output not in transfer_event.get(
                        "outputs",
                        [],
                    ):
                        issues.append(
                            (
                                transfer_location,
                                "transfer provenance candidate-anchor output drifted",
                            )
                        )
                    expected_local_outputs = {
                        (
                            candidate.get("source_content_ref"),
                            "gitignored-local-only-pdftotext-page-candidate",
                            candidate.get("source_content_sha256"),
                        )
                        for candidate in candidate_units
                    }
                    actual_local_outputs = {
                        (
                            output.get("ref"),
                            output.get("role"),
                            output.get("sha256"),
                        )
                        for output in transfer_event.get("outputs", [])
                        if isinstance(output, dict)
                    }
                    if not expected_local_outputs.issubset(
                        actual_local_outputs
                    ):
                        issues.append(
                            (
                                transfer_location,
                                "transfer provenance private candidate outputs drifted",
                            )
                        )

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
                "source_sample_plan_sha256": _sha256(sample_path),
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

        referenced_anchor_ids = (
            sample_anchor_ids
            | ocr_plan_anchor_ids
            | retrieval_anchor_ids
            | graph_anchor_ids
            | {
                anchor_ref
                for anchor_ref in (
                    (initial_sign_packet or {})
                    .get("task_specific_source_gate", {})
                    .get("source_anchor_refs", [])
                )
                if isinstance(anchor_ref, str)
            }
            | {
                anchor_ref
                for stage in (initial_sign_packet or {}).get("stages", [])
                if isinstance(stage, dict)
                for anchor_ref in stage.get("source_anchor_refs", [])
                if isinstance(anchor_ref, str)
            }
            | {
                fragment.get("source_anchor_ref")
                for fragment in (translation_plan or {}).get("fragments", [])
                if isinstance(fragment, dict)
            }
            | {
                unit.get("anchor_ref")
                for unit in (transfer_plan or {}).get(
                    "candidate_target_units",
                    [],
                )
                if isinstance(unit, dict)
            }
        )
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
                events_by_id[event_id] = event
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
        coverage_posture = boundary_map.get("coverage_posture")
        explicit_coverage = isinstance(coverage_posture, str)
        source_sequences = [
            member.get("source_sequence")
            for member in members
            if isinstance(member, dict) and "source_sequence" in member
        ]
        if coverage_posture == "partial_membership_representation":
            if len(source_sequences) != len(members):
                issues.append(
                    (
                        location,
                        "partial work-boundary members must carry source_sequence",
                    )
                )
            elif source_sequences != sorted(set(source_sequences)):
                issues.append(
                    (
                        location,
                        "partial work-boundary source_sequence values must be "
                        "strictly increasing and unique",
                    )
                )
        previous_end: int | None = None
        represented_ranges: list[tuple[int, int, str]] = []
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
                if (
                    not explicit_coverage
                    and previous_end is not None
                    and start_page != previous_end + 1
                ):
                    issues.append((location, f"work-boundary members are not contiguous at sequence {member.get('sequence')}"))
                previous_end = end_page
                represented_ranges.append(
                    (start_page, end_page, f"member sequence {member.get('sequence')}")
                )
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
            responsibility_ref = member.get("responsibility_claim_ref")
            if responsibility_ref is None:
                responsibility_ref = member.get("translation_responsibility_claim_ref")
            if isinstance(responsibility_ref, str):
                boundary_map_responsibility_refs.add(responsibility_ref)

        non_member_sections = boundary_map.get("non_member_sections", [])
        if explicit_coverage:
            previous_end = None
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
                represented_ranges.append(
                    (start_page, end_page, f"non-member section {section.get('label')}")
                )
            anchor_ref = section.get("boundary_anchor_ref")
            if anchor_ref not in local_anchor_ids:
                issues.append((location, f"unresolved non-member boundary anchor: {anchor_ref}"))
            elif anchor_page_by_id.get(anchor_ref) != start_page:
                issues.append((location, f"non-member boundary anchor does not match start_page: {anchor_ref}"))

        unrepresented_sections = boundary_map.get("unrepresented_sections", [])
        if (
            coverage_posture == "complete_membership_representation"
            and unrepresented_sections
        ):
            issues.append(
                (
                    location,
                    "complete work-boundary representation cannot contain "
                    "unrepresented sections",
                )
            )
        if (
            coverage_posture == "partial_membership_representation"
            and not unrepresented_sections
        ):
            issues.append(
                (
                    location,
                    "partial work-boundary representation requires at least one "
                    "unrepresented section",
                )
            )
        for section in unrepresented_sections:
            if not isinstance(section, dict):
                continue
            start_page = section.get("start_page")
            end_page = section.get("end_page")
            if isinstance(start_page, int) and isinstance(end_page, int):
                if start_page > end_page:
                    issues.append((location, f"unrepresented section start exceeds end: {section.get('label')}"))
                represented_ranges.append(
                    (start_page, end_page, f"unrepresented section {section.get('label')}")
                )
            anchor_ref = section.get("boundary_anchor_ref")
            if anchor_ref not in local_anchor_ids:
                issues.append((location, f"unresolved unrepresented boundary anchor: {anchor_ref}"))
            elif anchor_page_by_id.get(anchor_ref) != start_page:
                issues.append((location, f"unrepresented boundary anchor does not match start_page: {anchor_ref}"))

        if explicit_coverage:
            ordered_ranges = sorted(represented_ranges)
            coverage_end = 0
            for start_page, end_page, label in ordered_ranges:
                if start_page != coverage_end + 1:
                    issues.append(
                        (
                            location,
                            f"explicit work-boundary coverage has a gap or overlap before {label}",
                        )
                    )
                coverage_end = max(coverage_end, end_page)
            if coverage_end != boundary_map.get("page_count"):
                issues.append(
                    (
                        location,
                        "explicit work-boundary coverage does not cover the exact page_count",
                    )
                )
        elif previous_end != boundary_map.get("page_count"):
            issues.append((location, "work and non-member boundaries do not cover the exact page_count"))
        for anchor_ref in boundary_map.get("crosscheck_anchor_refs", []):
            if anchor_ref not in local_anchor_ids:
                issues.append((location, f"unresolved boundary crosscheck anchor: {anchor_ref}"))

    evidence_anchor_ids = set(boundary_anchor_ids)
    source_anchor_event_locations: list[tuple[str, str]] = []
    for anchor_path in sorted((repo_root / SOURCE_ROOT).rglob("anchors.jsonl")):
        if anchor_path.with_name("work-boundary-map.json").is_file():
            continue
        for index, anchor in enumerate(
            _load_jsonl(anchor_path, repo_root, issues),
            start=1,
        ):
            anchor_location = f"{_relative(anchor_path, repo_root)}:{index}"
            _validate_payload(anchor, anchor_validator, anchor_location, issues)
            anchor_id = anchor.get("anchor_id")
            if isinstance(anchor_id, str):
                if anchor_id in evidence_anchor_ids:
                    issues.append(
                        (anchor_location, f"duplicate source evidence anchor_id: {anchor_id}")
                    )
                evidence_anchor_ids.add(anchor_id)
            require_record(anchor.get("item_id"), "item", anchor_location)
            event_ref = anchor.get("provenance_event_ref")
            if isinstance(event_ref, str):
                source_anchor_event_locations.append((anchor_location, event_ref))

    topology_provenance_events = _load_jsonl(
        repo_root / BIBLIOGRAPHIC_TOPOLOGY_PROVENANCE,
        repo_root,
        issues,
    )
    if len(topology_provenance_events) != 1:
        issues.append(
            (
                BIBLIOGRAPHIC_TOPOLOGY_PROVENANCE.as_posix(),
                "bibliographic topology must have exactly one batch provenance event",
            )
        )
    topology_event: dict[str, Any] | None = None
    for index, event in enumerate(topology_provenance_events, start=1):
        location = f"{BIBLIOGRAPHIC_TOPOLOGY_PROVENANCE.as_posix()}:{index}"
        _validate_payload(event, provenance_validator, location, issues)
        _validate_source_refs(repo_root, event, location, issues)
        event_id = event.get("event_id")
        if event_id != BIBLIOGRAPHIC_TOPOLOGY_EVENT_REF:
            issues.append(
                (
                    location,
                    "bibliographic topology provenance event_id differs from the owned route",
                )
            )
        if event.get("event_type") != "annotation":
            issues.append((location, "bibliographic topology provenance must be annotation"))
        if event.get("agent_refs") != ["model:codex"]:
            issues.append(
                (location, "bibliographic topology provenance agent must be model:codex")
            )
        method = event.get("method", {})
        if (
            method.get("maker_type") != "model"
            or method.get("name")
            != "declared-bibliographic-topology-materialization"
            or method.get("version") != "1"
        ):
            issues.append(
                (
                    location,
                    "bibliographic topology provenance method identity drifted",
                )
            )
        if event.get("status") != "completed_with_warnings":
            issues.append(
                (
                    location,
                    "bibliographic topology provenance must retain completed-with-warnings posture",
                )
            )
        if isinstance(event_id, str):
            if event_id in event_ids:
                issues.append((location, f"duplicate event_id: {event_id}"))
            else:
                event_ids.add(event_id)
                events_by_id[event_id] = event
        if event_id == BIBLIOGRAPHIC_TOPOLOGY_EVENT_REF:
            topology_event = event

    expected_topology_objects: dict[str, dict[str, set[str]]] = {
        "has_expression": {
            record_id: set()
            for record_id, (payload, _) in records_by_id.items()
            if payload.get("record_type") == "work"
        },
        "embodied_by": {
            record_id: set()
            for record_id, (payload, _) in records_by_id.items()
            if payload.get("record_type") == "expression"
        },
        "exemplified_by": {
            record_id: set()
            for record_id, (payload, _) in records_by_id.items()
            if payload.get("record_type") == "edition"
        },
    }
    for expression_id, (expression, _) in records_by_id.items():
        if expression.get("record_type") != "expression":
            continue
        work_ref = expression.get("work_ref")
        if isinstance(work_ref, str):
            expected_topology_objects["has_expression"].setdefault(
                work_ref,
                set(),
            ).add(expression_id)
    for edition_id, (edition, _) in records_by_id.items():
        if edition.get("record_type") != "edition":
            continue
        for expression_ref in edition.get("embodies_expression_refs", []):
            if isinstance(expression_ref, str):
                expected_topology_objects["embodied_by"].setdefault(
                    expression_ref,
                    set(),
                ).add(edition_id)
    for item_id, edition_ref in item_edition_by_id.items():
        expected_topology_objects["exemplified_by"].setdefault(
            edition_ref,
            set(),
        ).add(item_id)

    topology_claims_by_predicate_subject: dict[
        str,
        dict[str, dict[str, str]],
    ] = {
        predicate: {}
        for _, predicate, _, _, _, _ in BIBLIOGRAPHIC_TOPOLOGY_ROUTES
    }
    seen_topology_pairs: set[tuple[str, str, str]] = set()

    for (
        claim_relative,
        predicate,
        subject_type,
        object_type,
        _claim_ref_field,
        output_role,
    ) in BIBLIOGRAPHIC_TOPOLOGY_ROUTES:
        claim_path = repo_root / claim_relative
        actual_paths = {
            path.relative_to(repo_root)
            for path in (repo_root / SOURCE_ROOT).rglob(claim_relative.name)
        }
        if actual_paths != {claim_relative}:
            issues.append(
                (
                    claim_relative.as_posix(),
                    "bibliographic topology claim basename must exist only at its owned relation route",
                )
            )
        claim_file_ref = claim_relative.as_posix()
        claim_file_digest = _sha256(claim_path) if claim_path.is_file() else None
        if topology_event is not None and claim_file_digest is not None:
            expected_output = {
                "ref": claim_file_ref,
                "role": output_role,
                "sha256": claim_file_digest,
            }
            if expected_output not in topology_event.get("outputs", []):
                issues.append(
                    (
                        claim_file_ref,
                        "bibliographic topology provenance event does not digest-bind the claim file",
                    )
                )

        for index, claim in enumerate(
            _load_jsonl(claim_path, repo_root, issues),
            start=1,
        ):
            location = f"{claim_file_ref}:{index}"
            _validate_payload(claim, claim_validator, location, issues)
            claim_id = claim.get("claim_id")
            subject_ref = claim.get("subject_ref")
            object_ref = claim.get("object")
            if isinstance(claim_id, str):
                if claim_id in claim_ids:
                    issues.append((location, f"duplicate claim_id: {claim_id}"))
                claim_ids.add(claim_id)
            if claim.get("claim_type") != "bibliographic":
                issues.append(
                    (location, "bibliographic topology claim_type must be bibliographic")
                )
            if claim.get("assertion_layer") != "bibliographic_assertion":
                issues.append(
                    (
                        location,
                        "bibliographic topology assertion_layer must be bibliographic_assertion",
                    )
                )
            if claim.get("predicate") != predicate:
                issues.append(
                    (location, f"{claim_relative.name} predicate must be {predicate}")
                )
            require_record(subject_ref, subject_type, location)
            require_record(object_ref, object_type, location)
            if claim.get("maker") != {
                "maker_type": "model",
                "agent_ref": "model:codex",
            }:
                issues.append((location, "bibliographic topology maker must be model:codex"))
            if claim.get("provenance_event_ref") != BIBLIOGRAPHIC_TOPOLOGY_EVENT_REF:
                issues.append(
                    (location, "bibliographic topology claim cites the wrong provenance event")
                )
            if claim.get("epistemic_status") != "observed":
                issues.append(
                    (location, "declared topology materialization must remain observed")
                )
            if claim.get("review_status") != "unreviewed" or claim.get("reviews") != []:
                issues.append(
                    (location, "bibliographic topology claims must remain unreviewed")
                )
            if claim.get("visibility") != "public_metadata_only":
                issues.append(
                    (
                        location,
                        "bibliographic topology claims must remain public metadata only",
                    )
                )

            if isinstance(subject_ref, str) and isinstance(object_ref, str):
                pair = (predicate, subject_ref, object_ref)
                if pair in seen_topology_pairs:
                    issues.append((location, f"duplicate bibliographic topology pair: {pair}"))
                seen_topology_pairs.add(pair)
                if isinstance(claim_id, str):
                    topology_claims_by_predicate_subject[predicate].setdefault(
                        subject_ref,
                        {},
                    )[claim_id] = object_ref
                if object_ref not in expected_topology_objects[predicate].get(
                    subject_ref,
                    set(),
                ):
                    issues.append(
                        (
                            location,
                            "claim is not backed by the declared corpus-record topology",
                        )
                    )

            expected_evidence: set[str] = set()
            for ref in (subject_ref, object_ref):
                record = records_by_id.get(str(ref))
                if record is not None:
                    expected_evidence.add(_relative(record[1], repo_root))
            if object_type == "item":
                item_record = records_by_id.get(str(object_ref))
                if item_record is not None:
                    manifest_ref = item_record[0].get("item_manifest_ref")
                    if isinstance(manifest_ref, str):
                        expected_evidence.add(manifest_ref)
            actual_evidence = set(claim.get("evidence_refs", []))
            if actual_evidence != expected_evidence:
                issues.append(
                    (
                        location,
                        "bibliographic topology evidence must be the exact linked records and item manifest",
                    )
                )
            if topology_event is not None:
                event_inputs = {
                    (entry.get("ref"), entry.get("sha256"))
                    for entry in topology_event.get("inputs", [])
                    if isinstance(entry, dict)
                }
                for evidence_ref in expected_evidence:
                    evidence_path = repo_root / evidence_ref
                    if evidence_path.is_file() and (
                        evidence_ref,
                        _sha256(evidence_path),
                    ) not in event_inputs:
                        issues.append(
                            (
                                location,
                                "bibliographic topology provenance does not digest-bind "
                                f"evidence input: {evidence_ref}",
                            )
                        )

    for (
        _claim_relative,
        predicate,
        subject_type,
        _object_type,
        claim_ref_field,
        _output_role,
    ) in BIBLIOGRAPHIC_TOPOLOGY_ROUTES:
        for subject_ref, (subject, subject_path) in records_by_id.items():
            if subject.get("record_type") != subject_type:
                continue
            location = _relative(subject_path, repo_root)
            claims_for_subject = topology_claims_by_predicate_subject[predicate].get(
                subject_ref,
                {},
            )
            actual_claim_refs = set(subject.get(claim_ref_field, []))
            expected_claim_refs = set(claims_for_subject)
            if actual_claim_refs != expected_claim_refs:
                issues.append(
                    (
                        location,
                        f"{claim_ref_field} does not close over exact {predicate} claims",
                    )
                )
            actual_objects = set(claims_for_subject.values())
            expected_objects = expected_topology_objects[predicate].get(
                subject_ref,
                set(),
            )
            if actual_objects != expected_objects:
                issues.append(
                    (
                        location,
                        f"{predicate} claims do not close over declared corpus-record links",
                    )
                )

    if topology_event is not None:
        configuration = topology_event.get("method", {}).get("configuration", {})
        expected_counts = {
            "work_expression_claims_materialized": sum(
                len(values)
                for values in expected_topology_objects["has_expression"].values()
            ),
            "expression_edition_claims_materialized": sum(
                len(values)
                for values in expected_topology_objects["embodied_by"].values()
            ),
            "edition_item_claims_materialized": sum(
                len(values)
                for values in expected_topology_objects["exemplified_by"].values()
            ),
            "topology_claims_reviewed": 0,
            "source_text_admitted": False,
            "human_review_performed": False,
            "textual_equivalence_claims_created": 0,
            "semantic_claims_created": 0,
            "canon_promotion_performed": False,
        }
        if configuration != expected_counts:
            issues.append(
                (
                    BIBLIOGRAPHIC_TOPOLOGY_PROVENANCE.as_posix(),
                    "bibliographic topology provenance configuration differs from exact closure counts and authority limits",
                )
            )

    derivation_provenance_events = _load_jsonl(
        repo_root / EXPRESSION_DERIVATION_PROVENANCE,
        repo_root,
        issues,
    )
    if len(derivation_provenance_events) != 1:
        issues.append(
            (
                EXPRESSION_DERIVATION_PROVENANCE.as_posix(),
                "Expression derivation must have exactly one batch provenance event",
            )
        )
    derivation_event: dict[str, Any] | None = None
    for index, event in enumerate(derivation_provenance_events, start=1):
        location = f"{EXPRESSION_DERIVATION_PROVENANCE.as_posix()}:{index}"
        _validate_payload(event, provenance_validator, location, issues)
        _validate_source_refs(repo_root, event, location, issues)
        event_id = event.get("event_id")
        if event_id != EXPRESSION_DERIVATION_EVENT_REF:
            issues.append((location, "Expression-derivation provenance event_id drifted"))
        if event.get("event_type") != "annotation":
            issues.append((location, "Expression-derivation provenance must be annotation"))
        if event.get("agent_refs") != ["model:codex"]:
            issues.append((location, "Expression-derivation provenance agent must be model:codex"))
        method = event.get("method", {})
        if (
            method.get("maker_type") != "model"
            or method.get("name")
            != "source-reported-expression-derivation-materialization"
            or method.get("version") != "1"
        ):
            issues.append((location, "Expression-derivation provenance method identity drifted"))
        if event.get("status") != "completed_with_warnings":
            issues.append((location, "Expression derivation must retain warning posture"))
        if isinstance(event_id, str):
            if event_id in event_ids:
                issues.append((location, f"duplicate event_id: {event_id}"))
            else:
                event_ids.add(event_id)
                events_by_id[event_id] = event
        if event_id == EXPRESSION_DERIVATION_EVENT_REF:
            derivation_event = event

    actual_derivation_paths = {
        path.relative_to(repo_root)
        for path in (repo_root / SOURCE_ROOT).rglob(
            EXPRESSION_DERIVATION_CLAIMS.name
        )
    }
    if actual_derivation_paths != {EXPRESSION_DERIVATION_CLAIMS}:
        issues.append(
            (
                EXPRESSION_DERIVATION_CLAIMS.as_posix(),
                "Expression-derivation claim basename must exist only at its owned route",
            )
        )

    derivation_claim_ids: set[str] = set()
    derivation_claim_subjects: dict[str, str] = {}
    derivation_edges: dict[str, set[str]] = {}
    derivation_pairs: set[tuple[str, str]] = set()
    revision_claim_count = 0
    collated_claim_count = 0
    reviewed_claim_count = 0
    derivation_evidence_paths: set[str] = set()
    derivation_claim_path = repo_root / EXPRESSION_DERIVATION_CLAIMS
    for index, claim in enumerate(
        _load_jsonl(derivation_claim_path, repo_root, issues),
        start=1,
    ):
        location = f"{EXPRESSION_DERIVATION_CLAIMS.as_posix()}:{index}"
        _validate_payload(claim, claim_validator, location, issues)
        _validate_payload(
            claim.get("qualifiers"),
            expression_derivation_validator,
            f"{location}['qualifiers']",
            issues,
        )
        claim_id = claim.get("claim_id")
        subject_ref = claim.get("subject_ref")
        object_ref = claim.get("object")
        if isinstance(claim_id, str):
            if claim_id in claim_ids:
                issues.append((location, f"duplicate claim_id: {claim_id}"))
            claim_ids.add(claim_id)
            derivation_claim_ids.add(claim_id)
            if isinstance(subject_ref, str):
                derivation_claim_subjects[claim_id] = subject_ref
        if claim.get("claim_type") != "relation":
            issues.append((location, "Expression derivation claim_type must be relation"))
        if claim.get("assertion_layer") != "bibliographic_assertion":
            issues.append((location, "Expression derivation must remain bibliographic"))
        if claim.get("predicate") != "is_derivative_of":
            issues.append((location, "Expression derivation predicate drifted"))
        require_record(subject_ref, "expression", location)
        require_record(object_ref, "expression", location)
        if subject_ref == object_ref:
            issues.append((location, "Expression derivation is irreflexive"))
        subject_record = records_by_id.get(str(subject_ref))
        object_record = records_by_id.get(str(object_ref))
        if subject_record is not None and object_record is not None:
            if subject_record[0].get("work_ref") != object_record[0].get("work_ref"):
                issues.append((location, "v1 Expression derivation endpoints must realize the same Work"))
        if isinstance(subject_ref, str) and isinstance(object_ref, str):
            pair = (subject_ref, object_ref)
            if pair in derivation_pairs:
                issues.append((location, "duplicate Expression-derivation endpoint pair"))
            derivation_pairs.add(pair)
            derivation_edges.setdefault(subject_ref, set()).add(object_ref)
        if claim.get("maker") != {
            "maker_type": "model",
            "agent_ref": "model:codex",
        }:
            issues.append((location, "Expression derivation maker must be model:codex"))
        if claim.get("provenance_event_ref") != EXPRESSION_DERIVATION_EVENT_REF:
            issues.append((location, "Expression derivation cites the wrong provenance event"))
        if claim.get("epistemic_status") != "reported":
            issues.append((location, "current Expression derivation claims must remain reported"))
        if claim.get("review_status") != "unreviewed" or claim.get("reviews") != []:
            issues.append((location, "Expression derivation claims must remain unreviewed"))
        if claim.get("visibility") != "public_metadata_only":
            issues.append((location, "Expression derivation must remain public metadata only"))
        qualifiers = claim.get("qualifiers", {})
        if qualifiers.get("derivation_kind") == "revision":
            revision_claim_count += 1
        if qualifiers.get("collation_status") != "not_collated":
            collated_claim_count += 1
        if claim.get("review_status") != "unreviewed":
            reviewed_claim_count += 1
        evidence_refs = claim.get("evidence_refs", [])
        if not any(
            isinstance(ref, str) and ref.startswith("tos.anchor.")
            for ref in evidence_refs
        ):
            issues.append((location, "Expression derivation lacks exact source-anchor return"))
        for evidence_ref in evidence_refs:
            if isinstance(evidence_ref, str) and evidence_ref.startswith("tos.anchor."):
                if evidence_ref not in evidence_anchor_ids:
                    issues.append((location, f"unresolved derivation anchor: {evidence_ref}"))
            elif isinstance(evidence_ref, str) and evidence_ref.startswith("ToS/"):
                evidence_path = repo_root / evidence_ref
                if not evidence_path.is_file():
                    issues.append((location, f"unresolved derivation evidence: {evidence_ref}"))
                derivation_evidence_paths.add(evidence_ref)

    visit_state: dict[str, int] = {}
    cycle_reported = False

    def visit_derivation_node(expression_ref: str) -> None:
        nonlocal cycle_reported
        visit_state[expression_ref] = 1
        for source_ref in sorted(derivation_edges.get(expression_ref, set())):
            if visit_state.get(source_ref) == 1:
                if not cycle_reported:
                    issues.append(
                        (
                            EXPRESSION_DERIVATION_CLAIMS.as_posix(),
                            "Expression derivation cycle detected",
                        )
                    )
                    cycle_reported = True
            elif visit_state.get(source_ref, 0) == 0:
                visit_derivation_node(source_ref)
        visit_state[expression_ref] = 2

    for start_ref in sorted(derivation_edges):
        if visit_state.get(start_ref, 0) == 0:
            visit_derivation_node(start_ref)

    for expression_ref, (expression, expression_path) in records_by_id.items():
        if expression.get("record_type") != "expression":
            continue
        location = _relative(expression_path, repo_root)
        expected_refs = {
            claim_id
            for claim_id, subject_ref in derivation_claim_subjects.items()
            if subject_ref == expression_ref
        }
        actual_refs = set(expression.get("derivation_claim_refs", []))
        if actual_refs != expected_refs:
            issues.append(
                (
                    location,
                    "derivation_claim_refs do not close over exact outgoing derivation claims",
                )
            )

    if derivation_event is not None:
        claim_digest = _sha256(derivation_claim_path)
        expected_output = {
            "ref": EXPRESSION_DERIVATION_CLAIMS.as_posix(),
            "role": "unreviewed-source-reported-expression-derivation-claims",
            "sha256": claim_digest,
        }
        if derivation_event.get("outputs") != [expected_output]:
            issues.append(
                (
                    EXPRESSION_DERIVATION_PROVENANCE.as_posix(),
                    "Expression-derivation provenance output digest drifted",
                )
            )
        endpoint_refs = set(derivation_edges)
        for source_refs in derivation_edges.values():
            endpoint_refs.update(source_refs)
        endpoint_paths = {
            _relative(records_by_id[ref][1], repo_root)
            for ref in endpoint_refs
            if ref in records_by_id
        }
        expected_input_refs = derivation_evidence_paths | endpoint_paths | {
            CLAIM_SCHEMA.as_posix(),
            EXPRESSION_DERIVATION_SCHEMA.as_posix(),
        }
        actual_inputs = {
            entry.get("ref"): entry.get("sha256")
            for entry in derivation_event.get("inputs", [])
            if isinstance(entry, dict)
        }
        if set(actual_inputs) != expected_input_refs:
            issues.append(
                (
                    EXPRESSION_DERIVATION_PROVENANCE.as_posix(),
                    "Expression-derivation provenance inputs differ from exact evidence and endpoints",
                )
            )
        for input_ref, input_digest in actual_inputs.items():
            input_path = repo_root / str(input_ref)
            if not input_path.is_file() or _sha256(input_path) != input_digest:
                issues.append(
                    (
                        EXPRESSION_DERIVATION_PROVENANCE.as_posix(),
                        f"Expression-derivation provenance input digest drifted: {input_ref}",
                    )
                )
        expected_configuration = {
            "expression_identities_materialized": len(endpoint_refs),
            "derivation_claims_materialized": len(derivation_claim_ids),
            "revision_claims_materialized": revision_claim_count,
            "claims_collated": collated_claim_count,
            "claims_reviewed": reviewed_claim_count,
            "unsupported_1911_to_1907_edge_created": False,
            "unsupported_2007_to_1911_edge_created": False,
            "source_text_admitted": False,
            "human_review_performed": False,
            "equivalence_claims_created": 0,
            "semantic_claims_created": 0,
            "canon_promotion_performed": False,
        }
        if derivation_event.get("method", {}).get("configuration") != expected_configuration:
            issues.append(
                (
                    EXPRESSION_DERIVATION_PROVENANCE.as_posix(),
                    "Expression-derivation provenance configuration drifted",
                )
            )

    provision_activity_events: dict[str, dict[str, Any]] = {}
    for provenance_path in sorted(
        (repo_root / SOURCE_ROOT).rglob(PROVISION_ACTIVITY_PROVENANCE_BASENAME)
    ):
        provenance_ref = _relative(provenance_path, repo_root)
        for index, event in enumerate(
            _load_jsonl(provenance_path, repo_root, issues),
            start=1,
        ):
            location = f"{provenance_ref}:{index}"
            _validate_payload(event, provenance_validator, location, issues)
            event_id = event.get("event_id")
            if not isinstance(event_id, str):
                continue
            if event_id in event_ids:
                issues.append((location, f"duplicate event_id: {event_id}"))
            event_ids.add(event_id)
            events_by_id[event_id] = event
            provision_activity_events[event_id] = event
            if event.get("event_type") != "annotation":
                issues.append(
                    (location, "provision-activity provenance must be annotation")
                )
            for field in ("inputs", "outputs"):
                for entity in event.get(field, []):
                    if not isinstance(entity, dict):
                        continue
                    ref = entity.get("ref")
                    expected_digest = entity.get("sha256")
                    if not (
                        isinstance(ref, str)
                        and ref.startswith("ToS/")
                        and isinstance(expected_digest, str)
                    ):
                        continue
                    path = repo_root / ref
                    if not path.is_file():
                        issues.append(
                            (location, f"provision-activity {field} is missing: {ref}")
                        )
                    elif _sha256(path) != expected_digest:
                        issues.append(
                            (location, f"provision-activity {field} digest drifted: {ref}")
                        )

    for anchor_location, event_ref in source_anchor_event_locations:
        if event_ref not in event_ids:
            issues.append(
                (anchor_location, f"unresolved source-anchor provenance event: {event_ref}")
            )

    membership_claim_ids: set[str] = set()
    responsibility_claim_ids: set[str] = set()
    responsibility_claim_subjects: dict[str, str] = {}
    responsibility_claim_predicates: dict[str, str] = {}
    responsibility_claim_objects: dict[str, str] = {}
    publication_claim_ids: set[str] = set()
    publication_claim_subjects: dict[str, str] = {}
    provision_activity_claim_ids: set[str] = set()
    provision_activity_claim_subjects: dict[str, str] = {}
    chronology_claim_ids: set[str] = set()
    chronology_claim_subjects: dict[str, str] = {}
    claim_routes = (
        ("membership-claims.jsonl", "contains_work", "collection", "work", membership_claim_ids),
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
                        if evidence_ref not in evidence_anchor_ids:
                            issues.append((location, f"unresolved source evidence anchor: {evidence_ref}"))
                    elif isinstance(evidence_ref, str) and evidence_ref.startswith("ToS/"):
                        if not (repo_root / evidence_ref).is_file():
                            issues.append((location, f"unresolved repository evidence ref: {evidence_ref}"))

    responsibility_predicate_subject_types = {
        "authored_by": {"work"},
        "translated_by": {"expression"},
        "edited_by": {"edition"},
        "afterword_by": {"edition"},
        "designed_by": {"edition"},
    }
    validated_responsibility_event_refs: set[str] = set()
    for claim_path in sorted((repo_root / SOURCE_ROOT).rglob("responsibility-claims.jsonl")):
        claim_file_ref = _relative(claim_path, repo_root)
        claim_file_digest = _sha256(claim_path)
        for index, claim in enumerate(
            _load_jsonl(claim_path, repo_root, issues),
            start=1,
        ):
            location = f"{claim_file_ref}:{index}"
            _validate_payload(claim, claim_validator, location, issues)
            claim_id = claim.get("claim_id")
            subject_ref = claim.get("subject_ref")
            if isinstance(claim_id, str):
                if claim_id in claim_ids:
                    issues.append((location, f"duplicate claim_id: {claim_id}"))
                claim_ids.add(claim_id)
                responsibility_claim_ids.add(claim_id)
                if isinstance(subject_ref, str):
                    responsibility_claim_subjects[claim_id] = subject_ref
                predicate_value = claim.get("predicate")
                if isinstance(predicate_value, str):
                    responsibility_claim_predicates[claim_id] = predicate_value
                object_value = claim.get("object")
                if isinstance(object_value, str):
                    responsibility_claim_objects[claim_id] = object_value
            if claim.get("claim_type") != "bibliographic":
                issues.append(
                    (
                        location,
                        "responsibility claim claim_type must be bibliographic",
                    )
                )
            if claim.get("assertion_layer") not in {
                "bibliographic_assertion",
                "scholarly_report",
            }:
                issues.append(
                    (
                        location,
                        "responsibility claim assertion_layer must be "
                        "bibliographic_assertion or scholarly_report",
                    )
                )
            predicate = claim.get("predicate")
            allowed_subject_types = responsibility_predicate_subject_types.get(
                str(predicate)
            )
            if allowed_subject_types is None:
                issues.append(
                    (
                        location,
                        f"unsupported responsibility predicate: {predicate}",
                    )
                )
            subject = records_by_id.get(str(subject_ref))
            if subject is None:
                issues.append(
                    (
                        location,
                        f"unresolved responsibility subject: {subject_ref}",
                    )
                )
            elif (
                allowed_subject_types is not None
                and subject[0].get("record_type") not in allowed_subject_types
            ):
                issues.append(
                    (
                        location,
                        f"{predicate} subject resolves to "
                        f"{subject[0].get('record_type')}, expected one of "
                        f"{sorted(allowed_subject_types)}",
                    )
                )
            require_record(claim.get("object"), "agent", location)
            event_ref = claim.get("provenance_event_ref")
            event = events_by_id.get(str(event_ref))
            if event is None:
                issues.append(
                    (
                        location,
                        f"unresolved provenance_event_ref: {event_ref}",
                    )
                )
            else:
                matching_outputs = [
                    output
                    for output in event.get("outputs", [])
                    if isinstance(output, dict)
                    and output.get("ref") == claim_file_ref
                    and output.get("sha256") == claim_file_digest
                    and output.get("role")
                    in {
                        "unreviewed-translation-responsibility-claims",
                        "unreviewed-evidence-bearing-responsibility-claims",
                    }
                ]
                if not matching_outputs:
                    issues.append(
                        (
                            location,
                            "responsibility claim provenance event does not "
                            "digest-bind the claim file",
                        )
                    )
                if str(event_ref) not in validated_responsibility_event_refs:
                    validated_responsibility_event_refs.add(str(event_ref))
                    for input_record in event.get("inputs", []):
                        if not isinstance(input_record, dict):
                            continue
                        input_ref = input_record.get("ref")
                        input_digest = input_record.get("sha256")
                        if (
                            isinstance(input_ref, str)
                            and input_ref.startswith("ToS/")
                            and isinstance(input_digest, str)
                        ):
                            input_path = repo_root / input_ref
                            if not input_path.is_file():
                                issues.append(
                                    (
                                        location,
                                        "responsibility claim provenance "
                                        f"input is missing: {input_ref}",
                                    )
                                )
                            elif _sha256(input_path) != input_digest:
                                issues.append(
                                    (
                                        location,
                                        "responsibility claim provenance "
                                        f"input digest drifted: {input_ref}",
                                    )
                                )
            for evidence_ref in claim.get("evidence_refs", []):
                if (
                    isinstance(evidence_ref, str)
                    and evidence_ref.startswith("tos.anchor.")
                ):
                    if evidence_ref not in evidence_anchor_ids:
                        issues.append(
                            (
                                location,
                                "unresolved source evidence anchor: "
                                f"{evidence_ref}",
                            )
                        )
                elif (
                    isinstance(evidence_ref, str)
                    and evidence_ref.startswith("ToS/")
                    and not (repo_root / evidence_ref).is_file()
                ):
                    issues.append(
                        (
                            location,
                            "unresolved repository evidence ref: "
                            f"{evidence_ref}",
                        )
                    )

    validated_publication_event_refs: set[str] = set()
    for claim_path in sorted((repo_root / SOURCE_ROOT).rglob("publication-claims.jsonl")):
        owner_path = claim_path.parent / "edition.json"
        owner = _load_json(owner_path, repo_root, issues)
        owner_id = owner.get("record_id") if owner is not None else None
        claim_file_ref = _relative(claim_path, repo_root)
        claim_file_digest = _sha256(claim_path)
        for index, claim in enumerate(
            _load_jsonl(claim_path, repo_root, issues),
            start=1,
        ):
            location = f"{claim_file_ref}:{index}"
            _validate_payload(claim, claim_validator, location, issues)
            claim_id = claim.get("claim_id")
            subject_ref = claim.get("subject_ref")
            if isinstance(claim_id, str):
                if claim_id in claim_ids:
                    issues.append((location, f"duplicate claim_id: {claim_id}"))
                claim_ids.add(claim_id)
                publication_claim_ids.add(claim_id)
                if isinstance(subject_ref, str):
                    publication_claim_subjects[claim_id] = subject_ref
            if claim.get("claim_type") != "bibliographic":
                issues.append(
                    (
                        location,
                        "publication claim claim_type must be bibliographic",
                    )
                )
            if claim.get("assertion_layer") not in {
                "bibliographic_assertion",
                "scholarly_report",
            }:
                issues.append(
                    (
                        location,
                        "publication claim assertion_layer must be "
                        "bibliographic_assertion or scholarly_report",
                    )
                )
            require_record(subject_ref, "edition", location)
            if owner_id != subject_ref:
                issues.append(
                    (
                        location,
                        "publication claim subject_ref differs from sibling "
                        "edition.json",
                    )
                )
            object_ref = claim.get("object")
            if (
                isinstance(object_ref, str)
                and object_ref.startswith("tos.")
                and object_ref not in records_by_id
                and object_ref not in event_ids
                and object_ref not in rights_ids
            ):
                issues.append(
                    (
                        location,
                        f"unresolved publication claim object: {object_ref}",
                    )
                )
            event_ref = claim.get("provenance_event_ref")
            event = events_by_id.get(str(event_ref))
            if event is None:
                issues.append(
                    (
                        location,
                        f"unresolved provenance_event_ref: {event_ref}",
                    )
                )
            else:
                expected_output = {
                    "ref": claim_file_ref,
                    "role": "unreviewed-evidence-bearing-publication-claims",
                    "sha256": claim_file_digest,
                }
                if expected_output not in event.get("outputs", []):
                    issues.append(
                        (
                            location,
                            "publication claim provenance event does not "
                            "digest-bind the claim file",
                        )
                    )
                if str(event_ref) not in validated_publication_event_refs:
                    validated_publication_event_refs.add(str(event_ref))
                    for input_record in event.get("inputs", []):
                        if not isinstance(input_record, dict):
                            continue
                        input_ref = input_record.get("ref")
                        input_digest = input_record.get("sha256")
                        if (
                            isinstance(input_ref, str)
                            and input_ref.startswith("ToS/")
                            and isinstance(input_digest, str)
                        ):
                            input_path = repo_root / input_ref
                            if not input_path.is_file():
                                issues.append(
                                    (
                                        location,
                                        "publication claim provenance input "
                                        f"is missing: {input_ref}",
                                    )
                                )
                            elif _sha256(input_path) != input_digest:
                                issues.append(
                                    (
                                        location,
                                        "publication claim provenance input "
                                        f"digest drifted: {input_ref}",
                                    )
                                )
            for evidence_ref in claim.get("evidence_refs", []):
                if (
                    isinstance(evidence_ref, str)
                    and evidence_ref.startswith("ToS/")
                    and not (repo_root / evidence_ref).is_file()
                ):
                    issues.append(
                        (
                            location,
                            "unresolved repository evidence ref: "
                            f"{evidence_ref}",
                        )
                    )

    provision_role_profile = {
        "publication": ({"publication_place"}, {"publisher"}),
        "production": ({"production_place"}, {"producer"}),
        "distribution": ({"distribution_place"}, {"distributor"}),
        "manufacture": ({"manufacture_place"}, {"manufacturer", "printer"}),
    }
    validated_provision_event_refs: set[str] = set()
    for claim_path in sorted(
        (repo_root / SOURCE_ROOT).rglob("provision-activity-claims.jsonl")
    ):
        owner_path = claim_path.parent / "edition.json"
        owner = _load_json(owner_path, repo_root, issues)
        owner_id = owner.get("record_id") if owner is not None else None
        claim_file_ref = _relative(claim_path, repo_root)
        claim_file_digest = _sha256(claim_path)
        for index, claim in enumerate(
            _load_jsonl(claim_path, repo_root, issues),
            start=1,
        ):
            location = f"{claim_file_ref}:{index}"
            _validate_payload(claim, claim_validator, location, issues)
            claim_id = claim.get("claim_id")
            subject_ref = claim.get("subject_ref")
            if isinstance(claim_id, str):
                if claim_id in claim_ids:
                    issues.append((location, f"duplicate claim_id: {claim_id}"))
                claim_ids.add(claim_id)
                provision_activity_claim_ids.add(claim_id)
                if isinstance(subject_ref, str):
                    provision_activity_claim_subjects[claim_id] = subject_ref
            if claim.get("claim_type") != "bibliographic":
                issues.append(
                    (location, "provision-activity claim_type must be bibliographic")
                )
            if claim.get("assertion_layer") != "bibliographic_assertion":
                issues.append(
                    (
                        location,
                        "provision-activity assertion_layer must be bibliographic_assertion",
                    )
                )
            if claim.get("predicate") != "provision_activity":
                issues.append(
                    (location, "provision-activity predicate must be provision_activity")
                )
            require_record(subject_ref, "edition", location)
            if owner_id != subject_ref:
                issues.append(
                    (
                        location,
                        "provision-activity subject_ref differs from sibling edition.json",
                    )
                )

            activity = claim.get("object")
            if isinstance(activity, dict):
                _validate_payload(
                    activity,
                    provision_activity_validator,
                    f"{location}#object",
                    issues,
                )
                issues.extend(
                    (location, issue)
                    for issue in _provision_temporal_issues(activity)
                )
                provision_kind = activity.get("provision_kind")
                allowed_roles = provision_role_profile.get(str(provision_kind))
                if allowed_roles is not None:
                    allowed_place_roles, allowed_agent_roles = allowed_roles
                    for place in activity.get("places", []):
                        if not isinstance(place, dict):
                            continue
                        if place.get("role") not in allowed_place_roles:
                            issues.append(
                                (
                                    location,
                                    f"{provision_kind} provision has incompatible place role: {place.get('role')}",
                                )
                            )
                        normalized_ref = place.get("normalized_place_ref")
                        if normalized_ref is not None:
                            require_record(normalized_ref, "place", location)
                    for agent in activity.get("agents", []):
                        if not isinstance(agent, dict):
                            continue
                        if agent.get("role") not in allowed_agent_roles:
                            issues.append(
                                (
                                    location,
                                    f"{provision_kind} provision has incompatible agent role: {agent.get('role')}",
                                )
                            )
                        normalized_ref = agent.get("normalized_agent_ref")
                        if normalized_ref is not None:
                            target = records_by_id.get(str(normalized_ref))
                            if target is None:
                                issues.append(
                                    (
                                        location,
                                        f"unresolved provision agent reference: {normalized_ref}",
                                    )
                                )
                            elif target[0].get("record_type") not in {
                                "agent",
                                "organization",
                            }:
                                issues.append(
                                    (
                                        location,
                                        f"{normalized_ref} resolves to {target[0].get('record_type')}, expected agent or organization",
                                    )
                                )
                if activity.get("event_posture") == "source_statement_only" and (
                    isinstance(activity.get("temporal"), dict)
                    and activity["temporal"].get("role") != "statement_date"
                ):
                    issues.append(
                        (
                            location,
                            "source_statement_only provision must keep its temporal role at statement_date",
                        )
                    )
            else:
                issues.append((location, "provision-activity object must be an object"))

            event_ref = claim.get("provenance_event_ref")
            event = provision_activity_events.get(str(event_ref))
            if event is None:
                issues.append(
                    (
                        location,
                        f"unresolved provision-activity provenance_event_ref: {event_ref}",
                    )
                )
            else:
                expected_output = {
                    "ref": claim_file_ref,
                    "role": "unreviewed-evidence-bearing-provision-activity-claims",
                    "sha256": claim_file_digest,
                }
                if expected_output not in event.get("outputs", []):
                    issues.append(
                        (
                            location,
                            "provision-activity provenance event does not digest-bind the claim file",
                        )
                    )
                validated_provision_event_refs.add(str(event_ref))
            for evidence_ref in claim.get("evidence_refs", []):
                if (
                    isinstance(evidence_ref, str)
                    and evidence_ref.startswith("ToS/")
                    and not (repo_root / evidence_ref).is_file()
                ):
                    issues.append(
                        (
                            location,
                            f"unresolved repository evidence ref: {evidence_ref}",
                        )
                    )
                elif (
                    isinstance(evidence_ref, str)
                    and evidence_ref.startswith("tos.")
                    and evidence_ref not in records_by_id
                ):
                    issues.append(
                        (location, f"unresolved identity evidence ref: {evidence_ref}")
                    )

    unused_provision_events = sorted(
        set(provision_activity_events) - validated_provision_event_refs
    )
    if unused_provision_events:
        issues.append(
            (
                SOURCE_ROOT.as_posix(),
                f"provision-activity provenance events are not referenced by claims: {unused_provision_events}",
            )
        )

    chronology_provenance_events = _load_jsonl(
        repo_root / WORK_CHRONOLOGY_PROVENANCE,
        repo_root,
        issues,
    )
    if len(chronology_provenance_events) != 1:
        issues.append(
            (
                WORK_CHRONOLOGY_PROVENANCE.as_posix(),
                "work chronology must have exactly one batch provenance event",
            )
        )
    chronology_event: dict[str, Any] | None = None
    for index, event in enumerate(chronology_provenance_events, start=1):
        location = f"{WORK_CHRONOLOGY_PROVENANCE.as_posix()}:{index}"
        _validate_payload(event, provenance_validator, location, issues)
        _validate_source_refs(repo_root, event, location, issues)
        event_id = event.get("event_id")
        if event_id != WORK_CHRONOLOGY_EVENT_REF:
            issues.append((location, "work chronology provenance event_id drifted"))
        if event.get("event_type") != "annotation":
            issues.append((location, "work chronology provenance must be annotation"))
        if event.get("agent_refs") != ["model:codex"]:
            issues.append((location, "work chronology provenance agent must be model:codex"))
        method = event.get("method", {})
        if (
            method.get("maker_type") != "model"
            or method.get("name")
            != "faceted-first-publication-chronology-materialization"
            or method.get("version") != "1"
        ):
            issues.append((location, "work chronology provenance method identity drifted"))
        if event.get("status") != "completed_with_warnings":
            issues.append(
                (location, "work chronology provenance must remain completed_with_warnings")
            )
        if isinstance(event_id, str):
            if event_id in event_ids:
                issues.append((location, f"duplicate event_id: {event_id}"))
            else:
                event_ids.add(event_id)
                events_by_id[event_id] = event
        if event_id == WORK_CHRONOLOGY_EVENT_REF:
            chronology_event = event

    actual_chronology_paths = {
        path.relative_to(repo_root)
        for path in (repo_root / SOURCE_ROOT).rglob(WORK_CHRONOLOGY_CLAIMS.name)
    }
    if actual_chronology_paths != {WORK_CHRONOLOGY_CLAIMS}:
        issues.append(
            (
                WORK_CHRONOLOGY_CLAIMS.as_posix(),
                "work chronology claim basename must exist only at its owned route",
            )
        )

    chronology_claim_path = repo_root / WORK_CHRONOLOGY_CLAIMS
    chronology_claim_digest = (
        _sha256(chronology_claim_path) if chronology_claim_path.is_file() else None
    )
    expected_chronology_evidence_refs: set[str] = set()
    for index, claim in enumerate(
        _load_jsonl(chronology_claim_path, repo_root, issues),
        start=1,
    ):
        location = f"{WORK_CHRONOLOGY_CLAIMS.as_posix()}:{index}"
        _validate_payload(claim, claim_validator, location, issues)
        claim_id = claim.get("claim_id")
        subject_ref = claim.get("subject_ref")
        if isinstance(claim_id, str):
            if claim_id in claim_ids:
                issues.append((location, f"duplicate claim_id: {claim_id}"))
            claim_ids.add(claim_id)
            chronology_claim_ids.add(claim_id)
            if isinstance(subject_ref, str):
                chronology_claim_subjects[claim_id] = subject_ref
        if claim.get("claim_type") != "bibliographic":
            issues.append((location, "work chronology claim_type must be bibliographic"))
        if claim.get("assertion_layer") != "scholarly_report":
            issues.append((location, "work chronology must remain a scholarly_report"))
        if claim.get("predicate") != "first_publication_chronology":
            issues.append(
                (location, "work chronology predicate must be first_publication_chronology")
            )
        require_record(subject_ref, "work", location)
        if claim.get("maker") != {
            "maker_type": "model",
            "agent_ref": "model:codex",
        }:
            issues.append((location, "work chronology maker must be model:codex"))
        if claim.get("provenance_event_ref") != WORK_CHRONOLOGY_EVENT_REF:
            issues.append((location, "work chronology claim cites the wrong provenance event"))
        if claim.get("epistemic_status") != "reported":
            issues.append((location, "work chronology must remain reported"))
        if claim.get("review_status") != "unreviewed" or claim.get("reviews") != []:
            issues.append((location, "work chronology claims must remain unreviewed"))
        if claim.get("visibility") != "public_metadata_only":
            issues.append((location, "work chronology must remain public metadata only"))

        evidence_refs = claim.get("evidence_refs", [])
        for evidence_ref in evidence_refs:
            if not isinstance(evidence_ref, str) or not evidence_ref.startswith("ToS/"):
                issues.append(
                    (location, "work chronology evidence must be a tracked repository path")
                )
                continue
            evidence_path = repo_root / evidence_ref
            if not evidence_path.is_file():
                issues.append(
                    (location, f"unresolved work chronology evidence: {evidence_ref}")
                )
            expected_chronology_evidence_refs.add(evidence_ref)
        if not any("authorial-witness-route" in str(ref) for ref in evidence_refs):
            issues.append((location, "work chronology lacks its ordered discovery receipt"))
        if not any("AUTHORIAL_WITNESS_ROUTE.md" in str(ref) for ref in evidence_refs):
            issues.append((location, "work chronology lacks its documentary synthesis"))

        chronology = claim.get("object")
        if isinstance(chronology, dict):
            _validate_payload(
                chronology,
                first_publication_chronology_validator,
                f"{location}['object']",
                issues,
            )
            interval = chronology.get("interval", {})
            start = interval.get("start") if isinstance(interval, dict) else None
            end = interval.get("end") if isinstance(interval, dict) else None
            if isinstance(start, str) and isinstance(end, str) and start > end:
                issues.append((location, "work chronology interval starts after it ends"))
            stages = chronology.get("stages", [])
            stage_dates = [
                stage.get("date")
                for stage in stages
                if isinstance(stage, dict) and isinstance(stage.get("date"), str)
            ]
            if stage_dates != sorted(stage_dates):
                issues.append((location, "work chronology stages are not date ordered"))
            sequence_posture = chronology.get("sequence_posture")
            boundary_meaning = (
                interval.get("boundary_meaning") if isinstance(interval, dict) else None
            )
            if sequence_posture == "single_event" and (
                len(stages) != 1 or boundary_meaning != "single_stage"
            ):
                issues.append(
                    (location, "single-event chronology must contain one single-stage boundary")
                )
            if sequence_posture == "staged_sequence" and (
                len(stages) < 2
                or boundary_meaning != "earliest_stage_to_sequence_completion"
            ):
                issues.append(
                    (location, "staged chronology must retain multiple sequence stages")
                )
            if stage_dates and isinstance(start, str) and not stage_dates[0].startswith(start):
                issues.append((location, "chronology interval start differs from first stage"))
            if stage_dates and isinstance(end, str) and not stage_dates[-1].startswith(end):
                issues.append((location, "chronology interval end differs from last stage"))
            for stage in stages:
                if not isinstance(stage, dict):
                    continue
                edition_ref = stage.get("edition_ref")
                if not isinstance(edition_ref, str):
                    continue
                require_record(edition_ref, "edition", location)
                edition = records_by_id.get(edition_ref)
                if edition is None or not isinstance(subject_ref, str):
                    continue
                expression_refs = edition[0].get("embodies_expression_refs", [])
                if not any(
                    records_by_id.get(str(expression_ref), ({}, Path()))[0].get("work_ref")
                    == subject_ref
                    for expression_ref in expression_refs
                ):
                    issues.append(
                        (location, f"chronology stage edition belongs to another Work: {edition_ref}")
                    )

    if chronology_event is not None and chronology_claim_digest is not None:
        expected_output = {
            "ref": WORK_CHRONOLOGY_CLAIMS.as_posix(),
            "role": "unreviewed-evidence-bearing-work-chronology-claims",
            "sha256": chronology_claim_digest,
        }
        if chronology_event.get("outputs") != [expected_output]:
            issues.append(
                (
                    WORK_CHRONOLOGY_PROVENANCE.as_posix(),
                    "work chronology provenance does not digest-bind the exact claim file",
                )
            )
        expected_input_refs = expected_chronology_evidence_refs | {
            CLAIM_SCHEMA.as_posix(),
            FIRST_PUBLICATION_CHRONOLOGY_SCHEMA.as_posix(),
        }
        actual_inputs = {
            input_record.get("ref"): input_record.get("sha256")
            for input_record in chronology_event.get("inputs", [])
            if isinstance(input_record, dict)
        }
        if set(actual_inputs) != expected_input_refs:
            issues.append(
                (
                    WORK_CHRONOLOGY_PROVENANCE.as_posix(),
                    "work chronology provenance inputs differ from the exact claim evidence set",
                )
            )
        for input_ref, input_digest in actual_inputs.items():
            input_path = repo_root / str(input_ref)
            if not input_path.is_file():
                issues.append(
                    (
                        WORK_CHRONOLOGY_PROVENANCE.as_posix(),
                        f"work chronology provenance input is missing: {input_ref}",
                    )
                )
            elif _sha256(input_path) != input_digest:
                issues.append(
                    (
                        WORK_CHRONOLOGY_PROVENANCE.as_posix(),
                        f"work chronology provenance input digest drifted: {input_ref}",
                    )
                )
        expected_configuration = {
            "works_materialized": 7,
            "chronology_claims_materialized": 7,
            "staged_sequence_claims": 1,
            "single_event_claims": 6,
            "chronology_claims_reviewed": 0,
            "composition_claims_created": 0,
            "source_text_admitted": False,
            "human_review_performed": False,
            "semantic_claims_created": 0,
            "canon_promotion_performed": False,
        }
        if chronology_event.get("method", {}).get("configuration") != expected_configuration:
            issues.append(
                (
                    WORK_CHRONOLOGY_PROVENANCE.as_posix(),
                    "work chronology provenance configuration differs from the bounded profile",
                )
            )

    for payload, path in (value for value in records_by_id.values() if value[0].get("record_type") == "collection"):
        location = _relative(path, repo_root)
        missing = sorted(set(payload.get("membership_claim_refs", [])) - membership_claim_ids)
        if missing:
            issues.append((location, f"unresolved membership claims: {missing}"))

    for payload, path in (
        value
        for value in records_by_id.values()
        if value[0].get("record_type") in {"work", "expression", "edition"}
    ):
        location = _relative(path, repo_root)
        subject_id = payload.get("record_id")
        actual_refs = set(payload.get("responsibility_claim_refs", []))
        missing = sorted(actual_refs - responsibility_claim_ids)
        if missing:
            issues.append((location, f"unresolved responsibility claims: {missing}"))
        misbound = sorted(
            claim_ref
            for claim_ref in actual_refs & responsibility_claim_ids
            if responsibility_claim_subjects.get(claim_ref) != subject_id
        )
        if misbound:
            issues.append(
                (
                    location,
                    f"responsibility claims belong to another subject: {misbound}",
                )
            )
        unreferenced = sorted(
            claim_id
            for claim_id, claim_subject_ref in responsibility_claim_subjects.items()
            if claim_subject_ref == subject_id and claim_id not in actual_refs
        )
        if unreferenced:
            issues.append(
                (
                    location,
                    f"subject responsibility claims are not referenced: {unreferenced}",
                )
            )
        if (
            payload.get("record_type") == "work"
            and location.startswith(
                "ToS/source-witnesses/works/friedrich-nietzsche/"
            )
        ):
            authorship_refs = sorted(
                claim_ref
                for claim_ref in actual_refs
                if responsibility_claim_predicates.get(claim_ref) == "authored_by"
            )
            if len(authorship_refs) != 1:
                issues.append(
                    (
                        location,
                        "current Nietzsche Work must reference exactly one "
                        f"authored_by claim; found {authorship_refs}",
                    )
                )
            elif (
                responsibility_claim_objects.get(authorship_refs[0])
                != "tos.agent.friedrich-nietzsche"
            ):
                issues.append(
                    (
                        location,
                        "current Nietzsche Work authored_by claim must resolve "
                        "to tos.agent.friedrich-nietzsche",
                    )
                )

    current_nietzsche_work_ids = {
        record_id
        for record_id, (payload, path) in records_by_id.items()
        if payload.get("record_type") == "work"
        and _relative(path, repo_root).startswith(
            "ToS/source-witnesses/works/friedrich-nietzsche/"
        )
    }
    chronology_subject_ids = set(chronology_claim_subjects.values())
    if chronology_subject_ids != current_nietzsche_work_ids:
        issues.append(
            (
                WORK_CHRONOLOGY_CLAIMS.as_posix(),
                "work chronology subjects do not close over the current Nietzsche Works",
            )
        )
    for work_id in sorted(current_nietzsche_work_ids):
        work, work_path = records_by_id[work_id]
        location = _relative(work_path, repo_root)
        actual_refs = set(work.get("chronology_claim_refs", []))
        expected_refs = {
            claim_id
            for claim_id, subject_ref in chronology_claim_subjects.items()
            if subject_ref == work_id
        }
        if actual_refs != expected_refs or len(expected_refs) != 1:
            issues.append(
                (
                    location,
                    "current Nietzsche Work must reference exactly one "
                    f"first_publication_chronology claim; found {sorted(actual_refs)}",
                )
            )

    for payload, path in (
        value
        for value in records_by_id.values()
        if value[0].get("record_type") == "edition"
    ):
        location = _relative(path, repo_root)
        edition_id = payload.get("record_id")
        actual_refs = set(payload.get("publication_claim_refs", []))
        missing = sorted(actual_refs - publication_claim_ids)
        if missing:
            issues.append((location, f"unresolved publication claims: {missing}"))
        misbound = sorted(
            claim_ref
            for claim_ref in actual_refs & publication_claim_ids
            if publication_claim_subjects.get(claim_ref) != edition_id
        )
        if misbound:
            issues.append(
                (
                    location,
                    f"publication claims belong to another edition: {misbound}",
                )
            )
        unreferenced = sorted(
            claim_id
            for claim_id, subject_ref in publication_claim_subjects.items()
            if subject_ref == edition_id and claim_id not in actual_refs
        )
        if unreferenced:
            issues.append(
                (
                    location,
                    f"sibling publication claims are not referenced: {unreferenced}",
                )
            )

        actual_provision_refs = set(
            payload.get("provision_activity_claim_refs", [])
        )
        missing_provision_refs = sorted(
            actual_provision_refs - provision_activity_claim_ids
        )
        if missing_provision_refs:
            issues.append(
                (
                    location,
                    f"unresolved provision-activity claims: {missing_provision_refs}",
                )
            )
        misbound_provision_refs = sorted(
            claim_ref
            for claim_ref in actual_provision_refs & provision_activity_claim_ids
            if provision_activity_claim_subjects.get(claim_ref) != edition_id
        )
        if misbound_provision_refs:
            issues.append(
                (
                    location,
                    "provision-activity claims belong to another edition: "
                    f"{misbound_provision_refs}",
                )
            )
        unreferenced_provision_refs = sorted(
            claim_id
            for claim_id, subject_ref in provision_activity_claim_subjects.items()
            if subject_ref == edition_id
            and claim_id not in actual_provision_refs
        )
        if unreferenced_provision_refs:
            issues.append(
                (
                    location,
                    "sibling provision-activity claims are not referenced: "
                    f"{unreferenced_provision_refs}",
                )
            )

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
    claim_entry_schema = catalog_schema.get("$defs", {}).get("claim_entry")
    if isinstance(claim_entry_schema, dict):
        claim_entry_schema = dict(claim_entry_schema)
        claim_entry_schema["$defs"] = {
            "tosId": catalog_schema["$defs"]["tosId"],
        }
        claim_entry_class = validator_for(claim_entry_schema)
        claim_entry_class.check_schema(claim_entry_schema)
        claim_entry_validator = claim_entry_class(
            claim_entry_schema,
            format_checker=FormatChecker(),
        )
        claim_catalog_path = repo_root / CLAIM_CATALOG_PATH
        for index, entry in enumerate(
            _load_jsonl(claim_catalog_path, repo_root, issues),
            start=1,
        ):
            _validate_payload(
                entry,
                claim_entry_validator,
                f"{_relative(claim_catalog_path, repo_root)}:{index}",
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
    parser.add_argument(
        "--source-anchor-v2-lab-only",
        action="store_true",
        help="resolve and report only the public synthetic source-anchor v2 A/B/C",
    )
    parser.add_argument(
        "--source-text-layer-lab-only",
        action="store_true",
        help="replay and report only the public synthetic source-text-layer A/B/C",
    )
    parser.add_argument(
        "--provenance-v2-lab-only",
        action="store_true",
        help="replay and report only the public synthetic provenance-event-v2 A/B/C",
    )
    args = parser.parse_args(argv)

    if args.source_anchor_v2_lab_only:
        issues, report = validate_source_anchor_v2_lab(args.repo_root)
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
        if issues:
            print("Source-anchor v2 laboratory validation failed.", file=sys.stderr)
            for location, message in issues:
                print(f"- {location}: {message}", file=sys.stderr)
            return 1
        print("[boundary] synthetic mechanical resolution only; no source, review, translation, semantic, or canon authority")
        return 0

    if args.source_text_layer_lab_only:
        issues, report = validate_source_text_layer_lab(args.repo_root)
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
        if issues:
            print("Source-text-layer laboratory validation failed.", file=sys.stderr)
            for location, message in issues:
                print(f"- {location}: {message}", file=sys.stderr)
            return 1
        print("[boundary] public synthetic layer mechanics only; no human review, accepted text, translation, linguistic, semantic, graph, canon, or publication authority")
        return 0

    if args.provenance_v2_lab_only:
        issues, report = validate_provenance_v2_lab(args.repo_root)
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
        if issues:
            print("Provenance-event-v2 laboratory validation failed.", file=sys.stderr)
            for location, message in issues:
                print(f"- {location}: {message}", file=sys.stderr)
            return 1
        print("[boundary] public synthetic provenance mechanics only; unsigned receipts and green closure do not establish execution truth, content quality, rights, human review, semantics, canon, or publication authority")
        return 0

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

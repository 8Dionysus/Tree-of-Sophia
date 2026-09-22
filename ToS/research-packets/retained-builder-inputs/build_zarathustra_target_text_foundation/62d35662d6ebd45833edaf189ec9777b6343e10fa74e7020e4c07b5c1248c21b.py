#!/usr/bin/env python3
"""Materialize one real, non-promoting Antonovsky target-text foundation.

The exact PDF, Poppler bbox output, and extracted Russian layer remain mode-0600
below the ignored owner-local boundary. Tracked records retain only identity,
selectors, digests, ranges, provenance, rights posture, and closed authority.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import stat
import subprocess
import sys
import unicodedata
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
PLAN_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1/"
    "za-i-vorrede-1-antonovsky-1911-target-text-foundation.plan.v1.json"
)
ANCHOR_SCHEMA_REF = "ToS/contracts/source-anchor-v2.schema.json"
LAYER_SCHEMA_REF = "ToS/contracts/source-text-layer.schema.json"
UNIT_SCHEMA_REF = "ToS/contracts/source-text-unit-packet-v1.schema.json"
PROVENANCE_SCHEMA_REF = "ToS/contracts/provenance-event-v2.schema.json"
BUILDER_REF = "scripts/build_zarathustra_target_text_foundation.py"
EVENT_ID = (
    "tos.event.native-extraction.za-i-vorrede-1."
    "antonovsky-1911-pdf-page-6-opening.2026-08-12"
)


class TargetTextFoundationError(RuntimeError):
    """Raised when the exact PDF boundary or a fail-closed gate drifts."""


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _json_bytes(payload: Any) -> bytes:
    return (
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def _canonical_value_sha256(payload: Any) -> str:
    return _sha256_bytes(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    )


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _direct_children(element: ET.Element, name: str) -> list[ET.Element]:
    return [child for child in list(element) if _local_name(child.tag) == name]


def _binding(repo_root: Path, ref: str) -> dict[str, str]:
    return {"ref": ref, "sha256": _sha256(repo_root / ref)}


def _pdftotext_binary_and_version(plan: dict[str, Any]) -> tuple[Path, str]:
    resolved = shutil.which("pdftotext")
    if resolved is None:
        raise TargetTextFoundationError("pdftotext is unavailable")
    binary = Path(resolved).resolve()
    result = subprocess.run(
        [str(binary), "-v"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    version_output = (result.stdout + result.stderr).decode("utf-8", errors="strict")
    first_line = version_output.splitlines()[0] if version_output.splitlines() else ""
    prefix = "pdftotext version "
    if result.returncode != 0 or not first_line.startswith(prefix):
        raise TargetTextFoundationError("cannot establish exact pdftotext version")
    version = first_line[len(prefix) :]
    expected = plan["extraction_policy"]["software_version"]
    if version != expected:
        raise TargetTextFoundationError(
            f"pdftotext version drifted: expected {expected}, observed {version}"
        )
    return binary, version


def _extract_bbox(
    source_path: Path, plan: dict[str, Any]
) -> tuple[bytes, bytes, list[str], tuple[float, float, float, float], Path, str]:
    binary, version = _pdftotext_binary_and_version(plan)
    page = str(plan["selector"]["page_number"])
    argv = [str(binary), "-f", page, "-l", page, "-bbox-layout", str(source_path), "-"]
    result = subprocess.run(
        argv,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise TargetTextFoundationError(
            f"pdftotext bbox extraction failed with exit code {result.returncode}"
        )
    policy = plan["extraction_policy"]
    bbox_bytes = result.stdout
    if len(bbox_bytes) != policy["expected_bbox_bytes"]:
        raise TargetTextFoundationError("Poppler bbox byte count drifted")
    if _sha256_bytes(bbox_bytes) != policy["expected_bbox_sha256"]:
        raise TargetTextFoundationError("Poppler bbox digest drifted")
    warnings = result.stderr.decode("utf-8", errors="strict").splitlines()
    if warnings != [policy["expected_warning"]] * policy["expected_warning_count"]:
        raise TargetTextFoundationError("Poppler warning surface drifted")

    try:
        root = ET.fromstring(bbox_bytes)
    except ET.ParseError as exc:
        raise TargetTextFoundationError(f"cannot parse Poppler bbox XML: {exc}") from exc
    pages = [element for element in root.iter() if _local_name(element.tag) == "page"]
    if len(pages) != 1:
        raise TargetTextFoundationError(f"expected one Poppler page, observed {len(pages)}")
    page_element = pages[0]
    selector = plan["selector"]
    if (
        float(page_element.attrib.get("width", "nan")) != selector["page_width_points"]
        or float(page_element.attrib.get("height", "nan"))
        != selector["page_height_points"]
    ):
        raise TargetTextFoundationError("selected PDF page geometry drifted")

    first_digest = selector["first_word_sha256"]
    candidate_blocks: list[ET.Element] = []
    for block in (element for element in page_element.iter() if _local_name(element.tag) == "block"):
        lines = _direct_children(block, "line")
        words = _direct_children(lines[0], "word") if lines else []
        if words and _sha256_bytes((words[0].text or "").encode("utf-8")) == first_digest:
            candidate_blocks.append(block)
    if len(candidate_blocks) != selector["block_match_count"]:
        raise TargetTextFoundationError("opening-paragraph Poppler block match count drifted")
    lines = _direct_children(candidate_blocks[0], "line")
    start = selector["selected_line_start"]
    count = selector["selected_line_count"]
    if len(lines) <= start + count:
        raise TargetTextFoundationError("selected Poppler line window has no next-line guard")
    selected_lines = lines[start : start + count]
    next_words = _direct_children(lines[start + count], "word")
    if not next_words or _sha256_bytes((next_words[0].text or "").encode("utf-8")) != selector[
        "next_line_first_word_sha256"
    ]:
        raise TargetTextFoundationError("next-line boundary guard drifted")

    line_texts: list[str] = []
    selected_words: list[ET.Element] = []
    for line in selected_lines:
        words = _direct_children(line, "word")
        if not words or any(word.text in {None, ""} or list(word) for word in words):
            raise TargetTextFoundationError("selected Poppler line has invalid word elements")
        selected_words.extend(words)
        line_texts.append(" ".join(word.text or "" for word in words))
    if len(selected_words) != selector["expected_word_count"]:
        raise TargetTextFoundationError("selected Poppler word count drifted")
    if _sha256_bytes((selected_words[-1].text or "").encode("utf-8")) != selector[
        "last_word_sha256"
    ]:
        raise TargetTextFoundationError("opening-paragraph final-word guard drifted")

    bounds = (
        min(float(line.attrib["xMin"]) for line in selected_lines),
        min(float(line.attrib["yMin"]) for line in selected_lines),
        max(float(line.attrib["xMax"]) for line in selected_lines),
        max(float(line.attrib["yMax"]) for line in selected_lines),
    )
    region = selector["region_points"]
    expected_bounds = (
        region["x"],
        region["y"],
        region["x"] + region["width"],
        region["y"] + region["height"],
    )
    if any(abs(observed - expected) > 0.000001 for observed, expected in zip(bounds, expected_bounds, strict=True)):
        raise TargetTextFoundationError("opening-paragraph Poppler region drifted")

    text = "\n".join(line_texts)
    content = text.encode("utf-8")
    if "\r" in text or text.endswith("\n"):
        raise TargetTextFoundationError("extracted target text violates line-ending policy")
    if len(content) != policy["expected_content_bytes"]:
        raise TargetTextFoundationError("target-text byte count drifted")
    if len(text) != policy["expected_content_codepoints"]:
        raise TargetTextFoundationError("target-text code-point count drifted")
    if _sha256_bytes(content) != policy["expected_content_sha256"]:
        raise TargetTextFoundationError("target-text digest drifted")
    return bbox_bytes, content, line_texts, bounds, binary, version


def _anchor_record(plan: dict[str, Any], plan_digest: str) -> dict[str, Any]:
    scope = plan["scope"]
    ids = plan["opaque_ids"]
    region = plan["selector"]["region_points"]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json",
        "schema_version": "tos_source_anchor_v2",
        "anchor_id": ids["anchor_id"],
        "passage_id": ids["passage_id"],
        "target": {
            "item_id": scope["item_ref"],
            "file_id": scope["file_ref"],
            "file_sha256": scope["file_sha256"],
            "media_type": "application/pdf",
        },
        "selector_payload": {
            "kind": "selector_expression",
            "expression": {
                "mode": "single",
                "selector": {
                    "state": {
                        "state_type": "digest_state",
                        "representation_ref": scope["source_relative_ref"],
                        "representation_sha256": scope["file_sha256"],
                        "media_type": "application/pdf",
                        "character_normalization": "none",
                    },
                    "selector": {
                        "type": "page_region",
                        "page_identity": {"page_number": plan["selector"]["page_number"]},
                        "x": region["x"],
                        "y": region["y"],
                        "width": region["width"],
                        "height": region["height"],
                        "coordinate_space": "points",
                        "source_width": plan["selector"]["page_width_points"],
                        "source_height": plan["selector"]["page_height_points"],
                    },
                },
            },
        },
        "publication_boundary": {
            "record_storage": "tracked",
            "source_content_visibility": "local_only",
            "source_text_in_record": False,
            "public_payload_expected": False,
        },
        "selector_method": {
            "maker_type": "software",
            "method": plan["extraction_policy"]["method"],
            "version": plan["extraction_policy"]["method_version"],
            "configuration_ref": PLAN_REF,
            "configuration_digest": plan_digest,
        },
        "resolution_status": "mechanically_resolved",
        "review_status": "unreviewed",
        "provenance_event_ref": EVENT_ID,
        "anchor_version": 1,
        "supersedes_anchor_ref": None,
        "review_ref": None,
    }


def _layer_record(
    plan: dict[str, Any],
    plan_digest: str,
    anchor_digest: str,
    rights_digest: str,
    content: bytes,
) -> dict[str, Any]:
    scope = plan["scope"]
    ids = plan["opaque_ids"]
    content_digest = _sha256_bytes(content)
    text = content.decode("utf-8")
    visual = plan["visual_check"]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json",
        "schema_version": "tos_source_text_layer_v1",
        "layer_id": ids["layer_id"],
        "layer_role": "raw_ocr",
        "source_binding": {
            "work_ref": scope["work_ref"],
            "expression_ref": scope["expression_ref"],
            "edition_ref": scope["edition_ref"],
            "item_ref": scope["item_ref"],
            "source_file_ref": scope["file_ref"],
            "source_file_sha256": scope["file_sha256"],
            "anchor_contract": "tos_source_anchor_v2",
            "anchors": [
                {
                    "anchor_id": ids["anchor_id"],
                    "anchor_record_ref": plan["outputs"]["anchor_ref"],
                    "anchor_record_sha256": anchor_digest,
                }
            ],
        },
        "representation": {
            "content_file_id": f"tos.file.sha256.{content_digest}",
            "content_ref": plan["outputs"]["private_content_ref"],
            "content_sha256": content_digest,
            "media_type": "text/plain",
            "charset": "UTF-8",
            "language": "ru",
            "text_scope": {
                "start": 0,
                "end": len(text),
                "position_unit": "unicode_code_point",
                "interval": "half_open",
            },
            "character_normalization": "none",
            "line_break_posture": "source_preserved",
            "storage": "ignored_local",
            "content_visibility": "local_only",
            "tracked_content": False,
            "publication_authorized": False,
            "rights_record_refs": [{"ref": scope["rights_ref"], "sha256": rights_digest}],
            "publication_authority_refs": [],
        },
        "derivation": {
            "method": "structural_extraction",
            "input_layers": [],
            "maker": {
                "maker_type": "software",
                "agent_ref": "software:tos-zarathustra-target-text-foundation-builder",
                "method": plan["extraction_policy"]["method"],
                "version": plan["extraction_policy"]["method_version"],
                "configuration_ref": PLAN_REF,
                "configuration_digest": plan_digest,
            },
            "preservation_goal": "source_near",
            "loss_posture": "lossy_for_declared_use",
            "silent_changes_allowed": False,
            "change_payload": {"kind": "none"},
        },
        "editorial_policy": {
            "policy_ref": PLAN_REF,
            "policy_sha256": plan_digest,
            "transcription_goal": "machine_candidate",
            "historical_language_preserved": True,
            "printing_errors_silently_corrected": False,
            "typography_posture": "encode_explicitly",
            "layout_posture": "encode_explicitly",
            "unicode_normalization": "none",
            "uncertainty_representation": "explicit-never-silent",
            "method_declared": True,
        },
        "uncertainty": {
            "status": "unresolved",
            "annotations": [
                {
                    "annotation_id": ids["uncertainty_annotation_id"],
                    "anchor_ref": ids["anchor_id"],
                    "kind": "ambiguous_reading",
                    "alternatives": [
                        {
                            "value_sha256": visual["embedded_token_sequence_sha256"],
                            "value_in_record": False,
                            "value": None,
                            "status": "possible",
                        },
                        {
                            "value_sha256": visual["visually_joined_candidate_sha256"],
                            "value_in_record": False,
                            "value": None,
                            "status": "possible",
                        },
                    ],
                    "resolution": "proposed",
                }
            ],
        },
        "admission": {
            "mechanical_status": "fixity_verified",
            "review_status": "unreviewed",
            "review_ref": None,
            "human_review_performed": False,
            "human_language_competence": "not_assessed",
            "language_competence_evidence_refs": [],
            "accepted_uses": [],
            "automatic_validation_complete": True,
            "model_output_is_ground_truth": False,
            "validator_proves_content_truth": False,
            "routine_human_task_created": False,
            "promotion_authorized": False,
        },
        "provenance_event_ref": EVENT_ID,
        "authority_boundary": (
            "a source text layer is one immutable, source-returnable representation with "
            "explicit derivation, uncertainty, review, competence, rights, and use scope; "
            "mechanical validation, model output, normalization, or agreement with another "
            "layer does not make it accepted source text, translation evidence, linguistic "
            "truth, semantic evidence, graph truth, canon authority, or publication permission"
        ),
        "layer_version": 1,
        "supersedes_layer_ref": None,
    }


def _unit_packet(plan: dict[str, Any], content: bytes) -> dict[str, Any]:
    scope = plan["scope"]
    ids = plan["opaque_ids"]
    text = content.decode("utf-8")
    content_digest = _sha256_bytes(content)
    anchor_ids = ids["anchor_ids"]
    unit_ids = ids["unit_ids"]
    lines = text.split("\n")
    segments: list[tuple[str, int, int]] = []
    cursor = 0
    for index, line in enumerate(lines):
        end = cursor + len(line)
        segments.append(("physical_line", cursor, end))
        cursor = end
        if index < len(lines) - 1:
            segments.append(("whitespace", cursor, cursor + 1))
            cursor += 1
    if cursor != len(text) or len(lines) != 6 or len(segments) != 11:
        raise TargetTextFoundationError("target-layout coverage construction drifted")

    anchors = [
        {
            "anchor_ref": anchor_ids[0],
            "anchor_role": "scope",
            "ordinal": 1,
            "text_layer_ref": plan["outputs"]["text_layer_ref"],
            "text_layer_sha256": content_digest,
            "selector": {
                "type": "text_position",
                "start": 0,
                "end": len(text),
                "position_unit": "unicode_code_point",
                "interval": "half_open",
            },
            "exact_sha256": content_digest,
            "source_return": {
                "required": True,
                "locator_ref": plan["outputs"]["private_content_ref"],
            },
        }
    ]
    units: list[dict[str, Any]] = []
    for ordinal, ((kind, start, end), anchor_id, unit_id) in enumerate(
        zip(segments, anchor_ids[1:], unit_ids, strict=True), start=2
    ):
        exact = text[start:end].encode("utf-8")
        anchors.append(
            {
                "anchor_ref": anchor_id,
                "anchor_role": "content" if kind == "physical_line" else "whitespace",
                "ordinal": ordinal,
                "text_layer_ref": plan["outputs"]["text_layer_ref"],
                "text_layer_sha256": content_digest,
                "selector": {
                    "type": "text_position",
                    "start": start,
                    "end": end,
                    "position_unit": "unicode_code_point",
                    "interval": "half_open",
                },
                "exact_sha256": _sha256_bytes(exact),
                "source_return": {
                    "required": True,
                    "locator_ref": plan["outputs"]["private_content_ref"],
                },
            }
        )
        units.append(
            {
                "unit_id": unit_id,
                "unit_version": 1,
                "supersedes_unit_ref": None,
                "unit_kind": kind,
                "surface_posture": "source_bearing",
                "continuity": "contiguous",
                "ordered_anchor_refs": [anchor_id],
                "ordered_child_unit_refs": [],
                "parent_unit_refs": [],
                "boundary_posture": "source_attested",
                "certainty": {
                    "value": 1.0,
                    "meaning": "maker-declared-boundary-confidence-not-truth-probability",
                },
                "source_text_mutated": False,
                "semantic_promotion": False,
                "identity_policy": (
                    "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis"
                ),
                "status_reason": (
                    "The unit records only one exact Poppler bbox line or its line-break "
                    "code point in the unreviewed embedded-text layer; it is not accepted "
                    "Russian, a linguistic unit, or a translation alignment."
                ),
            }
        )

    method = {
        "maker_kind": "software",
        "agent_ref": "software:tos-zarathustra-target-text-foundation-builder",
        "method_name": "exact Poppler bbox source-layout observation",
        "method_version": "1",
        "software_refs": [BUILDER_REF],
        "model_ref": None,
        "configuration_ref": PLAN_REF,
        "locale": "ru",
        "unicode_version": unicodedata.unidata_version,
        "unicode_revision": None,
        "tailoring_ref": None,
        "provenance_event_ref": EVENT_ID,
        "made_at": plan["created_at"],
        "output_posture": "method_result_not_source_or_linguistic_truth",
    }
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "source-text-unit-packet-v1.schema.json"
        ),
        "schema_version": "tos_source_text_unit_packet_v1",
        "packet_id": ids["packet_id"],
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "source_bound",
        "source_scope": {
            "work_ref": scope["work_ref"],
            "expression_ref": scope["expression_ref"],
            "edition_ref": scope["edition_ref"],
            "item_ref": scope["item_ref"],
            "file_ref": scope["file_ref"],
            "file_sha256": scope["file_sha256"],
        },
        "source_layer": {
            "text_layer_ref": plan["outputs"]["text_layer_ref"],
            "text_layer_sha256": content_digest,
            "language": "ru",
            "media_type": "text/plain; charset=utf-8",
            "unicode_form": "source_preserved",
            "position_unit": "unicode_code_point",
            "interval": "half_open",
            "immutable": True,
            "visibility": "local_only",
            "publication_authorized": False,
        },
        "schemes": [
            {
                "scheme_id": ids["scheme_id"],
                "scheme_version": 1,
                "supersedes_scheme_ref": None,
                "scheme_name": "Antonovsky 1911 embedded-PDF bbox source-layout observation",
                "analysis_role": "source_layout",
                "unit_kinds": ["physical_line", "whitespace"],
                "boundary_basis": "source_layout",
                "policies": {
                    "punctuation": "included_in_neighbor",
                    "whitespace": "standalone_units",
                    "line_break": "standalone_units",
                    "hyphenation": "preserve_source",
                    "normalization": "no-text-mutation-separate-successor-layer",
                    "overlap": "forbid",
                    "unreported_gaps_allowed": False,
                },
                "method": method,
                "authority_limit": {
                    "algorithmic_output_is_source_truth": False,
                    "algorithmic_output_is_linguistic_truth": False,
                    "model_subword_is_lexeme": False,
                    "unit_identity_is_semantic_identity": False,
                    "text_mutation_allowed": False,
                },
                "identity_policy": (
                    "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis"
                ),
            }
        ],
        "anchors": anchors,
        "units": units,
        "segmentations": [
            {
                "segmentation_id": ids["segmentation_id"],
                "segmentation_version": 1,
                "supersedes_segmentation_ref": None,
                "scheme_ref": ids["scheme_id"],
                "ordered_unit_refs": unit_ids,
                "coverage": {
                    "scope_anchor_ref": anchor_ids[0],
                    "coverage_posture": "exhaustive_nonoverlapping",
                    "excluded_anchor_refs": [],
                    "unreported_gaps_allowed": False,
                    "overlap_requires_declaration": True,
                    "source_reconstruction_required": True,
                },
                "competing_segmentation_refs": [],
                "maker": method,
                "status": "observed_source_structure",
                "status_reason": (
                    "Only six exact embedded-PDF bbox lines are observed; word joining is "
                    "the declared machine rule and no sentence, token, accepted text, or "
                    "translation boundary is asserted."
                ),
                "review_refs": [],
                "source_text_authority": False,
                "linguistic_authority": False,
                "semantic_authority": False,
                "declared_uses": ["navigation", "source_observation"],
                "identity_policy": (
                    "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries"
                ),
            }
        ],
        "reviews": [],
        "projections": [],
        "rights_and_visibility": {
            "source_visibility": "local_only",
            "packet_visibility": "public_metadata_only",
            "effective_visibility": "local_only",
            "rights_record_refs": [scope["rights_ref"]],
            "private_source_used": True,
            "publication_authorized": False,
            "inheritance_policy": "most-restrictive-source-packet-and-destination-wins",
        },
        "authority_boundary": {
            "source_role": "authority",
            "tree_role": "orientation",
            "graph_role": "relation",
            "projection_is_owner_truth": False,
            "validators_prove_mechanics_not_truth": True,
            "segmentation_is_method_result_not_source_truth": True,
            "model_token_is_not_linguistic_token": True,
            "source_unit_is_not_lexeme_sign_or_concept": True,
            "legacy_bulk_migration_authorized": False,
        },
    }


def _provenance_event(
    *,
    repo_root: Path,
    plan: dict[str, Any],
    plan_digest: str,
    source_path: Path,
    bbox: bytes,
    content: bytes,
    output_records: dict[str, bytes],
    pdftotext_binary: Path,
    pdftotext_version: str,
) -> dict[str, Any]:
    builder_digest = _sha256(repo_root / BUILDER_REF)
    executable = Path(sys.executable).resolve()
    argv = [
        "python",
        BUILDER_REF,
        "--build",
        "--local-input-root",
        "/srv/AbyssOS/Tree-of-Sophia",
        "--local-output-root",
        "/srv/AbyssOS/Tree-of-Sophia",
    ]
    private_outputs = [
        (
            plan["outputs"]["private_content_ref"],
            "ignored-local-raw-embedded-text",
            content,
            "text/plain; charset=utf-8",
        ),
        (
            plan["outputs"]["private_bbox_ref"],
            "ignored-local-poppler-bbox-intermediate",
            bbox,
            "application/xhtml+xml",
        ),
    ]
    outputs = [
        {
            "entity_ref": ref,
            "role": role,
            "sha256": _sha256_bytes(payload),
            "size_bytes": len(payload),
            "media_type": media_type,
            "availability": "ignored_local",
            "content_disclosure": "private_content",
            "fixity_verified": True,
            "fixity_verified_at": plan["created_at"],
        }
        for ref, role, payload, media_type in private_outputs
    ]
    for ref, payload in output_records.items():
        outputs.append(
            {
                "entity_ref": ref,
                "role": "tracked-text-free-foundation-record",
                "sha256": _sha256_bytes(payload),
                "size_bytes": len(payload),
                "media_type": "application/json",
                "availability": "tracked",
                "content_disclosure": "public_metadata_only",
                "fixity_verified": True,
                "fixity_verified_at": plan["created_at"],
            }
        )
    derivations = [
        {
            "derivation_id": f"tos.derivation.za-i-vorrede-1.antonovsky-1911-target-text-foundation.{ordinal}",
            "input_entity_ref": plan["scope"]["source_relative_ref"],
            "output_entity_ref": output["entity_ref"],
            "relation": "selection_from",
            "influence_asserted": True,
            "description": (
                "The exact fixity-bound PDF, tracked plan, pinned Poppler version, and "
                "declared word/line rule determine this bounded text, address, or layout record."
            ),
        }
        for ordinal, output in enumerate(outputs, start=1)
    ]
    total_output_bytes = sum(output["size_bytes"] for output in outputs)
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json",
        "schema_version": "tos_provenance_event_v2",
        "event_id": EVENT_ID,
        "event_version": 1,
        "supersedes_event_ref": None,
        "record_binding": {
            "manifest_ref": PLAN_REF,
            "digest_algorithm": "sha256",
            "digest_scope": "exact_event_record_bytes",
        },
        "activity": {
            "event_type": "native_extraction",
            "started_at": plan["created_at"],
            "ended_at": plan["created_at"],
            "status": "completed_with_warnings",
            "terminal_reason": None,
            "exit_code": 0,
            "warnings": [
                "Poppler emitted seventeen Invalid Font Weight warnings while returning the frozen bbox bytes.",
                "The embedded text is an unreviewed raw candidate and one visual letterspacing divergence remains unresolved.",
                "The exact PDF, bbox intermediate, and extracted text remain local-only; publication is unauthorized.",
                "The unsigned self-recorded event proves neither execution truth nor source fidelity.",
            ],
        },
        "entities": {
            "inputs": [
                {
                    "entity_ref": plan["scope"]["source_relative_ref"],
                    "role": "fixity-verified-local-antonovsky-1911-pdf",
                    "sha256": plan["scope"]["file_sha256"],
                    "size_bytes": source_path.stat().st_size,
                    "media_type": "application/pdf",
                    "availability": "owner_local",
                    "content_disclosure": "private_content",
                    "fixity_verified": True,
                    "fixity_verified_at": plan["created_at"],
                },
                {
                    "entity_ref": PLAN_REF,
                    "role": "tracked-text-free-materialization-plan",
                    "sha256": plan_digest,
                    "size_bytes": (repo_root / PLAN_REF).stat().st_size,
                    "media_type": "application/json",
                    "availability": "tracked",
                    "content_disclosure": "public_metadata_only",
                    "fixity_verified": True,
                    "fixity_verified_at": plan["created_at"],
                },
            ],
            "outputs": outputs,
            "byproducts": [],
        },
        "derivations": derivations,
        "responsibility": [
            {
                "agent_ref": "software:tos-zarathustra-target-text-foundation-builder",
                "agent_kind": "software",
                "role": "executor",
                "responsibility_posture": "performed",
                "evidence_binding": {"ref": BUILDER_REF, "sha256": builder_digest},
                "human_evidence_status": "not_applicable",
            },
            {
                "agent_ref": "model:openai-codex",
                "agent_kind": "model",
                "role": "observer",
                "responsibility_posture": "observed",
                "evidence_binding": {"ref": PLAN_REF, "sha256": plan_digest},
                "human_evidence_status": "not_applicable",
            },
        ],
        "method": {
            "procedure": {
                "name": plan["extraction_policy"]["method"],
                "version": plan["extraction_policy"]["method_version"],
                "purpose": plan["question"],
            },
            "command_capture": {
                "disclosure": "inline",
                "argv": argv,
                "argv_sha256": _canonical_value_sha256(argv),
                "withholding_reason": None,
            },
            "configuration_binding": {"ref": PLAN_REF, "sha256": plan_digest},
            "software_components": [
                {
                    "name": "Tree of Sophia Zarathustra target-text foundation builder",
                    "version": "1",
                    "role": "native-extraction-and-record-builder",
                    "artifact_ref": BUILDER_REF,
                    "artifact_sha256": builder_digest,
                    "verification_status": "verified",
                },
                {
                    "name": "pdftotext",
                    "version": pdftotext_version,
                    "role": "pdf-embedded-text-bbox-extractor",
                    "artifact_ref": "runtime:pdftotext-executable",
                    "artifact_sha256": _sha256(pdftotext_binary),
                    "verification_status": "verified",
                },
                {
                    "name": platform.python_implementation(),
                    "version": platform.python_version(),
                    "role": "language-runtime",
                    "artifact_ref": "runtime:cpython-executable",
                    "artifact_sha256": _sha256(executable),
                    "verification_status": "verified",
                },
            ],
            "model_invocations": [],
            "environment": {
                "runtime": platform.python_implementation(),
                "runtime_version": platform.python_version(),
                "runtime_artifact_sha256": _sha256(executable),
                "backend": f"poppler-pdftotext-{pdftotext_version}-bbox-layout",
                "hardware_target": "cpu",
                "unicode_version": unicodedata.unidata_version,
                "environment_profile_binding": {"ref": PLAN_REF, "sha256": plan_digest},
            },
        },
        "manual_changes": {
            "status": "none_declared",
            "change_receipts": [],
            "statement": (
                "No manual edit occurred between Poppler bbox extraction and the private "
                "raw layer; words and lines are joined only by the tracked plan's rule."
            ),
        },
        "measurements": [
            {
                "metric": "input_bytes",
                "status": "measured",
                "value": source_path.stat().st_size,
                "unit": "bytes",
                "method": "filesystem stat after exact digest verification",
                "evidence_binding": None,
            },
            {
                "metric": "output_bytes",
                "status": "measured",
                "value": total_output_bytes,
                "unit": "bytes",
                "method": "sum of exact private and tracked output entity byte counts",
                "evidence_binding": None,
            },
            {
                "metric": "human_active_seconds",
                "status": "not_applicable",
                "value": None,
                "unit": None,
                "method": "no human content review or correction was scheduled or performed",
                "evidence_binding": None,
            },
        ],
        "evidence_authentication": {
            "capture_posture": "tool_captured",
            "signature_status": "unsigned",
            "signature_bindings": [],
            "verification_status": "mechanically_verified",
            "producer_control_boundary": (
                "The same local builder executed extraction and emitted this unsigned record; "
                "independent byte checks can replay closure but cannot authenticate execution, "
                "the provider OCR process, or textual fidelity."
            ),
        },
        "rights_and_visibility": {
            "rights_record_bindings": [_binding(repo_root, plan["scope"]["rights_ref"])],
            "intended_uses": ["local_research", "preservation", "indexing"],
            "content_visibility": "local_only",
            "publication_authorized": False,
            "publication_authority_bindings": [],
        },
        "review_and_authority": {
            "mechanical_validation": "passed",
            "human_review_status": "not_performed",
            "review_bindings": [],
            "accepted_uses": [],
            "promotion_authorized": False,
            "competence_evidence_bindings": [],
        },
        "reproducibility": {
            "classification": "replay_ready",
            "known_gaps": [
                "The receipt is unsigned and self-recorded by the transformation runner.",
                "No human reviewed the embedded text or source-visible line content.",
                "The provider's embedded-text production history remains unknown.",
                "The exact source, bbox intermediate, and extracted text are absent from Git.",
            ],
            "replay_scope": (
                "Exact Antonovsky 1911 PDF digest, tracked plan and builder, pinned Poppler "
                "version and executable digest, page-region guards, private bbox/text digests, "
                "tracked record digests, and fail-closed authority posture."
            ),
        },
        "authority_boundary": {
            "validator_role": "mechanics_and_closure_only_not_truth",
            "claims_not_established": [
                "execution_truth",
                "content_truth",
                "source_fidelity",
                "translation_quality",
                "semantic_correctness",
                "rights_clearance",
                "human_review",
                "publication_authority",
                "canon_authority",
            ],
        },
    }


def _validate_schema(repo_root: Path, schema_ref: str, payload: dict[str, Any]) -> None:
    schema = json.loads((repo_root / schema_ref).read_text(encoding="utf-8"))
    errors = sorted(
        Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{'/'.join(str(part) for part in error.absolute_path) or '<root>'}: {error.message}"
            for error in errors[:12]
        )
        raise TargetTextFoundationError(f"{schema_ref} validation failed: {details}")


def _semantic_checks(
    repo_root: Path,
    anchor: dict[str, Any],
    layer: dict[str, Any],
    packet: dict[str, Any],
    event: dict[str, Any],
    text: str,
) -> None:
    sys.path.insert(0, str(repo_root / "scripts"))
    import validate_source_witness_foundation as foundation  # noqa: PLC0415

    checks = [
        ("anchor", foundation._anchor_v2_semantic_issues(anchor)),
        ("text layer", foundation._source_text_layer_semantic_issues(layer)),
        ("text units", foundation._source_text_unit_v1_issues(packet, text=text)),
        ("provenance", foundation._provenance_v2_semantic_issues(event)),
    ]
    failures = [f"{label}: {message}" for label, rows in checks for message in rows]
    if failures:
        raise TargetTextFoundationError("; ".join(failures[:20]))


def _assert_ignored(repo_root: Path, ref: str) -> None:
    result = subprocess.run(
        ["git", "check-ignore", "--quiet", "--", ref],
        cwd=repo_root,
        check=False,
    )
    if result.returncode != 0:
        raise TargetTextFoundationError(f"private output is not Git-ignored: {ref}")


def _write_private(path: Path, payload: bytes) -> None:
    if path.exists() and path.is_symlink():
        raise TargetTextFoundationError(f"private output path is symlinked: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    os.chmod(path, 0o600)


def _write_tracked(path: Path, payload: bytes) -> None:
    if path.exists() and path.is_symlink():
        raise TargetTextFoundationError(f"tracked output path is symlinked: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)


def _expected(
    repo_root: Path, local_input_root: Path
) -> tuple[dict[str, bytes], dict[str, bytes], dict[str, Any], int]:
    plan_path = repo_root / PLAN_REF
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    plan_digest = _sha256(plan_path)
    source_path = local_input_root / plan["scope"]["source_relative_ref"]
    if source_path.is_symlink() or not source_path.is_file():
        raise TargetTextFoundationError(f"exact local source is absent or symlinked: {source_path}")
    if _sha256(source_path) != plan["scope"]["file_sha256"]:
        raise TargetTextFoundationError("exact local Antonovsky source digest drifted")

    inventory = json.loads((repo_root / plan["scope"]["resource_inventory_ref"]).read_text(encoding="utf-8"))
    resources = [resource for file in inventory["files"] for resource in file["resources"]]
    page_resource = [
        resource
        for resource in resources
        if resource.get("resource_id") == plan["selector"]["page_resource_id"]
    ]
    if len(page_resource) != 1:
        raise TargetTextFoundationError("exact page resource identity is absent or ambiguous")
    locator = page_resource[0]["locator"]
    if (
        locator.get("page_index") != plan["selector"]["page_number"]
        or locator.get("width_points") != plan["selector"]["page_width_points"]
        or locator.get("height_points") != plan["selector"]["page_height_points"]
        or locator.get("rotation_degrees") != 0
    ):
        raise TargetTextFoundationError("tracked page resource geometry drifted")

    bbox, content, line_texts, _bounds, pdftotext_binary, pdftotext_version = _extract_bbox(
        source_path, plan
    )
    rights_digest = _sha256(repo_root / plan["scope"]["rights_ref"])
    anchor = _anchor_record(plan, plan_digest)
    anchor_bytes = _json_bytes(anchor)
    layer = _layer_record(plan, plan_digest, _sha256_bytes(anchor_bytes), rights_digest, content)
    packet = _unit_packet(plan, content)
    records = {
        plan["outputs"]["anchor_ref"]: anchor_bytes,
        plan["outputs"]["text_layer_ref"]: _json_bytes(layer),
        plan["outputs"]["text_unit_packet_ref"]: _json_bytes(packet),
    }
    event = _provenance_event(
        repo_root=repo_root,
        plan=plan,
        plan_digest=plan_digest,
        source_path=source_path,
        bbox=bbox,
        content=content,
        output_records=records,
        pdftotext_binary=pdftotext_binary,
        pdftotext_version=pdftotext_version,
    )
    records[plan["outputs"]["provenance_event_ref"]] = _json_bytes(event)
    _validate_schema(repo_root, ANCHOR_SCHEMA_REF, anchor)
    _validate_schema(repo_root, LAYER_SCHEMA_REF, layer)
    _validate_schema(repo_root, UNIT_SCHEMA_REF, packet)
    _validate_schema(repo_root, PROVENANCE_SCHEMA_REF, event)
    _semantic_checks(repo_root, anchor, layer, packet, event, content.decode("utf-8"))
    if any(content.decode("utf-8") in payload.decode("utf-8") for payload in records.values()):
        raise TargetTextFoundationError("tracked record exposes the complete private paragraph")
    private = {
        plan["outputs"]["private_content_ref"]: content,
        plan["outputs"]["private_bbox_ref"]: bbox,
    }
    return records, private, plan, len(line_texts)


def run(
    *,
    repo_root: Path,
    local_input_root: Path,
    local_output_root: Path,
    build: bool,
) -> dict[str, Any]:
    records, private, plan, line_count = _expected(repo_root, local_input_root)
    for ref in private:
        _assert_ignored(repo_root, ref)
    if build:
        for ref, payload in private.items():
            _write_private(local_output_root / ref, payload)
        for ref, payload in records.items():
            _write_tracked(repo_root / ref, payload)
    for ref, payload in private.items():
        path = local_output_root / ref
        if path.is_symlink() or not path.is_file() or path.read_bytes() != payload:
            raise TargetTextFoundationError(f"private output is absent or stale: {ref}")
        if stat.S_IMODE(path.stat().st_mode) != 0o600:
            raise TargetTextFoundationError(f"private output mode is not 0600: {ref}")
    for ref, payload in records.items():
        path = repo_root / ref
        if path.is_symlink() or not path.is_file() or path.read_bytes() != payload:
            raise TargetTextFoundationError(f"tracked output is absent or stale: {ref}")
    content = private[plan["outputs"]["private_content_ref"]]
    bbox = private[plan["outputs"]["private_bbox_ref"]]
    return {
        "status": "passed",
        "question": plan["question"],
        "source_sha256": plan["scope"]["file_sha256"],
        "bbox_sha256": _sha256_bytes(bbox),
        "bbox_bytes": len(bbox),
        "private_content_sha256": _sha256_bytes(content),
        "private_content_bytes": len(content),
        "private_content_codepoints": len(content.decode("utf-8")),
        "source_observed_print_lines": line_count,
        "source_observed_line_breaks": line_count - 1,
        "tracked_record_count": len(records),
        "accepted_russian": False,
        "accepted_translation": False,
        "human_review_performed": False,
        "alignment_created": False,
        "semantic_or_canon_effect": False,
        "publication_authorized": False,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--local-input-root", type=Path, required=True)
    parser.add_argument("--local-output-root", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    try:
        result = run(
            repo_root=args.repo_root.resolve(),
            local_input_root=args.local_input_root.resolve(),
            local_output_root=args.local_output_root.resolve(),
            build=args.build,
        )
    except (OSError, KeyError, TypeError, ValueError, TargetTextFoundationError) as exc:
        print(f"[zarathustra-target-text-foundation] {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Materialize one real, non-promoting Zarathustra source-text foundation.

The tracked records contain identity, selectors, digests, ranges, provenance,
rights posture, and authority limits only. The extracted DTA paragraph remains
mode-0600 below the already ignored owner-local ``local-content`` boundary.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
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
    "za-i-vorrede-1-source-text-foundation.plan.v1.json"
)
ANCHOR_SCHEMA_REF = "ToS/contracts/source-anchor-v2.schema.json"
LAYER_SCHEMA_REF = "ToS/contracts/source-text-layer.schema.json"
UNIT_SCHEMA_REF = "ToS/contracts/source-text-unit-packet-v1.schema.json"
PROVENANCE_SCHEMA_REF = "ToS/contracts/provenance-event-v2.schema.json"
BUILDER_REF = "scripts/build_zarathustra_source_text_foundation.py"
EVENT_ID = "tos.event.native-extraction.za-i-vorrede-1.dta-part-1-p1.2026-08-12"
EXPECTED_OPENING_SENTENCE_SHA256 = (
    "a507f7293ac73ec37f7c78a38b3270f45bc98418b8fd824f2ab72ebc18377224"
)


class SourceTextFoundationError(RuntimeError):
    """Raised when the exact source boundary or a fail-closed gate drifts."""


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _json_bytes(payload: Any) -> bytes:
    return (json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode(
        "utf-8"
    )


def _canonical_value_sha256(payload: Any) -> str:
    return _sha256_bytes(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    )


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _direct_children(element: ET.Element, name: str) -> list[ET.Element]:
    return [child for child in list(element) if _local_name(child.tag) == name]


def _select_exact_paragraph(source_path: Path) -> ET.Element:
    try:
        root = ET.fromstring(source_path.read_bytes())
    except (OSError, ET.ParseError) as exc:
        raise SourceTextFoundationError(f"cannot parse exact TEI source: {exc}") from exc
    if _local_name(root.tag) != "TEI":
        raise SourceTextFoundationError("source root is not TEI")
    texts = _direct_children(root, "text")
    if len(texts) != 1:
        raise SourceTextFoundationError(f"expected one direct text, observed {len(texts)}")
    bodies = _direct_children(texts[0], "body")
    if len(bodies) != 1:
        raise SourceTextFoundationError(f"expected one direct body, observed {len(bodies)}")
    outer_divs = _direct_children(bodies[0], "div")
    if not outer_divs:
        raise SourceTextFoundationError("TEI body has no direct div")
    inner_divs = _direct_children(outer_divs[0], "div")
    if not inner_divs:
        raise SourceTextFoundationError("first TEI body div has no direct div")
    paragraphs = _direct_children(inner_divs[0], "p")
    if not paragraphs:
        raise SourceTextFoundationError("selected TEI div has no direct paragraph")
    return paragraphs[0]


def _render_lb_aware_paragraph(paragraph: ET.Element) -> str:
    chunks = [paragraph.text or ""]
    for child in list(paragraph):
        name = _local_name(child.tag)
        if name != "lb":
            raise SourceTextFoundationError(f"unexpected child in bounded paragraph: {name}")
        if child.text not in {None, ""} or list(child):
            raise SourceTextFoundationError("bounded lb is not an empty TEI element")
        chunks.append("\n")
        tail = child.tail or ""
        if not tail.startswith("\n"):
            raise SourceTextFoundationError(
                "bounded lb lacks the one declared XML-formatting line feed"
            )
        chunks.append(tail[1:])
    rendered = "".join(chunks)
    if "\r" in rendered or rendered.endswith("\n"):
        raise SourceTextFoundationError("rendered paragraph violates line-ending policy")
    if len(rendered.split("\n")) != 7:
        raise SourceTextFoundationError("expected seven source-observed print lines")
    opening = rendered.split(".", 1)[0].replace("\n", " ") + "."
    if _sha256_bytes(opening.encode("utf-8")) != EXPECTED_OPENING_SENTENCE_SHA256:
        raise SourceTextFoundationError("opening-sentence calibration digest drifted")
    return rendered


def _binding(repo_root: Path, ref: str) -> dict[str, str]:
    return {"ref": ref, "sha256": _sha256(repo_root / ref)}


def _anchor_record(plan: dict[str, Any], plan_digest: str) -> dict[str, Any]:
    scope = plan["scope"]
    ids = plan["opaque_ids"]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-anchor-v2.schema.json",
        "schema_version": "tos_source_anchor_v2",
        "anchor_id": ids["anchor_id"],
        "passage_id": ids["passage_id"],
        "target": {
            "item_id": scope["item_ref"],
            "file_id": scope["file_ref"],
            "file_sha256": scope["file_sha256"],
            "media_type": "application/xml",
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
                        "media_type": "application/xml",
                        "character_normalization": "none",
                    },
                    "selector": {
                        "type": "structural",
                        "scheme": plan["selector"]["scheme"],
                        "value": plan["selector"]["value"],
                        "conforms_to": "https://www.w3.org/TR/1999/REC-xpath-19991116/",
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
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json",
        "schema_version": "tos_source_text_layer_v1",
        "layer_id": ids["layer_id"],
        "layer_role": "machine_transcription",
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
            "language": "de",
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
            "rights_record_refs": [
                {"ref": scope["rights_ref"], "sha256": rights_digest}
            ],
            "publication_authority_refs": [],
        },
        "derivation": {
            "method": "structural_extraction",
            "input_layers": [],
            "maker": {
                "maker_type": "software",
                "agent_ref": "software:tos-zarathustra-source-text-foundation-builder",
                "method": plan["extraction_policy"]["method"],
                "version": plan["extraction_policy"]["method_version"],
                "configuration_ref": PLAN_REF,
                "configuration_digest": plan_digest,
            },
            "preservation_goal": "source_near",
            "loss_posture": "preservation_intended",
            "silent_changes_allowed": False,
            "change_payload": {"kind": "none"},
        },
        "editorial_policy": {
            "policy_ref": PLAN_REF,
            "policy_sha256": plan_digest,
            "transcription_goal": "machine_candidate",
            "historical_language_preserved": True,
            "printing_errors_silently_corrected": False,
            "typography_posture": "preserve",
            "layout_posture": "encode_explicitly",
            "unicode_normalization": "none",
            "uncertainty_representation": "explicit-never-silent",
            "method_declared": True,
        },
        "uncertainty": {"status": "none", "annotations": []},
        "admission": {
            "mechanical_status": "fixity_verified",
            "review_status": "unreviewed",
            "review_ref": None,
            "human_review_performed": False,
            "human_language_competence": "blocked",
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


def _unit_packet(
    plan: dict[str, Any], content: bytes, layer_record_ref: str
) -> dict[str, Any]:
    scope = plan["scope"]
    ids = plan["opaque_ids"]
    text = content.decode("utf-8")
    content_digest = _sha256_bytes(content)
    anchor_ids = ids["anchor_ids"]
    unit_ids = ids["unit_ids"]
    segments: list[tuple[str, int, int]] = []
    cursor = 0
    for index, line in enumerate(text.split("\n")):
        end = cursor + len(line)
        segments.append(("physical_line", cursor, end))
        cursor = end
        if index < 6:
            segments.append(("whitespace", cursor, cursor + 1))
            cursor += 1
    if cursor != len(text) or len(segments) != 13:
        raise SourceTextFoundationError("source-layout coverage construction drifted")

    anchors = [
        {
            "anchor_ref": anchor_ids[0],
            "anchor_role": "scope",
            "ordinal": 1,
            "text_layer_ref": layer_record_ref,
            "text_layer_sha256": content_digest,
            "selector": {
                "type": "text_position",
                "start": 0,
                "end": len(text),
                "position_unit": "unicode_code_point",
                "interval": "half_open",
            },
            "exact_sha256": _sha256_bytes(content),
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
                "text_layer_ref": layer_record_ref,
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
                    "The unit records only one exact TEI lb-delimited print line or its "
                    "line-break code point; it is not accepted German or linguistic analysis."
                ),
            }
        )

    made_at = plan["created_at"]
    method = {
        "maker_kind": "software",
        "agent_ref": "software:tos-zarathustra-source-text-foundation-builder",
        "method_name": "exact TEI lb-delimited source-layout observation",
        "method_version": "1",
        "software_refs": [BUILDER_REF],
        "model_ref": None,
        "configuration_ref": PLAN_REF,
        "locale": "de",
        "unicode_version": unicodedata.unidata_version,
        "unicode_revision": None,
        "tailoring_ref": None,
        "provenance_event_ref": EVENT_ID,
        "made_at": made_at,
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
            "text_layer_ref": layer_record_ref,
            "text_layer_sha256": content_digest,
            "language": "de",
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
                "scheme_name": "DTA TEI lb-delimited source-layout observation",
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
                    "Only the exact TEI lb-delimited layout of one provider-transcription "
                    "paragraph is observed; no word or sentence boundary is asserted."
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
    source_digest: str,
    content: bytes,
    output_records: dict[str, bytes],
) -> dict[str, Any]:
    private_ref = plan["outputs"]["private_content_ref"]
    builder_path = repo_root / BUILDER_REF
    builder_digest = _sha256(builder_path)
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
    outputs = [
        {
            "entity_ref": private_ref,
            "role": "ignored-local-machine-transcription",
            "sha256": _sha256_bytes(content),
            "size_bytes": len(content),
            "media_type": "text/plain; charset=utf-8",
            "availability": "ignored_local",
            "content_disclosure": "private_content",
            "fixity_verified": True,
            "fixity_verified_at": plan["created_at"],
        }
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
    derivations = []
    for ordinal, output in enumerate(outputs, start=1):
        derivations.append(
            {
                "derivation_id": f"tos.derivation.za-i-vorrede-1.source-text-foundation.{ordinal}",
                "input_entity_ref": plan["scope"]["source_relative_ref"],
                "output_entity_ref": output["entity_ref"],
                "relation": "selection_from",
                "influence_asserted": True,
                "description": (
                    "The exact fixity-bound TEI source and the tracked plan determine this "
                    "bounded text, address, lineage, or layout record."
                ),
            }
        )
    total_output_bytes = sum(output["size_bytes"] for output in outputs)
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json"
        ),
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
                "The extracted layer is unreviewed provider transcription, not accepted German.",
                "The source and derived text remain local-only and publication is unauthorized.",
                "The unsigned self-recorded event proves neither execution truth nor content truth.",
            ],
        },
        "entities": {
            "inputs": [
                {
                    "entity_ref": plan["scope"]["source_relative_ref"],
                    "role": "fixity-verified-local-dta-tei-source",
                    "sha256": source_digest,
                    "size_bytes": source_path.stat().st_size,
                    "media_type": "application/xml",
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
                "agent_ref": "software:tos-zarathustra-source-text-foundation-builder",
                "agent_kind": "software",
                "role": "executor",
                "responsibility_posture": "performed",
                "evidence_binding": {"ref": BUILDER_REF, "sha256": builder_digest},
                "human_evidence_status": "not_applicable",
            }
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
                    "name": "Tree of Sophia Zarathustra source-text foundation builder",
                    "version": "1",
                    "role": "native-extraction-and-record-builder",
                    "artifact_ref": BUILDER_REF,
                    "artifact_sha256": builder_digest,
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
                "backend": "python-standard-library-elementtree",
                "hardware_target": "cpu",
                "unicode_version": unicodedata.unidata_version,
                "environment_profile_binding": {"ref": PLAN_REF, "sha256": plan_digest},
            },
        },
        "manual_changes": {
            "status": "none_declared",
            "change_receipts": [],
            "statement": (
                "No manual edit occurred between the exact TEI parse and the private output; "
                "the declared lb rule is executed by the tracked builder."
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
                "method": "sum of exact serialized output entity byte counts",
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
                "The same local builder executed the extraction and emitted this unsigned "
                "record; independent byte checks can replay closure but cannot authenticate "
                "execution or philological truth."
            ),
        },
        "rights_and_visibility": {
            "rights_record_bindings": [
                _binding(repo_root, plan["scope"]["rights_ref"])
            ],
            "intended_uses": ["local_research", "preservation", "indexing"],
            "content_visibility": "local_only",
            "publication_authorized": False,
            "publication_authority_bindings": [],
        },
        "review_and_authority": {
            "mechanical_validation": "passed",
            "human_review_status": "blocked",
            "review_bindings": [],
            "accepted_uses": [],
            "promotion_authorized": False,
            "competence_evidence_bindings": [],
        },
        "reproducibility": {
            "classification": "replay_ready",
            "known_gaps": [
                "The receipt is unsigned and self-recorded by the transformation runner.",
                "No German-competent human reviewed the extracted content or boundaries.",
                "The exact source and extracted text are intentionally absent from Git.",
            ],
            "replay_scope": (
                "Exact DTA TEI digest, tracked plan and builder, bounded structural selector, "
                "declared lb rule, private output digest, tracked record digests, CPython "
                "runtime digest, and fail-closed authority posture."
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
        raise SourceTextFoundationError(f"{schema_ref} validation failed: {details}")


def _semantic_checks(
    anchor: dict[str, Any],
    layer: dict[str, Any],
    packet: dict[str, Any],
    event: dict[str, Any],
    text: str,
) -> None:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import validate_source_witness_foundation as foundation  # noqa: PLC0415

    checks = [
        ("anchor", foundation._anchor_v2_semantic_issues(anchor)),
        ("text layer", foundation._source_text_layer_semantic_issues(layer)),
        ("text units", foundation._source_text_unit_v1_issues(packet, text=text)),
        ("provenance", foundation._provenance_v2_semantic_issues(event)),
    ]
    failures = [f"{label}: {message}" for label, rows in checks for message in rows]
    if failures:
        raise SourceTextFoundationError("; ".join(failures[:20]))


def _assert_ignored(repo_root: Path, ref: str) -> None:
    result = subprocess.run(
        ["git", "check-ignore", "--quiet", "--", ref],
        cwd=repo_root,
        check=False,
    )
    if result.returncode != 0:
        raise SourceTextFoundationError(f"private output is not Git-ignored: {ref}")


def _write_private(path: Path, payload: bytes) -> None:
    if path.exists() and path.is_symlink():
        raise SourceTextFoundationError(f"private output path is symlinked: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    os.chmod(path, 0o600)


def _write_tracked(path: Path, payload: bytes) -> None:
    if path.exists() and path.is_symlink():
        raise SourceTextFoundationError(f"tracked output path is symlinked: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)


def _expected(
    repo_root: Path, local_input_root: Path
) -> tuple[dict[str, bytes], str, dict[str, Any]]:
    plan_path = repo_root / PLAN_REF
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    plan_digest = _sha256(plan_path)
    source_path = local_input_root / plan["scope"]["source_relative_ref"]
    if source_path.is_symlink() or not source_path.is_file():
        raise SourceTextFoundationError(f"exact local source is absent or symlinked: {source_path}")
    source_digest = _sha256(source_path)
    if source_digest != plan["scope"]["file_sha256"]:
        raise SourceTextFoundationError("exact local DTA source digest drifted")
    paragraph = _select_exact_paragraph(source_path)
    text = _render_lb_aware_paragraph(paragraph)
    content = text.encode("utf-8")
    rights_digest = _sha256(repo_root / plan["scope"]["rights_ref"])

    anchor = _anchor_record(plan, plan_digest)
    anchor_bytes = _json_bytes(anchor)
    layer = _layer_record(
        plan,
        plan_digest,
        _sha256_bytes(anchor_bytes),
        rights_digest,
        content,
    )
    layer_bytes = _json_bytes(layer)
    packet = _unit_packet(plan, content, plan["outputs"]["text_layer_ref"])
    packet_bytes = _json_bytes(packet)
    records = {
        plan["outputs"]["anchor_ref"]: anchor_bytes,
        plan["outputs"]["text_layer_ref"]: layer_bytes,
        plan["outputs"]["text_unit_packet_ref"]: packet_bytes,
    }
    event = _provenance_event(
        repo_root=repo_root,
        plan=plan,
        plan_digest=plan_digest,
        source_path=source_path,
        source_digest=source_digest,
        content=content,
        output_records=records,
    )
    records[plan["outputs"]["provenance_event_ref"]] = _json_bytes(event)

    _validate_schema(repo_root, ANCHOR_SCHEMA_REF, anchor)
    _validate_schema(repo_root, LAYER_SCHEMA_REF, layer)
    _validate_schema(repo_root, UNIT_SCHEMA_REF, packet)
    _validate_schema(repo_root, PROVENANCE_SCHEMA_REF, event)
    _semantic_checks(anchor, layer, packet, event, text)
    if any(text in payload.decode("utf-8") for payload in records.values()):
        raise SourceTextFoundationError("tracked record exposes the complete private paragraph")
    return records, text, plan


def run(
    *,
    repo_root: Path,
    local_input_root: Path,
    local_output_root: Path,
    build: bool,
) -> dict[str, Any]:
    records, text, plan = _expected(repo_root, local_input_root)
    private_ref = plan["outputs"]["private_content_ref"]
    _assert_ignored(repo_root, private_ref)
    private_path = local_output_root / private_ref
    content = text.encode("utf-8")
    if build:
        _write_private(private_path, content)
        for ref, payload in records.items():
            _write_tracked(repo_root / ref, payload)
    if not private_path.is_file() or private_path.is_symlink():
        raise SourceTextFoundationError("private materialized content is absent or symlinked")
    if private_path.read_bytes() != content:
        raise SourceTextFoundationError("private materialized content differs from exact replay")
    if stat.S_IMODE(private_path.stat().st_mode) != 0o600:
        raise SourceTextFoundationError("private materialized content mode is not 0600")
    for ref, payload in records.items():
        path = repo_root / ref
        if not path.is_file() or path.is_symlink() or path.read_bytes() != payload:
            raise SourceTextFoundationError(f"tracked output is absent or stale: {ref}")
    return {
        "status": "passed",
        "question": plan["question"],
        "source_sha256": plan["scope"]["file_sha256"],
        "private_content_sha256": _sha256_bytes(content),
        "private_content_bytes": len(content),
        "private_content_codepoints": len(text),
        "source_observed_print_lines": 7,
        "source_observed_line_breaks": 6,
        "tracked_record_count": len(records),
        "accepted_german": False,
        "accepted_translation_input": False,
        "human_review_performed": False,
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
        report = run(
            repo_root=args.repo_root.resolve(),
            local_input_root=args.local_input_root.resolve(),
            local_output_root=args.local_output_root.resolve(),
            build=args.build,
        )
    except SourceTextFoundationError as exc:
        print(f"source-text foundation: FAIL: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

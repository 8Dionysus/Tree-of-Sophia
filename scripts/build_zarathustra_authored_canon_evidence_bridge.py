#!/usr/bin/env python3
"""Bridge the authored Zarathustra route to the exact source evidence spine.

The builder preserves the pre-existing authored canon unchanged. It creates a
private exact DTA section layer, a separate declared comparison-normalized
layer, two competing text-unit segmentations, and a text-free reconciliation
record. Mechanical agreement never becomes source, language, translation,
semantic, review, graph, canon, publication, or transfer authority.
"""

from __future__ import annotations

import argparse
import csv
import difflib
import hashlib
import io
import json
import os
import platform
import re
import stat
import subprocess
import sys
import unicodedata
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = (
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1"
)
PLAN_REF = f"{GOLD_ROOT}/authored-canon-evidence-bridge.plan.v1.json"
BUILDER_REF = "scripts/build_zarathustra_authored_canon_evidence_bridge.py"
ANCHOR_SCHEMA_REF = "ToS/contracts/source-anchor-v2.schema.json"
LAYER_SCHEMA_REF = "ToS/contracts/source-text-layer.schema.json"
UNIT_SCHEMA_REF = "ToS/contracts/source-text-unit-packet-v1.schema.json"
BRIDGE_SCHEMA_REF = "ToS/contracts/authored-route-evidence-bridge-v1.schema.json"
PROVENANCE_SCHEMA_REF = "ToS/contracts/provenance-event-v2.schema.json"
EVENT_ID = "tos.event.reconciliation.zarathustra-authored-canon-evidence-bridge.2026-08-12"
LAYER_AUTHORITY_BOUNDARY = (
    "a source text layer is one immutable, source-returnable representation with "
    "explicit derivation, uncertainty, review, competence, rights, and use scope; "
    "mechanical validation, model output, normalization, or agreement with another "
    "layer does not make it accepted source text, translation evidence, linguistic "
    "truth, semantic evidence, graph truth, canon authority, or publication permission"
)
BRIDGE_AUTHORITY_BOUNDARY = (
    "the bridge proves exact representation and inventory closure only; it preserves "
    "the authored route without converting its witness roles, review notes, nodes, or "
    "relations into accepted German, accepted translation, semantic truth, modern human "
    "attestation, claim-evidence closure, graph admission, canon revision, publication "
    "permission, or server-transfer authority"
)


class EvidenceBridgeError(RuntimeError):
    """Raised when source identity, crosswalk, or authority closure drifts."""


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _json_bytes(payload: Any) -> bytes:
    return (json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode(
        "utf-8"
    )


def _canonical_bytes(payload: Any) -> bytes:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode(
        "utf-8"
    )


def _canonical_sha256(payload: Any) -> str:
    return _sha256_bytes(_canonical_bytes(payload))


def _binding(repo_root: Path, ref: str) -> dict[str, str]:
    return {"ref": ref, "sha256": _sha256(repo_root / ref)}


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _direct_children(element: ET.Element, name: str) -> list[ET.Element]:
    return [child for child in list(element) if _local_name(child.tag) == name]


def _select_section(source_path: Path) -> tuple[ET.Element, list[ET.Element]]:
    try:
        root = ET.fromstring(source_path.read_bytes())
    except (OSError, ET.ParseError) as exc:
        raise EvidenceBridgeError(f"cannot parse exact TEI source: {exc}") from exc
    if _local_name(root.tag) != "TEI":
        raise EvidenceBridgeError("source root is not TEI")
    texts = _direct_children(root, "text")
    bodies = _direct_children(texts[0], "body") if len(texts) == 1 else []
    outer = _direct_children(bodies[0], "div") if len(bodies) == 1 else []
    inner = _direct_children(outer[0], "div") if outer else []
    if not inner:
        raise EvidenceBridgeError("exact first inner TEI div is absent")
    paragraphs = _direct_children(inner[0], "p")
    if len(paragraphs) != 12:
        raise EvidenceBridgeError(
            f"expected twelve direct source paragraphs, observed {len(paragraphs)}"
        )
    return inner[0], paragraphs


def _render_inline(element: ET.Element) -> str:
    chunks = [element.text or ""]
    for child in list(element):
        name = _local_name(child.tag)
        if name == "lb":
            if child.text not in {None, ""} or list(child):
                raise EvidenceBridgeError("TEI lb is not empty")
            chunks.append("\n")
            tail = child.tail or ""
            if not tail.startswith("\n"):
                raise EvidenceBridgeError("TEI lb lacks declared XML-formatting line feed")
            chunks.append(tail[1:])
        elif name == "hi":
            chunks.append(_render_inline(child))
            chunks.append(child.tail or "")
        else:
            raise EvidenceBridgeError(f"unexpected inline element in source paragraph: {name}")
    return "".join(chunks)


def _normalize_source(value: str) -> str:
    value = re.sub("\u00ac\\n", "", value)
    return re.sub(r"\s+", " ", value).strip()


def _normalize_authored(value: str) -> str:
    value = value.replace("*", "")
    value = re.sub(r"-\n\s*", "", value)
    return re.sub(r"\s+", " ", value).strip()


def _spans(parts: list[str], complete: str) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
    if " ".join(parts) != complete:
        raise EvidenceBridgeError("normalized parts do not reconstruct the exact complete layer")
    spans: list[tuple[int, int]] = []
    gaps: list[tuple[int, int]] = []
    cursor = 0
    for index, part in enumerate(parts):
        end = cursor + len(part)
        if complete[cursor:end] != part:
            raise EvidenceBridgeError("normalized part position drifted")
        spans.append((cursor, end))
        cursor = end
        if index < len(parts) - 1:
            if complete[cursor : cursor + 1] != " ":
                raise EvidenceBridgeError("expected one normalized inter-unit space")
            gaps.append((cursor, cursor + 1))
            cursor += 1
    if cursor != len(complete):
        raise EvidenceBridgeError("normalized coverage does not end at layer boundary")
    return spans, gaps


def _normalization_operations(raw: str, normalized: str) -> dict[str, Any]:
    opcodes = []
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, raw, normalized).get_opcodes():
        if tag == "equal":
            continue
        input_value = raw[i1:i2]
        output_value = normalized[j1:j2]
        opcodes.append(
            {
                "operation": tag,
                "input_span": [i1, i2],
                "output_span": [j1, j2],
                "input_exact": input_value,
                "input_sha256": _sha256_bytes(input_value.encode("utf-8")),
                "output_exact": output_value,
                "output_sha256": _sha256_bytes(output_value.encode("utf-8")),
            }
        )
    return {
        "schema_version": "tos_private_editorial_normalization_operations_v1",
        "input_sha256": _sha256_bytes(raw.encode("utf-8")),
        "output_sha256": _sha256_bytes(normalized.encode("utf-8")),
        "operation_count": len(opcodes),
        "declared_steps": [
            "remove U+00AC plus following line feed",
            "collapse remaining Unicode whitespace runs to U+0020",
            "trim leading and trailing whitespace",
        ],
        "discretionary_break_count": raw.count("\u00ac\n"),
        "operations": opcodes,
    }


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
                        "scheme": plan["source_selector"]["scheme"],
                        "value": plan["source_selector"]["value"],
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
            "method": "exact-tei-section-selection",
            "version": "1",
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


def _common_layer(
    plan: dict[str, Any],
    *,
    layer_id: str,
    layer_role: str,
    content_ref: str,
    content: bytes,
    anchor_digest: str,
    rights_digest: str,
    plan_digest: str,
    derivation: dict[str, Any],
    character_normalization: str,
    line_break_posture: str,
    transcription_goal: str,
    layout_posture: str,
) -> dict[str, Any]:
    scope = plan["scope"]
    text = content.decode("utf-8")
    content_digest = _sha256_bytes(content)
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-layer.schema.json",
        "schema_version": "tos_source_text_layer_v1",
        "layer_id": layer_id,
        "layer_role": layer_role,
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
                    "anchor_id": plan["opaque_ids"]["anchor_id"],
                    "anchor_record_ref": plan["outputs"]["anchor_ref"],
                    "anchor_record_sha256": anchor_digest,
                }
            ],
        },
        "representation": {
            "content_file_id": f"tos.file.sha256.{content_digest}",
            "content_ref": content_ref,
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
            "character_normalization": character_normalization,
            "line_break_posture": line_break_posture,
            "storage": "ignored_local",
            "content_visibility": "local_only",
            "tracked_content": False,
            "publication_authorized": False,
            "rights_record_refs": [
                {"ref": scope["rights_ref"], "sha256": rights_digest}
            ],
            "publication_authority_refs": [],
        },
        "derivation": derivation,
        "editorial_policy": {
            "policy_ref": PLAN_REF,
            "policy_sha256": plan_digest,
            "transcription_goal": transcription_goal,
            "historical_language_preserved": True,
            "printing_errors_silently_corrected": False,
            "typography_posture": (
                "preserve" if character_normalization == "none" else "normalize_declared"
            ),
            "layout_posture": layout_posture,
            "unicode_normalization": character_normalization,
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
        "authority_boundary": LAYER_AUTHORITY_BOUNDARY,
        "layer_version": 1,
        "supersedes_layer_ref": None,
    }


def _unit_anchor(
    *,
    anchor_ref: str,
    ordinal: int,
    layer_ref: str,
    layer_digest: str,
    content: str,
    start: int,
    end: int,
    role: str,
    locator_ref: str,
) -> dict[str, Any]:
    return {
        "anchor_ref": anchor_ref,
        "ordinal": ordinal,
        "text_layer_ref": layer_ref,
        "text_layer_sha256": layer_digest,
        "selector": {
            "type": "text_position",
            "start": start,
            "end": end,
            "position_unit": "unicode_code_point",
            "interval": "half_open",
        },
        "exact_sha256": _sha256_bytes(content[start:end].encode("utf-8")),
        "anchor_role": role,
        "source_return": {"required": True, "locator_ref": locator_ref},
    }


def _unit(
    unit_id: str,
    anchor_ref: str,
    *,
    kind: str,
    boundary_posture: str,
    status_reason: str,
) -> dict[str, Any]:
    return {
        "unit_id": unit_id,
        "unit_version": 1,
        "supersedes_unit_ref": None,
        "identity_policy": "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis",
        "unit_kind": kind,
        "surface_posture": "source_bearing",
        "continuity": "contiguous",
        "ordered_anchor_refs": [anchor_ref],
        "parent_unit_refs": [],
        "ordered_child_unit_refs": [],
        "boundary_posture": boundary_posture,
        "certainty": {
            "value": 1.0,
            "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        },
        "status_reason": status_reason,
        "source_text_mutated": False,
        "semantic_promotion": False,
    }


def _unit_packet(
    plan: dict[str, Any],
    *,
    normalized: str,
    source_parts: list[str],
    authored_parts: list[str],
) -> dict[str, Any]:
    ids = plan["opaque_ids"]
    scope = plan["scope"]
    layer_ref = plan["outputs"]["normalized_layer_ref"]
    layer_content_digest = _sha256_bytes(normalized.encode("utf-8"))
    source_spans, source_gaps = _spans(source_parts, normalized)
    authored_spans, authored_gaps = _spans(authored_parts, normalized)
    locator = plan["outputs"]["private_normalized_content_ref"]
    anchors = [
        _unit_anchor(
            anchor_ref=ids["scope_anchor_id"],
            ordinal=1,
            layer_ref=layer_ref,
            layer_digest=layer_content_digest,
            content=normalized,
            start=0,
            end=len(normalized),
            role="scope",
            locator_ref=locator,
        )
    ]
    ordinal = 2
    for anchor_ref, (start, end) in zip(ids["source_anchor_ids"], source_spans, strict=True):
        anchors.append(
            _unit_anchor(
                anchor_ref=anchor_ref,
                ordinal=ordinal,
                layer_ref=layer_ref,
                layer_digest=layer_content_digest,
                content=normalized,
                start=start,
                end=end,
                role="content",
                locator_ref=locator,
            )
        )
        ordinal += 1
    for anchor_ref, (start, end) in zip(ids["source_gap_anchor_ids"], source_gaps, strict=True):
        anchors.append(
            _unit_anchor(
                anchor_ref=anchor_ref,
                ordinal=ordinal,
                layer_ref=layer_ref,
                layer_digest=layer_content_digest,
                content=normalized,
                start=start,
                end=end,
                role="gap",
                locator_ref=locator,
            )
        )
        ordinal += 1
    for anchor_ref, (start, end) in zip(ids["authored_anchor_ids"], authored_spans, strict=True):
        anchors.append(
            _unit_anchor(
                anchor_ref=anchor_ref,
                ordinal=ordinal,
                layer_ref=layer_ref,
                layer_digest=layer_content_digest,
                content=normalized,
                start=start,
                end=end,
                role="content",
                locator_ref=locator,
            )
        )
        ordinal += 1
    for anchor_ref, (start, end) in zip(ids["authored_gap_anchor_ids"], authored_gaps, strict=True):
        anchors.append(
            _unit_anchor(
                anchor_ref=anchor_ref,
                ordinal=ordinal,
                layer_ref=layer_ref,
                layer_digest=layer_content_digest,
                content=normalized,
                start=start,
                end=end,
                role="gap",
                locator_ref=locator,
            )
        )
        ordinal += 1

    source_units = [
        _unit(
            unit_id,
            anchor_ref,
            kind="paragraph",
            boundary_posture="source_attested",
            status_reason=(
                "The boundary derives from one exact direct TEI p element after a separate "
                "declared comparison normalization; it is not accepted German or semantics."
            ),
        )
        for unit_id, anchor_ref in zip(
            ids["source_unit_ids"], ids["source_anchor_ids"], strict=True
        )
    ]
    authored_units = [
        _unit(
            unit_id,
            anchor_ref,
            kind="other",
            boundary_posture="method_proposed",
            status_reason=(
                "The boundary is imported from the pre-existing authored twelve-segment route "
                "and mechanically crosswalked only; older authored/review posture is not a "
                "modern source, linguistic, translation, semantic, or human-review decision."
            ),
        )
        for unit_id, anchor_ref in zip(
            ids["authored_unit_ids"], ids["authored_anchor_ids"], strict=True
        )
    ]
    common_method = {
        "agent_ref": "software:tos-zarathustra-authored-canon-evidence-bridge-builder",
        "method_version": "1",
        "software_refs": [BUILDER_REF],
        "model_ref": None,
        "configuration_ref": PLAN_REF,
        "locale": "de",
        "unicode_version": unicodedata.unidata_version,
        "unicode_revision": None,
        "tailoring_ref": None,
        "provenance_event_ref": EVENT_ID,
        "made_at": plan["created_at"],
        "output_posture": "method_result_not_source_or_linguistic_truth",
    }
    source_method = {
        **common_method,
        "maker_kind": "software",
        "method_name": "exact TEI paragraph boundaries on declared normalized layer",
    }
    authored_method = {
        **common_method,
        "maker_kind": "import",
        "agent_ref": "record:tos-source-thus-spoke-zarathustra-prologue",
        "method_name": "digest-bound import of legacy authored segment boundaries",
    }
    source_policy = {
        "normalization": "no-text-mutation-separate-successor-layer",
        "punctuation": "included_in_neighbor",
        "whitespace": "declared_excluded",
        "line_break": "not_applicable",
        "hyphenation": "separate_successor_layer",
        "unreported_gaps_allowed": False,
        "overlap": "forbid",
    }
    authority_limit = {
        "algorithmic_output_is_source_truth": False,
        "algorithmic_output_is_linguistic_truth": False,
        "model_subword_is_lexeme": False,
        "unit_identity_is_semantic_identity": False,
        "text_mutation_allowed": False,
    }
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-text-unit-packet-v1.schema.json",
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
            "text_layer_ref": layer_ref,
            "text_layer_sha256": layer_content_digest,
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
                "scheme_id": ids["source_scheme_id"],
                "scheme_version": 1,
                "supersedes_scheme_ref": None,
                "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
                "scheme_name": "DTA source paragraph structure on comparison layer",
                "analysis_role": "source_structure",
                "boundary_basis": "source_markup",
                "unit_kinds": ["paragraph"],
                "method": source_method,
                "policies": source_policy,
                "authority_limit": authority_limit,
            },
            {
                "scheme_id": ids["authored_scheme_id"],
                "scheme_version": 1,
                "supersedes_scheme_ref": None,
                "identity_policy": "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis",
                "scheme_name": "pre-existing authored prologue route segment crosswalk",
                "analysis_role": "interchange",
                "boundary_basis": "imported",
                "unit_kinds": ["other"],
                "method": authored_method,
                "policies": source_policy,
                "authority_limit": authority_limit,
            },
        ],
        "anchors": anchors,
        "units": source_units + authored_units,
        "segmentations": [
            {
                "segmentation_id": ids["source_segmentation_id"],
                "segmentation_version": 1,
                "supersedes_segmentation_ref": None,
                "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
                "scheme_ref": ids["source_scheme_id"],
                "ordered_unit_refs": ids["source_unit_ids"],
                "coverage": {
                    "scope_anchor_ref": ids["scope_anchor_id"],
                    "coverage_posture": "declared_partial",
                    "excluded_anchor_refs": ids["source_gap_anchor_ids"],
                    "unreported_gaps_allowed": False,
                    "overlap_requires_declaration": True,
                    "source_reconstruction_required": True,
                },
                "status": "observed_source_structure",
                "status_reason": "Twelve exact TEI paragraph boundaries are mechanically observed on the separate normalized layer; excluded spaces represent declared inter-paragraph separators.",
                "maker": source_method,
                "competing_segmentation_refs": [ids["authored_segmentation_id"]],
                "review_refs": [],
                "declared_uses": ["navigation", "source_observation"],
                "source_text_authority": False,
                "linguistic_authority": False,
                "semantic_authority": False,
            },
            {
                "segmentation_id": ids["authored_segmentation_id"],
                "segmentation_version": 1,
                "supersedes_segmentation_ref": None,
                "identity_policy": "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries",
                "scheme_ref": ids["authored_scheme_id"],
                "ordered_unit_refs": ids["authored_unit_ids"],
                "coverage": {
                    "scope_anchor_ref": ids["scope_anchor_id"],
                    "coverage_posture": "declared_partial",
                    "excluded_anchor_refs": ids["authored_gap_anchor_ids"],
                    "unreported_gaps_allowed": False,
                    "overlap_requires_declaration": True,
                    "source_reconstruction_required": True,
                },
                "status": "proposed",
                "status_reason": "The twelve authored boundaries match the complete normalized source sequence but retain legacy authored rather than modern source, language, translation, semantic, or review assurance.",
                "maker": authored_method,
                "competing_segmentation_refs": [ids["source_segmentation_id"]],
                "review_refs": [],
                "declared_uses": ["navigation", "interchange"],
                "source_text_authority": False,
                "linguistic_authority": False,
                "semantic_authority": False,
            },
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
            "tree_role": "orientation",
            "graph_role": "relation",
            "source_role": "authority",
            "validators_prove_mechanics_not_truth": True,
            "segmentation_is_method_result_not_source_truth": True,
            "source_unit_is_not_lexeme_sign_or_concept": True,
            "model_token_is_not_linguistic_token": True,
            "projection_is_owner_truth": False,
            "legacy_bulk_migration_authorized": False,
        },
    }


def _route_nodes(repo_root: Path, source_node_ref: str) -> list[dict[str, str]]:
    refs = {source_node_ref}
    for family in ("analogy", "event", "lineage", "principle", "state", "support", "synthesis"):
        base = repo_root / "ToS/canon" / family / "friedrich-nietzsche" / "thus-spoke-zarathustra" / "prologue-1"
        refs.update(path.relative_to(repo_root).as_posix() for path in base.rglob("node.json"))
    refs.update(
        {
            "ToS/canon/concept/becoming/node.json",
            "ToS/canon/concept/overcoming/node.json",
        }
    )
    rows = []
    for ref in sorted(refs):
        path = repo_root / ref
        payload = json.loads(path.read_text(encoding="utf-8"))
        rows.append(
            {
                "ref": ref,
                "sha256": _sha256(path),
                "node_id": payload["node_id"],
                "node_type": payload["node_type"],
            }
        )
    if len(rows) != 92:
        raise EvidenceBridgeError(f"expected 92 authored route nodes, observed {len(rows)}")
    if len({row["node_id"] for row in rows}) != len(rows):
        raise EvidenceBridgeError("authored route node IDs are not unique")
    return rows


def _relation_inventory(repo_root: Path, ref: str) -> tuple[list[dict[str, str]], dict[str, Any]]:
    with (repo_root / ref).open(encoding="utf-8", newline="") as handle:
        rows = list(csv.DictReader(handle))
    if len(rows) != 125:
        raise EvidenceBridgeError(f"expected 125 authored canon relations, observed {len(rows)}")
    ids = [row["edge_id"] for row in rows]
    if len(set(ids)) != len(ids):
        raise EvidenceBridgeError("authored canon relation IDs are not unique")
    valid_segments = {f"seg.1.1.1.{index}" for index in range(1, 13)}
    anchored = 0
    for row in rows:
        segments = {value for value in row["anchor_segment_ids"].split("|") if value}
        if not segments or not segments <= valid_segments:
            raise EvidenceBridgeError(f"relation {row['edge_id']} has invalid segment locators")
        anchored += 1
    inventory = {
        "relation_count": len(rows),
        "relation_kind_counts": dict(sorted(Counter(row["edge_kind"] for row in rows).items())),
        "relation_ids_sha256": _sha256_bytes(("\n".join(ids) + "\n").encode("utf-8")),
        "relations_with_legacy_segment_locators": anchored,
    }
    return rows, inventory


def _bridge_record(
    repo_root: Path,
    plan: dict[str, Any],
    *,
    source_node: dict[str, Any],
    authored_segments: list[dict[str, Any]],
    normalized: str,
    authored_spans: list[tuple[int, int]],
    record_bindings: dict[str, str],
) -> dict[str, Any]:
    node_rows = _route_nodes(repo_root, plan["authored_surfaces"]["source_node_ref"])
    _, relations = _relation_inventory(repo_root, plan["authored_surfaces"]["relation_pack_ref"])
    node_counts = dict(sorted(Counter(row["node_type"] for row in node_rows).items()))
    witnesses = source_node["language_witnesses"]
    witness_roles = []
    for witness in witnesses:
        witness_digest = _canonical_sha256(witness["segments"])
        is_source = witness["language"] == "de"
        witness_roles.append(
            {
                "language": witness["language"],
                "legacy_role": witness["role"],
                "segment_count": len(witness["segments"]),
                "content_sha256": witness_digest,
                "authorship_posture": (
                    "nietzsche-source-witness-role"
                    if is_source
                    else "dionysus-authored-translation-witness"
                ),
                "current_assurance": (
                    "mechanically-crosswalked-not-philologically-accepted"
                    if is_source
                    else "legacy-authored-translation-not-modernly-reviewed"
                ),
                "modern_review_refs": [],
            }
        )
    if {row["language"] for row in witness_roles} != {"de", "ru", "en"}:
        raise EvidenceBridgeError("expected exact de/ru/en authored witness roles")

    crosswalk = []
    for segment, unit_ref, anchor_ref, (start, end) in zip(
        authored_segments,
        plan["opaque_ids"]["authored_unit_ids"],
        plan["opaque_ids"]["authored_anchor_ids"],
        authored_spans,
        strict=True,
    ):
        legacy = _normalize_authored(segment["text"])
        observed = normalized[start:end]
        if legacy != observed:
            raise EvidenceBridgeError(f"authored segment does not match DTA: {segment['segment_id']}")
        crosswalk.append(
            {
                "legacy_segment_id": segment["segment_id"],
                "source_unit_ref": unit_ref,
                "source_anchor_ref": anchor_ref,
                "dta_normalized_sha256": _sha256_bytes(observed.encode("utf-8")),
                "legacy_normalized_sha256": _sha256_bytes(legacy.encode("utf-8")),
                "normalized_match": True,
                "match_scope": "mechanical-representation-only",
                "modern_review_refs": [],
            }
        )

    review_refs = plan["authored_surfaces"]["review_record_refs"]
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/authored-route-evidence-bridge-v1.schema.json",
        "schema_version": "tos_authored_route_evidence_bridge_v1",
        "bridge_id": "tos.authored-route-evidence-bridge.zarathustra-prologue-1",
        "bridge_version": 1,
        "research": _binding(repo_root, plan["research_ref"]),
        "route_scope": {
            key: plan["scope"][key]
            for key in (
                "route_ref",
                "work_ref",
                "expression_ref",
                "edition_ref",
                "item_ref",
                "file_ref",
                "file_sha256",
            )
        },
        "source_foundation": {
            "source_anchor": {"ref": plan["outputs"]["anchor_ref"], "sha256": record_bindings["anchor"]},
            "raw_layer": {"ref": plan["outputs"]["raw_layer_ref"], "sha256": record_bindings["raw_layer"]},
            "normalized_layer": {"ref": plan["outputs"]["normalized_layer_ref"], "sha256": record_bindings["normalized_layer"]},
            "unit_packet": {"ref": plan["outputs"]["unit_packet_ref"], "sha256": record_bindings["unit_packet"]},
            "rights": _binding(repo_root, plan["scope"]["rights_ref"]),
        },
        "authored_surfaces": {
            "source_node": _binding(repo_root, plan["authored_surfaces"]["source_node_ref"]),
            "alignment_witness": _binding(repo_root, plan["authored_surfaces"]["alignment_witness_ref"]),
            "relation_pack": _binding(repo_root, plan["authored_surfaces"]["relation_pack_ref"]),
            "review_records": [_binding(repo_root, ref) for ref in review_refs],
        },
        "witness_roles": witness_roles,
        "segment_crosswalk": crosswalk,
        "canon_inventory": {
            "node_count": len(node_rows),
            "node_kind_counts": node_counts,
            "node_records_sha256": _canonical_sha256(node_rows),
            **relations,
            "relations_with_modern_claim_refs": 0,
            "relations_with_modern_evidence_refs": 0,
            "legacy_review_record_count": len(review_refs),
            "machine_readable_human_attestation_count": 0,
        },
        "assurance": {
            "source_comparison": "complete-mechanical-normalized-match",
            "authored_review_posture": "legacy-review-records-without-machine-readable-human-attestation",
            "modern_semantic_packet_posture": "not-materialized-by-this-bridge",
            "claim_evidence_closure": False,
            "graph_admission": False,
            "current_canon_retained": True,
            "bulk_migration_authorized": False,
        },
        "effects": {
            "source_text_accepted": False,
            "german_competence_attested": False,
            "translation_accepted": False,
            "sign_or_concept_promoted": False,
            "legacy_relation_migrated": False,
            "graph_projection_admitted": False,
            "canon_revised": False,
            "human_task_created": False,
            "new_publication_authorized": False,
            "server_transfer_authorized": False,
        },
        "authority_boundary": BRIDGE_AUTHORITY_BOUNDARY,
    }


def _private_comparison(
    *,
    raw: str,
    normalized: str,
    source_parts: list[str],
    source_spans: list[tuple[int, int]],
    authored_segments: list[dict[str, Any]],
    authored_spans: list[tuple[int, int]],
) -> dict[str, Any]:
    rows = []
    for segment, (start, end) in zip(authored_segments, authored_spans, strict=True):
        legacy = _normalize_authored(segment["text"])
        rows.append(
            {
                "legacy_segment_id": segment["segment_id"],
                "source_span": [start, end],
                "dta_normalized": normalized[start:end],
                "legacy_normalized": legacy,
                "exact_match": normalized[start:end] == legacy,
            }
        )
    return {
        "schema_version": "tos_private_authored_route_comparison_v1",
        "raw_sha256": _sha256_bytes(raw.encode("utf-8")),
        "normalized_sha256": _sha256_bytes(normalized.encode("utf-8")),
        "source_paragraph_count": len(source_parts),
        "source_paragraph_spans": [list(span) for span in source_spans],
        "authored_segment_count": len(rows),
        "authored_segments": rows,
        "complete_sequence_match": all(row["exact_match"] for row in rows),
        "authority_boundary": "private mechanical comparison details only; no language, translation, semantic, review, or canon authority",
    }


def _provenance_event(
    *,
    repo_root: Path,
    plan: dict[str, Any],
    plan_digest: str,
    source_path: Path,
    private_outputs: dict[str, bytes],
    tracked_outputs: dict[str, bytes],
) -> dict[str, Any]:
    builder_path = repo_root / BUILDER_REF
    builder_digest = _sha256(builder_path)
    executable = Path(sys.executable).resolve()
    outputs = []
    for ref, payload in private_outputs.items():
        media_type = "application/json" if ref.endswith(".json") else "text/plain; charset=utf-8"
        outputs.append(
            {
                "entity_ref": ref,
                "role": "ignored-local-evidence-bridge-artifact",
                "sha256": _sha256_bytes(payload),
                "size_bytes": len(payload),
                "media_type": media_type,
                "availability": "ignored_local",
                "content_disclosure": "private_content",
                "fixity_verified": True,
                "fixity_verified_at": plan["created_at"],
            }
        )
    for ref, payload in tracked_outputs.items():
        outputs.append(
            {
                "entity_ref": ref,
                "role": "tracked-text-free-evidence-bridge-record",
                "sha256": _sha256_bytes(payload),
                "size_bytes": len(payload),
                "media_type": "application/json",
                "availability": "tracked",
                "content_disclosure": "public_metadata_only",
                "fixity_verified": True,
                "fixity_verified_at": plan["created_at"],
            }
        )
    tracked_input_specs = [
        (PLAN_REF, "tracked-text-free-bridge-plan", "public_metadata_only"),
        (plan["research_ref"], "tracked-reconciliation-research", "public_content"),
        (plan["scope"]["rights_ref"], "tracked-source-rights-record", "public_metadata_only"),
        (
            plan["authored_surfaces"]["source_node_ref"],
            "tracked-authored-source-node",
            "public_content",
        ),
        (
            plan["authored_surfaces"]["alignment_witness_ref"],
            "tracked-authored-trilingual-donor",
            "public_content",
        ),
        (
            plan["authored_surfaces"]["relation_pack_ref"],
            "tracked-authored-relation-pack",
            "public_content",
        ),
    ]
    tracked_input_specs.extend(
        (ref, "tracked-legacy-review-record", "public_content")
        for ref in plan["authored_surfaces"]["review_record_refs"]
    )
    tracked_inputs = []
    for ref, role, disclosure in tracked_input_specs:
        path = repo_root / ref
        suffix = path.suffix.lower()
        media_type = {
            ".json": "application/json",
            ".md": "text/markdown; charset=utf-8",
            ".csv": "text/csv; charset=utf-8",
        }.get(suffix, "application/octet-stream")
        tracked_inputs.append(
            {
                "entity_ref": ref,
                "role": role,
                "sha256": _sha256(path),
                "size_bytes": path.stat().st_size,
                "media_type": media_type,
                "availability": "tracked",
                "content_disclosure": disclosure,
                "fixity_verified": True,
                "fixity_verified_at": plan["created_at"],
            }
        )
    derivations = [
        {
            "derivation_id": f"tos.derivation.zarathustra-authored-canon-evidence-bridge.{index}",
            "input_entity_ref": plan["scope"]["source_relative_ref"],
            "output_entity_ref": output["entity_ref"],
            "relation": "selection_from",
            "influence_asserted": True,
            "description": "The exact DTA TEI, frozen plan, and digest-bound authored route surfaces determine this non-promoting bridge output.",
        }
        for index, output in enumerate(outputs, start=1)
    ]
    bridge_ref = plan["outputs"]["bridge_ref"]
    for index, (ref, _, _) in enumerate(tracked_input_specs[3:], start=len(derivations) + 1):
        derivations.append(
            {
                "derivation_id": (
                    "tos.derivation.zarathustra-authored-canon-evidence-bridge."
                    f"authored-{index}"
                ),
                "input_entity_ref": ref,
                "output_entity_ref": bridge_ref,
                "relation": "was_derived_from",
                "influence_asserted": True,
                "description": "The text-free bridge inventory and assurance posture bind this exact authored-route or historical-review surface without promoting it.",
            }
        )
    argv = [
        "python",
        BUILDER_REF,
        "--build",
        "--local-input-root",
        "/srv/AbyssOS/Tree-of-Sophia",
        "--local-output-root",
        "/srv/AbyssOS/Tree-of-Sophia",
    ]
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
            "event_type": "alignment",
            "started_at": plan["created_at"],
            "ended_at": plan["created_at"],
            "status": "completed_with_warnings",
            "terminal_reason": None,
            "exit_code": 0,
            "warnings": [
                "All twelve normalized German segment matches are mechanical representation evidence only.",
                "Legacy review notes contain no modern machine-readable human attestation.",
                "The bridge changes no source, translation, semantic, graph, canon, publication, or transfer authority.",
            ],
        },
        "entities": {
            "inputs": [
                {
                    "entity_ref": plan["scope"]["source_relative_ref"],
                    "role": "fixity-verified-local-dta-tei-source",
                    "sha256": plan["scope"]["file_sha256"],
                    "size_bytes": source_path.stat().st_size,
                    "media_type": "application/xml",
                    "availability": "owner_local",
                    "content_disclosure": "private_content",
                    "fixity_verified": True,
                    "fixity_verified_at": plan["created_at"],
                },
                *tracked_inputs,
            ],
            "outputs": outputs,
            "byproducts": [],
        },
        "derivations": derivations,
        "responsibility": [
            {
                "agent_ref": "software:tos-zarathustra-authored-canon-evidence-bridge-builder",
                "agent_kind": "software",
                "role": "executor",
                "responsibility_posture": "performed",
                "evidence_binding": {"ref": BUILDER_REF, "sha256": builder_digest},
                "human_evidence_status": "not_applicable",
            }
        ],
        "method": {
            "procedure": {"name": "authored-canon-evidence-bridge", "version": "1", "purpose": plan["question"]},
            "command_capture": {
                "disclosure": "withheld_digest_only",
                "argv": None,
                "argv_sha256": _canonical_sha256(argv),
                "withholding_reason": (
                    "The exact build argv contains the owner-local absolute corpus root; "
                    "its digest is retained without embedding a non-portable machine path."
                ),
            },
            "configuration_binding": {"ref": PLAN_REF, "sha256": plan_digest},
            "software_components": [
                {
                    "name": "Tree of Sophia authored canon evidence bridge builder",
                    "version": "1",
                    "role": "section-extraction-normalization-crosswalk-and-record-builder",
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
                "backend": "python-standard-library-elementtree-difflib-csv",
                "hardware_target": "cpu",
                "unicode_version": unicodedata.unidata_version,
                "environment_profile_binding": {"ref": PLAN_REF, "sha256": plan_digest},
            },
        },
        "manual_changes": {
            "status": "none_declared",
            "change_receipts": [],
            "statement": "No manual content edit occurred; raw extraction, declared comparison normalization, source/authored boundary construction, and digest inventory are deterministic builder operations.",
        },
        "measurements": [
            {"metric": "input_bytes", "status": "measured", "value": source_path.stat().st_size, "unit": "bytes", "method": "filesystem stat after exact digest verification", "evidence_binding": None},
            {"metric": "output_bytes", "status": "measured", "value": sum(row["size_bytes"] for row in outputs), "unit": "bytes", "method": "sum of exact serialized output entity byte counts", "evidence_binding": None},
            {"metric": "human_active_seconds", "status": "not_applicable", "value": None, "unit": None, "method": "no human content review or correction was scheduled or performed", "evidence_binding": None},
        ],
        "evidence_authentication": {
            "capture_posture": "tool_captured",
            "signature_status": "unsigned",
            "signature_bindings": [],
            "verification_status": "mechanically_verified",
            "producer_control_boundary": "The same local builder emitted the unsigned receipt; exact replay proves closure, not execution authenticity, human authorship, language competence, or semantic truth.",
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
            "human_review_status": "blocked",
            "review_bindings": [],
            "accepted_uses": [],
            "promotion_authorized": False,
            "competence_evidence_bindings": [],
        },
        "reproducibility": {
            "classification": "partially_specified",
            "known_gaps": [
                "The receipt is unsigned and self-recorded by the transformation runner.",
                "The exact build argv is withheld because it contains the owner-local absolute corpus root; replay requires an operator-supplied local input/output root.",
                "No German-competent human reviewed the extracted text or authored boundary map.",
                "No old review note was reinterpreted as a modern human attestation.",
                "Private source and comparison artifacts remain intentionally absent from Git.",
            ],
            "replay_scope": "Exact DTA digest, frozen plan and builder, source/authored surface digests, declared normalization, complete twelve-segment sequence match, private artifact fixity, text-free bridge records, and zero-authority posture.",
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
            for error in errors[:16]
        )
        raise EvidenceBridgeError(f"{schema_ref} validation failed: {details}")


def _semantic_checks(
    anchor: dict[str, Any],
    raw_layer: dict[str, Any],
    normalized_layer: dict[str, Any],
    packet: dict[str, Any],
    event: dict[str, Any],
    normalized: str,
) -> None:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import validate_source_witness_foundation as foundation  # noqa: PLC0415

    checks = [
        ("anchor", foundation._anchor_v2_semantic_issues(anchor)),
        ("raw layer", foundation._source_text_layer_semantic_issues(raw_layer)),
        ("normalized layer", foundation._source_text_layer_semantic_issues(normalized_layer)),
        ("text units", foundation._source_text_unit_v1_issues(packet, text=normalized)),
        ("provenance", foundation._provenance_v2_semantic_issues(event)),
    ]
    failures = [f"{label}: {message}" for label, rows in checks for message in rows]
    if failures:
        raise EvidenceBridgeError("; ".join(failures[:24]))


def _assert_ignored(repo_root: Path, ref: str) -> None:
    result = subprocess.run(
        ["git", "check-ignore", "--quiet", "--", ref],
        cwd=repo_root,
        check=False,
    )
    if result.returncode != 0:
        raise EvidenceBridgeError(f"private output is not Git-ignored: {ref}")


def _write_private(path: Path, payload: bytes) -> None:
    if path.exists() and path.is_symlink():
        raise EvidenceBridgeError(f"private output path is symlinked: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    os.chmod(path, 0o600)


def _write_tracked(path: Path, payload: bytes) -> None:
    if path.exists() and path.is_symlink():
        raise EvidenceBridgeError(f"tracked output path is symlinked: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)


def _expected(
    repo_root: Path, local_input_root: Path
) -> tuple[dict[str, bytes], dict[str, bytes], dict[str, Any], dict[str, Any]]:
    plan_path = repo_root / PLAN_REF
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    plan_digest = _sha256(plan_path)
    source_path = local_input_root / plan["scope"]["source_relative_ref"]
    if source_path.is_symlink() or not source_path.is_file():
        raise EvidenceBridgeError(f"exact local source is absent or symlinked: {source_path}")
    if _sha256(source_path) != plan["scope"]["file_sha256"]:
        raise EvidenceBridgeError("exact local DTA source digest drifted")

    _, paragraphs = _select_section(source_path)
    raw_parts = [_render_inline(paragraph) for paragraph in paragraphs]
    raw = "\n\n".join(raw_parts)
    normalized_source_parts = [_normalize_source(part) for part in raw_parts]
    normalized = _normalize_source(raw)
    source_spans, _ = _spans(normalized_source_parts, normalized)

    source_node_path = repo_root / plan["authored_surfaces"]["source_node_ref"]
    source_node = json.loads(source_node_path.read_text(encoding="utf-8"))
    if source_node.get("node_id") != plan["scope"]["route_ref"]:
        raise EvidenceBridgeError("authored source node identity drifted")
    de_witnesses = [
        row for row in source_node.get("language_witnesses", []) if row.get("language") == "de"
    ]
    if len(de_witnesses) != 1 or len(de_witnesses[0].get("segments", [])) != 12:
        raise EvidenceBridgeError("expected one twelve-segment German authored witness")
    authored_segments = de_witnesses[0]["segments"]
    expected_segment_ids = [f"seg.1.1.1.{index}" for index in range(1, 13)]
    if [segment["segment_id"] for segment in authored_segments] != expected_segment_ids:
        raise EvidenceBridgeError("authored segment identity/order drifted")
    authored_parts = [_normalize_authored(segment["text"]) for segment in authored_segments]
    authored_spans, _ = _spans(authored_parts, normalized)

    operations = _normalization_operations(raw, normalized)
    comparison = _private_comparison(
        raw=raw,
        normalized=normalized,
        source_parts=normalized_source_parts,
        source_spans=source_spans,
        authored_segments=authored_segments,
        authored_spans=authored_spans,
    )
    private_outputs = {
        plan["outputs"]["private_raw_content_ref"]: raw.encode("utf-8"),
        plan["outputs"]["private_normalized_content_ref"]: normalized.encode("utf-8"),
        plan["outputs"]["private_operations_ref"]: _json_bytes(operations),
        plan["outputs"]["private_comparison_ref"]: _json_bytes(comparison),
    }

    rights_digest = _sha256(repo_root / plan["scope"]["rights_ref"])
    anchor = _anchor_record(plan, plan_digest)
    anchor_bytes = _json_bytes(anchor)
    raw_content = private_outputs[plan["outputs"]["private_raw_content_ref"]]
    raw_layer = _common_layer(
        plan,
        layer_id=plan["opaque_ids"]["raw_layer_id"],
        layer_role="machine_transcription",
        content_ref=plan["outputs"]["private_raw_content_ref"],
        content=raw_content,
        anchor_digest=_sha256_bytes(anchor_bytes),
        rights_digest=rights_digest,
        plan_digest=plan_digest,
        derivation={
            "method": "structural_extraction",
            "input_layers": [],
            "maker": {
                "maker_type": "software",
                "agent_ref": "software:tos-zarathustra-authored-canon-evidence-bridge-builder",
                "method": "tei-lb-aware-section-extraction",
                "version": "1",
                "configuration_ref": PLAN_REF,
                "configuration_digest": plan_digest,
            },
            "preservation_goal": "source_near",
            "loss_posture": "preservation_intended",
            "silent_changes_allowed": False,
            "change_payload": {"kind": "none"},
        },
        character_normalization="none",
        line_break_posture="source_preserved",
        transcription_goal="machine_candidate",
        layout_posture="encode_explicitly",
    )
    raw_layer_bytes = _json_bytes(raw_layer)
    normalized_content = private_outputs[plan["outputs"]["private_normalized_content_ref"]]
    normalized_layer = _common_layer(
        plan,
        layer_id=plan["opaque_ids"]["normalized_layer_id"],
        layer_role="normalized_text",
        content_ref=plan["outputs"]["private_normalized_content_ref"],
        content=normalized_content,
        anchor_digest=_sha256_bytes(anchor_bytes),
        rights_digest=rights_digest,
        plan_digest=plan_digest,
        derivation={
            "method": "editorial_normalization",
            "input_layers": [
                {
                    "layer_id": plan["opaque_ids"]["raw_layer_id"],
                    "record_ref": plan["outputs"]["raw_layer_ref"],
                    "record_sha256": _sha256_bytes(raw_layer_bytes),
                    "content_sha256": _sha256_bytes(raw_content),
                }
            ],
            "maker": {
                "maker_type": "software",
                "agent_ref": "software:tos-zarathustra-authored-canon-evidence-bridge-builder",
                "method": plan["normalization_policy"]["method"],
                "version": plan["normalization_policy"]["method_version"],
                "configuration_ref": PLAN_REF,
                "configuration_digest": plan_digest,
            },
            "preservation_goal": "normalized_for_search",
            "loss_posture": "normalization_intended",
            "silent_changes_allowed": False,
            "change_payload": {
                "kind": "withheld_operations_receipt",
                "operation_count": operations["operation_count"],
                "operations_sha256": _sha256_bytes(
                    private_outputs[plan["outputs"]["private_operations_ref"]]
                ),
                "private_ref": plan["outputs"]["private_operations_ref"],
            },
        },
        character_normalization="editorial",
        line_break_posture="logical_reflow",
        transcription_goal="normalized_access",
        layout_posture="logical_reflow",
    )
    normalized_layer_bytes = _json_bytes(normalized_layer)
    packet = _unit_packet(
        plan,
        normalized=normalized,
        source_parts=normalized_source_parts,
        authored_parts=authored_parts,
    )
    packet_bytes = _json_bytes(packet)
    record_bindings = {
        "anchor": _sha256_bytes(anchor_bytes),
        "raw_layer": _sha256_bytes(raw_layer_bytes),
        "normalized_layer": _sha256_bytes(normalized_layer_bytes),
        "unit_packet": _sha256_bytes(packet_bytes),
    }
    bridge = _bridge_record(
        repo_root,
        plan,
        source_node=source_node,
        authored_segments=authored_segments,
        normalized=normalized,
        authored_spans=authored_spans,
        record_bindings=record_bindings,
    )
    bridge_bytes = _json_bytes(bridge)
    tracked_outputs = {
        plan["outputs"]["anchor_ref"]: anchor_bytes,
        plan["outputs"]["raw_layer_ref"]: raw_layer_bytes,
        plan["outputs"]["normalized_layer_ref"]: normalized_layer_bytes,
        plan["outputs"]["unit_packet_ref"]: packet_bytes,
        plan["outputs"]["bridge_ref"]: bridge_bytes,
    }
    event = _provenance_event(
        repo_root=repo_root,
        plan=plan,
        plan_digest=plan_digest,
        source_path=source_path,
        private_outputs=private_outputs,
        tracked_outputs=tracked_outputs,
    )
    tracked_outputs[plan["outputs"]["provenance_event_ref"]] = _json_bytes(event)

    _validate_schema(repo_root, ANCHOR_SCHEMA_REF, anchor)
    _validate_schema(repo_root, LAYER_SCHEMA_REF, raw_layer)
    _validate_schema(repo_root, LAYER_SCHEMA_REF, normalized_layer)
    _validate_schema(repo_root, UNIT_SCHEMA_REF, packet)
    _validate_schema(repo_root, BRIDGE_SCHEMA_REF, bridge)
    _validate_schema(repo_root, PROVENANCE_SCHEMA_REF, event)
    _semantic_checks(anchor, raw_layer, normalized_layer, packet, event, normalized)

    for protected in (raw, normalized):
        if any(protected in payload.decode("utf-8") for payload in tracked_outputs.values()):
            raise EvidenceBridgeError("tracked output exposes a complete private source layer")
    return tracked_outputs, private_outputs, plan, bridge


def run(
    *,
    repo_root: Path,
    local_input_root: Path,
    local_output_root: Path,
    build: bool,
) -> dict[str, Any]:
    tracked, private, plan, bridge = _expected(repo_root, local_input_root)
    for ref in private:
        _assert_ignored(repo_root, ref)
    if build:
        for ref, payload in private.items():
            _write_private(local_output_root / ref, payload)
        for ref, payload in tracked.items():
            _write_tracked(repo_root / ref, payload)
    for ref, expected in private.items():
        path = local_output_root / ref
        if not path.is_file() or path.is_symlink() or path.read_bytes() != expected:
            raise EvidenceBridgeError(f"private output is absent or stale: {ref}")
        if stat.S_IMODE(path.stat().st_mode) != 0o600:
            raise EvidenceBridgeError(f"private output mode is not 0600: {ref}")
    for ref, expected in tracked.items():
        path = repo_root / ref
        if not path.is_file() or path.is_symlink() or path.read_bytes() != expected:
            raise EvidenceBridgeError(f"tracked output is absent or stale: {ref}")
    return {
        "status": "passed",
        "question": plan["question"],
        "source_sha256": plan["scope"]["file_sha256"],
        "source_paragraph_count": 12,
        "authored_segment_count": len(bridge["segment_crosswalk"]),
        "normalized_match_count": sum(
            1 for row in bridge["segment_crosswalk"] if row["normalized_match"]
        ),
        "canon_node_count": bridge["canon_inventory"]["node_count"],
        "canon_relation_count": bridge["canon_inventory"]["relation_count"],
        "modern_claim_closure_count": bridge["canon_inventory"]["relations_with_modern_claim_refs"],
        "modern_human_attestation_count": bridge["canon_inventory"]["machine_readable_human_attestation_count"],
        "tracked_record_count": len(tracked),
        "private_artifact_count": len(private),
        "accepted_german": False,
        "accepted_translation": False,
        "semantic_or_graph_promotion": False,
        "canon_revised": False,
        "human_task_created": False,
        "new_publication_authorized": False,
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
    except EvidenceBridgeError as exc:
        print(f"authored-canon evidence bridge: FAIL: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

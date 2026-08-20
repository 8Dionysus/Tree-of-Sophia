#!/usr/bin/env python3
"""Build one real, source-returnable, non-promoting sentence alignment proposal.

The builder reads two already materialized owner-local paragraph layers, freezes
only exact selectors and digests in Git, and never copies either sentence into
tracked output.  Its one-to-one mapping is a mechanical proposal, not accepted
German, accepted Russian, translation fidelity, lexical equivalence, semantics,
graph truth, canon authority, or publication permission.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import subprocess
import sys
import unicodedata
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
PLAN_REF = (
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1/"
    "za-i-vorrede-1-opening-sentence-alignment.plan.v1.json"
)
UNIT_SCHEMA_REF = "ToS/contracts/source-text-unit-packet-v1.schema.json"
ALIGNMENT_SCHEMA_REF = "ToS/contracts/translation-alignment-packet-v1.schema.json"
PROVENANCE_SCHEMA_REF = "ToS/contracts/provenance-event-v2.schema.json"
BUILDER_REF = "scripts/build_zarathustra_opening_sentence_alignment.py"
EVENT_ID = (
    "tos.event.alignment.za-i-vorrede-1."
    "dta-1883-antonovsky-1911-opening-sentence.2026-08-12"
)


class OpeningSentenceAlignmentError(RuntimeError):
    """Raised when exact source return or a fail-closed boundary drifts."""


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
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode(
            "utf-8"
        )
    )


def _load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise OpeningSentenceAlignmentError(f"cannot load JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise OpeningSentenceAlignmentError(f"JSON root is not an object: {path}")
    return payload


def _assert_binding(repo_root: Path, ref: str, expected: str, label: str) -> Path:
    path = repo_root / ref
    if path.is_symlink() or not path.is_file():
        raise OpeningSentenceAlignmentError(f"{label} is absent or symlinked: {ref}")
    observed = _sha256(path)
    if observed != expected:
        raise OpeningSentenceAlignmentError(
            f"{label} digest drifted: expected {expected}, observed {observed}"
        )
    return path


def _assert_private_ref(repo_root: Path, ref: str) -> None:
    result = subprocess.run(
        ["git", "check-ignore", "--quiet", "--", ref],
        cwd=repo_root,
        check=False,
    )
    if result.returncode != 0:
        raise OpeningSentenceAlignmentError(f"private source is not Git-ignored: {ref}")


def _read_side(
    *, repo_root: Path, local_input_root: Path, side_name: str, side: dict[str, Any]
) -> tuple[str, str, str]:
    _assert_binding(
        repo_root,
        side["text_layer_ref"],
        side["text_layer_record_sha256"],
        f"{side_name} text-layer record",
    )
    _assert_binding(
        repo_root,
        side["rights_ref"],
        side["rights_sha256"],
        f"{side_name} rights record",
    )
    _assert_binding(
        repo_root,
        side["layout_packet_ref"],
        side["layout_packet_sha256"],
        f"{side_name} layout packet",
    )
    if side_name == "source":
        _assert_binding(
            repo_root,
            side["edition_reading_admission_ref"],
            side["edition_reading_admission_sha256"],
            "source edition-reading admission",
        )
    else:
        _assert_binding(
            repo_root,
            side["expression_record_ref"],
            side["expression_record_sha256"],
            "target expression record",
        )
        _assert_binding(
            repo_root,
            side["responsibility_claims_ref"],
            side["responsibility_claims_sha256"],
            "target responsibility claims",
        )

    layer = _load_json(repo_root / side["text_layer_ref"])
    representation = layer.get("representation", {})
    if representation.get("content_ref") != side["private_content_ref"]:
        raise OpeningSentenceAlignmentError(f"{side_name} private content ref drifted")
    if representation.get("content_sha256") != side["text_layer_sha256"]:
        raise OpeningSentenceAlignmentError(f"{side_name} content digest binding drifted")
    if representation.get("language") != side["language"]:
        raise OpeningSentenceAlignmentError(f"{side_name} language binding drifted")
    if representation.get("tracked_content") is not False:
        raise OpeningSentenceAlignmentError(f"{side_name} content became tracked")
    if representation.get("publication_authorized") is not False:
        raise OpeningSentenceAlignmentError(
            f"{side_name} publication boundary unexpectedly widened"
        )

    private_ref = side["private_content_ref"]
    _assert_private_ref(repo_root, private_ref)
    private_path = local_input_root / private_ref
    if private_path.is_symlink() or not private_path.is_file():
        raise OpeningSentenceAlignmentError(
            f"{side_name} private layer is absent or symlinked: {private_path}"
        )
    try:
        text = private_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        raise OpeningSentenceAlignmentError(
            f"cannot read exact {side_name} private layer: {exc}"
        ) from exc
    if _sha256_bytes(text.encode("utf-8")) != side["text_layer_sha256"]:
        raise OpeningSentenceAlignmentError(f"{side_name} private layer digest drifted")
    if len(text) != side["scope_end"]:
        raise OpeningSentenceAlignmentError(f"{side_name} private layer length drifted")

    start = side["sentence_start"]
    end = side["sentence_end"]
    if start != 0 or not (0 < end < len(text)):
        raise OpeningSentenceAlignmentError(f"{side_name} sentence selector is invalid")
    if text.find(".") + 1 != end or not text[start:end].endswith("."):
        raise OpeningSentenceAlignmentError(
            f"{side_name} first-full-stop boundary drifted"
        )
    if not text[end].isspace():
        raise OpeningSentenceAlignmentError(
            f"{side_name} post-sentence whitespace guard drifted"
        )
    sentence = text[start:end]
    remainder = text[end:]
    sentence_digest = _sha256_bytes(sentence.encode("utf-8"))
    remainder_digest = _sha256_bytes(remainder.encode("utf-8"))
    if sentence_digest != side["sentence_sha256"]:
        raise OpeningSentenceAlignmentError(f"{side_name} sentence digest drifted")
    if remainder_digest != side["remainder_sha256"]:
        raise OpeningSentenceAlignmentError(f"{side_name} remainder digest drifted")
    return text, sentence, remainder


def _method(
    *, plan: dict[str, Any], side_name: str, language: str
) -> dict[str, Any]:
    return {
        "maker_kind": "software",
        "agent_ref": "software:tos-zarathustra-opening-sentence-alignment-builder",
        "method_name": "exact first-full-stop-inclusive sentence-boundary proposal",
        "method_version": plan["method"]["version"],
        "software_refs": [BUILDER_REF],
        "model_ref": None,
        "configuration_ref": PLAN_REF,
        "locale": language,
        "unicode_version": unicodedata.unidata_version,
        "unicode_revision": None,
        "tailoring_ref": None,
        "provenance_event_ref": EVENT_ID,
        "made_at": plan["created_at"],
        "output_posture": "method_result_not_source_or_linguistic_truth",
    }


def _unit_packet(
    *,
    plan: dict[str, Any],
    side_name: str,
    text: str,
    sentence: str,
    remainder: str,
) -> dict[str, Any]:
    side = plan[side_name]
    ids = plan["opaque_ids"]
    prefix = "source" if side_name == "source" else "target"
    packet_id = ids[f"{prefix}_packet_id"]
    scheme_id = ids[f"{prefix}_scheme_id"]
    segmentation_id = ids[f"{prefix}_segmentation_id"]
    scope_anchor_id = ids[f"{prefix}_scope_anchor_id"]
    sentence_anchor_id = ids[f"{prefix}_sentence_anchor_id"]
    remainder_anchor_id = ids[f"{prefix}_remainder_anchor_id"]
    sentence_unit_id = ids[f"{prefix}_sentence_unit_id"]
    method = _method(plan=plan, side_name=side_name, language=side["language"])
    sentence_end = side["sentence_end"]

    def anchor(
        *, anchor_ref: str, ordinal: int, role: str, start: int, end: int, digest: str
    ) -> dict[str, Any]:
        return {
            "anchor_ref": anchor_ref,
            "ordinal": ordinal,
            "text_layer_ref": side["text_layer_ref"],
            "text_layer_sha256": side["text_layer_sha256"],
            "selector": {
                "type": "text_position",
                "start": start,
                "end": end,
                "position_unit": "unicode_code_point",
                "interval": "half_open",
            },
            "exact_sha256": digest,
            "anchor_role": role,
            "source_return": {
                "required": True,
                "locator_ref": side["private_content_ref"],
            },
        }

    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "source-text-unit-packet-v1.schema.json"
        ),
        "schema_version": "tos_source_text_unit_packet_v1",
        "packet_id": packet_id,
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "source_bound",
        "source_scope": {
            key: side[key]
            for key in (
                "work_ref",
                "expression_ref",
                "edition_ref",
                "item_ref",
                "file_ref",
                "file_sha256",
            )
        },
        "source_layer": {
            "text_layer_ref": side["text_layer_ref"],
            "text_layer_sha256": side["text_layer_sha256"],
            "language": side["language"],
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
                "scheme_id": scheme_id,
                "scheme_version": 1,
                "supersedes_scheme_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis"
                ),
                "scheme_name": (
                    f"{side['language']} exact first-full-stop-inclusive sentence proposal"
                ),
                "analysis_role": "orthographic",
                "boundary_basis": "rule_based",
                "unit_kinds": ["sentence"],
                "method": method,
                "policies": {
                    "normalization": "no-text-mutation-separate-successor-layer",
                    "punctuation": "included_in_neighbor",
                    "whitespace": "declared_excluded",
                    "line_break": "declared_excluded",
                    "hyphenation": "preserve_source",
                    "unreported_gaps_allowed": False,
                    "overlap": "forbid",
                },
                "authority_limit": {
                    "algorithmic_output_is_source_truth": False,
                    "algorithmic_output_is_linguistic_truth": False,
                    "model_subword_is_lexeme": False,
                    "unit_identity_is_semantic_identity": False,
                    "text_mutation_allowed": False,
                },
            }
        ],
        "anchors": [
            anchor(
                anchor_ref=scope_anchor_id,
                ordinal=1,
                role="scope",
                start=0,
                end=len(text),
                digest=side["text_layer_sha256"],
            ),
            anchor(
                anchor_ref=sentence_anchor_id,
                ordinal=2,
                role="content",
                start=0,
                end=sentence_end,
                digest=_sha256_bytes(sentence.encode("utf-8")),
            ),
            anchor(
                anchor_ref=remainder_anchor_id,
                ordinal=3,
                role="gap",
                start=sentence_end,
                end=len(text),
                digest=_sha256_bytes(remainder.encode("utf-8")),
            ),
        ],
        "units": [
            {
                "unit_id": sentence_unit_id,
                "unit_version": 1,
                "supersedes_unit_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis"
                ),
                "unit_kind": "sentence",
                "surface_posture": "source_bearing",
                "continuity": "contiguous",
                "ordered_anchor_refs": [sentence_anchor_id],
                "parent_unit_refs": [],
                "ordered_child_unit_refs": [],
                "boundary_posture": "method_proposed",
                "certainty": {
                    "value": 0.5,
                    "meaning": (
                        "maker-declared-boundary-confidence-not-truth-probability"
                    ),
                },
                "status_reason": (
                    "The exact first U+002E boundary is mechanically selected for a bounded "
                    "alignment proposal; it is not accepted sentence analysis or accepted text."
                ),
                "source_text_mutated": False,
                "semantic_promotion": False,
            }
        ],
        "segmentations": [
            {
                "segmentation_id": segmentation_id,
                "segmentation_version": 1,
                "supersedes_segmentation_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries"
                ),
                "scheme_ref": scheme_id,
                "ordered_unit_refs": [sentence_unit_id],
                "coverage": {
                    "scope_anchor_ref": scope_anchor_id,
                    "coverage_posture": "declared_partial",
                    "excluded_anchor_refs": [remainder_anchor_id],
                    "unreported_gaps_allowed": False,
                    "overlap_requires_declaration": True,
                    "source_reconstruction_required": True,
                },
                "status": "proposed",
                "status_reason": (
                    "Only the first full-stop-delimited span is proposed; the exact remainder "
                    "is digest-bound as excluded coverage and no linguistic review occurred."
                ),
                "maker": method,
                "competing_segmentation_refs": [],
                "review_refs": [],
                "declared_uses": ["source_observation", "translation_alignment"],
                "source_text_authority": False,
                "linguistic_authority": False,
                "semantic_authority": False,
            }
        ],
        "reviews": [],
        "projections": [],
        "rights_and_visibility": {
            "source_visibility": "local_only",
            "packet_visibility": "public_metadata_only",
            "effective_visibility": "local_only",
            "rights_record_refs": [side["rights_ref"]],
            "private_source_used": True,
            "publication_authorized": False,
            "inheritance_policy": (
                "most-restrictive-source-packet-and-destination-wins"
            ),
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


def _alignment_side(
    *,
    plan: dict[str, Any],
    side_name: str,
    packet_ref: str,
    packet_digest: str,
) -> dict[str, Any]:
    side = plan[side_name]
    prefix = "source" if side_name == "source" else "target"
    anchor_ref = plan["opaque_ids"][f"{prefix}_sentence_anchor_id"]
    return {
        "side_role": side_name,
        "work_ref": side["work_ref"],
        "expression_ref": side["expression_ref"],
        "edition_ref": side["edition_ref"],
        "item_ref": side["item_ref"],
        "file_ref": side["file_ref"],
        "file_sha256": side["file_sha256"],
        "text_layer_ref": side["text_layer_ref"],
        "text_layer_sha256": side["text_layer_sha256"],
        "language": side["language"],
        "segmentation": {
            "artifact_ref": packet_ref,
            "sha256": packet_digest,
            "state": "frozen",
        },
        "tokenization": None,
        "anchors": [
            {
                "anchor_ref": anchor_ref,
                "ordinal": 1,
                "text_layer_ref": side["text_layer_ref"],
                "text_layer_sha256": side["text_layer_sha256"],
                "selector": {
                    "type": "text_position",
                    "start": side["sentence_start"],
                    "end": side["sentence_end"],
                    "position_unit": "unicode_code_point",
                    "interval": "half_open",
                },
                "exact_sha256": side["sentence_sha256"],
                "source_return": {
                    "required": True,
                    "locator_ref": side["private_content_ref"],
                },
            }
        ],
        "rights_refs": [side["rights_ref"]],
        "visibility": "local_only",
        "publication_authorized": False,
    }


def _alignment_packet(
    *,
    plan: dict[str, Any],
    plan_digest: str,
    source_packet_ref: str,
    source_packet_digest: str,
    target_packet_ref: str,
    target_packet_digest: str,
) -> dict[str, Any]:
    ids = plan["opaque_ids"]
    source_anchor = ids["source_sentence_anchor_id"]
    target_anchor = ids["target_sentence_anchor_id"]
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "translation-alignment-packet-v1.schema.json"
        ),
        "schema_version": "tos_translation_alignment_packet_v1",
        "packet_id": ids["alignment_packet_id"],
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "source_bound",
        "granularity": "sentence",
        "source_side": _alignment_side(
            plan=plan,
            side_name="source",
            packet_ref=source_packet_ref,
            packet_digest=source_packet_digest,
        ),
        "target_side": _alignment_side(
            plan=plan,
            side_name="target",
            packet_ref=target_packet_ref,
            packet_digest=target_packet_digest,
        ),
        "alignments": [
            {
                "alignment_id": ids["alignment_id"],
                "alignment_version": 1,
                "supersedes_alignment_ref": None,
                "identity_policy": (
                    "opaque-id-independent-of-text-label-translation-and-current-mapping"
                ),
                "claim_id": ids["alignment_claim_id"],
                "claim_version": 1,
                "supersedes_claim_ref": None,
                "direction": "source_to_target",
                "correspondence_shape": "one_to_one",
                "order_posture": "monotonic",
                "ordered_source_anchor_refs": [source_anchor],
                "ordered_target_anchor_refs": [target_anchor],
                "translation_techniques": ["unresolved"],
                "epistemic_status": "inferred",
                "certainty": {
                    "value": 0.5,
                    "meaning": (
                        "maker_declared_uncertainty_not_truth_probability"
                    ),
                },
                "status": "proposed",
                "status_reason": (
                    "The two exact spans are proposed as an ordinal structural "
                    "correspondence only. Translation fidelity, wording, grammar, lexical "
                    "equivalence, and semantic interpretation remain unreviewed."
                ),
                "maker": {
                    "maker_kind": "software",
                    "agent_ref": (
                        "software:tos-zarathustra-opening-sentence-alignment-builder"
                    ),
                    "made_at": plan["created_at"],
                    "method": plan["method"]["name"],
                    "provenance_event_ref": EVENT_ID,
                    "method_output_posture": "proposal_not_truth",
                    "software_ref": BUILDER_REF,
                    "software_version": plan["method"]["version"],
                    "configuration_sha256": plan_digest,
                },
                "evidence": [
                    {
                        "evidence_ref": PLAN_REF,
                        "role": "method_input",
                        "source_anchor_refs": [source_anchor],
                        "target_anchor_refs": [target_anchor],
                        "description": (
                            "The tracked text-free plan freezes the exact selection and "
                            "ordinal-pairing rule, selectors, digests, and authority limits."
                        ),
                    },
                    {
                        "evidence_ref": plan["source"][
                            "edition_reading_admission_ref"
                        ],
                        "role": "qualification",
                        "source_anchor_refs": [source_anchor],
                        "target_anchor_refs": [],
                        "description": (
                            "The source expression has bounded edition-reading admission; "
                            "this does not accept the sentence segmentation or alignment."
                        ),
                    },
                    {
                        "evidence_ref": plan["target"]["expression_record_ref"],
                        "role": "support",
                        "source_anchor_refs": [],
                        "target_anchor_refs": [target_anchor],
                        "description": (
                            "The target expression record establishes the independently "
                            "modeled Russian expression identity."
                        ),
                    },
                    {
                        "evidence_ref": plan["target"][
                            "responsibility_claims_ref"
                        ],
                        "role": "support",
                        "source_anchor_refs": [source_anchor],
                        "target_anchor_refs": [target_anchor],
                        "description": (
                            "The responsibility record supplies the translated-by lineage; "
                            "it does not prove this sentence correspondence or its quality."
                        ),
                    },
                    {
                        "evidence_ref": plan["target"][
                            "unresolved_spacing_annotation_ref"
                        ],
                        "role": "qualification",
                        "source_anchor_refs": [],
                        "target_anchor_refs": [target_anchor],
                        "description": (
                            "The target raw layer retains an unresolved embedded-text versus "
                            "visual-spacing discrepancy; the proposal does not resolve it."
                        ),
                    },
                ],
                "competing_alignment_refs": [],
                "review_refs": [],
            }
        ],
        "reviews": [],
        "projections": [],
        "rights_and_visibility": {
            "source_visibility": "local_only",
            "target_visibility": "local_only",
            "packet_visibility": "public_metadata_only",
            "effective_visibility": "local_only",
            "rights_record_refs": [
                plan["source"]["rights_ref"],
                plan["target"]["rights_ref"],
            ],
            "private_source_used": True,
            "publication_authorized": False,
            "inheritance_policy": "most_restrictive_side_or_packet_wins",
        },
        "authority_boundary": {
            "tree_role": "orientation",
            "graph_role": "relation",
            "source_role": "authority",
            "validators_prove_mechanics_not_truth": True,
            "alignment_is_claim_not_translation_truth": True,
            "machine_or_model_may_propose_not_accept": True,
            "exports_are_not_owner_truth": True,
            "unaligned_members_are_first_class": True,
            "lexical_equivalence_not_inferred": True,
            "canon_effect": False,
        },
    }


def _tracked_entity(
    *, repo_root: Path, ref: str, digest: str, created_at: str, role: str
) -> dict[str, Any]:
    path = repo_root / ref
    return {
        "entity_ref": ref,
        "role": role,
        "sha256": digest,
        "size_bytes": path.stat().st_size,
        "media_type": "application/json",
        "availability": "tracked",
        "content_disclosure": "public_metadata_only",
        "fixity_verified": True,
        "fixity_verified_at": created_at,
    }


def _provenance_event(
    *,
    repo_root: Path,
    local_input_root: Path,
    plan: dict[str, Any],
    plan_digest: str,
    output_records: dict[str, bytes],
) -> dict[str, Any]:
    created_at = plan["created_at"]
    builder_digest = _sha256(repo_root / BUILDER_REF)
    executable = Path(sys.executable).resolve()
    argv = [
        "python",
        BUILDER_REF,
        "--build",
        "--local-input-root",
        "/srv/AbyssOS/Tree-of-Sophia",
    ]
    inputs: list[dict[str, Any]] = []
    for side_name in ("source", "target"):
        side = plan[side_name]
        private_path = local_input_root / side["private_content_ref"]
        inputs.append(
            {
                "entity_ref": side["private_content_ref"],
                "role": f"ignored-local-{side_name}-paragraph-text-layer",
                "sha256": side["text_layer_sha256"],
                "size_bytes": private_path.stat().st_size,
                "media_type": "text/plain; charset=utf-8",
                "availability": "ignored_local",
                "content_disclosure": "private_content",
                "fixity_verified": True,
                "fixity_verified_at": created_at,
            }
        )
    tracked_input_specs = [
        (PLAN_REF, plan_digest, "tracked-text-free-alignment-plan"),
        (
            plan["source"]["text_layer_ref"],
            plan["source"]["text_layer_record_sha256"],
            "source-text-layer-record",
        ),
        (
            plan["target"]["text_layer_ref"],
            plan["target"]["text_layer_record_sha256"],
            "target-text-layer-record",
        ),
        (
            plan["source"]["layout_packet_ref"],
            plan["source"]["layout_packet_sha256"],
            "source-layout-packet",
        ),
        (
            plan["target"]["layout_packet_ref"],
            plan["target"]["layout_packet_sha256"],
            "target-layout-packet",
        ),
        (
            plan["source"]["edition_reading_admission_ref"],
            plan["source"]["edition_reading_admission_sha256"],
            "source-edition-reading-admission",
        ),
        (
            plan["target"]["expression_record_ref"],
            plan["target"]["expression_record_sha256"],
            "target-expression-record",
        ),
        (
            plan["target"]["responsibility_claims_ref"],
            plan["target"]["responsibility_claims_sha256"],
            "target-translated-by-responsibility-claims",
        ),
    ]
    for ref, digest, role in tracked_input_specs:
        inputs.append(
            _tracked_entity(
                repo_root=repo_root,
                ref=ref,
                digest=digest,
                created_at=created_at,
                role=role,
            )
        )

    outputs = [
        {
            "entity_ref": ref,
            "role": "tracked-text-free-source-target-or-alignment-record",
            "sha256": _sha256_bytes(payload),
            "size_bytes": len(payload),
            "media_type": "application/json",
            "availability": "tracked",
            "content_disclosure": "public_metadata_only",
            "fixity_verified": True,
            "fixity_verified_at": created_at,
        }
        for ref, payload in output_records.items()
    ]
    derivations: list[dict[str, Any]] = []
    ordinal = 0
    for output in outputs:
        for side_name in ("source", "target"):
            ordinal += 1
            derivations.append(
                {
                    "derivation_id": (
                        "tos.derivation.za-i-vorrede-1.opening-sentence-alignment."
                        f"{ordinal}"
                    ),
                    "input_entity_ref": plan[side_name]["private_content_ref"],
                    "output_entity_ref": output["entity_ref"],
                    "relation": "aggregation_from",
                    "influence_asserted": True,
                    "description": (
                        "The exact private layer contributes only its fixed selector and "
                        "digest to this text-free proposal; no source text is disclosed."
                    ),
                }
            )

    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "provenance-event-v2.schema.json"
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
            "event_type": "alignment",
            "started_at": created_at,
            "ended_at": created_at,
            "status": "completed_with_warnings",
            "terminal_reason": None,
            "exit_code": 0,
            "warnings": [
                "Both sentence segmentations and their one-to-one mapping are unreviewed proposals.",
                "The target layer retains an unresolved embedded-text versus visual-spacing discrepancy.",
                "No translation fidelity, lexical equivalence, semantics, graph truth, canon authority, or publication permission is established.",
                "The unsigned self-recorded event proves mechanics and closure, not execution or content truth.",
            ],
        },
        "entities": {"inputs": inputs, "outputs": outputs, "byproducts": []},
        "derivations": derivations,
        "responsibility": [
            {
                "agent_ref": (
                    "software:tos-zarathustra-opening-sentence-alignment-builder"
                ),
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
                "name": plan["method"]["name"],
                "version": plan["method"]["version"],
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
                    "name": "Tree of Sophia opening-sentence alignment builder",
                    "version": plan["method"]["version"],
                    "role": "sentence-selection-and-alignment-proposal-builder",
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
                "backend": "python-standard-library-exact-code-point-selection",
                "hardware_target": "cpu",
                "unicode_version": unicodedata.unidata_version,
                "environment_profile_binding": {
                    "ref": PLAN_REF,
                    "sha256": plan_digest,
                },
            },
        },
        "manual_changes": {
            "status": "none_declared",
            "change_receipts": [],
            "statement": (
                "No manual change was applied to either private source span; the builder "
                "emits only selectors, digests, identities, provenance, and closed authority."
            ),
        },
        "measurements": [
            {
                "metric": "input_bytes",
                "status": "measured",
                "value": sum(row["size_bytes"] for row in inputs),
                "unit": "bytes",
                "method": "sum of exact fixity-bound input entity byte counts",
                "evidence_binding": None,
            },
            {
                "metric": "output_bytes",
                "status": "measured",
                "value": sum(row["size_bytes"] for row in outputs),
                "unit": "bytes",
                "method": "sum of exact serialized tracked output byte counts",
                "evidence_binding": None,
            },
            {
                "metric": "human_active_seconds",
                "status": "not_applicable",
                "value": None,
                "unit": None,
                "method": "no human content review or adjudication was scheduled or performed",
                "evidence_binding": None,
            },
        ],
        "evidence_authentication": {
            "capture_posture": "tool_captured",
            "signature_status": "unsigned",
            "signature_bindings": [],
            "verification_status": "mechanically_verified",
            "producer_control_boundary": (
                "The same local builder read the private layers and emitted this unsigned "
                "record; independent replay can verify bytes and selectors but cannot "
                "authenticate execution, language correctness, or translation fidelity."
            ),
        },
        "rights_and_visibility": {
            "rights_record_bindings": [
                {
                    "ref": plan["source"]["rights_ref"],
                    "sha256": plan["source"]["rights_sha256"],
                },
                {
                    "ref": plan["target"]["rights_ref"],
                    "sha256": plan["target"]["rights_sha256"],
                },
            ],
            "intended_uses": ["local_research", "indexing"],
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
                "Neither sentence segmentation nor their correspondence has human adjudication.",
                "The German machine layer remains unaccepted and the Russian OCR layer retains unresolved spacing.",
                "Both exact private paragraph layers are intentionally absent from Git.",
            ],
            "replay_scope": (
                "Exact private layer digests, code-point selectors, first-full-stop guards, "
                "tracked input bindings, deterministic packet serialization, CPython runtime "
                "digest, and fail-closed authority posture."
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


def _validate_schema(
    repo_root: Path, schema_ref: str, payload: dict[str, Any]
) -> None:
    schema = _load_json(repo_root / schema_ref)
    errors = sorted(
        Draft202012Validator(
            schema, format_checker=FormatChecker()
        ).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{'/'.join(str(part) for part in error.absolute_path) or '<root>'}: {error.message}"
            for error in errors[:16]
        )
        raise OpeningSentenceAlignmentError(
            f"{schema_ref} validation failed: {details}"
        )


def _semantic_checks(
    *, source_packet: dict[str, Any], target_packet: dict[str, Any],
    alignment: dict[str, Any], event: dict[str, Any], source_text: str,
    target_text: str,
) -> None:
    sys.path.insert(0, str(REPO_ROOT / "scripts"))
    import validate_source_witness_foundation as foundation  # noqa: PLC0415

    checks = [
        (
            "source sentence units",
            foundation._source_text_unit_v1_issues(
                source_packet, text=source_text
            ),
        ),
        (
            "target sentence units",
            foundation._source_text_unit_v1_issues(
                target_packet, text=target_text
            ),
        ),
        ("alignment", foundation._translation_alignment_v1_issues(alignment)),
        ("provenance", foundation._provenance_v2_semantic_issues(event)),
    ]
    failures = [f"{label}: {row}" for label, rows in checks for row in rows]
    if failures:
        raise OpeningSentenceAlignmentError("; ".join(failures[:24]))


def _expected(
    *, repo_root: Path, local_input_root: Path
) -> tuple[dict[str, bytes], dict[str, Any]]:
    plan_path = repo_root / PLAN_REF
    plan = _load_json(plan_path)
    plan_digest = _sha256(plan_path)
    source_text, source_sentence, source_remainder = _read_side(
        repo_root=repo_root,
        local_input_root=local_input_root,
        side_name="source",
        side=plan["source"],
    )
    target_text, target_sentence, target_remainder = _read_side(
        repo_root=repo_root,
        local_input_root=local_input_root,
        side_name="target",
        side=plan["target"],
    )
    source_packet = _unit_packet(
        plan=plan,
        side_name="source",
        text=source_text,
        sentence=source_sentence,
        remainder=source_remainder,
    )
    target_packet = _unit_packet(
        plan=plan,
        side_name="target",
        text=target_text,
        sentence=target_sentence,
        remainder=target_remainder,
    )
    source_ref = plan["outputs"]["source_sentence_packet_ref"]
    target_ref = plan["outputs"]["target_sentence_packet_ref"]
    source_bytes = _json_bytes(source_packet)
    target_bytes = _json_bytes(target_packet)
    alignment = _alignment_packet(
        plan=plan,
        plan_digest=plan_digest,
        source_packet_ref=source_ref,
        source_packet_digest=_sha256_bytes(source_bytes),
        target_packet_ref=target_ref,
        target_packet_digest=_sha256_bytes(target_bytes),
    )
    alignment_bytes = _json_bytes(alignment)
    records = {
        source_ref: source_bytes,
        target_ref: target_bytes,
        plan["outputs"]["alignment_packet_ref"]: alignment_bytes,
    }
    event = _provenance_event(
        repo_root=repo_root,
        local_input_root=local_input_root,
        plan=plan,
        plan_digest=plan_digest,
        output_records=records,
    )
    event_bytes = _json_bytes(event)
    records[plan["outputs"]["provenance_event_ref"]] = event_bytes

    _validate_schema(repo_root, UNIT_SCHEMA_REF, source_packet)
    _validate_schema(repo_root, UNIT_SCHEMA_REF, target_packet)
    _validate_schema(repo_root, ALIGNMENT_SCHEMA_REF, alignment)
    _validate_schema(repo_root, PROVENANCE_SCHEMA_REF, event)
    _semantic_checks(
        source_packet=source_packet,
        target_packet=target_packet,
        alignment=alignment,
        event=event,
        source_text=source_text,
        target_text=target_text,
    )
    tracked_text = b"\n".join(records.values()) + plan_path.read_bytes()
    for private_value in (
        source_text,
        source_sentence,
        target_text,
        target_sentence,
    ):
        if private_value.encode("utf-8") in tracked_text:
            raise OpeningSentenceAlignmentError(
                "tracked plan or output exposes private source text"
            )
    return records, plan


def run(
    *, repo_root: Path, local_input_root: Path, build: bool
) -> dict[str, Any]:
    records, plan = _expected(
        repo_root=repo_root, local_input_root=local_input_root
    )
    if build:
        for ref, payload in records.items():
            path = repo_root / ref
            if path.exists() and path.is_symlink():
                raise OpeningSentenceAlignmentError(
                    f"tracked output path is symlinked: {ref}"
                )
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(payload)
    for ref, payload in records.items():
        path = repo_root / ref
        if path.is_symlink() or not path.is_file() or path.read_bytes() != payload:
            raise OpeningSentenceAlignmentError(
                f"tracked output is absent or stale: {ref}"
            )
    return {
        "status": "passed",
        "question": plan["question"],
        "source_sentence_selector": [
            plan["source"]["sentence_start"],
            plan["source"]["sentence_end"],
        ],
        "source_sentence_sha256": plan["source"]["sentence_sha256"],
        "target_sentence_selector": [
            plan["target"]["sentence_start"],
            plan["target"]["sentence_end"],
        ],
        "target_sentence_sha256": plan["target"]["sentence_sha256"],
        "tracked_record_count": len(records),
        "sentence_segmentations": "proposed",
        "alignment_status": "proposed",
        "human_review_performed": False,
        "translation_fidelity_established": False,
        "lexical_equivalence_established": False,
        "semantic_or_canon_effect": False,
        "publication_authorized": False,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument("--local-input-root", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    try:
        report = run(
            repo_root=args.repo_root.resolve(),
            local_input_root=args.local_input_root.resolve(),
            build=args.build,
        )
    except OpeningSentenceAlignmentError as exc:
        print(f"opening-sentence alignment: FAIL: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

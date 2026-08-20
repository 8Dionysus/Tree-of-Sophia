#!/usr/bin/env python3
"""Build the public-synthetic semantic identity/annotation v2 A/B/C lab.

The fixtures exercise contract mechanics only. They do not record a model run,
human review, source interpretation, stable sign, concept, or graph truth.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
LAB_ROOT = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "semantic-annotation-v2-abc"
)
SCHEMA_REF = "ToS/contracts/semantic-annotation-packet-v2.schema.json"
SCRIPT_REF = "scripts/build_semantic_annotation_v2_lab.py"
RESEARCH_REF = (
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "SEMANTIC_IDENTITY_ANNOTATION_RESEARCH_2026-08-11.md"
)
SOURCE_REF = (LAB_ROOT / "public-synthetic-source.txt").as_posix()
PLAN_REF = (LAB_ROOT / "plan.json").as_posix()
MANIFEST_REF = (LAB_ROOT / "lab.manifest.json").as_posix()
VARIANT_REFS = {
    "A": (LAB_ROOT / "variant-a-occurrences-only.json").as_posix(),
    "B": (LAB_ROOT / "variant-b-competing-sign-proposals.json").as_posix(),
    "C": (LAB_ROOT / "variant-c-invalid-model-promotion.json").as_posix(),
}
SOURCE_TEXT = (
    "At dawn, the flame kept the record of a promise.\n"
    "At dusk, the flame became a question rather than an answer.\n"
)
FIXED_AT = "2026-08-11T12:00:00Z"


def _path(ref: str) -> Path:
    return REPO_ROOT / ref


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _sha256(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _sha256_text(value: str) -> str:
    return _sha256_bytes(value.encode("utf-8"))


def _sid(kind: str, seed: str) -> str:
    digest = _sha256_text(f"tos-semantic-v2:{kind}:{seed}")[:32]
    return f"tos.{kind}.sid-{digest}"


def _write_json(ref: str, payload: dict[str, Any]) -> None:
    path = _path(ref)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _binding(ref: str) -> dict[str, str]:
    return {"ref": ref, "sha256": _sha256(_path(ref))}


def _anchor(number: int, start: int) -> dict[str, Any]:
    exact = "flame"
    return {
        "anchor_ref": f"tos.anchor.semantic-v2.synthetic-flame-{number}",
        "source_ref": SOURCE_REF,
        "source_sha256": _sha256_text(SOURCE_TEXT),
        "selector": {
            "type": "text_position",
            "start": start,
            "end": start + len(exact),
            "position_unit": "unicode_code_point",
            "interval": "half_open",
        },
        "exact_sha256": _sha256_text(exact),
        "page_return": {"required": True, "locator_ref": SOURCE_REF},
    }


def _occurrence(number: int) -> dict[str, Any]:
    return {
        "entity_id": f"tos.occurrence.semantic-v2.synthetic-flame-{number}",
        "entity_kind": "occurrence",
        "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
        "admission_status": "observed",
        "identity_basis": {
            "anchor_refs": [f"tos.anchor.semantic-v2.synthetic-flame-{number}"],
            "claim_refs": [],
            "parent_entity_refs": [],
        },
        "display_labels": [
            {"language": "en", "value": "flame", "role": "display", "mutable": True}
        ],
        "admission_review_refs": [],
        "entity_version": 1,
        "supersedes_entity_ref": None,
    }


def _fixture_maker(simulates_kind: str) -> dict[str, Any]:
    return {
        "maker_kind": "synthetic_fixture",
        "agent_ref": "software:tos-semantic-annotation-v2-lab-builder",
        "made_at": FIXED_AT,
        "method": "deterministic public-synthetic contract fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "simulates_kind": simulates_kind,
        "simulation_notice": "synthetic fixture only; no corresponding human, software, or model act occurred",
    }


def _observation_claim(number: int) -> dict[str, Any]:
    occurrence_ref = f"tos.occurrence.semantic-v2.synthetic-flame-{number}"
    anchor_ref = f"tos.anchor.semantic-v2.synthetic-flame-{number}"
    return {
        "claim_id": _sid("claim", f"occurrence-{number}-exact-form"),
        "claim_version": 1,
        "supersedes_claim_ref": None,
        "claim_type": "textual_observation",
        "stage": "exact_form",
        "proposition": {
            "subject_ref": occurrence_ref,
            "predicate": "has_exact_form_digest",
            "object": {
                "kind": "digest",
                "sha256": _sha256_text("flame"),
                "media_type": "text/plain; charset=utf-8",
            },
        },
        "target_anchor_refs": [anchor_ref],
        "evidence": [
            {
                "evidence_ref": SOURCE_REF,
                "role": "support",
                "direction": "supports",
                "anchor_refs": [anchor_ref],
                "description": "Exact public-synthetic source span bound by digest and position selector.",
            }
        ],
        "maker": _fixture_maker("software"),
        "epistemic_status": "observed",
        "certainty": {
            "value": 1.0,
            "meaning": "maker_declared_uncertainty_not_truth_probability",
        },
        "claim_status": "observed",
        "status_reason": "Synthetic occurrence observation exercises exact-form mechanics only.",
        "competing_claim_refs": [],
        "review_refs": [],
    }


def _sign_entity(slot: str, claim_ref: str, label: str) -> dict[str, Any]:
    return {
        "entity_id": _sid("sign", f"issued-{slot}"),
        "entity_kind": "sign",
        "identity_policy": "opaque-id-independent-of-label-gloss-translation-and-current-interpretation",
        "admission_status": "proposed",
        "identity_basis": {
            "anchor_refs": [
                "tos.anchor.semantic-v2.synthetic-flame-1",
                "tos.anchor.semantic-v2.synthetic-flame-2",
            ],
            "claim_refs": [claim_ref],
            "parent_entity_refs": [
                "tos.occurrence.semantic-v2.synthetic-flame-1",
                "tos.occurrence.semantic-v2.synthetic-flame-2",
            ],
        },
        "display_labels": [
            {"language": "en", "value": label, "role": "editorial", "mutable": True}
        ],
        "admission_review_refs": [],
        "entity_version": 1,
        "supersedes_entity_ref": None,
    }


def _sign_claim(slot: str, competing_ref: str, description: str) -> dict[str, Any]:
    claim_ref = _sid("claim", f"issued-sign-claim-{slot}")
    return {
        "claim_id": claim_ref,
        "claim_version": 1,
        "supersedes_claim_ref": None,
        "claim_type": "sign_identity",
        "stage": "stable_sign_candidate",
        "proposition": {
            "subject_ref": _sid("sign", f"issued-{slot}"),
            "predicate": "groups_occurrences_as_candidate_sign",
            "object": {
                "kind": "entity_set",
                "entity_refs": [
                    "tos.occurrence.semantic-v2.synthetic-flame-1",
                    "tos.occurrence.semantic-v2.synthetic-flame-2",
                ],
                "description": description,
            },
        },
        "target_anchor_refs": [
            "tos.anchor.semantic-v2.synthetic-flame-1",
            "tos.anchor.semantic-v2.synthetic-flame-2",
        ],
        "evidence": [
            {
                "evidence_ref": SOURCE_REF,
                "role": "context",
                "direction": "does_not_decide",
                "anchor_refs": [
                    "tos.anchor.semantic-v2.synthetic-flame-1",
                    "tos.anchor.semantic-v2.synthetic-flame-2",
                ],
                "description": "The same two source spans permit competing synthetic readings; recurrence alone does not decide identity.",
            }
        ],
        "maker": _fixture_maker("model"),
        "epistemic_status": "interpreted",
        "certainty": {
            "value": 0.55,
            "meaning": "maker_declared_uncertainty_not_truth_probability",
        },
        "claim_status": "proposed",
        "status_reason": "Model-shaped synthetic proposal remains unreviewed and non-authoritative.",
        "competing_claim_refs": [competing_ref],
        "review_refs": [],
    }


def _base_packet(variant: str) -> dict[str, Any]:
    first = SOURCE_TEXT.index("flame")
    second = SOURCE_TEXT.index("flame", first + 1)
    occurrences = [_occurrence(1), _occurrence(2)]
    observations = [_observation_claim(1), _observation_claim(2)]
    packet: dict[str, Any] = {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/semantic-annotation-packet-v2.schema.json",
        "schema_version": "tos_semantic_annotation_packet_v2",
        "annotation_id": _sid("annotation", f"variant-{variant.lower()}"),
        "annotation_version": 1,
        "supersedes_annotation_ref": None,
        "content_posture": "public_synthetic_contract_exercise",
        "source_scope": {
            "work_ref": "tos.work.semantic-v2.synthetic-miniature",
            "expression_ref": "tos.expression.semantic-v2.synthetic-miniature.en",
            "item_ref": "tos.item.semantic-v2.synthetic-miniature.tracked",
            "file_ref": "tos.file.semantic-v2.synthetic-miniature.utf8",
            "file_sha256": _sha256_text(SOURCE_TEXT),
            "source_text_layer_ref": SOURCE_REF,
            "source_anchors": [_anchor(1, first), _anchor(2, second)],
            "rights_refs": [PLAN_REF],
        },
        "entities": occurrences,
        "claims": observations,
        "relations": [],
        "reviews": [],
        "graph_projection": {
            "posture": "downstream-derived-projection-only",
            "projection_event_refs": [],
            "node_refs": [],
            "edges": [],
            "source_return_required": True,
        },
        "rights_and_visibility": {
            "source_content_visibility": "public_synthetic",
            "record_visibility": "public",
            "publication_authorized": True,
            "private_source_used": False,
            "rights_basis_refs": [PLAN_REF],
        },
        "authority_boundary": {
            "tree_role": "orientation",
            "graph_role": "relation",
            "source_role": "authority",
            "validators_prove_mechanics_not_truth": True,
            "frequency_is_semantic_evidence_not_semantic_proof": True,
            "model_may_propose_but_not_promote_stable_sign": True,
            "routine_human_review_per_occurrence": False,
            "promotion_review_posture": "rare-triggered-source-visible-human-checkpoint",
        },
    }
    return packet


def _variant_b() -> dict[str, Any]:
    packet = _base_packet("B")
    first_claim = _sid("claim", "issued-sign-claim-slot-a")
    second_claim = _sid("claim", "issued-sign-claim-slot-b")
    packet["entities"].extend(
        [
            _sign_entity("slot-a", first_claim, "flame as continuity"),
            _sign_entity("slot-b", second_claim, "flame as transformation"),
        ]
    )
    packet["claims"].extend(
        [
            _sign_claim(
                "slot-a",
                second_claim,
                "Synthetic reading A treats both occurrences as one sign of continuity.",
            ),
            _sign_claim(
                "slot-b",
                first_claim,
                "Synthetic reading B treats the later occurrence as transforming the earlier sign.",
            ),
        ]
    )
    return packet


def _plan() -> dict[str, Any]:
    return {
        "schema_version": "tos_semantic_annotation_v2_lab_plan_v1",
        "question": "Can ToS keep occurrence, sign proposal, competing reading, review, and graph admission identities separate and fail closed?",
        "input_posture": "public_synthetic_only",
        "variants": [
            {"variant_id": "A", "purpose": "Exact source occurrences only", "expected_schema_valid": True, "expected_semantic_valid": True},
            {"variant_id": "B", "purpose": "Two reciprocal competing sign proposals without review or graph effect", "expected_schema_valid": True, "expected_semantic_valid": True},
            {"variant_id": "C", "purpose": "Invalid model-shaped promotion without a real-human review", "expected_schema_valid": False, "expected_semantic_valid": False},
        ],
        "negative_controls": [
            "label-derived-id",
            "duplicate-entity-id",
            "missing-target-anchor",
            "unresolved-entity-reference",
            "one-way-competing-claim",
            "accepted-model-claim-without-review",
            "sign-promotion-without-baseline-or-competence",
            "accepted-relation-without-accepted-claim",
            "graph-projection-of-proposed-claim",
            "relation-proposition-mismatch",
            "self-supersession",
            "publication-boundary-widening",
        ],
        "authority_limits": {
            "private_source_used": False,
            "model_invoked": False,
            "human_review_performed": False,
            "stable_sign_established": False,
            "concept_established": False,
            "graph_truth_established": False,
            "semantic_truth_established": False,
            "canon_effect": False,
        },
    }


def build() -> None:
    source_path = _path(SOURCE_REF)
    source_path.parent.mkdir(parents=True, exist_ok=True)
    source_path.write_text(SOURCE_TEXT, encoding="utf-8", newline="")
    _write_json(PLAN_REF, _plan())

    variant_a = _base_packet("A")
    variant_b = _variant_b()
    variant_c = json.loads(json.dumps(variant_b))
    variant_c["annotation_id"] = _sid("annotation", "variant-c")
    continuity_sign = next(
        entity
        for entity in variant_c["entities"]
        if entity["entity_id"] == _sid("sign", "issued-slot-a")
    )
    continuity_sign["admission_status"] = "accepted"
    continuity_claim = next(
        claim
        for claim in variant_c["claims"]
        if claim["claim_id"] == _sid("claim", "issued-sign-claim-slot-a")
    )
    continuity_claim["claim_status"] = "accepted"
    continuity_claim["status_reason"] = "Deliberately invalid: synthetic model-shaped proposal attempts promotion without human review."

    for variant_id, payload in (("A", variant_a), ("B", variant_b), ("C", variant_c)):
        _write_json(VARIANT_REFS[variant_id], payload)

    manifest = {
        "schema_version": "tos_semantic_annotation_v2_lab_manifest_v1",
        "authority_posture": "public_synthetic_contract_mechanics_only",
        "contract": _binding(SCHEMA_REF),
        "research": _binding(RESEARCH_REF),
        "plan": _binding(PLAN_REF),
        "builder": _binding(SCRIPT_REF),
        "input_fixture": _binding(SOURCE_REF),
        "variants": [
            {
                "variant_id": variant_id,
                "packet_ref": VARIANT_REFS[variant_id],
                "packet_sha256": _sha256(_path(VARIANT_REFS[variant_id])),
                "expected_schema_valid": variant_id != "C",
                "expected_semantic_valid": variant_id != "C",
            }
            for variant_id in ("A", "B", "C")
        ],
        "authority_limits": _plan()["authority_limits"],
    }
    _write_json(MANIFEST_REF, manifest)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--build", action="store_true", help="build all deterministic lab fixtures")
    args = parser.parse_args()
    if not args.build:
        parser.error("use --build")
    build()
    print(f"prepared {MANIFEST_REF}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Build the public-synthetic translation-alignment v1 A/B/C laboratory.

The fixtures exercise identity, anchor, claim, review, projection, and rights
mechanics only. No corresponding translation, model, software aligner, or
human review act occurred.
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
    "translation-alignment-v1-abc"
)
SCHEMA_REF = "ToS/contracts/translation-alignment-packet-v1.schema.json"
SCRIPT_REF = "scripts/build_translation_alignment_v1_lab.py"
RESEARCH_REF = (
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "TRANSLATION_ALIGNMENT_IDENTITY_RESEARCH_2026-08-11.md"
)
PLAN_REF = (LAB_ROOT / "plan.json").as_posix()
MANIFEST_REF = (LAB_ROOT / "lab.manifest.json").as_posix()
SOURCE_REF = (LAB_ROOT / "public-synthetic-source.x-tos-src.txt").as_posix()
TARGET_REF = (LAB_ROOT / "public-synthetic-target.x-tos-tgt.txt").as_posix()
SOURCE_SEGMENTATION_REF = (LAB_ROOT / "source-segmentation.json").as_posix()
TARGET_SEGMENTATION_REF = (LAB_ROOT / "target-segmentation.json").as_posix()
SOURCE_TOKENIZATION_REF = (LAB_ROOT / "source-tokenization.json").as_posix()
TARGET_TOKENIZATION_REF = (LAB_ROOT / "target-tokenization.json").as_posix()
VARIANT_REFS = {
    "A": (LAB_ROOT / "variant-a-one-to-one-proposal.json").as_posix(),
    "B": (LAB_ROOT / "variant-b-competing-mappings.json").as_posix(),
    "C": (LAB_ROOT / "variant-c-invalid-acceptance.json").as_posix(),
}
SOURCE_TEXT = "ember-one remains.\nember-two turns.\n"
TARGET_TEXT = "echo-one stays.\necho-two shifts and opens.\n"
FIXED_AT = "2026-08-11T14:00:00Z"


def _path(ref: str) -> Path:
    return REPO_ROOT / ref


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _sha256_text(value: str) -> str:
    return _sha256_bytes(value.encode("utf-8"))


def _sha256(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _sid(kind: str, slot: str) -> str:
    digest = _sha256_text(f"tos-translation-alignment-v1:{kind}:issued-{slot}")[:32]
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


def _anchor(
    *,
    side: str,
    slot: str,
    ordinal: int,
    text: str,
    exact: str,
    layer_ref: str,
) -> dict[str, Any]:
    start = text.index(exact)
    return {
        "anchor_ref": f"tos.anchor.translation-alignment-v1.synthetic-{side}-{slot}",
        "ordinal": ordinal,
        "text_layer_ref": layer_ref,
        "text_layer_sha256": _sha256_text(text),
        "selector": {
            "type": "text_position",
            "start": start,
            "end": start + len(exact),
            "position_unit": "unicode_code_point",
            "interval": "half_open",
        },
        "exact_sha256": _sha256_text(exact),
        "source_return": {"required": True, "locator_ref": layer_ref},
    }


def _analysis_payload(*, side: str, kind: str, text_ref: str, text: str) -> dict[str, Any]:
    return {
        "schema_version": "tos_translation_alignment_synthetic_analysis_v1",
        "side": side,
        "analysis_kind": kind,
        "source_ref": text_ref,
        "source_sha256": _sha256_text(text),
        "state": "frozen",
        "authority": "synthetic mechanical fixture only",
    }


def _maker(simulates_kind: str) -> dict[str, Any]:
    return {
        "maker_kind": "synthetic_fixture",
        "agent_ref": "software:tos-translation-alignment-v1-lab-builder",
        "made_at": FIXED_AT,
        "method": "deterministic public-synthetic mapping fixture construction",
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "method_output_posture": "proposal_not_truth",
        "simulates_kind": simulates_kind,
        "simulation_notice": "synthetic fixture only; no corresponding human, software, model, translation, or review act occurred",
    }


def _side(*, role: str) -> dict[str, Any]:
    if role == "source":
        text = SOURCE_TEXT
        text_ref = SOURCE_REF
        language = "x-tos-src"
        segmentation_ref = SOURCE_SEGMENTATION_REF
        tokenization_ref = SOURCE_TOKENIZATION_REF
        anchors = [
            _anchor(
                side="source",
                slot="one",
                ordinal=1,
                text=text,
                exact="ember-one remains.",
                layer_ref=text_ref,
            ),
            _anchor(
                side="source",
                slot="two",
                ordinal=2,
                text=text,
                exact="ember-two turns.",
                layer_ref=text_ref,
            ),
        ]
    else:
        text = TARGET_TEXT
        text_ref = TARGET_REF
        language = "x-tos-tgt"
        segmentation_ref = TARGET_SEGMENTATION_REF
        tokenization_ref = TARGET_TOKENIZATION_REF
        anchors = [
            _anchor(
                side="target",
                slot="one",
                ordinal=1,
                text=text,
                exact="echo-one stays.",
                layer_ref=text_ref,
            ),
            _anchor(
                side="target",
                slot="two-whole",
                ordinal=2,
                text=text,
                exact="echo-two shifts and opens.",
                layer_ref=text_ref,
            ),
            _anchor(
                side="target",
                slot="two-part-a",
                ordinal=3,
                text=text,
                exact="echo-two shifts",
                layer_ref=text_ref,
            ),
            _anchor(
                side="target",
                slot="two-part-b",
                ordinal=4,
                text=text,
                exact="and opens.",
                layer_ref=text_ref,
            ),
        ]
    return {
        "side_role": role,
        "work_ref": "tos.work.translation-alignment-v1.synthetic-miniature",
        "expression_ref": f"tos.expression.translation-alignment-v1.synthetic-miniature.{role}",
        "edition_ref": f"tos.edition.translation-alignment-v1.synthetic-miniature.{role}",
        "item_ref": f"tos.item.translation-alignment-v1.synthetic-miniature.{role}",
        "file_ref": f"tos.file.translation-alignment-v1.synthetic-miniature.{role}.utf8",
        "file_sha256": _sha256_text(text),
        "text_layer_ref": text_ref,
        "text_layer_sha256": _sha256_text(text),
        "language": language,
        "segmentation": {
            "artifact_ref": segmentation_ref,
            "sha256": _sha256(_path(segmentation_ref)),
            "state": "frozen",
        },
        "tokenization": {
            "artifact_ref": tokenization_ref,
            "sha256": _sha256(_path(tokenization_ref)),
            "state": "frozen",
        },
        "anchors": anchors,
        "rights_refs": [PLAN_REF],
        "visibility": "public",
        "publication_authorized": True,
    }


def _alignment(
    *,
    slot: str,
    shape: str,
    source_anchor_refs: list[str],
    target_anchor_refs: list[str],
    competing_refs: list[str],
    simulates_kind: str,
) -> dict[str, Any]:
    return {
        "alignment_id": _sid("translation-alignment", slot),
        "alignment_version": 1,
        "supersedes_alignment_ref": None,
        "identity_policy": "opaque-id-independent-of-text-label-translation-and-current-mapping",
        "claim_id": _sid("translation-alignment-claim", slot),
        "claim_version": 1,
        "supersedes_claim_ref": None,
        "direction": "source_to_target",
        "correspondence_shape": shape,
        "order_posture": "monotonic",
        "ordered_source_anchor_refs": source_anchor_refs,
        "ordered_target_anchor_refs": target_anchor_refs,
        "translation_techniques": ["unresolved"],
        "epistemic_status": "observed_structure",
        "certainty": {
            "value": 0.5,
            "meaning": "maker_declared_uncertainty_not_truth_probability",
        },
        "status": "proposed",
        "status_reason": "Public-synthetic proposal exercises mapping mechanics and establishes no translation truth.",
        "maker": _maker(simulates_kind),
        "evidence": [
            {
                "evidence_ref": PLAN_REF,
                "role": "method_input",
                "source_anchor_refs": source_anchor_refs,
                "target_anchor_refs": target_anchor_refs,
                "description": "Exact invented spans are present only to exercise ordered stand-off mapping mechanics.",
            }
        ],
        "competing_alignment_refs": competing_refs,
        "review_refs": [],
    }


def _base_packet(variant: str) -> dict[str, Any]:
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-alignment-packet-v1.schema.json",
        "schema_version": "tos_translation_alignment_packet_v1",
        "packet_id": _sid("translation-alignment-packet", f"variant-{variant.lower()}"),
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "public_synthetic_contract_exercise",
        "granularity": "mixed",
        "source_side": _side(role="source"),
        "target_side": _side(role="target"),
        "alignments": [],
        "reviews": [],
        "projections": [],
        "rights_and_visibility": {
            "source_visibility": "public",
            "target_visibility": "public",
            "packet_visibility": "public",
            "effective_visibility": "public",
            "rights_record_refs": [PLAN_REF],
            "private_source_used": False,
            "publication_authorized": True,
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


def _variant_a() -> dict[str, Any]:
    packet = _base_packet("A")
    packet["alignments"] = [
        _alignment(
            slot="slot-a",
            shape="one_to_one",
            source_anchor_refs=["tos.anchor.translation-alignment-v1.synthetic-source-one"],
            target_anchor_refs=["tos.anchor.translation-alignment-v1.synthetic-target-one"],
            competing_refs=[],
            simulates_kind="software",
        )
    ]
    return packet


def _variant_b() -> dict[str, Any]:
    packet = _base_packet("B")
    one_to_one_ref = _sid("translation-alignment", "slot-b")
    one_to_many_ref = _sid("translation-alignment", "slot-c")
    packet["alignments"] = [
        _alignment(
            slot="slot-b",
            shape="one_to_one",
            source_anchor_refs=["tos.anchor.translation-alignment-v1.synthetic-source-two"],
            target_anchor_refs=["tos.anchor.translation-alignment-v1.synthetic-target-two-whole"],
            competing_refs=[one_to_many_ref],
            simulates_kind="model",
        ),
        _alignment(
            slot="slot-c",
            shape="one_to_many",
            source_anchor_refs=["tos.anchor.translation-alignment-v1.synthetic-source-two"],
            target_anchor_refs=[
                "tos.anchor.translation-alignment-v1.synthetic-target-two-part-a",
                "tos.anchor.translation-alignment-v1.synthetic-target-two-part-b",
            ],
            competing_refs=[one_to_one_ref],
            simulates_kind="model",
        ),
    ]
    return packet


def _plan() -> dict[str, Any]:
    return {
        "schema_version": "tos_translation_alignment_v1_lab_plan_v1",
        "question": "Can ToS bind exact bilingual sides and preserve competing mappings without turning a method output into translation truth?",
        "input_posture": "public_synthetic_only",
        "languages": ["x-tos-src", "x-tos-tgt"],
        "variants": [
            {
                "variant_id": "A",
                "purpose": "One deterministic one-to-one structural proposal",
                "expected_schema_valid": True,
                "expected_semantic_valid": True,
            },
            {
                "variant_id": "B",
                "purpose": "Reciprocal one-to-one and one-to-many competing proposals",
                "expected_schema_valid": True,
                "expected_semantic_valid": True,
            },
            {
                "variant_id": "C",
                "purpose": "Invalid synthetic/model-shaped acceptance without review or competence",
                "expected_schema_valid": False,
                "expected_semantic_valid": False,
            },
        ],
        "negative_controls": [
            "text-derived-alignment-id",
            "duplicate-alignment-id",
            "duplicate-claim-id",
            "missing-source-anchor",
            "missing-target-anchor",
            "source-layer-digest-drift",
            "target-layer-digest-drift",
            "naked-unresolved-anchor",
            "shape-cardinality-mismatch",
            "source-omission-with-target-members",
            "target-addition-with-source-members",
            "one-way-competing-mapping",
            "accepted-model-proposal-without-review",
            "accepted-without-language-competence",
            "projection-of-proposed-alignment",
            "projection-visibility-widening",
            "self-supersession",
        ],
        "authority_limits": {
            "private_source_used": False,
            "translation_performed": False,
            "model_invoked": False,
            "aligner_invoked": False,
            "human_review_performed": False,
            "accepted_alignment_established": False,
            "lexical_equivalence_established": False,
            "semantic_truth_established": False,
            "graph_truth_established": False,
            "canon_effect": False,
        },
    }


def build() -> None:
    for ref, text in ((SOURCE_REF, SOURCE_TEXT), (TARGET_REF, TARGET_TEXT)):
        path = _path(ref)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="")

    _write_json(
        SOURCE_SEGMENTATION_REF,
        _analysis_payload(side="source", kind="segmentation", text_ref=SOURCE_REF, text=SOURCE_TEXT),
    )
    _write_json(
        TARGET_SEGMENTATION_REF,
        _analysis_payload(side="target", kind="segmentation", text_ref=TARGET_REF, text=TARGET_TEXT),
    )
    _write_json(
        SOURCE_TOKENIZATION_REF,
        _analysis_payload(side="source", kind="tokenization", text_ref=SOURCE_REF, text=SOURCE_TEXT),
    )
    _write_json(
        TARGET_TOKENIZATION_REF,
        _analysis_payload(side="target", kind="tokenization", text_ref=TARGET_REF, text=TARGET_TEXT),
    )
    _write_json(PLAN_REF, _plan())

    variant_a = _variant_a()
    variant_b = _variant_b()
    variant_c = json.loads(json.dumps(variant_b))
    variant_c["packet_id"] = _sid("translation-alignment-packet", "variant-c")
    variant_c["alignments"][0]["status"] = "accepted"
    variant_c["alignments"][0]["status_reason"] = (
        "Deliberately invalid: a synthetic/model-shaped proposal attempts acceptance without a real-human review."
    )

    for variant_id, payload in (("A", variant_a), ("B", variant_b), ("C", variant_c)):
        _write_json(VARIANT_REFS[variant_id], payload)

    manifest = {
        "schema_version": "tos_translation_alignment_v1_lab_manifest_v1",
        "authority_posture": "public_synthetic_contract_mechanics_only",
        "contract": _binding(SCHEMA_REF),
        "research": _binding(RESEARCH_REF),
        "plan": _binding(PLAN_REF),
        "builder": _binding(SCRIPT_REF),
        "inputs": [_binding(SOURCE_REF), _binding(TARGET_REF)],
        "analysis_artifacts": [
            _binding(SOURCE_SEGMENTATION_REF),
            _binding(TARGET_SEGMENTATION_REF),
            _binding(SOURCE_TOKENIZATION_REF),
            _binding(TARGET_TOKENIZATION_REF),
        ],
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

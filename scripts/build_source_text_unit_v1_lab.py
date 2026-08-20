#!/usr/bin/env python3
"""Build the public-synthetic source-text-unit v1 A/B/C laboratory.

The fixtures exercise exact text-layer, scheme, unit, coverage, competition,
review, projection, and rights mechanics only. No real language boundary,
model run, imported analysis, or human review act occurred.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
LAB_ROOT = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "source-text-unit-v1-abc"
)
SCHEMA_REF = "ToS/contracts/source-text-unit-packet-v1.schema.json"
SCRIPT_REF = "scripts/build_source_text_unit_v1_lab.py"
RESEARCH_REF = (
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "SOURCE_TEXT_UNIT_SEGMENTATION_RESEARCH_2026-08-11.md"
)
PLAN_REF = (LAB_ROOT / "plan.json").as_posix()
MANIFEST_REF = (LAB_ROOT / "lab.manifest.json").as_posix()
SOURCE_REF = (LAB_ROOT / "public-synthetic-source.x-tos-unit.txt").as_posix()
VARIANT_REFS = {
    "A": (LAB_ROOT / "variant-a-source-layout-observation.json").as_posix(),
    "B": (LAB_ROOT / "variant-b-competing-segmentations.json").as_posix(),
    "C": (LAB_ROOT / "variant-c-invalid-acceptance.json").as_posix(),
}
SOURCE_TEXT = "mark A. ember-turns.\nmark B: ember turns.\n"
FIXED_AT = "2026-08-11T18:00:00Z"


def _path(ref: str) -> Path:
    return REPO_ROOT / ref


def _sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _sha256_text(value: str) -> str:
    return _sha256_bytes(value.encode("utf-8"))


def _sha256(path: Path) -> str:
    return _sha256_bytes(path.read_bytes())


def _sid(kind: str, slot: str) -> str:
    digest = _sha256_text(f"tos-source-text-unit-v1:{kind}:issued-{slot}")[:32]
    return f"tos.{kind}.sid-{digest}"


def _write_json(ref: str, payload: dict[str, Any]) -> None:
    path = _path(ref)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _method(
    *,
    method_name: str,
    method_version: str,
    maker_kind: str = "synthetic_fixture",
    model_ref: str | None = None,
) -> dict[str, Any]:
    return {
        "maker_kind": maker_kind,
        "agent_ref": "software:tos-source-text-unit-v1-lab-builder",
        "method_name": method_name,
        "method_version": method_version,
        "software_refs": [SCRIPT_REF],
        "model_ref": model_ref,
        "configuration_ref": PLAN_REF,
        "locale": "x-tos-unit",
        "unicode_version": "17.0.0",
        "unicode_revision": 47,
        "tailoring_ref": None,
        "provenance_event_ref": "synthetic:no-runtime-provenance-event-claimed",
        "made_at": FIXED_AT,
        "output_posture": "method_result_not_source_or_linguistic_truth",
    }


def _policies(
    *,
    punctuation: str,
    whitespace: str,
    line_break: str,
    overlap: str = "forbid",
) -> dict[str, Any]:
    return {
        "normalization": "no-text-mutation-separate-successor-layer",
        "punctuation": punctuation,
        "whitespace": whitespace,
        "line_break": line_break,
        "hyphenation": "preserve_source",
        "unreported_gaps_allowed": False,
        "overlap": overlap,
    }


def _scheme(
    *,
    slot: str,
    name: str,
    analysis_role: str,
    boundary_basis: str,
    unit_kinds: list[str],
    policies: dict[str, Any],
) -> dict[str, Any]:
    return {
        "scheme_id": _sid("text-unit-scheme", slot),
        "scheme_version": 1,
        "supersedes_scheme_ref": None,
        "identity_policy": (
            "opaque-id-independent-of-name-label-text-ordinal-offset-and-current-analysis"
        ),
        "scheme_name": name,
        "analysis_role": analysis_role,
        "boundary_basis": boundary_basis,
        "unit_kinds": unit_kinds,
        "method": _method(method_name=name, method_version="1"),
        "policies": policies,
        "authority_limit": {
            "algorithmic_output_is_source_truth": False,
            "algorithmic_output_is_linguistic_truth": False,
            "model_subword_is_lexeme": False,
            "unit_identity_is_semantic_identity": False,
            "text_mutation_allowed": False,
        },
    }


def _anchor(
    *, slot: str, ordinal: int, start: int, end: int, role: str
) -> dict[str, Any]:
    exact = SOURCE_TEXT[start:end]
    return {
        "anchor_ref": f"tos.anchor.source-text-unit-v1.synthetic-{slot}",
        "ordinal": ordinal,
        "text_layer_ref": SOURCE_REF,
        "text_layer_sha256": _sha256_text(SOURCE_TEXT),
        "selector": {
            "type": "text_position",
            "start": start,
            "end": end,
            "position_unit": "unicode_code_point",
            "interval": "half_open",
        },
        "exact_sha256": _sha256_text(exact),
        "anchor_role": role,
        "source_return": {"required": True, "locator_ref": SOURCE_REF},
    }


def _unit(
    *,
    slot: str,
    kind: str,
    anchor_ref: str,
    boundary_posture: str,
    certainty: float,
) -> dict[str, Any]:
    return {
        "unit_id": _sid("text-unit", slot),
        "unit_version": 1,
        "supersedes_unit_ref": None,
        "identity_policy": (
            "opaque-id-independent-of-text-label-ordinal-offset-and-current-analysis"
        ),
        "unit_kind": kind,
        "surface_posture": "source_bearing",
        "continuity": "contiguous",
        "ordered_anchor_refs": [anchor_ref],
        "parent_unit_refs": [],
        "ordered_child_unit_refs": [],
        "boundary_posture": boundary_posture,
        "certainty": {
            "value": certainty,
            "meaning": "maker-declared-boundary-confidence-not-truth-probability",
        },
        "status_reason": (
            "Public-synthetic unit exists only to exercise exact boundary mechanics."
        ),
        "source_text_mutated": False,
        "semantic_promotion": False,
    }


def _add_member(
    *,
    anchors: list[dict[str, Any]],
    units: list[dict[str, Any]],
    slot: str,
    start: int,
    end: int,
    kind: str,
    role: str,
    boundary_posture: str,
    certainty: float,
) -> str:
    anchor = _anchor(
        slot=slot,
        ordinal=len(anchors) + 1,
        start=start,
        end=end,
        role=role,
    )
    unit = _unit(
        slot=slot,
        kind=kind,
        anchor_ref=anchor["anchor_ref"],
        boundary_posture=boundary_posture,
        certainty=certainty,
    )
    anchors.append(anchor)
    units.append(unit)
    return unit["unit_id"]


def _segmentation(
    *,
    slot: str,
    scheme_ref: str,
    ordered_unit_refs: list[str],
    competing_refs: list[str],
    status: str,
    status_reason: str,
    uses: list[str],
    maker_kind: str = "synthetic_fixture",
    model_ref: str | None = None,
) -> dict[str, Any]:
    return {
        "segmentation_id": _sid("text-segmentation", slot),
        "segmentation_version": 1,
        "supersedes_segmentation_ref": None,
        "identity_policy": (
            "opaque-id-independent-of-scheme-name-unit-order-text-and-current-boundaries"
        ),
        "scheme_ref": scheme_ref,
        "ordered_unit_refs": ordered_unit_refs,
        "coverage": {
            "scope_anchor_ref": "tos.anchor.source-text-unit-v1.synthetic-scope",
            "coverage_posture": "exhaustive_nonoverlapping",
            "excluded_anchor_refs": [],
            "unreported_gaps_allowed": False,
            "overlap_requires_declaration": True,
            "source_reconstruction_required": True,
        },
        "status": status,
        "status_reason": status_reason,
        "maker": _method(
            method_name=f"public-synthetic segmentation result {slot}",
            method_version="1",
            maker_kind=maker_kind,
            model_ref=model_ref,
        ),
        "competing_segmentation_refs": competing_refs,
        "review_refs": [],
        "declared_uses": uses,
        "source_text_authority": False,
        "linguistic_authority": False,
        "semantic_authority": False,
    }


def _base_packet(variant: str) -> dict[str, Any]:
    scope_anchor = _anchor(
        slot="scope", ordinal=1, start=0, end=len(SOURCE_TEXT), role="scope"
    )
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "source-text-unit-packet-v1.schema.json"
        ),
        "schema_version": "tos_source_text_unit_packet_v1",
        "packet_id": _sid("source-text-unit-packet", f"variant-{variant.lower()}"),
        "packet_version": 1,
        "supersedes_packet_ref": None,
        "content_posture": "public_synthetic_contract_exercise",
        "source_scope": {
            "work_ref": "tos.work.source-text-unit-v1.synthetic-miniature",
            "expression_ref": "tos.expression.source-text-unit-v1.synthetic-miniature",
            "edition_ref": "tos.edition.source-text-unit-v1.synthetic-miniature",
            "item_ref": "tos.item.source-text-unit-v1.synthetic-miniature",
            "file_ref": "tos.file.source-text-unit-v1.synthetic-miniature.utf8",
            "file_sha256": _sha256_text(SOURCE_TEXT),
        },
        "source_layer": {
            "text_layer_ref": SOURCE_REF,
            "text_layer_sha256": _sha256_text(SOURCE_TEXT),
            "language": "x-tos-unit",
            "media_type": "text/plain; charset=utf-8",
            "unicode_form": "source_preserved",
            "position_unit": "unicode_code_point",
            "interval": "half_open",
            "immutable": True,
            "visibility": "public",
            "publication_authorized": True,
        },
        "schemes": [],
        "anchors": [scope_anchor],
        "units": [],
        "segmentations": [],
        "reviews": [],
        "projections": [],
        "rights_and_visibility": {
            "source_visibility": "public",
            "packet_visibility": "public",
            "effective_visibility": "public",
            "rights_record_refs": [PLAN_REF],
            "private_source_used": False,
            "publication_authorized": True,
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


def _build_variant_a() -> dict[str, Any]:
    packet = _base_packet("A")
    scheme = _scheme(
        slot="layout-lines",
        name="public-synthetic exact physical-line observation",
        analysis_role="source_layout",
        boundary_basis="source_layout",
        unit_kinds=["physical_line", "whitespace"],
        policies=_policies(
            punctuation="included_in_neighbor",
            whitespace="included_in_neighbor",
            line_break="standalone_units",
        ),
    )
    packet["schemes"].append(scheme)
    refs = []
    for slot, start, end, kind, role in [
        ("a-line-1", 0, 20, "physical_line", "content"),
        ("a-break-1", 20, 21, "whitespace", "whitespace"),
        ("a-line-2", 21, 41, "physical_line", "content"),
        ("a-break-2", 41, 42, "whitespace", "whitespace"),
    ]:
        refs.append(
            _add_member(
                anchors=packet["anchors"],
                units=packet["units"],
                slot=slot,
                start=start,
                end=end,
                kind=kind,
                role=role,
                boundary_posture="source_attested",
                certainty=1.0,
            )
        )
    packet["segmentations"].append(
        _segmentation(
            slot="a-layout",
            scheme_ref=scheme["scheme_id"],
            ordered_unit_refs=refs,
            competing_refs=[],
            status="observed_source_structure",
            status_reason=(
                "Only exact physical line content and line-break code points are observed."
            ),
            uses=["navigation", "source_observation"],
        )
    )
    return packet


def _sentence_candidate(
    *, packet: dict[str, Any], prefix: str, split_first_line: bool
) -> tuple[dict[str, Any], list[str]]:
    scheme = _scheme(
        slot=f"{prefix}-sentence",
        name=f"public-synthetic sentence candidate {prefix}",
        analysis_role="linguistic",
        boundary_basis="public_synthetic_fixture",
        unit_kinds=["sentence", "whitespace"],
        policies=_policies(
            punctuation="included_in_neighbor",
            whitespace="standalone_units",
            line_break="standalone_units",
        ),
    )
    refs: list[str] = []
    if split_first_line:
        spans = [
            (f"{prefix}-sentence-1a", 0, 7, "sentence", "content"),
            (f"{prefix}-space-1", 7, 8, "whitespace", "whitespace"),
            (f"{prefix}-sentence-1b", 8, 20, "sentence", "content"),
            (f"{prefix}-break-1", 20, 21, "whitespace", "whitespace"),
            (f"{prefix}-sentence-2", 21, 41, "sentence", "content"),
            (f"{prefix}-break-2", 41, 42, "whitespace", "whitespace"),
        ]
    else:
        spans = [
            (f"{prefix}-sentence-1", 0, 20, "sentence", "content"),
            (f"{prefix}-break-1", 20, 21, "whitespace", "whitespace"),
            (f"{prefix}-sentence-2", 21, 41, "sentence", "content"),
            (f"{prefix}-break-2", 41, 42, "whitespace", "whitespace"),
        ]
    for slot, start, end, kind, role in spans:
        refs.append(
            _add_member(
                anchors=packet["anchors"],
                units=packet["units"],
                slot=slot,
                start=start,
                end=end,
                kind=kind,
                role=role,
                boundary_posture="method_proposed",
                certainty=0.5,
            )
        )
    return scheme, refs


def _token_candidate(
    *, packet: dict[str, Any], prefix: str, split_hyphen: bool
) -> tuple[dict[str, Any], list[str]]:
    scheme = _scheme(
        slot=f"{prefix}-token",
        name=f"public-synthetic orthographic token candidate {prefix}",
        analysis_role="orthographic",
        boundary_basis="public_synthetic_fixture",
        unit_kinds=["surface_token", "punctuation", "whitespace"],
        policies=_policies(
            punctuation="standalone_units",
            whitespace="standalone_units",
            line_break="standalone_units",
        ),
    )
    spans: list[tuple[str, int, int, str, str]] = [
        (f"{prefix}-mark-1", 0, 4, "surface_token", "content"),
        (f"{prefix}-space-1", 4, 5, "whitespace", "whitespace"),
        (f"{prefix}-a", 5, 6, "surface_token", "content"),
        (f"{prefix}-period-1", 6, 7, "punctuation", "punctuation"),
        (f"{prefix}-space-2", 7, 8, "whitespace", "whitespace"),
    ]
    if split_hyphen:
        spans.extend(
            [
                (f"{prefix}-ember-1", 8, 13, "surface_token", "content"),
                (f"{prefix}-hyphen", 13, 14, "punctuation", "punctuation"),
                (f"{prefix}-turns-1", 14, 19, "surface_token", "content"),
            ]
        )
    else:
        spans.append(
            (f"{prefix}-ember-turns", 8, 19, "surface_token", "content")
        )
    spans.extend(
        [
            (f"{prefix}-period-2", 19, 20, "punctuation", "punctuation"),
            (f"{prefix}-break-1", 20, 21, "whitespace", "whitespace"),
            (f"{prefix}-mark-2", 21, 25, "surface_token", "content"),
            (f"{prefix}-space-3", 25, 26, "whitespace", "whitespace"),
            (f"{prefix}-b", 26, 27, "surface_token", "content"),
            (f"{prefix}-colon", 27, 28, "punctuation", "punctuation"),
            (f"{prefix}-space-4", 28, 29, "whitespace", "whitespace"),
            (f"{prefix}-ember-2", 29, 34, "surface_token", "content"),
            (f"{prefix}-space-5", 34, 35, "whitespace", "whitespace"),
            (f"{prefix}-turns-2", 35, 40, "surface_token", "content"),
            (f"{prefix}-period-3", 40, 41, "punctuation", "punctuation"),
            (f"{prefix}-break-2", 41, 42, "whitespace", "whitespace"),
        ]
    )
    refs: list[str] = []
    for slot, start, end, kind, role in spans:
        refs.append(
            _add_member(
                anchors=packet["anchors"],
                units=packet["units"],
                slot=slot,
                start=start,
                end=end,
                kind=kind,
                role=role,
                boundary_posture="method_proposed",
                certainty=0.5,
            )
        )
    return scheme, refs


def _build_variant_b() -> dict[str, Any]:
    packet = _base_packet("B")
    sentence_whole_id = _sid("text-segmentation", "b-sentence-whole")
    sentence_split_id = _sid("text-segmentation", "b-sentence-split")
    token_whole_id = _sid("text-segmentation", "b-token-hyphen-whole")
    token_split_id = _sid("text-segmentation", "b-token-hyphen-split")

    sentence_whole_scheme, sentence_whole_refs = _sentence_candidate(
        packet=packet, prefix="b-whole", split_first_line=False
    )
    sentence_split_scheme, sentence_split_refs = _sentence_candidate(
        packet=packet, prefix="b-split", split_first_line=True
    )
    token_whole_scheme, token_whole_refs = _token_candidate(
        packet=packet, prefix="b-token-whole", split_hyphen=False
    )
    token_split_scheme, token_split_refs = _token_candidate(
        packet=packet, prefix="b-token-split", split_hyphen=True
    )
    packet["schemes"] = [
        sentence_whole_scheme,
        sentence_split_scheme,
        token_whole_scheme,
        token_split_scheme,
    ]
    packet["segmentations"] = [
        _segmentation(
            slot="b-sentence-whole",
            scheme_ref=sentence_whole_scheme["scheme_id"],
            ordered_unit_refs=sentence_whole_refs,
            competing_refs=[sentence_split_id],
            status="ambiguous",
            status_reason="Whole-first-line sentence candidate remains unresolved.",
            uses=["linguistic_analysis", "translation_alignment"],
        ),
        _segmentation(
            slot="b-sentence-split",
            scheme_ref=sentence_split_scheme["scheme_id"],
            ordered_unit_refs=sentence_split_refs,
            competing_refs=[sentence_whole_id],
            status="ambiguous",
            status_reason="Split-first-line sentence candidate remains unresolved.",
            uses=["linguistic_analysis", "translation_alignment"],
        ),
        _segmentation(
            slot="b-token-hyphen-whole",
            scheme_ref=token_whole_scheme["scheme_id"],
            ordered_unit_refs=token_whole_refs,
            competing_refs=[token_split_id],
            status="ambiguous",
            status_reason="Hyphenated surface-token candidate remains unresolved.",
            uses=["linguistic_analysis", "model_input"],
        ),
        _segmentation(
            slot="b-token-hyphen-split",
            scheme_ref=token_split_scheme["scheme_id"],
            ordered_unit_refs=token_split_refs,
            competing_refs=[token_whole_id],
            status="ambiguous",
            status_reason="Split-hyphen surface-token candidate remains unresolved.",
            uses=["linguistic_analysis", "model_input"],
        ),
    ]
    return packet


def _build_variant_c(variant_b: dict[str, Any]) -> dict[str, Any]:
    packet = copy.deepcopy(variant_b)
    packet["packet_id"] = _sid("source-text-unit-packet", "variant-c")
    packet["schemes"] = [packet["schemes"][0]]
    segmentation = packet["segmentations"][0]
    segmentation["segmentation_id"] = _sid("text-segmentation", "c-invalid")
    segmentation["scheme_ref"] = packet["schemes"][0]["scheme_id"]
    segmentation["status"] = "accepted"
    segmentation["status_reason"] = (
        "Invalid control: a synthetic model-shaped result claims acceptance without review."
    )
    segmentation["maker"] = _method(
        method_name="invalid model-shaped sentence boundary output",
        method_version="synthetic-invalid-control",
        maker_kind="model",
        model_ref="model:synthetic-invalid-control-not-invoked",
    )
    segmentation["competing_segmentation_refs"] = []
    segmentation["review_refs"] = []
    # Hide the final line break while falsely retaining exhaustive coverage.
    removed_unit_ref = segmentation["ordered_unit_refs"].pop()
    referenced = set(segmentation["ordered_unit_refs"])
    packet["units"] = [u for u in packet["units"] if u["unit_id"] in referenced]
    anchor_refs = {
        ref for unit in packet["units"] for ref in unit["ordered_anchor_refs"]
    }
    packet["anchors"] = [
        anchor
        for anchor in packet["anchors"]
        if anchor["anchor_ref"]
        == "tos.anchor.source-text-unit-v1.synthetic-scope"
        or anchor["anchor_ref"] in anchor_refs
    ]
    packet["segmentations"] = [segmentation]
    packet["reviews"] = []
    packet["projections"] = []
    segmentation["status_reason"] += f" Removed unit {removed_unit_ref}."
    return packet


def _build_plan() -> dict[str, Any]:
    return {
        "schema_version": "tos_source_text_unit_v1_lab_plan_v1",
        "question": (
            "Can ToS preserve exact source text units and reciprocal competing "
            "segmentations without turning a method result into source or linguistic truth?"
        ),
        "input_posture": "public_synthetic_only",
        "language": "x-tos-unit",
        "negative_controls": [
            "text-derived-unit-id",
            "duplicate-unit-id",
            "duplicate-scheme-id",
            "duplicate-segmentation-id",
            "missing-scheme-ref",
            "missing-unit-ref",
            "missing-anchor-ref",
            "source-layer-digest-drift",
            "anchor-exact-digest-drift",
            "anchor-out-of-bounds",
            "anchor-reversed-range",
            "unit-anchor-order-overlap",
            "one-way-parent-child",
            "self-parent-unit",
            "one-way-competing-segmentation",
            "self-competing-segmentation",
            "hidden-exhaustive-coverage-gap",
            "undeclared-overlap",
            "accepted-machine-result-without-review",
            "accepted-with-sample-only-review",
            "accepted-without-language-competence",
            "model-subword-promoted-to-linguistic-authority",
            "graph-projection-of-proposed-segmentation",
            "projection-visibility-widening",
            "self-supersession",
        ],
        "variants": [
            {
                "variant_id": "A",
                "purpose": "Exact physical-line and line-break observation only",
                "expected_schema_valid": True,
                "expected_semantic_valid": True,
            },
            {
                "variant_id": "B",
                "purpose": (
                    "Reciprocal sentence and hyphen-tokenization alternatives, all unresolved"
                ),
                "expected_schema_valid": True,
                "expected_semantic_valid": True,
            },
            {
                "variant_id": "C",
                "purpose": (
                    "Invalid model-shaped acceptance without review and with hidden coverage gap"
                ),
                "expected_schema_valid": False,
                "expected_semantic_valid": False,
            },
        ],
        "authority_limits": {
            "private_source_used": False,
            "real_language_boundary_accepted": False,
            "model_invoked": False,
            "human_review_performed": False,
            "linguistic_word_established": False,
            "lexeme_established": False,
            "semantic_truth_established": False,
            "graph_truth_established": False,
            "legacy_migration_performed": False,
            "canon_effect": False,
        },
    }


def build() -> dict[str, Any]:
    _path(SOURCE_REF).parent.mkdir(parents=True, exist_ok=True)
    _path(SOURCE_REF).write_text(SOURCE_TEXT, encoding="utf-8")
    plan = _build_plan()
    _write_json(PLAN_REF, plan)

    variant_a = _build_variant_a()
    variant_b = _build_variant_b()
    variant_c = _build_variant_c(variant_b)
    variants = {"A": variant_a, "B": variant_b, "C": variant_c}
    for variant_id, ref in VARIANT_REFS.items():
        _write_json(ref, variants[variant_id])

    manifest = {
        "schema_version": "tos_source_text_unit_v1_lab_manifest_v1",
        "authority_posture": "public_synthetic_contract_mechanics_only",
        "source": {"ref": SOURCE_REF, "sha256": _sha256(_path(SOURCE_REF))},
        "plan": {"ref": PLAN_REF, "sha256": _sha256(_path(PLAN_REF))},
        "research": {
            "ref": RESEARCH_REF,
            "sha256": _sha256(_path(RESEARCH_REF)),
        },
        "contract": {"ref": SCHEMA_REF, "sha256": _sha256(_path(SCHEMA_REF))},
        "builder": {"ref": SCRIPT_REF, "sha256": _sha256(_path(SCRIPT_REF))},
        "variants": [
            {
                "variant_id": variant_id,
                "packet_ref": ref,
                "packet_sha256": _sha256(_path(ref)),
                "expected_schema_valid": next(
                    item["expected_schema_valid"]
                    for item in plan["variants"]
                    if item["variant_id"] == variant_id
                ),
                "expected_semantic_valid": next(
                    item["expected_semantic_valid"]
                    for item in plan["variants"]
                    if item["variant_id"] == variant_id
                ),
            }
            for variant_id, ref in VARIANT_REFS.items()
        ],
        "authority_limits": plan["authority_limits"],
    }
    _write_json(MANIFEST_REF, manifest)
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--build", action="store_true", help="write the deterministic A/B/C laboratory"
    )
    args = parser.parse_args()
    if not args.build:
        parser.error("--build is required")
    manifest = build()
    print(json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

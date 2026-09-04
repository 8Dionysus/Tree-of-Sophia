#!/usr/bin/env python3
"""Build the bounded review-preparation layer for eternal return."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import secrets
import stat
import sys
import tempfile
from collections import Counter
from pathlib import Path
from typing import Any, Iterable


REPO = Path(__file__).resolve().parents[1]
WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")
PARENT = Path("ToS/candidate-intake/zarathustra/eternal-return-concept-candidate-v1")
ROUTE = PARENT / "review-preparation-v1"
ALIGN = WORK / "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1"
PRIVATE = WORK / (
    "gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/"
    "review-preparation-v1/review-preparation-analysis.v1.json"
)
PLAN_REF = ROUTE / "plan.v1.json"
ISSUANCE_REF = ROUTE / "identity-issuance.v1.json"
GENERATOR_REF = Path("scripts/build_zarathustra_eternal_return_review_preparation_v1.py")
PARENT_BUILDER = Path("scripts/build_zarathustra_eternal_return_concept_candidate_v1.py")
RU_BUILDER = Path("scripts/build_antonovsky_1911_structural_paragraph_v2.py")

OUTPUTS = {
    "gaps": ROUTE / "gap-review-candidates.v1.jsonl",
    "speakers": ROUTE / "speaker-attribution-candidates.v1.jsonl",
    "matrix": ROUTE / "interpretation-review-matrix.v1.json",
    "worklist": ROUTE / "review-worklist.v1.json",
    "coverage": ROUTE / "coverage-receipt.v1.json",
    "summary": ROUTE / "summary.v1.json",
    "provenance": ROUTE / "provenance.jsonl",
    "manifest": ROUTE / "manifest.v1.json",
}

CORE_READINGS = ("p3.r2", "p3.r13", "p3.r16", "p4.r19")
AXES = (
    "cosmological_recurrence",
    "temporality_and_augenblick",
    "existential_test_and_affirmation",
    "ring_and_wholeness",
    "life_joy_pain_affirmation",
    "poetic_symbolic_affirmation",
    "amor_fati_cross_work",
)
GAPS = (
    {
        "alignment_ref": "tos.translation-alignment.sid-6922c0b268a890cf94d0454b67143f43",
        "reading_ref": "p3.r2",
        "gap_code": "one_to_many_mapping",
        "disposition": "prepared_alignment_shape_review",
        "proposal_kind": "none",
    },
    {
        "alignment_ref": "tos.translation-alignment.sid-a1f9250a6975af780fb35023b6070ff2",
        "reading_ref": "p3.r13",
        "gap_code": "ru_formula_broken_by_letterspacing",
        "disposition": "prepared_reversible_letterspacing_normalization",
        "proposal_kind": "normalization_candidate_not_witness_correction",
    },
    {
        "alignment_ref": "tos.translation-alignment.sid-a474f471a32163007afa33b3ae624ab9",
        "reading_ref": "p3.r13",
        "gap_code": "ru_formula_blocked_by_ocr_substitution",
        "disposition": "prepared_reversible_ocr_correction_candidate",
        "proposal_kind": "correction_candidate_requires_page_image_review",
    },
    {
        "alignment_ref": "tos.translation-alignment.sid-b3ce5734f20040829abd1acbbf5c17bd",
        "reading_ref": "p3.r15",
        "gap_code": "machine_target_gap_not_translation_omission",
        "disposition": "mechanically_explained_as_alignment_scope_gap",
        "proposal_kind": "retain_related_ru_verse_as_separate_witness_local_unit",
    },
    {
        "alignment_ref": "tos.translation-alignment.sid-f7e6b8c00ed9861bf01841a9b0212e78",
        "reading_ref": "p3.r16",
        "gap_code": "one_to_many_mapping",
        "disposition": "prepared_alignment_shape_review",
        "proposal_kind": "none",
    },
)

COUNTERPRESSURE = {
    "cosmological_recurrence": (
        "tos.translation-alignment.sid-43f9afb7f3baf9652a9d36beafeaccca",
        "tos.translation-alignment.sid-038becaa517bcf833165edd4732466ee",
        "tos.translation-alignment.sid-5a853b515e18fded22147d4b35d92173",
        "tos.translation-alignment.sid-a474f471a32163007afa33b3ae624ab9",
    ),
    "temporality_and_augenblick": (
        "tos.translation-alignment.sid-43f9afb7f3baf9652a9d36beafeaccca",
        "tos.translation-alignment.sid-0b5132d2533e747cc08f0f961635c66d",
    ),
    "existential_test_and_affirmation": (
        "tos.translation-alignment.sid-5a853b515e18fded22147d4b35d92173",
        "tos.translation-alignment.sid-a474f471a32163007afa33b3ae624ab9",
        "tos.translation-alignment.sid-038becaa517bcf833165edd4732466ee",
    ),
    "ring_and_wholeness": (
        "tos.translation-alignment.sid-f7e6b8c00ed9861bf01841a9b0212e78",
        "tos.translation-alignment.sid-42502a80bd28842dcda92248e79ff6c6",
    ),
    "life_joy_pain_affirmation": (
        "tos.translation-alignment.sid-038becaa517bcf833165edd4732466ee",
        "tos.translation-alignment.sid-a474f471a32163007afa33b3ae624ab9",
        "tos.translation-alignment.sid-f0f8819d450f5a35530abfc4f6eff1a3",
    ),
    "poetic_symbolic_affirmation": (
        "tos.translation-alignment.sid-a1f9250a6975af780fb35023b6070ff2",
        "tos.translation-alignment.sid-54a3fb7fbfaab700a862c7c0bf797d96",
    ),
    "amor_fati_cross_work": (),
}

QUESTIONS = {
    "cosmological_recurrence": [
        "Does the passage assert recurrence of the whole course of becoming, or report a character's formulation?",
        "Does Zarathustra's warning against making the circle too easy qualify the cosmological formulation?",
    ],
    "temporality_and_augenblick": [
        "Does Augenblick organize the two temporal paths without being reducible to a circular-time thesis?",
        "Which circle statement belongs to the dwarf rather than to Zarathustra's settled voice?",
    ],
    "existential_test_and_affirmation": [
        "Does Noch Ein Mal operate as an existential test here, and what textual scope does the affirmation include?",
        "Do nausea, sickness, and the smallest human prevent a purely affirmative reduction?",
    ],
    "ring_and_wholeness": [
        "Is the ring temporal recurrence, marriage/composition, wholeness, or an intentionally plural symbol in this passage?",
        "Which ring formulas occur in song rather than discursive teaching?",
    ],
    "life_joy_pain_affirmation": [
        "Does joy's demand for eternity include pain and failure rather than cancel them?",
        "Is this axis evidence for eternal return or a neighboring affirmation complex?",
    ],
    "poetic_symbolic_affirmation": [
        "Can ring, song, dance, and midnight sustain a poetic-symbolic reading without deciding cosmology?",
    ],
    "amor_fati_cross_work": [
        "What separately admitted cross-work witness would be required before relating this candidate to amor fati?",
    ],
}

AXIS_ANALYSIS = {
    "cosmological_recurrence": {
        "candidate_synthesis": "The corpus contains formulations of the return of all things, the same causal knot, and the same life in greatest and smallest detail.",
        "counterpressure_summary": "The most systematic formulation is voiced by the animals, while Zarathustra warns the dwarf not to make the circle too easy and reacts with nausea to the return of the smallest human.",
        "speaker_dependency": "A cosmological reading must distinguish Zarathustra's questions and resistance from the dwarf's circle formula and the animals' reconstruction of his teaching.",
    },
    "temporality_and_augenblick": {
        "candidate_synthesis": "The gateway Augenblick joins opposed eternal paths and makes the present moment a structural problem rather than a mere date in linear time.",
        "counterpressure_summary": "The concise claim that time itself is a circle belongs to the dwarf and is immediately treated as an over-easy answer.",
        "speaker_dependency": "The dwarf's circle statement cannot be silently promoted into Zarathustra's unqualified doctrine; the surrounding questions remain Zarathustra's narrated trial.",
    },
    "existential_test_and_affirmation": {
        "candidate_synthesis": "Noch Ein Mal and the wish that everything return can function as a test of whether a life, moment, joy, and pain can be affirmed again.",
        "counterpressure_summary": "The test includes disgust, sickness, the smallest human, and the smallest detail, so it cannot be reduced to cheerful repetition or preference for selected moments.",
        "speaker_dependency": "Zarathustra's first-person ordeal and midnight song provide the strongest existential evidence, while the animals' formulas supply a different discursive register.",
    },
    "ring_and_wholeness": {
        "candidate_synthesis": "Ring language connects recurrence, composition, marriage to eternity, self-return, and the desire for wholeness across song and discourse.",
        "counterpressure_summary": "The ring is also a nuptial and poetic figure and the ring's will appears within joy's song; neither use alone proves temporal cosmology.",
        "speaker_dependency": "Most ring evidence is performed in Zarathustra's song or midnight voice, so symbolic voice and propositional teaching must remain distinct.",
    },
    "life_joy_pain_affirmation": {
        "candidate_synthesis": "Joy wants itself and the eternity of all things, and therefore draws pain, failure, hatred, graves, and the world into the candidate affirmation complex.",
        "counterpressure_summary": "Sickness, disgust, deepest pain, and commands for pain to pass resist a shallow identity of affirmation with pleasure or optimism.",
        "speaker_dependency": "The densest formulas belong to the performative midnight song, including personified joy, pain, bell, and midnight voices rather than a single flat speaker.",
    },
    "poetic_symbolic_affirmation": {
        "candidate_synthesis": "Ring, dance, song, bell, midnight, and marriage can carry a coherent poetic-symbolic affirmation without settling the cosmological question.",
        "counterpressure_summary": "Explicit all-things, causal-knot, and same-life formulas prevent the poetic reading from simply erasing the corpus's stronger recurrence assertions.",
        "speaker_dependency": "This reading depends especially on song and personification, but must coexist with the animals' more discursive recurrence formulas.",
    },
    "amor_fati_cross_work": {
        "candidate_synthesis": "No relation is asserted inside this Zarathustra-only scope.",
        "counterpressure_summary": "A familiar philosophical association is not source evidence; a separate cross-work witness and review are required.",
        "speaker_dependency": "No speaker attribution can fill the missing cross-work evidence boundary.",
    },
}


class BuildError(RuntimeError):
    pass


def jb(value: Any, pretty: bool = True) -> bytes:
    text = json.dumps(value, ensure_ascii=False, sort_keys=True,
                      indent=2 if pretty else None,
                      separators=None if pretty else (",", ":"))
    return (text + "\n").encode()


def jlb(rows: Iterable[dict[str, Any]]) -> bytes:
    return b"".join(jb(row, False) for row in rows)


def sha_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha_file(path: Path) -> str:
    return sha_bytes(path.read_bytes())


def h(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads((REPO / path).read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise BuildError(f"object required: {path}")
    return value


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in (REPO / path).read_text(encoding="utf-8").splitlines()]


def import_builder(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, REPO / path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def verify_inputs(plan: dict[str, Any]) -> None:
    for name, record in plan["inputs"].items():
        path = REPO / record["ref"]
        if not path.is_file() or sha_file(path) != record["sha256"]:
            raise BuildError(f"input drift: {name}")


def source_ordered_rows() -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]], Any]:
    parent = import_builder(PARENT_BUILDER, "tos_er_parent_for_review")
    units = {x["alignment_id"]: x for x in parent.hydrate_units()}
    public = {x["alignment_ref"]: x for x in load_jsonl(PARENT / "evidence-spine.v1.jsonl")}
    private = {x["alignment_ref"]: x for x in load_json(
        WORK / "gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/eternal-return-analysis.v1.json"
    )["evidence"]}
    spine = {x["alignment_id"]: x for x in load_jsonl(ALIGN / "alignment-spine.v1.jsonl")}
    ordered: list[dict[str, Any]] = []
    for part in range(1, 5):
        packet = load_json(ALIGN / f"part-{part}.translation-alignment-packet.v1.json")
        for alignment in packet["alignments"]:
            aid = alignment["alignment_id"]
            if aid not in public:
                continue
            reading = f"p{part}.r{spine[aid]['reading_ordinal_within_part']}"
            if reading not in CORE_READINGS or public[aid]["evidence_class"] not in {"core", "supporting"}:
                continue
            exact = private[aid]
            if h(exact["de_text"]) != public[aid]["de_exact_sha256"] or h(exact["ru_text"]) != public[aid]["ru_exact_sha256"]:
                raise BuildError(f"parent exact-text digest drift: {aid}")
            ordered.append({**units[aid], **public[aid], "de_text": exact["de_text"], "ru_text": exact["ru_text"]})
    counts = Counter(x["reading_ref"] for x in ordered)
    if counts != Counter({"p3.r2": 31, "p3.r13": 50, "p3.r16": 34, "p4.r19": 44}):
        raise BuildError(f"speaker population drift: {dict(counts)}")
    return ordered, units, parent


def speaker_rule(reading: str, position: int) -> dict[str, Any]:
    base = {"status": "proposed", "alternative_roles": [], "exception_group": None}
    if reading == "p3.r2":
        if position == 1:
            return {**base, "primary_role": "external_narrator", "cue": "chapter framing introduces Zarathustra's speech"}
        if position in {3, 4}:
            return {**base, "primary_role": "spirit_of_gravity_as_dwarf_voice",
                    "alternative_roles": ["zarathustra_quoting_internal_voice"], "status": "ambiguous",
                    "exception_group": "p3.r2.embedded_gravity_voice", "cue": "embedded second-person taunt inside Zarathustra's narration"}
        if position == 14:
            return {**base, "primary_role": "dwarf", "cue": "explicit speech report names the dwarf"}
        return {**base, "primary_role": "zarathustra_as_storyteller", "cue": "first-person address or narrated vision within Zarathustra's announced speech"}
    if reading == "p3.r13":
        if position == 1:
            return {**base, "primary_role": "mixed_external_narrator_and_zarathustra",
                    "alternative_roles": ["external_narrator", "zarathustra"], "status": "ambiguous",
                    "exception_group": "p3.r13.opening_transition", "cue": "narrator frame ends in direct Zarathustra speech"}
        if position in {5, 6, 50}:
            return {**base, "primary_role": "external_narrator", "cue": "explicit narrative report outside quoted exchange"}
        if position in set(range(7, 10)) | set(range(16, 20)) | {36} | set(range(39, 50)):
            alternatives: list[str] = []
            status = "proposed"
            group = None
            if 39 <= position <= 49:
                alternatives = ["animals_voicing_a_hypothetical_zarathustra"]
                status = "ambiguous"
                group = "p3.r13.animals_voice_zarathustra_formula"
            return {**base, "primary_role": "animals_eagle_and_serpent", "alternative_roles": alternatives,
                    "status": status, "exception_group": group,
                    "cue": "animals' reply, including their representation of what Zarathustra teaches or would say"}
        return {**base, "primary_role": "zarathustra", "cue": "Zarathustra's direct answer to the animals"}
    if reading == "p3.r16":
        if position == 1:
            return {**base, "primary_role": "paratext_heading", "status": "ambiguous",
                    "alternative_roles": ["editorial_or_authorial_subtitle"],
                    "exception_group": "p3.r16.heading_voice", "cue": "parenthetical alternate title, not a dramatic utterance"}
        return {**base, "primary_role": "zarathustra_song_voice", "cue": "first-person refrain within the Yes-and-Amen song"}
    if reading == "p4.r19":
        if position in {1, 2, 7}:
            return {**base, "primary_role": "external_narrator", "cue": "explicit narrative report"}
        if 3 <= position <= 6:
            return {**base, "primary_role": "ugliest_man", "cue": "speech explicitly introduced and closed as the ugliest man's"}
        if position in {8, 9}:
            return {**base, "primary_role": "mixed_external_narrator_and_zarathustra",
                    "alternative_roles": ["external_narrator", "zarathustra"], "status": "ambiguous",
                    "exception_group": "p4.r19.narrator_to_midnight_transition", "cue": "narrator frame transitions into Zarathustra's altered voice"}
        if position == 10:
            return {**base, "primary_role": "zarathustra", "cue": "explicit narrator report followed by Zarathustra's direct command"}
        return {**base, "primary_role": "zarathustra_midnight_song_voice",
                "alternative_roles": ["personified_midnight_or_bell_voice"], "status": "ambiguous",
                "exception_group": "p4.r19.performative_midnight_voice",
                "cue": "Zarathustra performs a midnight song while attributing speech to midnight, bell, pain, or joy"}
    raise BuildError(f"unknown reading: {reading}")


def bindings(rows: list[dict[str, Any]]) -> list[tuple[str, str]]:
    result = [("packet", "review-preparation-v1")]
    result.extend(("gap", x["alignment_ref"]) for x in GAPS)
    result.extend(("speaker", x["alignment_ref"]) for x in rows)
    result.extend(("axis", x) for x in AXES)
    return sorted(result)


def issue(rows: list[tuple[str, str]]) -> None:
    target = REPO / ISSUANCE_REF
    if target.exists():
        raise BuildError("identity issuance already exists; refusing to remint")
    prefixes = {
        "packet": "tos.annotation.review-preparation.sid-",
        "gap": "tos.annotation.gap-review-candidate.sid-",
        "speaker": "tos.annotation.speaker-attribution-candidate.sid-",
        "axis": "tos.annotation.interpretation-review-candidate.sid-",
    }
    payload = {
        "schema_version": "tos_zarathustra_eternal_return_review_preparation_identity_issuance_v1",
        "identity_policy": "opaque-id-independent-of-source-text-label-speaker-name-and-current-interpretation",
        "issued_at": load_json(PLAN_REF)["frozen_at"],
        "records": [{"kind": kind, "binding": binding, "id": prefixes[kind] + secrets.token_hex(16)}
                    for kind, binding in rows],
    }
    write(ISSUANCE_REF, jb(payload))


def identity_map(expected: list[tuple[str, str]]) -> dict[tuple[str, str], str]:
    value = load_json(ISSUANCE_REF)
    result = {(x["kind"], x["binding"]): x["id"] for x in value["records"]}
    if set(result) != set(expected) or len(set(result.values())) != len(result):
        raise BuildError("identity issuance mismatch")
    return result


def return_ru_verse() -> dict[str, Any]:
    builder = import_builder(RU_BUILDER, "tos_ru_structural_for_er_review")
    model = builder.reconstruct()
    ids = builder.load_identities(model)["logical_rows"]
    wanted = "tos.text-unit.sid-d523bc897647d01030f767c2faf5266b"
    for row in model["rows"]:
        if ids[builder.row_binding(row)] == wanted:
            expected = "0627928ac02cab8159b250950b4a9b23088c9fe5886ebedf86634be3488da558"
            if row.text_sha256 != expected:
                raise BuildError("related Russian verse text drift")
            return {
                "text_unit_ref": wanted,
                "text": row.text,
                "text_sha256": row.text_sha256,
                "physical_line_refs": [x.source_line_ref for x in row.lines],
            }
    raise BuildError("related Russian verse row not found")


def make_gap_rows(units: dict[str, dict[str, Any]], ids: dict[tuple[str, str], str],
                  ru_verse: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    public, private = [], []
    for spec in GAPS:
        row = units[spec["alignment_ref"]]
        if row["reading"] != spec["reading_ref"]:
            raise BuildError(f"gap reading drift: {spec['alignment_ref']}")
        related = []
        if spec["gap_code"] == "machine_target_gap_not_translation_omission":
            related = [{
                "text_unit_ref": ru_verse["text_unit_ref"],
                "text_sha256": ru_verse["text_sha256"],
                "relation": "related_verse_line_outside_paragraph_alignment_scope_not_accepted_translation_mapping",
            }]
        tracked = {
            "schema_version": "tos_zarathustra_eternal_return_gap_review_candidate_v1",
            "gap_review_candidate_id": ids[("gap", spec["alignment_ref"])],
            "parent_annotation_ref": load_json(PARENT / "concept-candidate.v1.json")["annotation_id"],
            **spec,
            "alignment_shape": row["shape"],
            "alignment_status": row["status"],
            "source_anchor_refs": row["source_anchor_refs"],
            "target_anchor_refs": row["target_anchor_refs"],
            "de_exact_sha256": h(row["de_text"]),
            "ru_exact_sha256": h(row["ru_text"]),
            "related_witness_units": related,
            "observed_text_preserved": True,
            "proposal_applied_to_witness": False,
            "proposal_applied_to_alignment": False,
            "review_status": "unreviewed",
            "accepted": False,
            "human_judgment": False,
            "translation_equivalence_asserted": False,
            "graph_effect": False,
            "canon_effect": False,
            "source_text_included": False,
        }
        proposal = None
        if spec["gap_code"] == "ru_formula_broken_by_letterspacing":
            proposal = {"language": "ru", "candidate_tokens": ["вѣчнаго", "возвращенія"],
                        "operation": "collapse_letterspacing_for_search_index_only"}
        elif spec["gap_code"] == "ru_formula_blocked_by_ocr_substitution":
            proposal = {"language": "ru", "observed_token": "вЪчное", "candidate_token": "вѣчное",
                        "operation": "single_token_ocr_correction_candidate_requires_page_image_review"}
        private.append({**tracked, "de_text": row["de_text"], "ru_text": row["ru_text"],
                        "reversible_proposal": proposal,
                        "related_ru_verse": ru_verse if related else None})
        public.append(tracked)
    return public, private


def make_speakers(rows: list[dict[str, Any]], ids: dict[tuple[str, str], str]) -> list[dict[str, Any]]:
    counters: Counter[str] = Counter()
    result = []
    for row in rows:
        reading = row["reading_ref"]
        counters[reading] += 1
        rule = speaker_rule(reading, counters[reading])
        result.append({
            "schema_version": "tos_zarathustra_speaker_attribution_candidate_v1",
            "speaker_attribution_candidate_id": ids[("speaker", row["alignment_ref"])],
            "parent_annotation_ref": row["annotation_ref"],
            "evidence_ref": row["evidence_id"],
            "alignment_ref": row["alignment_ref"],
            "reading_ref": reading,
            "source_order_within_selected_reading": counters[reading],
            "primary_role": rule["primary_role"],
            "alternative_roles": rule["alternative_roles"],
            "attribution_status": rule["status"],
            "attribution_cue": rule["cue"],
            "exception_group": rule["exception_group"],
            "source_anchor_refs": row["source_anchor_refs"],
            "target_anchor_refs": row["target_anchor_refs"],
            "de_exact_sha256": row["de_exact_sha256"],
            "ru_exact_sha256": row["ru_exact_sha256"],
            "accepted": False,
            "review_status": "unreviewed",
            "human_judgment": False,
            "materialized_claim": False,
            "graph_effect": False,
            "canon_effect": False,
        })
    return result


def make_matrix(speakers: list[dict[str, Any]], ids: dict[tuple[str, str], str]) -> dict[str, Any]:
    templates = {x["claim_code"]: x for x in load_jsonl(PARENT / "interpretation-templates.v1.jsonl")}
    parent_evidence = {x["alignment_ref"]: x for x in load_jsonl(PARENT / "evidence-spine.v1.jsonl")}
    by_evidence = {x["evidence_ref"]: x for x in speakers}
    axes = []
    for code in AXES:
        template = templates[code]
        positive = template["evidence_refs"]
        counter = [parent_evidence[x]["evidence_id"] for x in COUNTERPRESSURE[code]]
        speaker_counts = Counter()
        missing_speaker_refs = 0
        for ref in positive:
            if ref in by_evidence:
                speaker_counts[by_evidence[ref]["primary_role"]] += 1
            else:
                missing_speaker_refs += 1
        axes.append({
            "interpretation_review_candidate_id": ids[("axis", code)],
            "interpretation_template_ref": template["interpretation_template_id"],
            "axis_code": code,
            "status": "blocked_cross_work" if code == "amor_fati_cross_work" else "prepared_for_review",
            "positive_evidence_refs": positive,
            "counterpressure_evidence_refs": counter,
            "speaker_role_counts_within_core_readings": dict(sorted(speaker_counts.items())),
            "positive_evidence_without_core_reading_speaker_candidate_count": missing_speaker_refs,
            "candidate_synthesis": AXIS_ANALYSIS[code]["candidate_synthesis"],
            "counterpressure_summary": AXIS_ANALYSIS[code]["counterpressure_summary"],
            "speaker_dependency": AXIS_ANALYSIS[code]["speaker_dependency"],
            "review_questions": QUESTIONS[code],
            "accepted": False,
            "human_judgment": False,
            "materialized_claim": False,
            "semantic_fact_asserted": False,
            "graph_effect": False,
            "canon_effect": False,
        })
    return {
        "schema_version": "tos_zarathustra_eternal_return_interpretation_review_matrix_v1",
        "parent_annotation_ref": load_json(PARENT / "concept-candidate.v1.json")["annotation_id"],
        "axes": axes,
        "comparison_law": "positive evidence and counterpressure coexist; counts do not rank truth",
        "accepted_axis_count": 0,
        "human_review_count": 0,
        "graph_effect": False,
        "canon_effect": False,
    }


def generate(include_ids: bool) -> tuple[dict[Path, bytes], bytes, list[tuple[str, str]], dict[str, Any]]:
    plan = load_json(PLAN_REF)
    verify_inputs(plan)
    rows, units, _parent = source_ordered_rows()
    expected = bindings(rows)
    if not include_ids:
        return {}, b"", expected, {"speaker_candidate_count": len(rows)}
    ids = identity_map(expected)
    ru_verse = return_ru_verse()
    gaps, private_gaps = make_gap_rows(units, ids, ru_verse)
    speakers = make_speakers(rows, ids)
    matrix = make_matrix(speakers, ids)
    exception_groups = sorted({x["exception_group"] for x in speakers if x["exception_group"]})
    work_items = [
        {"work_item_code": f"gap.{n}", "kind": "gap_exception_bundle",
         "candidate_refs": [row["gap_review_candidate_id"]], "review_status": "unreviewed"}
        for n, row in enumerate(gaps, 1)
    ]
    for group in exception_groups:
        members = [x["speaker_attribution_candidate_id"] for x in speakers if x["exception_group"] == group]
        work_items.append({"work_item_code": f"speaker.{group}", "kind": "speaker_boundary_bundle",
                           "candidate_refs": members, "review_status": "unreviewed"})
    for axis in matrix["axes"]:
        work_items.append({"work_item_code": f"axis.{axis['axis_code']}", "kind": "interpretation_axis_bundle",
                           "candidate_refs": [axis["interpretation_review_candidate_id"]],
                           "review_status": "unreviewed"})
    worklist = {
        "schema_version": "tos_zarathustra_eternal_return_review_worklist_v1",
        "review_preparation_ref": ids[("packet", "review-preparation-v1")],
        "work_items": work_items,
        "work_item_count": len(work_items),
        "speaker_candidate_population": len(speakers),
        "compression_law": "all candidates remain inspectable; future human attention is routed to grouped exceptions rather than every candidate row",
        "review_outcome_recorded": False,
        "review_ledger_ref": None,
    }
    role_counts = Counter(x["primary_role"] for x in speakers)
    status_counts = Counter(x["attribution_status"] for x in speakers)
    reading_counts = Counter(x["reading_ref"] for x in speakers)
    coverage = {
        "schema_version": "tos_zarathustra_eternal_return_review_preparation_coverage_v1",
        "gap_population": 5,
        "gap_candidates_prepared": len(gaps),
        "gap_exact_source_return_count": len(private_gaps),
        "alignment_scope_gap_with_related_ru_verse_count": sum(bool(x["related_witness_units"]) for x in gaps),
        "speaker_population": len(rows),
        "speaker_candidates_prepared": len(speakers),
        "speaker_reading_counts": dict(sorted(reading_counts.items())),
        "speaker_status_counts": dict(sorted(status_counts.items())),
        "speaker_primary_role_counts": dict(sorted(role_counts.items())),
        "speaker_exception_group_count": len(exception_groups),
        "interpretation_axis_count": len(matrix["axes"]),
        "five_primary_axes_prepared": all(x["status"] == "prepared_for_review" for x in matrix["axes"][:5]),
        "amor_fati_cross_work_blocked": matrix["axes"][-1]["status"] == "blocked_cross_work",
        "source_return_verified": True,
        "complete_for_declared_scope": len(gaps) == 5 and len(speakers) == 159,
    }
    summary = {
        "schema_version": "tos_zarathustra_eternal_return_review_preparation_summary_v1",
        "review_preparation_id": ids[("packet", "review-preparation-v1")],
        "gap_candidate_count": len(gaps),
        "speaker_candidate_count": len(speakers),
        "speaker_exception_group_count": len(exception_groups),
        "interpretation_review_candidate_count": len(matrix["axes"]),
        "future_review_work_item_count": len(work_items),
        "witness_correction_count": 0,
        "alignment_mutation_count": 0,
        "accepted_candidate_count": 0,
        "human_review_count": 0,
        "materialized_claim_count": 0,
        "review_ledger_write_count": 0,
        "graph_effect": False,
        "canon_effect": False,
    }
    provenance = [{
        "schema_version": "tos_zarathustra_eternal_return_review_preparation_event_v1",
        "event_id": "tos.event.zarathustra-eternal-return-review-preparation-v1.build",
        "event_type": "candidate_review_preparation_built",
        "event_at": plan["frozen_at"],
        "input_refs": [x["ref"] for x in plan["inputs"].values()],
        "output_refs": [str(x) for x in OUTPUTS.values()] + [str(PRIVATE)],
        "authority_effect": "candidate_only_no_human_review_graph_or_canon_effect",
    }]
    outputs: dict[Path, bytes] = {
        OUTPUTS["gaps"]: jlb(gaps),
        OUTPUTS["speakers"]: jlb(speakers),
        OUTPUTS["matrix"]: jb(matrix),
        OUTPUTS["worklist"]: jb(worklist),
        OUTPUTS["coverage"]: jb(coverage),
        OUTPUTS["summary"]: jb(summary),
        OUTPUTS["provenance"]: jlb(provenance),
    }
    private_rows = {x["alignment_ref"]: x for x in rows}
    counter_refs = sorted({x for values in COUNTERPRESSURE.values() for x in values})
    private = {
        "schema_version": "tos_zarathustra_eternal_return_review_preparation_private_analysis_v1",
        "review_preparation_id": ids[("packet", "review-preparation-v1")],
        "content_posture": "private_exact_source_return_and_reversible_analysis_not_semantic_authority",
        "gaps": private_gaps,
        "speaker_evidence": [{
            "speaker_attribution_candidate_id": speaker["speaker_attribution_candidate_id"],
            "alignment_ref": speaker["alignment_ref"],
            "de_text": private_rows[speaker["alignment_ref"]]["de_text"],
            "ru_text": private_rows[speaker["alignment_ref"]]["ru_text"],
        } for speaker in speakers],
        "counterpressure_source_return": [{
            "alignment_ref": aid,
            "de_text": units[aid]["de_text"],
            "ru_text": units[aid]["ru_text"],
        } for aid in counter_refs],
    }
    private_bytes = jb(private)
    artifacts = [{"role": name, "ref": str(path), "sha256": sha_bytes(outputs[path])}
                 for name, path in OUTPUTS.items() if name != "manifest"]
    manifest = {
        "schema_version": "tos_zarathustra_eternal_return_review_preparation_manifest_v1",
        "route_id": "zarathustra-eternal-return-review-preparation-v1",
        "review_preparation_id": ids[("packet", "review-preparation-v1")],
        "plan_ref": str(PLAN_REF),
        "plan_sha256": sha_file(REPO / PLAN_REF),
        "identity_issuance_ref": str(ISSUANCE_REF),
        "identity_issuance_sha256": sha_file(REPO / ISSUANCE_REF),
        "generator_ref": str(GENERATOR_REF),
        "generator_sha256": sha_file(REPO / GENERATOR_REF),
        "artifacts": artifacts,
        "private_artifact": {"ref": str(PRIVATE), "sha256": sha_bytes(private_bytes),
                             "mode": "0600", "tracked": False},
        "accepted_candidate_count": 0,
        "human_review_count": 0,
        "review_ledger_write_count": 0,
        "graph_effect": False,
        "canon_effect": False,
    }
    outputs[OUTPUTS["manifest"]] = jb(manifest)
    return outputs, private_bytes, expected, summary


def write(path: Path, payload: bytes, mode: int = 0o644) -> None:
    target = REPO / path
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, raw = tempfile.mkstemp(prefix=f".{target.name}.", dir=target.parent)
    temporary = Path(raw)
    try:
        with os.fdopen(fd, "wb") as handle:
            handle.write(payload)
        os.chmod(temporary, mode)
        os.replace(temporary, target)
    finally:
        temporary.unlink(missing_ok=True)


def build(issue_identities: bool) -> dict[str, Any]:
    _outputs, _private, expected, _summary = generate(False)
    if issue_identities:
        issue(expected)
    identity_map(expected)
    outputs, private, _expected, summary = generate(True)
    for path, payload in outputs.items():
        write(path, payload)
    write(PRIVATE, private, 0o600)
    return summary


def check() -> dict[str, Any]:
    outputs, private, expected, summary = generate(True)
    identity_map(expected)
    for path, payload in outputs.items():
        target = REPO / path
        if not target.is_file() or target.read_bytes() != payload:
            raise BuildError(f"tracked parity mismatch: {path}")
    target = REPO / PRIVATE
    if not target.is_file() or target.read_bytes() != private:
        raise BuildError(f"private parity mismatch: {PRIVATE}")
    if stat.S_IMODE(target.stat().st_mode) != 0o600:
        raise BuildError(f"private mode mismatch: {PRIVATE}")
    return summary


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true")
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--preview", action="store_true")
    parser.add_argument("--issue-identities", action="store_true")
    args = parser.parse_args()
    try:
        if args.preview:
            _outputs, _private, expected, summary = generate(False)
            result = {"identity_count": len(expected), **summary}
        elif args.build:
            result = build(args.issue_identities)
        else:
            if args.issue_identities:
                raise BuildError("--issue-identities is valid only with --build")
            result = check()
    except (BuildError, OSError, KeyError, ValueError, json.JSONDecodeError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

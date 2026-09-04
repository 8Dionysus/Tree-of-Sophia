#!/usr/bin/env python3
"""Build a source-addressed DE/RU eternal-return concept candidate dossier."""

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
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable


REPO = Path(__file__).resolve().parents[1]
WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")
ALIGN = WORK / "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1"
ROUTE = Path("ToS/candidate-intake/zarathustra/eternal-return-concept-candidate-v1")
PRIVATE = WORK / "gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/eternal-return-analysis.v1.json"
PLAN_REF = ROUTE / "plan.v1.json"
ISSUANCE_REF = ROUTE / "identity-issuance.v1.json"
PREVIOUS_BUILDER = Path("scripts/build_zarathustra_parallel_lexical_candidates_v1.py")
GENERATOR_REF = Path("scripts/build_zarathustra_eternal_return_concept_candidate_v1.py")

OUTPUTS = {
    "concept": ROUTE / "concept-candidate.v1.json",
    "evidence": ROUTE / "evidence-spine.v1.jsonl",
    "formulas": ROUTE / "formula-candidates.v1.jsonl",
    "templates": ROUTE / "interpretation-templates.v1.jsonl",
    "exclusions": ROUTE / "exclusion-ledger.v1.jsonl",
    "promotion": ROUTE / "promotion-readiness.v1.json",
    "coverage": ROUTE / "coverage-receipt.v1.json",
    "summary": ROUTE / "summary.v1.json",
    "audit": ROUTE / "independent-agent-audit.v1.json",
    "provenance": ROUTE / "provenance.jsonl",
    "manifest": ROUTE / "manifest.v1.json",
}

CORE_READINGS = frozenset({"p3.r2", "p3.r13", "p3.r16", "p4.r19"})
FORMULAS = {
    "de_recurrence_noun": ("de", ("wiederkunft",)),
    "de_eternal_again": ("de", ("ewig", "wieder")),
    "de_ring_of_recurrence": ("de", ("ring", "der", "wiederkunft")),
    "de_address_to_eternity": ("de", ("dich", "oh", "ewigkeit")),
    "de_deep_eternity": ("de", ("tiefe", "tiefe", "ewigkeit")),
    "ru_forever_returns": ("ru", ("вечно", "возвращается")),
    "ru_ring_of_return": ("ru", ("кольцу", "возвращения")),
    "ru_joy_wants_eternity": ("ru", ("радость", "хочет", "вечности")),
}

CLAIM_SPECS = {
    "cosmological_recurrence": {
        "kind": "interpretive_axis", "status": "ambiguous",
        "statement": "The dossier candidate includes recurrence of the whole course of becoming, not merely a local act of going back.",
    },
    "temporality_and_augenblick": {
        "kind": "interpretive_axis", "status": "ambiguous",
        "statement": "The dossier candidate preserves temporality and the moment as a distinct interpretive axis.",
    },
    "existential_test_and_affirmation": {
        "kind": "interpretive_axis", "status": "ambiguous",
        "statement": "The recurrence formulation can function as an existential test of affirmation; the dossier does not reduce it to this reading.",
    },
    "ring_and_wholeness": {
        "kind": "interpretive_axis", "status": "ambiguous",
        "statement": "Ring and wholeness remain a distinct symbolic axis rather than an automatic synonym for recurrence.",
    },
    "life_joy_pain_affirmation": {
        "kind": "interpretive_axis", "status": "ambiguous",
        "statement": "Life, joy, pain, and affirmation form a distinct candidate axis without asserting semantic identity with eternal return.",
    },
    "poetic_symbolic_affirmation": {
        "kind": "competing_reading", "status": "ambiguous",
        "statement": "The ring, song, dance, and eternity formulas can be read as poetic-symbolic affirmation without deciding a cosmology.",
    },
    "amor_fati_cross_work": {
        "kind": "blocked_cross_work_relation_slot", "status": "deferred",
        "statement": "A relation to amor fati is reserved for cross-work evidence and is not established by this Zarathustra-only dossier.",
    },
    "distinguished_from_return_as_action": {
        "kind": "candidate_relation", "status": "proposed",
        "statement": "Return to human beings in Prologue 1 is distinguished from the later eternal-return candidate.",
    },
}

METHOD_CONTROLS = {
    "bare_again_is_not_membership": "Bare again/once-more vocabulary does not establish eternal return.",
    "isolated_eternity_is_not_membership": "Eternity vocabulary outside a recurrence argument remains ambiguous rather than core.",
    "ring_without_recurrence_is_not_membership": "Ring imagery without a recurrence link does not establish eternal return.",
    "liturgical_eternity_is_not_membership": "A liturgical or rhetorical eternity formula does not become doctrine by lexical recurrence alone.",
    "speaker_flattening_forbidden": "Narrator, Zarathustra, dwarf, animals, and chorus must not be collapsed into one unmarked speaker.",
    "alignment_proposal_is_not_semantic_equivalence": "A machine DE/RU paragraph alignment does not establish translation or semantic equivalence.",
}

ALIGNMENT_GAPS = [
    {
        "alignment_ref": "tos.translation-alignment.sid-6922c0b268a890cf94d0454b67143f43",
        "reading_ref": "p3.r2", "gap_code": "one_to_many_mapping",
    },
    {
        "alignment_ref": "tos.translation-alignment.sid-a1f9250a6975af780fb35023b6070ff2",
        "reading_ref": "p3.r13", "gap_code": "ru_formula_broken_by_letterspacing",
    },
    {
        "alignment_ref": "tos.translation-alignment.sid-a474f471a32163007afa33b3ae624ab9",
        "reading_ref": "p3.r13", "gap_code": "ru_formula_blocked_by_ocr_substitution",
    },
    {
        "alignment_ref": "tos.translation-alignment.sid-b3ce5734f20040829abd1acbbf5c17bd",
        "reading_ref": "p3.r15", "gap_code": "machine_target_gap_not_translation_omission",
    },
    {
        "alignment_ref": "tos.translation-alignment.sid-f7e6b8c00ed9861bf01841a9b0212e78",
        "reading_ref": "p3.r16", "gap_code": "one_to_many_mapping",
    },
]


class BuildError(RuntimeError):
    pass


def jb(value: Any, pretty: bool = True) -> bytes:
    text = json.dumps(
        value, ensure_ascii=False, sort_keys=True,
        indent=2 if pretty else None,
        separators=None if pretty else (",", ":"),
    )
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


def verify_inputs(plan: dict[str, Any]) -> None:
    for name, record in plan["inputs"].items():
        path = REPO / record["ref"]
        if not path.is_file() or sha_file(path) != record["sha256"]:
            raise BuildError(f"input drift: {name}")


def import_previous_builder() -> Any:
    path = REPO / PREVIOUS_BUILDER
    spec = importlib.util.spec_from_file_location("tos_eternal_return_lexical_source", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def contiguous_count(tokens: list[str], pattern: tuple[str, ...]) -> int:
    width = len(pattern)
    return sum(tuple(tokens[i:i + width]) == pattern for i in range(len(tokens) - width + 1))


def signal_codes(de: list[str], ru: list[str]) -> list[str]:
    signals: set[str] = set()
    rules = {
        "de_recurrence_noun": any(x.startswith("wiederkunft") for x in de),
        "de_recurrence_verb": any(x.startswith("wiederkehr") for x in de),
        "de_eternity": any(x.startswith("ewig") for x in de),
        "de_ring": any(x.startswith("ring") for x in de),
        "de_time": any(x.startswith("zeit") for x in de),
        "de_moment": any(x.startswith("augenblick") for x in de),
        "de_joy": any(x.startswith("freud") for x in de),
        "de_pain": any(x.startswith(("schmerz", "leid")) for x in de),
        "de_love": any(x.startswith("lieb") for x in de),
        "de_life": any(x.startswith("leb") for x in de),
        "de_affirmation": "ja" in de or any(x.startswith("bejah") for x in de),
        "de_universal_scope": any(x in {"all", "alle", "allen", "aller", "alles"} or x.startswith(("gleich", "selb")) for x in de),
        "ru_recurrence": any(x.startswith("возвращ") for x in ru),
        "ru_eternity": any(x.startswith("вечн") for x in ru),
        "ru_ring": any(x.startswith("кольц") for x in ru),
        "ru_time": any(x.startswith("врем") for x in ru),
        "ru_moment": any(x.startswith("мгновен") for x in ru),
        "ru_joy": any(x.startswith("радост") for x in ru),
        "ru_pain": any(x.startswith(("боль", "страд")) for x in ru),
        "ru_love": any(x.startswith("люб") for x in ru),
        "ru_life": any(x.startswith("жизн") for x in ru),
        "ru_affirmation": "да" in ru or any(x.startswith("утвержд") for x in ru),
        "ru_universal_scope": any(x in {"все", "всё", "всего", "вся"} or x.startswith(("одинаков", "тотже")) for x in ru),
    }
    signals.update(name for name, matched in rules.items() if matched)
    return sorted(signals)


def exclusion_code(row: dict[str, Any]) -> str | None:
    de, ru = row["de"], row["ru"]
    if row["reading"] == "p1.r1" and "wieder" in de and "mensch" in de:
        return "return_to_humans_not_eternal_return"
    if row["reading"] == "p3.r9" and (
        any(x.startswith("heimkehr") for x in de)
        or any(x.startswith("возврат") for x in ru)
    ):
        return "return_home_not_eternal_return"
    return None


def evidence_class(row: dict[str, Any], signals: list[str]) -> str | None:
    if exclusion_code(row):
        return "excluded"
    recurrence = any(x in signals for x in (
        "de_recurrence_noun", "de_recurrence_verb", "ru_recurrence",
    ))
    eternity = any(x in signals for x in ("de_eternity", "ru_eternity"))
    ring = any(x in signals for x in ("de_ring", "ru_ring"))
    universal = any(x in signals for x in ("de_universal_scope", "ru_universal_scope"))
    explicit_noun = "de_recurrence_noun" in signals
    eternal_again = contiguous_count(row["de"], FORMULAS["de_eternal_again"][1]) > 0
    if row["reading"] in CORE_READINGS:
        if explicit_noun or eternal_again or (recurrence and (eternity or ring or universal)):
            return "core"
        if signals:
            return "supporting"
    elif recurrence or eternity:
        return "ambiguous"
    return None


def hydrate_units() -> list[dict[str, Any]]:
    previous = import_previous_builder()
    units, _de, _ru, _census = previous.load_parallel()
    by_id = {x["alignment_id"]: x for x in units}
    spine = {x["alignment_id"]: x for x in load_jsonl(ALIGN / "alignment-spine.v1.jsonl")}
    caches: dict[str, str] = {}
    for part in range(1, 5):
        packet = load_json(ALIGN / f"part-{part}.translation-alignment-packet.v1.json")
        source = dict(packet["source_side"])
        target = dict(packet["target_side"])
        source["anchor_map"] = {x["anchor_ref"]: x for x in source["anchors"]}
        target["anchor_map"] = {x["anchor_ref"]: x for x in target["anchors"]}
        for alignment in packet["alignments"]:
            aid = alignment["alignment_id"]
            row = by_id[aid]
            de_texts = [previous.slice_anchor(source, x, caches) for x in alignment["ordered_source_anchor_refs"]]
            ru_texts = [previous.slice_anchor(target, x, caches) for x in alignment["ordered_target_anchor_refs"]]
            row.update({
                "source_anchor_refs": alignment["ordered_source_anchor_refs"],
                "target_anchor_refs": alignment["ordered_target_anchor_refs"],
                "source_paragraph_unit_refs": spine[aid]["source_paragraph_unit_refs"],
                "target_paragraph_unit_refs": spine[aid]["target_paragraph_unit_refs"],
                "de_text": "\n".join(de_texts),
                "ru_text": "\n".join(ru_texts),
            })
    def reading_order(row: dict[str, Any]) -> tuple[int, int, str]:
        raw = row["reading"].split("r", 1)[1]
        return row["part"], int(raw) if raw.isdigit() else 10_000, row["alignment_id"]

    return sorted(units, key=reading_order)


def claim_codes(row: dict[str, Any], signals: list[str], klass: str) -> list[str]:
    result: set[str] = set()
    recurrence = any(x.endswith("recurrence") or "recurrence_" in x for x in signals)
    if klass == "core":
        result.update({"cosmological_recurrence", "poetic_symbolic_affirmation"})
    if row["reading"] == "p3.r2":
        result.add("existential_test_and_affirmation")
    if any(x.endswith(("ring", "time", "moment")) for x in signals):
        if any(x.endswith(("time", "moment")) for x in signals):
            result.add("temporality_and_augenblick")
        if any(x.endswith("ring") for x in signals):
            result.add("ring_and_wholeness")
    if any(x.endswith(("life", "joy", "pain", "love", "affirmation")) for x in signals):
        result.add("life_joy_pain_affirmation")
    if klass == "excluded" and exclusion_code(row) == "return_to_humans_not_eternal_return":
        result.add("distinguished_from_return_as_action")
    if recurrence and klass == "core":
        result.add("existential_test_and_affirmation")
    return sorted(result)


def identity_bindings(units: list[dict[str, Any]]) -> list[tuple[str, str]]:
    rows: list[tuple[str, str]] = [("annotation", "eternal-return-dossier-v1")]
    for code in FORMULAS:
        rows.append(("formula", code))
    for code in CLAIM_SPECS:
        rows.append(("template", code))
    for row in units:
        signals = signal_codes(row["de"], row["ru"])
        klass = evidence_class(row, signals)
        if klass:
            rows.append(("evidence", f"{klass}|{row['alignment_id']}"))
    return sorted(rows)


def issue(bindings: list[tuple[str, str]]) -> None:
    target = REPO / ISSUANCE_REF
    if target.exists():
        raise BuildError("identity issuance already exists; refusing to remint")
    prefix = {
        "annotation": "tos.annotation.eternal-return-candidate.sid-",
        "formula": "tos.annotation.formula-candidate.sid-",
        "template": "tos.annotation.interpretation-template.sid-",
        "evidence": "tos.annotation.semantic-evidence-candidate.sid-",
    }
    rows = []
    for kind, binding in bindings:
        rows.append({
            "kind": kind, "binding": binding,
            "id": prefix[kind] + secrets.token_hex(16),
        })
    payload = {
        "schema_version": "tos_eternal_return_candidate_identity_issuance_v1",
        "identity_policy": "opaque-id-independent-of-source-text-label-translation-and-current-interpretation",
        "issued_at": load_json(PLAN_REF)["frozen_at"],
        "records": rows,
    }
    write(ISSUANCE_REF, jb(payload))


def identity_map(bindings: list[tuple[str, str]]) -> dict[tuple[str, str], str]:
    path = REPO / ISSUANCE_REF
    if not path.is_file():
        raise BuildError("missing identity issuance; run --build --issue-identities once")
    value = load_json(ISSUANCE_REF)
    result = {(x["kind"], x["binding"]): x["id"] for x in value["records"]}
    if set(result) != set(bindings):
        raise BuildError("identity issuance binding mismatch")
    if len(set(result.values())) != len(result):
        raise BuildError("duplicate opaque identity")
    return result


def build_analysis(units: list[dict[str, Any]], ids: dict[tuple[str, str], str]) -> dict[str, Any]:
    annotation_id = ids[("annotation", "eternal-return-dossier-v1")]
    evidence, private_evidence, exclusions = [], [], []
    claim_evidence: dict[str, list[str]] = defaultdict(list)
    formula_evidence: dict[str, list[str]] = defaultdict(list)
    formula_counts: Counter[str] = Counter()
    for row in units:
        signals = signal_codes(row["de"], row["ru"])
        klass = evidence_class(row, signals)
        for code, (language, pattern) in FORMULAS.items():
            count = contiguous_count(row[language], pattern)
            if count:
                formula_counts[code] += count
                if klass:
                    formula_evidence[code].append(row["alignment_id"])
        if not klass:
            continue
        binding = f"{klass}|{row['alignment_id']}"
        evidence_id = ids[("evidence", binding)]
        claims = claim_codes(row, signals, klass)
        for code in claims:
            claim_evidence[code].append(evidence_id)
        tracked = {
            "schema_version": "tos_zarathustra_eternal_return_evidence_candidate_v1",
            "evidence_id": evidence_id,
            "annotation_ref": annotation_id,
            "evidence_class": klass,
            "part": row["part"],
            "reading_ref": row["reading"],
            "alignment_ref": row["alignment_id"],
            "alignment_status": row["status"],
            "alignment_shape": row["shape"],
            "source_anchor_refs": row["source_anchor_refs"],
            "target_anchor_refs": row["target_anchor_refs"],
            "source_paragraph_unit_refs": row["source_paragraph_unit_refs"],
            "target_paragraph_unit_refs": row["target_paragraph_unit_refs"],
            "de_exact_sha256": h(row["de_text"]),
            "ru_exact_sha256": h(row["ru_text"]),
            "signal_codes": signals,
            "interpretation_template_refs": [ids[("template", x)] for x in claims],
            "positive_evidence_eligible": row["positive_evidence_eligible"],
            "speaker_sensitive": row["reading"] in {"p3.r2", "p3.r13", "p4.r19"},
            "speaker_attribution_status": "unresolved_source_visible_review_required",
            "accepted": False,
            "review_refs": [],
            "graph_effect": False,
            "canon_effect": False,
            "source_text_included": False,
        }
        evidence.append(tracked)
        private_evidence.append({
            **tracked,
            "de_text": row["de_text"], "ru_text": row["ru_text"],
            "de_tokens": row["de"], "ru_tokens": row["ru"],
            "exclusion_code": exclusion_code(row),
        })
        if klass == "excluded":
            exclusions.append({
                "schema_version": "tos_zarathustra_eternal_return_exclusion_v1",
                "evidence_ref": evidence_id,
                "annotation_ref": annotation_id,
                "exclusion_code": exclusion_code(row),
                "reading_ref": row["reading"],
                "alignment_ref": row["alignment_id"],
                "accepted": False,
                "review_refs": [],
                "semantic_equivalence_asserted": False,
                "graph_effect": False,
                "canon_effect": False,
            })

    for code, rationale in METHOD_CONTROLS.items():
        exclusions.append({
            "schema_version": "tos_zarathustra_eternal_return_exclusion_v1",
            "control_kind": "method_rule",
            "control_code": code,
            "rationale": rationale,
            "evidence_ref": None,
            "annotation_ref": annotation_id,
            "accepted": False,
            "review_refs": [],
            "semantic_equivalence_asserted": False,
            "graph_effect": False,
            "canon_effect": False,
        })

    formulas = []
    private_formulas = []
    for code, (language, pattern) in FORMULAS.items():
        formula_id = ids[("formula", code)]
        refs = sorted(set(formula_evidence[code]))
        formulas.append({
            "schema_version": "tos_zarathustra_formula_candidate_v1",
            "formula_candidate_id": formula_id,
            "annotation_ref": annotation_id,
            "formula_code": code,
            "language": language,
            "token_width": len(pattern),
            "occurrence_count": formula_counts[code],
            "aligned_evidence_unit_count": len(refs),
            "alignment_refs": refs,
            "status": "proposed" if formula_counts[code] >= 3 else "ambiguous",
            "accepted": False,
            "review_refs": [],
            "source_text_included": False,
            "graph_effect": False,
            "canon_effect": False,
        })
        private_formulas.append({
            "formula_candidate_id": formula_id,
            "formula_code": code,
            "language": language,
            "normalized_tokens": list(pattern),
            "display_formula": " ".join(pattern),
            "occurrence_count": formula_counts[code],
            "alignment_refs": refs,
        })

    claims = []
    for code, spec in CLAIM_SPECS.items():
        refs = sorted(set(claim_evidence[code]))
        claims.append({
            "schema_version": "tos_zarathustra_interpretation_template_v1",
            "interpretation_template_id": ids[("template", code)],
            "annotation_ref": annotation_id,
            "claim_code": code,
            "claim_kind": spec["kind"],
            "candidate_statement": spec["statement"],
            "status": spec["status"] if refs else "deferred",
            "evidence_refs": refs,
            "accepted": False,
            "review_refs": [],
            "human_judgment": False,
            "materialized_claim": False,
            "semantic_fact_asserted": False,
            "graph_effect": False,
            "canon_effect": False,
        })

    all_anchor_refs = sorted({x for row in evidence for x in row["source_anchor_refs"] + row["target_anchor_refs"]})
    concept = {
        "schema_version": "tos_sign_annotation_v1",
        "annotation_id": annotation_id,
        "sign_layer": "concept_candidate",
        "assertion_layer": "semantic_interpretation",
        "anchor_refs": all_anchor_refs,
        "body": {
            "candidate_type": "semantic_concept_dossier",
            "labels": {
                "de": "ewige Wiederkunft",
                "ru": "вечное возвращение",
                "en": "eternal return",
                "identity_dependency": False
            },
            "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
            "languages": ["de", "ru"],
            "parts": [1, 2, 3, 4],
            "core_readings": sorted(CORE_READINGS),
            "inclusion_law": "include source-addressed recurrence formulas and their eternity/ring/time/affirmation neighborhood; retain lexical neighbors as ambiguous and declared local-return passages as excluded controls",
            "interpretation_template_refs": [ids[("template", x)] for x in CLAIM_SPECS],
            "formula_candidate_refs": [ids[("formula", x)] for x in FORMULAS],
            "competing_reading_refs": [ids[("template", x)] for x in (
                "cosmological_recurrence", "existential_test_and_affirmation", "poetic_symbolic_affirmation"
            )],
            "blocked_cross_work_relation_ref": ids[("template", "amor_fati_cross_work")],
            "distinguished_from_refs": [
                "ToS/canon/principle/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/return-as-action/node.json"
            ],
            "sign_id": None,
            "concept_id": None,
            "materialized_claim_refs": [],
            "independent_agent_audit_ref": str(OUTPUTS["audit"]),
            "semantic_fact_asserted": False,
            "graph_effect": False,
            "canon_effect": False
        },
        "source_refs": [
            str(ALIGN / "manifest.v1.json"),
            str(PLAN_REF)
        ],
        "maker": {
            "maker_type": "model",
            "agent_ref": "codex-internal-agents.eternal-return-dossier-v1"
        },
        "provenance_event_ref": "tos.event.zarathustra-eternal-return-concept-candidate-v1.build",
        "epistemic_status": "uncertain",
        "alternatives": [
            {"interpretation_template_ref": ids[("template", "cosmological_recurrence")]},
            {"interpretation_template_ref": ids[("template", "existential_test_and_affirmation")]},
            {"interpretation_template_ref": ids[("template", "poetic_symbolic_affirmation")]}
        ],
        "review_status": "unreviewed",
        "review_refs": [],
        "annotation_version": 1,
        "supersedes_annotation_ref": None
    }
    return {
        "concept": concept,
        "evidence": evidence,
        "formulas": formulas,
        "claims": claims,
        "exclusions": exclusions,
        "private": {
            "schema_version": "tos_zarathustra_eternal_return_private_analysis_v1",
            "annotation_id": annotation_id,
            "content_posture": "private_exact_source_return_and_mutable_analysis_not_semantic_authority",
            "evidence": private_evidence,
            "formulas": private_formulas,
            "claim_specs": CLAIM_SPECS,
        },
    }


def generate(include_ids: bool) -> tuple[dict[Path, bytes], bytes, list[tuple[str, str]], dict[str, Any]]:
    plan = load_json(PLAN_REF)
    verify_inputs(plan)
    units = hydrate_units()
    bindings = identity_bindings(units)
    if not include_ids:
        return {}, b"", bindings, {"unit_count": len(units)}
    ids = identity_map(bindings)
    analysis = build_analysis(units, ids)
    evidence = analysis["evidence"]
    formulas = analysis["formulas"]
    claims = analysis["claims"]
    exclusions = analysis["exclusions"]
    annotation_id = analysis["concept"]["annotation_id"]
    status_counts = Counter(x["evidence_class"] for x in evidence)
    part_counts = Counter(str(x["part"]) for x in evidence)
    reading_counts = Counter(x["reading_ref"] for x in evidence)
    source_anchors = {x for row in evidence for x in row["source_anchor_refs"]}
    target_anchors = {x for row in evidence for x in row["target_anchor_refs"]}
    bilingual_count = sum(bool(x["source_anchor_refs"] and x["target_anchor_refs"]) for x in evidence)
    coverage = {
        "schema_version": "tos_zarathustra_eternal_return_coverage_receipt_v1",
        "annotation_ref": annotation_id,
        "corpus_parts_scanned": 4,
        "alignment_units_scanned": len(units),
        "selected_evidence_unit_count": len(evidence),
        "evidence_class_counts": dict(sorted(status_counts.items())),
        "part_counts": dict(sorted(part_counts.items())),
        "reading_counts": dict(sorted(reading_counts.items())),
        "distinct_source_anchor_count": len(source_anchors),
        "distinct_target_anchor_count": len(target_anchors),
        "core_readings_declared": sorted(CORE_READINGS),
        "negative_control_count": len(exclusions),
        "source_return_verified_during_build": True,
        "bilingual_selected_evidence_count": bilingual_count,
        "one_sided_scope_residue_count": len(evidence) - bilingual_count,
        "de_ru_parallel_evidence_complete": bilingual_count == len(evidence),
        "parallel_coverage_posture": "preserve one-sided alignment residue without treating it as bilingual support",
        "independent_alignment_gap_count": len(ALIGNMENT_GAPS),
        "independent_alignment_gap_refs": [x["alignment_ref"] for x in ALIGNMENT_GAPS],
        "unselected_units_are_not_negative_findings": True,
    }
    summary = {
        "schema_version": "tos_zarathustra_eternal_return_candidate_summary_v1",
        "annotation_id": annotation_id,
        "evidence_unit_count": len(evidence),
        "evidence_class_counts": dict(sorted(status_counts.items())),
        "formula_candidate_count": len(formulas),
        "interpretation_template_count": len(claims),
        "exclusion_count": len(exclusions),
        "accepted_candidate_count": 0,
        "human_review_count": 0,
        "sign_identity_asserted": False,
        "concept_identity_asserted": False,
        "semantic_fact_asserted": False,
        "translation_equivalence_asserted": False,
        "graph_effect": False,
        "canon_effect": False,
    }
    promotion = {
        "schema_version": "tos_zarathustra_eternal_return_promotion_readiness_v1",
        "annotation_ref": annotation_id,
        "semantic_ladder_contract_ref": "ToS/contracts/semantic-ladder-packet.schema.json",
        "semantic_ladder_packet_materialized": False,
        "semantic_ladder_posture": "witness-local child packets remain future lifecycle controllers; this bilingual umbrella does not impersonate them",
        "blocked_semantic_ladder_stages": [
            "stable_sign_candidate",
            "manual_confirmation_or_rejection",
            "relations_between_signs",
            "conceptual_interpretations",
            "competing_readings",
            "graph_projection",
        ],
        "ready_for_source_visible_semantic_review": bool(status_counts["core"] and exclusions),
        "ready_for_sign_or_concept_identity_issuance": False,
        "ready_for_canon": False,
        "ready_for_graph_projection": False,
        "blockers": [
            "human semantic review has not occurred",
            "competing cosmological, existential, and poetic-symbolic readings remain open",
            "paragraph alignments are machine proposals rather than accepted translation mappings",
            "lexical and morphology inputs remain candidate layers",
            "exact DE/RU passages require source-visible review against both witnesses",
            "two central Russian formulas in p3.r13 are detector gaps caused by letterspacing and OCR substitution and must not be silently corrected",
            "a relation to amor fati is deferred because it is not established by this scoped Zarathustra dossier",
        ],
        "next_owner_route": "ToS/review-ledger after a source-visible semantic review; only an accepted review may route a new sign or concept identity toward ToS/canon",
    }
    provenance_rows = [
        {
            "schema_version": "tos_zarathustra_eternal_return_provenance_event_v1",
            "event_id": "tos.event.zarathustra-eternal-return-concept-candidate-v1.plan",
            "event_type": "plan_frozen",
            "event_at": plan["frozen_at"],
            "input_refs": [x["ref"] for x in plan["inputs"].values()],
            "output_refs": [str(PLAN_REF)],
            "authority_effect": "none",
        },
        {
            "schema_version": "tos_zarathustra_eternal_return_provenance_event_v1",
            "event_id": "tos.event.zarathustra-eternal-return-concept-candidate-v1.build",
            "event_type": "candidate_dossier_built",
            "event_at": plan["frozen_at"],
            "input_refs": [str(PLAN_REF), str(ISSUANCE_REF), str(ALIGN / "manifest.v1.json")],
            "output_refs": [str(x) for x in OUTPUTS.values()] + [str(PRIVATE)],
            "authority_effect": "candidate_only_zero_graph_and_canon_effect",
        },
    ]
    outputs: dict[Path, bytes] = {
        OUTPUTS["concept"]: jb(analysis["concept"]),
        OUTPUTS["evidence"]: jlb(evidence),
        OUTPUTS["formulas"]: jlb(formulas),
        OUTPUTS["templates"]: jlb(claims),
        OUTPUTS["exclusions"]: jlb(exclusions),
        OUTPUTS["promotion"]: jb(promotion),
        OUTPUTS["coverage"]: jb(coverage),
        OUTPUTS["summary"]: jb(summary),
        OUTPUTS["provenance"]: jlb(provenance_rows),
    }
    audit = {
        "schema_version": "tos_zarathustra_eternal_return_independent_agent_audit_v1",
        "annotation_ref": annotation_id,
        "audit_kind": "two_independent_codex_agent_challengers",
        "human_review": False,
        "semantic_design_challenger": {
            "artifact_sha256": {
                "schema_fit": "91a685a2f205bf9868cfffc33c233eb742bee712bd81062ccbf23ba343e0435f",
                "claim_relation_design": "8d2c061fc9ead09c0b1553aec2db3c2b46cffb13be730ac8ea0c62f079667ddb",
                "negative_controls": "db9ccabc7dd3c665e7fa93f34b960e0991d7866290be147789a2448bcd3721bc",
            },
            "route_fit_confirmed": True,
            "sign_annotation_concept_candidate_selected": True,
            "semantic_annotation_v2_rejected_for_bilingual_umbrella": True,
            "interpretations_materialized_as_claims": False,
            "five_axes_preserved": True,
            "amor_fati_cross_work_slot_blocked": True,
            "graph_must_remain_empty": True,
        },
        "source_evidence_challenger": {
            "manifest_sha256": "bc3ab711c1de33110ed073948abb14f9cee027cffd0677a51a499466a0b5095e",
            "dossier_sha256": "92e81e1e85dcf8fd4818934517b04778bcd38f1891c35e3f0759a34e2b2832c6",
            "alignment_groups_scanned": 3423,
            "source_paragraph_anchors_visited": 3447,
            "target_paragraph_anchors_visited": 3569,
            "occurrence_candidate_count": 791,
            "occurrence_class_counts": {
                "core": 127, "supporting": 146, "ambiguous": 161, "excluded": 357,
            },
            "whole_work_lexical_seed_occurrence_count": 823,
            "passage_candidate_count": 171,
            "formula_correspondence_count": 34,
            "alignment_gaps": ALIGNMENT_GAPS,
            "deterministic_check_passed": True,
        },
        "integrated_controls": {
            "five_interpretive_axes_are_separate": True,
            "interpretations_are_templates_not_claims": True,
            "bare_again_eternity_ring_and_liturgical_controls_present": True,
            "speaker_flattening_forbidden": True,
            "one_sided_alignment_residue_preserved": True,
            "ocr_and_letterspacing_gaps_not_silently_corrected": True,
            "accepted_candidate_count": 0,
            "human_review_count": 0,
            "graph_effect": False,
            "canon_effect": False,
        },
        "comparison_limit": "the source challenger counts anchor occurrences and passages while the primary dossier counts selected alignment units; their denominators are intentionally not collapsed",
        "authority_boundary": "independent model challenge and integration receipt, not human semantic review or acceptance",
    }
    outputs[OUTPUTS["audit"]] = jb(audit)
    private_bytes = jb(analysis["private"])
    artifact_rows = []
    for name, path in OUTPUTS.items():
        if name == "manifest":
            continue
        artifact_rows.append({"role": name, "ref": str(path), "sha256": sha_bytes(outputs[path])})
    manifest = {
        "schema_version": "tos_zarathustra_eternal_return_candidate_manifest_v1",
        "route_id": "zarathustra-eternal-return-concept-candidate-v1",
        "plan_ref": str(PLAN_REF),
        "plan_sha256": sha_file(REPO / PLAN_REF),
        "identity_issuance_ref": str(ISSUANCE_REF),
        "identity_issuance_sha256": sha_file(REPO / ISSUANCE_REF),
        "generator_ref": str(GENERATOR_REF),
        "generator_sha256": sha_file(REPO / GENERATOR_REF),
        "artifacts": artifact_rows,
        "private_artifact": {
            "ref": str(PRIVATE), "sha256": sha_bytes(private_bytes),
            "mode": "0600", "tracked": False,
        },
        "accepted_candidate_count": 0,
        "human_review_count": 0,
        "graph_effect": False,
        "canon_effect": False,
    }
    outputs[OUTPUTS["manifest"]] = jb(manifest)
    return outputs, private_bytes, bindings, summary


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
    _outputs, _private, bindings, _summary = generate(False)
    if issue_identities:
        issue(bindings)
    identity_map(bindings)
    outputs, private, _bindings, summary = generate(True)
    for path, payload in outputs.items():
        write(path, payload)
    write(PRIVATE, private, 0o600)
    return summary


def check() -> dict[str, Any]:
    outputs, private, bindings, summary = generate(True)
    identity_map(bindings)
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


def preview() -> dict[str, Any]:
    _outputs, _private, bindings, summary = generate(False)
    return {"identity_count": len(bindings), **summary}


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
            result = preview()
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

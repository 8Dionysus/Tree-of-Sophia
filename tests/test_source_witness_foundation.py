from __future__ import annotations

import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = REPO_ROOT / "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1"
PRIVATE_HANDOFF_PATH = (
    REPO_ROOT
    / "ToS/research-packets/foundation-laboratory-2026-07/"
    "private-evidence-handoff.v1.json"
)
GERMAN_ASSISTED_REVIEW_PATH = (
    GOLD_ROOT / "german-assisted-source-review.v1.json"
)
CRITICAL_EDITION_WITNESS_PATH = (
    GOLD_ROOT / "critical-edition-witness.ekgwb.za-i-vorrede-1.v1.json"
)
GERMAN_SOURCE_TRIANGULATION_PATH = (
    GOLD_ROOT
    / "german-source-triangulation."
    "ekgwb-dta-naumann.za-i-vorrede-1.v1.json"
)
BOUNDED_TRANSLATION_RESEARCH_INPUT_PATH = (
    GOLD_ROOT
    / "bounded-translation-research-input."
    "za-i-vorrede-1-opening-sentence.v1.json"
)
EKGWB_RIGHTS_PATH = (
    GOLD_ROOT / "rights.ekgwb.za-i-vorrede-1.v1.json"
)
MYSL_WORK_BOUNDARY_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/collections/friedrich-nietzsche/"
    "works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries"
)
JENSEITS_1886_ITEM_ROOT = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "jenseits-von-gut-und-boese/expressions/de-naumann-1886/"
    "editions/leipzig-c-g-naumann-1886/items/"
    "internet-archive-google-harvard-scan-pdf"
)
JENSEITS_1886_DISCOVERY_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/discovery/runs/"
    "jenseits-naumann-1886-open-scan-witness.2026-07-28.v1.json"
)
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import validate_source_witness_foundation as foundation
import build_zarathustra_german_source_triangulation as triangulation_builder
import build_zarathustra_bounded_translation_input as bounded_input_builder


PRE_DRAFT_STAGE_NAMES = [
    "morphology",
    "syntax",
    "polysemy_unusual_forms_and_repetition",
    "historical_german_senses",
    "sourced_etymology",
    "intra_zarathustra_parallels",
    "nietzsche_corpus_parallels",
    "evidenced_allusions_idioms_and_cultural_links",
    "literal_interlinear",
]

TRANSLATION_EVALUATION_AXES = [
    "primary_meaning",
    "secondary_meanings",
    "etymological_connection",
    "polysemy",
    "terminological_consistency",
    "recurring_signs",
    "metaphor",
    "image",
    "syntax",
    "rhythm",
    "spoken_pronounceability",
    "intentional_strangeness",
    "cultural_allusion",
    "omissions",
    "additions",
    "stylistic_smoothing",
    "unjustified_embellishment",
    "recognized_translation_influence",
    "decision_explainability",
]

SEMANTIC_STAGE_ORDER = [
    "exact_form",
    "frequency_and_concordance",
    "context",
    "morphology",
    "lemma",
    "recurrence_within_section",
    "recurrence_within_work",
    "recurrence_within_author_corpus",
    "translation_correspondences",
    "stable_sign_candidate",
    "manual_confirmation_or_rejection",
    "relations_between_signs",
    "conceptual_interpretations",
    "competing_readings",
    "graph_projection",
]


def _synthetic_pre_draft_packet(*, lane: str = "human_only") -> dict:
    maker_type = "human" if lane == "human_only" else "model"
    findings = []
    for index, stage_name in enumerate(PRE_DRAFT_STAGE_NAMES, start=1):
        reference_entry_ids = []
        citations = []
        if stage_name == "sourced_etymology":
            reference_entry_ids = ["tos-ref.synthetic-etymology"]
            citations = [
                {
                    "citation_id": "tos-citation.synthetic-etymology",
                    "reference_entry_id": "tos-ref.synthetic-etymology",
                    "locator": "synthetic dictionary entry",
                    "evidence_url": "https://example.invalid/dictionary/entry",
                    "accessed_at": "2026-07-23T00:00:00Z",
                    "rights_posture": "citation-only synthetic test fixture",
                    "content_stored": "citation-only",
                }
            ]
        findings.append(
            {
                "stage": stage_name,
                "status": "frozen",
                "findings": [
                    {
                        "finding_id": f"tos-finding.synthetic-{index}",
                        "statement": f"Synthetic finding for stage {index}.",
                        "source_anchor_refs": ["tos.anchor.synthetic-source"],
                        "reference_entry_ids": reference_entry_ids,
                        "citations": citations,
                        "epistemic_status": (
                            "sourced" if stage_name == "sourced_etymology" else "observed"
                        ),
                        "confidence": "not-applicable",
                        "maker": {
                            "maker_type": maker_type,
                            "agent_ref": f"synthetic-{maker_type}",
                        },
                        "provenance_event_ref": "tos.event.synthetic-analysis",
                        "review_status": (
                            "human-reviewed" if lane == "human_only" else "unreviewed"
                        ),
                    }
                ],
                "source_return_verified": True,
                "frozen_at": "2026-07-23T00:00:00Z",
                "review_status": (
                    "human-reviewed" if lane == "human_only" else "unreviewed"
                ),
            }
        )

    is_human = lane == "human_only"
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-pre-draft-analysis.schema.json",
        "schema_version": "tos_translation_pre_draft_analysis_v1",
        "packet_id": f"tos.translation-pre-draft-analysis.synthetic-{lane.replace('_', '-')}",
        "experiment_id": "tos-translation-foundation-v1",
        "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
        "source_expression_ref": "tos.expression.synthetic-german",
        "source_anchor_refs": ["tos.anchor.synthetic-source"],
        "source_review_evidence": {
            "review_unit_id": "tos-translation-source-review-v2-001",
            "review_receipt_refs": ["synthetic-pass-1", "synthetic-pass-2"],
            "accepted_diplomatic_sha256": "a" * 64,
            "pass_1_performed_by_real_human": True,
            "pass_2_performed_by_real_human": True,
            "pass_timestamps_distinct": True,
            "source_acceptance": "accept",
        },
        "source_forms": {
            "diplomatic": "Synthetic source form.",
            "diplomatic_sha256": "a" * 64,
            "normalized": "Synthetic source form.",
            "normalization_posture": "none",
            "normalization_event_ref": "tos.event.synthetic-normalization",
        },
        "target_language": "ru",
        "lane": lane,
        "blindness": {
            "recognized_comparator_visible": False,
            "recognized_comparator_consulted": False,
            "other_lane_analysis_visible": False,
            "other_lane_drafts_visible": False,
            "translation_draft_created": False,
        },
        "maker": {
            "maker_type": maker_type,
            "agent_ref": f"synthetic-{maker_type}",
            "performed_by_real_human": is_human,
            "ai_assistance_used": not is_human,
            "human_editing_used": is_human,
            "model_refs": [] if is_human else ["synthetic-model"],
            "runtime_receipt_refs": [] if is_human else ["synthetic-runtime-receipt"],
        },
        "reference_register": {
            "ref": "ToS/source-witnesses/synthetic-reference-register.json",
            "sha256": "b" * 64,
            "status": "researched-not-content-admitted",
            "content_admission_is_per_citation": True,
        },
        "analysis_order": PRE_DRAFT_STAGE_NAMES,
        "stages": findings,
        "all_claims_return_to_source": True,
        "frequency_is_not_semantic_sufficiency": True,
        "model_memory_is_not_etymological_evidence": True,
        "rights_and_visibility": {
            "visibility": "local-only",
            "restricted_source_text_redistribution": False,
            "reference_rights_inherited_per_citation": True,
            "public_metadata_separate": True,
        },
        "packet_status": "frozen-ready-for-lane-draft",
        "provenance_event_refs": ["tos.event.synthetic-analysis"],
        "authority_boundary": "blind pre-draft linguistic and philological evidence only; this packet contains no translation draft, recognized comparator content, accepted etymology without citation, semantic sign decision, or graph claim",
        "packet_version": 1,
    }


def _synthetic_pre_draft_link(lane: str, suffix: str) -> dict:
    posture = {
        "human_only": "real-human-only",
        "ai_only": "model-only",
        "ai_alternative": "model-alternative-only",
    }[lane]
    return {
        "packet_id": f"tos.translation-pre-draft-analysis.synthetic-{suffix}",
        "ref": f"ToS/local-content/translation/synthetic-{suffix}.json",
        "sha256": "b" * 64,
        "lane": lane,
        "maker_posture": posture,
        "accepted_source_sha256": "a" * 64,
        "status": "frozen-ready-for-lane-draft",
        "frozen_at": "2026-07-23T01:00:00Z",
    }


def _synthetic_draft(
    draft_type: str,
    suffix: str,
    pre_draft_links: list[dict],
    *,
    input_drafts: list[dict] | None = None,
) -> dict:
    if draft_type == "human_only":
        maker = {
            "maker_type": "human",
            "agent_refs": ["human:synthetic-translator"],
            "performed_by_real_human": True,
            "ai_assistance_used": False,
            "human_editing_used": True,
            "model_refs": [],
            "prompt_receipt_refs": [],
            "runtime_receipt_refs": [],
            "human_intervention_refs": [],
        }
    elif draft_type == "ai_human":
        maker = {
            "maker_type": "human-ai-collaboration",
            "agent_refs": ["human:synthetic-translator", "model:synthetic-model"],
            "performed_by_real_human": True,
            "ai_assistance_used": True,
            "human_editing_used": True,
            "model_refs": ["synthetic-model"],
            "prompt_receipt_refs": ["synthetic-prompt-receipt"],
            "runtime_receipt_refs": ["synthetic-runtime-receipt"],
            "human_intervention_refs": ["synthetic-human-intervention"],
        }
    else:
        maker = {
            "maker_type": "model",
            "agent_refs": ["model:synthetic-model"],
            "performed_by_real_human": False,
            "ai_assistance_used": True,
            "human_editing_used": False,
            "model_refs": ["synthetic-model"],
            "prompt_receipt_refs": ["synthetic-prompt-receipt"],
            "runtime_receipt_refs": ["synthetic-runtime-receipt"],
            "human_intervention_refs": [],
        }
    return {
        "draft_id": f"tos-translation-draft.synthetic-{suffix}",
        "draft_type": draft_type,
        "text": f"Synthetic translation draft {suffix}.",
        "text_sha256": "c" * 64,
        "pre_draft_inputs": [
            {
                "packet_id": link["packet_id"],
                "lane": link["lane"],
                "sha256": link["sha256"],
                "status": link["status"],
            }
            for link in pre_draft_links
        ],
        "maker": maker,
        "recognized_comparator_visible": False,
        "other_lane_drafts_visible": draft_type == "ai_human",
        "input_drafts": input_drafts or [],
        "alternatives": [],
        "frozen_at": "2026-07-23T02:00:00Z",
        "status": "frozen",
        "provenance_event_ref": "tos.event.synthetic-translation-draft",
    }


def _synthetic_translation_packet() -> dict:
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/translation-packet.schema.json",
        "schema_version": "tos_translation_packet_v2",
        "packet_id": "tos.translation-packet.synthetic-foundation",
        "experiment_id": "tos-translation-foundation-v1",
        "laboratory_plan": {
            "ref": "ToS/local-content/translation/translation-laboratory-plan.json",
            "sha256": "d" * 64,
        },
        "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
        "source_expression_ref": "tos.expression.synthetic-german",
        "source_anchor_refs": ["tos.anchor.synthetic-source"],
        "source_review_evidence": {
            "review_unit_id": "tos-translation-source-review-v2-001",
            "review_receipt_refs": ["synthetic-pass-1", "synthetic-pass-2"],
            "accepted_diplomatic_sha256": "a" * 64,
            "pass_1_performed_by_real_human": True,
            "pass_2_performed_by_real_human": True,
            "pass_timestamps_distinct": True,
            "source_acceptance": "accept",
        },
        "source_language": "de",
        "target_language": "ru",
        "source_forms": {
            "diplomatic": "Synthetic accepted German source.",
            "diplomatic_sha256": "a" * 64,
            "normalized": "Synthetic accepted German source.",
            "normalized_sha256": "a" * 64,
            "normalization_posture": "none",
            "normalization_event_ref": "tos.event.synthetic-normalization",
        },
        "pre_draft_analysis": {
            "human_only": None,
            "ai_only": None,
            "ai_alternatives": [],
            "cross_lane_analysis_visible": False,
            "recognized_comparator_visible": False,
        },
        "drafts": [],
        "recognized_comparator": {
            "state": "sealed",
            "witnesses": [],
            "content_visible": False,
            "content_consulted": False,
            "revealed_at": None,
            "revealed_by_real_human_ref": None,
            "pre_reveal_freeze": {
                "human_only_draft_ref": None,
                "ai_only_draft_ref": None,
                "ai_alternative_draft_refs": [],
                "ai_human_draft_ref": None,
                "all_required_drafts_frozen": False,
                "freeze_set_sha256": None,
            },
            "post_reveal_change_ledger_required": True,
            "recognized_translation_is_ground_truth": False,
        },
        "alignments": [],
        "evaluation_axes": TRANSLATION_EVALUATION_AXES,
        "evaluations": [],
        "evaluation_completion": {
            "required_axis_count": 19,
            "reviewed_axis_count": 0,
            "all_required_axes_reviewed": False,
            "drafts_with_complete_axis_coverage": [],
        },
        "post_reveal_changes": [],
        "preserved_alternatives": [],
        "adjudications": [],
        "accepted_output": None,
        "personal_read_aloud_layer": {
            "state": "not-provided",
            "review_refs": [],
            "performed_by_real_human": False,
            "separate_from_philological_adjudication": True,
            "used_as_philological_ground_truth": False,
        },
        "rights_and_visibility": {
            "visibility": "local-only",
            "rights_record_refs": ["tos.rights.synthetic-source"],
            "restricted_source_text_redistribution": False,
            "recognized_translation_rights_inherited": True,
            "public_metadata_separate": True,
        },
        "packet_status": "preparing",
        "provenance_event_refs": ["tos.event.synthetic-translation-packet"],
        "authority_boundary": "a translation packet preserves source, independent blind analyses and drafts, comparator influence, alternatives, and human decisions; it does not make a recognized translation, model output, metric, or validator ground truth",
        "packet_version": 1,
    }


def _freeze_synthetic_translation_packet(packet: dict) -> dict:
    human_link = _synthetic_pre_draft_link("human_only", "human")
    ai_link = _synthetic_pre_draft_link("ai_only", "ai")
    alternative_links = [
        _synthetic_pre_draft_link("ai_alternative", "alternative-1"),
        _synthetic_pre_draft_link("ai_alternative", "alternative-2"),
    ]
    human_draft = _synthetic_draft("human_only", "human", [human_link])
    ai_draft = _synthetic_draft("ai_only", "ai", [ai_link])
    alternative_drafts = [
        _synthetic_draft("ai_alternative", "alternative-1", [alternative_links[0]]),
        _synthetic_draft("ai_alternative", "alternative-2", [alternative_links[1]]),
    ]
    ai_human_draft = _synthetic_draft(
        "ai_human",
        "ai-human",
        [human_link, ai_link],
        input_drafts=[
            {
                "role": "human_only",
                "draft_ref": human_draft["draft_id"],
                "draft_sha256": human_draft["text_sha256"],
                "status": "frozen",
            },
            {
                "role": "ai_only",
                "draft_ref": ai_draft["draft_id"],
                "draft_sha256": ai_draft["text_sha256"],
                "status": "frozen",
            },
        ],
    )
    packet["pre_draft_analysis"] = {
        "human_only": human_link,
        "ai_only": ai_link,
        "ai_alternatives": alternative_links,
        "cross_lane_analysis_visible": False,
        "recognized_comparator_visible": False,
    }
    packet["drafts"] = [
        human_draft,
        ai_draft,
        *alternative_drafts,
        ai_human_draft,
    ]
    packet["packet_status"] = "blind-drafts-frozen"
    return packet


def _reveal_synthetic_comparator(packet: dict) -> dict:
    drafts_by_type = {
        draft["draft_type"]: draft
        for draft in packet["drafts"]
        if draft["draft_type"] != "ai_alternative"
    }
    alternatives = [
        draft["draft_id"]
        for draft in packet["drafts"]
        if draft["draft_type"] == "ai_alternative"
    ]
    packet["recognized_comparator"] = {
        "state": "revealed-after-independent-freeze",
        "witnesses": [
            {
                "reference_entry_id": "tos-ref.synthetic-recognized-translation",
                "expression_ref": "tos.expression.synthetic-russian-comparator",
                "item_ref": "tos.item.synthetic-russian-comparator",
                "anchor_refs": ["tos.anchor.synthetic-russian-comparator"],
                "rights_ref": "tos.rights.synthetic-russian-comparator",
                "authority_posture": "authoritative-witness-not-ground-truth",
            }
        ],
        "content_visible": True,
        "content_consulted": False,
        "revealed_at": "2026-07-23T03:00:00Z",
        "revealed_by_real_human_ref": "human:synthetic-reviewer",
        "pre_reveal_freeze": {
            "human_only_draft_ref": drafts_by_type["human_only"]["draft_id"],
            "ai_only_draft_ref": drafts_by_type["ai_only"]["draft_id"],
            "ai_alternative_draft_refs": alternatives,
            "ai_human_draft_ref": drafts_by_type["ai_human"]["draft_id"],
            "all_required_drafts_frozen": True,
            "freeze_set_sha256": "e" * 64,
        },
        "post_reveal_change_ledger_required": True,
        "recognized_translation_is_ground_truth": False,
    }
    packet["packet_status"] = "comparators-revealed"
    return packet


def _freeze_synthetic_comparison(packet: dict) -> dict:
    packet["alignments"] = [
        {
            "alignment_id": "synthetic-alignment",
            "source_anchor_refs": ["tos.anchor.synthetic-source"],
            "target_ref": packet["drafts"][0]["draft_id"],
            "mapping": "1:1",
            "status": "proposed",
            "maker_ref": "human:synthetic-reviewer",
            "provenance_event_ref": "tos.event.synthetic-alignment",
        }
    ]
    packet["post_reveal_changes"] = [
        {
            "change_id": f"synthetic-change-{index}",
            "pre_reveal_draft_ref": draft["draft_id"],
            "change_status": "unchanged",
            "post_reveal_text": draft["text"],
            "post_reveal_text_sha256": draft["text_sha256"],
            "consulted_comparator_reference_ids": [
                "tos-ref.synthetic-recognized-translation"
            ],
            "recognized_translation_influence": "none",
            "accepted_changes": [],
            "rejected_changes": ["Synthetic comparator wording not adopted."],
            "rationale": "Synthetic lifecycle fixture preserves the decision.",
            "maker_ref": "human:synthetic-reviewer",
            "provenance_event_ref": "tos.event.synthetic-post-reveal-comparison",
        }
        for index, draft in enumerate(packet["drafts"], start=1)
    ]
    packet["packet_status"] = "comparison-frozen"
    return packet


def _synthetic_semantic_maker(maker_type: str = "model") -> dict:
    is_human = maker_type == "human"
    return {
        "maker_type": maker_type,
        "agent_ref": f"{maker_type}:synthetic-semantic-worker",
        "performed_by_real_human": is_human,
        "ai_assistance_used": not is_human,
        "model_refs": [] if is_human else ["synthetic-semantic-model"],
    }


def _synthetic_semantic_stage(
    stage_name: str,
    *,
    status: str,
    body: dict | None = None,
    maker: dict | None = None,
    blocker_refs: list[str] | None = None,
) -> dict:
    active = status not in {"not-started", "blocked"}
    return {
        "stage": stage_name,
        "status": status,
        "source_anchor_refs": ["tos.anchor.synthetic-source"],
        "source_return_verified": active,
        "body": body or {},
        "maker": maker if active else None,
        "provenance_event_ref": "tos.event.synthetic-semantic-stage" if active else None,
        "review_status": "unreviewed" if active else "not-started",
        "blocker_refs": blocker_refs or [],
    }


def _synthetic_semantic_packet() -> dict:
    maker = _synthetic_semantic_maker()
    bodies = {
        "exact_form": {
            "exact_form": "Synthetic exact form",
            "exact_form_sha256": "a" * 64,
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "page_return_verified": True,
        },
        "frequency_and_concordance": {
            "count": 3,
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "concordance_refs": ["synthetic-concordance"],
            "claim_refs": ["tos.claim.synthetic-frequency"],
            "counting_method_ref": "synthetic-counting-method",
            "frequency_only_basis": False,
        },
        "context": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "claim_refs": ["tos.claim.synthetic-context"],
            "bounded_context_note": "Synthetic bounded context.",
        },
        "morphology": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "claim_refs": ["tos.claim.synthetic-morphology"],
            "analysis": {"part_of_speech": "synthetic"},
        },
        "lemma": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "lexeme_ref": "tos.lexeme.synthetic-form",
            "lemma_label": "synthetic-form",
            "claim_refs": ["tos.claim.synthetic-lemma"],
        },
        "recurrence_within_section": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "declared_set_ref": "synthetic-section-set",
            "claim_refs": ["tos.claim.synthetic-section-recurrence"],
            "membership_method_ref": "synthetic-membership-method",
        },
        "recurrence_within_work": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "declared_set_ref": "synthetic-work-set",
            "claim_refs": ["tos.claim.synthetic-work-recurrence"],
            "membership_method_ref": "synthetic-membership-method",
        },
        "recurrence_within_author_corpus": {
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "declared_set_ref": "synthetic-author-set",
            "claim_refs": ["tos.claim.synthetic-author-recurrence"],
            "membership_method_ref": "synthetic-membership-method",
        },
        "translation_correspondences": {
            "translation_evidence_ids": ["tos-translation-evidence.synthetic"],
            "correspondence_claim_refs": [
                "tos.claim.synthetic-translation-correspondence"
            ],
            "translation_differences_preserved": True,
            "recognized_translation_is_ground_truth": False,
        },
        "stable_sign_candidate": {
            "candidate_statement": "Synthetic stable-sign candidate, not a decision.",
            "candidate_ref": "tos.annotation.synthetic-sign-candidate",
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
            "lexeme_refs": ["tos.lexeme.synthetic-form"],
            "proposal_claim_refs": ["tos.claim.synthetic-sign-candidate"],
            "recurrence_evidence_refs": ["synthetic-section", "synthetic-work"],
            "context_variation_considered": True,
            "metaphorical_development_considered": True,
            "compositional_role_considered": True,
            "translation_divergence_considered": True,
            "negations_and_inversions_considered": True,
            "related_sign_leads_considered": True,
            "frequency_only_basis": False,
            "human_opinion_required": True,
            "proposed_status": "stable-sign-candidate-not-confirmed",
        },
    }
    stages = []
    for index, stage_name in enumerate(SEMANTIC_STAGE_ORDER):
        if index < 10:
            stages.append(
                _synthetic_semantic_stage(
                    stage_name,
                    status="proposed",
                    body=bodies.get(stage_name, {"statement": f"Synthetic {stage_name}."}),
                    maker=maker,
                )
            )
        elif index == 10:
            stages.append(
                _synthetic_semantic_stage(stage_name, status="not-started")
            )
        else:
            stages.append(
                _synthetic_semantic_stage(
                    stage_name,
                    status="blocked",
                    blocker_refs=["manual-sign-decision-missing"],
                )
            )
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/semantic-ladder-packet.schema.json",
        "schema_version": "tos_semantic_ladder_packet_v3",
        "packet_id": "tos.semantic-ladder-packet.synthetic-foundation",
        "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
        "source_expression_ref": "tos.expression.synthetic-german",
        "task_specific_source_gate": {
            "gate_kind": "task-specific-promotion-candidate-source-bundle",
            "source_anchor_refs": ["tos.anchor.synthetic-source"],
            "accepted_source_sha256": "a" * 64,
            "source_review_event_ref": "tos.event.synthetic-source-review",
            "local_content_ref": "ToS/local-content/semantic/synthetic-source.txt",
            "local_content_sha256": "a" * 64,
            "accepted_source_is_packet_local": True,
            "universal_packet_completion_required": False,
            "language_competence_status": "evidence-attested",
            "language_competence_evidence_refs": [
                "tos.review.synthetic-language-competence"
            ],
            "missing_competence_effect": "leave-unresolved-never-infer-acceptance",
            "gate_status": "satisfied",
        },
        "source_forms": {
            "diplomatic": "Synthetic exact form",
            "diplomatic_sha256": "a" * 64,
            "normalized": "Synthetic exact form",
            "normalized_sha256": "a" * 64,
        },
        "candidate_ref": "tos.annotation.synthetic-sign-candidate",
        "accepted_sign_ref": None,
        "translation_evidence": [
            {
                "evidence_id": "tos-translation-evidence.synthetic",
                "evidence_kind": "translation-packet",
                "ref": "ToS/local-content/translation/synthetic-packet.json",
                "sha256": "b" * 64,
                "review_status": "human-adjudicated",
                "rights_ref": "tos.rights.synthetic-translation",
                "recognized_translation_is_ground_truth": False,
            }
        ],
        "assurance_policy": {
            "operating_model": "solo_human_plus_ai",
            "routine_human_work_for_prepared_packet": False,
            "promotion_checkpoint_posture": (
                "opens-only-for-concrete-source-grounded-sign-candidate"
            ),
            "unassisted_operator_baseline_required": True,
            "baseline_frozen_before_model_suggestions": True,
            "second_human_review_posture": "triggered-exception-not-routine",
            "second_human_review_triggers": [
                "declared-language-competence-gap",
                "high-impact-canon-promotion",
                "persistent-source-grounded-ambiguity",
                "operator-baseline-instability",
            ],
            "model_disagreement_is_human_perspective": False,
            "rationale_preserved_separately_from_label": True,
            "alternatives_uncertainty_and_refusal_preserved": True,
            "human_work_scheduled": True,
        },
        "stage_order": SEMANTIC_STAGE_ORDER,
        "stages": stages,
        "frequency_is_not_sufficient": True,
        "every_stage_returns_to_source_page": True,
        "model_may_propose_but_not_confirm": True,
        "manual_sign_decision_required_before_interpretation": True,
        "packet_status": "awaiting-manual-sign-decision",
        "result": {
            "human_decision_refs": [],
            "relation_refs": [],
            "concept_refs": [],
            "claim_refs": [],
            "graph_projection_refs": [],
            "promotion_authorized": False,
            "conclusion": "Synthetic candidate awaits a real-human decision.",
        },
        "provenance_event_refs": ["tos.event.synthetic-semantic-packet"],
        "authority_boundary": "this packet preserves a task-specific source-returnable path from exact form to graph; packet preparation, frequency, model proposals, interpretations, and projections cannot confirm a stable sign without an attested real-human evidence-bearing promotion decision",
        "packet_version": 1,
    }


def _accept_synthetic_sign(packet: dict) -> dict:
    packet["stages"][10] = _synthetic_semantic_stage(
        "manual_confirmation_or_rejection",
        status="human-accepted",
        body={
            "decision": "accept-with-limits",
            "sign_status_assigned": "stable-sign",
            "accepted_sign_ref": "tos.sign.synthetic-foundation",
            "rationale": "Synthetic real-human decision fixture.",
            "review_receipt_ref": "tos.review.synthetic-manual-sign",
            "unassisted_baseline_ref": "tos.review.synthetic-unassisted-baseline",
            "model_suggestions_hidden_until_baseline_frozen": True,
            "considered_evidence_refs": ["synthetic-context", "synthetic-recurrence"],
            "decision_owned_by_real_human": True,
            "frequency_was_not_sole_basis": True,
            "ai_proposal_is_authority": False,
        },
        maker=_synthetic_semantic_maker("human"),
    )
    packet["stages"][10]["review_status"] = "accepted-with-limits"
    packet["accepted_sign_ref"] = "tos.sign.synthetic-foundation"
    packet["assurance_policy"]["human_work_scheduled"] = False
    packet["result"]["human_decision_refs"] = [
        "tos.review.synthetic-manual-sign"
    ]
    packet["result"]["promotion_authorized"] = True
    packet["result"]["conclusion"] = "Synthetic stable sign accepted with limits."
    packet["packet_status"] = "manual-sign-accepted"
    return packet


def _project_synthetic_semantic_graph(packet: dict) -> dict:
    packet["stages"][11] = _synthetic_semantic_stage(
        "relations_between_signs",
        status="proposed",
        body={
            "sign_refs": [
                "tos.sign.synthetic-foundation",
                "tos.sign.synthetic-related",
            ],
            "relation_records": [
                {
                    "relation_ref": "tos.relation.synthetic-sign-relation",
                    "subject_sign_ref": "tos.sign.synthetic-foundation",
                    "predicate": "synthetic_relation",
                    "object_sign_ref": "tos.sign.synthetic-related",
                    "claim_ref": "tos.claim.synthetic-sign-relation",
                    "evidence_anchor_refs": ["tos.anchor.synthetic-source"],
                    "review_status": "unreviewed",
                }
            ],
            "each_relation_resolves_to_claim_and_evidence": True,
        },
        maker=_synthetic_semantic_maker(),
    )
    packet["stages"][12] = _synthetic_semantic_stage(
        "conceptual_interpretations",
        status="proposed",
        body={
            "accepted_sign_ref": "tos.sign.synthetic-foundation",
            "concept_refs": ["tos.concept.synthetic-interpretation"],
            "claim_refs": ["tos.claim.synthetic-concept"],
            "interpretation_statement": "Synthetic conceptual interpretation.",
        },
        maker=_synthetic_semantic_maker(),
    )
    packet["stages"][13] = _synthetic_semantic_stage(
        "competing_readings",
        status="proposed",
        body={
            "primary_claim_refs": ["tos.claim.synthetic-concept"],
            "competing_claim_refs": ["tos.claim.synthetic-counterreading"],
            "bounded_rationale": "Synthetic competing reading.",
            "unresolved_is_allowed": True,
        },
        maker=_synthetic_semantic_maker(),
    )
    packet["stages"][14] = _synthetic_semantic_stage(
        "graph_projection",
        status="projected",
        body={
            "projection_ref": "ToS/derived-exports/synthetic-semantic-graph.json",
            "projection_sha256": "c" * 64,
            "sign_refs": ["tos.sign.synthetic-foundation"],
            "relation_refs": ["tos.relation.synthetic-sign-relation"],
            "claim_refs": ["tos.claim.synthetic-graph"],
            "rebuildable_from_stronger_records": True,
            "projection_is_authority": False,
        },
        maker={
            "maker_type": "software",
            "agent_ref": "software:synthetic-projector",
            "performed_by_real_human": False,
            "ai_assistance_used": False,
            "model_refs": [],
        },
    )
    packet["result"]["relation_refs"] = ["tos.relation.synthetic-sign-relation"]
    packet["result"]["concept_refs"] = ["tos.concept.synthetic-interpretation"]
    packet["result"]["claim_refs"] = [
        "tos.claim.synthetic-sign-relation",
        "tos.claim.synthetic-concept",
        "tos.claim.synthetic-counterreading",
        "tos.claim.synthetic-graph",
    ]
    packet["result"]["graph_projection_refs"] = [
        "ToS/derived-exports/synthetic-semantic-graph.json"
    ]
    packet["result"]["conclusion"] = "Synthetic graph projection completed."
    packet["packet_status"] = "graph-projected"
    return packet


def _synthetic_discovery_record() -> dict:
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/material-discovery-record.schema.json",
        "schema_version": "tos_material_discovery_record_v1",
        "discovery_id": "tos.discovery.synthetic-source",
        "protocol_ref": "ToS/source-witnesses/discovery/DISCOVERY_PROTOCOL.md",
        "target": {
            "target_kind": "edition",
            "known_tos_refs": ["tos.work.friedrich-nietzsche.also-sprach-zarathustra"],
            "description": "Synthetic exact-edition discovery target.",
            "required_properties": ["edition identity", "originating record"],
            "acceptable_substitutions": [],
            "languages": ["de"],
            "formats": ["metadata"],
            "purpose_ref": "synthetic-test",
        },
        "channels": [
            {
                "channel_id": "channel-national-catalog",
                "sequence": 1,
                "channel_type": "national-catalog",
                "role": "originating-record",
                "source_name": "Synthetic national catalog",
                "endpoint_url": "https://example.invalid/catalog",
                "interface_type": "api",
                "interface_version": "synthetic-v1",
                "exact_query": "synthetic exact query",
                "queried_at": "2026-07-23T00:00:00Z",
                "elapsed_seconds": 1.0,
                "result_order_preserved": True,
                "results": [
                    {
                        "result_id": "tos-discovery-result.synthetic-1",
                        "rank": 1,
                        "title_as_displayed": "Synthetic catalog result",
                        "result_url": "https://example.invalid/catalog/1",
                        "originating_record_url": "https://example.invalid/catalog/1",
                        "identifiers": [{"scheme": "synthetic", "value": "1"}],
                        "available_formats": ["metadata"],
                        "declared_rights": {
                            "statement": None,
                            "scope": "metadata",
                            "evidence_url": None,
                            "tos_conclusion": "evidence-only-not-a-rights-conclusion",
                        },
                        "availability": "metadata-only",
                        "machine_interface": "api",
                        "decision": "select",
                        "rationale": "Synthetic originating record fixture.",
                        "acquisition": {
                            "downloaded": False,
                            "acquired_at": None,
                            "byte_size": None,
                            "sha256": None,
                            "event_ref": None,
                        },
                        "snapshot": {
                            "state": "not-captured",
                            "format": None,
                            "sha256": None,
                            "reason": "Synthetic fixture does not capture external content.",
                        },
                    }
                ],
            }
        ],
        "channel_comparison": [
            {
                "channel_id": "channel-national-catalog",
                "completeness": "adequate",
                "metadata_precision": "strong",
                "rights_clarity": "limited",
                "machine_interface_quality": "strong",
                "human_minutes": 1,
                "machine_seconds": 1,
                "notes": "Synthetic comparison fixture.",
            }
        ],
        "selected_result_ids": ["tos-discovery-result.synthetic-1"],
        "rejected_result_ids": [],
        "rights_inference_from_availability_prohibited": True,
        "general_web_search_is_last_resort": True,
        "technical_access_bypass_used": False,
        "maker": {"maker_type": "model", "agent_ref": "model:synthetic-discovery"},
        "started_at": "2026-07-23T00:00:00Z",
        "ended_at": "2026-07-23T00:00:01Z",
        "status": "executed",
        "provenance_event_refs": ["tos.event.synthetic-discovery"],
        "record_version": 1,
    }


class SourceWitnessFoundationTests(unittest.TestCase):
    def test_ekgwb_rights_separate_private_adaptation_from_sharing(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.RIGHTS_SCHEMA,
            REPO_ROOT,
        )
        rights = json.loads(EKGWB_RIGHTS_PATH.read_text(encoding="utf-8"))

        self.assertEqual([], list(validator.iter_errors(rights)))
        self.assertEqual("conflicting_evidence", rights["assessment_status"])
        self.assertEqual("local_only", rights["visibility"])
        self.assertEqual("metadata_only", rights["redistribution_posture"])
        self.assertEqual("local_research_only", rights["derivative_posture"])
        self.assertTrue(
            any(
                "private local reproduction and adaptation" in permission
                for permission in rights["permissions"]
            )
        )
        self.assertTrue(
            any(
                "adapted material may not be shared" in restriction
                for restriction in rights["restrictions"]
            )
        )
        self.assertEqual("unreviewed", rights["review_status"])

        for ref in [*rights["source_refs"], rights["access_request_ref"]]:
            if ref.startswith("ToS/"):
                self.assertTrue((REPO_ROOT / ref).is_file(), ref)

        access_request = json.loads(
            (REPO_ROOT / rights["access_request_ref"]).read_text(encoding="utf-8")
        )
        self.assertEqual("draft-not-sent", access_request["request_status"])
        self.assertFalse(access_request["human_send_approval"])
        self.assertEqual(
            {
                "ocr_or_transcription": "not-requested",
                "indexing": "not-requested",
                "embeddings": "not-requested",
                "derivative_publication": "not-requested",
                "source_redistribution": "not-requested",
            },
            {
                key: access_request["requested_permissions"][key]
                for key in (
                    "ocr_or_transcription",
                    "indexing",
                    "embeddings",
                    "derivative_publication",
                    "source_redistribution",
                )
            },
        )

    def test_blind_pre_draft_contract_enforces_independent_lane_evidence(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.TRANSLATION_PRE_DRAFT_ANALYSIS_SCHEMA,
            REPO_ROOT,
        )

        human_packet = _synthetic_pre_draft_packet()
        self.assertEqual([], list(validator.iter_errors(human_packet)))

        contaminated_maker = copy.deepcopy(human_packet)
        contaminated_maker["maker"]["ai_assistance_used"] = True
        contaminated_maker["maker"]["model_refs"] = ["synthetic-model"]
        self.assertTrue(list(validator.iter_errors(contaminated_maker)))

        contaminated_finding = copy.deepcopy(human_packet)
        contaminated_finding["stages"][0]["findings"][0]["maker"]["maker_type"] = "model"
        self.assertTrue(list(validator.iter_errors(contaminated_finding)))

        comparator_visible = copy.deepcopy(human_packet)
        comparator_visible["blindness"]["recognized_comparator_visible"] = True
        self.assertTrue(list(validator.iter_errors(comparator_visible)))

        unsupported_etymology = copy.deepcopy(human_packet)
        unsupported_etymology["stages"][4]["findings"][0]["reference_entry_ids"] = []
        unsupported_etymology["stages"][4]["findings"][0]["citations"] = []
        self.assertTrue(list(validator.iter_errors(unsupported_etymology)))

        incompletely_frozen = copy.deepcopy(human_packet)
        incompletely_frozen["stages"][7]["status"] = "proposed"
        self.assertTrue(list(validator.iter_errors(incompletely_frozen)))

        ai_packet = _synthetic_pre_draft_packet(lane="ai_only")
        self.assertEqual([], list(validator.iter_errors(ai_packet)))

        human_edited_ai = copy.deepcopy(ai_packet)
        human_edited_ai["maker"]["human_editing_used"] = True
        self.assertTrue(list(validator.iter_errors(human_edited_ai)))

    def test_translation_packet_enforces_blind_lifecycle_and_lane_inputs(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.TRANSLATION_PACKET_SCHEMA,
            REPO_ROOT,
        )

        preparing = _synthetic_translation_packet()
        self.assertEqual([], list(validator.iter_errors(preparing)))

        frozen = _freeze_synthetic_translation_packet(copy.deepcopy(preparing))
        self.assertEqual([], list(validator.iter_errors(frozen)))

        wrong_analysis_lane = copy.deepcopy(frozen)
        wrong_analysis_lane["drafts"][0]["pre_draft_inputs"][0]["lane"] = "ai_only"
        self.assertTrue(list(validator.iter_errors(wrong_analysis_lane)))

        contaminated_human = copy.deepcopy(frozen)
        contaminated_human["drafts"][0]["maker"]["ai_assistance_used"] = True
        contaminated_human["drafts"][0]["maker"]["model_refs"] = ["synthetic-model"]
        self.assertTrue(list(validator.iter_errors(contaminated_human)))

        human_edited_ai = copy.deepcopy(frozen)
        human_edited_ai["drafts"][1]["maker"]["human_editing_used"] = True
        self.assertTrue(list(validator.iter_errors(human_edited_ai)))

        one_machine_alternative = copy.deepcopy(frozen)
        one_machine_alternative["drafts"] = [
            draft
            for draft in one_machine_alternative["drafts"]
            if draft["draft_id"] != "tos-translation-draft.synthetic-alternative-2"
        ]
        self.assertTrue(list(validator.iter_errors(one_machine_alternative)))

        mixed_without_independent_ai = copy.deepcopy(frozen)
        mixed_without_independent_ai["drafts"][-1]["input_drafts"] = [
            mixed_without_independent_ai["drafts"][-1]["input_drafts"][0]
        ]
        self.assertTrue(list(validator.iter_errors(mixed_without_independent_ai)))

        revealed = _reveal_synthetic_comparator(copy.deepcopy(frozen))
        self.assertEqual([], list(validator.iter_errors(revealed)))

        premature_reveal = copy.deepcopy(revealed)
        premature_reveal["packet_status"] = "blind-drafts-frozen"
        self.assertTrue(list(validator.iter_errors(premature_reveal)))

        comparator_as_ground_truth = copy.deepcopy(revealed)
        comparator_as_ground_truth["recognized_comparator"][
            "recognized_translation_is_ground_truth"
        ] = True
        self.assertTrue(list(validator.iter_errors(comparator_as_ground_truth)))

        comparison = _freeze_synthetic_comparison(copy.deepcopy(revealed))
        self.assertEqual([], list(validator.iter_errors(comparison)))

        incomplete_change_ledger = copy.deepcopy(comparison)
        incomplete_change_ledger["post_reveal_changes"].pop()
        self.assertTrue(list(validator.iter_errors(incomplete_change_ledger)))

    def test_semantic_ladder_requires_real_human_sign_gate_before_graph(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.SEMANTIC_LADDER_PACKET_SCHEMA,
            REPO_ROOT,
        )

        awaiting_human = _synthetic_semantic_packet()
        self.assertEqual([], list(validator.iter_errors(awaiting_human)))
        self.assertEqual(
            [],
            foundation._semantic_ladder_identity_issues(awaiting_human),
        )

        premature_relations = copy.deepcopy(awaiting_human)
        premature_relations["stages"][11] = _synthetic_semantic_stage(
            "relations_between_signs",
            status="proposed",
            body={"claim_refs": ["tos.claim.synthetic-premature-relation"]},
            maker=_synthetic_semantic_maker(),
        )
        self.assertTrue(list(validator.iter_errors(premature_relations)))

        frequency_promoted = copy.deepcopy(awaiting_human)
        frequency_promoted["stages"][9]["body"]["frequency_only_basis"] = True
        self.assertTrue(list(validator.iter_errors(frequency_promoted)))

        accepted = _accept_synthetic_sign(copy.deepcopy(awaiting_human))
        self.assertEqual([], list(validator.iter_errors(accepted)))
        self.assertEqual(
            [],
            foundation._semantic_ladder_identity_issues(accepted),
        )

        mismatched_sign_identity = copy.deepcopy(accepted)
        mismatched_sign_identity["accepted_sign_ref"] = (
            "tos.sign.synthetic-different"
        )
        self.assertIn(
            "human sign decision identity differs from packet accepted_sign_ref",
            foundation._semantic_ladder_identity_issues(
                mismatched_sign_identity
            ),
        )

        simulated_human = copy.deepcopy(accepted)
        simulated_human["stages"][10]["maker"]["performed_by_real_human"] = False
        self.assertTrue(list(validator.iter_errors(simulated_human)))

        contradictory_decision = copy.deepcopy(accepted)
        contradictory_decision["stages"][10]["body"]["decision"] = "reject"
        self.assertTrue(list(validator.iter_errors(contradictory_decision)))

        validator_only_decision = copy.deepcopy(accepted)
        validator_only_decision["stages"][10]["body"][
            "frequency_was_not_sole_basis"
        ] = False
        self.assertTrue(list(validator.iter_errors(validator_only_decision)))

        graph_packet = _project_synthetic_semantic_graph(copy.deepcopy(accepted))
        self.assertEqual([], list(validator.iter_errors(graph_packet)))
        self.assertEqual(
            [],
            foundation._semantic_ladder_identity_issues(graph_packet),
        )

        missing_relation_result = copy.deepcopy(graph_packet)
        missing_relation_result["result"]["relation_refs"] = []
        self.assertIn(
            "relation identities are absent from packet result",
            foundation._semantic_ladder_identity_issues(
                missing_relation_result
            ),
        )

        skipped_counterreading = copy.deepcopy(graph_packet)
        skipped_counterreading["stages"][13] = _synthetic_semantic_stage(
            "competing_readings",
            status="blocked",
            blocker_refs=["counterreading-not-completed"],
        )
        self.assertTrue(list(validator.iter_errors(skipped_counterreading)))

        authoritative_projection = copy.deepcopy(graph_packet)
        authoritative_projection["stages"][14]["body"]["projection_is_authority"] = True
        self.assertTrue(list(validator.iter_errors(authoritative_projection)))

    def test_initial_sign_packet_is_tracked_without_invented_content_or_human_debt(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.SEMANTIC_LADDER_PACKET_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            (GOLD_ROOT / "initial-sign-packet.v3.json").read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual("blocked-not-materialized", packet["packet_status"])
        self.assertEqual("blocked", packet["task_specific_source_gate"]["gate_status"])
        self.assertEqual([], packet["task_specific_source_gate"]["source_anchor_refs"])
        self.assertIsNone(packet["source_forms"])
        self.assertIsNone(packet["candidate_ref"])
        self.assertIsNone(packet["accepted_sign_ref"])
        self.assertFalse(packet["assurance_policy"]["human_work_scheduled"])
        self.assertTrue(all(stage["status"] == "blocked" for stage in packet["stages"]))
        self.assertTrue(all(stage["body"] == {} for stage in packet["stages"]))
        self.assertFalse(packet["result"]["promotion_authorized"])

        fabricated_form = copy.deepcopy(packet)
        fabricated_form["source_forms"] = {
            "diplomatic": "fabricated",
            "diplomatic_sha256": "a" * 64,
            "normalized": "fabricated",
            "normalized_sha256": "a" * 64,
        }
        self.assertTrue(list(validator.iter_errors(fabricated_form)))

        fabricated_human_debt = copy.deepcopy(packet)
        fabricated_human_debt["assurance_policy"]["human_work_scheduled"] = True
        self.assertTrue(list(validator.iter_errors(fabricated_human_debt)))

    def test_discovery_access_and_server_boundaries_fail_closed(self) -> None:
        discovery_validator, _ = foundation._schema_validator(
            foundation.MATERIAL_DISCOVERY_SCHEMA,
            REPO_ROOT,
        )
        discovery = _synthetic_discovery_record()
        self.assertEqual([], list(discovery_validator.iter_errors(discovery)))
        self.assertEqual([], foundation._discovery_decision_issues(discovery))

        bypassed = copy.deepcopy(discovery)
        bypassed["technical_access_bypass_used"] = True
        self.assertTrue(list(discovery_validator.iter_errors(bypassed)))

        false_download = copy.deepcopy(discovery)
        false_download["channels"][0]["results"][0]["acquisition"]["downloaded"] = True
        self.assertTrue(list(discovery_validator.iter_errors(false_download)))

        missing_selection = copy.deepcopy(discovery)
        missing_selection["selected_result_ids"] = []
        self.assertIn(
            "selected_result_ids do not match results whose decision is select",
            foundation._discovery_decision_issues(missing_selection),
        )

        duplicate_result = copy.deepcopy(discovery)
        duplicate_result["channels"][0]["results"].append(
            copy.deepcopy(duplicate_result["channels"][0]["results"][0])
        )
        self.assertIn(
            "duplicate discovery result_id: tos-discovery-result.synthetic-1",
            foundation._discovery_decision_issues(duplicate_result),
        )

        access_validator, _ = foundation._schema_validator(
            foundation.ACCESS_REQUEST_SCHEMA,
            REPO_ROOT,
        )
        access_path = (
            REPO_ROOT
            / "ToS/source-witnesses/access-requests/public-ledger/"
            "nietzsche-woerterbuch.access-request.json"
        )
        access_record = json.loads(access_path.read_text(encoding="utf-8"))
        self.assertEqual([], list(access_validator.iter_errors(access_record)))
        self.assertEqual("draft-not-sent", access_record["request_status"])
        self.assertFalse(access_record["human_send_approval"])

        falsely_sent = copy.deepcopy(access_record)
        falsely_sent["request_status"] = "sent"
        self.assertTrue(list(access_validator.iter_errors(falsely_sent)))

        falsely_granted = copy.deepcopy(access_record)
        falsely_granted["request_status"] = "permission-granted"
        falsely_granted["human_send_approval"] = True
        falsely_granted["sent_at"] = "2026-07-23T01:00:00Z"
        falsely_granted["private_correspondence_ref"] = (
            "ToS/source-witnesses/access-requests/private/synthetic/request.eml"
        )
        self.assertTrue(list(access_validator.iter_errors(falsely_granted)))

        server_validator, _ = foundation._schema_validator(
            foundation.SERVER_IMPORT_SCHEMA,
            REPO_ROOT,
        )
        plan_paths = sorted(
            (REPO_ROOT / "ToS/source-witnesses/server-import/plans").glob("*.json")
        )
        manifest_paths = sorted(
            (REPO_ROOT / "ToS/source-witnesses").rglob("item.manifest.json")
        )
        self.assertEqual(len(manifest_paths), len(plan_paths))
        for plan_path in plan_paths:
            plan = json.loads(plan_path.read_text(encoding="utf-8"))
            self.assertEqual([], list(server_validator.iter_errors(plan)))
            self.assertEqual("blocked-rights", plan["server_import_status"])
            self.assertEqual("metadata-only", plan["access_class"])
            self.assertFalse(plan["payload_transfer_authorized"])
            self.assertFalse(plan["operator_transfer_approval"]["approved"])

        unauthorized_transfer = json.loads(plan_paths[0].read_text(encoding="utf-8"))
        unauthorized_transfer["payload_transfer_authorized"] = True
        self.assertTrue(list(server_validator.iter_errors(unauthorized_transfer)))

        unreviewed_import = json.loads(plan_paths[0].read_text(encoding="utf-8"))
        unreviewed_import["access_class"] = "controlled-research"
        unreviewed_import["payload_transfer_authorized"] = True
        unreviewed_import["operator_transfer_approval"] = {
            "approved": True,
            "approved_by_real_human": True,
            "approved_at": "2026-07-23T01:00:00Z",
            "approval_ref": "synthetic-human-approval",
        }
        unreviewed_import["server_import_status"] = "imported"
        unreviewed_import["server_receipt_refs"] = ["synthetic-server-receipt"]
        self.assertTrue(list(server_validator.iter_errors(unreviewed_import)))

        private_route = REPO_ROOT / "ToS/source-witnesses/access-requests/private/README.md"
        private_example = REPO_ROOT / "ToS/source-witnesses/access-requests/private/request/message.eml"
        self.assertFalse(foundation._git_ignored(REPO_ROOT, private_route))
        self.assertTrue(foundation._git_ignored(REPO_ROOT, private_example))

    def test_jenseits_1886_witness_opens_transfer_soil_without_acceptance(self) -> None:
        manifest = json.loads(
            (JENSEITS_1886_ITEM_ROOT / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        rights = json.loads(
            (JENSEITS_1886_ITEM_ROOT / "rights.json").read_text(encoding="utf-8")
        )
        inventory = json.loads(
            (JENSEITS_1886_ITEM_ROOT / "resource-inventory.json").read_text(
                encoding="utf-8"
            )
        )
        discovery = json.loads(
            JENSEITS_1886_DISCOVERY_PATH.read_text(encoding="utf-8")
        )
        server_plan = json.loads(
            (
                REPO_ROOT
                / "ToS/source-witnesses/server-import/plans/"
                "jenseits-naumann-1886-internet-archive-google-harvard-"
                "scan-pdf.server-import.json"
            ).read_text(encoding="utf-8")
        )

        self.assertEqual("digitized_physical_copy", manifest["item_kind"])
        self.assertEqual("local_only", manifest["visibility"])
        self.assertEqual(
            {
                "6ae316c90f958d09045fea27b2430b86623ebb85f8a27146099d028775cdc80a",
                "6227d4a797fb27608386733a9d71fd06c049e5458c9e0687cb582f0c31177be0",
                "ba8f4c91a317a3de03ab1f318860aaba6837d979e1ec99365e6d13def7db5a34",
            },
            {
                payload_file["sha256"]
                for payload_file in manifest["payload_files"]
            },
        )
        self.assertTrue(
            all(
                foundation._git_ignored(
                    REPO_ROOT,
                    JENSEITS_1886_ITEM_ROOT
                    / payload_file["relative_path"],
                )
                for payload_file in manifest["payload_files"]
            )
        )

        self.assertEqual(
            "http://creativecommons.org/publicdomain/mark/1.0/",
            rights["rights_statement_uri"],
        )
        self.assertEqual("copyright_undetermined", rights["assessment_status"])
        self.assertEqual("unreviewed", rights["review_status"])
        self.assertEqual("unknown", rights["redistribution_posture"])
        self.assertIn(
            "the operator-held payload remains local and is not a future-site upload",
            rights["restrictions"],
        )

        self.assertFalse(inventory["source_text_included"])
        files_by_profile = {
            file_inventory["profile"]: file_inventory
            for file_inventory in inventory["files"]
        }
        self.assertEqual(
            {
                "pdf_pages_v1",
                "djvu_xml_pages_v1",
                "abbyy_xml_pages_v1",
            },
            set(files_by_profile),
        )
        self.assertTrue(
            all(
                file_inventory["summary"]["page_count"] == 274
                for file_inventory in files_by_profile.values()
            )
        )
        self.assertEqual(
            820,
            files_by_profile["pdf_pages_v1"]["summary"][
                "image_resource_count"
            ],
        )
        self.assertEqual(
            62700,
            files_by_profile["djvu_xml_pages_v1"]["summary"]["word_count"],
        )
        self.assertEqual(
            61704,
            files_by_profile["abbyy_xml_pages_v1"]["summary"]["word_count"],
        )

        self.assertEqual(
            [
                "channel-dnb-jenseits-work-authority",
                "channel-dta-nietzsche-author-corpus",
                "channel-textgrid-jenseits-editions",
                "channel-internet-archive-jenseits",
                "channel-google-books-jenseits-id",
            ],
            [channel["channel_id"] for channel in discovery["channels"]],
        )
        self.assertEqual(
            ["tos-discovery-result.ia-bub-gb-yiuraaaayaaaj"],
            discovery["selected_result_ids"],
        )
        self.assertEqual([], discovery["channels"][-1]["results"])
        self.assertFalse(discovery["technical_access_bypass_used"])

        self.assertEqual("metadata-only", server_plan["access_class"])
        self.assertEqual("blocked-rights", server_plan["server_import_status"])
        self.assertFalse(server_plan["payload_transfer_authorized"])
        self.assertEqual(3, len(server_plan["payload_files"]))
        self.assertEqual(2, server_plan["contract_version"])
        self.assertEqual(
            "prohibited",
            server_plan["allowed_derivatives"]["transcription"]["state"],
        )

    def test_golden_kernel_transfer_plan_fails_closed_without_human_kernel(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLDEN_KERNEL_TRANSFER_PLAN_SCHEMA,
            REPO_ROOT,
        )
        transfer_path = GOLD_ROOT / "transfer-samples.json"
        transfer_plan = json.loads(transfer_path.read_text(encoding="utf-8"))

        self.assertEqual([], list(validator.iter_errors(transfer_plan)))
        self.assertEqual(
            "tos_golden_kernel_transfer_plan_v2",
            transfer_plan["schema_version"],
        )
        self.assertEqual("blocked-not-run", transfer_plan["status"])
        self.assertEqual("blocked", transfer_plan["kernel_evidence_gate"]["gate_status"])
        self.assertFalse(
            transfer_plan["kernel_evidence_gate"]["semantic_transfer_claim_authorized"]
        )
        self.assertEqual(0, transfer_plan["result"]["run_count"])
        self.assertIsNone(transfer_plan["result"]["winner"])
        self.assertEqual([], transfer_plan["result"]["metric_results"])
        self.assertEqual(3, len(transfer_plan["scouting_units"]))
        self.assertEqual([], transfer_plan["target_units"])
        candidates = transfer_plan["candidate_target_units"]
        self.assertEqual(20, len(candidates))
        self.assertEqual(
            {"random": 10, "hard": 10},
            {
                stratum: sum(
                    candidate["stratum"] == stratum
                    for candidate in candidates
                )
                for stratum in ("random", "hard")
            },
        )
        self.assertEqual(
            20,
            len(
                {
                    (candidate["file_ref"], candidate["page"])
                    for candidate in candidates
                }
            ),
        )
        self.assertTrue(
            all(
                candidate["source_review_status"] == "model_source_visible"
                and candidate["target_gold_status"] == "not_started"
                and candidate["frozen_before_variant_outputs"] is True
                and candidate["eligible_for_variant_execution"] is False
                for candidate in candidates
            )
        )
        self.assertFalse(
            transfer_plan["candidate_preparation"]["human_review_performed"]
        )
        self.assertFalse(
            transfer_plan["candidate_preparation"]["variant_outputs_visible"]
        )
        self.assertTrue(
            all(
                unit["unit_kind"] == "title-page"
                and unit["selection_status"] == "scouting-only"
                and unit["eligible_for_semantic_transfer"] is False
                for unit in transfer_plan["scouting_units"]
            )
        )

        false_run = copy.deepcopy(transfer_plan)
        false_run["result"]["run_count"] = 1
        false_run["result"]["run_refs"] = ["synthetic-run"]
        self.assertTrue(list(validator.iter_errors(false_run)))

        false_authority = copy.deepcopy(transfer_plan)
        false_authority["kernel_evidence_gate"][
            "semantic_transfer_claim_authorized"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_authority)))

        premature_ready = copy.deepcopy(transfer_plan)
        premature_ready["status"] = "ready"
        self.assertTrue(list(validator.iter_errors(premature_ready)))

        counters_only_ready = copy.deepcopy(transfer_plan)
        counters_only_ready["status"] = "ready"
        counters_only_ready["kernel_evidence_gate"].update(
            {
                "accepted_source_units": 30,
                "human_double_checked_gold_units": 15,
                "human_accepted_sign_packets": 1,
                "human_accepted_translation_packets": 1,
                "human_double_checked_target_units": 20,
                "gate_status": "satisfied",
                "semantic_transfer_claim_authorized": True,
                "blockers": [],
            }
        )
        self.assertTrue(list(validator.iter_errors(counters_only_ready)))

        semantic_title_page = copy.deepcopy(transfer_plan)
        semantic_title_page["scouting_units"][0][
            "eligible_for_semantic_transfer"
        ] = True
        self.assertTrue(list(validator.iter_errors(semantic_title_page)))

        false_candidate_authority = copy.deepcopy(transfer_plan)
        false_candidate_authority["candidate_target_units"][0][
            "eligible_for_variant_execution"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_candidate_authority)))

        incomplete_metrics = copy.deepcopy(transfer_plan)
        incomplete_metrics["metrics"].pop()
        self.assertTrue(list(validator.iter_errors(incomplete_metrics)))

        events = [
            json.loads(line)
            for line in (GOLD_ROOT / "transfer-provenance.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        event = next(
            candidate
            for candidate in events
            if candidate["event_id"] == transfer_plan["provenance_event_ref"]
        )
        output = next(
            output
            for output in event["outputs"]
            if output["ref"].endswith("/transfer-samples.json")
        )
        self.assertEqual(
            hashlib.sha256(transfer_path.read_bytes()).hexdigest(),
            output["sha256"],
        )

    def test_human_gold_requires_materialized_content_and_review_receipts(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLD_STATUS_SCHEMA,
            REPO_ROOT,
        )
        status = json.loads((GOLD_ROOT / "gold-status.json").read_text(encoding="utf-8"))
        unit = status["units"][0]
        unit["content_sha256"] = "a" * 64
        unit["gold_status"] = "human_double_checked"
        for field, maker in (
            ("human_pass_1", "human:reviewer-one"),
            ("human_pass_2", "human:reviewer-two"),
        ):
            unit[field] = {
                "status": "complete",
                "maker_ref": maker,
                "completed_at": "2026-07-24T02:00:00Z",
                "receipt_ref": f"local-review/{field}.json",
            }
        self.assertEqual([], list(validator.iter_errors(status)))

        for field_path in (
            ("content_sha256",),
            ("human_pass_1", "maker_ref"),
            ("human_pass_1", "completed_at"),
            ("human_pass_1", "receipt_ref"),
            ("human_pass_2", "maker_ref"),
            ("human_pass_2", "completed_at"),
            ("human_pass_2", "receipt_ref"),
        ):
            invalid = copy.deepcopy(status)
            target = invalid["units"][0]
            if len(field_path) == 1:
                target[field_path[0]] = None
            else:
                target[field_path[0]][field_path[1]] = None
            self.assertTrue(
                list(validator.iter_errors(invalid)),
                f"{'.'.join(field_path)} remained optional for human gold",
            )

    def test_manual_gold_assurance_preserves_solo_and_language_boundaries(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLD_ASSURANCE_SCHEMA,
            REPO_ROOT,
        )
        assurance_path = GOLD_ROOT / "gold-assurance.v2.json"
        assurance = json.loads(assurance_path.read_text(encoding="utf-8"))
        status = json.loads((GOLD_ROOT / "gold-status.json").read_text(encoding="utf-8"))

        self.assertEqual([], list(validator.iter_errors(assurance)))
        self.assertEqual(
            {unit["sample_id"] for unit in status["units"]},
            {unit["sample_id"] for unit in assurance["units"]},
        )
        self.assertEqual(
            {
                unit["sample_id"]: unit["anchor_ref"]
                for unit in status["units"]
            },
            {
                unit["sample_id"]: unit["anchor_ref"]
                for unit in assurance["units"]
            },
        )

        russian_units = [unit for unit in assurance["units"] if unit["language"] == "ru"]
        german_units = [unit for unit in assurance["units"] if unit["language"] == "de"]
        self.assertEqual(10, len(russian_units))
        self.assertEqual(5, len(german_units))
        self.assertTrue(
            all(
                unit["current_assurance"] == "unreviewed"
                and unit["next_route"] == "none"
                for unit in russian_units
            )
        )
        self.assertTrue(
            all(
                unit["competence_level"] == "visual_only"
                and unit["current_assurance"] == "language_competence_blocked"
                and unit["next_route"] == "resolve_language_competence"
                for unit in german_units
            )
        )
        schedule = assurance["human_work_schedule"]
        self.assertFalse(schedule["packet_units_are_human_debt"])
        self.assertEqual("closed_no_human_debt", schedule["current_status"])
        self.assertEqual(0, schedule["human_debt_units"])
        self.assertEqual(
            ["tos-sample-antonovsky-p011"],
            schedule["selected_calibration_unit_ids"],
        )
        self.assertEqual(
            {unit["sample_id"] for unit in assurance["units"]},
            set(schedule["selected_calibration_unit_ids"])
            | set(schedule["unscheduled_unit_ids"]),
        )
        self.assertFalse(schedule["promotion_authorized"])
        self.assertEqual("not_collected", schedule["attestation_status"])
        for field in ("runtime_closure", "runtime_receipt", "source_autosave"):
            local_ref = schedule[field]
            self.assertEqual("abyss-stack", local_ref["owner"])
            self.assertEqual(
                "abyss_machine_artifact_store",
                local_ref["artifact_root"],
            )
            self.assertFalse(Path(local_ref["relative_path"]).is_absolute())
            self.assertNotIn("..", Path(local_ref["relative_path"]).parts)

    def test_private_evidence_handoff_freezes_destination_before_raw_read(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.PRIVATE_EVIDENCE_HANDOFF_SCHEMA,
            REPO_ROOT,
        )
        handoff = json.loads(PRIVATE_HANDOFF_PATH.read_text(encoding="utf-8"))
        self.assertEqual([], list(validator.iter_errors(handoff)))
        self.assertEqual(
            [],
            foundation._private_evidence_handoff_issues(
                handoff,
                repo_root=REPO_ROOT,
            ),
        )
        self.assertFalse(handoff["effects"]["raw_read"])
        self.assertFalse(handoff["effects"]["derivative_created"])
        self.assertFalse(handoff["effects"]["publication"])
        self.assertFalse(handoff["destination"]["publication_authority"])

        unauthorized_publication = copy.deepcopy(handoff)
        unauthorized_publication["effects"]["publication"] = True
        self.assertTrue(list(validator.iter_errors(unauthorized_publication)))

        missing_prohibition = copy.deepcopy(handoff)
        missing_prohibition["disclosure_policy"]["forbidden_classes"].remove(
            "source_text_or_transcription"
        )
        self.assertTrue(
            foundation._private_evidence_handoff_issues(
                missing_prohibition,
                repo_root=REPO_ROOT,
            )
        )

    def test_german_assisted_review_adds_no_false_competence_or_human_debt(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GERMAN_ASSISTED_SOURCE_REVIEW_SCHEMA,
            REPO_ROOT,
        )
        plan = json.loads(
            GERMAN_ASSISTED_REVIEW_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(plan)))
        self.assertEqual("visual_only", plan["competence_boundary"]["operator_german_competence"])
        self.assertFalse(
            plan["competence_boundary"][
                "machine_agreement_supplies_language_competence"
            ]
        )
        self.assertEqual(30, plan["current_state"]["prepared_units"])
        self.assertEqual(
            ["tos-translation-source-review-v2-001"],
            plan["current_state"]["selected_unit_ids"],
        )
        self.assertEqual(0, plan["current_state"]["human_debt_units"])
        self.assertEqual(1, plan["current_state"]["runs"])
        self.assertEqual(
            1,
            plan["current_state"]["machine_triangulated_units"],
        )
        self.assertEqual(0, plan["current_state"]["accepted_german_units"])
        self.assertEqual(
            1,
            plan["current_state"][
                "prepared_critical_edition_witness_packets"
            ],
        )
        self.assertEqual([], plan["current_state"]["translation_lanes_opened"])
        self.assertFalse(plan["current_state"]["promotion_authorized"])

        false_acceptance = copy.deepcopy(plan)
        false_acceptance["current_state"]["accepted_german_units"] = 1
        self.assertTrue(list(validator.iter_errors(false_acceptance)))

        false_competence = copy.deepcopy(plan)
        false_competence["competence_boundary"][
            "machine_agreement_supplies_language_competence"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_competence)))

    def test_german_source_triangulation_is_machine_only_and_text_free(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GERMAN_SOURCE_TRIANGULATION_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            GERMAN_SOURCE_TRIANGULATION_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual("machine_triangulated_candidate", packet["status"])
        self.assertEqual(
            12,
            packet["results"]["dta_exact_comparison"]["equal_paragraphs"],
        )
        self.assertEqual(
            261,
            packet["results"]["dta_exact_comparison"]["equal_tokens"],
        )
        self.assertEqual(
            260,
            packet["results"]["naumann_ocr_comparison"][
                "equal_reference_tokens"
            ],
        )
        self.assertEqual(
            3,
            packet["results"]["normalization_failure_control"][
                "naive_generic_whitespace_join_false_token_splits"
            ],
        )
        self.assertFalse(
            packet["method"]["model_used_for_text_decision"]
        )
        self.assertFalse(packet["method"]["source_text_emitted"])
        self.assertFalse(
            packet["inputs"]["ekgwb"]["source_text_tracked"]
        )
        self.assertTrue(
            packet["rights_and_transport_boundary"][
                "unencrypted_transport_is_authenticity_risk"
            ]
        )
        self.assertEqual([], packet["gate_effects"]["translation_lanes_opened"])
        self.assertEqual(0, packet["gate_effects"]["accepted_german_units"])
        self.assertFalse(packet["gate_effects"]["promotion_authorized"])

        false_acceptance = copy.deepcopy(packet)
        false_acceptance["gate_effects"]["accepted_german_units"] = 1
        self.assertTrue(list(validator.iter_errors(false_acceptance)))

        false_rights_clearance = copy.deepcopy(packet)
        false_rights_clearance["rights_and_transport_boundary"][
            "rights_clearance_claimed"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_rights_clearance)))

        tracked_source_text = copy.deepcopy(packet)
        tracked_source_text["inputs"]["ekgwb"]["source_text_tracked"] = True
        self.assertTrue(list(validator.iter_errors(tracked_source_text)))

        raw_tokens = triangulation_builder._alpha_tokens("prüf¬ wort")
        source_aware_tokens = triangulation_builder._alpha_tokens(
            triangulation_builder.re.sub(r"¬\s*", "", "prüf¬ wort")
        )
        self.assertEqual(["prüf", "wort"], raw_tokens)
        self.assertEqual(["prüfwort"], source_aware_tokens)

    def test_bounded_translation_input_opens_only_local_calibration(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.BOUNDED_TRANSLATION_RESEARCH_INPUT_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            BOUNDED_TRANSLATION_RESEARCH_INPUT_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual(
            "eligible_for_local_machine_calibration",
            packet["status"],
        )
        self.assertEqual(
            "translation_method_calibration",
            packet["experiment_scope"]["purpose"],
        )
        self.assertFalse(
            packet["source_derivation"]["model_used_for_boundary_or_text"]
        )
        self.assertFalse(
            packet["source_derivation"][
                "source_text_emitted_to_tracked_packet"
            ]
        )
        self.assertTrue(packet["local_artifact"]["gitignored"])
        self.assertFalse(
            packet["local_artifact"][
                "source_text_copied_into_tracked_admission"
            ]
        )
        self.assertFalse(
            packet["experiment_scope"][
                "preexisting_authored_translation_surfaces_visible"
            ]
        )
        self.assertEqual(
            20,
            packet["local_artifact"]["normalized_alpha_tokens"],
        )
        self.assertEqual(
            0,
            packet["gate_effects"]["accepted_german_units"],
        )
        self.assertEqual(
            [],
            packet["gate_effects"]["accepted_translation_lanes_opened"],
        )
        self.assertEqual(
            0,
            packet["gate_effects"]["semantic_tasks_opened"],
        )
        self.assertFalse(
            packet["gate_effects"][
                "canon_or_graph_promotion_authorized"
            ]
        )

        false_acceptance = copy.deepcopy(packet)
        false_acceptance["gate_effects"]["accepted_german_units"] = 1
        self.assertTrue(list(validator.iter_errors(false_acceptance)))

        false_publication = copy.deepcopy(packet)
        false_publication["rights_and_visibility"][
            "public_site_upload_authorized"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_publication)))

        tracked_text = copy.deepcopy(packet)
        tracked_text["local_artifact"][
            "source_text_copied_into_tracked_admission"
        ] = True
        self.assertTrue(list(validator.iter_errors(tracked_text)))

        contaminated_blind_run = copy.deepcopy(packet)
        contaminated_blind_run["experiment_scope"][
            "preexisting_authored_translation_surfaces_visible"
        ] = True
        self.assertTrue(list(validator.iter_errors(contaminated_blind_run)))

    def test_bounded_translation_input_builder_uses_synthetic_source_only(
        self,
    ) -> None:
        sentence = bounded_input_builder._first_sentence(
            "Erste synthetische Aussage. Zweite Aussage."
        )
        self.assertEqual("Erste synthetische Aussage.", sentence)
        self.assertEqual(
            "Mehrere Leerzeichen bleiben lesbar.",
            bounded_input_builder._first_sentence(
                "Mehrere   Leerzeichen\nbleiben lesbar. Danach."
            ),
        )
        with self.assertRaises(
            bounded_input_builder.BoundedInputBuildError
        ):
            bounded_input_builder._first_sentence(
                "Synthetischer Text ohne Abschluss"
            )

    def test_critical_edition_witness_freezes_metadata_without_admission(
        self,
    ) -> None:
        validator, _ = foundation._schema_validator(
            foundation.CRITICAL_EDITION_WITNESS_ADMISSION_SCHEMA,
            REPO_ROOT,
        )
        packet = json.loads(
            CRITICAL_EDITION_WITNESS_PATH.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(packet)))
        self.assertEqual(
            "https://www.nietzschesource.org/eKGWB/Za-I-Vorrede-1",
            packet["target"]["critical_locator_url"],
        )
        self.assertEqual(
            "local_structure_compatible_exact_critical_passage_unverified",
            packet["target"]["alignment_state"],
        )
        self.assertEqual(
            "local_section_boundary_observed_critical_text_not_compared",
            packet["local_structural_context"]["state"],
        )
        self.assertFalse(
            packet["local_structural_context"]["comparison"][
                "exact_critical_text_compared"
            ]
        )
        self.assertFalse(
            packet["local_structural_context"]["comparison"][
                "exact_passage_alignment_claimed"
            ]
        )
        self.assertFalse(packet["content_boundary"]["source_text_stored"])
        self.assertFalse(
            packet["rights_gate"]["citation_witness_admission_authorized"]
        )
        self.assertFalse(
            packet["gate_effects"]["critical_edition_unit_admitted"]
        )
        self.assertEqual([], packet["gate_effects"]["translation_lanes_opened"])
        self.assertFalse(packet["gate_effects"]["human_task_created"])

        false_admission = copy.deepcopy(packet)
        false_admission["gate_effects"][
            "critical_edition_unit_admitted"
        ] = True
        self.assertTrue(list(validator.iter_errors(false_admission)))

        captured_text = copy.deepcopy(packet)
        captured_text["content_boundary"]["source_text_stored"] = True
        self.assertTrue(list(validator.iter_errors(captured_text)))

        source_review_plan = json.loads(
            (GOLD_ROOT / "translation-source-review-plan.v2.json").read_text(
                encoding="utf-8"
            )
        )
        visual_sample_plan = json.loads(
            (GOLD_ROOT / "ocr-visual-samples.json").read_text(encoding="utf-8")
        )
        target_unit = source_review_plan["units"][0]
        self.assertEqual(
            [],
            foundation._critical_edition_local_structural_context_issues(
                packet,
                target_unit=target_unit,
                source_review_plan=source_review_plan,
                ocr_sample_plan=visual_sample_plan,
            ),
        )

        drifted_member = copy.deepcopy(packet)
        drifted_member["local_structural_context"]["epub_witness"][
            "section_start_member"
        ]["path"] = "EPUB/page_56.html"
        self.assertIn(
            "critical-edition local section start drifted from the target source unit",
            foundation._critical_edition_local_structural_context_issues(
                drifted_member,
                target_unit=target_unit,
                source_review_plan=source_review_plan,
                ocr_sample_plan=visual_sample_plan,
            ),
        )

        drifted_page = copy.deepcopy(packet)
        drifted_page["local_structural_context"]["visual_witness"][
            "section_start_pdf_page"
        ] = 57
        alignment_issues = (
            foundation._critical_edition_local_structural_context_issues(
                drifted_page,
                target_unit=target_unit,
                source_review_plan=source_review_plan,
                ocr_sample_plan=visual_sample_plan,
            )
        )
        self.assertIn(
            "critical-edition local section start page drifted from the target source unit",
            alignment_issues,
        )
        self.assertIn(
            "critical-edition local section start anchor is absent or ambiguous in the visual sample plan",
            alignment_issues,
        )

    def test_manual_gold_assurance_schedule_partitions_packet_without_promotion(
        self,
    ) -> None:
        assurance = json.loads(
            (GOLD_ROOT / "gold-assurance.v2.json").read_text(encoding="utf-8")
        )
        units = {unit["sample_id"]: unit for unit in assurance["units"]}
        self.assertEqual(
            [],
            foundation._human_work_schedule_issues(
                units, assurance["human_work_schedule"]
            ),
        )

        duplicate = copy.deepcopy(assurance)
        duplicate["human_work_schedule"]["unscheduled_unit_ids"].append(
            duplicate["human_work_schedule"]["selected_calibration_unit_ids"][0]
        )
        self.assertIn(
            "selected calibration and unscheduled units overlap",
            foundation._human_work_schedule_issues(
                units, duplicate["human_work_schedule"]
            ),
        )

    def test_manual_gold_assurance_allows_only_disclosed_delayed_solo_recheck(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLD_ASSURANCE_SCHEMA,
            REPO_ROOT,
        )
        assurance = json.loads(
            (GOLD_ROOT / "gold-assurance.v2.json").read_text(encoding="utf-8")
        )
        unit = assurance["units"][0]
        unit["current_assurance"] = "solo_human_delayed_rechecked"
        unit["reference_use"] = "calibration_metrics_with_disclosure"
        unit["next_route"] = "independent_multi_human_review"
        unit["review_evidence"] = {
            "pass_1_receipt_ref": "local-review/pass-1.json",
            "pass_2_receipt_ref": "local-review/pass-2.json",
            "adjudication_receipt_ref": None,
            "content_sha256": "a" * 64,
            "observed_delay_hours": 24,
            "same_reviewer": True,
        }
        self.assertEqual([], list(validator.iter_errors(assurance)))
        self.assertIsNone(
            foundation._solo_recheck_delay_issue(
                unit,
                assurance["solo_recheck_policy"]["minimum_delay_hours"],
            )
        )

        below_floor = copy.deepcopy(unit)
        below_floor["review_evidence"]["observed_delay_hours"] = 23.9
        self.assertIn(
            "below the declared delay floor",
            foundation._solo_recheck_delay_issue(below_floor, 24),
        )

        wrong_identity = copy.deepcopy(assurance)
        wrong_identity["units"][0]["review_evidence"]["same_reviewer"] = False
        self.assertTrue(list(validator.iter_errors(wrong_identity)))

        false_multi_human = copy.deepcopy(assurance)
        false_multi_human["units"][0]["reference_use"] = "independent_multi_human_gold"
        self.assertTrue(list(validator.iter_errors(false_multi_human)))
        self.assertIn(
            "invalid for solo_human_delayed_rechecked",
            foundation._assurance_reference_use_issue(
                false_multi_human["units"][0]
            ),
        )

        overlapping_scope = copy.deepcopy(assurance["language_scopes"][0])
        overlapping_scope["blocked_claims"].append(
            overlapping_scope["allowed_claims"][0]
        )
        self.assertIn(
            "both allows and blocks",
            foundation._language_scope_overlap_issue(overlapping_scope),
        )

    def test_manual_gold_assurance_digest_binding_detects_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            owner_path = repo_root / "owner.json"
            owner_path.write_text('{"version":1}\n', encoding="utf-8")
            digest_bound_ref = {
                "ref": "owner.json",
                "sha256": hashlib.sha256(owner_path.read_bytes()).hexdigest(),
            }
            self.assertIsNone(
                foundation._digest_bound_ref_issue(
                    digest_bound_ref,
                    expected_path=owner_path,
                    repo_root=repo_root,
                    field="owner",
                )
            )
            owner_path.write_text('{"version":2}\n', encoding="utf-8")
            self.assertEqual(
                "owner digest drifted",
                foundation._digest_bound_ref_issue(
                    digest_bound_ref,
                    expected_path=owner_path,
                    repo_root=repo_root,
                    field="owner",
                ),
            )

    def test_semantic_and_llm_evaluation_plans_do_not_materialize_false_tasks(self) -> None:
        semantic_validator, _ = foundation._schema_validator(
            foundation.SOURCE_GATED_SEMANTIC_EVALUATION_PLAN_SCHEMA,
            REPO_ROOT,
        )
        llm_validator, _ = foundation._schema_validator(
            foundation.SOURCE_GATED_LLM_EVALUATION_PLAN_SCHEMA,
            REPO_ROOT,
        )
        for filename, expected_kind, validator in (
            (
                "semantic-samples.json",
                "semantic-annotation",
                semantic_validator,
            ),
            (
                "llm-tasks.json",
                "llm-assistance",
                llm_validator,
            ),
        ):
            plan = json.loads((GOLD_ROOT / filename).read_text(encoding="utf-8"))
            self.assertEqual([], list(validator.iter_errors(plan)))
            self.assertEqual(expected_kind, plan["plan_kind"])
            self.assertEqual("blocked-not-materialized", plan["status"])
            self.assertEqual([], plan["tasks"])
            self.assertEqual(0, plan["result"]["run_count"])
            self.assertIsNone(plan["result"]["winner"])
            self.assertFalse(plan["result"]["promotion_authorized"])
            if expected_kind == "llm-assistance":
                prepared = plan["prepared_task_contract"]
                self.assertEqual(
                    "sign-candidate-and-refusal",
                    prepared["selected_task_family"],
                )
                self.assertEqual(20, prepared["required_total_tasks"])
                self.assertEqual([], prepared["source_anchor_refs"])
                self.assertFalse(prepared["task_instances_materialized"])
                self.assertFalse(prepared["source_text_present"])
                self.assertFalse(prepared["human_work_scheduled"])
                self.assertTrue(
                    prepared["profile_refresh_required_on_plan_change"]
                )
                self.assertEqual(
                    "task-specific-accepted-anchor-set",
                    plan["source_evidence_gate"]["gate_kind"],
                )
                self.assertEqual(
                    "task-specific-unassisted-human-baseline",
                    plan["human_baseline_gate"]["gate_kind"],
                )
                self.assertFalse(
                    plan["human_baseline_gate"]["human_work_scheduled"]
                )
                historical = plan["historical_gate_snapshot"]
                self.assertEqual(
                    "superseded-universal-30-15-gate",
                    historical["snapshot_kind"],
                )
                self.assertFalse(historical["scheduling_authority"])
                self.assertFalse(historical["execution_authority"])
            else:
                self.assertEqual(
                    "task-specific-accepted-anchor-set",
                    plan["task_specific_source_gate"]["gate_kind"],
                )
                self.assertFalse(
                    plan["task_specific_source_gate"][
                        "universal_packet_completion_required"
                    ]
                )
                assurance = plan["assurance_policy"]
                self.assertFalse(assurance["routine_human_work_for_prepared_rows"])
                self.assertFalse(assurance["human_work_scheduled"])
                self.assertEqual(
                    "triggered-exception-not-routine",
                    assurance["second_human_review_posture"],
                )
                self.assertFalse(assurance["model_disagreement_is_human_perspective"])
                historical = plan["historical_gate_snapshot"]
                self.assertFalse(historical["scheduling_authority"])
                self.assertFalse(historical["execution_authority"])

            false_task = copy.deepcopy(plan)
            false_task_payload = {
                "task_id": "tos-task-synthetic",
                "task_family": plan["task_families"][0],
                "stratum": "random",
                "source_anchor_refs": ["tos.anchor.synthetic"],
                "accepted_source_sha256": "a" * 64,
                "local_content_ref": "local-content/synthetic.json",
                "local_content_sha256": "b" * 64,
                "eligible_for_variant_execution": True,
            }
            if expected_kind == "llm-assistance":
                false_task_payload.update(
                    {
                        "task_family": "sign-candidate-and-refusal",
                        "source_review_event_ref": "tos.event.synthetic-review",
                        "human_baseline_ref": None,
                        "human_baseline_sha256": None,
                    }
                )
            else:
                false_task_payload.update(
                    {
                        "epistemic_layer": "textual_observation",
                        "assurance_route": "source-visible-evidence",
                        "source_review_event_ref": "tos.event.synthetic-review",
                        "required_context_refs": ["tos.anchor.synthetic-context"],
                        "language_competence_evidence_refs": [],
                        "unassisted_human_baseline_ref": None,
                        "unassisted_human_baseline_sha256": None,
                    }
                )
            false_task["tasks"] = [false_task_payload]
            self.assertTrue(list(validator.iter_errors(false_task)))

            premature_ready = copy.deepcopy(plan)
            premature_ready["status"] = "ready"
            self.assertTrue(list(validator.iter_errors(premature_ready)))

            false_run = copy.deepcopy(plan)
            false_run["result"]["run_count"] = 1
            false_run["result"]["run_refs"] = ["synthetic-run"]
            self.assertTrue(list(validator.iter_errors(false_run)))

            if expected_kind == "llm-assistance":
                false_prepared = copy.deepcopy(plan)
                false_prepared["prepared_task_contract"][
                    "source_anchor_refs"
                ] = ["tos.anchor.synthetic"]
                self.assertTrue(list(validator.iter_errors(false_prepared)))

                scheduled_human = copy.deepcopy(plan)
                scheduled_human["prepared_task_contract"][
                    "human_work_scheduled"
                ] = True
                self.assertTrue(list(validator.iter_errors(scheduled_human)))

                scheduled_historical_debt = copy.deepcopy(plan)
                scheduled_historical_debt["historical_gate_snapshot"][
                    "scheduling_authority"
                ] = True
                self.assertTrue(
                    list(validator.iter_errors(scheduled_historical_debt))
                )
            else:
                scheduled_human = copy.deepcopy(plan)
                scheduled_human["assurance_policy"]["human_work_scheduled"] = True
                self.assertTrue(list(validator.iter_errors(scheduled_human)))

                scheduled_historical_debt = copy.deepcopy(plan)
                scheduled_historical_debt["historical_gate_snapshot"][
                    "scheduling_authority"
                ] = True
                self.assertTrue(
                    list(validator.iter_errors(scheduled_historical_debt))
                )

    def test_mysl_work_boundaries_are_text_free_contiguous_and_unreviewed(self) -> None:
        boundary_map = json.loads(
            (MYSL_WORK_BOUNDARY_ROOT / "work-boundary-map.json").read_text(
                encoding="utf-8"
            )
        )
        anchors = [
            json.loads(line)
            for line in (MYSL_WORK_BOUNDARY_ROOT / "anchors.jsonl")
            .read_text(encoding="utf-8")
            .splitlines()
            if line
        ]

        self.assertFalse(boundary_map["source_text_included"])
        self.assertEqual("unreviewed", boundary_map["review_status"])
        self.assertTrue(
            all(
                member["epistemic_status"] == "inferred"
                for member in boundary_map["members"]
            )
        )
        self.assertEqual(
            [
                (5, 237),
                (238, 406),
                (407, 524),
                (525, 555),
                (556, 630),
                (631, 692),
                (693, 769),
            ],
            [
                (member["start_page"], member["end_page"])
                for member in boundary_map["members"]
            ],
        )
        self.assertEqual(11, len(anchors))
        self.assertTrue(all(anchor["status"] == "proposed" for anchor in anchors))
        self.assertTrue(
            all(
                selector["type"] == "page_region"
                for anchor in anchors
                for selector in anchor["selectors"]
            )
        )
        self.assertIn(
            "semantic units, signs, concepts, or relations",
            boundary_map["does_not_establish"],
        )

    def test_current_foundation_validates_without_private_payloads(self) -> None:
        self.assertEqual(
            [],
            foundation.validate_foundation(REPO_ROOT, require_local_payloads=False),
        )

    def test_current_foundation_validates_with_local_payloads_when_available(self) -> None:
        issues = foundation.validate_foundation(REPO_ROOT, require_local_payloads=True)
        missing_local_content = [
            issue
            for issue in issues
            if issue[1].startswith("required local ") and issue[1].endswith(" is missing")
        ]
        other_issues = [issue for issue in issues if issue not in missing_local_content]

        self.assertEqual([], other_issues)
        if missing_local_content:
            self.skipTest(
                f"{len(missing_local_content)} private local content files are unavailable"
            )
        self.assertEqual([], issues)

    def test_frozen_pilot_has_declared_counts_and_no_false_human_gold(self) -> None:
        sample_plan = json.loads((GOLD_ROOT / "sample-plan.json").read_text(encoding="utf-8"))
        ocr_plan = json.loads(
            (GOLD_ROOT / "ocr-visual-samples.json").read_text(encoding="utf-8")
        )
        gold_status = json.loads((GOLD_ROOT / "gold-status.json").read_text(encoding="utf-8"))
        translation_plan = json.loads(
            (GOLD_ROOT / "translation-samples.json").read_text(encoding="utf-8")
        )
        translation_laboratory_plan = json.loads(
            (GOLD_ROOT / "translation-laboratory-plan.v1.json").read_text(encoding="utf-8")
        )
        translation_reference_register = json.loads(
            (GOLD_ROOT / "translation-reference-register.v1.json").read_text(
                encoding="utf-8"
            )
        )
        retrieval_plan = json.loads(
            (GOLD_ROOT / "retrieval-queries.json").read_text(encoding="utf-8")
        )
        visual_retrieval_plan = json.loads(
            (GOLD_ROOT / "visual-retrieval-plan.v1.json").read_text(
                encoding="utf-8"
            )
        )
        graph_plan = json.loads((GOLD_ROOT / "graph-queries.json").read_text(encoding="utf-8"))
        transfer_plan = json.loads(
            (GOLD_ROOT / "transfer-samples.json").read_text(encoding="utf-8")
        )
        graph_claims = [
            json.loads(line)
            for line in (GOLD_ROOT / "graph-claims.jsonl").read_text(encoding="utf-8").splitlines()
        ]

        self.assertTrue(sample_plan["frozen_before_variant_outputs"])
        self.assertEqual([12, 12, 12], [group["sample_count"] for group in sample_plan["source_groups"]])
        self.assertEqual(
            [5, 5, 5],
            [sum(sample["gold_candidate"] for sample in group["samples"]) for group in sample_plan["source_groups"]],
        )
        self.assertTrue(ocr_plan["frozen_before_variant_outputs"])
        self.assertEqual([12, 12, 12], [group["sample_count"] for group in ocr_plan["source_groups"]])
        self.assertEqual(
            [5, 5, 5],
            [sum(sample["gold_candidate"] for sample in group["samples"]) for group in ocr_plan["source_groups"]],
        )
        self.assertEqual(36, sum(len(group["samples"]) for group in ocr_plan["source_groups"]))
        scan_group = next(group for group in ocr_plan["source_groups"] if group["language"] == "de")
        self.assertEqual(
            "tos.item.friedrich-nietzsche.also-sprach-zarathustra.de-naumann-1893.internet-archive-image-container-pdf",
            scan_group["item_ref"],
        )
        self.assertEqual(
            "tos.file.sha256.61c947e5aff76a64d82600cc52dcb25ff1b5862530d3a99c96824da885c1e6cf",
            scan_group["file_ref"],
        )
        self.assertEqual(
            [1, 2, 44, 46, 48, 50, 54, 56, 60, 201, 381, 523],
            [sample["page"] for sample in scan_group["samples"]],
        )
        self.assertEqual(
            1,
            sum(
                sample["projection_change"] == "replacement_for_nonvisual_unit"
                for group in ocr_plan["source_groups"]
                for sample in group["samples"]
            ),
        )
        self.assertEqual(
            {
                "model_draft": "not_started",
                "human_pass_1": "not_started",
                "human_pass_2": "not_started",
                "human_gold_materialized": False,
                "human_gold_ref": None,
                "formal_quality_metrics_status": "blocked_until_human_gold",
                "human_correction_time_seconds": None,
            },
            {
                key: ocr_plan["gold_gate"][key]
                for key in (
                    "model_draft",
                    "human_pass_1",
                    "human_pass_2",
                    "human_gold_materialized",
                    "human_gold_ref",
                    "formal_quality_metrics_status",
                    "human_correction_time_seconds",
                )
            },
        )
        self.assertEqual(15, len(gold_status["units"]))
        self.assertTrue(all(unit["gold_status"] == "candidate" for unit in gold_status["units"]))
        self.assertTrue(
            all(
                unit[pass_name]["status"] == "not_started"
                for unit in gold_status["units"]
                for pass_name in ("human_pass_1", "human_pass_2")
            )
        )
        self.assertTrue(translation_plan["frozen_before_drafts"])
        self.assertEqual(30, len(translation_plan["fragments"]))
        self.assertEqual("sealed", translation_plan["lanes"]["recognized_comparator"])
        self.assertTrue(
            all(fragment["human_source_acceptance"] is False for fragment in translation_plan["fragments"])
        )
        self.assertEqual(
            0,
            translation_laboratory_plan["source_review_gate"]["current_human_accepted_units"],
        )
        self.assertEqual(
            "frozen-blocked-on-human-source-acceptance",
            translation_laboratory_plan["status"],
        )
        self.assertEqual(
            {
                "human_only": "blocked-on-source-acceptance",
                "ai_only": "blocked-on-source-acceptance",
                "ai_alternatives": "blocked-on-source-acceptance",
                "ai_human": "blocked-on-source-acceptance",
            },
            {
                key: value["state"]
                for key, value in translation_laboratory_plan["blind_lanes"].items()
            },
        )
        self.assertEqual(
            "sealed",
            translation_laboratory_plan["recognized_comparator"]["visibility"],
        )
        self.assertEqual(17, len(translation_laboratory_plan["workflow_order"]))
        self.assertEqual(14, len(translation_reference_register["entries"]))
        self.assertEqual(
            set(translation_reference_register["required_categories"]),
            {
                entry["category"]
                for entry in translation_reference_register["entries"]
            },
        )
        self.assertEqual(0, translation_reference_register["coverage"]["content_admitted_entries"])
        self.assertEqual(0, translation_reference_register["coverage"]["human_bibliographic_reviews"])
        self.assertEqual(0, translation_reference_register["coverage"]["human_rights_reviews"])
        self.assertTrue(
            all(
                entry["access"]["content_ingested_for_translation_lab"] is False
                and entry["admission"]["accepted_as_truth"] is False
                for entry in translation_reference_register["entries"]
            )
        )
        comparator_entry = next(
            entry
            for entry in translation_reference_register["entries"]
            if entry["reference_id"]
            == "tos-ref.ru.antonovsky-cultural-revolution-2007"
        )
        self.assertIn(
            translation_laboratory_plan["recognized_comparator"]["expression_ref"],
            comparator_entry["tos_refs"]["record_refs"],
        )
        self.assertIn(
            translation_laboratory_plan["recognized_comparator"]["item_ref"],
            comparator_entry["tos_refs"]["record_refs"],
        )
        self.assertTrue(retrieval_plan["frozen_before_variant_outputs"])
        self.assertEqual(20, len(retrieval_plan["queries"]))
        self.assertEqual("not_started", retrieval_plan["human_judgment_status"])
        self.assertTrue(
            all(query["relevance_status"] == "model-proposed-awaiting-human" for query in retrieval_plan["queries"])
        )
        self.assertTrue(
            visual_retrieval_plan["frozen_before_challenger_outputs"]
        )
        self.assertEqual(
            "frozen-awaiting-runtime-admission",
            visual_retrieval_plan["status"],
        )
        self.assertEqual(
            hashlib.sha256(
                (GOLD_ROOT / "retrieval-queries.json").read_bytes()
            ).hexdigest(),
            visual_retrieval_plan["query_projection"][
                "source_query_plan_sha256"
            ],
        )
        self.assertEqual(
            20,
            visual_retrieval_plan["query_projection"]["resolved_query_count"],
        )
        self.assertEqual(
            36,
            visual_retrieval_plan["page_image_corpus"]["render_count"],
        )
        self.assertEqual(
            ["A", "B"],
            [
                control["label"]
                for control in visual_retrieval_plan["fixed_controls"]
            ],
        )
        self.assertTrue(
            all(
                control["reuse_posture"]
                == "immutable-completed-control-no-rerun"
                for control in visual_retrieval_plan["fixed_controls"]
            )
        )
        self.assertEqual(
            "Qwen/Qwen3-VL-Embedding-2B",
            visual_retrieval_plan["challenger"]["model_id"],
        )
        self.assertEqual(
            "9f2f7e710d6d81056aa5c0a4f04764fec6bb7bda",
            visual_retrieval_plan["challenger"]["source_revision"],
        )
        self.assertFalse(
            visual_retrieval_plan["human_assurance"][
                "routine_review_scheduled"
            ]
        )
        self.assertEqual(0, visual_retrieval_plan["result"]["run_count"])
        self.assertIsNone(visual_retrieval_plan["result"]["winner"])
        self.assertFalse(
            visual_retrieval_plan["rights_posture"][
                "public_page_images_authorized"
            ]
        )
        self.assertEqual(
            [],
            foundation._visual_retrieval_plan_issues(
                visual_retrieval_plan,
                source_query_plan=retrieval_plan,
                source_sample_plan=sample_plan,
                visual_sample_plan=ocr_plan,
            ),
        )
        self.assertTrue(graph_plan["frozen_before_variant_outputs"])
        self.assertEqual(10, len(graph_plan["queries"]))
        self.assertEqual(13, len(graph_claims))
        self.assertEqual("not_started", graph_plan["human_judgment_status"])
        self.assertTrue(all(claim["review_status"] == "unreviewed" for claim in graph_claims))
        self.assertEqual("blocked-not-run", transfer_plan["status"])
        self.assertEqual(0, transfer_plan["result"]["run_count"])
        self.assertEqual(
            {"tos-sample-mysl-p238", "tos-sample-mysl-p407", "tos-sample-mysl-p631"},
            {unit["sample_id"] for unit in transfer_plan["scouting_units"]},
        )
        self.assertTrue(
            all(
                unit["eligible_for_semantic_transfer"] is False
                for unit in transfer_plan["scouting_units"]
            )
        )
        self.assertEqual(
            hashlib.sha256((GOLD_ROOT / "graph-claims.jsonl").read_bytes()).hexdigest(),
            graph_plan["claim_set_sha256"],
        )

    def test_visual_retrieval_crosswalk_fails_closed_on_unresolved_query(self) -> None:
        visual_retrieval_plan = json.loads(
            (GOLD_ROOT / "visual-retrieval-plan.v1.json").read_text(
                encoding="utf-8"
            )
        )
        retrieval_plan = json.loads(
            (GOLD_ROOT / "retrieval-queries.json").read_text(encoding="utf-8")
        )
        sample_plan = json.loads(
            (GOLD_ROOT / "sample-plan.json").read_text(encoding="utf-8")
        )
        ocr_plan = json.loads(
            (GOLD_ROOT / "ocr-visual-samples.json").read_text(encoding="utf-8")
        )
        expected_anchor = retrieval_plan["queries"][0][
            "expected_source_anchor_refs"
        ][0]
        source_sample_id = next(
            sample["sample_id"]
            for group in sample_plan["source_groups"]
            for sample in group["samples"]
            if sample["anchor_ref"] == expected_anchor
        )
        broken_ocr_plan = copy.deepcopy(ocr_plan)
        visual_sample = next(
            sample
            for group in broken_ocr_plan["source_groups"]
            for sample in group["samples"]
            if sample["source_sample_id"] == source_sample_id
        )
        visual_sample["source_sample_id"] = "tos-sample-broken-crosswalk"

        issues = foundation._visual_retrieval_plan_issues(
            visual_retrieval_plan,
            source_query_plan=retrieval_plan,
            source_sample_plan=sample_plan,
            visual_sample_plan=broken_ocr_plan,
        )

        self.assertIn(
            "visual retrieval resolved query count drifted",
            issues,
        )
        self.assertIn(
            "visual retrieval unresolved query IDs drifted",
            issues,
        )
        self.assertIn(
            "visual retrieval one-to-one query crosswalk drifted",
            issues,
        )

    def test_frozen_visual_retrieval_plan_rejects_false_outputs(self) -> None:
        plan = json.loads(
            (GOLD_ROOT / "visual-retrieval-plan.v1.json").read_text(
                encoding="utf-8"
            )
        )
        validator, _ = foundation._schema_validator(
            foundation.VISUAL_RETRIEVAL_PLAN_SCHEMA,
            REPO_ROOT,
        )

        false_run = copy.deepcopy(plan)
        false_run["result"]["run_count"] = 1
        false_run["result"]["run_refs"] = ["synthetic-run"]
        self.assertTrue(list(validator.iter_errors(false_run)))

        scheduled_human = copy.deepcopy(plan)
        scheduled_human["human_assurance"]["routine_review_scheduled"] = True
        self.assertTrue(list(validator.iter_errors(scheduled_human)))

        moving_model = copy.deepcopy(plan)
        moving_model["challenger"]["source_revision"] = "0" * 40
        self.assertTrue(list(validator.iter_errors(moving_model)))

    def test_restricted_gold_content_is_ignored_but_route_card_is_trackable(self) -> None:
        route_card = GOLD_ROOT / "local-content/README.md"
        restricted_example = GOLD_ROOT / "local-content/gold/example.json"
        retrieval_content = GOLD_ROOT / "local-content/retrieval/queries.v1.json"
        self.assertFalse(foundation._git_ignored(REPO_ROOT, route_card))
        self.assertTrue(foundation._git_ignored(REPO_ROOT, restricted_example))
        self.assertTrue(foundation._git_ignored(REPO_ROOT, retrieval_content))

    def test_missing_local_payload_is_optional_only_when_not_required(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            item_directory = repo_root / "item"
            item_directory.mkdir()
            entry = {
                "relative_path": "payload/source.txt",
                "byte_size": 3,
                "sha256": hashlib.sha256(b"abc").hexdigest(),
            }

            self.assertEqual(
                [],
                foundation.validate_payload_file(
                    repo_root,
                    item_directory,
                    entry,
                    require_local_payloads=False,
                ),
            )
            issues = foundation.validate_payload_file(
                repo_root,
                item_directory,
                entry,
                require_local_payloads=True,
            )
            self.assertTrue(any("required local payload is missing" in message for _, message in issues))

    def test_present_payload_must_match_size_and_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo_root = Path(temporary)
            item_directory = repo_root / "item"
            payload_directory = item_directory / "payload"
            payload_directory.mkdir(parents=True)
            (payload_directory / "source.txt").write_bytes(b"different")
            entry = {
                "relative_path": "payload/source.txt",
                "byte_size": 3,
                "sha256": hashlib.sha256(b"abc").hexdigest(),
            }

            issues = foundation.validate_payload_file(
                repo_root,
                item_directory,
                entry,
                require_local_payloads=True,
            )
            messages = [message for _, message in issues]
            self.assertTrue(any("byte size" in message for message in messages))
            self.assertTrue(any("sha256" in message for message in messages))


if __name__ == "__main__":
    unittest.main()

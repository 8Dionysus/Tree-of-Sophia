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
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import validate_source_witness_foundation as foundation


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
            "page_return_verified": True,
        },
        "frequency_and_concordance": {
            "count": 3,
            "concordance_refs": ["synthetic-concordance"],
            "counting_method_ref": "synthetic-counting-method",
            "frequency_only_basis": False,
        },
        "translation_correspondences": {
            "translation_evidence_ids": ["tos-translation-evidence.synthetic"],
            "correspondence_claim_refs": ["synthetic-translation-correspondence"],
            "translation_differences_preserved": True,
            "recognized_translation_is_ground_truth": False,
        },
        "stable_sign_candidate": {
            "candidate_statement": "Synthetic stable-sign candidate, not a decision.",
            "occurrence_refs": ["tos.occurrence.synthetic-form"],
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
        "schema_version": "tos_semantic_ladder_packet_v2",
        "packet_id": "tos.semantic-ladder-packet.synthetic-foundation",
        "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
        "source_expression_ref": "tos.expression.synthetic-german",
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
            "diplomatic": "Synthetic exact form",
            "diplomatic_sha256": "a" * 64,
            "normalized": "Synthetic exact form",
            "normalized_sha256": "a" * 64,
        },
        "candidate_ref": "tos.annotation.synthetic-sign-candidate",
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
        "stage_order": SEMANTIC_STAGE_ORDER,
        "stages": stages,
        "frequency_is_not_sufficient": True,
        "every_stage_returns_to_source_page": True,
        "model_may_propose_but_not_confirm": True,
        "manual_sign_decision_required_before_interpretation": True,
        "packet_status": "awaiting-manual-sign-decision",
        "provenance_event_refs": ["tos.event.synthetic-semantic-packet"],
        "authority_boundary": "this packet preserves a source-returnable path from exact form to graph; frequency, model proposals, interpretations, and projections cannot confirm a stable sign without a real-human evidence-bearing decision",
        "packet_version": 1,
    }


def _accept_synthetic_sign(packet: dict) -> dict:
    packet["stages"][10] = _synthetic_semantic_stage(
        "manual_confirmation_or_rejection",
        status="human-accepted",
        body={
            "decision": "accept-with-limits",
            "sign_status_assigned": "stable-sign",
            "rationale": "Synthetic real-human decision fixture.",
            "review_receipt_ref": "synthetic-manual-sign-review",
            "considered_evidence_refs": ["synthetic-context", "synthetic-recurrence"],
            "decision_owned_by_real_human": True,
            "frequency_was_not_sole_basis": True,
            "ai_proposal_is_authority": False,
        },
        maker=_synthetic_semantic_maker("human"),
    )
    packet["stages"][10]["review_status"] = "accepted-with-limits"
    packet["packet_status"] = "manual-sign-accepted"
    return packet


def _project_synthetic_semantic_graph(packet: dict) -> dict:
    for index in (11, 12, 13):
        stage_name = SEMANTIC_STAGE_ORDER[index]
        packet["stages"][index] = _synthetic_semantic_stage(
            stage_name,
            status="proposed",
            body={"claim_refs": [f"tos.claim.synthetic-{index}"]},
            maker=_synthetic_semantic_maker(),
        )
    packet["stages"][14] = _synthetic_semantic_stage(
        "graph_projection",
        status="projected",
        body={
            "projection_ref": "ToS/derived-exports/synthetic-semantic-graph.json",
            "projection_sha256": "c" * 64,
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

    def test_discovery_access_and_server_boundaries_fail_closed(self) -> None:
        discovery_validator, _ = foundation._schema_validator(
            foundation.MATERIAL_DISCOVERY_SCHEMA,
            REPO_ROOT,
        )
        discovery = _synthetic_discovery_record()
        self.assertEqual([], list(discovery_validator.iter_errors(discovery)))

        bypassed = copy.deepcopy(discovery)
        bypassed["technical_access_bypass_used"] = True
        self.assertTrue(list(discovery_validator.iter_errors(bypassed)))

        false_download = copy.deepcopy(discovery)
        false_download["channels"][0]["results"][0]["acquisition"]["downloaded"] = True
        self.assertTrue(list(discovery_validator.iter_errors(false_download)))

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

    def test_golden_kernel_transfer_plan_fails_closed_without_human_kernel(self) -> None:
        validator, _ = foundation._schema_validator(
            foundation.GOLDEN_KERNEL_TRANSFER_PLAN_SCHEMA,
            REPO_ROOT,
        )
        transfer_path = GOLD_ROOT / "transfer-samples.json"
        transfer_plan = json.loads(transfer_path.read_text(encoding="utf-8"))

        self.assertEqual([], list(validator.iter_errors(transfer_plan)))
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
        validator, _ = foundation._schema_validator(
            foundation.SOURCE_GATED_EVALUATION_PLAN_SCHEMA,
            REPO_ROOT,
        )
        for filename, expected_kind in (
            ("semantic-samples.json", "semantic-annotation"),
            ("llm-tasks.json", "llm-assistance"),
        ):
            plan = json.loads((GOLD_ROOT / filename).read_text(encoding="utf-8"))
            self.assertEqual([], list(validator.iter_errors(plan)))
            self.assertEqual(expected_kind, plan["plan_kind"])
            self.assertEqual("blocked-not-materialized", plan["status"])
            self.assertEqual([], plan["tasks"])
            self.assertEqual(0, plan["result"]["run_count"])
            self.assertIsNone(plan["result"]["winner"])
            self.assertFalse(plan["result"]["promotion_authorized"])

            false_task = copy.deepcopy(plan)
            false_task["tasks"] = [
                {
                    "task_id": "tos-task-synthetic",
                    "task_family": plan["task_families"][0],
                    "stratum": "random",
                    "source_anchor_refs": ["tos.anchor.synthetic"],
                    "accepted_source_sha256": "a" * 64,
                    "local_content_ref": "local-content/synthetic.json",
                    "local_content_sha256": "b" * 64,
                    "human_gold_ref": "local-content/synthetic-gold.json",
                    "human_gold_sha256": "c" * 64,
                    "eligible_for_variant_execution": True,
                }
            ]
            self.assertTrue(list(validator.iter_errors(false_task)))

            premature_ready = copy.deepcopy(plan)
            premature_ready["status"] = "ready"
            self.assertTrue(list(validator.iter_errors(premature_ready)))

            false_run = copy.deepcopy(plan)
            false_run["result"]["run_count"] = 1
            false_run["result"]["run_refs"] = ["synthetic-run"]
            self.assertTrue(list(validator.iter_errors(false_run)))

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

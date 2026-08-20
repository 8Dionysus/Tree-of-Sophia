#!/usr/bin/env python3
"""Build one local-only German input for bounded translation calibration.

The source string is derived deterministically from the registered DTA TEI
payload and written only below an explicit local output root. The tracked
packet and provenance contain selectors, digests, counts, and closed gates,
but no source text. This builder performs no network access, model inference,
translation, philological acceptance, or promotion.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

import build_zarathustra_german_source_triangulation as triangulation


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1"
)
PACKET_PATH = GOLD_ROOT / (
    "bounded-translation-research-input."
    "za-i-vorrede-1-opening-sentence.v1.json"
)
PROVENANCE_PATH = (
    GOLD_ROOT / "provenance.bounded-translation-research-input.jsonl"
)
LOCAL_ARTIFACT_RELATIVE = Path(
    "works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1/local-content/translation/"
    "research-inputs/za-i-vorrede-1-opening-sentence.v1.json"
)
LOCAL_ARTIFACT_REPO_REF = (
    Path("ToS/source-witnesses") / LOCAL_ARTIFACT_RELATIVE
)

SCHEMA_REF = (
    "https://tree-of-sophia.local/ToS/contracts/"
    "bounded-translation-research-input.schema.json"
)
PACKET_ID = (
    "tos.bounded-translation-research-input."
    "za-i-vorrede-1-opening-sentence.v1"
)
EVENT_ID = (
    "tos.event.segmentation.zarathustra-bounded-translation-input."
    "za-i-vorrede-1-opening-sentence.2026-07-29"
)
PREPARED_AT = "2026-07-29T08:31:00Z"

DECISION_PATH = Path(
    "docs/decisions/"
    "TOS-D-0020-corpus-evidence-spine-and-witness-storage.md"
)
RESEARCH_PATH = Path(
    "ToS/research-packets/foundation-laboratory-2026-07/"
    "BOUNDED_TRANSLATION_SOURCE_ADMISSION_RESEARCH.md"
)
TRIANGULATION_PATH = GOLD_ROOT / (
    "german-source-triangulation."
    "ekgwb-dta-naumann.za-i-vorrede-1.v1.json"
)
SOURCE_REVIEW_PATH = GOLD_ROOT / "translation-source-review-plan.v2.json"
TRANSLATION_PLAN_PATH = GOLD_ROOT / "translation-laboratory-plan.v1.json"
PREEXISTING_CANON_NODE_PATH = Path(
    "ToS/canon/source/friedrich-nietzsche/"
    "thus-spoke-zarathustra/prologue-1/node.json"
)
PREEXISTING_PUBLIC_MIRROR_PATH = Path(
    "ToS/public-compatibility/source_node.example.json"
)
DTA_MANIFEST_PATH = (
    Path("ToS/source-witnesses")
    / triangulation.DTA_ITEM_ROOT
    / "item.manifest.json"
)
DTA_RIGHTS_PATH = (
    Path("ToS/source-witnesses")
    / triangulation.DTA_ITEM_ROOT
    / "rights.json"
)

AUTHORITY_BOUNDARY = (
    "one exact local DTA-derived string is eligible only for blind local "
    "machine-method calibration; the packet grants no German, translation, "
    "semantic, rights, graph, or canon authority"
)


class BoundedInputBuildError(RuntimeError):
    """Raised when an input, binding, or non-promotional boundary drifts."""


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _render_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2) + "\n"


def _render_jsonl(payloads: list[dict[str, Any]]) -> str:
    return "".join(
        json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n"
        for payload in payloads
    )


def _digest_bound(repo_root: Path, path: Path) -> dict[str, str]:
    absolute = repo_root / path
    if not absolute.is_file():
        raise BoundedInputBuildError(f"binding is absent: {path}")
    return {
        "ref": path.as_posix(),
        "sha256": _sha256_path(absolute),
    }


def _collapse_whitespace(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def _first_sentence(paragraph: str) -> str:
    boundary = paragraph.find(".")
    if boundary < 0:
        raise BoundedInputBuildError(
            "first paragraph has no deterministic full-stop boundary"
        )
    sentence = _collapse_whitespace(paragraph[: boundary + 1])
    if not sentence:
        raise BoundedInputBuildError("selected sentence is empty")
    return sentence


def _load_source_strings(
    *,
    payload_source_root: Path,
) -> tuple[str, str, list[str]]:
    dta_path = payload_source_root / triangulation.DTA_PAYLOAD_PATH
    ekgwb_path = payload_source_root / triangulation.EKGWB_LOCAL_PATH
    naumann_path = payload_source_root / triangulation.NAUMANN_PAYLOAD_PATH
    for path, expected_digest in (
        (dta_path, triangulation.DTA_DIGEST),
        (ekgwb_path, triangulation.EKGWB_DIGEST),
        (naumann_path, triangulation.NAUMANN_DIGEST),
    ):
        if not path.is_file():
            raise BoundedInputBuildError(f"local payload is absent: {path}")
        if _sha256_path(path) != expected_digest:
            raise BoundedInputBuildError(f"local payload digest drifted: {path}")

    _raw_dta, dta_paragraphs = triangulation._extract_dta_paragraphs(
        dta_path.read_bytes()
    )
    ekgwb_paragraphs = triangulation._extract_ekgwb_paragraphs(
        ekgwb_path.read_bytes()
    )
    dta_sentence = _first_sentence(dta_paragraphs[0])
    ekgwb_sentence = _first_sentence(ekgwb_paragraphs[0])
    naumann_tokens, _member_refs = triangulation._extract_epub_tokens(
        naumann_path
    )
    return dta_sentence, ekgwb_sentence, naumann_tokens


def build_outputs(
    *,
    repo_root: Path,
    payload_source_root: Path,
    prepared_at: str = PREPARED_AT,
) -> tuple[str, str, str]:
    """Return tracked packet, tracked provenance, and local source artifact."""

    dta_sentence, ekgwb_sentence, naumann_tokens = _load_source_strings(
        payload_source_root=payload_source_root
    )
    dta_tokens = triangulation._alpha_tokens(dta_sentence)
    ekgwb_tokens = triangulation._alpha_tokens(ekgwb_sentence)
    if len(dta_tokens) != 20:
        raise BoundedInputBuildError(
            f"selected normalized token count drifted: {len(dta_tokens)}"
        )
    if dta_sentence.encode("utf-8") != ekgwb_sentence.encode("utf-8"):
        raise BoundedInputBuildError(
            "DTA and eKGWB selected strings are no longer byte-identical"
        )
    if dta_tokens != ekgwb_tokens:
        raise BoundedInputBuildError(
            "DTA and eKGWB selected token sequences drifted"
        )
    if naumann_tokens[:20] != dta_tokens:
        raise BoundedInputBuildError(
            "Naumann opening tokens no longer corroborate the selected unit"
        )

    source_text_bytes = dta_sentence.encode("utf-8")
    sequence_digest = triangulation._sequence_digest(dta_tokens)
    local_artifact = {
        "schema_version": "tos_local_translation_research_input_v1",
        "artifact_id": (
            "tos.local-translation-research-input."
            "za-i-vorrede-1-opening-sentence.v1"
        ),
        "prepared_at": prepared_at,
        "visibility": "gitignored-local-only",
        "purpose": "translation_method_calibration",
        "source": {
            "provider_role": "registered_dta_provider_transcription",
            "item_ref": (
                "tos.item.friedrich-nietzsche.also-sprach-zarathustra."
                "de-schmeitzner-1883-part-1.dta-sbb-corrected-tei-p5"
            ),
            "file_ref": (
                "tos.file.sha256."
                "d3fa5f8af39d87c8f0ede7a0b52b26da0c57e73125d3b0bd2a836b80eae24a4b"
            ),
            "file_sha256": triangulation.DTA_DIGEST,
            "selector": {
                "section": "TEI/text[1]/body[1]/div[1]/div[1]",
                "paragraph_ordinal": 1,
                "sentence_ordinal": 1,
                "boundary_rule": (
                    "first-full-stop-inclusive-after-source-aware-"
                    "dta-dehyphenation"
                ),
            },
        },
        "transformations": [
            (
                "remove_dta_printed_hyphen_marker_u00ac_and_following_"
                "whitespace"
            ),
            "select_first_full_stop_inclusive",
            "collapse_unicode_whitespace_to_ascii_space",
        ],
        "source_text": dta_sentence,
        "source_text_sha256": _sha256_bytes(source_text_bytes),
        "source_text_codepoints": len(dta_sentence),
        "normalized_alpha_tokens": len(dta_tokens),
        "normalized_sequence_sha256": sequence_digest,
        "corroboration": {
            "ekgwb_whitespace_collapsed_exact": True,
            "naumann_first_twenty_normalized_tokens_exact": True,
        },
        "authority": {
            "philologically_accepted_source": False,
            "accepted_translation_input": False,
            "redistribution_authorized": False,
            "semantic_or_canon_promotion_authorized": False,
            "preexisting_authored_canon_source_revalidated": False,
        },
    }
    local_rendered = _render_json(local_artifact)
    local_bytes = local_rendered.encode("utf-8")

    packet = {
        "$schema": SCHEMA_REF,
        "schema_version": "tos_bounded_translation_research_input_v1",
        "packet_id": PACKET_ID,
        "status": "eligible_for_local_machine_calibration",
        "prepared_at": prepared_at,
        "work_ref": (
            "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        ),
        "target": {
            "review_unit_id": "tos-translation-source-review-v2-001",
            "context_anchor_ref": (
                "tos.anchor.zarathustra-translation-pilot-v1.t001"
            ),
            "critical_locator_siglum": "Za-I-Vorrede-1",
            "unit_kind": "first-complete-sentence",
            "unit_ordinal": 1,
        },
        "bindings": {
            "corpus_decision": _digest_bound(repo_root, DECISION_PATH),
            "admission_research": _digest_bound(repo_root, RESEARCH_PATH),
            "source_review_plan": _digest_bound(
                repo_root,
                SOURCE_REVIEW_PATH,
            ),
            "accepted_translation_plan": _digest_bound(
                repo_root,
                TRANSLATION_PLAN_PATH,
            ),
            "german_source_triangulation": _digest_bound(
                repo_root,
                TRIANGULATION_PATH,
            ),
            "preexisting_authored_canon_node": _digest_bound(
                repo_root,
                PREEXISTING_CANON_NODE_PATH,
            ),
            "preexisting_public_compatibility_mirror": _digest_bound(
                repo_root,
                PREEXISTING_PUBLIC_MIRROR_PATH,
            ),
            "dta_item_manifest": _digest_bound(
                repo_root,
                DTA_MANIFEST_PATH,
            ),
            "dta_rights_record": _digest_bound(
                repo_root,
                DTA_RIGHTS_PATH,
            ),
        },
        "source_derivation": {
            "source_role": "registered_dta_provider_transcription",
            "item_ref": (
                "tos.item.friedrich-nietzsche.also-sprach-zarathustra."
                "de-schmeitzner-1883-part-1.dta-sbb-corrected-tei-p5"
            ),
            "file_ref": (
                "tos.file.sha256."
                "d3fa5f8af39d87c8f0ede7a0b52b26da0c57e73125d3b0bd2a836b80eae24a4b"
            ),
            "file_sha256": triangulation.DTA_DIGEST,
            "payload_relative_path": triangulation.DTA_PAYLOAD_PATH.as_posix(),
            "selector": {
                "kind": (
                    "tei-structural-plus-deterministic-sentence-boundary"
                ),
                "section": "TEI/text[1]/body[1]/div[1]/div[1]",
                "paragraph_ordinal": 1,
                "sentence_ordinal": 1,
                "boundary_rule": (
                    "first-full-stop-inclusive-after-source-aware-"
                    "dta-dehyphenation"
                ),
            },
            "transformations": [
                (
                    "remove_dta_printed_hyphen_marker_u00ac_and_following_"
                    "whitespace"
                ),
                "select_first_full_stop_inclusive",
                "collapse_unicode_whitespace_to_ascii_space",
            ],
            "model_used_for_boundary_or_text": False,
            "source_text_emitted_to_tracked_packet": False,
        },
        "local_artifact": {
            "ref": LOCAL_ARTIFACT_REPO_REF.as_posix(),
            "artifact_sha256": _sha256_bytes(local_bytes),
            "artifact_byte_size": len(local_bytes),
            "source_text_sha256": _sha256_bytes(source_text_bytes),
            "source_text_codepoints": len(dta_sentence),
            "normalized_alpha_tokens": len(dta_tokens),
            "normalized_sequence_sha256": sequence_digest,
            "gitignored": True,
            "source_text_copied_into_tracked_admission": False,
            "source_text_present_locally": True,
        },
        "machine_corroboration": {
            "dta_ekgwb_whitespace_collapsed_exact": True,
            "dta_ekgwb_normalized_tokens_exact": True,
            "naumann_first_twenty_normalized_tokens_exact": True,
            "ekgwb_is_one_way_corroboration_only": True,
            "philological_acceptance_performed": False,
        },
        "experiment_scope": {
            "purpose": "translation_method_calibration",
            "runtime_owner": "abyss-stack",
            "environment": "local-only",
            "allowed_uses": [
                "blind-ai-only-model-prompt-decoding-calibration",
                "runtime-cost-speed-and-resource-measurement",
                "source-return-and-reproducibility-checks",
                (
                    "operator-russian-readability-rhythm-and-usefulness-"
                    "feedback"
                ),
                "sourced-or-unresolved-machine-analysis-proposals",
            ],
            "recognized_comparator_visible": False,
            "preexisting_authored_translation_surfaces_visible": False,
            "outputs_are_candidates_only": True,
        },
        "rights_and_visibility": {
            "dta_rights_posture": "conflicting-provider-fields-unreviewed",
            "human_rights_review": False,
            "local_research_only": True,
            (
                "source_or_derivative_redistribution_authorized_by_this_route"
            ): False,
            "public_site_upload_authorized": False,
            "tracked_packet_contains_source_text": False,
        },
        "gate_effects": {
            "bounded_local_research_inputs": 1,
            "human_debt_units": 0,
            "accepted_german_units": 0,
            "accepted_translation_lanes_opened": [],
            "semantic_tasks_opened": 0,
            "canon_or_graph_promotion_authorized": False,
        },
        "authority_boundary": AUTHORITY_BOUNDARY,
        "does_not_establish": [
            "philologically_accepted_source",
            "accepted_german_orthography_or_grammar",
            "translation_fidelity_or_quality",
            "accepted_translation_lane",
            "rights_clearance_or_redistribution",
            "semantic_sign_or_relation",
            "graph_truth_or_canon_promotion",
            "preexisting_authored_canon_source_revalidated",
        ],
    }
    packet_rendered = _render_json(packet)
    provenance = {
        "schema_version": "tos_provenance_event_v1",
        "event_id": EVENT_ID,
        "event_type": "segmentation",
        "started_at": prepared_at,
        "ended_at": prepared_at,
        "agent_refs": ["model:codex", "software:python-stdlib"],
        "inputs": [
            {
                "ref": (
                    "tos.file.sha256."
                    "d3fa5f8af39d87c8f0ede7a0b52b26da0c57e73125d3b0bd2a836b80eae24a4b"
                ),
                "role": "fixity-verified-dta-provider-transcription",
                "sha256": triangulation.DTA_DIGEST,
            },
            {
                "ref": TRIANGULATION_PATH.as_posix(),
                "role": "text-free-three-witness-machine-corroboration",
                "sha256": _sha256_path(repo_root / TRIANGULATION_PATH),
            },
            {
                "ref": RESEARCH_PATH.as_posix(),
                "role": "source-ordered-non-promotional-admission-research",
                "sha256": _sha256_path(repo_root / RESEARCH_PATH),
            },
            {
                "ref": PREEXISTING_CANON_NODE_PATH.as_posix(),
                "role": (
                    "preexisting-authored-source-and-translation-surface-"
                    "sealed-from-blind-run"
                ),
                "sha256": _sha256_path(
                    repo_root / PREEXISTING_CANON_NODE_PATH
                ),
            },
            {
                "ref": PREEXISTING_PUBLIC_MIRROR_PATH.as_posix(),
                "role": (
                    "preexisting-public-compatibility-translation-surface-"
                    "sealed-from-blind-run"
                ),
                "sha256": _sha256_path(
                    repo_root / PREEXISTING_PUBLIC_MIRROR_PATH
                ),
            },
        ],
        "outputs": [
            {
                "ref": PACKET_PATH.as_posix(),
                "role": "text-free-bounded-local-calibration-admission",
                "sha256": _sha256_bytes(packet_rendered.encode("utf-8")),
            },
            {
                "ref": LOCAL_ARTIFACT_REPO_REF.as_posix(),
                "role": "gitignored-local-only-source-string",
                "sha256": _sha256_bytes(local_bytes),
            },
        ],
        "method": {
            "maker_type": "software",
            "name": "deterministic-dta-opening-sentence-derivation",
            "version": "1",
            "artifact_digest": _sha256_path(Path(__file__)),
            "runtime": "Python standard library",
            "device": "abyss-machine",
            "configuration": {
                "network_access": False,
                "model_inference": False,
                "translation_performed": False,
                "source_text_written_to_gitignored_artifact_only": True,
                "preexisting_authored_translation_surfaces_visible": False,
                "normalized_alpha_tokens": 20,
                "accepted_german_units": 0,
                "accepted_translation_lanes_opened": 0,
            },
            "prompt_or_instruction_ref": RESEARCH_PATH.as_posix(),
        },
        "status": "completed_with_warnings",
        "warnings": [
            (
                "the DTA rights record contains conflicting current provider "
                "fields and has no human rights review"
            ),
            (
                "machine agreement across witnesses is not philological "
                "acceptance"
            ),
            (
                "the local source string may be used only for bounded local "
                "machine-method calibration"
            ),
            (
                "matching source and translation strings already exist in "
                "preexisting authored canon and compatibility surfaces; "
                "those surfaces are sealed from the blind run and are not "
                "revalidated by this route"
            ),
            (
                "no accepted German, translation, semantic, graph, rights, "
                "or canon gate was opened"
            ),
        ],
        "receipt_refs": [
            PACKET_PATH.as_posix(),
        ],
        "rights_basis_ref": DTA_RIGHTS_PATH.as_posix(),
        "event_version": 1,
        "supersedes_event_ref": None,
    }
    return packet_rendered, _render_jsonl([provenance]), local_rendered


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _check(path: Path, expected: str) -> bool:
    try:
        actual = path.read_text(encoding="utf-8")
    except OSError as exc:
        print(f"{path}: cannot read output: {exc}", file=sys.stderr)
        return False
    if actual != expected:
        print(f"{path}: output differs from deterministic build", file=sys.stderr)
        return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help="Tree of Sophia checkout containing tracked packet outputs",
    )
    parser.add_argument(
        "--payload-source-root",
        type=Path,
        required=True,
        help="Explicit ToS/source-witnesses root containing local payloads",
    )
    parser.add_argument(
        "--local-output-root",
        type=Path,
        required=True,
        help="Explicit ToS/source-witnesses root for the ignored local artifact",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check tracked and local outputs without modifying them",
    )
    args = parser.parse_args()
    repo_root = args.repo_root.resolve()
    payload_source_root = args.payload_source_root.resolve()
    local_output_root = args.local_output_root.resolve()
    try:
        packet, provenance, local_artifact = build_outputs(
            repo_root=repo_root,
            payload_source_root=payload_source_root,
        )
    except (BoundedInputBuildError, OSError) as exc:
        print(f"bounded translation input build failed: {exc}", file=sys.stderr)
        return 1

    outputs = (
        (repo_root / PACKET_PATH, packet),
        (repo_root / PROVENANCE_PATH, provenance),
        (local_output_root / LOCAL_ARTIFACT_RELATIVE, local_artifact),
    )
    if args.check:
        return 0 if all(_check(path, content) for path, content in outputs) else 1
    for path, content in outputs:
        _write(path, content)
    print(f"wrote {repo_root / PACKET_PATH}")
    print(f"wrote {repo_root / PROVENANCE_PATH}")
    print(f"wrote local-only {local_output_root / LOCAL_ARTIFACT_RELATIVE}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Prepare or validate one source-bound on-demand Zarathustra word analysis."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker, ValidationError


REPO = Path(__file__).resolve().parents[1]
ROUTE = REPO / "ToS/candidate-intake/zarathustra/concept-workbench-v1"
QUERY_PATH = REPO / "scripts/query_zarathustra_concept_workbench_v1.py"
TASK_SCHEMA_PATH = ROUTE / "word-analysis-task.v1.schema.json"
CANDIDATE_SCHEMA_PATH = ROUTE / "english-translation-candidate.v1.schema.json"
DEFAULT_REQUEST = ROUTE / "requests/fate.concept-request.v2.json"


class WordAnalysisError(RuntimeError):
    pass


def sha_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def sha_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise WordAnalysisError(f"JSON object required: {path}")
    return value


def load_query_module() -> Any:
    spec = importlib.util.spec_from_file_location("tos_zarathustra_concept_search_for_word_analysis", QUERY_PATH)
    if spec is None or spec.loader is None:
        raise WordAnalysisError(f"cannot load concept-search adapter: {QUERY_PATH}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def build_task(
    query: str,
    language: str,
    rank: int = 1,
    include_semantic_neighbors: bool = False,
    request_path: Path | None = None,
) -> dict[str, Any]:
    if rank < 1:
        raise WordAnalysisError("rank must be one or greater")
    selected_request = (request_path or DEFAULT_REQUEST).resolve()
    request = load_json(selected_request)
    search = load_query_module().build_result(
        query,
        language,
        selected_request,
        include_semantic_neighbors,
        rank,
    )
    if len(search["results"]) < rank:
        raise WordAnalysisError(
            f"rank {rank} exceeds {search['coverage']['total_source_results']} source results"
        )
    row = search["results"][rank - 1]
    task_id = "tos.annotation.word-analysis-task.sid-" + sha_text(
        "zarathustra-word-analysis-v1\n" + row["source_occurrence_candidate_ref"]
    )[:32]
    russian = [
        {
            **comparator,
            "context_sha256": sha_text(comparator["exact_text"]),
        }
        for comparator in row["russian_comparators"]
    ]
    relative_builder = Path(__file__).resolve().relative_to(REPO).as_posix()
    task = {
        "schema_version": "tos_zarathustra_word_analysis_task_v1",
        "analysis_task_id": task_id,
        "request_ref": search["concept_search_route"]["current_request_id"],
        "concept_search_result_ref": search["search_result_id"],
        "query_analysis": search["query_analysis"],
        "source": {
            "language": "de",
            "occurrence_candidate_ref": row["source_occurrence_candidate_ref"],
            "existing_occurrence_ref": row["source_existing_occurrence_ref"],
            "context_unit_ref": row["source_context_unit_ref"],
            "surface": row["source_surface"],
            "analysis_form": row["source_analysis_form"],
            "headword_candidate": row["source_headword_candidate"],
            "headword_status": row["source_headword_status"],
            "exact_context": row["source_context"],
            "surface_sha256": sha_text(row["source_surface"]),
            "context_sha256": sha_text(row["source_context"]),
            "anchor_refs": row["anchor_refs"],
            "part": row["part"],
            "reading_ref": row["reading_ref"],
            "unit_kind": row["unit_kind"],
            "witness_order": row["witness_order"],
            "token_ordinal": row["token_ordinal"],
            "speaker": row["speaker"],
        },
        "russian_comparators": russian,
        "recurrence_navigation": {
            "operation_id": "tos.zarathustra.word-analysis.prepare",
            "total_source_results": search["coverage"]["total_source_results"],
            "current_rank": rank,
            "next_rank": rank + 1 if rank < search["coverage"]["total_source_results"] else None,
            "query": query,
            "language": language,
            "semantic_neighbors_included": search["coverage"]["semantic_neighbors_included"],
        },
        "english_on_demand_task_ref": row["english_on_demand_task_ref"],
        "analysis_policy": {
            "required_stages": [
                "morphology", "syntax", "historical_sense", "sourced_etymology",
                "contextual_semantics", "intra_work_recurrence",
                "russian_witness_comparison", "english_rendering",
            ],
            "source_language": "de",
            "output_language": "en",
            "lemma_posture": "candidate_until_occurrence_visible_review",
            "etymology": {
                "state": "citation_required_before_claim",
                "minimum_citation_count": 1,
                "point_consultation_only": True,
                "bulk_ingest_allowed": False,
                "evidence_route_ref": request["english_generation"]["etymology_route"]["ref"],
                "reference_register_ref": request["english_generation"]["reference_register"]["ref"],
                "etymological_fallacy_guard": (
                    "word_history_may_inform_but_never_determine_contextual_meaning_or_authorial_intent"
                ),
            },
            "translation_posture": (
                "german_source_first_russian_comparator_english_unreviewed_candidate"
            ),
        },
        "response_contract": {
            "schema_ref": str(CANDIDATE_SCHEMA_PATH.relative_to(REPO)),
            "validation_command": (
                f"python {relative_builder} --query {json.dumps(query, ensure_ascii=False)} "
                f"--language {language} --rank {rank} --validate-candidate CANDIDATE.json"
            ),
            "persistence": "none_return_candidate_to_caller",
        },
        "authority": {
            "task_status": "ready_for_on_demand_agent_analysis",
            "content_posture": "local_runtime_exact_source_return_not_for_public_bundle",
            "accepted": False,
            "review_status": "unreviewed",
            "translation_truth_asserted": False,
            "semantic_fact_asserted": False,
            "graph_effect": False,
            "canon_effect": False,
        },
        "provenance": {
            "concept_search_adapter_ref": str(QUERY_PATH.relative_to(REPO)),
            "concept_search_adapter_sha256": sha_file(QUERY_PATH),
            "task_builder_ref": relative_builder,
            "task_builder_sha256": sha_file(Path(__file__).resolve()),
            "task_schema_ref": str(TASK_SCHEMA_PATH.relative_to(REPO)),
            "task_schema_sha256": sha_file(TASK_SCHEMA_PATH),
        },
    }
    Draft202012Validator(load_json(TASK_SCHEMA_PATH), format_checker=FormatChecker()).validate(task)
    return task


def validate_candidate(task: dict[str, Any], candidate: dict[str, Any]) -> dict[str, Any]:
    validator = Draft202012Validator(load_json(CANDIDATE_SCHEMA_PATH), format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(candidate), key=lambda item: list(item.path))
    if errors:
        raise WordAnalysisError("candidate schema violation: " + errors[0].message)
    bindings = {
        "english task": (candidate["english_task_ref"], task["english_on_demand_task_ref"]),
        "request": (candidate["request_ref"], task["request_ref"]),
        "source occurrence": (
            candidate["source_occurrence_ref"], task["source"]["occurrence_candidate_ref"]
        ),
        "source context": (
            candidate["source_context_unit_ref"], task["source"]["context_unit_ref"]
        ),
        "source surface digest": (
            candidate["source_echo_sha256"], task["source"]["surface_sha256"]
        ),
        "source context digest": (
            candidate["source_context_echo_sha256"], task["source"]["context_sha256"]
        ),
    }
    for name, (actual, expected) in bindings.items():
        if actual != expected:
            raise WordAnalysisError(f"{name} does not match prepared task")
    return {
        "schema_version": "tos_zarathustra_word_analysis_validation_receipt_v1",
        "analysis_task_ref": task["analysis_task_id"],
        "candidate_ref": candidate["translation_candidate_id"],
        "valid": True,
        "binding_checks": sorted(bindings),
        "authority": task["authority"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--query", required=True)
    parser.add_argument("--language", choices=("de", "ru", "en"), required=True)
    parser.add_argument("--rank", type=int, default=1)
    parser.add_argument("--include-semantic-neighbors", action="store_true")
    parser.add_argument("--request", type=Path, default=DEFAULT_REQUEST)
    parser.add_argument("--validate-candidate", type=Path)
    args = parser.parse_args()
    request_path = args.request if args.request.is_absolute() else REPO / args.request
    try:
        task = build_task(
            args.query,
            args.language,
            args.rank,
            args.include_semantic_neighbors,
            request_path,
        )
        output = validate_candidate(task, load_json(args.validate_candidate)) if args.validate_candidate else task
    except (WordAnalysisError, ValidationError, OSError, KeyError, ValueError, json.JSONDecodeError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(output, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

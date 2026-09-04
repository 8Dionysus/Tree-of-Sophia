#!/usr/bin/env python3
"""Resolve a multilingual concept query back to exact German Zarathustra evidence."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sqlite3
import stat
import sys
import unicodedata
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, ValidationError


REPO = Path(__file__).resolve().parents[1]
ROUTE = REPO / "ToS/candidate-intake/zarathustra/concept-workbench-v1"
DEFAULT_REQUEST = ROUTE / "requests/fate.concept-request.v2.json"
PRIVATE_ROOT = (
    REPO
    / "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra"
    / "gold-sets/foundation-pilot-v1/local-content/concept-workbench-v1"
)
RESULT_SCHEMA = ROUTE / "concept-search-result.v1.schema.json"
BUILDER_PATH = REPO / "scripts/build_zarathustra_concept_workbench_v1.py"


class SearchError(RuntimeError):
    pass


def digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def load_module(path: Path, name: str) -> Any:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise SearchError(f"cannot load module: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise SearchError(f"JSON object required: {path}")
    return value


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def sha_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def english_key(value: str) -> str:
    return " ".join(unicodedata.normalize("NFKC", value).casefold().split())


def normalize(value: str, language: str, lexical: Any) -> str:
    if language == "ru":
        return lexical.ru_key(value)
    if language == "de":
        return lexical.base_key(value)
    return english_key(value)


def alias_profiles(request: dict[str, Any], lexical: Any) -> list[dict[str, str]]:
    rank = {"preferred_label": 0, "direct_lexical_seed": 1, "semantic_neighbor_seed": 2}
    rows: list[dict[str, str]] = []
    for language, display in request["labels"].items():
        rows.append({"language": language, "display": display, "normalized": normalize(display, language, lexical),
                     "role": "preferred_label", "tier": "direct"})
    for language in ("de", "ru"):
        for display in request["lexical_seeds"][language]:
            rows.append({"language": language, "display": display,
                         "normalized": normalize(display, language, lexical),
                         "role": "direct_lexical_seed", "tier": "direct"})
        for display in request["semantic_neighbor_seeds"][language]:
            rows.append({"language": language, "display": display,
                         "normalized": normalize(display, language, lexical),
                         "role": "semantic_neighbor_seed", "tier": "semantic_neighbor"})
    strongest: dict[tuple[str, str, str], dict[str, str]] = {}
    for row in rows:
        key = (row["language"], row["normalized"], row["tier"])
        if key not in strongest or rank[row["role"]] < rank[strongest[key]["role"]]:
            strongest[key] = row
    return sorted(strongest.values(), key=lambda row: (rank[row["role"]], row["language"], row["normalized"]))


def resolve_query(query: str, language: str, aliases: list[dict[str, str]],
                  lexical: Any, morphology: Any) -> dict[str, str]:
    normalized = normalize(query, language, lexical)
    candidates: list[tuple[int, dict[str, str], str]] = []
    for alias in aliases:
        if alias["language"] != language:
            continue
        if normalized == alias["normalized"]:
            candidates.append((0 if alias["tier"] == "direct" else 2, alias, "exact_alias"))
            continue
        if language in {"de", "ru"}:
            query_signatures = {row[0] for row in morphology.signatures(normalized, language)}
            alias_signatures = {row[0] for row in morphology.signatures(alias["normalized"], language)}
            if query_signatures & alias_signatures:
                candidates.append((1 if alias["tier"] == "direct" else 3,
                                   alias, "morphology_alias_candidate"))
    if not candidates:
        raise SearchError(f"no concept-search route for {language} query {query!r}")
    candidates.sort(key=lambda row: (row[0], row[1]["normalized"], row[1]["display"]))
    _rank, alias, method = candidates[0]
    return {
        "input": query,
        "language": language,
        "normalized": normalized,
        "matched_alias": alias["display"],
        "matched_alias_language": alias["language"],
        "matched_alias_role": alias["role"],
        "match_method": method,
        "resolution_tier": alias["tier"],
        "resolution_status": "proposed" if method == "exact_alias" and alias["tier"] == "direct" else "ambiguous",
    }


def require_private_file(path: Path) -> None:
    if path.is_symlink() or not path.is_file():
        raise SearchError(f"private source-return artifact must be a regular non-symlink: {path}")
    if stat.S_IMODE(path.stat().st_mode) != 0o600:
        raise SearchError(f"private source-return artifact must be mode 0600: {path}")


def request_paths(builder: Any, request_path: Path) -> tuple[Path, Path]:
    builder.configure_request(request_path.relative_to(REPO) if request_path.is_relative_to(REPO) else request_path)
    return REPO / builder.PRIVATE_REQUEST, REPO / builder.PRIVATE_DB


def source_rows(database_path: Path, occurrence_refs: list[str]) -> tuple[dict[str, dict[str, Any]],
                                                                          dict[str, dict[str, Any]],
                                                                          dict[str, dict[str, list[str]]]]:
    uri = f"file:{database_path}?mode=ro&immutable=1"
    database = sqlite3.connect(uri, uri=True)
    database.row_factory = sqlite3.Row
    placeholders = ",".join("?" for _ in occurrence_refs)
    exact = {
        row["existing_occurrence_ref"]: dict(row)
        for row in database.execute(
            f"SELECT * FROM exact_occurrences WHERE existing_occurrence_ref IN ({placeholders})",
            occurrence_refs,
        )
    }
    contexts: dict[str, dict[str, Any]] = {}
    alignments: dict[str, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    for raw in database.execute("SELECT * FROM context_units ORDER BY language,witness_order"):
        row = dict(raw)
        row["alignment_links"] = json.loads(row.pop("alignment_links_json"))
        row.pop("analysis_tokens_json")
        contexts[row["context_unit_ref"]] = row
        for link in row["alignment_links"]:
            alignments[link["alignment_ref"]][row["language"]].append(row["context_unit_ref"])
    database.close()
    missing = sorted(set(occurrence_refs) - set(exact))
    if missing:
        raise SearchError(f"private source return missing {len(missing)} selected occurrences")
    return exact, contexts, alignments


def build_result(query: str, language: str, request_path: Path,
                 include_semantic_neighbors: bool, limit: int) -> dict[str, Any]:
    if limit < 0:
        raise SearchError("--limit must be zero or greater")
    builder = load_module(BUILDER_PATH, "tos_zarathustra_concept_workbench_query_builder")
    lexical = builder.import_builder(builder.LEXICAL_BUILDER, "tos_zarathustra_concept_search_lexical")
    morphology = builder.import_builder(builder.MORPH_BUILDER, "tos_zarathustra_concept_search_morphology")
    request = load_json(request_path)
    Draft202012Validator(load_json(REPO / builder.SCHEMA_REF)).validate(request)
    private_request_path, database_path = request_paths(builder, request_path)
    require_private_file(private_request_path)
    require_private_file(database_path)

    aliases = alias_profiles(request, lexical)
    query_analysis = resolve_query(query, language, aliases, lexical, morphology)
    include_semantic = include_semantic_neighbors or query_analysis["resolution_tier"] == "semantic_neighbor"

    concept_path = REPO / builder.OUTPUTS["concept"]
    occurrences_path = REPO / builder.OUTPUTS["occurrences"]
    relations_path = REPO / builder.OUTPUTS["relations"]
    tasks_path = REPO / builder.OUTPUTS["english_tasks"]
    public_contexts_path = REPO / builder.OUTPUTS["contexts"]
    manifest_path = REPO / builder.OUTPUTS["manifest"]
    required_tracked = (concept_path, occurrences_path, relations_path, tasks_path, public_contexts_path)
    for path in (*required_tracked, manifest_path):
        if not path.is_file():
            raise SearchError(f"workbench artifact is missing; build the request first: {path}")

    manifest = load_json(manifest_path)
    if manifest.get("concept_search_query_sha256") != sha_file(Path(__file__).resolve()):
        raise SearchError("concept-search adapter drift from workbench manifest")
    if manifest.get("concept_search_result_schema_sha256") != sha_file(RESULT_SCHEMA):
        raise SearchError("concept-search result schema drift from workbench manifest")
    tracked_fixity = {REPO / row["ref"]: row["sha256"] for row in manifest["artifacts"]}
    private_fixity = {REPO / row["ref"]: row["sha256"] for row in manifest["private_artifacts"]}
    for path in required_tracked:
        if tracked_fixity.get(path) != sha_file(path):
            raise SearchError(f"tracked workbench artifact fixity mismatch: {path}")
    for path in (private_request_path, database_path):
        if private_fixity.get(path) != sha_file(path):
            raise SearchError(f"private source-return artifact fixity mismatch: {path}")

    concept = load_json(concept_path)
    occurrences = load_jsonl(occurrences_path)
    relations = load_jsonl(relations_path)
    english_tasks = load_jsonl(tasks_path)
    public_contexts = {row["context_unit_ref"]: row for row in load_jsonl(public_contexts_path)}
    private_request = load_json(private_request_path)
    if private_request["concept_candidate_ref"] != concept["concept_candidate_id"]:
        raise SearchError("private analysis and tracked concept candidate disagree")

    source = [row for row in occurrences if row["language"] == "de"]
    if not include_semantic:
        source = [row for row in source if row["evidence_tier"] == "direct_or_morphological"]
    source.sort(key=lambda row: (row["part"], row["witness_order"], row["token_ordinal"],
                                 row["occurrence_candidate_id"]))
    all_occurrence_refs = [row["existing_occurrence_ref"] for row in occurrences]
    exact, contexts, alignment_members = source_rows(database_path, all_occurrence_refs)

    source_realizations: dict[str, dict[str, Any]] = {}
    translation_relations: dict[str, list[str]] = defaultdict(list)
    realization_types = {"lexical_realization", "morphological_realization", "semantic_neighbor_candidate"}
    for relation in relations:
        if relation["relation_type"] in realization_types and concept["concept_candidate_id"] in relation["object_refs"]:
            for ref in relation["subject_refs"]:
                source_realizations[ref] = relation
        if relation["relation_type"] == "translation_parallel_candidate":
            for ref in relation["subject_refs"] + relation["object_refs"]:
                translation_relations[ref].append(relation["relation_candidate_id"])
    english_by_source = {row["source_occurrence_ref"]: row["english_task_id"] for row in english_tasks}
    occurrences_by_context: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in occurrences:
        if row["context_unit_ref"]:
            occurrences_by_context[row["context_unit_ref"]].append(row)
    form_seed = {
        (row["language"], row["analysis_key_sha256"], row["selection_kind"]): row["seed_display"]
        for row in private_request["selected_forms"]
    }

    route_id = "tos.navigation.concept-search-route.sid-" + digest(
        "zarathustra-concept-search-route\n" + request["request_identity_key"]
    )[:32]
    result_id = "tos.navigation.concept-search-result.sid-" + digest(
        f"{route_id}\n{language}\n{query_analysis['normalized']}\nsemantic={include_semantic}\nlimit={limit}"
    )[:32]
    rows = []
    selected_source = source[:limit]
    for rank, occurrence in enumerate(selected_source, 1):
        occurrence_ref = occurrence["occurrence_candidate_id"]
        exact_row = exact[occurrence["existing_occurrence_ref"]]
        context = contexts[occurrence["context_unit_ref"]]
        relation = source_realizations.get(occurrence_ref)
        if relation is None:
            raise SearchError(f"source occurrence has no concept-realization path: {occurrence_ref}")
        alignment_candidates = context["alignment_links"]
        russian_comparators = []
        for context_ref in sorted({ref for link in alignment_candidates
                                   for ref in alignment_members[link["alignment_ref"]]["ru"]}):
            russian = contexts[context_ref]
            selected_ru = occurrences_by_context.get(context_ref, [])
            russian_comparators.append({
                "context_unit_ref": context_ref,
                "exact_text": russian["exact_text"],
                "selected_occurrence_refs": [row["occurrence_candidate_id"] for row in selected_ru],
                "selected_surfaces": [exact[row["existing_occurrence_ref"]]["exact_form"] for row in selected_ru],
                "role": "historical_translation_comparator_not_source_authority",
            })
        seed = form_seed[("de", occurrence["analysis_key_sha256"], occurrence["selection_kind"])]
        rows.append({
            "rank": rank,
            "source_language": "de",
            "evidence_tier": occurrence["evidence_tier"],
            "selection_kind": occurrence["selection_kind"],
            "part": occurrence["part"],
            "reading_ref": occurrence["reading_ref"],
            "unit_kind": occurrence["unit_kind"],
            "witness_order": occurrence["witness_order"],
            "token_ordinal": occurrence["token_ordinal"],
            "occurrence_ordinal_within_context": occurrence["occurrence_ordinal_within_context"],
            "source_occurrence_candidate_ref": occurrence_ref,
            "source_existing_occurrence_ref": occurrence["existing_occurrence_ref"],
            "source_context_unit_ref": occurrence["context_unit_ref"],
            "source_surface": exact_row["exact_form"],
            "source_analysis_form": exact_row["analysis_key"],
            "source_headword_candidate": seed,
            "source_headword_status": "request_seed_not_accepted_lemma",
            "source_context": context["exact_text"],
            "anchor_refs": public_contexts[occurrence["context_unit_ref"]]["anchor_refs"],
            "speaker": {"role": context["speaker_role"], "status": context["speaker_status"]},
            "alignment_candidates": alignment_candidates,
            "russian_comparators": russian_comparators,
            "realization_relation_candidate_ref": relation["relation_candidate_id"],
            "translation_parallel_candidate_refs": sorted(translation_relations.get(occurrence_ref, [])),
            "english_on_demand_task_ref": english_by_source.get(occurrence_ref),
            "query_to_source_path": [
                {"step": "query_alias_match", "from_ref": f"query:{language}:{query_analysis['normalized']}",
                 "to_ref": route_id, "status": query_analysis["resolution_status"]},
                {"step": "routes_to_concept_candidate", "from_ref": route_id,
                 "to_ref": concept["concept_candidate_id"], "status": "navigation_only"},
                {"step": "candidate_realization", "from_ref": concept["concept_candidate_id"],
                 "to_ref": occurrence_ref,
                 "status": f"reverse_navigation_over_{relation['status']}_{relation['relation_type']}"},
                {"step": "source_return", "from_ref": occurrence_ref,
                 "to_ref": occurrence["existing_occurrence_ref"], "status": "exact_witness_return"},
            ],
            "accepted": False,
            "review_status": "unreviewed",
            "semantic_fact_asserted": False,
            "translation_truth_asserted": False,
            "graph_effect": False,
            "canon_effect": False,
        })

    counts = Counter(row["evidence_tier"] for row in source)
    result = {
        "schema_version": "tos_zarathustra_concept_search_result_v1",
        "search_result_id": result_id,
        "query_analysis": query_analysis,
        "concept_search_route": {
            "route_id": route_id,
            "identity_basis_ref": request["request_identity_key"],
            "identity_posture": "stable_navigation_identity_not_semantic_concept_identity",
            "labels": request["labels"],
            "aliases": aliases,
            "current_request_ref": str(request_path.relative_to(REPO) if request_path.is_relative_to(REPO) else request_path),
            "current_request_id": concept["request_id"],
            "accepted_concept_ref": concept["concept_id"],
        },
        "concept_candidate_ref": concept["concept_candidate_id"],
        "work_ref": request["scope"]["work_ref"],
        "content_posture": "local_runtime_exact_source_return_not_tracked",
        "authority_boundary": "navigation_to_candidate_evidence_only_no_semantic_translation_graph_or_canon_acceptance",
        "provenance": {
            "source_manifest_ref": str(manifest_path.relative_to(REPO)),
            "source_manifest_sha256": sha_file(manifest_path),
            "query_adapter_ref": str(Path(__file__).resolve().relative_to(REPO)),
            "query_adapter_sha256": sha_file(Path(__file__).resolve()),
            "result_schema_ref": str(RESULT_SCHEMA.relative_to(REPO)),
            "result_schema_sha256": sha_file(RESULT_SCHEMA),
        },
        "coverage": {
            "total_source_results": len(source),
            "returned_source_results": len(rows),
            "source_evidence_tier_counts": dict(sorted(counts.items())),
            "semantic_neighbors_included": include_semantic,
            "original_language": "de",
            "russian_query_is_source_authority": False,
        },
        "results": rows,
    }
    Draft202012Validator(load_json(RESULT_SCHEMA)).validate(result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--query", required=True)
    parser.add_argument("--language", choices=("de", "ru", "en"), required=True)
    parser.add_argument("--request", type=Path, default=DEFAULT_REQUEST)
    parser.add_argument("--include-semantic-neighbors", action="store_true")
    parser.add_argument("--limit", type=int, default=20,
                        help="number of exact German result cards; zero returns coverage only")
    args = parser.parse_args()
    request_path = args.request if args.request.is_absolute() else REPO / args.request
    try:
        result = build_result(args.query, args.language, request_path,
                              args.include_semantic_neighbors, args.limit)
    except (SearchError, OSError, KeyError, ValueError, sqlite3.Error,
            json.JSONDecodeError, ValidationError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

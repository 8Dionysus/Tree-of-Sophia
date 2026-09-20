#!/usr/bin/env python3
"""Return concept hits through the source-bound discourse and recurrence layer.

The predecessor concept query is loaded from this software installation.
Explicit roots select data only. Its own manifest checks remain in force. This adapter adds reading
evidence; it never turns a paragraph speaker into a word's speaker.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import sqlite3
import stat
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, ValidationError

REPO = Path(__file__).resolve().parents[1]
ROUTE_REF = Path("ToS/candidate-intake/zarathustra/reading-workbench-v1")
MANIFEST_REF = ROUTE_REF / "manifest.v1.json"
SCHEMA_REF = ROUTE_REF / "reading-search-result.v1.schema.json"
BASELINE_QUERY_REF = Path("scripts/query_zarathustra_concept_workbench_v1.py")
DEFAULT_REQUEST_REF = Path("ToS/candidate-intake/zarathustra/concept-workbench-v1/requests/fate.concept-request.v2.json")


class ReadingError(RuntimeError):
    pass


class ReadingUnavailable(ReadingError):
    """Source-bearing material was intentionally not installed here."""


def sha_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def sha_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ReadingError(f"JSON object required: {path}")
    return value


def owned_path(root: Path, ref: str | Path) -> Path:
    relative = Path(ref)
    if relative.is_absolute() or ".." in relative.parts:
        raise ReadingError("reading artifact ref must be root-relative")
    candidate = root / relative
    if candidate.is_symlink() or not candidate.resolve().is_relative_to(root.resolve()):
        raise ReadingError("reading artifact escapes its configured root")
    return candidate


def load_baseline() -> Any:
    path = owned_path(REPO, BASELINE_QUERY_REF)
    if not path.is_file():
        raise ReadingUnavailable("source-bound concept query is not installed")
    name = "tos_reading_baseline_" + sha_text(str(path))[:16]
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ReadingError("cannot load the source-bound concept query")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def open_reading_database(analysis_root: Path, source_root: Path | None = None) -> tuple[sqlite3.Connection, dict[str, Any]]:
    source_root = source_root or analysis_root
    manifest_path = owned_path(analysis_root, MANIFEST_REF)
    if not manifest_path.is_file():
        raise ReadingUnavailable("reading workbench manifest is not installed")
    manifest = load_json(manifest_path)
    if manifest.get("schema_version") != "tos_zarathustra_reading_manifest_v1":
        raise ReadingError("unsupported reading workbench manifest")
    if (any(manifest.get(field) is not False for field in ("accepted", "human_review", "canon_effect"))
            or manifest.get("publication_posture") != "excluded_from_public_bundle"):
        raise ReadingError("reading workbench exceeds its candidate authority boundary")
    descriptor = manifest["private_database"]
    path = owned_path(analysis_root, descriptor["ref"])
    if not path.is_file():
        raise ReadingUnavailable("private reading workbench is not installed")
    if stat.S_IMODE(path.stat().st_mode) != 0o600:
        raise ReadingError("private reading workbench must be mode 0600")
    if sha_file(path) != descriptor["sha256"]:
        raise ReadingError("reading workbench database fixity mismatch")
    # Implementation hashes describe the build. Data fixes and schema version
    # constrain reading; upgrading compatible software does not mutate a snapshot.
    for artifact in manifest.get("artifacts", []):
        source = owned_path(analysis_root, artifact["ref"])
        if not source.is_file() or sha_file(source) != artifact["sha256"]:
            raise ReadingError(f"reading workbench companion drift: {artifact['ref']}")
    for source in manifest.get("inputs", []):
        # Source inputs are fixed under the explicitly selected source dataset.
        ref = source.get("manifest_ref", source["ref"])
        if source.get("manifest_ref") or source.get("role") == "source_visible_voice_policy":
            path_to_check = owned_path(source_root, ref)
            expected = source.get("manifest_sha256", source["sha256"])
            if not path_to_check.is_file() or sha_file(path_to_check) != expected:
                raise ReadingError(f"reading workbench input drift: {ref}")
    database = sqlite3.connect(f"{path.as_uri()}?mode=ro&immutable=1", uri=True)
    database.row_factory = sqlite3.Row
    return database, {
        "manifest_ref": MANIFEST_REF.as_posix(),
        "manifest_sha256": sha_file(manifest_path),
        "private_database_ref": descriptor["ref"],
        "private_database_sha256": descriptor["sha256"],
    }


def rows(database: sqlite3.Connection, table: str, context_ref: str) -> list[dict[str, Any]]:
    # Table names are constant call sites, never request input.
    return [dict(row) for row in database.execute(
        f"SELECT * FROM {table} WHERE context_unit_ref=? ORDER BY start_offset,end_offset",
        (context_ref,),
    )]


def checked_anchor(row: dict[str, Any], context: str) -> dict[str, Any]:
    start, end = row["start_offset"], row["end_offset"]
    if not isinstance(start, int) or not isinstance(end, int) or not 0 <= start < end <= len(context):
        raise ReadingError("invalid source-local reading anchor")
    exact = context[start:end]
    if exact != row["exact_text"] or sha_text(exact) != row["exact_sha256"]:
        raise ReadingError("reading evidence is not an exact source substring")
    return {
        "context_unit_ref": row["context_unit_ref"],
        "start_offset": start,
        "end_offset": end,
        "offset_unit": "unicode_codepoint",
        "offset_scope": "context_local_half_open",
        "exact_text": exact,
        "exact_sha256": row["exact_sha256"],
    }


def contains(outer: dict[str, Any], inner: dict[str, Any]) -> bool:
    return outer["start_offset"] <= inner["start_offset"] and outer["end_offset"] >= inner["end_offset"]


def overlaps(left: dict[str, Any], right: dict[str, Any]) -> bool:
    return left["start_offset"] < right["end_offset"] and left["end_offset"] > right["start_offset"]


def decode_list(row: dict[str, Any], field: str) -> list[Any]:
    value = json.loads(row.get(field, "[]"))
    if not isinstance(value, list):
        raise ReadingError(f"array required in {field}")
    return value


def alignment_index(database: sqlite3.Connection) -> dict[str, list[dict[str, Any]]]:
    index: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for raw in database.execute("SELECT * FROM translation_alignments ORDER BY alignment_id"):
        row = dict(raw)
        if row["semantic_equivalence_asserted"] or row["human_acceptance"]:
            raise ReadingError("candidate alignment unexpectedly asserts acceptance")
        packet = {
            "alignment_id": row["alignment_id"],
            "granularity": row["granularity"],
            "candidate_role": row["candidate_role"],
            "status": row["status"],
            "shape": row["correspondence_shape"],
            "source_unit_refs": decode_list(row, "ordered_source_unit_refs_json"),
            "target_unit_refs": decode_list(row, "ordered_target_unit_refs_json"),
            "exact_source_text": row["exact_source_text"],
            "exact_target_text": row["exact_target_text"],
            "parent_paragraph_alignment_ref": row["parent_paragraph_alignment_ref"],
            "reason_codes": decode_list(row, "reason_codes_json"),
            "competing_alignment_refs": decode_list(row, "competing_alignment_refs_json"),
            "word_correspondence_asserted": False,
            "translation_truth_asserted": False,
        }
        for unit_ref in packet["source_unit_refs"]:
            index[unit_ref].append(packet)
    return index


def enrich_card(database: sqlite3.Connection, baseline: dict[str, Any],
                alignments: dict[str, list[dict[str, Any]]], *,
                explicit_span_rows: list[dict[str, Any]] | None = None) -> dict[str, Any]:
    card = dict(baseline)
    context_ref, context = card["source_context_unit_ref"], card["source_context"]
    span_rows = explicit_span_rows if explicit_span_rows is not None else [dict(row) for row in database.execute(
        "SELECT * FROM occurrence_spans WHERE existing_occurrence_ref=? ORDER BY start_offset,end_offset",
        (card["source_existing_occurrence_ref"],),
    )]
    spans = []
    for row in span_rows:
        if row["context_unit_ref"] != context_ref or row["exact_text"] != card["source_surface"]:
            raise ReadingError("occurrence crosswalk binds the wrong source text")
        spans.append({**checked_anchor(row, context), "surface_unit_ref": row["surface_unit_ref"],
                      "status": row["status"]})
    span = spans[0] if len(spans) == 1 else None
    card["source_occurrence_spans"] = spans
    card["source_occurrence_anchor_status"] = "exact" if span else "ambiguous" if spans else "deferred"
    card["speaker_predecessor"] = {
        **card["speaker"], "scope": "paragraph_candidate_not_occurrence_attribution",
        "layer": "concept-workbench-v1",
    }
    discourse = []
    for row in rows(database, "discourse_segments", context_ref):
        if span is not None and overlaps(row, span):
            discourse.append({
                "segment_id": row["segment_id"],
                "sentence_unit_ref": row["sentence_unit_ref"],
                "source_anchor": checked_anchor(row, context),
                "role": row["speaker_role"], "status": row["speaker_status"],
                "candidates": decode_list(row, "speaker_candidates_json"),
                "evidence_refs": decode_list(row, "evidence_refs_json"),
                "kind": row["kind"], "quote_depth": row["quote_depth"],
                "speech_turn_id": row.get("speech_turn_id"),
                "utterer_role": row.get("utterer_role"),
                "performed_role": row.get("performed_role"),
                "modality": row.get("modality"),
                "attribution_basis": row.get("attribution_basis"),
                "contains_occurrence": contains(row, span),
            })
    selected = [row for row in discourse if row["contains_occurrence"]]
    if len(selected) == 1:
        candidate = selected[0]
        speaker = {key: candidate[key] for key in ("role", "status", "candidates", "evidence_refs")}
        speaker.update({"segment_id": candidate["segment_id"], "scope": "occurrence_containing_segment",
                        "utterer_role": candidate["utterer_role"], "performed_role": candidate["performed_role"],
                        "modality": candidate["modality"], "attribution_basis": candidate["attribution_basis"]})
    else:
        speaker = {"role": "unresolved", "status": "ambiguous" if discourse or spans else "deferred",
                   "scope": "occurrence_not_resolved", "segment_id": None,
                   "candidates": sorted({row["role"] for row in discourse}), "evidence_refs": [],
                   "utterer_role": None, "performed_role": None, "modality": None,
                   "attribution_basis": "no_unique_containing_segment"}
    card["speaker"] = speaker
    card["discourse_segments"] = discourse
    unit_refs: list[str] = []
    for table, id_field, output in (("source_sentences", "sentence_unit_ref", "source_sentences"),
                                    ("source_clauses", "clause_id", "source_clauses")):
        units = []
        for row in rows(database, table, context_ref):
            if span is not None and overlaps(row, span):
                unit_refs.append(row[id_field])
                units.append({"unit_ref": row[id_field], "source_anchor": checked_anchor(row, context),
                              "contains_occurrence": contains(row, span),
                              "status": row.get("status", "proposed")})
        card[output] = units
    fine = {row["alignment_id"]: row for ref in unit_refs for row in alignments.get(ref, [])}
    card["fine_alignment_candidates"] = list(fine.values())
    card["alignment_granularity_note"] = (
        "sentence_and_clause_candidates_not_word_alignment" if fine
        else "paragraph_comparator_only_or_explicit_alignment_gap"
    )
    formulas = []
    nearby_formulas = []
    for row in database.execute(
        "SELECT fo.*,f.normalized_text,f.token_count,f.occurrence_count,f.reading_count "
        "FROM formula_occurrences fo JOIN formulas f USING(formula_id) "
        "WHERE fo.context_unit_ref=? ORDER BY fo.start_offset,fo.formula_id,fo.occurrence_id",
        (context_ref,),
    ):
        item = dict(row)
        packet = {key: item[key] for key in ("formula_id", "occurrence_id", "normalized_text", "token_count",
                                            "occurrence_count", "reading_count", "status")}
        packet["source_anchor"] = checked_anchor(item, context)
        packet["relation"] = "contains_occurrence" if span is not None and contains(item, span) else "same_context_only"
        (formulas if packet["relation"] == "contains_occurrence" else nearby_formulas).append(packet)
    card["formula_memberships"] = formulas
    card["context_formula_memberships"] = nearby_formulas
    card["english_analysis_context"] = {
        "task_ref": card["english_on_demand_task_ref"],
        "execution_status": "not_executed",
        "source_language": "de",
        "source_sentence_refs": [row["unit_ref"] for row in card["source_sentences"]],
        "speaker_segment_ref": speaker["segment_id"],
        "formula_refs": sorted({row["formula_id"] for row in formulas}),
        "historical_etymology_requires_cited_evidence": True,
        "english_is_generated_candidate_not_witness": True,
    }
    return card


def explicit_dehyphenations(text: str, selected_forms: set[str]) -> list[dict[str, Any]]:
    """Only the witness's explicit line-break mark is removed, not arbitrary dashes."""
    candidates = []
    for match in re.finditer(r"(?<!\w)([^\W\d_]+)(¬[ \t]*\r?\n[ \t]*)([^\W\d_]+)(?!\w)", text):
        normalized = match.group(1) + match.group(3)
        if normalized.casefold() in selected_forms:
            candidates.append({
                "start_offset": match.start(), "end_offset": match.end(),
                "exact_text": match.group(), "exact_sha256": sha_text(match.group()),
                "normalized_form": normalized,
                "normalization_operations": [{
                    "operation": "remove_explicit_linebreak_hyphen_mark",
                    "start_offset": match.start(2), "end_offset": match.end(2),
                    "exact_removed_text": match.group(2), "offset_scope": "context_local_half_open",
                }],
            })
    return candidates


def additional_normalization_candidates(database: sqlite3.Connection, cards: list[dict[str, Any]],
                                        alignments: dict[str, list[dict[str, Any]]]) -> list[dict[str, Any]]:
    selected_forms = {str(row[field]).casefold() for row in cards for field in ("source_analysis_form", "source_surface")}
    existing_spans = {(row["source_context_unit_ref"], span["start_offset"], span["end_offset"])
                      for row in cards for span in row["source_occurrence_spans"]}
    candidates = []
    for raw in database.execute("SELECT * FROM contexts WHERE language='de' ORDER BY part,witness_order"):
        context = dict(raw)
        context_ref, text = context["context_unit_ref"], context["exact_text"]
        if sha_text(text) != context["exact_sha256"]:
            raise ReadingError("normalization context hash mismatch")
        for match in explicit_dehyphenations(text, selected_forms):
            if (context_ref, match["start_offset"], match["end_offset"]) in existing_spans:
                continue
            identity = "tos.annotation.normalization-search-candidate.sid-" + sha_text(
                f"explicit-linebreak-v1\n{context_ref}\n{match['start_offset']}\n{match['end_offset']}\n{match['exact_sha256']}"
            )[:32]
            candidate = {
                "candidate_id": identity,
                "source_occurrence_candidate_ref": identity,
                "source_existing_occurrence_ref": None,
                "legacy_occurrence_id_asserted": False,
                "source_context_unit_ref": context_ref,
                "source_language": "de", "source_context": text,
                "source_surface": match["exact_text"], "source_analysis_form": match["normalized_form"],
                "part": context["part"], "reading_ref": context["reading_ref"],
                "witness_order": context["witness_order"], "rank": len(candidates) + 1,
                "evidence_tier": "normalization_candidate",
                "selection_reason": "explicit_linebreak_normalization_matches_selected_german_form",
                "normalization_operations": match["normalization_operations"],
                "speaker": {"role": "not_applicable", "status": "not_applicable"},
                "english_on_demand_task_ref": None, "russian_comparators": [],
                "accepted": False, "semantic_fact_asserted": False,
                "translation_truth_asserted": False, "graph_effect": False, "canon_effect": False,
            }
            span_row = {**match, "context_unit_ref": context_ref, "surface_unit_ref": None, "status": "normalization_candidate"}
            enriched = enrich_card(database, candidate, alignments, explicit_span_rows=[span_row])
            enriched["speaker_predecessor"] = {"role": "not_applicable", "status": "not_applicable",
                                               "scope": "no_legacy_occurrence", "layer": "not_applicable"}
            enriched["english_analysis_context"]["source_normalization_candidate_ref"] = identity
            enriched["english_analysis_context"]["normalization_operations"] = match["normalization_operations"]
            # Only an existing primary sentence candidate is shown as the
            # comparator. Competing/clause candidates remain in the fine ledger.
            for parallel in enriched["fine_alignment_candidates"]:
                if parallel["granularity"] == "sentence" and parallel["candidate_role"] == "primary":
                    enriched["russian_comparators"].append({
                        "alignment_id": parallel["alignment_id"], "exact_text": parallel["exact_target_text"],
                        "target_unit_refs": parallel["target_unit_refs"], "status": parallel["status"],
                        "granularity": "sentence_candidate", "role": "historical_translation_comparator_not_source_authority",
                    })
            candidates.append(enriched)
    return candidates


def group_cards(cards: list[dict[str, Any]], returned: list[dict[str, Any]], group_by: tuple[str, ...]) -> dict[str, Any]:
    returned_refs = {row["source_occurrence_candidate_ref"] for row in returned}
    result: dict[str, Any] = {"scope": "all_matching_source_occurrences_before_limit"}
    if "speaker" in group_by:
        speakers: dict[tuple[str, ...], list[str]] = defaultdict(list)
        for card in cards:
            speaker = card["speaker"]
            key = tuple(speaker.get(field) or "" for field in ("role", "status", "utterer_role", "performed_role", "modality"))
            speakers[key].append(card["source_occurrence_candidate_ref"])
        result["by_speaker"] = [
            {"role": role, "status": status, "matching_count": len(refs),
             "utterer_role": utterer or None, "performed_role": performed or None, "modality": modality or None,
             "returned_occurrence_refs": [ref for ref in refs if ref in returned_refs]}
            for (role, status, utterer, performed, modality), refs in sorted(speakers.items())
        ]
    if "formula" in group_by:
        formulas: dict[str, set[str]] = defaultdict(set)
        descriptions = {}
        for card in cards:
            for item in card["formula_memberships"]:
                formulas[item["formula_id"]].add(card["source_occurrence_candidate_ref"])
                descriptions[item["formula_id"]] = item
        result["by_formula"] = [
            {"formula_id": formula_id, "normalized_text": descriptions[formula_id]["normalized_text"],
             "matching_count": len(refs), "whole_book_occurrence_count": descriptions[formula_id]["occurrence_count"],
             "returned_occurrence_refs": sorted(refs & returned_refs)}
            for formula_id, refs in sorted(formulas.items())
        ]
        result["no_formula_membership_count"] = sum(not row["formula_memberships"] for row in cards)
        result["formula_group_posture"] = "overlapping_exact_normalized_formulas_not_semantic_equivalence"
    return result


def build_result(query: str, language: str = "ru", *, limit: int = 20,
                 include_semantic_neighbors: bool = False,
                 group_by: tuple[str, ...] = ("speaker", "formula"),
                 source_root: Path | None = None, analysis_root: Path | None = None,
                 request_path: Path | None = None) -> dict[str, Any]:
    query = str(query).strip()
    if not query or len(query) > 256:
        raise ValueError("reading query must have 1..256 characters")
    if language not in {"de", "ru", "en"}:
        raise ValueError("reading language must be de, ru, or en")
    if not isinstance(limit, int) or isinstance(limit, bool) or not 0 <= limit <= 100:
        raise ValueError("reading limit must be 0..100")
    if set(group_by) - {"speaker", "formula"}:
        raise ValueError("reading group_by supports speaker and formula only")
    if source_root is None or analysis_root is None:
        raise ReadingUnavailable("select source and analysis data roots explicitly")
    source_root, analysis_root = Path(source_root).absolute(), Path(analysis_root).absolute()
    if any(root != root.resolve() for root in (source_root, analysis_root)):
        raise ReadingError("reading data roots may not contain symlinks")
    baseline_query = load_baseline()
    request_path = request_path or owned_path(source_root, DEFAULT_REQUEST_REF)
    database, provenance = open_reading_database(analysis_root, source_root)
    try:
        try:
            baseline = baseline_query.build_result(query, language, request_path,
                                                   include_semantic_neighbors, sys.maxsize, data_root=source_root)
        except RuntimeError as exc:
            if str(exc).startswith("private source-return artifact must be a regular non-symlink:"):
                raise ReadingUnavailable("private concept source-return artifacts are not installed") from exc
            raise
        metadata = dict(database.execute("SELECT key,value FROM metadata"))
        # The baseline already verifies its private artifact bytes; bind those
        # checked bytes to the reading build rather than trusting either root.
        baseline_manifest = load_json(owned_path(source_root, baseline["provenance"]["source_manifest_ref"]))
        db_hashes = [row["sha256"] for row in baseline_manifest["private_artifacts"]
                     if row["ref"].endswith(".sqlite3")]
        if metadata.get("concept_workbench_sha256") not in db_hashes:
            raise ReadingError("reading workbench is stale relative to the selected concept source")
        alignments = alignment_index(database)
        cards = [enrich_card(database, row, alignments) for row in baseline["results"]]
        additions = additional_normalization_candidates(database, cards, alignments)
    finally:
        database.close()
    if len(cards) != baseline["coverage"]["total_source_results"]:
        raise ReadingError("predecessor did not return all matches for pre-limit grouping")
    returned = cards[:limit]
    result = {key: value for key, value in baseline.items() if key not in {"schema_version", "provenance", "results", "coverage"}}
    result["schema_version"] = "tos_zarathustra_reading_search_result_v1"
    result["reading_search_result_id"] = "tos.navigation.reading-search-result.sid-" + sha_text(
        baseline["search_result_id"] + provenance["private_database_sha256"] + str(limit) + ",".join(group_by)
    )[:32]
    result["provenance"] = {
        "concept_predecessor": baseline["provenance"], "reading_layer": provenance,
        "adapter_ref": "scripts/query_zarathustra_reading_workbench_v1.py",
        "adapter_sha256": sha_file(Path(__file__)),
        "result_schema_ref": SCHEMA_REF.as_posix(),
        "result_schema_sha256": sha_file(REPO / SCHEMA_REF),
        "method_version": metadata.get("method_version"),
    }
    result["coverage"] = {
        **baseline["coverage"], "returned_source_results": len(returned), "limit": limit,
        "grouping_scope": "all_matching_source_occurrences_before_limit",
        "speaker_status_counts": dict(sorted(Counter(row["speaker"]["status"] for row in cards).items())),
        "occurrence_anchor_status_counts": dict(sorted(Counter(row["source_occurrence_anchor_status"] for row in cards).items())),
        "occurrences_with_formula_membership": sum(bool(row["formula_memberships"]) for row in cards),
        "additional_source_candidate_count": len(additions),
        "returned_additional_source_candidates": len(additions[:limit]),
        "primary_result_scope": "verified_predecessor_concept_request_occurrences",
        "grouping_excludes_additional_candidates": True,
        "whole_book_semantic_recall_asserted": False,
    }
    result["groups"] = group_cards(cards, returned, group_by)
    result["results"] = returned
    result["additional_source_candidates"] = additions[:limit]
    result["limitations"] = [
        "Speaker attribution and sentence/clause alignment remain reviewable candidates, not accepted interpretation.",
        "Morphology and dependencies are inherited heuristic candidates; this layer does not repair full contextual syntax.",
        "Counts cover the selected concept request, not every possible semantic mention of a concept.",
        "Additional explicit line-break normalization candidates have separate IDs/counts and are not hidden in predecessor result groups.",
        "Formula membership is exact after declared normalization; nearby formulas are not membership.",
        "English translation and etymological research are on-demand agent work and have not been executed by this query.",
    ]
    Draft202012Validator(load_json(REPO / SCHEMA_REF)).validate(result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--query", required=True)
    parser.add_argument("--language", choices=("de", "ru", "en"), default="ru")
    parser.add_argument("--limit", type=int, default=20)
    parser.add_argument("--group-by", default="speaker,formula")
    parser.add_argument("--include-semantic-neighbors", action="store_true")
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--analysis-root", type=Path, required=True)
    parser.add_argument("--request", type=Path)
    args = parser.parse_args()
    try:
        result = build_result(args.query, args.language, limit=args.limit,
                              include_semantic_neighbors=args.include_semantic_neighbors,
                              group_by=tuple(item for item in args.group_by.split(",") if item),
                              source_root=args.source_root, analysis_root=args.analysis_root,
                              request_path=args.request)
    except (RuntimeError, OSError, ValueError, KeyError, sqlite3.Error, ValidationError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Build the reusable, candidate-only Zarathustra concept workbench."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import sqlite3
import stat
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft202012Validator, ValidationError


REPO = Path(__file__).resolve().parents[1]
WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")
ROUTE = Path("ToS/candidate-intake/zarathustra/concept-workbench-v1")
ALIGN = WORK / "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1"
DE_TECH = WORK / "technical-markup/dta-first-editions-parts-1-4-v1"
RU_TECH = WORK / "technical-markup/antonovsky-1911-structural-paragraph-v2"
DE_OCC_DB = WORK / "gold-sets/foundation-pilot-v1/local-content/lexical-search/zarathustra-dta-first-editions-parts-1-4-v1.sqlite3"
PRIVATE_ROOT = WORK / "gold-sets/foundation-pilot-v1/local-content/concept-workbench-v1"
PRIVATE_DB = PRIVATE_ROOT / "workbench-index.v1.sqlite3"
PRIVATE_REQUEST = PRIVATE_ROOT / "requests/fate.request-analysis.v1.json"
PLAN_REF = ROUTE / "plan.v1.json"
SCHEMA_REF = ROUTE / "concept-request.v2.schema.json"
RELATION_SCHEMA_REF = ROUTE / "concept-relation-candidate.v1.schema.json"
ENGLISH_TASK_SCHEMA_REF = ROUTE / "english-on-demand-task.v1.schema.json"
ENGLISH_CANDIDATE_SCHEMA_REF = ROUTE / "english-translation-candidate.v1.schema.json"
SEARCH_RESULT_SCHEMA_REF = ROUTE / "concept-search-result.v1.schema.json"
SEARCH_QUERY_REF = Path("scripts/query_zarathustra_concept_workbench_v1.py")
WORD_ANALYSIS_TASK_SCHEMA_REF = ROUTE / "word-analysis-task.v1.schema.json"
WORD_ANALYSIS_PREPARE_REF = Path("scripts/prepare_zarathustra_word_analysis_v1.py")
REQUEST_REF = ROUTE / "requests/fate.concept-request.v2.json"
ISSUANCE_REF = ROUTE / "identity-issuance.v1.json"
GENERATOR_REF = Path("scripts/build_zarathustra_concept_workbench_v1.py")
LEXICAL_BUILDER = Path("scripts/build_zarathustra_parallel_lexical_candidates_v1.py")
MORPH_BUILDER = Path("scripts/build_zarathustra_morphology_theme_candidates_v1.py")
RU_BUILDER = Path("scripts/build_antonovsky_1911_structural_paragraph_v2.py")

OUTPUTS = {
    "contexts": ROUTE / "context-unit-spine.v1.jsonl",
    "speakers": ROUTE / "speaker-state-candidates.v1.jsonl",
    "all_forms": ROUTE / "all-form-coverage.v1.json",
    "speaker_worklist": ROUTE / "speaker-exception-worklist.v1.json",
    "concept": ROUTE / "outputs/fate/concept-candidate.v1.json",
    "forms": ROUTE / "outputs/fate/form-family-candidates.v1.jsonl",
    "occurrences": ROUTE / "outputs/fate/occurrence-spine.v1.jsonl",
    "relations": ROUTE / "outputs/fate/relation-candidates.v1.jsonl",
    "gaps": ROUTE / "outputs/fate/gap-ledger.v1.jsonl",
    "exclusions": ROUTE / "outputs/fate/exclusion-ledger.v1.jsonl",
    "graph": ROUTE / "outputs/fate/candidate-graph.v1.json",
    "coverage": ROUTE / "outputs/fate/coverage-receipt.v1.json",
    "english_tasks": ROUTE / "outputs/fate/english-on-demand-worklist.v1.jsonl",
    "summary": ROUTE / "summary.v1.json",
    "provenance": ROUTE / "provenance.jsonl",
    "manifest": ROUTE / "manifest.v1.json",
}


def configure_request(request_ref: Path) -> None:
    """Select one request without changing the workbench implementation."""
    global REQUEST_REF, PRIVATE_REQUEST, ISSUANCE_REF, OUTPUTS
    REQUEST_REF = request_ref
    request = load_json(request_ref)
    key = request["request_key"]
    if key == "fate" and request_ref == ROUTE / "requests/fate.concept-request.v2.json":
        PRIVATE_REQUEST = PRIVATE_ROOT / "requests/fate.request-analysis.v1.json"
        ISSUANCE_REF = ROUTE / "identity-issuance.v1.json"
        return
    identity_suffix = request["request_identity_key"].rsplit("-", 1)[-1]
    request_scope = f"{key}-v{request['request_version']}-{identity_suffix}"
    PRIVATE_REQUEST = PRIVATE_ROOT / f"requests/{request_scope}.request-analysis.v1.json"
    base = ROUTE / f"outputs/{request_scope}"
    ISSUANCE_REF = base / "identity-issuance.v1.json"
    OUTPUTS = {**OUTPUTS,
        "concept": base / "concept-candidate.v1.json", "forms": base / "form-family-candidates.v1.jsonl",
        "occurrences": base / "occurrence-spine.v1.jsonl", "relations": base / "relation-candidates.v1.jsonl",
        "gaps": base / "gap-ledger.v1.jsonl", "exclusions": base / "exclusion-ledger.v1.jsonl",
        "graph": base / "candidate-graph.v1.json", "coverage": base / "coverage-receipt.v1.json",
        "english_tasks": base / "english-on-demand-worklist.v1.jsonl",
        "summary": base / "summary.v1.json", "provenance": base / "provenance.jsonl",
        "manifest": base / "manifest.v1.json"}


def request_binding(request: dict[str, Any]) -> str:
    return f"{request['request_identity_key']}|v{request['request_version']}"


def request_local_binding(request: dict[str, Any], binding: str) -> str:
    return f"{request_binding(request)}|{binding}"


def provenance_event_ref(request: dict[str, Any]) -> str:
    return "tos.event.zarathustra-concept-workbench-v1.build.sid-" + h(request_binding(request))[:32]
EXPECTED = {
    "alignments": 3423, "de_paragraphs": 3447, "de_verse": 368,
    "ru_paragraphs": 3569, "ru_verse": 359,
    "de_exact_occurrences": 86287, "ru_exact_occurrences": 93643,
    "de_exact_forms": 11352, "de_analysis_forms": 10113,
    "de_work_scope_occurrences": 84491, "de_work_scope_exact_forms": 11118,
    "de_work_scope_analysis_forms": 9909,
    "ru_exact_forms": 17443, "ru_normalized_forms": 16240,
    "ru_analysis_forms": 15152,
}
DE_OCC_DB_SHA256 = "c2912a9f481205f0de9a1a0242b26a3419e3d82d44163ab91a2a2ab16ced5736"

DE_SPEAKERS = (
    ("zarathustra", r"(?:sprach|antwortete|rief|sagte)\s+zarathustra|zarathustra\s+(?:sprach|antwortete|rief|sagte)"),
    ("animals_eagle_and_serpent", r"(?:sprachen|antworteten|sagten)\s+(?:seine\s+)?thiere"),
    ("dwarf", r"(?:sprach|antwortete|sagte|murmelte)\s+(?:der\s+)?zwerg"),
    ("ugliest_man", r"(?:sprach|antwortete|sagte)\s+(?:der\s+)?hässlichste\s+mensch"),
    ("soothsayer", r"(?:sprach|antwortete|sagte)\s+(?:der\s+)?wahrsager"),
    ("magician", r"(?:sprach|antwortete|sagte)\s+(?:der\s+)?zauberer"),
    ("shadow", r"(?:sprach|antwortete|sagte)\s+(?:der\s+)?schatten"),
    ("kings", r"(?:sprachen|antworteten|sagten)\s+(?:die\s+)?könige"),
    ("people_or_crowd", r"(?:sprach|sprachen|rief|riefen)\s+(?:das\s+)?volk"),
)
RU_SPEAKERS = (
    ("zarathustra", r"(?:сказал|говорил|отвечал|воскликнул)\s+заратустра|заратустра\s+(?:сказал|говорил|отвечал|воскликнул)"),
    ("animals_eagle_and_serpent", r"(?:сказали|говорили|отвечали)\s+(?:его\s+)?животн"),
    ("dwarf", r"(?:сказал|говорил|ответил|пробормотал)\s+(?:карлик|карлика)"),
    ("soothsayer", r"(?:сказал|говорил|ответил)\s+прорицатель"),
    ("magician", r"(?:сказал|говорил|ответил)\s+волшебник"),
    ("shadow", r"(?:сказала|говорила|ответила)\s+тень"),
    ("kings", r"(?:сказали|говорили|ответили)\s+цари"),
    ("people_or_crowd", r"(?:сказал|говорил|кричал)\s+народ"),
)


class BuildError(RuntimeError):
    pass


def jb(value: Any, pretty: bool = True) -> bytes:
    value = json.dumps(value, ensure_ascii=False, sort_keys=True,
                       indent=2 if pretty else None,
                       separators=None if pretty else (",", ":"))
    return (value + "\n").encode()


def jlb(rows: Iterable[dict[str, Any]]) -> bytes:
    return b"".join(jb(row, False) for row in rows)


def h(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def sha_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha_file(path: Path) -> str:
    return sha_bytes(path.read_bytes())


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
    if not (REPO / DE_OCC_DB).is_file():
        raise BuildError("German exact-occurrence database missing")
    database = REPO / DE_OCC_DB
    if database.is_symlink() or stat.S_IMODE(database.stat().st_mode) != 0o600:
        raise BuildError("German exact-occurrence database must be regular, non-symlink, and mode 0600")
    if sha_file(database) != DE_OCC_DB_SHA256:
        raise BuildError("German exact-occurrence database fixity drift")


def source_maps() -> tuple[dict[str, tuple[int, str]], dict[str, tuple[int, str]],
                           dict[str, list[dict[str, str]]], int, Counter[str], Counter[str]]:
    spine = load_jsonl(ALIGN / "alignment-spine.v1.jsonl")
    de_reading: dict[str, tuple[int, str]] = {}
    ru_reading: dict[str, tuple[int, str]] = {}
    for row in spine:
        part = int(row["part_order"])
        reading = (f"p{part}.r{int(row['reading_ordinal_within_part'])}"
                   if row["reading_ordinal_within_part"] is not None
                   else f"p{part}.unscoped-technical")
        for ref in row["source_paragraph_unit_refs"]:
            de_reading[ref] = (part, reading)
        for ref in row["target_paragraph_unit_refs"]:
            ru_reading[ref] = (part, reading)
    for row in load_jsonl(ALIGN / "scope-exclusions.v1.jsonl"):
        part = int(row["part_order"])
        reading = f"p{part}.r{int(row['reading_ordinal_within_part'])}"
        target = de_reading if row["side"] == "source" else ru_reading
        for ref in row["ordered_excluded_verse_line_refs"]:
            target[ref] = (part, reading)
    by_unit: dict[str, list[dict[str, str]]] = defaultdict(list)
    statuses: Counter[str] = Counter()
    shapes: Counter[str] = Counter()
    count = 0
    spine_by_id = {x["alignment_id"]: x for x in spine}
    for part in range(1, 5):
        packet = load_json(ALIGN / f"part-{part}.translation-alignment-packet.v1.json")
        for alignment in packet["alignments"]:
            count += 1
            aid = alignment["alignment_id"]
            link = {"alignment_ref": aid, "status": alignment["status"],
                    "shape": alignment["correspondence_shape"]}
            statuses[link["status"]] += 1
            shapes[link["shape"]] += 1
            row = spine_by_id[aid]
            for ref in row["source_paragraph_unit_refs"] + row["target_paragraph_unit_refs"]:
                by_unit[ref].append(link)
    if count != EXPECTED["alignments"]:
        raise BuildError(f"alignment census drift: {count}")
    return de_reading, ru_reading, by_unit, count, statuses, shapes


def load_de_units(lexical: Any, readings: dict[str, tuple[int, str]],
                  alignments: dict[str, list[dict[str, str]]]) -> list[dict[str, Any]]:
    citations = load_jsonl(DE_TECH / "citation-spine.v1.jsonl")
    packets = {part: load_json(DE_TECH / f"part-{part}.source-text-unit.v1.json") for part in range(1, 5)}
    units = {part: {x["unit_id"]: x for x in packet["units"]} for part, packet in packets.items()}
    anchors = {part: {x["anchor_ref"]: x for x in packet["anchors"]} for part, packet in packets.items()}
    caches: dict[str, str] = {}
    result = []
    filtered = [x for x in citations if x["unit_kind"] in {"paragraph", "verse_line"}]
    for witness_order, citation in enumerate(filtered, 1):
        part = int(citation["part_order"])
        unit = units[part][citation["unit_id"]]
        fragments = []
        for anchor_ref in unit["ordered_anchor_refs"]:
            anchor = anchors[part][anchor_ref]
            ref = anchor["text_layer_ref"]
            path = REPO / ref
            if stat.S_IMODE(path.stat().st_mode) != 0o600:
                raise BuildError(f"private German text layer is not 0600: {ref}")
            if ref not in caches:
                caches[ref] = path.read_text(encoding="utf-8")
                if h(caches[ref]) != anchor["text_layer_sha256"]:
                    raise BuildError(f"German text layer drift: {ref}")
            selector = anchor["selector"]
            text = caches[ref][selector["start"]:selector["end"]]
            if h(text) != anchor["exact_sha256"]:
                raise BuildError(f"German anchor return mismatch: {anchor_ref}")
            fragments.append(text)
        ref = citation["unit_id"]
        if ref not in readings:
            raise BuildError(f"German reading map missing: {ref}")
        mapped_part, reading = readings[ref]
        if mapped_part != part:
            raise BuildError("German reading part mismatch")
        text = "\n".join(fragments)
        result.append({"context_unit_ref": ref, "language": "de", "part": part,
                       "reading_ref": reading, "unit_kind": citation["unit_kind"],
                       "witness_order": witness_order, "text": text, "exact_sha256": h(text),
                       "anchor_refs": unit["ordered_anchor_refs"], "source_locator": citation["source_locator"],
                       "alignment_links": alignments.get(ref, []), "analysis_tokens": lexical.tokens(text, "de")})
    kinds = Counter(x["unit_kind"] for x in result)
    if kinds != {"paragraph": EXPECTED["de_paragraphs"], "verse_line": EXPECTED["de_verse"]}:
        raise BuildError(f"German witness-unit census drift: {dict(kinds)}")
    return result


def load_ru_units(lexical: Any, builder: Any, readings: dict[str, tuple[int, str]],
                  alignments: dict[str, list[dict[str, str]]]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    model = builder.reconstruct()
    ids = builder.load_identities(model)
    row_id = {row.row_ref: ids["logical_rows"][builder.row_binding(row)] for row in model["rows"]}
    row_text = {row_id[row.row_ref]: row.text for row in model["rows"]}
    row_order = {row_id[row.row_ref]: n for n, row in enumerate(model["rows"], 1)}
    logical_owner: dict[str, str] = {}
    result = []
    for record in load_jsonl(RU_TECH / "paragraph-spine.v2.jsonl"):
        ref = record["paragraph_unit_id"]
        for logical in record["ordered_logical_row_refs"]:
            logical_owner[logical] = ref
        part, reading = readings[ref]
        text = "\n".join(row_text[x] for x in record["ordered_logical_row_refs"])
        result.append({"context_unit_ref": ref, "language": "ru", "part": part,
                       "reading_ref": reading, "unit_kind": "paragraph",
                       "witness_order": min(row_order[x] for x in record["ordered_logical_row_refs"]),
                       "text": text, "exact_sha256": h(text), "anchor_refs": record["ordered_logical_row_refs"],
                       "source_locator": record["display_citation"], "alignment_links": alignments.get(ref, []),
                       "analysis_tokens": lexical.tokens(text, "ru")})
    for record in load_jsonl(RU_TECH / "verse-line-spine.v2.jsonl"):
        ref = record["verse_line_unit_id"]
        logical = record["logical_row_unit_ref"]
        logical_owner[logical] = ref
        part, reading = readings[ref]
        text = row_text[logical]
        if h(text) != record["exact_sha256"]:
            raise BuildError(f"Russian verse return mismatch: {ref}")
        result.append({"context_unit_ref": ref, "language": "ru", "part": part,
                       "reading_ref": reading, "unit_kind": "verse_line", "witness_order": row_order[logical],
                       "text": text, "exact_sha256": h(text), "anchor_refs": [logical],
                       "source_locator": logical, "alignment_links": [], "analysis_tokens": lexical.tokens(text, "ru")})
    result.sort(key=lambda x: x["witness_order"])
    kinds = Counter(x["unit_kind"] for x in result)
    if kinds != {"paragraph": EXPECTED["ru_paragraphs"], "verse_line": EXPECTED["ru_verse"]}:
        raise BuildError(f"Russian witness-unit census drift: {dict(kinds)}")
    occurrences, _units, _meta = lexical.build_ru_observations()
    for row in occurrences:
        row["context_unit_ref"] = logical_owner.get(row["unit_id"])
    return result, occurrences


def load_de_occurrences(lexical: Any, units: list[dict[str, Any]]) -> list[dict[str, Any]]:
    locator_to_ref = {(x["part"], x["source_locator"]): x["context_unit_ref"] for x in units}
    verse_locators: dict[int, list[tuple[str, str]]] = defaultdict(list)
    for unit in units:
        if unit["unit_kind"] == "verse_line":
            verse_locators[unit["part"]].append((unit["source_locator"], unit["context_unit_ref"]))
    meta = {x["context_unit_ref"]: x for x in units}
    db = sqlite3.connect(f"file:{REPO / DE_OCC_DB}?mode=ro", uri=True)
    rows = db.execute("""SELECT o.occurrence_id,s.part_order,o.token_ordinal,o.exact_form,o.normalized_form,
      o.exact_form_sha256,o.normalized_form_sha256,o.text_node_path,o.start_offset,o.end_offset
      FROM occurrences o JOIN source_items s USING(item_ref) ORDER BY s.part_order,o.token_ordinal""").fetchall()
    db.close()
    result = []
    for oid, part, ordinal, surface, normalized, exact_sha, normalized_sha, path, start, end in rows:
        cursor = path
        ref = None
        while "/" in cursor:
            if (int(part), cursor) in locator_to_ref:
                ref = locator_to_ref[(int(part), cursor)]
                break
            cursor = cursor.rsplit("/", 1)[0]
        # The existing German occurrence index addresses nested verse paths
        # with a document-global outer div ordinal, while the part-local
        # technical witness restarts that ordinal.  Resolve the unique inner
        # verse suffix without changing either source address.
        if ref is None and "/lg[" in path:
            inner = path[path.find("/div[", path.find("/div[") + 1):]
            candidates = [unit_ref for locator, unit_ref in verse_locators[int(part)]
                          if locator.endswith(inner)]
            if len(candidates) == 1:
                ref = candidates[0]
        unit = meta.get(ref)
        key = lexical.base_key(normalized)
        result.append({"existing_occurrence_ref": oid, "language": "de", "part": int(part),
                       "token_ordinal": int(ordinal), "surface": surface, "exact_sha256": exact_sha,
                       "normalized": normalized, "normalized_sha256": normalized_sha,
                       "analysis_key": key, "analysis_key_sha256": h(key), "context_unit_ref": ref,
                       "reading_ref": unit["reading_ref"] if unit else None,
                       "unit_kind": unit["unit_kind"] if unit else "structural_or_unmapped",
                       "witness_order": unit["witness_order"] if unit else int(ordinal),
                       "source_locator_sha256": h(path), "start": int(start), "end": int(end),
                       "in_work_scope": not (int(part) == 4 and path.startswith("TEI/text[1]/body[1]/div[21]/"))})
    return result


def normalize_ru_occurrences(rows: list[dict[str, Any]], units: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    result = []
    for row in rows:
        unit = units.get(row["context_unit_ref"])
        result.append({"existing_occurrence_ref": row["occurrence_id"], "language": "ru", "part": row["part"],
                       "token_ordinal": row["ordinal"], "surface": row["surface"], "exact_sha256": row["exact_sha256"],
                       "normalized": row["normalized"], "normalized_sha256": row["normalized_sha256"],
                       "analysis_key": row["analysis_key"], "analysis_key_sha256": row["analysis_key_sha256"],
                       "context_unit_ref": row["context_unit_ref"], "reading_ref": unit["reading_ref"] if unit else row["reading"],
                       "unit_kind": unit["unit_kind"] if unit else row["role"],
                       "witness_order": unit["witness_order"] if unit else 1_000_000 + row["ordinal"],
                       "source_locator_sha256": h(row["unit_id"]), "start": row["start"], "end": row["end"],
                       "in_work_scope": True})
    return result


def verify_census(rows: list[dict[str, Any]]) -> dict[str, int]:
    de = [x for x in rows if x["language"] == "de"]
    ru = [x for x in rows if x["language"] == "ru"]
    de_scope = [x for x in de if x["in_work_scope"]]
    census = {"de_exact_occurrences": len(de), "ru_exact_occurrences": len(ru),
              "de_exact_forms": len({x["surface"] for x in de}),
              "de_analysis_forms": len({x["analysis_key"] for x in de}),
              "de_work_scope_occurrences": len(de_scope),
              "de_work_scope_exact_forms": len({x["surface"] for x in de_scope}),
              "de_work_scope_analysis_forms": len({x["analysis_key"] for x in de_scope}),
              "ru_exact_forms": len({x["surface"] for x in ru}),
              "ru_normalized_forms": len({x["normalized"] for x in ru}),
              "ru_analysis_forms": len({x["analysis_key"] for x in ru})}
    expected = {key: EXPECTED[key] for key in census}
    if census != expected:
        raise BuildError(f"exact occurrence/form census drift: {census}")
    return census


def speaker_candidates(units: list[dict[str, Any]]) -> list[dict[str, Any]]:
    states: dict[tuple[str, str], str] = {}
    result = []
    for row in sorted(units, key=lambda x: (x["language"], x["witness_order"])):
        key = (row["language"], row["reading_ref"])
        state = states.get(key, "zarathustra_or_external_narrator")
        patterns = DE_SPEAKERS if row["language"] == "de" else RU_SPEAKERS
        text = row["text"].casefold().replace("\n", " ")
        detected = next((role for role, pattern in patterns if re.search(pattern, text)), None)
        if row["unit_kind"] == "verse_line":
            primary, alternatives, status = "song_or_performative_voice", ["zarathustra", "named_or_personified_voice"], "ambiguous"
            cue, group = "witness_attested_verse_line", f"{row['language']}.{row['reading_ref']}.verse_voice"
        elif detected:
            primary, alternatives, status = f"mixed_external_narrator_and_{detected}", ["external_narrator", detected], "ambiguous"
            cue, group = "explicit_speech_reporting_cue_inside_unit", f"{row['language']}.{row['reading_ref']}.explicit_transition.{detected}"
            states[key] = detected
        elif state != "zarathustra_or_external_narrator":
            primary, alternatives, status = state, ["external_narrator"], "proposed"
            cue, group = "continuation_of_last_explicit_state_within_reading", None
        else:
            primary, alternatives, status = state, ["zarathustra", "external_narrator"], "unresolved"
            cue, group = "no_explicit_cue_or_prior_state_in_reading", f"{row['language']}.{row['reading_ref']}.unresolved_opening_state"
        result.append({"binding": row["context_unit_ref"], "context_unit_ref": row["context_unit_ref"],
                       "language": row["language"], "part": row["part"], "reading_ref": row["reading_ref"],
                       "unit_kind": row["unit_kind"], "witness_order": row["witness_order"],
                       "primary_role": primary, "alternative_roles": alternatives,
                       "attribution_status": status, "attribution_cue": cue, "exception_group": group,
                       "accepted": False, "human_judgment": False, "graph_effect": False, "canon_effect": False})
    if len(result) != 7743:
        raise BuildError(f"speaker population drift: {len(result)}")
    return result


def form_inventory(occurrences: list[dict[str, Any]]) -> dict[tuple[str, str], dict[str, Any]]:
    result: dict[tuple[str, str], dict[str, Any]] = {}
    for occ in occurrences:
        if not occ["in_work_scope"]:
            continue
        key = (occ["language"], occ["analysis_key"])
        row = result.setdefault(key, {"language": occ["language"], "analysis_key": occ["analysis_key"],
                                      "analysis_key_sha256": occ["analysis_key_sha256"], "occurrence_count": 0,
                                      "exact_hashes": set(), "normalized_hashes": set(), "parts": set(), "readings": set()})
        row["occurrence_count"] += 1
        row["exact_hashes"].add(occ["exact_sha256"])
        row["normalized_hashes"].add(occ["normalized_sha256"])
        row["parts"].add(occ["part"])
        if occ["reading_ref"]:
            row["readings"].add(occ["reading_ref"])
    return result


def probe_profiles(request: dict[str, Any], lexical: Any, morphology: Any) -> list[dict[str, Any]]:
    result = []
    for field, tier in (("lexical_probes", "direct"), ("semantic_neighbor_probes", "semantic_neighbor")):
        for language in ("de", "ru"):
            for display in request[field][language]:
                normalized = lexical.ru_key(display) if language == "ru" else lexical.base_key(display)
                result.append({"tier": tier, "language": language, "display": display, "normalized": normalized,
                               "normalized_sha256": h(normalized),
                               "signatures": {x[0] for x in morphology.signatures(normalized, language)}})
    return result


def matches(form: str, profile: dict[str, Any], morphology: Any) -> tuple[bool, bool, str]:
    if form == profile["normalized"]:
        return True, True, "exact_analysis_form"
    variants = [form] + (form.split("-") if profile["language"] == "de" and "-" in form else [])
    for variant in variants:
        signatures = {x[0] for x in morphology.signatures(variant, profile["language"])}
        if signatures & profile["signatures"]:
            return True, False, "shared_reversible_signature"
    if profile["language"] == "de" and profile["normalized"].endswith("niss"):
        stem = profile["normalized"][:-4]
        if len(stem) >= 4 and form.startswith(stem):
            return True, False, "historical_niss_prefix_candidate"
    return False, False, ""


def select_forms(forms: dict[tuple[str, str], dict[str, Any]], profiles: list[dict[str, Any]],
                 morphology: Any) -> tuple[dict[tuple[str, str], dict[str, Any]], list[dict[str, Any]]]:
    selected: dict[tuple[str, str], dict[str, Any]] = {}
    gaps = []
    rank = {"direct_exact": 0, "direct_morphology": 1, "semantic_exact": 2, "semantic_morphology": 3}
    for profile in profiles:
        exact_seen = False
        for key, form in forms.items():
            if form["language"] != profile["language"]:
                continue
            matched, exact, method = matches(form["analysis_key"], profile, morphology)
            if not matched:
                continue
            exact_seen |= exact
            kind = ("direct" if profile["tier"] == "direct" else "semantic") + ("_exact" if exact else "_morphology")
            candidate = {**form, "selection_kind": kind, "selection_method": method,
                         "probe_normalized_sha256": profile["normalized_sha256"], "probe_display": profile["display"],
                         "status": "proposed" if exact else "ambiguous"}
            if key not in selected or rank[kind] < rank[selected[key]["selection_kind"]]:
                selected[key] = candidate
        if not exact_seen:
            gaps.append({"schema_version": "tos_zarathustra_concept_probe_gap_v1",
                         "gap_code": "probe_exact_form_unobserved", "tier": profile["tier"],
                         "language": profile["language"], "probe_normalized_sha256": profile["normalized_sha256"],
                         "review_status": "unreviewed", "accepted": False, "graph_effect": False, "canon_effect": False})
    return selected, gaps


def select_occurrences(all_occurrences: list[dict[str, Any]],
                       forms: dict[tuple[str, str], dict[str, Any]]) -> list[dict[str, Any]]:
    result = []
    local: Counter[tuple[str, str]] = Counter()
    for occ in all_occurrences:
        if not occ["in_work_scope"]:
            continue
        form = forms.get((occ["language"], occ["analysis_key"]))
        if not form:
            continue
        unit_key = occ["context_unit_ref"] or f"unmapped:{occ['source_locator_sha256']}"
        local[(occ["language"], unit_key)] += 1
        tier = "semantic_neighbor" if form["selection_kind"].startswith("semantic") else "direct_or_morphological"
        binding = f"{tier}|{occ['language']}|{occ['existing_occurrence_ref']}|{occ['exact_sha256']}"
        result.append({"binding": binding, "language": occ["language"], "part": occ["part"],
                       "reading_ref": occ["reading_ref"], "unit_kind": occ["unit_kind"],
                       "context_unit_ref": occ["context_unit_ref"], "witness_order": occ["witness_order"],
                       "token_ordinal": occ["token_ordinal"],
                       "occurrence_ordinal_within_context": local[(occ["language"], unit_key)],
                       "existing_occurrence_ref": occ["existing_occurrence_ref"],
                       "surface_private": occ["surface"],
                       "exact_form_sha256": occ["exact_sha256"], "analysis_key_sha256": occ["analysis_key_sha256"],
                       "source_locator_sha256": occ["source_locator_sha256"],
                       "form_binding": f"{form['selection_kind']}|{form['language']}|{form['analysis_key_sha256']}|{form['probe_normalized_sha256']}",
                       "evidence_tier": tier, "selection_kind": form["selection_kind"],
                       "status": "proposed" if form["selection_kind"].endswith("exact") else "ambiguous"})
    return result


def english_task_protos(request: dict[str, Any], occurrences: list[dict[str, Any]],
                        units: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    """Prepare one source-first English task for every selected German occurrence."""
    alignment_members: dict[str, dict[str, set[str]]] = defaultdict(lambda: defaultdict(set))
    for unit in units.values():
        for link in unit["alignment_links"]:
            alignment_members[link["alignment_ref"]][unit["language"]].add(unit["context_unit_ref"])
    policy = request["english_generation"]
    tasks = []
    for occurrence in occurrences:
        if occurrence["language"] != policy["source_language"] or not occurrence["context_unit_ref"]:
            continue
        unit = units[occurrence["context_unit_ref"]]
        alignment_refs = sorted({link["alignment_ref"] for link in unit["alignment_links"]})
        ru_refs = sorted({ref for aid in alignment_refs for ref in alignment_members[aid]["ru"]})
        tasks.append({
            "binding": f"english_on_demand|{occurrence['binding']}",
            "source_occurrence_binding": occurrence["binding"],
            "source_context_unit_ref": occurrence["context_unit_ref"],
            "source_form_sha256": occurrence["exact_form_sha256"],
            "source_surface_private": occurrence["surface_private"],
            "alignment_refs": alignment_refs,
            "russian_comparator_context_refs": ru_refs,
            "required_views": policy["required_views"],
            "required_analysis_stages": policy["required_analysis_stages"],
        })
    return tasks


def relation_protos(occurrences: list[dict[str, Any]], units: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    result = []
    for occ in occurrences:
        relation = {"direct_exact": "lexical_realization", "direct_morphology": "morphological_realization",
                    "semantic_exact": "semantic_neighbor_candidate", "semantic_morphology": "semantic_neighbor_candidate"}[occ["selection_kind"]]
        result.append({"binding": f"{relation}|{occ['binding']}|concept", "relation_type": relation,
                       "subject_bindings": [occ["binding"]], "object_kind": "concept_candidate",
                       "object_bindings": ["concept"], "status": occ["status"], "support": 1})
        result.append({"binding": f"form_membership_candidate|{occ['binding']}|{occ['form_binding']}",
                       "relation_type": "form_membership_candidate", "subject_bindings": [occ["binding"]],
                       "object_kind": "form_candidate", "object_bindings": [occ["form_binding"]],
                       "status": occ["status"], "support": 1})
        if occ["context_unit_ref"]:
            result.append({"binding": f"spoken_by_candidate|{occ['binding']}|{occ['context_unit_ref']}",
                           "relation_type": "spoken_by_candidate", "subject_bindings": [occ["binding"]],
                           "object_kind": "speaker_candidate", "object_bindings": [occ["context_unit_ref"]],
                           "status": "ambiguous", "support": 1})
    parallel: dict[str, dict[str, list[dict[str, Any]]]] = defaultdict(lambda: defaultdict(list))
    reprises: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    direct: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for occ in occurrences:
        if occ["context_unit_ref"]:
            for link in units[occ["context_unit_ref"]]["alignment_links"]:
                parallel[link["alignment_ref"]][occ["language"]].append(occ)
        if occ["reading_ref"]:
            reprises[(occ["language"], occ["reading_ref"], occ["form_binding"])].append(occ)
        if occ["evidence_tier"] == "direct_or_morphological":
            direct[occ["language"]].append(occ)
    for aid, languages in sorted(parallel.items()):
        if languages["de"] and languages["ru"]:
            left = sorted({x["binding"] for x in languages["de"]})
            right = sorted({x["binding"] for x in languages["ru"]})
            result.append({"binding": f"translation_parallel_candidate|{aid}|{h('|'.join(left + right))}",
                           "relation_type": "translation_parallel_candidate", "subject_bindings": left,
                           "object_kind": "occurrence_candidate", "object_bindings": right,
                           "status": "ambiguous", "support": min(len(left), len(right)),
                           "alignment_ref": aid})
    for rows in reprises.values():
        ordered = sorted(rows, key=lambda x: (x["witness_order"], x["token_ordinal"]))
        for left, right in zip(ordered, ordered[1:]):
            result.append({"binding": f"same_reading_reprise|{left['binding']}|{right['binding']}",
                           "relation_type": "same_reading_reprise", "subject_bindings": [left["binding"]],
                           "object_kind": "occurrence_candidate", "object_bindings": [right["binding"]],
                           "status": "proposed", "support": 1})
    for rows in direct.values():
        ordered = sorted(rows, key=lambda x: (x["part"], x["witness_order"], x["token_ordinal"]))
        for left, right in zip(ordered, ordered[1:]):
            result.append({"binding": f"precedes_in_work|{left['binding']}|{right['binding']}",
                           "relation_type": "precedes_in_work", "subject_bindings": [left["binding"]],
                           "object_kind": "occurrence_candidate", "object_bindings": [right["binding"]],
                           "status": "proposed", "support": 1})
    return [row for _, row in sorted({x["binding"]: x for x in result}.items())]


def exclusions(request: dict[str, Any], occurrences: list[dict[str, Any]], lexical: Any) -> list[dict[str, Any]]:
    result = []
    for control in request["negative_controls"]:
        normalized = lexical.ru_key(control["form"]) if control["language"] == "ru" else lexical.base_key(control["form"])
        for occ in occurrences:
            if occ["in_work_scope"] and occ["language"] == control["language"] and occ["analysis_key"] == normalized:
                result.append({"schema_version": "tos_zarathustra_concept_exclusion_candidate_v1",
                               "control_code": control["control_code"], "language": occ["language"],
                               "existing_occurrence_ref": occ["existing_occurrence_ref"],
                               "context_unit_ref": occ["context_unit_ref"], "exact_form_sha256": occ["exact_sha256"],
                               "exclusion_status": "request_declared_negative_control",
                               "review_status": "unreviewed", "accepted": False,
                               "graph_effect": False, "canon_effect": False})
    return result


def identity_bindings(request: dict[str, Any], speakers: list[dict[str, Any]],
                      forms: dict[tuple[str, str], dict[str, Any]], occurrences: list[dict[str, Any]],
                      relations: list[dict[str, Any]], english_tasks: list[dict[str, Any]]) -> list[tuple[str, str]]:
    scoped_request = request_binding(request)
    result = [("workbench", "foundation-v1"), ("request", scoped_request),
              ("concept", f"concept|{scoped_request}")]
    result.extend(("speaker", x["binding"]) for x in speakers)
    result.extend(("form", request_local_binding(
        request, f"{x['selection_kind']}|{x['language']}|{x['analysis_key_sha256']}|{x['probe_normalized_sha256']}"))
        for x in forms.values())
    result.extend(("occurrence", request_local_binding(request, x["binding"])) for x in occurrences)
    result.extend(("relation", request_local_binding(request, x["binding"])) for x in relations)
    result.extend(("english_task", request_local_binding(request, x["binding"])) for x in english_tasks)
    return sorted(result)


def issue(expected: list[tuple[str, str]], frozen_at: str) -> None:
    if (REPO / ISSUANCE_REF).exists():
        raise BuildError("identity issuance already exists; refusing to remint")
    prefixes = {"workbench": "tos.annotation.concept-workbench.sid-", "request": "tos.annotation.concept-request.sid-",
                "concept": "tos.annotation.concept-candidate.sid-", "speaker": "tos.annotation.speaker-state-candidate.sid-",
                "form": "tos.annotation.form-family-candidate.sid-", "occurrence": "tos.annotation.concept-occurrence-candidate.sid-",
                "relation": "tos.annotation.concept-relation-candidate.sid-",
                "english_task": "tos.annotation.english-translation-task.sid-"}
    payload = {"schema_version": "tos_zarathustra_concept_workbench_identity_issuance_v1",
               "identity_policy": "opaque-id-independent-of-source-text-label-translation-speaker-name-and-current-interpretation",
               "issued_at": frozen_at,
               "records": [{"kind": kind, "binding": binding,
                            "id": prefixes[kind] + h(f"{kind}\n{binding}")[:32]}
                           for kind, binding in expected]}
    write(ISSUANCE_REF, jb(payload))


def identity_map(expected: list[tuple[str, str]]) -> dict[tuple[str, str], str]:
    payload = load_json(ISSUANCE_REF)
    result = {(x["kind"], x["binding"]): x["id"] for x in payload["records"]}
    if set(result) != set(expected) or len(set(result.values())) != len(result):
        raise BuildError("identity issuance mismatch")
    return result


def public_contexts(units: list[dict[str, Any]], speakers: list[dict[str, Any]],
                    ids: dict[tuple[str, str], str]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    speaker_rows = []
    speaker_by = {}
    for row in speakers:
        public = {k: v for k, v in row.items() if k != "binding"}
        public.update({"schema_version": "tos_zarathustra_speaker_state_candidate_v1",
                       "speaker_state_candidate_id": ids[("speaker", row["binding"])], "review_status": "unreviewed"})
        speaker_rows.append(public)
        speaker_by[row["context_unit_ref"]] = public
    readings: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in units:
        readings[(row["language"], row["reading_ref"])].append(row)
    contexts = []
    for rows in readings.values():
        rows.sort(key=lambda x: x["witness_order"])
        for n, row in enumerate(rows):
            contexts.append({"schema_version": "tos_zarathustra_context_unit_candidate_v1",
                             "context_unit_ref": row["context_unit_ref"], "language": row["language"],
                             "part": row["part"], "reading_ref": row["reading_ref"], "unit_kind": row["unit_kind"],
                             "witness_order": row["witness_order"],
                             "previous_context_unit_ref": rows[n - 1]["context_unit_ref"] if n else None,
                             "next_context_unit_ref": rows[n + 1]["context_unit_ref"] if n + 1 < len(rows) else None,
                             "anchor_refs": row["anchor_refs"], "exact_sha256": row["exact_sha256"],
                             "alignment_refs": [x["alignment_ref"] for x in row["alignment_links"]],
                             "alignment_statuses": sorted({x["status"] for x in row["alignment_links"]}),
                             "alignment_shapes": sorted({x["shape"] for x in row["alignment_links"]}),
                             "speaker_state_candidate_ref": speaker_by[row["context_unit_ref"]]["speaker_state_candidate_id"],
                             "source_text_included": False, "accepted": False, "graph_effect": False, "canon_effect": False})
    contexts.sort(key=lambda x: (x["language"], x["witness_order"]))
    speaker_rows.sort(key=lambda x: (x["language"], x["witness_order"]))
    return contexts, speaker_rows


def fate_probe(occurrences: list[dict[str, Any]], exclusions_rows: list[dict[str, Any]],
               selected: dict[tuple[str, str], dict[str, Any]], units: dict[str, dict[str, Any]]) -> dict[str, int]:
    direct = [x for x in occurrences if x["evidence_tier"] == "direct_or_morphological"]
    language_counts = Counter(x["language"] for x in direct)
    kind_counts = Counter((x["language"], x["unit_kind"]) for x in direct)
    alignments: dict[str, set[str]] = defaultdict(set)
    for row in direct:
        if row["context_unit_ref"]:
            for link in units[row["context_unit_ref"]]["alignment_links"]:
                alignments[row["language"]].add(link["alignment_ref"])
    de, ru = alignments["de"], alignments["ru"]
    semantic_form_hashes = {x["analysis_key_sha256"] for x in selected.values()
                            if x["language"] == "de" and x["probe_display"] == "Verhängniss"}
    return {"de_direct_occurrences": language_counts["de"], "ru_direct_occurrences": language_counts["ru"],
            "de_direct_prose_occurrences": kind_counts[("de", "paragraph")],
            "de_direct_verse_occurrences": kind_counts[("de", "verse_line")],
            "ru_direct_prose_occurrences": kind_counts[("ru", "paragraph")],
            "ru_direct_verse_occurrences": kind_counts[("ru", "verse_line")],
            "parallel_alignment_intersection": len(de & ru), "de_only_alignment_groups": len(de - ru),
            "ru_only_alignment_groups": len(ru - de), "alignment_union": len(de | ru),
            "lowercase_los_hard_negative_occurrences": sum(x["control_code"] == "de_los_particle_or_command" for x in exclusions_rows),
            "verhaengniss_family_semantic_candidates": sum(x["language"] == "de" and x["analysis_key_sha256"] in semantic_form_hashes for x in occurrences)}


def render(plan: dict[str, Any], request: dict[str, Any], units: list[dict[str, Any]], speakers: list[dict[str, Any]],
           all_forms: dict[tuple[str, str], dict[str, Any]], selected: dict[tuple[str, str], dict[str, Any]],
           occurrences: list[dict[str, Any]], relations: list[dict[str, Any]], gaps: list[dict[str, Any]],
           exclusions_rows: list[dict[str, Any]], english_tasks: list[dict[str, Any]],
           ids: dict[tuple[str, str], str], census: dict[str, int],
           alignment_count: int, statuses: Counter[str], shapes: Counter[str]) -> tuple[dict[Path, bytes], bytes, dict[str, Any]]:
    scoped_request = request_binding(request)
    concept_binding = f"concept|{scoped_request}"
    request_id = ids[("request", scoped_request)]
    concept_id = ids[("concept", concept_binding)]
    unit_by = {x["context_unit_ref"]: x for x in units}
    contexts, speaker_rows = public_contexts(units, speakers, ids)
    speaker_by = {x["context_unit_ref"]: x for x in speaker_rows}
    form_rows, form_ids = [], {}
    for row in sorted(selected.values(), key=lambda x: (x["language"], x["analysis_key_sha256"], x["selection_kind"])):
        binding = f"{row['selection_kind']}|{row['language']}|{row['analysis_key_sha256']}|{row['probe_normalized_sha256']}"
        fid = ids[("form", request_local_binding(request, binding))]
        form_ids[binding] = fid
        form_rows.append({"schema_version": "tos_zarathustra_request_form_family_candidate_v1",
                          "form_family_candidate_id": fid, "request_ref": request_id,
                          "language": row["language"], "analysis_key_sha256": row["analysis_key_sha256"],
                          "probe_normalized_sha256": row["probe_normalized_sha256"], "selection_kind": row["selection_kind"],
                          "selection_method": row["selection_method"], "occurrence_count": row["occurrence_count"],
                          "exact_form_variant_count": len(row["exact_hashes"]), "part_count": len(row["parts"]),
                          "reading_count": len(row["readings"]), "status": row["status"], "accepted": False,
                          "review_status": "unreviewed", "source_text_included": False,
                          "graph_effect": False, "canon_effect": False})
    occurrence_rows, occurrence_ids, occurrence_public_by_binding = [], {}, {}
    for row in occurrences:
        oid = ids[("occurrence", request_local_binding(request, row["binding"]))]
        occurrence_ids[row["binding"]] = oid
        public = {k: v for k, v in row.items() if k not in {"binding", "form_binding", "surface_private"}}
        public.update({"schema_version": "tos_zarathustra_concept_occurrence_candidate_v1",
                       "occurrence_candidate_id": oid,
                       "concept_candidate_ref": concept_id,
                       "form_family_candidate_ref": form_ids[row["form_binding"]],
                       "speaker_state_candidate_ref": speaker_by[row["context_unit_ref"]]["speaker_state_candidate_id"] if row["context_unit_ref"] else None,
                       "accepted": False, "review_status": "unreviewed", "source_text_included": False,
                       "graph_effect": False, "canon_effect": False})
        occurrence_rows.append(public)
        occurrence_public_by_binding[row["binding"]] = public
    english_task_rows = []
    english_task_validator = Draft202012Validator(load_json(ENGLISH_TASK_SCHEMA_REF))
    for task in english_tasks:
        row = {
            "schema_version": "tos_zarathustra_english_on_demand_task_v1",
            "english_task_id": ids[("english_task", request_local_binding(request, task["binding"]))],
            "request_ref": request_id,
            "source_occurrence_ref": occurrence_ids[task["source_occurrence_binding"]],
            "source_context_unit_ref": task["source_context_unit_ref"],
            "source_form_sha256": task["source_form_sha256"],
            "alignment_refs": task["alignment_refs"],
            "russian_comparator_context_refs": task["russian_comparator_context_refs"],
            "target_language": "en",
            "required_views": task["required_views"],
            "required_analysis_stages": task["required_analysis_stages"],
            "etymology_state": "citation_required_before_claim",
            "recognized_english_comparator_state": "sealed_until_candidate_frozen",
            "task_status": "ready_for_on_demand_ai_candidate",
            "source_text_included": False, "accepted": False, "review_status": "unreviewed",
            "graph_effect": False, "canon_effect": False,
        }
        english_task_validator.validate(row)
        english_task_rows.append(row)
    relation_rows = []
    for row in relations:
        if row["object_kind"] == "occurrence_candidate":
            object_refs = [occurrence_ids[x] for x in row["object_bindings"]]
        elif row["object_kind"] == "speaker_candidate":
            object_refs = [speaker_by[x]["speaker_state_candidate_id"] for x in row["object_bindings"]]
        elif row["object_kind"] == "form_candidate":
            object_refs = [form_ids[x] for x in row["object_bindings"]]
        else:
            object_refs = [concept_id]
        evidence_bindings = list(row["subject_bindings"])
        if row["object_kind"] == "occurrence_candidate":
            evidence_bindings.extend(row["object_bindings"])
        evidence_bindings = sorted(set(evidence_bindings))
        source_context_refs = sorted({
            occurrence_public_by_binding[binding]["context_unit_ref"]
            for binding in evidence_bindings
            if occurrence_public_by_binding[binding]["context_unit_ref"]
        })
        relation_rows.append({"schema_version": "tos_zarathustra_concept_relation_candidate_v1",
                              "relation_candidate_id": ids[("relation", request_local_binding(request, row["binding"]))],
                              "request_ref": request_id,
                              "relation_type": row["relation_type"],
                              "subject_refs": [occurrence_ids[x] for x in row["subject_bindings"]],
                              "object_refs": object_refs, "support": row["support"], "status": row["status"],
                              "evidence_refs": [occurrence_ids[x] for x in evidence_bindings],
                              "evidence_roles": ([{"evidence_ref": occurrence_ids[x], "role": "subject_occurrence"}
                                                  for x in row["subject_bindings"]]
                                                 + [{"evidence_ref": occurrence_ids[x], "role": "object_occurrence"}
                                                    for x in row["object_bindings"]]
                                                 if row["object_kind"] == "occurrence_candidate"
                                                 else [{"evidence_ref": occurrence_ids[x], "role": "subject_occurrence"}
                                                       for x in row["subject_bindings"]]),
                              "source_context_refs": source_context_refs,
                              "alignment_ref": row.get("alignment_ref"),
                              "provenance_event_ref": provenance_event_ref(request),
                              "maker": {"maker_kind": "software", "software_ref": str(GENERATOR_REF),
                                        "method_output_posture": "candidate_not_truth"},
                              "certainty": {"meaning": "candidate_status_not_truth_probability", "value": 0.5},
                              "competing_relation_refs": [],
                              "accepted": False, "review_status": "unreviewed", "semantic_fact_asserted": False,
                              "graph_effect": False, "canon_effect": False})
    relation_validator = Draft202012Validator(load_json(RELATION_SCHEMA_REF))
    for relation_row in relation_rows:
        relation_validator.validate(relation_row)
    concept = {"schema_version": "tos_zarathustra_generic_concept_candidate_v1",
               "concept_candidate_id": concept_id,
               "request_id": request_id,
               "workbench_ref": ids[("workbench", "foundation-v1")],
               "labels": {**request["labels"], "identity_dependency": False},
               "work_ref": "tos.work.friedrich-nietzsche.also-sprach-zarathustra", "languages": ["de", "ru"],
               "distinctions": request["distinctions"],
               "direct_or_morphological_occurrence_refs": [x["occurrence_candidate_id"] for x in occurrence_rows if x["evidence_tier"] == "direct_or_morphological"],
               "semantic_neighbor_occurrence_refs": [x["occurrence_candidate_id"] for x in occurrence_rows if x["evidence_tier"] == "semantic_neighbor"],
               "form_family_candidate_refs": [x["form_family_candidate_id"] for x in form_rows],
               "relation_candidate_refs": [x["relation_candidate_id"] for x in relation_rows],
               "sign_id": None, "concept_id": None, "review_status": "unreviewed", "accepted": False,
               "semantic_fact_asserted": False, "graph_effect": False, "canon_effect": False}
    groups: dict[str, list[str]] = defaultdict(list)
    for row in speaker_rows:
        if row["exception_group"]:
            groups[row["exception_group"]].append(row["speaker_state_candidate_id"])
    worklist = {"schema_version": "tos_zarathustra_speaker_exception_worklist_v1",
                "speaker_population": len(speaker_rows),
                "exception_groups": [{"exception_group": key, "candidate_refs": refs, "candidate_count": len(refs)} for key, refs in sorted(groups.items())],
                "exception_group_count": len(groups), "review_outcome_recorded": False}
    probe = fate_probe(occurrence_rows, exclusions_rows, selected, unit_by)
    expected_probe = {"de_direct_occurrences": 26, "ru_direct_occurrences": 29,
                      "de_direct_prose_occurrences": 26, "de_direct_verse_occurrences": 0,
                      "ru_direct_prose_occurrences": 29, "ru_direct_verse_occurrences": 0,
                      "parallel_alignment_intersection": 20, "de_only_alignment_groups": 4,
                      "ru_only_alignment_groups": 7, "alignment_union": 31,
                      "lowercase_los_hard_negative_occurrences": 10,
                      "verhaengniss_family_semantic_candidates": 6}
    if request["request_key"] == "fate" and probe != expected_probe:
        raise BuildError(f"fate acceptance probe drift: {probe}")
    evidence_counts = Counter(x["evidence_tier"] for x in occurrence_rows)
    relation_counts = Counter(x["relation_type"] for x in relation_rows)
    coverage = {"schema_version": "tos_zarathustra_concept_request_coverage_v1",
                "request_id": concept["request_id"], "concept_candidate_ref": concept["concept_candidate_id"],
                "alignment_units_scanned": alignment_count, "witness_context_units_scanned": len(units),
                "exact_occurrences_scanned": census["de_work_scope_occurrences"] + census["ru_exact_occurrences"],
                "source_item_occurrences_examined": census["de_exact_occurrences"] + census["ru_exact_occurrences"],
                "outside_work_occurrences_excluded": census["de_exact_occurrences"] - census["de_work_scope_occurrences"],
                "parts_scanned": 4, "languages_scanned": ["de", "ru"], "form_candidate_count": len(form_rows),
                "occurrence_candidate_count": len(occurrence_rows), "evidence_tier_counts": dict(sorted(evidence_counts.items())),
                "relation_type_counts": dict(sorted(relation_counts.items())), "probe_gap_count": len(gaps),
                "selected_occurrences_without_content_context": sum(x["context_unit_ref"] is None for x in occurrence_rows),
                "english_on_demand_task_count": len(english_task_rows),
                "english_translation_candidate_count": 0,
                "source_return_verified": True, "minimum_form_frequency": 1,
                "semantic_neighbors_counted_as_direct_mentions": False,
                "acceptance_probe": probe if request["request_key"] == "fate" else None,
                "requested_probe_exact_absence_count": len(gaps),
                "mechanically_complete_with_explicit_probe_gaps": all(x["context_unit_ref"] for x in occurrence_rows),
                "complete_for_declared_request": all(x["context_unit_ref"] for x in occurrence_rows)}
    referenced_graph_nodes = {
        ref for relation in relation_rows for ref in relation["subject_refs"] + relation["object_refs"]
    }
    referenced_graph_nodes.add(concept["concept_candidate_id"])
    graph = {"schema_version": "tos_zarathustra_candidate_concept_graph_v1", "request_ref": concept["request_id"],
             "topology": "concept_hub_not_occurrence_clique",
             "empty_result": not occurrence_rows,
             "nodes": ([{"id": concept["concept_candidate_id"], "kind": "concept_candidate", "labels": request["labels"]}]
                       + [{"id": x["form_family_candidate_id"], "kind": "form_family_candidate", "language": x["language"], "status": x["status"]}
                          for x in form_rows if x["form_family_candidate_id"] in referenced_graph_nodes]
                       + [{"id": x["occurrence_candidate_id"], "kind": "occurrence_candidate", "language": x["language"], "reading_ref": x["reading_ref"], "status": x["status"]}
                          for x in occurrence_rows if x["occurrence_candidate_id"] in referenced_graph_nodes]
                       + [{"id": x["speaker_state_candidate_id"], "kind": "speaker_state_candidate",
                           "language": x["language"], "status": x["attribution_status"]}
                          for x in speaker_rows
                          if x["speaker_state_candidate_id"] in referenced_graph_nodes]),
             "edges": [{"relation_candidate_ref": x["relation_candidate_id"], "relation_type": x["relation_type"],
                        "subject_refs": x["subject_refs"], "object_refs": x["object_refs"], "status": x["status"]} for x in relation_rows],
             "accepted_edge_count": 0, "semantic_fact_asserted": False, "graph_effect": False, "canon_effect": False}
    all_form_coverage = {"schema_version": "tos_zarathustra_all_form_coverage_v1",
                         "alignment_unit_count": alignment_count,
                         "witness_context_unit_counts": {"de": 3815, "ru": 3928},
                         "unit_kind_counts": {"de_paragraph": 3447, "de_verse_line": 368,
                                              "ru_paragraph": 3569, "ru_verse_line": 359},
                         "source_item_exact_occurrence_counts": {"de": census["de_exact_occurrences"], "ru": census["ru_exact_occurrences"]},
                         "work_scope_exact_occurrence_counts": {"de": census["de_work_scope_occurrences"], "ru": census["ru_exact_occurrences"]},
                         "outside_work_exact_occurrence_counts": {"de": census["de_exact_occurrences"] - census["de_work_scope_occurrences"], "ru": 0},
                         "work_scope_exact_form_counts": {"de": census["de_work_scope_exact_forms"], "ru": census["ru_exact_forms"]},
                         "work_scope_analysis_form_counts": {"de": census["de_work_scope_analysis_forms"], "ru": census["ru_analysis_forms"]},
                         "analysis_token_counts_in_content_units": {"de": sum(len(x["analysis_tokens"]) for x in units if x["language"] == "de"),
                                                                     "ru": sum(len(x["analysis_tokens"]) for x in units if x["language"] == "ru")},
                         "minimum_form_frequency": 1, "frequency_one_forms_retained": True,
                         "all_analysis_forms_have_explicit_state": True,
                         "exact_and_analysis_tokens_distinct": True, "line_join_reconstruction_overwrites_witness": False,
                         "readable_forms_tracked": False, "private_index_ref": str(PRIVATE_DB),
                         "accepted_lemma_count": 0, "semantic_effect": False}
    summary = {"schema_version": "tos_zarathustra_concept_workbench_summary_v1",
               "workbench_id": ids[("workbench", "foundation-v1")], "alignment_unit_count": alignment_count,
               "alignment_status_counts": dict(sorted(statuses.items())), "alignment_shape_counts": dict(sorted(shapes.items())),
               "witness_context_unit_count": len(units), "speaker_candidate_count": len(speaker_rows),
               "context_unit_count": len(contexts), "all_form_minimum_frequency": 1,
               "english_on_demand_task_count": len(english_task_rows),
               "english_translation_candidate_count": 0,
               "work_scope_exact_occurrence_count": census["de_work_scope_occurrences"] + census["ru_exact_occurrences"],
               "source_item_exact_occurrence_count": census["de_exact_occurrences"] + census["ru_exact_occurrences"],
               "request_count": 1, "sample_request": request["request_key"], "sample_form_candidate_count": len(form_rows),
               "sample_occurrence_candidate_count": len(occurrence_rows),
               "sample_direct_or_morphological_occurrence_count": evidence_counts["direct_or_morphological"],
               "sample_semantic_neighbor_occurrence_count": evidence_counts["semantic_neighbor"],
               "sample_relation_candidate_count": len(relation_rows), "sample_exclusion_count": len(exclusions_rows),
               "accepted_candidate_count": 0, "human_review_count": 0, "graph_effect": False, "canon_effect": False}
    outputs = {OUTPUTS["contexts"]: jlb(contexts), OUTPUTS["speakers"]: jlb(speaker_rows),
               OUTPUTS["all_forms"]: jb(all_form_coverage), OUTPUTS["speaker_worklist"]: jb(worklist),
               OUTPUTS["concept"]: jb(concept), OUTPUTS["forms"]: jlb(form_rows),
               OUTPUTS["occurrences"]: jlb(occurrence_rows), OUTPUTS["relations"]: jlb(relation_rows),
               OUTPUTS["gaps"]: jlb(gaps), OUTPUTS["exclusions"]: jlb(exclusions_rows),
               OUTPUTS["graph"]: jb(graph), OUTPUTS["coverage"]: jb(coverage),
               OUTPUTS["english_tasks"]: jlb(english_task_rows), OUTPUTS["summary"]: jb(summary)}
    selected_refs = {x["context_unit_ref"] for x in occurrence_rows if x["context_unit_ref"]}
    readings: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in units:
        readings[(row["language"], row["reading_ref"])].append(row)
    private_contexts = []
    for rows in readings.values():
        rows.sort(key=lambda x: x["witness_order"])
        positions = {x["context_unit_ref"]: n for n, x in enumerate(rows)}
        for ref in sorted(selected_refs & set(positions), key=lambda x: positions[x]):
            n = positions[ref]
            window = rows[max(0, n - 1):min(len(rows), n + 2)]
            private_contexts.append({"context_unit_ref": ref, "language": rows[n]["language"],
                                     "reading_ref": rows[n]["reading_ref"], "speaker_candidate": speaker_by[ref],
                                     "window": [{"context_unit_ref": x["context_unit_ref"], "exact_text": x["text"]} for x in window]})
    private_request = {"schema_version": "tos_zarathustra_concept_request_private_analysis_v1",
                       "request_id": concept["request_id"], "concept_candidate_ref": concept["concept_candidate_id"],
                       "content_posture": "private_exact_source_return_and_mutable_analysis_not_semantic_authority",
                       "request": request,
                       "selected_forms": [{**{k: v for k, v in row.items() if k not in {"parts", "readings", "exact_hashes", "normalized_hashes"}},
                                           "parts": sorted(row["parts"]), "readings": sorted(row["readings"]),
                                           "exact_hashes": sorted(row["exact_hashes"]), "normalized_hashes": sorted(row["normalized_hashes"])}
                                          for row in selected.values()], "contexts": private_contexts,
                       "english_generation_policy": request["english_generation"],
                       "english_tasks": [{
                           "english_task_id": ids[("english_task", request_local_binding(request, task["binding"]))],
                           "source_occurrence_ref": occurrence_ids[task["source_occurrence_binding"]],
                           "source_surface": task["source_surface_private"],
                           "source_context_unit_ref": task["source_context_unit_ref"],
                           "source_context": unit_by[task["source_context_unit_ref"]]["text"],
                           "russian_comparators": [
                               {"context_unit_ref": ref, "exact_text": unit_by[ref]["text"]}
                               for ref in task["russian_comparator_context_refs"]
                           ],
                           "candidate_output_schema_ref": str(ENGLISH_CANDIDATE_SCHEMA_REF),
                           "candidate_materialized": False,
                       } for task in english_tasks]}
    return outputs, jb(private_request), summary


def build_private_db(units: list[dict[str, Any]], speakers: list[dict[str, Any]],
                     occurrences: list[dict[str, Any]]) -> bytes:
    speaker_by = {x["context_unit_ref"]: x for x in speakers}
    PRIVATE_DB.parent.mkdir(parents=True, exist_ok=True)
    fd, raw = tempfile.mkstemp(prefix=".workbench-index.", suffix=".sqlite3", dir=PRIVATE_DB.parent)
    os.close(fd)
    path = Path(raw)
    try:
        db = sqlite3.connect(path)
        db.executescript("""PRAGMA page_size=4096; PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;
          CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID;
          CREATE TABLE context_units(context_unit_ref TEXT PRIMARY KEY,language TEXT NOT NULL,part INTEGER NOT NULL,
            reading_ref TEXT NOT NULL,unit_kind TEXT NOT NULL,witness_order INTEGER NOT NULL,exact_text TEXT NOT NULL,
            exact_sha256 TEXT NOT NULL,analysis_tokens_json TEXT NOT NULL,alignment_links_json TEXT NOT NULL,
            speaker_role TEXT NOT NULL,speaker_status TEXT NOT NULL) WITHOUT ROWID;
          CREATE TABLE exact_occurrences(existing_occurrence_ref TEXT PRIMARY KEY,language TEXT NOT NULL,part INTEGER NOT NULL,
            context_unit_ref TEXT,reading_ref TEXT,unit_kind TEXT NOT NULL,witness_order INTEGER NOT NULL,token_ordinal INTEGER NOT NULL,
            exact_form TEXT NOT NULL,exact_form_sha256 TEXT NOT NULL,normalized_form TEXT NOT NULL,
            normalized_form_sha256 TEXT NOT NULL,analysis_key TEXT NOT NULL,analysis_key_sha256 TEXT NOT NULL,
            source_locator_sha256 TEXT NOT NULL,start_offset INTEGER NOT NULL,end_offset INTEGER NOT NULL,
            work_ref TEXT NOT NULL,in_work_scope INTEGER NOT NULL,scope_exclusion_code TEXT) WITHOUT ROWID;
          CREATE TABLE analysis_forms(language TEXT NOT NULL,analysis_key TEXT NOT NULL,analysis_key_sha256 TEXT NOT NULL,
            occurrence_count INTEGER NOT NULL,exact_variant_count INTEGER NOT NULL,morphology_state TEXT NOT NULL,
            PRIMARY KEY(language,analysis_key)) WITHOUT ROWID;
          CREATE INDEX occurrence_analysis_idx ON exact_occurrences(language,analysis_key);
          CREATE INDEX occurrence_context_idx ON exact_occurrences(context_unit_ref,token_ordinal);
          CREATE INDEX context_order_idx ON context_units(language,witness_order);""")
        db.executemany("INSERT INTO metadata VALUES(?,?)", [
            ("schema_version", "tos_zarathustra_concept_workbench_private_index_v1"),
            ("authority_boundary", "private_exact_source_return_and_candidate_analysis_not_semantic_authority"),
            ("context_unit_count", str(len(units))), ("exact_occurrence_count", str(len(occurrences))),
            ("analysis_form_count", str(len({(x['language'], x['analysis_key']) for x in occurrences if x['in_work_scope']})))])
        for row in units:
            speaker = speaker_by[row["context_unit_ref"]]
            db.execute("INSERT INTO context_units VALUES(?,?,?,?,?,?,?,?,?,?,?,?)", (
                row["context_unit_ref"], row["language"], row["part"], row["reading_ref"], row["unit_kind"], row["witness_order"],
                row["text"], row["exact_sha256"], json.dumps(row["analysis_tokens"], ensure_ascii=False),
                json.dumps(row["alignment_links"], sort_keys=True), speaker["primary_role"], speaker["attribution_status"]))
        db.executemany("INSERT INTO exact_occurrences VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", [(
            x["existing_occurrence_ref"], x["language"], x["part"], x["context_unit_ref"], x["reading_ref"], x["unit_kind"],
            x["witness_order"], x["token_ordinal"], x["surface"], x["exact_sha256"], x["normalized"], x["normalized_sha256"],
            x["analysis_key"], x["analysis_key_sha256"], x["source_locator_sha256"], x["start"], x["end"],
            "tos.work.friedrich-nietzsche.also-sprach-zarathustra" if x["in_work_scope"] else "tos.work.friedrich-nietzsche.dionysos-dithyramben",
            int(x["in_work_scope"]), None if x["in_work_scope"] else "appended_separate_work_part4_div21")
            for x in occurrences])
        grouped: dict[tuple[str, str], dict[str, Any]] = {}
        for row in occurrences:
            if not row["in_work_scope"]:
                continue
            key = (row["language"], row["analysis_key"])
            form = grouped.setdefault(key, {"sha": row["analysis_key_sha256"], "count": 0, "exact": set()})
            form["count"] += 1
            form["exact"].add(row["exact_sha256"])
        db.executemany("INSERT INTO analysis_forms VALUES(?,?,?,?,?,?)", [
            (language, key, row["sha"], row["count"], len(row["exact"]),
             "unresolved_candidate_available_for_request_expansion")
            for (language, key), row in sorted(grouped.items())
        ])
        db.commit(); db.execute("VACUUM"); db.close()
        return path.read_bytes()
    finally:
        path.unlink(missing_ok=True)


def generate(include_ids: bool) -> tuple[dict[Path, bytes], dict[Path, tuple[bytes, int]], list[tuple[str, str]], dict[str, Any]]:
    plan = load_json(PLAN_REF); verify_inputs(plan)
    request = load_json(REQUEST_REF)
    Draft202012Validator(load_json(SCHEMA_REF)).validate(request)
    for role in ("reference_register", "etymology_route"):
        reference = request["english_generation"][role]
        if sha_file(REPO / reference["ref"]) != reference["sha256"]:
            raise BuildError(f"English generation reference drift: {role}")
    lexical = import_builder(LEXICAL_BUILDER, "tos_concept_workbench_lexical")
    morphology = import_builder(MORPH_BUILDER, "tos_concept_workbench_morphology")
    ru_builder = import_builder(RU_BUILDER, "tos_concept_workbench_ru")
    de_readings, ru_readings, alignment_map, alignment_count, statuses, shapes = source_maps()
    de_units = load_de_units(lexical, de_readings, alignment_map)
    ru_units, ru_raw = load_ru_units(lexical, ru_builder, ru_readings, alignment_map)
    units = de_units + ru_units
    unit_by = {x["context_unit_ref"]: x for x in units}
    all_occurrences = load_de_occurrences(lexical, de_units) + normalize_ru_occurrences(ru_raw, unit_by)
    census = verify_census(all_occurrences)
    speakers = speaker_candidates(units)
    all_forms = form_inventory(all_occurrences)
    selected, gaps = select_forms(all_forms, probe_profiles(request, lexical, morphology), morphology)
    occurrences = select_occurrences(all_occurrences, selected)
    english_tasks = english_task_protos(request, occurrences, unit_by)
    relations = relation_protos(occurrences, unit_by)
    allowed_relations = (set(request["relation_policy"]["structural_types"])
                         | set(request["relation_policy"]["allowed_types"]))
    relations = [row for row in relations if row["relation_type"] in allowed_relations]
    exclusion_rows = exclusions(request, all_occurrences, lexical)
    selected_keys = set(selected)
    exclusion_rows.extend({
        "schema_version": "tos_zarathustra_concept_exclusion_candidate_v1",
        "control_code": "outside_zarathustra_work_scope", "language": row["language"],
        "existing_occurrence_ref": row["existing_occurrence_ref"], "context_unit_ref": None,
        "exact_form_sha256": row["exact_sha256"], "exclusion_status": "separate_appended_work",
        "review_status": "source_scope_verified", "accepted": False, "graph_effect": False, "canon_effect": False,
    } for row in all_occurrences
      if not row["in_work_scope"] and (row["language"], row["analysis_key"]) in selected_keys)
    expected = identity_bindings(request, speakers, selected, occurrences, relations, english_tasks)
    preview = {"witness_context_unit_count": len(units), "speaker_candidate_count": len(speakers),
               "all_exact_occurrence_count": len(all_occurrences), "all_analysis_form_count": len(all_forms),
               "selected_form_candidate_count": len(selected), "occurrence_candidate_count": len(occurrences),
               "english_on_demand_task_count": len(english_tasks),
               "relation_candidate_count": len(relations), "exclusion_count": len(exclusion_rows),
               "request_output_root": str(OUTPUTS["concept"].parent),
               "private_request_ref": str(PRIVATE_REQUEST)}
    if not include_ids:
        return {}, {}, expected, preview
    ids = identity_map(expected)
    outputs, private_request, summary = render(plan, request, units, speakers, all_forms, selected,
                                                occurrences, relations, gaps, exclusion_rows, english_tasks, ids, census,
                                                alignment_count, statuses, shapes)
    private_db = build_private_db(units, speakers, all_occurrences)
    outputs[OUTPUTS["provenance"]] = jlb([{
        "schema_version": "tos_zarathustra_concept_workbench_event_v1",
        "event_id": provenance_event_ref(request),
        "event_type": "candidate_workbench_and_sample_request_built", "event_at": plan["frozen_at"],
        "input_refs": [x["ref"] for x in plan["inputs"].values()] + [
            str(SCHEMA_REF), str(RELATION_SCHEMA_REF), str(ENGLISH_TASK_SCHEMA_REF),
            str(ENGLISH_CANDIDATE_SCHEMA_REF), str(SEARCH_RESULT_SCHEMA_REF),
            str(WORD_ANALYSIS_TASK_SCHEMA_REF), str(SEARCH_QUERY_REF),
            str(WORD_ANALYSIS_PREPARE_REF), str(REQUEST_REF),
            request["english_generation"]["reference_register"]["ref"],
            request["english_generation"]["etymology_route"]["ref"],
        ],
        "output_refs": [str(x) for x in OUTPUTS.values()] + [str(PRIVATE_DB), str(PRIVATE_REQUEST)],
        "authority_effect": "candidate_only_no_human_review_graph_or_canon_effect"}])
    artifacts = [{"role": name, "ref": str(path), "sha256": sha_bytes(outputs[path])}
                 for name, path in OUTPUTS.items() if name != "manifest"]
    outputs[OUTPUTS["manifest"]] = jb({
        "schema_version": "tos_zarathustra_concept_workbench_manifest_v1", "route_id": "zarathustra-concept-workbench-v1",
        "workbench_id": ids[("workbench", "foundation-v1")], "plan_ref": str(PLAN_REF),
        "plan_sha256": sha_file(REPO / PLAN_REF), "request_schema_ref": str(SCHEMA_REF),
        "request_schema_sha256": sha_file(REPO / SCHEMA_REF),
        "relation_schema_ref": str(RELATION_SCHEMA_REF),
        "relation_schema_sha256": sha_file(REPO / RELATION_SCHEMA_REF),
        "english_task_schema_ref": str(ENGLISH_TASK_SCHEMA_REF),
        "english_task_schema_sha256": sha_file(REPO / ENGLISH_TASK_SCHEMA_REF),
        "english_candidate_schema_ref": str(ENGLISH_CANDIDATE_SCHEMA_REF),
        "english_candidate_schema_sha256": sha_file(REPO / ENGLISH_CANDIDATE_SCHEMA_REF),
        "concept_search_result_schema_ref": str(SEARCH_RESULT_SCHEMA_REF),
        "concept_search_result_schema_sha256": sha_file(REPO / SEARCH_RESULT_SCHEMA_REF),
        "concept_search_query_ref": str(SEARCH_QUERY_REF),
        "concept_search_query_sha256": sha_file(REPO / SEARCH_QUERY_REF),
        "word_analysis_task_schema_ref": str(WORD_ANALYSIS_TASK_SCHEMA_REF),
        "word_analysis_task_schema_sha256": sha_file(REPO / WORD_ANALYSIS_TASK_SCHEMA_REF),
        "word_analysis_prepare_ref": str(WORD_ANALYSIS_PREPARE_REF),
        "word_analysis_prepare_sha256": sha_file(REPO / WORD_ANALYSIS_PREPARE_REF),
        "request_refs": [{"ref": str(REQUEST_REF), "sha256": sha_file(REPO / REQUEST_REF)}],
        "identity_issuance_ref": str(ISSUANCE_REF), "identity_issuance_sha256": sha_file(REPO / ISSUANCE_REF),
        "generator_ref": str(GENERATOR_REF), "generator_sha256": sha_file(REPO / GENERATOR_REF), "artifacts": artifacts,
        "private_artifacts": [{"ref": str(PRIVATE_DB), "sha256": sha_bytes(private_db), "mode": "0600", "tracked": False},
                              {"ref": str(PRIVATE_REQUEST), "sha256": sha_bytes(private_request), "mode": "0600", "tracked": False}],
        "accepted_candidate_count": 0, "human_review_count": 0, "graph_effect": False, "canon_effect": False})
    return outputs, {PRIVATE_DB: (private_db, 0o600), PRIVATE_REQUEST: (private_request, 0o600)}, expected, summary


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
    _outputs, _private, expected, preview = generate(False)
    if issue_identities:
        issue(expected, load_json(PLAN_REF)["frozen_at"])
    identity_map(expected)
    outputs, private, _expected, summary = generate(True)
    for path, payload in outputs.items():
        write(path, payload)
    for path, (payload, mode) in private.items():
        write(path, payload, mode)
    return {**preview, **summary}


def check() -> dict[str, Any]:
    outputs, private, expected, summary = generate(True)
    identity_map(expected)
    for path, payload in outputs.items():
        target = REPO / path
        if not target.is_file() or target.read_bytes() != payload:
            raise BuildError(f"tracked parity mismatch: {path}")
    for path, (payload, mode) in private.items():
        target = REPO / path
        if not target.is_file() or target.read_bytes() != payload:
            raise BuildError(f"private parity mismatch: {path}")
        if stat.S_IMODE(target.stat().st_mode) != mode:
            raise BuildError(f"private mode mismatch: {path}")
        if target.is_symlink() or not target.is_file():
            raise BuildError(f"private artifact must be a regular non-symlink: {path}")
        ignored = subprocess.run(["git", "check-ignore", "-q", str(path)], cwd=REPO).returncode == 0
        tracked = subprocess.run(["git", "ls-files", "--error-unmatch", str(path)], cwd=REPO,
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
        if not ignored or tracked:
            raise BuildError(f"private artifact tracking boundary violation: {path}")
    return summary


def main() -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true")
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--preview", action="store_true")
    parser.add_argument("--issue-identities", action="store_true")
    parser.add_argument("--request", type=Path, default=ROUTE / "requests/fate.concept-request.v2.json",
                        help="repository-relative concept request JSON")
    args = parser.parse_args()
    try:
        configure_request(args.request)
        if args.preview:
            _outputs, _private, expected, result = generate(False)
            result = {"identity_count": len(expected), **result}
        elif args.build:
            result = build(args.issue_identities)
        else:
            if args.issue_identities:
                raise BuildError("--issue-identities is valid only with --build")
            result = check()
    except (BuildError, OSError, KeyError, ValueError, sqlite3.Error, json.JSONDecodeError,
            ValidationError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

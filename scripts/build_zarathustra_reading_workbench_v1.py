#!/usr/bin/env python3
"""Build a reversible whole-book discourse/formula layer over immutable sources."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import stat
import tempfile
from collections import Counter, defaultdict
from pathlib import Path

from zarathustra_discourse import METHOD, build_discourse, sha, validate_partition
from zarathustra_voice_policy import find_reporting_cues
from zarathustra_recurring_formulas import build_formulas


REPO = Path(__file__).resolve().parents[1]
WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")
ROUTE = Path("ToS/candidate-intake/zarathustra/reading-workbench-v1")
PRIVATE = WORK / "gold-sets/foundation-pilot-v1/local-content"
DATABASE = PRIVATE / "reading-workbench-v1/reading-workbench.v1.sqlite3"
POLICIES = ROUTE / "chapter-voice-policies.v1.json"
INPUTS = {
    "source_spine": (PRIVATE / "linguistic-source-spine-v1/linguistic-spine.v1.sqlite3",
                     WORK / "technical-markup/zarathustra-linguistic-source-spine-v1/manifest.v1.json"),
    "analysis_predecessor": (PRIVATE / "linguistic-analysis-spine-v1/linguistic-analysis.v1.sqlite3",
                             WORK / "technical-markup/zarathustra-linguistic-analysis-spine-v1/manifest.v1.json"),
    "concept_workbench": (PRIVATE / "concept-workbench-v1/workbench-index.v1.sqlite3",
                          Path("ToS/candidate-intake/zarathustra/concept-workbench-v1/manifest.v1.json")),
}
IMPLEMENTATIONS = [Path("scripts") / name for name in (
    "build_zarathustra_reading_workbench_v1.py", "zarathustra_discourse.py",
    "zarathustra_voice_policy.py", "zarathustra_recurring_formulas.py")]


def dumps(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def file_sha(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def read_json(path):
    return json.loads(path.read_text())


def open_ro(path):
    if path.is_symlink() or not path.is_file() or stat.S_IMODE(path.stat().st_mode) != 0o600:
        raise ValueError(f"expected mode-0600 regular private input: {path}")
    db = sqlite3.connect(path.resolve().as_uri()+"?mode=ro&immutable=1", uri=True)
    db.row_factory = sqlite3.Row
    if db.execute("PRAGMA quick_check").fetchone()[0] != "ok":
        raise ValueError("private input quick_check failed")
    return db


def owned_path(root, ref):
    relative = Path(ref)
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError("reading artifact ref must be root-relative")
    candidate = root / relative
    if candidate.is_symlink() or not candidate.resolve().is_relative_to(root.resolve()):
        raise ValueError("reading artifact escapes its configured root")
    return candidate


def load_inputs(source_root):
    databases, evidence = {}, []
    for role, (ref, manifest_ref) in INPUTS.items():
        path = owned_path(source_root, ref)
        # The source spine was issued on this branch; its reviewed input
        # manifests stay here while private bytes may live in a separate root.
        manifest = read_json(owned_path(source_root, manifest_ref))
        expected = next((x["sha256"] for x in manifest["private_artifacts"] if x["ref"] == str(ref)), None)
        actual = file_sha(path)
        if expected != actual:
            raise ValueError(f"predecessor fixity drift: {role}")
        databases[role] = open_ro(path)
        evidence.append({"ref": str(ref), "sha256": actual, "role": role,
                         "manifest_ref": str(manifest_ref), "manifest_sha256": file_sha(owned_path(source_root, manifest_ref))})
    return databases, evidence


def crosswalk_occurrences(contexts, surfaces, occurrences):
    """German XML occurrences -> exact current spans, never ordinal token joins.

    XML-local offsets cannot be re-labelled context-local offsets. Recover the
    ordered exact stream and verify every character; missing context assignments
    and unmatched strings stay explicit. The Russian old index has page/block
    coordinates, so this compatibility bridge deliberately covers German only.
    The complete new Russian surface spine remains independently addressable.
    """
    by_ref = {c["context_unit_ref"]: c for c in contexts}
    surface_by_span = {(s["context_unit_ref"], s["start_offset"], s["end_offset"]): s["surface_unit_id"] for s in surfaces}
    grouped = defaultdict(list)
    gaps = []
    for row in occurrences:
        if row["language"] != "de" or not row["in_work_scope"]:
            continue
        if row["context_unit_ref"] not in by_ref:
            gaps.append({"kind": "legacy_occurrence_context_unmapped", "occurrence_ref": row["existing_occurrence_ref"],
                         "status": "deferred"})
            continue
        grouped[row["context_unit_ref"]].append(row)
    mappings = []
    for ref, rows in grouped.items():
        cursor, text = 0, by_ref[ref]["exact_text"]
        for row in sorted(rows, key=lambda r: r["token_ordinal"]):
            form = row["exact_form"]
            start = text.find(form, cursor)
            if start < 0 or sha(form) != row["exact_form_sha256"]:
                gaps.append({"kind": "legacy_occurrence_exact_crosswalk_failed", "context_unit_ref": ref,
                             "occurrence_ref": row["existing_occurrence_ref"], "status": "deferred"})
                continue
            end = start + len(form)
            # No legacy numeric offsets are reused. Exact sequence alignment is
            # an explicit compatibility candidate, separate from surface IDs.
            mappings.append({"existing_occurrence_ref": row["existing_occurrence_ref"], "context_unit_ref": ref,
                             "surface_unit_ref": surface_by_span.get((ref, start, end)),
                             "start_offset": start, "end_offset": end, "exact_text": form,
                             "exact_sha256": sha(form), "status": "proposed"})
            cursor = end
    return mappings, gaps


def validate_policies(contexts, policies):
    grouped = defaultdict(list)
    for context in contexts:
        grouped[context["language"], context["reading_ref"]].append(context)
    checked = 0
    for chapter in policies["chapters"]:
        reading = chapter["reading_ref"]
        for language, witness in chapter["witnesses"].items():
            rows = grouped[language, reading]
            if len(rows) != witness["context_count"] or sha("".join(c["exact_text"] for c in rows)) != witness["chapter_exact_sha256"]:
                raise ValueError("chapter voice policy source drift")
        for rule in chapter["overrides"]:
            if rule["status"] not in {"proposed", "ambiguous", "deferred"}:
                raise ValueError("voice rule unexpectedly accepted")
            rows = grouped[rule["language"], reading]
            by_ref = {c["context_unit_ref"]: i for i, c in enumerate(rows)}
            if "start_context_ref" in rule:
                selected = rows[by_ref[rule["start_context_ref"]]:by_ref[rule["end_context_ref"]]+1]
                if not selected or len(selected) != rule["context_count"]:
                    raise ValueError("voice policy range extent drift")
                if selected[0]["exact_sha256"] != rule["start_context_exact_sha256"] or selected[-1]["exact_sha256"] != rule["end_context_exact_sha256"]:
                    raise ValueError("voice policy endpoint fixity drift")
                if sha("".join(c["exact_text"] for c in selected)) != rule["full_range_sha256"]:
                    raise ValueError("voice policy range fixity drift")
            checked += 1
    return checked


def create_database(path, contexts, sentences, clauses, alignments, segments, events,
                    mappings, families, memberships, relations, metadata):
    db = sqlite3.connect(path)
    db.executescript("""
        PRAGMA page_size=4096; PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;
        CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID;
        CREATE TABLE contexts(context_unit_ref TEXT PRIMARY KEY,language TEXT,part INTEGER,reading_ref TEXT,
          unit_kind TEXT,witness_order INTEGER,exact_text TEXT,exact_sha256 TEXT,anchor_refs_json TEXT) WITHOUT ROWID;
        CREATE TABLE source_sentences(sentence_unit_ref TEXT PRIMARY KEY,context_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT) WITHOUT ROWID;
        CREATE TABLE source_clauses(clause_id TEXT PRIMARY KEY,sentence_unit_ref TEXT,context_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT) WITHOUT ROWID;
        CREATE TABLE discourse_segments(segment_id TEXT PRIMARY KEY,context_unit_ref TEXT,sentence_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,speaker_role TEXT,speaker_status TEXT,
          speaker_candidates_json TEXT,evidence_refs_json TEXT,kind TEXT,quote_depth INTEGER,speech_turn_id TEXT,
          utterer_role TEXT,attribution_basis TEXT,performed_role TEXT,modality TEXT) WITHOUT ROWID;
        CREATE INDEX discourse_context_idx ON discourse_segments(context_unit_ref,start_offset,end_offset);
        CREATE TABLE quote_events(event_id TEXT PRIMARY KEY,context_unit_ref TEXT,offset INTEGER,event_json TEXT) WITHOUT ROWID;
        CREATE TABLE occurrence_spans(existing_occurrence_ref TEXT,context_unit_ref TEXT,surface_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT,
          PRIMARY KEY(existing_occurrence_ref,context_unit_ref,start_offset)) WITHOUT ROWID;
        CREATE TABLE formulas(formula_id TEXT PRIMARY KEY,normalized_text TEXT,token_count INTEGER,
          occurrence_count INTEGER,reading_count INTEGER,status TEXT) WITHOUT ROWID;
        CREATE TABLE formula_occurrences(formula_id TEXT,occurrence_id TEXT,context_unit_ref TEXT,start_offset INTEGER,
          end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT,
          PRIMARY KEY(occurrence_id,context_unit_ref,start_offset)) WITHOUT ROWID;
        CREATE INDEX formula_context_idx ON formula_occurrences(context_unit_ref,start_offset,end_offset);
        CREATE TABLE formula_relations(relation_id TEXT PRIMARY KEY,relation_json TEXT) WITHOUT ROWID;
        CREATE TABLE translation_alignments(alignment_id TEXT PRIMARY KEY,claim_id TEXT NOT NULL,granularity TEXT NOT NULL,
          part INTEGER NOT NULL,parent_paragraph_alignment_ref TEXT NOT NULL,parent_sentence_alignment_ref TEXT,
          candidate_role TEXT NOT NULL,correspondence_shape TEXT NOT NULL,ordered_source_unit_refs_json TEXT NOT NULL,
          ordered_target_unit_refs_json TEXT NOT NULL,exact_source_text TEXT NOT NULL,exact_target_text TEXT NOT NULL,
          score_millionths INTEGER NOT NULL,score_components_json TEXT NOT NULL,status TEXT NOT NULL,reason_codes_json TEXT NOT NULL,
          competing_alignment_refs_json TEXT NOT NULL,semantic_equivalence_asserted INTEGER NOT NULL,human_acceptance INTEGER NOT NULL) WITHOUT ROWID;
    """)
    db.executemany("INSERT INTO metadata VALUES(?,?)", sorted(metadata.items()))
    db.executemany("INSERT INTO contexts VALUES(?,?,?,?,?,?,?,?,?)", [tuple(c[k] for k in (
        "context_unit_ref", "language", "part", "reading_ref", "unit_kind", "witness_order", "exact_text", "exact_sha256", "anchor_refs_json")) for c in contexts])
    db.executemany("INSERT INTO source_sentences VALUES(?,?,?,?,?,?)", [tuple(s[k] for k in (
        "sentence_id", "context_unit_ref", "start_offset", "end_offset", "exact_text", "exact_sha256")) for s in sentences])
    db.executemany("INSERT INTO source_clauses VALUES(?,?,?,?,?,?,?,?)", [tuple(s[k] for k in (
        "clause_id", "sentence_id", "context_unit_ref", "start_offset", "end_offset", "exact_text", "exact_sha256", "boundary_status")) for s in clauses])
    db.executemany("INSERT INTO translation_alignments VALUES("+",".join("?"*19)+")", [tuple(a.values()) for a in alignments])
    db.executemany("INSERT INTO discourse_segments VALUES("+",".join("?"*18)+")", [(
        s["segment_id"], s["context_unit_ref"], s["sentence_unit_ref"], s["start_offset"], s["end_offset"],
        s["exact_text"], s["exact_sha256"], s["speaker_role"], s["speaker_status"], dumps(s["speaker_candidates"]),
        dumps(s["evidence_refs"]), s["kind"], s["quote_depth"], s["speech_turn_id"], s["utterer_role"], s["attribution_basis"],
        s["performed_role"], s["modality"]
    ) for s in segments])
    db.executemany("INSERT INTO quote_events VALUES(?,?,?,?)", [(e["event_id"], e["context_unit_ref"], e["offset"], dumps(e)) for e in events])
    db.executemany("INSERT INTO occurrence_spans VALUES(?,?,?,?,?,?,?,?)", [tuple(m[k] for k in (
        "existing_occurrence_ref", "context_unit_ref", "surface_unit_ref", "start_offset", "end_offset", "exact_text", "exact_sha256", "status")) for m in mappings])
    db.executemany("INSERT INTO formulas VALUES(?,?,?,?,?,?)", [(f["formula_id"], " ".join(f["normalized_tokens"]),
        f["token_count"], f["occurrence_count"], f["reading_count"], f["status"]) for f in families])
    db.executemany("INSERT INTO formula_occurrences VALUES(?,?,?,?,?,?,?,?)", [(
        m["formula_id"], m["formula_occurrence_id"], s["context_unit_ref"], s["start_offset"], s["end_offset"],
        s["exact_text"], s["exact_sha256"], m.get("status", "proposed")) for m in memberships for s in m["source_spans"]])
    db.executemany("INSERT INTO formula_relations VALUES(?,?)", [(r["relation_id"], dumps(r)) for r in relations])
    db.commit()
    db.execute("VACUUM")
    if db.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
        raise ValueError("output database failed integrity_check")
    db.close()


def materialize(source_root, destination):
    start_implementation = [{"ref": str(p), "sha256": file_sha(REPO / p)} for p in IMPLEMENTATIONS]
    policy_sha = file_sha(owned_path(source_root, POLICIES))
    inputs, evidence = load_inputs(source_root)
    contexts = [dict(r) for r in inputs["source_spine"].execute("SELECT * FROM contexts ORDER BY language,witness_order")]
    sentences = [dict(r) for r in inputs["source_spine"].execute("SELECT * FROM sentences ORDER BY language,witness_ordinal")]
    surfaces = [dict(r) for r in inputs["source_spine"].execute("SELECT * FROM surface_units ORDER BY language,witness_ordinal")]
    clauses = [dict(r) for r in inputs["analysis_predecessor"].execute(
        "SELECT clause_unit_id AS clause_id,sentence_unit_ref AS sentence_id,context_unit_ref,"
        "start_offset,end_offset,exact_text,exact_sha256,boundary_status FROM clauses "
        "ORDER BY language,part,context_unit_ref,sentence_clause_ordinal")]
    alignments = [dict(r) for r in inputs["analysis_predecessor"].execute("SELECT * FROM translation_alignments ORDER BY alignment_id")]
    old_occurrences = [dict(r) for r in inputs["concept_workbench"].execute("SELECT * FROM exact_occurrences ORDER BY language,part,token_ordinal")]
    policies = read_json(owned_path(source_root, POLICIES))
    chapter_refs = {c["reading_ref"] for c in contexts if ".r" in c["reading_ref"]}
    if {c["reading_ref"] for c in policies["chapters"]} != chapter_refs or len(chapter_refs) != 81:
        raise ValueError("voice policy must cover exactly all 81 chapter refs")
    policy_count = validate_policies(contexts, policies)
    segments, events, gaps = build_discourse(contexts, sentences, policies, find_reporting_cues)
    conservation = validate_partition(contexts, segments)
    mappings, mapping_gaps = crosswalk_occurrences(contexts, surfaces, old_occurrences)
    gaps.extend(mapping_gaps)
    families, memberships, relations, formula_receipt = build_formulas(contexts, surfaces)
    # Validate copied alignment anchors against unchanged source contexts, using
    # the real sentence/clause tables, not the predecessor's faulty locator refs.
    context_map = {c["context_unit_ref"]: c for c in contexts}
    anchors = {s["sentence_id"]: s for s in sentences}
    anchors.update({c["clause_id"]: c for c in clauses})
    anchor_count = 0
    for alignment in alignments:
        for field in ("ordered_source_unit_refs_json", "ordered_target_unit_refs_json"):
            for ref in json.loads(alignment[field]):
                unit = anchors[ref]
                exact = context_map[unit["context_unit_ref"]]["exact_text"][unit["start_offset"]:unit["end_offset"]]
                if exact != unit["exact_text"] or sha(exact) != unit["exact_sha256"]:
                    raise ValueError("predecessor alignment source anchor mismatch")
                anchor_count += 1
    metadata = {e["role"]+"_sha256": e["sha256"] for e in evidence}
    metadata.update(method_version=METHOD, builder_sha256=file_sha(Path(__file__)),
                    source_root_posture="explicit_private_input_root_not_public_fallback",
                    accepted="false", human_review="false", semantic_equivalence_asserted="false")
    create_database(destination, contexts, sentences, clauses, alignments, segments, events,
                    mappings, families, memberships, relations, metadata)
    per_reading = []
    for language, reading in sorted({(c["language"], c["reading_ref"]) for c in contexts}):
        group_contexts = [c for c in contexts if (c["language"], c["reading_ref"]) == (language, reading)]
        group_segments = [s for s in segments if (s["language"], s["reading_ref"]) == (language, reading)]
        per_reading.append({"language": language, "reading_ref": reading, "context_count": len(group_contexts),
                            "context_refs_sha256": sha(dumps([c["context_unit_ref"] for c in group_contexts])),
                            "segment_count": len(group_segments),
                            "speaker_roles": dict(Counter(s["speaker_role"] for s in group_segments)),
                            "speaker_status_counts": dict(Counter(s["speaker_status"] for s in group_segments))})
    receipt = {"schema_version": "tos_zarathustra_reading_coverage_v1", "method_version": METHOD,
               "chapters_per_language": {lang: len({c["reading_ref"] for c in contexts if c["language"] == lang and ".r" in c["reading_ref"]}) for lang in ("de", "ru")},
               "conservation": conservation, "sentence_count": len(sentences), "clause_count": len(clauses),
               "source_bound_voice_policies_checked": policy_count,
               "alignment_anchors_checked_with_real_locator": anchor_count,
               "quotation_actions": {lang: dict(Counter(e["action"] for e in events if e["language"] == lang)) for lang in ("de", "ru")},
               "max_quote_depth": {lang: max(e["depth_after"] for e in events if e["language"] == lang) for lang in ("de", "ru")},
               "legacy_german_occurrence_crosswalk": {"mapped": len(mappings), "gaps": len(mapping_gaps),
                   "method": "ordered_exact_stream_candidate_not_relabelled_XML_offsets", "russian_legacy_bridge": "deferred_page_block_coordinates"},
               "formulas": formula_receipt, "gap_counts": dict(Counter(g["kind"] for g in gaps)),
               "accepted": False, "human_review": False, "canon_effect": False,
               "limitations": ["voice policies and explicit reporting cues are candidates, not a coreference model",
                               "quoted frame utterer does not resolve the embedded voice",
                               "legacy morphology and first-verb dependency heuristics are not improved or certified here",
                               "DE/RU sentence and clause alignments remain predecessor proposals; verse exclusions retained",
                               "etymology and English require a separate source-bound on-demand agent analysis"]}
    evidence.append({"ref": str(POLICIES), "sha256": policy_sha, "role": "source_visible_voice_policy"})
    implementation = [{"ref": str(p), "sha256": file_sha(REPO / p)} for p in IMPLEMENTATIONS]
    if start_implementation != implementation or policy_sha != file_sha(owned_path(source_root, POLICIES)):
        raise ValueError("method or voice policies changed during materialization; rebuild from one snapshot")
    tracked = {
        "coverage-receipt.v1.json": receipt,
        "reading-census.v1.jsonl": per_reading,
        "quote-boundary-ledger.v1.jsonl": events,
        "gap-ledger.v1.jsonl": gaps,
        "formula-census.v1.jsonl": [{k: v for k, v in f.items() if k not in {"normalized_tokens", "normalized_text", "exact_text", "display_text"}} for f in families],
    }
    encoded = {name: (("\n".join(dumps(r) for r in value)+"\n") if name.endswith("jsonl")
                      else json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True)+"\n").encode()
               for name, value in tracked.items()}
    manifest = {"schema_version": "tos_zarathustra_reading_manifest_v1", "method_version": METHOD,
                "private_database": {"ref": str(DATABASE), "sha256": file_sha(destination), "mode": "0600"},
                "inputs": evidence, "implementation": implementation,
                "artifacts": [{"ref": str(ROUTE / name), "sha256": hashlib.sha256(payload).hexdigest()} for name, payload in encoded.items()],
                "predecessor_retained": True, "tracked_source_strings": False, "accepted": False,
                "publication_posture": "excluded_from_public_bundle", "human_review": False, "canon_effect": False}
    encoded["manifest.v1.json"] = (json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True)+"\n").encode()
    for db in inputs.values():
        db.close()
    return encoded, receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--build", action="store_true")
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--validate-tracked", action="store_true")
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True,
                        help="separate private data directory; never writes into the software checkout")
    args = parser.parse_args()
    source_root, output_root = args.source_root.absolute(), args.output_root.absolute()
    if any(root != root.resolve() for root in (source_root, output_root)):
        raise ValueError("data roots may not contain symlinks")
    if (output_root.is_relative_to(source_root) or source_root.is_relative_to(output_root)
            or output_root.is_relative_to(REPO) or REPO.is_relative_to(output_root)):
        raise ValueError("output root must be separate from source data and the software checkout")
    if args.validate_tracked:
        manifest = read_json(owned_path(output_root, ROUTE / "manifest.v1.json"))
        for entry in manifest["artifacts"]:
            if file_sha(owned_path(output_root, entry["ref"])) != entry["sha256"]:
                raise ValueError(f"tracked currentness drift: {entry['ref']}")
        policy_input = next(e for e in manifest["inputs"] if e["role"] == "source_visible_voice_policy")
        if file_sha(owned_path(source_root, policy_input["ref"])) != policy_input["sha256"]:
            raise ValueError("voice policy changed without rebuild")
        print("reading workbench tracked currentness OK; no semantic acceptance")
        return
    target = owned_path(output_root, DATABASE)
    if args.build and (target.exists() or owned_path(output_root, ROUTE).exists()):
        raise ValueError("build requires a new output dataset; existing reading output is immutable")
    if args.check and not target.is_file():
        raise ValueError("reading dataset to check does not exist")
    target.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="reading-build-", dir=target.parent) as temporary:
        database = Path(temporary) / "reading.sqlite3"
        encoded, receipt = materialize(source_root, database)
        os.chmod(database, 0o600)
        if args.check:
            if file_sha(database) != file_sha(target):
                raise ValueError("private database deterministic parity drift")
            for name, payload in encoded.items():
                if owned_path(output_root, ROUTE / name).read_bytes() != payload:
                    raise ValueError(f"tracked deterministic parity drift: {name}")
        else:
            os.replace(database, target)
            for name, payload in encoded.items():
                companion = owned_path(output_root, ROUTE / name)
                companion.parent.mkdir(parents=True, exist_ok=True)
                companion.write_bytes(payload)
                os.chmod(companion, 0o600)
    print(json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()

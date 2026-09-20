from __future__ import annotations

import hashlib
import importlib.util
import json
import sqlite3
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("reading_query_contract", ROOT / "scripts/query_zarathustra_reading_workbench_v1.py")
assert SPEC and SPEC.loader
QUERY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(QUERY)

CONCEPT_QUERY_SPEC = importlib.util.spec_from_file_location(
    "concept_query_contract", ROOT / "scripts/query_zarathustra_concept_workbench_v1.py"
)
assert CONCEPT_QUERY_SPEC and CONCEPT_QUERY_SPEC.loader
CONCEPT_QUERY = importlib.util.module_from_spec(CONCEPT_QUERY_SPEC)
CONCEPT_QUERY_SPEC.loader.exec_module(CONCEPT_QUERY)

CONCEPT_BUILDER_SPEC = importlib.util.spec_from_file_location(
    "concept_builder_contract", ROOT / "scripts/build_zarathustra_concept_workbench_v1.py"
)
assert CONCEPT_BUILDER_SPEC and CONCEPT_BUILDER_SPEC.loader
CONCEPT_BUILDER = importlib.util.module_from_spec(CONCEPT_BUILDER_SPEC)
CONCEPT_BUILDER_SPEC.loader.exec_module(CONCEPT_BUILDER)


def fixture_database() -> sqlite3.Connection:
    database = sqlite3.connect(":memory:")
    database.row_factory = sqlite3.Row
    database.executescript("""
        CREATE TABLE occurrence_spans(existing_occurrence_ref TEXT, context_unit_ref TEXT,
          surface_unit_ref TEXT, start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT);
        CREATE TABLE discourse_segments(segment_id TEXT,context_unit_ref TEXT,sentence_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,speaker_role TEXT,
          speaker_status TEXT,speaker_candidates_json TEXT,evidence_refs_json TEXT,kind TEXT,quote_depth INTEGER,
          speech_turn_id TEXT,utterer_role TEXT,attribution_basis TEXT);
        CREATE TABLE source_sentences(sentence_unit_ref TEXT,context_unit_ref TEXT,start_offset INTEGER,
          end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT);
        CREATE TABLE source_clauses(clause_id TEXT,sentence_unit_ref TEXT,context_unit_ref TEXT,start_offset INTEGER,
          end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT);
        CREATE TABLE formula_occurrences(formula_id TEXT,occurrence_id TEXT,context_unit_ref TEXT,start_offset INTEGER,
          end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT);
        CREATE TABLE formulas(formula_id TEXT,normalized_text TEXT,token_count INTEGER,occurrence_count INTEGER,reading_count INTEGER,status TEXT);
    """)
    return database


def file_sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reading_provider_fixture(root: Path) -> dict[str, Path]:
    """Create only the small manifest/database surface needed by provider checks."""
    source_root = root / "source-data"
    analysis_root = root / "analysis-data"
    source_root.mkdir()
    analysis_root.mkdir()

    policy = source_root / "policy.json"
    policy.write_text("{\"status\":\"proposed\"}\n", encoding="utf-8")

    database_path = analysis_root / "private-reading.sqlite3"
    database = sqlite3.connect(database_path)
    database.execute("CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT)")
    database.execute("INSERT INTO metadata VALUES(?, ?)", ("concept_workbench_sha256", "concept-source-hash"))
    database.execute("INSERT INTO metadata VALUES(?, ?)", ("method_version", "synthetic-v1"))
    database.commit()
    database.close()
    database_path.chmod(0o600)
    database_sha = file_sha(database_path)

    source_manifest = source_root / "source-manifest.json"
    source_manifest.write_text(json.dumps({
        "private_artifacts": [{"ref": "concept-workbench.sqlite3", "sha256": "concept-source-hash"}],
    }), encoding="utf-8")

    companion = analysis_root / "coverage-receipt.v1.json"
    companion.write_text("{}\n", encoding="utf-8")
    manifest_path = analysis_root / QUERY.MANIFEST_REF
    manifest_path.parent.mkdir(parents=True)
    manifest_path.write_text(json.dumps({
        "schema_version": "tos_zarathustra_reading_manifest_v1",
        "private_database": {
            "ref": database_path.relative_to(analysis_root).as_posix(),
            "sha256": database_sha, "mode": "0600",
        },
        "inputs": [{"ref": policy.relative_to(source_root).as_posix(),
                    "sha256": file_sha(policy), "role": "source_visible_voice_policy"}],
        "implementation": [{"ref": "scripts/query_zarathustra_reading_workbench_v1.py",
                             "sha256": "historical-compatible-software-hash"}],
        "artifacts": [{"ref": companion.relative_to(analysis_root).as_posix(),
                        "sha256": file_sha(companion)}],
        "accepted": False, "human_review": False, "canon_effect": False,
        "publication_posture": "excluded_from_public_bundle",
    }), encoding="utf-8")
    return {
        "source_root": source_root,
        "analysis_root": analysis_root,
        "database": database_path,
        "companion": companion,
        "manifest": manifest_path,
        "policy": policy,
        "database_sha": database_sha,
    }


class ReadingQueryTests(unittest.TestCase):
    def setUp(self):
        self.database = fixture_database()
        self.context = '😀 „Schicksal!“ sprach Zarathustra. So sprach er.'
        self.start = self.context.index("Schicksal")
        self.end = self.start + len("Schicksal")
        self.baseline = {
            "source_context_unit_ref": "ctx", "source_context": self.context,
            "source_existing_occurrence_ref": "old-occ", "source_occurrence_candidate_ref": "candidate-occ",
            "source_surface": "Schicksal", "speaker": {"role": "narrator", "status": "proposed"},
            "english_on_demand_task_ref": "en-task",
        }
        self.database.execute("INSERT INTO occurrence_spans VALUES(?,?,?,?,?,?,?,?)", (
            "old-occ", "ctx", "surface", self.start, self.end, "Schicksal", QUERY.sha_text("Schicksal"), "proposed"))
        self.add_segment("quoted", 0, self.end + 2, "dwarf")
        self.add_segment("narrated", self.end + 2, len(self.context), "narrator")
        self.database.execute("INSERT INTO source_sentences VALUES(?,?,?,?,?,?)", (
            "sentence", "ctx", 0, len(self.context), self.context, QUERY.sha_text(self.context)))
        self.database.execute("INSERT INTO source_clauses VALUES(?,?,?,?,?,?,?,?)", (
            "clause", "sentence", "ctx", 0, self.end + 2, self.context[:self.end + 2], QUERY.sha_text(self.context[:self.end + 2]), "ambiguous"))

    def tearDown(self):
        self.database.close()

    def add_segment(self, identity, start, end, role):
        exact = self.context[start:end]
        self.database.execute("INSERT INTO discourse_segments VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)", (
            identity, "ctx", "sentence", start, end, exact, QUERY.sha_text(exact), role, "proposed",
            json.dumps([role]), '["cue-source"]', "quoted_speech", 1, "turn", "zarathustra", "reporting_clause"))

    def formula(self, identity, start, end):
        exact = self.context[start:end]
        self.database.execute("INSERT INTO formulas VALUES(?,?,?,?,?,?)", (identity, exact.lower(), 4, 3, 2, "proposed"))
        self.database.execute("INSERT INTO formula_occurrences VALUES(?,?,?,?,?,?,?,?)", (
            identity, f"{identity}:occ", "ctx", start, end, exact, QUERY.sha_text(exact), "proposed"))

    def test_precise_occurrence_uses_quoted_speaker_not_paragraph_narrator(self):
        result = QUERY.enrich_card(self.database, self.baseline, {})
        self.assertEqual(result["speaker"]["role"], "dwarf")
        self.assertEqual(result["speaker_predecessor"]["role"], "narrator")
        self.assertEqual(result["speaker"]["scope"], "occurrence_containing_segment")
        anchor = result["source_occurrence_spans"][0]
        self.assertEqual(self.context[anchor["start_offset"]:anchor["end_offset"]], "Schicksal")
        self.assertEqual(result["source_clauses"][0]["status"], "ambiguous")
        self.assertEqual(result["english_analysis_context"]["execution_status"], "not_executed")

    def test_missing_span_never_falls_back_to_old_speaker(self):
        self.database.execute("DELETE FROM occurrence_spans")
        result = QUERY.enrich_card(self.database, self.baseline, {})
        self.assertEqual(result["speaker"]["role"], "unresolved")
        self.assertEqual(result["source_occurrence_anchor_status"], "deferred")

    def test_crossing_segment_span_is_not_assigned_the_first_voice(self):
        self.database.execute("DELETE FROM discourse_segments")
        self.add_segment("a", 0, self.start + 3, "dwarf")
        self.add_segment("b", self.start + 3, len(self.context), "narrator")
        result = QUERY.enrich_card(self.database, self.baseline, {})
        self.assertEqual(result["speaker"]["role"], "unresolved")
        self.assertEqual(result["speaker"]["status"], "ambiguous")
        self.assertEqual(result["speaker"]["candidates"], ["dwarf", "narrator"])

    def test_source_hash_corruption_fails_closed(self):
        self.database.execute("UPDATE occurrence_spans SET exact_sha256=?", ("0" * 64,))
        with self.assertRaisesRegex(QUERY.ReadingError, "exact source substring"):
            QUERY.enrich_card(self.database, self.baseline, {})

    def test_same_context_formula_is_not_occurrence_membership(self):
        self.formula("nearby", self.context.index("So sprach"), len(self.context))
        self.formula("member", 0, self.end + 2)
        result = QUERY.enrich_card(self.database, self.baseline, {})
        self.assertEqual([row["formula_id"] for row in result["formula_memberships"]], ["member"])
        self.assertEqual([row["formula_id"] for row in result["context_formula_memberships"]], ["nearby"])
        groups = QUERY.group_cards([result], [result], ("speaker", "formula"))
        self.assertEqual([row["formula_id"] for row in groups["by_formula"]], ["member"])

    def test_group_counts_cover_full_set_before_card_limit(self):
        first = QUERY.enrich_card(self.database, self.baseline, {})
        second = {**first, "source_occurrence_candidate_ref": "second"}
        groups = QUERY.group_cards([first, second], [first], ("speaker", "formula"))
        self.assertEqual(groups["by_speaker"][0]["matching_count"], 2)
        self.assertEqual(groups["by_speaker"][0]["returned_occurrence_refs"], ["candidate-occ"])
        counts_only = QUERY.group_cards([first, second], [], ("speaker", "formula"))
        self.assertEqual(counts_only["by_speaker"][0]["matching_count"], 2)
        self.assertEqual(counts_only["by_speaker"][0]["returned_occurrence_refs"], [])

    def test_hypothetical_and_performed_voices_are_separate_groups(self):
        first = QUERY.enrich_card(self.database, self.baseline, {})
        second = {**first, "source_occurrence_candidate_ref": "performed",
                  "speaker": {**first["speaker"], "performed_role": "evil_spirit"}}
        third = {**first, "source_occurrence_candidate_ref": "hypothetical",
                 "speaker": {**first["speaker"], "modality": "hypothetical"}}
        groups = QUERY.group_cards([first, second, third], [first], ("speaker",))
        self.assertEqual(len(groups["by_speaker"]), 3)
        self.assertEqual(sum(row["matching_count"] for row in groups["by_speaker"]), 3)

    def test_private_database_path_keeps_uri_reserved_characters(self):
        baseline = QUERY.load_baseline()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "data#private.sqlite3"
            with sqlite3.connect(path) as db:
                db.execute("CREATE TABLE exact_occurrences(existing_occurrence_ref TEXT)")
                db.execute("CREATE TABLE context_units(language TEXT, witness_order INTEGER)")
            exact, contexts, alignments = baseline.source_rows(path, [])
            self.assertEqual((exact, contexts, dict(alignments)), ({}, {}, {}))

    def test_manifest_missing_is_explicit_unavailable(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(QUERY.ReadingUnavailable):
                QUERY.open_reading_database(Path(directory))

    def test_only_explicit_witness_linebreak_hyphen_is_joined(self):
        text = "Schick¬\nsal Schick-\nsal Schick\nsal"
        found = QUERY.explicit_dehyphenations(text, {"schicksal"})
        self.assertEqual(len(found), 1)
        self.assertEqual(found[0]["exact_text"], "Schick¬\nsal")
        self.assertEqual(found[0]["normalized_form"], "Schicksal")
        operation = found[0]["normalization_operations"][0]
        self.assertEqual(text[operation["start_offset"]:operation["end_offset"]], "¬\n")
        self.assertEqual(QUERY.explicit_dehyphenations(text, {"fate"}), [])

    def test_dehyphenated_candidate_has_new_id_and_no_legacy_occurrence(self):
        self.database.execute("CREATE TABLE contexts(context_unit_ref TEXT,language TEXT,part INTEGER,reading_ref TEXT,witness_order INTEGER,exact_text TEXT,exact_sha256 TEXT)")
        text = "Mein Schick¬\nsal."
        self.database.execute("INSERT INTO contexts VALUES(?,?,?,?,?,?,?)", ("new-context", "de", 3, "p3.r13", 1, text, QUERY.sha_text(text)))
        baseline = QUERY.enrich_card(self.database, {**self.baseline, "source_analysis_form": "schicksal"}, {})
        candidates = QUERY.additional_normalization_candidates(self.database, [baseline], {})
        self.assertEqual(len(candidates), 1)
        self.assertIsNone(candidates[0]["source_existing_occurrence_ref"])
        self.assertFalse(candidates[0]["legacy_occurrence_id_asserted"])
        self.assertTrue(candidates[0]["candidate_id"].startswith("tos.annotation.normalization-search-candidate.sid-"))
        self.assertEqual(candidates[0]["source_occurrence_spans"][0]["exact_text"], "Schick¬\nsal")
        self.assertEqual(candidates[0]["speaker"]["role"], "unresolved")

    def test_owned_path_rejects_escape_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(QUERY.ReadingError):
                QUERY.owned_path(root, "../escape")
            (root / "link").symlink_to(ROOT / "AGENTS.md")
            with self.assertRaises(QUERY.ReadingError):
                QUERY.owned_path(root, "link")


class ProviderBoundaryTests(unittest.TestCase):
    def test_concept_request_uses_parsed_request_and_confines_data_refs(self):
        request = {
            "request_key": "sample",
            "request_version": 7,
            "request_identity_key": "tos.request.sample.sid-xyz",
        }
        original = (
            CONCEPT_BUILDER.REQUEST_REF,
            CONCEPT_BUILDER.PRIVATE_REQUEST,
            CONCEPT_BUILDER.ISSUANCE_REF,
            dict(CONCEPT_BUILDER.OUTPUTS),
        )
        try:
            with patch.object(CONCEPT_BUILDER, "load_json", side_effect=AssertionError("request file was loaded")):
                CONCEPT_BUILDER.configure_request(Path("missing/request.json"), request=request)
            self.assertEqual(CONCEPT_BUILDER.REQUEST_REF, Path("missing/request.json"))
            self.assertEqual(
                CONCEPT_BUILDER.PRIVATE_REQUEST,
                CONCEPT_BUILDER.PRIVATE_ROOT / "requests/sample-v7-xyz.request-analysis.v1.json",
            )
            self.assertEqual(
                CONCEPT_BUILDER.OUTPUTS["concept"],
                CONCEPT_BUILDER.ROUTE / "outputs/sample-v7-xyz/concept-candidate.v1.json",
            )
        finally:
            (CONCEPT_BUILDER.REQUEST_REF, CONCEPT_BUILDER.PRIVATE_REQUEST,
             CONCEPT_BUILDER.ISSUANCE_REF, outputs) = original
            CONCEPT_BUILDER.OUTPUTS = outputs

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            request_path = root / "request.json"
            request_path.write_text("{}", encoding="utf-8")
            seen = {}

            def configure(request_ref, *, request):
                seen.update(request_ref=request_ref, request=request)

            builder = SimpleNamespace(
                PRIVATE_REQUEST=Path("private/request.json"),
                PRIVATE_DB=Path("private/workbench.sqlite3"),
                configure_request=configure,
            )
            private_request, database = CONCEPT_QUERY.request_paths(builder, request_path, root, request)
            self.assertEqual(private_request, root / "private/request.json")
            self.assertEqual(database, root / "private/workbench.sqlite3")
            self.assertEqual(seen, {"request_ref": Path("request.json"), "request": request})

            builder.PRIVATE_REQUEST = Path("../escape.json")
            with self.assertRaisesRegex(CONCEPT_QUERY.SearchError, "data-root-relative"):
                CONCEPT_QUERY.request_paths(builder, request_path, root, request)
            outside = root.parent / "outside.json"
            outside.write_text("{}", encoding="utf-8")
            (root / "link.json").symlink_to(outside)
            with self.assertRaisesRegex(CONCEPT_QUERY.SearchError, "escapes"):
                CONCEPT_QUERY.data_path(root, "link.json")

    def test_reading_query_never_executes_a_data_root_baseline_sentinel(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture = reading_provider_fixture(root)
            software_root = root / "software"
            baseline_path = software_root / QUERY.BASELINE_QUERY_REF
            baseline_path.parent.mkdir(parents=True)
            baseline_path.write_text(
                """
def build_result(query, language, request_path, include_semantic_neighbors, limit, *, data_root=None):
    if data_root is None:
        raise RuntimeError('missing explicit source data root')
    return {
        'software_baseline': True,
        'search_result_id': 'software-baseline',
        'provenance': {'source_manifest_ref': 'source-manifest.json'},
        'coverage': {'total_source_results': 0},
        'results': [],
    }
""",
                encoding="utf-8",
            )
            schema_path = software_root / QUERY.SCHEMA_REF
            schema_path.parent.mkdir(parents=True)
            schema_path.write_text("{}\n", encoding="utf-8")
            sentinel = fixture["source_root"] / QUERY.BASELINE_QUERY_REF
            sentinel.parent.mkdir(parents=True)
            sentinel.write_text("raise RuntimeError('DATA_ROOT_EXECUTABLE_SENTINEL_LOADED')\n", encoding="utf-8")
            request_path = fixture["source_root"] / "request.json"
            request_path.write_text("{}\n", encoding="utf-8")

            with patch.object(QUERY, "REPO", software_root), \
                    patch.object(QUERY, "alignment_index", return_value={}), \
                    patch.object(QUERY, "additional_normalization_candidates", return_value=[]), \
                    patch.object(QUERY, "group_cards", return_value={}):
                result = QUERY.build_result(
                    "Schicksal", "ru", limit=0, request_path=request_path,
                    source_root=fixture["source_root"], analysis_root=fixture["analysis_root"],
                )

            self.assertTrue(result["software_baseline"])
            self.assertEqual(result["provenance"]["concept_predecessor"]["source_manifest_ref"], "source-manifest.json")

    def test_v1_data_fixities_reject_corruption_but_allow_compatible_software_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = reading_provider_fixture(Path(directory))
            database, _provenance = QUERY.open_reading_database(
                fixture["analysis_root"], fixture["source_root"]
            )
            database.close()

            original_companion = fixture["companion"].read_bytes()
            fixture["companion"].write_bytes(b"corrupt companion\n")
            with self.assertRaisesRegex(QUERY.ReadingError, "companion drift"):
                QUERY.open_reading_database(fixture["analysis_root"], fixture["source_root"])
            fixture["companion"].write_bytes(original_companion)

            original_policy = fixture["policy"].read_bytes()
            fixture["policy"].write_bytes(b"{\"status\":\"changed\"}\n")
            with self.assertRaisesRegex(QUERY.ReadingError, "input drift"):
                QUERY.open_reading_database(fixture["analysis_root"], fixture["source_root"])
            fixture["policy"].write_bytes(original_policy)

            original_database = fixture["database"].read_bytes()
            fixture["database"].write_bytes(b"corrupt database\n")
            with self.assertRaisesRegex(QUERY.ReadingError, "fixity mismatch"):
                QUERY.open_reading_database(fixture["analysis_root"], fixture["source_root"])
            fixture["database"].write_bytes(original_database)

    def test_reading_manifest_authority_boundary_rejects_non_candidate_state(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = reading_provider_fixture(Path(directory))
            baseline = json.loads(fixture["manifest"].read_text(encoding="utf-8"))
            for field, value in (
                ("accepted", True),
                ("human_review", True),
                ("canon_effect", True),
                ("publication_posture", "public_bundle"),
            ):
                with self.subTest(field=field):
                    manifest = dict(baseline)
                    manifest[field] = value
                    fixture["manifest"].write_text(json.dumps(manifest), encoding="utf-8")
                    with self.assertRaisesRegex(QUERY.ReadingError, "authority boundary"):
                        QUERY.open_reading_database(fixture["analysis_root"], fixture["source_root"])
            fixture["manifest"].write_text(json.dumps(baseline), encoding="utf-8")


if __name__ == "__main__":
    unittest.main()

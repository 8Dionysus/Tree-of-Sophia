from __future__ import annotations

import importlib.util
import hashlib
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
PLAN_PATH = (
    ROOT / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/index-plan.v1.json"
)
PROJECTION_PATH = (
    ROOT / "ToS/derived-exports/lexical-search/"
    "zarathustra-dta-first-editions-parts-1-4-v1.min.json"
)
MORPHOLOGY_PLAN_PATH = (
    ROOT / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "morphology-evaluation-plan.v1.json"
)
MORPHOLOGY_RECEIPT_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "morphology-input-receipt.v1.json"
)
MORPHOLOGY_RESULT_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "morphology-census-result.a-dwdsmor-open-0.18.0.v1.json"
)


def _load_module(name: str, relative_path: str):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative_path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BUILDER = _load_module(
    "tos_zarathustra_lexical_builder",
    "scripts/build_zarathustra_lexical_index.py",
)
VALIDATOR = _load_module(
    "tos_zarathustra_lexical_validator",
    "scripts/validate_zarathustra_lexical_index.py",
)
MORPHOLOGY_BUILDER = _load_module(
    "tos_zarathustra_morphology_input_builder",
    "scripts/build_zarathustra_morphology_input.py",
)
MORPHOLOGY_RESULT_RECORDER = _load_module(
    "tos_zarathustra_morphology_result_recorder",
    "scripts/record_zarathustra_morphology_census_result.py",
)


class ZarathustraLexicalIndexTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = json.loads(PLAN_PATH.read_text(encoding="utf-8"))
        cls.projection = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        cls.morphology_plan = json.loads(
            MORPHOLOGY_PLAN_PATH.read_text(encoding="utf-8")
        )
        cls.morphology_receipt = json.loads(
            MORPHOLOGY_RECEIPT_PATH.read_text(encoding="utf-8")
        )
        cls.morphology_result = json.loads(
            MORPHOLOGY_RESULT_PATH.read_text(encoding="utf-8")
        )
        cls.validation = VALIDATOR.validate()

    def test_unicode_tokenizer_preserves_exact_forms_and_internal_joiners(self) -> None:
        text = "Über-Mensch O’Connor Straße 123 -- Wort"
        tokens = [
            token
            for _, _, token in BUILDER.iter_word_spans(text, {"-", "'", "’", "‐", "‑"})
        ]
        self.assertEqual(
            ["Über-Mensch", "O’Connor", "Straße", "Wort"],
            tokens,
        )
        self.assertEqual("über-mensch", BUILDER.normalize_form(tokens[0]))
        self.assertEqual("strasse", BUILDER.normalize_form(tokens[2]))
        self.assertEqual("Straße", tokens[2])

    def test_release_safe_projection_closes_without_local_payload(self) -> None:
        self.assertEqual("ok", self.validation["status"])
        self.assertFalse(self.validation["local_database_verified"])
        self.assertEqual(
            self.projection["summary"],
            self.validation["summary"],
        )

    def test_projection_is_whole_work_mechanical_observation(self) -> None:
        self.assertEqual(
            "generated_from_source",
            self.projection["generated_or_authored"],
        )
        self.assertEqual(
            self.projection["generator_ref"],
            self.projection["builder"]["surface"],
        )
        summary = self.projection["summary"]
        self.assertEqual(4, summary["source_item_count"])
        self.assertEqual(506, summary["body_page_count"])
        self.assertEqual(213, summary["section_count"])
        self.assertEqual(86287, summary["token_occurrence_count"])
        self.assertEqual(11352, summary["exact_form_row_count"])
        self.assertEqual(10113, summary["normalized_form_hash_count"])
        self.assertEqual(0, summary["semantic_fields_populated"])
        self.assertEqual(
            summary["token_occurrence_count"],
            sum(
                item["token_occurrence_count"]
                for item in self.projection["source_items"]
            ),
        )

    def test_tracked_form_rows_carry_hashes_and_resource_hits_only(self) -> None:
        expected_keys = {
            "form_key",
            "exact_form_sha256",
            "normalized_form_sha256",
            "occurrence_count",
            "source_editorial_occurrence_count",
            "unsectioned_occurrence_count",
            "source_items",
        }
        expected_item_keys = {
            "item_ref",
            "occurrence_count",
            "page_hits",
            "section_hits",
        }
        for row in self.projection["form_rows"]:
            self.assertEqual(expected_keys, set(row))
            self.assertEqual(
                f"lexical-form:sha256:{row['exact_form_sha256']}",
                row["form_key"],
            )
            self.assertEqual(64, len(row["exact_form_sha256"]))
            self.assertEqual(64, len(row["normalized_form_sha256"]))
            for item in row["source_items"]:
                self.assertEqual(expected_item_keys, set(item))
                self.assertTrue(item["page_hits"])
                self.assertTrue(
                    all(
                        set(hit) == {"resource_id", "occurrence_count"}
                        for hit in item["page_hits"] + item["section_hits"]
                    )
                )
        exposure = self.projection["content_exposure"]
        self.assertFalse(exposure["tracked_exact_strings"])
        self.assertFalse(exposure["tracked_sequence"])
        self.assertFalse(exposure["tracked_context"])
        self.assertFalse(exposure["tracked_occurrence_positions"])
        self.assertTrue(exposure["dictionary_recovery_possible"])
        self.assertFalse(exposure["confidentiality_claimed"])

    def test_local_query_capabilities_and_semantic_blockers_are_distinct(self) -> None:
        probes = self.projection["local_projection_receipt"]["query_probes"]
        for field in (
            "exact_form",
            "normalized_form",
            "prefix",
            "phrase",
            "section",
            "page",
            "language",
            "edition",
        ):
            self.assertEqual("passed", probes[field]["status"])
            self.assertGreater(probes[field]["result_count"], 0)
        self.assertEqual(
            "blocked-not-materialized",
            probes["lemma"]["status"],
        )
        self.assertEqual(
            "blocked-not-materialized",
            probes["sign_candidate"]["status"],
        )
        self.assertEqual("not-applicable", probes["translation"]["status"])
        self.assertFalse(self.projection["semantic_boundary"]["creates_lexeme"])
        self.assertFalse(self.projection["semantic_boundary"]["creates_lemma"])
        self.assertFalse(self.projection["semantic_boundary"]["creates_sign_candidate"])
        self.assertFalse(
            self.projection["semantic_boundary"]["opens_initial_sign_packet"]
        )

    def test_source_bearing_database_route_is_gitignored(self) -> None:
        local_ref = self.plan["local_projection"]["relative_path"]
        result = subprocess.run(
            ["git", "check-ignore", "-q", local_ref],
            cwd=ROOT,
            check=False,
        )
        self.assertEqual(0, result.returncode)
        self.assertIn("local-content", Path(local_ref).parts)
        self.assertEqual(
            "gitignored-local-only",
            self.plan["local_projection"]["storage_posture"],
        )

    def test_rights_and_publication_remain_blocked(self) -> None:
        self.assertEqual(
            {"licensed"},
            {
                item["rights_assessment_status"]
                for item in self.projection["source_items"]
            },
        )
        self.assertEqual(
            {"unreviewed"},
            {item["rights_review_status"] for item in self.projection["source_items"]},
        )
        self.assertEqual(
            "blocked",
            self.projection["rights_and_visibility"]["future_site_route"],
        )
        self.assertTrue(
            self.projection["rights_and_visibility"][
                "rights_review_required_before_public_route"
            ]
        )

    def test_morphology_plan_and_receipt_close_over_exact_lexical_floor(self) -> None:
        plan_schema = json.loads(
            (ROOT / "ToS/contracts/morphology-evaluation-plan.schema.json").read_text(
                encoding="utf-8"
            )
        )
        receipt_schema = json.loads(
            (ROOT / "ToS/contracts/morphology-input-receipt.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            [],
            list(Draft202012Validator(plan_schema).iter_errors(self.morphology_plan)),
        )
        self.assertEqual(
            [],
            list(
                Draft202012Validator(receipt_schema).iter_errors(
                    self.morphology_receipt
                )
            ),
        )
        source = self.morphology_plan["source_lexical_index"]
        receipt = self.morphology_receipt
        self.assertEqual(11352, source["exact_form_row_count"])
        self.assertEqual(86287, source["token_occurrence_count"])
        self.assertEqual(
            source["exact_form_row_count"],
            receipt["summary"]["exact_form_row_count"],
        )
        self.assertEqual(
            source["token_occurrence_count"],
            receipt["summary"]["token_occurrence_count"],
        )
        self.assertEqual(
            hashlib.sha256(MORPHOLOGY_PLAN_PATH.read_bytes()).hexdigest(),
            receipt["plan_sha256"],
        )
        self.assertEqual(
            hashlib.sha256((ROOT / receipt["generator_ref"]).read_bytes()).hexdigest(),
            receipt["generator_sha256"],
        )
        self.assertEqual(
            source["tracked_projection_sha256"],
            receipt["source_projection"]["tracked_projection_sha256"],
        )
        self.assertTrue(self.morphology_plan["frozen_before_variant_outputs"])

    def test_lexical_provenance_preserves_one_supersession_lineage(self) -> None:
        provenance_path = ROOT / self.plan["provenance_ref"]
        events = VALIDATOR._load_provenance(provenance_path)
        latest = VALIDATOR._latest_provenance_event(events)
        self.assertEqual(2, len(events))
        self.assertEqual(2, latest["event_version"])
        self.assertEqual(
            VALIDATOR.BASE_PROVENANCE_EVENT_REF,
            latest["supersedes_event_ref"],
        )

    def test_morphology_input_stays_private_and_does_not_open_abc_or_gold(self) -> None:
        local_ref = self.morphology_plan["a_census"]["local_packet"]["relative_path"]
        result = subprocess.run(
            ["git", "check-ignore", "-q", local_ref],
            cwd=ROOT,
            check=False,
        )
        self.assertEqual(0, result.returncode)
        self.assertFalse(
            self.morphology_receipt["content_exposure"]["tracked_exact_strings"]
        )
        self.assertEqual(
            "blocked-not-materialized",
            self.morphology_plan["abc_followup"]["status"],
        )
        self.assertFalse(self.morphology_plan["abc_followup"]["human_work_scheduled"])
        self.assertFalse(self.morphology_plan["semantic_boundary"]["creates_lemma"])
        self.assertFalse(self.morphology_plan["semantic_boundary"]["creates_lexeme"])

    def test_morphology_result_is_text_free_and_closes_over_raw_aggregates(
        self,
    ) -> None:
        result_schema = json.loads(
            (
                ROOT / "ToS/contracts/morphology-census-result-receipt.schema.json"
            ).read_text(encoding="utf-8")
        )
        result = self.morphology_result
        self.assertEqual(
            [],
            list(Draft202012Validator(result_schema).iter_errors(result)),
        )
        self.assertEqual(
            hashlib.sha256(MORPHOLOGY_PLAN_PATH.read_bytes()).hexdigest(),
            result["plan"]["sha256"],
        )
        self.assertEqual(
            hashlib.sha256(
                (ROOT / result["generator"]["ref"]).read_bytes()
            ).hexdigest(),
            result["generator"]["sha256"],
        )
        coverage = result["coverage"]
        self.assertEqual(6610, coverage["covered_type_count"])
        self.assertEqual(75872, coverage["covered_token_count"])
        self.assertEqual(4742, coverage["unknown_type_count"])
        self.assertEqual(10415, coverage["unknown_token_count"])
        self.assertEqual(
            result["source_input"]["exact_form_row_count"],
            coverage["covered_type_count"] + coverage["unknown_type_count"],
        )
        self.assertEqual(
            result["source_input"]["token_occurrence_count"],
            coverage["covered_token_count"] + coverage["unknown_token_count"],
        )
        distributions = result["distributions"]
        self.assertEqual(
            result["source_input"]["exact_form_row_count"],
            sum(distributions["lemma_analysis_count"].values()),
        )
        self.assertEqual(
            result["source_input"]["exact_form_row_count"],
            sum(distributions["root_analysis_count"].values()),
        )
        self.assertEqual(
            distributions["lemma_analysis_total"],
            sum(distributions["provider_pos"].values()),
        )
        self.assertEqual(
            distributions["lemma_analysis_total"],
            sum(distributions["provider_category"].values()),
        )

    def test_morphology_result_preserves_authority_and_followup_boundaries(
        self,
    ) -> None:
        result = self.morphology_result
        self.assertFalse(
            result["mechanical_unknown_residue"]["triggers_contextual_followup"]
        )
        self.assertFalse(
            result["mechanical_unknown_residue"]["source_strings_included"]
        )
        self.assertEqual(
            "unreviewed-mechanical-aggregation",
            result["mechanical_unknown_residue"]["review_status"],
        )
        self.assertFalse(result["followup"]["b_acquired"])
        self.assertFalse(result["followup"]["c_acquired"])
        self.assertFalse(result["followup"]["human_work_scheduled"])
        self.assertFalse(result["followup"]["mechanical_residue_is_reviewed_residue"])
        self.assertEqual(
            "unmeasured-no-german-competent-gold",
            result["accuracy"]["status"],
        )
        self.assertEqual(0, result["accuracy"]["accepted_lemma_count"])
        self.assertTrue(
            all(value is False for value in result["semantic_boundary"].values())
        )
        visibility = result["rights_and_visibility"]
        self.assertFalse(visibility["tracked_receipt_contains_source_strings"])
        self.assertFalse(visibility["tracked_receipt_contains_provider_lemma_strings"])
        self.assertFalse(visibility["source_payload_publication_authorized"])
        self.assertFalse(visibility["raw_output_publication_authorized"])
        self.assertFalse(visibility["tracked_receipt_publication_authorized"])
        self.assertFalse(Path(result["private_run"]["relative_ref"]).is_absolute())

    def test_morphology_result_schema_rejects_source_like_aggregate_keys(
        self,
    ) -> None:
        schema = json.loads(
            (
                ROOT / "ToS/contracts/morphology-census-result-receipt.schema.json"
            ).read_text(encoding="utf-8")
        )
        contaminated = json.loads(json.dumps(self.morphology_result))
        contaminated["distributions"]["provider_pos"]["Übermensch"] = 1
        errors = list(Draft202012Validator(schema).iter_errors(contaminated))
        self.assertTrue(errors)

    def test_raw_result_inspector_recomputes_without_returning_source_strings(
        self,
    ) -> None:
        rows = []
        fixtures = [
            ("bekannt", 4, [{"pos": "V", "category": None}], []),
            ("unbekannt", 2, [], []),
        ]
        for surface, count, lemma, root in fixtures:
            digest = hashlib.sha256(surface.encode("utf-8")).hexdigest()
            rows.append(
                {
                    "schema_version": "tos_dwdsmor_analysis_row_v1",
                    "form_key": f"lexical-form:sha256:{digest}",
                    "exact_form": surface,
                    "exact_form_sha256": digest,
                    "normalized_form_sha256": "a" * 64,
                    "occurrence_count": count,
                    "input_preserved": True,
                    "provider": MORPHOLOGY_RESULT_RECORDER.EXPECTED_PROVIDER,
                    "lemma_analyses": lemma,
                    "root_analyses": root,
                    "lemma_analysis_count": len(lemma),
                    "root_analysis_count": len(root),
                    "unknown": not lemma,
                    "authority": "unreviewed-provider-candidate",
                }
            )
        rows.sort(key=lambda row: (row["exact_form_sha256"], row["exact_form"]))
        with self.subTest("aggregate"):
            with tempfile.TemporaryDirectory() as temporary:
                raw_path = Path(temporary) / "raw.jsonl"
                raw_path.write_bytes(
                    b"".join(
                        MORPHOLOGY_RESULT_RECORDER.canonical_line(row) for row in rows
                    )
                )
                aggregate = MORPHOLOGY_RESULT_RECORDER.inspect_raw_output(raw_path)
        self.assertEqual(2, aggregate["row_count"])
        self.assertEqual(6, aggregate["token_occurrence_count"])
        self.assertEqual(1, aggregate["covered_type_count"])
        self.assertEqual(1, aggregate["unknown_type_count"])
        self.assertEqual(2, aggregate["unknown_token_count"])
        self.assertEqual(1, aggregate["lemma_analysis_total"])
        self.assertNotIn("bekannt", json.dumps(aggregate, ensure_ascii=False))
        self.assertNotIn("unbekannt", json.dumps(aggregate, ensure_ascii=False))

    def test_morphology_private_rows_preserve_surface_and_frozen_order(self) -> None:
        surfaces = ["Über-Mensch", "Straße"]
        rows = []
        for surface, count in zip(surfaces, (2, 1), strict=True):
            digest = hashlib.sha256(surface.encode("utf-8")).hexdigest()
            rows.append(
                {
                    "form_key": f"lexical-form:sha256:{digest}",
                    "exact_form": surface,
                    "exact_form_sha256": digest,
                    "normalized_form_sha256": hashlib.sha256(
                        surface.casefold().encode("utf-8")
                    ).hexdigest(),
                    "occurrence_count": count,
                }
            )
        rows.sort(key=lambda row: (row["exact_form_sha256"], row["exact_form"]))
        packet, summary = MORPHOLOGY_BUILDER.build_packet(rows)
        decoded = [json.loads(line) for line in packet.decode("utf-8").splitlines()]
        self.assertEqual(
            [row["exact_form"] for row in rows],
            [row["exact_form"] for row in decoded],
        )
        self.assertEqual(2, summary["exact_form_row_count"])
        self.assertEqual(3, summary["token_occurrence_count"])
        self.assertEqual(1, summary["joiner_form_count"])


if __name__ == "__main__":
    unittest.main()

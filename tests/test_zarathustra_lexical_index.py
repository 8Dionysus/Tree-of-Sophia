from __future__ import annotations

import importlib.util
import json
import subprocess
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PLAN_PATH = (
    ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/index-plan.v1.json"
)
PROJECTION_PATH = (
    ROOT
    / "ToS/derived-exports/lexical-search/"
    "zarathustra-dta-first-editions-parts-1-4-v1.min.json"
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


class ZarathustraLexicalIndexTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = json.loads(PLAN_PATH.read_text(encoding="utf-8"))
        cls.projection = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        cls.validation = VALIDATOR.validate()

    def test_unicode_tokenizer_preserves_exact_forms_and_internal_joiners(self) -> None:
        text = "Über-Mensch O’Connor Straße 123 -- Wort"
        tokens = [
            token
            for _, _, token in BUILDER.iter_word_spans(
                text, {"-", "'", "’", "‐", "‑"}
            )
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
        self.assertFalse(
            self.projection["semantic_boundary"]["creates_sign_candidate"]
        )
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
            {"conflicting_evidence"},
            {
                item["rights_assessment_status"]
                for item in self.projection["source_items"]
            },
        )
        self.assertEqual(
            {"unreviewed"},
            {
                item["rights_review_status"]
                for item in self.projection["source_items"]
            },
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


if __name__ == "__main__":
    unittest.main()

from __future__ import annotations

import importlib.util
import hashlib
import json
import sqlite3
import subprocess
import tempfile
import unittest
from fractions import Fraction
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
MORPHOLOGY_CONTEXT_PLAN_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "morphology-contextual-episode.selected-form-b.v1.json"
)
MORPHOLOGY_CONTEXT_RECEIPT_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "morphology-contextual-episode.selected-form-b.receipt.v1.json"
)
MORPHOLOGY_CONTEXT_PROVENANCE_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "provenance.morphology-contextual-episode.selected-form-b.v1.jsonl"
)
MORPHOLOGY_CONTEXT_ADMISSION_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "morphology-contextual-episode.selected-form-b.artifact-admission.v1.json"
)
MORPHOLOGY_CONTEXT_ADMISSION_PROVENANCE_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "provenance.morphology-contextual-episode.selected-form-b."
    "artifact-admission.v1.jsonl"
)
MORPHOLOGY_CONTEXT_RESULT_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "morphology-contextual-episode.selected-form-b.result.v1.json"
)
MORPHOLOGY_CONTEXT_RESULT_PROVENANCE_PATH = MORPHOLOGY_PLAN_PATH.with_name(
    "provenance.morphology-contextual-episode.selected-form-b.result.v1.jsonl"
)
RECURRENCE_PLAN_PATH = PLAN_PATH.with_name("recurrence-plan.v1.json")
RECURRENCE_PROJECTION_PATH = (
    ROOT / "ToS/derived-exports/lexical-search/"
    "zarathustra-dta-first-editions-parts-1-4-recurrence-v1.min.json"
)
RECURRENCE_PROVENANCE_PATH = PLAN_PATH.with_name("recurrence-provenance.jsonl")
USAGE_CONTEXT_PLAN_PATH = PLAN_PATH.with_name("usage-context-plan.v1.json")
USAGE_CONTEXT_RECEIPT_PATH = PLAN_PATH.with_name("usage-context-receipt.v1.json")
USAGE_CONTEXT_PROVENANCE_PATH = PLAN_PATH.with_name(
    "usage-context-provenance.jsonl"
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
MORPHOLOGY_CONTEXT_BUILDER = _load_module(
    "tos_zarathustra_morphology_context_builder",
    "scripts/build_zarathustra_morphology_context_packet.py",
)
MORPHOLOGY_CONTEXT_RESULT_RECORDER = _load_module(
    "tos_zarathustra_morphology_context_result_recorder",
    "scripts/record_zarathustra_morphology_contextual_result.py",
)
RECURRENCE_BUILDER = _load_module(
    "tos_zarathustra_recurrence_projection_builder",
    "scripts/build_zarathustra_recurrence_projection.py",
)
USAGE_CONTEXT_BUILDER = _load_module(
    "tos_zarathustra_usage_context_builder",
    "scripts/build_zarathustra_usage_context_bundle.py",
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
        cls.morphology_context_plan = json.loads(
            MORPHOLOGY_CONTEXT_PLAN_PATH.read_text(encoding="utf-8")
        )
        cls.morphology_context_receipt = json.loads(
            MORPHOLOGY_CONTEXT_RECEIPT_PATH.read_text(encoding="utf-8")
        )
        cls.morphology_context_provenance = json.loads(
            MORPHOLOGY_CONTEXT_PROVENANCE_PATH.read_text(encoding="utf-8")
        )
        cls.morphology_context_admission = json.loads(
            MORPHOLOGY_CONTEXT_ADMISSION_PATH.read_text(encoding="utf-8")
        )
        cls.morphology_context_admission_provenance = json.loads(
            MORPHOLOGY_CONTEXT_ADMISSION_PROVENANCE_PATH.read_text(
                encoding="utf-8"
            )
        )
        cls.morphology_context_result = json.loads(
            MORPHOLOGY_CONTEXT_RESULT_PATH.read_text(encoding="utf-8")
        )
        cls.morphology_context_result_provenance = json.loads(
            MORPHOLOGY_CONTEXT_RESULT_PROVENANCE_PATH.read_text(
                encoding="utf-8"
            )
        )
        cls.recurrence_plan = json.loads(
            RECURRENCE_PLAN_PATH.read_text(encoding="utf-8")
        )
        cls.recurrence_projection = json.loads(
            RECURRENCE_PROJECTION_PATH.read_text(encoding="utf-8")
        )
        cls.recurrence_provenance = json.loads(
            RECURRENCE_PROVENANCE_PATH.read_text(encoding="utf-8")
        )
        cls.usage_context_plan = json.loads(
            USAGE_CONTEXT_PLAN_PATH.read_text(encoding="utf-8")
        )
        cls.usage_context_receipt = json.loads(
            USAGE_CONTEXT_RECEIPT_PATH.read_text(encoding="utf-8")
        )
        cls.usage_context_provenance = json.loads(
            USAGE_CONTEXT_PROVENANCE_PATH.read_text(encoding="utf-8")
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

    def test_local_database_verification_checks_fixity_and_schema(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_root:
            local_root = Path(temporary_root)
            database_path = local_root / "local-content" / "lexical.db"
            database_path.parent.mkdir(parents=True)
            plan_id = "lexical-plan:test"
            plan_sha256 = "a" * 64
            with sqlite3.connect(database_path) as connection:
                connection.execute(
                    "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
                )
                connection.executemany(
                    "INSERT INTO metadata(key, value) VALUES (?, ?)",
                    [("plan_id", plan_id), ("plan_sha256", plan_sha256)],
                )
                connection.commit()
            receipt = {
                "relative_path": "local-content/lexical.db",
                "database_bytes": database_path.stat().st_size,
                "database_sha256": hashlib.sha256(
                    database_path.read_bytes()
                ).hexdigest(),
                "table_counts": {"metadata": 2},
            }
            projection = {
                "plan_id": plan_id,
                "plan_sha256": plan_sha256,
                "local_projection_receipt": receipt,
            }

            VALIDATOR._validate_local_database(local_root, projection)

            database_path.write_bytes(database_path.read_bytes() + b"drift")
            with self.assertRaisesRegex(
                VALIDATOR.LexicalIndexValidationError,
                "local database byte-size drift",
            ):
                VALIDATOR._validate_local_database(local_root, projection)

            database_path.unlink()
            with self.assertRaisesRegex(
                VALIDATOR.LexicalIndexValidationError,
                "local lexical database is absent",
            ):
                VALIDATOR._validate_local_database(local_root, projection)

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

    def test_recurrence_plan_projection_and_provenance_close_without_source_text(
        self,
    ) -> None:
        plan_schema = json.loads(
            (ROOT / "ToS/contracts/lexical-recurrence-plan.schema.json").read_text(
                encoding="utf-8"
            )
        )
        projection_schema = json.loads(
            (
                ROOT / "ToS/contracts/lexical-recurrence-projection.schema.json"
            ).read_text(encoding="utf-8")
        )
        provenance_schema = json.loads(
            (ROOT / "ToS/contracts/provenance-event.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            [],
            list(
                Draft202012Validator(plan_schema).iter_errors(
                    self.recurrence_plan
                )
            ),
        )
        self.assertEqual(
            [],
            list(
                Draft202012Validator(projection_schema).iter_errors(
                    self.recurrence_projection
                )
            ),
        )
        self.assertEqual(
            [],
            list(
                Draft202012Validator(provenance_schema).iter_errors(
                    self.recurrence_provenance
                )
            ),
        )
        projection = self.recurrence_projection
        self.assertEqual(
            hashlib.sha256(RECURRENCE_PLAN_PATH.read_bytes()).hexdigest(),
            projection["plan"]["sha256"],
        )
        self.assertEqual(
            hashlib.sha256(PROJECTION_PATH.read_bytes()).hexdigest(),
            projection["source_projection"]["sha256"],
        )
        self.assertEqual(
            hashlib.sha256(
                (ROOT / projection["generator"]["ref"]).read_bytes()
            ).hexdigest(),
            projection["generator"]["sha256"],
        )
        self.assertEqual(11352, projection["summary"]["row_count"])
        self.assertEqual(86287, projection["summary"]["token_occurrence_count"])
        self.assertEqual(1017, projection["summary"]["all_four_parts_form_count"])
        self.assertEqual(7604, projection["summary"]["single_part_form_count"])
        self.assertEqual(6529, projection["summary"]["singleton_form_count"])
        self.assertEqual(0, projection["summary"]["semantic_fields_populated"])
        self.assertFalse(projection["content_exposure"]["tracked_exact_strings"])
        self.assertFalse(
            projection["semantic_boundary"]["creates_sign_candidate"]
        )
        self.assertFalse(
            projection["semantic_boundary"]["opens_initial_sign_packet"]
        )
        self.assertFalse(projection["semantic_boundary"]["opens_human_backlog"])

    def test_recurrence_control_forms_preserve_manually_checked_tuples(self) -> None:
        by_digest = {
            row["exact_form_sha256"]: row
            for row in self.recurrence_projection["rows"]
        }
        expected = {
            # Zarathustra
            "9f1d4250b0115ee2a8bbc876f90f2048159bafd860d61a4fd1de3df44ee0aa55": (
                527,
                4,
                110,
                256,
                144983,
            ),
            # Übermensch
            "b62f170c6421a0762ff1f51e2924d073f5a9f873a40259c97a56be7540d2c496": (
                14,
                4,
                10,
                10,
                342599,
            ),
            # Untergang
            "896135f538a6ff8b20075ded163e029f713adc024955ef670120305a35a29cbd": (
                13,
                3,
                7,
                9,
                617324,
            ),
            # Zeichen
            "e6a8d073a52678889e0cf0edee1064db6450e98d9d6802449644245e83d1ec5f": (
                23,
                4,
                15,
                18,
                220802,
            ),
        }
        for digest, values in expected.items():
            with self.subTest(digest=digest):
                row = by_digest[digest]
                self.assertEqual(
                    values,
                    (
                        row["occurrence_count"],
                        row["part_range"],
                        row["section_range"],
                        row["page_range"],
                        row["part_dp_millionths"],
                    ),
                )

    def test_recurrence_dp_is_independently_recomputed_from_part_counts(self) -> None:
        part_tokens = {
            item["item_ref"]: item["token_occurrence_count"]
            for item in self.projection["source_items"]
        }
        total_tokens = sum(part_tokens.values())
        source_by_digest = {
            row["exact_form_sha256"]: row for row in self.projection["form_rows"]
        }
        recurrence_by_digest = {
            row["exact_form_sha256"]: row
            for row in self.recurrence_projection["rows"]
        }

        def round_millionths(value: Fraction) -> int:
            scaled = value * 1_000_000
            quotient, remainder = divmod(scaled.numerator, scaled.denominator)
            if remainder * 2 > scaled.denominator or (
                remainder * 2 == scaled.denominator and quotient % 2
            ):
                quotient += 1
            return quotient

        for digest in (
            "9f1d4250b0115ee2a8bbc876f90f2048159bafd860d61a4fd1de3df44ee0aa55",
            "b62f170c6421a0762ff1f51e2924d073f5a9f873a40259c97a56be7540d2c496",
            "896135f538a6ff8b20075ded163e029f713adc024955ef670120305a35a29cbd",
        ):
            source_row = source_by_digest[digest]
            counts = {item_ref: 0 for item_ref in part_tokens}
            for hit in source_row["source_items"]:
                counts[hit["item_ref"]] = hit["occurrence_count"]
            observed_total = source_row["occurrence_count"]
            dp = sum(
                abs(
                    Fraction(part_tokens[item_ref], total_tokens)
                    - Fraction(counts[item_ref], observed_total)
                )
                for item_ref in part_tokens
            ) / 2
            self.assertEqual(
                round_millionths(dp),
                recurrence_by_digest[digest]["part_dp_millionths"],
            )

    def test_recurrence_contract_rejects_semantic_score_or_source_surface(
        self,
    ) -> None:
        schema = json.loads(
            (
                ROOT / "ToS/contracts/lexical-recurrence-projection.schema.json"
            ).read_text(encoding="utf-8")
        )
        for field, value in (("sign_score", 0.9), ("exact_form", "Übermensch")):
            contaminated = json.loads(json.dumps(self.recurrence_projection))
            contaminated["rows"][0][field] = value
            with self.subTest(field=field):
                self.assertTrue(
                    list(Draft202012Validator(schema).iter_errors(contaminated))
                )

    def test_recurrence_rounding_and_dp_controls_are_exact(self) -> None:
        self.assertEqual(
            0,
            RECURRENCE_BUILDER.round_fraction_ties_to_even(Fraction(0), 1_000_000),
        )
        self.assertEqual(
            500000,
            RECURRENCE_BUILDER.round_fraction_ties_to_even(
                Fraction(1, 2), 1_000_000
            ),
        )
        self.assertEqual(
            990000,
            RECURRENCE_BUILDER.round_fraction_ties_to_even(
                Fraction(99, 100), 1_000_000
            ),
        )

    def test_usage_context_release_receipt_closes_without_local_source(self) -> None:
        plan_schema = json.loads(
            (
                ROOT / "ToS/contracts/lexical-usage-context-plan.schema.json"
            ).read_text(encoding="utf-8")
        )
        receipt_schema = json.loads(
            (
                ROOT / "ToS/contracts/lexical-usage-context-receipt.schema.json"
            ).read_text(encoding="utf-8")
        )
        provenance_schema = json.loads(
            (ROOT / "ToS/contracts/provenance-event.schema.json").read_text(
                encoding="utf-8"
            )
        )
        for label, schema, payload in (
            ("plan", plan_schema, self.usage_context_plan),
            ("receipt", receipt_schema, self.usage_context_receipt),
            ("provenance", provenance_schema, self.usage_context_provenance),
        ):
            with self.subTest(label=label):
                self.assertEqual(
                    [],
                    list(Draft202012Validator(schema).iter_errors(payload)),
                )
        receipt = self.usage_context_receipt
        self.assertEqual(
            hashlib.sha256(USAGE_CONTEXT_PLAN_PATH.read_bytes()).hexdigest(),
            receipt["plan"]["sha256"],
        )
        self.assertEqual(
            hashlib.sha256(
                (ROOT / receipt["generator"]["ref"]).read_bytes()
            ).hexdigest(),
            receipt["generator"]["sha256"],
        )
        self.assertEqual(
            receipt["summary"],
            self.validation["usage_context"]["summary"],
        )
        self.assertFalse(
            self.validation["usage_context"]["local_bundle_verified"]
        )
        tracked_bytes = (
            USAGE_CONTEXT_RECEIPT_PATH.read_bytes()
            + USAGE_CONTEXT_PROVENANCE_PATH.read_bytes()
        )
        for source_row_key in (
            b'"target_exact_form":',
            b'"left_exact_tokens":',
            b'"right_exact_tokens":',
            b'"occurrence_id":',
            b'"token_ordinal":',
            b'"text_node_path":',
            b'"start_offset":',
            b'"end_offset":',
        ):
            with self.subTest(source_row_key=source_row_key):
                self.assertNotIn(source_row_key, tracked_bytes)

    def test_usage_context_is_complete_question_scoped_nonsemantic_control(
        self,
    ) -> None:
        plan = self.usage_context_plan
        receipt = self.usage_context_receipt
        summary = receipt["summary"]
        identity = receipt["identity_closure"]
        self.assertTrue(plan["frozen_before_output"])
        self.assertTrue(
            plan["recurrence_control"][
                "selection_frozen_before_context_output"
            ]
        )
        self.assertEqual(
            "preexisting-work-identity-control-not-recurrence-rank-or-sign-likeness",
            plan["recurrence_control"]["selection_basis"],
        )
        self.assertEqual(527, summary["row_count"])
        self.assertEqual(527, summary["target_occurrence_count"])
        self.assertEqual(4, summary["source_item_count"])
        self.assertEqual(256, summary["page_count"])
        self.assertEqual(110, summary["section_count"])
        self.assertEqual(3, summary["unsectioned_occurrence_count"])
        self.assertEqual(0, summary["source_editorial_occurrence_count"])
        self.assertEqual(0, summary["semantic_fields_populated"])
        self.assertTrue(identity["complete_occurrence_census"])
        self.assertEqual(527, identity["unique_context_id_count"])
        self.assertEqual(527, identity["unique_occurrence_id_count"])
        self.assertEqual(524, identity["section_selector_resolution_count"])
        self.assertTrue(
            all(value is False for value in receipt["semantic_boundary"].values())
        )
        exposure = receipt["content_exposure"]
        self.assertFalse(exposure["tracked_exact_strings"])
        self.assertFalse(exposure["tracked_sequence"])
        self.assertFalse(exposure["tracked_context"])
        self.assertFalse(exposure["tracked_occurrence_positions"])
        self.assertFalse(exposure["confidentiality_claimed"])
        self.assertEqual(
            "blocked",
            receipt["rights_and_visibility"]["future_site_route"],
        )
        self.assertTrue(
            receipt["rights_and_visibility"][
                "fresh_public_acquisition_and_rights_gate_required"
            ]
        )
        ignored = subprocess.run(
            ["git", "check-ignore", "-q", plan["local_bundle"]["relative_path"]],
            cwd=ROOT,
            check=False,
        )
        self.assertEqual(0, ignored.returncode)

    def test_usage_context_private_row_contract_resists_semantic_fields(self) -> None:
        schema = json.loads(
            (
                ROOT / "ToS/contracts/lexical-usage-context-row.schema.json"
            ).read_text(encoding="utf-8")
        )
        target_surface = "synthetic-control-token"
        occurrence_id = "tos.occurrence.synthetic-control-000001"
        row = {
            "schema_version": "tos_lexical_usage_context_row_v1",
            "context_id": USAGE_CONTEXT_BUILDER._context_id(
                self.usage_context_plan["plan_id"], "a" * 64, occurrence_id
            ),
            "question_id": "zarathustra-work-identity-control-context-v1",
            "form_key": self.usage_context_plan["recurrence_control"]["form_key"],
            "exact_form_sha256": self.usage_context_plan["recurrence_control"][
                "exact_form_sha256"
            ],
            "occurrence_id": occurrence_id,
            "item_ref": "tos.item.synthetic-control",
            "part_order": 1,
            "source_file_sha256": "b" * 64,
            "token_ordinal": 1,
            "page_resource_id": "tei-page:synthetic-1",
            "section_resource_id": None,
            "text_node_path": "/TEI/text/body/p[1]/text()[1]",
            "start_offset": 0,
            "end_offset": len(target_surface),
            "editorial_status": "witness-text",
            "target_exact_form": target_surface,
            "left_exact_tokens": [],
            "right_exact_tokens": ["neighbor"],
            "left_token_count": 0,
            "right_token_count": 1,
            "requested_window_each_side": 24,
            "page_start_clipped": True,
            "page_end_clipped": True,
            "source_database_sha256": "a" * 64,
            "authority": "unreviewed-source-visible-method-control",
        }
        validator = Draft202012Validator(schema)
        self.assertEqual([], list(validator.iter_errors(row)))
        expected_context_id = (
            "usage-context:sha256:"
            + hashlib.sha256(
                (
                    self.usage_context_plan["plan_id"]
                    + "\n"
                    + "a" * 64
                    + "\n"
                    + occurrence_id
                ).encode("utf-8")
            ).hexdigest()
        )
        self.assertEqual(expected_context_id, row["context_id"])
        for field, value in (
            ("lemma", "synthetic"),
            ("sign_score", 1.0),
            ("concept_ref", "tos.concept.synthetic"),
        ):
            contaminated = dict(row)
            contaminated[field] = value
            with self.subTest(field=field):
                self.assertTrue(list(validator.iter_errors(contaminated)))

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

    def test_morphology_context_episode_is_output_blind_and_b_only(self) -> None:
        schemas = [
            (
                "plan",
                "morphology-contextual-episode-plan.schema.json",
                self.morphology_context_plan,
            ),
            (
                "receipt",
                "morphology-contextual-episode-receipt.schema.json",
                self.morphology_context_receipt,
            ),
            (
                "provenance",
                "provenance-event.schema.json",
                self.morphology_context_provenance,
            ),
        ]
        for label, schema_name, payload in schemas:
            with self.subTest(label=label):
                schema = json.loads(
                    (ROOT / "ToS/contracts" / schema_name).read_text(
                        encoding="utf-8"
                    )
                )
                self.assertEqual(
                    [], list(Draft202012Validator(schema).iter_errors(payload))
                )
        plan = self.morphology_context_plan
        receipt = self.morphology_context_receipt
        self.assertTrue(plan["frozen_before_b_output"])
        self.assertEqual([1, 73, 145], plan["selection"]["recurrence_ranks"])
        self.assertFalse(plan["selection"]["b_output_visible_during_selection"])
        self.assertFalse(plan["selection"]["semantic_labels_used"])
        self.assertEqual(
            "admitted-unacquired", receipt["variant_state"]["b"]
        )
        self.assertEqual(
            "blocked-question-inapplicable", receipt["variant_state"]["c"]
        )
        self.assertFalse(receipt["variant_state"]["human_work_scheduled"])
        self.assertTrue(
            all(value is False for value in receipt["semantic_boundary"].values())
        )
        self.assertEqual(
            self.validation["morphology_context"]["receipt_sha256"],
            hashlib.sha256(MORPHOLOGY_CONTEXT_RECEIPT_PATH.read_bytes()).hexdigest(),
        )

    def test_morphology_context_b_artifact_denial_is_retained_without_run(self) -> None:
        schema = json.loads(
            (
                ROOT
                / "ToS/contracts/"
                "morphology-contextual-artifact-admission.schema.json"
            ).read_text(encoding="utf-8")
        )
        provenance_schema = json.loads(
            (ROOT / "ToS/contracts/provenance-event.schema.json").read_text(
                encoding="utf-8"
            )
        )
        admission = self.morphology_context_admission
        self.assertEqual(
            [], list(Draft202012Validator(schema).iter_errors(admission))
        )
        self.assertEqual(
            [],
            list(
                Draft202012Validator(provenance_schema).iter_errors(
                    self.morphology_context_admission_provenance
                )
            ),
        )
        self.assertEqual(
            "artifact-acquired-admission-denied-b-not-run",
            admission["status"],
        )
        self.assertEqual("private-cache-complete", admission["acquisition"]["status"])
        self.assertEqual("deny", admission["trust_admission"]["trust_gate_verdict"])
        self.assertFalse(admission["trust_admission"]["verification_ok"])
        self.assertEqual([], admission["trust_admission"]["verified_controls"])
        self.assertTrue(
            all(value is False for value in admission["execution_effects"].values())
        )
        self.assertTrue(
            all(value is False for value in admission["content_boundary"].values())
        )
        self.assertEqual(
            hashlib.sha256(MORPHOLOGY_CONTEXT_ADMISSION_PATH.read_bytes()).hexdigest(),
            self.validation["morphology_context"]["b_artifact_admission"]["sha256"],
        )
        tracked = (
            MORPHOLOGY_CONTEXT_ADMISSION_PATH.read_text(encoding="utf-8")
            + MORPHOLOGY_CONTEXT_ADMISSION_PROVENANCE_PATH.read_text(
                encoding="utf-8"
            )
        )
        self.assertNotIn("/srv/", tracked)
        self.assertNotIn("local-content/", tracked)
        self.assertNotIn("wieder", tracked)

    def test_morphology_context_b_result_is_text_free_and_non_authoritative(
        self,
    ) -> None:
        result_schema = json.loads(
            (
                ROOT
                / "ToS/contracts/"
                "morphology-contextual-result-receipt.schema.json"
            ).read_text(encoding="utf-8")
        )
        provenance_schema = json.loads(
            (ROOT / "ToS/contracts/provenance-event.schema.json").read_text(
                encoding="utf-8"
            )
        )
        result = self.morphology_context_result
        provenance = self.morphology_context_result_provenance
        self.assertEqual(
            [], list(Draft202012Validator(result_schema).iter_errors(result))
        )
        self.assertEqual(
            [],
            list(Draft202012Validator(provenance_schema).iter_errors(provenance)),
        )
        self.assertEqual(
            "b-executed-machine-proposal-awaiting-real-trigger",
            result["status"],
        )
        self.assertEqual(
            "unmeasured-no-german-competent-gold",
            result["quality"]["status"],
        )
        self.assertTrue(result["repeat_determinism"]["deterministic"])
        self.assertEqual(
            result["repeat_determinism"]["pass_1_stream_sha256"],
            result["repeat_determinism"]["pass_2_stream_sha256"],
        )
        self.assertFalse(result["followup"]["human_work_scheduled"])
        self.assertTrue(
            all(value is False for value in result["semantic_boundary"].values())
        )
        self.assertFalse(
            self.validation["morphology_context"]["b_result"][
                "human_work_scheduled"
            ]
        )
        self.assertFalse(
            self.validation["morphology_context"]["b_result"]["semantic_effect"]
        )
        self.assertEqual(
            hashlib.sha256(MORPHOLOGY_CONTEXT_RESULT_PATH.read_bytes()).hexdigest(),
            self.validation["morphology_context"]["b_result"]["sha256"],
        )
        tracked = (
            MORPHOLOGY_CONTEXT_RESULT_PATH.read_text(encoding="utf-8")
            + MORPHOLOGY_CONTEXT_RESULT_PROVENANCE_PATH.read_text(
                encoding="utf-8"
            )
        )
        for prohibited in (
            "/srv/",
            "local-content/",
            '"context_text"',
            '"target_exact_form"',
            '"occurrence_id"',
            '"target_start_offset"',
            '"target_end_offset"',
            "wieder",
        ):
            self.assertNotIn(prohibited, tracked)

    def test_contextual_result_aggregate_does_not_return_source_strings(self) -> None:
        surface = "Testform"
        context = "Alpha Testform omega"
        start = context.index(surface)
        end = start + len(surface)
        form_digest = hashlib.sha256(surface.encode("utf-8")).hexdigest()
        context_digest = hashlib.sha256(context.encode("utf-8")).hexdigest()
        token_payloads = []
        cursor = 0
        for index, (text, whitespace, pos, tag) in enumerate(
            (
                ("Alpha", " ", "NOUN", "NN"),
                ("Testform", " ", "ADV", "ADV"),
                ("omega", "", "NOUN", "NN"),
            )
        ):
            token_start = cursor
            token_end = token_start + len(text)
            token_payloads.append(
                {
                    "dep": "dep",
                    "end_offset": token_end,
                    "ent_type": "",
                    "head_token_index": index,
                    "is_sent_start": index == 0,
                    "lemma": text.casefold(),
                    "morph": {},
                    "pos": pos,
                    "start_offset": token_start,
                    "tag": tag,
                    "text": text,
                    "token_index": index,
                    "whitespace": whitespace,
                }
            )
            cursor = token_end + len(whitespace)
        rows = []
        for rank, role, part in zip(
            (1, 73, 145),
            ("first", "inclusive-median", "last"),
            (1, 3, 4),
            strict=True,
        ):
            row = {
                "authority": "unreviewed-contextual-provider-proposal",
                "context_id": f"private-{rank}",
                "context_sha256": context_digest,
                "context_text": context,
                "episode_id": "zarathustra-selected-form-context-b-v1",
                "exact_form_sha256": form_digest,
                "form_key": f"lexical-form:sha256:{form_digest}",
                "input_preserved": True,
                "item_ref": "tos.item.private",
                "occurrence_id": f"tos.occurrence.private-{rank}",
                "part_order": part,
                "provider": {
                    **MORPHOLOGY_CONTEXT_RESULT_RECORDER.EXPECTED_PROVIDER,
                },
                "schema_version": "tos_zdl_contextual_morphology_row_v1",
                "selection_rank": rank,
                "selection_role": role,
                "target_end_offset": end,
                "target_exact_form": surface,
                "target_start_offset": start,
                "target_tokens": [token_payloads[1]],
                "tokenization": {
                    "token_count": 3,
                    "target_token_count": 1,
                    "exact_single_token_alignment": True,
                    "split_or_expanded_alignment": False,
                    "target_covered": True,
                },
                "tokens": token_payloads,
            }
            rows.append(row)
        with tempfile.TemporaryDirectory() as temporary:
            raw_path = Path(temporary) / "raw.jsonl"
            raw_path.write_bytes(
                b"".join(
                    MORPHOLOGY_CONTEXT_RESULT_RECORDER.canonical_line(row)
                    for row in rows
                )
            )
            original_form = MORPHOLOGY_CONTEXT_RESULT_RECORDER.EXPECTED_FORM_SHA256
            MORPHOLOGY_CONTEXT_RESULT_RECORDER.EXPECTED_FORM_SHA256 = form_digest
            try:
                aggregate = MORPHOLOGY_CONTEXT_RESULT_RECORDER.inspect_raw_output(
                    raw_path
                )
            finally:
                MORPHOLOGY_CONTEXT_RESULT_RECORDER.EXPECTED_FORM_SHA256 = original_form
        serialized = json.dumps(aggregate, ensure_ascii=False)
        self.assertNotIn(surface, serialized)
        self.assertNotIn(context, serialized)
        self.assertEqual(3, aggregate["exact_single_token_alignment_count"])
        self.assertEqual({"ADV": 3}, aggregate["target_pos"])

    def test_morphology_context_tracked_files_withhold_private_rows(self) -> None:
        plan = self.morphology_context_plan
        private_row_withholding_payload = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (
                MORPHOLOGY_CONTEXT_RECEIPT_PATH,
                MORPHOLOGY_CONTEXT_PROVENANCE_PATH,
            )
        )
        for prohibited in (
            '"context_text"',
            '"target_exact_form"',
            '"occurrence_id"',
            '"text_node_path"',
            '"target_start_offset"',
        ):
            self.assertNotIn(prohibited, private_row_withholding_payload)
        tracked_payload = (
            MORPHOLOGY_CONTEXT_PLAN_PATH.read_text(encoding="utf-8")
            + private_row_withholding_payload
        )
        self.assertNotIn("wieder", tracked_payload)
        self.assertFalse(
            self.morphology_context_receipt["content_exposure"][
                "tracked_context"
            ]
        )
        result = subprocess.run(
            ["git", "check-ignore", "-q", plan["local_packet"]["relative_path"]],
            cwd=ROOT,
            check=False,
        )
        self.assertEqual(0, result.returncode)

    def test_morphology_context_raw_tei_tail_offset_is_exact(self) -> None:
        xml = (
            b'<TEI xmlns="http://www.tei-c.org/ns/1.0"><text><body><div>'
            b'<p>alpha <lb/>wieder omega</p></div></body></text></TEI>'
        )
        tree = MORPHOLOGY_CONTEXT_BUILDER.etree.ElementTree(
            MORPHOLOGY_CONTEXT_BUILDER.etree.fromstring(xml)
        )
        target_path = "TEI/text[1]/body[1]/div[1]/p[1]/lb[1]/tail()[1]"
        owner_path, owner_kind = MORPHOLOGY_CONTEXT_BUILDER.target_owner(
            target_path
        )
        owner = MORPHOLOGY_CONTEXT_BUILDER.one_element(
            tree, owner_path, "synthetic owner"
        )
        context_path, context_kind = MORPHOLOGY_CONTEXT_BUILDER.context_unit(
            target_path
        )
        context = MORPHOLOGY_CONTEXT_BUILDER.one_element(
            tree, context_path, "synthetic context"
        )
        text, target_base = MORPHOLOGY_CONTEXT_BUILDER.flatten_with_target_base(
            context, owner, owner_kind
        )
        self.assertEqual("paragraph", context_kind)
        self.assertEqual("alpha wieder omega", text)
        self.assertEqual("wieder", text[target_base : target_base + 6])


if __name__ == "__main__":
    unittest.main()

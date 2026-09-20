from __future__ import annotations

from hashlib import sha256
import importlib.util
from pathlib import Path
import re
import unittest


REPO = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("zarathustra_recurring_formulas", REPO / "scripts/zarathustra_recurring_formulas.py")
assert SPEC and SPEC.loader
FORMULAS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FORMULAS)


def digest(value: str) -> str:
    return sha256(value.encode()).hexdigest()


def fixture(rows: list[tuple], normalizations: dict | None = None) -> tuple[list[dict], list[dict]]:
    """Independent full-coverage scanner, not the production source builder."""
    contexts, surfaces = [], []
    for index, (language, reading, order, kind, exact) in enumerate(rows):
        ref = f"context-{index}"
        contexts.append({"context_unit_ref": ref, "language": language, "part": 1,
                         "reading_ref": reading, "witness_order": order, "unit_kind": kind,
                         "exact_text": exact, "exact_sha256": digest(exact)})
        for ordinal, match in enumerate(re.finditer(r"\w+|\s+|[^\w\s]", exact)):
            text = match.group()
            surface_kind = "word" if text[0].isalnum() else "space" if text.isspace() else "punctuation"
            normalized = (normalizations or {}).get(text, text.casefold())
            surfaces.append({"surface_unit_id": f"surface-{index}-{ordinal}", "sentence_id": f"sentence-{index}",
                             "context_unit_ref": ref, "language": language, "part": 1,
                             "surface_kind": surface_kind, "start_offset": match.start(), "end_offset": match.end(),
                             "exact_text": text, "exact_sha256": digest(text), "normalized_text": normalized,
                             "normalized_sha256": digest(normalized)})
    return contexts, surfaces


class RecurringFormulaTests(unittest.TestCase):
    def build(self, rows: list[tuple], **options):
        return FORMULAS.build_formulas(*fixture(rows), **options)

    def test_exact_repeat_has_a_sparse_ordered_chain(self):
        rows = [("de", f"r{index}", index, "paragraph", "So sprach er zu mir.") for index in (1, 2, 3)]
        families, members, relations, receipt = self.build(rows)
        self.assertEqual(len(families), 1)
        self.assertEqual(families[0]["token_count"], 5)
        self.assertEqual(families[0]["match_kind"], "repeats_exact")
        self.assertEqual(len(members), 3)
        self.assertEqual(len(relations), 2)
        self.assertEqual(relations[0]["target_occurrence_ref"], relations[1]["source_occurrence_ref"])
        self.assertEqual(receipt["counts"]["suppressed_nested_sequences"], 2)

    def test_case_punctuation_and_newlines_are_normalized_not_exact(self):
        families, members, relations, _ = self.build([
            ("de", "r1", 1, "paragraph", "So sprach er zu mir!"),
            ("de", "r2", 2, "paragraph", "so, sprach\ner zu mir?"),
        ])
        self.assertEqual(families[0]["match_kind"], "reprises_normalized")
        self.assertEqual(relations[0]["relation_type"], "reprises_normalized")
        self.assertEqual(members[1]["exact_text"], "so, sprach\ner zu mir")
        self.assertEqual(members[1]["exact_sha256"], digest("so, sprach\ner zu mir"))

    def test_prose_paragraphs_are_hard_barriers(self):
        families, _, _, receipt = self.build([
            ("de", "r1", 1, "paragraph", "eins zwei"),
            ("de", "r1", 2, "paragraph", "drei vier"),
            ("de", "r2", 3, "paragraph", "eins zwei drei vier"),
        ])
        self.assertFalse(families)
        self.assertEqual(receipt["coverage"]["lexical_streams"], 3)

    def test_adjacent_verse_lines_preserve_separate_source_spans(self):
        contexts, surfaces = fixture([
            ("de", "r1", 1, "verse_line", "eins zwei\n"),
            ("de", "r1", 2, "verse_line", "drei vier"),
            ("de", "r2", 3, "verse_line", "eins zwei\n"),
            ("de", "r2", 4, "verse_line", "drei vier"),
        ])
        families, members, _, receipt = FORMULAS.build_formulas(contexts, surfaces)
        self.assertEqual(len(families), 1)
        self.assertEqual(receipt["coverage"]["adjacent_verse_context_joins"], 2)
        self.assertEqual(len(members[0]["source_spans"]), 2)
        self.assertIsNone(members[0]["exact_text"])
        self.assertIsNone(members[0]["exact_sha256"])
        self.assertTrue(members[0]["display_joined_across_contexts"])
        for member in members:
            for span in member["source_spans"]:
                context = next(row for row in contexts if row["context_unit_ref"] == span["context_unit_ref"])
                self.assertEqual(span["exact_text"], context["exact_text"][span["start_offset"]:span["end_offset"]])
                self.assertEqual(span["exact_sha256"], digest(span["exact_text"]))

    def test_verse_does_not_bridge_a_chapter_or_witness_order_gap(self):
        for reading, order in (("r2", 2), ("r1", 3)):
            with self.subTest(reading=reading, order=order):
                families, _, _, receipt = self.build([
                    ("de", "r1", 1, "verse_line", "eins zwei"),
                    ("de", reading, order, "verse_line", "drei vier"),
                    ("de", "r3", 5, "paragraph", "eins zwei drei vier"),
                ])
                self.assertFalse(families)
                self.assertEqual(receipt["coverage"]["adjacent_verse_context_joins"], 0)

    def test_normalized_strings_are_never_cross_language_equivalence(self):
        families, _, _, _ = self.build([
            ("de", "r1", 1, "paragraph", "a b c d"),
            ("ru", "r1", 1, "paragraph", "a b c d"),
        ])
        self.assertFalse(families)

    def test_a_shorter_formula_with_extra_occurrence_survives(self):
        families, _, _, _ = self.build([
            ("de", "r1", 1, "paragraph", "eins zwei drei vier fünf"),
            ("de", "r2", 2, "paragraph", "eins zwei drei vier fünf"),
            ("de", "r3", 3, "paragraph", "eins zwei drei vier sechs"),
        ])
        counts = {tuple(row["normalized_tokens"]): row["occurrence_count"] for row in families}
        self.assertEqual(counts[("eins", "zwei", "drei", "vier")], 3)
        self.assertEqual(counts[("eins", "zwei", "drei", "vier", "fünf")], 2)

    def test_repeated_occurrences_inside_one_context_are_kept(self):
        families, members, _, _ = self.build([
            ("de", "r1", 1, "paragraph", "eins zwei drei vier; eins zwei drei vier."),
        ])
        self.assertEqual(len(families), 1)
        self.assertEqual(families[0]["independent_occurrence_count"], 2)
        self.assertEqual([row["start_offset"] for row in members], [0, 21])

    def test_maximal_extension_cannot_destroy_independent_occurrences(self):
        families, _, _, _ = self.build([
            ("de", "r1", 1, "paragraph", "vier eins zwei drei vier eins zwei drei vier"),
        ])
        match = next(row for row in families if row["normalized_tokens"] == ["eins", "zwei", "drei", "vier"])
        self.assertEqual(match["independent_occurrence_count"], 2)

    def test_overlapping_only_matches_do_not_count_as_two_occurrences(self):
        families, _, _, _ = self.build([("de", "r1", 1, "paragraph", "a a a a a")])
        self.assertFalse(families)

    def test_length_cap_retains_prefix_not_every_shifted_window(self):
        families, _, _, receipt = self.build([
            ("de", "r1", 1, "paragraph", "eins zwei drei vier fünf sechs sieben"),
            ("de", "r2", 2, "paragraph", "eins zwei drei vier fünf sechs sieben"),
        ], max_tokens=5)
        self.assertEqual(len(families), 1)
        self.assertEqual(families[0]["normalized_tokens"], ["eins", "zwei", "drei", "vier", "fünf"])
        self.assertTrue(families[0]["right_extension_capped"])
        self.assertEqual(receipt["counts"]["length_capped_families"], 1)

    def test_normalization_reuses_upstream_historical_forms(self):
        contexts, surfaces = fixture([
            ("ru", "r1", 1, "paragraph", "Міръ и его вѣчный путь"),
            ("ru", "r2", 2, "paragraph", "мир и его вечный путь"),
        ], {"Міръ": "мир", "вѣчный": "вечный"})
        families, members, _, _ = FORMULAS.build_formulas(contexts, surfaces)
        self.assertEqual(len(families), 1)
        self.assertEqual(families[0]["match_kind"], "reprises_normalized")
        self.assertIn("Міръ", members[0]["exact_text"])

    def test_no_implicit_hyphenation_or_ocr_repair(self):
        families, _, _, receipt = self.build([
            ("de", "r1", 1, "paragraph", "ewig wieder-\nkehren alle dinge"),
            ("de", "r2", 2, "paragraph", "ewig wiederkehren alle dinge"),
        ])
        self.assertFalse(families)
        self.assertTrue(any("dehyphenation" in item for item in receipt["limitations"]))

    def test_letter_spaced_surfaces_are_deferred_not_four_word_formulas(self):
        families, members, relations, receipt = self.build([
            ("ru", "r1", 1, "paragraph", "т а к ъ"),
            ("ru", "r2", 2, "paragraph", "т  а к ъ"),
        ])
        self.assertEqual(families[0]["status"], "deferred")
        self.assertEqual(families[0]["quality_deferred_occurrence_count"], 2)
        self.assertTrue(all(row["quality_status"] == "deferred" for row in members))
        self.assertEqual(relations[0]["status"], "deferred")
        self.assertEqual(receipt["counts"]["quality_statuses"], {"deferred": 1})

    def test_punctuation_separated_letters_are_not_spacing_repair_candidates(self):
        families, _, _, _ = self.build([
            ("de", "r1", 1, "paragraph", "A! B! C! D!"),
            ("de", "r2", 2, "paragraph", "A! B! C! D!"),
        ])
        self.assertEqual(families[0]["status"], "proposed")

    def test_unscoped_epigraph_groups_are_not_extra_chapters(self):
        _, _, _, receipt = self.build([
            ("de", "p1.r1", 1, "paragraph", "eins zwei drei vier"),
            ("de", "p2.unscoped-technical", 2, "paragraph", "eins zwei drei vier"),
        ])
        self.assertEqual(receipt["coverage"]["readings_by_language"], {"de": 1})
        self.assertEqual(receipt["coverage"]["unscoped_technical_groups_by_language"], {"de": 1})

    def test_complete_source_reconstruction_is_required(self):
        contexts, surfaces = fixture([("de", "r1", 1, "paragraph", "eins zwei drei vier")])
        del surfaces[2]
        with self.assertRaisesRegex(ValueError, "coverage"):
            FORMULAS.build_formulas(contexts, surfaces)

    def test_corrupt_source_surface_is_rejected(self):
        contexts, surfaces = fixture([("de", "r1", 1, "paragraph", "eins zwei drei vier")])
        surfaces[0]["exact_text"] = "fünf"
        with self.assertRaisesRegex(ValueError, "differs"):
            FORMULAS.build_formulas(contexts, surfaces)

    def test_input_order_does_not_change_output_identity_or_order(self):
        contexts, surfaces = fixture([
            ("de", "r1", 1, "paragraph", "eins zwei drei vier fünf"),
            ("de", "r2", 2, "paragraph", "eins zwei drei vier fünf"),
        ])
        self.assertEqual(FORMULAS.build_formulas(contexts, surfaces), FORMULAS.build_formulas(contexts[::-1], surfaces[::-1]))

    def test_empty_scope_and_no_single_occurrence_families(self):
        families, members, relations, receipt = FORMULAS.build_formulas([], [])
        self.assertEqual((families, members, relations), ([], [], []))
        self.assertEqual(receipt["coverage"]["input_contexts"], 0)
        families, _, _, _ = self.build([("de", "r1", 1, "paragraph", "eins zwei drei vier fünf")])
        self.assertFalse(families)

    def test_invalid_settings_fail_closed(self):
        for settings in ({"min_tokens": 3}, {"min_occurrences": 1}, {"max_tokens": 3}, {"max_tokens": 513}):
            with self.subTest(settings=settings), self.assertRaises(ValueError):
                FORMULAS.build_formulas([], [], **settings)


if __name__ == "__main__":
    unittest.main()

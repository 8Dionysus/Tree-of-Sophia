import hashlib
import json
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import build_zarathustra_reading_workbench_v1 as READING_BUILDER
from build_zarathustra_reading_workbench_v1 import DATABASE, ROUTE, crosswalk_occurrences


class OccurrenceBridgeTests(unittest.TestCase):
    def test_xml_local_offsets_are_not_relabelled_context_offsets(self):
        text = "A word. Another word."
        occurrences = [dict(language="de", in_work_scope=1, context_unit_ref="c", exact_form="word",
                            exact_form_sha256=hashlib.sha256(b"word").hexdigest(), existing_occurrence_ref=f"o{i}",
                            token_ordinal=i, start_offset=0, end_offset=4) for i in (1, 2)]
        mapped, gaps = crosswalk_occurrences([dict(context_unit_ref="c", exact_text=text)], [], occurrences)
        self.assertEqual([(r["start_offset"], r["end_offset"]) for r in mapped], [(2, 6), (16, 20)])
        self.assertFalse(gaps)
        self.assertTrue(all(r["surface_unit_ref"] is None for r in mapped))

    def test_missing_context_and_surface_are_not_fabricated(self):
        mapped, gaps = crosswalk_occurrences([], [], [dict(language="de", in_work_scope=1,
            context_unit_ref="missing", existing_occurrence_ref="o")])
        self.assertFalse(mapped)
        self.assertEqual(gaps[0]["kind"], "legacy_occurrence_context_unmapped")


@unittest.skipUnless((ROOT / DATABASE).is_file(), "private reading corpus not installed")
class WholeCorpusReadingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect((ROOT / DATABASE).as_uri()+"?mode=ro&immutable=1", uri=True)
        cls.db.row_factory = sqlite3.Row

    @classmethod
    def tearDownClass(cls):
        cls.db.close()

    def test_every_context_is_an_exact_segment_partition(self):
        for context in self.db.execute("SELECT * FROM contexts"):
            segments = list(self.db.execute("SELECT * FROM discourse_segments WHERE context_unit_ref=? ORDER BY start_offset", (context["context_unit_ref"],)))
            self.assertEqual("".join(s["exact_text"] for s in segments), context["exact_text"])
            offset = 0
            for s in segments:
                self.assertEqual(s["start_offset"], offset)
                exact = context["exact_text"][s["start_offset"]:s["end_offset"]]
                self.assertEqual(s["exact_sha256"], hashlib.sha256(exact.encode()).hexdigest())
                offset = s["end_offset"]
            self.assertEqual(offset, len(context["exact_text"]))

    def test_all_bilingual_readings_and_no_accepted_claims(self):
        for language in ("de", "ru"):
            count = self.db.execute("SELECT count(distinct reading_ref) FROM contexts WHERE language=? AND reading_ref LIKE '%.r%'", (language,)).fetchone()[0]
            self.assertEqual(count, 81)
        self.assertEqual(self.db.execute("SELECT count(*) FROM translation_alignments WHERE human_acceptance<>0 OR semantic_equivalence_asserted<>0").fetchone()[0], 0)
        self.assertFalse(self.db.execute("SELECT 1 FROM discourse_segments WHERE speaker_status NOT IN ('proposed','ambiguous','deferred')").fetchone())

    def test_formula_members_are_exact_substrings_with_explicit_quality(self):
        for member in self.db.execute("SELECT f.*,c.exact_text AS source_text FROM formula_occurrences f JOIN contexts c USING(context_unit_ref)"):
            exact = member["source_text"][member["start_offset"]:member["end_offset"]]
            self.assertEqual(exact, member["exact_text"])
            self.assertEqual(hashlib.sha256(exact.encode()).hexdigest(), member["exact_sha256"])
        self.assertGreater(self.db.execute("SELECT count(*) FROM formulas WHERE status='deferred'").fetchone()[0], 0)


class TrackedReadingTests(unittest.TestCase):
    def test_companions_remain_text_free_and_candidate_only(self):
        manifest_path = ROOT / ROUTE / "manifest.v1.json"
        if not manifest_path.exists():
            self.skipTest("reading materialization not issued")
        manifest = json.loads(manifest_path.read_text())
        self.assertFalse(manifest["accepted"])
        self.assertFalse(manifest["canon_effect"])
        self.assertFalse(manifest["tracked_source_strings"])
        for artifact in manifest["artifacts"]:
            text = (ROOT / artifact["ref"]).read_text()
            self.assertNotIn('"exact_text":', text)
            self.assertNotIn('"normalized_tokens":', text)


class ReadingBuilderBoundaryTests(unittest.TestCase):
    def test_builder_rejects_output_inside_source_or_software_roots(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_root = root / "source-data"
            source_root.mkdir()
            cases = (
                (source_root, source_root / "nested-output"),
                (source_root / "nested-source", source_root),
                (source_root, ROOT / "synthetic-output"),
            )
            for selected_source, selected_output in cases:
                with self.subTest(source=selected_source, output=selected_output), \
                        patch.object(sys, "argv", [
                            "build_zarathustra_reading_workbench_v1.py", "--build",
                            "--source-root", str(selected_source),
                            "--output-root", str(selected_output),
                        ]), self.assertRaisesRegex(ValueError, "separate"):
                    READING_BUILDER.main()
            self.assertFalse((ROOT / "synthetic-output").exists())


if __name__ == "__main__":
    unittest.main()

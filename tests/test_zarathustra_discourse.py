import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from zarathustra_discourse import build_discourse, quote_action, sha, validate_partition


class DiscourseTests(unittest.TestCase):
    def run_texts(self, texts, language="de", cues=lambda text, language: [], readings=None):
        contexts, sentences = [], []
        for i, text in enumerate(texts):
            ref = f"c{i}"
            contexts.append(dict(context_unit_ref=ref, language=language, part=1,
                                 reading_ref=readings[i] if readings else "p1.r1", unit_kind="paragraph",
                                 witness_order=i, exact_text=text, exact_sha256=sha(text)))
            sentences.append(dict(sentence_id=f"s{i}", context_unit_ref=ref, start_offset=0,
                                  end_offset=len(text), exact_text=text))
        policy = {"chapters": [{"reading_ref": "p1.r1", "baseline_role": "narrator",
                                "baseline_mode": "narration", "status": "proposed"}]}
        segments, events, gaps = build_discourse(contexts, sentences, policy, cues)
        validate_partition(contexts, segments)
        return segments, events, gaps

    def test_expected_german_closer_wins(self):
        _, events, gaps = self.run_texts(['„Hallo“, sprach Zarathustra.'])
        self.assertEqual([e["action"] for e in events], ["open", "close"])
        self.assertFalse(gaps)
        self.assertEqual(events[-1]["depth_after"], 0)

    def test_nested_same_glyph_and_reporting_tail(self):
        segments, events, gaps = self.run_texts(['„Ein „Wort“ bleibt“, sagte er.'])
        self.assertEqual([e["action"] for e in events], ["open", "open", "close", "close"])
        self.assertFalse(gaps)
        self.assertEqual(segments[-1]["kind"], "narration")

    def test_english_closing_glyph_is_scanned(self):
        _, events, gaps = self.run_texts(['“A word”'])
        self.assertEqual([e["action"] for e in events], ["open", "close"])
        self.assertFalse(gaps)

    def test_german_paragraph_continuation_does_not_push(self):
        _, events, gaps = self.run_texts(['„Eins', '„Zwei“'])
        self.assertEqual([e["action"] for e in events], ["open", "continuation", "close"])
        self.assertFalse(gaps)

    def test_russian_continuation_does_not_close(self):
        _, events, gaps = self.run_texts(['«Один', '»Два', 'Три».'], "ru")
        self.assertEqual([e["action"] for e in events], ["open", "continuation", "close"])
        self.assertFalse(gaps)

    def test_unpaired_marks_are_not_repaired(self):
        _, events, gaps = self.run_texts(['»Один'], "ru")
        self.assertEqual(events[0]["action"], "unmatched_close")
        self.assertEqual(len(gaps), 1)

    def test_historical_letter_ocr_candidate_does_not_close_speech(self):
        _, events, gaps = self.run_texts(['Они любят» выдавать себя за людей.'], "ru")
        self.assertEqual([e["action"] for e in events], ["lexical_ocr_candidate"])
        self.assertEqual(gaps[0]["kind"], "guillemet_or_historical_letter_ambiguity")

    def test_stack_does_not_leak_between_readings(self):
        _, events, gaps = self.run_texts(['„Eins', 'Zwei“'], readings=["p1.r1", "p1.r2"])
        self.assertEqual(events[0]["match_status"], "unclosed_at_reading_end")
        self.assertEqual(events[1]["action"], "unmatched_close")
        self.assertEqual(len(gaps), 2)

    def test_source_conservation_rejects_mutation(self):
        context = dict(context_unit_ref="c", exact_text="abc")
        with self.assertRaises(ValueError):
            validate_partition([context], [dict(context_unit_ref="c", start_offset=0, end_offset=3,
                                               exact_text="abd", exact_sha256=sha("abd"))])

    def test_reporting_intro_selects_speaker_not_narrator(self):
        text = 'sprach Zwerg: „Hallo“'
        cues = lambda text, language: [dict(start=0, end=12, role="dwarf", status="proposed")]
        segments, events, gaps = self.run_texts([text], cues=cues)
        word = next(s for s in segments if s["exact_text"] == "Hallo")
        self.assertEqual(word["speaker_role"], "dwarf")
        self.assertEqual(word["utterer_role"], "dwarf")


if __name__ == "__main__":
    unittest.main()

"""Independent source-return and nested-voice regressions for the reading layer."""

from __future__ import annotations

from hashlib import sha256
from pathlib import Path
import sys
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from zarathustra_discourse import build_discourse, validate_partition
from zarathustra_voice_policy import find_reporting_cues


def digest(text):
    return sha256(text.encode("utf-8")).hexdigest()


def parse(text, *, role="narrator", mode="narration", language="de"):
    context = {
        "context_unit_ref": "challenger-context", "language": language, "part": 1,
        "reading_ref": "p1.r1", "unit_kind": "paragraph", "witness_order": 1,
        "exact_text": text, "exact_sha256": digest(text),
    }
    sentence = {
        "sentence_id": "challenger-sentence", "context_unit_ref": context["context_unit_ref"],
        "start_offset": 0, "end_offset": len(text), "exact_text": text,
    }
    policies = {"chapters": [{"reading_ref": "p1.r1", "baseline_role": role,
                              "baseline_mode": mode, "status": "proposed"}]}
    result = build_discourse([context], [sentence], policies, find_reporting_cues)
    validate_partition([context], result[0])
    return result


class ReadingChallengerTests(unittest.TestCase):
    def test_every_supported_single_quote_has_a_scanned_closer(self):
        _, events, gaps = parse("‘A word’")
        self.assertEqual([event["action"] for event in events], ["open", "close"])
        self.assertEqual(events[0]["paired_event_ref"], events[1]["event_id"])
        self.assertFalse(gaps)

    def test_nested_reporting_clause_belongs_to_outer_utterer(self):
        text = 'Zarathustra sprach: „Der Zwerg sagte: ‚Alles geht.‘“'
        segments, _, gaps = parse(text)
        reporting = next(row for row in segments if row["exact_text"] == "Der Zwerg sagte")
        self.assertEqual(reporting["speaker_role"], "zarathustra")
        self.assertEqual(reporting["utterer_role"], "zarathustra")
        spoken = next(row for row in segments if row["exact_text"] == "Alles geht.")
        self.assertEqual(spoken["speaker_role"], "dwarf")
        self.assertEqual(spoken["utterer_role"], "zarathustra")
        self.assertFalse(gaps)

    def test_reported_speech_inside_quote_does_not_resolve_its_speaker(self):
        segments, _, _ = parse('„Die Thiere sprachen mit mir, als ich schlief.“',
                               role="zarathustra", mode="unquoted_speech")
        # The inner report gives no evidence that the animals uttered the whole
        # enclosing quotation. Keep its unresolved embedded voice if necessary.
        for row in segments:
            if "als ich schlief" in row["exact_text"]:
                self.assertFalse(row["speaker_role"] == "animals" and row["speaker_status"] == "proposed")

    def test_partition_rejects_invented_context_spans(self):
        context = {"context_unit_ref": "known", "exact_text": "abc"}
        real = {"segment_id": "real", "context_unit_ref": "known", "start_offset": 0,
                "end_offset": 3, "exact_text": "abc", "exact_sha256": digest("abc"),
                "speaker_status": "ambiguous"}
        invented = dict(real, segment_id="invented", context_unit_ref="unknown")
        with self.assertRaises(ValueError):
            validate_partition([context], [real, invented])

    def test_quote_links_are_reciprocal_and_depth_is_conserved(self):
        _, events, gaps = parse('Zarathustra sprach: „Ein ‚Wort‘ und «anderes».“')
        event_map = {row["event_id"]: row for row in events}
        for event in events:
            self.assertGreaterEqual(event["depth_before"], 0)
            self.assertGreaterEqual(event["depth_after"], 0)
            delta = 1 if event["action"] == "open" else -1 if event["action"] == "close" else 0
            self.assertEqual(event["depth_after"] - event["depth_before"], delta)
            if event["paired_event_ref"]:
                peer = event_map[event["paired_event_ref"]]
                self.assertEqual(peer["paired_event_ref"], event["event_id"])
                self.assertEqual(peer["context_unit_ref"], event["context_unit_ref"])
        self.assertFalse(gaps)

    def test_partition_rejects_duplicate_stable_segment_ids(self):
        contexts = [{"context_unit_ref": ref, "exact_text": "abc"} for ref in ("one", "two")]
        segments = [
            {"segment_id": "same-id", "context_unit_ref": ref, "start_offset": 0,
             "end_offset": 3, "exact_text": "abc", "exact_sha256": digest("abc"),
             "speaker_status": "ambiguous"}
            for ref in ("one", "two")
        ]
        with self.assertRaisesRegex(ValueError, "duplicate"):
            validate_partition(contexts, segments)

    def test_ancestor_close_recovery_records_the_unclosed_inner_scope(self):
        segments, events, gaps = parse('„A ‚B“ C')
        self.assertEqual([row["action"] for row in events], ["open", "open", "recover_ancestor_close"])
        self.assertEqual(events[-1]["depth_after"], 0)
        self.assertEqual(events[1]["match_status"], "unclosed_before_ancestor_close")
        self.assertIsNone(events[1]["paired_event_ref"])
        self.assertEqual(events[-1]["paired_event_ref"], events[0]["event_id"])
        self.assertEqual([row["kind"] for row in gaps], ["interrupted_nested_quote_scope"])
        self.assertEqual(segments[-1]["quote_depth"], 0)
        self.assertEqual(segments[-1]["exact_text"], " C")

    def test_russian_paired_quoted_term_is_not_an_ocr_hard_sign(self):
        segments, events, _ = parse("«Мир» и жизнь.", language="ru")
        self.assertEqual([row["action"] for row in events], ["open", "close"])
        self.assertEqual(events[0]["paired_event_ref"], events[1]["event_id"])
        self.assertEqual(segments[-1]["exact_text"], " и жизнь.")
        self.assertEqual(segments[-1]["quote_depth"], 0)


if __name__ == "__main__":
    unittest.main()

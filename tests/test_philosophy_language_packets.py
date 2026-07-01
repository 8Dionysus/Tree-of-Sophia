from __future__ import annotations

import json
import re
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from plant_table_i_prepared_dossiers import build_language_packets, load_jsonl  # noqa: E402


PACKETS_PATH = REPO_ROOT / "ToS/philosophy/graph-workbench/language-packets/table-i-text-bearing-nodes.jsonl"
PROPOSED_NODES_PATH = REPO_ROOT / "ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl"
CONTRACT_REF = "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json"
REGISTRY_REF = "ToS/philosophy/atlas/multilingual/language-registry.json"


class PhilosophyLanguagePacketsTest(unittest.TestCase):
    def load_packets(self) -> list[dict[str, object]]:
        return [json.loads(line) for line in PACKETS_PATH.read_text(encoding="utf-8").splitlines() if line.strip()]

    def test_generated_packets_match_builder(self) -> None:
        nodes = load_jsonl(PROPOSED_NODES_PATH)
        expected = "".join(
            json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n"
            for row in build_language_packets(nodes)
        )
        self.assertEqual(PACKETS_PATH.read_text(encoding="utf-8"), expected)

    def test_packets_cover_text_corpus_nodes(self) -> None:
        nodes = load_jsonl(PROPOSED_NODES_PATH)
        text_corpus_ids = {
            str(row["candidate_id"])
            for row in nodes
            if row.get("node_kind") == "text_corpus"
        }
        packets = self.load_packets()
        self.assertEqual(len(packets), 225)
        self.assertEqual({str(row["node_ref"]["id"]) for row in packets}, text_corpus_ids)
        self.assertEqual({str(row["node_ref"]["id_kind"]) for row in packets}, {"candidate_id"})

    def test_packets_keep_original_ru_en_and_unresolved_relation_pressure(self) -> None:
        packets = self.load_packets()
        first = packets[0]
        self.assertEqual(first["schema_version"], "tos_philosophy_text_bearing_language_packet_v1")
        self.assertEqual(first["language_registry_ref"], REGISTRY_REF)
        self.assertEqual(first["text_bearing_nodes_contract_ref"], CONTRACT_REF)
        title_block = first["title_block"]
        self.assertEqual(set(title_block), {"original", "ru", "en"})
        self.assertEqual(title_block["original"]["attestation_status"], "unknown")
        self.assertEqual(title_block["original"]["review_status"], "pending_original_witness")
        self.assertIn(title_block["ru"]["translation_status"], {"source", "reviewed", "draft", "pending"})
        self.assertIn(title_block["en"]["translation_status"], {"source", "reviewed", "draft", "pending"})
        self.assertNotRegex(str(title_block["en"]["value"]), r"[А-Яа-яЁё]")
        predicates = {row["predicate"] for row in first["relation_pressure"]}
        self.assertEqual(predicates, {"has_original_language", "uses_script", "has_witness"})
        self.assertTrue(all(row["target_status"] == "unresolved" for row in first["relation_pressure"]))

    def test_english_labels_do_not_leave_cyrillic_drafts(self) -> None:
        packets = self.load_packets()
        cyrillic_english = [
            row["title_block"]["en"]["value"]
            for row in packets
            if re.search(r"[А-Яа-яЁё]", str(row["title_block"]["en"]["value"]))
        ]
        self.assertEqual(cyrillic_english, [])


if __name__ == "__main__":
    unittest.main()

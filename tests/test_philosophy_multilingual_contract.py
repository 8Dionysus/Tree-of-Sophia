from __future__ import annotations

import json
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
MULTILINGUAL_ROOT = REPO_ROOT / "ToS/philosophy/atlas/multilingual"


def load_json(name: str) -> dict[str, object]:
    payload = json.loads((MULTILINGUAL_ROOT / name).read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise TypeError(f"{name} must contain a JSON object")
    return payload


class PhilosophyMultilingualContractTest(unittest.TestCase):
    def test_branch_manifest_names_language_contract_surfaces(self) -> None:
        manifest = load_json("branch.manifest.json")
        self.assertEqual(
            manifest["children"],
            [
                "README.md",
                "content-labels.json",
                "language-registry.json",
                "text-bearing-nodes.contract.json",
            ],
        )

    def test_language_registry_keeps_original_ru_en_slots(self) -> None:
        registry = load_json("language-registry.json")
        slots = {slot["slot"]: slot for slot in registry["display_slots"]}  # type: ignore[index]
        self.assertEqual(set(slots), {"original", "ru", "en"})
        self.assertIn("work", slots["original"]["required_for"])
        self.assertIn("all_projected_nodes", slots["ru"]["required_for"])
        self.assertIn("all_projected_nodes", slots["en"]["required_for"])
        self.assertIn("reviewed", registry["translation_statuses"])
        self.assertIn("contested", registry["translation_statuses"])
        self.assertIn("reconstructed", registry["attestation_statuses"])

    def test_text_bearing_contract_routes_work_language_packets(self) -> None:
        contract = load_json("text-bearing-nodes.contract.json")
        self.assertEqual(
            contract["language_registry_ref"],
            "ToS/philosophy/atlas/multilingual/language-registry.json",
        )
        self.assertEqual(contract["title_block"]["required_slots"], ["original", "ru", "en"])  # type: ignore[index]
        self.assertIn("authored_work", contract["applies_to_node_kinds"])
        self.assertIn("text_corpus", contract["applies_to_node_kinds"])
        predicates = {row["predicate"] for row in contract["relation_emission"]}  # type: ignore[index]
        self.assertTrue(
            {
                "has_original_language",
                "uses_script",
                "has_witness",
                "has_translation",
                "is_version_of",
            }
            <= predicates
        )

    def test_label_ledger_points_to_contracts(self) -> None:
        labels = load_json("content-labels.json")
        contracts = labels["planting_contracts"]
        self.assertEqual(
            contracts["language_registry_ref"],
            "ToS/philosophy/atlas/multilingual/language-registry.json",
        )
        self.assertEqual(
            contracts["text_bearing_nodes_ref"],
            "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json",
        )
        self.assertIn("Works, corpora, inscriptions", labels["text_bearing_node_rule"])


if __name__ == "__main__":
    unittest.main()

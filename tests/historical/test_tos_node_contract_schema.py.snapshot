from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = REPO_ROOT / "ToS" / "contracts" / "tos-node-contract.schema.json"
SOURCE_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "source_node.example.json"
CONCEPT_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "concept_node.example.json"
PRINCIPLE_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "principle_node.example.json"
ZARATHUSTRA_PRINCIPLE_DIR = (
    REPO_ROOT
    / "ToS"
    / "canon"
    / "principle"
    / "friedrich-nietzsche"
    / "thus-spoke-zarathustra"
    / "prologue-1"
)
LINEAGE_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "lineage_node.example.json"
EVENT_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "event_node.example.json"
STATE_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "state_node.example.json"
SUPPORT_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "support_node.example.json"
ANALOGY_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "analogy_node.example.json"
SYNTHESIS_NODE_EXAMPLE_PATH = REPO_ROOT / "ToS" / "public-compatibility" / "synthesis_node.example.json"
ZARATHUSTRA_EVENT_DIR = (
    REPO_ROOT
    / "ToS"
    / "canon"
    / "event"
    / "friedrich-nietzsche"
    / "thus-spoke-zarathustra"
    / "prologue-1"
)
ZARATHUSTRA_STATE_DIR = (
    REPO_ROOT
    / "ToS"
    / "canon"
    / "state"
    / "friedrich-nietzsche"
    / "thus-spoke-zarathustra"
    / "prologue-1"
)
ZARATHUSTRA_SUPPORT_DIR = (
    REPO_ROOT
    / "ToS"
    / "canon"
    / "support"
    / "friedrich-nietzsche"
    / "thus-spoke-zarathustra"
    / "prologue-1"
)
ZARATHUSTRA_ANALOGY_DIR = (
    REPO_ROOT
    / "ToS"
    / "canon"
    / "analogy"
    / "friedrich-nietzsche"
    / "thus-spoke-zarathustra"
    / "prologue-1"
)
ZARATHUSTRA_SYNTHESIS_DIR = (
    REPO_ROOT
    / "ToS"
    / "canon"
    / "synthesis"
    / "friedrich-nietzsche"
    / "thus-spoke-zarathustra"
    / "prologue-1"
)


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def canonical_node_paths(directory: Path) -> list[Path]:
    return sorted(directory.rglob("node.json"))


def canonical_node_ids(directory: Path) -> set[str]:
    return {
        str(load_json(path)["node_id"])
        for path in canonical_node_paths(directory)
    }


class TosNodeContractSchemaTestCase(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        schema = load_json(SCHEMA_PATH)
        assert isinstance(schema, dict)
        cls.validator = Draft202012Validator(schema)

        source_node = load_json(SOURCE_NODE_EXAMPLE_PATH)
        assert isinstance(source_node, dict)
        cls.source_node = source_node

        concept_node = load_json(CONCEPT_NODE_EXAMPLE_PATH)
        assert isinstance(concept_node, dict)
        cls.concept_node = concept_node

        principle_node = load_json(PRINCIPLE_NODE_EXAMPLE_PATH)
        assert isinstance(principle_node, dict)
        cls.principle_node = principle_node

        lineage_node = load_json(LINEAGE_NODE_EXAMPLE_PATH)
        assert isinstance(lineage_node, dict)
        cls.lineage_node = lineage_node

        event_node = load_json(EVENT_NODE_EXAMPLE_PATH)
        assert isinstance(event_node, dict)
        cls.event_node = event_node

        state_node = load_json(STATE_NODE_EXAMPLE_PATH)
        assert isinstance(state_node, dict)
        cls.state_node = state_node

        support_node = load_json(SUPPORT_NODE_EXAMPLE_PATH)
        assert isinstance(support_node, dict)
        cls.support_node = support_node

        analogy_node = load_json(ANALOGY_NODE_EXAMPLE_PATH)
        assert isinstance(analogy_node, dict)
        cls.analogy_node = analogy_node

        synthesis_node = load_json(SYNTHESIS_NODE_EXAMPLE_PATH)
        assert isinstance(synthesis_node, dict)
        cls.synthesis_node = synthesis_node

    def collect_errors(self, payload: object) -> list[str]:
        return [error.message for error in self.validator.iter_errors(payload)]

    def test_source_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.source_node), [])

    def test_concept_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.concept_node), [])

    def test_principle_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.principle_node), [])

    def test_lineage_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.lineage_node), [])

    def test_event_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.event_node), [])

    def test_state_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.state_node), [])

    def test_support_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.support_node), [])

    def test_analogy_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.analogy_node), [])

    def test_synthesis_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.synthesis_node), [])

    def test_all_canonical_tree_nodes_still_validate(self) -> None:
        for path in sorted((REPO_ROOT / "ToS" / "canon").rglob("node.json")):
            with self.subTest(path=path.relative_to(REPO_ROOT).as_posix()):
                self.assertEqual(self.collect_errors(load_json(path)), [])

    def test_unknown_top_level_node_fields_fail_schema_validation(self) -> None:
        payload = copy.deepcopy(self.concept_node)
        payload["unexpected_top_level_field"] = "schema strictness should reject this"

        self.assertNotEqual(self.collect_errors(payload), [])

    def test_zarathustra_route_keeps_canonical_principles_open_for_growth(self) -> None:
        principle_paths = canonical_node_paths(ZARATHUSTRA_PRINCIPLE_DIR)
        self.assertGreater(len(principle_paths), 0)
        principle_ids = canonical_node_ids(ZARATHUSTRA_PRINCIPLE_DIR)
        self.assertNotIn(
            "tos.principle.thus-spoke-zarathustra.prologue.departure-from-reflective-origin",
            principle_ids,
        )

    def test_zarathustra_route_keeps_canonical_events_open_for_growth(self) -> None:
        event_paths = canonical_node_paths(ZARATHUSTRA_EVENT_DIR)
        self.assertGreater(len(event_paths), 0)
        event_ids = canonical_node_ids(ZARATHUSTRA_EVENT_DIR)
        self.assertNotIn(
            "tos.event.thus-spoke-zarathustra.prologue.bee-honey-analogy",
            event_ids,
        )

    def test_zarathustra_route_keeps_canonical_states_open_for_growth(self) -> None:
        state_paths = canonical_node_paths(ZARATHUSTRA_STATE_DIR)
        self.assertGreater(len(state_paths), 0)

    def test_zarathustra_route_keeps_canonical_support_nodes_open_for_growth(self) -> None:
        support_paths = canonical_node_paths(ZARATHUSTRA_SUPPORT_DIR)
        self.assertGreater(len(support_paths), 0)

    def test_zarathustra_route_keeps_bee_honey_as_canonical_analogy(self) -> None:
        analogy_ids = canonical_node_ids(ZARATHUSTRA_ANALOGY_DIR)
        self.assertIn(
            "tos.analogy.thus-spoke-zarathustra.prologue.bee-honey-analogy",
            analogy_ids,
        )

    def test_zarathustra_route_keeps_departure_as_canonical_synthesis_node(self) -> None:
        synthesis_ids = canonical_node_ids(ZARATHUSTRA_SYNTHESIS_DIR)
        self.assertIn(
            "tos.synthesis.thus-spoke-zarathustra.prologue.departure-from-reflective-origin",
            synthesis_ids,
        )

    def test_non_source_nodes_reject_multilingual_witness_payloads(self) -> None:
        payload = copy.deepcopy(self.source_node)
        payload["node_id"] = "tos.concept.becoming"
        payload["node_type"] = "concept"

        self.assertNotEqual(self.collect_errors(payload), [])

    def test_non_source_nodes_reject_translation_tensions(self) -> None:
        payload = copy.deepcopy(self.source_node)
        payload["node_id"] = "tos.context.classical-greece"
        payload["node_type"] = "context"
        payload.pop("language_witnesses", None)

        self.assertNotEqual(self.collect_errors(payload), [])

    def test_duplicate_witness_roles_fail_schema_validation(self) -> None:
        payload = copy.deepcopy(self.source_node)
        payload["language_witnesses"] = [
            copy.deepcopy(payload["language_witnesses"][0]),
            copy.deepcopy(payload["language_witnesses"][0]),
        ]

        self.assertNotEqual(self.collect_errors(payload), [])

    def test_duplicate_witness_languages_fail_schema_validation(self) -> None:
        payload = copy.deepcopy(self.source_node)
        payload["language_witnesses"][1]["language"] = payload["language_witnesses"][0]["language"]

        self.assertNotEqual(self.collect_errors(payload), [])

    def test_additional_language_codes_pass_schema_validation(self) -> None:
        payload = copy.deepcopy(self.source_node)
        payload["language_witnesses"][2]["language"] = "el"

        self.assertEqual(self.collect_errors(payload), [])

    def test_documented_optional_node_layers_can_extend_minimum_contract(self) -> None:
        payload = copy.deepcopy(self.concept_node)
        payload["semantic_field_notes"] = [
            "becoming remains a live interpretive field, not a flat keyword"
        ]
        payload["temporal_context"] = "bounded to the current Zarathustra entry route"

        self.assertEqual(self.collect_errors(payload), [])


if __name__ == "__main__":
    unittest.main()

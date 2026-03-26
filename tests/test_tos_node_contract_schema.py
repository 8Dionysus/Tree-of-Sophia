from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = REPO_ROOT / "schemas" / "tos-node-contract.schema.json"
SOURCE_NODE_EXAMPLE_PATH = REPO_ROOT / "examples" / "source_node.example.json"


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


class TosNodeContractSchemaTestCase(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        schema = load_json(SCHEMA_PATH)
        assert isinstance(schema, dict)
        cls.validator = Draft202012Validator(schema)

        source_node = load_json(SOURCE_NODE_EXAMPLE_PATH)
        assert isinstance(source_node, dict)
        cls.source_node = source_node

    def collect_errors(self, payload: object) -> list[str]:
        return [error.message for error in self.validator.iter_errors(payload)]

    def test_source_node_example_still_validates(self) -> None:
        self.assertEqual(self.collect_errors(self.source_node), [])

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
        payload["language_witnesses"] = [
            copy.deepcopy(payload["language_witnesses"][0]),
            copy.deepcopy(payload["language_witnesses"][1]),
        ]
        payload["language_witnesses"][1]["language"] = payload["language_witnesses"][0]["language"]
        payload["language_witnesses"][1]["role"] = "working_translation"

        self.assertNotEqual(self.collect_errors(payload), [])


if __name__ == "__main__":
    unittest.main()

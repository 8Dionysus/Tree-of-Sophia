from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = REPO_ROOT / "ToS" / "contracts" / "tos-source-home.schema.json"
SOURCE_HOME_PATH = REPO_ROOT / "ToS" / "source_home.manifest.json"


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


class ToSSourceHomeSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        schema = load_json(SCHEMA_PATH)
        assert isinstance(schema, dict)
        cls.validator = Draft202012Validator(schema)

        source_home = load_json(SOURCE_HOME_PATH)
        assert isinstance(source_home, dict)
        cls.source_home = source_home

    def collect_errors(self, payload: object) -> list[str]:
        return [error.message for error in self.validator.iter_errors(payload)]

    def test_source_home_manifest_schema_reference_exists(self) -> None:
        self.assertTrue(SCHEMA_PATH.is_file())
        self.assertEqual(
            self.source_home["$schema"],
            "https://tree-of-sophia.local/ToS/contracts/tos-source-home.schema.json",
        )

    def test_source_home_manifest_validates_against_schema(self) -> None:
        self.assertEqual(self.collect_errors(self.source_home), [])


if __name__ == "__main__":
    unittest.main()

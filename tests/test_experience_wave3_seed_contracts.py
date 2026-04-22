from __future__ import annotations

import copy
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
WAVE3_STEMS = (
    "aoa_experience_candidate_dossier",
    "tos_adoption_boundary_dossier",
    "tos_intake_boundary_decision",
    "tos_no_direct_write_guard",
    "tos_no_runtime_adoption_guard",
    "tos_pattern_review_note",
)


def load_contract(stem: str) -> tuple[dict[str, object], dict[str, object]]:
    schema_path = ROOT / "schemas" / f"{stem}_v1.json"
    example_path = ROOT / "examples" / f"{stem}.example.json"
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    example = json.loads(example_path.read_text(encoding="utf-8"))
    return schema, example


def validation_errors(schema: dict[str, object], value: dict[str, object]) -> list[object]:
    return sorted(Draft202012Validator(schema).iter_errors(value), key=lambda error: list(error.path))


def wrong_type_value(value: object) -> object:
    if isinstance(value, bool):
        return "not-a-boolean"
    if isinstance(value, int) and not isinstance(value, bool):
        return "not-an-integer"
    if isinstance(value, float):
        return "not-a-number"
    if isinstance(value, str):
        return 12345
    if isinstance(value, list):
        return {"not": "an array"}
    if isinstance(value, dict):
        return "not-an-object"
    return "not-null"


class ExperienceWave3SeedContractTests(unittest.TestCase):
    def assert_invalid(self, schema: dict[str, object], value: dict[str, object], label: str) -> None:
        errors = validation_errors(schema, value)
        self.assertTrue(errors, f"{label} unexpectedly validated")

    def test_experience_wave3_examples_match_schemas(self) -> None:
        missing_pairs: list[str] = []
        for stem in WAVE3_STEMS:
            schema_path = ROOT / "schemas" / f"{stem}_v1.json"
            example_path = ROOT / "examples" / f"{stem}.example.json"
            if not schema_path.exists():
                missing_pairs.append(f"{example_path.relative_to(ROOT)} -> {schema_path.relative_to(ROOT)}")
            if not example_path.exists():
                missing_pairs.append(f"{schema_path.relative_to(ROOT)} -> {example_path.relative_to(ROOT)}")
        self.assertFalse(missing_pairs, "missing wave3 contract pair(s): " + ", ".join(missing_pairs))

        self.assertTrue(WAVE3_STEMS)
        for stem in WAVE3_STEMS:
            with self.subTest(stem=stem):
                schema, example = load_contract(stem)
                Draft202012Validator.check_schema(schema)
                errors = validation_errors(schema, example)
                self.assertFalse(errors, f"{stem}: {errors[0].message}" if errors else stem)

    def test_experience_wave3_schemas_reject_escape_hatches(self) -> None:
        self.assertTrue(WAVE3_STEMS)
        for stem in WAVE3_STEMS:
            with self.subTest(stem=stem):
                schema, example = load_contract(stem)

                with_unknown_top = copy.deepcopy(example)
                with_unknown_top["contract_escape"] = True
                self.assert_invalid(schema, with_unknown_top, f"{stem} unknown top-level field")

                refs = example.get("refs")
                if isinstance(refs, dict):
                    with_unknown_ref = copy.deepcopy(example)
                    self.assertIsInstance(with_unknown_ref["refs"], dict)
                    with_unknown_ref["refs"]["contract_escape"] = "loose-ref"
                    self.assert_invalid(schema, with_unknown_ref, f"{stem} unknown refs field")

                payload = example.get("payload")
                if isinstance(payload, dict):
                    with_unknown_payload = copy.deepcopy(example)
                    self.assertIsInstance(with_unknown_payload["payload"], dict)
                    with_unknown_payload["payload"]["contract_escape"] = "loose-payload"
                    self.assert_invalid(schema, with_unknown_payload, f"{stem} unknown payload field")

                    if payload:
                        key = next(iter(payload))
                        with_wrong_payload_type = copy.deepcopy(example)
                        self.assertIsInstance(with_wrong_payload_type["payload"], dict)
                        with_wrong_payload_type["payload"][key] = wrong_type_value(payload[key])
                        self.assert_invalid(schema, with_wrong_payload_type, f"{stem} wrong payload type")


if __name__ == "__main__":
    unittest.main()

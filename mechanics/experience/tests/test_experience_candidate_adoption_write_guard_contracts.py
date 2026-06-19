from __future__ import annotations

import copy
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[3]
EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS = (
    "aoa_experience_candidate_dossier",
    "tos_adoption_boundary_dossier",
    "tos_intake_boundary_decision",
    "tos_no_direct_write_guard",
    "tos_no_runtime_adoption_guard",
    "tos_pattern_review_note",
)
EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_PARTS = {
    "aoa_experience_candidate_dossier": "candidate-review",
    "tos_adoption_boundary_dossier": "adoption-boundary",
    "tos_intake_boundary_decision": "candidate-review",
    "tos_no_direct_write_guard": "write-guards",
    "tos_no_runtime_adoption_guard": "adoption-boundary",
    "tos_pattern_review_note": "pattern-review",
}
GUARDRAIL_BOOLEAN_FIELDS = {
    "authority_required",
    "derived_only",
    "direct_tos_write",
    "direct_write",
    "direct_write_allowed",
    "direct_write_blocked",
    "dossier_allowed",
    "drill_required",
    "kag_may_force_uptake",
    "kag_may_propose",
    "lineage_indexed",
    "meaning_authority",
    "release_required",
    "required_trial",
    "requires_eval_verdict",
    "requires_owner_consent",
    "rollback_required",
    "scar_required",
    "source_theft",
    "submit_only",
}
RATIO_FIELD_HINTS = ("rate", "threshold")
ENUM_ESCAPE_VALUE = "__experience_candidate_adoption_write_guard_not_allowed__"


def load_contract(stem: str) -> tuple[dict[str, object], dict[str, object]]:
    schema_path, example_path = contract_paths(stem)
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    example = json.loads(example_path.read_text(encoding="utf-8"))
    return schema, example


def contract_paths(stem: str) -> tuple[Path, Path]:
    part = EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_PARTS[stem]
    base = ROOT / "mechanics" / "experience" / "parts" / part
    return base / "schemas" / f"{stem}_v1.json", base / "examples" / f"{stem}.example.json"


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


def payload_schema_properties(schema: dict[str, object]) -> dict[str, object]:
    properties = schema.get("properties")
    if not isinstance(properties, dict):
        return {}
    payload = properties.get("payload")
    if not isinstance(payload, dict):
        return {}
    payload_properties = payload.get("properties")
    if not isinstance(payload_properties, dict):
        return {}
    return payload_properties


def array_field_targets(example: dict[str, object]) -> list[tuple[str, str]]:
    targets: list[tuple[str, str]] = []
    for key, value in example.items():
        if isinstance(value, list):
            targets.append(("top", key))
    refs = example.get("refs")
    if isinstance(refs, dict):
        for key, value in refs.items():
            if isinstance(value, list):
                targets.append(("refs", key))
    payload = example.get("payload")
    if isinstance(payload, dict):
        for key, value in payload.items():
            if isinstance(value, list):
                targets.append(("payload", key))
    return targets


def set_section_value(value: dict[str, object], section: str, key: str, replacement: object) -> None:
    if section == "top":
        value[key] = replacement
        return
    nested = value[section]
    if not isinstance(nested, dict):
        raise AssertionError(f"{section} is not an object")
    nested[key] = replacement


class ExperienceCandidateAdoptionWriteGuardContractTests(unittest.TestCase):
    def assert_invalid(self, schema: dict[str, object], value: dict[str, object], label: str) -> None:
        errors = validation_errors(schema, value)
        self.assertTrue(errors, f"{label} unexpectedly validated")

    def test_experience_candidate_adoption_write_guard_examples_match_schemas(self) -> None:
        missing_pairs: list[str] = []
        for stem in EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS:
            schema_path, example_path = contract_paths(stem)
            if not schema_path.exists():
                missing_pairs.append(f"{example_path.relative_to(ROOT)} -> {schema_path.relative_to(ROOT)}")
            if not example_path.exists():
                missing_pairs.append(f"{schema_path.relative_to(ROOT)} -> {example_path.relative_to(ROOT)}")
        self.assertFalse(
            missing_pairs,
            "missing candidate/adoption/write-guard contract pair(s): " + ", ".join(missing_pairs),
        )

        self.assertTrue(EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS)
        for stem in EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS:
            with self.subTest(stem=stem):
                schema, example = load_contract(stem)
                Draft202012Validator.check_schema(schema)
                errors = validation_errors(schema, example)
                self.assertFalse(errors, f"{stem}: {errors[0].message}" if errors else stem)

    def test_experience_candidate_adoption_write_guard_schemas_reject_escape_hatches(self) -> None:
        self.assertTrue(EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS)
        for stem in EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS:
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

                    for key, value in payload.items():
                        with self.subTest(stem=stem, key=key, case="wrong-type"):
                            with_wrong_payload_type = copy.deepcopy(example)
                            self.assertIsInstance(with_wrong_payload_type["payload"], dict)
                            with_wrong_payload_type["payload"][key] = wrong_type_value(value)
                            self.assert_invalid(schema, with_wrong_payload_type, f"{stem} wrong {key} type")

    def test_experience_candidate_adoption_write_guard_schemas_reject_guardrail_boolean_inversions(self) -> None:
        for stem in EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS:
            schema, example = load_contract(stem)
            payload = example.get("payload")
            if not isinstance(payload, dict):
                continue
            for key, value in payload.items():
                if key not in GUARDRAIL_BOOLEAN_FIELDS or not isinstance(value, bool):
                    continue
                with self.subTest(stem=stem, key=key):
                    mutated = copy.deepcopy(example)
                    self.assertIsInstance(mutated["payload"], dict)
                    mutated["payload"][key] = not value
                    self.assert_invalid(schema, mutated, f"{stem} inverted {key}")

    def test_experience_candidate_adoption_write_guard_schemas_reject_invalid_numeric_ranges(self) -> None:
        for stem in EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS:
            schema, example = load_contract(stem)
            payload = example.get("payload")
            if not isinstance(payload, dict):
                continue
            for key, value in payload.items():
                if not isinstance(value, (int, float)) or isinstance(value, bool):
                    continue
                with self.subTest(stem=stem, key=key, case="negative"):
                    mutated = copy.deepcopy(example)
                    self.assertIsInstance(mutated["payload"], dict)
                    mutated["payload"][key] = -1
                    self.assert_invalid(schema, mutated, f"{stem} negative {key}")
                if key == "retention_cycles":
                    with self.subTest(stem=stem, key=key, case="zero"):
                        mutated = copy.deepcopy(example)
                        self.assertIsInstance(mutated["payload"], dict)
                        mutated["payload"][key] = 0
                        self.assert_invalid(schema, mutated, f"{stem} zero {key}")
                if key == "value" or any(hint in key for hint in RATIO_FIELD_HINTS):
                    with self.subTest(stem=stem, key=key, case="above-one"):
                        mutated = copy.deepcopy(example)
                        self.assertIsInstance(mutated["payload"], dict)
                        mutated["payload"][key] = 1.5
                        self.assert_invalid(schema, mutated, f"{stem} out-of-range {key}")

    def test_experience_candidate_adoption_write_guard_schemas_reject_non_string_array_items(self) -> None:
        exercised = 0
        for stem in EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS:
            schema, example = load_contract(stem)
            for section, key in array_field_targets(example):
                exercised += 1
                with self.subTest(stem=stem, section=section, key=key, case="non-string"):
                    mutated = copy.deepcopy(example)
                    set_section_value(mutated, section, key, [12345])
                    self.assert_invalid(schema, mutated, f"{stem} non-string {section}.{key} item")
                with self.subTest(stem=stem, section=section, key=key, case="empty-string"):
                    mutated = copy.deepcopy(example)
                    set_section_value(mutated, section, key, [""])
                    self.assert_invalid(schema, mutated, f"{stem} empty {section}.{key} item")
        self.assertGreater(exercised, 0, "no candidate/adoption/write-guard array fields were exercised")

    def test_experience_candidate_adoption_write_guard_schemas_reject_payload_enum_escape_values(self) -> None:
        for stem in EXPERIENCE_CANDIDATE_ADOPTION_WRITE_GUARD_STEMS:
            schema, example = load_contract(stem)
            payload = example.get("payload")
            if not isinstance(payload, dict):
                continue
            for key, prop in payload_schema_properties(schema).items():
                if not isinstance(prop, dict) or "enum" not in prop or key not in payload:
                    continue
                with self.subTest(stem=stem, key=key):
                    mutated = copy.deepcopy(example)
                    self.assertIsInstance(mutated["payload"], dict)
                    mutated["payload"][key] = ENUM_ESCAPE_VALUE
                    self.assert_invalid(schema, mutated, f"{stem} enum escape {key}")


if __name__ == "__main__":
    unittest.main()

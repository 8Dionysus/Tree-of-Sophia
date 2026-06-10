from __future__ import annotations

import copy
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
ENUM_ESCAPE_VALUE = "__batch4_not_allowed__"

BATCH4_CONTRACTS = (
    ('tos_governance_review_note', 'tos_governance_review_note_v1.json'),
    ('tos_governance_dossier_boundary_v1', 'tos_governance_dossier_boundary_v1.json'),
)
BATCH4_BASE = ROOT / "mechanics" / "experience" / "parts" / "governance-boundary"



def load_contract(stem: str, schema_file: str) -> tuple[dict[str, object], dict[str, object]]:
    schema_path, example_path = contract_paths(stem, schema_file)
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    example = json.loads(example_path.read_text(encoding="utf-8"))
    return schema, example


def contract_paths(stem: str, schema_file: str) -> tuple[Path, Path]:
    return BATCH4_BASE / "schemas" / schema_file, BATCH4_BASE / "examples" / f"{stem}.example.json"


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


def const_escape_value(value: object) -> object:
    if isinstance(value, bool):
        return not value
    if isinstance(value, int) and not isinstance(value, bool):
        return value + 1
    if isinstance(value, float):
        return value + 1.0
    if isinstance(value, str):
        return f"{value}{ENUM_ESCAPE_VALUE}"
    return ENUM_ESCAPE_VALUE


def get_path(value: object, path: tuple[object, ...]) -> object:
    cursor = value
    for part in path:
        if isinstance(part, int):
            assert isinstance(cursor, list)
            cursor = cursor[part]
        else:
            assert isinstance(cursor, dict)
            cursor = cursor[part]
    return cursor


def set_path(value: object, path: tuple[object, ...], replacement: object) -> None:
    cursor = value
    for part in path[:-1]:
        if isinstance(part, int):
            assert isinstance(cursor, list)
            cursor = cursor[part]
        else:
            assert isinstance(cursor, dict)
            cursor = cursor[part]
    last = path[-1]
    if isinstance(last, int):
        assert isinstance(cursor, list)
        cursor[last] = replacement
    else:
        assert isinstance(cursor, dict)
        cursor[last] = replacement


def delete_path(value: object, path: tuple[object, ...]) -> None:
    cursor = value
    for part in path[:-1]:
        if isinstance(part, int):
            assert isinstance(cursor, list)
            cursor = cursor[part]
        else:
            assert isinstance(cursor, dict)
            cursor = cursor[part]
    last = path[-1]
    if isinstance(last, int):
        assert isinstance(cursor, list)
        del cursor[last]
    else:
        assert isinstance(cursor, dict)
        del cursor[last]


def walk_values(value: object, path: tuple[object, ...] = ()) -> list[tuple[tuple[object, ...], object]]:
    found: list[tuple[tuple[object, ...], object]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            child_path = (*path, key)
            found.append((child_path, child))
            found.extend(walk_values(child, child_path))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            child_path = (*path, index)
            found.append((child_path, child))
            found.extend(walk_values(child, child_path))
    return found


def object_paths(value: object, path: tuple[object, ...] = ()) -> list[tuple[object, ...]]:
    found: list[tuple[object, ...]] = []
    if isinstance(value, dict):
        found.append(path)
        for key, child in value.items():
            found.extend(object_paths(child, (*path, key)))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            found.extend(object_paths(child, (*path, index)))
    return found


def array_paths(value: object, path: tuple[object, ...] = ()) -> list[tuple[object, ...]]:
    found: list[tuple[object, ...]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            found.extend(array_paths(child, (*path, key)))
    elif isinstance(value, list):
        found.append(path)
        for index, child in enumerate(value):
            found.extend(array_paths(child, (*path, index)))
    return found


def schema_type(schema: object) -> object:
    return schema.get("type") if isinstance(schema, dict) else None


def schema_properties(schema: object) -> dict[str, object]:
    if not isinstance(schema, dict):
        return {}
    props = schema.get("properties")
    return props if isinstance(props, dict) else {}


def required_paths(schema: object, example: object, path: tuple[object, ...] = ()) -> list[tuple[object, ...]]:
    found: list[tuple[object, ...]] = []
    if not isinstance(schema, dict):
        return found
    if schema_type(schema) == "object" and isinstance(example, dict):
        required = schema.get("required")
        properties = schema_properties(schema)
        if isinstance(required, list):
            for key in required:
                if isinstance(key, str) and key in example:
                    found.append((*path, key))
        for key, prop in properties.items():
            if key in example:
                found.extend(required_paths(prop, example[key], (*path, key)))
    if schema_type(schema) == "array" and isinstance(example, list) and example:
        found.extend(required_paths(schema.get("items"), example[0], (*path, 0)))
    return found


def constrained_paths(schema: object, example: object, keyword: str, path: tuple[object, ...] = ()) -> list[tuple[tuple[object, ...], object]]:
    found: list[tuple[tuple[object, ...], object]] = []
    if not isinstance(schema, dict):
        return found
    if keyword in schema:
        found.append((path, schema[keyword]))
    if schema_type(schema) == "object" and isinstance(example, dict):
        for key, prop in schema_properties(schema).items():
            if key in example:
                found.extend(constrained_paths(prop, example[key], keyword, (*path, key)))
    if schema_type(schema) == "array" and isinstance(example, list) and example:
        found.extend(constrained_paths(schema.get("items"), example[0], keyword, (*path, 0)))
    return found


class ExperienceBatch4ContractTests(unittest.TestCase):
    def assert_invalid(self, schema: dict[str, object], value: dict[str, object], label: str) -> None:
        errors = validation_errors(schema, value)
        self.assertTrue(errors, f"{label} unexpectedly validated")

    def test_experience_batch4_examples_match_schemas(self) -> None:
        self.assertTrue(BATCH4_CONTRACTS)
        missing_pairs: list[str] = []
        for stem, schema_file in BATCH4_CONTRACTS:
            schema_path, example_path = contract_paths(stem, schema_file)
            if not schema_path.exists():
                missing_pairs.append(f"{example_path.relative_to(ROOT)} -> {schema_path.relative_to(ROOT)}")
            if not example_path.exists():
                missing_pairs.append(f"{schema_path.relative_to(ROOT)} -> {example_path.relative_to(ROOT)}")
        self.assertFalse(missing_pairs, "missing batch4 contract pair(s): " + ", ".join(missing_pairs))

        for stem, schema_file in BATCH4_CONTRACTS:
            with self.subTest(stem=stem):
                schema, example = load_contract(stem, schema_file)
                Draft202012Validator.check_schema(schema)
                errors = validation_errors(schema, example)
                self.assertFalse(errors, f"{stem}: {errors[0].message}" if errors else stem)

    def test_experience_batch4_schemas_reject_unknown_fields(self) -> None:
        exercised = 0
        for stem, schema_file in BATCH4_CONTRACTS:
            schema, example = load_contract(stem, schema_file)
            for path in object_paths(example):
                exercised += 1
                with self.subTest(stem=stem, path=".".join(str(part) for part in path) or "top"):
                    mutated = copy.deepcopy(example)
                    target = get_path(mutated, path) if path else mutated
                    self.assertIsInstance(target, dict)
                    target["contract_escape"] = "loose-field"
                    self.assert_invalid(schema, mutated, f"{stem} unknown field at {path}")
        self.assertGreater(exercised, 0, "no batch4 object fields were exercised")

    def test_experience_batch4_schemas_reject_wrong_types_for_every_field(self) -> None:
        exercised = 0
        for stem, schema_file in BATCH4_CONTRACTS:
            schema, example = load_contract(stem, schema_file)
            for path, value in walk_values(example):
                exercised += 1
                with self.subTest(stem=stem, path=".".join(str(part) for part in path)):
                    mutated = copy.deepcopy(example)
                    set_path(mutated, path, wrong_type_value(value))
                    self.assert_invalid(schema, mutated, f"{stem} wrong type at {path}")
        self.assertGreater(exercised, 0, "no batch4 fields were exercised")

    def test_experience_batch4_schemas_reject_missing_required_fields(self) -> None:
        exercised = 0
        for stem, schema_file in BATCH4_CONTRACTS:
            schema, example = load_contract(stem, schema_file)
            for path in required_paths(schema, example):
                exercised += 1
                with self.subTest(stem=stem, path=".".join(str(part) for part in path)):
                    mutated = copy.deepcopy(example)
                    delete_path(mutated, path)
                    self.assert_invalid(schema, mutated, f"{stem} missing required field at {path}")
        self.assertGreater(exercised, 0, "no batch4 required fields were exercised")

    def test_experience_batch4_schemas_reject_bad_array_items(self) -> None:
        exercised = 0
        for stem, schema_file in BATCH4_CONTRACTS:
            schema, example = load_contract(stem, schema_file)
            for path in array_paths(example):
                value = get_path(example, path)
                if not isinstance(value, list):
                    continue
                exercised += 1
                replacement = [wrong_type_value(value[0])] if value else [12345]
                with self.subTest(stem=stem, path=".".join(str(part) for part in path), case="wrong-item"):
                    mutated = copy.deepcopy(example)
                    set_path(mutated, path, replacement)
                    self.assert_invalid(schema, mutated, f"{stem} wrong array item at {path}")
                if not value or isinstance(value[0], str):
                    with self.subTest(stem=stem, path=".".join(str(part) for part in path), case="empty-string"):
                        mutated = copy.deepcopy(example)
                        set_path(mutated, path, [""])
                        self.assert_invalid(schema, mutated, f"{stem} empty string array item at {path}")
        self.assertGreater(exercised, 0, "no batch4 array fields were exercised")

    def test_experience_batch4_schemas_reject_const_escapes(self) -> None:
        exercised = 0
        for stem, schema_file in BATCH4_CONTRACTS:
            schema, example = load_contract(stem, schema_file)
            for path, _const_value in constrained_paths(schema, example, "const"):
                if not path:
                    continue
                exercised += 1
                with self.subTest(stem=stem, path=".".join(str(part) for part in path)):
                    mutated = copy.deepcopy(example)
                    set_path(mutated, path, const_escape_value(get_path(example, path)))
                    self.assert_invalid(schema, mutated, f"{stem} const escape at {path}")
        self.assertGreater(exercised, 0, "no batch4 const fields were exercised")

    def test_experience_batch4_schemas_reject_enum_escapes(self) -> None:
        exercised = 0
        for stem, schema_file in BATCH4_CONTRACTS:
            schema, example = load_contract(stem, schema_file)
            for path, _enum_values in constrained_paths(schema, example, "enum"):
                if not path:
                    continue
                exercised += 1
                with self.subTest(stem=stem, path=".".join(str(part) for part in path)):
                    mutated = copy.deepcopy(example)
                    set_path(mutated, path, ENUM_ESCAPE_VALUE)
                    self.assert_invalid(schema, mutated, f"{stem} enum escape at {path}")
        self.assertGreater(exercised, 0, "no batch4 enum fields were exercised")

    def test_experience_batch4_schemas_reject_invalid_numeric_ranges(self) -> None:
        for stem, schema_file in BATCH4_CONTRACTS:
            schema, example = load_contract(stem, schema_file)
            for path, value in walk_values(example):
                if not isinstance(value, (int, float)) or isinstance(value, bool):
                    continue
                with self.subTest(stem=stem, path=".".join(str(part) for part in path), case="negative"):
                    mutated = copy.deepcopy(example)
                    set_path(mutated, path, -1)
                    self.assert_invalid(schema, mutated, f"{stem} negative number at {path}")


if __name__ == "__main__":
    unittest.main()

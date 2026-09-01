import copy
import importlib.util
import json
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
LAB_ROOT = (
    REPO_ROOT
    / "ToS"
    / "research-packets"
    / "foundation-laboratory-2026-07"
    / "generic-xml-resource-inventory-uxlc-abc-v1"
)


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


EVALUATOR = load_module("tos_uxlc_evaluate_lab", LAB_ROOT / "evaluate_lab.py")
CONSUMER = load_module("tos_uxlc_consume_owner", LAB_ROOT / "consume_owner.py")


class GenericXmlUxLcLabContractTests(unittest.TestCase):
    def test_consumer_requires_complete_unique_selection_keys(self) -> None:
        observations = {
            "processes": [
                {
                    "candidate": "A",
                    "selection_kind": "fixture",
                    "selection_id": "one",
                    "exit_code": 0,
                    "output_sha256": "a-run-1",
                },
                {
                    "candidate": "A",
                    "selection_kind": "fixture",
                    "selection_id": "one",
                    "exit_code": 0,
                    "output_sha256": "a-run-2",
                },
                {
                    "candidate": "B",
                    "selection_kind": "fixture",
                    "selection_id": "two",
                    "exit_code": 0,
                    "output_sha256": "b-run-1",
                },
            ]
        }
        complete = {
            "checks": [
                {
                    "candidate": "A",
                    "selection_kind": "fixture",
                    "selection_id": "one",
                    "output_sha256": "a-run-1",
                },
                {
                    "candidate": "B",
                    "selection_kind": "fixture",
                    "selection_id": "two",
                    "output_sha256": "b-run-1",
                },
            ]
        }
        result = EVALUATOR.verify_consumer_output_bindings(observations, complete)
        self.assertTrue(result["ok"])
        self.assertEqual(2, result["observed_unique_selection_count"])

        incomplete = copy.deepcopy(complete)
        incomplete["checks"].pop()
        result = EVALUATOR.verify_consumer_output_bindings(observations, incomplete)
        self.assertFalse(result["ok"])
        self.assertIn("B:fixture:two", result["missing_selection_keys"])

        duplicated = copy.deepcopy(complete)
        duplicated["checks"].append(copy.deepcopy(duplicated["checks"][0]))
        result = EVALUATOR.verify_consumer_output_bindings(observations, duplicated)
        self.assertFalse(result["ok"])
        self.assertIn("A:fixture:one", result["duplicate_consumer_selection_keys"])

    def test_provider_projection_requires_exact_source_membership_for_c_and_bc(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "PC1-uxlc-shape")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))

        c_path = LAB_ROOT / "public-synthetic" / "run-1" / "c" / "PC1-uxlc-shape.json"
        c_payload = json.loads(c_path.read_text(encoding="utf-8"))
        result = CONSUMER.validate_projection(c_payload, root, None)
        self.assertEqual(7, result["expected_resource_count"])
        self.assertEqual(7, result["unique_resource_count"])

        incomplete = copy.deepcopy(c_payload)
        incomplete["resources"].pop()
        with self.assertRaisesRegex(ValueError, "incomplete or duplicated"):
            CONSUMER.validate_projection(incomplete, root, None)

        duplicated = copy.deepcopy(c_payload)
        duplicated["resources"].append(copy.deepcopy(duplicated["resources"][0]))
        with self.assertRaisesRegex(ValueError, "incomplete or duplicated"):
            CONSUMER.validate_projection(duplicated, root, None)

        bc_path = LAB_ROOT / "public-synthetic" / "run-1" / "bc" / "PC1-uxlc-shape.json"
        bc_payload = json.loads(bc_path.read_text(encoding="utf-8"))
        result = CONSUMER.validate_projection(bc_payload["projection"], root, bc_payload["owner"])
        self.assertEqual(7, result["expected_resource_count"])
        self.assertEqual(7, result["unique_resource_count"])


if __name__ == "__main__":
    unittest.main()

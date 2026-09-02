import copy
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock


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

    def test_source_value_controls_include_all_available_exact_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = Path(temp_dir)
            selected = temp_root / "selected.xml"
            replay = temp_root / "replay.xml"
            selected.write_text("<root><value>selected-only</value></root>", encoding="utf-8")
            replay.write_text("<root><value>replay-only</value></root>", encoding="utf-8")

            values = EVALUATOR.exact_source_values_from_manifest(
                {
                    "exact_sources": {
                        "selected": {"path": str(selected)},
                        "replay": {"path": str(replay)},
                    }
                }
            )

        self.assertIn("selected-only", values)
        self.assertIn("replay-only", values)

    def test_source_value_controls_have_deterministic_tie_breaking(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            source = Path(temp_dir) / "source.xml"
            source.write_text(
                "<root><value>bbb</value><value>aaa</value></root>",
                encoding="utf-8",
            )
            values = EVALUATOR.exact_source_values_from_manifest(
                {"exact_sources": {"selected": {"path": str(source)}}}
            )

        self.assertEqual(["aaa", "bbb"], values)

    def test_private_output_posture_requires_0600_and_gitignored_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            private_root = Path(temp_dir) / "private"
            output = private_root / "outputs" / "selected" / "run-1" / "a.json"
            output.parent.mkdir(parents=True)
            output.write_text("{}", encoding="utf-8")
            observations = {
                "processes": [
                    {
                        "selection_kind": "source",
                        "output_scope": "absolute",
                        "output_ref": str(output),
                    }
                ]
            }
            manifest = {"private_output_root": str(private_root)}

            os.chmod(output, 0o644)
            with mock.patch.object(EVALUATOR, "gitignored", return_value=True):
                result = EVALUATOR.verify_private_output_posture(observations, manifest)
            self.assertFalse(result["ok"])
            self.assertTrue(any("file-mode-0644" in failure for failure in result["failures"]))

            os.chmod(output, 0o600)
            with mock.patch.object(EVALUATOR, "gitignored", return_value=False):
                result = EVALUATOR.verify_private_output_posture(observations, manifest)
            self.assertFalse(result["ok"])
            self.assertTrue(any("not-ignored" in failure for failure in result["failures"]))

            with mock.patch.object(EVALUATOR, "gitignored", return_value=True):
                result = EVALUATOR.verify_private_output_posture(observations, manifest)
            self.assertTrue(result["ok"])

    def test_tracked_output_scan_covers_the_complete_tracked_lab_scope(self) -> None:
        with tempfile.TemporaryDirectory(dir=EVALUATOR.LAB_DIR) as temp_dir:
            tracked_output = Path(temp_dir) / "tracked-output.md"
            tracked_output.write_text(
                "replay-only-sensitive-value\n",
                encoding="utf-8",
            )
            with mock.patch.object(
                EVALUATOR,
                "tracked_lab_files",
                return_value=[tracked_output],
            ):
                result = EVALUATOR.scan_tracked_generated_for_source_values(
                    ["replay-only-sensitive-value"],
                    {},
                )

        self.assertFalse(result["ok"])
        self.assertEqual(1, result["tracked_file_count"])
        self.assertEqual(1, result["scanned_file_count"])
        self.assertTrue(
            any(
                item.endswith("/tracked-output.md:source-value-1")
                for item in result["leaks"]
            )
        )

    def test_tracked_output_scan_does_not_republish_manifest_control_collisions(self) -> None:
        manifest = json.loads(
            (LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8")
        )
        source_value = "Unicode/XML Leningrad Codex"
        with mock.patch.object(
            EVALUATOR,
            "tracked_lab_files",
            return_value=[EVALUATOR.INPUT_PATH],
        ):
            result = EVALUATOR.scan_tracked_generated_for_source_values(
                [source_value],
                manifest,
            )

        self.assertTrue(result["ok"])
        self.assertEqual(1, result["manifest_control_collision_count"])
        self.assertNotIn("manifest_control_collisions", result)
        self.assertNotIn(source_value, json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    unittest.main()

import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
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

    def test_consumer_rejects_duplicate_projection_resource_ids(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "PC1-uxlc-shape")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "c" / "PC1-uxlc-shape.json").read_text(
                encoding="utf-8"
            )
        )
        payload["resources"][1]["resource_id"] = payload["resources"][0]["resource_id"]
        with self.assertRaisesRegex(ValueError, "resource IDs must be unique"):
            CONSUMER.validate_projection(payload, root, None)

    def test_consumer_rejects_non_opaque_a_resource_shape(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "P1-no-namespace")
        source = fixture["xml"].encode("utf-8")
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "a" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        payload["resources"][0]["resource_kind"] = "xml_element"
        with self.assertRaisesRegex(ValueError, "A resource shape mismatch"):
            CONSUMER.validate_candidate(
                payload,
                source,
                expected_candidate="A",
                expected_input_id=fixture["id"],
            )

        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "a" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        payload["resources"].append(copy.deepcopy(payload["resources"][0]))
        with self.assertRaisesRegex(ValueError, "A resource shape mismatch"):
            CONSUMER.validate_candidate(
                payload,
                source,
                expected_candidate="A",
                expected_input_id=fixture["id"],
            )

    def test_consumer_requires_xml_element_resource_kind_for_b(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "P1-no-namespace")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "b" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        payload["resources"][0]["resource_kind"] = "provider_word"
        with self.assertRaisesRegex(ValueError, "resource kind mismatch"):
            CONSUMER.validate_b(payload, root)

    def test_consumer_rejects_payload_candidate_and_file_identity_mismatch(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "P1-no-namespace")
        source = fixture["xml"].encode("utf-8")
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "b" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        with self.assertRaisesRegex(ValueError, "requested candidate"):
            CONSUMER.validate_candidate(
                payload,
                source,
                expected_candidate="A",
                expected_input_id=fixture["id"],
            )

        payload["file_binding"]["file_id"] = "tos.file.sha256.wrong"
        with self.assertRaisesRegex(ValueError, "file ID mismatch"):
            CONSUMER.validate_candidate(
                payload,
                source,
                expected_candidate="B",
                expected_input_id=fixture["id"],
            )

        payload["file_binding"]["file_id"] = f"tos.file.sha256.{payload['file_binding']['sha256']}"
        payload["file_binding"]["input_id"] = "another-selection"
        with self.assertRaisesRegex(ValueError, "input ID mismatch"):
            CONSUMER.validate_candidate(
                payload,
                source,
                expected_candidate="B",
                expected_input_id=fixture["id"],
            )

    def test_consumer_rejects_duplicate_b_resource_ids(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "P1-no-namespace")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "b" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        payload["resources"][1]["resource_id"] = payload["resources"][0]["resource_id"]
        with self.assertRaisesRegex(ValueError, "resource IDs must be unique"):
            CONSUMER.validate_b(payload, root)

    def test_consumer_binds_provider_context_to_registered_selection(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "PC1-uxlc-shape")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "c" / "PC1-uxlc-shape.json").read_text(
                encoding="utf-8"
            )
        )
        registered_context = copy.deepcopy(payload["provider_context"])
        payload["provider_context"]["selector"] = "wrong-selector"
        with self.assertRaisesRegex(ValueError, "registered selection"):
            CONSUMER.validate_projection(payload, root, None, registered_context)

    def test_consumer_recomputes_provider_summary(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "PC1-uxlc-shape")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "c" / "PC1-uxlc-shape.json").read_text(
                encoding="utf-8"
            )
        )
        payload["summary"]["resource_count"] = 999
        with self.assertRaisesRegex(ValueError, "summary mismatch"):
            CONSUMER.validate_projection(payload, root, None)

    def test_consumer_recomputes_complete_b_summary(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "P1-no-namespace")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "b" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        payload["summary"]["max_depth"] = 999
        with self.assertRaisesRegex(ValueError, "B summary mismatch"):
            CONSUMER.validate_b(payload, root)

        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "b" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        payload["summary"]["namespace_uris"] = ["urn:wrong"]
        with self.assertRaisesRegex(ValueError, "B summary mismatch"):
            CONSUMER.validate_b(payload, root)

    def test_consumer_binds_declared_b_scope_values(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "P1-no-namespace")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "b" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        payload["scope"]["path_identity"] = "element ordinal only"
        with self.assertRaisesRegex(ValueError, "B scope value mismatch"):
            CONSUMER.validate_b(payload, root)

    def test_consumer_binds_candidate_lab_and_schema(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "P1-no-namespace")
        source = fixture["xml"].encode("utf-8")
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "b" / "P1-no-namespace.json").read_text(
                encoding="utf-8"
            )
        )
        kwargs = {
            "expected_candidate": "B",
            "expected_input_id": fixture["id"],
            "expected_lab_id": manifest["lab_id"],
            "expected_schema_version": CONSUMER.EXPECTED_SCHEMA_VERSIONS["B"],
        }
        payload["lab_id"] = "tos.lab.other.v1"
        with self.assertRaisesRegex(ValueError, "payload lab ID mismatch"):
            CONSUMER.validate_candidate(payload, source, **kwargs)

        payload["lab_id"] = manifest["lab_id"]
        payload["schema_version"] = CONSUMER.EXPECTED_SCHEMA_VERSIONS["A"]
        with self.assertRaisesRegex(ValueError, "payload schema version mismatch"):
            CONSUMER.validate_candidate(payload, source, **kwargs)

    def test_consumer_applies_b_claim_posture_to_bc_owner(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "PC1-uxlc-shape")
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "bc" / "PC1-uxlc-shape.json").read_text(
                encoding="utf-8"
            )
        )
        payload["owner"]["cross_file_identity_claimed"] = True
        with self.assertRaisesRegex(ValueError, "BC owner claim posture mismatch"):
            CONSUMER.validate_candidate(
                payload,
                fixture["xml"].encode("utf-8"),
                expected_candidate="BC",
                expected_input_id=fixture["id"],
                expected_lab_id=manifest["lab_id"],
                expected_schema_version=CONSUMER.EXPECTED_SCHEMA_VERSIONS["BC"],
            )

    def test_consumer_requires_b_as_generic_projection_owner(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "PC1-uxlc-shape")
        root = CONSUMER.strict_parse(fixture["xml"].encode("utf-8"))
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "bc" / "PC1-uxlc-shape.json").read_text(
                encoding="utf-8"
            )
        )
        payload["projection"]["generic_owner_candidate"] = "A"
        with self.assertRaisesRegex(ValueError, "generic projection owner"):
            CONSUMER.validate_candidate(
                payload,
                fixture["xml"].encode("utf-8"),
                expected_candidate="BC",
                expected_input_id=fixture["id"],
            )

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

    def test_source_value_controls_include_non_element_xml_content(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            source = Path(temp_dir) / "source.xml"
            source.write_text(
                "<root>before<!--comment-secret-->after<?pi pi-secret?>tail</root>",
                encoding="utf-8",
            )
            values = EVALUATOR.exact_source_values_from_manifest(
                {"exact_sources": {"selected": {"path": str(source)}}}
            )

        self.assertIn("comment-secret", values)
        self.assertIn("pi-secret", values)
        self.assertIn("after", values)
        self.assertIn("tail", values)

    def test_short_source_values_match_inside_generated_strings(self) -> None:
        self.assertEqual(
            ["alpha"],
            EVALUATOR.source_value_hits(["alpha"], ["prefix-alpha-suffix"]),
        )

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

    def test_tracked_json_scan_matches_short_source_values_inside_strings(self) -> None:
        with tempfile.TemporaryDirectory(dir=EVALUATOR.LAB_DIR) as temp_dir:
            tracked_output = Path(temp_dir) / "tracked-output.json"
            tracked_output.write_text(
                '{"value": "prefix-alpha-suffix"}\n',
                encoding="utf-8",
            )
            with mock.patch.object(
                EVALUATOR,
                "tracked_lab_files",
                return_value=[tracked_output],
            ):
                result = EVALUATOR.scan_tracked_generated_for_source_values(
                    ["alpha"],
                    {},
                )

        self.assertFalse(result["ok"])
        self.assertTrue(
            any(
                item.endswith("/tracked-output.json:source-value-1")
                for item in result["leaks"]
            )
        )

    def test_tracked_text_scan_matches_delimited_short_source_values(self) -> None:
        with tempfile.TemporaryDirectory(dir=EVALUATOR.LAB_DIR) as temp_dir:
            tracked_output = Path(temp_dir) / "tracked-output.md"
            tracked_output.write_text("prefix-alpha-suffix\n", encoding="utf-8")
            with mock.patch.object(
                EVALUATOR,
                "tracked_lab_files",
                return_value=[tracked_output],
            ):
                result = EVALUATOR.scan_tracked_generated_for_source_values(
                    ["alpha"],
                    {},
                )

        self.assertFalse(result["ok"])
        self.assertTrue(
            any(
                item.endswith("/tracked-output.md:source-value-1")
                for item in result["leaks"]
            )
        )

    def test_malformed_fixture_fallback_scans_text_and_attributes(self) -> None:
        manifest = {
            "fixtures": [
                {
                    "id": "malformed",
                    "xml": '<root secret="private-secret"><item>private-secret</root>',
                }
            ],
            "security_fixtures": [],
        }
        strings = EVALUATOR.fixture_content_strings(manifest)
        self.assertIn("private-secret", strings)
        self.assertEqual(["private-secret"], EVALUATOR.source_value_hits(["private-secret"], strings))

    def test_evaluator_recomputes_b_topology_summary(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        fixture = next(item for item in manifest["fixtures"] if item["id"] == "P2-prefix-a")
        payload = json.loads(
            (LAB_ROOT / "public-synthetic" / "run-1" / "b" / "P2-prefix-a.json").read_text(
                encoding="utf-8"
            )
        )
        source = fixture["xml"].encode("utf-8")
        self.assertTrue(EVALUATOR.verify_topology_summary(payload, source)["ok"])
        payload["summary"]["ordered_topology_sha256"] = "wrong"
        self.assertFalse(EVALUATOR.verify_topology_summary(payload, source)["ok"])

    def test_authority_boundary_must_remain_negative_and_nonempty(self) -> None:
        self.assertFalse(EVALUATOR.non_authoritative_boundary_is_valid(""))
        self.assertFalse(
            EVALUATOR.non_authoritative_boundary_is_valid("this is canonical authority")
        )
        self.assertTrue(
            EVALUATOR.non_authoritative_boundary_is_valid(
                "generic return only; no accepted source-text or publication authority"
            )
        )

    def test_fingerprint_detection_is_not_limited_to_one_field_spelling(self) -> None:
        resources = [{"resource_id": "x", "value_sha256": "deadbeef"}]
        self.assertTrue(EVALUATOR.find_source_content_fingerprint_keys(resources))
        self.assertTrue(
            any(
                "value_sha256" in item
                for item in EVALUATOR.find_source_content_fingerprint_keys(resources)
            )
        )

    def test_word_identity_claims_cover_provider_payloads(self) -> None:
        payloads = {
            "selected:C": {
                "candidate": "C",
                "intrinsic_or_cross_corpus_word_ids_claimed": False,
                "accepted_structure_claimed": False,
            },
            "replay:C": {
                "candidate": "C",
                "intrinsic_or_cross_corpus_word_ids_claimed": True,
                "accepted_structure_claimed": False,
            },
        }
        result = EVALUATOR.verify_word_identity_claims(payloads)
        self.assertFalse(result["ok"])
        self.assertIn("replay:C", result["invalid_labels"])

    def test_g12_rejects_content_equality_claims_in_observed_payloads(self) -> None:
        result = EVALUATOR.verify_content_equality_claims(
            {"B:fixture:P1": {"content_equality_claimed": True}}
        )
        self.assertFalse(result["ok"])
        self.assertIn("B:fixture:P1", result["invalid_labels"])

    def test_consumer_independence_is_derived_from_source(self) -> None:
        freeze = {"frozen_files": []}
        consumer_ref = EVALUATOR.CONSUMER_SOURCE_PATH.relative_to(EVALUATOR.REPO_ROOT).as_posix()
        source_text = EVALUATOR.CONSUMER_SOURCE_PATH.read_text(encoding="utf-8")
        freeze["frozen_files"].append(
            {"ref": consumer_ref, "sha256": EVALUATOR.sha256_bytes(source_text.encode("utf-8"))}
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            bad_source = Path(temp_dir) / "consume_owner.py"
            bad_source.write_text("import build_candidate\n", encoding="utf-8")
            with mock.patch.object(EVALUATOR, "CONSUMER_SOURCE_PATH", bad_source):
                result = EVALUATOR.verify_consumer_independence(freeze)
        self.assertFalse(result["ok"])
        self.assertTrue(result["forbidden_imports"])

    def test_admission_boundary_reports_changes_outside_allowlist(self) -> None:
        manifest = {
            "public_contract_control": {
                "admission_boundary": {
                    "baseline_ref": "base",
                    "roots": ["ToS/canon"],
                    "baseline_tree_ids": {"ToS/canon": "tree"},
                    "allowed_changed_paths": [],
                }
            }
        }
        with mock.patch.object(EVALUATOR, "git_tree_id", return_value="tree"), mock.patch.object(
            EVALUATOR,
            "git_changed_paths_since",
            return_value=["ToS/canon/example-anchor.json"],
        ):
            result = EVALUATOR.verify_admission_boundaries(manifest)
        self.assertFalse(result["ok"])
        self.assertEqual(["ToS/canon/example-anchor.json"], result["unexpected_changed_paths"])

    def test_admission_boundary_includes_worktree_and_untracked_paths(self) -> None:
        diff = subprocess.CompletedProcess(
            ["git"],
            0,
            stdout="ToS/canon/committed.json\nToS/canon/staged.json\n",
            stderr="",
        )
        untracked = subprocess.CompletedProcess(
            ["git"],
            0,
            stdout="ToS/canon/untracked.json\n",
            stderr="",
        )
        ignored = subprocess.CompletedProcess(
            ["git"],
            0,
            stdout="ToS/canon/ignored.json\n",
            stderr="",
        )
        with mock.patch.object(EVALUATOR.subprocess, "run", side_effect=[diff, untracked, ignored]):
            result = EVALUATOR.git_changed_paths_since("base", ["ToS/canon"])
        self.assertEqual(
            [
                "ToS/canon/committed.json",
                "ToS/canon/ignored.json",
                "ToS/canon/staged.json",
                "ToS/canon/untracked.json",
            ],
            result,
        )

    def test_evaluator_checks_parser_posture_on_every_candidate_output(self) -> None:
        manifest = json.loads((LAB_ROOT / "input-manifest.json").read_text(encoding="utf-8"))
        expected = manifest["parser_posture"]
        payloads = {
            "A:fixture:P1": {"candidate": "A", "parser_posture": copy.deepcopy(expected)},
            "B:fixture:P1": {"candidate": "B", "parser_posture": copy.deepcopy(expected)},
            "C:fixture:PC1": {"candidate": "C", "parser_posture": copy.deepcopy(expected)},
            "BC:fixture:PC1": {"candidate": "BC"},
            "BC:fixture:PC1:owner": {
                "candidate": "B",
                "parser_posture": copy.deepcopy(expected),
            },
            "BC:fixture:PC1:projection": {"schema_version": "projection"},
        }
        result = EVALUATOR.verify_observed_parser_postures(payloads, expected)
        self.assertTrue(result["ok"])
        payloads["A:fixture:P1"]["parser_posture"]["resolve_entities"] = True
        result = EVALUATOR.verify_observed_parser_postures(payloads, expected)
        self.assertFalse(result["ok"])
        self.assertIn("A:fixture:P1", result["invalid_labels"])

    def test_replay_binding_requires_current_digest_and_size(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            replay = Path(temp_dir) / "replay.xml"
            replay.write_bytes(b"<root/>")
            digest = EVALUATOR.sha256_path(replay)
            manifest = {"exact_sources": {"replay": {"path": str(replay)}}}
            receipt = {
                "sources": {
                    "replay": {
                        "source_sha256": digest,
                        "source_bytes": replay.stat().st_size,
                    }
                }
            }
            self.assertTrue(EVALUATOR.verify_replay_binding(manifest, receipt)["ok"])
            replay.write_bytes(b"<root><changed/></root>")
            self.assertFalse(EVALUATOR.verify_replay_binding(manifest, receipt)["ok"])

    def test_selected_receipt_binding_requires_current_digest_and_size(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            selected = Path(temp_dir) / "selected.xml"
            selected.write_bytes(b"<root/>")
            digest = EVALUATOR.sha256_path(selected)
            manifest = {"exact_sources": {"selected": {"path": str(selected)}}}
            receipt = {
                "sources": {
                    "selected": {
                        "source_sha256": digest,
                        "source_bytes": selected.stat().st_size,
                    }
                }
            }
            self.assertTrue(
                EVALUATOR.verify_source_receipt_binding(manifest, receipt, "selected")["ok"]
            )
            selected.write_bytes(b"<root><changed/></root>")
            self.assertFalse(
                EVALUATOR.verify_source_receipt_binding(manifest, receipt, "selected")["ok"]
            )

    def test_observed_authority_boundary_covers_nested_candidate_payloads(self) -> None:
        payloads = {
            "ok": {
                "authority_boundary": "mechanical output only; no source-text, semantic, graph, canon or publication authority",
                "owner": {
                    "authority_boundary": "generic owner only; no source-text, semantic, graph, canon or publication authority"
                },
                "projection": {
                    "authority_boundary": "provider navigation only; no source-text, semantic, graph, canon or publication authority"
                },
            },
            "bad": {
                "authority_boundary": "mechanical output only; no source-text, semantic, graph, canon or publication authority",
                "owner": {"authority_boundary": ""},
            },
        }
        result = EVALUATOR.verify_authority_boundaries(payloads)
        self.assertFalse(result["ok"])
        self.assertTrue(result["checks"]["ok"])
        self.assertFalse(result["checks"]["bad:owner"])

    def test_old_method_freeze_does_not_bind_current_method_files(self) -> None:
        freeze = json.loads(
            (LAB_ROOT / "freeze-receipt-v6.json").read_text(encoding="utf-8")
        )
        self.assertFalse(EVALUATOR.verify_active_method_bindings(freeze)["ok"])

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

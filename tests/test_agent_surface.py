from __future__ import annotations

import sys
import tempfile
import unittest
import json
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import build_agent_surface_currentness as builder  # noqa: E402
import validate_agent_surface as validator  # noqa: E402


class AgentSurfaceTests(unittest.TestCase):
    def test_current_surface_has_expected_inventory_and_probe_depths(self) -> None:
        current = builder.build_currentness(ROOT)
        self.assertEqual(
            current["package_inventory"],
            {
                "agents_metadata": 25,
                "assets": 56,
                "checks": 16,
                "examples": 25,
                "references": 58,
                "scripts": 6,
                "skill_entrypoints": 25,
            },
        )
        self.assertEqual(len(current["packages"]), 25)
        self.assertEqual(max(current["task_probe_depths"].values()), 5)
        self.assertEqual(validator.validate_manifest(ROOT), [])

    def test_activation_metadata_positive_and_negative_controls(self) -> None:
        self.assertEqual(
            validator.activation_policy_issues(
                "explicit-preferred", "invoke", True, "aoa-change-protocol"
            ),
            [],
        )
        self.assertEqual(
            validator.activation_policy_issues(
                "explicit-preferred", "suggest", False, "aoa-checkpoint-closeout-bridge"
            ),
            [],
        )
        self.assertTrue(
            validator.activation_policy_issues(
                "explicit-only", "invoke", True, "aoa-approval-gate-check"
            )
        )

    def test_external_owner_reference_is_not_a_local_companion_route(self) -> None:
        package = ROOT / ".agents/skills/aoa-summon"
        self.assertEqual(validator._check_local_reference_routes(ROOT, package), [])

    def test_ignored_package_artifacts_are_not_currentness_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / ".agents/skills/example"
            (package / "__pycache__").mkdir(parents=True)
            tracked = package / "SKILL.md"
            ignored = package / "__pycache__/SKILL.cpython-314.pyc"
            tracked.write_text("tracked", encoding="utf-8")
            ignored.write_bytes(b"machine-local")
            files = builder.package_files(root, package)
            self.assertEqual(files, [tracked])

    def test_activation_metadata_rejects_non_boolean_scalars(self) -> None:
        with self.assertRaisesRegex(ValueError, "boolean literal"):
            builder.boolean_scalar(
                "definitely-not-a-boolean",
                key="allow_implicit_invocation",
                path=Path("agents/openai.yaml"),
            )

    def test_activation_metadata_must_be_in_policy_mapping(self) -> None:
        with self.assertRaisesRegex(ValueError, "immediate child of policy"):
            builder.policy_scalars(
                "implicit_activation_policy: manual\n"
                "policy:\n"
                "  implicit_activation_policy: invoke\n"
                "  allow_implicit_invocation: true\n",
                path=Path("agents/openai.yaml"),
            )

    def test_policy_scope_closes_at_next_top_level_mapping(self) -> None:
        with self.assertRaisesRegex(ValueError, "immediate child of policy"):
            builder.policy_scalars(
                "policy:\n"
                "  implicit_activation_policy: invoke\n"
                "  allow_implicit_invocation: true\n"
                "display:\n"
                "  implicit_activation_policy: manual\n"
                "  allow_implicit_invocation: false\n",
                path=Path("agents/openai.yaml"),
            )

    def test_frontmatter_fields_follow_metadata_mapping(self) -> None:
        with self.assertRaisesRegex(ValueError, "immediate child of metadata"):
            builder.frontmatter_scalars(
                "metadata:\n"
                "  aoa_invocation_mode: explicit-only\n"
                "name: example\n"
                "aoa_invocation_mode: explicit-preferred\n",
                path=Path("SKILL.md"),
            )

    def test_generated_kag_family_has_a_matching_receipt(self) -> None:
        manifest = json.loads(
            (ROOT / ".agents/agent-surface.manifest.json").read_text(encoding="utf-8")
        )
        self.assertEqual(
            validator.generated_family_issues(ROOT, manifest["owner_ports"]["kag_provider"]),
            [],
        )

    def test_duplicate_task_probe_ids_are_rejected(self) -> None:
        manifest = json.loads(
            (ROOT / ".agents/agent-surface.manifest.json").read_text(encoding="utf-8")
        )
        manifest["task_probes"].append(dict(manifest["task_probes"][0]))
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = Path(temporary) / "manifest.json"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with mock.patch.object(validator, "MANIFEST_PATH", manifest_path):
                issues = validator.validate_manifest(ROOT)
        self.assertIn(
            (str(manifest_path), "duplicate task probe source_authority"),
            issues,
        )

    def test_owner_cards_are_currentness_inputs(self) -> None:
        current = builder.build_currentness(ROOT)
        manifest = json.loads(
            (ROOT / ".agents/agent-surface.manifest.json").read_text(encoding="utf-8")
        )
        for port in current["owner_ports"]:
            owner = manifest["owner_ports"][port["id"]]["local_owner"]
            self.assertIn(owner, {item["path"] for item in port["inputs"]})

    def test_stale_currentness_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".agents").mkdir()
            (root / ".agents/agent-surface.current.json").write_bytes(b"stale")
            with mock.patch.object(validator, "rendered_currentness", return_value=b"fresh"):
                self.assertEqual(
                    validator.currentness_parity_issues(root),
                    [
                        (
                            ".agents/agent-surface.current.json",
                            "generated currentness is stale",
                        )
                    ],
                )

    def test_public_safety_rejects_host_local_and_secret_markers(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".agents").mkdir()
            (root / ".agents/README.md").write_text("operator path /srv/private", encoding="utf-8")
            (root / ".agents/agent-surface.manifest.json").write_text(
                '{"token":"OPENAI_API_KEY"}', encoding="utf-8"
            )
            issues = validator.public_safety_issues(root)
            self.assertEqual(len(issues), 2)


if __name__ == "__main__":
    unittest.main()

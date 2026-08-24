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
    @staticmethod
    def _budget_case(
        *,
        changed_generated_bytes: int,
        default_limit_bytes: int,
        tracked_bytes: int,
        tracked_bytes_max: int,
        scope: str,
    ) -> tuple[dict, dict, str]:
        digest = "a" * 64
        base_ref = "b" * 40
        manifest = {
            "schema_version": "aoa-repo-local-kag-family-manifest-v3",
            "repo": {"name": "Tree-of-Sophia", "git_ref": "git-index-source-tree"},
            "budgets": {
                "changed_generated_bytes_max": default_limit_bytes,
                "tracked_bytes_max": tracked_bytes_max,
            },
            "summary": {"tracked_bytes": tracked_bytes},
        }
        receipt = {
            "schema_version": validator.KAG_BUDGET_RECEIPT_SCHEMA_VERSION,
            "repo": "Tree-of-Sophia",
            "scope": scope,
            "base_ref": base_ref,
            "head_family_digest": digest,
            "changed_generated_bytes": changed_generated_bytes,
            "changed_generated_files": 1,
            "default_limit_bytes": default_limit_bytes,
            "allowed_bytes": changed_generated_bytes,
            "tracked_bytes": tracked_bytes,
            "tracked_bytes_max": tracked_bytes_max,
            "allowed_tracked_bytes": tracked_bytes,
            "reason": "focused budget scope control",
            "approved_by": "test-owner",
            "decision_ref": validator.KAG_BUDGET_DECISION_REF,
        }
        return manifest, receipt, digest

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

    def test_multiline_descriptions_are_rejected_before_budget_counting(self) -> None:
        with self.assertRaisesRegex(ValueError, "multiline description"):
            builder.frontmatter_scalars(
                "name: example\n"
                "description: >\n"
                "  This description is folded by YAML.\n",
                path=Path("SKILL.md"),
            )

    def test_single_quoted_escape_pairs_do_not_close_a_continuation(self) -> None:
        self.assertFalse(builder.quoted_scalar_is_open("'Nietzsche''s description'"))
        self.assertTrue(builder.quoted_scalar_is_open("'Nietzsche''s description"))
        with self.assertRaisesRegex(ValueError, "multiline description"):
            builder.frontmatter_scalars(
                "description: 'The line ends with an escaped apostrophe ''\n"
                "  and continues physically.\n",
                path=Path("SKILL.md"),
            )

        with self.assertRaisesRegex(ValueError, "multiline description"):
            builder.frontmatter_scalars(
                "name: example\n"
                "description: \"first line\n"
                "  continuation line\n",
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

    def test_budget_exceedance_relation_maps_single_joint_and_equality_cases(self) -> None:
        cases = (
            ((10, 10, 20, 20), None),
            ((11, 10, 20, 20), "generated_delta"),
            ((10, 10, 21, 20), "tracked_size"),
            ((11, 10, 21, 20), "generated_delta_and_tracked_size"),
        )
        for (changed, default, tracked, tracked_max), expected in cases:
            with self.subTest(changed=changed, tracked=tracked):
                self.assertEqual(
                    validator.budget_exceedance_relation(
                        changed_generated_bytes=changed,
                        default_limit_bytes=default,
                        tracked_bytes=tracked,
                        tracked_bytes_max=tracked_max,
                    ),
                    expected,
                )

    def test_budget_scope_mismatches_are_rejected_in_both_directions(self) -> None:
        cases = (
            ((11, 10, 20, 20), "tracked_size", "generated_delta"),
            ((10, 10, 21, 20), "generated_delta", "tracked_size"),
            ((11, 10, 21, 20), "generated_delta", "generated_delta_and_tracked_size"),
            ((11, 10, 21, 20), "tracked_size", "generated_delta_and_tracked_size"),
        )
        for (changed, default, tracked, tracked_max), scope, expected in cases:
            with self.subTest(scope=scope, changed=changed, tracked=tracked):
                family_manifest, receipt, digest = self._budget_case(
                    changed_generated_bytes=changed,
                    default_limit_bytes=default,
                    tracked_bytes=tracked,
                    tracked_bytes_max=tracked_max,
                    scope=scope,
                )
                issues = validator.budget_receipt_contract_issues(
                    ROOT,
                    family_manifest,
                    receipt,
                    digest,
                    ROOT / "focused-budget-receipt.json",
                    base_has_v3=True,
                )
                self.assertIn(
                    (
                        "focused-budget-receipt.json",
                        f"budget receipt scope does not match the current exceedance: expected {expected!r}",
                    ),
                    issues,
                )

    def test_budget_receipt_scope_rejects_no_exceedance(self) -> None:
        family_manifest, receipt, digest = self._budget_case(
            changed_generated_bytes=10,
            default_limit_bytes=10,
            tracked_bytes=20,
            tracked_bytes_max=20,
            scope="generated_delta",
        )
        self.assertIn(
            (
                "focused-budget-receipt.json",
                "budget receipt scope cannot authorize a family with no exceeded budget dimension",
            ),
            validator.budget_receipt_contract_issues(
                ROOT,
                family_manifest,
                receipt,
                digest,
                ROOT / "focused-budget-receipt.json",
                base_has_v3=True,
            ),
        )

    def test_malformed_budget_relation_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "must be integers"):
            validator.budget_exceedance_relation(
                changed_generated_bytes="not-an-integer",
                default_limit_bytes=10,
                tracked_bytes=20,
                tracked_bytes_max=20,
            )

    def test_v2_to_v3_migration_scope_is_a_narrow_base_exception(self) -> None:
        relation = validator.budget_exceedance_relation(
            changed_generated_bytes=11,
            default_limit_bytes=10,
            tracked_bytes=20,
            tracked_bytes_max=20,
        )
        self.assertEqual(
            validator.canonical_budget_scope(relation, base_has_v3=False),
            "v2_to_v3_migration",
        )
        self.assertEqual(
            validator.canonical_budget_scope(relation, base_has_v3=True),
            "generated_delta",
        )
        family_manifest, receipt, digest = self._budget_case(
            changed_generated_bytes=11,
            default_limit_bytes=10,
            tracked_bytes=20,
            tracked_bytes_max=20,
            scope="v2_to_v3_migration",
        )
        self.assertEqual(
            validator.budget_receipt_contract_issues(
                ROOT,
                family_manifest,
                receipt,
                digest,
                ROOT / "focused-budget-receipt.json",
                base_has_v3=False,
            ),
            [],
        )
        self.assertIn(
            (
                "focused-budget-receipt.json",
                "v2_to_v3_migration requires a base without a v3 family manifest",
            ),
            validator.budget_receipt_contract_issues(
                ROOT,
                family_manifest,
                receipt,
                digest,
                ROOT / "focused-budget-receipt.json",
                base_has_v3=True,
            ),
        )

    def test_current_receipt_rejects_review_scope_case(self) -> None:
        manifest = json.loads(
            (ROOT / ".agents/agent-surface.manifest.json").read_text(encoding="utf-8")
        )
        port = manifest["owner_ports"]["kag_provider"]
        family = port["generated_family"]
        family_manifest = json.loads((ROOT / family["manifest"]).read_text(encoding="utf-8"))
        digest = family_manifest["family_identity"]["content_digest"]
        receipt_path = ROOT / family["receipt_root"] / f"{digest}.json"
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        relation = validator.budget_exceedance_relation(
            changed_generated_bytes=receipt["changed_generated_bytes"],
            default_limit_bytes=family_manifest["budgets"]["changed_generated_bytes_max"],
            tracked_bytes=family_manifest["summary"]["tracked_bytes"],
            tracked_bytes_max=family_manifest["budgets"]["tracked_bytes_max"],
        )
        wrong_scope = "generated_delta" if relation != "generated_delta" else "tracked_size"
        receipt["scope"] = wrong_scope
        issues = validator.budget_receipt_contract_issues(
            ROOT,
            family_manifest,
            receipt,
            digest,
            receipt_path,
            base_has_v3=True,
        )
        self.assertIn(
            (
                receipt_path.relative_to(ROOT).as_posix(),
                (
                    f"budget receipt scope does not match the current exceedance: expected {relation!r}"
                    if relation is not None
                    else "budget receipt scope cannot authorize a family with no exceeded budget dimension"
                ),
            ),
            issues,
        )

    def test_generated_kag_family_rejects_a_digest_only_receipt(self) -> None:
        manifest = json.loads(
            (ROOT / ".agents/agent-surface.manifest.json").read_text(encoding="utf-8")
        )
        port = manifest["owner_ports"]["kag_provider"]
        family = port["generated_family"]
        family_manifest = json.loads(
            (ROOT / family["manifest"]).read_text(encoding="utf-8")
        )
        digest = family_manifest["family_identity"]["content_digest"]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = root / family["manifest"]
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(json.dumps(family_manifest), encoding="utf-8")
            (root / family["shards"]).mkdir(parents=True)
            receipt_path = root / family["receipt_root"] / f"{digest}.json"
            receipt_path.parent.mkdir(parents=True)
            receipt_path.write_text(
                json.dumps({"head_family_digest": digest}),
                encoding="utf-8",
            )

            issues = validator.generated_family_issues(root, port)

        messages = "\n".join(message for _, message in issues)
        for field in validator.KAG_BUDGET_RECEIPT_REQUIRED_FIELDS - {"head_family_digest"}:
            self.assertIn(f"missing required field {field}", messages)

    def test_generated_kag_family_rejects_untyped_budget_evidence(self) -> None:
        manifest = json.loads(
            (ROOT / ".agents/agent-surface.manifest.json").read_text(encoding="utf-8")
        )
        port = manifest["owner_ports"]["kag_provider"]
        family = port["generated_family"]
        family_manifest = json.loads(
            (ROOT / family["manifest"]).read_text(encoding="utf-8")
        )
        digest = family_manifest["family_identity"]["content_digest"]
        receipt = json.loads(
            (ROOT / family["receipt_root"] / f"{digest}.json").read_text(encoding="utf-8")
        )
        receipt["allowed_bytes"] = True
        receipt["tracked_bytes"] = "not-an-integer"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = root / family["manifest"]
            manifest_path.parent.mkdir(parents=True)
            manifest_path.write_text(json.dumps(family_manifest), encoding="utf-8")
            (root / family["shards"]).mkdir(parents=True)
            receipt_path = root / family["receipt_root"] / f"{digest}.json"
            receipt_path.parent.mkdir(parents=True)
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")

            issues = validator.generated_family_issues(root, port)

        self.assertIn(
            (str(receipt_path.relative_to(root)), "budget receipt field allowed_bytes must be an integer"),
            issues,
        )
        self.assertIn(
            (str(receipt_path.relative_to(root)), "budget receipt field tracked_bytes must be an integer"),
            issues,
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

    def test_port_manifests_are_currentness_inputs(self) -> None:
        manifest = json.loads(
            (ROOT / ".agents/agent-surface.manifest.json").read_text(encoding="utf-8")
        )
        manifest["owner_ports"]["eval_port"]["currentness_inputs"].remove("evals/PORT.yaml")
        with tempfile.TemporaryDirectory() as temporary:
            manifest_path = Path(temporary) / "manifest.json"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with mock.patch.object(validator, "MANIFEST_PATH", manifest_path):
                issues = validator.validate_manifest(ROOT)
        self.assertIn(
            (str(manifest_path), "eval_port manifest must be a currentness input"),
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

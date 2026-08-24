from __future__ import annotations

import json
import sys
import tempfile
import unittest
from copy import deepcopy
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import build_documentation_family_currentness as builder  # noqa: E402
import validate_documentation_cross_corpus as validator  # noqa: E402


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


class DocumentationCrossCorpusTests(unittest.TestCase):
    def test_current_map_and_projection_cover_the_atlas_surface(self) -> None:
        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        current = builder.build_currentness(ROOT)
        human_extensions = set(source_map["atlas_method"]["human_extensions"])
        human_exclusion = source_map["atlas_method"]["human_exclusion"]
        self.assertEqual(source_map["schema_ref"], "docs/validation/documentation-family-map.schema.json")
        human_records = [
            record
            for record in current["tracked_surfaces"]
            if builder.in_human_scope(Path(record["path"]), human_extensions, human_exclusion)
        ]
        self.assertEqual(current["coverage"]["human_scope"]["count"], len(human_records))
        self.assertEqual(
            current["coverage"]["human_scope"]["bytes"],
            sum(record["bytes"] for record in human_records),
        )
        self.assertEqual(current["coverage"]["unhandled_family_count"], 0)
        self.assertEqual(
            {family["id"] for family in source_map["families"]},
            {summary["family_id"] for summary in current["family_summaries"]},
        )
        self.assertEqual([], validator.run_validation(ROOT, reuse_existing=False))

    def test_authority_guard_rejects_duplicate_and_conflicting_claims(self) -> None:
        duplicate: list[tuple[str, str]] = []
        validator.validate_authority_declarations(
            [
                {"id": "claim", "owner": "README.md", "strength": "authored"},
                {"id": "claim", "owner": "README.md", "strength": "authored"},
            ],
            duplicate,
        )
        self.assertTrue(any("duplicate authority declaration" in message for _, message in duplicate))

        conflict: list[tuple[str, str]] = []
        validator.validate_authority_declarations(
            [
                {"id": "claim", "owner": "README.md", "strength": "authored"},
                {"id": "claim", "owner": "ToS/", "strength": "authored"},
            ],
            conflict,
        )
        self.assertTrue(any("conflicting authority declaration" in message for _, message in conflict))

        harmless: list[tuple[str, str]] = []
        validator.validate_authority_declarations(
            [{"id": "claim", "owner": "README.md", "strength": "authored"}],
            harmless,
        )
        self.assertEqual([], harmless)

    def test_family_authority_guard_binds_owner_and_keys_to_declarations(self) -> None:
        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        families = deepcopy(source_map["families"])
        declarations = deepcopy(source_map["authority_declarations"])
        families[0]["owner"] = "wrong-owner"
        issues: list[tuple[str, str]] = []
        validator.validate_family_authority_claims(families, declarations, issues)
        self.assertTrue(any("family owner conflicts" in message for _, message in issues))

        families = deepcopy(source_map["families"])
        families[0]["authority_keys"] = ["missing_claim"]
        issues = []
        validator.validate_family_authority_claims(families, declarations, issues)
        self.assertTrue(any("has no declaration" in message for _, message in issues))

        families = deepcopy(source_map["families"])
        families[0]["authority_keys"] = []
        issues = []
        validator.validate_family_authority_claims(families, declarations, issues)
        self.assertTrue(any("non-empty list" in message for _, message in issues))

        families = deepcopy(source_map["families"])
        families[0]["authority_keys"].remove("repository_identity")
        issues = []
        validator.validate_family_authority_claims(families, declarations, issues)
        self.assertTrue(any("not referenced by authority_keys" in message for _, message in issues))

    def test_source_map_requires_declared_validation_lane(self) -> None:
        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        source_map["families"][0]["validation_lane"] = "missing-lane"
        issues: list[tuple[str, str]] = []
        validator.validate_source_map(ROOT, source_map, issues)
        self.assertTrue(any("not in command authority" in message for _, message in issues))

        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        source_map["generated_currentness"] = "README.md"
        issues = []
        validator.validate_source_map(ROOT, source_map, issues)
        self.assertTrue(any("canonical route" in message for _, message in issues))

        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        source_map["surface_rules"]["generated_carrier_prefixes"] = ["ToS/"]
        issues = []
        validator.validate_surface_rules(source_map["surface_rules"], issues)
        self.assertTrue(any("canonical generated carriers" in message for _, message in issues))

        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        source_map["public_authored_surfaces"].remove("README.md")
        issues = []
        validator.validate_source_map(ROOT, source_map, issues)
        self.assertTrue(any("missing required entries" in message for _, message in issues))

        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        source_map["context_probes"][0]["max_tokens"] = 9999
        issues = []
        validator.validate_context_probe_configuration(source_map["context_probes"], issues)
        self.assertTrue(any("canonical context contract" in message for _, message in issues))

    def test_human_scope_uses_authored_exclusion_glob(self) -> None:
        human_extensions = {".yaml"}
        self.assertFalse(
            builder.in_human_scope(
                Path(".agents/skills/example/agents/openai.yaml"),
                human_extensions,
                ".agents/skills/**/agents/openai.yaml",
            )
        )
        self.assertTrue(
            builder.in_human_scope(
                Path(".agents/skills/example/agents/openai.yaml"),
                human_extensions,
                ".agents/skills/**/agents/other.yaml",
            )
        )

    def test_source_map_requires_public_safety_lists(self) -> None:
        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        source_map["public_authored_surfaces"] = []
        source_map["public_forbidden_markers"] = []
        issues: list[tuple[str, str]] = []
        validator.validate_source_map(ROOT, source_map, issues)
        self.assertTrue(any("public_authored_surfaces" in message for _, message in issues))
        self.assertTrue(any("public_forbidden_markers" in message for _, message in issues))

        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        source_map["public_forbidden_markers"].remove("BEGIN PRIVATE KEY")
        issues = []
        validator.validate_source_map(ROOT, source_map, issues)
        self.assertTrue(any("missing required entries" in message for _, message in issues))

    def test_family_match_guard_binds_ids_to_canonical_matchers(self) -> None:
        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        families = deepcopy(source_map["families"])
        families[0]["match"], families[1]["match"] = families[1]["match"], families[0]["match"]
        issues: list[tuple[str, str]] = []
        validator.validate_family_match_rules(families, issues)
        self.assertTrue(any("canonical matcher" in message for _, message in issues))

    def test_markdown_file_and_fragment_routes_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "README.md", "[missing](missing.md)\n[fragment](target.md#absent)\n")
            write_text(root / "target.md", "# Present\n")
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md"), Path("target.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_markdown_routes(root, issues)
        self.assertTrue(any("broken local documentation route" in message for _, message in issues))
        self.assertTrue(any("broken local documentation fragment" in message for _, message in issues))

    def test_unresolved_reference_style_markdown_route_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "README.md", "[guide][missing]\n")
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_markdown_routes(root, issues)
        self.assertTrue(any("unresolved reference-style documentation route" in message for _, message in issues))

    def test_active_stale_executable_route_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "README.md", "Run `python scripts/not-present.py`.\n")
            write_text(root / "docs/validation/script_inventory.json", '{"script_surfaces": []}\n')
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_executable_routes(root, issues)
        self.assertTrue(any("stale executable reference" in message for _, message in issues))

    def test_active_route_card_shell_reference_is_not_exempt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "evals/AGENTS.md", "Run `scripts/not-present.sh`.\n")
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("evals/AGENTS.md")]):
                with mock.patch.object(validator.validate_nested_agents, "discover_route_cards", return_value=[]):
                    issues: list[tuple[str, str]] = []
                    validator.validate_executable_routes(root, issues)
        self.assertTrue(any("stale executable reference: scripts/not-present.sh" in message for _, message in issues))

    def test_external_or_historical_executable_carrier_is_not_local_stale_route(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "README.md", "Run `scripts/external.py` through the aoa-kag external owner.\n")
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_executable_routes(root, issues)
        self.assertEqual([], issues)

    def test_external_marker_does_not_exempt_an_unrelated_local_reference(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(
                root / "README.md",
                "Run `scripts/not-present.py` locally.\n"
                "The separate aoa-kag command is an external owner.\n",
            )
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_executable_routes(root, issues)
        self.assertTrue(any("stale executable reference: scripts/not-present.py" in message for _, message in issues))

    def test_external_marker_is_command_local_when_commands_share_a_line(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(
                root / "README.md",
                "`scripts/not-present.py` locally; aoa-kag external owner runs `scripts/external.py`.\n",
            )
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_executable_routes(root, issues)
        self.assertTrue(any("stale executable reference: scripts/not-present.py" in message for _, message in issues))

    def test_context_probe_rejects_unknown_measure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "entry.md", "one two\n")
            payload = {
                "context_probes": [
                    {"id": "entry", "surfaces": ["entry.md"], "measure": "typo", "max_tokens": 2}
                ]
            }
            issues: list[tuple[str, str]] = []
            validator.validate_context_probes(root, payload, issues)
        self.assertTrue(any("unsupported context measure" in message for _, message in issues))

    def test_external_marker_does_not_cross_a_comma_clause(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(
                root / "README.md",
                "Run scripts/not-present.py locally, while the aoa-kag external owner handles deployment.\\n",
            )
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_executable_routes(root, issues)
        self.assertTrue(any("stale executable reference: scripts/not-present.py" in message for _, message in issues))

    def test_external_marker_does_not_cross_a_markdown_table_cell(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(
                root / "README.md",
                "| local | scripts/not-present.py | owner | aoa-kag external owner |\n",
            )
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_executable_routes(root, issues)
        self.assertTrue(any("stale executable reference: scripts/not-present.py" in message for _, message in issues))

    def test_generated_projection_mismatch_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "docs/validation/documentation-family.current.json", "stale\n")
            with (
                mock.patch.object(validator, "build_currentness", return_value={"ok": True}),
                mock.patch.object(validator, "render_currentness", return_value="expected\n"),
            ):
                issues: list[tuple[str, str]] = []
                validator.validate_generated_projection(root, issues)
        self.assertTrue(any("projection is stale" in message for _, message in issues))

    def test_generated_summary_probe_uses_its_configured_surface(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "configured.json", json.dumps({"context_summary": {"one": "two"}}))
            payload = {
                "context_probes": [
                    {"id": "summary", "surfaces": ["missing.json"], "measure": "generated_summary", "max_tokens": 20}
                ]
            }
            issues: list[tuple[str, str]] = []
            validator.validate_context_probes(root, payload, issues)
        self.assertTrue(any("cannot measure probe" in message for _, message in issues))

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "wrong.json", json.dumps({"not_a_summary": {}}))
            payload = {
                "context_probes": [
                    {"id": "summary", "surfaces": ["wrong.json"], "measure": "generated_summary", "max_tokens": 20}
                ]
            }
            issues = []
            validator.validate_context_probes(root, payload, issues)
        self.assertTrue(any("lacks context_summary" in message for _, message in issues))

    def test_missing_family_coverage_is_detected(self) -> None:
        payload = {"families": [{"id": "root"}]}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with mock.patch.object(
                validator,
                "build_currentness",
                return_value={
                    "coverage": {"unhandled_family_count": 1, "unhandled_paths": ["new/file.md"]},
                    "family_summaries": [{"family_id": "root"}],
                },
            ):
                issues: list[tuple[str, str]] = []
                validator.validate_family_coverage(root, payload, issues)
        self.assertTrue(any("unhandled family count" in message for _, message in issues))

    def test_public_safety_allows_terminology_but_rejects_host_and_receipt_markers(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "public.md", "The provider route is public-safe.\n")
            safe_payload = {"public_authored_surfaces": ["public.md"], "public_forbidden_markers": ["/srv/", "holder_pid"]}
            safe_issues: list[tuple[str, str]] = []
            validator.validate_public_safety(root, safe_payload, safe_issues)
            write_text(root / "public.md", "provider route /srv/private holder_pid\n")
            unsafe_issues: list[tuple[str, str]] = []
            validator.validate_public_safety(root, safe_payload, unsafe_issues)
        self.assertEqual([], safe_issues)
        self.assertEqual(2, len(unsafe_issues))

    def test_context_configuration_requires_non_empty_probe_set_and_surfaces(self) -> None:
        issues: list[tuple[str, str]] = []
        validator.validate_context_probe_configuration([], issues)
        self.assertTrue(any("non-empty list" in message for _, message in issues))

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = {
                "context_probes": [
                    {"id": "entry", "surfaces": [], "measure": "sum", "max_tokens": 2}
                ]
            }
            validator.validate_context_probe_configuration(payload["context_probes"], issues)
            validator.validate_context_probes(root, payload, issues)
        self.assertTrue(any("surfaces must be a non-empty list" in message for _, message in issues))

    def test_surface_rules_reject_empty_or_unsafe_atlas_configuration(self) -> None:
        issues: list[tuple[str, str]] = []
        validator.validate_surface_rules({"include_extensions": [], "exclude_paths": [], "exclude_prefixes": []}, issues)
        self.assertTrue(any("include_extensions" in message for _, message in issues))

        issues = []
        validator.validate_surface_rules(
            {
                "include_extensions": sorted(validator.EXPECTED_SURFACE_EXTENSIONS),
                "exclude_paths": ["../outside"],
                "exclude_prefixes": ["kag/indexes"],
            },
            issues,
        )
        self.assertTrue(any("unsafe path" in message for _, message in issues))
        self.assertTrue(any("must end with '/'" in message for _, message in issues))

        issues = []
        validator.validate_surface_rules(
            {
                "include_extensions": sorted(validator.EXPECTED_SURFACE_EXTENSIONS),
                "exclude_paths": [],
                "exclude_prefixes": ["ToS/"],
                "generated_carrier_paths": ["docs/validation/documentation-family.current.json"],
                "generated_carrier_prefixes": ["kag/indexes/", "kag/receipts/index_family_budget/"],
            },
            issues,
        )
        self.assertTrue(any("generated_carrier_prefixes" in message for _, message in issues))

    def test_family_match_rules_reject_unresolved_overlap(self) -> None:
        source_map = json.loads(
            (ROOT / "docs/validation/documentation_family_map.json").read_text(encoding="utf-8")
        )
        families = deepcopy(source_map["families"])
        families[1]["match"]["prefix"] = ".agents/skills-extra/"
        issues: list[tuple[str, str]] = []
        validator.validate_family_match_rules(families, issues)
        self.assertTrue(any("overlapping family prefixes" in message for _, message in issues))

    def test_context_probe_overflow_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "entry.md", "one two three\n")
            payload = {
                "context_probes": [
                    {"id": "entry", "surfaces": ["entry.md"], "measure": "sum", "max_tokens": 2}
                ]
            }
            issues: list[tuple[str, str]] = []
            validator.validate_context_probes(root, payload, issues)
        self.assertTrue(any("context budget exceeded" in message for _, message in issues))

    def test_inherited_route_probe_measures_all_configured_surfaces(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "first.json", json.dumps({"task_routes": [{"inherited_context_tokens": 2}]}))
            write_text(root / "second.json", json.dumps({"task_routes": [{"inherited_context_tokens": 5}]}))
            payload = {
                "context_probes": [
                    {
                        "id": "routes",
                        "surfaces": ["first.json", "second.json"],
                        "measure": "agents_route_max_inherited",
                        "max_tokens": 4,
                    }
                ]
            }
            issues: list[tuple[str, str]] = []
            validator.validate_context_probes(root, payload, issues)
        self.assertTrue(any("context budget exceeded: 5>4" in message for _, message in issues))

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "empty.json", json.dumps({"task_routes": []}))
            payload = {
                "context_probes": [
                    {
                        "id": "routes",
                        "surfaces": ["empty.json"],
                        "measure": "agents_route_max_inherited",
                        "max_tokens": 4,
                    }
                ]
            }
            issues = []
            validator.validate_context_probes(root, payload, issues)
        self.assertTrue(any("produced no inherited measurements" in message for _, message in issues))


if __name__ == "__main__":
    unittest.main()

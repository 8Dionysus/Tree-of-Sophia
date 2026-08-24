from __future__ import annotations

import json
import sys
import tempfile
import unittest
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
        self.assertEqual(source_map["schema_ref"], "docs/validation/documentation-family-map.schema.json")
        human_records = [
            record
            for record in current["tracked_surfaces"]
            if builder.in_human_scope(Path(record["path"]))
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

    def test_active_stale_executable_route_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            write_text(root / "README.md", "Run `python scripts/not-present.py`.\n")
            write_text(root / "docs/validation/script_inventory.json", '{"script_surfaces": []}\n')
            with mock.patch.object(validator, "tracked_paths", return_value=[Path("README.md")]):
                issues: list[tuple[str, str]] = []
                validator.validate_executable_routes(root, issues)
        self.assertTrue(any("stale executable reference" in message for _, message in issues))

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


if __name__ == "__main__":
    unittest.main()

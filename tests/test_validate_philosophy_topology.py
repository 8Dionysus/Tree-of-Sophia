from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import validate_philosophy_topology  # noqa: E402


def write_text(path: Path, content: str = "") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def write_json(path: Path, payload: dict[str, object]) -> None:
    write_text(path, json.dumps(payload, ensure_ascii=True, indent=2) + "\n")


class ValidatePhilosophyTopologyTests(unittest.TestCase):
    def write_valid_surface(self, repo_root: Path) -> None:
        for relative_path in (
            Path("ToS/philosophy/AGENTS.md"),
            Path("ToS/philosophy/README.md"),
            Path("ToS/research-packets/AGENTS.md"),
            Path("ToS/research-packets/deep-research/philosophy/AGENTS.md"),
        ):
            write_text(repo_root / relative_path)
        write_json(
            repo_root / "ToS/research-packets/deep-research/philosophy/research.manifest.json",
            {
                "schema_version": "tos_research_packet_v1",
                "path": "ToS/research-packets/deep-research/philosophy",
                "domain_branch": "ToS/philosophy",
                "authority": {
                    "source_status": "not_source_witness",
                    "canon_status": "not_canon",
                },
                "capture_container": {
                    "page_id": "fixture-page",
                    "title": "Fixture Packet",
                },
                "branch_child_pages": [
                    {
                        "id": "fixture-child-page",
                        "title": "Fixture Child",
                    }
                ],
            },
        )
        write_json(
            repo_root / "ToS/philosophy/trunk/branch.manifest.json",
            {
                "path": "ToS/philosophy/trunk",
                "branch_id": "philosophy.trunk",
                "role": "fixture trunk",
            },
        )
        write_json(
            repo_root / "ToS/philosophy/philosophy.manifest.json",
            {
                "schema_version": "tos_philosophy_topology_v1",
                "branch_id": "philosophy",
                "path": "ToS/philosophy",
                "boundary_routes": {
                    "provisional_extraction": "ToS/candidate-intake",
                    "research_packets": "ToS/research-packets",
                    "source_witnesses": "ToS/source-witnesses",
                    "canon_promotion": "ToS/canon",
                },
                "path_component_policy": {
                    "repository_paths_describe": ["philosophy branch"],
                    "metadata_only_inputs": ["capture titles"],
                },
                "research_packet_routes": [
                    "ToS/research-packets/deep-research/philosophy/research.manifest.json"
                ],
                "branch_manifests": [
                    "ToS/philosophy/trunk/branch.manifest.json"
                ],
            },
        )

    def test_valid_research_packet_route_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertEqual(issues, [])

    def test_research_packet_route_traversal_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            manifest_path = repo_root / "ToS/philosophy/philosophy.manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            traversal_route = "ToS/research-packets/../canon/rogue/research.manifest.json"
            manifest["research_packet_routes"] = [traversal_route]
            write_json(manifest_path, manifest)
            write_text(repo_root / "ToS/canon/rogue/AGENTS.md")
            write_json(
                repo_root / "ToS/canon/rogue/research.manifest.json",
                {
                    "schema_version": "tos_research_packet_v1",
                    "path": "ToS/research-packets/../canon/rogue",
                    "domain_branch": "ToS/philosophy",
                    "authority": {
                        "source_status": "not_source_witness",
                        "canon_status": "not_canon",
                    },
                    "capture_container": {
                        "page_id": "fixture-page",
                        "title": "Fixture Packet",
                    },
                    "branch_child_pages": ["fixture-child"],
                },
            )

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertIn(
            (
                traversal_route,
                "research packet routes must be normalized repo-relative paths under ToS/research-packets",
            ),
            issues,
        )

    def test_child_page_title_path_component_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            manifest_path = repo_root / "ToS/philosophy/philosophy.manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            branch_path = "ToS/philosophy/eras/fixture-child/branch.manifest.json"
            manifest["branch_manifests"].append(branch_path)
            write_json(manifest_path, manifest)
            write_json(
                repo_root / branch_path,
                {
                    "path": "ToS/philosophy/eras/fixture-child",
                    "branch_id": "philosophy.fixture_child",
                    "role": "fixture branch named from a child page title",
                },
            )

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertIn(
            (
                branch_path,
                "metadata-only source label used as path component: fixture-child",
            ),
            issues,
        )

    def test_path_component_sweep_uses_supplied_repo_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(repo_root / "ToS/rogue/fixture-child/README.md")

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertIn(
            (
                "ToS/rogue/fixture-child",
                "metadata-only source label used as path component: fixture-child",
            ),
            issues,
        )


if __name__ == "__main__":
    unittest.main()

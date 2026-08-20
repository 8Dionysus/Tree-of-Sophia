from __future__ import annotations

import json
import copy
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
                "mature_branch_shape": [
                    "eras/<era>",
                    "eras/<era>/regions/<region>",
                    "eras/<era>/regions/<region>/traditions/<tradition>",
                ],
                "promotion_pipeline": [
                    "research packet",
                    "branch review",
                    "local branch skeleton",
                    "proposed nodes",
                    "proposed relations",
                    "relation pack",
                    "canon promotion",
                    "derived graph/export",
                ],
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

    def test_declared_empty_branch_manifests_are_checked(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_json(
                repo_root / "ToS/philosophy/trunk/branch.manifest.json",
                {
                    "path": "ToS/philosophy/trunk",
                    "branch_id": "philosophy.trunk",
                    "role": "fixture trunk",
                    "source_planting_refs": [
                        "ToS/philosophy/trunk/sources/plantings/stale/source-planting.json"
                    ],
                    "source_planting_count": 1,
                },
            )
            write_json(
                repo_root / "ToS/philosophy/trunk/sources/branch.manifest.json",
                {
                    "planting_refs": [
                        "ToS/philosophy/trunk/sources/plantings/stale/source-planting.json"
                    ],
                    "planting_count": 1,
                },
            )

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertIn(
            (
                "ToS/philosophy/trunk/branch",
                "source_planting_refs differs from exact planting files",
            ),
            issues,
        )
        self.assertIn(
            (
                "ToS/philosophy/trunk/branch",
                "source_planting_count differs from exact planting count",
            ),
            issues,
        )
        self.assertIn(
            (
                "ToS/philosophy/trunk/sources",
                "planting_refs differs from exact planting files",
            ),
            issues,
        )
        self.assertIn(
            (
                "ToS/philosophy/trunk/sources",
                "planting_count differs from exact planting count",
            ),
            issues,
        )

    def test_current_source_planting_contract_fails_closed_on_promotion(self) -> None:
        schema_path = Path("ToS/contracts/philosophy-source-planting.schema.json")
        validator = validate_philosophy_topology.load_schema_validator(
            REPO_ROOT,
            schema_path,
        )
        planting_path = (
            REPO_ROOT
            / "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/"
            "proto-cuneiform-accounting-ontologies/sources/plantings/"
            "cdli-p000015/source-planting.json"
        )
        planting = json.loads(planting_path.read_text(encoding="utf-8"))
        self.assertEqual([], list(validator.iter_errors(planting)))

        composite_planting_path = (
            REPO_ROOT
            / "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/"
            "proto-cuneiform-accounting-ontologies/sources/plantings/"
            "dcclt-q000023/source-planting.json"
        )
        composite_planting = json.loads(
            composite_planting_path.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(composite_planting)))

        bibliographic_planting_path = (
            REPO_ROOT
            / "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/"
            "proto-cuneiform-accounting-ontologies/sources/plantings/"
            "atu2-green-sign-list/source-planting.json"
        )
        bibliographic_planting = json.loads(
            bibliographic_planting_path.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(bibliographic_planting)))

        direct_work_planting_path = (
            REPO_ROOT
            / "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/"
            "proto-cuneiform-accounting-ontologies/sources/plantings/"
            "atu3-lexical-lists/source-planting.json"
        )
        direct_work_planting = json.loads(
            direct_work_planting_path.read_text(encoding="utf-8")
        )
        self.assertEqual([], list(validator.iter_errors(direct_work_planting)))

        cdlb_planting_root = (
            REPO_ROOT
            / "ToS/philosophy/eras/bronze-age/regions/west-asia/traditions/"
            "proto-cuneiform-accounting-ontologies/sources/plantings"
        )
        cdlb_source_planting = json.loads(
            (
                cdlb_planting_root
                / "cdlb-2021-6-quantitative-sign-use/source-planting.json"
            ).read_text(encoding="utf-8")
        )
        cdlb_control_planting = json.loads(
            (
                cdlb_planting_root
                / "born-kelley-2021-tribute-control/source-planting.json"
            ).read_text(encoding="utf-8")
        )
        self.assertEqual(
            [], list(validator.iter_errors(cdlb_source_planting))
        )
        self.assertEqual(
            [], list(validator.iter_errors(cdlb_control_planting))
        )
        self.assertEqual(
            cdlb_source_planting["source_witness"],
            cdlb_control_planting["source_witness"],
        )
        self.assertEqual(
            {16, 22},
            {
                cdlb_source_planting["source_backlog_anchor"]["line"],
                cdlb_control_planting["source_backlog_anchor"]["line"],
            },
        )
        self.assertFalse(
            cdlb_source_planting["authority"]["source_text_admitted"]
        )
        self.assertEqual(
            "not_started",
            cdlb_control_planting["authority"]["semantic_status"],
        )

        false_graph_promotion = copy.deepcopy(planting)
        false_graph_promotion["authority"]["graph_status"] = "promoted"
        self.assertTrue(list(validator.iter_errors(false_graph_promotion)))

        false_human_task = copy.deepcopy(planting)
        false_human_task["authority"]["human_task_created"] = True
        self.assertTrue(list(validator.iter_errors(false_human_task)))

        false_composite_promotion = copy.deepcopy(composite_planting)
        false_composite_promotion["authority"]["source_text_admitted"] = True
        self.assertTrue(list(validator.iter_errors(false_composite_promotion)))

        unbound_bibliographic_planting = copy.deepcopy(bibliographic_planting)
        del unbound_bibliographic_planting["source_witness"]["membership_claim_ref"]
        self.assertTrue(list(validator.iter_errors(unbound_bibliographic_planting)))

        partial_container_binding = copy.deepcopy(direct_work_planting)
        partial_container_binding["source_witness"]["container_id"] = (
            "tos.collection.proto-cuneiform.unsupported-wrapper"
        )
        self.assertTrue(list(validator.iter_errors(partial_container_binding)))

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

    def test_child_page_current_title_path_component_fails_when_original_title_exists(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            research_packet_path = (
                repo_root / "ToS/research-packets/deep-research/philosophy/research.manifest.json"
            )
            research_packet = json.loads(research_packet_path.read_text(encoding="utf-8"))
            research_packet["branch_child_pages"] = [
                {
                    "id": "fixture-child-page",
                    "original_title": "Original Child",
                    "title": "Renamed Child",
                }
            ]
            write_json(research_packet_path, research_packet)
            manifest_path = repo_root / "ToS/philosophy/philosophy.manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            branch_path = "ToS/philosophy/eras/renamed-child/branch.manifest.json"
            manifest["branch_manifests"].append(branch_path)
            write_json(manifest_path, manifest)
            write_json(
                repo_root / branch_path,
                {
                    "path": "ToS/philosophy/eras/renamed-child",
                    "branch_id": "philosophy.renamed_child",
                    "role": "fixture branch named from a current child page title",
                },
            )

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertIn(
            (
                branch_path,
                "metadata-only source label used as path component: renamed-child",
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

    def test_graph_view_route_must_stay_under_graph_views(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            manifest_path = repo_root / "ToS/philosophy/philosophy.manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["graph_view_routes"] = ["ToS/philosophy/graph-workbench/rogue.graph.md"]
            write_json(manifest_path, manifest)
            write_text(repo_root / "ToS/philosophy/graph-workbench/rogue.graph.md")

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertIn(
            (
                "ToS/philosophy/graph-workbench/rogue.graph.md",
                "graph_view_routes entries must stay under ToS/philosophy/graph-workbench/views",
            ),
            issues,
        )

    def test_graph_view_contract_route_must_stay_under_graph_views(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            manifest_path = repo_root / "ToS/philosophy/philosophy.manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["graph_view_contracts"] = ["ToS/philosophy/graph-workbench/rogue-contract.json"]
            write_json(manifest_path, manifest)
            write_json(repo_root / "ToS/philosophy/graph-workbench/rogue-contract.json", {"ok": True})

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertIn(
            (
                "ToS/philosophy/graph-workbench/rogue-contract.json",
                "graph_view_contracts entries must stay under ToS/philosophy/graph-workbench/views",
            ),
            issues,
        )

    def test_atlas_route_must_stay_under_philosophy_atlas(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            manifest_path = repo_root / "ToS/philosophy/philosophy.manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["atlas_routes"] = ["ToS/philosophy/rogue-atlas.json"]
            write_json(manifest_path, manifest)
            write_json(repo_root / "ToS/philosophy/rogue-atlas.json", {"ok": True})

            issues = validate_philosophy_topology.run_validation(repo_root)

        self.assertIn(
            (
                "ToS/philosophy/rogue-atlas.json",
                "atlas_routes entries must stay under ToS/philosophy/atlas",
            ),
            issues,
        )


if __name__ == "__main__":
    unittest.main()

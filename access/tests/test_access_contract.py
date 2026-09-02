from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import threading
import unittest
import urllib.request
from pathlib import Path

ACCESS_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ACCESS_ROOT.parent
sys.path.insert(0, (ACCESS_ROOT / "src").as_posix())

from tos_access.core import ToSAccessCore  # noqa: E402
from tos_access.doctor import doctor_report  # noqa: E402
from tos_access.http_server import make_server  # noqa: E402
from tos_access.mcp_server import build_server  # noqa: E402


def write_fixture(root: Path) -> None:
    derived = root / "ToS/derived-exports"
    audit = root / "ToS/philosophy/graph-workbench/review-packets"
    derived.mkdir(parents=True)
    audit.mkdir(parents=True)
    index = {
        "schema_version": "tos_corpus_index_v1",
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived",
        "counts": {"nodes": 1},
        "nodes": [{"node_id": "a", "label": "Alpha", "source_ref": "ToS/canon/a.json"}],
        "resources": [],
        "manifests": [],
        "branches": [],
        "relation_edges": [],
        "relation_packs": [],
        "graph_views": [{"view_id": "corpus-topology", "title": "Corpus"}],
        "authority_order": ["ToS/canon"],
        "runtime_projection_boundary": {"runtime_owner": "abyss-stack"},
    }
    graph = {
        "schema_version": "tos_philosophy_graph_projection_v2",
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived",
        "counts": {"nodes": 2, "edges": 1},
        "nodes": [
            {"node_id": "a", "label": "Alpha", "graph_layers": ["source-relation"], "source_ref": "ToS/canon/a.json"},
            {"node_id": "b", "label": "Beta", "graph_layers": ["source-relation"], "source_ref": "ToS/canon/b.json"},
        ],
        "edges": [
            {
                "edge_id": "e",
                "from_id": "a",
                "to_id": "b",
                "predicate_id": "relates",
                "graph_layers": ["source-relation"],
                "source_ref": "ToS/canon/relations.json",
            }
        ],
        "views": [
            {
                "view_id": "chronology",
                "title": "Chronology",
                "node_ids": ["a", "b"],
                "edge_ids": ["e"],
                "graph_layers": ["source-relation"],
                "source_refs": ["ToS/philosophy/graph-workbench/views/chronology.graph.md"],
            }
        ],
        "clusters": [
            {
                "cluster_id": "c",
                "cluster_kind": "region",
                "label": "Fixture",
                "view_ids": ["chronology"],
                "member_node_ids": ["a", "b"],
                "member_edge_ids": ["e"],
                "graph_layers": ["source-relation"],
                "source_ref": "ToS/philosophy/graph-workbench/clusters/cluster-contracts.json",
            }
        ],
        "review_packets": [{"view_id": "chronology", "unresolved_diagnostics": []}],
        "graph_layers": [{"layer_id": "source-relation"}],
        "layer_counts": [],
        "source_refs": {"source_view_contract_ref": "ToS/philosophy/graph-workbench/view-contracts.json"},
        "runtime_projection_boundary": {"runtime_owner": "abyss-stack"},
        "snapshot_review": {"snapshot_schema_version": "tos_philosophy_graph_projection_snapshot_v1"},
        "unresolved_review_surfaces": [],
    }
    (derived / "tos_corpus_index.min.json").write_text(json.dumps(index), encoding="utf-8")
    (derived / "philosophy_graph_projection.min.json").write_text(json.dumps(graph), encoding="utf-8")
    (audit / "table-i-post-planting-audit.json").write_text(
        json.dumps({"schema_version": "tos_philosophy_post_planting_audit_v1"}),
        encoding="utf-8",
    )
    web_assets = root / "access/web/dist/assets"
    web_assets.mkdir(parents=True)
    (web_assets / "tos-graph.js").write_text("console.log('fixture')", encoding="utf-8")
    (web_assets / "tos-graph.css").write_text("", encoding="utf-8")
    contracts = root / "access/contracts"
    contracts.mkdir(parents=True)
    for name in ("runtime-manifest.v1.json", "runtime-data.v1.json", "web-actions.v1.json"):
        (contracts / name).write_text("{}\n", encoding="utf-8")


class CoreContractTests(unittest.TestCase):
    def test_projection_v2_materializes_global_membership(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            view = core.philosophy_view("chronology")
            self.assertEqual(view["node_count"], 2)
            self.assertEqual(view["edge_count"], 1)
            self.assertEqual(core.philosophy_views()["views"][0]["node_count"], 2)
            self.assertTrue(core.philosophy_path_between("a", "b")["found"])
            self.assertEqual(core.philosophy_neighborhood("a")["neighbors"][0]["node_id"], "b")

    def test_doctor_and_http_use_same_core(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            report = doctor_report(tos_root=root)
            self.assertTrue(report["ok"], report)
            server = make_server(core, port=0)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                base = f"http://127.0.0.1:{server.server_port}"
                health = json.load(urllib.request.urlopen(base + "/health"))
                view = json.load(urllib.request.urlopen(base + "/api/philosophy/views/chronology"))
                self.assertTrue(health["ok"])
                self.assertEqual(view["node_count"], 2)
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)

    @unittest.skipUnless(importlib.util.find_spec("mcp"), "mcp dependency is not installed")
    def test_native_mcp_builds_over_portable_root(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            self.assertIsNotNone(build_server(tos_root=root))


class AuthoredContractTests(unittest.TestCase):
    def test_web_action_contract_matches_browser_adapter(self) -> None:
        payload = json.loads((ACCESS_ROOT / "contracts/web-actions.v1.json").read_text(encoding="utf-8"))
        expected = {
            "tos.status",
            "tos.search",
            "tos.view.open",
            "tos.node.inspect",
            "tos.neighborhood",
            "tos.path.find",
        }
        self.assertEqual({item["action_id"] for item in payload["actions"]}, expected)
        source = (ACCESS_ROOT / "web/src/actions.ts").read_text(encoding="utf-8")
        for action_id in expected:
            self.assertIn(f'"{action_id}"', source)

    def test_standalone_profile_is_abyssos_independent(self) -> None:
        runtime = json.loads((ACCESS_ROOT / "contracts/runtime-manifest.v1.json").read_text(encoding="utf-8"))
        profiles = {item["profile_id"]: item for item in runtime["runtime_profiles"]}
        self.assertFalse(profiles["standalone"]["requires_abyssos"])
        self.assertTrue(profiles["abyssos"]["adapter_only"])
        allowlist = json.loads((ACCESS_ROOT / "contracts/runtime-data.v1.json").read_text(encoding="utf-8"))
        paths = {item["source_path"] for item in allowlist["subjects"]}
        self.assertFalse(any("lexical-search" in path or "/payload/" in path for path in paths))


if __name__ == "__main__":
    unittest.main()


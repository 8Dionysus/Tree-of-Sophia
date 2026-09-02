from __future__ import annotations

import importlib.util
import hashlib
import json
import os
import socket
import sys
import tempfile
import threading
import tomllib
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request
from pathlib import Path

ACCESS_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ACCESS_ROOT.parent
sys.path.insert(0, (ACCESS_ROOT / "src").as_posix())

from tos_access.core import ToSAccessCore  # noqa: E402
from tos_access.doctor import doctor_report  # noqa: E402
from tos_access.http_server import _scale_rows, make_server  # noqa: E402
from tos_access.mcp_server import build_server  # noqa: E402


def load_script(name: str, path: Path) -> object:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


validate_standalone = load_script(
    "validate_standalone",
    ACCESS_ROOT / "packaging/validate_standalone.py",
)


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
        "counts": {"nodes": 3, "edges": 3},
        "nodes": [
            {"node_id": "a", "label": "Alpha", "graph_layers": ["source-relation"], "source_ref": "ToS/canon/a.json"},
            {"node_id": "b", "label": "Beta", "graph_layers": ["source-relation"], "source_ref": "ToS/canon/b.json"},
            {"node_id": "c", "label": "Gamma", "graph_layers": ["source-relation"], "source_ref": "ToS/canon/c.json"},
        ],
        "edges": [
            {
                "edge_id": "e2",
                "from_id": "a",
                "to_id": "c",
                "predicate_id": "relates",
                "graph_layers": ["source-relation"],
                "source_ref": "ToS/canon/relations.json",
            },
            {
                "edge_id": "e",
                "from_id": "a",
                "to_id": "b",
                "predicate_id": "relates",
                "graph_layers": ["source-relation"],
                "source_ref": "ToS/canon/relations.json",
            },
            {
                "edge_id": "e3",
                "from_id": "c",
                "to_id": "b",
                "predicate_id": "relates",
                "graph_layers": ["source-relation"],
                "source_ref": "ToS/canon/relations.json",
            },
        ],
        "views": [
            {
                "view_id": "chronology",
                "title": "Chronology",
                "node_ids": ["a", "b", "c"],
                "edge_ids": ["e2", "e", "e3"],
                "graph_layers": ["source-relation"],
                "source_refs": ["ToS/philosophy/graph-workbench/views/chronology.graph.md"],
            },
            {
                "view_id": "direct-only",
                "title": "Direct only",
                "node_ids": ["a", "b"],
                "edge_ids": ["e"],
                "graph_layers": ["source-relation"],
                "source_refs": ["ToS/philosophy/graph-workbench/views/direct-only.graph.md"],
            },
        ],
        "clusters": [
            {
                "cluster_id": "c",
                "cluster_kind": "region",
                "label": "Fixture",
                "view_ids": ["chronology"],
                "member_node_ids": ["a", "b", "c", "outside"],
                "member_edge_ids": ["e2", "e", "outside-edge"],
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
    def test_standalone_validator_accepts_split_web_contracts(self) -> None:
        validate_standalone._validate_contracts(REPO_ROOT)

    def test_projection_v2_materializes_global_membership(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            view = core.philosophy_view("chronology")
            self.assertEqual(view["node_count"], 3)
            self.assertEqual(view["edge_count"], 3)
            self.assertEqual(len(view["clusters"]), 1)
            self.assertEqual(core.philosophy_views()["views"][0]["node_count"], 3)
            self.assertTrue(core.philosophy_path_between("a", "b")["found"])
            self.assertFalse(core.philosophy_path_between("b", "a")["found"])
            self.assertEqual(core.philosophy_neighborhood("a")["neighbors"][0]["node_id"], "b")

    def test_bounded_graph_packets_keep_edge_endpoints_present(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            neighborhood = core.philosophy_neighborhood("a", limit=1)
            node_ids = {neighborhood["node"]["node_id"], *(node["node_id"] for node in neighborhood["neighbors"])}
            self.assertEqual([node["node_id"] for node in neighborhood["neighbors"]], ["b"])
            self.assertEqual([edge["edge_id"] for edge in neighborhood["edges"]], ["e"])
            self.assertTrue(
                all(edge["from_id"] in node_ids and edge["to_id"] in node_ids for edge in neighborhood["edges"])
            )

            view = core.philosophy_view("chronology", limit=1)
            self.assertEqual(view["node_count"], 1)
            self.assertEqual(view["edge_count"], 0)
            self.assertEqual(view["view"]["node_ids"], ["a"])
            self.assertEqual(view["view"]["edge_ids"], [])
            connected_view = core.philosophy_view("chronology", limit=2)
            connected_node_ids = {node["node_id"] for node in connected_view["nodes"]}
            self.assertEqual(len(connected_view["edges"]), 1)
            self.assertTrue(
                all(
                    edge["from_id"] in connected_node_ids and edge["to_id"] in connected_node_ids
                    for edge in connected_view["edges"]
                )
            )
            self.assertEqual(connected_view["clusters"][0]["member_node_ids"], ["a", "c"])
            self.assertEqual(connected_view["clusters"][0]["member_edge_ids"], ["e2"])
            self.assertEqual(connected_view["clusters"][0]["available_member_node_count"], 4)
            self.assertEqual(connected_view["clusters"][0]["available_member_edge_count"], 3)
            packet = core.philosophy_packet(view_id="chronology", limit=-1)
            self.assertEqual(len(packet["view"]["nodes"]), 1)
            self.assertEqual(packet["view"]["edges"], [])

            deep_neighborhood = core.philosophy_neighborhood("a", depth=2)
            deep_edge_ids = [edge["edge_id"] for edge in deep_neighborhood["edges"]]
            self.assertEqual(len(deep_edge_ids), len(set(deep_edge_ids)))

            projection_path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
            projection = json.loads(projection_path.read_text(encoding="utf-8"))
            projection["nodes"].append(
                {"node_id": "d", "label": "Delta", "graph_layers": ["source-relation"]}
            )
            projection["edges"].insert(
                0,
                {
                    "edge_id": "e3",
                    "from_id": "c",
                    "to_id": "d",
                    "predicate_id": "relates",
                    "graph_layers": ["source-relation"],
                },
            )
            projection_path.write_text(json.dumps(projection), encoding="utf-8")
            connected_prefix = ToSAccessCore.discover(tos_root=root).philosophy_neighborhood("a", depth=2, limit=2)
            connected_ids = {
                connected_prefix["node"]["node_id"],
                *(node["node_id"] for node in connected_prefix["neighbors"]),
            }
            self.assertEqual(len(connected_ids), 3)
            self.assertTrue(
                all(
                    edge["from_id"] in connected_ids and edge["to_id"] in connected_ids
                    for edge in connected_prefix["edges"]
                )
            )
            reached = {connected_prefix["node"]["node_id"]}
            for edge in connected_prefix["edges"]:
                if edge["from_id"] in reached:
                    reached.add(edge["to_id"])
                if edge["to_id"] in reached:
                    reached.add(edge["from_id"])
            self.assertEqual(reached, connected_ids)

    def test_view_packet_does_not_silently_cap_clusters_at_forty(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            projection_path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
            projection = json.loads(projection_path.read_text(encoding="utf-8"))
            template = projection["clusters"][0]
            projection["clusters"] = [
                {**template, "cluster_id": f"cluster-{index}", "label": f"Fixture {index:02d}"}
                for index in range(45)
            ]
            projection_path.write_text(json.dumps(projection), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)
            self.assertEqual(len(core.philosophy_view("chronology", limit=1000)["clusters"]), 45)

    def test_view_packet_applies_cluster_cap_after_graph_intersection(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            projection_path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
            projection = json.loads(projection_path.read_text(encoding="utf-8"))
            template = projection["clusters"][0]
            projection["clusters"] = [
                {
                    **template,
                    "cluster_id": f"irrelevant-{index}",
                    "label": f"A irrelevant {index:02d}",
                    "member_node_ids": ["outside"],
                    "member_edge_ids": ["outside-edge"],
                }
                for index in range(20)
            ] + [
                {**template, "cluster_id": "relevant", "label": "Z relevant"}
            ]
            projection_path.write_text(json.dumps(projection), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)

            packet = core.philosophy_view("chronology", limit=2)

            self.assertEqual([cluster["cluster_id"] for cluster in packet["clusters"]], ["relevant"])

    def test_inline_v1_projection_supports_bounded_and_direct_graph_queries(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            projection_path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
            projection = json.loads(projection_path.read_text(encoding="utf-8"))
            projection["schema_version"] = "tos_philosophy_graph_projection_v1"
            projection["views"][0]["nodes"] = projection.pop("nodes")
            projection["views"][0]["edges"] = projection.pop("edges")
            projection_path.write_text(json.dumps(projection), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)

            bounded = core.philosophy_view("chronology", limit=1)
            self.assertEqual(bounded["node_count"], 1)
            self.assertEqual(bounded["edge_count"], 0)
            self.assertNotIn("nodes", bounded["view"])
            self.assertNotIn("edges", bounded["view"])
            self.assertEqual(bounded["view"]["node_ids"], ["a"])
            self.assertEqual(core.philosophy_node("a")["node"]["label"], "Alpha")
            self.assertEqual(core.philosophy_edge("e")["edge"]["to_id"], "b")
            self.assertEqual(core.philosophy_neighborhood("a")["neighbors"][0]["node_id"], "b")
            self.assertTrue(core.philosophy_path_between("a", "b")["found"])
            self.assertEqual(core.philosophy_scale_packet("nodes")["total_row_count"], 3)
            self.assertIn(
                "nodes",
                {result["collection"] for result in core.philosophy_search("Alpha")["results"]},
            )
            view_result = core.philosophy_search("Chronology", limit=1)["results"][0]["item"]
            self.assertNotIn("nodes", view_result)
            self.assertNotIn("edges", view_result)
            self.assertNotIn("node_ids", view_result)
            self.assertNotIn("edge_ids", view_result)
            self.assertEqual(view_result["node_count"], 3)
            self.assertEqual(view_result["edge_count"], 3)

    def test_path_query_supports_direction_view_exclusion_and_alternatives(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)

            outgoing = core.philosophy_path_between(
                "a",
                "b",
                direction="outgoing",
                alternative_limit=2,
            )
            self.assertEqual(outgoing["schema"], "tos_philosophy_mcp_path_v2")
            self.assertEqual(
                [[edge["edge_id"] for edge in path["edges"]] for path in outgoing["paths"]],
                [["e"], ["e2", "e3"]],
            )

            rerouted = core.philosophy_path_between(
                "a",
                "b",
                direction="outgoing",
                excluded_edge_ids=["e"],
                alternative_limit=2,
            )
            self.assertEqual([edge["edge_id"] for edge in rerouted["edges"]], ["e2", "e3"])
            self.assertEqual(rerouted["excluded_edge_ids"], ["e"])

            reverse = core.philosophy_path_between("b", "a", direction="incoming")
            self.assertEqual([edge["edge_id"] for edge in reverse["edges"]], ["e"])
            self.assertFalse(core.philosophy_path_between("b", "a", direction="outgoing")["found"])

            constrained = core.philosophy_path_between(
                "a",
                "b",
                direction="outgoing",
                view_id="direct-only",
                excluded_edge_ids=["e"],
            )
            self.assertFalse(constrained["found"])
            self.assertEqual(constrained["view_id"], "direct-only")

    def test_path_query_bounds_enqueued_frontier_states(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)

            with patch("tos_access.core.PHILOSOPHY_PATH_FRONTIER_LIMIT", 1):
                bounded = core.philosophy_path_between(
                    "a",
                    "b",
                    direction="outgoing",
                    alternative_limit=2,
                )

            self.assertTrue(bounded["found"])
            self.assertTrue(bounded["exploration_truncated"])
            self.assertEqual(bounded["frontier_limit"], 1)
            self.assertLessEqual(bounded["max_frontier_size"], 1)

    def test_scale_memberships_reference_only_selected_rows(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            node_ids = {row["node_id"] for row in _scale_rows(core, "nodes", "chronology", [])}
            edge_ids = {row["edge_id"] for row in _scale_rows(core, "edges", "chronology", [])}
            node_memberships = _scale_rows(core, "cluster-node-memberships", "chronology", [])
            edge_memberships = _scale_rows(core, "cluster-edge-memberships", "chronology", [])
            clusters = _scale_rows(core, "clusters", "chronology", [])
            self.assertTrue(all(row["node_id"] in node_ids for row in node_memberships))
            self.assertTrue(all(row["edge_id"] in edge_ids for row in edge_memberships))
            self.assertNotIn("outside", {row["node_id"] for row in node_memberships})
            self.assertNotIn("outside-edge", {row["edge_id"] for row in edge_memberships})
            self.assertEqual(clusters[0]["member_node_ids"], ["a", "b", "c"])
            self.assertEqual(clusters[0]["member_edge_ids"], ["e2", "e"])
            expected_cluster_ref = "ToS/philosophy/graph-workbench/clusters/cluster-contracts.json"
            self.assertTrue(all(row["source_ref"] == expected_cluster_ref for row in node_memberships))
            self.assertTrue(all(row["source_ref"] == expected_cluster_ref for row in edge_memberships))

    def test_corpus_runtime_advertises_only_materialized_views(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            index_path = root / "ToS/derived-exports/tos_corpus_index.min.json"
            index = json.loads(index_path.read_text(encoding="utf-8"))
            index["graph_views"].extend(
                [
                    {"view_id": "authority-layers", "title": "Authority layers"},
                    {"view_id": "diff-snapshot", "title": "Snapshot diff"},
                    {"view_id": "node-neighborhood", "title": "Node neighborhood"},
                    {"view_id": "provenance-dag", "title": "Provenance"},
                ]
            )
            index_path.write_text(json.dumps(index), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)

            self.assertEqual(core.status()["graph_views"], ["corpus-topology"])
            self.assertEqual(
                [view["view_id"] for view in core.summary()["graph_views"]],
                ["corpus-topology"],
            )
            self.assertEqual(
                [view["view_id"] for view in core.read_resource("tos-corpus://graph-views")["graph_views"]],
                ["corpus-topology"],
            )
            with self.assertRaisesRegex(KeyError, "unsupported standalone"):
                core.graph_view("authority-layers")
            with self.assertRaisesRegex(KeyError, "unsupported standalone"):
                core.graph_view("node-neighborhood")
            with self.assertRaisesRegex(KeyError, "unsupported standalone"):
                core.graph_view("provenance-dag")

            topology = core.graph_view("corpus-topology")
            node_ids = {node["node_id"] for node in topology["nodes"]}
            self.assertEqual(node_ids, {"view:corpus-topology"})
            self.assertEqual(topology["edges"], [])

    def test_corpus_topology_materializes_branch_tree_in_shared_core(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            index_path = root / "ToS/derived-exports/tos_corpus_index.min.json"
            index = json.loads(index_path.read_text(encoding="utf-8"))
            index["branches"] = [
                {
                    "id": "canon",
                    "path": "ToS/canon",
                    "owner_surface": "ToS/canon/AGENTS.md",
                    "authority_layer": "canon",
                }
            ]
            index_path.write_text(json.dumps(index), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)

            topology = core.graph_view("corpus-topology")
            self.assertEqual(
                [node["node_id"] for node in topology["nodes"]],
                ["view:corpus-topology", "canon"],
            )
            self.assertEqual(topology["edges"][0]["from_id"], "view:corpus-topology")
            self.assertEqual(topology["edges"][0]["to_id"], "canon")
            self.assertEqual(topology["edges"][0]["source_ref"], "ToS/canon/AGENTS.md")
            self.assertEqual(topology["node_count"], 2)
            self.assertEqual(topology["edge_count"], 1)

    def test_corpus_route_and_promotion_views_preserve_relation_topology(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            index_path = root / "ToS/derived-exports/tos_corpus_index.min.json"
            index = json.loads(index_path.read_text(encoding="utf-8"))
            index["graph_views"].extend(
                [
                    {"view_id": "route-graph", "title": "Routes"},
                    {"view_id": "promotion-flow", "title": "Promotion"},
                ]
            )
            index["nodes"] = [
                {"node_id": "a", "label": "Alpha"},
                {"node_id": "b", "label": "Beta"},
            ]
            index["relation_packs"] = [
                {"pack_id": "candidate-intake/fixture", "owner_branch": "ToS/candidate-intake", "path": "ToS/candidate-intake/fixture/edges.csv"},
                {"pack_id": "canon/fixture", "owner_branch": "ToS/canon", "path": "ToS/canon/fixture/edges.csv"},
            ]
            index["relation_edges"] = [
                {
                    "edge_id": "candidate-edge",
                    "owner_branch": "ToS/candidate-intake",
                    "pack_id": "candidate-intake/fixture",
                    "from_id": "candidate-a",
                    "to_id": "candidate-b",
                },
                {
                    "edge_id": "canon-edge",
                    "owner_branch": "ToS/canon",
                    "pack_id": "canon/fixture",
                    "from_id": "a",
                    "to_id": "b",
                },
            ]
            index_path.write_text(json.dumps(index), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)

            routes = core.graph_view("route-graph")
            self.assertEqual([item["edge_id"] for item in routes["items"]], ["canon-edge"])
            self.assertEqual(routes["edges"][0]["source_ref"], "ToS/canon/fixture/edges.csv")
            self.assertEqual([node["node_id"] for node in routes["nodes"]], ["a", "b"])
            promotion = core.graph_view("promotion-flow")
            self.assertEqual(promotion["items"][0]["source_ref"], "ToS/candidate-intake/fixture/edges.csv")
            self.assertEqual(
                [node["node_id"] for node in promotion["nodes"]],
                ["candidate-a", "candidate-b"],
            )
            self.assertEqual(promotion["node_count"], 2)
            inspected_endpoint = core.node("candidate-a")
            self.assertEqual(inspected_endpoint["matches"][0]["node_type"], "relation-endpoint")
            self.assertEqual(
                inspected_endpoint["matches"][0]["source_refs"],
                ["ToS/candidate-intake/fixture/edges.csv"],
            )
            self.assertTrue(
                all(
                    edge["from_id"] in {node["node_id"] for node in promotion["nodes"]}
                    and edge["to_id"] in {node["node_id"] for node in promotion["nodes"]}
                    for edge in promotion["edges"]
                )
            )

    def test_scale_manifest_routes_every_table_to_retrievable_rows(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            manifest = core.philosophy_scale_manifest(view_id="chronology", layers=["source-relation"])
            for table, descriptor in manifest["tables"].items():
                self.assertEqual(descriptor["packet_route"], "tos_philosophy_graph_scale_rows")
                self.assertEqual(
                    descriptor["packet_route_args"],
                    {"table": table, "view_id": "chronology", "layers": ["source-relation"]},
                )
                route_args = descriptor["packet_route_args"]
                packet = core.philosophy_scale_packet(
                    route_args["table"],
                    view_id=route_args["view_id"],
                    layers=route_args["layers"],
                    limit=1,
                )
                self.assertEqual(packet["total_row_count"], descriptor["row_count"])
                self.assertLessEqual(packet["row_count"], 1)
            membership = core.philosophy_scale_packet(
                "cluster-node-memberships",
                view_id="chronology",
                layers=["source-relation"],
                limit=1,
            )
            self.assertEqual(membership["row_count"], 1)
            self.assertEqual(membership["next_offset"], 1)

    def test_direct_corpus_relation_packets_carry_owner_source_refs(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            index_path = root / "ToS/derived-exports/tos_corpus_index.min.json"
            index = json.loads(index_path.read_text(encoding="utf-8"))
            index["relation_packs"] = [
                {"pack_id": "canon/fixture", "path": "ToS/canon/fixture/edges.csv"}
            ]
            index["relation_edges"] = [
                {"edge_id": "e", "pack_id": "canon/fixture", "from_id": "a", "to_id": "b"}
            ]
            index_path.write_text(json.dumps(index), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)
            self.assertEqual(core.node("a")["related_edges"][0]["source_ref"], "ToS/canon/fixture/edges.csv")
            self.assertEqual(core.relation_pack("canon/fixture")["edges"][0]["source_ref"], "ToS/canon/fixture/edges.csv")

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
                limited_view = json.load(urllib.request.urlopen(base + "/api/philosophy/views/chronology?limit=1"))
                rerouted = json.load(urllib.request.urlopen(
                    base + "/api/philosophy/query/paths?from=a&to=b&direction=outgoing&exclude=e&alternatives=2"
                ))
                self.assertTrue(health["ok"])
                self.assertEqual(view["node_count"], 3)
                self.assertEqual(limited_view["node_count"], 1)
                self.assertEqual(limited_view["edge_count"], 0)
                self.assertEqual([edge["edge_id"] for edge in rerouted["edges"]], ["e2", "e3"])
                self.assertEqual(rerouted["direction"], "outgoing")
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)

    def test_health_rejects_invalid_projection_content(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            (root / "ToS/derived-exports/philosophy_graph_projection.min.json").write_text(
                "{not-json",
                encoding="utf-8",
            )
            doctor = doctor_report(tos_root=root)
            self.assertFalse(doctor["ok"])
            self.assertIn("philosophy-graph-schema", doctor["required_failures"])
            server = make_server(ToSAccessCore.discover(tos_root=root), port=0)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                with self.assertRaises(urllib.error.HTTPError) as caught:
                    urllib.request.urlopen(f"http://127.0.0.1:{server.server_port}/health")
                self.assertEqual(caught.exception.code, 503)
                with caught.exception as response:
                    health = json.load(response)
                self.assertFalse(health["ok"])
                self.assertTrue(any("philosophy projection invalid" in error for error in health["errors"]))
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)

    def test_health_and_doctor_reject_unmaterializable_projection(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            projection_path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
            projection = json.loads(projection_path.read_text(encoding="utf-8"))
            projection["review_packets"] = []
            projection_path.write_text(json.dumps(projection), encoding="utf-8")

            doctor = doctor_report(tos_root=root)
            self.assertFalse(doctor["ok"])
            self.assertIn("graph-view-materialization", doctor["required_failures"])
            server = make_server(ToSAccessCore.discover(tos_root=root), port=0)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                with self.assertRaises(urllib.error.HTTPError) as caught:
                    urllib.request.urlopen(f"http://127.0.0.1:{server.server_port}/health")
                self.assertEqual(caught.exception.code, 503)
                with caught.exception as response:
                    health = json.load(response)
                self.assertFalse(health["ok"])
                self.assertTrue(any("philosophy projection invalid" in error for error in health["errors"]))
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)

    def test_direct_corpus_lookups_reject_unknown_identities(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            with self.assertRaisesRegex(KeyError, "unknown ToS corpus node"):
                core.node("missing")
            with self.assertRaisesRegex(KeyError, "unknown ToS corpus relation pack"):
                core.relation_pack("missing")

    def test_ipv6_loopback_uses_ipv6_server(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            try:
                server = make_server(core, host="::1", port=0)
            except OSError as exc:
                self.skipTest(f"IPv6 loopback unavailable: {exc}")
            try:
                self.assertEqual(server.address_family, socket.AF_INET6)
            finally:
                server.server_close()

    def test_abyssos_profile_requires_configured_runtime_root(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            with patch.dict(os.environ, {"TOS_ABYSSOS_ROOT": ""}, clear=False):
                report = doctor_report(tos_root=root, profile="abyssos")
            integration = next(item for item in report["checks"] if item["check_id"] == "abyssos-integration")
            self.assertTrue(integration["required"])
            self.assertFalse(integration["ok"])
            self.assertFalse(report["ok"])

            abyssos_root = root / "AbyssOS"
            (abyssos_root / "abyss-stack").mkdir(parents=True)
            with patch.dict(os.environ, {"TOS_ABYSS_ROOT": "", "TOS_ABYSSOS_ROOT": abyssos_root.as_posix()}):
                configured = doctor_report(tos_root=root, profile="abyssos")
            configured_integration = next(
                item for item in configured["checks"] if item["check_id"] == "abyssos-integration"
            )
            self.assertTrue(configured_integration["ok"])

    def test_bundle_validation_requires_external_archive_digest(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            bundle = root / "candidate.zip"
            bundle.write_bytes(b"tampered")
            sidecar = bundle.with_suffix(bundle.suffix + ".manifest.json")
            expected = b"original"
            sidecar.write_text(
                json.dumps(
                    {
                        "archive_sha256": hashlib.sha256(expected).hexdigest(),
                        "archive_size_bytes": len(expected),
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "archive digest"):
                validate_standalone.validate_bundle(bundle)

    @unittest.skipUnless(importlib.util.find_spec("mcp"), "mcp dependency is not installed")
    def test_native_mcp_builds_over_portable_root(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            self.assertIsNotNone(build_server(tos_root=root))


class AuthoredContractTests(unittest.TestCase):
    def test_release_workflow_installs_standalone_mcp_extra(self) -> None:
        workflow = (REPO_ROOT / ".github/workflows/repo-validation.yml").read_text(encoding="utf-8")
        package = tomllib.loads((ACCESS_ROOT / "pyproject.toml").read_text(encoding="utf-8"))
        requirements = package["project"]["optional-dependencies"]["mcp"]
        self.assertEqual(len(requirements), 1)
        self.assertIn(f"'{requirements[0]}'", workflow)
        self.assertNotIn("-e './access[mcp]'", workflow)
        self.assertNotIn("'./access[mcp]'", workflow)

    def test_query_and_page_contracts_match_browser_adapters(self) -> None:
        query_contract = json.loads(
            (ACCESS_ROOT / "contracts/query-operations.v1.json").read_text(encoding="utf-8")
        )
        expected_operations = {
            "tos.status",
            "tos.search",
            "tos.view.open",
            "tos.node.inspect",
            "tos.neighborhood",
            "tos.path.find",
        }
        self.assertEqual(
            {item["operation_id"] for item in query_contract["operations"]},
            expected_operations,
        )
        query_source = (ACCESS_ROOT / "web/src/query-operations.ts").read_text(encoding="utf-8")
        for operation_id in expected_operations:
            self.assertIn(f'"{operation_id}"', query_source)

        page_contract = json.loads(
            (ACCESS_ROOT / "contracts/page-commands.v1.json").read_text(encoding="utf-8")
        )
        expected_commands = {item["command_id"] for item in page_contract["commands"]}
        page_source = (ACCESS_ROOT / "web/src/page-commands.ts").read_text(encoding="utf-8")
        main_source = (ACCESS_ROOT / "web/src/main.ts").read_text(encoding="utf-8")
        for command_id in expected_commands:
            self.assertIn(f'"{command_id}"', page_source)
            if command_id not in {"tos.page.context", "tos.page.cancel"}:
                self.assertIn(f'"{command_id}"', main_source)

        migration = json.loads((ACCESS_ROOT / "contracts/web-actions.v1.json").read_text(encoding="utf-8"))
        self.assertEqual(migration["status"], "superseded")
        webmcp_source = (ACCESS_ROOT / "web/src/webmcp.ts").read_text(encoding="utf-8")
        self.assertIn("document.modelContext", (ACCESS_ROOT / "README.md").read_text(encoding="utf-8"))
        self.assertIn("registerTool", webmcp_source)
        self.assertIn("context.revision", webmcp_source)

    def test_browser_drops_superseded_view_loads(self) -> None:
        source = (ACCESS_ROOT / "web/src/main.ts").read_text(encoding="utf-8")
        self.assertIn("const loadRevision = ++viewLoadRevision", source)
        self.assertGreaterEqual(source.count("loadRevision !== viewLoadRevision"), 2)
        self.assertIn("const loadRevision = ++modeLoadRevision", source)
        self.assertGreaterEqual(source.count("loadRevision !== modeLoadRevision"), 4)

    def test_browser_preserves_empty_filters_and_hides_unsupported_routes(self) -> None:
        actions = (ACCESS_ROOT / "web/src/query-operations.ts").read_text(encoding="utf-8")
        page = (ACCESS_ROOT / "web/src/main.ts").read_text(encoding="utf-8")
        self.assertIn('["__tos_none__"]', actions)
        self.assertIn('state.mode === "philosophy"', page)
        self.assertGreaterEqual(page.count("state.activeLayers.size === 0"), 1)
        self.assertGreaterEqual(page.count("state.activePredicates.size === 0"), 1)
        self.assertIn('id="scale-export-controls" class="scale-export-controls"', page)
        self.assertNotIn('id="scale-export-controls" hidden', page)
        self.assertIn('"__tos_none__"', page)
        self.assertIn('t("export.noLayers")', page)
        self.assertIn("state.relationItems = relations", page)
        self.assertIn("addGraphEdge(text(item.edge_id", page)

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

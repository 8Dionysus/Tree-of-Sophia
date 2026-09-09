from __future__ import annotations

import importlib.util
import hashlib
import json
import os
import socket
import sqlite3
import sys
import tempfile
import threading
import tomllib
import unittest
from contextlib import closing
from unittest.mock import patch
import urllib.error
import urllib.request
from pathlib import Path

from jsonschema import Draft202012Validator

ACCESS_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ACCESS_ROOT.parent
sys.path.insert(0, (ACCESS_ROOT / "src").as_posix())

from tos_access.core import ToSAccessCore  # noqa: E402
from tos_access.doctor import doctor_report  # noqa: E402
from tos_access.http_server import _scale_rows, make_server  # noqa: E402
from tos_access.mcp_server import build_server  # noqa: E402

# Keep local fixture shutdown responsive; the production server keeps its
# standard serve_forever default.
TEST_SERVER_POLL_INTERVAL = 0.01


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
edge_build = load_script(
    "edge_build_runtime",
    ACCESS_ROOT / "deploy/cloudflare-worker/scripts/build_runtime.py",
)


def write_fixture(root: Path) -> None:
    derived = root / "ToS/derived-exports"
    graph_derived = derived / "graph"
    audit = root / "ToS/philosophy/graph-workbench/review-packets"
    derived.mkdir(parents=True)
    graph_derived.mkdir(parents=True)
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
        "source_navigation": {
            "schema_version": "tos_source_navigation_v1",
            "authority_boundary": "fixture source authority",
            "counts": {"nodes": 7, "edges": 6, "rights": 1},
            "nodes": [
                {"node_id": "philosophy.eras.fixture", "node_kind": "era", "label": "Fixture era", "source_ref": "ToS/philosophy/eras/fixture/branch.manifest.json", "identity_status": "not_applicable", "properties": {}},
                {"node_id": "tos.work.fixture", "node_kind": "work", "label": "Fixture Work", "source_ref": "ToS/source-witnesses/works/fixture/work.json", "identity_status": "verified", "properties": {}},
                {"node_id": "tos.expression.fixture", "node_kind": "expression", "label": "Fixture expression", "source_ref": "ToS/source-witnesses/works/fixture/expression.json", "identity_status": "verified", "properties": {}},
                {"node_id": "tos.edition.fixture", "node_kind": "edition", "label": "Fixture edition", "source_ref": "ToS/source-witnesses/works/fixture/edition.json", "identity_status": "verified", "properties": {}},
                {"node_id": "tos.item.fixture", "node_kind": "item", "label": "Fixture Item", "source_ref": "ToS/source-witnesses/works/fixture/item.json", "identity_status": "verified", "properties": {}},
                {"node_id": "tos.file.sha256.fixture", "node_kind": "file", "label": "fixture.pdf", "source_ref": "ToS/source-witnesses/works/fixture/item.manifest.json", "identity_status": "content_addressed", "properties": {}},
                {"node_id": "tos.link.fixture.download", "node_kind": "link", "label": "Fixture download", "source_ref": "ToS/source-witnesses/links/fixture/link.json", "identity_status": "verified", "properties": {"uri": "https://example.test/fixture.pdf", "access_status": "open_download"}},
            ],
            "edges": [
                {"edge_id": "sn1", "from_id": "philosophy.eras.fixture", "predicate_id": "grounds", "to_id": "tos.work.fixture", "edge_kind": "authored_source_planting", "review_status": "unreviewed", "source_refs": ["ToS/philosophy/eras/fixture/source-planting.json"]},
                {"edge_id": "sn1a", "from_id": "tos.work.fixture", "predicate_id": "has_expression", "to_id": "tos.expression.fixture", "edge_kind": "evidence_claim", "review_status": "unreviewed", "source_refs": ["ToS/source-witnesses/relations/fixture.jsonl"]},
                {"edge_id": "sn1b", "from_id": "tos.expression.fixture", "predicate_id": "embodied_by", "to_id": "tos.edition.fixture", "edge_kind": "evidence_claim", "review_status": "unreviewed", "source_refs": ["ToS/source-witnesses/relations/fixture.jsonl"]},
                {"edge_id": "sn2", "from_id": "tos.edition.fixture", "predicate_id": "exemplified_by", "to_id": "tos.item.fixture", "edge_kind": "evidence_claim", "review_status": "unreviewed", "source_refs": ["ToS/source-witnesses/relations/fixture.jsonl"]},
                {"edge_id": "sn3", "from_id": "tos.item.fixture", "predicate_id": "has_file", "to_id": "tos.file.sha256.fixture", "edge_kind": "authored_item_manifest", "review_status": "not_applicable", "source_refs": ["ToS/source-witnesses/works/fixture/item.manifest.json"]},
                {"edge_id": "sn4", "from_id": "tos.item.fixture", "predicate_id": "downloadable_at", "to_id": "tos.link.fixture.download", "edge_kind": "evidence_claim", "review_status": "unreviewed", "source_refs": ["ToS/source-witnesses/relations/object-link-claims.jsonl"]},
            ],
            "rights": [
                {"rights_id": "tos.rights.fixture", "scope_refs": ["tos.item.fixture", "tos.file.sha256.fixture"], "assessment_status": "licensed", "review_status": "unreviewed", "redistribution_posture": "authorized_with_conditions", "derivative_posture": "allowed_with_conditions", "server_processing_posture": "authorized_with_conditions", "visibility": "public_payload", "license_uri": "https://example.test/license", "rights_statement_uri": "https://example.test/metadata", "restrictions": ["attribution"], "source_ref": "ToS/source-witnesses/works/fixture/rights.json"}
            ],
        },
    }
    graph = {
        "schema_version": "tos_philosophy_graph_projection_v2",
        "owner_repo": "Tree-of-Sophia",
        "surface_kind": "derived",
        "counts": {"nodes": 3, "edges": 3},
        "nodes": [
            {
                "node_id": "a",
                "label": "Alpha",
                "node_type": "candidate-node",
                "multilingual": {"label": {"ru": "Альфа", "en": "Alpha", "original": None}},
                "graph_layers": ["source-relation"],
                "view_ids": ["chronology", "direct-only"],
                "source_ref": "ToS/canon/a.json",
                "properties": {
                    "original_node_type": "concept",
                    "canon_status": "pre-canon",
                    "period": "fixture period",
                },
            },
            {
                "node_id": "b",
                "label": "Beta",
                "node_type": "work",
                "graph_layers": ["source-relation"],
                "view_ids": ["chronology", "direct-only"],
                "source_ref": "ToS/canon/b.json",
                "properties": {},
            },
            {
                "node_id": "c",
                "label": "Gamma",
                "node_type": "source",
                "graph_layers": ["source-relation"],
                "view_ids": ["chronology"],
                "source_ref": "ToS/canon/c.json",
                "properties": {},
            },
        ],
        "edges": [
            {
                "edge_id": "e2",
                "from_id": "a",
                "to_id": "c",
                "predicate_id": "relates",
                "graph_layers": ["source-relation"],
                "source_ref": "ToS/canon/relations.json",
                "view_ids": ["chronology"],
                "properties": {"comment": "Alpha is evidenced by Gamma."},
            },
            {
                "edge_id": "e",
                "from_id": "a",
                "to_id": "b",
                "predicate_id": "relates",
                "graph_layers": ["source-relation"],
                "source_ref": "ToS/canon/relations.json",
                "view_ids": ["chronology", "direct-only"],
                "properties": {},
            },
            {
                "edge_id": "e3",
                "from_id": "c",
                "to_id": "b",
                "predicate_id": "relates",
                "graph_layers": ["source-relation"],
                "source_ref": "ToS/canon/relations.json",
                "view_ids": ["chronology"],
                "properties": {},
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
    bibliographic_nodes = {}
    bibliographic_edges = []
    claim_traces = []
    for edge in index.get('source_navigation', {}).get('edges', []):
        if edge.get('edge_kind') != 'evidence_claim':
            continue
        claim_ref = 'tos.claim.fixture.' + edge['edge_id']
        edge['claim_ref'] = claim_ref
        claim_node = 'claim:' + claim_ref
        refs = edge['source_refs']
        bibliographic_nodes[claim_node] = {'node_id': claim_node, 'node_kind': 'claim', 'label': edge['predicate_id'],
            'source_refs': refs, 'properties': {'claim_ref': claim_ref, 'predicate': edge['predicate_id'], 'claim_version': 1, 'review_status': 'unreviewed'}}
        for role, endpoint in [('subject', edge['from_id']), ('object', edge['to_id'])]:
            identity = 'identity:' + endpoint
            bibliographic_nodes[identity] = {'node_id': identity, 'node_kind': 'identity', 'label': endpoint,
                'source_refs': refs, 'properties': {'identity_ref': endpoint, 'identity_kind': endpoint.split('.')[1]}}
            bibliographic_edges.append({'edge_id': edge['edge_id'] + ':' + role, 'edge_kind': 'has_' + role,
                'from_id': claim_node, 'to_id': identity, 'claim_ref': claim_ref, 'review_status': 'unreviewed',
                'source_claim_file_ref': refs[0]})
        claim_traces.append({'claim_ref': claim_ref, 'claim_node_id': claim_node, 'predicate': edge['predicate_id'],
            'subject_node_id': 'identity:' + edge['from_id'], 'object_node_id': 'identity:' + edge['to_id'],
            'evidence_node_ids': [], 'review_status': 'unreviewed', 'epistemic_status': 'reported'})
    (derived / "tos_corpus_index.min.json").write_text(json.dumps(index), encoding="utf-8")
    (derived / "philosophy_graph_projection.min.json").write_text(json.dumps(graph), encoding="utf-8")
    (graph_derived / "source-witness-bibliographic-claims.min.json").write_text(
        json.dumps(
            {
                "schema_version": "tos_source_witness_bibliographic_graph_v1",
                "nodes": list(bibliographic_nodes.values()),
                "edges": bibliographic_edges,
                "claim_traces": claim_traces,
            }
        ),
        encoding="utf-8",
    )
    (derived / "epistemic_evidence_projection.min.json").write_text(
        json.dumps(
            {
                "schema_version": "tos_epistemic_evidence_projection_v1",
                "owner_repo": "Tree-of-Sophia",
                "surface_kind": "derived_public_evidence_navigation",
                "scenes": [
                    {
                        "scene_id": "fixture-scene",
                        "selections": [
                            {"mode": "philosophy", "view_id": "chronology", "item_ids": ["a"]}
                        ],
                        "selection_ids": ["a"],
                        "posture": "contested-pre-canon",
                        "finding": "Fixture evidence route remains open.",
                        "conclusion": {
                            "can_conclude": False,
                            "canon_membership": False,
                            "claim_evidence_closed": False,
                            "allowed": ["the selection is present in the projection"],
                            "not_allowed": ["semantic truth"],
                        },
                        "source_anchors": [],
                        "routes": [
                            {
                                "route_kind": "candidate",
                                "ref": "ToS/canon/a.json",
                                "status": "fixture",
                                "exists": True,
                            }
                        ],
                        "gaps": ["review"],
                        "source_refs": ["ToS/canon/a.json"],
                    }
                ],
                "authority_boundary": {
                    "is_source": False,
                    "is_canon": False,
                    "is_semantic_truth": False,
                    "is_rights_clearance": False,
                    "note": "Fixture authority remains with the referenced source.",
                },
            }
        ),
        encoding="utf-8",
    )
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
    for name in (
        "knowledge-api.v1.json",
        "knowledge-graph.v1.schema.json",
        "lens-spec.v1.schema.json",
        "lens-result.v1.schema.json",
        "temporal-comparison-request.v1.schema.json",
        "temporal-comparison-result.v1.schema.json",
        "exploration-request.v1.schema.json",
        "exploration-result.v1.schema.json",
        "exploration-request.v2.schema.json",
        "exploration-result.v2.schema.json",
    ):
        (contracts / name).write_text(
            (ACCESS_ROOT / "contracts" / name).read_text(encoding="utf-8"),
            encoding="utf-8",
        )
    tos_contracts = root / "ToS/contracts"
    semantic_interchange = root / "ToS/doctrine/semantic-interchange"
    tos_contracts.mkdir(parents=True)
    semantic_interchange.mkdir(parents=True)
    for name in (
        "semantic-entity-type-registry.schema.json",
        "semantic-relation-type-registry.schema.json",
    ):
        (tos_contracts / name).write_text(
            (REPO_ROOT / "ToS/contracts" / name).read_text(encoding="utf-8"),
            encoding="utf-8",
        )
    for name in ("entity-types.v1.json", "relation-types.v1.json"):
        (semantic_interchange / name).write_text(
            (REPO_ROOT / "ToS/doctrine/semantic-interchange" / name).read_text(
                encoding="utf-8"
            ),
            encoding="utf-8",
        )


class CoreContractTests(unittest.TestCase):
    def test_core_inspection_reuses_and_replaces_one_snapshot_index(self) -> None:
        from concurrent.futures import ThreadPoolExecutor
        from copy import deepcopy
        from tos_access.knowledge import KnowledgeGraphIndex, inspect_knowledge_node, inspect_knowledge_relation
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            write_fixture(root)
            core = ToSAccessCore.discover(root)
            initial = core.knowledge_graph()
            identifier = initial['nodes'][0]['id']
            relation_id = initial['relations'][0]['id']
            changed = deepcopy(initial)
            changed['nodes'][0]['display']['title']['default'] = 'New snapshot title'
            # A new normalization snapshot may share the source revision. Bind
            # the immutable graph instance, not just that convenient string.
            with patch('tos_access.core.KnowledgeGraphIndex', wraps=KnowledgeGraphIndex) as prepare:
                for graph, count in ((initial, 1), (changed, 2)):
                    with patch.object(ToSAccessCore, 'knowledge_graph', return_value=graph):
                        with ThreadPoolExecutor(max_workers=4) as pool:
                            packets = list(pool.map(core.knowledge_node, [identifier] * 8))
                        self.assertTrue(all(packet == inspect_knowledge_node(graph, identifier)
                                            for packet in packets))
                        for limit in (0, 3):
                            self.assertEqual(core.knowledge_node(identifier, limit),
                                             inspect_knowledge_node(graph, identifier, limit))
                            self.assertEqual(core.knowledge_relation(relation_id),
                                             inspect_knowledge_relation(graph, relation_id))
                        self.assertEqual(prepare.call_count, count)
                        self.assertIs(core._graph_index.graph, graph)

    def test_core_search_reuses_only_its_current_snapshot(self) -> None:
        from unittest.mock import patch
        from copy import deepcopy
        from tos_access.knowledge import KnowledgeSearchIndex, search_knowledge_graph
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder); write_fixture(root)
            core = ToSAccessCore.discover(root)
            initial = core.knowledge_graph()
            changed = deepcopy(initial)
            changed['nodes'][0]['attributes']['search_probe'] = 'new-snapshot-only'
            with patch('tos_access.core.KnowledgeSearchIndex', wraps=KnowledgeSearchIndex) as prepare:
                with patch.object(ToSAccessCore, 'knowledge_graph', return_value=initial):
                    for query in ('Alpha', 'Альфа', 'missing'):
                        self.assertEqual(core.knowledge_search(query), search_knowledge_graph(initial, query))
                    self.assertEqual(prepare.call_count, 1)
                with patch.object(ToSAccessCore, 'knowledge_graph', return_value=changed):
                    packet = core.knowledge_search('new-snapshot-only')
                    self.assertEqual(packet, search_knowledge_graph(changed, 'new-snapshot-only'))
                    self.assertEqual(packet['counts']['matching_nodes'], 1)
                    self.assertEqual(prepare.call_count, 2)

    def test_knowledge_graph_normalizes_every_item_for_humans_and_agents(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)

            graph = core.knowledge_graph()

            self.assertEqual(graph["schema"], "tos_knowledge_graph_v1")
            self.assertIn("philosophy:a", {node["id"] for node in graph["nodes"]})
            self.assertIn("canon:a", {node["id"] for node in graph["nodes"]})
            philosophy_alpha = next(node for node in graph["nodes"] if node["id"] == "philosophy:a")
            self.assertEqual(philosophy_alpha["kind_id"], "concept")
            self.assertEqual(philosophy_alpha["type_id"], "tos.entity.concept")
            self.assertEqual(philosophy_alpha["type_mapping"]["status"], "mapped")
            self.assertEqual(philosophy_alpha["display"]["title"]["ru"], "Альфа")
            self.assertEqual(philosophy_alpha["epistemic"]["canon_status"], "pre-canon")
            for node in graph["nodes"]:
                self.assertTrue(node["display"]["title"]["default"])
                self.assertTrue(node["display"]["kind_label"]["default"])
                self.assertTrue(node["display"]["summary"]["default"])
                self.assertIn(node["display"]["summary_state"], {"authored", "source-derived", "metadata-synthesis", "missing"})
                self.assertTrue(node["source_refs"])
            for relation in graph["relations"]:
                self.assertTrue(relation["display"]["label"]["default"])
                self.assertTrue(relation["display"]["statement"]["default"])
                self.assertTrue(relation["display"]["explanation"]["default"])
                self.assertIn(relation["display"]["explanation_state"], {"authored", "source-derived", "metadata-synthesis", "missing"})
                self.assertTrue(relation["source_refs"])

    def test_arbitrary_lens_spec_is_compiled_without_a_known_view_id(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            spec = {
                "schema_version": "tos_lens_spec_v1",
                "lens_id": "operator.concepts-with-context",
                "title": {"default": "Concepts with context", "ru": "Понятия с контекстом", "en": "Concepts with context"},
                "sources": ["philosophy"],
                "node_query": {
                    "match": "all",
                    "filters": [{"field": "kind_id", "op": "eq", "value": "concept"}],
                },
                "relation_query": {
                    "match": "all",
                    "filters": [{"field": "predicate_id", "op": "eq", "value": "relates"}],
                },
                "traversal": {"depth": 1, "direction": "either", "predicate_ids": ["relates"]},
                "composition": {
                    "endpoint_policy": "either",
                    "group_by": ["kind_id"],
                    "sort_nodes": [{"field": "display.title.default", "direction": "asc"}],
                    "sort_relations": [{"field": "id", "direction": "asc"}],
                },
                "presentation": {
                    "layout": "semantic",
                    "color_by": "kind_id",
                    "lane_by": "epistemic.canon_status",
                    "size_by": None,
                    "inspector_fields": ["display.summary", "epistemic", "source_refs"],
                },
                "limits": {"nodes": 20, "relations": 20, "groups": 20},
            }

            result = core.compile_knowledge_lens(spec)

            self.assertEqual(result["schema"], "tos_lens_result_v1")
            self.assertEqual(result["lens"]["lens_id"], "operator.concepts-with-context")
            self.assertEqual(result["presentation"]["layout"], "semantic")
            self.assertEqual(
                {node["id"] for node in result["nodes"]},
                {"philosophy:a", "philosophy:b", "philosophy:c"},
            )
            self.assertEqual(
                {relation["id"] for relation in result["relations"]},
                {"philosophy:e", "philosophy:e2", "philosophy:e3"},
            )
            self.assertEqual(result["groups"][0]["field"], "kind_id")
            self.assertEqual(len(result["fingerprint"]), 64)
            self.assertEqual(result["fingerprint"], core.compile_knowledge_lens(spec)["fingerprint"])
            self.assertFalse(result["authority_boundary"]["is_source"])

    def test_knowledge_catalog_exposes_saved_lenses_and_safe_composition_grammar(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)

            catalog = core.knowledge_catalog()

            self.assertEqual(catalog["schema"], "tos_knowledge_catalog_v1")
            self.assertIn("chronology", {lens["lens_id"] for lens in catalog["lenses"]})
            self.assertIn("concept", {kind["kind_id"] for kind in catalog["node_kinds"]})
            self.assertIn("relates", {predicate["predicate_id"] for predicate in catalog["predicates"]})
            self.assertIn("contains", catalog["capabilities"]["filter_operators"])
            self.assertEqual(catalog["capabilities"]["maximums"]["traversal_depth"], 5)
            self.assertEqual(catalog["counts"]["display_coverage"]["node_titles"], catalog["counts"]["nodes"])
            contracts = core.knowledge_contracts()
            self.assertEqual(contracts["schema"], "tos_knowledge_contract_bundle_v1")
            self.assertEqual(
                set(contracts["contracts"]),
                {
                    "api",
                    "knowledge_graph",
                    "lens_spec",
                    "lens_result",
                    "temporal_comparison_request",
                    "temporal_comparison_result",
                    "entity_type_registry_schema",
                    "relation_type_registry_schema",
                    "entity_type_registry",
                    "relation_type_registry",
                },
            )
            self.assertEqual(
                contracts["contracts"]["lens_spec"]["title"],
                "Tree of Sophia declarative LensSpec v1",
            )

            with self.assertRaisesRegex(ValueError, "unsupported node filter field"):
                core.compile_knowledge_lens(
                    {
                        "schema_version": "tos_lens_spec_v1",
                        "lens_id": "unsafe",
                        "node_query": {"match": "all", "filters": [{"field": "__proto__.polluted", "op": "eq", "value": "yes"}]},
                    }
                )

            with self.assertRaisesRegex(ValueError, "unsupported node filter field"):
                core.compile_knowledge_lens(
                    {
                        "schema_version": "tos_lens_spec_v1",
                        "lens_id": "unsafe-nested",
                        "node_query": {"match": "all", "filters": [{"field": "attributes.safe.constructor.name", "op": "eq", "value": "x"}]},
                    }
                )

    def test_edge_data_revision_ignores_api_only_changes_but_tracks_rows_and_sources(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            source_paths = [
                root / name
                for name in (
                    "index.json",
                    "philosophy.json",
                    "bibliographic.json",
                    "entity-registry.json",
                    "relation-registry.json",
                    "evidence.json",
                    "audit.json",
                )
            ]
            for index, source_path in enumerate(source_paths):
                source_path.write_text(json.dumps({"revision": index}), encoding="utf-8")
            contract = root / "access/contracts/lens-spec.v1.schema.json"
            contract.parent.mkdir(parents=True)
            contract.write_text('{"revision":1}', encoding="utf-8")
            graph = {
                "source_revision": "a" * 64,
                "nodes": [{"id": "n", "content_revision": "b" * 64}],
                "relations": [{"id": "r", "content_revision": "c" * 64}],
                "catalog_hint": "first",
            }

            class FakeCore:
                tos_root = root
                (
                    index_path,
                    philosophy_graph_projection_path,
                    bibliographic_graph_path,
                    entity_type_registry_path,
                    relation_type_registry_path,
                    evidence_projection_path,
                    philosophy_post_planting_audit_path,
                ) = source_paths

                @staticmethod
                def knowledge_graph() -> dict[str, object]:
                    return graph

                @staticmethod
                def zarathustra_word_analysis_public_capability() -> dict[str, object]:
                    return {"available": False, "reason": "fixture"}

            with patch.object(edge_build, "REPO_ROOT", root):
                baseline = edge_build.data_revision(FakeCore())
                contract.write_text('{"revision":2}', encoding="utf-8")
                graph["catalog_hint"] = "second"
                self.assertEqual(edge_build.data_revision(FakeCore()), baseline)

                graph["nodes"][0]["content_revision"] = "d" * 64
                self.assertNotEqual(edge_build.data_revision(FakeCore()), baseline)
                graph["nodes"][0]["content_revision"] = "b" * 64

                graph['query_properties'] = [{'property_id': 'tos.property.test', 'field': 'attributes.test'}]
                self.assertNotEqual(edge_build.data_revision(FakeCore()), baseline)
                graph.pop('query_properties')
                self.assertEqual(edge_build.data_revision(FakeCore()), baseline)

                source_paths[0].write_text('{"revision":"changed"}', encoding="utf-8")
                self.assertNotEqual(edge_build.data_revision(FakeCore()), baseline)

    def test_edge_sql_preserves_oversized_lossless_json_in_bounded_statements(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            target = Path(raw) / "read-model.sql"
            writer = edge_build.SqlStatementWriter(target)
            writer.append(
                "CREATE TABLE knowledge_nodes_next "
                "(id TEXT PRIMARY KEY, search_text TEXT NOT NULL, json TEXT NOT NULL);"
            )
            item_json = json.dumps(
                {"source_payload": ("Zarathustra's Überfluss — " * 6_000)},
                ensure_ascii=False,
                separators=(",", ":"),
            )
            edge_build.append_chunkable_insert(
                writer,
                "knowledge_nodes_next",
                ("id", "search_text", "json"),
                (
                    edge_build.sql_text("fixture:large"),
                    edge_build.sql_text(item_json.lower()),
                    edge_build.sql_text(item_json),
                ),
                selector_sql=f"id = {edge_build.sql_text('fixture:large')}",
                chunked_text={"search_text": item_json.lower(), "json": item_json},
            )
            writer.finish()

            statements = target.read_text(encoding="utf-8").splitlines()
            self.assertTrue(
                all(
                    len(statement.encode("utf-8"))
                    <= edge_build.MAX_D1_SQL_STATEMENT_BYTES
                    for statement in statements
                )
            )
            with closing(sqlite3.connect(":memory:")) as database:
                database.executescript(target.read_text(encoding="utf-8"))
                stored = database.execute(
                    "SELECT search_text, json FROM knowledge_nodes_next WHERE id = ?",
                    ("fixture:large",),
                ).fetchone()
            self.assertEqual(stored, (item_json.lower(), item_json))

    def test_relation_first_lenses_and_unified_inspection_need_no_legacy_mode(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            result = core.compile_knowledge_lens(
                {
                    "schema_version": "tos_lens_spec_v1",
                    "lens_id": "relations-first",
                    "sources": ["philosophy"],
                    "node_query": {"enabled": False, "match": "all", "filters": []},
                    "relation_query": {
                        "match": "all",
                        "filters": [{"field": "predicate_id", "op": "eq", "value": "relates"}],
                    },
                    "composition": {"endpoint_policy": "independent"},
                    "limits": {"nodes": 10, "relations": 10, "groups": 10},
                }
            )
            self.assertEqual({item["id"] for item in result["relations"]}, {"philosophy:e", "philosophy:e2", "philosophy:e3"})
            self.assertEqual({item["id"] for item in result["nodes"]}, {"philosophy:a", "philosophy:b", "philosophy:c"})

            search = core.knowledge_search("Альфа", sources=["philosophy"], limit=5)
            self.assertEqual(search["nodes"][0]["id"], "philosophy:a")
            exact = core.knowledge_node("philosophy:a")
            self.assertFalse(exact["ambiguous_native_id"])
            ambiguous = core.knowledge_node("a")
            self.assertTrue(ambiguous["ambiguous_native_id"])
            relation = core.knowledge_relation("philosophy:e")
            self.assertEqual({item["id"] for item in relation["endpoints"]}, {"philosophy:a", "philosophy:b"})
            focused = core.knowledge_focus("philosophy:a", sources=["philosophy"], depth=1)
            self.assertEqual(focused["focus"]["node_id"], "philosophy:a")
            self.assertEqual(focused["presentation"]["layout"], "radial")

    def test_http_accepts_only_read_only_lens_compilation_post(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            server = make_server(ToSAccessCore.discover(tos_root=root), port=0)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                spec = json.dumps(
                    {
                        "schema_version": "tos_lens_spec_v1",
                        "lens_id": "http.fixture",
                        "sources": ["philosophy"],
                        "node_query": {"match": "all", "filters": []},
                        "limits": {"nodes": 2, "relations": 2, "groups": 10},
                    }
                ).encode("utf-8")
                request = urllib.request.Request(
                    f"http://127.0.0.1:{server.server_port}/api/knowledge/lenses/compile",
                    data=spec,
                    method="POST",
                    headers={"Content-Type": "application/json"},
                )
                packet = json.load(urllib.request.urlopen(request))
                self.assertEqual(packet["schema"], "tos_lens_result_v1")
                self.assertEqual(packet["lens"]["lens_id"], "http.fixture")

                base = f"http://127.0.0.1:{server.server_port}"
                catalog = json.load(urllib.request.urlopen(base + "/api/knowledge/catalog"))
                contracts = json.load(urllib.request.urlopen(base + "/api/knowledge/contracts"))
                search = json.load(urllib.request.urlopen(base + "/api/knowledge/search?query=Alpha&sources=philosophy"))
                node = json.load(urllib.request.urlopen(base + "/api/knowledge/nodes/philosophy%3Aa"))
                relation = json.load(urllib.request.urlopen(base + "/api/knowledge/relations/philosophy%3Ae"))
                focused = json.load(urllib.request.urlopen(
                    base + "/api/knowledge/focus/philosophy%3Aa?sources=philosophy&depth=5"
                ))
                self.assertEqual(catalog["schema"], "tos_knowledge_catalog_v1")
                self.assertEqual(contracts["schema"], "tos_knowledge_contract_bundle_v1")
                self.assertEqual(search["nodes"][0]["id"], "philosophy:a")
                self.assertEqual(node["matches"][0]["id"], "philosophy:a")
                self.assertEqual(relation["matches"][0]["id"], "philosophy:e")
                self.assertEqual(focused["focus"]["node_id"], "philosophy:a")
                self.assertEqual(focused["lens"]["traversal"]["depth"], 5)
                for inspected in (search, node, relation):
                    self.assertEqual(inspected['source_revision'], focused['source_revision'])
                paged_spec = {**json.loads(spec), 'pagination': {'nodes': 1, 'relations': 1}}
                def compile_page(value):
                    return urllib.request.urlopen(urllib.request.Request(base + '/api/knowledge/lenses/compile',
                        data=json.dumps(value).encode(), headers={'Content-Type': 'application/json'}))
                with compile_page(paged_spec) as response:
                    page = json.load(response)
                self.assertTrue(page['page']['next_cursor'])
                paged_spec['pagination']['cursor'] = page['page']['next_cursor']
                paged_spec['lens_id'] = 'different-query'
                with self.assertRaises(urllib.error.HTTPError) as conflict:
                    compile_page(paged_spec)
                self.assertEqual(conflict.exception.code, 409)
                conflict.exception.close()

                health = json.load(urllib.request.urlopen(base + "/health"))
                self.assertTrue(health["ok"])
                self.assertEqual(health["knowledge_schema"], "tos_knowledge_graph_v1")
                knowledge_counts = health["knowledge_counts"]
                coverage = knowledge_counts["display_coverage"]
                self.assertEqual(coverage["node_summaries"], knowledge_counts["nodes"])
                self.assertEqual(coverage["relation_explanations"], knowledge_counts["relations"])

                oversized = urllib.request.Request(
                    base + "/api/knowledge/lenses/compile",
                    data=b"x" * (64 * 1024 + 1),
                    method="POST",
                    headers={"Content-Type": "application/json"},
                )
                with self.assertRaises(urllib.error.HTTPError) as caught:
                    urllib.request.urlopen(oversized)
                self.assertEqual(caught.exception.code, 413)
                caught.exception.close()

                forbidden = urllib.request.Request(
                    f"http://127.0.0.1:{server.server_port}/api/corpus/status",
                    data=b"{}",
                    method="POST",
                    headers={"Content-Type": "application/json"},
                )
                with self.assertRaises(urllib.error.HTTPError) as caught:
                    urllib.request.urlopen(forbidden)
                self.assertEqual(caught.exception.code, 405)
                caught.exception.close()
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)
    def test_source_navigation_and_dossiers_keep_access_separate_from_rights(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            index_path = root / "ToS/derived-exports/tos_corpus_index.min.json"
            index = json.loads(index_path.read_text(encoding="utf-8"))
            navigation = index["source_navigation"]
            navigation["nodes"].extend(
                [
                    {
                        "node_id": "tos.work.neighbor",
                        "node_kind": "work",
                        "label": "Neighbor Work",
                        "source_ref": "ToS/source-witnesses/works/neighbor/work.json",
                        "identity_status": "verified",
                        "properties": {},
                    },
                    {
                        "node_id": "tos.link.neighbor",
                        "node_kind": "link",
                        "label": "Neighbor Link",
                        "source_ref": "ToS/source-witnesses/links/neighbor/link.json",
                        "identity_status": "verified",
                        "properties": {
                            "uri": "https://example.test/neighbor",
                            "access_status": "open_download",
                        },
                    },
                ]
            )
            navigation["edges"].extend(
                [
                    {
                        "edge_id": "sn5",
                        "from_id": "philosophy.eras.fixture",
                        "predicate_id": "grounds",
                        "to_id": "tos.work.neighbor",
                        "edge_kind": "authored_source_planting",
                        "review_status": "unreviewed",
                        "source_refs": [
                            "ToS/philosophy/eras/fixture/neighbor-source-planting.json"
                        ],
                    },
                    {
                        "edge_id": "sn6",
                        "from_id": "tos.work.neighbor",
                        "predicate_id": "downloadable_at",
                        "to_id": "tos.link.neighbor",
                        "edge_kind": "evidence_claim",
                        "review_status": "unreviewed",
                        "source_refs": ["ToS/source-witnesses/relations/neighbor.jsonl"],
                    },
                ]
            )
            navigation["counts"] = {"nodes": 9, "edges": 8, "rights": 1}
            index_path.write_text(json.dumps(index), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)

            descent = core.source_descend("philosophy.eras.fixture")
            self.assertEqual(descent["counts"], {"nodes": 9, "edges": 8})
            self.assertEqual(descent["nodes"][-1]["depth"], 5)
            dossier = core.source_dossier("tos.link.fixture.download")
            self.assertEqual(dossier["agent_summary"]["technical_access"], "downloadable")
            self.assertEqual(dossier["agent_summary"]["rights_posture"], "candidate_requires_human_review")
            self.assertFalse(dossier["agent_summary"]["can_conclude_legal_openness"])
            self.assertFalse(dossier["agent_summary"]["availability_is_license"])
            self.assertEqual(dossier["tree_paths"][0]["node_ids"][-1], "tos.link.fixture.download")
            self.assertEqual(dossier["chain"]["work"][0]["node_id"], "tos.work.fixture")
            self.assertNotIn("tos.work.neighbor", {node["node_id"] for node in dossier["chain"]["work"]})
            self.assertNotIn("tos.link.neighbor", {node["node_id"] for node in dossier["chain"]["link"]})
            for object_id, kind in [
                ("tos.work.fixture", "work"), ("tos.expression.fixture", "expression"),
                ("tos.edition.fixture", "edition"), ("tos.item.fixture", "item"),
                ("tos.file.sha256.fixture", "file"), ("tos.link.fixture.download", "link"),
            ]:
                with self.subTest(object_id=object_id):
                    selected = core.source_dossier(object_id)
                    self.assertEqual(selected["object"]["node_kind"], kind)
                    self.assertEqual(selected["tree_paths"][0]["node_ids"][-1], object_id)
                    self.assertIn("tos.work.fixture", {node["node_id"] for node in selected["chain"]["work"]})

    def test_local_word_analysis_provider_is_capability_gated_and_source_bound(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)
            public_capability = core.zarathustra_word_analysis_public_capability()
            self.assertFalse(public_capability["available"])
            self.assertIsNone(public_capability["task"])
            self.assertEqual(
                public_capability["reason"],
                "local source-bound word-analysis provider is excluded from the public bundle",
            )
            unavailable = core.zarathustra_word_analysis_task("судьбы", "ru", rank=2)
            self.assertFalse(unavailable["available"])
            self.assertIsNone(unavailable["task"])
            self.assertEqual(unavailable["publication_posture"], "excluded_from_public_bundle")

            provider = root / "scripts/prepare_zarathustra_word_analysis_v1.py"
            provider.parent.mkdir(parents=True)
            symlink_target = provider.parent / "provider-target.py"
            symlink_target.write_text("raise AssertionError('must not load through symlink')\n", encoding="utf-8")
            provider.symlink_to(symlink_target)
            symlinked = ToSAccessCore.discover(tos_root=root).zarathustra_word_analysis_task(
                "судьбы", "ru",
            )
            self.assertFalse(symlinked["available"])
            provider.unlink()
            provider.write_text(
                "def build_task(*args, **kwargs):\n"
                "    raise RuntimeError('private source-return artifact must be a regular non-symlink: fixture')\n",
                encoding="utf-8",
            )
            private_missing = ToSAccessCore.discover(tos_root=root).zarathustra_word_analysis_task(
                "судьбы", "ru",
            )
            self.assertFalse(private_missing["available"])
            self.assertIsNone(private_missing["task"])
            self.assertEqual(private_missing["reason"], "private source-return artifacts are not installed")
            provider.write_text(
                "def build_task(query, language, rank=1, include_semantic_neighbors=False, request_path=None):\n"
                "    return {\n"
                "        'schema_version': 'tos_zarathustra_word_analysis_task_v1',\n"
                "        'analysis_task_id': 'task:fixture',\n"
                "        'query': query, 'language': language, 'rank': rank,\n"
                "        'include_semantic_neighbors': include_semantic_neighbors,\n"
                "        'source': {'language': 'de', 'surface': 'Schicksal', 'exact_context': 'mein Schicksal'},\n"
                "        'authority': {'accepted': False, 'semantic_fact_asserted': False, 'canon_effect': False},\n"
                "    }\n",
                encoding="utf-8",
            )
            available = ToSAccessCore.discover(tos_root=root).zarathustra_word_analysis_task(
                "судьбы", "ru", rank=2, include_semantic_neighbors=True,
            )
            self.assertTrue(available["available"])
            self.assertEqual(available["task"]["source"]["surface"], "Schicksal")
            self.assertEqual(available["task"]["rank"], 2)
            self.assertTrue(available["task"]["include_semantic_neighbors"])
            self.assertFalse(available["authority"]["is_semantic_truth"])
            self.assertFalse(available["authority"]["writes_to_tree"])

    def test_http_exposes_same_local_word_analysis_capability(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            provider = root / "scripts/prepare_zarathustra_word_analysis_v1.py"
            provider.parent.mkdir(parents=True)
            provider.write_text(
                "def build_task(query, language, rank=1, include_semantic_neighbors=False, request_path=None):\n"
                "    return {'schema_version': 'tos_zarathustra_word_analysis_task_v1', "
                "'query': query, 'language': language, 'rank': rank, "
                "'include_semantic_neighbors': include_semantic_neighbors, "
                "'source': {'language': 'de', 'exact_context': 'mein Schicksal'}, "
                "'authority': {'accepted': False, 'semantic_fact_asserted': False, 'canon_effect': False}}\n",
                encoding="utf-8",
            )
            server = make_server(ToSAccessCore.discover(tos_root=root), port=0)
            thread = threading.Thread(
                target=server.serve_forever,
                kwargs={"poll_interval": TEST_SERVER_POLL_INTERVAL},
                daemon=True,
            )
            thread.start()
            try:
                url = (
                    f"http://127.0.0.1:{server.server_port}/api/zarathustra/word-analysis"
                    "?query=%D1%81%D1%83%D0%B4%D1%8C%D0%B1%D1%8B&language=ru&rank=3"
                    "&include_semantic_neighbors=true"
                )
                packet = json.load(urllib.request.urlopen(url))
                self.assertTrue(packet["available"])
                self.assertEqual(packet["task"]["query"], "судьбы")
                self.assertEqual(packet["task"]["rank"], 3)
                self.assertTrue(packet["task"]["include_semantic_neighbors"])
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=5)

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

    def test_epistemic_packet_separates_challenges_from_adjudicated_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            projection_path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
            projection = json.loads(projection_path.read_text(encoding="utf-8"))
            projection["nodes"][0]["properties"] = {
                "authority_posture": "prepared_research_candidate",
                "canon_status": "pre-canon",
                "dossier_id": "A01",
            }
            projection["edges"][1]["predicate_id"] = "contested_by"
            projection["edges"][1]["properties"] = {
                "authority_posture": "prepared_research_candidate",
                "canon_status": "pre-canon",
                "comment": "the proposed reading is disputed",
                "confidence": "High",
            }
            projection["edges"][0]["predicate_id"] = "uncertain_relation"
            projection["edges"][0]["properties"] = {
                "authority_posture": "prepared_research_candidate",
                "canon_status": "pre-canon",
                "comment": "the chronology remains uncertain",
            }
            projection_path.write_text(json.dumps(projection), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)

            packet = core.philosophy_epistemic_packet("a", view_id="direct-only")

            self.assertEqual(packet["schema"], "tos_philosophy_epistemic_packet_v1")
            self.assertEqual(packet["selection"]["node_id"], "a")
            self.assertEqual([row["edge_id"] for row in packet["challenge_relations"]], ["e"])
            self.assertEqual(packet["context_relations"], [])
            self.assertEqual(packet["selection_posture"]["authority_posture"], "prepared_research_candidate")
            self.assertEqual(packet["selection_posture"]["canon_status"], "pre-canon")
            self.assertIsNone(packet["selection_posture"]["confidence"])
            self.assertFalse(packet["selection_posture"]["claim_evidence_closed"])
            self.assertEqual(packet["field_posture"]["authority_postures"], ["prepared_research_candidate"])
            self.assertEqual(packet["field_posture"]["canon_statuses"], ["pre-canon"])
            self.assertEqual(packet["field_posture"]["confidence_values"], ["High"])
            self.assertEqual(packet["coverage"]["posture"], "partial")
            self.assertEqual(packet["coverage"]["challenge_state"], "projected_signals")
            self.assertIn("claim-level support and counterevidence", packet["coverage"]["missing_surfaces"])
            self.assertIn("not adjudicated counterevidence", packet["authority_note"])
            self.assertEqual(
                packet["source_refs"],
                ["ToS/canon/a.json", "ToS/canon/b.json", "ToS/canon/relations.json"],
            )

            edge_packet = core.philosophy_epistemic_packet("e", view_id="direct-only")
            self.assertEqual(edge_packet["selection"]["edge_id"], "e")
            self.assertEqual([node["node_id"] for node in edge_packet["neighbor_nodes"]], ["a", "b"])
            self.assertEqual([row["edge_id"] for row in edge_packet["challenge_relations"]], ["e"])
            self.assertEqual(edge_packet["field_posture"]["confidence_values"], [])

            truncated = core.philosophy_epistemic_packet("e3", view_id="chronology", limit=1)
            self.assertEqual(truncated["challenge_relations"], [])
            self.assertEqual(truncated["context_relations"][0]["edge_id"], "e3")
            self.assertEqual(truncated["coverage"]["challenge_state"], "projected_signals_truncated")
            self.assertEqual(truncated["coverage"]["available_challenge_relations"], 2)
            self.assertEqual(truncated["coverage"]["returned_challenge_relations"], 0)

    def test_evidence_lens_joins_selection_to_explicit_routes_without_inventing_closure(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            core = ToSAccessCore.discover(tos_root=root)

            packet = core.evidence_lens_packet("philosophy", "a", view_id="chronology")

            self.assertEqual(packet["schema"], "tos_evidence_lens_packet_v1")
            self.assertEqual(packet["mode"], "philosophy")
            self.assertEqual(packet["selection"]["node_id"], "a")
            self.assertEqual(packet["scene"]["scene_id"], "fixture-scene")
            self.assertFalse(packet["scene"]["conclusion"]["can_conclude"])
            self.assertEqual(packet["agent_summary"]["finding"], "Fixture evidence route remains open.")
            self.assertFalse(packet["agent_summary"]["can_conclude"])
            self.assertLess(len(json.dumps(packet["agent_summary"])), 1500)
            schema = json.loads(
                (ACCESS_ROOT / "contracts/evidence-lens-packet.v1.schema.json").read_text(encoding="utf-8")
            )
            Draft202012Validator(schema).validate(packet)

    def test_research_workspace_contract_keeps_session_hypotheses_non_authoritative(self) -> None:
        schema = json.loads(
            (ACCESS_ROOT / "contracts/research-workspace.v1.schema.json").read_text(encoding="utf-8")
        )
        Draft202012Validator.check_schema(schema)
        packet = {
            "schema": "tos_research_workspace_session_v1",
            "version": 1,
            "session_id": "demo",
            "revision": 3,
            "selected_lens": {"id": "edge:contested", "kind": "edge", "label": "Contested relation"},
            "excluded_edge_ids": ["edge:contested"],
            "route_snapshots": [],
            "hypotheses": [
                {
                    "id": "hypothesis:alternate",
                    "title": "Alternate reading",
                    "body": "Treat this relation as a local possibility.",
                    "target_id": "edge:contested",
                    "from_id": "node:a",
                    "to_id": "node:b",
                    "predicate_label": "might imply",
                    "posture": {
                        "session_hypothesis": True,
                        "source": False,
                        "reviewed": False,
                        "canon": False,
                    },
                }
            ],
            "notes": [],
            "journal": [{"sequence": 1, "action": "hypothesis.add", "target_id": "hypothesis:alternate"}],
        }
        Draft202012Validator(schema).validate(packet)
        packet["hypotheses"][0]["posture"]["canon"] = True
        with self.assertRaises(Exception):
            Draft202012Validator(schema).validate(packet)

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
            index_path = root / "ToS/derived-exports/tos_corpus_index.min.json"
            index = json.loads(index_path.read_text(encoding="utf-8"))
            index["nodes"].append(
                {"node_id": "b", "label": "Beta", "source_ref": "ToS/canon/b.json"}
            )
            index["relation_packs"] = [
                {"pack_id": "canon/fixture", "owner_branch": "ToS/canon", "path": "ToS/canon/fixture/edges.csv"}
            ]
            index["relation_edges"] = [
                {
                    "edge_id": "corpus-e",
                    "pack_id": "canon/fixture",
                    "owner_branch": "ToS/canon",
                    "authority_layer": "canon",
                    "status": "canon",
                    "from_id": "a",
                    "to_id": "b",
                }
            ]
            index["graph_views"].append({"view_id": "route-graph", "title": "Routes"})
            index_path.write_text(json.dumps(index), encoding="utf-8")
            core = ToSAccessCore.discover(tos_root=root)
            report = doctor_report(tos_root=root)
            self.assertTrue(report["ok"], report)
            server = make_server(core, port=0)
            thread = threading.Thread(
                target=server.serve_forever,
                kwargs={"poll_interval": TEST_SERVER_POLL_INTERVAL},
                daemon=True,
            )
            thread.start()
            try:
                base = f"http://127.0.0.1:{server.server_port}"
                health = json.load(urllib.request.urlopen(base + "/health"))
                with urllib.request.urlopen(
                    urllib.request.Request(base + "/health", method="HEAD")
                ) as head:
                    self.assertEqual(head.status, 200)
                    self.assertGreater(int(head.headers["Content-Length"]), 0)
                    self.assertEqual(head.read(), b"")
                view = json.load(urllib.request.urlopen(base + "/api/philosophy/views/chronology"))
                limited_view = json.load(urllib.request.urlopen(base + "/api/philosophy/views/chronology?limit=1"))
                rerouted = json.load(urllib.request.urlopen(
                    base + "/api/philosophy/query/paths?from=a&to=b&direction=outgoing&exclude=e&alternatives=2"
                ))
                epistemic = json.load(urllib.request.urlopen(
                    base + "/api/philosophy/query/epistemic/a?view_id=direct-only"
                ))
                corpus_evidence = json.load(urllib.request.urlopen(
                    base + "/api/corpus/query/epistemic/corpus-e?view_id=route-graph"
                ))
                source_descent = json.load(urllib.request.urlopen(
                    base + "/api/source/navigation/philosophy.eras.fixture?max_depth=8&limit=20"
                ))
                source_dossier = json.load(urllib.request.urlopen(
                    base + "/api/source/dossiers/tos.link.fixture.download?limit=20"
                ))
                self.assertTrue(health["ok"])
                self.assertEqual(view["node_count"], 3)
                self.assertEqual(limited_view["node_count"], 1)
                self.assertEqual(limited_view["edge_count"], 0)
                self.assertEqual([edge["edge_id"] for edge in rerouted["edges"]], ["e2", "e3"])
                self.assertEqual(rerouted["direction"], "outgoing")
                self.assertEqual(epistemic["selection"]["node_id"], "a")
                self.assertEqual(epistemic["view_id"], "direct-only")
                self.assertNotIn("query_backend", epistemic)
                self.assertEqual(corpus_evidence["schema"], "tos_evidence_lens_packet_v1")
                self.assertEqual(corpus_evidence["selection"]["edge_id"], "corpus-e")
                self.assertEqual(source_descent["schema"], "tos_source_descent_v1")
                self.assertEqual(source_dossier["schema"], "tos_source_dossier_v1")
                self.assertFalse(source_dossier["agent_summary"]["availability_is_license"])
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
            thread = threading.Thread(
                target=server.serve_forever,
                kwargs={"poll_interval": TEST_SERVER_POLL_INTERVAL},
                daemon=True,
            )
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
            thread = threading.Thread(
                target=server.serve_forever,
                kwargs={"poll_interval": TEST_SERVER_POLL_INTERVAL},
                daemon=True,
            )
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

    def test_abyssos_profile_is_blocked_by_tos_freeze(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            runtime_path = root / "access/contracts/runtime-manifest.v1.json"
            runtime_path.write_text(
                json.dumps(
                    {
                        "integration_posture": {
                            "state": "paused",
                            "scope": ["abyssos"],
                            "external_activation": "disabled",
                        }
                    }
                ),
                encoding="utf-8",
            )
            abyssos_root = root / "AbyssOS"
            (abyssos_root / "abyss-stack").mkdir(parents=True)
            with patch.dict(os.environ, {"TOS_ABYSSOS_ROOT": abyssos_root.as_posix()}):
                report = doctor_report(tos_root=root, profile="abyssos")
            integration = next(item for item in report["checks"] if item["check_id"] == "abyssos-integration")
            freeze = next(item for item in report["checks"] if item["check_id"] == "abyssos-integration-freeze")
            self.assertTrue(integration["ok"])
            self.assertFalse(freeze["ok"])
            self.assertFalse(report["ok"])

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
    def test_repo_validation_publishes_a_validated_standalone_candidate(self) -> None:
        workflow = (REPO_ROOT / ".github/workflows/repo-validation.yml").read_text(encoding="utf-8")
        self.assertIn("playwright install --with-deps chromium", workflow)
        self.assertIn("access/e2e/test_webmcp.py", workflow)
        self.assertIn("build_standalone_bundle.py", workflow)
        self.assertIn("validate_standalone.py --bundle", workflow)
        self.assertIn("actions/upload-artifact@", workflow)
        self.assertIn("tree-of-sophia-standalone.zip.manifest.json", workflow)
        self.assertIn("if-no-files-found: error", workflow)

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
            "tos.snapshot",
            "tos.search",
            "tos.source-gaps.search",
            "tos.source.descend",
            "tos.dossier.inspect",
            "tos.view.open",
            "tos.node.inspect",
            "tos.neighborhood",
            "tos.path.find",
            "tos.epistemic.inspect",
            "tos.zarathustra.word-analysis.prepare",
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

    def test_browser_commits_only_current_completed_view_loads(self) -> None:
        source = (ACCESS_ROOT / "web/src/main.ts").read_text(encoding="utf-8")
        load_mode = source.split("async function loadMode(", 1)[1].split("async function loadView(", 1)[0]
        load_view = source.split("async function loadView(", 1)[1].split("async function search(", 1)[0]
        self.assertIn("const loadRevision = ++viewLoadRevision", load_view)
        self.assertIn("loadRevision !== viewLoadRevision", load_view)
        self.assertLess(load_view.index("await prepareView"), load_view.index("commitPreparedView"))
        self.assertIn("const loadRevision = ++modeLoadRevision", load_mode)
        self.assertGreaterEqual(load_mode.count("loadRevision !== modeLoadRevision"), 2)
        self.assertGreaterEqual(load_mode.count("signal?.throwIfAborted()"), 2)
        self.assertEqual(load_mode.count("commitPreparedView"), 2)

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
        self.assertEqual(runtime["integration_posture"]["state"], "paused")
        self.assertEqual(runtime["integration_posture"]["external_activation"], "disabled")
        abyssos_profile = json.loads((ACCESS_ROOT / "profiles/abyssos.v1.json").read_text(encoding="utf-8"))
        self.assertEqual(abyssos_profile["availability"], "paused")
        allowlist = json.loads((ACCESS_ROOT / "contracts/runtime-data.v1.json").read_text(encoding="utf-8"))
        paths = {item["source_path"] for item in allowlist["subjects"]}
        self.assertFalse(any("lexical-search" in path or "/payload/" in path for path in paths))


if __name__ == "__main__":
    unittest.main()

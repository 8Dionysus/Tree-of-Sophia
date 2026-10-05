from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


ACCESS_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ACCESS_ROOT.parent
KNOWLEDGE_FIXTURE_ROOT = ACCESS_ROOT / "tests" / "fixtures" / "knowledge-contract"


def knowledge_fixture_path(relative: str) -> Path:
    """Return one bounded source-shaped knowledge fixture, never a corpus path."""
    path = KNOWLEDGE_FIXTURE_ROOT / relative
    if (
        path.is_symlink()
        or not path.is_file()
        or path.resolve() != path.absolute()
        or not path.resolve().is_relative_to(KNOWLEDGE_FIXTURE_ROOT.resolve())
    ):
        raise FileNotFoundError(f"knowledge fixture is not a regular local file: {relative}")
    return path


def load_knowledge_fixture(relative: str):
    return json.loads(knowledge_fixture_path(relative).read_text(encoding="utf-8"))


@contextmanager
def canonical_node_fixture():
    """Materialize exact canonical snapshots under their native ToS paths."""
    pairs = (
        "ToS/canon/support/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/zarathustra/node.json",
        "ToS/canon/support/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/zarathustra/node.human-forms.json",
        "ToS/canon/event/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/departure-from-origin/node.json",
        "ToS/canon/event/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/departure-from-origin/node.human-forms.json",
    )
    with tempfile.TemporaryDirectory(prefix="tos-knowledge-canon-") as directory:
        root = Path(directory)
        for relative in pairs:
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(knowledge_fixture_path(relative), target)
        schema_ref = "ToS/contracts/tos-node-contract.schema.json"
        schema = REPO_ROOT / schema_ref
        if schema.is_symlink() or not schema.is_file():
            raise FileNotFoundError(f"canonical node schema is unavailable: {schema}")
        schema_target = root / schema_ref
        schema_target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(schema, schema_target)
        yield root, tuple(root / relative for relative in (pairs[0], pairs[2]))


def write_corpus_topology_fixture(root: Path) -> None:
    """The existing route/promotion corpus input shared by native read parity."""
    write_fixture(root)
    index_path = root / "ToS/derived-exports/tos_corpus_index.min.json"
    index = json.loads(index_path.read_text(encoding="utf-8"))
    index["graph_views"].extend([
        {"view_id": "route-graph", "title": "Routes"},
        {"view_id": "promotion-flow", "title": "Promotion"},
    ])
    index["nodes"] = [{"node_id": "a", "label": "Alpha"}, {"node_id": "b", "label": "Beta"}]
    index["relation_packs"] = [
        {"pack_id": "candidate-intake/fixture", "owner_branch": "ToS/candidate-intake", "path": "ToS/candidate-intake/fixture/edges.csv"},
        {"pack_id": "canon/fixture", "owner_branch": "ToS/canon", "path": "ToS/canon/fixture/edges.csv"},
    ]
    index["relation_edges"] = [
        {"edge_id": "candidate-edge", "owner_branch": "ToS/candidate-intake", "pack_id": "candidate-intake/fixture", "from_id": "candidate-a", "to_id": "candidate-b"},
        {"edge_id": "canon-edge", "owner_branch": "ToS/canon", "pack_id": "canon/fixture", "from_id": "a", "to_id": "b"},
    ]
    index_path.write_text(json.dumps(index), encoding="utf-8")


def write_evidence_fixture(root: Path) -> None:
    """Write bounded authored inputs and derive Evidence via the explicit oracle.

    The shared fixture has no canonical anchors or evidence closure. Both the
    Python adapter and native compiler can inspect the same complete source
    definition, schema and route bytes without a production corpus.
    """
    source_ref = "ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json"
    candidate_ref = "ToS/philosophy/graph-workbench/proposed-nodes/fixture.json"
    canon_ref = (
        "ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/"
        "prologue-1/edges.csv"
    )
    schema_ref = "ToS/contracts/epistemic-evidence-projection.schema.json"
    source = {
        "schema_version": "tos_evidence_lens_scene_bindings_v1",
        "surface_role": "fixture-only navigation bindings; no canon or rights authority",
        "scenes": [{
            "scene_id": "fixture-scene",
            "selections": [{"mode": "philosophy", "view_id": "chronology", "item_ids": ["a"]}],
            "posture": "contested-pre-canon",
            "finding": "Fixture evidence route remains open.",
            "finding_ru": "Маршрут Evidence в тестовом fixture остаётся открытым.",
            "conclusion": {
                "can_conclude": False,
                "canon_membership": False,
                "claim_evidence_closed": False,
                "allowed": ["the selection is present in the projection"],
                "not_allowed": ["semantic truth", "rights clearance"],
            },
            "anchor_edge_ids": [],
            "routes": [{"route_kind": "candidate", "ref": candidate_ref, "status": "fixture-only"}],
            "gaps": ["review"],
            "gaps_ru": ["review"],
        }],
    }
    for relative, text in (
        (source_ref, json.dumps(source, ensure_ascii=False) + "\n"),
        (candidate_ref, json.dumps({
            "node_id": "a", "label": "Alpha", "canon_status": "pre-canon", "fixture_only": True,
        }) + "\n"),
        (canon_ref, "edge_id,anchor_segment_ids,witness_scope\n"),
    ):
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
    schema_target = root / schema_ref
    schema_target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(REPO_ROOT / schema_ref, schema_target)
    (root / "ToS/derived-exports").mkdir(parents=True, exist_ok=True)
    # The maintained builder exposes an explicit reference mode. A subprocess
    # isolates its source-root binding from other fixture callers and imports.
    subprocess.run(
        [sys.executable, str(REPO_ROOT / "scripts/build_epistemic_evidence_projection.py"),
         "--legacy-oracle", "--source-root", str(root.resolve())],
        check=True, stdout=subprocess.DEVNULL,
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
    write_evidence_fixture(root)
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
        "exploration-request.v1.schema.json",
        "exploration-result.v1.schema.json",
    ):
        (contracts / name).write_text(
            (ACCESS_ROOT / "contracts" / name).read_text(encoding="utf-8"),
            encoding="utf-8",
        )
    tos_contracts = root / "ToS/contracts"
    semantic_interchange = root / "ToS/doctrine/semantic-interchange"
    tos_contracts.mkdir(parents=True, exist_ok=True)
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


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Generate the access test fixture.")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output
    if output.exists():
        if not output.is_dir():
            parser.error(f"output is not a directory: {output}")
        if any(output.iterdir()):
            parser.error(f"output directory is not empty: {output}")
    else:
        output.mkdir(parents=True)
    write_fixture(output)

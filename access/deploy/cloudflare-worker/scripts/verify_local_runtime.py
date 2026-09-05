#!/usr/bin/env python3
"""Compare representative Cloudflare Worker packets with the ToS Python core."""

from __future__ import annotations

import json
import socket
import subprocess
import tempfile
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable


WORKER_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = Path(__file__).resolve().parents[4]
ACCESS_SRC = REPO_ROOT / "access" / "src"
if ACCESS_SRC.as_posix() not in sys.path:
    sys.path.insert(0, ACCESS_SRC.as_posix())

from tos_access.core import ToSAccessCore  # noqa: E402


def free_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def normalize_paths(value: Any) -> Any:
    prefix = REPO_ROOT.resolve().as_posix() + "/"
    if isinstance(value, str):
        if value == REPO_ROOT.resolve().as_posix():
            return "Tree-of-Sophia"
        if value.startswith(prefix):
            return value.removeprefix(prefix)
        return value
    if isinstance(value, list):
        return [normalize_paths(item) for item in value]
    if isinstance(value, dict):
        return {key: normalize_paths(item) for key, item in value.items()}
    return value


def fetch_json(base: str, path: str) -> dict[str, Any]:
    with urllib.request.urlopen(base + path, timeout=20) as response:
        payload = json.load(response)
    if not isinstance(payload, dict):
        raise AssertionError(f"{path} did not return a JSON object")
    return payload


def post_json(base: str, path: str, payload: dict[str, Any]) -> dict[str, Any]:
    request = urllib.request.Request(
        base + path,
        data=json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8"),
        method="POST",
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        result = json.load(response)
    if not isinstance(result, dict):
        raise AssertionError(f"{path} did not return a JSON object")
    return result


def fetch_jsonl(base: str, path: str) -> list[dict[str, Any]]:
    with urllib.request.urlopen(base + path, timeout=30) as response:
        rows = [json.loads(line) for line in response if line.strip()]
    if not all(isinstance(row, dict) for row in rows):
        raise AssertionError(f"{path} did not return JSON objects")
    return rows


def wait_ready(base: str, process: subprocess.Popen[str]) -> None:
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if process.poll() is not None:
            output = process.stdout.read() if process.stdout else ""
            raise RuntimeError(f"wrangler dev exited before readiness:\n{output}")
        try:
            if fetch_json(base, "/health").get("ok") is True:
                return
        except (OSError, ValueError):
            time.sleep(0.2)
    raise TimeoutError("wrangler dev did not become healthy within 30 seconds")


def main() -> int:
    core = ToSAccessCore.discover(REPO_ROOT)
    port = free_port()
    base = f"http://127.0.0.1:{port}"
    # A never-drained PIPE can block Wrangler logging and internal asset reads.
    logs = tempfile.TemporaryFile(mode='w+t')
    process = subprocess.Popen(
        ["npx", "wrangler", "dev", "--local", "--ip", "127.0.0.1", "--port", str(port)],
        cwd=WORKER_ROOT,
        stdout=logs,
        stderr=subprocess.STDOUT,
        text=True,
    )
    try:
        wait_ready(base, process)
        node_id = "candidate-node:table-i-a01-node-016"
        target_id = "candidate-node:table-i-a01-node-014"
        source_work_id = (
            "tos.work.egyptian-scholarship."
            "on-four-songs-contained-in-an-egyptian-papyrus-in-the-british-museum"
        )
        knowledge_node_id = "philosophy:philosophy.atlas"
        knowledge_author_id = "source-navigation:tos.agent.friedrich-nietzsche"
        knowledge_work_id = "tos.work.friedrich-nietzsche.also-sprach-zarathustra"
        large_knowledge_node_id = "canon:tos.source.thus-spoke-zarathustra.prologue"
        knowledge_node_packet = core.knowledge_node(knowledge_node_id, 20)
        related_relations = knowledge_node_packet.get("related_relations", [])
        if not related_relations:
            raise AssertionError(f"knowledge parity node has no relation: {knowledge_node_id}")
        knowledge_relation_id = str(related_relations[0]["id"])
        quote = lambda value: urllib.parse.quote(value, safe="")
        cases: list[tuple[str, Callable[[], dict[str, Any]], str]] = [
            ("corpus status", core.status, "/api/corpus/status"),
            ("corpus summary", core.summary, "/api/corpus/summary"),
            ("philosophy status", core.philosophy_status, "/api/philosophy/status"),
            ("philosophy views", core.philosophy_views, "/api/philosophy/views"),
            ("knowledge catalog", core.knowledge_catalog, "/api/knowledge/catalog"),
            ("knowledge contracts", core.knowledge_contracts, "/api/knowledge/contracts"),
            (
                "knowledge search",
                lambda: core.knowledge_search("Zarathustra", sources=["philosophy"], limit=5),
                "/api/knowledge/search?query=Zarathustra&sources=philosophy&limit=5",
            ),
            (
                "Unicode knowledge search",
                lambda: core.knowledge_search("Заратустра", sources=["philosophy"], limit=5),
                f"/api/knowledge/search?query={quote('Заратустра')}&sources=philosophy&limit=5",
            ),
            (
                "knowledge node",
                lambda: knowledge_node_packet,
                f"/api/knowledge/nodes/{quote(knowledge_node_id)}?relation_limit=20",
            ),
            (
                "lossless large knowledge node",
                lambda: core.knowledge_node(large_knowledge_node_id, 20),
                f"/api/knowledge/nodes/{quote(large_knowledge_node_id)}?relation_limit=20",
            ),
            (
                "knowledge relation",
                lambda: core.knowledge_relation(knowledge_relation_id),
                f"/api/knowledge/relations/{quote(knowledge_relation_id)}",
            ),
            (
                "focused knowledge neighborhood",
                lambda: core.knowledge_focus(
                    knowledge_node_id,
                    sources=["philosophy"],
                    depth=1,
                    direction="either",
                    node_limit=40,
                    relation_limit=40,
                ),
                f"/api/knowledge/focus/{quote(knowledge_node_id)}?sources=philosophy&depth=1&direction=either&node_limit=40&relation_limit=40",
            ),
            (
                "author-to-works knowledge neighborhood",
                lambda: core.knowledge_focus(
                    knowledge_author_id,
                    sources=["source-navigation"],
                    depth=1,
                    direction="either",
                    node_limit=40,
                    relation_limit=40,
                ),
                f"/api/knowledge/focus/{quote(knowledge_author_id)}?sources=source-navigation&depth=1&direction=either&node_limit=40&relation_limit=40",
            ),
            (
                "cross-layer work knowledge neighborhood",
                lambda: core.knowledge_focus(
                    knowledge_work_id,
                    sources=["canon", "source-navigation", "source-claims", "semantic-interchange"],
                    depth=5,
                    direction="either",
                    predicate_ids=[
                        "authored_by",
                        "has_expression",
                        "embodied_by",
                        "has_subject",
                        "has_object",
                        "has_normalized_place",
                        "projects",
                        "grounded_in",
                        "commentary-on",
                    ],
                    node_limit=400,
                    relation_limit=800,
                ),
                f"/api/knowledge/focus/{quote(knowledge_work_id)}?"
                "sources=canon,source-navigation,source-claims,semantic-interchange&depth=5&direction=either&"
                "predicates=authored_by,has_expression,embodied_by,has_subject,has_object,has_normalized_place,projects,grounded_in,commentary-on&"
                "node_limit=400&relation_limit=800",
            ),
            (
                "stored knowledge lens",
                lambda: core.stored_knowledge_lens("corpus-topology"),
                "/api/knowledge/lenses/corpus-topology",
            ),
            (
                "Zarathustra word-analysis capability",
                core.zarathustra_word_analysis_public_capability,
                "/api/zarathustra/word-analysis?query=Geist&language=de&rank=2&include_semantic_neighbors=true",
            ),
            ("chronology view", lambda: core.philosophy_view("chronology", 1000), "/api/philosophy/views/chronology?limit=1000"),
            ("dynamic corpus view", lambda: core.graph_view("route-graph", 37), "/api/corpus/graph-views/route-graph?limit=37"),
            ("corpus search", lambda: core.search("zarathustra", 5), "/api/corpus/search?query=zarathustra&limit=5"),
            ("philosophy search", lambda: core.philosophy_search("Gilgamesh", 5), "/api/philosophy/search?query=Gilgamesh&limit=5"),
            (
                "source descent",
                lambda: core.source_descend(source_work_id, 3, 40),
                f"/api/source/navigation/{quote(source_work_id)}?max_depth=3&limit=40",
            ),
            (
                "source dossier",
                lambda: core.source_dossier(source_work_id, 300),
                f"/api/source/dossiers/{quote(source_work_id)}?limit=300",
            ),
            ("node packet", lambda: core.philosophy_node(node_id), f"/api/philosophy/nodes/{quote(node_id)}"),
            (
                "neighborhood",
                lambda: core.philosophy_neighborhood(node_id, 1, [], [], 10),
                f"/api/philosophy/neighborhood/{quote(node_id)}?depth=1&limit=10",
            ),
            (
                "path",
                lambda: core.philosophy_path_between(node_id, target_id, [], [], 2, "outgoing", None, [], 2),
                f"/api/philosophy/paths?from={quote(node_id)}&to={quote(target_id)}&max_depth=2&direction=outgoing&alternatives=2",
            ),
            (
                "philosophy evidence lens",
                lambda: core.evidence_lens_packet("philosophy", node_id, None, 10),
                f"/api/philosophy/query/epistemic/{quote(node_id)}?limit=10",
            ),
            (
                "corpus evidence lens",
                lambda: core.evidence_lens_packet("corpus", "m113", "route-graph", 10),
                "/api/corpus/query/epistemic/m113?view_id=route-graph&limit=10",
            ),
        ]
        for label, expected, path in cases:
            actual = fetch_json(base, path)
            reference = normalize_paths(expected())
            if actual != reference:
                raise AssertionError(f"Cloudflare contract drift for {label}")
            print(f"ok: {label}")

        arbitrary_lens = {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "edge-contract-smoke",
            "sources": ["philosophy"],
            "node_query": {"enabled": False},
            "relation_query": {
                "filters": [{"field": "predicate_id", "op": "eq", "value": "uses_script"}]
            },
            "composition": {"endpoint_policy": "independent"},
            "limits": {"nodes": 20, "relations": 10, "groups": 10},
        }
        actual_lens = post_json(base, "/api/knowledge/lenses/compile", arbitrary_lens)
        expected_lens = normalize_paths(core.compile_knowledge_lens(arbitrary_lens))
        if actual_lens != expected_lens:
            raise AssertionError("Cloudflare contract drift for arbitrary knowledge lens")
        print("ok: arbitrary knowledge lens")
        compact_lens = {**arbitrary_lens, 'detail': 'compact'}
        if post_json(base, '/api/knowledge/lenses/compile', compact_lens) != normalize_paths(core.compile_knowledge_lens(compact_lens)):
            raise AssertionError('Cloudflare contract drift for compact knowledge carrier')
        print('ok: compact knowledge carrier')
        scoped_lens = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'source-scope-parity',
                       'sources': ['source-claims', 'semantic-interchange'],
                       'seed': {'focus_node_id': knowledge_work_id}, 'node_query': {'enabled': False},
                       'traversal': {'depth': 2, 'profile': 'all'}}
        if post_json(base, '/api/knowledge/lenses/compile', scoped_lens) != normalize_paths(core.compile_knowledge_lens(scoped_lens)):
            raise AssertionError('Cloudflare traversal escaped the selected source scope')
        print('ok: cross-layer traversal preserves source scope')

        null_lens = {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "edge-null-filter-smoke",
            "sources": ["philosophy"],
            "node_query": {
                "filters": [
                    {"field": "attributes.missing_contract_probe", "op": "in", "value": [None]}
                ]
            },
            "relation_query": {"enabled": False},
            "limits": {"nodes": 3, "relations": 0, "groups": 3},
        }
        actual_null_lens = post_json(base, "/api/knowledge/lenses/compile", null_lens)
        expected_null_lens = normalize_paths(core.compile_knowledge_lens(null_lens))
        if actual_null_lens != expected_null_lens:
            raise AssertionError("Cloudflare contract drift for null-valued knowledge filter")
        print("ok: null-valued knowledge filter")

        scale_layers = ["evidence-relation", "historical-relation"]
        scale_query = urllib.parse.urlencode({"view_id": "chronology", "layers": ",".join(scale_layers)})
        actual_manifest = fetch_json(base, f"/api/philosophy/scale-export/manifest?{scale_query}")
        expected_manifest = normalize_paths(core.philosophy_scale_manifest("chronology", scale_layers))
        if actual_manifest != expected_manifest:
            raise AssertionError("Cloudflare contract drift for scale manifest")
        print("ok: scale manifest")

        for table in (
            "nodes",
            "edges",
            "clusters",
            "cluster-node-memberships",
            "cluster-edge-memberships",
        ):
            actual_rows = fetch_jsonl(
                base,
                f"/api/philosophy/scale-export/{table}.jsonl?{scale_query}",
            )
            expected_rows = normalize_paths(core.philosophy_scale_rows(table, "chronology", scale_layers))
            if actual_rows != expected_rows:
                raise AssertionError(f"Cloudflare contract drift for scale export {table}")
            print(f"ok: scale export {table}")

        empty_manifest = fetch_json(
            base,
            "/api/philosophy/scale-export/manifest?view_id=chronology&layers=__tos_none__",
        )
        if any(descriptor["row_count"] for descriptor in empty_manifest["tables"].values()):
            raise AssertionError("Cloudflare scale export did not preserve the explicit empty layer filter")
        with urllib.request.urlopen(
            base + f"/api/philosophy/scale-export/nodes.csv?{scale_query}",
            timeout=30,
        ) as response:
            if response.headers.get_content_type() != "text/csv" or not response.readline().strip():
                raise AssertionError("Cloudflare CSV scale export is not downloadable")
        print("ok: scale export CSV and empty filter")
    except Exception:
        logs.seek(0)
        print(logs.read()[-6000:], file=sys.stderr)
        raise
    finally:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
        logs.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Compare representative Cloudflare Worker packets with the ToS Python core."""

from __future__ import annotations

import json
import socket
import subprocess
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
    process = subprocess.Popen(
        ["npx", "wrangler", "dev", "--local", "--ip", "127.0.0.1", "--port", str(port)],
        cwd=WORKER_ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    try:
        wait_ready(base, process)
        node_id = "candidate-node:table-i-a01-node-016"
        target_id = "candidate-node:table-i-a01-node-014"
        quote = lambda value: urllib.parse.quote(value, safe="")
        cases: list[tuple[str, Callable[[], dict[str, Any]], str]] = [
            ("corpus status", core.status, "/api/corpus/status"),
            ("corpus summary", core.summary, "/api/corpus/summary"),
            ("philosophy status", core.philosophy_status, "/api/philosophy/status"),
            ("philosophy views", core.philosophy_views, "/api/philosophy/views"),
            (
                "Zarathustra word-analysis capability",
                lambda: core.zarathustra_word_analysis_task("Geist", "de", 2, True),
                "/api/zarathustra/word-analysis?query=Geist&language=de&rank=2&include_semantic_neighbors=true",
            ),
            ("chronology view", lambda: core.philosophy_view("chronology", 1000), "/api/philosophy/views/chronology?limit=1000"),
            ("dynamic corpus view", lambda: core.graph_view("route-graph", 37), "/api/corpus/graph-views/route-graph?limit=37"),
            ("corpus search", lambda: core.search("zarathustra", 5), "/api/corpus/search?query=zarathustra&limit=5"),
            ("philosophy search", lambda: core.philosophy_search("Gilgamesh", 5), "/api/philosophy/search?query=Gilgamesh&limit=5"),
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
    finally:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

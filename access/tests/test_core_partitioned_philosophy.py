"""Focused coverage for the core's lazy partitioned philosophy carrier."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from unittest.mock import patch

import sys

ACCESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS_ROOT / "src"))
sys.path.insert(0, str(ACCESS_ROOT / "tests"))

from test_access_contract import write_fixture  # noqa: E402
from tos_access import http_server  # noqa: E402
from tos_access.core import (  # noqa: E402
    ToSAccessCore,
    _LazyPartitionedProjection,
)
from tos_access.http_server import make_server  # noqa: E402
from tos_access.knowledge_compile import compile_knowledge_store  # noqa: E402
from tos_access.projection_store import (  # noqa: E402
    Collection,
    ProjectionReader,
    ProjectionStoreError,
    write_projection,
)
from tos_access.query_store import QueryStoreRequired  # noqa: E402


_COLLECTIONS = {
    "nodes": ("node_id", ("node_id",)),
    "edges": ("edge_id", ("edge_id",)),
    "clusters": ("cluster_id", ("cluster_id",)),
    "views": ("view_id", ("order", "view_id")),
}


def _sorted_fixture_philosophy(root: Path) -> dict:
    """Make the monolith use the same declared order as the carrier."""
    path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
    payload = json.loads(path.read_text(encoding="utf-8"))
    for name, (key_field, order_fields) in _COLLECTIONS.items():
        payload[name] = sorted(payload[name], key=lambda row: tuple(row.get(field, "") for field in order_fields))
    path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    return payload


def _partition_philosophy(root: Path, *, schema_version: str | None = None, nested: bool = False) -> dict:
    path = root / "ToS/derived-exports/philosophy_graph_projection.min.json"
    payload = json.loads(path.read_text(encoding="utf-8"))
    if schema_version is not None:
        payload["schema_version"] = schema_version
    header = {key: value for key, value in payload.items() if key not in _COLLECTIONS}
    collections = {
        name: Collection(payload[name], key_field, order_fields)
        for name, (key_field, order_fields) in _COLLECTIONS.items()
    }
    if nested:
        collections["graph/nodes"] = collections.pop("nodes")
    write_projection(path, header, collections, target_part_bytes=256)
    return payload


def _compile_store(root: Path) -> Path:
    output = root / "ToS/derived-exports/runtime/knowledge.sqlite3"
    compile_knowledge_store(root, output, allow_legacy=True, search_accelerator="scan")
    return output


def _health(core: ToSAccessCore, query_store_path: Path | None = None) -> tuple[int, dict]:
    env_value = str(query_store_path) if query_store_path is not None else ""
    with patch.dict(os.environ, {"TOS_QUERY_STORE_PATH": env_value}), patch.object(
        http_server, "web_root_for", return_value=core.tos_root
    ):
        server = make_server(core, port=0)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            try:
                with urllib.request.urlopen(
                    f"http://127.0.0.1:{server.server_port}/health"
                ) as response:
                    return response.status, json.load(response)
            except urllib.error.HTTPError as error:
                with error:
                    return error.code, json.load(error)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


def _selected_leaf(reader: ProjectionReader, collection: str, key: str) -> Path:
    digest = hashlib.sha256(key.encode("utf-8")).hexdigest()
    descriptor = reader.manifest["collections"][collection]["root"]
    prefix = ""
    while descriptor["kind"] == "index":
        children = reader._children(descriptor, prefix)
        digit = digest[len(prefix)]
        descriptor = children[digit]
        prefix += digit
    return reader._descriptor(descriptor, prefix)


class CorePartitionedPhilosophyTests(unittest.TestCase):
    def test_keys_and_selected_collection_access_remain_lazy(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            expected = _sorted_fixture_philosophy(root)
            _partition_philosophy(root)
            core = ToSAccessCore.discover(tos_root=root)

            with patch.object(
                ProjectionReader,
                "materialize",
                side_effect=AssertionError("whole projection materialized"),
            ):
                projection = core.philosophy_projection()
                self.assertIsInstance(projection, _LazyPartitionedProjection)
                self.assertEqual(projection._reader.parts_read, 0)
                with patch.object(
                    projection._reader,
                    "iter_items",
                    wraps=projection._reader.iter_items,
                ) as iter_items:
                    self.assertEqual(set(projection.keys()), set(expected))
                    self.assertEqual(iter_items.call_count, 0)
                    self.assertEqual(projection._reader.parts_read, 0)

                    nodes = projection["nodes"]
                    self.assertIs(type(nodes), list)
                    self.assertEqual(nodes, expected["nodes"])
                    self.assertEqual(
                        [call.args[0] for call in iter_items.call_args_list],
                        ["nodes"],
                    )

    def test_partitioned_views_and_health_match_ordered_monolith(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            base = Path(raw)
            monolith_root = base / "monolith"
            partitioned_root = base / "partitioned"
            write_fixture(monolith_root)
            write_fixture(partitioned_root)
            _sorted_fixture_philosophy(monolith_root)
            _sorted_fixture_philosophy(partitioned_root)
            _partition_philosophy(partitioned_root)

            monolith_core = ToSAccessCore.discover(tos_root=monolith_root)
            partitioned_core = ToSAccessCore.discover(tos_root=partitioned_root)
            monolith_store = _compile_store(monolith_root)
            partitioned_store = _compile_store(partitioned_root)

            self.assertEqual(
                monolith_core.philosophy_views(),
                partitioned_core.philosophy_views(),
            )
            self.assertEqual(
                monolith_core.philosophy_view("chronology", limit=2),
                partitioned_core.philosophy_view("chronology", limit=2),
            )
            monolith_health = _health(monolith_core, monolith_store)
            partitioned_health = _health(partitioned_core, partitioned_store)
            self.assertEqual(monolith_health[0], 200)
            self.assertTrue(monolith_health[1]["ok"])
            self.assertEqual(monolith_health, partitioned_health)

    def test_lazy_materialization_preserves_sequence_mapping_and_keyed_order(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "projection.min.json"
            header = {"schema_version": "tos_philosophy_graph_projection_v2"}
            collections = {
                "positionals": Collection(["second", "first"], []),
                "mapping": Collection(
                    [("z", {"value": 2}), ("a", {"value": 1})], None
                ),
                "keyed": Collection(
                    [
                        {"id": "b", "rank": 2},
                        {"id": "a", "rank": 1},
                    ],
                    "id",
                    ("rank", "id"),
                ),
            }
            write_projection(path, header, collections, target_part_bytes=256)
            reader = ProjectionReader(path)
            projection = _LazyPartitionedProjection(reader)
            expected = reader.materialize()

            self.assertIs(type(projection["positionals"]), list)
            self.assertIs(type(projection["mapping"]), dict)
            self.assertIs(type(projection["keyed"]), list)
            self.assertEqual(projection["positionals"], expected["positionals"])
            self.assertEqual(projection["mapping"], expected["mapping"])
            self.assertEqual(projection["keyed"], expected["keyed"])

    def test_incompatible_schema_and_nested_collection_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            incompatible_root = Path(raw) / "incompatible"
            nested_root = Path(raw) / "nested"
            write_fixture(incompatible_root)
            write_fixture(nested_root)
            _partition_philosophy(incompatible_root, schema_version="tos_unknown_v1")
            _partition_philosophy(nested_root, nested=True)

            with self.assertRaisesRegex(RuntimeError, "schema_version"):
                ToSAccessCore.discover(tos_root=incompatible_root).philosophy_projection()
            with self.assertRaisesRegex(ProjectionStoreError, "flat top-level"):
                ToSAccessCore.discover(tos_root=nested_root).philosophy_projection()

    def test_corrupt_selected_part_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            _sorted_fixture_philosophy(root)
            _partition_philosophy(root)
            projection = ToSAccessCore.discover(tos_root=root).philosophy_projection()
            selected = _selected_leaf(projection._reader, "nodes", "a")
            selected.write_bytes(selected.read_bytes() + b"corrupt")

            with self.assertRaisesRegex(ProjectionStoreError, "size mismatch"):
                projection["nodes"]

    def test_partitioned_root_requires_completed_query_store(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            write_fixture(root)
            _partition_philosophy(root)
            core = ToSAccessCore.discover(tos_root=root)

            with self.assertRaises(QueryStoreRequired):
                core.knowledge_header()
            with patch(
                "tos_access.core._knowledge_graph_version",
                side_effect=AssertionError("full graph fallback"),
            ):
                status, health = _health(core)
            self.assertEqual(status, 503)
            self.assertFalse(health["ok"])
            self.assertTrue(
                any("query store build required" in error for error in health["errors"])
            )


if __name__ == "__main__":
    unittest.main()

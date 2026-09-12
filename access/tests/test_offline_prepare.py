"""Real tiny source bootstrap, exclusive completion and explicit prepared reads."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from tos_access import core as source
from tos_access import prepare as producer
from tos_access.knowledge import execute_knowledge_lens, inspect_knowledge_node, search_knowledge_graph
from tos_access.portable_paths import normalize_paths
from tos_access.published_lens import PublishedLensService
from tos_access.published_read_model import PublishedKnowledgeReadModel
from tos_access.published_search import PublishedSearchService

REPO = Path(__file__).resolve().parents[2]


def write_source(root):
    """Only the five real core inputs, no edge builder or aggregate fixture."""
    values = {
        source.INDEX_RELATIVE_PATH: {
            "schema_version": "tos_corpus_index_v1", "nodes": [], "resources": [],
            "manifests": [], "branches": [], "relation_edges": [], "relation_packs": [],
            "graph_views": [], "source_navigation": {"nodes": [], "edges": [], "rights": []}},
        source.PHILOSOPHY_PROJECTION_RELATIVE_PATH: {
            "schema_version": "tos_philosophy_graph_projection_v2",
            "nodes": [{"node_id": identifier, "node_type": "concept", "label": label,
                       "source_ref": str(root / "ToS/synthetic.json"),
                       "properties": {"future": {"zero": 0, "false": False, "null": None}}}
                      for identifier, label in (("a", "Альфа Alpha"), ("b", "Beta"))],
            "edges": [{"edge_id": "r", "from_id": "a", "to_id": "b", "predicate_id": "related_to",
                       "source_ref": "ToS/synthetic.json"}],
            "layers": [], "views": [], "clusters": []},
        source.BIBLIOGRAPHIC_GRAPH_RELATIVE_PATH: {
            "schema_version": "tos_source_witness_bibliographic_graph_v1",
            "nodes": [], "edges": [], "claim_traces": []},
    }
    for relative in (source.ENTITY_TYPE_REGISTRY_RELATIVE_PATH, source.RELATION_TYPE_REGISTRY_RELATIVE_PATH):
        values[relative] = json.loads((REPO / relative).read_text(encoding="utf-8"))
    for relative, value in values.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value, ensure_ascii=False), encoding="utf-8")


class OfflinePrepareTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="offline-prepare-", dir=os.environ.get("TMPDIR"))
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "source"
        self.output = Path(self.tmp.name) / "output"
        write_source(self.root)

    def command(self, *extra, env=None):
        environment = {**os.environ, "PYTHONPATH": str(REPO / "access/src"),
                       "PYTHONDONTWRITEBYTECODE": "1", **(env or {})}
        return subprocess.run([sys.executable, "-m", "tos_access.prepare", "--source-root", str(self.root),
                               "--output-dir", str(self.output), *extra],
                              env=environment, capture_output=True, text=True, timeout=30)

    def test_real_command_fenced_environment_and_explicit_reads(self):
        bad = str(Path(self.tmp.name) / "do-not-select")
        names = ("TOS_ROOT", "TOS_CORPUS_INDEX_PATH", "TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH",
                 "TOS_BIBLIOGRAPHIC_GRAPH_PATH", "TOS_ENTITY_TYPE_REGISTRY_PATH",
                 "TOS_RELATION_TYPE_REGISTRY_PATH", "TOS_PHILOSOPHY_POST_PLANTING_AUDIT_PATH",
                 "TOS_EVIDENCE_PROJECTION_PATH", "TOS_SEARCH_READ_MODEL_PATH",
                 "TOS_SEARCH_READ_MODEL_MAX_BYTES", "AOA_TOS_ROOT")
        result = self.command(env={name: bad for name in names})
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(result.stdout)
        self.assertEqual(receipt, json.loads((self.output / "completed.json").read_text()))
        self.assertEqual(receipt["mode"], "full_bootstrap")
        self.assertEqual(receipt["normalization_cache"], "disabled")
        self.assertTrue(receipt["source_state_checked"])
        self.assertFalse(receipt["ongoing_currentness_granted"])
        self.assertFalse(receipt["consumer_switched"])
        self.assertEqual({p.name for p in self.output.iterdir()}, {"snapshot.sqlite", "binding.json", "completed.json"})
        self.assertEqual(self.output.stat().st_mode & 0o777, 0o700)
        for path in self.output.iterdir():
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        expected = source.ToSAccessCore.discover(self.root).knowledge_snapshot()
        graph = normalize_paths(expected["graph"], self.root)
        catalog = normalize_paths(expected["catalog"], self.root)
        self.assertEqual(receipt["source_revision"], graph["source_revision"])
        binding = json.loads((self.output / "binding.json").read_text())
        self.assertEqual(binding, receipt["binding"])
        reader = PublishedKnowledgeReadModel(self.output / "snapshot.sqlite", binding)
        with patch.object(source.ToSAccessCore, "knowledge_graph", side_effect=AssertionError("cold graph")):
            self.assertEqual(reader.catalog(), catalog)
            for node in graph["nodes"]:
                self.assertEqual(reader.node(node["id"]), inspect_knowledge_node(graph, node["id"]))
            spec = {"schema_version": "tos_lens_spec_v1", "lens_id": "bootstrap-fixture",
                    "sources": ["philosophy"], "explain": True}
            self.assertEqual(PublishedLensService(reader).execute(spec), execute_knowledge_lens(graph, spec))
            for query in ("", "Альфа", '"zero": 0', "notfound"):
                actual = PublishedSearchService(reader).search(query, limit=10)
                reference = search_knowledge_graph(graph, query, limit=10)
                self.assertEqual(actual["nodes"], reference["nodes"])
                self.assertEqual(actual["relations"], reference["relations"])

    def test_output_collision_refuses_without_altering_success(self):
        self.assertEqual(self.command().returncode, 0)
        before = {p.name: p.read_bytes() for p in self.output.iterdir()}
        result = self.command()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertEqual(json.loads(result.stderr)["status"], "failed")
        self.assertEqual(before, {p.name: p.read_bytes() for p in self.output.iterdir()})

    def test_missing_source_and_partial_publication_never_complete(self):
        (self.root / source.INDEX_RELATIVE_PATH).unlink()
        result = self.command()
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertNotIn(str(self.root), result.stderr)
        self.assertFalse((self.output / "completed.json").exists())
        self.assertFalse((self.output / "binding.json").exists())
        write_source(self.root)
        partial = self.output.with_name("partial-publication")
        def fail(path, **kwargs):
            path.write_text("unfinished")
            raise RuntimeError("injected failure")
        with patch.object(producer, "publish_prepared", side_effect=fail):
            with self.assertRaisesRegex(RuntimeError, "injected failure"):
                producer.prepare(self.root, partial)
        self.assertEqual((partial / "snapshot.sqlite").read_text(), "unfinished")
        self.assertFalse((partial / "binding.json").exists())
        self.assertFalse((partial / "completed.json").exists())

    def test_source_drift_after_publication_never_describes_success(self):
        original = producer.publish_prepared
        def publish_then_drift(path, **kwargs):
            binding = original(path, **kwargs)
            with (self.root / source.INDEX_RELATIVE_PATH).open("a") as stream:
                stream.write(" ")
            return binding
        with patch.object(producer, "publish_prepared", side_effect=publish_then_drift):
            with self.assertRaisesRegex(RuntimeError, "source changed"):
                producer.prepare(self.root, self.output)
        self.assertTrue((self.output / "snapshot.sqlite").exists())
        self.assertFalse((self.output / "binding.json").exists())
        self.assertFalse((self.output / "completed.json").exists())

    def test_receipt_failure_leaves_no_completion(self):
        original = producer._exclusive_json
        def fail_completion(path, value):
            if path.name == "completed.json":
                raise OSError("injected receipt failure")
            return original(path, value)
        with patch.object(producer, "_exclusive_json", side_effect=fail_completion):
            with self.assertRaisesRegex(OSError, "injected receipt failure"):
                producer.prepare(self.root, self.output)
        self.assertTrue((self.output / "binding.json").exists())
        self.assertFalse((self.output / "completed.json").exists())

    def test_actual_publication_limit_failure_and_last_marker_io_failure(self):
        with self.assertRaises(Exception):
            producer.prepare(self.root, self.output, limits=producer.PublicationLimits(max_bytes=4096))
        self.assertFalse((self.output / "snapshot.sqlite").exists())
        self.assertFalse((self.output / "binding.json").exists())
        self.assertFalse((self.output / "completed.json").exists())
        partial = self.output.with_name("marker-io-failure")
        original = producer._sync_directory
        def fail_final_sync(path):
            if (path / "completed.json").exists():
                raise OSError("injected final sync failure")
            return original(path)
        with patch.object(producer, "_sync_directory", side_effect=fail_final_sync):
            with self.assertRaisesRegex(OSError, "injected final sync failure"):
                producer.prepare(self.root, partial)
        self.assertTrue((partial / "binding.json").exists())
        self.assertFalse((partial / "completed.json").exists())

    def test_recursive_portable_paths_preserve_native_value_kinds(self):
        root = self.root.resolve()
        opaque = (str(root),)
        source_value = {str(root): [str(root), str(root / "ToS/a.json"), str(root) + "-neighbor/a",
                                  0, 1.0, False, None, {"future": str(root / "unknown")}, opaque]}
        actual = normalize_paths(source_value, root)
        self.assertEqual(actual[str(root)], ["Tree-of-Sophia", "ToS/a.json", str(root) + "-neighbor/a",
                                            0, 1.0, False, None, {"future": "unknown"}, opaque])
        self.assertIs(type(actual[str(root)][3]), int)
        self.assertIs(type(actual[str(root)][4]), float)
        self.assertIs(type(actual[str(root)][5]), bool)
        self.assertIs(actual[str(root)][-1], opaque)


if __name__ == "__main__":
    unittest.main()

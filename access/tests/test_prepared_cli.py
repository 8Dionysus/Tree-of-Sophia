"""Explicit executable reader selection must not rediscover a different backend."""
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from tos_access.cli import main
from tos_access.core import ToSAccessCore
from tos_access.prepared_publication import publish_prepared
from tos_access.published_read_model import PublishedSnapshotConflict
from test_prepared_publication import fixture


class PreparedCliTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.path = self.root / "publication.sqlite"
        graph, self.catalog = fixture()
        self.binding = publish_prepared(self.path, graph=graph, catalog=self.catalog)
        self.binding_path = self.root / "selected.json"
        self.binding_path.write_text(json.dumps(self.binding), encoding="utf-8")
        self.args = ["--root", str(self.root), "--prepared-read-model", str(self.path),
                     "--prepared-binding", str(self.binding_path)]

    def test_actual_catalog_uses_exact_file_without_source_graph(self):
        output = io.StringIO()
        with patch("tos_access.core.build_knowledge_graph", side_effect=AssertionError("hidden graph")), redirect_stdout(output):
            main([*self.args, "knowledge", "catalog"])
        self.assertEqual(json.loads(output.getvalue()), self.catalog)

    def test_stale_binding_is_not_reselected_from_database(self):
        changed = {**self.binding, "publication_epoch": self.binding["publication_epoch"] + 1}
        self.binding_path.write_text(json.dumps(changed), encoding="utf-8")
        with self.assertRaises(PublishedSnapshotConflict):
            main([*self.args, "knowledge", "catalog"])

    def test_incomplete_selection_and_wrong_profile_fail_before_discovery(self):
        with patch.object(ToSAccessCore, "discover", side_effect=AssertionError("unexpected discovery")):
            for args in (["--prepared-read-model", str(self.path), "knowledge", "catalog"],
                         ["--prepared-binding", str(self.binding_path), "mcp"],
                         ["--exploration-checkpoints", "unused.sqlite", "mcp"],
                         [*self.args, "doctor"], [*self.args, "verify"]):
                with self.subTest(args=args), self.assertRaises(SystemExit):
                    main(args)

    def test_binding_input_is_bounded_strict_json(self):
        for raw in (b" " * 65_537, b'{"schema":"x","schema":"y"}', b"\xff", b"NaN"):
            self.binding_path.write_bytes(raw)
            with self.subTest(raw=raw[:60]), patch.object(ToSAccessCore, "discover", side_effect=AssertionError("unexpected discovery")), self.assertRaises(SystemExit):
                main([*self.args, "knowledge", "catalog"])

    def test_mcp_and_http_receive_the_selected_core_and_checkpoint_policy(self):
        checkpoint = self.root / "continuations.sqlite"
        args = [*self.args, "--exploration-checkpoints", str(checkpoint)]
        with patch("tos_access.mcp_server.build_server", return_value=Mock()) as build, patch("tos_access.mcp_server._run_server") as run:
            main([*args, "mcp"])
            core = build.call_args.kwargs["core"]
            self.assertEqual(core.published_read_model_expected, self.binding)
            self.assertEqual(core.published_read_model_path, self.path)
            self.assertEqual(core.published_exploration_checkpoint_path, checkpoint)
            run.assert_called_once_with(build.return_value)
        with patch("tos_access.http_server.serve") as serve:
            main([*args, "serve", "--port", "8199"])
            core = serve.call_args.args[0]
            self.assertEqual(core.published_read_model_expected, self.binding)
            self.assertEqual(serve.call_args.kwargs, {"host": "127.0.0.1", "port": 8199})
        with patch("tos_access.mcp_server.build_server", return_value=Mock()) as build, patch("tos_access.mcp_server._run_server"):
            main(["--root", str(self.root), "mcp"])
            self.assertEqual(build.call_args.kwargs, {"tos_root": self.root})


if __name__ == "__main__":
    unittest.main()

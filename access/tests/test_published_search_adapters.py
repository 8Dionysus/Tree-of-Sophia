"""Actual selected compressed search through executable HTTP and native MCP."""
import asyncio
from contextlib import closing, redirect_stdout
from datetime import timedelta
import hashlib
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
import io
import json
from pathlib import Path
import sqlite3
import sys
import tempfile
from threading import Thread
import unittest
from unittest.mock import patch
from urllib.parse import urlencode

from tos_access.cli import main
from tos_access.core import ToSAccessCore
from tos_access.compressed_search_store import (
    SearchStaleBinding, SearchCursorError, SearchCursorExpired,
    SearchUnavailable, SearchBudgetExceeded,
)
from tos_access.http_server import build_handler
from tos_access.mcp_server import build_server
from tos_access.prepared_publication import publish_prepared
from tos_access.published_read_metadata import _compact
from test_prepared_publication import fixture


class PublishedSearchAdapterTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="published-search-adapters-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.path = self.root / "publication.sqlite"
        self.graph, self.catalog = fixture()
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        self.binding_path = self.root / "selected.json"
        self.binding_path.write_text(json.dumps(self.binding), encoding="utf-8")
        self.core = ToSAccessCore.discover(self.root, published_read_model_path=self.path,
                                          published_read_model_expected=self.binding)
        self.args = ["--root", str(self.root), "--prepared-read-model", str(self.path),
                     "--prepared-binding", str(self.binding_path)]

    def http(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), build_handler(self.core, self.root))
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        def close():
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
        self.addCleanup(close)
        return server

    def get(self, server, path):
        with closing(HTTPConnection("127.0.0.1", server.server_port, timeout=5)) as connection:
            connection.request("GET", path)
            response = connection.getresponse()
            raw = response.read()
            return response.status, json.loads(raw), raw

    def test_core_capabilities_and_explicit_mode_do_not_fall_back(self):
        default = ToSAccessCore.discover(self.root)
        with patch("tos_access.core.build_knowledge_graph", side_effect=AssertionError("cold graph")):
            capability = self.core.knowledge_search_capabilities()
            self.assertTrue(capability["modes"]["compressed"]["available"])
            self.assertFalse(capability["modes"]["legacy"]["available"])
            self.assertTrue(capability["explicit_mode_required"])
            self.assertFalse(default.knowledge_search_capabilities()["modes"]["compressed"]["available"])
            self.assertTrue(default.knowledge_search_capabilities()["modes"]["legacy"]["available"])
            with self.assertRaises(SearchUnavailable):
                default.knowledge_search_compressed("a")
            with self.assertRaisesRegex(RuntimeError, "does not materialize"):
                self.core.knowledge_search("a")
            page = self.core.knowledge_search_compressed("a", limit=1)
            self.assertEqual(page["nodes"], [self.graph["nodes"][0]])
            self.assertEqual(page["schema"], "tos_knowledge_search_compressed_v3")

    def test_cli_compact_packet_and_continuation_match_shared_core(self):
        with patch("tos_access.core.build_knowledge_graph", side_effect=AssertionError("cold graph")):
            output = io.StringIO()
            with redirect_stdout(output):
                main([*self.args, "knowledge", "search", "--mode", "compressed", "--limit", "1"])
            first = json.loads(output.getvalue())
            self.assertEqual(output.getvalue(), _compact(first) + "\n")
            output = io.StringIO()
            with redirect_stdout(output):
                main([*self.args, "knowledge", "search", "--mode", "compressed", "--limit", "1",
                      "--cursor", first["page"]["next_cursor"]])
            following = json.loads(output.getvalue())
            self.assertEqual(first["nodes"] + following["nodes"], self.graph["nodes"][:2])
            with self.assertRaises(SystemExit):
                main([*self.args, "knowledge", "search", "--mode", "compressed", "--offset", "1"])
            output = io.StringIO()
            with redirect_stdout(output):
                main([*self.args, "knowledge", "search-capabilities"])
            self.assertTrue(json.loads(output.getvalue())["modes"]["compressed"]["available"])

    def test_loopback_http_compact_search_caps_and_error_mapping(self):
        server = self.http()
        with patch("tos_access.core.build_knowledge_graph", side_effect=AssertionError("cold graph")):
            status, caps, _ = self.get(server, "/api/knowledge/search/capabilities")
            self.assertEqual(status, 200)
            self.assertTrue(caps["modes"]["compressed"]["available"])
            status, first, raw = self.get(server, "/api/knowledge/search?mode=compressed&limit=1")
            self.assertEqual(status, 200)
            self.assertEqual(raw, _compact(first).encode("utf-8"))
            params = urlencode({"mode": "compressed", "limit": 1, "cursor": first["page"]["next_cursor"]})
            status, following, _ = self.get(server, "/api/knowledge/search?" + params)
            self.assertEqual(status, 200)
            self.assertEqual(first["nodes"] + following["nodes"], self.graph["nodes"][:2])
            for suffix in ("&offset=1", "&cursor=bad", "&query=" + "x" * 257):
                self.assertEqual(self.get(server, "/api/knowledge/search?mode=compressed" + suffix)[0], 400)
            for error, expected in ((SearchCursorError, 400), (SearchStaleBinding, 409),
                                    (SearchCursorExpired, 410), (SearchBudgetExceeded, 413), (SearchUnavailable, 503)):
                with self.subTest(error=error), patch.object(ToSAccessCore, "knowledge_search_compressed", side_effect=error("fixture refusal")):
                    self.assertEqual(self.get(server, "/api/knowledge/search?mode=compressed")[0], expected)
            with closing(sqlite3.connect(self.path)) as db:
                db.execute("UPDATE knowledge_exploration_clock SET epoch=epoch+1")
                db.commit()
            self.assertEqual(self.get(server, "/api/knowledge/search?mode=compressed")[0], 409)

    def test_native_mcp_dispatch_keeps_compact_and_structured_packets(self):
        from mcp.types import CallToolResult
        with patch("tos_access.core.build_knowledge_graph", side_effect=AssertionError("cold graph")):
            server = build_server(core=self.core)
            result = asyncio.run(server.call_tool("tos_knowledge_search", {"mode": "compressed", "limit": 1}))
            self.assertIsInstance(result, CallToolResult)
            packet = result.structuredContent
            self.assertEqual(result.content[0].text, _compact(packet))
            self.assertEqual(packet["nodes"], self.graph["nodes"][:1])
            caps = asyncio.run(server.call_tool("tos_knowledge_search_capabilities", {}))[1]
            self.assertTrue(caps["modes"]["compressed"]["available"])

    def test_stdio_cli_search_continues_after_real_server_restart(self):
        from mcp import ClientSession, StdioServerParameters
        from mcp.client.stdio import stdio_client
        first = self.core.knowledge_search_compressed(limit=1)
        request = {"mode": "compressed", "cursor": first["page"]["next_cursor"], "limit": 1}
        expected = self.core.knowledge_search_compressed(cursor=request["cursor"], limit=1)
        before = hashlib.sha256(self.path.read_bytes()).hexdigest()
        parameters = StdioServerParameters(command=sys.executable,
            args=["-B", "-m", "tos_access", *self.args, "mcp"],
            env={"PYTHONPATH": str(Path(__file__).resolve().parents[1] / "src"),
                 "PYTHONDONTWRITEBYTECODE": "1", "TOS_MCP_TRANSPORT": "stdio"})
        async def query():
            async with stdio_client(parameters) as (incoming, outgoing):
                async with ClientSession(incoming, outgoing, read_timeout_seconds=timedelta(seconds=10)) as session:
                    await session.initialize()
                    self.assertIn("tos_knowledge_search_capabilities", {tool.name for tool in (await session.list_tools()).tools})
                    result = await session.call_tool("tos_knowledge_search", request)
                    self.assertFalse(result.isError)
                    self.assertEqual(result.structuredContent, expected)
                    self.assertEqual(result.content[0].text, _compact(expected))
        for _ in range(2):
            asyncio.run(query())
        self.assertEqual(hashlib.sha256(self.path.read_bytes()).hexdigest(), before)


if __name__ == "__main__":
    unittest.main()

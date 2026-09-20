from __future__ import annotations

import asyncio
import importlib.util
import io
import json
import sys
import tempfile
import threading
import unittest
import urllib.request
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "access/src"))
from tos_access.cli import main
from tos_access.core import READING_PROVIDER_RELATIVE_PATH, ToSAccessCore
from tos_access.http_server import make_server
from tos_access.mcp_server import build_server


def provider(software_root: Path, body: str) -> Path:
    path = software_root / READING_PROVIDER_RELATIVE_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    return path


def provider_resolver(software_root: Path | None):
    original = __import__("tos_access.core", fromlist=["program_path"]).program_path

    def resolve(relative: str | Path) -> Path:
        if Path(relative) == READING_PROVIDER_RELATIVE_PATH:
            return (software_root or Path("/nonexistent/tos-reading-provider")) / relative
        return original(relative)

    return patch("tos_access.core.program_path", side_effect=resolve)


VALID_PROVIDER = """
def build_result(query, language, **options):
    return {
        'schema_version': 'tos_zarathustra_reading_search_result_v1',
        'coverage': {'returned_source_results': 0, 'total_source_results': 2,
                     'whole_book_semantic_recall_asserted': False},
        'results': [], 'groups': {'scope':'all_matching_source_occurrences_before_limit'},
        'fixture_observed_input': [query, language, options['limit'], list(options['group_by'])],
        'fixture_source_root': str(options['source_root']),
        'fixture_analysis_root': str(options['analysis_root']),
    }
"""


class ReadingAccessTests(unittest.TestCase):
    def test_missing_provider_returns_unavailable_and_public_never_loads_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with provider_resolver(None):
                core = ToSAccessCore.discover(tos_root=root)
                result = core.zarathustra_reading_search("fate", "en")
                self.assertFalse(result["available"])
                self.assertIsNone(result["task"])
                self.assertIsNone(result["result"])
                self.assertEqual(result["publication_posture"], "excluded_from_public_bundle")
                software = root / "software"
                provider(software, "raise AssertionError('must not load private provider')")
                with provider_resolver(software):
                    self.assertFalse(core.zarathustra_reading_public_capability()["available"])

    def test_private_missing_and_drift_are_different_states(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            core = ToSAccessCore.discover(tos_root=root)
            software = root / "software"
            with provider_resolver(software):
                provider(software, "class ReadingUnavailable(RuntimeError): pass\ndef build_result(*args, **kwargs): raise ReadingUnavailable('private material is absent')")
                self.assertFalse(core.zarathustra_reading_search("fate", "en")["available"])
                provider(software, "def build_result(*args, **kwargs): raise RuntimeError('source fixity mismatch')")
                with self.assertRaisesRegex(RuntimeError, "fixity mismatch"):
                    core.zarathustra_reading_search("fate", "en")

    def test_zero_limit_empty_groups_and_selected_root_reach_provider(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            software = root / "software"
            with provider_resolver(software):
                provider(software, VALID_PROVIDER)
                core = ToSAccessCore.discover(tos_root=root)
                envelope = core.zarathustra_reading_search(" судьбы ", "RU", 0, False, [])
                self.assertTrue(envelope["available"])
                self.assertEqual(envelope["result"]["fixture_observed_input"], ["судьбы", "ru", 0, []])
                self.assertEqual(envelope["result"]["fixture_source_root"], str(root))
                self.assertEqual(envelope["result"]["fixture_analysis_root"], str(root))
                self.assertFalse(envelope["authority"]["is_semantic_truth"])

    def test_data_root_provider_sentinel_is_never_executed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data_sentinel = root / READING_PROVIDER_RELATIVE_PATH
            data_sentinel.parent.mkdir(parents=True)
            data_sentinel.write_text("raise AssertionError('data-root provider executed')", encoding="utf-8")
            software = root / "software"
            with provider_resolver(software):
                provider(software, VALID_PROVIDER)
                result = ToSAccessCore.discover(tos_root=root).zarathustra_reading_search("fate", "en", 0)
            self.assertTrue(result["available"])
            self.assertEqual(result["result"]["fixture_source_root"], str(root))

    def test_semantic_acceptance_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            software = root / "software"
            with provider_resolver(software):
                provider(software, VALID_PROVIDER.replace("'whole_book_semantic_recall_asserted': False", "'whole_book_semantic_recall_asserted': True"))
                with self.assertRaisesRegex(RuntimeError, "authority boundary"):
                    ToSAccessCore.discover(tos_root=root).zarathustra_reading_search("fate", "en")

    def test_http_and_cli_are_adapters_over_same_core(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            software = root / "software"
            with provider_resolver(software):
                provider(software, VALID_PROVIDER)
                core = ToSAccessCore.discover(tos_root=root)
                with patch("tos_access.http_server.web_root_for", return_value=ROOT / "access/web/dist"):
                    server = make_server(core, port=0)
                thread = threading.Thread(target=server.serve_forever, daemon=True)
                thread.start()
                try:
                    url = f"http://127.0.0.1:{server.server_port}/api/zarathustra/reading?query=fate&language=en&limit=0&group_by=none"
                    with urllib.request.urlopen(url) as response:
                        result = json.load(response)
                    self.assertEqual(result, core.zarathustra_reading_search("fate", "en", 0, group_by=[]))
                    output = io.StringIO()
                    with redirect_stdout(output):
                        main(["--root", str(root), "reading-search", "--query", "fate", "--language", "en", "--limit", "0", "--group-by", ""])
                    self.assertEqual(json.loads(output.getvalue()), result)
                finally:
                    server.shutdown()
                    thread.join(timeout=2)
                    server.server_close()

    @unittest.skipUnless(importlib.util.find_spec("mcp"), "mcp dependency not installed")
    def test_native_mcp_operation_calls_same_core(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            software = root / "software"
            with provider_resolver(software):
                provider(software, VALID_PROVIDER)
                server = build_server(tos_root=root)
                tool = server._tool_manager.get_tool("tos_zarathustra_reading_search")
                self.assertIsNotNone(tool)
                result = asyncio.run(tool.run({"query": "fate", "language": "en", "limit": 0}))
                self.assertEqual(result, ToSAccessCore.discover(tos_root=root).zarathustra_reading_search("fate", "en", 0))


if __name__ == "__main__":
    unittest.main()

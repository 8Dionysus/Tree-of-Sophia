"""Behavior at the boundary between installed software and selected data."""
from __future__ import annotations

import asyncio
import json
import os
from pathlib import Path
import sys
import signal
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch

ACCESS_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ACCESS_ROOT.parent
sys.path.insert(0, str(ACCESS_ROOT / "src"))

from tos_access.core import ToSAccessCore
from tos_access.locations import data_root


class SoftwareBoundaryTests(unittest.TestCase):
    def test_admitted_snapshot_passes_stage_root_and_fd_without_ambient_data_selectors(self):
        from contextlib import contextmanager
        from tos_access.native_core_snapshot import (
            NativeCoreSnapshotClient, NativeCoreSnapshotSelection,
            NativeCoreSnapshotAdmission, NativeCoreJsonLimits, NativeCoreColdLimits,
            NativeCoreProcessLimits,
        )
        # This checks the real SDK-to-child handoff. Native sealed-ticket
        # verification remains with the Rust issuer/receiver, not this stub.
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryFile() as ticket:
            selection = NativeCoreSnapshotSelection.discover(directory)
            admission = NativeCoreSnapshotAdmission(
                absolute_deadline=time.monotonic() + 20, operation_seconds=20,
                stage_ticket_fd=ticket.fileno(), max_build_seconds=10,
                tmpfs_quota_bytes=1048576, inode_limit=64, working_ram_bytes=16777216,
                whole_max_rows=16, whole_max_row_bytes=4096,
                whole_max_graph_bytes=4096, whole_max_catalog_bytes=4096,
                whole_max_catalog_inputs_bytes=4096, whole_max_state_bytes=4096,
                json=NativeCoreJsonLimits(4096, 32, 1024, 20),
                cold=NativeCoreColdLimits(4096, 100, 64, 16, 4096, 4096, 4096, 16),
                process=NativeCoreProcessLimits(16777216, 4096),
            )
            observed = []
            class Channel:
                def write_input(self, payload, *, close):
                    request = json.loads(payload)
                    self_outer.assertEqual(request['admission']['stage_ticket_fd'], ticket.fileno())
                    self_outer.assertTrue(close)
                def frames(self):
                    yield json.dumps({'schema_version': 'tos_native_core_snapshot_result_v1',
                                      'ok': True, 'result': {'handoff': 'checked'}}).encode()
            self_outer = self
            @contextmanager
            def child(arguments, **options):
                observed.append(arguments)
                self.assertEqual(options['env']['TOS_STAGE_ROOT'], directory)
                self.assertEqual(options['pass_fds'], (ticket.fileno(),))
                self.assertNotIn('TOS_STAGE_TICKET_FD', options['env'])
                self.assertNotIn('TOS_DATA_ROOT', options['env'])
                self.assertNotIn('TOS_QUERY_STORE_PATH', options['env'])
                yield Channel()
            client = NativeCoreSnapshotClient('/owned/software', selection, lambda _: admission)
            with patch.dict(os.environ, {'TOS_STAGE_ROOT': directory,
                    'TOS_STAGE_TICKET_FD': str(ticket.fileno()),
                    'TOS_DATA_ROOT': '/unselected/data', 'TOS_QUERY_STORE_PATH': '/unselected/store'}), \
                    patch('tos_access.native_core_snapshot.owned_exchange', child):
                with client._operation('tos_corpus_header') as call:
                    result, _, _ = client._exchange('tos_corpus_header', {}, call)
            self.assertEqual(result, {'handoff': 'checked'})
            self.assertEqual(len(observed), 1)
            client.close()

    def test_native_dispatch_accepts_profiled_access_receipt_and_keeps_legacy_shape(self):
        from tos_access.native_dispatch import _proof
        source = 'b' * 40
        legacy = {
            'schema_version': 'tos_native_access_build_v1', 'sha256': 'a' * 64,
            'size_bytes': 64, 'target': 'x86_64-unknown-linux-gnu',
            'source_commit': source, 'source_tree': 'c' * 40,
            'lock_sha256': 'd' * 64, 'toolchain': '1.98.1', 'profile': 'debug',
        }
        _proof(legacy, source)
        profiled = {**legacy, 'schema_version': 'tos_native_access_build_v2',
                    'opt_level': '1'}
        _proof(profiled, source)
        with self.assertRaisesRegex(ValueError, 'receipt differs'):
            _proof({**profiled, 'opt_level': '2'}, source)

    def test_native_core_exploration_contracts_preserves_zero_argument_packet_contract(self):
        from tos_access.native_core import NativeCore
        core = object.__new__(NativeCore)
        calls = []
        def packet(tool, request, **options):
            calls.append((tool, request, options))
            return {"test_only": "mapping sentinel"}
        core._packet = packet

        self.assertEqual(core.knowledge_exploration_contracts(),
                         {"test_only": "mapping sentinel"})
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][0], "tos_knowledge_exploration_contracts")
        self.assertEqual(calls[0][1], {})
        self.assertEqual(calls[0][2], {"source_errors": False})

    def test_native_access_factory_keeps_generic_and_source_selections_independent(self):
        from tos_access.native_access_core import NativeAccessCore
        core = NativeAccessCore.discover('/owned/source', native_prefix='/owned/software',
            release_root='/owned/release', reading_analysis_root='/owned/reading',
            reading_max_file_bytes=100, reading_max_total_file_bytes=200)
        self.assertEqual(core._server.arguments, ('--release-root', '/owned/release'))
        self.assertEqual(core._word_core._server.arguments, ('--root', '/owned/source'))
        self.assertEqual(core._reading_core._server.arguments,
            ('--root', '/owned/source', '--reading-analysis-root', '/owned/reading',
             '--reading-max-file-bytes', '100', '--reading-max-total-file-bytes', '200'))
        prepared = NativeAccessCore.discover('/owned/source', native_prefix='/owned/software',
            published_read_model_path='/owned/model', published_read_model_binding_path='/owned/binding',
            published_exploration_checkpoint_path='/owned/checkpoints', source_inputs_path='/owned/inputs')
        self.assertEqual(prepared._server.arguments,
            ('--prepared-read-model', '/owned/model', '--prepared-binding', '/owned/binding',
             '--root', '/owned/source', '--exploration-checkpoints', '/owned/checkpoints',
             '--source-inputs', '/owned/inputs'))
        with patch.dict(os.environ, {'TOS_NATIVE_PREFIX': '', 'PATH': ''}), self.assertRaisesRegex(ValueError, 'native_prefix'):
            NativeAccessCore.discover('/owned/source')
        with self.assertRaisesRegex(ValueError, 'model and owner-selected binding'):
            NativeAccessCore('/owned/software', published_read_model_path='/owned/model')
        with self.assertRaises(TypeError):
            NativeAccessCore.discover('/owned/source', native_prefix='/owned/software', source_provider=object())
        with self.assertRaisesRegex(TypeError, 'embedded source owner requires a native source provider'):
            NativeAccessCore.discover('/owned/source', native_prefix='/owned/software', source_read_service=object())
        with self.assertRaisesRegex(ValueError, 'select source_provider or source_read_service'):
            NativeAccessCore('/owned/software', source_provider=object(), source_read_service=object())
        with self.assertRaisesRegex(TypeError, 'embedded source owner requires a native source provider'):
            ToSAccessCore.discover('/owned/source', native_prefix='/owned/software',
                published_read_model_path='/owned/model', source_read_service=object())

    def test_native_access_ephemeral_checkpoint_lifetime_is_per_core(self):
        from tos_access.native_access_core import NativeAccessCore
        from tos_access.native_core import NativeCore
        with tempfile.TemporaryDirectory(prefix='native-core-source-check-', dir=REPO_ROOT) as selected:
            root = Path(selected)
            core = NativeAccessCore('/owned/software', release_root='/owned/release', native_state_root=root)
            other = NativeAccessCore('/owned/software', release_root='/owned/release', native_state_root=root)
            sentinel = {'page': {'next_cursor': 'native-token'}}
            with patch.object(NativeAccessCore, '_packet', return_value=sentinel):
                self.assertIs(core.knowledge_explore({'seed': 'owned'}), sentinel)
                first = core._server.arguments
                state = Path(first[-1]).parent
                self.assertEqual(state.stat().st_mode & 0o777, 0o700)
                self.assertIs(core.knowledge_explore({'cursor': 'native-token'}), sentinel)
                self.assertEqual(core._server.arguments, first)
                other.knowledge_explore({'seed': 'owned'})
                self.assertNotEqual(other._server.arguments[-1], first[-1])
            core.close()
            core.close()
            self.assertFalse(state.exists())
            with self.assertRaisesRegex(RuntimeError, 'Core is closed'):
                core.knowledge_catalog()
            with self.assertRaisesRegex(RuntimeError, 'Core is closed'):
                core.zarathustra_reading_search('owned')
            other.close()
            explicit = root / 'caller-owned.sqlite'
            explicit.write_text('caller-owned sentinel')
            configured = NativeAccessCore('/owned/software', release_root='/owned/release',
                                          published_exploration_checkpoint_path=explicit)
            configured.close()
            self.assertEqual(explicit.read_text(), 'caller-owned sentinel')

    def test_native_access_deadline_includes_lifetime_and_setup(self):
        from tos_access.native_access_core import NativeAccessCore
        from tos_access.native_core import NativeCore
        from threading import Event, Thread
        import time
        core = NativeAccessCore('/owned/software')
        acquired, release = Event(), Event()
        def hold():
            with core._lifetime_lock:
                acquired.set()
                release.wait(1)
        thread = Thread(target=hold)
        thread.start()
        try:
            self.assertTrue(acquired.wait(1))
            with patch.object(NativeCore, '_native_result') as child:
                with self.assertRaisesRegex(TimeoutError, 'waiting for its lifetime'):
                    core._native_result('call', (), absolute_deadline=time.monotonic() + 0.01)
                child.assert_not_called()
        finally:
            release.set()
            thread.join()
        with patch.object(NativeCore, '_native_result', return_value='packet') as child:
            deadline = time.monotonic() + 1
            self.assertEqual(core._native_result('call', (), absolute_deadline=deadline), 'packet')
            self.assertEqual(child.call_args.kwargs['absolute_deadline'], deadline)
        import tos_access
        from tos_access import ToSAccessCore
        from tos_access.source_read_errors import SourceReadError
        from tos_access.source_read_errors import SourceReadError as platform_error
        from tos_access.native_mcp import NativeMCPServer
        self.assertEqual(ToSAccessCore.__name__, 'NativeToSAccessCore')
        self.assertFalse(hasattr(tos_access, 'ReferenceToSAccessCore'))
        self.assertIs(SourceReadError, platform_error)
        self.assertEqual(NativeMCPServer.__module__, 'tos_access.native_mcp')
        core.close()

    def test_native_mcp_sdk_joins_the_shared_exchange_owner(self):
        from contextlib import contextmanager
        from queue import Queue, Empty
        from types import SimpleNamespace
        from tos_access.native_mcp import NativeMCPServer
        from tos_access.native_io import _bounded_json, _contract, MAX_NATIVE_MCP_FRAME_BYTES
        calls, closed, respond = [], [], [True]
        @contextmanager
        def exchange(arguments, **options):
            calls.append((arguments, options))
            queue = Queue()
            input_closed = []
            class Channel:
                def send(self, value):
                    if value.get('method') == 'initialize':
                        queue.put({'jsonrpc': '2.0', 'id': value['id'], 'result': {
                            'protocolVersion': value['params']['protocolVersion'],
                            'capabilities': {'tools': {}},
                            'serverInfo': {'name': 'source-only-sdk-check', 'version': '1'}}})
                    elif value.get('method') == 'tools/list' and respond[0]:
                        queue.put({'jsonrpc': '2.0', 'id': value['id'], 'result': {'tools': []}})
                def close_input(self):
                    input_closed.append(True)
                def frames(self):
                    while not options['cancelled'].is_set() and not input_closed:
                        try:
                            yield json.dumps(queue.get(timeout=0.01)).encode()
                        except Empty:
                            pass
                    if options['cancelled'].is_set():
                        raise InterruptedError('owned exchange cancelled')
            try:
                yield Channel()
            finally:
                closed.append(True)
        with patch.dict(sys.modules, {'tos_access.native_io': SimpleNamespace(owned_exchange=exchange, _bounded_json=_bounded_json, MAX_NATIVE_MCP_FRAME_BYTES=MAX_NATIVE_MCP_FRAME_BYTES)}):
            server = NativeMCPServer('/owned/software', inherit_data_selection=False)
            self.assertEqual(asyncio.run(server.list_tools()), [])
            with self.assertRaisesRegex(ValueError, 'byte budget'):
                asyncio.run(server._native_api('call', ('owned', {'query': 'x' * 65536})))
            self.assertEqual(len(calls), 1)
            respond[0] = False
            async def cancel_call():
                task = asyncio.create_task(server.list_tools())
                await asyncio.sleep(0.02)
                task.cancel()
                with self.assertRaises(asyncio.CancelledError):
                    await asyncio.wait_for(task, 1)
            asyncio.run(cancel_call())
        self.assertEqual(closed, [True, True])
        self.assertEqual(calls[0][0], ['mcp'])
        self.assertEqual(calls[0][1]['frame_cap'], MAX_NATIVE_MCP_FRAME_BYTES)
        _contract(calls[0][0], calls[0][1]['input_cap'], calls[0][1]['frame_cap'], (0,))
        with self.assertRaisesRegex(ValueError, 'host frame contract'):
            _contract(['mcp'], 65536, MAX_NATIVE_MCP_FRAME_BYTES + 1, (0,))
        self.assertNotIn('TOS_DATA_ROOT', calls[0][1]['env'])
        self.assertNotIn('TOS_RELEASE_ROOT', calls[0][1]['env'])
        self.assertTrue(calls[0][1]['cancelled'].is_set())

    def test_native_core_forwarding_preserves_raw_rule_inputs_and_packet_identity(self):
        from tos_access.native_core import NativeCore
        core = object.__new__(NativeCore)
        calls = []
        sentinel = {'full': {'opaque': 'packet'}, 'source_ref': 'owned'}
        def packet(tool, request, **options):
            calls.append((tool, request, options))
            return sentinel
        core._packet = packet
        request = {'left': {'opaque': 'left'}, 'right': {'opaque': 'right'}}
        self.assertIs(core.knowledge_temporal_compare(request), sentinel)
        self.assertIs(calls[-1][1]['request'], request)
        self.assertEqual(calls[-1][0], 'tos_knowledge_temporal_compare')
        self.assertIs(core.knowledge_search_indexed('  Я  ', kind_ids=['owned'], limit=103), sentinel)
        self.assertEqual(calls[-1][1], {'query': '  Я  ', 'sources': None, 'kind_ids': ['owned'],
            'predicate_ids': None, 'cursor': None, 'limit': 103, 'mode': 'indexed'})
        self.assertIs(core.philosophy_path_between('owned:left', 'owned:right'), sentinel)
        self.assertEqual(calls[-1][1]['excluded_edge_ids'], [])
        rows = [{'native_id': 'owned', 'source_refs': ['original:owned']}]
        core._packet = lambda tool, request, **options: {
            'rows': rows, 'next_offset': None, 'row_count': 1, 'total_row_count': 1}
        self.assertIs(core.philosophy_scale_rows('nodes'), rows)
        core._packet = lambda tool, request, **options: {
            'rows': rows, 'next_offset': 1, 'row_count': 1, 'total_row_count': 2}
        with self.assertRaisesRegex(ValueError, 'complete table'):
            core.philosophy_scale_rows('nodes')

    def test_native_core_reading_method_preserves_reference_arguments(self):
        # Mapping only; genuine source-bound packet parity needs the native child.
        from tos_access.native_core import NativeCore
        core = object.__new__(NativeCore)
        calls = []
        def packet(tool, request, **options):
            calls.append((tool, request, options))
            return {"test_only": "mapping sentinel"}
        core._packet = packet
        self.assertEqual(core.zarathustra_reading_search(
            123, "\x1cRU\x1f", " +١_٢ ", "false", ["formula", "speaker", "formula"]),
            {"test_only": "mapping sentinel"})
        self.assertEqual(calls[0][0], "tos_zarathustra_reading_search")
        self.assertEqual(calls[0][1], {"query": "123", "language": "ru", "limit": 12,
                                     "include_semantic_neighbors": True,
                                     "group_by": ["formula", "speaker"]})
        self.assertIs(calls[0][2]["source_errors"], False)
        self.assertGreater(calls[0][2]["absolute_deadline"], time.monotonic())
        core.zarathustra_reading_search("q", limit="1.5")
        self.assertEqual(calls[-1][1]["limit"], 20)
        self.assertEqual(calls[-1][1]["group_by"], ["speaker", "formula"])
        core.zarathustra_reading_search("q", limit=-1, group_by=[])
        self.assertEqual(calls[-1][1]["limit"], 0)
        self.assertEqual(calls[-1][1]["group_by"], [])
        core.zarathustra_reading_search("q", limit=10**100)
        self.assertEqual(calls[-1][1]["limit"], 100)
        for arguments in [(" ",), ("q" * 257,), ("q", "xx")]:
            with self.assertRaises(ValueError):
                core.zarathustra_reading_search(*arguments)
        for groups in [("speaker",), ["unknown"], [None]]:
            with self.assertRaises(ValueError):
                core.zarathustra_reading_search("q", group_by=groups)
        self.assertEqual(len(calls), 4)

    def test_native_core_word_method_preserves_reference_argument_contract(self):
        from tos_access.native_core import NativeCore
        from tos_access.native_io import _bounded_json
        from mcp import types
        wire_query = "Я\ud800\ud83d\ude00"
        request = types.JSONRPCMessage(root=types.JSONRPCRequest(
            jsonrpc="2.0", id=1, method="tools/call",
            params={"name": "tos_zarathustra_prepare_word_analysis",
                    "arguments": {"query": wire_query}}))
        raw = _bounded_json(
            request.model_dump(mode="json", by_alias=True, exclude_none=True),
            65_536, time.monotonic() + 5,
        )
        self.assertIn("Я".encode("utf-8"), raw)
        self.assertIn(b"\\ud800", raw)
        self.assertIn(b"\\ud83d\\ude00", raw)
        decoded = json.loads(raw)["params"]["arguments"]["query"]
        self.assertEqual(decoded.encode("utf-16-le", "surrogatepass"),
                         wire_query.encode("utf-16-le", "surrogatepass"))
        calls = []
        core = object.__new__(NativeCore)
        def packet(tool, request, **options):
            calls.append((tool, request, options))
            return {"test_only": "mapping sentinel"}
        core._packet = packet
        result = core.zarathustra_word_analysis_task(123, "\x1cRU\x1f", " +١_٢ ", "false")
        self.assertEqual(result, {"test_only": "mapping sentinel"})
        self.assertEqual(calls[0][0], "tos_zarathustra_prepare_word_analysis")
        self.assertEqual(calls[0][1], {"query": "123", "language": "ru", "rank": 12,
                                     "include_semantic_neighbors": True})
        self.assertIs(calls[0][2]["source_errors"], False)
        self.assertTrue(calls[0][2]["absolute_deadline"] > time.monotonic())
        core.zarathustra_word_analysis_task(" q ", rank="1.5")
        self.assertEqual(calls[-1][1]["rank"], 1)
        core.zarathustra_word_analysis_task("q", rank=10**100)
        self.assertEqual(calls[-1][1]["rank"], 100)
        for query, language in [(" ", "ru"), ("q" * 257, "ru"), ("q", "xx")]:
            with self.assertRaises(ValueError):
                core.zarathustra_word_analysis_task(query, language)
        self.assertEqual(len(calls), 3)

    def test_native_core_source_method_mapping_and_error_boundaries(self):
        # Imported caller control only; the SDK result stub is not native parity.
        from types import SimpleNamespace
        from threading import get_ident, enumerate as threads
        from tos_access.native_core import NativeCore
        from tos_access.native_mcp import NativeMCPServer
        from tos_access.source_read_errors import SourceReadError

        calls = []
        caller = get_ident()
        packet = {"schema_version": "complete-stub-packet", "authority": {"is_source": False}}

        async def selected(server, operation, arguments, *, absolute_deadline=None):
            calls.append((operation, arguments, absolute_deadline, get_ident()))
            return SimpleNamespace(isError=False, structuredContent=packet, content=[])

        core = NativeCore('/explicit/installed', ['--root', '/explicit/data'])
        target = {"target": {"opaque": "owned-selector"}}
        handle = {"handle": {"opaque": "owned-handle"}, "representation": "record"}
        before = {thread.ident for thread in threads()}
        with patch.object(NativeMCPServer, '_native_api', selected), patch('tos_access.native_core.time', SimpleNamespace(monotonic=lambda: 11)):
            self.assertIs(core.source_read_capabilities(), packet)
            self.assertIs(core.source_read_contract(), packet)
            self.assertIs(core.source_handle_discover(target), packet)
            self.assertIs(core.source_read(handle), packet)
            async def imported_async_caller():
                return core.source_read_capabilities()
            self.assertIs(asyncio.run(imported_async_caller()), packet)
        self.assertEqual([call[:3] for call in calls], [
            ('call', ('tos_source_read_capabilities', {}), 61),
            ('call', ('tos_source_read_contract', {}), 61),
            ('call', ('tos_source_handle_discover', target), 61),
            ('call', ('tos_source_read', handle), 61),
            ('call', ('tos_source_read_capabilities', {}), 61),
        ])
        self.assertTrue(all(call[3] != caller for call in calls))
        self.assertEqual({thread.ident for thread in threads()}, before)

        clock = {'now': 11}
        async def late_success(server, operation, arguments, *, absolute_deadline=None):
            clock['now'] = 62
            return SimpleNamespace(isError=False, structuredContent=packet, content=[])
        with patch.object(NativeMCPServer, '_native_api', late_success), patch('tos_access.native_core.time', SimpleNamespace(monotonic=lambda: clock['now'])):
            with self.assertRaisesRegex(TimeoutError, 'expired before returning the packet'):
                core.source_read_capabilities()
        self.assertEqual({thread.ident for thread in threads()}, before)

        async def refused(server, operation, arguments, *, absolute_deadline=None):
            return SimpleNamespace(isError=True, structuredContent=None,
                                   content=[SimpleNamespace(text=diagnostic)])

        with patch.object(NativeMCPServer, '_native_api', refused):
            diagnostic = 'exact source reader unavailable: no selected owner'
            with self.assertRaisesRegex(SourceReadError, '^source-owner-reader-not-configured$'):
                core.source_handle_discover(target)
            diagnostic = 'stale_selection: exact source reader unavailable: no selected owner'
            with self.assertRaisesRegex(SourceReadError, '^stale_selection:') as caught:
                core.source_read(handle)
            self.assertEqual(str(caught.exception), diagnostic)

        transport_error = OSError('held native image unavailable')
        async def unavailable(server, operation, arguments, *, absolute_deadline=None):
            raise transport_error
        with patch.object(NativeMCPServer, '_native_api', unavailable):
            with self.assertRaises(OSError) as caught:
                core.source_read_capabilities()
            self.assertIs(caught.exception, transport_error)

    def test_native_core_deadline_refuses_before_native_setup(self):
        import anyio
        from tos_access.native_mcp import NativeMCPServer
        server = NativeMCPServer('/explicit/installed')
        async def attempt(deadline):
            return await server._native_api('call', ('tos_source_read_capabilities', {}),
                                            absolute_deadline=deadline)
        for value in [float('nan'), float('inf'), True, '50']:
            with self.assertRaisesRegex(ValueError, 'deadline must be finite'):
                anyio.run(attempt, value)
        with self.assertRaisesRegex(TimeoutError, 'expired before setup'):
            anyio.run(attempt, time.monotonic() - 1)
        with self.assertRaisesRegex(TimeoutError, 'no remaining operation budget'):
            anyio.run(attempt, time.monotonic() + 1)


    def test_imported_native_tool_api_owned_child_cleanup(self):
        # Real tiny OS children/groups exercise the production stdlib owner.
        # Only native image admission and SDK semantics are substituted; this
        # is lifecycle evidence, never an installed native MCP association.
        import anyio
        from types import SimpleNamespace
        from tos_access import native_io, native_mcp
        real_popen = subprocess.Popen
        real_kill = os.killpg

        async def scenario(kind, directory):
            children, events = [], []
            pid_file = directory / 'descendant.pid'

            class CloseFailure:
                def __init__(self, stream):
                    self.stream = stream
                def fileno(self):
                    return self.stream.fileno()
                def close(self):
                    self.stream.close()
                    raise OSError('owned stdin close failure')

            class Child(real_popen):
                def wait(self, *args, **kwargs):
                    events.append(('wait', self.pid))
                    return super().wait(*args, **kwargs)

            def open_owned(*args, **kwargs):
                if kind == 'exited-leader':
                    code = (
                        "import subprocess,sys;from pathlib import Path;sys.stdin.buffer.readline();"
                        "p=subprocess.Popen([sys.executable,'-B','-c','import time;time.sleep(60)'],"
                        "stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);"
                        "Path(sys.argv[1]).write_text(str(p.pid))"
                    )
                    argv = [sys.executable, '-I', '-S', '-B', '-c', code, str(pid_file)]
                elif kind == 'cancel':
                    argv = [sys.executable, '-I', '-S', '-B', '-c', 'import time;time.sleep(60)']
                else:
                    argv = [sys.executable, '-I', '-S', '-B', '-c', 'import sys;sys.stdin.buffer.read()']
                child = Child(argv, **kwargs)
                children.append(child)
                if kind == 'close-error':
                    child.stdin = CloseFailure(child.stdin)
                return child

            def kill_owned(pid, sig):
                self.assertEqual(pid, children[0].pid)
                self.assertIn(sig, (signal.SIGTERM, signal.SIGKILL))
                self.assertNotIn(('wait', pid), events)
                events.append((sig, pid))
                return real_kill(pid, sig)

            class LifecycleSession:
                def __init__(self, incoming, outgoing, **kwargs):
                    self.outgoing = outgoing
                async def __aenter__(self):
                    return self
                async def __aexit__(self, *args):
                    pass
                async def initialize(self):
                    # The real SDK initialization writes a request. Preserve
                    # that zero-capacity stream handshake: its writer awaits
                    # channel_ready before accepting the message.
                    message = SimpleNamespace(model_dump=lambda **kwargs: {
                        'jsonrpc': '2.0', 'method': 'initialize', 'id': 1})
                    await self.outgoing.send(SimpleNamespace(message=message))
                    if kind == 'exited-leader':
                        with anyio.fail_after(2):
                            while not pid_file.exists():
                                await anyio.sleep(.01)
                async def list_tools(self):
                    if kind == 'cancel':
                        await anyio.sleep_forever()
                    return SimpleNamespace(tools=[])

            with patch.object(native_io.subprocess, 'Popen', open_owned), \
                    patch('mcp.ClientSession', LifecycleSession), patch('os.killpg', kill_owned):
                server = native_mcp.NativeMCPServer(directory.absolute())
                if kind == 'cancel':
                    with self.assertRaises(TimeoutError):
                        with anyio.fail_after(.1):
                            await server.list_tools()
                elif kind == 'close-error':
                    with self.assertRaises(BaseException) as raised:
                        await server.list_tools()
                    # AnyIO may wrap the primary close failure and custody
                    # failure in an ExceptionGroup; retain both error causes.
                    def errors(error):
                        yield str(error)
                        for member in getattr(error, 'exceptions', ()):
                            yield from errors(member)
                        if error.__cause__ is not None:
                            yield from errors(error.__cause__)
                    self.assertIn('owned stdin close failure', ' '.join(errors(raised.exception)))
                else:
                    self.assertEqual(await server.list_tools(), [])
            self.assertEqual(len(children), 1)
            child = children[0]
            self.assertEqual(events.count(('wait', child.pid)), 1)
            self.assertIn((signal.SIGKILL, child.pid), events)
            self.assertIsNotNone(child.returncode)
            if kind == 'exited-leader':
                self.assertEqual(child.returncode, 0)
                descendant = int(pid_file.read_text())
                with anyio.fail_after(2):
                    while True:
                        try:
                            state = Path(f'/proc/{descendant}/stat').read_text().split(') ', 1)[1].split()[0]
                            if state == 'Z':
                                break  # Reaping belongs to the adopted OS parent.
                        except FileNotFoundError:
                            break
                        await anyio.sleep(.01)

        with tempfile.TemporaryDirectory() as temporary:
            for kind in ('close-error', 'exited-leader', 'cancel'):
                with self.subTest(kind=kind):
                    directory = Path(temporary) / kind
                    directory.mkdir()
                    anyio.run(scenario, kind, directory)



    def test_imported_native_mcp_serving_has_explicit_software_and_stdio_boundary(self):
        from tos_access.native_mcp import NativeMCPServer

        options = ['--root', '/selected/data', '--source-inputs', '/selected/inputs.raw']
        # The serving caller must not require a reference core or Python MCP
        # dependency before dispatching the explicitly associated native image.
        with patch.dict(sys.modules, {'tos_access.core': None, 'mcp': None}), patch(
                'tos_access.native_dispatch.run', return_value='native-serving') as dispatch:
            server = NativeMCPServer(Path('/selected/software'), options)
            self.assertEqual(server.run(transport='stdio'), 'native-serving')
            dispatch.assert_called_once_with(Path('/selected/software'), [*options, 'mcp'])
            server.settings.host = '::1'
            server.settings.port = 5429
            server.run(transport='streamable-http')
            self.assertEqual(dispatch.call_args.args, (Path('/selected/software'),
                [*options, 'mcp', '--transport', 'streamable-http', '--host', '::1', '--port', '5429']))
            with self.assertRaises(ValueError):
                server.run(transport='unsupported')
            server.settings.host = '0.0.0.0'
            with self.assertRaises(ValueError):
                server.run(transport='streamable-http')
        with self.assertRaises(ValueError):
            NativeMCPServer('relative/software')
        with self.assertRaises(ValueError):
            NativeMCPServer('/selected/software', ['--root', 'bad\0path'])

    def test_data_selection_does_not_discover_cwd_corpus(self):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary)
            projection = root / "ToS/derived-exports/tos_corpus_index.min.json"
            projection.parent.mkdir(parents=True)
            projection.write_text('{"schema_version":"tos_corpus_index_v1"}')
            with patch("pathlib.Path.cwd", return_value=root):
                selected = data_root()
            self.assertNotEqual(selected, root)
            self.assertEqual(data_root(root), root)
            missing = root / "missing-explicit-data"
            with patch.dict(os.environ, {"TOS_DATA_ROOT": str(missing)}):
                self.assertEqual(data_root(), missing)



if __name__ == "__main__":
    unittest.main()

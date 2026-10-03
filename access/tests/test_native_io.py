"""Significant process custody failure paths, without launching a child."""
import signal
import time
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from tos_access.native_io import _Exchange, NativeCustodyError


class NativeIOCustodyTests(unittest.TestCase):
    def channel(self, events):
        channel = _Exchange([], '/installed', 1, 1024, (0,), None,
                            time.monotonic() + 10, None)
        child = SimpleNamespace(pid=123, stdin=Mock(), stdout=Mock(), stderr=Mock())
        child.wait = Mock(side_effect=lambda **kwargs: events.append('wait'))
        channel._child = child
        return channel

    def test_mismatched_waitid_never_signals_or_explicitly_reaps(self):
        channel = self.channel([])
        with patch('tos_access.native_io.os.waitid', return_value=SimpleNamespace(si_pid=124)):
            with patch('tos_access.native_io.os.killpg') as kill:
                with self.assertRaises(NativeCustodyError):
                    channel._release()
                kill.assert_not_called()
        channel._child.wait.assert_not_called()
        self.assertTrue(channel._unknown)

    def test_term_failure_still_anchors_kill_and_wait_preserving_failure(self):
        events = []
        channel = self.channel(events)
        def observe(*args):
            events.append('anchor')
            return None
        def kill(pid, sig):
            events.append(sig.name)
            if sig == signal.SIGTERM:
                raise PermissionError('controlled TERM failure')
        with patch('tos_access.native_io.os.waitid', side_effect=observe):
            with patch('tos_access.native_io.os.killpg', side_effect=kill):
                with self.assertRaisesRegex(NativeCustodyError, 'SIGTERM: PermissionError'):
                    channel._release()
        self.assertEqual(events, ['anchor', 'SIGTERM', 'anchor', 'SIGKILL', 'wait'])
        channel._child.wait.assert_called_once()
        self.assertTrue(channel._reaped)

"""Host resource bounds; subject coverage conformance uses an installed prefix."""
import io
from contextlib import redirect_stdout, redirect_stderr
import time
import unittest
from unittest.mock import patch

from tos_access.coverage import _bounded_json, coverage_row, main


class CoverageForwardingTests(unittest.TestCase):
    def test_oversized_string_refuses_before_encoder_or_child(self):
        with patch('tos_access.native_io.json.JSONEncoder.iterencode') as encoder:
            with self.assertRaisesRegex(ValueError, 'byte budget'):
                _bounded_json({'a': 'x' * 101}, 100, time.monotonic() + 5)
            encoder.assert_not_called()

    def test_escaped_string_and_expired_encoding_are_bounded(self):
        with patch('tos_access.native_io.json.JSONEncoder.iterencode') as encoder:
            with self.assertRaisesRegex(ValueError, 'byte budget'):
                _bounded_json({'a': '\0' * 20}, 100, time.monotonic() + 5)
            encoder.assert_not_called()
        with self.assertRaisesRegex(TimeoutError, 'deadline'):
            _bounded_json({'a': 'small'}, 100, time.monotonic() - 1)

    def test_language_host_type_cap_refuses_before_process_setup(self):
        with patch('tos_access.native_io.subprocess.Popen') as child:
            for value in (None, ['en'], 'x' * 129):
                with self.assertRaisesRegex(ValueError, 'language'):
                    coverage_row({}, language=value)
            child.assert_not_called()

    def test_cli_withholds_completion_after_transport_failure(self):
        def failed_stream(*args, **kwargs):
            yield {'schema_version': 'tos_knowledge_coverage_v1', 'enumeration_complete': True}
            raise ValueError('native exit refused')
        output = io.StringIO()
        with patch('tos_access.coverage._packets', failed_stream):
            with redirect_stdout(output), redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    main(['--root', '.', '--native-prefix', '/installed'])
        self.assertEqual(output.getvalue(), '')

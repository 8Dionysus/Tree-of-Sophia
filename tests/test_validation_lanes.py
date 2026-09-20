"""Command selection and failure propagation, independent of production data."""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import release_check  # noqa: E402
import validation_lanes  # noqa: E402


class ValidationLaneTests(unittest.TestCase):
    def fixture(self, root: Path, steps: list) -> None:
        path = root / validation_lanes.LANES_PATH
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps({'command_sequences': {'sample': steps}}))

    def test_selected_sequence_preserves_arguments_and_order(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self.fixture(root, [{'label': 'first', 'command': ['python', 'a.py', 'a b']},
                                {'label': 'second', 'command': ['tool', '--check']}])
            self.assertEqual(validation_lanes.command_sequence('sample', root),
                             [('first', [sys.executable, 'a.py', 'a b']), ('second', ['tool', '--check'])])
            with self.assertRaises(KeyError):
                validation_lanes.command_sequence('missing', root)

    def test_invalid_or_empty_sequence_cannot_succeed(self):
        for steps in ([], [None], [{'label':'invalid','command':[]}], [{'label':'invalid','command':['python',None]}]):
            with self.subTest(steps=steps), tempfile.TemporaryDirectory() as raw:
                root = Path(raw)
                self.fixture(root, steps)
                with self.assertRaises(ValueError):
                    validation_lanes.command_sequence('sample', root)

    def test_runner_stops_at_failure_and_preserves_exit_status(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            self.fixture(root, [{'label':'first','command':['a']}, {'label':'later','command':['b']}])
            with mock.patch.object(validation_lanes.subprocess, 'run', return_value=subprocess.CompletedProcess(['a'], 17)) as run:
                self.assertEqual(validation_lanes.run_sequence('sample', root), 17)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.kwargs['cwd'], root)

    def test_default_release_does_not_select_integration(self):
        steps = [('contracts', ['check']), ('run tests', ['test'])]
        with mock.patch.object(release_check, 'command_sequence', return_value=steps) as select, mock.patch.object(release_check, 'run_step', return_value=0) as run:
            self.assertEqual(release_check.main([]), 0)
        select.assert_called_once_with('release_check', release_check.REPO_ROOT)
        self.assertEqual(run.call_args_list, [mock.call(*step) for step in steps])

    def test_release_stops_at_failure(self):
        with mock.patch.object(release_check, 'command_sequence', return_value=[('bad',['a']),('later',['b'])]), mock.patch.object(release_check, 'run_step', return_value=23) as run:
            self.assertEqual(release_check.main([]), 23)
        self.assertEqual(run.call_count, 1)

    def test_phase_partition_is_complete_and_rejects_ambiguous_boundary(self):
        steps = [('contracts',['a']), ('run tests',['b'])]
        self.assertEqual(release_check.select_steps(steps,'checks') + release_check.select_steps(steps,'tests'), steps)
        for invalid in ([('other',['a'])], [('run tests',['a']),('later',['b'])], [('run tests',['a']),('run tests',['b'])]):
            with self.assertRaises(ValueError):
                release_check.select_steps(invalid,'tests')


if __name__ == '__main__':
    unittest.main()

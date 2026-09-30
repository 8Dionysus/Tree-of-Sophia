"""Command selection and failure propagation, independent of production data."""
from __future__ import annotations

import ast
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
        steps = [('contracts',['a']), ('run tests: access',['b']), ('run tests: source',['c'])]
        self.assertEqual(release_check.select_steps(steps,'checks') + release_check.select_steps(steps,'tests'), steps)
        self.assertEqual(release_check.select_steps(steps, 'tests'), steps[1:])
        legacy = [('contracts', ['a']), ('run tests', ['b'])]
        self.assertEqual(release_check.select_steps(legacy, 'tests'), legacy[1:])
        for invalid in ([('other',['a'])], [('run tests: one',['a']),('later',['b'])],
                        [('run tests',['a']),('run tests',['b'])],
                        [('run tests: one',['a']),('run tests',['b'])],
                        [('run tests: ', ['a'])]):
            with self.assertRaises(ValueError):
                release_check.select_steps(invalid,'tests')

    def test_release_test_groups_cover_full_software_inventory_once(self):
        sequence = validation_lanes.command_sequence('release_check', ROOT)
        test_steps = release_check.select_steps(sequence, 'tests')
        self.assertEqual(len(test_steps), 4)
        expected_prefix = [sys.executable, '-m', 'pytest', '-q', '-p', 'no:cacheprovider',
                           '--strict-markers', '-m', 'not data_release']
        selected = []
        for label, command in test_steps:
            with self.subTest(label=label):
                self.assertTrue(label.startswith('run tests: '))
                self.assertEqual(command[:len(expected_prefix)], expected_prefix)
                self.assertTrue(command[len(expected_prefix):])
                selected.extend(command[len(expected_prefix):])

        access_files = sorted({
            path.relative_to(ROOT).as_posix()
            for pattern in ('test_*.py', '*_test.py')
            for path in (ROOT / 'access/tests').rglob(pattern)
        })
        software_support_files = [
            'tests/test_validation_lanes.py', 'tests/test_acquisition_batch.py',
            'tests/test_acquisition_handoff_adapter.py', 'tests/test_file_membership.py',
            'tests/test_corpus_archive.py', 'tests/test_corpus_store.py',
            'tests/test_corpus_r2.py', 'tests/test_corpus_locator.py',
            'tests/test_corpus_admit.py', 'tests/test_corpus_source_validation.py',
            'tests/test_corpus_source_retirement.py', 'tests/test_corpus_build_worker.py',
            'tests/test_local_stats_port.py', 'tests/test_build_kag_export.py',
            'tests/test_downstream_status.py', 'tests/test_publish_kag_release.py',
            'tests/test_publish_stats_release.py', 'tests/test_software_ci.py',
        ]
        self.assertEqual(len(selected), len(set(selected)))
        self.assertEqual([path for path in selected if path.startswith('access/tests/')], access_files)
        self.assertEqual([path for path in selected if path.startswith('tests/')], software_support_files)
        self.assertEqual(set(selected), set(access_files + software_support_files))
        self.assertEqual(release_check.select_steps(sequence, 'checks') + test_steps, sequence)

    def test_rust_workspace_timeout_partition_covers_each_process_cold_case_once(self):
        cases = [
            ('source_creation_store::revision_publication::tests::native_record_revisions_cover_fixed_handlers_process_cold_and_exact_recovery',
             'rust/crates/tos-command/src/source_record_revision_tests.rs',
             'native_record_revisions_cover_fixed_handlers_process_cold_and_exact_recovery'),
            ('source_creation_store::work_expression::tests::native_work37_cli_creates_process_cold_replays_and_recovers_exact_pending',
             'rust/crates/tos-command/src/source_work_expression_tests.rs',
             'native_work37_cli_creates_process_cold_replays_and_recovers_exact_pending'),
            ('source_creation_store::work_expression::tests::real_pending_refuses_changed_dependency_then_resumes_or_rolls_back',
             'rust/crates/tos-command/src/source_work_expression_tests.rs',
             'real_pending_refuses_changed_dependency_then_resumes_or_rolls_back'),
        ]
        names = [name for name, _, _ in cases]
        sequence = validation_lanes.command_sequence('rust_workspace', ROOT)
        by_label = dict(sequence)
        workspace_label = 'test Rust workspace excluding isolated process-cold fixtures'
        workspace = by_label[workspace_label]
        self.assertEqual(workspace[:4], ['cargo', 'test', '--workspace', '--locked'])
        self.assertEqual(workspace[4], '--')
        self.assertEqual(
            [workspace[index + 1] for index, value in enumerate(workspace) if value == '--skip'],
            names,
        )

        singleton_labels = [
            'test isolated process-cold source revision fixture',
            'test isolated process-cold Work37 fixture',
            'test isolated process-cold Work recovery fixture',
        ]
        rust_test_labels = [
            label for label, _ in sequence
            if label == workspace_label or label.startswith('test isolated process-cold ')
        ]
        self.assertEqual(rust_test_labels, [workspace_label, *singleton_labels])
        self.assertEqual(
            [by_label[label] for label in singleton_labels],
            [
                ['cargo', 'test', '--locked', '-p', 'tos-command', '--lib', name, '--', '--exact']
                for name in names
            ],
        )
        self.assertEqual(
            [label for label, _ in sequence if label.startswith('test isolated process-cold ')],
            singleton_labels,
        )

        for (_, source_path, function_name) in cases:
            source = (ROOT / source_path).read_text(encoding='utf-8')
            self.assertEqual(source.count(f'#[test]\nfn {function_name}('), 1)

    def test_browser_behavior_groups_cover_the_exact_e2e_function_inventory(self):
        sequence = validation_lanes.command_sequence('software_browser', ROOT)
        groups = [
            (label, command)
            for label, command in sequence
            if label.startswith('browser behavior: ')
        ]
        self.assertEqual(len(groups), 5)
        self.assertEqual(len({label for label, _ in groups}), len(groups))

        module = ast.parse((ROOT / 'access/e2e/test_webmcp.py').read_text(encoding='utf-8'))
        expected = [
            f'access/e2e/test_webmcp.py::{node.name}'
            for node in module.body
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and node.name.startswith('test_')
        ]
        selected = []
        for label, command in groups:
            with self.subTest(label=label):
                self.assertEqual(command[1:3], ['-m', 'pytest'])
                self.assertEqual(command[3], '-q')
                self.assertTrue(command[4:])
                selected.extend(command[4:])
        self.assertEqual(selected, expected)
        self.assertEqual(len(selected), len(set(selected)))

    def test_native_release_forwards_only_explicit_executor_limits(self):
        executable = '/tmp/tos-release-check-test-executor'
        with mock.patch.dict(release_check.os.environ,
                             {'TOS_RELEASE_CHECK_EXECUTOR': executable}), \
             mock.patch.object(release_check.os, 'execv') as execv:
            self.assertIsNone(release_check.native_main(['--phase', 'tests']))
            execv.assert_called_once_with(executable, [
                executable, '--repo-root', str(ROOT), '--python', sys.executable,
                '--phase', 'tests',
            ])
            execv.reset_mock()

            self.assertIsNone(release_check.native_main([
                '--phase', 'tests', '--command-timeout-ms', '900000',
                '--lane-timeout-ms', '3600000', '--cleanup-grace-ms', '2000',
                '--max-output-bytes', '8388608',
            ]))
            execv.assert_called_once_with(executable, [
                executable, '--repo-root', str(ROOT), '--python', sys.executable,
                '--phase', 'tests', '--command-timeout-ms', '900000',
                '--lane-timeout-ms', '3600000', '--cleanup-grace-ms', '2000',
                '--max-output-bytes', '8388608',
            ])


if __name__ == '__main__':
    unittest.main()

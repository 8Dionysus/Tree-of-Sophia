"""Command selection and failure propagation, independent of production data."""
from __future__ import annotations

import ast
import hashlib
import io
import json
from pathlib import Path
import re
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

    def test_rust_workspace_partitions_cover_each_conformance_family_once(self):
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
        conformance_cases = [
            ('command_artifact_cases::native_artifact_cli_describes_prepares_creates_and_cold_replays_exact_bytes',
             'tests/conformance/rust/command_artifact_cases.rs',
             'native_artifact_cli_describes_prepares_creates_and_cold_replays_exact_bytes'),
            ('command_claim_cases::claim_successor_retains_bytes_replays_current_scope_and_refuses_unissued_admission',
             'tests/conformance/rust/command_claim_cases.rs',
             'claim_successor_retains_bytes_replays_current_scope_and_refuses_unissued_admission'),
            ('command_claim_cases::initial_claim_creation_publishes_five_native_files_and_cold_replays',
             'tests/conformance/rust/command_claim_cases.rs',
             'initial_claim_creation_publishes_five_native_files_and_cold_replays'),
            ('command_claim_cases::initial_collection_order_binds_retained_version_and_cold_replays',
             'tests/conformance/rust/command_claim_cases.rs',
             'initial_collection_order_binds_retained_version_and_cold_replays'),
        ]
        names = [name for name, _, _ in cases]
        conformance_names = [name for name, _, _ in conformance_cases]
        sequence = validation_lanes.command_sequence('rust_workspace', ROOT)
        by_label = dict(sequence)
        self.assertEqual(
            [label for label, _ in sequence[:5]],
            [
                'check Rust formatting',
                'build exact native schema worker for source-cut fixtures',
                'build exact native owner CLI for process-cold fixtures',
                'build exact native prepared consumer for conformance fixtures',
                'compile exact Rust conformance test image',
            ],
        )
        self.assertEqual(
            by_label['build exact native prepared consumer for conformance fixtures'],
            ['cargo', 'build', '--locked', '-p', 'tos-access', '--bin', 'tos-access'],
        )
        self.assertEqual(
            by_label['compile exact Rust conformance test image'],
            ['cargo', 'test', '--no-run', '--workspace', '--locked', '--message-format=json'],
        )
        workspace_label = 'test Rust workspace remainder excluding conformance and isolated process-cold fixtures'
        workspace = by_label[workspace_label]
        manifest_steps = validation_lanes.load_manifest(ROOT)['command_sequences']['rust_workspace']
        timeout_steps = [(step['label'], step['command_timeout_ms']) for step in manifest_steps
                         if 'command_timeout_ms' in step]
        family_labels = {
            'test Rust conformance Form and Record families': {
                'command_form_cases', 'command_record_cases',
            },
            'test Rust conformance Claim publication families': {
                'command_claim_cases', 'command_claim_publication_cases',
            },
            'test Rust conformance lifecycle families': {
                'command_collection_cases', 'command_item_cases', 'command_work_cases',
                'command_edition_cases',
                'command_object_link_cases', 'command_legacy_claim_cases',
            },
            'test Rust conformance text alignment case': {
                'command_text_cases',
            },
            'test Rust conformance derived and public Text families': {
                'command_text_cases', 'command_public_text_cases',
            },
            'test Rust conformance Responsibility family': {
                'command_responsibility_cases',
            },
            'test Rust conformance owner assessment family': {
                'command_owner_text_cases',
            },
            'test Rust conformance private Claim family': {
                'command_private_claim_cases',
            },
            'test Rust conformance private Profile and metadata families': {
                'command_metadata_publication_cases', 'command_private_profile_cases',
            },
        }
        self.assertEqual(
            timeout_steps,
            [(workspace_label, 900000),
             ('test Rust conformance root and source families', 900000),
             *[(label, 900000) for label in family_labels],
             ('test isolated process-cold source revision fixture', 1020000)],
        )
        self.assertEqual(
            workspace[:8],
            ['cargo', 'test', '--workspace', '--locked', '--no-fail-fast', '--exclude', 'tos-conformance', '--'],
        )
        self.assertIn('--nocapture', workspace)
        self.assertEqual(
            [workspace[index + 1] for index, value in enumerate(workspace) if value == '--skip'],
            names,
        )

        runner = (ROOT / 'tests/conformance/rust/runner.rs').read_text(encoding='utf-8')
        modules = re.findall(r'(?m)^\s*mod (command_[a-z0-9_]+);$', runner)
        self.assertEqual(len(modules), len(set(modules)))
        self.assertEqual(
            set(modules),
            set().union(*family_labels.values())
            | {name.split('::', 1)[0] for name in conformance_names},
        )

        def filters(command):
            args = command[command.index('--') + 1:]
            return [args[index + 1] for index, value in enumerate(args) if value == '--skip']

        root_source_label = 'test Rust conformance root and source families'
        root_source = by_label[root_source_label]
        self.assertEqual(
            root_source[:7],
            ['cargo', 'test', '--workspace', '--locked', '--test', 'conformance', '--'],
        )
        self.assertEqual(filters(root_source), [f'{module}::' for module in modules])
        self.assertTrue(all(value.endswith('::') for value in filters(root_source)))

        module_paths = re.findall(
            r'(?m)^\s*(?:#\[path = "([^"]+)"\]\s*\n)?\s*mod ([a-z0-9_]+);$',
            runner,
        )
        command_test_names = {}
        for relative, module in module_paths:
            if not module.startswith('command_'):
                continue
            source_path = ROOT / 'tests/conformance/rust' / (relative or f'{module}.rs')
            source = source_path.read_text(encoding='utf-8')
            test_names = re.findall(
                r'(?m)^\s*#\[test\]\s*\n(?:^\s*#\[[^\n]*\]\s*\n)*^\s*fn\s+([a-zA-Z0-9_]+)\s*\(',
                source,
            )
            self.assertTrue(test_names, f'{source_path.relative_to(ROOT)} has no direct tests')
            test_ids = [f'{module}::{name}' for name in test_names]
            self.assertEqual(len(test_ids), len(set(test_ids)))
            command_test_names[module] = set(test_ids)

        self.assertEqual(set(command_test_names), set(modules))
        all_command_test_ids = set().union(*command_test_names.values())

        def selected_tests(command):
            self.assertEqual(
                command[:6],
                ['cargo', 'test', '--workspace', '--locked', '--test', 'conformance'],
            )
            self.assertEqual(command[7], '--')
            test_filter = command[6]
            test_args = command[command.index('--') + 1:]
            if '--exact' in test_args:
                selected = {test_id for test_id in all_command_test_ids
                            if test_id == test_filter}
            else:
                selected = {test_id for test_id in all_command_test_ids
                            if test_filter in test_id}
            return {
                test_id for test_id in selected
                if not any(skip in test_id for skip in filters(command))
            }

        family_test_coverage = []
        for label, expected in family_labels.items():
            command = by_label[label]
            module_filters = [value[:-2] for value in filters(command) if value.endswith('::')]
            self.assertEqual(len(module_filters), len(set(module_filters)))
            self.assertTrue(set(module_filters).issubset(set(modules)))
            selected = selected_tests(command)
            selected_modules = {test_id.split('::', 1)[0] for test_id in selected}
            self.assertEqual(selected_modules, expected)
            self.assertTrue(all(value.endswith('::') or value in all_command_test_ids
                                for value in filters(command)))
            self.assertTrue(selected)
            family_test_coverage.extend(selected)
        self.assertEqual(len(family_test_coverage), len(set(family_test_coverage)))
        claim_family_exact_skips = [
            value for value in filters(by_label['test Rust conformance Claim publication families'])
            if not value.endswith('::')
        ]
        claim_conformance_names = [
            name for name in conformance_names if name.startswith('command_claim_cases::')
        ]
        self.assertCountEqual(claim_family_exact_skips, claim_conformance_names)
        self.assertEqual(
            set(family_test_coverage) | set(conformance_names),
            all_command_test_ids,
        )
        self.assertFalse(set(family_test_coverage) & set(conformance_names))

        for relative, module in module_paths:
            if module.startswith('command_'):
                continue
            source_path = ROOT / 'tests/conformance/rust' / (relative or f'{module}.rs')
            source = source_path.read_text(encoding='utf-8')
            test_names = re.findall(
                r'(?m)^\s*#\[test\]\s*\n(?:^\s*#\[[^\n]*\]\s*\n)*^\s*fn\s+([a-zA-Z0-9_]+)\s*\(',
                source,
            )
            self.assertFalse(
                any('command_' in name for name in test_names),
                f'{source_path.relative_to(ROOT)} has tests that would leak into command_ filtered groups',
            )

        segment = by_label['test Rust segment conformance target']
        self.assertEqual(
            segment,
            ['cargo', 'test', '--workspace', '--locked', '--test', 'segment-conformance', '--',
             '--nocapture'],
        )
        self.assertTrue(all(
            '--workspace' in command
            for label, command in sequence
            if label.startswith('test Rust ')
        ))

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

        conformance_labels = [
            'test isolated conformance Artifact fixture',
            'test isolated conformance Claim successor fixture',
            'test isolated conformance initial Claim fixture',
            'test isolated conformance Collection order fixture',
        ]
        self.assertEqual(
            [label for label, _ in sequence if label.startswith('test isolated conformance ')],
            conformance_labels,
        )
        self.assertEqual(
            [by_label[label] for label in conformance_labels],
            [
                ['cargo', 'test', '--workspace', '--locked', '--test', 'conformance', name,
                 '--', '--exact', '--nocapture']
                for name in conformance_names
            ],
        )
        self.assertEqual(len(names + conformance_names), len(set(names + conformance_names)))

        ordered_test_labels = [
            workspace_label,
            root_source_label,
            *family_labels,
            'test Rust segment conformance target',
            *singleton_labels,
            *conformance_labels,
        ]
        self.assertEqual(
            [label for label, _ in sequence if label in set(ordered_test_labels)],
            ordered_test_labels,
        )

        for (_, source_path, function_name) in cases + conformance_cases:
            source = (ROOT / source_path).read_text(encoding='utf-8')
            self.assertEqual(source.count(f'#[test]\nfn {function_name}('), 1)

    def test_explicit_command_deadline_refuses_malformed_or_unapplied_budget(self):
        self.assertIsNone(validation_lanes._command_timeout_ms('rust_workspace', {}))
        self.assertEqual(validation_lanes._command_timeout_ms(
            'rust_workspace', {'command_timeout_ms': 900000}), 900000)
        with self.assertRaises(ValueError):
            validation_lanes._command_timeout_ms('software_browser', {'command_timeout_ms': 900000})
        for value in (None, True, 0, -1, 900000.0, 3600001):
            with self.subTest(value=value), self.assertRaises(ValueError):
                validation_lanes._command_timeout_ms('rust_workspace', {'command_timeout_ms': value})

    def test_rust_conformance_artifact_selection_uses_exact_test_package_and_target(self):
        selected = Path('/runner/cargo-target/debug/deps/conformance-deadbeef')
        messages = [
            {
                'reason': 'compiler-artifact',
                'package_id': 'path+file:///workspace/tests/conformance/rust#tos-conformance@1.0.0',
                'target': {'name': 'conformance', 'kind': ['test']},
                'executable': str(selected),
            },
            {
                'reason': 'compiler-artifact',
                'package_id': 'path+file:///workspace/rust/crates/tos-access#tos-access@1.0.0',
                'target': {'name': 'conformance', 'kind': ['test']},
                'executable': '/runner/cargo-target/debug/deps/not-the-case',
            },
            {
                'reason': 'compiler-artifact',
                'package_id': 'path+file:///workspace/tests/conformance/rust#tos-conformance@1.0.0',
                'target': {'name': 'conformance', 'kind': ['bin']},
                'executable': '/runner/cargo-target/debug/conformance',
            },
        ]
        self.assertEqual(
            validation_lanes._cargo_test_artifacts(messages, 'tos-conformance', 'conformance'),
            {selected},
        )

    def test_cargo_json_runner_streams_diagnostics_and_returns_exact_case_image(self):
        selected = Path('/runner/cargo-target/debug/deps/conformance-deadbeef')
        artifact = {
            'reason': 'compiler-artifact',
            'package_id': 'path+file:///workspace/tests/conformance/rust#tos-conformance@1.0.0',
            'target': {'name': 'conformance', 'kind': ['test']},
            'executable': str(selected),
        }
        process = mock.Mock(stdout=io.StringIO(json.dumps(artifact) + '\n'), wait=mock.Mock(return_value=0))
        root = Path('/workspace')
        env = {'CARGO_TARGET_DIR': '/runner/cargo-target'}
        with mock.patch.object(validation_lanes.subprocess, 'Popen', return_value=process) as popen:
            result = validation_lanes._run_cargo_json_build(
                ['cargo', 'test', '--no-run'], root, env, 'tos-conformance', 'conformance'
            )
        self.assertEqual(result, (0, {selected}))
        popen.assert_called_once_with(
            ['cargo', 'test', '--no-run'], cwd=root, env=env,
            stdout=subprocess.PIPE, stderr=None, text=True,
        )

    def test_rust_workspace_hashes_lane_products_and_forwards_exact_evidence(self):
        with tempfile.TemporaryDirectory() as raw:
            target = Path(raw) / 'cargo-target'
            consumer = target / 'debug' / 'tos-access'
            conformance = target / 'debug' / 'deps' / 'conformance-deadbeef'
            consumer.parent.mkdir(parents=True)
            conformance.parent.mkdir(parents=True)
            consumer.write_bytes(b'prepared access consumer')
            conformance.write_bytes(b'Claim publication conformance image')
            consumer.chmod(0o700)
            conformance.chmod(0o700)
            steps = [
                ('compile exact Rust conformance test image', ['cargo', 'test', '--no-run']),
                ('test Rust workspace fixtures', ['cargo', 'test', '--workspace']),
            ]
            with mock.patch.object(validation_lanes, 'command_sequence', return_value=steps), \
                 mock.patch.dict(validation_lanes.os.environ, {
                     'CARGO_TARGET_DIR': str(target),
                     'TOS_NATIVE_PREPARED_CONSUMER_BIN': str(consumer),
                 }), \
                 mock.patch.object(validation_lanes, '_run_cargo_json_build', return_value=(0, {conformance})), \
                 mock.patch.object(validation_lanes.subprocess, 'run',
                                   return_value=subprocess.CompletedProcess(['cargo'], 0)) as run:
                self.assertEqual(validation_lanes.run_sequence('rust_workspace', Path(raw)), 0)

            run.assert_called_once()
            executed_env = run.call_args.kwargs['env']
            self.assertEqual(
                executed_env['TOS_NATIVE_PREPARED_CONSUMER_SHA256'],
                hashlib.sha256(b'prepared access consumer').hexdigest(),
            )
            self.assertEqual(
                executed_env['TOS_NATIVE_CLAIM_PUBLICATION_CASE_SHA256'],
                hashlib.sha256(b'Claim publication conformance image').hexdigest(),
            )
            self.assertEqual(executed_env['TOS_NATIVE_PREPARED_CONSUMER_BIN'], str(consumer))

    def test_rust_artifact_hash_fails_closed_outside_current_cargo_target(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            target = root / 'target'
            target.mkdir()
            outside = root / 'outside'
            outside.write_bytes(b'not a lane artifact')
            outside.chmod(0o700)
            with self.assertRaisesRegex(ValueError, 'inside Cargo target'):
                validation_lanes._sha256_executable(outside, 'consumer', target, root)

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

    def test_native_validation_forwards_rust_run_to_exact_executor(self):
        executable = '/tmp/tos-validation-lanes-test-executor'
        with mock.patch.dict(validation_lanes.os.environ,
                             {'TOS_VALIDATION_LANES_EXECUTOR': executable}), \
             mock.patch.object(validation_lanes.os, 'execv') as execv:
            self.assertIsNone(validation_lanes.native_main([
                '--run', 'rust_workspace', '--lane-timeout-ms', '5400000',
            ]))
            execv.assert_called_once_with(executable, [
                executable, '--repo-root', str(ROOT), '--python', sys.executable,
                '--run', 'rust_workspace', '--lane-timeout-ms', '5400000',
            ])


if __name__ == '__main__':
    unittest.main()

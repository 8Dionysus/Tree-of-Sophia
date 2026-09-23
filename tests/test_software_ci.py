"""Check omission must follow the changed surface, and may never hide failure."""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import yaml

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import software_ci as ci


class SoftwareSelectionTests(unittest.TestCase):
    def test_surface_and_mixed_change_selection(self):
        cases = [
            (['README.md', 'docs/RELEASING.md'], 'none', False),
            (['access/web/src/Graph.tsx'], 'browser', False),
            (['access/e2e/test_webmcp.py'], 'browser', False),
            (['access/src/tos_access/cli.py'], 'reader', True),
            (['access/tests/test_data_access.py'], 'reader', True),
            (['access/deploy/cloudflare-worker/src/index.ts'], 'none', True),
            (['access/web/package-lock.json', 'access/deploy/cloudflare-worker/package.json'], 'browser', True),
            (['docs/RELEASING.md', 'access/src/tos_access/core.py'], 'reader', True),
        ]
        for paths, mode, worker in cases:
            with self.subTest(paths=paths):
                plan = ci.select(paths)
                self.assertEqual((plan['software_mode'], plan['worker']), (mode, worker))

    def test_shared_unknown_source_and_selection_changes_fail_closed_to_full(self):
        for path in ['access/contracts/query-store.v1.json', 'access/profiles/reader.json',
                     'access/packaging/build_software_bundle.py', 'requirements-dev.txt',
                     'pytest.ini', '.github/workflows/repo-validation.yml',
                     'scripts/software_ci.py', 'scripts/new_compiler.py',
                     'tests/test_software_ci.py', 'AGENTS.md', 'access/AGENTS.md',
                     'ToS/doctrine/README.md', 'ToS/source-witnesses/record.json',
                     'new-unclassified-directory/input.xyz']:
            with self.subTest(path=path):
                plan = ci.select([path])
                self.assertEqual((plan['software_mode'], plan['worker']), ('full', True))
        for paths, full in [([], False), (['README.md'], True)]:
            self.assertEqual(ci.select(paths, full)['software_mode'], 'full')

    def test_renaming_code_to_documentation_still_selects_original_owner(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, stderr=subprocess.DEVNULL)
            git('init')
            git('config', 'user.name', 'CI fixture')
            git('config', 'user.email', 'fixture@example.invalid')
            old = root / 'access/src/tos_access/core.py'
            old.parent.mkdir(parents=True)
            old.write_text('some example\n')
            git('add', '.')
            git('commit', '-m', 'baseline')
            base = git('rev-parse', 'HEAD').decode().strip()
            old.rename(root / 'README.md')
            git('add', '-A')
            git('commit', '-m', 'move')
            paths = ci.changed_paths(root, base)
            self.assertEqual(paths, ['README.md', 'access/src/tos_access/core.py'])
            self.assertEqual(ci.select(paths)['software_mode'], 'reader')

    def test_required_gate_rejects_failed_cancelled_missing_and_unexpected_skips(self):
        for mode, worker in [('none', False), ('browser', False), ('reader', True), ('full', True), ('none', True)]:
            needs = {'plan': {'result':'success', 'outputs': {'software_mode':mode, 'worker':str(worker).lower()}},
                     'software': {'result':'skipped' if mode == 'none' else 'success'},
                     'worker': {'result':'success' if worker else 'skipped'}}
            ci.gate(needs)
            for job in needs:
                for bad in ['failure', 'cancelled', None]:
                    changed = json.loads(json.dumps(needs)); changed[job]['result'] = bad
                    with self.subTest(mode=mode, worker=worker, job=job, bad=bad), self.assertRaises(ValueError):
                        ci.gate(changed)
                changed = json.loads(json.dumps(needs)); del changed[job]
                with self.assertRaises(ValueError):
                    ci.gate(changed)
            if mode != 'none':
                needs['software']['result'] = 'skipped'
                with self.assertRaises(ValueError):
                    ci.gate(needs)
        for outputs in [{}, {'software_mode':'none', 'worker':'maybe'}, {'software_mode':'typo', 'worker':'false'}]:
            with self.assertRaises(ValueError):
                ci.gate({'plan': {'result':'success', 'outputs': outputs}})

    def test_document_links_check_new_repo_targets_without_fetching_external_urls(self):
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw)
            subprocess.run(['git','init',str(root)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            (root/'README.md').write_text('[missing](absent.md)\n[web](https://example.invalid/page)\n')
            errors=ci.check_docs(root,'HEAD',['README.md'])
            self.assertEqual(len(errors),1)
            self.assertIn('absent.md',errors[0])
            (root/'absent.md').write_text('exists\n')
            self.assertEqual(ci.check_docs(root,'HEAD',['README.md']),[])
            (root/'README.md').write_text('<<<<<<< branch\n')
            self.assertIn('merge marker',ci.check_docs(root,'HEAD',['README.md'])[0])

    def test_fenced_examples_and_reference_links(self):
        self.assertEqual(ci.links('```md\n[x](fake.md)\n```\n[x](real.md#part)\n[r]: other.md\n'), {'real.md#part','other.md'})

    def test_workflow_preserves_selection_and_full_release_entrypoint(self):
        workflow=yaml.safe_load((ROOT/'.github/workflows/repo-validation.yml').read_text())
        jobs=workflow['jobs']
        self.assertIn('workflow_dispatch',workflow.get('on',workflow.get(True)))
        self.assertEqual(set(jobs['required_gate']['needs']), {'plan','software','worker'})
        self.assertIn('always()',jobs['required_gate']['if'])
        self.assertIn("!= 'none'",jobs['software']['if'])
        self.assertIn("== 'true'",jobs['worker']['if'])
        self.assertEqual(jobs['software']['needs'],'plan')
        steps=jobs['software']['steps']
        full=[s for s in steps if s.get('run')=='python scripts/release_check.py --phase tests']
        self.assertEqual(len(full),1)
        self.assertIn("== 'full'",full[0]['if'])
        reader=[s for s in steps if s.get('run')=='python scripts/validation_lanes.py --run software_reader']
        self.assertEqual(len(reader),1)
        self.assertIn("== 'reader'",reader[0]['if'])
        gate_steps=jobs['required_gate']['steps']
        self.assertEqual(gate_steps[-1]['run'],'python scripts/software_ci.py gate')
        self.assertEqual(gate_steps[-1]['env']['CI_NEEDS'],'${{ toJSON(needs) }}')

    def test_acquisition_custody_tests_are_in_required_software_validation(self):
        required_tests={
            'tests/test_acquisition_batch.py',
            'tests/test_acquisition_handoff_adapter.py',
        }
        required_fixtures={
            'ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/item.json',
            'ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/item.manifest.json',
            'ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/rights.json',
            'ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/provenance.jsonl',
        }
        lanes=json.loads((ROOT/'docs/validation/validation_lanes.json').read_text())
        test_step=next(
            step for step in lanes['command_sequences']['release_check']
            if step.get('label')=='run tests'
        )
        self.assertTrue(required_tests <= set(test_step['command']))

        workflow=yaml.safe_load((ROOT/'.github/workflows/repo-validation.yml').read_text())
        checkout=next(
            step for step in workflow['jobs']['software']['steps']
            if 'sparse-checkout' in step.get('with',{})
        )
        sparse_paths=set(checkout['with']['sparse-checkout'].splitlines())
        self.assertTrue({f'/{path}' for path in required_tests | required_fixtures} <= sparse_paths)


if __name__ == '__main__':
    unittest.main()

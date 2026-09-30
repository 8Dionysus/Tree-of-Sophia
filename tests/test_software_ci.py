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
            (['rust/crates/tos-foundation/src/lib.rs'], 'browser', False),
            (['tests/conformance/rust/source-profile.json'], 'browser', False),
            (['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'], 'browser', False),
        ]
        for paths, mode, worker in cases:
            with self.subTest(paths=paths):
                plan = ci.select(paths)
                self.assertEqual((plan['software_mode'], plan['worker']), (mode, worker))
                self.assertEqual(plan['rust'], any(path in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml') or path.startswith(('rust/', 'tests/conformance/rust/')) for path in paths))

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
                self.assertTrue(plan['rust'])
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
        for mode, worker, rust in [('none', False, False), ('none', False, True), ('browser', False, False), ('reader', True, False), ('full', True, True), ('none', True, False)]:
            needs = {'plan': {'result':'success', 'outputs': {'software_mode':mode, 'worker':str(worker).lower(), 'rust':str(rust).lower()}},
                     'software': {'result':'skipped' if mode == 'none' else 'success'},
                     'worker': {'result':'success' if worker else 'skipped'},
                     'rust': {'result':'success' if rust else 'skipped'}}
            ci.gate(needs)
            for job in needs:
                for bad in ['failure', 'cancelled', None]:
                    changed = json.loads(json.dumps(needs)); changed[job]['result'] = bad
                    with self.subTest(mode=mode, worker=worker, rust=rust, job=job, bad=bad), self.assertRaises(ValueError):
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
        self.assertEqual(set(jobs['required_gate']['needs']), {'plan','software','worker','rust'})
        self.assertIn('always()',jobs['required_gate']['if'])
        self.assertIn("!= 'none'",jobs['software']['if'])
        self.assertIn("== 'true'",jobs['worker']['if'])
        self.assertIn("== 'true'",jobs['rust']['if'])
        self.assertEqual(jobs['software']['needs'],'plan')
        steps=jobs['software']['steps']
        full=[s for s in steps if s.get('run','').startswith('python scripts/release_check.py --phase tests')]
        self.assertEqual(len(full),1)
        self.assertIn('--command-timeout-ms 900000', full[0]['run'])
        self.assertIn("== 'full'",full[0]['if'])
        reader=[s for s in steps if s.get('run')=='python scripts/validation_lanes.py --run software_reader']
        self.assertEqual(len(reader),1)
        self.assertIn("== 'reader'",reader[0]['if'])
        native_package = [s for s in steps if 'software install --archive' in s.get('run', '')]
        self.assertEqual(len(native_package), 1)
        for operation in ('software build --root', 'software verify --archive', 'software install --archive'):
            self.assertIn(operation, native_package[0]['run'])
        package_run = native_package[0]['run']
        self.assertLess(package_run.index('mkdir -p -- "$root/dist"'),
                        package_run.index('software build --root'))
        self.assertIn('env -i PATH=', native_package[0]['run'])
        self.assertNotIn('pip install', native_package[0]['run'])
        legacy_reference = [s for s in steps if 'tree-of-sophia-legacy-reference.zip' in s.get('run', '')]
        self.assertEqual(len(legacy_reference), 1)
        self.assertIn('validate_software_bundle.py', legacy_reference[0]['run'])
        command_lab = [s for s in jobs['rust']['steps']
                       if 'cargo test -p tos-command --features postgres-lab --test postgres_durable_lab --locked -- --nocapture' in s.get('run', '')]
        self.assertEqual(len(command_lab), 1)
        self.assertTrue(command_lab[0]['env']['TOS_CMD_POSTGRES_URL'])
        self.assertIn('postgres', jobs['rust']['services'])
        # The entry wrappers execute native products before Cargo lanes can run.
        plan_runs = [step.get('run', '') for step in jobs['plan']['steps']]
        prepare = next(i for i, run in enumerate(plan_runs) if '--bin tos-software-ci' in run)
        self.assertIn('--no-default-features', plan_runs[prepare])
        selector = next(i for i, run in enumerate(plan_runs) if 'scripts/software_ci.py plan' in run)
        self.assertLess(prepare, selector)
        for job in ('software', 'rust', 'required_gate'):
            steps = jobs[job]['steps']
            bind = next(i for i, step in enumerate(steps) if step.get('name') == 'Verify and bind exact native CI executors')
            caller = next(i for i, step in enumerate(steps) if any(command in step.get('run', '') for command in
                          ('python scripts/release_check.py', 'python scripts/validation_lanes.py', 'python scripts/software_ci.py gate')))
            self.assertLess(bind, caller)
            self.assertIn("manifest['binaries'][name]", steps[bind]['run'])
            self.assertIn('lock_sha256', steps[bind]['run'])
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
        test_steps=[
            step for step in lanes['command_sequences']['release_check']
            if step.get('label')=='run tests' or step.get('label', '').startswith('run tests: ')
        ]
        selected_tests=set().union(*(set(step['command']) for step in test_steps))
        self.assertTrue(required_tests <= selected_tests)

        workflow=yaml.safe_load((ROOT/'.github/workflows/repo-validation.yml').read_text())
        checkout=next(
            step for step in workflow['jobs']['software']['steps']
            if 'sparse-checkout' in step.get('with',{})
        )
        sparse_paths=set(checkout['with']['sparse-checkout'].splitlines())
        self.assertTrue({f'/{path}' for path in required_tests | required_fixtures} <= sparse_paths)

    def test_rust_sparse_checkout_includes_exact_native_conformance_sources(self):
        required_sources = {
            'tests/test_bibliographic_claim_assembler.py',
            'tests/test_source_agent_publication.py',
            'tests/test_source_catalog_projection.py',
            'tests/test_source_catalog_slots.py',
            'tests/test_source_claim_publication.py',
            'tests/test_source_witness_bibliographic_graph.py',
            'access/tests/source_assembly_fixture.py',
            'access/tests/test_indexed_lens.py',
            'ToS/source-witnesses/artifacts/old-babylonian/uncertain/penn-cbs-07771/artifact-witness.json',
            'ToS/source-witnesses/artifacts/old-babylonian/uncertain/penn-cbs-07771/rights.json',
            'ToS/source-witnesses/discovery/runs/old-babylonian-gilgamesh-cbs7771.2026-08-22.v1.json',
            'ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.json',
            'ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/collection.human-forms.json',
            'ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/membership-claims.jsonl',
            'ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/responsibility-claims.jsonl',
            'ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/source-revision-history.json',
            'ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/work-boundary-map.json',
            'ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/structure/work-boundaries/anchors.jsonl',
            'ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-fafe3ef84b65b29968511c6a06ff018e46571d11c604ec14fc5ef5018f70c1b2/',
            'ToS/source-witnesses/.record-revisions/20d58ef2b14655526fac62cc34f8d84c72126453d2f171317ed6c4114dfd107a-d34e996729a4bb5b04a3a6cc486f2e1ef7e020747fc18afcf10e5ab7628feb73/',
            'ToS/source-witnesses/.metadata-transactions/64289eef67ba46afc5338aff9422484b17707f15796a402a947117d7f0eb7583/',
            'ToS/source-witnesses/.metadata-transactions/ea6aab06fd43ea735791de0ae5e973950f67900dfd9b70a6af1fee853673705c/',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/work.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json',
            'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json',
            'ToS/source-witnesses/places/chemnitz/place.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/zur-genealogie-der-moral/work.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/der-fall-wagner/work.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/goetzen-daemmerung/work.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/der-antichrist/work.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/ecce-homo/work.json',
            'ToS/source-witnesses/relations/mysl-1996-volume-2-member-order/source-claims.jsonl',
            'ToS/review-ledger/2026-09-10-mysl-collection-order-source-reading.md',
        }
        workflow = yaml.safe_load((ROOT / '.github/workflows/repo-validation.yml').read_text())
        checkouts = [
            step for step in workflow['jobs']['rust']['steps']
            if step.get('name') == 'Checkout Rust sources and validation route'
        ]
        self.assertEqual(len(checkouts), 1)
        sparse_paths = set(checkouts[0]['with']['sparse-checkout'].splitlines())
        self.assertTrue({f'/{path}' for path in required_sources} <= sparse_paths)
        self.assertIn('!/ToS/source-witnesses/**/payload/', sparse_paths)
        self.assertNotIn('/ToS/source-witnesses/', sparse_paths)
        self.assertNotIn('/ToS/', sparse_paths)


if __name__ == '__main__':
    unittest.main()

"""Explicit Python reference oracle and workflow topology contracts.

Native software_ci controls are retained separately; these historical
assertions do not establish production execution or retirement by themselves.
"""
from __future__ import annotations

import json
import re
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
                plan = ci.reference_select(paths)
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
                plan = ci.reference_select([path])
                self.assertEqual((plan['software_mode'], plan['worker']), ('full', True))
                self.assertTrue(plan['rust'])
        for paths, full in [([], False), (['README.md'], True)]:
            self.assertEqual(ci.reference_select(paths, full)['software_mode'], 'full')

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
            paths = ci.reference_changed_paths(root, base)
            self.assertEqual(paths, ['README.md', 'access/src/tos_access/core.py'])
            self.assertEqual(ci.reference_select(paths)['software_mode'], 'reader')

    def test_required_gate_rejects_failed_cancelled_missing_and_unexpected_skips(self):
        for mode, worker, rust in [('none', False, False), ('none', False, True), ('browser', False, False), ('reader', True, False), ('full', True, True), ('none', True, False)]:
            needs = {'plan': {'result':'success', 'outputs': {'software_mode':mode, 'worker':str(worker).lower(), 'rust':str(rust).lower()}},
                     'software': {'result':'skipped' if mode == 'none' else 'success'},
                     'worker': {'result':'success' if worker else 'skipped'},
                     'rust': {'result':'success' if rust else 'skipped'}}
            ci.reference_gate(needs)
            for job in needs:
                for bad in ['failure', 'cancelled', None]:
                    changed = json.loads(json.dumps(needs)); changed[job]['result'] = bad
                    with self.subTest(mode=mode, worker=worker, rust=rust, job=job, bad=bad), self.assertRaises(ValueError):
                        ci.reference_gate(changed)
                changed = json.loads(json.dumps(needs)); del changed[job]
                with self.assertRaises(ValueError):
                    ci.reference_gate(changed)
            if mode != 'none':
                needs['software']['result'] = 'skipped'
                with self.assertRaises(ValueError):
                    ci.reference_gate(needs)
        for outputs in [{}, {'software_mode':'none', 'worker':'maybe'}, {'software_mode':'typo', 'worker':'false'}]:
            with self.assertRaises(ValueError):
                ci.reference_gate({'plan': {'result':'success', 'outputs': outputs}})

    def test_document_links_check_new_repo_targets_without_fetching_external_urls(self):
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw)
            subprocess.run(['git','init',str(root)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            (root/'README.md').write_text('[missing](absent.md)\n[web](https://example.invalid/page)\n')
            errors=ci.reference_check_docs(root,'HEAD',['README.md'])
            self.assertEqual(len(errors),1)
            self.assertIn('absent.md',errors[0])
            (root/'absent.md').write_text('exists\n')
            self.assertEqual(ci.reference_check_docs(root,'HEAD',['README.md']),[])
            (root/'README.md').write_text('<<<<<<< branch\n')
            self.assertIn('merge marker',ci.reference_check_docs(root,'HEAD',['README.md'])[0])

    def test_fenced_examples_and_reference_links(self):
        self.assertEqual(ci.reference_links('```md\n[x](fake.md)\n```\n[x](real.md#part)\n[r]: other.md\n'), {'real.md#part','other.md'})

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
        native_owner = next(s for s in jobs['rust']['steps']
                            if s.get('run', '').startswith('python scripts/validation_lanes.py --run rust_workspace'))
        self.assertIn('--lane-timeout-ms 5400000', native_owner['run'])
        native_owner_path = '${{ runner.temp }}/cargo-target/debug/tos-native-owner-command'
        self.assertEqual(native_owner['env']['TOS_NATIVE_OWNER_COMMAND_PATH'], native_owner_path)
        self.assertEqual(native_owner['env']['TOS_NATIVE_OWNER_COMMAND_BIN'], native_owner_path)
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
            run = steps[bind]['run']
            self.assertIn('executor-bind --repo-root', run)
            self.assertEqual(steps[bind]['env']['TOS_CI_EXECUTOR_SHA256'], '${{ needs.plan.outputs.executor_sha256 }}')
            self.assertEqual(steps[bind]['env']['TOS_CI_MANIFEST_SHA256'], '${{ needs.plan.outputs.executor_manifest_sha256 }}')
            self.assertLess(run.index('sha256sum --check --status'), run.index('"$root/tos-software-ci" executor-bind'))
            self.assertNotIn('PY_OPS', run)
        self.assertIn('executor-manifest', plan_runs[prepare])
        self.assertIn('--message-format=json', plan_runs[prepare])
        self.assertIn('--github-output "$GITHUB_OUTPUT"', plan_runs[prepare])
        native_receipts = next(step['run'] for step in jobs['software']['steps'] if 'software-receipts' in step.get('run', ''))
        self.assertEqual(native_receipts.count('--message-format=json'), 4)
        self.assertNotIn('PY_RECEIPT', native_receipts)
        self.assertIn('software-limits --repo-root', native_package[0]['run'])
        self.assertNotIn('PY_LIMITS', native_package[0]['run'])
        gate_steps=jobs['required_gate']['steps']
        self.assertEqual(gate_steps[-1]['run'],'python scripts/software_ci.py gate')
        self.assertEqual(gate_steps[-1]['env']['CI_NEEDS'],'${{ toJSON(needs) }}')

    def test_edge_sql_consumers_use_the_pinned_native_access_product(self):
        workflow=yaml.safe_load((ROOT/'.github/workflows/repo-validation.yml').read_text())
        software_checkout=next(
            step for step in workflow['jobs']['software']['steps']
            if 'sparse-checkout' in step.get('with',{})
        )
        self.assertIn('/.github/workflows/cloudflare-edge.yml',
                      software_checkout['with']['sparse-checkout'].splitlines())
        worker_steps=workflow['jobs']['worker']['steps']
        prepare=next(step for step in worker_steps if step.get('name')=='Prepare pinned Worker rules')
        self.assertIn(
            'cargo +1.98.1 build --locked -p tos-access --bin tos-access --target x86_64-unknown-linux-gnu',
            prepare['run'],
        )
        test=next(step for step in worker_steps if step.get('name')=='Test Worker contracts without deployment or production data')
        self.assertEqual(
            test['env']['TOS_ACCESS_BIN'],
            '${{ runner.temp }}/worker-cargo-target/x86_64-unknown-linux-gnu/debug/tos-access',
        )
        self.assertIn('test -x "$TOS_ACCESS_BIN"',test['run'])

        historical=yaml.safe_load((ROOT/'.github/workflows/cloudflare-edge.yml').read_text())
        event=historical.get('on',historical.get(True))['workflow_dispatch']
        build_seconds=event['inputs']['build_seconds']
        self.assertTrue(build_seconds['required'])
        self.assertEqual(build_seconds['type'],'number')
        self.assertNotIn('default',build_seconds)
        job=historical['jobs']['contract']
        self.assertEqual(job['timeout-minutes'],120)
        steps=job['steps']
        native=next(step for step in steps if step.get('name')=='Prepare pinned native access product')
        self.assertIn(
            'cargo +1.98.1 build --locked -p tos-access --bin tos-access --target x86_64-unknown-linux-gnu',
            native['run'],
        )
        self.assertIn('TOS_ACCESS_BIN=',native['run'])
        self.assertIn('TOS_BUILD_MAX_SECONDS=',native['run'])
        check=next(step for step in steps if step.get('name')=='Build and check Worker')
        self.assertLess(steps.index(native),steps.index(check))
        self.assertIn('npm run check',check['run'])
        wasm=next(step for step in steps if step.get('name')=='Prepare pinned Worker rules WASM')
        self.assertIn(
            'rustup toolchain install 1.98.1 --profile minimal --target wasm32-unknown-unknown',
            wasm['run'],
        )
        self.assertIn(
            'cargo +1.98.1 build --locked --release -p tos-web-rules --features wasm --target wasm32-unknown-unknown',
            wasm['run'],
        )
        self.assertIn('b51f0208fdff83515a787bd8ab9ac5865ed84dabb66d0c709957bb59793c645f',wasm['run'])
        self.assertIn('--target web --out-name tos_web_rules --out-dir generated',wasm['run'])
        self.assertLess(steps.index(native),steps.index(wasm))
        self.assertLess(steps.index(wasm),steps.index(check))
        self.assertIn('npx wrangler deploy --dry-run',steps[-1]['run'])

    def test_acquisition_custody_tests_are_in_required_software_validation(self):
        required_tests={
            'tests/test_acquisition_batch.py',
            'tests/test_acquisition_handoff_adapter.py',
        }
        required_fixtures={
            'tests/oracles/acquisition/acquisition_batch.py',
            'tests/oracles/acquisition/acquisition_handoff_adapter.py',
            'tests/oracles/acquisition/source_payload_custody.py',
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

    def test_rust_sparse_checkout_includes_exact_rust_test_inputs(self):
        required_sources = {
            'QUESTBOOK.md',
            'mechanics/agon/parts/threshold-intake/schemas/tos-agon-threshold-intake.schema.json',
            'mechanics/agon/parts/threshold-registry/config/tos_agon_threshold_intakes.config.json',
            'mechanics/agon/parts/threshold-registry/generated/tos_agon_threshold_intake_registry.min.json',
            'mechanics/agon/parts/threshold-registry/schemas/tos-agon-threshold-intake-registry.schema.json',
            'mechanics/experience/parts/adoption-boundary/examples/tos_adoption_boundary_dossier.example.json',
            'mechanics/experience/parts/adoption-boundary/examples/tos_no_runtime_adoption_guard.example.json',
            'mechanics/experience/parts/adoption-boundary/schemas/tos_adoption_boundary_dossier_v1.json',
            'mechanics/experience/parts/adoption-boundary/schemas/tos_no_runtime_adoption_guard_v1.json',
            'mechanics/experience/parts/candidate-review/examples/aoa_experience_candidate_dossier.example.json',
            'mechanics/experience/parts/candidate-review/examples/tos_intake_boundary_decision.example.json',
            'mechanics/experience/parts/candidate-review/schemas/aoa_experience_candidate_dossier_v1.json',
            'mechanics/experience/parts/candidate-review/schemas/tos_intake_boundary_decision_v1.json',
            'mechanics/experience/parts/governance-boundary/examples/tos_governance_dossier_boundary_v1.example.json',
            'mechanics/experience/parts/governance-boundary/examples/tos_governance_review_note.example.json',
            'mechanics/experience/parts/governance-boundary/schemas/tos_governance_dossier_boundary_v1.json',
            'mechanics/experience/parts/governance-boundary/schemas/tos_governance_review_note_v1.json',
            'mechanics/experience/parts/installation-boundary/examples/tos_installation_dossier_boundary_v1.example.json',
            'mechanics/experience/parts/installation-boundary/schemas/tos_installation_dossier_boundary_v1.json',
            'mechanics/experience/parts/pattern-review/examples/tos_pattern_review_note.example.json',
            'mechanics/experience/parts/pattern-review/schemas/tos_pattern_review_note_v1.json',
            'mechanics/experience/parts/service-office-boundary/examples/tos_no_runtime_office_write_guard_v1.example.json',
            'mechanics/experience/parts/service-office-boundary/examples/tos_service_dossier_boundary_v1.example.json',
            'mechanics/experience/parts/service-office-boundary/schemas/tos_no_runtime_office_write_guard_v1.json',
            'mechanics/experience/parts/service-office-boundary/schemas/tos_service_dossier_boundary_v1.json',
            'mechanics/experience/parts/write-guards/examples/tos_no_direct_write_guard.example.json',
            'mechanics/experience/parts/write-guards/schemas/tos_no_direct_write_guard_v1.json',
            'mechanics/questbook/parts/dispatch-contracts/examples/quest_catalog.min.example.json',
            'mechanics/questbook/parts/dispatch-contracts/examples/quest_dispatch.min.example.json',
            'mechanics/questbook/parts/dispatch-contracts/schemas/quest.schema.json',
            'mechanics/questbook/parts/dispatch-contracts/schemas/quest_dispatch.schema.json',
            'mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md',
            'quests/TOS-Q-0001.yaml',
            'quests/TOS-Q-0002.yaml',
            'quests/TOS-Q-0003.yaml',
            'quests/TOS-Q-0004.yaml',
            'ToS/candidate-intake/AGENTS.md',
            'ToS/candidate-intake/zarathustra/concept-workbench-v1/word-analysis-task.v1.schema.json',
            'ToS/candidate-intake/zarathustra/concept-workbench-v1/english-translation-candidate.v1.schema.json',
            'ToS/candidate-intake/zarathustra/concept-workbench-v1/plan.v1.json',
            'ToS/candidate-intake/thus-spoke-zarathustra/prologue-1/mode-b/edges.csv',
            'ToS/canon/**/node.human-forms.json',
            'ToS/canon/**/node.json',
            'ToS/canon/AGENTS.md',
            'ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv',
            'ToS/derived-exports/AGENTS.md',
            'ToS/doctrine/AGENTS.md',
            'ToS/philosophy/AGENTS.md',
            'ToS/philosophy/philosophy.manifest.json',
            'ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json',
            'ToS/public-compatibility/AGENTS.md',
            'ToS/public-compatibility/source_node.example.json',
            'ToS/research-packets/AGENTS.md',
            'ToS/research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md',
            'ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-a-occurrences-only.json',
            'ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-b-competing-sign-proposals.json',
            'ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-c-invalid-model-promotion.json',
            'ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-a-one-to-one-proposal.json',
            'ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-b-competing-mappings.json',
            'ToS/research-packets/foundation-laboratory-2026-07/translation-alignment-v1-abc/variant-c-invalid-acceptance.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-a.anchor.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-b.anchor.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-c.anchor.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/variant-b-unicode.txt',
            'ToS/research-packets/foundation-laboratory-2026-07/source-anchor-v2-abc/lab.manifest.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-a.layer.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-b.layer.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-c.layer.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-a-raw-ocr.txt',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/variant-b-diplomatic.txt',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-layer-abc/editorial-policy.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-a-source-layout-observation.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-b-competing-segmentations.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/variant-c-invalid-acceptance.json',
            'ToS/research-packets/foundation-laboratory-2026-07/source-text-unit-v1-abc/public-synthetic-source.x-tos-unit.txt',
            'ToS/review-ledger/AGENTS.md',
            'ToS/source-witnesses/.record-revisions/2c4c3a4f5cb2cbf1713ebdaa0b27dfcb0729cf980f33591a6e1e2ea6296b8d25-f63f2f0562a6a662be9c5340ddde5686a6de35a8991ad2ad7b53e3a8fd134eba/',
            'ToS/source-witnesses/.record-revisions/3b6ca195bb9bb9fb57cc1e0d9bece8b18011ef12c3d99d614aac5fa3760ad712-8d38fda8bf756906f8ed3543a8cc069082d39b04b188db5050d76a2ad663a497/',
            'ToS/source-witnesses/.record-revisions/709df7fb307a1331d25fa253f7159a3ee27b3862898ddf8db74c6cbfc6965438-75afe571bb0254a738c20c0d5d09fac8b11ce0f299068b534ef652ef09575422/',
            'ToS/source-witnesses/AGENTS.md',
            'ToS/source-witnesses/agents/constantin-georg-naumann/',
            'ToS/source-witnesses/documents/friedrich-nietzsche/naumann-letter-705/',
            'ToS/source-witnesses/links/internet-archive/onfoursongsconta00good/landing/link.human-forms.json',
            'ToS/source-witnesses/links/internet-archive/onfoursongsconta00good/landing/link.json',
            'ToS/source-witnesses/relations/nietzsche-letter-705-addressee/',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/rights.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/editions/saint-petersburg-prometey-1911-fourth/items/rsl-neb-scan-pdf/rights.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/expression.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/ru-antonovsky-1911/responsibility-claims.jsonl',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/edition-reading-admission.dta-ekgwb.za-i-vorrede-1.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/initial-sign-packet.v5.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/provenance.opening-sentence-alignment.za-i-vorrede-1.v2.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-layer.za-i-vorrede-1-p1.antonovsky-1911-embedded.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-layer.za-i-vorrede-1-p1.dta-machine.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.antonovsky-1911-layout.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.antonovsky-1911-sentence-proposal.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.dta-layout.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/source-text-unit.za-i-vorrede-1-p1.dta-sentence-proposal.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/translation-alignment.za-i-vorrede-1-opening-sentence.dta-1883-antonovsky-1911.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/za-i-vorrede-1-opening-sentence-alignment.plan.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/transfer-candidate-page-crosswalk.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-samples.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/transfer-target-anchors.v1.jsonl',
            'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/de-naumann-1886/editions/leipzig-c-g-naumann-1886/items/internet-archive-google-harvard-scan-pdf/rights.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/ru-polilov-mysl-1996/structure/mysl-1996-volume-2-operator-pdf/numbered-unit-page-map.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/alignments/structure/naumann-1886-polilov-mysl-1996/numbered-unit-label-correspondence.json',
            'ToS/source-witnesses/collections/friedrich-nietzsche/works-in-two-volumes-volume-2-mysl-1996/editions/moscow-mysl-1996-volume-2/items/operator-pdf/rights.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.human-forms.json',
            'ToS/source_home.manifest.json',
            'ToS/zarathustra/AGENTS.md',
            'access/tests/fixtures/knowledge-contract/ToS/source-witnesses/semantic-descriptions/crosscutting-concept-freedom/crosscutting-concept.human-forms.json',
            'access/tests/fixtures/source-assembly/ToS/source-witnesses/agents/friedrich-nietzsche/agent.json',
            'access/tests/fixtures/source-assembly/ToS/source-witnesses/places/chemnitz/place.json',
            'access/tests/fixtures/source-assembly/ToS/source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/work.json',
            'tests/test_source_owner_claim_profiles.py',
            'tests/test_source_owner_record_profiles.py',
            'tests/test_bibliographic_claim_assembler.py',
            'tests/test_source_agent_publication.py',
            'tests/test_source_catalog_projection.py',
            'tests/test_source_catalog_slots.py',
            'tests/test_source_claim_publication.py',
            'tests/test_source_witness_bibliographic_graph.py',
            'access/tests/source_assembly_fixture.py',
            'access/tests/test_indexed_lens.py',
            'access/tests/source_agent_publication_fixture.py',
            'access/tests/test_source_metadata_publication.py',
            'tests/test_native_text_binding.py',
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
            'ToS/source-witnesses/research-corpora/foundation-source-routes/research-corpus.json',
            'ToS/source-witnesses/artifacts/old-babylonian/susa/hammurabi-stele-sb-8/artifact-witness.json',
            'ToS/source-witnesses/agents/erasmus-of-rotterdam/agent.json',
            'ToS/source-witnesses/links/cdli/cdlb-2006-1/article/link.json',
            'ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/work.json',
            'ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/expression.json',
            'ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-claims.jsonl',
            'ToS/source-witnesses/relations/oim-a00645-physical-composition/source-claims.jsonl',
            'ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645-plus-a00649a-i/artifact-witness.json',
            'ToS/source-witnesses/artifacts/sumerian/adab/oim-a00645/artifact-witness.json',
            'ToS/source-witnesses/discovery/DISCOVERY_PROTOCOL.md',
            'ToS/source-witnesses/discovery/provenance.jsonl',
            'ToS/source-witnesses/discovery/runs/zarathustra-parts-2-3-provision-identity.2026-08-01.v1.json',
            'ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-2/editions/chemnitz-schmeitzner-1883-part-2/items/dta-sbb-corrected-tei-p5/source-metadata-snapshot.json',
            'ToS/research-packets/foundation-laboratory-2026-07/ZARATHUSTRA_PARTS_2_3_PROVISION_IDENTITY_RESEARCH.md',
        }
        generic_xml_sources = (
            ROOT / 'rust/crates/tos-compiler/tests/generic_xml_uxlc_lab.rs',
            ROOT / 'rust/crates/tos-compiler/tests/generic_xml_uxlc_lab/inputs.rs',
        )
        generic_xml_fixture_root = (
            'ToS/research-packets/foundation-laboratory-2026-07/'
            'generic-xml-resource-inventory-uxlc-abc-v1/'
        )
        include_literal = re.compile(r'include_(?:bytes|str)!\s*\(\s*"([^"]+)"\s*\)', re.S)
        generic_xml_inputs = set()
        for source in generic_xml_sources:
            for literal in include_literal.findall(source.read_text()):
                if 'generic-xml-resource-inventory-uxlc-abc-v1' not in literal:
                    continue
                resolved = (source.parent / literal).resolve()
                relative = resolved.relative_to(ROOT).as_posix()
                self.assertTrue(relative.startswith(generic_xml_fixture_root), relative)
                self.assertTrue(resolved.is_file(), relative)
                generic_xml_inputs.add(relative)
        self.assertEqual(len(generic_xml_inputs), 58)
        required_sources.update(generic_xml_inputs)
        for source in (ROOT / 'tests/conformance/rust').glob('*.rs'):
            text = source.read_text()
            self.assertNotIn('Command::new("/usr/bin/python3")', text, source.name)
            self.assertNotIn('Command::new("python3")', text, source.name)
        self.assertIn('var_os("TOS_MAINTAINED_PYTHON")',
                      (ROOT / 'tests/conformance/rust/runner.rs').read_text())
        workflow = yaml.safe_load((ROOT / '.github/workflows/repo-validation.yml').read_text())
        def checkout_paths(job, step_name):
            checkouts = [
                step for step in workflow['jobs'][job]['steps']
                if step.get('name') == step_name and 'sparse-checkout' in step.get('with', {})
            ]
            self.assertEqual(len(checkouts), 1, job)
            return set(checkouts[0]['with']['sparse-checkout'].splitlines())

        sparse_paths = checkout_paths('rust', 'Checkout Rust sources and validation route')
        software_sparse_paths = checkout_paths('software', 'Checkout software and bounded test fixtures')
        worker_sparse_paths = checkout_paths('worker', 'Checkout Worker software')
        software_schemas = {
            'ToS/candidate-intake/zarathustra/concept-workbench-v1/word-analysis-task.v1.schema.json',
            'ToS/candidate-intake/zarathustra/concept-workbench-v1/english-translation-candidate.v1.schema.json',
        }
        worker_schemas = software_schemas | {
            'ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-search-result.v1.schema.json',
            'ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-request.v2.schema.json',
            'ToS/candidate-intake/zarathustra/reading-workbench-v1/reading-search-result.v1.schema.json',
            'ToS/doctrine/semantic-interchange/query-vocabulary.v1.json',
        }
        shared_compiled_source_inputs = {
            'ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json',
        }
        for path in software_schemas | worker_schemas | shared_compiled_source_inputs:
            self.assertTrue((ROOT / path).is_file(), path)
        self.assertTrue({f'/{path}' for path in required_sources} <= sparse_paths)
        self.assertTrue({f'/{path}' for path in software_schemas} <= software_sparse_paths)
        self.assertTrue({f'/{path}' for path in generic_xml_inputs} <= software_sparse_paths)
        self.assertTrue({f'/{path}' for path in worker_schemas} <= worker_sparse_paths)
        for checkout in (sparse_paths, software_sparse_paths, worker_sparse_paths):
            self.assertTrue({f'/{path}' for path in shared_compiled_source_inputs} <= checkout)
        self.assertIn('!/ToS/source-witnesses/**/payload/', sparse_paths)
        for checkout in (sparse_paths, software_sparse_paths, worker_sparse_paths):
            self.assertNotIn('/ToS/source-witnesses/', checkout)
            self.assertNotIn('/ToS/', checkout)
            self.assertNotIn('/ToS/candidate-intake/', checkout)
        self.assertNotIn(f'/{generic_xml_fixture_root}', sparse_paths)
        self.assertNotIn(f'/{generic_xml_fixture_root}', software_sparse_paths)


if __name__ == '__main__':
    unittest.main()

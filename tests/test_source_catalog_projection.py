"""Real small source transactions and catalog bytes, never source admission."""
from dataclasses import replace
import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
for directory in (ROOT / 'scripts', ROOT / 'access/src', ROOT / 'mechanics/growth-cycle/tests',
                  ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'):
    sys.path.insert(0, str(directory))

import build_source_witness_catalog as legacy
import source_catalog_projection as catalog
import source_commands as commands
import source_metadata_snapshot as publication
import test_source_revisions as revision_tests
from tos_access.projection_mutation import MutationLimits, ProjectionSnapshotView, ProjectionMutationError
from tos_access.projection_store import ProjectionReader, canonical_bytes


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


class SourceCatalogProjectionTests(unittest.TestCase):
    def setUp(self):
        self.fixture = revision_tests.NativeSourceRevisionTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.record = copy.deepcopy(self.fixture.record)
        self.identity = self.record['record_id']
        self.relative = self.fixture.relative
        self.fixture.config['schema_version'] = commands.CORPUS_SELECTED_REVISION_CONFIG
        self.fixture.owner.write_text(json.dumps(self.fixture.config))
        self.other = {**self.record, 'record_id': 'tos.agent.untouched-fixture',
                      'preferred_label': 'Untouched synthetic Agent', 'notes': 'Different identity.'}
        self.other_ref = 'ToS/source-witnesses/agents/untouched/agent.json'
        self.write(self.other_ref, canonical_bytes(self.other))
        historical = {**self.record, 'schema_version': 'tos_historical_record_v1',
                      'record_type': 'historical-event', 'record_id': 'tos.historical-event.catalog-fixture',
                      'visibility': 'public_metadata_only'}
        self.write('ToS/source-witnesses/history/catalog-fixture/episode/historical-event.json', canonical_bytes(historical))
        self.output = self.root / 'derived/catalog-v2.json'
        self.output.parent.mkdir()
        self.scratch = self.root / 'scratch'
        self.scratch.mkdir()
        self.rebuild()

    def write(self, ref, raw):
        path = self.root / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)

    def rebuild(self):
        outputs = legacy.render_outputs(self.root)
        legacy.write_outputs(self.root, outputs)

    def bootstrap(self, **kwargs):
        return catalog.bootstrap_source_catalog(self.root, self.output,
            catalog_namespace='tos.catalog.synthetic.records',
            expected_manifest_sha256=sha((self.root / legacy.MANIFEST_PATH).read_bytes()),
            expected_publication_token=publication.PublicationSnapshot(self.root).token,
            work_dir=self.scratch, target_part_bytes=256, **kwargs)

    def reader(self, candidate, **kwargs):
        return catalog.SourceCatalogSnapshot(candidate.snapshot(), expected_root_sha256=candidate.root_sha256,
            trusted_baseline_sha256=candidate.root_sha256, **kwargs)

    def revise(self, label='Revised fixture note', *, command_id='synthetic:catalog-revision'):
        proposal = {'fields': {'notes': label}, 'forms': self.fixture.selections,
                    'reason': 'Synthetic addressed catalog transition, not source acceptance.'}
        preview = self.fixture.run_command('prepare-revise', **proposal)
        result = self.fixture.run_command('record.revise', command_id=command_id,
            expected_source=preview['source'], expected_revision=preview['revision'],
            expected_configuration=preview['owner_configuration'], expected_dependencies=preview['expected_dependencies'],
            expected_publication=preview['publication_snapshot'], **proposal)
        return result['receipt']['publication']['transaction_id'], result['publication_snapshot']

    def transition(self, before, transaction_id, token, **kwargs):
        return catalog.stage_agent_catalog_transition(self.root, before, transaction_id=transaction_id,
            expected_publication_token=token, target_part_bytes=256, **kwargs)

    def test_bootstrap_full_owner_parity_without_selecting_target_root(self):
        original = {ref: (self.root / ref).read_bytes() for ref in legacy.render_outputs(self.root)}
        candidate = self.bootstrap()
        self.assertFalse(self.output.exists())
        reader = self.reader(candidate)
        rows = candidate.snapshot().materialize()['records']
        expected = {entry['record_id']: entry for entries in legacy.collect_records(self.root).values() for entry in entries}
        self.assertEqual({row['record_id']: row['entry'] for row in rows}, expected)
        self.assertEqual(reader.header['record_count'], 3)
        row = reader.get(self.identity)
        self.assertEqual(row.source['raw_sha256'], sha(self.fixture.path.read_bytes()))
        self.assertEqual(row.source['record_ref'], commands.metadata_subject(self.record).ref)
        self.assertNotEqual(row.source['raw_sha256'], row.source['record_ref']['digest'][7:])
        catalog._schema('address').validate(row.provenance)
        self.assertEqual(row.provenance['row_sha256'], sha(row.row_bytes))
        for field in ('root_sha256', 'publication_token', 'epoch', 'catalog_sha256', 'line'):
            self.assertNotIn(field, row.provenance)
        self.assertEqual({ref: (self.root / ref).read_bytes() for ref in original}, original)
        self.assertFalse(candidate.published)
        self.assertFalse(candidate.establishes_epoch)
        self.assertFalse(candidate.source_reference_closure_verified)
        self.assertEqual(candidate.verification['mode'], 'explicit-full-bootstrap')
        self.assertFalse(any(ref.endswith('catalog-v2.json') for ref in candidate.created_parts))

    def test_accessor_is_addressed_budgeted_detached_and_not_currentness(self):
        candidate = self.bootstrap()
        with patch.object(ProjectionReader, '__init__', side_effect=AssertionError('selected root opened')), \
                patch.object(ProjectionReader, 'iter_items', side_effect=AssertionError('full projection scan')), \
                patch.object(ProjectionReader, 'materialize', side_effect=AssertionError('full materialization')), \
                patch.object(Path, 'rglob', side_effect=AssertionError('source or namespace scan')):
            reader = self.reader(candidate)
            row = reader.get(self.identity)
            self.assertIsNone(reader.lookup('tos.agent.not-in-snapshot'))
            with self.assertRaises(catalog.SourceCatalogError):
                reader.require_current()
        detached = row.entry
        detached['links'].clear()
        detached['preferred_label'] = 'Not stored'
        self.assertEqual(reader.get(self.identity).entry['preferred_label'], self.record['preferred_label'])
        with self.assertRaises(ProjectionMutationError):
            self.reader(candidate, limits=replace(MutationLimits(), max_decoded_bytes=0))
        with self.assertRaises(catalog.SourceCatalogError):
            catalog.SourceCatalogSnapshot(candidate.snapshot(), expected_root_sha256=candidate.root_sha256,
                trusted_baseline_sha256='0' * 64)

    def test_bootstrap_preserves_existing_target_and_refuses_private_or_symlink_source(self):
        self.output.write_bytes(b'existing separately selected root')
        self.bootstrap()
        self.assertEqual(self.output.read_bytes(), b'existing separately selected root')
        forbidden = 'ToS/source-witnesses/private/fixture/agent.json'
        self.write(forbidden, canonical_bytes(self.other))
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'outside exact public metadata'):
            self.bootstrap()
        (self.root / forbidden).unlink()
        target = self.root / self.other_ref
        raw = target.read_bytes()
        retained = target.with_name('retained-bytes.json')
        retained.write_bytes(raw)
        target.unlink()
        target.symlink_to(retained)
        with self.assertRaises(OSError):
            self.bootstrap()
        self.assertEqual(self.output.read_bytes(), b'existing separately selected root')

    def test_accessor_rejects_malformed_row_and_missing_execution_binding(self):
        baseline = self.bootstrap()
        row = self.reader(baseline).get(self.identity)
        from tos_access.projection_mutation import ProjectionChange, stage_projection_snapshot_changes
        malformed = json.loads(row.row_bytes)
        malformed['source']['record_ref']['id'] = self.other['record_id']
        changed = stage_projection_snapshot_changes(baseline.snapshot(), expected_before_sha256=baseline.root_sha256,
            trusted_baseline_sha256=baseline.root_sha256,
            changes=[ProjectionChange('records', self.identity, True, row.row_sha256, True, malformed)])
        reader = catalog.SourceCatalogSnapshot(changed.snapshot(), expected_root_sha256=changed.after_sha256,
            trusted_baseline_sha256=changed.after_sha256)
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'identities or source bindings'):
            reader.get(self.identity)
        manifest = json.loads(baseline.root_bytes)
        manifest['header']['profile_bindings']['execution'].pop(catalog.CONTRACT)
        view = ProjectionSnapshotView(canonical_bytes(manifest), self.output)
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'required profile bindings'):
            catalog.SourceCatalogSnapshot(view, expected_root_sha256=view.snapshot_digest,
                trusted_baseline_sha256=view.snapshot_digest)

    def test_transition_refuses_non_agent_plan_and_tampered_retained_output(self):
        baseline = self.bootstrap()
        transaction_id, token = self.revise()
        retained = catalog.transactions.inspect_transaction(self.root, transaction_id)
        for change in ('foreign-kind', 'deleted-after', 'wrong-fields'):
            fake = copy.deepcopy(retained)
            if change == 'foreign-kind':
                fake['plan']['authorization']['record_type'] = 'place'
            elif change == 'deleted-after':
                fake['plan']['files'][0]['after'] = None
            else:
                fake['plan']['authorization']['request']['fields']['identity_status'] = 'verified'
            with self.subTest(change=change), patch.object(catalog.transactions, 'inspect_transaction', return_value=fake), \
                    patch.object(catalog, 'stage_projection_snapshot_changes') as stage:
                with self.assertRaises(catalog.SourceCatalogError):
                    self.transition(self.reader(baseline), transaction_id, token)
                stage.assert_not_called()

    def test_native_renderer_is_pure_and_full_builder_uses_it(self):
        with patch.object(Path, 'open', side_effect=AssertionError('pure renderer opened source')):
            entry = legacy.render_native_catalog_entry(self.record, self.relative)
        self.assertEqual(entry['record_sha256'], sha(legacy.canonical_json(self.record).encode()))
        with patch.object(legacy, 'render_native_catalog_entry', wraps=legacy.render_native_catalog_entry) as renderer:
            records = legacy.collect_records(self.root)
        self.assertEqual(next(row for row in records['agent'] if row['record_id'] == self.identity), entry)
        self.assertEqual(renderer.call_count, 2)

    def test_bootstrap_refuses_stale_source_missing_member_and_invalid_native_schema(self):
        cases = ('stale-row', 'missing-source', 'invalid-schema')
        for case in cases:
            original = self.fixture.path.read_bytes()
            with self.subTest(case=case), patch.object(catalog, '_install') as install:
                if case == 'stale-row':
                    self.fixture.path.write_bytes(canonical_bytes({**self.record, 'notes': 'untracked drift'}))
                elif case == 'missing-source':
                    self.fixture.path.unlink()
                else:
                    self.fixture.path.write_bytes(canonical_bytes({**self.record, 'unrecognized-field': True}))
                    self.rebuild()  # Exact catalog parity cannot replace the native schema check.
                with self.assertRaises((catalog.SourceCatalogError, commands.ValidationError)):
                    self.bootstrap()
                install.assert_not_called()
            self.fixture.path.write_bytes(original)
            self.rebuild()

    def test_bootstrap_refuses_duplicate_id_and_midflight_membership_change(self):
        extra = 'ToS/source-witnesses/agents/extra/agent.json'
        self.write(extra, canonical_bytes(self.record))
        with self.assertRaises(legacy.CatalogBuildError):
            self.bootstrap()
        (self.root / extra).unlink()
        real = legacy.collect_records
        def drift(*args, **kwargs):
            result = real(*args, **kwargs)
            self.write(extra, canonical_bytes({**self.other, 'record_id': 'tos.agent.extra'}))
            return result
        with patch.object(legacy, 'collect_records', side_effect=drift), patch.object(catalog, '_install') as install:
            with self.assertRaisesRegex(catalog.SourceCatalogError, 'membership changed'):
                self.bootstrap()
            install.assert_not_called()

    def test_bootstrap_explicit_manifest_and_limits_are_not_implicit_fallbacks(self):
        with self.assertRaises(catalog.SourceCatalogError):
            catalog.bootstrap_source_catalog(self.root, self.output, catalog_namespace='tos.catalog.test',
                expected_manifest_sha256='0' * 64, expected_publication_token=None, work_dir=self.scratch)
        for limits in (replace(catalog.CatalogLimits(), max_records=0),
                       replace(catalog.CatalogLimits(), max_source_files=0),
                       replace(catalog.CatalogLimits(), max_input_bytes=0),
                       replace(catalog.CatalogLimits(), max_catalog_bytes=0)):
            with self.subTest(limits=limits), patch.object(catalog, '_install') as install:
                with self.assertRaises(catalog.SourceCatalogBudgetExceeded):
                    self.bootstrap(limits=limits)
                install.assert_not_called()

    def test_real_agent_transition_chains_without_legacy_catalog_reads(self):
        baseline = self.bootstrap()
        first_reader = self.reader(baseline)
        untouched = first_reader.get(self.other['record_id'])
        transaction_id, token = self.revise()
        # A selected transaction intentionally leaves legacy generated files
        # stale. The addressed route must not consult or rebuild them.
        original_read = catalog._Capture.read
        def no_catalog(observer, ref, limit=None):
            if ref.startswith('ToS/source-witnesses/catalog/'):
                self.fail('incremental transition read legacy catalog')
            return original_read(observer, ref, limit)
        with patch.object(catalog._Capture, 'read', new=no_catalog), \
                patch.object(legacy, 'collect_records', side_effect=AssertionError('full collector')), \
                patch.object(legacy, 'collect_claims', side_effect=AssertionError('Claim scan')), \
                patch.object(Path, 'rglob', side_effect=AssertionError('source inventory scan')):
            first = self.transition(first_reader, transaction_id, token)
        next_reader = self.reader(first)
        revised = next_reader.get(self.identity)
        self.assertEqual(revised.source['record_ref']['version'], 2)
        self.assertEqual(revised.source['raw_sha256'], sha(self.fixture.path.read_bytes()))
        self.assertEqual(next_reader.get(self.other['record_id']).provenance, untouched.provenance)
        self.assertEqual(next_reader.get(self.other['record_id']).row_bytes, untouched.row_bytes)
        self.assertFalse(first.source_reference_closure_verified)
        self.assertFalse(self.output.exists())
        second_id, second_token = self.revise('Second exact descriptive revision', command_id='synthetic:catalog-revision-2')
        second = self.transition(next_reader, second_id, second_token)
        self.assertEqual(self.reader(second).get(self.identity).source['record_ref']['version'], 3)
        self.assertEqual(second.before_root_sha256, first.root_sha256)
        self.assertEqual(self.reader(baseline).get(self.identity).source['record_ref']['version'], 1)
        self.rebuild()
        full_entry = next(row for row in legacy.collect_records(self.root)['agent'] if row['record_id'] == self.identity)
        self.assertEqual(self.reader(second).get(self.identity).entry, full_entry)

    def test_transition_refuses_non_immediate_predecessor_and_current_raw_drift(self):
        baseline = self.bootstrap()
        transaction_id, token = self.revise()
        first = self.transition(self.reader(baseline), transaction_id, token)
        second_id, second_token = self.revise('Another revision', command_id='synthetic:next')
        with patch.object(catalog, 'stage_projection_snapshot_changes') as stage:
            with self.assertRaisesRegex(catalog.SourceCatalogError, 'immediate'):
                self.transition(self.reader(baseline), second_id, second_token)
            stage.assert_not_called()
        self.fixture.path.write_bytes(self.fixture.path.read_bytes() + b' ')
        with patch.object(catalog, 'stage_projection_snapshot_changes') as stage:
            with self.assertRaisesRegex(catalog.SourceCatalogError, 'current selected metadata'):
                self.transition(self.reader(first), second_id, second_token)
            stage.assert_not_called()

    def test_transition_refuses_changed_profile_and_wrong_catalog_before_bytes(self):
        baseline = self.bootstrap()
        transaction_id, token = self.revise()
        schema_path = self.root / catalog.CORPUS_REF
        original = schema_path.read_bytes()
        schema_path.write_bytes(original + b'\n')
        with self.assertRaises(catalog.SourceCatalogRequiresBootstrap):
            self.transition(self.reader(baseline), transaction_id, token)
        schema_path.write_bytes(original)
        row = self.reader(baseline).get(self.identity)
        corrupt = json.loads(row.row_bytes)
        corrupt['source']['raw_sha256'] = '0' * 64
        from tos_access.projection_mutation import ProjectionChange, stage_projection_snapshot_changes
        changed = stage_projection_snapshot_changes(baseline.snapshot(), expected_before_sha256=baseline.root_sha256,
            trusted_baseline_sha256=baseline.root_sha256,
            changes=[ProjectionChange('records', self.identity, True, row.row_sha256, True, corrupt)])
        foreign = catalog.SourceCatalogSnapshot(changed.snapshot(), expected_root_sha256=changed.after_sha256,
            trusted_baseline_sha256=changed.after_sha256)
        with self.assertRaisesRegex(catalog.SourceCatalogError, 'predecessor differs'):
            self.transition(foreign, transaction_id, token)


if __name__ == '__main__':
    unittest.main()

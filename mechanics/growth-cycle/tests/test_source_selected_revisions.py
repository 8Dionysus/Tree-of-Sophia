"""Selected native corrections preserve nested source homes and exact recovery.

All subject content is synthetic. These tests exercise the real owner command,
archives, publication transport and independent metadata-version reader.
"""
import copy
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
sys.path.insert(0, str(MECHANIC))
import source_commands as source
import source_revisions as revisions
import source_metadata_transactions as transactions
import metadata_version_reader
from source_metadata_snapshot import PublicationSnapshot, PublicationPending, PublicationChanged
import test_source_revisions as fixtures


class SelectedSourceRevisionTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.NativeSourceRevisionTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        f = self.fixture
        oldpath, oldform = f.path, f.formpath
        f.relative = str(Path(f.relative).with_name('work.json'))
        f.path, f.formpath = f.root / f.relative, (f.root / f.relative).with_name('work.human-forms.json')
        f.record.update(record_type='work', record_id='tos.work.selected-fixture', expression_claim_refs=[])
        f.path.write_bytes(revisions._encode(f.record))
        subject = source.Record.from_payload(f.record['record_id'], 1, f.record)
        forms = source._apply(None, subject, [source.prepare_metadata_change(
            f.record, None, 'test:author', **selection) for selection in f.selections])
        f.formpath.write_bytes(revisions._encode(forms))
        oldpath.unlink()
        oldform.unlink()
        f.config.update(schema_version=source.CORPUS_SELECTED_REVISION_CONFIG,
            record_type='work', record_id=f.record['record_id'], source_path=f.relative,
            allowed_operations=['record.revise', 'record.recover'])
        f.owner.write_bytes(revisions._encode(f.config))
        # Nested material and an unrelated companion are not part of a selected
        # revision, regardless of byte count, shape or filesystem permissions.
        self.nested = f.path.parent / 'expressions' / 'untouched' / 'payload'
        self.nested.mkdir(parents=True)
        (self.nested / 'private.bin').write_bytes(b'opaque synthetic descendant')
        self.before = revisions._selected_package(f.path)
        self.root, self.path = f.root, f.path

    def request(self, identifier='test:selected-correction'):
        request = self.fixture.request(identifier)
        request['expected_publication'] = PublicationSnapshot(self.root).token
        return request

    def run_request(self, request):
        return source.run_local_command(self.fixture.owner, request)

    def recover(self, request, decision):
        config, digest, _ = source._configuration(self.fixture.owner)
        from source_selected_revisions import _transaction_id
        return self.run_request({'schema_version': 'tos_local_source_command_v1',
            'operation': 'record.recover', 'transaction_id': _transaction_id(request),
            'decision': decision, 'expected_configuration': digest})

    def interrupt(self, request):
        original = transactions._replace_file
        def stop_after_first(*args):
            original(*args)
            raise RuntimeError('synthetic interruption after first exact file')
        with patch.object(transactions, '_replace_file', side_effect=stop_after_first):
            with self.assertRaisesRegex(RuntimeError, 'synthetic interruption'):
                self.run_request(request)
        with self.assertRaises(PublicationPending):
            PublicationSnapshot(self.root)

    def sync_catalog(self):
        record = json.loads(self.path.read_bytes())
        catalog = self.root / 'ToS/source-witnesses/catalog'
        catalog.mkdir(exist_ok=True)
        entry = {'schema_version': 'tos_source_witness_catalog_entry_v1',
            'record_id': record['record_id'], 'record_type': 'work',
            'preferred_label': record['preferred_label'], 'identity_status': record['identity_status'],
            'source_record_ref': self.fixture.relative,
            'record_sha256': metadata_version_reader._record_ref(record)['digest'][7:], 'links': {}}
        files = {'ToS/source-witnesses/catalog/works.jsonl': source._canonical(entry) + b'\n',
                 'ToS/source-witnesses/catalog/claims.jsonl': b''}
        manifest = {'schema_version': 'tos_source_witness_catalog_v3',
            'record_files': {'work': 'ToS/source-witnesses/catalog/works.jsonl'},
            'claim_file': 'ToS/source-witnesses/catalog/claims.jsonl'}
        snapshot = PublicationSnapshot(self.root)
        if snapshot.token is not None:
            manifest['selected_metadata_publication'] = {'protocol': revisions.SELECTED_PROTOCOL,
                'token': snapshot.token, 'files': {ref: source._digest(raw)[7:] for ref, raw in files.items()}}
        for ref, raw in files.items():
            (self.root / ref).write_bytes(raw)
        (catalog / 'catalog.manifest.json').write_bytes(source._canonical(manifest))

    def test_nested_work_correction_never_enumerates_or_reads_descendants(self):
        request = self.request()
        original_read, original_iterdir = source._read, Path.iterdir
        def read_selected(path, limit):
            self.assertFalse(path.is_relative_to(self.nested))
            self.assertNotEqual(path, self.path.parent / 'unrecognized.json')
            return original_read(path, limit)
        def no_parent_listing(path):
            self.assertNotEqual(path, self.path.parent)
            return original_iterdir(path)
        with (patch.object(source, '_read', side_effect=read_selected),
              patch.object(Path, 'iterdir', no_parent_listing)):
            result = self.run_request(request)
            previous = self.fixture.run_command('inspect-version', source=request['expected_source'])
        self.assertEqual(previous['record'], self.fixture.record)
        self.assertEqual(set(previous['files']), set(self.before))
        self.assertEqual(json.loads(self.path.read_bytes())['record_version'], 2)
        self.assertTrue(all(view['state'] == 'ready' for view in result['materializations']))
        self.assertEqual((self.nested / 'private.bin').read_bytes(), b'opaque synthetic descendant')
        self.assertEqual(result['publication_protocol'], revisions.SELECTED_PROTOCOL)
        self.assertFalse(result['grants_admission'])

    def test_v1_grant_is_not_widened_to_nested_selected_writes(self):
        self.fixture.config['schema_version'] = source.CORPUS_REVISION_CONFIG
        self.fixture.config['allowed_operations'] = ['record.revise']
        self.fixture.owner.write_bytes(revisions._encode(self.fixture.config))
        with self.assertRaises(PermissionError):
            self.fixture.run_command('describe')
        self.assertEqual(revisions._selected_package(self.path), self.before)
        self.assertIsNone(PublicationSnapshot(self.root).token)

    def test_prepare_final_dependency_read_stays_inside_publication_snapshot(self):
        request = self.request()
        original = revisions._dependencies
        changed = False
        def publish_after_dependency_read(config, record):
            nonlocal changed
            dependencies = original(config, record)
            if not changed:
                changed = True
                target = (self.path.parent / 'synthetic-concurrent-observation.json').relative_to(self.root)
                plan = {'authorization': {'synthetic_test_only': True}, 'new_directories': [],
                    'files': [{'path': target.as_posix(), 'before': None, 'after': b'{"synthetic":true}\n'}]}
                with source._locked(self.root / 'ToS/source-witnesses/historical-create', allow_pending=True):
                    transactions.apply_transaction(self.root, plan, expected_snapshot=PublicationSnapshot(self.root),
                        authorization_guard=lambda authorization, summary: authorization == {'synthetic_test_only': True},
                        transaction_id=source._digest(b'synthetic-prepare-dependency-read-race'))
            return dependencies
        with patch.object(revisions, '_dependencies', side_effect=publish_after_dependency_read):
            with self.assertRaises(PublicationChanged):
                self.fixture.run_command('prepare-revise',
                    **{key: request[key] for key in ('fields', 'forms', 'reason')})
        self.assertTrue(changed)

    def test_pending_blocks_reads_and_exact_retry_retains_one_transition(self):
        self.sync_catalog()
        reader = metadata_version_reader.MetadataVersionReader(self.root)
        request = self.request()
        self.interrupt(request)
        with self.assertRaises(PublicationPending):
            self.fixture.run_command('describe')
        resolved = reader.resolve(request['expected_source'])
        self.assertEqual(resolved['status'], 'stale', resolved)
        completed = self.run_request(request)
        self.assertEqual(completed['source']['version'], 2)
        self.assertEqual(len(json.loads((self.path.parent / revisions.HISTORY).read_bytes())['receipts']), 1)
        replayed = self.run_request(request)
        self.assertTrue(replayed['replayed'])
        self.assertEqual(replayed['receipt'], completed['receipt'])
        stale = metadata_version_reader.MetadataVersionReader(self.root).resolve(request['expected_source'])
        self.assertEqual(stale['status'], 'stale', stale)
        self.sync_catalog()
        retained = metadata_version_reader.MetadataVersionReader(self.root).resolve(request['expected_source'])
        self.assertEqual(retained['status'], 'available', retained)
        self.assertEqual(retained['version_status'], 'historical')

    def test_recovery_only_renewal_may_resume_without_granting_new_corrections(self):
        request = self.request()
        self.interrupt(request)
        self.fixture.config['allowed_operations'] = ['record.recover']
        self.fixture.owner.write_bytes(revisions._encode(self.fixture.config))
        with self.assertRaises(PermissionError):
            self.run_request(request)
        result = self.recover(request, 'resume')
        state = result['recovery']['publication']
        self.assertEqual(state['outcome'], 'committed')
        self.assertEqual(state['recovery_authorization']['owner_configuration'], result['owner_configuration'])
        self.assertNotEqual(result['owner_configuration'], request['expected_configuration'])
        with self.assertRaises(PermissionError):
            self.request('test:new-correction-not-granted')

    def test_rollback_restores_exact_selected_bytes_and_invalidates_preexisting_snapshot(self):
        snapshot = PublicationSnapshot(self.root)
        request = self.request()
        self.interrupt(request)
        result = self.recover(request, 'rollback')
        self.assertEqual(result['recovery']['status'], 'rolled-back')
        self.assertEqual(revisions._selected_package(self.path), self.before)
        self.assertIsNone(result['receipt'])
        self.assertIsNotNone(PublicationSnapshot(self.root).token)
        with self.assertRaises(PublicationChanged):
            snapshot.verify_current()
        # A retained uncommitted archive is not an available source version.
        with self.assertRaises(source.JournalConflict):
            self.fixture.run_command('inspect-version', source=request['expected_source'])

    def test_revoked_scope_third_state_and_changed_dependencies_keep_pending(self):
        request = self.request()
        self.interrupt(request)
        self.fixture.config['allowed_operations'] = []
        self.fixture.owner.write_bytes(revisions._encode(self.fixture.config))
        with self.assertRaises(PermissionError):
            self.recover(request, 'rollback')
        self.fixture.config['allowed_operations'] = ['record.recover']
        self.fixture.owner.write_bytes(revisions._encode(self.fixture.config))
        schema = self.root / 'ToS/contracts/corpus-record.schema.json'
        original = schema.read_bytes()
        schema.write_bytes(original + b'\n')
        with self.assertRaises(source.JournalConflict):
            self.recover(request, 'resume')
        schema.write_bytes(original)
        before_edit = self.path.read_bytes()
        self.path.write_bytes(before_edit + b'\n')
        with self.assertRaises(transactions.TransactionConflict):
            self.recover(request, 'rollback')
        with self.assertRaises(PublicationPending):
            PublicationSnapshot(self.root)
        self.path.write_bytes(before_edit)
        self.recover(request, 'rollback')

    def test_two_selected_revisions_keep_exact_historical_transaction_receipts(self):
        first = self.request()
        first_result = self.run_request(first)
        second = self.request('test:selected-second')
        second['fields']['notes'] = 'Another synthetic description.'
        self.run_request(second)
        repeated = self.run_request(first)
        self.assertTrue(repeated['replayed'])
        self.assertEqual(repeated['receipt'], first_result['receipt'])
        self.assertEqual(repeated['source']['version'], 3)
        self.sync_catalog()
        reader = metadata_version_reader.MetadataVersionReader(self.root)
        for ref in (first['expected_source'], second['expected_source']):
            self.assertEqual(reader.resolve(ref)['status'], 'available')
        transaction_id = first_result['receipt']['publication']['transaction_id']
        terminal = self.root / 'ToS/source-witnesses/.metadata-transactions' / transaction_id[7:] / 'completion.json'
        terminal.unlink()
        with self.assertRaises(source.JournalCorruption):
            self.run_request(first)

    def test_original_creation_replay_survives_selected_revision_and_later_descendants(self):
        from test_source_commands import HistoricalCreationTests
        fixture = HistoricalCreationTests()
        with fixture.native_creation('work') as (root, owner, config, initial, _rebuild, _graph):
            created = source.run_local_command(owner, initial)
            path = root / config['source_path']
            # Original creation remains immutable outside the exact metadata unit.
            original_receipt = (path.parent / 'source-create-receipt.json').read_bytes()
            descendant = path.parent / 'expressions' / 'untouched'
            descendant.mkdir(parents=True)
            (descendant / 'opaque.json').write_bytes(b'{"uninterpreted":false}')
            delegated = {key: value for key, value in config.items() if key not in {'maker_type', 'provenance_event_id'}}
            delegated.update(schema_version=source.CORPUS_SELECTED_REVISION_CONFIG,
                allowed_operations=['record.revise'], allowed_fields=['notes', 'field_languages'])
            owner.write_bytes(revisions._encode(delegated))
            proposal = {'fields': {'notes': 'Synthetic later description.',
                'field_languages': {**initial['record'].get('field_languages', {}),
                                    'notes': {'language': 'en', 'script': 'Latn'}}},
                'forms': initial['forms'], 'reason': 'Test retained creation versus selected correction.'}
            prepared = source.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'prepare-revise', **proposal})
            corrected = source.run_local_command(owner, {'schema_version': 'tos_local_source_command_v1',
                'operation': 'record.revise', 'command_id': 'test:created-selected-work', **proposal,
                'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
                'expected_configuration': prepared['owner_configuration'],
                'expected_dependencies': prepared['expected_dependencies'],
                'expected_publication': prepared['expected_publication']})
            self.assertEqual(corrected['source']['version'], 2)
            owner.write_bytes(revisions._encode(config))
            retried = source.run_local_command(owner, initial)
            self.assertTrue(retried['replayed'])
            self.assertEqual(retried['receipt'], created['receipt'])
            self.assertEqual((path.parent / 'source-create-receipt.json').read_bytes(), original_receipt)
            self.assertEqual((descendant / 'opaque.json').read_bytes(), b'{"uninterpreted":false}')

    def test_flat_v1_history_keeps_its_original_archive_scope_after_explicit_v2_transition(self):
        fixture = fixtures.NativeSourceRevisionTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        initial = fixture.request()
        first = source.run_local_command(fixture.owner, initial)
        fixture.config.update(schema_version=source.CORPUS_SELECTED_REVISION_CONFIG,
                              allowed_operations=['record.revise', 'record.recover'])
        fixture.owner.write_bytes(revisions._encode(fixture.config))
        (fixture.path.parent / 'new-descendant').mkdir()
        second = fixture.request('test:explicit-scope-migration')
        second['expected_publication'] = None
        source.run_local_command(fixture.owner, second)
        history = json.loads((fixture.path.parent / revisions.HISTORY).read_bytes())
        self.assertEqual(history['schema_version'], 'tos_source_revision_history_v2')
        self.assertNotIn('publication', history['receipts'][0])
        self.assertEqual(history['receipts'][0], first['receipt'])
        prior = fixture.run_command('inspect-version', source=initial['expected_source'])
        self.assertIn('unrecognized.json', prior['files'])
        later = fixture.run_command('inspect-version', source=second['expected_source'])
        self.assertEqual(set(later['files']), set(revisions._selected_names(fixture.path)))


if __name__ == '__main__':
    unittest.main()

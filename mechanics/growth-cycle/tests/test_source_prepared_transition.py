"""Real selected source transactions captured read-only for derived assembly."""
from dataclasses import replace
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
import source_commands as source
import source_revisions as revisions
import source_selected_revisions as selected
import source_prepared_transition as bridge
from source_metadata_snapshot import PublicationPending
import test_source_selected_revisions as fixtures


class SourcePreparedTransitionTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.SelectedSourceRevisionTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.f = self.fixture.fixture

    def commit(self, identifier='test:prepared-transition'):
        request = self.fixture.request(identifier)
        result = self.fixture.run_request(request)
        return request, result['receipt']['publication']['transaction_id']

    def capture(self, request, identifier):
        return bridge.capture_selected_prepared_transition(self.f.owner, identifier,
            expected_before_publication=request['expected_publication'])

    def test_exact_detached_before_after_archive_and_replay_without_source_writes(self):
        request, identifier = self.commit()
        before_read = revisions._selected_package(self.f.path)
        first = self.capture(request, identifier)
        self.assertEqual(dict(first.before_files), self.fixture.before)
        self.assertEqual(dict(first.after_files), before_read)
        self.assertEqual(first.record('before'), self.f.record)
        self.assertEqual(first.record('after'), {**self.f.record, **request['fields'], 'record_version': 2})
        self.assertEqual(first.receipt()['request'], request)
        self.assertEqual(first.receipt()['source'], source.metadata_subject(first.record('after')).ref)
        self.assertFalse(first.receipt()['grants_admission'])
        detached = first.record('after')
        detached['notes'] = 'Mutating a decoded copy cannot alter the capture'
        self.assertNotEqual(first.record('after')['notes'], detached['notes'])
        self.assertTrue(bridge.verify_selected_prepared_transition_current(first))
        replay = self.fixture.run_request(request)
        self.assertTrue(replay['replayed'])
        self.assertEqual(first, self.capture(request, identifier))
        self.assertEqual(revisions._selected_package(self.f.path), before_read)

    def test_capture_does_not_enumerate_source_home_or_descendants(self):
        request, identifier = self.commit()
        original = Path.iterdir
        def list_only_archive(path):
            self.assertNotEqual(path, self.f.path.parent)
            self.assertFalse(path.is_relative_to(self.fixture.nested))
            return original(path)
        with patch.object(Path, 'iterdir', list_only_archive):
            result = self.capture(request, identifier)
        self.assertEqual(len(result.after_files), 3)
        self.assertNotIn('unrecognized.json', dict(result.after_files))

    def test_wrong_predecessor_or_current_delegation_refuses(self):
        request, identifier = self.commit()
        with self.assertRaises(source.JournalConflict):
            bridge.capture_selected_prepared_transition(self.f.owner, identifier,
                expected_before_publication='sha256:' + 'e' * 64)
        self.f.config['allowed_operations'] = ['record.recover']
        self.f.owner.write_bytes(revisions._encode(self.f.config))
        with self.assertRaises((ValueError, PermissionError)):
            self.capture(request, identifier)

    def test_untracked_source_byte_change_refuses_even_with_unchanged_publication(self):
        request, identifier = self.commit()
        observed = self.capture(request, identifier)
        record = observed.record('after')
        record['notes'] += ' Changed without participating source publication.'
        self.f.path.write_bytes(revisions._encode(record))
        with self.assertRaises(ValueError):
            bridge.verify_selected_prepared_transition_current(observed)

    def test_pending_and_rolled_back_are_not_committed_source_transitions(self):
        request = self.fixture.request()
        identifier = selected._transaction_id(request)
        self.fixture.interrupt(request)
        with self.assertRaises(PublicationPending):
            self.capture(request, identifier)
        self.fixture.recover(request, 'rollback')
        with self.assertRaises(source.JournalConflict):
            self.capture(request, identifier)

    def test_historical_committed_transition_is_not_current_after_another_commit(self):
        request, identifier = self.commit()
        observed = self.capture(request, identifier)
        second, next_id = self.commit('test:next-prepared-transition')
        with self.assertRaises(source.JournalConflict):
            self.capture(request, identifier)
        current = self.capture(second, next_id)
        self.assertEqual(current.before_publication, observed.after_publication)
        self.assertNotEqual(current.after_publication, observed.after_publication)
        self.assertEqual(current.record('after')['record_version'], 3)

    def test_final_selected_file_recheck_detects_mid_capture_edit(self):
        request, identifier = self.commit()
        original = revisions._selected_package
        reads = 0
        def changed_after_read(path):
            nonlocal reads
            result = original(path)
            reads += 1
            if reads == 1:
                record = json.loads(self.f.path.read_bytes())
                record['notes'] += ' Synthetic concurrent edit.'
                self.f.path.write_bytes(revisions._encode(record))
            return result
        with patch.object(revisions, '_selected_package', side_effect=changed_after_read), \
                self.assertRaises(source.JournalConflict):
            self.capture(request, identifier)
        self.assertGreaterEqual(reads, 2)

    def test_forged_detached_capture_or_retained_blob_cannot_be_reverified(self):
        request, identifier = self.commit()
        observed = self.capture(request, identifier)
        with self.assertRaises(source.JournalConflict):
            bridge.verify_selected_prepared_transition_current(replace(observed, dependencies='sha256:' + 'f' * 64))
        archive = self.f.root / observed.receipt()['archive_path']
        manifest = json.loads((archive / 'manifest.json').read_bytes())
        blob = archive / manifest['files'][self.f.path.name]['blob']
        blob.write_bytes(b'{}')
        with self.assertRaises(ValueError):
            self.capture(request, identifier)


if __name__ == '__main__':
    unittest.main()

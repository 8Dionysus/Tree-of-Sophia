"""Exact public Claim reads; synthetic bytes/history, no active owner grants."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
sys.path.insert(0, str(MECHANIC))
import claim_revisions as claims
import claim_version_reader as reader
import source_commands as source
import source_revisions as packages


class ClaimVersionReaderTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.relative = 'ToS/source-witnesses/relations/synthetic/source-claims.jsonl'
        self.path = self.root / self.relative
        self.path.parent.mkdir(parents=True)
        self.catalog = self.root / reader.CATALOG_REF
        self.catalog.parent.mkdir()
        self.record = {'schema_version': 'tos_synthetic_claim_v1', 'claim_id': 'tos.claim.synthetic.selected',
            'claim_type': 'relation', 'claim_version': 1, 'visibility': 'public_metadata_only',
            'subject_ref': 'tos.occurrence.synthetic.first', 'predicate': 'synthetic_qualified_proposal',
            'object': {'kind': 'motif-proposal', 'members': ['tos.occurrence.synthetic.first', 'tos.occurrence.synthetic.second']},
            'assertion_layer': 'interpretation', 'epistemic_status': 'hypothesis', 'review_status': 'unreviewed',
            'maker': {'agent_ref': 'test:synthetic', 'maker_type': 'model'},
            'provenance_event_ref': 'tos.event.synthetic.only', 'evidence_refs': ['test:synthetic-only'],
            'qualifiers': {'statement': 'Условная гипотеза, НЕ принятое значение.',
                           'statement_language': 'ru', 'unknown': {'false': False, 'zero': 0, 'null': None, 'empty': ''}},
            'extensions': {'uninterpreted': [False, 0, None, 'Ω']}}
        self.sibling = {**copy.deepcopy(self.record), 'claim_id': 'tos.claim.synthetic.sibling',
                        'qualifiers': {'statement': 'NEIGHBOR PROSE MUST NOT LEAVE THE READER'}}
        self.path.write_bytes(source._canonical(self.record) + b'\r\n\n' + source._canonical(self.sibling) + b'\n')
        (self.path.parent / 'unrecognized.json').write_bytes(b'{ "metadata": false }\n')
        self.navigation = {'source_root': str(self.root), 'source_path': self.relative}
        self.sync_catalog()

    def sync_catalog(self):
        entries = []
        for line, encoded in enumerate(self.path.read_bytes().splitlines(), start=1):
            if not encoded.strip():
                continue
            record = source._json_object(encoded)
            entries.append({'schema_version': 'tos_source_witness_claim_catalog_entry_v1',
                'claim_id': record['claim_id'], 'claim_version': record['claim_version'],
                'source_claim_file_ref': self.relative, 'source_claim_line': line,
                'claim_sha256': claims._subject(record).ref['digest'].removeprefix('sha256:'),
                'visibility': record['visibility']})
        self.catalog.write_bytes(b''.join(source._canonical(entry) + b'\n' for entry in entries))

    def change_catalog(self, **fields):
        entries = [source._json_object(row) for row in self.catalog.read_bytes().splitlines()]
        entries[0].update(fields)
        self.catalog.write_bytes(b''.join(source._canonical(entry) + b'\n' for entry in entries))

    def correct(self, identity, statement):
        """Retain actual byte-bound packages with the existing pure helpers.

        This constructs synthetic receipts, not a delegated production command.
        No owner configuration, grant, assessment or form materializer is used.
        """
        files = packages._package(self.path.parent)
        previous = claims._claims(files[self.path.name])[identity]
        previous_ref, revision = claims._subject(previous).ref, packages._revision(files)
        history = source._json_object(files[claims.HISTORY]) if claims.HISTORY in files else {
            'schema_version': 'tos_claim_revision_history_v1', 'source_path': self.relative, 'receipts': []}
        request = {'operation': 'claim.revise', 'command_id': 'test:correction-' + str(len(history['receipts']) + 1),
            'fields': {'qualifiers': {'statement': statement}}, 'forms': [], 'reason': 'Synthetic test only.',
            'expected_source': previous_ref, 'expected_revision': revision,
            'expected_configuration': 'sha256:' + '0' * 64, 'expected_dependencies': 'sha256:' + '1' * 64,
            'expected_inputs': {}}
        revised = claims._advance(previous, request['fields'])
        archive = packages._archive(self.root, {**self.navigation, 'record_id': identity},
            files, claims._subject(previous), revision, reader=claims._read_archive)
        receipt = {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
            'principal_id': 'test:synthetic', 'authority_ref': 'test:not-production-authority',
            'owner_configuration': request['expected_configuration'], 'recorded_at': '2026-01-01T00:00:00Z',
            'reason': request['reason'], 'previous_source': previous_ref, 'source': claims._subject(revised).ref,
            'previous_revision': revision, 'archive_path': archive.as_posix(),
            'dependencies': request['expected_dependencies'], 'source_bindings': request['expected_inputs'],
            'changed_fields': sorted(request['fields']), 'forms': [], 'grants_admission': False, 'request': request}
        history['receipts'].append(receipt)
        self.path.write_bytes(claims._replace(files[self.path.name], revised))
        (self.path.parent / claims.HISTORY).write_bytes(packages._encode(history))
        self.sync_catalog()
        return previous, revised, receipt

    def resolve(self, ref=None):
        return reader.resolve_claim_version(self.root, ref or claims._subject(self.record).ref)

    def assert_unavailable(self, result, status, reason=None):
        self.assertEqual(result['status'], status, result)
        if reason:
            self.assertEqual(result['reason'], reason)
        for key in ('record', 'record_digest', 'version_status', 'provenance'):
            self.assertIsNone(result[key])
        for key in ('grants_current_use', 'performs_assessment', 'writes_to_source'):
            self.assertFalse(result[key])

    def test_current_exact_record_preserves_all_fields_but_not_sibling_prose_or_permissions(self):
        before = {path: path.read_bytes() for path in self.root.rglob('*') if path.is_file()}
        with (patch.object(source, '_configuration', side_effect=AssertionError('no owner config')),
              patch.object(source, 'run_local_command', side_effect=AssertionError('no owner command'))):
            result = self.resolve()
        self.assertEqual(result['status'], 'available')
        self.assertEqual(result['version_status'], 'current')
        self.assertEqual(result['record'], self.record)
        self.assertEqual(result['record_digest'], claims._subject(self.record).ref['digest'])
        self.assertEqual(result['provenance']['source']['stream_sha256'], source._digest(self.path.read_bytes()))
        self.assertNotEqual(result['record_digest'], result['provenance']['source']['stream_sha256'])
        self.assertEqual(result['provenance']['source']['line'], 1)
        self.assertIsNone(result['provenance']['source']['archive_blob_ref'])
        self.assertNotIn('NEIGHBOR PROSE', json.dumps(result))
        self.assertFalse(result['grants_current_use'])
        self.assertEqual(before, {path: path.read_bytes() for path in self.root.rglob('*') if path.is_file()})

    def test_interleaved_shared_corrections_resolve_exact_old_versions_and_bind_current_catalog_separately(self):
        first, second, receipt = self.correct(self.record['claim_id'], 'Second, still qualified.')
        self.correct(self.sibling['claim_id'], 'Sibling correction stays separate.')
        _, third, _ = self.correct(self.record['claim_id'], 'Third, no new admission.')
        instance = reader.ClaimVersionReader(self.root)
        for record, status in ((first, 'historical'), (second, 'historical'), (third, 'current'), (self.sibling, 'historical')):
            with self.subTest(version=claims._subject(record).ref):
                result = instance.resolve(claims._subject(record).ref)
                self.assertEqual(result['status'], 'available', result)
                self.assertEqual(result['record'], record)
                self.assertEqual(result['version_status'], status)
                self.assertEqual(result['provenance']['history']['receipt_count'], 3)
        result = instance.resolve(claims._subject(first).ref)
        self.assertEqual(result['provenance']['catalog']['current_record_ref'], claims._subject(third).ref)
        self.assertEqual(result['provenance']['transition']['previous_source'], claims._subject(first).ref)
        self.assertEqual(result['provenance']['transition']['source'], claims._subject(second).ref)
        blob = self.root / result['provenance']['source']['archive_blob_ref']
        self.assertEqual(source._digest(blob.read_bytes()), result['provenance']['source']['stream_sha256'])
        self.assertNotIn('request', result['provenance']['transition'])
        self.assertEqual(result['provenance']['source']['package_revision'], receipt['previous_revision'])
        instance.verify_current()

    def test_unretained_version_and_wrong_digest_never_fall_back_to_latest(self):
        self.correct(self.record['claim_id'], 'New current version.')
        ref = claims._subject(self.record).ref
        self.assert_unavailable(self.resolve({**ref, 'version': 9}), 'missing', 'exact-version-not-retained')
        self.assert_unavailable(self.resolve({**ref, 'digest': 'sha256:' + 'f' * 64}), 'stale', 'exact-version-digest-mismatch')
        self.assert_unavailable(self.resolve({**ref, 'id': 'tos.claim.synthetic.absent'}), 'missing', 'claim-not-in-public-catalog')

    def test_stale_catalog_line_version_digest_and_visibility_do_not_authorize_source(self):
        for change in ({'source_claim_line': 3}, {'claim_version': 2}, {'claim_version': True},
                       {'claim_sha256': 'a' * 64}, {'visibility': 'public'}):
            with self.subTest(change=change):
                self.sync_catalog()
                self.change_catalog(**change)
                self.assert_unavailable(self.resolve(), 'stale')
        self.change_catalog(visibility='restricted')
        self.assert_unavailable(self.resolve(), 'access-restricted', 'catalog-claim-not-public-metadata')

    def test_duplicate_catalog_or_stream_id_fails_closed(self):
        self.catalog.write_bytes(self.catalog.read_bytes() + self.catalog.read_bytes().splitlines()[0] + b'\n')
        self.assert_unavailable(self.resolve(), 'corrupt')
        self.sync_catalog()
        self.path.write_bytes(self.path.read_bytes() + source._canonical(self.sibling) + b'\n')
        self.assert_unavailable(self.resolve(), 'corrupt')

    def test_missing_corrupt_and_uncommitted_archives_are_distinct_from_current_data(self):
        _, _, receipt = self.correct(self.record['claim_id'], 'Second version.')
        archive = self.root / receipt['archive_path']
        manifest = source._json_object((archive / 'manifest.json').read_bytes())
        blob = archive / manifest['files'][self.path.name]['blob']
        original = blob.read_bytes()
        blob.write_bytes(b'corrupt')
        self.assert_unavailable(self.resolve(), 'corrupt', 'history-integrity-failed')
        blob.unlink()
        self.assert_unavailable(self.resolve(), 'missing', 'retained-archive-file-missing')
        blob.write_bytes(original)
        (self.path.parent / claims.HISTORY).unlink()
        self.assert_unavailable(self.resolve(), 'corrupt', 'history-integrity-failed')

    def test_changed_request_or_unrecorded_sibling_edit_breaks_complete_chain(self):
        self.correct(self.record['claim_id'], 'Second version.')
        history_path = self.path.parent / claims.HISTORY
        original = history_path.read_bytes()
        history = source._json_object(original)
        receipt = history['receipts'][0]
        receipt['request']['fields']['qualifiers']['statement'] = 'Different successor.'
        receipt['request_digest'] = source._digest(source._canonical(receipt['request']))
        history_path.write_bytes(packages._encode(history))
        self.assert_unavailable(self.resolve(), 'corrupt', 'history-integrity-failed')
        history_path.write_bytes(original)
        sibling = claims._claims(self.path.read_bytes())[self.sibling['claim_id']]
        sibling['qualifiers']['statement'] = 'Unrecorded sibling edit.'
        self.path.write_bytes(claims._replace(self.path.read_bytes(), sibling))
        self.sync_catalog()
        self.assert_unavailable(self.resolve(), 'corrupt', 'history-integrity-failed')

    def test_found_predecessor_is_not_returned_before_later_sibling_archive_is_verified(self):
        self.correct(self.record['claim_id'], 'Second selected version.')
        _, _, later = self.correct(self.sibling['claim_id'], 'Later sibling correction.')
        archive = self.root / later['archive_path']
        manifest = source._json_object((archive / 'manifest.json').read_bytes())
        (archive / manifest['files'][self.path.name]['blob']).write_bytes(b'corrupt later archive')
        self.assert_unavailable(self.resolve(), 'corrupt', 'history-integrity-failed')

    def test_public_locator_and_package_paths_cannot_read_private_payload_or_symlinks(self):
        for directory in ('payload', 'private', 'local-content', 'owner-local', '.record-revisions'):
            with self.subTest(directory=directory):
                self.sync_catalog()
                self.change_catalog(source_claim_file_ref=f'ToS/source-witnesses/{directory}/source-claims.jsonl')
                with patch.object(packages, '_package', side_effect=AssertionError('must reject locator first')):
                    self.assert_unavailable(self.resolve(), 'access-restricted')
        self.sync_catalog()
        payload = self.path.parent / 'payload'
        payload.mkdir()
        self.assert_unavailable(self.resolve(), 'access-restricted')
        payload.rmdir()
        original = self.path.read_bytes()
        elsewhere = self.root / 'elsewhere.jsonl'
        elsewhere.write_bytes(original)
        self.path.unlink()
        self.path.symlink_to(elsewhere)
        self.assert_unavailable(self.resolve(), 'access-restricted')

    def test_private_stream_and_private_archive_member_are_not_delivered(self):
        private = {**self.record, 'visibility': 'restricted'}
        self.path.write_bytes(claims._replace(self.path.read_bytes(), private))
        self.assert_unavailable(self.resolve(), 'access-restricted', 'claim-stream-not-public-metadata')
        self.path.write_bytes(claims._replace(self.path.read_bytes(), self.record))
        _, _, receipt = self.correct(self.record['claim_id'], 'Second version.')
        archive = self.root / receipt['archive_path']
        path = archive / 'manifest.json'
        manifest = source._json_object(path.read_bytes())
        manifest['files']['private'] = manifest['files'].pop('unrecognized.json')
        path.write_bytes(packages._encode(manifest))
        with patch.object(claims, '_read_archive', side_effect=AssertionError('private manifest must stop blob read')):
            self.assert_unavailable(self.resolve(), 'access-restricted', 'package-outside-public-metadata')

    def test_declared_budgets_are_not_misreported_as_corruption(self):
        for module, name, value, reason in (
            (reader, 'MAX_CATALOG_BYTES', 1, 'catalog-byte-budget'),
            (reader, 'MAX_CATALOG_ROWS', 1, 'catalog-record-budget'),
            (reader, 'MAX_TOTAL_BYTES', 1, 'total-read-byte-budget'),
            (packages, 'MAX_FILES', 1, 'package-file-count-budget'),
            (packages, 'MAX_PACKAGE_BYTES', 1, 'package-byte-budget'),
        ):
            with self.subTest(budget=name), patch.object(module, name, value):
                self.assert_unavailable(self.resolve(), 'over-budget', reason)
        self.correct(self.record['claim_id'], 'Second version.')
        with patch.object(packages, 'MAX_REVISIONS', 0):
            self.assert_unavailable(self.resolve(), 'over-budget', 'correction-receipt-count-budget')

    def test_batched_reads_reuse_bytes_but_still_fail_closed_on_later_drift(self):
        _, second, _ = self.correct(self.record['claim_id'], 'Second version.')
        _, third, _ = self.correct(self.record['claim_id'], 'Third version.')
        instance = reader.ClaimVersionReader(self.root)
        with (patch.object(source, '_read', wraps=source._read) as reads,
              patch.object(claims, '_history', wraps=claims._history) as histories):
            first = instance.resolve(claims._subject(self.record).ref)
            self.assertEqual(first['status'], 'available', first)
            count = reads.call_count
            first['record']['qualifiers']['statement'] = 'Caller mutation cannot rewrite cached source.'
            for record in (second, third, self.sibling, self.record):
                result = instance.resolve(claims._subject(record).ref)
                self.assertEqual(result['record'], record, result)
            instance.verify_current()
            self.assertEqual(reads.call_count, count)
            self.assertEqual(histories.call_count, 1)
            self.assertEqual(sum(call.args[0] == self.catalog for call in reads.call_args_list), 1)
        (self.path.parent / 'unrecognized.json').write_bytes(b'{"changed": true}\n')
        self.assert_unavailable(instance.resolve(claims._subject(self.record).ref), 'stale', 'source-changed-during-read')
        with self.assertRaises(source.JournalConflict):
            instance.verify_current()

    def test_concurrent_change_during_history_verification_never_returns_mixed_snapshot(self):
        self.correct(self.record['claim_id'], 'Second version.')
        original = claims._history
        def changed(*args, **kwargs):
            history = original(*args, **kwargs)
            (self.path.parent / 'unrecognized.json').write_bytes(b'{"changed": true}\n')
            return history
        with patch.object(claims, '_history', side_effect=changed):
            self.assert_unavailable(self.resolve(), 'stale', 'source-changed-during-read')

    def test_invalid_exact_refs_do_not_read_source(self):
        ref = claims._subject(self.record).ref
        for invalid in ({**ref, 'version': True}, {**ref, 'id': '../payload'}, {**ref, 'digest': 'wrong'},
                        {**ref, 'extra': 'not allowed'}):
            with (self.subTest(ref=invalid), patch.object(source, '_read', side_effect=AssertionError('no read')),
                  self.assertRaises(ValueError)):
                self.resolve(invalid)


if __name__ == '__main__':
    unittest.main()

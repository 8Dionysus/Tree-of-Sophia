"""Synthetic selected-metadata transport; real filesystem/process crash edges.

These checks protect transport/currentness boundaries, not Work/Expression
semantics, author competence, public admission, or arbitrary raw-reader atomicity.
"""
import base64
import copy
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(MECHANIC))
import source_metadata_snapshot as publication
import source_metadata_transactions as transactions

WORK = 'ToS/source-witnesses/works/synthetic/work'
IDENTIFIER = publication._digest(b'synthetic command identity, independent of after bytes')
AUTHORIZATION = {'owner_configuration': publication._digest(b'synthetic-owner-v1'),
                 'dependencies': publication._digest(b'synthetic-contracts-v1'),
                 'command_id': 'test:selected-metadata', 'authority_ref': 'test:explicit-owner'}


@contextmanager
def corpus_lock(root):
    """The actual stable lock locator; engine deliberately does not acquire it."""
    descriptor = os.open(root / 'ToS/source-witnesses/.historical-create.writer.lock',
                         os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        yield
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def fixture(root):
    directory = root / WORK
    directory.mkdir(parents=True)
    (directory / 'work.json').write_bytes(b'{ "record_version": 1, "label": "synthetic" }\n')
    (directory / 'work.human-forms.json').write_bytes(b'{ "subject_version": 1 }\n')
    return {'authorization': copy.deepcopy(AUTHORIZATION), 'files': [
        {'path': WORK + '/work.json', 'before': (directory / 'work.json').read_bytes(),
         'after': b'{ "record_version": 2, "label": "synthetic revision" }\n'},
        {'path': WORK + '/work.human-forms.json', 'before': (directory / 'work.human-forms.json').read_bytes(),
         'after': b'{ "subject_version": 2 }\n'},
        {'path': WORK + '/expressions/fixture/expression.json', 'before': None,
         'after': b'{ "record_version": 1, "synthetic": true }\n'}],
        'new_directories': [WORK + '/expressions', WORK + '/expressions/fixture']}


def item_fixture(root):
    """A deposited Item payload is outside this selected metadata transaction."""
    edition = WORK + '/editions/synthetic'
    item = edition + '/items/synthetic'
    payload = root / item / 'payload/original.bin'
    payload.parent.mkdir(parents=True)
    payload.write_bytes(b'synthetic deposited bytes outside metadata authority')
    before = b'{"item_ids":[]}\n'
    (root / edition / 'edition.json').write_bytes(before)
    authorization = {**copy.deepcopy(AUTHORIZATION),
        'schema_version': 'tos_item_adoption_authorization_v1',
        'scope': {'item_source_path': item + '/item.json'}}
    return {'authorization': authorization,
        'path_profile': {'schema_version': 'tos_item_metadata_paths_v1',
                         'item_source_path': item + '/item.json'},
        'files': [
            {'path': edition + '/edition.json', 'before': before,
             'after': b'{"item_ids":["synthetic:item"]}\n'},
            {'path': item + '/item.json', 'before': None, 'after': b'{"id":"synthetic:item"}\n'},
            {'path': item + '/fixity.sha256', 'before': None, 'after': b'synthetic fixity metadata\n'},
            {'path': item + '/forensic-report.md', 'before': None, 'after': b'# Synthetic forensic evidence\n'},
            {'path': item + '/provenance.jsonl', 'before': None, 'after': b'{"event":"synthetic"}\n'}],
        'new_directories': []}


CHILD = r'''
import base64, fcntl, json, os, stat, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1]); sys.path.insert(0, sys.argv[2])
import source_metadata_transactions as tx
from source_metadata_snapshot import PublicationSnapshot
root = Path(sys.argv[3]); operation, edge = sys.argv[4:6]
data = json.load(sys.stdin)
plan = data['plan']
for item in plan['files']:
    for side in ('before', 'after'):
        if item[side] is not None:
            item[side] = base64.b64decode(item[side])
original_replace, original_publish, original_fsync = tx._replace_file, tx._publish_state, os.fsync
count = 0
def replace(*args):
    global count
    count += 1
    if edge == 'before-file-' + str(count): os._exit(86)
    original_replace(*args)
    if edge == 'after-file-' + str(count): os._exit(86)
def publish(root, state, expected):
    original_publish(root, state, expected)
    if edge == 'after-' + state['phase']: os._exit(86)
def sync(descriptor):
    selected = root / 'ToS/source-witnesses/works/synthetic/work/expressions/fixture/expression.json'
    if (edge == 'after-rename-before-directory-fsync' and stat.S_ISDIR(os.fstat(descriptor).st_mode)
            and selected.exists()): os._exit(86)
    original_fsync(descriptor)
tx._replace_file, tx._publish_state = replace, publish
os.fsync = sync
guard = lambda authority, summary: authority == data['authorization']
with open(root / 'ToS/source-witnesses/.historical-create.writer.lock', 'ab') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    if operation == 'apply':
        result = tx.apply_transaction(root, plan, expected_snapshot=PublicationSnapshot(root),
            authorization_guard=guard, transaction_id=data['transaction_id'])
    else:
        method = tx.resume_transaction if operation == 'resume' else tx.rollback_transaction
        result = method(root, authorization_guard=guard, transaction_id=data['transaction_id'])
print(json.dumps(result))
'''


class SelectedMetadataTransactionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='selected-metadata-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.plan = fixture(self.root)
        self.snapshot = publication.PublicationSnapshot(self.root)
        self.authorized = True

    def guard(self, authorization, summary):
        self.assertEqual(authorization, AUTHORIZATION)
        self.assertEqual(summary['authorization'], AUTHORIZATION)
        return self.authorized

    def apply(self, plan=None, *, snapshot=None, identifier=IDENTIFIER, guard=None):
        with corpus_lock(self.root):
            return transactions.apply_transaction(self.root, plan or self.plan,
                expected_snapshot=snapshot or self.snapshot, authorization_guard=guard or self.guard,
                transaction_id=identifier)

    def recover(self, *, rollback=False, **options):
        with corpus_lock(self.root):
            method = transactions.rollback_transaction if rollback else transactions.resume_transaction
            return method(self.root, authorization_guard=options.pop('authorization_guard', self.guard), **options)

    def assert_side(self, root, plan, side):
        for item in plan['files']:
            path = root / item['path']
            if item[side] is None:
                self.assertFalse(os.path.lexists(path), item['path'])
            else:
                self.assertEqual(path.read_bytes(), item[side], item['path'])

    def child(self, root, plan, operation='apply', edge='', *, identifier=IDENTIFIER):
        encoded = copy.deepcopy(plan)
        for item in encoded['files']:
            for side in ('before', 'after'):
                if item[side] is not None:
                    item[side] = base64.b64encode(item[side]).decode()
        request = {'plan': encoded, 'transaction_id': identifier, 'authorization': plan['authorization']}
        return subprocess.run([sys.executable, '-c', CHILD, str(MECHANIC), str(ROOT / 'scripts'),
                               str(root), operation, edge], input=json.dumps(request), text=True,
                              capture_output=True, timeout=15)

    def pause_after_first_file(self):
        original = transactions._replace_file
        count = 0
        def stop(*args):
            nonlocal count
            original(*args)
            count += 1
            if count == 1:
                raise RuntimeError('synthetic interrupted response')
        with patch.object(transactions, '_replace_file', side_effect=stop):
            with self.assertRaises(RuntimeError):
                self.apply()
        return transactions.read_pending_transaction(self.root)

    def test_selected_multi_file_publication_keeps_descendants_and_retained_exact_bytes(self):
        payload = self.root / WORK / 'unselected/nested/payload/original.bin'
        payload.parent.mkdir(parents=True)
        payload.write_bytes(b'unknown descendant bytes must never be copied or opened by transport')
        alias = self.root / WORK / 'unselected/dangling'
        alias.symlink_to('/does-not-exist')
        before = payload.stat()
        result = self.apply()
        self.assert_side(self.root, self.plan, 'after')
        self.assertEqual(payload.stat().st_ino, before.st_ino)
        self.assertEqual(payload.stat().st_mtime_ns, before.st_mtime_ns)
        self.assertTrue(alias.is_symlink())
        self.assertEqual(result['status'], 'committed')
        self.assertFalse(result['grants_admission'])
        self.assertEqual(result['publication']['generation'], 2)
        retained = transactions.inspect_transaction(self.root, IDENTIFIER)
        self.assertEqual(retained['plan']['files'], sorted(self.plan['files'], key=lambda item: item['path']))
        self.assertEqual(retained['manifest']['schema_version'], 'tos_selected_metadata_transaction_v1')
        self.assertEqual(set(retained['plan']), {'authorization', 'files', 'new_directories'})
        self.assertEqual(retained['manifest']['plan']['authorization'], AUTHORIZATION)
        self.assertFalse(retained['writes_to_source'])
        self.assertIsNone(transactions.read_pending_transaction(self.root))
        with self.assertRaises(publication.PublicationChanged):
            self.snapshot.verify_current()
        publication.PublicationSnapshot(self.root).verify_current()
        self.assertTrue(self.apply()['replayed'])

    def test_forbidden_paths_duplicate_targets_and_undeclared_parents_fail_before_state(self):
        denied = ['ToS/source-witnesses/catalog/works.json', 'ToS/source-witnesses/owner-local/x/a.json',
                  'ToS/source-witnesses/works/x/payload/a.json', 'ToS/source-witnesses/private/x/a.json',
                  'ToS/source-witnesses/works/local-content/a.json', 'ToS/source-witnesses/.record-revisions/a.json',
                  'ToS/source-witnesses/.metadata-publication.json', '/tmp/arbitrary.json',
                  'ToS/source-witnesses/works/../a.json', 'ToS/source-witnesses/works/a.txt',
                  WORK + '/editions/synthetic/items/synthetic/fixity.sha256',
                  WORK + '/editions/synthetic/items/synthetic/forensic-report.md',
                  'ToS\\source-witnesses\\works\\a.json']
        for ref in denied:
            plan = copy.deepcopy(self.plan)
            plan['files'][0]['path'] = ref
            with self.subTest(path=ref), self.assertRaises((ValueError, PermissionError)):
                self.apply(plan)
            self.assertIsNone(publication.read_publication_state(self.root))
        for modify in (lambda p: p['files'].append(copy.deepcopy(p['files'][0])),
                       lambda p: p['new_directories'].clear(),
                       lambda p: p['new_directories'].append(WORK + '/unselected-empty'),
                       lambda p: p['new_directories'].append(WORK + '/work.json/child')):
            plan = copy.deepcopy(self.plan)
            modify(plan)
            with self.assertRaises((ValueError, PermissionError)):
                self.apply(plan)
        self.assert_side(self.root, self.plan, 'before')

    def test_item_path_profile_is_retained_and_does_not_touch_deposited_payload(self):
        plan = item_fixture(self.root)
        item = Path(plan['path_profile']['item_source_path']).parent
        payload = self.root / item / 'payload/original.bin'
        before = payload.stat()
        frozen, _ = transactions._freeze_plan(plan)
        frozen_profile = copy.deepcopy(frozen['path_profile'])
        plan['path_profile']['item_source_path'] = 'changed caller-owned input'
        self.assertEqual(frozen['path_profile'], frozen_profile)
        plan['path_profile'] = frozen_profile
        def guard(authority, summary):
            self.assertEqual(authority, plan['authorization'])
            self.assertEqual(summary['path_profile'], plan['path_profile'])
            summary['path_profile'].clear()  # Callback mutation must not alter retained scope.
            return True
        result = self.apply(plan, guard=guard)
        self.assertEqual(result['status'], 'committed')
        self.assert_side(self.root, plan, 'after')
        retained = transactions.inspect_transaction(self.root, IDENTIFIER)
        self.assertEqual(retained['manifest']['schema_version'], 'tos_selected_metadata_transaction_v2')
        self.assertEqual(retained['plan']['path_profile'], plan['path_profile'])
        self.assertEqual(retained['plan']['files'], sorted(plan['files'], key=lambda entry: entry['path']))
        self.assertEqual((payload.stat().st_ino, payload.stat().st_mtime_ns),
                         (before.st_ino, before.st_mtime_ns))
        self.assertEqual(payload.read_bytes(), b'synthetic deposited bytes outside metadata authority')
        self.assertTrue(self.apply(plan, guard=guard)['replayed'])

    def test_item_profile_requires_exact_version_and_matching_adoption_authority(self):
        original = item_fixture(self.root)
        invalid_profiles = [None, {}, {'schema_version': 'tos_item_metadata_paths_v0',
                                      'item_source_path': original['path_profile']['item_source_path']},
                            {**original['path_profile'], 'additional_suffix': '.md'}]
        for profile in invalid_profiles:
            plan = copy.deepcopy(original)
            plan['path_profile'] = profile
            with self.subTest(profile=profile), self.assertRaises((ValueError, PermissionError)):
                self.apply(plan, guard=lambda *args: True)
        for change in (lambda p: p.pop('path_profile'),
                       lambda p: p['authorization'].pop('schema_version'),
                       lambda p: p['authorization'].update(schema_version='tos_item_adoption_authorization_v2'),
                       lambda p: p['authorization'].pop('scope'),
                       lambda p: p['authorization'].update(scope=None),
                       lambda p: p['authorization']['scope'].update(item_source_path=WORK + '/item.json')):
            plan = copy.deepcopy(original)
            change(plan)
            with self.assertRaises((ValueError, PermissionError)):
                self.apply(plan, guard=lambda *args: True)
        self.assertIsNone(publication.read_publication_state(self.root))
        self.assertFalse((self.root / publication.TRANSACTIONS_REF).exists())
        self.assert_side(self.root, original, 'before')

    def test_item_profile_does_not_grant_other_homes_suffixes_or_forbidden_paths(self):
        original = item_fixture(self.root)
        item = Path(original['path_profile']['item_source_path']).parent.as_posix()
        denied = [item + '/notes.md', item + '/other.sha256',
                  item + '/child/forensic-report.md', str(Path(item).with_name('other')) + '/fixity.sha256',
                  WORK + '/forensic-report.md', item + '/payload/fixity.sha256',
                  item + '/private/forensic-report.md', item + '/owner-local/forensic-report.md',
                  item + '/local-content/forensic-report.md', item + '/catalog/fixity.sha256',
                  item + '/.hidden/forensic-report.md', item + '/../synthetic/fixity.sha256',
                  '/' + item + '/fixity.sha256', item + '//fixity.sha256']
        for ref in denied:
            plan = copy.deepcopy(original)
            plan['files'][3]['path'] = ref
            with self.subTest(path=ref), self.assertRaises((ValueError, PermissionError)):
                self.apply(plan, guard=lambda *args: True)
        for ref in (WORK + '/item.json', item + '/other.json', item + '/payload/item.json',
                    item + '/private/items/another/item.json'):
            plan = copy.deepcopy(original)
            plan['path_profile']['item_source_path'] = ref
            plan['authorization']['scope']['item_source_path'] = ref
            with self.subTest(profile_path=ref), self.assertRaises((ValueError, PermissionError)):
                self.apply(plan, guard=lambda *args: True)
        self.assertIsNone(publication.read_publication_state(self.root))
        self.assertFalse((self.root / publication.TRANSACTIONS_REF).exists())

    def test_item_profile_survives_fresh_process_resume_and_rollback(self):
        for operation in ('resume', 'rollback'):
            with self.subTest(operation=operation):
                root = self.root / operation
                plan = item_fixture(root)
                interrupted = self.child(root, plan, edge='after-file-3')
                self.assertEqual(interrupted.returncode, 86, interrupted.stdout + interrupted.stderr)
                pending = transactions.read_pending_transaction(root)
                self.assertEqual(pending['manifest']['schema_version'], 'tos_selected_metadata_transaction_v2')
                self.assertEqual(pending['plan']['path_profile'], plan['path_profile'])
                recovered = self.child(root, plan, operation=operation)
                self.assertEqual(recovered.returncode, 0, recovered.stdout + recovered.stderr)
                retained = transactions.inspect_transaction(root, IDENTIFIER)
                self.assertEqual(retained['plan']['path_profile'], plan['path_profile'])
                self.assertEqual(retained['status'], 'committed' if operation == 'resume' else 'rolled-back')
                self.assert_side(root, plan, 'after' if operation == 'resume' else 'before')
                item = Path(plan['path_profile']['item_source_path']).parent
                self.assertEqual((root / item / 'payload/original.bin').read_bytes(),
                                 b'synthetic deposited bytes outside metadata authority')

    def test_manifest_versions_cannot_reinterpret_each_others_path_grammar(self):
        plan = item_fixture(self.root)
        interrupted = self.child(self.root, plan, edge='after-file-3')
        self.assertEqual(interrupted.returncode, 86, interrupted.stdout + interrupted.stderr)
        retained = transactions.read_pending_transaction(self.root)
        manifest = retained['manifest']
        downgraded = copy.deepcopy(manifest)
        downgraded['schema_version'] = 'tos_selected_metadata_transaction_v1'
        with self.assertRaises(transactions.TransactionCorruption):
            transactions._validate_manifest(downgraded, IDENTIFIER)
        for schema in ('tos_selected_metadata_transaction_v1', 'tos_selected_metadata_transaction_v2'):
            unprofiled = copy.deepcopy(manifest)
            unprofiled['schema_version'] = schema
            unprofiled['plan'].pop('path_profile')
            with self.assertRaises((ValueError, PermissionError)):
                transactions._validate_manifest(unprofiled, IDENTIFIER)
        # Even an all-JSON plan has one explicit interpretation, not a v2 fallback.
        legacy_root = self.root / 'legacy'
        legacy_plan = fixture(legacy_root)
        self.assertEqual(self.child(legacy_root, legacy_plan, edge='after-pending').returncode, 86)
        legacy = transactions.read_pending_transaction(legacy_root)['manifest']
        legacy['schema_version'] = 'tos_selected_metadata_transaction_v2'
        with self.assertRaises(transactions.TransactionCorruption):
            transactions._validate_manifest(legacy, IDENTIFIER)

    def test_byte_file_and_authorization_budgets_are_checked_before_staging(self):
        invalid = copy.deepcopy(self.plan)
        invalid['files'][0]['after'] = b'x' * (transactions.MAX_SIDE_BYTES + 1)
        with self.assertRaises(ValueError):
            self.apply(invalid)
        invalid['files'][0]['after'] = b'x' * (transactions.MAX_SIDE_BYTES // 2 + 1)
        invalid['files'][1]['after'] = b'y' * (transactions.MAX_SIDE_BYTES // 2 + 1)
        with self.assertRaises(ValueError):
            self.apply(invalid)
        invalid = copy.deepcopy(self.plan)
        invalid['files'] = [{'path': WORK + '/new-' + str(index) + '.json', 'before': None, 'after': b'{}'}
                            for index in range(transactions.MAX_FILES + 1)]
        with self.assertRaises(ValueError):
            self.apply(invalid)
        invalid = copy.deepcopy(self.plan)
        invalid['authorization']['oversize'] = 'x' * transactions.MAX_AUTHORIZATION_BYTES
        with self.assertRaises(ValueError):
            self.apply(invalid)
        self.assertIsNone(publication.read_publication_state(self.root))
        self.assertFalse((self.root / publication.TRANSACTIONS_REF).exists())

    def test_symlink_file_or_ancestor_never_redirects_selected_writes(self):
        path = self.root / self.plan['files'][0]['path']
        outside = self.root / 'outside.json'
        outside.write_bytes(self.plan['files'][0]['before'])
        path.unlink()
        path.symlink_to(outside)
        with self.assertRaises((OSError, ValueError)):
            self.apply()
        self.assertEqual(outside.read_bytes(), self.plan['files'][0]['before'])
        self.assertIsNone(publication.read_publication_state(self.root))

    def test_revocation_before_and_midway_never_clears_pending(self):
        self.authorized = False
        with self.assertRaises(PermissionError):
            self.apply()
        self.assertIsNone(publication.read_publication_state(self.root))
        self.authorized = True
        original = transactions._replace_file
        def revoke(*args):
            original(*args)
            self.authorized = False
        with patch.object(transactions, '_replace_file', side_effect=revoke):
            with self.assertRaises(PermissionError):
                self.apply()
        pending = transactions.read_pending_transaction(self.root)
        self.assertIsNotNone(pending)
        with self.assertRaises(PermissionError):
            self.recover()
        self.assertEqual(transactions.read_pending_transaction(self.root)['state'], pending['state'])
        with self.assertRaises(publication.PublicationPending):
            publication.PublicationSnapshot(self.root)
        self.authorized = True
        self.assertEqual(self.recover()['status'], 'committed')

    def test_guard_drift_before_pending_and_third_state_midway_are_not_overwritten(self):
        path = self.root / self.plan['files'][0]['path']
        calls = 0
        def drift(authority, summary):
            nonlocal calls
            calls += 1
            if calls == 2:
                path.write_bytes(b'owner edit before pending')
            return True
        with self.assertRaises(transactions.TransactionConflict):
            self.apply(guard=drift)
        self.assertEqual(path.read_bytes(), b'owner edit before pending')
        self.assertIsNone(publication.read_publication_state(self.root))
        # Restore only the synthetic fixture, then interrupt an actual mutation.
        path.write_bytes(self.plan['files'][0]['before'])
        self.pause_after_first_file()
        path.write_bytes(b'third state belongs to its editor')
        with self.assertRaises(transactions.TransactionConflict):
            self.recover()
        with self.assertRaises(transactions.TransactionConflict):
            self.recover(rollback=True)
        self.assertEqual(path.read_bytes(), b'third state belongs to its editor')
        self.assertIsNotNone(transactions.read_pending_transaction(self.root))

    def test_rollback_restores_exact_bytes_removes_only_declared_empty_dirs_and_changes_epoch(self):
        pending = self.pause_after_first_file()
        self.assertEqual(pending['plan']['files'], sorted(self.plan['files'], key=lambda item: item['path']))
        result = self.recover(rollback=True)
        self.assert_side(self.root, self.plan, 'before')
        self.assertFalse((self.root / (WORK + '/expressions')).exists())
        self.assertEqual(result['status'], 'rolled-back')
        self.assertGreater(result['publication']['generation'], pending['state']['generation'])
        self.assertNotEqual(result['publication']['token'], pending['state']['token'])
        with self.assertRaises(publication.PublicationChanged):
            self.snapshot.verify_current()
        self.assertEqual(self.apply()['status'], 'rolled-back')
        with self.assertRaises(transactions.TransactionConflict):
            self.recover(rollback=True)

    def test_rollback_does_not_delete_an_unselected_new_descendant(self):
        self.pause_after_first_file()
        other = self.root / (WORK + '/expressions/fixture/unselected.json')
        other.write_bytes(b'owner content outside selected plan')
        with self.assertRaises(OSError):
            self.recover(rollback=True)
        self.assertEqual(other.read_bytes(), b'owner content outside selected plan')
        self.assertIsNotNone(transactions.read_pending_transaction(self.root))
        self.assert_side(self.root, self.plan, 'before')

    def test_fresh_process_recovers_every_selected_write_and_control_edge(self):
        edges = ('after-pending', 'before-file-1', 'after-file-1', 'after-file-2', 'after-file-3',
                 'after-rename-before-directory-fsync', 'after-ready')
        for edge in edges:
            with self.subTest(edge=edge):
                root = self.root / edge
                plan = fixture(root)
                original = publication.PublicationSnapshot(root)
                interrupted = self.child(root, plan, edge=edge)
                self.assertEqual(interrupted.returncode, 86, interrupted.stdout + interrupted.stderr)
                if edge == 'after-ready':
                    retained = transactions.inspect_transaction(root, IDENTIFIER)
                    self.assertEqual(retained['status'], 'committed')
                    self.assertFalse((root / publication.TRANSACTIONS_REF / IDENTIFIER[7:] / 'completion.json').exists())
                    # Replay uses the original snapshot, not a newly forged baseline.
                    with corpus_lock(root):
                        reply = transactions.apply_transaction(root, plan, expected_snapshot=original,
                            authorization_guard=lambda *args: True, transaction_id=IDENTIFIER)
                    self.assertTrue(reply['replayed'])
                else:
                    with self.assertRaises(publication.PublicationPending):
                        publication.PublicationSnapshot(root)
                    recovered = self.child(root, plan, operation='resume')
                    self.assertEqual(recovered.returncode, 0, recovered.stdout + recovered.stderr)
                    self.assertEqual(json.loads(recovered.stdout)['status'], 'committed')
                self.assert_side(root, plan, 'after')
                publication.PublicationSnapshot(root).verify_current()

    def test_fresh_process_rollback_changes_aba_epoch(self):
        interrupted = self.child(self.root, self.plan, edge='after-file-2')
        self.assertEqual(interrupted.returncode, 86, interrupted.stderr)
        rolled = self.child(self.root, self.plan, operation='rollback')
        self.assertEqual(rolled.returncode, 0, rolled.stdout + rolled.stderr)
        self.assert_side(self.root, self.plan, 'before')
        with self.assertRaises(publication.PublicationChanged):
            self.snapshot.verify_current()

    def test_rollback_after_an_initialized_ready_epoch_does_not_reuse_its_token(self):
        self.apply()
        before = publication.PublicationSnapshot(self.root)
        plan = {'authorization': AUTHORIZATION, 'new_directories': [], 'files': [{
            'path': WORK + '/work.json', 'before': self.plan['files'][0]['after'], 'after': b'temporary after'}]}
        original = transactions._replace_file
        def stop(*args):
            original(*args)
            raise RuntimeError('synthetic second transaction interruption')
        with patch.object(transactions, '_replace_file', side_effect=stop):
            with self.assertRaises(RuntimeError):
                self.apply(plan, snapshot=before, identifier=publication._digest(b'ABA second transaction'))
        result = self.recover(rollback=True)
        self.assert_side(self.root, plan, 'before')
        self.assertNotEqual(result['publication']['token'], before.token)
        with self.assertRaises(publication.PublicationChanged):
            before.verify_current()

    def test_explicit_file_removal_and_absent_recreation_rollback_are_exact(self):
        plan = {'authorization': AUTHORIZATION, 'new_directories': [], 'files': [
            {'path': WORK + '/work.json', 'before': self.plan['files'][0]['before'], 'after': None},
            {'path': WORK + '/zero.json', 'before': None, 'after': b''}]}
        original = transactions._remove_file
        def stop(*args):
            original(*args)
            raise RuntimeError('synthetic interruption after deletion')
        with patch.object(transactions, '_remove_file', side_effect=stop):
            with self.assertRaises(RuntimeError):
                self.apply(plan)
        self.recover(rollback=True)
        self.assert_side(self.root, plan, 'before')

    def test_parent_path_swap_stops_pending_and_does_not_follow_replacement(self):
        self.pause_after_first_file()
        parent = self.root / WORK
        retained = parent.with_name('work-moved-by-owner')
        parent.rename(retained)
        parent.symlink_to(retained, target_is_directory=True)
        before = (retained / 'work.json').read_bytes()
        with self.assertRaises((OSError, transactions.TransactionConflict)):
            self.recover()
        self.assertEqual((retained / 'work.json').read_bytes(), before)
        self.assertIsNotNone(transactions.read_pending_transaction(self.root))

    def test_recovery_resynchronizes_existing_after_files_before_ready(self):
        self.pause_after_first_file()
        synced = []
        original = os.fsync
        def observe(descriptor):
            synced.append(os.fstat(descriptor).st_ino)
            return original(descriptor)
        already_after = self.root / (WORK + '/expressions/fixture/expression.json')
        inode = already_after.stat().st_ino
        with patch.object(os, 'fsync', side_effect=observe):
            self.recover()
        self.assertIn(inode, synced)

    def test_revocation_during_final_durability_pass_keeps_publication_pending(self):
        original = transactions._sync_selected
        def revoke(parents):
            original(parents)
            self.authorized = False
        with patch.object(transactions, '_sync_selected', side_effect=revoke):
            with self.assertRaises(PermissionError):
                self.apply()
        self.assert_side(self.root, self.plan, 'after')
        self.assertIsNotNone(transactions.read_pending_transaction(self.root))
        self.authorized = True
        self.assertEqual(self.recover()['status'], 'committed')

    def test_lost_completion_is_durable_before_a_later_transaction_supersedes_head(self):
        with patch.object(transactions, '_record_completion', side_effect=RuntimeError('lost final response')):
            with self.assertRaises(RuntimeError):
                self.apply()
        first = publication.read_publication_state(self.root)
        self.assertEqual(first['phase'], 'ready')
        self.assertFalse((self.root / publication.TRANSACTIONS_REF / IDENTIFIER[7:] / 'completion.json').exists())
        second = {'authorization': AUTHORIZATION, 'new_directories': [], 'files': [{
            'path': WORK + '/work.json', 'before': self.plan['files'][0]['after'], 'after': b'{"record_version": 3}\n'}]}
        identifier = publication._digest(b'synthetic next command')
        original = transactions._publish_state
        def pause(root, state, expected):
            original(root, state, expected)
            if state['phase'] == 'pending':
                raise RuntimeError('next transaction interrupted')
        with patch.object(transactions, '_publish_state', side_effect=pause):
            with self.assertRaises(RuntimeError):
                self.apply(second, snapshot=publication.PublicationSnapshot(self.root), identifier=identifier)
        historical = transactions.inspect_transaction(self.root, IDENTIFIER)
        self.assertEqual(historical['publication'], first)
        self.assertFalse(historical['is_current_publication'])
        self.recover()
        old_replay = self.apply()
        self.assertTrue(old_replay['replayed'])
        self.assertFalse(old_replay['current_selected_bytes_verified'])
        self.assertEqual((self.root / (WORK + '/work.json')).read_bytes(), second['files'][0]['after'])

    def test_renewed_recovery_authorization_is_retained_without_rewriting_original_authority(self):
        self.pause_after_first_file()
        self.authorized = False
        renewal = {'owner_configuration': publication._digest(b'explicit-renewed-owner'),
                   'authority_ref': 'test:explicit-recovery-only'}
        with self.assertRaises(PermissionError):
            self.recover(recovery_authorization=renewal)
        def renewed_guard(original, summary):
            self.assertEqual(original, AUTHORIZATION)
            return renewal['authority_ref'] == 'test:explicit-recovery-only'
        result = self.recover(recovery_authorization=renewal, authorization_guard=renewed_guard)
        self.assertEqual(result['publication']['recovery_authorization'], renewal)
        retained = transactions.inspect_transaction(self.root, IDENTIFIER)
        self.assertEqual(retained['manifest']['plan']['authorization'], AUTHORIZATION)
        self.assertEqual(retained['publication']['recovery_authorization'], renewal)

    def test_corrupt_or_missing_blob_state_and_third_state_stop_recovery(self):
        self.pause_after_first_file()
        directory = self.root / publication.TRANSACTIONS_REF / IDENTIFIER[7:]
        retained = transactions.read_pending_transaction(self.root)
        digest = retained['manifest']['plan']['files'][0]['after']['sha256']
        blob = directory / (digest[7:] + '.blob')
        original = blob.read_bytes()
        blob.write_bytes(b'corrupt')
        with self.assertRaises((ValueError, OSError)):
            self.recover()
        with self.assertRaises((ValueError, OSError)):
            self.apply()  # A retry must not repair a damaged retained blob from caller bytes.
        blob.unlink()
        with self.assertRaises(transactions.TransactionCorruption):
            self.apply()
        blob.write_bytes(original)
        control = self.root / publication.CONTROL_REF
        state = json.loads(control.read_bytes())
        state['generation'] += 1
        control.write_text(json.dumps(state))
        with self.assertRaises(publication.PublicationStateError):
            publication.PublicationSnapshot(self.root)
        with self.assertRaises(publication.PublicationStateError):
            self.recover()

    def test_orphan_never_becomes_implicit_recovery_or_cleanup_authority(self):
        with patch.object(transactions, '_publish_state', side_effect=RuntimeError('before pending')):
            with self.assertRaises(RuntimeError):
                self.apply()
        retained = transactions.inspect_transaction(self.root, IDENTIFIER)
        self.assertEqual(retained['status'], 'orphan')
        self.assertIsNone(transactions.read_pending_transaction(self.root))
        self.assertIsNone(publication.PublicationSnapshot(self.root).token)
        with self.assertRaises(transactions.TransactionConflict):
            self.recover()
        self.authorized = False
        with self.assertRaises(PermissionError):
            self.apply()
        self.assert_side(self.root, self.plan, 'before')
        self.authorized = True
        self.assertEqual(self.apply()['status'], 'committed')  # Exact explicit original plan, not automatic orphan recovery.

    def test_historical_parent_owner_is_portable_but_recovery_stays_strict(self):
        self.apply()
        transaction = self.root / publication.TRANSACTIONS_REF / IDENTIFIER[7:]
        historical_root = self.root / 'historical-copy'
        historical_transaction = historical_root / publication.TRANSACTIONS_REF / IDENTIFIER[7:]
        historical_transaction.parent.mkdir(parents=True)
        shutil.copytree(transaction, historical_transaction)
        manifest_path = historical_transaction / 'manifest.json'
        manifest = json.loads(manifest_path.read_bytes())
        for binding in manifest['parents'].values():
            if binding is not None:
                binding['uid'] = 424242
        manifest_raw = publication._canonical(manifest) + b'\n'
        manifest_path.write_bytes(manifest_raw)
        completion_path = historical_transaction / 'completion.json'
        completion = json.loads(completion_path.read_bytes())
        completed = completion['publication']
        completed['manifest_sha256'] = publication._digest(manifest_raw)
        completed['token'] = publication._digest(publication._canonical(
            {key: value for key, value in completed.items() if key != 'token'}))
        completion_path.write_bytes(publication._canonical(completion) + b'\n')

        inspected = transactions.inspect_transaction(historical_root, IDENTIFIER)
        self.assertEqual(inspected['status'], 'committed')
        self.assertTrue(all(binding is None or binding['uid'] == 424242
                            for binding in inspected['manifest']['parents'].values()))
        with self.assertRaises(transactions.TransactionCorruption):
            transactions._validate_manifest(manifest, IDENTIFIER)

        pending_root = self.root / 'pending-foreign-owner'
        pending_plan = fixture(pending_root)
        interrupted = self.child(pending_root, pending_plan, edge='after-pending')
        self.assertEqual(interrupted.returncode, 86, interrupted.stdout + interrupted.stderr)
        pending_manifest_path = pending_root / publication.TRANSACTIONS_REF / IDENTIFIER[7:] / 'manifest.json'
        pending_manifest = json.loads(pending_manifest_path.read_bytes())
        for binding in pending_manifest['parents'].values():
            if binding is not None:
                binding['uid'] = 424242
        pending_manifest_path.write_bytes(publication._canonical(pending_manifest) + b'\n')
        with self.assertRaises(transactions.TransactionCorruption):
            transactions.read_pending_transaction(pending_root)
        with corpus_lock(pending_root):
            with self.assertRaises(transactions.TransactionCorruption):
                transactions.resume_transaction(pending_root, authorization_guard=lambda *args: True,
                                                transaction_id=IDENTIFIER)

    def test_reused_identity_stale_snapshot_and_current_replay_revocation_are_refused(self):
        self.apply()
        changed = copy.deepcopy(self.plan)
        changed['files'][0]['after'] = b'other request'
        with self.assertRaises(transactions.TransactionConflict):
            self.apply(changed)
        next_plan = {'authorization': AUTHORIZATION, 'new_directories': [], 'files': [{
            'path': WORK + '/work.json', 'before': self.plan['files'][0]['after'], 'after': b'next'}]}
        with self.assertRaises(publication.PublicationChanged):
            self.apply(next_plan, identifier=publication._digest(b'another command'))
        self.authorized = False
        with self.assertRaises(PermissionError):
            self.apply()
        self.authorized = True
        (self.root / (WORK + '/work.json')).write_bytes(b'current owner edit')
        with self.assertRaises(transactions.TransactionConflict):
            self.apply()


class PublicationSnapshotTests(unittest.TestCase):
    def test_absent_control_is_read_only_and_rejects_invalid_symlink_or_oversize_state(self):
        with tempfile.TemporaryDirectory(prefix='metadata-snapshot-') as directory:
            root = Path(directory)
            first = publication.PublicationSnapshot(root)
            self.assertIsNone(first.token)
            first.verify_current()
            self.assertEqual(list(root.iterdir()), [])
            control = root / publication.CONTROL_REF
            control.parent.mkdir(parents=True)
            for raw in (b'{}', b'{"duplicate":1,"duplicate":2}', b'[]', b'x' * (publication.MAX_STATE_BYTES + 1)):
                control.write_bytes(raw)
                with self.subTest(raw=raw[:30]), self.assertRaises(publication.PublicationStateError):
                    publication.PublicationSnapshot(root)
            control.unlink()
            control.symlink_to(root / 'absent.json')
            with self.assertRaises(OSError):
                publication.PublicationSnapshot(root)


if __name__ == '__main__':
    unittest.main()

"""Private Claim growth over synthetic sources; no real grant or admission."""
from __future__ import annotations

import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(ROOT), str(ROOT / 'scripts'), str(ROOT / 'mechanics/growth-cycle/tests'),
               str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
from tests.test_source_owner_claim_profiles import OwnerLocalClaimFixture, CLAIM_REF, RELATION_TYPE_ID
from test_occurrence_growth import copy_contracts
import source_commands as source
import source_owner_claim_commands as private


class PrivateClaimCommandTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='tos-private-claim-growth-')
        self.addCleanup(temporary.cleanup)
        self.local = OwnerLocalClaimFixture(Path(temporary.name))
        copy_contracts(self.local.public)
        # Existing reader input remains a separate source; creation uses a new
        # identity and a new, absent package in the same protected store.
        self.claim = copy.deepcopy(self.local.claim)
        self.claim['claim_id'] = 'tos.claim.synthetic.created-private-form'
        self.claim['provenance_event_ref'] = 'tos.event.synthetic.private-claim-creation'
        self.source_ref = str(Path(CLAIM_REF).parent.parent / 'new-growth' / 'source-claims.jsonl')
        self.path = self.local.private / self.source_ref
        self.form_id = 'tos.form.synthetic.private-created-statement'
        selected = self.local.claim_selection(exact=True)
        for key in ('path', 'form_ids'):
            selected.pop(key)
        selected['claim_id'] = self.claim['claim_id']
        self.config = {'schema_version': private.CONFIG, 'uid': os.getuid(),
            'principal_id': self.claim['maker']['agent_ref'], 'maker_type': 'model',
            'authority_ref': 'operator:synthetic-private-claim-growth', 'expires_at': '2099-01-01T00:00:00Z',
            'source_context_ref': str(self.local.config_path), 'source_path': self.source_ref,
            'provenance_event_id': self.claim['provenance_event_ref'],
            'allowed_operations': list(private.OPERATIONS), 'allowed_claim_ids': [self.claim['claim_id']],
            'allowed_subject_refs': [self.claim['subject_ref']], 'allowed_object_refs': [self.claim['object']],
            'allowed_predicates': [self.claim['predicate']], 'allowed_evidence_refs': self.claim['evidence_refs'],
            'allowed_form_ids': [self.form_id], 'allowed_fields': ['qualifiers', 'epistemic_status'],
            'claim_selections': [selected]}
        self.forms = [{'claim_id': self.claim['claim_id'], 'form_id': self.form_id, 'field_id': 'claim.statement'}]
        self.owner = self.local.base / 'owner.json'
        self.write_owner()

    def write_owner(self):
        self.owner.write_bytes(self.local.encode(self.config))
        self.owner.chmod(0o600)

    def run_command(self, request):
        return source.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1', **request})

    def add_sibling(self):
        sibling = copy.deepcopy(self.claim)
        sibling['claim_id'] += '.sibling'
        sibling_form = self.form_id + '.sibling'
        selection = copy.deepcopy(self.config['claim_selections'][0])
        selection['claim_id'] = sibling['claim_id']
        self.config['allowed_claim_ids'].append(sibling['claim_id'])
        self.config['claim_selections'].append(selection)
        self.config['allowed_form_ids'].append(sibling_form)
        self.forms.append({'claim_id': sibling['claim_id'], 'form_id': sibling_form, 'field_id': 'claim.statement'})
        self.write_owner()
        return sibling

    def prepare_create(self, records=None):
        records = [self.claim] if records is None else records
        prepared = self.run_command({'operation': 'prepare-create', 'claims': records, 'forms': self.forms})
        self.creation = {'operation': 'claims.create', 'command_id': 'synthetic-create',
            'claims': records, 'forms': self.forms, 'expected_configuration': prepared['owner_configuration'],
            'expected_source': None, 'expected_revision': None, 'expected_dependencies': prepared['expected_dependencies'],
            'expected_inputs': prepared['source_bindings']}
        return prepared

    def create(self, records=None):
        self.prepare_create(records)
        return self.run_command(self.creation)

    def revise(self, *, apply=True):
        proposal = {'claim_id': self.claim['claim_id'],
            'fields': {'qualifiers': {'statement': 'Исправленная синтетическая возможность; не установленный разбор.'}},
            'forms': [{key: value for key, value in self.forms[0].items() if key != 'claim_id'}],
            'reason': 'Synthetic description correction without a new relation identity.'}
        prepared = self.run_command({'operation': 'prepare-revise', **proposal})
        self.revision = {'operation': 'claim.revise', 'command_id': 'synthetic-revise', **proposal,
            'expected_configuration': prepared['owner_configuration'], 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_dependencies': prepared['expected_dependencies'],
            'expected_inputs': prepared['source_bindings']}
        return self.run_command(self.revision) if apply else prepared

    def prepare_form(self, *, claim_id=None, form_id=None, command_id='synthetic-form'):
        claim_id, form_id = claim_id or self.claim['claim_id'], form_id or self.form_id
        prepared = self.run_command({'operation': 'prepare', 'claim_id': claim_id,
            'form_id': form_id, 'field_id': 'claim.statement'})
        self.form_request = {'operation': 'apply', 'claim_id': claim_id, 'command_id': command_id,
            'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
            'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'], 'expected_inputs': prepared['source_bindings'],
            'changes': [prepared['prepared_change']]}
        return prepared

    def files(self):
        return {path.name: path.read_bytes() for path in self.path.parent.iterdir()}

    def test_later_claim_relation_mismatch_is_rejected_before_any_private_source_read(self):
        from source_owner_context import OwnerLocalSourceContext
        from source_record_profiles import SourceProfileError
        sibling = self.add_sibling()
        self.config['claim_selections'][-1]['relation_type_id'] = 'tos.relation.occurrence-has-sense'
        self.write_owner()
        observed = []
        original = OwnerLocalSourceContext.read_bytes
        def read(context, path, *args, **kwargs):
            observed.append(path)
            return original(context, path, *args, **kwargs)
        with patch.object(OwnerLocalSourceContext, 'read_bytes', read):
            with self.assertRaises((PermissionError, SourceProfileError)):
                self.prepare_create([self.claim, sibling])
        self.assertEqual([path for path in observed if path.is_relative_to(self.local.private)], [])
        self.assertNotIn(self.local.public / self.local.native.content_ref, observed)
        self.assertFalse(self.path.parent.exists())

    def test_corrupt_creation_receipt_refuses_describe_and_prepare_revise(self):
        self.create()
        target = self.path.parent / private.transport.RECEIPT_FILE
        original = target.read_bytes()
        damaged = json.loads(original)
        damaged['files'][self.path.name]['sha256'] = 'sha256:' + '0' * 64
        target.write_bytes(self.local.encode(damaged))
        try:
            for operation in ('describe', 'prepare-revise'):
                with self.subTest(operation=operation), self.assertRaises(source.JournalCorruption):
                    if operation == 'describe':
                        self.run_command({'operation': operation})
                    else:
                        self.revise(apply=False)
        finally:
            target.write_bytes(original)

    def test_retained_configuration_bytes_cannot_change_outside_creation(self):
        self.create()
        target = self.path.parent / private.transport.CONFIG_FILE
        original = target.read_bytes()
        target.write_bytes(original + b'\n')
        try:
            for operation in ('describe', 'prepare-revise'):
                with self.subTest(operation=operation), self.assertRaises(source.JournalCorruption):
                    if operation == 'describe':
                        self.run_command({'operation': operation})
                    else:
                        self.revise(apply=False)
        finally:
            target.write_bytes(original)

    def test_current_initial_claim_and_matching_forms_cannot_replace_created_bytes(self):
        self.create()
        changed = copy.deepcopy(self.claim)
        changed['qualifiers']['statement'] = 'Synthetically replaced outside the source command history.'
        context = private.OwnerLocalSourceContext.load(self.local.config_path)
        change = source.prepare_claim_change(changed, None, self.config['principal_id'], self.form_id, 'claim.statement')
        forms = source._apply(None, private.revisions._subject(changed), [change],
                              validator=private.transport._form_grammar(context)[0])
        self.path.write_bytes(source._canonical(changed) + b'\n')
        source.claim_forms_path(self.path, changed['claim_id']).write_bytes(self.local.encode(forms))
        for operation in ('describe', 'prepare-revise'):
            with self.subTest(operation=operation), self.assertRaises(source.JournalCorruption):
                if operation == 'describe':
                    self.run_command({'operation': operation})
                else:
                    self.revise(apply=False)

    def test_two_claim_batch_preserves_sibling_rows_forms_and_all_replays(self):
        sibling = self.add_sibling()
        created = self.create([self.claim, sibling])
        original = self.files()
        sibling_form = source.claim_forms_path(self.path, sibling['claim_id'])
        sibling_row = self.path.read_bytes().splitlines(keepends=True)[1]
        revised = self.revise()
        self.assertEqual(self.path.read_bytes().splitlines(keepends=True)[1], sibling_row)
        self.assertEqual(sibling_form.read_bytes(), original[sibling_form.name])
        first_form = source.claim_forms_path(self.path, self.claim['claim_id'])
        first_form_bytes, revised_stream = first_form.read_bytes(), self.path.read_bytes()
        self.prepare_form(claim_id=sibling['claim_id'], form_id=self.forms[1]['form_id'])
        formed = self.run_command(self.form_request)
        self.assertEqual(formed['source']['id'], sibling['claim_id'])
        self.assertEqual(self.path.read_bytes(), revised_stream)
        self.assertEqual(first_form.read_bytes(), first_form_bytes)
        self.assertEqual(json.loads(sibling_form.read_bytes())['forms'][0]['form_version'], 2)
        before = self.files()
        for request, receipt in ((self.creation, created['receipt']), (self.revision, revised['receipt']),
                                  (self.form_request, formed['receipt'])):
            replay = self.run_command(request)
            self.assertTrue(replay['replayed'])
            self.assertEqual(replay['receipt'], receipt)
            self.assertEqual(before, self.files())
        self.assertEqual({row['id']: row['version'] for row in replay['sources']},
                         {self.claim['claim_id']: 2, sibling['claim_id']: 1})

    def test_duplicate_batch_and_neighbor_owned_claim_identities_refuse_creation(self):
        sibling = self.add_sibling()
        with self.assertRaises(source.JournalConflict):
            self.prepare_create([self.claim, self.claim])
        forms = copy.deepcopy(self.forms)
        self.forms[1]['form_id'] = self.form_id
        with self.assertRaises(PermissionError):
            self.prepare_create([self.claim, sibling])
        self.forms = forms
        self.local.write_claims(self.local.claim, self.claim)
        with self.assertRaises(source.JournalConflict):
            self.prepare_create([self.claim, sibling])
        self.assertFalse(self.path.parent.exists())

    def test_corrupt_retained_archive_blocks_creation_and_revision_replay(self):
        self.create()
        revised = self.revise()
        archive = self.local.private / revised['receipt']['archive_path']
        manifest = json.loads((archive / 'manifest.json').read_bytes())
        blob = archive / manifest['files'][self.path.name]['blob']
        original, before = blob.read_bytes(), self.files()
        blob.write_bytes(b'Corrupted synthetic predecessor bytes.\n')
        try:
            for request in (self.creation, self.revision):
                with self.subTest(operation=request['operation']), self.assertRaises(source.JournalCorruption):
                    self.run_command(request)
            self.assertEqual(self.files(), before)
        finally:
            blob.write_bytes(original)
        self.assertTrue(self.run_command(self.revision)['replayed'])

    def test_command_identity_cannot_cross_creation_form_and_claim_revision(self):
        self.create()
        self.prepare_form()
        for command_id in (self.creation['command_id'],):
            with self.assertRaises(source.JournalConflict):
                self.run_command({**self.form_request, 'command_id': command_id})
        formed = self.run_command(self.form_request)
        self.revise(apply=False)
        for command_id in (self.creation['command_id'], formed['receipt']['command_id']):
            with self.subTest(command_id=command_id), self.assertRaises(source.JournalConflict):
                self.run_command({**self.revision, 'command_id': command_id})
        revised = self.run_command(self.revision)
        self.prepare_form(command_id=revised['receipt']['command_id'])
        before = self.files()
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.form_request)
        self.assertEqual(self.files(), before)

    def test_new_correction_owner_preserves_maker_and_initial_form_authorship(self):
        created = self.create()
        creator = self.config['principal_id']
        self.config.update(principal_id='agent:synthetic-private-claim-corrector',
            authority_ref='operator:synthetic-correction-only', allowed_operations=['claim.revise'])
        self.write_owner()
        revised = self.revise()
        current = json.loads(self.path.read_bytes())
        self.assertEqual(current['maker'], self.claim['maker'])
        self.assertEqual(current['provenance_event_ref'], self.claim['provenance_event_ref'])
        self.assertEqual(revised['receipt']['principal_id'], self.config['principal_id'])
        forms = json.loads(source.claim_forms_path(self.path, self.claim['claim_id']).read_bytes())
        self.assertEqual(forms['forms'][0]['creator_id'], self.config['principal_id'])
        self.assertEqual(forms['prior_forms'][0]['creator_id'], creator)
        self.assertEqual(forms['prior_forms'][0]['subject'], created['sources'][0])
        self.assertTrue(self.run_command(self.revision)['replayed'])
        with self.assertRaises(PermissionError):
            self.run_command(self.creation)

    def test_every_replay_rechecks_late_owner_and_context_revocation(self):
        self.create()
        self.revise()
        self.prepare_form()
        self.run_command(self.form_request)
        before, owner_bytes = self.files(), self.owner.read_bytes()
        original = private._result
        for request in (self.creation, self.revision, self.form_request):
            for revoke in ('owner', 'context'):
                changed = []
                def response(*args, **kwargs):
                    value = original(*args, **kwargs)
                    if kwargs.get('replayed'):
                        changed.append(revoke)
                        if revoke == 'owner':
                            denied = json.loads(owner_bytes)
                            denied['expires_at'] = '2000-01-01T00:00:00Z'
                            self.owner.write_bytes(self.local.encode(denied))
                        else:
                            self.local.config_path.chmod(0o644)
                    return value
                try:
                    with self.subTest(operation=request['operation'], revoke=revoke), \
                            patch.object(private, '_result', side_effect=response), \
                            self.assertRaises((ValueError, PermissionError, source.JournalConflict)):
                        self.run_command(request)
                    self.assertEqual(changed, [revoke])
                    self.assertEqual(self.files(), before)
                finally:
                    self.owner.write_bytes(owner_bytes)
                    self.local.config_path.chmod(0o600)

    def test_process_loss_before_and_after_exchange_preserves_retry_history(self):
        self.create()
        self.revise(apply=False)
        before = self.files()
        script = '''import json, os, sys
from pathlib import Path
root = Path(sys.argv[1])
sys.path[:0] = [str(root / 'scripts'), str(root / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
import source_commands, source_revisions
exchange = source_revisions._exchange
def lose_process(staging, target):
    if sys.argv[3] == 'after':
        exchange(staging, target)
    os._exit(73)
source_revisions._exchange = lose_process
source_commands.run_local_command(Path(sys.argv[2]), json.load(sys.stdin))
'''
        request = json.dumps({'schema_version': 'tos_local_source_command_v1', **self.revision})
        for stage, version in (('before', 1), ('after', 2)):
            with self.subTest(stage=stage):
                child = subprocess.run([sys.executable, '-c', script, str(ROOT), str(self.owner), stage],
                    input=request, text=True, capture_output=True, timeout=60)
                self.assertEqual(child.returncode, 73, child.stderr)
                self.assertEqual(json.loads(self.path.read_bytes())['claim_version'], version)
                if stage == 'before':
                    self.assertEqual(self.files(), before)
        replay = self.run_command(self.revision)
        self.assertTrue(replay['replayed'])
        history = json.loads((self.path.parent / private.revisions.HISTORY).read_bytes())
        self.assertEqual(len(history['receipts']), 1)
        self.assertEqual(history['receipts'][0]['command_id'], self.revision['command_id'])
        self.assertTrue(self.run_command(self.creation)['replayed'])

    def test_create_forms_provenance_and_exact_replay_are_private_and_unadmitted(self):
        prepared = self.prepare_create()
        self.assertFalse(self.path.parent.exists())
        result = self.run_command(self.creation)
        self.assertEqual(json.loads(self.path.read_bytes()), self.claim)
        self.assertEqual(result['sources'], prepared['prepared_sources'])
        self.assertFalse(result['publication_authorized'])
        self.assertFalse(result['grants_admission'])
        self.assertEqual(result['materializations'][0]['display_text'], self.claim['qualifiers']['statement'])
        self.assertTrue(all(view['admission'] is None for view in result['materializations']))
        private_forms = json.loads(source.claim_forms_path(self.path, self.claim['claim_id']).read_bytes())
        public_views = source.materialize_claim_forms(self.claim, private_forms, access_allowed=True)
        self.assertTrue(all(view['state'] == 'restricted' for view in public_views))
        self.assertEqual(self.path.parent.stat().st_mode & 0o777, 0o700)
        self.assertTrue(all(path.stat().st_mode & 0o777 == 0o600 for path in self.path.parent.iterdir()))
        event = json.loads((self.path.parent / 'source-create-provenance.jsonl').read_bytes())
        self.assertEqual(event['activity']['event_type'], 'annotation')
        self.assertEqual(event['rights_and_visibility']['content_visibility'], 'local_only')
        self.assertIn('mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_claim_commands.py',
                      [component['artifact_ref'] for component in event['method']['software_components']])
        before = self.files()
        replay = self.run_command(self.creation)
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], result['receipt'])
        self.assertEqual(before, self.files())

    def test_revision_preserves_original_unknowns_forms_and_creation_replay(self):
        initial = self.create()
        result = self.revise()
        current = json.loads(self.path.read_bytes())
        self.assertEqual(current['claim_version'], 2)
        self.assertEqual(current['extensions'], self.claim['extensions'])
        self.assertEqual(current['qualifiers']['unknown'], self.claim['qualifiers']['unknown'])
        self.assertEqual(current['subject_ref'], self.claim['subject_ref'])
        old = self.run_command({'operation': 'inspect-version', 'claim_id': self.claim['claim_id'],
                                'source': initial['sources'][0]})
        self.assertEqual(old['record'], self.claim)
        self.assertTrue(self.run_command(self.revision)['replayed'])
        self.assertTrue(self.run_command(self.creation)['replayed'])
        self.assertEqual(result['source']['version'], 2)
        form_set = json.loads(source.claim_forms_path(self.path, self.claim['claim_id']).read_bytes())
        self.assertEqual(form_set['forms'][0]['form_version'], 2)
        self.assertEqual(form_set['prior_forms'][0]['subject'], initial['sources'][0])

    def test_stale_source_and_revoked_exact_grant_refuse_without_publication(self):
        self.prepare_create()
        self.config['claim_selections'][0]['source_records'][0]['source_access']['access_allowed'] = False
        self.write_owner()
        with self.assertRaises((ValueError, PermissionError)):
            self.run_command(self.creation)
        self.assertFalse(self.path.parent.exists())

    def test_public_claim_reader_rejects_private_path_before_context_read(self):
        from source_record_profiles import SourceClaimProfiles, SourceProfileError
        profiles = SourceClaimProfiles(self.local.public)
        with patch('source_owner_context.OwnerLocalSourceContext.load', side_effect=AssertionError('private I/O')):
            with self.assertRaises(SourceProfileError):
                list(profiles.read_rows(self.source_ref))

    def test_quote_anchor_needs_independent_grant_before_private_source_reads(self):
        from source_owner_context import OwnerLocalSourceContext
        self.claim['supporting_quotes'] = [{'anchor_ref': self.local.binding['ordered_anchor_refs'][0],
                                           'exact': 'Synthetic authored quote, not authenticated wording.'}]
        observed = []
        original = OwnerLocalSourceContext.read_bytes
        def read(context, path, *args, **kwargs):
            if path.is_relative_to(self.local.private):
                observed.append(path)
            return original(context, path, *args, **kwargs)
        with patch.object(OwnerLocalSourceContext, 'read_bytes', read):
            with self.assertRaises(PermissionError):
                self.prepare_create()
        self.assertEqual(observed, [])
        self.assertFalse(self.path.parent.exists())

    def test_prepared_claim_refuses_changed_endpoint_then_replay_allows_neighbor_growth(self):
        self.prepare_create()
        self.local.form['notes'] = 'Same identity, changed synthetic source description.'
        self.local.native.write_json('ToS/source-witnesses/lexical-descriptions/synthetic/lexical-form.json', self.local.form)
        with self.assertRaises(source.JournalConflict):
            self.run_command(self.creation)
        self.assertFalse(self.path.parent.exists())
        created = self.create()
        neighbor = copy.deepcopy(self.local.claim)
        neighbor['claim_id'] = 'tos.claim.synthetic.unrelated-neighbor'
        self.local.write_claims(self.local.claim, neighbor)
        replayed = self.run_command(self.creation)
        self.assertEqual(replayed['receipt'], created['receipt'])
        self.assertTrue(replayed['replayed'])


if __name__ == '__main__':
    unittest.main()

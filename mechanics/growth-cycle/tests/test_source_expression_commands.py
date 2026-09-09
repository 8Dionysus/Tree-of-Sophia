"""Native compound growth on tiny synthetic sources, never the live corpus."""
from __future__ import annotations

import copy
import hashlib
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
for directory in (ROOT / 'scripts', MECHANIC):
    sys.path.insert(0, str(directory))

import source_commands as commands
import source_expression_commands as compound
import source_revisions as revisions
from source_record_profiles import SourceClaimProfiles, SourceRecordProfiles
from source_metadata_snapshot import PublicationSnapshot
from source_metadata_snapshot import PublicationPending, PublicationChanged


CRASH_WRITER = r'''
import json, os, sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import source_commands as commands
import source_metadata_transactions as tx
original = tx._replace_file
count = 0
def replace(*args):
    global count
    original(*args)
    count += 1
    if count == int(sys.argv[3]): os._exit(86)
tx._replace_file = replace
commands.run_local_command(Path(sys.argv[2]), json.load(sys.stdin))
'''


class NativeExpressionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.work_ref = 'ToS/source-witnesses/works/synthetic/parent/work.json'
        self.work_path = self.root / self.work_ref
        self.work = {'schema_version': 'tos_corpus_record_v1', 'record_type': 'work',
            'record_id': 'tos.work.synthetic.parent', 'record_version': 4,
            'preferred_label': 'Synthetic parent', 'variant_labels': [], 'identity_status': 'verified',
            'source_refs': ['https://example.invalid/source'], 'external_identifiers': [],
            'same_as_posture': 'no_equivalence_claim', 'expression_claim_refs': [],
            'responsibility_claim_refs': ['tos.claim.synthetic.authorship'],
            'chronology_claim_refs': ['tos.claim.synthetic.chronology'], 'supersedes_ref': None,
            'notes': 'Synthetic qualified wording, preserved by the compound operation.',
            'field_languages': {name: {'language': 'ru', 'script': 'Cyrl'}
                                for name in ('preferred_label', 'notes')}}
        self.write(self.work_ref, self.work)
        profiles = SourceClaimProfiles(ROOT)
        profiles.validate(self.claim())
        refs = {*profiles.input_digests, *SourceRecordProfiles(ROOT).input_digests,
                'ToS/contracts/corpus-record.schema.json', *compound.FORM_CONTRACTS,
                'ToS/contracts/provenance-event-v2.schema.json'}
        for ref in refs:
            self.write(ref, (ROOT / ref).read_bytes())
        self.owner = self.root / 'compound-owner.json'
        self.config = {'schema_version': compound.CONFIG, 'uid': os.getuid(),
            'principal_id': 'model:synthetic', 'maker_type': 'model', 'source_root': str(self.root),
            'authority_ref': 'test-only:bounded-native-compound-not-assessment',
            'expires_at': '2099-01-01T00:00:00Z',
            'allowed_operations': [compound.OPERATION, compound.RECOVERY],
            'work_id': self.work['record_id'], 'work_source_path': self.work_ref,
            'allowed_work_form_ids': ['tos.form.synthetic.parent.name', 'tos.form.synthetic.parent.note']}
        self.select_child('first')
        forms = [{'form_id': self.config['allowed_work_form_ids'][index], 'field_id': field}
                 for index, field in enumerate(('metadata.preferred-name', 'metadata.source-note'))]
        changes = [commands.prepare_metadata_change(self.work, None, 'model:synthetic', **item) for item in forms]
        value = commands._apply(None, commands.Record.from_payload(self.work['record_id'], 4, self.work), changes)
        self.write(self.work_path.with_name('work.human-forms.json').relative_to(self.root).as_posix(), value)
        self.original_forms = copy.deepcopy(value)
        # A nested old descendant is neither transaction input nor touched output.
        self.untouched = self.work_path.parent / 'expressions/untouched/editions/child/item.txt'
        self.untouched.parent.mkdir(parents=True)
        self.untouched.write_bytes(b'Unrelated synthetic descendant\n')
        self.rebuild()

    def write(self, ref, value):
        path = self.root / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value if isinstance(value, bytes) else revisions._encode(value))

    def select_child(self, suffix):
        base = self.work_path.parent.relative_to(self.root) / 'expressions' / suffix
        self.config.update(expression_id='tos.expression.synthetic.' + suffix,
            expression_source_path=str(base / 'expression.json'), claim_id='tos.claim.synthetic.' + suffix,
            provenance_event_id='tos.event.synthetic.' + suffix,
            allowed_expression_form_ids=['tos.form.synthetic.' + suffix + '.name'],
            allowed_claim_form_ids=['tos.form.synthetic.' + suffix + '.statement'])
        self.owner.write_text(json.dumps(self.config))

    def claim(self, suffix='first'):
        child = str(Path(self.work_ref).parent / 'expressions' / suffix / 'expression.json')
        return {'schema_version': 'tos_source_relation_claim_v1', 'claim_id': 'tos.claim.synthetic.' + suffix,
            'claim_type': 'relation', 'claim_version': 1, 'assertion_layer': 'bibliographic_assertion',
            'predicate': 'has_expression', 'subject_ref': 'tos.work.synthetic.parent',
            'object': 'tos.expression.synthetic.' + suffix, 'epistemic_status': 'observed',
            'polarity': 'positive', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
            'maker': {'maker_type': 'model', 'agent_ref': 'model:synthetic'},
            'provenance_event_ref': 'tos.event.synthetic.' + suffix,
            'evidence_refs': [self.work_ref, child], 'assessment_refs': [],
            'qualifiers': {'statement': 'The synthetic records declare this link, without textual equivalence.',
                'statement_language': 'en', 'statement_script': 'Latn',
                'unknown_qualification': {'flag': False, 'missing': None, 'text': 'Keep this context.'}}}

    def proposal(self):
        suffix = Path(self.config['expression_source_path']).parent.name
        expression = {**copy.deepcopy(self.work), 'record_id': self.config['expression_id'],
            'record_type': 'expression', 'record_version': 1, 'identity_status': 'provisional',
            'preferred_label': 'Синтетическое английское выражение', 'work_ref': self.work['record_id'],
            'language': 'en', 'expression_role': 'translation', 'responsibility_claim_refs': [],
            'embodiment_claim_refs': [], 'derivation_claim_refs': []}
        expression.pop('expression_claim_refs')
        expression.pop('chronology_claim_refs')
        return {'schema_version': compound.REQUEST, 'operation': 'prepare-create', 'record': expression,
            'claim': self.claim(suffix), 'forms': [
                {'form_id': self.config['allowed_work_form_ids'][0], 'field_id': 'metadata.preferred-name'},
                {'form_id': self.config['allowed_work_form_ids'][1], 'field_id': 'metadata.source-note'}],
            'expression_forms': [{'form_id': self.config['allowed_expression_form_ids'][0],
                                  'field_id': 'metadata.preferred-name'}],
            'claim_forms': [{'form_id': self.config['allowed_claim_form_ids'][0], 'field_id': 'claim.statement'}],
            'reason': 'Synthetic exact typed child addition; no admission.'}

    def request(self):
        proposal = self.proposal()
        prepared = commands.run_local_command(self.owner, proposal)
        return {**proposal, 'operation': compound.OPERATION, 'command_id': self.config['expression_id'],
            'fields': prepared['prepared_fields'], 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'],
            'expected_publication': prepared['expected_publication']}

    def rebuild(self):
        """Tiny exact manifest projection, not the whole repository builder."""
        records = {'work': [], 'expression': []}
        claims = []
        paths = [self.work_path, *self.work_path.parent.glob('expressions/*/expression.json')]
        for path in paths:
            record = json.loads(path.read_bytes())
            records[record['record_type']].append({'schema_version': 'tos_source_witness_catalog_entry_v1',
                'record_id': record['record_id'],
                'record_type': record['record_type'], 'source_record_ref': path.relative_to(self.root).as_posix(),
                'preferred_label': record['preferred_label'], 'identity_status': record['identity_status'],
                'record_sha256': hashlib.sha256(commands._canonical(record)).hexdigest(),
                'links': {'work_ref': record['work_ref']} if 'work_ref' in record else {}})
            carrier = path.with_name('source-claims.jsonl')
            if carrier.exists():
                for number, line in enumerate(carrier.read_bytes().splitlines(), 1):
                    claim = json.loads(line)
                    claims.append({**claim, 'schema_version': 'tos_source_witness_claim_catalog_entry_v1',
                        'source_claim_file_ref': carrier.relative_to(self.root).as_posix(),
                        'source_claim_line': number, 'claim_sha256': hashlib.sha256(commands._canonical(claim)).hexdigest()})
        manifest = {'schema_version': 'tos_source_witness_catalog_v3',
                    'record_files': {kind: 'ToS/source-witnesses/catalog/' + kind + 's.jsonl' for kind in records},
                    'claim_file': 'ToS/source-witnesses/catalog/claims.jsonl'}
        digests = {}
        for kind, rows in [*records.items(), ('claim', claims)]:
            ref = manifest['claim_file'] if kind == 'claim' else manifest['record_files'][kind]
            raw = b''.join(commands._canonical(row) + b'\n' for row in rows)
            self.write(ref, raw)
            digests[ref] = hashlib.sha256(raw).hexdigest()
        snapshot = PublicationSnapshot(self.root)
        if snapshot.token is not None:
            manifest['selected_metadata_publication'] = {'protocol': revisions.SELECTED_PROTOCOL,
                'token': snapshot.token, 'files': digests}
        self.write(compound.CATALOG_MANIFEST, manifest)

    def test_prepare_create_current_read_and_fresh_process_replay(self):
        before = self.work_path.read_bytes()
        request = self.request()
        self.assertEqual(self.work_path.read_bytes(), before)
        result = commands.run_local_command(self.owner, request)
        self.assertFalse(result['grants_admission'])
        self.assertEqual(result['source_profiles']['work']['type_id'], 'tos.entity.work')
        self.assertEqual(result['source_profiles']['expression']['type_id'], 'tos.entity.expression')
        self.assertEqual(result['source_profiles']['has_expression']['relation_type_id'], 'tos.relation.has-expression')
        self.assertEqual(result['source_profiles']['has_expression']['schema_ref'],
                         'ToS/contracts/source-relation-claim.schema.json')
        self.assertTrue(result['source_fields'])
        self.assertTrue(all('pointer' not in field and 'context' not in field for field in result['source_fields']))
        revised = json.loads(self.work_path.read_bytes())
        self.assertEqual(revised, {**self.work, 'record_version': 5,
            'expression_claim_refs': [self.config['claim_id']]})
        forms = json.loads(self.work_path.with_name('work.human-forms.json').read_bytes())
        self.assertEqual(forms['prior_forms'], self.original_forms['forms'])
        self.assertTrue(all(view['state'] == 'ready' for group in result['materializations'].values() for view in group))
        self.assertEqual(result['materializations']['expression'][0]['language'], 'ru')
        verified = compound.verify_compound(self.root,
            str(Path(self.config['expression_source_path']).with_name('source-claims.jsonl')), request['claim'])
        self.assertFalse(verified['grants_admission'])
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        self.assertTrue(json.loads(process.stdout)['replayed'])
        self.assertEqual(self.untouched.read_bytes(), b'Unrelated synthetic descendant\n')

    def test_exact_replay_after_catalog_rebuild_and_later_sibling(self):
        first_config = copy.deepcopy(self.config)
        first = self.request()
        result = commands.run_local_command(self.owner, first)
        self.rebuild()
        self.assertTrue(commands.run_local_command(self.owner, first)['replayed'])
        self.select_child('second')
        commands.run_local_command(self.owner, self.request())
        self.rebuild()
        self.config = first_config
        self.owner.write_text(json.dumps(self.config))
        replay = commands.run_local_command(self.owner, first)
        self.assertTrue(replay['replayed'])
        self.assertEqual(replay['receipt'], result['receipt'])
        self.assertEqual(replay['source']['version'], 6)

    def crash(self, request, edge=2):
        process = subprocess.run([sys.executable, '-c', CRASH_WRITER, str(MECHANIC), str(self.owner), str(edge)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 86, process.stdout + process.stderr)
        with self.assertRaises(PublicationPending):
            PublicationSnapshot(self.root)
        return compound.transactions.read_pending_transaction(self.root)

    def recovery(self, pending, decision):
        return {'schema_version': compound.REQUEST, 'operation': compound.RECOVERY,
            'transaction_id': pending['manifest']['transaction_id'], 'decision': decision,
            'expected_configuration': compound.configuration(self.config)[1]}

    def test_fresh_process_resume_revalidates_exact_pending_request(self):
        import validate_source_witness_foundation as foundation
        request = self.request()
        pending = self.crash(request)
        with self.assertRaises(PublicationPending):
            commands.run_local_command(self.owner, {'schema_version': compound.REQUEST, 'operation': 'describe'})
        with patch.object(foundation, '_validate_foundation') as inner:
            self.assertTrue(foundation.validate_foundation(self.root))
            inner.assert_not_called()
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(self.recovery(pending, 'resume')), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        self.assertFalse(json.loads(process.stdout)['grants_admission'])
        self.assertTrue(commands.run_local_command(self.owner, request)['replayed'])

    def test_pending_rollback_restores_parent_without_removing_descendants(self):
        before = revisions._selected_package(self.work_path)
        pending = self.crash(self.request(), edge=10)
        result = commands.run_local_command(self.owner, self.recovery(pending, 'rollback'))
        self.assertIsNone(result['receipt'])
        self.assertEqual(revisions._selected_package(self.work_path), before)
        self.assertFalse((self.root / self.config['expression_source_path']).parent.exists())
        self.assertTrue(self.untouched.exists())
        self.assertEqual(compound.transactions.inspect_transaction(self.root,
            pending['manifest']['transaction_id'])['status'], 'rolled-back')

    def test_pending_revocation_expiry_and_dependency_drift_refuse_recovery(self):
        pending = self.crash(self.request())
        original = copy.deepcopy(self.config)
        self.config['allowed_operations'] = []
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, self.recovery(pending, 'resume'))
        self.config = {**copy.deepcopy(original), 'expires_at': '2000-01-01T00:00:00Z'}
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, {'schema_version': compound.REQUEST, 'operation': 'describe'})
        self.config = original
        self.owner.write_text(json.dumps(self.config))
        selected = self.root / 'ToS/contracts/corpus-record.schema.json'
        raw = selected.read_bytes()
        selected.write_bytes(raw + b'\n')
        with self.assertRaises(commands.JournalConflict):
            commands.run_local_command(self.owner, self.recovery(pending, 'resume'))
        selected.write_bytes(raw)
        commands.run_local_command(self.owner, self.recovery(pending, 'rollback'))

    def test_pending_third_state_is_not_overwritten_by_resume_or_rollback(self):
        pending = self.crash(self.request())
        expression = self.root / self.config['expression_source_path']
        initial = expression.read_bytes()
        expression.write_bytes(b'{"unrelated": "external third state"}\n')
        for decision in ('resume', 'rollback'):
            with self.subTest(decision=decision), self.assertRaises((ValueError, OSError)):
                commands.run_local_command(self.owner, self.recovery(pending, decision))
        self.assertEqual(expression.read_bytes(), b'{"unrelated": "external third state"}\n')
        expression.write_bytes(initial)
        commands.run_local_command(self.owner, self.recovery(pending, 'rollback'))

    def test_prepare_rejects_nonprovisional_child_wrong_fields_and_stale_catalog(self):
        initial = self.work_path.read_bytes()
        proposal = self.proposal()
        bad = copy.deepcopy(proposal)
        bad['record']['identity_status'] = 'verified'
        with self.assertRaises(ValueError):
            commands.run_local_command(self.owner, bad)
        bad = copy.deepcopy(proposal)
        bad['forms'] = bad['forms'][:1]
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, bad)
        request = self.request()
        changed = copy.deepcopy(request)
        changed['fields']['preferred_label'] = 'Unauthorized rewrite'
        with self.assertRaises(ValueError):
            commands.run_local_command(self.owner, changed)
        self.assertEqual(self.work_path.read_bytes(), initial)
        commands.run_local_command(self.owner, request)
        self.select_child('second')
        with self.assertRaises(ValueError):
            self.request()

    def test_native_carrier_requires_exact_committed_capture_and_source_bytes(self):
        from validate_source_witness_foundation import _native_topology_claims
        request = self.request()
        carrier_ref = str(Path(self.config['expression_source_path']).with_name('source-claims.jsonl'))
        self.write(carrier_ref, commands._canonical(request['claim']) + b'\n')
        issues = []
        self.assertEqual(_native_topology_claims(self.root, issues), [])
        self.assertTrue(issues)
        (self.root / carrier_ref).unlink()
        (self.root / carrier_ref).parent.rmdir()
        commands.run_local_command(self.owner, request)
        issues = []
        self.assertEqual(len(_native_topology_claims(self.root, issues)), 1)
        self.assertEqual(issues, [])
        # A normal script entry adds only scripts/, not the mechanic test path.
        process = subprocess.run([sys.executable, '-c',
            'import sys; from pathlib import Path; sys.path.insert(0,sys.argv[1]); '
            'import validate_source_witness_foundation as f; issues=[]; '
            'assert len(f._native_topology_claims(Path(sys.argv[2]),issues)) == 1; '
            'assert not issues; from metadata_version_reader import MetadataVersionReader',
            str(ROOT / 'scripts'), str(self.root)], text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        capture = (self.root / carrier_ref).with_name(compound.PROVENANCE_FILE)
        capture.write_bytes(capture.read_bytes() + b'\n')
        issues = []
        self.assertEqual(_native_topology_claims(self.root, issues), [])
        self.assertTrue(issues)

    def test_prepared_dependency_drift_and_midpublication_revocation_fail_closed(self):
        request = self.request()
        read = commands._read
        def changed_grammar(path, limit):
            raw = read(path, limit)
            return raw + b'\n' if Path(path) == commands.ROOT / commands.contract.MODULE_REF else raw
        with patch.object(commands, '_read', side_effect=changed_grammar), self.assertRaises(commands.JournalConflict):
            commands.run_local_command(self.owner, request)
        self.assertEqual(json.loads(self.work_path.read_bytes()), self.work)
        manifest = self.root / compound.CATALOG_MANIFEST
        before = manifest.read_bytes()
        manifest.write_bytes(before + b'\n')
        with self.assertRaises(commands.JournalConflict):
            commands.run_local_command(self.owner, request)
        self.assertEqual(json.loads(self.work_path.read_bytes()), self.work)
        manifest.write_bytes(before)
        original = compound.transactions._replace_file
        def revoke(*args):
            original(*args)
            self.config['allowed_operations'] = []
            self.owner.write_text(json.dumps(self.config))
        with patch.object(compound.transactions, '_replace_file', side_effect=revoke), self.assertRaises(ValueError):
            commands.run_local_command(self.owner, request)
        self.assertIsNotNone(compound.transactions.read_pending_transaction(self.root))

    def correct_expression(self):
        config = {key: self.config[key] for key in ('uid', 'principal_id', 'source_root',
                  'authority_ref', 'expires_at')}
        config.update(schema_version=commands.CORPUS_SELECTED_REVISION_CONFIG,
            record_type='expression', record_id=self.config['expression_id'],
            source_path=self.config['expression_source_path'], allowed_fields=['notes'],
            allowed_operations=['record.revise'], allowed_form_ids=self.config['allowed_expression_form_ids'])
        owner = self.root / 'correction-owner.json'
        owner.write_text(json.dumps(config))
        proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
            'fields': {'notes': 'Corrected synthetic descriptive note, no semantic admission.'},
            'forms': self.proposal()['expression_forms'], 'reason': 'Synthetic descriptive correction.'}
        prepared = commands.run_local_command(owner, proposal)
        request = {**proposal, 'operation': 'record.revise', 'command_id': 'synthetic:expression-correction',
            'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
            'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'],
            'expected_publication': prepared['publication_snapshot']}
        return commands.run_local_command(owner, request)

    def test_descriptive_expression_successor_retains_exact_initial_bytes(self):
        from metadata_version_reader import MetadataVersionReader
        initial_work_raw = self.work_path.read_bytes()
        request = self.request()
        commands.run_local_command(self.owner, request)
        self.correct_expression()
        self.rebuild()
        verified = compound.verify_compound(self.root,
            str(Path(self.config['expression_source_path']).with_name('source-claims.jsonl')), request['claim'])
        self.assertEqual(verified['receipt']['expression']['version'], 1)
        reader = MetadataVersionReader(self.root)
        historical = reader.resolve_source_bytes(self.work_ref, hashlib.sha256(initial_work_raw).hexdigest())
        self.assertEqual(historical['status'], 'available', historical)
        self.assertEqual(historical['record'], self.work)
        self.assertIsNotNone(historical['provenance']['source']['archive_blob_ref'])
        reader.verify_current()

    def test_canonical_reference_without_exact_initial_archive_bytes_is_not_enough(self):
        request = self.request()
        commands.run_local_command(self.owner, request)
        expression = self.root / self.config['expression_source_path']
        # Same ID/version/canonical digest, but no longer the created byte layer.
        expression.write_bytes(expression.read_bytes() + b'\n')
        self.correct_expression()
        with self.assertRaisesRegex(commands.JournalCorruption, 'exact compound bytes'):
            compound.verify_compound(self.root,
                str(Path(self.config['expression_source_path']).with_name('source-claims.jsonl')), request['claim'])

    def test_committed_evidence_survives_copy_without_owner_grant_or_original_inodes(self):
        request = self.request()
        commands.run_local_command(self.owner, request)
        with tempfile.TemporaryDirectory() as destination:
            relocated = Path(destination) / 'relocated'
            shutil.copytree(self.root, relocated)
            (relocated / self.owner.name).unlink()
            self.assertNotEqual((relocated / self.work_ref).stat().st_ino, self.work_path.stat().st_ino)
            verified = compound.verify_compound(relocated,
                str(Path(self.config['expression_source_path']).with_name('source-claims.jsonl')), request['claim'])
            self.assertFalse(verified['grants_admission'])
            self.assertFalse(verified['writes_to_source'])

    def test_recovery_only_renewal_does_not_grant_new_creation(self):
        request = self.request()
        pending = self.crash(request)
        self.config['allowed_operations'] = [compound.RECOVERY]
        self.config['principal_id'] = 'model:synthetic-recovery'
        self.config['authority_ref'] = 'test-only:renewed-recovery-not-creation'
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, request)
        recovered = commands.run_local_command(self.owner, self.recovery(pending, 'resume'))
        self.assertEqual(recovered['receipt']['principal_id'], 'model:synthetic')
        self.assertEqual(recovered['recovery']['publication']['recovery_authorization']['principal_id'],
                         'model:synthetic-recovery')

    def test_prepare_checks_original_snapshot_after_full_result_assembly(self):
        request = self.request()
        original_result = compound._result
        committed = False
        def commit_during_result(*args, **kwargs):
            nonlocal committed
            if not committed:
                committed = True
                commands.run_local_command(self.owner, request)
            return original_result(*args, **kwargs)
        with patch.object(compound, '_result', side_effect=commit_during_result):
            with self.assertRaises(PublicationChanged):
                commands.run_local_command(self.owner, self.proposal())
        self.assertTrue(committed)
        self.assertEqual(json.loads(self.work_path.read_bytes())['record_version'], 5)


if __name__ == '__main__':
    unittest.main()

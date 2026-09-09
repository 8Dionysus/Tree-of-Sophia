"""Qualified translator attachment on tiny synthetic metadata, never the corpus."""
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
import source_responsibility_commands as attachment
import source_revisions as revisions
from source_record_profiles import SourceClaimProfiles, SourceRecordProfiles
from source_metadata_snapshot import PublicationSnapshot, PublicationPending, PublicationChanged
from source_bibliographic_responsibility import validate_expression_responsibility_closure

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


class NativeResponsibilityTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.expression_ref = 'ToS/source-witnesses/works/synthetic/parent/expressions/english/expression.json'
        self.agent_ref = 'ToS/source-witnesses/agents/synthetic-translator/agent.json'
        self.expression = {'schema_version': 'tos_corpus_record_v1', 'record_type': 'expression',
            'record_id': 'tos.expression.synthetic.english', 'record_version': 1,
            'preferred_label': 'Synthetic English expression', 'variant_labels': [], 'identity_status': 'provisional',
            'source_refs': ['https://example.invalid/reported-attribution'], 'external_identifiers': [],
            'same_as_posture': 'no_equivalence_claim', 'work_ref': 'tos.work.synthetic.parent',
            'language': 'en', 'expression_role': 'translation', 'responsibility_claim_refs': [],
            'embodiment_claim_refs': [], 'derivation_claim_refs': [], 'supersedes_ref': None,
            'notes': 'Qualified synthetic metadata, not a historical source or accepted translation.',
            'field_languages': {name: {'language': 'ru', 'script': 'Cyrl'} for name in ('preferred_label', 'notes')}}
        self.agent = {key: copy.deepcopy(self.expression[key]) for key in ('schema_version', 'record_version',
            'preferred_label', 'variant_labels', 'identity_status', 'source_refs', 'external_identifiers',
            'same_as_posture', 'supersedes_ref', 'notes', 'field_languages')}
        self.agent.update(record_type='agent', record_id='tos.agent.synthetic.translator', preferred_label='Synthetic translator')
        self.write(self.expression_ref, self.expression)
        self.write(self.agent_ref, self.agent)
        self.expression_path = self.root / self.expression_ref
        self.agent_path = self.root / self.agent_ref
        profiles = SourceClaimProfiles(ROOT)
        for ref in {*profiles.input_digests, *SourceRecordProfiles(ROOT).input_digests,
                    'ToS/contracts/corpus-record.schema.json', *attachment.FORM_CONTRACTS,
                    'ToS/contracts/claim-packet.schema.json', 'ToS/contracts/source-relation-claim.schema.json',
                    'ToS/contracts/source-claim-record.schema.json', 'ToS/contracts/knowledge-assessment.schema.json',
                    'ToS/contracts/provenance-event-v2.schema.json'}:
            self.write(ref, (ROOT / ref).read_bytes())
        self.owner = self.root / 'responsibility-owner.json'
        self.config = {'schema_version': attachment.CONFIG, 'uid': os.getuid(), 'principal_id': 'model:synthetic',
            'maker_type': 'model', 'source_root': str(self.root), 'authority_ref': 'test-only:translator-attachment',
            'expires_at': '2099-01-01T00:00:00Z', 'allowed_operations': [attachment.OPERATION, attachment.RECOVERY],
            'expression_id': self.expression['record_id'], 'expression_source_path': self.expression_ref,
            'agent_id': self.agent['record_id'], 'agent_source_path': self.agent_ref, 'predicate': 'translated_by',
            'allowed_expression_form_ids': ['tos.form.synthetic.expression.name'],
            'allowed_evidence_refs': ['https://example.invalid/reported-attribution']}
        self.select_claim('first')
        self.forms = [{'form_id': self.config['allowed_expression_form_ids'][0], 'field_id': 'metadata.preferred-name'}]
        value, _, _ = attachment._forms(self.expression, None, self.forms, self.config['principal_id'])
        self.write(self.expression_path.with_name('expression.human-forms.json').relative_to(self.root).as_posix(), value)
        self.original_forms = copy.deepcopy(value)
        self.untouched = self.expression_path.parent / 'source-claims.jsonl'
        old_link = {**self.claim(), 'claim_id': 'tos.claim.synthetic.parent-link', 'predicate': 'has_expression',
            'subject_ref': self.expression['work_ref'], 'object': self.expression['record_id'],
            'assertion_layer': 'bibliographic_assertion', 'epistemic_status': 'observed',
            'provenance_event_ref': 'tos.event.synthetic.parent-link'}
        self.untouched.write_bytes(commands._canonical(old_link) + b'\n')
        self.rebuild()

    def write(self, ref, value):
        path = self.root / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value if isinstance(value, bytes) else revisions._encode(value))

    def select_claim(self, suffix):
        self.config.update(claim_id='tos.claim.synthetic.translator.' + suffix,
            claim_source_path='ToS/source-witnesses/relations/synthetic-translator-' + suffix + '/source-claims.jsonl',
            provenance_event_id='tos.event.synthetic.translator.' + suffix,
            allowed_claim_form_ids=['tos.form.synthetic.translator.' + suffix + '.statement'])
        (self.root / 'ToS/source-witnesses/relations').mkdir(parents=True, exist_ok=True)
        self.owner.write_text(json.dumps(self.config))

    def claim(self):
        return {'schema_version': 'tos_source_relation_claim_v1', 'claim_id': self.config['claim_id'],
            'claim_type': 'relation', 'claim_version': 1, 'assertion_layer': 'scholarly_report',
            'predicate': 'translated_by', 'subject_ref': self.config['expression_id'], 'object': self.config['agent_id'],
            'epistemic_status': 'reported', 'polarity': 'positive', 'review_status': 'unreviewed',
            'visibility': 'public_metadata_only', 'maker': {'maker_type': 'model', 'agent_ref': 'model:synthetic'},
            'provenance_event_ref': self.config['provenance_event_id'], 'evidence_refs': self.config['allowed_evidence_refs'],
            'assessment_refs': [], 'qualifiers': {'statement': 'The selected provider attributes this translation to the selected person.',
                'statement_language': 'ru', 'statement_script': 'Cyrl',
                'attribution_scope': 'Provider attribution only; later electronic editing is not attributed to the translator.',
                'unknown_qualification': {'flag': False, 'missing': None, 'text': 'Keep this context.'}}}

    def proposal(self):
        return {'schema_version': attachment.REQUEST, 'operation': attachment.PREPARE,
            'agent': copy.deepcopy(self.agent), 'claim': self.claim(), 'forms': self.forms,
            'claim_forms': [{'form_id': self.config['allowed_claim_form_ids'][0], 'field_id': 'claim.statement'}],
            'reason': 'Synthetic qualified translator attribution without source admission.'}

    def request(self):
        proposal = self.proposal()
        prepared = commands.run_local_command(self.owner, proposal)
        return {**proposal, 'operation': attachment.OPERATION, 'command_id': self.config['claim_id'],
            'fields': prepared['prepared_fields'], 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'], 'expected_publication': prepared['expected_publication']}

    def rebuild(self):
        records, claims = {'expression': [], 'agent': []}, []
        for path in (self.expression_path, self.agent_path):
            record = json.loads(path.read_bytes())
            records[record['record_type']].append({'schema_version': 'tos_source_witness_catalog_entry_v1',
                'record_id': record['record_id'], 'record_type': record['record_type'],
                'source_record_ref': path.relative_to(self.root).as_posix(),
                'preferred_label': record['preferred_label'], 'identity_status': record['identity_status'],
                'record_sha256': hashlib.sha256(commands._canonical(record)).hexdigest(), 'links': {}})
        for path in (self.root / 'ToS/source-witnesses/relations').glob('*/source-claims.jsonl'):
            for number, line in enumerate(path.read_bytes().splitlines(), 1):
                claim = json.loads(line)
                claims.append({**claim, 'schema_version': 'tos_source_witness_claim_catalog_entry_v1',
                    'source_claim_file_ref': path.relative_to(self.root).as_posix(), 'source_claim_line': number,
                    'claim_sha256': hashlib.sha256(commands._canonical(claim)).hexdigest()})
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
        self.write(attachment.common.CATALOG_MANIFEST, manifest)

    def test_prepare_attach_current_read_and_fresh_process_replay(self):
        before, agent_before, untouched = self.expression_path.read_bytes(), self.agent_path.read_bytes(), self.untouched.read_bytes()
        request = self.request()
        self.assertEqual(self.expression_path.read_bytes(), before)
        result = commands.run_local_command(self.owner, request)
        self.assertFalse(result['grants_admission'])
        self.assertEqual(result['source_profiles']['translated_by']['relation_type_id'], 'tos.relation.translated-by')
        self.assertEqual(result['source_profiles']['agent']['type_id'], 'tos.entity.agent')
        self.assertEqual(json.loads(self.expression_path.read_bytes()), {**self.expression, 'record_version': 2,
            'responsibility_claim_refs': [self.config['claim_id']]})
        self.assertEqual(self.agent_path.read_bytes(), agent_before)
        self.assertEqual(self.untouched.read_bytes(), untouched)
        forms = json.loads(self.expression_path.with_name('expression.human-forms.json').read_bytes())
        self.assertEqual(forms['prior_forms'], self.original_forms['forms'])
        self.assertTrue(all(view['state'] == 'ready' for group in result['materializations'].values() for view in group))
        verified = attachment.verify_compound(self.root, self.config['claim_source_path'], request['claim'])
        from source_witness_bibliographic_graph_common import _event_node, canonical_digest
        projected_event = _event_node({'payload': verified['event'], 'source_line': 1,
            'source_ref': str(Path(self.config['claim_source_path']).with_name(attachment.PROVENANCE_FILE)),
            'source_sha256': canonical_digest(verified['event'])}, repo_root=self.root)
        self.assertEqual(projected_event['properties']['source_event'], verified['event'])
        self.assertEqual(verified['receipt']['agent_source_binding']['source_sha256'], commands._digest(agent_before))
        self.assertEqual(verified['claim']['evidence_refs'], ['https://example.invalid/reported-attribution'])
        self.rebuild()
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        self.assertTrue(json.loads(process.stdout)['replayed'])

    def test_competing_claims_keep_distinct_identity_and_exact_append_order(self):
        first = self.request()
        original_config = copy.deepcopy(self.config)
        commands.run_local_command(self.owner, first)
        self.rebuild()
        self.select_claim('second')
        second = self.request()
        second['claim']['epistemic_status'] = 'disputed'
        # Re-prepare changed wording/status, rather than reusing a stale digest.
        with patch.object(self, 'claim', return_value=second['claim']):
            second = self.request()
        commands.run_local_command(self.owner, second)
        self.rebuild()
        expression = json.loads(self.expression_path.read_bytes())
        self.assertEqual(expression['responsibility_claim_refs'], [first['claim']['claim_id'], second['claim']['claim_id']])
        validate_expression_responsibility_closure(expression, {self.agent['record_id']: self.agent}, [first['claim'], second['claim']])
        self.config = original_config
        self.owner.write_text(json.dumps(self.config))
        self.assertTrue(commands.run_local_command(self.owner, first)['replayed'])
        with self.assertRaises(ValueError):
            validate_expression_responsibility_closure(expression, {self.agent['record_id']: self.agent}, [first['claim']])
        with self.assertRaises(ValueError):
            validate_expression_responsibility_closure(expression, {self.agent['record_id']: self.agent}, [first['claim'], first['claim']])

    def test_current_grant_packet_and_qualified_delta_are_fail_closed(self):
        before = self.expression_path.read_bytes()
        alterations = [
            lambda p: p['agent'].update(preferred_label='Injected alias'),
            lambda p: p['agent'].update(record_id='tos.agent.synthetic.other'),
            lambda p: p['claim'].update(object='tos.agent.synthetic.other'),
            lambda p: p['claim'].update(predicate='authored_by'),
            lambda p: p['claim'].update(review_status='accepted'),
            lambda p: p['claim'].update(evidence_refs=['https://example.invalid/not-delegated']),
            lambda p: p['claim']['qualifiers'].update(attribution_scope=''),
            lambda p: p['claim']['qualifiers'].update(statement_language=''),
            lambda p: p['forms'][0].update(form_id='tos.form.synthetic.not-delegated'),
        ]
        for alteration in alterations:
            proposal = copy.deepcopy(self.proposal())
            alteration(proposal)
            with self.subTest(alteration=alteration), self.assertRaises((ValueError, OSError)):
                commands.run_local_command(self.owner, proposal)
            self.assertEqual(self.expression_path.read_bytes(), before)
        request = self.request()
        request['fields']['responsibility_claim_refs'].append('tos.claim.synthetic.extra')
        with self.assertRaises(ValueError):
            commands.run_local_command(self.owner, request)
        from source_claim_commands import _scope
        with self.assertRaises(PermissionError):
            _scope({'allowed_operations': ['claims.create'], 'source_root': str(self.root)}, [self.claim()])
        import source_owner_claim_commands
        for initial in (True, False):
            with self.subTest(private_initial=initial), self.assertRaises(PermissionError):
                source_owner_claim_commands._scope({}, [self.claim()], SourceClaimProfiles(self.root), initial=initial)

    def crash(self, request, edge=2):
        process = subprocess.run([sys.executable, '-c', CRASH_WRITER, str(MECHANIC), str(self.owner), str(edge)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 86, process.stdout + process.stderr)
        with self.assertRaises(PublicationPending):
            PublicationSnapshot(self.root)
        return attachment.transactions.read_pending_transaction(self.root)

    def recovery(self, pending, decision):
        return {'schema_version': attachment.REQUEST, 'operation': attachment.RECOVERY,
            'transaction_id': pending['manifest']['transaction_id'], 'decision': decision,
            'expected_configuration': attachment.configuration(self.config)[1]}

    def test_fresh_process_resume_and_recovery_only_renewal(self):
        request = self.request()
        pending = self.crash(request)
        with self.assertRaises(PublicationPending):
            commands.run_local_command(self.owner, {'schema_version': attachment.REQUEST, 'operation': 'describe'})
        self.config['allowed_operations'] = [attachment.RECOVERY]
        self.config['principal_id'] = 'model:synthetic-recoverer'
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, request)
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(self.recovery(pending, 'resume')), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        result = json.loads(process.stdout)
        self.assertEqual(result['receipt']['principal_id'], 'model:synthetic')
        self.assertEqual(result['recovery']['publication']['recovery_authorization']['principal_id'], 'model:synthetic-recoverer')

    def test_partial_parent_rollback_preserves_existing_carrier(self):
        before, untouched = revisions._selected_package(self.expression_path), self.untouched.read_bytes()
        # Six child files precede the parent paths; edge eight reaches the parent.
        pending = self.crash(self.request(), edge=8)
        result = commands.run_local_command(self.owner, self.recovery(pending, 'rollback'))
        self.assertIsNone(result['receipt'])
        self.assertEqual(revisions._selected_package(self.expression_path), before)
        self.assertEqual(self.untouched.read_bytes(), untouched)
        self.assertFalse((self.root / self.config['claim_source_path']).parent.exists())

    def test_pending_authority_dependency_and_third_state_are_revalidated(self):
        pending = self.crash(self.request())
        original = copy.deepcopy(self.config)
        self.config['allowed_operations'] = []
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, self.recovery(pending, 'resume'))
        self.config = {**copy.deepcopy(original), 'expires_at': '2000-01-01T00:00:00Z'}
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, {'schema_version': attachment.REQUEST, 'operation': 'describe'})
        self.config = original
        self.owner.write_text(json.dumps(self.config))
        raw = self.agent_path.read_bytes()
        self.agent_path.write_bytes(raw + b'\n')
        with self.assertRaises((ValueError, OSError)):
            commands.run_local_command(self.owner, self.recovery(pending, 'resume'))
        self.agent_path.write_bytes(raw)
        selected = next(item for item in pending['plan']['files'] if (self.root / item['path']).exists()
                        and item['before'] is None)
        path = self.root / selected['path']
        original_bytes = path.read_bytes()
        path.write_bytes(b'External third-state bytes.\n')
        for decision in ('resume', 'rollback'):
            with self.subTest(decision=decision), self.assertRaises((ValueError, OSError)):
                commands.run_local_command(self.owner, self.recovery(pending, decision))
            self.assertEqual(path.read_bytes(), b'External third-state bytes.\n')
        path.write_bytes(original_bytes)
        commands.run_local_command(self.owner, self.recovery(pending, 'rollback'))

    def test_native_responsibility_requires_exact_committed_capture(self):
        from validate_source_witness_foundation import _native_responsibility_claims
        request = self.request()
        carrier = self.root / self.config['claim_source_path']
        self.write(self.config['claim_source_path'], commands._canonical(request['claim']) + b'\n')
        issues = []
        self.assertEqual(_native_responsibility_claims(self.root, issues), [])
        self.assertTrue(issues)
        carrier.unlink()
        carrier.parent.rmdir()
        commands.run_local_command(self.owner, request)
        issues = []
        self.assertEqual(len(_native_responsibility_claims(self.root, issues)), 1, issues)
        self.assertEqual(issues, [])
        capture = carrier.with_name(attachment.PROVENANCE_FILE)
        capture.write_bytes(capture.read_bytes() + b'\n')
        issues = []
        self.assertEqual(_native_responsibility_claims(self.root, issues), [])
        self.assertTrue(issues)

    def test_preparation_snapshot_survives_full_response_assembly(self):
        request = self.request()
        original_result = attachment._result
        fired = False
        def publish_during_result(*args, **kwargs):
            nonlocal fired
            if not fired:
                fired = True
                commands.run_local_command(self.owner, request)
            return original_result(*args, **kwargs)
        with patch.object(attachment, '_result', side_effect=publish_during_result), self.assertRaises(PublicationChanged):
            self.request()

    def correct_agent(self):
        config = {key: self.config[key] for key in ('uid', 'principal_id', 'source_root', 'authority_ref', 'expires_at')}
        config.update(schema_version=commands.CORPUS_SELECTED_REVISION_CONFIG, record_type='agent',
            record_id=self.config['agent_id'], source_path=self.agent_ref, allowed_fields=['notes'],
            allowed_operations=['record.revise'], allowed_form_ids=['tos.form.synthetic.agent.name'])
        owner = self.root / 'agent-correction-owner.json'
        owner.write_text(json.dumps(config))
        proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
            'fields': {'notes': 'Corrected synthetic Agent description; no change of identity.'},
            'forms': [{'form_id': 'tos.form.synthetic.agent.name', 'field_id': 'metadata.preferred-name'}],
            'reason': 'Synthetic descriptive Agent successor.'}
        prepared = commands.run_local_command(owner, proposal)
        return commands.run_local_command(owner, {**proposal, 'operation': 'record.revise', 'command_id': 'synthetic:agent-correction',
            'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
            'expected_configuration': prepared['owner_configuration'], 'expected_dependencies': prepared['expected_dependencies'],
            'expected_publication': prepared['publication_snapshot']})

    def test_agent_successor_resolves_exact_initial_bytes_and_requires_committed_history(self):
        from metadata_version_reader import MetadataVersionReader
        agent_raw, expression_raw = self.agent_path.read_bytes(), self.expression_path.read_bytes()
        request = self.request()
        commands.run_local_command(self.owner, request)
        self.correct_agent()
        self.rebuild()
        verified = attachment.verify_compound(self.root, self.config['claim_source_path'], request['claim'])
        self.assertEqual(verified['receipt']['agent_source_binding']['source_sha256'], commands._digest(agent_raw))
        self.assertTrue(commands.run_local_command(self.owner, request)['replayed'])
        reader = MetadataVersionReader(self.root)
        old_expression = reader.resolve_source_bytes(self.expression_ref, hashlib.sha256(expression_raw).hexdigest())
        self.assertEqual(old_expression['status'], 'available', old_expression)
        self.assertEqual(old_expression['record'], self.expression)
        old_agent = reader.resolve_source_bytes(self.agent_ref, hashlib.sha256(agent_raw).hexdigest())
        self.assertEqual(old_agent['status'], 'available', old_agent)
        self.assertEqual(old_agent['record'], self.agent)
        reader.verify_current()
        agent_history = json.loads(self.agent_path.with_name(revisions.HISTORY).read_bytes())
        identifier = agent_history['receipts'][0]['publication']['transaction_id']
        original_inspect = attachment.transactions.inspect_transaction
        def orphan(root, selected):
            result = original_inspect(root, selected)
            return {**result, 'status': 'orphan'} if selected == identifier else result
        with patch.object(attachment.transactions, 'inspect_transaction', side_effect=orphan), self.assertRaises(commands.JournalCorruption):
            attachment.verify_compound(self.root, self.config['claim_source_path'], request['claim'])
        # Same canonical JSON is insufficient if the actual initial raw layer
        # was removed from the real continuous history.
        history_path = self.agent_path.with_name(revisions.HISTORY)
        retained = history_path.read_bytes()
        history_path.unlink()
        with self.assertRaises(commands.JournalCorruption):
            attachment.verify_compound(self.root, self.config['claim_source_path'], request['claim'])
        history_path.write_bytes(retained)

    def test_claim_correction_uses_existing_continuous_history_not_a_same_id_substitution(self):
        request = self.request()
        commands.run_local_command(self.owner, request)
        self.rebuild()
        config = {key: self.config[key] for key in ('uid', 'principal_id', 'source_root', 'authority_ref', 'expires_at')}
        config.update(schema_version=commands.CLAIM_REVISION_CONFIG, source_path=self.config['claim_source_path'],
            claim_id=self.config['claim_id'], allowed_operations=['claim.revise'], allowed_fields=['qualifiers'],
            allowed_evidence_refs=self.config['allowed_evidence_refs'], allowed_form_ids=self.config['allowed_claim_form_ids'])
        owner = self.root / 'claim-correction-owner.json'
        owner.write_text(json.dumps(config))
        proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
            'fields': {'qualifiers': {'statement': 'Corrected wording of the same qualified provider attribution.'}},
            'forms': self.proposal()['claim_forms'], 'reason': 'Synthetic qualified statement correction.'}
        prepared = commands.run_local_command(owner, proposal)
        external = prepared['source_bindings']['evidence'][self.config['allowed_evidence_refs'][0]]
        self.assertEqual(external['citation_status'], 'candidate_claim')
        self.assertIsNone(external['remote_content_sha256'])
        self.assertFalse(external['resolved'])
        self.assertEqual(external['source_ref'], self.config['claim_source_path'])
        correction = {**proposal, 'operation': 'claim.revise', 'command_id': 'synthetic:claim-correction',
            'expected_configuration': prepared['owner_configuration'], 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_dependencies': prepared['expected_dependencies'],
            'expected_inputs': prepared['source_bindings']}
        original_read = commands._read
        def changed_validation(path, *args, **kwargs):
            raw = original_read(path, *args, **kwargs)
            return raw + b'\n' if Path(path) == ROOT / 'scripts/source_bibliographic_responsibility.py' else raw
        with patch.object(commands, '_read', side_effect=changed_validation), self.assertRaises(commands.JournalConflict):
            commands.run_local_command(owner, correction)
        result = commands.run_local_command(owner, correction)
        claim = result['materializations'][0]['context'][0]['value']
        self.assertEqual(claim['claim_version'], 2)
        verified = attachment.verify_compound(self.root, self.config['claim_source_path'], claim)
        self.assertEqual(verified['claim'], claim)
        self.assertEqual(verified['receipt']['claim']['version'], 1)
        self.assertTrue(commands.run_local_command(self.owner, request)['replayed'])
        for value in ('', None, False):
            invalid = {**proposal, 'fields': {'qualifiers': {'attribution_scope': value}}}
            with self.subTest(attribution_scope=value), self.assertRaises(ValueError):
                commands.run_local_command(owner, invalid)
        self.rebuild()
        self.select_claim('second')
        commands.run_local_command(self.owner, self.request())
        first_path = self.root / config['source_path']
        history = first_path.with_name('claim-revision-history.json')
        history.unlink()
        with self.assertRaises(commands.JournalCorruption):
            attachment.verify_compound(self.root, config['source_path'], claim)


if __name__ == '__main__':
    unittest.main()

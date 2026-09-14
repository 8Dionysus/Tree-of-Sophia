"""Edition growth and longitudinal native receipts on bounded synthetic sources."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
for directory in (ROOT / 'scripts', MECHANIC, Path(__file__).parent):
    sys.path.insert(0, str(directory))

import test_source_expression_commands as expression_fixture
import source_commands as commands
import source_edition_commands as edition
import source_revisions as revisions
from source_metadata_snapshot import PublicationSnapshot, PublicationPending
from metadata_version_reader import MetadataVersionReader, resolve_metadata_version


class NativeEditionTests(unittest.TestCase):
    def setUp(self):
        # Use the real earlier command, not forged native history or copied live data.
        self.origin = expression_fixture.NativeExpressionTests('runTest')
        self.origin.setUp()
        self.addCleanup(self.origin.doCleanups)
        self.root, self.write = self.origin.root, self.origin.write
        self.origin_request = self.origin.request()
        commands.run_local_command(self.origin.owner, self.origin_request)
        self.expression_ref = self.origin.config['expression_source_path']
        self.expression_path = self.root / self.expression_ref
        self.expression = json.loads(self.expression_path.read_bytes())
        self.original_work = self.origin.work_path.read_bytes()
        self.original_origin = self.expression_path.with_name('source-claims.jsonl').read_bytes()
        self.owner = self.root / 'edition-owner.json'
        self.config = {key: copy.deepcopy(self.origin.config[key]) for key in
            ('uid', 'principal_id', 'maker_type', 'source_root', 'authority_ref', 'expires_at',
             'work_id', 'work_source_path', 'expression_id', 'expression_source_path', 'allowed_expression_form_ids')}
        self.config.update(schema_version=edition.CONFIG, allowed_operations=[edition.OPERATION, edition.RECOVERY])
        self.extra_records, self.extra_claims = [], []
        self.select_child('first')
        self.rebuild()

    def select_child(self, suffix):
        base = Path(self.expression_ref).parent / 'editions' / suffix
        self.config.update(edition_id='tos.edition.synthetic.' + suffix, edition_source_path=str(base / 'edition.json'),
            claim_id='tos.claim.synthetic.edition.' + suffix, provenance_event_id='tos.event.synthetic.edition.' + suffix,
            allowed_edition_form_ids=['tos.form.synthetic.edition.' + suffix + '.name'],
            allowed_claim_form_ids=['tos.form.synthetic.edition.' + suffix + '.statement'])
        self.owner.write_text(json.dumps(self.config))

    def proposal(self):
        record = {key: copy.deepcopy(self.expression[key]) for key in
            ('schema_version', 'record_version', 'variant_labels', 'identity_status', 'source_refs',
             'external_identifiers', 'same_as_posture', 'supersedes_ref', 'notes', 'field_languages')}
        record.update(record_type='edition', record_id=self.config['edition_id'], record_version=1,
            preferred_label='Synthetic electronic edition', embodies_expression_refs=[self.config['expression_id']],
            publication_claim_refs=[], exemplar_claim_refs=[])
        claim = {**copy.deepcopy(self.origin_request['claim']), 'claim_id': self.config['claim_id'],
            'predicate': 'embodied_by', 'subject_ref': self.config['expression_id'], 'object': self.config['edition_id'],
            'provenance_event_ref': self.config['provenance_event_id'],
            'evidence_refs': [self.expression_ref, self.config['edition_source_path']]}
        return {'schema_version': edition.REQUEST, 'operation': edition.PREPARE,
            'record': record, 'claim': claim,
            'forms': [{'form_id': self.config['allowed_expression_form_ids'][0], 'field_id': 'metadata.preferred-name'}],
            'edition_forms': [{'form_id': self.config['allowed_edition_form_ids'][0], 'field_id': 'metadata.preferred-name'}],
            'claim_forms': [{'form_id': self.config['allowed_claim_form_ids'][0], 'field_id': 'claim.statement'}],
            'reason': 'Synthetic declared electronic manifestation; not a printing, Item, File or admitted truth.'}

    def request(self):
        proposal = self.proposal()
        result = commands.run_local_command(self.owner, proposal)
        return {**proposal, 'operation': edition.OPERATION, 'command_id': self.config['claim_id'],
            'fields': result['prepared_fields'], 'expected_source': result['source'],
            'expected_revision': result['revision'], 'expected_configuration': result['owner_configuration'],
            'expected_dependencies': result['expected_dependencies'], 'expected_publication': result['expected_publication']}

    def rebuild(self):
        """Only tiny fixture records/carriers, never the production builder."""
        records, claims = {'work': [], 'expression': [], 'edition': [], 'agent': []}, []
        paths = [self.origin.work_path, self.expression_path,
                 *self.expression_path.parent.glob('editions/*/edition.json'), *self.extra_records]
        carriers = [self.expression_path.with_name('source-claims.jsonl'), *self.extra_claims]
        for path in paths:
            record = json.loads(path.read_bytes())
            records[record['record_type']].append({'schema_version': 'tos_source_witness_catalog_entry_v1',
                'record_id': record['record_id'], 'record_type': record['record_type'],
                'source_record_ref': path.relative_to(self.root).as_posix(), 'preferred_label': record['preferred_label'],
                'identity_status': record['identity_status'], 'record_sha256': hashlib.sha256(commands._canonical(record)).hexdigest(),
                'links': {key: record[key] for key in ('work_ref', 'embodies_expression_refs') if key in record}})
            if record['record_type'] == 'edition':
                carriers.append(path.with_name('source-claims.jsonl'))
        for carrier in carriers:
            for number, line in enumerate(carrier.read_bytes().splitlines(), 1):
                claim = json.loads(line)
                claims.append({**claim, 'schema_version': 'tos_source_witness_claim_catalog_entry_v1',
                    'source_claim_file_ref': carrier.relative_to(self.root).as_posix(), 'source_claim_line': number,
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
        manifest['selected_metadata_publication'] = {'protocol': revisions.SELECTED_PROTOCOL,
            'token': PublicationSnapshot(self.root).token, 'files': digests}
        self.write(edition.CATALOG_MANIFEST, manifest)

    def assert_origin(self):
        self.assertEqual(self.origin.work_path.read_bytes(), self.original_work)
        self.assertEqual(self.expression_path.with_name('source-claims.jsonl').read_bytes(), self.original_origin)
        result = expression_fixture.compound.verify_compound(self.root,
            self.expression_path.with_name('source-claims.jsonl').relative_to(self.root).as_posix(),
            self.origin_request['claim'])
        self.assertFalse(result['grants_admission'])

    def test_create_exact_read_restart_and_preserved_origin(self):
        before = self.expression_path.read_bytes()
        request = self.request()
        self.assertEqual(before, self.expression_path.read_bytes())
        result = commands.run_local_command(self.owner, request)
        self.assertFalse(result['grants_admission'])
        self.assertEqual(result['source_profiles']['edition']['type_id'], 'tos.entity.edition')
        self.assertEqual(result['source_profiles']['embodied_by']['reader'], 'identity-relation-v1')
        self.assertEqual(json.loads(self.expression_path.read_bytes()), {**self.expression, 'record_version': 2,
            'embodiment_claim_refs': [self.config['claim_id']]})
        self.assertEqual(len(edition.transactions.inspect_transaction(self.root, result['receipt']['transaction_id'])['plan']['files']), 11)
        self.rebuild()
        for reference in (result['receipt']['edition'], request['expected_source'], result['source']):
            resolved = resolve_metadata_version(self.root, reference)
            self.assertEqual(resolved['status'], 'available', resolved)
            self.assertFalse(resolved['grants_current_use'])
        from validate_source_witness_foundation import _topology_evidence_matches
        reader = MetadataVersionReader(self.root)
        self.assertTrue(_topology_evidence_matches(self.root, self.expression_ref,
            [{'ref': self.expression_ref, 'sha256': hashlib.sha256(before).hexdigest()}], reader))
        self.assertFalse(_topology_evidence_matches(self.root, self.expression_ref,
            [{'ref': self.expression_ref, 'sha256': '0' * 64}], reader))
        reader.verify_current()
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        self.assertTrue(json.loads(process.stdout)['replayed'])
        self.assert_origin()
        from validate_source_witness_foundation import _native_topology_claims
        issues = []
        accepted = _native_topology_claims(self.root, issues)
        self.assertEqual(issues, [])
        self.assertEqual({claim['predicate'] for _, claim, _ in accepted}, {'has_expression', 'embodied_by'})

    def test_later_sibling_retains_both_editions_and_exact_old_retry(self):
        first, first_config = self.request(), copy.deepcopy(self.config)
        first_result = commands.run_local_command(self.owner, first)
        self.rebuild()
        self.select_child('second')
        second = self.request()
        commands.run_local_command(self.owner, second)
        self.rebuild()
        self.assertEqual(json.loads(self.expression_path.read_bytes())['embodiment_claim_refs'],
            [first['claim']['claim_id'], second['claim']['claim_id']])
        self.config = first_config
        self.owner.write_text(json.dumps(self.config))
        self.assertTrue(commands.run_local_command(self.owner, first)['replayed'])
        self.assertEqual(resolve_metadata_version(self.root, first_result['source'])['status'], 'available')
        self.assert_origin()

    def test_grant_and_no_ladder_or_acceptance_invention(self):
        before = self.expression_path.read_bytes()
        mutations = [lambda p: p['record'].update(identity_status='verified'),
            lambda p: p['record'].update(exemplar_claim_refs=['tos.claim.item']),
            lambda p: p['record'].update(publication_claim_refs=['tos.claim.date']),
            lambda p: p['record'].update(work_ref=self.config['work_id']),
            lambda p: p['record'].update(embodies_expression_refs=[self.config['expression_id'], 'tos.expression.other']),
            lambda p: p['claim'].update(evidence_refs=[self.expression_ref]),
            lambda p: p['claim'].update(review_status='accepted'),
            lambda p: p['forms'][0].update(form_id='tos.form.not-delegated'),
            lambda p: p['edition_forms'][0].update(form_id=self.config['allowed_expression_form_ids'][0])]
        for mutate in mutations:
            proposal = self.proposal()
            mutate(proposal)
            with self.subTest(mutation=mutate), self.assertRaises((ValueError, OSError)):
                commands.run_local_command(self.owner, proposal)
            self.assertEqual(self.expression_path.read_bytes(), before)
        self.config['allowed_operations'] = []
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, self.proposal())

    def crash(self, request):
        process = subprocess.run([sys.executable, '-c', expression_fixture.CRASH_WRITER,
            str(MECHANIC), str(self.owner), '4'], input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 86, process.stdout + process.stderr)
        with self.assertRaises(PublicationPending):
            PublicationSnapshot(self.root)
        return edition.transactions.read_pending_transaction(self.root)

    def recovery(self, pending, decision):
        return {'schema_version': edition.REQUEST, 'operation': edition.RECOVERY,
            'transaction_id': pending['manifest']['transaction_id'], 'decision': decision,
            'expected_configuration': edition.configuration(self.config)[1]}

    def test_pending_resume_requires_current_exact_recovery_grant(self):
        request = self.request()
        pending = self.crash(request)
        with self.assertRaises(PublicationPending):
            commands.run_local_command(self.owner, {'schema_version': edition.REQUEST, 'operation': 'describe'})
        self.config.update(allowed_operations=[edition.RECOVERY], principal_id='model:synthetic-recoverer')
        self.owner.write_text(json.dumps(self.config))
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, request)
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(self.recovery(pending, 'resume')), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        self.assert_origin()

    def test_pending_rollback_restores_exact_parent_and_does_not_admit_orphan(self):
        before = revisions._selected_package(self.expression_path)
        request = self.request()
        pending = self.crash(request)
        commands.run_local_command(self.owner, self.recovery(pending, 'rollback'))
        self.assertEqual(revisions._selected_package(self.expression_path), before)
        self.assertFalse((self.root / self.config['edition_source_path']).exists())
        self.assert_origin()

    def test_completed_capture_is_immutable_and_changed_dependencies_reject(self):
        request = self.request()
        work = json.loads(self.original_work)
        self.write(self.origin.work_ref, {**work, 'notes': 'Concurrent changed context'})
        with self.assertRaises((ValueError, OSError)):
            commands.run_local_command(self.owner, request)
        self.write(self.origin.work_ref, self.original_work)
        commands.run_local_command(self.owner, request)
        # A re-catalogued manual child rewrite cannot masquerade as retained
        # Edition lineage when preparing a later sibling.
        original_config = copy.deepcopy(self.config)
        child_ref = self.config['edition_source_path']
        child_bytes = (self.root / child_ref).read_bytes()
        self.write(child_ref, {**json.loads(child_bytes), 'notes': 'Unretained rewrite'})
        self.rebuild()
        self.select_child('second')
        with self.assertRaisesRegex(ValueError, 'committed initial bytes'):
            self.request()
        self.write(child_ref, child_bytes)
        self.rebuild()
        self.config = original_config
        self.owner.write_text(json.dumps(self.config))
        capture = (self.root / self.config['edition_source_path']).with_name(edition.ENVIRONMENT_FILE)
        capture.write_bytes(capture.read_bytes() + b' ')
        with self.assertRaises((ValueError, OSError)):
            commands.run_local_command(self.owner, request)

    def test_real_translator_receipt_survives_new_edition_and_exact_old_reads(self):
        import source_responsibility_commands as attachment
        agent = {key: copy.deepcopy(self.expression[key]) for key in ('schema_version', 'record_version',
            'preferred_label', 'variant_labels', 'identity_status', 'source_refs', 'external_identifiers',
            'same_as_posture', 'supersedes_ref', 'notes', 'field_languages')}
        agent.update(record_type='agent', record_id='tos.agent.synthetic.translator', preferred_label='Synthetic translator')
        agent_ref = 'ToS/source-witnesses/agents/translator/agent.json'
        carrier = 'ToS/source-witnesses/relations/translator/source-claims.jsonl'
        self.write(agent_ref, agent)
        self.extra_records.append(self.root / agent_ref)
        (self.root / 'ToS/source-witnesses/relations').mkdir(parents=True)
        self.rebuild()
        config = {key: copy.deepcopy(self.config[key]) for key in ('uid', 'principal_id', 'maker_type',
            'source_root', 'authority_ref', 'expires_at', 'expression_id', 'expression_source_path', 'allowed_expression_form_ids')}
        config.update(schema_version=attachment.CONFIG, allowed_operations=[attachment.OPERATION],
            agent_id=agent['record_id'], agent_source_path=agent_ref, predicate='translated_by',
            claim_id='tos.claim.synthetic.translator', claim_source_path=carrier,
            provenance_event_id='tos.event.synthetic.translator',
            allowed_claim_form_ids=['tos.form.synthetic.translator.statement'],
            allowed_evidence_refs=['https://example.invalid/translator-attribution'])
        owner = self.root / 'translator-owner.json'
        owner.write_text(json.dumps(config))
        claim = {**self.proposal()['claim'], 'claim_id': config['claim_id'], 'predicate': 'translated_by',
            'object': agent['record_id'], 'assertion_layer': 'scholarly_report', 'epistemic_status': 'reported',
            'evidence_refs': config['allowed_evidence_refs'], 'provenance_event_ref': config['provenance_event_id']}
        claim['qualifiers']['attribution_scope'] = 'Reported translation only; later editorial changes are not attributed.'
        proposal = {'schema_version': attachment.REQUEST, 'operation': attachment.PREPARE, 'agent': agent,
            'claim': claim, 'forms': self.proposal()['forms'], 'claim_forms': [
                {'form_id': config['allowed_claim_form_ids'][0], 'field_id': 'claim.statement'}], 'reason': 'Synthetic attribution.'}
        prepared = commands.run_local_command(owner, proposal)
        request = {**proposal, 'operation': attachment.OPERATION, 'command_id': config['claim_id'],
            'fields': prepared['prepared_fields'], 'expected_source': prepared['source'],
            'expected_revision': prepared['revision'], 'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'], 'expected_publication': prepared['expected_publication']}
        result = commands.run_local_command(owner, request)
        self.extra_claims.append(self.root / carrier)
        self.rebuild()
        commands.run_local_command(self.owner, self.request())
        self.rebuild()
        self.assertTrue(commands.run_local_command(owner, request)['replayed'])
        self.assertFalse(attachment.verify_compound(self.root, carrier, claim)['grants_admission'])
        for reference in (prepared['source'], result['source']):
            self.assertEqual(resolve_metadata_version(self.root, reference)['status'], 'available')
        self.assert_origin()

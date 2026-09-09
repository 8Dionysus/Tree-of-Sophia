"""Acquired Item boundaries on tiny synthetic files and real older commands."""
from __future__ import annotations

import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[3]
MECHANIC = ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'
for directory in (ROOT / 'scripts', MECHANIC, Path(__file__).parent):
    sys.path.insert(0, str(directory))
import test_source_edition_commands as edition_fixture
import source_commands as commands
import source_item_commands as item
import source_item_deposit as deposit
from source_metadata_snapshot import PublicationSnapshot
from metadata_version_reader import resolve_metadata_version


def epub_bytes():
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, 'w') as archive:
        archive.writestr('mimetype', 'application/epub+zip')
        archive.writestr('META-INF/container.xml', '<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>')
        archive.writestr('content.opf', '<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata/><manifest><item id="c" href="chapter.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c"/></spine></package>')
        archive.writestr('chapter.xhtml', '<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Synthetic source.</p></body></html>')
    return stream.getvalue()


class NativeItemTests(unittest.TestCase):
    def setUp(self):
        self.origin = edition_fixture.NativeEditionTests('runTest')
        self.origin.setUp()
        self.addCleanup(self.origin.doCleanups)
        self.root, self.write = self.origin.root, self.origin.write
        for name in ('source-item-manifest', 'source-resource-inventory', 'rights-record', 'provenance-event'):
            ref = 'ToS/contracts/' + name + '.schema.json'
            self.write(ref, (ROOT / ref).read_bytes())
        self.edition_request = self.origin.request()
        self.edition_result = commands.run_local_command(self.origin.owner, self.edition_request)
        self.edition_ref = self.origin.config['edition_source_path']
        self.edition_path = self.root / self.edition_ref
        self.edition = json.loads(self.edition_path.read_bytes())
        self.owner = self.root / 'item-owner.json'
        self.payload_root = self.root / 'canonical-payload-root'
        self.payload_root.mkdir()
        recovery = tempfile.TemporaryDirectory()
        self.addCleanup(recovery.cleanup)
        self.recovery_root = Path(recovery.name)
        self.input = self.root / 'already-acquired.epub'
        self.input.write_bytes(epub_bytes())
        self.original_bytes = self.input.read_bytes()
        self.config = {key: copy.deepcopy(self.origin.config[key]) for key in
            ('uid', 'principal_id', 'maker_type', 'source_root', 'authority_ref', 'expires_at',
             'edition_id', 'edition_source_path', 'allowed_edition_form_ids')}
        self.config.update(schema_version=item.CONFIG, allowed_operations=[item.OPERATION, item.RECOVERY],
            payload_root=str(self.payload_root), input_path=str(self.input), recovery_root=str(self.recovery_root),
            payload_authority_ref='test-only:local-retention',
            payload_expires_at='2099-01-01T00:00:00Z', payload_basename='source.epub', original_basename='original.epub',
            media_type='application/epub+zip', byte_size=len(self.original_bytes),
            sha256=hashlib.sha256(self.original_bytes).hexdigest())
        self.select_item('first')
        self.rebuild()

    def select_item(self, suffix):
        base = Path(self.edition_ref).parent / 'items' / suffix
        self.config.update(item_id='tos.item.synthetic.' + suffix, item_source_path=str(base / 'item.json'),
            file_id='tos.file.synthetic.' + suffix, claim_id='tos.claim.synthetic.item.' + suffix,
            rights_id='tos.rights.synthetic.' + suffix, provenance_event_id='tos.event.synthetic.item.' + suffix,
            acquisition_event_id='tos.event.synthetic.acquisition.' + suffix,
            inventory_event_id='tos.event.synthetic.inventory.' + suffix,
            allowed_item_form_ids=['tos.form.synthetic.item.' + suffix + '.name'],
            allowed_claim_form_ids=['tos.form.synthetic.item.' + suffix + '.statement'])
        self.owner.write_text(json.dumps(self.config))

    def proposal(self):
        record = {key: copy.deepcopy(self.edition[key]) for key in
            ('schema_version', 'record_version', 'variant_labels', 'identity_status', 'source_refs',
             'external_identifiers', 'same_as_posture', 'supersedes_ref', 'notes', 'field_languages')}
        record.update(record_type='item', record_id=self.config['item_id'], record_version=1,
            preferred_label='Synthetic acquired Item', item_manifest_ref=str(Path(self.config['item_source_path']).with_name('item.manifest.json')))
        claim = {**copy.deepcopy(self.edition_request['claim']), 'claim_id': self.config['claim_id'],
            'predicate': 'exemplified_by', 'subject_ref': self.config['edition_id'], 'object': self.config['item_id'],
            'provenance_event_ref': self.config['provenance_event_id'],
            'evidence_refs': [self.edition_ref, self.config['item_source_path']]}
        rights = {'schema_version': 'tos_rights_record_v1', 'rights_id': self.config['rights_id'],
            'scope_refs': [self.config['item_id'], self.config['file_id']], 'assessment_status': 'not_assessed',
            'jurisdictions_reviewed': [], 'source_refs': ['https://example.invalid/synthetic-source'],
            'permissions': [], 'restrictions': ['No public redistribution or server processing.'],
            'visibility': 'local_only', 'redistribution_posture': 'not_authorized', 'derivative_posture': 'local_research_only',
            'assessed_by': {'maker_type': 'model', 'agent_ref': 'model:synthetic'},
            'assessed_at': '2026-09-09T00:00:00Z', 'rationale': 'Synthetic test; no rights clearance.',
            'review_status': 'unreviewed', 'record_version': 1}
        return {'schema_version': item.REQUEST, 'operation': item.PREPARE, 'record': record, 'claim': claim,
            'rights': rights, 'item_kind': 'born_digital',
            'forms': [{'form_id': self.config['allowed_edition_form_ids'][0], 'field_id': 'metadata.preferred-name'}],
            'item_forms': [{'form_id': self.config['allowed_item_form_ids'][0], 'field_id': 'metadata.preferred-name'}],
            'claim_forms': [{'form_id': self.config['allowed_claim_form_ids'][0], 'field_id': 'claim.statement'}],
            'reason': 'Synthetic local acquired Item; no source reading or admission.'}

    def request(self):
        proposal = self.proposal()
        result = commands.run_local_command(self.owner, proposal)
        return {**proposal, 'operation': item.OPERATION, 'command_id': self.config['claim_id'],
            'fields': result['prepared_fields'], 'expected_source': result['source'],
            'expected_revision': result['revision'], 'expected_configuration': result['owner_configuration'],
            'expected_dependencies': result['expected_dependencies'], 'expected_publication': result['expected_publication'],
            **{key: result[key] for key in ('inventory', 'inventory_limitation', 'fixity_verified_at')}}

    def rebuild(self):
        self.origin.rebuild()
        catalog = self.root / item.CATALOG_MANIFEST
        manifest = json.loads(catalog.read_bytes())
        ref = 'ToS/source-witnesses/catalog/items.jsonl'
        rows, claims = [], []
        for path in self.edition_path.parent.glob('items/*/item.json'):
            record = json.loads(path.read_bytes())
            entry = {'schema_version': 'tos_source_witness_catalog_entry_v1', 'record_type': 'item',
                'record_id': record['record_id'], 'source_record_ref': str(path.relative_to(self.root)),
                'preferred_label': record['preferred_label'], 'identity_status': record['identity_status'],
                'record_sha256': hashlib.sha256(commands._canonical(record)).hexdigest(),
                'links': {'item_manifest_ref': record['item_manifest_ref']}}
            rows.append(entry)
            carrier = path.with_name('source-claims.jsonl')
            for number, line in enumerate(carrier.read_bytes().splitlines(), 1):
                claim = json.loads(line)
                claims.append({**claim, 'schema_version': 'tos_source_witness_claim_catalog_entry_v1',
                    'source_claim_file_ref': str(carrier.relative_to(self.root)), 'source_claim_line': number,
                    'claim_sha256': hashlib.sha256(commands._canonical(claim)).hexdigest()})
        self.write(ref, b''.join(commands._canonical(row) + b'\n' for row in rows))
        claim_path = self.root / manifest['claim_file']
        claim_path.write_bytes(claim_path.read_bytes() + b''.join(commands._canonical(row) + b'\n' for row in claims))
        manifest['record_files']['item'] = ref
        manifest['selected_metadata_publication'] = {'protocol': item.revisions.SELECTED_PROTOCOL,
            'token': PublicationSnapshot(self.root).token,
            'files': {name: hashlib.sha256((self.root / name).read_bytes()).hexdigest()
                      for name in [*manifest['record_files'].values(), manifest['claim_file']]}}
        self.write(item.CATALOG_MANIFEST, manifest)

    def test_acquired_item_preserves_original_and_older_compound_lineage(self):
        original_edition = self.edition_path.read_bytes()
        request = self.request()
        self.assertEqual(self.edition_path.read_bytes(), original_edition)
        self.assertFalse(deposit.destination(self.config).exists())
        result = commands.run_local_command(self.owner, request)
        self.assertTrue(result['deposit']['metadata_committed'])
        self.assertEqual(self.input.read_bytes(), self.original_bytes)
        self.assertEqual(deposit.destination(self.config).read_bytes(), self.original_bytes)
        item_home = (self.root / self.config['item_source_path']).parent
        byte_receipt = json.loads((item_home / item.BYTE_RECEIPT_FILE).read_bytes())
        acquisition, forensic = [json.loads(line) for line in (item_home / 'provenance.jsonl').read_bytes().splitlines()]
        self.assertEqual({key: forensic[key] for key in ('started_at', 'ended_at')}, byte_receipt['observation_interval'])
        self.assertLessEqual(commands._instant(forensic['ended_at']), commands._instant(acquisition['started_at']))
        self.assertEqual(acquisition['ended_at'], byte_receipt['deposited_at'])
        self.assertEqual(json.loads(self.edition_path.read_bytes()), {**self.edition, 'record_version': 2,
            'exemplar_claim_refs': [self.config['claim_id']]})
        self.rebuild()
        for reference in (request['expected_source'], result['source'], result['receipt']['item'], self.edition_result['receipt']['edition']):
            resolved = resolve_metadata_version(self.root, reference)
            self.assertEqual(resolved['status'], 'available', resolved)
        self.origin.assert_origin()
        edition_fixture.edition.verify_compound(self.root, self.edition_path.with_name('source-claims.jsonl').relative_to(self.root).as_posix(), self.edition_request['claim'])
        self.assertFalse(item.verify_compound(self.root, item._claim_source_ref(self.config), request['claim'])['grants_admission'])
        replay = commands.run_local_command(self.owner, request)
        self.assertTrue(replay['replayed'])
        # A separate payload root leaves this real Item metadata package flat,
        # so its topology Claim must not rely on a directory-layout rejection.
        self.assertTrue(all(path.is_file() for path in item_home.iterdir()))
        generic_owner = self.root / 'generic-claim-owner.json'
        generic_owner.write_text(json.dumps({
            **{key: self.config[key] for key in ('uid', 'principal_id', 'source_root', 'authority_ref', 'expires_at')},
            'schema_version': commands.CLAIM_REVISION_CONFIG, 'source_path': item._claim_source_ref(self.config),
            'claim_id': self.config['claim_id'], 'allowed_operations': ['claim.revise'], 'allowed_fields': ['qualifiers'],
            'allowed_evidence_refs': [], 'allowed_form_ids': self.config['allowed_claim_form_ids']}))
        description = commands.run_local_command(generic_owner, {'schema_version': 'tos_local_source_command_v1', 'operation': 'describe'})
        correction = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
            'fields': {'qualifiers': {'statement': 'A changed synthetic topology statement.'}},
            'forms': request['claim_forms'], 'reason': 'Synthetic attempted generic bypass.'}
        direct = {**correction, 'operation': 'claim.revise', 'command_id': 'synthetic:blocked-item-claim-correction',
            'expected_configuration': description['owner_configuration'], 'expected_source': description['source'],
            'expected_revision': description['revision'], 'expected_dependencies': 'sha256:' + '0' * 64,
            'expected_inputs': {}}
        package_before = {path.name: path.read_bytes() for path in item_home.iterdir()}
        for attempted in (correction, direct):
            with self.subTest(operation=attempted['operation']), self.assertRaisesRegex(PermissionError, 'compound bibliographic operation'):
                commands.run_local_command(generic_owner, attempted)
        self.assertEqual({path.name: path.read_bytes() for path in item_home.iterdir()}, package_before)
        self.assertFalse(item.verify_compound(self.root, item._claim_source_ref(self.config), request['claim'])['grants_admission'])
        stage_before = deposit.read_stage(self.config, item._transaction_id(request))
        publication_before = PublicationSnapshot(self.root).token
        source_before = self.edition_path.read_bytes()
        recovery = {'schema_version': item.REQUEST, 'operation': item.RECOVERY, 'decision': 'rollback',
            'transaction_id': item._transaction_id(request), 'expected_configuration': request['expected_configuration']}
        with self.assertRaisesRegex(ValueError, 'already committed'):
            commands.run_local_command(self.owner, recovery)
        self.assertEqual(deposit.read_stage(self.config, item._transaction_id(request)), stage_before)
        self.assertEqual(PublicationSnapshot(self.root).token, publication_before)
        self.assertEqual(self.edition_path.read_bytes(), source_before)
        encoded = commands._canonical(result)
        self.assertNotIn(str(self.input).encode(), encoded)
        self.assertNotIn(str(self.payload_root).encode(), encoded)
        self.assertNotIn(str(self.recovery_root).encode(), encoded)
        for path in (self.root / 'ToS/source-witnesses').rglob('*'):
            if path.is_file():
                raw = path.read_bytes()
                self.assertNotIn(str(self.input).encode(), raw, str(path))
                self.assertNotIn(str(self.recovery_root).encode(), raw, str(path))

    def test_unsupported_format_retained_without_acquired_metadata(self):
        self.input.write_bytes(b'Synthetic unsupported plain text.\n')
        self.config.update(media_type='text/plain', byte_size=self.input.stat().st_size,
            sha256=hashlib.sha256(self.input.read_bytes()).hexdigest())
        self.owner.write_text(json.dumps(self.config))
        before = self.edition_path.read_bytes()
        request = self.request()
        result = commands.run_local_command(self.owner, request)
        self.assertEqual(result['deposit']['state'], 'deposited')
        self.assertFalse(result['deposit']['metadata_committed'])
        self.assertIsNone(result['receipt'])
        self.assertFalse((self.root / self.config['item_source_path']).exists())
        self.assertEqual(self.edition_path.read_bytes(), before)
        recovery = {'schema_version': item.REQUEST, 'operation': item.RECOVERY, 'decision': 'rollback',
            'transaction_id': item._transaction_id(request), 'expected_configuration': request['expected_configuration']}
        result = commands.run_local_command(self.owner, recovery)
        self.assertEqual(result['deposit']['state'], 'rolled-back-retained')
        self.assertEqual(deposit.destination(self.config).read_bytes(), self.input.read_bytes())

    def test_metadata_crash_rollback_preserves_canonical_colocated_payload(self):
        self.config['payload_root'] = str(self.root / 'ToS/source-witnesses')
        self.owner.write_text(json.dumps(self.config))
        before = self.edition_path.read_bytes()
        request = self.request()
        worker = edition_fixture.expression_fixture.CRASH_WRITER
        process = subprocess.run([sys.executable, '-c', worker, str(MECHANIC), str(self.owner), '2'],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 86, process.stdout + process.stderr)
        pending = item.transactions.read_pending_transaction(self.root)
        home = str(Path(self.config['item_source_path']).parent)
        self.assertNotIn(home, pending['plan']['new_directories'])
        recovery = {'schema_version': item.REQUEST, 'operation': item.RECOVERY, 'decision': 'rollback',
            'transaction_id': item._transaction_id(request), 'expected_configuration': request['expected_configuration']}
        commands.run_local_command(self.owner, recovery)
        self.assertEqual(self.edition_path.read_bytes(), before)
        self.assertEqual(deposit.destination(self.config).read_bytes(), self.original_bytes)
        self.assertEqual(self.input.read_bytes(), self.original_bytes)

    def test_deposited_before_metadata_resumes_under_exact_renewal(self):
        request = self.request()
        with patch.object(item.revisions, '_archive', side_effect=OSError('synthetic pre-metadata interruption')):
            with self.assertRaises(OSError):
                commands.run_local_command(self.owner, request)
        self.assertEqual(deposit.read_stage(self.config, item._transaction_id(request))['state'], 'deposited')
        self.assertFalse((self.root / self.config['item_source_path']).exists())
        self.config.update(allowed_operations=[item.RECOVERY], authority_ref='test-only:exact-recovery-renewal',
                           payload_authority_ref='test-only:exact-payload-recovery-renewal')
        self.owner.write_text(json.dumps(self.config))
        current_digest = item.configuration(self.config)[1]
        recovery = {'schema_version': item.REQUEST, 'operation': item.RECOVERY, 'decision': 'resume',
            'transaction_id': item._transaction_id(request), 'expected_configuration': current_digest}
        result = commands.run_local_command(self.owner, recovery)
        self.assertTrue(result['deposit']['metadata_committed'])
        receipt = json.loads((self.root / self.config['item_source_path']).with_name(item.BYTE_RECEIPT_FILE).read_bytes())
        self.assertEqual(receipt['recovery_configuration'], current_digest)
        self.assertEqual(receipt['owner_configuration'], request['expected_configuration'])

    def test_later_item_retains_old_item_and_edition_versions(self):
        first = self.request()
        first_config = copy.deepcopy(self.config)
        first_result = commands.run_local_command(self.owner, first)
        self.rebuild()
        self.select_item('second')
        second = self.request()
        commands.run_local_command(self.owner, second)
        self.rebuild()
        self.config = first_config
        self.owner.write_text(json.dumps(self.config))
        self.assertTrue(commands.run_local_command(self.owner, first)['replayed'])
        self.assertEqual(resolve_metadata_version(self.root, first_result['source'])['status'], 'available')
        self.assertEqual(json.loads(self.edition_path.read_bytes())['exemplar_claim_refs'],
                         [first['claim']['claim_id'], second['claim']['claim_id']])
        self.origin.assert_origin()

    def test_closed_rights_and_authority_boundaries(self):
        proposal = self.proposal()
        for field, value in (('visibility', 'public_payload'), ('permissions', ['redistribute']),
                             ('assessment_status', 'public_domain_reviewed'), ('review_status', 'human_reviewed')):
            altered = copy.deepcopy(proposal)
            altered['rights'][field] = value
            with self.subTest(field=field), self.assertRaises(PermissionError):
                commands.run_local_command(self.owner, altered)
        for key in ('expires_at', 'payload_expires_at'):
            config = {**self.config, key: '2000-01-01T00:00:00Z'}
            with self.subTest(key=key), self.assertRaises(PermissionError):
                item.configuration(config)
        request = self.request()
        altered = {**request, 'payload_root': str(self.payload_root)}
        with self.assertRaises(ValueError):
            commands.run_local_command(self.owner, altered)
        self.assertFalse(deposit.destination(self.config).exists())

    def test_retained_metadata_before_pending_resumes_exact_original_plan(self):
        request = self.request()
        original = item.transactions._publish_state
        def interrupted(root, state, previous):
            if state['phase'] == 'pending':
                raise OSError('synthetic interruption after metadata retention before pending')
            return original(root, state, previous)
        with patch.object(item.transactions, '_publish_state', interrupted):
            with self.assertRaises(OSError):
                commands.run_local_command(self.owner, request)
        retained = item.transactions.inspect_transaction(self.root, item._transaction_id(request))
        self.assertEqual(retained['status'], 'orphan')
        self.assertFalse((self.root / self.config['item_source_path']).exists())
        self.config.update(allowed_operations=[item.RECOVERY], authority_ref='test-only:exact-retained-plan-renewal')
        self.owner.write_text(json.dumps(self.config))
        recovery = {'schema_version': item.REQUEST, 'operation': item.RECOVERY, 'decision': 'resume',
            'transaction_id': item._transaction_id(request), 'expected_configuration': item.configuration(self.config)[1]}
        result = commands.run_local_command(self.owner, recovery)
        after = item.transactions.inspect_transaction(self.root, item._transaction_id(request))
        self.assertEqual(after['manifest_sha256'], retained['manifest_sha256'])
        self.assertEqual(after['status'], 'committed')
        self.assertEqual(after['publication']['recovery_authorization']['owner_configuration'], recovery['expected_configuration'])
        self.assertEqual(result['receipt']['transaction_id'], item._transaction_id(request))


if __name__ == '__main__':
    unittest.main()

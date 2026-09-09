"""Synthetic native object-Link publication and retained lineage boundaries."""
from __future__ import annotations

import copy
import hashlib
import json
import os
from pathlib import Path
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
import source_link_commands as links
import source_revisions as revisions
from source_record_profiles import SourceClaimProfiles, SourceRecordProfiles
from source_metadata_snapshot import PublicationSnapshot, PublicationPending, PublicationChanged
from build_source_witness_catalog import artifact_catalog_entry

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


class NativeObjectLinkTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='corpus-completion-link-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.subject = {'schema_version': 'tos_corpus_record_v1', 'record_type': 'work',
            'record_id': 'tos.work.synthetic.link-subject', 'record_version': 1,
            'preferred_label': 'Synthetic source object', 'variant_labels': [], 'identity_status': 'provisional',
            'source_refs': ['https://example.invalid/observation'], 'external_identifiers': [],
            'same_as_posture': 'no_equivalence_claim', 'expression_claim_refs': [], 'supersedes_ref': None,
            'notes': 'Synthetic source object; no historical truth asserted.'}
        self.subject_ref = 'ToS/source-witnesses/works/synthetic/link-subject/work.json'
        self.write(self.subject_ref, self.subject)
        self.subject_path = self.root / self.subject_ref
        for ref in {*SourceClaimProfiles(ROOT).input_digests, *SourceRecordProfiles(ROOT).input_digests,
            links.LINK_SCHEMA, links.CLAIM_SCHEMA, 'ToS/contracts/source-claim-record.schema.json',
            'ToS/contracts/claim-packet.schema.json', 'ToS/contracts/knowledge-assessment.schema.json',
            'ToS/contracts/corpus-record.schema.json', 'ToS/contracts/provenance-event-v2.schema.json',
            'ToS/contracts/record-version-view.schema.json',
            'ToS/contracts/source-witness-bibliographic-graph.schema.json', *links.FORM_CONTRACTS}:
            self.write(ref, (ROOT / ref).read_bytes())
        self.config = {'schema_version': links.CONFIG, 'uid': os.getuid(), 'source_root': str(self.root),
            'principal_id': 'model:synthetic', 'maker_type': 'model', 'authority_ref': 'test:link-creation',
            'expires_at': '2099-01-01T00:00:00Z', 'allowed_operations': [links.OPERATION, links.RECOVERY],
            'subject_id': self.subject['record_id'], 'subject_source_path': self.subject_ref, 'subject_record_type': 'work',
            'link_id': 'tos.link.synthetic.object', 'link_source_path': 'ToS/source-witnesses/links/synthetic/link.json',
            'claim_id': 'tos.claim.synthetic.object-link', 'claim_source_path': 'ToS/source-witnesses/relations/synthetic-object-link/source-claims.jsonl',
            'predicate': 'described_by', 'provenance_event_id': 'tos.event.synthetic.object-link',
            'allowed_link_form_ids': ['tos.form.synthetic.link-name'],
            'allowed_claim_form_ids': ['tos.form.synthetic.link-statement'],
            'uri': 'https://example.invalid/object', 'observation_ref': 'https://example.invalid/observation',
            'allowed_evidence_refs': ['https://example.invalid/observation']}
        for ref in ('ToS/source-witnesses/links', 'ToS/source-witnesses/relations'):
            (self.root / ref).mkdir(parents=True, exist_ok=True)
        self.owner = self.root / 'link-owner.json'
        self.save_config()
        self.untouched = self.subject_path.parent / 'payload' / 'opaque.bin'
        self.untouched.parent.mkdir()
        self.untouched.write_bytes(b'Synthetic private payload must not be read.')
        self.rebuild()

    def write(self, ref, value):
        path = self.root / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value if isinstance(value, bytes) else revisions._encode(value))

    def save_config(self):
        self.owner.write_bytes(revisions._encode(self.config))

    def proposal(self):
        link = {'schema_version': 'tos_source_link_v1', 'record_type': 'link', 'record_id': self.config['link_id'],
            'record_version': 1, 'preferred_label': 'Synthetic reported address', 'variant_labels': [],
            'identity_status': 'provisional', 'source_refs': self.config['allowed_evidence_refs'],
            'external_identifiers': [], 'same_as_posture': 'no_equivalence_claim', 'link_kind': 'landing_page',
            'uri': self.config['uri'], 'provider_label': 'Synthetic provider', 'interface_type': 'web',
            'access_status': 'unknown', 'observed_at': '2026-09-09T00:00:00Z',
            'observation_ref': self.config['observation_ref'], 'mutable': True,
            'association_claim_refs': [self.config['claim_id']], 'provenance_event_ref': self.config['provenance_event_id'],
            'supersedes_ref': None}
        claim = {'schema_version': 'tos_object_link_claim_v2', 'claim_id': self.config['claim_id'],
            'claim_type': 'relation', 'claim_version': 1, 'assertion_layer': 'forensic_observation',
            'subject_ref': self.config['subject_id'], 'predicate': self.config['predicate'], 'object': self.config['link_id'],
            'evidence_refs': self.config['allowed_evidence_refs'],
            'maker': {'maker_type': 'model', 'agent_ref': 'model:synthetic'}, 'provenance_event_ref': self.config['provenance_event_id'],
            'epistemic_status': 'reported', 'review_status': 'unreviewed', 'visibility': 'public_metadata_only',
            'qualifiers': {'statement': 'A synthetic record reports this address for the selected object.',
                'statement_language': 'en', 'statement_script': 'Latn', 'link_role': 'Reported descriptive address only.',
                'availability_is_rights_conclusion': False, 'unknown_context': {'flag': False, 'missing': None}},
            'assessment_refs': []}
        return {'schema_version': links.REQUEST, 'operation': links.PREPARE, 'subject': copy.deepcopy(self.subject),
            'link': link, 'claim': claim, 'forms': [{'form_id': self.config['allowed_link_form_ids'][0], 'field_id': 'metadata.preferred-name'}],
            'claim_forms': [{'form_id': self.config['allowed_claim_form_ids'][0], 'field_id': 'claim.statement'}],
            'reason': 'Synthetic association, no rights or source content admitted.'}

    def request(self):
        proposal = self.proposal()
        prepared = commands.run_local_command(self.owner, proposal)
        return {**proposal, 'operation': links.OPERATION, 'command_id': 'test:object-link',
            'expected_configuration': prepared['owner_configuration'],
            'expected_dependencies': prepared['expected_dependencies'], 'expected_publication': prepared['expected_publication']}

    def rebuild(self):
        subject = json.loads(self.subject_path.read_bytes())
        kind = self.config['subject_record_type']
        if kind == 'artifact':
            entry = artifact_catalog_entry(self.root, subject, self.subject_ref)
        else:
            entry = {'schema_version': 'tos_source_witness_catalog_entry_v1', 'record_id': commands.metadata_subject(subject).id,
                'record_type': kind, 'source_record_ref': self.subject_ref, 'preferred_label': subject['preferred_label'],
                'identity_status': subject['identity_status'], 'record_sha256': commands.metadata_subject(subject).ref['digest'][7:], 'links': {}}
        records = {name: [] for name in ('agent', 'place', 'organization', 'work', 'expression', 'edition', 'collection', 'item', 'link')}
        records[kind] = [entry]
        claims = []
        link_path = self.root / self.config['link_source_path']
        if link_path.is_file():
            record = json.loads(link_path.read_bytes())
            records['link'].append({'schema_version': 'tos_source_witness_catalog_entry_v1',
                'record_id': record['record_id'], 'record_type': 'link', 'source_record_ref': self.config['link_source_path'],
                'preferred_label': record['preferred_label'], 'identity_status': record['identity_status'],
                'record_sha256': commands.metadata_subject(record).ref['digest'][7:], 'links': {'association_claim_refs': record['association_claim_refs']}})
        claim_path = self.root / self.config['claim_source_path']
        if claim_path.is_file():
            for number, line in enumerate(claim_path.read_bytes().splitlines(), 1):
                claim = json.loads(line)
                claims.append({**claim, 'schema_version': 'tos_source_witness_claim_catalog_entry_v1',
                    'source_claim_file_ref': self.config['claim_source_path'], 'source_claim_line': number,
                    'claim_sha256': hashlib.sha256(commands._canonical(claim)).hexdigest()})
        manifest = {'schema_version': 'tos_source_witness_catalog_v3',
            'record_files': {kind: 'ToS/source-witnesses/catalog/' + kind + 's.jsonl' for kind in records},
            'claim_file': 'ToS/source-witnesses/catalog/claims.jsonl',
            'counts': {**{kind: len(rows) for kind, rows in records.items()}, 'claim': len(claims)}}
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
        self.write(links.common.CATALOG_MANIFEST, manifest)

    def test_atomic_create_real_native_forms_claim_reader_and_fresh_process_replay(self):
        request, before = self.request(), self.subject_path.read_bytes()
        original_read = commands._read
        def selected_read(path, limit):
            self.assertFalse(path.is_relative_to(self.untouched.parent))
            return original_read(path, limit)
        with patch.object(commands, '_read', selected_read):
            result = commands.run_local_command(self.owner, request)
        self.assertFalse(result['grants_admission'])
        self.assertEqual(self.subject_path.read_bytes(), before)
        self.assertTrue(all(view['state'] == 'ready' for group in result['materializations'].values() for view in group))
        self.assertTrue(commands.run_local_command(self.owner, request)['replayed'])
        verified = links.verify_compound(self.root, self.config['claim_source_path'], request['claim'])
        self.assertEqual(verified['receipt']['subject_source_sha256'], hashlib.sha256(before).hexdigest())
        self.rebuild()
        from metadata_version_reader import MetadataVersionReader
        from claim_version_reader import ClaimVersionReader
        self.assertEqual(MetadataVersionReader(self.root).resolve_typed(result['receipt']['link'])['status'], 'available')
        self.assertEqual(ClaimVersionReader(self.root).resolve(result['receipt']['claim'])['status'], 'available')
        process = subprocess.run([sys.executable, str(MECHANIC / 'source_commands.py'), '--owner-config', str(self.owner)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        self.assertTrue(json.loads(process.stdout)['replayed'])

    def test_artifact_uses_native_identity_without_corpus_recasting_or_subject_revision(self):
        sample = 'ToS/source-witnesses/artifacts/old-babylonian/uncertain/penn-cbs-07771/artifact-witness.json'
        self.subject = json.loads((ROOT / sample).read_bytes())
        self.subject.update(artifact_id='tos.artifact.synthetic.object-link', record_version=1)
        self.subject_ref = 'ToS/source-witnesses/artifacts/synthetic/site/object/artifact-witness.json'
        self.subject_path = self.root / self.subject_ref
        self.write(self.subject_ref, self.subject)
        from build_source_witness_catalog import native_witness_contract
        schema_ref = native_witness_contract(self.subject, self.subject_ref)[0]
        self.write(schema_ref, (ROOT / schema_ref).read_bytes())
        self.config.update(subject_id=self.subject['artifact_id'], subject_record_type='artifact', subject_source_path=self.subject_ref)
        self.save_config()
        self.rebuild()
        before = self.subject_path.read_bytes()
        request = self.request()
        commands.run_local_command(self.owner, request)
        self.assertEqual(self.subject_path.read_bytes(), before)
        self.assertNotIn('record_id', self.subject)
        self.assertFalse(self.subject_path.with_name(revisions.HISTORY).exists())
        self.assertEqual(links.verify_compound(self.root, self.config['claim_source_path'], request['claim'])['receipt']['subject']['id'],
                         self.subject['artifact_id'])
        old_schema = json.loads((ROOT / 'ToS/contracts/object-link-claim.schema.json').read_bytes())
        self.assertFalse(commands.Draft202012Validator(old_schema['properties']['subject_ref']).is_valid(self.subject['artifact_id']))

    def test_qualified_scope_and_independent_grants_are_closed(self):
        mutations = [lambda p: p['link'].update(uri='https://example.invalid/other'),
            lambda p: p['link'].update(association_claim_refs=[]), lambda p: p['link'].update(identity_status='verified'),
            lambda p: p['subject'].update(preferred_label='Changed source'), lambda p: p['claim'].update(object=self.config['subject_id']),
            lambda p: p['claim'].update(schema_version='tos_object_link_claim_v1'),
            lambda p: p['claim']['qualifiers'].update(availability_is_rights_conclusion=True),
            lambda p: p['claim']['qualifiers'].pop('statement'), lambda p: p['claim'].update(review_status='accepted'),
            lambda p: p['claim'].update(evidence_refs=['https://example.invalid/not-granted'])]
        for mutate in mutations:
            proposal = self.proposal()
            mutate(proposal)
            with self.subTest(mutate=mutate), self.assertRaises((ValueError, PermissionError, KeyError)):
                commands.run_local_command(self.owner, proposal)
        self.config['allowed_operations'] = []
        self.save_config()
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, self.proposal())
        self.assertFalse((self.root / self.config['link_source_path']).parent.exists())
        import source_claim_commands
        with self.assertRaises(PermissionError):
            source_claim_commands._scope({'allowed_operations': ['claim.create'], 'source_root': str(self.root)}, [self.proposal()['claim']])

    def test_new_form_ids_cannot_reuse_an_existing_subject_form_identity(self):
        forms, _, _ = links.common._forms(self.subject, None, self.proposal()['forms'], self.config['principal_id'])
        self.write(str(Path(self.subject_ref).with_name('work.human-forms.json')), forms)
        with self.assertRaises(commands.JournalConflict):
            self.request()

    def test_all_four_predicates_preserve_disputed_qualified_accounts_and_exact_domain(self):
        profiles = SourceClaimProfiles(self.root)
        for predicate in sorted(links.PREDICATES):
            claim = {**self.proposal()['claim'], 'predicate': predicate, 'epistemic_status': 'disputed'}
            with self.subTest(predicate=predicate):
                profiles.validate(claim, {self.config['subject_id']: self.subject,
                    self.config['link_id']: self.proposal()['link']})
                with self.assertRaises(ValueError):
                    profiles.validate(claim, {self.config['subject_id']: {'record_type': 'agent'},
                        self.config['link_id']: self.proposal()['link']})

    def crash(self, edge=3):
        request = self.request()
        process = subprocess.run([sys.executable, '-c', CRASH_WRITER, str(MECHANIC), str(self.owner), str(edge)],
            input=json.dumps(request), text=True, capture_output=True)
        self.assertEqual(process.returncode, 86, process.stdout + process.stderr)
        with self.assertRaises(PublicationPending):
            PublicationSnapshot(self.root)
        return request, links.transactions.read_pending_transaction(self.root)

    def recovery(self, pending, decision):
        return {'schema_version': links.REQUEST, 'operation': links.RECOVERY, 'decision': decision,
            'transaction_id': pending['manifest']['transaction_id'], 'expected_configuration': links.configuration(self.config)[1]}

    def test_interrupted_two_home_publication_supports_exact_recovery_only_renewal(self):
        request, pending = self.crash()
        self.config.update(principal_id='model:synthetic-recoverer', allowed_operations=[links.RECOVERY])
        self.save_config()
        with self.assertRaises(PermissionError):
            commands.run_local_command(self.owner, request)
        result = commands.run_local_command(self.owner, self.recovery(pending, 'resume'))
        self.assertEqual(result['receipt']['principal_id'], 'model:synthetic')
        self.assertEqual(result['recovery']['publication']['recovery_authorization']['principal_id'], self.config['principal_id'])

    def test_rollback_preserves_subject_and_rejects_hostile_third_state(self):
        before = self.subject_path.read_bytes()
        _, pending = self.crash(edge=4)
        row = next(row for row in pending['plan']['files'] if (self.root / row['path']).is_file())
        path = self.root / row['path']
        original = path.read_bytes()
        path.write_bytes(b'Foreign writer state\n')
        for decision in ('resume', 'rollback'):
            with self.subTest(decision=decision), self.assertRaises(ValueError):
                commands.run_local_command(self.owner, self.recovery(pending, decision))
            self.assertEqual(path.read_bytes(), b'Foreign writer state\n')
        path.write_bytes(original)
        commands.run_local_command(self.owner, self.recovery(pending, 'rollback'))
        self.assertEqual(self.subject_path.read_bytes(), before)
        self.assertFalse((self.root / self.config['link_source_path']).parent.exists())
        self.assertFalse((self.root / self.config['claim_source_path']).parent.exists())

    def test_pending_subject_byte_and_original_capture_changes_fail_closed(self):
        _, pending = self.crash()
        original = self.subject_path.read_bytes()
        self.subject_path.write_bytes(original + b'\n')
        with self.assertRaises(ValueError):
            commands.run_local_command(self.owner, self.recovery(pending, 'resume'))
        self.subject_path.write_bytes(original)
        commands.run_local_command(self.owner, self.recovery(pending, 'rollback'))
        self.rebuild()
        request = self.request()
        commands.run_local_command(self.owner, request)
        capture = self.root / Path(self.config['claim_source_path']).with_name(links.PROVENANCE_FILE)
        capture.write_bytes(capture.read_bytes() + b'\n')
        with self.assertRaises(ValueError):
            links.verify_compound(self.root, self.config['claim_source_path'], request['claim'])

    def test_native_foundation_requires_real_compound_origin(self):
        from validate_source_witness_foundation import _native_object_link_claims
        request = self.request()
        self.write(self.config['claim_source_path'], commands._canonical(request['claim']) + b'\n')
        issues = []
        self.assertEqual(_native_object_link_claims(self.root, issues), [])
        self.assertTrue(issues)
        path = self.root / self.config['claim_source_path']
        path.unlink()
        path.parent.rmdir()
        commands.run_local_command(self.owner, request)
        issues = []
        self.assertEqual(len(_native_object_link_claims(self.root, issues)), 1, issues)
        self.assertEqual(issues, [])


    def revise_link(self, fields=None):
        import source_native_metadata_commands as native
        config = {key: self.config[key] for key in ('uid', 'principal_id', 'source_root', 'authority_ref', 'expires_at')}
        config.update(schema_version=native.CONFIG, source_path=self.config['link_source_path'],
            record_type='link', record_schema_version='tos_source_link_v1', record_id=self.config['link_id'],
            allowed_fields=['preferred_label'], allowed_operations=['record.revise'],
            allowed_form_ids=self.config['allowed_link_form_ids'])
        owner = self.root / 'link-revision-owner.json'
        owner.write_bytes(revisions._encode(config))
        proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
            'fields': fields or {'preferred_label': 'Corrected synthetic Link label'}, 'forms': self.proposal()['forms'],
            'reason': 'Synthetic native descriptive correction; URI remains unchanged.'}
        prepared = commands.run_local_command(owner, proposal)
        correction = {**proposal, 'operation': 'record.revise', 'command_id': 'test:link-revise',
            'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
            'expected_configuration': prepared['owner_configuration'], 'expected_dependencies': prepared['expected_dependencies'],
            'expected_publication': prepared['expected_publication']}
        return commands.run_local_command(owner, correction), correction

    def revise_claim(self):
        config = {key: self.config[key] for key in ('uid', 'principal_id', 'source_root', 'authority_ref', 'expires_at')}
        config.update(schema_version=commands.CLAIM_REVISION_CONFIG, source_path=self.config['claim_source_path'],
            claim_id=self.config['claim_id'], allowed_operations=['claim.revise'], allowed_fields=['qualifiers'],
            allowed_evidence_refs=self.config['allowed_evidence_refs'], allowed_form_ids=self.config['allowed_claim_form_ids'])
        owner = self.root / 'claim-revision-owner.json'
        owner.write_bytes(revisions._encode(config))
        proposal = {'schema_version': 'tos_local_source_command_v1', 'operation': 'prepare-revise',
            'fields': {'qualifiers': {'statement': 'Corrected qualified report of the same synthetic address.'}},
            'forms': self.proposal()['claim_forms'], 'reason': 'Synthetic qualified association correction.'}
        prepared = commands.run_local_command(owner, proposal)
        self.assertFalse(prepared['source_bindings']['evidence'][self.config['observation_ref']]['resolved'])
        correction = {**proposal, 'operation': 'claim.revise', 'command_id': 'test:link-claim-revise',
            'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
            'expected_configuration': prepared['owner_configuration'], 'expected_dependencies': prepared['expected_dependencies'],
            'expected_inputs': prepared['source_bindings']}
        return commands.run_local_command(owner, correction), owner, proposal

    def test_independent_link_and_claim_corrections_retain_compound_origin_and_full_forms(self):
        request = self.request()
        created = commands.run_local_command(self.owner, request)
        self.rebuild()
        self.revise_link()
        self.rebuild()
        revised, owner, proposal = self.revise_claim()
        claim = revised['materializations'][0]['context'][0]['value']
        self.assertEqual(claim['claim_version'], 2)
        self.assertEqual(claim['qualifiers']['unknown_context'], {'flag': False, 'missing': None})
        self.assertTrue(commands.run_local_command(self.owner, request)['replayed'])
        verified = links.verify_compound(self.root, self.config['claim_source_path'], claim)
        self.assertEqual(verified['receipt'], created['receipt'])
        for qualifier in ('statement_language', 'statement_script', 'link_role', 'availability_is_rights_conclusion'):
            bad = {**proposal, 'fields': {'qualifiers': {qualifier: None}}}
            with self.subTest(qualifier=qualifier), self.assertRaises(ValueError):
                commands.run_local_command(owner, bad)
        self.rebuild()
        from metadata_version_reader import MetadataVersionReader
        from claim_version_reader import ClaimVersionReader
        self.assertEqual(MetadataVersionReader(self.root).resolve_typed(created['receipt']['link'])['version_status'], 'historical')
        self.assertEqual(ClaimVersionReader(self.root).resolve(created['receipt']['claim'])['version_status'], 'historical')
        link_history = self.root / Path(self.config['link_source_path']).with_name(revisions.HISTORY)
        raw = link_history.read_bytes()
        link_history.unlink()
        with self.assertRaises(ValueError):
            links.verify_compound(self.root, self.config['claim_source_path'], claim)
        link_history.write_bytes(raw)
        claim_history = self.root / Path(self.config['claim_source_path']).with_name('claim-revision-history.json')
        claim_history.unlink()
        with self.assertRaises(ValueError):
            links.verify_compound(self.root, self.config['claim_source_path'], claim)

    def test_native_link_replay_rejects_uncommitted_descriptive_transition(self):
        request = self.request()
        commands.run_local_command(self.owner, request)
        self.rebuild()
        _, correction = self.revise_link()
        inspected = links.transactions.inspect_transaction
        identifier = json.loads((self.root / Path(self.config['link_source_path']).with_name(revisions.HISTORY)).read_bytes())['receipts'][0]['publication']['transaction_id']
        def uncommitted(root, candidate):
            result = inspected(root, candidate)
            return {**result, 'status': 'orphan'} if candidate == identifier else result
        with patch.object(links.transactions, 'inspect_transaction', side_effect=uncommitted), self.assertRaises(ValueError):
            links.verify_compound(self.root, self.config['claim_source_path'], request['claim'])

    def test_typed_graph_consumer_validates_native_link_and_shared_claim(self):
        request = self.request()
        commands.run_local_command(self.owner, request)
        from build_source_witness_catalog import render_outputs, write_outputs
        write_outputs(self.root, render_outputs(self.root))
        from source_witness_bibliographic_graph_common import (_load_object_catalog, _load_claim_catalog,
            BibliographicGraphBuildError, build_payload, query_projection)
        objects = _load_object_catalog(self.root)
        claims = _load_claim_catalog(self.root, SourceClaimProfiles(self.root))
        self.assertEqual(objects[self.config['link_id']]['_source_record'], request['link'])
        self.assertEqual(len(claims), 1)
        SourceClaimProfiles(self.root).validate(request['claim'], objects)
        graph = build_payload(self.root)
        self.assertTrue(any(node['properties'].get('identity_kind') == 'link' for node in graph['nodes']))
        queried = query_projection(graph, claim_ref=self.config['claim_id'], repo_root=self.root)
        self.assertEqual(queried['result_count'], 1)
        self.assertEqual(queried['matches'][0]['source_return']['source_claim'], request['claim'])
        import tos_corpus_index_common as corpus
        diagnostics = []
        with patch.object(corpus, 'REPO_ROOT', self.root), patch.object(corpus, 'TOS_ROOT', self.root / 'ToS'):
            navigation = corpus.build_source_navigation(diagnostics)
        self.assertEqual(diagnostics, [])
        link_node = next(node for node in navigation['nodes'] if node['node_id'] == self.config['link_id'])
        self.assertTrue(link_node['properties']['human_forms'])
        self.assertEqual(link_node['properties']['record_history']['status'], 'available')
        path = self.root / self.config['link_source_path']
        invalid = {**request['link'], 'record_type': 'work'}
        path.write_bytes(revisions._encode(invalid))
        # Rebind canonical bytes to isolate the native contract check itself.
        catalog = self.root / 'ToS/source-witnesses/catalog/links.jsonl'
        entry = json.loads(catalog.read_bytes())
        entry['record_sha256'] = commands._digest(commands._canonical(invalid))[7:]
        catalog.write_bytes(commands._canonical(entry) + b'\n')
        with self.assertRaises(BibliographicGraphBuildError):
            _load_object_catalog(self.root)

    def test_preparation_response_keeps_its_original_snapshot(self):
        request, result = self.request(), links._result
        fired = False
        def concurrent(*args, **kwargs):
            nonlocal fired
            if not fired:
                fired = True
                commands.run_local_command(self.owner, request)
            return result(*args, **kwargs)
        with patch.object(links, '_result', side_effect=concurrent), self.assertRaises(PublicationChanged):
            self.request()

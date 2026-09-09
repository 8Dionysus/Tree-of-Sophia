"""Native descriptive revisions keep exact identities, history and descendants.

Public source shapes are reused only as synthetic serializer fixtures; no test
performs research, assesses a witness or touches the real source corpus.
"""
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
import source_commands as source
import source_revisions as revisions
import source_native_metadata_commands as native
import source_metadata_transactions as transactions
from metadata_version_reader import MetadataVersionReader
from source_metadata_snapshot import PublicationSnapshot, PublicationPending
from build_source_witness_catalog import artifact_catalog_entry, composite_catalog_entry

SAMPLES = {
    'artifact': 'ToS/source-witnesses/artifacts/old-babylonian/uncertain/penn-cbs-07771/artifact-witness.json',
    'composite': 'ToS/source-witnesses/scholarly-composites/critical/sumerian/instructions-of-shuruppag/composite-witness.json',
    'link': 'ToS/source-witnesses/links/cdli/cdlb-2006-1/article/link.json',
}


class NativeFixture:
    def __init__(self, case, kind):
        temporary = tempfile.TemporaryDirectory(prefix='corpus-completion-')
        case.addCleanup(temporary.cleanup)
        self.root, self.kind = Path(temporary.name), kind
        subtree = {'artifact': 'artifacts', 'composite': 'scholarly-composites', 'link': 'links'}[kind]
        self.relative = f'ToS/source-witnesses/{subtree}/synthetic/fixture/{Path(SAMPLES[kind]).name}'
        self.path = self.root / self.relative
        self.path.parent.mkdir(parents=True)
        self.record = json.loads((ROOT / SAMPLES[kind]).read_bytes())
        identity = {'artifact': 'artifact_id', 'composite': 'composite_id', 'link': 'record_id'}[kind]
        self.record[identity] = 'tos.' + kind + '.native-synthetic'
        self.record['record_version'] = 1
        self.path.write_bytes(revisions._encode(self.record))
        self.selections = [{'form_id': 'tos.form.native-name', 'field_id': 'metadata.preferred-name'}]
        if kind != 'link':
            self.selections.append({'form_id': 'tos.form.native-note', 'field_id': 'metadata.source-note'})
        forms = source._apply(None, source.metadata_subject(self.record), [source.prepare_metadata_change(
            self.record, None, 'test:synthetic-author', **selection) for selection in self.selections])
        self.formpath = self.path.with_name(self.path.stem + '.human-forms.json')
        self.formpath.write_bytes(revisions._encode(forms))
        self.original_forms = forms
        self.config = {'schema_version': native.CONFIG, 'uid': os.getuid(), 'source_root': str(self.root),
            'source_path': self.relative, 'record_id': self.record[identity], 'record_type': kind,
            'record_schema_version': self.record['schema_version'], 'principal_id': 'test:synthetic-reviser',
            'authority_ref': 'test:independent-native-descriptive-grant', 'expires_at': '2099-01-01T00:00:00Z',
            'allowed_operations': ['record.revise', 'record.recover'],
            'allowed_form_ids': [selection['form_id'] for selection in self.selections],
            'allowed_fields': sorted(native.REVISION_FIELDS[kind])}
        profile = native.record_profile(self.config, self.record)
        for ref in (profile['schema_ref'], 'ToS/contracts/corpus-record.schema.json',
                    'ToS/contracts/semantic-entity-type-registry.schema.json',
                    'ToS/doctrine/semantic-interchange/entity-types.v1.json'):
            (self.root / ref).parent.mkdir(parents=True, exist_ok=True)
            (self.root / ref).write_bytes((ROOT / ref).read_bytes())
        self.owner = self.root / 'owner.json'
        self.save_config()
        self.nested = self.path.parent / 'representations/private/payload'
        self.nested.mkdir(parents=True)
        (self.nested / 'untouched.bin').write_bytes(b'synthetic opaque bytes')
        self.origin = self.path.parent / 'provenance.jsonl'
        self.origin.write_bytes(b'SYNTHETIC ORIGINAL PROVENANCE MUST NOT BE READ OR REWRITTEN\n')
        self.sync_catalog()

    def save_config(self):
        self.owner.write_bytes(revisions._encode(self.config))

    def command(self, operation, **fields):
        return source.run_local_command(self.owner, {'schema_version': 'tos_local_source_command_v1',
                                                     'operation': operation, **fields})

    def request(self):
        record = json.loads(self.path.read_bytes())
        if self.kind == 'artifact':
            fields = {'path_identity': {**record['path_identity'], 'note': 'Synthetic descriptive correction.'}}
        elif self.kind == 'composite':
            fields = {'editorial_object': {**record['editorial_object'], 'description': 'Synthetic descriptive correction.'}}
        else:
            fields = {'preferred_label': 'Synthetic corrected navigation label.'}
        proposal = {'fields': fields, 'forms': self.selections, 'reason': 'Synthetic serialization test only.'}
        prepared = self.command('prepare-revise', **proposal)
        return {'schema_version': 'tos_local_source_command_v1', 'operation': 'record.revise',
            'command_id': 'test:native-correction', 'expected_configuration': prepared['owner_configuration'],
            'expected_source': prepared['source'], 'expected_revision': prepared['revision'],
            'expected_dependencies': prepared['expected_dependencies'],
            'expected_publication': prepared['expected_publication'], **proposal}

    def sync_catalog(self):
        record = json.loads(self.path.read_bytes())
        if self.kind == 'artifact':
            entry = artifact_catalog_entry(self.root, record, self.relative)
        elif self.kind == 'composite':
            entry = composite_catalog_entry(self.root, record, self.relative)
        else:
            entry = {'schema_version': 'tos_source_witness_catalog_entry_v1', 'record_id': record['record_id'],
                'record_type': 'link', 'preferred_label': record['preferred_label'],
                'identity_status': record['identity_status'], 'source_record_ref': self.relative,
                'record_sha256': source.metadata_subject(record).ref['digest'][7:], 'links': {}}
        catalog_ref = f'ToS/source-witnesses/catalog/{self.kind}s.jsonl'
        files = {catalog_ref: source._canonical(entry) + b'\n', 'ToS/source-witnesses/catalog/claims.jsonl': b''}
        catalog = self.root / 'ToS/source-witnesses/catalog'
        catalog.mkdir(exist_ok=True)
        manifest = {'schema_version': 'tos_source_witness_catalog_v3', 'record_files': {self.kind: catalog_ref},
                    'claim_file': 'ToS/source-witnesses/catalog/claims.jsonl'}
        snapshot = PublicationSnapshot(self.root)
        if snapshot.token is not None:
            manifest['selected_metadata_publication'] = {'protocol': revisions.SELECTED_PROTOCOL,
                'token': snapshot.token, 'files': {ref: source._digest(raw)[7:] for ref, raw in files.items()}}
        for ref, raw in files.items():
            (self.root / ref).write_bytes(raw)
        (catalog / 'catalog.manifest.json').write_bytes(source._canonical(manifest))


class NativeMetadataRevisionTests(unittest.TestCase):
    def test_artifact_creation_replay_uses_native_identity_and_selected_files_without_history(self):
        fixture = NativeFixture(self, 'artifact')
        config = {'schema_version': source.ARTIFACT_CREATION_CONFIG, 'source_root': str(fixture.root),
            'source_path': fixture.relative, 'record_id': fixture.record['artifact_id'],
            'principal_id': 'test:synthetic-author', 'authority_ref': 'test:native-origin'}
        request = {'record': fixture.record, 'forms': fixture.selections,
                   'expected_configuration': 'sha256:' + 'a' * 64, 'expected_dependencies': 'sha256:' + 'b' * 64}
        files = {fixture.path.name: fixture.path.read_bytes(), fixture.formpath.name: fixture.formpath.read_bytes()}
        receipt = {'schema_version': 'tos_local_source_create_receipt_v1', 'command_id': 'test:native-origin',
            'request_digest': source._digest(source._canonical(request)), 'principal_id': config['principal_id'],
            'authority_ref': config['authority_ref'], 'owner_configuration': request['expected_configuration'],
            'recorded_at': '2026-09-09T00:00:00Z', 'source_path': fixture.relative,
            'source': source.metadata_subject(fixture.record).ref, 'dependencies': request['expected_dependencies'],
            'files': revisions._file_refs(files), 'grants_admission': False}
        fixture.path.with_name('source-create-receipt.json').write_bytes(revisions._encode(receipt))
        original = Path.iterdir
        def forbid_descendant_enumeration(path):
            self.assertNotEqual(path, fixture.path.parent)
            self.assertFalse(path.is_relative_to(fixture.nested))
            return original(path)
        with patch.object(Path, 'iterdir', forbid_descendant_enumeration):
            source._creation_replay(config, fixture.path, request, receipt)
        self.assertEqual((fixture.nested / 'untouched.bin').read_bytes(), b'synthetic opaque bytes')

    def test_each_native_identity_keeps_exact_history_forms_and_untouched_descendants(self):
        for kind in SAMPLES:
            with self.subTest(kind=kind):
                fixture = NativeFixture(self, kind)
                request = fixture.request()
                before = revisions._selected_package(fixture.path)
                read, listing = source._read, Path.iterdir
                def selected_read(path, limit):
                    self.assertFalse(path.is_relative_to(fixture.nested))
                    self.assertNotEqual(path, fixture.origin)
                    return read(path, limit)
                def selected_listing(path):
                    self.assertNotEqual(path, fixture.path.parent)
                    return listing(path)
                with patch.object(source, '_read', side_effect=selected_read), patch.object(Path, 'iterdir', selected_listing):
                    result = source.run_local_command(fixture.owner, request)
                    old = fixture.command('inspect-version', source=request['expected_source'])
                self.assertEqual(old['record'], fixture.record)
                self.assertEqual(set(old['files']), set(before))
                self.assertTrue(all(view['state'] == 'ready' and view['admission'] is None for view in result['materializations']))
                self.assertEqual(json.loads(fixture.formpath.read_bytes())['prior_forms'], fixture.original_forms['forms'])
                self.assertEqual((fixture.nested / 'untouched.bin').read_bytes(), b'synthetic opaque bytes')
                self.assertTrue(source.run_local_command(fixture.owner, request)['replayed'])
                fixture.sync_catalog()
                reader = MetadataVersionReader(fixture.root)
                self.assertTrue(reader.supports(kind, source_ref=fixture.relative))
                current = json.loads(fixture.path.read_bytes())
                typed = reader.resolve_typed(source.metadata_subject(current).ref)
                historical = reader.resolve_typed(request['expected_source'])
                self.assertEqual(typed['status'], 'available', typed)
                self.assertEqual(historical['status'], 'available', historical)
                self.assertEqual(historical['record'], fixture.record)
                self.assertEqual(typed['descriptor']['identity_field'], {'artifact': 'artifact_id', 'composite': 'composite_id', 'link': 'record_id'}[kind])
                self.assertEqual(typed['descriptor']['record_kind'], 'subject')
                self.assertEqual(typed['descriptor']['type_id'], 'tos.entity.' + kind)
                self.assertFalse(typed['performs_assessment'])
                resolved_bytes = reader.resolve_source_bytes(fixture.relative, source._digest(before[fixture.path.name])[7:])
                self.assertEqual(resolved_bytes['record'], fixture.record, resolved_bytes)

    def test_native_pending_publication_requires_exact_recovery_and_reconstructs_actual_identity(self):
        for decision in ('resume', 'rollback'):
            with self.subTest(decision=decision):
                fixture = NativeFixture(self, 'artifact')
                request, replace = fixture.request(), transactions._replace_file
                before = revisions._selected_package(fixture.path)
                def interrupt(*args):
                    replace(*args)
                    raise RuntimeError('synthetic native interruption')
                with patch.object(transactions, '_replace_file', side_effect=interrupt):
                    with self.assertRaisesRegex(RuntimeError, 'synthetic native interruption'):
                        source.run_local_command(fixture.owner, request)
                with self.assertRaises(PublicationPending):
                    PublicationSnapshot(fixture.root)
                from source_selected_revisions import _transaction_id
                recovered = fixture.command('record.recover', decision=decision, transaction_id=_transaction_id(request),
                    expected_configuration=source._configuration(fixture.owner)[1])
                PublicationSnapshot(fixture.root)
                if decision == 'rollback':
                    self.assertEqual(revisions._selected_package(fixture.path), before)
                else:
                    self.assertEqual(recovered['source']['version'], 2)

    def test_structural_identity_schema_and_old_grants_remain_closed(self):
        fixture = NativeFixture(self, 'artifact')
        before = revisions._selected_package(fixture.path)
        request = fixture.request()
        for fields in ({'artifact_id': 'tos.artifact.other'}, {'rights_ref': 'ToS/elsewhere.json'},
                       {'path_identity': {**fixture.record['path_identity'], 'basis': 'excavation_identity'}}):
            invalid = copy.deepcopy(request)
            invalid['fields'] = fields
            with self.subTest(fields=fields), self.assertRaises((PermissionError, ValueError)):
                source.run_local_command(fixture.owner, invalid)
            self.assertEqual(revisions._selected_package(fixture.path), before)
        fixture.config.update(schema_version=source.CORPUS_COMPLETE_REVISION_CONFIG)
        fixture.config.pop('record_schema_version')
        fixture.save_config()
        with self.assertRaises((PermissionError, ValueError)):
            fixture.command('describe')
        invalid_ref = {**request['expected_source'], 'digest': 'sha256:' + '0' * 64}
        result = MetadataVersionReader(fixture.root).resolve_typed(invalid_ref)
        self.assertEqual(result['status'], 'stale')
        self.assertIsNone(result['descriptor'])

    def test_old_generic_form_grant_is_not_implicitly_extended_to_link(self):
        fixture = NativeFixture(self, 'link')
        fixture.config = {key: value for key, value in fixture.config.items()
                          if key not in {'record_id', 'record_type', 'record_schema_version', 'allowed_fields'}}
        fixture.config.update(schema_version='tos_local_source_command_owner_v1', allowed_operations=['form.create'])
        fixture.save_config()
        with self.assertRaises(ValueError):
            fixture.command('describe')

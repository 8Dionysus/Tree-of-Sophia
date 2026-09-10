"""Synthetic File -> private layer -> first TextUnit -> Occurrence boundaries.

All source bytes, rights statements and grants here are temporary fixtures;
these checks neither open retained Items nor assess any historical text.
"""
from __future__ import annotations

import copy
import io
import json
import os
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'tests'))
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))
import test_source_text_unit_commands as unit_fixtures
from test_occurrence_growth import copy_contracts, occurrence
from source_owner_context import OwnerLocalSourceContext
from native_text_binding import NativeTextBindingResolver
from source_text_layer_proposal import DEFAULT_POLICY
import source_commands as source
import source_text_layer_commands as layers
import source_text_unit_commands as units
import source_owner_profile_commands as private


def encode(value):
    return source._canonical(value) + b'\n'


class NativeLayerCommandTests(unittest.TestCase):
    def setUp(self):
        self.seed = unit_fixtures.NativeUnitCommandTests()
        self.seed.setUp()
        self.addCleanup(self.seed.doCleanups)
        self.base, self.public, self.store = self.seed.base, self.seed.public, self.seed.private
        self.fixture, self.prefix = self.seed.fixture, self.seed.prefix
        self.context_path = self.seed.context_path
        self.source_ref = self.prefix + 'layers/first/source-text-layer.v1.json'
        self.path = self.store / self.source_ref
        self.path.parent.parent.mkdir(mode=0o700)
        self.member_name = 'EPUB/source.xhtml'
        self.member = b'<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>Other.</p><p id="chosen">  A\n cafe\xcc\x81 <em>test</em><br/> tail. </p></body></html>'
        self.content = '  A\n cafe\u0301 test\n tail. '.encode()
        self.payload_root = self.base / 'canonical-payload-owner'
        self.payload_root.mkdir()
        self.payload = (self.payload_root / Path(self.fixture.item_home).relative_to('ToS/source-witnesses')
                        / 'payload' / 'source.epub')
        self.payload.parent.mkdir(parents=True)
        self.original = self.epub(self.member)
        self.payload.write_bytes(self.original)
        self.payload.chmod(0o600)
        self.file_sha = source._digest(self.original)[7:]
        self.file_id = 'tos.file.sha256.' + self.file_sha
        self.identities = {key: 'tos.' + kind + '.sid-' + char * 32 for key, kind, char in (
            ('layer_id', 'text-layer', '1'), ('anchor_id', 'anchor', '2'),
            ('passage_id', 'passage', '3'), ('provenance_event_id', 'event', '4'))}
        self.fixture.manifest['payload_files'] = [{'file_id': self.file_id,
            'relative_path': 'payload/source.epub', 'original_basename': 'source.epub',
            'media_type': 'application/epub+zip', 'byte_size': len(self.original),
            'sha256': self.file_sha, 'fixity_verified_at': '2026-01-01T00:00:00Z'}]
        self.fixture.rights['scope_refs'] = [self.fixture.ids['item'], self.file_id]
        self.fixture.write_json(self.fixture.manifest_ref, self.fixture.manifest)
        self.fixture.write_json(self.fixture.rights_ref, self.fixture.rights)
        self.derivative_ref = self.prefix + 'rights/derivative.json'
        derivative_path = self.store / self.derivative_ref
        derivative_path.parent.mkdir(mode=0o700)
        derivative = copy.deepcopy(self.fixture.rights)
        derivative.update(rights_id='tos.rights.synthetic.exact-new-layer', scope_refs=[self.identities['layer_id']])
        derivative_path.write_bytes(encode(derivative))
        derivative_path.chmod(0o600)
        # Prove bootstrap without any source TextUnit or old layer available.
        for ref in (self.fixture.packet_ref, self.fixture.layer_ref, self.fixture.anchor_ref, self.fixture.content_ref):
            (self.public / ref).unlink()
        self.config = {'schema_version': layers.CONFIG, 'uid': os.getuid(),
            'principal_id': 'software:synthetic-layer-extractor', 'authority_ref': 'operator:synthetic-layer-write',
            'expires_at': '2099-01-01T00:00:00Z', 'source_context_ref': str(self.context_path),
            'source_path': self.source_ref, 'allowed_operations': [layers.OPERATION],
            'source_scope': {**{kind + '_ref': value for kind, value in self.fixture.ids.items()},
                             'file_ref': self.file_id, 'file_sha256': self.file_sha},
            'source_record_refs': dict(self.fixture.refs),
            'source_record_sha256': {kind: self.fixture.file_digest(ref) for kind, ref in self.fixture.refs.items()},
            'manifest_sha256': self.fixture.file_digest(self.fixture.manifest_ref),
            'source_access': {'read_scope': 'exact_acquired_file', 'access_allowed': True,
                'payload_root': str(self.payload_root), 'byte_size': len(self.original),
                'authority_ref': 'operator:synthetic-exact-file-read', 'expires_at': '2099-01-01T00:00:00Z'},
            'derivation_access': {'derivation_allowed': True, 'operation': 'structural_extraction',
                'rights_record_refs': [{'ref': self.fixture.rights_ref, 'sha256': self.fixture.file_digest(self.fixture.rights_ref)},
                    {'ref': self.derivative_ref, 'sha256': source._digest(derivative_path.read_bytes())[7:]}],
                'content_visibility': 'local_only', 'authority_ref': 'operator:synthetic-exact-layer-derive',
                'expires_at': '2099-01-01T00:00:00Z'},
            'member': {'member_path': self.member_name, 'member_sha256': source._digest(self.member)[7:]},
            'selector': {'type': 'structural', 'scheme': 'tos.xhtml.element-ordinal.v1', 'value': 'p:2'},
            'policy': copy.deepcopy(DEFAULT_POLICY), 'identities': self.identities,
            'maker': {'maker_type': 'software', 'agent_ref': 'software:synthetic-layer-extractor',
                      'method': 'bounded-synthetic-xhtml-extraction', 'version': '1'},
            'language': 'und', 'limits': {'max_output_bytes': 65536, 'max_seconds': 60}}
        self.owner = self.base / 'layer-owner.json'
        self.write_owner()
        self.proposal = {'schema_version': contract_request(), 'operation': 'prepare-create'}

    def epub(self, member, *, duplicate=False):
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
            archive.writestr(self.member_name, member)
            if duplicate:
                archive.writestr(self.member_name, member)
        return buffer.getvalue()

    def write_owner(self):
        self.owner.write_bytes(encode(self.config))
        self.owner.chmod(0o600)

    def invoke(self, request):
        return source.run_local_command(self.owner, request)

    def prepare(self):
        response = self.invoke(self.proposal)
        self.request = {**self.proposal, 'operation': layers.OPERATION, 'command_id': 'synthetic-layer-1',
            'expected_configuration': response['owner_configuration'], 'expected_dependencies': response['expected_dependencies'],
            'expected_source': None, 'expected_revision': None}
        return response

    def created(self):
        self.prepare()
        return self.invoke(self.request)

    def files(self):
        return {path.name: path.read_bytes() for path in self.path.parent.iterdir()}

    def test_discovery_describe_and_results_do_not_disclose_private_source_fields(self):
        with patch.object(layers, '_payload', side_effect=AssertionError('describe opened payload')):
            described = self.invoke({'schema_version': contract_request(), 'operation': 'describe'})
        prepared = self.prepare()
        created = self.invoke(self.request)
        for response in (described, prepared, created, self.invoke(self.request), source.discover_commands()):
            rendered = json.dumps(response)
            for private_value in (str(self.payload), str(self.payload_root), str(self.context_path), self.source_ref,
                                  self.member_name, self.file_sha, self.config['member']['member_sha256'], 'p:2', self.content.decode()):
                self.assertNotIn(private_value, rendered)
            for field in ('source_record_refs', 'unit_slots', 'allowed_text_scope', 'exact_sha256'):
                self.assertNotIn(field, rendered)
        self.assertFalse(created['grants_admission'])

    def test_private_layer_is_exact_unreviewed_and_replay_preserves_all_evidence(self):
        result = self.created()
        files = self.files()
        self.assertEqual(files['content.txt'], self.content)
        layer = json.loads(files[self.path.name])
        self.assertEqual(layer['layer_role'], 'machine_transcription')
        self.assertEqual(layer['derivation']['method'], 'structural_extraction')
        self.assertEqual(layer['derivation']['input_layers'], [])
        self.assertEqual(layer['admission']['review_status'], 'unreviewed')
        self.assertFalse(layer['admission']['human_review_performed'])
        self.assertFalse(layer['representation']['publication_authorized'])
        event = json.loads(files['source-create-provenance.jsonl'])
        self.assertEqual(event['activity']['event_type'], 'native_extraction')
        self.assertEqual(event['rights_and_visibility']['content_visibility'], 'local_only')
        self.assertTrue(all(row['content_disclosure'] == 'private_content' for group in event['entities'].values() for row in group))
        self.assertEqual(self.payload.read_bytes(), self.original)
        self.assertEqual(self.path.parent.stat().st_mode & 0o777, 0o700)
        self.assertTrue(all(path.stat().st_mode & 0o777 == 0o600 for path in self.path.parent.iterdir()))
        self.assertEqual(self.invoke(self.request)['receipt_sha256'], result['receipt_sha256'])
        self.assertEqual(self.files(), files)
        self.assertFalse((self.public / self.source_ref).exists())

    def test_read_and_derivation_grants_and_exact_rights_fail_before_payload_io(self):
        cases = []
        for section, key, value in (('source_access', 'access_allowed', False),
                ('derivation_access', 'derivation_allowed', False), ('source_access', 'expires_at', '2000-01-01T00:00:00Z'),
                ('derivation_access', 'expires_at', '2000-01-01T00:00:00Z')):
            changed = copy.deepcopy(self.config)
            changed[section][key] = value
            cases.append(changed)
        changed = copy.deepcopy(self.config)
        changed['derivation_access']['rights_record_refs'] = changed['derivation_access']['rights_record_refs'][:1]
        cases.append(changed)
        for changed in cases:
            with self.subTest(case=len(cases)), patch.object(layers, '_payload', side_effect=AssertionError('rights failure opened payload')):
                self.owner.write_bytes(encode(changed))
                with self.assertRaises((ValueError, PermissionError)):
                    self.invoke(self.proposal)
        self.write_owner()
        self.assertFalse(self.path.parent.exists())

    def test_source_and_implementation_drift_fail_without_touching_committed_package(self):
        self.created()
        retained = self.files()
        original = source._read
        def changed(path, limit):
            raw = original(path, limit)
            return raw + b'\n' if path == ROOT / layers.IMPLEMENTATIONS[-1] else raw
        with patch.object(source, '_read', side_effect=changed), self.assertRaises(ValueError):
            self.invoke(self.request)
        self.payload.write_bytes(self.original + b'changed')
        with self.assertRaises((ValueError, source.JournalConflict)):
            self.invoke(self.request)
        self.assertEqual(self.files(), retained)

    def test_member_fixity_output_limit_and_zero_selector_fail_without_layer(self):
        for mutate in ('member', 'selector', 'limit'):
            changed = copy.deepcopy(self.config)
            if mutate == 'member':
                changed['member']['member_sha256'] = '0' * 64
            elif mutate == 'selector':
                changed['selector']['value'] = 'p:999'
            else:
                changed['limits']['max_output_bytes'] = 1
            self.owner.write_bytes(encode(changed))
            with self.subTest(mutation=mutate), self.assertRaises(ValueError):
                self.invoke(self.proposal)
            self.assertFalse(self.path.parent.exists())

    def test_zip_directory_actual_count_is_bounded_before_zipfile_parser(self):
        malformed = bytearray(self.original)
        eocd = malformed.rfind(b'PK\x05\x06')
        struct.pack_into('<HH', malformed, eocd + 8, 0, 0)
        with patch.object(zipfile, 'ZipFile', side_effect=AssertionError('unbounded ZIP parsed')), self.assertRaises(ValueError):
            layers._read_member(io.BytesIO(malformed), len(malformed), self.config['member'], float('inf'))

    def test_expired_payload_deadline_precedes_open_and_first_read(self):
        entry = self.fixture.manifest['payload_files'][0]
        with patch.object(layers.deposit, '_file', side_effect=AssertionError('expired payload opened')):
            with self.assertRaises(ValueError):
                layers._payload(self.config, entry, deadline=-1)
        original = layers.os.fdopen
        reads = []
        class Guarded:
            def __init__(self, descriptor, *args, **kwargs):
                self.stream = original(descriptor, *args, **kwargs)
            def __enter__(self):
                return self
            def __exit__(self, *_args):
                self.stream.close()
            def read(self, *_args):
                reads.append(True)
                raise AssertionError('expired payload bytes read')
        # Deadline can elapse while protected path/descriptor checks run.
        with patch.object(layers, '_deadline', side_effect=[None, ValueError('synthetic expiry')]), \
                patch.object(layers.os, 'fdopen', side_effect=Guarded), self.assertRaises(ValueError):
            layers._payload(self.config, entry, deadline=float('inf'))
        self.assertEqual(reads, [])

    def test_symlink_payload_fails_closed_without_disclosing_its_path(self):
        saved = self.payload.with_suffix('.retained')
        self.payload.rename(saved)
        self.payload.symlink_to(saved)
        with self.assertRaises(ValueError) as caught:
            self.prepare()
        self.assertNotIn(str(self.payload), str(caught.exception))
        self.assertFalse(self.path.parent.exists())

    def test_existing_empty_destination_is_not_overwritten(self):
        self.prepare()
        self.path.parent.mkdir(mode=0o700)
        identity = self.path.parent.stat().st_ino
        with self.assertRaises(ValueError):
            self.invoke(self.request)
        self.assertEqual(self.path.parent.stat().st_ino, identity)
        self.assertEqual(list(self.path.parent.iterdir()), [])
        self.assertEqual(self.payload.read_bytes(), self.original)

    def test_recovery_directory_enumeration_stops_at_its_budget(self):
        scanned = []
        class Entry:
            def __init__(self, name):
                self.name = name
        class Entries:
            def __enter__(self):
                return self
            def __exit__(self, *_args):
                return False
            def __iter__(self):
                for index in range(100):
                    scanned.append(index)
                    yield Entry(str(index))
        with patch.object(layers.os, 'scandir', return_value=Entries()), self.assertRaises(ValueError):
            layers._bounded_names(self.store, 2)
        self.assertEqual(scanned, [0, 1, 2])

    def test_changed_recovery_control_is_not_committed_or_erased(self):
        self.prepare()
        original = layers._write_new
        def changed(path, raw):
            original(path, raw)
            if path.name == 'content.txt':
                control = path.parent.parent
                plan = control / 'plan.json'
                plan.write_bytes(plan.read_bytes() + b' ')
                # Replacing its directory, even with byte-identical files,
                # must not be treated as the pinned recovery owner.
                retained = control.with_name(control.name + '.retained')
                control.rename(retained)
                control.mkdir(mode=0o700)
                for child in retained.iterdir():
                    child.rename(control / child.name)
        with patch.object(layers, '_write_new', side_effect=changed), self.assertRaises(ValueError):
            self.invoke(self.request)
        self.assertFalse(self.path.parent.exists())
        self.assertEqual(len(list(self.store.glob('.native-construction-*'))), 2)
        self.assertEqual(self.payload.read_bytes(), self.original)

    def test_completed_staged_files_resume_after_interruption_without_overwrite(self):
        self.prepare()
        original = layers._write_new
        count = 0
        def interrupted(path, raw):
            nonlocal count
            original(path, raw)
            if path.parent.name == 'output':
                count += 1
                if count == 2:
                    raise OSError('synthetic interruption after a completed staged file')
        with patch.object(layers, '_write_new', side_effect=interrupted), self.assertRaises(OSError):
            self.invoke(self.request)
        self.assertFalse(self.path.parent.exists())
        controls = list(self.store.glob('.native-construction-*.pending'))
        self.assertEqual(len(controls), 1)
        before = {path.name: path.read_bytes() for path in (controls[0] / 'output').iterdir()}
        self.invoke(self.request)
        self.assertTrue(self.path.exists())
        self.assertTrue(all(self.files()[name] == raw for name, raw in before.items()))
        self.assertEqual(len(list(self.store.glob('.native-construction-*.pending'))), 1)
        self.assertEqual(self.payload.read_bytes(), self.original)

    def test_torn_staged_bytes_are_retained_and_retry_does_not_multiply_stages(self):
        self.prepare()
        original = layers._write_new
        def torn(path, raw):
            original(path, raw[:3] if path.name == 'content.txt' else raw)
            if path.name == 'content.txt':
                raise OSError('synthetic torn write')
        with patch.object(layers, '_write_new', side_effect=torn), self.assertRaises(OSError):
            self.invoke(self.request)
        control = next(self.store.glob('.native-construction-*.pending'))
        for _ in range(2):
            with self.assertRaises(ValueError):
                self.invoke(self.request)
        self.assertEqual((control / 'output/content.txt').read_bytes(), self.content[:3])
        self.assertEqual(len(list(self.store.glob('.native-construction-*.pending'))), 1)
        self.assertFalse(self.path.parent.exists())

    def test_first_segmentation_then_existing_private_occurrence_and_public_denial(self):
        self.created()
        layer_raw = self.path.read_bytes()
        layer = json.loads(layer_raw)
        binding = {'schema_version': 'tos_native_text_layer_binding_v1',
            'text_layer': {'record_ref': self.source_ref, 'record_sha256': source._digest(layer_raw)[7:],
                'layer_id': layer['layer_id'], 'layer_version': layer['layer_version']},
            'source_record_refs': dict(self.fixture.refs)}
        cfg = copy.deepcopy(self.seed.config)
        cfg.update(schema_version=units.LAYER_CONFIG, source_binding=binding,
                   allowed_text_scope={'start': 0, 'end': len(self.content.decode())})
        cfg['unit_slots'] = [cfg['unit_slots'][0]]
        cfg['unit_slots'][0]['unit_kind'] = 'document'
        cfg['scheme'].update(analysis_role='source_structure', boundary_basis='source_markup')
        cfg['scheme']['policies'].update(whitespace='included_in_neighbor', line_break='included_in_neighbor')
        unit_owner = self.base / 'first-segmentation-owner.json'
        unit_owner.write_bytes(encode(cfg))
        unit_owner.chmod(0o600)
        proposal = copy.deepcopy(self.seed.proposal)
        proposal['spans'] = [{**proposal['spans'][0], 'start': 0, 'end': cfg['allowed_text_scope']['end']}]
        proposal['excluded_gaps'] = []
        prepared = source.run_local_command(unit_owner, proposal)
        request = {**proposal, 'operation': units.OPERATION, 'command_id': 'synthetic-first-segmentation',
            'expected_configuration': prepared['owner_configuration'], 'expected_dependencies': prepared['expected_dependencies'],
            'expected_source': None, 'expected_revision': None}
        result = source.run_local_command(unit_owner, request)
        self.assertFalse(result['grants_admission'])
        self.assertTrue(source.run_local_command(unit_owner, request)['replayed'])
        unit_path = self.store / cfg['source_path']
        packet = json.loads(unit_path.read_bytes())
        segment, unit = packet['segmentations'][0], packet['units'][0]
        native = {'schema_version': 'tos_native_text_unit_binding_v1',
            'packet_ref': cfg['source_path'], 'packet_sha256': source._digest(unit_path.read_bytes())[7:],
            'packet_id': packet['packet_id'], 'packet_version': 1, 'segmentation_id': segment['segmentation_id'],
            'segmentation_version': 1, 'unit_id': unit['unit_id'], 'unit_version': 1,
            'ordered_anchor_refs': unit['ordered_anchor_refs'], 'text_layer': binding['text_layer'],
            'source_record_refs': binding['source_record_refs']}
        context = OwnerLocalSourceContext.load(self.context_path)
        summary = NativeTextBindingResolver(self.public, owner_context=context).resolve(native, verify_content=True, allow_private_content=True)
        self.assertFalse(summary['public_content_declared'])
        self.assertTrue(summary['content_verified'])
        copy_contracts(self.public)
        from source_owner_record_profiles import OwnerLocalSourceRecordProfiles
        from source_record_profiles import SourceRecordProfiles
        record = occurrence(native)
        record['visibility'] = 'local_only'
        access = {'read_scope': 'exact_owner_local', 'access_allowed': True, 'authority_ref': 'operator:synthetic-occurrence-read'}
        # Existing private reader consumes the native binding; public reader cannot.
        profiles = OwnerLocalSourceRecordProfiles(context, access, native)
        profiles.validate('occurrence', record)
        profiles.validate_native_binding('occurrence', record, verify_content=True)
        with self.assertRaises(ValueError):
            SourceRecordProfiles(self.public).validate_native_binding('occurrence', record, verify_content=True)
        occurrence_ref = self.prefix + 'descriptions/first/occurrence.json'
        occurrence_path = self.store / occurrence_ref
        occurrence_path.parent.parent.mkdir(mode=0o700)
        occurrence_config = {'schema_version': private.CONFIG, 'uid': os.getuid(),
            'principal_id': 'agent:synthetic-private-writer', 'authority_ref': 'operator:synthetic-occurrence-write',
            'expires_at': '2099-01-01T00:00:00Z', 'source_context_ref': str(self.context_path),
            'source_path': occurrence_ref, 'source_access': access, 'source_binding': native,
            'profile_type_id': 'tos.entity.occurrence', 'record_id': record['record_id'],
            'allowed_operations': list(private.OPERATIONS), 'allowed_fields': ['preferred_label', 'notes'],
            'allowed_form_ids': ['tos.form.synthetic.layer-occurrence-name'],
            'provenance_event_id': 'tos.event.synthetic.layer-occurrence'}
        occurrence_owner = self.base / 'occurrence-owner.json'
        occurrence_owner.write_bytes(encode(occurrence_config))
        occurrence_owner.chmod(0o600)
        occurrence_proposal = {'schema_version': contract_request(), 'operation': 'prepare-create',
            'record': record, 'forms': [{'form_id': occurrence_config['allowed_form_ids'][0], 'field_id': 'metadata.preferred-name'}]}
        prepared = source.run_local_command(occurrence_owner, occurrence_proposal)
        occurrence_request = {**occurrence_proposal, 'operation': 'source.create', 'command_id': 'synthetic-layer-occurrence',
            'expected_configuration': prepared['owner_configuration'], 'expected_dependencies': prepared['expected_dependencies'],
            'expected_source': None, 'expected_revision': None}
        created = source.run_local_command(occurrence_owner, occurrence_request)
        self.assertFalse(created['publication_authorized'])
        self.assertEqual(json.loads(occurrence_path.read_bytes()), record)
        self.assertTrue(source.run_local_command(occurrence_owner, occurrence_request)['replayed'])
        self.assertEqual(self.path.read_bytes(), layer_raw)
        self.assertEqual(self.payload.read_bytes(), self.original)


def contract_request():
    return 'tos_local_source_command_v1'


class NativeLayerDerivationTests(unittest.TestCase):
    def setUp(self):
        from source_text_layer_proposal import derivation_policy
        self.seed = NativeLayerCommandTests()
        self.seed.setUp()
        self.addCleanup(self.seed.doCleanups)
        self.seed.created()
        self.base, self.store, self.public = self.seed.base, self.seed.store, self.seed.public
        self.prior_files = self.seed.files()
        self.prior = json.loads(self.seed.path.read_bytes())
        self.config = copy.deepcopy(self.seed.config)
        self.config.update(schema_version=layers.DERIVE_CONFIG,
            source_path=self.seed.prefix + 'layers/normalized/source-text-layer.v1.json',
            allowed_operations=['text-layer.normalize'],
            input={'kind': 'text_layer', 'binding': self.binding(self.seed.source_ref)}, material={},
            policy=derivation_policy('text-layer.normalize', unicode_form='NFC'))
        self.config.pop('member')
        self.config.pop('selector')
        self.config['identities'] = {'layer_id': 'tos.text-layer.sid-' + '5' * 32,
                                     'provenance_event_id': 'tos.event.sid-' + '6' * 32}
        self.config['source_access'] = {'read_scope': 'exact_text_layer', 'access_allowed': True,
            'byte_size': len(self.seed.content), 'authority_ref': 'operator:synthetic-layer-reading',
            'expires_at': '2099-01-01T00:00:00Z'}
        self.config['maker'].update(method='tos.unicode.normalize.v1', version=self.config['policy']['unicode_database_version'])
        self.rights_ref = self.seed.prefix + 'rights/derived-result.json'
        rights = copy.deepcopy(self.seed.fixture.rights)
        rights.update(rights_id='tos.rights.synthetic.new-derived-layer', scope_refs=[self.config['identities']['layer_id']])
        self.write_private(self.rights_ref, encode(rights))
        self.config['derivation_access'].update(operation='unicode_normalization', rights_record_refs=[
            copy.deepcopy(self.seed.config['derivation_access']['rights_record_refs'][0]),
            {'ref': self.rights_ref, 'sha256': source._digest((self.store / self.rights_ref).read_bytes())[7:]}])
        self.owner = self.base / 'derived-owner.json'
        self.path = self.store / self.config['source_path']
        self.write_owner()

    def write_private(self, ref, raw):
        path = self.store / ref
        path.write_bytes(raw)
        path.chmod(0o600)

    def write_owner(self):
        self.owner.write_bytes(encode(self.config))
        self.owner.chmod(0o600)

    def binding(self, ref):
        raw = (self.store / ref).read_bytes()
        row = json.loads(raw)
        return {'schema_version': 'tos_native_text_layer_binding_v1',
            'text_layer': {'record_ref': ref, 'record_sha256': source._digest(raw)[7:],
                'layer_id': row['layer_id'], 'layer_version': row['layer_version']},
            'source_record_refs': dict(self.seed.fixture.refs)}

    def invoke(self, request):
        return source.run_local_command(self.owner, request)

    def prepare(self):
        response = self.invoke({'schema_version': contract_request(), 'operation': 'prepare-create'})
        self.request = {'schema_version': contract_request(), 'operation': self.config['allowed_operations'][0],
            'command_id': 'synthetic-native-derivation', 'expected_configuration': response['owner_configuration'],
            'expected_dependencies': response['expected_dependencies'], 'expected_source': None, 'expected_revision': None}
        return response

    def created(self):
        self.prepare()
        return self.invoke(self.request)

    def output(self):
        return {path.name: path.read_bytes() for path in self.path.parent.iterdir()}

    def correction(self):
        from source_text_layer_proposal import derivation_policy
        self.config['allowed_operations'] = ['text-layer.correct']
        self.config['derivation_access']['operation'] = 'correction'
        self.config['policy'] = derivation_policy('text-layer.correct')
        self.config['maker'].update(maker_type='human', method='supplied synthetic correction', version='1')
        start = self.seed.content.decode().index('test')
        self.config['material'] = {'edits': [{'start': start, 'end': start + 4,
            'input_exact': 'test', 'input_sha256': source._digest(b'test')[7:], 'output_exact': 'trial',
            'reason': 'Synthetic source-visible correction proposal, not accepted reading.', 'confidence': 0.7}]}
        self.write_owner()

    def supplied(self, operation):
        from source_text_layer_proposal import derivation_policy
        self.config['allowed_operations'] = [operation]
        self.config['derivation_access']['operation'] = layers.DERIVE_OPERATIONS[operation]
        self.config['policy'] = derivation_policy(operation)
        self.config['maker'].update(method='record explicitly supplied text', version='1')
        self.config['source_access'] = copy.deepcopy(self.seed.config['source_access'])
        anchor = self.prior['source_binding']['anchors'][0]
        self.config['input'] = {'kind': 'acquired_file', 'anchor': {'anchor_id': anchor['anchor_id'],
            'record_ref': anchor['anchor_record_ref'], 'record_sha256': anchor['anchor_record_sha256']}}
        self.supplied_text = 'Supplied OCR & transcript\r\n\u212b fi\ufb01.'
        ref = self.seed.prefix + 'supplied-result.txt'
        self.write_private(ref, self.supplied_text.encode())
        self.config['material'] = {'content_ref': ref, 'content_sha256': source._digest(self.supplied_text.encode())[7:],
            'byte_size': len(self.supplied_text.encode()), 'access_allowed': True,
            'authority_ref': 'operator:synthetic-supplied-result-read', 'expires_at': '2099-01-01T00:00:00Z',
            'provider_execution': 'not_observed', 'reported_maker': {
                'maker_type': 'human' if operation == 'text-layer.record-transcription' else 'software',
                'agent_ref': 'synthetic:unverified-reported-producer', 'method': 'supplied method declaration', 'version': '1'}}
        self.write_owner()

    def test_normalization_is_new_exact_version_with_no_inherited_quality(self):
        import unicodedata
        result = self.created()
        files = self.output()
        row = json.loads(files[self.path.name])
        self.assertEqual(files['content.txt'], unicodedata.normalize('NFC', self.seed.content.decode()).encode())
        self.assertEqual(row['layer_version'], 2)
        self.assertEqual(row['supersedes_layer_ref'], self.prior['layer_id'])
        self.assertEqual(row['derivation']['input_layers'][0]['record_sha256'], self.binding(self.seed.source_ref)['text_layer']['record_sha256'])
        self.assertEqual(row['derivation']['change_payload']['operations'][0]['status'], 'proposed')
        self.assertEqual(row['admission']['accepted_uses'], [])
        self.assertFalse(row['admission']['human_review_performed'])
        self.assertEqual(row['layer_role'], 'normalized_text')
        self.assertEqual(json.loads(files['source-create-provenance.jsonl'])['activity']['event_type'], 'normalization')
        self.assertEqual(self.invoke(self.request)['receipt_sha256'], result['receipt_sha256'])
        self.assertEqual(self.output(), files)
        self.assertEqual(self.seed.files(), self.prior_files)
        self.assertEqual(self.seed.payload.read_bytes(), self.seed.original)
        resolver = NativeTextBindingResolver(self.public, owner_context=OwnerLocalSourceContext.load(self.seed.context_path))
        self.assertTrue(resolver.resolve_layer(self.binding(self.config['source_path']), verify_content=True, allow_private_content=True)['content_verified'])

    def test_supplied_correction_replays_exact_edits_without_admission(self):
        self.correction()
        self.created()
        row = json.loads(self.path.read_bytes())
        self.assertEqual(self.output()['content.txt'], self.seed.content.replace(b'test', b'trial'))
        self.assertEqual(row['derivation']['method'], 'correction')
        edit = row['derivation']['change_payload']['operations'][0]
        self.assertEqual(edit['input_exact'], 'test')
        self.assertEqual(edit['output_exact'], 'trial')
        self.assertEqual(edit['responsibility']['maker_type'], 'human')
        self.assertEqual(row['admission']['review_status'], 'unreviewed')
        self.assertFalse(row['admission']['human_review_performed'])
        self.assertEqual(self.seed.files(), self.prior_files)

    def test_supplied_ocr_is_annotation_not_provider_execution(self):
        self.supplied('text-layer.record-ocr')
        self.created()
        files = self.output()
        row, event = json.loads(files[self.path.name]), json.loads(files['source-create-provenance.jsonl'])
        self.assertEqual(files['content.txt'], self.supplied_text.encode())
        self.assertEqual(row['layer_role'], 'raw_ocr')
        self.assertEqual(row['derivation']['method'], 'ocr')
        self.assertEqual(row['derivation']['input_layers'], [])
        self.assertEqual(row['layer_version'], 1)
        self.assertEqual(event['activity']['event_type'], 'annotation')
        self.assertEqual(event['method']['model_invocations'], [])
        self.assertIn('without executing', event['activity']['warnings'][0])
        self.assertFalse(json.loads(files['derivation-policy.json'])['provider_execution_verified'])
        self.assertEqual(self.invoke(self.request)['replayed'], True)
        self.assertEqual(self.seed.payload.read_bytes(), self.seed.original)

    def test_supplied_manual_transcription_preserves_bytes_and_reported_origin(self):
        self.supplied('text-layer.record-transcription')
        self.created()
        row = json.loads(self.path.read_bytes())
        self.assertEqual(self.output()['content.txt'], self.supplied_text.encode())
        self.assertEqual(row['derivation']['method'], 'manual_transcription')
        self.assertEqual(row['representation']['character_normalization'], 'none')
        self.assertEqual(row['admission']['accepted_uses'], [])
        retained = json.loads(self.output()[layers.CONFIG_FILE])
        self.assertEqual(retained['material']['reported_maker']['maker_type'], 'human')
        self.assertEqual(retained['material']['provider_execution'], 'not_observed')

    def test_supplied_model_transcription_does_not_create_a_model_invocation(self):
        from source_text_layer_proposal import derivation_policy
        self.supplied('text-layer.record-transcription')
        self.config['policy'] = derivation_policy('text-layer.record-transcription', transcription_method='model_transcription')
        self.config['material']['reported_maker']['maker_type'] = 'model'
        self.config['derivation_access']['operation'] = 'model_transcription'
        self.write_owner()
        self.created()
        row = json.loads(self.path.read_bytes())
        event = json.loads(self.output()['source-create-provenance.jsonl'])
        self.assertEqual(row['derivation']['method'], 'model_transcription')
        self.assertEqual(row['layer_role'], 'machine_transcription')
        self.assertEqual(event['method']['model_invocations'], [])
        self.assertEqual(event['activity']['event_type'], 'annotation')

    def test_grants_input_versions_and_injection_fail_without_output(self):
        original = copy.deepcopy(self.config)
        cases = []
        for section, key, value in [('source_access', 'access_allowed', False),
                ('derivation_access', 'derivation_allowed', False), ('source_access', 'expires_at', '2000-01-01T00:00:00Z'),
                ('derivation_access', 'operation', 'correction')]:
            changed = copy.deepcopy(original)
            changed[section][key] = value
            cases.append(changed)
        changed = copy.deepcopy(original)
        changed['input']['binding']['text_layer']['layer_version'] += 1
        cases.append(changed)
        changed = copy.deepcopy(original)
        changed['derivation_access']['rights_record_refs'] = changed['derivation_access']['rights_record_refs'][:1]
        cases.append(changed)
        changed = copy.deepcopy(original)
        changed['policy']['unicode_database_version'] = 'invented'
        cases.append(changed)
        for index, changed in enumerate(cases):
            with self.subTest(case=index):
                self.config = changed
                self.write_owner()
                with self.assertRaises((ValueError, PermissionError)):
                    self.prepare()
                self.assertFalse(self.path.parent.exists())
        self.config = original
        self.write_owner()
        self.prepare()
        for request in ({**self.request, 'operation': 'text-layer.correct'}, {**self.request, 'content': 'injected'},
                        {**self.request, 'expected_source': self.prior['layer_id']}):
            with self.assertRaises((ValueError, PermissionError)):
                self.invoke(request)
        self.assertEqual(self.seed.files(), self.prior_files)

    def test_correction_refuses_wrong_text_overlap_and_missing_edit_evidence(self):
        self.correction()
        original = copy.deepcopy(self.config)
        for mutation in ('text', 'overlap', 'digest', 'empty', 'reason'):
            self.config = copy.deepcopy(original)
            edit = self.config['material']['edits'][0]
            if mutation == 'text':
                edit['input_exact'] = 'else'
            elif mutation == 'overlap':
                self.config['material']['edits'].append(copy.deepcopy(edit))
            elif mutation == 'digest':
                edit['input_sha256'] = '0' * 64
            elif mutation == 'empty':
                self.config['material']['edits'] = []
            else:
                edit['reason'] = ''
            self.write_owner()
            with self.subTest(case=mutation), self.assertRaises(ValueError):
                self.prepare()
            self.assertFalse(self.path.parent.exists())

    def test_supplied_source_and_new_layer_rights_precede_payload_io(self):
        self.supplied('text-layer.record-ocr')
        self.config['derivation_access']['rights_record_refs'] = self.config['derivation_access']['rights_record_refs'][:1]
        self.write_owner()
        with patch.object(layers, '_payload', side_effect=AssertionError('rights failure opened raw file')):
            with self.assertRaises(PermissionError):
                self.prepare()
        self.assertFalse(self.path.parent.exists())

    def test_supplied_claim_cannot_assert_provider_execution_or_change_bound_bytes(self):
        self.supplied('text-layer.record-ocr')
        self.config['material']['provider_execution'] = 'completed'
        self.write_owner()
        with self.assertRaises(PermissionError):
            self.prepare()
        self.config['material']['provider_execution'] = 'not_observed'
        self.write_owner()
        self.prepare()
        self.write_private(self.config['material']['content_ref'], b'changed supplied bytes')
        with self.assertRaises(ValueError):
            self.invoke(self.request)
        self.assertFalse(self.path.parent.exists())

    def test_supplied_missing_anchor_fixity_and_wrong_source_refuse_before_payload_io(self):
        self.supplied('text-layer.record-ocr')
        original = copy.deepcopy(self.config)
        for key, value in [('record_sha256', None), ('record_sha256', '0' * 64),
                           ('anchor_id', 'tos.anchor.synthetic.other')]:
            self.config = copy.deepcopy(original)
            self.config['input']['anchor'][key] = value
            self.write_owner()
            with (self.subTest(field=key, value=value),
                  patch.object(layers, '_payload', side_effect=AssertionError('unbound anchor opened raw file')),
                  self.assertRaises(ValueError)):
                self.prepare()

    def test_native_reader_rejects_forged_lineage_and_independently_replays_edits(self):
        self.created()
        original = self.path.read_bytes()
        row = json.loads(original)
        row['layer_version'] += 1
        self.write_private(self.config['source_path'], encode(row))
        context = OwnerLocalSourceContext.load(self.seed.context_path)
        with self.assertRaises(ValueError):
            NativeTextBindingResolver(self.public, owner_context=context).resolve_layer(self.binding(self.config['source_path']))
        row = json.loads(original)
        edit = row['derivation']['change_payload']['operations'][0]
        edit['input_exact'] = 'X' * len(edit['input_exact'])
        edit['input_sha256'] = source._digest(edit['input_exact'].encode())[7:]
        self.write_private(self.config['source_path'], encode(row))
        selected = self.binding(self.config['source_path'])
        self.assertTrue(NativeTextBindingResolver(self.public, owner_context=context).resolve_layer(selected)['metadata_verified'])
        with self.assertRaisesRegex(ValueError, 'does not replay'):
            NativeTextBindingResolver(self.public, owner_context=context).resolve_layer(selected,
                verify_content=True, allow_private_content=True)
        # Writer recovery independently rejects tampered committed bytes too.
        with self.assertRaises(ValueError):
            self.invoke(self.request)

    def test_original_extraction_grant_does_not_authorize_derivation(self):
        with self.assertRaises(ValueError):
            source.run_local_command(self.seed.owner, {'schema_version': contract_request(),
                'operation': 'text-layer.normalize', 'command_id': 'not-delegated',
                'expected_configuration': None, 'expected_source': None,
                'expected_revision': None, 'expected_dependencies': None})

    def test_correction_normalization_then_first_segmentation_keeps_each_layer(self):
        from source_text_layer_proposal import derivation_policy
        self.correction()
        self.created()
        corrected_ref, corrected_files = self.config['source_path'], self.output()
        self.config['source_path'] = self.seed.prefix + 'layers/final-normalized/source-text-layer.v1.json'
        self.path = self.store / self.config['source_path']
        self.config['input']['binding'] = self.binding(corrected_ref)
        self.config['identities'] = {'layer_id': 'tos.text-layer.sid-' + '7' * 32,
                                    'provenance_event_id': 'tos.event.sid-' + '8' * 32}
        rights = copy.deepcopy(self.seed.fixture.rights)
        rights.update(rights_id='tos.rights.synthetic.normalized-successor', scope_refs=[self.config['identities']['layer_id']])
        rights_ref = self.seed.prefix + 'rights/normalized-successor.json'
        self.write_private(rights_ref, encode(rights))
        self.config['derivation_access']['rights_record_refs'][1] = {'ref': rights_ref, 'sha256': source._digest(encode(rights))[7:]}
        self.config['source_access']['byte_size'] = len(corrected_files['content.txt'])
        self.config['allowed_operations'] = ['text-layer.normalize']
        self.config['derivation_access']['operation'] = 'unicode_normalization'
        self.config['policy'] = derivation_policy('text-layer.normalize', unicode_form='NFC')
        self.config['material'] = {}
        self.config['maker'].update(maker_type='software', method='tos.unicode.normalize.v1', version=self.config['policy']['unicode_database_version'])
        self.write_owner()
        self.created()
        layer = json.loads(self.path.read_bytes())
        self.assertEqual(layer['layer_version'], 3)
        self.assertEqual(layer['supersedes_layer_ref'], json.loads(corrected_files[self.path.name])['layer_id'])
        self.assertTrue(self.invoke(self.request)['replayed'])
        unit_config = copy.deepcopy(self.seed.seed.config)
        unit_config.update(schema_version=units.LAYER_CONFIG, source_binding=self.binding(self.config['source_path']),
            allowed_text_scope={'start': 0, 'end': layer['representation']['text_scope']['end']})
        unit_config['unit_slots'] = [unit_config['unit_slots'][0]]
        unit_config['unit_slots'][0]['unit_kind'] = 'document'
        unit_config['scheme'].update(analysis_role='source_structure', boundary_basis='source_markup')
        unit_config['scheme']['policies'].update(whitespace='included_in_neighbor', line_break='included_in_neighbor')
        unit_owner = self.base / 'derived-first-segmentation.json'
        unit_owner.write_bytes(encode(unit_config))
        unit_owner.chmod(0o600)
        request = copy.deepcopy(self.seed.seed.proposal)
        request['spans'] = [{**request['spans'][0], 'start': 0, 'end': unit_config['allowed_text_scope']['end']}]
        request['excluded_gaps'] = []
        prepared = source.run_local_command(unit_owner, request)
        request.update(operation=units.OPERATION, command_id='synthetic-derived-first-segmentation',
            expected_configuration=prepared['owner_configuration'], expected_dependencies=prepared['expected_dependencies'],
            expected_source=None, expected_revision=None)
        result = source.run_local_command(unit_owner, request)
        self.assertFalse(result['grants_admission'])
        packet = json.loads((self.store / unit_config['source_path']).read_bytes())
        self.assertEqual(packet['source_layer']['unicode_form'], 'NFC')
        self.assertEqual(packet['source_layer']['text_layer_ref'], self.config['source_path'])
        self.assertEqual((self.store / corrected_ref).read_bytes(), corrected_files[self.path.name])
        self.assertEqual(self.seed.files(), self.prior_files)

    def test_derived_recovery_reuses_exact_plan_and_preserves_predecessor(self):
        self.prepare()
        write = layers._write_new
        def interrupted(path, raw):
            write(path, raw)
            if path.name == 'content.txt':
                raise OSError('synthetic interruption after complete derived content')
        with patch.object(layers, '_write_new', side_effect=interrupted), self.assertRaises(OSError):
            self.invoke(self.request)
        self.assertFalse(self.path.parent.exists())
        before = set(self.store.glob('.native-construction-*.pending'))
        self.invoke(self.request)
        self.assertEqual(set(self.store.glob('.native-construction-*.pending')), before)
        self.assertEqual(self.seed.files(), self.prior_files)
        self.assertTrue(self.invoke(self.request)['replayed'])

    def test_changed_predecessor_blocks_recovery_without_erasing_evidence(self):
        self.prepare()
        with patch.object(source, '_publish_new_directory', side_effect=OSError('synthetic prepublication interruption')):
            with self.assertRaises(OSError):
                self.invoke(self.request)
        before = set(self.store.glob('.native-construction-*.pending'))
        content = self.seed.path.parent / 'content.txt'
        content.write_bytes(content.read_bytes() + b'foreign')
        for _ in range(2):
            with self.assertRaises(ValueError):
                self.invoke(self.request)
        self.assertEqual(set(self.store.glob('.native-construction-*.pending')), before)
        self.assertFalse(self.path.parent.exists())
        self.assertTrue(content.read_bytes().endswith(b'foreign'))

    def test_derivation_write_grant_cannot_supply_current_assessment_read_authority(self):
        import native_text_layer_assessment as assessment
        self.created()
        with patch.object(assessment.NativeTextBindingResolver, '_read', side_effect=AssertionError('source read')):
            with self.assertRaises(ValueError):
                assessment.NativeLayerAssessmentSources(OwnerLocalSourceContext.load(self.seed.context_path),
                    [{'binding': self.binding(self.config['source_path'])}], {})

    def test_describe_discovery_and_result_do_not_disclose_native_content(self):
        with patch.object(layers, '_prepare_derivation', side_effect=AssertionError('describe opened source')):
            described = self.invoke({'schema_version': contract_request(), 'operation': 'describe'})
        prepared, created = self.prepare(), self.invoke(self.request)
        for result in (described, prepared, created, self.invoke(self.request), source.discover_commands()):
            value = json.dumps(result)
            for secret in (self.seed.content.decode(), self.config['source_path'], self.prior['layer_id'], str(self.owner)):
                self.assertNotIn(secret, value)
            self.assertFalse(result['grants_admission'])


if __name__ == '__main__':
    unittest.main()

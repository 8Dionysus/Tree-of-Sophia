"""Synthetic exact-source comparison checks, never textual acceptance.

All Items, grants and source bytes are temporary fixtures. The reusable fixture
constructs inert packages directly; it never invokes a writer or grants access
to retained source material.
"""
from __future__ import annotations

import copy
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import sys
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'tests'))
sys.path.insert(0, str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'))

import test_source_text_layer_commands as construction_tests
import native_text_layer_assessment as assessment
from knowledge_assessment import Record, _canonical
from source_owner_context import OwnerLocalSourceContext
from source_text_layer_proposal import build_text_layer_proposal, record_bytes


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


class NativeLayerAssessmentFixture:
    """An independently readable synthetic extraction and exact assessment scope."""

    def __init__(self, test_case):
        self.seed = construction_tests.NativeLayerCommandTests()
        self.seed.setUp()
        test_case.addCleanup(self.seed.doCleanups)
        for name in ('base', 'public', 'store', 'payload', 'payload_root',
                     'context_path', 'source_ref', 'member', 'content'):
            setattr(self, name, getattr(self.seed, name))
        self.fixture = self.seed.fixture
        self.configuration = copy.deepcopy(self.seed.config)
        self.grant = copy.deepcopy(self.configuration['source_access'])
        self.layer_id = self.configuration['identities']['layer_id']
        self.fixture.write_bytes('ToS/contracts/' + assessment.COMPARISON_SCHEMA,
            (ROOT / 'ToS/contracts' / assessment.COMPARISON_SCHEMA).read_bytes())
        self.rebuild()

    def rebuild(self, configuration=None, text=None):
        if configuration is not None:
            self.configuration = copy.deepcopy(configuration)
        config = self.configuration
        home = Path(self.source_ref).parent
        refs = {'layer_ref': self.source_ref, 'anchor_ref': str(home / 'anchor.json'),
            'content_ref': str(home / 'content.txt'), 'policy_ref': str(home / 'policy.json'),
            'configuration_ref': str(home / assessment.construction.CONFIG_FILE),
            'configuration_sha256': digest(record_bytes(config)),
            'source_payload_ref': str(Path(config['source_record_refs']['item']).parent / 'payload/source.epub')}
        proposal = build_text_layer_proposal(exact_text=self.content.decode() if text is None else text,
            source_scope=config['source_scope'], identities=config['identities'], refs=refs,
            member=config['member'], selector=config['selector'], policy=config['policy'],
            maker=config['maker'], language=config['language'],
            rights_record_refs=config['derivation_access']['rights_record_refs'])
        destination = self.store / home
        destination.mkdir(mode=0o700, exist_ok=True)
        for name, raw in ((Path(self.source_ref).name, record_bytes(proposal['layer'])),
                ('anchor.json', record_bytes(proposal['anchor'])), ('content.txt', proposal['content']),
                ('policy.json', record_bytes(proposal['policy'])),
                (assessment.construction.CONFIG_FILE, record_bytes(config))):
            path = destination / name
            path.write_bytes(raw)
            path.chmod(0o600)
        self.write_layer(proposal['layer'])
        self.context = OwnerLocalSourceContext.load(self.context_path)

    def write_layer(self, layer):
        self.layer = copy.deepcopy(layer)
        self.layer_id = layer['layer_id']
        raw = record_bytes(layer)
        path = self.store / self.source_ref
        path.write_bytes(raw)
        path.chmod(0o600)
        self.binding = {'schema_version': 'tos_native_text_layer_binding_v1',
            'text_layer': {'record_ref': self.source_ref, 'record_sha256': digest(raw),
                'layer_id': self.layer_id, 'layer_version': layer['layer_version']},
            'source_record_refs': copy.deepcopy(self.configuration['source_record_refs'])}
        self.selections = [{'binding': self.binding, 'origin_id': 'synthetic-private-layer',
            'source_access': {'read_scope': 'exact_owner_local', 'access_allowed': True,
                              'authority_ref': 'operator:synthetic-current-metadata-and-representation-read'},
            'payload_access': self.grant}]
        record = Record.from_payload(self.layer_id, layer['layer_version'], layer)
        self.subjects = {self.layer_id: {'record': record.ref, 'assertion_layer': 'textual_observation',
            'risk': 'low', 'languages': [layer['representation']['language']],
            'maker_id': layer['derivation']['maker']['agent_ref'],
            'requested_use': 'text-layer:citation', 'access_allowed': True}}

    def reader(self):
        return assessment.NativeLayerAssessmentSources(self.context, self.selections, self.subjects)

    def replace_member(self, member):
        """Replace only this fixture's acquired Item and update its exact inputs."""
        self.member = member
        original = self.seed.epub(member)
        self.payload.write_bytes(original)
        file_sha = digest(original)
        file_id = 'tos.file.sha256.' + file_sha
        entry = self.fixture.manifest['payload_files'][0]
        entry.update(file_id=file_id, sha256=file_sha, byte_size=len(original))
        self.fixture.rights['scope_refs'] = [self.fixture.ids['item'], file_id]
        self.fixture.write_json(self.fixture.manifest_ref, self.fixture.manifest)
        self.fixture.write_json(self.fixture.rights_ref, self.fixture.rights)
        config = self.configuration
        config['source_scope'].update(file_ref=file_id, file_sha256=file_sha)
        config['manifest_sha256'] = self.fixture.file_digest(self.fixture.manifest_ref)
        config['derivation_access']['rights_record_refs'][0]['sha256'] = self.fixture.file_digest(self.fixture.rights_ref)
        config['source_access']['byte_size'] = len(original)
        self.grant['byte_size'] = len(original)
        config['member']['member_sha256'] = digest(member)
        self.content = assessment.extract_xhtml_text(member, selector=config['selector'], policy=config['policy']).encode()
        self.rebuild()


class NativeLayerAssessmentTests(unittest.TestCase):
    def setUp(self):
        self.fx = NativeLayerAssessmentFixture(self)

    def test_exact_comparison_preserves_raw_layer_and_exposes_no_quality_verdict(self):
        reader = self.fx.reader()
        view = reader.layers[self.fx.layer_id]
        comparison = view['comparison']
        body = comparison.payload
        self.assertTrue(view['read_ready'])
        self.assertEqual(view['record'].payload, self.fx.layer)
        self.assertEqual(body['layer'], view['record'].ref)
        self.assertEqual(body['source_member_utf8'], self.fx.member.decode())
        self.assertEqual(body['expected_text'], self.fx.content.decode())
        self.assertEqual(body['representation_text'], self.fx.content.decode())
        self.assertEqual(body['representation'], self.fx.layer['representation'])
        self.assertTrue(body['deterministic_match'])
        self.assertFalse(body['performs_semantic_assessment'])
        self.assertFalse(body['publication_authorized'])
        self.assertEqual(body['visibility'], 'local_only')
        self.assertEqual(body['editorial_policy']['quality_assessment'], 'not-performed')
        self.assertEqual(body['comparison_id'], 'tos.text-comparison.sha256.' +
            digest(_canonical({key: value for key, value in body.items() if key != 'comparison_id'})))
        self.assertEqual([row['payload'] for row in reader.records], [self.fx.layer, body])
        self.assertEqual([row['origin_id'] for row in reader.records], ['synthetic-private-layer'] * 2)
        self.assertIn('ToS/contracts/' + assessment.COMPARISON_SCHEMA, reader.contracts)
        self.assertEqual(reader.snapshot(), reader.snapshot())

    def test_metadata_only_never_reads_original_or_representation(self):
        self.fx.selections[0]['source_access']['read_scope'] = 'metadata_only'
        self.fx.selections[0]['payload_access'] = None
        self.fx.payload.unlink()
        (self.fx.store / self.fx.layer['representation']['content_ref']).unlink()
        with patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')):
            reader = self.fx.reader()
            reader.snapshot()
        self.assertFalse(reader.layers[self.fx.layer_id]['read_ready'])
        self.assertIsNone(reader.layers[self.fx.layer_id]['comparison'])
        self.assertEqual(len(reader.records), 1)

    def test_historical_creation_grants_are_data_not_current_authority(self):
        config = copy.deepcopy(self.fx.configuration)
        config['expires_at'] = '2000-01-01T00:00:00Z'
        for name in ('source_access', 'derivation_access'):
            config[name]['expires_at'] = '2000-01-01T00:00:00Z'
        config['source_access']['payload_root'] = str(self.fx.base / 'historical-not-current')
        config['source_access']['access_allowed'] = False
        config['derivation_access']['derivation_allowed'] = False
        self.fx.rebuild(configuration=config)
        self.assertTrue(self.fx.reader().layers[self.fx.layer_id]['read_ready'])

    def test_current_grant_and_scope_validation_precedes_all_context_or_source_io(self):
        cases = []
        for key, value in (('access_allowed', False), ('expires_at', '2000-01-01T00:00:00Z'),
                           ('authority_ref', ''), ('read_scope', 'any_file')):
            selections = copy.deepcopy(self.fx.selections)
            selections[0]['payload_access'][key] = value
            cases.append((selections, self.fx.subjects))
        selection = copy.deepcopy(self.fx.selections[0])
        selection['binding']['text_layer']['layer_id'] = 'tos.text-layer.synthetic.second'
        selection['payload_access']['access_allowed'] = False
        cases.append((self.fx.selections + [selection], self.fx.subjects))
        subjects = copy.deepcopy(self.fx.subjects)
        subjects[self.fx.layer_id]['requested_use'] = []
        cases.append((self.fx.selections, subjects))
        for selections, subjects in cases:
            with self.subTest(selection=selections[-1]['binding']['text_layer']['layer_id']), \
                    patch.object(OwnerLocalSourceContext, 'snapshot', side_effect=AssertionError('context read')), \
                    patch.object(assessment, '_open', side_effect=AssertionError('source read')), \
                    self.assertRaises((ValueError, PermissionError)):
                assessment.NativeLayerAssessmentSources(self.fx.context, selections, subjects)

    def test_scope_metadata_mismatch_fails_before_original_read(self):
        for field, value in (('maker_id', 'software:another-maker'), ('languages', ['ru']),
                             ('record', {**self.fx.subjects[self.fx.layer_id]['record'], 'digest': 'sha256:' + '0' * 64})):
            subjects = copy.deepcopy(self.fx.subjects)
            subjects[self.fx.layer_id][field] = value
            with self.subTest(field=field), \
                    patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                    self.assertRaises(PermissionError):
                assessment.NativeLayerAssessmentSources(self.fx.context, self.fx.selections, subjects)

    def test_deterministic_mismatch_keeps_both_texts_but_is_not_ready(self):
        self.fx.rebuild(text='Different extraction.')
        view = self.fx.reader().layers[self.fx.layer_id]
        self.assertFalse(view['read_ready'])
        self.assertFalse(view['comparison'].payload['deterministic_match'])
        self.assertEqual(view['comparison'].payload['expected_text'], self.fx.content.decode())
        self.assertEqual(view['comparison'].payload['representation_text'], 'Different extraction.')

    def test_current_grant_exact_size_must_match_original_manifest(self):
        self.fx.grant['byte_size'] += 1
        with patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                self.assertRaises(PermissionError):
            self.fx.reader()

    def test_snapshot_rejects_rights_or_grant_drift_before_original_read(self):
        reader = self.fx.reader()
        self.fx.grant['expires_at'] = '2000-01-01T00:00:00Z'
        with patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                self.assertRaises(PermissionError):
            reader.snapshot()
        self.fx.grant['expires_at'] = '2099-01-01T00:00:00Z'
        rights_path = self.fx.public / self.fx.fixture.rights_ref
        rights_path.write_bytes(rights_path.read_bytes() + b'\n')
        with patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                self.assertRaises(ValueError):
            reader.snapshot()

    def test_snapshot_rejects_original_identity_replacement(self):
        reader = self.fx.reader()
        old = self.fx.payload.with_suffix('.retained')
        self.fx.payload.rename(old)
        self.fx.payload.write_bytes(old.read_bytes())
        self.fx.payload.chmod(0o600)
        with self.assertRaises(ValueError):
            reader.snapshot()

    def test_symlink_original_fails_without_disclosing_local_path(self):
        old = self.fx.payload.with_suffix('.retained')
        self.fx.payload.rename(old)
        self.fx.payload.symlink_to(old)
        with self.assertRaises(ValueError) as caught:
            self.fx.reader()
        self.assertNotIn(str(self.fx.payload), str(caught.exception))
        self.assertNotIn(str(self.fx.payload_root), str(caught.exception))

    def test_selection_limits_are_pure_and_never_truncate(self):
        with self.assertRaises(ValueError):
            assessment.preflight_layer_selections(self.fx.selections * 9, self.fx.subjects)
        selections, subjects = [], {}
        for index in range(2):
            selection = copy.deepcopy(self.fx.selections[0])
            target = selection['binding']['text_layer']
            target['layer_id'] = 'tos.text-layer.synthetic.' + str(index)
            selection['payload_access']['byte_size'] = assessment.MAX_ORIGINAL_BYTES
            subject = copy.deepcopy(self.fx.subjects[self.fx.layer_id])
            subject['record']['id'] = target['layer_id']
            selections.append(selection)
            subjects[target['layer_id']] = subject
        with patch.object(assessment, '_open', side_effect=AssertionError('source read')), \
                self.assertRaises(ValueError):
            assessment.preflight_layer_selections(selections, subjects)

    def test_unsupported_derivation_profile_fails_before_original_read(self):
        original = copy.deepcopy(self.fx.layer)
        cases = []
        for field, value in (('method', 'ocr'), ('change_payload', {'kind': 'unspecified'})):
            layer = copy.deepcopy(original)
            layer['derivation'][field] = value
            cases.append(layer)
        layer = copy.deepcopy(original)
        layer['representation']['character_normalization'] = 'NFC'
        cases.append(layer)
        layer = copy.deepcopy(original)
        layer['source_binding']['anchors'] *= 2
        cases.append(layer)
        layer = copy.deepcopy(original)
        layer['derivation']['input_layers'] = [{'layer_id': layer['layer_id'],
            'record_ref': self.fx.source_ref, 'record_sha256': self.fx.binding['text_layer']['record_sha256'],
            'content_sha256': layer['representation']['content_sha256']}]
        cases.append(layer)
        for layer in cases:
            self.fx.write_layer(layer)
            with self.subTest(method=layer['derivation']['method']), \
                    patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                    self.assertRaises(ValueError):
                self.fx.reader()

    def test_fixed_configuration_selector_mismatch_is_not_reinterpreted(self):
        config = copy.deepcopy(self.fx.configuration)
        config['selector']['value'] = 'p:1'
        self.fx.rebuild(configuration=config)
        anchor_ref = self.fx.layer['source_binding']['anchors'][0]['anchor_record_ref']
        path = self.fx.store / anchor_ref
        anchor = json.loads(path.read_bytes())
        anchor['selector_payload']['expression']['steps'][1]['selector']['value'] = 'p:2'
        raw = record_bytes(anchor)
        path.write_bytes(raw)
        layer = copy.deepcopy(self.fx.layer)
        layer['source_binding']['anchors'][0]['anchor_record_sha256'] = digest(raw)
        self.fx.write_layer(layer)
        with patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                self.assertRaises(assessment.NativeLayerAssessmentError):
            self.fx.reader()

    def test_record_budget_refuses_a_full_comparison_without_truncation(self):
        member = b'<html xmlns="http://www.w3.org/1999/xhtml"><body><p>First.</p><p>' + b'x' * 350000 + b'</p></body></html>'
        self.fx.replace_member(member)
        self.assertLess(len(self.fx.member), assessment.MAX_RECORD_BYTES)
        self.assertLess(len(self.fx.content), assessment.MAX_RECORD_BYTES)
        with self.assertRaisesRegex(ValueError, 'bounded assessment input size'):
            self.fx.reader()

    def test_schema_and_private_representation_drift_invalidate_snapshot(self):
        reader = self.fx.reader()
        path = self.fx.store / self.fx.layer['representation']['content_ref']
        raw = path.read_bytes()
        path.write_bytes(raw + b' ')
        with self.assertRaises(ValueError):
            reader.snapshot()
        path.write_bytes(raw)
        reader = self.fx.reader()
        schema_path = self.fx.public / 'ToS/contracts' / assessment.COMPARISON_SCHEMA
        schema_path.write_bytes(schema_path.read_bytes() + b'\n')
        with self.assertRaises(ValueError):
            reader.snapshot()

    def test_private_permission_or_ancestor_replacement_invalidates_snapshot(self):
        reader = self.fx.reader()
        layer_home = (self.fx.store / self.fx.source_ref).parent
        saved = layer_home.with_name('retained')
        layer_home.rename(saved)
        layer_home.mkdir(mode=0o700)
        for old in saved.iterdir():
            path = layer_home / old.name
            path.write_bytes(old.read_bytes())
            path.chmod(0o600)
        with self.assertRaises(ValueError):
            reader.snapshot()
        reader = self.fx.reader()
        (layer_home / 'content.txt').chmod(0o640)
        with self.assertRaises(ValueError):
            reader.snapshot()

    def test_grant_mutation_while_metadata_resolves_prevents_original_read(self):
        resolve = assessment.NativeTextBindingResolver.resolve_layer
        def revoke(*args, **kwargs):
            result = resolve(*args, **kwargs)
            self.fx.grant['access_allowed'] = False
            return result
        with patch.object(assessment.NativeTextBindingResolver, 'resolve_layer', side_effect=revoke, autospec=True), \
                patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                self.assertRaises(PermissionError):
            self.fx.reader()

    def test_payload_deadline_does_not_outlive_current_grant(self):
        self.fx.grant['expires_at'] = (datetime.now(timezone.utc) + timedelta(seconds=10)).isoformat()
        read = assessment._read_payload
        remaining = []
        def observed(config, entry, deadline):
            remaining.append(deadline - time.monotonic())
            return read(config, entry, deadline)
        with patch.object(assessment, '_read_payload', side_effect=observed):
            self.fx.reader()
        self.assertTrue(remaining)
        self.assertTrue(all(0 < value <= 10 for value in remaining))

    def test_comparison_and_snapshots_make_no_source_writes(self):
        paths = [path for root in (self.fx.public, self.fx.store, self.fx.payload_root)
                 for path in root.rglob('*') if path.is_file()]
        before = {path: (path.read_bytes(), path.stat().st_mode, path.stat().st_mtime_ns) for path in paths}
        reader = self.fx.reader()
        reader.snapshot()
        self.assertEqual(before, {path: (path.read_bytes(), path.stat().st_mode, path.stat().st_mtime_ns) for path in paths})
        self.assertEqual(set(paths), {path for root in (self.fx.public, self.fx.store, self.fx.payload_root)
                                    for path in root.rglob('*') if path.is_file()})

    def test_later_layer_metadata_mismatch_prevents_every_original_read(self):
        selection = copy.deepcopy(self.fx.selections[0])
        target = selection['binding']['text_layer']
        target['layer_id'] = 'tos.text-layer.synthetic.second'
        scope = copy.deepcopy(self.fx.subjects[self.fx.layer_id])
        scope['record']['id'] = target['layer_id']
        self.fx.selections.append(selection)
        self.fx.subjects[target['layer_id']] = scope
        with patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                self.assertRaises(ValueError):
            self.fx.reader()

    def test_unsupported_fixed_policy_does_not_read_original(self):
        layer = copy.deepcopy(self.fx.layer)
        policy_path = self.fx.store / layer['editorial_policy']['policy_ref']
        policy = json.loads(policy_path.read_bytes())
        policy['unicode_normalization'] = 'NFC'
        raw = record_bytes(policy)
        policy_path.write_bytes(raw)
        layer['editorial_policy']['policy_sha256'] = digest(raw)
        self.fx.write_layer(layer)
        with patch.object(assessment, '_read_payload', side_effect=AssertionError('original read')), \
                self.assertRaisesRegex(ValueError, 'unsupported XHTML extraction policy'):
            self.fx.reader()

    def test_inert_comparison_does_not_launch_network_or_processes(self):
        with patch('socket.socket.connect', side_effect=AssertionError('network operation')), \
                patch('subprocess.Popen', side_effect=AssertionError('process operation')):
            reader = self.fx.reader()
            reader.snapshot()


if __name__ == '__main__':
    unittest.main()

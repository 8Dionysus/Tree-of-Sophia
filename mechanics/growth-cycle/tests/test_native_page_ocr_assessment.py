"""Synthetic fixtures protect independent image/disclosure/quality boundaries.

Receipt authentication is isolated here; the owner-bridge tests own real
signature refusal. These tests never supply historical access or competence.
"""
from __future__ import annotations
import copy
import hashlib
import json
import os
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import patch
import zlib

ROOT = Path(__file__).resolve().parents[3]
sys.path[:0] = [str(ROOT / 'scripts'), str(ROOT / 'tests'),
    str(ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts')]
import native_page_ocr_assessment as assessment
import native_owner_ocr
import source_text_layer_commands as construction
import test_source_text_layer_commands as construction_tests
from knowledge_assessment import Record
from source_owner_context import OwnerLocalSourceContext
from source_text_layer_proposal import derivation_policy


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True) + '\n').encode()


def png():
    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 3, 2, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress((b'\0' + b'\xff' * 9) * 2)) + chunk(b'IEND', b'')


class ImageComparisonFixture:
    def __init__(self, test, *, synthetic=False, disclose=False):
        self.seed = construction_tests.NativeLayerDerivationTests()
        self.seed.setUp()
        test.addCleanup(self.seed.doCleanups)
        seed = self.seed
        seed.owner_page_ocr()
        self.config = seed.config
        self.image = seed.base / 'retained.png'
        self.raw = png()
        self.image.write_bytes(self.raw)
        self.image.chmod(0o600)
        selected = self.config['material']['input_representation']
        selected.update(input_file_ref='tos.file.sha256.' + digest(self.raw), input_sha256=digest(self.raw),
            input_bytes=len(self.raw), width_pixels=3, height_pixels=2)
        if synthetic:
            fixture = seed.seed.fixture
            entry = fixture.manifest['payload_files'][-1]
            self.image = seed.pdf_path.with_suffix('.png')
            self.image.write_bytes(self.raw)
            self.image.chmod(0o600)
            entry.update(file_id='tos.file.sha256.' + digest(self.raw), relative_path='payload/source-page.png',
                original_basename='source-page.png', media_type='image/png', byte_size=len(self.raw), sha256=digest(self.raw))
            fixture.rights['scope_refs'].append(entry['file_id'])
            fixture.write_json(fixture.manifest_ref, fixture.manifest)
            fixture.write_json(fixture.rights_ref, fixture.rights)
            self.config.update(schema_version=native_owner_ocr.OWNER_OCR_CONFIG,
                allowed_operations=[native_owner_ocr.OWNER_OCR_OPERATION], policy=derivation_policy(native_owner_ocr.OWNER_OCR_OPERATION),
                manifest_sha256=fixture.file_digest(fixture.manifest_ref))
            self.config['source_scope'].update(file_ref=entry['file_id'], file_sha256=entry['sha256'])
            self.config['source_access']['byte_size'] = len(self.raw)
            self.config['derivation_access']['rights_record_refs'][0]['sha256'] = fixture.file_digest(fixture.rights_ref)
            self.config['material'].pop('input_representation')
            self.config['input']['kind'] = 'acquired_file'
            anchor_path = seed.store / self.config['input']['anchor']['record_ref']
            anchor = json.loads(anchor_path.read_bytes())
            anchor['target'].update(file_id=entry['file_id'], file_sha256=entry['sha256'], media_type='image/png')
            envelope = anchor['selector_payload']['expression']['selector']
            envelope['state'].update(representation_ref=fixture.item_home + '/payload/source-page.png', representation_sha256=entry['sha256'], media_type='image/png')
            envelope['selector'] = {'type': 'page_region', 'page_identity': {'page_number': 1}, 'x': 0, 'y': 0,
                'width': 3, 'height': 2, 'source_width': 3, 'source_height': 2, 'coordinate_space': 'pixels'}
            seed.write_private(self.config['input']['anchor']['record_ref'], encoded(anchor))
            self.config['input']['anchor']['record_sha256'] = digest(encoded(anchor))
        receipt = {'owner': {'owner_repo': 'abyss-stack', 'source_ref': self.config['material']['owner_source_ref'],
            'adapter_path': native_owner_ocr.ADAPTER if synthetic else native_owner_ocr.PAGE_ADAPTER,
            'adapter_sha256': self.config['material']['adapter_sha256']}}
        if not synthetic:
            receipt['input_verification'] = {'schema_version': 'tos_retained_pdf_page_verification_capture_v1',
                'render_execution': 'not_performed', 'historical_receipt_signature': 'absent'}
        seed.owner_files['owner-ocr-receipt.json'] = encoded(receipt)
        self.config['material']['receipt_sha256'] = digest(encoded(receipt))
        seed.write_owner()
        with patch.object(construction, 'verify_owner_ocr', return_value=seed.supplied_text.encode()), \
                patch.object(construction, 'evidence_bytes', return_value=seed.owner_files):
            seed.created()
        for name in (assessment.COMPARISON_SCHEMA, assessment.base.COMPARISON_SCHEMA):
            seed.seed.fixture.write_bytes('ToS/contracts/' + name, (ROOT / 'ToS/contracts' / name).read_bytes())
        self.layer = json.loads(seed.path.read_bytes())
        self.binding = seed.binding(self.config['source_path'])
        self.context = OwnerLocalSourceContext.load(seed.seed.context_path)
        self.layer_id = self.layer['layer_id']
        record = Record.from_payload(self.layer_id, 1, self.layer)
        self.subjects = {self.layer_id: {'record': record.ref, 'assertion_layer': 'textual_observation', 'risk': 'low',
            'languages': ['de'], 'maker_id': self.layer['derivation']['maker']['agent_ref'],
            'requested_use': 'text-layer:citation', 'access_allowed': True}}
        self.selections = [{'binding': self.binding, 'origin_id': 'synthetic-image-comparison',
            'source_access': {'read_scope': 'exact_owner_local', 'access_allowed': True, 'authority_ref': 'operator:synthetic-test-read'},
            'payload_access': copy.deepcopy(self.config['source_access']), 'comparison_profile': assessment.SYNTHETIC_PROFILE if synthetic else assessment.PROFILE,
            'image_access': {'read_scope': 'exact_retained_page', 'access_allowed': True, 'authority_ref': 'operator:synthetic-test-image-read',
                'expires_at': '2099-01-01T00:00:00Z', 'path': str(self.image), 'byte_size': len(self.raw), 'sha256': digest(self.raw),
                'page_number': 1 if synthetic else 44, 'source_file_ref': self.config['source_scope']['file_ref'],
                'source_file_sha256': self.config['source_scope']['file_sha256'], 'processing_boundary': 'local_only',
                'width_pixels': 3, 'height_pixels': 2}, 'disclosure_access': None}]
        if disclose:
            self.selections[0]['disclosure_access'] = self.disclosure()

    def disclosure(self):
        image = self.selections[0]['image_access']
        return {'allowed': True, 'authority_ref': 'operator:synthetic-test-exact-disclosure', 'expires_at': '2099-01-01T00:00:00Z',
            'read_scope': 'exact_source_image_and_ocr', 'basis': 'operator_created_synthetic_source', 'processing_boundary': 'current_assistant_session',
            'source_file_ref': image['source_file_ref'], 'source_file_sha256': image['source_file_sha256'], 'image_sha256': image['sha256'],
            'layer_record_sha256': self.binding['text_layer']['record_sha256']}

    def reader(self):
        with patch.object(native_owner_ocr, 'verify_owner_ocr', return_value={}):
            return assessment.NativePageOCRAssessmentSources(self.context, self.selections, self.subjects)


class NativePageOCRAssessmentTests(unittest.TestCase):
    def test_retained_comparison_keeps_original_png_and_ocr_layers_distinct_without_disclosure(self):
        fx = ImageComparisonFixture(self)
        before = fx.seed.output()
        reader = fx.reader()
        body = reader.layers[fx.layer_id]['comparison'].payload
        self.assertTrue(reader.layers[fx.layer_id]['read_ready'])
        self.assertNotEqual(body['source_scope']['file_sha256'], body['source_image']['sha256'])
        self.assertEqual(body['source_image']['sha256'], digest(fx.raw))
        self.assertFalse(body['source_image']['model_disclosure_authorized'])
        self.assertIsNone(body['deterministic_text_match'])
        self.assertIsNone(body['disclosure_access'])
        self.assertEqual(body['source_visible_judgment'], 'not_performed')
        self.assertEqual(body['owner_execution']['input_verification']['render_execution'], 'not_performed')
        self.assertEqual(fx.seed.output(), before)
        self.assertEqual(fx.layer['admission']['accepted_uses'], [])

    def test_synthetic_positive_comparison_needs_its_own_exact_disclosure_grant(self):
        fx = ImageComparisonFixture(self, synthetic=True, disclose=True)
        body = fx.reader().layers[fx.layer_id]['comparison'].payload
        self.assertTrue(body['source_image']['model_disclosure_authorized'])
        self.assertEqual(body['source_scope']['file_sha256'], body['source_image']['sha256'])
        self.assertEqual(body['historical_render_fidelity'], 'not_applicable')
        self.assertFalse(body['performs_semantic_assessment'])
        fx.selections[0]['disclosure_access'] = None
        self.assertFalse(fx.reader().layers[fx.layer_id]['comparison'].payload['source_image']['model_disclosure_authorized'])

    def test_historical_disclosure_refusal_precedes_every_source_or_context_read(self):
        fx = ImageComparisonFixture(self)
        fx.selections[0]['disclosure_access'] = fx.disclosure()
        with patch.object(OwnerLocalSourceContext, 'snapshot', side_effect=AssertionError('context read')), \
                patch.object(assessment, '_original', side_effect=AssertionError('original read')), \
                self.assertRaisesRegex(PermissionError, 'never authorizes'):
            fx.reader()

    def test_exact_disclosure_and_image_grant_mismatches_fail_before_io(self):
        fx = ImageComparisonFixture(self, synthetic=True, disclose=True)
        original = copy.deepcopy(fx.selections)
        for name, key, value in (('disclosure_access', 'allowed', False), ('disclosure_access', 'expires_at', '2000-01-01T00:00:00Z'),
                ('disclosure_access', 'image_sha256', 'f' * 64), ('disclosure_access', 'layer_record_sha256', 'e' * 64),
                ('disclosure_access', 'basis', 'public_domain_reviewed'), ('image_access', 'access_allowed', False),
                ('image_access', 'expires_at', '2000-01-01T00:00:00Z')):
            fx.selections = copy.deepcopy(original)
            fx.selections[0][name][key] = value
            with self.subTest(key=key), patch.object(OwnerLocalSourceContext, 'snapshot', side_effect=AssertionError('context read')), self.assertRaises(PermissionError):
                fx.reader()

    def test_metadata_only_reads_neither_pdf_png_nor_ocr(self):
        fx = ImageComparisonFixture(self)
        fx.selections[0].update(payload_access=None, image_access=None, disclosure_access=None)
        fx.selections[0]['source_access']['read_scope'] = 'metadata_only'
        fx.image.unlink()
        fx.seed.pdf_path.unlink()
        (fx.seed.path.parent / 'content.txt').unlink()
        with patch.object(assessment, '_original', side_effect=AssertionError('original read')):
            reader = fx.reader()
        self.assertFalse(reader.layers[fx.layer_id]['read_ready'])
        self.assertIsNone(reader.layers[fx.layer_id]['comparison'])

    def test_current_image_identity_and_grant_drift_invalidates_comparison(self):
        fx = ImageComparisonFixture(self)
        reader = fx.reader()
        fx.image.write_bytes(fx.raw + b' ')
        with self.assertRaises(ValueError):
            reader.snapshot()
        fx.selections[0]['image_access']['expires_at'] = '2000-01-01T00:00:00Z'
        with patch.object(assessment, '_original', side_effect=AssertionError('original read')), self.assertRaises(PermissionError):
            reader.snapshot()

    def test_old_epub_profile_refuses_new_image_fields(self):
        fx = ImageComparisonFixture(self, synthetic=True)
        with self.assertRaises(ValueError):
            assessment.base.preflight_layer_selections(fx.selections, fx.subjects)

    def test_v6_journal_returns_exact_image_comparison_without_assessment_authority(self):
        import assessment_journal
        from test_occurrence_growth import copy_contracts
        fx = ImageComparisonFixture(self, synthetic=True, disclose=True)
        copy_contracts(fx.seed.public)
        policy = Record.from_payload('tos.policy.knowledge-assessment', 3,
            json.loads((ROOT / 'ToS/doctrine/semantic-interchange/assessment-policy.v3.json').read_bytes()))
        journal = fx.seed.store / '.image-review-journal'
        journal.mkdir(mode=0o700)
        config = {'schema_version': 'tos_local_assessment_owner_v6', 'uid': os.getuid(), 'principal_id': 'synthetic:read-only-comparison',
            'execution_profile': None, 'policy': {'id': policy.id, 'version': policy.version, 'payload': policy.payload, 'origin_id': policy.origin_id},
            'authorities': [], 'competencies': [], 'records': [], 'subjects': fx.subjects, 'journal_directory': str(journal),
            'source_context_ref': str(fx.seed.seed.context_path), 'source_records': [], 'owner_local_source_records': [],
            'native_text_units': [], 'native_text_layers': fx.selections, 'quality_dependencies': {}}
        owner = fx.seed.base / 'image-owner.json'
        owner.write_bytes(encoded(config))
        owner.chmod(0o600)
        with patch.object(native_owner_ocr, 'verify_owner_ocr', return_value={}):
            described = assessment_journal.run_local_command(owner, {'schema_version': 'tos_local_assessment_command_v1',
                'operation': 'describe', 'subject_id': fx.layer_id})
            result = assessment_journal.run_local_command(owner, {'schema_version': 'tos_local_assessment_command_v1',
                'operation': 'read-layer-comparison', 'subject_id': fx.layer_id, 'expected_subject': fx.subjects[fx.layer_id]['record'],
                'expected_snapshot': described['owner_snapshot']})
        self.assertTrue(result['result']['source_comparison']['payload']['source_image']['model_disclosure_authorized'])
        self.assertFalse(result['result']['current_admission']['can_use'])
        self.assertTrue(all(value in result['result']['current_admission']['limits'] for value in assessment.SYNTHETIC_LIMITS))
        from jsonschema import Draft202012Validator
        validator = Draft202012Validator(json.loads((ROOT / 'ToS/contracts/native-text-layer-quality-basis.schema.json').read_bytes()))
        basis = assessment_journal._quality_basis(fx.reader().layers[fx.layer_id],
            result['result']['current_admission'], 'text-layer:citation', validator)
        self.assertTrue(all(value in basis.payload['limits'] for value in assessment.SYNTHETIC_LIMITS))
        self.assertIsNone(result['result']['revision'])
        with patch.object(OwnerLocalSourceContext, 'load', side_effect=AssertionError('private context read')), self.assertRaises(PermissionError):
            assessment_journal.run_public_source_command(owner, {'schema_version': 'tos_local_assessment_command_v1',
                'operation': 'describe', 'subject_id': fx.layer_id})


if __name__ == '__main__':
    unittest.main()

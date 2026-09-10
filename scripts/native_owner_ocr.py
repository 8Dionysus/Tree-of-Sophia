"""Verify one pinned abyss-stack OCR receipt; this bridge never executes OCR."""
from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
import re
import subprocess

from source_owner_context import _absolute, _read


OWNER_OCR_CONFIG = 'tos_local_text_layer_record_owner_ocr_v1'
OWNER_OCR_OPERATION = 'text-layer.record-owner-ocr'
OWNER_PAGE_OCR_CONFIG = 'tos_local_text_layer_record_owner_page_ocr_v1'
OWNER_PAGE_OCR_OPERATION = 'text-layer.record-owner-page-ocr'
OWNER_OCR_PROFILES = {OWNER_OCR_OPERATION: OWNER_OCR_CONFIG, OWNER_PAGE_OCR_OPERATION: OWNER_PAGE_OCR_CONFIG}
ADAPTER = 'mechanics/inference-pilots/parts/tos-foundation-lab/bounded_tesseract_ocr.py'
PAGE_ADAPTER = 'mechanics/inference-pilots/parts/tos-foundation-lab/retained_page_tesseract_ocr.py'
EVIDENCE = {'owner-ocr-receipt.json': 'receipt.json',
    'owner-ocr-signature.sigstore.json': 'signature.sigstore.json', 'owner-ocr-signer.pub': 'signer.pub'}
MATERIAL_FIELDS = {'authority_ref', 'expires_at', 'access_allowed', 'receipt_root', 'receipt_sha256',
    'signature_sha256', 'public_key_sha256', 'owner_source_root', 'owner_source_ref', 'adapter_sha256',
    'content_sha256', 'byte_size'}


def validate_material(material, *, retained_page=False):
    if (type(material) is not dict or set(material) != MATERIAL_FIELDS | ({'input_representation'} if retained_page else set()) or material['access_allowed'] is not True
            or type(material['byte_size']) is not int or not 1 <= material['byte_size'] <= 131072
            or not isinstance(material['owner_source_ref'], str) or not re.fullmatch(r'commit:[a-f0-9]{40}', material['owner_source_ref'])):
        raise ValueError('owner OCR needs its separate exact recording grant')
    for key in ('receipt_sha256', 'signature_sha256', 'public_key_sha256', 'adapter_sha256', 'content_sha256'):
        if not isinstance(material[key], str) or not re.fullmatch('[a-f0-9]{64}', material[key]):
            raise ValueError('owner OCR requires independent exact evidence digests')
    for key in ('receipt_root', 'owner_source_root'):
        _absolute(material[key])
    if retained_page:
        validate_page_binding(material['input_representation'])


def validate_page_binding(binding, *, source_scope=None):
    fields = {'schema_version', 'source_file_ref', 'source_file_sha256', 'page_number', 'page_index_origin',
        'render_id', 'sample_id', 'render_manifest_sha256', 'render_receipt_sha256', 'sample_plan_sha256',
        'input_file_ref', 'input_sha256', 'input_bytes', 'media_type', 'width_pixels', 'height_pixels',
        'renderer', 'renderer_version', 'resolution_dpi', 'render_execution', 'historical_receipt_signature'}
    if (type(binding) is not dict or set(binding) != fields or binding['schema_version'] != 'tos_retained_pdf_page_input_binding_v1'
            or type(binding['page_number']) is not int or not 1 <= binding['page_number'] <= 10000 or binding['page_index_origin'] != 1
            or binding['source_file_ref'] != 'tos.file.sha256.' + str(binding['source_file_sha256'])
            or binding['input_file_ref'] != 'tos.file.sha256.' + str(binding['input_sha256'])
            or binding['media_type'] != 'image/png' or binding['renderer'] != 'poppler-pdftoppm'
            or binding['renderer_version'] != '26.01.0' or binding['resolution_dpi'] != 300
            or binding['render_execution'] != 'retained-not-observed-this-run' or binding['historical_receipt_signature'] != 'absent'
            or type(binding['input_bytes']) is not int or not 1 <= binding['input_bytes'] <= 10 * 1024 * 1024
            or any(type(binding[key]) is not int or binding[key] < 1 for key in ('width_pixels', 'height_pixels'))
            or binding['width_pixels'] * binding['height_pixels'] > 12000000):
        raise ValueError('owner page OCR needs a distinct exact original-PDF/retained-PNG binding')
    for key in ('source_file_sha256', 'input_sha256', 'render_manifest_sha256', 'render_receipt_sha256', 'sample_plan_sha256'):
        if not isinstance(binding[key], str) or not re.fullmatch('[a-f0-9]{64}', binding[key]):
            raise ValueError('owner page OCR requires every original and retained input digest')
    if source_scope is not None and (binding['source_file_ref'] != source_scope['file_ref'] or binding['source_file_sha256'] != source_scope['file_sha256']):
        raise ValueError('owner page OCR representation addresses another original File')


def validate_page_anchor(anchor, binding, source_scope):
    validate_page_binding(binding, source_scope=source_scope)
    try:
        expression = anchor['selector_payload']['expression']
        envelope = expression['selector']
        selector, state = envelope['selector'], envelope['state']
        if (anchor['target']['file_id'] != source_scope['file_ref'] or anchor['target']['file_sha256'] != source_scope['file_sha256']
                or anchor['target']['item_id'] != source_scope['item_ref'] or anchor['target']['media_type'] != 'application/pdf'
                or anchor['selector_payload']['kind'] != 'selector_expression' or expression['mode'] != 'single'
                or state['state_type'] != 'digest_state' or state['representation_sha256'] != source_scope['file_sha256']
                or state['media_type'] != 'application/pdf' or selector['type'] != 'page_region'
                or selector['page_identity'] != {'page_number': binding['page_number']}
                or selector['coordinate_space'] != 'normalized_0_1'
                or [selector[key] for key in ('x', 'y', 'width', 'height')] != [0, 0, 1, 1]):
            raise ValueError('owner page OCR source return must name the exact whole original PDF page')
    except (KeyError, TypeError) as error:
        raise ValueError('owner page OCR source anchor is outside its exact page profile') from error


def _adapter(material, *, retained_page=False):
    root = _absolute(material['owner_source_root'])
    path = root / (PAGE_ADAPTER if retained_page else ADAPTER)
    if hashlib.sha256(_read(path, 128 * 1024)).hexdigest() != material['adapter_sha256']:
        raise ValueError('owner OCR adapter differs from the protected source pin')
    for args, expected in ((['rev-parse', 'HEAD'], material['owner_source_ref'][7:]),
                           (['status', '--porcelain', '--untracked-files=normal'], '')):
        proc = subprocess.run(['/usr/bin/git', *args], cwd=root, capture_output=True, text=True, timeout=10)
        if proc.returncode or proc.stdout.strip() != expected:
            raise ValueError('owner OCR verifier source is not its clean exact commit')
    return path


def verify_owner_ocr(material, *, source_scope, language, metadata_paths=None, retained_page=False):
    """metadata_paths selects copied metadata only; no output/source-text read."""
    validate_material(material, retained_page=retained_page)
    if retained_page:
        validate_page_binding(material['input_representation'], source_scope=source_scope)
    adapter = _adapter(material, retained_page=retained_page)
    argv = ['/usr/bin/python3', str(adapter)]
    if metadata_paths is None:
        argv += ['--emit-content', 'verify', '--receipt-root', material['receipt_root']]
    else:
        argv += ['verify-record', '--receipt', str(metadata_paths['owner-ocr-receipt.json']),
            '--signature', str(metadata_paths['owner-ocr-signature.sigstore.json']),
            '--public-key', str(metadata_paths['owner-ocr-signer.pub'])]
    argv += ['--receipt-sha256', material['receipt_sha256'], '--public-key-sha256', material['public_key_sha256'],
        '--owner-source-ref', material['owner_source_ref']]
    proc = subprocess.run(argv, shell=False, capture_output=True, timeout=30,
        env={'PATH': '/usr/bin', 'LC_ALL': 'C.UTF-8', 'PYTHONDONTWRITEBYTECODE': '1'})
    if proc.returncode or len(proc.stdout) > 512 * 1024:
        raise ValueError('exact owner OCR evidence verification failed')
    result = json.loads(proc.stdout)
    receipt = result.get('receipt', {})
    if (result.get('ok') is not True or result.get('receipt_sha256') != material['receipt_sha256']
            or result.get('signature_sha256') != material['signature_sha256']
            or result.get('public_key_sha256') != material['public_key_sha256']
            or receipt.get('owner', {}).get('source_ref') != material['owner_source_ref']
            or receipt.get('owner', {}).get('adapter_sha256') != material['adapter_sha256']
            or receipt.get('source_scope') != source_scope
            or receipt.get('language') != {'de': 'deu', 'ru': 'rus'}.get(language, language)
            or receipt.get('output_sha256') != material['content_sha256'] or receipt.get('output_bytes') != material['byte_size']):
        raise ValueError('authenticated OCR evidence does not match the exact native source or output')
    if retained_page and (receipt.get('schema_version') != 'tos_retained_pdf_page_ocr_execution_v1'
            or receipt.get('input_representation') != material['input_representation']
            or receipt.get('input_verification', {}).get('render_execution') != 'not_performed'
            or receipt.get('input_verification', {}).get('historical_receipt_signature') != 'absent'):
        raise ValueError('authenticated page OCR differs from the independent page/representation or capture posture')
    if metadata_paths is None:
        content = base64.b64decode(result['content_base64'], validate=True)
        if hashlib.sha256(content).hexdigest() != material['content_sha256'] or len(content) != material['byte_size']:
            raise ValueError('owner OCR output differs from its independent recording grant')
        content.decode('utf-8')
        return content
    if 'content_base64' in result:
        raise ValueError('metadata-only owner verifier unexpectedly disclosed text')
    return receipt


def evidence_bytes(material):
    root = _absolute(material['receipt_root'])
    files = {name: _read(root / original, 128 * 1024, confidential_file=True) for name, original in EVIDENCE.items()}
    for name, key in (('owner-ocr-receipt.json', 'receipt_sha256'),
                      ('owner-ocr-signature.sigstore.json', 'signature_sha256'), ('owner-ocr-signer.pub', 'public_key_sha256')):
        if hashlib.sha256(files[name]).hexdigest() != material[key]:
            raise ValueError('owner OCR metadata changed while copying exact evidence')
    return files

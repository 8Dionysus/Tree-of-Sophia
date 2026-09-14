"""Exact retained-page image/OCR comparison; no rendering or quality verdict.

This separate confidential v6 source adapter does not broaden the EPUB-only v5
comparison. It makes an independently granted original PDF, retained PNG and
authenticated OCR available for actual source-visible review. Pixel fixity and
an available image locator do not perform that review or grant model disclosure.
"""
from __future__ import annotations

import copy
from datetime import datetime, timezone
from pathlib import Path
import time

from assessment_journal import JournalConflict
from knowledge_assessment import Record, _canonical
from native_text_binding import NativeTextBindingResolver, check_local_research_rights
from native_owner_ocr import (OWNER_PAGE_OCR_CONFIG, OWNER_PAGE_OCR_OPERATION, OWNER_OCR_CONFIG,
    OWNER_OCR_OPERATION, validate_page_anchor, validate_page_binding)
from source_owner_context import _absolute
from source_text_layer_proposal import derivation_policy
import native_text_layer_assessment as base
import source_text_layer_commands as construction


PROFILE = 'tos_retained_page_ocr_image_comparison_v1'
SYNTHETIC_PROFILE = 'tos_operator_synthetic_png_ocr_image_comparison_v1'
COMPARISON_SCHEMA = 'native-page-ocr-comparison.schema.json'
MAX_PDF_BYTES = 128 * 1024 * 1024
MAX_IMAGE_BYTES = 10 * 1024 * 1024
COMPARISON_LIMITS = (
    'Quality review is limited to this exact OCR layer and retained page image; no full-document or translation-source admission.',
    'Historical PDF-page rendering remains unsigned retained provenance; current fixity is not a new render or independent page-fidelity attestation.',
    'Local source comparison grants no model/server disclosure, publication, diplomatic fidelity or canon authority.')
SYNTHETIC_LIMITS = (
    'Comparison concerns one operator-declared synthetic PNG and its exact authenticated OCR, not a historical witness.',
    'The protected issuer declares synthetic-source ownership; software checks exact bytes and grant scope, not authorship.',
    'An exact current source-disclosure grant permits only this image and OCR in the current assistant session; no publication or canon authority.')


def preflight_page_selections(selections, subjects):
    base._preflight_layer_selections(selections, subjects, original_limit=MAX_PDF_BYTES,
        maximum_layers=1, extra_fields={'comparison_profile', 'image_access', 'disclosure_access'})
    now = datetime.now(timezone.utc)
    for selection in selections:
        profile = selection['comparison_profile']
        if profile not in {PROFILE, SYNTHETIC_PROFILE}:
            base._fail('image comparison needs its separately selected supported profile')
        image = selection['image_access']
        disclosure = selection['disclosure_access']
        if profile == PROFILE and disclosure is not None:
            raise PermissionError('historical retained-page profile never authorizes model/server disclosure, including public-domain sources')
        if selection['source_access']['read_scope'] == 'metadata_only':
            if image is not None or disclosure is not None:
                raise PermissionError('metadata-only retained page comparison cannot grant image reads')
            continue
        base._keys(image, {'read_scope', 'access_allowed', 'authority_ref', 'expires_at', 'path', 'byte_size', 'sha256',
            'page_number', 'source_file_ref', 'source_file_sha256', 'processing_boundary', 'width_pixels', 'height_pixels'})
        try:
            expires = datetime.fromisoformat(image['expires_at'].replace('Z', '+00:00'))
        except (TypeError, ValueError, AttributeError):
            raise PermissionError('retained image grant has an invalid expiry') from None
        if (image['read_scope'] != 'exact_retained_page' or image['access_allowed'] is not True
                or not base._string(image['authority_ref']) or expires.tzinfo is None or expires <= now
                or image['processing_boundary'] != 'local_only' or type(image['byte_size']) is not int
                or not 1 <= image['byte_size'] <= MAX_IMAGE_BYTES or not base._sha(image['sha256'])
                or type(image['page_number']) is not int or not 1 <= image['page_number'] <= 10000
                or any(type(image[k]) is not int or image[k] < 1 for k in ('width_pixels', 'height_pixels'))
                or image['width_pixels'] * image['height_pixels'] > 12000000
                or not base._sha(image['source_file_sha256']) or image['source_file_ref'] != 'tos.file.sha256.' + image['source_file_sha256']):
            raise PermissionError('exact retained page image reading is absent, expired or outside its bounded grant')
        _absolute(image['path'])
        if profile == SYNTHETIC_PROFILE:
            if image['page_number'] != 1 or selection['payload_access']['byte_size'] > MAX_IMAGE_BYTES:
                raise PermissionError('synthetic comparison covers one whole bounded PNG only')
            if disclosure is not None:
                base._keys(disclosure, {'allowed', 'authority_ref', 'expires_at', 'read_scope', 'basis', 'processing_boundary',
                    'source_file_ref', 'source_file_sha256', 'image_sha256', 'layer_record_sha256'})
                try:
                    expires = datetime.fromisoformat(disclosure['expires_at'].replace('Z', '+00:00'))
                except (TypeError, ValueError, AttributeError):
                    raise PermissionError('synthetic disclosure grant has an invalid expiry') from None
                if (disclosure['allowed'] is not True or not base._string(disclosure['authority_ref'])
                        or expires.tzinfo is None or expires <= now or disclosure['basis'] != 'operator_created_synthetic_source'
                        or disclosure['read_scope'] != 'exact_source_image_and_ocr'
                        or disclosure['processing_boundary'] != 'current_assistant_session'
                        or disclosure['source_file_ref'] != image['source_file_ref']
                        or disclosure['source_file_sha256'] != image['source_file_sha256']
                        or disclosure['image_sha256'] != image['sha256']
                        or disclosure['layer_record_sha256'] != selection['binding']['text_layer']['record_sha256']):
                    raise PermissionError('synthetic disclosure requires its own current exact operator-issued grant')


def _original(config, entry, deadline):
    _, identity = construction._payload(config, entry, deadline=deadline, include_member=False)
    return identity


class NativePageOCRAssessmentSources(base.NativeLayerAssessmentSources):
    """One source-visible image comparison under distinct current grants."""

    def __init__(self, context, selections, subjects):
        self._images = []
        super().__init__(context, selections, subjects)

    @staticmethod
    def _preflight(selections, subjects):
        preflight_page_selections(selections, subjects)

    def _payload_deadline(self, deadline):
        deadline = super()._payload_deadline(deadline)
        now, monotonic = datetime.now(timezone.utc), time.monotonic()
        for selection in self._selections:
            for key in ('image_access', 'disclosure_access'):
                access = selection[key]
                if access is not None:
                    expires = datetime.fromisoformat(access['expires_at'].replace('Z', '+00:00'))
                    deadline = min(deadline, monotonic + (expires - now).total_seconds())
        return deadline

    def _metadata(self, selection):
        resolver = NativeTextBindingResolver(self.context.public_root, owner_context=self.context,
            read_bytes=self._read, max_content_bytes=128 * 1024)
        binding = selection['binding']
        retained = selection['comparison_profile'] == PROFILE
        schema = OWNER_PAGE_OCR_CONFIG if retained else OWNER_OCR_CONFIG
        operation = OWNER_PAGE_OCR_OPERATION if retained else OWNER_OCR_OPERATION
        resolver.resolve_layer(binding, verify_content=False)
        layer = resolver._record(binding['text_layer']['record_ref'], expected=binding['text_layer']['record_sha256'])
        derivation, rep = layer['derivation'], layer['representation']
        maker = derivation['maker']
        if (derivation['method'] != 'ocr' or derivation['input_layers'] or derivation['change_payload'] != {'kind': 'none'}
                or layer['layer_role'] != 'raw_ocr' or rep['character_normalization'] != 'none'
                or rep['content_visibility'] != 'local_only' or rep['publication_authorized'] is not False
                or rep['text_scope']['start'] != 0 or len(layer['source_binding']['anchors']) != 1
                or maker['maker_type'] != 'software'
                or maker['configuration_ref'] != (Path(binding['text_layer']['record_ref']).parent / construction.CONFIG_FILE).as_posix()):
            base._fail('image comparison supports only the distinct authenticated retained-page raw OCR layer')
        configuration = resolver._record(maker['configuration_ref'], expected=maker['configuration_digest'])
        policy = resolver._record(layer['editorial_policy']['policy_ref'], expected=layer['editorial_policy']['policy_sha256'])
        scope = {key: layer['source_binding'][key] for key in ('work_ref', 'expression_ref', 'edition_ref', 'item_ref')}
        scope.update(file_ref=layer['source_binding']['source_file_ref'], file_sha256=layer['source_binding']['source_file_sha256'])
        if (configuration['schema_version'] != schema or configuration['allowed_operations'] != [operation]
                or configuration['input']['kind'] != ('retained_pdf_page' if retained else 'acquired_file') or configuration['source_scope'] != scope
                or configuration['source_path'] != binding['text_layer']['record_ref'] or configuration['source_record_refs'] != binding['source_record_refs']
                or configuration['policy'] != policy or policy != derivation_policy(operation)
                or configuration['language'] != rep['language'] or configuration['derivation_access']['rights_record_refs'] != rep['rights_record_refs']):
            base._fail('image comparison needs the unchanged exact retained-page OCR source configuration')
        selected = self._input(configuration, selection)
        if retained:
            validate_page_binding(selected, source_scope=scope)
        item = resolver._record(binding['source_record_refs']['item'])
        manifest = resolver._record(item['item_manifest_ref'], expected=configuration['manifest_sha256'])
        entries = [row for row in manifest['payload_files'] if row['file_id'] == scope['file_ref']]
        if len(entries) != 1:
            base._fail('image comparison original PDF File must resolve uniquely')
        entry = entries[0]
        if (manifest['visibility'] != 'local_only' or entry['media_type'] != ('application/pdf' if retained else 'image/png')
                or len(Path(entry['relative_path']).parts) != 2 or Path(entry['relative_path']).parts[0] != 'payload'
                or not construction._member_path(entry['relative_path']) or entry['sha256'] != scope['file_sha256']):
            base._fail('image comparison source is outside the separately granted acquired PDF profile')
        for kind, ref in binding['source_record_refs'].items():
            resolver._read(ref, expected=configuration['source_record_sha256'][kind])
        anchor_ref = layer['source_binding']['anchors'][0]
        anchor = resolver._record(anchor_ref['anchor_record_ref'], expected=anchor_ref['anchor_record_sha256'])
        if retained:
            validate_page_anchor(anchor, selected, scope)
        else:
            self._synthetic_anchor(anchor, selected, scope)
        grant, image = selection['payload_access'], selection['image_access']
        if grant is not None:
            if (entry['byte_size'] != grant['byte_size'] or image['byte_size'] != selected['input_bytes']
                    or image['sha256'] != selected['input_sha256'] or image['page_number'] != selected['page_number']
                    or image['width_pixels'] != selected['width_pixels'] or image['height_pixels'] != selected['height_pixels']
                    or image['source_file_ref'] != scope['file_ref'] or image['source_file_sha256'] != scope['file_sha256']):
                raise PermissionError('current original-PDF or retained-image reading grant addresses another exact source')
        record = Record.from_payload(layer['layer_id'], layer['layer_version'], layer, origin_id=selection['origin_id'])
        target = self._subjects[layer['layer_id']]
        if target['record'] != record.ref or target['languages'] != [rep['language']] or target['maker_id'] != maker['agent_ref']:
            raise PermissionError('image comparison subject differs from the exact raw OCR record, language or maker')
        resolver._validate_schema_resource(COMPARISON_SCHEMA)
        self._resolvers.append(resolver)
        return selection, resolver, layer, configuration, entry

    @staticmethod
    def _input(configuration, selection):
        if selection['comparison_profile'] == PROFILE:
            return configuration['material']['input_representation']
        # The independently authenticated owner's receipt binds this source PNG;
        # dimensions are checked against actual PNG bytes before comparison.
        scope, grant = configuration['source_scope'], selection['image_access']
        return {'schema_version': 'tos_operator_synthetic_png_input_binding_v1',
            'source_file_ref': scope['file_ref'], 'source_file_sha256': scope['file_sha256'],
            'input_file_ref': scope['file_ref'], 'input_sha256': scope['file_sha256'],
            'input_bytes': configuration['source_access']['byte_size'], 'media_type': 'image/png',
            'page_number': 1, 'page_index_origin': 1,
            'width_pixels': grant['width_pixels'] if grant is not None else None,
            'height_pixels': grant['height_pixels'] if grant is not None else None}

    @staticmethod
    def _synthetic_anchor(anchor, selected, scope):
        try:
            envelope = anchor['selector_payload']['expression']['selector']
            region, state = envelope['selector'], envelope['state']
            if (anchor['target']['file_id'] != scope['file_ref'] or anchor['target']['file_sha256'] != scope['file_sha256']
                    or anchor['target']['item_id'] != scope['item_ref'] or anchor['target']['media_type'] != 'image/png'
                    or state['representation_sha256'] != scope['file_sha256'] or state['media_type'] != 'image/png'
                    or region['type'] != 'page_region' or region['page_identity'] != {'page_number': 1}
                    or region['coordinate_space'] != 'pixels' or region['x'] != 0 or region['y'] != 0
                    or region['width'] != region['source_width'] or region['height'] != region['source_height']
                    or (selected['width_pixels'] is not None and (region['width'] != selected['width_pixels']
                        or region['height'] != selected['height_pixels']))):
                base._fail('synthetic image comparison needs the exact whole source PNG anchor')
        except (KeyError, TypeError):
            base._fail('synthetic image comparison source anchor is outside its exact whole-PNG profile')

    def _image(self, access, binding):
        raw = self._read(_absolute(access['path']), access['byte_size'])
        if (len(raw) != access['byte_size'] or base._hash(raw) != access['sha256'] or len(raw) < 33
                or raw[:8] != b'\x89PNG\r\n\x1a\n' or raw[12:16] != b'IHDR'
                or int.from_bytes(raw[16:20], 'big') != binding['width_pixels']
                or int.from_bytes(raw[20:24], 'big') != binding['height_pixels'] or raw[24:26] != b'\x08\x02'):
            raise JournalConflict('exact retained source image bytes or dimensions changed')
        return raw

    def _materialize(self, selection, resolver, layer, configuration, entry, deadline):
        record = Record.from_payload(layer['layer_id'], layer['layer_version'], layer, origin_id=selection['origin_id'])
        rep, comparison = layer['representation'], None
        if selection['payload_access'] is not None:
            self._check_selections()
            check_local_research_rights(resolver, layer)
            current = {key: copy.deepcopy(configuration[key]) for key in ('source_record_refs', 'source_scope')}
            current['source_access'] = copy.deepcopy(selection['payload_access'])
            identity = _original(current, entry, self._payload_deadline(deadline))
            selected = self._input(configuration, selection)
            self._image(selection['image_access'], selected)
            resolver.resolve_layer(selection['binding'], verify_content=True, allow_private_content=True)
            actual = resolver._read(rep['content_ref'], expected=rep['content_sha256'], content=True).decode('utf-8')
            if rep['text_scope']['end'] != len(actual):
                base._fail('image comparison requires the whole exact raw OCR representation')
            source_ref = (Path(configuration['source_record_refs']['item']).parent / entry['relative_path']).as_posix()
            disclosed = selection['disclosure_access'] is not None
            limits = COMPARISON_LIMITS if selection['comparison_profile'] == PROFILE else SYNTHETIC_LIMITS
            locator = {'path': selection['image_access']['path'], 'media_type': 'image/png', 'sha256': selected['input_sha256'],
                'byte_size': selected['input_bytes'], 'width_pixels': selected['width_pixels'], 'height_pixels': selected['height_pixels'],
                'page_number': selected['page_number'], 'page_index_origin': 1,
                'processing_boundary': 'current_assistant_session' if disclosed else 'local_only', 'model_disclosure_authorized': disclosed}
            fixity = [{'ref': ref, 'category': category, 'sha256': digest} for (ref, category), digest in sorted(resolver._inputs.items())]
            fixity += [{'ref': source_ref, 'category': 'original_payload', 'sha256': configuration['source_scope']['file_sha256']},
                {'ref': locator['path'], 'category': 'retained_image', 'sha256': selected['input_sha256']}]
            receipt = resolver._record((Path(selection['binding']['text_layer']['record_ref']).parent / 'owner-ocr-receipt.json').as_posix(),
                expected=configuration['material']['receipt_sha256'])
            body = {'schema_version': 'tos_native_page_ocr_comparison_v1', 'comparison_version': 1, 'comparison_profile': selection['comparison_profile'],
                'layer': record.ref, 'source_scope': copy.deepcopy(configuration['source_scope']), 'source_anchor': copy.deepcopy(layer['source_binding']['anchors'][0]),
                'input_representation': copy.deepcopy(selected), 'source_image': locator, 'representation_text': actual,
                'representation': copy.deepcopy(rep), 'editorial_policy': copy.deepcopy(configuration['policy']), 'maker': copy.deepcopy(layer['derivation']['maker']),
                'configuration_sha256': layer['derivation']['maker']['configuration_digest'], 'input_fixity': fixity,
                'owner_execution': {'receipt_sha256': configuration['material']['receipt_sha256'], 'owner': copy.deepcopy(receipt['owner']),
                    'input_verification': copy.deepcopy(receipt.get('input_verification'))},
                'disclosure_access': copy.deepcopy(selection['disclosure_access']),
                'deterministic_text_match': None, 'source_visible_judgment': 'not_performed',
                'historical_render_fidelity': 'not_independently_assessed' if selection['comparison_profile'] == PROFILE else 'not_applicable',
                'limits': list(limits), 'visibility': 'local_only', 'publication_authorized': False, 'performs_semantic_assessment': False}
            body['comparison_id'] = 'tos.text-comparison.sha256.' + base._hash(_canonical(body))
            resolver._validate(body, COMPARISON_SCHEMA)
            comparison = Record.from_payload(body['comparison_id'], 1, body, origin_id=selection['origin_id'])
            self._payloads.append((current, copy.deepcopy(entry), identity, configuration['source_scope']['file_sha256']))
            self._images.append((copy.deepcopy(selection['image_access']), copy.deepcopy(selected)))
        self.records.append({'id': record.id, 'version': record.version, 'payload': record.payload, 'origin_id': record.origin_id})
        if comparison is not None:
            self.records.append({'id': comparison.id, 'version': comparison.version, 'payload': comparison.payload, 'origin_id': comparison.origin_id})
        self.layers[record.id] = {'record': record, 'comparison': comparison, 'binding': copy.deepcopy(selection['binding']),
            'language': rep['language'], 'maker_id': layer['derivation']['maker']['agent_ref'],
            'scope': {key: copy.deepcopy(rep[key]) for key in ('content_file_id', 'content_sha256', 'text_scope')},
            'read_ready': comparison is not None, 'comparison_limits': list(COMPARISON_LIMITS if selection['comparison_profile'] == PROFILE else SYNTHETIC_LIMITS)}
        self.contracts.update(resolver.schema_digests)

    def snapshot(self):
        self._check_selections()
        if self.context.snapshot() != self._context_snapshot:
            raise JournalConflict('image comparison owner context changed')
        sources = [resolver.snapshot() for resolver in self._resolvers]
        deadline = time.monotonic() + base.MAX_SECONDS
        for config, entry, identity, _ in self._payloads:
            if _original(config, entry, self._payload_deadline(deadline)) != identity:
                raise JournalConflict('image comparison original PDF or its ancestors changed')
        for access, binding in self._images:
            self._image(access, binding)
        self._check_selections()
        if sources != [resolver.snapshot() for resolver in self._resolvers] or self.context.snapshot() != self._context_snapshot:
            raise JournalConflict('image comparison source changed while checking acquired inputs')
        return 'sha256:' + base._hash(_canonical({'selection': base._hash(self._selection_bytes), 'context': self._context_snapshot,
            'sources': sources, 'payloads': [{'identity': identity, 'source_sha256': digest} for _, _, identity, digest in self._payloads],
            'images': [{'sha256': binding['input_sha256'], 'path': access['path']} for access, binding in self._images]}))

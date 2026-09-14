"""Exact private derivation evidence, separate from a quality decision.

The first source-view profile is bounded EPUB/XHTML. Correction, normalization
and supplied text remain distinct, as do a reproducible delta and source
fidelity. An image/PDF needs its own source renderer; a File hash is not one.
Historical construction grants are inert evidence, never reading authority.
"""
from __future__ import annotations

import copy
import hashlib
from pathlib import Path
from time import monotonic

from knowledge_assessment import MAX_RECORD_BYTES, Record, _canonical
from native_text_binding import check_local_research_rights
from source_text_layer_proposal import (
    DEFAULT_POLICY, DERIVE_CONFIG, DERIVE_OPERATIONS, extract_xhtml_text,
    validate_extraction_profile,
)
import source_text_layer_commands as construction


COMPARISON_SCHEMA = 'native-text-layer-derivation-comparison.schema.json'
MAX_LINEAGE = 16


def _fail(message):
    raise ValueError(message) from None


def _check_time(deadline):
    if monotonic() >= deadline:
        _fail('derived layer comparison exceeded its cooperative time budget')


class NativeDerivedLayerComparison:
    """Prepare metadata/rights before ANY selected comparison reads content."""

    def __init__(self, owner, selection, resolver, layer):
        self.owner, self.selection, self.resolver = owner, selection, resolver
        self.lineage = []
        binding = copy.deepcopy(selection['binding'])
        seen = set()
        while True:
            if len(self.lineage) >= MAX_LINEAGE:
                _fail('derived layer comparison exceeds its lineage budget')
            resolver.resolve_layer(binding, verify_content=False)
            target = binding['text_layer']
            current = resolver._record(target['record_ref'], expected=target['record_sha256'])
            if current['layer_id'] in seen:
                _fail('derived layer comparison repeats a lineage identity')
            seen.add(current['layer_id'])
            rep, maker = current['representation'], current['derivation']['maker']
            config_ref = (Path(target['record_ref']).parent / construction.CONFIG_FILE).as_posix()
            if (maker.get('configuration_ref') != config_ref
                    or rep['content_visibility'] != 'local_only'
                    or rep['publication_authorized'] is not False
                    or rep['text_scope']['start'] != 0):
                _fail('derived layer comparison requires exact private whole-layer configurations')
            config = resolver._record(config_ref, expected=maker['configuration_digest'])
            policy = resolver._record(current['editorial_policy']['policy_ref'],
                expected=current['editorial_policy']['policy_sha256'])
            scope = current['source_binding']
            source_scope = {key: scope[key] for key in ('work_ref', 'expression_ref', 'edition_ref', 'item_ref')}
            source_scope.update(file_ref=scope['source_file_ref'], file_sha256=scope['source_file_sha256'])
            if (config.get('source_path') != target['record_ref']
                    or config.get('source_record_refs') != binding['source_record_refs']
                    or config.get('source_scope') != source_scope
                    or config.get('language') != rep['language']
                    or config.get('policy') != policy
                    or config.get('maker') != {key: maker[key] for key in ('maker_type', 'agent_ref', 'method', 'version')}):
                _fail('derived layer comparison differs from retained source configuration')
            # Validate bindings, not the historical expiry/current write grant.
            for kind, ref in binding['source_record_refs'].items():
                resolver._read(ref, expected=config['source_record_sha256'][kind])
            self.lineage.append((binding, current, config, policy))
            schema = config.get('schema_version')
            if schema == construction.CONFIG:
                validate_extraction_profile(config['selector'], policy)
                if (current['derivation']['method'] != 'structural_extraction' or current['derivation']['input_layers']
                        or current['derivation']['change_payload'] != {'kind': 'none'}
                        or current['layer_role'] != 'machine_transcription' or maker['maker_type'] != 'software'
                        or rep['character_normalization'] != 'none'
                        or config['identities']['layer_id'] != current['layer_id']
                        or config['identities']['provenance_event_id'] != current['provenance_event_ref']
                        or config['derivation_access']['rights_record_refs'] != rep['rights_record_refs']):
                    _fail('derived layer origin is not the retained extraction profile')
                break
            if schema != DERIVE_CONFIG or config.get('allowed_operations', [None])[0] not in DERIVE_OPERATIONS:
                _fail('derived layer comparison needs a supported exact method profile')
            if config['allowed_operations'][0] in ('text-layer.record-transcription', 'text-layer.record-ocr'):
                break
            binding = copy.deepcopy(config['input']['binding'])
        if self.lineage[0][2].get('schema_version') != DERIVE_CONFIG:
            _fail('derived layer comparison cannot replace the extraction comparison')
        self.source_scope = self.lineage[0][2]['source_scope']
        if any(current['source_binding'] != layer['source_binding'] for _, current, _, _ in self.lineage):
            _fail('derived comparison lineage has another exact source scope')
        item = resolver._record(selection['binding']['source_record_refs']['item'])
        self.manifest = resolver._record(item['item_manifest_ref'], expected=self.lineage[0][2]['manifest_sha256'])
        entries = [entry for entry in self.manifest['payload_files'] if entry['file_id'] == self.source_scope['file_ref']]
        if len(entries) != 1:
            _fail('derived comparison source File is not unique')
        self.entry = entries[0]
        if (self.manifest['visibility'] != 'local_only'
                or self.entry['media_type'] != 'application/epub+zip'
                or self.entry['sha256'] != self.source_scope['file_sha256']
                or len(Path(self.entry['relative_path']).parts) != 2
                or Path(self.entry['relative_path']).parts[0] != 'payload'
                or not construction._member_path(self.entry['relative_path'])):
            _fail('derived comparison requires the bounded EPUB source view; image/PDF needs a source renderer')
        if selection['payload_access'] is not None and self.entry['byte_size'] != selection['payload_access']['byte_size']:
            raise PermissionError('derived comparison current source grant has another byte scope')
        anchors = layer['source_binding']['anchors']
        if len(anchors) != 1:
            _fail('derived comparison requires one exact source-view anchor')
        anchor = resolver._record(anchors[0]['anchor_record_ref'], expected=anchors[0]['anchor_record_sha256'])
        origin = self.lineage[-1]
        if origin[2]['schema_version'] == construction.CONFIG:
            owner._check_anchor(anchor, origin[2], origin[1]['derivation']['maker'], self.entry)
        self.payload_ref = (Path(selection['binding']['source_record_refs']['item']).parent / self.entry['relative_path']).as_posix()
        self.member, self.selector = self._source_view(anchor)
        self.source_policy = copy.deepcopy(DEFAULT_POLICY)
        validate_extraction_profile(self.selector, self.source_policy)
        resolver._validate_schema_resource(COMPARISON_SCHEMA)

    def _source_view(self, anchor):
        payload = anchor['selector_payload']
        try:
            steps = payload['expression']['steps']
            member_selector, selector = steps[0]['selector'], steps[1]['selector']
            member = {key: member_selector[key] for key in ('member_path', 'member_sha256')}
            expected = {'kind': 'selector_expression', 'expression': {'mode': 'refinement_chain', 'steps': [
                {'state': {'state_type': 'digest_state', 'representation_ref': self.payload_ref,
                    'representation_sha256': self.source_scope['file_sha256'], 'media_type': 'application/epub+zip'},
                 'selector': {'type': 'container_member', **member, 'member_media_type': 'application/xhtml+xml'}},
                {'state': {'state_type': 'digest_state', 'representation_ref': member['member_path'],
                    'representation_sha256': member['member_sha256'], 'media_type': 'application/xhtml+xml'},
                 'selector': selector}]}}
            if payload != expected or not construction._member_path(member['member_path']):
                _fail('derived comparison source anchor is outside its exact XHTML profile')
        except (KeyError, TypeError, IndexError):
            _fail('derived comparison needs a supported source renderer for its exact anchor')
        return copy.deepcopy(member), copy.deepcopy(selector)

    def materialize(self, deadline):
        owner, resolver, selection = self.owner, self.resolver, self.selection
        if selection['payload_access'] is None:
            return None, False
        _check_time(deadline)
        owner._check_selections()
        for _, layer, _, _ in self.lineage:
            check_local_research_rights(resolver, layer)
        config = {'source_record_refs': copy.deepcopy(selection['binding']['source_record_refs']),
            'source_scope': copy.deepcopy(self.source_scope), 'member': copy.deepcopy(self.member),
            'source_access': copy.deepcopy(selection['payload_access'])}
        # Same current-grant payload reader and snapshot identity as extraction.
        member, identity = construction._payload(config, self.entry, deadline=owner._payload_deadline(deadline))
        _check_time(deadline)
        if len(member) > MAX_RECORD_BYTES:
            _fail('derived comparison source member exceeds its full-read budget')
        source_text = extract_xhtml_text(member, selector=self.selector, policy=self.source_policy)
        _check_time(deadline)
        lineage = []
        for binding, layer, configuration, policy in reversed(self.lineage):
            owner._check_selections()
            _check_time(deadline)
            resolver.resolve_layer(binding, verify_content=True, allow_private_content=True)
            _check_time(deadline)
            rep = layer['representation']
            text = resolver._read(rep['content_ref'], expected=rep['content_sha256'], content=True).decode('utf-8')
            if rep['text_scope']['end'] != len(text):
                _fail('derived comparison requires the full exact lineage representation')
            lineage.append({'record': Record.from_payload(layer['layer_id'], layer['layer_version'], layer).ref,
                'record_ref': binding['text_layer']['record_ref'], 'record_payload': copy.deepcopy(layer),
                'representation_text': text, 'editorial_policy': copy.deepcopy(policy),
                'configuration_sha256': layer['derivation']['maker']['configuration_digest'],
                'reported_producer': copy.deepcopy(configuration.get('material', {}).get('reported_maker')),
                'operation': configuration.get('allowed_operations', ['text-layer.extract'])[0]})
            _check_time(deadline)
        fixity = [{'ref': ref, 'category': category, 'sha256': digest}
                  for (ref, category), digest in sorted(resolver._inputs.items())]
        fixity.extend([
            {'ref': self.payload_ref, 'category': 'original_payload', 'sha256': self.source_scope['file_sha256']},
            {'ref': self.payload_ref + '!/' + self.member['member_path'], 'category': 'source_member',
                'sha256': self.member['member_sha256']}])
        body = {'schema_version': 'tos_native_text_layer_derivation_comparison_v1',
            'comparison_version': 1, 'layer': lineage[-1]['record'], 'source_scope': copy.deepcopy(self.source_scope),
            'source_view': {'member': self.member, 'selector': self.selector, 'source_member_utf8': member.decode('utf-8'),
                'selected_text': source_text, 'policy': self.source_policy,
                'method': 'bounded-XHTML-character-data-not-textual-quality'},
            'lineage': lineage, 'input_fixity': fixity, 'transformation_integrity': True,
            'source_text_equals_output': source_text == lineage[-1]['representation_text'],
            'positive_use_boundary': 'source-visible-assessment-required-not-byte-equality',
            'provider_execution': 'not-attested-by-comparison', 'inherited_quality': 'not-transferred',
            'visibility': 'local_only', 'publication_authorized': False, 'performs_semantic_assessment': False}
        body['comparison_id'] = 'tos.text-comparison.sha256.' + hashlib.sha256(_canonical(body)).hexdigest()
        _check_time(deadline)
        if len(_canonical(body)) > MAX_RECORD_BYTES:
            _fail('derived comparison exceeds its aggregate record budget; truncation is forbidden')
        resolver._validate(body, COMPARISON_SCHEMA)
        _check_time(deadline)
        owner._payloads.append((config, copy.deepcopy(self.entry), identity, hashlib.sha256(member).hexdigest()))
        comparison = Record.from_payload(body['comparison_id'], 1, body, origin_id=selection['origin_id'])
        return comparison, True

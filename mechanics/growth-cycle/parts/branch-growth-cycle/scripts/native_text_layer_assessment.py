"""Confidential exact TextLayer comparison, without admission or source writes.

The issuer selects all access grants independently of source material. Historical
creation configuration is immutable DATA, never current reading authority. This
bounded adapter supports only the declared EPUB structural-extraction profile;
byte equality and an available comparison are not a textual-quality judgment.
"""
from __future__ import annotations

import copy
from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import re
import time

from assessment_journal import JournalConflict
from knowledge_assessment import MAX_RECORD_BYTES, Record, _canonical
from native_text_binding import NativeTextBindingResolver, check_local_research_rights
from source_owner_context import OwnerLocalSourceContext, _absolute, _open
from source_text_layer_proposal import extract_xhtml_text, validate_extraction_profile
import source_text_layer_commands as construction
import source_item_deposit as deposit


COMPARISON_SCHEMA = 'native-text-layer-comparison.schema.json'
MAX_LAYERS = 8
MAX_ORIGINAL_BYTES = 16 * 1024 * 1024
MAX_SOURCE_BYTES = 16 * 1024 * 1024
MAX_SOURCE_FILES = 128
MAX_SECONDS = 30
USES = frozenset({'text-layer:citation', 'text-layer:linguistic-analysis',
                  'text-layer:semantic-analysis', 'text-layer:search-projection'})


class NativeLayerAssessmentError(ValueError):
    """The selected source cannot supply this exact bounded comparison."""


def _fail(message):
    raise NativeLayerAssessmentError(message) from None


def _keys(value, fields):
    if type(value) is not dict or set(value) != set(fields):
        _fail('native layer assessment has an incompatible field set')


def _hash(raw):
    return hashlib.sha256(raw).hexdigest()


def _string(value):
    return type(value) is str and bool(value.strip()) and len(value) <= 4096


def _sha(value, *, prefixed=False):
    return type(value) is str and re.fullmatch(('sha256:' if prefixed else '') + '[a-f0-9]{64}', value) is not None


def _metadata_ref(value):
    if not _string(value) or '\\' in value or '\x00' in value:
        _fail('native layer assessment requires a bounded metadata locator')
    path = Path(value)
    if (path.as_posix() != value or not path.is_relative_to('ToS/source-witnesses')
            or path.suffix != '.json' or any(part in {'.', '..', 'payload', 'local-content', 'catalog'} for part in path.parts)):
        _fail('native layer assessment metadata locator leaves its owner route')


def preflight_layer_selections(selections, subjects):
    """Pure validation of ALL grants/scopes before any context or source I/O.

    Original byte_size is charged per exact selection, including repeated
    originals: this conservative preflight does not infer file identity from
    metadata it has not yet been authorized to read. No truncation is allowed.
    """
    if type(selections) is not list or len(selections) > MAX_LAYERS or type(subjects) is not dict:
        _fail('native layer selection exceeds its bounded scope')
    try:
        if len(_canonical({'selections': selections, 'subjects': subjects})) > MAX_RECORD_BYTES:
            _fail('native layer selection exceeds its metadata budget')
    except (TypeError, ValueError, UnicodeError, RecursionError):
        _fail('native layer selection is not bounded finite JSON')
    seen, original_bytes = set(), 0
    now = datetime.now(timezone.utc)
    for selection in selections:
        _keys(selection, {'binding', 'origin_id', 'source_access', 'payload_access'})
        binding = selection['binding']
        _keys(binding, {'schema_version', 'text_layer', 'source_record_refs'})
        target = binding['text_layer']
        _keys(target, {'record_ref', 'record_sha256', 'layer_id', 'layer_version'})
        _keys(binding['source_record_refs'], {'work', 'expression', 'edition', 'item'})
        if (binding['schema_version'] != 'tos_native_text_layer_binding_v1'
                or not _string(target['layer_id']) or re.fullmatch(r'tos\.text-layer\.[a-z0-9]+(?:[.-][a-z0-9]+)*', target['layer_id']) is None
                or type(target['layer_version']) is not int or target['layer_version'] < 1
                or not _sha(target['record_sha256']) or not _string(selection['origin_id'])
                or target['layer_id'] in seen):
            _fail('native layer binding is invalid or repeats a target')
        seen.add(target['layer_id'])
        _metadata_ref(target['record_ref'])
        for kind, ref in binding['source_record_refs'].items():
            _metadata_ref(ref)
            if Path(ref).name != kind + '.json':
                _fail('native layer source locator has another metadata kind')
        access = selection['source_access']
        _keys(access, {'read_scope', 'access_allowed', 'authority_ref'})
        if (access['read_scope'] not in ('metadata_only', 'exact_owner_local')
                or access['access_allowed'] is not True or not _string(access['authority_ref'])):
            raise PermissionError('native layer needs an independent current source-read scope')
        grant = selection['payload_access']
        if access['read_scope'] == 'metadata_only':
            if grant is not None:
                raise PermissionError('metadata-only layer selection cannot carry payload access')
        else:
            _keys(grant, {'read_scope', 'access_allowed', 'authority_ref', 'expires_at', 'payload_root', 'byte_size'})
            try:
                expires = datetime.fromisoformat(grant['expires_at'].replace('Z', '+00:00'))
            except (TypeError, ValueError, AttributeError):
                raise PermissionError('native layer payload grant has an invalid expiry') from None
            if (grant['read_scope'] != 'exact_acquired_file' or grant['access_allowed'] is not True
                    or not _string(grant['authority_ref']) or expires.tzinfo is None or expires <= now
                    or type(grant['payload_root']) is not str or type(grant['byte_size']) is not int
                    or not 1 <= grant['byte_size'] <= MAX_ORIGINAL_BYTES):
                raise PermissionError('native layer payload grant is absent, expired or over budget')
            _absolute(grant['payload_root'])  # Lexical validation only; no resolve/open.
            original_bytes += grant['byte_size']
            if original_bytes > MAX_ORIGINAL_BYTES:
                _fail('native layer original inputs exceed the aggregate byte budget')
        scope = subjects.get(target['layer_id'])
        _keys(scope, {'record', 'assertion_layer', 'risk', 'languages', 'maker_id', 'requested_use', 'access_allowed'})
        record = scope['record']
        _keys(record, {'id', 'version', 'digest'})
        if (record['id'] != target['layer_id'] or type(record['version']) is not int
                or record['version'] != target['layer_version'] or not _sha(record['digest'], prefixed=True)
                or scope['assertion_layer'] != 'textual_observation' or scope['risk'] not in ('low', 'moderate', 'high')
                or type(scope['requested_use']) is not str or scope['requested_use'] not in USES or scope['access_allowed'] is not True
                or type(scope['languages']) is not list or len(scope['languages']) != 1
                or not _string(scope['languages'][0]) or not _string(scope['maker_id'])):
            raise PermissionError('native layer assessment target is outside its declared purpose or scope')


def _identity(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_mode, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _read_payload(config, entry, deadline):
    try:
        return construction._payload(config, entry, deadline=deadline)
    except (OSError, ValueError):
        _fail('native layer acquired input is unsafe, unsupported or changed')


class NativeLayerAssessmentSources:
    """One current source comparison snapshot; all returns remain local-only."""

    def __init__(self, context, selections, subjects):
        preflight_layer_selections(selections, subjects)
        if not isinstance(context, OwnerLocalSourceContext):
            _fail('native layer assessment requires its independently selected owner context')
        self.context, self.records, self.layers, self.contracts = context, [], {}, {}
        self._provided = (selections, subjects)
        self._selection_bytes = _canonical({'selections': selections, 'subjects': subjects})
        self._selections, self._subjects = copy.deepcopy((selections, subjects))
        self._observed, self._total, self._resolvers, self._payloads = {}, 0, [], []
        self._context_snapshot = context.snapshot()
        for selection in self._selections:
            grant = selection['payload_access']
            if grant is not None:
                root = _absolute(grant['payload_root'])
                if any(root.is_relative_to(other) or other.is_relative_to(root)
                       for other in (context.public_root, context.private_root)):
                    raise PermissionError('native layer acquired payload needs a distinct third root')
        # Resolve every layer's metadata/rights before reading any representation
        # or original payload. A later bad rights binding cannot trigger an earlier
        # content read merely because its grant shape passed preflight.
        prepared = [self._metadata(selection) for selection in self._selections]
        deadline = time.monotonic() + MAX_SECONDS
        for selection, resolver, layer, configuration, entry in prepared:
            self._check_selections()
            for selected in self._resolvers:
                selected.snapshot()
            self._materialize(selection, resolver, layer, configuration, entry, deadline)
        self._snapshot = self.snapshot()

    def _check_selections(self):
        preflight_layer_selections(*self._provided)
        if _canonical({'selections': self._provided[0], 'subjects': self._provided[1]}) != self._selection_bytes:
            raise JournalConflict('native layer access selection changed during comparison')

    def _payload_deadline(self, deadline):
        """A bounded read cannot outlive any current exact input grant."""
        self._check_selections()
        now, monotonic = datetime.now(timezone.utc), time.monotonic()
        for selection in self._selections:
            grant = selection['payload_access']
            if grant is not None:
                expiry = datetime.fromisoformat(grant['expires_at'].replace('Z', '+00:00'))
                deadline = min(deadline, monotonic + (expiry - now).total_seconds())
        return deadline

    def _read(self, path, limit):
        path = Path(path)
        if path not in self._observed and len(self._observed) >= MAX_SOURCE_FILES:
            _fail('native layer source closure exceeds its file budget')
        limit = min(limit, MAX_SOURCE_BYTES - self._total) if path not in self._observed else limit
        private = self.context.private_root if path.is_relative_to(self.context.private_root) else None
        parents = deposit._pins(path)
        with os.fdopen(_open(path, private_root=private), 'rb') as stream:
            before = os.fstat(stream.fileno())
            raw = stream.read(limit + 1)
            after = os.fstat(stream.fileno())
        descriptor = _open(path, private_root=private)
        try:
            current = os.fstat(descriptor)
        finally:
            os.close(descriptor)
        if (len(raw) > limit or _identity(before) != _identity(after) or _identity(before) != _identity(current)
                or deposit._pins(path) != parents):
            raise JournalConflict('native layer source changed or exceeded its exact read budget')
        observed = (_hash(raw), _identity(before), parents)
        if path in self._observed:
            if self._observed[path] != observed:
                raise JournalConflict('native layer source bytes or file identity changed')
        else:
            self._observed[path] = observed
            self._total += len(raw)
        return raw

    def _metadata(self, selection):
        resolver = NativeTextBindingResolver(self.context.public_root, owner_context=self.context,
            read_bytes=self._read, max_content_bytes=MAX_RECORD_BYTES)
        binding = selection['binding']
        resolver.resolve_layer(binding, verify_content=False)
        layer = resolver._record(binding['text_layer']['record_ref'], expected=binding['text_layer']['record_sha256'])
        derivation, rep = layer['derivation'], layer['representation']
        if (derivation['method'] != 'structural_extraction' or derivation['input_layers']
                or derivation['change_payload'] != {'kind': 'none'} or layer['layer_role'] != 'machine_transcription'
                or rep['character_normalization'] != 'none' or rep['content_visibility'] != 'local_only'
                or rep['publication_authorized'] is not False or rep['text_scope']['start'] != 0
                or len(layer['source_binding']['anchors']) != 1):
            _fail('native layer comparison supports only the bounded private EPUB extraction profile')
        maker = derivation['maker']
        if (maker['maker_type'] != 'software' or not _string(maker.get('configuration_ref'))
                or maker['configuration_ref'] != (Path(binding['text_layer']['record_ref']).parent / construction.CONFIG_FILE).as_posix()):
            _fail('native layer comparison requires its exact retained extraction configuration')
        configuration = resolver._record(maker['configuration_ref'], expected=maker['configuration_digest'])
        policy = resolver._record(layer['editorial_policy']['policy_ref'], expected=layer['editorial_policy']['policy_sha256'])
        try:
            validate_extraction_profile(configuration['selector'], policy)
            scope = configuration['source_scope']
            actual_scope = {key: layer['source_binding'][key] for key in ('work_ref', 'expression_ref', 'edition_ref', 'item_ref')}
            actual_scope.update(file_ref=layer['source_binding']['source_file_ref'], file_sha256=layer['source_binding']['source_file_sha256'])
            if (configuration['schema_version'] != construction.CONFIG or configuration['source_path'] != binding['text_layer']['record_ref']
                    or configuration['source_record_refs'] != binding['source_record_refs'] or scope != actual_scope
                    or configuration['policy'] != policy or configuration['language'] != rep['language']
                    or {k: v for k, v in maker.items() if k not in {'configuration_ref', 'configuration_digest'}} != configuration['maker']
                    or configuration['identities']['layer_id'] != layer['layer_id']
                    or configuration['identities']['provenance_event_id'] != layer['provenance_event_ref']
                    or configuration['derivation_access']['rights_record_refs'] != rep['rights_record_refs']):
                _fail('native layer extraction configuration differs from its fixed source carrier')
            item = resolver._record(binding['source_record_refs']['item'])
            manifest = resolver._record(item['item_manifest_ref'], expected=configuration['manifest_sha256'])
            entries = [row for row in manifest['payload_files'] if row['file_id'] == scope['file_ref']]
            if len(entries) != 1:
                _fail('native layer original File does not resolve uniquely')
            entry = entries[0]
            if (manifest['visibility'] != 'local_only' or entry['media_type'] != 'application/epub+zip'
                    or len(Path(entry['relative_path']).parts) != 2 or Path(entry['relative_path']).parts[0] != 'payload'
                    or not construction._member_path(entry['relative_path']) or entry['sha256'] != scope['file_sha256']):
                _fail('native layer source is outside the exact acquired EPUB profile')
            for kind, ref in binding['source_record_refs'].items():
                resolver._read(ref, expected=configuration['source_record_sha256'][kind])
            anchor_ref = layer['source_binding']['anchors'][0]
            anchor = resolver._record(anchor_ref['anchor_record_ref'], expected=anchor_ref['anchor_record_sha256'])
            self._check_anchor(anchor, configuration, maker, entry)
        except (KeyError, TypeError, AttributeError, IndexError):
            _fail('native layer extraction configuration lacks its exact supported data fields')
        grant = selection['payload_access']
        if grant is not None and entry['byte_size'] != grant['byte_size']:
            raise PermissionError('current acquired-file grant has another exact byte scope')
        record = Record.from_payload(layer['layer_id'], layer['layer_version'], layer, origin_id=selection['origin_id'])
        target = self._subjects[layer['layer_id']]
        if (target['record'] != record.ref or target['languages'] != [rep['language']]
                or target['maker_id'] != maker['agent_ref']):
            raise PermissionError('native layer scope disagrees with its source-owned record, language or maker')
        resolver._validate_schema_resource(COMPARISON_SCHEMA)
        self._resolvers.append(resolver)
        return selection, resolver, layer, configuration, entry

    @staticmethod
    def _check_anchor(anchor, configuration, maker, entry):
        scope, member, ids = configuration['source_scope'], configuration['member'], configuration['identities']
        if (type(member) is not dict or set(member) != {'member_path', 'member_sha256'}
                or not _string(member['member_path']) or not construction._member_path(member['member_path'])
                or not _sha(member['member_sha256'])):
            _fail('native layer configuration has an unsupported container member')
        payload_ref = (Path(configuration['source_record_refs']['item']).parent / entry['relative_path']).as_posix()
        expected = {'kind': 'selector_expression', 'expression': {'mode': 'refinement_chain', 'steps': [
            {'state': {'state_type': 'digest_state', 'representation_ref': payload_ref,
                       'representation_sha256': scope['file_sha256'], 'media_type': 'application/epub+zip'},
             'selector': {'type': 'container_member', **member, 'member_media_type': 'application/xhtml+xml'}},
            {'state': {'state_type': 'digest_state', 'representation_ref': member['member_path'],
                       'representation_sha256': member['member_sha256'], 'media_type': 'application/xhtml+xml'},
             'selector': configuration['selector']} ]}}
        if (anchor['selector_payload'] != expected or anchor['anchor_id'] != ids['anchor_id']
                or anchor.get('passage_id') != ids['passage_id']
                or anchor['provenance_event_ref'] != ids['provenance_event_id']
                or anchor['selector_method'] != {key: value for key, value in maker.items() if key != 'agent_ref'}):
            _fail('native layer source anchor differs from its exact extraction data')

    def _materialize(self, selection, resolver, layer, configuration, entry, deadline):
        record = Record.from_payload(layer['layer_id'], layer['layer_version'], layer, origin_id=selection['origin_id'])
        rep, comparison = layer['representation'], None
        read_ready = False
        if selection['payload_access'] is not None:
            self._check_selections()
            check_local_research_rights(resolver, layer)
            # This deliberately does not invoke creation/configuration admission.
            payload_config = {key: copy.deepcopy(configuration[key]) for key in ('source_record_refs', 'source_scope', 'member')}
            payload_config['source_access'] = copy.deepcopy(selection['payload_access'])
            member, identity = _read_payload(payload_config, entry, self._payload_deadline(deadline))
            self._check_selections()
            if len(member) > MAX_RECORD_BYTES:
                _fail('native layer comparison must narrow its source member; truncation is forbidden')
            expected = extract_xhtml_text(member, selector=configuration['selector'], policy=configuration['policy'])
            resolver.resolve_layer(selection['binding'], verify_content=True, allow_private_content=True)
            actual = resolver._read(rep['content_ref'], expected=rep['content_sha256'], content=True).decode('utf-8')
            if rep['text_scope']['end'] != len(actual):
                _fail('native layer comparison requires the whole declared extraction representation')
            payload_ref = (Path(configuration['source_record_refs']['item']).parent / entry['relative_path']).as_posix()
            fixity = [{'ref': ref, 'category': category, 'sha256': digest}
                      for (ref, category), digest in sorted(resolver._inputs.items())]
            fixity += [{'ref': payload_ref, 'category': 'original_payload', 'sha256': configuration['source_scope']['file_sha256']},
                       {'ref': payload_ref + '!/' + configuration['member']['member_path'], 'category': 'source_member',
                        'sha256': configuration['member']['member_sha256']}]
            body = {'schema_version': 'tos_native_text_layer_comparison_v1', 'comparison_version': 1,
                'layer': record.ref, 'source_scope': copy.deepcopy(configuration['source_scope']),
                'member': copy.deepcopy(configuration['member']), 'selector': copy.deepcopy(configuration['selector']),
                'source_member_utf8': member.decode('utf-8'), 'expected_text': expected, 'representation_text': actual,
                'representation': copy.deepcopy(rep), 'editorial_policy': copy.deepcopy(configuration['policy']),
                'maker': copy.deepcopy(layer['derivation']['maker']), 'configuration_sha256': layer['derivation']['maker']['configuration_digest'],
                'input_fixity': fixity, 'deterministic_match': expected == actual,
                'visibility': 'local_only', 'publication_authorized': False, 'performs_semantic_assessment': False}
            body['comparison_id'] = 'tos.text-comparison.sha256.' + _hash(_canonical(body))
            resolver._validate(body, COMPARISON_SCHEMA)
            comparison = Record.from_payload(body['comparison_id'], 1, body, origin_id=selection['origin_id'])
            read_ready = body['deterministic_match']
            self._payloads.append((payload_config, copy.deepcopy(entry), identity, _hash(member)))
        self.records.append({'id': record.id, 'version': record.version, 'payload': record.payload, 'origin_id': record.origin_id})
        if comparison is not None:
            self.records.append({'id': comparison.id, 'version': comparison.version, 'payload': comparison.payload, 'origin_id': comparison.origin_id})
        self.layers[record.id] = {'record': record, 'comparison': comparison, 'binding': copy.deepcopy(selection['binding']),
            'language': rep['language'], 'maker_id': layer['derivation']['maker']['agent_ref'],
            'scope': {key: copy.deepcopy(rep[key]) for key in ('content_file_id', 'content_sha256', 'text_scope')},
            'read_ready': read_ready}
        self.contracts.update(resolver.schema_digests)

    def snapshot(self):
        """Recheck grants, all source bytes/ancestors, rights and acquired inputs."""
        self._check_selections()
        if self.context.snapshot() != self._context_snapshot:
            raise JournalConflict('native layer owner context changed during comparison')
        sources = [resolver.snapshot() for resolver in self._resolvers]
        deadline = time.monotonic() + MAX_SECONDS
        for config, entry, expected_identity, digest in self._payloads:
            member, identity = _read_payload(config, entry, self._payload_deadline(deadline))
            if identity != expected_identity or _hash(member) != digest:
                raise JournalConflict('native layer acquired payload or its ancestors changed')
        self._check_selections()
        if sources != [resolver.snapshot() for resolver in self._resolvers] or self.context.snapshot() != self._context_snapshot:
            raise JournalConflict('native layer source changed while rechecking acquired inputs')
        return 'sha256:' + _hash(_canonical({'selection': _hash(self._selection_bytes), 'context': self._context_snapshot,
            'sources': sources, 'payloads': [{'identity': identity, 'member_sha256': digest}
                                           for _, _, identity, digest in self._payloads]}))

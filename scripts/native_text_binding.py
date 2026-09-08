"""Bounded return from a description to native TextUnit/Anchor evidence.

The default read is metadata-only and works without local source text. Exact
resolution is explicit, reads one frozen UTF-8 representation without newline
or Unicode rewriting, and never returns its text. A caller must separately
hold authority to read private content. No mode admits a source, accepts a
segmentation, authenticates a producer, grants publication, or executes a
selector, model, command, URL or supplied code.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import unicodedata

from jsonschema import Draft202012Validator, FormatChecker
from jsonschema.exceptions import SchemaError
from referencing import Registry, Resource
from referencing.exceptions import Unresolvable


SOURCE_HOME = Path('ToS/source-witnesses')
CONTRACT_HOME = Path('ToS/contracts')
BINDING_SCHEMA = 'native-text-unit-binding.schema.json'
ASSESSMENT_SCHEMA = 'native-text-unit-assessment-subject.schema.json'
MAX_METADATA_FILE_BYTES = 1_048_576
MAX_INPUT_FILES = 128


class NativeTextBindingError(ValueError):
    """A binding cannot resolve without widening or inventing its evidence."""


def _hash(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _json(raw: bytes) -> dict:
    def pairs(items):
        value = {}
        for key, item in items:
            if key in value:
                raise NativeTextBindingError('native binding input has duplicate JSON fields')
            value[key] = item
        return value

    try:
        value = json.loads(raw.decode('utf-8'), object_pairs_hook=pairs)
        if not isinstance(value, dict):
            raise NativeTextBindingError('native binding input must be a JSON object')
        json.dumps(value, allow_nan=False)
        return value
    except (UnicodeError, ValueError, RecursionError) as error:
        raise NativeTextBindingError('native binding input is not bounded, finite UTF-8 JSON') from error


def _regular_bytes(path: Path, limit: int) -> bytes:
    """No-follow every ancestor, reject special files, detect in-place change.

    Source commands can instead supply their stricter local-account reader;
    this library does not infer write or account authority from readability.
    """
    descriptor = None
    try:
        descriptor = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
        for index, part in enumerate(path.parts[1:]):
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
            if index < len(path.parts) - 2:
                flags |= os.O_DIRECTORY
            child = os.open(part, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise NativeTextBindingError('native binding input is not a regular file')
        with os.fdopen(descriptor, 'rb') as stream:
            descriptor = None
            raw = stream.read(limit + 1)
            after = os.fstat(stream.fileno())
        if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
                after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise NativeTextBindingError('native binding input changed during read')
        return raw
    except OSError as error:
        # Paths, selectors and short-span hashes can be private source content.
        raise NativeTextBindingError('native binding input is missing, unsafe or unreadable') from error
    finally:
        if descriptor is not None:
            os.close(descriptor)


class NativeTextBindingResolver:
    """One explicit dependency snapshot, without scanning the corpus.

    Only schema digests are public. Native packet/anchor bodies, short-span
    hashes, content locators and rights inputs stay in the opaque snapshot.
    Observed dependencies are never refreshed midway through a command.
    """

    def __init__(self, root: Path, *, read_bytes=None,
                 max_metadata_bytes=8_388_608, max_content_bytes=8_388_608):
        self.root = Path(root)
        if (not self.root.is_absolute() or '..' in self.root.parts
                or self.root == Path('/')):
            raise NativeTextBindingError('native binding needs an absolute dedicated source root')
        if (type(max_metadata_bytes) is not int or max_metadata_bytes < 1
                or type(max_content_bytes) is not int or max_content_bytes < 1):
            raise NativeTextBindingError('native binding budgets must be positive integers')
        self._reader = read_bytes or _regular_bytes
        self._budgets = {'metadata': max_metadata_bytes, 'content': max_content_bytes}
        self._remaining = dict(self._budgets)
        self._inputs = {}
        self._cache = {}
        self._validators = {}
        self.schema_digests = {}
        self._support_refs = set()

    def _path(self, ref: str, *, content=False, schema=False, support=False) -> Path:
        if not isinstance(ref, str) or not ref or '\x00' in ref or '\\' in ref:
            raise NativeTextBindingError('native binding reference is not a local owner path')
        path = Path(ref)
        if (path.is_absolute() or '..' in path.parts or path.as_posix() != ref
                or not path.is_relative_to(CONTRACT_HOME if schema else Path('ToS') if support else SOURCE_HOME)
                or 'catalog' in path.parts
                or (not content and any(part in {'payload', 'local-content'} for part in path.parts))):
            raise NativeTextBindingError('native binding reference escapes its declared owner home')
        return self.root / path

    def _read(self, ref: str, *, expected=None, content=False, schema=False, support=False) -> bytes:
        path = self._path(ref, content=content, schema=schema, support=support)
        if support:
            self._support_refs.add(ref)
        category = 'content' if content else 'metadata'
        key = ref, category
        if key not in self._cache:
            if len(self._inputs) >= MAX_INPUT_FILES:
                raise NativeTextBindingError('native binding exceeds its dependency-count budget')
            limit = self._remaining[category]
            if not content:
                limit = min(limit, MAX_METADATA_FILE_BYTES)
            if limit < 0:
                raise NativeTextBindingError('native binding exceeds its input-byte budget')
            try:
                raw = self._reader(path, limit)
            except (OSError, ValueError) as error:
                raise NativeTextBindingError('native binding input is missing, unsafe or changed') from error
            if not isinstance(raw, bytes) or len(raw) > limit:
                raise NativeTextBindingError('native binding exceeds its input-byte budget')
            self._remaining[category] -= len(raw)
            digest = _hash(raw)
            self._inputs[key] = digest
            self._cache[key] = raw
        raw = self._cache[key]
        # One dependency can first be encountered as a fixed support record
        # and later as an actual grammar. Its roles do not depend on cache order.
        if schema:
            self.schema_digests[ref] = _hash(raw)
        if expected is not None and _hash(raw) != expected:
            raise NativeTextBindingError('native binding exact input digest differs')
        return raw

    def _record(self, ref, *, expected=None):
        return _json(self._read(ref, expected=expected))

    def _validate(self, value, schema_name):
        if schema_name not in self._validators:
            ref = (CONTRACT_HOME / schema_name).as_posix()
            schema = _json(self._read(ref, schema=True))
            if schema.get('$id') != 'https://tree-of-sophia.local/' + ref:
                raise NativeTextBindingError('native binding schema identity differs from its owner path')
            try:
                Draft202012Validator.check_schema(schema)
                # Most native contracts are self-contained. The assessment
                # view composes exactly two existing contracts, never a URL
                # selected by source material or by a changed schema.
                def reject_external(_uri):
                    raise NativeTextBindingError('native binding schema selects an undeclared resource')
                registry = Registry(retrieve=reject_external)
                if schema_name == ASSESSMENT_SCHEMA:
                    for name in (BINDING_SCHEMA, 'source-text-unit-packet-v1.schema.json'):
                        self._validate_schema_resource(name)
                        dependency = _json(self._read((CONTRACT_HOME / name).as_posix(), schema=True))
                        registry = registry.with_resource(dependency['$id'], Resource.from_contents(dependency))
                self._validators[schema_name] = Draft202012Validator(
                    schema, format_checker=FormatChecker(), registry=registry)
            except (ValueError, SchemaError) as error:
                raise NativeTextBindingError('native binding schema is invalid') from error
        try:
            if not self._validators[schema_name].is_valid(value):
                raise NativeTextBindingError('native binding violates its exact schema')
        except Unresolvable as error:
            raise NativeTextBindingError('native binding schema selects an undeclared resource') from error
        except RecursionError as error:
            raise NativeTextBindingError('native binding exceeds schema nesting limits') from error

    def _validate_schema_resource(self, name):
        ref = (CONTRACT_HOME / name).as_posix()
        value = _json(self._read(ref, schema=True))
        if value.get('$id') != 'https://tree-of-sophia.local/' + ref:
            raise NativeTextBindingError('native schema resource identity differs')
        Draft202012Validator.check_schema(value)

    def assessment_records(self, binding: dict, *, origin_id: str,
                           verify_content=False, allow_private_content=False) -> dict:
        """Adapt one native unit and its distinct layer for local assessment.

        The subject digest identifies this versioned view, NOT the raw unit
        or packet. Native IDs/versions stay intact. All native packet fields
        survive in the view; the layer is a separate evidence Record with the
        same origin, not an independent corroborating source. Nothing here is
        a public projection or permission to publish these private metadata.
        """
        if not isinstance(origin_id, str) or not origin_id.strip():
            raise NativeTextBindingError('native assessment needs an explicit evidence origin')
        summary = self.resolve(binding, verify_content=verify_content,
                               allow_private_content=allow_private_content)
        packet = self._record(binding['packet_ref'])
        layer = self._record(binding['text_layer']['record_ref'])
        canonical = lambda value: json.dumps(value, sort_keys=True, separators=(',', ':'),
                                             ensure_ascii=False, allow_nan=False).encode('utf-8')
        layer_ref = {'id': layer['layer_id'], 'version': layer['layer_version'],
                     'digest': 'sha256:' + _hash(canonical(layer))}
        # Load the view's public grammar before fixing its dependency closure.
        self._validate_schema_resource(ASSESSMENT_SCHEMA)
        payload = {'schema_version': 'tos_native_text_unit_assessment_subject_v1',
                   'native_binding': binding, 'packet': packet, 'text_layer': layer_ref,
                   'content_verified': summary['content_verified'], 'input_snapshot': self.snapshot()}
        self._validate(payload, ASSESSMENT_SCHEMA)
        if self.snapshot() != payload['input_snapshot']:
            raise NativeTextBindingError('native assessment view changed while being assembled')
        records = [{'id': binding['unit_id'], 'version': binding['unit_version'],
                    'payload': payload, 'origin_id': origin_id},
                   {'id': layer_ref['id'], 'version': layer_ref['version'],
                    'payload': layer, 'origin_id': origin_id}]
        if any(len(canonical(row['payload'])) > MAX_METADATA_FILE_BYTES for row in records):
            raise NativeTextBindingError('native assessment view exceeds the one-record byte budget')
        return {'records': records, 'summary': summary}

    def _source_scope(self, binding, packet, layer):
        """Validate current metadata topology, not historical or textual truth."""
        scope = packet['source_scope']
        layer_scope = layer['source_binding']
        records = {}
        for kind, ref in binding['source_record_refs'].items():
            if Path(ref).name != kind + '.json':
                raise NativeTextBindingError('native source-scope locator has another metadata kind')
            record = self._record(ref)
            self._validate(record, 'corpus-record.schema.json')
            if (record.get('record_type') != kind or record.get('record_id') != scope[kind + '_ref']
                    or layer_scope[kind + '_ref'] != scope[kind + '_ref']):
                raise NativeTextBindingError('native source-scope identity differs')
            records[kind] = record
        if (records['expression'].get('work_ref') != scope['work_ref']
                or scope['expression_ref'] not in records['edition'].get('embodies_expression_refs', [])):
            raise NativeTextBindingError('native source-scope bibliographic topology differs')
        item = records['item']
        manifest_ref = item.get('item_manifest_ref')
        if (not isinstance(manifest_ref, str)
                or Path(manifest_ref).parent != Path(binding['source_record_refs']['item']).parent
                or Path(manifest_ref).name != 'item.manifest.json'):
            raise NativeTextBindingError('native source-scope manifest leaves its exact item')
        manifest = self._record(manifest_ref)
        self._validate(manifest, 'source-item-manifest.schema.json')
        if (manifest['item_id'] != scope['item_ref']
                or manifest['embodiment_ref'] != scope['edition_ref']
                or layer_scope['source_file_ref'] != scope['file_ref']
                or layer_scope['source_file_sha256'] != scope['file_sha256']):
            raise NativeTextBindingError('native source-scope item/file binding differs')
        matches = [entry for entry in manifest['payload_files'] if entry['file_id'] == scope['file_ref']]
        if len(matches) != 1 or matches[0]['sha256'] != scope['file_sha256']:
            raise NativeTextBindingError('native source file is not uniquely present in its item manifest')
        return manifest

    def _layer_dependencies(self, layer, *, visiting=frozenset()):
        """Retain fixed predecessor and policy bytes, without replaying OCR.

        Returning to immutable input records is not proof that the recorded
        transformation occurred or that its output is a faithful transcription.
        """
        from validate_source_witness_foundation import _source_text_layer_semantic_issues
        if layer['layer_id'] in visiting or len(visiting) >= 16:
            raise NativeTextBindingError('native text-layer lineage is cyclic or exceeds its depth budget')
        if _source_text_layer_semantic_issues(layer):
            raise NativeTextBindingError('native predecessor layer violates its evidence contract')
        policy = layer['editorial_policy']
        self._read(policy['policy_ref'], expected=policy['policy_sha256'], support=True)
        maker = layer['derivation']['maker']
        if maker.get('configuration_ref') is not None:
            self._read(maker['configuration_ref'], expected=maker['configuration_digest'], support=True)
        for target in layer['derivation']['input_layers']:
            previous = self._record(target['record_ref'], expected=target['record_sha256'])
            self._validate(previous, 'source-text-layer.schema.json')
            if (previous['layer_id'] != target['layer_id']
                    or previous['representation']['content_sha256'] != target['content_sha256']):
                raise NativeTextBindingError('native predecessor identity or content binding differs')
            self._layer_dependencies(previous, visiting=visiting | {layer['layer_id']})

    def resolve(self, binding: dict, *, verify_content=False, allow_private_content=False) -> dict:
        if type(verify_content) is not bool or type(allow_private_content) is not bool:
            raise NativeTextBindingError('native content-read controls must be explicit booleans')
        self._validate(binding, BINDING_SCHEMA)
        packet = self._record(binding['packet_ref'], expected=binding['packet_sha256'])
        self._validate(packet, 'source-text-unit-packet-v1.schema.json')
        if (packet['content_posture'] != 'source_bound'
                or packet['packet_id'] != binding['packet_id']
                or packet['packet_version'] != binding['packet_version']):
            raise NativeTextBindingError('native packet identity/version or source posture differs')
        layer_binding = binding['text_layer']
        if packet['source_layer']['text_layer_ref'] != layer_binding['record_ref']:
            raise NativeTextBindingError('native packet addresses another text-layer record')
        layer = self._record(layer_binding['record_ref'], expected=layer_binding['record_sha256'])
        self._validate(layer, 'source-text-layer.schema.json')
        if (layer['layer_id'] != layer_binding['layer_id']
                or layer['layer_version'] != layer_binding['layer_version']):
            raise NativeTextBindingError('native text-layer identity/version differs')

        # Reuse the source owner's pure mechanics; never its laboratory reads.
        from validate_source_witness_foundation import (
            _anchor_v2_semantic_issues, _source_text_layer_semantic_issues,
            _source_text_unit_v1_issues,
        )
        if (_source_text_unit_v1_issues(packet)
                or _source_text_layer_semantic_issues(layer)):
            raise NativeTextBindingError('native packet or layer violates its internal evidence contract')
        self._layer_dependencies(layer)
        manifestation = self._source_scope(binding, packet, layer)
        rep = layer['representation']
        packet_layer = packet['source_layer']
        unicode_form = {'none': 'source_preserved'}.get(rep['character_normalization'], rep['character_normalization'])
        if (packet_layer['text_layer_sha256'] != rep['content_sha256']
                or packet_layer['language'] != rep['language']
                or packet_layer['unicode_form'] != unicode_form
                or packet_layer['visibility'] != rep['content_visibility']
                or packet_layer['publication_authorized'] != rep['publication_authorized']
                or rep['media_type'] not in {'text/plain', 'text/plain; charset=utf-8'}):
            raise NativeTextBindingError('native packet and UTF-8 layer declarations differ')
        scope = rep['text_scope']
        for anchor in packet['anchors']:
            if (anchor['source_return']['locator_ref'] != rep['content_ref']
                    or not scope['start'] <= anchor['selector']['start'] <= anchor['selector']['end'] <= scope['end']):
                raise NativeTextBindingError('native unit anchor leaves the exact representation scope')

        for target in layer['source_binding']['anchors']:
            anchor = self._record(target['anchor_record_ref'], expected=target['anchor_record_sha256'])
            self._validate(anchor, 'source-anchor-v2.schema.json')
            if (anchor['anchor_id'] != target['anchor_id']
                    or anchor['target']['item_id'] != packet['source_scope']['item_ref']
                    or anchor['target']['file_id'] != packet['source_scope']['file_ref']
                    or anchor['target']['file_sha256'] != packet['source_scope']['file_sha256']
                    or _anchor_v2_semantic_issues(anchor)):
                raise NativeTextBindingError('native text-layer source-anchor binding differs')
        # Rights records are fixed inputs, not new rights judgments. The exact
        # Item/File gate remains separate from a narrower textual-layer gate.
        rights_refs = set(packet['rights_and_visibility']['rights_record_refs'])
        declared_rights = {entry['ref'] for entry in rep['rights_record_refs']}
        if (not declared_rights or rights_refs != declared_rights
                or manifestation['rights_ref'] not in declared_rights):
            raise NativeTextBindingError('native packet/layer rights closure differs')
        relevant = {layer['layer_id'], rep['content_file_id'],
                    *(value for key, value in packet['source_scope'].items() if key.endswith('_ref'))}
        rights_records = []
        for target in rep['rights_record_refs']:
            rights = self._record(target['ref'], expected=target['sha256'])
            self._validate(rights, 'rights-record.schema.json')
            if not relevant.intersection(rights['scope_refs']):
                raise NativeTextBindingError('native rights record addresses a different source')
            if target['ref'] == manifestation['rights_ref'] and not {
                    packet['source_scope']['item_ref'], packet['source_scope']['file_ref']
                    }.issubset(rights['scope_refs']):
                raise NativeTextBindingError('native item rights do not cover the exact item and file')
            rights_records.append(rights)
        for target in rep['publication_authority_refs']:
            self._read(target['ref'], expected=target['sha256'], support=True)

        units = [unit for unit in packet['units'] if unit['unit_id'] == binding['unit_id']]
        segments = [segment for segment in packet['segmentations']
                    if segment['segmentation_id'] == binding['segmentation_id']]
        if len(units) != 1 or len(segments) != 1:
            raise NativeTextBindingError('native unit or segmentation identity is not unique in its packet')
        unit, segment = units[0], segments[0]
        if (unit['unit_version'] != binding['unit_version']
                or unit['ordered_anchor_refs'] != binding['ordered_anchor_refs']
                or unit['surface_posture'] != 'source_bearing'
                or segment['segmentation_version'] != binding['segmentation_version']
                or unit['unit_id'] not in segment['ordered_unit_refs']):
            raise NativeTextBindingError('native unit membership, version or ordered anchors differ')

        rights = packet['rights_and_visibility']
        declared_public = (rep['content_visibility'] == 'public'
            and rep['publication_authorized'] is True
            and rights['packet_visibility'] == rights['effective_visibility'] == 'public'
            and rights['publication_authorized'] is True and rights['private_source_used'] is False)
        if declared_public:
            # Exact layer/file decisions may be narrower than the original
            # Item's aggregate gate. A positive original-Work entry alone is
            # not such a decision. This checks recorded gates, not legal truth.
            exact_scope = {layer['layer_id'], rep['content_file_id']}
            applicable = [record for record in rights_records if exact_scope.intersection(record['scope_refs'])]
            applicable = applicable or rights_records
            if any(record['assessment_status'] not in {'public_domain_reviewed', 'licensed', 'permission_granted'}
                    or record['visibility'] != 'public_payload'
                    or record['redistribution_posture'] not in {'authorized', 'authorized_with_conditions'}
                    or record['derivative_posture'] not in {'allowed', 'allowed_with_conditions'}
                    or record['review_status'] in {'legal_review_requested', 'superseded'}
                    for record in applicable):
                raise NativeTextBindingError('native public-content declaration conflicts with its current rights gate')
        if verify_content:
            if not declared_public and not allow_private_content:
                raise NativeTextBindingError('exact private text resolution requires explicit owner-local access')
            raw = self._read(rep['content_ref'], expected=rep['content_sha256'], content=True)
            try:
                text = raw.decode('utf-8')
            except UnicodeError as error:
                raise NativeTextBindingError('native text representation is not exact UTF-8') from error
            if not 0 <= scope['start'] <= scope['end'] <= len(text):
                raise NativeTextBindingError('native text-layer scope leaves the exact content')
            selected = text[scope['start']:scope['end']]
            if unicode_form in {'NFC', 'NFD', 'NFKC', 'NFKD'} and unicodedata.normalize(unicode_form, selected) != selected:
                raise NativeTextBindingError('native text contradicts its declared Unicode form')
            if _source_text_unit_v1_issues(packet, text=text):
                raise NativeTextBindingError('native unit anchors or coverage do not resolve exact bytes')
        # A cached resolver must not return a stale successful validation. A
        # command still rechecks this opaque snapshot at its publication edge.
        self.snapshot()
        return {
            'metadata_verified': True, 'content_verified': bool(verify_content),
            'original_payload_verified': False,
            'unit_id': unit['unit_id'], 'unit_version': unit['unit_version'],
            'unit_kind': unit['unit_kind'], 'segmentation_id': segment['segmentation_id'],
            'segmentation_version': segment['segmentation_version'],
            'layer_id': layer['layer_id'], 'layer_version': layer['layer_version'],
            'language': rep['language'], 'effective_visibility': rights['effective_visibility'],
            'public_content_available': bool(verify_content and declared_public),
            'native_status': {'unit_boundary_posture': unit['boundary_posture'],
                              'segmentation_status': segment['status'],
                              'layer_review_status': layer['admission']['review_status']},
            'assessment_applied': False,
        }

    def snapshot(self, *, read_bytes=None) -> str:
        """Recheck the exact closure; expose only an opaque fingerprint."""
        reader = read_bytes or self._reader
        remaining = dict(self._budgets)
        for (ref, category), digest in sorted(self._inputs.items()):
            limit = remaining[category]
            if category != 'content':
                limit = min(limit, MAX_METADATA_FILE_BYTES)
            try:
                raw = reader(self._path(ref, content=category == 'content',
                            schema=ref in self.schema_digests, support=ref in self._support_refs), limit)
            except (OSError, ValueError) as error:
                raise NativeTextBindingError('native binding dependency changed or became unreadable') from error
            if not isinstance(raw, bytes) or len(raw) > limit or _hash(raw) != digest:
                raise NativeTextBindingError('native binding dependency changed after resolution')
            remaining[category] -= len(raw)
        value = [[ref, category, digest] for (ref, category), digest in sorted(self._inputs.items())]
        return 'sha256:' + _hash(json.dumps(value, separators=(',', ':'), ensure_ascii=True).encode())

"""Independent public project-text capture through the common source front door.

Only exact existing project-authored UTF-8 source, explicit current owner rights
and publication scope, and a supplied finite partition are supported. This is
not a private grant extension, a third-party text import, semantic admission,
external publication, OCR, model execution or a new native ontology.
"""
from __future__ import annotations

import base64
from datetime import datetime, timezone
import os
from pathlib import Path
import stat
import time
from types import SimpleNamespace

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

import source_commands as source
import source_command_contracts as contract
import source_text_layer_commands as layers
import source_text_unit_commands as units
import source_item_deposit as deposit
from source_owner_context import _absolute, _open, _read as protected_read
from source_revisions import _file_refs
from native_text_binding import NativeTextBindingResolver
from source_text_layer_proposal import build_public_utf8_layer, PUBLIC_UTF8_POLICY, record_bytes
from source_text_unit_proposal import build_public_text_unit_proposal


CONFIG = 'tos_public_native_text_create_owner_v1'
OPERATION = 'native-text.create'
CONFIG_SCHEMA = 'ToS/contracts/public-native-text-create-owner.schema.json'
AUTHORITY_SCHEMA = 'ToS/contracts/public-native-text-authority.schema.json'
PLAN = 'tos_public_native_construction_plan_v1'
PLAN_FILE = 'construction-plan.json'
BASENAME = 'source-text-unit.v1.json'
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_public_native_commands.py'
SCHEMAS = (CONFIG_SCHEMA, AUTHORITY_SCHEMA, units.PACKET_SCHEMA)
IMPLEMENTATIONS = (*layers.IMPLEMENTATIONS, MODULE_REF)
MAX_FILES = 12
MAX_PACKAGE_BYTES = 2 * 1024 * 1024
MAX_INPUT_BYTES = 16 * 1024 * 1024
MAX_INPUT_FILES = 128
PRIVATE_PARTS = {'owner-local', 'payload', 'local-content', 'catalog'}
SOURCE_HOMES = ('ToS/review-ledger', 'ToS/doctrine', 'docs')


def _ref(value, *, source_text=False):
    if not isinstance(value, str) or not value or '\x00' in value or '\\' in value:
        raise ValueError('public construction requires a canonical owner-relative reference')
    path = Path(value)
    if (path.is_absolute() or path.as_posix() != value or '..' in path.parts
            or any(part in PRIVATE_PARTS or part.startswith('.') for part in path.parts)):
        raise PermissionError('public construction cannot read private or indirect source paths')
    if source_text and not any(path.is_relative_to(home) for home in SOURCE_HOMES):
        raise PermissionError('public capture selects existing project documentation, not a corpus payload')
    return path


def _schemas(root):
    raw = {ref: source._read(root / ref, source.MAX_COMMAND_BYTES) for ref in SCHEMAS}
    def reject(_uri):
        raise ValueError('public-native schema cannot select another resource')
    registry = Registry(retrieve=reject)
    parsed = {}
    for ref, body in raw.items():
        schema = source._json_object(body)
        if schema.get('$id') != 'https://tree-of-sophia.local/' + ref:
            raise ValueError('public-native schema differs from its owner identity')
        Draft202012Validator.check_schema(schema)
        registry = registry.with_resource(schema['$id'], Resource.from_contents(schema))
        parsed[ref] = schema
    return ({ref: Draft202012Validator(schema, registry=registry, format_checker=FormatChecker())
             for ref, schema in parsed.items()}, raw)


def _native_ids(config):
    ids = config['identities']
    result = [ids[key] for key in ('layer_id', 'anchor_id', 'passage_id', 'provenance_event_id',
        'packet_id', 'scheme_id', 'segmentation_id', 'scope_anchor_ref')]
    for slot in ids['unit_slots']:
        result.extend((slot['unit_id'], slot['anchor_ref']))
    return sorted([*result, *ids['gap_anchor_refs']])


def _current(config):
    if (config['uid'] != os.getuid() or type(config['uid']) is not int
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)):
        raise PermissionError('public-native delegation is not current for this account')


def configuration(config, *, owner_config):
    raw = protected_read(Path(owner_config), source.MAX_COMMAND_BYTES, confidential_file=True)
    if source._json_object(raw) != config:
        raise source.JournalConflict('public-native grant changed during selection')
    root, recovery = _absolute(config.get('source_root')), _absolute(config.get('recovery_root'))
    os.close(source._owned_path(root, directory=True))
    os.close(_open(recovery, directory=True, private_root=recovery))
    if recovery.is_relative_to(root) or root.is_relative_to(recovery):
        raise PermissionError('public source and confidential recovery control must be disjoint')
    validators, schemas = _schemas(root)
    if not validators[CONFIG_SCHEMA].is_valid(config):
        raise ValueError('public-native grant violates its independent exact schema')
    _current(config)
    relative = _ref(config['source_path'])
    if (not relative.is_relative_to('ToS/source-witnesses/works')
            or relative.name != BASENAME or len(relative.parts) < 7):
        raise PermissionError('public-native output must be a new exact source-owned package')
    os.close(source._owned_path(root / relative.parent.parent, directory=True))
    _ref(config['source']['ref'], source_text=True)
    for binding in [*config['rights_record_refs'], config['publication_authority'], *config['license_bindings']]:
        _ref(binding['ref'])
    if not Path(config['publication_authority']['ref']).is_relative_to('ToS'):
        raise PermissionError('public authority must have a retained ToS owner source')
    for kind, ref in config['source_record_refs'].items():
        if _ref(ref).name != kind + '.json' or not Path(ref).is_relative_to('ToS/source-witnesses'):
            raise PermissionError('public-native corpus bindings require exact authored metadata')
    if (config['source']['sha256'] != config['source_scope']['file_sha256']
            or config['source_scope']['file_ref'] != 'tos.file.sha256.' + config['source']['sha256']
            or config['source']['byte_size'] > config['limits']['max_source_bytes']):
        raise ValueError('public input does not bind the exact declared original File')
    ids = _native_ids(config)
    if len(ids) != len(set(ids)):
        raise ValueError('public-native identities must be distinct')
    method = config['unit_proposal']['method']
    if (method['maker_kind'] != 'software' or method['agent_ref'] != config['principal_id']
            or method['provenance_event_ref'] != config['identities']['provenance_event_id']
            or method['configuration_ref'] != (relative.parent / PLAN_FILE).as_posix()):
        raise PermissionError('public-native method must declare the actual local constructor and public plan')
    for bindings in (config['rights_record_refs'], config['license_bindings']):
        if len({row['ref'] for row in bindings}) != len(bindings):
            raise ValueError('public-native evidence cannot repeat one ref with different declarations')
    digest = source._digest(source._canonical({'protected_configuration_bytes': source._digest(raw),
        'contracts': {ref: source._digest(body) for ref, body in schemas.items()}}))
    return config, digest, root / relative


def _public_plan(config, configuration_digest):
    # The public plan is an explicit projection, never a retained credential.
    selected = {key: value for key, value in config.items()
                if key not in {'source_root', 'recovery_root', 'uid'}}
    return {'schema_version': PLAN, 'selection': selected,
            'protected_grant_digest': configuration_digest,
            'authority_boundary': 'construction-plan-not-a-grant-license-or-assessment'}


def _authority(config, resolver, read):
    root = Path(config['source_root'])
    authority_binding = config['publication_authority']
    authority = source._json_object(read(authority_binding['ref'], authority_binding['sha256']))
    validators, _ = _schemas(root)
    if not validators[AUTHORITY_SCHEMA].is_valid(authority):
        raise ValueError('public-native authority violates its exact source contract')
    now = datetime.now(timezone.utc)
    if (authority['authority_id'] != config['authority_ref']
            or authority['granted_to'] != config['principal_id']
            or source._instant(authority['issued_at']) > now
            or source._instant(authority['expires_at']) <= now
            or source._instant(config['expires_at']) > source._instant(authority['expires_at'])
            or authority['source'] != config['source'] or authority['source_scope'] != config['source_scope']
            or authority['license_bindings'] != config['license_bindings']
            or authority['output_scope']['package_ref'] != Path(config['source_path']).parent.as_posix()
            or authority['output_scope']['native_identities'] != _native_ids(config)
            or authority['output_scope']['content_file_id'] != 'tos.file.sha256.' + authority['output_scope']['content_sha256']):
        raise PermissionError('public-native authority does not cover this exact source, maker, time and output scope')
    for binding in config['license_bindings']:
        read(binding['ref'], binding['sha256'])
    scope = config['source_scope']
    for kind, ref in config['source_record_refs'].items():
        row = resolver._record(ref, expected=config['source_record_sha256'][kind])
        resolver._validate(row, 'corpus-record.schema.json')
        if row.get('record_id') != scope[kind + '_ref'] or row.get('record_type') != kind:
            raise ValueError('public-native source metadata differs from its independently selected identity')
    layer_scope = {key: scope[key] for key in ('work_ref', 'expression_ref', 'edition_ref', 'item_ref')}
    layer_scope.update(source_file_ref=scope['file_ref'], source_file_sha256=scope['file_sha256'])
    manifest = resolver._source_scope(config, scope, {'source_binding': layer_scope})
    item = resolver._record(config['source_record_refs']['item'])
    resolver._read(item['item_manifest_ref'], expected=config['manifest_sha256'])
    entry = next(row for row in manifest['payload_files'] if row['file_id'] == scope['file_ref'])
    if entry['byte_size'] != config['source']['byte_size'] or entry['media_type'] != config['source']['media_type']:
        raise ValueError('project text does not match the acquired Item File metadata')
    if manifest['rights_ref'] not in {row['ref'] for row in config['rights_record_refs']}:
        raise PermissionError('public-native construction cannot omit the original Item rights')
    output_scope = {config['identities']['layer_id'], authority['output_scope']['content_file_id']}
    covered = set()
    for binding in config['rights_record_refs']:
        record = resolver._record(binding['ref'], expected=binding['sha256'])
        resolver._validate(record, 'rights-record.schema.json')
        scopes = set(record['scope_refs'])
        if (record['assessment_status'] in {'permission_denied', 'conflicting_evidence'}
                or record['review_status'] in {'superseded', 'legal_review_requested'}
                or not scopes.intersection(output_scope | {scope['item_ref'], scope['file_ref']})):
            raise PermissionError('public-native rights are denied, inactive or scoped to another source')
        if binding['ref'] == manifest['rights_ref'] and not {scope['item_ref'], scope['file_ref']}.issubset(scopes):
            raise PermissionError('public-native Item rights must cover its exact Item and File')
        if scopes.intersection(output_scope):
            if (record['assessment_status'] not in {'licensed', 'permission_granted', 'public_domain_reviewed'}
                    or record['visibility'] != 'public_payload'
                    or record['redistribution_posture'] not in {'authorized', 'authorized_with_conditions'}
                    or record['derivative_posture'] not in {'allowed', 'allowed_with_conditions'}
                    or authority_binding['ref'] not in record['source_refs']
                    or not {row['ref'] for row in config['license_bindings']}.issubset(record['source_refs'])):
                raise PermissionError('every output-bearing rights decision needs exact positive public scope and evidence')
            covered.update(scopes.intersection(output_scope))
    if covered != output_scope:
        raise PermissionError('new public layer and representation File both require affirmative exact rights')
    return authority


def _prepare(config, configuration_digest, *, deadline, exclude=None, retained_inventory=None):
    root = Path(config['source_root'])
    inputs = {}
    remaining = MAX_INPUT_BYTES
    def read(ref, expected=None, limit=source.MAX_COMMAND_BYTES):
        nonlocal remaining
        _current(config)
        layers._deadline(deadline)
        relative = _ref(ref)
        raw = source._read(root / relative, min(limit, remaining))
        digest = source._digest(raw)[7:]
        if expected is not None and digest != expected:
            raise source.JournalConflict('public-native input changed from its exact byte binding')
        if ref not in inputs:
            remaining -= len(raw)
            if len(inputs) >= MAX_INPUT_FILES:
                raise ValueError('public-native input dependency budget exceeded')
        elif inputs[ref] != digest:
            raise source.JournalConflict('public-native input changed while assembling the package')
        inputs[ref] = digest
        return raw
    def reader(path, limit):
        return read(path.relative_to(root).as_posix(), limit=limit)
    resolver = NativeTextBindingResolver(root, read_bytes=reader)
    for ref in SCHEMAS:
        read(ref)
    authority = _authority(config, resolver, read)
    context = SimpleNamespace(public_root=root, private_root=None, read_bytes=lambda path, limit: source._read(path, limit))
    identities_snapshot = units._identity_snapshot(context, {}, identities=_native_ids(config),
        exclude=exclude, include_private=False)
    if retained_inventory is not None:
        if (not isinstance(retained_inventory, str) or not retained_inventory.startswith('sha256:')
                or len(retained_inventory) != 71 or any(char not in '0123456789abcdef' for char in retained_inventory[7:])):
            raise source.JournalCorruption('retained native identity observation is not an exact digest')
        # The live scan still refuses duplicate owners. Unrelated later native
        # growth does not rewrite the actual first construction observation.
        identities_snapshot = retained_inventory
    # The affirmative exact output gate has completed before opening text.
    original = read(config['source']['ref'], config['source']['sha256'], config['limits']['max_source_bytes'])
    if len(original) != config['source']['byte_size']:
        raise source.JournalConflict('public source byte count differs')
    text = original.decode('utf-8')
    ids, base = config['identities'], Path(config['source_path']).parent
    plan = _public_plan(config, configuration_digest)
    plan_bytes = record_bytes(plan)
    refs = {key: (base / name).as_posix() for key, name in
            (('layer_ref', 'source-text-layer.v1.json'), ('anchor_ref', 'source-anchor.v2.json'),
             ('content_ref', 'content.txt'), ('policy_ref', 'extraction-policy.json'), ('configuration_ref', PLAN_FILE))}
    refs.update(configuration_sha256=source._digest(plan_bytes)[7:], source_ref=config['source']['ref'])
    built = build_public_utf8_layer(source_text=text, source_selector=config['source']['selector'],
        source_media_type=config['source']['media_type'], source_scope=config['source_scope'],
        identities={key: ids[key] for key in ('layer_id', 'anchor_id', 'passage_id', 'provenance_event_id')},
        refs=refs, maker={'maker_type': 'software', 'agent_ref': config['principal_id'],
            'method': PUBLIC_UTF8_POLICY['method'], 'version': '1'}, language=config['language'],
        rights_record_refs=config['rights_record_refs'], publication_authority_refs=[config['publication_authority']])
    content = built['content']
    if (len(content) > config['limits']['max_output_bytes']
            or source._digest(content)[7:] != authority['output_scope']['content_sha256']):
        raise source.JournalConflict('literal range differs from the independently authorized output File')
    files = {'source-text-layer.v1.json': record_bytes(built['layer']),
        'source-anchor.v2.json': record_bytes(built['anchor']), 'content.txt': content,
        'extraction-policy.json': record_bytes(built['policy']), PLAN_FILE: plan_bytes}
    layer_binding = {'schema_version': 'tos_native_text_layer_binding_v1', 'text_layer': {
        'record_ref': refs['layer_ref'], 'record_sha256': source._digest(files['source-text-layer.v1.json'])[7:],
        'layer_id': ids['layer_id'], 'layer_version': 1}, 'source_record_refs': config['source_record_refs']}
    packet = build_public_text_unit_proposal(verified_layer=built['layer'],
        verified_layer_binding=layer_binding, exact_text=content.decode('utf-8'), scope={'start': 0, 'end': len(content.decode('utf-8'))},
        identities={key: ids[key] for key in ('packet_id', 'scheme_id', 'segmentation_id', 'scope_anchor_ref', 'unit_slots', 'gap_anchor_refs')},
        **config['unit_proposal'])
    files[BASENAME] = record_bytes(packet)
    bindings = [{
        'schema_version': 'tos_native_text_unit_binding_v1', 'packet_ref': config['source_path'],
        'packet_sha256': source._digest(files[BASENAME])[7:], 'packet_id': ids['packet_id'], 'packet_version': 1,
        'segmentation_id': ids['segmentation_id'], 'segmentation_version': 1,
        'unit_id': unit['unit_id'], 'unit_version': 1, 'ordered_anchor_refs': unit['ordered_anchor_refs'],
        'text_layer': layer_binding['text_layer'], 'source_record_refs': config['source_record_refs']}
        for unit in packet['units']]
    def overlay(path, limit):
        if path.parent == root / base and path.name in files:
            body = files[path.name]
            if len(body) > limit:
                raise ValueError('public-native proposed output exceeds its native validation budget')
            return body
        return reader(path, limit)
    verifier = NativeTextBindingResolver(root, read_bytes=overlay)
    for binding in bindings:
        summary = verifier.resolve(binding, verify_content=True)
        if not summary['public_content_available']:
            raise PermissionError('public-native output does not have a genuine public source closure')
    for ref in dict.fromkeys(IMPLEMENTATIONS):
        read(ref, limit=source.MAX_SET_BYTES)
    runtime = layers._runtime()
    resolver.snapshot()
    verifier.snapshot()
    captured = {'schema_version': 'tos_public_native_construction_inputs_v1',
        'inputs': inputs, 'identity_inventory': identities_snapshot, 'runtime': runtime,
        'source_files_verified': True, 'original_payload_opened': False,
        'source_authorship_authenticated': False, 'assessment_performed': False}
    files['source-create-inputs.json'] = record_bytes(captured)
    files['native-bindings.json'] = record_bytes({'schema_version': 'tos_native_unit_bindings_v1', 'bindings': bindings})
    dependencies = source._digest(source._canonical(captured))
    subject = source.Record.from_payload(ids['packet_id'], 1, packet)
    return subject, dependencies, files, bindings


class _Store:
    def __init__(self, config):
        self.root = Path(config['source_root'])
        self.recovery = Path(config['recovery_root'])

    def directory(self, path):
        if path.is_relative_to(self.recovery):
            os.close(_open(path, directory=True, private_root=self.recovery))
        elif path.is_relative_to(self.root / 'ToS/source-witnesses'):
            os.close(source._owned_path(path, directory=True))
        else:
            raise PermissionError('public-native storage path leaves its explicit owner roots')

    def read(self, path, limit):
        return (protected_read(path, limit, private_root=self.recovery)
                if path.is_relative_to(self.recovery) else source._read(path, limit))

    def files(self, directory):
        self.directory(directory)
        pins, before = deposit._pins(directory / 'package-pin'), directory.stat()
        result, size = {}, 0
        with os.scandir(directory) as entries:
            for entry in entries:
                if len(result) >= MAX_FILES or not stat.S_ISREG(entry.stat(follow_symlinks=False).st_mode):
                    raise source.JournalCorruption('public-native package contains foreign or excess files')
                raw = self.read(Path(entry.path), min(source.MAX_COMMAND_BYTES, MAX_PACKAGE_BYTES - size))
                size += len(raw)
                result[entry.name] = raw
        after = directory.stat()
        if ((before.st_dev, before.st_ino, before.st_mtime_ns, before.st_ctime_ns)
                != (after.st_dev, after.st_ino, after.st_mtime_ns, after.st_ctime_ns)
                or deposit._pins(directory / 'package-pin') != pins):
            raise source.JournalConflict('public-native package or ancestor changed during verification')
        return result

    def control(self, target, request):
        key = source._digest(source._canonical({'target': str(target), 'command_id': request['command_id']}))[7:]
        return self.recovery / ('.public-native-construction-' + key + '.pending')

    def plan(self, control):
        self.directory(control)
        if layers._bounded_names(control, 2) - {'plan.json', 'output'}:
            raise source.JournalCorruption('public-native control has foreign residue')
        plan = source._json_object(self.read(control / 'plan.json', layers.MAX_CONTROL_BYTES))
        source._keys(plan, {'schema_version', 'target_ref', 'request_digest', 'files'})
        if (plan['schema_version'] != 'tos_native_construction_stage_v1'
                or not isinstance(plan['files'], dict) or not 1 <= len(plan['files']) <= MAX_FILES):
            raise source.JournalCorruption('public-native recovery plan has an incompatible shape')
        files, size = {}, 0
        for name, value in plan['files'].items():
            if (not isinstance(name, str) or Path(name).name != name or name.startswith('.')
                    or not isinstance(value, str)):
                raise source.JournalCorruption('public-native recovery file slot is invalid')
            raw = base64.b64decode(value, validate=True)
            size += len(raw)
            if size > MAX_PACKAGE_BYTES:
                raise source.JournalCorruption('public-native recovery exceeds its byte budget')
            files[name] = raw
        return plan, files


def _verify(files, config, configuration_digest, request, *, deadline, exclude=None):
    source.command_handler(CONFIG).validate_request(request)
    receipt = source._json_object(files[layers.RECEIPT_FILE])
    source._keys(receipt, {'schema_version', 'command_id', 'request_digest', 'principal_id', 'authority_ref',
        'owner_configuration', 'recorded_at', 'source_path', 'source', 'dependencies', 'files', 'grants_admission'})
    source._instant(receipt['recorded_at'])
    if (receipt['schema_version'] != 'tos_local_source_create_receipt_v1'
            or receipt['command_id'] != request['command_id']
            or receipt['request_digest'] != source._digest(source._canonical(request))
            or receipt['principal_id'] != config['principal_id'] or receipt['authority_ref'] != config['authority_ref']
            or receipt['owner_configuration'] != configuration_digest
            or request['expected_configuration'] != configuration_digest
            or request['expected_source'] is not None or request['expected_revision'] is not None
            or receipt['source_path'] != config['source_path']
            or receipt['dependencies'] != request['expected_dependencies'] or receipt['grants_admission'] is not False
            or receipt['files'] != _file_refs({k: v for k, v in files.items() if k != layers.RECEIPT_FILE})
            or source._json_object(files['source-create-request.json']) != request
            or source._json_object(files[PLAN_FILE]) != _public_plan(config, configuration_digest)):
        raise source.JournalConflict('public-native retained receipt differs from its exact request and delegation')
    captured = source._json_object(files['source-create-inputs.json'])
    subject, dependencies, prepared, bindings = _prepare(config, configuration_digest, deadline=deadline, exclude=exclude,
        retained_inventory=captured.get('identity_inventory'))
    if (receipt['source'] != subject.ref or dependencies != receipt['dependencies']
            or any(files.get(name) != raw for name, raw in prepared.items())):
        raise source.JournalConflict('public-native retained output differs from current exact source closure')
    expected_names = {*prepared, 'source-create-request.json', 'source-create-environment.json',
                      'source-create-provenance.jsonl', layers.RECEIPT_FILE}
    if set(files) != expected_names:
        raise source.JournalCorruption('public-native retained package has an unexpected file set')
    event = source._json_object(files['source-create-provenance.jsonl'])
    source._validator_for_provenance(Path(config['source_root'])).validate(event)
    if event['event_id'] != config['identities']['provenance_event_id']:
        raise source.JournalCorruption('public-native execution evidence addresses another event')
    return receipt, bindings


def run_command(owner_config, config, configuration_digest, path, request):
    source.command_handler(CONFIG).validate_request(request)
    deadline = time.monotonic() + config['limits']['max_seconds']
    operation = request['operation']
    store, target = _Store(config), path.parent
    if operation == 'describe':
        return {'status': 'ready', 'configuration': configuration_digest, 'source_path': config['source_path'],
                'operation': OPERATION, 'grants_admission': False, 'external_publication_authorized': False}
    if operation == 'prepare-create':
        subject, dependencies, _, bindings = _prepare(config, configuration_digest, deadline=deadline)
        return {'configuration': configuration_digest, 'dependencies': dependencies, 'source': subject.ref,
                'expected_source': None, 'expected_revision': None, 'native_bindings': bindings, 'grants_admission': False}
    if not isinstance(request.get('command_id'), str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('public-native create/recovery requires an exact bounded command identity')
    if operation == 'inspect-recovery':
        control = store.control(target, request)
        if not os.path.lexists(control):
            return {'status': 'no_retained_control', 'grants_admission': False}
        plan, files = store.plan(control)
        original = source._json_object(files['source-create-request.json'])
        if (original['command_id'] != request['command_id'] or plan['target_ref'] != target.relative_to(store.root).as_posix()
                or plan['request_digest'] != source._digest(source._canonical(original))):
            raise source.JournalConflict('public-native recovery addresses another command or source')
        _verify(files, config, configuration_digest, original, deadline=deadline,
                exclude=target if os.path.lexists(target) else None)
        if os.path.lexists(target):
            committed = store.files(target)
            if committed != files:
                raise source.JournalConflict('public-native installed package differs from its retained plan')
            _verify(committed, config, configuration_digest, original, deadline=deadline, exclude=target)
        return {'status': 'committed' if os.path.lexists(target) else 'retained_plan',
                'resume_operation': OPERATION, 'file_count': len(files), 'grants_admission': False}
    if operation != OPERATION or config['allowed_operations'] != [OPERATION]:
        raise PermissionError('public-native operation is not independently delegated')
    def result(receipt, bindings, *, replay=False):
        return {'status': 'replayed' if replay else 'created', 'source': receipt['source'],
                'source_path': config['source_path'], 'receipt_digest': source._digest(source._canonical(receipt)),
                'native_bindings': bindings, 'grants_admission': False, 'external_publication_authorized': False}
    with source._locked(store.root / 'ToS/source-witnesses/historical-create'), source._locked(store.recovery / 'public-native-create'):
        if os.path.lexists(target):
            files = store.files(target)
            receipt, bindings = _verify(files, config, configuration_digest, request, deadline=deadline, exclude=target)
            return result(receipt, bindings, replay=True)
        started_at, started_ns = datetime.now(timezone.utc).isoformat(), time.perf_counter_ns()
        subject, dependencies, files, bindings = _prepare(config, configuration_digest, deadline=deadline)
        if (request['expected_configuration'] != configuration_digest or request['expected_dependencies'] != dependencies
                or request['expected_source'] is not None or request['expected_revision'] is not None):
            raise source.JournalConflict('public-native command does not match its exact prepared configuration and inputs')
        captured = source._json_object(files['source-create-inputs.json'])
        entities = [{'entity_ref': ref, 'role': 'exact-public-construction-input', 'sha256': digest,
            'size_bytes': len(source._read(store.root / ref, source.MAX_SET_BYTES)),
            'media_type': config['source']['media_type'] if ref == config['source']['ref'] else 'application/octet-stream',
            'availability': 'tracked', 'content_disclosure': 'public_content', 'fixity_verified': True,
            'fixity_verified_at': started_at} for ref, digest in captured['inputs'].items()]
        source._capture_creation_provenance({**config, 'provenance_event_id': config['identities']['provenance_event_id']},
            request, files, started_at, started_ns, procedure_name=PUBLIC_UTF8_POLICY['method'],
            additional_software_refs=(MODULE_REF,), public_native_inputs={
                'entities': entities, 'rights': config['rights_record_refs'], 'authority': [config['publication_authority']]})
        receipt = {'schema_version': 'tos_local_source_create_receipt_v1', 'command_id': request['command_id'],
            'request_digest': source._digest(source._canonical(request)), 'principal_id': config['principal_id'],
            'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
            'recorded_at': datetime.now(timezone.utc).isoformat(), 'source_path': config['source_path'],
            'source': subject.ref, 'dependencies': dependencies, 'files': _file_refs(files), 'grants_admission': False}
        files[layers.RECEIPT_FILE] = record_bytes(receipt)
        def guard():
            layers._deadline(deadline)
            if (source._configuration(owner_config)[1:] != (configuration_digest, path)
                    or _prepare(config, configuration_digest, deadline=deadline)[1] != dependencies):
                raise source.JournalConflict('public-native source, authority or grant changed before commit')
        retained = layers.install_native_package(target, files, request=request, guard=guard,
            verify_retained=lambda stored: _verify(stored, config, configuration_digest, request, deadline=deadline),
            control=store.control(target, request), control_root=store.recovery,
            target_ref=target.relative_to(store.root).as_posix(), ensure_directory=store.directory,
            read_control=store.plan, read_package=store.files, read_file=store.read)
        return result(source._json_object(retained[layers.RECEIPT_FILE]), bindings)


def command_handlers():
    return (contract.Handler('public-project-native-text-create', (CONFIG,), (
        contract.describe(), contract.operation('prepare-create',
            definition='Prepare one independently authorized public project-text range and proposed native partition.', grants=(OPERATION,)),
        contract.operation(OPERATION, contract.COMMIT_KEYS,
            definition='Atomically create a new public native layer and first segmentation with exact replay and retained recovery.',
            mutation='public_native_source_package', grants=(OPERATION,)),
        contract.operation('inspect-recovery', {'command_id'},
            definition='Inspect one exact retained construction plan without replacing or deleting its evidence.')),
        run_command, 'Separate bounded public project-authored UTF-8 source construction; not third-party import or assessment.',
        configure=configuration, typed_handles=(CONFIG_SCHEMA, AUTHORITY_SCHEMA, layers.LAYER_SCHEMA,
            layers.ANCHOR_SCHEMA, units.PACKET_SCHEMA, units.BINDING_SCHEMA),
        preconditions=('Protected public-native grant, exact current source/rights/authority, and new native identities are all required.',
            'Private grants, original Item payload visibility, semantic admission and external deployment remain unchanged.')) ,)

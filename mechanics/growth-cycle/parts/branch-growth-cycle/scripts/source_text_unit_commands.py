"""Delegated confidential native TextUnit creation over an exact source closure.

The request is only a bounded interval partition. It cannot supply a packet,
choose another source, grant access, overwrite a source or admit knowledge.
Existing atomic source packages and receipt envelopes carry immutable output;
no second history registry or semantic Description identity is introduced.
"""
from __future__ import annotations

from datetime import datetime, timezone
import fnmatch
import os
from pathlib import Path
import re
import stat
import tempfile
import time

from jsonschema import Draft202012Validator, FormatChecker

import source_commands as source
import source_command_contracts as contract
from source_owner_context import OwnerLocalSourceContext, _open, _read as context_read
from native_text_binding import NativeTextBindingResolver, check_local_research_rights
from source_revisions import _encode, _file_refs


CONFIG = 'tos_local_text_unit_create_owner_v1'
OPERATION = 'text-unit.create'
PACKET_SCHEMA = 'ToS/contracts/source-text-unit-packet-v1.schema.json'
BINDING_SCHEMA = 'ToS/contracts/native-text-unit-binding.schema.json'
PROVENANCE_SCHEMA = 'ToS/contracts/provenance-event-v2.schema.json'
CONFIG_FILE = 'source-create-owner-configuration.json'
RECEIPT_FILE = 'source-create-receipt.json'
MAX_INVENTORY_FILES = 2048
MAX_INVENTORY_ENTRIES = 32768
MAX_INVENTORY_BYTES = 64 * 1024 * 1024
MAX_INVENTORY_FILE_BYTES = 32 * 1024 * 1024
MAX_INVENTORY_RECORDS = 65536
IMPLEMENTATIONS = (
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_text_unit_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py', contract.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
    'scripts/source_text_unit_proposal.py', 'scripts/native_text_binding.py',
    'scripts/source_owner_context.py', 'scripts/validate_source_witness_foundation.py',
)


def _private_directory(context, path):
    if not path.is_relative_to(context.private_root):
        raise PermissionError('native writer requires the explicit owner-local store')
    os.close(_open(path, directory=True, private_root=context.private_root))


def _interval(value):
    source._keys(value, {'start', 'end'})
    if (any(type(value[key]) is not int for key in ('start', 'end'))
            or not 0 <= value['start'] < value['end']):
        raise ValueError('native text scope requires a nonempty code-point interval')
    return value['start'], value['end']


def _identities(config):
    return {key: config[key] for key in ('packet_id', 'scheme_id', 'segmentation_id',
        'scope_anchor_ref', 'unit_slots', 'gap_anchor_refs')}


def _delegated_ids(config):
    values = [config['packet_id'], config['scheme_id'], config['segmentation_id'],
              config['scope_anchor_ref'], config['provenance_event_id']]
    values.extend(config['gap_anchor_refs'])
    for row in config['unit_slots']:
        values.extend((row['unit_id'], row['anchor_ref']))
    return values


def configuration(config, *, owner_config):
    source._keys(config, {'schema_version', 'uid', 'principal_id', 'authority_ref', 'expires_at',
        'source_context_ref', 'source_path', 'allowed_operations', 'source_binding',
        'source_access', 'allowed_text_scope', 'packet_id', 'scheme_id', 'segmentation_id',
        'scope_anchor_ref', 'unit_slots', 'gap_anchor_refs', 'scheme', 'method', 'provenance_event_id'})
    raw = context_read(Path(owner_config), source.MAX_COMMAND_BYTES, confidential_file=True)
    if source._json_object(raw) != config:
        raise source.JournalConflict('native delegation changed while being selected')
    if (config['schema_version'] != CONFIG or type(config['uid']) is not int
            or config['uid'] != os.getuid() or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref'))
            or config['allowed_operations'] != [OPERATION]):
        raise PermissionError('native creation delegation is invalid or expired')
    access = config['source_access']
    source._keys(access, {'read_scope', 'access_allowed', 'authority_ref'})
    if (access['read_scope'] != 'exact_owner_local' or access['access_allowed'] is not True
            or not isinstance(access['authority_ref'], str) or not access['authority_ref'].strip()):
        raise PermissionError('native construction requires separately delegated exact source reading')
    _interval(config['allowed_text_scope'])
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    path = context.path(config['source_path'])
    if (context.role(config['source_path']) != 'owner-local-root'
            or path.name != 'source-text-unit.v1.json'
            or any(part in {'payload', 'local-content', 'catalog'} or part.startswith('.')
                   for part in Path(config['source_path']).parts)
            or len(Path(config['source_path']).parts) < 7):
        raise PermissionError('native construction requires a new private source package')
    _private_directory(context, path.parent.parent)
    contracts = {ref: context.read_bytes(context.public_root / ref, source.MAX_COMMAND_BYTES)
                 for ref in (PACKET_SCHEMA, BINDING_SCHEMA, PROVENANCE_SCHEMA)}
    schema = source._json_object(contracts[PACKET_SCHEMA])
    method_schema = {'$ref': '#/$defs/method', '$defs': schema['$defs']}
    if (not Draft202012Validator(method_schema, format_checker=FormatChecker()).is_valid(config['method'])
            or not Draft202012Validator(source._json_object(contracts[BINDING_SCHEMA])).is_valid(config['source_binding'])):
        raise ValueError('native delegation does not use the existing method and binding grammar')
    base = Path(config['source_path']).parent
    method = config['method']
    if (method['agent_ref'] != config['principal_id'] or method['maker_kind'] == 'synthetic_fixture'
            or method['provenance_event_ref'] != config['provenance_event_id']
            or method['configuration_ref'] != (base / CONFIG_FILE).as_posix()):
        raise PermissionError('native method must bind its delegated maker and retained configuration')
    source._keys(config['scheme'], {'scheme_name', 'analysis_role', 'boundary_basis', 'policies'})
    slots, gaps = config['unit_slots'], config['gap_anchor_refs']
    if not isinstance(slots, list) or not 1 <= len(slots) <= 256 or not isinstance(gaps, list) or len(gaps) > 257:
        raise ValueError('native creation exceeds its delegated unit or gap count')
    patterns = {'packet_id': 'source-text-unit-packet', 'scheme_id': 'text-unit-scheme',
                'segmentation_id': 'text-segmentation'}
    for key, kind in patterns.items():
        if not isinstance(config[key], str) or not re.fullmatch(r'tos\.' + kind + r'\.sid-[a-f0-9]{32}', config[key]):
            raise ValueError('native creation requires delegated opaque identities')
    for row in slots:
        source._keys(row, {'unit_id', 'anchor_ref', 'unit_kind'})
        if (not isinstance(row['unit_id'], str) or not re.fullmatch(r'tos\.text-unit\.sid-[a-f0-9]{32}', row['unit_id'])
                or row['unit_kind'] not in schema['$defs']['unitKind']['enum']):
            raise ValueError('native unit identity or kind is not supported')
    for identifier in [config['scope_anchor_ref'], *gaps, *(row['anchor_ref'] for row in slots)]:
        if not isinstance(identifier, str) or not re.fullmatch(r'tos\.anchor\.[a-z0-9]+(?:[.-][a-z0-9]+)*', identifier):
            raise ValueError('native anchor identity must be explicit')
    if not isinstance(config['provenance_event_id'], str) or not re.fullmatch(
            r'tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['provenance_event_id']):
        raise ValueError('native provenance identity must be explicit')
    ids = _delegated_ids(config)
    if len(ids) != len(set(ids)):
        raise ValueError('native creation identities must be distinct')
    digest = source._digest(source._canonical({'owner_configuration_bytes': source._digest(raw),
        'context': context.snapshot(), 'contracts': {ref: source._digest(body) for ref, body in contracts.items()}}))
    return config, digest, path


def _inventory_paths(context, *, exclude=None):
    """Bounded native ID-owner metadata only, never payloads or local content."""
    paths, visited = [], 0
    for root, start in ((context.public_root, context.public_root / 'ToS/source-witnesses'),
                        (context.private_root, context.private_root / context.private_prefix)):
        if not start.exists():
            raise ValueError('native identity home is absent')
        pending = [start]
        while pending:
            directory = pending.pop()
            if directory == exclude:
                continue  # Only after exact replay package verification.
            if root == context.private_root:
                _private_directory(context, directory)
            else:
                os.close(source._owned_path(directory, directory=True))
            with os.scandir(directory) as entries:
                for entry in entries:
                    visited += 1
                    if visited > MAX_INVENTORY_ENTRIES:
                        raise ValueError('native identity discovery exceeds its directory-entry budget')
                    if entry.name.startswith('.') or entry.name in {'payload', 'local-content', 'catalog'}:
                        continue
                    path = Path(entry.path)
                    info = entry.stat(follow_symlinks=False)
                    if stat.S_ISDIR(info.st_mode):
                        pending.append(path)
                    elif (fnmatch.fnmatchcase(entry.name, '*source-text-unit*.json')
                          or fnmatch.fnmatchcase(entry.name, '*source-anchor*.json')
                          or fnmatch.fnmatchcase(entry.name, '*anchor*.jsonl')
                          or 'provenance' in entry.name and entry.name.endswith('.jsonl')):
                        if not stat.S_ISREG(info.st_mode):
                            raise PermissionError('native identity metadata must be a regular non-symlink file')
                        paths.append((path.relative_to(root).as_posix(), path))
                        if len(paths) > MAX_INVENTORY_FILES:
                            raise ValueError('native identity file budget exceeded')
                    elif stat.S_ISLNK(info.st_mode):
                        raise PermissionError('native identity discovery does not follow aliases')
    return sorted(paths)


def _identity_snapshot(context, config, *, exclude=None):
    paths = _inventory_paths(context, exclude=exclude)
    remaining, inputs, owned, record_count = MAX_INVENTORY_BYTES, {}, set(), 0
    for ref, path in paths:
        raw = context.read_bytes(path, min(MAX_INVENTORY_FILE_BYTES, remaining))
        remaining -= len(raw)
        inputs[ref] = source._digest(raw)
        records = raw.splitlines() if path.suffix == '.jsonl' else [raw]
        for line in records:
            if not line.strip():
                continue
            record_count += 1
            # Legacy DTA text-unit packets are multi-megabyte JSON objects;
            # individual JSONL anchor/event records retain the smaller bound.
            if ((path.suffix == '.jsonl' and len(line) > source.MAX_COMMAND_BYTES)
                    or record_count > MAX_INVENTORY_RECORDS):
                raise ValueError('native identity record or record-count budget exceeded')
            packet = source._json_object(line)
            if packet.get('schema_version') == 'tos_source_text_unit_packet_v1':
                owned.add(packet.get('packet_id'))
                for group, key in (('schemes', 'scheme_id'), ('anchors', 'anchor_ref'), ('units', 'unit_id'),
                                   ('segmentations', 'segmentation_id')):
                    owned.update(row.get(key) for row in packet.get(group, []))
            elif packet.get('schema_version') in {'tos_source_anchor_v2', 'tos_source_anchor_v1'}:
                owned.add(packet.get('anchor_id'))
            elif 'provenance' in path.name:
                owned.add(packet.get('event_id'))
            else:
                raise ValueError('native identity metadata has an unsupported owner shape')
    if set(_delegated_ids(config)) & owned:
        raise source.JournalConflict('a delegated native identity already has an owner')
    if paths != _inventory_paths(context, exclude=exclude):
        raise source.JournalConflict('native identity membership changed during inspection')
    return source._digest(source._canonical(inputs))


def _local_research_gate(resolver, layer):
    """Retain the writer's gate while sharing its existing exact-layer law."""
    check_local_research_rights(resolver, layer)


def _prepare(config, request, *, exclude=None):
    from source_text_unit_proposal import build_text_unit_proposal
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    resolver = NativeTextBindingResolver(context.public_root, owner_context=context, read_bytes=source._read)
    binding = config['source_binding']
    # Access to bytes and permission to derive from them are different gates.
    resolver.resolve(binding)
    packet = resolver._record(binding['packet_ref'], expected=binding['packet_sha256'])
    layer = resolver._record(binding['text_layer']['record_ref'], expected=binding['text_layer']['record_sha256'])
    _local_research_gate(resolver, layer)
    identity_snapshot = _identity_snapshot(context, config, exclude=exclude)
    summary = resolver.resolve(binding, verify_content=True, allow_private_content=True)
    representation = layer['representation']
    raw = resolver._read(representation['content_ref'], expected=representation['content_sha256'], content=True)
    text = raw.decode('utf-8')
    unit = next(row for row in packet['units'] if row['unit_id'] == binding['unit_id'])
    anchors = {row['anchor_ref']: row['selector'] for row in packet['anchors']}
    intervals = [(anchors[ref]['start'], anchors[ref]['end']) for ref in unit['ordered_anchor_refs']]
    start, end = _interval(config['allowed_text_scope'])
    if (unit['continuity'] != 'contiguous' or not intervals
            or any(left[1] != right[0] for left, right in zip(intervals, intervals[1:]))
            or not intervals[0][0] <= start < end <= intervals[-1][1]):
        raise PermissionError('delegated construction scope leaves the selected contiguous native unit')
    output = build_text_unit_proposal(verified_packet=packet, verified_layer=layer, exact_text=text,
        scope=config['allowed_text_scope'], identities=_identities(config), spans=request['spans'],
        excluded_gaps=request['excluded_gaps'], scheme=config['scheme'], method=config['method'])
    resolver._validate(output, 'source-text-unit-packet-v1.schema.json')
    if (output['source_scope'] != packet['source_scope'] or output['source_layer'] != packet['source_layer']
            or any(row['source_return']['locator_ref'] != representation['content_ref'] for row in output['anchors'])):
        raise ValueError('constructed native proposal changed its verified source closure')
    # Inputs retain raw source bindings only inside the confidential provenance.
    native_inputs = {'rights': representation['rights_record_refs'], 'entities': []}
    for ref, body, role, media in (
            (binding['packet_ref'], resolver._read(binding['packet_ref']), 'verified-native-packet', 'application/json'),
            (binding['text_layer']['record_ref'], resolver._read(binding['text_layer']['record_ref']), 'verified-text-layer', 'application/json'),
            (representation['content_ref'], raw, 'verified-exact-representation', representation['media_type'])):
        native_inputs['entities'].append({'entity_ref': ref, 'role': role, 'sha256': source._digest(body)[7:],
            'size_bytes': len(body), 'media_type': media, 'availability': 'owner_local',
            'content_disclosure': 'private_content', 'fixity_verified': True,
            'fixity_verified_at': datetime.now(timezone.utc).isoformat()})
    dependencies = source._digest(source._canonical({
        'native_snapshot': resolver.snapshot(), 'identity_snapshot': identity_snapshot,
        'implementation': {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES)) for ref in IMPLEMENTATIONS}}))
    subject = source.Record.from_payload(output['packet_id'], output['packet_version'], output)
    files = {Path(config['source_path']).name: _encode(output), CONFIG_FILE: _encode(config)}
    if sum(map(len, files.values())) > 2 * source.MAX_COMMAND_BYTES:
        raise ValueError('native package proposal exceeds its metadata byte budget')
    return subject, files, dependencies, native_inputs, summary


def _package(context, target):
    _private_directory(context, target)
    before = target.stat()
    files, remaining = {}, 8 * source.MAX_COMMAND_BYTES
    for path in sorted(target.iterdir()):
        if len(files) >= 6 or not stat.S_ISREG(path.lstat().st_mode):
            raise source.JournalCorruption('native package must contain only its bounded regular files')
        raw = context.read_bytes(path, min(2 * source.MAX_COMMAND_BYTES, remaining))
        remaining -= len(raw)
        files[path.name] = raw
    after = target.stat()
    if (before.st_ino, before.st_mtime_ns, before.st_ctime_ns) != (after.st_ino, after.st_mtime_ns, after.st_ctime_ns):
        raise source.JournalConflict('native package changed during inspection')
    return files


def _replay(config, configuration_digest, context, path, request, *, owner_config):
    files = _package(context, path.parent)
    original_files = dict(files)
    if set(files) != {path.name, CONFIG_FILE, RECEIPT_FILE, 'source-create-request.json',
                      'source-create-environment.json', 'source-create-provenance.jsonl'}:
        raise source.JournalCorruption('native package has missing or unbound files')
    receipt = source._json_object(files.pop(RECEIPT_FILE))
    expected_keys = {'schema_version', 'command_id', 'request_digest', 'principal_id', 'authority_ref',
        'owner_configuration', 'recorded_at', 'source_path', 'source', 'dependencies', 'files', 'grants_admission'}
    if (set(receipt) != expected_keys or receipt['schema_version'] != 'tos_local_source_create_receipt_v1'
            or receipt['command_id'] != request['command_id'] or receipt['request_digest'] != source._digest(source._canonical(request))
            or receipt['source_path'] != config['source_path']):
        raise source.JournalConflict('native target or command identity is already occupied')
    packet = source._json_object(files[path.name])
    original = source.Record.from_payload(packet['packet_id'], packet['packet_version'], packet)
    source._instant(receipt['recorded_at'])
    if (receipt['owner_configuration'] != configuration_digest or request['expected_configuration'] != configuration_digest
            or request['expected_source'] is not None or request['expected_revision'] is not None
            or receipt['principal_id'] != config['principal_id'] or receipt['authority_ref'] != config['authority_ref']
            or receipt['dependencies'] != request['expected_dependencies'] or receipt['source'] != original.ref
            or receipt['grants_admission'] is not False or receipt['files'] != _file_refs(files)
            or source._json_object(files[CONFIG_FILE]) != config
            or source._json_object(files['source-create-request.json']) != request):
        raise source.JournalCorruption('native creation package no longer binds its exact delegation and request')
    # The exclusion is earned only by byte verification above, never existence.
    subject, expected_files, dependencies, _, _ = _prepare(config, request, exclude=path.parent)
    if (subject.ref != original.ref
            or any(files[name] != body for name, body in expected_files.items())):
        raise source.JournalConflict('native retry no longer reproduces the retained exact proposal')
    # The receipt's opaque dependency digest belongs to its original request,
    # not to every future inventory or implementation. Revalidate current
    # source/rights/collisions and compare two snapshots of THIS retry instead.
    # This does not prove historical equality of unpinned bibliography inputs.
    if (_prepare(config, request, exclude=path.parent)[2] != dependencies
            or _package(context, path.parent) != original_files
            or context.snapshot() != OwnerLocalSourceContext.load(config['source_context_ref']).snapshot()
            or source._configuration(owner_config)[1:] != (configuration_digest, path)):
        raise source.JournalConflict('native source, context, delegation or package changed during replay')
    return receipt


def run_command(owner_config, config, configuration_digest, path, request):
    operation = request.get('operation')
    source.command_handler(config['schema_version']).validate_request(request)
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    target = path.parent
    def result(receipt=None, replayed=False):
        return {'schema_version': 'tos_local_text_unit_create_result_v1', 'authentication': 'local-unix-account',
            'owner_configuration': configuration_digest, 'source_path': config['source_path'],
            'packet_id': config['packet_id'], 'target_exists': os.path.lexists(target),
            'supported_operations': [OPERATION], 'command_operations': ['describe', 'prepare-create', OPERATION],
            'allowed_operations': config['allowed_operations'], 'expected_source': None, 'expected_revision': None,
            'allowed_text_scope': config['allowed_text_scope'], 'unit_slots': config['unit_slots'],
            'gap_anchor_refs': config['gap_anchor_refs'],
            'proposal_fields': {'spans': ['unit_id', 'start', 'end', 'certainty', 'status_reason'],
                                'excluded_gaps': ['anchor_ref', 'start', 'end']},
            'position_unit': 'unicode_code_point', 'interval': 'half_open',
            'record_schema_ref': PACKET_SCHEMA, 'receipt': receipt, 'replayed': replayed,
            'grants_admission': False, 'content_disclosure': 'owner_local_only',
            'replay_input_posture': 'historical_request_current_validation' if replayed else None}
    if operation == 'describe':
        return result()
    if operation == 'prepare-create':
        subject, files, dependencies, _, _ = _prepare(config, request)
        response = result()
        response.update(prepared_source=subject.ref, prepared_files=_file_refs(files), expected_dependencies=dependencies,
            capture_at_apply=['source-create-request.json', 'source-create-environment.json', 'source-create-provenance.jsonl'])
        return response
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('native command identity must contain one to 256 characters')
    # Reuse the public source-owner coordination lock, then the private store
    # lock in fixed order. Neither lock file is authored knowledge.
    with source._locked(context.public_root / 'ToS/source-witnesses/historical-create'), source._locked(context.private_root / 'native-create'):
        os.close(_open(context.private_root / '.native-create.writer.lock', private_root=context.private_root))
        current = source._configuration(owner_config)
        if current[1:] != (configuration_digest, path):
            raise source.JournalConflict('native delegation changed before transaction')
        if os.path.lexists(target):
            return result(_replay(config, configuration_digest, context, path, request,
                                  owner_config=owner_config), True)
        if (request['expected_configuration'] != configuration_digest or request['expected_source'] is not None
                or request['expected_revision'] is not None):
            raise source.JournalConflict('native creation requires exact delegation and absent source/revision')
        started_at, started_ns = datetime.now(timezone.utc).isoformat(), time.perf_counter_ns()
        subject, files, dependencies, native_inputs, _ = _prepare(config, request)
        if request['expected_dependencies'] != dependencies:
            raise source.JournalConflict('prepared native source dependencies are stale')
        source._capture_creation_provenance({**config, 'source_root': str(context.public_root)}, request,
            files, started_at, started_ns, procedure_name='exact-native-text-unit-construction',
            additional_software_refs=IMPLEMENTATIONS[:1] + ('scripts/source_text_unit_proposal.py',), native_inputs=native_inputs)
        receipt = {'schema_version': 'tos_local_source_create_receipt_v1', 'command_id': request['command_id'],
            'request_digest': source._digest(source._canonical(request)), 'principal_id': config['principal_id'],
            'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
            'recorded_at': datetime.now(timezone.utc).isoformat(), 'source_path': config['source_path'],
            'source': subject.ref, 'dependencies': dependencies, 'files': _file_refs(files), 'grants_admission': False}
        files[RECEIPT_FILE] = source._canonical(receipt) + b'\n'
        _private_directory(context, target.parent)
        staging = Path(tempfile.mkdtemp(prefix='.native-create-', suffix='.pending', dir=context.private_root))
        try:
            for name, raw in files.items():
                source._publish(staging / name, raw)
            if (_prepare(config, request)[2] != dependencies or source._configuration(owner_config)[1] != configuration_digest
                    or context.snapshot() != OwnerLocalSourceContext.load(config['source_context_ref']).snapshot()):
                raise source.JournalConflict('native source or delegation changed before publication')
            _private_directory(context, target.parent)
            _private_directory(context, staging)
            for name, raw in files.items():
                if context_read(staging / name, len(raw), private_root=context.private_root) != raw:
                    raise source.JournalConflict('native staging bytes changed before publication')
            source._publish_new_directory(staging, target)
        finally:
            if staging.exists():
                for name in files:
                    (staging / name).unlink(missing_ok=True)
                staging.rmdir()
        return result(receipt)


def command_handlers():
    proposal = {'spans', 'excluded_gaps'}
    return (contract.Handler('owner-local-text-unit-create', (CONFIG,), (contract.describe(),
        contract.operation('prepare-create', proposal, definition='Prepare an explicit interval partition of the independently selected native text closure.', grants=(OPERATION,)),
        contract.operation(OPERATION, proposal | contract.COMMIT_KEYS,
            definition='Create a confidential native TextUnit packet with immutable source and serialization bindings.', mutation='private_text_unit_package', grants=(OPERATION,))),
        run_command, 'Explicit native TextUnit segmentation in the independently selected owner-local store.', configure=configuration,
        typed_handles=(PACKET_SCHEMA, BINDING_SCHEMA, PROVENANCE_SCHEMA),
        profile_selection='The owner selects one exact native binding and bounded unit slots; request spans cannot select another source or executable.',
        preconditions=('Execution requires an independently protected owner-local context, exact read scope and valid local-research rights.',
                       'Discovery does not open that context or reveal its source text, slots or targets.')),)

"""Explicit source-owner commands for forms and initial historical subjects.

The independently selected protected configuration delegates local-account
source writing, not semantic admission. Source prose cannot choose a path,
principal, grant or executable. Access remains read-only.
"""
from __future__ import annotations

from contextlib import contextmanager
import ctypes
from datetime import datetime, timezone
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
import time
from jsonschema import ValidationError

from assessment_journal import (
    JournalBusy, JournalConflict, JournalCorruption, _json_object, _keys,
    _owned_path, _sync_directory,
)
from knowledge_assessment import Record, _canonical, _instant

ROOT = Path(__file__).resolve().parents[5]
if str(ROOT / 'scripts') not in sys.path:
    sys.path.insert(0, str(ROOT / 'scripts'))
from source_witness_human_forms import MAX_SET_BYTES, _validator, materialize_metadata_forms, metadata_field_catalog

OPERATIONS = ('form.create', 'form.revise')
CREATION_OPERATION = 'historical.create'
MAX_COMMAND_BYTES = 1_048_576


def _digest(raw):
    return 'sha256:' + hashlib.sha256(raw).hexdigest()


def _read(path, limit):
    with os.fdopen(_owned_path(path), 'rb') as stream:
        before = os.fstat(stream.fileno())
        raw = stream.read(limit + 1)
        after = os.fstat(stream.fileno())
    if len(raw) > limit:
        raise ValueError('source-command input exceeds its declared byte budget')
    if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        raise JournalConflict('owner file changed during read')
    return raw


def _configuration(path):
    raw = _read(path, MAX_COMMAND_BYTES)
    config = _json_object(raw)
    creation = config.get('schema_version') == 'tos_local_historical_create_owner_v1'
    _keys(config, {'schema_version', 'uid', 'principal_id', 'source_root', 'source_path',
                   'authority_ref', 'allowed_form_ids', 'allowed_operations', 'expires_at'}
          | ({'record_id', 'allowed_claim_ids', 'maker_type'} if creation else set()))
    if (config['schema_version'] not in {'tos_local_source_command_owner_v1', 'tos_local_historical_create_owner_v1'}
            or type(config['uid']) is not int or config['uid'] != os.getuid()
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref'))
            or _instant(config['expires_at']) <= datetime.now(timezone.utc)):
        raise PermissionError('source-command delegation is invalid or expired')
    for key, allowed in (('allowed_operations', (CREATION_OPERATION,) if creation else OPERATIONS), ('allowed_form_ids', None)):
        values = config[key]
        if (not isinstance(values, list) or len(values) > 32
                or any(not isinstance(value, str) for value in values)
                or len(set(values)) != len(values)
                or any(value not in allowed if allowed else not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value)
                       for value in values)):
            raise ValueError('invalid source-command delegation scope')
    if creation:
        if (not isinstance(config['record_id'], str)
                or not re.fullmatch(r'tos\.historical-(event|process|state)\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['record_id'])
                or config['maker_type'] not in {'human', 'software', 'model'}):
            raise ValueError('invalid historical creation identity or maker kind')
        values = config['allowed_claim_ids']
        if (not isinstance(values, list) or len(values) > 32
                or any(not isinstance(value, str) or not re.fullmatch(r'tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*', value) for value in values)
                or len(set(values)) != len(values)):
            raise ValueError('invalid historical claim identity scope')
    root = Path(config['source_root'])
    os.close(_owned_path(root, directory=True))
    relative = Path(config['source_path'])
    if (relative.is_absolute() or relative.as_posix() != config['source_path']
            or '..' in relative.parts or relative.parts[:2] != ('ToS', 'source-witnesses')
            or any(part in ('payload', 'local-content') for part in relative.parts)
            or relative.suffix != '.json' or relative.name.endswith('.human-forms.json')):
        raise PermissionError('source-command target must be explicit source metadata')
    if creation and (relative.name != config['record_id'].split('.')[1] + '.json'
                     or len(relative.parts) < 5):
        raise PermissionError('historical creation requires its typed record in a new subject directory')
    return config, _digest(_canonical(config)), root / relative


def _form_ref(value):
    return Record.from_payload(value['form_id'], value['form_version'], value).ref


def _validate_history(payload):
    """Retained versions must form one complete, nonbranching chain per form."""
    _validator().validate(payload)
    indexed, current, commands = {}, set(), set()
    for form in [*payload['prior_forms'], *payload['forms']]:
        ref = _form_ref(form)
        key = (ref['id'], ref['version'])
        if key in indexed or form['subject']['id'] != payload['subject']['id']:
            raise JournalCorruption('duplicate form version or mixed subject history')
        indexed[key] = form
    for form in payload['forms']:
        if form['form_id'] in current:
            raise JournalCorruption('duplicate current form identity')
        current.add(form['form_id'])
    for (identifier, version), form in indexed.items():
        if version == 1:
            if form['revises'] is not None:
                raise JournalCorruption('initial form has a predecessor')
        else:
            previous = indexed.get((identifier, version - 1))
            if previous is None or form['revises'] != _form_ref(previous):
                raise JournalCorruption('broken retained form lineage')
        if identifier not in current:
            raise JournalCorruption('retained form has no current successor')
    for form in payload['forms']:
        if any(identifier == form['form_id'] and version > form['form_version'] for identifier, version in indexed):
            raise JournalCorruption('current form is not its latest retained version')
    for receipt in payload.get('growth_history', []):
        if receipt['command_id'] in commands or receipt['source']['id'] != payload['subject']['id']:
            raise JournalCorruption('duplicate command or mixed subject receipt')
        commands.add(receipt['command_id'])
        _instant(receipt['recorded_at'])
        for ref in receipt['results']:
            form = indexed.get((ref['id'], ref['version']))
            if form is None or _form_ref(form) != ref:
                raise JournalCorruption('receipt result is absent from retained forms')


def _snapshot(source_path):
    source_raw = _read(source_path, MAX_COMMAND_BYTES)
    source = _json_object(source_raw)
    if source.get('schema_version') not in {'tos_corpus_record_v1', 'tos_historical_record_v1'}:
        raise ValueError('source-command adapter does not understand this source family')
    if (source.get('schema_version') == 'tos_historical_record_v1'
            and source.get('visibility') not in {'public', 'public_metadata_only'}):
        raise PermissionError('historical source visibility is outside the public-metadata adapter')
    subject = Record.from_payload(source['record_id'], source['record_version'], source)
    target = source_path.with_name(source_path.stem + '.human-forms.json')
    try:
        raw = _read(target, MAX_SET_BYTES)
    except FileNotFoundError:
        raw = None
    payload = _json_object(raw) if raw is not None else None
    if payload is not None:
        _validate_history(payload)
        if payload['subject']['id'] != subject.id:
            raise JournalCorruption('source and form set have different subjects')
    return source_raw, source, subject, target, raw, payload


@contextmanager
def _locked(target, timeout=5.0):
    # Stable sibling lock survives atomic replacement of the actual source set.
    os.close(_owned_path(target.parent, directory=True))
    lock_path = target.with_name('.' + target.name + '.writer.lock')
    descriptor = os.open(lock_path, os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    with os.fdopen(descriptor, 'wb') as lock:
        if not stat.S_ISREG(os.fstat(lock.fileno()).st_mode):
            raise PermissionError('source-command lock is not a regular file')
        os.close(_owned_path(lock_path))
        deadline = time.monotonic() + timeout
        while True:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise JournalBusy('source-command writer is busy') from None
                time.sleep(0.01)
        try:
            yield
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def _publish(target, raw):
    descriptor, name = tempfile.mkstemp(prefix='.' + target.name + '.', suffix='.pending', dir=target.parent)
    try:
        with os.fdopen(descriptor, 'wb') as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, target)
        _sync_directory(target.parent)
    finally:
        # Only our unpublished staging file; never source or retained history.
        if os.path.exists(name):
            os.unlink(name)


def _changes(request, config):
    changes = request['changes']
    if not isinstance(changes, list) or not 1 <= len(changes) <= 32:
        raise ValueError('one to thirty-two source changes are required')
    identifiers = set()
    for change in changes:
        _keys(change, {'operation', 'expected_form', 'form'})
        form = change['form']
        if not isinstance(form, dict):
            raise ValueError('form must be an object')
        identifier = form.get('form_id')
        if (change['operation'] not in config['allowed_operations']
                or identifier not in config['allowed_form_ids']
                or form.get('creator_id') != config['principal_id']):
            raise PermissionError('change is outside delegated operation, identity or creator scope')
        if identifier in identifiers:
            raise ValueError('a batch must change each form identity once')
        identifiers.add(identifier)
    return changes


def _apply(payload, subject, changes):
    value = json.loads(_canonical(payload)) if payload else {
        'schema_version': 'tos_human_form_set_v1', 'subject': subject.ref, 'forms': [], 'prior_forms': []}
    current = {form['form_id']: index for index, form in enumerate(value['forms'])}
    for change in changes:
        form = change['form']
        if form.get('subject') != subject.ref:
            raise JournalConflict('new form must bind the exact current subject')
        index = current.get(form['form_id'])
        if change['operation'] == 'form.create':
            if index is not None or change['expected_form'] is not None or form.get('form_version') != 1 or form.get('revises') is not None:
                raise JournalConflict('create requires a new identity and initial version')
            value['forms'].append(form)
        else:
            if index is None:
                raise JournalConflict('revise requires an existing current form')
            old = value['forms'][index]
            previous = _form_ref(old)
            if (change['expected_form'] != previous or form.get('revises') != previous
                    or form.get('form_version') != old['form_version'] + 1):
                raise JournalConflict('revision must extend the exact current form')
            value['prior_forms'].append(old)
            value['forms'][index] = form
    # Do not mark old forms current against a new subject by rewriting their refs.
    # Each old form retains its exact subject and becomes stale in the reader.
    value['subject'] = subject.ref
    _validate_history(value)
    return value


def prepare_metadata_change(source, payload, principal_id, form_id, field_id):
    """Construct a proposal from the reader's finite field catalog; grant nothing."""
    subject = Record.from_payload(source['record_id'], source['record_version'], source)
    field = next((field for field in metadata_field_catalog(source) if field['field_id'] == field_id), None)
    if field is None:
        raise ValueError('unknown metadata field selector')
    old = next((form for form in payload['forms'] if form['form_id'] == form_id), None) if payload else None
    selected_operation = 'form.revise' if old else 'form.create'
    prior = _form_ref(old) if old else None
    form = {'schema_version': 'tos_human_form_v1', 'form_id': form_id,
            'form_version': old['form_version'] + 1 if old else 1, 'subject': subject.ref,
            'role': field['role'], 'language': field['language'], 'script': field['script'],
            'creator_id': principal_id, 'revises': prior,
            'bindings': {'wording': {'record': subject.ref, 'pointer': field['pointer']},
                         **{f'context-{index}': {'record': subject.ref, 'pointer': pointer}
                            for index, pointer in enumerate(field['context'])}},
            'content': {'kind': 'source-copy', 'slot': 'wording'}}
    return {'operation': selected_operation, 'expected_form': prior, 'form': form}


def _historical_scope(config, request):
    """Current delegated IDs and maker apply even to an exact old retry."""
    source, claims, selections = request['record'], request['claims'], request['forms']
    if not isinstance(source, dict) or source.get('record_id') != config['record_id']:
        raise PermissionError('historical subject identity is not delegated')
    if not isinstance(claims, list) or len(claims) > 32:
        raise ValueError('at most thirty-two initial claims are supported')
    if not isinstance(selections, list) or not 1 <= len(selections) <= 32:
        raise ValueError('one to thirty-two source-copy selections are required')
    for claim in claims:
        if (not isinstance(claim, dict) or claim.get('claim_id') not in config['allowed_claim_ids']
                or not isinstance(claim.get('maker'), dict)
                or claim['maker'].get('agent_ref') != config['principal_id']
                or claim['maker'].get('maker_type') != config['maker_type']):
            raise PermissionError('historical claim identity or maker is not delegated')
    for selection in selections:
        _keys(selection, {'form_id', 'field_id'})
        if selection['form_id'] not in config['allowed_form_ids']:
            raise PermissionError('form identity is not delegated')


def _initial_historical_record(config, source):
    from source_witness_bibliographic_graph_common import historical_schema_validator
    historical_schema_validator(Path(config['source_root'])).validate(source)
    if (source['record_id'] != config['record_id'] or source['record_version'] != 1
            or source.get('supersedes_ref') is not None or source['identity_status'] != 'provisional'
            or source['same_as_posture'] != 'no_equivalence_claim'
            or source['visibility'] not in {'public', 'public_metadata_only'}):
        raise PermissionError('creation requires a delegated provisional public-metadata identity')
    return Record.from_payload(source['record_id'], 1, source)


def _historical_creation(config, request):
    """Use authored catalogs and the existing historical reader's contracts."""
    from build_source_witness_catalog import collect_records, collect_claims
    from source_witness_bibliographic_graph_common import (
        _historical_claim_contract, _validate_historical_claim,
        _scan_index, _evidence_node, BibliographicGraphBuildError,
    )
    root = Path(config['source_root'])
    source, claims, selections = request['record'], request['claims'], request['forms']
    subject = _initial_historical_record(config, source)
    records = collect_records(root)
    existing_claims = collect_claims(root)
    objects = {row['record_id']: row for rows in records.values() for row in rows}
    if source['record_id'] in objects:
        raise JournalConflict('subject identity already exists in authored sources')
    form_inputs = {}
    new_form_ids = {selection['form_id'] for selection in selections}
    for row in objects.values():
        path = root / row['source_record_ref']
        adjacent = path.with_name(path.stem + '.human-forms.json')
        if not adjacent.exists():
            continue
        raw = _read(adjacent, MAX_SET_BYTES)
        forms = _json_object(raw)
        _validate_history(forms)
        if new_form_ids.intersection(form['form_id'] for form in [*forms['forms'], *forms['prior_forms']]):
            raise JournalConflict('form identity already exists on another subject')
        form_inputs[adjacent.relative_to(root).as_posix()] = _digest(raw)
    objects[source['record_id']] = {'record_type': source['record_type'],
        'source_record_ref': config['source_path'], 'record_sha256': _digest(_canonical(source))[7:]}
    claim_ids = {claim['claim_id'] for claim in existing_claims}
    events = _scan_index(root, filename_pattern='*provenance*.jsonl', id_field='event_id')
    anchors = _scan_index(root, filename_pattern='*anchor*.jsonl', id_field='anchor_id')
    contract = _historical_claim_contract(root)
    evidence = []
    for claim in claims:
        contract[0].validate(claim)
        if (claim['claim_id'] not in config['allowed_claim_ids']
                or claim['subject_ref'] != source['record_id']
                or claim['maker']['agent_ref'] != config['principal_id']
                or claim['maker']['maker_type'] != config['maker_type']
                or claim['visibility'] not in {'public', 'public_metadata_only'}
                or claim['claim_version'] != 1 or claim.get('assessment_refs')
                or claim.get('supersedes_claim_ref') is not None):
            raise PermissionError('claim exceeds initial identity, maker, visibility or assessment scope')
        if claim['claim_id'] in claim_ids:
            raise JournalConflict('claim identity already exists or repeats in the batch')
        claim_ids.add(claim['claim_id'])
        try:
            _validate_historical_claim(claim, objects, contract)
            if claim['provenance_event_ref'] not in events:
                raise ValueError('claim must refer to an existing provenance event')
            for ref in [*claim['evidence_refs'], *claim.get('counterevidence_refs', [])]:
                if ref.startswith('ToS/'):
                    relative = Path(ref)
                    if relative.is_absolute() or '..' in relative.parts or any(part in {'payload', 'local-content'} for part in relative.parts):
                        raise PermissionError('evidence must address owned metadata, not payload paths')
                    os.close(_owned_path(root / relative))
                evidence.append(_evidence_node(ref, repo_root=root, anchors=anchors, objects=objects, events=events))
        except BibliographicGraphBuildError as error:
            raise ValueError('historical claim does not satisfy the existing graph reader') from error
    for claim in claims:
        if any(ref not in claim_ids for ref in claim.get('alternative_claim_refs', [])):
            raise ValueError('alternative claim must resolve in authored sources or this batch')
    changes, seen_forms = [], set()
    for selection in selections:
        _keys(selection, {'form_id', 'field_id'})
        if selection['form_id'] not in config['allowed_form_ids']:
            raise PermissionError('form identity is not delegated')
        if selection['form_id'] in seen_forms:
            raise ValueError('duplicate form identity')
        seen_forms.add(selection['form_id'])
        changes.append(prepare_metadata_change(source, None, config['principal_id'], **selection))
    forms = _apply(None, subject, changes)
    views = materialize_metadata_forms(source, forms, access_allowed=True)
    if not all(view['state'] == 'ready' for view in views):
        raise ValueError('initial forms must satisfy the current source-copy reader')
    if not any(view['role'] == 'name' for view in views):
        raise ValueError('a new subject requires a source-bound name form')
    encode = lambda value: (json.dumps(value, ensure_ascii=False, allow_nan=False, indent=2) + '\n').encode()
    filename = Path(config['source_path']).name
    files = {filename: encode(source), filename[:-5] + '.human-forms.json': encode(forms),
             'historical-claims.jsonl': b''.join(_canonical(claim) + b'\n' for claim in claims)}
    if any(len(raw) > MAX_SET_BYTES for raw in files.values()):
        raise ValueError('initial source file exceeds its byte budget')
    dependencies = _digest(_canonical({'records': records, 'claims': existing_claims,
        'events': events, 'anchors': anchors, 'evidence': evidence, 'forms': form_inputs,
        'contracts': {ref: _digest(_read(root / ref, MAX_SET_BYTES)) for ref in (
            'ToS/contracts/historical-record.schema.json', 'ToS/contracts/corpus-record.schema.json',
            'ToS/contracts/historical-claim.schema.json', 'ToS/contracts/claim-packet.schema.json',
            'ToS/contracts/knowledge-assessment.schema.json',
            'ToS/doctrine/semantic-interchange/entity-types.v1.json',
            'ToS/doctrine/semantic-interchange/relation-types.v1.json')},
        'implementation': {ref: _digest(_read(ROOT / ref, MAX_SET_BYTES)) for ref in (
            'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
            'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
            'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
            'scripts/source_witness_human_forms.py', 'scripts/build_source_witness_catalog.py',
            'scripts/source_witness_bibliographic_graph_common.py',
            'ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
            'ToS/contracts/human-form-template.schema.json')}}))
    return subject, files, dependencies


def _publish_new_directory(staging, target):
    """Linux no-replace rename: even an empty competing directory must survive."""
    libc = ctypes.CDLL(None, use_errno=True)
    try:
        rename = libc.renameat2
    except AttributeError:
        raise OSError(errno.ENOSYS, 'atomic no-replace directory creation is unavailable') from None
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(-100, os.fsencode(staging), -100, os.fsencode(target), 1) != 0:
        code = ctypes.get_errno()
        if code == errno.EEXIST:
            raise JournalConflict('subject directory was concurrently created')
        raise OSError(code, os.strerror(code))
    _sync_directory(target.parent)
    _sync_directory(staging.parent)


def _create_historical(owner_config, config, configuration, source_path, request):
    fields = {'schema_version', 'operation'}
    if request.get('operation') == CREATION_OPERATION:
        fields |= {'command_id', 'expected_configuration', 'expected_source', 'expected_revision',
                   'expected_dependencies', 'record', 'claims', 'forms'}
    elif request.get('operation') == 'prepare':
        fields |= {'record'}
    elif request.get('operation') == 'prepare-create':
        fields |= {'record', 'claims', 'forms'}
    elif request.get('operation') != 'describe':
        raise ValueError('unsupported historical source command')
    _keys(request, fields)
    if request['schema_version'] != 'tos_local_source_command_v1':
        raise ValueError('unknown source command version')
    root, target = Path(config['source_root']), source_path.parent
    os.close(_owned_path(target.parent, directory=True))
    def result(receipt=None, replayed=False):
        return {'schema_version': 'tos_local_historical_create_result_v1',
            'authentication': 'local-unix-account', 'owner_configuration': configuration,
            'source_path': config['source_path'], 'record_id': config['record_id'],
            'target_exists': target.exists(), 'supported_operations': [CREATION_OPERATION],
            'command_operations': ['describe', 'prepare', 'prepare-create', CREATION_OPERATION],
            'allowed_operations': config['allowed_operations'], 'allowed_claim_ids': config['allowed_claim_ids'],
            'allowed_form_ids': config['allowed_form_ids'], 'expected_source': None, 'expected_revision': None,
            'record_schema_ref': 'ToS/contracts/historical-record.schema.json',
            'claim_schema_ref': 'ToS/contracts/historical-claim.schema.json',
            'receipt': receipt, 'replayed': replayed, 'grants_admission': False}
    if request['operation'] == 'describe':
        return result()
    if CREATION_OPERATION not in config['allowed_operations']:
        raise PermissionError('historical creation is not delegated')
    if request['operation'] == 'prepare':
        subject = _initial_historical_record(config, request['record'])
        response = result()
        response['prepared_source'] = subject.ref
        response['source_fields'] = [{key: value for key, value in field.items() if key not in ('pointer', 'context')}
                                    for field in metadata_field_catalog(request['record'])]
        return response
    _historical_scope(config, request)
    if request['operation'] == 'prepare-create':
        subject, files, dependencies = _historical_creation(config, request)
        response = result()
        response.update(prepared_source=subject.ref, expected_dependencies=dependencies,
                        prepared_files={name: {'sha256': _digest(raw), 'bytes': len(raw)} for name, raw in files.items()})
        return response
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('command identity must contain one to 256 characters')
    request_digest = _digest(_canonical(request))
    # This lock coordinates new identities across all cooperating create callers.
    with _locked(root / 'ToS/source-witnesses/historical-create'):
        _, current_digest, current_path = _configuration(owner_config)
        if current_path != source_path or current_digest != configuration:
            raise JournalConflict('creation delegation changed before transaction')
        receipt_path = target / 'source-create-receipt.json'
        if target.exists() or target.is_symlink():
            os.close(_owned_path(target, directory=True))
            try:
                receipt = _json_object(_read(receipt_path, MAX_COMMAND_BYTES))
            except FileNotFoundError:
                raise JournalConflict('existing subject directory is not this creation') from None
            if (receipt.get('command_id') != request['command_id'] or receipt.get('request_digest') != request_digest
                    or receipt.get('source_path') != config['source_path']):
                raise JournalConflict('creation target or command identity is already occupied')
            if _snapshot(source_path)[2].id != config['record_id']:
                raise JournalCorruption('created subject identity has been replaced')
            return result(receipt, True)
        if (request['expected_configuration'] != configuration or request['expected_source'] is not None
                or request['expected_revision'] is not None):
            raise JournalConflict('creation requires exact delegation and absent source/revision')
        subject, files, dependencies = _historical_creation(config, request)
        if request['expected_dependencies'] != dependencies:
            raise JournalConflict('prepared creation dependencies are stale')
        receipt = {'schema_version': 'tos_local_historical_create_receipt_v1',
            'command_id': request['command_id'], 'request_digest': request_digest,
            'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
            'owner_configuration': configuration, 'recorded_at': datetime.now(timezone.utc).isoformat(),
            'source_path': config['source_path'], 'source': subject.ref, 'dependencies': dependencies,
            'files': {name: {'sha256': _digest(raw), 'bytes': len(raw)} for name, raw in files.items()},
            'grants_admission': False}
        files['source-create-receipt.json'] = _canonical(receipt) + b'\n'
        # Outside source-witnesses, so every existing scanner sees either the
        # complete published directory or nothing, including after process loss.
        staging = Path(tempfile.mkdtemp(prefix='.source-create-', suffix='.pending', dir=root / 'ToS'))
        try:
            for name, raw in files.items():
                _publish(staging / name, raw)
            current_dependencies = _historical_creation(config, request)[2]
            if (_configuration(owner_config)[1] != configuration or current_dependencies != dependencies):
                raise JournalConflict('creation configuration or source dependencies changed')
            _publish_new_directory(staging, target)
        finally:
            if staging.exists():
                # Exact private staging directory and only this call's files.
                for name in files:
                    (staging / name).unlink(missing_ok=True)
                staging.rmdir()
        return result(receipt)


def run_local_command(owner_config: Path, request: dict):
    """One selected owner route: form-set change or no-replace subject creation."""
    if not isinstance(request, dict) or len(_canonical(request)) > MAX_COMMAND_BYTES:
        raise ValueError('source command exceeds the 1 MiB input budget')
    request = _json_object(_canonical(request))  # Freeze caller-owned mutable input.
    config, configuration, source_path = _configuration(owner_config)
    if config['schema_version'] == 'tos_local_historical_create_owner_v1':
        return _create_historical(owner_config, config, configuration, source_path, request)
    operation = request.get('operation')
    fields = {'schema_version', 'operation'}
    if operation == 'apply':
        fields |= {'command_id', 'expected_source', 'expected_revision', 'expected_configuration', 'changes'}
    elif operation == 'prepare':
        fields |= {'field_id', 'form_id'}
    elif operation != 'describe':
        raise ValueError('unknown source command')
    _keys(request, fields)
    if request['schema_version'] != 'tos_local_source_command_v1':
        raise ValueError('unknown source command version')
    snapshot = _snapshot(source_path)

    def result(snapshot, receipt=None, replayed=False):
        _, source, subject, target, raw, payload = snapshot
        return {'schema_version': 'tos_local_source_command_result_v1',
                'authentication': 'local-unix-account', 'owner_configuration': configuration,
                'source': subject.ref, 'source_path': config['source_path'],
                'target_path': target.relative_to(Path(config['source_root'])).as_posix(),
                'revision': _digest(raw) if raw is not None else None,
                'supported_operations': list(OPERATIONS), 'allowed_operations': config['allowed_operations'],
                'command_operations': ['describe', 'prepare', 'apply'],
                'source_fields': [{key: value for key, value in field.items() if key not in ('pointer', 'context')}
                                  for field in metadata_field_catalog(source)],
                'allowed_form_ids': config['allowed_form_ids'],
                'forms': [_form_ref(form) for form in payload['forms']] if payload else [],
                'materializations': materialize_metadata_forms(source, payload, access_allowed=True) if payload else [],
                'receipt': receipt, 'replayed': replayed, 'grants_admission': False}

    if operation == 'describe':
        return result(snapshot)
    if operation == 'prepare':
        source_raw, source, subject, target, raw, payload = snapshot
        if request['form_id'] not in config['allowed_form_ids']:
            raise PermissionError('prepared form is outside the delegated identity scope')
        change = prepare_metadata_change(source, payload, config['principal_id'], request['form_id'], request['field_id'])
        if change['operation'] not in config['allowed_operations']:
            raise PermissionError('prepared operation is not delegated')
        response = result(snapshot)
        response['prepared_change'] = change
        return response
    changes = _changes(request, config)
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('command identity must contain one to 256 characters')
    request_digest = _digest(_canonical(request))
    with _locked(snapshot[3]):
        config, configuration, current_source_path = _configuration(owner_config)
        if current_source_path != source_path:
            raise JournalConflict('owner source route changed before the transaction')
        changes = _changes(request, config)  # Current revocation also applies to replay.
        snapshot = _snapshot(source_path)
        source_raw, source, subject, target, raw, payload = snapshot
        for receipt in payload.get('growth_history', []) if payload else []:
            if receipt['command_id'] == request['command_id']:
                if receipt['request_digest'] != request_digest:
                    raise JournalConflict('command identity was reused for different input')
                return result(snapshot, receipt, True)
        if (request['expected_source'] != subject.ref or request['expected_configuration'] != configuration
                or request['expected_revision'] != (_digest(raw) if raw is not None else None)):
            raise JournalConflict('expected source, configuration or form-set revision is stale')
        value = _apply(payload, subject, changes)
        # Source-copy must satisfy the real existing reader, including its
        # independently selected mandatory context. Other modes are stored as
        # unassessed source proposals, never rendered by this metadata adapter.
        views = {view['form']['id']: view for view in materialize_metadata_forms(source, value, access_allowed=True)}
        for change in changes:
            if change['form']['content']['kind'] == 'source-copy' and views[change['form']['form_id']]['state'] != 'ready':
                raise ValueError('source-copy does not satisfy the source metadata reader')
        receipt = {'command_id': request['command_id'], 'request_digest': request_digest,
                   'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
                   'owner_configuration': configuration, 'recorded_at': datetime.now(timezone.utc).isoformat(),
                   'source': subject.ref, 'previous_revision': request['expected_revision'],
                   'results': [_form_ref(change['form']) for change in changes]}
        value.setdefault('growth_history', []).append(receipt)
        _validate_history(value)
        encoded = (json.dumps(value, ensure_ascii=False, allow_nan=False, indent=2) + '\n').encode()
        if len(encoded) > MAX_SET_BYTES:
            raise ValueError('source-command history exceeds the 2 MiB form-set budget')
        # Cooperating command writers hold the same lock. Editors/configuration
        # publishers must keep their files stable for the command duration.
        if _configuration(owner_config)[1] != configuration or _read(source_path, MAX_COMMAND_BYTES) != source_raw:
            raise JournalConflict('owner configuration or source changed before publication')
        try:
            latest = _read(target, MAX_SET_BYTES)
        except FileNotFoundError:
            latest = None
        if latest != raw:
            raise JournalConflict('form set changed outside the command lock')
        _publish(target, encoded)
        return result((source_raw, source, subject, target, encoded, value), receipt)


def main():
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--owner-config', type=Path, required=True)
    args = parser.parse_args()
    try:
        raw = sys.stdin.buffer.read(MAX_COMMAND_BYTES + 1)
        if len(raw) > MAX_COMMAND_BYTES:
            raise ValueError('source command exceeds the stdin budget')
        response = run_local_command(args.owner_config, _json_object(raw))
    except (ValueError, KeyError, TypeError, OSError, ValidationError, RuntimeError) as error:
        print(json.dumps({'schema_version': 'tos_local_source_command_error_v1', 'error': type(error).__name__}))
        return 2
    print(json.dumps(response, ensure_ascii=False, allow_nan=False))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

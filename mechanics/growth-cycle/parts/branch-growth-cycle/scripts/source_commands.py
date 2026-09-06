"""Explicit source-owner commands; first adapter is adjacent metadata forms.

The independently selected protected configuration delegates local-account
source writing, not semantic admission. Source prose cannot choose a path,
principal, grant or executable. Access remains read-only.
"""
from __future__ import annotations

from contextlib import contextmanager
from datetime import datetime, timezone
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
    _keys(config, {'schema_version', 'uid', 'principal_id', 'source_root', 'source_path',
                   'authority_ref', 'allowed_form_ids', 'allowed_operations', 'expires_at'})
    if (config['schema_version'] != 'tos_local_source_command_owner_v1'
            or type(config['uid']) is not int or config['uid'] != os.getuid()
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref'))
            or _instant(config['expires_at']) <= datetime.now(timezone.utc)):
        raise PermissionError('source-command delegation is invalid or expired')
    for key, allowed in (('allowed_operations', OPERATIONS), ('allowed_form_ids', None)):
        values = config[key]
        if (not isinstance(values, list) or len(values) > 32
                or any(not isinstance(value, str) for value in values)
                or len(set(values)) != len(values)
                or any(value not in allowed if allowed else not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value)
                       for value in values)):
            raise ValueError('invalid source-command delegation scope')
    root = Path(config['source_root'])
    os.close(_owned_path(root, directory=True))
    relative = Path(config['source_path'])
    if (relative.is_absolute() or relative.as_posix() != config['source_path']
            or '..' in relative.parts or relative.parts[:2] != ('ToS', 'source-witnesses')
            or any(part in ('payload', 'local-content') for part in relative.parts)
            or relative.suffix != '.json' or relative.name.endswith('.human-forms.json')):
        raise PermissionError('source-command target must be explicit source metadata')
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


def run_local_command(owner_config: Path, request: dict):
    """One local account, one selected source, one atomic form-set transaction."""
    if not isinstance(request, dict) or len(_canonical(request)) > MAX_COMMAND_BYTES:
        raise ValueError('source command exceeds the 1 MiB input budget')
    config, configuration, source_path = _configuration(owner_config)
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
    except (ValueError, KeyError, TypeError, OSError, ValidationError) as error:
        print(json.dumps({'schema_version': 'tos_local_source_command_error_v1', 'error': type(error).__name__}))
        return 2
    print(json.dumps(response, ensure_ascii=False, allow_nan=False))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

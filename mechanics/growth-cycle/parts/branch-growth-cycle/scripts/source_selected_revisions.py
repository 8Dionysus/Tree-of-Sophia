"""Explicit v2 selected-file correction, including nested native source homes.

The old flat-directory grant and archive meaning are not widened. Publication
uses a cooperating-reader barrier and recoverable exact-file transaction; it
does not claim atomic filesystem visibility to arbitrary raw-file readers.
"""
from datetime import datetime, timezone
from pathlib import Path
import re

import source_commands as source
import source_revisions as revisions
from source_metadata_snapshot import PublicationSnapshot
import source_metadata_transactions as transactions

AUTHORIZATION = 'tos_selected_metadata_revision_authorization_v1'
RECOVERY = 'tos_selected_metadata_recovery_authorization_v1'


def _request(request):
    source._keys(request, {'schema_version', 'operation', 'fields', 'forms', 'reason', 'command_id',
        'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies',
        'expected_publication'})
    if (request['schema_version'] != 'tos_local_source_command_v1' or request['operation'] != 'record.revise'
            or not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256
            or len(source._canonical(request)) > source.MAX_COMMAND_BYTES
            or request['expected_publication'] is not None and (
                not isinstance(request['expected_publication'], str)
                or not re.fullmatch(r'sha256:[a-f0-9]{64}', request['expected_publication']))):
        raise ValueError('invalid selected metadata revision request')


def _transaction_id(request):
    return source._digest(source._canonical({key: request[key] for key in
        ('command_id', 'expected_configuration')}) + source._canonical(request))


def _authorization(config, request):
    return {'schema_version': AUTHORIZATION, 'principal_id': config['principal_id'],
            'authority_ref': config['authority_ref'], 'source_path': config['source_path'],
            'record_id': config['record_id'], 'record_type': config['record_type'],
            'request': request}


def _inspect(config, path):
    files = revisions._selected_package(path)
    record = source._json_object(files[path.name])
    revisions._validate_record(config, record)
    subject = source.Record.from_payload(record['record_id'], record['record_version'], record)
    return files, record, subject, revisions._history(files, record)


def _result(config, configuration, path, snapshot, *, receipt=None, replayed=False, recovery=None):
    files, record, subject, _ = _inspect(config, path)
    formname = path.stem + '.human-forms.json'
    forms = source._json_object(files[formname]) if formname in files else None
    result = {'schema_version': 'tos_local_source_revision_result_v2', 'authentication': 'local-unix-account',
        'owner_configuration': configuration, 'source_path': config['source_path'], 'source': subject.ref,
        'revision': revisions._revision(files), 'publication_snapshot': snapshot.token,
        'publication_protocol': revisions.SELECTED_PROTOCOL,
        'selected_files': sorted(revisions._selected_names(path)),
        'command_operations': ['describe', 'prepare-revise', 'record.revise', 'record.recover', 'inspect-version'],
        'supported_operations': ['record.revise', 'record.recover'],
        'allowed_operations': config['allowed_operations'], 'allowed_fields': config['allowed_fields'],
        'allowed_form_ids': config['allowed_form_ids'], 'record_type': config['record_type'],
        'source_profile': source._configured_corpus_profile(config), 'receipt': receipt, 'replayed': replayed,
        'recovery': recovery, 'grants_admission': False,
        'materializations': source.materialize_metadata_forms(record, forms, access_allowed=True) if forms else []}
    snapshot.verify_current()
    return result


def _receipt(config, request, subject, proposed, refs, *, recorded_at):
    return {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'reason': request['reason'], 'previous_source': subject.ref, 'source': proposed.ref,
        'previous_revision': request['expected_revision'],
        'archive_path': revisions._archive_path(config, request['expected_revision']).as_posix(),
        'dependencies': request['expected_dependencies'], 'changed_fields': sorted(request['fields']),
        'forms': refs, 'grants_admission': False, 'request': request,
        'publication': {'protocol': revisions.SELECTED_PROTOCOL, 'transaction_id': _transaction_id(request),
                        'selected_files': sorted(revisions._selected_names(Path(config['source_path'])))}}


def _proposal(config, path, files, record, history, request, *, recorded_at, scope_operation='record.revise'):
    if len(history['receipts']) >= revisions.MAX_REVISIONS:
        raise ValueError('selected metadata history capacity reached; retain history and use its archive owner')
    revised, proposed, output, views, refs = revisions._proposal(
        config, path, files, record, request, scope_operation=scope_operation)
    subject = source.Record.from_payload(record['record_id'], record['record_version'], record)
    receipt = _receipt(config, request, subject, proposed, refs, recorded_at=recorded_at)
    output[revisions.HISTORY] = revisions._encode({**history,
        'schema_version': 'tos_source_revision_history_v2', 'receipts': [*history['receipts'], receipt]})
    if (any(len(raw) > source.MAX_SET_BYTES for raw in output.values())
            or sum(map(len, output.values())) > revisions.MAX_PACKAGE_BYTES):
        raise ValueError('selected metadata successor exceeds its byte budget')
    return revised, proposed, output, views, receipt


def _guard(owner, config, configuration, path, authorization, before, receipt, *, recovery=False):
    """Current scope is checked independently of the retained journal prose."""
    request = authorization['request']
    def check(retained, _summary):
        current, digest, current_path = source._configuration(owner)
        if (digest != configuration or current_path != path or current != config
                or retained != authorization
                or any(authorization[key] != current[key] for key in
                       ('principal_id', 'authority_ref', 'source_path', 'record_id', 'record_type'))):
            raise source.JournalConflict('selected correction authority or exact owner route changed')
        revisions._scope(current, request, operation='record.recover' if recovery else 'record.revise')
        if revisions._dependencies(current, before) != request['expected_dependencies']:
            raise source.JournalConflict('selected correction dependencies changed')
        revisions._read_archive(Path(current['source_root']), current, receipt)
        return True
    return check


def _pending_plan(config, path, pending, *, scope_operation='record.revise'):
    """Reconstruct the entire selected delta before permitting any recovery."""
    plan = pending['plan']
    authorization = plan['authorization']
    source._keys(authorization, {'schema_version', 'principal_id', 'authority_ref', 'source_path',
                                'record_id', 'record_type', 'request'})
    if authorization['schema_version'] != AUTHORIZATION:
        raise PermissionError('pending transaction is not this selected revision adapter')
    if any(authorization[key] != config[key] for key in
           ('principal_id', 'authority_ref', 'source_path', 'record_id', 'record_type')):
        raise PermissionError('pending revision lies outside the selected owner scope')
    request = authorization['request']
    _request(request)
    expected = {str(Path(config['source_path']).parent / name) for name in revisions._selected_names(path)}
    if (plan['new_directories'] or len(plan['files']) != len(expected)
            or {item['path'] for item in plan['files']} != expected):
        raise PermissionError('pending revision selects other files or descendants')
    before = {Path(item['path']).name: item['before'] for item in plan['files'] if item['before'] is not None}
    after = {Path(item['path']).name: item['after'] for item in plan['files'] if item['after'] is not None}
    if set(after) != set(revisions._selected_names(path)):
        raise source.JournalCorruption('pending revision must retain all three successor metadata files')
    record = source._json_object(before[path.name])
    revisions._validate_record(config, record)
    history = revisions._history(before, record)
    successor = source._json_object(after[path.name])
    output_history = revisions._history(after, successor)
    if len(output_history['receipts']) != len(history['receipts']) + 1:
        raise source.JournalCorruption('pending correction does not append one retained transition')
    receipt = output_history['receipts'][-1]
    _, _, expected_after, _, expected_receipt = _proposal(config, path, before, record, history, request,
                                                       recorded_at=receipt['recorded_at'],
                                                       scope_operation=scope_operation)
    if (after != expected_after or receipt != expected_receipt
            or revisions._revision(before) != request['expected_revision']
            or source.Record.from_payload(record['record_id'], record['record_version'], record).ref
               != request['expected_source']):
        raise source.JournalCorruption('pending bytes do not reconstruct the delegated exact correction')
    return authorization, record, receipt


def run_selected_revision(owner, config, configuration, path, request):
    root = Path(config['source_root'])
    operation = request.get('operation')
    if operation == 'record.revise':
        _request(request)
    elif operation == 'record.recover':
        source._keys(request, {'schema_version', 'operation', 'transaction_id', 'decision', 'expected_configuration'})
        if (request['schema_version'] != 'tos_local_source_command_v1'
                or request['decision'] not in {'resume', 'rollback'}
                or request['expected_configuration'] != configuration
                or 'record.recover' not in config['allowed_operations']):
            raise PermissionError('recovery requires current explicitly delegated scope and decision')
    else:
        fields = {'schema_version', 'operation'}
        if operation == 'prepare-revise':
            fields |= {'fields', 'forms', 'reason'}
        elif operation == 'inspect-version':
            fields.add('source')
        elif operation != 'describe':
            raise ValueError('unknown selected metadata operation')
        source._keys(request, fields)
        if request['schema_version'] != 'tos_local_source_command_v1':
            raise ValueError('unknown source command version')

    if operation not in {'record.revise', 'record.recover'}:
        snapshot = PublicationSnapshot(root)
        files, record, subject, history = _inspect(config, path)
        result = _result(config, configuration, path, snapshot)
        if operation == 'describe':
            return result
        if operation == 'inspect-version':
            for receipt in history['receipts']:
                if receipt['previous_source'] == request['source']:
                    archived, locations = revisions._read_archive(root, config, receipt)
                    snapshot.verify_current()
                    return {**result, 'record': source._json_object(archived[path.name]),
                            'inspected_source': request['source'], 'files': locations}
            snapshot.verify_current()
            raise source.JournalConflict('requested exact source version is not retained')
        _, proposed, _, views, refs = revisions._proposal(config, path, files, record, request)
        dependencies = revisions._dependencies(config, record)
        snapshot.verify_current()
        return {**result, 'prepared_source': proposed.ref, 'prepared_forms': refs,
                'prepared_materializations': views, 'expected_dependencies': dependencies,
                'expected_publication': snapshot.token}

    # The exact pending adapter is inspected under the existing lock, before
    # trying to parse possibly intermediate source/history files.
    with source._locked(root / 'ToS/source-witnesses/historical-create', allow_pending=True):
        current, digest, current_path = source._configuration(owner)
        if current != config or digest != configuration or current_path != path:
            raise source.JournalConflict('selected revision delegation changed before transaction')
        pending = transactions.read_pending_transaction(root)
        if pending is not None:
            authorization, before, receipt = _pending_plan(config, path, pending,
                scope_operation='record.recover' if operation == 'record.recover' else 'record.revise')
            original = authorization['request']
            recovery = operation == 'record.recover'
            if not recovery and (request != original or configuration != request['expected_configuration']):
                raise source.JournalConflict('only the exact current command or delegated recovery may resume pending work')
            guard = _guard(owner, config, configuration, path, authorization, before, receipt, recovery=recovery)
            identifier = receipt['publication']['transaction_id']
            if recovery and request['transaction_id'] != identifier:
                raise source.JournalConflict('recovery selects another transaction')
            decision = request['decision'] if recovery else 'resume'
            operation_fn = transactions.resume_transaction if decision == 'resume' else transactions.rollback_transaction
            recovery_binding = {'schema_version': RECOVERY, 'principal_id': config['principal_id'],
                'authority_ref': config['authority_ref'], 'owner_configuration': configuration,
                'transaction_id': identifier, 'decision': decision} if recovery else None
            completed = operation_fn(root, authorization_guard=guard, transaction_id=identifier,
                                     **({'recovery_authorization': recovery_binding} if recovery else {}))
            return _result(config, configuration, path, PublicationSnapshot(root),
                           receipt=receipt if decision == 'resume' else None, recovery=completed)
        if operation == 'record.recover':
            raise source.JournalConflict('no exact pending transaction is selected for recovery')
        snapshot = PublicationSnapshot(root)
        revisions._scope(config, request)
        files, record, subject, history = _inspect(config, path)
        for receipt in history['receipts']:
            if receipt['command_id'] == request['command_id']:
                if receipt['request_digest'] != source._digest(source._canonical(request)):
                    raise source.JournalConflict('selected correction command identity was reused')
                revisions._read_archive(root, config, receipt)
                if request['expected_configuration'] != configuration or 'publication' not in receipt:
                    raise source.JournalConflict('selected correction replay has no exact current delegation')
                retained = transactions.inspect_transaction(root, receipt['publication']['transaction_id'])
                if retained['status'] != 'committed':
                    raise source.JournalCorruption('revision history has no committed publication evidence')
                authorization, _, reconstructed = _pending_plan(config, path, retained)
                if authorization != _authorization(config, request) or reconstructed != receipt:
                    raise source.JournalCorruption('committed publication differs from exact correction receipt')
                return _result(config, configuration, path, snapshot, receipt=receipt, replayed=True)
        if (request['expected_configuration'] != configuration or request['expected_source'] != subject.ref
                or request['expected_revision'] != revisions._revision(files)
                or request['expected_dependencies'] != revisions._dependencies(config, record)
                or request['expected_publication'] != snapshot.token):
            raise source.JournalConflict('selected metadata source, publication or dependencies are stale')
        _, _, output, _, receipt = _proposal(config, path, files, record, history, request,
                                             recorded_at=datetime.now(timezone.utc).isoformat())
        revisions._archive(root, config, files, subject, request['expected_revision'])
        authorization = _authorization(config, request)
        plan = {'authorization': authorization, 'new_directories': [], 'files': [
            {'path': str(Path(config['source_path']).parent / name), 'before': files.get(name), 'after': output[name]}
            for name in sorted(revisions._selected_names(path))]}
        guard = _guard(owner, config, configuration, path, authorization, record, receipt)
        transactions.apply_transaction(root, plan, expected_snapshot=snapshot, authorization_guard=guard,
                                       transaction_id=receipt['publication']['transaction_id'])
        return _result(config, configuration, path, PublicationSnapshot(root), receipt=receipt)

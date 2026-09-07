"""Exact Claim correction inside a shared, source-owned metadata package.

The stream and selected source-copy forms move atomically. Source attribution,
assessment, identity changes and publication remain separate owner actions.
"""
from datetime import datetime, timezone
import os
from pathlib import Path
import re

import source_commands as source
import source_revisions as packages
from source_record_profiles import SourceClaimProfiles, SOURCE_CLAIM_BASENAME

HISTORY = 'claim-revision-history.json'
OPERATION = 'claim.revise'
FIELDS = {'qualifiers', 'evidence_refs', 'counterevidence_refs', 'alternative_claim_refs',
          'supporting_quotes', 'epistemic_status', 'confidence'}
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py'


def configuration(config):
    values_allowed = config['schema_version'] == source.CLAIM_VALUE_REVISION_CONFIG
    allowed_fields = FIELDS | ({'object'} if values_allowed else set())
    source._keys(config, {'schema_version', 'uid', 'principal_id', 'source_root', 'source_path',
        'authority_ref', 'expires_at', 'claim_id', 'allowed_operations', 'allowed_fields',
        'allowed_evidence_refs', 'allowed_form_ids'}
        | ({'allowed_object_values', 'allowed_object_refs'} if values_allowed else set()))
    if (config['schema_version'] not in {source.CLAIM_REVISION_CONFIG, source.CLAIM_VALUE_REVISION_CONFIG}
            or type(config['uid']) is not int or config['uid'] != os.getuid()
            or any(not isinstance(config[k], str) or not config[k].strip() for k in ('principal_id', 'authority_ref'))
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
            or not isinstance(config['claim_id'], str)
            or not re.fullmatch(r'tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['claim_id'])):
        raise PermissionError('Claim correction delegation is invalid or expired')
    for key, maximum, allowed in (('allowed_operations', 1, {OPERATION}),
            ('allowed_fields', len(allowed_fields), allowed_fields), ('allowed_evidence_refs', 128, None),
            ('allowed_form_ids', 32, None),
            *([('allowed_object_refs', 128, None)] if values_allowed else [])):
        values = config[key]
        if (not isinstance(values, list) or len(values) > maximum
                or any(not isinstance(v, str) or not v.strip() for v in values)
                or len(set(values)) != len(values) or allowed is not None and set(values) - allowed):
            raise ValueError('invalid bounded Claim correction scope')
    if values_allowed:
        from source_claim_commands import validate_value_scope
        validate_value_scope(config)
    if any(not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', v) for v in config['allowed_form_ids']):
        raise ValueError('invalid delegated Claim form identity')
    root, relative = Path(config['source_root']), Path(config['source_path'])
    os.close(source._owned_path(root, directory=True))
    if (relative.is_absolute() or relative.as_posix() != config['source_path'] or '..' in relative.parts
            or relative.parts[:2] != ('ToS', 'source-witnesses') or len(relative.parts) < 4
            or relative.name != SOURCE_CLAIM_BASENAME
            or any(p in {'catalog', 'payload', 'local-content'} for p in relative.parts)):
        raise PermissionError('Claim correction requires an exact source metadata stream')
    return config, source._digest(source._canonical(config)), root / relative


def _claims(raw):
    if len(raw) > source.MAX_COMMAND_BYTES:
        raise ValueError('Claim correction stream exceeds 1 MiB')
    records = {}
    for line in raw.splitlines():
        if not line.strip():
            continue
        record = source._json_object(line)
        identity = record.get('claim_id')
        if not isinstance(identity, str) or identity in records:
            raise source.JournalCorruption('Claim stream has missing or repeated identities')
        records[identity] = record
    return records


def _subject(record):
    return source.Record.from_payload(record['claim_id'], record['claim_version'], record)


def _advance(record, fields):
    if not isinstance(fields, dict) or not fields or set(fields) - (FIELDS | {'object'}):
        raise PermissionError('Claim correction cannot change identity, endpoints, layer, maker or admission')
    if 'object' in fields and (not isinstance(record.get('object'), dict) or not isinstance(fields['object'], dict)):
        raise PermissionError('Claim correction cannot change an identity endpoint into a value or conversely')
    changes = dict(fields)
    if 'qualifiers' in changes:
        if not isinstance(changes['qualifiers'], dict):
            raise ValueError('qualifier correction must be an explicit field patch')
        changes['qualifiers'] = {**record.get('qualifiers', {}), **changes['qualifiers']}
    revised = {**record, **changes}
    if revised == record:
        raise ValueError('Claim correction must change source content')
    revised['claim_version'] = record['claim_version'] + 1
    return revised


def _replace(raw, revised):
    """Serialize only the selected row; preserve every other byte and row order."""
    _claims(raw)
    output, found = [], False
    for line in raw.splitlines(keepends=True):
        if line.strip() and source._json_object(line).get('claim_id') == revised['claim_id']:
            ending = b'\r\n' if line.endswith(b'\r\n') else b'\n' if line.endswith(b'\n') else b''
            output.append(source._canonical(revised) + ending)
            found = True
        else:
            output.append(line)
    if not found:
        raise source.JournalConflict('selected Claim is absent from the shared stream')
    return b''.join(output)


def _archive_config(config, identity):
    return {**config, 'record_id': identity}


def _read_archive(root, config, receipt):
    identity = receipt['previous_source']['id']
    files, locations = packages._read_archive_files(root, _archive_config(config, identity), receipt)
    previous = _claims(files[SOURCE_CLAIM_BASENAME]).get(identity)
    if previous is None or _subject(previous).ref != receipt['previous_source']:
        raise source.JournalCorruption('archive does not preserve the exact previous Claim')
    if 'request' in receipt and _subject(_advance(previous, receipt['request']['fields'])).ref != receipt['source']:
        raise source.JournalCorruption('retained correction does not produce the recorded Claim successor')
    return files, locations


def _history(files, config):
    history = source._json_object(files[HISTORY]) if HISTORY in files else {
        'schema_version': 'tos_claim_revision_history_v1', 'source_path': config['source_path'], 'receipts': []}
    source._keys(history, {'schema_version', 'source_path', 'receipts'})
    if (history['schema_version'] != 'tos_claim_revision_history_v1' or history['source_path'] != config['source_path']
            or not isinstance(history['receipts'], list) or len(history['receipts']) > packages.MAX_REVISIONS):
        raise source.JournalCorruption('invalid shared Claim correction history')
    if not history['receipts'] and any(record.get('claim_version') != 1
            for record in _claims(files[SOURCE_CLAIM_BASENAME]).values()):
        raise source.JournalCorruption('noninitial Claim stream is missing its correction history')
    expected, commands = None, set()
    for receipt in history['receipts']:
        source._keys(receipt, {'command_id', 'request_digest', 'principal_id', 'authority_ref',
            'owner_configuration', 'recorded_at', 'reason', 'previous_source', 'source',
            'previous_revision', 'archive_path', 'dependencies', 'source_bindings', 'changed_fields', 'forms', 'grants_admission', 'request'})
        request = receipt['request']
        source._instant(receipt['recorded_at'])
        if (not isinstance(receipt['command_id'], str) or receipt['command_id'] in commands
                or request.get('operation') != OPERATION
                or source._digest(source._canonical(request)) != receipt['request_digest']
                or any(receipt[left] != request[right] for left, right in (
                    ('command_id', 'command_id'), ('previous_source', 'expected_source'),
                    ('previous_revision', 'expected_revision'), ('owner_configuration', 'expected_configuration'),
                    ('dependencies', 'expected_dependencies'), ('source_bindings', 'expected_inputs'), ('reason', 'reason')))
                or receipt['changed_fields'] != sorted(request['fields']) or receipt['grants_admission'] is not False
                or receipt['source']['id'] != receipt['previous_source']['id']
                or receipt['source']['version'] != receipt['previous_source']['version'] + 1):
            raise source.JournalCorruption('broken Claim correction receipt')
        archived, _ = _read_archive(Path(config['source_root']), config, receipt)
        before = archived[SOURCE_CLAIM_BASENAME]
        if expected is None and any(record.get('claim_version') != 1 for record in _claims(before).values()):
            raise source.JournalCorruption('Claim correction history is missing its initial stream')
        if expected is not None and before != expected:
            raise source.JournalCorruption('shared stream changed outside its retained correction sequence')
        previous = _claims(before)[receipt['previous_source']['id']]
        expected = _replace(before, _advance(previous, request['fields']))
        commands.add(receipt['command_id'])
    if expected is not None and files[SOURCE_CLAIM_BASENAME] != expected:
        raise source.JournalCorruption('current Claim stream is not its retained revision head')
    return history


def creation_source_files(files, config):
    """Return creation bytes only after verifying the complete correction chain."""
    history = _history(files, config)
    return (_read_archive(Path(config['source_root']), config, history['receipts'][0])[0]
            if history['receipts'] else files)


def _scope(config, request):
    if OPERATION not in config['allowed_operations']:
        raise PermissionError('Claim correction is not delegated')
    fields = request['fields']
    if not isinstance(fields, dict) or not fields or not set(fields) <= set(config['allowed_fields']):
        raise PermissionError('Claim correction fields exceed the delegated scope')
    if 'object' in fields:
        from source_claim_commands import value_is_delegated
        if not value_is_delegated(config, fields['object']):
            raise PermissionError('Claim value correction is not explicitly delegated')
    for name in ('evidence_refs', 'counterevidence_refs'):
        if name in fields and (not isinstance(fields[name], list)
                or any(not isinstance(ref, str) or ref not in config['allowed_evidence_refs'] for ref in fields[name])):
            raise PermissionError('new Claim evidence is not delegated')
    if not isinstance(request['reason'], str) or not 1 <= len(request['reason'].strip()) <= 4096:
        raise ValueError('Claim correction requires a bounded authored reason')
    selections = request['forms']
    if not isinstance(selections, list) or not 1 <= len(selections) <= 32:
        raise ValueError('Claim correction requires bounded source-copy forms')
    seen = set()
    for item in selections:
        source._keys(item, {'form_id', 'field_id'})
        if item['form_id'] not in config['allowed_form_ids'] or item['form_id'] in seen:
            raise PermissionError('Claim form is repeated or outside delegated scope')
        seen.add(item['form_id'])


def _proposal(config, path, files, record, request):
    _scope(config, request)
    revised = _advance(record, request['fields'])
    from source_claim_commands import _ground_claims
    _, grounding, bindings = _ground_claims(config, [revised], initial=False)
    dependencies = source._digest(source._canonical({'grounding': grounding,
        'implementation': {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES)) for ref in
            (MODULE_REF, 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
             'scripts/source_witness_human_forms.py', 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
             'ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
             'ToS/contracts/human-form-template.schema.json')}}))
    formname = source.claim_forms_path(path, config['claim_id']).name
    payload = source._json_object(files[formname]) if formname in files else None
    selected_ids = {item['form_id'] for item in request['forms']}
    if payload is not None:
        source._validate_history(payload)
        if payload['subject']['id'] != config['claim_id'] or {f['form_id'] for f in payload['forms']} - selected_ids:
            raise ValueError('Claim correction must rebind every current form of the selected Claim')
    for name, raw in files.items():
        if name.endswith('.human-forms.json') and name != formname:
            sibling = source._json_object(raw)
            if selected_ids & {f['form_id'] for f in sibling['forms']}:
                raise source.JournalConflict('Claim correction cannot reuse a sibling form identity')
    changes = [source.prepare_claim_change(revised, payload, config['principal_id'], **item) for item in request['forms']]
    value = source._apply(payload, _subject(revised), changes)
    views = source.materialize_claim_forms(revised, value, access_allowed=True)
    if not all(v['state'] == 'ready' for v in views) or not any(v['role'] == 'statement' for v in views):
        raise ValueError('Claim correction forms must be ready source copies including the statement')
    output = {**files, path.name: _replace(files[path.name], revised), formname: packages._encode(value)}
    return revised, output, views, [source._form_ref(c['form']) for c in changes], dependencies, bindings


def run_command(owner, config, configuration_digest, path, request):
    operation = request.get('operation')
    fields = {'schema_version', 'operation'}
    if operation in {OPERATION, 'prepare-revise'}:
        fields |= {'fields', 'forms', 'reason'}
    if operation == OPERATION:
        fields |= {'command_id', 'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies', 'expected_inputs'}
    elif operation == 'inspect-version':
        fields |= {'source'}
    elif operation not in {'describe', 'prepare-revise'}:
        raise ValueError('unsupported Claim correction operation')
    source._keys(request, fields)
    if request['schema_version'] != 'tos_local_source_command_v1':
        raise ValueError('unknown source command version')
    root = Path(config['source_root'])

    def inspect():
        files = packages._package(path.parent)
        records = _claims(files[path.name])
        record = records.get(config['claim_id'])
        if record is None:
            raise source.JournalConflict('delegated Claim is absent')
        profiles = SourceClaimProfiles(root)
        profiles.validate(record)
        return files, record, _history(files, config)

    def result(files, record, receipt=None, replayed=False):
        formname = source.claim_forms_path(path, config['claim_id']).name
        payload = source._json_object(files[formname]) if formname in files else None
        return {'schema_version': 'tos_local_claim_revision_result_v1', 'authentication': 'local-unix-account',
            'owner_configuration': configuration_digest, 'source_path': config['source_path'], 'source': _subject(record).ref,
            'revision': packages._revision(files), 'command_operations': ['describe', 'prepare-revise', OPERATION, 'inspect-version'],
            'supported_operations': [OPERATION], 'allowed_operations': config['allowed_operations'],
            'allowed_fields': config['allowed_fields'], 'allowed_form_ids': config['allowed_form_ids'],
            **({key: config[key] for key in ('allowed_object_values', 'allowed_object_refs')}
               if 'allowed_object_values' in config else {}),
            'receipt': receipt, 'replayed': replayed, 'grants_admission': False,
            'materializations': source.materialize_claim_forms(record, payload, access_allowed=True) if payload else []}

    files, record, history = inspect()
    if operation == 'describe':
        return result(files, record)
    if operation == 'inspect-version':
        for receipt in history['receipts']:
            if receipt['previous_source'] == request['source'] and receipt['previous_source']['id'] == config['claim_id']:
                archived, locations = _read_archive(root, config, receipt)
                return {**result(files, record), 'record': _claims(archived[path.name])[config['claim_id']],
                    'inspected_source': request['source'], 'files': locations}
        raise source.JournalConflict('exact Claim version is not retained in this delegated history')
    _scope(config, request)
    if operation == 'prepare-revise':
        if len(history['receipts']) >= packages.MAX_REVISIONS:
            raise ValueError('Claim correction history capacity reached')
        revised, _, views, refs, dependencies, bindings = _proposal(config, path, files, record, request)
        return {**result(files, record), 'prepared_source': _subject(revised).ref, 'prepared_forms': refs,
            'prepared_materializations': views, 'expected_dependencies': dependencies, 'source_bindings': bindings}
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('invalid Claim correction command identity')
    digest = source._digest(source._canonical(request))
    with source._locked(root / 'ToS/source-witnesses/historical-create'):
        _, current_digest, current_path = source._configuration(owner)
        if current_digest != configuration_digest or current_path != path:
            raise source.JournalConflict('Claim correction delegation changed before transaction')
        files, record, history = inspect()
        for receipt in history['receipts']:
            if receipt['command_id'] == request['command_id']:
                if receipt['request_digest'] != digest or receipt['source']['id'] != config['claim_id']:
                    raise source.JournalConflict('Claim correction command identity was reused')
                source._sync_directory(path.parent.parent)
                return result(files, record, receipt, True)
        if (request['expected_configuration'] != configuration_digest or request['expected_source'] != _subject(record).ref
                or request['expected_revision'] != packages._revision(files)):
            raise source.JournalConflict('Claim correction source or package snapshot is stale')
        if len(history['receipts']) >= packages.MAX_REVISIONS:
            raise ValueError('Claim correction history capacity reached; retain history and route to its owner')
        revised, output, views, refs, dependencies, bindings = _proposal(config, path, files, record, request)
        if dependencies != request['expected_dependencies'] or bindings != request['expected_inputs']:
            raise source.JournalConflict('Claim correction dependencies are stale')
        archive_config = _archive_config(config, record['claim_id'])
        revision = packages._revision(files)
        receipt = {'command_id': request['command_id'], 'request_digest': digest,
            'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
            'owner_configuration': configuration_digest, 'recorded_at': datetime.now(timezone.utc).isoformat(),
            'reason': request['reason'], 'previous_source': _subject(record).ref, 'source': _subject(revised).ref,
            'previous_revision': revision, 'archive_path': packages._archive_path(archive_config, revision).as_posix(),
            'dependencies': dependencies, 'source_bindings': bindings, 'changed_fields': sorted(request['fields']), 'forms': refs,
            'grants_admission': False, 'request': request}
        output[HISTORY] = packages._encode({**history, 'receipts': [*history['receipts'], receipt]})
        if (len(output) > packages.MAX_FILES or any(len(v) > source.MAX_SET_BYTES for v in output.values())
                or len(output[path.name]) > source.MAX_COMMAND_BYTES or sum(map(len, output.values())) > packages.MAX_PACKAGE_BYTES):
            raise ValueError('revised Claim package exceeds its bounded metadata budget')
        packages._archive(root, archive_config, files, _subject(record), revision, reader=_read_archive)
        staging = packages._stage(root, output, '.claim-revision-')
        try:
            if (packages._package(path.parent) != files or source._configuration(owner)[1] != configuration_digest
                    or _proposal(config, path, files, record, request)[4] != dependencies):
                raise source.JournalConflict('Claim correction inputs changed before publication')
            packages._exchange(staging, path.parent)
        finally:
            if staging.exists():
                remaining = packages._package(staging)
                if remaining == files and _read_archive(root, config, receipt)[0] == files:
                    packages._discard_staging(staging, files)
                elif remaining == output:
                    packages._discard_staging(staging, output)
        return result(output, revised, receipt)

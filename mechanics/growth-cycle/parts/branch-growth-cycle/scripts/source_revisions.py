"""Bounded source-metadata correction with exact retained source packages.

Called only by the separately delegated source command route. No admission,
claim rewriting, model execution, payload access or graph publication.
"""
from datetime import datetime, timezone
import ctypes
import errno
import json
import os
from pathlib import Path
import stat
import tempfile

import source_commands as source

HISTORY = 'source-revision-history.json'
MAX_PACKAGE_BYTES = 8 * 1024 * 1024
MAX_FILES = 64
MAX_REVISIONS = 128


def _encode(value):
    return (json.dumps(value, ensure_ascii=False, allow_nan=False, indent=2) + '\n').encode()


def _package(directory, *, archive=False):
    """Fail closed on non-flat packages, symlinks and over-budget metadata."""
    os.close(source._owned_path(directory, directory=True))
    before = directory.stat()
    paths = sorted(directory.iterdir())
    if not 1 <= len(paths) <= MAX_FILES + int(archive):
        raise ValueError('source package file-count budget exceeded')
    files, total = {}, 0
    for path in paths:
        if not stat.S_ISREG(path.lstat().st_mode):
            raise PermissionError('revision requires a flat regular-file metadata package')
        raw = source._read(path, source.MAX_SET_BYTES)
        total += len(raw)
        if total > MAX_PACKAGE_BYTES + (source.MAX_SET_BYTES if archive else 0):
            raise ValueError('source package byte budget exceeded')
        files[path.name] = raw
    after = directory.stat()
    if (before.st_ino, before.st_mtime_ns, before.st_ctime_ns) != (after.st_ino, after.st_mtime_ns, after.st_ctime_ns):
        raise source.JournalConflict('source package changed during inspection')
    return files


def _file_refs(files):
    return {name: {'sha256': source._digest(raw), 'bytes': len(raw)} for name, raw in files.items()}


def _revision(files):
    return source._digest(source._canonical(_file_refs(files)))


def _history(files, record):
    history = source._json_object(files[HISTORY]) if HISTORY in files else {
        'schema_version': 'tos_source_revision_history_v1', 'record_id': record['record_id'], 'receipts': []}
    source._keys(history, {'schema_version', 'record_id', 'receipts'})
    if (history['schema_version'] != 'tos_source_revision_history_v1' or history['record_id'] != record['record_id']
            or not isinstance(history['receipts'], list) or len(history['receipts']) > MAX_REVISIONS):
        raise source.JournalCorruption('invalid source revision history')
    previous, commands = None, set()
    for receipt in history['receipts']:
        source._keys(receipt, {'command_id', 'request_digest', 'principal_id', 'authority_ref',
            'owner_configuration', 'recorded_at', 'reason', 'previous_source', 'source',
            'previous_revision', 'archive_path', 'dependencies', 'changed_fields', 'forms', 'grants_admission', 'request'})
        source._instant(receipt['recorded_at'])
        if (not isinstance(receipt['command_id'], str) or receipt['command_id'] in commands
                or source._digest(source._canonical(receipt['request'])) != receipt['request_digest']
                or receipt['command_id'] != receipt['request']['command_id']
                or receipt['previous_source'] != receipt['request']['expected_source']
                or receipt['previous_revision'] != receipt['request']['expected_revision']
                or receipt['owner_configuration'] != receipt['request']['expected_configuration']
                or receipt['dependencies'] != receipt['request']['expected_dependencies']
                or receipt['reason'] != receipt['request']['reason']
                or receipt['changed_fields'] != sorted(receipt['request']['fields'])
                or receipt['grants_admission'] is not False
                or receipt['source']['id'] != record['record_id']
                or receipt['previous_source']['id'] != record['record_id']
                or receipt['source']['version'] != receipt['previous_source']['version'] + 1
                or previous is not None and receipt['previous_source'] != previous):
            raise source.JournalCorruption('broken source revision lineage')
        commands.add(receipt['command_id'])
        previous = receipt['source']
    if previous is not None and previous != source.Record.from_payload(record['record_id'], record['record_version'], record).ref:
        raise source.JournalCorruption('current source is not the retained revision head')
    return history


def _validate_record(config, record):
    root = Path(config['source_root'])
    if config['schema_version'] == source.PROFILE_REVISION_CONFIG:
        profiles, profile = source._configured_profile(config)
        profiles.validate(profile['record_type'], record)
        return source._profile_input_snapshot(profiles)
    if record.get('schema_version') != 'tos_historical_record_v1':
        raise PermissionError('legacy revision requires the historical source schema')
    from source_witness_bibliographic_graph_common import historical_schema_validator
    historical_schema_validator(root).validate(record)
    return {path: source._digest(source._read(root / path, source.MAX_SET_BYTES))
            for path in ('ToS/contracts/historical-record.schema.json', 'ToS/contracts/corpus-record.schema.json')}


def _dependencies(config, record):
    inputs = _validate_record(config, record)
    for path in ('mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
                 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
                 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
                 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
                 'scripts/source_record_profiles.py', 'scripts/source_witness_human_forms.py',
                 'ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
                 'ToS/contracts/human-form-template.schema.json'):
        inputs[path] = source._digest(source._read(source.ROOT / path, source.MAX_SET_BYTES))
    return source._digest(source._canonical(inputs))


def _scope(config, request):
    if 'record.revise' not in config['allowed_operations']:
        raise PermissionError('record revision is not delegated')
    fields = request['fields']
    if not isinstance(fields, dict) or not fields or not set(fields) <= set(config['allowed_fields']):
        raise PermissionError('source field correction is outside delegated scope')
    selections = request['forms']
    if not isinstance(selections, list) or not 1 <= len(selections) <= 32:
        raise ValueError('source revision requires bounded form selections')
    identifiers = set()
    for item in selections:
        source._keys(item, {'form_id', 'field_id'})
        if item['form_id'] not in config['allowed_form_ids']:
            raise PermissionError('source revision form identity is not delegated')
        if item['form_id'] in identifiers:
            raise ValueError('duplicate revision form identity')
        identifiers.add(item['form_id'])
    if not isinstance(request['reason'], str) or not 1 <= len(request['reason'].strip()) <= 4096:
        raise ValueError('revision requires a bounded authored reason')


def _proposal(config, path, files, record, request):
    _scope(config, request)
    revised = {**record, **request['fields'], 'record_version': record['record_version'] + 1}
    _validate_record(config, revised)
    subject = source.Record.from_payload(revised['record_id'], revised['record_version'], revised)
    formname = path.stem + '.human-forms.json'
    payload = source._json_object(files[formname]) if formname in files else None
    if payload is not None:
        source._validate_history(payload)
        if {form['form_id'] for form in payload['forms']} - {item['form_id'] for item in request['forms']}:
            raise ValueError('revision must explicitly rebind every current form')
    changes = [source.prepare_metadata_change(revised, payload, config['principal_id'], **item) for item in request['forms']]
    value = source._apply(payload, subject, changes)
    views = source.materialize_metadata_forms(revised, value, access_allowed=True)
    if not all(view['state'] == 'ready' for view in views) or not any(view['role'] == 'name' for view in views):
        raise ValueError('revision forms must be ready source copies including a name')
    output = {**files, path.name: _encode(revised), formname: _encode(value)}
    if len(output[path.name]) > source.MAX_COMMAND_BYTES:
        raise ValueError('revised source record exceeds the existing metadata reader budget')
    return revised, subject, output, views, [source._form_ref(change['form']) for change in changes]


def _archive_path(config, revision):
    identifier = source._digest(config['record_id'].encode()).removeprefix('sha256:')
    return Path('ToS/source-witnesses/.record-revisions') / (identifier + '-' + revision.removeprefix('sha256:'))


def _read_archive_files(root, config, receipt):
    """Verify a byte-bound package independently of its selected record shape."""
    relative = _archive_path(config, receipt['previous_revision'])
    if receipt['archive_path'] != relative.as_posix():
        raise source.JournalCorruption('archive locator is not derived from exact subject and package')
    directory = root / relative
    contents = _package(directory, archive=True)
    manifest = source._json_object(contents.pop('manifest.json'))
    source._keys(manifest, {'schema_version', 'source_path', 'source', 'revision', 'files'})
    if (manifest['schema_version'] != 'tos_source_package_archive_v1' or manifest['source_path'] != config['source_path']
            or manifest['source'] != receipt['previous_source'] or manifest['revision'] != receipt['previous_revision']
            or not isinstance(manifest['files'], dict)):
        raise source.JournalCorruption('archive metadata does not bind the requested revision')
    restored, locations = {}, {}
    for name, binding in manifest['files'].items():
        source._keys(binding, {'blob', 'sha256', 'bytes'})
        blob = binding['blob']
        if (not isinstance(name, str) or Path(name).name != name
                or blob != binding['sha256'].removeprefix('sha256:') + '.blob'
                or blob not in contents or source._digest(contents[blob]) != binding['sha256']
                or len(contents[blob]) != binding['bytes']):
            raise source.JournalCorruption('archive byte binding is invalid')
        restored[name] = contents[blob]
        locations[name] = {'archive_path': (relative / blob).as_posix(), 'sha256': binding['sha256'], 'bytes': binding['bytes']}
    if _revision(restored) != receipt['previous_revision']:
        raise source.JournalCorruption('archive package digest is invalid')
    if set(contents) != {binding['blob'] for binding in manifest['files'].values()}:
        raise source.JournalCorruption('archive contains unbound files')
    return restored, locations


def _read_archive(root, config, receipt):
    restored, locations = _read_archive_files(root, config, receipt)
    old = source._json_object(restored[Path(config['source_path']).name])
    if source.Record.from_payload(old['record_id'], old['record_version'], old).ref != receipt['previous_source']:
        raise source.JournalCorruption('archived source bytes do not bind the previous source ref')
    if 'request' in receipt:
        revised = {**old, **receipt['request']['fields'], 'record_version': old['record_version'] + 1}
        if source.Record.from_payload(revised['record_id'], revised['record_version'], revised).ref != receipt['source']:
            raise source.JournalCorruption('retained request does not produce the recorded source successor')
    return restored, locations


def _stage(root, files, prefix):
    staging = Path(tempfile.mkdtemp(prefix=prefix, suffix='.pending', dir=root / 'ToS'))
    try:
        for name, raw in files.items():
            source._publish(staging / name, raw)
    except BaseException:
        # Abrupt process loss leaves this exact staging outside source scanners.
        # Ordinary exception cleanup concerns only files created in this call.
        _discard_staging(staging, files)
        raise
    return staging


def _discard_staging(staging, files):
    if staging.exists():
        for name in files:
            (staging / name).unlink(missing_ok=True)
        staging.rmdir()


def _archive(root, config, files, subject, revision, *, reader=None):
    reader = reader or _read_archive
    relative = _archive_path(config, revision)
    receipt = {'archive_path': relative.as_posix(), 'previous_source': subject.ref, 'previous_revision': revision}
    if (root / relative).exists():
        if reader(root, config, receipt)[0] != files:
            raise source.JournalCorruption('existing archive does not preserve this package')
        return relative
    target = root / relative
    target.parent.mkdir(mode=0o700, exist_ok=True)
    os.close(source._owned_path(target.parent, directory=True))
    source._sync_directory(target.parent.parent)
    refs = {name: {**binding, 'blob': binding['sha256'].removeprefix('sha256:') + '.blob'}
            for name, binding in _file_refs(files).items()}
    archived = {refs[name]['blob']: raw for name, raw in files.items()}
    archived['manifest.json'] = _encode({'schema_version': 'tos_source_package_archive_v1',
        'source_path': config['source_path'], 'source': subject.ref, 'revision': revision, 'files': refs})
    staging = _stage(root, archived, '.source-archive-')
    try:
        source._publish_new_directory(staging, target)
    finally:
        _discard_staging(staging, archived)
    if reader(root, config, receipt)[0] != files:
        raise source.JournalCorruption('stored archive differs from prior source bytes')
    return relative


def _exchange(staging, target):
    """Linux directory exchange; no non-atomic multi-file fallback."""
    libc = ctypes.CDLL(None, use_errno=True)
    try:
        rename = libc.renameat2
    except AttributeError:
        raise OSError(errno.ENOSYS, 'atomic directory exchange unavailable') from None
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(-100, os.fsencode(staging), -100, os.fsencode(target), 2) != 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))
    source._sync_directory(target.parent)
    source._sync_directory(staging.parent)


def run_revision(owner, config, configuration, path, request):
    root = Path(config['source_root'])
    operation = request.get('operation')
    fields = {'schema_version', 'operation'}
    if operation in {'record.revise', 'prepare-revise'}:
        fields |= {'fields', 'forms', 'reason'}
    if operation == 'record.revise':
        fields |= {'command_id', 'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies'}
    elif operation == 'inspect-version':
        fields |= {'source'}
    elif operation not in {'describe', 'prepare-revise'}:
        raise ValueError('unsupported source revision operation')
    source._keys(request, fields)
    if request['schema_version'] != 'tos_local_source_command_v1':
        raise ValueError('unknown source command version')

    def inspect():
        files = _package(path.parent)
        record = source._json_object(files[path.name])
        if (record.get('record_id') != config['record_id']
                or record.get('visibility') not in {'public', 'public_metadata_only'}):
            raise PermissionError('source revision subject or visibility is outside this adapter')
        _validate_record(config, record)
        subject = source.Record.from_payload(record['record_id'], record['record_version'], record)
        history = _history(files, record)
        return files, record, subject, history

    def result(files, record, subject, receipt=None, replayed=False):
        formname = path.stem + '.human-forms.json'
        payload = source._json_object(files[formname]) if formname in files else None
        return {'schema_version': 'tos_local_source_revision_result_v1', 'authentication': 'local-unix-account',
            'owner_configuration': configuration, 'source_path': config['source_path'], 'source': subject.ref,
            'revision': _revision(files), 'command_operations': ['describe', 'prepare-revise', 'record.revise', 'inspect-version'],
            'supported_operations': ['record.revise'], 'allowed_operations': config['allowed_operations'],
            'allowed_fields': config['allowed_fields'], 'allowed_form_ids': config['allowed_form_ids'],
            **({'profile_type_id': config['profile_type_id'],
                'source_record_profile': source._configured_profile(config)[1]}
               if config['schema_version'] == source.PROFILE_REVISION_CONFIG else {}),
            'receipt': receipt, 'replayed': replayed, 'grants_admission': False,
            'materializations': source.materialize_metadata_forms(record, payload, access_allowed=True) if payload else []}

    files, record, subject, history = inspect()
    if operation == 'describe':
        return result(files, record, subject)
    if operation == 'inspect-version':
        for receipt in history['receipts']:
            if receipt['previous_source'] == request['source']:
                archived, locations = _read_archive(root, config, receipt)
                return {**result(files, record, subject), 'record': source._json_object(archived[path.name]),
                        'inspected_source': request['source'], 'files': locations}
        raise source.JournalConflict('requested exact source version is not retained in committed history')
    _scope(config, request)  # Scope revocation applies even to a retry.
    if operation == 'prepare-revise':
        if len(history['receipts']) >= MAX_REVISIONS:
            raise ValueError('source revision history capacity reached')
        revised, proposed, _, views, refs = _proposal(config, path, files, record, request)
        return {**result(files, record, subject), 'prepared_source': proposed.ref, 'prepared_forms': refs,
                'prepared_materializations': views, 'expected_dependencies': _dependencies(config, record)}
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('invalid source revision command identity')
    digest = source._digest(source._canonical(request))
    with source._locked(root / 'ToS/source-witnesses/historical-create'):
        latest, latest_digest, latest_path = source._configuration(owner)
        if latest_digest != configuration or latest_path != path:
            raise source.JournalConflict('revision delegation changed before transaction')
        files, record, subject, history = inspect()
        for receipt in history['receipts']:
            if receipt['command_id'] == request['command_id']:
                if receipt['request_digest'] != digest:
                    raise source.JournalConflict('source revision command identity was reused')
                _read_archive(root, config, receipt)
                source._sync_directory(path.parent.parent)
                return result(files, record, subject, receipt, True)
        revision, dependencies = _revision(files), _dependencies(config, record)
        if (request['expected_configuration'] != configuration or request['expected_source'] != subject.ref
                or request['expected_revision'] != revision or request['expected_dependencies'] != dependencies):
            raise source.JournalConflict('source revision snapshot or dependencies are stale')
        if len(history['receipts']) >= MAX_REVISIONS:
            raise ValueError('source revision history capacity reached; retain history and route to archive reader')
        revised, proposed, output, views, refs = _proposal(config, path, files, record, request)
        archive = _archive_path(config, revision)
        receipt = {'command_id': request['command_id'], 'request_digest': digest,
            'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
            'owner_configuration': configuration, 'recorded_at': datetime.now(timezone.utc).isoformat(),
            'reason': request['reason'], 'previous_source': subject.ref, 'source': proposed.ref,
            'previous_revision': revision, 'archive_path': archive.as_posix(), 'dependencies': dependencies,
            'changed_fields': sorted(request['fields']), 'forms': refs, 'grants_admission': False, 'request': request}
        output[HISTORY] = _encode({**history, 'receipts': [*history['receipts'], receipt]})
        if (len(output) > MAX_FILES or any(len(raw) > source.MAX_SET_BYTES for raw in output.values())
                or sum(map(len, output.values())) > MAX_PACKAGE_BYTES):
            raise ValueError('revised source package exceeds its byte or file-count budget')
        _archive(root, config, files, subject, revision)
        staging = _stage(root, output, '.source-revision-')
        try:
            if (_package(path.parent) != files or source._configuration(owner)[1] != configuration
                    or _dependencies(config, record) != dependencies):
                raise source.JournalConflict('source revision inputs changed before publication')
            _exchange(staging, path.parent)
        finally:
            # If exchange completed even when its response/fsync failed, this
            # contains OLD bytes. Remove only an exact, independently verified
            # duplicate of the retained archive; otherwise leave it for recovery.
            if staging.exists():
                remaining = _package(staging)
                if remaining == files and _read_archive(root, config, receipt)[0] == files:
                    _discard_staging(staging, files)
                elif remaining == output:
                    _discard_staging(staging, output)
        return result(output, revised, proposed, receipt)

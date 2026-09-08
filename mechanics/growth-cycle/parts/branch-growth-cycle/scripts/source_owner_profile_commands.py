"""Explicit owner-local source profiles under the common record/form grammar.

The protected context selects storage, never a schema override or admission.
Public catalogs are not consumers. Existing source creation, revision archive
and form receipts carry the same identities and histories in a private store.
"""
from __future__ import annotations

from contextlib import contextmanager
from datetime import datetime, timezone
import os
from pathlib import Path
import re
import stat
import tempfile
import time

import source_commands as source
import source_revisions as revisions
from source_owner_context import OwnerLocalSourceContext, _open, _read as context_read
from source_owner_record_profiles import OwnerLocalSourceRecordProfiles
from source_witness_human_forms import _materialize_forms, metadata_field_catalog


CONFIG = 'tos_local_owner_profile_command_v1'
OPERATIONS = ('source.create', 'record.revise', 'form.create', 'form.revise')
CONFIG_FILE = 'source-create-owner-configuration.json'
RECEIPT_FILE = 'source-create-receipt.json'
IMPLEMENTATIONS = (
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_profile_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_text_unit_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
    'scripts/source_owner_record_profiles.py', 'scripts/source_record_profiles.py',
    'scripts/source_owner_context.py', 'scripts/native_text_binding.py',
    'scripts/source_witness_human_forms.py', 'ToS/contracts/human-form.schema.json',
    'ToS/contracts/human-form-set.schema.json', 'ToS/contracts/human-form-template.schema.json',
    'ToS/contracts/provenance-event-v2.schema.json',
)


def _directory(context, path):
    if not path.is_relative_to(context.private_root):
        raise PermissionError('private source package leaves its owner-local root')
    os.close(_open(path, directory=True, private_root=context.private_root))


def _profiles(config, context):
    profiles = OwnerLocalSourceRecordProfiles(context, config['source_access'], config['source_binding'],
                                             source_reader=source._read)
    entry = next((row for row in profiles.registry['types'] if row['type_id'] == config['profile_type_id']), None)
    profile = entry.get('source_record_profile') if entry else None
    if profile is None or profile['reader'] != 'semantic-metadata-v1':
        raise PermissionError('owner-local writing requires an explicit semantic source-record profile')
    profiles.validate_path(profile['record_type'], config['source_path'])
    return profiles, profile


def configuration(config, *, owner_config):
    source._keys(config, {'schema_version', 'uid', 'principal_id', 'authority_ref', 'expires_at',
        'source_context_ref', 'source_path', 'source_access', 'source_binding', 'profile_type_id',
        'record_id', 'allowed_operations', 'allowed_fields', 'allowed_form_ids', 'provenance_event_id'})
    raw = context_read(Path(owner_config), source.MAX_COMMAND_BYTES, confidential_file=True)
    if source._json_object(raw) != config:
        raise source.JournalConflict('owner-local delegation changed during selection')
    if (config['schema_version'] != CONFIG or type(config['uid']) is not int or config['uid'] != os.getuid()
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref', 'profile_type_id'))):
        raise PermissionError('owner-local record delegation is invalid or expired')
    for key, allowed in (('allowed_operations', OPERATIONS), ('allowed_fields', source.REVISION_FIELDS)):
        values = config[key]
        if (not isinstance(values, list) or len(values) > 32 or any(not isinstance(value, str) for value in values)
                or len(set(values)) != len(values) or not set(values) <= set(allowed)):
            raise PermissionError('owner-local delegation exceeds operation or descriptive field scope')
    ids = config['allowed_form_ids']
    if (not isinstance(ids, list) or len(ids) > 32
            or any(not isinstance(value, str) or not re.fullmatch(r'tos\.form\.[a-z0-9]+(?:[.-][a-z0-9]+)*', value) for value in ids)
            or len(ids) != len(set(ids))
            or not isinstance(config['provenance_event_id'], str)
            or not re.fullmatch(r'tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['provenance_event_id'])):
        raise ValueError('owner-local form and provenance identities must be bounded and explicit')
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    profiles, profile = _profiles(config, context)
    if (not isinstance(config['record_id'], str)
            or not re.fullmatch(re.escape(profile['id_prefix']) + r'[a-z0-9]+(?:[.-][a-z0-9]+)*', config['record_id'])):
        raise PermissionError('owner-local record identity does not match its selected profile')
    path = context.path(config['source_path'])
    _directory(context, path.parent.parent)
    digest = source._digest(source._canonical({'configuration_bytes': source._digest(raw),
        'context': context.snapshot(), 'profile_inputs': profiles.input_digests}))
    return config, digest, path


@contextmanager
def _locked(context):
    # Every cooperating public creator and both private writers use this order.
    with source._locked(context.public_root / 'ToS/source-witnesses/historical-create'), source._locked(context.private_root / 'native-create'):
        os.close(_open(context.private_root / '.native-create.writer.lock', private_root=context.private_root))
        yield


def _package(context, directory, *, archive=False):
    _directory(context, directory)
    before = directory.stat()
    paths = sorted(directory.iterdir())
    if not 1 <= len(paths) <= revisions.MAX_FILES + int(archive):
        raise ValueError('owner-local package file-count budget exceeded')
    result, remaining = {}, revisions.MAX_PACKAGE_BYTES + (source.MAX_SET_BYTES if archive else 0)
    for path in paths:
        if not stat.S_ISREG(path.lstat().st_mode):
            raise PermissionError('owner-local package must contain flat regular protected files')
        raw = context_read(path, min(source.MAX_SET_BYTES, remaining), private_root=context.private_root)
        remaining -= len(raw)
        result[path.name] = raw
    after = directory.stat()
    if (before.st_ino, before.st_mtime_ns, before.st_ctime_ns) != (after.st_ino, after.st_mtime_ns, after.st_ctime_ns):
        raise source.JournalConflict('owner-local package changed while reading')
    return result


def _stage(context, files, *, archive=False):
    if (len(files) > revisions.MAX_FILES + int(archive) or any(len(raw) > source.MAX_SET_BYTES for raw in files.values())
            or sum(map(len, files.values())) > revisions.MAX_PACKAGE_BYTES + (source.MAX_SET_BYTES if archive else 0)):
        raise ValueError('owner-local package exceeds metadata storage budgets')
    staging = Path(tempfile.mkdtemp(prefix='.owner-source-', suffix='.pending', dir=context.private_root))
    try:
        for name, raw in files.items():
            if Path(name).name != name or name in {'.', '..'}:
                raise ValueError('source package filename is not flat')
            source._publish(staging / name, raw)
        if _package(context, staging, archive=archive) != files:
            raise source.JournalConflict('owner-local staging changed')
    except BaseException:
        revisions._discard_staging(staging, files)
        raise
    return staging


def _archive_ref(context, config, revision):
    identifier = source._digest(config['record_id'].encode())[7:]
    return Path(context.private_prefix) / '.record-revisions' / (identifier + '-' + revision[7:])


def _read_archive_files(context, config, receipt):
    """Verify protected archive bytes without interpreting the source subject."""
    relative = _archive_ref(context, config, receipt['previous_revision'])
    if receipt['archive_path'] != relative.as_posix():
        raise source.JournalCorruption('owner-local archive has a different logical owner')
    contents = _package(context, context.path(relative.as_posix()), archive=True)
    if 'manifest.json' not in contents:
        raise source.JournalCorruption('owner-local archive lacks its manifest')
    manifest = source._json_object(contents.pop('manifest.json'))
    source._keys(manifest, {'schema_version', 'source_path', 'source', 'revision', 'files'})
    if (manifest['schema_version'] != 'tos_source_package_archive_v1'
            or manifest['source_path'] != config['source_path'] or manifest['source'] != receipt['previous_source']
            or manifest['revision'] != receipt['previous_revision'] or not isinstance(manifest['files'], dict)):
        raise source.JournalCorruption('owner-local archive does not bind the exact previous package')
    restored, locations = {}, {}
    for name, binding in manifest['files'].items():
        source._keys(binding, {'blob', 'sha256', 'bytes'})
        blob = binding['blob']
        if (Path(name).name != name or name in {'.', '..'} or not isinstance(binding['sha256'], str)
                or not re.fullmatch(r'sha256:[a-f0-9]{64}', binding['sha256'])
                or blob != binding['sha256'][7:] + '.blob' or blob not in contents
                or source._digest(contents[blob]) != binding['sha256'] or len(contents[blob]) != binding['bytes']):
            raise source.JournalCorruption('owner-local archive byte binding is invalid')
        restored[name] = contents[blob]
        locations[name] = {'archive_path': (relative / blob).as_posix(), 'sha256': binding['sha256'], 'bytes': binding['bytes']}
    if (set(contents) != {row['blob'] for row in manifest['files'].values()}
            or revisions._revision(restored) != receipt['previous_revision']):
        raise source.JournalCorruption('owner-local archive contains missing or unbound bytes')
    return restored, locations


def _read_archive(context, config, receipt):
    restored, locations = _read_archive_files(context, config, receipt)
    basename = Path(config['source_path']).name
    if basename not in restored:
        raise source.JournalCorruption('owner-local archive lacks its bound source record')
    old = source._json_object(restored[basename])
    if source.metadata_subject(old).ref != receipt['previous_source']:
        raise source.JournalCorruption('owner-local archived record differs from its source binding')
    if 'request' in receipt:
        revised = {**old, **receipt['request']['fields'], 'record_version': old['record_version'] + 1}
        if source.metadata_subject(revised).ref != receipt['source']:
            raise source.JournalCorruption('retained revision does not produce its recorded successor')
    return restored, locations


def _archive(context, config, files, subject, revision, *, reader=None):
    reader = reader if reader is not None else _read_archive
    relative = _archive_ref(context, config, revision)
    receipt = {'archive_path': relative.as_posix(), 'previous_source': subject.ref, 'previous_revision': revision}
    target = context.path(relative.as_posix())
    if os.path.lexists(target):
        if reader(context, config, receipt)[0] != files:
            raise source.JournalCorruption('existing owner-local archive differs from the previous package')
        return relative
    target.parent.mkdir(mode=0o700, exist_ok=True)
    _directory(context, target.parent)
    source._sync_directory(target.parent.parent)
    refs = {name: {**binding, 'blob': binding['sha256'][7:] + '.blob'} for name, binding in revisions._file_refs(files).items()}
    archived = {refs[name]['blob']: raw for name, raw in files.items()}
    archived['manifest.json'] = revisions._encode({'schema_version': 'tos_source_package_archive_v1',
        'source_path': config['source_path'], 'source': subject.ref, 'revision': revision, 'files': refs})
    staging = _stage(context, archived, archive=True)
    try:
        source._publish_new_directory(staging, target)
    finally:
        revisions._discard_staging(staging, archived)
    if reader(context, config, receipt)[0] != files:
        raise source.JournalCorruption('owner-local stored archive differs from its input bytes')
    return relative


def _inventory(context, profiles, config, *, exclude=None, creating=False, reserved_ids=None):
    """Current source identities only; no private catalog or source export.

    Native IDs remain reserved and fields inside prose are not interpreted as
    identity declarations. Archived/staged versions do not become competitors.
    """
    from source_text_unit_commands import MAX_INVENTORY_ENTRIES, MAX_INVENTORY_BYTES, MAX_INVENTORY_FILE_BYTES
    from source_record_profiles import SOURCE_CLAIM_BASENAME
    basenames = {*profiles.source_basenames.values(), SOURCE_CLAIM_BASENAME}
    pending = [context.public_root / 'ToS/source-witnesses', context.private_root / context.private_prefix]
    visited, files, remaining, inputs = 0, 0, MAX_INVENTORY_BYTES, {}
    reserved = ({config['record_id'], *config['allowed_form_ids']} if reserved_ids is None else set(reserved_ids))
    if creating:
        reserved.add(config['provenance_event_id'])
    while pending:
        directory = pending.pop()
        if directory == exclude:
            continue
        private = directory.is_relative_to(context.private_root)
        root = context.private_root if private else context.public_root
        if private:
            _directory(context, directory)
        else:
            os.close(source._owned_path(directory, directory=True))
        with os.scandir(directory) as entries:
            for entry in entries:
                visited += 1
                if visited > MAX_INVENTORY_ENTRIES:
                    raise ValueError('source identity directory-entry budget exceeded')
                if entry.name.startswith('.') or entry.name in {'payload', 'local-content', 'catalog'}:
                    continue
                path = Path(entry.path)
                info = entry.stat(follow_symlinks=False)
                if stat.S_ISLNK(info.st_mode):
                    raise PermissionError('source identity discovery refuses aliases')
                if stat.S_ISDIR(info.st_mode):
                    pending.append(path)
                    continue
                if not (entry.name in basenames or entry.name.endswith('.human-forms.json')
                        or entry.name.startswith('semantic-annotation') and entry.name.endswith('.json')
                        or creating and 'provenance' in entry.name and entry.name.endswith('.jsonl')):
                    continue
                files += 1
                if files > 2048 or not stat.S_ISREG(info.st_mode):
                    raise ValueError('source identity metadata file budget or regular-file contract violated')
                raw = context.read_bytes(path, min(MAX_INVENTORY_FILE_BYTES, remaining), read_bytes=source._read)
                remaining -= len(raw)
                inputs[path.relative_to(root).as_posix()] = source._digest(raw)
                rows = raw.splitlines() if path.suffix == '.jsonl' else [raw]
                if len(rows) > 65536:
                    raise ValueError('source identity JSONL record budget exceeded')
                for row in rows:
                    if not row.strip():
                        continue
                    value = source._json_object(row)
                    ids = {value.get(key) for key in ('record_id', 'claim_id', 'event_id') if isinstance(value.get(key), str)}
                    for key, field in (('forms', 'form_id'), ('prior_forms', 'form_id'), ('occurrences', 'occurrence_id'),
                                       ('lexemes', 'lexeme_id'), ('senses', 'sense_id'), ('signs', 'sign_id'), ('concepts', 'concept_id'),
                                       ('entities', 'entity_id'), ('claims', 'claim_id')):
                        for item in value.get(key, []):
                            if isinstance(item, dict) and isinstance(item.get(field), str):
                                ids.add(item[field])
                    if ids.intersection(reserved):
                        raise source.JournalConflict('delegated source, form or provenance identity already has another owner')
    return source._digest(source._canonical(inputs))


def _dependencies(config, context, record, *, exclude=None, creating=False):
    profiles, profile = _profiles(config, context)
    profiles.validate(profile['record_type'], record)
    if record.get('record_id') != config['record_id']:
        raise PermissionError('owner-local record differs from the delegated subject')
    if config['source_binding'] is not None:
        # Check derivation permission before any source representation is read.
        from native_text_binding import NativeTextBindingResolver
        from source_text_unit_commands import _local_research_gate
        resolver = NativeTextBindingResolver(context.public_root, owner_context=context, read_bytes=source._read)
        binding = config['source_binding']
        resolver.resolve(binding)
        layer = resolver._record(binding['text_layer']['record_ref'], expected=binding['text_layer']['record_sha256'])
        _local_research_gate(resolver, layer)
        profiles.validate_native_binding(profile['record_type'], record, verify_content=True)
        native = resolver.snapshot()
    else:
        native = None
    return source._digest(source._canonical({'profiles': profiles.snapshot(), 'rights': native,
        'form_grammar': _form_grammar(context)[2],
        'identity': _inventory(context, profiles, config, exclude=exclude, creating=creating),
        'implementation': {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES)) for ref in IMPLEMENTATIONS}}))


def _form_grammar(context):
    from human_forms import compile_source_form_validators
    schemas, digests = {}, {}
    for name in ('knowledge-assessment', 'human-form', 'human-form-set', 'human-form-template'):
        ref = 'ToS/contracts/' + name + '.schema.json'
        raw = context.read_bytes(context.path(ref), source.MAX_COMMAND_BYTES, read_bytes=source._read)
        schemas[name], digests[ref] = source._json_object(raw), source._digest(raw)
    validator, materializer = compile_source_form_validators(schemas)
    return validator, materializer, digests


def _materialize(record, payload, context):
    # Only the explicit private adapter calls the common pure form machinery.
    # The public metadata renderer still rejects this visibility.
    if record.get('visibility') != 'local_only':
        raise PermissionError('owner-local form materialization requires a private source record')
    validator, materializer, _ = _form_grammar(context)
    return _materialize_forms(source.metadata_subject(record), metadata_field_catalog(record), payload, access_allowed=True,
                              validator=validator, materializer_validators=materializer)


def _forms(config, record, payload, selections, context, *, rebind=False):
    if not isinstance(selections, list) or not 1 <= len(selections) <= 32:
        raise ValueError('owner-local source requires bounded source-copy form selections')
    seen = set()
    for row in selections:
        source._keys(row, {'form_id', 'field_id'})
        if row['form_id'] not in config['allowed_form_ids'] or row['form_id'] in seen:
            raise PermissionError('owner-local form identity is not delegated or repeats')
        seen.add(row['form_id'])
    validator, _, _ = _form_grammar(context)
    if payload is not None:
        source._validate_history(payload, validator=validator)
        if rebind and {row['form_id'] for row in payload['forms']} - seen:
            raise ValueError('record revision must explicitly rebind every current form')
    changes = [source.prepare_metadata_change(record, payload, config['principal_id'], **row) for row in selections]
    value = source._apply(payload, source.metadata_subject(record), changes, validator=validator)
    views = _materialize(record, value, context)
    if not all(view['state'] == 'ready' for view in views) or not any(view['role'] == 'name' for view in views):
        raise ValueError('owner-local forms must be complete source copies including a name')
    return value, views, [source._form_ref(change['form']) for change in changes]


def _form_changes(request, config):
    changes = source._changes(request, config)
    # This combined owner config also contains record operations. They are
    # never aliases for form.revise in the common form application helper.
    if any(change['operation'] not in source.OPERATIONS for change in changes):
        raise PermissionError('apply accepts only explicitly delegated form operations')
    return changes


def _inspect(config, context, path):
    files = _package(context, path.parent)
    if not {path.name, path.stem + '.human-forms.json', RECEIPT_FILE} <= files.keys():
        raise source.JournalCorruption('owner-local package lacks its source, forms or creation receipt')
    record = source._json_object(files[path.name])
    profiles, profile = _profiles(config, context)
    profiles.validate(profile['record_type'], record)
    if record['record_id'] != config['record_id']:
        raise PermissionError('owner-local package has another subject')
    history = revisions._history(files, record)
    payload = source._json_object(files[path.stem + '.human-forms.json'])
    source._validate_history(payload, validator=_form_grammar(context)[0])
    subject = source.metadata_subject(record)
    if payload['subject'] != subject.ref:
        raise source.JournalCorruption('owner-local current forms do not bind the current source')
    retained = {source._canonical(source._form_ref(form)) for form in [*payload['forms'], *payload['prior_forms']]}
    if any(source._canonical(ref) not in retained for receipt in history['receipts'] for ref in receipt['forms']):
        raise source.JournalCorruption('owner-local revision-produced form is absent from retained history')
    return files, record, subject, history, payload


def _verify_revision_forms(context, config, path, receipt, payload):
    """Bind a revision's exact forms to its archived input and retained request."""
    archived, locations = _read_archive(context, config, receipt)
    old = source._json_object(archived[path.name])
    revised = {**old, **receipt['request']['fields'], 'record_version': old['record_version'] + 1}
    prior = source._json_object(archived[path.stem + '.human-forms.json'])
    changes = [source.prepare_metadata_change(revised, prior, receipt['principal_id'], **selection)
               for selection in receipt['request']['forms']]
    # Re-run the common form transition against exact predecessor bytes;
    # altered results cannot be blessed just by altering a stored digest.
    source._apply(prior, source.metadata_subject(revised), changes, validator=_form_grammar(context)[0])
    refs = [source._form_ref(change['form']) for change in changes]
    retained = {source._canonical(source._form_ref(form)) for form in [*payload['forms'], *payload['prior_forms']]}
    if receipt['forms'] != refs or any(source._canonical(ref) not in retained for ref in refs):
        raise source.JournalCorruption('owner-local revision form outputs differ from their exact request')
    return archived, locations


def _current(owner, configuration_digest, path, config, context, record, dependencies, *, exclude=None,
             creating=False, expected_files=None):
    if (_dependencies(config, context, record, exclude=exclude, creating=creating) != dependencies
            or (expected_files is not None and _package(context, path.parent) != expected_files)
            or context.snapshot() != OwnerLocalSourceContext.load(config['source_context_ref']).snapshot()
            or source._configuration(owner)[1:] != (configuration_digest, path)):
        raise source.JournalConflict('owner-local source, context, rights or delegation changed')


def _result(config, configuration_digest, path, *, state=None, receipt=None, replayed=False):
    result = {'schema_version': 'tos_local_source_command_result_v1', 'authentication': 'local-unix-account',
        'owner_configuration': configuration_digest, 'source_path': config['source_path'], 'record_id': config['record_id'],
        'profile_type_id': config['profile_type_id'], 'target_exists': os.path.lexists(path.parent),
        'supported_operations': list(OPERATIONS), 'allowed_operations': config['allowed_operations'],
        'command_operations': ['describe', 'prepare-create', 'source.create', 'prepare-revise', 'record.revise', 'prepare', 'apply', 'inspect-version'],
        'allowed_form_ids': config['allowed_form_ids'], 'allowed_fields': config['allowed_fields'],
        'visibility': 'local_only', 'publication_authorized': False, 'grants_admission': False,
        'receipt': receipt, 'replayed': replayed,
        'replay_input_posture': 'historical_request_current_validation' if replayed else None,
        'source': None, 'revision': None, 'materializations': []}
    if state is not None:
        files, record, subject, _, payload = state
        result.update(source=subject.ref, revision=revisions._revision(files),
            source_fields=[{key: value for key, value in row.items() if key not in {'pointer', 'context'}}
                           for row in metadata_field_catalog(record)],
            forms=[source._form_ref(form) for form in payload['forms']],
            materializations=_materialize(record, payload, OwnerLocalSourceContext.load(config['source_context_ref'])))
    return result


def _prepare_create(config, context, path, request, *, exclude=None):
    if _profiles(config, context)[1].get('creation_gate'):
        raise PermissionError('this profile requires its explicit promotion adapter, not generic private source creation')
    record = request['record']
    if (not isinstance(record, dict) or record.get('record_id') != config['record_id']
            or record.get('record_version') != 1 or record.get('identity_status') != 'provisional'
            or record.get('same_as_posture') != 'no_equivalence_claim' or record.get('supersedes_ref') is not None):
        raise PermissionError('source creation requires a provisional initial identity without equivalence admission')
    dependencies = _dependencies(config, context, record, exclude=exclude, creating=True)
    forms, views, _ = _forms(config, record, None, request['forms'], context)
    files = {path.name: revisions._encode(record), path.stem + '.human-forms.json': revisions._encode(forms),
             CONFIG_FILE: revisions._encode(config)}
    return source.metadata_subject(record), files, dependencies, views


def _creation_replay(config, context, path, request, configuration_digest):
    if not {path.name, path.stem + '.human-forms.json', RECEIPT_FILE} <= _package(context, path.parent).keys():
        raise source.JournalConflict('existing owner-local target is not this source creation')
    state = _inspect(config, context, path)
    files, record, _, history, forms = state
    receipt = source._json_object(files[RECEIPT_FILE])
    source._keys(receipt, {'schema_version', 'command_id', 'request_digest', 'principal_id', 'authority_ref',
        'owner_configuration', 'recorded_at', 'source_path', 'source', 'dependencies', 'files', 'grants_admission'})
    if (receipt['command_id'] != request['command_id'] or receipt['request_digest'] != source._digest(source._canonical(request))
            or receipt['source_path'] != config['source_path']):
        raise source.JournalConflict('owner-local source creation identity is already occupied')
    if (receipt['schema_version'] != 'tos_local_source_create_receipt_v1' or receipt['grants_admission'] is not False
            or receipt['owner_configuration'] != configuration_digest or request['expected_configuration'] != configuration_digest
            or receipt['principal_id'] != config['principal_id'] or receipt['authority_ref'] != config['authority_ref']
            or request['expected_source'] is not None or request['expected_revision'] is not None):
        raise source.JournalCorruption('owner-local creation receipt differs from its delegation')
    source._instant(receipt['recorded_at'])
    original = files
    for index, revision in enumerate(history['receipts']):
        archived, _ = _verify_revision_forms(context, config, path, revision, forms)
        if archived.get(RECEIPT_FILE) != files[RECEIPT_FILE]:
            raise source.JournalCorruption('creation receipt changed across owner-local history')
        if index == 0:
            original = archived
    subject, prepared, dependencies, _ = _prepare_create(config, context, path, request, exclude=path.parent)
    if receipt['source'] != subject.ref or receipt['dependencies'] != request['expected_dependencies']:
        raise source.JournalCorruption('owner-local creation receipt differs from its original source or request')
    expected = {path.name, path.stem + '.human-forms.json', CONFIG_FILE,
                'source-create-request.json', 'source-create-environment.json', 'source-create-provenance.jsonl'}
    if (not isinstance(receipt['files'], dict) or set(receipt['files']) != expected
            or set(files) - expected - {RECEIPT_FILE, revisions.HISTORY}):
        raise source.JournalCorruption('owner-local creation package has unbound files')
    for name in expected:
        raw = original[name] if name == path.name else prepared[name] if name == path.stem + '.human-forms.json' else files[name]
        if receipt['files'][name] != {'sha256': source._digest(raw), 'bytes': len(raw)}:
            raise source.JournalCorruption('owner-local creation bytes differ from their retained receipt')
    if (original[path.name] != prepared[path.name] or files[CONFIG_FILE] != prepared[CONFIG_FILE]
            or source._json_object(files['source-create-request.json']) != request):
        raise source.JournalCorruption('owner-local original source or retained request changed')
    initial = {form['form_id']: form for form in [*forms['prior_forms'], *forms['forms']] if form['form_version'] == 1}
    if any(initial.get(form['form_id']) != form for form in source._json_object(prepared[path.stem + '.human-forms.json'])['forms']):
        raise source.JournalCorruption('owner-local initial forms are not retained')
    _dependencies(config, context, record, exclude=path.parent)
    # Current validation/collision evidence is separate from the opaque CAS
    # evidence captured with the historical request. Return this retry's
    # snapshot so the final guard cannot mistake neighbor growth for a rewrite.
    return state, receipt, dependencies


def run_command(owner, config, configuration_digest, path, request):
    operation = request.get('operation')
    fields = {'schema_version', 'operation'}
    if operation in {'prepare-create', 'source.create'}:
        fields |= {'record', 'forms'}
    elif operation in {'prepare-revise', 'record.revise'}:
        fields |= {'fields', 'forms', 'reason'}
    elif operation == 'prepare':
        fields |= {'form_id', 'field_id'}
    elif operation == 'apply':
        fields |= {'changes'}
    elif operation == 'inspect-version':
        fields |= {'source'}
    elif operation != 'describe':
        raise ValueError('unknown owner-local source operation')
    if operation in {'source.create', 'record.revise', 'apply'}:
        fields |= {'command_id', 'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies'}
    source._keys(request, fields)
    if request['schema_version'] != 'tos_local_source_command_v1':
        raise ValueError('unknown source command envelope')
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    if operation == 'describe' and not os.path.lexists(path.parent):
        return _result(config, configuration_digest, path)
    if operation in {'prepare-create', 'source.create'}:
        if 'source.create' not in config['allowed_operations']:
            raise PermissionError('owner-local source creation is not delegated')
        if operation == 'prepare-create':
            subject, files, dependencies, views = _prepare_create(config, context, path, request)
            return {**_result(config, configuration_digest, path), 'prepared_source': subject.ref,
                'expected_source': None, 'expected_revision': None, 'expected_dependencies': dependencies,
                'prepared_files': revisions._file_refs(files), 'prepared_materializations': views}
    else:
        state = _inspect(config, context, path)
        dependencies = _dependencies(config, context, state[1], exclude=path.parent)
        if operation == 'describe':
            return {**_result(config, configuration_digest, path, state=state), 'expected_dependencies': dependencies}
        if operation == 'inspect-version':
            for receipt in state[3]['receipts']:
                if receipt['previous_source'] == request['source']:
                    archived, locations = _read_archive(context, config, receipt)
                    return {**_result(config, configuration_digest, path, state=state),
                        'record': source._json_object(archived[path.name]), 'inspected_source': request['source'], 'files': locations}
            raise source.JournalConflict('exact source version is not retained')
        if operation in {'prepare-revise', 'record.revise'}:
            revisions._scope(config, request)
            if operation == 'prepare-revise':
                proposed, _, _, refs = _revision_proposal(config, context, path, state, request)
                return {**_result(config, configuration_digest, path, state=state),
                    'prepared_source': source.metadata_subject(proposed).ref, 'prepared_forms': refs,
                    'expected_dependencies': dependencies}
        elif operation == 'prepare':
            if request['form_id'] not in config['allowed_form_ids']:
                raise PermissionError('owner-local form identity is not delegated')
            change = source.prepare_metadata_change(state[1], state[4], config['principal_id'], request['form_id'], request['field_id'])
            if change['operation'] not in config['allowed_operations']:
                raise PermissionError('owner-local form operation is not delegated')
            return {**_result(config, configuration_digest, path, state=state), 'prepared_change': change,
                    'expected_dependencies': dependencies}
        else:
            _form_changes(request, config)
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('source command identity must contain one to 256 characters')
    with _locked(context):
        if source._configuration(owner)[1:] != (configuration_digest, path):
            raise source.JournalConflict('owner-local delegation changed before transaction')
        if operation == 'source.create':
            if os.path.lexists(path.parent):
                state, receipt, replay_dependencies = _creation_replay(config, context, path, request, configuration_digest)
                response = _result(config, configuration_digest, path, state=state, receipt=receipt, replayed=True)
                _current(owner, configuration_digest, path, config, context, request['record'],
                         replay_dependencies, exclude=path.parent, creating=True, expected_files=state[0])
                return response
            return _create(owner, config, configuration_digest, context, path, request)
        return _update(owner, config, configuration_digest, context, path, request)


def _create(owner, config, configuration_digest, context, path, request):
    if (request['expected_configuration'] != configuration_digest or request['expected_source'] is not None
            or request['expected_revision'] is not None):
        raise source.JournalConflict('creation requires exact delegation and absent source/revision')
    started_at, started_ns = datetime.now(timezone.utc).isoformat(), time.perf_counter_ns()
    subject, files, dependencies, _ = _prepare_create(config, context, path, request)
    if request['expected_dependencies'] != dependencies:
        raise source.JournalConflict('owner-local creation dependencies are stale')
    source._capture_creation_provenance({**config, 'source_root': str(context.public_root)}, request, files,
        started_at, started_ns, procedure_name='owner-local-source-profile-metadata-serialization',
        additional_software_refs=IMPLEMENTATIONS[:1], owner_local_metadata=True)
    receipt = {'schema_version': 'tos_local_source_create_receipt_v1', 'command_id': request['command_id'],
        'request_digest': source._digest(source._canonical(request)), 'principal_id': config['principal_id'],
        'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
        'recorded_at': datetime.now(timezone.utc).isoformat(), 'source_path': config['source_path'],
        'source': subject.ref, 'dependencies': dependencies, 'files': revisions._file_refs(files), 'grants_admission': False}
    files[RECEIPT_FILE] = revisions._encode(receipt)
    staging = _stage(context, files)
    try:
        _current(owner, configuration_digest, path, config, context, request['record'], dependencies, creating=True)
        _directory(context, path.parent.parent)
        if _package(context, staging) != files:
            raise source.JournalConflict('owner-local staged creation changed')
        source._publish_new_directory(staging, path.parent)
    finally:
        revisions._discard_staging(staging, files)
    return _result(config, configuration_digest, path, state=_inspect(config, context, path), receipt=receipt)


def _revision_proposal(config, context, path, state, request):
    files, record, _, history, payload = state
    revisions._scope(config, request)
    if len(history['receipts']) >= revisions.MAX_REVISIONS:
        raise ValueError('source revision history capacity reached')
    revised = {**record, **request['fields'], 'record_version': record['record_version'] + 1}
    profiles, profile = _profiles(config, context)
    profiles.validate(profile['record_type'], revised)
    forms, views, refs = _forms(config, revised, payload, request['forms'], context, rebind=True)
    output = {**files, path.name: revisions._encode(revised), path.stem + '.human-forms.json': revisions._encode(forms)}
    if len(output[path.name]) > source.MAX_COMMAND_BYTES:
        raise ValueError('owner-local revised record exceeds its metadata budget')
    return revised, output, views, refs


def _update(owner, config, configuration_digest, context, path, request):
    state = _inspect(config, context, path)
    files, record, subject, history, payload = state
    operation = request['operation']
    dependencies = _dependencies(config, context, record, exclude=path.parent)
    # One command identity per owned subject, not separate reuse spaces for
    # form and record operations or for the original creation.
    other_receipts = (payload.get('growth_history', []) if operation == 'record.revise' else history['receipts'])
    creation = source._json_object(files[RECEIPT_FILE])
    if creation.get('command_id') == request['command_id'] or any(row['command_id'] == request['command_id'] for row in other_receipts):
        raise source.JournalConflict('owner-local command identity was used by another operation')
    receipts = history['receipts'] if operation == 'record.revise' else payload.get('growth_history', [])
    digest = source._digest(source._canonical(request))
    for receipt in receipts:
        if receipt['command_id'] == request['command_id']:
            if receipt['request_digest'] != digest:
                raise source.JournalConflict('owner-local command identity was reused')
            if (receipt['owner_configuration'] != configuration_digest
                    or request['expected_configuration'] != configuration_digest):
                raise source.JournalConflict('owner-local retry has stale delegation')
            if receipt['principal_id'] != config['principal_id'] or receipt['authority_ref'] != config['authority_ref']:
                raise source.JournalCorruption('owner-local receipt has another delegated actor or authority')
            if operation == 'record.revise':
                if receipt['dependencies'] != request['expected_dependencies']:
                    raise source.JournalCorruption('owner-local revision receipt differs from its original dependency evidence')
                _verify_revision_forms(context, config, path, receipt, payload)
            elif (receipt['results'] != [source._form_ref(change['form']) for change in request['changes']]
                    or receipt['source'] != request['expected_source']
                    or receipt['previous_revision'] != request['expected_revision']
                    or receipt['principal_id'] != config['principal_id'] or receipt['authority_ref'] != config['authority_ref']):
                raise source.JournalCorruption('owner-local form receipt differs from the exact replay request')
            response = _result(config, configuration_digest, path, state=state, receipt=receipt, replayed=True)
            _current(owner, configuration_digest, path, config, context, record, dependencies,
                     exclude=path.parent, expected_files=files)
            return response
    revision = revisions._revision(files)
    if (request['expected_configuration'] != configuration_digest or request['expected_source'] != subject.ref
            or request['expected_revision'] != revision or request['expected_dependencies'] != dependencies):
        raise source.JournalConflict('owner-local source, package or dependencies are stale')
    if operation == 'record.revise':
        revised, output, _, refs = _revision_proposal(config, context, path, state, request)
        proposed = source.metadata_subject(revised)
        archive = _archive_ref(context, config, revision)
        receipt = {'command_id': request['command_id'], 'request_digest': digest, 'principal_id': config['principal_id'],
            'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
            'recorded_at': datetime.now(timezone.utc).isoformat(), 'reason': request['reason'],
            'previous_source': subject.ref, 'source': proposed.ref, 'previous_revision': revision,
            'archive_path': archive.as_posix(), 'dependencies': dependencies,
            'changed_fields': sorted(request['fields']), 'forms': refs, 'grants_admission': False, 'request': request}
        output[revisions.HISTORY] = revisions._encode({**history, 'receipts': [*history['receipts'], receipt]})
        _archive(context, config, files, subject, revision)
    else:
        changes = _form_changes(request, config)
        value = source._apply(payload, subject, changes, validator=_form_grammar(context)[0])
        views = {view['form']['id']: view for view in _materialize(record, value, context)}
        for change in changes:
            if change['form']['content']['kind'] == 'source-copy' and views[change['form']['form_id']]['state'] != 'ready':
                raise ValueError('owner-local source-copy form omits mandatory context')
        receipt = {'command_id': request['command_id'], 'request_digest': digest, 'principal_id': config['principal_id'],
            'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
            'recorded_at': datetime.now(timezone.utc).isoformat(), 'source': subject.ref,
            'previous_revision': request['expected_revision'], 'results': [source._form_ref(change['form']) for change in changes]}
        value.setdefault('growth_history', []).append(receipt)
        source._validate_history(value, validator=_form_grammar(context)[0])
        output = {**files, path.stem + '.human-forms.json': revisions._encode(value)}
    staging = _stage(context, output)
    try:
        _current(owner, configuration_digest, path, config, context, record, dependencies, exclude=path.parent)
        if _package(context, path.parent) != files or _package(context, staging) != output:
            raise source.JournalConflict('owner-local package changed before exchange')
        revisions._exchange(staging, path.parent)
    finally:
        if staging.exists():
            remaining = _package(context, staging)
            # Exact previous bytes are archived for record changes. Form-only
            # exchange retains every prior form and receipt inside the new set.
            if remaining == files and (operation == 'apply' or _read_archive(context, config, receipt)[0] == files):
                revisions._discard_staging(staging, files)
            elif remaining == output:
                revisions._discard_staging(staging, output)
    return _result(config, configuration_digest, path, state=_inspect(config, context, path), receipt=receipt)

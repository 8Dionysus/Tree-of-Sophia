"""Explicit source-owner commands for forms, subjects, claims and native text.

The independently selected protected configuration delegates local-account
source writing, not semantic admission. Source prose cannot choose a path,
principal, grant or executable. Access remains read-only.
"""
from __future__ import annotations

from contextlib import contextmanager, nullcontext
import ctypes
from datetime import datetime, timezone
import errno
import fcntl
import hashlib
import json
import os
import platform
from pathlib import Path
import re
import stat
import sys
import tempfile
import time
import unicodedata
from jsonschema import Draft202012Validator, ValidationError, FormatChecker

from assessment_journal import (
    JournalBusy, JournalConflict, JournalCorruption, _json_object, _keys,
    _owned_path, _sync_directory,
)
from knowledge_assessment import Record, _canonical, _instant

ROOT = Path(__file__).resolve().parents[5]
if str(ROOT / 'scripts') not in sys.path:
    sys.path.insert(0, str(ROOT / 'scripts'))
from source_witness_human_forms import MAX_SET_BYTES, _validator, materialize_metadata_forms, metadata_field_catalog
from source_witness_human_forms import claim_field_catalog, claim_forms_path, materialize_claim_forms
from source_witness_human_forms import metadata_subject

OPERATIONS = ('form.create', 'form.revise')
CREATION_OPERATION = 'historical.create'
CREATION_CONFIGS = {'tos_local_historical_create_owner_v1', 'tos_local_historical_create_owner_v2'}
PROFILE_CONFIG = 'tos_local_profile_create_owner_v1'
SIGN_CONFIG = 'tos_local_sign_promote_owner_v1'
PROFILE_CREATION_CONFIGS = {PROFILE_CONFIG, SIGN_CONFIG}
CORPUS_CONFIG = 'tos_local_corpus_create_owner_v1'
CORPUS_REVISION_CONFIG = 'tos_local_corpus_revision_owner_v1'
CORPUS_SELECTED_REVISION_CONFIG = 'tos_local_corpus_revision_owner_v2'
REVISION_CONFIG = 'tos_local_source_revision_owner_v1'
PROFILE_REVISION_CONFIG = 'tos_local_profile_revision_owner_v1'
CLAIM_REVISION_CONFIG = 'tos_local_claim_revision_owner_v1'
CLAIM_CONFIG = 'tos_local_claim_create_owner_v1'
CLAIM_VALUE_CONFIG = 'tos_local_claim_create_owner_v2'
CLAIM_VALUE_REVISION_CONFIG = 'tos_local_claim_revision_owner_v2'
CLAIM_STRUCTURED_CONFIG = 'tos_local_claim_create_owner_v3'
CLAIM_STRUCTURED_REVISION_CONFIG = 'tos_local_claim_revision_owner_v3'
CLAIM_REFERENCE_CONFIG = 'tos_local_claim_create_owner_v4'
CLAIM_REFERENCE_REVISION_CONFIG = 'tos_local_claim_revision_owner_v4'
CLAIM_LAYER_REVISION_CONFIG = 'tos_local_claim_layer_revision_owner_v1'
CLAIM_FORM_CONFIG = 'tos_local_claim_form_owner_v1'
TEXT_UNIT_CONFIG = 'tos_local_text_unit_create_owner_v1'
OWNER_PROFILE_CONFIG = 'tos_local_owner_profile_command_v1'
OWNER_CLAIM_CONFIG = 'tos_local_owner_claim_command_v1'
OWNER_CLAIM_REFERENCE_CONFIG = 'tos_local_owner_claim_command_v2'
REVISION_FIELDS = {'preferred_label', 'variant_labels', 'notes', 'field_languages', 'source_refs', 'extensions',
                   'semantic_content'}
CORPUS_REVISION_FIELDS = {'preferred_label', 'notes', 'field_languages', 'source_refs'}
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
    if config.get('schema_version') == 'tos_local_work_expression_owner_v1':
        from source_expression_commands import configuration
        return configuration(config, owner_config=path)
    if config.get('schema_version') in {OWNER_CLAIM_CONFIG, OWNER_CLAIM_REFERENCE_CONFIG}:
        from source_owner_claim_commands import configuration
        return configuration(config, owner_config=path)
    if config.get('schema_version') == OWNER_PROFILE_CONFIG:
        from source_owner_profile_commands import configuration
        return configuration(config, owner_config=path)
    if config.get('schema_version') == TEXT_UNIT_CONFIG:
        from source_text_unit_commands import configuration
        return configuration(config, owner_config=path)
    if config.get('schema_version') in {CLAIM_CONFIG, CLAIM_VALUE_CONFIG, CLAIM_STRUCTURED_CONFIG, CLAIM_REFERENCE_CONFIG}:
        from source_claim_commands import configuration
        return configuration(config)
    if config.get('schema_version') in {CLAIM_REVISION_CONFIG, CLAIM_VALUE_REVISION_CONFIG, CLAIM_STRUCTURED_REVISION_CONFIG, CLAIM_REFERENCE_REVISION_CONFIG, CLAIM_LAYER_REVISION_CONFIG}:
        from claim_revisions import configuration
        return configuration(config)
    creation = config.get('schema_version') in CREATION_CONFIGS
    profile_creation = config.get('schema_version') in PROFILE_CREATION_CONFIGS
    sign_promotion = config.get('schema_version') == SIGN_CONFIG
    corpus_creation = config.get('schema_version') == CORPUS_CONFIG
    corpus_revision = config.get('schema_version') in {CORPUS_REVISION_CONFIG, CORPUS_SELECTED_REVISION_CONFIG}
    profile_revision = config.get('schema_version') == PROFILE_REVISION_CONFIG
    revision = config.get('schema_version') in {REVISION_CONFIG, PROFILE_REVISION_CONFIG, CORPUS_REVISION_CONFIG,
                                               CORPUS_SELECTED_REVISION_CONFIG}
    claim_forms = config.get('schema_version') == CLAIM_FORM_CONFIG
    captures_provenance = profile_creation or corpus_creation or config.get('schema_version') == 'tos_local_historical_create_owner_v2'
    _keys(config, {'schema_version', 'uid', 'principal_id', 'source_root', 'source_path',
                   'authority_ref', 'allowed_form_ids', 'allowed_operations', 'expires_at'}
          | ({'record_id', 'allowed_claim_ids', 'maker_type'} if creation else set())
          | ({'record_id', 'profile_type_id', 'maker_type'} if profile_creation else set())
          | ({'promotion_assessment_owner_config', 'promotion_candidate_id'} if sign_promotion else set())
          | ({'record_id', 'record_type', 'maker_type'} if corpus_creation else set())
          | ({'record_type'} if corpus_revision else set())
          | ({'record_id', 'allowed_fields'} if revision else set())
          | ({'profile_type_id'} if profile_revision else set())
          | ({'claim_id'} if claim_forms else set())
          | ({'provenance_event_id'} if captures_provenance else set()))
    if (config['schema_version'] not in {'tos_local_source_command_owner_v1', REVISION_CONFIG, PROFILE_REVISION_CONFIG, CORPUS_REVISION_CONFIG, CORPUS_SELECTED_REVISION_CONFIG, *PROFILE_CREATION_CONFIGS, CORPUS_CONFIG, CLAIM_FORM_CONFIG, *CREATION_CONFIGS}
            or type(config['uid']) is not int or config['uid'] != os.getuid()
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref'))
            or _instant(config['expires_at']) <= datetime.now(timezone.utc)):
        raise PermissionError('source-command delegation is invalid or expired')
    operations = (('record.revise', 'record.recover') if config['schema_version'] == CORPUS_SELECTED_REVISION_CONFIG
                  else ('sign.promote',) if sign_promotion else ('source.create',) if profile_creation or corpus_creation
                  else (CREATION_OPERATION,) if creation else ('record.revise',) if revision else OPERATIONS)
    for key, allowed in (('allowed_operations', operations), ('allowed_form_ids', None)):
        values = config[key]
        if (not isinstance(values, list) or len(values) > 32
                or any(not isinstance(value, str) for value in values)
                or len(set(values)) != len(values)
                or any(value not in allowed if allowed else not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value)
                       for value in values)):
            raise ValueError('invalid source-command delegation scope')
    if revision:
        values = config['allowed_fields']
        allowed_fields = CORPUS_REVISION_FIELDS if corpus_revision else REVISION_FIELDS
        if (not isinstance(values, list) or any(not isinstance(value, str) or value not in allowed_fields for value in values)
                or len(set(values)) != len(values)
                or not isinstance(config['record_id'], str)
                or not (profile_revision or corpus_revision) and not re.fullmatch(r'tos\.historical-(event|process|state)\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['record_id'])):
            raise ValueError('invalid source revision identity or field scope')
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
    if captures_provenance:
        if (not isinstance(config['provenance_event_id'], str)
                or not re.fullmatch(r'tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['provenance_event_id'])):
            raise ValueError('invalid delegated provenance identity')
    root = Path(config['source_root'])
    os.close(_owned_path(root, directory=True))
    relative = Path(config['source_path'])
    if (relative.is_absolute() or relative.as_posix() != config['source_path']
            or '..' in relative.parts or relative.parts[:2] != ('ToS', 'source-witnesses')
            or relative.is_relative_to('ToS/source-witnesses/owner-local')
            or any(part in ('payload', 'local-content', 'catalog') for part in relative.parts)
            or (relative.name != 'source-claims.jsonl' if claim_forms else relative.suffix != '.json')
            or relative.name.endswith('.human-forms.json')):
        raise PermissionError('source-command target must be explicit source metadata')
    if claim_forms:
        claim_forms_path(root / relative, config['claim_id'])
        _, _, contracts = _claim_form_source(root / relative, root, config['claim_id'])
        return config, _digest(_canonical({'configuration': config, 'source_contracts': contracts})), root / relative
    if not (creation or revision or profile_creation or corpus_creation) and relative.name in {
            'artifact-witness.json', 'composite-witness.json'}:
        _, _, contracts = _native_form_source(root / relative, root)
        return config, _digest(_canonical({'configuration': config, 'source_contracts': contracts})), root / relative
    if (creation or revision and not (profile_revision or corpus_revision)) and (relative.name != config['record_id'].split('.')[1] + '.json'
                     or len(relative.parts) < 5):
        raise PermissionError('historical creation requires its typed record in a new subject directory')
    if profile_creation or corpus_creation or profile_revision or corpus_revision:
        profile = _configured_corpus_profile(config) if corpus_creation or corpus_revision else _configured_profile(config)[1]
        if profile_creation:
            if (profile.get('creation_gate') == 'sign-promotion-v1') != sign_promotion:
                raise PermissionError('Sign identity requires its separately delegated promotion operation')
            if sign_promotion:
                if (config['profile_type_id'] != 'tos.entity.sign'
                        or not isinstance(config['promotion_candidate_id'], str)
                        or not re.fullmatch(r'tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['promotion_candidate_id'])
                        or not isinstance(config['promotion_assessment_owner_config'], str)):
                    raise PermissionError('Sign promotion requires its exact independently selected candidate and assessment owner')
                os.close(_owned_path(Path(config['promotion_assessment_owner_config'])))
        if (not isinstance(config['record_id'], str)
                or not re.fullmatch(re.escape(profile['id_prefix']) + r'[a-z0-9]+(?:[.-][a-z0-9]+)*', config['record_id'])
                or not revision and config['maker_type'] not in {'human', 'software', 'model'}
                or relative.name != profile['source_basename'] or len(relative.parts) < 5
                or 'catalog' in relative.parts):
            raise PermissionError('profile writing requires its delegated identity and typed source path')
        if not (corpus_creation or corpus_revision):
            profiles, _ = _configured_profile(config)
            profiles.validate_path(profile['record_type'], config['source_path'])
        if (corpus_creation and config['record_type'] == 'work'
                and relative.is_relative_to('ToS/source-witnesses/works/friedrich-nietzsche')):
            raise PermissionError('the Nietzsche Work source home requires its stronger authorship and chronology closure')
    if not (creation or revision or profile_creation or corpus_creation):
        inputs = _profile_form_inputs(root / relative, root)
        if inputs is not None:
            return config, _digest(_canonical({'configuration': config, **inputs})), root / relative
    return config, _digest(_canonical(config)), root / relative


def _configured_corpus_profile(config):
    """Existing standalone native identities, not role subclasses or a new ontology.

    A provisional Work can start with an explicitly empty expression-claim
    list. Realizations and mandatory related objects are not inferred or
    created by this metadata transaction.
    """
    kind = config.get('record_type')
    allowed = {'agent', 'place', 'organization', 'work'}
    if config.get('schema_version') == CORPUS_SELECTED_REVISION_CONFIG:
        allowed.add('expression')
    if not isinstance(kind, str) or kind not in allowed:
        raise PermissionError('native metadata writing requires Agent, Place, Organization or Work')
    return {'record_type': kind, 'id_prefix': f'tos.{kind}.', 'source_basename': kind + '.json',
            'schema_ref': 'ToS/contracts/corpus-record.schema.json', 'schema_version': 'tos_corpus_record_v1',
            'source_scope': 'public_metadata_only'}


def _configured_profile(config, profiles=None):
    from source_record_profiles import SourceRecordProfiles
    profiles = profiles or SourceRecordProfiles(Path(config['source_root']))
    entry = next((entry for entry in profiles.registry['types']
                  if entry['type_id'] == config['profile_type_id']), None)
    if entry is None or 'source_record_profile' not in entry:
        raise PermissionError('writing requires an explicitly declared source metadata profile')
    return profiles, entry['source_record_profile']


def _form_ref(value):
    return Record.from_payload(value['form_id'], value['form_version'], value).ref


def _validate_history(payload, *, validator=None):
    """Retained versions must form one complete, nonbranching chain per form."""
    (validator if validator is not None else _validator()).validate(payload)
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


def _claim_form_source(source_path, root, claim_id):
    """Select exactly one Claim from one bounded protected stream, not a corpus scan.

    Checks profile/schema/layer/visibility. Endpoint existence and semantic
    domain/range remain the source and graph validators' responsibility; a
    form write cannot change or admit the Claim.
    """
    from source_record_profiles import SourceClaimProfiles
    raw = _read(source_path, MAX_COMMAND_BYTES)
    claims = [_json_object(line) for line in raw.splitlines() if line.strip()]
    selected = [claim for claim in claims if claim.get('claim_id') == claim_id]
    if len(selected) != 1:
        raise ValueError('delegated Claim must resolve exactly once in the source stream')
    source = selected[0]
    profiles = SourceClaimProfiles(root)
    profiles.validate(source)
    total_bytes = len(raw)
    for ref, digest in profiles.input_digests.items():
        contract = _read(root / ref, MAX_COMMAND_BYTES)
        total_bytes += len(contract)
        if total_bytes > 8_388_608 or len(profiles.input_digests) > 128:
            raise ValueError('Claim form source and contracts exceed the bounded input snapshot')
        if hashlib.sha256(contract).hexdigest() != digest:
            raise JournalConflict('Claim form source contract changed while resolving')
    return raw, source, {ref: 'sha256:' + digest for ref, digest in profiles.input_digests.items()}


def _native_form_source(source_path, root):
    from build_source_witness_catalog import native_witness_contract
    raw = _read(source_path, MAX_COMMAND_BYTES)
    source = _json_object(raw)
    schema_ref, _, _ = native_witness_contract(source, source_path.relative_to(root).as_posix())
    schema_raw = _read(root / schema_ref, MAX_COMMAND_BYTES)
    if not Draft202012Validator(_json_object(schema_raw), format_checker=FormatChecker()).is_valid(source):
        raise ValueError('native form source violates its exact public metadata schema')
    return raw, source, {schema_ref: _digest(schema_raw)}


def _profile_input_snapshot(profiles):
    """Protected profile closure with an opaque, separately held native hash."""
    native_snapshot = profiles.native_identity_snapshot(read_bytes=_read, only_if_used=True)
    native_text_snapshot = profiles.native_text_snapshot(read_bytes=_read)
    total = 0
    for ref, digest in profiles.input_digests.items():
        raw = _read(profiles.root / ref, MAX_COMMAND_BYTES)
        total += len(raw)
        if total > 8_388_608 or len(profiles.input_digests) > 128:
            raise ValueError('source profile contracts exceed the bounded input snapshot')
        if hashlib.sha256(raw).hexdigest() != digest:
            raise JournalConflict('source profile contract changed during resolution')
    return {'source_contracts': {ref: 'sha256:' + digest for ref, digest in profiles.input_digests.items()},
            **({'native_binding_implementation': {
                ref: _digest(_read(ROOT / ref, MAX_COMMAND_BYTES)) for ref in
                ('scripts/native_text_binding.py', 'scripts/source_owner_context.py')}}
               if native_text_snapshot is not None else {}),
            **({'native_text_binding_snapshot': native_text_snapshot} if native_text_snapshot is not None else {}),
            **({'native_semantic_identity_snapshot': native_snapshot} if native_snapshot is not None else {})}


def _profile_form_inputs(source_path, root, source=None):
    source = _json_object(_read(source_path, MAX_COMMAND_BYTES)) if source is None else source
    if (source.get('schema_version') in {'tos_corpus_record_v1', 'tos_historical_record_v1'}
            and source_path.name != 'composite.json'):
        return None  # Existing native metadata adapters retain their own contracts.
    from source_record_profiles import SourceRecordProfiles
    profiles = SourceRecordProfiles(root)
    kind = source.get('record_type')
    if kind not in profiles.profiles or source_path.name != profiles.profiles[kind]['source_basename']:
        raise ValueError('source-command adapter does not understand this source family')
    profiles.validate_path(kind, source_path.relative_to(root).as_posix())
    profiles.validate(kind, source)
    return _profile_input_snapshot(profiles)


def _snapshot(source_path, root=None, claim_id=None):
    if claim_id is not None:
        source_raw, source, _ = _claim_form_source(source_path, root, claim_id)
        subject = Record.from_payload(source['claim_id'], source['claim_version'], source)
        target = claim_forms_path(source_path, claim_id)
    elif source_path.name in {'artifact-witness.json', 'composite-witness.json'}:
        if root is None:
            raise ValueError('native witness forms require the explicit source owner')
        source_raw, source, _ = _native_form_source(source_path, root)
        target = source_path.with_name(source_path.stem + '.human-forms.json')
    else:
        source_raw = _read(source_path, MAX_COMMAND_BYTES)
        source = _json_object(source_raw)
        target = source_path.with_name(source_path.stem + '.human-forms.json')
    if claim_id is None and source_path.name not in {'artifact-witness.json', 'composite-witness.json'} and (source.get('schema_version') not in {'tos_corpus_record_v1', 'tos_historical_record_v1'}
                             or source_path.name == 'composite.json'):
        if root is None:
            raise ValueError('source-command adapter does not understand this source family')
        _profile_form_inputs(source_path, root, source)
    if (source.get('schema_version') == 'tos_historical_record_v1'
            and source.get('visibility') not in {'public', 'public_metadata_only'}):
        raise PermissionError('historical source visibility is outside the public-metadata adapter')
    if claim_id is None:
        subject = metadata_subject(source)
    try:
        raw = _read(target, MAX_SET_BYTES)
    except FileNotFoundError:
        raw = None
    payload = _json_object(raw) if raw is not None else None
    if payload is not None:
        _validate_history(payload)
        if payload['subject']['id'] != subject.id:
            raise JournalCorruption('source and form set have different subjects')
    if _read(source_path, MAX_COMMAND_BYTES) != source_raw:
        raise JournalConflict('source changed while reading adjacent forms')
    return source_raw, source, subject, target, raw, payload


@contextmanager
def _locked(target, timeout=5.0, *, allow_pending=False):
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
            if (not allow_pending and target.name == 'historical-create'
                    and target.parent.name == 'source-witnesses' and target.parent.parent.name == 'ToS'):
                # Check after acquiring the shared writer lock, not before
                # waiting for a possibly interrupted selected-file writer.
                from source_metadata_snapshot import PublicationSnapshot
                PublicationSnapshot(target.parents[2])
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


def _apply(payload, subject, changes, *, validator=None):
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
    _validate_history(value, validator=validator)
    return value


def prepare_metadata_change(source, payload, principal_id, form_id, field_id):
    """Construct a proposal from the reader's finite field catalog; grant nothing."""
    subject = metadata_subject(source)
    return _prepare_form_change(subject, metadata_field_catalog(source), payload, principal_id, form_id, field_id)


def prepare_claim_change(source, payload, principal_id, form_id, field_id):
    subject = Record.from_payload(source['claim_id'], source['claim_version'], source)
    return _prepare_form_change(subject, claim_field_catalog(source), payload, principal_id, form_id, field_id)


def _prepare_form_change(subject, fields, payload, principal_id, form_id, field_id):
    field = next((field for field in fields if field['field_id'] == field_id), None)
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


def _creation_scope(config, request):
    """Current delegated IDs and maker apply even to an exact old retry."""
    source, claims, selections = request['record'], request.get('claims', []), request['forms']
    if not isinstance(source, dict) or source.get('record_id') != config['record_id']:
        raise PermissionError('source subject identity is not delegated')
    if not isinstance(claims, list) or len(claims) > 32:
        raise ValueError('at most thirty-two initial claims are supported')
    if not isinstance(selections, list) or not 1 <= len(selections) <= 32:
        raise ValueError('one to thirty-two source-copy selections are required')
    for claim in claims:
        if (not isinstance(claim, dict) or claim.get('claim_id') not in config['allowed_claim_ids']
                or not isinstance(claim.get('maker'), dict)
                or claim['maker'].get('agent_ref') != config['principal_id']
                or claim['maker'].get('maker_type') != config['maker_type']
                or (config.get('provenance_event_id')
                    and claim.get('provenance_event_ref') != config['provenance_event_id'])):
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


def _sign_promotion(config, *, require_ready=True):
    """Read a current source-bound judgment; never trust a submitted verdict.

    Issuance is separately delegated by SIGN_CONFIG. This public metadata
    operation cannot consume inline targets or confidential assessment roots.
    Its small returned basis records the past check, not continuing admission.
    """
    from assessment_journal import run_local_command as assessment_command, _source_records
    owner = Path(config['promotion_assessment_owner_config'])
    encoded = _read(owner, 8 * MAX_COMMAND_BYTES)
    assessment_config = _json_object(encoded)
    root = Path(config['source_root'])
    if (assessment_config.get('schema_version') not in {'tos_local_assessment_owner_v2', 'tos_local_assessment_owner_v3'}
            or assessment_config.get('source_root') != str(root)):
        raise PermissionError('Sign promotion needs the same public source root and a source-bound assessment owner')
    candidate_id = config['promotion_candidate_id']
    selected = [row for row in assessment_config.get('source_records', [])
                if isinstance(row, dict) and row.get('record_id') == candidate_id]
    if len(selected) != 1:
        raise PermissionError('Sign promotion candidate must be one selected authored Claim, not an inline record')
    # The Claim's typed endpoints and all motif members belong to the same
    # configured closure. Selecting only its row would discard the grounds
    # before the shared adapter can validate domain/range and completeness.
    records, _ = _source_records(root, assessment_config['source_records'])
    candidate = next((row for row in records if row['id'] == candidate_id), None)
    body = candidate['payload'] if candidate else {}
    if (body.get('schema_version') != 'tos_source_occurrence_motif_claim_v1'
            or body.get('predicate') != 'occurrence_motif_proposal'
            or body.get('assertion_layer') != 'semantic_interpretation'
            or body.get('visibility') not in {'public', 'public_metadata_only'}):
        raise PermissionError('this Sign transition requires an exact qualified public motif Claim')
    view = assessment_command(owner, {'schema_version': 'tos_local_assessment_command_v1',
        'operation': 'describe', 'subject_id': candidate_id}, contract_root=root,
        accepted_owner_versions=frozenset({'tos_local_assessment_owner_v2', 'tos_local_assessment_owner_v3'}))
    result = view['result']
    context, admission = result['command_context'], result['current_admission']
    expected = Record.from_payload(**candidate).ref
    if (_read(owner, 8 * MAX_COMMAND_BYTES) != encoded or context['subject'] != expected
            or admission['subject'] != expected):
        raise JournalConflict('Sign candidate or assessment owner changed during resolution')
    if (context['scope']['requested_use'] != 'sign-promotion'
            or context['scope']['risk'] not in {'moderate', 'high'}
            or context['scope']['assertion_layer'] != 'semantic_interpretation'):
        raise PermissionError('research admission or a lowered risk does not delegate Sign promotion')
    ready = (admission['use'] == 'sign-promotion' and admission['can_use'] is True
             and admission['status'] in {'admitted', 'admitted-with-limits'}
             and bool(admission['assessment_refs']) and result['revision'] is not None
             and bool(context.get('required_sources'))
             and context.get('source_read', {}).get('ready') is True)
    basis = None
    if ready:
        basis = {'schema_version': 'tos_sign_promotion_basis_v1', 'candidate': expected,
            'policy': admission['policy'], 'required_sources': context['required_sources'],
            'assessment_refs': admission['assessment_refs'], 'owner_snapshot': view['owner_snapshot'],
            'journal_revision': result['revision'], 'status': admission['status'],
            'use': 'sign-promotion', 'limits': admission['limits'], 'grants_current_use': False}
    elif require_ready:
        raise PermissionError('Sign promotion lacks current qualified assessment and exact source reading')
    return {'eligible': ready, 'basis': basis, 'current_admission': admission,
            'grants_issuance_authority': False}


@contextmanager
def _sign_promotion_lock(config):
    """Serialize final current judgment -> issuance with normal journal writes.

    Lock order is corpus creation then the selected assessment subject. The
    assessment journal does not acquire the corpus lock. A later withdrawal
    remains valid history, but cannot precede issuance after its final check.
    External configuration/source publishers retain their ordinary obligation
    to keep owner inputs stable during the operation (same-UID trust boundary).
    """
    from assessment_journal import AssessmentJournal
    owner = Path(config['promotion_assessment_owner_config'])
    encoded = _read(owner, 8 * MAX_COMMAND_BYTES)
    selected = _json_object(encoded)
    if (selected.get('schema_version') not in {'tos_local_assessment_owner_v2', 'tos_local_assessment_owner_v3'}
            or selected.get('source_root') != config['source_root']):
        raise PermissionError('Sign publication requires the same public assessment owner')
    directory = Path(selected['journal_directory'])
    os.close(_owned_path(directory, directory=True))
    journal = AssessmentJournal(directory, contract_root=Path(config['source_root']), protected_storage=True)
    with journal._locked(journal._home(config['promotion_candidate_id'])):
        if _read(owner, 8 * MAX_COMMAND_BYTES) != encoded:
            raise JournalConflict('Sign assessment owner changed while waiting for its journal')
        yield


def _initial_source_record(config, source, profiles=None):
    if config['schema_version'] == CORPUS_CONFIG:
        profile = _configured_corpus_profile(config)
        from source_record_profiles import METADATA_LINK_FIELDS
        schema = _json_object(_read(Path(config['source_root']) / profile['schema_ref'], MAX_COMMAND_BYTES))
        Draft202012Validator.check_schema(schema)
        Draft202012Validator(schema).validate(source)
        # Corpus requires this field for Work. An empty initial list records
        # no supplied expression assertions; it does not mean none exist.
        link_fields = set(METADATA_LINK_FIELDS)
        if profile['record_type'] == 'work':
            work_fields = {'schema_version', 'record_type', 'record_id', 'record_version', 'preferred_label',
                'variant_labels', 'field_languages', 'identity_status', 'source_refs', 'external_identifiers',
                'same_as_posture', 'notes', 'supersedes_ref', 'expression_claim_refs'}
            if set(source) - work_fields:
                raise PermissionError('initial Work creation owns identity metadata, not realization or publication fields')
            if source['expression_claim_refs'] != []:
                raise PermissionError('initial standalone Work cannot assert expression closure')
            link_fields.remove('expression_claim_refs')
        if (source['record_type'] != profile['record_type'] or source['record_id'] != config['record_id']
                or source['record_version'] != 1 or source.get('supersedes_ref') is not None
                or source['identity_status'] != 'provisional' or source['same_as_posture'] != 'no_equivalence_claim'
                or any(key in source for key in link_fields)
                or any(value['status'] != 'unverified' for key in ('variant_labels', 'external_identifiers')
                       for value in source.get(key, []))):
            raise PermissionError('native creation requires a provisional standalone identity without accepted attributions')
        return Record.from_payload(source['record_id'], 1, source)
    if config['schema_version'] not in PROFILE_CREATION_CONFIGS:
        return _initial_historical_record(config, source)
    profiles, profile = _configured_profile(config, profiles)
    if profile.get('creation_gate'):
        if config['schema_version'] != SIGN_CONFIG or profile['creation_gate'] != 'sign-promotion-v1':
            raise PermissionError('this source profile requires its explicit creation gate')
        if _canonical(source.get('promotion_basis')) != _canonical(_sign_promotion(config)['basis']):
            raise JournalConflict('Sign description must retain the exact current promotion basis')
    profiles.validate(profile['record_type'], source)
    if (source['record_id'] != config['record_id'] or source['record_version'] != 1
            or source.get('supersedes_ref') is not None or source['identity_status'] != 'provisional'
            or source['same_as_posture'] != 'no_equivalence_claim'):
        raise PermissionError('creation requires a delegated provisional initial identity')
    profiles.validate_native_binding(profile['record_type'], source, verify_content=True)
    return Record.from_payload(source['record_id'], 1, source)


def _prepare_creation(config, request):
    """Shared initial metadata serialization; historical claims stay optional.

    The historical route alone admits initial claims under its own contract.
    A profile declaration neither selects a writer nor authorizes claims.
    """
    from build_source_witness_catalog import collect_records, collect_claims
    from source_record_profiles import SourceRecordProfiles
    from source_witness_bibliographic_graph_common import (
        _historical_claim_contract, _validate_historical_claim,
        _scan_index, _evidence_node, BibliographicGraphBuildError,
    )
    root = Path(config['source_root'])
    source, claims, selections = request['record'], request.get('claims', []), request['forms']
    profiles = SourceRecordProfiles(root)
    subject = _initial_source_record(config, source, profiles)
    profiles.assert_identity_not_native(source['record_id'])
    records = collect_records(root, profiles=profiles)
    claim_profile_inputs = {}
    existing_claims = collect_claims(root, input_digests=claim_profile_inputs)
    objects = {row['record_id']: row for rows in records.values() for row in rows}
    if source['record_id'] in objects:
        raise JournalConflict('subject identity already exists in authored sources')
    if config['schema_version'] == SIGN_CONFIG:
        for row in records.get('sign', []):
            prior = profiles.load('sign', row['source_record_ref'])
            if prior['promotion_basis']['candidate']['id'] == config['promotion_candidate_id']:
                raise JournalConflict('candidate already has a Sign identity; revision or explicit lineage is required')
    form_inputs = {}
    new_form_ids = {selection['form_id'] for selection in selections}
    adjacent_forms = set()
    for row in objects.values():
        path = root / row['source_record_ref']
        adjacent_forms.add(path.with_name(path.stem + '.human-forms.json'))
    from source_record_profiles import SOURCE_CLAIM_BASENAME
    for claim in existing_claims:
        path = root / claim['source_claim_file_ref']
        if path.name == SOURCE_CLAIM_BASENAME:
            adjacent_forms.add(claim_forms_path(path, claim['claim_id']))
    for adjacent in sorted(adjacent_forms):
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
    new_event = config.get('provenance_event_id')
    if new_event in events:
        raise JournalConflict('provenance identity already exists')
    anchors = _scan_index(root, filename_pattern='*anchor*.jsonl', id_field='anchor_id')
    contract = _historical_claim_contract(root) if claims else None
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
            if new_event and claim['provenance_event_ref'] != new_event:
                raise PermissionError('new claims must bind the delegated creation provenance')
            if claim['provenance_event_ref'] not in events and claim['provenance_event_ref'] != new_event:
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
    files = {filename: encode(source), filename[:-5] + '.human-forms.json': encode(forms)}
    if config['schema_version'] in CREATION_CONFIGS:
        files['historical-claims.jsonl'] = b''.join(_canonical(claim) + b'\n' for claim in claims)
    if any(len(raw) > MAX_SET_BYTES for raw in files.values()):
        raise ValueError('initial source file exceeds its byte budget')
    provenance_contract = ({'ToS/contracts/provenance-event-v2.schema.json':
        _digest(_read(root / 'ToS/contracts/provenance-event-v2.schema.json', MAX_SET_BYTES))} if new_event else {})
    dependencies = _digest(_canonical({'records': records, 'claims': existing_claims,
        'source_profiles': profiles.input_digests,
        'native_semantic_identity_snapshot': profiles.native_identity_snapshot(read_bytes=_read),
        'native_text_binding_snapshot': profiles.native_text_snapshot(read_bytes=_read),
        'source_claim_profiles': claim_profile_inputs,
        **({'promotion_implementation': _digest(_read(ROOT /
            'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py', MAX_SET_BYTES))}
           if config['schema_version'] == SIGN_CONFIG else {}),
        'provenance_contract': provenance_contract,
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
            'scripts/source_record_profiles.py',
            'scripts/native_text_binding.py',
            'scripts/source_owner_context.py',
            'scripts/source_witness_bibliographic_graph_common.py',
            'ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
            'ToS/contracts/human-form-template.schema.json')}}))
    return subject, files, dependencies


def _validator_for_provenance(root):
    return Draft202012Validator(_json_object(_read(
        root / 'ToS/contracts/provenance-event-v2.schema.json', MAX_SET_BYTES)))


def _capture_creation_provenance(config, request, files, started_at, started_ns,
                                 *, procedure_name=None, additional_software_refs=(),
                                 native_inputs=None, owner_local_metadata=False):
    """Capture buffer serialization, not upstream research or future publication.

    The event and its hash-bearing receipt travel with the atomic directory.
    Output digests describe serialized buffers, not an independent disk audit.
    """
    base = Path(config['source_path']).parent
    script_ref = Path(__file__).resolve().relative_to(ROOT).as_posix()
    script_hash = _digest(_read(ROOT / script_ref, MAX_SET_BYTES))[7:]
    runtime = Path(sys.executable).resolve()
    with runtime.open('rb') as stream:
        runtime_hash = hashlib.file_digest(stream, 'sha256').hexdigest()
    environment = {'runtime': platform.python_implementation(),
        'runtime_version': platform.python_version(), 'runtime_artifact_sha256': runtime_hash,
        'backend': 'python-standard-library-and-jsonschema', 'hardware_target': 'cpu',
        'unicode_version': unicodedata.unidata_version}
    original_outputs = dict(files)
    files['source-create-request.json'] = _canonical(request) + b'\n'
    files['source-create-environment.json'] = _canonical(environment) + b'\n'
    binding = lambda name: {'ref': (base / name).as_posix(), 'sha256': _digest(files[name])[7:]}
    ended_at = datetime.now(timezone.utc).isoformat()
    def entity(name, raw, role):
        return {'entity_ref': (base / name).as_posix(), 'role': role,
            'sha256': _digest(raw)[7:], 'size_bytes': len(raw),
            'media_type': 'application/x-ndjson' if name.endswith('.jsonl') else 'application/json',
            'availability': 'owner_local', 'content_disclosure': 'public_metadata_only',
            'fixity_verified': False, 'fixity_verified_at': None}
    event = {
        '$schema': 'https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json',
        'schema_version': 'tos_provenance_event_v2', 'event_id': config['provenance_event_id'],
        'event_version': 1, 'supersedes_event_ref': None,
        'record_binding': {'manifest_ref': (base / 'source-create-receipt.json').as_posix(),
            'digest_algorithm': 'sha256', 'digest_scope': 'exact_event_record_bytes'},
        'activity': {'event_type': 'annotation', 'started_at': started_at, 'ended_at': ended_at,
            'status': 'completed_with_warnings', 'terminal_reason': None, 'exit_code': 0,
            'warnings': ['Completed serialization only; atomic publication occurs afterward.',
                         'Buffer hashes are captured; independent stored-byte fixity is not attested.']},
        'entities': {'inputs': [entity('source-create-request.json', files['source-create-request.json'],
                                      'caller-supplied-metadata-request')],
            'outputs': [entity(name, raw, 'serialized-source-metadata') for name, raw in original_outputs.items()],
            'byproducts': [entity('source-create-environment.json', files['source-create-environment.json'],
                                 'runtime-description')]},
        'derivations': [{'derivation_id': config['provenance_event_id'].replace('tos.event.', 'tos.derivation.', 1) + f'.output-{index}',
            'input_entity_ref': (base / 'source-create-request.json').as_posix(),
            'output_entity_ref': (base / name).as_posix(), 'relation': 'was_derived_from',
            'influence_asserted': True,
            'description': 'Technical metadata selection and serialization from the supplied request, not historical influence.'}
            for index, name in enumerate(original_outputs)],
        'responsibility': [{'agent_ref': 'software:tos-source-commands', 'agent_kind': 'software',
            'role': 'executor', 'responsibility_posture': 'performed',
            'evidence_binding': {'ref': script_ref, 'sha256': script_hash},
            'human_evidence_status': 'not_applicable'}],
        'method': {'procedure': {'name': (procedure_name or ('source-profile-metadata-serialization' if config['schema_version'] == PROFILE_CONFIG
                                        else 'historical-source-metadata-serialization')), 'version': '2',
            'purpose': 'Serialize supplied source records without judging their content.' if procedure_name else
                       'Serialize supplied source metadata and source-copy forms without judging their content.'},
            'command_capture': {'disclosure': 'withheld_digest_only', 'argv': None,
                'argv_sha256': _digest(_canonical(sys.argv))[7:],
                'withholding_reason': 'Process argv can contain private owner configuration paths; request is bound separately.'},
            'configuration_binding': binding('source-create-request.json'),
            'software_components': [{'name': 'ToS source commands', 'version': '2', 'role': 'serialization-runner',
                'artifact_ref': script_ref, 'artifact_sha256': script_hash, 'verification_status': 'verified'},
                {'name': environment['runtime'], 'version': environment['runtime_version'], 'role': 'language-runtime',
                 'artifact_ref': 'runtime:python-executable', 'artifact_sha256': runtime_hash, 'verification_status': 'verified'}],
            'model_invocations': [],
            'environment': {**environment, 'environment_profile_binding': binding('source-create-environment.json')}},
        'manual_changes': {'status': 'none_declared', 'change_receipts': [],
            'statement': 'No manual editing inside this serialization operation; caller authorship is outside its scope.'},
        'measurements': [{'metric': 'wall_duration_ms', 'status': 'measured',
            'value': (time.perf_counter_ns() - started_ns) / 1_000_000, 'unit': 'ms',
            'method': 'perf_counter_ns from pre-serialization validation through capture; excludes staging and commit.',
            'evidence_binding': None}],
        'evidence_authentication': {'capture_posture': 'tool_captured', 'signature_status': 'unsigned',
            'signature_bindings': [], 'verification_status': 'unverified',
            'producer_control_boundary': 'The same unsigned process serializes and records; hashes do not authenticate execution truth.'},
        'rights_and_visibility': {'rights_record_bindings': [], 'intended_uses': ['local_research', 'public_metadata'],
            'content_visibility': 'tracked_public_metadata', 'publication_authorized': False, 'publication_authority_bindings': []},
        'review_and_authority': {'mechanical_validation': 'not_run', 'human_review_status': 'not_performed',
            'review_bindings': [], 'accepted_uses': [], 'promotion_authorized': False, 'competence_evidence_bindings': []},
        'reproducibility': {'classification': 'partially_specified',
            'known_gaps': ['Upstream research, source reading and model invocations are not captured by this operation.',
                           'Dependencies are bound by the creation receipt; no complete runtime environment is archived.',
                           'Runtime timestamps and durations are not deterministic.'],
            'replay_scope': 'Supplied JSON and source-copy serialization only; not historical or semantic correctness.'},
        'authority_boundary': {'validator_role': 'mechanics_and_closure_only_not_truth',
            'claims_not_established': ['execution_truth', 'content_truth', 'source_fidelity', 'translation_quality',
                'semantic_correctness', 'rights_clearance', 'human_review', 'publication_authority', 'canon_authority']}}
    # Internal source-module refs, never request/config-selected executables.
    for ref in additional_software_refs:
        event['method']['software_components'].append({'name': ref, 'version': '1',
            'role': 'source-command-adapter', 'artifact_ref': ref,
            'artifact_sha256': _digest(_read(ROOT / ref, MAX_SET_BYTES))[7:],
            'verification_status': 'verified'})
    if native_inputs is not None:
        # Internal native adapter only. Source reading/anchor construction is
        # not metadata-only serialization; its whole receipt stays private.
        event['activity']['event_type'] = 'segmentation'
        event['activity']['warnings'][0] = (
            'Exact representation and native metadata verified; proposed anchors computed; '
            'atomic publication occurs afterward and no linguistic assessment is performed.')
        event['entities']['inputs'].extend(native_inputs['entities'])
        for group in event['entities'].values():
            for item in group:
                item['content_disclosure'] = 'private_content'
        event['method']['procedure']['purpose'] = (
            'Validate a delegated interval partition against exact source bytes and construct '
            'a new method-proposed native packet without accepting its linguistic analysis.')
        event['method']['configuration_binding'] = binding('source-create-owner-configuration.json')
        event['rights_and_visibility'].update(
            rights_record_bindings=native_inputs['rights'], intended_uses=['local_research'],
            content_visibility='local_only')
        for index, item in enumerate(native_inputs['entities']):
            event['derivations'].append({
                'derivation_id': config['provenance_event_id'].replace('tos.event.', 'tos.derivation.', 1) + f'.native-{index}',
                'input_entity_ref': item['entity_ref'],
                'output_entity_ref': config['source_path'], 'relation': 'selection_from',
                'influence_asserted': True,
                'description': 'Technical exact-source dependency for proposed native anchors, not historical influence.'})
        event['reproducibility']['known_gaps'][0] = (
            'Upstream transcription and caller linguistic analysis are not executed or authenticated here; '
            'the exact native source closure is bound by the command dependency snapshot.')
        event['reproducibility']['replay_scope'] = (
            'Exact source-byte verification and deterministic packet construction from the retained '
            'delegation/request, not linguistic correctness or deterministic provenance timestamps.')
    if owner_local_metadata:
        # Explicit internal private metadata adapter, never a caller flag.
        # This records serialization; it does not pretend to segment text.
        for group in event['entities'].values():
            for item in group:
                item['content_disclosure'] = 'private_content'
        event['method']['configuration_binding'] = binding('source-create-owner-configuration.json')
        event['rights_and_visibility'].update(intended_uses=['local_research'], content_visibility='local_only')
    _validator_for_provenance(Path(config['source_root'])).validate(event)
    files['source-create-provenance.jsonl'] = _canonical(event) + b'\n'


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


def _creation_replay(config, source_path, request, receipt):
    """Verify the original creation while permitting separately retained revisions.

    This is historical byte/lineage evidence, not current admission. The
    existing revision owner reads archives; no new history format is invented.
    """
    from source_revisions import _package, _selected_package, _selected_names, _history, _read_archive, HISTORY, _encode
    fields = {'schema_version', 'command_id', 'request_digest', 'principal_id', 'authority_ref',
              'owner_configuration', 'recorded_at', 'source_path', 'source', 'dependencies', 'files', 'grants_admission'}
    schema = ('tos_local_historical_create_receipt_v1' if config['schema_version'] in CREATION_CONFIGS
              else 'tos_local_source_create_receipt_v1')
    original = Record.from_payload(request['record']['record_id'], request['record']['record_version'], request['record'])
    if (set(receipt) != fields or receipt['schema_version'] != schema
            or receipt['principal_id'] != config['principal_id']
            or receipt['authority_ref'] != config['authority_ref']
            or receipt['owner_configuration'] != request['expected_configuration']
            or receipt['dependencies'] != request['expected_dependencies']
            or receipt['source'] != original.ref or receipt['grants_admission'] is not False):
        raise JournalCorruption('creation receipt no longer binds its original request')
    _instant(receipt['recorded_at'])
    formname = source_path.stem + '.human-forms.json'
    expected_files = {source_path.name, formname}
    if config['schema_version'] in CREATION_CONFIGS:
        expected_files.add('historical-claims.jsonl')
    if config.get('provenance_event_id'):
        expected_files.update({'source-create-request.json', 'source-create-environment.json', 'source-create-provenance.jsonl'})
    if not isinstance(receipt['files'], dict) or set(receipt['files']) != expected_files:
        raise JournalCorruption('creation receipt file closure changed')
    history_path = source_path.parent / HISTORY
    selected_history = (os.path.lexists(history_path)
        and _json_object(_read(history_path, MAX_SET_BYTES)).get('schema_version') == 'tos_source_revision_history_v2')
    if selected_history:
        # Later explicitly selected corrections may coexist with descendants.
        # Rechecking a historical creation reads only its exact original files;
        # it does not retrospectively widen the old creation write grant.
        files = _selected_package(source_path)
        for name in sorted(expected_files | {'source-create-receipt.json'}):
            if name not in files:
                files[name] = _read(source_path.parent / name, MAX_SET_BYTES)
    else:
        files = _package(source_path.parent)
    extras = set(files) - expected_files - {'source-create-receipt.json'}
    lockname = '.' + formname + '.writer.lock'
    if extras - {HISTORY, lockname} or files.get(lockname, b''):
        raise JournalCorruption('creation package contains unbound files')
    if not expected_files <= files.keys():
        raise JournalCorruption('creation package is incomplete')
    record = _json_object(files[source_path.name])
    history = _history(files, record)
    original_files = files
    for index, revision in enumerate(history['receipts']):
        archived, _ = _read_archive(Path(config['source_root']), config, revision)
        selected_revision = 'publication' in revision
        if selected_revision and (
                revision['publication']['selected_files'] != sorted(_selected_names(source_path))
                or not set(archived) <= set(_selected_names(source_path))):
            raise JournalCorruption('retained correction exceeds its selected metadata scope')
        if not selected_revision and archived.get('source-create-receipt.json') != files['source-create-receipt.json']:
            raise JournalCorruption('creation receipt changed across source history')
        if index == 0:
            if revision['previous_source'] != original.ref:
                raise JournalCorruption('source history does not start at the created record')
            original_files = archived
    if not history['receipts'] and Record.from_payload(record['record_id'], record['record_version'], record).ref != original.ref:
        raise JournalCorruption('created source changed without retained revision history')
    forms = _json_object(files[formname])
    _validate_history(forms)
    if forms['subject']['id'] != original.id:
        raise JournalCorruption('created form set belongs to another subject')
    initial_forms = {(form['form_id'], form['form_version']): form for form in [*forms['forms'], *forms['prior_forms']]}
    selected = [initial_forms.get((selection['form_id'], 1)) for selection in request['forms']]
    if any(form is None or form['subject'] != original.ref for form in selected):
        raise JournalCorruption('initial forms are not retained against the created source')
    # The initial serializer's ordering is part of its exact byte contract;
    # later form revisions canonicalize dictionaries while retaining values.
    initial_changes = [prepare_metadata_change(request['record'], None, receipt['principal_id'], **selection)
                       for selection in request['forms']]
    retained_set = _apply(None, original, initial_changes)
    if selected != retained_set['forms']:
        raise JournalCorruption('retained initial forms no longer match the source-copy request')
    for name, binding in receipt['files'].items():
        raw = original_files[name] if name == source_path.name else files[name]
        if name == formname and (forms['prior_forms'] or forms.get('growth_history')):
            raw = _encode(retained_set)
        if binding != {'sha256': _digest(raw), 'bytes': len(raw)}:
            raise JournalCorruption('creation output bytes no longer match their retained binding')
    if config.get('provenance_event_id') and _json_object(files['source-create-request.json']) != request:
        raise JournalCorruption('captured creation request changed')
    _snapshot(source_path, Path(config['source_root']))  # Fresh source visibility/profile checks, not admission.


def _create_source(owner_config, config, configuration, source_path, request):
    profile_creation = config['schema_version'] in {*PROFILE_CREATION_CONFIGS, CORPUS_CONFIG}
    corpus_creation = config['schema_version'] == CORPUS_CONFIG
    creation_operation = 'sign.promote' if config['schema_version'] == SIGN_CONFIG else 'source.create' if profile_creation else CREATION_OPERATION
    claim_fields = set() if profile_creation else {'claims'}
    fields = {'schema_version', 'operation'}
    if request.get('operation') == creation_operation:
        fields |= {'command_id', 'expected_configuration', 'expected_source', 'expected_revision',
                   'expected_dependencies', 'record', 'forms'} | claim_fields
    elif request.get('operation') == 'prepare':
        fields |= {'record'}
    elif request.get('operation') == 'prepare-create':
        fields |= {'record', 'forms'} | claim_fields
    elif request.get('operation') != 'describe':
        raise ValueError('unsupported source creation command')
    _keys(request, fields)
    if request['schema_version'] != 'tos_local_source_command_v1':
        raise ValueError('unknown source command version')
    root, target = Path(config['source_root']), source_path.parent
    os.close(_owned_path(target.parent, directory=True))
    profile = (_configured_corpus_profile(config) if corpus_creation else _configured_profile(config)[1]) if profile_creation else None
    def result(receipt=None, replayed=False):
        return {'schema_version': ('tos_local_source_create_result_v1' if profile_creation else 'tos_local_historical_create_result_v1'),
            'authentication': 'local-unix-account', 'owner_configuration': configuration,
            'source_path': config['source_path'], 'record_id': config['record_id'],
            'target_exists': target.exists(), 'supported_operations': [creation_operation],
            'command_operations': ['describe', 'prepare', 'prepare-create', creation_operation],
            'allowed_operations': config['allowed_operations'],
            **({'source_profile': profile, **({'record_type': config['record_type']} if corpus_creation else
                                             {'profile_type_id': config['profile_type_id']})} if profile_creation else {
                'allowed_claim_ids': config['allowed_claim_ids'],
                'record_schema_ref': 'ToS/contracts/historical-record.schema.json',
                'claim_schema_ref': 'ToS/contracts/historical-claim.schema.json'}),
            'allowed_form_ids': config['allowed_form_ids'], 'expected_source': None, 'expected_revision': None,
            'creation_provenance_event_id': config.get('provenance_event_id'),
            'receipt': receipt, 'replayed': replayed, 'grants_admission': False}
    if request['operation'] == 'describe':
        response = result()
        if config['schema_version'] == SIGN_CONFIG:
            response['promotion'] = _sign_promotion(config, require_ready=False)
        return response
    if creation_operation not in config['allowed_operations']:
        raise PermissionError('source creation is not delegated')
    if request['operation'] == 'prepare':
        subject = _initial_source_record(config, request['record'])
        response = result()
        response['prepared_source'] = subject.ref
        response['source_fields'] = [{key: value for key, value in field.items() if key not in ('pointer', 'context')}
                                    for field in metadata_field_catalog(request['record'])]
        return response
    _creation_scope(config, request)
    if request['operation'] == 'prepare-create':
        subject, files, dependencies = _prepare_creation(config, request)
        response = result()
        response.update(prepared_source=subject.ref, expected_dependencies=dependencies,
                        prepared_files={name: {'sha256': _digest(raw), 'bytes': len(raw)} for name, raw in files.items()})
        response['capture_at_apply'] = (['source-create-request.json', 'source-create-environment.json',
            'source-create-provenance.jsonl'] if config.get('provenance_event_id') else [])
        return response
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('command identity must contain one to 256 characters')
    request_digest = _digest(_canonical(request))
    # Stable corpus lock coordinates creation, form writers and source revisions.
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
            _creation_replay(config, source_path, request, receipt)
            return result(receipt, True)
        if (request['expected_configuration'] != configuration or request['expected_source'] is not None
                or request['expected_revision'] is not None):
            raise JournalConflict('creation requires exact delegation and absent source/revision')
        started_at, started_ns = datetime.now(timezone.utc).isoformat(), time.perf_counter_ns()
        subject, files, dependencies = _prepare_creation(config, request)
        if request['expected_dependencies'] != dependencies:
            raise JournalConflict('prepared creation dependencies are stale')
        if config.get('provenance_event_id'):
            _capture_creation_provenance(config, request, files, started_at, started_ns,
                procedure_name='sign-promoted-identity-serialization' if config['schema_version'] == SIGN_CONFIG
                else 'source-corpus-metadata-serialization' if corpus_creation else None)
        receipt = {'schema_version': ('tos_local_source_create_receipt_v1' if profile_creation else 'tos_local_historical_create_receipt_v1'),
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
            with _sign_promotion_lock(config) if config['schema_version'] == SIGN_CONFIG else nullcontext():
                current_dependencies = _prepare_creation(config, request)[2]
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
    """One independently delegated source owner route; never semantic admission."""
    if not isinstance(request, dict) or len(_canonical(request)) > MAX_COMMAND_BYTES:
        raise ValueError('source command exceeds the 1 MiB input budget')
    request = _json_object(_canonical(request))  # Freeze caller-owned mutable input.
    config, configuration, source_path = _configuration(owner_config)
    if config['schema_version'] == 'tos_local_work_expression_owner_v1':
        from source_expression_commands import run_expression_command
        return run_expression_command(owner_config, config, configuration, source_path, request)
    if config['schema_version'] == CORPUS_SELECTED_REVISION_CONFIG:
        from source_revisions import run_revision
        return run_revision(owner_config, config, configuration, source_path, request)
    from source_metadata_snapshot import PublicationSnapshot
    snapshot = PublicationSnapshot(Path(config['source_root'])) if 'source_root' in config else None
    result = _run_configured_command(owner_config, config, configuration, source_path, request)
    if snapshot is not None:
        snapshot.verify_current()
    return result


def _run_configured_command(owner_config, config, configuration, source_path, request):
    if config['schema_version'] in {OWNER_CLAIM_CONFIG, OWNER_CLAIM_REFERENCE_CONFIG}:
        from source_owner_claim_commands import run_command
        return run_command(owner_config, config, configuration, source_path, request)
    if config['schema_version'] == OWNER_PROFILE_CONFIG:
        from source_owner_profile_commands import run_command
        return run_command(owner_config, config, configuration, source_path, request)
    if config['schema_version'] == TEXT_UNIT_CONFIG:
        from source_text_unit_commands import run_command
        return run_command(owner_config, config, configuration, source_path, request)
    if config['schema_version'] in {CLAIM_CONFIG, CLAIM_VALUE_CONFIG, CLAIM_STRUCTURED_CONFIG, CLAIM_REFERENCE_CONFIG}:
        from source_claim_commands import run_command
        return run_command(owner_config, config, configuration, source_path, request)
    if config['schema_version'] in {CLAIM_REVISION_CONFIG, CLAIM_VALUE_REVISION_CONFIG, CLAIM_STRUCTURED_REVISION_CONFIG, CLAIM_REFERENCE_REVISION_CONFIG, CLAIM_LAYER_REVISION_CONFIG}:
        from claim_revisions import run_command
        return run_command(owner_config, config, configuration, source_path, request)
    if config['schema_version'] in {*CREATION_CONFIGS, *PROFILE_CREATION_CONFIGS, CORPUS_CONFIG}:
        return _create_source(owner_config, config, configuration, source_path, request)
    if config['schema_version'] in {REVISION_CONFIG, PROFILE_REVISION_CONFIG, CORPUS_REVISION_CONFIG,
                                    CORPUS_SELECTED_REVISION_CONFIG}:
        from source_revisions import run_revision
        return run_revision(owner_config, config, configuration, source_path, request)
    claim_id = config['claim_id'] if config['schema_version'] == CLAIM_FORM_CONFIG else None
    field_catalog = claim_field_catalog if claim_id is not None else metadata_field_catalog
    materialize = materialize_claim_forms if claim_id is not None else materialize_metadata_forms
    prepare_change = prepare_claim_change if claim_id is not None else prepare_metadata_change
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
    snapshot = _snapshot(source_path, Path(config['source_root']), claim_id)
    source_contracts = (_claim_form_source(source_path, Path(config['source_root']), claim_id)[2]
                        if claim_id is not None else
                        _native_form_source(source_path, Path(config['source_root']))[2]
                        if source_path.name in {'artifact-witness.json', 'composite-witness.json'} else None)
    if source_contracts is not None and configuration != _digest(_canonical({
            'configuration': config, 'source_contracts': source_contracts})):
        raise JournalConflict('Claim form source contracts changed before command preparation')

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
                                  for field in field_catalog(source)],
                'allowed_form_ids': config['allowed_form_ids'],
                **({'source_contracts': source_contracts} if source_contracts is not None else {}),
                'forms': [_form_ref(form) for form in payload['forms']] if payload else [],
                'materializations': materialize(source, payload, access_allowed=True) if payload else [],
                'receipt': receipt, 'replayed': replayed, 'grants_admission': False}

    if operation == 'describe':
        return result(snapshot)
    if operation == 'prepare':
        source_raw, source, subject, target, raw, payload = snapshot
        if request['form_id'] not in config['allowed_form_ids']:
            raise PermissionError('prepared form is outside the delegated identity scope')
        change = prepare_change(source, payload, config['principal_id'], request['form_id'], request['field_id'])
        if change['operation'] not in config['allowed_operations']:
            raise PermissionError('prepared operation is not delegated')
        response = result(snapshot)
        response['prepared_change'] = change
        return response
    changes = _changes(request, config)
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('command identity must contain one to 256 characters')
    request_digest = _digest(_canonical(request))
    with _locked(Path(config['source_root']) / 'ToS/source-witnesses/historical-create'), _locked(snapshot[3]):
        config, configuration, current_source_path = _configuration(owner_config)
        if current_source_path != source_path or config.get('claim_id') != claim_id:
            raise JournalConflict('owner source route changed before the transaction')
        if source_contracts is not None and configuration != _digest(_canonical({
                'configuration': config, 'source_contracts': source_contracts})):
            raise JournalConflict('Claim form source contracts changed before transaction')
        changes = _changes(request, config)  # Current revocation also applies to replay.
        snapshot = _snapshot(source_path, Path(config['source_root']), claim_id)
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
        views = {view['form']['id']: view for view in materialize(source, value, access_allowed=True)}
        for change in changes:
            if change['form']['content']['kind'] == 'source-copy' and views[change['form']['form_id']]['state'] != 'ready':
                raise ValueError('source-copy does not satisfy the source metadata reader')
        receipt = {'command_id': request['command_id'], 'request_digest': request_digest,
                   'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
                   'owner_configuration': configuration, 'recorded_at': datetime.now(timezone.utc).isoformat(),
                   **({'source_contracts': source_contracts} if source_contracts is not None else {}),
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

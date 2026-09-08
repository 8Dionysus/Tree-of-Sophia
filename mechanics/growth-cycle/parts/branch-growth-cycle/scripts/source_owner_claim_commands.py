"""Protected Claim growth using existing source, form and revision grammars.

The source context chooses transport, not an ontology, grant or assessment.
Candidate grounding and stored-source reading share one reader. Atomic flat
packages, locks and exact archives remain the existing source command route.
"""
from __future__ import annotations

from datetime import datetime, timezone
import os
from pathlib import Path
import re
import time

import source_commands as source
import source_claim_commands as claims
import source_owner_profile_commands as transport
import source_revisions as packages
import claim_revisions as revisions
from source_owner_context import OwnerLocalSourceContext, _read as context_read
from source_owner_claim_profiles import OwnerLocalSourceClaimProfiles
from source_record_profiles import SourceClaimProfiles, SourceRecordProfiles, SOURCE_CLAIM_BASENAME
from source_witness_human_forms import _materialize_forms, claim_field_catalog


CONFIG = source.OWNER_CLAIM_CONFIG
REFERENCE_CONFIG = source.OWNER_CLAIM_REFERENCE_CONFIG
REFERENCE_READER = 'structured-reference-value-v1'
OPERATIONS = ('claims.create', 'claim.revise', 'form.create', 'form.revise')
SELECTION_KEYS = {'claim_id', 'relation_type_id', 'origin_id', 'source_access',
                  'source_records', 'native_bindings', 'verify_content'}
CONFIG_KEYS = {'schema_version', 'uid', 'principal_id', 'maker_type', 'authority_ref',
    'expires_at', 'source_context_ref', 'source_path', 'provenance_event_id', 'allowed_operations',
    'allowed_claim_ids', 'allowed_subject_refs', 'allowed_object_refs', 'allowed_predicates',
    'allowed_evidence_refs', 'allowed_form_ids', 'allowed_fields', 'claim_selections'}
CREATION_KEYS = {'schema_version', 'operation', 'command_id', 'claims', 'forms',
    'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies', 'expected_inputs'}
CREATION_RECEIPT_KEYS = {'schema_version', 'command_id', 'request_digest', 'principal_id', 'authority_ref',
    'owner_configuration', 'recorded_at', 'source_path', 'dependencies', 'source_bindings', 'claims', 'files', 'grants_admission'}
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_owner_claim_commands.py'
IMPLEMENTATIONS = (*transport.IMPLEMENTATIONS, MODULE_REF,
    claims.MODULE_REF, revisions.MODULE_REF, 'scripts/source_owner_claim_profiles.py')
BASE_FILES = {SOURCE_CLAIM_BASENAME, transport.CONFIG_FILE, 'source-create-request.json',
              'source-create-environment.json', 'source-create-provenance.jsonl'}


def _config_keys(config):
    return CONFIG_KEYS | ({'allowed_object_values'} if config.get('schema_version') == REFERENCE_CONFIG else set())


def _supported_readers(config):
    return {'semantic-relation-v1', 'identity-relation-v1'} | (
        {REFERENCE_READER} if config.get('schema_version') == REFERENCE_CONFIG else set())


def _readers(config, context, *, records=(), profiles=None):
    # Construct all bounded selectors before grounding any candidate. Their
    # constructors preflight every read grant without opening private content.
    result = {}
    selected = {record['claim_id']: record for record in records}
    for row in config['claim_selections']:
        sources, natives = row['source_records'], row['native_bindings']
        record = selected.get(row['claim_id'])
        if (config['schema_version'] == REFERENCE_CONFIG and record is not None
                and profiles.profiles[record['predicate']]['reader'] == REFERENCE_READER):
            # The independently preflighted v2 allowlist can cover both the
            # present and proposed member sets. Each frozen reader still gets
            # only its exact declared closure, never an inferred evidence item.
            evidence = {*record['evidence_refs'], *record.get('counterevidence_refs', ())}
            refs = profiles.identity_refs(record) | evidence
            sources = [item for item in sources if item['record_id'] in refs or item['path'] in evidence]
            anchors = {item['anchor_ref'] for item in record.get('supporting_quotes', ())}
            natives = [item for item in natives if evidence.intersection({
                item['binding']['packet_ref'], item['binding']['packet_id'], item['binding']['unit_id'],
                item['binding']['text_layer']['record_ref'], item['binding']['text_layer']['layer_id'],
                *item['binding']['ordered_anchor_refs']})
                or anchors.intersection(item['binding']['ordered_anchor_refs'])]
        result[row['claim_id']] = OwnerLocalSourceClaimProfiles(context, row['source_access'],
            source_records=sources, native_bindings=natives,
            verify_content=row['verify_content'], source_reader=source._read)
    return result


def configuration(config, *, owner_config):
    source._keys(config, _config_keys(config))
    raw = context_read(Path(owner_config), source.MAX_COMMAND_BYTES, confidential_file=True)
    if source._json_object(raw) != config:
        raise source.JournalConflict('private Claim delegation changed during selection')
    if (config['schema_version'] not in {CONFIG, REFERENCE_CONFIG} or type(config['uid']) is not int or config['uid'] != os.getuid()
            or config['maker_type'] not in {'human', 'software', 'model'}
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref'))):
        raise PermissionError('private Claim delegation is invalid or expired')
    allowed_fields = revisions.FIELDS | ({'object'} if config['schema_version'] == REFERENCE_CONFIG else set())
    for key, maximum, allowed in (
            ('allowed_operations', 4, OPERATIONS), ('allowed_fields', len(allowed_fields), allowed_fields),
            ('allowed_claim_ids', 32, None), ('allowed_subject_refs', 128, None),
            ('allowed_object_refs', 128, None), ('allowed_predicates', 32, None),
            ('allowed_evidence_refs', 128, None), ('allowed_form_ids', 32, None)):
        values = config[key]
        if (not isinstance(values, list) or len(values) > maximum
                or any(not isinstance(value, str) or not value.strip() for value in values)
                or len(set(values)) != len(values) or allowed is not None and set(values) - set(allowed)):
            raise PermissionError('private Claim scope must be bounded, explicit and understood')
    if config['schema_version'] == REFERENCE_CONFIG:
        claims.validate_value_scope(config)
    for key, prefix in (('allowed_claim_ids', 'tos.claim.'), ('allowed_form_ids', 'tos.form.')):
        if any(not re.fullmatch(re.escape(prefix) + r'[a-z0-9]+(?:[.-][a-z0-9]+)*', value) for value in config[key]):
            raise ValueError('private Claim and form identities must be explicit ToS identities')
    if (not isinstance(config['provenance_event_id'], str)
            or not re.fullmatch(r'tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['provenance_event_id'])):
        raise ValueError('private Claim serialization needs its explicit provenance identity')
    selections = config['claim_selections']
    if not isinstance(selections, list) or not 1 <= len(selections) <= 32:
        raise ValueError('private Claim delegation requires one to thirty-two independent source selections')
    seen = set()
    for row in selections:
        source._keys(row, SELECTION_KEYS)
        if (any(not isinstance(row[key], str) or not row[key].strip()
                for key in ('claim_id', 'relation_type_id', 'origin_id'))
                or row['claim_id'] in seen or row['claim_id'] not in config['allowed_claim_ids']
                or type(row['verify_content']) is not bool
                or not isinstance(row['source_records'], list) or not isinstance(row['native_bindings'], list)):
            raise PermissionError('private Claim selection is repeated or exceeds its identity scope')
        seen.add(row['claim_id'])
        if (row['native_bindings'] or any(isinstance(value, dict) and value.get('source_binding') is not None
                for value in row['source_records'])) and row['verify_content'] is not True:
            raise PermissionError('private Claim growth with native evidence needs exact source verification')
    if seen != set(config['allowed_claim_ids']):
        raise PermissionError('every delegated Claim needs its independent source selection')
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    path = context.path(config['source_path'])
    relative = Path(config['source_path'])
    suffix = relative.relative_to(Path(context.private_prefix)) if context.role(config['source_path']) == 'owner-local-root' else None
    if (suffix is None or len(suffix.parts) != 3 or suffix.parts[0] != 'claims'
            or not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', suffix.parts[1])
            or suffix.name != SOURCE_CLAIM_BASENAME):
        raise PermissionError('private Claim creation requires one named owner-local claims package')
    transport._directory(context, path.parent.parent)
    readers = _readers(config, context)
    profiles = SourceClaimProfiles(context.public_root)
    if set(config['allowed_predicates']) - profiles.profiles.keys():
        raise PermissionError('private Claim delegation contains an undeclared predicate')
    if any(profiles.profiles[predicate]['reader'] not in _supported_readers(config)
           for predicate in config['allowed_predicates']):
        raise PermissionError('private Claim reader mode is outside this delegation version')
    # Readers enforce the exact understood source Claim profile, not a generic
    # Thing -> Thing predicate or a caller-supplied endpoint type.
    digest = source._digest(source._canonical({'configuration_bytes': source._digest(raw),
        'context': context.snapshot(), 'grammars': {key: reader.contract_digests for key, reader in readers.items()}}))
    return config, digest, path


def _scope(config, records, profiles, *, initial):
    if not isinstance(records, list) or not 1 <= len(records) <= 32:
        raise ValueError('private source growth requires one to thirty-two Claims')
    identifiers = {record.get('claim_id') for record in records if isinstance(record, dict)}
    for record in records:
        scoped = {**config, 'allowed_operations': ['claims.create']}
        if not initial and isinstance(record, dict) and isinstance(record.get('maker'), dict):
            # A correction actor is not retroactively the maker of the source
            # Claim. Immutable maker history is checked by the archive chain.
            scoped.update(principal_id=record['maker'].get('agent_ref'),
                maker_type=record['maker'].get('maker_type'), provenance_event_id=record.get('provenance_event_ref'))
        claims._scope(scoped, [record], profiles=profiles)
        quotes = record.get('supporting_quotes', [])
        if (not isinstance(quotes, list) or any(not isinstance(quote, dict)
                or quote.get('anchor_ref') not in config['allowed_evidence_refs'] for quote in quotes)):
            raise PermissionError('private Claim quote anchors require their independent evidence grant')
        if record.get('visibility') != 'local_only':
            raise PermissionError('private Claim commands never write a public source Claim')
        if initial and (record.get('claim_version') != 1 or record.get('assessment_refs')
                        or record.get('supersedes_claim_ref') is not None):
            raise PermissionError('initial private Claims cannot revise or carry assessment admission')
        if any(ref not in identifiers for ref in record.get('alternative_claim_refs', [])):
            raise PermissionError('private growth alternatives require an independently present Claim in this batch')
    if len(identifiers) != len(records):
        raise source.JournalConflict('private Claim identity repeats in the source batch')


def _ground(config, context, records, *, exclude=None, initial=False):
    profiles = SourceClaimProfiles(context.public_root)
    if not isinstance(records, list) or not 1 <= len(records) <= 32:
        raise ValueError('private source growth requires one to thirty-two Claims')
    selections = {row['claim_id']: row for row in config['claim_selections']}
    # All Claim shapes and request-to-selection meaning are public grammar
    # checks. A later mismatched selector must fail before the first Claim's
    # private metadata or representation is read.
    for record in records:
        profiles._validate_shape(record)
        selected = selections.get(record['claim_id'])
        predicate = record['predicate']
        if (selected is None or profiles.profiles[predicate]['reader'] not in _supported_readers(config)
                or selected['relation_type_id'] != profiles.relations[predicate]['relation_type_id']):
            raise PermissionError('private Claim does not match its independently delegated relation profile')
    _scope(config, records, profiles, initial=initial)
    readers = _readers(config, context, records=records, profiles=profiles)
    bindings, snapshots = [], {}
    for record in records:
        identity = record['claim_id']
        selected, reader = selections[identity], readers[identity]
        reader.prepare_candidate(record, origin_id=selected['origin_id'], relation_type_id=selected['relation_type_id'])
        bindings.append({'claim': revisions._subject(record).ref,
            'required_sources': list(reader.dependency_refs(identity)),
            'native_sources': list(reader.native_summaries)})
        snapshots[identity] = reader.snapshot()
    inventory = transport._inventory(context, SourceRecordProfiles(context.public_root), config,
        exclude=exclude, creating=initial, reserved_ids={*config['allowed_claim_ids'], *config['allowed_form_ids']})
    # Retain freshness after all Claim closures have been read. A later Claim
    # cannot hide a change to an earlier selected endpoint.
    if any(reader.snapshot() != snapshots[identity] for identity, reader in readers.items() if identity in snapshots):
        raise source.JournalConflict('private Claim grounding changed during the batch')
    dependencies = source._digest(source._canonical({'grounding': snapshots, 'identity': inventory,
        'form_grammar': transport._form_grammar(context)[2],
        'provenance_contract': source._digest(context.read_bytes(
            context.path('ToS/contracts/provenance-event-v2.schema.json'), source.MAX_SET_BYTES, read_bytes=source._read)),
        'implementation': {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES)) for ref in IMPLEMENTATIONS}}))
    return dependencies, bindings


def _materialize(record, payload, context):
    if record.get('visibility') != 'local_only':
        raise PermissionError('private form rendering requires an owner-local Claim')
    validator, materializer, _ = transport._form_grammar(context)
    return _materialize_forms(revisions._subject(record), claim_field_catalog(record), payload,
        access_allowed=True, validator=validator, materializer_validators=materializer)


def _forms(config, context, record, payload, selections, *, rebind=False):
    if not isinstance(selections, list) or not 1 <= len(selections) <= 32:
        raise ValueError('each private Claim requires bounded source-copy form selections')
    seen = set()
    for row in selections:
        source._keys(row, {'form_id', 'field_id'})
        if row['form_id'] not in config['allowed_form_ids'] or row['form_id'] in seen:
            raise PermissionError('private Claim form is repeated or outside its delegated identity scope')
        seen.add(row['form_id'])
    validator = transport._form_grammar(context)[0]
    if payload is not None:
        source._validate_history(payload, validator=validator)
        if payload['subject']['id'] != record['claim_id'] or rebind and {form['form_id'] for form in payload['forms']} - seen:
            raise PermissionError('Claim correction must explicitly rebind every current form of this Claim')
    changes = [source.prepare_claim_change(record, payload, config['principal_id'], **row) for row in selections]
    value = source._apply(payload, revisions._subject(record), changes, validator=validator)
    views = _materialize(record, value, context)
    if not all(view['state'] == 'ready' for view in views) or not any(view['role'] == 'statement' for view in views):
        raise ValueError('private Claim forms require ready source copies including the full statement')
    return value, views, [source._form_ref(change['form']) for change in changes]


def _creation_files(config, context, path, records, selections):
    """Pure source/form serialization; do not reinterpret historical grounding."""
    if not isinstance(selections, list) or not 1 <= len(selections) <= 32:
        raise ValueError('private Claim batch requires one to thirty-two source-copy forms')
    grouped, seen = {record['claim_id']: [] for record in records}, set()
    for row in selections:
        source._keys(row, {'claim_id', 'form_id', 'field_id'})
        if row['claim_id'] not in grouped or row['form_id'] in seen:
            raise PermissionError('private Claim form is unselected or collides with a sibling')
        seen.add(row['form_id'])
        grouped[row['claim_id']].append({key: row[key] for key in ('form_id', 'field_id')})
    files = {path.name: b''.join(source._canonical(record) + b'\n' for record in records),
             transport.CONFIG_FILE: packages._encode(config)}
    views = []
    for record in records:
        payload, rendered, _ = _forms(config, context, record, None, grouped[record['claim_id']])
        files[source.claim_forms_path(path, record['claim_id']).name] = packages._encode(payload)
        views.extend(rendered)
    return files, views


def _prepare_create(config, context, path, request, *, exclude=None):
    dependencies, bindings = _ground(config, context, request['claims'], exclude=exclude, initial=True)
    files, views = _creation_files(config, context, path, request['claims'], request['forms'])
    return files, dependencies, bindings, views


def _archive_reader(context):
    def read(root, config, receipt):
        return revisions._read_archive(root, config, receipt,
            read_files=lambda _root, selected, retained: transport._read_archive_files(context, selected, retained))
    return read


def _history_config(config, context):
    return {**config, 'source_root': str(context.public_root)}


def _inspect(config, context, path):
    files = transport._package(context, path.parent)
    if not BASE_FILES | {transport.RECEIPT_FILE} <= files.keys():
        raise source.JournalCorruption('private Claim package lacks its creation evidence')
    records = revisions._claims(files[path.name])
    if not records or set(records) - set(config['allowed_claim_ids']):
        raise PermissionError('private Claim package has an undelegated subject')
    formnames = {source.claim_forms_path(path, identity).name: identity for identity in records}
    if set(files) - BASE_FILES - {transport.RECEIPT_FILE, revisions.HISTORY} - formnames.keys():
        raise source.JournalCorruption('private Claim package has unbound files')
    history = revisions._history(files, _history_config(config, context), archive_reader=_archive_reader(context))
    forms, identities = {}, set()
    validator = transport._form_grammar(context)[0]
    for name, identity in formnames.items():
        payload = source._json_object(files[name])
        source._validate_history(payload, validator=validator)
        if payload['subject'] != revisions._subject(records[identity]).ref:
            raise source.JournalCorruption('private Claim forms do not bind the current Claim')
        selected = {form['form_id'] for form in [*payload['forms'], *payload['prior_forms']]}
        if selected & identities or selected - set(config['allowed_form_ids']):
            raise PermissionError('private Claim form identity is undelegated or shared by siblings')
        identities |= selected
        forms[identity] = payload
    archives = [_verify_revision_forms(config, context, path, receipt, forms)[0] for receipt in history['receipts']]
    creation = _creation_integrity(config, context, path, files, records, forms, archives)
    commands = [creation['command_id'], *(row['command_id'] for row in history['receipts']),
                *(row['command_id'] for value in forms.values() for row in value.get('growth_history', []))]
    if len(commands) != len(set(commands)):
        raise source.JournalCorruption('private Claim history reuses a command identity across operations or siblings')
    dependencies, bindings = _ground(config, context, list(records.values()), exclude=path.parent)
    return {'files': files, 'records': records, 'history': history, 'forms': forms,
            'dependencies': dependencies, 'bindings': bindings}


def _creation_integrity(config, context, path, files, records, forms, archives):
    """Every read/update starts from byte-bound creation, not just a v1 label.

    Historical request/configuration and initial form serialization are
    inspected without claiming that their old unpinned dependency bytes are
    still current. Current source access/grounding is checked separately.
    """
    try:
        receipt = source._json_object(files[transport.RECEIPT_FILE])
        request = source._json_object(files['source-create-request.json'])
        retained = source._json_object(files[transport.CONFIG_FILE])
        source._keys(receipt, CREATION_RECEIPT_KEYS)
        source._keys(request, CREATION_KEYS)
        source._keys(retained, _config_keys(retained))
        source._instant(receipt['recorded_at'])
        if (receipt['schema_version'] != 'tos_local_claim_create_receipt_v1' or receipt['grants_admission'] is not False
                or retained['schema_version'] not in {CONFIG, REFERENCE_CONFIG} or retained['source_path'] != config['source_path']
                or 'claims.create' not in retained['allowed_operations']
                or request['schema_version'] != 'tos_local_source_command_v1' or request['operation'] != 'claims.create'
                or not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256
                or receipt['command_id'] != request['command_id']
                or receipt['request_digest'] != source._digest(source._canonical(request))
                or receipt['source_path'] != config['source_path']
                or receipt['principal_id'] != retained['principal_id'] or receipt['authority_ref'] != retained['authority_ref']
                or receipt['owner_configuration'] != request['expected_configuration']
                or request['expected_source'] is not None or request['expected_revision'] is not None
                or receipt['dependencies'] != request['expected_dependencies'] or receipt['source_bindings'] != request['expected_inputs']):
            raise source.JournalCorruption('private Claim creation is not its exact retained request/delegation')
        profiles = SourceClaimProfiles(context.public_root)
        for record in request['claims']:
            profiles._validate_shape(record)
        _scope(retained, request['claims'], profiles, initial=True)
        if (receipt['claims'] != [revisions._subject(record).ref for record in request['claims']]
                or {record['claim_id'] for record in request['claims']} != set(records)):
            raise source.JournalCorruption('private Claim creation subject closure changed')
        initial, _ = _creation_files(retained, context, path, request['claims'], request['forms'])
        original = archives[0] if archives else files
        expected = BASE_FILES | initial.keys()
        if (set(receipt['files']) != expected or original[path.name] != initial[path.name]
                or files[transport.CONFIG_FILE] != initial[transport.CONFIG_FILE]
                or files['source-create-request.json'] != source._canonical(request) + b'\n'):
            raise source.JournalCorruption('private Claim initial stream, request or file closure changed')
        for name in expected:
            raw = initial[name] if name in initial else original[name]
            if receipt['files'][name] != {'sha256': source._digest(raw), 'bytes': len(raw)}:
                raise source.JournalCorruption('private Claim initial bytes differ from their creation receipt')
        immutable = (BASE_FILES - {path.name}) | {transport.RECEIPT_FILE}
        if any(snapshot.get(name) != files[name] for snapshot in archives for name in immutable):
            raise source.JournalCorruption('private Claim correction rewrote immutable creation evidence')
        for record in request['claims']:
            identity = record['claim_id']
            first = source._json_object(initial[source.claim_forms_path(path, identity).name])['forms']
            history = [*forms[identity]['forms'], *forms[identity]['prior_forms']]
            if any(form not in history for form in first):
                raise source.JournalCorruption('private Claim initial forms are no longer retained')
        return receipt
    except (KeyError, TypeError, ValueError) as error:
        if isinstance(error, source.JournalCorruption):
            raise
        raise source.JournalCorruption('private Claim creation evidence is malformed or inconsistent') from error


def _verify_revision_forms(config, context, path, receipt, forms):
    archived, locations = _archive_reader(context)(context.public_root, config, receipt)
    identity = receipt['previous_source']['id']
    previous = revisions._claims(archived[path.name])[identity]
    revised = revisions._advance(previous, receipt['request']['fields'])
    prior = source._json_object(archived[source.claim_forms_path(path, identity).name])
    _, _, refs = _forms({**config, 'principal_id': receipt['principal_id']}, context, revised, prior,
                        receipt['request']['forms'], rebind=True)
    retained = {source._canonical(source._form_ref(form)) for form in [*forms[identity]['forms'], *forms[identity]['prior_forms']]}
    if refs != receipt['forms'] or any(source._canonical(ref) not in retained for ref in refs):
        raise source.JournalCorruption('private Claim correction forms differ from their exact retained request')
    return archived, locations


def _result(config, digest, path, context, *, state=None, identity=None, receipt=None, replayed=False):
    result = {'schema_version': 'tos_local_source_command_result_v1', 'authentication': 'local-unix-account',
        'owner_configuration': digest, 'source_path': config['source_path'], 'target_exists': os.path.lexists(path.parent),
        'supported_operations': list(OPERATIONS), 'allowed_operations': config['allowed_operations'],
        'command_operations': ['describe', 'prepare-create', 'claims.create', 'prepare-revise', 'claim.revise', 'prepare', 'apply', 'inspect-version'],
        'allowed_claim_ids': config['allowed_claim_ids'], 'allowed_form_ids': config['allowed_form_ids'],
        'allowed_subject_refs': config['allowed_subject_refs'], 'allowed_object_refs': config['allowed_object_refs'],
        **({'allowed_object_values': config['allowed_object_values']} if 'allowed_object_values' in config else {}),
        'allowed_predicates': config['allowed_predicates'], 'allowed_evidence_refs': config['allowed_evidence_refs'],
        'allowed_fields': config['allowed_fields'], 'visibility': 'local_only', 'publication_authorized': False,
        'grants_admission': False, 'receipt': receipt, 'replayed': replayed,
        'replay_input_posture': 'historical_request_current_validation' if replayed else None,
        'sources': [], 'source': None, 'revision': None, 'source_fields': [], 'materializations': []}
    if state is not None:
        result.update(sources=[revisions._subject(record).ref for record in state['records'].values()],
            source=revisions._subject(state['records'][identity]).ref if identity else None,
            revision=packages._revision(state['files']), expected_dependencies=state['dependencies'],
            source_fields=[{'claim_id': key, **{name: value for name, value in field.items() if name not in {'pointer', 'context'}}}
                for key, record in state['records'].items() if identity is None or identity == key for field in claim_field_catalog(record)],
            source_bindings=state['bindings'], materializations=[view for key, record in state['records'].items()
                if identity is None or identity == key for view in _materialize(record, state['forms'][key], context)])
    return result


def _current(owner, digest, config, context, path, records, dependencies, *, exclude=None, initial=False, files=None,
             proposed=None):
    if proposed is not None and _ground(config, context, proposed[0], exclude=exclude)[0] != proposed[1]:
        raise source.JournalConflict('private Claim proposed grounding changed')
    if _ground(config, context, records, exclude=exclude, initial=initial)[0] != dependencies:
        raise source.JournalConflict('private Claim source, grammar, rights or identity grounding changed')
    if files is not None and transport._package(context, path.parent) != files:
        raise source.JournalConflict('private Claim package changed')
    # These cheap permission/configuration guards follow the final heavy source
    # read and all response materialization, including an idempotent replay.
    if (context.snapshot() != OwnerLocalSourceContext.load(config['source_context_ref']).snapshot()
            or source._configuration(owner)[1:] != (digest, path)):
        raise source.JournalConflict('private Claim context or delegation changed')


def _creation_replay(config, context, path, request, digest):
    state = _inspect(config, context, path)
    files = state['files']
    receipt = source._json_object(files[transport.RECEIPT_FILE])
    source._keys(receipt, CREATION_RECEIPT_KEYS)
    if (receipt['command_id'] != request['command_id'] or receipt['request_digest'] != source._digest(source._canonical(request))
            or receipt['source_path'] != config['source_path']):
        raise source.JournalConflict('private Claim creation target or command identity is occupied')
    if (receipt['schema_version'] != 'tos_local_claim_create_receipt_v1' or receipt['grants_admission'] is not False
            or receipt['owner_configuration'] != digest or request['expected_configuration'] != digest
            or receipt['principal_id'] != config['principal_id'] or receipt['authority_ref'] != config['authority_ref']
            or request['expected_source'] is not None or request['expected_revision'] is not None
            or receipt['dependencies'] != request['expected_dependencies'] or receipt['source_bindings'] != request['expected_inputs']):
        raise source.JournalCorruption('private Claim receipt differs from its original request or delegation')
    source._instant(receipt['recorded_at'])
    prepared, dependencies, bindings, _ = _prepare_create(config, context, path, request, exclude=path.parent)
    # _inspect checked all exact historical creation bytes and retained forms;
    # replay additionally requires the same *current* delegated configuration.
    if files[transport.CONFIG_FILE] != prepared[transport.CONFIG_FILE]:
        raise source.JournalCorruption('private Claim replay configuration differs from creation')
    return state, receipt, dependencies


def _proposal(config, context, path, state, request):
    identity = request['claim_id']
    record = state['records'][identity]
    revisions._scope(config, request, record, profiles=SourceClaimProfiles(context.public_root))
    if len(state['history']['receipts']) >= packages.MAX_REVISIONS:
        raise ValueError('private Claim revision history capacity reached')
    revised = revisions._advance(record, request['fields'])
    records = [revised if key == identity else value for key, value in state['records'].items()]
    dependencies, bindings = _ground(config, context, records, exclude=path.parent)
    payload, views, refs = _forms(config, context, revised, state['forms'][identity], request['forms'], rebind=True)
    selected_ids = {form['form_id'] for form in payload['forms']}
    if any(selected_ids & {form['form_id'] for form in [*value['forms'], *value['prior_forms']]}
           for key, value in state['forms'].items() if key != identity):
        raise source.JournalConflict('private Claim revision cannot reuse a sibling form identity')
    output = {**state['files'], path.name: revisions._replace(state['files'][path.name], revised),
              source.claim_forms_path(path, identity).name: packages._encode(payload)}
    return revised, output, views, refs, dependencies, bindings


def run_command(owner, config, digest, path, request):
    operation = request.get('operation')
    keys = {'schema_version', 'operation'}
    if operation in {'prepare-create', 'claims.create'}:
        keys |= {'claims', 'forms'}
        needed = 'claims.create'
    elif operation in {'prepare-revise', 'claim.revise'}:
        keys |= {'claim_id', 'fields', 'forms', 'reason'}
        needed = 'claim.revise'
    elif operation == 'prepare':
        keys |= {'claim_id', 'form_id', 'field_id'}
        needed = None
    elif operation == 'apply':
        keys |= {'claim_id', 'changes'}
        needed = None
    elif operation == 'inspect-version':
        keys |= {'claim_id', 'source'}
        needed = None
    elif operation == 'describe':
        needed = None
    else:
        raise ValueError('unknown private Claim command')
    writing = operation in {'claims.create', 'claim.revise', 'apply'}
    if writing:
        keys |= {'command_id', 'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies', 'expected_inputs'}
    source._keys(request, keys)
    if request['schema_version'] != 'tos_local_source_command_v1':
        raise ValueError('unknown source command envelope')
    if needed is not None and needed not in config['allowed_operations']:
        raise PermissionError('private Claim operation is not delegated')
    identity = request.get('claim_id')
    if identity is not None and identity not in config['allowed_claim_ids']:
        raise PermissionError('private Claim command selected an undelegated identity')
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    if writing:
        if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
            raise ValueError('private Claim command identity must contain one to 256 characters')
        with transport._locked(context):
            if source._configuration(owner)[1:] != (digest, path):
                raise source.JournalConflict('private Claim delegation changed before transaction')
            if operation == 'claims.create':
                if os.path.lexists(path.parent):
                    state, receipt, dependencies = _creation_replay(config, context, path, request, digest)
                    response = _result(config, digest, path, context, state=state, receipt=receipt, replayed=True)
                    _current(owner, digest, config, context, path, request['claims'], dependencies,
                             exclude=path.parent, initial=True, files=state['files'])
                    return response
                return _create(owner, config, digest, context, path, request)
            return _update(owner, config, digest, context, path, request)
    if operation == 'describe' and not os.path.lexists(path.parent):
        response = _result(config, digest, path, context)
        if source._configuration(owner)[1:] != (digest, path):
            raise source.JournalConflict('private Claim delegation changed')
        return response
    if operation == 'prepare-create':
        files, dependencies, bindings, views = _prepare_create(config, context, path, request)
        response = {**_result(config, digest, path, context),
            'prepared_sources': [revisions._subject(record).ref for record in request['claims']],
            'prepared_files': packages._file_refs(files), 'prepared_materializations': views,
            'expected_source': None, 'expected_revision': None, 'expected_dependencies': dependencies, 'source_bindings': bindings}
        _current(owner, digest, config, context, path, request['claims'], dependencies, initial=True)
        return response
    state = _inspect(config, context, path)
    if identity is not None and identity not in state['records']:
        raise source.JournalConflict('delegated Claim is absent from this package')
    response = _result(config, digest, path, context, state=state, identity=identity)
    proposed = None
    if operation == 'prepare-revise':
        revised, _, views, refs, dependencies, bindings = _proposal(config, context, path, state, request)
        response.update(prepared_source=revisions._subject(revised).ref, prepared_forms=refs,
                        prepared_materializations=views, expected_dependencies=dependencies, source_bindings=bindings)
        proposed = ([revised if key == identity else value for key, value in state['records'].items()], dependencies)
    elif operation == 'prepare':
        if request['form_id'] not in config['allowed_form_ids']:
            raise PermissionError('private Claim form identity is not delegated')
        change = source.prepare_claim_change(state['records'][identity], state['forms'][identity], config['principal_id'],
                                             request['form_id'], request['field_id'])
        transport._form_changes({'changes': [change]}, config)
        response['prepared_change'] = change
    elif operation == 'inspect-version':
        receipt = next((row for row in state['history']['receipts']
            if row['previous_source'] == request['source'] and row['previous_source']['id'] == identity), None)
        if receipt is None:
            raise source.JournalConflict('exact Claim version is not retained in this delegated history')
        archived, locations = _archive_reader(context)(context.public_root, config, receipt)
        response.update(record=revisions._claims(archived[path.name])[identity], files=locations, inspected_source=request['source'])
    _current(owner, digest, config, context, path, list(state['records'].values()), state['dependencies'],
             exclude=path.parent, files=state['files'], proposed=proposed)
    return response


def _create(owner, config, digest, context, path, request):
    if (request['expected_configuration'] != digest or request['expected_source'] is not None
            or request['expected_revision'] is not None):
        raise source.JournalConflict('private Claim creation requires exact delegation and an absent package')
    started_at, started_ns = datetime.now(timezone.utc).isoformat(), time.perf_counter_ns()
    files, dependencies, bindings, _ = _prepare_create(config, context, path, request)
    if dependencies != request['expected_dependencies'] or bindings != request['expected_inputs']:
        raise source.JournalConflict('private Claim creation dependencies are stale')
    source._capture_creation_provenance(_history_config(config, context), request, files, started_at, started_ns,
        procedure_name='owner-local-source-claim-serialization',
        additional_software_refs=(MODULE_REF, claims.MODULE_REF, revisions.MODULE_REF,
                                  'scripts/source_owner_claim_profiles.py'), owner_local_metadata=True)
    receipt = {'schema_version': 'tos_local_claim_create_receipt_v1', 'command_id': request['command_id'],
        'request_digest': source._digest(source._canonical(request)), 'principal_id': config['principal_id'],
        'authority_ref': config['authority_ref'], 'owner_configuration': digest, 'recorded_at': datetime.now(timezone.utc).isoformat(),
        'source_path': config['source_path'], 'dependencies': dependencies, 'source_bindings': bindings,
        'claims': [revisions._subject(record).ref for record in request['claims']],
        'files': packages._file_refs(files), 'grants_admission': False}
    files[transport.RECEIPT_FILE] = packages._encode(receipt)
    staging = transport._stage(context, files)
    try:
        if transport._package(context, staging) != files:
            raise source.JournalConflict('private Claim staged creation changed')
        _current(owner, digest, config, context, path, request['claims'], dependencies, initial=True)
        source._publish_new_directory(staging, path.parent)
    finally:
        packages._discard_staging(staging, files)
    state = _inspect(config, context, path)
    response = _result(config, digest, path, context, state=state, receipt=receipt)
    _current(owner, digest, config, context, path, list(state['records'].values()), state['dependencies'],
             exclude=path.parent, files=state['files'])
    return response


def _update(owner, config, digest, context, path, request):
    state = _inspect(config, context, path)
    identity, operation = request['claim_id'], request['operation']
    if identity not in state['records']:
        raise source.JournalConflict('delegated Claim is absent')
    record, payload, files = state['records'][identity], state['forms'][identity], state['files']
    if operation == 'claim.revise':
        revisions._scope(config, request, record, profiles=SourceClaimProfiles(context.public_root))
    else:
        transport._form_changes(request, config)
    receipts = [(row, 'claim.revise', row['source']['id']) for row in state['history']['receipts']]
    receipts += [(row, 'apply', key) for key, value in state['forms'].items() for row in value.get('growth_history', [])]
    creation = source._json_object(files[transport.RECEIPT_FILE])
    if creation['command_id'] == request['command_id']:
        raise source.JournalConflict('private Claim command identity belongs to package creation')
    for receipt, retained_operation, retained_identity in receipts:
        if receipt['command_id'] != request['command_id']:
            continue
        if (retained_operation != operation or retained_identity != identity
                or receipt['request_digest'] != source._digest(source._canonical(request))
                or receipt['owner_configuration'] != digest or request['expected_configuration'] != digest):
            raise source.JournalConflict('private Claim command identity was reused or delegation changed')
        if receipt['principal_id'] != config['principal_id'] or receipt['authority_ref'] != config['authority_ref']:
            raise source.JournalCorruption('private Claim retry differs from its delegated actor')
        if operation == 'apply' and (receipt['source'] != request['expected_source']
                or receipt['previous_revision'] != request['expected_revision']
                or receipt['results'] != [source._form_ref(change['form']) for change in request['changes']]):
            raise source.JournalCorruption('private Claim form retry differs from its exact request')
        proposed = None
        if (operation == 'claim.revise'
                and SourceClaimProfiles(context.public_root).profiles[record['predicate']]['reader'] == REFERENCE_READER):
            # A later correction may have removed one member of this retained
            # request. Ground its exact historical result as well as today's
            # Claim; an old receipt cannot authorize a now-unavailable member.
            archived, _ = _archive_reader(context)(context.public_root, config, receipt)
            previous = revisions._claims(archived[path.name])[identity]
            replayed = revisions._advance(previous, request['fields'])
            records = [replayed if key == identity else value for key, value in state['records'].items()]
            dependencies, _ = _ground(config, context, records, exclude=path.parent)
            proposed = (records, dependencies)
        response = _result(config, digest, path, context, state=state, identity=identity, receipt=receipt, replayed=True)
        _current(owner, digest, config, context, path, list(state['records'].values()), state['dependencies'],
                 exclude=path.parent, files=files, proposed=proposed)
        return response
    if (request['expected_configuration'] != digest or request['expected_source'] != revisions._subject(record).ref
            or request['expected_revision'] != packages._revision(files)):
        raise source.JournalConflict('private Claim source or package is stale')
    revision = packages._revision(files)
    proposed = None
    if operation == 'claim.revise':
        revised, output, _, refs, dependencies, bindings = _proposal(config, context, path, state, request)
        if dependencies != request['expected_dependencies'] or bindings != request['expected_inputs']:
            raise source.JournalConflict('private Claim correction grounding is stale')
        proposed = ([revised if key == identity else value for key, value in state['records'].items()], dependencies)
        archive_config = {**config, 'record_id': identity}
        receipt = {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
            'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'], 'owner_configuration': digest,
            'recorded_at': datetime.now(timezone.utc).isoformat(), 'reason': request['reason'],
            'previous_source': revisions._subject(record).ref, 'source': revisions._subject(revised).ref,
            'previous_revision': revision, 'archive_path': transport._archive_ref(context, archive_config, revision).as_posix(),
            'dependencies': dependencies, 'source_bindings': bindings, 'changed_fields': sorted(request['fields']),
            'forms': refs, 'grants_admission': False, 'request': request}
        output[revisions.HISTORY] = packages._encode({**state['history'], 'receipts': [*state['history']['receipts'], receipt]})
        transport._archive(context, archive_config, files, revisions._subject(record), revision,
            reader=lambda _context, selected, retained: _archive_reader(context)(context.public_root, selected, retained))
    else:
        if request['expected_dependencies'] != state['dependencies'] or request['expected_inputs'] != state['bindings']:
            raise source.JournalConflict('private Claim form grounding is stale')
        changes = transport._form_changes(request, config)
        value = source._apply(payload, revisions._subject(record), changes, validator=transport._form_grammar(context)[0])
        sibling_ids = {form['form_id'] for key, other in state['forms'].items() if key != identity
                       for form in [*other['forms'], *other['prior_forms']]}
        if any(change['form']['form_id'] in sibling_ids for change in changes):
            raise source.JournalConflict('private Claim form identity belongs to a sibling')
        views = {view['form']['id']: view for view in _materialize(record, value, context)}
        if any(change['form']['content']['kind'] == 'source-copy' and views[change['form']['form_id']]['state'] != 'ready'
               for change in changes):
            raise ValueError('private Claim source-copy form loses mandatory context')
        receipt = {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
            'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'], 'owner_configuration': digest,
            'recorded_at': datetime.now(timezone.utc).isoformat(), 'source': revisions._subject(record).ref,
            'previous_revision': revision, 'results': [source._form_ref(change['form']) for change in changes]}
        value.setdefault('growth_history', []).append(receipt)
        source._validate_history(value, validator=transport._form_grammar(context)[0])
        output = {**files, source.claim_forms_path(path, identity).name: packages._encode(value)}
    staging = transport._stage(context, output)
    try:
        if transport._package(context, staging) != output:
            raise source.JournalConflict('private Claim staged correction changed')
        _current(owner, digest, config, context, path, list(state['records'].values()), state['dependencies'],
                 exclude=path.parent, files=files, proposed=proposed)
        packages._exchange(staging, path.parent)
    finally:
        if staging.exists():
            remaining = transport._package(context, staging)
            if remaining == files and (operation == 'apply'
                    or _archive_reader(context)(context.public_root, config, receipt)[0] == files):
                packages._discard_staging(staging, files)
            elif remaining == output:
                packages._discard_staging(staging, output)
    current = _inspect(config, context, path)
    response = _result(config, digest, path, context, state=current, identity=identity, receipt=receipt)
    _current(owner, digest, config, context, path, list(current['records'].values()), current['dependencies'],
             exclude=path.parent, files=current['files'])
    return response

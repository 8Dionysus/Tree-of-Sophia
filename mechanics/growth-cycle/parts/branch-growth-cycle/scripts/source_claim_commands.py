"""Separately delegated creation of declared source Claim packages.

This is a source-command adapter, not assessment or read-only access. It uses
the existing local account, shared source lock, no-replace publication and
serialization capture. All scopes come from the protected owner configuration.
"""
from datetime import datetime, timezone
import os
from pathlib import Path
import re
import tempfile
import time

import source_commands as source
from source_record_profiles import SourceClaimProfiles, SourceRecordProfiles, SOURCE_CLAIM_BASENAME

OPERATION = 'claims.create'
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_claim_commands.py'
PACKAGE_FILES = {SOURCE_CLAIM_BASENAME, 'source-create-request.json', 'source-create-environment.json',
                 'source-create-provenance.jsonl', 'source-create-receipt.json'}


def configuration(config):
    values_allowed = config['schema_version'] in {
        source.CLAIM_VALUE_CONFIG, source.CLAIM_STRUCTURED_CONFIG, source.CLAIM_REFERENCE_CONFIG}
    source._keys(config, {'schema_version', 'uid', 'principal_id', 'maker_type', 'source_root',
        'source_path', 'authority_ref', 'expires_at', 'provenance_event_id', 'allowed_operations',
        'allowed_claim_ids', 'allowed_subject_refs', 'allowed_object_refs', 'allowed_predicates',
        'allowed_evidence_refs'} | ({'allowed_object_values'} if values_allowed else set()))
    if (config['schema_version'] not in {source.CLAIM_CONFIG, source.CLAIM_VALUE_CONFIG,
            source.CLAIM_STRUCTURED_CONFIG, source.CLAIM_REFERENCE_CONFIG} or type(config['uid']) is not int
            or config['uid'] != os.getuid() or config['maker_type'] not in {'human', 'software', 'model'}
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref'))
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)):
        raise PermissionError('claim creation delegation is invalid or expired')
    for key, maximum in (('allowed_operations', 1), ('allowed_claim_ids', 32),
                         ('allowed_subject_refs', 128), ('allowed_object_refs', 128),
                         ('allowed_predicates', 32), ('allowed_evidence_refs', 128)):
        values = config[key]
        if (not isinstance(values, list) or len(values) > maximum
                or any(not isinstance(value, str) or not value.strip() for value in values)
                or len(set(values)) != len(values)):
            raise ValueError('claim delegation scope must be a bounded list of unique identifiers')
    if values_allowed:
        validate_value_scope(config)
    if (set(config['allowed_operations']) - {OPERATION}
            or any(not re.fullmatch(r'tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*', value)
                   for value in config['allowed_claim_ids'])
            or not isinstance(config['provenance_event_id'], str)
            or not re.fullmatch(r'tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['provenance_event_id'])):
        raise ValueError('invalid delegated operation, claim or provenance identity')
    root = Path(config['source_root'])
    os.close(source._owned_path(root, directory=True))
    relative = Path(config['source_path'])
    if (relative.is_absolute() or relative.as_posix() != config['source_path'] or '..' in relative.parts
            or len(relative.parts) != 5 or relative.parts[:3] != ('ToS', 'source-witnesses', 'relations')
            or relative.parts[3] in {'catalog', 'payload', 'local-content'}
            or not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', relative.parts[3])
            or relative.name != SOURCE_CLAIM_BASENAME):
        raise PermissionError('claim creation requires one new named source relation package')
    profiles = SourceClaimProfiles(root)
    if set(config['allowed_predicates']) - profiles.profiles.keys():
        raise PermissionError('claim creation can delegate only declared source predicates')
    return config, source._digest(source._canonical(config)), root / relative


def validate_value_scope(config):
    values = config['allowed_object_values']
    if (not isinstance(values, list) or len(values) > 32 or any(not isinstance(value, dict) for value in values)
            or len({source._canonical(value) for value in values}) != len(values)):
        raise ValueError('value delegation requires at most 32 distinct exact JSON objects')


def value_is_delegated(config, value):
    """Exact data allowlist, not executable matching expressions or entity scope."""
    if (config['schema_version'] not in {source.CLAIM_VALUE_CONFIG, source.CLAIM_VALUE_REVISION_CONFIG,
                                       source.CLAIM_STRUCTURED_CONFIG, source.CLAIM_STRUCTURED_REVISION_CONFIG,
                                       source.CLAIM_REFERENCE_CONFIG, source.CLAIM_REFERENCE_REVISION_CONFIG,
                                       source.OWNER_CLAIM_REFERENCE_CONFIG}
            or not isinstance(value, dict)
            or source._canonical(value) not in {source._canonical(v) for v in config['allowed_object_values']}):
        return False
    if config['schema_version'] in {source.CLAIM_STRUCTURED_CONFIG, source.CLAIM_STRUCTURED_REVISION_CONFIG,
                                   source.CLAIM_REFERENCE_CONFIG, source.CLAIM_REFERENCE_REVISION_CONFIG,
                                   source.OWNER_CLAIM_REFERENCE_CONFIG}:
        return True  # Exact bytes only; _value_scope checks declared identity dependencies.
    relative = value.get('relative')
    return (relative is None or isinstance(relative, dict)
            and relative.get('anchor_ref') in config['allowed_object_refs'])


def _scope(config, claims, *, profiles=None):
    """Current scope applies before preparation and even to an exact replay."""
    if OPERATION not in config['allowed_operations']:
        raise PermissionError('claim creation is not delegated')
    if not isinstance(claims, list) or not 1 <= len(claims) <= 32:
        raise ValueError('one to thirty-two initial claims are required')
    profiles = profiles if profiles is not None else SourceClaimProfiles(Path(config['source_root']))
    seen = set()
    for claim in claims:
        if isinstance(claim, dict) and claim.get('predicate') in {'has_expression', 'translated_by'}:
            # A readable Claim profile is not a standalone writer grant. This
            # predicate changes its parent's exact outgoing closure and belongs
            # to the separately authorized compound bibliographic operation.
            # Owner-local forwarding deliberately receives the same refusal.
            raise PermissionError('this relation requires its compound bibliographic operation')
        if (not isinstance(claim, dict) or not isinstance(claim.get('claim_id'), str)
                or claim['claim_id'] not in config['allowed_claim_ids']
                or claim.get('subject_ref') not in config['allowed_subject_refs']
                or not (isinstance(claim.get('object'), str) and claim['object'] in config['allowed_object_refs']
                        or value_is_delegated(config, claim.get('object')))
                or claim.get('predicate') not in config['allowed_predicates']
                or not isinstance(claim.get('maker'), dict)
                or claim['maker'].get('agent_ref') != config['principal_id']
                or claim['maker'].get('maker_type') != config['maker_type']
                or claim.get('provenance_event_ref') != config['provenance_event_id']):
            raise PermissionError('claim identity, endpoints, predicate or maker is not delegated')
        _value_scope(config, claim, profiles)
        if claim['claim_id'] in seen:
            raise source.JournalConflict('claim identity repeats in the batch')
        seen.add(claim['claim_id'])
        for field in ('evidence_refs', 'counterevidence_refs'):
            refs = claim.get(field, [])
            if (not isinstance(refs, list) or any(not isinstance(ref, str)
                    or ref not in config['allowed_evidence_refs'] for ref in refs)):
                raise PermissionError('claim evidence is not delegated')


def _value_scope(config, claim, profiles):
    """Check declared value scope before new writes and exact replays alike."""
    reader = profiles.profiles[claim['predicate']]['reader']
    if reader == 'structured-reference-value-v1':
        if config['schema_version'] not in {source.CLAIM_REFERENCE_CONFIG, source.CLAIM_REFERENCE_REVISION_CONFIG,
                                            source.OWNER_CLAIM_REFERENCE_CONFIG}:
            raise PermissionError('reference value creation or correction requires separate v4 or private v2 delegation')
        if any(identity not in config['allowed_object_refs'] for identity in profiles.reference_members(claim)):
            # Focal subject scope cannot grant the same identity's member role.
            raise PermissionError('declared reference value members are not separately delegated')
    if (reader == 'structured-value-v1'
            and config['schema_version'] in {source.CLAIM_VALUE_CONFIG, source.CLAIM_VALUE_REVISION_CONFIG}):
        raise PermissionError('structured value creation or correction requires separate v3 delegation')
    value = claim['object']
    relative = value.get('relative') if isinstance(value, dict) else None
    if (profiles.is_temporal(claim) and 'allowed_object_refs' in config
            and isinstance(value, dict) and value.get('kind') == 'relative-order'
            and (not isinstance(relative, dict) or relative.get('anchor_ref') not in config['allowed_object_refs'])):
        # A subject grant does not grant use of the same identity as an anchor.
        raise PermissionError('declared value identity dependencies are not delegated')


def _prepare(config, claims):
    _scope(config, claims)
    return _ground_claims(config, claims, initial=True)


def reference_replay_snapshot(config, records):
    """Current closure for reference-value retries, not historical admission.

    A retained receipt cannot substitute for present source/member validation.
    Older reader modes keep their existing replay contract unchanged.
    """
    profiles = SourceClaimProfiles(Path(config['source_root']))
    selected = [record for record in records
                if profiles.profiles[record['predicate']]['reader'] == 'structured-reference-value-v1']
    if not selected:
        return None
    return source._digest(source._canonical([
        _ground_claims(config, [record], initial=False)[1] for record in selected]))


def _ground_claims(config, claims, *, initial):
    """Source grounding shared by separately authorized creation and correction.

    This function grants no write scope. Callers validate their own exact
    operation, immutable subject identity and current delegation first.
    """
    from build_source_witness_catalog import collect_records, collect_claims
    from source_witness_bibliographic_graph_common import _scan_index, _evidence_node, _external_citation_node
    root = Path(config['source_root'])
    metadata = SourceRecordProfiles(root)
    records = collect_records(root, profiles=metadata)
    objects = {record['record_id']: record for rows in records.values() for record in rows}
    input_digests = {}
    prior_claims = collect_claims(root, input_digests=input_digests)
    identifiers = set(objects) | {claim['claim_id'] for claim in prior_claims}
    profiles = SourceClaimProfiles(root)
    events = _scan_index(root, filename_pattern='*provenance*.jsonl', id_field='event_id')
    anchors = _scan_index(root, filename_pattern='*anchor*.jsonl', id_field='anchor_id')
    if initial and config['provenance_event_id'] in events:
        raise source.JournalConflict('provenance event identity already exists')
    evidence = []
    for claim in claims:
        profiles.validate(claim, objects)
        _value_scope(config, claim, profiles)
        if initial and (claim['claim_version'] != 1 or claim.get('assessment_refs')
                or claim.get('supersedes_claim_ref') is not None):
            raise PermissionError('initial claim creation does not revise or assess claims')
        if initial and claim['claim_id'] in identifiers:
            raise source.JournalConflict('claim identity already exists')
        identifiers.add(claim['claim_id'])
        for ref in [*claim['evidence_refs'], *claim.get('counterevidence_refs', [])]:
            if ref.startswith('ToS/'):
                relative = Path(ref)
                if (relative.as_posix() != ref or '..' in relative.parts
                        or any(part in {'payload', 'local-content'} for part in relative.parts)):
                    raise PermissionError('claim evidence must address explicit owned metadata')
                # Same account is trusted; paths must still exclude symlinks and
                # other-user writes. Permission to cite comes from the exact
                # independent allowed_evidence_refs, not source prose.
                os.close(source._owned_path(root / relative))
                source._read(root / relative, source.MAX_SET_BYTES)
            if (not initial and claim.get('predicate') == 'translated_by'
                    and ref.split(':', 1)[0].lower() in {'http', 'https'}):
                entry = next((entry for entry in prior_claims if entry['claim_id'] == claim['claim_id']), None)
                if entry is None or entry['source_claim_file_ref'] != config['source_path']:
                    raise ValueError('external citation correction requires the exact current Claim source locator')
                evidence.append(_external_citation_node(ref, claim,
                    {**entry, 'claim_sha256': source._digest(source._canonical(claim))[7:]}, citation_status='candidate_claim'))
            else:
                evidence.append(_evidence_node(ref, repo_root=root, anchors=anchors, objects=objects, events=events))
    claim_ids = {claim['claim_id'] for claim in [*prior_claims, *claims]}
    if any(ref not in claim_ids for claim in claims for ref in claim.get('alternative_claim_refs', [])):
        raise ValueError('alternative claim does not resolve in source or this batch')
    raw = b''.join(source._canonical(claim) + b'\n' for claim in claims)
    if len(raw) > source.MAX_COMMAND_BYTES:
        raise ValueError('initial claim stream exceeds its bounded byte budget')
    source_bindings = {'objects': {}, 'evidence': {}}
    for identity in sorted({identity for claim in claims for identity in profiles.identity_refs(claim)}):
        entry = objects[identity]
        source_raw = source._read(root / entry['source_record_ref'], source.MAX_SET_BYTES)
        payload = source._json_object(source_raw)
        source_bindings['objects'][identity] = {'source_ref': entry['source_record_ref'],
            'source_sha256': source._digest(source_raw), 'canonical_record_sha256': 'sha256:' + entry['record_sha256'],
            'schema_version': payload.get('schema_version'), 'record_version': payload.get('record_version')}
    values = {claim['claim_id']: {'value': claim['object'], 'sha256': source._digest(source._canonical(claim['object'])),
        'type_ids': profiles.relations[claim['predicate']]['range_type_ids']}
        for claim in claims if profiles.is_value(claim)}
    if values:
        source_bindings['values'] = values
    for node in evidence:
        source_bindings['evidence'][node['properties']['evidence_ref']] = {
            'source_ref': node['source_ref'], 'source_sha256': 'sha256:' + node['source_sha256'],
            'source_line': node.get('source_line'), 'evidence_kind': node['properties']['evidence_kind']}
        if node['properties']['evidence_kind'] == 'external_citation':
            source_bindings['evidence'][node['properties']['evidence_ref']].update(
                citation_status='candidate_claim', citing_claim_ref=node['properties']['citing_claim_ref'],
                resolved=False, remote_content_sha256=None,
                source_hash_scope='candidate_claim_declaration_not_preexisting_input_or_remote_content')
    dependencies = source._digest(source._canonical({'records': records, 'claims': prior_claims,
        'source_profiles': source._profile_input_snapshot(metadata), 'existing_claim_profiles': input_digests,
        'new_claim_profiles': profiles.input_digests, 'events': events, 'anchors': anchors, 'evidence': evidence,
        'selected_source_bindings': source_bindings,
        'provenance_contract': source._digest(source._read(root / 'ToS/contracts/provenance-event-v2.schema.json', source.MAX_SET_BYTES)),
        'implementation': {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES)) for ref in
            (MODULE_REF, 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
             'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py',
             'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
             'scripts/source_record_profiles.py', 'scripts/native_text_binding.py', 'scripts/source_owner_context.py',
             'scripts/build_source_witness_catalog.py',
             'scripts/source_witness_bibliographic_graph_common.py')}}))
    return {SOURCE_CLAIM_BASENAME: raw}, dependencies, source_bindings


def _replay(target, config, request):
    os.close(source._owned_path(target, directory=True))
    names = {path.name for path in target.iterdir()}
    from claim_revisions import HISTORY, creation_source_files
    from source_revisions import _package
    if HISTORY in names:
        current_files = _package(target)
        original_files = creation_source_files(current_files, config)
    else:
        # Unrevised creation retains its prior file/size contract. The bounded
        # revision package budget must not retroactively narrow this reader.
        current_files = {name: source._read(target / name, source.MAX_SET_BYTES)
                         for name in PACKAGE_FILES if name in names}
        original_files = current_files
    form_targets = {source.claim_forms_path(target / SOURCE_CLAIM_BASENAME, claim['claim_id']).name: claim['claim_id']
                    for claim in request['claims']}
    allowed = PACKAGE_FILES | form_targets.keys() | {'.' + name + '.writer.lock' for name in form_targets} | {HISTORY}
    if not PACKAGE_FILES <= names or names - allowed:
        raise source.JournalConflict('claim package is occupied or no longer an initial package')
    # Only independently retained form sets of these exact Claims may extend
    # the initial package. The creation receipt still binds its original five
    # files, not the current form content or any semantic decision.
    for name in names - PACKAGE_FILES - {HISTORY}:
        raw = source._read(target / name, source.MAX_SET_BYTES)
        if name in form_targets:
            forms = source._json_object(raw)
            source._validate_history(forms)
            if forms['subject']['id'] != form_targets[name]:
                raise source.JournalCorruption('adjacent Claim forms belong to another subject')
        elif raw:
            raise source.JournalCorruption('Claim form lock contains unexpected data')
    receipt = source._json_object(source._read(target / 'source-create-receipt.json', source.MAX_COMMAND_BYTES))
    if (receipt.get('schema_version') != 'tos_local_claim_create_receipt_v1'
            or receipt.get('command_id') != request['command_id']
            or receipt.get('request_digest') != source._digest(source._canonical(request))
            or receipt.get('source_path') != config['source_path']):
        raise source.JournalConflict('claim creation target or command identity is occupied')
    expected_fields = {'schema_version', 'command_id', 'request_digest', 'principal_id', 'authority_ref',
        'owner_configuration', 'recorded_at', 'source_path', 'dependencies', 'source_bindings', 'claims',
        'files', 'grants_admission'}
    if (set(receipt) != expected_fields or receipt['grants_admission'] is not False
            or receipt['principal_id'] != config['principal_id']
            or receipt['owner_configuration'] != request['expected_configuration']
            or receipt['dependencies'] != request['expected_dependencies']
            or receipt['source_bindings'] != request['expected_inputs']
            or receipt['claims'] != [source.Record.from_payload(c['claim_id'], c['claim_version'], c).ref
                                    for c in request['claims']]):
        raise source.JournalCorruption('claim receipt no longer binds its original request and source snapshot')
    if set(receipt.get('files', {})) != PACKAGE_FILES - {'source-create-receipt.json'}:
        raise source.JournalCorruption('claim creation receipt file closure changed')
    for name, binding in receipt['files'].items():
        raw = original_files[name]
        if binding != {'sha256': source._digest(raw), 'bytes': len(raw)}:
            raise source.JournalCorruption('created claim package bytes changed')
    if any(current_files[name] != original_files[name] for name in PACKAGE_FILES - {SOURCE_CLAIM_BASENAME}):
        raise source.JournalCorruption('Claim correction rewrote immutable creation evidence')
    if (original_files['source-create-request.json'] != source._canonical(request) + b'\n'
            or original_files[SOURCE_CLAIM_BASENAME] !=
               b''.join(source._canonical(claim) + b'\n' for claim in request['claims'])):
        raise source.JournalCorruption('claim package differs from the retained original command')
    return receipt


def run_command(owner_config, config, configuration_digest, path, request):
    operation = request.get('operation')
    fields = {'schema_version', 'operation'}
    if operation == 'prepare-create':
        fields |= {'claims'}
    elif operation == OPERATION:
        fields |= {'claims', 'command_id', 'expected_configuration', 'expected_revision', 'expected_dependencies', 'expected_inputs'}
    elif operation != 'describe':
        raise ValueError('unsupported claim source command')
    source._keys(request, fields)
    if request['schema_version'] != 'tos_local_source_command_v1':
        raise ValueError('unsupported source command version')
    root, target = Path(config['source_root']), path.parent
    os.close(source._owned_path(target.parent, directory=True))
    def result(receipt=None, replayed=False):
        profiles = SourceClaimProfiles(root)
        return {'schema_version': 'tos_local_claim_create_result_v1', 'authentication': 'local-unix-account',
            'owner_configuration': configuration_digest, 'source_path': config['source_path'],
            'supported_operations': [OPERATION], 'command_operations': ['describe', 'prepare-create', OPERATION],
            'allowed_operations': config['allowed_operations'], 'target_exists': target.exists(),
            'allowed_claim_ids': config['allowed_claim_ids'], 'allowed_subject_refs': config['allowed_subject_refs'],
            'allowed_object_refs': config['allowed_object_refs'], 'allowed_evidence_refs': config['allowed_evidence_refs'],
            **({'allowed_object_values': config['allowed_object_values']} if 'allowed_object_values' in config else {}),
            'source_claim_profiles': {predicate: profiles.profiles[predicate] for predicate in config['allowed_predicates']},
            'expected_revision': None, 'creation_provenance_event_id': config['provenance_event_id'],
            'receipt': receipt, 'replayed': replayed, 'grants_admission': False}
    if operation == 'describe':
        return result()
    _scope(config, request['claims'])
    if operation == 'prepare-create':
        files, dependencies, source_bindings = _prepare(config, request['claims'])
        response = result()
        response.update(expected_dependencies=dependencies, source_bindings=source_bindings,
            prepared_files={name: {'sha256': source._digest(raw), 'bytes': len(raw)} for name, raw in files.items()})
        return response
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('command identity must contain one to 256 characters')
    request_digest = source._digest(source._canonical(request))
    with source._locked(root / 'ToS/source-witnesses/historical-create'):
        _, current_digest, current_path = source._configuration(owner_config)
        if current_digest != configuration_digest or current_path != path:
            raise source.JournalConflict('claim creation delegation changed before transaction')
        if target.exists() or target.is_symlink():
            receipt = _replay(target, config, request)
            snapshot = reference_replay_snapshot(config, request['claims'])
            response = result(receipt, True)
            if snapshot is not None and (source._configuration(owner_config)[1:] != (configuration_digest, path)
                    or reference_replay_snapshot(config, request['claims']) != snapshot):
                raise source.JournalConflict('reference Claim replay scope or source closure changed')
            return response
        if request['expected_configuration'] != configuration_digest or request['expected_revision'] is not None:
            raise source.JournalConflict('claim creation requires exact delegation and an absent package')
        started_at, started_ns = datetime.now(timezone.utc).isoformat(), time.perf_counter_ns()
        files, dependencies, source_bindings = _prepare(config, request['claims'])
        if request['expected_dependencies'] != dependencies or request['expected_inputs'] != source_bindings:
            raise source.JournalConflict('prepared claim dependencies are stale')
        source._capture_creation_provenance(config, request, files, started_at, started_ns,
            procedure_name='source-claim-serialization', additional_software_refs=(MODULE_REF,))
        receipt = {'schema_version': 'tos_local_claim_create_receipt_v1', 'command_id': request['command_id'],
            'request_digest': request_digest, 'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
            'owner_configuration': configuration_digest, 'recorded_at': datetime.now(timezone.utc).isoformat(),
            'source_path': config['source_path'], 'dependencies': dependencies, 'source_bindings': source_bindings,
            'claims': [source.Record.from_payload(claim['claim_id'], claim['claim_version'], claim).ref for claim in request['claims']],
            'files': {name: {'sha256': source._digest(raw), 'bytes': len(raw)} for name, raw in files.items()},
            'grants_admission': False}
        files['source-create-receipt.json'] = source._canonical(receipt) + b'\n'
        staging = Path(tempfile.mkdtemp(prefix='.source-claims-create-', suffix='.pending', dir=root / 'ToS'))
        try:
            for name, raw in files.items():
                source._publish(staging / name, raw)
            if (source._configuration(owner_config)[1] != configuration_digest
                    or _prepare(config, request['claims'])[1] != dependencies):
                raise source.JournalConflict('claim delegation or dependencies changed during staging')
            source._publish_new_directory(staging, target)
        finally:
            if staging.exists():
                for name in files:
                    (staging / name).unlink(missing_ok=True)
                staging.rmdir()
        return result(receipt)

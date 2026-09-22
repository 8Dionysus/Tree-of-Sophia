"""Revise descriptive wording on explicitly selected captured historical Claims.

Only descriptive qualifiers and adjacent source-copy forms may grow. The
original historical.create bytes, historical record history, Claim identity,
endpoints, evidence and source schema stay with their existing owners. Shared
Claim revisions own locking, archives, compare-and-swap and publication.
"""
from datetime import datetime, timezone
import os
from pathlib import Path
import re
import sys

import source_commands as source
import source_command_contracts as contract

BASENAME = 'historical-claims.jsonl'
REVISION_CONFIG = 'tos_local_historical_claim_revision_owner_v1'
FORM_CONFIG = 'tos_local_historical_claim_form_owner_v1'
FIELDS = {'qualifiers'}
QUALIFIER_FIELDS = {'statement', 'statement_language', 'statement_script', 'display_fields'}
CONFIG_FIELDS = {'historical_record_id', 'creation_receipt_sha256', 'allowed_qualifier_fields', 'allowed_form_field_ids'}
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_historical_claims.py'
SCHEMA_REF = 'ToS/contracts/historical-claim.schema.json'
CAPTURE = {'source-create-request.json', 'source-create-receipt.json',
           'source-create-environment.json', 'source-create-provenance.jsonl'}
CONTRACT_REFS = (SCHEMA_REF, 'ToS/contracts/claim-packet.schema.json',
    'ToS/contracts/historical-record.schema.json', 'ToS/contracts/corpus-record.schema.json',
    'ToS/contracts/knowledge-assessment.schema.json', 'ToS/contracts/claim-display-fields.schema.json',
    'ToS/doctrine/semantic-interchange/entity-types.v1.json',
    'ToS/doctrine/semantic-interchange/relation-types.v1.json')


def is_path(path):
    """Recognize only the public history district, including rooted local paths."""
    path = Path(path)
    parts = path.parts
    if path.is_absolute():
        matches = [i for i in range(len(parts) - 2) if parts[i:i + 3] == ('ToS', 'source-witnesses', 'history')]
        if len(matches) != 1:
            return False
        parts = parts[matches[0]:]
    return (len(parts) >= 5 and parts[:3] == ('ToS', 'source-witnesses', 'history')
            and parts[-1] == BASENAME
            and all(re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', part) for part in parts[3:-1]))


def validate(root, record):
    from source_witness_bibliographic_graph_common import historical_schema_validator, _historical_display_inputs
    from source_witness_human_forms import claim_field_catalog
    historical_schema_validator(root, claim=True).validate(record)
    _historical_display_inputs(root, record)
    if record.get('visibility') not in {'public', 'public_metadata_only'}:
        raise PermissionError('historical Claim adapter requires public metadata')
    claim_field_catalog(record)  # The shared exact display-field grammar, not inferred prose.
    return {ref: source._digest(source._read(Path(root) / ref, source.MAX_SET_BYTES))
            for ref in CONTRACT_REFS}


def _binding(config, path, files):
    if (not is_path(Path(config['source_path'])) or Path(config['source_path']).is_absolute()
            or path.name != BASENAME
            or not isinstance(config['historical_record_id'], str)
            or not re.fullmatch(r'tos\.historical-(event|process|state)\.[a-z0-9]+(?:[.-][a-z0-9]+)*',
                                config['historical_record_id'])
            or not isinstance(config['creation_receipt_sha256'], str)
            or not re.fullmatch(r'sha256:[a-f0-9]{64}', config['creation_receipt_sha256'])
            or source._digest(files.get('source-create-receipt.json', b'')) != config['creation_receipt_sha256']):
        raise PermissionError('historical Claim delegation requires its exact captured package identity')
    origin = verify_creation(Path(config['source_root']), config['source_path'], files=files)
    if origin is None or origin['record_id'] != config['historical_record_id']:
        raise PermissionError('historical Claim adapter requires captured historical.create v2 origin')
    if config['claim_id'] not in origin['claim_ids']:
        raise PermissionError('delegated Claim was not created in this historical package')
    return origin


def configuration(config, owner_config=None):
    import claim_revisions as claims
    if config.get('schema_version') == REVISION_CONFIG:
        result = claims.configuration(config, family=sys.modules[__name__])
        values = config['allowed_qualifier_fields']
        if (not isinstance(values, list) or not values or len(set(values)) != len(values)
                or any(value not in QUALIFIER_FIELDS for value in values)
                or config['allowed_evidence_refs']):
            raise PermissionError('legacy correction delegates named descriptive qualifiers only')
    else:
        source._keys(config, {'schema_version', 'uid', 'principal_id', 'source_root', 'source_path',
            'authority_ref', 'expires_at', 'claim_id', 'allowed_operations', 'allowed_form_ids',
            'allowed_form_field_ids', 'historical_record_id', 'creation_receipt_sha256'})
        if (config['schema_version'] != FORM_CONFIG or type(config['uid']) is not int
                or config['uid'] != os.getuid() or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
                or any(not isinstance(config[key], str) or not config[key].strip()
                       for key in ('principal_id', 'authority_ref', 'claim_id'))):
            raise PermissionError('historical Claim form delegation is invalid or expired')
        for key, allowed in (('allowed_operations', set(source.OPERATIONS)), ('allowed_form_ids', None)):
            values = config[key]
            if (not isinstance(values, list) or len(values) > 32 or len(set(values)) != len(values)
                    or any(not isinstance(value, str) or (value not in allowed if allowed is not None else
                        not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value)) for value in values)):
                raise PermissionError('historical Claim form scope is invalid')
        source._claim_form_field_ids(config)
        root, relative = Path(config['source_root']), Path(config['source_path'])
        os.close(source._owned_path(root, directory=True))
        if relative.is_absolute() or relative.as_posix() != config['source_path'] or not is_path(relative):
            raise PermissionError('historical Claim forms require their exact public owner path')
        source.claim_forms_path(root / relative, config['claim_id'])
        _, _, inputs = source._claim_form_source(root / relative, root, config['claim_id'])
        result = config, source._digest(source._canonical({'configuration': config, 'source_contracts': inputs})), root / relative
    from source_revisions import _package
    _binding(config, result[2], _package(result[2].parent))
    if config.get('schema_version') == FORM_CONFIG:
        form_identity_inputs(config)
    return result


def scope(config, request, record):
    fields = request['fields']
    if (not isinstance(fields, dict) or record.get('schema_version') != 'tos_historical_claim_v1'
            or record.get('subject_ref') != config['historical_record_id']
            or set(fields) != {'qualifiers'} or not isinstance(fields['qualifiers'], dict)
            or not fields['qualifiers'] or not set(fields['qualifiers']) <= set(config['allowed_qualifier_fields'])):
        raise PermissionError('legacy historical correction cannot change structural qualifications or another field')


def inspect(config, path, files, record):
    validate(Path(config['source_root']), record)
    _binding(config, path, files)


def form_identity_inputs(config, objects=None):
    """Check allocated IDs through public source locators, never private scans."""
    own = source.claim_forms_path(Path(config['source_path']), config['claim_id'])
    return source._form_identity_inputs(config['source_root'], config['allowed_form_ids'],
        records=objects.values() if objects is not None else None, own=own)


def creation_lineage(root, source_path, files, request, *, archive_reader=None):
    """Prove additions and recover original Claim bytes; no write grant lookup."""
    import claim_revisions as claims
    config = {'source_root': str(root), 'source_path': source_path.with_name(BASENAME).relative_to(root).as_posix()}
    original = {claim['claim_id']: claim for claim in request['claims']}
    if len(original) != len(request['claims']):
        raise source.JournalCorruption('historical creation repeats a Claim identity')
    form_targets = {source.claim_forms_path(Path(config['source_path']), identity).name: identity for identity in original}
    additions = {claims.HISTORY, *form_targets, *('.' + name + '.writer.lock' for name in form_targets)}
    if not CAPTURE <= files.keys():
        raise source.JournalCorruption('historical Claim extensions require complete original capture')
    read_archive = archive_reader if archive_reader is not None else claims._read_archive
    history = claims._history(files, config, archive_reader=read_archive)
    initial = files
    for index, receipt in enumerate(history['receipts']):
        archived, _ = read_archive(root, config, receipt)
        if any(archived.get(name) != files[name] for name in CAPTURE):
            raise source.JournalCorruption('Claim correction changed original historical creation evidence')
        fields = receipt['request']['fields']
        if (receipt['source']['id'] not in original or set(fields) != {'qualifiers'}
                or not isinstance(fields['qualifiers'], dict)
                or not set(fields['qualifiers']) <= QUALIFIER_FIELDS):
            raise source.JournalCorruption('retained historical Claim correction exceeds its descriptive family')
        if index == 0:
            initial = archived
    expected = b''.join(source._canonical(claim) + b'\n' for claim in request['claims'])
    if initial[BASENAME] != expected or set(claims._claims(files[BASENAME])) != set(original):
        raise source.JournalCorruption('historical Claim lineage differs from its original source stream')
    for name, identity in form_targets.items():
        if name in files:
            payload = source._json_object(files[name])
            source._validate_history(payload)
            if payload['subject']['id'] != identity:
                raise source.JournalCorruption('historical Claim forms belong to another subject')
        lockname = '.' + name + '.writer.lock'
        if files.get(lockname, b''):
            raise source.JournalCorruption('historical Claim lock contains data')
    return initial[BASENAME], additions


def verify_claim_capture(root, source_ref, files, *, archive_reader):
    """Claim-only origin within the exact reader's already budgeted package IO.

    This checks captured Claim/creation bytes, not the HistoricalEvent's own
    revision archives. The mutating adapter additionally verifies those through
    verify_creation; this read-only route never grants a correction.
    """
    import claim_revisions as claims
    present = CAPTURE & files.keys()
    if not present or present == {'source-create-receipt.json'}:
        if claims.HISTORY in files:
            raise source.JournalCorruption('historical Claim revisions lack captured origin')
        return None
    if present != CAPTURE:
        raise source.JournalCorruption('historical creation capture is partial')
    request, receipt = (source._json_object(files[name]) for name in
                        ('source-create-request.json', 'source-create-receipt.json'))
    source.command_handler('tos_local_historical_create_owner_v2').validate_request(request)
    record_ref = receipt.get('source_path')
    if (request['operation'] != 'historical.create' or request['expected_source'] is not None
            or request['expected_revision'] is not None or not isinstance(record_ref, str)
            or Path(record_ref).parent != Path(source_ref).parent
            or Path(record_ref).name != request['record'].get('record_type', '') + '.json'
            or receipt.get('schema_version') != 'tos_local_historical_create_receipt_v1'
            or receipt.get('command_id') != request['command_id']
            or receipt.get('request_digest') != source._digest(source._canonical(request))
            or receipt.get('owner_configuration') != request['expected_configuration']
            or receipt.get('dependencies') != request['expected_dependencies']
            or receipt.get('source') != source.metadata_subject(request['record']).ref
            or receipt.get('grants_admission') is not False):
        raise source.JournalCorruption('historical Claim creation capture is not request-bound')
    initial, _ = creation_lineage(root, root / record_ref, files, request, archive_reader=archive_reader)
    for name in (BASENAME, *sorted(CAPTURE - {'source-create-receipt.json'})):
        raw = initial if name == BASENAME else files[name]
        if receipt.get('files', {}).get(name) != {'sha256': source._digest(raw), 'bytes': len(raw)}:
            raise source.JournalCorruption('historical Claim creation bytes differ from captured fixity')
    return {'adapter': 'captured-historical-claim-v1', 'record_history_verified': False}


def verify_creation(root, source_ref, *, files=None):
    """Read retained unsigned origin; never synthesize a current write grant."""
    from source_revisions import _package
    path = Path(root) / source_ref
    if not is_path(Path(source_ref)) or Path(source_ref).is_absolute():
        raise PermissionError('historical origin requires an exact public Claim stream')
    files = _package(path.parent) if files is None else files
    present = CAPTURE & files.keys()
    if not present or present == {'source-create-receipt.json'}:
        return None  # Old uncaptured historical creation is not new adapter authority.
    if present != CAPTURE:
        raise source.JournalCorruption('historical creation capture is partial')
    request, receipt = (source._json_object(files[name]) for name in
                        ('source-create-request.json', 'source-create-receipt.json'))
    handler = source.command_handler('tos_local_historical_create_owner_v2')
    handler.validate_request(request)
    original = request['record']
    record_ref = receipt.get('source_path')
    if (request['operation'] != 'historical.create' or request['expected_source'] is not None
            or request['expected_revision'] is not None or not isinstance(record_ref, str)
            or Path(record_ref).parent != Path(source_ref).parent
            or Path(record_ref).name != original.get('record_type', '') + '.json'
            or receipt.get('command_id') != request['command_id']
            or receipt.get('request_digest') != source._digest(source._canonical(request))):
        raise source.JournalCorruption('historical creation does not bind its exact original request')
    from source_witness_bibliographic_graph_common import historical_schema_validator
    historical_schema_validator(root).validate(original)
    rows = files['source-create-provenance.jsonl'].splitlines()
    if len(rows) != 1:
        raise source.JournalCorruption('historical creation requires its one retained serialization event')
    event = source._json_object(rows[0])
    source._validator_for_provenance(root).validate(event)
    for claim in request['claims']:
        validate(root, claim)
        if (claim['subject_ref'] != original['record_id'] or claim['claim_version'] != 1
                or claim['provenance_event_ref'] != event['event_id']
                or claim['maker']['agent_ref'] != receipt['principal_id']):
            raise source.JournalCorruption('original historical Claim has foreign identity or noninitial version')
    # This evidence context is never configured, persisted or passed to a writer.
    context = {'schema_version': 'tos_local_historical_create_owner_v2', 'source_root': str(root),
        'source_path': record_ref, 'record_id': original['record_id'], 'principal_id': receipt['principal_id'],
        'authority_ref': receipt['authority_ref'], 'provenance_event_id': event['event_id']}
    source._creation_replay(context, root / record_ref, request, receipt)
    expected_outputs = {record_ref, (Path(record_ref).parent / BASENAME).as_posix(),
                        (Path(record_ref).parent / (Path(record_ref).stem + '.human-forms.json')).as_posix()}
    if (event['event_id'] != context['provenance_event_id']
            or event['method']['procedure']['name'] != 'historical-source-metadata-serialization'
            or event['record_binding']['manifest_ref'] != (Path(record_ref).parent / 'source-create-receipt.json').as_posix()
            or {entry['entity_ref'] for entry in event['entities']['outputs']} != expected_outputs
            or len(event['entities']['outputs']) != len(expected_outputs)
            or event['rights_and_visibility']['publication_authorized'] is not False
            or event['review_and_authority']['accepted_uses'] != []
            or event['review_and_authority']['promotion_authorized'] is not False):
        raise source.JournalCorruption('historical origin is not its exact non-admitting serialization event')
    for entry in event['entities']['outputs']:
        binding = receipt['files'][Path(entry['entity_ref']).name]
        if binding != {'sha256': 'sha256:' + entry['sha256'], 'bytes': entry['size_bytes']}:
            raise source.JournalCorruption('historical provenance differs from original output fixity')
    if _package(path.parent) != files:
        raise source.JournalConflict('historical package changed during origin verification')
    return {'status': 'verified-captured-historical-origin', 'record_id': original['record_id'],
            'claim_ids': [claim['claim_id'] for claim in request['claims']],
            'receipt_sha256': source._digest(files['source-create-receipt.json']),
            'writes_to_source': False, 'grants_admission': False}


def ground(config, record):
    """Reuse historical schema/domain/evidence readers, never initial-create scope."""
    from build_source_witness_catalog import collect_records
    from source_record_profiles import SourceRecordProfiles
    from source_witness_bibliographic_graph_common import (_historical_claim_contract,
        _validate_historical_claim, _scan_index, _evidence_node)
    root = Path(config['source_root'])
    contracts = validate(root, record)
    metadata = SourceRecordProfiles(root)
    records = collect_records(root, profiles=metadata)
    objects = {entry['record_id']: entry for rows in records.values() for entry in rows}
    form_inputs = form_identity_inputs(config, objects)
    _validate_historical_claim(record, objects, _historical_claim_contract(root))
    events = _scan_index(root, filename_pattern='*provenance*.jsonl', id_field='event_id')
    anchors = _scan_index(root, filename_pattern='*anchor*.jsonl', id_field='anchor_id')
    if record['provenance_event_ref'] not in events:
        raise ValueError('historical Claim provenance is unresolved')
    identities = {record['subject_ref']}
    if isinstance(record['object'], str):
        identities.add(record['object'])
    elif record['object'].get('relative', {}).get('anchor_ref'):
        identities.add(record['object']['relative']['anchor_ref'])
    bindings = {'objects': {}, 'evidence': {}}
    for identity in sorted(identities):
        entry = objects[identity]
        raw = source._read(root / entry['source_record_ref'], source.MAX_SET_BYTES)
        payload = source._json_object(raw)
        bindings['objects'][identity] = {'source_ref': entry['source_record_ref'],
            'source_sha256': source._digest(raw), 'canonical_record_sha256': 'sha256:' + entry['record_sha256'],
            'schema_version': payload.get('schema_version'), 'record_version': payload.get('record_version')}
    for ref in [*record['evidence_refs'], *record.get('counterevidence_refs', [])]:
        if ref.startswith('ToS/'):
            relative = Path(ref)
            if relative.as_posix() != ref or '..' in relative.parts or any(
                    part in {'payload', 'local-content', 'owner-local', 'private'} for part in relative.parts):
                raise PermissionError('historical Claim evidence must address public owner metadata')
            source._read(root / relative, source.MAX_SET_BYTES)
        node = _evidence_node(ref, repo_root=root, anchors=anchors, objects=objects, events=events)
        bindings['evidence'][ref] = {'source_ref': node['source_ref'],
            'source_sha256': 'sha256:' + node['source_sha256'], 'source_line': node.get('source_line'),
            'evidence_kind': node['properties']['evidence_kind']}
    dependencies = source._digest(source._canonical({'contracts': contracts, 'objects': objects,
        'profiles': source._profile_input_snapshot(metadata), 'events': events, 'anchors': anchors,
        'bindings': bindings, 'form_identity_inputs': form_inputs,
        'implementation': {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES))
            for ref in (MODULE_REF, 'scripts/source_witness_bibliographic_graph_common.py',
                        'scripts/build_source_witness_catalog.py', 'scripts/source_record_profiles.py',
                        'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py')}}))
    return None, dependencies, bindings


def command_handlers():
    import claim_revisions as claims
    proposal = {'fields', 'forms', 'reason'}
    handles = (SCHEMA_REF, *contract.FORM_HANDLES, 'ToS/contracts/claim-display-fields.schema.json')
    return (
        contract.Handler('legacy-historical-claim-revision', (REVISION_CONFIG,), (contract.describe(),
            contract.operation('prepare-revise', proposal, definition='Prepare a descriptive legacy historical Claim successor.', grants=(claims.OPERATION,)),
            contract.operation(claims.OPERATION, proposal | contract.COMMIT_KEYS | {'expected_inputs'},
                definition='Revise exact descriptive qualifiers with retained stream bytes and forms.',
                mutation='claim_successor', grants=(claims.OPERATION,)), contract.inspect_version()),
            claims.run_command, 'Correct a captured historical Claim without changing its legacy schema or structural assertion.',
            configure=configuration, typed_handles=handles,
            preconditions=('Requires captured historical.create v2 origin and an exact independently delegated receipt binding.',)),
        contract.Handler('legacy-historical-claim-forms', (FORM_CONFIG,), (contract.describe(),
            contract.operation('prepare', {'form_id', 'field_id'}, definition='Prepare an exact historical Claim source-copy form.', grants=source.OPERATIONS),
            contract.operation('apply', {'changes', 'command_id', 'expected_source', 'expected_revision', 'expected_configuration'},
                definition='Apply separately scoped historical Claim source-copy forms without revising the Claim.',
                mutation='human_form_set', grants=source.OPERATIONS)),
            source._run_form_command, 'Materialize exact selected fields of a captured historical Claim with whole-Claim context.',
            configure=configuration, typed_handles=handles,
            preconditions=('Neither the original historical.create grant nor public native Claim grants select this legacy writer.',)),
    )

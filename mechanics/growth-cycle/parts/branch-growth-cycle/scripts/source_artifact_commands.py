"""Native physical Artifact metadata creation, never discovery or rights intake.

The shared source writer owns atomic publication and retained creation history.
This adapter owns the actual artifact_id, its v2 source shape and independently
bound public metadata inputs. It never scans or acquires artifact descendants.
"""
from __future__ import annotations

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re

import source_commands as source
import source_command_contracts as contract

CONFIG = source.ARTIFACT_CREATION_CONFIG
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_artifact_commands.py'
SCHEMA_REF = 'ToS/contracts/artifact-source-witness-v2.schema.json'
SCHEMA_VERSION = 'tos_artifact_source_witness_v2'
INPUTS = ('rights_ref', 'discovery_ref', 'research_ref')
INPUT_SCHEMAS = {'rights_ref': 'ToS/contracts/rights-record.schema.json',
                 'discovery_ref': 'ToS/contracts/material-discovery-record.schema.json'}
COMPANIONS = ('source-create-request.json', 'source-create-receipt.json',
              'source-create-environment.json', 'source-create-provenance.jsonl')


def _public_path(value):
    if not isinstance(value, str):
        raise PermissionError('Artifact input requires an explicit public source path')
    path = Path(value)
    if (path.is_absolute() or path.as_posix() != value or '..' in path.parts
            or path.parts[:1] != ('ToS',) or len(path.parts) < 3
            or any(part.startswith('.') or part in {'payload', 'local-content', 'owner-local', 'catalog'}
                   for part in path.parts)):
        raise PermissionError('Artifact input cannot address private, derived or payload storage')
    return path


def _bindings(value):
    source._keys(value, set(INPUTS))
    for binding in value.values():
        source._keys(binding, {'ref', 'sha256'})
        _public_path(binding['ref'])
        if not isinstance(binding['sha256'], str) or not re.fullmatch(r'[a-f0-9]{64}', binding['sha256']):
            raise ValueError('Artifact source input needs an exact SHA-256 byte binding')
    if len({item['ref'] for item in value.values()}) != len(INPUTS):
        raise PermissionError('Artifact rights, discovery and research are distinct input records')
    if not Path(value['discovery_ref']['ref']).is_relative_to('ToS/source-witnesses/discovery/runs'):
        raise PermissionError('Artifact discovery must use the existing discovery owner route')
    return value


def record_profile(config, record=None):
    relative = _public_path(config['source_path'])
    if (relative.parts[:3] != ('ToS', 'source-witnesses', 'artifacts')
            or len(relative.parts) != 7 or relative.name != 'artifact-witness.json'
            or 'cdli' in {part.lower() for part in relative.parts[3:-1]}):
        raise PermissionError('Artifact creation requires a provider-independent tradition/site/identity path')
    if record is not None and (not isinstance(record, dict) or record.get('schema_version') != SCHEMA_VERSION
                              or record.get('artifact_id') != config['record_id']):
        raise PermissionError('Artifact creation cannot recast another identity or native schema')
    return {'record_type': 'artifact', 'identity_field': 'artifact_id', 'id_prefix': 'tos.artifact.',
            'source_basename': 'artifact-witness.json', 'schema_ref': SCHEMA_REF,
            'schema_version': SCHEMA_VERSION, 'source_scope': 'public_metadata_only'}


def configuration(config, *, owner_config=None):
    source._keys(config, {'schema_version', 'uid', 'principal_id', 'maker_type', 'source_root',
        'source_path', 'authority_ref', 'expires_at', 'record_id', 'provenance_event_id',
        'allowed_operations', 'allowed_form_ids', 'source_bindings'})
    if (config['schema_version'] != CONFIG or type(config['uid']) is not int or config['uid'] != os.getuid()
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref'))
            or config['maker_type'] not in {'human', 'software', 'model'}
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
            or not isinstance(config['record_id'], str)
            or not re.fullmatch(r'tos\.artifact\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['record_id'])
            or not isinstance(config['provenance_event_id'], str)
            or not re.fullmatch(r'tos\.event\.[a-z0-9]+(?:[.-][a-z0-9]+)*', config['provenance_event_id'])):
        raise PermissionError('Artifact creation delegation is invalid or expired')
    for key in ('allowed_operations', 'allowed_form_ids'):
        values = config[key]
        if (not isinstance(values, list) or len(values) > 32
                or any(not isinstance(value, str) for value in values) or len(set(values)) != len(values)
                or any(value != 'source.create' if key == 'allowed_operations' else
                       not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value) for value in values)):
            raise PermissionError('Artifact creation delegation exceeds its bounded operations or forms')
    record_profile(config)
    _bindings(config['source_bindings'])
    root = Path(config['source_root'])
    os.close(source._owned_path(root, directory=True))
    return config, source._digest(source._canonical(config)), root / config['source_path']


def _schema(root, ref):
    raw = source._read(root / ref, source.MAX_SET_BYTES)
    schema = source._json_object(raw)
    if schema.get('$id') != 'https://tree-of-sophia.local/' + ref:
        raise ValueError('Artifact input schema identity differs from its owner path')
    source.Draft202012Validator.check_schema(schema)
    return source.Draft202012Validator(schema, format_checker=source.FormatChecker()), source._digest(raw)


def _validate(validator, record):
    try:
        validator.validate(record)
    except source.ValidationError as error:
        raise ValueError('Artifact metadata or its selected input violates the exact source schema') from error


def initial_record(config, record):
    record_profile(config, record)
    _validate(_schema(Path(config['source_root']), SCHEMA_REF)[0], record)
    if (record['record_version'] != 1 or record['authority']['review_status'] != 'unreviewed'
            or record['maker'] != {'maker_type': config['maker_type'], 'agent_ref': config['principal_id'],
                                   'human_review_performed': False}
            or record['provenance_event_ref'] != config['provenance_event_id']
            or record['philosophy_planting_refs'] != []
            or any(record[field] != config['source_bindings'][field]['ref'] for field in INPUTS)):
        raise PermissionError('Artifact creation requires an unreviewed native identity and separately bound source inputs')
    stack = [record]
    forbidden = {'text', 'source_text', 'transliteration', 'translation', 'image_data', 'line_art_data', 'payload'}
    while stack:
        value = stack.pop()
        if isinstance(value, dict):
            if forbidden & value.keys():
                raise PermissionError('Artifact metadata cannot contain source text or visual payload')
            stack.extend(value.values())
        elif isinstance(value, list):
            stack.extend(value)
        elif isinstance(value, str) and value.startswith(('/srv/', '/home/', '/tmp/', '/var/tmp/')):
            raise PermissionError('Artifact metadata cannot expose local owner storage paths')
    _read_inputs(Path(config['source_root']), record, config['source_bindings'])
    return source.metadata_subject(record)


def _read_inputs(root, record, bindings):
    """Exact owned inputs only; no inferred rights or research acceptance."""
    _bindings(bindings)
    inputs, schemas = {}, {}
    for field in INPUTS:
        binding = bindings[field]
        if record[field] != binding['ref']:
            raise PermissionError('Artifact metadata changed a separately bound input path')
        raw = source._read(root / _public_path(binding['ref']), source.MAX_COMMAND_BYTES)
        if source._digest(raw)[7:] != binding['sha256']:
            raise source.JournalConflict('Artifact exact source input is stale or unavailable')
        inputs[field] = raw
        if field in INPUT_SCHEMAS:
            validator, digest = _schema(root, INPUT_SCHEMAS[field])
            _validate(validator, source._json_object(raw))
            schemas[INPUT_SCHEMAS[field]] = digest
    rights, discovery = (source._json_object(inputs[field]) for field in ('rights_ref', 'discovery_ref'))
    if (record['artifact_id'] not in rights['scope_refs'] or rights['visibility'] != 'public_metadata_only'
            or rights['redistribution_posture'] != 'metadata_only'):
        raise PermissionError('Artifact source rights do not cover this exact metadata-only identity')
    if (discovery['target']['target_kind'] != 'artifact'
            or record['artifact_id'] not in discovery['target']['known_tos_refs']):
        raise PermissionError('Artifact discovery does not address this physical identity')
    return {'bindings': bindings, 'schemas': schemas}


def prepare_creation(config, request):
    from build_source_witness_catalog import collect_records, collect_claims
    from source_record_profiles import SourceRecordProfiles, SOURCE_CLAIM_BASENAME
    from source_witness_bibliographic_graph_common import _scan_index
    root = Path(config['source_root'])
    record, selections = request['record'], request['forms']
    if request['source_bindings'] != config['source_bindings']:
        raise PermissionError('Artifact request cannot replace separately delegated exact source inputs')
    subject = initial_record(config, record)
    inputs = _read_inputs(root, record, request['source_bindings'])
    profiles = SourceRecordProfiles(root)
    profiles.assert_identity_not_native(subject.id)
    records, claim_inputs = collect_records(root, profiles=profiles), {}
    claims = collect_claims(root, input_digests=claim_inputs)
    objects = [row for values in records.values() for row in values]
    if subject.id in {row['record_id'] for row in objects}:
        raise source.JournalConflict('Artifact identity already exists in authored sources')
    events = _scan_index(root, filename_pattern='*provenance*.jsonl', id_field='event_id')
    if config['provenance_event_id'] in events:
        raise source.JournalConflict('Artifact creation event identity already exists')
    if (not isinstance(selections, list) or not 1 <= len(selections) <= 32
            or any(not isinstance(item, dict) or set(item) != {'form_id', 'field_id'} for item in selections)):
        raise ValueError('Artifact creation requires bounded exact source-copy form selections')
    form_ids = [item['form_id'] for item in selections]
    if (any(not isinstance(value, str) or value not in config['allowed_form_ids'] for value in form_ids)
            or len(form_ids) != len(set(form_ids))):
        raise PermissionError('Artifact creation form identities are not separately delegated')
    adjacent = {Path(row['source_record_ref']).with_name(Path(row['source_record_ref']).stem + '.human-forms.json')
                for row in objects}
    adjacent.update(source.claim_forms_path(Path(row['source_claim_file_ref']), row['claim_id'])
                    for row in claims if Path(row['source_claim_file_ref']).name == SOURCE_CLAIM_BASENAME)
    form_inputs = {}
    for relative in sorted(adjacent):
        try:
            raw = source._read(root / relative, source.MAX_SET_BYTES)
        except FileNotFoundError:
            continue
        forms = source._json_object(raw)
        source._validate_history(forms)
        if set(form_ids).intersection(item['form_id'] for item in [*forms['forms'], *forms['prior_forms']]):
            raise source.JournalConflict('Artifact form identity already belongs to another source')
        form_inputs[relative.as_posix()] = source._digest(raw)
    forms = source._apply(None, subject, [source.prepare_metadata_change(record, None, config['principal_id'], **selection)
                                         for selection in selections])
    views = source.materialize_metadata_forms(record, forms, access_allowed=True)
    if not all(view['state'] == 'ready' for view in views) or not any(view['role'] == 'name' for view in views):
        raise ValueError('Artifact requires complete ready source-copy forms including its native name')
    encode = lambda value: (json.dumps(value, ensure_ascii=False, allow_nan=False, indent=2) + '\n').encode()
    files = {'artifact-witness.json': encode(record), 'artifact-witness.human-forms.json': encode(forms)}
    if any(len(raw) > source.MAX_SET_BYTES for raw in files.values()):
        raise ValueError('Artifact source or form set exceeds its bounded byte budget')
    contract_refs = (SCHEMA_REF, 'ToS/contracts/provenance-event-v2.schema.json')
    implementation_refs = (MODULE_REF, contract.MODULE_REF,
        'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
        'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
        'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
        'scripts/build_source_witness_catalog.py', 'scripts/source_record_profiles.py',
        'scripts/source_witness_human_forms.py', 'scripts/source_witness_bibliographic_graph_common.py',
        'ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
        'ToS/contracts/human-form-template.schema.json')
    dependencies = source._digest(source._canonical({'records': records, 'claims': claims, 'events': events,
        'source_profiles': profiles.input_digests, 'source_claim_profiles': claim_inputs,
        'native_semantic_identity_snapshot': profiles.native_identity_snapshot(read_bytes=source._read),
        'native_text_binding_snapshot': profiles.native_text_snapshot(read_bytes=source._read),
        'inputs': inputs, 'forms': form_inputs,
        'contracts': {ref: source._digest(source._read(root / ref, source.MAX_SET_BYTES)) for ref in contract_refs},
        'implementation': {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES))
                           for ref in implementation_refs}}))
    return subject, files, dependencies


def _verify_creation(root, source_ref, current_record):
    """Verify retained native origin without consulting a mutable write grant.

    None means an unmarked legacy packet, not verified origin. Partial native
    capture, unavailable exact inputs, altered bytes and incomplete history
    raise; callers must not fall back to a legacy discovery event on failure.
    This evidence-only reader grants no present operation or rights decision.
    """
    from source_metadata_snapshot import PublicationSnapshot
    import source_revisions as revisions
    import source_selected_revisions as selected
    import source_metadata_transactions as transactions
    import source_native_metadata_commands as native
    root, relative = Path(root), _public_path(source_ref)
    path = root / relative
    present = [os.path.lexists(path.parent / name) for name in COMPANIONS]
    if not any(present):
        return None
    if not all(present):
        raise source.JournalCorruption('Artifact native creation capture is incomplete')
    snapshot = PublicationSnapshot(root)
    files = {name: source._read(path.parent / name, source.MAX_SET_BYTES) for name in COMPANIONS}
    raw_current = source._read(path, source.MAX_COMMAND_BYTES)
    if source._json_object(raw_current) != current_record:
        raise source.JournalConflict('Artifact source changed during origin verification')
    request, receipt = (source._json_object(files[name]) for name in
                        ('source-create-request.json', 'source-create-receipt.json'))
    command_handlers()[0].validate_request(request)
    if (request['operation'] != 'source.create' or request['expected_source'] is not None
            or request['expected_revision'] is not None or receipt.get('source_path') != source_ref
            or not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256
            or receipt.get('command_id') != request['command_id']
            or receipt.get('request_digest') != source._digest(source._canonical(request))):
        raise source.JournalCorruption('Artifact creation receipt does not bind its retained request')
    original = request['record']
    if not isinstance(original, dict):
        raise source.JournalCorruption('Artifact retained creation request has no native record')
    _bindings(request['source_bindings'])
    # This reconstructed evidence context is never parsed as an active grant,
    # written to disk, or supplied to a mutating operation.
    context = {'schema_version': CONFIG, 'source_root': str(root), 'source_path': source_ref,
        'record_id': current_record.get('artifact_id'), 'principal_id': receipt['principal_id'],
        'maker_type': original.get('maker', {}).get('maker_type'), 'authority_ref': receipt['authority_ref'],
        'provenance_event_id': original.get('provenance_event_ref'), 'source_bindings': request['source_bindings']}
    initial = initial_record(context, original)
    record_profile(context, current_record)
    _validate(_schema(root, SCHEMA_REF)[0], current_record)
    if current_record['provenance_event_ref'] != context['provenance_event_id']:
        raise source.JournalCorruption('Artifact correction changed its original creation event')
    source._creation_replay(context, path, request, receipt)
    selected_files = revisions._selected_package(path)
    history = revisions._history(selected_files, current_record)
    for correction in history['receipts']:
        retained_request = correction['request']
        selected._request(retained_request)
        if 'publication' not in correction:
            raise source.JournalCorruption('Native Artifact correction lacks selected publication evidence')
        publication = transactions.inspect_transaction(root, correction['publication']['transaction_id'])
        if publication['status'] != 'committed':
            raise source.JournalCorruption('Artifact correction has no committed selected publication')
        replay_context = {'schema_version': source.NATIVE_METADATA_REVISION_CONFIG,
            'source_root': str(root), 'source_path': source_ref, 'record_id': initial.id,
            'record_type': 'artifact', 'record_schema_version': SCHEMA_VERSION,
            'principal_id': correction['principal_id'], 'authority_ref': correction['authority_ref'],
            'allowed_operations': ['record.revise'], 'allowed_fields': sorted(native.REVISION_FIELDS['artifact']),
            'allowed_form_ids': [selection['form_id'] for selection in retained_request['forms']]}
        _, _, reconstructed = selected._pending_plan(replay_context, path, publication)
        if reconstructed != correction:
            raise source.JournalCorruption('Artifact retained publication differs from its correction history')
    rows = files['source-create-provenance.jsonl'].splitlines()
    if len(rows) != 1:
        raise source.JournalCorruption('Artifact creation requires one exact serialization event')
    event = source._json_object(rows[0])
    _validate(source._validator_for_provenance(root), event)
    expected_outputs = {source_ref, (relative.parent / 'artifact-witness.human-forms.json').as_posix()}
    if (event['event_id'] != context['provenance_event_id']
            or event['record_binding']['manifest_ref'] != (relative.parent / 'source-create-receipt.json').as_posix()
            or event['method']['procedure']['name'] != 'native-artifact-metadata-serialization'
            or event['activity']['event_type'] != 'annotation'
            or {item['entity_ref'] for item in event['entities']['outputs']} != expected_outputs
            or len(event['entities']['outputs']) != len(expected_outputs)
            or event['rights_and_visibility']['publication_authorized'] is not False
            or event['review_and_authority']['accepted_uses'] != []
            or event['review_and_authority']['promotion_authorized'] is not False
            or MODULE_REF not in {item['artifact_ref'] for item in event['method']['software_components']}):
        raise source.JournalCorruption('Artifact native origin is not its exact non-admitting serialization event')
    for output in event['entities']['outputs']:
        binding = receipt['files'][Path(output['entity_ref']).name]
        if output['sha256'] != binding['sha256'][7:] or output['size_bytes'] != binding['bytes']:
            raise source.JournalCorruption('Artifact provenance output differs from its original receipt')
    _read_inputs(root, original, request['source_bindings'])
    if (source._read(path, source.MAX_COMMAND_BYTES) != raw_current
            or any(source._read(path.parent / name, source.MAX_SET_BYTES) != raw for name, raw in files.items())):
        raise source.JournalConflict('Artifact creation capture changed during verification')
    snapshot.verify_current()
    return {'status': 'verified-native-origin', 'source': initial.ref,
        'current_source': source.metadata_subject(current_record).ref,
        'receipt_ref': (relative.parent / 'source-create-receipt.json').as_posix(), 'receipt': receipt,
        'event_ref': (relative.parent / 'source-create-provenance.jsonl').as_posix(), 'event': event,
        'source_bindings': request['source_bindings'], 'writes_to_source': False, 'grants_admission': False}


def verify_creation(root, source_ref, current_record):
    """Evidence-only native origin; no live grant lookup or legacy fallback."""
    try:
        return _verify_creation(root, source_ref, current_record)
    except source.ValidationError as error:
        raise source.JournalCorruption('Artifact retained origin violates its exact source or form schema') from error


def command_handlers():
    proposal = {'record', 'forms', 'source_bindings'}
    return (contract.Handler('native-artifact-create', (CONFIG,),
        (contract.describe(),
         contract.operation('prepare', {'record'}, definition='Inspect a delegated unreviewed native Artifact shape.', grants=('source.create',)),
         contract.operation('prepare-create', proposal, definition='Prepare Artifact metadata and forms against three exact existing owner inputs.', grants=('source.create',)),
         contract.operation('source.create', proposal | contract.COMMIT_KEYS,
             definition='Publish a new Artifact metadata package with exact serialization provenance, never rights or discovery outputs.',
             mutation='new_native_artifact_package', grants=('source.create',))),
        source._create_source, 'Create an unreviewed physical Artifact v2 without acquiring content, fabricating discovery or accepting rights.',
        configure=configuration, typed_handles=(SCHEMA_REF, *INPUT_SCHEMAS.values(), *contract.FORM_HANDLES),
        preconditions=('Existing exact metadata-only rights, artifact-target discovery and research remain independent inputs.',
                       'No payload, publication, canon, identity merge or philosophical planting is delegated.')),)

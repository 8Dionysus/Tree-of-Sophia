"""Atomically create one native Link and one qualified source association Claim.

The independently delegated source object is read-only. URI presence, retained
metadata and serialization evidence never establish retrieval or rights.
"""
from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import re
import sys

import source_commands as source
import source_command_contracts as contract
import source_revisions as revisions
import source_metadata_transactions as transactions
import source_compound_commands as common
from source_metadata_snapshot import PublicationSnapshot
from source_record_profiles import SourceClaimProfiles, SOURCE_CLAIM_BASENAME
from source_witness_bibliographic_graph_common import validate_external_citation_address
from build_source_witness_catalog import native_witness_contract

CONFIG = 'tos_local_object_link_create_owner_v1'
REQUEST = 'tos_local_object_link_command_v1'
OPERATION = 'object.link.create'
PREPARE = 'prepare-create'
RECOVERY = 'object.link.recover'
AUTHORIZATION = 'tos_object_link_authorization_v1'
RECOVERY_AUTHORIZATION = 'tos_object_link_recovery_authorization_v1'
RECEIPT = 'tos_object_link_receipt_v1'
RECEIPT_FILE = 'object-link-creation-receipt.json'
REQUEST_FILE = 'source-create-request.json'
ENVIRONMENT_FILE = 'source-create-environment.json'
PROVENANCE_FILE = 'source-create-provenance.jsonl'
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_link_commands.py'
PREDICATES = frozenset(('described_by', 'metadata_at', 'downloadable_at', 'rights_statement_at'))
SUBJECT_KINDS = frozenset(('work', 'expression', 'edition', 'collection', 'item', 'artifact'))
LINK_SCHEMA = 'ToS/contracts/source-link.schema.json'
CLAIM_SCHEMA = 'ToS/contracts/object-link-claim-v2.schema.json'
FORM_CONTRACTS = ('ToS/contracts/human-form.schema.json',
    'ToS/contracts/human-form-set.schema.json', 'ToS/contracts/human-form-template.schema.json')
SCOPE_KEYS = {'subject_id', 'subject_source_path', 'subject_record_type', 'link_id', 'link_source_path',
    'claim_id', 'claim_source_path', 'predicate', 'provenance_event_id', 'allowed_link_form_ids',
    'allowed_claim_form_ids', 'allowed_evidence_refs', 'uri', 'observation_ref'}
CONFIG_KEYS = SCOPE_KEYS | {'schema_version', 'uid', 'source_root', 'principal_id', 'maker_type',
                           'authority_ref', 'expires_at', 'allowed_operations'}
PROPOSAL_KEYS = {'subject', 'link', 'claim', 'forms', 'claim_forms', 'reason'}
CREATE_KEYS = PROPOSAL_KEYS | {'command_id', 'expected_configuration', 'expected_dependencies', 'expected_publication'}
IMPLEMENTATIONS = (MODULE_REF, common.MODULE_REF, contract.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_native_metadata_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
    'scripts/source_record_profiles.py', 'scripts/source_metadata_snapshot.py',
    'scripts/source_witness_bibliographic_graph_common.py', 'scripts/source_witness_human_forms.py',
    'scripts/build_source_witness_catalog.py')
EVENT_PROFILE = {'warning': 'The caller supplies the link observation and qualification. No remote content is fetched or rights conclusion reached.',
    'executor': 'software:tos-source-link-commands', 'procedure': 'native-object-link-metadata-serialization',
    'purpose': 'Serialize one native Link and its qualified association Claim without observing a remote provider.',
    'component': 'ToS native object-Link adapter'}
HASH = re.compile(r'sha256:[a-f0-9]{64}')


def _ref(record):
    return source.metadata_subject(record).ref


def _scope_shape(scope):
    source._keys(scope, SCOPE_KEYS)
    kind = scope['subject_record_type']
    if kind not in SUBJECT_KINDS or scope['predicate'] not in PREDICATES:
        raise PermissionError('Link creation needs a separately delegated exact subject type and predicate')
    for key, prefix in (('subject_id', kind), ('link_id', 'link'), ('claim_id', 'claim'), ('provenance_event_id', 'event')):
        if not isinstance(scope[key], str) or not re.fullmatch(r'tos\.' + prefix + r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*', scope[key]):
            raise PermissionError('Link scope requires exact typed identities')
    subject, link, claim = (transactions._path(scope[key]) for key in ('subject_source_path', 'link_source_path', 'claim_source_path'))
    if (subject.name != ('artifact-witness.json' if kind == 'artifact' else kind + '.json')
            or link.name != 'link.json' or link.parts[:3] != ('ToS', 'source-witnesses', 'links') or len(link.parts) != 5
            or claim.name != SOURCE_CLAIM_BASENAME or claim.parts[:3] != ('ToS', 'source-witnesses', 'relations')
            or len(claim.parts) != 5):
        raise PermissionError('Link and Claim require separate exact public native homes')
    seen = set()
    for key in ('allowed_link_form_ids', 'allowed_claim_form_ids'):
        values = scope[key]
        if (not isinstance(values, list) or not 1 <= len(values) <= 32 or len(set(values)) != len(values)
                or any(not isinstance(value, str) or not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value) for value in values)
                or seen.intersection(values)):
            raise PermissionError('forms require disjoint bounded subject-local identities')
        seen.update(values)
    evidence = scope['allowed_evidence_refs']
    if (not isinstance(evidence, list) or not 1 <= len(evidence) <= 128 or len(evidence) != len(set(evidence))
            or any(not isinstance(value, str) or not value.strip() or len(value) > 4096 for value in evidence)
            or scope['observation_ref'] not in evidence):
        raise PermissionError('observation and evidence require exact bounded delegation')
    validate_external_citation_address(scope['uri'])


def configuration(config, *, owner_config=None):
    source._keys(config, CONFIG_KEYS)
    if (config['schema_version'] != CONFIG or type(config['uid']) is not int or config['uid'] != os.getuid()
            or config['maker_type'] not in {'human', 'software', 'model'}
            or any(not isinstance(config[key], str) or not config[key].strip() for key in ('principal_id', 'authority_ref'))
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)):
        raise PermissionError('object-Link delegation is invalid or expired')
    operations = config['allowed_operations']
    if (not isinstance(operations, list) or len(set(operations)) != len(operations)
            or not set(operations) <= {OPERATION, RECOVERY}):
        raise PermissionError('object-Link grant authorizes only creation and exact recovery')
    _scope_shape({key: config[key] for key in SCOPE_KEYS})
    os.close(source._owned_path(Path(config['source_root']), directory=True))
    return config, source._digest(source._canonical(config)), Path(config['source_root']) / config['link_source_path']


def validate_qualified_link_claim(claim):
    qualifiers = claim.get('qualifiers')
    if (claim.get('schema_version') != 'tos_object_link_claim_v2' or claim.get('predicate') not in PREDICATES
            or not isinstance(qualifiers, dict) or qualifiers.get('availability_is_rights_conclusion') is not False
            or any(not isinstance(qualifiers.get(key), str) or not qualifiers[key].strip()
                   for key in ('statement', 'statement_language', 'statement_script', 'link_role'))):
        raise ValueError('native object-Link Claim requires an explicit qualified statement and no inferred rights')


def _scope(config, request, *, recovery=False, original=None):
    if (RECOVERY if recovery else OPERATION) not in config['allowed_operations']:
        raise PermissionError('object-Link operation is not delegated')
    original = original if recovery else config
    subject, link, claim = request['subject'], request['link'], request['claim']
    if (not all(isinstance(record, dict) for record in (subject, link, claim))
            or _ref(subject)['id'] != config['subject_id']
            or link.get('record_id') != config['link_id'] or link.get('record_type') != 'link'
            or link.get('uri') != config['uri'] or link.get('observation_ref') != config['observation_ref']
            or link.get('association_claim_refs') != [config['claim_id']]
            or link.get('provenance_event_ref') != config['provenance_event_id']
            or link.get('record_version') != 1 or link.get('supersedes_ref') is not None
            or link.get('identity_status') != 'provisional'
            or link.get('same_as_posture') not in {'not_assessed', 'no_equivalence_claim'}
            or link.get('external_identifiers') or link.get('variant_labels')
            or claim.get('claim_id') != config['claim_id'] or claim.get('claim_version') != 1
            or claim.get('subject_ref') != config['subject_id'] or claim.get('object') != config['link_id']
            or claim.get('predicate') != config['predicate'] or claim.get('provenance_event_ref') != config['provenance_event_id']
            or claim.get('maker') != {'maker_type': original['maker_type'], 'agent_ref': original['principal_id']}
            or claim.get('assessment_refs') or claim.get('supersedes_claim_ref') is not None):
        raise PermissionError('object-Link initial metadata exceeds its exact delegated scope')
    validate_qualified_link_claim(claim)
    for values in (link.get('source_refs'), claim.get('evidence_refs'), claim.get('counterevidence_refs', [])):
        if (not isinstance(values, list) or any(not isinstance(ref, str) for ref in values)
                or not set(values) <= set(config['allowed_evidence_refs'])):
            raise PermissionError('object-Link evidence exceeds its exact allowlist')
    common._selections(request['forms'], config['allowed_link_form_ids'])
    common._selections(request['claim_forms'], config['allowed_claim_form_ids'])


def _request(request, *, create=False):
    source.command_handler(CONFIG).validate_request(request)
    if (request['schema_version'] != REQUEST or request['operation'] != (OPERATION if create else PREPARE)
            or len(source._canonical(request)) > source.MAX_COMMAND_BYTES
            or not isinstance(request['reason'], str) or not 1 <= len(request['reason'].strip()) <= 4096):
        raise ValueError('invalid bounded object-Link request')
    if create and (not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256
            or any(not isinstance(request[key], str) or not HASH.fullmatch(request[key])
                   for key in ('expected_configuration', 'expected_dependencies'))
            or request['expected_publication'] is not None and (
                not isinstance(request['expected_publication'], str) or not HASH.fullmatch(request['expected_publication']))):
        raise ValueError('object-Link publication requires exact prepared dependency bindings')


def _grammar(root, scope, request):
    subject, link, claim = request['subject'], request['link'], request['claim']
    kind = scope['subject_record_type']
    if kind == 'artifact':
        subject_schema, identity, actual_kind = native_witness_contract(subject, scope['subject_source_path'])
    else:
        subject_schema, identity, actual_kind = 'ToS/contracts/corpus-record.schema.json', 'record_id', subject.get('record_type')
    if actual_kind != kind or subject.get(identity) != scope['subject_id']:
        raise PermissionError('association subject differs from its exact native source adapter')
    digests = {}
    for ref, value in ((subject_schema, subject), (LINK_SCHEMA, link)):
        raw = source._read(root / ref, source.MAX_SET_BYTES)
        schema = source._json_object(raw)
        if schema.get('$id') != 'https://tree-of-sophia.local/' + ref:
            raise ValueError('object-Link source contract identity mismatch')
        source.Draft202012Validator.check_schema(schema)
        source.Draft202012Validator(schema, format_checker=source.FormatChecker()).validate(value)
        digests[ref] = hashlib.sha256(raw).hexdigest()
    profiles = SourceClaimProfiles(root)
    profiles.validate(claim, {scope['subject_id']: {'record_type': kind}, scope['link_id']: link})
    validate_qualified_link_claim(claim)
    for ref in (*FORM_CONTRACTS, 'ToS/contracts/provenance-event-v2.schema.json'):
        digests[ref] = hashlib.sha256(source._read(root / ref, source.MAX_SET_BYTES)).hexdigest()
    return {**profiles.input_digests, **digests}


def _context(root, scope, request):
    records, claims, digests = common._read_catalog(root, request['expected_publication'])
    if any(identity in records or identity in claims for identity in (scope['link_id'], scope['claim_id'])):
        raise source.JournalConflict('new Link or Claim identity already occurs in the source catalog')
    if any(entry.get('provenance_event_ref') == scope['provenance_event_id'] for entry in claims.values()):
        raise source.JournalConflict('provenance identity already occurs in the source catalog')
    entry = records.get(scope['subject_id'])
    if (entry is None or entry.get('record_type') != scope['subject_record_type']
            or entry.get('source_record_ref') != scope['subject_source_path']
            or entry.get('record_sha256') != _ref(request['subject'])['digest'][7:]):
        raise source.JournalConflict('association subject lacks its exact current catalog binding')
    raw = source._read(root / scope['subject_source_path'], source.MAX_SET_BYTES)
    if source._json_object(raw) != request['subject']:
        raise source.JournalConflict('association subject differs from current source bytes')
    digests[scope['subject_source_path']] = hashlib.sha256(raw).hexdigest()
    selected_form_ids = {selection['form_id'] for key in ('forms', 'claim_forms') for selection in request[key]}
    adjacent = {transactions._path(entry['source_record_ref']).with_name(
        Path(entry['source_record_ref']).stem + '.human-forms.json') for entry in records.values()}
    adjacent.update(source.claim_forms_path(transactions._path(entry['source_claim_file_ref']), identity)
        for identity, entry in claims.items() if Path(entry['source_claim_file_ref']).name == SOURCE_CLAIM_BASENAME)
    form_bytes = 0
    for relative in sorted(adjacent):
        try:
            raw = source._read(root / relative, source.MAX_SET_BYTES)
        except FileNotFoundError:
            continue
        form_bytes += len(raw)
        if form_bytes > 16 * 1024 * 1024 or len(digests) >= 384:
            raise ValueError('object-Link adjacent Form identity check exceeds its bounded source budget')
        forms = source._json_object(raw)
        source._validate_history(forms)
        if selected_form_ids.intersection(form['form_id'] for form in [*forms['forms'], *forms['prior_forms']]):
            raise source.JournalConflict('object-Link Form identity already belongs to an existing source')
        digests[str(relative)] = hashlib.sha256(raw).hexdigest()
    for ref in set([*request['link']['source_refs'], scope['observation_ref'],
                    *request['claim']['evidence_refs'], *request['claim'].get('counterevidence_refs', [])]):
        if ref.startswith('ToS/'):
            digests[ref] = hashlib.sha256(source._read(root / transactions._path(ref), source.MAX_SET_BYTES)).hexdigest()
        else:
            validate_external_citation_address(ref)
    if any(identity not in claims for identity in request['claim'].get('alternative_claim_refs', [])):
        raise ValueError('alternative association Claim has no current catalog identity')
    return {'catalog_and_sources': digests, 'contracts': _grammar(root, scope, request),
        'implementation': {ref: hashlib.sha256(source._read(source.ROOT / ref, source.MAX_SET_BYTES)).hexdigest()
                           for ref in IMPLEMENTATIONS}, 'retained_transactions': {}}


def _new_directories(root, scope):
    paths = sorted([str(Path(scope[key]).parent) for key in ('link_source_path', 'claim_source_path')])
    for ref in paths:
        path = root / ref
        os.close(source._owned_path(path.parent, directory=True))
        if os.path.lexists(path):
            raise source.JournalConflict('object-Link creation home is already occupied')
    return paths


def _transaction_id(request):
    return source._digest(source._canonical({'operation': OPERATION, 'command_id': request['command_id'],
        'owner_configuration': request['expected_configuration'], 'request_digest': source._digest(source._canonical(request))}))


def _claim_source_ref(scope):
    return scope['claim_source_path']


def _compose(root, scope, request, dependencies, *, recorded_at, environment):
    _grammar(root, scope, request)
    link, claim = request['link'], request['claim']
    forms, views, form_refs = common._forms(link, None, request['forms'], scope['principal_id'])
    claim_forms, claim_views, claim_refs = common._forms(claim, None, request['claim_forms'], scope['principal_id'], claim=True)
    link_path, claim_path = Path(scope['link_source_path']), Path(scope['claim_source_path'])
    files = {str(link_path): revisions._encode(link),
        str(link_path.with_name('link.human-forms.json')): revisions._encode(forms),
        str(claim_path): source._canonical(claim) + b'\n',
        str(source.claim_forms_path(claim_path, scope['claim_id'])): revisions._encode(claim_forms)}
    source._instant(recorded_at)
    source._keys(environment, {'runtime', 'runtime_version', 'runtime_artifact_sha256', 'backend',
                              'hardware_target', 'unicode_version', 'argv_sha256'})
    if (any(not isinstance(value, str) or not value for value in environment.values())
            or any(not re.fullmatch(r'[a-f0-9]{64}', environment[key])
                   for key in ('runtime_artifact_sha256', 'argv_sha256'))):
        raise ValueError('invalid retained runtime capture')
    event = common._event(sys.modules[__name__], scope, request, {}, files, environment, dependencies, recorded_at)
    source._validator_for_provenance(root).validate(event)
    files.update({str(claim_path.parent / REQUEST_FILE): source._canonical(request) + b'\n',
        str(claim_path.parent / ENVIRONMENT_FILE): source._canonical(environment) + b'\n',
        str(claim_path.parent / PROVENANCE_FILE): source._canonical(event) + b'\n'})
    receipt = {'schema_version': RECEIPT, 'operation': OPERATION, 'transaction_id': _transaction_id(request),
        'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'owner_configuration': request['expected_configuration'], 'principal_id': scope['principal_id'],
        'authority_ref': scope['authority_ref'], 'recorded_at': recorded_at,
        'scope': {key: scope[key] for key in sorted(SCOPE_KEYS)}, 'dependencies': request['expected_dependencies'],
        'subject': _ref(request['subject']), 'subject_source_sha256': dependencies['catalog_and_sources'][scope['subject_source_path']],
        'link': _ref(link), 'claim': common._claim_ref(claim), 'forms': {'link': form_refs, 'claim': claim_refs},
        'files': revisions._file_refs(files), 'grants_admission': False}
    files[str(claim_path.parent / RECEIPT_FILE)] = revisions._encode(receipt)
    if any(len(raw) > source.MAX_SET_BYTES for raw in files.values()):
        raise ValueError('object-Link file exceeds its metadata byte budget')
    return files, receipt, {'link': views, 'claim': claim_views}


def _authorization(scope, request, dependencies):
    return {'schema_version': AUTHORIZATION, 'scope': {key: scope[key] for key in SCOPE_KEYS},
        'principal_id': scope['principal_id'], 'maker_type': scope['maker_type'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'command_id': request['command_id'],
        'request_digest': source._digest(source._canonical(request)), 'dependency_bindings': dependencies}


def _plan(scope, authorization, files):
    return {'authorization': authorization,
        'new_directories': sorted(str(Path(scope[key]).parent) for key in ('link_source_path', 'claim_source_path')),
        'files': [{'path': ref, 'before': None, 'after': raw} for ref, raw in sorted(files.items())]}


def _validate_plan(root, plan):
    authority = plan['authorization']
    source._keys(authority, {'schema_version', 'scope', 'principal_id', 'maker_type', 'authority_ref',
        'owner_configuration', 'command_id', 'request_digest', 'dependency_bindings'})
    if authority['schema_version'] != AUTHORIZATION:
        raise PermissionError('transaction is not the exact native object-Link adapter')
    _scope_shape(authority['scope'])
    scope = {**authority['scope'], **{key: authority[key] for key in ('principal_id', 'maker_type', 'authority_ref')}}
    base = Path(scope['claim_source_path']).parent
    rows = {item['path']: item for item in plan['files']}
    request = source._json_object(rows[str(base / REQUEST_FILE)]['after'])
    receipt = source._json_object(rows[str(base / RECEIPT_FILE)]['after'])
    environment = source._json_object(rows[str(base / ENVIRONMENT_FILE)]['after'])
    _request(request, create=True)
    _scope({**scope, 'allowed_operations': [OPERATION]}, request)
    if (authority['owner_configuration'] != request['expected_configuration']
            or authority['command_id'] != request['command_id']
            or authority['request_digest'] != source._digest(source._canonical(request))
            or source._digest(source._canonical(authority['dependency_bindings'])) != request['expected_dependencies']):
        raise source.JournalCorruption('object-Link authorization does not bind its exact retained request')
    files, expected_receipt, views = _compose(root, scope, request, authority['dependency_bindings'],
        recorded_at=receipt['recorded_at'], environment=environment)
    if plan != _plan(scope, authority, files) or receipt != expected_receipt:
        raise source.JournalCorruption('object-Link transaction does not reconstruct its whole exact delta')
    return scope, request, files, receipt, views


def _link_creation_bytes(root, scope, initial):
    """Use the shared exact archive/history mechanics, with native delta limits."""
    from source_native_metadata_commands import validate_descriptive_delta, REVISION_FIELDS
    path = Path(scope['link_source_path'])
    files = revisions._selected_package(root / path)
    current = source._json_object(files[path.name])
    if current.get('record_id') != scope['link_id'] or current.get('record_type') != 'link':
        raise source.JournalCorruption('created Link current identity changed')
    history = revisions._history(files, current)
    previous_ref = _ref(initial)
    original_files = files
    for index, receipt in enumerate(history['receipts']):
        request = receipt['request']
        if (receipt['previous_source'] != previous_ref or request.get('operation') != 'record.revise'
                or request.get('schema_version') != 'tos_local_source_command_v1'
                or not request.get('fields') or not set(request['fields']) <= REVISION_FIELDS['link']
                or 'publication' not in receipt
                or receipt['publication']['selected_files'] != sorted(revisions._selected_names(path))):
            raise source.JournalCorruption('Link lineage is not an exact native descriptive correction')
        archived, _ = revisions._read_archive(root, {'record_id': scope['link_id'], 'source_path': str(path)}, receipt)
        old = source._json_object(archived[path.name])
        revised = {**old, **request['fields'], 'record_version': old['record_version'] + 1}
        validate_descriptive_delta(old, revised, 'link')
        inspected = transactions.inspect_transaction(root, receipt['publication']['transaction_id'])
        row = next((item for item in inspected['plan']['files'] if item['path'] == str(path)), None)
        if (inspected['status'] != 'committed' or row is None or row['before'] != archived[path.name]
                or source._json_object(row['after']) != revised):
            raise source.JournalCorruption('Link correction is not a committed exact selected transition')
        if index == 0:
            original_files = archived
        previous_ref = receipt['source']
    if _ref(current) != previous_ref:
        raise source.JournalCorruption('Link metadata lacks continuous history from its compound origin')
    return original_files, current


def verify_compound(root, claim_source_ref, claim):
    import claim_revisions
    root = Path(root)
    snapshot = PublicationSnapshot(root)
    claim_path = transactions._path(claim_source_ref)
    raw = source._read(root / claim_path.parent / RECEIPT_FILE, source.MAX_SET_BYTES)
    receipt = source._json_object(raw)
    if receipt.get('schema_version') != RECEIPT:
        raise source.JournalCorruption('object-Link Claim lacks its exact native compound receipt')
    inspected = transactions.inspect_transaction(root, receipt['transaction_id'])
    if inspected['status'] != 'committed':
        raise source.JournalCorruption('object-Link publication is not committed')
    scope, request, files, expected, _ = _validate_plan(root, inspected['plan'])
    if str(claim_path) != scope['claim_source_path'] or receipt != expected or raw != files[str(claim_path.parent / RECEIPT_FILE)]:
        raise source.JournalCorruption('object-Link current receipt differs from committed evidence')
    for name in (REQUEST_FILE, ENVIRONMENT_FILE, PROVENANCE_FILE):
        ref = str(claim_path.parent / name)
        if source._read(root / ref, source.MAX_SET_BYTES) != files[ref]:
            raise source.JournalCorruption('immutable object-Link capture changed')
    package = revisions._package(root / claim_path.parent)
    initial = claim_revisions.creation_source_files(package, {'source_root': str(root),
        'source_path': str(claim_path), 'claim_id': scope['claim_id']})
    if (initial[SOURCE_CLAIM_BASENAME] != files[str(claim_path)]
            or claim_revisions._claims(package[SOURCE_CLAIM_BASENAME]) != {scope['claim_id']: claim}):
        raise source.JournalCorruption('association Claim lost its exact compound creation lineage')
    original_link, current_link = _link_creation_bytes(root, scope, request['link'])
    if original_link['link.json'] != files[scope['link_source_path']]:
        raise source.JournalCorruption('Link initial bytes differ from compound publication')
    for ref in (str(Path(scope['link_source_path']).with_name('link.human-forms.json')),
                str(source.claim_forms_path(claim_path, scope['claim_id']))):
        current_forms = source._json_object(source._read(root / ref, source.MAX_SET_BYTES))
        source._validate_history(current_forms)
        expected_forms = source._json_object(files[ref])
        retained = {(form['form_id'], form['form_version']): form
                    for form in [*current_forms['forms'], *current_forms['prior_forms']]}
        if any(retained.get((form['form_id'], form['form_version'])) != form for form in expected_forms['forms']):
            raise source.JournalCorruption('initial object-Link source-copy Forms are not continuously retained')
    subject_raw = source._read(root / scope['subject_source_path'], source.MAX_SET_BYTES)
    if hashlib.sha256(subject_raw).hexdigest() != receipt['subject_source_sha256']:
        from metadata_version_reader import MetadataVersionReader
        resolved = MetadataVersionReader(root).resolve_source_bytes(scope['subject_source_path'], receipt['subject_source_sha256'])
        if resolved['status'] != 'available' or resolved['record'] != request['subject']:
            raise source.JournalCorruption('association subject no longer retains its exact original source bytes')
    elif source._json_object(subject_raw) != request['subject']:
        raise source.JournalCorruption('association subject bytes differ from retained input')
    _grammar(root, scope, {**request, 'link': current_link, 'claim': claim})
    if current_link['association_claim_refs'] != [scope['claim_id']] or current_link['provenance_event_ref'] != scope['provenance_event_id']:
        raise source.JournalCorruption('Link association closure changed outside its native creation')
    snapshot.verify_current()
    return {'transaction_id': receipt['transaction_id'], 'manifest_sha256': inspected['manifest_sha256'],
        'receipt': receipt, 'claim': claim, 'event': source._json_object(files[str(claim_path.parent / PROVENANCE_FILE)]),
        'grants_admission': False, 'writes_to_source': False}


def _read_owner(owner):
    return configuration(source._json_object(source._read(owner, source.MAX_COMMAND_BYTES)), owner_config=owner)


def _check_dependencies(root, bindings):
    return common._check_dependencies(sys.modules[__name__], root, bindings)


def _result(config, configuration_digest, *, receipt=None, replayed=False, recovery=None, views=None):
    snapshot = PublicationSnapshot(Path(config['source_root']))
    profiles = SourceClaimProfiles(Path(config['source_root']))
    result = {'schema_version': 'tos_object_link_result_v1', 'authentication': 'local-unix-account',
        'owner_configuration': configuration_digest, 'operation': OPERATION,
        'command_operations': ['describe', PREPARE, OPERATION, RECOVERY], 'allowed_operations': config['allowed_operations'],
        'scope': {key: config[key] for key in sorted(SCOPE_KEYS)}, 'publication_snapshot': snapshot.token,
        'source_profiles': {'link': {'record_type': 'link', 'type_id': profiles.mappings['link'],
            'identity_field': 'record_id', 'schema_ref': LINK_SCHEMA,
            'schema_version': 'tos_source_link_v1', 'source_basename': 'link.json'},
            'claim': {'predicate': config['predicate'], 'schema_ref': CLAIM_SCHEMA,
                'relation_type_id': profiles.relations[config['predicate']]['relation_type_id'],
                'schema_version': 'tos_object_link_claim_v2', 'source_basename': SOURCE_CLAIM_BASENAME,
                'reader': 'identity-relation-v1'}},
        'receipt': receipt, 'replayed': replayed, 'recovery': recovery, 'materializations': views, 'grants_admission': False}
    snapshot.verify_current()
    return result


def run_command(owner, config, configuration_digest, path, request):
    root = Path(config['source_root'])
    operation = request.get('operation')
    source.command_handler(CONFIG).validate_request(request)
    if request['schema_version'] != REQUEST:
        raise ValueError('unknown object-Link command version')
    if operation == 'describe':
        return _result(config, configuration_digest)
    recovery = operation == RECOVERY
    if recovery:
        if (request['decision'] not in {'resume', 'rollback'} or request['expected_configuration'] != configuration_digest
                or RECOVERY not in config['allowed_operations']):
            raise PermissionError('object-Link recovery requires its current exact grant')
    elif operation in {PREPARE, OPERATION}:
        _request(request, create=operation == OPERATION)
        _scope(config, request)
    else:
        raise ValueError('unknown object-Link operation')
    if operation == PREPARE:
        snapshot = PublicationSnapshot(root)
        _new_directories(root, config)
        proposal = {**request, 'operation': OPERATION, 'command_id': 'preview:uncommitted',
            'expected_configuration': configuration_digest, 'expected_publication': snapshot.token}
        dependencies = _context(root, config, proposal)
        proposal['expected_dependencies'] = source._digest(source._canonical(dependencies))
        _, receipt, views = _compose(root, config, proposal, dependencies,
            recorded_at=datetime.now(timezone.utc).isoformat(), environment=common._environment())
        result = {**_result(config, configuration_digest), 'prepared_link': receipt['link'], 'prepared_claim': receipt['claim'],
            'prepared_forms': receipt['forms'], 'prepared_materializations': views,
            'expected_publication': snapshot.token, 'expected_dependencies': proposal['expected_dependencies']}
        snapshot.verify_current()
        return result
    with source._locked(root / 'ToS/source-witnesses/historical-create', allow_pending=True):
        current, digest, current_path = _read_owner(owner)
        if current != config or digest != configuration_digest or current_path != path:
            raise source.JournalConflict('object-Link delegation changed before publication')
        pending = transactions.read_pending_transaction(root)
        if pending is not None:
            scope, original, _, receipt, views = _validate_plan(root, pending['plan'])
            if any(config[key] != scope[key] for key in SCOPE_KEYS if not key.startswith('allowed_')):
                raise PermissionError('pending object-Link transaction belongs to another exact scope')
            if not recovery and (request != original or request['expected_configuration'] != configuration_digest):
                raise source.JournalConflict('only the exact original request may resume without recovery')
            _scope(config, original, recovery=recovery, original=scope)
            dependencies = _context(root, scope, original)
            if dependencies != pending['plan']['authorization']['dependency_bindings']:
                raise source.JournalConflict('object-Link pending dependencies changed')
            identifier = receipt['transaction_id']
            if recovery and request['transaction_id'] != identifier:
                raise source.JournalConflict('recovery selects another pending transaction')
            decision = request['decision'] if recovery else 'resume'
            authority = pending['plan']['authorization']
            guard = common._guard(sys.modules[__name__], owner, config, configuration_digest, scope, original, {}, authority, recovery=recovery)
            renewal = {'schema_version': RECOVERY_AUTHORIZATION, 'principal_id': config['principal_id'],
                'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
                'transaction_id': identifier, 'decision': decision} if recovery else None
            action = transactions.resume_transaction if decision == 'resume' else transactions.rollback_transaction
            completed = action(root, authorization_guard=guard, transaction_id=identifier,
                               **({'recovery_authorization': renewal} if recovery else {}))
            return _result(config, configuration_digest, receipt=receipt if decision == 'resume' else None,
                recovery=completed, views=views if decision == 'resume' else None)
        if recovery:
            raise source.JournalConflict('no pending object-Link transaction is selected')
        snapshot = PublicationSnapshot(root)
        receipt_path = root / Path(config['claim_source_path']).parent / RECEIPT_FILE
        if os.path.lexists(receipt_path):
            import claim_revisions
            claims = claim_revisions._claims(source._read(root / config['claim_source_path'], source.MAX_SET_BYTES))
            verified = verify_compound(root, config['claim_source_path'], claims[config['claim_id']])
            if (verified['receipt']['request_digest'] != source._digest(source._canonical(request))
                    or request['expected_configuration'] != configuration_digest):
                raise source.JournalConflict('object-Link replay differs from its original committed request')
            snapshot.verify_current()
            return _result(config, configuration_digest, receipt=verified['receipt'], replayed=True)
        if request['expected_configuration'] != configuration_digest or request['expected_publication'] != snapshot.token:
            raise source.JournalConflict('object-Link prepared authority or publication snapshot is stale')
        _new_directories(root, config)
        dependencies = _context(root, config, request)
        if source._digest(source._canonical(dependencies)) != request['expected_dependencies']:
            raise source.JournalConflict('object-Link prepared source dependencies changed')
        files, receipt, views = _compose(root, config, request, dependencies,
            recorded_at=datetime.now(timezone.utc).isoformat(), environment=common._environment())
        authority = _authorization(config, request, dependencies)
        plan = _plan(config, authority, files)
        _validate_plan(root, plan)
        guard = common._guard(sys.modules[__name__], owner, config, configuration_digest, config, request, {}, authority, recovery=False)
        transactions.apply_transaction(root, plan, expected_snapshot=snapshot, authorization_guard=guard,
                                       transaction_id=receipt['transaction_id'])
        return _result(config, configuration_digest, receipt=receipt, views=views)


def command_handlers():
    return (contract.Handler('native-object-link-create', (CONFIG,), (contract.describe(),
        contract.operation(PREPARE, PROPOSAL_KEYS, definition='Prepare one exact native Link with its qualified object association.', grants=(OPERATION,)),
        contract.operation(OPERATION, CREATE_KEYS, definition='Publish two new exact Link and Claim homes; never revise the existing subject.',
            mutation='new_link_and_claim_packages', grants=(OPERATION,)), contract.recovery(RECOVERY)),
        run_command, 'Create a native Link and its qualified association Claim atomically; no observation, rights or admission is inferred.',
        configure=configuration, request_schema=REQUEST,
        owner_route='mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_OBJECT_LINK_GROWTH.md',
        typed_handles=(LINK_SCHEMA, CLAIM_SCHEMA, *contract.CLAIM_HANDLES, *FORM_CONTRACTS),
        profile_selection='Only exact object-link-v2 identity relations with Artifact or one of five bibliographic subject types.',
        preconditions=('Requires exact cataloged subject, new separate homes, URI/observation, qualified statement and bounded form/evidence selections.',
                       'The cited object is read-only; source reading, remote observation, rights clearance and admission remain separate.'), manages_publication=True),)

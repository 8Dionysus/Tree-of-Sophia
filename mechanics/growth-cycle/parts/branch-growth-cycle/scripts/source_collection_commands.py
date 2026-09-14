"""Native qualified membership attachment for one existing Collection and Work.

Only this exact typed append is delegated. Shared publication mechanics do not
supply semantic admission, source-reading evidence or a generic relation grant.
"""
from __future__ import annotations

import copy
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
from source_compound_commands import _record_ref, _claim_ref, _selections, _forms, _read_catalog, _catalog_record, _catalog_claim
from source_metadata_snapshot import PublicationSnapshot
from source_record_profiles import SourceClaimProfiles, SOURCE_CLAIM_BASENAME, CORPUS_REF
from source_bibliographic_topology import validate_collection_membership_delta, validate_collection_membership_closure, validate_qualified_membership_claim
from source_witness_bibliographic_graph_common import validate_external_citation_address

CONFIG = 'tos_local_collection_membership_owner_v1'
REQUEST = 'tos_local_collection_membership_command_v1'
OPERATION = 'collection.work.attach'
PREPARE = 'prepare-attach'
RECOVERY = 'collection.work.recover'
AUTHORIZATION = 'tos_collection_membership_authorization_v1'
RECOVERY_AUTHORIZATION = 'tos_collection_membership_recovery_authorization_v1'
RECEIPT = 'tos_collection_membership_receipt_v1'
RECEIPT_FILE = 'membership-attachment-receipt.json'
REQUEST_FILE = 'source-create-request.json'
ENVIRONMENT_FILE = 'source-create-environment.json'
PROVENANCE_FILE = 'source-create-provenance.jsonl'
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_collection_commands.py'
PARENT_ID = 'collection_id'
HASH = re.compile(r'sha256:[a-f0-9]{64}')
SCOPE_KEYS = {'collection_id', 'collection_source_path', 'work_id', 'work_source_path', 'predicate',
              'claim_id', 'claim_source_path', 'provenance_event_id', 'allowed_collection_form_ids',
              'allowed_claim_form_ids', 'allowed_evidence_refs', 'retained_membership_provenance_refs'}
CONFIG_KEYS = SCOPE_KEYS | {'schema_version', 'uid', 'principal_id', 'maker_type', 'source_root',
                          'authority_ref', 'expires_at', 'allowed_operations'}
PROPOSAL_KEYS = {'work', 'claim', 'forms', 'claim_forms', 'reason'}
CREATE_KEYS = PROPOSAL_KEYS | {'schema_version', 'operation', 'command_id', 'fields',
    'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies', 'expected_publication'}
IMPLEMENTATIONS = (MODULE_REF, common.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py', contract.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py',
    'scripts/source_bibliographic_topology.py', 'scripts/source_metadata_snapshot.py',
    'scripts/source_witness_bibliographic_graph_common.py',
    'scripts/source_witness_human_forms.py', 'scripts/source_record_profiles.py',
    'scripts/build_source_witness_catalog.py')
FORM_CONTRACTS = ('ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
                  'ToS/contracts/human-form-template.schema.json')
EVENT_PROFILE = {
    'warning': 'A qualified membership account is supplied by the caller; serialization and URL presence do not prove source reading or its truth.',
    'executor': 'software:tos-source-membership-commands',
    'procedure': 'native-collection-membership-metadata-serialization',
    'purpose': 'Serialize one qualified membership Claim and a Collection membership reference without judging membership.',
    'component': 'ToS native Collection membership adapter',
}


def _validate_scope_shape(scope):
    source._keys(scope, SCOPE_KEYS)
    for key, prefix in (('collection_id', 'collection'), ('work_id', 'work'),
                        ('claim_id', 'claim'), ('provenance_event_id', 'event')):
        if not isinstance(scope[key], str) or not re.fullmatch(r'tos\.' + prefix + r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*', scope[key]):
            raise PermissionError('membership scope requires exact typed identities')
    collection, work, claim = (transactions._path(scope[key]) for key in
                              ('collection_source_path', 'work_source_path', 'claim_source_path'))
    if (scope['predicate'] != 'contains_work' or collection.name != 'collection.json'
            or collection.parts[:3] != ('ToS', 'source-witnesses', 'collections')
            or len(collection.parts) < 5 or work.name != 'work.json'
            or work.parts[:3] != ('ToS', 'source-witnesses', 'works')
            or claim.parts[:3] != ('ToS', 'source-witnesses', 'relations') or len(claim.parts) != 5
            or claim.name != SOURCE_CLAIM_BASENAME
            or not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', claim.parent.name)):
        raise PermissionError('membership attachment requires exact public Collection, Work and a separate relation home')
    seen = set()
    for key in ('allowed_collection_form_ids', 'allowed_claim_form_ids'):
        values = scope[key]
        if (not isinstance(values, list) or not 1 <= len(values) <= 32 or len(values) != len(set(values))
                or any(not isinstance(value, str) or not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value) for value in values)
                or seen.intersection(values)):
            raise PermissionError('membership forms require distinct bounded subject-local identities')
        seen.update(values)
    evidence = scope['allowed_evidence_refs']
    if (not isinstance(evidence, list) or not 1 <= len(evidence) <= 128
            or any(not isinstance(value, str) or not value.strip() or len(value) > 4096 for value in evidence)
            or len(evidence) != len(set(evidence))):
        raise PermissionError('membership evidence requires a bounded explicit allowlist')

    provenance = scope['retained_membership_provenance_refs']
    if (not isinstance(provenance, list) or len(provenance) > 32 or len(set(provenance)) != len(provenance)
            or any(not isinstance(ref, str) for ref in provenance)):
        raise PermissionError('legacy membership provenance requires bounded explicit owner paths')
    for ref in provenance:
        path = transactions._path(ref)
        if not path.name.endswith('.jsonl') or 'provenance' not in path.name:
            raise PermissionError('legacy membership evidence must select exact provenance streams')


def configuration(config, *, owner_config=None):
    """Parse only this explicit grant; old creation/revision grants stay closed."""
    source._keys(config, CONFIG_KEYS)
    if (config['schema_version'] != CONFIG or type(config['uid']) is not int or config['uid'] != os.getuid()
            or config['maker_type'] not in {'human', 'software', 'model'}
            or any(not isinstance(config[key], str) or not config[key].strip()
                   for key in ('principal_id', 'authority_ref'))
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)):
        raise PermissionError('compound source delegation is invalid or expired')
    operations = config['allowed_operations']
    if (not isinstance(operations, list) or len(operations) > 2 or len(set(operations)) != len(operations)
            or any(operation not in {OPERATION, RECOVERY} for operation in operations)):
        raise PermissionError('compound delegation grants only exact creation or recovery')
    _validate_scope_shape({key: config[key] for key in SCOPE_KEYS})
    root = Path(config['source_root'])
    os.close(source._owned_path(root, directory=True))
    return config, source._digest(source._canonical(config)), root / config['collection_source_path']


def _request(request, *, create=False):
    source.command_handler(CONFIG).validate_request(request)
    if (request['schema_version'] != REQUEST or request['operation'] != (OPERATION if create else PREPARE)
            or len(source._canonical(request)) > source.MAX_COMMAND_BYTES
            or not isinstance(request['reason'], str) or not 1 <= len(request['reason'].strip()) <= 4096):
        raise ValueError('invalid bounded compound source request')
    if create:
        if (not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256
                or any(not isinstance(request[key], str) or not HASH.fullmatch(request[key])
                       for key in ('expected_configuration', 'expected_revision', 'expected_dependencies'))
                or request['expected_publication'] is not None and (
                    not isinstance(request['expected_publication'], str) or not HASH.fullmatch(request['expected_publication']))
                or not isinstance(request['fields'], dict) or set(request['fields']) != {'membership_claim_refs'}):
            raise ValueError('compound publication requires exact prepared lineage and dependency bindings')


def _scope(config, request, *, recovery=False, original=None):
    if (RECOVERY if recovery else OPERATION) not in config['allowed_operations']:
        raise PermissionError('membership operation is not delegated')
    claimed = original if recovery else config
    work, claim = request['work'], request['claim']
    if (not isinstance(work, dict) or work.get('record_id') != config['work_id'] or work.get('record_type') != 'work'
            or not isinstance(claim, dict) or claim.get('claim_id') != config['claim_id']
            or claim.get('predicate') != config['predicate'] or claim.get('subject_ref') != config['collection_id']
            or claim.get('object') != config['work_id'] or claim.get('provenance_event_ref') != config['provenance_event_id']
            or claim.get('maker') != {'maker_type': claimed['maker_type'], 'agent_ref': claimed['principal_id']}):
        raise PermissionError('membership endpoints, maker or provenance exceed the exact grant')
    for key in ('evidence_refs', 'counterevidence_refs'):
        refs = claim.get(key, [])
        if (not isinstance(refs, list) or any(not isinstance(ref, str) for ref in refs)
                or not set(refs) <= set(config['allowed_evidence_refs'])):
            raise PermissionError('membership evidence exceeds its exact allowlist')
    _selections(request['forms'], config['allowed_collection_form_ids'])
    _selections(request['claim_forms'], config['allowed_claim_form_ids'])


def _grammar(root, collection, work, claim):
    corpus_ref = 'ToS/contracts/corpus-record.schema.json'
    raw = source._read(root / corpus_ref, source.MAX_SET_BYTES)
    schema = source._json_object(raw)
    source.Draft202012Validator.check_schema(schema)
    validator = source.Draft202012Validator(schema, format_checker=source.FormatChecker())
    validator.validate(work)
    validator.validate(collection)
    profiles = SourceClaimProfiles(root)
    profiles.validate(claim, {work['record_id']: work, collection['record_id']: collection})
    refs = {**profiles.input_digests, corpus_ref: hashlib.sha256(raw).hexdigest()}
    for ref in (*FORM_CONTRACTS, 'ToS/contracts/provenance-event-v2.schema.json'):
        refs[ref] = hashlib.sha256(source._read(root / ref, source.MAX_SET_BYTES)).hexdigest()
    return refs


def _context(root, scope, request, before):
    collection = source._json_object(before['collection.json'])
    if collection.get('record_type') != 'collection' or collection.get('record_id') != scope['collection_id']:
        raise PermissionError('selected parent is not the delegated Collection')
    history = revisions._history(before, collection)
    records, claims, digests = _read_catalog(root, request.get('expected_publication'))
    parent = records.get(scope['collection_id'])
    if (parent is None or parent.get('source_record_ref') != scope['collection_source_path']
            or parent.get('record_sha256') != source._digest(source._canonical(collection))[7:]):
        raise source.JournalConflict('selected Collection catalog binding is absent or stale')
    if scope['claim_id'] in records or scope['claim_id'] in claims:
        raise source.JournalConflict('new membership Claim identity already occurs in the catalog')
    if any(entry.get('provenance_event_ref') == scope['provenance_event_id'] for entry in claims.values()):
        raise source.JournalConflict('membership provenance identity already occurs in the catalog')
    work_entry = records.get(scope['work_id'])
    if (work_entry is None or work_entry.get('record_type') != 'work'
            or work_entry.get('source_record_ref') != scope['work_source_path']):
        raise PermissionError('membership target is not an existing exact cataloged Work')
    work = _catalog_record(root, work_entry, digests)
    if source._canonical(work) != source._canonical(request['work']):
        raise source.JournalConflict('caller Work packet differs from current source metadata')
    selected = {identity: _catalog_claim(root, entry, digests) for identity, entry in claims.items()
                if entry.get('predicate') == 'contains_work' and entry.get('subject_ref') == scope['collection_id']}
    works = {scope['work_id']: work}
    retained = {}
    for identity, claim in selected.items():
        entry = records.get(claim.get('object'))
        if entry is None or entry.get('record_type') != 'work':
            raise ValueError('existing membership Claim has no cataloged Work endpoint')
        works[entry['record_id']] = _catalog_record(root, entry, digests)
        ref = claims[identity]['source_claim_file_ref']
        if Path(ref).name == SOURCE_CLAIM_BASENAME:
            verified = verify_compound(root, ref, claim, _parent_before=before, _verify_current=False)
            if verified['parent_receipt'] not in history['receipts']:
                raise source.JournalCorruption('native membership is not in this Collection lineage')
            retained[verified['transaction_id']] = verified['manifest_sha256']
        elif Path(ref).name == 'membership-claims.jsonl':
            matches = []
            for provenance in scope['retained_membership_provenance_refs']:
                raw = source._read(root / provenance, source.MAX_SET_BYTES)
                digests[provenance] = hashlib.sha256(raw).hexdigest()
                matches.extend(event for event in (source._json_object(line) for line in raw.splitlines() if line.strip())
                               if event.get('event_id') == claim.get('provenance_event_ref'))
            if (claim.get('claim_type') != 'bibliographic'
                    or claim.get('assertion_layer') not in {'bibliographic_assertion', 'scholarly_report'}
                    or len(matches) != 1 or not any(output.get('ref') == ref and output.get('sha256') == digests[ref]
                    for output in matches[0].get('outputs', []))):
                raise source.JournalCorruption('legacy membership stream is not bound by its exact retained batch')
        else:
            raise PermissionError('existing membership has no declared evidence carrier')
    validate_collection_membership_closure(collection, works, selected.values())
    for ref in [*request['claim']['evidence_refs'], *request['claim'].get('counterevidence_refs', [])]:
        if ref.startswith('ToS/'):
            path = transactions._path(ref)
            if ref != scope['collection_source_path']:
                digests[ref] = hashlib.sha256(source._read(root / path, source.MAX_SET_BYTES)).hexdigest()
            # The selected parent is already bound by exact before-package and
            # pending before/after verification; it necessarily changes here.
        else:
            validate_external_citation_address(ref)
    if any(ref not in claims for ref in request['claim'].get('alternative_claim_refs', [])):
        raise ValueError('alternative membership Claim must already have an exact catalog identity')
    return {'catalog_and_sources': digests, 'contracts': _grammar(root, collection, work, request['claim']),
            'implementation': {ref: hashlib.sha256(source._read(source.ROOT / ref, source.MAX_SET_BYTES)).hexdigest()
                               for ref in IMPLEMENTATIONS}, 'retained_transactions': retained}


def _transaction_id(request):
    return source._digest(source._canonical({'operation': OPERATION, 'command_id': request['command_id'],
        'owner_configuration': request['expected_configuration'], 'request_digest': source._digest(source._canonical(request))}))


def _archive_config(root, scope):
    # Read/archive storage grammar only, never an invented record.revise grant.
    return {'schema_version': source.CORPUS_SELECTED_REVISION_CONFIG, 'source_root': str(root),
            'source_path': scope['collection_source_path'], 'record_id': scope['collection_id']}


def validate_parent_receipt(receipt):
    request = receipt.get('request') if isinstance(receipt, dict) else None
    if not isinstance(request, dict):
        raise source.JournalCorruption('membership receipt lacks its exact original request')
    _request(request, create=True)
    publication = receipt.get('publication')
    if (not isinstance(publication, dict) or set(publication) != {'protocol', 'transaction_id', 'selected_files'}
            or publication['protocol'] != revisions.SELECTED_PROTOCOL
            or publication['transaction_id'] != _transaction_id(request)
            or publication['selected_files'] != sorted(revisions._selected_names(Path('collection.json')))
            or receipt.get('changed_fields') != ['membership_claim_refs']
            or request['claim'].get('predicate') != 'contains_work'
            or request['claim'].get('subject_ref') != receipt['previous_source']['id']
            or request['claim'].get('object') != request['work'].get('record_id')
            or not isinstance(request['fields']['membership_claim_refs'], list)
            or not request['fields']['membership_claim_refs']
            or request['fields']['membership_claim_refs'][-1] != request['claim'].get('claim_id')):
        raise source.JournalCorruption('invalid explicit Collection membership lineage binding')


def _new_directories(root, scope):
    base = Path(scope['claim_source_path']).parent
    os.close(source._owned_path(root / base.parent, directory=True))
    try:
        descriptor = source._owned_path(root / base, directory=True)
    except FileNotFoundError:
        return [base.as_posix()]
    os.close(descriptor)
    raise source.JournalConflict('new membership Claim home is already occupied')


def _compose(root, scope, request, before, dependencies, *, recorded_at, environment):
    collection_path = Path(scope['collection_source_path'])
    base = Path(scope['claim_source_path']).parent
    collection = source._json_object(before[collection_path.name])
    revised = {**collection, **request['fields'], 'record_version': collection['record_version'] + 1}
    work, claim = request['work'], request['claim']
    _grammar(root, revised, work, claim)
    validate_collection_membership_delta(collection, revised, work, claim)
    history = revisions._history(before, collection)
    if len(history['receipts']) >= revisions.MAX_REVISIONS:
        raise ValueError('parent Collection history capacity reached')
    formname = collection_path.stem + '.human-forms.json'
    previous_forms = source._json_object(before[formname]) if formname in before else None
    parent_forms, parent_views, parent_refs = _forms(revised, previous_forms, request['forms'], scope['principal_id'])
    claim_forms, claim_views, claim_refs = _forms(claim, None, request['claim_forms'], scope['principal_id'], claim=True)
    identifier = _transaction_id(request)
    parent_receipt = {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'reason': request['reason'], 'previous_source': _record_ref(collection), 'source': _record_ref(revised),
        'previous_revision': request['expected_revision'],
        'archive_path': revisions._archive_path({'record_id': scope['collection_id']}, request['expected_revision']).as_posix(),
        'dependencies': request['expected_dependencies'], 'changed_fields': ['membership_claim_refs'],
        'forms': parent_refs, 'grants_admission': False, 'request': request,
        'publication': {'protocol': revisions.SELECTED_PROTOCOL, 'transaction_id': identifier,
                        'selected_files': sorted(revisions._selected_names(collection_path))}}
    validate_parent_receipt(parent_receipt)
    parent = {collection_path.name: revisions._encode(revised), formname: revisions._encode(parent_forms),
        revisions.HISTORY: revisions._encode({'schema_version': 'tos_source_revision_history_v2',
            'record_id': scope['collection_id'], 'receipts': [*history['receipts'], parent_receipt]})}
    child = {SOURCE_CLAIM_BASENAME: source._canonical(claim) + b'\n',
        source.claim_forms_path(base / SOURCE_CLAIM_BASENAME, scope['claim_id']).name: revisions._encode(claim_forms)}
    outputs = {**{str(collection_path.parent / name): raw for name, raw in parent.items()},
               **{str(base / name): raw for name, raw in child.items()}}
    source._instant(recorded_at)
    source._keys(environment, {'runtime', 'runtime_version', 'runtime_artifact_sha256', 'backend',
                              'hardware_target', 'unicode_version', 'argv_sha256'})
    if (any(not isinstance(value, str) or not value for value in environment.values())
            or any(not re.fullmatch(r'[a-f0-9]{64}', environment[key])
                   for key in ('runtime_artifact_sha256', 'argv_sha256'))):
        raise ValueError('invalid retained runtime capture')
    event = _event(scope, request, before, outputs, environment, dependencies, recorded_at)
    source._validator_for_provenance(root).validate(event)
    child.update({REQUEST_FILE: source._canonical(request) + b'\n',
        ENVIRONMENT_FILE: source._canonical(environment) + b'\n',
        PROVENANCE_FILE: source._canonical(event) + b'\n'})
    files = {**{str(collection_path.parent / name): raw for name, raw in parent.items()},
             **{str(base / name): raw for name, raw in child.items()}}
    receipt = {'schema_version': RECEIPT, 'operation': OPERATION, 'transaction_id': identifier,
        'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'scope': {key: scope[key] for key in sorted(SCOPE_KEYS)}, 'dependencies': request['expected_dependencies'],
        'parent_before': _record_ref(collection), 'parent_after': _record_ref(revised),
        'parent_revision': request['expected_revision'], 'parent_archive_ref': parent_receipt['archive_path'],
        'parent_transition_sha256': source._digest(source._canonical(parent_receipt)),
        'parent_before_files': revisions._file_refs(before), 'work': _record_ref(work),
        'work_source_binding': _verify_work_binding(root, scope, work, dependencies), 'claim': _claim_ref(claim),
        'forms': {'collection': parent_refs, 'claim': claim_refs},
        'files': revisions._file_refs(files), 'grants_admission': False}
    child[RECEIPT_FILE] = revisions._encode(receipt)
    if any(len(raw) > source.MAX_SET_BYTES for raw in [*parent.values(), *child.values()]):
        raise ValueError('compound selected metadata exceeds the per-file budget')
    return parent, child, receipt, parent_receipt, {'collection': parent_views, 'claim': claim_views}


def _authorization(scope, request, dependencies):
    return {'schema_version': AUTHORIZATION, 'scope': {key: scope[key] for key in SCOPE_KEYS},
        'principal_id': scope['principal_id'], 'maker_type': scope['maker_type'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'command_id': request['command_id'],
        'request_digest': source._digest(source._canonical(request)), 'dependency_bindings': dependencies}


def _plan(scope, authorization, before, parent, child, directories):
    collection = Path(scope['collection_source_path'])
    base = Path(scope['claim_source_path']).parent
    return {'authorization': authorization, 'new_directories': directories, 'files': sorted([
        *({'path': str(collection.parent / name), 'before': before.get(name), 'after': parent[name]}
          for name in revisions._selected_names(collection)),
        *({'path': str(base / name), 'before': None, 'after': raw} for name, raw in child.items()),
    ], key=lambda item: item['path'])}


def _validate_plan(root, plan):
    """Reconstruct every intended byte, not an authorization claim in prose."""
    authority = plan['authorization']
    source._keys(authority, {'schema_version', 'scope', 'principal_id', 'maker_type', 'authority_ref',
                             'owner_configuration', 'command_id', 'request_digest', 'dependency_bindings'})
    if authority['schema_version'] != AUTHORIZATION:
        raise PermissionError('selected transaction is not the native membership adapter')
    _validate_scope_shape(authority['scope'])
    scope = {**authority['scope'], **{key: authority[key] for key in ('principal_id', 'maker_type', 'authority_ref')}}
    collection, base = Path(scope['collection_source_path']), Path(scope['claim_source_path']).parent
    rows = {item['path']: item for item in plan['files']}
    if len(rows) != len(plan['files']):
        raise source.JournalCorruption('duplicate compound selected path')
    request = source._json_object(rows[str(base / REQUEST_FILE)]['after'])
    receipt = source._json_object(rows[str(base / RECEIPT_FILE)]['after'])
    environment = source._json_object(rows[str(base / ENVIRONMENT_FILE)]['after'])
    _request(request, create=True)
    _scope({**scope, 'allowed_operations': [OPERATION]}, request)
    before = {name: rows[str(collection.parent / name)]['before'] for name in revisions._selected_names(collection)
              if str(collection.parent / name) in rows and rows[str(collection.parent / name)]['before'] is not None}
    if collection.name not in before:
        raise source.JournalCorruption('compound has no exact retained parent input')
    record = source._json_object(before[collection.name])
    if (_record_ref(record) != request['expected_source'] or revisions._revision(before) != request['expected_revision']
            or authority['owner_configuration'] != request['expected_configuration']
            or authority['command_id'] != request['command_id']
            or authority['request_digest'] != source._digest(source._canonical(request))
            or source._digest(source._canonical(authority['dependency_bindings'])) != request['expected_dependencies']):
        raise source.JournalCorruption('compound retained authorization does not bind its request and parent input')
    directories = plan['new_directories']
    if directories != [base.as_posix()]:
        raise PermissionError('membership transaction may create only its separate exact Claim home')
    parent, child, expected_receipt, parent_receipt, views = _compose(root, scope, request, before,
        authority['dependency_bindings'], recorded_at=receipt['recorded_at'], environment=environment)
    expected = _plan(scope, authority, before, parent, child, directories)
    if plan != expected or receipt != expected_receipt:
        raise source.JournalCorruption('compound retained plan does not reconstruct the exact whole before/after delta')
    archived, _ = revisions._read_archive(root, _archive_config(root, scope), parent_receipt)
    if archived != before:
        raise source.JournalCorruption('compound parent archive differs from transaction input bytes')
    return scope, request, before, parent, child, receipt, parent_receipt, views


def _verify_work_binding(root, scope, work, dependencies):
    """Resolve the recorded raw bytes only through the actual current lineage.

    This internal selected-source read also works under our own pending barrier;
    it never accepts a caller-selected archive or skips a transaction witness.
    """
    ref = scope['work_source_path']
    digest = dependencies['catalog_and_sources'].get(ref)
    if not isinstance(digest, str) or not re.fullmatch(r'[a-f0-9]{64}', digest):
        raise source.JournalCorruption('Work dependency lacks its exact original raw hash')
    files = revisions._selected_package(root / ref)
    current = source._json_object(files['work.json'])
    if current.get('record_id') != scope['work_id'] or current.get('record_type') != 'work':
        raise source.JournalCorruption('membership Work current typed identity changed')
    history = revisions._history(files, current)
    candidates = [files['work.json']]
    for receipt in history['receipts']:
        request = receipt['request']
        if request.get('operation') == 'work.expression.create':
            from source_expression_commands import validate_parent_receipt
            validate_parent_receipt(receipt)
        elif (request.get('schema_version') != 'tos_local_source_command_v1' or request.get('operation') != 'record.revise'
                or not isinstance(request.get('fields'), dict) or not request['fields']
                or not set(request['fields']) <= source.CORPUS_REVISION_FIELDS):
            raise source.JournalCorruption('Work lineage contains an undeclared metadata transition')
        archived, _ = revisions._read_archive(root, {'record_id': scope['work_id'], 'source_path': ref}, receipt)
        if 'publication' in receipt:
            inspected = transactions.inspect_transaction(root, receipt['publication']['transaction_id'])
            rows = {item['path']: item for item in inspected['plan']['files']}
            row = rows.get(ref)
            if (inspected['status'] != 'committed' or row is None or row['before'] != archived['work.json']
                    or _record_ref(source._json_object(row['after'])) != receipt['source']):
                raise source.JournalCorruption('Work selected revision is not a committed exact transition')
        candidates.append(archived['work.json'])
    matched = [raw for raw in candidates if hashlib.sha256(raw).hexdigest() == digest]
    if not matched or source._canonical(source._json_object(matched[0])) != source._canonical(work):
        raise source.JournalCorruption('Work packet does not resolve to exact current or continuously retained source bytes')
    return {'source_path': ref, 'source': _record_ref(work), 'source_sha256': 'sha256:' + digest,
            'source_bytes': len(matched[0])}


def verify_compound(root, claim_source_ref, claim, *, _parent_before=None, _verify_current=True):
    root = Path(root)
    snapshot = PublicationSnapshot(root) if _verify_current else None
    claim_path = transactions._path(claim_source_ref)
    receipt_raw = source._read(root / claim_path.parent / RECEIPT_FILE, source.MAX_SET_BYTES)
    receipt = source._json_object(receipt_raw)
    if receipt.get('schema_version') != RECEIPT:
        raise source.JournalCorruption('native membership Claim lacks its attachment receipt')
    inspected = transactions.inspect_transaction(root, receipt['transaction_id'])
    if inspected['status'] != 'committed':
        raise source.JournalCorruption('native membership has no committed compound publication')
    scope, request, before, parent, child, expected, parent_receipt, _ = _validate_plan(root, inspected['plan'])
    if (claim_path.as_posix() != scope['claim_source_path'] or receipt != expected or receipt_raw != child[RECEIPT_FILE]):
        raise source.JournalCorruption('native membership receipt or source locator differs from committed evidence')
    for name in (REQUEST_FILE, ENVIRONMENT_FILE, PROVENANCE_FILE):
        if source._read(root / claim_path.parent / name, source.MAX_SET_BYTES) != child[name]:
            raise source.JournalCorruption('native membership immutable capture was changed')
    import claim_revisions
    package = revisions._package(root / claim_path.parent)
    config = {'source_root': str(root), 'source_path': claim_path.as_posix(), 'claim_id': scope['claim_id']}
    initial = claim_revisions.creation_source_files(package, config)
    if (initial[SOURCE_CLAIM_BASENAME] != child[SOURCE_CLAIM_BASENAME]
            or claim_revisions._claims(package[SOURCE_CLAIM_BASENAME]) != {scope['claim_id']: claim}):
        raise source.JournalCorruption('current membership Claim does not retain the exact initial compound stream')
    # Corrections keep identity endpoints fixed; current profile/typed closure is
    # still checked, while old membership wording remains in exact history.
    _grammar(root, source._json_object(parent['collection.json']), request['work'], claim)
    validate_qualified_membership_claim(claim)
    current_parent = _parent_before if _parent_before is not None else revisions._selected_package(root / scope['collection_source_path'])
    collection = source._json_object(current_parent['collection.json'])
    if collection.get('record_id') != scope['collection_id'] or collection.get('record_type') != 'collection':
        raise source.JournalCorruption('membership parent current typed identity changed')
    history = revisions._history(current_parent, collection)
    if parent_receipt not in history['receipts']:
        raise source.JournalCorruption('attachment transition is absent from current Collection lineage')
    for item in history['receipts']:
        revisions._read_archive(root, _archive_config(root, scope), item)
    if _verify_current:
        snapshot.verify_current()
    return {'transaction_id': receipt['transaction_id'], 'manifest_sha256': inspected['manifest_sha256'],
        'parent_receipt': parent_receipt, 'claim': copy.deepcopy(claim), 'receipt': receipt,
        'event': source._json_object(child[PROVENANCE_FILE]), 'grants_admission': False, 'writes_to_source': False}


def _check_dependencies(*args, **kwargs):
    return common._check_dependencies(sys.modules[__name__], *args, **kwargs)


def _guard(*args, **kwargs):
    return common._guard(sys.modules[__name__], *args, **kwargs)


def _event(*args, **kwargs):
    return common._event(sys.modules[__name__], *args, **kwargs)


def _read_owner(owner):
    return configuration(source._json_object(source._read(owner, source.MAX_COMMAND_BYTES)), owner_config=owner)


def _claim_source_ref(scope):
    return scope['claim_source_path']


def verify_replay(root, scope, request):
    import claim_revisions
    current = claim_revisions._claims(source._read(root / scope['claim_source_path'], source.MAX_COMMAND_BYTES))
    claim = current.get(scope['claim_id'])
    if claim is None:
        raise source.JournalCorruption('committed attachment has no exact current Claim successor')
    verified = verify_compound(root, scope['claim_source_path'], claim)
    if verified['receipt']['request_digest'] != source._digest(source._canonical(request)):
        raise source.JournalConflict('replay request differs from the original committed attachment')
    return verified


def _prepare_fields(record, scope):
    return {'membership_claim_refs': [*record['membership_claim_refs'], scope['claim_id']]}


def _prepared_refs(receipt):
    return {'prepared_collection': receipt['parent_after'], 'prepared_work': receipt['work']}


def _source_descriptors(root):
    profiles = SourceClaimProfiles(root)
    corpus = source._json_object(source._read(root / CORPUS_REF, source.MAX_SET_BYTES))
    result = {kind: {'type_id': profiles.mappings[kind], 'record_type': kind, 'schema_ref': CORPUS_REF,
        'schema_version': corpus['properties']['schema_version']['const'], 'source_basename': kind + '.json'}
        for kind in ('collection', 'work')}
    profile = profiles.profiles['contains_work']
    route = profiles.schema_routes['contains_work', 'tos_source_relation_claim_v1']
    result['contains_work'] = {'relation_type_id': profiles.relations['contains_work']['relation_type_id'],
        'predicate': 'contains_work', 'reader': profile['reader'], 'schema_ref': route['schema_ref'],
        'schema_version': route['schema_version'], 'assertion_layers': list(profile['assertion_layers']),
        'source_basename': SOURCE_CLAIM_BASENAME}
    return result


def _result(config, configuration_digest, *, receipt=None, replayed=False, recovery=None, views=None):
    root = Path(config['source_root'])
    snapshot = PublicationSnapshot(root)
    files = revisions._selected_package(root / config['collection_source_path'])
    collection = source._json_object(files['collection.json'])
    revisions._history(files, collection)
    work_raw = source._read(root / config['work_source_path'], source.MAX_COMMAND_BYTES)
    work = source._json_object(work_raw)
    if work.get('record_type') != 'work' or work.get('record_id') != config['work_id']:
        raise source.JournalCorruption('described target is not the exact existing Work')
    result = {'schema_version': 'tos_collection_membership_result_v1', 'authentication': 'local-unix-account',
        'owner_configuration': configuration_digest, 'operation': OPERATION,
        'command_operations': ['describe', PREPARE, OPERATION, RECOVERY], 'allowed_operations': config['allowed_operations'],
        'source': _record_ref(collection), 'revision': revisions._revision(files), 'publication_snapshot': snapshot.token,
        'collection_source_path': config['collection_source_path'], 'claim_source_path': config['claim_source_path'],
        'work_record': work, 'work_source_binding': {'source_path': config['work_source_path'],
            'source': _record_ref(work), 'source_sha256': source._digest(work_raw), 'source_bytes': len(work_raw)},
        'source_profiles': _source_descriptors(root),
        'source_fields': [{key: value for key, value in field.items() if key not in ('pointer', 'context')}
                          for field in source.metadata_field_catalog(collection)],
        'scope': {key: config[key] for key in sorted(SCOPE_KEYS)}, 'receipt': receipt,
        'replayed': replayed, 'recovery': recovery, 'materializations': views, 'grants_admission': False}
    snapshot.verify_current()
    return result


def run_membership_command(owner, config, configuration_digest, path, request):
    return common.run_command(sys.modules[__name__], owner, config, configuration_digest, path, request)


def command_handlers():
    return (contract.Handler('native-collection-membership', (CONFIG,), (contract.describe(),
        contract.operation(PREPARE, PROPOSAL_KEYS, definition='Prepare one qualified membership Claim for an existing Collection and Work.', grants=(OPERATION,)),
        contract.operation(OPERATION, CREATE_KEYS - contract.BASE_KEYS,
            definition='Append exactly one membership Claim reference and publish its separate qualified Claim/form package.',
            mutation='selected_collection_and_new_claim_package', grants=(OPERATION,)), contract.recovery(RECOVERY)),
        run_membership_command, 'Attach a qualified contains_work assertion without creating or revising the existing Work.',
        configure=configuration, request_schema=REQUEST,
        owner_route='mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_COLLECTION_GROWTH.md',
        typed_handles=(CORPUS_REF, 'ToS/contracts/source-relation-claim.schema.json', *contract.CLAIM_HANDLES, *FORM_CONTRACTS),
        profile_selection='Only contains_work under identity-relation-v1; the Claim needs statement, statement_language, statement_script and membership_scope.',
        preconditions=('Requires exact current Collection and Work byte bindings, a new relation home, bounded forms and evidence allowlists.',
                       'URL addresses are not observed remote contents; competing Claims retain separate identities.'),
        manages_publication=True),)


"""Separately delegated native Work -> Expression growth and public evidence.

The exact selected parent and new child travel through the cooperating metadata
publication protocol. No old grant is widened, no descendant is enumerated, and
a committed transport is not bibliographic, textual or assessment admission.
"""
from __future__ import annotations

import copy
from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import platform
import re
import sys
import unicodedata

import source_compound_commands as common
from source_compound_commands import _record_ref, _claim_ref, _selections, _forms, _read_catalog, _catalog_record, _catalog_claim, _environment

import source_commands as source
import source_revisions as revisions
import source_metadata_transactions as transactions
from source_metadata_snapshot import PublicationSnapshot
from source_bibliographic_topology import validate_work_expression_delta
from source_record_profiles import SourceClaimProfiles, SourceRecordProfiles, SOURCE_CLAIM_BASENAME, CORPUS_REF
from build_source_witness_catalog import RECORD_FILES, ADAPTED_RECORD_FILES, verify_catalog_publication

CONFIG = 'tos_local_work_expression_owner_v1'
REQUEST = 'tos_local_work_expression_command_v1'
OPERATION = 'work.expression.create'
RECOVERY = 'work.expression.recover'
AUTHORIZATION = 'tos_work_expression_authorization_v1'
RECEIPT = 'tos_work_expression_receipt_v1'
RECEIPT_FILE = 'work-expression-receipt.json'
REQUEST_FILE = 'source-create-request.json'
ENVIRONMENT_FILE = 'source-create-environment.json'
PROVENANCE_FILE = 'source-create-provenance.jsonl'
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py'
CATALOG_MANIFEST = 'ToS/source-witnesses/catalog/catalog.manifest.json'
MAX_CATALOG_BYTES = 16 * 1024 * 1024
MAX_CATALOG_ROWS = 8192
HASH = re.compile(r'sha256:[a-f0-9]{64}')
SCOPE_KEYS = {'work_id', 'work_source_path', 'expression_id', 'expression_source_path', 'claim_id',
              'provenance_event_id', 'allowed_work_form_ids', 'allowed_expression_form_ids', 'allowed_claim_form_ids'}
CONFIG_KEYS = SCOPE_KEYS | {'schema_version', 'uid', 'principal_id', 'maker_type', 'source_root',
                          'authority_ref', 'expires_at', 'allowed_operations'}
PROPOSAL_KEYS = {'record', 'claim', 'forms', 'expression_forms', 'claim_forms', 'reason'}
CREATE_KEYS = PROPOSAL_KEYS | {'schema_version', 'operation', 'command_id', 'fields',
    'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies', 'expected_publication'}
IMPLEMENTATIONS = (MODULE_REF, common.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py',
    'scripts/source_bibliographic_topology.py', 'scripts/source_metadata_snapshot.py',
    'scripts/source_witness_human_forms.py', 'scripts/source_record_profiles.py',
    'scripts/build_source_witness_catalog.py')
PREPARE = 'prepare-create'
RECOVERY_AUTHORIZATION = 'tos_work_expression_recovery_authorization_v1'
PARENT_ID = 'work_id'
EVENT_PROFILE = {
    'warning': 'Observed denotes the declared record link, not accepted bibliographic or textual truth.',
    'executor': 'software:tos-source-expression-commands',
    'procedure': 'native-work-expression-metadata-serialization',
    'purpose': 'Serialize one declared parent link and explicit source-copy forms without judging their content.',
    'component': 'ToS native Work Expression adapter',
}
FORM_CONTRACTS = ('ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
                  'ToS/contracts/human-form-template.schema.json')






def _validate_scope_shape(scope):
    source._keys(scope, SCOPE_KEYS)
    for key, prefix in (('work_id', 'work'), ('expression_id', 'expression'),
                        ('claim_id', 'claim'), ('provenance_event_id', 'event')):
        if not isinstance(scope[key], str) or not re.fullmatch(r'tos\.' + prefix + r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*', scope[key]):
            raise PermissionError('compound scope requires exact typed identities')
    work = transactions._path(scope['work_source_path'])
    expression = transactions._path(scope['expression_source_path'])
    if (work.name != 'work.json' or work.parts[:3] != ('ToS', 'source-witnesses', 'works')
            or len(work.parts) < 5 or expression.name != 'expression.json'
            or expression.parent.parent != work.parent / 'expressions'
            or not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', expression.parent.name)):
        raise PermissionError('compound scope must name one new child of the exact Work home')
    seen = set()
    for name in ('allowed_work_form_ids', 'allowed_expression_form_ids', 'allowed_claim_form_ids'):
        values = scope[name]
        if (not isinstance(values, list) or not 1 <= len(values) <= 32
                or any(not isinstance(value, str) or not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value)
                       for value in values) or len(set(values)) != len(values) or seen.intersection(values)):
            raise PermissionError('compound form identities must be bounded and distinct across subjects')
        seen.update(values)


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
    return config, source._digest(source._canonical(config)), root / config['work_source_path']


def _request(request, *, create=False):
    source._keys(request, CREATE_KEYS if create else PROPOSAL_KEYS | {'schema_version', 'operation'})
    if (request['schema_version'] != REQUEST or request['operation'] != (OPERATION if create else 'prepare-create')
            or len(source._canonical(request)) > source.MAX_COMMAND_BYTES
            or not isinstance(request['reason'], str) or not 1 <= len(request['reason'].strip()) <= 4096):
        raise ValueError('invalid bounded compound source request')
    if create:
        if (not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256
                or any(not isinstance(request[key], str) or not HASH.fullmatch(request[key])
                       for key in ('expected_configuration', 'expected_revision', 'expected_dependencies'))
                or request['expected_publication'] is not None and (
                    not isinstance(request['expected_publication'], str) or not HASH.fullmatch(request['expected_publication']))
                or not isinstance(request['fields'], dict) or set(request['fields']) != {'expression_claim_refs'}):
            raise ValueError('compound publication requires exact prepared lineage and dependency bindings')




def _scope(config, request, *, recovery=False, original=None):
    if (RECOVERY if recovery else OPERATION) not in config['allowed_operations']:
        raise PermissionError('compound source operation is not delegated')
    claimed = original if recovery else config
    record, claim = request['record'], request['claim']
    if (not isinstance(record, dict) or record.get('record_id') != config['expression_id']
            or record.get('work_ref') != config['work_id'] or not isinstance(claim, dict)
            or claim.get('claim_id') != config['claim_id'] or claim.get('subject_ref') != config['work_id']
            or claim.get('object') != config['expression_id']
            or claim.get('provenance_event_ref') != config['provenance_event_id']
            or claim.get('maker') != {'maker_type': claimed['maker_type'], 'agent_ref': claimed['principal_id']}):
        raise PermissionError('compound identities, endpoints, provenance or maker are not delegated')
    for field, allowed in (('forms', 'allowed_work_form_ids'), ('expression_forms', 'allowed_expression_form_ids'),
                           ('claim_forms', 'allowed_claim_form_ids')):
        _selections(request[field], config[allowed])


def _grammar(root, work, expression, claim):
    corpus_ref = 'ToS/contracts/corpus-record.schema.json'
    raw = source._read(root / corpus_ref, source.MAX_SET_BYTES)
    schema = source._json_object(raw)
    source.Draft202012Validator.check_schema(schema)
    validator = source.Draft202012Validator(schema, format_checker=source.FormatChecker())
    validator.validate(work)
    validator.validate(expression)
    profiles = SourceClaimProfiles(root)
    profiles.validate(claim, {work['record_id']: work, expression['record_id']: expression})
    refs = {**profiles.input_digests, corpus_ref: hashlib.sha256(raw).hexdigest()}
    for ref in (*FORM_CONTRACTS, 'ToS/contracts/provenance-event-v2.schema.json'):
        refs[ref] = hashlib.sha256(source._read(root / ref, source.MAX_SET_BYTES)).hexdigest()
    return refs










def _context(root, scope, request, before):
    """Bind the existing Work closure via exact catalog/source locators."""
    work_path = Path(scope['work_source_path'])
    work = source._json_object(before[work_path.name])
    if work.get('record_id') != scope['work_id'] or work.get('record_type') != 'work':
        raise PermissionError('selected parent is not the exact delegated Work')
    history = revisions._history(before, work)
    records, claims, digests = _read_catalog(root, request.get('expected_publication'))
    entry = records.get(scope['work_id'])
    if (entry is None or entry.get('source_record_ref') != scope['work_source_path']
            or entry.get('record_sha256') != source._digest(source._canonical(work))[7:]):
        raise source.JournalConflict('the selected parent Work is absent or stale in its catalog')
    if scope['expression_id'] in records or scope['expression_id'] in claims or scope['claim_id'] in records or scope['claim_id'] in claims:
        raise source.JournalConflict('new compound identity already occurs in the catalog')
    if any(entry.get('provenance_event_ref') == scope['provenance_event_id'] for entry in claims.values()):
        raise source.JournalConflict('new compound provenance identity already belongs to a cataloged Claim')
    expressions = {identity: _catalog_record(root, entry, digests)
                   for identity, entry in records.items() if entry.get('record_type') == 'expression'
                   and entry.get('links', {}).get('work_ref') == scope['work_id']}
    selected_claims = {identity: _catalog_claim(root, entry, digests)
                       for identity, entry in claims.items() if entry.get('predicate') == 'has_expression'
                       and entry.get('subject_ref') == scope['work_id']}
    if (set(work.get('expression_claim_refs', [])) != set(selected_claims)
            or len(work.get('expression_claim_refs', [])) != len(selected_claims)
            or {claim['object'] for claim in selected_claims.values()} != set(expressions)
            or len(selected_claims) != len(expressions)
            or any(expression.get('work_ref') != scope['work_id'] for expression in expressions.values())):
        raise ValueError('existing Work topology does not have exact forward/backlink closure')
    retained = {}
    for identity, claim in selected_claims.items():
        if Path(claims[identity]['source_claim_file_ref']).name == SOURCE_CLAIM_BASENAME:
            verified = verify_compound(root, claims[identity]['source_claim_file_ref'], claim,
                                       _parent_before=before, _verify_current=False)
            if verified['parent_receipt'] not in history['receipts']:
                raise source.JournalCorruption('native topology Claim is not in this parent Work lineage')
            retained[verified['transaction_id']] = verified['manifest_sha256']
        else:
            if (claim.get('claim_type') != 'bibliographic' or claim.get('assertion_layer') != 'bibliographic_assertion'
                    or claim.get('provenance_event_ref') != 'tos.event.annotation.source-witness-bibliographic-topology.2026-07-31'):
                raise PermissionError('existing topology Claim has no declared legacy or native evidence route')
            legacy_ref = 'ToS/source-witnesses/relations/provenance.jsonl'
            raw = source._read(root / legacy_ref, source.MAX_SET_BYTES)
            rows = [source._json_object(line) for line in raw.splitlines() if line.strip()]
            if len(rows) != 1 or not any(output.get('ref') == claims[identity]['source_claim_file_ref']
                    and output.get('sha256') == digests[claims[identity]['source_claim_file_ref']]
                    for output in rows[0].get('outputs', [])):
                raise source.JournalCorruption('legacy topology stream is not bound by its retained batch')
            digests[legacy_ref] = hashlib.sha256(raw).hexdigest()
    contract_digests = _grammar(root, work, request['record'], request['claim'])
    implementation = {ref: hashlib.sha256(source._read(source.ROOT / ref, source.MAX_SET_BYTES)).hexdigest()
                      for ref in IMPLEMENTATIONS}
    return {'catalog_and_sources': digests, 'contracts': contract_digests,
            'implementation': implementation, 'retained_transactions': retained}


def _transaction_id(request):
    return source._digest(source._canonical({'operation': OPERATION, 'command_id': request['command_id'],
        'owner_configuration': request['expected_configuration'], 'request_digest': source._digest(source._canonical(request))}))


def _archive_config(root, scope):
    # Read/archive storage grammar only, never an invented record.revise grant.
    return {'schema_version': source.CORPUS_SELECTED_REVISION_CONFIG, 'source_root': str(root),
            'source_path': scope['work_source_path'], 'record_id': scope['work_id']}


def validate_parent_receipt(receipt):
    """Scope-independent retained-history shape; no current grant is consulted."""
    request = receipt.get('request') if isinstance(receipt, dict) else None
    if not isinstance(request, dict):
        raise source.JournalCorruption('compound parent receipt lacks its exact original request')
    _request(request, create=True)
    publication = receipt.get('publication')
    if (not isinstance(publication, dict) or set(publication) != {'protocol', 'transaction_id', 'selected_files'}
            or publication['protocol'] != revisions.SELECTED_PROTOCOL
            or publication['transaction_id'] != _transaction_id(request)
            or publication['selected_files'] != sorted(revisions._selected_names(Path('work.json')))
            or receipt.get('changed_fields') != ['expression_claim_refs']
            or request['claim'].get('predicate') != 'has_expression'
            or request['claim'].get('subject_ref') != receipt['previous_source']['id']
            or request['record'].get('work_ref') != receipt['previous_source']['id']
            or request['claim'].get('object') != request['record'].get('record_id')
            or not isinstance(request['fields']['expression_claim_refs'], list)
            or not request['fields']['expression_claim_refs']
            or request['fields']['expression_claim_refs'][-1] != request['claim'].get('claim_id')):
        raise source.JournalCorruption('invalid explicit compound parent lineage binding')


def _new_directories(root, scope):
    child = Path(scope['expression_source_path']).parent
    result = []
    for path in (child.parent, child):
        try:
            descriptor = source._owned_path(root / path, directory=True)
        except FileNotFoundError:
            result.append(path.as_posix())
        else:
            os.close(descriptor)
            if path == child:
                raise source.JournalConflict('the new Expression home is already occupied')
    return result




def _event(*args, **kwargs):
    return common._event(sys.modules[__name__], *args, **kwargs)



def _compose(root, scope, request, before, dependencies, *, recorded_at, environment):
    work_path = Path(scope['work_source_path'])
    base = Path(scope['expression_source_path']).parent
    work = source._json_object(before[work_path.name])
    revised = {**work, **request['fields'], 'record_version': work['record_version'] + 1}
    expression, claim = request['record'], request['claim']
    _grammar(root, revised, expression, claim)
    validate_work_expression_delta(work, revised, expression, claim,
        work_source_ref=scope['work_source_path'], expression_source_ref=scope['expression_source_path'])
    if (any(value.get('status') != 'unverified' for field in ('variant_labels', 'external_identifiers')
            for value in expression[field]) or not expression.get('language') or not expression.get('expression_role')):
        raise PermissionError('a new Expression needs explicit language/role and unverified identity variants')
    history = revisions._history(before, work)
    if len(history['receipts']) >= revisions.MAX_REVISIONS:
        raise ValueError('parent Work history capacity reached')
    formname = work_path.stem + '.human-forms.json'
    previous_forms = source._json_object(before[formname]) if formname in before else None
    parent_forms, parent_views, parent_refs = _forms(revised, previous_forms, request['forms'], scope['principal_id'])
    expression_forms, expression_views, expression_refs = _forms(expression, None, request['expression_forms'], scope['principal_id'])
    claim_forms, claim_views, claim_refs = _forms(claim, None, request['claim_forms'], scope['principal_id'], claim=True)
    identifier = _transaction_id(request)
    parent_receipt = {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'reason': request['reason'], 'previous_source': _record_ref(work), 'source': _record_ref(revised),
        'previous_revision': request['expected_revision'],
        'archive_path': revisions._archive_path({'record_id': scope['work_id']}, request['expected_revision']).as_posix(),
        'dependencies': request['expected_dependencies'], 'changed_fields': ['expression_claim_refs'],
        'forms': parent_refs, 'grants_admission': False, 'request': request,
        'publication': {'protocol': revisions.SELECTED_PROTOCOL, 'transaction_id': identifier,
                        'selected_files': sorted(revisions._selected_names(work_path))}}
    validate_parent_receipt(parent_receipt)
    parent = {work_path.name: revisions._encode(revised), formname: revisions._encode(parent_forms),
        revisions.HISTORY: revisions._encode({'schema_version': 'tos_source_revision_history_v2',
            'record_id': scope['work_id'], 'receipts': [*history['receipts'], parent_receipt]})}
    child = {'expression.json': revisions._encode(expression),
        'expression.human-forms.json': revisions._encode(expression_forms),
        SOURCE_CLAIM_BASENAME: source._canonical(claim) + b'\n',
        source.claim_forms_path(base / SOURCE_CLAIM_BASENAME, scope['claim_id']).name: revisions._encode(claim_forms)}
    outputs = {**{str(work_path.parent / name): raw for name, raw in parent.items()},
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
    files = {**{str(work_path.parent / name): raw for name, raw in parent.items()},
             **{str(base / name): raw for name, raw in child.items()}}
    receipt = {'schema_version': RECEIPT, 'operation': OPERATION, 'transaction_id': identifier,
        'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'scope': {key: scope[key] for key in sorted(SCOPE_KEYS)}, 'dependencies': request['expected_dependencies'],
        'parent_before': _record_ref(work), 'parent_after': _record_ref(revised),
        'parent_revision': request['expected_revision'], 'parent_archive_ref': parent_receipt['archive_path'],
        'parent_transition_sha256': source._digest(source._canonical(parent_receipt)),
        'parent_before_files': revisions._file_refs(before), 'expression': _record_ref(expression), 'claim': _claim_ref(claim),
        'forms': {'work': parent_refs, 'expression': expression_refs, 'claim': claim_refs},
        'files': revisions._file_refs(files), 'grants_admission': False}
    child[RECEIPT_FILE] = revisions._encode(receipt)
    if any(len(raw) > source.MAX_SET_BYTES for raw in [*parent.values(), *child.values()]):
        raise ValueError('compound selected metadata exceeds the per-file budget')
    return parent, child, receipt, parent_receipt, {'work': parent_views, 'expression': expression_views, 'claim': claim_views}


def _authorization(scope, request, dependencies):
    return {'schema_version': AUTHORIZATION, 'scope': {key: scope[key] for key in SCOPE_KEYS},
        'principal_id': scope['principal_id'], 'maker_type': scope['maker_type'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'command_id': request['command_id'],
        'request_digest': source._digest(source._canonical(request)), 'dependency_bindings': dependencies}


def _plan(scope, authorization, before, parent, child, directories):
    work = Path(scope['work_source_path'])
    base = Path(scope['expression_source_path']).parent
    return {'authorization': authorization, 'new_directories': directories, 'files': sorted([
        *({'path': str(work.parent / name), 'before': before.get(name), 'after': parent[name]}
          for name in revisions._selected_names(work)),
        *({'path': str(base / name), 'before': None, 'after': raw} for name, raw in child.items()),
    ], key=lambda item: item['path'])}


def _validate_plan(root, plan):
    """Reconstruct every intended byte, not an authorization claim in prose."""
    authority = plan['authorization']
    source._keys(authority, {'schema_version', 'scope', 'principal_id', 'maker_type', 'authority_ref',
                             'owner_configuration', 'command_id', 'request_digest', 'dependency_bindings'})
    if authority['schema_version'] != AUTHORIZATION:
        raise PermissionError('selected transaction is not the native Work Expression adapter')
    _validate_scope_shape(authority['scope'])
    scope = {**authority['scope'], **{key: authority[key] for key in ('principal_id', 'maker_type', 'authority_ref')}}
    work, base = Path(scope['work_source_path']), Path(scope['expression_source_path']).parent
    rows = {item['path']: item for item in plan['files']}
    if len(rows) != len(plan['files']):
        raise source.JournalCorruption('duplicate compound selected path')
    request = source._json_object(rows[str(base / REQUEST_FILE)]['after'])
    receipt = source._json_object(rows[str(base / RECEIPT_FILE)]['after'])
    environment = source._json_object(rows[str(base / ENVIRONMENT_FILE)]['after'])
    _request(request, create=True)
    _scope({**scope, 'allowed_operations': [OPERATION]}, request)
    before = {name: rows[str(work.parent / name)]['before'] for name in revisions._selected_names(work)
              if str(work.parent / name) in rows and rows[str(work.parent / name)]['before'] is not None}
    if work.name not in before:
        raise source.JournalCorruption('compound has no exact retained parent input')
    record = source._json_object(before[work.name])
    if (_record_ref(record) != request['expected_source'] or revisions._revision(before) != request['expected_revision']
            or authority['owner_configuration'] != request['expected_configuration']
            or authority['command_id'] != request['command_id']
            or authority['request_digest'] != source._digest(source._canonical(request))
            or source._digest(source._canonical(authority['dependency_bindings'])) != request['expected_dependencies']):
        raise source.JournalCorruption('compound retained authorization does not bind its request and parent input')
    directories = plan['new_directories']
    if directories not in ([base.as_posix()], [base.parent.as_posix(), base.as_posix()]):
        raise PermissionError('compound transaction may create only its exact Expression home and missing expressions parent')
    parent, child, expected_receipt, parent_receipt, views = _compose(root, scope, request, before,
        authority['dependency_bindings'], recorded_at=receipt['recorded_at'], environment=environment)
    expected = _plan(scope, authority, before, parent, child, directories)
    if plan != expected or receipt != expected_receipt:
        raise source.JournalCorruption('compound retained plan does not reconstruct the exact whole before/after delta')
    archived, _ = revisions._read_archive(root, _archive_config(root, scope), parent_receipt)
    if archived != before:
        raise source.JournalCorruption('compound parent archive differs from transaction input bytes')
    return scope, request, before, parent, child, receipt, parent_receipt, views


def verify_compound(root, claim_source_ref, claim, *, _parent_before=None, _verify_current=True):
    """Read only verified committed native publication evidence for one Claim.

    The underscore arguments are internal pending-writer read plumbing, never
    owner-config or caller-request switches. Public callers use the defaults.
    """
    root = Path(root)
    snapshot = PublicationSnapshot(root) if _verify_current else None
    claim_path = transactions._path(claim_source_ref)
    if claim_path.name != SOURCE_CLAIM_BASENAME:
        raise ValueError('native compound Claim must use its declared source carrier')
    receipt_raw = source._read(root / claim_path.parent / RECEIPT_FILE, source.MAX_SET_BYTES)
    receipt = source._json_object(receipt_raw)
    if receipt.get('schema_version') != RECEIPT:
        raise source.JournalCorruption('native topology Claim lacks its compound receipt')
    inspected = transactions.inspect_transaction(root, receipt['transaction_id'])
    if inspected['status'] != 'committed':
        raise source.JournalCorruption('native topology Claim has no committed compound publication')
    scope, request, before, parent, child, expected, parent_receipt, _ = _validate_plan(root, inspected['plan'])
    if (claim_path != Path(scope['expression_source_path']).with_name(SOURCE_CLAIM_BASENAME)
            or receipt != expected or receipt_raw != child[RECEIPT_FILE] or claim != request['claim']
            or source._read(root / claim_path, source.MAX_SET_BYTES) != child[SOURCE_CLAIM_BASENAME]):
        raise source.JournalCorruption('native topology source is not the exact committed compound Claim')
    # Capture/receipt files remain immutable even after descriptive corrections.
    for name in (REQUEST_FILE, ENVIRONMENT_FILE, PROVENANCE_FILE):
        if source._read(root / claim_path.parent / name, source.MAX_SET_BYTES) != child[name]:
            raise source.JournalCorruption('native compound capture bytes were changed')
    current_parent = _parent_before if _parent_before is not None else revisions._selected_package(root / scope['work_source_path'])
    record = source._json_object(current_parent['work.json'])
    if record.get('record_id') != scope['work_id'] or record.get('record_type') != 'work':
        raise source.JournalCorruption('current compound parent changed its typed identity')
    history = revisions._history(current_parent, record)
    if parent_receipt not in history['receipts']:
        raise source.JournalCorruption('native compound transition is absent from the current parent lineage')
    for item in history['receipts']:
        revisions._read_archive(root, _archive_config(root, scope), item)
    if _verify_current:
        current_expression = revisions._selected_package(root / scope['expression_source_path'])
        expression = source._json_object(current_expression['expression.json'])
        if (expression.get('record_id') != scope['expression_id'] or expression.get('record_type') != 'expression'
                or expression.get('work_ref') != scope['work_id']):
            raise source.JournalCorruption('current Expression changed its typed parent binding')
        expression_history = revisions._history(current_expression, expression)
        initial_found = current_expression['expression.json'] == child['expression.json']
        for item in expression_history['receipts']:
            archived, _ = revisions._read_archive(root, {'record_id': scope['expression_id'],
                'source_path': scope['expression_source_path']}, item)
            if item['previous_source'] == receipt['expression']:
                if archived['expression.json'] != child['expression.json']:
                    raise source.JournalCorruption('Expression initial lineage does not retain exact compound bytes')
                initial_found = True
        if not initial_found:
            raise source.JournalCorruption('current Expression does not descend from its committed initial record')
        snapshot.verify_current()
    return {'transaction_id': receipt['transaction_id'], 'manifest_sha256': inspected['manifest_sha256'],
        'parent_receipt': parent_receipt, 'claim': copy.deepcopy(claim), 'receipt': receipt,
        'event': source._json_object(child[PROVENANCE_FILE]),
        'grants_admission': False, 'writes_to_source': False}


def _check_dependencies(*args, **kwargs):
    return common._check_dependencies(sys.modules[__name__], *args, **kwargs)



def _read_owner(owner):
    return configuration(source._json_object(source._read(owner, source.MAX_COMMAND_BYTES)), owner_config=owner)


def _guard(*args, **kwargs):
    return common._guard(sys.modules[__name__], *args, **kwargs)



def _source_descriptors(root):
    """Expose existing owner contracts, without a second registry or grant."""
    claims = SourceClaimProfiles(root)
    corpus = source._json_object(source._read(root / CORPUS_REF, source.MAX_SET_BYTES))
    result = {kind: {'type_id': claims.mappings[kind], 'record_type': kind,
        'schema_ref': CORPUS_REF, 'schema_version': corpus['properties']['schema_version']['const'],
        'source_basename': kind + '.json'} for kind in ('work', 'expression')}
    profile = claims.profiles['has_expression']
    route = claims.schema_routes['has_expression', 'tos_source_relation_claim_v1']
    result['has_expression'] = {'relation_type_id': claims.relations['has_expression']['relation_type_id'],
        'predicate': 'has_expression', 'reader': profile['reader'],
        'schema_ref': route['schema_ref'], 'schema_version': route['schema_version'],
        'assertion_layers': list(profile['assertion_layers']), 'source_basename': SOURCE_CLAIM_BASENAME}
    return result


def _result(config, configuration_digest, *, receipt=None, replayed=False, recovery=None, views=None):
    root = Path(config['source_root'])
    snapshot = PublicationSnapshot(root)
    path = root / config['work_source_path']
    files = revisions._selected_package(path)
    work = source._json_object(files[path.name])
    revisions._history(files, work)
    result = {'schema_version': 'tos_work_expression_result_v1', 'authentication': 'local-unix-account',
        'owner_configuration': configuration_digest, 'operation': OPERATION,
        'command_operations': ['describe', 'prepare-create', OPERATION, RECOVERY],
        'allowed_operations': config['allowed_operations'], 'work_source_path': config['work_source_path'],
        'expression_source_path': config['expression_source_path'], 'source': _record_ref(work),
        'revision': revisions._revision(files), 'publication_snapshot': snapshot.token,
        'source_profiles': _source_descriptors(root),
        'source_fields': [{key: value for key, value in field.items() if key not in ('pointer', 'context')}
                          for field in source.metadata_field_catalog(work)],
        'scope': {key: config[key] for key in SCOPE_KEYS}, 'receipt': receipt,
        'replayed': replayed, 'recovery': recovery, 'materializations': views,
        'grants_admission': False}
    snapshot.verify_current()
    return result


def run_expression_command(owner, config, configuration_digest, path, request):
    return common.run_command(sys.modules[__name__], owner, config, configuration_digest, path, request)


def _claim_source_ref(scope):
    return Path(scope['expression_source_path']).with_name(SOURCE_CLAIM_BASENAME).as_posix()


def verify_replay(root, scope, request):
    return verify_compound(root, _claim_source_ref(scope), request['claim'])


def _prepare_fields(record, scope):
    return {'expression_claim_refs': [*record['expression_claim_refs'], scope['claim_id']]}


def _prepared_refs(receipt):
    return {'prepared_work': receipt['parent_after'], 'prepared_expression': receipt['expression']}

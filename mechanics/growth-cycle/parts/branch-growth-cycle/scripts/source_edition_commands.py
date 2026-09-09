"""Separately delegated native Expression -> Edition growth and public evidence.

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
import re
import sys

import source_compound_commands as common
from source_compound_commands import _record_ref, _claim_ref, _selections, _forms, _read_catalog, _catalog_record, _catalog_claim, _environment

import source_commands as source
import source_command_contracts as contract
import source_revisions as revisions
import source_metadata_transactions as transactions
from source_metadata_snapshot import PublicationSnapshot
from source_bibliographic_topology import validate_expression_edition_delta
from source_record_profiles import SourceClaimProfiles, SOURCE_CLAIM_BASENAME, CORPUS_REF

CONFIG = 'tos_local_expression_edition_owner_v1'
REQUEST = 'tos_local_expression_edition_command_v1'
OPERATION = 'expression.edition.create'
RECOVERY = 'expression.edition.recover'
AUTHORIZATION = 'tos_expression_edition_authorization_v1'
RECEIPT = 'tos_expression_edition_receipt_v1'
RECEIPT_FILE = 'expression-edition-receipt.json'
REQUEST_FILE = 'source-create-request.json'
ENVIRONMENT_FILE = 'source-create-environment.json'
PROVENANCE_FILE = 'source-create-provenance.jsonl'
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_edition_commands.py'
CATALOG_MANIFEST = 'ToS/source-witnesses/catalog/catalog.manifest.json'
MAX_CATALOG_BYTES = 16 * 1024 * 1024
MAX_CATALOG_ROWS = 8192
HASH = re.compile(r'sha256:[a-f0-9]{64}')
SCOPE_KEYS = {'work_id', 'work_source_path', 'expression_id', 'expression_source_path', 'edition_id', 'edition_source_path', 'claim_id',
              'provenance_event_id', 'allowed_expression_form_ids', 'allowed_edition_form_ids', 'allowed_claim_form_ids'}
CONFIG_KEYS = SCOPE_KEYS | {'schema_version', 'uid', 'principal_id', 'maker_type', 'source_root',
                          'authority_ref', 'expires_at', 'allowed_operations'}
PROPOSAL_KEYS = {'record', 'claim', 'forms', 'edition_forms', 'claim_forms', 'reason'}
CREATE_KEYS = PROPOSAL_KEYS | {'schema_version', 'operation', 'command_id', 'fields',
    'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies', 'expected_publication'}
IMPLEMENTATIONS = (MODULE_REF, common.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py', contract.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_responsibility_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py',
    'scripts/source_bibliographic_topology.py', 'scripts/source_metadata_snapshot.py',
    'scripts/source_witness_human_forms.py', 'scripts/source_record_profiles.py',
    'scripts/build_source_witness_catalog.py')
PREPARE = 'prepare-create'
RECOVERY_AUTHORIZATION = 'tos_expression_edition_recovery_authorization_v1'
PARENT_ID = 'expression_id'
EVENT_PROFILE = {
    'warning': 'Observed denotes the declared record link, not accepted bibliographic or textual truth.',
    'executor': 'software:tos-source-edition-commands',
    'procedure': 'native-expression-edition-metadata-serialization',
    'purpose': 'Serialize one declared parent link and explicit source-copy forms without judging their content.',
    'component': 'ToS native Expression Edition adapter',
}
FORM_CONTRACTS = ('ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
                  'ToS/contracts/human-form-template.schema.json')

def _validate_scope_shape(scope):
    source._keys(scope, SCOPE_KEYS)
    for key, prefix in (('work_id', 'work'), ('expression_id', 'expression'), ('edition_id', 'edition'),
                        ('claim_id', 'claim'), ('provenance_event_id', 'event')):
        if not isinstance(scope[key], str) or not re.fullmatch(r'tos\.' + prefix + r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*', scope[key]):
            raise PermissionError('compound scope requires exact typed identities')
    work, expression, edition = (transactions._path(scope[key]) for key in
        ('work_source_path', 'expression_source_path', 'edition_source_path'))
    if (work.name != 'work.json' or work.parts[:3] != ('ToS', 'source-witnesses', 'works')
            or len(work.parts) < 5 or expression.name != 'expression.json'
            or expression.parent.parent != work.parent / 'expressions'
            or edition.name != 'edition.json' or edition.parent.parent != expression.parent / 'editions'
            or any(not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', path.parent.name)
                   for path in (expression, edition))):
        raise PermissionError('compound scope requires one existing Work/Expression and its exact new Edition home')
    seen = set()
    for name in ('allowed_expression_form_ids', 'allowed_edition_form_ids', 'allowed_claim_form_ids'):
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
    return config, source._digest(source._canonical(config)), root / config['expression_source_path']

def _request(request, *, create=False):
    source.command_handler(CONFIG).validate_request(request)
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
                or not isinstance(request['fields'], dict) or set(request['fields']) != {'embodiment_claim_refs'}):
            raise ValueError('compound publication requires exact prepared lineage and dependency bindings')

def _scope(config, request, *, recovery=False, original=None):
    if (RECOVERY if recovery else OPERATION) not in config['allowed_operations']:
        raise PermissionError('compound source operation is not delegated')
    claimed = original if recovery else config
    record, claim = request['record'], request['claim']
    if (not isinstance(record, dict) or record.get('record_id') != config['edition_id']
            or record.get('embodies_expression_refs') != [config['expression_id']] or not isinstance(claim, dict)
            or claim.get('claim_id') != config['claim_id'] or claim.get('subject_ref') != config['expression_id']
            or claim.get('object') != config['edition_id']
            or claim.get('provenance_event_ref') != config['provenance_event_id']
            or claim.get('maker') != {'maker_type': claimed['maker_type'], 'agent_ref': claimed['principal_id']}):
        raise PermissionError('compound identities, endpoints, provenance or maker are not delegated')
    for field, allowed in (('forms', 'allowed_expression_form_ids'), ('edition_forms', 'allowed_edition_form_ids'),
                           ('claim_forms', 'allowed_claim_form_ids')):
        _selections(request[field], config[allowed])

def _grammar(root, expression, edition, claim):
    corpus_ref = 'ToS/contracts/corpus-record.schema.json'
    raw = source._read(root / corpus_ref, source.MAX_SET_BYTES)
    schema = source._json_object(raw)
    source.Draft202012Validator.check_schema(schema)
    validator = source.Draft202012Validator(schema, format_checker=source.FormatChecker())
    validator.validate(expression)
    validator.validate(edition)
    profiles = SourceClaimProfiles(root)
    profiles.validate(claim, {expression['record_id']: expression, edition['record_id']: edition})
    refs = {**profiles.input_digests, corpus_ref: hashlib.sha256(raw).hexdigest()}
    for ref in (*FORM_CONTRACTS, 'ToS/contracts/provenance-event-v2.schema.json'):
        refs[ref] = hashlib.sha256(source._read(root / ref, source.MAX_SET_BYTES)).hexdigest()
    return refs

def _legacy_binding(root, entry, claim, digests):
    if (claim.get('claim_type') != 'bibliographic' or claim.get('assertion_layer') != 'bibliographic_assertion'
            or claim.get('provenance_event_ref') != 'tos.event.annotation.source-witness-bibliographic-topology.2026-07-31'):
        raise PermissionError('existing topology Claim has no declared legacy or native evidence route')
    ref = 'ToS/source-witnesses/relations/provenance.jsonl'
    raw = source._read(root / ref, source.MAX_SET_BYTES)
    rows = [source._json_object(line) for line in raw.splitlines() if line.strip()]
    if (len(rows) != 1 or rows[0].get('event_id') != claim['provenance_event_ref']
            or not any(output.get('ref') == entry['source_claim_file_ref']
                and output.get('sha256') == digests[entry['source_claim_file_ref']]
                for output in rows[0].get('outputs', []))):
        raise source.JournalCorruption('legacy topology stream is not bound by its retained batch')
    digests[ref] = hashlib.sha256(raw).hexdigest()

def _verify_initial_child(root, source_ref, files, receipt, child_key):
    """Check a committed child against exact pre-publication or current bytes.

    Pending recovery cannot use an ordinary current reader: its own selected
    parent may already be partially replaced. Only the retained before-package
    substitutes for that selected parent, never for another source or catalog.
    """
    path = Path(source_ref)
    record = source._json_object(files[path.name])
    reference = receipt[child_key]
    if record.get('record_id') != reference['id'] or record.get('record_type') != child_key:
        raise source.JournalCorruption('prior compound child changed its typed identity')
    expected = receipt['files'][source_ref]
    initial_found = revisions._file_refs({path.name: files[path.name]})[path.name] == expected
    for item in revisions._history(files, record)['receipts']:
        archived, _ = revisions._read_archive(root, {'record_id': reference['id'], 'source_path': source_ref}, item)
        if item['previous_source'] == reference:
            if revisions._file_refs({path.name: archived[path.name]})[path.name] != expected:
                raise source.JournalCorruption('prior compound child initial bytes changed in retained history')
            initial_found = True
    if not initial_found:
        raise source.JournalCorruption('selected child does not descend from its committed initial bytes')

def _context(root, scope, request, before):
    """Bind only the current Expression's declared Work and Edition neighborhood."""
    expression_path = Path(scope['expression_source_path'])
    expression = source._json_object(before[expression_path.name])
    if (expression.get('record_id') != scope['expression_id'] or expression.get('record_type') != 'expression'
            or expression.get('work_ref') != scope['work_id']):
        raise PermissionError('selected parent is not the exact delegated Expression and Work')
    history = revisions._history(before, expression)
    records, claims, digests = _read_catalog(root, request.get('expected_publication'))
    entry = records.get(scope['expression_id'])
    if (entry is None or entry.get('source_record_ref') != scope['expression_source_path']
            or entry.get('record_sha256') != source._digest(source._canonical(expression))[7:]):
        raise source.JournalConflict('the selected Expression is absent or stale in its catalog')
    if any(identity in records or identity in claims for identity in (scope['edition_id'], scope['claim_id'])):
        raise source.JournalConflict('new compound identity already occurs in the catalog')
    if any(entry.get('provenance_event_ref') == scope['provenance_event_id'] for entry in claims.values()):
        raise source.JournalConflict('new compound provenance identity already belongs to a cataloged Claim')
    work_entry = records.get(scope['work_id'])
    if (work_entry is None or work_entry.get('record_type') != 'work'
            or work_entry.get('source_record_ref') != scope['work_source_path']):
        raise PermissionError('the existing Work does not have its exact delegated catalog binding')
    work = _catalog_record(root, work_entry, digests)
    origins = [entry for entry in claims.values() if entry.get('predicate') == 'has_expression'
               and entry.get('object') == scope['expression_id']]
    if (len(origins) != 1 or origins[0].get('subject_ref') != scope['work_id']
            or not isinstance(work.get('expression_claim_refs'), list)
            or work['expression_claim_refs'].count(origins[0].get('claim_id')) != 1):
        raise ValueError('the existing Expression has no unique declared Work origin')
    origin = _catalog_claim(root, origins[0], digests)
    retained = {}
    if Path(origins[0]['source_claim_file_ref']).name == SOURCE_CLAIM_BASENAME:
        from source_expression_commands import verify_compound as verify_origin
        verified = verify_origin(root, origins[0]['source_claim_file_ref'], origin, _verify_current=False)
        _verify_initial_child(root, scope['expression_source_path'], before, verified['receipt'], 'expression')
        retained[verified['transaction_id']] = verified['manifest_sha256']
    else:
        _legacy_binding(root, origins[0], origin, digests)
    editions = {identity: _catalog_record(root, entry, digests)
                for identity, entry in records.items() if entry.get('record_type') == 'edition'
                and scope['expression_id'] in entry.get('links', {}).get('embodies_expression_refs', [])}
    selected = {identity: _catalog_claim(root, entry, digests) for identity, entry in claims.items()
                if entry.get('predicate') == 'embodied_by' and entry.get('subject_ref') == scope['expression_id']}
    if (set(expression.get('embodiment_claim_refs', [])) != set(selected)
            or len(expression.get('embodiment_claim_refs', [])) != len(selected)
            or {claim['object'] for claim in selected.values()} != set(editions)
            or len(selected) != len(editions)
            or any(scope['expression_id'] not in edition.get('embodies_expression_refs', []) for edition in editions.values())):
        raise ValueError('existing Expression topology does not have exact forward/backlink closure')
    for identity, claim in selected.items():
        if Path(claims[identity]['source_claim_file_ref']).name == SOURCE_CLAIM_BASENAME:
            verified = verify_compound(root, claims[identity]['source_claim_file_ref'], claim,
                                       _parent_before=before, _verify_current=False)
            if verified['parent_receipt'] not in history['receipts']:
                raise source.JournalCorruption('native embodiment is not in this Expression lineage')
            child_ref = records[claim['object']]['source_record_ref']
            _verify_initial_child(root, child_ref, revisions._selected_package(root / child_ref),
                                  verified['receipt'], 'edition')
            retained[verified['transaction_id']] = verified['manifest_sha256']
        else:
            _legacy_binding(root, claims[identity], claim, digests)
    contract_digests = _grammar(root, expression, request['record'], request['claim'])
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
            'source_path': scope['expression_source_path'], 'record_id': scope['expression_id']}

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
            or publication['selected_files'] != sorted(revisions._selected_names(Path('expression.json')))
            or receipt.get('changed_fields') != ['embodiment_claim_refs']
            or request['claim'].get('predicate') != 'embodied_by'
            or request['claim'].get('subject_ref') != receipt['previous_source']['id']
            or request['record'].get('embodies_expression_refs') != [receipt['previous_source']['id']]
            or request['claim'].get('object') != request['record'].get('record_id')
            or not isinstance(request['fields']['embodiment_claim_refs'], list)
            or not request['fields']['embodiment_claim_refs']
            or request['fields']['embodiment_claim_refs'][-1] != request['claim'].get('claim_id')):
        raise source.JournalCorruption('invalid explicit compound parent lineage binding')

def _new_directories(root, scope):
    child = Path(scope['edition_source_path']).parent
    result = []
    for path in (child.parent, child):
        try:
            descriptor = source._owned_path(root / path, directory=True)
        except FileNotFoundError:
            result.append(path.as_posix())
        else:
            os.close(descriptor)
            if path == child:
                raise source.JournalConflict('the new Edition home is already occupied')
    return result

def _event(*args, **kwargs):
    return common._event(sys.modules[__name__], *args, **kwargs)

def _compose(root, scope, request, before, dependencies, *, recorded_at, environment):
    expression_path = Path(scope['expression_source_path'])
    base = Path(scope['edition_source_path']).parent
    expression = source._json_object(before[expression_path.name])
    revised = {**expression, **request['fields'], 'record_version': expression['record_version'] + 1}
    edition, claim = request['record'], request['claim']
    _grammar(root, revised, edition, claim)
    validate_expression_edition_delta(expression, revised, edition, claim,
        expression_source_ref=scope['expression_source_path'], edition_source_ref=scope['edition_source_path'])
    history = revisions._history(before, expression)
    if len(history['receipts']) >= revisions.MAX_REVISIONS:
        raise ValueError('parent Expression history capacity reached')
    formname = expression_path.stem + '.human-forms.json'
    previous_forms = source._json_object(before[formname]) if formname in before else None
    parent_forms, parent_views, parent_refs = _forms(revised, previous_forms, request['forms'], scope['principal_id'])
    edition_forms, edition_views, edition_refs = _forms(edition, None, request['edition_forms'], scope['principal_id'])
    claim_forms, claim_views, claim_refs = _forms(claim, None, request['claim_forms'], scope['principal_id'], claim=True)
    identifier = _transaction_id(request)
    parent_receipt = {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'reason': request['reason'], 'previous_source': _record_ref(expression), 'source': _record_ref(revised),
        'previous_revision': request['expected_revision'],
        'archive_path': revisions._archive_path({'record_id': scope['expression_id']}, request['expected_revision']).as_posix(),
        'dependencies': request['expected_dependencies'], 'changed_fields': ['embodiment_claim_refs'],
        'forms': parent_refs, 'grants_admission': False, 'request': request,
        'publication': {'protocol': revisions.SELECTED_PROTOCOL, 'transaction_id': identifier,
                        'selected_files': sorted(revisions._selected_names(expression_path))}}
    validate_parent_receipt(parent_receipt)
    parent = {expression_path.name: revisions._encode(revised), formname: revisions._encode(parent_forms),
        revisions.HISTORY: revisions._encode({'schema_version': 'tos_source_revision_history_v2',
            'record_id': scope['expression_id'], 'receipts': [*history['receipts'], parent_receipt]})}
    child = {'edition.json': revisions._encode(edition),
        'edition.human-forms.json': revisions._encode(edition_forms),
        SOURCE_CLAIM_BASENAME: source._canonical(claim) + b'\n',
        source.claim_forms_path(base / SOURCE_CLAIM_BASENAME, scope['claim_id']).name: revisions._encode(claim_forms)}
    outputs = {**{str(expression_path.parent / name): raw for name, raw in parent.items()},
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
    files = {**{str(expression_path.parent / name): raw for name, raw in parent.items()},
             **{str(base / name): raw for name, raw in child.items()}}
    receipt = {'schema_version': RECEIPT, 'operation': OPERATION, 'transaction_id': identifier,
        'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'scope': {key: scope[key] for key in sorted(SCOPE_KEYS)}, 'dependencies': request['expected_dependencies'],
        'parent_before': _record_ref(expression), 'parent_after': _record_ref(revised),
        'parent_revision': request['expected_revision'], 'parent_archive_ref': parent_receipt['archive_path'],
        'parent_transition_sha256': source._digest(source._canonical(parent_receipt)),
        'parent_before_files': revisions._file_refs(before), 'edition': _record_ref(edition), 'claim': _claim_ref(claim),
        'forms': {'expression': parent_refs, 'edition': edition_refs, 'claim': claim_refs},
        'files': revisions._file_refs(files), 'grants_admission': False}
    child[RECEIPT_FILE] = revisions._encode(receipt)
    if any(len(raw) > source.MAX_SET_BYTES for raw in [*parent.values(), *child.values()]):
        raise ValueError('compound selected metadata exceeds the per-file budget')
    return parent, child, receipt, parent_receipt, {'expression': parent_views, 'edition': edition_views, 'claim': claim_views}

def _authorization(scope, request, dependencies):
    return {'schema_version': AUTHORIZATION, 'scope': {key: scope[key] for key in SCOPE_KEYS},
        'principal_id': scope['principal_id'], 'maker_type': scope['maker_type'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'command_id': request['command_id'],
        'request_digest': source._digest(source._canonical(request)), 'dependency_bindings': dependencies}

def _plan(scope, authorization, before, parent, child, directories):
    expression = Path(scope['expression_source_path'])
    base = Path(scope['edition_source_path']).parent
    return {'authorization': authorization, 'new_directories': directories, 'files': sorted([
        *({'path': str(expression.parent / name), 'before': before.get(name), 'after': parent[name]}
          for name in revisions._selected_names(expression)),
        *({'path': str(base / name), 'before': None, 'after': raw} for name, raw in child.items()),
    ], key=lambda item: item['path'])}

def _validate_plan(root, plan):
    """Reconstruct every intended byte, not an authorization claim in prose."""
    authority = plan['authorization']
    source._keys(authority, {'schema_version', 'scope', 'principal_id', 'maker_type', 'authority_ref',
                             'owner_configuration', 'command_id', 'request_digest', 'dependency_bindings'})
    if authority['schema_version'] != AUTHORIZATION:
        raise PermissionError('selected transaction is not the native Expression Edition adapter')
    _validate_scope_shape(authority['scope'])
    scope = {**authority['scope'], **{key: authority[key] for key in ('principal_id', 'maker_type', 'authority_ref')}}
    expression, base = Path(scope['expression_source_path']), Path(scope['edition_source_path']).parent
    rows = {item['path']: item for item in plan['files']}
    if len(rows) != len(plan['files']):
        raise source.JournalCorruption('duplicate compound selected path')
    request = source._json_object(rows[str(base / REQUEST_FILE)]['after'])
    receipt = source._json_object(rows[str(base / RECEIPT_FILE)]['after'])
    environment = source._json_object(rows[str(base / ENVIRONMENT_FILE)]['after'])
    _request(request, create=True)
    _scope({**scope, 'allowed_operations': [OPERATION]}, request)
    before = {name: rows[str(expression.parent / name)]['before'] for name in revisions._selected_names(expression)
              if str(expression.parent / name) in rows and rows[str(expression.parent / name)]['before'] is not None}
    if expression.name not in before:
        raise source.JournalCorruption('compound has no exact retained parent input')
    record = source._json_object(before[expression.name])
    if (_record_ref(record) != request['expected_source'] or revisions._revision(before) != request['expected_revision']
            or authority['owner_configuration'] != request['expected_configuration']
            or authority['command_id'] != request['command_id']
            or authority['request_digest'] != source._digest(source._canonical(request))
            or source._digest(source._canonical(authority['dependency_bindings'])) != request['expected_dependencies']):
        raise source.JournalCorruption('compound retained authorization does not bind its request and parent input')
    directories = plan['new_directories']
    if directories not in ([base.as_posix()], [base.parent.as_posix(), base.as_posix()]):
        raise PermissionError('compound transaction may create only its exact Edition home and missing editions parent')
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
    if (claim_path != Path(scope['edition_source_path']).with_name(SOURCE_CLAIM_BASENAME)
            or receipt != expected or receipt_raw != child[RECEIPT_FILE] or claim != request['claim']
            or source._read(root / claim_path, source.MAX_SET_BYTES) != child[SOURCE_CLAIM_BASENAME]):
        raise source.JournalCorruption('native topology source is not the exact committed compound Claim')
    # Capture/receipt files remain immutable even after descriptive corrections.
    for name in (REQUEST_FILE, ENVIRONMENT_FILE, PROVENANCE_FILE):
        if source._read(root / claim_path.parent / name, source.MAX_SET_BYTES) != child[name]:
            raise source.JournalCorruption('native compound capture bytes were changed')
    current_parent = _parent_before if _parent_before is not None else revisions._selected_package(root / scope['expression_source_path'])
    record = source._json_object(current_parent['expression.json'])
    if (record.get('record_id') != scope['expression_id'] or record.get('record_type') != 'expression'
            or record.get('work_ref') != scope['work_id']):
        raise source.JournalCorruption('current compound parent changed its typed identity')
    history = revisions._history(current_parent, record)
    if parent_receipt not in history['receipts']:
        raise source.JournalCorruption('native compound transition is absent from the current parent lineage')
    for item in history['receipts']:
        revisions._read_archive(root, _archive_config(root, scope), item)
    if _verify_current:
        current_edition = revisions._selected_package(root / scope['edition_source_path'])
        edition = source._json_object(current_edition['edition.json'])
        if (edition.get('record_id') != scope['edition_id'] or edition.get('record_type') != 'edition'
                or scope['expression_id'] not in edition.get('embodies_expression_refs', [])):
            raise source.JournalCorruption('current Edition changed its typed parent binding')
        edition_history = revisions._history(current_edition, edition)
        initial_found = current_edition['edition.json'] == child['edition.json']
        for item in edition_history['receipts']:
            archived, _ = revisions._read_archive(root, {'record_id': scope['edition_id'],
                'source_path': scope['edition_source_path']}, item)
            if item['previous_source'] == receipt['edition']:
                if archived['edition.json'] != child['edition.json']:
                    raise source.JournalCorruption('Edition initial lineage does not retain exact compound bytes')
                initial_found = True
        if not initial_found:
            raise source.JournalCorruption('current Edition does not descend from its committed initial record')
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
        'source_basename': kind + '.json'} for kind in ('expression', 'edition')}
    profile = claims.profiles['embodied_by']
    route = claims.schema_routes['embodied_by', 'tos_source_relation_claim_v1']
    result['embodied_by'] = {'relation_type_id': claims.relations['embodied_by']['relation_type_id'],
        'predicate': 'embodied_by', 'reader': profile['reader'],
        'schema_ref': route['schema_ref'], 'schema_version': route['schema_version'],
        'assertion_layers': list(profile['assertion_layers']), 'source_basename': SOURCE_CLAIM_BASENAME}
    return result

def _result(config, configuration_digest, *, receipt=None, replayed=False, recovery=None, views=None):
    root = Path(config['source_root'])
    snapshot = PublicationSnapshot(root)
    path = root / config['expression_source_path']
    files = revisions._selected_package(path)
    expression = source._json_object(files[path.name])
    revisions._history(files, expression)
    result = {'schema_version': 'tos_expression_edition_result_v1', 'authentication': 'local-unix-account',
        'owner_configuration': configuration_digest, 'operation': OPERATION,
        'command_operations': ['describe', 'prepare-create', OPERATION, RECOVERY],
        'allowed_operations': config['allowed_operations'], 'expression_source_path': config['expression_source_path'],
        'edition_source_path': config['edition_source_path'], 'source': _record_ref(expression),
        'revision': revisions._revision(files), 'publication_snapshot': snapshot.token,
        'source_profiles': _source_descriptors(root),
        'source_fields': [{key: value for key, value in field.items() if key not in ('pointer', 'context')}
                          for field in source.metadata_field_catalog(expression)],
        'scope': {key: config[key] for key in SCOPE_KEYS}, 'receipt': receipt,
        'replayed': replayed, 'recovery': recovery, 'materializations': views,
        'grants_admission': False}
    snapshot.verify_current()
    return result

def run_edition_command(owner, config, configuration_digest, path, request):
    return common.run_command(sys.modules[__name__], owner, config, configuration_digest, path, request)

def _claim_source_ref(scope):
    return Path(scope['edition_source_path']).with_name(SOURCE_CLAIM_BASENAME).as_posix()

def verify_replay(root, scope, request):
    return verify_compound(root, _claim_source_ref(scope), request['claim'])

def _prepare_fields(record, scope):
    return {'embodiment_claim_refs': [*record['embodiment_claim_refs'], scope['claim_id']]}

def _prepared_refs(receipt):
    return {'prepared_expression': receipt['parent_after'], 'prepared_edition': receipt['edition']}

def command_handlers():
    return (contract.Handler('native-expression-edition', (CONFIG,), (contract.describe(),
        contract.operation(PREPARE, PROPOSAL_KEYS, definition='Prepare one provisional Edition and exact parent Expression append.', grants=(OPERATION,)),
        contract.operation(OPERATION, CREATE_KEYS - contract.BASE_KEYS,
            definition='Create the new Edition, distinct embodied_by Claim and forms with one Expression successor.',
            mutation='selected_expression_and_new_edition_package', grants=(OPERATION,)), contract.recovery(RECOVERY)),
        run_edition_command, 'Separately delegated native Expression to Edition growth.', configure=configuration,
        request_schema=REQUEST, owner_route='mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_EXPRESSION_EDITION_GROWTH.md',
        typed_handles=(CORPUS_REF, 'ToS/contracts/source-relation-claim.schema.json', *contract.CLAIM_HANDLES, *FORM_CONTRACTS),
        profile_selection='Exact Expression and new Edition; embodied_by uses the canonical identity-relation-v1 profile.',
        preconditions=('Requires exact current catalog/parent/source-copy forms, absent child home and distinct Claim identity.',
                       'Only embodiment_claim_refs appends; no Item/File, publication, responsibility or equivalence is inferred.'),
        manages_publication=True),)

"""Native qualified translator attachment for one existing Expression and Agent.

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
import source_revisions as revisions
import source_metadata_transactions as transactions
import source_compound_commands as common
from source_compound_commands import _record_ref, _claim_ref, _selections, _forms, _read_catalog, _catalog_record, _catalog_claim
from source_metadata_snapshot import PublicationSnapshot
from source_record_profiles import SourceClaimProfiles, SOURCE_CLAIM_BASENAME, CORPUS_REF
from source_bibliographic_responsibility import validate_expression_responsibility_delta, validate_expression_responsibility_closure, validate_qualified_translator_claim
from source_witness_bibliographic_graph_common import validate_external_citation_address

CONFIG = 'tos_local_expression_responsibility_owner_v1'
REQUEST = 'tos_local_expression_responsibility_command_v1'
OPERATION = 'expression.responsibility.attach'
PREPARE = 'prepare-attach'
RECOVERY = 'expression.responsibility.recover'
AUTHORIZATION = 'tos_expression_responsibility_authorization_v1'
RECOVERY_AUTHORIZATION = 'tos_expression_responsibility_recovery_authorization_v1'
RECEIPT = 'tos_expression_responsibility_receipt_v1'
RECEIPT_FILE = 'responsibility-attachment-receipt.json'
REQUEST_FILE = 'source-create-request.json'
ENVIRONMENT_FILE = 'source-create-environment.json'
PROVENANCE_FILE = 'source-create-provenance.jsonl'
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_responsibility_commands.py'
PARENT_ID = 'expression_id'
HASH = re.compile(r'sha256:[a-f0-9]{64}')
SCOPE_KEYS = {'expression_id', 'expression_source_path', 'agent_id', 'agent_source_path', 'predicate',
              'claim_id', 'claim_source_path', 'provenance_event_id', 'allowed_expression_form_ids',
              'allowed_claim_form_ids', 'allowed_evidence_refs'}
CONFIG_KEYS = SCOPE_KEYS | {'schema_version', 'uid', 'principal_id', 'maker_type', 'source_root',
                          'authority_ref', 'expires_at', 'allowed_operations'}
PROPOSAL_KEYS = {'agent', 'claim', 'forms', 'claim_forms', 'reason'}
CREATE_KEYS = PROPOSAL_KEYS | {'schema_version', 'operation', 'command_id', 'fields',
    'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies', 'expected_publication'}
IMPLEMENTATIONS = (MODULE_REF, common.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py',
    'scripts/source_bibliographic_responsibility.py', 'scripts/source_metadata_snapshot.py',
    'scripts/source_witness_bibliographic_graph_common.py',
    'scripts/source_witness_human_forms.py', 'scripts/source_record_profiles.py',
    'scripts/build_source_witness_catalog.py')
FORM_CONTRACTS = ('ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
                  'ToS/contracts/human-form-template.schema.json')
EVENT_PROFILE = {
    'warning': 'A qualified attribution is supplied by the caller; serialization and URL presence do not prove source reading or its truth.',
    'executor': 'software:tos-source-responsibility-commands',
    'procedure': 'native-expression-responsibility-metadata-serialization',
    'purpose': 'Serialize one qualified translator Claim and an Expression responsibility reference without judging attribution.',
    'component': 'ToS native Expression responsibility adapter',
}


def _validate_scope_shape(scope):
    source._keys(scope, SCOPE_KEYS)
    for key, prefix in (('expression_id', 'expression'), ('agent_id', 'agent'),
                        ('claim_id', 'claim'), ('provenance_event_id', 'event')):
        if not isinstance(scope[key], str) or not re.fullmatch(r'tos\.' + prefix + r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*', scope[key]):
            raise PermissionError('responsibility scope requires exact typed identities')
    expression, agent, claim = (transactions._path(scope[key]) for key in
                              ('expression_source_path', 'agent_source_path', 'claim_source_path'))
    if (scope['predicate'] != 'translated_by' or expression.name != 'expression.json'
            or expression.parts[:3] != ('ToS', 'source-witnesses', 'works')
            or 'expressions' not in expression.parts or agent.name != 'agent.json'
            or agent.parts[:3] != ('ToS', 'source-witnesses', 'agents')
            or claim.parts[:3] != ('ToS', 'source-witnesses', 'relations') or len(claim.parts) != 5
            or claim.name != SOURCE_CLAIM_BASENAME
            or not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', claim.parent.name)):
        raise PermissionError('translator attachment requires exact public Expression, Agent and a separate relation home')
    seen = set()
    for key in ('allowed_expression_form_ids', 'allowed_claim_form_ids'):
        values = scope[key]
        if (not isinstance(values, list) or not 1 <= len(values) <= 32 or len(values) != len(set(values))
                or any(not isinstance(value, str) or not re.fullmatch(r'tos\.form\.[a-z0-9][a-z0-9._-]*', value) for value in values)
                or seen.intersection(values)):
            raise PermissionError('responsibility forms require distinct bounded subject-local identities')
        seen.update(values)
    evidence = scope['allowed_evidence_refs']
    if (not isinstance(evidence, list) or not 1 <= len(evidence) <= 128
            or any(not isinstance(value, str) or not value.strip() or len(value) > 4096 for value in evidence)
            or len(evidence) != len(set(evidence))):
        raise PermissionError('responsibility evidence requires a bounded explicit allowlist')


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
    source._keys(request, CREATE_KEYS if create else PROPOSAL_KEYS | {'schema_version', 'operation'})
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
                or not isinstance(request['fields'], dict) or set(request['fields']) != {'responsibility_claim_refs'}):
            raise ValueError('compound publication requires exact prepared lineage and dependency bindings')


def _scope(config, request, *, recovery=False, original=None):
    if (RECOVERY if recovery else OPERATION) not in config['allowed_operations']:
        raise PermissionError('responsibility operation is not delegated')
    claimed = original if recovery else config
    agent, claim = request['agent'], request['claim']
    if (not isinstance(agent, dict) or agent.get('record_id') != config['agent_id'] or agent.get('record_type') != 'agent'
            or not isinstance(claim, dict) or claim.get('claim_id') != config['claim_id']
            or claim.get('predicate') != config['predicate'] or claim.get('subject_ref') != config['expression_id']
            or claim.get('object') != config['agent_id'] or claim.get('provenance_event_ref') != config['provenance_event_id']
            or claim.get('maker') != {'maker_type': claimed['maker_type'], 'agent_ref': claimed['principal_id']}):
        raise PermissionError('responsibility endpoints, maker or provenance exceed the exact grant')
    for key in ('evidence_refs', 'counterevidence_refs'):
        refs = claim.get(key, [])
        if (not isinstance(refs, list) or any(not isinstance(ref, str) for ref in refs)
                or not set(refs) <= set(config['allowed_evidence_refs'])):
            raise PermissionError('responsibility evidence exceeds its exact allowlist')
    _selections(request['forms'], config['allowed_expression_form_ids'])
    _selections(request['claim_forms'], config['allowed_claim_form_ids'])


def _grammar(root, expression, agent, claim):
    corpus_ref = 'ToS/contracts/corpus-record.schema.json'
    raw = source._read(root / corpus_ref, source.MAX_SET_BYTES)
    schema = source._json_object(raw)
    source.Draft202012Validator.check_schema(schema)
    validator = source.Draft202012Validator(schema, format_checker=source.FormatChecker())
    validator.validate(agent)
    validator.validate(expression)
    profiles = SourceClaimProfiles(root)
    profiles.validate(claim, {agent['record_id']: agent, expression['record_id']: expression})
    refs = {**profiles.input_digests, corpus_ref: hashlib.sha256(raw).hexdigest()}
    for ref in (*FORM_CONTRACTS, 'ToS/contracts/provenance-event-v2.schema.json'):
        refs[ref] = hashlib.sha256(source._read(root / ref, source.MAX_SET_BYTES)).hexdigest()
    return refs


def _context(root, scope, request, before):
    expression = source._json_object(before['expression.json'])
    if expression.get('record_type') != 'expression' or expression.get('record_id') != scope['expression_id']:
        raise PermissionError('selected parent is not the delegated Expression')
    history = revisions._history(before, expression)
    records, claims, digests = _read_catalog(root, request.get('expected_publication'))
    parent = records.get(scope['expression_id'])
    if (parent is None or parent.get('source_record_ref') != scope['expression_source_path']
            or parent.get('record_sha256') != source._digest(source._canonical(expression))[7:]):
        raise source.JournalConflict('selected Expression catalog binding is absent or stale')
    if scope['claim_id'] in records or scope['claim_id'] in claims:
        raise source.JournalConflict('new responsibility Claim identity already occurs in the catalog')
    if any(entry.get('provenance_event_ref') == scope['provenance_event_id'] for entry in claims.values()):
        raise source.JournalConflict('responsibility provenance identity already occurs in the catalog')
    agent_entry = records.get(scope['agent_id'])
    if (agent_entry is None or agent_entry.get('record_type') != 'agent'
            or agent_entry.get('source_record_ref') != scope['agent_source_path']):
        raise PermissionError('responsibility target is not an existing exact cataloged Agent')
    agent = _catalog_record(root, agent_entry, digests)
    if source._canonical(agent) != source._canonical(request['agent']):
        raise source.JournalConflict('caller Agent packet differs from current source metadata')
    selected = {identity: _catalog_claim(root, entry, digests) for identity, entry in claims.items()
                if entry.get('predicate') == 'translated_by' and entry.get('subject_ref') == scope['expression_id']}
    agents = {scope['agent_id']: agent}
    retained = {}
    for identity, claim in selected.items():
        entry = records.get(claim.get('object'))
        if entry is None or entry.get('record_type') != 'agent':
            raise ValueError('existing translator Claim has no cataloged Agent endpoint')
        agents[entry['record_id']] = _catalog_record(root, entry, digests)
        ref = claims[identity]['source_claim_file_ref']
        if Path(ref).name == SOURCE_CLAIM_BASENAME:
            verified = verify_compound(root, ref, claim, _parent_before=before, _verify_current=False)
            if verified['parent_receipt'] not in history['receipts']:
                raise source.JournalCorruption('native responsibility is not in this Expression lineage')
            retained[verified['transaction_id']] = verified['manifest_sha256']
        elif Path(ref).name == 'responsibility-claims.jsonl':
            provenance = (Path(ref).parent / 'provenance.jsonl').as_posix()
            raw = source._read(root / provenance, source.MAX_SET_BYTES)
            events = [source._json_object(line) for line in raw.splitlines() if line.strip()]
            matches = [event for event in events if event.get('event_id') == claim.get('provenance_event_ref')]
            if (claim.get('claim_type') != 'bibliographic'
                    or claim.get('assertion_layer') not in {'bibliographic_assertion', 'scholarly_report'}
                    or len(matches) != 1 or not any(output.get('ref') == ref and output.get('sha256') == digests[ref]
                    for output in matches[0].get('outputs', []))):
                raise source.JournalCorruption('legacy responsibility stream is not bound by its exact retained batch')
            digests[provenance] = hashlib.sha256(raw).hexdigest()
        else:
            raise PermissionError('existing responsibility has no declared evidence carrier')
    validate_expression_responsibility_closure(expression, agents, selected.values())
    for ref in [*request['claim']['evidence_refs'], *request['claim'].get('counterevidence_refs', [])]:
        if ref.startswith('ToS/'):
            path = transactions._path(ref)
            if ref != scope['expression_source_path']:
                digests[ref] = hashlib.sha256(source._read(root / path, source.MAX_SET_BYTES)).hexdigest()
            # The selected parent is already bound by exact before-package and
            # pending before/after verification; it necessarily changes here.
        else:
            validate_external_citation_address(ref)
    if any(ref not in claims for ref in request['claim'].get('alternative_claim_refs', [])):
        raise ValueError('alternative responsibility Claim must already have an exact catalog identity')
    return {'catalog_and_sources': digests, 'contracts': _grammar(root, expression, agent, request['claim']),
            'implementation': {ref: hashlib.sha256(source._read(source.ROOT / ref, source.MAX_SET_BYTES)).hexdigest()
                               for ref in IMPLEMENTATIONS}, 'retained_transactions': retained}


def _transaction_id(request):
    return source._digest(source._canonical({'operation': OPERATION, 'command_id': request['command_id'],
        'owner_configuration': request['expected_configuration'], 'request_digest': source._digest(source._canonical(request))}))


def _archive_config(root, scope):
    # Read/archive storage grammar only, never an invented record.revise grant.
    return {'schema_version': source.CORPUS_SELECTED_REVISION_CONFIG, 'source_root': str(root),
            'source_path': scope['expression_source_path'], 'record_id': scope['expression_id']}


def validate_parent_receipt(receipt):
    request = receipt.get('request') if isinstance(receipt, dict) else None
    if not isinstance(request, dict):
        raise source.JournalCorruption('responsibility receipt lacks its exact original request')
    _request(request, create=True)
    publication = receipt.get('publication')
    if (not isinstance(publication, dict) or set(publication) != {'protocol', 'transaction_id', 'selected_files'}
            or publication['protocol'] != revisions.SELECTED_PROTOCOL
            or publication['transaction_id'] != _transaction_id(request)
            or publication['selected_files'] != sorted(revisions._selected_names(Path('expression.json')))
            or receipt.get('changed_fields') != ['responsibility_claim_refs']
            or request['claim'].get('predicate') != 'translated_by'
            or request['claim'].get('subject_ref') != receipt['previous_source']['id']
            or request['claim'].get('object') != request['agent'].get('record_id')
            or not isinstance(request['fields']['responsibility_claim_refs'], list)
            or not request['fields']['responsibility_claim_refs']
            or request['fields']['responsibility_claim_refs'][-1] != request['claim'].get('claim_id')):
        raise source.JournalCorruption('invalid explicit Expression responsibility lineage binding')


def _new_directories(root, scope):
    base = Path(scope['claim_source_path']).parent
    os.close(source._owned_path(root / base.parent, directory=True))
    try:
        descriptor = source._owned_path(root / base, directory=True)
    except FileNotFoundError:
        return [base.as_posix()]
    os.close(descriptor)
    raise source.JournalConflict('new responsibility Claim home is already occupied')


def _compose(root, scope, request, before, dependencies, *, recorded_at, environment):
    work_path = Path(scope['expression_source_path'])
    base = Path(scope['claim_source_path']).parent
    work = source._json_object(before[work_path.name])
    revised = {**work, **request['fields'], 'record_version': work['record_version'] + 1}
    agent, claim = request['agent'], request['claim']
    _grammar(root, revised, agent, claim)
    validate_expression_responsibility_delta(work, revised, agent, claim)
    history = revisions._history(before, work)
    if len(history['receipts']) >= revisions.MAX_REVISIONS:
        raise ValueError('parent Expression history capacity reached')
    formname = work_path.stem + '.human-forms.json'
    previous_forms = source._json_object(before[formname]) if formname in before else None
    parent_forms, parent_views, parent_refs = _forms(revised, previous_forms, request['forms'], scope['principal_id'])
    claim_forms, claim_views, claim_refs = _forms(claim, None, request['claim_forms'], scope['principal_id'], claim=True)
    identifier = _transaction_id(request)
    parent_receipt = {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'reason': request['reason'], 'previous_source': _record_ref(work), 'source': _record_ref(revised),
        'previous_revision': request['expected_revision'],
        'archive_path': revisions._archive_path({'record_id': scope['expression_id']}, request['expected_revision']).as_posix(),
        'dependencies': request['expected_dependencies'], 'changed_fields': ['responsibility_claim_refs'],
        'forms': parent_refs, 'grants_admission': False, 'request': request,
        'publication': {'protocol': revisions.SELECTED_PROTOCOL, 'transaction_id': identifier,
                        'selected_files': sorted(revisions._selected_names(work_path))}}
    validate_parent_receipt(parent_receipt)
    parent = {work_path.name: revisions._encode(revised), formname: revisions._encode(parent_forms),
        revisions.HISTORY: revisions._encode({'schema_version': 'tos_source_revision_history_v2',
            'record_id': scope['expression_id'], 'receipts': [*history['receipts'], parent_receipt]})}
    child = {SOURCE_CLAIM_BASENAME: source._canonical(claim) + b'\n',
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
        'parent_before_files': revisions._file_refs(before), 'agent': _record_ref(agent),
        'agent_source_binding': _verify_agent_binding(root, scope, agent, dependencies), 'claim': _claim_ref(claim),
        'forms': {'expression': parent_refs, 'claim': claim_refs},
        'files': revisions._file_refs(files), 'grants_admission': False}
    child[RECEIPT_FILE] = revisions._encode(receipt)
    if any(len(raw) > source.MAX_SET_BYTES for raw in [*parent.values(), *child.values()]):
        raise ValueError('compound selected metadata exceeds the per-file budget')
    return parent, child, receipt, parent_receipt, {'expression': parent_views, 'claim': claim_views}


def _authorization(scope, request, dependencies):
    return {'schema_version': AUTHORIZATION, 'scope': {key: scope[key] for key in SCOPE_KEYS},
        'principal_id': scope['principal_id'], 'maker_type': scope['maker_type'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'command_id': request['command_id'],
        'request_digest': source._digest(source._canonical(request)), 'dependency_bindings': dependencies}


def _plan(scope, authorization, before, parent, child, directories):
    work = Path(scope['expression_source_path'])
    base = Path(scope['claim_source_path']).parent
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
        raise PermissionError('selected transaction is not the native responsibility adapter')
    _validate_scope_shape(authority['scope'])
    scope = {**authority['scope'], **{key: authority[key] for key in ('principal_id', 'maker_type', 'authority_ref')}}
    work, base = Path(scope['expression_source_path']), Path(scope['claim_source_path']).parent
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
    if directories != [base.as_posix()]:
        raise PermissionError('responsibility transaction may create only its separate exact Claim home')
    parent, child, expected_receipt, parent_receipt, views = _compose(root, scope, request, before,
        authority['dependency_bindings'], recorded_at=receipt['recorded_at'], environment=environment)
    expected = _plan(scope, authority, before, parent, child, directories)
    if plan != expected or receipt != expected_receipt:
        raise source.JournalCorruption('compound retained plan does not reconstruct the exact whole before/after delta')
    archived, _ = revisions._read_archive(root, _archive_config(root, scope), parent_receipt)
    if archived != before:
        raise source.JournalCorruption('compound parent archive differs from transaction input bytes')
    return scope, request, before, parent, child, receipt, parent_receipt, views


def _verify_agent_binding(root, scope, agent, dependencies):
    """Resolve the recorded raw bytes only through the actual current lineage.

    This internal selected-source read also works under our own pending barrier;
    it never accepts a caller-selected archive or skips a transaction witness.
    """
    ref = scope['agent_source_path']
    digest = dependencies['catalog_and_sources'].get(ref)
    if not isinstance(digest, str) or not re.fullmatch(r'[a-f0-9]{64}', digest):
        raise source.JournalCorruption('Agent dependency lacks its exact original raw hash')
    files = revisions._selected_package(root / ref)
    current = source._json_object(files['agent.json'])
    if current.get('record_id') != scope['agent_id'] or current.get('record_type') != 'agent':
        raise source.JournalCorruption('responsibility Agent current typed identity changed')
    history = revisions._history(files, current)
    candidates = [files['agent.json']]
    for receipt in history['receipts']:
        request = receipt['request']
        if (request.get('schema_version') != 'tos_local_source_command_v1' or request.get('operation') != 'record.revise'
                or not isinstance(request.get('fields'), dict) or not request['fields']
                or not set(request['fields']) <= source.CORPUS_REVISION_FIELDS):
            raise source.JournalCorruption('Agent lineage contains an undeclared metadata transition')
        archived, _ = revisions._read_archive(root, {'record_id': scope['agent_id'], 'source_path': ref}, receipt)
        if 'publication' in receipt:
            inspected = transactions.inspect_transaction(root, receipt['publication']['transaction_id'])
            rows = {item['path']: item for item in inspected['plan']['files']}
            row = rows.get(ref)
            if (inspected['status'] != 'committed' or row is None or row['before'] != archived['agent.json']
                    or _record_ref(source._json_object(row['after'])) != receipt['source']):
                raise source.JournalCorruption('Agent selected revision is not a committed exact transition')
        candidates.append(archived['agent.json'])
    matched = [raw for raw in candidates if hashlib.sha256(raw).hexdigest() == digest]
    if not matched or source._canonical(source._json_object(matched[0])) != source._canonical(agent):
        raise source.JournalCorruption('Agent packet does not resolve to exact current or continuously retained source bytes')
    return {'source_path': ref, 'source': _record_ref(agent), 'source_sha256': 'sha256:' + digest,
            'source_bytes': len(matched[0])}


def verify_compound(root, claim_source_ref, claim, *, _parent_before=None, _verify_current=True):
    root = Path(root)
    snapshot = PublicationSnapshot(root) if _verify_current else None
    claim_path = transactions._path(claim_source_ref)
    receipt_raw = source._read(root / claim_path.parent / RECEIPT_FILE, source.MAX_SET_BYTES)
    receipt = source._json_object(receipt_raw)
    if receipt.get('schema_version') != RECEIPT:
        raise source.JournalCorruption('native responsibility Claim lacks its attachment receipt')
    inspected = transactions.inspect_transaction(root, receipt['transaction_id'])
    if inspected['status'] != 'committed':
        raise source.JournalCorruption('native responsibility has no committed compound publication')
    scope, request, before, parent, child, expected, parent_receipt, _ = _validate_plan(root, inspected['plan'])
    if (claim_path.as_posix() != scope['claim_source_path'] or receipt != expected or receipt_raw != child[RECEIPT_FILE]):
        raise source.JournalCorruption('native responsibility receipt or source locator differs from committed evidence')
    for name in (REQUEST_FILE, ENVIRONMENT_FILE, PROVENANCE_FILE):
        if source._read(root / claim_path.parent / name, source.MAX_SET_BYTES) != child[name]:
            raise source.JournalCorruption('native responsibility immutable capture was changed')
    import claim_revisions
    package = revisions._package(root / claim_path.parent)
    config = {'source_root': str(root), 'source_path': claim_path.as_posix(), 'claim_id': scope['claim_id']}
    initial = claim_revisions.creation_source_files(package, config)
    if (initial[SOURCE_CLAIM_BASENAME] != child[SOURCE_CLAIM_BASENAME]
            or claim_revisions._claims(package[SOURCE_CLAIM_BASENAME]) != {scope['claim_id']: claim}):
        raise source.JournalCorruption('current responsibility Claim does not retain the exact initial compound stream')
    # Corrections keep identity endpoints fixed; current profile/typed closure is
    # still checked, while old attribution wording remains in exact history.
    _grammar(root, source._json_object(parent['expression.json']), request['agent'], claim)
    validate_qualified_translator_claim(claim)
    current_parent = _parent_before if _parent_before is not None else revisions._selected_package(root / scope['expression_source_path'])
    expression = source._json_object(current_parent['expression.json'])
    if expression.get('record_id') != scope['expression_id'] or expression.get('record_type') != 'expression':
        raise source.JournalCorruption('responsibility parent current typed identity changed')
    history = revisions._history(current_parent, expression)
    if parent_receipt not in history['receipts']:
        raise source.JournalCorruption('attachment transition is absent from current Expression lineage')
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
    return {'responsibility_claim_refs': [*record['responsibility_claim_refs'], scope['claim_id']]}


def _prepared_refs(receipt):
    return {'prepared_expression': receipt['parent_after'], 'prepared_agent': receipt['agent']}


def _source_descriptors(root):
    profiles = SourceClaimProfiles(root)
    corpus = source._json_object(source._read(root / CORPUS_REF, source.MAX_SET_BYTES))
    result = {kind: {'type_id': profiles.mappings[kind], 'record_type': kind, 'schema_ref': CORPUS_REF,
        'schema_version': corpus['properties']['schema_version']['const'], 'source_basename': kind + '.json'}
        for kind in ('expression', 'agent')}
    profile = profiles.profiles['translated_by']
    route = profiles.schema_routes['translated_by', 'tos_source_relation_claim_v1']
    result['translated_by'] = {'relation_type_id': profiles.relations['translated_by']['relation_type_id'],
        'predicate': 'translated_by', 'reader': profile['reader'], 'schema_ref': route['schema_ref'],
        'schema_version': route['schema_version'], 'assertion_layers': list(profile['assertion_layers']),
        'source_basename': SOURCE_CLAIM_BASENAME}
    return result


def _result(config, configuration_digest, *, receipt=None, replayed=False, recovery=None, views=None):
    root = Path(config['source_root'])
    snapshot = PublicationSnapshot(root)
    files = revisions._selected_package(root / config['expression_source_path'])
    expression = source._json_object(files['expression.json'])
    revisions._history(files, expression)
    agent_raw = source._read(root / config['agent_source_path'], source.MAX_COMMAND_BYTES)
    agent = source._json_object(agent_raw)
    if agent.get('record_type') != 'agent' or agent.get('record_id') != config['agent_id']:
        raise source.JournalCorruption('described target is not the exact existing Agent')
    result = {'schema_version': 'tos_expression_responsibility_result_v1', 'authentication': 'local-unix-account',
        'owner_configuration': configuration_digest, 'operation': OPERATION,
        'command_operations': ['describe', PREPARE, OPERATION, RECOVERY], 'allowed_operations': config['allowed_operations'],
        'source': _record_ref(expression), 'revision': revisions._revision(files), 'publication_snapshot': snapshot.token,
        'expression_source_path': config['expression_source_path'], 'claim_source_path': config['claim_source_path'],
        'agent_record': agent, 'agent_source_binding': {'source_path': config['agent_source_path'],
            'source': _record_ref(agent), 'source_sha256': source._digest(agent_raw), 'source_bytes': len(agent_raw)},
        'source_profiles': _source_descriptors(root),
        'source_fields': [{key: value for key, value in field.items() if key not in ('pointer', 'context')}
                          for field in source.metadata_field_catalog(expression)],
        'scope': {key: config[key] for key in sorted(SCOPE_KEYS)}, 'receipt': receipt,
        'replayed': replayed, 'recovery': recovery, 'materializations': views, 'grants_admission': False}
    snapshot.verify_current()
    return result


def run_responsibility_command(owner, config, configuration_digest, path, request):
    return common.run_command(sys.modules[__name__], owner, config, configuration_digest, path, request)

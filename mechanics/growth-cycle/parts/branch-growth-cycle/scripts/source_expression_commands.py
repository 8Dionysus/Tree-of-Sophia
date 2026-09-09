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
IMPLEMENTATIONS = (MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py',
    'scripts/source_bibliographic_topology.py', 'scripts/source_metadata_snapshot.py',
    'scripts/source_witness_human_forms.py', 'scripts/source_record_profiles.py',
    'scripts/build_source_witness_catalog.py')
FORM_CONTRACTS = ('ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
                  'ToS/contracts/human-form-template.schema.json')


def _record_ref(record):
    return source.Record.from_payload(record['record_id'], record['record_version'], record).ref


def _claim_ref(claim):
    return source.Record.from_payload(claim['claim_id'], claim['claim_version'], claim).ref


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


def _selections(values, allowed):
    if not isinstance(values, list) or not 1 <= len(values) <= 32:
        raise PermissionError('compound source-copy forms require bounded explicit selections')
    selected = set()
    for item in values:
        source._keys(item, {'form_id', 'field_id'})
        if item['form_id'] not in allowed or item['form_id'] in selected or not isinstance(item['field_id'], str):
            raise PermissionError('compound source form selection exceeds its exact subject grant')
        selected.add(item['form_id'])


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


def _forms(record, previous, selections, principal, *, claim=False):
    if previous is not None:
        source._validate_history(previous)
        if {form['form_id'] for form in previous['forms']} - {item['form_id'] for item in selections}:
            raise PermissionError('compound Work update must explicitly rebind every current form')
        if any(form['content'].get('kind') != 'source-copy' for form in previous['forms']):
            raise PermissionError('authored parent forms need an explicit authored rebind owner; compound cannot convert them')
    prepare = source.prepare_claim_change if claim else source.prepare_metadata_change
    changes = [prepare(record, previous, principal, **selection) for selection in selections]
    subject = _claim_ref(record) if claim else _record_ref(record)
    value = source._apply(previous, source.Record.from_payload(subject['id'], subject['version'], record), changes)
    materialize = source.materialize_claim_forms if claim else source.materialize_metadata_forms
    views = materialize(record, value, access_allowed=True)
    if (not all(view['state'] == 'ready' for view in views)
            or not any(view['role'] == ('statement' if claim else 'name') for view in views)):
        raise ValueError('compound forms must be ready source copies with a name or qualified statement')
    return value, views, [source._form_ref(change['form']) for change in changes]


def _read_catalog(root, token):
    """Exact catalog routes only; no source tree or descendant enumeration."""
    manifest_raw = source._read(root / CATALOG_MANIFEST, source.MAX_SET_BYTES)
    manifest = source._json_object(manifest_raw)
    profiles = SourceRecordProfiles(root)
    allowed = {**RECORD_FILES, **profiles.catalog_files, **ADAPTED_RECORD_FILES}
    files = manifest.get('record_files')
    if (manifest.get('schema_version') != 'tos_source_witness_catalog_v3'
            or not isinstance(files, dict) or not files or len(files) > 128
            or manifest.get('claim_file') != 'ToS/source-witnesses/catalog/claims.jsonl'
            or any(kind not in allowed or ref != 'ToS/source-witnesses/catalog/' + allowed[kind]
                   for kind, ref in files.items())):
        raise ValueError('compound catalog has undeclared file routes')
    digests, records, claims, total = {}, {}, {}, len(manifest_raw)
    for kind, ref in [*sorted(files.items()), ('claim', manifest['claim_file'])]:
        raw = source._read(root / ref, MAX_CATALOG_BYTES)
        total += len(raw)
        if total > MAX_CATALOG_BYTES:
            raise ValueError('compound catalog exceeds its aggregate byte budget')
        digests[ref] = hashlib.sha256(raw).hexdigest()
        for line in raw.splitlines():
            if not line.strip():
                continue
            entry = source._json_object(line)
            identity = entry.get('claim_id' if kind == 'claim' else 'record_id')
            target = claims if kind == 'claim' else records
            expected_schema = ('tos_source_witness_claim_catalog_entry_v1' if kind == 'claim'
                               else 'tos_source_witness_catalog_entry_v1')
            if (entry.get('schema_version') != expected_schema
                    or not isinstance(identity, str) or identity in records or identity in claims
                    or kind != 'claim' and entry.get('record_type') != kind
                    or len(records) + len(claims) >= MAX_CATALOG_ROWS):
                raise ValueError('compound catalog identities are invalid, duplicated or over budget')
            target[identity] = entry
    verify_catalog_publication(manifest, token, digests)
    digests[CATALOG_MANIFEST] = hashlib.sha256(manifest_raw).hexdigest()
    return records, claims, digests


def _catalog_record(root, entry, digests):
    ref = entry.get('source_record_ref')
    path = transactions._path(ref)
    if path.name != entry['record_type'] + '.json':
        raise ValueError('selected corpus metadata path differs from its typed catalog')
    raw = source._read(root / path, source.MAX_COMMAND_BYTES)
    value = source._json_object(raw)
    if (value.get('record_id') != entry['record_id'] or value.get('record_type') != entry['record_type']
            or source._digest(source._canonical(value))[7:] != entry.get('record_sha256')):
        raise source.JournalConflict('selected source record differs from its catalog locator/digest')
    digests[ref] = hashlib.sha256(raw).hexdigest()
    return value


def _catalog_claim(root, entry, digests):
    ref = entry.get('source_claim_file_ref')
    path = transactions._path(ref)
    raw = source._read(root / path, source.MAX_SET_BYTES)
    number = entry.get('source_claim_line')
    lines = raw.splitlines()
    if type(number) is not int or not 1 <= number <= len(lines):
        raise ValueError('selected topology Claim has an invalid source line')
    claim = source._json_object(lines[number - 1])
    if (claim.get('claim_id') != entry['claim_id']
            or source._digest(source._canonical(claim))[7:] != entry.get('claim_sha256')
            or any(claim.get(field) != entry.get(field) for field in ('subject_ref', 'object', 'predicate'))):
        raise source.JournalConflict('selected topology Claim differs from its catalog locator/digest')
    digests[ref] = hashlib.sha256(raw).hexdigest()
    return claim


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


def _environment():
    with Path(sys.executable).resolve().open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    return {'runtime': platform.python_implementation(), 'runtime_version': platform.python_version(),
        'runtime_artifact_sha256': digest, 'backend': 'python-standard-library-and-jsonschema',
        'hardware_target': 'cpu', 'unicode_version': unicodedata.unidata_version,
        'argv_sha256': source._digest(source._canonical(sys.argv))[7:]}


def _event(scope, request, before, outputs, environment, dependencies, recorded_at):
    """A reconstructible buffer-serialization event, not a commit attestation."""
    base = Path(scope['expression_source_path']).parent
    request_ref = (base / REQUEST_FILE).as_posix()
    request_raw = source._canonical(request) + b'\n'
    environment_raw = source._canonical(environment) + b'\n'
    archive = revisions._archive_path({'record_id': scope['work_id']}, request['expected_revision'])
    def entity(ref, raw, role):
        return {'entity_ref': ref, 'role': role, 'sha256': source._digest(raw)[7:], 'size_bytes': len(raw),
            'media_type': 'application/x-ndjson' if ref.endswith('.jsonl') else 'application/json',
            'availability': 'owner_local', 'content_disclosure': 'public_metadata_only',
            'fixity_verified': False, 'fixity_verified_at': None}
    prior = {str(archive / (source._digest(raw)[7:] + '.blob')): raw for raw in before.values()}
    environment_ref = (base / ENVIRONMENT_FILE).as_posix()
    script_digest = dependencies['implementation'][MODULE_REF]
    return {
        '$schema': 'https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json',
        'schema_version': 'tos_provenance_event_v2', 'event_id': scope['provenance_event_id'],
        'event_version': 1, 'supersedes_event_ref': None,
        'record_binding': {'manifest_ref': (base / RECEIPT_FILE).as_posix(),
            'digest_algorithm': 'sha256', 'digest_scope': 'exact_event_record_bytes'},
        'activity': {'event_type': 'annotation', 'started_at': recorded_at, 'ended_at': recorded_at,
            'status': 'completed_with_warnings', 'terminal_reason': None, 'exit_code': 0,
            'warnings': ['Captured prepared metadata buffers; the committed transaction is a separate verification.',
                         'Observed denotes the declared record link, not accepted bibliographic or textual truth.']},
        'entities': {'inputs': [entity(request_ref, request_raw, 'caller-supplied-metadata-request'),
                               *(entity(ref, raw, 'retained-parent-metadata-input') for ref, raw in sorted(prior.items()))],
            'outputs': [entity(ref, raw, 'prepared-compound-source-metadata') for ref, raw in sorted(outputs.items())],
            'byproducts': [entity(environment_ref, environment_raw, 'runtime-description')]},
        'derivations': [{'derivation_id': scope['provenance_event_id'].replace('tos.event.', 'tos.derivation.', 1) + f'.output-{index}',
            'input_entity_ref': request_ref, 'output_entity_ref': ref, 'relation': 'was_derived_from',
            'influence_asserted': True,
            'description': 'Technical source metadata serialization; no historical influence or textual identity is asserted.'}
            for index, ref in enumerate(sorted(outputs))],
        'responsibility': [{'agent_ref': 'software:tos-source-expression-commands', 'agent_kind': 'software',
            'role': 'executor', 'responsibility_posture': 'performed',
            'evidence_binding': {'ref': MODULE_REF, 'sha256': script_digest}, 'human_evidence_status': 'not_applicable'}],
        'method': {'procedure': {'name': 'native-work-expression-metadata-serialization', 'version': '1',
            'purpose': 'Serialize one declared parent link and explicit source-copy forms without judging their content.'},
            'command_capture': {'disclosure': 'withheld_digest_only', 'argv': None,
                'argv_sha256': environment['argv_sha256'],
                'withholding_reason': 'Process arguments may contain a private owner-configuration path.'},
            'configuration_binding': {'ref': request_ref, 'sha256': source._digest(request_raw)[7:]},
            'software_components': [{'name': 'ToS native Work Expression adapter', 'version': '1',
                'role': 'serialization-runner', 'artifact_ref': MODULE_REF, 'artifact_sha256': script_digest,
                'verification_status': 'verified'}], 'model_invocations': [],
            'environment': {**{key: value for key, value in environment.items() if key != 'argv_sha256'},
                'environment_profile_binding': {'ref': environment_ref, 'sha256': source._digest(environment_raw)[7:]}}},
        'manual_changes': {'status': 'none_declared', 'change_receipts': [],
            'statement': 'Caller authorship precedes this operation; no manual edits are performed inside serialization.'},
        'measurements': [{'metric': 'output_bytes', 'status': 'measured', 'value': sum(map(len, outputs.values())),
            'unit': 'bytes', 'method': 'Sum of prepared source record, form and parent history buffers; excludes capture and receipt.',
            'evidence_binding': None}],
        'evidence_authentication': {'capture_posture': 'tool_captured', 'signature_status': 'unsigned',
            'signature_bindings': [], 'verification_status': 'unverified',
            'producer_control_boundary': 'The same unsigned local process serializes and records; hashes do not authenticate execution truth.'},
        'rights_and_visibility': {'rights_record_bindings': [], 'intended_uses': ['local_research', 'public_metadata'],
            'content_visibility': 'tracked_public_metadata', 'publication_authorized': False, 'publication_authority_bindings': []},
        'review_and_authority': {'mechanical_validation': 'not_run', 'human_review_status': 'not_performed',
            'review_bindings': [], 'accepted_uses': [], 'promotion_authorized': False, 'competence_evidence_bindings': []},
        'reproducibility': {'classification': 'partially_specified',
            'known_gaps': ['Upstream research, source reading and model invocations are outside this operation.',
                           'Runtime metadata is captured, not a complete archived execution environment.'],
            'replay_scope': 'Exact retained request, metadata and source-copy buffer construction; not bibliographic truth.'},
        'authority_boundary': {'validator_role': 'mechanics_and_closure_only_not_truth',
            'claims_not_established': ['execution_truth', 'content_truth', 'source_fidelity', 'translation_quality',
                'semantic_correctness', 'rights_clearance', 'human_review', 'publication_authority', 'canon_authority']}}


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


def _check_dependencies(root, bindings):
    source._keys(bindings, {'catalog_and_sources', 'contracts', 'implementation', 'retained_transactions'})
    if set(bindings['implementation']) != set(IMPLEMENTATIONS):
        raise source.JournalCorruption('compound implementation closure is not explicit')
    total, count = 0, 0
    for group in ('catalog_and_sources', 'contracts', 'implementation'):
        values = bindings[group]
        if not isinstance(values, dict):
            raise source.JournalCorruption('compound dependency bindings must be objects')
        for ref, digest in values.items():
            count += 1
            if (not isinstance(ref, str) or not isinstance(digest, str)
                    or not re.fullmatch(r'[a-f0-9]{64}', digest) or count > 512):
                raise source.JournalCorruption('compound dependency binding is malformed or over budget')
            path = Path(ref)
            if group == 'catalog_and_sources':
                if path.parent == Path('ToS/source-witnesses/catalog'):
                    if not re.fullmatch(r'[a-z][a-z0-9.-]*\.jsonl?', path.name):
                        raise PermissionError('invalid catalog dependency path')
                else:
                    transactions._path(ref)
            elif group == 'contracts':
                if (path.is_absolute() or path.as_posix() != ref or any(part.startswith('.') for part in path.parts)
                        or '\\' in ref or '\x00' in ref
                        or not (path.is_relative_to('ToS/contracts') or path.is_relative_to('ToS/doctrine'))):
                    raise PermissionError('compound grammar path leaves the declared contract district')
            origin = source.ROOT if group == 'implementation' else root
            raw = source._read(origin / ref, MAX_CATALOG_BYTES if group == 'catalog_and_sources' else source.MAX_SET_BYTES)
            total += len(raw)
            if total > 32 * 1024 * 1024 or hashlib.sha256(raw).hexdigest() != digest:
                raise source.JournalConflict('compound source or grammar dependencies changed')
    if not isinstance(bindings['retained_transactions'], dict) or len(bindings['retained_transactions']) > 128:
        raise source.JournalCorruption('compound retained dependency count exceeds its bound')
    for identifier, digest in bindings['retained_transactions'].items():
        result = transactions.inspect_transaction(root, identifier)
        if result['status'] != 'committed' or result['manifest_sha256'] != digest:
            raise source.JournalConflict('a prior native topology dependency is no longer committed exact evidence')


def _read_owner(owner):
    return configuration(source._json_object(source._read(owner, source.MAX_COMMAND_BYTES)), owner_config=owner)


def _guard(owner, config, configuration_digest, scope, request, before, authority, *, recovery):
    root = Path(config['source_root'])
    def guard(retained, _summary):
        current, digest, _ = _read_owner(owner)
        if current != config or digest != configuration_digest or retained != authority:
            raise source.JournalConflict('compound source authority changed during publication')
        if any(current[key] != scope[key] for key in SCOPE_KEYS if not key.startswith('allowed_')):
            raise PermissionError('current recovery scope names a different compound destination')
        _scope(current, request, recovery=recovery, original=scope)
        _check_dependencies(root, authority['dependency_bindings'])
        return True
    return guard


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
    """Dispatch this exact grant before the generic non-pending read wrapper."""
    root = Path(config['source_root'])
    operation = request.get('operation')
    if operation in {'prepare-create', OPERATION}:
        _request(request, create=operation == OPERATION)
        _scope(config, request)
    elif operation == RECOVERY:
        source._keys(request, {'schema_version', 'operation', 'transaction_id', 'decision', 'expected_configuration'})
        if (request['schema_version'] != REQUEST or request['decision'] not in {'resume', 'rollback'}
                or request['expected_configuration'] != configuration_digest or RECOVERY not in config['allowed_operations']):
            raise PermissionError('compound recovery requires its current exact delegation and explicit decision')
    elif operation == 'describe':
        source._keys(request, {'schema_version', 'operation'})
        if request['schema_version'] != REQUEST:
            raise ValueError('unknown compound source command version')
        return _result(config, configuration_digest)
    else:
        raise ValueError('unknown native Work Expression operation')

    if operation == 'prepare-create':
        snapshot = PublicationSnapshot(root)
        before = revisions._selected_package(path)
        work = source._json_object(before[path.name])
        _new_directories(root, config)
        proposal = {**request, 'operation': OPERATION, 'command_id': 'preview:uncommitted',
            'fields': {'expression_claim_refs': [*work['expression_claim_refs'], config['claim_id']]},
            'expected_configuration': configuration_digest, 'expected_source': _record_ref(work),
            'expected_revision': revisions._revision(before), 'expected_publication': snapshot.token}
        dependencies = _context(root, config, proposal, before)
        proposal['expected_dependencies'] = source._digest(source._canonical(dependencies))
        _, _, receipt, _, views = _compose(root, config, proposal, before, dependencies,
            recorded_at=datetime.now(timezone.utc).isoformat(), environment=_environment())
        result = {**_result(config, configuration_digest), 'prepared_fields': proposal['fields'],
            'prepared_work': receipt['parent_after'], 'prepared_expression': receipt['expression'],
            'prepared_claim': receipt['claim'], 'prepared_forms': receipt['forms'],
            'prepared_materializations': views, 'expected_dependencies': proposal['expected_dependencies'],
            'expected_publication': snapshot.token}
        # The result reader also reads current metadata. Keep the original
        # preparation snapshot authoritative through that complete assembly.
        snapshot.verify_current()
        return result

    with source._locked(root / 'ToS/source-witnesses/historical-create', allow_pending=True):
        current, digest, current_path = _read_owner(owner)
        if current != config or digest != configuration_digest or current_path != path:
            raise source.JournalConflict('compound delegation changed before publication')
        pending = transactions.read_pending_transaction(root)
        if pending is not None:
            scope, original, before, _, _, receipt, _, views = _validate_plan(root, pending['plan'])
            recovery = operation == RECOVERY
            if not recovery and (request != original or request['expected_configuration'] != configuration_digest):
                raise source.JournalConflict('only the exact original command may resume without a recovery decision')
            if any(config[key] != scope[key] for key in SCOPE_KEYS if not key.startswith('allowed_')):
                raise PermissionError('pending compound publication is outside this owner scope')
            _scope(config, original, recovery=recovery, original=scope)
            dependencies = _context(root, scope, original, before)
            if dependencies != pending['plan']['authorization']['dependency_bindings']:
                raise source.JournalConflict('current compound dependencies differ from the exact retained request')
            identifier = receipt['transaction_id']
            if recovery and request['transaction_id'] != identifier:
                raise source.JournalConflict('recovery selects another pending transaction')
            decision = request['decision'] if recovery else 'resume'
            guard = _guard(owner, config, configuration_digest, scope, original, before,
                           pending['plan']['authorization'], recovery=recovery)
            renewal = {'schema_version': 'tos_work_expression_recovery_authorization_v1',
                'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
                'owner_configuration': configuration_digest, 'transaction_id': identifier,
                'decision': decision} if recovery else None
            action = transactions.resume_transaction if decision == 'resume' else transactions.rollback_transaction
            completed = action(root, authorization_guard=guard, transaction_id=identifier,
                               **({'recovery_authorization': renewal} if recovery else {}))
            return _result(config, configuration_digest, receipt=receipt if decision == 'resume' else None,
                           recovery=completed, views=views if decision == 'resume' else None)
        if operation == RECOVERY:
            raise source.JournalConflict('no exact pending compound transaction is selected for recovery')

        snapshot = PublicationSnapshot(root)
        child_receipt = root / Path(config['expression_source_path']).parent / RECEIPT_FILE
        try:
            existing = source._json_object(source._read(child_receipt, source.MAX_SET_BYTES))
        except FileNotFoundError:
            existing = None
        if existing is not None:
            verified = verify_compound(root, Path(config['expression_source_path']).with_name(SOURCE_CLAIM_BASENAME).as_posix(),
                                       request['claim'])
            if (existing['request_digest'] != source._digest(source._canonical(request))
                    or request['expected_configuration'] != configuration_digest):
                raise source.JournalConflict('compound target or command identity is already occupied')
            # This is an observation of an already committed exact operation,
            # not a new mutation. Catalog rebuilds and later sibling growth may
            # change its former preparation inputs; retained plan and continuous
            # current lineage above own historical verification instead.
            snapshot.verify_current()
            return _result(config, configuration_digest, receipt=existing, replayed=True)
        before = revisions._selected_package(path)
        work = source._json_object(before[path.name])
        if (request['expected_configuration'] != configuration_digest or request['expected_source'] != _record_ref(work)
                or request['expected_revision'] != revisions._revision(before)
                or request['expected_publication'] != snapshot.token):
            raise source.JournalConflict('compound parent, revision, authority or publication snapshot is stale')
        directories = _new_directories(root, config)
        dependencies = _context(root, config, request, before)
        if source._digest(source._canonical(dependencies)) != request['expected_dependencies']:
            raise source.JournalConflict('compound preparation dependencies are stale')
        parent, child, receipt, _, views = _compose(root, config, request, before, dependencies,
            recorded_at=datetime.now(timezone.utc).isoformat(), environment=_environment())
        revisions._archive(root, _archive_config(root, config), before,
            source.Record.from_payload(work['record_id'], work['record_version'], work), request['expected_revision'])
        authority = _authorization(config, request, dependencies)
        plan = _plan(config, authority, before, parent, child, directories)
        _validate_plan(root, plan)
        guard = _guard(owner, config, configuration_digest, config, request, before, authority, recovery=False)
        transactions.apply_transaction(root, plan, expected_snapshot=snapshot, authorization_guard=guard,
                                       transaction_id=receipt['transaction_id'])
        return _result(config, configuration_digest, receipt=receipt, views=views)

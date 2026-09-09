"""Shared bounded mechanics for separately owned compound source operations.

Adapters retain scope, typed delta, reconstruction and current-lineage judgment.
This module supplies identical read/guard/publication mechanics, never grants.
"""
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
from source_record_profiles import SourceRecordProfiles
from build_source_witness_catalog import RECORD_FILES, ADAPTED_RECORD_FILES, verify_catalog_publication

MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_compound_commands.py'
CATALOG_MANIFEST = 'ToS/source-witnesses/catalog/catalog.manifest.json'
MAX_CATALOG_BYTES = 16 * 1024 * 1024
MAX_CATALOG_ROWS = 8192


def _record_ref(record):
    return source.Record.from_payload(record['record_id'], record['record_version'], record).ref


def _claim_ref(claim):
    return source.Record.from_payload(claim['claim_id'], claim['claim_version'], claim).ref


def _selections(values, allowed):
    if not isinstance(values, list) or not 1 <= len(values) <= 32:
        raise PermissionError('compound source-copy forms require bounded explicit selections')
    selected = set()
    for item in values:
        source._keys(item, {'form_id', 'field_id'})
        if item['form_id'] not in allowed or item['form_id'] in selected or not isinstance(item['field_id'], str):
            raise PermissionError('compound source form selection exceeds its exact subject grant')
        selected.add(item['form_id'])


def _forms(record, previous, selections, principal, *, claim=False):
    if previous is not None:
        source._validate_history(previous)
        if {form['form_id'] for form in previous['forms']} - {item['form_id'] for item in selections}:
            raise PermissionError('compound parent update must explicitly rebind every current form')
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


def _environment():
    with Path(sys.executable).resolve().open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    return {'runtime': platform.python_implementation(), 'runtime_version': platform.python_version(),
        'runtime_artifact_sha256': digest, 'backend': 'python-standard-library-and-jsonschema',
        'hardware_target': 'cpu', 'unicode_version': unicodedata.unidata_version,
        'argv_sha256': source._digest(source._canonical(sys.argv))[7:]}


def _check_dependencies(adapter, root, bindings):
    source._keys(bindings, {'catalog_and_sources', 'contracts', 'implementation', 'retained_transactions'})
    if set(bindings['implementation']) != set(adapter.IMPLEMENTATIONS):
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


def _guard(adapter, owner, config, configuration_digest, scope, request, before, authority, *, recovery):
    root = Path(config['source_root'])
    def guard(retained, _summary):
        current, digest, _ = adapter._read_owner(owner)
        if current != config or digest != configuration_digest or retained != authority:
            raise source.JournalConflict('compound source authority changed during publication')
        if any(current[key] != scope[key] for key in adapter.SCOPE_KEYS if not key.startswith('allowed_')):
            raise PermissionError('current recovery scope names a different compound destination')
        adapter._scope(current, request, recovery=recovery, original=scope)
        adapter._check_dependencies(root, authority['dependency_bindings'])
        return True
    return guard


def run_command(adapter, owner, config, configuration_digest, path, request):
    """Dispatch this exact grant before the generic non-pending read wrapper."""
    root = Path(config['source_root'])
    operation = request.get('operation')
    if operation in {adapter.PREPARE, adapter.OPERATION}:
        adapter._request(request, create=operation == adapter.OPERATION)
        adapter._scope(config, request)
    elif operation == adapter.RECOVERY:
        source._keys(request, {'schema_version', 'operation', 'transaction_id', 'decision', 'expected_configuration'})
        if (request['schema_version'] != adapter.REQUEST or request['decision'] not in {'resume', 'rollback'}
                or request['expected_configuration'] != configuration_digest or adapter.RECOVERY not in config['allowed_operations']):
            raise PermissionError('compound recovery requires its current exact delegation and explicit decision')
    elif operation == 'describe':
        source._keys(request, {'schema_version', 'operation'})
        if request['schema_version'] != adapter.REQUEST:
            raise ValueError('unknown compound source command version')
        return adapter._result(config, configuration_digest)
    else:
        raise ValueError('unknown native compound operation')

    if operation == adapter.PREPARE:
        snapshot = PublicationSnapshot(root)
        before = revisions._selected_package(path)
        work = source._json_object(before[path.name])
        adapter._new_directories(root, config)
        proposal = {**request, 'operation': adapter.OPERATION, 'command_id': 'preview:uncommitted',
            'fields': adapter._prepare_fields(work, config),
            'expected_configuration': configuration_digest, 'expected_source': _record_ref(work),
            'expected_revision': revisions._revision(before), 'expected_publication': snapshot.token}
        dependencies = adapter._context(root, config, proposal, before)
        proposal['expected_dependencies'] = source._digest(source._canonical(dependencies))
        _, _, receipt, _, views = adapter._compose(root, config, proposal, before, dependencies,
            recorded_at=datetime.now(timezone.utc).isoformat(), environment=_environment())
        result = {**adapter._result(config, configuration_digest), 'prepared_fields': proposal['fields'],
            **adapter._prepared_refs(receipt),
            'prepared_claim': receipt['claim'], 'prepared_forms': receipt['forms'],
            'prepared_materializations': views, 'expected_dependencies': proposal['expected_dependencies'],
            'expected_publication': snapshot.token}
        # The result reader also reads current metadata. Keep the original
        # preparation snapshot authoritative through that complete assembly.
        snapshot.verify_current()
        return result

    with source._locked(root / 'ToS/source-witnesses/historical-create', allow_pending=True):
        current, digest, current_path = adapter._read_owner(owner)
        if current != config or digest != configuration_digest or current_path != path:
            raise source.JournalConflict('compound delegation changed before publication')
        pending = transactions.read_pending_transaction(root)
        if pending is not None:
            scope, original, before, _, _, receipt, _, views = adapter._validate_plan(root, pending['plan'])
            recovery = operation == adapter.RECOVERY
            if not recovery and (request != original or request['expected_configuration'] != configuration_digest):
                raise source.JournalConflict('only the exact original command may resume without a recovery decision')
            if any(config[key] != scope[key] for key in adapter.SCOPE_KEYS if not key.startswith('allowed_')):
                raise PermissionError('pending compound publication is outside this owner scope')
            adapter._scope(config, original, recovery=recovery, original=scope)
            dependencies = adapter._context(root, scope, original, before)
            if dependencies != pending['plan']['authorization']['dependency_bindings']:
                raise source.JournalConflict('current compound dependencies differ from the exact retained request')
            identifier = receipt['transaction_id']
            if recovery and request['transaction_id'] != identifier:
                raise source.JournalConflict('recovery selects another pending transaction')
            decision = request['decision'] if recovery else 'resume'
            guard = adapter._guard(owner, config, configuration_digest, scope, original, before,
                           pending['plan']['authorization'], recovery=recovery)
            renewal = {'schema_version': adapter.RECOVERY_AUTHORIZATION,
                'principal_id': config['principal_id'], 'authority_ref': config['authority_ref'],
                'owner_configuration': configuration_digest, 'transaction_id': identifier,
                'decision': decision} if recovery else None
            action = transactions.resume_transaction if decision == 'resume' else transactions.rollback_transaction
            completed = action(root, authorization_guard=guard, transaction_id=identifier,
                               **({'recovery_authorization': renewal} if recovery else {}))
            return adapter._result(config, configuration_digest, receipt=receipt if decision == 'resume' else None,
                           recovery=completed, views=views if decision == 'resume' else None)
        if operation == adapter.RECOVERY:
            raise source.JournalConflict('no exact pending compound transaction is selected for recovery')

        snapshot = PublicationSnapshot(root)
        child_receipt = root / Path(adapter._claim_source_ref(config)).parent / adapter.RECEIPT_FILE
        try:
            existing = source._json_object(source._read(child_receipt, source.MAX_SET_BYTES))
        except FileNotFoundError:
            existing = None
        if existing is not None:
            adapter.verify_replay(root, config, request)
            if (existing['request_digest'] != source._digest(source._canonical(request))
                    or request['expected_configuration'] != configuration_digest):
                raise source.JournalConflict('compound target or command identity is already occupied')
            # This is an observation of an already committed exact operation,
            # not a new mutation. Catalog rebuilds and later sibling growth may
            # change its former preparation inputs; retained plan and continuous
            # current lineage above own historical verification instead.
            snapshot.verify_current()
            return adapter._result(config, configuration_digest, receipt=existing, replayed=True)
        before = revisions._selected_package(path)
        work = source._json_object(before[path.name])
        if (request['expected_configuration'] != configuration_digest or request['expected_source'] != _record_ref(work)
                or request['expected_revision'] != revisions._revision(before)
                or request['expected_publication'] != snapshot.token):
            raise source.JournalConflict('compound parent, revision, authority or publication snapshot is stale')
        directories = adapter._new_directories(root, config)
        dependencies = adapter._context(root, config, request, before)
        if source._digest(source._canonical(dependencies)) != request['expected_dependencies']:
            raise source.JournalConflict('compound preparation dependencies are stale')
        parent, child, receipt, _, views = adapter._compose(root, config, request, before, dependencies,
            recorded_at=datetime.now(timezone.utc).isoformat(), environment=_environment())
        revisions._archive(root, adapter._archive_config(root, config), before,
            source.Record.from_payload(work['record_id'], work['record_version'], work), request['expected_revision'])
        authority = adapter._authorization(config, request, dependencies)
        plan = adapter._plan(config, authority, before, parent, child, directories)
        adapter._validate_plan(root, plan)
        guard = adapter._guard(owner, config, configuration_digest, config, request, before, authority, recovery=False)
        transactions.apply_transaction(root, plan, expected_snapshot=snapshot, authorization_guard=guard,
                                       transaction_id=receipt['transaction_id'])
        return adapter._result(config, configuration_digest, receipt=receipt, views=views)


def _event(adapter, scope, request, before, outputs, environment, dependencies, recorded_at):
    """A reconstructible buffer-serialization event, not a commit attestation."""
    base = Path(adapter._claim_source_ref(scope)).parent
    request_ref = (base / adapter.REQUEST_FILE).as_posix()
    request_raw = source._canonical(request) + b'\n'
    environment_raw = source._canonical(environment) + b'\n'
    archive = revisions._archive_path({'record_id': scope[adapter.PARENT_ID]}, request['expected_revision'])
    def entity(ref, raw, role):
        return {'entity_ref': ref, 'role': role, 'sha256': source._digest(raw)[7:], 'size_bytes': len(raw),
            'media_type': 'application/x-ndjson' if ref.endswith('.jsonl') else 'application/json',
            'availability': 'owner_local', 'content_disclosure': 'public_metadata_only',
            'fixity_verified': False, 'fixity_verified_at': None}
    prior = {str(archive / (source._digest(raw)[7:] + '.blob')): raw for raw in before.values()}
    environment_ref = (base / adapter.ENVIRONMENT_FILE).as_posix()
    script_digest = dependencies['implementation'][adapter.MODULE_REF]
    return {
        '$schema': 'https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json',
        'schema_version': 'tos_provenance_event_v2', 'event_id': scope['provenance_event_id'],
        'event_version': 1, 'supersedes_event_ref': None,
        'record_binding': {'manifest_ref': (base / adapter.RECEIPT_FILE).as_posix(),
            'digest_algorithm': 'sha256', 'digest_scope': 'exact_event_record_bytes'},
        'activity': {'event_type': 'annotation', 'started_at': recorded_at, 'ended_at': recorded_at,
            'status': 'completed_with_warnings', 'terminal_reason': None, 'exit_code': 0,
            'warnings': ['Captured prepared metadata buffers; the committed transaction is a separate verification.',
                         adapter.EVENT_PROFILE['warning']]},
        'entities': {'inputs': [entity(request_ref, request_raw, 'caller-supplied-metadata-request'),
                               *(entity(ref, raw, 'retained-parent-metadata-input') for ref, raw in sorted(prior.items()))],
            'outputs': [entity(ref, raw, 'prepared-compound-source-metadata') for ref, raw in sorted(outputs.items())],
            'byproducts': [entity(environment_ref, environment_raw, 'runtime-description')]},
        'derivations': [{'derivation_id': scope['provenance_event_id'].replace('tos.event.', 'tos.derivation.', 1) + f'.output-{index}',
            'input_entity_ref': request_ref, 'output_entity_ref': ref, 'relation': 'was_derived_from',
            'influence_asserted': True,
            'description': 'Technical source metadata serialization; no historical influence or textual identity is asserted.'}
            for index, ref in enumerate(sorted(outputs))],
        'responsibility': [{'agent_ref': adapter.EVENT_PROFILE['executor'], 'agent_kind': 'software',
            'role': 'executor', 'responsibility_posture': 'performed',
            'evidence_binding': {'ref': adapter.MODULE_REF, 'sha256': script_digest}, 'human_evidence_status': 'not_applicable'}],
        'method': {'procedure': {'name': adapter.EVENT_PROFILE['procedure'], 'version': '1',
            'purpose': adapter.EVENT_PROFILE['purpose']},
            'command_capture': {'disclosure': 'withheld_digest_only', 'argv': None,
                'argv_sha256': environment['argv_sha256'],
                'withholding_reason': 'Process arguments may contain a private owner-configuration path.'},
            'configuration_binding': {'ref': request_ref, 'sha256': source._digest(request_raw)[7:]},
            'software_components': [{'name': adapter.EVENT_PROFILE['component'], 'version': '1',
                'role': 'serialization-runner', 'artifact_ref': adapter.MODULE_REF, 'artifact_sha256': script_digest,
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

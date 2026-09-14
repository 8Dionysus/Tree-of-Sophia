"""Separately delegated native Edition -> Item growth and public evidence.

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
import source_item_deposit as deposit
from build_source_resource_inventories import AUTHORITY_BOUNDARY as INVENTORY_BOUNDARY, SCHEMA_REF as INVENTORY_SCHEMA
from source_metadata_snapshot import PublicationSnapshot
from source_bibliographic_topology import validate_edition_item_delta
from source_record_profiles import SourceClaimProfiles, SOURCE_CLAIM_BASENAME, CORPUS_REF

CONFIG = 'tos_local_item_adoption_owner_v1'
REQUEST = 'tos_local_item_adoption_command_v1'
OPERATION = 'item.adopt'
RECOVERY = 'item.adoption.recover'
AUTHORIZATION = 'tos_item_adoption_authorization_v1'
RECEIPT = 'tos_edition_item_receipt_v1'
RECEIPT_FILE = 'edition-item-receipt.json'
REQUEST_FILE = 'source-create-request.json'
ENVIRONMENT_FILE = 'source-create-environment.json'
PROVENANCE_FILE = 'source-create-provenance.jsonl'
BYTE_RECEIPT_FILE = 'item-deposit-receipt.json'
MODULE_REF = 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_item_commands.py'
CATALOG_MANIFEST = 'ToS/source-witnesses/catalog/catalog.manifest.json'
MAX_CATALOG_BYTES = 16 * 1024 * 1024
MAX_CATALOG_ROWS = 8192
HASH = re.compile(r'sha256:[a-f0-9]{64}')
SCOPE_KEYS = {'edition_id', 'edition_source_path', 'item_id', 'item_source_path', 'claim_id',
    'provenance_event_id', 'allowed_edition_form_ids', 'allowed_item_form_ids', 'allowed_claim_form_ids',
    'file_id', 'payload_basename', 'original_basename', 'media_type', 'byte_size', 'sha256',
    'rights_id', 'acquisition_event_id', 'inventory_event_id'}
CONFIG_KEYS = SCOPE_KEYS | {'schema_version', 'uid', 'principal_id', 'maker_type', 'source_root',
                          'authority_ref', 'expires_at', 'allowed_operations'} | deposit.PRIVATE_KEYS
PROPOSAL_KEYS = {'record', 'claim', 'forms', 'item_forms', 'claim_forms', 'reason', 'rights', 'item_kind'}
CREATE_KEYS = PROPOSAL_KEYS | {'schema_version', 'operation', 'command_id', 'fields',
    'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies', 'expected_publication', 'inventory', 'inventory_limitation', 'fixity_verified_at'}
IMPLEMENTATIONS = (MODULE_REF, common.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py', contract.MODULE_REF,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/assessment_journal.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_edition_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_responsibility_commands.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/metadata_version_reader.py',
    'scripts/source_bibliographic_topology.py', 'scripts/source_metadata_snapshot.py',
    'scripts/source_witness_human_forms.py', 'scripts/source_record_profiles.py',
    'scripts/build_source_witness_catalog.py', 'scripts/build_source_resource_inventories.py',
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_item_deposit.py')
PREPARE = 'prepare-create'
RECOVERY_AUTHORIZATION = 'tos_edition_item_recovery_authorization_v1'
PARENT_ID = 'edition_id'
EVENT_PROFILE = {
    'warning': 'Observed denotes the declared record link, not accepted bibliographic or textual truth.',
    'executor': 'software:tos-source-item-commands',
    'procedure': 'native-item-adoption-metadata-serialization',
    'purpose': 'Serialize one declared parent link and explicit source-copy forms without judging their content.',
    'component': 'ToS native Edition Item adapter',
}
FORM_CONTRACTS = ('ToS/contracts/human-form.schema.json', 'ToS/contracts/human-form-set.schema.json',
                  'ToS/contracts/human-form-template.schema.json')

def _validate_scope_shape(scope):
    source._keys(scope, SCOPE_KEYS)
    for key, prefix in (('edition_id', 'edition'), ('item_id', 'item'), ('file_id', 'file'),
            ('claim_id', 'claim'), ('provenance_event_id', 'event'), ('rights_id', 'rights'),
            ('acquisition_event_id', 'event'), ('inventory_event_id', 'event')):
        if not isinstance(scope[key], str) or not re.fullmatch(r'tos\.' + prefix + r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*', scope[key]):
            raise PermissionError('Item adoption requires exact typed identities')
    if len({scope[key] for key in ('provenance_event_id', 'acquisition_event_id', 'inventory_event_id')}) != 3:
        raise PermissionError('copy, enumeration and metadata serialization are separate events')
    edition, item = (transactions._path(scope[key]) for key in ('edition_source_path', 'item_source_path'))
    if (edition.name != 'edition.json' or 'editions' not in edition.parts or item.name != 'item.json'
            or item.parent.parent != edition.parent / 'items'
            or not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*', item.parent.name)):
        raise PermissionError('Item must occupy the exact new child of the existing Edition; no Work ladder is inferred')
    if (not isinstance(scope['payload_basename'], str)
            or not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9._-]{0,200}', scope['payload_basename'])
            or not isinstance(scope['original_basename'], str) or not 1 <= len(scope['original_basename']) <= 256
            or any(char in scope['original_basename'] for char in ('/', '\\', '\n', '\r', '\x00'))
            or not isinstance(scope['media_type'], str) or not re.fullmatch(r'[a-z0-9.+-]+/[a-z0-9.+-]+', scope['media_type'])
            or not isinstance(scope['sha256'], str) or not re.fullmatch(r'[a-f0-9]{64}', scope['sha256'])
            or type(scope['byte_size']) is not int or not 1 <= scope['byte_size'] <= deposit.MAX_BYTES):
        raise PermissionError('one bounded File identity, safe destination and exact fixity are required')
    seen = set()
    for name in ('allowed_edition_form_ids', 'allowed_item_form_ids', 'allowed_claim_form_ids'):
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
    deposit.validate_config(config)
    root = Path(config['source_root'])
    os.close(source._owned_path(root, directory=True))
    return config, source._digest(source._canonical(config)), root / config['edition_source_path']

def _request(request, *, create=False):
    source.command_handler(CONFIG).validate_request(request)
    if (request['schema_version'] != REQUEST or request['operation'] != (OPERATION if create else 'prepare-create')
            or len(source._canonical(request)) > source.MAX_COMMAND_BYTES
            or not isinstance(request['reason'], str) or not 1 <= len(request['reason'].strip()) <= 4096):
        raise ValueError('invalid bounded compound source request')
    if create:
        source._instant(request['fixity_verified_at'])
        if ((request['inventory'] is None) != (request['inventory_limitation'] is not None)):
            raise ValueError('inventory completeness and limitation must be explicit')
        if (not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256
                or any(not isinstance(request[key], str) or not HASH.fullmatch(request[key])
                       for key in ('expected_configuration', 'expected_revision', 'expected_dependencies'))
                or request['expected_publication'] is not None and (
                    not isinstance(request['expected_publication'], str) or not HASH.fullmatch(request['expected_publication']))
                or not isinstance(request['fields'], dict) or set(request['fields']) != {'exemplar_claim_refs'}):
            raise ValueError('compound publication requires exact prepared lineage and dependency bindings')

def _scope(config, request, *, recovery=False, original=None):
    if (RECOVERY if recovery else OPERATION) not in config['allowed_operations']:
        raise PermissionError('compound source operation is not delegated')
    claimed = original if recovery else config
    record, claim = request['record'], request['claim']
    if (not isinstance(record, dict) or record.get('record_id') != config['item_id']
            or record.get('item_manifest_ref') != str(Path(config['item_source_path']).with_name('item.manifest.json')) or not isinstance(claim, dict)
            or claim.get('claim_id') != config['claim_id'] or claim.get('subject_ref') != config['edition_id']
            or claim.get('object') != config['item_id']
            or claim.get('provenance_event_ref') != config['provenance_event_id']
            or claim.get('maker') != {'maker_type': claimed['maker_type'], 'agent_ref': claimed['principal_id']}):
        raise PermissionError('compound identities, endpoints, provenance or maker are not delegated')
    if (not isinstance(request['rights'], dict) or request['rights'].get('rights_id') != config['rights_id']
            or set(request['rights'].get('scope_refs', [])) != {config['item_id'], config['file_id']}
            or request['rights'].get('visibility') != 'local_only'
            or request['rights'].get('review_status') != 'unreviewed'
            or request['rights'].get('assessment_status') not in {'not_assessed', 'copyright_not_evaluated', 'copyright_undetermined'}
            or request['rights'].get('redistribution_posture') != 'not_authorized'
            or request['rights'].get('derivative_posture') != 'local_research_only'
            or request['rights'].get('permissions') != [] or request['rights'].get('layer_assessments', []) != []
            or request['item_kind'] not in {'born_digital', 'digitized_physical_copy', 'derived_publication', 'unknown'}):
        raise PermissionError('the separate supplied rights record must remain exact, unreviewed and local-only')
    for field, allowed in (('forms', 'allowed_edition_form_ids'), ('item_forms', 'allowed_item_form_ids'),
                           ('claim_forms', 'allowed_claim_form_ids')):
        _selections(request[field], config[allowed])

def _grammar(root, edition, item, claim):
    corpus_ref = 'ToS/contracts/corpus-record.schema.json'
    raw = source._read(root / corpus_ref, source.MAX_SET_BYTES)
    schema = source._json_object(raw)
    source.Draft202012Validator.check_schema(schema)
    validator = source.Draft202012Validator(schema, format_checker=source.FormatChecker())
    validator.validate(edition)
    validator.validate(item)
    profiles = SourceClaimProfiles(root)
    profiles.validate(claim, {edition['record_id']: edition, item['record_id']: item})
    refs = {**profiles.input_digests, corpus_ref: hashlib.sha256(raw).hexdigest()}
    for ref in (*FORM_CONTRACTS, 'ToS/contracts/provenance-event-v2.schema.json',
            'ToS/contracts/source-item-manifest.schema.json', 'ToS/contracts/source-resource-inventory.schema.json',
            'ToS/contracts/rights-record.schema.json', 'ToS/contracts/provenance-event.schema.json'):
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
    """Bind the exact Edition/Item neighborhood, including native older lineage."""
    from source_edition_commands import verify_compound as verify_edition, _verify_initial_child as verify_initial
    edition_path = Path(scope['edition_source_path'])
    edition = source._json_object(before[edition_path.name])
    if edition.get('record_id') != scope['edition_id'] or edition.get('record_type') != 'edition':
        raise PermissionError('selected parent is not the exact delegated Edition')
    history = revisions._history(before, edition)
    records, claims, digests = _read_catalog(root, request.get('expected_publication'))
    entry = records.get(scope['edition_id'])
    if (entry is None or entry.get('source_record_ref') != scope['edition_source_path']
            or entry.get('record_sha256') != _record_ref(edition)['digest'][7:]):
        raise source.JournalConflict('the selected Edition is absent or stale in its catalog')
    for identity in (scope['item_id'], scope['file_id'], scope['claim_id']):
        if identity in records or identity in claims:
            raise source.JournalConflict('new Item, File or Claim identity is already cataloged')
    if any(entry.get('provenance_event_ref') in {scope['provenance_event_id'], scope['acquisition_event_id'],
            scope['inventory_event_id']} for entry in claims.values()):
        raise source.JournalConflict('new event identity already belongs to a cataloged Claim')
    retained = {}
    origins = [entry for entry in claims.values() if entry.get('predicate') == 'embodied_by'
               and entry.get('object') == scope['edition_id']]
    if {entry.get('subject_ref') for entry in origins} != set(edition.get('embodies_expression_refs', [])):
        raise ValueError('the existing Edition lacks exact declared Expression origins')
    for origin_entry in origins:
        origin = _catalog_claim(root, origin_entry, digests)
        if Path(origin_entry['source_claim_file_ref']).name == SOURCE_CLAIM_BASENAME:
            verified = verify_edition(root, origin_entry['source_claim_file_ref'], origin, _verify_current=False)
            verify_initial(root, scope['edition_source_path'], before, verified['receipt'], 'edition')
            retained[verified['transaction_id']] = verified['manifest_sha256']
        else:
            _legacy_binding(root, origin_entry, origin, digests)
    # The catalog locates Item records, not File identities or their Edition
    # backlink. Bind their exact declared manifests; never infer links from a
    # convenient directory spelling or fabricate a File catalog.
    items = {}
    item_entries = [(identity, entry) for identity, entry in records.items() if entry.get('record_type') == 'item']
    if len(item_entries) > 96:
        raise ValueError('Item manifest identity check exceeds its explicit selected read budget')
    for identity, entry in item_entries:
        record = _catalog_record(root, entry, digests)
        manifest_ref = record.get('item_manifest_ref')
        expected_manifest = str(Path(entry['source_record_ref']).with_name('item.manifest.json'))
        if manifest_ref != expected_manifest or entry.get('links', {}).get('item_manifest_ref') != manifest_ref:
            raise source.JournalConflict('existing Item manifest locator differs from its source and catalog')
        raw = source._read(root / manifest_ref, source.MAX_SET_BYTES)
        manifest = source._json_object(raw)
        if manifest.get('item_id') != identity or manifest.get('schema_version') != 'tos_source_item_manifest_v1':
            raise source.JournalConflict('existing Item manifest changed identity or grammar')
        digests[manifest_ref] = hashlib.sha256(raw).hexdigest()
        if any(row.get('file_id') == scope['file_id'] for row in manifest.get('payload_files', [])):
            raise source.JournalConflict('the granted new File identity already belongs to a manifest')
        if manifest.get('acquisition_event_ref') in {scope['acquisition_event_id'], scope['inventory_event_id'], scope['provenance_event_id']}:
            raise source.JournalConflict('the granted event identity already belongs to an Item manifest')
        for field, identity_key, forbidden in (
                ('rights_ref', 'rights_id', {scope['rights_id']}),
                ('resource_inventory_ref', 'provenance_event_ref',
                 {scope['acquisition_event_id'], scope['inventory_event_id'], scope['provenance_event_id']})):
            ref = transactions._path(manifest[field]).as_posix()
            raw = source._read(root / ref, source.MAX_SET_BYTES)
            value = source._json_object(raw)
            digests[ref] = hashlib.sha256(raw).hexdigest()
            if value.get(identity_key) in forbidden:
                raise source.JournalConflict('a granted new rights/event identity already belongs to an Item companion')
        if manifest.get('embodiment_ref') == scope['edition_id']:
            items[identity] = record
    selected = {identity: _catalog_claim(root, entry, digests) for identity, entry in claims.items()
                if entry.get('predicate') == 'exemplified_by' and entry.get('subject_ref') == scope['edition_id']}
    if (set(edition.get('exemplar_claim_refs', [])) != set(selected)
            or len(edition.get('exemplar_claim_refs', [])) != len(selected)
            or {claim['object'] for claim in selected.values()} != set(items) or len(selected) != len(items)):
        raise ValueError('existing Edition topology lacks exact Item forward/backlink closure')
    for identity, claim in selected.items():
        if Path(claims[identity]['source_claim_file_ref']).name == SOURCE_CLAIM_BASENAME:
            verified = verify_compound(root, claims[identity]['source_claim_file_ref'], claim,
                                       _parent_before=before, _verify_current=False)
            if verified['parent_receipt'] not in history['receipts']:
                raise source.JournalCorruption('native exemplar is absent from Edition lineage')
            child_ref = records[claim['object']]['source_record_ref']
            verify_initial(root, child_ref, revisions._selected_package(root / child_ref), verified['receipt'], 'item')
            retained[verified['transaction_id']] = verified['manifest_sha256']
        else:
            _legacy_binding(root, claims[identity], claim, digests)
    return {'catalog_and_sources': digests, 'contracts': _grammar(root, edition, request['record'], request['claim']),
        'implementation': {ref: hashlib.sha256(source._read(source.ROOT / ref, source.MAX_SET_BYTES)).hexdigest()
                           for ref in IMPLEMENTATIONS}, 'retained_transactions': retained}

def _transaction_id(request):
    return source._digest(source._canonical({'operation': OPERATION, 'command_id': request['command_id'],
        'owner_configuration': request['expected_configuration'], 'request_digest': source._digest(source._canonical(request))}))

def _archive_config(root, scope):
    # Read/archive storage grammar only, never an invented record.revise grant.
    return {'schema_version': source.CORPUS_SELECTED_REVISION_CONFIG, 'source_root': str(root),
            'source_path': scope['edition_source_path'], 'record_id': scope['edition_id']}

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
            or publication['selected_files'] != sorted(revisions._selected_names(Path('edition.json')))
            or receipt.get('changed_fields') != ['exemplar_claim_refs']
            or request['claim'].get('predicate') != 'exemplified_by'
            or request['claim'].get('subject_ref') != receipt['previous_source']['id']
            or request['claim'].get('object') != request['record'].get('record_id')
            or not isinstance(request['fields']['exemplar_claim_refs'], list)
            or not request['fields']['exemplar_claim_refs']
            or request['fields']['exemplar_claim_refs'][-1] != request['claim'].get('claim_id')):
        raise source.JournalCorruption('invalid explicit compound parent lineage binding')

def _new_directories(root, scope, *, stage=None):
    child = Path(scope['item_source_path']).parent
    result = []
    for path in (child.parent, child):
        try:
            descriptor = source._owned_path(root / path, directory=True)
        except FileNotFoundError:
            result.append(path.as_posix())
        else:
            os.close(descriptor)
            if path == child:
                # Only this operation's independently granted payload home may
                # preexist. Never include payload-bearing directories in rollback.
                if stage is None or 'payload_root' not in scope or deposit.destination(scope).parent.parent != root / child:
                    raise source.JournalConflict('the new Item metadata home is already occupied')
                if {entry.name for entry in (root / child).iterdir()} != {'payload'}:
                    raise source.JournalConflict('the payload-only Item home contains unrelated metadata')
    return result

def _event(*args, **kwargs):
    value = common._event(sys.modules[__name__], *args, **kwargs)
    for entity in value['entities']['outputs']:
        if entity['entity_ref'].endswith('/forensic-report.md'):
            entity['media_type'] = 'text/markdown'
        elif entity['entity_ref'].endswith('/fixity.sha256'):
            entity['media_type'] = 'text/plain'
    return value

def _compose(root, scope, request, before, dependencies, *, recorded_at, environment, byte_receipt=None):
    edition_path = Path(scope['edition_source_path'])
    base = Path(scope['item_source_path']).parent
    edition = source._json_object(before[edition_path.name])
    revised = {**edition, **request['fields'], 'record_version': edition['record_version'] + 1}
    item, claim = request['record'], request['claim']
    _grammar(root, revised, item, claim)
    validate_edition_item_delta(edition, revised, item, claim,
        edition_source_ref=scope['edition_source_path'], item_source_ref=scope['item_source_path'])
    history = revisions._history(before, edition)
    if len(history['receipts']) >= revisions.MAX_REVISIONS:
        raise ValueError('parent Edition history capacity reached')
    formname = edition_path.stem + '.human-forms.json'
    previous_forms = source._json_object(before[formname]) if formname in before else None
    parent_forms, parent_views, parent_refs = _forms(revised, previous_forms, request['forms'], scope['principal_id'])
    item_forms, item_views, item_refs = _forms(item, None, request['item_forms'], scope['principal_id'])
    claim_forms, claim_views, claim_refs = _forms(claim, None, request['claim_forms'], scope['principal_id'], claim=True)
    identifier = _transaction_id(request)
    parent_receipt = {'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'reason': request['reason'], 'previous_source': _record_ref(edition), 'source': _record_ref(revised),
        'previous_revision': request['expected_revision'],
        'archive_path': revisions._archive_path({'record_id': scope['edition_id']}, request['expected_revision']).as_posix(),
        'dependencies': request['expected_dependencies'], 'changed_fields': ['exemplar_claim_refs'],
        'forms': parent_refs, 'grants_admission': False, 'request': request,
        'publication': {'protocol': revisions.SELECTED_PROTOCOL, 'transaction_id': identifier,
                        'selected_files': sorted(revisions._selected_names(edition_path))}}
    validate_parent_receipt(parent_receipt)
    parent = {edition_path.name: revisions._encode(revised), formname: revisions._encode(parent_forms),
        revisions.HISTORY: revisions._encode({'schema_version': 'tos_source_revision_history_v2',
            'record_id': scope['edition_id'], 'receipts': [*history['receipts'], parent_receipt]})}
    child = {'item.json': revisions._encode(item),
        'item.human-forms.json': revisions._encode(item_forms),
        SOURCE_CLAIM_BASENAME: source._canonical(claim) + b'\n',
        source.claim_forms_path(base / SOURCE_CLAIM_BASENAME, scope['claim_id']).name: revisions._encode(claim_forms)}
    if byte_receipt is None:
        # Read-only preview only. The mutation guard requires the actual
        # completed private stage's digest-bound receipt before any metadata.
        byte_receipt = {'schema_version': 'tos_item_deposit_receipt_v1', 'transaction_id': identifier,
            'owner_configuration': request['expected_configuration'], 'private_stage_digest': 'sha256:' + '0' * 64,
            'recovery_configuration': None,
            'file': deposit.payload_entry(scope), 'started_at': request['fixity_verified_at'],
            'observation_interval': {'started_at': request['fixity_verified_at'],
                                     'ended_at': request['fixity_verified_at']},
            'deposited_at': request['fixity_verified_at'], 'original_preserved': True,
            'metadata_committed': False, 'grants_admission': False}
    deposit.validate_public_receipt(byte_receipt, scope, request, identifier)
    child.update(_item_companions(root, scope, request, byte_receipt))
    child[BYTE_RECEIPT_FILE] = revisions._encode(byte_receipt)
    outputs = {**{str(edition_path.parent / name): raw for name, raw in parent.items()},
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
    files = {**{str(edition_path.parent / name): raw for name, raw in parent.items()},
             **{str(base / name): raw for name, raw in child.items()}}
    receipt = {'schema_version': RECEIPT, 'operation': OPERATION, 'transaction_id': identifier,
        'command_id': request['command_id'], 'request_digest': source._digest(source._canonical(request)),
        'principal_id': scope['principal_id'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'recorded_at': recorded_at,
        'scope': {key: scope[key] for key in sorted(SCOPE_KEYS)}, 'dependencies': request['expected_dependencies'],
        'parent_before': _record_ref(edition), 'parent_after': _record_ref(revised),
        'parent_revision': request['expected_revision'], 'parent_archive_ref': parent_receipt['archive_path'],
        'parent_transition_sha256': source._digest(source._canonical(parent_receipt)),
        'parent_before_files': revisions._file_refs(before), 'item': _record_ref(item), 'claim': _claim_ref(claim),
        'forms': {'edition': parent_refs, 'item': item_refs, 'claim': claim_refs},
        'files': revisions._file_refs(files), 'grants_admission': False}
    child[RECEIPT_FILE] = revisions._encode(receipt)
    if any(len(raw) > source.MAX_SET_BYTES for raw in [*parent.values(), *child.values()]):
        raise ValueError('compound selected metadata exceeds the per-file budget')
    return parent, child, receipt, parent_receipt, {'edition': parent_views, 'item': item_views, 'claim': claim_views}

def _authorization(scope, request, dependencies):
    return {'schema_version': AUTHORIZATION, 'scope': {key: scope[key] for key in SCOPE_KEYS},
        'principal_id': scope['principal_id'], 'maker_type': scope['maker_type'], 'authority_ref': scope['authority_ref'],
        'owner_configuration': request['expected_configuration'], 'command_id': request['command_id'],
        'request_digest': source._digest(source._canonical(request)), 'dependency_bindings': dependencies}

def _plan(scope, authorization, before, parent, child, directories):
    edition = Path(scope['edition_source_path'])
    base = Path(scope['item_source_path']).parent
    return {'authorization': authorization, 'path_profile': {'schema_version': 'tos_item_metadata_paths_v1',
        'item_source_path': scope['item_source_path']}, 'new_directories': directories, 'files': sorted([
        *({'path': str(edition.parent / name), 'before': before.get(name), 'after': parent[name]}
          for name in revisions._selected_names(edition)),
        *({'path': str(base / name), 'before': None, 'after': raw} for name, raw in child.items()),
    ], key=lambda item: item['path'])}

def _validate_plan(root, plan):
    """Reconstruct every intended byte, not an authorization claim in prose."""
    authority = plan['authorization']
    source._keys(authority, {'schema_version', 'scope', 'principal_id', 'maker_type', 'authority_ref',
                             'owner_configuration', 'command_id', 'request_digest', 'dependency_bindings'})
    if authority['schema_version'] != AUTHORIZATION:
        raise PermissionError('selected transaction is not the native Edition Item adapter')
    _validate_scope_shape(authority['scope'])
    scope = {**authority['scope'], **{key: authority[key] for key in ('principal_id', 'maker_type', 'authority_ref')}}
    edition, base = Path(scope['edition_source_path']), Path(scope['item_source_path']).parent
    rows = {item['path']: item for item in plan['files']}
    if len(rows) != len(plan['files']):
        raise source.JournalCorruption('duplicate compound selected path')
    request = source._json_object(rows[str(base / REQUEST_FILE)]['after'])
    receipt = source._json_object(rows[str(base / RECEIPT_FILE)]['after'])
    environment = source._json_object(rows[str(base / ENVIRONMENT_FILE)]['after'])
    _request(request, create=True)
    _scope({**scope, 'allowed_operations': [OPERATION]}, request)
    before = {name: rows[str(edition.parent / name)]['before'] for name in revisions._selected_names(edition)
              if str(edition.parent / name) in rows and rows[str(edition.parent / name)]['before'] is not None}
    if edition.name not in before:
        raise source.JournalCorruption('compound has no exact retained parent input')
    record = source._json_object(before[edition.name])
    if (_record_ref(record) != request['expected_source'] or revisions._revision(before) != request['expected_revision']
            or authority['owner_configuration'] != request['expected_configuration']
            or authority['command_id'] != request['command_id']
            or authority['request_digest'] != source._digest(source._canonical(request))
            or source._digest(source._canonical(authority['dependency_bindings'])) != request['expected_dependencies']):
        raise source.JournalCorruption('compound retained authorization does not bind its request and parent input')
    directories = plan['new_directories']
    if directories not in ([], [base.as_posix()], [base.parent.as_posix(), base.as_posix()]):
        raise PermissionError('compound transaction may create only its exact Item home and missing items parent')
    parent, child, expected_receipt, parent_receipt, views = _compose(root, scope, request, before,
        authority['dependency_bindings'], recorded_at=receipt['recorded_at'], environment=environment,
        byte_receipt=source._json_object(rows[str(base / BYTE_RECEIPT_FILE)]['after']))
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
    if (claim_path != Path(scope['item_source_path']).with_name(SOURCE_CLAIM_BASENAME)
            or receipt != expected or receipt_raw != child[RECEIPT_FILE] or claim != request['claim']
            or source._read(root / claim_path, source.MAX_SET_BYTES) != child[SOURCE_CLAIM_BASENAME]):
        raise source.JournalCorruption('native topology source is not the exact committed compound Claim')
    # Capture/receipt files remain immutable even after descriptive corrections.
    for name in (REQUEST_FILE, ENVIRONMENT_FILE, PROVENANCE_FILE, BYTE_RECEIPT_FILE, 'item.manifest.json', 'rights.json',
                 'provenance.jsonl', 'resource-inventory.json', 'fixity.sha256', 'forensic-report.md'):
        if source._read(root / claim_path.parent / name, source.MAX_SET_BYTES) != child[name]:
            raise source.JournalCorruption('native compound capture bytes were changed')
    current_parent = _parent_before if _parent_before is not None else revisions._selected_package(root / scope['edition_source_path'])
    record = source._json_object(current_parent['edition.json'])
    if record.get('record_id') != scope['edition_id'] or record.get('record_type') != 'edition':
        raise source.JournalCorruption('current compound parent changed its typed identity')
    history = revisions._history(current_parent, record)
    if parent_receipt not in history['receipts']:
        raise source.JournalCorruption('native compound transition is absent from the current parent lineage')
    for item in history['receipts']:
        revisions._read_archive(root, _archive_config(root, scope), item)
    if _verify_current:
        current_item = revisions._selected_package(root / scope['item_source_path'])
        item = source._json_object(current_item['item.json'])
        if (item.get('record_id') != scope['item_id'] or item.get('record_type') != 'item'
                or item.get('item_manifest_ref') != str(Path(scope['item_source_path']).with_name('item.manifest.json'))):
            raise source.JournalCorruption('current Item changed its typed parent binding')
        item_history = revisions._history(current_item, item)
        initial_found = current_item['item.json'] == child['item.json']
        for item in item_history['receipts']:
            archived, _ = revisions._read_archive(root, {'record_id': scope['item_id'],
                'source_path': scope['item_source_path']}, item)
            if item['previous_source'] == receipt['item']:
                if archived['item.json'] != child['item.json']:
                    raise source.JournalCorruption('Item initial lineage does not retain exact compound bytes')
                initial_found = True
        if not initial_found:
            raise source.JournalCorruption('current Item does not descend from its committed initial record')
        snapshot.verify_current()
    return {'transaction_id': receipt['transaction_id'], 'manifest_sha256': inspected['manifest_sha256'],
        'parent_receipt': parent_receipt, 'claim': copy.deepcopy(claim), 'receipt': receipt,
        'event': source._json_object(child[PROVENANCE_FILE]),
        'grants_admission': False, 'writes_to_source': False}

def _check_dependencies(*args, **kwargs):
    return common._check_dependencies(sys.modules[__name__], *args, **kwargs)

def _read_owner(owner):
    return configuration(source._json_object(source._read(owner, source.MAX_COMMAND_BYTES)), owner_config=owner)

def _guard(owner, config, configuration_digest, scope, request, before, authority, *, recovery):
    base_guard = common._guard(sys.modules[__name__], owner, config, configuration_digest,
                              scope, request, before, authority, recovery=recovery)
    def guard(retained, summary):
        base_guard(retained, summary)
        stage = deposit.read_stage(config, _transaction_id(request))
        if (stage is None or stage['state'] != 'deposited' or stage['binding']['request'] != request
                or source._digest(source._canonical(stage['binding']['configuration'])) != request['expected_configuration']):
            raise source.JournalCorruption('metadata publication lacks its exact retained completed File deposit')
        if any(config[key] != stage['binding']['configuration'][key]
               for key in ('source_root', 'input_path', 'payload_root', 'recovery_root')):
            raise PermissionError('recovery cannot change metadata, original, payload or private companion roots')
        deposit.verify_deposit(config, stage)
        receipt_ref = str(Path(config['item_source_path']).with_name(BYTE_RECEIPT_FILE))
        selected = next((row for row in summary['files'] if row['path'] == receipt_ref), None)
        if selected is None or selected['after'] != transactions._binding(revisions._encode(deposit.public_receipt(stage))):
            raise source.JournalCorruption('metadata plan does not retain the actual source-safe byte deposit receipt')
        return True
    return guard

def _source_descriptors(root):
    """Expose existing owner contracts, without a second registry or grant."""
    claims = SourceClaimProfiles(root)
    corpus = source._json_object(source._read(root / CORPUS_REF, source.MAX_SET_BYTES))
    result = {kind: {'type_id': claims.mappings[kind], 'record_type': kind,
        'schema_ref': CORPUS_REF, 'schema_version': corpus['properties']['schema_version']['const'],
        'source_basename': kind + '.json'} for kind in ('edition', 'item')}
    profile = claims.profiles['exemplified_by']
    route = claims.schema_routes['exemplified_by', 'tos_source_relation_claim_v1']
    result['exemplified_by'] = {'relation_type_id': claims.relations['exemplified_by']['relation_type_id'],
        'predicate': 'exemplified_by', 'reader': profile['reader'],
        'schema_ref': route['schema_ref'], 'schema_version': route['schema_version'],
        'assertion_layers': list(profile['assertion_layers']), 'source_basename': SOURCE_CLAIM_BASENAME}
    return result

def _result(config, configuration_digest, *, receipt=None, replayed=False, recovery=None, views=None):
    root = Path(config['source_root'])
    snapshot = PublicationSnapshot(root)
    path = root / config['edition_source_path']
    files = revisions._selected_package(path)
    edition = source._json_object(files[path.name])
    revisions._history(files, edition)
    result = {'schema_version': 'tos_edition_item_result_v1', 'authentication': 'local-unix-account',
        'owner_configuration': configuration_digest, 'operation': OPERATION,
        'command_operations': ['describe', 'prepare-create', OPERATION, RECOVERY],
        'allowed_operations': config['allowed_operations'], 'edition_source_path': config['edition_source_path'],
        'item_source_path': config['item_source_path'], 'source': _record_ref(edition),
        'revision': revisions._revision(files), 'publication_snapshot': snapshot.token,
        'source_profiles': _source_descriptors(root),
        'source_fields': [{key: value for key, value in field.items() if key not in ('pointer', 'context')}
                          for field in source.metadata_field_catalog(edition)],
        'scope': {key: config[key] for key in SCOPE_KEYS}, 'receipt': receipt,
        'replayed': replayed, 'recovery': recovery, 'materializations': views,
        'grants_admission': False}
    snapshot.verify_current()
    return result

def run_item_command(owner, config, configuration_digest, path, request):
    """Dispatch this exact grant before the generic non-pending read wrapper."""
    adapter = sys.modules[__name__]
    root = Path(config['source_root'])
    operation = request.get('operation')
    source.command_handler(adapter.CONFIG).validate_request(request)
    if operation in {adapter.PREPARE, adapter.OPERATION}:
        adapter._request(request, create=operation == adapter.OPERATION)
        adapter._scope(config, request)
    elif operation == adapter.RECOVERY:
        if (request['schema_version'] != adapter.REQUEST or request['decision'] not in {'resume', 'rollback'}
                or request['expected_configuration'] != configuration_digest or adapter.RECOVERY not in config['allowed_operations']):
            raise PermissionError('compound recovery requires its current exact delegation and explicit decision')
    elif operation == 'describe':
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
        observed = deposit.observe(config)
        proposal.update(inventory=observed['inventory'], inventory_limitation=observed['limitation'],
                        fixity_verified_at=datetime.now(timezone.utc).isoformat())
        dependencies = adapter._context(root, config, proposal, before)
        proposal['expected_dependencies'] = source._digest(source._canonical(dependencies))
        receipt, views = None, None
        if proposal['inventory'] is not None:
            _, _, receipt, _, views = adapter._compose(root, config, proposal, before, dependencies,
                recorded_at=datetime.now(timezone.utc).isoformat(), environment=_environment())
        else:
            revised = {**work, **proposal['fields'], 'record_version': work['record_version'] + 1}
            validate_edition_item_delta(work, revised, proposal['record'], proposal['claim'],
                edition_source_ref=config['edition_source_path'], item_source_ref=config['item_source_path'])
        result = {**adapter._result(config, configuration_digest), 'prepared_fields': proposal['fields'],
            **(adapter._prepared_refs(receipt) if receipt is not None else {}),
            'prepared_claim': receipt['claim'] if receipt is not None else None,
            'prepared_forms': receipt['forms'] if receipt is not None else None,
            'inventory': proposal['inventory'], 'inventory_limitation': proposal['inventory_limitation'],
            'fixity_verified_at': proposal['fixity_verified_at'],
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
            byte_state = (deposit.rollback_retained(config, identifier) if decision == 'rollback'
                          else {**deposit.public_state(deposit.read_stage(config, identifier)), 'metadata_committed': True})
            return {**adapter._result(config, configuration_digest, receipt=receipt if decision == 'resume' else None,
                           recovery=completed, views=views if decision == 'resume' else None), 'deposit': byte_state}
        active_scope = config
        if operation == adapter.RECOVERY:
            stage = deposit.read_stage(config, request['transaction_id'])
            if stage is None:
                raise source.JournalConflict('no exact pending metadata or retained byte stage is selected')
            original_config = stage['binding']['configuration']
            for key in (*SCOPE_KEYS, 'source_root', 'input_path', 'payload_root', 'recovery_root'):
                if config[key] != original_config[key]:
                    raise PermissionError('byte recovery selects another input, destination or identity')
            try:
                retained = transactions.inspect_transaction(root, request['transaction_id'])
            except FileNotFoundError:
                retained = None
            child_receipt = root / Path(config['item_source_path']).with_name(RECEIPT_FILE)
            if retained is not None and retained['status'] == 'committed':
                adapter.verify_replay(root, config, stage['binding']['request'])
                raise source.JournalConflict('this Item adoption is already committed; recovery cannot undo acquired metadata')
            if child_receipt.exists():
                raise source.JournalCorruption('Item receipt exists without exact committed or pending metadata publication')
            if request['decision'] == 'rollback':
                return {**adapter._result(config, configuration_digest),
                        'deposit': deposit.rollback_retained(config, request['transaction_id'])}
            request = stage['binding']['request']
            active_scope = original_config
            adapter._request(request, create=True)
            adapter._scope(config, request, recovery=True, original=original_config)

        snapshot = PublicationSnapshot(root)
        child_receipt = root / Path(adapter._claim_source_ref(config)).parent / adapter.RECEIPT_FILE
        try:
            existing = source._json_object(source._read(child_receipt, source.MAX_SET_BYTES))
        except FileNotFoundError:
            existing = None
        if existing is not None:
            adapter.verify_replay(root, config, request)
            stage = deposit.read_stage(config, adapter._transaction_id(request))
            if stage is None or stage['binding']['request'] != request:
                raise source.JournalCorruption('committed Item lost its exact deposit stage')
            deposit.verify_deposit(config, stage)
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
        if (request['expected_configuration'] != source._digest(source._canonical(active_scope)) or request['expected_source'] != _record_ref(work)
                or request['expected_revision'] != revisions._revision(before)
                or request['expected_publication'] != snapshot.token):
            raise source.JournalConflict('compound parent, revision, authority or publication snapshot is stale')
        try:
            retained = transactions.inspect_transaction(root, adapter._transaction_id(request))
        except FileNotFoundError:
            retained = None
        if retained is not None:
            if retained['status'] != 'orphan':
                raise source.JournalConflict('an exact terminated metadata transaction cannot be republished')
            scope, original, kept_before, _, _, receipt, _, views = adapter._validate_plan(root, retained['plan'])
            if original != request or kept_before != before:
                raise source.JournalConflict('retained pre-publication metadata selects another exact command or source')
            dependencies = adapter._context(root, scope, request, before)
            authority = retained['plan']['authorization']
            if dependencies != authority['dependency_bindings']:
                raise source.JournalConflict('retained pre-publication metadata dependencies changed')
            guard = adapter._guard(owner, config, configuration_digest, scope, request, before, authority,
                                   recovery=operation == adapter.RECOVERY)
            renewal = {'schema_version': adapter.RECOVERY_AUTHORIZATION, 'principal_id': config['principal_id'],
                'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
                'transaction_id': receipt['transaction_id'], 'decision': 'resume'} if operation == adapter.RECOVERY else None
            transactions.apply_transaction(root, retained['plan'], expected_snapshot=snapshot, authorization_guard=guard,
                transaction_id=receipt['transaction_id'], recovery_authorization=renewal)
            return adapter._result(config, configuration_digest, receipt=receipt, views=views)
        dependencies = adapter._context(root, config, request, before)
        if source._digest(source._canonical(dependencies)) != request['expected_dependencies']:
            raise source.JournalConflict('compound preparation dependencies are stale')
        def authorize_bytes():
            current, digest, current_path = adapter._read_owner(owner)
            if current != config or digest != configuration_digest or current_path != path:
                raise source.JournalConflict('Item authority changed during byte deposit')
            adapter._scope(config, request, recovery=operation == adapter.RECOVERY, original=active_scope if operation == adapter.RECOVERY else None)
            snapshot.verify_current()
        stage = deposit.ensure_deposit(config, request, adapter._transaction_id(request), authorize=authorize_bytes,
                                       recovery=operation == adapter.RECOVERY)
        if request['inventory'] is None:
            return {**adapter._result(config, configuration_digest), 'deposit': deposit.public_state(stage),
                    'next_route': 'source inventory owner: add a bounded supported profile; explicit rollback retains these bytes'}
        directories = adapter._new_directories(root, config, stage=stage)
        parent, child, receipt, _, views = adapter._compose(root, active_scope, request, before, dependencies,
            recorded_at=datetime.now(timezone.utc).isoformat(), environment=_environment(),
            byte_receipt=deposit.public_receipt(stage))
        revisions._archive(root, adapter._archive_config(root, config), before,
            source.Record.from_payload(work['record_id'], work['record_version'], work), request['expected_revision'])
        authority = adapter._authorization(active_scope, request, dependencies)
        plan = adapter._plan(active_scope, authority, before, parent, child, directories)
        adapter._validate_plan(root, plan)
        guard = adapter._guard(owner, config, configuration_digest, active_scope, request, before, authority,
                               recovery=operation == adapter.RECOVERY)
        transactions.apply_transaction(root, plan, expected_snapshot=snapshot, authorization_guard=guard,
                                       transaction_id=receipt['transaction_id'])
        return {**adapter._result(config, configuration_digest, receipt=receipt, views=views),
                'deposit': {**deposit.public_state(stage), 'metadata_committed': True}}
def _claim_source_ref(scope):
    return Path(scope['item_source_path']).with_name(SOURCE_CLAIM_BASENAME).as_posix()

def verify_replay(root, scope, request):
    return verify_compound(root, _claim_source_ref(scope), request['claim'])

def _prepare_fields(record, scope):
    return {'exemplar_claim_refs': [*record['exemplar_claim_refs'], scope['claim_id']]}

def _prepared_refs(receipt):
    return {'prepared_edition': receipt['parent_after'], 'prepared_item': receipt['item']}

def command_handlers():
    return (contract.Handler('native-item-adoption', (CONFIG,), (contract.describe(),
        contract.operation(PREPARE, PROPOSAL_KEYS, definition='Prepare one provisional Item and exact parent Edition append.', grants=(OPERATION,)),
        contract.operation(OPERATION, CREATE_KEYS - contract.BASE_KEYS,
            definition='Create the new Item, distinct exemplified_by Claim and forms with one Edition successor.',
            mutation='selected_edition_and_new_item_package', grants=(OPERATION,)), contract.recovery(RECOVERY)),
        run_item_command, 'Separately delegated native Edition to Item growth.', configure=configuration,
        request_schema=REQUEST, owner_route='mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_ITEM_ADOPTION.md',
        typed_handles=(CORPUS_REF, 'ToS/contracts/source-relation-claim.schema.json',
            'ToS/contracts/source-item-manifest.schema.json', 'ToS/contracts/source-resource-inventory.schema.json',
            'ToS/contracts/rights-record.schema.json', 'ToS/contracts/provenance-event.schema.json',
            *contract.CLAIM_HANDLES, *FORM_CONTRACTS),
        profile_selection='Exact Edition and new Item; exemplified_by uses the canonical identity-relation-v1 profile.',
        preconditions=('Requires exact current catalog/parent/source-copy forms, absent child home and distinct Claim identity.',
                       'Local file retention is separately delegated; no rights, publication, textual acceptance or equivalence is granted.'),
        manages_publication=True),)

def _schema(root, name, value):
    schema = source._json_object(source._read(root / ('ToS/contracts/' + name + '.schema.json'), source.MAX_SET_BYTES))
    source.Draft202012Validator(schema, format_checker=source.FormatChecker()).validate(value)


def _item_companions(root, scope, request, byte_receipt):
    """Preserve the existing acquired Item v1 grammar; no source text is emitted."""
    base = Path(scope['item_source_path']).parent
    ref = lambda name: (base / name).as_posix()
    stamp = byte_receipt['deposited_at']
    manifest = {'schema_version': 'tos_source_item_manifest_v1', 'item_id': scope['item_id'],
        'item_kind': request['item_kind'], 'embodiment_ref': scope['edition_id'],
        'storage_posture': 'local_gitignored_payload', 'payload_files': [deposit.payload_entry(scope, verified_at=stamp)],
        'acquisition_event_ref': scope['acquisition_event_id'], 'rights_ref': ref('rights.json'),
        'provenance_ref': ref('provenance.jsonl'), 'forensic_report_ref': ref('forensic-report.md'),
        'resource_inventory_ref': ref('resource-inventory.json'), 'visibility': 'local_only', 'manifest_version': 1}
    if request['inventory'] is None or request['inventory_limitation'] is not None:
        raise ValueError('resource inventory unavailable; bytes must remain retained without acquired Item metadata')
    inventory = {'$schema': INVENTORY_SCHEMA, 'schema_version': 'tos_source_resource_inventory_v1',
        'item_id': scope['item_id'], 'generated_from_manifest_ref': ref('item.manifest.json'),
        'inventory_authority': 'mechanical_metadata_only', 'source_text_included': False,
        'files': [request['inventory']], 'generator': {'name': 'build_source_resource_inventories.py', 'version': '1'},
        'provenance_event_ref': scope['inventory_event_id'], 'inventory_version': 1, 'supersedes_inventory_ref': None,
        'authority_boundary': INVENTORY_BOUNDARY}
    if any(request['inventory'].get(key) != value for key, value in
           (('file_id', scope['file_id']), ('file_sha256', scope['sha256']), ('media_type', scope['media_type']))):
        raise ValueError('resource inventory does not bind the exact granted File')
    rights = request['rights']
    for name, value in (('source-item-manifest', manifest), ('source-resource-inventory', inventory), ('rights-record', rights)):
        _schema(root, name, value)
    inventory_raw = revisions._encode(inventory)
    event = {'schema_version': 'tos_provenance_event_v1', 'event_id': scope['acquisition_event_id'],
        'event_type': 'acquisition', 'started_at': byte_receipt['started_at'], 'ended_at': stamp,
        'agent_refs': ['software:tos-source-item-commands'],
        'inputs': [{'ref': scope['file_id'], 'role': 'previously_acquired_local_input', 'sha256': scope['sha256']}],
        'outputs': [{'ref': ref('payload/' + scope['payload_basename']), 'role': 'retained_local_witness_bytes',
                     'sha256': scope['sha256']}],
        'method': {'maker_type': 'software', 'name': 'bounded-local-file-adoption', 'version': '1',
            'configuration': {'transaction_id': _transaction_id(request), 'owner_configuration': request['expected_configuration'],
                              'byte_receipt_ref': ref(BYTE_RECEIPT_FILE)}},
        'status': 'completed_with_warnings', 'warnings': ['Local retention only; not rights, bibliographic or textual admission.'],
        'receipt_refs': [ref(RECEIPT_FILE)], 'rights_basis_ref': ref('rights.json'), 'event_version': 1}
    enumeration = {**copy.deepcopy(event), 'event_id': scope['inventory_event_id'], 'event_type': 'forensic_inspection',
        **byte_receipt['observation_interval'],
        'inputs': [{'ref': scope['file_id'], 'role': 'resource_inventory_input', 'sha256': scope['sha256']}],
        'outputs': [{'ref': ref('resource-inventory.json'), 'role': 'tracked_text_free_resource_inventory',
                     'sha256': hashlib.sha256(inventory_raw).hexdigest()}],
        'method': {'maker_type': 'software', 'name': 'build_source_resource_inventories.py', 'version': '1',
                  'configuration': {'scope': 'resource enumeration only; no text extraction or semantic reading'}}}
    for value in (event, enumeration):
        _schema(root, 'provenance-event', value)
    report = ('# Local Item adoption forensic boundary\n\n'
        + 'File: ' + scope['file_id'] + '\nSHA-256: ' + scope['sha256'] + '\n'
        + 'Retains one unchanged previously acquired local file; the input is preserved.\n'
        + 'The inventory enumerates resources only. No OCR, correction, translation, source reading,\n'
        + 'copyright clearance, publication authorization or semantic acceptance was performed.\n'
        + 'Copy and metadata publication are separate stages; retained transaction evidence owns recovery.\n')
    return {'item.manifest.json': revisions._encode(manifest), 'rights.json': revisions._encode(rights),
        'resource-inventory.json': inventory_raw,
        'fixity.sha256': (scope['sha256'] + '  payload/' + scope['payload_basename'] + '\n').encode(),
        'forensic-report.md': report.encode(),
        'provenance.jsonl': b''.join(source._canonical(value) + b'\n' for value in (event, enumeration))}

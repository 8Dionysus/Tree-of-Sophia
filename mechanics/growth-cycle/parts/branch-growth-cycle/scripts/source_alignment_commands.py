"""Native records of the existing translation-alignment owner.

One stable Alignment subject, versioned descriptions and separately versioned
Claims. Supplied mappings are not an executed aligner or assessed translation.
Legacy packet-v1 remains unchanged; its source, mapping and rights mechanics
are reused through a transient, non-published validation view.
"""
from __future__ import annotations

import copy
from datetime import datetime, timezone
import os
from pathlib import Path
import re
import time

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

from native_text_binding import NativeTextBindingError, NativeTextBindingResolver, check_local_research_rights
from source_owner_context import OwnerLocalSourceContext, _open, _read as private_read
from source_revisions import _file_refs
import source_commands as source
import source_command_contracts as contract
import source_text_unit_commands as units
import source_text_layer_commands as layers

SCHEMA = 'native-translation-alignment-record-v1.schema.json'
LEGACY_SCHEMA = 'translation-alignment-packet-v1.schema.json'
CONTRACTS = (SCHEMA, LEGACY_SCHEMA, 'native-text-unit-binding.schema.json')
SCHEMA_VERSION = 'tos_native_translation_alignment_record_v1'
MAX_HISTORY = 64
MAX_SECONDS = 60
VISIBILITY = ('public', 'public_metadata_only', 'controlled', 'local_only', 'restricted', 'unknown')
CONFIG = 'tos_local_native_alignment_owner_v1'
BASENAME = 'native-translation-alignment.v1.json'
CONFIG_FILE, RECEIPT_FILE, INPUT_FILE = units.CONFIG_FILE, units.RECEIPT_FILE, layers.INPUT_FILE
IMPLEMENTATIONS = (*layers.IMPLEMENTATIONS,
    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_alignment_commands.py')


def _canonical(value):
    return source._canonical(value)


def _current_grants(config, *, owner_config=None, deadline=None):
    if deadline is not None and time.monotonic() >= deadline:
        raise ValueError('native alignment exceeded its cooperative command deadline')
    now = datetime.now(timezone.utc)
    if any(source._instant(value) <= now for value in (
            config['expires_at'], config['source_access']['expires_at'], config['alignment_access']['expires_at'])):
        raise PermissionError('native alignment authority expired during the command')
    if owner_config is not None and source._json_object(private_read(
            Path(owner_config), source.MAX_COMMAND_BYTES, confidential_file=True)) != config:
        raise PermissionError('native alignment authority changed before source access')


def _resolver(config, context, *, owner_config=None, deadline=None):
    def read(path, limit):
        _current_grants(config, owner_config=owner_config, deadline=deadline)
        return source._read(path, limit)
    _current_grants(config, owner_config=owner_config, deadline=deadline)
    return NativeTextBindingResolver(context.public_root, owner_context=context, read_bytes=read)


def _description(qualifications):
    """Serialization time/event alone is not a substantive description delta."""
    value = copy.deepcopy(qualifications)
    value['maker'].pop('made_at', None)
    value['maker'].pop('provenance_event_ref', None)
    return value


def claim_ref(claim):
    return {'claim_id': claim['claim_id'], 'claim_version': claim['claim_version'],
            'sha256': source._digest(_canonical(claim))[7:]}


def record_ref(path, body, raw):
    return {'record_ref': path, 'record_id': body['record_id'],
            'record_version': body['record_version'], 'sha256': source._digest(raw)[7:]}


def _validator(resolver, name=SCHEMA):
    cached = getattr(resolver, '_alignment_contracts', None)
    if cached is not None:
        return cached[0][name], cached[1]
    def reject(_uri):
        raise NativeTextBindingError('alignment schema selects an undeclared resource')
    registry, schemas = Registry(retrieve=reject), {}
    for selected in CONTRACTS:
        ref = 'ToS/contracts/' + selected
        schema = source._json_object(resolver._read(ref, schema=True))
        if schema.get('$id') != 'https://tree-of-sophia.local/' + ref:
            raise NativeTextBindingError('alignment schema identity differs from its source owner')
        Draft202012Validator.check_schema(schema)
        registry = registry.with_resource(schema['$id'], Resource.from_contents(schema))
        schemas[selected] = schema
    validators = {key: Draft202012Validator(value, registry=registry, format_checker=FormatChecker())
                  for key, value in schemas.items()}
    resolver._alignment_contracts = validators, schemas
    return validators[name], schemas


def source_side(resolver, bindings, role, *, tokenization=False, verify_content=False):
    """Resolve selected units of one exact segmentation; never infer units.

    Both metadata and recorded derivative rights are checked before any byte
    read. Tokenization is an explicit owner-selected use of this same frozen
    native segmentation, not a new tokenizer execution or linguistic verdict.
    """
    if not isinstance(bindings, list) or not 1 <= len(bindings) <= 256:
        raise NativeTextBindingError('alignment side exceeds its bounded native selection')
    first, selected, summaries = bindings[0], [], []
    common = ('packet_ref', 'packet_sha256', 'packet_id', 'packet_version',
              'segmentation_id', 'segmentation_version', 'text_layer', 'source_record_refs')
    for binding in bindings:
        if any(binding.get(key) != first.get(key) for key in common):
            raise NativeTextBindingError('alignment side crosses its exact frozen native segmentation')
        summaries.append(resolver.resolve(binding))
        selected.append(binding['unit_id'])
    packet = resolver._record(first['packet_ref'], expected=first['packet_sha256'])
    layer = resolver._record(first['text_layer']['record_ref'], expected=first['text_layer']['record_sha256'])
    check_local_research_rights(resolver, layer)
    segmentation = next(row for row in packet['segmentations'] if row['segmentation_id'] == first['segmentation_id'])
    if (len(set(selected)) != len(selected)
            or selected != [key for key in segmentation['ordered_unit_refs'] if key in selected]):
        raise NativeTextBindingError('alignment side repeats or reorders native unit membership')
    if verify_content:
        for binding in bindings:
            resolver.resolve(binding, verify_content=True, allow_private_content=True)
    refs = [ref for binding in bindings for ref in binding['ordered_anchor_refs']]
    if len(refs) != len(set(refs)):
        raise NativeTextBindingError('alignment side repeats a selected native anchor')
    anchors = {row['anchor_ref']: row for row in packet['anchors']}
    # The native packet owns ordinals, selectors and exact source-return bytes.
    if refs != sorted(refs, key=lambda ref: anchors[ref]['ordinal']):
        raise NativeTextBindingError('alignment side changes native anchor order')
    _, schemas = _validator(resolver)
    keys = schemas[LEGACY_SCHEMA]['$defs']['sourceAnchor']['properties']
    selected_anchors = [{key: anchors[ref][key] for key in keys} for ref in refs]
    frozen = {'artifact_ref': first['packet_ref'], 'sha256': first['packet_sha256'], 'state': 'frozen'}
    visibility = max((row['effective_visibility'] for row in summaries), key=VISIBILITY.index)
    rep = layer['representation']
    return {'side_role': role, **packet['source_scope'],
        'text_layer_ref': first['text_layer']['record_ref'], 'text_layer_sha256': rep['content_sha256'],
        'language': rep['language'], 'segmentation': frozen,
        'tokenization': copy.deepcopy(frozen) if tokenization else None,
        'anchors': selected_anchors, 'rights_refs': [row['ref'] for row in rep['rights_record_refs']],
        'visibility': visibility, 'publication_authorized': False}


def rights_for(source_side, target_side):
    effective = max((source_side['visibility'], target_side['visibility'], 'local_only'), key=VISIBILITY.index)
    return {'source_visibility': source_side['visibility'], 'target_visibility': target_side['visibility'],
        'packet_visibility': 'local_only', 'effective_visibility': effective,
        'rights_record_refs': sorted(set(source_side['rights_refs']) | set(target_side['rights_refs'])),
        'private_source_used': True, 'publication_authorized': False,
        'inheritance_policy': 'most_restrictive_side_or_packet_wins'}


def legacy_view(body):
    """Only shared v1 mechanical validation, never a stored/history projection.

    v1 has ID-only lineage and reciprocal in-packet competition. Neither is
    forged: exact native lineage/competition is checked independently below.
    """
    claim = body['claim']
    alignment = {'alignment_id': body['alignment_id'], 'alignment_version': 1,
        'supersedes_alignment_ref': None,
        'identity_policy': 'opaque-id-independent-of-text-label-translation-and-current-mapping',
        'claim_id': claim['claim_id'], 'claim_version': 1, 'supersedes_claim_ref': None,
        **claim['mapping'], **claim['qualifications'], 'status': 'proposed',
        'competing_alignment_refs': [], 'review_refs': []}
    return {'$schema': 'https://tree-of-sophia.local/ToS/contracts/' + LEGACY_SCHEMA,
        'schema_version': 'tos_translation_alignment_packet_v1',
        'packet_id': body['record_id'].replace('translation-alignment-record', 'translation-alignment-packet'),
        'packet_version': 1, 'supersedes_packet_ref': None, 'content_posture': 'source_bound',
        'granularity': body['granularity'], 'source_side': body['source_side'], 'target_side': body['target_side'],
        'alignments': [alignment], 'reviews': [], 'projections': [],
        'rights_and_visibility': body['rights_and_visibility'], 'authority_boundary': body['authority_boundary']}


def validate_record(resolver, body, *, verify_content=False, visiting=(), cache=None):
    """Schema + shared mapping law + exact native closure + bounded history.

    No positive translation verdict, source text, private selector or locator
    is returned. A live caller supplies its own independent exact read grant.
    """
    if type(verify_content) is not bool:
        raise NativeTextBindingError('native alignment content verification must be explicit')
    cache = {} if cache is None else cache
    validator, _ = _validator(resolver)
    if not validator.is_valid(body):
        raise NativeTextBindingError('native alignment record violates its source schema')
    identity = body['record_id'], body['record_version']
    if identity in visiting or len(visiting) >= MAX_HISTORY:
        raise NativeTextBindingError('native alignment history is cyclic or over budget')
    visiting = (*visiting, identity)
    view = legacy_view(body)
    validator, _ = _validator(resolver, LEGACY_SCHEMA)
    from validate_source_witness_foundation import _translation_alignment_v1_issues
    if not validator.is_valid(view) or _translation_alignment_v1_issues(view):
        raise NativeTextBindingError('native alignment violates existing mapping, evidence or rights law')
    sides = {}
    # First check metadata and rights on BOTH sides, then allow representation
    # reads. Denial on the second side must not leak the first side's bytes.
    for role in ('source', 'target'):
        declared = body[role + '_side']
        actual = source_side(resolver, body['native_bindings'][role], role,
                             tokenization=declared['tokenization'] is not None)
        if actual != declared:
            raise NativeTextBindingError('native alignment side differs from its exact source return')
        sides[role] = actual
    if rights_for(sides['source'], sides['target']) != body['rights_and_visibility']:
        raise NativeTextBindingError('native alignment widens or changes its exact inherited rights')
    source_binding, target_binding = (body['native_bindings'][role][0] for role in ('source', 'target'))
    if (source_binding['packet_id'] == target_binding['packet_id']
            or source_binding['segmentation_id'] == target_binding['segmentation_id']
            or {row['unit_id'] for row in body['native_bindings']['source']}
               & {row['unit_id'] for row in body['native_bindings']['target']}):
        raise NativeTextBindingError('different translation expressions reuse a native packet, segmentation or unit identity')
    mapping = body['claim']['mapping']
    if mapping['order_posture'] == 'monotonic':
        for role in ('source', 'target'):
            ordinals = {row['anchor_ref']: row['ordinal'] for row in sides[role]['anchors']}
            positions = [ordinals[ref] for ref in mapping['ordered_' + role + '_anchor_refs']]
            if positions != sorted(positions):
                raise NativeTextBindingError('monotonic alignment reverses a declared native anchor order')
    def read_previous(ref):
        # Cache the entire asserted immutable reference. Equal bytes/path do
        # not validate a different claimed record identity or version.
        key = ref['record_ref'], ref['sha256'], ref['record_id'], ref['record_version']
        if key not in cache:
            raw = resolver._read(ref['record_ref'], expected=ref['sha256'])
            prior = source._json_object(raw)
            if record_ref(ref['record_ref'], prior, raw) != ref:
                raise NativeTextBindingError('native alignment predecessor identity/version differs')
            # Add only after full validation: a cache cannot mask a cycle.
            # History must first prove the exact SAME native scope. Opening a
            # differently bound predecessor before that check would widen the
            # current source selection even when publication later fails.
            validate_record(resolver, prior, verify_content=False, visiting=visiting, cache=cache)
            cache[key] = prior
        return cache[key]

    claim, change = body['claim'], body['change_kind']
    if change in {'describe', 'remap'}:
        previous = read_previous(body['predecessor'])
        if (body['record_id'] != previous['record_id'] or body['record_version'] != previous['record_version'] + 1
                or body['alignment_id'] != previous['alignment_id']
                or claim['predecessor'] != claim_ref(previous['claim'])
                or any(body[key] != previous[key] for key in ('native_bindings', 'source_side', 'target_side',
                    'granularity', 'rights_and_visibility', 'competing_records'))):
            raise NativeTextBindingError('native descriptive succession changed its stable subject or exact source scope')
        if change == 'describe':
            if (claim['claim_id'] != previous['claim']['claim_id']
                    or claim['claim_version'] != previous['claim']['claim_version'] + 1
                    or claim['mapping'] != previous['claim']['mapping']
                    or _description(claim['qualifications']) == _description(previous['claim']['qualifications'])):
                raise NativeTextBindingError('descriptive revision rekeys/remaps a Claim or has no descriptive change')
        elif (claim['claim_id'] == previous['claim']['claim_id']
                or claim['mapping'] == previous['claim']['mapping']):
            raise NativeTextBindingError('remapping requires a distinct new Claim and an actual mapping change')
    for ref in body['competing_records']:
        alternative = read_previous(ref)
        if (body['alignment_id'] == alternative['alignment_id'] or body['record_id'] == alternative['record_id']
                or claim['claim_id'] == alternative['claim']['claim_id']
                or body['native_bindings'] != alternative['native_bindings']):
            raise NativeTextBindingError('competition needs a distinct Alignment and exact shared source comparison scope')
    if change == 'initial' and body['competing_records']:
        raise NativeTextBindingError('initial record cannot conceal an explicit competing introduction')
    if verify_content:
        # All validated ancestors/alternatives now bind these identical units;
        # one exact verification covers their shared native bytes, not their
        # supplied translation or qualification judgments.
        for role in ('source', 'target'):
            source_side(resolver, body['native_bindings'][role], role,
                tokenization=body[role + '_side']['tokenization'] is not None, verify_content=True)
    resolver.snapshot()
    return {'metadata_verified': True, 'content_verified': verify_content,
        'history_verified': True, 'assessment_applied': False, 'grants_admission': False,
        'content_disclosure': 'withheld'}


def configuration(config, *, owner_config):
    """Select independent bounded authority; describe never opens source text."""
    source._keys(config, {'schema_version', 'uid', 'principal_id', 'authority_ref', 'expires_at',
        'source_context_ref', 'source_path', 'allowed_operations', 'source_access', 'alignment_access',
        'record_id', 'alignment_id', 'claim_id', 'provenance_event_id', 'change_kind', 'predecessor',
        'competing_records', 'native_bindings', 'granularity', 'tokenization', 'maker'})
    raw = private_read(Path(owner_config), source.MAX_COMMAND_BYTES, confidential_file=True)
    if source._json_object(raw) != config:
        raise source.JournalConflict('native alignment delegation changed during selection')
    change = config['change_kind']
    operation = 'alignment.create' if change in {'initial', 'competing'} else 'alignment.revise'
    if (config['schema_version'] != CONFIG or type(config['uid']) is not int or config['uid'] != os.getuid()
            or change not in {'initial', 'competing', 'describe', 'remap'}
            or config['allowed_operations'] != [operation]
            or source._instant(config['expires_at']) <= datetime.now(timezone.utc)
            or any(not isinstance(config[key], str) or not config[key].strip() for key in ('principal_id', 'authority_ref'))):
        raise PermissionError('native alignment operation is not currently delegated')
    access, alignment = config['source_access'], config['alignment_access']
    layers._grant(access, {'read_scope', 'access_allowed'})
    layers._grant(alignment, {'derivation_allowed'})
    if (access['read_scope'] != 'exact_owner_local' or access['access_allowed'] is not True
            or alignment['derivation_allowed'] is not True):
        raise PermissionError('native alignment needs separate exact-source reading and proposal authority')
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    path = context.path(config['source_path'])
    if (context.role(config['source_path']) != 'owner-local-root' or path.name != BASENAME
            or len(Path(config['source_path']).parts) < 7
            or any(part in {'payload', 'local-content', 'catalog'} or part.startswith('.')
                   for part in Path(config['source_path']).parts)):
        raise PermissionError('native alignment requires an immutable private owner package')
    units._private_directory(context, path.parent.parent)
    for key, kind in (('record_id', 'translation-alignment-record'), ('alignment_id', 'translation-alignment'),
                      ('claim_id', 'translation-alignment-claim'), ('provenance_event_id', 'event')):
        pattern = r'tos\.' + kind + (r'\.sid-[a-f0-9]{32}' if key != 'provenance_event_id'
                                    else r'\.[a-z0-9]+(?:[.-][a-z0-9]+)*')
        if not isinstance(config[key], str) or not re.fullmatch(pattern, config[key]):
            raise ValueError('native alignment requires exact independently delegated opaque identities')
    source._keys(config['tokenization'], {'source', 'target'})
    source._keys(config['native_bindings'], {'source', 'target'})
    if any(type(config['tokenization'][role]) is not bool for role in ('source', 'target')):
        raise ValueError('native tokenization use must be explicitly selected for each side')
    if (change in {'initial', 'competing'}) != (config['predecessor'] is None):
        raise ValueError('native alignment predecessor does not match the delegated change kind')
    resolver = NativeTextBindingResolver(context.public_root, owner_context=context, read_bytes=source._read)
    validator, schemas = _validator(resolver)
    legacy = schemas[LEGACY_SCHEMA]
    for key in ('record_id', 'alignment_id', 'predecessor', 'competing_records', 'native_bindings', 'granularity'):
        field_validator = validator.evolve(schema={'$ref': schemas[SCHEMA]['$id'] + '#/properties/' + key})
        if not field_validator.is_valid(config[key]):
            raise ValueError('native alignment delegation violates its selected source field grammar')
    maker_validator = Draft202012Validator({'$ref': '#/$defs/maker', '$defs': legacy['$defs']}, format_checker=FormatChecker())
    maker = config['maker']
    if (not maker_validator.is_valid(maker) or maker['maker_kind'] != 'imported_source'
            or maker['agent_ref'] != config['principal_id'] or maker['provenance_event_ref'] != config['provenance_event_id']):
        raise PermissionError('native mapping maker must identify supplied attribution, not an executed aligner')
    resolver._read(units.PROVENANCE_SCHEMA, schema=True)
    digest = source._digest(_canonical({'owner_configuration_bytes': source._digest(raw),
        'context': context.snapshot(), 'contracts': resolver.schema_digests}))
    return config, digest, path


def _history_refs(resolver, body):
    """Exact owner ancestry only; alternatives never license identity reuse."""
    refs, current = {}, body['predecessor']
    while current is not None:
        if current['record_ref'] in refs or len(refs) >= MAX_HISTORY:
            raise NativeTextBindingError('native record history repeats or exceeds its bound')
        raw = resolver._read(current['record_ref'], expected=current['sha256'])
        previous = source._json_object(raw)
        if record_ref(current['record_ref'], previous, raw) != current:
            raise NativeTextBindingError('native history does not bind its exact predecessor')
        refs[current['record_ref']] = current['sha256']
        current = previous['predecessor']
    return refs


def _identity_inventory(context, body, config, resolver, *, exclude=None):
    """Use the existing bounded source walk, not a new alignment registry.

    A new version may reuse only IDs belonging to its exact ancestry. A
    sibling/future version or another same-ID path prevents stale forks.
    """
    patterns = ('*translation-alignment*.json',)
    paths = units._inventory_paths(context, exclude=exclude, additional_patterns=patterns)
    inputs, remaining, count = {}, units.MAX_INVENTORY_BYTES, 0
    reserved = {body['record_id'], body['alignment_id'], body['claim']['claim_id'], config['provenance_event_id']}
    ancestry = _history_refs(resolver, body)
    for ref, path in paths:
        if 'translation-alignment' not in path.name and not ('provenance' in path.name and path.suffix == '.jsonl'):
            continue
        raw = context.read_bytes(path, min(units.MAX_INVENTORY_FILE_BYTES, remaining), read_bytes=resolver._reader)
        remaining -= len(raw)
        inputs[ref] = source._digest(raw)
        for line in raw.splitlines() if path.suffix == '.jsonl' else [raw]:
            if not line.strip():
                continue
            count += 1
            if count > units.MAX_INVENTORY_RECORDS:
                raise ValueError('native alignment identity record budget exceeded')
            value = source._json_object(line)
            if path.suffix == '.jsonl':
                owned = {value.get('event_id')}
            elif value.get('schema_version') == SCHEMA_VERSION:
                owned = {value.get('record_id'), value.get('alignment_id'), value.get('claim', {}).get('claim_id')}
            elif value.get('schema_version') == 'tos_translation_alignment_packet_v1':
                owned = {value.get('packet_id')}
                for row in value.get('alignments', []):
                    owned.update((row.get('alignment_id'), row.get('claim_id')))
            else:
                raise ValueError('alignment identity owner has an unsupported record shape')
            if exclude is None and reserved & owned and ancestry.get(ref) != source._digest(raw)[7:]:
                raise source.JournalConflict('native alignment identity is occupied outside its exact predecessor chain')
    if paths != units._inventory_paths(context, exclude=exclude, additional_patterns=patterns):
        raise source.JournalConflict('native alignment identity membership changed during preparation')
    return source._digest(_canonical(inputs))


def _prepare(config, proposal, *, exclude=None, owner_config=None, deadline=None):
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    resolver = _resolver(config, context, owner_config=owner_config, deadline=deadline)
    _, schemas = _validator(resolver)
    sides = {role: source_side(resolver, config['native_bindings'][role], role,
                              tokenization=config['tokenization'][role]) for role in ('source', 'target')}
    previous = None
    if config['predecessor'] is not None:
        target = config['predecessor']
        raw = resolver._read(target['record_ref'], expected=target['sha256'])
        previous = source._json_object(raw)
        if record_ref(target['record_ref'], previous, raw) != target:
            raise NativeTextBindingError('delegated predecessor differs from its exact owner record')
    q = proposal['qualifications']
    source._keys(q, {'translation_techniques', 'epistemic_status', 'certainty', 'status_reason', 'evidence'})
    body = {'$schema': schemas[SCHEMA]['$id'], 'schema_version': SCHEMA_VERSION,
        'record_id': config['record_id'], 'record_version': previous['record_version'] + 1 if previous else 1,
        'predecessor': config['predecessor'], 'change_kind': config['change_kind'],
        'alignment_id': config['alignment_id'], 'granularity': config['granularity'],
        'claim': {'claim_id': config['claim_id'],
            'claim_version': previous['claim']['claim_version'] + 1 if config['change_kind'] == 'describe' else 1,
            'predecessor': claim_ref(previous['claim']) if previous else None,
            'mapping': copy.deepcopy(proposal['mapping']), 'qualifications': {**copy.deepcopy(q), 'maker': copy.deepcopy(config['maker'])}},
        'source_side': sides['source'], 'target_side': sides['target'],
        'native_bindings': copy.deepcopy(config['native_bindings']),
        'competing_records': copy.deepcopy(config['competing_records']),
        'rights_and_visibility': rights_for(sides['source'], sides['target']),
        'status': 'proposed', 'execution_posture': 'supplied_mapping_captured_exact_sources_verified_not_aligner_execution',
        'assessment_posture': 'unassessed_translation_proposal', 'publication_authorized': False,
        'authority_boundary': {key: value['const'] for key, value in schemas[LEGACY_SCHEMA]['$defs']['authorityBoundary']['properties'].items()}}
    validate_record(resolver, body, verify_content=True)
    source.Record.from_payload(body['record_id'], body['record_version'], body)
    identity = _identity_inventory(context, body, config, resolver, exclude=exclude)
    implementations = {ref: source._digest(source._read(source.ROOT / ref, source.MAX_SET_BYTES)) for ref in IMPLEMENTATIONS}
    inputs = {'schema_version': 'tos_native_construction_inputs_v1', 'context': context.snapshot(),
        'inputs': [[ref, category, digest] for (ref, category), digest in sorted(resolver._inputs.items())],
        'implementation': implementations, 'runtime': layers._runtime()}
    files = {BASENAME: layers._encoded(body), CONFIG_FILE: layers._encoded(config), INPUT_FILE: layers._encoded(inputs)}
    dependencies = source._digest(_canonical({'native': resolver.snapshot(), 'identity': identity,
        'retained_inputs': source._digest(files[INPUT_FILE])}))
    native_inputs = {'native_alignment': True, 'event_type': 'annotation', 'rights': [], 'entities': []}
    rights = {}
    for role in ('source', 'target'):
        binding = config['native_bindings'][role][0]
        layer = resolver._record(binding['text_layer']['record_ref'])
        for row in layer['representation']['rights_record_refs']:
            rights[row['ref']] = row
    native_inputs['rights'] = [rights[ref] for ref in sorted(rights)]
    for (ref, category), digest in sorted(resolver._inputs.items()):
        if category == 'content' or ref in {binding['packet_ref'] for role in ('source', 'target') for binding in config['native_bindings'][role]}:
            raw = resolver._cache[(ref, category)]
            native_inputs['entities'].append({'entity_ref': ref, 'role': 'verified-exact-native-alignment-input',
                'sha256': digest, 'size_bytes': len(raw), 'media_type': 'text/plain; charset=utf-8' if category == 'content' else 'application/json',
                'availability': 'owner_local', 'content_disclosure': 'private_content',
                'fixity_verified': True, 'fixity_verified_at': datetime.now(timezone.utc).isoformat()})
    return body, files, dependencies, native_inputs


def _verify_package(files, *, config, configuration_digest, request, exclude=None, owner_config=None, deadline=None):
    # Earn target exclusion by verifying the entire retained receipt first.
    # A verified old immutable package can be replayed after a later version;
    # replay does not reserve its already-owned IDs as a fresh introduction.
    retained_body = source._json_object(files.get(BASENAME, b'{}'))
    receipt = layers._verify_receipt(files, config=config, configuration_digest=configuration_digest,
        request=request, source_id=retained_body['record_id'], source_version=retained_body['record_version'])
    body, expected, dependencies, _ = _prepare(config, request, exclude=exclude,
                                            owner_config=owner_config, deadline=deadline)
    required = set(expected) | {RECEIPT_FILE, 'source-create-request.json', 'source-create-environment.json', 'source-create-provenance.jsonl'}
    if set(files) != required or any(files.get(name) != raw for name, raw in expected.items()):
        raise source.JournalConflict('native alignment retry differs from retained output or exact source dependencies')
    # Creation receipt envelope describes this new immutable version package;
    # descriptive predecessor identity remains in the exact native record.
    return receipt, dependencies


def _selected_records(config, context, path, *, owner_config=None, deadline=None):
    """Read only the delegated target and its exact ancestry, no arbitrary ref."""
    resolver = _resolver(config, context, owner_config=owner_config, deadline=deadline)
    if os.path.lexists(path.parent):
        files = layers._private_package(context, path.parent)
        request = source._json_object(files.get('source-create-request.json', b'{}'))
        body, raw = source._json_object(files[BASENAME]), files[BASENAME]
        retained_config = source._json_object(files[CONFIG_FILE])
        receipt = source._json_object(files[RECEIPT_FILE])
        layers._verify_receipt(files, config=retained_config, configuration_digest=receipt['owner_configuration'],
            request=request, source_id=body['record_id'], source_version=body['record_version'])
        if (retained_config['source_path'] != config['source_path'] or body['record_id'] != config['record_id']
                or body['alignment_id'] != config['alignment_id'] or body['native_bindings'] != config['native_bindings']):
            raise PermissionError('inspection target differs from its independent native source scope')
        selected = record_ref(config['source_path'], body, raw)
    else:
        selected = config['predecessor']
        if selected is None:
            raise source.JournalConflict('no selected native alignment record exists')
        raw = resolver._read(selected['record_ref'], expected=selected['sha256'])
        body = source._json_object(raw)
    if (record_ref(selected['record_ref'], body, raw) != selected
            or body['record_id'] != config['record_id'] or body['alignment_id'] != config['alignment_id']
            or body['native_bindings'] != config['native_bindings']):
        raise PermissionError('inspection predecessor differs from its exact delegated identity, version or native scope')
    validate_record(resolver, body)
    records = {selected['record_ref']: (selected, body)}
    while body['predecessor'] is not None:
        selected = body['predecessor']
        raw = resolver._read(selected['record_ref'], expected=selected['sha256'])
        body = source._json_object(raw)
        records[selected['record_ref']] = (selected, body)
    return resolver, records


def run_command(owner_config, config, configuration_digest, path, request):
    source.command_handler(CONFIG).validate_request(request)
    operation, delegated = request['operation'], config['allowed_operations'][0]
    prepare_operation = 'prepare-create' if delegated == 'alignment.create' else 'prepare-revise'
    if operation not in {'describe', prepare_operation, delegated, 'inspect', 'inspect-version', 'inspect-recovery'}:
        raise PermissionError('native alignment operation is outside the exact delegated change kind')
    context = OwnerLocalSourceContext.load(config['source_context_ref'])
    target = path.parent
    deadline = time.monotonic() + MAX_SECONDS
    guarded = {'owner_config': owner_config, 'deadline': deadline}
    def result(receipt=None, replayed=False):
        return {'schema_version': 'tos_local_native_alignment_result_v1',
            'authentication': 'local-unix-account', 'owner_configuration': configuration_digest,
            'target_exists': os.path.lexists(target), 'expected_source': None, 'expected_revision': None,
            'supported_operations': [delegated],
            'command_operations': ['describe', prepare_operation, delegated, 'inspect', 'inspect-version', 'inspect-recovery'],
            'receipt_sha256': source._digest(_canonical(receipt)) if receipt is not None else None,
            'replayed': replayed, 'content_disclosure': 'withheld', 'grants_admission': False,
            'assessment_applied': False, 'aligner_executed': False}
    if operation == 'describe':
        return result()
    if operation in {'inspect', 'inspect-version'}:
        resolver, records = _selected_records(config, context, path, **guarded)
        selected = next(iter(records.values()))
        if operation == 'inspect-version':
            matches = [row for row in records.values() if row[0] == request['source']]
            if len(matches) != 1:
                raise PermissionError('inspection does not select an exact version of the delegated native history')
            selected = matches[0]
        ref, body = selected
        resolver.snapshot()
        return {**result(), 'inspected_source_sha256': ref['sha256'], 'record_version': body['record_version'],
            'claim_version': body['claim']['claim_version'], 'change_kind': body['change_kind'],
            'history_depth': len(records), 'metadata_verified': True, 'content_verified': False,
            'mapping_summary': {'granularity': body['granularity'],
                'correspondence_shape': body['claim']['mapping']['correspondence_shape'],
                'order_posture': body['claim']['mapping']['order_posture'],
                'source_member_count': len(body['claim']['mapping']['ordered_source_anchor_refs']),
                'target_member_count': len(body['claim']['mapping']['ordered_target_anchor_refs'])},
            'competition_forward_count': len(body['competing_records']),
            'reverse_competition_posture': 'derived_from_explicit_external_record_refs_not_claimed_complete'}
    if operation == 'inspect-recovery':
        command_id = request['command_id']
        if not isinstance(command_id, str) or not 1 <= len(command_id) <= 256:
            raise ValueError('native alignment command identity must be bounded')
        control = layers._control_path(context, target, request)
        if not os.path.lexists(control):
            return {**result(), 'recovery_state': 'absent'}
        plan, files = layers._control_files(context, control)
        if plan['target_ref'] != target.relative_to(context.private_root).as_posix():
            raise source.JournalConflict('native alignment recovery belongs to another target')
        retained_request = source._json_object(files['source-create-request.json'])
        if (retained_request['command_id'] != command_id
                or plan['request_digest'] != source._digest(_canonical(retained_request))):
            raise source.JournalConflict('native alignment recovery plan does not bind the selected exact command')
        _verify_package(files, config=config, configuration_digest=configuration_digest,
            request=retained_request, exclude=target if os.path.lexists(target) else None, **guarded)
        return {**result(), 'recovery_state': 'committed' if os.path.lexists(target) else 'retained_exact_plan',
            'recovery_action': 'retry_exact_original_command; torn_or_foreign_stage_requires_owner_review'}
    if operation == prepare_operation:
        _, _, dependencies, _ = _prepare(config, request, **guarded)
        return {**result(), 'expected_dependencies': dependencies}
    if not isinstance(request['command_id'], str) or not 1 <= len(request['command_id']) <= 256:
        raise ValueError('native alignment command identity must be bounded')
    with source._locked(context.public_root / 'ToS/source-witnesses/historical-create'), source._locked(context.private_root / 'native-create'):
        os.close(_open(context.private_root / '.native-create.writer.lock', private_root=context.private_root))
        if source._configuration(owner_config)[1:] != (configuration_digest, path):
            raise source.JournalConflict('native alignment delegation changed before construction')
        if os.path.lexists(target):
            files = layers._private_package(context, target)
            receipt, dependencies = _verify_package(files, config=config, configuration_digest=configuration_digest,
                                                   request=request, exclude=target, **guarded)
            if (_prepare(config, request, exclude=target, **guarded)[2] != dependencies
                    or layers._private_package(context, target) != files
                    or source._configuration(owner_config)[1:] != (configuration_digest, path)):
                raise source.JournalConflict('native alignment dependencies changed during exact replay')
            return result(receipt, True)
        if (request['expected_configuration'] != configuration_digest or request['expected_source'] is not None
                or request['expected_revision'] is not None):
            raise source.JournalConflict('native alignment version requires an exact grant and absent immutable package')
        started_at, started_ns = datetime.now(timezone.utc).isoformat(), time.perf_counter_ns()
        body, files, dependencies, inputs = _prepare(config, request, **guarded)
        if dependencies != request['expected_dependencies']:
            raise source.JournalConflict('prepared native alignment dependencies changed')
        source._capture_creation_provenance({**config, 'source_root': str(context.public_root)}, request,
            files, started_at, started_ns, procedure_name='native-supplied-alignment-proposal-capture',
            additional_software_refs=IMPLEMENTATIONS[-1:], native_inputs=inputs)
        subject = source.Record.from_payload(body['record_id'], body['record_version'], body)
        receipt = {'schema_version': 'tos_local_source_create_receipt_v1', 'command_id': request['command_id'],
            'request_digest': source._digest(_canonical(request)), 'principal_id': config['principal_id'],
            'authority_ref': config['authority_ref'], 'owner_configuration': configuration_digest,
            'recorded_at': datetime.now(timezone.utc).isoformat(), 'source_path': config['source_path'],
            'source': subject.ref, 'dependencies': dependencies, 'files': _file_refs(files), 'grants_admission': False}
        files[RECEIPT_FILE] = layers._encoded(receipt)
        def guard():
            if (_prepare(config, request, **guarded)[2] != dependencies
                    or source._configuration(owner_config)[1:] != (configuration_digest, path)
                    or context.snapshot() != OwnerLocalSourceContext.load(config['source_context_ref']).snapshot()):
                raise source.JournalConflict('native alignment inputs, current rights or grant changed before commit')
        def verify_retained(retained):
            _verify_package(retained, config=config, configuration_digest=configuration_digest, request=request, **guarded)
        retained = layers.install_private_package(context, target, files, request=request,
                                                  guard=guard, verify_retained=verify_retained)
        return result(source._json_object(retained[RECEIPT_FILE]))


def command_handlers():
    proposal = {'mapping', 'qualifications'}
    return (contract.Handler('owner-local-native-translation-alignment', (CONFIG,), (contract.describe(),
        contract.operation('prepare-create', proposal, definition='Prepare a supplied alignment or competing proposal against two exact native source closures.', grants=('alignment.create',)),
        contract.operation('prepare-revise', proposal, definition='Prepare a descriptive successor or separately identified remapped Claim without rekeying its Alignment.', grants=('alignment.revise',)),
        *(contract.operation(name, proposal | contract.COMMIT_KEYS,
            definition='Create an immutable private source version; exact retry resumes retained staging and never executes an aligner or assesses translation.',
            mutation='private_native_alignment_version_package', grants=(name,)) for name in ('alignment.create', 'alignment.revise')),
        contract.operation('inspect', definition='Inspect the exact delegated native alignment and its source-visible metadata closure without returning private source bytes.'),
        contract.inspect_version(),
        contract.operation('inspect-recovery', {'command_id'}, definition='Inspect the selected command retained recovery plan; exact original request resumes it.')),
        run_command, 'Versioned native records owned by the existing translation-alignment source contract, not witness collation or semantic admission.',
        configure=configuration, typed_handles=tuple('ToS/contracts/' + name for name in CONTRACTS),
        profile_selection='A protected owner grant selects one change kind, both native sides, stable subject/Claim IDs, exact predecessor and new immutable private destination.',
        preconditions=('No discovery or describe read opens source text; both recorded rights gates precede exact representation reads.',
            'Source/target selections, metadata descriptions and supplied mapping never establish an executed aligner, translation quality, publication or canon.',
            'Legacy translation packet-v1 and human review semantics remain unchanged; native wrappers are unassessed proposals only.')),)

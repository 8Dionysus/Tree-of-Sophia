"""Meaning-preserving identity proposal checks, without identity mutation.

The fixed slots below are the complete dependency grammar. Arbitrary prose is
never mined for references. MetadataVersionReader owns exact endpoint reading;
its owner-derived descriptor, not an ID prefix or submitted type tag, establishes
that an endpoint has a supported source representation.
"""
from __future__ import annotations
from pathlib import PurePosixPath
import re

READER = 'identity-transition-v1'
PREDICATE = 'identity_transition_proposal'
CREATE_CONFIG = 'tos_local_identity_proposal_create_owner_v1'
REVISION_CONFIG = 'tos_local_identity_proposal_revision_owner_v1'
SCHEMA_REF = 'ToS/contracts/source-identity-transition-claim.schema.json'
READER_V2 = 'identity-transition-v2'
PREDICATE_V2 = 'subject_identity_transition_proposal'
CREATE_CONFIG_V2 = 'tos_local_identity_proposal_create_owner_v2'
REVISION_CONFIG_V2 = 'tos_local_identity_proposal_revision_owner_v2'
SCHEMA_REF_V2 = 'ToS/contracts/subject-identity-transition-claim.schema.json'
SEMANTIC_ADAPTER = 'exact-semantic-metadata-v1'
READERS = frozenset({READER, READER_V2})
PREDICATES = frozenset({PREDICATE, PREDICATE_V2})
CREATE_CONFIGS = frozenset({CREATE_CONFIG, CREATE_CONFIG_V2})
REVISION_CONFIGS = frozenset({REVISION_CONFIG, REVISION_CONFIG_V2})
CONFIGS = CREATE_CONFIGS | REVISION_CONFIGS
CONFIG_READERS = {CREATE_CONFIG: READER, REVISION_CONFIG: READER,
                  CREATE_CONFIG_V2: READER_V2, REVISION_CONFIG_V2: READER_V2}
READER_PREDICATES = {READER: PREDICATE, READER_V2: PREDICATE_V2}
READER_SCHEMAS = {READER: ('tos_source_identity_transition_claim_v1', SCHEMA_REF),
                  READER_V2: ('tos_subject_identity_transition_claim_v1', SCHEMA_REF_V2)}
MODULE_REF = 'scripts/source_identity_proposals.py'
FROZEN = ('kind', 'operation', 'members', 'predecessors', 'successors', 'mapping', 'supersedes_proposal')


def exact_ref(value, *, claim=False):
    return (isinstance(value, dict) and set(value) == {'id', 'version', 'digest'}
        and isinstance(value['id'], str) and re.fullmatch(
            r'tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*' if claim else
            r'tos\.[a-z][a-z0-9-]*\.[a-z0-9]+(?:[.-][a-z0-9]+)*', value['id']) is not None
        and type(value['version']) is int and 1 <= value['version'] <= 9_007_199_254_740_991
        and isinstance(value['digest'], str) and re.fullmatch(r'sha256:[a-f0-9]{64}', value['digest']) is not None)


def related_claims(claim):
    value = claim.get('object')
    if not isinstance(value, dict) or 'supersedes_proposal' not in value:
        raise ValueError('proposal requires an explicit predecessor proposal or null')
    unresolved = value.get('unresolved_links')
    if (not isinstance(unresolved, list) or len(unresolved) > 32
            or any(not isinstance(item, dict) or not exact_ref(item.get('claim'), claim=True) for item in unresolved)):
        raise ValueError('unresolved links require bounded exact Claim references')
    previous = value['supersedes_proposal']
    if previous is not None and not exact_ref(previous, claim=True):
        raise ValueError('predecessor proposal requires an exact Claim reference')
    return tuple([item['claim'] for item in unresolved] + ([previous] if previous is not None else []))


def participants(claim):
    """Require a complete N-to-one or one-to-N proposed mapping, never aliases."""
    value = claim.get('object')
    if not isinstance(value, dict):
        raise ValueError('proposal requires its whole structured plan')
    predecessors, successors = value.get('predecessors'), value.get('successors')
    if (any(not isinstance(refs, list) or not 1 <= len(refs) <= 8
            or any(not exact_ref(ref) for ref in refs) for refs in (predecessors, successors))
            or not isinstance(value.get('members'), list) or not 3 <= len(value['members']) <= 9
            or any(not isinstance(identity, str) for identity in value['members'])
            or not isinstance(value.get('mapping'), list) or not 2 <= len(value['mapping']) <= 8
            or any(not isinstance(edge, dict) or set(edge) != {'predecessor', 'successor'}
                   or any(not isinstance(identity, str) for identity in edge.values()) for edge in value['mapping'])):
        raise ValueError('proposal participants and mapping require their bounded exact shape')
    related_claims(claim)
    left, right = [ref['id'] for ref in predecessors], [ref['id'] for ref in successors]
    if (len(set(left + right)) != len(left + right)
            or set(value['members']) != set(left + right)
            or len(value['members']) != len(left + right)
            or claim['subject_ref'] not in left
            or claim['claim_id'] in left + right):
        raise ValueError('proposal requires distinct frozen participants and a predecessor focal subject')
    if not ((value.get('operation') == 'merge' and 2 <= len(left) <= 8 and len(right) == 1)
            or (value.get('operation') == 'split' and len(left) == 1 and 2 <= len(right) <= 8)):
        raise ValueError('proposal topology must be merge N-to-one or split one-to-N')
    expected = {(old, new) for old in left for new in right}
    actual = [(edge['predecessor'], edge['successor']) for edge in value['mapping']]
    if set(actual) != expected or len(actual) != len(expected):
        raise ValueError('proposal mapping must cover every declared predecessor and successor exactly')
    previous = value['supersedes_proposal']
    if claim.get('supersedes_claim_ref') != (previous['id'] if previous is not None else None):
        raise ValueError('proposal succession navigation must match the exact predecessor assertion')
    if previous is not None and previous['id'] == claim['claim_id']:
        raise ValueError('proposal succession requires a distinct assertion identity')
    if any(item['claim']['id'] == claim['claim_id'] for item in value['unresolved_links']):
        raise ValueError('proposal cannot use itself as a pre-existing unresolved link')
    return tuple(predecessors + successors)


def preserve_topology(previous, following):
    value = following.get('object')
    if not isinstance(value, dict) or any(field not in value or previous['object'][field] != value[field] for field in FROZEN):
        raise PermissionError('changed participants, topology or predecessor proposal require a new Claim identity')


def semantic_profile_eligible(entry):
    """Explicit exact-subject adapter, never blanket semantic ancestry.

    Registry/schema loaders validate the complete descriptor. These additional
    invariant checks also protect capability consumers from a forged role,
    reader, identity shape or mapping. No field selects executable code.
    """
    if not isinstance(entry, dict):
        return False
    profile = entry.get('source_record_profile')
    if (entry.get('abstract') is not False or entry.get('object_role') != 'semantic'
            or not isinstance(profile, dict) or profile.get('reader') != 'semantic-metadata-v1'
            or profile.get('identity_proposal_adapter') != SEMANTIC_ADAPTER
            or profile.get('graph_layer') != 'source-profile'):
        return False
    kind = profile.get('record_type')
    if (not isinstance(kind, str) or not re.fullmatch(r'[a-z][a-z0-9]*(?:-[a-z0-9]+)*', kind)
            or kind in {'claim', 'literal', 'temporal-assertion'}
            or profile.get('id_prefix') != 'tos.' + kind + '.'
            or profile.get('source_basename') != kind + '.json'
            or not isinstance(profile.get('schemas'), list) or not profile['schemas']):
        return False
    mappings = entry.get('source_mappings')
    return isinstance(mappings, list) and all(sum(
        isinstance(mapping, dict) and mapping.get('source_graph') == graph
        and mapping.get('source_kind_id') == kind for mapping in mappings) == 1
        for graph in ('source-claims', 'source-navigation'))


def eligible_type(entities, type_id, *, reader=READER):
    """V1 is unchanged; V2 additionally requires the declared semantic adapter."""
    entry = entities.get(type_id)
    if reader not in READERS or not isinstance(entry, dict) or entry.get('abstract') is not False:
        return False
    if entry.get('object_role') == 'identity':
        return any(mapping.get('source_graph') == 'source-claims' for mapping in entry.get('source_mappings', []))
    return reader == READER_V2 and semantic_profile_eligible(entry)


def reader_for_claim(claim):
    for reader, predicate in READER_PREDICATES.items():
        if claim.get('predicate') == predicate and claim.get('schema_version') == READER_SCHEMAS[reader][0]:
            return reader
    raise ValueError('identity proposal requires its exact versioned predicate and schema')


def predecessor_allowed(claim, previous):
    """A V2 successor may cite V1; old V1 never inherits the wider route."""
    reader = reader_for_claim(claim)
    try:
        previous_reader = reader_for_claim(previous)
    except ValueError:
        return False
    return previous_reader in ({READER} if reader == READER else READERS)


def _semantic_descriptor_matches(entry, reference, view):
    profile, descriptor, record = entry['source_record_profile'], view.get('descriptor'), view.get('record')
    if not isinstance(descriptor, dict) or not isinstance(record, dict):
        return False
    schema = record.get('schema_version')
    routes = [route for route in profile['schemas'] if route.get('schema_version') == schema]
    if len(routes) != 1:
        return False
    expected = {'adapter': 'declared-profile', 'record_kind': 'subject', 'identity_field': 'record_id',
        'record_type': profile['record_type'], 'type_id': entry['type_id'], 'profile_type_id': entry['type_id'],
        'source_basename': profile['source_basename'], 'source_scope': 'public_metadata_only',
        'schema_version': schema, 'source_schema_version': schema,
        'schema_ref': routes[0]['schema_ref'], 'source_schema_ref': routes[0]['schema_ref']}
    if (any(descriptor.get(key) != value for key, value in expected.items())
            or record.get('record_id') != reference['id'] or record.get('record_version') != reference['version']
            or record.get('record_type') != profile['record_type']
            or record.get('visibility') not in {'public', 'public_metadata_only'}
            or view.get('exact_ref') != reference or view.get('record_digest') != reference['digest']):
        return False
    provenance = view.get('provenance')
    if not isinstance(provenance, dict) or not isinstance(provenance.get('source'), dict):
        return False
    source_ref = provenance['source'].get('source_ref')
    if not isinstance(source_ref, str) or '\\' in source_ref or '\x00' in source_ref:
        return False
    path = PurePosixPath(source_ref)
    return (path.as_posix() == source_ref and not path.is_absolute() and len(path.parts) >= 5
        and path.parts[:2] == ('ToS', 'source-witnesses') and path.name == profile['source_basename']
        and not any(part in {'catalog', 'payload', 'private', 'local-content', 'owner-local'}
                    or part.startswith('.') for part in path.parts))


def validate_assessment_scope(claim, scope):
    if claim.get('predicate') in PREDICATES and (scope.get('risk'), scope.get('requested_use')) != ('high', 'research'):
        raise PermissionError('identity proposals require high-risk research assessment; execution is not a use grant')


def ground(claim, profiles, metadata_reader, claim_reader):
    """Exact source-visible closure; research assessment never executes a plan."""
    reader = reader_for_claim(claim)
    bindings = {'participants': [], 'claims': []}
    for ref in participants(claim):
        view = metadata_reader.resolve_typed(ref)
        descriptor = view.get('descriptor')
        if (view.get('status') != 'available' or not isinstance(descriptor, dict)
                or not eligible_type(profiles.entities, descriptor.get('type_id'), reader=reader)):
            raise ValueError('proposal endpoint lacks available exact source identity capability')
        entry = profiles.entities[descriptor['type_id']]
        if entry['object_role'] == 'semantic' and not _semantic_descriptor_matches(entry, ref, view):
            raise ValueError('proposal semantic endpoint lacks its exact declared subject descriptor')
        bindings['participants'].append({'ref': ref, 'descriptor': descriptor,
                                         'provenance': view.get('provenance')})
    previous = claim['object']['supersedes_proposal']
    for ref in related_claims(claim):
        view = claim_reader.resolve(ref)
        if view.get('status') != 'available':
            raise ValueError('proposal referenced Claim version is not available')
        if ref == previous:
            if not predecessor_allowed(claim, view.get('record', {})):
                raise ValueError('proposal predecessor must itself be an identity proposal')
            profiles.validate(view['record'])
        bindings['claims'].append({'ref': ref, 'provenance': view.get('provenance')})
    metadata_reader.verify_current()
    claim_reader.verify_current()
    return bindings

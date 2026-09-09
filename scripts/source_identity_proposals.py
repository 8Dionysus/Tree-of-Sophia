"""Meaning-preserving identity proposal checks, without identity mutation.

The fixed slots below are the complete dependency grammar. Arbitrary prose is
never mined for references. MetadataVersionReader owns exact endpoint reading;
its owner-derived descriptor, not an ID prefix or submitted type tag, establishes
that an endpoint has a supported source representation.
"""
from __future__ import annotations
import re

READER = 'identity-transition-v1'
PREDICATE = 'identity_transition_proposal'
CREATE_CONFIG = 'tos_local_identity_proposal_create_owner_v1'
REVISION_CONFIG = 'tos_local_identity_proposal_revision_owner_v1'
SCHEMA_REF = 'ToS/contracts/source-identity-transition-claim.schema.json'
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


def eligible_type(entities, type_id):
    """Concrete source identity role is a capability, not broad ancestry fallback."""
    entry = entities.get(type_id)
    return bool(entry and not entry['abstract'] and entry['object_role'] == 'identity'
                and any(mapping['source_graph'] == 'source-claims' for mapping in entry['source_mappings']))


def validate_assessment_scope(claim, scope):
    if claim.get('predicate') == PREDICATE and (scope.get('risk'), scope.get('requested_use')) != ('high', 'research'):
        raise PermissionError('identity proposals require high-risk research assessment; execution is not a use grant')


def ground(claim, profiles, metadata_reader, claim_reader):
    """Exact source-visible closure; research assessment never executes a plan."""
    bindings = {'participants': [], 'claims': []}
    for ref in participants(claim):
        view = metadata_reader.resolve_typed(ref)
        descriptor = view.get('descriptor')
        if (view.get('status') != 'available' or not isinstance(descriptor, dict)
                or not eligible_type(profiles.entities, descriptor.get('type_id'))):
            raise ValueError('proposal endpoint lacks available exact source identity capability')
        bindings['participants'].append({'ref': ref, 'descriptor': descriptor,
                                         'provenance': view.get('provenance')})
    previous = claim['object']['supersedes_proposal']
    for ref in related_claims(claim):
        view = claim_reader.resolve(ref)
        if view.get('status') != 'available':
            raise ValueError('proposal referenced Claim version is not available')
        if ref == previous:
            if view.get('record', {}).get('predicate') != PREDICATE:
                raise ValueError('proposal predecessor must itself be an identity proposal')
            profiles.validate(view['record'])
        bindings['claims'].append({'ref': ref, 'provenance': view.get('provenance')})
    metadata_reader.verify_current()
    claim_reader.verify_current()
    return bindings

"""Compare two exact, source-declared date envelopes, never historical truth.

The owner normalizer supplies comparison keys. This reader does not parse
prose, convert calendars, follow relative anchors or estimate uncertainty.
Only exact index lookups are used after the shared graph index is prepared.
"""
from __future__ import annotations

import copy
import math
import re

from .lens_pagination import KnowledgeRevisionConflict


REQUEST_SCHEMA = 'tos_temporal_comparison_request_v1'
RESULT_SCHEMA = 'tos_temporal_comparison_result_v1'
SUPPORTED_TIME_ROLES = frozenset({'historical-time'})


class TemporalReadModelInvalid(RuntimeError):
    """The selected projection cannot supply a structurally valid operand."""


BOUNDARY = {
    'is_source': False, 'writes_to_tree': False, 'performs_assessment': False,
    'creates_inferred_claim': False,
    'comparison_basis': 'normalized-source-date-envelopes',
    'note': 'Relations describe date envelopes only. They do not establish event '
            'simultaneity, duration, causality, identity or the truth/admission of either Claim. '
            'Numeric keys are ordering keys, not timestamps or elapsed-time quantities.',
}


def normalize_temporal_comparison_request(value):
    if not isinstance(value, dict) or set(value) != {'schema_version', 'source_revision', 'left', 'right'}:
        raise ValueError('temporal comparison requires schema_version, source_revision, left and right only')
    if value['schema_version'] != REQUEST_SCHEMA:
        raise ValueError('unsupported temporal comparison request schema')
    result = {'schema_version': REQUEST_SCHEMA}
    revision = value['source_revision']
    if not isinstance(revision, str) or not re.fullmatch('[0-9a-f]{64}', revision):
        raise ValueError('source_revision must be an exact knowledge snapshot revision')
    result['source_revision'] = revision
    for side in ('left', 'right'):
        ref = value[side]
        if not isinstance(ref, dict) or set(ref) != {'node_id', 'content_revision'}:
            raise ValueError(f'{side} requires node_id and content_revision only')
        identifier = ref['node_id']
        if not isinstance(identifier, str) or not 1 <= len(identifier) <= 1024 or identifier != identifier.strip():
            raise ValueError(f'{side}.node_id must be a nonempty exact normalized ID of at most 1024 characters')
        if not isinstance(ref['content_revision'], str) or not re.fullmatch('[0-9a-f]{64}', ref['content_revision']):
            raise ValueError(f'{side}.content_revision must be an exact carrier revision')
        result[side] = dict(ref)
    return result


def _same_json(left, right):
    # Compare JSON values, not serialization spelling: 1 and 1.0 are numbers,
    # but True and 1 must never alias through Python equality. Exact source
    # record digests remain in the returned carriers, separately from this join.
    if isinstance(left, bool) or isinstance(right, bool):
        return type(left) is type(right) and left == right
    if isinstance(left, (int, float)) and isinstance(right, (int, float)):
        return left == right
    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(_same_json(left[key], right[key]) for key in left)
    if isinstance(left, list):
        return len(left) == len(right) and all(_same_json(a, b) for a, b in zip(left, right))
    return left == right


def _safe_integer(value):
    return type(value) in (int, float) and abs(value) <= 9007199254740991 and math.isfinite(value) and value == int(value)


def _fields(value):
    return value if isinstance(value, dict) else {}


def _require_operand_containers(node):
    # These known containers must obey the existing normalized-node ABI.
    # Reject damaged carriers, never scrub them into a successful packet.
    if any(not isinstance(node.get(key), dict) for key in ('attributes', 'semantics', 'type_mapping')):
        raise TemporalReadModelInvalid('selected normalized carrier has invalid structural containers')
    if any(key in node['semantics'] and not isinstance(node['semantics'][key], dict) for key in ('claim', 'time')):
        raise TemporalReadModelInvalid('selected normalized carrier has invalid structural containers')


def _operand(ref, lookup):
    matches = lookup(ref['node_id'])
    if len(matches) != 1:
        raise KeyError(f'expected one exact knowledge node: {ref["node_id"]}')
    claim = matches[0]
    if claim.get('content_revision') != ref['content_revision']:
        raise KnowledgeRevisionConflict('selected Claim content changed; select again from the current snapshot')
    _require_operand_containers(claim)
    packet = {'claim': copy.deepcopy(claim), 'value': None, 'normalized_time': None}
    if (claim.get('source_graph') != 'source-claims' or claim.get('kind_id') != 'claim'
            or claim.get('type_id') != 'tos.entity.claim' or _fields(claim.get('type_mapping')).get('status') != 'mapped'):
        return packet, 'unsupported', ['selected-node-is-not-a-source-claim']
    semantics = _fields(_fields(claim.get('semantics')).get('claim'))
    source = _fields(claim.get('attributes')).get('source_claim')
    if (not isinstance(source, dict) or not isinstance(source.get('claim_id'), str)
            or source['claim_id'] != semantics.get('claim_id')
            or not _safe_integer(source.get('claim_version')) or source['claim_version'] < 1
            or not _safe_integer(semantics.get('claim_version'))
            or not _same_json(source.get('claim_version'), semantics.get('claim_version'))
            or source.get('predicate') != semantics.get('source_predicate_id')
            or semantics.get('predicate_mapping_status') != 'mapped'):
        return packet, 'undetermined', ['claim-source-binding-inconsistent']
    identifier = semantics.get('object_node_id')
    if not isinstance(identifier, str):
        return packet, 'undetermined', ['claim-object-binding-unavailable']
    values = lookup(identifier)
    if len(values) != 1:
        return packet, 'undetermined', ['claim-object-unavailable-or-ambiguous']
    value = values[0]
    _require_operand_containers(value)
    packet['value'] = copy.deepcopy(value)
    if (value.get('source_graph') != 'source-claims' or value.get('type_id') != 'tos.entity.temporal-assertion'
            or _fields(value.get('type_mapping')).get('status') != 'mapped'):
        return packet, 'unsupported', ['claim-object-is-not-a-declared-temporal-assertion']
    attributes = _fields(value.get('attributes'))
    if attributes.get('claim_ref') != source['claim_id'] or 'value' not in attributes or not _same_json(attributes['value'], source.get('object')):
        return packet, 'undetermined', ['temporal-object-source-binding-inconsistent']
    time = _fields(value.get('semantics')).get('time')
    if not isinstance(time, dict):
        return packet, 'undetermined', ['temporal-normalization-unavailable']
    packet['normalized_time'] = copy.deepcopy(time)
    if 'raw' not in time or not _same_json(time['raw'], attributes['value']):
        return packet, 'undetermined', ['temporal-normalization-source-binding-inconsistent']
    raw_issues = time.get('issues', [])
    if not isinstance(raw_issues, list) or any(not isinstance(issue, str) or not issue for issue in raw_issues):
        return packet, 'unsupported', ['temporal-normalization-issues-invalid']
    issues = list(raw_issues)
    if time.get('kind') not in ('date-assertion', 'interval-assertion', 'relative-order', 'unknown-date'):
        return packet, 'unsupported', ['unsupported-temporal-normalization-kind']
    # An absent declaration is unknown; a declared unsupported system or an
    # invalid/conflicting shape is not an alternate coordinate conversion.
    unsupported = [issue for issue in issues if issue.startswith('conflicting-')
                   or issue in ('reversed-interval', 'invalid-date-parts', 'unparsed-date-value')]
    if time.get('calendar') not in (None, 'gregorian', 'proleptic-gregorian'):
        unsupported.append('unsupported-declared-calendar')
    if time.get('declared_year_numbering') not in (None, 'astronomical'):
        unsupported.append('unsupported-declared-year-numbering')
    if unsupported:
        return packet, 'unsupported', list(dict.fromkeys([*issues, *unsupported]))
    # Stale or malformed normalized carriers cannot substitute ready keys for
    # an explicit absolute-date kind and declared coordinate system.
    if time.get('kind') not in ('date-assertion', 'interval-assertion'):
        issues.append('no-absolute-date-bounds')
    if time.get('calendar') is None:
        issues.append('declared-calendar-unavailable')
    if time.get('declared_year_numbering') is None:
        issues.append('declared-year-numbering-unavailable')
    if time.get('precision') in ('approximate', 'uncertain', 'unknown'):
        issues.append('non-exact-date-precision')
    if time.get('certainty') != 'exact':
        issues.append('explicit-exact-certainty-unavailable')
    if time.get('comparison_calendar') != 'proleptic-gregorian':
        issues.append('comparison-calendar-unavailable')
    if time.get('year_numbering') != 'astronomical':
        issues.append('comparison-year-numbering-unavailable')
    if any(not _safe_integer(time.get(key)) for key in ('sort_start', 'sort_end')):
        issues.append('absolute-date-envelope-unavailable')
    elif time['sort_start'] > time['sort_end']:
        issues.append('reversed-date-envelope')
    if issues:
        return packet, 'undetermined', list(dict.fromkeys(issues))
    return packet, 'comparable', []


def _relation(left, right):
    a, b = left['sort_start'], left['sort_end']
    c, d = right['sort_start'], right['sort_end']
    if b < c:
        return 'before'
    if d < a:
        return 'after'
    if a == c and b == d:
        return 'equal'
    if a <= c and b >= d:
        return 'contains'
    if c <= a and d >= b:
        return 'contained-by'
    return 'overlaps'


def compare_temporal_operands(source_revision, request_value, lookup):
    """Transport-neutral computation over an immutable, exact-ID lookup.

Adapters bind the surrounding read snapshot. A lookup returns a sequence so
that missing and ambiguous IDs cannot be silently resolved by source priority.
    """
    request = normalize_temporal_comparison_request(request_value)
    if request['source_revision'] != source_revision:
        raise KnowledgeRevisionConflict('knowledge snapshot changed; select both Claims again')
    left, left_status, left_issues = _operand(request['left'], lookup)
    right, right_status, right_issues = _operand(request['right'], lookup)
    statuses = (left_status, right_status)
    status = 'unsupported' if 'unsupported' in statuses else 'undetermined' if 'undetermined' in statuses else 'comparable'
    reasons = [{'side': side, 'code': issue} for side, issues in (('left', left_issues), ('right', right_issues)) for issue in issues]
    relation = None
    if status == 'comparable':
        left_role, right_role = left['normalized_time'].get('role'), right['normalized_time'].get('role')
        if not isinstance(left_role, str) or not left_role or not isinstance(right_role, str) or not right_role:
            status = 'undetermined'
            reasons.append({'side': 'pair', 'code': 'time-role-unavailable'})
        elif left_role != right_role:
            status = 'unsupported'
            reasons.append({'side': 'pair', 'code': 'different-time-roles'})
        elif left_role not in SUPPORTED_TIME_ROLES:
            status = 'unsupported'
            reasons.append({'side': 'pair', 'code': 'unsupported-time-role'})
        else:
            relation = _relation(left['normalized_time'], right['normalized_time'])
    refs = sorted({ref for operand in (left, right) for item in (operand['claim'], operand['value'])
                   if item for ref in item.get('source_refs', []) if isinstance(ref, str)})
    return {'schema_version': RESULT_SCHEMA, 'source_revision': source_revision, 'request': request,
            'comparison': {'status': status, 'relation': relation, 'reasons': reasons,
                           'basis': 'normalized-source-date-envelopes'},
            'left': left, 'right': right, 'source_refs': refs, 'authority_boundary': dict(BOUNDARY)}


def compare_temporal_claims(graph, request, *, graph_index):
    graph_index.require_snapshot(graph)
    return compare_temporal_operands(graph['source_revision'], request,
                                     lambda identifier: graph_index.node_ids.get(identifier, ()))

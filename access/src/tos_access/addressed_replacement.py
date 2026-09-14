"""Bounded local candidates, not source membership or publication authority."""
from __future__ import annotations

import copy
from dataclasses import dataclass, fields
import json
import math

from .knowledge import AddressedUpdateError, _replace_direct_carrier_rows
from .normalization_cache import active_cache


@dataclass(frozen=True)
class ReplacementLimits:
    max_relations: int = 1024
    max_endpoints: int = 2048
    max_input_bytes: int = 16 * 1024 * 1024
    max_output_bytes: int = 16 * 1024 * 1024


def _json_size(value, remaining, depth=0):
    """Exact compact UTF-8 cost, bounded before hashing or cloning inputs.

    Container traversal never materializes an unbounded stack or key list.
    Strings are length-checked before encoding; cycles fail the depth bound.
    This is an admission cost, not the typed content-revision encoding.
    """
    if depth > 64:
        raise AddressedUpdateError('replacement JSON exceeds depth budget')
    if remaining < 1:
        raise AddressedUpdateError('replacement JSON exceeds byte budget')
    kind = type(value)
    if kind in (dict, list):
        # At least one byte per list member / key+value, plus framing.
        cost = 2 + max(0, len(value) - 1)
        if cost + len(value) > remaining:
            raise AddressedUpdateError('replacement JSON exceeds byte budget')
        if kind is dict:
            for key, item in value.items():
                if type(key) is not str:
                    raise AddressedUpdateError('replacement JSON requires string object keys')
                cost += _json_size(key, remaining - cost, depth + 1) + 1
                cost += _json_size(item, remaining - cost, depth + 1)
        else:
            for item in value:
                cost += _json_size(item, remaining - cost, depth + 1)
    elif kind is str:
        if len(value) + 2 > remaining:
            raise AddressedUpdateError('replacement JSON exceeds byte budget')
        try:
            cost = len(json.dumps(value, ensure_ascii=False).encode('utf-8'))
        except UnicodeError as error:
            raise AddressedUpdateError('replacement JSON has invalid Unicode') from error
    elif value is None or kind in (bool, int, float):
        if kind is float and not math.isfinite(value):
            raise AddressedUpdateError('replacement JSON requires finite numbers')
        if kind is int and value.bit_length() > remaining * 4:
            raise AddressedUpdateError('replacement JSON exceeds byte budget')
        try:
            cost = len(json.dumps(value, allow_nan=False))
        except ValueError as error:
            raise AddressedUpdateError('replacement JSON number exceeds encoding bounds') from error
    else:
        raise AddressedUpdateError('replacement requires exact JSON types')
    if cost > remaining:
        raise AddressedUpdateError('replacement JSON exceeds byte budget')
    return cost


def replace_direct_carrier_candidate(
    prior_carrier, replacement_record, *, incident_relations, endpoint_nodes,
    normalization_binding, entity_registry, relation_registry, limits=None,
):
    """Replace a direct carrier using only explicitly supplied local rows.

    ``endpoint_nodes`` excludes the prior carrier and must contain every other
    endpoint of each supplied incident relation. Omitted incident relations
    cannot be discovered here. The caller owns complete incidence, source
    admission, before/after membership, snapshot/registry provenance, global
    semantics, catalog and publication. Self-consistent digests are not trust.

    Exact JSON input bytes include both registries and the binding. Output
    bytes count complete candidate rows (not the small report). Limits are
    refusal/work bounds, not RSS guarantees. This route disables ambient cache
    writes and returns no processing run, source revision or published graph.
    """
    if limits is None:
        limits = ReplacementLimits()
    if type(limits) is not ReplacementLimits:
        raise AddressedUpdateError('explicit ReplacementLimits required')
    for field in fields(limits):
        value = getattr(limits, field.name)
        if type(value) is not int or value < 0:
            raise AddressedUpdateError('replacement limits must be nonnegative integers')
    if type(incident_relations) is not list or type(endpoint_nodes) is not list:
        raise AddressedUpdateError('complete supplied row lists required')
    if len(incident_relations) > limits.max_relations or len(endpoint_nodes) > limits.max_endpoints:
        raise AddressedUpdateError('replacement row count budget exceeded')
    if any(type(value) is not dict for value in
           (prior_carrier, replacement_record, normalization_binding, entity_registry, relation_registry)):
        raise AddressedUpdateError('explicit carrier, replacement, binding and registry objects required')
    inputs = [prior_carrier, replacement_record, incident_relations, endpoint_nodes,
              normalization_binding, entity_registry, relation_registry]
    input_bytes = _json_size(inputs, limits.max_input_bytes)
    for row in [prior_carrier, *endpoint_nodes, *incident_relations]:
        if type(row) is not dict:
            raise AddressedUpdateError('supplied normalized row must be an object')
        source, identifier, native = (row.get(key) for key in ('source_graph', 'id', 'native_id'))
        if (not all(type(value) is str and value.strip() for value in (source, identifier, native))
                or not identifier.startswith(source + ':') or identifier == source + ':'):
            raise AddressedUpdateError('supplied row has malformed normalized/source/native identity')
    output_bytes = 0

    def reserve_output(row):
        nonlocal output_bytes
        output_bytes += _json_size(row, limits.max_output_bytes - output_bytes)

    # Existing full-wrapper cache behavior is retained. A candidate must not
    # mutate that cache or accidentally publish a sparse processing lineage.
    token = active_cache.set(None)
    try:
        node, relations = _replace_direct_carrier_rows(
            prior_carrier, prior_carrier['source_graph'], replacement_record,
            incident_relations, endpoint_nodes, entity_registry, relation_registry,
            normalization_binding, check_input_revisions=True, on_output=reserve_output)
    finally:
        active_cache.reset(token)
    return {
        'schema': 'tos_direct_carrier_replacement_candidate_v1',
        'node': copy.deepcopy(node), 'relations': copy.deepcopy(relations),
        'scope': {
            'kind': 'supplied-neighborhood-local-only',
            'carrier_id': prior_carrier['id'],
            'supplied_relation_ids': [row['id'] for row in incident_relations],
            'supplied_endpoint_ids': [row['id'] for row in endpoint_nodes],
            'complete_incidence_verified': False, 'source_transition_verified': False,
            'global_semantics_validated': False, 'catalog_updated': False,
            'publication_current': False, 'published': False,
        },
        'accounting': {'input_bytes': input_bytes, 'output_row_bytes': output_bytes,
                       'relations': len(incident_relations), 'endpoints': len(endpoint_nodes)},
        'is_semantic_acceptance': False,
    }

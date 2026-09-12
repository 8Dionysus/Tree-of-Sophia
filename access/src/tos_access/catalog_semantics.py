"""Shared exact catalog contributions and rendering, subordinate to owner inputs.

No SQLite, source reads, publication, or semantic admission lives here. Sequence
order is observable: representatives, examples and casefold facet ties retain
the owner's first encounter. Both reducers consume these same row facts.
"""
from __future__ import annotations

import copy
import hashlib
import json
from collections import Counter, defaultdict
from dataclasses import dataclass
from typing import Any

from . import knowledge as k

PROJECTOR_VERSION = 'tos-exact-catalog-contributions-v1'
SEQUENCE_ORDER = 'owner-sequence-v1'
CANONICAL_ORDER = 'source-graph-id-v1'
FACETS = ('source_graph', 'kind_id', 'type_id', 'type_mapping.status',
          'epistemic.authority_layer', 'epistemic.canon_status',
          'epistemic.review_posture', 'graph_layers', 'view_ids')

ROUTES = (
    (
        "concept",
        ("concept", "principle"),
        (),
        ("tos.entity.concept", "tos.entity.principle"),
        (),
    ),
    (
        "author",
        ("agent",),
        ("authored_by",),
        ("tos.entity.agent",),
        ("tos.relation.authored-by",),
    ),
    (
        "work",
        ("work",),
        ("authored_by", "has_expression"),
        ("tos.entity.work",),
        ("tos.relation.authored-by", "tos.relation.has-expression"),
    ),
    (
        "word",
        ("lexeme", "word", "token", "word-occurrence"),
        ("occurs_in", "expresses_concept"),
        (),
        (),
    ),
    (
        "tradition",
        ("tradition", "school_tradition"),
        (),
        ("tos.entity.tradition", "tos.entity.school-tradition"),
        (),
    ),
    (
        "place",
        ("place", "region"),
        (),
        ("tos.entity.place",),
        ("tos.relation.has-normalized-place",),
    ),
    (
        "source-object",
        ("work", "expression", "edition", "item", "file", "source_witness"),
        (),
        (
            "tos.entity.intellectual-object",
            "tos.entity.source-witness",
        ),
        (),
    ),
)



def encoded(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False)


def owner_json(value):
    """Copy/emit owner values without sorting their observable dictionary order."""
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'), allow_nan=False)


def catalog_digest(value):
    return hashlib.sha256(encoded(value).encode('utf-8')).hexdigest()


class CatalogInputs:
    """Copy-isolated owner header, registries and already normalized lens inputs.

    The header is explicit for EVERY transition. In particular a reducer never
    carries a prior global semantic_validation report into an after snapshot.
    """
    def __init__(self, header, entity_type_registry=None, relation_type_registry=None,
                 lenses=None, *, source_order_profile=SEQUENCE_ORDER):
        if not isinstance(header, dict) or 'nodes' in header or 'relations' in header:
            raise ValueError('catalog header must exclude row collections')
        if source_order_profile not in (SEQUENCE_ORDER, CANONICAL_ORDER):
            raise ValueError('unsupported catalog source order profile')
        self._header = owner_json(header)
        self._entities = owner_json(entity_type_registry)
        self._relations = owner_json(relation_type_registry)
        self._lenses = owner_json([] if lenses is None else lenses)
        self.source_order_profile = source_order_profile

    @classmethod
    def from_graph(cls, graph, corpus, philosophy, entity_type_registry=None,
                   relation_type_registry=None, **options):
        return cls({key: value for key, value in graph.items() if key not in ('nodes', 'relations')},
                   entity_type_registry, relation_type_registry, k.saved_lens_specs(corpus, philosophy),
                   **options)

    @property
    def header(self):
        return json.loads(self._header)

    @property
    def entity_type_registry(self):
        return json.loads(self._entities)

    @property
    def relation_type_registry(self):
        return json.loads(self._relations)

    @property
    def lenses(self):
        return json.loads(self._lenses)

    @property
    def binding(self):
        return catalog_digest([PROJECTOR_VERSION, self.source_order_profile, self.entity_type_registry,
                               self.relation_type_registry, self.lenses,
                               self.header.get('normalization_binding')])

    @property
    def header_digest(self):
        return catalog_digest(self.header)


@dataclass(frozen=True)
class CatalogRow:
    kind: str
    id: str
    source_order: int | tuple[str, str]
    item: dict[str, Any]


@dataclass(frozen=True)
class CatalogChange:
    operation: str
    kind: str
    id: str
    expected_old_digest: str | None = None
    new_item: dict[str, Any] | None = None
    source_order: int | tuple[str, str] | None = None


def order_key(value):
    """Order-preserving BLOB, including NUL and non-ASCII Unicode codepoints.

    UTF-8 lexical order agrees with Unicode scalar order. Escape embedded NUL
    as 00 ff and terminate each component with 00 00: a shorter component sorts
    first, then comparison proceeds to the second component. JSON escaping and
    length-prefixed encodings do not have this property.
    """
    if type(value) is int and 0 <= value < 2 ** 63:
        return b'I' + value.to_bytes(8, 'big')
    if isinstance(value, tuple) and len(value) == 2 and all(isinstance(part, str) for part in value):
        return b'T' + b''.join(part.encode('utf-8').replace(b'\x00', b'\x00\xff') + b'\x00\x00' for part in value)
    raise ValueError('catalog order requires a nonnegative 63-bit integer or two-string tuple')


def order_value(raw):
    if raw[:1] == b'I' and len(raw) == 9:
        return int.from_bytes(raw[1:], 'big')
    if raw[:1] != b'T':
        raise ValueError('invalid stored catalog order')
    parts, current, index = [], bytearray(), 1
    while index < len(raw):
        value = raw[index]
        if value:
            current.append(value); index += 1
        elif raw[index:index + 2] == b'\x00\xff':
            current.append(0); index += 2
        elif raw[index:index + 2] == b'\x00\x00':
            parts.append(current.decode('utf-8')); current.clear(); index += 2
        else:
            raise ValueError('invalid stored catalog tuple escape')
    if len(parts) != 2 or current:
        raise ValueError('invalid stored catalog tuple components')
    return tuple(parts)


def facet_names(kind):
    return FACETS if kind == 'node' else tuple(
        {'kind_id': 'predicate_id', 'type_id': 'relation_type_id',
         'type_mapping.status': 'predicate_mapping.status'}.get(field, field) for field in FACETS)


def node_route_summary(item, entries):
    return {'kind_id': item['kind_id'], 'type_id': item['type_id'],
            'legacy': [route for route, kinds, _, _, _ in ROUTES if item['kind_id'] in kinds],
            'typed': [route for route, _, _, types, _ in ROUTES
                      if types and k._type_is_a(str(item.get('type_id') or ''), types, entries)]}


def route_counts(kind, summary, endpoint):
    result = Counter()
    if kind == 'node':
        for route in summary['typed']:
            result[(kind, 'route-type', route, str(summary['type_id']))] += 1
    else:
        ends = [endpoint(summary[key]) for key in ('from_id', 'to_id')]
        for route, _, predicates, _, types in ROUTES:
            if summary['predicate_id'] in predicates and any(end and route in end['legacy'] for end in ends):
                result[(kind, 'route-predicate', route, str(summary['predicate_id']))] += 1
            if summary['relation_type_id'] in types and any(end and route in end['typed'] for end in ends):
                result[(kind, 'route-type', route, str(summary['relation_type_id']))] += 1
    return result


def surface_facts(item, kind, *, wants_bucket=None):
    counts, posts = Counter(), []
    def add(metric, *keys, count=1):
        counts[(kind, metric, *keys)] += count
    def post(metric, key, value, position=0):
        posts.append(((kind, metric, key), encoded(value), position))
    attributes = item.get('attributes')
    if isinstance(attributes, dict):
        for field, value in k._attribute_values(attributes):
            if not k._allowed_field(field, kind):
                continue
            add('attribute', field); add('attribute-value-type', field, k._json_value_kind(value))
            source = str(item.get('source_graph') or '')
            if source:
                add('attribute-source', field, source)
            candidates = value if isinstance(value, list) else [value]
            seen = set()
            retain_examples = wants_bucket is None or wants_bucket((kind, 'examples', field))
            for position, candidate in enumerate(candidates):
                if isinstance(value, list):
                    add('attribute-array-type', field, k._json_value_kind(candidate))
                if not retain_examples or candidate is None or isinstance(candidate, (dict, list)) or len(seen) == 5:
                    continue
                # Legacy limit is Unicode codepoints with json.dumps defaults,
                # not UTF-8 bytes or a canonical-number-equivalence collapse.
                key = json.dumps(candidate, ensure_ascii=False, sort_keys=True)
                if len(key) <= 180 and key not in seen:
                    seen.add(key)
                    post('examples', field, candidate, position)
    for field, forms in (item.get('display') or {}).items():
        for language, text in k._form_items(forms).items():
            path = f'display.{field}.{language}'
            if text and k._allowed_field(path, kind):
                add('display', path)
    return counts, posts



def row_facts(row, entries, *, wants_bucket=None):
    """One carrier -> intrinsic counters, order postings and minimal route facts."""
    kind, item = row.kind, row.item
    if kind not in ('node', 'relation') or not isinstance(item, dict) or item.get('id') != row.id:
        raise ValueError('invalid catalog row identity')
    if not isinstance(row.id, str) or not row.id:
        raise ValueError('catalog row requires stable id')
    order_key(row.source_order)
    counts, posts = surface_facts(item, kind, wants_bucket=wants_bucket)
    def add(metric, *keys, count=1):
        counts[(kind, metric, *keys)] += count
    def post(metric, key, value, position=0):
        bucket = (kind, metric, key)
        if wants_bucket is None or wants_bucket(bucket):
            posts.append((bucket, owner_json(value) if metric == 'representative' else encoded(value), position))
    group_key, type_key, mapping_key = ('kind_id', 'type_id', 'type_mapping') if kind == 'node' else (
        'predicate_id', 'relation_type_id', 'predicate_mapping')
    group, type_id = str(item[group_key]), str(item[type_key])
    status = str((item.get(mapping_key) or {}).get('status'))
    add('total'); add('group', group); add('type', type_id)
    add('group-type', group, type_id); add('group-status', group, status)
    post('representative', group, item['display']['kind_label' if kind == 'node' else 'label'])
    for field in facet_names(kind):
        raw = k._field(item, field)
        values = raw if isinstance(raw, list) else [raw]
        seen = set()
        for position, value in enumerate(values):
            if value is None or not str(value):
                continue
            text = str(value)
            add('facet', field, text)
            if text not in seen:
                post('facet-order', field, text, position)
                seen.add(text)
    display = item.get('display') or {}
    # These counters also provide exact row-owned graph-count companions.
    if kind == 'node':
        add('source', str(item['source_graph']))
        add('summary-state', str(display.get('summary_state')))
        if (display.get('provenance') or {}).get('source_summary_available') is False:
            add('without-source')
        semantics = item.get('semantics') if isinstance(item.get('semantics'), dict) else {}
        claim = semantics.get('claim') if isinstance(semantics.get('claim'), dict) else {}
        claim_type = k._string(claim.get('relation_type_id'))
        if claim_type:
            add('claim-type', claim_type)
        summary = node_route_summary(item, entries)
    else:
        add('explanation-state', str(display.get('explanation_state')))
        if (display.get('provenance') or {}).get('source_explanation_available') is False:
            add('without-source')
        if item['source_graph'] == 'semantic-interchange':
            add('cross-layer')
        summary = {key: item[key] for key in ('from_id', 'to_id', 'predicate_id', 'relation_type_id')}
    if status in ('mapped', 'unmapped'):
        add('mapping', status)
    return counts, posts, summary


class MemoryCatalogView:
    def __init__(self):
        self.counts = Counter()
        self.posts = defaultdict(dict)

    def wants_bucket(self, bucket):
        limit = 1 if bucket[1] == 'representative' else 5 if bucket[1] == 'examples' else None
        return limit is None or len(self.posts.get(bucket, {})) < limit

    def add(self, counts, posts, order):
        self.counts.update(counts)
        for bucket, value, position in posts:
            previous = self.posts[bucket].get(value)
            candidate = (order, position)
            # Native input is already in source sequence. A rebuild has no
            # deletions, so only the first representative / five examples need
            # retention. SQLite deliberately keeps every row's candidates.
            limit = 1 if bucket[1] == 'representative' else 5 if bucket[1] == 'examples' else None
            if previous is None and limit is not None and len(self.posts[bucket]) >= limit:
                continue
            if previous is None or candidate < previous:
                self.posts[bucket][value] = candidate

    def first_values(self, bucket, limit=None):
        values = sorted(self.posts.get(bucket, {}).items(), key=lambda pair: pair[1])
        return [json.loads(value) for value, _ in (values if limit is None else values[:limit])]


def scalar_counts(view, *prefix):
    # Renderer-local partitioning is O(number of distinct aggregate keys), not
    # one full aggregate scan for every discovered attribute field.
    if not hasattr(view, '_scalar_groups'):
        groups = defaultdict(dict)
        for key, count in view.counts.items():
            if count and len(key) >= 3:
                groups[key[:-1]][key[-1]] = count
        view._scalar_groups = groups
    return dict(sorted(view._scalar_groups.get(prefix, {}).items()))


def display_fields(view, kind):
    return [{'field': field, 'available_item_count': count}
            for field, count in scalar_counts(view, kind, 'display').items()]


def surface_catalog(items, kind, surface):
    view = MemoryCatalogView()
    for order, item in enumerate(items):
        counts, posts = surface_facts(item, kind, wants_bucket=view.wants_bucket)
        view.add(counts, posts, order)
    return attribute_fields(view, kind) if surface == 'attribute' else display_fields(view, kind)


def attribute_fields(view, kind):
    return [{'field': field, 'item_count': count,
             'value_types': scalar_counts(view, kind, 'attribute-value-type', field),
             'array_item_types': scalar_counts(view, kind, 'attribute-array-type', field),
             'sources': sorted(scalar_counts(view, kind, 'attribute-source', field)),
             'examples': view.first_values((kind, 'examples', field), 5)}
            for field, count in scalar_counts(view, kind, 'attribute').items()]


def facet_fields(view, kind):
    result = {}
    for field in facet_names(kind):
        counts = scalar_counts(view, kind, 'facet', field)
        ordered = view.first_values((kind, 'facet-order', field))
        result[field] = [{'value': value, 'count': counts[value]}
                         for value in sorted(ordered, key=str.casefold)]
    return result


def graph_counts(view, original):
    """Replace only existing row-owned fields; preserve unknown owner extensions."""
    result = copy.deepcopy(original)
    n, r = view.counts[('node', 'total')], view.counts[('relation', 'total')]
    replacements = {'nodes': n, 'relations': r, 'sources': scalar_counts(view, 'node', 'source'),
        'display_coverage': {'node_titles': n, 'node_summaries': n,
            'node_summary_states': scalar_counts(view, 'node', 'summary-state'),
            'nodes_without_source_summary': view.counts[('node', 'without-source')],
            'relation_labels': r, 'relation_statements': r, 'relation_explanations': r,
            'relation_explanation_states': scalar_counts(view, 'relation', 'explanation-state'),
            'relations_without_source_explanation': view.counts[('relation', 'without-source')]},
        'semantic_mapping': {'mapped_nodes': view.counts[('node', 'mapping', 'mapped')],
            'unmapped_nodes': view.counts[('node', 'mapping', 'unmapped')],
            'mapped_relations': view.counts[('relation', 'mapping', 'mapped')],
            'unmapped_relations': view.counts[('relation', 'mapping', 'unmapped')],
            'cross_layer_relations': view.counts[('relation', 'cross-layer')]}}
    for key in result.keys() & replacements.keys():
        if key in ('display_coverage', 'semantic_mapping') and isinstance(result[key], dict):
            for child in result[key].keys() & replacements[key].keys():
                result[key][child] = replacements[key][child]
        else:
            result[key] = replacements[key]
    return result


def finalized_header(inputs, catalog):
    """Explicit draft-after -> final publication header, using this result's counts.

    No semantic report or unknown owner field is manufactured or carried from
    a preceding index state. The owner must use this header for subsequent
    before/render calls; those calls verify it exactly, without normalization.
    """
    header = inputs.header
    if catalog.get('schema') != 'tos_knowledge_catalog_v1' or catalog.get('source_revision') != header.get('source_revision'):
        raise ValueError('catalog result does not match its draft source header')
    if catalog_digest(catalog.get('authority_boundary', {})) != catalog_digest(header.get('authority_boundary', {})):
        raise ValueError('catalog result authority boundary mismatch')
    if 'counts' in header:
        # The shared renderer already preserves unknown fields. This helper
        # transfers its exact output, never reconstructs or reinterprets counts.
        header['counts'] = copy.deepcopy(catalog['counts'])
    elif catalog.get('counts') != {}:
        raise ValueError('catalog result invented counts absent from its draft header')
    return header


def render_catalog(inputs, view, *, derive_counts=False):
    graph = inputs.header
    if derive_counts:
        graph['counts'] = graph_counts(view, graph.get('counts', {}))
    counts = view.counts
    entity_type_registry, relation_type_registry = inputs.entity_type_registry, inputs.relation_type_registry
    entity_entries, _, fallback_type_id = k._entity_registry_indexes(entity_type_registry)
    relation_entries, _, fallback_relation_type_id = k._relation_registry_indexes(relation_type_registry)
    kind_counts = Counter(scalar_counts(view, 'node', 'group'))
    type_counts = Counter(scalar_counts(view, 'node', 'type'))
    relation_type_counts = Counter(scalar_counts(view, 'relation', 'type'))
    kinds, predicates = [], []
    for kind, target in (('node', kinds), ('relation', predicates)):
        for group, count in scalar_counts(view, kind, 'group').items():
            types = sorted(scalar_counts(view, kind, 'group-type', group))
            item = {'kind_id' if kind == 'node' else 'predicate_id': group,
                    'display': view.first_values((kind, 'representative', group), 1)[0],
                    'count': count, 'type_ids' if kind == 'node' else 'relation_type_ids': types,
                    'mapping_statuses': sorted(scalar_counts(view, kind, 'group-status', group))}
            if kind == 'relation':
                item['semantic_definitions'] = [
                    {'relation_type_id': type_id, **{field: relation_entries[type_id].get(field)
                     for field in ('labels', 'definition', 'domain_type_ids', 'range_type_ids')}}
                    for type_id in types if type_id in relation_entries]
            target.append(item)
    entity_registry_entries = [{**entry, 'instance_count': type_counts[type_id]}
                               for type_id, entry in sorted(entity_entries.items())]
    relation_registry_entries = []
    for type_id, entry in sorted(relation_entries.items()):
        claims = counts[('node', 'claim-type', type_id)]
        edges = relation_type_counts[type_id]
        relation_registry_entries.append({**entry, 'edge_instance_count': edges,
                                          'claim_instance_count': claims, 'instance_count': edges + claims})
    entity_routes = []
    for route, candidate_kinds, confirming_predicates, candidate_types, confirming_relation_types in ROUTES:
        available_kinds = [kind for kind in candidate_kinds if kind_counts[kind]]
        typed = scalar_counts(view, 'node', 'route-type', route)
        legacy_relations = scalar_counts(view, 'relation', 'route-predicate', route)
        typed_relations = scalar_counts(view, 'relation', 'route-type', route)
        semantic_mode = bool(entity_entries and candidate_types)
        availability = 'available' if (typed if semantic_mode else available_kinds) else 'not_projected'
        has_confirmation = bool(typed_relations if semantic_mode else legacy_relations)
        expects_confirmation = bool(confirming_relation_types if semantic_mode else confirming_predicates)
        readiness = ('not_projected' if availability == 'not_projected' else 'kind_only'
                     if expects_confirmation and not has_confirmation else 'confirmed')
        entity_routes.append({'route_id': route, 'candidate_kind_ids': list(candidate_kinds),
            'available_kind_ids': available_kinds, 'confirming_predicate_ids': list(confirming_predicates),
            'available_confirming_predicate_ids': sorted(legacy_relations),
            'confirming_relation_count': sum(legacy_relations.values()), 'candidate_type_ids': list(candidate_types),
            'available_type_ids': sorted(typed), 'confirming_relation_type_ids': list(confirming_relation_types),
            'available_confirming_relation_type_ids': sorted(typed_relations),
            'semantic_confirming_relation_count': sum(typed_relations.values()),
            'node_count': sum(typed.values()) if semantic_mode else sum(kind_counts[kind] for kind in available_kinds),
            'availability': availability, 'role_readiness': readiness,
            'note': ('No trustworthy nodes of these kinds are projected; the backend will not synthesize them from unrelated predicates.'
                     if availability == 'not_projected' else
                     'Kinds are projected, but no confirming relation currently establishes the requested contextual role.'
                     if readiness == 'kind_only' else
                     'Kinds select candidate entities; predicates establish contextual roles such as authorship.'
                     if confirming_predicates else
                     'Kinds are source-derived candidates and retain their exact kind_id on every node.')})
    return {
        "schema": "tos_knowledge_catalog_v1",
        "source_revision": graph.get("source_revision"),
        "context_presentation": k.presentation_catalog(entity_type_registry),
        "contract_refs": {
            "public_bundle": "/api/knowledge/contracts",
            "knowledge_api": "access/contracts/knowledge-api.v1.json",
            "lens_spec": "access/contracts/lens-spec.v1.schema.json",
            "lens_result": "access/contracts/lens-result.v1.schema.json",
            "temporal_comparison_request": "access/contracts/temporal-comparison-request.v1.schema.json",
            "temporal_comparison_result": "access/contracts/temporal-comparison-result.v1.schema.json",
            "knowledge_graph": "access/contracts/knowledge-graph.v1.schema.json",
            "readable_context": "access/contracts/readable-context.v1.schema.json",
            "entity_type_registry_schema": "ToS/contracts/semantic-entity-type-registry.schema.json",
            "relation_type_registry_schema": "ToS/contracts/semantic-relation-type-registry.schema.json",
            "entity_type_registry": k.ENTITY_REGISTRY_REF,
            "relation_type_registry": k.RELATION_REGISTRY_REF,
        },
        "counts": graph.get("counts", {}),
        "node_kinds": kinds,
        "predicates": predicates,
        "semantic_registries": {
            "properties": copy.deepcopy((entity_type_registry or {}).get("property_definitions", [])),
            "entity_types": {
                "registry_id": entity_type_registry.get("registry_id")
                if isinstance(entity_type_registry, dict)
                else None,
                "registry_version": entity_type_registry.get("registry_version")
                if isinstance(entity_type_registry, dict)
                else None,
                "source_refs": k._strings(entity_type_registry.get("source_refs"))
                if isinstance(entity_type_registry, dict)
                else [],
                "fallback_type_id": fallback_type_id,
                "mapped_instance_count": counts[('node', 'total')] - type_counts[fallback_type_id],
                "unmapped_instance_count": type_counts[fallback_type_id],
                "entries": entity_registry_entries,
            },
            "relation_types": {
                "registry_id": relation_type_registry.get("registry_id")
                if isinstance(relation_type_registry, dict)
                else None,
                "registry_version": relation_type_registry.get("registry_version")
                if isinstance(relation_type_registry, dict)
                else None,
                "source_refs": k._strings(relation_type_registry.get("source_refs"))
                if isinstance(relation_type_registry, dict)
                else [],
                "fallback_relation_type_id": fallback_relation_type_id,
                "mapped_edge_instance_count": counts[('relation', 'total')]
                - relation_type_counts[fallback_relation_type_id],
                "unmapped_edge_instance_count": relation_type_counts[
                    fallback_relation_type_id
                ],
                "entries": relation_registry_entries,
            },
        },
        "lenses": copy.deepcopy(inputs.lenses),
        "capabilities": {
            "execution_version": "tos-lens-execution-v7",
            "property_filters": {"selector": "property_id", "scope": "node-query-and-path-node-query",
                                 "binding": "same-graph-snapshot", "field_and_property_id": "mutually-exclusive",
                                 "unknown_value": "does-not-match-except-exists-false",
                                 "outside_applicable_type": "does-not-match",
                                 "unknown_property": "error", "operators": "declared-per-property",
                                 "string_comparison": "exact-codepoints-no-casefold-or-normalization",
                                 "units_and_languages": "source-declared-no-implicit-conversion"},
            "path_query": {"conditions": 4, "steps_per_condition": 4,
                           "quantifiers": ["exists", "not_exists"], "combination": "all",
                           "scope": "node-selector-roots-and-selected-sources", "walks_may_revisit_nodes": True},
            "inclusion": {"request_field": "explain", "authority": "query-execution-not-semantic-proof"},
            "pagination": {"request_field": "pagination", "scope": "bounded-lens-result",
                           "snapshot_bound": True, "historical_snapshot_retention": False,
                           "reexecutes_bounded_lens": True, "maximum_primary_nodes": 100,
                           "maximum_relations": 100, "context_endpoints_may_repeat": True,
                           "changed_query_or_snapshot_http_status": 409},
            "neighborhood_profiles": [
                {"profile": "overview", "definition": "Bibliographic and conceptual overview; dense text units, anchors and record-maker/provenance links are inspected separately. Shared record production does not establish semantic proximity. Source-filtered carriers of one declared ToS entity expand at zero distance before a relation hop, within node budgets.", "identity_expansion": "declared-tos-entity-id-zero-distance", "excluded_predicates": sorted(k.OVERVIEW_EXCLUDED_PREDICATES), "excluded_relation_type_ids": sorted(k.OVERVIEW_EXCLUDED_RELATION_TYPES)},
                {"profile": "all", "definition": "All declared relation kinds, including detailed text structure; result limits still apply.", "excluded_predicates": []},
            ],
            "sources": list(k.KNOWLEDGE_SOURCES),
            "filter_operators": list(k.FILTER_OPERATORS),
            "operator_value_contracts": {
                "eq": "scalar",
                "neq": "scalar",
                "in": "scalar-or-scalar-array",
                "contains": "scalar-or-scalar-array",
                "prefix": "string",
                "exists": "boolean",
                "gt": "number",
                "gte": "number",
                "lt": "number",
                "lte": "number",
            },
            "node_fields": sorted(k.NODE_FIELDS),
            "relation_fields": sorted(k.RELATION_FIELDS),
            "human_languages": {
                "key_pattern": k._LANGUAGE_KEY.pattern,
                "reserved_roles": ["default", "original"],
                "registration_verified": False,
                "node_fields": display_fields(view, 'node'),
                "relation_fields": display_fields(view, 'relation'),
                "fallback_order": ["default", "ru", "en", "original", "remaining-keys-sorted"],
                "boundary": "Availability is not translation, semantic quality, or interface-language equivalence.",
            },
            "attribute_field_pattern": k._ATTRIBUTE_FIELD.pattern,
            "node_attribute_fields": attribute_fields(view, 'node'),
            "relation_attribute_fields": attribute_fields(view, 'relation'),
            "facets": {
                "nodes": facet_fields(view, 'node'),
                "relations": facet_fields(view, 'relation'),
            },
            "layouts": list(k.LAYOUTS),
            "endpoint_policies": ["both", "either", "independent"],
            "focus": {
                "seed_field": "seed.focus_node_id",
                "resolution_order": ["id", "entity_id", "unique_native_id"],
                "shared_entity_id_resolution": "source-priority-then-node-id",
                "ambiguous_native_id": "rejected",
                "default_depth": 1,
                "default_direction": "either",
                "default_layout": "radial",
            },
            "entity_routes": entity_routes,
            "maximums": {
                "filters_per_item_kind": k.MAX_FILTERS,
                "traversal_depth": k.MAX_TRAVERSAL_DEPTH,
                "nodes": k.MAX_NODE_LIMIT,
                "relations": k.MAX_RELATION_LIMIT,
                "groups": k.MAX_GROUP_LIMIT,
            },
        },
        "authority_boundary": graph.get("authority_boundary", {}),
    }


def memory_catalog(graph, corpus, philosophy, entity_type_registry=None, relation_type_registry=None):
    inputs = CatalogInputs.from_graph(graph, corpus, philosophy, entity_type_registry, relation_type_registry)
    entries, _, _ = k._entity_registry_indexes(entity_type_registry)
    nodes = k._objects(graph.get('nodes'))
    summaries = {str(item['id']): node_route_summary(item, entries) for item in nodes}
    view = MemoryCatalogView()
    for kind, items in (('node', nodes), ('relation', k._objects(graph.get('relations')))):
        for order, item in enumerate(items):
            facts, posts, summary = row_facts(CatalogRow(kind, str(item['id']), order, item), entries,
                                            wants_bucket=view.wants_bucket)
            facts.update(route_counts(kind, summary, summaries.get))
            view.add(facts, posts, order)
    return render_catalog(inputs, view)

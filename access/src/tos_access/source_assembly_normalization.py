"""Bounded shared normalization of an explicitly supplied source closure.

Selection, source checks, new membership and complete reducer inputs belong to
the assembler. This function never discovers a missing dependency or loads a
whole graph. It calls the same node/relation/context kernels as the full build.
"""
from dataclasses import dataclass, fields
import copy

from . import knowledge as k
from .addressed_replacement import _json_size
from .normalization_cache import active_cache


@dataclass(frozen=True)
class AssemblyNormalizationLimits:
    max_nodes: int = 1024
    max_retained_nodes: int = 2048
    max_relations: int = 2048
    max_traces: int = 512
    max_input_bytes: int = 32 * 1024 * 1024
    max_output_bytes: int = 32 * 1024 * 1024


def _id(value):
    if type(value) is not str or not value or len(value.encode('utf-8')) > 4096:
        raise ValueError('exact bounded assembly identifier required')
    return value


def normalize_source_assembly_candidate(*, node_records, relation_records,
        retained_nodes, claim_traces, source_dossier_refs, context_node_order,
        normalization_binding, entity_registry, relation_registry, limits=None):
    """Normalize supplied before/after source cohorts without publishing them.

    node_records are {source_graph, record}, restricted to source-navigation and
    source-claims. relation_records are {source_graph, record, identity_id?};
    bibliographic edges are enriched with the full builder's shared kernel.
    retained_nodes are exact normalized endpoints/context contributors that are
    not being replaced. context_node_order lists ALL supplied Claim/annotation
    contributor IDs in original source encounter order. claim_traces similarly
    retains owner order (including last-wins literal-kind mapping).

    The caller supplies complete incidence of every output node, every trace
    governing it, all referenced Claim context contributors, and exact dossier
    membership. For removed or changed contributions, include the affected node
    as a raw node_record so its old finalization is not reused. Completeness is
    not asserted here, even after every supplied reference resolves.
    """
    limits = limits or AssemblyNormalizationLimits()
    if type(limits) is not AssemblyNormalizationLimits or any(
            type(getattr(limits, field.name)) is not int or getattr(limits, field.name) < 0
            for field in fields(limits)):
        raise ValueError('explicit nonnegative assembly limits required')
    for rows, maximum in ((node_records, limits.max_nodes),
                          (retained_nodes, limits.max_retained_nodes),
                          (relation_records, limits.max_relations),
                          (claim_traces, limits.max_traces)):
        if type(rows) is not list or len(rows) > maximum:
            raise ValueError('assembly row count budget exceeded')
    if type(source_dossier_refs) is not list or type(context_node_order) is not list:
        raise ValueError('explicit dossier membership and context encounter order required')
    material = [node_records, relation_records, retained_nodes, claim_traces,
                source_dossier_refs, context_node_order, normalization_binding,
                entity_registry, relation_registry]
    input_bytes = _json_size(material, limits.max_input_bytes)
    if (any(type(value) is not dict for value in (normalization_binding, entity_registry, relation_registry))
            or k._stable_digest(normalization_binding) != k._stable_digest(
                k._normalization_binding(entity_registry, relation_registry))):
        raise ValueError('normalization dependency changed; explicit migration required')
    dossier_refs = {_id(value) for value in source_dossier_refs}
    if len(dossier_refs) != len(source_dossier_refs):
        raise ValueError('duplicate dossier membership')
    token = active_cache.set(None)
    output_bytes = 7  # [[],[]], plus row bytes and inner-array commas.
    emitted = {'node': 0, 'relation': 0}
    def reserve_output(kind, row):
        nonlocal output_bytes
        output_bytes += int(emitted[kind] > 0)
        output_bytes += _json_size(row, limits.max_output_bytes - output_bytes)
        emitted[kind] += 1
    try:
        nodes, relations = _normalize(node_records, relation_records, retained_nodes,
            claim_traces, dossier_refs, context_node_order, entity_registry, relation_registry, reserve_output)
        output_bytes = _json_size([nodes, relations], limits.max_output_bytes)
        # Shared kernels may borrow subobjects. The returned candidate must not.
        nodes, relations = copy.deepcopy((nodes, relations))
    finally:
        active_cache.reset(token)
    return {'schema': 'tos_source_assembly_normalization_candidate_v1',
        'nodes': nodes, 'relations': relations,
        'accounting': {'input_bytes': input_bytes, 'output_bytes': output_bytes,
            'nodes': len(nodes), 'relations': len(relations), 'traces': len(claim_traces),
            'retained_nodes': len(retained_nodes)},
        'scope': {'complete_incidence_verified': False, 'source_transition_verified': False,
            'reducer_closure_verified': False, 'global_semantics_validated': False,
            'catalog_updated': False, 'published': False}, 'is_semantic_acceptance': False}


def _normalize(node_records, relation_records, retained_nodes, claim_traces,
        dossier_refs, context_node_order, entity_registry, relation_registry, reserve_output):
    entities, entity_mappings, fallback_entity = k._entity_registry_indexes(entity_registry)
    predicates, predicate_mappings, fallback_predicate = k._relation_registry_indexes(relation_registry)
    traces = {}
    for trace in claim_traces:
        if type(trace) is not dict or _id(trace.get('claim_ref')) in traces:
            raise ValueError('duplicate or malformed assembly Claim trace')
        for field in ('claim_node_id', 'subject_node_id', 'object_node_id', 'predicate'):
            _id(trace.get(field))
        traces[trace['claim_ref']] = trace
    predicate_by_object = {trace['object_node_id']: trace['predicate'] for trace in traces.values()}
    by_id, raw_claims = {}, {}
    for node in retained_nodes:
        if type(node) is not dict or _id(node.get('id')) in by_id:
            raise ValueError('duplicate or malformed retained endpoint')
        envelope = node.get('source_record')
        if (type(envelope) is not dict or type(envelope.get('payload')) is not dict
                or envelope.get('digest') != k._stable_digest(envelope['payload'])
                or node.get('content_revision') != k._content_revision(node)):
            raise ValueError('retained endpoint source/content digest differs')
        by_id[node['id']] = node
        if node.get('source_graph') == 'source-claims':
            raw_claims[_id(node.get('native_id'))] = envelope['payload']
    output_ids = []
    for spec in node_records:
        if type(spec) is not dict or set(spec) != {'source_graph', 'record'}:
            raise ValueError('exact assembly node frame required')
        graph, raw = spec['source_graph'], spec['record']
        if graph not in ('source-navigation', 'source-claims') or type(raw) is not dict:
            raise ValueError('source assembly node owner unsupported')
        native = _id(raw.get('node_id') or raw.get('id'))
        identifier = graph + ':' + native
        if identifier in by_id:
            raise ValueError('duplicate assembly node or retained replacement')
        kind = None
        material = raw
        if graph == 'source-claims':
            predicate = predicate_by_object.get(native)
            policy = predicates.get(predicate_mappings.get(('source-claims', predicate, 'claim-predicate')), {})
            kind = k._source_claim_kind(raw, predicate, policy.get('source_claim_profile'))
            material = {**raw, 'graph_layers': sorted({*k._strings(raw.get('graph_layers')), 'bibliographic-claim'})}
            raw_claims[native] = material
        dossier = k._source_dossier_candidate(raw, graph)
        node = k._normalize_node(material, graph, source_kind_id=kind,
            source_dossier_ref=dossier if dossier in dossier_refs else None,
            entity_type_entries=entities, entity_type_mappings=entity_mappings,
            fallback_type_id=fallback_entity)
        by_id[identifier] = node
        output_ids.append(identifier)
    # Reuse full-builder source-bound syntax checks for changed Claims and
    # their supplied identity/literal endpoints. Unchanged endpoint-only Claims
    # do not become new cohort targets requiring unrelated member incidence.
    output_set = set(output_ids)
    checked_raw = [raw for native, raw in raw_claims.items()
                   if raw.get('node_kind') != 'claim' or 'source-claims:' + native in output_set]
    k._validate_claim_navigation_carriers(checked_raw, relation_registry, entities, entity_mappings)
    k._validate_reference_claim_carriers({'claim_traces': claim_traces,
        'edges': [spec['record'] for spec in relation_records
                  if type(spec) is dict and spec.get('source_graph') == 'source-claims' and type(spec.get('record')) is dict]},
        checked_raw, by_id, entities, predicates, predicate_mappings)
    expected_contexts = {identifier for identifier, node in by_id.items()
                         if node.get('kind_id') in ('claim', 'annotation-claim')}
    if (any(type(value) is not str for value in context_node_order)
            or len(set(context_node_order)) != len(context_node_order)
            or set(context_node_order) != expected_contexts):
        raise ValueError('exact complete supplied Claim context encounter order required')
    contexts = k._prepare_claim_context_groups([by_id[key] for key in context_node_order])
    for trace in traces.values():
        for field in ('claim_node_id', 'subject_node_id', 'object_node_id'):
            if 'source-claims:' + trace[field] not in by_id:
                raise ValueError('assembly trace lacks required endpoint')
    for identifier in output_ids:
        node = by_id[identifier]
        if node['source_graph'] == 'source-claims' and node['kind_id'] == 'claim':
            if not any(trace['claim_node_id'] == node['native_id'] for trace in traces.values()):
                raise ValueError('assembly Claim lacks governing trace')
    relations, relation_ids = [], set()
    for spec in relation_records:
        if (type(spec) is not dict or not {'source_graph', 'record'} <= set(spec)
                or set(spec) - {'source_graph', 'record', 'identity_id'}):
            raise ValueError('exact assembly relation frame required')
        graph, raw = spec['source_graph'], spec['record']
        if graph not in k.KNOWLEDGE_SOURCES or type(raw) is not dict:
            raise ValueError('unknown assembly relation source')
        native = _id(raw.get('edge_id') or raw.get('id'))
        identity = spec.get('identity_id')
        if identity is not None:
            _id(identity)
        if graph == 'source-claims':
            raw = k._bibliographic_relation_source(raw, traces, raw_claims)
        for end in ('from', 'to'):
            endpoint = _id(raw.get(end + '_source_graph') or graph) + ':' + _id(raw.get(end + '_id'))
            if endpoint not in by_id:
                raise ValueError('assembly relation lacks required endpoint')
        ref = raw.get('claim_ref')
        if graph == 'source-claims' and isinstance(ref, str) and (graph, ref) not in contexts:
            raise ValueError('assembly relation lacks referenced Claim context')
        relation = k._normalize_relation(raw, graph, by_id,
            native_id=native, identity_id=identity,
            claim_contexts=contexts.get((graph, ref)) if isinstance(ref, str) else None,
            relation_type_entries=predicates, relation_type_mappings=predicate_mappings,
            fallback_relation_type_id=fallback_predicate)
        if relation['id'] in relation_ids:
            raise ValueError('duplicate assembly relation')
        relation_ids.add(relation['id'])
        relations.append(relation)
    updates, literals = k._prepare_claim_finalization(by_id, traces, contexts, relation_registry, raw_claims)
    views = k._prepare_inherited_views(relations)
    compiler = k.ReadableContextCompiler(entity_registry, digest=k._stable_digest)
    nodes = []
    for key in output_ids:
        node = k._attach_readable_context(k._finalize_knowledge_node(by_id[key],
            updates.get(key), views.get(key, []), literals.get(key)), compiler, 'node')
        reserve_output('node', node)
        nodes.append(node)
    for position, row in enumerate(relations):
        relation = k._attach_readable_context(row, compiler, 'relation')
        reserve_output('relation', relation)
        relations[position] = relation
    for row in (*nodes, *relations):
        if row.get('content_revision') != k._content_revision(row):
            raise ValueError('assembly output content revision differs')
    return nodes, relations

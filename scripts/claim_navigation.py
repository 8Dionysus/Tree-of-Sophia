"""Pure claim-navigation descriptor helpers."""

from __future__ import annotations

import hashlib
import json
from typing import Any


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def canonical_digest(value: Any) -> str:
    return hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def _navigation_source_endpoint(node: dict[str, Any], identity_ref: Any) -> dict[str, Any] | None:
    """Bind a whole navigation name to exact source bytes, not a carrier label."""
    properties = node.get('properties')
    if not isinstance(properties, dict):
        return None
    source = properties.get('source_record')
    if not isinstance(source, dict):
        return None
    schema_version = source.get('schema_version')
    if not isinstance(schema_version, str) or not schema_version.strip():
        return None
    identity_field = {
        'tos_artifact_source_witness_v1': 'artifact_id',
        'tos_artifact_source_witness_v2': 'artifact_id',
        'tos_scholarly_composite_witness_v1': 'composite_id',
    }.get(schema_version, 'record_id')
    version, source_ref = source.get('record_version'), node.get('source_ref')
    if (not isinstance(identity_ref, str) or not identity_ref
            or node.get('node_kind') != 'identity'
            or node.get('node_id') != _node_id('identity', identity_ref)
            or properties.get('identity_ref') != identity_ref
            or source.get(identity_field) != identity_ref
            or type(version) is not int or version < 1
            or not isinstance(source_ref, str) or not source_ref.strip()):
        return None
    pointer = ('/custody/inventory_numbers/0' if identity_field == 'artifact_id'
               else '/preferred_label')
    if ('label_source_pointer' in properties and properties['label_source_pointer'] != pointer
            or identity_field == 'artifact_id' and properties.get('label_source_pointer') != pointer):
        return None
    # Only owner-defined native adapters may choose a source-name field. A
    # carrier-supplied pointer cannot turn arbitrary narrative into a title.
    if identity_field == 'artifact_id':
        custody = source.get('custody')
        numbers = custody.get('inventory_numbers') if isinstance(custody, dict) else None
        value = numbers[0] if isinstance(numbers, list) and numbers else None
    else:
        value = source.get('preferred_label')
    if (not isinstance(value, str) or not value.strip()
            or value in {identity_ref, source_ref}
            or properties.get('preferred_label') != value):
        return None
    try:
        digest = hashlib.sha256(json.dumps(source, ensure_ascii=False, sort_keys=True,
                                           separators=(',', ':'), allow_nan=False).encode('utf-8')).hexdigest()
    except (ValueError, UnicodeError):
        return None
    if node.get('source_sha256') != digest:
        return None
    return {'identity_ref': identity_ref, 'node_id': node['node_id'], 'source_ref': source_ref,
            'record_version': version, 'sha256': digest, 'label_pointer': pointer, 'label': value}


def _navigation_endpoint_types(
    subject_node: dict[str, Any], object_node: dict[str, Any], relation: dict[str, Any],
    entity_registry: dict[str, Any], *, object_kind: str | None = None,
) -> bool:
    """Exact source-kind mapping and parent closure; IDs never imply types."""
    entries = entity_registry.get('types', [])
    entities = {entry['type_id']: entry for entry in entries}
    if len(entities) != len(entries):
        return False

    def ancestry(type_id: str, visiting: frozenset[str] = frozenset()) -> set[str] | None:
        if type_id not in entities or type_id in visiting:
            return None
        result = {type_id}
        for parent in entities[type_id]['parent_type_ids']:
            inherited = ancestry(parent, visiting | {type_id})
            if inherited is None:
                return None
            result.update(inherited)
        return result

    for node, allowed, override in ((subject_node, relation['domain_type_ids'], None),
                                    (object_node, relation['range_type_ids'], object_kind)):
        properties = node.get('properties')
        kind = override or (properties.get('identity_kind') if isinstance(properties, dict) else None)
        if not isinstance(properties, dict) or not isinstance(kind, str):
            return False
        mappings = [entry for entry in entries for mapping in entry['source_mappings']
                    if mapping.get('source_graph') == 'source-claims'
                    and mapping.get('source_kind_id') == kind]
        if (len(mappings) != 1 or mappings[0].get('abstract') is not False
                or any(type_id not in entities for type_id in allowed)):
            return False
        closure = ancestry(mappings[0]['type_id'])
        if closure is None or not closure.intersection(allowed):
            return False
    return True


def _navigation_time_endpoint(node: dict[str, Any], claim: dict[str, Any]) -> dict[str, Any] | None:
    """Exact historical-time source wording, not a formatted/converted date."""
    value = claim['object']
    properties = node.get('properties') or {}
    wording = value.get('source_wording')
    if (not isinstance(properties, dict) or not isinstance(wording, dict) or not isinstance(wording.get('text'), str)
            or not wording['text'].strip() or 'language' not in wording
            or wording['language'] is not None and not isinstance(wording['language'], str)
            or node.get('node_kind') != 'literal'
            or node.get('node_id') != 'literal:sha256:' + canonical_digest({'claim_ref': claim['claim_id'], 'value': value})
            or properties.get('claim_ref') != claim['claim_id']
            or canonical_digest(properties.get('value')) != canonical_digest(value)
            or properties.get('value_sha256') != canonical_digest(value)
            or node.get('source_sha256') != canonical_digest(claim)
            or not isinstance(node.get('source_ref'), str) or not node['source_ref']
            or type(node.get('source_line')) is not int or node['source_line'] < 1):
        return None
    return {'claim_ref': claim['claim_id'], 'node_id': node['node_id'],
            'source_ref': node['source_ref'], 'source_line': node['source_line'],
            'claim_version': claim['claim_version'], 'sha256': canonical_digest(claim),
            'value_sha256': canonical_digest(value), 'label_pointer': '/object/source_wording/text',
            'label': wording['text'], 'language': wording['language']}


def build_claim_navigation_descriptor(
    claim: dict[str, Any], subject_node: dict[str, Any], object_node: dict[str, Any],
    registry: dict[str, Any], entity_registry: dict[str, Any],
) -> dict[str, Any] | None:
    """Derive bounded Claim navigation, never a HumanForm or an assertion.

    Registry/template must come from ``load_claim_navigation_registry``.
    Failure priority is mapping, object, endpoint types/names, statuses, render.
    A template opt-in can name exact historical-time source wording; it does
    not convert temporal values or widen source-profile admission.
    Source-profile and legacy Claim validators retain source admission authority.
    """
    template = registry.get('claim_navigation_template')
    if template is None:
        return None
    descriptor: dict[str, Any] = {
        'schema_version': 'tos_claim_navigation_descriptor_v1',
        'purpose': 'claim-navigation-only', 'standalone': False,
        'state': 'unavailable', 'reason': None,
        'template': {'id': template['template_id'], 'version': template['template_version'],
                     'sha256': canonical_digest(template)},
        'claim': {'id': claim['claim_id'], 'version': claim['claim_version'],
                  'sha256': canonical_digest(claim)},
    }

    def unavailable(reason: str) -> dict[str, Any]:
        descriptor['reason'] = reason
        return descriptor

    predicate = claim.get('predicate')
    candidates = [(relation, mapping) for relation in registry['relations']
                  for mapping in relation['source_mappings']
                  if mapping.get('source_graph') == 'source-claims'
                  and mapping.get('scope') == 'claim-predicate'
                  and mapping.get('source_predicate_id') == predicate]
    if (len(candidates) != 1 or candidates[0][0].get('abstract') is not False
            or candidates[0][0].get('assertion_mode') != 'reified-claim'):
        return unavailable('predicate-not-understood')
    relation, mapping = candidates[0]
    value = claim.get('object')
    reader = (relation.get('source_claim_profile') or {}).get('reader')
    temporal_adapter = {'historical-temporal-v1': ('historical-time-source-wording-v1', 'historical-time'),
                        'document-catalogue-temporal-v1': ('document-catalogue-time-source-wording-v1', 'catalogue-assigned-document-date')}.get(reader)
    temporal = (temporal_adapter is not None and temporal_adapter[0] in template.get('object_label_adapters', [])
                and isinstance(value, dict) and value.get('role') == temporal_adapter[1]
                and isinstance(value.get('kind'), str)
                and value.get('kind') in {'date-assertion', 'interval-assertion', 'relative-order', 'unknown-date'}
                and object_node.get('node_kind') == 'literal')
    if not temporal and (not isinstance(value, str) or object_node.get('node_kind') != 'identity'):
        return unavailable('object-not-identity')
    if not _navigation_endpoint_types(subject_node, object_node, relation, entity_registry,
                                      object_kind='temporal-assertion' if temporal else None):
        return unavailable('endpoint-type-not-understood')
    subject = _navigation_source_endpoint(subject_node, claim.get('subject_ref'))
    target = (_navigation_time_endpoint(object_node, claim) if temporal
              else _navigation_source_endpoint(object_node, claim['object']))
    if subject is None or target is None:
        return unavailable('source-name-unavailable')
    statuses = {}
    for field in ('epistemic_status', 'review_status'):
        value = claim.get(field)
        if not isinstance(value, str) or value not in template['status_labels'][field]:
            return unavailable('source-status-unavailable')
        statuses[field] = {'present': True, 'value': value}
    labels = mapping.get('labels')
    if 'labels' not in mapping and sum(
            candidate.get('source_graph') == 'source-claims' and candidate.get('scope') == 'claim-predicate'
            for candidate in relation['source_mappings']) == 1:
        labels = relation.get('labels')
    if not isinstance(labels, dict) or any(
            not isinstance(labels.get(language), str) or not labels[language].strip()
            for language in template['renderings']):
        return unavailable('predicate-label-unavailable')
    title = {}
    for language, parts in template['renderings'].items():
        values = {'claim-marker': template['marker'][language], 'subject-label': subject['label'],
                  'predicate-label': labels[language], 'object-label': target['label'],
                  'declared-epistemic-status': template['status_labels']['epistemic_status'][claim['epistemic_status']][language],
                  'declared-review-status': template['status_labels']['review_status'][claim['review_status']][language]}
        title[language] = ''.join(part['literal'] if 'literal' in part else values[part['slot']] for part in parts)
    title['default'] = title[template['default_language']]
    try:
        output_bytes = len(canonical_json(title).encode('utf-8'))
    except UnicodeError:
        return unavailable('predicate-label-unavailable')
    if output_bytes > template['max_output_bytes']:
        return unavailable('over-budget')
    descriptor.update(state='ready', reason=None,
        predicate={'id': predicate, 'relation_type_id': relation['relation_type_id'],
                   'sha256': canonical_digest(relation), 'mapping_sha256': canonical_digest(mapping)},
        subject=subject, object=target, statuses=statuses, title=title)
    return descriptor


def _node_id(kind: str, ref: str) -> str:
    if kind in {"identity", "claim", "provenance_event", "review"}:
        return f"{kind}:{ref}"
    digest = hashlib.sha256(ref.encode("utf-8")).hexdigest()
    return f"{kind}:sha256:{digest}"

"""Pure text-packet projection helpers."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


def repo_ref(path: Path, repo_root: Path) -> str:
    return path.relative_to(repo_root).as_posix()


def load_json(path: Path, repo_root: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{repo_ref(path, repo_root)} must contain a JSON object")
    return payload


def project_text_packet(packet: dict[str, Any], source_ref: str, *, repo_root: Path = Path(__file__).resolve().parents[1]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Project declared stand-off structure, never read the underlying text.

    Restricted packets are not exported. Public metadata of a private text uses
    an explicit field allowlist, excluding exact-form hashes and display text.
    IDs of representations include packet/version, while record_id keeps the
    source-owned entity identity. Competing schemes therefore never overwrite.
    """
    schema = packet.get('schema_version')
    if schema not in {'tos_source_text_unit_packet_v1', 'tos_semantic_annotation_packet_v2'}:
        return [], []
    rights = packet.get('rights_and_visibility', {})
    visibility = rights.get('packet_visibility', rights.get('record_visibility'))
    if visibility not in {'public', 'public_metadata_only'}:
        return [], []
    content = (visibility == 'public' and rights.get('publication_authorized') is True
               and not rights.get('private_source_used')
               and rights.get('effective_visibility', rights.get('source_content_visibility')) in {'public', 'public_synthetic'})
    # Semantic bodies must not be reconstructed from restricted lexical hashes.
    if schema == 'tos_semantic_annotation_packet_v2' and not content:
        return [], []
    schema_name = 'source-text-unit-packet-v1.schema.json' if schema == 'tos_source_text_unit_packet_v1' else 'semantic-annotation-packet-v2.schema.json'
    Draft202012Validator(load_json(repo_root / 'ToS/contracts' / schema_name, repo_root)).validate(packet)
    scope = packet['source_scope']
    packet_id = packet.get('packet_id', packet.get('annotation_id'))
    version = packet.get('packet_version', packet.get('annotation_version'))
    namespace = hashlib.sha256(f'{source_ref}:{packet_id}:{version}'.encode()).hexdigest()[:20]
    nodes: list[dict[str, Any]] = []
    edges: list[dict[str, Any]] = []
    identities: dict[str, str] = {}

    def add(identity: str, kind: str, record: dict[str, Any], title: str | None = None) -> str:
        identifier = f'{identity}@{namespace}'
        identities[identity] = identifier
        nodes.append({'node_id': identifier, 'node_kind': kind, 'label': title or identity,
                      'source_ref': source_ref, 'identity_status': 'source-declared-versioned-record',
                      'properties': {**record, 'record_id': identity, 'packet_id': packet_id,
                         'packet_version': version, 'content_available': content,
                         'publication_posture': 'public' if content else 'public_metadata_only',
                         'content_posture': packet.get('content_posture'),
                         'review_status': record.get('admission_status', record.get('boundary_posture', 'not-recorded'))}})
        return identifier

    def edge(left: str, predicate: str, right: str, claim_ref: str | None = None):
        key = hashlib.sha256(f'{namespace}:{left}:{predicate}:{right}'.encode()).hexdigest()
        value = {'edge_id': f'text-spine:{key}', 'from_id': left, 'to_id': right,
                 'predicate_id': predicate, 'edge_kind': 'stand-off-source-structure',
                 'source_refs': [source_ref], 'review_status': 'source-declared-not-semantic-acceptance'}
        if claim_ref:
            value['claim_ref'] = claim_ref
        edges.append(value)

    layer = packet.get('source_layer', {})
    layer_ref = layer.get('text_layer_ref', scope.get('source_text_layer_ref'))
    layer_digest = layer.get('text_layer_sha256')
    layer_identity = 'text-layer:' + hashlib.sha256(f'{layer_ref}:{layer_digest}'.encode()).hexdigest()
    layer_id = add(layer_identity, 'text-layer', dict(layer) if content else
                   {k: layer[k] for k in ('text_layer_ref', 'language', 'immutable', 'position_unit', 'interval', 'visibility') if k in layer},
                   f"Text layer · {layer.get('language', 'source')} · {source_ref.rsplit('/', 1)[-1]}")
    edge(scope['work_ref'], 'has_text_layer', layer_id)
    annotation = add(packet_id, 'annotation', {'rights_and_visibility': rights, 'source_scope': scope},
                     f"Stand-off packet · version {version}")
    edge(layer_id, 'has_annotation', annotation)
    anchors = packet.get('anchors', scope.get('source_anchors', []))
    for anchor in anchors:
        if anchor['selector']['start'] > anchor['selector']['end']:
            raise ValueError(f'{source_ref}: reversed anchor selector')
        record = dict(anchor) if content else {key: anchor[key] for key in ('anchor_ref', 'ordinal', 'selector', 'anchor_role', 'text_layer_ref') if key in anchor}
        identifier = add(anchor['anchor_ref'], 'anchor', record, f"Anchor · {anchor.get('ordinal', anchor['anchor_ref'])}")
        edge(identifier, 'anchored_in', layer_id)
    for unit in packet.get('units', []):
        record = dict(unit) if content else {key: unit[key] for key in ('unit_id', 'unit_version', 'unit_kind', 'surface_posture', 'continuity', 'ordered_anchor_refs', 'parent_unit_refs', 'ordered_child_unit_refs', 'boundary_posture', 'semantic_promotion') if key in unit}
        identifier = add(unit['unit_id'], 'text-unit', record, f"{unit['unit_kind']} · {unit['unit_id']}")
        edge(layer_id, 'has_text_unit', identifier)
        for anchor in unit['ordered_anchor_refs']:
            if anchor not in identities:
                raise ValueError(f'{source_ref}: unresolved unit anchor {anchor}')
            edge(identifier, 'has_anchor', identities[anchor])
    for entity in packet.get('entities', []):
        labels = entity.get('display_labels', [])
        record = {**entity, 'variant_labels': labels}
        # Native stand-off entities are not authored description profiles.
        # Only the adapter kind changes; source kind, identity and body remain
        # exact, including lexical_sense's native spelling and admission state.
        kind = entity['entity_kind'].replace('_', '-')
        if entity['entity_kind'] in {'occurrence', 'lexeme', 'lexical_sense', 'sign'}:
            kind = 'annotation-' + kind
        identifier = add(entity['entity_id'], kind, record,
                         labels[0]['value'] if labels else None)
        edge(annotation, 'annotation_member', identifier)
        for anchor in entity['identity_basis']['anchor_refs']:
            if anchor not in identities:
                raise ValueError(f'{source_ref}: unresolved occurrence anchor {anchor}')
            edge(identifier, 'has_anchor', identities[anchor])
    # Keep assertions, their evidence, and reviews addressable; no materialized
    # semantic edge is emitted merely because an annotation mentions two things.
    for claim in packet.get('claims', []):
        identifier = add(claim['claim_id'], 'annotation-claim', dict(claim), claim['proposition']['predicate'])
        edge(annotation, 'annotation_member', identifier)
        subject = identities.get(claim['proposition']['subject_ref'])
        if subject is None:
            raise ValueError(f'{source_ref}: unresolved annotation claim subject')
        edge(identifier, 'assertion_subject', subject)
        obj = claim['proposition']['object']
        if obj.get('kind') == 'entity_ref':
            if obj['entity_ref'] not in identities:
                raise ValueError(f'{source_ref}: unresolved annotation claim object')
            edge(identifier, 'assertion_object', identities[obj['entity_ref']])
        else:
            value_id = add(f"{claim['claim_id']}:object", 'literal', dict(obj), f"Claim object · {obj['kind']}")
            edge(identifier, 'assertion_object', value_id)
        for anchor in claim['target_anchor_refs']:
            edge(identifier, 'has_anchor', identities[anchor])
        for index, evidence in enumerate(claim['evidence']):
            evidence_id = add(f"{claim['claim_id']}:evidence:{index}", 'annotation-evidence', dict(evidence), evidence['description'])
            edge(identifier, 'assertion_evidence', evidence_id)
            for anchor in evidence['anchor_refs']:
                edge(evidence_id, 'has_anchor', identities[anchor])
    for review in packet.get('reviews', []):
        identifier = add(review['review_id'], 'annotation-review', dict(review), f"Review · {review.get('decision', review.get('outcome'))}")
        edge(annotation, 'annotation_member', identifier)
        for claim in packet.get('claims', []):
            if review['review_id'] in claim['review_refs']:
                edge(identities[claim['claim_id']], 'assertion_review', identifier)
    for relation in packet.get('relations', []):
        if relation['claim_ref'] not in identities:
            raise ValueError(f'{source_ref}: unresolved semantic relation claim')
        identifier = add(relation['relation_id'], 'annotation-relation', dict(relation), relation['relation_type'])
        edge(identifier, 'assertion_subject', identities[relation['subject_ref']])
        edge(identifier, 'assertion_object', identities[relation['object_ref']])
        edge(identifier, 'asserted_by', identities[relation['claim_ref']])
    return nodes, edges

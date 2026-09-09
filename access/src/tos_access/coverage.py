"""Read-model migration observations, not content assessment or a source registry.

Run explicitly: this scans a complete normalized snapshot, never a hover/query
hot path. Rows contain references and mechanical states, not source wording.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import json
from typing import Any, Iterator

from .knowledge import HUMAN_FORM_ROLES, _LANGUAGE_KEY, _display_selection, select_human_forms


def _validate_language(language):
    if (not isinstance(language, str) or len(language) > 128 or
            (language not in {'auto', 'original'} and not _LANGUAGE_KEY.fullmatch(language))):
        raise ValueError('invalid coverage language preference')


def coverage_row(item: dict[str, Any], *, language: str = 'auto') -> dict[str, Any]:
    """Observe one complete carrier. Identity duplicates remain separate rows."""
    _validate_language(language)
    relation = 'from_id' in item
    mapping = item.get('predicate_mapping' if relation else 'type_mapping', {})
    attributes = item.get('attributes') or {}
    display = item.get('display') or {}
    provenance = display.get('provenance') or {}
    selection = _display_selection(item, language)
    observed_fields = {}
    for field, selected in selection['fields'].items():
        # content_available is a delivery signal, not a quality judgment. A
        # deterministic relationship statement may be available without being
        # source-authored prose. Preserve its declared derivation separately.
        observed_fields[field] = {
            'content_available': selected['content_available'],
            'wording_state': ('missing' if not selected['content_available'] else
                'derived-navigation' if provenance.get(field) in
                {'identifier-fallback', 'projected-path', 'navigation-template',
                 'record-version-navigation', 'endpoint-label-synthesis'} else
                'source-marked' if provenance.get(field) in
                {'source-derived', 'authored', 'exact-record-quotation', 'projected-label',
                 'projected-predicate-label', 'registry-label'} else 'unclassified'),
            'derivation': provenance.get(field),
            'selected_key': selected['selected_key'],
            'actual_language': selected['actual_language'],
            'selection_reason': selected['reason'],
            'source_pointer': selected['source_form_pointer'],
        }
    form_selection = select_human_forms(item, language) if 'human_forms' in attributes else None
    roles = {}
    for role in HUMAN_FORM_ROLES:
        selected = form_selection['roles'][role] if form_selection else None
        packet = selected.get('packet') if selected else None
        candidates = [candidate for candidate in form_selection['candidates']
                      if candidate['role'] == role] if form_selection else []
        roles[role] = {
            'state': selected['state'] if selected else 'not-provided',
            'reason': selected['reason'] if selected else 'no-materialized-form-collection',
            'candidate_states': dict(sorted(Counter(candidate['state'] for candidate in candidates).items())),
            'form': selected['form'] if selected else None,
            'derivation': packet.get('derivation') if packet else None,
            'language': packet.get('language') if packet else None,
            'assessment_snapshot_present': bool(packet and packet.get('assessment_snapshot')),
            # A form's ready state and the subject's separate admission are
            # deliberately not collapsed into accepted/rejected or a score.
            'subject_assessment_present': bool(packet and packet.get('subject_assessment')),
        }
    source_records = [key for key in ('source_record', 'source_claim', 'record_version_view')
                      if key in attributes]
    next_actions = []
    if mapping.get('status') != 'mapped':
        next_actions.append('review-source-mapping-with-semantic-registry-owner')
    if any(not field['content_available'] for field in observed_fields.values()):
        next_actions.append('review-missing-wording-at-source')
    if not form_selection:
        next_actions.append('source-owner-must-declare-form-adapter-or-explicit-gap')
    elif form_selection['state'] != 'available':
        next_actions.append('inspect-form-collection-with-source-owner')
    if form_selection and any(role['state'] != 'ready' for role in roles.values()):
        next_actions.append('inspect-role-gaps-and-applicability-with-source-owner')
    return {
        'kind': 'relation' if relation else 'node', 'id': item['id'],
        'entity_id': item.get('entity_id'), 'source_graph': item.get('source_graph'),
        'content_revision': item['content_revision'],
        'source_refs': list(item.get('source_refs', [])),
        'mapping': {'status': mapping.get('status', 'unknown'),
                    'semantic_id': item.get('relation_type_id' if relation else 'type_id'),
                    'source_id': item.get('predicate_id' if relation else 'kind_id'),
                    'basis': 'declared-normalizer-mapping-not-semantic-acceptance'},
        'retained_record_pointers': ['/attributes/' + key for key in source_records],
        'display': observed_fields,
        'forms': {'collection_state': form_selection['state'] if form_selection else 'not-provided',
                  'source_ref': form_selection['source_ref'] if form_selection else None,
                  'issues': form_selection['issues'] if form_selection else [], 'roles': roles},
        'next_actions': next_actions,
    }


def coverage_rows(graph: dict[str, Any], *, language: str = 'auto') -> Iterator[dict[str, Any]]:
    """No source reads, hidden classifications or mutations of the snapshot."""
    _validate_language(language)
    for collection in ('nodes', 'relations'):
        for item in graph[collection]:
            yield coverage_row(item, language=language)


def coverage_report(graph: dict[str, Any], *, language: str = 'auto', emit_row=None) -> dict[str, Any]:
    """Aggregate without retaining a second per-object corpus in memory.

    The optional sink receives one row at a time. A terminal summary certifies
    enumeration of this snapshot only; a partial stream without it is incomplete.
    """
    groups = defaultdict(lambda: {'carriers': 0, 'mapping': Counter(),
        'display': defaultdict(Counter), 'form_collections': Counter(),
        'form_roles': defaultdict(Counter), 'candidate_states': defaultdict(Counter),
        'retained_record_carriers': 0})
    for row in coverage_rows(graph, language=language):
        group = groups[(row['kind'], row['source_graph'])]
        group['carriers'] += 1
        group['mapping'][row['mapping']['status']] += 1
        group['retained_record_carriers'] += bool(row['retained_record_pointers'])
        for field, observation in row['display'].items():
            group['display'][field]['available' if observation['content_available'] else 'missing'] += 1
            group['display'][field]['derivation:' + str(observation['derivation'])] += 1
            group['display'][field]['wording:' + observation['wording_state']] += 1
        group['form_collections'][row['forms']['collection_state']] += 1
        for role, observation in row['forms']['roles'].items():
            group['form_roles'][role][observation['state']] += 1
            group['candidate_states'][role].update(observation['candidate_states'])
        if emit_row is not None:
            emit_row({'schema_version': 'tos_knowledge_coverage_row_v1',
                      'source_revision': graph['source_revision'], 'observation': row})

    def plain(value):
        return {key: plain(member) for key, member in sorted(value.items())} if isinstance(value, dict) else value

    return {'schema_version': 'tos_knowledge_coverage_v1',
        'source_revision': graph['source_revision'], 'language': language,
        'scope': 'normalized-snapshot-carriers-only', 'enumeration_complete': True,
        'counting_unit': 'carrier-not-distinct-subject-or-assessed-content',
        'nodes': len(graph['nodes']), 'relations': len(graph['relations']),
        'groups': [{'kind': kind, 'source_graph': source, **plain(value)}
                   for (kind, source), value in sorted(groups.items())],
        'limitations': ['Source objects absent from the snapshot are not enumerated.',
            'Source record pointers do not establish byte preservation against live source.',
            'Source-marked wording reports projected provenance, not verified source authorship.',
            'Display presence and ready forms do not establish substantive quality or admission.',
            'Role absence does not establish applicability; source owner must review the gap.',
            'No rights, payload availability, language, or truth is inferred from missing data.',
            'The report describes this exact projection, not generated currentness or runtime health.'],
        'performs_assessment': False, 'writes_to_source': False}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True, help='ToS root with existing derived projections')
    parser.add_argument('--language', default='auto')
    parser.add_argument('--rows', action='store_true', help='Stream NDJSON carrier rows before the terminal summary')
    args = parser.parse_args(argv)
    try:
        _validate_language(args.language)
    except ValueError as error:
        parser.error(str(error))
    from .core import ToSAccessCore
    graph = ToSAccessCore.discover(tos_root=args.root).knowledge_graph()
    def emit(value):
        print(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False))
    emit(coverage_report(graph, language=args.language, emit_row=emit if args.rows else None))


if __name__ == '__main__':
    main()

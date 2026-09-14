#!/usr/bin/env python3
"""Observe exact public catalog records in the ordinary normalized read model.

Explicit offline scan, not a query hot path, new registry or content assessment.
Rows disclose identities, source references, field names and mechanical states,
not source wording. No payloads, native private identity inventory or grants
are exported. A stream without its final summary is incomplete.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import sys

from build_source_witness_catalog import MANIFEST_PATH, check_outputs, render_outputs
from source_record_profiles import SourceClaimProfiles
from source_witness_bibliographic_graph_common import (
    COMPOSITE_SCHEMA, _load_source_claim, canonical_digest, iter_jsonl,
)

REPO_ROOT = Path(__file__).resolve().parents[1]


def observe_record(identity, record, source_ref, candidates, *, kind, adapter, source_line=None):
    """Compare complete JSON values, including unknown fields and nulls.

    A matching identity or one correct alias does not conceal a conflicting
    materialized record. Navigation-only aliases need not duplicate raw data.
    Canonical field equality is not exact source-file byte preservation.
    """
    field = 'source_claim' if kind == 'claim' else 'source_record'
    observed = []
    for item in sorted(candidates, key=lambda value: value['id']):
        raw = (item.get('attributes') or {}).get(field)
        supplied = isinstance(raw, dict)
        missing = sorted(set(record) - set(raw)) if supplied else []
        added = sorted(set(raw) - set(record)) if supplied else []
        changed = sorted(key for key in set(record) & set(raw)
                         if canonical_digest(record[key]) != canonical_digest(raw[key])) if supplied else []
        exact = supplied and not (missing or added or changed)
        mapping_key = 'predicate_mapping' if 'from_id' in item else 'type_mapping'
        mapped = (item.get(mapping_key) or {}).get('status') == 'mapped'
        source_return = source_ref in item.get('source_refs', [])
        observed.append({'id': item['id'], 'source_graph': item.get('source_graph'),
            'kind': 'relation' if 'from_id' in item else 'node',
            'raw_record_state': 'exact' if exact else 'different' if supplied else 'not-provided',
            'mapping_state': 'mapped' if mapped else 'unmapped',
            'source_return_present': source_return,
            'missing_fields': missing, 'added_fields': added, 'changed_fields': changed,
            'record_pointer': '/attributes/' + field if supplied else None})
    if not observed:
        state = 'missing-from-projection'
    elif any(item['raw_record_state'] == 'different' for item in observed):
        state = 'conflicting-record-carriers'
    elif any(item['raw_record_state'] == 'exact' and item['mapping_state'] == 'mapped'
             and item['source_return_present'] for item in observed):
        state = 'mapped-through-adapter' if adapter == 'native-witness' else 'mapped-directly'
    else:
        state = 'requires-clarification'
    return {'identity': identity, 'kind': kind, 'state': state, 'adapter': adapter,
        'source_ref': source_ref, 'source_line': source_line,
        'record_digest': 'sha256:' + canonical_digest(record),
        'field_count': len(record), 'carriers': observed,
        'next_owner': 'ToS/source-witnesses',
        'next_action': ('retain-source-and-review-separate-content-form-and-admission-gaps'
                        if state.startswith('mapped-') else
                        'inspect-exact-source-and-projection-mapping-with-the-source-owner')}


def _file_digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for part in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(part)
    return value.hexdigest()


def _catalog_objects(root, manifest):
    """Read the entire verified source catalog, not a consumer's selected families."""
    objects = {}
    for kind, catalog_ref in manifest['record_files'].items():
        for _, entry in iter_jsonl(root / catalog_ref, root):
            ref = Path(entry['source_record_ref'])
            path = root / ref
            if (ref.parts[:2] != ('ToS', 'source-witnesses') or ref.is_absolute()
                    or ref.as_posix() != entry['source_record_ref'] or '..' in ref.parts
                    or any(part in {'payload', 'local-content', 'owner-local', 'catalog'} for part in ref.parts)
                    or path.resolve() != path.absolute()):
                raise ValueError('catalog metadata source is outside its non-symlink public owner route')
            with path.open('rb') as stream:
                raw = stream.read(1_048_577)
            if len(raw) > 1_048_576:
                raise ValueError('catalog source metadata exceeds the diagnostic 1 MiB per-record budget')
            record = json.loads(raw)
            identity = entry['record_id']
            if (identity in objects or entry['record_type'] != kind or not isinstance(record, dict)
                    or canonical_digest(record) != entry['record_sha256']):
                raise ValueError('catalog metadata identity, family or exact source digest drifted')
            objects[identity] = {**entry, '_source_record': record}
    return objects


def coverage_report(root, graph, *, emit_row=None, verify_graph=None):
    """Enumerate the current public catalog and retain source-check boundaries.

    A stale catalog or changing source aborts the report, never inventing a
    completed migration or silently rebuilding the inspected inputs.
    """
    root = Path(root).resolve()
    outputs = render_outputs(root)
    if check_outputs(root, outputs):
        raise ValueError('public source catalog is stale; rebuild through its owner before observing coverage')
    manifest = json.loads(outputs[MANIFEST_PATH])
    objects = _catalog_objects(root, manifest)
    claim_profiles = SourceClaimProfiles(root)
    # The bibliographic graph loader deliberately filters legacy object-Link
    # Claims. A source-first inventory must not inherit that consumer filter.
    claims = [entry for _, entry in iter_jsonl(root / manifest['claim_file'], root)]
    source_hashes = {}
    candidates = defaultdict(list)
    identities = set(objects) | {entry['claim_id'] for entry in claims}
    for collection in ('nodes', 'relations'):
        for item in graph[collection]:
            attributes = item.get('attributes') or {}
            declared = {item.get('entity_id'), item.get('native_id'), item.get('id'),
                        attributes.get('record_id'), attributes.get('identity_ref'),
                        attributes.get('claim_id'), attributes.get('claim_ref')}
            for identity in declared & identities:
                candidates[identity].append(item)
    counts = {kind: Counter() for kind in (*manifest['record_files'], 'claim')}
    total = 0

    def emit(identity, record, source_ref, kind, adapter, source_line=None):
        nonlocal total
        if source_ref not in source_hashes:
            source_hashes[source_ref] = _file_digest(root / source_ref)
        row = observe_record(identity, record, source_ref, candidates[identity],
                             kind=kind, adapter=adapter, source_line=source_line)
        row['source_file_sha256'] = source_hashes[source_ref]
        counts[kind][row['state']] += 1
        total += 1
        if emit_row is not None:
            emit_row({'schema_version': 'tos_source_projection_coverage_row_v1',
                      'source_revision': graph['source_revision'], 'observation': row})

    for identity, entry in sorted(objects.items()):
        adapter = ('native-witness' if entry['record_type'] == 'artifact'
                   or entry.get('source_schema_ref') == COMPOSITE_SCHEMA else 'source-record')
        emit(identity, entry['_source_record'], entry['source_record_ref'], entry['record_type'], adapter)
    for entry in claims:
        record = _load_source_claim(entry, repo_root=root, profiles=claim_profiles)
        emit(entry['claim_id'], record, entry['source_claim_file_ref'], 'claim', 'reified-claim',
             entry['source_claim_line'])
    if total != manifest['counts']['total']:
        raise ValueError('enumerated source identities differ from the verified catalog count')
    # Revalidate source membership as well as bytes. The selected publication
    # protocol remains narrower than a global filesystem transaction.
    if render_outputs(root) != outputs or check_outputs(root, outputs):
        raise ValueError('source catalog or its inputs changed during coverage observation')
    if any(_file_digest(root / ref) != digest for ref, digest in source_hashes.items()):
        raise ValueError('source-file bytes changed during coverage observation')
    if verify_graph is not None:
        verify_graph(graph['source_revision'])
    return {'schema_version': 'tos_source_projection_coverage_v1',
        'scope': 'current-public-source-witness-catalog', 'enumeration_complete': True,
        'source_revision': graph['source_revision'], 'catalog_sha256': manifest['catalog_sha256'],
        'source_files_digest': 'sha256:' + canonical_digest(dict(sorted(source_hashes.items()))),
        'objects': len(objects), 'claims': len(claims), 'source_identities': total,
        'source_files': len(source_hashes),
        'groups': [{'kind': kind, 'source_identities': sum(states.values()), 'states': dict(sorted(states.items()))}
                   for kind, states in sorted(counts.items())],
        'limitations': [
            'Only catalog-owned public metadata identities are enumerated, not all ToS corpus resources.',
            'Private native semantic packets, payloads and owner-local material are outside this report.',
            'Unknown uncatalogued families require their source owner; absence does not imply restriction or falsity.',
            'Direct/adapted mapping means exact retained JSON fields and source return, not semantic understanding.',
            'Source-file hashes bind observed bytes; a JSON carrier does not preserve file formatting.',
            'Historical versions, form quality, assessment, rights and canon require their separate owner routes.',
            'The report compares this snapshot; it does not certify full generated, runtime or deployment currentness.',
            'Boundary rechecks do not provide an atomic snapshot against arbitrary non-cooperating editors.'],
        'performs_assessment': False, 'grants_admission': False, 'writes_to_source': False}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=REPO_ROOT)
    parser.add_argument('--rows', action='store_true', help='Stream per-source NDJSON before the terminal summary')
    args = parser.parse_args(argv)
    sys.path.insert(0, str(REPO_ROOT / 'access/src'))
    from tos_access.core import ToSAccessCore
    def emit(value):
        print(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False))

    def verify_graph(revision):
        if core.knowledge_graph()['source_revision'] != revision:
            raise ValueError('normalized read-model inputs changed during coverage observation')

    try:
        core = ToSAccessCore.discover(tos_root=args.root)
        graph = core.knowledge_graph()
        report = coverage_report(args.root, graph, emit_row=emit if args.rows else None,
                                 verify_graph=verify_graph)
    except (ValueError, OSError, RuntimeError) as error:
        parser.exit(2, str(error) + '\n')
    emit(report)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())

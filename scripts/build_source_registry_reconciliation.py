"""Reconcile reported registry leads with exact current owner records, without merging."""
from __future__ import annotations
import argparse
import gzip
import json
from collections import Counter, defaultdict
from source_registry_common import ROOT, PACKET, canonical_url, digest, encoded, read, safe, urls


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, list):
        for child in value:
            yield from strings(child)
    elif isinstance(value, dict):
        for child in value.values():
            yield from strings(child)


def build(root=ROOT):
    packet = root / PACKET
    snapshot_id = read(packet / 'current.json')['snapshot_id']
    snapshot = packet / 'snapshots' / snapshot_id
    summary = read(snapshot / 'snapshot.json')
    by_url, by_label, by_identifier = defaultdict(set), defaultdict(set), defaultdict(set)
    sources = {}
    for catalog in sorted((root / 'ToS/source-witnesses/catalog').glob('*.jsonl')):
        if catalog.name == 'claims.jsonl':
            continue
        for line in catalog.read_text().splitlines():
            if not line:
                continue
            entry = json.loads(line)
            ref = entry['source_record_ref']
            if ref in sources:
                continue
            source_path = safe(root, ref)
            record = read(source_path)
            actual = digest(json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode())
            if entry['record_sha256'] != actual:
                raise ValueError(f'catalog navigation stale against owner record: {ref}')
            source = {'record_id': entry['record_id'], 'record_type': entry['record_type'], 'path': ref,
                      'sha256': digest(source_path.read_bytes()), 'preferred_label': entry['preferred_label'],
                      'declared_identity_status': record.get('identity_status', entry['identity_status'])}
            manifest_ref = record.get('item_manifest_ref')
            if manifest_ref:
                item_manifest = read(safe(root, manifest_ref))
                source['item_files'] = []
                for f in item_manifest['payload_files']:
                    local = safe(root, str(safe(root, manifest_ref).parent.relative_to(root) / f['relative_path']))
                    present = local.is_file()
                    source['item_files'].append({'path': str(local.relative_to(root)), 'exists': present,
                        'expected_sha256': f['sha256'], 'fixity_matches': digest(local.read_bytes()) == f['sha256'] if present else None})
            sources[ref] = source
            for text in strings(record):
                for url in urls(text):
                    by_url[canonical_url(url)].add(ref)
            for label in [record.get('preferred_label', ''), *[v.get('value', '') for v in record.get('variant_labels', [])]]:
                if label:
                    by_label[label.casefold().strip()].add(ref)
            for identifier in record.get('external_identifiers', []):
                by_identifier[identifier['value'].strip()].add(ref)
    entries = []
    statuses = Counter()
    field_statuses = Counter()
    matched_count = 0
    for name in summary['documents']:
        document = read(snapshot / name)
        for record in document['records']:
            raw = {f['source_field']: f['value'] for f in record['raw_fields']}
            matches = defaultdict(set)
            for text in raw.values():
                for url in urls(text):
                    for ref in by_url[canonical_url(url)]:
                        matches[ref].add('same_reported_url')
            for field in record['reported_fields']:
                field_statuses[(field['target'], field['normalization_status'])] += 1
                if field['target'] in ('subject.title', 'subject.label') and field['value']:
                    for ref in by_label[str(field['value']).casefold().strip()]:
                        matches[ref].add('same_reported_label')
                if field['target'] in ('source.stable_identifier', 'source.witness_or_catalog_identifier') and field['value']:
                    for ref in by_identifier[str(field['value']).strip()]:
                        matches[ref].add('same_reported_identifier_string')
            if matches:
                matched_count += 1
            unresolved = sum(f['normalization_status'] in ('partial', 'reported_unparsed') for f in record['reported_fields'])
            statuses['with_unresolved_fields' if unresolved else 'all_fields_accounted_without_unparsed_fragments'] += 1
            entry = {'record_id': record['record_id'], 'source_record_id': record['source_record_id'],
                     'corpus_id': record['corpus_id'], 'document_id': record['document_id'], 'kind': record['kind'],
                     'source_record_ref': str((snapshot / name).relative_to(root)),
                     'owner_matches': [{'owner_ref': ref, 'basis': sorted(basis), 'status': 'possible_correspondence_requires_identity_review'} for ref, basis in sorted(matches.items())],
                     'readiness': {
                         'version': {'status': 'unreviewed', 'reported': raw.get('version_edition_or_translation')},
                         'access': {'status': 'unverified_current', 'reported': raw.get('access_mode'), 'reported_checked_at': raw.get('checked_at')},
                         'rights': {'status': 'review_required', 'reported_use': raw.get('tos_use')},
                         'file': {'status': 'identity_unresolved', 'possible_owner_file_refs': [ref for ref in matches if sources[ref].get('item_files')]},
                         'branch': {'status': 'linkage_unreviewed', 'reported_atlas_row': raw.get('tos_row_id')}},
                     'unresolved_field_count': unresolved,
                     'next_owner': 'ToS/source-witnesses/discovery/',
                     'next_condition': 'Review exact version and intended use against owner evidence before acquisition; matches are navigation only.'}
            entries.append(entry)
    return {'schema_version': 'tos_source_registry_reconciliation_v1', 'snapshot_id': snapshot_id,
            'semantic_ceiling': 'review_preparation_no_identity_or_rights_admission',
            'owner_sources': [sources[k] for k in sorted(sources)],
            'summary': {'records': len(entries), 'records_with_owner_match_candidates': matched_count,
                        'owner_records_inspected': len(sources), **statuses},
            'field_status_counts': [{'target': target, 'status': status, 'count': count} for (target, status), count in sorted(field_statuses.items())],
            'records': entries}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    result = build()
    path = ROOT / PACKET / 'reconciliation.current.json.gz'
    body = gzip.compress(encoded(result), mtime=0)
    if args.check:
        if not path.exists() or path.read_bytes() != body:
            raise SystemExit('source registry reconciliation drift')
    else:
        path.write_bytes(body)
    print(result['summary'])

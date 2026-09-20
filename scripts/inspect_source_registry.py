"""Read one normalized record or report directly from the current research snapshot."""
import argparse
import json
from source_registry_common import ROOT, PACKET, read

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', required=True)
    parser.add_argument('--document', required=True)
    parser.add_argument('--record', help='Original source ID or full normalized ID; omit to inspect the report')
    args = parser.parse_args()
    packet = ROOT / PACKET
    snapshot = packet / 'snapshots' / read(packet / 'current.json')['snapshot_id']
    matched = []
    for name in read(snapshot / 'snapshot.json')['documents']:
        document = read(snapshot / name)
        if (document['corpus_id'], document['document_id']) != (args.corpus, args.document):
            continue
        if args.record:
            matched.extend(r for r in document['records'] if args.record in (r['source_record_id'], r['record_id']))
        else:
            matched.append({'files': document['files'], 'report_parts': document['report_parts']})
    if not matched:
        parser.error('No matching record/document in the current snapshot')
    print(json.dumps(matched, ensure_ascii=False, indent=2))

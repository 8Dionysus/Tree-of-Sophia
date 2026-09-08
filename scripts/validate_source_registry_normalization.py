"""Verify research source fixity, full field accounting and deterministic parity."""
from collections import Counter
from jsonschema import Draft202012Validator
from source_registry_common import ROOT, PACKET, read, run

def validate(packet_root=ROOT / PACKET):
    result = run(packet_root, check=True)
    validator = Draft202012Validator(read(ROOT / 'ToS/contracts/source-registry-normalization.schema.json'))
    snapshot = packet_root / 'snapshots' / result['snapshot_id']
    counts = Counter()
    ids = set()
    for name in result['documents']:
        document = read(snapshot / name)
        validator.validate(document)
        for record in document['records']:
            raw = [f['source_field'] for f in record['raw_fields']]
            reported = [f['source_field'] for f in record['reported_fields']]
            if Counter(raw) != Counter(reported) or len(set(raw)) != len(raw):
                raise ValueError(f'field accounting error: {record["record_id"]}')
            if record['record_id'] in ids:
                raise ValueError('duplicate global record ID')
            ids.add(record['record_id'])
            counts[record['kind']] += 1
            if record['source']['original_sha256'] != document['files']['xlsx']['sha256']:
                raise ValueError('record original fixity mismatch')
        for sheet in document['sheets']:
            if sheet['record_count'] != sum(r['source']['sheet'] == sheet['name'] for r in document['records']):
                raise ValueError('sheet record coverage mismatch')
    for kind, count in counts.items():
        if result['counts'][kind] != count:
            raise ValueError('snapshot record total mismatch')
    return result

if __name__ == '__main__':
    result = validate()
    print('source registry normalization: complete reproducible snapshot', result['counts'])

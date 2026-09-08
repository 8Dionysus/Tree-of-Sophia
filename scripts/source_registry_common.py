"""Deterministic research imports; no source identity or rights admission."""
from __future__ import annotations
import hashlib
import gzip
import datetime as dt
import json
import re
from collections import defaultdict
from pathlib import Path
from urllib.parse import urlsplit, urlunsplit
from source_registry_ooxml import workbook, docx, column_index, column_name
from source_registry_values import normalize_record_values, load_profile, extract_reported_links

ROOT = Path(__file__).resolve().parents[1]
PACKET = Path('ToS/research-packets/source-registries')

def encoded(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')) + '\n').encode()

def digest(value):
    return hashlib.sha256(value).hexdigest()

def read(path):
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())

def safe(root, name):
    path = (root / name).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(f'path escapes owner root: {name}')
    return path

def canonical_url(url):
    p = urlsplit(url)
    if p.scheme.lower() not in ('http', 'https') or not p.netloc:
        return url
    # Preserve path, query, fragment, ports and percent spelling. No archive expansion.
    userinfo, marker, hostport = p.netloc.rpartition('@')
    netloc = userinfo + marker + hostport.lower() if marker else p.netloc.lower()
    return urlunsplit((p.scheme.lower(), netloc, p.path, p.query, p.fragment))

def urls(text):
    return [link['url'] for link in extract_reported_links(str(text or ''))]

def normalize_document(spec, files, adapter):
    records = []
    sheets_out = []
    explicit = set()
    anonymous = defaultdict(int)
    sheet_rules = {rule['name']: rule for rule in spec['sheets']}
    if len(sheet_rules) != len(spec['sheets']):
        raise ValueError('duplicate sheet rule')
    for sheet in workbook(files['xlsx']['path']):
        rule = sheet_rules.get(sheet['name'])
        if rule is None:
            raise ValueError(f'unmapped sheet: {spec["document_id"]}/{sheet["name"]}')
        header = next((r for r in sheet['rows'] if r['row'] == rule.get('header_row', 1)), None)
        if header is None:
            raise ValueError('missing header row')
        headers = {column_index(c['cell']): c['value'] for c in header['cells']}
        nonempty = [v for v in headers.values() if v not in ('', None)]
        if len(set(nonempty)) != len(nonempty):
            raise ValueError('duplicate source field header')
        kind = rule['kind']
        start = len(records)
        for row in sheet['rows']:
            if row['row'] <= header['row'] or not any(c['value'] is not None or c['formula'] is not None for c in row['cells']):
                continue
            cells = {column_index(c['cell']): c for c in row['cells']}
            raw = {}
            raw_fields = []
            for col in sorted(set(headers) | set(cells)):
                name = headers.get(col)
                cell = cells.get(col, {'cell': f'{column_name(col)}{row["row"]}', 'value': None,
                                       'type': 'absent', 'xml_value': None, 'style': None, 'formula': None})
                if name in ('', None):
                    if cell['value'] is not None or cell['formula'] is not None:
                        raise ValueError(f'populated unnamed field: {spec["document_id"]}/{sheet["name"]}/{cell["cell"]}')
                    continue
                raw[name] = cell['value']
                raw_fields.append({'source_field': name, **cell})
            normalizer_input = dict(raw)
            for field in raw_fields:
                if field['type'] == 'excel_datetime':
                    normalizer_input[field['source_field']] = dt.datetime.fromisoformat(field['value'])
            values = normalize_record_values(kind, normalizer_input, adapter)
            source_ids = [raw.get(name) for name, mapping in adapter[kind].items()
                          if mapping['target'] == 'identity.source_record_id' and raw.get(name) not in ('', None)]
            if len(set(map(str, source_ids))) > 1:
                raise ValueError('conflicting source ids')
            content_digest = digest(encoded([{k: v for k, v in field.items() if k not in ('cell', 'style')} for field in raw_fields]))
            scope = f'{spec["corpus_id"]}/{spec["document_id"]}/{kind}'
            issues = list(values.get('issues', []))
            if source_ids:
                source_id = str(source_ids[0])
                key = (scope, source_id)
                if key in explicit:
                    raise ValueError(f'duplicate source record id: {key}')
                explicit.add(key)
                local_id = 'source-' + digest(source_id.encode())[:24]
            else:
                source_id = None
                key = (scope, content_digest)
                anonymous[key] += 1
                local_id = 'anonymous-' + content_digest[:24] + '-' + str(anonymous[key])
                issues.append('source_record_id_absent_content_identity_only')
            override_key = f'{kind}:{content_digest}:{anonymous.get((scope, content_digest), 1)}'
            local_id = spec.get('identity_overrides', {}).get(override_key, local_id)
            record_id = f'tos-registry:{scope}/{local_id}'
            locator = {'original_sha256': files['xlsx']['sha256'], 'part': sheet['part'],
                       'sheet': sheet['name'], 'row': row['row']}
            records.append({'record_id': record_id, 'source_record_id': source_id,
                            'corpus_id': spec['corpus_id'], 'document_id': spec['document_id'], 'kind': kind,
                            'occurrence_id': 'tos-occurrence:' + digest(encoded(locator)),
                            'source': locator, 'row_attributes': row['attributes'], 'content_sha256': content_digest,
                            'raw_fields': raw_fields, 'reported_fields': values['reported_fields'],
                            'issues': issues, 'assessment_status': 'reported_unreviewed'})
        sheets_out.append({'name': sheet['name'], 'part': sheet['part'], 'kind': kind, 'state': sheet['state'],
                           'header_cells': header['cells'], 'hyperlinks': sheet['hyperlinks'], 'columns': sheet['columns'], 'record_count': len(records) - start, 'merges': sheet['merges']})
    if set(sheet_rules) != {s['name'] for s in sheets_out}:
        raise ValueError('declared worksheet absent')
    # Mark all members of an indistinguishable anonymous group, not just the last.
    for record in records:
        if record['source_record_id'] is None and anonymous[(f'{spec["corpus_id"]}/{spec["document_id"]}/{record["kind"]}', record['content_sha256'])] > 1:
            record['issues'].append('indistinguishable_anonymous_occurrences')
    reports = docx(files['docx']['path'])
    source_map = defaultdict(list)
    for record in records:
        if record['source_record_id']:
            source_map[record['source_record_id']].append(record['record_id'])
    mention_pattern = re.compile(r'(?<![\w-])(?:' + '|'.join(re.escape(key) for key in sorted(source_map, key=len, reverse=True)) + r')(?![\w-])') if source_map else None
    for part in reports:
        for block in part['blocks']:
            block['occurrence_id'] = 'tos-occurrence:' + digest(encoded([files['docx']['sha256'], part['part'], block['xml_path']]))
            block['record_mentions'] = sorted({rid for match in mention_pattern.finditer(block['text']) for rid in source_map[match[0]]}) if mention_pattern else []
    return {'corpus_id': spec['corpus_id'], 'document_id': spec['document_id'],
            'files': {k: {key: value for key, value in f.items() if key != 'path'} for k, f in files.items()},
            'sheets': sheets_out, 'records': records, 'report_parts': reports,
            'pair_relation': 'manifest_declared_research_companions'}

def build(packet_root, input_root=None):
    manifest = read(packet_root / 'input.manifest.json')
    documents = []
    originals = {}
    profiles = {}
    seen = set()
    corpus_ids = [c['corpus_id'] for c in manifest['corpora']]
    if len(set(corpus_ids)) != len(corpus_ids):
        raise ValueError('duplicate corpus namespace')
    for spec in manifest['documents']:
        if spec['corpus_id'] not in corpus_ids or any(not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9.-]*', spec[k]) for k in ('corpus_id', 'document_id')):
            raise ValueError('invalid corpus/document namespace')
        identity = (spec['corpus_id'], spec['document_id'])
        if identity in seen:
            raise ValueError(f'duplicate document identity: {identity}')
        seen.add(identity)
        profile_path = safe(ROOT, spec['adapter'])
        profile_bytes = profile_path.read_bytes()
        profiles[spec['adapter']] = {'sha256': digest(profile_bytes), 'profile': load_profile(profile_path)}
        files = {}
        for kind in ('xlsx', 'docx'):
            file_spec = spec[kind]
            path = safe(input_root, file_spec['source_path']) if input_root else safe(packet_root, file_spec['original_path'])
            body = path.read_bytes()
            sha = digest(body)
            if file_spec.get('sha256') and sha != file_spec['sha256']:
                raise ValueError(f'input changed; review and update manifest: {file_spec["source_path"]}')
            relative = f'originals/{sha}.{kind}'
            originals[relative] = body
            files[kind] = {'path': path, 'source_path': file_spec['source_path'], 'sha256': sha,
                           'size_bytes': len(body), 'original_path': relative}
        documents.append(normalize_document(spec, files, profiles[spec['adapter']]['profile']))
    links = defaultdict(list)
    titles = defaultdict(set)
    all_ids = set()
    counts = defaultdict(int)
    for document in documents:
        for record in document['records']:
            rid = record['record_id']
            if rid in all_ids:
                raise ValueError(f'duplicate normalized record identity: {rid}')
            all_ids.add(rid)
            counts[record['kind']] += 1
            for field in record['raw_fields']:
                for url in urls(field['value']):
                    links[canonical_url(url)].append({'record_id': rid, 'source_field': field['source_field'],
                                                       'cell': field['cell'], 'original_url': url})
            for field in record['reported_fields']:
                if field['target'] in ('subject.title', 'subject.label') and field['value']:
                    titles[str(field['value']).casefold().strip()].add(rid)
    link_index = [{'link_id': 'tos-research-link:' + digest(url.encode()), 'url': url,
                   'occurrences': refs, 'assessment_status': 'reported_unreviewed'} for url, refs in sorted(links.items())]
    correspondences = [{'basis': 'same_reported_title', 'label': title, 'record_ids': sorted(ids),
                        'status': 'unreviewed_possible_correspondence_no_identity_merge'}
                       for title, ids in sorted(titles.items()) if len(ids) > 1]
    processor = {name: digest((Path(__file__).parent / name).read_bytes()) for name in ('source_registry_common.py', 'source_registry_ooxml.py', 'source_registry_values.py')}
    snapshot_id = digest(encoded({'manifest': manifest, 'profiles': profiles, 'processor': processor,
                                 'originals': sorted(originals)}))
    summary = {'schema_version': 'tos_source_registry_snapshot_v1', 'snapshot_id': snapshot_id,
               'semantic_ceiling': 'research_reported_unreviewed', 'manifest': manifest, 'profiles': profiles, 'processor_sha256': processor,
               'counts': {**counts, 'documents': len(documents), 'docx': len(documents),
                          'paragraphs': sum(b['kind'] == 'paragraph' for d in documents for p in d['report_parts'] for b in p['blocks']),
                          'tables': sum(b['kind'] == 'table' for d in documents for p in d['report_parts'] for b in p['blocks']),
                          'links': len(link_index)},
               'documents': [f'documents/{d["corpus_id"]}/{d["document_id"]}.json.gz' for d in documents]}
    outputs = {'snapshot.json': encoded(summary), 'links.json.gz': gzip.compress(encoded(link_index), mtime=0),
               'correspondences.json.gz': gzip.compress(encoded(correspondences), mtime=0)}
    for document, name in zip(documents, summary['documents']):
        outputs[name] = gzip.compress(encoded(document), mtime=0)
    return summary, outputs, originals

def delta(previous_root, outputs):
    def records_from(root, data=None):
        if data is None:
            names = read(root / 'snapshot.json')['documents']
            docs = [read(root / name) for name in names]
        else:
            names = json.loads(data['snapshot.json'])['documents']
            docs = [json.loads(gzip.decompress(data[name]) if name.endswith('.gz') else data[name]) for name in names]
        return {r['record_id']: r for d in docs for r in d['records']}
    old = records_from(previous_root) if previous_root else {}
    new = records_from(None, outputs)
    old_summary = read(previous_root / 'snapshot.json') if previous_root else None
    old_docs = {(d['corpus_id'], d['document_id']): d for name in old_summary['documents'] for d in [read(previous_root / name)]} if previous_root else {}
    new_docs = {(d['corpus_id'], d['document_id']): d for name in json.loads(outputs['snapshot.json'])['documents'] for d in [json.loads(gzip.decompress(outputs[name]))]}
    file_changes = []
    report_changes = []
    for key in sorted(old_docs.keys() | new_docs.keys()):
        before, after = old_docs.get(key, {}), new_docs.get(key, {})
        for kind in ('xlsx', 'docx'):
            old_hash = before.get('files', {}).get(kind, {}).get('sha256')
            new_hash = after.get('files', {}).get(kind, {}).get('sha256')
            if old_hash != new_hash:
                file_changes.append({'corpus_id': key[0], 'document_id': key[1], 'kind': kind, 'before_sha256': old_hash, 'after_sha256': new_hash})
        def blocks(doc):
            return {(part['part'], b['xml_path']): {k: v for k, v in b.items() if k != 'occurrence_id'} for part in doc.get('report_parts', []) for b in part['blocks']}
        old_blocks, new_blocks = blocks(before), blocks(after)
        changed = [list(k) for k in sorted(old_blocks.keys() | new_blocks.keys()) if old_blocks.get(k) != new_blocks.get(k)]
        if changed:
            report_changes.append({'corpus_id': key[0], 'document_id': key[1], 'changed_block_locators': changed})
    return {'files': file_changes, 'reports': report_changes, 'added': sorted(new.keys() - old.keys()), 'removed': sorted(old.keys() - new.keys()),
            'changed': [{'record_id': rid, 'previous_occurrence': old[rid]['occurrence_id'],
                         'current_occurrence': new[rid]['occurrence_id'],
                         'changed_fields': sorted(k for k in set({f['source_field'] for f in old[rid]['raw_fields']}) | set({f['source_field'] for f in new[rid]['raw_fields']})
                         if {f['source_field']: {a: b for a, b in f.items() if a not in ('cell', 'style')} for f in old[rid]['raw_fields']}.get(k) != {f['source_field']: {a: b for a, b in f.items() if a not in ('cell', 'style')} for f in new[rid]['raw_fields']}.get(k))}
                        for rid in sorted(new.keys() & old.keys()) if old[rid]['content_sha256'] != new[rid]['content_sha256']],
            'relocated': sorted(rid for rid in new.keys() & old.keys()
                                if old[rid]['content_sha256'] == new[rid]['content_sha256'] and old[rid]['source']['row'] != new[rid]['source']['row']),
            'unchanged_count': sum(old[rid]['content_sha256'] == new[rid]['content_sha256'] for rid in new.keys() & old.keys())}

def write_immutable(path, body):
    if path.exists():
        if path.read_bytes() != body:
            raise ValueError(f'immutable output collision: {path}')
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as stream:
        stream.write(body)

def run(packet_root, input_root=None, check=False):
    summary, outputs, originals = build(packet_root, input_root)
    snapshot = packet_root / 'snapshots' / summary['snapshot_id']
    current_path = packet_root / 'current.json'
    current = read(current_path) if current_path.exists() else None
    if check:
        if not current or current['snapshot_id'] != summary['snapshot_id']:
            raise ValueError('current snapshot identity drift')
        for name, body in outputs.items():
            if (snapshot / name).read_bytes() != body:
                raise ValueError(f'generated snapshot drift: {name}')
        for name, body in originals.items():
            if (packet_root / name).read_bytes() != body:
                raise ValueError(f'original byte drift: {name}')
        return summary
    previous = packet_root / 'snapshots' / current['snapshot_id'] if current and current['snapshot_id'] != summary['snapshot_id'] else None
    for name, body in originals.items():
        write_immutable(packet_root / name, body)
    for name, body in outputs.items():
        write_immutable(snapshot / name, body)
    if not (snapshot / 'delta.json').exists():
        write_immutable(snapshot / 'delta.json', encoded({'previous_snapshot_id': current['snapshot_id'] if previous else None,
                                                       **delta(previous, outputs)}))
    current_path.write_bytes(encoded({'schema_version': 'tos_source_registry_pointer_v1', 'snapshot_id': summary['snapshot_id']}))
    return summary

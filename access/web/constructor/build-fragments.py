#!/usr/bin/env python3
"""Assemble a separate recording reader from explicitly reviewed demo fragments.

This never republishes the old private source library. It preserves seven local
navigation IDs and their parent closure, replacing their display metadata with
references to the newly selected editions. Corpus sources are not modified.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path

SOURCE_IDS = ['work', 'chapter-p3.r2', 'moment', 'chapter-p3.r13', 'all-things', 'same-life', 'dossier']

def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

def encode(value) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode()

def load(path: Path):
    raw = path.read_bytes()
    if len(raw) > 8_000_000:
        raise ValueError('Input exceeds the 8 MB reader bound')
    return json.loads(raw), {'name': path.name, 'sha256': digest(raw), 'bytes': len(raw)}

def bi(ru, en):
    return {'ru': ru, 'en': en}

def assemble(source, passages, bindings):
    by_id = {item['id']: item for item in passages}
    if len(by_id) != len(passages):
        raise ValueError('Duplicate passage IDs')
    by_node = {item['nodeId']: item for item in bindings}
    if len(by_node) != len(bindings):
        raise ValueError('Duplicate material bindings')
    for binding in bindings:
        if not binding['passageIds'] or any(identity not in by_id for identity in binding['passageIds']):
            raise ValueError('Unresolved passage binding: ' + binding['nodeId'])
    for passage in passages:
        if passage['status'] == 'available':
            if passage.get('complete') is not True or set(passage.get('versions', {})) != {'ru', 'en'}:
                raise ValueError('A displayed source unit must be complete and bilingual')
            for code, version in passage['versions'].items():
                text = '\n\n'.join(version['paragraphs'])
                if not text.strip() or digest(text.encode()) != version['textSha256']:
                    raise ValueError('Text digest mismatch: ' + passage['id'] + '/' + code)
                if not {'local-reading', 'video-display'} <= set(version['rights'].get('uses', [])):
                    raise ValueError('Video display basis is missing: ' + passage['id'] + '/' + code)
        elif passage['status'] == 'link-only':
            if 'versions' in passage:
                raise ValueError('Unavailable passages cannot contain hidden text')
        else:
            raise ValueError('Unknown passage status')
    catalog = {'schema': 'tos_demo_fragments_v1', 'audience': 'local-reading-and-recorded-video', 'passages': passages, 'bindings': bindings}
    catalog_bytes = encode(catalog)
    catalog_ref = {'path': 'assets/fragments-' + digest(catalog_bytes)[:16] + '.json', 'sha256': digest(catalog_bytes)}
    originals = {item['id']: item for item in source['nodes']}
    needed = set(SOURCE_IDS)
    for identity in SOURCE_IDS:
        if identity not in originals or identity not in by_node:
            raise ValueError('Source navigation or fragment binding is missing: ' + identity)
        parent = originals[identity]['parentId']
        seen = {identity}
        while parent is not None:
            if parent not in originals or parent in seen:
                raise ValueError('Invalid source parent closure')
            seen.add(parent); needed.add(parent); parent = originals[parent]['parentId']
    nodes = []
    for identity in sorted(needed):
        original = originals[identity]
        binding = by_node.get(identity)
        source_refs = []
        if binding:
            for passage_id in binding['passageIds']:
                passage = by_id[passage_id]
                if passage['status'] == 'available':
                    for code, version in passage['versions'].items():
                        source_refs.append({'label': passage['title'][code] + ' · ' + code.upper(), 'ref': version['sourceUrl']})
                else:
                    source_refs.extend({'label': link['label'], 'ref': link['url']} for link in passage['links'])
        # Strict field selection: exact/quote/speaker/private paths cannot leak from
        # the older local-only library, including its unlisted archival materials.
        nodes.append({'id': identity, 'kind': original['kind'], 'parentId': original['parentId'],
            'title': original['title'], 'body': binding['context'] if binding else bi('Часть III книги.', 'Part III of the book.'),
            'sourceRefs': source_refs, 'sourceNote': bi(
                'Полные разделы в читалке подписаны именами переводчиков и точными изданиями. Русская и английская версии сохраняют собственные границы абзацев.',
                'The reader credits each complete section to its translator and specific edition. Russian and English retain their own paragraph divisions.')})
    library = {'schema': 'tos_constructor_library_v1', 'rootId': source['rootId'], 'nodes': nodes,
        'fragmentCatalog': catalog_ref, 'displayProfile': 'recorded-demo-selected-editions-v1'}
    library['fingerprint'] = digest(encode(library))
    return catalog, catalog_bytes, library

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-library', required=True, type=Path)
    parser.add_argument('--passages', required=True, type=Path, nargs='+')
    parser.add_argument('--bindings', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--receipt', required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if (output / 'library.json').exists() or (output / 'constructor.html').exists():
        raise ValueError('Refusing to replace an existing release; select a new immutable directory')
    source, source_receipt = load(args.source_library)
    passages, input_receipts = [], []
    for path in args.passages:
        data, receipt = load(path)
        passages.extend(data if isinstance(data, list) else data['passages']); input_receipts.append(receipt)
    data, binding_receipt = load(args.bindings)
    bindings = data if isinstance(data, list) else data['bindings']
    catalog, catalog_bytes, library = assemble(source, passages, bindings)
    (output / 'assets').mkdir(parents=True, exist_ok=True)
    (output / library['fragmentCatalog']['path']).write_bytes(catalog_bytes)
    (output / 'library.json').write_bytes(encode(library))
    os.chmod(output / 'library.json', 0o600)
    receipt = {'schema': 'tos_demo_fragment_assembly_v1', 'input_library': source_receipt, 'inputs': input_receipts,
        'bindings_input': binding_receipt, 'output': str(output), 'catalog': library['fragmentCatalog'],
        'source_navigation_count': len(library['nodes']), 'material_binding_count': len(bindings),
        'available_passages': [p['id'] for p in passages if p['status'] == 'available'],
        'link_only_passages': [p['id'] for p in passages if p['status'] == 'link-only'],
        'historical_private_payload_copied': False, 'corpus_modified': False,
        'limits': 'Mechanical assembly only; source quality and rights reasoning require their separate review records.'}
    args.receipt.parent.mkdir(parents=True, exist_ok=True); args.receipt.write_bytes(encode(receipt))
    print(json.dumps({key: value for key, value in receipt.items() if key not in {'inputs', 'input_library'}}, ensure_ascii=False))

if __name__ == '__main__':
    main()

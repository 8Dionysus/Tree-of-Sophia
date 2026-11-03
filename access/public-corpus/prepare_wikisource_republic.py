#!/usr/bin/env python3
"""Prepare one source-bound Republic reading from ten captured Wikisource books.

Each input book supplies its exact parse API response, render, article revision,
and independently rechecked transcluded-page revisions. No network or textual
correction is performed here. The retained API responses preserve the source.
"""
import argparse
import collections
import datetime
import json
import pathlib
import urllib.parse

from bs4 import BeautifulSoup
import prepare_wikisource_karpov as karpov

retained = karpov.retained
ROOT = karpov.ROOT


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--capture-dir', required=True, type=pathlib.Path)
    capture = parser.parse_args().capture_dir.resolve(strict=True)
    snapshots, responses, sections = [], [], []
    for i in range(1, 11):
        snapshot = json.loads((capture / f'book-{i:02}.json').read_text())
        raw = (capture / f'book-{i:02}-api.json').read_bytes()
        render = json.loads(raw)['parse']
        body = BeautifulSoup(render['text']['*'], 'html.parser').select_one('.prp-pages-output')
        if snapshot['book'] != i or snapshot['work'] != 'republic' or str(render['revid']) != str(snapshot['article_revid']) or str(body) != snapshot['html']:
            raise ValueError(f'book {i}: capture identity, revision or text mismatch')
        if not snapshot['pages'] or any(not p.get('revid') or p.get('quality') not in ('3', '4') for p in snapshot['pages']):
            raise ValueError(f'book {i}: page revision or proofreading status missing')
        book_sections = karpov.extract(snapshot['html'])
        if sections and int(book_sections[0]['id']) <= int(sections[-1]['id']):
            raise ValueError(f'book {i}: overlapping or out-of-order Stephanus sections')
        sections.extend(book_sections)
        snapshots.append(snapshot)
        responses.append(raw)
    all_pages = [p for snapshot in snapshots for p in snapshot['pages']]
    counts = dict(collections.Counter(p['quality'] for p in all_pages))
    limit = (f"Ten book transclusions, {len(all_pages)} transcluded pages; ProofreadPage quality counts {counts}. "
             "Scan images were not collated. A Wikisource template error outside the reading body was observed. Automatic alignment is unreviewed.")
    retained.REPO = pathlib.Path('.')
    retained.LEGACY = capture
    retained.MADE_AT = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')
    retained.MINT_IDENTITIES = True
    source_url = 'https://ru.wikisource.org/w/index.php?title=' + urllib.parse.quote('Политика_или_государство_(Платон;_Карпов)') + '&oldid=5642624'
    data = {'header':'<teiHeader><title>Сочинения Платона</title><author>Платон</author><translator>В. Н. Карпов</translator></teiHeader>',
            'sections':sections, 'notes':[], 'adjustments':[], 'source_snapshots':snapshots}
    stage = capture/'texts'
    stage.mkdir(exist_ok=True)
    staged = stage/'republic-ru-karpov1863-wikisource.json'
    if staged.exists() and staged.read_bytes() != retained.json_bytes(data):
        raise ValueError(f'will not overwrite different staged data: {staged}')
    staged.write_bytes(retained.json_bytes(data))
    version = {'id':'republic-ru-karpov1863-wikisource', 'language':'ru', 'credit':'Перевод: В. Н. Карпов', 'creditRole':'translator',
        'license':karpov.LICENSE, 'source':source_url, 'publication':{'titles':['Сочинения Платона, том 3'], 'dates':['1863']},
        'attribution':'Платон; перевод В. Н. Карпова (1863); участники русской Викитеки (история десяти книг и включённых страниц скана).',
        'coverageLimit':limit, 'proofreadingStatus':str(counts), 'coverageStatus':'complete-source-transclusion', 'pinnedPageRevisions':all_pages}
    files, details = retained.output_for({'id':'republic','author':'plato','title':'Государство'},version,data)
    item = details['item_dir_ref']
    for ref, note in (
        (f"{item.split('/editions/')[0]}/expression.json", 'A separately captured Wikisource translation; no equivalence to another local edition or review is claimed.'),
        (f"{item.split('/items/')[0]}/edition.json", 'Captured Wikisource book renderings and their bounded public reading derivative; the exact parse API responses are retained with the item.'),
    ):
        record = json.loads(files[ref]); record['notes'] = note; files[ref] = retained.json_bytes(record)
    manifest = json.loads(files[f'{item}/item.manifest.json'])
    outputs = []
    article_records = []
    for i, (snapshot, raw) in enumerate(zip(snapshots, responses), 1):
        rel = f'payload/wikisource-book-{i:02}-api.json'
        ref = f'{item}/{rel}'
        sha = retained.sha(raw)
        files[ref] = raw
        manifest['payload_files'].append({'file_id':f'tos.file.sha256.{sha}', 'relative_path':rel, 'original_basename':rel.split('/')[-1],
            'media_type':'application/json', 'byte_size':len(raw), 'sha256':sha, 'fixity_verified_at':retained.MADE_AT})
        files[f'{item}/fixity.sha256'] += f'{sha}  {rel}\n'.encode()
        outputs.append({'ref':ref,'sha256':sha,'role':'wikisource_book_api_response'})
        url = 'https://ru.wikisource.org/w/index.php?title=' + urllib.parse.quote(snapshot['title'].replace(' ', '_')) + '&oldid=' + str(snapshot['article_revid'])
        article_records.append({'book':i, 'article_url':url, 'article_oldid':snapshot['article_revid'],
            'captured_at':snapshot['captured_at'], 'transcluded_pages':snapshot['pages'],
            'outside_body_error_count':snapshot['outside_body_error_count'], 'raw_api_response_ref':ref, 'raw_api_response_sha256':sha})
    files[f'{item}/item.manifest.json'] = retained.json_bytes(manifest)
    metadata = {'schema_version':'tos_public_source_metadata_snapshot_v1','record_version':1,'item_id':details['item_id'],
        'captured_at':snapshots[-1]['captured_at'], 'source':{'provider':'Russian Wikisource','work':'Государство',
        'translator':'Василий Николаевич Карпов (1798–1867)','edition':'Сочинения Платона, том 3, 1863',
        'article_url':source_url,'books':article_records,'page_quality_counts':counts,'scan_collation_performed':False},
        'rights_evidence':karpov.RIGHTS, 'limitations':'Each article oldid does not freeze included pages or templates. Page revisions were checked before and after an identical render; each API response is retained. Page status does not certify accuracy.'}
    meta_ref = f'{item}/source-metadata-snapshot.json'
    files[meta_ref] = retained.json_bytes(metadata)
    rights_ref = f'{item}/rights.json'
    rights = json.loads(files[rights_ref])
    rights['jurisdictions_reviewed'] = ['RU','US']
    rights['source_refs'] = [source_url] + [a['article_url'] for a in article_records] + karpov.RIGHTS
    rights['scope_refs'].extend('tos.file.sha256.' + o['sha256'] for o in outputs)
    rights['rationale'] = 'Historical translation published 1863; translator died 1867. Public domain in Russia and the United States by age. Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'
    files[rights_ref] = retained.json_bytes(rights)
    prov = [json.loads(line) for line in files[f'{item}/provenance.jsonl'].decode().splitlines()]
    prov[0]['method']['name'] = 'captured-wikisource-book-renders-and-page-revisions'
    prov[0]['method']['configuration']['purpose'] = 'Exact API responses and page revision lists retained; article oldids alone do not freeze transclusions.'
    prov[0]['inputs'] = []
    prov[0]['outputs'] = outputs + [{'ref':f'{item}/payload/retained-reading.json','sha256':details['source_json_sha'],'role':'captured_and_extracted_representation'}]
    prov[1]['method']['configuration']['purpose'] = 'Extracts dialogue text nodes; excludes headings, navigation, note references and Stephanus margin markers; inserts paragraph separators. No spelling corrections. All ten API responses are retained intact.'
    prov[3]['method']['configuration']['purpose'] = 'Rights assessment based on the identified 19th-century translation and Wikisource transcription license; no text or canon acceptance.'
    prov[3]['inputs'] = [{'ref':meta_ref,'sha256':retained.sha(files[meta_ref]),'role':'rights_source_metadata_snapshot'}]
    prov[3]['outputs'][0]['sha256'] = retained.sha(files[rights_ref])
    files[f'{item}/provenance.jsonl'] = (''.join(json.dumps(e,ensure_ascii=False,separators=(',',':'))+'\n' for e in prov)).encode()
    files[f'{item}/forensic-report.md'] = (f'# Wikisource acquisition: Государство\n\n- Index: {source_url}\n- Ten book article oldids, page revisions and response digests: source metadata snapshot.\n- {limit}\n- Page revisions and article render checked twice. Oldids alone do not pin transclusions or templates; all exact API responses retained.\n- Reading layer excludes headings, navigation, note references and marginal marks. It preserves displayed dialogue and OCR errors; Stephanus marks may fall inside sentences.\n').encode()
    version_record = details['version_record']
    version_record['rightsSha256'] = retained.sha(files[rights_ref])
    selection_path = ROOT/'access/public-corpus/selection.v1.json'
    selection = json.loads(selection_path.read_text())
    doc = next(d for d in selection['documents'] if d['id'] == details['work_id'])
    if any(v['id'] == version_record['id'] for v in doc['versions']): raise ValueError('version already selected')
    for ref in files:
        if (ROOT/ref).exists(): raise ValueError(f'will not overwrite {ref}')
    for ref, content in files.items():
        path = ROOT/ref
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(content)
    doc['versions'].append(version_record)
    selection_path.write_bytes(retained.json_bytes(selection))
    print(f'republic: {len(sections)} sections, {len(all_pages)} page transclusions, {counts}')


if __name__ == '__main__':
    main()

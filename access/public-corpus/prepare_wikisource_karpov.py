#!/usr/bin/env python3
"""Prepare locally captured, revision-checked Wikisource transclusions of Karpov's Plato.

Input is an explicit directory containing WORK.json (page revisions and DOM
snapshot) and WORK-api.json (the exact parse API response), captured after a
second identical render and a recheck of every included page revision. This
one-off intake keeps both bytes and metadata as source-item payloads. It uses
BeautifulSoup 4 for HTML parsing; it does not fetch or correct text.
"""
import argparse
import collections
import datetime
import importlib.util
import json
import pathlib
import re
import urllib.parse
from bs4 import BeautifulSoup, NavigableString, Tag

ROOT = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('retained', ROOT / 'access/public-corpus/prepare_retained_corpus.py')
retained = importlib.util.module_from_spec(spec)
spec.loader.exec_module(retained)
LICENSE = 'https://creativecommons.org/licenses/by-sa/4.0/'
RIGHTS = ['https://search.rsl.ru/ru/record/01003507492',
          'https://ru.wikisource.org/wiki/Сочинения_Платона_(Платон;_Карпов)',
          'https://ru.wikisource.org/wiki/Викитека:Авторские_права', LICENSE,
          'https://www.copyright.gov/circs/circ22.pdf',
          'https://www.consultant.ru/document/cons_doc_LAW_64629/675b81f0bc9b8ecd68ea332ecfd9d1562541fb98/']
WORKS = {'phaedo':'Федон', 'phaedrus':'Федр', 'symposium':'Пир', 'timaeus':'Тимей',
         'sophist':'Софист', 'protagoras':'Протагор', 'theaetetus':'Теэтет',
         'gorgias':'Горгиас', 'meno':'Менон', 'parmenides':'Парменид', 'philebus':'Филеб',
         'cratylus':'Кратил', 'ion':'Ион', 'euthyphro':'Эвтифрон', 'laches':'Лахес', 'lysis':'Лизис'}


def extract(html):
    body = BeautifulSoup(html, 'html.parser').select_one('.prp-pages-output')
    if body is None:
        raise ValueError('missing transcluded text body')
    parts, prefix, current = {}, [], None
    def add(text):
        (prefix if current is None else parts[current]).append(text)
    def walk(node):
        nonlocal current, prefix
        if isinstance(node, NavigableString):
            add(str(node)); return
        if not isinstance(node, Tag): return
        if set(node.get('class', [])) & {'pagenumber', 'reference', 'mw-editsection', 'noprint'} or node.name in ('style', 'script', 'math'):
            return
        marker = re.fullmatch(r'(\d+)[a-e]', node.get('id', ''))
        if marker:
            number = int(marker.group(1))
            if current is None:
                current = number; parts.setdefault(current, []).extend(prefix); prefix = []
            elif 0 < number - current <= 2:
                current = number; parts.setdefault(current, [])
            return
        if re.fullmatch(r'\d+[A-E]', node.get('id', '')): return
        for child in node.children: walk(child)
        if node.name in ('p', 'div', 'tr', 'br'): add('\n')
    started = False
    for child in body.children:
        if isinstance(child, Tag) and child.name == 'p' and not started:
            probe = BeautifulSoup(str(child), 'html.parser')
            for item in probe.select('.pagenumber,.reference'): item.decompose()
            if probe.get_text(strip=True): started = True
        if started: walk(child)
    if not parts or prefix: raise ValueError('no Stephanus-marked dialogue')
    return [{'id':str(number), 'label':str(number), 'group':'',
             'blocks':[{'text':''.join(text)}], 'notes':[], 'headings':[], 'variants':[]}
            for number, text in parts.items() if ''.join(text).strip()]


def prepare(work, capture, selection):
    snapshot = json.loads((capture / f'{work}.json').read_text())
    raw = (capture / f'{work}-api.json').read_bytes()
    rendered = json.loads(raw)['parse']
    if snapshot['work'] != work or rendered['revid'] != snapshot['article_revid']:
        raise ValueError('capture identity/revision mismatch')
    if str(BeautifulSoup(rendered['text']['*'], 'html.parser').select_one('.prp-pages-output')) != snapshot['html']:
        raise ValueError('capture rendered text mismatch')
    pages = snapshot['pages']
    if not pages or any(not page.get('revid') or page.get('quality') not in ('3', '4') for page in pages):
        raise ValueError('missing page revision or insufficient proofreading')
    volume = int(re.search(r'Том (\d+),', pages[0]['title']).group(1))
    if any(f'Том {volume},' not in p['title'] for p in pages): raise ValueError('mixed source volumes')
    year = 1863 if volume <= 4 else 1879
    sections = extract(snapshot['html'])
    ident = f'{work}-ru-karpov{year}-wikisource'
    data = {'header':'<teiHeader><title>Сочинения Платона</title><author>Платон</author><translator>В. Н. Карпов</translator></teiHeader>',
            'sections':sections, 'notes':[], 'adjustments':[], 'source_snapshot':snapshot}
    stage = capture / 'texts'; stage.mkdir(exist_ok=True)
    (stage / f'{ident}.json').write_bytes(retained.json_bytes(data))
    retained.REPO = pathlib.Path('.')
    retained.LEGACY = capture
    retained.MADE_AT = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')
    retained.MINT_IDENTITIES = True
    url = 'https://ru.wikisource.org/w/index.php?title=' + urllib.parse.quote(snapshot['title'].replace(' ', '_')) + '&oldid=' + str(snapshot['article_revid'])
    counts = dict(collections.Counter(p['quality'] for p in pages))
    limit = f"{len(pages)} transcluded pages; ProofreadPage quality counts {counts}. Scan images were not collated. Automatic alignment is unreviewed."
    version = {'id':ident, 'language':'ru', 'credit':'Перевод: В. Н. Карпов', 'creditRole':'translator',
               'license':LICENSE, 'source':url, 'publication':{'titles':[f'Сочинения Платона, том {volume}'], 'dates':[str(year)]},
               'attribution':f'Платон; перевод В. Н. Карпова ({year}); участники русской Викитеки (история указанной страницы и включённых страниц скана).',
               'coverageLimit':limit, 'proofreadingStatus':str(counts), 'coverageStatus':'complete-source-transclusion', 'pinnedPageRevisions':pages}
    book = {'id':work, 'author':'plato', 'title':WORKS[work]}
    files, details = retained.output_for(book, version, data)
    for ref, note in (
        (f"{details['item_dir_ref'].split('/editions/')[0]}/expression.json", 'A separately captured Wikisource translation; no equivalence to another local edition or review is claimed.'),
        (f"{details['item_dir_ref'].split('/items/')[0]}/edition.json", 'A captured Wikisource rendering and its bounded public reading derivative; the exact parse API response is retained with the item.'),
    ):
        record = json.loads(files[ref]); record['notes'] = note; files[ref] = retained.json_bytes(record)
    item = details['item_dir_ref']; api_ref = f'{item}/payload/wikisource-api.json'; api_sha = retained.sha(raw)
    files[api_ref] = raw
    manifest = json.loads(files[f'{item}/item.manifest.json'])
    manifest['payload_files'].append({'file_id':f'tos.file.sha256.{api_sha}', 'relative_path':'payload/wikisource-api.json',
        'original_basename':'wikisource-api.json', 'media_type':'application/json', 'byte_size':len(raw), 'sha256':api_sha, 'fixity_verified_at':retained.MADE_AT})
    files[f'{item}/item.manifest.json'] = retained.json_bytes(manifest)
    files[f'{item}/fixity.sha256'] += f'{api_sha}  payload/wikisource-api.json\n'.encode()
    metadata = {'schema_version':'tos_public_source_metadata_snapshot_v1', 'record_version':1, 'item_id':details['item_id'],
      'captured_at':snapshot['captured_at'], 'source':{'provider':'Russian Wikisource', 'work':WORKS[work],
      'translator':'Василий Николаевич Карпов (1798–1867)', 'edition':f'Сочинения Платона, том {volume}, {year}',
      'article_url':url, 'article_oldid':snapshot['article_revid'], 'transcluded_pages':pages, 'page_quality_counts':counts,
      'scan_collation_performed':False, 'raw_api_response_ref':api_ref, 'raw_api_response_sha256':api_sha}, 'rights_evidence':RIGHTS,
      'limitations':'Article oldid does not freeze included pages or templates. Page revisions were checked before and after an identical render; the API response is retained. Page status does not certify accuracy.'}
    meta_ref = f'{item}/source-metadata-snapshot.json'; files[meta_ref] = retained.json_bytes(metadata)
    rights_ref = f'{item}/rights.json'; rights = json.loads(files[rights_ref]);rights['jurisdictions_reviewed']=['RU','US']
    rights['source_refs']=[url]+RIGHTS;rights['scope_refs'].append('tos.file.sha256.'+api_sha)
    rights['rationale']=f'Historical translation published {year}; translator died 1867. Public domain in Russia and the United States by age. Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'
    files[rights_ref]=retained.json_bytes(rights)
    prov=[json.loads(line) for line in files[f'{item}/provenance.jsonl'].decode().splitlines()]
    prov[0]['method']['name']='captured-wikisource-render-and-page-revisions'
    prov[0]['method']['configuration']['purpose']='Exact API response and page revision list retained; article oldid alone does not freeze transclusions.'
    prov[0]['inputs']=[];prov[0]['outputs']=[{'ref':api_ref,'sha256':api_sha,'role':'wikisource_api_response'}, {'ref':f'{item}/payload/retained-reading.json','sha256':details['source_json_sha'],'role':'captured_and_extracted_representation'}]
    prov[1]['method']['configuration']['purpose']='Extracts dialogue text nodes; excludes cast list, page navigation, note references and Stephanus margin markers; inserts paragraph separators. No spelling corrections. The API response is retained intact.'
    prov[3]['method']['configuration']['purpose']='Rights assessment based on the identified 19th-century translation and the Wikisource transcription license; no text or canon acceptance.'
    prov[3]['inputs']=[{'ref':meta_ref,'sha256':retained.sha(files[meta_ref]),'role':'rights_source_metadata_snapshot'}]
    prov[3]['outputs'][0]['sha256']=retained.sha(files[rights_ref])
    files[f'{item}/provenance.jsonl']=(''.join(json.dumps(e,ensure_ascii=False,separators=(',',':'))+'\n' for e in prov)).encode()
    files[f'{item}/forensic-report.md']=(f'# Wikisource acquisition: {WORKS[work]}\n\n- Source: {url}\n- Exact API response SHA-256: `{api_sha}`; included pages and oldids: source metadata snapshot.\n- {limit}\n- Page revisions and article render checked twice. Article oldid alone does not pin transclusions or templates; exact API response retained.\n- Reading layer excludes cast list, navigation, note references and marginal marks. It preserves displayed dialogue and OCR errors; Stephanus marks may fall inside sentences.\n').encode()
    v=details['version_record'];v['rightsSha256']=retained.sha(files[rights_ref])
    doc=next(d for d in selection['documents'] if d['id']==details['work_id'])
    if any(x['id']==v['id'] for x in doc['versions']):raise ValueError(f'existing version {v["id"]}')
    for ref, content in files.items():
        path=ROOT/ref
        if path.exists():raise ValueError(f'will not overwrite {ref}')
        path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(content)
    doc['versions'].append(v)
    print(work, len(sections), len(pages), counts, flush=True)
    return details


def main():
    p=argparse.ArgumentParser();p.add_argument('--capture-dir',type=pathlib.Path,required=True);p.add_argument('works',nargs='+',choices=WORKS)
    args=p.parse_args();capture=args.capture_dir.resolve(strict=True);selection_path=ROOT/'access/public-corpus/selection.v1.json';selection=json.loads(selection_path.read_text())
    # Save after each completed work so interruption leaves an inspectable, selected intake.
    for work in args.works:
        prepare(work,capture,selection);selection_path.write_bytes(retained.json_bytes(selection))

if __name__ == '__main__': main()

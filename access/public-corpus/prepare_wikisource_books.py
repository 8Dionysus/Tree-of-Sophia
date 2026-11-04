#!/usr/bin/env python3
"""Prepare Russian Wikisource translations of Antiquity works as retained readings.

Input is a capture directory with, per work, `capture.json` (page titles,
revisions and capture times) and the exact parse API response of every page
(`NN-api.json`). The reading layer keeps the text as Wikisource renders it:
headers, navigation, note markers, notes and marginal line numbers are left
out, nothing is corrected. A work is divided as its original is wherever the
translation marks the same places (the chapters of the Metaphysics and the
Poetics); otherwise by book, and the stand's alignment pairs the sentences.
"""
import argparse
import collections
import datetime
import json
import pathlib
import re
import urllib.parse

from bs4 import BeautifulSoup, Comment, NavigableString, Tag
import prepare_wikisource_karpov as karpov

retained = karpov.retained
ROOT = karpov.ROOT
LICENSE = karpov.LICENSE
LAW = ['https://ru.wikisource.org/wiki/Викитека:Авторские_права', LICENSE,
       'https://www.copyright.gov/circs/circ22.pdf',
       'https://www.consultant.ru/document/cons_doc_LAW_64629/675b81f0bc9b8ecd68ea332ecfd9d1562541fb98/']

UNITS = {'первая': 1, 'вторая': 2, 'третья': 3, 'четвертая': 4, 'пятая': 5, 'шестая': 6, 'седьмая': 7,
         'восьмая': 8, 'девятая': 9, 'десятая': 10, 'одиннадцатая': 11, 'двенадцатая': 12, 'тринадцатая': 13,
         'четырнадцатая': 14, 'пятнадцатая': 15, 'шестнадцатая': 16, 'семнадцатая': 17, 'восемнадцатая': 18,
         'девятнадцатая': 19, 'двадцатая': 20, 'тридцатая': 30}
TENS = {'двадцать': 20, 'тридцать': 30}
ROMAN = {'I': 1, 'V': 5, 'X': 10, 'L': 50}

WORKS = {
    'metaphysics': {'author': 'aristotle', 'title': 'Метафизика', 'split': 'chapters',
        'id': 'metaphysics-ru-kubitsky1934-wikisource', 'translator': 'Александр Владиславович Кубицкий (1880–1937)',
        'credit': 'Перевод: А. В. Кубицкий', 'edition': 'Аристотель. Метафизика. М.; Л.: Соцэкгиз, 1934', 'year': '1934',
        'attribution': 'Аристотель; перевод А. В. Кубицкого (1934); участники русской Викитеки (история указанных страниц).',
        'rationale': 'Translation first published 1934; the translator died 1937. Public domain in Russia (70 years after death). The Russian term had expired before 1 January 1996, so the translation was not restored in the United States. Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'},
    'poetics': {'author': 'aristotle', 'title': 'Поэтика', 'split': 'roman',
        'id': 'poetics-ru-novosadsky1927-wikisource', 'translator': 'Николай Иванович Новосадский (1859–1941)',
        'credit': 'Перевод: Н. И. Новосадский', 'edition': 'Аристотель. Поэтика. Л.: Academia, 1927', 'year': '1927',
        'attribution': 'Аристотель; перевод Н. И. Новосадского (1927); участники русской Викитеки (история указанной страницы).',
        'rationale': 'Translation published 1927; the translator died 1941. Public domain in Russia (70 years after death) and in the United States (published before 1931). Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'},
    'politics': {'author': 'aristotle', 'title': 'Политика', 'split': 'books', 'books': [1, 1, 1, 2, 2, 2, 2, 3, 4],
        'margins': True, 'id': 'politics-ru-zhebelev1911-wikisource', 'translator': 'Сергей Александрович Жебелёв (1867–1941)',
        'credit': 'Перевод: С. А. Жебелёв', 'edition': 'Аристотель. Политика. СПб., 1911', 'year': '1911',
        'attribution': 'Аристотель; перевод С. А. Жебелёва (1911); участники русской Викитеки (история указанных страниц).',
        'coverage': 'Books I–IV of eight are transcribed on Wikisource; books V–VIII have no Russian text here.',
        'rationale': 'Translation published 1911; the translator died 1941. Public domain in Russia (70 years after death) and in the United States (published before 1931). Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'},
    'laws': {'author': 'plato', 'title': 'Законы', 'split': 'books',
        'id': 'laws-ru-obolensky1827-wikisource', 'translator': 'Василий Иванович Оболенский',
        'credit': 'Перевод: В. И. Оболенский', 'edition': 'Платоновы разговоры о законах. М., 1827', 'year': '1827',
        'attribution': 'Платон; перевод В. И. Оболенского (1827); участники русской Викитеки (история указанных страниц).',
        'rationale': 'Translation published 1827. Public domain in Russia and the United States by age. Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'},
    'meditations': {'author': 'marcus', 'title': 'К самому себе', 'split': 'books',
        'id': 'meditations-ru-rogovin1914-wikisource', 'translator': 'Сергей Мартынович Роговин',
        'credit': 'Перевод: С. М. Роговин', 'edition': 'Марк Аврелий. Наедине с собой. Размышления. М., 1914', 'year': '1914',
        'attribution': 'Марк Аврелий; перевод С. М. Роговина (1914); участники русской Викитеки (история указанных страниц).',
        'margins': True, 'rationale': 'Translation published 1914; Wikisource marks it PD-RusEmpire (published before 7 November 1917). Public domain in Russia and in the United States (published before 1931). Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'},
}

SKIP_CLASSES = {'reference', 'mw-editsection', 'noprint', 'ws-noexport', 'navbox', 'reflist', 'references',
                'mw-references-wrap', 'licenseContainer', 'ws-footer', 'pagenumber'}
SKIP_IDS = {'headertemplate', 'textquality', 'ws-data'}
HEAD = '\x00H'


def ordinal(words):
    words = words.lower().replace('ё', 'е').split()
    if len(words) == 2 and words[0] in TENS and words[1] in UNITS:
        return TENS[words[0]] + UNITS[words[1]]
    if len(words) == 1 and words[0] in UNITS:
        return UNITS[words[0]]
    return None


def roman(value):
    total = 0
    for a, b in zip(value, value[1:] + ' '):
        total += -ROMAN[a] if ROMAN.get(b, 0) > ROMAN[a] else ROMAN[a]
    return total


def lines_of(html):
    """The rendered text of a page, a line per paragraph; headings marked."""
    root = BeautifulSoup(html, 'html.parser').select_one('.mw-parser-output')
    out = []

    def walk(node):
        if isinstance(node, Comment):
            return
        if isinstance(node, NavigableString):
            out.append(str(node))
            return
        if not isinstance(node, Tag) or node.name in ('style', 'script', 'math', 'meta', 'link'):
            return
        if set(node.get('class', [])) & SKIP_CLASSES or node.get('id') in SKIP_IDS or node.name in ('sup',) and 'reference' in node.get('class', []):
            return
        if node.name in ('h1', 'h2', 'h3', 'h4', 'h5'):
            out.append('\n' + HEAD + node.get_text(' ', strip=True) + '\n')
            return
        for child in node.children:
            walk(child)
        if node.name in ('p', 'div', 'dd', 'li', 'tr', 'br', 'center'):
            out.append('\n')

    walk(root)
    return [re.sub(r'[ \t ]+', ' ', line).strip() for line in ''.join(out).split('\n')]


def section(group, label, paragraphs):
    text = '\n'.join(p for p in paragraphs if p)
    return {'id': f'{group} {label}'.strip(), 'label': label, 'group': group,
            'blocks': [{'text': text}], 'notes': [], 'headings': [], 'variants': []}


def sections_for(work, config, pages):
    sections = []
    for n, page in enumerate(pages, 1):
        lines = lines_of(page['html'])
        # Notes follow their heading at the end of a page; they are not the work's text.
        for i, line in enumerate(lines):
            if line.startswith(HEAD) and line[len(HEAD):].strip().lower() == 'примечания':
                lines = lines[:i]
                break
        # Marginal line numbers (Politics) and note numbers (Meditations) stand in parentheses.
        if config.get('margins'):
            lines = [re.sub(r'\s*\(\d{1,3}\)\s*', ' ', line).strip() for line in lines]
        if config['split'] == 'books':
            book = config.get('books', list(range(1, len(pages) + 1)))[n - 1]
            body = [line for line in lines if line and not line.startswith(HEAD)]
            if sections and sections[-1]['label'] == f'Книга {book}':
                sections[-1]['blocks'][0]['text'] += '\n' + '\n'.join(body)
            else:
                sections.append(section('', f'Книга {book}', body))
            continue
        current, body = None, []
        for line in lines:
            heading = line[len(HEAD):] if line.startswith(HEAD) else line
            number = None
            if config['split'] == 'chapters':
                match = re.fullmatch(r'(?i)глава\s+([а-яё ]+)', heading)
                number = ordinal(match.group(1)) if match else None
            elif line.startswith(HEAD):
                match = re.match(r'([IVXL]+)\b', heading)
                number = roman(match.group(1)) if match else None
            if number:
                if current is not None:
                    sections.append(section(f'Книга {n}' if config['split'] == 'chapters' else '', f'§{current}' if config['split'] == 'chapters' else f'Глава {current}', body))
                current, body = number, []
            elif current is not None and line and not line.startswith(HEAD):
                body.append(line)
        if current is None:
            raise ValueError(f'{work} page {n}: no chapter heading')
        sections.append(section(f'Книга {n}' if config['split'] == 'chapters' else '', f'§{current}' if config['split'] == 'chapters' else f'Глава {current}', body))
    empty = [s['id'] for s in sections if not s['blocks'][0]['text'].strip()]
    if empty:
        raise ValueError(f'{work}: empty sections {empty}')
    return sections


def load(capture, work):
    folder = capture / work
    record = json.loads((folder / 'capture.json').read_text())
    pages, raws = [], []
    for page in record['pages']:
        raw = (folder / f"{page['n']:02}-api.json").read_bytes()
        parsed = json.loads(raw)['parse']
        if parsed['revid'] != page['revid']:
            raise ValueError(f"{page['title']}: capture revision mismatch")
        pages.append({**page, 'html': parsed['text']['*'], 'templates': [t['*'] for t in parsed.get('templates', [])]})
        raws.append(raw)
    return record, pages, raws


def url_of(title, revid):
    return 'https://ru.wikisource.org/w/index.php?title=' + urllib.parse.quote(title.replace(' ', '_')) + f'&oldid={revid}'


def prepare(work, capture, selection, owner):
    config = WORKS[work]
    record, pages, raws = load(capture, work)
    sections = sections_for(work, config, pages)
    index_url = 'https://ru.wikisource.org/wiki/' + urllib.parse.quote(record['index'].replace(' ', '_'))
    articles = [{'n': p['n'], 'title': p['title'], 'article_url': url_of(p['title'], p['revid']), 'article_oldid': p['revid'],
                 'revision_timestamp': p['timestamp'], 'captured_at': p['captured_at']} for p in pages]
    licenses = sorted({t for p in pages for t in p['templates'] if re.search(r'PD|License', t)})
    limit = (f"{len(pages)} Wikisource page{'s' if len(pages) > 1 else ''} without page-scan proofreading; "
             f"licence templates {', '.join(licenses) or 'none listed'}. {config.get('coverage', '')} "
             'Scan images were not collated. Automatic alignment is unreviewed.').replace('  ', ' ')
    data = {'header': f"<teiHeader><title>{config['title']}</title><translator>{config['translator']}</translator></teiHeader>",
            'sections': sections, 'notes': [], 'adjustments': [], 'source_pages': articles}
    stage = capture / 'texts'
    stage.mkdir(exist_ok=True)
    (stage / f"{config['id']}.json").write_bytes(retained.json_bytes(data))
    retained.REPO = pathlib.Path('.')
    retained.LEGACY = capture
    retained.MADE_AT = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')
    retained.MINT_IDENTITIES = True
    source = articles[0]['article_url'] if len(articles) == 1 else index_url
    version = {'id': config['id'], 'language': 'ru', 'credit': config['credit'], 'creditRole': 'translator',
               'license': LICENSE, 'source': source, 'publication': {'titles': [config['edition']], 'dates': [config['year']]},
               'attribution': config['attribution'], 'coverageLimit': limit, 'proofreadingStatus': 'not page-proofread',
               'coverageStatus': 'partial-source-transcription' if config.get('coverage') else 'complete-source-transcription'}
    files, details = retained.output_for({'id': work, 'author': config['author'], 'title': config['title']}, version, data)
    item = details['item_dir_ref']
    for ref, note in (
        (f"{item.split('/editions/')[0]}/expression.json", 'A separately captured Wikisource translation; no equivalence to another local edition or review is claimed.'),
        (f"{item.split('/items/')[0]}/edition.json", 'Captured Wikisource page renderings and their bounded public reading derivative; the exact parse API responses are retained with the item.'),
    ):
        entry = json.loads(files[ref]); entry['notes'] = note; files[ref] = retained.json_bytes(entry)
    manifest = json.loads(files[f'{item}/item.manifest.json'])
    outputs = []
    for article, raw in zip(articles, raws):
        rel = f"payload/wikisource-page-{article['n']:02}-api.json"
        sha = retained.sha(raw)
        files[f'{item}/{rel}'] = raw
        manifest['payload_files'].append({'file_id': f'tos.file.sha256.{sha}', 'relative_path': rel, 'original_basename': rel.split('/')[-1],
            'media_type': 'application/json', 'byte_size': len(raw), 'sha256': sha, 'fixity_verified_at': retained.MADE_AT})
        files[f'{item}/fixity.sha256'] += f'{sha}  {rel}\n'.encode()
        outputs.append({'ref': f'{item}/{rel}', 'sha256': sha, 'role': 'wikisource_page_api_response'})
        article.update(raw_api_response_ref=f'{item}/{rel}', raw_api_response_sha256=sha)
    files[f'{item}/item.manifest.json'] = retained.json_bytes(manifest)
    metadata = {'schema_version': 'tos_public_source_metadata_snapshot_v1', 'record_version': 1, 'item_id': details['item_id'],
        'captured_at': articles[-1]['captured_at'], 'source': {'provider': 'Russian Wikisource', 'work': config['title'],
        'translator': config['translator'], 'edition': config['edition'], 'index_url': index_url, 'pages': articles,
        'licence_templates': licenses, 'scan_collation_performed': False}, 'rights_evidence': [index_url] + LAW,
        'limitations': 'Each page revision was read again after its render; each API response is retained. The pages are not ProofreadPage transclusions, so no page-scan status is recorded.'}
    meta_ref = f'{item}/source-metadata-snapshot.json'
    files[meta_ref] = retained.json_bytes(metadata)
    rights_ref = f'{item}/rights.json'
    rights = json.loads(files[rights_ref])
    rights['jurisdictions_reviewed'] = ['RU', 'US']
    rights['source_refs'] = [index_url] + [a['article_url'] for a in articles] + LAW
    rights['scope_refs'].extend('tos.file.sha256.' + o['sha256'] for o in outputs)
    rights['rationale'] = config['rationale']
    files[rights_ref] = retained.json_bytes(rights)
    prov = [json.loads(line) for line in files[f'{item}/provenance.jsonl'].decode().splitlines()]
    prov[0]['method']['name'] = 'captured-wikisource-page-renders'
    prov[0]['method']['configuration']['purpose'] = 'Exact parse API responses retained; each page revision read again after its render.'
    prov[0]['inputs'] = []
    prov[0]['outputs'] = outputs + [{'ref': f'{item}/payload/retained-reading.json', 'sha256': details['source_json_sha'], 'role': 'captured_and_extracted_representation'}]
    prov[1]['method']['configuration']['purpose'] = ('Extracts the rendered text; excludes the header, navigation, headings, note markers, notes'
        + (' and parenthesised marginal or note numbers' if config.get('margins') else '') + '; a line per paragraph. No spelling corrections. All API responses are retained intact.')
    prov[3]['method']['configuration']['purpose'] = 'Rights assessment from the identified translation, its publication and the Wikisource transcription license; no text or canon acceptance.'
    prov[3]['inputs'] = [{'ref': meta_ref, 'sha256': retained.sha(files[meta_ref]), 'role': 'rights_source_metadata_snapshot'}]
    prov[3]['outputs'][0]['sha256'] = retained.sha(files[rights_ref])
    files[f'{item}/provenance.jsonl'] = (''.join(json.dumps(e, ensure_ascii=False, separators=(',', ':')) + '\n' for e in prov)).encode()
    files[f'{item}/forensic-report.md'] = (f"# Wikisource acquisition: {config['title']} ({config['credit']})\n\n- Index: {index_url}\n"
        f"- Page oldids, capture times and response digests: source metadata snapshot.\n- {limit}\n"
        '- Reading layer excludes the header, navigation, headings, note markers and notes. It preserves the displayed text and its errors.\n').encode()
    version_record = details['version_record']
    version_record['rightsSha256'] = retained.sha(files[rights_ref])
    doc = next(d for d in selection['documents'] if d['id'] == details['work_id'])
    if any(v['id'] == version_record['id'] for v in doc['versions']):
        raise ValueError(f"version already selected: {version_record['id']}")
    for ref in files:
        if (ROOT / ref).exists():
            raise ValueError(f'will not overwrite {ref}')
    for ref, content in files.items():
        path = ROOT / ref
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    retained.write_metadata_refresh(owner, ROOT, f'{item}/item.manifest.json', f"tos.event.inventory.{details['item_id']}.20261104")
    doc['versions'].append(version_record)
    print(f"{work}: {len(sections)} sections from {len(pages)} pages", flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--capture-dir', required=True, type=pathlib.Path)
    parser.add_argument('--native-owner-command')
    parser.add_argument('--dry-run', action='store_true')
    parser.add_argument('works', nargs='+', choices=WORKS)
    args = parser.parse_args()
    capture = args.capture_dir.resolve(strict=True)
    if args.dry_run:
        for work in args.works:
            _, pages, _ = load(capture, work)
            sections = sections_for(work, WORKS[work], pages)
            sizes = [len(s['blocks'][0]['text']) for s in sections]
            print(work, len(sections), 'sections', sum(sizes), 'chars', [f"{s['group']} · {s['label']}" if s['group'] else s['label'] for s in sections][:6],
                  'smallest', min(sizes), '|', sections[0]['blocks'][0]['text'][:120].replace('\n', ' / '), '|', sections[-1]['blocks'][0]['text'][-120:].replace('\n', ' / '))
        return
    owner = retained.resolve_inventory_owner(args.native_owner_command)
    selection_path = ROOT / 'access/public-corpus/selection.v1.json'
    selection = json.loads(selection_path.read_text())
    for work in args.works:
        prepare(work, capture, selection, owner)
        selection_path.write_bytes(retained.json_bytes(selection))


if __name__ == '__main__':
    main()

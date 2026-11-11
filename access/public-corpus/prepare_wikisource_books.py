#!/usr/bin/env python3
"""Prepare Wikisource translations of Antiquity works as retained readings.

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
LAW = {'ru': ['https://ru.wikisource.org/wiki/Викитека:Авторские_права'], 'en': ['https://en.wikisource.org/wiki/Wikisource:Copyright_policy']}
LAW_COMMON = [LICENSE, 'https://www.copyright.gov/circs/circ22.pdf',
       'https://www.consultant.ru/document/cons_doc_LAW_64629/675b81f0bc9b8ecd68ea332ecfd9d1562541fb98/']
EN_RIGHTS = ' Public domain in the United States (published before 1931) and in Russia (the translator died more than 70 years ago). Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'

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
        'rationale': ('Translation first published 1934 in the translator’s lifetime; the translator died 13 January 1937 in Moscow '
                      '(ru.wikipedia, «Кубицкий, Александр Владиславович», revision 150221856; Wikisource names him with the same dates). '
                      'Russia: under the 1993 Copyright Law the term ran 50 years after death and ended on 31 December 1987, before that '
                      'law took effect, so it was not revived; Federal Law 231-FZ art. 6 applies the 70-year term of Civil Code art. 1281 '
                      'only where the 50-year term was still running on 1 January 2004, and 70 years after death also ended on '
                      '31 December 2007. Neither extension applies: no record of repression or posthumous rehabilitation was found '
                      '(art. 1281 p. 4), and he died before the war (wartime extension). United States: not restored under URAA, because '
                      'the work was in the public domain in its source country on 1 January 1996. Wikisource transcription under '
                      'CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.')},
    'poetics': {'author': 'aristotle', 'title': 'Поэтика', 'split': 'roman',
        'id': 'poetics-ru-novosadsky1927-wikisource', 'translator': 'Николай Иванович Новосадский (1859–1941)',
        'credit': 'Перевод: Н. И. Новосадский', 'edition': 'Аристотель. Поэтика. Л.: Academia, 1927', 'year': '1927',
        'attribution': 'Аристотель; перевод Н. И. Новосадского (1927); участники русской Викитеки (история указанной страницы).',
        'source_warning': ('Source warning: the origin of this transcription is not established and serious errors are possible. '
                           'The Wikisource page has no scans and no quality rating (category 00%); it names as its source a 1998 '
                           'Minsk reprint taken from teatr-lib.ru, not the 1927 Academia printing, and gives the translator’s '
                           'initials as «Н. Н.».'),
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
        'id': 'meditations-ru-rogovin1914-wikisource', 'translator': 'Семён Миронович Роговин',
        'credit': 'Перевод: С. М. Роговин', 'edition': 'Марк Аврелий. Наедине с собой. Размышления. М., 1914', 'year': '1914',
        'attribution': 'Марк Аврелий; перевод С. М. Роговина (1914); участники русской Викитеки (история указанных страниц).',
        'margins': True, 'rationale': 'Translation published 1914; Wikisource marks it PD-RusEmpire (published before 7 November 1917). Public domain in Russia and in the United States (published before 1931). Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. No scan collation or semantic acceptance.'},
}

# English translations from English Wikisource for works the corpus held only in the original.
YONGE = {'translator': 'Charles Duke Yonge (1812–1891)', 'credit': 'Translated by C. D. Yonge'}
STEWART = {'translator': 'Aubrey Stewart (1844–1918)', 'credit': 'Translated by Aubrey Stewart'}
CICERO_1891 = 'The Academic Questions, Treatise De Finibus, and Tusculan Disputations. London: George Bell, 1891'
for key, config in {
    'athenian-constitution-en': {'work': 'athenian-constitution', 'author': 'aristotle', 'title': 'The Athenian Constitution', 'split': 'numbered', 'inline_notes': True,
        'id': 'athenian-constitution-en-kenyon1921-wikisource', 'translator': 'Frederic G. Kenyon (1863–1952)', 'credit': 'Translated by F. G. Kenyon',
        'edition': 'The Works of Aristotle, vol. X. Oxford: Clarendon Press, 1921', 'year': '1921', 'rationale': 'Translation published 1921; the translator died 1952.'},
    'lucullus-en': {'work': 'lucullus', 'author': 'cicero', 'title': 'Academic Questions, Book II (Lucullus)', 'split': 'single', 'label': 'Lucullus', **YONGE,
        'id': 'lucullus-en-yonge1891-wikisource', 'edition': CICERO_1891, 'year': '1891', 'rationale': 'Translation published 1853 (here the 1891 printing); the translator died 1891.'},
    'de-finibus-en': {'work': 'de-finibus-bonorum-et-malorum', 'author': 'cicero', 'title': 'De Finibus', 'split': 'books', **YONGE,
        'id': 'de-finibus-en-yonge1891-wikisource', 'edition': CICERO_1891, 'year': '1891', 'rationale': 'Translation published 1853 (here the 1891 printing); the translator died 1891.'},
    'de-natura-deorum-en': {'work': 'de-natura-deorum', 'author': 'cicero', 'title': 'On the Nature of the Gods', 'split': 'books', **YONGE,
        'id': 'de-natura-deorum-en-yonge1888-wikisource', 'edition': "Cicero's Tusculan Disputations; also Treatises on the Nature of the Gods and on the Commonwealth. New York: Harper, 1888", 'year': '1888',
        'rationale': 'Translation published 1888; the translator died 1891.'},
    'de-beneficiis-en': {'work': 'de-beneficiis', 'author': 'seneca', 'title': 'On Benefits', 'split': 'books', **STEWART,
        'id': 'de-beneficiis-en-stewart1887-wikisource', 'edition': 'L. Annaeus Seneca on Benefits. London: George Bell, 1887', 'year': '1887',
        'rationale': 'Translation published 1887; the translator died 1918.'},
    'de-clementia-en': {'work': 'de-clementia', 'author': 'seneca', 'title': 'Of Clemency', 'split': 'books', **STEWART,
        'id': 'de-clementia-en-stewart1900-wikisource', 'edition': 'L. Annaeus Seneca, Minor Dialogues together with the Dialogue On Clemency. London: George Bell, 1900', 'year': '1900',
        'rationale': 'Translation published 1889 (here the 1900 printing); the translator died 1918.'},
    'meditations-en': {'work': 'meditations', 'author': 'marcus', 'title': 'The Communings with Himself', 'split': 'books',
        'id': 'meditations-en-haines1916-wikisource', 'translator': 'Charles Reginald Haines (1858–1935)', 'credit': 'Translated by C. R. Haines',
        'edition': 'The Communings with Himself of Marcus Aurelius Antoninus. Loeb Classical Library. London: Heinemann, 1916', 'year': '1916',
        'rationale': 'Translation published 1916; the translator died 1935.'},
    'isagoge-en': {'work': 'isagoge', 'author': 'porphyry', 'title': 'The Introduction of Porphyry', 'split': 'single', 'label': 'Text',
        'id': 'isagoge-en-owen1853-wikisource', 'translator': 'Octavius Freire Owen (1816–1873)', 'credit': 'Translated by O. F. Owen',
        'edition': 'The Organon, or Logical Treatises, of Aristotle, with the Introduction of Porphyry. London: Bohn, 1853', 'year': '1853',
        'rationale': 'Translation published 1853; the translator died 1873.'},
}.items():
    config.update(wiki='en', language='en', attribution=f"{config['credit']} ({config['year']}); contributors to English Wikisource (history of the cited pages).")
    config['rationale'] += EN_RIGHTS
    WORKS[key] = config

# The first four volumes contain 45 tractates. The fifth volume is excluded:
# B. S. Page translated VI.1–3 and revised VI.4–9, so it needs a separate
# rights assessment and attribution.
WORKS['enneads-en-mackenna'] = {
    'work': 'enneads', 'author': 'plotinus', 'title': 'Enneads', 'split': 'enneads',
    'id': 'enneads-en-mackenna1917-wikisource', 'translator': 'Stephen MacKenna (1872–1934)',
    'credit': 'Translated by Stephen MacKenna', 'edition': 'Plotinus, translated by Stephen MacKenna, London: Medici Society, volumes 1–4, 1917–1926',
    'year': '1917–1926', 'wiki': 'en', 'language': 'en',
    'attribution': 'Plotinus, translated by Stephen MacKenna (1917–1926); contributors to English Wikisource (history of the cited pages).',
    'coverage': '45 of 54 tractates: the entire sixth Ennead is excluded because Bertram Samuel Page (1904–1993) translated VI.1–VI.3 and revised VI.4–VI.9.',
    'rights_sources': ['https://catalogue.nli.ie/Record/vtls000460398',
                       'https://ojs.library.dal.ca/dionysius/article/download/dio23todd/2745',
                       'https://www.mashraba.org/read/plotinus-6-9/about'],
    'transcluded_pages': True,
    'rationale': ('MacKenna died in 1934; his identified first-edition translations in volumes 1–4 were published in 1917–1926. '
                  'Public domain in the United States by publication date and in Russia and the EU after the applicable life-plus-70 term. '
                  'Page translated VI.1–VI.3, revised VI.4–VI.9, and died in 1993; the entire sixth Ennead is excluded from this witness pending separate rights assessment. '
                  'Wikisource transcription under CC BY-SA 4.0: retain attribution, source, license and change notices. '
                  'No scan collation or semantic acceptance.'),
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
    if config['split'] == 'enneads':
        expected = {(ennead, tractate) for ennead in range(1, 6) for tractate in range(1, 10)}
        actual = {(p['ennead'], p['tractate']) for p in pages}
        if actual != expected or len(pages) != len(expected):
            raise ValueError('Enneads witness must contain exactly I.1–V.9; VI requires separate rights assessment')
    for n, page in enumerate(pages, 1):
        lines = lines_of(page['html'])
        # Notes follow their heading at the end of a page; they are not the work's text.
        for i, line in enumerate(lines):
            if line.startswith(HEAD) and line[len(HEAD):].strip().lower() in ('примечания', 'notes', 'footnotes'):
                lines = lines[:i]
                break
        # Marginal line numbers and page marks [c.525] (Politics) and note numbers (Meditations).
        if config.get('margins'):
            lines = [re.sub(r'\s*(\(\d{1,3}\)|\[c\.\s*\d+\])', '', line).strip() for line in lines]
        # Navigation rows of chapter numbers ("1 - 2 - 3 - …") are not text.
        lines = [line for line in lines if not (re.search(r'\d', line) and re.fullmatch(r'[\d\s\-–—|·.]+', line))]
        # Wikisource maintenance notices, layout switches and section menus.
        lines = [line for line in lines if not re.match(r'(Layout \d+$|Sections:|This work has been imported|Please take a moment)', line)]
        # Translator's notes set as numbered lines after the text ("12 Ch. 2, 2.").
        if config.get('inline_notes'):
            lines = [line for line in lines if not re.match(r'\d{1,3} (?:[A-Z‘“(\[]|i\.e\.|e\.g\.|cf\.)', line)]
        if config['split'] == 'numbered':
            current, body = None, []
            for line in lines:
                match = re.match(r'(\d{1,2})\.\s', line[len(HEAD):]) if line.startswith(HEAD) else None
                if match:
                    if current is not None:
                        sections.append(section('', f'§{current}', body))
                    current, body = int(match.group(1)), []
                elif current is not None and line and not line.startswith(HEAD):
                    body.append(line)
            if current is not None:
                sections.append(section('', f'§{current}', body))
            continue
        if config['split'] == 'single':
            body = [line for line in lines if line and not line.startswith(HEAD)]
            if sections:
                sections[-1]['blocks'][0]['text'] += '\n' + '\n'.join(body)
            else:
                sections.append(section('', config['label'], body))
            continue
        if config['split'] == 'enneads':
            body = [line for line in lines if line and not line.startswith(HEAD)]
            sections.append(section(f"Книга {page['ennead']}", f"Глава {page['tractate']}", body))
            continue
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


def url_of(title, revid, wiki='ru'):
    return f'https://{wiki}.wikisource.org/w/index.php?title=' + urllib.parse.quote(title.replace(' ', '_')) + f'&oldid={revid}'


def prepare(work, capture, selection, owner):
    config = WORKS[work]
    record, pages, raws = load(capture, work)
    sections = sections_for(work, config, pages)
    wiki = config.get('wiki', 'ru')
    law = LAW[wiki] + LAW_COMMON + config.get('rights_sources', [])
    index_url = f'https://{wiki}.wikisource.org/wiki/' + urllib.parse.quote(record['index'].replace(' ', '_'))
    articles = [{'n': p['n'], 'title': p['title'], 'article_url': url_of(p['title'], p['revid'], wiki), 'article_oldid': p['revid'],
                 'revision_timestamp': p['timestamp'], 'captured_at': p['captured_at']} for p in pages]
    licenses = sorted({t for p in pages for t in p['templates'] if re.search(r'PD|License', t)})
    # A source's own warning about its text is kept first, verbatim in substance, wherever the limit is shown.
    warning = f"{config['source_warning']} " if config.get('source_warning') else ''
    proof = 'with scan-page proofreading status not assessed' if config.get('transcluded_pages') else 'without page-scan proofreading'
    limit = (f"{warning}{len(pages)} Wikisource page{'s' if len(pages) > 1 else ''} {proof}; "
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
    version = {'id': config['id'], 'language': config.get('language', 'ru'), 'credit': config['credit'], 'creditRole': 'translator',
               'license': LICENSE, 'source': source, 'publication': {'titles': [config['edition']], 'dates': [config['year']]},
               'attribution': config['attribution'], 'coverageLimit': limit, 'proofreadingStatus': 'not assessed' if config.get('transcluded_pages') else 'not page-proofread',
               'coverageStatus': 'partial-source-transcription' if config.get('coverage') else 'complete-source-transcription'}
    files, details = retained.output_for({'id': config.get('work', work), 'author': config['author'], 'title': config['title']}, version, data)
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
        'captured_at': articles[-1]['captured_at'], 'source': {'provider': 'Russian Wikisource' if wiki == 'ru' else 'English Wikisource', 'work': config['title'],
        'translator': config['translator'], 'edition': config['edition'], 'index_url': index_url, 'pages': articles,
        'licence_templates': licenses, 'scan_collation_performed': False}, 'rights_evidence': [index_url] + law,
        'limitations': warning + ('Each article was rendered twice with identical text and revision; exact API responses are retained. Article oldids do not freeze their transcluded scan pages; the scan leaves were not collated.' if config.get('transcluded_pages') else 'Each page revision was read again after its render; each API response is retained. The pages are not ProofreadPage transclusions, so no page-scan status is recorded.')}
    meta_ref = f'{item}/source-metadata-snapshot.json'
    files[meta_ref] = retained.json_bytes(metadata)
    rights_ref = f'{item}/rights.json'
    rights = json.loads(files[rights_ref])
    rights['jurisdictions_reviewed'] = ['RU', 'US']
    rights['source_refs'] = [index_url] + [a['article_url'] for a in articles] + law
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

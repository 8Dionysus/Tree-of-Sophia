#!/usr/bin/env python3
"""Bring the master tables of the ToS research into the Population bonsai's review folder: one row per research node.

    python3 -I bonsai/review/population/nodes.py --mirror <mirror> [--check]

<mirror> is the local mirror of the ToS registers: it holds MANIFEST_SHA256.csv and
0_Master-table/{1,2,3} мастер-таблица для ToS.docx, the three dispatch tables of the research: Table I
(antiquity, A01–A48), Table II (600–1500, T2-01–T2-58) and Table III (modernity, T3-01–T3-84). One row of a table
is one research node; a node's ID is the batch ID its registers carry. Every table must match, byte for byte, the
SHA-256 the manifest records for its path, or the intake stops. A table must have the columns this adapter reads,
under the names it knows: a new form of the tables needs its own reading here, never a guess (TOS-D-0086).

Writes, beside this file:

    registry/master-tables/nodes.jsonl   the cells of each node's row the bonsai's lenses rest on, as the table
                                         holds them: the ID cell, the three windows of time, the macro-region and
                                         the node; the rest of each row stays in the operator's memory
    layer/nodes/master-tables.jsonl      the same rows parsed: ID, table, launch order, the table's status and
                                         confidence, the node's title, its macro-region in the table's words with the
                                         region the stand groups it under, and each window read as years where its
                                         leading words are a date
    provenance-master-tables.json        the tables read: path in the mirror, bytes, SHA-256, rows

The stand's region is that of the longest known macro-region a cell begins with ("Европа / Средиземноморье" and
"Африка transregional" are Europe and Africa, "Южная Африка / global ethics" is Africa by its own name); the map from
those words to the regions is REGIONS below, and a cell that begins with none of them stops the intake.
"""

import argparse
import html
import json
import os
import re
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from intake import jsonl, nfc, read_manifest, sha256  # noqa: E402

TABLES_DIR = '0_Master-table'
TABLES = {'I': '1 мастер-таблица для ToS.docx', 'II': '2 мастер-таблица для ToS.docx', 'III': '3 мастер-таблица для ToS.docx'}
NODE_ID = re.compile(r'^(A\d{2}|T[23]-\d{2})\b')

# The columns each table's form keeps for the lenses, by the header the table gives them.
FORMS = {
    'I': {'id': 'ID / запуск / ст / ув', 'formation': 'Формирование', 'fixation': 'Письменная фиксация',
          'canonization': 'Канонизация / редакция / комментарий', 'place': 'Макрорегион и узел DR'},
    'II': {'id': 'ID', 'launch': 'Запуск', 'formation': 'Форм.', 'fixation': 'Фикс./пер.', 'canonization': 'Кан./комм.',
           'region': 'Макрорегион', 'node': 'Research-узел', 'status': 'St/Ув.'},
    'III': {'id': 'ID', 'launch': 'Запуск', 'formation': 'Формирование', 'fixation': 'Фиксация / печать / институц. вход',
            'canonization': 'Канонизация / академизация', 'region': 'Макрорегион / поле', 'node': 'Research-узел',
            'status': 'Статус', 'confidence': 'Увер.'},
}

# The first macro-region a cell names, and the region the stand groups the node under. A grouping for navigation,
# not a claim about the tradition: the cell's own words stay beside it.
REGIONS = {
    'europe': ('Европа', 'Западная Европа', 'Центр. Европа', 'Балканы', 'Британия', 'Франция', 'Россия', 'СССР'),
    'west-asia': ('Западная Азия', 'Зап. Азия', 'Иран', 'Османский мир', 'Арабский мир'),
    'north-africa': ('Северная Африка',),
    'africa': ('Африка к югу от Сахары', 'Северо-Восточная Африка', 'Эфиопия', 'Сахель', 'Африка', 'Южная Африка'),
    'south-asia': ('Южная Азия', 'Индия', 'Пенджаб'),
    'central-asia': ('Центральная Азия',),
    'east-asia': ('Восточная Азия', 'Китай', 'Япония', 'Корея'),
    'southeast-asia': ('Юго-Восточная Азия', 'ЮВА', 'Вьетнам', 'Малайский мир', 'Филиппины'),
    'caucasus': ('Кавказ',),
    'americas': ('Америка', 'Латинская Америка', 'США', 'Сев. Америка', 'Карибы', 'Мезоамерика', 'Canada'),
    'oceania': ('Океания', 'Тихий океан', 'Австралия', 'Рапа-Нуи'),
    'transnational': ('global field', 'global Islamic field', 'global Jewish field', 'global south', 'digital',
                      'Black Atlantic', 'Еврейская диаспора', 'Исламский мир'),
}
REGION_OF = {words: key for key, all_words in REGIONS.items() for words in all_words}

# The leading date of a window: years or centuries, an era on either end, «c.» before it; whatever follows stays words.
ERA = r'(?:до\s*н\.?\s*э\.?|н\.?\s*э\.?)'
END = r'(?P<{n}>\d{{1,4}})(?P<{n}d>-е)?\s*(?P<{n}c>вв?\.)?\s*(?P<{n}e>{era})?'
WINDOW = re.compile(r'^(?P<approx>(?:c\.|ca\.|ок\.|около)\s*)?' + END.format(n='a', era=ERA)
                    + r'(?:\s*[–—-]\s*(?:(?:c\.|ок\.)\s*)?' + END.format(n='b', era=ERA) + r')?', re.I)


def fail(message):
    raise SystemExit(f'nodes: {message}')


def cell_text(xml):
    return html.unescape(re.sub(r'<[^>]+>', '', re.sub(r'</w:p>', '\n', xml))).strip()


def rows_of(data, name):
    """Every row of every table of a docx, as lists of cell texts."""
    try:
        with zipfile.ZipFile(__import__('io').BytesIO(data)) as archive:
            xml = archive.read('word/document.xml').decode('utf-8')
    except (KeyError, zipfile.BadZipFile) as error:
        fail(f'{name}: not a readable docx ({error})')
    for table in re.findall(r'<w:tbl>.*?</w:tbl>', xml, flags=re.S):
        rows = [[nfc(cell_text(cell)) for cell in re.findall(r'<w:tc>.*?</w:tc>', row, flags=re.S)]
                for row in re.findall(r'<w:tr[ >].*?</w:tr>', table, flags=re.S)]
        if rows:
            yield rows


def bce(match):
    return bool(match) and match.strip().startswith('до')


def window(words):
    """A window's words and, where its leading words are a date, its first and last astronomical year."""
    first = words.split('\n')[0].strip()
    found = WINDOW.match(first)
    if not found or not found.group('a'):
        return {'words': words}
    a, b = int(found.group('a')), int(found.group('b')) if found.group('b') else None
    # A century mark at either end, or the plural «вв.» at the end, makes both ends centuries; a decade («1990-е»)
    # is its first year. An end without its own era takes the other end's.
    plural = (found.group('bc') or found.group('ac') or '').startswith('вв')
    a_century = bool(found.group('ac')) or (plural and not found.group('ad'))
    b_century = b is not None and (bool(found.group('bc')) or (plural and not found.group('bd') and a_century))
    a_bce = bce(found.group('ae')) if found.group('ae') else bce(found.group('be'))
    b_bce = bce(found.group('be')) if found.group('be') else (bce(found.group('ae')) if b is not None else a_bce)
    if found.group('ae') and not found.group('be') and b is not None and bce(found.group('ae')):
        b_bce = True
    def span(value, century, is_bce):
        if century:
            low, high = 100 * (value - 1) + 1, 100 * value
            return (1 - high, 1 - low) if is_bce else (low, high)
        return (1 - value, 1 - value) if is_bce else (value, value)
    start = span(a, a_century, a_bce)[0]
    end = span(b, b_century, b_bce)[1] if b is not None else span(a, a_century, a_bce)[1]
    if end < start:
        return {'words': words}
    lexeme = lambda year: ('-' if year < 0 else '') + f'{abs(year):04d}'
    approximate = bool(found.group('approx')) or a_century or b_century or bool(found.group('ad') or found.group('bd'))
    out = {'words': words, 'start': lexeme(start), 'end': lexeme(end), 'certainty': 'approximate' if approximate else 'exact'}
    if found.end() < len(first) or '\n' in words:
        out['more'] = True
    return out


def region(words):
    """The region of the longest known macro-region the cell begins with, as a whole word or before « / »."""
    text = words.strip()
    for known in sorted(REGION_OF, key=len, reverse=True):
        if text == known or text.startswith(known + ' ') or text.startswith(known + '/'):
            return {'words': words, 'key': REGION_OF[known]}
    fail(f'macro-region «{words}» begins with no macro-region in REGIONS; add it with the region the stand groups it under')


def read_table(key, data, name):
    form = FORMS[key]
    found = []
    for rows in rows_of(data, name):
        header = rows[0]
        if not any(NODE_ID.match(row[0]) for row in rows[1:] if row):
            continue
        missing = [column for column in form.values() if column not in header]
        if missing:
            fail(f'{name}: a table of nodes lacks the columns {missing}; this form needs its own reading')
        at = {field: header.index(column) for field, column in form.items()}
        for row in rows[1:]:
            if not row or not NODE_ID.match(row[0]):
                continue
            cells = {form[field]: row[index] for field, index in at.items()}
            node = NODE_ID.match(row[0]).group(1)
            found.append({'table': key, 'node': node, 'cells': cells})
    return found


def parse_node(record):
    key, cells, form = record['table'], record['cells'], FORMS[record['table']]
    out = {'node': record['node'], 'table': key}
    if key == 'I':
        parts = [part.strip() for part in cells[form['id']].split('/')]
        if len(parts) != 4 or parts[0] != record['node']:
            fail(f'{record["node"]}: the ID cell «{cells[form["id"]]}» is not «ID / launch / status / confidence»')
        out['launch'], out['status'], out['confidence'] = int(parts[1]), parts[2], int(parts[3])
        place, _, title = cells[form['place']].partition(' — ')
        if not title:
            fail(f'{record["node"]}: the macro-region cell has no « — » before its node')
        out['title'] = title.strip()
        out['region'] = region(place)
    else:
        out['launch'] = int(cells[form['launch']])
        if key == 'II':
            status, _, confidence = cells[form['status']].partition('/')
        else:
            status, confidence = cells[form['status']], cells[form['confidence']]
        out['status'], out['confidence'] = status.strip(), int(confidence)
        out['title'] = cells[form['node']].strip()
        out['region'] = region(cells[form['region']])
    out['windows'] = {field: window(cells[form[field]]) for field in ('formation', 'fixation', 'canonization')}
    return out


def build(mirror):
    manifest, manifest_file = read_manifest(mirror)
    registry, layer, tables = [], [], []
    for key, file_name in TABLES.items():
        rel = f'{TABLES_DIR}/{file_name}'
        path = os.path.join(mirror, TABLES_DIR, file_name)
        if not os.path.exists(path):
            fail(f'{rel}: missing from the mirror')
        data = open(path, 'rb').read()
        digest = sha256(data)
        if manifest.get(nfc(rel)) != digest:
            fail(f'{rel}: its SHA-256 is not the manifest\'s')
        records = read_table(key, data, rel)
        if len({record['node'] for record in records}) != len(records):
            fail(f'{rel}: a node appears twice')
        registry += records
        layer += [parse_node(record) for record in records]
        tables.append({'table': key, 'path': rel, 'bytes': len(data), 'sha256': digest, 'rows': len(records)})
    header = {'tables': [{'table': key, 'file': f'{TABLES_DIR}/{name}', 'columns': list(FORMS[key].values())} for key, name in TABLES.items()]}
    provenance = {
        'made': 'bonsai/review/population/nodes.py',
        'posture': {
            'ru': 'Мастер-таблицы исследования, из которых взяты узлы. Сами таблицы в репозитории не хранятся: здесь их путь в зеркале, размер и SHA-256.',
            'en': 'The master tables of the research the nodes are taken from. The tables are not kept here: only their path in the mirror, size and SHA-256.',
        },
        'manifest': manifest_file,
        'tables': tables,
    }
    return {
        'registry/master-tables/nodes.jsonl': jsonl([header] + registry),
        'layer/nodes/master-tables.jsonl': jsonl(layer),
        'provenance-master-tables.json': json.dumps(provenance, ensure_ascii=False, indent=1) + '\n',
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--mirror', required=True, help='local mirror of the ToS registers')
    parser.add_argument('--out', default=HERE, help='the review folder (default: beside this file)')
    parser.add_argument('--check', action='store_true', help='compare with the files on disk, write nothing')
    args = parser.parse_args()
    files = build(os.path.abspath(os.path.expanduser(args.mirror)))
    differ = []
    for rel, text in files.items():
        path = os.path.join(args.out, rel)
        if args.check:
            if not os.path.exists(path) or open(path, encoding='utf-8').read() != text:
                differ.append(rel)
            continue
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path + '.tmp', 'w', encoding='utf-8', newline='\n') as handle:
            handle.write(text)
        os.replace(path + '.tmp', path)
    rows = sum(table['rows'] for table in json.loads(files['provenance-master-tables.json'])['tables'])
    if args.check:
        if differ:
            print(f'nodes: {len(differ)} files differ from what the tables give: {", ".join(differ)}')
            sys.exit(1)
        print(f'nodes: matches what the tables give ({rows} nodes)')
    else:
        print(f'nodes: wrote {len(files)} files ({rows} nodes)')


if __name__ == '__main__':
    main()

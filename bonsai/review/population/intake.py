#!/usr/bin/env python3
"""Bring the ToS research registers of persons into the Population bonsai's review folder.

    python3 -I bonsai/review/population/intake.py --mirror <mirror> [--table <authors table>] [--check]

<mirror> is a local mirror of the ToS registers: it holds MANIFEST_SHA256.csv and
Реестры/Реестр персон и произведений/**/ToS_Identity_<batch>_registry.xlsx. <table> is the
authors table ToS_Авторы_для_бонсая.xlsx (by default beside the mirror).

Every Identity workbook must match, byte for byte, both the SHA-256 the mirror's manifest
records for its path and the SHA-256 the authors table's sheet «Реестры Identity» records for
its name; a workbook either list lacks, or a digest that differs, stops the intake.

Writes, beside this file:

    registry/<batch>/agents.jsonl            every row of the workbook's `agents` sheet as the
                                             workbook holds it
    registry/<batch>/works-responsibility.jsonl
                                             every row of its `works` sheet, five cells only:
                                             record_id, preferred_label, label_ru, work_dates and
                                             responsibility_statements (the rest of the sheet
                                             comes with the works ring)
    registry/authors-table/<sheet>.jsonl     the authors table's sheets «Имена» and «Произведения»
    provenance.json                          the files read: path in the mirror, bytes, SHA-256,
                                             rows

A registry file's first line names the sheet, its workbook and its columns; every other line
is one row: its sheet row number and its non-empty cells under the sheet's own column names,
each value the exact text of the cell (a number keeps the workbook's own lexical form). The
authors table's header line also keeps the cover lines above its column row.

--check builds everything in memory and compares it with the files on disk.
Only the standard library is used.
"""

import argparse
import csv
import hashlib
import io
import json
import os
import re
import sys
import unicodedata
import xml.etree.ElementTree as ET
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
IDENTITY_DIR = os.path.join('Реестры', 'Реестр персон и произведений')
IDENTITY_FILE = re.compile(r'^ToS_Identity_((?:A\d{2}|T[23]-\d{2}))_registry\.xlsx$')
TABLE_NAME = 'ToS_Авторы_для_бонсая.xlsx'
TABLE_SHEETS = {'Имена': 'names', 'Произведения': 'works'}
TABLE_SNAPSHOT = 'Реестры Identity'
REQUIRED_COLUMNS = ('record_id', 'record_type', 'preferred_label', 'label_ru', 'variant_labels',
                    'external_identifiers', 'identity_status', 'source_refs', 'scope_role', 'life_dates')
ROW_ID = re.compile(r'^(?:A\d{2}|T[23]-\d{2})-A\d{3,4}$')

M = '{http://schemas.openxmlformats.org/spreadsheetml/2006/main}'
R = '{http://schemas.openxmlformats.org/officeDocument/2006/relationships}'


def fail(message):
    raise SystemExit(f'intake: {message}')


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def nfc(text):
    return unicodedata.normalize('NFC', text)


def column_index(ref):
    n = 0
    for ch in ref:
        if not ch.isalpha():
            break
        n = n * 26 + (ord(ch.upper()) - 64)
    return n - 1


def run_text(element):
    """The text of a string item: its own <t> and the <t> of its runs, never phonetic guides."""
    parts = [t.text or '' for t in element.findall(M + 't')]
    parts += [t.text or '' for r in element.findall(M + 'r') for t in r.findall(M + 't')]
    return ''.join(parts)


class Workbook:
    def __init__(self, data):
        self.zip = zipfile.ZipFile(io.BytesIO(data))
        names = set(self.zip.namelist())
        self.shared = []
        if 'xl/sharedStrings.xml' in names:
            root = ET.fromstring(self.zip.read('xl/sharedStrings.xml'))
            self.shared = [run_text(si) for si in root.iter(M + 'si')]
        workbook = ET.fromstring(self.zip.read('xl/workbook.xml'))
        rels = ET.fromstring(self.zip.read('xl/_rels/workbook.xml.rels'))
        targets = {rel.get('Id'): rel.get('Target') for rel in rels}
        self.sheets = {}
        for sheet in workbook.iter(M + 'sheet'):
            target = targets[sheet.get(R + 'id')]
            self.sheets[sheet.get('name')] = target.lstrip('/') if target.startswith('/') else 'xl/' + target

    def cell(self, c):
        kind = c.get('t') or 'n'
        if kind == 'inlineStr':
            inline = c.find(M + 'is')
            return '' if inline is None else run_text(inline)
        value = c.find(M + 'v')
        if value is None:
            return ''
        if kind == 's':
            return self.shared[int(value.text)]
        return value.text or ''

    def rows(self, name):
        """(sheet row number, [cell texts]) for every row of a sheet, in order."""
        if name not in self.sheets:
            fail(f'no sheet {name!r}')
        for _, element in ET.iterparse(self.zip.open(self.sheets[name])):
            if element.tag != M + 'row':
                continue
            cells = []
            for c in element.findall(M + 'c'):
                index = column_index(c.get('r') or '')
                if index < 0:
                    index = len(cells)
                while len(cells) < index:
                    cells.append('')
                cells.append(self.cell(c))
            yield int(element.get('r')), cells
            element.clear()


def record(number, header, cells):
    row = {'row': number}
    for index, value in enumerate(cells):
        if index >= len(header) or not header[index]:
            if value.strip():
                fail(f'row {number}: a value outside the named columns')
            continue
        if value.strip():
            row[header[index]] = value
    return row


def read_manifest(mirror):
    path = os.path.join(mirror, 'MANIFEST_SHA256.csv')
    if not os.path.exists(path):
        fail(f'{path}: no manifest')
    data = open(path, 'rb').read()
    table = {}
    reader = csv.reader(io.StringIO(data.decode('utf-8-sig')))
    header = next(reader)
    if 'Relative path' not in header or 'SHA-256' not in header:
        fail('MANIFEST_SHA256.csv: unknown columns')
    at_path, at_sha = header.index('Relative path'), header.index('SHA-256')
    for line in reader:
        if line:
            table[nfc(line[at_path])] = line[at_sha]
    return table, {'path': 'MANIFEST_SHA256.csv', 'bytes': len(data), 'sha256': sha256(data)}


def identity_workbooks(mirror):
    base = os.path.join(mirror, IDENTITY_DIR)
    if not os.path.isdir(base):
        fail(f'{base}: no Identity registers')
    found = {}
    for folder, _, files in os.walk(base):
        for name in files:
            match = IDENTITY_FILE.match(nfc(name))
            if match:
                batch = match.group(1)
                if batch in found:
                    fail(f'two workbooks for batch {batch}')
                found[batch] = os.path.join(folder, name)
    if not found:
        fail(f'{base}: no ToS_Identity_*_registry.xlsx')
    return found


def read_table(path):
    if not os.path.exists(path):
        fail(f'{path}: no authors table')
    data = open(path, 'rb').read()
    book = Workbook(data)
    snapshot = {}
    header = None
    for number, cells in book.rows(TABLE_SNAPSHOT):
        if header is None:
            if 'Исходный файл' in cells:
                header = cells
            continue
        if not any(c.strip() for c in cells):
            continue
        row = dict(zip(header, cells))
        snapshot[nfc(row['Исходный файл'])] = row['SHA-256 по сводке'].strip()
    if header is None:
        fail(f'{TABLE_SNAPSHOT}: no column row')
    sheets = {}
    for sheet, slug in TABLE_SHEETS.items():
        cover, header, rows = [], None, []
        for number, cells in book.rows(sheet):
            if header is None:
                filled = [c for c in cells if c.strip()]
                if len(filled) >= 4:
                    header = [c.strip() for c in cells]
                elif filled:
                    cover.append(' '.join(filled))
                continue
            if any(c.strip() for c in cells):
                rows.append(record(number, header, cells))
        if header is None:
            fail(f'{sheet}: no column row')
        head = {'sheet': sheet, 'workbook': TABLE_NAME, 'cover': cover, 'columns': [c for c in header if c]}
        sheets[slug] = [head] + rows
    return sheets, snapshot, {'path': TABLE_NAME, 'bytes': len(data), 'sha256': sha256(data)}


def read_agents(batch, data, name):
    book = Workbook(data)
    header, rows, ids = None, [], set()
    for number, cells in book.rows('agents'):
        if header is None:
            header = [c.strip() for c in cells]
            missing = [c for c in REQUIRED_COLUMNS if c not in header]
            if missing:
                fail(f'{name}: agents sheet lacks {", ".join(missing)}')
            continue
        if not any(c.strip() for c in cells):
            continue
        row = record(number, header, cells)
        rid = row.get('record_id', '').strip()
        if not ROW_ID.match(rid) or not rid.startswith(batch + '-'):
            fail(f'{name}: row {number}: record_id {rid!r} does not belong to batch {batch}')
        if rid in ids:
            fail(f'{name}: record_id {rid} twice')
        ids.add(rid)
        rows.append(row)
    if header is None:
        fail(f'{name}: empty agents sheet')
    head = {'sheet': 'agents', 'workbook': name, 'columns': [c for c in header if c]}
    return [head] + rows


WORK_ID = re.compile(r'^(?:A\d{2}|T[23]-\d{2})-[A-Z]\d{3,5}$')
WORK_COLUMNS = ('record_id', 'preferred_label', 'label_ru', 'work_dates', 'responsibility_statements')


def read_works(batch, data, name):
    """The ID, labels, dates and responsibility cells of every row of the `works` sheet."""
    book = Workbook(data)
    header, rows, ids = None, [], set()
    for number, cells in book.rows('works'):
        if header is None:
            header = [c.strip() for c in cells]
            missing = [c for c in WORK_COLUMNS if c not in header]
            if missing:
                fail(f'{name}: works sheet lacks {", ".join(missing)}')
            keep = [header.index(c) for c in WORK_COLUMNS]
            continue
        if not any(c.strip() for c in cells):
            continue
        row = {'row': number}
        for index in keep:
            if index < len(cells) and cells[index].strip():
                row[header[index]] = cells[index]
        rid = row.get('record_id', '').strip()
        if not WORK_ID.match(rid) or not rid.startswith(batch + '-'):
            fail(f'{name}: works row {number}: record_id {rid!r} does not belong to batch {batch}')
        if rid in ids:
            fail(f'{name}: work {rid} twice')
        ids.add(rid)
        rows.append(row)
    if header is None:
        fail(f'{name}: empty works sheet')
    head = {'sheet': 'works', 'workbook': name, 'columns': list(WORK_COLUMNS),
            'projection': 'five cells of every row; the other columns come with the works ring'}
    return [head] + rows


def jsonl(records):
    return ''.join(json.dumps(item, ensure_ascii=False) + '\n' for item in records)


def build(mirror, table_path):
    manifest, manifest_file = read_manifest(mirror)
    table_sheets, snapshot, table_file = read_table(table_path)
    workbooks = identity_workbooks(mirror)
    names = {os.path.basename(path): batch for batch, path in workbooks.items()}
    missing = sorted(set(snapshot) - set(names))
    extra = sorted(set(names) - set(snapshot))
    if missing:
        fail(f'the authors table lists workbooks the mirror lacks: {", ".join(missing)}')
    if extra:
        fail(f'the mirror holds workbooks the authors table does not list: {", ".join(extra)}')
    files, read = {}, []
    for batch in sorted(workbooks):
        path = workbooks[batch]
        name = os.path.basename(path)
        rel = nfc(os.path.relpath(path, mirror)).replace(os.sep, '/')
        data = open(path, 'rb').read()
        digest = sha256(data)
        if manifest.get(rel) is None:
            fail(f'{rel}: not in MANIFEST_SHA256.csv')
        if manifest[rel] != digest:
            fail(f'{rel}: SHA-256 differs from MANIFEST_SHA256.csv')
        if snapshot[name] != digest:
            fail(f'{rel}: SHA-256 differs from the authors table')
        rows = read_agents(batch, data, name)
        works = read_works(batch, data, name)
        files[f'registry/{batch}/agents.jsonl'] = jsonl(rows)
        files[f'registry/{batch}/works-responsibility.jsonl'] = jsonl(works)
        read.append({'batch': batch, 'path': rel, 'bytes': len(data), 'sha256': digest,
                     'rows': len(rows) - 1, 'columns': len(rows[0]['columns']), 'works': len(works) - 1})
    for slug, rows in table_sheets.items():
        files[f'registry/authors-table/{slug}.jsonl'] = jsonl(rows)
    provenance = {
        'made': 'bonsai/review/population/intake.py',
        'posture': {
            'ru': 'Файлы, из которых собран слой реестров лиц. Сами книги в репозитории не хранятся: здесь их путь в зеркале реестров, размер и SHA-256.',
            'en': 'The files the persons register layer is built from. The workbooks are not kept here: only their path in the registers mirror, size and SHA-256.',
        },
        'mirror': {'manifest': manifest_file, 'identity_workbooks': len(read)},
        'table': dict(table_file, sheets={slug: len(rows) - 1 for slug, rows in table_sheets.items()}),
        'workbooks': read,
        'rows': {'agents': sum(item['rows'] for item in read), 'works': sum(item['works'] for item in read)},
    }
    files['provenance.json'] = json.dumps(provenance, ensure_ascii=False, indent=1) + '\n'
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--mirror', required=True, help='local mirror of the ToS registers')
    parser.add_argument('--table', help=f'the authors table (default: {TABLE_NAME} beside the mirror)')
    parser.add_argument('--out', default=HERE, help='the review folder (default: beside this file)')
    parser.add_argument('--check', action='store_true', help='compare with the files on disk, write nothing')
    args = parser.parse_args()
    mirror = os.path.abspath(os.path.expanduser(args.mirror))
    table = os.path.abspath(os.path.expanduser(args.table)) if args.table else os.path.join(os.path.dirname(mirror), TABLE_NAME)
    files = build(mirror, table)
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
    provenance = json.loads(files['provenance.json'])
    summary = f"{provenance['mirror']['identity_workbooks']} workbooks, {provenance['rows']['agents']} agent rows"
    if args.check:
        if differ:
            print(f'intake: {len(differ)} files differ from what the workbooks give: {", ".join(differ[:8])}')
            sys.exit(1)
        print(f'intake: matches what the workbooks give ({summary})')
    else:
        print(f'intake: wrote {len(files)} files ({summary})')


if __name__ == '__main__':
    main()

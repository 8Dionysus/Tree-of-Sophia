"""Lossless-addressable OOXML readers for research registry imports (stdlib)."""
from __future__ import annotations

import datetime as dt
import posixpath
import re
import xml.etree.ElementTree as ET
from zipfile import ZipFile

S = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
W = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
P = 'http://schemas.openxmlformats.org/package/2006/relationships'


def xml(z, path):
    return ET.fromstring(z.read(path))


def column_index(address):
    n = 0
    for c in re.match(r'[A-Z]+', address)[0]:
        n = n * 26 + ord(c) - 64
    return n


def column_name(n):
    result = ''
    while n:
        n, rem = divmod(n - 1, 26)
        result = chr(65 + rem) + result
    return result


def workbook(path):
    """Return all sheets, populated rows and exact cell representations."""
    with ZipFile(path) as z:
        strings = []
        if 'xl/sharedStrings.xml' in z.namelist():
            strings = [''.join(t.text or '' for t in e.iter(f'{{{S}}}t')) for e in xml(z, 'xl/sharedStrings.xml')]
        date_styles = set()
        if 'xl/styles.xml' in z.namelist():
            styles = xml(z, 'xl/styles.xml')
            custom = {int(n.get('numFmtId')): n.get('formatCode', '')
                      for n in styles.findall(f'{{{S}}}numFmts/{{{S}}}numFmt')}
            for i, xf in enumerate(styles.findall(f'{{{S}}}cellXfs/{{{S}}}xf')):
                fmt = int(xf.get('numFmtId', '0'))
                code = re.sub(r'"[^"]*"|\\.', '', custom.get(fmt, ''))
                if 14 <= fmt <= 22 or 45 <= fmt <= 47 or re.search('[ydhs]', code, re.I):
                    date_styles.add(i)
        wb = xml(z, 'xl/workbook.xml')
        prop = wb.find(f'{{{S}}}workbookPr')
        epoch = dt.datetime(1904, 1, 1) if prop is not None and prop.get('date1904') in ('1', 'true') else dt.datetime(1899, 12, 30)
        rels = {e.get('Id'): e.get('Target') for e in xml(z, 'xl/_rels/workbook.xml.rels')}
        result = []
        for sheet in wb.findall(f'{{{S}}}sheets/{{{S}}}sheet'):
            target = rels[sheet.get(f'{{{R}}}id')]
            part = target.lstrip('/') if target.startswith('/') else posixpath.normpath('xl/' + target)
            tree = xml(z, part)
            relpath = posixpath.dirname(part) + '/_rels/' + posixpath.basename(part) + '.rels'
            sheet_rels = {e.get('Id'): dict(e.attrib) for e in xml(z, relpath)} if relpath in z.namelist() else {}
            hyperlinks = [{**e.attrib, 'relationship': sheet_rels.get(e.get(f'{{{R}}}id'))} for e in tree.findall(f'{{{S}}}hyperlinks/{{{S}}}hyperlink')]
            rows = []
            for row in tree.findall(f'{{{S}}}sheetData/{{{S}}}row'):
                cells = []
                for cell in row.findall(f'{{{S}}}c'):
                    v = cell.find(f'{{{S}}}v')
                    raw = v.text if v is not None else None
                    typ = cell.get('t', 'n')
                    value = raw
                    if typ == 's' and raw is not None:
                        value = strings[int(raw)]
                    elif typ == 'inlineStr':
                        value = ''.join(t.text or '' for t in cell.iter(f'{{{S}}}t'))
                    elif typ == 'b' and raw is not None:
                        value = raw == '1'
                    elif typ == 'n' and raw is not None:
                        value = float(raw)
                        if int(cell.get('s', '0')) in date_styles:
                            value = (epoch + dt.timedelta(days=value)).isoformat()
                            typ = 'excel_datetime'
                        elif value.is_integer():
                            value = int(value)
                    formula = cell.find(f'{{{S}}}f')
                    cells.append({'cell': cell.get('r'), 'value': value, 'type': typ,
                                  'xml_value': raw, 'style': cell.get('s'),
                                  'formula': formula.text if formula is not None else None})
                rows.append({'row': int(row.get('r')), 'attributes': dict(row.attrib), 'cells': cells})
            result.append({'name': sheet.get('name'), 'part': part, 'state': sheet.get('state', 'visible'),
                           'rows': rows, 'hyperlinks': hyperlinks, 'columns': [dict(e.attrib) for e in tree.findall(f'{{{S}}}cols/{{{S}}}col')], 'merges': [e.get('ref') for e in tree.findall(f'{{{S}}}mergeCells/{{{S}}}mergeCell')]})
        return result


def docx(path):
    """Address all textual parts, paragraphs, tables and hyperlink relationships."""
    with ZipFile(path) as z:
        parts = []
        for part in sorted(z.namelist()):
            if not (part.startswith('word/') and part.endswith('.xml')):
                continue
            tree = xml(z, part)
            if not any(e.tag in (f'{{{W}}}p', f'{{{W}}}tbl') for e in tree.iter()):
                continue
            relpath = posixpath.dirname(part) + '/_rels/' + posixpath.basename(part) + '.rels'
            rels = {e.get('Id'): dict(e.attrib) for e in xml(z, relpath)} if relpath in z.namelist() else {}
            blocks = []
            def walk(node, address):
                local = node.tag.split('}')[-1]
                if node.tag in (f'{{{W}}}p', f'{{{W}}}tbl'):
                    links = []
                    for h in node.iter(f'{{{W}}}hyperlink'):
                        rid = h.get(f'{{{R}}}id')
                        links.append({'relationship_id': rid, 'target': rels.get(rid, {}).get('Target'),
                                      'target_mode': rels.get(rid, {}).get('TargetMode'),
                                      'anchor': h.get(f'{{{W}}}anchor'),
                                      'text': ''.join(t.text or '' for t in h.iter(f'{{{W}}}t'))})
                    text = ''.join((e.text or '') if e.tag == f'{{{W}}}t' else '\t' if e.tag == f'{{{W}}}tab' else '\n' if e.tag in (f'{{{W}}}br', f'{{{W}}}cr') else '' for e in node.iter())
                    blocks.append({'kind': 'paragraph' if local == 'p' else 'table', 'xml_path': address,
                                   'text': text, 'hyperlinks': links,
                                   'field_instructions': [e.text for e in node.iter(f'{{{W}}}instrText')]})
                counts = {}
                for child in node:
                    tag = child.tag.split('}')[-1]
                    counts[tag] = counts.get(tag, 0) + 1
                    walk(child, f'{address}/{tag}[{counts[tag]}]')
            walk(tree, '/' + tree.tag.split('}')[-1] + '[1]')
            parts.append({'part': part, 'blocks': blocks, 'relationships': list(rels.values())})
        return parts

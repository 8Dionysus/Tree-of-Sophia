#!/usr/bin/env python3
"""Project bounded, source-marked ranges without modifying corpus layers.

The output is presentation metadata. It carries no new textual or semantic
authority; every range is checked against its immutable layer at generation
and again against the delivered unit before display.
"""
import argparse
import difflib
import hashlib
import json
from html.parser import HTMLParser
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
BASE = ROOT / 'ToS/source-witnesses/works/aristotle'
ITEMS = {
    'perseus-rackham-1934-en': BASE / 'nicomachean-ethics/expressions/perseus-rackham-1934-en/editions/digital-source/items/public-acquisition-20261026',
    'retained-ethics-grc2-a203a8d2e793': BASE / 'nicomachean-ethics/expressions/retained-ethics-grc2-a203a8d2e793/editions/retained-reading/items/retained-antiquity-20261027',
    'retained-athenian-constitution-grc2-6e0c298a511e': BASE / 'athenian-constitution/expressions/retained-athenian-constitution-grc2-6e0c298a511e/editions/retained-reading/items/retained-antiquity-20261027',
}
KINDS = {'edition-addition': 'addition', 'edition-deletion': 'deletion', 'edition-gap': 'gap'}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inputs(path, source_name):
    packet = json.loads((path / 'source-text-unit-packet.v1.json').read_text())
    source = path / 'payload' / source_name
    layer = path / 'payload/reading-layer.txt'
    if sha(source) != packet['source_scope']['file_sha256'] or sha(layer) != packet['source_layer']['text_layer_sha256']:
        raise ValueError(f'source or text-layer digest mismatch: {path}')
    text = layer.read_text(encoding='utf-8')
    anchors = {anchor['anchor_ref']: anchor['selector'] for anchor in packet['anchors']}
    units = []
    for unit in packet['units']:
        selector = anchors[unit['ordered_anchor_refs'][0]]
        units.append({'id': unit['unit_id'], 'start': selector['start'], 'end': selector['end']})
    return packet, source, text, units


def range_for(units, text, start, end, kind, provenance):
    matches = [unit for unit in units if unit['start'] <= start < end <= unit['end']]
    if len(matches) != 1:
        raise ValueError(f'ambiguous or cross-unit annotation at {start}:{end}')
    unit = matches[0]
    return {'unitId': unit['id'], 'start': start-unit['start'], 'end': end-unit['start'],
            'kind': kind, 'expected': text[start:end], 'provenance': provenance}


class MarkedHTML(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack, self.chars, self.marks = [], [], []

    def handle_starttag(self, tag, attrs):
        classes = dict(attrs).get('class', '').split()
        self.stack.append((tag, next((KINDS[value] for value in classes if value in KINDS), None)))

    def handle_endtag(self, tag):
        for index in range(len(self.stack)-1, -1, -1):
            if self.stack[index][0] == tag:
                del self.stack[index:]
                break

    def handle_data(self, data):
        kind = next((kind for _, kind in reversed(self.stack) if kind), None)
        self.chars.extend(data)
        self.marks.extend([kind] * len(data))


def normalized(chars, marks=None):
    out, indices, kinds = [], [], []
    pending = None
    for index, char in enumerate(chars):
        if char.isspace():
            if out and pending is None:
                pending = index
            continue
        if pending is not None:
            out.append(' '); indices.append(pending); kinds.append(None); pending = None
        out.append(char); indices.append(index); kinds.append(marks[index] if marks else None)
    return ''.join(out), indices, kinds


def rackham(path):
    packet, source, text, units = inputs(path, 'source.tei.xml')
    root = ET.parse(source).getroot()
    ns = {'t': 'http://www.tei-c.org/ns/1.0'}
    sections = root.findall('.//t:text//t:div[@subtype="subsection"]', ns)
    if len(sections) != len(units):
        raise ValueError('Rackham section and unit counts diverged')
    spans = []
    for section, unit in zip(sections, units):
        unit_text = text[unit['start']:unit['end']]
        for note in section.findall('.//t:note', ns):
            value = ''.join(note.itertext())
            if not value or unit_text.count(value) != 1:
                raise ValueError('Ambiguous Rackham note in its source subsection')
            start = unit['start'] + unit_text.index(value)
            spans.append(range_for(units, text, start, start+len(value), 'note',
                                   {'markup': 'note', 'responsibility': note.get('resp')}))
    return packet, source, spans, 0


def retained(path):
    packet, source, text, units = inputs(path, 'retained-reading.json')
    data = json.loads(source.read_text())
    spans, rejected, cursor = [], 0, 0
    for section in data['sections']:
        for block in section.get('blocks', []):
            body = block.get('text', '')
            if not body:
                continue
            start = text.find(body, cursor)
            if start < 0:
                raise ValueError('Retained block not found in reading-layer order')
            cursor = start + len(body)
            parser = MarkedHTML(); parser.feed(block.get('html', ''))
            html, _, kinds = normalized(parser.chars, parser.marks)
            plain, original_indices, _ = normalized(body)
            mapping = {}
            for a, b, size in difflib.SequenceMatcher(None, html, plain, autojunk=False).get_matching_blocks():
                mapping.update((a+i, b+i) for i in range(size))
            index = 0
            while index < len(kinds):
                kind = kinds[index]
                if not kind:
                    index += 1; continue
                end = index+1
                while end < len(kinds) and (kinds[end] == kind or (kinds[end] is None and end+1 < len(kinds) and kinds[end+1] == kind)):
                    end += 1
                context = html[max(0,index-30):min(len(html),end+30)]
                mapped = [mapping.get(i) for i in range(index,end)]
                if not context or plain.count(context) != 1 or any(value is None for value in mapped) or mapped != list(range(mapped[0],mapped[0]+len(mapped))):
                    rejected += 1; index = end; continue
                raw_start = original_indices[mapped[0]]
                raw_end = original_indices[mapped[-1]]+1
                try:
                    spans.append(range_for(units,text,start+raw_start,start+raw_end,kind,
                                           {'markup': 'retained-html', 'section': section.get('id')}))
                except ValueError:
                    rejected += 1
                index = end
    return packet, source, spans, rejected


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=ROOT / 'access/public-corpus/editorial-annotations.v1.json')
    args = parser.parse_args()
    versions = []
    for version_id, path in ITEMS.items():
        packet, source, spans, rejected = rackham(path) if version_id.startswith('perseus-') else retained(path)
        versions.append({'versionId': version_id, 'revision': packet['source_layer']['text_layer_sha256'],
                         'sourceFileSha256': sha(source), 'sourceRef': str(source.relative_to(ROOT)),
                         'spans': spans, 'rejectedAmbiguous': rejected})
    output = {'schema': 'tos.reader.editorial-annotations.v1',
              'authority': 'source markup projection; no textual correction or semantic admission', 'versions': versions}
    args.output.write_text(json.dumps(output, ensure_ascii=False, separators=(',', ':'))+'\n', encoding='utf-8')
    for version in versions:
        print(f"{version['versionId']}: {len(version['spans'])} spans; {version['rejectedAmbiguous']} rejected as ambiguous")


if __name__ == '__main__':
    main()

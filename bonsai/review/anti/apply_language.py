#!/usr/bin/env python3
"""Apply the reviewed Antiquity language ledger without reformatting its modules."""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
LEDGER = Path(__file__).with_name('language-corrections-2026-11-02.json')


def string_spans(text):
    """Index JSON string values by their exact path while retaining source formatting."""
    decoder = json.JSONDecoder()
    spans = {}
    whitespace = re.compile(r'\s*')

    def ws(i):
        return whitespace.match(text, i).end()

    def value(i, path):
        i = ws(i)
        if text[i] == '{':
            i = ws(i + 1)
            while text[i] != '}':
                key, end = decoder.raw_decode(text, i)
                i = ws(end)
                if text[i] != ':':
                    raise ValueError('Expected colon')
                i = ws(value(i + 1, path + (key,)))
                if text[i] == ',':
                    i = ws(i + 1)
                elif text[i] != '}':
                    raise ValueError('Expected comma or object end')
            return i + 1
        if text[i] == '[':
            i = ws(i + 1)
            index = 0
            while text[i] != ']':
                i = ws(value(i, path + (index,)))
                index += 1
                if text[i] == ',':
                    i = ws(i + 1)
                elif text[i] != ']':
                    raise ValueError('Expected comma or array end')
            return i + 1
        result, end = decoder.raw_decode(text, i)
        if isinstance(result, str):
            spans[path] = (i, end, result)
        return end

    end = ws(value(0, ()))
    if end != len(text):
        raise ValueError('Trailing JSON data')
    return spans


def parse_path(path):
    filename, route = path.split(':', 1)
    if not re.fullmatch(r'bonsai/antiquity/[\w-]+\.json', filename):
        raise ValueError(f'Outside Antiquity modules: {filename}')
    parts = []
    for part in route.split('.'):
        match = re.fullmatch(r'([^\[\]]+)((?:\[\d+\])*)', part)
        if not match:
            raise ValueError(f'Invalid path: {path}')
        parts.append(match.group(1))
        parts.extend(int(i) for i in re.findall(r'\[(\d+)\]', match.group(2)))
    return filename, tuple(parts)


def main():
    ledger = json.loads(LEDGER.read_text(encoding='utf-8'))
    if ledger['schema'] != 'tos.bonsai.language-corrections.v1':
        raise ValueError('Unknown ledger schema')
    documents = {}
    edits = {}
    for entry in ledger['corrections']:
        for route in [entry['path'], *entry.get('also', [])]:
            filename, base = parse_path(route)
            if filename not in documents:
                text = (ROOT / filename).read_text(encoding='utf-8')
                documents[filename] = (text, string_spans(text))
            _, spans = documents[filename]
            for lang in ('ru', 'en'):
                if lang not in entry:
                    continue
                old, new = entry[lang]
                key = base + (lang,)
                if key not in spans:
                    raise ValueError(f'Missing {route}.{lang}')
                start, end, current = spans[key]
                if current not in (old, new):
                    raise ValueError(f'Old text mismatch at {route}.{lang}: {current!r}')
                target = (filename, key)
                if target in edits:
                    raise ValueError(f'Duplicate ledger target: {route}.{lang}')
                edits[target] = (start, end, current, new)
    # Validate the whole ledger before writing any file.
    for filename, (text, _) in documents.items():
        changes = [edit for (file, _), edit in edits.items() if file == filename and edit[2] != edit[3]]
        for start, end, _, new in sorted(changes, reverse=True):
            text = text[:start] + json.dumps(new, ensure_ascii=False) + text[end:]
        if changes:
            json.loads(text)
            (ROOT / filename).write_text(text, encoding='utf-8')
        print(f'{filename}: {len(changes)} strings changed')


if __name__ == '__main__':
    main()

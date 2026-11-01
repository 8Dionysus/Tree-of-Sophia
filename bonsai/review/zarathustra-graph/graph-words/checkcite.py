#!/usr/bin/env python3
"""Checks every cite in a JSON file against the German text: chapter exists, units exist in that chapter
and run in order, offsets fit, and an optional quote "q" stands in the cited text.

  python3 checkcite.py out/graph-words/concepts/fate.json"""
import json, os, re, sys
HERE = os.path.dirname(os.path.abspath(__file__))
rows = [json.loads(l) for l in open(os.path.join(HERE, 'corpus', 'rows.jsonl'))]
pos, text, seen = {}, {}, {}
for r in rows:
    for u in r['de']:
        n = seen.get(r['chapter'], 0); seen[r['chapter']] = n + 1
        pos[u['id']] = (r['chapter'], n); text[u['id']] = u['text']
norm = lambda t: re.sub(r'\s+', ' ', t).strip().lower()
bad = total = 0
def walk(x, path):
    global bad, total
    if isinstance(x, dict):
        if x.get('w') == 'also-sprach-zarathustra' and 'u' in x:
            total += 1; c, (a, b) = x.get('c'), x['u']; why = []
            if a not in pos or b not in pos: why.append('unknown unit')
            elif pos[a][0] != c or pos[b][0] != c: why.append(f'units are in {pos[a][0]}/{pos[b][0]}, not {c}')
            elif pos[a][1] > pos[b][1]: why.append('units out of order')
            else:
                ids = [k for k, v in pos.items() if v[0] == c and pos[a][1] <= v[1] <= pos[b][1]]
                joined = ' '.join(text[k] for k in ids)
                if 'q' in x and norm(x['q']) not in norm(joined): why.append('quote not in cited text')
                if 'e' in x and x['e'] > len(text[b]): why.append('end offset past unit')
            if why: bad += 1; print(path, json.dumps(x, ensure_ascii=False), '->', '; '.join(why))
        for k, v in x.items(): walk(v, f'{path}.{k}')
    elif isinstance(x, list):
        for i, v in enumerate(x): walk(v, f'{path}[{i}]')
for f in sys.argv[1:]: walk(json.load(open(f)), os.path.basename(f))
print(f'{total} cites, {bad} with problems')
sys.exit(1 if bad else 0)

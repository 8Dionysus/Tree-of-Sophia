#!/usr/bin/env python3
"""Every place of a word in «Zarathustra», with its neighbourhood.

  python3 kwic.py 'Schicksal|Loos|Verhängniss'            # hits in German, sentence + Russian row
  python3 kwic.py 'Muth' --count                          # forms and chapters only
  python3 kwic.py 'Löwe' --ctx 1                          # also the rows before and after (second order)
  python3 kwic.py 'судьб' --lang ru                       # search the translation (to see where it shifts)
  python3 kwic.py 'Kind' --chapters I.01,II.01 --full     # whole rows

The pattern is a Python regex, case-insensitive; \\b word edges help (r'\\bKind'). Each hit prints
chapter, row, unit id and the matched forms, so a cite {"w":"also-sprach-zarathustra","c":chapter,"u":[id,id]}
can be written straight from it."""
import argparse, collections, json, os, re
HERE = os.path.dirname(os.path.abspath(__file__))
ap = argparse.ArgumentParser()
ap.add_argument('pattern'); ap.add_argument('--lang', default='de', choices=['de', 'ru', 'en'])
ap.add_argument('--ctx', type=int, default=0); ap.add_argument('--chapters', default='')
ap.add_argument('--count', action='store_true'); ap.add_argument('--full', action='store_true')
ap.add_argument('--corpus', default=os.path.join(HERE, 'corpus'))
a = ap.parse_args()
pat = re.compile(a.pattern, re.I)
rows = [json.loads(l) for l in open(os.path.join(a.corpus, 'rows.jsonl'))]
titles = {c['chapter']: c['titles'] for c in json.load(open(os.path.join(a.corpus, 'chapters.json')))}
only = set(filter(None, a.chapters.split(',')))
forms, per_chapter, hits = collections.Counter(), collections.Counter(), []
for i, r in enumerate(rows):
    if only and r['chapter'] not in only: continue
    for u in r[a.lang]:
        found = [m.group() for m in pat.finditer(u['text'])]
        if found:
            forms.update(found); per_chapter[r['chapter']] += len(found); hits.append((i, u, found))
print(f'{sum(forms.values())} hits in {len(hits)} units, {len(per_chapter)} chapters')
print('forms:', ', '.join(f'{k}×{v}' for k, v in forms.most_common()))
print('chapters:', ', '.join(f'{k}×{v}' for k, v in per_chapter.items()))
if a.count: raise SystemExit
sentences = lambda t: re.split(r'(?<=[.!?;:—])\s+', t)
shorten = lambda t, n=600: t if a.full or len(t) <= n else t[:n] + ' …'
for i, u, found in hits:
    r = rows[i]
    print(f"\n## {r['chapter']} «{titles[r['chapter']]['de']}» row {r['row']} · {u['id']} · {', '.join(dict.fromkeys(found))}")
    text = u['text'] if a.full else ' '.join(s for s in sentences(u['text']) if pat.search(s))
    print(f'{a.lang}: {shorten(text)}')
    for other in ('de', 'ru'):
        if other != a.lang: print(f"{other}: {shorten(' '.join(x['text'] for x in r[other]), 900)}")
    for j in range(max(0, i - a.ctx), min(len(rows), i + a.ctx + 1)):
        if j != i and rows[j]['chapter'] == r['chapter']:
            print(f"  ctx row {rows[j]['row']} ({'before' if j < i else 'after'}) {rows[j]['de'][0]['id'] if rows[j]['de'] else ''}: {shorten(' '.join(x['text'] for x in rows[j]['de']), 500)}")

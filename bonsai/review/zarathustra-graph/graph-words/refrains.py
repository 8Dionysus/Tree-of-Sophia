#!/usr/bin/env python3
"""The book quoting itself: word sequences that recur across chapters.

  python3 refrains.py 'Übermensch'          # recurring 3-6-word sequences in the rows where the pattern occurs,
                                            # with every other chapter they recur in
  python3 refrains.py --all --min-chapters 3  # the book's refrains overall

A refrain joins places the way the book itself joins them: by repeating its own words."""
import argparse, collections, json, os, re
HERE = os.path.dirname(os.path.abspath(__file__))
ap = argparse.ArgumentParser(); ap.add_argument('pattern', nargs='?'); ap.add_argument('--all', action='store_true')
ap.add_argument('--min', type=int, default=3); ap.add_argument('--max', type=int, default=6)
ap.add_argument('--min-chapters', type=int, default=2); ap.add_argument('--max-count', type=int, default=12)
ap.add_argument('--corpus', default=os.path.join(HERE, 'corpus'))
a = ap.parse_args()
rows = [json.loads(l) for l in open(os.path.join(a.corpus, 'rows.jsonl'))]
COMMON = set('und der die das ich du er es sie wir ihr nicht ist zu in den dem des ein eine einer mit so also sprach zarathustra was wie da dass daß auf an von für mir mich dich dir sich sein seine'.split())
words = lambda t: re.findall(r"[A-Za-zÄÖÜäöüß]+", t.lower())
where = collections.defaultdict(list)
for r in rows:
    for u in r['de']:
        w = words(u['text'])
        for n in range(a.min, a.max + 1):
            for k in range(len(w) - n + 1):
                g = tuple(w[k:k + n])
                if sum(x not in COMMON for x in g) >= 2: where[g].append((r['chapter'], u['id']))
def show(g, places):
    chs = list(dict.fromkeys(c for c, _ in places))
    print(f"«{' '.join(g)}» ×{len(places)} in {len(chs)}: " + '; '.join(f'{c} {u}' for c, u in places))
good = {g: p for g, p in where.items() if len({c for c, _ in p}) >= a.min_chapters and len(p) <= a.max_count}
# keep the longest sequences: drop a sequence contained in a longer one with the same places
keys = sorted(good, key=len, reverse=True); kept = []
for g in keys:
    s = ' '.join(g)
    if not any(s in ' '.join(h) and len(good[h]) == len(good[g]) for h in kept): kept.append(g)
if a.pattern:
    pat = re.compile(a.pattern, re.I)
    units = {u['id'] for r in rows for u in r['de'] if pat.search(u['text'])}
    kept = [g for g in kept if any(u in units for _, u in good[g])]
for g in sorted(kept, key=lambda g: (-len({c for c, _ in good[g]}), -len(g))): show(g, good[g])
print(f'{len(kept)} refrains')

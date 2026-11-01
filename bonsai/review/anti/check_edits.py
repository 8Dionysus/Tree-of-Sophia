#!/usr/bin/env python3
"""Checks an edit output against its slice: one line per item, same paths, both languages present or both empty."""
import json, os, sys
S = os.path.dirname(os.path.abspath(__file__))
name = sys.argv[1]
items_path = f'{S}/edit-slices/{name}.json'
out_path = f'{S}/edit-out/{name}.jsonl'
if not os.path.isfile(items_path):
    raise SystemExit(
        f'Cannot check Antiquity edit output: {items_path} is missing. '
        f'Rebuild its context with `python3 -B bonsai/review/anti/context.py {name} <classification-slice>...` '
        'after generating anti/alignment.current.json.'
    )
if not os.path.isfile(out_path):
    raise SystemExit(f'Cannot check Antiquity edit output: {out_path} is missing.')
items = json.load(open(items_path))
out = [json.loads(l) for l in open(out_path) if l.strip()]
problems = []
if len(out) != len(items): problems.append(f'{len(out)} lines for {len(items)} items')
for i, (a, b) in enumerate(zip(items, out)):
    if a['path'] != b.get('path'): problems.append(f'#{i}: path {b.get("path")} != {a["path"]}')
    ru, en = (b.get('ru') or '').strip(), (b.get('en') or '').strip()
    if bool(ru) != bool(en): problems.append(f'#{i} {a["path"]}: only one language')
    if not ru and not a.get('cite') and a['verdict'] == 'keep': problems.append(f'#{i} {a["path"]}: emptied a form that has no fragment')
    if 'changed' not in b or not b.get('why') and b.get('changed'): problems.append(f'#{i}: changed without why')
    if not b.get('changed') and a.get('kept') and (ru, en) != ((a['kept'].get('ru') or '').strip(), (a['kept'].get('en') or '').strip()) and a['verdict'] == 'trim':
        problems.append(f'#{i} {a["path"]}: text differs but changed is false')
for p in problems[:100]: print(p)
print(f'{name}: {len(out)} lines, {len(problems)} problems')

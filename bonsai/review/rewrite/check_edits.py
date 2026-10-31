#!/usr/bin/env python3
"""Checks an edit output against its slice: one line per item, same paths, both languages present or both empty."""
import json, sys
name = sys.argv[1]
items = json.load(open(f'edit-slices/{name}.json'))
out = [json.loads(l) for l in open(f'edit-out/{name}.jsonl') if l.strip()]
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

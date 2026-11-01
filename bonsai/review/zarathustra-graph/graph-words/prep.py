#!/usr/bin/env python3
"""Flat, aligned text of «Also sprach Zarathustra» for searching: one row per aligned paragraph.

  python3 prep.py <witnesses-dir> <out-dir>

Writes rows.jsonl ({chapter, row, de, ru, en}, each language a list of {id, text}) and chapters.json.
German is the DTA text of the first editions (old spelling: Muth, Loos, Thier, Noth); ru is Antonovsky 1913,
en is Common 1909. Line-break hyphens (¬) are joined and whitespace collapsed; ids are the Tree's unit ids."""
import glob, json, os, re, sys
src, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
clean = lambda t: re.sub(r'\s+', ' ', re.sub(r'¬\s*', '', t)).strip()
order = lambda p: [{'I':1,'II':2,'III':3,'IV':4}.get(x, 0) if i == 0 else int(x) for i, x in enumerate(os.path.basename(p)[:-5].split('.'))]
chapters, rows = [], 0
with open(os.path.join(out, 'rows.jsonl'), 'w') as f:
    for path in sorted(glob.glob(os.path.join(src, '*.json')), key=order):
        d = json.load(open(path))
        chapters.append({'chapter': d['chapter'], 'titles': d['titles']})
        for n, r in enumerate(d['rows']):
            item = {'chapter': d['chapter'], 'row': n}
            for lang in ('de', 'ru', 'en'):
                item[lang] = [{'id': d['units'][lang][i][0], 'text': clean(d['units'][lang][i][2])} for i in r.get(lang, [])]
            f.write(json.dumps(item, ensure_ascii=False) + '\n'); rows += 1
json.dump(chapters, open(os.path.join(out, 'chapters.json'), 'w'), ensure_ascii=False, indent=1)
print(f'{len(chapters)} chapters, {rows} rows -> {out}')

"""Applies the edited own words to the mechanically rewritten content and the few own-word fixes the
pass found outside the classified forms; writes final.json."""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, glob, re, sys
D = REVIEW + '/rewrite'
new = json.load(open(f'{D}/new.json'))
edits = {e['path']: e for e in json.load(open(f'{D}/edits.json'))}
outs = {}
for f in sorted(glob.glob(f'{D}/edit-out/e*.jsonl')):
    for line in open(f):
        if line.strip():
            o = json.loads(line); outs[o['path']] = o
def walk(path):
    m = re.match(r'^([A-Z_]+)\.?(.*)$', path)
    obj = new[m.group(1)]; rest = m.group(2)
    while True:
        if rest.startswith('['):
            i = rest.index(']'); key = int(rest[1:i]); rest = rest[i + 1:]
        else:
            keys = sorted((k for k in obj if rest == k or rest.startswith(k + '.') or rest.startswith(k + '[')), key=len, reverse=True)
            key = keys[0]; rest = rest[len(key):]
        if rest.startswith('.'): rest = rest[1:]
        if not rest: return obj, key
        obj = obj[key]
applied = emptied = unchanged = missing = 0
for path, e in edits.items():
    o = outs.get(path)
    if o is None: missing += 1; continue
    if e['verdict'] == 'replace': continue
    parent, key = walk(path)
    ru, en = (o.get('ru') or '').strip(), (o.get('en') or '').strip()
    value = parent.get(key) if key != 'focus' else parent.get('focus')
    if key == 'focus':
        if ru: parent['focus'] = {'ru': ru, 'en': en}
        elif parent.get('cite'): parent.pop('focus', None); emptied += 1; continue
        else: unchanged += 1; continue
    else:
        cite = (value or {}).get('cite')
        if ru: parent[key] = {'ru': ru, 'en': en, **({'cite': cite} if cite else {})}
        elif cite: parent[key] = {'cite': cite}; emptied += 1; continue
        else: unchanged += 1; continue
    applied += 1
# The tree lens reads the foundation's own grounds (one object in the authored module).
new['LENS_INQUIRY']['tree']['grounds'] = json.loads(json.dumps(new['INQUIRY_FOUNDATION']['grounds']))
# Own words outside the classified forms that the pass found wrong.
step = new['RESEARCH_ROUTES'][6]['steps'][2]
assert step['title']['ru'] == 'Следовать вопросу собаки', step['title']
step['title'] = {'ru': 'Следовать за зовом собаки', 'en': 'Follow the dog’s call'}
# "Ripeness for work" rests on «Zarathustra ward reif» (IV.20, the departure), which the retelling had misplaced.
ripe = new['NODE_CONTENT']['work']['perspectives'][1]
assert ripe['title']['en'] == 'Ripeness for work'
reif = {'w': 'also-sprach-zarathustra', 'c': 'IV.20', 'u': ['tos.text-unit.sid-3aed787db3bc681b14b7a606fac6fe70'] * 2, 's': 46, 'e': 67}
if reif not in ripe['body'].setdefault('cite', []): ripe['body']['cite'].append(reif)
# A link between two places of the text keeps its own words, and the places it links are shown beside them.
W = REPO_ROOT + '/bonsai/zarathustra/witnesses'
chapter_order = [c['key'] for c in json.load(open(W + '.json'))['chapters']]
unit_order = {}
def order_of(c):
    if c not in unit_order:
        unit_order[c] = {u[0]: i for i, u in enumerate(json.load(open(f'{W}/{c}.json'))['units']['de'])}
    return unit_order[c]
def span(cite):
    o = order_of(cite['c'])
    return (o[cite['u'][0]], cite.get('s', 0)), (o[cite['u'][1]], cite.get('e', 10 ** 9))
def overlaps(x, y):
    if x.get('w') != y.get('w'): return False
    if 'loc' in x or 'loc' in y: return x.get('loc') == y.get('loc')
    if x.get('c') != y.get('c'): return False
    if not x.get('c'): return x.get('u') == y.get('u')
    (xa, xb), (ya, yb) = span(x), span(y)
    return xa < yb and ya < xb
def work_order(cite):
    if cite.get('c'): return (0, chapter_order.index(cite['c']), span(cite)[0])
    return (1, cite.get('w', ''), cite.get('loc', ''))
links = json.load(open(f'{D}/links.json'))
placed = speechless = 0
for link in links:
    parent, key = walk(link['path'])
    form = parent if key == 'focus' else parent[key]
    if not (form.get('ru') if key != 'focus' else (parent.get('focus') or {}).get('ru')): speechless += 1
    cites = form.setdefault('cite', [])
    ends = [end for end in sorted([link['from'], link['to']], key=work_order) if not any(overlaps(end, c) for c in cites)]
    cites.extend(ends); placed += len(ends)
print('link ends placed', placed, 'links whose words are gone', speechless)
json.dump(new, open(f'{D}/final.json', 'w'), ensure_ascii=False, indent=1)
print('applied', applied, 'emptied', emptied, 'unchanged', unchanged, 'missing outputs', missing)

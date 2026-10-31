"""Mechanical pass over the Antiquity bonsai: every retelling (R) and navigation (N) sentence gives way to
a citation of the place in the original (Greek or Latin) that it retold; own words stay as authored.

Writes anti/rewrite/new-collection.json, new-first-branches.json, edits.json, links.json, report.json."""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, glob, re, sys, os, collections, subprocess
S = REVIEW
sys.path.insert(0, f'{S}/frag')
from corpus import version_units
import zc
REPO = REPO_ROOT
OUT = f'{S}/anti/rewrite'
os.makedirs(OUT, exist_ok=True)
git = lambda path: json.loads(subprocess.check_output(['git', '-C', REPO, 'show', f'HEAD:{path}']))
data = {'collection': git('bonsai/antiquity/collection.json'), 'first-branches': git('bonsai/antiquity/first-branches.json')}
identities = json.load(open(f'{REPO}/bonsai/antiquity/identities.json'))
work_key = {v['tree']: k for k, v in identities['works'].items()}
rows, also = [], {}
for f in sorted(glob.glob(f'{S}/anti/out/a*.jsonl')):
    name = os.path.basename(f)[:-6]
    for item in json.load(open(f'{S}/anti/slices/{name}.json')): also[item['path']] = item.get('also', [])
    for line in open(f):
        r = json.loads(line); r['_slice'] = name; rows.append(r)
norm = lambda s: re.sub(r'\s+', ' ', s).strip()
units_cache = {}
def unit_of(doc_id, lang, uid):
    key = (doc_id, lang)
    if key not in units_cache:
        v = zc.pick(doc_id, lang); us, _ = version_units(doc_id, v)
        units_cache[key] = {u['id']: u for u in us}
    return units_cache[key].get(uid)
def locate(text, quote):
    q = norm(quote)
    chars, pos, prev = [], [], False
    for i, ch in enumerate(text):
        if ch.isspace():
            if chars and not prev: chars.append(' '); pos.append(i)
            prev = True
        else: chars.append(ch); pos.append(i); prev = False
    t = ''.join(chars); k = t.find(q)
    if k < 0: return None
    return pos[k], pos[k + len(q) - 1] + 1
EXTERNAL = [  # (pattern in the work string, bonsai work key)
    (r'De anima', 'de-anima'), (r'Seneca.*Epistul|Epistulae morales|Letters to Lucilius', 'letters'), (r'Sextus|Outlines of Pyrrhonism|Pyrrh', 'outlines'),
    (r'Memorabilia', 'memorabilia'), (r'Menoeceus', 'menoeceus'), (r'Principal Doctrines|Kyriai', 'doctrines'),
    (r'Sententiae Vaticanae|Vatican Sayings', 'vatican-sayings'), (r'Protagoras, fragments|Diels.Kranz.*Protagoras|DK 80', 'protagoras-fragments'),
    (r'Vita Plotini|Life of Plotinus', 'vita-plotini'),
]
def external_key(work):
    for pat, key in EXTERNAL:
        if re.search(pat, work, re.I): return key
    return None
def loc_of(locator, key=None):
    loc = re.sub(r'\s+', ' ', locator.replace('–', '-').replace(',', ' ')).strip()
    # Sextus is cited by book in Roman numerals (I.1), as the stand's other places of the Outlines are.
    if key == 'outlines': loc = re.sub(r'^1\.', 'I.', loc)
    return loc
problems = []
# Spans the edit pass found short: the quote runs on to the end of the sentence the kept own words discuss.
QUOTE_FIX = {('collection.terms[10].uses[1].body', 'καὶ τοῖς γιγνωσκομένοις τοίνυν μὴ μόνον τὸ γιγνώσκεσθαι φάναι ὑπὸ τοῦ ἀγαθοῦ παρεῖναι, ἀλλὰ καὶ τὸ εἶναί τε καὶ τὴν οὐσίαν ὑπʼ ἐκείνου αὐτοῖς προσεῖναι'):
             'καὶ τοῖς γιγνωσκομένοις τοίνυν μὴ μόνον τὸ γιγνώσκεσθαι φάναι ὑπὸ τοῦ ἀγαθοῦ παρεῖναι, ἀλλὰ καὶ τὸ εἶναί τε καὶ τὴν οὐσίαν ὑπʼ ἐκείνου αὐτοῖς προσεῖναι, οὐκ οὐσίας ὄντος τοῦ ἀγαθοῦ, ἀλλʼ ἔτι ἐπέκεινα τῆς οὐσίας πρεσβείᾳ καὶ δυνάμει ὑπερέχοντος.'}
def cites_of(r, s):
    out = []
    for a in s.get('anchors') or []:
        if 'work' in a:
            key = external_key(a['work'])
            if not key: problems.append((r['path'], f'external work not mapped: {a["work"]}')); continue
            out.append({'w': key, 'loc': loc_of(a.get('locator') or '', key)}); continue
        try: doc_id = zc.resolve(a['doc'])
        except SystemExit as e: problems.append((r['path'], str(e))); continue
        key = work_key.get(doc_id) or doc_id.split('.')[-1]
        us = a.get('units') or []
        if not us: problems.append((r['path'], 'anchor without units')); continue
        cite = {'w': key, 'u': [us[0], us[-1]]}
        if a.get('quote'):
            quote = QUOTE_FIX.get((r['path'], a['quote']), a['quote'])
            u = unit_of(doc_id, a['lang'], us[0])
            at = locate(u['text'], quote) if u else None
            if at:
                cite['s'] = at[0]
                if len(us) == 1: cite['e'] = at[1]
            else: problems.append((r['path'], f'quote not located: {quote[:50]}'))
        out.append(cite)
    return out
def walk(path):
    m = re.match(r'^(collection|first-branches)\.?(.*)$', path)
    obj = data[m.group(1)]; rest = m.group(2)
    while True:
        if rest.startswith('['):
            i = rest.index(']'); key = int(rest[1:i]); rest = rest[i + 1:]
        else:
            keys = sorted((k for k in obj if rest == k or rest.startswith(k + '.') or rest.startswith(k + '[')), key=len, reverse=True)
            if not keys: raise KeyError(path)
            key = keys[0]; rest = rest[len(key):]
        if rest.startswith('.'): rest = rest[1:]
        if not rest: return obj, key
        obj = obj[key]
def dedupe(items):
    seen, out = set(), []
    for c in items:
        k = json.dumps(c, sort_keys=True)
        if k not in seen: seen.add(k); out.append(c)
    return out
report = collections.Counter(); edits = []; links = []
for r in rows:
    retold, own, linked = [], [], []
    for s in r['sentences']:
        cs = cites_of(r, s)
        if s['class'] in 'RN': retold.extend(cs)
        else: own.append(s)
        if s['class'] == 'L' and s.get('link'):
            # A link keeps its words, and the two places it links are shown beside them.
            ends = cites_of(r, {'anchors': [e for e in (s['link'].get('from'), s['link'].get('to')) if e]})
            links.append({'path': r['path'], 'ru': s['ru'], 'en': s['en'], 'kind': s['link'].get('kind'), 'ends': ends})
            linked.extend(ends)
    retold = dedupe(retold)
    # A linked place already shown by a cited place that covers it (the same locator, or the same units
    # with a span that holds the linked span) is not shown twice.
    def covered(c, x):
        if c['w'] != x['w']: return False
        if 'loc' in c or 'loc' in x: return c.get('loc') == x.get('loc')
        return c['u'] == x['u'] and x.get('s', 0) <= c.get('s', 0) and x.get('e', 10**9) >= c.get('e', 10**9)
    added = [c for c in dedupe(linked) if not any(covered(c, x) for x in retold)]
    report['link-ends-shown'] += len(added)
    retold += added
    for path in [r['path']] + also.get(r['path'], []):
        parent, key = walk(path)
        value = parent[key]
        if path == r['path'] and (norm(value.get('ru', '')) != norm(' '.join(s['ru'] for s in r['sentences'])) or norm(value.get('en', '')) != norm(' '.join(s['en'] for s in r['sentences']))):
            problems.append((path, 'text differs from classification'))
        if r['verdict'] == 'replace':
            if not retold: problems.append((path, 'replace without citation: kept')); continue
            parent[key] = {'cite': retold}
        elif r['verdict'] == 'trim':
            parent[key] = {'ru': ' '.join(s['ru'] for s in own), 'en': ' '.join(s['en'] for s in own), **({'cite': retold} if retold else {})}
        report[r['verdict']] += 1
    flags = [f for f in r.get('flags') or [] if not f.startswith('corpus-defect')]
    if r['verdict'] == 'trim' or any(f.split(':')[0] in ('factual-error', 'misattribution', 'self-contradiction') for f in flags):
        parent, key = walk(r['path'])
        edits.append({'path': r['path'], 'also': also.get(r['path'], []), 'slice': r['_slice'], 'verdict': r['verdict'],
                      'original': {'ru': ' '.join(s['ru'] for s in r['sentences']), 'en': ' '.join(s['en'] for s in r['sentences'])},
                      'sentences': [{'class': s['class'], 'ru': s['ru'], 'en': s['en'], 'note': s.get('note', '')} for s in r['sentences']],
                      'kept': parent[key] if r['verdict'] != 'replace' else None, 'cite': retold, 'flags': r.get('flags') or []})
json.dump(data['collection'], open(f'{OUT}/new-collection.json', 'w'), ensure_ascii=False)
json.dump(data['first-branches'], open(f'{OUT}/new-first-branches.json', 'w'), ensure_ascii=False)
json.dump(edits, open(f'{OUT}/edits.json', 'w'), ensure_ascii=False, indent=1)
json.dump(links, open(f'{OUT}/links.json', 'w'), ensure_ascii=False, indent=1)
flagged = [{'path': r['path'], 'flags': r['flags']} for r in rows if r.get('flags')]
json.dump({'counts': report, 'problems': problems, 'flags': flagged}, open(f'{OUT}/report.json', 'w'), ensure_ascii=False, indent=1)
print(dict(report), 'edits', len(edits), 'links', len(links), 'problems', len(problems))
print(collections.Counter(re.sub(r':.*', '', p) for _, p in problems).most_common(8))

"""Applies the edited own words (edit-out/ae*.jsonl) to the mechanically rewritten Antiquity modules
(rewrite/new-*.json) and writes bonsai/antiquity/collection.json and first-branches.json in their own
formatting. Material IDs and the shape of every module stay as they were; only forms change."""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, glob, re, collections, sys, os
S = REVIEW + '/anti'
original = json.load(open(f'{S}/original.json'))
data = {'collection': json.load(open(f'{S}/rewrite/new-collection.json')), 'first-branches': json.load(open(f'{S}/rewrite/new-first-branches.json'))}
edits = json.load(open(f'{S}/rewrite/edits.json'))
outs = {}
for f in sorted(glob.glob(f'{S}/edit-out/ae*.jsonl')):
    for line in open(f):
        if line.strip():
            o = json.loads(line); outs[o['path']] = o

def walk(path):
    m = re.match(r'^(collection|first-branches)\.?(.*)$', path)
    obj, rest = data[m.group(1)], m.group(2)
    while True:
        if rest.startswith('['):
            i = rest.index(']'); key = int(rest[1:i]); rest = rest[i + 1:]
        else:
            k = re.match(r'[^.\[]+', rest).group(0); key = k; rest = rest[len(k):]
        if rest.startswith('.'): rest = rest[1:]
        if not rest: return obj, key
        obj = obj[key]

applied = emptied = kept = missing = 0
for e in edits:
    if e['verdict'] == 'replace': continue
    o = outs.get(e['path'])
    if o is None: missing += 1; continue
    if not o.get('changed'): kept += 1; continue
    ru, en = (o.get('ru') or '').strip(), (o.get('en') or '').strip()
    for path in [e['path'], *(e.get('also') or [])]:
        parent, key = walk(path)
        value = parent[key] if isinstance(parent[key], dict) else {}
        cite = value.get('cite')
        if ru: parent[key] = {'ru': ru, 'en': en, **({'cite': cite} if cite else {})}
        elif cite: parent[key] = {'cite': cite}
        else: raise SystemExit(f'{path}: emptied without a citation')
    if ru: applied += 1
    else: emptied += 1

# A link labelled as the stand's working translation into Russian opens an English translation: it is
# named as what it opens.
ENGLISH = [(r'^https://github\.com/PerseusDL/.*eng\d*\.xml$', {'ru': 'Английский перевод (TEI)', 'en': 'English translation (TEI)'}),
           (r'^https://classics\.mit\.edu/', {'ru': 'Английский перевод · The Internet Classics Archive (MIT)', 'en': 'English translation · The Internet Classics Archive (MIT)'}),
           (r'^https://www\.gutenberg\.org/', {'ru': 'Английский перевод · Project Gutenberg', 'en': 'English translation · Project Gutenberg'}),
           (r'^https://en\.wikisource\.org/', {'ru': 'Английский перевод · Викитека', 'en': 'English translation · Wikisource'})]
relabelled = 0
def relabel(value):
    global relabelled
    if isinstance(value, dict):
        for ref in value.get('sourceRefs') or []:
            if isinstance(ref, dict) and (ref.get('label') or {}).get('en') in ('Workbench translation', 'Demo stand: working translation'):
                label = next((l for pattern, l in ENGLISH if re.match(pattern, ref.get('url', ''))), None)
                if not label: raise SystemExit(f'unlabelled working translation link: {ref.get("url")}')
                ref['label'] = dict(label); ref['language'] = 'en'; relabelled += 1
        for v in value.values(): relabel(v)
    elif isinstance(value, list):
        for v in value: relabel(v)
for module in data.values(): relabel(module)
# Links that cannot open their place as written: a CTS range written with an en dash (Scaife reads it as the
# first page only) is written as a range; Long's selection of the Discourses opens at his biographical note
# (#chap01) and lacks Arrian's letter and I.11 altogether, so it is linked as the book, and not at all where
# the passage is not in it.
def cts_range(url):
    def fix(m):
        a, b = m.group(1), m.group(2)
        if re.fullmatch(r'[a-e]', b): b = re.match(r'\d+', a).group(0) + b
        return f':{a}-{b}/'
    return re.sub(r':(\d+[a-e]?)\u2013(\d*[a-e]?)/$', fix, url)
NOT_IN_LONG = {'discourses-record', 'discourses-affection'}
fixed = collections.Counter()
def fix_links(value, owner=None):
    if isinstance(value, dict):
        owner = value.get('id', owner)
        refs = value.get('sourceRefs')
        if isinstance(refs, list):
            kept = []
            for ref in refs:
                url = ref.get('url', '') if isinstance(ref, dict) else ''
                if 'scaife.perseus.org' in url and cts_range(url) != url: ref['url'] = cts_range(url); fixed['cts range'] += 1
                if '10661-h.htm#chap01' in url:
                    if owner in NOT_IN_LONG: fixed['dropped: not in the selection'] += 1; continue
                    ref['url'] = url.replace('#chap01', ''); fixed['selection as a book'] += 1
                kept.append(ref)
            value['sourceRefs'] = kept
        for v in value.values(): fix_links(v, owner)
    elif isinstance(value, list):
        for v in value: fix_links(v, owner)
for module in data.values(): fix_links(module)
# The Internet Classics Archive (MIT) gives a book of a work per page; a passage links the page of its own book
# (many linked the page of Book I or VII whatever their book), and a work links the work's contents page. Every
# page used here was found listed under classics.mit.edu by a web search on 30 October 2026, or was already used.
ROMAN = {r: i for i, r in enumerate('I II III IV V VI VII VIII IX X XI XII XIII XIV'.split(), 1)}
WORD = {1: 'one', 2: 'two', 3: 'three', 4: 'four', 5: 'five', 6: 'six', 7: 'seven', 8: 'eight'}
REPUBLIC_BOOKS = [(327, 354), (357, 383), (386, 417), (419, 445), (449, 480), (484, 511), (514, 541), (543, 569), (571, 592), (595, 621)]
MIT = 'https://classics.mit.edu/'
def rom(n): return next(r for r, i in ROMAN.items() if i == n).lower()
MIT_BOOK = {'republic': lambda n: f'{MIT}Plato/republic.{n + 1}.{rom(n)}.html', 'ethics': lambda n: f'{MIT}Aristotle/nicomachaen.{n}.{rom(n)}.html',
            'metaphysics': lambda n: f'{MIT}Aristotle/metaphysics.{n}.{rom(n)}.html', 'politics': lambda n: f'{MIT}Aristotle/politics.{n}.{WORD[n]}.html',
            'de-anima': lambda n: f'{MIT}Aristotle/soul.{n}.{rom(n)}.html', 'on-nature': lambda n: f'{MIT}Carus/nature_things.{n}.{rom(n)}.html',
            'meditations': lambda n: f'{MIT}Antoninus/meditations.{n}.{WORD[n]}.html'}
MIT_WORK = {'republic': 'Plato/republic.html', 'ethics': 'Aristotle/nicomachaen.html', 'metaphysics': 'Aristotle/metaphysics.html', 'politics': 'Aristotle/politics.html',
            'de-anima': 'Aristotle/soul.html', 'on-nature': 'Carus/nature_things.html', 'meditations': 'Antoninus/meditations.html'}
FOUND = {MIT + x for x in (
    'Plato/republic.2.i.html Plato/republic.3.ii.html Plato/republic.5.iv.html Plato/republic.6.v.html Plato/republic.7.vi.html '
    'Plato/republic.8.vii.html Plato/republic.9.viii.html Plato/republic.11.x.html Aristotle/nicomachaen.1.i.html Aristotle/nicomachaen.2.ii.html '
    'Aristotle/nicomachaen.3.iii.html Aristotle/nicomachaen.5.v.html Aristotle/nicomachaen.6.vi.html Aristotle/nicomachaen.8.viii.html '
    'Aristotle/nicomachaen.10.x.html Aristotle/metaphysics.1.i.html Aristotle/metaphysics.3.iii.html Aristotle/metaphysics.4.iv.html '
    'Aristotle/metaphysics.7.vii.html Aristotle/metaphysics.9.ix.html Aristotle/metaphysics.12.xii.html Aristotle/politics.1.one.html '
    'Aristotle/politics.2.two.html Aristotle/politics.3.three.html Aristotle/politics.4.four.html Aristotle/politics.8.eight.html '
    'Aristotle/soul.2.ii.html Aristotle/soul.3.iii.html Carus/nature_things.1.i.html Carus/nature_things.2.ii.html Carus/nature_things.3.iii.html '
    'Carus/nature_things.4.iv.html Carus/nature_things.5.v.html Antoninus/meditations.2.two.html Antoninus/meditations.8.eight.html').split()} | {MIT + x for x in MIT_WORK.values()}
def book_of(passage):
    meta = passage['meta']['en']
    m = re.match(r'^([IVX]+)(?=[.·\s]|$)', meta)
    if m: return ROMAN[m.group(1)]
    if passage['workId'] == 'republic':
        page = int(re.match(r'\d+', meta).group(0))
        return next(n for n, (a, b) in enumerate(REPUBLIC_BOOKS, 1) if a <= page <= b)
    return None
def book_label(label, n):
    return {'ru': re.sub(r'книга [IVX]+', f'книга {rom(n).upper()}', label['ru']), 'en': re.sub(r'Book [IVX]+', f'Book {rom(n).upper()}', label['en'])}
def work_label(label):
    return {'ru': re.sub(r' · книга [IVX]+', '', label['ru']), 'en': re.sub(r' · Book [IVX]+', '', label['en'])}
def same_refs(refs):
    """One link per page; of two links to one page the one that names its translator stays."""
    named = lambda ref: not (ref.get('label') or {}).get('en', '').startswith('English translation ·')
    out = []
    for ref in refs:
        twin = next((i for i, x in enumerate(out) if x.get('url') == ref.get('url')), None)
        if twin is None: out.append(ref); continue
        fixed['MIT: the same page listed twice'] += 1
        if named(ref) and not named(out[twin]): out[twin] = ref
    return out
for passage in data['collection']['passages']:
    if passage['workId'] not in MIT_BOOK or not any(MIT in (ref.get('url') or '') for ref in passage.get('sourceRefs') or []): continue
    n = book_of(passage); url = MIT_BOOK[passage['workId']](n)
    if url not in FOUND: raise SystemExit(f'{passage["id"]}: {url} not found')
    for ref in passage['sourceRefs']:
        if MIT in ref.get('url', '') and ref['url'] != url: ref['url'] = url; ref['label'] = book_label(ref['label'], n); fixed["MIT: the page of the passage's book"] += 1
    passage['sourceRefs'] = same_refs(passage['sourceRefs'])
for entity in data['collection']['entities']:
    if entity['kind'] != 'work' or entity['id'] not in MIT_WORK: continue
    for ref in entity.get('sourceRefs') or []:
        if MIT in ref.get('url', '') and ref['url'] != MIT + MIT_WORK[entity['id']]:
            ref['url'] = MIT + MIT_WORK[entity['id']]; ref['label'] = work_label(ref['label']); fixed["MIT: a work's contents page"] += 1
    entity['sourceRefs'] = same_refs(entity['sourceRefs'])
# Locators the close reading found too narrow for the place they name (the cited text runs on into the next
# section) or past the end of a book.
LOCATORS = [('collection', ['passages', 219, 'meta'], {'ru': 'II · 52', 'en': 'II · 52'}, {'ru': 'II · 52–53', 'en': 'II · 52–53'}),
            ('collection', ['passages', 220, 'meta'], {'ru': 'III · 21', 'en': 'III · 21'}, {'ru': 'III · 21–22', 'en': 'III · 21–22'}),
            ('collection', ['passages', 249, 'meta'], {'ru': '466d–e', 'en': '466d–e'}, {'ru': '466d–468e', 'en': '466d–468e'}),
            ('collection', ['passages', 230, 'meta'], {'ru': '313c', 'en': '313c'}, {'ru': '310b–314b', 'en': '310b–314b'}),
            ('collection', ['entities', 26, 'workMap', 'stages', 5, 'span'], 'II · 52–90', 'II · 52–89')]
for module, path, before, after in LOCATORS:
    parent = data[module]
    for key in path[:-1]: parent = parent[key]
    assert parent[path[-1]] == before, (path, parent[path[-1]])
    parent[path[-1]] = after
# The collection's posture says what it now holds.
data['collection']['posture'] = ('Authored research navigation: the stand’s own words, and the places of the works they rest on as '
                                 'citations of the original in the public corpus or of places kept in fragments.json; no source text '
                                 'or translation is copied into this module.')
def with_cites_inline(value):
    """The collection's own indentation, with each citation of a place on one line."""
    inline = {}
    def mark(x):
        if isinstance(x, dict):
            if 'w' in x and ('u' in x or 'loc' in x) and set(x) <= {'w', 'c', 'u', 's', 'e', 'loc'}:
                token = f'@@cite{len(inline)}@@'; inline[token] = json.dumps(x, ensure_ascii=False, separators=(',', ':')); return token
            return {k: mark(y) for k, y in x.items()}
        return [mark(y) for y in x] if isinstance(x, list) else x
    return re.sub(r'"(@@cite\d+@@)"', lambda m: inline[m.group(1)], json.dumps(mark(value), ensure_ascii=False, indent=2)) + '\n'
for name, path, dump in [('collection', 'bonsai/antiquity/collection.json', with_cites_inline),
                         ('first-branches', 'bonsai/antiquity/first-branches.json', lambda v: json.dumps(v, ensure_ascii=False, separators=(',', ':')) + '\n')]:
    before = original[name]
    # The IDs of every material stay: same keys and ids in the same places.
    ids = lambda v: [x for x in re.findall(r'"id":\s*"([^"]+)"', json.dumps(v, ensure_ascii=False))]
    assert ids(before) == ids(data[name]), f'{name}: material ids changed'
    # A dry run writes under another root: python3 merge_antiquity.py <root>
    root = sys.argv[1] if len(sys.argv) > 1 else REPO
    os.makedirs(os.path.dirname(f'{root}/{path}'), exist_ok=True)
    open(f'{root}/{path}', 'w').write(dump(data[name]))
print('applied', applied, 'emptied', emptied, 'kept', kept, 'missing outputs', missing, 'links relabelled', relabelled, 'links fixed', dict(fixed))

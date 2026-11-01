"""Mechanical pass of the Zarathustra constructor content: every retelling (R) and navigation (N) sentence
gives way to a citation of the place it retold; own words (I, L, Q, M, C, T) stay as authored.

Writes new.json (the rewritten modules), edits.json (forms whose own words need a reading after the
change: trimmed forms and forms with flags) and report.json."""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, glob, re, sys, collections
sys.path.insert(0, REVIEW + '/frag')
from corpus import version_units
import zc
S = REVIEW
orig = json.load(open(f'{S}/rewrite/original.json'))
new = json.loads(json.dumps(orig))
rows = []
for f in sorted(glob.glob(f'{S}/frag/out/*.jsonl')):
    slice_name = f.split('/')[-1][:-6]
    for line in open(f):
        r = json.loads(line); r['_slice'] = slice_name; rows.append(r)
anchors = {}
for a in json.load(open(f'{S}/align/anchors.dta.json')):
    anchors[(a['module'], a['path'], a['sentence'], a['anchor'])] = a
# Witness module: DTA unit ids per chapter and their raw text, to check spans.
REPO = REPO_ROOT
dta_text, dta_chapter = {}, {}
for f in glob.glob(f'{REPO}/bonsai/zarathustra/witnesses/*.json'):
    d = json.load(open(f))
    for u in d['units']['de']: dta_text[u[0]] = u[2]; dta_chapter[u[0]] = d['chapter']
dta_order, dta_section, chapter_units = {}, {}, collections.defaultdict(list)
for f in sorted(glob.glob(f'{REPO}/bonsai/zarathustra/witnesses/*.json')):
    d = json.load(open(f))
    for i, u in enumerate(d['units']['de']):
        dta_order[u[0]] = (d['chapter'], i); dta_section[u[0]] = u[3]; chapter_units[d['chapter']].append(u[0])
# The public German text's units and the heading units the original keeps only as sections.
_gut, _ = version_units('tos.work.friedrich-nietzsche.also-sprach-zarathustra', 'gutenberg-7205-de')
gut_index = {u['id']: i for i, u in enumerate(_gut)}
gut_to_dta = collections.defaultdict(list)
for f in glob.glob(f'{REPO}/bonsai/zarathustra/witnesses/*.json'):
    d = json.load(open(f))
    for u in d['units']['de']:
        for g in u[4]: gut_to_dta[g].append(u[0])
def letters(raw):
    chars, pos = [], []
    for i, ch in enumerate(raw):
        if ch.isalpha(): chars.append(ch.lower().replace('ſ', 's')); pos.append(i)
    return ''.join(chars), pos
def find_letters(quote, units):
    q, _ = letters(quote)
    if not q: return None
    for u in units:
        t, pos = letters(dta_text[u]); i = t.find(q)
        if i >= 0: return u, pos[i], pos[i + len(q) - 1] + 1
    return None
def heading_place(gid):
    """A section number or chapter title of the public text: the original's section or chapter it heads."""
    i = gut_index.get(gid)
    if i is None: return None
    text = _gut[i]['text'].strip()
    for j in range(i + 1, min(i + 4, len(_gut))):
        targets = gut_to_dta.get(_gut[j]['id'])
        if targets: break
    else: return None
    first = targets[0]; chapter, k = dta_order[first]; units = chapter_units[chapter]
    if re.fullmatch(r'\d+\.', text):
        sec = dta_section[first]; run = [u for u in units if dta_section[u] == sec]
        return chapter, run
    return chapter, units
corpus_units = {}
def corpus_unit(doc, uid):
    if doc not in corpus_units:
        d = zc.resolve(doc); v = zc.pick(d, 'grc'); us, _ = version_units(d, v)
        corpus_units[doc] = {u['id']: u for u in us}
    return corpus_units[doc].get(uid)
CORPUS_WORK = {'ench': 'enchiridion', 'med': 'meditations', 'cratylus': 'cratylus'}
norm = lambda s: re.sub(r'\s+', ' ', s).strip()

def locate(text, quote):
    """Offsets of a whitespace-normalized quote inside an exact text, or None."""
    q = norm(quote)
    if not q: return None
    # map normalized positions back to raw offsets
    chars, pos = [], []
    prev_space = False
    for i, ch in enumerate(text):
        if ch.isspace():
            if chars and not prev_space: chars.append(' '); pos.append(i)
            prev_space = True
        else:
            chars.append(ch); pos.append(i); prev_space = False
    t = ''.join(chars); k = t.find(q)
    if k < 0: return None
    return pos[k], pos[k + len(q) - 1] + 1

def external(a):
    w, loc = a.get('work', ''), a.get('locator', '') or ''
    L = loc
    if 'fröhliche' in w:
        m = re.search(r'\b(276|341)\b', L); return [('die-froehliche-wissenschaft', m.group(1))] if m else None
    if 'Spinoza' in w:
        out = []
        part = None
        for m in re.finditer(r'(Pars|Part)?\s*(I{1,3}|[1-5])\b[ ,]*(Def(?:initio(?:n)?)?\.?|D|Prop(?:osition)?s?\.?|P|Definition)\s*([0-9IVX]+)(?:\s*[-–]\s*(\d+))?((?:[^;]*?)(corollary|scholium|Dem|demonstration))?', L, re.I):
            part = {'1': 'I', '3': 'III'}.get(m.group(2), m.group(2).upper())
            kind = 'D' if m.group(3).lower().startswith('d') else 'P'
            num = m.group(4); num = str({'VII': 7}.get(num, num))
            nums = [int(num)] if not m.group(5) else list(range(int(num), int(m.group(5)) + 1))
            for n in nums:
                extra = (m.group(7) or '').lower()
                suffix = '.C' if extra.startswith('corollary') else '.S' if extra.startswith('scholium') else ''
                out.append(('ethica', f'{part}.{kind}{n}{suffix}'))
        if not out:
            m = re.search(r'(I{1,3})\s*,?\s*Prop(?:s)?\.?\s*(\d+)\s*[-–]\s*(\d+)', L)
        return out or None
    if 'Leibniz' in w:
        nums = [int(x) for x in re.findall(r'\d+', L)]
        if len(nums) == 2 and nums[1] > nums[0] and '–' in L or (len(nums) == 2 and '-' in L): nums = list(range(nums[0], nums[1] + 1))
        return [('monadologie', str(n)) for n in nums if 53 <= n <= 55] or None
    if 'Kierkegaard' in w or 'Gjentagelsen' in w:
        return [('gjentagelsen', 'opening')]
    if 'Heraclitus' in w or 'Eusebius' in w or 'Plutarch' in w:
        m = re.findall(r'B\s?(\d+)', L + ' ' + w)
        if not m and 'Arius' in L + w: m = ['12']
        return [('heraclitus-book', f'B{x}') for x in dict.fromkeys(m)] or None
    if 'Camus' in w:
        keys = []
        low = L.lower()
        for key, pats in [('return', ['ce retour', 'return down', 'cette pause']), ('tragic', ['est tragique', "hero's consciousness", 'conscient']),
                          ('descent', ['la descente']), ('subtle', ['subtil']), ('joy', ['joie silencieuse', 'penultimate']),
                          ('last', ['je laisse', 'last paragraph', 'last two', 'heureux', 'closing paragraph']), ('ascent', ['ascent', 'rolls back', 'héros absurde'])]:
            if any(p in low for p in pats): keys.append(key)
        if 'subtle' in keys and 'joy' in keys and 'joie' not in low: keys.remove('joy')
        if not keys: keys = ['whole']
        return [('le-mythe-de-sisyphe', k) for k in keys]
    if 'Différence' in w:
        low = L.lower(); keys = []
        for key, pats in [('programme', ['point 1', 'programme', 'first of the four', 'supreme object']), ('corrections', ['correction', 'rebukes', 'dwarf']),
                          ('selective', ['selective', 'select']), ('movement', ['in motion', 'movement']), ('insurmountable', ['insurmountable', 'dionysus']),
                          ('generality', ['generality']), ('propositions', ['four propositions'])]:
            if any(p in low for p in pats): keys.append(key)
        return [('difference-et-repetition', 'intro' + ('.' + k if k else '')) for k in (keys or [''])]
    if 'Nietzsche et la philosophie' in w:
        low = L.lower()
        keys = []
        if 'end of §5' in low or '§5' in low: keys.append('II.5')
        if '§6' in low: keys.append('II.6')
        return [('nietzsche-et-la-philosophie', k) for k in (keys or ['II'])]
    return None

def cites_of(r, si, s):
    out, problems = [], []
    for ai, a in enumerate(s.get('anchors') or []):
        if a.get('doc') == 'za':
            c = anchors.get((r['module'], r['path'], si, ai))
            d = (c or {}).get('dta')
            if not d or not d.get('units'):
                # Headings the original keeps as sections: the section or chapter they head.
                places = [heading_place(g) for g in a.get('units') or []]
                if places and all(places):
                    chapter = places[0][0]; run = [u for _, us in places for u in us if dta_order[u][0] == chapter]
                    run.sort(key=lambda u: dta_order[u][1])
                    out.append({'w': 'also-sprach-zarathustra', 'c': chapter, 'u': [run[0], run[-1]]}); report['heading-cites'] += 1; continue
                problems.append(f'unmapped za anchor {a.get("units")}'); continue
            if a.get('quote') and not d.get('span'):
                # The quote stands elsewhere in the chapter than the mapped units: find it there.
                found = find_letters(a['quote'], chapter_units[d['chapter']])
                if found:
                    u, a0, a1 = found; units = sorted(set(d['units']) | {u}, key=lambda x: dta_order[x][1])
                    d = {**d, 'units': units, 'span': [u, a0, a1]}; report['quotes-found-in-chapter'] += 1
            units = d['units']; cite = {'w': 'also-sprach-zarathustra', 'c': d['chapter'], 'u': [units[0], units[-1]]}
            if any(dta_chapter.get(u) != d['chapter'] for u in units): problems.append('anchor crosses chapters')
            span = d.get('span')
            if span and span[0] != units[0] and span[0] in units:
                # The quote marks where the retold place starts: the citation starts there.
                units = units[units.index(span[0]):]; cite['u'] = [units[0], units[-1]]; report['started-at-quote'] += 1
            if span and span[0] == units[0]:
                cite['s'] = span[1]
                if len(units) == 1: cite['e'] = span[2]
            elif a.get('quote'): problems.append(f'quote not found in DTA: {a["quote"][:50]}')
            out.append(cite)
        elif a.get('doc') in CORPUS_WORK:
            units = a.get('units') or []
            cite = {'w': CORPUS_WORK[a['doc']], 'u': [units[0], units[-1]]}
            if a.get('quote'):
                u = corpus_unit(a['doc'], units[0])
                at = locate(u['text'], a['quote']) if u else None
                if at:
                    cite['s'] = at[0]
                    if len(units) == 1: cite['e'] = at[1]
                else: problems.append(f'quote not located in {a["doc"]}')
            out.append(cite)
        elif 'work' in a:
            found = external(a)
            if not found: problems.append(f'external not normalized: {a["work"]} | {a.get("locator")}'); continue
            for w, loc in found: out.append({'w': w, 'loc': loc})
        else: problems.append(f'unknown anchor {a}')
    return out, problems

def walk(path):
    m = re.match(r'^([A-Z_]+)\.?(.*)$', path)
    name, rest = m.group(1), m.group(2)
    obj = new[name]
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

def contains(big, small):
    """Whether a Zarathustra citation covers another of the same chapter entirely."""
    if big.get('w') != 'also-sprach-zarathustra' or small.get('w') != 'also-sprach-zarathustra' or big.get('c') != small.get('c'): return False
    order = lambda u: dta_order[u][1]
    a0, a1, b0, b1 = order(big['u'][0]), order(big['u'][1]), order(small['u'][0]), order(small['u'][1])
    if not (a0 <= b0 and b1 <= a1): return False
    if a0 == b0 and big.get('s', 0) > small.get('s', 0): return False
    if a1 == b1 and 'e' in big and big['e'] < small.get('e', 10**9): return False
    return big != small
def link_cite(anchor):
    """A link end in the public German text, moved to the original's units like the anchors were."""
    units = []
    for g in anchor.get('units') or []:
        for d in gut_to_dta.get(g, []):
            if d not in units: units.append(d)
    if not units:
        places = [heading_place(g) for g in anchor.get('units') or []]
        if places and all(places):
            chapter = places[0][0]; run = [u for _, us in places for u in us if dta_order[u][0] == chapter]
            return [{'w': 'also-sprach-zarathustra', 'c': chapter, 'u': [run[0], run[-1]]}], []
        return [], ['unmapped link end']
    units.sort(key=lambda u: dta_order[u][1]); chapter = dta_order[units[0]][0]
    units = [u for u in units if dta_order[u][0] == chapter]
    cite = {'w': 'also-sprach-zarathustra', 'c': chapter, 'u': [units[0], units[-1]]}
    if anchor.get('quote'):
        found = find_letters(anchor['quote'], units) or find_letters(anchor['quote'], chapter_units[chapter])
        if found and found[0] in units:
            units = units[units.index(found[0]):]; cite['u'] = [units[0], units[-1]]; cite['s'] = found[1]
            if len(units) == 1: cite['e'] = found[2]
    return [cite], []
def dedupe(items):
    seen, out = set(), []
    for c in items:
        k = json.dumps(c, sort_keys=True)
        if k not in seen: seen.add(k); out.append(c)
    return out

report = collections.Counter(); problems = []; edits = []; links = []
for r in rows:
    parent, key = walk(r['path'])
    value = parent[key]
    if norm(value.get('ru', '')) != norm(' '.join(s['ru'] for s in r['sentences'])) or norm(value.get('en', '')) != norm(' '.join(s['en'] for s in r['sentences'])):
        problems.append((r['path'], 'text differs from classification')); report['text-differs'] += 1
    retold, own, navigation = [], [], []
    for si, s in enumerate(r['sentences']):
        cs, ps = cites_of(r, si, s)
        for p in ps: problems.append((r['path'], p))
        if s['class'] == 'R': retold.extend(cs)
        elif s['class'] == 'N': navigation.extend(cs)
        else: own.append(s)
        if s['class'] == 'L' and s.get('link'):
            ends = {}
            for side in ('from', 'to'):
                anchor = s['link'].get(side)
                if not anchor: continue
                # The link's ends as citations, by the same conversion as the sentence's own anchors.
                fake = {'module': r['module'], 'path': r['path']}
                got, _ = cites_of({**fake, 'path': f"{r['path']}#link-{side}"}, si, {'anchors': [anchor]}) if anchor.get('doc') != 'za' else link_cite(anchor)
                ends[side] = got[0] if got else None
            links.append({'path': r['path'], 'ru': s['ru'], 'en': s['en'], 'kind': s['link'].get('kind'), 'from': ends.get('from'), 'to': ends.get('to')})
    # A navigation sentence ("§2:") only said where the retold places are: where a retold place of the
    # same form already stands inside it, its whole section would show the same lines again.
    retold = retold + [c for c in navigation if not any(contains(c, x) for x in retold)]
    # A place inside another place of the same form, short enough to be shown whole, is not shown twice.
    span = lambda c: dta_order[c['u'][1]][1] - dta_order[c['u'][0]][1] + 1 if c.get('w') == 'also-sprach-zarathustra' else 99
    retold = [c for c in retold if not any(contains(x, c) and span(x) <= 3 for x in retold)]
    retold = dedupe(retold)
    is_focus = key == 'focus'
    flags = r.get('flags') or []
    if r['verdict'] == 'replace':
        if not retold:
            report['replace-without-cite'] += 1; problems.append((r['path'], 'replace without any citation: kept'))
            continue
        if is_focus: del parent['focus']; parent['cite'] = dedupe(parent.get('cite', []) + retold)
        else: parent[key] = {'cite': retold}
        report['replaced'] += 1
    elif r['verdict'] == 'trim':
        kept = {'ru': ' '.join(s['ru'] for s in own), 'en': ' '.join(s['en'] for s in own)}
        if is_focus: parent['focus'] = kept; parent['cite'] = dedupe(parent.get('cite', []) + retold)
        else: parent[key] = {**kept, **({'cite': retold} if retold else {})}
        report['trimmed'] += 1
    else:
        report['kept'] += 1
    real_flags = [f for f in flags if not f.startswith('corpus-defect') and not f.startswith('copyrighted-source')]
    if r['verdict'] == 'trim' or real_flags:
        edits.append({'path': r['path'], 'slice': r['_slice'], 'verdict': r['verdict'], 'original': {'ru': value.get('ru'), 'en': value.get('en')},
                      'sentences': [{'class': s['class'], 'ru': s['ru'], 'en': s['en'], 'note': s.get('note', '')} for s in r['sentences']],
                      'kept': (parent.get('focus') if is_focus else parent[key]) if r['verdict'] != 'replace' else None,
                      'cite': retold, 'flags': flags})
json.dump(new, open(f'{S}/rewrite/new.json', 'w'), ensure_ascii=False, indent=1)
json.dump(edits, open(f'{S}/rewrite/edits.json', 'w'), ensure_ascii=False, indent=1)
json.dump(links, open(f'{S}/rewrite/links.json', 'w'), ensure_ascii=False, indent=1)
json.dump({'counts': report, 'problems': problems}, open(f'{S}/rewrite/report.json', 'w'), ensure_ascii=False, indent=1)
print(dict(report), 'edits', len(edits), 'links', len(links), 'problems', len(problems))
pc = collections.Counter(re.sub(r'[:|].*', '', p) for _, p in problems)
print(pc.most_common(12))

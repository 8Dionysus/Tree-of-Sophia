"""Moves the pass's anchors from the public Gutenberg German units to the DTA pivot: units through the
German-German alignment, quotes found again in the exact DTA unit text (letters only, so typography and
the Gutenberg typos do not matter), and gives exact DTA spans."""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, glob, re, sys, collections, difflib
sys.path.insert(0, '.')
import witnesses
W = witnesses.load_all()
dta_raw = {b['id']: b['raw'] for ch in W['de'].values() for b in ch['blocks']}
dta_chapter = {b['id']: k for k, ch in W['de'].items() for b in ch['blocks']}
order = {b['id']: i for i, b in enumerate(b for ch in W['de'].values() for b in ch['blocks'])}
g2d = collections.defaultdict(list)
for ch in json.load(open('alignment.de-deg.json'))['chapters'].values():
    for b in ch['beads']:
        for g in b['t']: g2d[g] += b['s']

def letters(raw):
    """Letters of the text with their offsets in the raw text (hyphenation joins are skipped)."""
    chars, pos = [], []
    for i, ch in enumerate(raw):
        if ch.isalpha(): chars.append(ch.lower().replace('ſ', 's')); pos.append(i)
    return ''.join(chars), pos

def find(quote, units):
    q, _ = letters(quote)
    if not q: return None
    for u in units:
        t, pos = letters(dta_raw[u]); i = t.find(q)
        if i >= 0: return (u, pos[i], pos[i + len(q) - 1] + 1, 'exact')
    # Gutenberg typos (gierig/gieng, de/der): the longest matching block decides
    best = None
    for u in units:
        t, pos = letters(dta_raw[u]); sm = difflib.SequenceMatcher(None, t, q, autojunk=False)
        blocks = [b for b in sm.get_matching_blocks() if b.size]
        if not blocks: continue
        start = blocks[0].a; end = blocks[-1].a + blocks[-1].size
        cover = sum(b.size for b in blocks) / len(q)
        if end > start and (best is None or cover > best[0]): best = (cover, u, pos[start], pos[end - 1] + 1)
    if best and best[0] >= 0.85: return (best[1], best[2], best[3], f'fuzzy:{best[0]:.2f}')
    return None

stats = collections.Counter(); out = []
for f in sorted(glob.glob(REVIEW + '/frag/out/*.jsonl')):
    for line in open(f):
        r = json.loads(line)
        for si, s in enumerate(r['sentences']):
            for ai, a in enumerate(s.get('anchors') or []):
                rec = {'module': r['module'], 'path': r['path'], 'sentence': si, 'anchor': ai, 'class': s['class']}
                if a.get('doc') == 'za' and a.get('lang') == 'de':
                    units = []
                    for g in a['units']:
                        for d in g2d.get(g, []):
                            if d not in units: units.append(d)
                    units.sort(key=order.get)
                    if not units: stats['za-unmapped'] += 1; rec['dta'] = None
                    else:
                        span = find(a['quote'], units) if a.get('quote') else None
                        if a.get('quote') and not span: stats['quote-not-found'] += 1
                        rec['dta'] = {'units': units, 'chapter': dta_chapter[units[0]], 'span': list(span[:3]) if span else None, 'match': span[3] if span else None}
                        stats['za-mapped'] += 1
                else:
                    rec['external'] = a; stats['external' if 'work' in a else f"corpus-{a.get('doc')}"] += 1
                if s.get('link'): rec['link'] = s['link']
                out.append(rec)
json.dump(out, open('anchors.dta.json', 'w'), ensure_ascii=False)
print(dict(stats), len(out))

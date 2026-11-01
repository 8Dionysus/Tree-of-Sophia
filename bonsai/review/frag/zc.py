import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
#!/usr/bin/env python3
"""Corpus helper for the fragment pass.

  zc.py docs                                   list documents and versions
  zc.py chapters DOC LANG                      chapters with ordinal ranges
  zc.py find DOC LANG "words|regex" [--chapter SUBSTR] [--limit N]
  zc.py show DOC LANG FROM [TO]                full units FROM..TO (ordinals)
  zc.py unit UNIT_ID                           one unit by id (any loaded doc)
  zc.py check UNIT_ID "exact quote"            is the quote inside the unit (whitespace-normalized)?

DOC is an alias (za, ench, disc, med, lives, meno, apology, crito, republic, ...) or a full work id.
LANG picks the version by language (de, en, grc, la, es, ru); with several versions of one
language, add :N (en:2) to choose the N-th.
"""
import sys, re, json
sys.path.insert(0, REVIEW + '/frag')
from corpus import manifest, document, version_units

def resolve(doc):
    ids = [d['id'] for d in manifest()['documents']]
    if doc in ids: return doc
    alias = {'za': 'tos.work.friedrich-nietzsche.also-sprach-zarathustra', 'ench': 'tos.work.retained-antiquity.enchiridion',
             'disc': 'tos.work.retained-antiquity.discourses', 'med': 'tos.work.retained-antiquity.meditations',
             'meno': 'tos.work.plato.meno', 'apology': 'tos.work.plato.apology', 'ne': 'tos.work.aristotle.nicomachean-ethics'}
    if doc in alias: return alias[doc]
    m = [i for i in ids if i.endswith('.' + doc) or i.endswith(doc)]
    if len(m) == 1: return m[0]
    sys.exit(f'unknown or ambiguous doc {doc}: {m[:5]}')

def pick(doc_id, lang):
    lang, _, n = lang.partition(':')
    vs = [v for v in document(doc_id)['versions'] if v['language'] == lang]
    if not vs: sys.exit(f'no {lang} version; have ' + ', '.join(f"{v['language']}:{v['id']}" for v in document(doc_id)['versions']))
    return vs[int(n) - 1 if n else 0]['id']

norm = lambda s: re.sub(r'\s+', ' ', s).strip()

def main(a):
    if not a or a[0] in ('-h', '--help'): print(__doc__); return
    cmd = a[0]
    if cmd == 'docs':
        for d in manifest()['documents']:
            doc = document(d['id'])
            print(d['id'], '|', d.get('title'), '|', ', '.join(f"{v['language']}:{v['role']}:{v['id']}" for v in doc['versions']))
        return
    if cmd == 'unit' or cmd == 'check':
        uid = a[1]
        for d in manifest()['documents']:
            for v in document(d['id'])['versions']:
                units, _ = version_units(d['id'], v['id'])
                for u in units:
                    if u['id'] == uid:
                        if cmd == 'unit':
                            print(f"{d['id']} {v['id']} #{u['ordinal']} [{u['chapter']}] ref={u.get('reference')}\n{u['text']}")
                        else:
                            q = norm(a[2]); t = norm(u['text'])
                            print('FOUND' if q in t else 'NOT FOUND', f"({len(q)} chars) in #{u['ordinal']}")
                        return
        print('NO SUCH UNIT'); return
    doc_id = resolve(a[1]); vid = pick(doc_id, a[2]); units, v = version_units(doc_id, vid)
    if cmd == 'chapters':
        cur = None
        for u in units:
            if u['chapter'] != (cur and cur[0]):
                if cur: print(f"{cur[1]}-{prev} {cur[0]}")
                cur = (u['chapter'], u['ordinal'])
            prev = u['ordinal']
        if cur: print(f"{cur[1]}-{prev} {cur[0]}")
        return
    if cmd == 'show':
        lo = int(a[3]); hi = int(a[4]) if len(a) > 4 else lo
        for u in units:
            if lo <= u['ordinal'] <= hi: print(f"#{u['ordinal']} {u['id']} [{u['chapter']}] ref={u.get('reference')}\n{u['text']}\n")
        return
    if cmd == 'find':
        pat = a[3]; chapter = None; limit = 12
        rest = a[4:]
        while rest:
            if rest[0] == '--chapter': chapter = rest[1]; rest = rest[2:]
            elif rest[0] == '--limit': limit = int(rest[1]); rest = rest[2:]
            else: rest = rest[1:]
        try: rx = re.compile(pat, re.I)
        except re.error: rx = re.compile(re.escape(pat), re.I)
        words = [w for w in re.split(r'\s+', pat) if w] if ' ' in pat and not any(c in pat for c in '.*+?[]()|\\') else None
        hits = []
        for u in units:
            if chapter and chapter.lower() not in u['chapter'].lower(): continue
            t = u['text']
            if words:
                score = sum(1 for w in words if w.lower() in t.lower())
                if score: hits.append((score, u))
            elif rx.search(t): hits.append((1, u))
        hits.sort(key=lambda h: (-h[0], h[1]['ordinal']))
        for s, u in hits[:limit]:
            t = norm(u['text'])
            print(f"#{u['ordinal']} {u['id']} [{u['chapter']}] ref={u.get('reference')} score={s}\n  {t[:400]}{'…' if len(t) > 400 else ''}")
        print(f'-- {len(hits)} hits')
        return
    print(__doc__)

if __name__ == '__main__':
    main(sys.argv[1:])

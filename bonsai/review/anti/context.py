"""Edit slices for the Antiquity own words: what each fragment will show (original and English) beside
the kept own words. Usage: python3 context.py <slice-name> [<classification slices>...]"""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, sys, os, re
S = REVIEW
sys.path.insert(0, f'{S}/frag')
from corpus import version_units, document
import zc
identities = json.load(open(REPO_ROOT + '/bonsai/antiquity/identities.json'))
doc_of = {k: v['tree'] for k, v in identities['works'].items()}
alignment_path = f'{S}/anti/alignment.current.json'
edits_path = f'{S}/anti/rewrite/edits.json'
if not os.path.isfile(alignment_path):
    raise SystemExit(
        'Cannot build Antiquity edit context: anti/alignment.current.json is missing. '
        'Regenerate it with `node bonsai/review/anti/alignx/cited.mjs` from the repository root.'
    )
if not os.path.isfile(edits_path):
    raise SystemExit(
        'Cannot build Antiquity edit context: anti/rewrite/edits.json is missing. '
        'Regenerate it with `python3 -B bonsai/review/anti/rewrite_antiquity.py` from the repository root.'
    )
align = json.load(open(alignment_path))
edits = json.load(open(edits_path))
name, wanted = sys.argv[1], set(sys.argv[2:])
cache = {}
def units(doc_id, version_id):
    key = (doc_id, version_id)
    if key not in cache:
        us, _ = version_units(doc_id, version_id); cache[key] = {u['id']: u for u in us}
    return cache[key]
clip = lambda t, n=700: t if len(t) <= n else t[:n] + ' …'
def shown(cite):
    if 'loc' in cite: return {'place': f"{cite['w']} {cite['loc']}"}
    w = align['works'].get(cite['w'])
    doc_id = (w or {}).get('documentId') or doc_of.get(cite['w']) or zc.resolve(cite['w'])
    out = {'place': f"{cite['w']} {cite['u'][0] == cite['u'][1] and 'unit' or 'units'}"}
    for v in document(doc_id)['versions']:
        us = units(doc_id, v['id'])
        if cite['u'][0] in us:
            u = us[cite['u'][0]]; t = u['text']
            out['original'] = clip(t[cite.get('s', 0):cite.get('e', len(t))].strip()); out['reference'] = u.get('reference'); break
    entry = align['units'].get(cite['u'][0], {}).get('t', {})
    # The English the stand shows: the first English translation in the module's order that pairs this unit.
    for vid in [x['versionId'] for x in (w or {}).get('translations', []) if x['language'] == 'en']:
        e = entry.get(vid)
        if not e: continue
        tus = units(doc_id, vid); s0 = cite.get('s', 0); e0 = cite.get('e', 10**9); parts = []
        for src, tgt in e['pairs']:
            if any(a < e0 and b > s0 for a, b in src):
                for i, a, b in tgt: parts.append(tus[e['u'][i]]['text'][a:b])
        out['en'] = clip(' '.join(parts).strip()) if parts else None; break
    return out
items = [dict(e, shows=[shown(c) for c in (e.get('cite') or [])]) for e in edits if not wanted or e['slice'] in wanted]
os.makedirs(f'{S}/anti/edit-slices', exist_ok=True)
json.dump(items, open(f'{S}/anti/edit-slices/{name}.json', 'w'), ensure_ascii=False, indent=1)
print(name, len(items))

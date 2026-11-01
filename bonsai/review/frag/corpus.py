"""Loads public corpus documents as ordered units with their chapter, for matching and anchoring."""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, os
ROOT = REPO_ROOT + '/access/web/public/public-corpus'
_manifest = None
def manifest():
    global _manifest
    if _manifest is None: _manifest = json.load(open(os.path.join(ROOT, 'manifest.json')))
    return _manifest
def document(doc_id):
    d = next(d for d in manifest()['documents'] if d['id'] == doc_id)
    return json.load(open(os.path.join(ROOT, d['manifest']['path'])))
def version_units(doc_id, version_id):
    doc = document(doc_id)
    v = next(v for v in doc['versions'] if v['id'] == version_id)
    units = []
    for p in v['pages']:
        units.extend(json.load(open(os.path.join(ROOT, p['path'])))['units'])
    units.sort(key=lambda u: u['ordinal'])
    heads = {s['unitId']: s for s in v.get('structure', [])}
    chapter = {}; current = []
    for u in units:
        if u['id'] in heads:
            h = heads[u['id']]
            current = [x for x in current if x['level'] < h['level']] + [h]
        u['chapter'] = ' / '.join(x['label'] for x in current)
        u['heading'] = u['id'] in heads
    return units, v

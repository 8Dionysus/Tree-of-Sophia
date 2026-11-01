"""The es/de/zh-Hans catalogs after the fragments pass.

  python3 catalogs.py plan     -> needed.json: every English form the stands now show, with what the
                                  catalogs hold, what can be derived from an earlier translation and what
                                  needs translating (todo.json)
  python3 catalogs.py apply    -> writes derived and translated entries (translated/*.json) into the
                                  catalogs and removes the entries of retellings no source shows any more
"""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, glob, os, re, subprocess, sys, collections
REPO = REPO_ROOT
S = REVIEW
D = f'{S}/catalogs'
LANGS = ['es', 'de', 'zh-Hans']
pub = {l: json.load(open(f'{REPO}/access/web/src/public-research/locales/{l}.json')) for l in LANGS}
bon = {l: json.load(open(f'{REPO}/bonsai/locales/{l}.json')) for l in LANGS}

def forms(value, out):
    if isinstance(value, dict):
        if isinstance(value.get('ru'), str) and isinstance(value.get('en'), str) and value['en'].strip() and re.search(r'[^\W\d_]', value['en']):
            out.add(value['en'])
        for v in value.values(): forms(v, out)
    elif isinstance(value, list):
        for v in value: forms(v, out)
    return out

def content_now():
    """English forms of the constructor content as the stand now has them."""
    js = subprocess.check_output(['node', '--input-type=module', '-e', '''
import {NODE_CONTENT} from './constructor/atlas-content.mjs';
import {RESEARCH_ROUTES} from './constructor/atlas-routes.mjs';
import {RELATION_INQUIRY} from './constructor/relation-inquiry.mjs';
import {INQUIRY_SOURCES_CONCEPTS} from './constructor/inquiry-sources-concepts.mjs';
import {INQUIRY_READINGS_ECHOES} from './constructor/inquiry-readings-echoes.mjs';
import {INQUIRY_FOUNDATION,LENS_INQUIRY} from './constructor/inquiry-foundation.mjs';
import {CLOSE_READING_GUIDES} from './constructor/close-reading-guides.mjs';
import {PRIMARY_SOURCES} from './constructor/source-references.mjs';
process.stdout.write(JSON.stringify({NODE_CONTENT,RESEARCH_ROUTES,RELATION_INQUIRY,INQUIRY_SOURCES_CONCEPTS,INQUIRY_READINGS_ECHOES,INQUIRY_FOUNDATION,LENS_INQUIRY,CLOSE_READING_GUIDES,PRIMARY_SOURCES}));
'''], cwd=f'{REPO}/access/web')
    return json.loads(js)

def bonsai_now():
    out = set()
    for stand in ('antiquity', 'zarathustra'):
        for f in glob.glob(f'{REPO}/bonsai/{stand}/*.json'): forms(json.load(open(f)), out)
    return out

SPLIT = {'es': r'(?<=[.!?…»”])\s+(?=[¿¡«“"(\[—–-]?[A-ZÁÉÍÓÚÑÜ0-9])', 'de': r'(?<=[.!?…»“”])\s+(?=[„«»“"(\[—–-]?[A-ZÄÖÜ0-9])', 'zh-Hans': r'(?<=[。！？])'}
def sentences(text, lang):
    parts = [p for p in re.split(SPLIT[lang], text.strip()) if p.strip()]
    return [p.strip() for p in parts]

def plan():
    os.makedirs(D, exist_ok=True)
    now = content_now()
    content = forms(now, set())
    bon_now = bonsai_now()
    # Old content forms and their classification, to derive the translation of a cut form.
    orig = json.load(open(f'{S}/rewrite/original.json'))
    old_content = forms(orig, set())
    rows = [json.loads(l) for f in sorted(glob.glob(f'{S}/frag/out/*.jsonl')) for l in open(f)]
    rows += [json.loads(l) for f in sorted(glob.glob(f'{S}/anti/out/a*.jsonl')) for l in open(f)]
    derive = {}
    for r in rows:
        en_all = ' '.join(s['en'] for s in r['sentences'])
        keep = [i for i, s in enumerate(r['sentences']) if s['class'] not in 'RN']
        if not keep or len(keep) == len(r['sentences']): continue
        kept_en = ' '.join(r['sentences'][i]['en'] for i in keep)
        derive.setdefault(kept_en, []).append((en_all, keep, len(r['sentences'])))
    needed = {'public': sorted(content), 'bonsai': sorted(bon_now)}
    todo = {l: {} for l in LANGS}; derived = {l: {} for l in LANGS}; stats = collections.Counter()
    for section, strings in needed.items():
        for en in strings:
            for l in LANGS:
                if en in pub[l] or en in bon[l]: stats[f'{l}:held'] += 1; continue
                got = None
                for en_all, keep, n in derive.get(en, []):
                    old = pub[l].get(en_all) or bon[l].get(en_all)
                    if not old: continue
                    parts = sentences(old, l)
                    if len(parts) == n: got = (' ' if l != 'zh-Hans' else '').join(parts[i] for i in keep); break
                if got: derived[l][en] = got; stats[f'{l}:derived'] += 1
                else: todo[l][en] = section; stats[f'{l}:todo'] += 1
    stale = {l: sorted(k for k in pub[l] if k in old_content and k not in content and k not in bon_now) for l in LANGS}
    json.dump({'derived': derived, 'todo': todo, 'stale': stale}, open(f'{D}/plan.json', 'w'), ensure_ascii=False, indent=1)
    print(dict(stats)); print({l: len(stale[l]) for l in LANGS})

def apply():
    p = json.load(open(f'{D}/plan.json'))
    translated = {l: {} for l in LANGS}
    for f in glob.glob(f'{D}/translated/*.json'):
        t = json.load(open(f))
        for l in LANGS: translated[l].update(t.get(l, {}))
    bon_now = bonsai_now()
    for l in LANGS:
        for en, value in {**p['derived'][l], **translated[l]}.items():
            section = p['todo'][l].get(en) or ('bonsai' if en in bon_now else 'public')
            (bon if section == 'bonsai' else pub)[l][en] = value
        for en in p['stale'][l]: pub[l].pop(en, None)
    # Every catalog of one section covers the same messages.
    for cat, name in ((pub, 'access/web/src/public-research/locales'), (bon, 'bonsai/locales')):
        keys = set.intersection(*(set(cat[l]) for l in LANGS))
        for l in LANGS:
            extra = set(cat[l]) - keys
            if extra: print(name, l, 'drops', len(extra), 'messages not translated in every language')
            data = {k: v for k, v in cat[l].items() if k in keys}
            open(f'{REPO}/{name}/{l}.json', 'w').write(json.dumps(data, ensure_ascii=False, indent=2) + '\n')
    print('applied')


def pairs(value, out):
    """English form -> its Russian, for every form."""
    if isinstance(value, dict):
        if isinstance(value.get('ru'), str) and isinstance(value.get('en'), str) and value['en'].strip() and re.search(r'[^\W\d_]', value['en']):
            out.setdefault(value['en'], value['ru'])
        for k, v in value.items():
            if k != 'texts': pairs(v, out)
    elif isinstance(value, list):
        for v in value: pairs(v, out)
    return out

def old_bonsai_forms():
    """Forms the stands' JSON held on main, before the fragments pass."""
    out = set()
    names = subprocess.check_output(['git', '-C', REPO, 'ls-tree', '--name-only', 'origin/main', 'bonsai/antiquity/', 'bonsai/zarathustra/'], text=True).split()
    for name in names:
        if name.endswith('.json'):
            forms(json.loads(subprocess.check_output(['git', '-C', REPO, 'show', f'origin/main:{name}'])), out)
    return out

def plan2():
    """Second round: todo2/<lang>.json for the forms the stands now show without a translation."""
    now = content_now()
    content = forms(now, set())
    ru_of = pairs(now, {})
    bon_now = set()
    for stand in ('antiquity', 'zarathustra'):
        for f in glob.glob(f'{REPO}/bonsai/{stand}/*.json'):
            d = json.load(open(f)); pairs(d, ru_of)
            # A place's texts are a work's words, not forms.
            def walk(v):
                if isinstance(v, dict):
                    if isinstance(v.get('ru'), str) and isinstance(v.get('en'), str) and v['en'].strip() and re.search(r'[^\W\d_]', v['en']): bon_now.add(v['en'])
                    for k, x in v.items():
                        if k != 'texts': walk(x)
                elif isinstance(v, list):
                    for x in v: walk(x)
            walk(d)
    rows = [json.loads(l) for f in sorted(glob.glob(f'{S}/frag/out/*.jsonl')) for l in open(f)]
    rows += [json.loads(l) for f in sorted(glob.glob(f'{S}/anti/out/a*.jsonl')) for l in open(f)]
    derive = {}
    for r in rows:
        en_all = ' '.join(s['en'] for s in r['sentences'])
        keep = [i for i, s in enumerate(r['sentences']) if s['class'] not in 'RN']
        if not keep or len(keep) == len(r['sentences']): continue
        kept_en = ' '.join(r['sentences'][i]['en'] for i in keep)
        derive.setdefault(kept_en, []).append((en_all, keep, len(r['sentences'])))
    todo = {l: [] for l in LANGS}; derived = {l: {} for l in LANGS}; stats = collections.Counter()
    for section, strings in (('public', sorted(content)), ('bonsai', sorted(bon_now - content))):
        for en in strings:
            for l in LANGS:
                if en in pub[l] or en in bon[l]: stats[f'{l}:held'] += 1; continue
                got = None
                for en_all, keep, n in derive.get(en, []):
                    old = pub[l].get(en_all) or bon[l].get(en_all)
                    if not old: continue
                    parts = sentences(old, l)
                    if len(parts) == n: got = (' ' if l != 'zh-Hans' else '').join(parts[i] for i in keep); break
                if got: derived[l][en] = {'section': section, 'value': got}; stats[f'{l}:derived'] += 1
                else: todo[l].append({'section': section, 'en': en, 'ru': ru_of.get(en, '')}); stats[f'{l}:todo'] += 1
    old = forms(json.load(open(f'{S}/rewrite/original.json')), set()) | old_bonsai_forms()
    live = content | bon_now
    stale = {l: {'public': sorted(k for k in pub[l] if k in old and k not in live), 'bonsai': sorted(k for k in bon[l] if k in old and k not in live)} for l in LANGS}
    os.makedirs(f'{D}/todo2', exist_ok=True); os.makedirs(f'{D}/translated2', exist_ok=True)
    for l in LANGS: json.dump(todo[l], open(f'{D}/todo2/{l}.json', 'w'), ensure_ascii=False, indent=1)
    json.dump({'derived': derived, 'stale': stale}, open(f'{D}/plan2.json', 'w'), ensure_ascii=False, indent=1)
    print(dict(stats)); print({l: {k: len(v) for k, v in stale[l].items()} for l in LANGS})

def apply2():
    p = json.load(open(f'{D}/plan2.json'))
    for l in LANGS:
        todo = {x['en']: x['section'] for x in json.load(open(f'{D}/todo2/{l}.json'))}
        t = json.load(open(f'{D}/translated2/{l}.json'))[l]
        missing = [en for en in todo if not (t.get(en) or '').strip()]
        if missing: sys.exit(f'{l}: {len(missing)} untranslated, e.g. {missing[:3]}')
        for en, entry in p['derived'][l].items(): (bon if entry['section'] == 'bonsai' else pub)[l][en] = entry['value']
        for en, section in todo.items(): (bon if section == 'bonsai' else pub)[l][en] = t[en]
        fixes = f'{D}/translated2/{l}-fixes.json'
        if os.path.exists(fixes):
            for en, value in json.load(open(fixes))[l].items():
                held = [cat for cat in (pub, bon) if en in cat[l]]
                if not held: print(l, 'fix for a message no catalog holds:', en[:60]); continue
                for cat in held: cat[l][en] = value
        for section, cat in (('public', pub), ('bonsai', bon)):
            for en in p['stale'][l][section]: cat[l].pop(en, None)
    for cat, name in ((pub, 'access/web/src/public-research/locales'), (bon, 'bonsai/locales')):
        keys = set.intersection(*(set(cat[l]) for l in LANGS))
        for l in LANGS:
            extra = set(cat[l]) - keys
            if extra: print(name, l, 'drops', len(extra), 'messages not translated in every language')
            data = {k: v for k, v in cat[l].items() if k in keys}
            open(f'{REPO}/{name}/{l}.json', 'w').write(json.dumps(data, ensure_ascii=False, indent=2) + '\n')
    print('applied')

if __name__ == '__main__':
    {'plan': plan, 'apply': apply, 'plan2': plan2, 'apply2': apply2}[sys.argv[1]]()

#!/usr/bin/env python3
"""Assembles the philosophical core of the Zarathustra bonsai (bonsai/CORE.md).

    python3 graph-words/assemble.py --work <out/graph-words> --corpus <graph-words/corpus> --tos <checkout>

Reads the grown nodes (concepts/*.json, characters/*.json), the voice map (speakers.json), the cards
given to the independent reviewer (verify-input-*.json) and its verdicts (verify-*.json), and the
aligned rows of the text (rows.jsonl, made from the bonsai's witnesses by prep.py). Writes
bonsai/zarathustra/core/: core.json (what the core holds and who made and judged it), voices.json,
concepts.json, figures.json, links.json, chains.json and occurrences/<concept>.json.

The core holds places, never the text at them: every citation keeps {w, c, u, q?}; copies of the
witnesses' wording, translation excerpts and rows are left behind. A claim keeps the maker's version
and the reviewer's verdict; a revision is admitted in the reviewer's wording with the maker's kept
beside it. A claim without a verdict is held.
"""
import argparse, json, os, pathlib, re

ap = argparse.ArgumentParser()
ap.add_argument('--work', required=True)
ap.add_argument('--corpus', required=True)
ap.add_argument('--tos', required=True)
ap.add_argument('--maker-date', default='2026-10-31')
ap.add_argument('--review-date', default='2026-11-01')
args = ap.parse_args()
W = pathlib.Path(args.work)
OUT = pathlib.Path(args.tos) / 'bonsai/zarathustra/core'
(OUT / 'occurrences').mkdir(parents=True, exist_ok=True)
load = lambda p: json.load(open(p))

# The text's units in order, chapter by chapter, and the rows they belong to.
rows, chapter_units = {}, {}
for line in open(pathlib.Path(args.corpus) / 'rows.jsonl'):
    r = json.loads(line)
    ids = [u['id'] for u in r['de']]
    rows[(r['chapter'], r['row'])] = ids
for (chapter, row) in sorted(rows, key=lambda k: k[1]):
    chapter_units.setdefault(chapter, []).extend(rows[(chapter, row)])
pos = {u: (c, i) for c, ids in chapter_units.items() for i, u in enumerate(ids)}

def cite(c):
    """A place: work, chapter, first and last unit, and the exact words where given."""
    if not isinstance(c, dict) or 'u' not in c:
        return c
    out = {'w': c['w'], 'c': c['c'], 'u': list(c['u'])}
    if c.get('q'):
        out['q'] = c['q']
    return out

TEXT_KEYS = {'de', 'translations', 'sentence', 'sentence_neighbours', 'context', 'voices', 'rows', 'row'}
def clean(value):
    """Every citation reduced to its place; copies of the text and row numbers left behind."""
    if isinstance(value, list):
        return [clean(v) for v in value]
    if isinstance(value, dict):
        if 'w' in value and 'u' in value and 'c' in value:
            return cite(value)
        return {k: clean(v) for k, v in value.items() if k not in TEXT_KEYS}
    return value

# The voice map: rows become units, so that a segment keeps its place when the alignment moves.
def unit_at(chapter, row, last):
    for step in range(0, 40):
        r = row + (-step if last else step)
        ids = rows.get((chapter, r))
        if ids:
            return ids[-1] if last else ids[0]
    return None

speakers = load(W / 'speakers.json')
voices = {}
for chapter, segments in speakers.items():
    out = []
    for s in segments:
        seg = {'u': [unit_at(chapter, s['from'], False), unit_at(chapter, s['to'], True)]}
        for k in ('speaker', 'addressee', 'mode', 'layer', 'via', 'actors'):
            if s.get(k) is not None:
                seg[k] = s[k]
        out.append(seg)
    voices[chapter] = out

def voice_at(c):
    """The primary voice at the first unit of a place, and the embedded one if the place lies inside it."""
    if not c or c.get('c') not in voices:
        return None
    chapter, at = c['c'], pos.get(c['u'][0], (None, None))[1]
    if at is None:
        return None
    found = None
    for seg in voices[chapter]:
        a, b = pos.get(seg['u'][0], (0, -1))[1], pos.get(seg['u'][1], (0, -1))[1]
        if a is not None and b is not None and a <= at <= b:
            if seg.get('layer') == 'embedded' or found is None:
                found = seg
    return found and {k: found[k] for k in ('speaker', 'mode', 'layer', 'via') if k in found}

# Verdicts, keyed by card id; the reviewer's steps are counted from zero.
def verdicts(kind):
    doc = load(W / f'verify-{kind}.json')
    inp = load(W / f'verify-input-{kind}.json')
    lengths = {c['card']: len(c['chain']) for c in inp['cards'] if 'chain' in c}
    chains = {}
    for ch in doc.get('chains', []):
        steps = [s.get('step') for s in ch.get('steps', []) if isinstance(s.get('step'), int)]
        shift = 1 if steps and min(steps) == 1 and max(steps) == lengths.get(ch['card']) else 0
        for s in ch.get('steps', []):
            if isinstance(s.get('step'), int):
                s['step'] -= shift
        chains[ch['card']] = ch
    return ({c['card']: c for c in doc.get('cards', [])}, chains,
            {f['id']: f for f in doc.get('formulas', [])}, inp)

REVIEWER = 'terra-verify'
def assessment(v):
    if not v:
        return {'verdict': 'undecided'}
    out = {'verdict': v['verdict'], 'reason': v.get('reason'), 'attention': v.get('attention', 'normal'), 'reviewer': REVIEWER}
    if v.get('counter_reading'):
        out['counter_reading'] = clean(v['counter_reading'])
    return {k: x for k, x in out.items() if x is not None}

def admit(claim, v, fields=('verb', 'sense', 'grounds')):
    """The stand's admission of a claim from its verdict; a revision replaces the maker's wording, which is kept."""
    a = assessment(v)
    claim['assessment'] = a
    verdict = a['verdict']
    if verdict == 'revise':
        proposed = {}
        for f in fields:
            if v.get(f) is None:
                continue
            new = v[f]
            if f in ('verb', 'sense') and isinstance(new, str):
                new = {'ru': new}  # the reviewer writes Russian; the English follows in a separate pass
            if f == 'grounds':
                new = clean(new)
                if 'cite' in claim and 'grounds' not in claim:  # a chain step keeps one place and the rest beside it
                    proposed['cite'] = claim['cite']
                    claim['cite'] = new[0]
                    if len(new) > 1:
                        claim['grounds'] = new[1:]
                    continue
            proposed[f] = claim.get(f)
            claim[f] = new
        if 'verb' in proposed and 'label' in claim:
            proposed['label'] = claim.pop('label')
        if proposed:
            claim['proposed'] = proposed
    claim['admission'] = {'confirmed': 'map', 'reconstruction': 'map', 'revise': 'map', 'rejected': 'rejected'}.get(verdict, 'held')
    if verdict in ('confirmed', 'revise') and claim.get('status') == 'hypothesis':
        claim['status'] = 'text'
    if verdict == 'reconstruction':
        claim['status'] = 'reconstruction'
    return claim

def formula_of(node, fv):
    """The node's formula with the voice that says it; a better formula from the reviewer takes the centre."""
    essence = node.get('essence', {})
    formula = clean(essence.get('formula')) if essence.get('formula') else None
    if formula:
        formula['voice'] = voice_at(formula.get('cite')) or {'speaker': formula.get('speaker')}
        formula.pop('speaker', None)
    if fv and fv.get('verdict') == 'better' and fv.get('better'):
        proposed = formula
        better = cite(fv['better'])
        formula = {'cite': better, 'why': {'ru': fv.get('why')} if isinstance(fv.get('why'), str) else fv.get('why'),
                   'voice': voice_at(better), 'proposed': proposed}
    if formula is not None and fv:
        formula['assessment'] = {k: x for k, x in {'verdict': fv['verdict'], 'reason': fv.get('why'), 'reviewer': REVIEWER}.items() if x}
    elif formula is not None:
        formula['assessment'] = {'verdict': 'undecided'}
    return formula

links, chains, concepts, figures = [], [], [], []
counts = {'links': {}, 'chains': {}, 'formulas': {}}
def tally(kind, verdict):
    counts[kind][verdict] = counts[kind].get(verdict, 0) + 1

for kind, folder in (('concepts', 'concepts'), ('characters', 'characters')):
    cards, chain_verdicts, formula_verdicts, inp = verdicts(kind)
    by_card = {c['card']: c for c in inp['cards']}
    for path in sorted((W / folder).glob('*.json')):
        node = load(path)
        nid = node['id']
        # Links: the card of the i-th link is <node>#<i>; its ends and verb must match the node file.
        for i, link in enumerate(node.get('links', [])):
            card = f'{nid}#{i}'
            given = by_card.get(card)
            assert given and given['to'] == link['to'] and given['verb'] == link['verb']['ru'], (card, given and given['to'], link['to'])
            claim = clean({k: link[k] for k in ('to', 'verb', 'relation', 'label', 'sense', 'grounds', 'order', 'path', 'status', 'question', 'answer', 'relation_note') if k in link})
            claim = {'id': link.get('id') or f'{nid}:link:{i:02d}:{link["to"]}', 'from': nid, **claim, 'maker': 'sol-run2', 'card': card}
            links.append(admit(claim, cards.get(card)))
            tally('links', claim['assessment']['verdict'])
        for i, chain in enumerate(node.get('chains', [])):
            card = f'{nid}#chain{i}'
            v = chain_verdicts.get(card)
            steps = clean(chain.get('steps', []))
            step_verdicts = {s['step']: s for s in (v or {}).get('steps', []) if isinstance(s.get('step'), int)}
            for j, step in enumerate(steps):
                admit(step, step_verdicts.get(j), fields=('verb', 'sense', 'grounds'))
                step.pop('admission', None)
            claim = {'id': chain.get('id') or f'{nid}:chain:{i}', 'from': nid, 'title': chain.get('title'),
                     **({'states': clean(chain['states'])} if chain.get('states') else {}),
                     'steps': steps, 'status': chain.get('status'), 'maker': 'sol-run2', 'card': card}
            if v:
                claim['assessment'] = assessment(v)
                claim['admission'] = {'confirmed': 'map', 'reconstruction': 'map', 'revise': 'map', 'rejected': 'rejected'}.get(v['verdict'], 'held')
            else:
                claim['assessment'] = {'verdict': 'undecided'}
                claim['admission'] = 'held'
            chains.append(claim)
            tally('chains', claim['assessment']['verdict'])
        fv = formula_verdicts.get(nid)
        tally('formulas', fv['verdict'] if fv else 'undecided')
        essence = clean({k: v for k, v in node.get('essence', {}).items() if k not in ('formula',)})
        essence['formula'] = formula_of(node, fv)
        if kind == 'concepts':
            occ = node.get('occurrences', [])
            groups = [{'id': g['id'], 'name': g['name'], 'count': len(g.get('cite', []))} for g in node.get('occurrence_groups', [])]
            out = []
            for o in occ:
                item = {'cite': cite(o['cite']), 'form': o.get('form'), 'speaker': o.get('speaker'), 'addressee': o.get('addressee'),
                        'mode': (o.get('voices') or [{}])[0].get('mode'), 'use': o.get('use'), 'group': o.get('usage_group'),
                        'neighbours': o.get('neighbours') or []}
                if o.get('line_kind') == 'authored-reading' and o.get('line'):
                    item['line'] = o['line']
                out.append({k: x for k, x in item.items() if x not in (None, [])})
            (OUT / 'occurrences' / f'{nid}.json').write_text(json.dumps({'concept': nid, 'groups': groups, 'occurrences': out}, ensure_ascii=False, separators=(',', ':')) + '\n')
            concepts.append({
                'id': nid, 'title': node['title'], 'wave': node.get('wave'),
                'question': essence.pop('question', None),
                'conception': essence,
                'counter_voices': [dict(clean(cv), voice=voice_at(cv.get('cite'))) for cv in node.get('counter_voices', [])],
                'development': clean(node.get('development')),
                'dimensions': clean(node.get('dimensions', [])),
                'questions': clean(node.get('questions', [])),
                'lexicon': {lang: [{k: f[k] for k in ('form', 'count', 'chapters') if k in f} for f in forms] for lang, forms in node.get('lexicon', {}).items()},
                'uses': groups,
                'occurrences': f'occurrences/{nid}.json',
                'beyond': clean(node.get('beyond', [])),
                'proposed': clean(node.get('new_concepts', [])),
            })
        else:
            figures.append({
                'id': nid, 'title': node['title'], 'existence': 'work-world',
                'names': clean(node.get('names')),
                'appearances': clean(node.get('appearances', [])),
                'voice': clean(node.get('voice', [])),
                'essence': essence,
                'development': clean(node.get('development')),
                'questions': clean(node.get('questions', [])),
                'beyond': clean(node.get('beyond', [])),
                'proposed': clean(node.get('new_characters', [])),
            })

def write(name, value):
    lines = ',\n'.join(' ' + json.dumps(v, ensure_ascii=False, separators=(',', ':')) for v in value) if isinstance(value, list) else None
    text = ('[\n' + lines + '\n]\n') if lines is not None else json.dumps(value, ensure_ascii=False, separators=(',', ':')) + '\n'
    (OUT / name).write_text(text)

write('voices.json', voices)
write('concepts.json', concepts)
write('figures.json', figures)
write('links.json', links)
write('chains.json', chains)
admitted = lambda items: {s: sum(1 for x in items if x.get('admission') == s) for s in ('map', 'held', 'rejected')}
core = {
    'schema': 'tos.bonsai.core.v1',
    'work': 'also-sprach-zarathustra',
    'spec': 'bonsai/CORE.md',
    'posture': {'ru': 'Предложение стенда: читается стендом, экспортируется кандидатами, в ToS не записывается.',
                'en': "The stand's proposal: read by the stand, exported as candidates, never written into ToS."},
    'makers': [{'id': 'sol-run2', 'model': 'gpt-6.1-sol', 'effort': 'max', 'date': args.maker_date,
                'briefs': ['bonsai/review/zarathustra-graph/graph-words/brief-concepts.md', 'bonsai/review/zarathustra-graph/graph-words/brief-characters.md'],
                'made': ['voices', 'concepts', 'figures', 'links', 'chains', 'occurrences']}],
    'reviewers': [{'id': REVIEWER, 'model': 'gpt-6.1-terra', 'effort': 'xhigh', 'date': args.review_date,
                   'method': 'bonsai/review/zarathustra-graph/graph-words/verify-batch.py',
                   'reads': 'the cards and the places of the book, without the maker\'s reasoning'}],
    'files': ['voices.json', 'concepts.json', 'figures.json', 'links.json', 'chains.json', 'occurrences/<concept>.json', 'control.json'],
    'counts': {'voices': sum(len(v) for v in voices.values()), 'chapters': len(voices), 'concepts': len(concepts), 'figures': len(figures),
               'links': len(links), 'chains': len(chains), 'occurrences': sum(1 for p in (OUT / 'occurrences').glob('*.json') for _ in load(p)['occurrences']),
               'verdicts': counts, 'admission': {'links': admitted(links), 'chains': admitted(chains)}},
}
(OUT / 'core.json').write_text(json.dumps(core, ensure_ascii=False, indent=1) + '\n')
print(json.dumps(core['counts'], ensure_ascii=False))

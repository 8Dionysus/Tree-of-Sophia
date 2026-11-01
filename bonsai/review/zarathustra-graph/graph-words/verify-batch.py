#!/usr/bin/env python3
"""Independent verification of grown links, in batches small enough for one reviewer to finish.

    python3 graph-words/verify-batch.py concepts   [--nodes 2] [--parallel 3]
    python3 graph-words/verify-batch.py characters [--nodes 2] [--parallel 3]
    python3 graph-words/verify-batch.py concepts --merge-only

Run from the work folder (~/.local/share/claude-codex). The cards to judge are
out/graph-words/verify-input-<kind>.json; the verdicts already given are in
out/graph-words/verify-<kind>.json. Every node whose cards, chains or formula
still lack a verdict goes into a batch with its neighbours in the list; each
batch is one reviewer run with the rules of briefs/verify-<kind>.md and its own
input and output under out/graph-words/verify/<kind>/. A batch whose output
already decides all its cards is not run again, so the script can be resumed.
At the end all verdicts are merged back into out/graph-words/verify-<kind>.json
with their coverage.
"""
import argparse, concurrent.futures as cf, json, os, pathlib, subprocess, sys

B = pathlib.Path.home() / '.local/share/claude-codex'
OUT = B / 'out/graph-words'

ap = argparse.ArgumentParser()
ap.add_argument('kind', choices=['concepts', 'characters'])
ap.add_argument('--nodes', type=int, default=2)
ap.add_argument('--parallel', type=int, default=3)
ap.add_argument('--model', default='gpt-6.1-terra')
ap.add_argument('--effort', default='xhigh')
ap.add_argument('--merge-only', action='store_true')
args = ap.parse_args()
kind = args.kind

def load(path, empty=None):
    return json.load(open(path)) if path.exists() else (empty if empty is not None else {})

src = load(OUT / f'verify-input-{kind}.json')
merged_path = OUT / f'verify-{kind}.json'
earlier = load(merged_path, {'cards': [], 'chains': [], 'formulas': []})
work = OUT / 'verify' / kind
work.mkdir(parents=True, exist_ok=True)

def verdicts(doc):
    cards = {c['card']: c for c in doc.get('cards', []) if c.get('verdict')}
    chains = {c['card']: c for c in doc.get('chains', []) if c.get('verdict')}
    formulas = {f['id']: f for f in doc.get('formulas', []) if f.get('verdict')}
    return cards, chains, formulas

def batch_outputs():
    return sorted(work.glob('batch-*-output.json'))

def decided():
    cards, chains, formulas = verdicts(earlier)
    for path in batch_outputs():
        c, ch, f = verdicts(load(path))
        cards.update(c); chains.update(ch); formulas.update(f)
    return cards, chains, formulas

def pending():
    cards, chains, formulas = decided()
    nodes = []
    for card in src['cards']:
        done = chains if 'chain' in card else cards
        if card['card'] not in done and card['from'] not in nodes:
            nodes.append(card['from'])
    for formula in src['formulas']:
        if formula['id'] not in formulas and formula['id'] not in nodes:
            nodes.append(formula['id'])
    order = []
    for card in src['cards']:
        if card['from'] not in order:
            order.append(card['from'])
    for formula in src['formulas']:
        if formula['id'] not in order:
            order.append(formula['id'])
    return [n for n in order if n in nodes], cards, chains, formulas

BRIEF = (B / f'briefs/verify-{kind}.md').read_text()
RULES = BRIEF[:BRIEF.index('Запиши out/graph-words/verify-')].replace(f'out/graph-words/verify-input-{kind}.json', '{INPUT}')

def brief_for(name, inp, out):
    return RULES.replace('{INPUT}', str(inp.relative_to(B))) + (
        f'Это одна часть проверки: в {inp.relative_to(B)} только карточки, цепочки и формулы '
        f'нескольких узлов, и каждой нужна своя оценка. Запиши {out.relative_to(B)}: '
        '{"cards":[{"card","verdict","reason","verb"?,"sense"?,"grounds"?,"counter_reading"?,"attention"}], '
        '"chains":[{"card","verdict","reason","attention","steps":[{"step","verdict","reason","verb"?,"sense"?,"grounds"?}]}], '
        '"formulas":[{"id","verdict","better"?,"why"?}]}; card — id карточки из входа, у цепочки шаги считаются с нуля. '
        f'Затем python3 graph-words/checkcite.py {out.relative_to(B)}. '
        'В ответе — путь к файлу и одна строка: сколько каких решений.\n')

def run(index, nodes):
    name = f'batch-{index:02d}'
    inp, out, reply = work / f'{name}-input.json', work / f'{name}-output.json', work / f'{name}-reply.md'
    cards, chains, formulas = decided()
    part = {
        'cards': [c for c in src['cards'] if c['from'] in nodes and c['card'] not in (chains if 'chain' in c else cards)],
        'formulas': [f for f in src['formulas'] if f['id'] in nodes and f['id'] not in formulas],
    }
    inp.write_text(json.dumps(part, ensure_ascii=False, indent=1))
    brief = work / f'{name}-brief.md'
    brief.write_text(brief_for(name, inp, out))
    cmd = ['codex', 'exec', '--skip-git-repo-check', '-C', str(B), '-s', 'workspace-write', '-m', args.model,
           '-c', f'model_reasoning_effort="{args.effort}"', '-o', str(reply), '-']
    with open(brief) as stdin, open(work / f'{name}.log', 'w') as log:
        code = subprocess.run(cmd, stdin=stdin, stdout=log, stderr=subprocess.STDOUT, cwd=B).returncode
    got = load(out)
    c, ch, f = verdicts(got)
    want = len(part['cards']) + len(part['formulas'])
    have = sum(1 for x in part['cards'] if x['card'] in (ch if 'chain' in x else c)) + sum(1 for x in part['formulas'] if x['id'] in f)
    return f'{name} {",".join(nodes)}: exit {code}, decided {have}/{want}'

def merge():
    cards, chains, formulas = decided()
    result = {
        'cards': [cards[c['card']] for c in src['cards'] if 'chain' not in c and c['card'] in cards],
        'chains': [chains[c['card']] for c in src['cards'] if 'chain' in c and c['card'] in chains],
        'formulas': [formulas[f['id']] for f in src['formulas'] if f['id'] in formulas],
    }
    total_cards = sum(1 for c in src['cards'] if 'chain' not in c)
    total_chains = sum(1 for c in src['cards'] if 'chain' in c)
    result['coverage'] = {'cards_reviewed': len(result['cards']), 'cards_total': total_cards,
                          'chains_reviewed': len(result['chains']), 'chains_total': total_chains,
                          'formulas_reviewed': len(result['formulas']), 'formulas_total': len(src['formulas'])}
    result['review_status'] = 'complete' if (len(result['cards']), len(result['chains']), len(result['formulas'])) == (total_cards, total_chains, len(src['formulas'])) else 'partial'
    merged_path.write_text(json.dumps(result, ensure_ascii=False, indent=1))
    from collections import Counter
    print(kind, result['review_status'], json.dumps(result['coverage']))
    print(' cards', dict(Counter(c['verdict'] for c in result['cards'])))
    print(' chains', dict(Counter(c['verdict'] for c in result['chains'])))
    print(' formulas', dict(Counter(f['verdict'] for f in result['formulas'])))

if not args.merge_only:
    for attempt in range(2):
        nodes, *_ = pending()
        if not nodes:
            break
        start = max([int(p.name[6:8]) for p in work.glob('batch-*-input.json')] or [0]) + 1
        groups = [nodes[i:i + args.nodes] for i in range(0, len(nodes), args.nodes)]
        print(f'{kind}: {len(nodes)} nodes pending, {len(groups)} batches (pass {attempt + 1})', flush=True)
        with cf.ThreadPoolExecutor(args.parallel) as pool:
            for line in pool.map(lambda pair: run(*pair), [(start + i, g) for i, g in enumerate(groups)]):
                print(line, flush=True)
merge()

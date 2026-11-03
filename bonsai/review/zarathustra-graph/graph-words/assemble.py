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
import argparse, hashlib, json, os, pathlib, re

ap = argparse.ArgumentParser()
ap.add_argument('--work', required=True)
ap.add_argument('--corpus', required=True)
ap.add_argument('--tos', required=True)
ap.add_argument('--registry', help='the semantic-interchange folder, when --tos is not a Tree checkout')
ap.add_argument('--maker-date', default='2026-10-31')
ap.add_argument('--review-date', default='2026-11-01')
args = ap.parse_args()
W = pathlib.Path(args.work)
OUT = pathlib.Path(args.tos) / 'bonsai/zarathustra/core'
(OUT / 'occurrences').mkdir(parents=True, exist_ok=True)
load = lambda p: json.load(open(p))

# The text's units in order, chapter by chapter, and the rows they belong to.
rows, chapter_units, unit_text = {}, {}, {}
for line in open(pathlib.Path(args.corpus) / 'rows.jsonl'):
    r = json.loads(line)
    ids = [u['id'] for u in r['de']]
    rows[(r['chapter'], r['row'])] = ids
    unit_text.update({u['id']: u['text'] for u in r['de']})
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
figure_scopes = load(W / 'figure-scopes.json') if (W / 'figure-scopes.json').exists() else {'figures': {}}
voices = {}
for chapter, segments in speakers.items():
    out = []
    for s in segments:
        seg = {'u': [unit_at(chapter, s['from'], False), unit_at(chapter, s['to'], True)]}
        a, b = (pos.get(u, (None, None))[1] for u in seg['u'])
        if a is None or b is None or a > b:
            continue  # rows without German text (a table of contents) carry no voice of their own
        for k in ('speaker', 'addressee', 'mode', 'layer', 'via', 'actors'):
            if s.get(k) is not None:
                seg[k] = s[k]
        # Attribution is source-qualified and candidate-only. The occurrence
        # ID is a proposal for the first unit, not evidence of a native record.
        scope = figure_scopes['figures'].get(seg.get('speaker'))
        if scope:
            unit = seg['u'][0]
            seg['attribution'] = {'relation_type': 'tos.relation.work-voice-attributed-to',
                                  'speaker_ref': scope['figure_ref'], 'work_ref': scope['work_ref'],
                                  'anchor_unit': unit, 'role': 'speaker',
                                  'layer': seg.get('layer'), 'mode': seg.get('mode'),
                                  'carrier': seg.get('via'), 'addressee': seg.get('addressee'),
                                  'occurrence_candidate': 'tos.occurrence.bonsai.za.' + unit.rsplit('.', 1)[-1],
                                  'scope': 'first-unit-of-segment', 'status': 'stand-proposal'}
        else:
            seg['attribution'] = {'speaker_designation': seg.get('speaker'),
                                  'work_ref': figure_scopes.get('work_ref'),
                                  'layer': seg.get('layer'), 'mode': seg.get('mode'),
                                  'carrier': seg.get('via'), 'addressee': seg.get('addressee'),
                                  'status': 'speaker-identity-unresolved'}
        # A descriptor identifies this bounded voice segment, not a person.
        # The 208 primary narrations share a work-scoped narrator role; neither
        # this role nor a repeated speaker label establishes a historical agent.
        anchor = json.dumps([chapter, seg['u'], seg.get('speaker'), seg.get('layer'), seg.get('mode'), seg.get('via'), seg.get('addressee')], ensure_ascii=False, separators=(',', ':'))
        descriptor_id = hashlib.sha256(anchor.encode()).hexdigest()[:24]
        is_narrator = seg.get('speaker') == 'narrator' and seg.get('layer') == 'primary' and seg.get('mode') == 'narration'
        seg['voice_descriptor'] = {'id': 'tos.bonsai.voice-segment.' + descriptor_id,
                                   'work_ref': figure_scopes.get('work_ref'),
                                   'cite': {'w': 'also-sprach-zarathustra', 'c': chapter, 'u': seg['u']},
                                   'speaker_designation': seg.get('speaker'),
                                   'layer': seg.get('layer'), 'mode': seg.get('mode'), 'via': seg.get('via'), 'addressee': seg.get('addressee'),
                                   'identity_status': 'narrator-role-only' if is_narrator else 'figure-candidate' if scope else 'unresolved'}
        if is_narrator:
            seg['voice_descriptor']['role'] = 'work-narrator'
            seg['voice_descriptor']['role_ref'] = 'tos.bonsai.voice-role.also-sprach-zarathustra.narrator'
        elif scope:
            seg['voice_descriptor']['figure_ref'] = scope['figure_ref']
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
# English for revised verbs and senses: english.json maps "<claim id>|verb" or "|sense" to the English; what is
# still missing is written to english-input.json in the work folder for the next pass.
english = load(W / 'english.json') if (W / 'english.json').exists() else {}
wanted = {}
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
                new = {'ru': new}  # the reviewer writes Russian; the English comes from english.json, written in a separate pass
                key = f"{claim.get('id') or claim.get('_key')}|{f}"
                if key in english:
                    new['en'] = english[key]
                else:
                    wanted[key] = {'ru': new['ru'], 'maker': (claim.get(f) or {}).get('en') if isinstance(claim.get(f), dict) else None}
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

# Types in the Tree's entity registry: a concept node is the work's conception, an image node an image
# of thought, a figure of the work's world an image of thought (bonsai/CORE.md, open question 1).
IMAGES = {'lion', 'child', 'wheel', 'ring', 'gateway', 'two-paths', 'bite', 'dance', 'circle'}
registry_dir = pathlib.Path(args.registry or pathlib.Path(args.tos) / 'ToS/doctrine/semantic-interchange')
entity_parents = {t['type_id']: t.get('parent_type_ids', []) for t in load(registry_dir / 'entity-types.v1.json')['types']}
registry = {r['relation_type_id']: r for r in load(registry_dir / 'relation-types.v1.json')['relations']}
def lineage(t):
    seen, stack = set(), [t]
    while stack:
        x = stack.pop()
        if x and x not in seen:
            seen.add(x); stack.extend(entity_parents.get(x, []))
    return seen
node_type = {}
for path in sorted((W / 'concepts').glob('*.json')):
    node_type[path.stem] = 'tos.entity.thought-image' if path.stem in IMAGES else 'tos.entity.conception'
for path in sorted((W / 'characters').glob('*.json')):
    node_type[path.stem] = 'tos.entity.work-figure' if path.stem in figure_scopes['figures'] else 'tos.entity.thought-image'
def relation_fits(rel, a, b):
    r = registry.get(rel)
    if not r or r.get('abstract') or a not in node_type or b not in node_type:
        return False
    within = lambda t, allowed: bool(lineage(t) & set(allowed or []))
    return within(node_type[a], r.get('domain_type_ids')) and within(node_type[b], r.get('range_type_ids'))

# A deliberately bounded exact-form inventory; grouping is authored, never
# inferred from voice mode or the matching form. One candidate is one form in
# one TextUnit, possibly more than one token and never a materialized binding.
semantic_input_path = W / 'semantic-uses.json'
semantic_uses = None
semantic_lookup = {}
if semantic_input_path.exists():
    source_uses = load(semantic_input_path)
    if source_uses.get('schema') != 'tos.bonsai.semantic-use-input.v1':
        raise ValueError('unsupported semantic-use input')
    readings = {}
    for reading in source_uses['readings']:
        unit = reading['cite']['u'][0]
        if reading['cite']['u'] != [unit, unit] or pos.get(unit, (None,))[0] != reading['cite']['c']:
            raise ValueError(f"semantic reading has no single source unit: {unit}")
        term, form = reading['term'], reading['form'].casefold()
        group = source_uses['groups'].get(reading['group_id'])
        if not group or group['term'] != term or term not in source_uses['terms']:
            raise ValueError(f"semantic reading has invalid group: {term} {unit}")
        key = (term, unit, form)
        if key in readings:
            raise ValueError(f"semantic reading is ambiguous: {key}")
        readings[key] = reading
    # A reader's reading of one use (the reader's lexical note, with its grounds and the word's history)
    # is authored with the inputs and travels to that use.
    reader_readings = {}
    for entry in source_uses.get('reader_readings', []):
        key = (entry['term'], entry['unit'], entry['form'].casefold())
        if key in reader_readings:
            raise ValueError(f"reader reading is ambiguous: {key}")
        reader_readings[key] = entry['reader_reading']
    attached = set()
    found = set()
    terms = []
    for term, spec in source_uses['terms'].items():
        form_set = {form.casefold() for form in spec['forms']}
        groups = [{**group, 'id': id, 'members': []} for id, group in source_uses['groups'].items() if group['term'] == term]
        by_group = {g['id']: g for g in groups}
        uses = []
        for chapter, units in chapter_units.items():
            for unit in units:
                text = unit_text[unit]
                matched = {}
                for token in re.findall(r"[^\W\d_]+(?:[-‐‑][^\W\d_]+)*", text, re.UNICODE):
                    form = token.casefold()
                    if form in form_set:
                        matched.setdefault(form, []).append(token)
                for form, tokens in matched.items():
                    key = (term, unit, form)
                    reading = readings.get(key)
                    use_digest = hashlib.sha256(json.dumps([term, unit, form], ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()[:16]
                    occurrence_id = 'tos.bonsai.lexical-use.' + term + '.' + use_digest
                    native_id = 'tos.occurrence.bonsai.za.' + unit.rsplit('.', 1)[-1] + '.' + term + '.' + use_digest
                    loc = {'w': source_uses['work'], 'c': chapter, 'u': [unit, unit], 'q': tokens[0]}
                    if reading:
                        if reading['cite']['q'] not in text:
                            raise ValueError(f"semantic reading wording absent: {key}")
                        found.add(key)
                        group = source_uses['groups'][reading['group_id']]
                        semantic = {'group_id': reading['group_id'], 'label': group['label'], 'meaning': group['meaning'],
                                    'grounds': [cite(c) for c in group['grounds']], 'review_status': 'stand-proposal', 'scope': group['scope'],
                                    'independent_review': group['independent_review']}
                        if group.get('proposed'):
                            semantic['proposed'] = group['proposed']
                        if len(group['grounds']) > 1:
                            semantic['grounding_revision_history'] = [{'grounds': [cite(reading['cite'])],
                                'note': 'Earlier unit-only grounding retained; the group reading uses all listed places.'}]
                        by_group[reading['group_id']]['members'].append(occurrence_id)
                    else:
                        semantic = {'group_id': None, 'label': None, 'meaning': None, 'grounds': [], 'review_status': 'unknown',
                                    'scope': {'ru': 'Контекстное значение ещё не разобрано.', 'en': 'The contextual meaning has not yet been assessed.'}}
                    item = {'id': occurrence_id, 'term': term, 'form': tokens[0], 'cite': loc, 'matching_tokens_in_unit': len(tokens),
                            'granularity': 'form-in-text-unit', 'voice': voice_at(loc), 'semantic_use': semantic,
                            'native_candidate': {'record_id': native_id, 'status': 'not-materialized',
                                                 'binding': 'requires-native-text-unit-binding'}}
                    if key in reader_readings:
                        item['reader_reading'] = reader_readings[key]
                        attached.add(key)
                    uses.append(item)
                    semantic_lookup[key] = item
        terms.append({'id': term, 'lemma': spec['lemma'], 'forms': spec['forms'], 'scope': spec['scope'], 'groups': groups, 'occurrences': uses})
    if found != set(readings):
        raise ValueError(f"authored semantic readings not found in witness: {sorted(set(readings)-found)}")
    if attached != set(reader_readings):
        raise ValueError(f"reader readings not found in witness: {sorted(set(reader_readings)-attached)}")
    semantic_uses = {'schema': 'tos.bonsai.semantic-uses.v1', 'work': source_uses['work'], 'witness': source_uses['witness'],
                     'posture': source_uses['posture'], 'terms': terms, 'source_connections': source_uses.get('source_connections', [])}

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
            # A registry relation stands only where its domain and range take both ends; otherwise it stays the maker's proposal.
            rel = claim.get('relation') or ''
            if rel.startswith('tos.relation.') and not relation_fits(rel, nid, link['to']):
                claim['relation_proposed'] = claim.pop('relation')
            links.append(admit(claim, cards.get(card)))
            tally('links', claim['assessment']['verdict'])
        for i, chain in enumerate(node.get('chains', [])):
            card = f'{nid}#chain{i}'
            v = chain_verdicts.get(card)
            steps = clean(chain.get('steps', []))
            step_verdicts = {s['step']: s for s in (v or {}).get('steps', []) if isinstance(s.get('step'), int)}
            for j, step in enumerate(steps):
                step['_key'] = f"{chain.get('id') or f'{nid}:chain:{i}'}#{j}"
                admit(step, step_verdicts.get(j), fields=('verb', 'sense', 'grounds'))
                step.pop('admission', None)
                step.pop('_key', None)
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
                # The old fields stay intact; voice_group makes their historical
                # role explicit without treating speech mode as lexical sense.
                semantic_matches = [use for (term, unit, form), use in semantic_lookup.items()
                                    if unit == o['cite']['u'][0] and form == (o.get('form') or '').casefold()]
                if semantic_matches:
                    item['voice_group'] = {k: item[k] for k in ('mode', 'use', 'group') if k in item and item[k] is not None}
                    item['semantic_use'] = semantic_matches[0]['semantic_use']
                    item['lexical_use_ref'] = semantic_matches[0]['id']
                out.append({k: x for k, x in item.items() if x not in (None, [])})
            (OUT / 'occurrences' / f'{nid}.json').write_text(json.dumps({'concept': nid, 'groups': groups, 'occurrences': out}, ensure_ascii=False, separators=(',', ':')) + '\n')
            concepts.append({
                'id': nid, 'type': node_type[nid], 'title': node['title'], 'wave': node.get('wave'),
                'question': essence.pop('question', None),
                'conception': essence,
                'counter_voices': [dict(clean(cv), voice=voice_at(cv.get('cite'))) for cv in node.get('counter_voices', [])],
                'development': clean(node.get('development')),
                'dimensions': clean(node.get('dimensions', [])),
                'questions': clean(node.get('questions', [])),
                'lexicon': {lang: [{k: f[k] for k in ('form', 'count', 'chapters') if k in f} for f in forms]
                            for lang, forms in node.get('lexicon', {}).items() if lang in ('de', 'ru', 'en') and isinstance(forms, list)},
                'uses': groups,
                'occurrences': f'occurrences/{nid}.json',
                'beyond': clean(node.get('beyond', [])),
                'proposed': clean(node.get('new_concepts', [])),
            })
        else:
            figures.append({
                'id': nid, 'type': node_type[nid], 'title': node['title'], 'existence': 'work-world',
                **({'work_scope': figure_scopes['figures'][nid], 'previous_type': 'tos.entity.thought-image'} if nid in figure_scopes['figures'] else {}),
                'names': {lang: clean(forms) for lang, forms in (node.get('names') or {}).items()
                          if lang in ('de', 'ru', 'en') and isinstance(forms, list)},
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

# Independent source review may revise a sense. Preserve the previous sense and
# assessment verbatim; the maker's own proposed fields remain independently kept.
sense_revisions_path = W / 'link-sense-revisions.json'
if sense_revisions_path.exists():
    revision_doc = load(sense_revisions_path)
    revisions = revision_doc['revisions']
    by_id = {link['id']: link for link in links}
    if not set(revisions) <= set(by_id):
        raise ValueError('sense revision names an unknown link')
    for link_id, revision in revisions.items():
        link = by_id[link_id]
        old_sense = link['sense']
        raw_sense = json.dumps(old_sense, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
        if revision['prior_sense_sha256'] != hashlib.sha256(raw_sense.encode()).hexdigest():
            raise ValueError(f"prior sense changed; review revision: {link_id}")
        old_assessment = link['assessment']
        previous_verdict = old_assessment['verdict']
        if previous_verdict != 'revise':
            counts['links'][previous_verdict] -= 1
            counts['links']['revise'] = counts['links'].get('revise', 0) + 1
        prior = {'sense': old_sense, 'assessment': old_assessment}
        if 'answer' in revision:
            old_answer = link.get('answer')
            raw_answer = json.dumps(old_answer, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
            if revision.get('prior_answer_sha256') != hashlib.sha256(raw_answer.encode()).hexdigest():
                raise ValueError(f"prior answer changed; review revision: {link_id}")
            prior['answer'] = old_answer
            link['answer'] = {**old_answer, **revision['answer']}
        link['sense_revision_history'] = [prior]
        link['sense'] = revision['sense']
        link['sense_revision'] = {'reviewer': revision_doc['reviewer'], 'reason': revision['reason']}
        if 'independent_review' in revision:
            link['sense_revision']['independent_review'] = clean(revision['independent_review'])
        link['assessment'] = {**old_assessment, 'verdict': 'revise', 'reason': revision['reason']['ru'],
                              'reviewer': revision_doc['reviewer']['id'],
                              'review_scope': revision_doc['reviewer']['scope']}
        if 'method_transfer' in revision:
            link['method_transfer'] = clean(revision['method_transfer'])
        link['admission'] = 'map'
# A source-owned family assignment never rewrites the source predicate or verb.
# The explicit unmapped family preserves the limits of current registry matching.
family_path = W / 'link-families.json'
if family_path.exists():
    family_doc = load(family_path)
    families = family_doc['links']
    if set(families) != {link['id'] for link in links}:
        raise ValueError('link family assignment must cover the exact set of link IDs')
    for link in links:
        family = families[link['id']]
        if family['relation_type'] not in registry:
            raise ValueError(f"unknown link family: {link['id']}")
        if family.get('source_relation') != (link.get('relation') or None):
            raise ValueError(f"source predicate changed; review link family: {link['id']}")
        if family.get('family') not in family_doc['families']:
            raise ValueError(f"missing subject-model family: {link['id']}")
        definition = family_doc['families'][family['family']]
        raw_sense = json.dumps(link['sense'], ensure_ascii=False, sort_keys=True, separators=(',', ':'))
        if family.get('source_sense_sha256') != hashlib.sha256(raw_sense.encode()).hexdigest():
            raise ValueError(f"source sense changed; review link family: {link['id']}")
        reasons = family.get('family_reason', {})
        if any(not isinstance(reasons.get(lang), str) or '\n' in reasons[lang] or not reasons[lang].strip() for lang in ('ru', 'en')):
            raise ValueError(f"missing one-line family rationale: {link['id']}")
        link['relation_family'] = {**family, 'family_label': {lang: definition[lang] for lang in ('ru', 'en')}}
# Stable statement identities are authored separately from the preserved research
# inputs. Collection fingerprints bind identities to members, never to their order.
identity_path = W / 'statement-identities.json'
if identity_path.exists():
    identities = load(identity_path)
    def fingerprint(value):
        raw = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
        return hashlib.sha256(raw.encode()).hexdigest()
    def at_path(value, path):
        for part in path.split('.'):
            value = value[part]
        return value
    for record in concepts + figures + links:
        entry = identities['records'].get(record['id'])
        if not entry:
            continue
        record['statement_ids'] = entry['fields']
        for path, members in entry['collections'].items():
            found = set()
            for member in at_path(record, path):
                digest = fingerprint(member)
                if digest not in members:
                    continue
                identity = members[digest]
                if digest in found:
                    raise ValueError(f"ambiguous statement member: {record['id']} {path}")
                found.add(digest)
                if not member.get('id'):
                    member['statement_anchor'] = identity['anchor']
                member['statement_ids'] = identity['statement_ids']
            if found != set(members):
                raise ValueError(f"statement identities need source review: {record['id']} {path}")
    for chain in chains:
        members = identities['chain_steps'][chain['id']]
        found = set()
        for step in chain['steps']:
            digest = fingerprint(step)
            if digest not in members or digest in found:
                raise ValueError(f"chain step identities need source review: {chain['id']}")
            found.add(digest)
            step.update(members[digest])
        if found != set(members):
            raise ValueError(f"chain step identities need source review: {chain['id']}")
# Apply source-reviewed corrections only after assigning the original step IDs.
step_revisions_path = W / 'step-revisions.json'
if step_revisions_path.exists():
    doc = load(step_revisions_path)
    pending = dict(doc['revisions'])
    for chain in chains:
        for step in chain['steps']:
            revision = pending.pop(step['id'], None)
            if not revision:
                continue
            step['step_revision_history'] = [{k: clean(v) for k, v in step.items() if k not in ('id', 'aliases')}]
            for key in ('verb', 'sense', 'cite', 'status'):
                if key in revision:
                    step[key] = clean(revision[key])
            step['grounds'] = step.get('grounds', []) + clean(revision.get('grounds', []))
            step['assessment'] = {**step.get('assessment', {}), 'verdict': 'revise',
                                  'reason': revision['sense']['ru'], 'reviewer': doc['reviewer']}
            if 'independent_review' in revision:
                step['assessment']['independent_review'] = clean(revision['independent_review'])
    if pending:
        raise ValueError(f"step revisions name unknown IDs: {sorted(pending)}")
route_path = W / 'route-growth.json'
route_growth = load(route_path) if route_path.exists() else {'nodes': [], 'chains': []}
for chain in route_growth['chains']:
    chains.append(clean(chain))
    verdict = chain['assessment']['verdict']
    counts['chains'][verdict] = counts['chains'].get(verdict, 0) + 1
write('voices.json', voices)
write('concepts.json', concepts)
write('figures.json', figures)
write('links.json', links)
write('chains.json', chains)
if semantic_uses:
    write('semantic-uses.json', semantic_uses)
# Exact-place comparisons and voice attribution remain authored stand proposals.
place_relations_path = W / 'place-relations.json'
if place_relations_path.exists():
    write('place-relations.json', clean(load(place_relations_path)))
# Objects the grown nodes propose and the links already name: kept with the node that proposes them.
proposed = {}
for node in concepts + figures:
    for item in node.get('proposed') or []:
        if isinstance(item, dict) and item.get('id') and item['id'] not in node_type:
            proposed.setdefault(item['id'], {'id': item['id'], 'title': item.get('title'), 'proposed_by': []})['proposed_by'].append(node['id'])
named = {l['to'] for l in links} | {p.get('id') for l in links for p in (l.get('path') or []) if isinstance(p, dict) and p.get('id')}
for x in sorted(named - set(node_type) - set(proposed)):
    proposed[x] = {'id': x, 'title': None, 'proposed_by': sorted({l['from'] for l in links if l['to'] == x or any(isinstance(p, dict) and p.get('id') == x for p in (l.get('path') or []))})}
# Source-visible stand readings add evidence without changing the original proposals,
# their IDs, or the independent-review/canon boundary.
readings_path = W / 'proposal-readings.json'
if readings_path.exists():
    readings = load(readings_path)
    for reading in readings['readings']:
        if reading['id'] not in proposed:
            raise ValueError(f"reading has no proposed object: {reading['id']}")
        if reading.get('corrected_title'):
            proposed[reading['id']]['title_revision_history'] = [proposed[reading['id']]['title']]
            proposed[reading['id']]['title'] = clean(reading['corrected_title'])
        proposed[reading['id']]['reading'] = {k: clean(v) for k, v in reading.items() if k != 'id'}
for node in route_growth['nodes']:
    if node['id'] in proposed or node['id'] in node_type:
        raise ValueError(f"route node already exists: {node['id']}")
    proposed[node['id']] = clean(node)
write('proposed.json', sorted(proposed.values(), key=lambda x: x['id']))
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
    'files': ['voices.json', 'concepts.json', 'figures.json', 'links.json', 'chains.json', 'proposed.json', 'occurrences/<concept>.json', 'control.json'],
    'counts': {'voices': sum(len(v) for v in voices.values()), 'chapters': len(voices), 'concepts': len(concepts), 'figures': len(figures),
               'links': len(links), 'chains': len(chains), 'proposed': len(proposed), 'occurrences': sum(1 for p in (OUT / 'occurrences').glob('*.json') for _ in load(p)['occurrences']),
               'verdicts': counts, 'admission': {'links': admitted(links), 'chains': admitted(chains)}},
}
if semantic_uses:
    core['files'].append('semantic-uses.json')
    core['counts']['semantic_use_terms'] = len(semantic_uses['terms'])
    core['counts']['semantic_uses_assessed'] = sum(1 for term in semantic_uses['terms'] for use in term['occurrences'] if use['semantic_use']['review_status'] == 'stand-proposal')
    core['counts']['semantic_uses_unknown'] = sum(1 for term in semantic_uses['terms'] for use in term['occurrences'] if use['semantic_use']['review_status'] == 'unknown')
if place_relations_path.exists():
    core['files'].append('place-relations.json')
    core['counts']['place_relations'] = len(load(place_relations_path)['relations'])

# Language reviews are their own events: they correct whole reader-text fields in the assembled core, while the
# makers' and reviewers' wordings stay in their inputs. The manifest keeps review order explicit; the original
# language review stays first because later ledgers extend its reader-language decisions.
corrections_manifest_path = W / 'corrections.json'
if corrections_manifest_path.exists():
    corrections_manifest = load(corrections_manifest_path)
    if corrections_manifest.get('schema') != 'tos.bonsai.correction-ledgers.v1':
        raise ValueError(f'unsupported correction-ledger manifest: {corrections_manifest_path}')
    correction_names = corrections_manifest.get('ledgers')
    if not isinstance(correction_names, list) or not correction_names or correction_names[0] != 'language-corrections-2026-11-02.json':
        raise ValueError('corrections.json must list language-corrections-2026-11-02.json first')
else:
    # Keep older isolated replay work copies usable; checked-in assembly inputs always carry the manifest.
    correction_names = ['language-corrections-2026-11-02.json']

correction_reviews = []
for correction_name in correction_names:
    if not isinstance(correction_name, str) or pathlib.Path(correction_name).name != correction_name:
        raise ValueError(f'correction ledger must be a file name: {correction_name!r}')
    corrections_path = W / correction_name
    if not corrections_path.exists():
        raise FileNotFoundError(f'correction ledger listed in corrections.json is missing: {corrections_path}')
    review = load(corrections_path)
    if not isinstance(review.get('corrections'), list):
        raise ValueError(f'correction ledger has no corrections array: {corrections_path}')
    ru_map, paired_en_map, en_only_map = {}, {}, {}
    for correction in review['corrections']:
        for lang in ('ru', 'en'):
            pair = correction.get(lang)
            if pair is not None and (not isinstance(pair, list) or len(pair) != 2 or not all(isinstance(item, str) and item for item in pair)):
                raise ValueError(f'{lang} correction must be [old, new] strings: {correction!r}')
        if correction.get('ru'):
            old_ru, new_ru = correction['ru']
            if old_ru in ru_map and ru_map[old_ru] != new_ru:
                raise ValueError(f'conflicting Russian corrections for {old_ru!r} in {corrections_path.name}')
            ru_map[old_ru] = new_ru
            if correction.get('en'):
                old_en, new_en = correction['en']
                if old_ru in paired_en_map and paired_en_map[old_ru] != (old_en, new_en):
                    raise ValueError(f'conflicting paired English corrections for {old_ru!r} in {corrections_path.name}')
                paired_en_map[old_ru] = (old_en, new_en)
        elif correction.get('en'):
            old_en, new_en = correction['en']
            if old_en in en_only_map and en_only_map[old_en] != new_en:
                raise ValueError(f'conflicting English-only corrections for {old_en!r} in {corrections_path.name}')
            en_only_map[old_en] = new_en
    correction_reviews.append({'name': correction_name, 'review': review, 'ru_map': ru_map,
                               'paired_en_map': paired_en_map, 'en_only_map': en_only_map,
                               'used_ru': set(), 'used_paired_en': set(), 'used_en': set()})

if correction_reviews:
    dropped = [0]
    # A proposed node's generated filler (its name 'occurs in the concept's verbal neighbourhood and routes') says
    # nothing of its own; the review leaves the node without it.
    filler = re.compile(r'возникает в словесном окружении и маршрутах понятия')
    def correct(value):
        if isinstance(value, list):
            for item in value:
                correct(item)
        elif isinstance(value, dict):
            why = value.get('why')
            if isinstance(why, dict) and isinstance(why.get('ru'), str) and filler.search(why['ru']):
                del value['why']
                dropped[0] += 1
            # Apply ledgers in manifest order. Paired entries preserve the old review's RU/EN linkage;
            # an en-only entry is intentionally global to every exact matching English reader text.
            for correction_review in correction_reviews:
                ru_map = correction_review['ru_map']
                paired_en_map = correction_review['paired_en_map']
                en_only_map = correction_review['en_only_map']
                old_ru, old_en = value.get('ru'), value.get('en')
                ru_matches = isinstance(old_ru, str) and old_ru in ru_map
                if ru_matches:
                    value['ru'] = ru_map[old_ru]
                    correction_review['used_ru'].add(old_ru)
                # Older source reviews use a plain Russian string for their
                # reason. Keep that shape and the original review input while
                # allowing the same exact-text reader correction as for forms.
                old_reason = value.get('reason')
                if correction_review['review'].get('scalar_reasons') is True and isinstance(old_reason, str) and old_reason in ru_map:
                    value['reason'] = ru_map[old_reason]
                    correction_review['used_ru'].add(old_reason)
                paired_applied = False
                paired = paired_en_map.get(old_ru) if ru_matches else None
                if paired and isinstance(old_en, str) and old_en == paired[0]:
                    value['en'] = paired[1]
                    correction_review['used_paired_en'].add(old_ru)
                    paired_applied = True
                if not paired_applied and isinstance(old_en, str) and old_en in en_only_map:
                    value['en'] = en_only_map[old_en]
                    correction_review['used_en'].add(old_en)
            for key, item in value.items():
                if key not in ('ru', 'en'):
                    correct(item)
    names = [name for name in ('voices.json', 'concepts.json', 'figures.json', 'links.json', 'chains.json', 'proposed.json',
                               'semantic-uses.json', 'place-relations.json') if (OUT / name).exists()]
    names += [f'occurrences/{path.name}' for path in sorted((OUT / 'occurrences').glob('*.json'))]
    for name in names:
        data = load(OUT / name)
        correct(data)
        write(name, data)
    language_counts = {'ru': {'admitted': 0, 'applied': 0, 'unmatched': 0},
                       'en': {'admitted': 0, 'applied': 0, 'unmatched': 0}}
    ledger_counts = []
    for index, correction_review in enumerate(correction_reviews):
        review = correction_review['review']
        ru_map, paired_en_map, en_only_map = (correction_review[k] for k in ('ru_map', 'paired_en_map', 'en_only_map'))
        used_ru = correction_review['used_ru']
        admitted_ru, applied_ru = len(ru_map), len(used_ru)
        admitted_en = len(paired_en_map) + len(en_only_map)
        applied_en = len(correction_review['used_paired_en']) + len(correction_review['used_en'])
        ledger_counts.append({'ledger': correction_review['name'],
                              'ru': {'admitted': admitted_ru, 'applied': applied_ru, 'unmatched': admitted_ru - applied_ru},
                              'en': {'admitted': admitted_en, 'applied': applied_en, 'unmatched': admitted_en - applied_en}})
        language_counts['ru']['admitted'] += admitted_ru
        language_counts['ru']['applied'] += applied_ru
        language_counts['en']['admitted'] += admitted_en
        language_counts['en']['applied'] += applied_en
        stem = pathlib.Path(correction_review['name']).stem
        reviewer = {'id': 'language-review-' + stem.removeprefix('language-corrections-'),
                    'date': review.get('date'), 'reviewer': review.get('reviewer'), 'reads': review.get('basis'),
                    'corrections': f'bonsai/review/zarathustra-graph/graph-words/assembly-inputs/{correction_review["name"]}'}
        # Retain the original review's established identity and authorship fields.
        if index == 0 and correction_review['name'] == 'language-corrections-2026-11-02.json':
            reviewer.update({'id': 'language-review-20261102', 'model': 'gpt-6.1-terra', 'effort': 'xhigh', 'admitted_by': 'claude'})
        core['reviewers'].append({k: v for k, v in reviewer.items() if v is not None})
    for lang in ('ru', 'en'):
        language_counts[lang]['unmatched'] = language_counts[lang]['admitted'] - language_counts[lang]['applied']
    core['counts']['language_corrections'] = {
        # Keep the original Russian aggregate keys for existing readers of core.json.
        'admitted': language_counts['ru']['admitted'],
        'applied': language_counts['ru']['applied'],
        'unmatched': language_counts['ru']['unmatched'],
        'by_language': language_counts,
        'ledgers': ledger_counts,
        'fillers_dropped': dropped[0]
    }
(W / 'english-input.json').write_text(json.dumps(wanted, ensure_ascii=False, indent=1) + '\n')
core['counts']['english_missing'] = len(wanted)
(OUT / 'core.json').write_text(json.dumps(core, ensure_ascii=False, indent=1) + '\n')
print(json.dumps(core['counts'], ensure_ascii=False))

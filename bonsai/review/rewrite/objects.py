"""Types the Zarathustra atlas's authored forms in the Tree's object types: bonsai/zarathustra/objects.json.

Only what the stand's own structure says is typed or related:
  a material by its kind (a tension-question is a problem, a reading a conception, a symbol or character an
  image of thought, a source a textual fragment, a note a method of thought, ...);
  the perspectives of a concept or a conception are aspects of it, those of a problem the positions that address it;
  a problem's formulated question articulates it; a counter-reading objects to the argument of its inquiry;
  a reading that interprets a concept is a conception of it; an image that supports or develops a reading presents it.
Edges that compare, question or develop say no more than that and stay unrelated; links between places of
texts are kept as the stand's own proposal.
"""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, collections
D = REVIEW
REPO = REPO_ROOT
f = json.load(open(f'{D}/rewrite/final.json'))
nodes = json.load(open(f'{D}/frag/context/nodes.json'))
edges = json.load(open(f'{D}/frag/context/edges.json'))
links = json.load(open(f'{D}/rewrite/links.json'))
NODE_TYPE = {'concept': 'tos.entity.crosscutting-concept', 'interpretation': 'tos.entity.conception', 'question': 'tos.entity.problem',
             'symbol': 'tos.entity.thought-image', 'character': 'tos.entity.thought-image', 'figure': 'tos.entity.figure',
             'tradition': 'tos.entity.intellectual-tradition', 'fragment': 'tos.entity.textual-fragment', 'excerpt': 'tos.entity.textual-fragment',
             'note': 'tos.entity.thought-method'}
PERSPECTIVE = {'concept': ('tos.entity.aspect', 'tos.relation.aspect-of-concept', 'out'),
               'interpretation': ('tos.entity.aspect', 'tos.relation.conception-has-aspect', 'in'),
               'question': ('tos.entity.position', 'tos.relation.position-addresses-problem', 'out')}
def cites(value):
    return list(value.get('cite', [])) if isinstance(value, dict) else []
def ground_cites(grounds):
    return [c for g in grounds or [] for c in g.get('cite', [])]
objects, relations, by_id = [], [], {}
def add(id_, type_, form, material=None, grounds=()):
    assert id_ not in by_id, id_
    o = {'id': id_, 'type': type_, 'form': form}
    if material: o['material'] = material
    unique = []
    for c in grounds:
        if c not in unique: unique.append(c)
    if unique: o['grounds'] = unique
    objects.append(o); by_id[id_] = o
    return id_
def relate(type_, a, b, basis):
    relations.append({'type': type_, 'from': a, 'to': b, 'basis': basis})

node_object = {}
for nid, node in nodes.items():
    kind, content = node['kind'], f['NODE_CONTENT'].get(nid, {})
    t = NODE_TYPE.get(kind)
    if not t: continue
    oid = node_object[nid] = add(f'za:{nid}', t, f'NODE_CONTENT.{nid}.body', nid, cites(content.get('body')))
    for i, p in enumerate(content.get('perspectives', [])):
        ptype, rel, direction = PERSPECTIVE.get(kind, ('tos.entity.aspect', None, None))
        pid = add(f'za:{nid}/perspective/{i}', ptype, f'NODE_CONTENT.{nid}.perspectives[{i}]', nid, cites(p.get('body')))
        if rel: relate(rel, *((pid, oid) if direction == 'out' else (oid, pid)), f'NODE_CONTENT.{nid}.perspectives[{i}]')
    if content.get('question'):
        qid = add(f'za:{nid}/question', 'tos.entity.question', f'NODE_CONTENT.{nid}.question', nid)
        if kind == 'question': relate('tos.relation.problem-has-question', oid, qid, f'NODE_CONTENT.{nid}.question')
for module in ('INQUIRY_SOURCES_CONCEPTS', 'INQUIRY_READINGS_ECHOES'):
    for mid, inquiry in f[module].items():
        argument = None
        if inquiry.get('argument'):
            argument = add(f'za:{mid}/argument', 'tos.entity.argument', f'{module}.{mid}.argument', mid, cites(inquiry['argument']) + ground_cites(inquiry.get('grounds')))
        if inquiry.get('counterReading'):
            objection = add(f'za:{mid}/objection', 'tos.entity.objection', f'{module}.{mid}.counterReading', mid, ground_cites(inquiry['counterReading'].get('grounds')))
            if argument: relate('tos.relation.objection-to-argument', objection, argument, f'{module}.{mid}.counterReading')
        if inquiry.get('experiment'): add(f'za:{mid}/experiment', 'tos.entity.thought-experiment', f'{module}.{mid}.experiment', mid)
        if inquiry.get('question'):
            qid = add(f'za:{mid}/inquiry-question', 'tos.entity.question', f'{module}.{mid}.question', mid)
            if nodes.get(mid, {}).get('kind') == 'question': relate('tos.relation.problem-has-question', node_object[mid], qid, f'{module}.{mid}.question')
for eid, inquiry in f['RELATION_INQUIRY'].items():
    if inquiry.get('question'): add(f'za:{eid}/question', 'tos.entity.question', f'RELATION_INQUIRY.{eid}.question', eid)
for i, route in enumerate(f['RESEARCH_ROUTES']):
    if route.get('question'): add(f'za:route:{route["id"]}/question', 'tos.entity.question', f'RESEARCH_ROUTES[{i}].question', f'route:{route["id"]}')
foundation = f['INQUIRY_FOUNDATION']
add('za:foundation/question', 'tos.entity.question', 'INQUIRY_FOUNDATION.question')
for i, _ in enumerate(foundation.get('distinctions', [])):
    add(f'za:foundation/distinction/{i}', 'tos.entity.distinction', f'INQUIRY_FOUNDATION.distinctions[{i}]')
add('za:foundation/practice', 'tos.entity.thought-method', 'INQUIRY_FOUNDATION.practice', grounds=ground_cites(foundation.get('grounds')))
for lens, value in f['LENS_INQUIRY'].items():
    if value.get('question'): add(f'za:lens:{lens}/question', 'tos.entity.question', f'LENS_INQUIRY.{lens}.question', f'lens:{lens}')
    if value.get('method'): add(f'za:lens:{lens}/method', 'tos.entity.thought-method', f'LENS_INQUIRY.{lens}.method', f'lens:{lens}', cites(value['method']) + ground_cites(value.get('grounds')))
for edge in edges:
    a, b = nodes.get(edge['from'], {}).get('kind'), nodes.get(edge['to'], {}).get('kind')
    if a == 'interpretation' and edge['kind'] == 'interprets' and b == 'concept':
        relate('tos.relation.conception-of', node_object[edge['from']], node_object[edge['to']], edge['id'])
    if a in ('symbol', 'character') and edge['kind'] in ('supports', 'develops') and b == 'interpretation':
        relate('tos.relation.image-presents-conception', node_object[edge['from']], node_object[edge['to']], edge['id'])
link_list = [{'kind': l['kind'], 'from': l['from'], 'to': l['to'], 'form': l['path']} for l in links]
module = {
    'schema': 'tos.bonsai.objects.v1',
    'posture': {
        'ru': 'Чем являются собственные слова стенда в типах объектов Древа — проблемы и вопросы, понятия и трактовки, их аспекты и позиции, доводы и возражения, различения, методы мысли, образы мысли и мысленные эксперименты — с формой, где стоят их слова, местами текста, на которые они опираются, и связями из реестра Древа там, где их даёт само устройство стенда. Рёбра «сравнивает», «ставит вопрос» и «развивает» не говорят, какое отношение реестра имеется в виду, и остаются без него; тезисы, посылки и шаги вывода в стенде пока не выделены. Связи между местами текстов (повторяет, варьирует, развивает, противопоставляет, перекликается) в реестре Древа отношения пока не имеют и хранятся как предложение стенда. Ничто здесь не принято в Древо.',
        'en': 'What the stand’s own words are in the Tree’s object types — problems and questions, concepts and conceptions, their aspects and positions, arguments and objections, distinctions, methods and images of thought, thought experiments — with the form that holds their words, the places of text they rest on, and relations from the Tree’s registry where the stand’s own structure gives them. Edges that compare, question or develop do not say which registry relation they mean and stay without one; theses, premises and inference steps are not yet set apart in the stand. Links between places of texts (repeats, varies, develops, contrasts, echoes) have no relation in the Tree’s registry yet and are kept as the stand’s proposal. Nothing here is admitted to the Tree.'},
    'registry': {'entities': 'ToS/doctrine/semantic-interchange/entity-types.v1.json', 'relations': 'ToS/doctrine/semantic-interchange/relation-types.v1.json'},
    'objects': objects, 'relations': relations, 'links': link_list,
}
def dump(m):
    """One entry per line: readable diffs and a small file."""
    out = ['{']
    for i, (k, v) in enumerate(m.items()):
        comma = ',' if i < len(m) - 1 else ''
        if isinstance(v, list):
            out.append(f' {json.dumps(k)}: [')
            out += ['  ' + json.dumps(x, ensure_ascii=False, separators=(',', ':')) + (',' if j < len(v) - 1 else '') for j, x in enumerate(v)]
            out.append(' ]' + comma)
        else:
            out.append(f' {json.dumps(k)}: ' + json.dumps(v, ensure_ascii=False, indent=1).replace('\n', '\n ') + comma)
    return '\n'.join(out + ['}']) + '\n'
open(f'{REPO}/bonsai/zarathustra/objects.json', 'w').write(dump(module))
print(len(objects), dict(collections.Counter(o['type'].split('.')[-1] for o in objects)))
print(len(relations), dict(collections.Counter(r['type'].split('.')[-1] for r in relations)), len(link_list))

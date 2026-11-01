"""Types the Antiquity stand's own words in the Tree's object types: bonsai/antiquity/objects.json.

What the stand's structure says is typed and related; nothing else:
  a question of the collection is a problem, articulated by its question, and its positions address it;
  a concept is a crosscutting concept; a term is a lexeme and each of its uses an occurrence of it;
  an argument is an argument; its steps are theses (premise, claim, condition, definition, consequence,
  explanation, qualification, reply, conclusion), objections, distinctions, problems or questions as their
  role says; where premises support a conclusion, the argument has an inference step with those premises
  and that conclusion, and the argument is for its conclusions; an objection step that tests, answers or
  qualifies a thesis objects to it; a reading of an argument and the reading of a branch node are
  conceptions; a branch node's question and a branch's question are questions.
Links between places of texts that the stand's words name are kept as the stand's proposal with both places.
The first branches repeat the collection's branches and are typed there."""
import os as _os
# The review folder (bonsai/review) and the repository root; TOS_REVIEW and TOS_REPO override them.
REVIEW = _os.environ.get('TOS_REVIEW') or _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__)))
REPO_ROOT = _os.environ.get('TOS_REPO') or _os.path.dirname(_os.path.dirname(REVIEW))
import json, collections
S = REVIEW
REPO = REPO_ROOT
c = json.load(open(f'{REPO}/bonsai/antiquity/collection.json'))
links = json.load(open(f'{S}/anti/rewrite/links.json'))
THESIS = {'premise', 'claim', 'condition', 'definition', 'consequence', 'explanation', 'qualification', 'reply', 'conclusion', 'inference'}
ROLE_TYPE = {'objection': 'tos.entity.objection', 'distinction': 'tos.entity.distinction', 'problem': 'tos.entity.problem', 'question': 'tos.entity.question'}
SUPPORT = {'supports', 'entails', 'leads'}
cites = lambda v: list(v.get('cite') or []) if isinstance(v, dict) else []
objects, relations, ids = [], [], set()
def add(id_, type_, form, material=None, grounds=(), role=None):
    assert id_ not in ids, id_
    ids.add(id_)
    o = {'id': id_, 'type': type_, 'form': form}
    if material: o['material'] = material
    if role: o['role'] = role
    g = []
    for x in grounds:
        if x not in g: g.append(x)
    if g: o['grounds'] = g
    objects.append(o); return id_
def relate(type_, a, b, basis): relations.append({'type': type_, 'from': a, 'to': b, 'basis': basis})

for i, e in enumerate(c['entities']):
    if e['kind'] == 'concept':
        add(f'an:{e["id"]}', 'tos.entity.crosscutting-concept', f'collection.entities[{i}].body', e['id'], cites(e.get('body')))
for i, q in enumerate(c['questions']):
    base = f'collection.questions[{i}]'
    problem = add(f'an:{q["id"]}', 'tos.entity.problem', f'{base}.body', q['id'], cites(q.get('body')))
    if q.get('question'):
        relate('tos.relation.problem-has-question', problem, add(f'an:{q["id"]}/question', 'tos.entity.question', f'{base}.question', q['id']), f'{base}.question')
    for j, p in enumerate(q.get('positions') or []):
        position = add(f'an:{q["id"]}/position/{j}', 'tos.entity.position', f'{base}.positions[{j}]', p.get('passage') or q['id'], cites(p.get('body')))
        relate('tos.relation.position-addresses-problem', position, problem, f'{base}.positions[{j}]')
for i, t in enumerate(c['terms']):
    base = f'collection.terms[{i}]'
    lexeme = add(f'an:term:{t["id"]}', 'tos.entity.lexeme', f'{base}.body', t['id'], cites(t.get('body')))
    for j, u in enumerate(t.get('uses') or []):
        relate('tos.relation.occurrence-of-lexeme', add(f'an:term:{t["id"]}/use/{j}', 'tos.entity.occurrence', f'{base}.uses[{j}]', u.get('part') or t['id'], cites(u.get('body'))), lexeme, f'{base}.uses[{j}]')
for i, p in enumerate(c['passages']):
    a = p.get('argument')
    if not a: continue
    base = f'collection.passages[{i}].argument'
    argument = add(f'an:{p["id"]}/argument', 'tos.entity.argument', base, p['id'], [g for s in a.get('steps') or [] for g in cites(s.get('body'))])
    step_object, role_of = {}, {}
    for k, s in enumerate(a.get('steps') or []):
        role = s.get('role') or a.get('roles', {}).get(s['id'])
        role_of[s['id']] = role
        type_ = 'tos.entity.thesis' if role in THESIS else ROLE_TYPE.get(role)
        if type_: step_object[s['id']] = add(f'an:{p["id"]}/step/{s["id"]}', type_, f'{base}.steps[{k}]', p['id'], cites(s.get('body')), role)
    # Premises that support a conclusion make one inference step of the argument.
    incoming = collections.defaultdict(list)
    for k, l in enumerate(a.get('links') or []):
        f, t = step_object.get(l['from']), step_object.get(l['to'])
        if not f or not t: continue
        if l['kind'] in SUPPORT and role_of[l['to']] in THESIS and role_of[l['from']] in THESIS: incoming[l['to']].append((l['from'], k))
        if role_of[l['from']] == 'objection' and l['kind'] in ('tests', 'answers', 'qualifies') and role_of[l['to']] in THESIS:
            relate('tos.relation.objection-to-thesis', f, t, f'{base}.links[{k}]')
    for target, sources in incoming.items():
        step = add(f'an:{p["id"]}/inference/{target}', 'tos.entity.inference-step', f'{base}.links[{sources[0][1]}]', p['id'])
        relate('tos.relation.argument-has-step', argument, step, base)
        for source, k in sources: relate('tos.relation.step-has-premise', step, step_object[source], f'{base}.links[{k}]')
        relate('tos.relation.step-has-conclusion', step, step_object[target], f'{base}.links[{sources[0][1]}]')
    for s in a.get('steps') or []:
        if role_of[s['id']] == 'conclusion': relate('tos.relation.argument-for-thesis', argument, step_object[s['id']], base)
    for k, r in enumerate(a.get('readings') or []):
        add(f'an:{p["id"]}/reading/{k}', 'tos.entity.conception', f'{base}.readings[{k}]', p['id'], cites(r.get('body')))
for b, branch in enumerate(c['branches']):
    base = f'collection.branches[{b}]'
    if branch.get('question'): add(f'an:branch:{branch["id"]}/question', 'tos.entity.question', f'{base}.question', branch['id'])
    for n, node in enumerate(branch.get('nodes') or []):
        add(f'an:branch:{branch["id"]}/{node["id"]}', 'tos.entity.conception', f'{base}.nodes[{n}].reading', node['id'], cites(node.get('reading')))
        if node.get('question'): add(f'an:branch:{branch["id"]}/{node["id"]}/question', 'tos.entity.question', f'{base}.nodes[{n}].question', node['id'])
# A saying a later work quotes from an earlier one recalls it (the one classifier kind outside the stand's vocabulary).
KIND = {'cites': 'recalls'}
link_list = [{'kind': KIND.get(l['kind'], l['kind']), 'from': l['ends'][0], 'to': l['ends'][-1], 'form': l['path']} for l in links if l.get('kind') and len(l.get('ends') or []) == 2 and l['path'].startswith('collection.')]
module = {
    'schema': 'tos.bonsai.objects.v1',
    'posture': {
        'ru': 'Чем являются собственные слова стенда «Античность» в типах объектов Древа — проблемы и вопросы, позиции, понятия, лексемы и их употребления, доводы с их тезисами, шагами вывода и возражениями, различения, трактовки — с формой, где стоят их слова, местами текста, на которые они опираются, и связями из реестра Древа там, где их даёт устройство стенда. Посылки и заключения — роли тезисов в шаге вывода. Связи между местами текстов хранятся как предложение стенда. Первые ветви повторяют ветви коллекции и типизированы там. Ничто здесь не принято в Древо.',
        'en': 'What the Antiquity stand’s own words are in the Tree’s object types — problems and questions, positions, concepts, lexemes and their occurrences, arguments with their theses, inference steps and objections, distinctions, conceptions — with the form that holds their words, the places of text they rest on, and relations from the Tree’s registry where the stand’s structure gives them. Premises and conclusions are roles of theses within an inference step. Links between places of texts are kept as the stand’s proposal. The first branches repeat the collection’s branches and are typed there. Nothing here is admitted to the Tree.'},
    'registry': {'entities': 'ToS/doctrine/semantic-interchange/entity-types.v1.json', 'relations': 'ToS/doctrine/semantic-interchange/relation-types.v1.json'},
    'objects': objects, 'relations': relations, 'links': link_list,
}
def dump(m):
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
open(f'{REPO}/bonsai/antiquity/objects.json', 'w').write(dump(module))
print(len(objects), dict(collections.Counter(o['type'].split('.')[-1] for o in objects)))
print(len(relations), dict(collections.Counter(r['type'].split('.')[-1] for r in relations)), 'links', len(link_list))

import assert from 'node:assert/strict';
import test from 'node:test';
import {boundedFormCost, decodeHumanFormSelection, encodeHumanFormSelection, FORM_CODEC_ROLES, FORM_WIRE_BUDGET} from '../../../shared/human-form-selection-codec.ts';
import {selectHumanForms} from '../src/human-forms.ts';

type Item = Record<string, unknown>;
function ref(id: string) { return {id: 'tos.test.' + id, version: 1, digest: 'sha256:' + 'a'.repeat(64)}; }
function fixture(large = false) {
  const subject = ref('subject');
  const context = [{slot: 'mandatory', binding: {record: subject, pointer: '/qualifiers'},
    value: {unreviewed: true, wording: 'Квалификация '.repeat(large ? 200 : 1)}}];
  const forms = FORM_CODEC_ROLES.slice(0, 4).map(role => ({
    schema_version: 'tos_human_form_materialization_v1', form: ref(role), subject, state: 'ready', role,
    language: 'ru', script: 'Cyrl', display_text: role + ' wording', derivation: 'source-copy',
    context: structuredClone(context), standalone_reading: false, performs_semantic_assessment: false,
    admission: {limits: ['Research only '.repeat(large ? 23 : 1), 'Not canon', role], is_semantic_evaluation: false},
    future: {empty: {}, null: null, false: false, zero: 0},
  }));
  return {entity_id: subject.id, content_revision: 'b'.repeat(64), attributes: {
    source_record: {record_id: subject.id, record_version: 1}, source_sha256: 'a'.repeat(64), human_forms: forms}};
}
function roles(selection: Item) { return selection.roles as Record<string, Item>; }
function logical(large = false) {
  const node = fixture(large), result = selectHumanForms(node);
  for (const packet of node.attributes.human_forms) roles(result)[packet.role] = {state: 'ready', reason: 'automatic', form: packet.form, packet};
  return result;
}

test('shared transport preserves every JSON field and copies each reconstructed packet', () => {
  const original = logical(true), wire = encodeHumanFormSelection(original);
  const decoded = decodeHumanFormSelection(JSON.parse(JSON.stringify(wire)));
  assert.deepEqual(decoded, original);
  assert.ok(boundedFormCost(wire, FORM_WIRE_BUDGET) <= FORM_WIRE_BUDGET);
  const first = roles(decoded).name!.packet as Item, second = roles(decoded).caption!.packet as Item;
  assert.notEqual(first.context, second.context);
  (first.context as Item[])[0]!.value = 'changed';
  assert.notDeepEqual(first.context, second.context);
  assert.deepEqual(decodeHumanFormSelection(wire), original);
});

test('v1 stays default; shared allocation keeps all four forms with complete context', () => {
  const node = fixture(true), before = structuredClone(node);
  const inline = selectHumanForms(node), wire = selectHumanForms(node, 'auto', 'shared-v2');
  assert.equal(inline.schema_version, 'tos_human_form_selection_v1');
  assert.ok(Object.values(roles(inline)).filter(role => role.state === 'ready').length < 4);
  assert.equal(Object.values(roles(wire)).filter(role => role.state === 'ready').length, 4);
  assert.deepEqual(decodeHumanFormSelection(wire), logical(true));
  assert.deepEqual(node, before);
});

test('missing, null, empty and repeated limits remain distinct', () => {
  const value = logical();
  delete (roles(value).name!.packet as Item).admission;
  (roles(value).caption!.packet as Item).admission = null;
  (roles(value).hover!.packet as Item).admission = {unknown: []};
  (roles(value).statement!.packet as Item).admission = {limits: ['a', 'a', 'b', 'a']};
  assert.deepEqual(decodeHumanFormSelection(encodeHumanFormSelection(value)), value);
});

test('bad refs, unsafe keys, overlaps, role contradictions and unused pools fail closed', () => {
  const wire = encodeHumanFormSelection(logical());
  for (const index of [true, false, -1, 10000, 1.5, null, '0']) {
    const corrupt = structuredClone(wire), delta = roles(corrupt).name!.packet_delta as Item;
    (delta.admission as Item).limit_refs = [index];
    assert.throws(() => decodeHumanFormSelection(corrupt));
  }
  const mutations: ((value: Item) => void)[] = [
    value => { (roles(value).name!.packet_delta as Item).context = (value.packet_base as Item).context; },
    value => { (value.shared_limits as string[]).push('unused'); },
    value => { (value.shared_limits as string[]).push((value.shared_limits as string[])[0]!); },
    value => { roles(value).name!.packet_delta = null; },
    value => { roles(value).technical!.packet_delta = {}; },
    value => { roles(value).eighth = roles(value).name!; },
    value => { Object.defineProperty(value.packet_base, '__proto__', {value: {}, enumerable: true}); },
    value => { (value.packet_base as Item).large = 'x'.repeat(16_384); },
    value => { ((value.packet_base as Item).admission as Item).limits = []; },
  ];
  for (const mutate of mutations) {
    const corrupt = structuredClone(wire); mutate(corrupt);
    assert.throws(() => decodeHumanFormSelection(corrupt));
  }
  const source = logical();
  ((roles(source).name!.packet as Item).admission as Item).limit_refs = [];
  assert.throws(() => encodeHumanFormSelection(source));
});

test('bounded traversal rejects non-JSON values and recursion before cloning', () => {
  const cycle: unknown[] = []; cycle.push(cycle);
  const accessor = Object.defineProperty({}, 'x', {get() { throw new Error('getter must not run'); }, enumerable: true});
  const arrayAccessor = Object.defineProperty([], '0', {get() { throw new Error('getter must not run'); }, enumerable: true});
  const extras: unknown[] & {extra?: string} = []; extras.extra = 'not JSON';
  for (const value of [cycle, accessor, arrayAccessor, extras, Array(2), [undefined], new Date(), NaN, Infinity, 1n, Symbol('x'), Array(30_001).fill(null)]) {
    assert.throws(() => boundedFormCost(value, 524_288));
  }
  let depth: Item = {};
  for (let index = 0; index < 65; index++) depth = {next: depth};
  assert.throws(() => boundedFormCost(depth, 524_288));
  const oversized = logical();
  (roles(oversized).name!.packet as Item).future = 'x'.repeat(65_536);
  assert.throws(() => encodeHumanFormSelection(oversized, {enforceBudget: false}));
  const wire = encodeHumanFormSelection(logical());
  (wire.shared_limits as string[])[0] = 'x'.repeat(1000);
  ((roles(wire).name!.packet_delta as Item).admission as Item).limit_refs = Array(100).fill(0);
  assert.ok(boundedFormCost(wire, FORM_WIRE_BUDGET) <= FORM_WIRE_BUDGET);
  assert.throws(() => decodeHumanFormSelection(wire));
});

test('selection preserves exact-language priority and all invalid source guards', () => {
  const node = fixture(true);
  node.attributes.human_forms[0]!.language = 'en';
  const wire = selectHumanForms(node, 'ru', 'shared-v2');
  assert.equal(roles(wire).caption!.reason, 'exact-language');
  assert.equal(roles(wire).name!.reason, 'fallback');
  const original = selectHumanForms(node, 'original', 'shared-v2');
  assert.equal(roles(original).caption!.state, 'unavailable');
  node.attributes.human_forms[0]!.subject = ref('wrong');
  assert.equal(selectHumanForms(node, 'ru', 'shared-v2').state, 'invalid');
  assert.throws(() => selectHumanForms(fixture(), 'auto', 'v3'));
});

test('form ref factoring rejects mismatches, malformed identities and inline shadow refs', () => {
  const original = logical();
  for (const change of [{id: 'tos.test.other'}, {version: 2}, {digest: 'sha256:' + 'b'.repeat(64)},
    {version: true}, {id: 42}, {extra: 'forbidden'}]) {
    const value = structuredClone(original);
    roles(value).name!.form = {...roles(value).name!.form as Item, ...change};
    assert.throws(() => encodeHumanFormSelection(value));
  }
  for (const bad of [null, {}, {id: 'x', version: 1}, {id: 'x', version: true, digest: 'sha256:' + 'a'.repeat(64)}]) {
    for (const target of ['role', 'packet']) {
      const value = structuredClone(original), selected = roles(value).name!;
      (target === 'role' ? selected : selected.packet as Item).form = bad;
      assert.throws(() => encodeHumanFormSelection(value));
    }
    const wire = encodeHumanFormSelection(original);
    roles(wire).name!.form = bad;
    assert.throws(() => decodeHumanFormSelection(wire));
  }
  const missing = structuredClone(original);
  delete (roles(missing).name!.packet as Item).form;
  assert.throws(() => encodeHumanFormSelection(missing));
  const wire = encodeHumanFormSelection(original);
  assert.equal(Object.hasOwn(wire.packet_base as Item, 'form'), false);
  for (const selected of Object.values(roles(wire))) if (selected.state === 'ready') assert.equal(Object.hasOwn(selected.packet_delta as Item, 'form'), false);
  for (const target of ['base', 'delta']) {
    const corrupt = structuredClone(wire);
    (target === 'base' ? corrupt.packet_base as Item : roles(corrupt).name!.packet_delta as Item).form = ref('name');
    assert.throws(() => decodeHumanFormSelection(corrupt));
  }
  const selected = roles(decodeHumanFormSelection(wire)).name!;
  assert.notEqual(selected.form, (selected.packet as Item).form);
});

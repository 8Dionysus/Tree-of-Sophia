import {expect, test} from 'vitest';
import {personalKind, readPersonalKinds} from './personal-kind.mjs';

function storage(entries) {
  const map = new Map(Object.entries(entries));
  return {length: map.size, key: i => [...map.keys()][i], getItem: key => map.get(key) ?? null};
}
const a = `tos.public-research.graph.v1.antiquity.${'a'.repeat(64)}`;
const z = `tos.public-research.graph.v1.zarathustra.${'b'.repeat(64)}`;
const graph = (kind, id = 'draft:1') => JSON.stringify({nodes: [{id, kind}]});
const owners = (id, draft = 'draft:1') => JSON.stringify({[id]: draft});

test('legacy type follows an unambiguous binding of the same owner, not a reused draft id', () => {
  const s = storage({[a]: graph('question'), [`${a}.owners`]: owners('owner-a'), [z]: graph('note'), [`${z}.owners`]: owners('other-owner')});
  expect(personalKind({id: 'owner-a'}, readPersonalKinds(s))).toBe('question');
  expect(personalKind({id: 'missing'}, readPersonalKinds(s))).toBe('thought');
});

test('conflicting legacy profile copies stay neutral and an explicit owner kind is authoritative', () => {
  const s = storage({[a]: graph('question'), [`${a}.owners`]: owners('owner-a'), [z]: graph('note'), [`${z}.owners`]: owners('owner-a')});
  expect(personalKind({id: 'owner-a'}, readPersonalKinds(s), 'note')).toBe('thought');
  expect(personalKind({id: 'owner-a', kind: 'question'}, readPersonalKinds(s), 'note')).toBe('question');
});

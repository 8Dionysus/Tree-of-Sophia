import {describe, expect, it} from 'vitest';
import {applyGraphSpace, exportGraphSpace, validateGraphSpace} from './graph-transfer.mjs';

const fingerprint = 'a'.repeat(64);
const graphKey = `tos.public-research.graph.v1.zarathustra.${fingerprint}`;
const packet = (title = 'Tree') => JSON.stringify({schema: 'tos_constructor_workspace_v1', version: 1, libraryFingerprint: fingerprint,
  title, nodes: [{id: 'material:work'}], edges: [], revision: 1});
function storage(seed = {}) {
  const values = new Map(Object.entries(seed));
  return {values, get length() { return values.size; }, key(index) { return [...values.keys()][index] ?? null; },
    getItem(key) { return values.get(key) ?? null; }, setItem(key, value) { values.set(key, String(value)); }};
}

describe('public research graph transfer', () => {
  it('keeps legacy v1 packets without the optional graph-space companion valid', () => {
    expect(validateGraphSpace(undefined)).toBeNull();
    expect(() => validateGraphSpace({schema: 'tos.public_research.graph_space.v1', version: 1, entries: [{key: 'private.secret', value: '{}'}]})).toThrow();
    expect(() => validateGraphSpace({schema: 'tos.public_research.graph_space.v1', version: 1, entries: [{key: 'tos.public-research.graph-positions.v1.__proto__', value: '{}'}]})).toThrow();
  });

  it('exports bounded graph packets and companion records without rewriting Rust packet bytes', () => {
    const raw = ` ${packet()} `;
    const store = storage({[graphKey]: raw, [`${graphKey}.owners`]: JSON.stringify({'space:zarathustra:one': 'material:work'}),
      'tos.public-research.graph-positions.v1.zarathustra': JSON.stringify({'personal:note': [1, 2, 3]}),
      'unrelated.private.key': 'secret'});
    const exported = exportGraphSpace(store);
    expect(exported.entries.map(item => item.key).sort()).toEqual([graphKey, `${graphKey}.owners`, 'tos.public-research.graph-positions.v1.zarathustra'].sort());
    expect(exported.entries.find(item => item.key === graphKey).value).toBe(raw);
    expect(validateGraphSpace(exported)).toEqual(exported);
  });

  it('adds only absent or equal records and preserves a conflicting original graph', () => {
    const ownersKey = `${graphKey}.owners`, positionsKey = 'tos.public-research.graph-positions.v1.zarathustra';
    const incoming = validateGraphSpace({schema: 'tos.public_research.graph_space.v1', version: 1, entries: [
      {key: graphKey, value: packet('Incoming')}, {key: ownersKey, value: JSON.stringify({'space:zarathustra:one': 'material:work'})},
      {key: positionsKey, value: JSON.stringify({'personal:note': [4, 5, 6]})},
    ]});
    const store = storage({[graphKey]: packet('Local')});
    const result = applyGraphSpace(store, incoming);
    expect(store.getItem(graphKey)).toBe(packet('Local'));
    expect(store.getItem(ownersKey)).toBeNull();
    expect(store.getItem(positionsKey)).toBeNull();
    expect(result.conflicts).toContain(graphKey);

    const equalStore = storage({[graphKey]: packet('Incoming')});
    const additive = applyGraphSpace(equalStore, incoming);
    expect(equalStore.getItem(graphKey)).toBe(packet('Incoming'));
    expect(equalStore.getItem(ownersKey)).toBe(incoming.entries[1].value);
    expect(equalStore.getItem(positionsKey)).toBe(incoming.entries[2].value);
    expect(additive.equal).toContain(graphKey);
    expect(additive.added).toEqual([ownersKey, positionsKey]);
  });

  it('imports into a fresh seeded space while protecting an edited destination', () => {
    const pristine = packet('Prepared tree');
    const incoming = validateGraphSpace({schema: 'tos.public_research.graph_space.v1', version: 1,
      entries: [{key: graphKey, value: packet('My saved tree')}]});
    const options = {pristineGraphs: {[graphKey]: pristine}};
    const fresh = storage({[graphKey]: pristine});
    expect(applyGraphSpace(fresh, incoming, options).added).toEqual([graphKey]);
    expect(fresh.getItem(graphKey)).toBe(packet('My saved tree'));
    const edited = storage({[graphKey]: packet('Existing personal work')});
    expect(applyGraphSpace(edited, incoming, options).conflicts).toEqual([graphKey]);
    expect(edited.getItem(graphKey)).toBe(packet('Existing personal work'));
  });
});

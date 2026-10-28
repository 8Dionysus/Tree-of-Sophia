import {expect, test} from 'vitest';
import {matchesCollectionKind} from './collection-view.mjs';

test('the visible fragments-and-passages filter includes both existing kinds without changing them', () => {
  const entries = [{id: 'one', kind: 'fragment'}, {id: 'two', kind: 'passage'}, {id: 'three', kind: 'work'}];
  expect(entries.filter(item => matchesCollectionKind('@passages', item.kind))).toEqual(entries.slice(0, 2));
  expect(entries.map(item => item.kind)).toEqual(['fragment', 'passage', 'work']);
  expect(entries.filter(item => matchesCollectionKind('work', item.kind))).toEqual([entries[2]]);
});

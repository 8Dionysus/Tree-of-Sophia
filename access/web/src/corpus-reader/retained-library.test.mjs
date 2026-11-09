import {expect,test} from 'vitest';
import data from '../../../public-corpus/editorial-annotations.v1.json' with {type:'json'};
import {editorialRanges} from './editorial.mjs';
import {loadRetainedLibrary,loadRetainedMap,retainedLibrary} from './retained-library.mjs';

test('the retained library comes once, on demand, and editorial ranges read it from then on',async()=>{
  const projected=data.versions.find(value=>value.versionId==='perseus-rackham-1934-en'),note=projected.spans[0];
  const version={id:projected.versionId,revision:projected.revision},unit={id:note.unitId,text:'·'.repeat(note.start)+note.expected};
  expect(retainedLibrary()).toBeNull();
  expect(editorialRanges(version,unit)).toEqual([]);
  const [first,second]=await Promise.all([loadRetainedLibrary(),loadRetainedLibrary()]);
  expect(first).toBe(second);
  expect(retainedLibrary()).toBe(first);
  expect(await loadRetainedMap()).toBe(first.map);
  expect(first.map.books.length).toBeGreaterThan(0);
  expect(editorialRanges(version,unit)).toEqual([note]);
});

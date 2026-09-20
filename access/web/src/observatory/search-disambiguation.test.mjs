import {expect,test} from 'vitest';
import {searchDisambiguators} from './search-disambiguation.mjs';
const row=(id,version,source='source-navigation')=>({title:'Одинаковое название',kind:'node',raw:{id,source_graph:source,attributes:{record_version:version}}});
test('colliding names have visible, stable distinctions while exact carriers stay unchanged',()=>{
  const rows=[row('tos.record.c'),row('tos.record.a'),row('tos.record.b')],before=structuredClone(rows);
  const initial=searchDisambiguators(rows);
  expect(new Set(initial.values()).size).toBe(3);
  for(const value of initial.values()){expect(value).toMatch(/Запись [1-3]/);expect(value).not.toContain('tos.');}
  let seed=915;
  for(let round=0;round<50;round++){
    const shuffled=[...rows];for(let i=shuffled.length-1;i>0;i--){seed=(Math.imul(seed,1664525)+1013904223)>>>0;const j=seed%(i+1);[shuffled[i],shuffled[j]]=[shuffled[j],shuffled[i]];}
    expect(searchDisambiguators(shuffled)).toEqual(initial);
  }
  expect(rows).toEqual(before);expect(searchDisambiguators(rows.slice(0,1)).size).toBe(0);
});
test('declared distinct versions and sources provide readable distinctions',()=>{
  const labels=searchDisambiguators([row('a',1),row('b',2),row('c',1,'philosophy')],'en');
  expect(labels.get('node:a')).toContain('Version 1');expect(labels.get('node:b')).toContain('Version 2');
  expect(labels.get('node:c')).not.toContain('Record');expect(new Set(labels.values()).size).toBe(3);
});

test('distinct supplied descriptions need no extra ordinal',()=>{expect(searchDisambiguators([{...row('a'),detail:'Первый источник'},{...row('b'),detail:'Второй источник'}]).size).toBe(0);});

test('repeated links to one carrier do not create a false record distinction',()=>{const a=row('a'),b=row('b');expect(searchDisambiguators([a,a]).size).toBe(0);expect(searchDisambiguators([a,a,b])).toEqual(searchDisambiguators([a,b]));});

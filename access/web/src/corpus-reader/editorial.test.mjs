import {expect,test} from 'vitest';
import data from '../../../public-corpus/editorial-annotations.v1.json' with {type:'json'};
import {editorialRanges,editorialKindForMatch,quotationWithEditorialRoles,sourceLayerKind} from './editorial.mjs';

test('source roles are bound to the exact layer, unit, offsets, and characters',()=>{
  const projected=data.versions.find(value=>value.versionId==='perseus-rackham-1934-en');
  expect(projected.spans).toHaveLength(696);
  const note=projected.spans.find(value=>value.unitId==='tos.text-unit.sid-ef390552b4ed407db5b54b74bc3e843c');
  const version={id:projected.versionId,revision:projected.revision};
  const unit={id:note.unitId,text:'🌳'.repeat(note.start)+note.expected+' end'};
  expect(editorialRanges(version,unit)).toEqual([note]);
  const quote=quotationWithEditorialRoles(version,unit,{start:note.start,end:note.end},'en');
  expect(quote).toContain('[Note by Rackham: '+note.expected+']');
  expect(editorialRanges({...version,revision:'different'},unit)).toEqual([]);
  expect(editorialRanges(version,{...unit,text:unit.text.replace('Or possibly','Changed')})).toEqual([]);
});

test('known addition and deletion remain source text, with explicit roles in copies',()=>{
  for(const [versionId,kind,expected] of [
    ['retained-athenian-constitution-grc2-6e0c298a511e','addition','κατηγοροῦντος'],
    ['retained-ethics-grc2-a203a8d2e793','deletion','καὶ ἑκόντι'],
  ]){
    const projected=data.versions.find(value=>value.versionId===versionId);
    const span=projected.spans.find(value=>value.kind===kind && value.expected===expected);
    expect(span).toBeTruthy();
    const unit={id:span.unitId,text:'α'.repeat(span.start)+expected+' end'};
    const version={id:versionId,revision:projected.revision};
    expect(editorialRanges(version,unit)).toContainEqual(span);
    expect(quotationWithEditorialRoles(version,unit,{start:span.start,end:span.end},'en')).toContain(`[Editorial ${kind}: ${expected}]`);
    expect(unit.text).toContain(expected);
  }
});

test('publication and collected testimonies are labeled only in the pinned source range',()=>{
  const gutenberg={id:'gutenberg-7205-de',revision:'4f71adfeeb39e64055acc2fc3f58964cf39f03d27bf2264d25313bc45d8658ec'};
  expect(sourceLayerKind(gutenberg,{ordinal:3724},20)).toBe('publication');
  expect(sourceLayerKind(gutenberg,{ordinal:3691},20)).toBeNull();
  const fate={id:'retained-de-fato-lat1-b6f9f292b185',revision:'dc65d650f24a16ca9d017debfe1c880191fa4eebdccd94d070731884e13d04e0'};
  const unit={id:'tos.text-unit.sid-d3bc0bc17eff4b6ca63c698e89b0f3de',text:'x'.repeat(619)+'Fragmenta\nGellius'};
  expect(sourceLayerKind(fate,unit,620)).toBe('fragments');
  expect(sourceLayerKind(fate,unit,100)).toBeNull();
  expect(quotationWithEditorialRoles(fate,unit,{start:619,end:628},'en')).toContain('[Editorial collection of fragments and testimonies: Fragmenta]');
});

test('later Hildebert witness is marked only after the verified boundary in the pinned unit',()=>{
  const version={id:'retained-de-clementia-lat2-a170840d8fc4',revision:'dff84f24224394557b5009e99cddb22c6c37e5f56f8b0c0a95e175b1e9d218c3'};
  const marker='Hildebertus Cenomanensis ep. 1.3 (CLXXI. 145 Mign.)';
  const unit={id:'tos.text-unit.sid-19fbcffd2c0f44c2bf340365636cc66b',text:'x'.repeat(2074)+marker+' De clementia quoque compendiosa principibus capitula'};
  const original=unit.text;
  expect(editorialRanges(version,unit)).toEqual([{start:2074,end:Array.from(unit.text).length,kind:'laterWitness'}]);
  expect(editorialKindForMatch(version,unit,2060,2070)).toBeNull();
  expect(editorialKindForMatch(version,unit,2074+marker.length+31,2074+marker.length+41)).toBe('laterWitness');
  expect(quotationWithEditorialRoles(version,unit,{start:2074,end:2074+marker.length},'en')).toContain(`[Later witness: Hildebert’s letter and transmitted fragments: ${marker}]`);
  expect(editorialRanges({...version,revision:'changed'},unit)).toEqual([]);
  expect(editorialRanges(version,{...unit,id:'different'})).toEqual([]);
  expect(editorialRanges(version,{...unit,text:unit.text.replace('Hildebertus','Differentus')})).toEqual([]);
  expect(unit.text).toBe(original);
});

import {describe,it,expect} from 'vitest';
import {bindInquiryLayer,checkInquiryTextReferences,checkGrounds,inquiryReadingContexts,routeGroundContext} from './inquiry-layer.mjs';
import {LENSES} from './atlas-view.mjs';
import {LENS_INQUIRY,INQUIRY_FOUNDATION} from './inquiry-foundation.mjs';
const bi={ru:'Содержание',en:'Content'};
const grounds=()=>[{ref:'z-vision',focus:bi}];
const node=()=>({argument:bi,grounds:grounds()});
const input=()=>({sources:{a:node()},readings:{b:node()},relations:{edge:{warrant:bi,grounds:grounds()}}});
describe('authored inquiry binding',()=>{
 it('requires exact coverage and refuses duplicate material ownership',()=>{
  expect(()=>bindInquiryLayer(['a','b','c'],['edge'],input())).toThrow(/coverage/);
  expect(()=>bindInquiryLayer(['a','b'],['edge','missing'],input())).toThrow(/coverage/);
  const overlap=input();overlap.readings={a:node()};expect(()=>bindInquiryLayer(['a','b'],['edge'],overlap)).toThrow(/Duplicate/);
 });
 it('requires both languages before exposing the layer',()=>{
  const incomplete=input();incomplete.sources.a.grounds[0].focus={ru:'Только русский'};
  expect(()=>bindInquiryLayer(['a','b'],['edge'],incomplete)).toThrow(/Missing en inquiry/);
 });
 it('allows a grounded reading without manufacturing an objection or exercise',()=>{
  const original=input(),before=structuredClone(original),bound=bindInquiryLayer(['a','b'],['edge'],original);
  expect(bound.material('a').counterReading).toBeUndefined();expect(bound.material('a').experiment).toBeUndefined();
  expect(bound.material('a').anchors).toEqual([{passageId:'z-vision',focus:bi}]);expect(original).toEqual(before);
 });
 it('requires an actual reference and checks an optional alternative on its own grounds',()=>{
  for(const bad of [[],[{ref:'invented',focus:bi}]])expect(()=>checkGrounds(bad,'example')).toThrow();
  const value=input();value.sources.a.counterReading={text:bi,grounds:[]};
  expect(()=>bindInquiryLayer(['a','b'],['edge'],value)).toThrow(/Missing textual grounds/);
  value.sources.a.counterReading.grounds=[{ref:'gs276',focus:bi}];
  expect(bindInquiryLayer(['a','b'],['edge'],value).material('a').anchors).toHaveLength(1);
 });
 it('refuses anchors to absent or link-only text while leaving the source catalog unchanged',()=>{
  const catalog={passages:[{id:'available',status:'available'},{id:'external',status:'link-only'}]},before=structuredClone(catalog);
  for(const id of ['external','missing'])expect(()=>checkInquiryTextReferences([{id:'a',inquiry:{anchors:[{passageId:id}]}}],catalog)).toThrow(/unavailable/);
  expect(()=>checkInquiryTextReferences([{id:'a',inquiry:{anchors:[{passageId:'available'}]}}],catalog)).not.toThrow();expect(catalog).toEqual(before);
 });
 it('gives each actual lens a bilingual question, method and limitation',()=>{
  expect(new Set(Object.keys(LENS_INQUIRY))).toEqual(new Set(LENSES.map(lens=>lens.id)));
  for(const lens of Object.values(LENS_INQUIRY))for(const field of ['question','method','blindSpot'])for(const code of ['ru','en'])expect(lens[field][code].trim()).not.toBe('');
  for(const field of ['question','orientation','practice'])for(const code of ['ru','en'])expect(INQUIRY_FOUNDATION[field][code].trim()).not.toBe('');
 });
 it('keeps a route-only passage attached to its stop when a reader note develops it',()=>{
  const own={ru:'Собственное чтение',en:'Node reading'},routeFocus={ru:'Чтение маршрута',en:'Route reading'};
  const nodes=[{id:'a',inquiry:{anchors:[{passageId:'z-vision',focus:own}]}},{id:'b'}],routes=[{steps:[{nodeId:'a',grounds:[{ref:'z-convalescent',focus:routeFocus},{ref:'z-vision',focus:routeFocus},{ref:'gs276',focus:routeFocus}]},{nodeId:'b',grounds:[]}],grounds:[{ref:'z-redemption',focus:routeFocus}]}],before=structuredClone({nodes,routes});
  const index=inquiryReadingContexts(nodes,routes);
  expect(index.get('a').get('z-convalescent')).toEqual(routeFocus);expect(index.get('a').get('z-vision')).toEqual(own);expect(index.get('a').has('gs276')).toBe(false);expect(index.get('b').get('z-redemption')).toEqual(routeFocus);expect({nodes,routes}).toEqual(before);
  expect(routeGroundContext(routes[0],'z-convalescent')).toBe('a');expect(routeGroundContext(routes[0],'z-redemption')).toBe('b');
 });
});

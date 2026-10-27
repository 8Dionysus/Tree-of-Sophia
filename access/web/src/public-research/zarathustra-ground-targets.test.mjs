import {describe,it,expect} from 'vitest';
import {createZarathustraGroundTargets,GROUND_VERSIONS,ZARATHUSTRA_DOCUMENT_ID} from './zarathustra-ground-targets.mjs';
const document = () => ({id:ZARATHUSTRA_DOCUMENT_ID,versions:Object.values(GROUND_VERSIONS).map(value=>({...value}))});
describe('public Zarathustra textual grounds',()=>{
  it('opens independently addressed German and English gateway paragraphs',()=>{
    const targets=createZarathustraGroundTargets(document())('z-vision','demo:gateway');
    expect(targets.filter(t=>t.scope==='passage').map(t=>[t.language,t.scope,t.address.unitId])).toEqual([
      ['de','passage','tos.text-unit.sid-9a7e0d9b40004d93935f6141982e678e'],
      ['en','passage','tos.text-unit.sid-327b4400bd0e447ba87e7c9159ba06f3'],
    ]);
    expect(targets.every(t=>t.address.documentId===ZARATHUSTRA_DOCUMENT_ID&&t.address.revision)).toBe(true);
  });
  it('labels a broader ground as a chapter rather than inventing a focused paragraph',()=>{
    const targets=createZarathustraGroundTargets(document())('z-prologue','work');
    expect(targets).toHaveLength(2);expect(targets.every(t=>t.scope==='chapter')).toBe(true);
  });
  it('suppresses a stale edition without disabling the separately valid edition',()=>{
    const doc=document();doc.versions[0].revision='f'.repeat(64);
    expect(createZarathustraGroundTargets(doc)('z-vision','demo:gateway').filter(t=>t.scope==='passage').map(t=>t.language)).toEqual(['en']);
  });
  it('does not substitute another work or a missing source for an authored reference',()=>{
    const doc=document();doc.id='tos.work.other';
    expect(createZarathustraGroundTargets(doc)('z-vision','demo:gateway')).toEqual([]);
    expect(createZarathustraGroundTargets(document())('gs341','demo:recurrence')).toEqual([]);
  });
});

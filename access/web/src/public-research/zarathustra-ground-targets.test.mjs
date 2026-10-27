import {describe,it,expect} from 'vitest';
import {readFileSync} from 'node:fs';
import {RESEARCH_ROUTES} from '../../constructor/atlas-routes.mjs';
import {PRIMARY_SOURCES} from '../../constructor/source-references.mjs';
import {createZarathustraGroundTargets,createZarathustraRouteGroundTargets,GROUND_VERSIONS,ZARATHUSTRA_DOCUMENT_ID,ZARATHUSTRA_ROUTE_GROUNDS} from './zarathustra-ground-targets.mjs';
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

describe('public Zarathustra route-step grounds',()=>{
  const corpusRoot=new URL('../../public/public-corpus/',import.meta.url);
  const manifest=JSON.parse(readFileSync(new URL('manifest.json',corpusRoot),'utf8'));
  const work=manifest.documents.find(entry=>entry.id===ZARATHUSTRA_DOCUMENT_ID);
  const published=JSON.parse(readFileSync(new URL(work.manifest.path,corpusRoot),'utf8'));
  const resolve=createZarathustraRouteGroundTargets(published);
  it('chooses a named source for every authored step and addresses the chosen edition',()=>{
    expect(Object.keys(ZARATHUSTRA_ROUTE_GROUNDS).sort()).toEqual(RESEARCH_ROUTES.map(route=>route.id).sort());
    for(const route of RESEARCH_ROUTES){
      expect(ZARATHUSTRA_ROUTE_GROUNDS[route.id]).toHaveLength(route.steps.length);
      route.steps.forEach((step,index)=>{
        const choice=ZARATHUSTRA_ROUTE_GROUNDS[route.id][index];
        expect(step.grounds.map(ground=>ground.ref)).toContain(choice.ref);
        expect(PRIMARY_SOURCES[choice.ref]).toBeDefined();
        for(const language of ['ru','en']){
          const result=resolve(route.id,index,language);
          if(result.kind==='external') expect(PRIMARY_SOURCES[result.ref].url).toMatch(/^https?:\/\//);
          else {
            expect(result.kind).toBe('exact');
            const version=published.versions.find(item=>item.id===result.address.versionId);
            expect(version.revision).toBe(result.address.revision);
            expect(version.units.some(unit=>unit.id===result.address.unitId)).toBe(true);
          }
        }
      });
    }
  });
  it('keeps the inscription before the dwarf and preserves an external author',()=>{
    const ordinal=(route,index,lang)=>{
      const address=resolve(route,index,lang).address;
      return published.versions.find(item=>item.id===address.versionId).units.find(unit=>unit.id===address.unitId).ordinal;
    };
    expect([ordinal('moment-time',0,'ru'),ordinal('moment-time',0,'en')]).toEqual([1848,2037]);
    expect([ordinal('affirmation-pain-action',2,'ru'),ordinal('affirmation-pain-action',2,'en')]).toEqual([3642,3885]);
    expect(resolve('repetition-difference',2,'en')).toEqual({kind:'external',ref:'deleuze-nietzsche-return'});
    expect(resolve('repetition-difference',3,'en')).toEqual({kind:'external',ref:'deleuze-nietzsche-return',fragment:'#page=12'});
    expect(resolve('thinkers-necessity',4,'ru')).toEqual({kind:'external',ref:'spinoza1d7'});
  });
  it('does not claim an exact address against a changed edition revision',()=>{
    const stale=structuredClone(published);stale.versions.find(version=>version.language==='de').revision='f'.repeat(64);
    const staleResolve=createZarathustraRouteGroundTargets(stale);
    expect(staleResolve('moment-time',0,'ru')).toEqual({kind:'bibliographic',ref:'z-vision'});
    expect(staleResolve('moment-time',0,'en').kind).toBe('exact');
  });
});

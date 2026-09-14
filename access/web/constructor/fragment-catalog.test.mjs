import {describe,it,expect} from 'vitest';
import {bindFragmentCatalog,textDigest,fragmentText} from './fragment-catalog.mjs';
const bi=t=>({ru:t,en:t});
async function fixture(){
 const version={paragraphs:['First whole paragraph.','Second whole paragraph.'],translator:bi('Translator'),edition:bi('Historical edition'),editorialNote:bi('No omissions'),sourceUrl:'https://example.org/source',sourceRevision:'edition-1900',rights:{label:'Public domain',url:'https://example.org/rights',uses:['local-reading','video-display'],basis:bi('Historical source'),credit:bi('Author, translator, edition'),jurisdictions:['US','RU']}};
 version.textSha256=await textDigest(fragmentText(version));
 return {schema:'tos_demo_fragments_v1',audience:'local-reading-and-recorded-video',passages:[{id:'one',title:bi('Unit'),work:bi('Work'),author:bi('Author'),locator:bi('§1'),status:'available',complete:true,boundary:bi('Entire section 1'),versions:{ru:structuredClone(version),en:structuredClone(version)}}],bindings:[{nodeId:'star',passageIds:['one'],context:bi('Why this passage belongs here')}]};
}
describe('complete fragment and display-rights bindings',()=>{
 it('retains every paragraph and resolves the selected material without changing the manifest',async()=>{const data=await fixture(),before=structuredClone(data),bound=await bindFragmentCatalog(data,['star']);expect(fragmentText(bound.forMaterial('star').passages[0].versions.ru)).toBe('First whole paragraph.\n\nSecond whole paragraph.');expect(bound.forMaterial('missing')).toBe(null);expect(data).toEqual(before);});
 it('rejects changed or truncated source content even when the claimed boundary remains complete',async()=>{const data=await fixture();data.passages[0].versions.ru.paragraphs.pop();await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('digest mismatch');});
 it('does not treat personal-use permission as permission for a recorded video',async()=>{const data=await fixture();data.passages[0].versions.en.rights.uses=['local-reading'];await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('video-display');});
 it('requires both language versions and a declared complete source unit',async()=>{const data=await fixture();delete data.passages[0].versions.en;await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('Missing en');const complete=await fixture();complete.passages[0].complete=false;await expect(bindFragmentCatalog(complete,['star'])).rejects.toThrow('complete');});
 it('rejects unresolved, duplicate or foreign material bindings',async()=>{const data=await fixture();data.bindings[0].passageIds=['missing'];await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('Unresolved');data.bindings[0].passageIds=['one'];data.bindings.push(structuredClone(data.bindings[0]));await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('duplicate');await expect(bindFragmentCatalog(await fixture(),['foreign'])).rejects.toThrow('Unknown');});
 it('keeps unavailable works as links without smuggling their text into the catalog',async()=>{const data=await fixture(),p=data.passages[0];p.status='link-only';p.reason=bi('Permission has not been obtained');p.links=[{label:'Publisher',url:'https://example.org/publisher'}];await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('hidden protected text');delete p.versions;expect((await bindFragmentCatalog(data,['star'])).forMaterial('star').passages[0].status).toBe('link-only');});
 it('rejects executable source and permission links',async()=>{const data=await fixture();data.passages[0].versions.ru.sourceUrl='javascript:alert(1)';await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('source URL');});
 it('validates every original paragraph and its display basis alongside unchanged translations',async()=>{
  const data=await fixture(),passage=data.passages[0],translations=structuredClone(passage.versions);
  passage.originalLanguage='grc';passage.versions.grc={...structuredClone(passage.versions.en),paragraphs:['τῶν ὄντων τὰ μέν ἐστιν ἐφ’ ἡμῖν']};passage.versions.grc.textSha256=await textDigest(fragmentText(passage.versions.grc));
  expect((await bindFragmentCatalog(data,['star'])).passages.get('one').originalLanguage).toBe('grc');
  expect({ru:passage.versions.ru,en:passage.versions.en}).toEqual(translations);
  const bad=structuredClone(data);bad.passages[0].versions.grc.paragraphs[0]+=' changed';await expect(bindFragmentCatalog(bad,['star'])).rejects.toThrow('digest mismatch');
  passage.versions.grc.rights.uses=['local-reading'];await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('video-display');
 });
 it('requires an explicit matching original language for any additional version',async()=>{
  const data=await fixture(),passage=data.passages[0];passage.versions.de=structuredClone(passage.versions.en);
  await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('declare its original');
  passage.originalLanguage='fr';await expect(bindFragmentCatalog(data,['star'])).rejects.toThrow('original-language');
  passage.originalLanguage='de';expect((await bindFragmentCatalog(data,['star'])).passages.size).toBe(1);
 });
});

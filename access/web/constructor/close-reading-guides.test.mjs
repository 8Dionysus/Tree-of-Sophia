import {describe,it,expect} from 'vitest';
import {bindCloseReadingGuides,CLOSE_READING_GUIDES,GUIDE_CATALOG_SHA256} from './close-reading-guides.mjs';

// No source text is copied into the repository to test positional safety.
const catalog=()=>({passages:Object.keys(CLOSE_READING_GUIDES).map(id=>({id,status:'available',versions:Object.fromEntries(Object.keys(CLOSE_READING_GUIDES[id].moves[0].paragraphs).map(code=>[code,{paragraphs:Array(80).fill('Text')}]))}))});
describe('edition-bound reading positions',()=>{
 it('withholds old guidance for a changed edition while allowing its reader to continue',()=>{
  expect(bindCloseReadingGuides({passages:[]},'b'.repeat(64)).size).toBe(0);
 });
 it('binds complete available guides without modifying the supplied catalog',()=>{
  const supplied=catalog(),before=structuredClone(supplied),guides=bindCloseReadingGuides(supplied,GUIDE_CATALOG_SHA256);
  expect(guides.size).toBe(13);expect(supplied).toEqual(before);
  expect(guides.has('camus-sisyphus')).toBe(false);expect(guides.has('deleuze-repetition')).toBe(false);
 });
 it('refuses a jump outside any version instead of applying the other version’s position',()=>{
  const supplied=catalog();supplied.passages.find(item=>item.id==='z-vision').versions.de.paragraphs.length=40;
  expect(()=>bindCloseReadingGuides(supplied,GUIDE_CATALOG_SHA256)).toThrow(/outside its version/);
 });
 it('refuses guidance for missing or link-only text even if a caller supplies the expected digest',()=>{
  const missing=catalog();missing.passages.pop();expect(()=>bindCloseReadingGuides(missing,GUIDE_CATALOG_SHA256)).toThrow(/available text/);
  const unavailable=catalog();unavailable.passages[0]={id:'z-vision',status:'link-only'};expect(()=>bindCloseReadingGuides(unavailable,GUIDE_CATALOG_SHA256)).toThrow(/available text/);
 });
});

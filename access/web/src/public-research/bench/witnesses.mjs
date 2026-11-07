// The book's witnesses as the bonsai keeps them, read chapter by chapter when a window asks for one.

import {readChapter} from '../../../../../bonsai/lib/witnesses.mjs';
import {findInChapter} from './place.mjs';

const files=import.meta.glob('../../../../../bonsai/zarathustra/witnesses/*.json',{import:'default'});
const loaded=new Map();
let manifest=null;

export const hasWitnesses=profile=>profile==='zarathustra';
export function witnessManifest(){return manifest??=import('../../../../../bonsai/zarathustra/witnesses.json').then(module=>module.default).catch(error=>{manifest=null;throw error;});}
export function chapterKeys(){return Object.keys(files).map(path=>path.match(/([^/]+)\.json$/)?.[1]).filter(Boolean).sort(compareChapters);}
export function compareChapters(x,y){
  const [px,cx]=x.split('.'),[py,cy]=y.split('.'),parts=['I','II','III','IV'];
  return parts.indexOf(px)-parts.indexOf(py)||Number(cx)-Number(cy);
}
export function loadChapter(key){
  const path=Object.keys(files).find(item=>item.endsWith(`/${key}.json`));
  if(!path)return Promise.reject(new Error(`No chapter ${key}`));
  if(!loaded.has(key))loaded.set(key,files[path]().then(readChapter).catch(error=>{loaded.delete(key);throw error;}));
  return loaded.get(key);
}
/** Every place of the book where a word or phrase stands in one witness, chapter after chapter. */
export async function findInBook(language,needle,{limit=400,signal}={}){
  const out=[];
  for(const key of chapterKeys()){
    if(signal?.aborted)break;
    const chapter=await loadChapter(key);
    for(const hit of findInChapter(chapter,language,needle)){out.push(hit);if(out.length>=limit)return {hits:out,more:true};}
  }
  return {hits:out,more:false};
}

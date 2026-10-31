// Every cited span becomes whole sentences of the original: a quote that started or ended inside a
// sentence (dropping a negation, a condition or a main clause) now shows the sentences it stood in,
// as the translations, aligned sentence by sentence, already do. Identical cites of one form merge.
//   node snap_cites.mjs [--write]
import {readFileSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
const REPO=new URL('../../../',import.meta.url).pathname.replace(/\/$/,'');
const S=new URL('../',import.meta.url).pathname.replace(/\/$/,'');
const {sentenceSpans}=await import(join(REPO,'bonsai/lib/align.mjs'));
const {readChapter}=await import(join(REPO,'bonsai/lib/witnesses.mjs'));
const {corpusReader}=await import(join(REPO,'bonsai/lib/corpus-align.mjs'));
const write=process.argv.includes('--write');
const read=corpusReader(join(REPO,'access/web/public/public-corpus'));
const json=path=>JSON.parse(readFileSync(path,'utf8'));
const chars=text=>Array.from(text);
const stats={snapped:0,merged:0,cites:0};const examples=[];

const chapters=new Map();
const witnessUnit=(c,id)=>{if(!chapters.has(c))chapters.set(c,readChapter(json(join(REPO,'bonsai/zarathustra/witnesses',`${c}.json`))));const ch=chapters.get(c);return ch.units.de[ch.index.de.get(id)];};
const units=new Map();
function corpusUnit(stand,w,id){
  const alignment=json(join(REPO,'bonsai',stand,'alignment.json')),work=alignment.works[w];
  if(!work)return null;
  const key=`${work.documentId} ${work.original.versionId}`;
  if(!units.has(key)){const doc=read(work.documentId),version=doc?.versions.find(v=>v.id===work.original.versionId);units.set(key,{language:version?.language,byId:new Map((version?.units??[]).map(u=>[u.id,u]))});}
  const {language,byId}=units.get(key),unit=byId.get(id);
  return unit?{text:unit.text,language}:null;
}
function snapSpan(text,language,s,e){
  const spans=sentenceSpans(text,language),length=chars(text).length;
  let ns=s,ne=e;
  if(Number.isInteger(s)){const span=spans.find(([a,b])=>a<=s&&s<b)??spans.find(([a])=>a>=s);if(span)ns=span[0];}
  if(Number.isInteger(e)){const span=spans.find(([a,b])=>a<e&&e<=b)??[...spans].reverse().find(([,b])=>b<=e);if(span)ne=span[1];}
  if(ns===0)ns=undefined;if(ne===length)ne=undefined;
  return [ns,ne];
}
function snapCite(stand,cite){
  if(!cite||typeof cite!=='object'||!Array.isArray(cite.u)||(!Number.isInteger(cite.s)&&!Number.isInteger(cite.e)))return cite;
  const [first,last]=cite.u;let a,b;
  if(cite.w==='also-sprach-zarathustra'){a=witnessUnit(cite.c,first);b=witnessUnit(cite.c,last);if(!a||!b)return cite;a={text:a.raw,language:'de'};b={text:b.raw,language:'de'};}
  else{a=corpusUnit(stand,cite.w,first);b=corpusUnit(stand,cite.w,last);if(!a||!b)return cite;}
  const out={...cite};
  if(Number.isInteger(cite.s)){const [s]=snapSpan(a.text,a.language,cite.s,undefined);if(s===undefined)delete out.s;else out.s=s;}
  if(Number.isInteger(cite.e)){const [,e]=snapSpan(b.text,b.language,undefined,cite.e);if(e===undefined)delete out.e;else out.e=e;}
  if(first===last){const len=chars(a.text).length,o=(cite.e??len)-(cite.s??0),n=(out.e??len)-(out.s??0);(globalThis.growth??=[]).push([o,n,cite.w,cite.s,cite.e]);}
  if(out.s!==cite.s||out.e!==cite.e){stats.snapped++;if(examples.length<12)examples.push([cite.w,cite.s,cite.e,'->',out.s,out.e]);}
  return out;
}
// Consecutive cites of one unit with only a space or a numeral between them are one place.
function adjacent(list){
  const out=[];
  for(const cite of list){
    const prev=out.at(-1);
    if(prev&&cite&&prev.w===cite.w&&prev.c===cite.c&&Array.isArray(prev.u)&&Array.isArray(cite.u)&&prev.u[0]===prev.u[1]&&JSON.stringify(prev.u)===JSON.stringify(cite.u)&&Number.isInteger(prev.e)&&Number.isInteger(cite.s)&&cite.s-prev.e>=0&&cite.s-prev.e<=20){
      const merged={...prev};if(Number.isInteger(cite.e))merged.e=cite.e;else delete merged.e;out[out.length-1]=merged;stats.adjacent=(stats.adjacent??0)+1;continue;
    }
    out.push(cite);
  }
  return out;
}
function walk(stand,value){
  if(Array.isArray(value))return value.map(item=>walk(stand,item));
  if(!value||typeof value!=='object')return value;
  const out={};
  for(const [key,item] of Object.entries(value)){
    if(key==='cite'&&Array.isArray(item)){
      const seen=new Set(),list=[];
      for(const cite of item){stats.cites++;const snapped=snapCite(stand,cite),k=JSON.stringify(snapped);if(seen.has(k)){stats.merged++;continue;}seen.add(k);list.push(snapped);}
      out[key]=adjacent(list);
    }else out[key]=walk(stand,item);
  }
  return out;
}

// Antiquity: the collection and the first branches, in their own formatting.
const collection=walk('antiquity',json(join(REPO,'bonsai/antiquity/collection.json')));
const first=walk('antiquity',json(join(REPO,'bonsai/antiquity/first-branches.json')));
// Zarathustra: the constructor modules.
const C=join(REPO,'access/web/constructor');
const modules={
  ...(await import(join(C,'atlas-content.mjs'))),...(await import(join(C,'atlas-routes.mjs'))),...(await import(join(C,'relation-inquiry.mjs'))),
  ...(await import(join(C,'inquiry-sources-concepts.mjs'))),...(await import(join(C,'inquiry-readings-echoes.mjs'))),...(await import(join(C,'inquiry-foundation.mjs'))),
  ...(await import(join(C,'close-reading-guides.mjs')))};
const names=['NODE_CONTENT','RESEARCH_ROUTES','RELATION_INQUIRY','INQUIRY_SOURCES_CONCEPTS','INQUIRY_READINGS_ECHOES','INQUIRY_FOUNDATION','LENS_INQUIRY','CLOSE_READING_GUIDES'];
const snapshot=Object.fromEntries(names.map(name=>[name,walk('zarathustra',modules[name])]));
snapshot.GUIDE_CATALOG_SHA256=modules.GUIDE_CATALOG_SHA256;
console.log(stats);console.log(examples);
const g=globalThis.growth.filter(([o,n])=>n!==o);const buckets={};for(const [o,n] of g){const k=n<=300?'<=300':n<=600?'<=600':n<=1000?'<=1000':n<=2000?'<=2000':'>2000';buckets[k]=(buckets[k]||0)+1;}
console.log('new lengths of changed single-unit cites',buckets,'ratio>4:',g.filter(([o,n])=>n>4*o).length);
console.log(g.filter(([o,n])=>n>1500).slice(0,15));
if(write){
  const inline=value=>{const tokens=[];const mark=x=>{if(Array.isArray(x))return x.map(mark);if(x&&typeof x==='object'){if('w' in x&&('u' in x||'loc' in x)&&Object.keys(x).every(k=>['w','c','u','s','e','loc'].includes(k))){tokens.push(JSON.stringify(x));return `@@cite${tokens.length-1}@@`;}return Object.fromEntries(Object.entries(x).map(([k,y])=>[k,mark(y)]));}return x;};
    return JSON.stringify(mark(value),null,2).replace(/"@@cite(\d+)@@"/g,(_,i)=>tokens[Number(i)])+'\n';};
  writeFileSync(join(REPO,'bonsai/antiquity/collection.json'),inline(collection));
  writeFileSync(join(REPO,'bonsai/antiquity/first-branches.json'),JSON.stringify(first)+'\n');
  writeFileSync(join(S,'verify/snapshot.json'),JSON.stringify(snapshot));
  console.log('written; run write_modules.mjs on verify/snapshot.json');
}

// The typed objects carry copies of cites (grounds, the ends of links): they take the same spans.
if(process.argv.includes('--objects')){
  const isCite=x=>x&&typeof x==='object'&&!Array.isArray(x)&&typeof x.w==='string'&&Array.isArray(x.u);
  const fix=(stand,x)=>{
    if(Array.isArray(x)){const seen=new Set(),out=[];for(const item of x){const v=fix(stand,item),k=isCite(v)?JSON.stringify(v):null;if(k&&seen.has(k))continue;if(k)seen.add(k);out.push(v);}return out.every(isCite)&&out.length?adjacent(out):out;}
    if(isCite(x))return snapCite(stand,x);
    if(x&&typeof x==='object')return Object.fromEntries(Object.entries(x).map(([k,v])=>[k,fix(stand,v)]));
    return x;
  };
  for(const stand of ['antiquity','zarathustra']){
    const path=join(REPO,'bonsai',stand,'objects.json'),module=fix(stand,json(path));
    const lines=['{'],entries=Object.entries(module);
    entries.forEach(([key,value],i)=>{const comma=i<entries.length-1?',':'';
      if(Array.isArray(value)){lines.push(` ${JSON.stringify(key)}: [`);value.forEach((item,j)=>lines.push('  '+JSON.stringify(item)+(j<value.length-1?',':'')));lines.push(' ]'+comma);}
      else lines.push(` ${JSON.stringify(key)}: `+JSON.stringify(value,null,1).replace(/\n/g,'\n ')+comma);});
    lines.push('}');if(write)writeFileSync(path,lines.join('\n')+'\n');
  }
  console.log('objects',stats);
}

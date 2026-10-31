// The corpus alignment for every unit the classified slices anchor to, made by the repository's mechanism
// (bonsai/lib/corpus-align.mjs). Writes anti/alignment.current.json for the edit slices; the stand's own
// alignment.json is made from the merged content by `cli.mjs align-corpus antiquity`.
import {readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {alignCorpus,corpusReader} from '../../../lib/corpus-align.mjs';
const ANTI=new URL('../',import.meta.url).pathname;
const REPO=new URL('../../../../',import.meta.url).pathname;
const read=corpusReader(join(REPO,'access/web/public/public-corpus'));
const manifest=JSON.parse(readFileSync(join(REPO,'access/web/public/public-corpus/manifest.json'),'utf8'));
const identities=JSON.parse(readFileSync(join(REPO,'bonsai/antiquity/identities.json'),'utf8'));
const keyOf=Object.fromEntries(Object.entries(identities.works).map(([key,value])=>[value.tree,key]));
const ALIAS={ench:'tos.work.retained-antiquity.enchiridion',disc:'tos.work.retained-antiquity.discourses',med:'tos.work.retained-antiquity.meditations',meno:'tos.work.plato.meno',apology:'tos.work.plato.apology',ne:'tos.work.aristotle.nicomachean-ethics'};
const ids=manifest.documents.map(d=>d.id);
const resolve=doc=>{if(ids.includes(doc))return doc;if(ALIAS[doc])return ALIAS[doc];const m=ids.filter(id=>id.endsWith('.'+doc));if(m.length===1)return m[0];throw new Error(`doc ${doc}: ${m}`);};
const cited=new Map(),works={};
for(const file of readdirSync(join(ANTI,'out')).filter(name=>name.endsWith('.jsonl')).sort()){
  for(const line of readFileSync(join(ANTI,'out',file),'utf8').split('\n').filter(Boolean)){
    for(const sentence of JSON.parse(line).sentences)for(const anchor of [...(sentence.anchors??[]),...(sentence.link?[sentence.link.from,sentence.link.to].filter(Boolean):[])]){
      if(!anchor?.doc||anchor.doc==='za'||!anchor.units?.length)continue;
      const doc=resolve(anchor.doc),key=keyOf[doc]??doc,work=read(doc);if(!work)continue;
      const original=work.versions.find(version=>version.units.some(unit=>unit.id===anchor.units[0]));if(!original)continue;
      works[key]=doc;
      const entry=cited.get(key)??cited.set(key,{original:original.id,units:new Set()}).get(key);
      for(const unit of anchor.units)entry.units.add(unit);
    }
  }
}
const t0=Date.now(),out=alignCorpus(cited,works,read);
writeFileSync(join(ANTI,'alignment.current.json'),JSON.stringify(out));
console.log(Object.keys(out.works).length,'works',Object.keys(out.units).length,'units',Math.round((Date.now()-t0)/1000)+'s');

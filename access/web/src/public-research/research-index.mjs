// One addressable projection for cards, reference reading and agent operations.
// Authored records retain their owners; this index records references, not new claims.
export {canonicalJSON} from '../../../../bonsai/lib/history-format.mjs';
import {canonicalJSON} from '../../../../bonsai/lib/history-format.mjs';

// The index's search belongs to the native query owner (tos-query, through the rules WASM): the index
// hands it each item's canonical texts once and asks it per query.
let SearchKernel=null;
const encoder=new TextEncoder(),decoder=new TextDecoder();
export function configureResearchIndexSearch(runtime){
  if(typeof runtime?.ResearchIndexSearchV1!=='function')throw new TypeError('Research index search WASM rule is unavailable');
  SearchKernel=runtime.ResearchIndexSearchV1;
}
const ZARATHUSTRA='also-sprach-zarathustra';
const hasUnits=cite=>Array.isArray(cite?.u)&&cite.u.length>0&&cite.u.every(id=>typeof id==='string'&&id.length>0);
const locatorKeys=['w','c','u','s','e','loc','documentId','workId','version','versionId','revision','witness','witnessId','layer','layerId','layerDigest','textRevision','reference'];
// Before v2 the address omitted the work and offsets whenever unit IDs were
// present. Keep it for whole-unit Zarathustra citations already in use.
const oldCitationId=cite=>typeof cite?.c==='string'&&(hasUnits(cite)||(Number.isInteger(cite.s)&&Number.isInteger(cite.e)))
  ?`tos.bonsai.passage.dta.${cite.c}.${hasUnits(cite)?cite.u.join('..'):`${cite.s}.${cite.e}`}`:null;
export function citationId(cite){
  const whole=cite?.w===ZARATHUSTRA&&typeof cite.c==='string'&&hasUnits(cite)
    &&cite.s==null&&cite.e==null&&cite.loc==null
    &&locatorKeys.slice(6).every(key=>cite[key]==null);
  if(whole)return oldCitationId(cite);
  // Compact the known TextUnit prefix without losing identity: `~` marks the
  // prefix, while `!` distinguishes any other unit ID. Keep the locator in
  // the address and the complete original citation on the index record.
  const locator=Object.fromEntries(locatorKeys.flatMap((key,index)=>cite?.[key]===undefined?[]:[[
    index.toString(36),key==='u'&&hasUnits(cite)?cite.u.map(id=>id.startsWith('tos.text-unit.sid-')?`~${id.slice(18)}`:`!${id}`):cite[key],
  ]]));
  const id=`tos.bonsai.p2.${canonicalJSON(locator)}`;
  if(id.length>256)throw new RangeError('Citation locator exceeds the research address limit.');
  return id;
}
export function citedPlaces(value){
  const found=new Map();
  const visit=item=>{if(!item||typeof item!=='object')return;
    if((typeof item.w==='string'&&(hasUnits(item)||typeof item.loc==='string'))
      ||(typeof item.c==='string'&&(hasUnits(item)||(Number.isInteger(item.s)&&Number.isInteger(item.e)))))found.set(canonicalJSON(item),item);
    else for(const child of Object.values(item))if(Array.isArray(child))child.forEach(visit);else visit(child);
  };visit(value);return [...found.values()];
}
const pathValue=(record,path)=>path==='$'?record:path.split('.').reduce((value,key)=>value?.[key],record);
const text=value=>typeof value==='string'?value:typeof value?.ru==='string'?value.ru:typeof value?.en==='string'?value.en:'';

/**
 * `placeTitle(cite)` names a cited place for readers ({ru, en}): the chapter or the work and its locator.
 * Without it a place is named by its work, chapter and locator; unit identifiers stay in the record.
 */
export function createResearchIndex(library,{revision=library.space?.researchRevision??library.fingerprint,placeTitle=null}={}){
  const core=library.space?.core,items=new Map(),aliases=new Map(),incoming=new Map(),oldCitationTargets=new Map();
  const add=item=>{if(!item?.id||items.has(item.id))return;items.set(item.id,{revision,...item});for(const alias of item.aliases??[])if(alias&&!aliases.has(alias))aliases.set(alias,item.id);};
  for(const material of library.nodes??[])add({id:material.id,kind:material.kind,title:material.title,record:material,materialId:material.id,owner:'library',dependencies:[],aliases:[`material:${material.id}`]});
  for(const record of [...(core?.concepts??[]),...(core?.figures??[]),...(core?.proposed??[])]){
    const materialId=core.materialIds?.[record.id];
    add({id:record.id,kind:record.existence?'figure':record.type??'conception',title:record.title,record,materialId,owner:'bonsai/core',dependencies:[],aliases:[materialId,`material:${materialId}`].filter(Boolean)});
    if(materialId){aliases.set(materialId,record.id);if(items.get(materialId)?.owner==='library')items.delete(materialId);}
  }
  for(const record of core?.links??[])add({id:record.id,kind:'relation',title:record.verb,record,owner:'bonsai/core',dependencies:[record.from,record.to],aliases:[`core-link:${record.id}`,`atlas:core-link:${record.id}`]});
  for(const chain of core?.chains??[]){
    add({id:chain.id,kind:'chain',title:chain.title,record:chain,owner:'bonsai/core',dependencies:chain.steps?.flatMap(step=>[step.from,step.to])??[],aliases:[`core:${chain.id}`]});
    for(const state of chain.states??[])add({id:state.id,kind:'state',title:state.title,record:state,owner:'bonsai/core',materialId:core.materialIds?.[state.id],dependencies:[chain.id]});
    for(const [i,step]of(chain.steps??[]).entries())add({id:step.id??`core-chain:${chain.id}:${i}`,kind:'relation',title:step.verb,record:step,owner:'bonsai/core',dependencies:[chain.id,step.from,step.to],aliases:[`core-chain:${chain.id}:${i}`,`atlas:core-chain:${chain.id}:${i}`,...(step.aliases??[])]});
  }
  for(const relation of core?.placeRelations?.relations??[])add({id:relation.id,kind:'passage-relation',title:relation.title??relation.verb??relation.relation,record:relation,owner:'bonsai/core',dependencies:[]});
  const owners=[...items.values()].filter(item=>item.owner==='bonsai/core');
  for(const owner of owners){
    const statements=(object,parent)=>{
      if(!object||typeof object!=='object')return;
      for(const [path,id]of Object.entries(object.statement_ids??{})){
        const value=pathValue(object,path);add({id,kind:'statement',title:{ru:`${text(owner.title)} · ${path}`,en:`${owner.record.title?.en??text(owner.title)} · ${path}`},record:value,owner:owner.owner,parentId:owner.id,path,dependencies:[owner.id]});
      }
      for(const [key,value]of Object.entries(object))if(key!=='statement_ids'&&key!=='proposed'&&value&&typeof value==='object'){
        if(Array.isArray(value))for(const entry of value)statements(entry,parent);else statements(value,parent);
      }
    };statements(owner.record,owner.id);
  }
  for(const item of [...items.values()]){
    for(const cite of citedPlaces(item.record)){
      const id=citationId(cite),old=oldCitationId(cite),units=hasUnits(cite),locator=units?cite.u.join(' – '):cite.loc??`${cite.s}–${cite.e}`;
      if(old){if(!oldCitationTargets.has(old))oldCitationTargets.set(old,new Set());oldCitationTargets.get(old).add(id);}
      const place=[cite.w,cite.c,units?null:locator].filter(Boolean).join(' · ');
      const title=placeTitle?.(cite)??{ru:place,en:place};
      add({id,kind:units?'passage':'bibliographic-pointer',title,record:cite,citations:[],owner:units?'bonsai/witnesses':'bonsai/bibliography',resolution:units?'source-reference':'unresolved-bibliographic',dependencies:[]});
      const stored=items.get(id);
      if(stored?.citations&&!stored.citations.some(existing=>canonicalJSON(existing)===canonicalJSON(cite)))stored.citations.push(cite);
      item.dependencies.push(id);
    }
    item.dependencies=[...new Set(item.dependencies.filter(Boolean))];
  }
  for(const [old,targets] of oldCitationTargets)if(targets.size===1&&!items.has(old)&&!aliases.has(old))aliases.set(old,[...targets][0]);
  const resolveId=id=>items.has(String(id))?String(id):aliases.get(String(id))??String(id);
  for(const item of items.values())for(const dependency of item.dependencies){const id=resolveId(dependency);if(!incoming.has(id))incoming.set(id,new Set());incoming.get(id).add(item.id);}
  const get=id=>items.get(resolveId(id))??null;
  const references=id=>[...(incoming.get(resolveId(id))??[])].map(get).filter(Boolean);
  let native=null;
  return {revision,items,resolveId,get,references,
    search(query,{limit=30,kind}={}){
      if(!SearchKernel)throw new Error('Research index search WASM rule is not configured');
      native??=new SearchKernel(encoder.encode(JSON.stringify([...items.values()].map(item=>({id:item.id,kind:item.kind,title:canonicalJSON(item.title)??'',record:canonicalJSON(item.record)??''})))));
      const ids=JSON.parse(decoder.decode(native.search(encoder.encode(JSON.stringify({query:String(query??''),limit,kind:kind??null})))));
      return ids.map(get).filter(Boolean);
    },
    inspect(id){const item=get(id);return item?{...item,dependencies:item.dependencies.map(key=>({id:key,kind:get(key)?.kind??'external'})),referencedBy:references(id).map(value=>({id:value.id,kind:value.kind,title:value.title}))}:null;},
    affected(id){const queue=[resolveId(id)],seen=new Set(queue);for(let i=0;i<queue.length;i++)for(const item of references(queue[i]))if(!seen.has(item.id)){seen.add(item.id);queue.push(item.id);}return queue.slice(1).map(get).filter(Boolean);}
  };
}

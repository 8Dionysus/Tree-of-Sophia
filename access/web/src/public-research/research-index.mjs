// One addressable projection for cards, reference reading and agent operations.
// Authored records retain their owners; this index records references, not new claims.
export {canonicalJSON} from '../../../../bonsai/lib/history-format.mjs';
import {canonicalJSON} from '../../../../bonsai/lib/history-format.mjs';
export const citationId=cite=>`tos.bonsai.passage.dta.${cite.c}.${Array.isArray(cite.u)?cite.u.join('..'):`${cite.s}.${cite.e}`}`;
export function citedPlaces(value){
  const found=new Map();
  const visit=item=>{if(!item||typeof item!=='object')return;
    if(typeof item.c==='string'&&((Array.isArray(item.u)&&item.u.length&&item.u.every(id=>typeof id==='string'))||(Number.isInteger(item.s)&&Number.isInteger(item.e))))found.set(citationId(item),item);
    else for(const child of Object.values(item))if(Array.isArray(child))child.forEach(visit);else visit(child);
  };visit(value);return [...found.values()];
}
const pathValue=(record,path)=>path==='$'?record:path.split('.').reduce((value,key)=>value?.[key],record);
const text=value=>typeof value==='string'?value:typeof value?.ru==='string'?value.ru:typeof value?.en==='string'?value.en:'';

export function createResearchIndex(library,{revision=library.space?.researchRevision??library.fingerprint}={}){
  const core=library.space?.core,items=new Map(),aliases=new Map(),incoming=new Map();
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
      const id=citationId(cite),locator=Array.isArray(cite.u)?cite.u.join(' – '):`${cite.s}–${cite.e}`;add({id,kind:'passage',title:{ru:`${cite.c} · ${locator}`,en:`${cite.c} · ${locator}`},record:cite,owner:'bonsai/witnesses',dependencies:[]});item.dependencies.push(id);
    }
    item.dependencies=[...new Set(item.dependencies.filter(Boolean))];
  }
  const resolveId=id=>aliases.get(String(id))??String(id);
  for(const item of items.values())for(const dependency of item.dependencies){const id=resolveId(dependency);if(!incoming.has(id))incoming.set(id,new Set());incoming.get(id).add(item.id);}
  const get=id=>items.get(resolveId(id))??null;
  const references=id=>[...(incoming.get(resolveId(id))??[])].map(get).filter(Boolean);
  return {revision,items,resolveId,get,references,
    search(query,{limit=30,kind}={}){const terms=String(query??'').toLocaleLowerCase().split(/\s+/).filter(Boolean);return [...items.values()].filter(item=>(!kind||item.kind===kind)&&terms.every(term=>`${item.id} ${canonicalJSON(item.title)} ${(canonicalJSON(item.record)??'').slice(0,5000)}`.toLocaleLowerCase().includes(term))).slice(0,Math.min(100,limit));},
    inspect(id){const item=get(id);return item?{...item,dependencies:item.dependencies.map(key=>({id:key,kind:get(key)?.kind??'external'})),referencedBy:references(id).map(value=>({id:value.id,kind:value.kind,title:value.title}))}:null;},
    affected(id){const queue=[resolveId(id)],seen=new Set(queue);for(let i=0;i<queue.length;i++)for(const item of references(queue[i]))if(!seen.has(item.id)){seen.add(item.id);queue.push(item.id);}return queue.slice(1).map(get).filter(Boolean);}
  };
}

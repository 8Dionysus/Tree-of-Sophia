// A bonsai's chronology: situations, each with one dating in the Tree's grammar.
//
// A situation is what a dating is about, as the Tree models it: a phase of a
// life (`biographical-phase`) or a process such as writing a work
// (`historical-process`). Its key is the Tree identity the bonsai proposes for
// it. A relative dating ("during") resolves through its anchor to the anchor's
// own span; a missing anchor or a cycle gives no span rather than a guess.

import {dateIssues,dateSpan,TREE_ID} from './time.mjs';

export const CHRONOLOGY_SCHEMA='tos.bonsai.chronology.v1';
export const SITUATION_TYPES=Object.freeze(['biographical-phase','historical-process','historical-event']);
export const FOOTINGS=Object.freeze(['high','medium','low']);
export const EPISTEMIC=Object.freeze(['reported','inferred']);

const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const bilingual=value=>object(value)&&typeof value.ru==='string'&&value.ru.trim()&&typeof value.en==='string'&&value.en.trim();

/** Read a chronology module; every lookup is safe on missing material. */
export function readChronology(module){
  const situations=object(module?.situations)?module.situations:{},sources=object(module?.sources)?module.sources:{};
  const spans=new Map();
  function span(id,trail=new Set()){
    if(spans.has(id))return spans.get(id);
    const own=situations[id];if(!own||trail.has(id))return null;
    trail.add(id);const result=dateSpan(own.dating,ref=>span(ref,trail));trail.delete(id);
    spans.set(id,result);return result;
  }
  const sourcesOf=id=>(situations[id]?.sources??[]).map(key=>object(sources[key])?{id:key,...sources[key]}:null).filter(Boolean);
  return {
    module,
    ids:()=>Object.keys(situations),
    situation:id=>situations[id]??null,
    span:id=>span(id),
    sources:sourcesOf,
    /** The situation whose dating a passage takes: its own composition, else its work's. */
    compositionOf:({passageId,workId}={})=>module?.compositions?.passages?.[passageId]??module?.compositions?.works?.[workId]??null,
    /** The phase that is an agent's life (or its documented activity, where only that is dated). */
    lifeOf:agentId=>module?.lives?.[agentId]??null,
  };
}

/** Where a chronology module departs from its own contract; empty when it holds. */
export function chronologyIssues(module){
  const issues=[];
  if(!object(module))return ['not an object'];
  if(module.schema!==CHRONOLOGY_SCHEMA)issues.push('schema');
  const situations=object(module.situations)?module.situations:{},sources=object(module.sources)?module.sources:{};
  for(const [id,source] of Object.entries(sources))if(!object(source)||typeof source.title!=='string'||typeof source.short!=='string'||!/^https:\/\//.test(source.url??''))issues.push(`source ${id}`);
  for(const [id,item] of Object.entries(situations)){
    const at=`situation ${id}`;
    if(!TREE_ID.test(id))issues.push(`${at}: key is not a Tree identity`);
    if(!SITUATION_TYPES.includes(item?.type)||!id.startsWith(`tos.${item?.type}.`))issues.push(`${at}: type`);
    if(!bilingual(item?.label))issues.push(`${at}: label`);
    if(!bilingual(item?.statement))issues.push(`${at}: statement`);
    if(!FOOTINGS.includes(item?.footing))issues.push(`${at}: footing`);
    if(!EPISTEMIC.includes(item?.epistemic))issues.push(`${at}: epistemic`);
    for(const problem of dateIssues(item?.dating))issues.push(`${at}: dating ${problem}`);
    const anchor=item?.dating?.relative?.anchor_ref;if(anchor&&!situations[anchor])issues.push(`${at}: anchor ${anchor} is not a situation of this chronology`);
    if(!Array.isArray(item?.sources)||!item.sources.length)issues.push(`${at}: sources`);
    else for(const key of item.sources)if(!sources[key])issues.push(`${at}: unknown source ${key}`);
    if(item?.display!==undefined&&!bilingual(item.display))issues.push(`${at}: display`);
    if(item?.note!==undefined&&!bilingual(item.note))issues.push(`${at}: note`);
  }
  const reader=readChronology(module);
  for(const [kind,map] of [['life',module.lives],['work',module.compositions?.works],['passage',module.compositions?.passages]])for(const [key,id] of Object.entries(object(map)?map:{})){
    if(!situations[id])issues.push(`${kind} ${key}: no situation ${id}`);
    else if(situations[id].dating?.kind!=='unknown-date'&&!reader.span(id))issues.push(`${kind} ${key}: ${id} gives no span`);
  }
  return issues;
}

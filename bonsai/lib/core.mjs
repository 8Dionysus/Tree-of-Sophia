// The philosophical core of a bonsai (Node only): bonsai/CORE.md.
//
// `core/core.json` (tos.bonsai.core.v1) says what the core holds and who made
// and judged it; beside it lie the voice map, the concepts and figures of the
// work, the links and chains between them with their verdicts and admission,
// the objects the grown nodes propose, the occurrences of each concept and the
// control vectors. The check reads every place the core cites against the
// witnesses, every claim against its statuses, every registry relation against
// its domain and range, and runs every control vector.

import {existsSync,readFileSync} from 'node:fs';
import {join} from 'node:path';

export const CORE_SCHEMA='tos.bonsai.core.v1';
export const CONTROL_SCHEMA='tos.bonsai.core.control.v1';
const STATUS=new Set(['text','reconstruction','hypothesis']);
const VERDICTS=new Set(['confirmed','reconstruction','revise','rejected','undecided']);
const MODES=new Set(['speech','song','narration','dream','quoted']);
const ADMITTED={map:new Set(['confirmed','reconstruction','revise']),held:new Set(['undecided']),rejected:new Set(['rejected'])};
const json=path=>JSON.parse(readFileSync(path,'utf8'));

/** The core's files, read from the folder of its `core.json`. Occurrences are read by concept. */
export function loadCore(dir){
  const read=name=>existsSync(join(dir,name))?json(join(dir,name)):null;
  const core=read('core.json'),concepts=read('concepts.json')??[];
  const occurrences=Object.fromEntries(concepts.map(concept=>[concept.id,concept.occurrences?read(concept.occurrences):null]));
  return {core,voices:read('voices.json')??{},concepts,figures:read('figures.json')??[],links:read('links.json')??[],chains:read('chains.json')??[],proposed:read('proposed.json')??[],control:read('control.json'),occurrences};
}

/** Text as the reader's search sees it: broken words joined, whitespace collapsed, case folded. */
const fold=raw=>String(raw??'').replace(/¬\s*/g,'').replace(/\s+/g,' ').trim().toLowerCase();

/** The German units of each chapter in order, with their folded text and positions. */
export function coreText(chapters,pivot='de'){
  const text={};
  for(const [key,file] of Object.entries(chapters)){
    const units=(file.units?.[pivot]??[]).map(([id,,raw])=>({id,text:fold(raw)}));
    text[key]={units,at:new Map(units.map((unit,i)=>[unit.id,i]))};
  }
  return text;
}

/** The span of a citation in its chapter, or the reason it has none. */
function span(text,cite){
  if(!cite||typeof cite!=='object'||!cite.c||!Array.isArray(cite.u))return {issue:'not a citation'};
  const chapter=text[cite.c];if(!chapter)return {issue:`no chapter ${cite.c}`};
  const a=chapter.at.get(cite.u[0]),b=chapter.at.get(cite.u[1]);
  if(a===undefined||b===undefined)return {issue:`units ${cite.u.join('..')} are not in ${cite.c}`};
  if(a>b)return {issue:`units of ${cite.c} out of order`};
  if(cite.q){const joined=chapter.units.slice(a,b+1).map(unit=>unit.text).join(' ');if(!joined.includes(fold(cite.q)))return {issue:`"${cite.q}" is not at ${cite.c} ${cite.u.join('..')}`};}
  return {c:cite.c,a,b};
}

/** Every citation inside a value, with the path that leads to it. */
function citations(value,path,out=[]){
  if(Array.isArray(value)){value.forEach((item,i)=>citations(item,`${path}[${i}]`,out));return out;}
  if(!value||typeof value!=='object')return out;
  if(typeof value.w==='string'&&Array.isArray(value.u)&&value.c){out.push([path,value]);return out;}
  for(const [key,item] of Object.entries(value))citations(item,`${path}.${key}`,out);
  return out;
}

/** The voice at a unit of a chapter: an embedded voice where the unit lies inside one, the primary voice otherwise. */
function voiceAt(text,voices,c,i){
  let found=null;
  for(const segment of voices[c]??[]){
    const a=text[c]?.at.get(segment.u?.[0]),b=text[c]?.at.get(segment.u?.[1]);
    if(a===undefined||b===undefined||i<a||i>b)continue;
    if(segment.layer==='embedded'||!found)found=segment;
  }
  return found;
}

const overlaps=(s,place)=>s&&place&&s.c===place.c&&s.a<=place.i&&place.i<=s.b;
const verbText=claim=>[claim.verb?.ru,claim.verb?.en,typeof claim.verb==='string'?claim.verb:null].filter(Boolean).join(' | ');

/** Problems of a core against the witnesses' chapters and the Tree's registries. */
export function coreIssues(loaded,chapters,registry=null){
  const problems=[],text=coreText(chapters);
  const {core,voices,concepts,figures,links,chains,proposed,control,occurrences}=loaded;
  if(core?.schema!==CORE_SCHEMA)problems.push(`schema is not ${CORE_SCHEMA}`);
  const place=(label,cite)=>{const s=span(text,cite);if(s.issue)problems.push(`${label}: ${s.issue}`);return s.issue?null:s;};

  // The voice map: every German unit of a chapter in exactly one primary segment.
  for(const [c,segments] of Object.entries(voices)){
    const chapter=text[c];if(!chapter){problems.push(`voices: no chapter ${c}`);continue;}
    const cover=new Array(chapter.units.length).fill(0);
    segments.forEach((segment,k)=>{
      const s=place(`voices ${c}[${k}]`,{c,u:segment.u});
      if(!MODES.has(segment.mode))problems.push(`voices ${c}[${k}]: mode ${segment.mode}`);
      if(!segment.speaker)problems.push(`voices ${c}[${k}]: no speaker`);
      if(segment.layer==='embedded'&&!segment.via)problems.push(`voices ${c}[${k}]: embedded speech names no carrier`);
      if(s&&segment.layer==='primary')for(let i=s.a;i<=s.b;i++)cover[i]++;
    });
    const gaps=cover.filter(n=>n===0).length,over=cover.filter(n=>n>1).length;
    if(gaps)problems.push(`voices ${c}: ${gaps} units in no primary segment`);
    if(over)problems.push(`voices ${c}: ${over} units in more than one primary segment`);
  }
  for(const c of Object.keys(text))if(!voices[c])problems.push(`voices: chapter ${c} has no voices`);

  // Objects and the places they rest on.
  const types=new Map(),ids=new Set();
  for(const [label,list] of [['concept',concepts],['figure',figures]])for(const node of list){
    if(ids.has(node.id))problems.push(`${label} ${node.id}: duplicate id`);
    ids.add(node.id);types.set(node.id,node.type);
    if(registry&&!registry.hasType(node.type))problems.push(`${label} ${node.id}: ${node.type} is not in the Tree's entity registry`);
    const formula=(node.conception??node.essence)?.formula;
    if(!formula?.cite)problems.push(`${label} ${node.id}: no formula`);
    else if(!formula.voice?.speaker)problems.push(`${label} ${node.id}: the formula names no voice`);
    if(label==='concept'&&!node.question?.ru)problems.push(`concept ${node.id}: no question`);
    for(const [path,cite] of citations(node,`${label} ${node.id}`))place(path,cite);
  }
  for(const item of proposed)ids.add(item.id);
  for(const [id,file] of Object.entries(occurrences)){
    if(!file){problems.push(`occurrences of ${id}: missing`);continue;}
    for(const [path,cite] of citations(file.occurrences,`occurrences ${id}`))place(path,cite);
  }

  // Claims: statuses, verdicts and admission agree; every place resolves; a registry relation fits its ends.
  const claimIssues=(label,claim)=>{
    if(!VERDICTS.has(claim.assessment?.verdict))problems.push(`${label}: verdict ${claim.assessment?.verdict}`);
    if(!ADMITTED[claim.admission]?.has(claim.assessment?.verdict))problems.push(`${label}: admitted as ${claim.admission} with verdict ${claim.assessment?.verdict}`);
    if(claim.status&&!STATUS.has(claim.status))problems.push(`${label}: status ${claim.status}`);
  };
  for(const link of links){
    const label=`link ${link.id}`;
    claimIssues(label,link);
    if(!verbText(link))problems.push(`${label}: no verb`);
    if(!ids.has(link.from))problems.push(`${label}: unknown ${link.from}`);
    if(!ids.has(link.to))problems.push(`${label}: unknown ${link.to}`);
    if(!(link.grounds??[]).length)problems.push(`${label}: no grounds`);
    if(![1,2,3].includes(link.order))problems.push(`${label}: order ${link.order}`);
    for(const step of link.path??[])if(step?.id&&!ids.has(step.id))problems.push(`${label}: the path names unknown ${step.id}`);
    for(const [path,cite] of citations(link,label))place(path,cite);
    if(registry&&link.relation?.startsWith('tos.relation.')){
      const type=registry.relation(link.relation);
      if(!type)problems.push(`${label}: ${link.relation} is not in the Tree's relation registry`);
      else{
        if(!registry.within(types.get(link.from),type.domain_type_ids))problems.push(`${label}: ${link.from} is outside the domain of ${link.relation}`);
        if(!registry.within(types.get(link.to),type.range_type_ids))problems.push(`${label}: ${link.to} is outside the range of ${link.relation}`);
      }
    }
  }
  for(const chain of chains){
    const label=`chain ${chain.id}`;
    claimIssues(label,chain);
    (chain.steps??[]).forEach((step,k)=>{
      if(!step.cite)problems.push(`${label} step ${k}: no place`);
      if(!verbText(step))problems.push(`${label} step ${k}: no verb`);
      if(!VERDICTS.has(step.assessment?.verdict))problems.push(`${label} step ${k}: verdict ${step.assessment?.verdict}`);
    });
    for(const [path,cite] of citations(chain,label))place(path,cite);
  }

  if(control)problems.push(...controlIssues(control,{text,voices,concepts,figures,links,chains}));
  return problems;
}

/** Control vectors: each case's places found by their words, then what it expects and forbids. */
export function controlIssues(control,{text,voices,concepts,figures,links,chains}){
  const problems=[];
  if(control?.schema!==CONTROL_SCHEMA)return [`control: schema is not ${CONTROL_SCHEMA}`];
  const admitted=[...links.filter(link=>link.admission==='map'),
    ...chains.filter(chain=>chain.admission==='map').flatMap(chain=>(chain.steps??[]).map(step=>({...step,from:step.from,to:step.to,grounds:[step.cite,...(step.grounds??[])]})))];
  const ends=(claim,a,b)=>(claim.from===a&&claim.to===b)||(claim.from===b&&claim.to===a);
  const owner=new Map((chains??[]).flatMap(chain=>(chain.states??[]).map(state=>[state.id,chain.from])));
  const endOf=id=>owner.get(id)??id;
  const covers=(claim,place)=>citations(claim,'').some(([,cite])=>overlaps(span(text,cite),place));
  for(const item of control.cases??[]){
    const where={},label=`control ${item.id}`;
    for(const [key,spec] of Object.entries(item.places??{})){
      const chapter=text[spec.c],q=fold(spec.q);
      const i=chapter?chapter.units.findIndex(unit=>unit.text.includes(q)):-1;
      if(i<0){problems.push(`${label}: "${spec.q}" is not in ${spec.c}`);continue;}
      where[key]={c:spec.c,i};
    }
    const nodes=[...concepts.map(node=>({node,kind:'concept',formula:node.conception?.formula})),...figures.map(node=>({node,kind:'figure',formula:node.essence?.formula}))];
    for(const expect of item.expect??[]){
      if(expect.voice){
        const at=where[expect.voice];if(!at)continue;
        const voice=voiceAt(text,voices,at.c,at.i);
        if(voice?.speaker!==expect.speaker)problems.push(`${label}: the voice at ${expect.voice} is ${voice?.speaker}, not ${expect.speaker}`);
      }else if(expect.counter_voice){
        const concept=concepts.find(node=>node.id===expect.counter_voice),figure=figures.find(node=>node.id===expect.speaker);
        const held=(concept?.counter_voices??[]).some(voice=>voice.speaker===expect.speaker||voice.voice?.speaker===expect.speaker)
          ||(figure?.essence?.counter_to??[]).some(item=>item.concept===expect.counter_voice);
        if(!held)problems.push(`${label}: no position of ${expect.speaker} against ${expect.counter_voice}`);
      }else if(expect.joined){
        const [a,b]=expect.joined.map(key=>where[key]);
        if(a&&b&&!admitted.some(claim=>covers(claim,a)&&covers(claim,b))&&!chains.filter(chain=>chain.admission==='map').some(chain=>covers(chain,a)&&covers(chain,b)))
          problems.push(`${label}: no admitted claim joins ${expect.joined.join(' and ')}`);
      }else if(expect.link){
        const [a,b]=expect.link;
        if(!admitted.some(claim=>ends({from:endOf(claim.from),to:endOf(claim.to)},a,b)))problems.push(`${label}: no admitted link between ${a} and ${b}`);
      }
    }
    for(const forbid of item.forbid??[]){
      if(forbid.formula_at){
        const at=where[forbid.formula_at];if(!at)continue;
        for(const {node,kind,formula} of nodes){
          if(forbid.node&&node.id!==forbid.node)continue;
          if(forbid.kind&&kind!==forbid.kind)continue;
          if(formula?.cite&&overlaps(span(text,formula.cite),at))problems.push(`${label}: ${node.id} has its formula at ${forbid.formula_at}`);
        }
      }else if(forbid.link){
        const [a,b]=forbid.link,pattern=new RegExp(forbid.verb,'i');
        for(const claim of admitted)if(ends({from:endOf(claim.from),to:endOf(claim.to)},a,b)&&pattern.test(verbText(claim)))problems.push(`${label}: ${claim.id??claim.from} joins ${a} and ${b} as "${verbText(claim)}"`);
      }else if(forbid.claim_at){
        const at=where[forbid.claim_at],pattern=new RegExp(forbid.verb,'i');if(!at)continue;
        for(const claim of admitted)if((claim.from===forbid.node||claim.to===forbid.node)&&covers(claim,at)&&pattern.test(verbText(claim)))problems.push(`${label}: ${claim.id??claim.from} says "${verbText(claim)}" at ${forbid.claim_at}`);
      }
    }
  }
  return problems;
}

#!/usr/bin/env node
// Plant the Population bonsai from its review folder.
//
//   node bonsai/review/population/plant.mjs [--check]
//
// Reads the parsed rows (layer/agents/, kept in R2: fetch the layers first), keys.jsonl, ids.jsonl,
// core.json and queue/identity.jsonl written by resolve.mjs, the layers manifest for the digests of
// the register rows, and the bonsai's manifest for its graft. Writes bonsai/population/:
//
//   population.json   the members whose rows the authors table binds (the authors of the corpus):
//                     kind, names, the register's statuses, the reader's verdict, phases, gaps and
//                     the register rows each grew from; the redirects of merged IDs
//   chronology.json   their lives as biographical phases, each dated in the register's own words
//   identities.json   the graft map: the Tree agent each member stands for, resolved or to mint
//
// A life is planted from the member row that dates it best: both years of birth and death (or a
// span of life), else the years of activity, else one year alone, else the register's explicit
// unknown. A date the register leaves unchecked or in a form parse.py does not read stays a gap.
// --check compares the modules with what the records give.

import {existsSync,readFileSync,readdirSync,renameSync,writeFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';

const HERE=dirname(fileURLToPath(import.meta.url));
const ROOT=join(HERE,'..','..','..');
const STAND='population';
const SITE='https://treeofsophia.com/__research/layers/sha256/';
const CERTAINTIES=['exact','approximate','uncertain'];

export const KINDS={
  person:{tree_type:'tos.entity.agent',label:{ru:'лицо',en:'person'}},
  pseudonymous:{tree_type:'tos.entity.agent',label:{ru:'псевдоавтор',en:'pseudonymous author'}},
  conventional:{tree_type:'tos.entity.agent',label:{ru:'условная или традиционная авторская идентичность',en:'conventional or traditional authorial identity'}},
  'attributed-voice':{tree_type:'tos.entity.agent',label:{ru:'голос, которому традиция приписывает текст',en:'voice a tradition attributes a text to'}},
  'unnamed-hand':{tree_type:'tos.entity.agent',label:{ru:'безымянная рука',en:'unnamed hand'}},
  collective:{tree_type:'tos.entity.organization',label:{ru:'коллектив',en:'collective'}}
};

const readJsonl=text=>text.split('\n').filter(Boolean).map(line=>JSON.parse(line));
const byKey=(a,b)=>a<b?-1:a>b?1:0;
const weaker=(...values)=>CERTAINTIES[Math.max(...values.map(value=>CERTAINTIES.indexOf(value)))]??'uncertain';
const precision=value=>value?(value.replace(/^-/,'').split('-').length):0;

function inputs(review,root,files){
  const read=rel=>files?.[rel]??readFileSync(join(review,rel),'utf8');
  const dir=join(review,'layer','agents');
  if(!existsSync(dir))throw new Error('plant: no layer/agents/ (fetch the layers: node bonsai/lib/layers.mjs fetch bonsai/review/population)');
  const rows=new Map();
  for(const file of readdirSync(dir).filter(name=>name.endsWith('.jsonl')).sort()){
    const batch=file.replace(/\.jsonl$/,'');
    for(const row of readJsonl(readFileSync(join(dir,file),'utf8')))rows.set(row.row,{...row,batch,record:row.row.split(':')[1]});
  }
  const layers=JSON.parse(readFileSync(join(review,'layers.json'),'utf8'));
  const digests=new Map(layers.entries.map(entry=>[entry.path,entry.sha256]));
  const manifest=JSON.parse(readFileSync(join(root,'bonsai',STAND,'bonsai.json'),'utf8'));
  const ledger=new Map();
  for(const line of readJsonl(read('ids.jsonl')))ledger.set(line.id,{...(ledger.get(line.id)??{}),...line});
  const decisions=files?.['decisions.jsonl']!==undefined||existsSync(join(review,'decisions.jsonl'))?readJsonl(read('decisions.jsonl')):[];
  return {rows,digests,manifest,ledger,decisions,keys:readJsonl(read('keys.jsonl')),core:JSON.parse(read('core.json')),queue:readJsonl(read('queue/identity.jsonl'))};
}

// Two readers' years: a year of an era as the Tree's lexeme (astronomical, four digits at least).
const lexeme=point=>{const year=point.era==='BCE'?1-point.year:point.year;return `${year<0?'-':''}${String(Math.abs(year)).padStart(4,'0')}`;};
// A year as the stand says it, in Russian and in English: ок. 428 до н. э., c. 428 BCE, 1904 (?).
const yearWords=(point,certainty)=>{
  const about=certainty==='approximate'?['ок. ','c. ']:['',''],doubt=certainty==='uncertain'?' (?)':'';
  return [`${about[0]}${point.year}${point.era==='BCE'?' до н. э.':''}${doubt}`,`${about[1]}${point.year}${point.era==='BCE'?' BCE':''}${doubt}`];
};
const said=(point,certainty,[ru,en])=>{const [r,e]=yearWords(point,certainty);return [`${ru} — ${r}`,`${en} ${e}`];};

/** The phases two readers' agreed years give, and how the stand says them. */
function readersLife(decision){
  const {birth,death,activity}=decision;
  const lifetime=birth||death?{shape:birth&&death?'closed':'open',start:birth?lexeme(birth):undefined,end:death?lexeme(death):undefined,
    certainty:weaker(...[birth,death].filter(Boolean).map(point=>point.certainty))}:null;
  const years=activity?(lexeme(activity.from)===lexeme(activity.to)
    ?{shape:'date',value:lexeme(activity.from),certainty:activity.certainty}
    :{shape:'span',start:lexeme(activity.from),end:lexeme(activity.to),certainty:activity.certainty}):null;
  const parts=[];
  if(birth)parts.push(said(birth,birth.certainty,['рождение','born']));
  if(death)parts.push(said(death,death.certainty,['смерть','died']));
  if(activity){
    const [from,to]=[yearWords(activity.from,activity.certainty),yearWords(activity.to,'exact')];
    parts.push(lexeme(activity.from)===lexeme(activity.to)?[`деятельность — ${from[0]}`,`active ${from[1]}`]
      :[`деятельность — ${from[0]}–${to[0]}`,`active ${from[1]}–${to[1]}`]);
  }
  if(decision.living)parts.push(['жив(а) по источникам читателей','living as the readers\' sources stand']);
  const statement={
    ru:`Два независимых читателя линии Популяции (решение ${decision.id}): ${parts.map(([ru])=>ru).join('; ')}.`,
    en:`Two independent readers of the Population line (decision ${decision.id}): ${parts.map(([,en])=>en).join('; ')}.`
  };
  return {lifetime,activity:years,statement};
}

/** The member's names: Latin from the first row that has one, Russian from its founding row, the original script where a row gives one. */
function names(rows){
  const latin=rows.map(row=>row.names.latin).find(Boolean);
  const original=rows.map(row=>row.names.original).find(Boolean);
  const ru=rows[0].names.ru||latin||original||rows[0].forms[0]?.text;
  const name={ru,en:latin??original??ru};
  if(original)name.original=original;
  return {name,latin:Boolean(latin)};
}

function lifetimeOf(row){
  const life=row.life??{},birth=life.birth,death=life.death;
  const point=entry=>entry?.kind==='point';
  if(point(birth)&&point(death)){
    if(birth.year>death.year)return null;
    return {shape:'closed',rank:3,start:birth.value,end:death.value,certainty:weaker(birth.certainty,death.certainty),entries:[['birth',birth],['death',death]]};
  }
  if(life.life?.kind==='span'&&!life.life.century)
    return {shape:'closed',rank:3,start:life.life.start,end:life.life.end,certainty:life.life.certainty,entries:[['life',life.life]]};
  if(point(birth))return {shape:'open',rank:1,start:birth.value,certainty:birth.certainty,entries:[['birth',birth]]};
  if(point(death))return {shape:'open',rank:1,end:death.value,certainty:death.certainty,entries:[['death',death]]};
  if((birth?.kind==='unknown'&&(!death||death.kind==='unknown'))||life.life?.kind==='unknown')
    return {shape:'unknown',rank:0,entries:[['birth',birth],['death',death],['life',life.life]].filter(([,entry])=>entry)};
  return null;
}

function activityOf(row){
  const entry=row.life?.activity;
  if(entry?.kind==='point')return {shape:'date',value:entry.value,certainty:entry.certainty,entries:[['activity',entry]]};
  if(entry?.kind==='span')return {shape:'span',start:entry.start,end:entry.end,certainty:entry.certainty,entries:[['activity',entry]]};
  return null;
}

function best(rows,read,order){
  let chosen=null;
  for(const row of rows){
    const found=read(row);
    if(found&&(!chosen||order(found,chosen.found)<0))chosen={row,found};
  }
  return chosen;
}

const lifetimeOrder=(a,b)=>b.rank-a.rank||precision(b.start)+precision(b.end)-precision(a.start)-precision(a.end)||CERTAINTIES.indexOf(a.certainty)-CERTAINTIES.indexOf(b.certainty);
const activityOrder=(a,b)=>CERTAINTIES.indexOf(a.certainty)-CERTAINTIES.indexOf(b.certainty)||(a.shape==='span'?Number(a.end)-Number(a.start):0)-(b.shape==='span'?Number(b.end)-Number(b.start):0);

const WORDS={birth:['рождение','birth'],death:['смерть','death'],life:['годы жизни','life'],activity:['деятельность','activity']};
function statement({row,found}){
  const said=found.entries.map(([kind,entry])=>[WORDS[kind],entry.text??entry.kind]);
  const inferred=found.entries.some(([,entry])=>entry.era_from);
  const ru=`Реестр ${row.batch}, строка ${row.record}: ${said.map(([[word],value])=>`${word} — «${value}»`).join('; ')}${inferred?'; эра одной даты взята из другой даты той же строки':''}.`;
  const en=`Register ${row.batch}, row ${row.record}: ${said.map(([[,word],value])=>`${word} “${value}”`).join('; ')}${inferred?'; the era of one date is taken from the other date of the same row':''}.`;
  return {statement:{ru,en},wording:found.entries.map(([,entry])=>entry.line).join('\n'),inferred};
}

function dating(found,wording){
  const base={role:'historical-time',calendar:null,year_numbering:'astronomical'};
  const source_wording={text:wording,language:'ru'};
  if(found.shape==='unknown')return {kind:'unknown-date',...base,certainty:'unknown',source_wording};
  if(found.shape==='date')return {kind:'date-assertion',...base,certainty:found.certainty,value:found.value,source_wording};
  const interval={};
  if(found.start)interval.start=found.start;
  if(found.end)interval.end=found.end;
  return {kind:'interval-assertion',...base,certainty:found.certainty,interval,source_wording};
}

/** The modules the records give, in memory: {files: {path: text}, summary}. */
export function plant({review=HERE,root=ROOT,files=null}={}){
  const {rows,digests,manifest,ledger,decisions,keys,core,queue}=inputs(review,root,files);
  // Two readers' decisions on a member's rows: a row's kind, the member's name in Latin script, its years; the latest stands.
  const decided={kind:new Map(),name:new Map(),life:new Map()};
  decisions.forEach((decision,at)=>{const map=decided[decision.type];if(map)for(const row of decision.rows??[])map.set(row,{...decision,at});});
  const latest=(map,list)=>list.map(row=>map.get(row.row)).filter(Boolean).sort((a,b)=>b.at-a.at)[0]??null;
  const memberRows=new Map(),leftOut=new Map();
  for(const line of keys){
    if(line.left_out){leftOut.set(line.row,line.left_out);continue;}
    if(!memberRows.has(line.member))memberRows.set(line.member,[]);
    memberRows.get(line.member).push(line.row);
  }
  const coreRows=new Set(core.names.flatMap(name=>Object.keys(name.rows)));
  const ambiguousRows=new Set(core.names.flatMap(name=>(name.ambiguous??[]).flatMap(item=>item.rows)));
  const queued=new Set(queue.flatMap(item=>[item.a.member,item.b.member]));
  const batches={};
  // Each batch names its register as the site's copy keeps it, so a reader can open the rows a member grew from.
  const register=batch=>{
    const path=`registry/${batch}/agents.jsonl`;
    if(!digests.has(path))throw new Error(`plant: layers.json has no digest for ${path}`);
    return `${SITE}${digests.get(path)}`;
  };
  for(const row of rows.values()){batches[row.batch]??={rows:0,core_rows:0,register:register(row.batch)};batches[row.batch].rows++;}
  const members={},situations={},lives={},sources={},agents={},organizations={};
  const treeAgents=join(root,'ToS','source-witnesses','agents');
  const planted=[...memberRows].filter(([,list])=>list.some(row=>coreRows.has(row))).sort(([a],[b])=>byKey(a,b));
  for(const [id,list] of planted){
    const entry=ledger.get(id);
    if(!entry||entry.state)throw new Error(`plant: ${id} is not a member in the ledger`);
    const ordered=[entry.founded_on,...list.filter(row=>row!==entry.founded_on)].map(row=>rows.get(row));
    if(ordered.some(row=>!row))throw new Error(`plant: ${id} names a row the layer lacks`);
    for(const row of ordered)batches[row.batch].core_rows++;
    const kind=decided.kind.get(ordered[0].row)?.kind??ordered[0].kind;
    const {name,latin}=names(ordered);
    // A name two readers gave in Latin script stands where no row gives one.
    const named=latin?null:latest(decided.name,ordered);
    if(named)name.en=named.latin;
    const gaps=new Set();
    if(!latin&&!named)gaps.add('name:latin');
    if(queued.has(id))gaps.add('identity:queue');
    if(ordered.some(row=>ambiguousRows.has(row.row)))gaps.add('core:ambiguous');
    if(ordered.some(row=>row.signals?.length&&!decided.kind.has(row.row)))gaps.add('kind:signal');
    const phases=[];
    const life=KINDS[kind].tree_type==='tos.entity.agent'?latest(decided.life,ordered):null;
    if(life){
      // Two readers' years stand in place of the registers' own: from the sources they name, else from the rows.
      const slug=entry.slug,{lifetime,activity,statement:words}=readersLife(life);
      const opened=(life.sources??[]).find(source=>/^https:\/\//.test(source));
      const source=`line-${life.id}`;
      sources[source]={short:`Линия Популяции, ${life.id}`,title:`Two independent readers of the Population line (${life.by}) agreed on these years${life.sources?.length?`, from: ${life.sources.join('; ')}`:', from the register rows'}`,
        url:opened??register(ordered[0].batch)};
      const note=life.readers?.[0]?.note??words.en;
      const wording=(found)=>({...dating(found,note),source_wording:{text:note,language:/[а-яё]/i.test(note)?'ru':'en'}});
      const add=(suffix,found,label)=>{
        const key=`tos.biographical-phase.${slug}.${suffix}`;
        situations[key]={type:'biographical-phase',subject:id,label,dating:wording(found),statement:words,sources:[source],footing:opened?'medium':'low',epistemic:'reported'};
        phases.push(key);
        return key;
      };
      const lifeKey=lifetime?add('lifetime',lifetime,{ru:`${name.ru}: годы жизни`,en:`${name.en}: lifetime`}):null;
      const activityKey=activity&&lifetime?.shape!=='closed'?add('activity',activity,{ru:`${name.ru}: годы деятельности`,en:`${name.en}: years of activity`}):null;
      if(lifetime?.shape==='closed')lives[id]=lifeKey;
      else if(activityKey)lives[id]=activityKey;
    }
    else if(KINDS[kind].tree_type==='tos.entity.agent'){
      const life=best(ordered,lifetimeOf,lifetimeOrder),activity=best(ordered,activityOf,activityOrder);
      const slug=entry.slug;
      const add=(suffix,chosen,label)=>{
        const key=`tos.biographical-phase.${slug}.${suffix}`;
        const {statement:said,wording,inferred}=statement(chosen);
        const source=`register-${chosen.row.batch.toLowerCase()}`;
        const path=`registry/${chosen.row.batch}/agents.jsonl`;
        if(!digests.has(path))throw new Error(`plant: layers.json has no digest for ${path}`);
        sources[source]={short:`Реестр ${chosen.row.batch}`,title:`ToS Identity register ${chosen.row.batch}, sheet agents: the rows as the workbook holds them (bonsai/review/population/${path})`,url:`${SITE}${digests.get(path)}`};
        situations[key]={type:'biographical-phase',subject:id,label,dating:dating(chosen.found,wording),statement:said,sources:[source],footing:'low',epistemic:inferred?'inferred':'reported'};
        phases.push(key);
        return key;
      };
      let lifeKey=null,activityKey=null;
      if(life&&(life.found.shape!=='unknown'||!activity))
        lifeKey=add('lifetime',life,{ru:`${name.ru}: годы жизни`,en:`${name.en}: lifetime`});
      if(activity&&life?.found.shape!=='closed')
        activityKey=add('activity',activity,{ru:`${name.ru}: годы деятельности`,en:`${name.en}: years of activity`});
      if(life?.found.shape==='closed')lives[id]=lifeKey;
      else if(activityKey)lives[id]=activityKey;
      else if(life?.found.shape==='unknown')lives[id]=lifeKey;
      if(!life&&!activity){
        const row=ordered[0];
        const codes=row.gaps.filter(gap=>gap.startsWith('life:')).map(gap=>gap.split(':').pop());
        gaps.add(codes.includes('unchecked')?'life:unchecked':codes.length?'life:unread':'life:none');
      }
      const year=lexeme=>Number.parseInt(lexeme.match(/^-?\d{4}/)[0],10);
      const closed=ordered.map(lifetimeOf).filter(found=>found?.shape==='closed').map(found=>`${year(found.start)}|${year(found.end)}`);
      if(new Set(closed).size>1)gaps.add('life:rows-differ');
    }
    const statuses=[...new Set(ordered.map(row=>row.status).filter(status=>['verified','provisional','disputed'].includes(status)))].sort();
    const roles=[...new Set(ordered.map(row=>row.role).filter(Boolean))].sort();
    members[id]={kind,name,core:true,verdict:'undecided',register:{statuses:statuses.length?statuses:['provisional'],roles},phases,gaps:[...gaps].sort(),grew_from:[...list].sort()};
    const tree=KINDS[kind].tree_type==='tos.entity.organization'?`tos.organization.${entry.slug}`:`tos.agent.${entry.slug}`;
    const resolved=Boolean(entry.tree)&&existsSync(join(treeAgents,entry.slug,'agent.json'));
    (KINDS[kind].tree_type==='tos.entity.organization'?organizations:agents)[id]={tree,status:resolved?'resolved':'to-mint'};
  }
  const redirects={};
  for(const [id,entry] of [...ledger].sort(([a],[b])=>byKey(a,b))){
    if(entry.state!=='redirect')continue;
    let to=entry.to;
    while(ledger.get(to)?.state==='redirect')to=ledger.get(to).to;
    if(members[to])redirects[id]=to;
  }
  const date=manifest.graft.grafted;
  const population={
    schema:'tos.bonsai.population.v1',
    bonsai:STAND,
    posture:{
      ru:'Ядро популяции: авторы философского корпуса Древа по реестрам персон и произведений. Каждый член вырос из строк реестров, которые соединила склейка идентичностей; его вид, имена, годы и пробелы — слова реестров или решения двух читателей линии, сверенные с источниками. Вердикта о приёме в Древо пока нет. Всё здесь — предложение стенда.',
      en:'The core of a population: the authors of the Tree\'s philosophical corpus, from the registers of persons and works. Each member grew from the register rows the identity resolution joined; its kind, names, years and gaps are the registers\' words or the decisions of two readers of the line, checked against sources. No verdict admits it to the Tree yet. Everything here is the stand\'s proposal.'
    },
    grew_from:{provenance:'bonsai/review/population/provenance.json',keys:'bonsai/review/population/keys.jsonl',ledger:'bonsai/review/population/ids.jsonl',core:'bonsai/review/population/core.json'},
    kinds:KINDS,
    batches:Object.fromEntries(Object.entries(batches).sort(([a],[b])=>byKey(a,b))),
    members,
    redirects
  };
  const chronology={
    schema:'tos.bonsai.chronology.v1',
    bonsai:STAND,
    checked:date,
    posture:{
      ru:'Годы жизни и деятельности авторов корпуса: значения реестров персон в формах, которые разбор читает механически, с исходной строкой реестра, и годы, которые дали два читателя линии. Основание низкое: реестр — непроверенное исследование; у годов читателей с источником по ссылке оно среднее.',
      en:'The years of life and activity of the corpus authors: the persons registers\' values in forms the parser reads mechanically, with the register\'s own line, and the years two readers of the line gave. The footing is low, a register being unreviewed research; it is medium for readers\' years with a linked source.'
    },
    numbering:{year_numbering:'astronomical',calendar:null,note:{ru:'Годы астрономические (1 г. до н. э. = 0); календарь не утверждается — даты взяты, как их пишет реестр.',en:'Years are astronomical (1 BCE = 0); no calendar is asserted: dates are taken as the register writes them.'}},
    sources:Object.fromEntries(Object.entries(sources).sort(([a],[b])=>byKey(a,b))),
    lives:Object.fromEntries(Object.entries(lives).sort(([a],[b])=>byKey(a,b))),
    compositions:{works:{},passages:{}},
    situations:Object.fromEntries(Object.entries(situations).sort(([a],[b])=>byKey(a,b)))
  };
  const identities={
    schema:'tos.bonsai.identities.v1',
    bonsai:STAND,
    checked:{tree_revision:manifest.graft.tree_revision,date,basis:{ru:'Записи ToS/source-witnesses на этой ревизии: «resolved» — Древо уже держит эту идентичность и член назван ею, «to-mint» — её пришлось бы создать маршрутом владельца источника.',en:'ToS/source-witnesses records at this revision: resolved means the Tree already holds the identity and the member is named by it; to-mint means its source-owner route would have to create it.'}},
    works:{},agents,organizations,places:{},situations:{}
  };
  const text=value=>JSON.stringify(value,null,1)+'\n';
  const out={'population.json':text(population),'chronology.json':text(chronology),'identities.json':text(identities)};
  const shapes={};
  for(const situation of Object.values(situations))shapes[situation.dating.kind]=(shapes[situation.dating.kind]??0)+1;
  const gapCounts={};
  for(const member of Object.values(members))for(const gap of member.gaps)gapCounts[gap]=(gapCounts[gap]??0)+1;
  return {files:out,summary:{members:planted.length,lives:Object.keys(lives).length,situations:Object.keys(situations).length,datings:shapes,gaps:gapCounts,redirects:Object.keys(redirects).length,
    resolved:Object.values(agents).filter(entry=>entry.status==='resolved').length,left_out:leftOut.size}};
}

function main(){
  const check=process.argv.includes('--check');
  const {files,summary}=plant();
  const differ=[];
  for(const [name,text] of Object.entries(files)){
    const path=join(ROOT,'bonsai',STAND,name);
    if(check){if(!existsSync(path)||readFileSync(path,'utf8')!==text)differ.push(name);continue;}
    writeFileSync(`${path}.tmp`,text);
    renameSync(`${path}.tmp`,path);
  }
  if(check){
    for(const name of Object.keys(files))console.log(`${name}: ${differ.includes(name)?'differs from':'matches'} what the records give`);
    process.exit(differ.length?1:0);
  }
  console.log(JSON.stringify(summary,null,1));
}

if(process.argv[1]===fileURLToPath(import.meta.url))main();

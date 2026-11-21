#!/usr/bin/env node
// The reading line of the Population bonsai.
//
//   node bonsai/review/population/line/kit.mjs queues [--check]    write queue/{kind,dates,latin,living}.jsonl and print every queue's size
//   node bonsai/review/population/line/kit.mjs packets [--type T] [--size 20]
//                                                                   cut the questions of a queue (identity by default; kind, latin,
//                                                                   dates or living) no packet holds yet into new packets
//   node bonsai/review/population/line/kit.mjs show <packet> [<item>]   the questions as a reader reads them
//   node bonsai/review/population/line/kit.mjs gate <packet> [--reader N]   0: open · 1: problems in the verdicts · 3: a fault of the line;
//                                                                   with --reader, only that reader's problems
//   node bonsai/review/population/line/kit.mjs accept <packet> --by <who>   append the verdicts both readers agree on to decisions.jsonl
//   node bonsai/review/population/line/kit.mjs status
//
// A packet (line/packets/p-NNNN/) holds questions of one type with the evidence of the register rows they are
// about (items.jsonl, rebuilt by `packets` and not tracked). An identity question asks whether two members are one
// bearer; a member's question asks its kind, its name in Latin script, or its years and whether it lives. Two
// readers answer each question in reader-1.jsonl and reader-2.jsonl; the gate checks every verdict against the
// items; accept writes the agreed ones as decisions, which resolve.mjs and plant.mjs read on their next run.
// Readers never write decisions.

import {createHash} from 'node:crypto';
import {appendFileSync,existsSync,mkdirSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {fold} from '../keys.mjs';
import {yearTolerance} from '../resolve.mjs';

const LINE=dirname(fileURLToPath(import.meta.url));
const REVIEW=join(LINE,'..');
const PACKETS=join(LINE,'packets');
const ROOT=join(REVIEW,'..','..','..');
export const TYPES=['identity','kind','latin','dates','living'];
export const VERDICTS=['same','different','related','undecidable'];
export const RELATIONS=['pseudonym_of','attributed_to','conventional_for','part_of','namesake_of'];
export const KINDS=['person','pseudonymous','conventional','attributed-voice','unnamed-hand','collective'];
export const CERTAINTIES=['exact','approximate','uncertain'];
export const ERAS=['BCE','CE'];
const AGENT_ID=/\b((?:A\d{2}|T[23]-\d{2})-A\d{3,4})\b/g;
const WORKS_SHOWN=12;
// A name in Latin script: letters of the Latin script with their marks, spaces and the punctuation of names.
const LATIN=/^[\p{Script=Latin}\p{M}][\p{Script=Latin}\p{M}\s'’ʿʾ.,()-]*$/u;
export const BASES=['name','dates','works','authority','teacher','place','office','text','register'];
const FIELDS=['preferred_label','label_ru','variant_labels','life_dates','external_identifiers','disambiguation_note','responsibility_summary','notes','work_inventory_scope','source_refs','evidence_notes'];
const AUTHORITY={
  orcid:value=>/^\d{4}-\d{4}-\d{4}-\d{3}[\dX]$/.test(value)&&mod11(value.replace(/-/g,'')),
  isni:value=>/^\d{15}[\dX]$/.test(value)&&mod11(value),
  viaf:value=>/^\d{2,22}$/.test(value),
  gnd:value=>/^(?:\d{6,10}[\dX]|\d{1,9}-[\dX])$/.test(value),
  lccn:value=>/^n[a-z]{0,2}\d{6,10}$/.test(value),
  wikidata:value=>/^Q\d+$/.test(value),
  bnf:value=>/^cb\d{8}[0-9bcdfghjkmnpqrstvwxz]$/.test(value),
  dila:value=>/^A\d{6}$/.test(value)
};

function mod11(digits){
  let total=0;
  for(const ch of digits.slice(0,-1))total=(total+Number(ch))*2;
  const check=(12-total%11)%11;
  return digits.at(-1)===(check===10?'X':String(check));
}
const readJsonl=path=>existsSync(path)?readFileSync(path,'utf8').split('\n').filter(Boolean).map(line=>JSON.parse(line)):[];
const writeJsonl=(path,items)=>{mkdirSync(dirname(path),{recursive:true});writeFileSync(path,items.map(item=>JSON.stringify(item)+'\n').join(''));};
const sha=value=>createHash('sha256').update(typeof value==='string'?value:JSON.stringify(value)).digest('hex');
const BASE32='abcdefghijklmnopqrstuvwxyz234567';
const base32=bytes=>{let bits=0,acc=0,out='';for(const byte of bytes){acc=(acc<<8)|byte;bits+=8;while(bits>=5){out+=BASE32[(acc>>>(bits-5))&31];bits-=5;}}return out;};
const shortId=(prefix,value)=>`${prefix}-${base32(createHash('sha256').update(value).digest()).slice(0,10)}`;
/** A member's question, named by its type and what it is about (the member, or the row for a kind). */
export const itemId=(type,anchor)=>`q-${base32(createHash('sha256').update(`${type}|${anchor}`).digest()).slice(0,12)}`;
const weaker=(...values)=>CERTAINTIES[Math.max(...values.map(value=>CERTAINTIES.indexOf(value??'exact')))]??'uncertain';

function layerRows(){
  const dir=join(REVIEW,'layer','agents'),registry=join(REVIEW,'registry');
  if(!existsSync(dir)||!existsSync(registry))throw Object.assign(new Error('the layers are not fetched (node bonsai/lib/layers.mjs fetch bonsai/review/population)'),{line:true});
  // The works of a batch whose responsibility lines name a row: their IDs and labels, so that a reader
  // sees titles where the row's own summary gives only work IDs.
  const works=new Map();
  for(const batch of readdirSync(registry).filter(name=>existsSync(join(registry,name,'works-responsibility.jsonl'))).sort()){
    for(const work of readJsonl(join(registry,batch,'works-responsibility.jsonl')).slice(1)){
      const label=[work.record_id,work.preferred_label,work.label_ru&&work.label_ru!==work.preferred_label?`(${work.label_ru})`:null].filter(Boolean).join(' ');
      for(const [,agent] of (work.responsibility_statements??'').matchAll(AGENT_ID)){
        const id=`${agent.replace(/-A\d+$/,'')}:${agent}`;
        if(!works.has(id))works.set(id,[]);
        if(!works.get(id).includes(label))works.get(id).push(label);
      }
    }
  }
  const rows=new Map();
  for(const file of readdirSync(dir).filter(name=>name.endsWith('.jsonl')).sort()){
    const batch=file.replace(/\.jsonl$/,'');
    const raw=new Map(readJsonl(join(registry,batch,'agents.jsonl')).slice(1).map(row=>[`${batch}:${row.record_id.trim()}`,row]));
    for(const row of readJsonl(join(dir,file)))rows.set(row.row,{parsed:row,raw:raw.get(row.row),works:works.get(row.row)??[]});
  }
  return rows;
}

function evidence(rows,id,{signals=false}={}){
  const {parsed,raw,works}=rows.get(id);
  const fields={};
  for(const field of FIELDS)if(raw?.[field])fields[field]=raw[field];
  if(works.length)fields.works=works.slice(0,WORKS_SHOWN).join('; ')+(works.length>WORKS_SHOWN?`; … ${works.length-WORKS_SHOWN} more`:'');
  // The typed identifiers the resolution compared, so that a reader sees what an id-conflict is about.
  const ids=[...(parsed.ids??[]).map(({scheme,value,valid})=>valid===false?{scheme,value,valid}:{scheme,value}),
    ...(parsed.existing??[]).filter(value=>value.startsWith('tos.agent.')).map(value=>({scheme:'tos-agent',value}))];
  const out={row:id,kind:parsed.kind,kind_basis:parsed.kind_basis,status:parsed.status,role:parsed.role,ids,fields};
  if(signals)out.signals=parsed.signals??[];
  return out;
}

function item(rows,entry){
  const body={item:entry.item,type:entry.type,reason:entry.reason,shared:entry.shared,
    a:{member:entry.a.member,rows:entry.a.rows.map(id=>evidence(rows,id))},
    b:{member:entry.b.member,rows:entry.b.rows.map(id=>evidence(rows,id))}};
  return {...body,sha:sha(body)};
}

/** A member's question with the member as the bonsai plants it now and the evidence of every row it grew from. */
function memberItem(rows,entry,population){
  const record=population.members[entry.member];
  const body={item:entry.item,type:entry.type,reason:entry.reason??null,member:entry.member,
    name:record?.name??null,kind:record?.kind??null,
    ...(entry.type==='kind'?{row:entry.row,signals:entry.signals}:{}),
    rows:entry.rows.map(id=>evidence(rows,id,{signals:entry.type==='kind'}))};
  return {...body,sha:sha(body)};
}

const packetIds=()=>existsSync(PACKETS)?readdirSync(PACKETS).filter(name=>/^p-\d{4}$/.test(name)).sort():[];
const packetFile=(packet,name)=>join(PACKETS,packet,name);
const retired=packet=>existsSync(packetFile(packet,'retired.json'));
const metaOf=packet=>JSON.parse(readFileSync(packetFile(packet,'packet.json'),'utf8'));

/** Whether a queued question is cut into a reader packet: not one a packet already holds, and never a
 *  `decision-conflict`, which readers cannot settle (accepting only adds decisions) and the operator withdraws. */
export function cutsIntoPacket(entry,held){
  return !held.has(entry.item)&&entry.reason!=='decision-conflict';
}

function packets(size,type='identity'){
  if(!TYPES.includes(type))throw new Error(`packets: type ${type} is not one of ${TYPES.join(', ')}`);
  const queue=readJsonl(join(REVIEW,'queue',`${type}.jsonl`));
  const byItem=new Map(queue.map(entry=>[entry.item,entry]));
  const rows=layerRows();
  const population=JSON.parse(readFileSync(join(ROOT,'bonsai','population','population.json'),'utf8'));
  const build=entry=>type==='identity'?item(rows,entry):memberItem(rows,entry,population);
  let next=packetIds().length?Number(packetIds().at(-1).slice(2))+1:1,made=0,gone=0;
  // A packet of this type not yet accepted whose questions left the queue or whose evidence changed (a later run
  // of resolve.mjs or plant.mjs read decisions) is retired; its questions still open are cut anew below.
  const own=packetIds().filter(packet=>(metaOf(packet).type??'identity')===type);
  for(const packet of own){
    if(existsSync(packetFile(packet,'accepted.json'))||retired(packet))continue;
    const meta=metaOf(packet);
    const items=meta.items.map(id=>byItem.get(id)).filter(Boolean).map(build);
    if(items.length<meta.items.length||items.some(entry=>entry.sha!==meta.shas[entry.item])){
      writeFileSync(packetFile(packet,'retired.json'),JSON.stringify({packet,reason:'its questions or their evidence changed since it was cut'},null,1)+'\n');
      gone++;continue;
    }
    writeJsonl(packetFile(packet,'items.jsonl'),items);
  }
  const held=new Set(own.filter(packet=>!retired(packet)).flatMap(packet=>metaOf(packet).items));
  // The identity questions about planted members come first: both sides planted, then one, then none.
  const planted=new Set(Object.keys(population.members));
  const weight=entry=>type==='identity'?2-[entry.a.member,entry.b.member].filter(member=>planted.has(member)).length:0;
  const waiting=queue.map((entry,at)=>({entry,at})).filter(({entry})=>cutsIntoPacket(entry,held))
    .sort((x,y)=>weight(x.entry)-weight(y.entry)||x.at-y.at).map(({entry})=>entry);
  for(let i=0;i<waiting.length;i+=size){
    const packet=`p-${String(next++).padStart(4,'0')}`,chunk=waiting.slice(i,i+size);
    const items=chunk.map(build);
    mkdirSync(join(PACKETS,packet),{recursive:true});
    writeFileSync(packetFile(packet,'packet.json'),JSON.stringify({packet,type,items:chunk.map(entry=>entry.item),shas:Object.fromEntries(items.map(entry=>[entry.item,entry.sha]))},null,1)+'\n');
    writeJsonl(packetFile(packet,'items.jsonl'),items);
    made++;
  }
  const operator=queue.filter(entry=>entry.reason==='decision-conflict').length;
  console.log(`packets (${type}): ${made} new, ${gone} retired, ${packetIds().length} in all; ${waiting.length} questions cut, ${held.size} already held${operator?`, ${operator} decision conflicts left to the operator`:''}`);
}

/** The texts a quote may come from: every field of every row the question holds, by side (a member's question has side a). */
function textsOf(entry){
  const out=new Map();
  const sides=entry.a?['a','b']:['a'];
  for(const side of sides)for(const row of (entry.a?entry[side].rows:entry.rows))for(const [field,text] of Object.entries(row.fields))out.set(`${side}|${row.row}|${field}`,text);
  return out;
}

const astronomical=point=>point.era==='BCE'?1-point.year:point.year;

/** What is wrong with a reader's years: each bound a year of an era with its certainty, an order the years keep. */
function datesProblems(verdict){
  const problems=[];
  const point=(value,name,{certainty=true}={})=>{
    if(value==null)return false;
    if(typeof value!=='object'||!Number.isInteger(value.year)||value.year<1||!ERAS.includes(value.era)){problems.push(`${name} must be {"year": a whole number from 1, "era": "BCE" or "CE"${certainty?', "certainty": exact, approximate or uncertain':''}}`);return false;}
    if(certainty&&!CERTAINTIES.includes(value.certainty)){problems.push(`${name}: certainty must be one of ${CERTAINTIES.join(', ')}`);return false;}
    return true;
  };
  const birth=point(verdict.birth,'birth'),death=point(verdict.death,'death');
  let from=false,to=false;
  if(verdict.activity!=null){
    if(typeof verdict.activity!=='object'||!CERTAINTIES.includes(verdict.activity.certainty))problems.push(`activity must be {"from": {year, era}, "to": {year, era}, "certainty": ${CERTAINTIES.join(', ')}}`);
    else{from=point(verdict.activity.from,'activity.from',{certainty:false});to=point(verdict.activity.to,'activity.to',{certainty:false});}
  }
  if(verdict.living!=null&&typeof verdict.living!=='boolean')problems.push('living must be true or false');
  if(verdict.birth==null&&verdict.death==null&&verdict.activity==null&&!verdict.living)problems.push('the years give nothing: name a birth, a death, years of activity, or that the person lives');
  if(birth&&death&&astronomical(verdict.birth)>astronomical(verdict.death))problems.push('the birth comes after the death');
  if(birth&&death&&astronomical(verdict.death)-astronomical(verdict.birth)>130)problems.push('a life of more than 130 years');
  if(from&&to&&astronomical(verdict.activity.from)>astronomical(verdict.activity.to))problems.push('the activity ends before it begins');
  if(verdict.living){
    if(verdict.death!=null)problems.push('a living person has no death');
    if(!birth)problems.push('a living person needs the birth');
    else if(astronomical(verdict.birth)<1880)problems.push('a living person born before 1880');
  }
  return problems;
}

/** The problems of one verdict against its question, without the reader's prefix; an empty list lets it pass. */
export function verdictProblems(entry,verdict){
  const problems=[];
  const type=entry.type??'identity',open=verdict.verdict==='undecidable';
  if(verdict.sha!==entry.sha)problems.push('sha does not match the question');
  const quotes=verdict.quotes??[],sources=verdict.sources??[];
  const basis=verdict.basis??[];
  const needBasis=()=>{if(!open&&(!basis.length||basis.some(item=>!BASES.includes(item))))problems.push(`basis must name what decided it, from ${BASES.join(', ')}`);};
  if(type==='identity'){
    if(!VERDICTS.includes(verdict.verdict))problems.push(`verdict ${verdict.verdict} is not one of ${VERDICTS.join(', ')}`);
    if(verdict.verdict==='related'?!RELATIONS.includes(verdict.relation):verdict.relation!=null)problems.push(`relation ${verdict.relation} does not fit the verdict`);
    needBasis();
    if(verdict.verdict==='same'&&!basis.some(item=>item!=='name'))problems.push('same needs a basis beyond the name');
    if(!open&&!quotes.length)problems.push('no quote of the rows');
  }else if(type==='kind'){
    if(![...KINDS,'undecidable'].includes(verdict.verdict))problems.push(`verdict ${verdict.verdict} is not a kind (${KINDS.join(', ')}) or undecidable`);
    needBasis();
    if(!open&&!quotes.length)problems.push('no quote of the rows');
  }else if(type==='latin'){
    if(!['latin','undecidable'].includes(verdict.verdict))problems.push(`verdict ${verdict.verdict} is not latin or undecidable`);
    if(verdict.verdict==='latin'){
      const name=typeof verdict.latin==='string'?verdict.latin.trim():'';
      if(!LATIN.test(name)||[...name.replace(/[^\p{L}]/gu,'')].length<2)problems.push('latin must be the name in Latin script');
      if(/[︠͠͡-︣]/u.test(name))problems.push('write ALA-LC without its ligature ties: Kuznetsova, not Kuznet͡sova');
      if(/(^|[\s'(-])(?!IJ)\p{Lu}{2}\p{Ll}/u.test(name))problems.push('capitalize a word by its first letter only: Iakov, not IAkov');
      if(name.includes('’'))problems.push("write the apostrophe as ' (a soft sign, an elision, an aspiration): Il'in, not Il’in");
      needBasis();
      if(!quotes.length&&!sources.length)problems.push('a Latin name needs a quote of the rows or a source you opened');
    }
  }else{
    if(!['dates','undecidable'].includes(verdict.verdict))problems.push(`verdict ${verdict.verdict} is not dates or undecidable`);
    if(verdict.verdict==='dates'){
      problems.push(...datesProblems(verdict));
      needBasis();
      if(!quotes.length&&!sources.length)problems.push('years need a quote of the rows or a source you opened');
      // Whether a person lives is asked of the world, not of the rows, and only by a question of its own: the
      // answer names what the reader opened.
      if(type==='living'){if(!sources.length)problems.push('a living question needs in sources the newest dated source you opened about the person');}
      else if(verdict.living)problems.push('a dates question leaves living out: whether a person lives is a question of its own');
    }
  }
  const texts=textsOf(entry);
  for(const quote of quotes){
    const side=quote.side??'a',text=texts.get(`${side}|${quote.row}|${quote.field}`);
    if(text===undefined)problems.push(`quotes ${entry.a?`${side} `:''}${quote.row} ${quote.field}, which the question does not hold`);
    else if(typeof quote.text!=='string'||!text.includes(quote.text))problems.push(`the quote from ${quote.row} ${quote.field} is not its exact words`);
    else if([...quote.text.trim()].length<4)problems.push(`the quote from ${quote.row} ${quote.field} is shorter than four characters; quote more of the field or another field`);
  }
  for(const found of verdict.authority??[]){
    const valid=AUTHORITY[found.scheme];
    if(!valid)problems.push(`authority scheme ${found.scheme} is not one of ${Object.keys(AUTHORITY).join(', ')}`);
    else if(!valid(String(found.value)))problems.push(`${found.scheme} ${found.value} is not a valid identifier`);
    if(entry.a&&!['a','b'].includes(found.side))problems.push('an authority names side a or b');
  }
  if(verdict.sources!=null&&(!Array.isArray(verdict.sources)||verdict.sources.some(source=>typeof source!=='string'||source.trim().length<4)))problems.push('sources must list the URLs or citations you opened');
  if(typeof verdict.note!=='string'||!verdict.note.trim())problems.push('no note');
  return problems;
}

const meet=(p,q,certainty)=>{
  if(!p&&!q)return true;
  if(!p||!q)return false;
  const a=astronomical(p),b=astronomical(q);
  return Math.abs(a-b)<=yearTolerance(Math.min(a,b),weaker(certainty?.[0]??p.certainty,certainty?.[1]??q.certainty));
};
// Two readers' spans of activity are one answer when their bounds meet, or when together they reach no wider than the
// wider of them and a working life: one span holds the other, they overlap, or each reader found a stretch of one career.
const WORKING_LIFE=40;
const spanOf=activity=>[astronomical(activity.from),astronomical(activity.to)];
const boundsMeet=(one,two)=>meet(one.from,two.from,[one.certainty,two.certainty])&&meet(one.to,two.to,[one.certainty,two.certainty]);
function activityAgrees(one,two){
  if(!one||!two)return !one&&!two;
  if(boundsMeet(one,two))return true;
  const [a,b]=[spanOf(one),spanOf(two)];
  return Math.max(a[1],b[1])-Math.min(a[0],b[0])<=Math.max(a[1]-a[0],b[1]-b[0])+WORKING_LIFE;
}
/** Whether two readers' years are one answer: each bound present in both or in neither, within the resolution's tolerance. */
function datesAgree(one,two){
  return meet(one.birth,two.birth)&&meet(one.death,two.death)&&activityAgrees(one.activity,two.activity)&&Boolean(one.living)===Boolean(two.living);
}
// The answer of two agreeing readers: the first reader's years, at the weaker certainty, approximate where the years differ.
const settle=(p,q)=>p?{year:p.year,era:p.era,certainty:astronomical(p)===astronomical(q)?weaker(p.certainty,q.certainty):weaker(p.certainty,q.certainty,'approximate')}:null;
// Their activity: the first reader's span where the bounds meet, else the span that covers both, approximate where they differ.
function settleActivity(one,two){
  if(!one)return null;
  const same=astronomical(one.from)===astronomical(two.from)&&astronomical(one.to)===astronomical(two.to);
  const certainty=same?weaker(one.certainty,two.certainty):weaker(one.certainty,two.certainty,'approximate');
  const [from,to]=boundsMeet(one,two)?[one.from,one.to]
    :[astronomical(one.from)<=astronomical(two.from)?one.from:two.from,astronomical(one.to)>=astronomical(two.to)?one.to:two.to];
  return {from:{year:from.year,era:from.era},to:{year:to.year,era:to.era},certainty};
}
// Of two Latin forms that fold alike, the one that keeps more of the name's marks.
const marks=text=>[...text.normalize('NFD')].filter(ch=>/\p{M}/u.test(ch)).length;

/** Whether two readers agree on a question, and on what: {agreed, verdict, relation, value}. */
export function agreement(entry,one,two){
  const type=entry.type??'identity';
  if(type==='identity'){
    let conflict=false;
    for(const side of ['a','b']){
      const schemes=new Map();
      for(const found of [...(one.authority??[]),...(two.authority??[])])if(found.side===side){if(!schemes.has(found.scheme))schemes.set(found.scheme,new Set());schemes.get(found.scheme).add(String(found.value));}
      for(const values of schemes.values())if(values.size>1)conflict=true;
    }
    // `different` and `related` as `namesake_of` both say two bearers; a split between them is no dispute.
    const bearers=verdict=>verdict.verdict==='different'||(verdict.verdict==='related'&&verdict.relation==='namesake_of');
    const exact=one.verdict===two.verdict&&(one.relation??null)===(two.relation??null);
    const agreed=(exact||(bearers(one)&&bearers(two)))&&!conflict;
    return {agreed,verdict:agreed?(exact?one.verdict:'different'):null,relation:agreed&&exact?one.relation??null:null};
  }
  if(one.verdict==='undecidable'&&two.verdict==='undecidable')return {agreed:true,verdict:'undecidable',relation:null};
  if(type==='kind'){const agreed=one.verdict===two.verdict;return {agreed,verdict:agreed?one.verdict:null,relation:null,value:agreed?{kind:one.verdict}:null};}
  if(type==='latin'){
    const agreed=one.verdict==='latin'&&two.verdict==='latin'&&fold(one.latin)===fold(two.latin);
    const latin=agreed?(marks(two.latin)>marks(one.latin)?two.latin:one.latin).trim():null;
    return {agreed,verdict:agreed?'latin':null,relation:null,value:agreed?{latin}:null};
  }
  const agreed=one.verdict==='dates'&&two.verdict==='dates'&&datesAgree(one,two);
  if(!agreed)return {agreed:false,verdict:null,relation:null};
  return {agreed:true,verdict:'dates',relation:null,value:{birth:settle(one.birth,two.birth),death:settle(one.death,two.death),activity:settleActivity(one.activity,two.activity),living:Boolean(one.living)}};
}

/** Problems of one packet's verdicts; `fault` marks a problem of the line rather than of a reader. */
export function gatePacket(packet){
  const problems=[],faults=[];
  if(retired(packet))return {problems,faults:[`${packet} is retired; its open questions are in newer packets`],agreed:[]};
  if(!existsSync(packetFile(packet,'packet.json'))||!existsSync(packetFile(packet,'items.jsonl')))return {problems,faults:['packet.json or items.jsonl is missing (run kit.mjs packets)'],agreed:[]};
  const meta=metaOf(packet);
  const items=new Map(readJsonl(packetFile(packet,'items.jsonl')).map(entry=>[entry.item,entry]));
  for(const id of meta.items){
    const entry=items.get(id);
    if(!entry)faults.push(`${id}: no longer in the queue; the packet is stale`);
    else if(entry.sha!==meta.shas[id])faults.push(`${id}: its evidence changed since the packet was cut`);
  }
  const answers=[];
  for(const reader of [1,2]){
    const path=packetFile(packet,`reader-${reader}.jsonl`);
    if(!existsSync(path)){problems.push(`reader-${reader}.jsonl is missing`);continue;}
    let lines;
    try{lines=readJsonl(path);}catch(error){problems.push(`reader-${reader}.jsonl: ${error.message}`);continue;}
    const seen=new Set();
    for(const verdict of lines){
      const at=`reader-${reader} ${verdict.item}`;
      const entry=items.get(verdict.item);
      if(!entry){problems.push(`${at}: not a question of this packet`);continue;}
      if(seen.has(verdict.item))problems.push(`${at}: answered twice`);
      seen.add(verdict.item);
      for(const problem of verdictProblems(entry,verdict))problems.push(`${at}: ${problem}`);
      answers.push({reader,verdict});
    }
    for(const id of meta.items)if(!seen.has(id))problems.push(`reader-${reader}: ${id} is not answered`);
  }
  const agreed=[];
  if(!problems.length&&!faults.length){
    for(const id of meta.items){
      const [one,two]=[1,2].map(reader=>answers.find(answer=>answer.reader===reader&&answer.verdict.item===id).verdict);
      agreed.push({item:id,...agreement(items.get(id),one,two),readers:[one,two]});
    }
  }
  return {problems,faults,agreed};
}

// What a decision answers for a member's question: the decision type an agreed verdict makes.
const DECIDES={kind:'kind',latin:'name',dates:'life',living:'life'};

function accept(packet,by){
  const {problems,faults,agreed}=gatePacket(packet);
  if(problems.length||faults.length)throw Object.assign(new Error(`${packet}: the gate is shut`),{line:faults.length>0});
  if(existsSync(packetFile(packet,'accepted.json')))throw new Error(`${packet}: already accepted`);
  const items=new Map(readJsonl(packetFile(packet,'items.jsonl')).map(entry=>[entry.item,entry]));
  const known=new Set(readJsonl(join(REVIEW,'decisions.jsonl')).map(decision=>decision.id));
  const decisions=[],held=[];
  for(const answer of agreed){
    const entry=items.get(answer.item),type=entry.type??'identity';
    const open=!answer.agreed||answer.verdict==='undecidable';
    let decision;
    if(type==='identity'){
      const rows=[entry.a.rows[0].row,entry.b.rows[0].row];
      const kind=open?'held':answer.verdict;
      decision={id:shortId('d',`${kind}|${rows.join('|')}`),type:kind,rows,item:answer.item,packet,by,readers:answer.readers.map(verdict=>({verdict:verdict.verdict,relation:verdict.relation??null,basis:verdict.basis,note:verdict.note}))};
      if(kind==='related')decision.relation=answer.relation;
    }else{
      // A member's decision names the rows it is about: the row of a kind, else every row the member grew from.
      const rows=type==='kind'?[entry.row]:entry.rows.map(row=>row.row);
      const kind=open?'held':DECIDES[type];
      const sources=[...new Set(answer.readers.flatMap(verdict=>verdict.sources??[]))];
      decision={id:shortId('d',`${kind}|${answer.item}`),type:kind,question:type,rows,item:answer.item,packet,by,
        ...(open?{}:answer.value),...(sources.length&&!open?{sources}:{}),
        readers:answer.readers.map(verdict=>({verdict:verdict.verdict,basis:verdict.basis??[],note:verdict.note,...(verdict.sources?.length?{sources:verdict.sources}:{})}))};
    }
    if(!answer.agreed)decision.reason='readers disagree';
    if(known.has(decision.id))continue;
    known.add(decision.id);
    decisions.push(decision);
    if(decision.type==='held')held.push(answer.item);
  }
  appendFileSync(join(REVIEW,'decisions.jsonl'),decisions.map(decision=>JSON.stringify(decision)+'\n').join(''));
  writeFileSync(packetFile(packet,'accepted.json'),JSON.stringify({packet,by,decisions:decisions.map(decision=>decision.id),held},null,1)+'\n');
  console.log(`${packet}: ${decisions.length} decisions, ${held.length} held for another reader`);
}

function queues({check=false}={}){
  const rows=layerRows();
  const population=JSON.parse(readFileSync(join(ROOT,'bonsai','population','population.json'),'utf8'));
  const decisions=readJsonl(join(REVIEW,'decisions.jsonl'));
  // A question two readers answered, or both left open, is not asked again; the years two readers gave answer a
  // member's dates, and a kind they gave answers its row. Whether a member lives is asked while its life is open (a
  // birth from 1900 on and no death, in the latest years readers gave or else in its rows) until a living question
  // answers it.
  const answered=new Set(decisions.map(decision=>decision.item).filter(Boolean));
  const lives=decisions.filter(decision=>decision.type==='life');
  const lived=new Set(lives.flatMap(decision=>decision.rows));
  const lifeOf=new Map();
  for(const [at,decision] of lives.entries())for(const row of decision.rows)lifeOf.set(row,{at,decision});
  const kinded=new Set(decisions.filter(decision=>decision.type==='kind').flatMap(decision=>decision.rows));
  const kind=[],dates=[],latin=[],living=[];
  const ask=(list,entry)=>{if(!answered.has(entry.item))list.push(entry);};
  for(const [member,record] of Object.entries(population.members)){
    if(record.gaps.includes('name:latin'))ask(latin,{item:itemId('latin',member),type:'latin',member,rows:record.grew_from,name:record.name});
    const datedRows=record.grew_from.map(id=>rows.get(id).parsed);
    const decided=record.grew_from.some(row=>lived.has(row));
    if(!decided&&(!record.phases.length||record.gaps.includes('life:rows-differ')))
      ask(dates,{item:itemId('dates',member),type:'dates',member,reason:record.phases.length?'rows-differ':record.gaps.find(gap=>gap.startsWith('life:'))??'life:none',rows:record.grew_from,gaps:[...new Set(datedRows.flatMap(row=>row.gaps.filter(gap=>gap.startsWith('life:'))))].sort()});
    const life=record.grew_from.map(row=>lifeOf.get(row)).filter(Boolean).sort((a,b)=>a.at-b.at).at(-1)?.decision;
    const open=life&&(life.birth||life.death)?Boolean(life.birth&&astronomical(life.birth)>=1900&&!life.death)
      :datedRows.some(row=>row.life.birth?.kind==='point'&&row.life.birth.year>=1900&&row.life.death?.kind!=='point');
    if(open&&!(life&&(life.question==='living'||life.living)))ask(living,{item:itemId('living',member),type:'living',member,rows:record.grew_from});
    for(const row of datedRows)if(row.signals.length&&!kinded.has(row.row))ask(kind,{item:itemId('kind',row.row),type:'kind',member,row:row.row,kind:row.kind,signals:row.signals,rows:record.grew_from});
  }
  if(check){
    let differs=false;
    for(const [name,list] of Object.entries({kind,dates,latin,living})){
      const path=join(REVIEW,'queue',`${name}.jsonl`);
      const same=existsSync(path)&&readFileSync(path,'utf8')===list.map(item=>JSON.stringify(item)+'\n').join('');
      differs||=!same;
      console.log(`queue/${name}.jsonl: ${same?'matches':'differs from'} what population.json and the rows give`);
    }
    if(differs)process.exit(1);
    return;
  }
  for(const [name,list] of Object.entries({kind,dates,latin,living}))writeJsonl(join(REVIEW,'queue',`${name}.jsonl`),list);
  const identity=readJsonl(join(REVIEW,'queue','identity.jsonl'));
  console.log(JSON.stringify({identity:identity.length,kind:kind.length,dates:dates.length,latin:latin.length,living:living.length}));
}

function showRow(out,row){
  const ids=(row.ids??[]).map(found=>`${found.scheme} ${found.value}${found.valid===false?' (bad check digit)':''}`);
  out.push(`    ${row.row} · ${row.kind} · ${row.status||'no status'}${ids.length?` · ${ids.join(', ')}`:''}${row.signals?.length?` · signals: ${row.signals.join(', ')}`:''}`);
  for(const [field,text] of Object.entries(row.fields))out.push(`      ${field}: ${text.replace(/\s*\n\s*/g,' ⏎ ')}`);
}

/** The questions of a packet as a reader reads them, with the sha each verdict copies. */
function show(packet,only){
  const items=readJsonl(packetFile(packet,'items.jsonl'));
  if(!items.length)throw Object.assign(new Error(`${packet}: no questions (run kit.mjs packets)`),{line:true});
  for(const entry of items){
    if(only&&entry.item!==only)continue;
    let out;
    if(entry.a){
      out=[`${entry.item} · identity · ${entry.reason} · shared: ${entry.shared.join(', ')}`,`  sha ${entry.sha}`];
      for(const side of ['a','b']){out.push(`  ${side} · ${entry[side].member}`);for(const row of entry[side].rows)showRow(out,row);}
    }else{
      out=[`${entry.item} · ${entry.type}${entry.reason?` · ${entry.reason}`:''}`,`  sha ${entry.sha}`,
        `  member ${entry.member} · ${[entry.name?.ru,entry.name?.en,entry.name?.original].filter(Boolean).join(' / ')} · planted as ${entry.kind}`];
      if(entry.type==='kind')out.push(`  the row in question: ${entry.row} · signals: ${entry.signals.join(', ')}`);
      out.push('  the rows it grew from:');
      for(const row of entry.rows)showRow(out,row);
    }
    console.log(`${out.join('\n')}\n`);
  }
}

function status(){
  const counts={packets:0,answered:0,open:0,accepted:0,retired:0,questions:0,types:{}};
  for(const packet of packetIds()){
    if(retired(packet)){counts.retired++;continue;}
    const meta=metaOf(packet),type=meta.type??'identity';
    counts.packets++;counts.types[type]=(counts.types[type]??0)+1;
    counts.questions+=meta.items.length;
    if(existsSync(packetFile(packet,'accepted.json')))counts.accepted++;
    else if(existsSync(packetFile(packet,'reader-1.jsonl'))&&existsSync(packetFile(packet,'reader-2.jsonl')))counts.answered++;
    else counts.open++;
  }
  const decisions=readJsonl(join(REVIEW,'decisions.jsonl'));
  counts.decisions=decisions.length;
  counts.held=decisions.filter(decision=>decision.type==='held').length;
  // A decision-conflict is two decisions that contradict each other; only the operator can withdraw one.
  counts.conflicts=readJsonl(join(REVIEW,'queue','identity.jsonl')).filter(entry=>entry.reason==='decision-conflict').length;
  console.log(JSON.stringify(counts));
}

function main([command,...args]){
  const option=name=>{const at=args.indexOf(name);return at>=0?args[at+1]:null;};
  try{
    if(command==='queues')queues({check:args.includes('--check')});
    else if(command==='packets')packets(Number(option('--size')??20),option('--type')??'identity');
    else if(command==='show'&&args[0])show(args[0],args[1]&&!args[1].startsWith('--')?args[1]:null);
    else if(command==='gate'&&args[0]){
      // A reader sees only its own problems and never whether the other reader agrees.
      const reader=option('--reader');
      const {problems,faults,agreed}=gatePacket(args[0]);
      for(const fault of faults)console.log(`line: ${fault}`);
      if(faults.length)process.exit(3);
      const shown=reader?problems.filter(problem=>[`reader-${reader} `,`reader-${reader}.`,`reader-${reader}:`].some(prefix=>problem.startsWith(prefix))):problems;
      for(const problem of shown)console.log(problem);
      if(shown.length)process.exit(1);
      console.log(reader?`gate: no problem of reader-${reader}`:`gate: open · both readers answered all ${agreed.length} questions`);
    }
    else if(command==='accept'&&args[0]&&option('--by'))accept(args[0],option('--by'));
    else if(command==='status')status();
    else{console.error(`usage: kit.mjs queues [--check] | packets [--type ${TYPES.join('|')}] [--size N] | show <packet> [<item>] | gate <packet> [--reader N] | accept <packet> --by <who> | status`);process.exit(2);}
  }catch(error){console.error(`${error.line?'line: ':''}${error.message}`);process.exit(error.line?3:1);}
}

if(process.argv[1]===fileURLToPath(import.meta.url))main(process.argv.slice(2));

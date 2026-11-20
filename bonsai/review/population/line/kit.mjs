#!/usr/bin/env node
// The reading line of the Population bonsai.
//
//   node bonsai/review/population/line/kit.mjs queues [--check]    write queue/{kind,dates,latin,living}.jsonl and print every queue's size
//   node bonsai/review/population/line/kit.mjs packets [--size 20] cut the identity questions no packet holds yet into new packets,
//                                                                   the planted members' questions first
//   node bonsai/review/population/line/kit.mjs show <packet> [<item>]   the questions as a reader reads them
//   node bonsai/review/population/line/kit.mjs gate <packet> [--reader N]   0: open · 1: problems in the verdicts · 3: a fault of the line;
//                                                                   with --reader, only that reader's problems
//   node bonsai/review/population/line/kit.mjs accept <packet> --by <who>   append the verdicts both readers agree on to decisions.jsonl
//   node bonsai/review/population/line/kit.mjs status
//
// A packet (line/packets/p-NNNN/) holds identity questions with the evidence of every register row on
// both sides (items.jsonl, rebuilt by `packets` and not tracked). Two readers answer each question in
// reader-1.jsonl and reader-2.jsonl; the gate checks every verdict against the items; accept writes the
// agreed ones as decisions, which resolve.mjs reads on its next run. Readers never write decisions.

import {createHash} from 'node:crypto';
import {appendFileSync,existsSync,mkdirSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';

const LINE=dirname(fileURLToPath(import.meta.url));
const REVIEW=join(LINE,'..');
const PACKETS=join(LINE,'packets');
const ROOT=join(REVIEW,'..','..','..');
export const VERDICTS=['same','different','related','undecidable'];
export const RELATIONS=['pseudonym_of','attributed_to','conventional_for','part_of','namesake_of'];
const AGENT_ID=/\b((?:A\d{2}|T[23]-\d{2})-A\d{3,4})\b/g;
const WORKS_SHOWN=12;
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
const shortId=(prefix,value)=>{const bytes=createHash('sha256').update(value).digest();let bits=0,acc=0,out='';for(const byte of bytes){acc=(acc<<8)|byte;bits+=8;while(bits>=5&&out.length<10){out+=BASE32[(acc>>>(bits-5))&31];bits-=5;}}return `${prefix}-${out}`;};

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

function evidence(rows,id){
  const {parsed,raw,works}=rows.get(id);
  const fields={};
  for(const field of FIELDS)if(raw?.[field])fields[field]=raw[field];
  if(works.length)fields.works=works.slice(0,WORKS_SHOWN).join('; ')+(works.length>WORKS_SHOWN?`; … ${works.length-WORKS_SHOWN} more`:'');
  // The typed identifiers the resolution compared, so that a reader sees what an id-conflict is about.
  const ids=[...(parsed.ids??[]).map(({scheme,value,valid})=>valid===false?{scheme,value,valid}:{scheme,value}),
    ...(parsed.existing??[]).filter(value=>value.startsWith('tos.agent.')).map(value=>({scheme:'tos-agent',value}))];
  return {row:id,kind:parsed.kind,kind_basis:parsed.kind_basis,status:parsed.status,role:parsed.role,ids,fields};
}

function item(rows,entry){
  const body={item:entry.item,type:entry.type,reason:entry.reason,shared:entry.shared,
    a:{member:entry.a.member,rows:entry.a.rows.map(id=>evidence(rows,id))},
    b:{member:entry.b.member,rows:entry.b.rows.map(id=>evidence(rows,id))}};
  return {...body,sha:sha(body)};
}

const packetIds=()=>existsSync(PACKETS)?readdirSync(PACKETS).filter(name=>/^p-\d{4}$/.test(name)).sort():[];
const packetFile=(packet,name)=>join(PACKETS,packet,name);
const retired=packet=>existsSync(packetFile(packet,'retired.json'));

function packets(size){
  const queue=readJsonl(join(REVIEW,'queue','identity.jsonl'));
  const byItem=new Map(queue.map(entry=>[entry.item,entry]));
  const rows=layerRows();
  let next=packetIds().length?Number(packetIds().at(-1).slice(2))+1:1,made=0,gone=0;
  // A packet not yet accepted whose questions left the queue or whose evidence changed (a later run of
  // resolve.mjs read decisions) is retired; its questions still open are cut anew below.
  for(const packet of packetIds()){
    if(existsSync(packetFile(packet,'accepted.json'))||retired(packet))continue;
    const meta=JSON.parse(readFileSync(packetFile(packet,'packet.json'),'utf8'));
    const items=meta.items.map(id=>byItem.get(id)).filter(Boolean).map(entry=>item(rows,entry));
    if(items.length<meta.items.length||items.some(entry=>entry.sha!==meta.shas[entry.item])){
      writeFileSync(packetFile(packet,'retired.json'),JSON.stringify({packet,reason:'its questions or their evidence changed since it was cut'},null,1)+'\n');
      gone++;continue;
    }
    writeJsonl(packetFile(packet,'items.jsonl'),items);
  }
  const held=new Set(packetIds().filter(packet=>!retired(packet)).flatMap(packet=>JSON.parse(readFileSync(packetFile(packet,'packet.json'),'utf8')).items));
  // The questions about planted members come first: both sides planted, then one, then none.
  const population=join(ROOT,'bonsai','population','population.json');
  const planted=new Set(existsSync(population)?Object.keys(JSON.parse(readFileSync(population,'utf8')).members):[]);
  const weight=entry=>2-[entry.a.member,entry.b.member].filter(member=>planted.has(member)).length;
  const waiting=queue.map((entry,at)=>({entry,at})).filter(({entry})=>!held.has(entry.item))
    .sort((x,y)=>weight(x.entry)-weight(y.entry)||x.at-y.at).map(({entry})=>entry);
  for(let i=0;i<waiting.length;i+=size){
    const packet=`p-${String(next++).padStart(4,'0')}`,chunk=waiting.slice(i,i+size);
    const items=chunk.map(entry=>item(rows,entry));
    mkdirSync(join(PACKETS,packet),{recursive:true});
    writeFileSync(packetFile(packet,'packet.json'),JSON.stringify({packet,type:'identity',items:chunk.map(entry=>entry.item),shas:Object.fromEntries(items.map(entry=>[entry.item,entry.sha]))},null,1)+'\n');
    writeJsonl(packetFile(packet,'items.jsonl'),items);
    made++;
  }
  console.log(`packets: ${made} new, ${gone} retired, ${packetIds().length} in all; ${waiting.length} questions cut, ${held.size} already held`);
}

function textsOf(entry){
  const out=new Map();
  for(const side of ['a','b'])for(const row of entry[side].rows)for(const [field,text] of Object.entries(row.fields))out.set(`${side}|${row.row}|${field}`,text);
  return out;
}

/** Problems of one packet's verdicts; `fault` marks a problem of the line rather than of a reader. */
export function gatePacket(packet){
  const problems=[],faults=[];
  if(retired(packet))return {problems,faults:[`${packet} is retired; its open questions are in newer packets`],agreed:[]};
  if(!existsSync(packetFile(packet,'packet.json'))||!existsSync(packetFile(packet,'items.jsonl')))return {problems,faults:['packet.json or items.jsonl is missing (run kit.mjs packets)'],agreed:[]};
  const meta=JSON.parse(readFileSync(packetFile(packet,'packet.json'),'utf8'));
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
      if(verdict.sha!==entry.sha)problems.push(`${at}: sha does not match the question`);
      if(!VERDICTS.includes(verdict.verdict))problems.push(`${at}: verdict ${verdict.verdict} is not one of ${VERDICTS.join(', ')}`);
      if(verdict.verdict==='related'?!RELATIONS.includes(verdict.relation):verdict.relation!=null)problems.push(`${at}: relation ${verdict.relation} does not fit the verdict`);
      const basis=verdict.basis??[];
      if(verdict.verdict!=='undecidable'&&(!basis.length||basis.some(item=>!BASES.includes(item))))problems.push(`${at}: basis must name what decided it, from ${BASES.join(', ')}`);
      if(verdict.verdict==='same'&&!basis.some(item=>item!=='name'))problems.push(`${at}: same needs a basis beyond the name`);
      const texts=textsOf(entry);
      const quotes=verdict.quotes??[];
      if(verdict.verdict!=='undecidable'&&!quotes.length)problems.push(`${at}: no quote of the rows`);
      for(const quote of quotes){
        const text=texts.get(`${quote.side}|${quote.row}|${quote.field}`);
        if(text===undefined)problems.push(`${at}: quotes ${quote.side} ${quote.row} ${quote.field}, which the question does not hold`);
        else if(typeof quote.text!=='string'||!text.includes(quote.text))problems.push(`${at}: the quote from ${quote.row} ${quote.field} is not its exact words`);
        else if([...quote.text.trim()].length<4)problems.push(`${at}: the quote from ${quote.row} ${quote.field} is shorter than four characters; quote more of the field or another field`);
      }
      for(const found of verdict.authority??[]){
        const valid=AUTHORITY[found.scheme];
        if(!valid)problems.push(`${at}: authority scheme ${found.scheme} is not one of ${Object.keys(AUTHORITY).join(', ')}`);
        else if(!valid(String(found.value)))problems.push(`${at}: ${found.scheme} ${found.value} is not a valid identifier`);
        if(!['a','b'].includes(found.side))problems.push(`${at}: an authority names side a or b`);
      }
      if(verdict.sources!=null&&(!Array.isArray(verdict.sources)||verdict.sources.some(source=>typeof source!=='string'||source.trim().length<4)))problems.push(`${at}: sources must list the URLs or citations you opened`);
      if(typeof verdict.note!=='string'||!verdict.note.trim())problems.push(`${at}: no note`);
      answers.push({reader,verdict});
    }
    for(const id of meta.items)if(!seen.has(id))problems.push(`reader-${reader}: ${id} is not answered`);
  }
  const agreed=[];
  if(!problems.length&&!faults.length){
    for(const id of meta.items){
      const [one,two]=[1,2].map(reader=>answers.find(answer=>answer.reader===reader&&answer.verdict.item===id).verdict);
      const authorities=side=>new Map([...(one.authority??[]),...(two.authority??[])].filter(found=>found.side===side).map(found=>[found.scheme,new Set()]));
      let conflict=false;
      for(const side of ['a','b']){
        const schemes=authorities(side);
        for(const found of [...(one.authority??[]),...(two.authority??[])])if(found.side===side)schemes.get(found.scheme).add(String(found.value));
        for(const values of schemes.values())if(values.size>1)conflict=true;
      }
      // `different` and `related` as `namesake_of` both say two bearers; a split between them is no dispute.
      const bearers=verdict=>verdict.verdict==='different'||(verdict.verdict==='related'&&verdict.relation==='namesake_of');
      const exact=one.verdict===two.verdict&&(one.relation??null)===(two.relation??null);
      const same=(exact||(bearers(one)&&bearers(two)))&&!conflict;
      agreed.push({item:id,agreed:same,verdict:same?(exact?one.verdict:'different'):null,relation:same&&exact?one.relation??null:null,readers:[one,two]});
    }
  }
  return {problems,faults,agreed};
}

function accept(packet,by){
  const {problems,faults,agreed}=gatePacket(packet);
  if(problems.length||faults.length)throw Object.assign(new Error(`${packet}: the gate is shut`),{line:faults.length>0});
  if(existsSync(packetFile(packet,'accepted.json')))throw new Error(`${packet}: already accepted`);
  const items=new Map(readJsonl(packetFile(packet,'items.jsonl')).map(entry=>[entry.item,entry]));
  const known=new Set(readJsonl(join(REVIEW,'decisions.jsonl')).map(decision=>decision.id));
  const decisions=[],held=[];
  for(const answer of agreed){
    const entry=items.get(answer.item);
    const rows=[entry.a.rows[0].row,entry.b.rows[0].row];
    const type=!answer.agreed||answer.verdict==='undecidable'?'held':answer.verdict;
    const decision={id:shortId('d',`${type}|${rows.join('|')}`),type,rows,item:answer.item,packet,by,readers:answer.readers.map(verdict=>({verdict:verdict.verdict,relation:verdict.relation??null,basis:verdict.basis,note:verdict.note}))};
    if(type==='related')decision.relation=answer.relation;
    if(!answer.agreed)decision.reason='readers disagree';
    if(known.has(decision.id))continue;
    known.add(decision.id);
    decisions.push(decision);
    if(type==='held')held.push(answer.item);
  }
  appendFileSync(join(REVIEW,'decisions.jsonl'),decisions.map(decision=>JSON.stringify(decision)+'\n').join(''));
  writeFileSync(packetFile(packet,'accepted.json'),JSON.stringify({packet,by,decisions:decisions.map(decision=>decision.id),held},null,1)+'\n');
  console.log(`${packet}: ${decisions.length} decisions, ${held.length} held for another reader`);
}

function queues({check=false}={}){
  const rows=layerRows();
  const population=JSON.parse(readFileSync(join(ROOT,'bonsai','population','population.json'),'utf8'));
  const kind=[],dates=[],latin=[],living=[];
  for(const [member,record] of Object.entries(population.members)){
    if(record.gaps.includes('name:latin'))latin.push({type:'latin',member,rows:record.grew_from,name:record.name});
    const datedRows=record.grew_from.map(id=>rows.get(id).parsed);
    if(!record.phases.length||record.gaps.includes('life:rows-differ'))
      dates.push({type:'dates',member,reason:record.phases.length?'rows-differ':record.gaps.find(gap=>gap.startsWith('life:'))??'life:none',rows:record.grew_from,gaps:[...new Set(datedRows.flatMap(row=>row.gaps.filter(gap=>gap.startsWith('life:'))))].sort()});
    const open=datedRows.some(row=>row.life.birth?.kind==='point'&&row.life.birth.year>=1900&&row.life.death?.kind!=='point');
    if(open)living.push({type:'living',member,rows:record.grew_from});
    for(const row of datedRows)if(row.signals.length)kind.push({type:'kind',member,row:row.row,kind:row.kind,signals:row.signals});
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

/** The questions of a packet as a reader reads them, with the sha each verdict copies. */
function show(packet,only){
  const items=readJsonl(packetFile(packet,'items.jsonl'));
  if(!items.length)throw Object.assign(new Error(`${packet}: no questions (run kit.mjs packets)`),{line:true});
  for(const entry of items){
    if(only&&entry.item!==only)continue;
    const out=[`${entry.item} · ${entry.reason} · shared: ${entry.shared.join(', ')}`,`  sha ${entry.sha}`];
    for(const side of ['a','b']){
      out.push(`  ${side} · ${entry[side].member}`);
      for(const row of entry[side].rows){
        const ids=(row.ids??[]).map(found=>`${found.scheme} ${found.value}${found.valid===false?' (bad check digit)':''}`);
        out.push(`    ${row.row} · ${row.kind} · ${row.status||'no status'}${ids.length?` · ${ids.join(', ')}`:''}`);
        for(const [field,text] of Object.entries(row.fields))out.push(`      ${field}: ${text.replace(/\s*\n\s*/g,' ⏎ ')}`);
      }
    }
    console.log(`${out.join('\n')}\n`);
  }
}

function status(){
  const counts={packets:0,answered:0,open:0,accepted:0,retired:0,questions:0};
  for(const packet of packetIds()){
    if(retired(packet)){counts.retired++;continue;}
    counts.packets++;
    counts.questions+=JSON.parse(readFileSync(packetFile(packet,'packet.json'),'utf8')).items.length;
    if(existsSync(packetFile(packet,'accepted.json')))counts.accepted++;
    else if(existsSync(packetFile(packet,'reader-1.jsonl'))&&existsSync(packetFile(packet,'reader-2.jsonl')))counts.answered++;
    else counts.open++;
  }
  counts.decisions=readJsonl(join(REVIEW,'decisions.jsonl')).length;
  console.log(JSON.stringify(counts));
}

function main([command,...args]){
  const option=name=>{const at=args.indexOf(name);return at>=0?args[at+1]:null;};
  try{
    if(command==='queues')queues({check:args.includes('--check')});
    else if(command==='packets')packets(Number(option('--size')??20));
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
    else{console.error('usage: kit.mjs queues [--check] | packets [--size N] | show <packet> [<item>] | gate <packet> [--reader N] | accept <packet> --by <who> | status');process.exit(2);}
  }catch(error){console.error(`${error.line?'line: ':''}${error.message}`);process.exit(error.line?3:1);}
}

if(process.argv[1]===fileURLToPath(import.meta.url))main(process.argv.slice(2));

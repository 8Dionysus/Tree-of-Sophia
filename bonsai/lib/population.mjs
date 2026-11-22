// The core of a population (bonsai/CORE.md, "A core of a population"): the members a stand's research
// registers name. Each member grew from the register rows the identity resolution joined, carries a
// kind the Tree's entity registry types, its names, the register's own statuses beside the reader's
// verdict, its life as phases of the stand's chronology and its Tree identity in the graft map.
//
// `populationIssues` checks the module, alone and against the chronology and the graft map;
// `populationReviewIssues` checks the review folder it grew from (bonsai/review/population/): every
// register row in exactly one member or left out with a reason, every member ID in the ledger.

import {existsSync,readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';

export const POPULATION_SCHEMA='tos.bonsai.population.v1';
export const MEMBER_ID=/^m-[a-z2-7]{10,16}$/;
export const REGISTER_ROW=/^((?:A\d{2}|T[23]-\d{2})):\1-A\d{3,4}$/;
export const VERDICTS=['undecided','confirmed','revise','rejected'];
export const REGISTER_STATUSES=['verified','provisional','disputed'];
const AGENT_TYPES=['tos.entity.agent','tos.entity.organization'];
const GAP=/^[a-z]+(?::[a-z0-9-]+)*\??$/;
const TREE_ID=/^tos\.[a-z][a-z0-9-]*\.[a-z0-9]+(?:[.-][a-z0-9]+)*$/;
const text=value=>typeof value==='string'&&value.trim().length>0;
const bilingual=value=>Boolean(value)&&text(value.ru)&&text(value.en);

/** Problems of a population module; `chronology`, `identities` and `registry` add the checks that need them. */
export function populationIssues(module,{chronology=null,identities=null,registry=null}={}){
  if(!module||typeof module!=='object')return ['not an object'];
  const problems=[];
  if(module.schema!==POPULATION_SCHEMA)problems.push('schema');
  if(!bilingual(module.posture))problems.push('posture: needs ru and en');
  const kinds=module.kinds??{};
  if(!Object.keys(kinds).length)problems.push('kinds: none');
  for(const [kind,entry] of Object.entries(kinds)){
    if(!bilingual(entry?.label))problems.push(`kind ${kind}: label needs ru and en`);
    if(!AGENT_TYPES.includes(entry?.tree_type)&&!(registry&&registry.within(entry?.tree_type,AGENT_TYPES)))problems.push(`kind ${kind}: tree_type ${entry?.tree_type} is not an agent or organization type`);
    else if(registry&&!registry.hasType(entry.tree_type))problems.push(`kind ${kind}: ${entry.tree_type} is not in the Tree's entity registry`);
  }
  const batches=module.batches??{};
  const members=module.members??{};
  const rows=new Map();
  const phases=new Map();
  for(const [id,member] of Object.entries(members)){
    const at=`member ${id}`;
    if(!MEMBER_ID.test(id))problems.push(`${at}: id is not m- and base32`);
    if(!member||typeof member!=='object'){problems.push(`${at}: not an object`);continue;}
    if(!(member.kind in kinds))problems.push(`${at}: unknown kind ${member.kind}`);
    if(!bilingual(member.name))problems.push(`${at}: name needs ru and en`);
    if(member.name?.original!==undefined&&!text(member.name.original))problems.push(`${at}: original name is empty`);
    if(typeof member.core!=='boolean')problems.push(`${at}: core is not true or false`);
    if(!VERDICTS.includes(member.verdict))problems.push(`${at}: verdict ${member.verdict} is not one of ${VERDICTS.join(', ')}`);
    const statuses=member.register?.statuses;
    if(!Array.isArray(statuses)||!statuses.length||statuses.some(status=>!REGISTER_STATUSES.includes(status)))problems.push(`${at}: register statuses ${JSON.stringify(statuses)}`);
    if(!Array.isArray(member.grew_from)||!member.grew_from.length)problems.push(`${at}: grew_from names no register row`);
    for(const row of member.grew_from??[]){
      const match=REGISTER_ROW.exec(row);
      if(!match){problems.push(`${at}: ${row} is not a register row`);continue;}
      if(!(match[1] in batches))problems.push(`${at}: ${row} names a batch the module does not list`);
      if(rows.has(row))problems.push(`${at}: ${row} already grew ${rows.get(row)}`);
      rows.set(row,id);
    }
    if(!Array.isArray(member.phases))problems.push(`${at}: phases is not a list`);
    for(const phase of member.phases??[]){
      if(phases.has(phase))problems.push(`${at}: phase ${phase} already belongs to ${phases.get(phase)}`);
      phases.set(phase,id);
    }
    for(const gap of member.gaps??[])if(!GAP.test(gap))problems.push(`${at}: gap ${gap} is not a gap code`);
    if(member.kind==='collective'&&member.phases?.length)problems.push(`${at}: a collective has no life phases`);
    // The row the member's ID was founded on, and the register's own words for each row, where the module keeps them.
    if(member.founded!==undefined&&!(member.grew_from??[]).includes(member.founded))problems.push(`${at}: founded on ${member.founded}, which it did not grow from`);
    const byRow=member.register?.rows;
    if(byRow!==undefined){
      if(!byRow||typeof byRow!=='object'||Object.keys(byRow).sort().join('|')!==[...(member.grew_from??[])].sort().join('|'))problems.push(`${at}: register rows are not the rows it grew from`);
      else for(const [row,said] of Object.entries(byRow)){
        if(said.status!==undefined&&!REGISTER_STATUSES.includes(said.status))problems.push(`${at}: ${row} has status ${said.status}`);
        if(said.role!==undefined&&!text(said.role))problems.push(`${at}: ${row} has an empty role`);
      }
    }
  }
  const regions=module.regions??{};
  for(const [key,names] of Object.entries(regions))if(!bilingual(names))problems.push(`region ${key}: needs ru and en`);
  for(const [batch,entry] of Object.entries(batches)){
    if(!/^(?:A\d{2}|T[23]-\d{2})$/.test(batch))problems.push(`batch ${batch}: not a batch name`);
    if(!Number.isInteger(entry?.rows)||entry.rows<0)problems.push(`batch ${batch}: rows is not a count`);
    if(entry?.register!==undefined&&!/^https:\/\/treeofsophia\.com\/__research\/layers\/sha256\/[0-9a-f]{64}$/.test(entry.register))problems.push(`batch ${batch}: register is not a layer address of the site`);
    // A batch is a research node of the master tables: its table, title, region and windows of time.
    const node=entry?.node;
    if(node!==undefined){
      if(!['I','II','III'].includes(node?.table))problems.push(`batch ${batch}: node table ${node?.table} is not I, II or III`);
      if(!bilingual(node?.title))problems.push(`batch ${batch}: node title needs ru and en`);
      if(!(node?.region in regions))problems.push(`batch ${batch}: node region ${node?.region} is not one the module names`);
      for(const window of ['formation','fixation','canonization'])if(!text(node?.windows?.[window]?.words))problems.push(`batch ${batch}: node window ${window} has no words`);
    }
  }
  for(const [from,to] of Object.entries(module.redirects??{})){
    if(!MEMBER_ID.test(from))problems.push(`redirect ${from}: not a member ID`);
    if(from in members)problems.push(`redirect ${from}: still a member`);
    if(!(to in members))problems.push(`redirect ${from}: leads to ${to}, which is not a member`);
  }
  if(chronology){
    const situations=chronology.situations??{};
    for(const [phase,id] of phases){
      const situation=situations[phase];
      if(!situation)problems.push(`member ${id}: phase ${phase} is not in the chronology`);
      else if(situation.type!=='biographical-phase'||situation.subject!==id)problems.push(`member ${id}: phase ${phase} is not a biographical phase of this member`);
    }
    for(const [key,situation] of Object.entries(situations)){
      if(situation.type!=='biographical-phase')continue;
      if(!(situation.subject in members))problems.push(`chronology ${key}: its subject ${situation.subject} is not a member`);
      else if(phases.get(key)!==situation.subject)problems.push(`chronology ${key}: its member does not list it`);
    }
    for(const [id,key] of Object.entries(chronology.lives??{})){
      if(!(id in members))problems.push(`chronology life of ${id}: not a member`);
      else if(phases.get(key)!==id)problems.push(`chronology life of ${id}: ${key} is not one of its phases`);
    }
  }
  if(identities){
    for(const [id,member] of Object.entries(members)){
      const section=kinds[member.kind]?.tree_type==='tos.entity.organization'?'organizations':'agents';
      const entry=identities[section]?.[id];
      if(!entry)problems.push(`member ${id}: no Tree identity in the graft map's ${section}`);
      else{
        if(!TREE_ID.test(entry.tree??''))problems.push(`member ${id}: Tree identity ${entry.tree} is not a Tree ID`);
        if(!['resolved','to-mint'].includes(entry.status))problems.push(`member ${id}: graft status ${entry.status}`);
      }
    }
    const trees=new Map();
    for(const section of ['agents','organizations'])for(const [id,entry] of Object.entries(identities[section]??{})){
      if(trees.has(entry.tree))problems.push(`graft map: ${entry.tree} stands for both ${trees.get(entry.tree)} and ${id}`);
      trees.set(entry.tree,id);
    }
  }
  return problems;
}

const readJsonl=path=>readFileSync(path,'utf8').split('\n').filter(Boolean).map(line=>JSON.parse(line));

/** The review folder a population bonsai grew from: register rows, keys, ledger and the planted members. */
export function populationReviewIssues(root,{population=null}={}){
  const review=join(root,'bonsai','review','population');
  const problems=[];
  const keysPath=join(review,'keys.jsonl'),idsPath=join(review,'ids.jsonl');
  if(!existsSync(keysPath)||!existsSync(idsPath))return ['keys.jsonl or ids.jsonl is missing'];
  const keys=readJsonl(keysPath);
  const ledger=new Map(),merged=new Set();
  for(const line of readJsonl(idsPath)){
    // A merged ID keeps leading to its member: retiring it afterwards would lose the way there.
    if(line.state==='retired'&&merged.has(line.id))problems.push(`ledger: ${line.id} was merged and then retired`);
    if(line.state==='redirect')merged.add(line.id);
    ledger.set(line.id,{...(ledger.get(line.id)??{}),...line});
  }
  const live=new Set([...ledger].filter(([,entry])=>!entry.state).map(([id])=>id));
  const seen=new Set(),byMember=new Map();
  for(const line of keys){
    if(!REGISTER_ROW.test(line.row))problems.push(`keys: ${line.row} is not a register row`);
    if(seen.has(line.row))problems.push(`keys: ${line.row} twice`);
    seen.add(line.row);
    if(line.member){
      if(!live.has(line.member))problems.push(`keys: ${line.row} belongs to ${line.member}, which the ledger does not hold as a member`);
      if(!byMember.has(line.member))byMember.set(line.member,[]);
      byMember.get(line.member).push(line.row);
    }else if(!text(line.left_out))problems.push(`keys: ${line.row} has neither a member nor a reason it was left out`);
  }
  for(const [id,entry] of ledger){
    if(entry.state==='redirect'){
      const seenIds=new Set([id]);let to=entry.to;
      while(ledger.get(to)?.state==='redirect'){if(seenIds.has(to)){problems.push(`ledger: redirect cycle at ${id}`);break;}seenIds.add(to);to=ledger.get(to).to;}
      if(!live.has(to))problems.push(`ledger: ${id} redirects to ${entry.to}, which leads to no member`);
    }else if(!entry.state&&!REGISTER_ROW.test(entry.founded_on??''))problems.push(`ledger: ${id} names no founding row`);
  }
  const registry=join(review,'registry');
  if(existsSync(registry)){
    let count=0;
    for(const batch of readdirSync(registry).sort()){
      const path=join(registry,batch,'agents.jsonl');
      if(!existsSync(path))continue;
      for(const row of readJsonl(path).slice(1)){
        const key=`${batch}:${row.record_id.trim()}`;count++;
        if(!seen.has(key))problems.push(`keys: register row ${key} is in no member and not left out`);
      }
    }
    if(count!==keys.length)problems.push(`keys: ${keys.length} lines for ${count} register rows`);
  }
  if(population){
    for(const [id,member] of Object.entries(population.members??{})){
      const rows=byMember.get(id)??[];
      if(JSON.stringify([...rows].sort())!==JSON.stringify([...(member.grew_from??[])].sort()))problems.push(`member ${id}: grew_from differs from the rows keys.jsonl gives it`);
    }
  }
  const controlPath=join(review,'control.json');
  if(existsSync(controlPath))problems.push(...controlIssues(JSON.parse(readFileSync(controlPath,'utf8')),keys).map(issue=>`control: ${issue}`));
  return problems;
}

/** The control cases of the identity resolution against keys.jsonl: same, never-same, never-auto. */
export function controlIssues(control,keys){
  const byRow=new Map(keys.map(line=>[line.row,line]));
  const problems=[];
  for(const item of control.cases??[]){
    const lines=item.rows.map(row=>byRow.get(row));
    if(lines.some(line=>!line)){problems.push(`${item.id}: names a row keys.jsonl lacks`);continue;}
    const members=lines.map(line=>line.member??null);
    if(item.must==='same'){
      if(members.some(member=>!member)||new Set(members).size!==1)problems.push(`${item.id}: the rows are not one member`);
      continue;
    }
    for(let i=0;i<lines.length;i++)for(let j=i+1;j<lines.length;j++){
      if(!members[i]||members[i]!==members[j])continue;
      if(item.must==='never-same')problems.push(`${item.id}: ${item.rows[i]} and ${item.rows[j]} are one member`);
      else if(item.must==='never-auto'&&![lines[i],lines[j]].some(line=>String(line.basis).startsWith('decision:')))problems.push(`${item.id}: ${item.rows[i]} and ${item.rows[j]} were joined without a reader's decision`);
    }
    if(!['never-same','never-auto'].includes(item.must))problems.push(`${item.id}: unknown requirement ${item.must}`);
  }
  return problems;
}

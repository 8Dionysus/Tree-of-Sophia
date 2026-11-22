#!/usr/bin/env node
// Resolve the persons register rows into members of the Population bonsai.
//
//   node bonsai/review/population/resolve.mjs [--check]
//
// Reads layer/agents/<batch>.jsonl (parse.py), the authors table rows under registry/authors-table/,
// the readers' decisions.jsonl and the ID ledger ids.jsonl, and writes:
//
//   keys.jsonl            every register row: the member it belongs to, or why it was left out
//   ids.jsonl             the ledger of member IDs: minted, merged into another, retired
//   core.json             the authors table's names bound to the register rows they stand for
//   queue/identity.jsonl  the pairs of members a reader has to decide (line/kit.mjs cuts it into packets)
//
// A member's ID is `m-` and ten base32 characters of the SHA-256 of the register row it was founded
// on. The ledger keeps every ID ever minted: a merge leaves the later ID as a redirect, a split keeps
// the ID with its founding row, a row split off a merge whose ID became a redirect gets a new ID, and
// no ID is given twice. keys.jsonl is read back on the next run,
// so a row keeps its member while the evidence for it stands.
//
// Two rows join only (strictly): on a shared authority identifier; on a shared strong name key with
// an agreeing year of birth or death; on a short name key with both years agreeing. Rows of one batch,
// a pseudo-author with anyone else, and a collective with a person never join; every other pair that
// shares a key goes to the queue. A union that would put two rows of one batch, two rows a reader
// called different, or two conflicting dates into one member is refused and queued as `blocked`.
//
// --check builds everything in memory and compares it with the files on disk.

import {createHash} from 'node:crypto';
import {existsSync,mkdirSync,readFileSync,readdirSync,renameSync,writeFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {fold,formHead,formKeys,ideographic,scriptOf,slugify} from './keys.mjs';

const HERE=dirname(fileURLToPath(import.meta.url));
const ROOT=join(HERE,'..','..','..');
export const CERTAINTIES=['exact','approximate','uncertain'];
const TIERS=['id','decision','name+year','short+years'];
const KEY_CAP=40;
const SHORT_GAP=100;

/**
 * The largest difference, in years, at which two register years of one person's birth (or death)
 * still count as the same year. `year` is the earlier of the two astronomical years (1 BCE = 0);
 * `certainty` is the weaker of their two certainties: 'exact', 'approximate' or 'uncertain'.
 * A difference within it lets a name key join two rows; a difference beyond it is a date conflict,
 * which keeps them apart until a reader decides.
 */
export function yearTolerance(year,certainty){
  // Measured on rows that share a name across batches: one person's exact years differ by one
  // year from 1500 on (Acosta 1539/1540: Julian and Hijri year boundaries, era conversions) and
  // by two before it (Xuanzang 600/602); approximate and uncertain years differ by up to a decade
  // (Ibn Sīnā c. 970/c. 980). Wider gaps, such as Galen's death c. 200/216, go to a reader.
  const documentary=year>=1500;
  if(certainty==='exact')return documentary?1:2;
  return documentary?5:10;
}

const readJsonl=path=>existsSync(path)?readFileSync(path,'utf8').split('\n').filter(Boolean).map(line=>JSON.parse(line)):[];
const sha=text=>createHash('sha256').update(text).digest();
const BASE32='abcdefghijklmnopqrstuvwxyz234567';
function base32(bytes){
  let bits=0,value=0,out='';
  for(const byte of bytes){
    value=(value<<8)|byte;bits+=8;
    while(bits>=5){out+=BASE32[(value>>>(bits-5))&31];bits-=5;}
  }
  return out;
}

/** Every parsed row, in batch order and in the row order of its sheet. */
export function loadRows(review){
  const dir=join(review,'layer','agents');
  if(!existsSync(dir))throw new Error('resolve: no layer/agents/ (run parse.py, or fetch the layers)');
  const rows=[];
  for(const file of readdirSync(dir).filter(name=>name.endsWith('.jsonl')).sort()){
    const batch=file.replace(/\.jsonl$/,'');
    for(const record of readJsonl(join(dir,file)))rows.push({...record,batch,index:rows.length});
  }
  return rows;
}

function lifeYears(row){
  const life=row.life??{};
  const at=entry=>entry?.kind==='point'?{year:entry.year,certainty:entry.certainty}:null;
  let birth=at(life.birth),death=at(life.death);
  if(life.life?.kind==='span'&&!life.life.century){
    birth??={year:life.life.years[0],certainty:life.life.certainty};
    death??={year:life.life.years[1],certainty:life.life.certainty};
  }
  return {birth,death};
}

function rowKeys(row){
  const cjk=row.forms.some(form=>ideographic(scriptOf(form.text)));
  const keys=new Map();
  for(const form of row.forms){
    for(const item of formKeys(form.text)){
      const level=cjk&&!ideographic(item.script)?'short':item.strength;
      if(!keys.has(item.key)||(keys.get(item.key)==='short'&&level==='strong'))keys.set(item.key,level);
    }
  }
  return keys;
}

function rowIds(row){
  const ids=new Map();
  const add=(scheme,value)=>{if(!ids.has(scheme))ids.set(scheme,new Set());ids.get(scheme).add(value);};
  for(const item of row.ids??[])if(item.valid!==false)add(item.scheme,item.value);
  // Only a Tree agent ID names a person; `table-…-node-…` and `candidate-node:…` point at a node of the row's
  // own batch, which differs between batches for one person and would make every such pair a conflict.
  for(const value of row.existing??[])if(value.startsWith('tos.agent.'))add('tos-agent',value);
  return ids;
}

const weaker=(a,b)=>CERTAINTIES[Math.max(CERTAINTIES.indexOf(a),CERTAINTIES.indexOf(b))]??'uncertain';

/** How the years of one event in two rows stand: 'agree', 'conflict' or null where either lacks it. */
function yearsMeet(x,y,tolerance){
  if(!x||!y)return null;
  const allowed=tolerance(Math.min(x.year,y.year),weaker(x.certainty,y.certainty));
  if(!Number.isInteger(allowed)||allowed<0)throw new Error(`yearTolerance must give a whole number of years ≥ 0, gave ${allowed}`);
  return Math.abs(x.year-y.year)<=allowed?'agree':'conflict';
}

function dates(a,b,tolerance){
  const birth=yearsMeet(a.years.birth,b.years.birth,tolerance),death=yearsMeet(a.years.death,b.years.death,tolerance);
  const gaps=[];
  if(birth)gaps.push(Math.abs(a.years.birth.year-b.years.birth.year));
  if(death)gaps.push(Math.abs(a.years.death.year-b.years.death.year));
  return {birth,death,conflict:birth==='conflict'||death==='conflict',gap:gaps.length?Math.max(...gaps):null};
}

function idsMeet(a,b){
  let shared=false,conflict=false;
  for(const [scheme,values] of a.idSets){
    const other=b.idSets.get(scheme);
    if(!other)continue;
    if([...values].some(value=>other.has(value)))shared=true;
    else conflict=true;
  }
  return {shared,conflict};
}

/** What two rows of different members are to each other: apart, joined, or a question for a reader. */
export function compare(a,b,tolerance){
  if(a.batch===b.batch)return {decision:'apart',reason:'same-batch'};
  if((a.kind==='pseudonymous')!==(b.kind==='pseudonymous'))return {decision:'apart',reason:'pseudo-with-other'};
  if((a.kind==='collective')!==(b.kind==='collective'))return {decision:'apart',reason:'collective-with-other'};
  const ids=idsMeet(a,b),meet=dates(a,b,tolerance);
  let strong=false,any=false;
  for(const [key,level] of a.keys){
    if(!b.keys.has(key))continue;
    any=true;
    if(level==='strong'&&b.keys.get(key)==='strong')strong=true;
  }
  const disputed=a.status==='disputed'||b.status==='disputed';
  const sameKind=a.kind===b.kind;
  if(ids.shared&&!ids.conflict&&!meet.conflict&&sameKind&&!disputed)return {decision:'auto',tier:'id'};
  if(!any&&!ids.shared){
    const contained=[...a.heads??[]].some(head=>b.keys.has(head))||[...b.heads??[]].some(head=>a.keys.has(head));
    if(!contained)return {decision:'apart',reason:'no-shared-key'};
    return meet.conflict&&meet.gap>SHORT_GAP?{decision:'apart',reason:'contained-conflict'}:{decision:'queue',reason:'contained'};
  }
  if(ids.conflict)return {decision:'queue',reason:'id-conflict'};
  if(!sameKind)return {decision:'queue',reason:'kind'};
  if(disputed)return {decision:'queue',reason:'disputed'};
  if(ids.shared)return {decision:'queue',reason:'id-with-date-conflict'};
  if(strong){
    if(meet.conflict)return {decision:'queue',reason:'date-conflict'};
    if(meet.birth==='agree'||meet.death==='agree')return {decision:'auto',tier:'name+year'};
    return {decision:'queue',reason:'strong-undated'};
  }
  if(meet.conflict)return meet.gap>SHORT_GAP?{decision:'apart',reason:'short-conflict'}:{decision:'queue',reason:'short-conflict'};
  if(meet.birth==='agree'&&meet.death==='agree')return {decision:'auto',tier:'short+years'};
  if(meet.birth==='agree'||meet.death==='agree')return {decision:'queue',reason:'short-partial'};
  return {decision:'queue',reason:'short-only'};
}

export class Clusters{
  constructor(rows){this.parent=rows.map((_,i)=>i);this.members=new Map(rows.map((_,i)=>[i,[i]]));}
  find(i){while(this.parent[i]!==i){this.parent[i]=this.parent[this.parent[i]];i=this.parent[i];}return i;}
  union(a,b){
    let ra=this.find(a),rb=this.find(b);
    if(ra===rb)return;
    if(this.members.get(ra).length<this.members.get(rb).length||(this.members.get(ra).length===this.members.get(rb).length&&rb<ra))[ra,rb]=[rb,ra];
    this.parent[rb]=ra;
    this.members.set(ra,[...this.members.get(ra),...this.members.get(rb)].sort((x,y)=>x-y));
    this.members.delete(rb);
  }
}

export function tolerated(rows,clusters,a,b,cannot,tolerance){
  const left=clusters.members.get(clusters.find(a)),right=clusters.members.get(clusters.find(b));
  for(const x of left)for(const y of right){
    const rx=rows[x],ry=rows[y];
    if(rx.batch===ry.batch)return 'same-batch';
    if(cannot.has(`${Math.min(x,y)}|${Math.max(x,y)}`))return 'decided-different';
    if(dates(rx,ry,tolerance).conflict)return 'date-conflict';
    // Two identifiers of one authority that differ refuse the union even when the edge itself joins rows with no
    // identifier: an identifier-free row between Q1 and Q2 must not carry them into one member past a reader.
    if(idsMeet(rx,ry).conflict)return 'id-conflict';
    if((rx.kind==='pseudonymous')!==(ry.kind==='pseudonymous')||(rx.kind==='collective')!==(ry.kind==='collective'))return 'kind';
  }
  return null;
}

/** The ID of a new member: the founding row's digest, longer while the ledger holds that ID for another row or holds
 *  it redirected or retired (a member split off a merged one gets an ID of its own; the old redirect stays). */
export function mintId(foundingRow,usedIds,ledger){
  const digest=base32(sha(`tos.population|${foundingRow}`));
  let length=10,id=`m-${digest.slice(0,length)}`;
  const reusable=id=>{const entry=ledger.get(id);return entry?.founded_on===foundingRow&&!entry.state;};
  while(usedIds.has(id)&&!reusable(id)){length+=2;id=`m-${digest.slice(0,length)}`;}
  return id;
}

function readLedger(review){
  const ledger=new Map(),order=[];
  for(const line of readJsonl(join(review,'ids.jsonl'))){
    if(!ledger.has(line.id))order.push(line.id);
    ledger.set(line.id,{...(ledger.get(line.id)??{}),...line});
  }
  return {ledger,order};
}

function finalId(ledger,id){
  const seen=new Set();
  while(ledger.get(id)?.state==='redirect'){
    if(seen.has(id))throw new Error(`resolve: redirect cycle at ${id}`);
    seen.add(id);id=ledger.get(id).to;
  }
  return id;
}

function treeAgents(root){
  const dir=join(root,'ToS','source-witnesses','agents'),agents=new Map();
  if(!existsSync(dir))return agents;
  for(const slug of readdirSync(dir).sort()){
    const path=join(dir,slug,'agent.json');
    if(!existsSync(path))continue;
    const record=JSON.parse(readFileSync(path,'utf8'));
    const labels=[record.preferred_label,...(record.variant_labels??[]).map(item=>item.value)].filter(Boolean);
    agents.set(slug,{id:record.record_id,folds:new Set(labels.map(fold))});
  }
  return agents;
}

function memberName(rows){
  const latin=rows.map(row=>row.names.latin).find(Boolean);
  return latin??rows[0].names.ru??rows[0].forms[0]?.text??rows[0].row;
}

function proposeSlug(rows,taken,tree,id){
  for(const row of rows)for(const value of row.existing??[]){
    if(value.startsWith('tos.agent.')){const slug=value.slice('tos.agent.'.length);if(!taken.has(slug))return {slug,tree:'declared'};}
  }
  let slug=slugify(memberName(rows))||id;
  const agent=tree.get(slug);
  if(agent&&rows.some(row=>row.forms.some(form=>agent.folds.has(fold(form.text))))&&!taken.has(slug))return {slug,tree:'label'};
  if(!taken.has(slug)&&!tree.has(slug))return {slug};
  const birth=rows.map(row=>row.years.birth?.year).find(year=>year!==undefined);
  if(birth!==undefined){
    const dated=`${slug}-${birth<1?`${1-birth}bce`:birth}`;
    if(!taken.has(dated)&&!tree.has(dated))return {slug:dated};
  }
  return {slug:`${slug}-${id.slice(2,7)}`};
}

const AUTHOR_ROLE=/автор|author|составител|составлен|атрибуц|attribut|compil|произнесени|сочин|написа/i;
const TRANSLATOR_ROLE=/перевод|translat/i;
const AGENT_ID=/\b((?:A\d{2}|T[23]-\d{2})-A\d{3,4})\b/g;

/** For every work of the registers: its responsibility lines, each with its role and the agent rows it names. */
function loadResponsibility(review){
  const dir=join(review,'registry'),works=new Map();
  for(const batch of readdirSync(dir).sort()){
    const path=join(dir,batch,'works-responsibility.jsonl');
    if(!existsSync(path))continue;
    for(const row of readJsonl(path).slice(1)){
      const items=[];
      for(const line of String(row.responsibility_statements??'').split('\n')){
        const agents=[...line.matchAll(AGENT_ID)].map(match=>`${match[1].replace(/-A\d+$/,'')}:${match[1]}`);
        if(agents.length)items.push({role:line.split(/[;|]/)[0].trim(),agents});
      }
      works.set(row.record_id.trim(),items);
    }
  }
  return works;
}

/**
 * The authors table's names bound to register rows. A name is bound through the works it is written
 * on: each unit of the table's sheet «Произведения» whose author field holds the name names its works,
 * and each work's responsibility lines name agent rows; among them the row bearing the name, else the
 * row sharing a key with it, else the work's single author. Only a name whose works give no row is
 * bound by its form within the batches the table names. A work that leaves several rows is an
 * ambiguity for a reader.
 */
function bindCore(review,rows){
  const names=readJsonl(join(review,'registry','authors-table','names.jsonl'));
  const units=readJsonl(join(review,'registry','authors-table','works.jsonl'));
  if(!names.length||!units.length)throw new Error('resolve: no registry/authors-table/ (run intake.py, or fetch the layers)');
  const [nameColumn,typeColumn,,,batchColumn]=names[0].columns;
  const [,,authorColumn,workColumn]=units[0].columns;
  const byRow=new Map(rows.map(row=>[row.row,row]));
  const responsibility=loadResponsibility(review);
  const unitsByFold=new Map();
  for(const unit of units.slice(1)){
    for(const part of String(unit[authorColumn]??'').split(/\s*;\s*/)){
      const key=fold(part);
      if(!key)continue;
      if(!unitsByFold.has(key))unitsByFold.set(key,[]);
      unitsByFold.get(key).push(unit);
    }
  }
  const byBatch=new Map();
  for(const row of rows){
    if(!byBatch.has(row.batch))byBatch.set(row.batch,[]);
    byBatch.get(row.batch).push(row);
  }
  const out=[];
  for(const entry of names.slice(1)){
    const name=String(entry[nameColumn]??'').trim();
    const key=fold(name),nameKeys=new Set(formKeys(name).map(item=>item.key));
    const bound=new Map(),ambiguous=[];
    let works=0;
    for(const unit of unitsByFold.get(key)??[]){
      for(const work of String(unit[workColumn]??'').split(/[;,\s]+/).filter(Boolean)){
        const items=responsibility.get(work);
        if(!items)continue;
        works++;
        const agents=[...new Set(items.flatMap(item=>item.agents))].filter(id=>byRow.has(id));
        const byName=agents.filter(id=>byRow.get(id).forms.some(form=>fold(form.text)===key));
        const byKey=agents.filter(id=>[...byRow.get(id).keys.keys()].some(k=>nameKeys.has(k)));
        const authors=[...new Set(items.filter(item=>AUTHOR_ROLE.test(item.role)&&!TRANSLATOR_ROLE.test(item.role)).flatMap(item=>item.agents))].filter(id=>byRow.has(id));
        const choice=byName.length?[byName,'work+name']:byKey.length?[byKey,'work+key']:authors.length?[authors,'work+role']:null;
        if(!choice)continue;
        if(choice[0].length>1){ambiguous.push({work,rows:choice[0].sort()});continue;}
        if(!bound.has(choice[0][0]))bound.set(choice[0][0],choice[1]);
      }
    }
    if(!bound.size&&!ambiguous.length){
      for(const batch of String(entry[batchColumn]??'').split(/[;,\s]+/).filter(Boolean).sort()){
        const found=(byBatch.get(batch)??[]).filter(row=>row.forms.some(form=>fold(form.text)===key)).map(row=>row.row);
        if(found.length===1)bound.set(found[0],'name');
        else if(found.length>1)ambiguous.push({batch,rows:found.sort()});
      }
    }
    const state=bound.size?(ambiguous.length?'bound-ambiguous':'bound'):(ambiguous.length?'ambiguous':'unbound');
    const record={name,sheet_row:entry.row,type:entry[typeColumn]??null,state,works,rows:Object.fromEntries([...bound].sort(([a],[b])=>a<b?-1:a>b?1:0))};
    if(ambiguous.length)record.ambiguous=ambiguous;
    out.push(record);
  }
  const counts={},bases={};
  for(const item of out){
    counts[item.state]=(counts[item.state]??0)+1;
    for(const basis of Object.values(item.rows))bases[basis]=(bases[basis]??0)+1;
  }
  return {
    made:'bonsai/review/population/resolve.mjs',
    posture:{
      ru:'Имена таблицы авторов философского корпуса и строки реестров персон, которые они называют. Имя привязывается через свои произведения: строки ответственности произведения называют лица, и среди них берётся строка с этим именем, иначе строка с общим ключом, иначе единственный автор произведения. Только имя, чьи произведения не дали строки, ищется по форме в партиях таблицы. Произведение, оставляющее несколько строк, — неоднозначность для читателя.',
      en:'The names of the authors table of the philosophical corpus and the persons register rows they name. A name is bound through its works: a work\'s responsibility lines name agents, and among them the row bearing the name is taken, else the row sharing a key with it, else the work\'s single author. Only a name whose works give no row is looked for by its form in the batches the table names. A work that leaves several rows is an ambiguity for a reader.'
    },
    counts,
    bases,
    names:out
  };
}

/** Everything resolve writes, built in memory: {files: {path: text}, summary}. */
export function resolve({review=HERE,root=ROOT,tolerance=yearTolerance}={}){
  const rows=loadRows(review);
  const byRow=new Map(rows.map(row=>[row.row,row]));
  const decisions=readJsonl(join(review,'decisions.jsonl'));
  const kindOverride=new Map(),leftOut=new Map(),same=[],cannot=new Set(),settled=[];
  for(const decision of decisions){
    const indices=(decision.rows??[decision.row]).map(id=>{if(!byRow.has(id))throw new Error(`resolve: decision ${decision.id} names unknown row ${id}`);return byRow.get(id).index;});
    if(decision.type==='kind')kindOverride.set(indices[0],decision.kind);
    else if(decision.type==='left_out')leftOut.set(indices[0],{reason:decision.reason,by:decision.id});
    else if(decision.type==='same')for(let i=1;i<indices.length;i++)same.push([indices[0],indices[i],decision.id]);
    else if(decision.type==='different'||decision.type==='related'){cannot.add(`${Math.min(...indices)}|${Math.max(...indices)}`);settled.push(indices);}
    // A held member's question (its kind, name or years) settles no pair of members.
    else if(decision.type==='held'){if((decision.question??'identity')==='identity')settled.push(indices);}
    // A member's name in Latin script and its years are the planting's (plant.mjs); resolution reads what joins rows.
    else if(decision.type==='name'||decision.type==='life')continue;
    else throw new Error(`resolve: decision ${decision.id} has unknown type ${decision.type}`);
  }
  for(const row of rows){
    row.kind=kindOverride.get(row.index)??row.kind;
    row.keys=rowKeys(row);
    row.heads=new Set(row.forms.map(form=>formHead(form.text)).filter(head=>head&&!row.keys.has(head)));
    row.idSets=rowIds(row);
    row.years=lifeYears(row);
    if(row.status==='superseded'&&!leftOut.has(row.index))leftOut.set(row.index,{reason:'superseded',by:'register'});
  }
  const live=rows.filter(row=>!leftOut.has(row.index));
  const index=new Map();
  const note=(key,row)=>{if(!index.has(key))index.set(key,[]);index.get(key).push(row.index);};
  for(const row of live){
    for(const key of row.keys.keys())note(`k:${key}`,row);
    for(const head of row.heads)note(`h:${head}`,row);
    for(const [scheme,values] of row.idSets)for(const value of values)note(`i:${scheme}:${value}`,row);
  }
  const pairs=new Map();
  const pairOf=(x,y)=>{const [a,b]=x<y?[x,y]:[y,x],id=`${a}|${b}`;if(!pairs.has(id))pairs.set(id,{a,b,shared:[]});return pairs.get(id);};
  let capped=0;
  for(const [key,members] of [...index].sort(([a],[b])=>a<b?-1:a>b?1:0)){
    if(key.startsWith('h:')){
      // A name qualified by its origin meets the same name standing alone, never another origin.
      const bare=index.get(`k:${key.slice(2)}`)??[];
      if(!bare.length)continue;
      if(members.length+bare.length>KEY_CAP){capped++;continue;}
      for(const x of members)for(const y of bare)if(x!==y)pairOf(x,y).shared.push(`${key.slice(2)} …`);
      continue;
    }
    if(members.length<2)continue;
    if(key.startsWith('k:')&&members.length>KEY_CAP){capped++;continue;}
    for(let i=0;i<members.length;i++)for(let j=i+1;j<members.length;j++){
      const pair=pairOf(members[i],members[j]);
      if(key.startsWith('k:'))pair.shared.push(key.slice(2));
    }
  }
  const auto=[],queued=[],apart={};
  for(const pair of pairs.values()){
    const verdict=compare(rows[pair.a],rows[pair.b],tolerance);
    if(verdict.decision==='auto')auto.push({...pair,tier:verdict.tier});
    else if(verdict.decision==='queue')queued.push({...pair,reason:verdict.reason});
    else apart[verdict.reason]=(apart[verdict.reason]??0)+1;
  }
  auto.sort((x,y)=>TIERS.indexOf(x.tier)-TIERS.indexOf(y.tier)||x.a-y.a||x.b-y.b);
  const clusters=new Clusters(rows);
  const accepted=[],blocked=[];
  for(const [a,b,by] of same){
    if(clusters.find(a)===clusters.find(b)){accepted.push({a,b,tier:'decision',by});continue;}
    // Readers' decisions may join rows of one batch (a register can list one person twice). Only a reader's
    // `different` or `related` between the two sides refuses the join; the pair then goes back to readers.
    const left=clusters.members.get(clusters.find(a)),right=clusters.members.get(clusters.find(b));
    if(left.some(x=>right.some(y=>cannot.has(`${Math.min(x,y)}|${Math.max(x,y)}`)))){queued.push({a,b,shared:[],reason:'decision-conflict'});continue;}
    clusters.union(a,b);accepted.push({a,b,tier:'decision',by});
  }
  for(const edge of auto){
    if(clusters.find(edge.a)===clusters.find(edge.b)){accepted.push(edge);continue;}
    const refusal=tolerated(rows,clusters,edge.a,edge.b,cannot,tolerance);
    if(refusal){blocked.push({...edge,reason:`blocked:${refusal}`});continue;}
    clusters.union(edge.a,edge.b);accepted.push(edge);
  }

  // Members and their IDs.
  const previous=new Map(readJsonl(join(review,'keys.jsonl')).filter(line=>line.member).map(line=>[line.row,line.member]));
  const {ledger,order}=readLedger(review);
  const run=Math.max(0,...[...ledger.values()].map(entry=>entry.run??0));
  const tree=treeAgents(root);
  const roots=[...clusters.members.keys()].filter(r=>!leftOut.has(r)||clusters.members.get(r).length>1).sort((x,y)=>clusters.members.get(x)[0]-clusters.members.get(y)[0]);
  const owner=new Map();
  const founding=new Map([...ledger.values()].filter(entry=>entry.founded_on).map(entry=>[entry.id,entry.founded_on]));
  for(const r of roots){
    const ids=clusters.members.get(r).filter(i=>!leftOut.has(i));
    for(const i of ids){
      const before=previous.get(rows[i].row);
      if(!before)continue;
      const id=finalId(ledger,before);
      const home=founding.get(id);
      const holds=home&&ids.some(k=>rows[k].row===home);
      const homeLive=home&&byRow.has(home)&&!leftOut.has(byRow.get(home).index);
      if(holds||!homeLive){
        if(!owner.has(id))owner.set(id,new Set());
        owner.get(id).add(r);
      }
    }
  }
  for(const [id,owners] of owner)if(owners.size>1){
    const keep=[...owners].sort((x,y)=>clusters.members.get(x)[0]-clusters.members.get(y)[0])[0];
    owner.set(id,new Set([keep]));
  }
  const claimed=new Map();
  for(const [id,owners] of owner)for(const r of owners){if(!claimed.has(r))claimed.set(r,[]);claimed.get(r).push(id);}
  const taken=new Set([...ledger.values()].map(entry=>entry.slug).filter(Boolean));
  const usedIds=new Set(ledger.keys());
  const additions=[],members=[];
  const position=id=>order.indexOf(id);
  for(const r of roots){
    const ids=clusters.members.get(r).filter(i=>!leftOut.has(i));
    if(!ids.length)continue;
    const own=(claimed.get(r)??[]).sort((x,y)=>(ledger.get(x).run??0)-(ledger.get(y).run??0)||position(x)-position(y));
    let id;
    if(own.length){
      id=own[0];
      for(const other of own.slice(1))additions.push({id:other,state:'redirect',to:id,by:'resolve:merge',run:run+1});
    }else{
      const foundingRow=rows[ids[0]].row;
      id=mintId(foundingRow,usedIds,ledger);
      const memberRows=ids.map(i=>rows[i]);
      const proposal=proposeSlug(memberRows,taken,tree,id);
      taken.add(proposal.slug);usedIds.add(id);
      const entry={id,founded_on:foundingRow,slug:proposal.slug,run:run+1};
      if(proposal.tree)entry.tree=proposal.tree;
      additions.push(entry);
      ledger.set(id,entry);
    }
    members.push({id,root:r,rows:ids});
  }
  const alive=new Set(members.map(member=>member.id));
  const merged=new Set(additions.filter(entry=>entry.state==='redirect').map(entry=>entry.id));
  for(const id of order){
    const entry=ledger.get(id);
    if(entry.state||alive.has(id)||merged.has(id))continue;
    additions.push({id,state:'retired',by:'resolve:no-rows',run:run+1});
  }

  // keys.jsonl
  const memberOf=new Map();
  for(const member of members)for(const i of member.rows)memberOf.set(i,member);
  const edgesOf=new Map();
  for(const edge of accepted)for(const i of [edge.a,edge.b]){if(!edgesOf.has(i))edgesOf.set(i,[]);edgesOf.get(i).push(edge.tier==='decision'?`decision:${edge.by}`:edge.tier);}
  const keysLines=rows.map(row=>{
    const gone=leftOut.get(row.index);
    if(gone)return {row:row.row,left_out:gone.reason,by:gone.by};
    const member=memberOf.get(row.index);
    const basis=member.rows[0]===row.index?'founding':(edgesOf.get(row.index)??[]).sort((x,y)=>TIERS.indexOf(x.split(':')[0])-TIERS.indexOf(y.split(':')[0]))[0]??'cluster';
    return {row:row.row,member:member.id,basis};
  });

  // The queue: one item per pair of members, with its strongest reason.
  const REASONS=['decision-conflict','blocked:date-conflict','blocked:kind','blocked:id-conflict','id-conflict','id-with-date-conflict','kind','disputed','date-conflict','strong-undated','short-partial','short-conflict','short-only','contained','blocked:same-batch','blocked:decided-different'];
  const items=new Map();
  const answered=new Set(settled.map(([x,y])=>{const mx=memberOf.get(x)?.id,my=memberOf.get(y)?.id;return [mx,my].sort().join('|');}));
  for(const pair of [...queued,...blocked]){
    const ma=memberOf.get(pair.a),mb=memberOf.get(pair.b);
    if(!ma||!mb||ma===mb)continue;
    if(answered.has([ma.id,mb.id].sort().join('|'))&&pair.reason!=='decision-conflict')continue;
    if(pair.reason==='blocked:same-batch'||pair.reason==='blocked:decided-different')continue;
    const [first,second]=ma.rows[0]<mb.rows[0]?[ma,mb]:[mb,ma];
    const id=`${first.id}|${second.id}`;
    const known=items.get(id);
    const shared=[...new Set([...(known?.shared??[]),...pair.shared])].sort();
    const reason=!known||REASONS.indexOf(pair.reason)<REASONS.indexOf(known.reason)?pair.reason:known.reason;
    items.set(id,{a:first,b:second,reason,shared});
  }
  const queue=[...items.values()].sort((x,y)=>x.a.rows[0]-y.a.rows[0]||x.b.rows[0]-y.b.rows[0]).map(item=>({
    item:`q-${base32(sha(`identity|${item.a.id}|${item.b.id}`)).slice(0,12)}`,
    type:'identity',
    reason:item.reason,
    shared:item.shared,
    a:{member:item.a.id,rows:item.a.rows.map(i=>rows[i].row)},
    b:{member:item.b.id,rows:item.b.rows.map(i=>rows[i].row)}
  }));

  const core=bindCore(review,rows);
  const lines=list=>list.map(item=>JSON.stringify(item)+'\n').join('');
  const files={
    'keys.jsonl':lines(keysLines),
    'ids.jsonl':(existsSync(join(review,'ids.jsonl'))?readFileSync(join(review,'ids.jsonl'),'utf8'):'')+lines(additions),
    'core.json':JSON.stringify(core,null,1)+'\n',
    'queue/identity.jsonl':lines(queue)
  };
  const reasons={};
  for(const item of queue)reasons[item.reason]=(reasons[item.reason]??0)+1;
  const tiers={};
  for(const edge of accepted)tiers[edge.tier]=(tiers[edge.tier]??0)+1;
  const sizes={};
  for(const member of members)sizes[member.rows.length]=(sizes[member.rows.length]??0)+1;
  const summary={rows:rows.length,left_out:leftOut.size,members:members.length,joined_rows:members.filter(m=>m.rows.length>1).reduce((n,m)=>n+m.rows.length,0),
    multi_row_members:members.filter(m=>m.rows.length>1).length,minted:additions.filter(entry=>!entry.state).length,redirects:additions.filter(entry=>entry.state==='redirect').length,
    accepted:tiers,blocked:blocked.length,queue:queue.length,queue_reasons:reasons,apart,capped_keys:capped,member_sizes:sizes,core:core.counts};
  return {files,summary,rows,members};
}

function main(){
  const check=process.argv.includes('--check');
  const {files,summary}=resolve();
  const differ=[];
  for(const [rel,text] of Object.entries(files)){
    const path=join(HERE,rel);
    if(check){
      if(!existsSync(path)||readFileSync(path,'utf8')!==text)differ.push(rel);
      continue;
    }
    mkdirSync(dirname(path),{recursive:true});
    writeFileSync(`${path}.tmp`,text);
    renameSync(`${path}.tmp`,path);
  }
  if(check){
    if(differ.length){console.log(`resolve: ${differ.join(', ')} differ from what the rows and decisions give`);process.exit(1);}
    console.log(`resolve: matches what the rows and decisions give (${summary.members} members from ${summary.rows} rows)`);
  }else console.log(JSON.stringify(summary,null,1));
}

if(process.argv[1]===fileURLToPath(import.meta.url))main();

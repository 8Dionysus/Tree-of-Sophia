#!/usr/bin/env node
// The chronology of the Aesthetics bonsai: the lives of its authors and the datings of its works.
//
//   node bonsai/review/aesthetics/chronology.mjs            write bonsai/aesthetics/chronology.json
//   node bonsai/review/aesthetics/chronology.mjs --fetch    first read from Wikidata the lives the layer does not hold yet
//   node bonsai/review/aesthetics/chronology.mjs --check    exit 1 when the module on disk differs from what the layer gives
//
// A life is read from Wikidata (layer/lives.json keeps what was fetched, with the item and the day). The dating of a
// work is the register's own: its wording, the year the layer reads in it, and the source the register names for the
// work. A work the register dates by a century stands inside its author's life and is no firmer than that life.

import {readFileSync,writeFileSync,renameSync,readdirSync,existsSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chronologyIssues} from '../../lib/chronology.mjs';

const here=dirname(fileURLToPath(import.meta.url)),root=join(here,'..','..','..');
const read=path=>JSON.parse(readFileSync(path,'utf8'));
const layer=name=>read(join(here,'layer',`${name}.json`));
const LIVES=join(here,'layer','lives.json'),OUT=join(root,'bonsai','aesthetics','chronology.json');
const AGENT='ToS-bonsai/1 (+https://github.com/8Dionysus/Tree-of-Sophia)';
// The authors read by hand before the planting line: the line's packets name the Wikidata item of every other author.
const ITEMS={'participant-Q77045':'Q77045','participant-Q6221660':'Q6221660',plato:'Q859',plotinus:'Q134189',aristotle:'Q868','immanuel-kant':'Q9312','edmund-burke':'Q165792','william-hogarth':'Q171344'};

const library=read(join(root,'bonsai','aesthetics','library.json'));
const packets=join(here,'planting','packets');
const agents=new Map(layer('agents').map(agent=>[agent.id,agent]));
const names=person=>[person?.name?.en,person?.name?.ru,person?.name?.original].filter(Boolean).map(value=>value.toLocaleLowerCase().normalize('NFKC').trim());
const samePerson=(person,agent)=>names(person).some(name=>names(agent).includes(name));
for(const name of existsSync(packets)?readdirSync(packets):[]){
  const at=join(packets,name),file=join(at,'evidence.json');
  const persons=existsSync(file)?[...(read(file).persons??[])]:[];
  const witnessDir=join(at,'witnesses');
  for(const witness of existsSync(witnessDir)?readdirSync(witnessDir).filter(file=>file.endsWith('.json')):[]){
    if(existsSync(join(witnessDir,`${witness}.accepted`)))persons.push(...(read(join(witnessDir,witness)).witness?.persons??[]));
  }
  const authors=library.works[name]?.agents??[];
  for(const person of persons){
    const item=person.qid??/^https:\/\/www\.wikidata\.org\/wiki\/(Q[1-9]\d*)\/?$/.exec(person.source??'')?.[1];
    if(!item)continue;
    const matches=authors.filter(agent=>samePerson(person,agents.get(agent)));
    if(person.role==='author'){
      if(matches.length===1&&!ITEMS[matches[0]])ITEMS[matches[0]]=item;
      else if(authors.length===1&&persons.filter(value=>value.role==='author').length===1&&!ITEMS[authors[0]])ITEMS[authors[0]]=item;
    }
    ITEMS[`participant-${item}`]=item;
  }
}

const args=new Set(process.argv.slice(2));
const lives=existsSync(LIVES)?read(LIVES):{};
const saveLives=()=>{
  const temporary=`${LIVES}.tmp-${process.pid}`;
  writeFileSync(temporary,`${JSON.stringify(Object.fromEntries(Object.keys(lives).sort().map(key=>[key,lives[key]])),null,1)}\n`);
  renameSync(temporary,LIVES);
};
// Open Library can bridge a catalogued work and author to the author's
// explicitly linked Wikidata item. Validate all three identities before use.
if(args.has('--fetch-queue-catalogue')){
  const byAuthor=new Map();
  for(const work of layer('works'))for(const id of work.agents??[]){
    if(!byAuthor.has(id))byAuthor.set(id,[]);byAuthor.get(id).push(work);
  }
  const fold=value=>String(value??'').normalize('NFKC').toLocaleLowerCase().replace(/[^\p{L}\p{N}]+/gu,' ').trim();
  const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
  const request=async url=>{
    await pause(1200);
    const response=await fetch(url,{headers:{'User-Agent':AGENT},signal:AbortSignal.timeout(30000)});
    if(!response.ok)throw new Error(`HTTP ${response.status}`);
    return response.json();
  };
  let completed=0,identified=0;
  for(const [id,works] of byAuthor){
    if(lives[id]?.item||(lives[id]?.catalogue_search?.complete&&lives[id].catalogue_search.version==='authority-v3'))continue;
    const agent=agents.get(id),name=agent?.name?.en;
    const titles=[...new Set(works.flatMap(work=>[work.title?.original,work.title?.en]).filter(Boolean))].slice(0,3);
    if(!name||!titles.length)continue;
    try{
      const choices=new Map();
      for(const title of titles){
        const url=new URL('https://openlibrary.org/search.json');
        for(const [key,value] of Object.entries({title,author:name,fields:'key,title,author_name,author_key',limit:'5'}))url.searchParams.set(key,value);
        const found=await request(url);
        for(const work of found.docs??[]){
          if(!/^\/works\/OL[1-9]\d*W$/.test(work.key??'')||fold(work.title)!==fold(title)||!(work.author_name??[]).some(value=>fold(value)===fold(name)))continue;
          for(const author of work.author_key??[]){
            if(!/^OL[1-9]\d*A$/.test(author))continue;
            const record=await request(`https://openlibrary.org/authors/${author}.json`);
            const names=[record.name,record.personal_name,...(record.alternate_names??[])].filter(Boolean).map(fold);
            if(!names.includes(fold(name)))continue;
            let qid=record.remote_ids?.wikidata;
            // National authority identifiers can bridge records that lack a direct Wikidata link.
            const authority=record.remote_ids?.viaf?['P214',record.remote_ids.viaf]:record.remote_ids?.lc_naf?['P244',record.remote_ids.lc_naf]:null;
            if(!/^Q[1-9]\d*$/.test(qid??'')&&authority){
              const query=`SELECT DISTINCT ?person WHERE { ?person wdt:${authority[0]} ${JSON.stringify(String(authority[1]))}. } LIMIT 3`;
              const source=new URL('https://query.wikidata.org/sparql');source.searchParams.set('query',query);source.searchParams.set('format','json');
              const answer=await request(source),ids=[...new Set((answer.results?.bindings??[]).map(row=>row.person.value.split('/').at(-1)))];
              if(ids.length===1)qid=ids[0];
            }
            let matchedYears=null;
            if(!/^Q[1-9]\d*$/.test(qid??'')){
              const exactYear=value=>{
                const text=String(value??'');if(/(?:\bca\.?|\bcirca\b|\bapprox|[?~])/i.test(text))return null;
                const years=[...text.matchAll(/\b(\d{4})\b/g)];return years.length===1?years[0][1]:null;
              };
              const born=exactYear(record.birth_date),died=exactYear(record.death_date);
              if(born&&died){
                const query=`SELECT DISTINCT ?person WHERE { ?person (rdfs:label|skos:altLabel) ${JSON.stringify(name)}@en; wdt:P31 wd:Q5; wdt:P569 ?born; wdt:P570 ?died. FILTER(YEAR(?born)=${born} && YEAR(?died)=${died}) } LIMIT 3`;
                const source=new URL('https://query.wikidata.org/sparql');source.searchParams.set('query',query);source.searchParams.set('format','json');
                const answer=await request(source),ids=[...new Set((answer.results?.bindings??[]).map(row=>row.person.value.split('/').at(-1)))];
                if(ids.length===1){qid=ids[0];matchedYears={born:Number(born),died:Number(died)};}
              }
            }
            if(!/^Q[1-9]\d*$/.test(qid??''))continue;
            const entity=(await request(`https://www.wikidata.org/wiki/Special:EntityData/${qid}.json`)).entities?.[qid];
            if(!entity)continue;
            const labels=[...Object.values(entity.labels??{}).map(value=>value.value),...Object.values(entity.aliases??{}).flatMap(items=>items.map(value=>value.value))].filter(Boolean).map(fold);
            const human=(entity.claims?.P31??[]).some(claim=>claim.mainsnak?.datavalue?.value?.id==='Q5');
            if(!human||!labels.includes(fold(name)))continue;
            if(matchedYears){
              const agrees=(property,year)=>{
                const claims=(entity.claims?.[property]??[]).filter(claim=>claim.rank!=='deprecated'&&claim.mainsnak.datavalue),preferred=claims.filter(claim=>claim.rank==='preferred');
                return (preferred.length?preferred:claims).some(claim=>claim.mainsnak.datavalue.value.precision>=9&&Number(/^([+-]\d+)-/.exec(claim.mainsnak.datavalue.value.time)?.[1])===year);
              };
              if(!agrees('P569',matchedYears.born)||!agrees('P570',matchedYears.died))continue;
            }
            choices.set(qid,{entity,author,work:work.key,title,authority,matchedYears});
          }
        }
      }
      const catalogue_search={complete:true,version:'authority-v3',titles,source:'https://openlibrary.org/',checked:new Date().toISOString().slice(0,10)};
      if(choices.size===1){
        const [qid,record]=[...choices][0],entity=record.entity;
        const times=property=>{
          const claims=(entity.claims?.[property]??[]).filter(claim=>claim.rank!=='deprecated'&&claim.mainsnak.datavalue),preferred=claims.filter(claim=>claim.rank==='preferred');
          return (preferred.length?preferred:claims).map(claim=>({time:claim.mainsnak.datavalue.value.time,precision:claim.mainsnak.datavalue.value.precision}));
        };
        lives[id]={item:qid,label:{en:entity.labels?.en?.value??null,ru:entity.labels?.ru?.value??null},born:times('P569'),died:times('P570'),fetched:new Date().toISOString().slice(0,10),identity_catalogue:{author:record.author,author_url:`https://openlibrary.org/authors/${record.author}`,work:record.work,work_url:`https://openlibrary.org${record.work}`,title:record.title,...(record.authority?{authority:record.authority}: {}),...(record.matchedYears?{matched_years:record.matchedYears}: {})},catalogue_search};
        identified++;console.log(`${id}: ${qid}, Open Library ${record.author} ${record.work}`);
      }else lives[id]={...(lives[id]??{item:null,label:agent?.name??{},born:[],died:[]}),status:'unresolved',reason:choices.size?'multiple catalogued identities':'no unique catalogued identity in the listed works',catalogue_search};
      completed++;saveLives();
    }catch(error){console.error(`catalogue search stopped at ${id}: ${error}`);break;}
  }
  console.log(`queue catalogue: ${completed} searched, ${identified} identified`);
}

// Persist each author's exact-label/P50 search independently, so rate limits
// stop the operation without losing completed searches or inventing identities.
if(args.has('--fetch-queue-relations')){
  const byAuthor=new Map();
  for(const work of layer('works'))for(const id of work.agents??[]){
    if(!byAuthor.has(id))byAuthor.set(id,[]);byAuthor.get(id).push(work);
  }
  let completed=0,identified=0;
  for(const [id,works] of byAuthor){
    if(lives[id]?.item||lives[id]?.relation_search?.complete)continue;
    const name=agents.get(id)?.name?.en;
    const titles=[...new Set(works.flatMap(work=>[work.title?.en,work.title?.original,work.title?.ru]).filter(Boolean))].slice(0,12);
    if(!name||!titles.length)continue;
    const values=titles.flatMap(title=>['en','de','fr','es','it','la','ru'].map(language=>`(${JSON.stringify(name)}@en ${JSON.stringify(title)}@${language})`)).join(' ');
    const query=`SELECT DISTINCT ?person ?work WHERE { VALUES (?name ?title) { ${values} } ?work (rdfs:label|skos:altLabel) ?title; wdt:P50 ?person. ?person (rdfs:label|skos:altLabel) ?name. }`;
    try{
      const response=await fetch('https://query.wikidata.org/sparql',{method:'POST',body:new URLSearchParams({query,format:'json'}),headers:{'User-Agent':AGENT,'Accept':'application/sparql-results+json','Content-Type':'application/x-www-form-urlencoded'},signal:AbortSignal.timeout(45000)});
      if(!response.ok)throw new Error(`SPARQL ${response.status}`);
      const choices=new Map();
      for(const row of (await response.json()).results.bindings){
        const qid=row.person.value.split('/').at(-1),work=row.work.value.split('/').at(-1);
        if(/^Q[1-9]\d*$/.test(qid)&&/^Q[1-9]\d*$/.test(work))choices.set(qid,work);
      }
      const relation_search={complete:true,titles,source:'https://query.wikidata.org/',checked:new Date().toISOString().slice(0,10)};
      if(choices.size===1){
        const [qid,work]=[...choices][0],source=await fetch(`https://www.wikidata.org/wiki/Special:EntityData/${qid}.json`,{headers:{'User-Agent':AGENT},signal:AbortSignal.timeout(30000)});
        if(!source.ok)throw new Error(`Wikidata ${source.status}`);
        const entity=(await source.json()).entities[qid];
        const times=property=>{
          const claims=(entity.claims?.[property]??[]).filter(claim=>claim.rank!=='deprecated'&&claim.mainsnak.datavalue),preferred=claims.filter(claim=>claim.rank==='preferred');
          return (preferred.length?preferred:claims).map(claim=>({time:claim.mainsnak.datavalue.value.time,precision:claim.mainsnak.datavalue.value.precision}));
        };
        lives[id]={item:qid,label:{en:entity.labels?.en?.value??null,ru:entity.labels?.ru?.value??null},born:times('P569'),died:times('P570'),fetched:new Date().toISOString().slice(0,10),identity_work:work,relation_search};
        identified++;console.log(`${id}: ${qid}, evidenced by ${work}`);
      }else lives[id]={...(lives[id]??{item:null,label:agents.get(id)?.name??{},born:[],died:[]}),status:'unresolved',reason:choices.size?'multiple exact work-author identities':'no exact match among the listed titles',relation_search};
      completed++;saveLives();
    }catch(error){console.error(`queue relation search stopped at ${id}: ${error}`);break;}
    await new Promise(resolve=>setTimeout(resolve,1500));
  }
  console.log(`queue relations: ${completed} searched, ${identified} identified`);
}
// A second evidence route identifies a unique exact name when both lifetime
// endpoints also agree with the register. A single name or date is insufficient.
if(args.has('--fetch-queue-dates')){
  const entries=[];
  for(const [id,agent] of agents){
    if(lives[id]?.item||!agent.name?.en)continue;
    const match=(agent.dates??[]).map(value=>/^\s*(\d{4})\s*[–-]\s*(\d{4})\s*$/.exec(value)).find(Boolean);
    if(match)entries.push({id,name:agent.name.en,born:Number(match[1]),died:Number(match[2])});
  }
  const hits=new Map();let complete=true;
  for(let start=0;start<entries.length;start+=5){
    const batch=entries.slice(start,start+5),values=batch.map(item=>`(${JSON.stringify(item.name)}@en ${item.born} ${item.died})`).join(' ');
    const query=`SELECT DISTINCT ?name ?birthYear ?deathYear ?person WHERE { VALUES (?name ?birthYear ?deathYear) { ${values} } ?person (rdfs:label|skos:altLabel) ?name; wdt:P31 wd:Q5; wdt:P569 ?born; wdt:P570 ?died. FILTER(YEAR(?born)=?birthYear && YEAR(?died)=?deathYear) }`;
    try{
      const response=await fetch('https://query.wikidata.org/sparql',{method:'POST',body:new URLSearchParams({query,format:'json'}),headers:{'User-Agent':AGENT,'Accept':'application/sparql-results+json','Content-Type':'application/x-www-form-urlencoded'},signal:AbortSignal.timeout(45000)});
      if(!response.ok)throw new Error(`SPARQL ${response.status}`);
      for(const row of (await response.json()).results.bindings){
        const qid=row.person.value.split('/').at(-1);
        for(const item of batch.filter(item=>item.name===row.name.value&&item.born===Number(row.birthYear.value)&&item.died===Number(row.deathYear.value))){
          if(!hits.has(item.id))hits.set(item.id,new Set());hits.get(item.id).add(qid);
        }
      }
    }catch(error){console.error(`date batch stopped: ${error}`);complete=false;break;}
    await new Promise(resolve=>setTimeout(resolve,1000));
  }
  if(complete){
    const unique=entries.filter(item=>hits.get(item.id)?.size===1).map(item=>({...item,qid:[...hits.get(item.id)][0]}));
    for(let start=0;start<unique.length;start+=25){
      const batch=unique.slice(start,start+25),url=new URL('https://www.wikidata.org/w/api.php');
      for(const [key,value] of Object.entries({action:'wbgetentities',ids:batch.map(item=>item.qid).join('|'),props:'labels|claims',format:'json'}))url.searchParams.set(key,value);
      try{
        const response=await fetch(url,{headers:{'User-Agent':AGENT},signal:AbortSignal.timeout(30000)});if(!response.ok)throw new Error(`Wikidata ${response.status}`);
        const entities=(await response.json()).entities??{};
        for(const item of batch){
          const entity=entities[item.qid];if(!entity||entity.missing!==undefined)continue;
          const times=property=>{
            const claims=(entity.claims?.[property]??[]).filter(claim=>claim.rank!=='deprecated'&&claim.mainsnak.datavalue),preferred=claims.filter(claim=>claim.rank==='preferred');
            return (preferred.length?preferred:claims).map(claim=>({time:claim.mainsnak.datavalue.value.time,precision:claim.mainsnak.datavalue.value.precision}));
          };
          const born=times('P569'),died=times('P570');
          const hasYear=(values,year)=>values.some(value=>Number(/^([+-]\d+)-/.exec(value.time)?.[1])===year&&value.precision>=9);
          if(!hasYear(born,item.born)||!hasYear(died,item.died))continue;
          lives[item.id]={item:item.qid,label:{en:entity.labels?.en?.value??null,ru:entity.labels?.ru?.value??null},born,died,fetched:new Date().toISOString().slice(0,10),identity_register:{born:item.born,died:item.died}};
        }
        saveLives();
      }catch(error){console.error(`date entity batch stopped: ${error}`);complete=false;break;}
      await new Promise(resolve=>setTimeout(resolve,1000));
    }
  }
  console.log(`queue dates: ${entries.filter(item=>lives[item.id]?.item).length}/${entries.length} identified, complete=${complete}`);
}
// Batch the same exact work/author relationship against Wikidata's graph. A
// unique matching P50 identity is still required before any life is admitted.
if(args.has('--fetch-queue-batch')){
  const pairs=[],owners=new Map(),hits=new Map(),pending=new Set();
  for(const work of layer('works'))for(const id of work.agents??[]){
    if(lives[id]?.item)continue;
    const name=agents.get(id)?.name?.en,titles=[...new Set([work.title?.en,work.title?.original,work.title?.ru].filter(Boolean))];
    if(!name||!titles.length)continue;
    for(const title of titles){
      const key=JSON.stringify([name,title]);
      if(!owners.has(key)){owners.set(key,[]);pairs.push([name,title]);}
      owners.get(key).push(id);
    }
    pending.add(id);
    if(!lives[id])lives[id]={item:null,label:agents.get(id)?.name??{},born:[],died:[],status:'unsearched'};
  }
  saveLives();
  let complete=true;
  for(let start=0;start<pairs.length;start+=15){
    const batch=pairs.slice(start,start+15),values=batch.flatMap(([name,title])=>['en','de','fr','es','it','la','ru'].map(language=>`(${JSON.stringify(name)}@en ${JSON.stringify(title)}@${language})`)).join(' ');
    const query=`SELECT DISTINCT ?name ?title ?person ?work WHERE { VALUES (?name ?title) { ${values} } ?work (rdfs:label|skos:altLabel) ?title; wdt:P50 ?person. ?person (rdfs:label|skos:altLabel) ?name. }`;
    const body=new URLSearchParams({query,format:'json'});
    try{
      const response=await fetch('https://query.wikidata.org/sparql',{method:'POST',body,headers:{'User-Agent':AGENT,'Accept':'application/sparql-results+json','Content-Type':'application/x-www-form-urlencoded'},signal:AbortSignal.timeout(45000)});
      if(!response.ok)throw new Error(`SPARQL ${response.status}`);
      for(const row of (await response.json()).results.bindings){
        const key=JSON.stringify([row.name.value,row.title.value]),qid=row.person.value.split('/').at(-1),work=row.work.value.split('/').at(-1);
        if(!/^Q[1-9]\d*$/.test(qid)||!/^Q[1-9]\d*$/.test(work))continue;
        for(const id of owners.get(key)??[]){if(!hits.has(id))hits.set(id,new Map());hits.get(id).set(qid,work);}
      }
      console.log(`queue batch ${Math.floor(start/15)+1}/${Math.ceil(pairs.length/15)}: ${hits.size} candidate identities`);
    }catch(error){console.error(`queue batch stopped: ${error}`);complete=false;break;}
    await new Promise(resolve=>setTimeout(resolve,1000));
  }
  // A partial traversal cannot establish that a name has only one match.
  if(complete){
    const unique=[...hits].filter(([,choices])=>choices.size===1).map(([id,choices])=>({id,qid:[...choices.keys()][0],work:[...choices.values()][0]}));
    for(let start=0;start<unique.length;start+=25){
      const batch=unique.slice(start,start+25),url=new URL('https://www.wikidata.org/w/api.php');
      for(const [key,value] of Object.entries({action:'wbgetentities',ids:batch.map(item=>item.qid).join('|'),props:'labels|claims',format:'json'}))url.searchParams.set(key,value);
      try{
        const response=await fetch(url,{headers:{'User-Agent':AGENT},signal:AbortSignal.timeout(30000)});
        if(!response.ok)throw new Error(`Wikidata ${response.status}`);
        const entities=(await response.json()).entities??{};
        for(const item of batch){
          const entity=entities[item.qid];if(!entity||entity.missing!==undefined)continue;
          const times=property=>{
            const claims=(entity.claims?.[property]??[]).filter(claim=>claim.rank!=='deprecated'&&claim.mainsnak.datavalue),preferred=claims.filter(claim=>claim.rank==='preferred');
            return (preferred.length?preferred:claims).map(claim=>({time:claim.mainsnak.datavalue.value.time,precision:claim.mainsnak.datavalue.value.precision}));
          };
          lives[item.id]={item:item.qid,label:{en:entity.labels?.en?.value??null,ru:entity.labels?.ru?.value??null},born:times('P569'),died:times('P570'),fetched:new Date().toISOString().slice(0,10),identity_work:item.work};
        }
        saveLives();
      }catch(error){console.error(`queue entity batch stopped: ${error}`);complete=false;break;}
      await new Promise(resolve=>setTimeout(resolve,1000));
    }
  }
  if(complete){
    for(const id of pending)if(!lives[id]?.item){
      const choices=hits.get(id);lives[id]={...lives[id],status:'unresolved',reason:choices?.size>1?'multiple exact work-author identities':'no exact work-author P50 match in the queried language labels',checked:new Date().toISOString().slice(0,10)};
    }
    saveLives();
  }
  console.log(`queue batch: ${[...pending].filter(id=>lives[id]?.item).length} identities established, complete=${complete}`);
}
// Queue identities are admitted only when the exact Wikidata work links the
// candidate by P50. A name search alone is not a disambiguation.
if(args.has('--fetch-queue')){
  const byAuthor=new Map();
  for(const work of layer('works'))for(const agent of work.agents??[]){
    if(!byAuthor.has(agent))byAuthor.set(agent,[]);byAuthor.get(agent).push(work);
  }
  const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
  const request=async url=>{
    await pause(1200);
    const response=await fetch(url,{headers:{'User-Agent':AGENT},signal:AbortSignal.timeout(15000)});
    if(response.status===429)throw new Error('Wikidata rate limit 429; stop and retry after the service recovers');
    if(!response.ok)throw new Error(`Wikidata ${response.status}`);
    return response;
  };
  const search=async text=>{
    const url=new URL('https://www.wikidata.org/w/api.php');
    for(const [key,value] of Object.entries({action:'wbsearchentities',search:text,language:'en',format:'json',limit:'8'}))url.searchParams.set(key,value);
    const response=await request(url);
    return (await response.json()).search??[];
  };
  const entity=async item=>{
    const response=await request(`https://www.wikidata.org/wiki/Special:EntityData/${item}.json`);
    return (await response.json()).entities[item];
  };
  const folded=value=>String(value??'').normalize('NFKC').toLocaleLowerCase().trim();
  const unresolved=[];
  for(const [id] of byAuthor)if(!lives[id])lives[id]={item:null,label:agents.get(id)?.name??{},born:[],died:[],status:'unsearched'};
  saveLives();
  for(const [id,works] of byAuthor){
    if(lives[id]?.item||(lives[id]?.status==='unresolved'&&!args.has('--retry-unresolved')))continue;
    const agent=agents.get(id);if(!agent?.name?.en){lives[id].status='unresolved';lives[id].reason='no English name in the register';unresolved.push({agent:id,reason:lives[id].reason});saveLives();continue;}
    const searched=[];
    try{
      searched.push({kind:'person',query:agent.name.en});
      const people=(await search(agent.name.en)).filter(item=>folded(item.label)===folded(agent.name.en));
      let found=null;
      for(const work of works.slice(0,3)){
        const title=work.title?.en??work.title?.original;if(!title)continue;
        searched.push({kind:'work',query:title});
        const choices=(await search(title)).filter(item=>folded(item.label)===folded(title)).slice(0,4);
        for(const choice of choices){
          const item=await entity(choice.id),authors=(item.claims?.P50??[]).map(claim=>claim.mainsnak?.datavalue?.value?.id).filter(Boolean);
          const matching=people.filter(person=>authors.includes(person.id));
          if(matching.length===1){found={person:matching[0].id,work:choice.id};break;}
        }
        if(found)break;
      }
      if(!found){lives[id]={...lives[id],status:'unresolved',reason:'exact work-to-author P50 match not found',searched,checked:new Date().toISOString().slice(0,10)};unresolved.push({agent:id,reason:lives[id].reason});saveLives();continue;}
      const person=await entity(found.person);
      const times=property=>{
        const claims=(person.claims?.[property]??[]).filter(claim=>claim.rank!=='deprecated'&&claim.mainsnak.datavalue),preferred=claims.filter(claim=>claim.rank==='preferred');
        return (preferred.length?preferred:claims).map(claim=>({time:claim.mainsnak.datavalue.value.time,precision:claim.mainsnak.datavalue.value.precision}));
      };
      lives[id]={item:found.person,label:{en:person.labels.en?.value??null,ru:person.labels.ru?.value??null},born:times('P569'),died:times('P570'),fetched:new Date().toISOString().slice(0,10),identity_work:found.work};
      console.log(`${id}: ${found.person}, evidenced by ${found.work}`);
      saveLives();
    }catch(error){lives[id]={...lives[id],status:'unsearched',last_error:String(error)};unresolved.push({agent:id,reason:String(error)});saveLives();if(String(error).includes('rate limit 429')){console.error(String(error));break;}}
  }
  saveLives();
  console.log(`queue lives unresolved: ${unresolved.length}`);
  for(const entry of unresolved)console.log(`${entry.agent}: ${entry.reason}`);
}
if(args.has('--fetch')){
  for(const agent of new Set([...Object.keys(library.agents),...Object.keys(ITEMS).filter(key=>key.startsWith('participant-'))])){
    if(lives[agent]?.item)continue;
    const item=ITEMS[agent];if(!item)throw new Error(`${agent}: no Wikidata item is named for this author`);
    const answer=await fetch(`https://www.wikidata.org/wiki/Special:EntityData/${item}.json`,{headers:{'User-Agent':AGENT}});
    if(!answer.ok)throw new Error(`${agent}: Wikidata answers ${answer.status} for ${item}`);
    const entity=(await answer.json()).entities[item];
    const times=property=>{
      const claims=(entity.claims[property]??[]).filter(claim=>claim.rank!=='deprecated'&&claim.mainsnak.datavalue),preferred=claims.filter(claim=>claim.rank==='preferred');
      return (preferred.length?preferred:claims).map(claim=>({time:claim.mainsnak.datavalue.value.time,precision:claim.mainsnak.datavalue.value.precision}));
    };
    lives[agent]={item,label:{en:entity.labels.en?.value??null,ru:entity.labels.ru?.value??null},born:times('P569'),died:times('P570'),fetched:new Date().toISOString().slice(0,10)};
    await new Promise(resolve=>setTimeout(resolve,400));
  }
  saveLives();
}

// Wikidata writes a year before the era without a year zero; the Tree counts astronomically (1 BCE = 0).
const yearOf=time=>{const year=Number(/^([+-]\d+)-/.exec(time)[1]);return year<0?year+1:year;};
const lexeme=year=>`${year<0?'-':''}${String(Math.abs(year)).padStart(4,'0')}`;
const spoken=(years,locale)=>{                                        // 428/427 BCE, 1632
  const shown=[...new Set(years)].map(year=>year<=0?1-year:year).join('/'),before=years.some(year=>year<=0);
  return before?`${shown} ${locale==='ru'?'до н. э.':'BCE'}`:shown;
};
const between=(from,to,locale)=>{
  const before=from.some(year=>year<=0)&&to.some(year=>year<=0),bare=years=>[...new Set(years)].map(year=>year<=0?1-year:year).join('/');
  return before?`${bare(from)}–${bare(to)} ${locale==='ru'?'до н. э.':'BCE'}`:`${spoken(from,locale)}–${spoken(to,locale)}`;
};
const dating=(more)=>({role:'historical-time',calendar:null,year_numbering:'astronomical',...more});

const sources={},situations={},lifeOf={},workOf={};
for(const [agent,entry] of Object.entries(library.agents)){
  const life=lives[agent];if(!life?.item)throw new Error(`${agent}: no verified Wikidata identity in layer/lives.json (resolve it before fetching)`);
  const born=life.born.map(item=>yearOf(item.time)),died=life.died.map(item=>yearOf(item.time));
  const complete=born.length>0&&died.length>0;
  const exact=complete&&new Set(born).size===1&&new Set(died).size===1&&[...life.born,...life.died].every(item=>item.precision>=9);
  const id=`tos.biographical-phase.${agent}.lifetime`,source=`wikidata-${agent}`;
  const wording=complete?between(born,died,'en'):born.length?`born ${spoken(born,'en')}; no death year recorded`:died.length?`death ${spoken(died,'en')}; no birth year recorded`:'no birth or death year recorded';
  const russian=complete?between(born,died,'ru'):born.length?`год рождения ${spoken(born,'ru')}; год смерти не указан`:died.length?`год смерти ${spoken(died,'ru')}; год рождения не указан`:'годы рождения и смерти не указаны';
  sources[source]={short:'Wikidata',title:`${life.label.en??entry.name.en} (${life.item})`,url:`https://www.wikidata.org/wiki/${life.item}`};
  const lifeDating=complete?{kind:'interval-assertion',...dating({certainty:exact?'exact':'approximate',interval:{start:lexeme(Math.min(...born)),end:lexeme(Math.max(...died))},source_wording:{text:wording,language:'en'}})}
    :{kind:'unknown-date',...dating({certainty:'unknown',source_wording:{text:wording,language:'en'}})};
  situations[id]={type:'biographical-phase',subject:agent,label:{ru:`${entry.name.ru}: годы жизни`,en:`${entry.name.en}: lifetime`},
    dating:lifeDating,
    statement:{ru:`Викиданные: ${entry.name.ru}, ${russian}.`,en:`Wikidata: ${entry.name.en}, ${wording}.`},
    sources:[source],footing:'medium',epistemic:'reported'};
  lifeOf[agent]=id;
}

const works=new Map(layer('works').map(work=>[work.id,work])),named=new Map(layer('sources').map(source=>[source.key,source]));
for(const [work,entry] of Object.entries(library.works)){
  const record=works.get(work);if(!record)throw new Error(`${work}: the layer holds no such work`);
  const cited=(record.sources??[]).map(key=>named.get(key)).find(source=>/^https:\/\//.test(source?.url??''));
  if(!cited)throw new Error(`${work}: the register names no source with an address`);
  const wording=(record.dates??[])[0]??'',agent=entry.agents[0],source=`register-${cited.key.replace(/[^A-Za-z0-9]+/g,'-').toLowerCase()}`;
  sources[source]={short:cited.title.split(/[.:—]/)[0].trim().slice(0,40),title:cited.title,url:cited.url};
  // The register's first span of years is the work's own when it opens with the year the layer reads; a later span names an edition or a translation.
  const year=record.year,found=/(\d{4})\s*[–-]\s*(\d{4})/.exec(wording),span=found&&Number(found[1])===year?found:null;
  const when=span?{kind:'interval-assertion',...dating({certainty:wording.trim()===span[0]?'exact':'approximate',interval:{start:span[1],end:span[2]},source_wording:{text:wording,language:'ru'}})}
    :Number.isInteger(year)?{kind:'date-assertion',...dating({certainty:wording.trim()===String(year)?'exact':'approximate',value:lexeme(year),source_wording:{text:wording,language:'ru'}})}
    :situations[lifeOf[agent]]?.dating?.kind==='unknown-date'?{kind:'unknown-date',...dating({certainty:'unknown',source_wording:{text:wording||'Date not recorded',language:'ru'}})}
    :{kind:'relative-order',role:'historical-time',calendar:null,year_numbering:null,certainty:'approximate',relative:{relation:'during',anchor_ref:lifeOf[agent]},source_wording:{text:wording,language:'ru'}};
  const title={ru:entry.title.ru,en:entry.title.en??entry.title.original??entry.title.ru};
  const said=span?`${span[1]}–${span[2]}`:Number.isInteger(year)?String(year):`within the life of ${library.agents[agent].name.en}`;
  const id=`tos.historical-process.${work}-dating`;
  situations[id]={type:'historical-process',work,label:{ru:`Датировка: «${title.ru}»`,en:`Dating: ${title.en}`},dating:when,
    statement:{ru:`Реестр бонсая датирует произведение: ${wording}. Источник реестра: ${cited.title}.`,en:`The bonsai’s register dates the work ${said}. The register’s source: ${cited.title}.`},
    sources:[source],footing:'medium',epistemic:when.kind==='relative-order'?'inferred':'reported'};
  workOf[work]=id;
}

const checked=Object.values(lives).map(life=>life.fetched).filter(Boolean).sort().at(-1);
const module={schema:'tos.bonsai.chronology.v1',bonsai:'aesthetics',checked,
  posture:{ru:'Годы жизни авторов и датировки произведений в грамматике дат Древа. Жизнь прочитана в Викиданных; датировка произведения — слова реестра бонсая и источник, который реестр называет. Это предложение стенда.',
    en:'The lives of the authors and the datings of the works in the Tree’s date grammar. A life is read from Wikidata; the dating of a work is the wording of the bonsai’s register and the source the register names. It is the stand’s proposal.'},
  numbering:{year_numbering:'astronomical',calendar:null,note:{ru:'Годы до н. э. переведены в астрономический счёт (1 г. до н. э. = 0); календарь источники не называют, и он не утверждается.',en:'Years BCE are converted to astronomical numbering (1 BCE = 0); the sources name no calendar and none is asserted.'}},
  sources,lives:lifeOf,compositions:{works:workOf,passages:{}},situations};
const issues=chronologyIssues(module);
if(issues.length){console.error(issues.map(issue=>`chronology: ${issue}`).join('\n'));process.exit(1);}
const text=`${JSON.stringify(module,null,1)}\n`;
if(args.has('--check')){
  const same=existsSync(OUT)&&readFileSync(OUT,'utf8')===text;
  console.log(same?'chronology: as the layer gives it':'chronology: the module on disk differs from what the layer gives');process.exit(same?0:1);
}
writeFileSync(OUT,text);
console.log(`chronology: ${Object.keys(lifeOf).length} lives, ${Object.keys(workOf).length} works`);

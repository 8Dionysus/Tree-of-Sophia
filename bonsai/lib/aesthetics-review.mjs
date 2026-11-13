// Check the research layer against its source registers before it is planted.
import {readFileSync} from 'node:fs';
import {join} from 'node:path';

export function readAestheticsReview(root){
  const home=join(root,'bonsai/review/aesthetics');
  const read=path=>JSON.parse(readFileSync(path,'utf8'));
  const layer=Object.fromEntries(['areas','concepts','agents','works','readings','links','sources','keys'].map(name=>[name,read(join(home,'layer',`${name}.json`))]));
  const registers={};
  for(const sheet of ['readings','links','sources','concepts','agents','works']){
    registers[sheet]=[];
    for(const letter of 'ABCD'){
      const lines=readFileSync(join(home,'registry',letter,`${sheet}.jsonl`),'utf8').trimEnd().split(/\r?\n/).map(line=>JSON.parse(line));
      registers[sheet].push(...lines.slice(1).map(row=>`${letter}:${row[lines[0].columns[0]]}`));
    }
  }
  return {layer,registers};
}
export function aestheticsReviewIssues({layer,registers}){
  const {areas,concepts,agents,works,readings,links,sources,keys}=layer,issues=[];
  const require=(condition,message)=>{if(!condition)issues.push(message);};
  for(const sheet of ['readings','links','sources','concepts','agents','works']){
    const held=layer[sheet].flatMap(record=>['readings','links','sources'].includes(sheet)?[record.key]:record.keys);
    const unique=new Set(held),expected=new Set(registers[sheet]);
    require(held.length>0&&unique.size===held.length&&unique.size===expected.size&&[...unique].every(key=>expected.has(key)),`${sheet}: register rows must appear exactly once`);
  }
  const areaIds=new Set(areas.map(x=>x.id)),conceptIds=new Set(concepts.map(x=>x.key)),agentIds=new Set(agents.map(x=>x.id)),workIds=new Set(works.map(x=>x.id)),readingIds=new Set(readings.map(x=>x.key)),sourceIds=new Set(sources.map(x=>x.key));
  const subset=(items,set)=>items.every(item=>set.has(item));
  require(areaIds.size===36,'areas: expected 36 distinct IDs');
  for(const work of works){
    require(work.agents.length>0&&subset(work.agents,agentIds),`work ${work.id}: agents`);
    require(work.id.split('.')[0]===work.agents[0],`work ${work.id}: first agent`);
  }
  for(const reading of readings){
    const at=`reading ${reading.key}`;
    require(reading.areas.length>0&&subset(reading.areas,areaIds),`${at}: areas`);
    require(reading.concepts.length>0&&subset(reading.concepts,conceptIds),`${at}: concepts`);
    require(workIds.has(reading.work),`${at}: work`);
    require(reading.agents.length>0&&subset(reading.agents,agentIds),`${at}: agents`);
    require(subset(reading.sources,sourceIds),`${at}: sources`);
    require(Boolean(reading.locus&&reading.url&&Object.values(reading.thesis)[0]),`${at}: locus, URL or thesis`);
    require(!Object.hasOwn(reading,'concepts_unresolved')&&!Object.hasOwn(reading,'authors_unresolved'),`${at}: unresolved references`);
  }
  for(const link of links){
    for(const end of [link.from,link.to]){
      const entries=Object.entries(end),[kind,target]=entries[0]??[];
      require(entries.length===1&&({concept:conceptIds,reading:readingIds}[kind]?.has(target)??false),`link ${link.key}: endpoint`);
    }
    require(link.grounds.length>0&&subset(link.grounds,readingIds),`link ${link.key}: grounds`);
    require(!Object.hasOwn(link,'grounds_unresolved'),`link ${link.key}: unresolved grounds`);
  }
  for(const concept of concepts)require(concept.areas.length>0&&subset(concept.areas,areaIds),`concept ${concept.key}: areas`);
  for(const [key,target] of Object.entries(keys))require(({agent:agentIds,work:workIds,concept:conceptIds}[target.kind]?.has(target.id)??false),`key ${key}: target`);
  for(const record of [...agents,...works])require(/^[a-z0-9]+(?:[-.][a-z0-9]+)*$/.test(record.id),`ID ${record.id}: invalid`);
  for(const [id,was] of [['valentin-wagner','Vera Wagner'],['gerhard-thonhauser','Shai Averbach'],['ophelia-deroy','Étienne Deroy']]){
    const names=agents.find(agent=>agent.id===id)?.register_names?.map(entry=>entry.original);
    require(names?.length===1&&names[0]===was,`agent ${id}: corrected register name`);
  }
  require(!agentIds.has('vera-wagner'),'agent vera-wagner: corrected identity');
  return issues;
}

import {sourceLabel} from './human-presentation.mjs';

// Distinguish colliding visible search names without printing transport IDs.
// Ordinals describe this result group only; identities remain on the actions.
export function searchDisambiguators(rows,language='ru'){
  const groups=new Map(),result=new Map();
  for(const row of rows){const key=JSON.stringify([row.title,row.kind,row.detail??'']);if(!groups.has(key))groups.set(key,[]);groups.get(key).push(row);}
  for(const entries of groups.values()){
    const group=[...new Map(entries.map(row=>[`${row.kind}:${row.raw.id}`,row])).values()];
    if(group.length<2)continue;
    const sources=new Map();
    for(const row of group){const source=sourceLabel(row.raw.source_graph,language);if(!sources.has(source))sources.set(source,[]);sources.get(source).push(row);}
    for(const [source,items]of sources){
      const ordered=[...items].sort((a,b)=>a.raw.id<b.raw.id?-1:a.raw.id>b.raw.id?1:0);
      const versions=ordered.map(row=>row.raw.attributes?.record_version);
      const distinctVersions=versions.every(value=>Number.isSafeInteger(value)&&value>0)&&new Set(versions).size===ordered.length;
      ordered.forEach((row,index)=>{
        const name=distinctVersions?{ru:'Версия',en:'Version',es:'Versión'}:{ru:'Запись',en:'Record',es:'Registro'};
        const suffix=ordered.length>1?`${name[language]??name.en} ${distinctVersions?versions[index]:index+1}`:'';
        result.set(`${row.kind}:${row.raw.id}`,[source,suffix].filter(Boolean).join(' · '));
      });
    }
  }
  return result;
}

// The space reads the field; the field remains the owner of its meaning.
import {readChronology} from '../../../../bonsai/lib/chronology.mjs';
const bi=(ru,en)=>({ru,en});
const cat=id=>`aesthetics:catalog:${id}`, reading=id=>`aesthetics:reading:${id}`, route=id=>`aesthetics:route:${id}`;
const place=cite=>`aesthetics:source:${cite.w}:${cite.loc??[...(cite.u??[]),cite.s,cite.e].join(':')}`;
const join=(values,sep)=>bi(values.map(v=>v?.ru??v?.en??'').join(sep),values.map(v=>v?.en??v?.ru??'').join(sep));
export function makeAesthetics({field,library:catalog,chronology}){
  const clock=readChronology(chronology),nodes=[],edges=[],routes=[];
  const concepts=new Map(field.concepts.map(item=>[item.id,item])),conceptions=new Map(field.conceptions.map(item=>[item.id,item]));
  const used=new Set(field.conceptions.flatMap(item=>item.of));
  const add=(id,kind,title,extra={})=>nodes.push({id,kind,title,parentId:null,...(['concept','figure','question','interpretation'].includes(kind)?{demo:true,body:title,position:[0,0,0]}:{}),...extra});
  const year=id=>clock.span(id)?.from??Infinity;
  for(const concept of field.concepts.filter(item=>used.has(item.id)))add(cat(concept.id),'concept',concept.title,{legacyId:concept.id,category:concept.type==='tos.entity.philosophical-category'});
  for(const [id,agent] of Object.entries(catalog.agents))add(cat(id),'figure',agent.name,{legacyId:id,life:chronology.lives[id]});
  for(const [id,work] of Object.entries(catalog.works))add(cat(id),'work',work.title,{legacyId:id,author:work.agents?.[0],dating:chronology.compositions.works[id]});
  const sources=new Map();
  for(const item of field.conceptions){
    const author=join((item.agents??[]).map(id=>catalog.agents[id]?.name),' · '),names=join(item.of.map(id=>concepts.get(id)?.title),' · ');
    const title=bi(`${names.ru} — ${author.ru}`,`${names.en} — ${author.en}`),sourceIds=[];
    for(const cite of item.formula.cite){
      const id=place(cite);sourceIds.push(id);
      if(!sources.has(id)){const work=catalog.works[cite.w]?.title??bi(cite.w,cite.w),locator=cite.loc??item.locus;sources.set(id,{id,kind:'fragment',parentId:cat(cite.w),title:bi(`${work.ru} · ${locator}`,`${work.en} · ${locator}`),body:{cite:[cite]},workId:cite.w,locator});}
      edges.push({id:`aesthetics:reading-source:${item.id}:${sourceIds.length}`,from:reading(item.id),to:id,kind:'supports',label:bi('Источник','Source')});
    }
    add(reading(item.id),'interpretation',title,{demo:true,body:{...item.thesis,cite:item.formula.cite},question:field.areas.find(area=>item.areas.includes(area.id))?.question,voice:item.voice,workId:item.work,agents:item.agents,locus:item.locus,concepts:item.of,areaIds:item.areas,sourceNodeId:sourceIds[0],sourceNodeIds:sourceIds});
  }
  nodes.push(...sources.values());
  for(const area of field.areas){
    const items=field.conceptions.filter(item=>item.areas.includes(area.id)).sort((a,b)=>year(chronology.compositions.works[a.work])-year(chronology.compositions.works[b.work]));
    if(!items.length)continue;
    const steps=items.map(item=>{const node=nodes.find(node=>node.id===reading(item.id));return {nodeId:node.id,sourceNodeId:node.sourceNodeId,title:node.title,body:node.body,question:area.question};});
    const path={id:area.id,title:area.title,question:area.question,steps,transitions:[]};routes.push(path);
    add(route(area.id),'question',area.title,{demo:true,question:area.question,group:area.group,route:path});
    for(const item of items)edges.push({id:`aesthetics:area-reading:${area.id}:${item.id}`,from:route(area.id),to:reading(item.id),kind:'contains',label:area.title});
  }
  for(const link of field.links){if(!used.has(link.from)||!used.has(link.to)||!link.grounds?.length||!link.grounds.every(id=>conceptions.has(id)))continue;edges.push({id:`aesthetics:relation:${link.id}`,from:cat(link.from),to:cat(link.to),kind:'relates',label:link.verb,grounds:link.grounds.map(reading)});}
  const rootId=routes.length?route(routes[0].id):nodes[0]?.id;
  return {schema:'tos_constructor_library_v1',rootId,fingerprint:'0'.repeat(64),nodes,atlas:{defaultIds:routes.map(item=>route(item.id)),edges,clusters:[],assemblies:[],extensions:[],routes},space:{profile:'aesthetics',title:field.title,posture:field.posture,collection:nodes.map(node=>({...node,materialId:node.id})),routes,lenses:[{id:'tree',icon:'✧',title:bi('Древо','The tree'),description:bi('Области, понятия и произведения.','Areas, concepts and works.')}],relationKinds:['contains','supports','relates']}};
}

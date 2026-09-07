import {localized} from './knowledge-client.mjs';

// Presentation groups use the advertised type registry, never ID prefixes or
// translated labels as evidence of meaning. Unknown mappings stay discoverable.
const groups={
  meaning:'Понятия и смыслы',identity:'Люди и произведения',history:'История и традиции',
  authorship:'Авторство и передача',evidence:'Источники и свидетельства',structure:'Структура источников',
  other:'Другие типы',technical:'Технические типы',unavailable:'Выбрано вне этих источников',
};
const order=Object.keys(groups),collator=new Intl.Collator('ru',{numeric:true,sensitivity:'base'});
export function lensVocabulary(catalog,key){
  const relation=key==='predicates',registry=catalog.semantic_registries||{};
  const entities=new Map((registry.entity_types?.entries||[]).map(e=>[e.type_id,e]));
  const entries=relation?new Map((registry.relation_types?.entries||[]).map(e=>[e.relation_type_id,e])):entities;
  const field=relation?'predicate_id':'kind_id',mappingField=relation?'source_predicate_id':'source_kind_id';
  return (relation?catalog.predicates:catalog.node_kinds).map(item=>{
    const types=(item[relation?'relation_type_ids':'type_ids']||[]).map(id=>entries.get(id));
    const matching=types.flatMap(e=>(e?.source_mappings||[]).filter(m=>m[mappingField]===item[field]&&(!relation||m.scope==='edge')));
    const sources=[...new Set(matching.map(m=>m.source_graph).filter(s=>typeof s==='string'))];
    const sourceKnown=types.length>0&&types.every(Boolean)&&sources.length>0&&!item.mapping_statuses?.includes('unmapped');
    function bucket(entry){
      if(!entry)return 'other';
      if(!relation)return ({semantic:'meaning',identity:'identity',navigation:'history',evidence:'evidence',assertion:'evidence',activity:'evidence',literal:'evidence',projection:'technical'})[entry.object_role]||'other';
      const domain=entry.domain_type_ids||[],range=entry.range_type_ids||[];
      if(entry.assertion_mode==='derived-projection'||[...domain,...range].length>0&&[...domain,...range].every(id=>entities.get(id)?.object_role==='projection'))return 'technical';
      if(entry.parent_relation_type_ids?.includes('tos.relation.responsibility'))return 'authorship';
      if(entry.assertion_mode==='structural')return 'structure';
      if(entry.assertion_mode==='reified-claim'||entry.parent_relation_type_ids?.includes('tos.relation.claim-structure'))return 'evidence';
      if(entry.assertion_mode==='direct')return 'meaning';
      return 'other';
    }
    const buckets=types.map(bucket),group=buckets.find(b=>b!=='technical')||buckets[0]||'other';
    return {id:item[field],title:localized(item.display,item[field]),group,sources,sourceKnown,count:item.count||0};
  });
}
export function vocabularyGroups(items,{sources,selected=[],query='',sort='alphabet'}={}){
  const needle=query.trim().toLocaleLowerCase('ru'),chosen=new Set(selected),result=new Map();
  const candidates=[...items];
  for(const id of chosen)if(!items.some(item=>item.id===id))candidates.push({id,title:id,group:'other',sources:[],sourceKnown:false,count:0});
  for(const item of candidates){
    const available=!item.sourceKnown||item.sources.some(source=>sources.includes(source));
    if(!available&&!chosen.has(item.id))continue;
    if(needle&&!(item.title+' '+item.id).toLocaleLowerCase('ru').includes(needle))continue;
    const group=available?item.group:'unavailable';
    if(!result.has(group))result.set(group,[]);result.get(group).push({...item,selected:chosen.has(item.id),available});
  }
  return order.filter(key=>result.has(key)).map(key=>({key,title:groups[key],items:result.get(key).sort((a,b)=>Number(b.selected)-Number(a.selected)||(sort==='frequency'?b.count-a.count:0)||collator.compare(a.title,b.title)||collator.compare(a.id,b.id))}));
}

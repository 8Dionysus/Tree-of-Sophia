import {BUDGET,ContractError,RevisionError,checkRevision,validateLens} from './knowledge-client.mjs';

export const CUSTOM_LENS='observatory-custom';
export const SAVED_LENSES_KEY='tos-observatory-lenses-v1';
const bad=message=>{throw new ContractError(message);};
const strings=(value,max,length=1024)=>Array.isArray(value)&&value.length<=max&&new Set(value).size===value.length&&value.every(v=>typeof v==='string'&&v.length>0&&v.length<=length);
const drafts=new WeakMap();
export const draftForPacket=packet=>drafts.get(packet)||null;

export function validateDraft(value){
  if(!value||value.v!==1||typeof value.name!=='string'||!value.name.trim()||value.name.length>64
    ||!['area','focus','all'].includes(value.scope)||!strings(value.sources,7)||!value.sources.length
    ||!strings(value.nodeIds,BUDGET.nodes)||!strings(value.kinds,100)||!strings(value.predicates,100)
    ||typeof value.query!=='string'||value.query.length>256
    ||!(value.focusId===null||typeof value.focusId==='string'&&value.focusId.length>0&&value.focusId.length<=1024)
    ||!Number.isInteger(value.depth)||value.depth<0||value.depth>3
    ||!['either','outgoing','incoming'].includes(value.direction)||!['overview','all'].includes(value.profile)
    ||!Number.isInteger(value.limit)||value.limit<1||value.limit>BUDGET.nodes||typeof value.relations!=='boolean')bad('Настройки линзы неполны или превышают допустимый размер.');
  if(value.scope==='area'&&!value.nodeIds.length)bad('Исходная область пуста. Выберите поиск по древу.');
  if(value.scope==='focus'&&!value.focusId)bad('Сначала выберите звезду.');
  // Keep only owned fields when reading an untrusted link or local definition.
  return {v:1,name:value.name.trim(),scope:value.scope,sources:[...value.sources],nodeIds:[...value.nodeIds],focusId:value.focusId,
    query:value.query,kinds:[...value.kinds],predicates:[...value.predicates],depth:value.depth,direction:value.direction,
    profile:value.profile,limit:value.limit,relations:value.relations};
}
export function encodeDraft(draft){const text=JSON.stringify(validateDraft(draft));if(text.length>12000)bad('Описание линзы слишком велико для ссылки. Сузьте исходную область.');return text;}
export function decodeDraft(text){if(typeof text!=='string'||text.length>12000)bad('Ссылка на линзу слишком велика.');try{return validateDraft(JSON.parse(text));}catch(error){if(error instanceof ContractError)throw error;bad('Не удалось прочитать настройки линзы.');}}

export async function constructorCatalog(client,signal){
  const [catalog,bundle]=await Promise.all([client.request('/catalog',{signal}),client.request('/contracts',{signal})]);
  checkRevision(catalog);
  const caps=catalog.capabilities,schema=bundle?.contracts?.lens_spec;
  if(catalog.schema!=='tos_knowledge_catalog_v1'||catalog.authority_boundary?.is_source!==false
    ||catalog.authority_boundary?.is_canon!==false||catalog.authority_boundary?.writes_to_tree!==false
    ||bundle?.schema!=='tos_knowledge_contract_bundle_v1'||bundle.authority_boundary?.writes_to_tree!==false
    ||schema?.properties?.schema_version?.const!=='tos_lens_spec_v1'
    ||!strings(caps?.sources,7)||!caps.sources.length
    ||!['in'].every(op=>caps.filter_operators?.includes(op))
    ||!caps.node_fields?.includes('kind_id')||!caps.relation_fields?.includes('predicate_id')
    ||!Array.isArray(catalog.node_kinds)||!Array.isArray(catalog.predicates)
    ||catalog.node_kinds.length>5000||catalog.predicates.length>5000
    ||!strings(catalog.node_kinds.map(k=>k?.kind_id),5000)||!strings(catalog.predicates.map(k=>k?.predicate_id),5000)
    ||!Array.isArray(schema.properties.sources?.items?.enum)
    ||!caps.sources.every(id=>schema.properties.sources.items.enum.includes(id))
    ||!['nodes','relations','groups','traversal_depth'].every(key=>Number.isInteger(caps.maximums?.[key])&&caps.maximums[key]>=1)
    ||!caps.neighborhood_profiles?.some(p=>p.profile==='all')||!caps.neighborhood_profiles?.some(p=>p.profile==='overview')
    ||!['nodes','relations','groups'].every(key=>Number.isInteger(schema.properties.limits?.properties?.[key]?.maximum)&&schema.properties.limits.properties[key].maximum>=1)
    ||!Number.isInteger(schema.properties.traversal?.properties?.depth?.maximum)
    ||caps.inclusion?.authority!=='query-execution-not-semantic-proof')bad('Сервер пока не предоставляет совместимый конструктор линз.');
  return {catalog,schema};
}
export function initialDraft(packet,context){
  return {v:1,name:'Моя линза',scope:packet?.nodes?.length?'area':'all',sources:[...context.catalog.capabilities.sources],
    nodeIds:(packet?.nodes||[]).map(n=>n.id),focusId:packet?.focus?.node_id||null,query:'',kinds:[],predicates:[],
    depth:0,direction:'either',profile:'all',limit:BUDGET.nodes,relations:true};
}
export function compileDraft(value,{catalog,schema}){
  const draft=validateDraft(value),caps=catalog.capabilities;
  const listed=(values,allowed)=>values.every(v=>allowed.includes(v));
  if(!listed(draft.sources,caps.sources)||!listed(draft.kinds,catalog.node_kinds.map(k=>k?.kind_id))
    ||!listed(draft.predicates,catalog.predicates.map(p=>p.predicate_id)))bad('Словарь данных изменился. Обновите каталог и проверьте выбранные условия.');
  const limit=Math.min(BUDGET.nodes,caps.maximums.nodes,schema.properties.limits.properties.nodes.maximum);
  if(draft.limit>limit||draft.depth>Math.min(caps.maximums.traversal_depth,schema.properties.traversal.properties.depth.maximum))bad('Сервер не поддерживает выбранный размер области.');
  const spec={schema_version:'tos_lens_spec_v1',lens_id:CUSTOM_LENS,title:draft.name,language:'ru',detail:'compact',explain:true,
    sources:draft.sources,seed:draft.scope==='focus'?{focus_node_id:draft.focusId}:{text_query:draft.query,...(draft.scope==='area'?{node_ids:draft.nodeIds}:{})},
    node_query:{enabled:draft.scope!=='focus',filters:draft.kinds.length?[{field:'kind_id',op:'in',value:draft.kinds}]:[]},
    relation_query:{enabled:draft.relations,filters:draft.predicates.length?[{field:'predicate_id',op:'in',value:draft.predicates}]:[]},
    traversal:{depth:draft.depth,direction:draft.direction,profile:draft.profile},composition:{endpoint_policy:'both'},
    limits:{nodes:draft.limit,relations:draft.relations?Math.min(BUDGET.relations,caps.maximums.relations,schema.properties.limits.properties.relations.maximum):0,groups:Math.min(8,caps.maximums.groups,schema.properties.limits.properties.groups.maximum)}};
  // In focus mode filters describe relations around the explicit center;
  // dormant root filters must not masquerade as conditions on neighbors.
  if(draft.scope==='focus')spec.node_query.filters=[];
  return spec;
}
export function summarizeLens(packet){
  validateLens(packet);
  const counts=packet.counts;
  if(!/^[a-f0-9]{64}$/.test(packet.fingerprint||'')||packet.inclusion?.authority!=='query-execution-not-semantic-proof'
    ||!['matched_nodes','eligible_relations','truncated_nodes','truncated_relations'].every(k=>Number.isInteger(counts?.[k])&&counts[k]>=0)
    ||counts.nodes!==packet.nodes.length||counts.relations!==packet.relations.length)bad('Сервер не подтвердил состав линзы.');
  return {nodes:packet.nodes.length,relations:packet.relations.length,matched:counts.matched_nodes,
    context:packet.nodes.filter(n=>['traversal','endpoint'].includes(packet.inclusion.nodes?.[n.id]?.kind)).length,
    limited:counts.truncated_nodes>0||counts.truncated_relations>0};
}
export async function previewDraft(client,draft,context,signal){
  const packet=await client.compile(compileDraft(draft,context),signal,context.catalog.source_revision);
  summarizeLens(packet);drafts.set(packet,validateDraft(draft));return packet;
}
export async function confirmDraft(client,draft,context,preview,signal){
  const packet=await previewDraft(client,draft,context,signal);
  if(packet.fingerprint!==preview.fingerprint)throw new RevisionError();
  return packet;
}
export function readSaved(storage){
  const text=storage.getItem(SAVED_LENSES_KEY);if(!text)return [];
  if(text.length>150000)bad('Сохранённые линзы превышают размер локального хранилища.');
  try{const entries=JSON.parse(text);if(!Array.isArray(entries)||entries.length>12)throw new Error();return entries.map(validateDraft);}catch{bad('Сохранённые линзы не удалось прочитать. Они остались в хранилище без изменений.');}
}
export function saveDraft(storage,draft){
  const valid=decodeDraft(encodeDraft(draft)),entries=readSaved(storage),index=entries.findIndex(e=>e.name===valid.name);
  if(index<0){if(entries.length>=12)bad('Уже сохранено 12 линз. Дайте этой линзе имя одной из существующих, чтобы обновить её.');entries.push(valid);}else entries[index]=valid;
  storage.setItem(SAVED_LENSES_KEY,JSON.stringify(entries));return entries;
}

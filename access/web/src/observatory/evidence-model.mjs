import {ContractError,RevisionError,localized} from './knowledge-client.mjs';

export const sourceRefs=item=>[...new Set([item?.source_ref,...(item?.source_refs||[])].filter(value=>typeof value==='string'&&value))];
export function evidenceRoute(raw){
  if(!raw?.native_id)return null;
  if(raw.source_graph==='philosophy')return {mode:'philosophy',item_id:raw.native_id};
  // Membership in the bounded corpus route graph is checked by the endpoint.
  if(raw.source_graph==='canon')return {mode:'corpus',item_id:raw.native_id,view_id:'route-graph'};
  return null;
}

export function validateEvidence(packet,raw,kind,route){
  const selection=packet?.selection,boundary=packet?.authority_boundary;
  if(packet?.schema!=='tos_evidence_lens_packet_v1'||packet.mode!==route.mode||packet.item_id!==raw.native_id
    ||selection?.[kind==='relation'?'edge_id':'node_id']!==raw.native_id
    ||!['is_source','is_canon','is_semantic_truth','is_rights_clearance'].every(key=>boundary?.[key]===false)
    ||!sourceRefs(selection).some(ref=>sourceRefs(raw).includes(ref))
    ||!['challenge_relations','context_relations','neighbor_nodes','source_refs','routes','source_anchors','gaps'].every(key=>Array.isArray(packet[key]))
    ||typeof packet.conclusion?.can_conclude!=='boolean'){
    throw new ContractError('Основания не удалось связать с выбранным объектом. Обновите область.');
  }
  return packet;
}

export async function loadEvidence(raw,kind,revision,{client,queries,signal,limit=60}){
  const {match}=await client.inspect(kind,raw.id,signal,revision);
  signal?.throwIfAborted();
  if(match.content_revision!==raw.content_revision)throw new RevisionError();
  const route=evidenceRoute(match);
  if(!route)return {raw:match,availability:'not_connected',packet:null};
  let packet;
  try{packet=await queries.invoke('tos.epistemic.inspect',{...route,limit},{signal});}
  catch(error){if(error.status!==404)throw error;signal?.throwIfAborted();return {raw:match,availability:'outside_route',packet:null};}
  signal?.throwIfAborted();validateEvidence(packet,match,kind,route);
  // The older evidence endpoint has no snapshot token. Do not cache it as if it
  // shared the knowledge transaction; verify the surrounding snapshot again.
  const after=await client.inspect(kind,raw.id,signal,revision);signal?.throwIfAborted();
  if(after.match.content_revision!==match.content_revision)throw new RevisionError();
  return {raw:match,availability:'available',packet,binding:{knowledge_id:raw.id,native_id:match.native_id,
    source_graph:match.source_graph,source_revision:revision,content_revision:match.content_revision}};
}

export function reading(item,nodes=[]){
  const properties=item.properties||{},id=item.edge_id||item.node_id;
  const name=id=>{const node=nodes.find(node=>node.node_id===id);return node?.label_ru||node?.preferred_label||node?.label||id;};
  return {id,label:item.label_ru||item.label||properties.relation_label||item.predicate_id||id,
    statement:item.summary_ru||item.summary||properties.comment||properties.description||'',
    route:item.from_id&&item.to_id?name(item.from_id)+' → '+name(item.to_id):'',
    predicate_id:item.predicate_id,from_id:item.from_id,to_id:item.to_id,source_refs:sourceRefs(item),
    authority_posture:properties.authority_posture,canon_status:properties.canon_status,
    review_posture:properties.review_posture,confidence:properties.confidence||properties.master_confidence};
}

export function compareEvidence(result,selection){
  const packet=result.packet;
  if(!packet)throw new Error('Для этого объекта сравнение прочтений пока не подключено.');
  const others=items=>items.filter(item=>(item.edge_id||item.node_id)!==packet.item_id).map(item=>reading(item,[packet.selection,...packet.neighbor_nodes]));
  const competing=others(packet.challenge_relations),context=others(packet.context_relations);
  return {schema:'tos_interpretation_comparison_v1',selection,binding:result.binding,
    posture:packet.posture,can_conclude:packet.conclusion.can_conclude===true,
    competing_reading_count:competing.length,competing_readings:competing,contextual_readings:context,
    coverage:packet.coverage,gaps:packet.gaps_ru||packet.gaps,authority_note:packet.authority_note};
}

export function selectionSummary(raw,kind){return {id:raw.id,kind:kind==='relation'?'edge':'node',
  label:localized(raw.display.title||raw.display.label,raw.id),source_refs:sourceRefs(raw)};}

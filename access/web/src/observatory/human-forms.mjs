import {t} from './ui-i18n.mjs';
import {decodeHumanFormSelection,FORM_WIRE_BUDGET} from '../../../shared/human-form-selection-codec.ts';
import {essentialContext} from './record-context.mjs';
import {contentLanguage,exactFormRef,sameFormRef,validFormIdentity as rustValidFormIdentity,
  createHumanFormRuleSession,inspectionIndex,inspectionSourcePointer,validateInspectedPacket} from './human-form-rules.mjs';
export {contentLanguage,exactFormRef,sameFormRef};

// Delivery validation only. ToS/contracts/human-form.schema.json owns the
// materialization; access/contracts/knowledge-graph.v1.schema.json owns selection.
export const FORM_ROLES=['name','caption','hover','statement','grounds','history','technical'];
export const FORM_STATES=['ready','missing','unavailable','ambiguous','over-budget'];
const own=(value,key)=>Object.prototype.hasOwnProperty.call(value,key);
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const strings=value=>Array.isArray(value)&&value.every(item=>typeof item==='string');
export class FormContractError extends Error {constructor(){super(t('Пакет формы неполон или не соответствует версии материала.'));}}
const requireForm=value=>{if(!value)throw new FormContractError();};
function boundedJSON(value,limit){
  let count=0,minimum=0;
  function visit(item,depth){
    requireForm(depth<=64&&++count<=30000);
    if(typeof item==='string'){minimum+=item.length;requireForm(minimum<=limit);return;}
    if(item===null||typeof item==='boolean')return;
    if(typeof item==='number'){requireForm(Number.isFinite(item));return;}
    requireForm(Array.isArray(item)||object(item));
    for(const key in item){
      if(!Object.hasOwn(item,key))continue;
      if(!Array.isArray(item))minimum+=key.length;
      requireForm(minimum<=limit);visit(item[key],depth+1);
    }
  }
  visit(value,0);requireForm(new TextEncoder().encode(JSON.stringify(value)).length<=limit);
}
function checkedHumanFormRules(raw,requested){
  if(!own(raw,'human_form_selection'))return {selection:null,session:null};
  const wire=raw.human_form_selection;let selection;
  try{
    requireForm(object(wire));
    if(wire.schema_version==='tos_human_form_selection_v2')selection=decodeHumanFormSelection(wire);
    else{
      requireForm(wire.schema_version==='tos_human_form_selection_v1'&&!own(wire,'packet_base')&&!own(wire,'shared_limits'));
      // Keep the legacy v1 byte boundary in the transport adapter.
      boundedJSON(wire,FORM_WIRE_BUDGET);
      selection=wire;
    }
    requireForm(requested===undefined||typeof requested==='string');
  }catch{throw new FormContractError();}
  let session;
  try{
    const forms=raw?.attributes?.human_forms;
    session=createHumanFormRuleSession(raw,selection,requested,Array.isArray(forms)?forms.length:0);
  }catch{throw new FormContractError();}
  // This mandatory carrier-owner check is independent of the Rust selection
  // rule and continues to fail closed when declared context is unavailable.
  try{requireForm(['available','not-declared'].includes(essentialContext(raw).state));}
  catch(error){session.free();throw error;}
  return {selection,session};
}

// The ordinary selection is deliberately capped at the transport budget. A
// full material read may still carry the source-owned packet which was
// represented there only by an exact ref. Keep this inspection separate from
// the bounded reader copy and never manufacture a shortened wording.
export function inspectExactHumanForm(raw,role){
  const {selection,session}=checkedHumanFormRules(raw);
  try{return inspectSelectedHumanForm(raw,role,selection,session);}
  finally{session?.free();}
}
function inspectSelectedHumanForm(raw,role,selection,session){
  if(!selection||!session)return null;
  const index=inspectionIndex(session,role),source_pointer=inspectionSourcePointer(session,role);
  const forms=raw?.attributes?.human_forms;
  if(index===undefined||source_pointer===undefined||!Array.isArray(forms)||index>=forms.length)return null;
  const packet=forms[index];
  try{validateInspectedPacket(session,role,index,packet);}catch{throw new FormContractError();}
  return {form:structuredClone(selection.roles[role].form),source_pointer,packet:structuredClone(packet)};
}

export function inspectExactHumanForms(raw){
  const {selection,session}=checkedHumanFormRules(raw);if(!selection)return null;
  try{
    const inspected={};
    for(const role of FORM_ROLES){
      const value=inspectSelectedHumanForm(raw,role,selection,session);if(value)inspected[role]=value;
    }
    return Object.keys(inspected).length?inspected:null;
  }finally{session.free();}
}

export function validateHumanForms(raw,requested){
  const {selection,session}=checkedHumanFormRules(raw,requested);
  try{return selection;}finally{session?.free();}
}
export function formIdentity(raw){
  const {selection,session}=checkedHumanFormRules(raw);
  try{return selection?session.identity():null;}finally{session?.free();}
}
export function validFormIdentity(value,requested){
  return rustValidFormIdentity(value,requested);
}
export function formLanguages(raw){
  const selection=validateHumanForms(raw);
  return selection?[...new Set(['auto','original',selection.requested_language,...selection.candidates.map(value=>value.language).filter(Boolean)])]:[];
}
export function formView(raw){
  const selection=validateHumanForms(raw);
  return selection?{selection,roles:FORM_ROLES.map(role=>({role,...selection.roles[role],
    candidates:selection.candidates.filter(candidate=>candidate.role===role)}))}:null;
}
// A compact Claim's wording pointer names an entire packet. Context pointers
// and explicit relation identities remain part of that same reading unit.
function claimContext(packet,reading){
  requireForm(object(reading)&&['claim-with-mandatory-context','claim-with-shared-form-context-v2'].includes(reading.mode)&&reading.standalone===false
    &&['available','missing'].includes(reading.wording_state)
    &&JSON.stringify(reading.context_pointers)===JSON.stringify(['/semantics','/epistemic'])
    &&strings(reading.relation_context_ids)&&new Set(reading.relation_context_ids).size===reading.relation_context_ids.length);
  const node=packet.nodes?.find(item=>item.id===reading.node_id);
  requireForm(node&&node.content_revision===reading.content_revision&&object(node.semantics)&&object(node.epistemic));
  const relations=reading.relation_context_ids.map(id=>{const value=packet.relations?.find(item=>item.id===id);requireForm(value);return value;});
  return {node,relations};
}
export function claimPathFor(packet,nodeId){
  const scene=packet?.scene;if(scene?.compact===undefined)return null;
  requireForm(scene.schema_version==='tos_knowledge_scene_v1'&&object(scene.compact)
    &&scene.compact.rule==='explicit-claim-paths-v1'&&scene.compact.authority==='presentation-only-no-new-assertion'
    &&Array.isArray(scene.compact.claim_paths));
  const paths=scene.compact.claim_paths.filter(path=>object(path)&&path.claim_node_id===nodeId);
  requireForm(paths.length<=1);return paths[0]||null;
}
export function claimPathClosure(packet,path){
  requireForm(object(path)&&claimPathFor(packet,path.claim_node_id)===path
    &&typeof path.id==='string'&&Boolean(path.id)&&typeof path.relation_type_id==='string'
    &&strings(path.node_ids)&&path.node_ids.length===3&&path.node_ids[1]===path.claim_node_id
    &&strings(path.relation_ids)&&path.relation_ids.length===2&&strings(path.detail_relation_ids)
    &&path.reading?.node_id===path.claim_node_id);
  const {node,relations}=claimContext(packet,path.reading),claim=node.semantics.claim;
  requireForm(object(claim)&&claim.predicate_mapping_status==='mapped'&&claim.relation_type_id===path.relation_type_id
    &&claim.subject_node_id===path.node_ids[0]&&claim.object_node_id===path.node_ids[2]
    &&path.node_ids[0]!==node.id&&path.node_ids[2]!==node.id);
  const relationIds=[...path.relation_ids,...path.detail_relation_ids];
  requireForm(new Set(relationIds).size===relationIds.length&&relationIds.length===relations.length
    &&relations.every(relation=>relationIds.includes(relation.id)));
  for(const [index,id]of path.relation_ids.entries()){
    const relation=relations.find(item=>item.id===id);
    requireForm(relation?.from_id===node.id&&relation.to_id===path.node_ids[index===0?0:2]
      &&relation.relation_type_id===['tos.relation.has-subject','tos.relation.has-object'][index]);
  }
  const memberRelations=[];
  for(const id of path.detail_relation_ids){
    const relation=relations.find(item=>item.id===id);
    requireForm(relation?.from_id===node.id&&['tos.relation.claim-supported-by','tos.relation.claim-value-member'].includes(relation.relation_type_id));
    if(relation.relation_type_id==='tos.relation.claim-value-member')memberRelations.push(relation);
  }
  const allMemberRelations=packet.relations.filter(relation=>relation.from_id===node.id&&relation.relation_type_id==='tos.relation.claim-value-member');
  if(own(claim,'value_member_node_ids')||allMemberRelations.length){
    const memberIds=claim.value_member_node_ids;
    // Only the normalized declaration owns the complete reference set. These
    // structural edges do not establish accepted membership or a Sign judgment.
    requireForm(strings(memberIds)&&memberIds.length>0&&memberIds.every(Boolean)
      &&new Set(memberIds).size===memberIds.length&&memberRelations.length===memberIds.length
      &&memberRelations.length===allMemberRelations.length
      &&new Set(memberRelations.map(relation=>relation.to_id)).size===memberIds.length
      &&memberRelations.every(relation=>memberIds.includes(relation.to_id)));
  }
  const nodeIds=[...new Set([...path.node_ids,...relations.flatMap(relation=>[relation.from_id,relation.to_id])])];
  requireForm(nodeIds.every(id=>typeof id==='string'&&packet.nodes.some(item=>item.id===id)));
  return {nodeIds,relationIds,node};
}
export function resolveClaimReading(packet,reading){
  const {node,relations}=claimContext(packet,reading);
  const forms=validateHumanForms(node),path=reading.wording_pointer;let wording=null;
  const shared=reading.mode==='claim-with-shared-form-context-v2';
  if(shared)requireForm(node.human_form_selection?.schema_version==='tos_human_form_selection_v2');
  if(reading.wording_state==='missing')requireForm(path===null);
  else if(shared){
    requireForm(typeof path==='string'&&/^\/human_form_selection\/roles\/(caption|statement|hover)$/.test(path));
    const role=path.split('/')[3];requireForm(forms?.roles[role]?.state==='ready');wording=forms.roles[role].packet;
  }else if(typeof path==='string'&&/^\/human_form_selection\/roles\/(caption|statement|hover)\/packet$/.test(path)){
    requireForm(node.human_form_selection?.schema_version==='tos_human_form_selection_v1');
    const role=path.split('/')[3];requireForm(forms?.roles[role]?.state==='ready');wording=forms.roles[role].packet;
  }else{
    requireForm(['/display_selection/fields/summary','/display_selection/fields/title'].includes(path));
    wording=node.display_selection?.fields?.[path.split('/')[3]];requireForm(object(wording)&&wording.content_available===true);
  }
  return {reading:structuredClone(reading),wording:structuredClone(wording),semantics:structuredClone(node.semantics),
    epistemic:structuredClone(node.epistemic),relations:structuredClone(relations)};
}

import {t} from './ui-i18n.mjs';

// Delivery validation only. ToS/contracts/human-form.schema.json owns the
// materialization; access/contracts/knowledge-graph.v1.schema.json owns selection.
export const FORM_ROLES=['name','caption','hover','statement','grounds','history','technical'];
export const FORM_STATES=['ready','missing','unavailable','ambiguous','over-budget'];
const reasons=['exact-language','less-specific-language','automatic','fallback','original','no-ready-form','multiple-forms','original-role-not-declared','inspect-exact-form'];
const candidateStates=['ready','invalid','unavailable','stale','restricted','needs-assessment','over-budget'];
const own=(value,key)=>Object.prototype.hasOwnProperty.call(value,key);
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const hash=value=>typeof value==='string'&&/^[a-f0-9]{64}$(?![\s\S])/.test(value);
export const contentLanguage=value=>typeof value==='string'&&value.length<=128&&/^(?:auto|original|[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|[iIxX](?:-[A-Za-z0-9]{1,8})+)$(?![\s\S])/.test(value);
const language=value=>value===null||(contentLanguage(value)&&!['auto','original'].includes(value));
const strings=value=>Array.isArray(value)&&value.every(item=>typeof item==='string');
const pointer=value=>typeof value==='string'&&value.length<=2048&&/^(?:\/(?:[^~/]|~[01])*)*$/.test(value);
export const exactFormRef=value=>object(value)&&Object.keys(value).length===3&&typeof value.id==='string'&&Boolean(value.id)
  &&Number.isSafeInteger(value.version)&&value.version>=1&&typeof value.digest==='string'&&/^sha256:[a-f0-9]{64}$(?![\s\S])/.test(value.digest);
export const sameFormRef=(a,b)=>exactFormRef(a)&&exactFormRef(b)&&a.id===b.id&&a.version===b.version&&a.digest===b.digest;
const binding=value=>object(value)&&exactFormRef(value.record)&&pointer(value.pointer);
export class FormContractError extends Error {constructor(){super(t('Пакет формы неполон или не соответствует версии материала.'));}}
const requireForm=value=>{if(!value)throw new FormContractError();};
function boundedJSON(value,limit){
  let count=0;
  function visit(item,depth){
    requireForm(depth<=64&&++count<=30000);
    if(item===null||typeof item==='string'||typeof item==='boolean')return;
    if(typeof item==='number'){requireForm(Number.isFinite(item));return;}
    requireForm(Array.isArray(item)||object(item));
    for(const child of Object.values(item))visit(child,depth+1);
  }
  visit(value,0);requireForm(new TextEncoder().encode(JSON.stringify(value)).length<=limit);
}
function validatePacket(packet,role,ref,raw){
  requireForm(object(packet)&&packet.schema_version==='tos_human_form_materialization_v1'&&packet.state==='ready'
    &&packet.role===role&&sameFormRef(packet.form,ref)&&exactFormRef(packet.subject)
    &&(!raw.entity_id||packet.subject.id===raw.entity_id)
    &&typeof packet.display_text==='string'&&Boolean(packet.display_text.trim())
    &&own(packet,'language')&&language(packet.language)&&own(packet,'script')
    &&(packet.script===null||typeof packet.script==='string'&&/^[A-Za-z]{4}$(?![\s\S])/.test(packet.script))
    &&['source-copy','template','freeform'].includes(packet.derivation)
    &&Array.isArray(packet.dependencies)&&packet.dependencies.every(exactFormRef)
    &&strings(packet.issues)&&packet.issues.length===0&&own(packet,'admission')&&(packet.admission===null||object(packet.admission))
    &&packet.performs_semantic_assessment===false&&typeof packet.standalone_reading==='boolean'
    &&Array.isArray(packet.context)&&packet.context.length<=256&&(!packet.context.length||packet.standalone_reading===false));
  for(const entry of packet.context)requireForm(object(entry)&&typeof entry.slot==='string'&&binding(entry.binding)&&own(entry,'value'));
  if(own(packet,'language_context')){
    const context=packet.language_context,value=context?.value;
    requireForm(object(context)&&binding(context.binding)&&object(value)&&language(value.language)&&value.language===packet.language
      &&value.script===packet.script&&['unknown','original','translation','transliteration','adaptation'].includes(value.relation)
      &&(['translation','transliteration','adaptation'].includes(value.relation)?binding(value.source):value.source===null));
  }
  if(own(packet,'assessment_snapshot')){
    const state=packet.assessment_snapshot;
    requireForm(object(state)&&typeof state.owner_snapshot==='string'&&/^sha256:[a-f0-9]{64}$(?![\s\S])/.test(state.owner_snapshot)
      &&Number.isSafeInteger(state.journal_batches)&&state.journal_batches>=0
      &&(state.journal_batches===0?state.journal_revision===null:hash(state.journal_revision))
      &&state.publication_authorized===false&&state.current_runtime_grant===false);
  }
}
export function validateHumanForms(raw,requested){
  if(!own(raw,'human_form_selection'))return null;
  const selection=raw.human_form_selection;boundedJSON(selection,16384);
  requireForm(object(selection)&&selection.schema_version==='tos_human_form_selection_v1'&&hash(selection.content_revision)
    &&selection.content_revision===raw.content_revision&&contentLanguage(selection.requested_language)
    &&(requested===undefined||selection.requested_language===requested)
    &&['available','invalid','over-budget'].includes(selection.state)
    &&own(selection,'source_ref')&&(selection.source_ref===null||typeof selection.source_ref==='string'&&selection.source_ref.length<=2048)
    &&selection.performs_translation===false&&selection.performs_assessment===false&&strings(selection.issues)
    &&object(selection.roles)&&Object.keys(selection.roles).length===FORM_ROLES.length
    &&FORM_ROLES.every(role=>own(selection.roles,role))&&Array.isArray(selection.candidates)&&selection.candidates.length<=32);
  const ids=new Set();
  for(const candidate of selection.candidates){
    requireForm(object(candidate)&&exactFormRef(candidate.form)&&!ids.has(candidate.form.id)
      &&(candidate.role===null||FORM_ROLES.includes(candidate.role))&&language(candidate.language)
      &&candidateStates.includes(candidate.state)&&typeof candidate.source_pointer==='string'&&/^\/attributes\/human_forms\/\d+$/.test(candidate.source_pointer));
    ids.add(candidate.form.id);
  }
  for(const role of FORM_ROLES){
    const selected=selection.roles[role];
    requireForm(object(selected)&&FORM_STATES.includes(selected.state)&&reasons.includes(selected.reason)
      &&(selected.form===null||exactFormRef(selected.form))&&own(selected,'packet'));
    if(selected.state==='ready'){
      requireForm(selection.state==='available');validatePacket(selected.packet,role,selected.form,raw);
      requireForm(selection.candidates.some(candidate=>candidate.role===role&&candidate.state==='ready'
        &&sameFormRef(candidate.form,selected.form)&&candidate.language===selected.packet.language));
      if(selected.reason==='exact-language')requireForm(selected.packet.language?.toLowerCase()===selection.requested_language.toLowerCase());
      if(selected.reason==='less-specific-language')requireForm(selected.packet.language&&selection.requested_language.toLowerCase().startsWith(selected.packet.language.toLowerCase()+'-'));
      if(selected.reason==='original')requireForm(selection.requested_language==='original'&&selected.packet.language_context?.value?.relation==='original');
      if(selected.reason==='automatic')requireForm(selection.requested_language==='auto');
    }else requireForm(selected.packet===null&&(selected.state==='over-budget'?exactFormRef(selected.form):selected.form===null));
  }
  return selection;
}
export function formIdentity(raw){
  const selection=validateHumanForms(raw);
  return selection?JSON.stringify([selection.requested_language,selection.state,...FORM_ROLES.map(role=>{
    const value=selection.roles[role],ref=value.form;return [role,value.state,value.reason,ref?{id:ref.id,version:ref.version,digest:ref.digest}:null,value.packet?.language??null];
  })]):null;
}
export function validFormIdentity(value,requested){
  if(typeof value!=='string'||value.length>16384)return false;
  let parts;try{parts=JSON.parse(value);}catch{return false;}
  return Array.isArray(parts)&&parts.length===9&&contentLanguage(parts[0])&&(requested===undefined||parts[0]===requested)&&['available','invalid','over-budget'].includes(parts[1])
    &&FORM_ROLES.every((role,index)=>{const row=parts[index+2];return Array.isArray(row)&&row.length===5&&row[0]===role
      &&FORM_STATES.includes(row[1])&&reasons.includes(row[2])&&(row[3]===null||exactFormRef(row[3]))&&language(row[4]);});
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
  requireForm(object(reading)&&reading.mode==='claim-with-mandatory-context'&&reading.standalone===false
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
  if(reading.wording_state==='missing')requireForm(path===null);
  else if(typeof path==='string'&&/^\/human_form_selection\/roles\/(caption|statement|hover)\/packet$/.test(path)){
    const role=path.split('/')[3];requireForm(forms?.roles[role]?.state==='ready');wording=forms.roles[role].packet;
  }else{
    requireForm(['/display_selection/fields/summary','/display_selection/fields/title'].includes(path));
    wording=node.display_selection?.fields?.[path.split('/')[3]];requireForm(object(wording)&&wording.content_available===true);
  }
  return {reading:structuredClone(reading),wording:structuredClone(wording),semantics:structuredClone(node.semantics),
    epistemic:structuredClone(node.epistemic),relations:structuredClone(relations)};
}

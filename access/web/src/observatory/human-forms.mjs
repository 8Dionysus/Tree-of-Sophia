import {t} from './ui-i18n.mjs';
import {selectedClaimContext,selectedClaimPath,selectedClaimClosure,selectedClaimWording} from './claim-reading-rules.mjs';
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
  try{return selectedClaimContext(packet,reading);}catch{throw new FormContractError();}
}
export function claimPathFor(packet,nodeId){
  try{return selectedClaimPath(packet,nodeId);}catch{throw new FormContractError();}
}
export function claimPathClosure(packet,path){
  try{
    const owned=object(path)&&claimPathFor(packet,path.claim_node_id)===path;
    const {node,relations}=claimContext(packet,path?.reading);
    return selectedClaimClosure(packet,path,node,relations,owned);
  }catch{throw new FormContractError();}
}
export function resolveClaimReading(packet,reading){
  const {node,relations}=claimContext(packet,reading);
  const forms=validateHumanForms(node);let wording;
  try{wording=selectedClaimWording(node,reading,forms);}catch{throw new FormContractError();}
  return {reading:structuredClone(reading),wording:structuredClone(wording),semantics:structuredClone(node.semantics),
    epistemic:structuredClone(node.epistemic),relations:structuredClone(relations)};
}

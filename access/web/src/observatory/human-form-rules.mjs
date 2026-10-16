// Host adapter for the source-owned Rust/WASM human-form rules. The host keeps
// JSON transport, exact source-object access, owner gates and presentation.
import {FORM_EXPANDED_BUDGET,FORM_PACKET_BUDGET} from '../../../shared/human-form-selection-codec.ts';
let installedRuntime;
const encoder=new TextEncoder();
const SOURCE_PROJECTION_BYTES=16*1024*1024;

function rules(){
  if(!installedRuntime)throw new Error('Human-form Rust rules are not installed');
  return installedRuntime;
}

function preflight(value,limit){
  const stack=[[value,0]];let visits=0,minimum=0;
  while(stack.length){
    const [item,depth]=stack.pop();
    if(depth>64||++visits>30_000)throw new TypeError('Human-form input exceeds structural budget');
    if(typeof item==='string')minimum+=item.length;
    else if(Array.isArray(item)){
      if(item.length>30_000-visits-stack.length)throw new TypeError('Human-form input exceeds structural budget');
      for(const member of item)stack.push([member,depth+1]);
    }else if(item!==null&&typeof item==='object'){
      for(const key in item){
        if(!Object.hasOwn(item,key))continue;
        if(visits+stack.length>=30_000)throw new TypeError('Human-form input exceeds structural budget');
        minimum+=key.length;stack.push([item[key],depth+1]);
      }
    }else if(item===undefined||typeof item==='function'||typeof item==='symbol'
      ||typeof item==='number'&&!Number.isFinite(item))throw new TypeError('Human-form input is not JSON data');
    if(minimum>limit)throw new TypeError('Human-form input exceeds byte budget');
  }
}

function bytes(value,limit){
  // UTF-8 JSON cannot be shorter than the sum of its retained UTF-16 leaves.
  // Reject oversized carrier data before forming a full JSON string or WASM copy.
  if(limit!==undefined)preflight(value,limit);
  const encoded=JSON.stringify(value,(_key,item)=>{
    if(item===undefined||typeof item==='function'||typeof item==='symbol'
      ||typeof item==='number'&&!Number.isFinite(item))throw new TypeError('Human-form input is not JSON data');
    return item;
  });
  if(typeof encoded!=='string')throw new TypeError('Human-form input is not JSON serializable');
  const raw=encoder.encode(encoded);
  if(limit!==undefined&&raw.length>limit)throw new TypeError('Human-form input exceeds byte budget');
  return raw;
}

function sourceProjection(raw){
  const projection={
    entity_id:raw?.entity_id??null,
    content_revision:raw?.content_revision??null,
  };
  const attributes=raw?.attributes;
  if(attributes!==null&&typeof attributes==='object'&&!Array.isArray(attributes)){
    const selected={};
    for(const key of ['source_claim','source_record','source_sha256']){
      if(!Object.prototype.hasOwnProperty.call(attributes,key))continue;
      const record=attributes[key];
      if((key==='source_claim'||key==='source_record')&&record!==null&&typeof record==='object'&&!Array.isArray(record)){
        const projected={};
        for(const field of ['schema_version','claim_id','claim_version','record_id','composite_id','artifact_id',
          'node_type','node_id','record_version']){
          if(Object.prototype.hasOwnProperty.call(record,field))projected[field]=record[field]===undefined?null:record[field];
        }
        selected[key]=projected;
      }else selected[key]=record===undefined?null:record;
    }
    projection.attributes=selected;
  }
  return projection;
}

export function installHumanFormRules(runtime){
  if(!runtime||typeof runtime.HumanFormRuleSession!=='function'
    ||typeof runtime.human_form_content_language_wasm_v1!=='function'
    ||typeof runtime.human_form_exact_ref_wasm_v1!=='function'
    ||typeof runtime.human_form_same_ref_wasm_v1!=='function'
    ||typeof runtime.human_form_valid_identity_wasm_v1!=='function'){
    throw new TypeError('Generated human-form Rust rule exports are incomplete');
  }
  installedRuntime=runtime;
}

export function contentLanguage(value){
  const runtime=rules();
  return typeof value==='string'&&runtime.human_form_content_language_wasm_v1(value);
}

export function exactFormRef(value){
  const runtime=rules();
  try{return runtime.human_form_exact_ref_wasm_v1(bytes(value));}catch{return false;}
}

export function sameFormRef(left,right){
  const runtime=rules();
  try{return runtime.human_form_same_ref_wasm_v1(bytes(left),bytes(right));}catch{return false;}
}

export function validFormIdentity(value,requested){
  const runtime=rules();
  return typeof value==='string'&&runtime.human_form_valid_identity_wasm_v1(value,requested);
}

export function createHumanFormRuleSession(raw,selection,requested,formsCount){
  const count=Number.isSafeInteger(formsCount)&&formsCount>=0?formsCount:0;
  return new (rules().HumanFormRuleSession)(
    bytes(selection,FORM_EXPANDED_BUDGET),bytes(sourceProjection(raw),SOURCE_PROJECTION_BYTES),requested,count,
  );
}

export function inspectionIndex(session,role){
  return session.inspectionIndex(role);
}

export function inspectionSourcePointer(session,role){
  return session.inspectionSourcePointer(role);
}

export function validateInspectedPacket(session,role,index,packet){
  session.validateInspectedPacket(role,index,bytes(packet,FORM_PACKET_BUDGET));
}

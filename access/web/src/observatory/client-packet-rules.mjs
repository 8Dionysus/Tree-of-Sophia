import {installLensSpecRules} from './lens-spec-rules.mjs';
import {installKnowledgeSceneRules} from '../../../shared/knowledge-scene.ts';
let runtime;
export function installClientPacketRules(value){
  if(['ClientPacketSession','ClientMaterialSession','client_request_set_key_v1'].some(name=>typeof value?.[name]!=='function'))throw new TypeError('Generated client packet Rust rules are unavailable');
  runtime=value;
  installKnowledgeSceneRules(value);
  installLensSpecRules(value);
}
export function createClientPacketSession(...args){
  if(!runtime)throw new Error('Client packet Rust rules are not installed');
  return new runtime.ClientPacketSession(...args);
}
// Longer property names cannot be either owner-normalized selector field.
export const packetSetKey=key=>key.length<=13&&runtime.client_request_set_key_v1(Uint16Array.from({length:key.length},(_,index)=>key.charCodeAt(index)));
export const createClientMaterialSession=(...args)=>new runtime.ClientMaterialSession(...args);
export const packetKindUnits=value=>runtime.ClientMaterialSession.kind_length(typeof value==='string',typeof value==='string'?value.length:0)
  ?Uint16Array.from({length:value.length},(_,index)=>value.charCodeAt(index)):new Uint16Array();

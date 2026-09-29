import {installKnowledgeSceneRules} from '../../../shared/knowledge-scene.ts';
let runtime;
export function installClientPacketRules(value){
  if(['ClientPacketSession','ClientJsonSession','ClientSelectorSession','ClientMaterialSession'].some(name=>typeof value?.[name]!=='function'))throw new TypeError('Generated client packet Rust rules are unavailable');
  runtime=value;
  installKnowledgeSceneRules(value);
}
export function createClientPacketSession(...args){
  if(!runtime)throw new Error('Client packet Rust rules are not installed');
  return new runtime.ClientPacketSession(...args);
}
export const packetMissing=value=>runtime.ClientPacketSession.member_missing(Boolean(value));

export const createClientJsonSession=()=>new runtime.ClientJsonSession();
export const packetSetKey=key=>runtime.ClientJsonSession.request_key_length(key.length)&&runtime.ClientJsonSession.request_set_key(Uint16Array.from({length:key.length},(_,index)=>key.charCodeAt(index)));
export const packetString=value=>runtime.ClientJsonSession.string_element(typeof value==='string');
export const createClientSelectorSession=()=>new runtime.ClientSelectorSession();
export const createClientMaterialSession=(...args)=>new runtime.ClientMaterialSession(...args);
export const packetKindUnits=value=>runtime.ClientMaterialSession.kind_length(typeof value==='string',typeof value==='string'?value.length:0)
  ?Uint16Array.from({length:value.length},(_,index)=>value.charCodeAt(index)):new Uint16Array();

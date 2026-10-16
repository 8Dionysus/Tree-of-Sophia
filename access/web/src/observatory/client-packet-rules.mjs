import {installLensSpecRules} from './lens-spec-rules.mjs';
import {installKnowledgeSceneRules} from '../../../shared/knowledge-scene.ts';
let runtime;
export function installClientPacketRules(value){
  if(['validate_client_packet_v1','validate_client_search_request_v1','validate_client_exploration_request_v1','validate_client_material_request_v1','validate_client_material_packet_v1','validate_client_claim_scope_v1','validate_client_claim_path_v1','client_request_set_key_v1'].some(name=>typeof value?.[name]!=='function'))throw new TypeError('Generated client packet Rust rules are unavailable');
  runtime=value;
  installKnowledgeSceneRules(value);
  installLensSpecRules(value);
}
function installed(){if(!runtime)throw new Error('Client packet Rust rules are not installed');return runtime;}
export const checkClientPacket=(...args)=>installed().validate_client_packet_v1(...args);
export const checkClientSearchRequest=(...args)=>installed().validate_client_search_request_v1(...args);
export const checkClientExplorationRequest=(...args)=>installed().validate_client_exploration_request_v1(...args);
export const packetSetKey=key=>installed().client_request_set_key_v1(key);
export const checkClientMaterialRequest=(...args)=>installed().validate_client_material_request_v1(...args);
export const checkClientMaterialPacket=(...args)=>installed().validate_client_material_packet_v1(...args);
export const checkClientClaimScope=(...args)=>installed().validate_client_claim_scope_v1(...args);
export const checkClientClaimPath=(...args)=>installed().validate_client_claim_path_v1(...args);

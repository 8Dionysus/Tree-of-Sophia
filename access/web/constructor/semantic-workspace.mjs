import {createConstructorModel,CONSTRUCTOR_LIMITS} from './model.mjs';
import {createJourneyNavigator} from './journey-state.mjs';

// These exact editions are admitted only after reviewing identical source bytes,
// material identities, hierarchy, edge topology and route stops. New editions
// need their own compatibility review.
export const SEMANTIC_WORKSPACE_CARRY=Object.freeze({
 from:['4137a5a6723e063394b98077a38ca9ea7a4303932b2ebeabd4cf249ec5cb7bb7','bee55dc3b8b786fa77811eeb9356da303c1e2d7e06e43b95e6e5e9e9fb9d5157'],
 to:'cc53c03033b11ffcabb9d650f9c7832dd8b56136892a70e92d736173e1527edf',
});

/** Copy a validated personal workspace once; never alter the earlier edition. */
export async function carrySemanticWorkspace(storage,library,routes,binding=SEMANTIC_WORKSPACE_CARRY,locks=globalThis.navigator?.locks){
 const result={copied:[],errors:[]},previous=Array.isArray(binding.from)?binding.from:[binding.from];
 if(!storage||library.fingerprint!==binding.to||!previous.length||previous.includes(binding.to)||new Set(previous).size!==previous.length||![...previous,binding.to].every(v=>/^[a-f0-9]{64}$/.test(v)))return result;
 // Editions are ordered newest first. A present but invalid newer workspace
 // must be surfaced, never silently replaced with an older saved version.
 const previousPacket=prefix=>{for(const fingerprint of previous){const raw=storage.getItem(prefix+fingerprint);if(raw!==null)return {fingerprint,raw};}return null;};
 const pairs=[['tree','tos-living-tree-v1:'],['route','tos-reading-route-v1:']];
 if(typeof locks?.request!=='function'){
  for(const [kind,prefix]of pairs)try{if(storage.getItem(prefix+binding.to)===null&&previousPacket(prefix))result.errors.push({kind,message:'Coordinated browser storage is unavailable'});}catch(error){result.errors.push({kind,message:String(error.message??error)});}
  return result;
 }
 try{return await locks.request('tos-semantic-workspace:'+binding.to,()=>{
 for(const [kind,prefix] of pairs){
  try{
   const target=prefix+binding.to;
   // Even invalid or deliberately empty current data belongs to the user.
   if(storage.getItem(target)!==null)continue;
   const earlier=previousPacket(prefix);if(!earlier)continue;const {raw,fingerprint}=earlier;
   if(typeof raw!=='string'||raw.length*2>CONSTRUCTOR_LIMITS.bytes)throw Error('Saved data exceeds the workspace limit');
   let packet=raw;
   if(kind==='tree'){
    const candidate=JSON.parse(raw);if(candidate?.libraryFingerprint!==fingerprint)throw Error('Previous workspace identity differs');
    packet=JSON.stringify({...candidate,libraryFingerprint:binding.to});
    const checked=createConstructorModel(library,{storage:{getItem:()=>packet},key:'semantic-carry-check'});
    if(checked.persistenceError())throw Error(checked.persistenceError());
   }else{
    const checked=createJourneyNavigator(routes,{storage:{getItem:()=>raw}});
    if(checked.error()||!checked.savedPoint())throw Error(checked.error()??'Saved route is empty');
   }
   if(storage.getItem(target)!==null)continue;
   storage.setItem(target,packet);result.copied.push(kind);
  }catch(error){result.errors.push({kind,message:String(error.message??error)});}
 }
 return result;
 });}catch(error){return {copied:[],errors:[{kind:'tree',message:String(error.message??error)}]};}
}

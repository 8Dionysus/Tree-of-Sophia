/** Browser custody for the Rust constructor authoring session. */
import {createConstructorModel as createLegacyConstructorModel} from './model-legacy.mjs';
import {createBrowserResearchWorkspace} from '../src/research-workspace-rust.ts';

export const CONSTRUCTOR_SCHEMA='tos_constructor_workspace_v1';
export const CONSTRUCTOR_LIMITS=Object.freeze({nodes:200,edges:600,history:64,bytes:1_000_000});
let ConstructorSession;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const bytes=value=>encoder.encode(JSON.stringify(value));
const decoded=value=>decoder.decode(value);
const errorText=cause=>cause instanceof Error?cause.message:String(cause);
const normalizePacket=packet=>{
  if(typeof packet!=='string'||packet.length*2>CONSTRUCTOR_LIMITS.bytes||encoder.encode(packet).length>CONSTRUCTOR_LIMITS.bytes)
    throw new Error('constructor packet exceeds the 1 MB storage limit');
  try{return JSON.stringify(JSON.parse(packet));}
  catch{throw new Error('constructor packet is not valid JSON');}
};

/** Called by the live browser entry after its exact generated WASM initializes. */
export function installConstructorRules(runtime){
  const session=runtime?.BrowserConstructorSession??runtime;
  if(typeof session!=='function')throw new Error('constructor WASM session is unavailable');
  ConstructorSession=session;
}

export function createConstructorModel(library,{storage,key='tos.constructor.v1'}={}){
  // Direct source tests retain the original oracle until OPS builds the matched
  // product. The serving entry installs the Rust session before creating a model.
  if(!ConstructorSession)return createLegacyConstructorModel(library,{storage,key});
  if(typeof key!=='string'||!key.trim()||key.length>256)throw new Error('storage key must be nonempty text of at most 256 characters');
  const machine=new ConstructorSession(bytes(library));
  let persistence=storage,blocked=false,lastError=null;
  if(storage===undefined){try{persistence=typeof globalThis.localStorage==='undefined'?null:globalThis.localStorage;}
    catch(cause){lastError=`Local storage unavailable: ${errorText(cause)}`;persistence=null;}}
  if(persistence){try{const saved=persistence.getItem(key);if(saved!==null)machine.import_saved(normalizePacket(saved));}
    catch(cause){blocked=true;lastError=`Saved tree could not be loaded; original storage is protected: ${errorText(cause)}`;}}
  const listeners=new Set();let disposed=false;
  // A rendering pass repeatedly asks for the same tree. Keep only the last
  // Rust-emitted projection; edits, validation and history remain in the machine.
  let currentPacket=decoded(machine.export_packet()),currentState=JSON.parse(currentPacket);
  const refresh=()=>{const packet=decoded(machine.export_packet()),state=JSON.parse(packet);
    currentPacket=packet;currentState=state;};
  const getState=()=>structuredClone(currentState);
  const exportPacket=()=>currentPacket;
  const persist=()=>{if(!persistence||blocked)return;
    try{persistence.setItem(key,exportPacket());lastError=null;}
    catch(cause){lastError=`Tree remains in memory; saving failed: ${errorText(cause)}`;}};
  const changed=()=>{refresh();persist();for(const listener of listeners)listener(getState());};
  const apply=(kind,fields={})=>{
    const result=JSON.parse(decoded(machine.apply(bytes({kind,...fields}))));
    if(result.changed)changed();return result.value;
  };
  const exportResearch=()=>{
    const plan=JSON.parse(decoded(machine.research_plan()));
    const workspace=createBrowserResearchWorkspace({sessionId:'constructor-local',persistence:false});
    try{
      for(const action of plan.actions){if(action.kind==='hypothesis')workspace.addHypothesis(action.input);
        else workspace.addNote(action.input);}
      const packet=workspace.exportPacket();
      if(packet.length>1_000_000)throw new Error('research packet exceeds its 1 MB limit; use the constructor packet to retain this tree');
      return packet;
    }finally{workspace.dispose?.();}
  };
  return {
    getState,persistenceError:()=>lastError,
    dispose(){if(disposed)return;disposed=true;listeners.clear();machine.free();},
    subscribe(listener){if(typeof listener!=='function')throw new Error('listener must be a function');listeners.add(listener);return()=>listeners.delete(listener);},
    addMaterial:(id,options={})=>apply('material.add',{id,options}),
    expand:id=>apply('material.expand',{id}),
    addDraft:input=>apply('draft.add',{input}),
    addDraftWithContext:(input,options={})=>apply('draft.context',{input,options}),
    editDraft:(id,patch)=>apply('draft.edit',{id,patch}),
    connect:(from,to,relation,label)=>apply('edge.connect',{from,to,relation,...(label===undefined?{}:{label})}),
    removeNode:id=>apply('node.remove',{id}),removeEdge:id=>apply('edge.remove',{id}),
    moveNode:(id,position)=>apply('node.move',{id,position}),rename:title=>apply('title.rename',{title}),
    canUndo:()=>machine.can_undo(),canRedo:()=>machine.can_redo(),
    undo:()=>apply('undo'),redo:()=>apply('redo'),clear:()=>apply('clear'),
    growAtlas:selection=>apply('atlas.grow',{selection}),seedAtlas:()=>apply('atlas.seed'),seed:()=>apply('seed'),
    exportPacket,importPacket:packet=>apply('import',{packet:normalizePacket(packet)}),exportResearch,
  };
}

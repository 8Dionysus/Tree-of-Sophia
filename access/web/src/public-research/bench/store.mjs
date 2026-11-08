// The workbench kept in this browser. Every change is an operation on the state (model.mjs); the store keeps the
// state with a revision, writes it once per change, keeps the way back and forward, and follows the same bench
// changed in another tab. When another tab wrote first, the operation runs again on what that tab wrote, so neither
// change is lost.

import {BENCH_SCHEMA,emptyState,validateState} from './model.mjs';

export const BENCH_KEY='tos.public-research.bench.v1';
const HISTORY=40;

export function benchKey(profile){return `${BENCH_KEY}.${profile}`;}

export function createBenchStore({profile,locks=globalThis.navigator?.locks,storage=locks?globalThis.localStorage:globalThis.sessionStorage}={}){
  const key=benchKey(profile),listeners=new Set();
  let state,error='',undo=[],redo=[];
  const read=()=>{try{const raw=storage?.getItem(key);return raw?validateState(JSON.parse(raw)):null;}catch(failure){error=failure?.message??String(failure);return null;}};
  state=read()??emptyState();
  const notify=()=>{for(const listener of listeners)listener(state);};
  const write=next=>{
    if(!storage){error='storage-unavailable';return;}
    try{storage.setItem(key,JSON.stringify(next));error='';}catch(failure){error=failure?.message??String(failure);}
  };
  // Browsers without Web Locks use per-tab session storage to avoid concurrent writes.
  const serialize=operation=>locks?locks.request(key,operation):operation();
  function refresh(){
    const stored=read();
    if(stored&&stored.revision>state.revision){state=stored;undo=[];redo=[];return true;}
    return false;
  }
  /** Applies `op(state) → state`. History keeps layout changes; a keystroke in a field passes `{history:false}`. */
  function update(op,{history=true}={}){
    const refreshed=refresh(),base=state;
    const next=op(base);
    if(!next||next===base){if(refreshed)notify();return state;}
    if(history){undo.push(base);if(undo.length>HISTORY)undo.shift();redo=[];}
    state={...next,schema:BENCH_SCHEMA,revision:base.revision+1};
    write(state);notify();return state;
  }
  function step(from,to){
    if(refresh()){notify();return state;}
    if(!from.length)return state;
    const previous=from.pop();to.push(state);
    state={...previous,revision:state.revision+1};write(state);notify();return state;
  }
  const onStorage=event=>{
    if(event.key!==key||event.storageArea!==storage)return;
    const stored=read();if(stored&&stored.revision>state.revision){state=stored;undo=[];redo=[];notify();}
  };
  globalThis.addEventListener?.('storage',onStorage);
  return {
    get state(){return state;},
    get error(){return error;},
    update:(op,options)=>serialize(()=>update(op,options)),
    undo:()=>serialize(()=>step(undo,redo)),redo:()=>serialize(()=>step(redo,undo)),
    canUndo:()=>undo.length>0,canRedo:()=>redo.length>0,
    subscribe(listener){listeners.add(listener);return ()=>listeners.delete(listener);},
    exportPacket:()=>JSON.stringify(state),
    dispose(){globalThis.removeEventListener?.('storage',onStorage);listeners.clear();},
  };
}

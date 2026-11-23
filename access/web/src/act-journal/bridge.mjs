// The act journal at the person's bridge (tier 1, TOS-D-0083). The bridge (`tos bridge`) serves the stands' pages
// from the person's own machine, keeps the journal in its store and folds it with the same Rust rules as the
// browser. The page holds no copy: it appends drafts there, reads folds and records in windows, and hears of every
// act the bridge takes (its own, other tabs', agents' over MCP) through one event stream. The interface is the
// browser journal's (journal.mjs), so the views do not know which tier they read.
import {draftAct} from './drafts.mjs';
import {ActJournalError} from './journal.mjs';

export const BRIDGE_API='/__bridge/v1/';
const LOOPBACK=new Set(['127.0.0.1','localhost','[::1]','::1']);

/**
 * The bridge's status when this page is the bridge's own (a loopback page whose origin answers as a bridge and
 * recognises the person), or null. The published site is never on a loopback address, so it never asks.
 */
export async function detectBridge({fetch=globalThis.fetch,location=globalThis.location,timeoutMs=1500}={}){
  if(!fetch||!LOOPBACK.has(String(location?.hostname??'')))return null;
  const controller=typeof AbortController==='function'?new AbortController():null;
  const timer=controller?setTimeout(()=>controller.abort(),timeoutMs):null;
  try{
    const response=await fetch(`${BRIDGE_API}hello`,{credentials:'same-origin',cache:'no-store',signal:controller?.signal});
    if(!response.ok)return null;
    const status=await response.json();
    return status?.schema==='tos.bridge.v1'&&status.person===true?status:null;
  }catch{return null;}
  finally{if(timer)clearTimeout(timer);}
}

/** The bridge's store over its HTTP interface: windows of records, appends, folds, control and the event stream. */
export function createBridgeStore({fetch=globalThis.fetch,EventSource=globalThis.EventSource}={}){
  const call=async(path,{method='GET',body}={})=>{
    const response=await fetch(`${BRIDGE_API}${path}`,{method,credentials:'same-origin',cache:'no-store',
      headers:body===undefined?undefined:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});
    const value=await response.json().catch(()=>null);
    if(!response.ok)throw new ActJournalError(value?.error??`bridge_http_${response.status}`);
    return value;
  };
  let source=null;
  const surfaceListeners=new Set();
  // The surface at the bridge (surface.mjs): a refusal keeps the rules' whole answer for the agent.
  const surfaceCall=async(path,body)=>{
    const response=await fetch(`${BRIDGE_API}${path}`,{method:body===undefined?'GET':'POST',credentials:'same-origin',cache:'no-store',
      headers:body===undefined?undefined:{'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});
    const value=await response.json().catch(()=>null);
    if(!response.ok)throw Object.assign(new Error(value?.message??value?.error??`bridge_http_${response.status}`),{detail:value});
    return value;
  };
  return {
    kind:'bridge',persistent:true,
    status:()=>call('hello'),
    control:change=>call('control',{method:'POST',body:change}),
    lastSeq:async()=>(await call('acts?from=1&limit=1')).last,
    readFrom:async(seq,limit=1000)=>(await call(`acts?from=${seq}&limit=${limit}`)).rows,
    readBefore:async(seq,limit=100)=>(await call(`acts?before=${Math.min(seq,Number.MAX_SAFE_INTEGER)}&limit=${limit}`)).rows,
    getById:async id=>(await call(`acts?id=${encodeURIComponent(id)}`)).row,
    getByIds:async ids=>(await call('records',{method:'POST',body:{ids}})).rows,
    append:drafts=>call('acts',{method:'POST',body:drafts}),
    query:request=>call('query',{method:'POST',body:request}),
    /** Hear the bridge's last sequence number after each append; returns the way to stop. */
    watch(listener){
      if(typeof EventSource!=='function')return ()=>{};
      source=new EventSource(`${BRIDGE_API}events`,{withCredentials:true});
      source.addEventListener('acts',event=>{try{listener(JSON.parse(event.data).last);}catch{}});
      source.addEventListener('surface',event=>{let value;try{value=JSON.parse(event.data);}catch{return;}for(const fn of surfaceListeners)fn(value);});
      return ()=>{source?.close();source=null;};
    },
    surfaceState:()=>surfaceCall('surface'),
    surfaceApply:body=>surfaceCall('surface',body),
    surfaceReceipt:receipt=>surfaceCall('surface/receipt',receipt),
    onSurface(listener){surfaceListeners.add(listener);return ()=>surfaceListeners.delete(listener);},
    close(){source?.close();source=null;surfaceListeners.clear();},
  };
}

/** The journal at the bridge, with the browser journal's interface. */
export function createBridgeJournal({store=createBridgeStore()}={}){
  const listeners=new Set();
  const status={loaded:0,damaged:null,persistent:true,warning:null,tier:'bridge'};
  let known=0,disposed=false;
  const notify=detail=>{for(const listener of listeners){try{listener(detail);}catch(error){console.error('Act journal listener',error);}}};
  const heard=last=>{
    if(!Number.isInteger(last)||last<=known)return;
    const added=last-known;known=last;status.loaded=last;notify({type:'remote',added});
  };
  const ready=store.lastSeq().then(last=>{known=Math.max(known,last);status.loaded=known;notify({type:'ready',loaded:known});return status;},
    error=>{status.damaged={seq:0,error:error?.code??String(error?.message??error)};return status;});
  const stop=store.watch?.(heard)??(()=>{});
  const fail=error=>error instanceof ActJournalError?error:new ActJournalError(String(error?.message??error));

  async function appendMany(drafts){
    if(disposed)throw new ActJournalError('act_journal_closed');
    await ready;
    let result;
    try{result=await store.append(drafts);}catch(error){throw fail(error);}
    const out=result.appended??[];
    if(out.length){known=Math.max(known,out.at(-1).seq);status.loaded=known;notify({type:'append',acts:out});}
    if(result.failure){
      const refused=new ActJournalError(String(result.failure.error));refused.index=result.failure.index;refused.appended=out;throw refused;
    }
    return out;
  }

  return {
    ready,status,tier:'bridge',
    append:async draft=>(await appendMany([draft]))[0],
    appendMany,
    act:fields=>appendMany([draftAct(fields)]).then(items=>items[0]),
    query:async request=>{await ready;try{return await store.query(request);}catch(error){throw fail(error);}},
    record:async id=>{await ready;return (await store.getById(id))?.record??null;},
    /** Whole records by ID in one read, in the order asked; a missing one is null. */
    records:async ids=>{await ready;const rows=await store.getByIds(ids);const by=new Map(rows.map(row=>[row.id,row.record]));return ids.map(id=>by.get(id)??null);},
    recent:async({before=Number.MAX_SAFE_INTEGER,limit=100}={})=>{await ready;return (await store.readBefore(before,limit)).map(row=>({seq:row.seq,record:row.record}));},
    /** The bridge's own state and the person's control over the army: pause and the agents' tier. */
    bridgeStatus:()=>store.status(),
    control:change=>store.control(change),
    /** The surface the bridge holds for agents to shape (surface.mjs). */
    surfaceChannel:()=>store.surfaceState?{state:store.surfaceState,apply:store.surfaceApply,receipt:store.surfaceReceipt,listen:fn=>store.onSurface(fn)}:null,
    get size(){return known;},
    subscribe(listener){listeners.add(listener);return ()=>listeners.delete(listener);},
    dispose(){disposed=true;listeners.clear();stop();store.close?.();},
  };
}

/**
 * The page's act journal: the bridge's when the page is served by the person's bridge, the browser's own
 * otherwise. The choice is made once, in the background; every method waits for it.
 */
export function openActJournal({detect=detectBridge,bridge=()=>createBridgeJournal(),browser}={}){
  let current=null;const pending=new Set();let disposed=false;
  const chosen=Promise.resolve().then(()=>detect()).catch(()=>null).then(found=>{
    current=found?bridge():browser();
    for(const listener of pending)listener.stop=current.subscribe(listener.fn);
    if(disposed)current.dispose();
    return current;
  });
  const wait=name=>(...args)=>chosen.then(journal=>journal[name](...args));
  return {
    ready:chosen.then(journal=>journal.ready),
    get status(){return current?.status??{loaded:0,damaged:null,persistent:false,warning:null};},
    get tier(){return current?.tier??(current?'browser':null);},
    chosen,
    append:wait('append'),appendMany:wait('appendMany'),act:wait('act'),query:wait('query'),record:wait('record'),records:wait('records'),recent:wait('recent'),
    bridgeStatus:async()=>{const journal=await chosen;return journal.bridgeStatus?.()??null;},
    control:async change=>{const journal=await chosen;if(!journal.control)throw new ActJournalError('bridge_absent');return journal.control(change);},
    get size(){return current?.size??0;},
    subscribe(fn){
      if(current)return current.subscribe(fn);
      const listener={fn,stop:null};pending.add(listener);
      return ()=>{pending.delete(listener);listener.stop?.();};
    },
    dispose(){disposed=true;current?.dispose();},
  };
}

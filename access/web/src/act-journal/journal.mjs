// The act journal of this browser: records in a store, folds in the shared Rust journal (tos-web-rules,
// act_journal.rs). Appending happens under one lock per journal, so tabs never give two records the same
// sequence; a tab that missed records reads them from the store before it appends. The machine holds only what
// the store holds: when the store refuses a write, the machine is built again from the stored records before the
// next act, so the next act takes the next stored sequence and the journal opens whole after a reload.
import {ACT_KINDS,draftAct} from './drafts.mjs';

let JournalRules=null;
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const bytes=value=>encoder.encode(JSON.stringify(value));
const parse=value=>JSON.parse(decoder.decode(value));
const LOAD_BATCH=1000;

/** Installed by the browser entry once the generated rules have initialized. */
export function installActJournalRules(runtime){
  if(typeof runtime?.ActJournalV1!=='function')throw new TypeError('The act journal rules are unavailable.');
  JournalRules=runtime.ActJournalV1;
}

export class ActJournalError extends Error{
  constructor(code,message=code){super(message);this.name='ActJournalError';this.code=code;}
}
const failure=error=>new ActJournalError(String(error?.message??error),String(error?.message??error));

/**
 * `store` is an act store (store.mjs or the bridge's) or a promise of one, so a page can create the journal before
 * its database has opened. `registry` defaults to the base kinds. `name` keys the lock and the cross-tab channel.
 * Every method waits for `ready`, which reads the stored records in the background.
 */
export function createActJournal({store,registry=ACT_KINDS,name='tos-act-journal-v1',locks=globalThis.navigator?.locks,channel=globalThis.BroadcastChannel}={}){
  if(!JournalRules)throw new ActJournalError('act_rules_unavailable','The act journal rules are unavailable.');
  if(!store)throw new ActJournalError('act_store_unavailable','The act store is unavailable.');
  let machine=new JournalRules(bytes(registry));
  const listeners=new Set();
  const status={loaded:0,damaged:null,persistent:false,warning:null};
  const opened=Promise.resolve(store).then(value=>{store=value;status.persistent=Boolean(value.persistent);status.warning=value.warning??null;});
  let known=0,disposed=false,stale=false;
  const broadcast=channel?new channel(name):null;

  // Read every record the machine has not seen yet. A refused stored record marks the journal damaged and stops.
  async function sync(){
    if(status.damaged)return 0;
    let added=0;
    for(;;){
      const rows=await store.readFrom(known+1,LOAD_BATCH);
      if(!rows.length)break;
      for(const [offset,row] of rows.entries())if(row.seq!==known+1+offset){status.damaged={seq:row.seq,error:'act_store_sequence'};return added;}
      const result=parse(machine.appendBatch(bytes(rows.map(row=>row.record))));
      known+=result.appended.length;added+=result.appended.length;
      if(result.failure){status.damaged={seq:rows[result.failure.index].seq,error:result.failure.error};break;}
      if(rows.length<LOAD_BATCH)break;
    }
    status.loaded=known;
    return added;
  }
  // The machine again from the store alone. Until this succeeds the journal stays stale and tries again before
  // the next act; a stored record it refuses marks the journal damaged, as on opening.
  async function rebuild(){
    stale=true;
    machine.free?.();machine=new JournalRules(bytes(registry));
    known=0;status.damaged=null;
    await sync();
    stale=false;
  }
  // Store the acts the machine has just taken; a refused write takes them back out of the machine.
  async function persist(out){
    if(!out.length)return;
    try{await store.put(out.map(item=>({seq:item.seq,id:item.record.id,record:item.record})));}
    catch(error){await rebuild().catch(()=>{});throw error;}
    known+=out.length;
  }
  const serialize=task=>locks?locks.request(name,task):task();
  const notify=detail=>{for(const listener of listeners){try{listener(detail);}catch(error){console.error('Act journal listener',error);}}};

  const ready=opened.then(()=>serialize(sync)).then(()=>{notify({type:'ready',loaded:known});return status;});
  if(broadcast)broadcast.onmessage=()=>{void serialize(sync).then(added=>{if(added)notify({type:'remote',added});});};

  /** Append drafts in order. Returns the stored records with their sequence numbers and lease collisions. */
  async function appendMany(drafts){
    if(disposed)throw new ActJournalError('act_journal_closed');
    await ready;
    return serialize(async()=>{
      if(stale)await rebuild();
      await sync();
      if(status.damaged)throw new ActJournalError('act_journal_damaged',`The act journal stopped at record ${status.damaged.seq}: ${status.damaged.error}`);
      const out=[];
      for(const draft of drafts){
        let result;
        try{result=parse(machine.append(bytes(draft)));}catch(error){
          // Records already appended in this call stay; the caller learns which draft was refused.
          await persist(out);
          const refused=failure(error);refused.index=out.length;refused.appended=out;throw refused;
        }
        out.push(result);
      }
      await persist(out);
      status.loaded=known;
      broadcast?.postMessage({seq:known});
      notify({type:'append',acts:out});
      return out;
    });
  }

  return {
    ready,status,
    append:async draft=>(await appendMany([draft]))[0],
    appendMany,
    /** Make and append one act from its parts (see draftAct). */
    act:fields=>appendMany([draftAct(fields)]).then(items=>items[0]),
    /** A fold: summary, act, target, board, review, leases, presence, history or kinds. */
    query:async request=>{await ready;try{return parse(machine.query(bytes(request)));}catch(error){throw failure(error);}},
    /** The whole stored record of an act. */
    record:async id=>{await ready;return (await store.getById(id))?.record??null;},
    /** Whole records by ID, in the order asked; a missing one is null. */
    records:async ids=>{await ready;return Promise.all(ids.map(async id=>(await store.getById(id))?.record??null));},
    /** Stored records, newest first, before a sequence number. */
    recent:async({before=Number.MAX_SAFE_INTEGER,limit=100}={})=>{await ready;return (await store.readBefore(before,limit)).map(row=>({seq:row.seq,record:row.record}));},
    get size(){return known;},
    subscribe(listener){listeners.add(listener);return ()=>listeners.delete(listener);},
    dispose(){disposed=true;listeners.clear();broadcast?.close();void opened.then(()=>{machine.free?.();store.close?.();});},
  };
}

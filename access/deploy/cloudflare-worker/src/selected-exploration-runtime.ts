/** Concrete physical reads selected by the shared Rust exploration algorithm.
 * Persisted checkpoint state is separate from the suspended per-page session. */
export type ExplorationNeed =
  | {operation:'rows';kind:'node'|'relation';ids:string[];allow_missing:boolean;allow_ambiguous:boolean}
  | {operation:'focus';field:'id'|'entity_id'|'native_id';id:string;sources:string[];source_priority:[string,number][];limit:number}
  | {operation:'identity';node_id:string;entity_id:string|null;declared_prefix:string|null;expanded_entities:string[];after:string;sources:string[];limit:number}
  | {operation:'adjacency';node_id:string;after:string;limit:number};

export class SelectedExplorationError extends Error {
  readonly code:string;
  constructor(code:string) {super(`published exploration continuation: ${code}`);this.code=code;}
}
export function explorationError(error:unknown):never {
  if(typeof error==='string')throw new SelectedExplorationError(error);
  throw error;
}

export interface PublishedExplorationSession {
  need():Uint8Array|undefined;
  resume_rows(rows:Uint8Array,sizes:Uint32Array,ambiguous:Uint8Array):void;
  resume_focus(matched:number,rows:Uint8Array,sizes:Uint32Array):void;
  resume_ids(ids:Uint8Array):void;
  paused():boolean;
  state():Uint8Array;
  finish(cursor?:string):Uint8Array;
  free():void;
}
export interface PublishedExplorationModule {
  ExplorationSession:new(request:Uint8Array,revision:string,data_revision:string,epoch:bigint,
    top:Uint8Array,state:Uint8Array,admission:Uint8Array)=>PublishedExplorationSession;
  validate_exploration_request_wasm_v1(request:Uint8Array,admission:Uint8Array):string|undefined;
  validate_exploration_replay_wasm_v1(packet:Uint8Array,admission:Uint8Array):void;
  exploration_cache_version_wasm_v1():string;
}
export interface PublishedExplorationReads {
  payloads(need:Extract<ExplorationNeed,{operation:'rows'}>):Promise<{rows:Uint8Array;sizes:number[];ambiguous:string[]}>;
  focus(need:Extract<ExplorationNeed,{operation:'focus'}>):Promise<{matched:number;rows:Uint8Array;sizes:number[]}>;
  identities(need:Extract<ExplorationNeed,{operation:'identity'}>):Promise<{id:string}[]>;
  adjacency(need:Extract<ExplorationNeed,{operation:'adjacency'}>):Promise<{id:string}[]>;
}

/** Only the pinned Rust needs drive physical reads. Cancellation is cooperative
 * around bounded synchronous WASM calls and D1 awaits, not an in-WASM interrupt.
 * This function does not select, mutate or serialize checkpoint state. */
export async function advanceExploration(session:PublishedExplorationSession,physical:PublishedExplorationReads,
  signal?:AbortSignal):Promise<void> {
  const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true});
  try {
    while(true) {
      signal?.throwIfAborted();
      const raw=session.need();
      signal?.throwIfAborted();
      if(raw===undefined)return;
      const need=JSON.parse(decoder.decode(raw)) as ExplorationNeed;
      switch(need.operation) {
        case 'rows': {
          const result=await physical.payloads(need);signal?.throwIfAborted();
          session.resume_rows(result.rows,Uint32Array.from(result.sizes),encoder.encode(JSON.stringify(result.ambiguous)));break;
        }
        case 'focus': {
          const result=await physical.focus(need);signal?.throwIfAborted();
          session.resume_focus(result.matched,result.rows,Uint32Array.from(result.sizes));break;
        }
        case 'identity':case 'adjacency': {
          const rows=need.operation==='identity'?await physical.identities(need):await physical.adjacency(need);
          signal?.throwIfAborted();session.resume_ids(encoder.encode(JSON.stringify(rows.map(row=>row.id))));break;
        }
        default:throw new SelectedExplorationError('CorruptSelectedCarrier');
      }
    }
  } catch(error){return explorationError(error);}
}

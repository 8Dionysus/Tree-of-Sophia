/** Concrete storage needs of the shared Rust published-v7 lens plan.
 * Request normalization, filter/fast-path selection, traversal and packet
 * construction belong to Rust. These fields describe bounded physical reads. */
export type LensKind='node'|'relation';
export type LensIdentityTerm={field:'id'|'entity_id'|'native_id';values:string[]};
export type LensIdentityGroup={all:boolean;terms:LensIdentityTerm[]};
export type LensCandidateIndex={kind:'source'|'union'}|{kind:'identity';field:LensIdentityTerm['field']};
export type LensMembership={all:boolean;terms:{field:'view_ids'|'graph_layers';all:boolean;values:string[]}[];drivers:[string,string][]};
export type LensEligibility={policy:'both'|'either';basis:string[];traversed:string[];pair_index?:boolean};
export type LensHeaderQuery={kind:LensKind;sources:string[];dimensions?:[string,string,string][];
  membership?:LensMembership;predicate_ids:string[];excluded_predicates:string[];excluded_relation_types:string[];
  endpoint?:{side:'from'|'to';id:string};eligible?:LensEligibility};
export type LensHeader={id:string;sort_key:string;from_id:string;to_id:string};
export type LensNeed=
  |{operation:'auxiliary';compact:boolean;membership:boolean}
  |{operation:'rows';kind:LensKind;ids:string[];representation:'full'|'covered_compact'}
  |{operation:'candidates';kind:LensKind;sources:string[];identities:LensIdentityGroup[];index:LensCandidateIndex;after?:{kind:'id';id:string};limit:number}
  |{operation:'focus';field:'id'|'entity_id'|'native_id';identifier:string;sources:string[];source_priority:string[];limit:number}
  |{operation:'incident';identifier:string;after:string;limit:number}
  |{operation:'ordered';query:LensHeaderQuery;after?:[string,string];limit:number}
  |{operation:'aliases';entities:string[];sources:string[];exclude:string[];limit:number}
  |{operation:'sources';ids:string[]}
  |{operation:'count';query:LensHeaderQuery};

import {snapshotPacketResponse} from './selected-temporal-runtime.ts';
export interface PublishedLensSession {
  need():Uint8Array|undefined;
  resume_rows(rows:Uint8Array,sizes:Uint32Array):void;
  resume_candidates(rows:Uint8Array):void;
  resume_ids(ids:Uint8Array):void;
  resume_headers(headers:Uint8Array):void;
  resume_sources(sources:Uint8Array):void;
  resume_count(count:bigint):void;
  resume_stores(compact:boolean,membership:boolean):void;
  finish():Uint8Array;
  free():void;
}
export interface PublishedLensModule {
  LensSession:new(request:Uint8Array,operation:string,revision:string,top:Uint8Array,
    metadata:Uint8Array,catalog:Uint8Array,publication:Uint8Array,admission:Uint8Array)=>PublishedLensSession;
  validate_lens_request_wasm_v1(request:Uint8Array,operation:string,admission:Uint8Array):void;
}
export interface PublishedLensReadAccess {
  readonly sourceRevision:string;
  readonly top:Uint8Array;
  readonly metadata:Uint8Array;
  readonly catalog:Uint8Array;
  readonly publication:Uint8Array;
  readonly admission:Uint8Array;
  checkSelected():Promise<void>;
  payloads(need:Extract<LensNeed,{operation:'rows'}>):Promise<{rows:Uint8Array;sizes:number[]}>;
  candidates(need:Extract<LensNeed,{operation:'candidates'}>):Promise<{id:string}[]>;
  focus(need:Extract<LensNeed,{operation:'focus'}>):Promise<{id:string}[]>;
  incident(need:Extract<LensNeed,{operation:'incident'}>):Promise<{id:string}[]>;
  ordered(need:Extract<LensNeed,{operation:'ordered'}>):Promise<LensHeader[]>;
  aliases(need:Extract<LensNeed,{operation:'aliases'}>):Promise<{id:string}[]>;
  sources(need:Extract<LensNeed,{operation:'sources'}>):Promise<{id:string;source_graph:string}[]>;
  count(need:Extract<LensNeed,{operation:'count'}>):Promise<number>;
  auxiliary(need:Extract<LensNeed,{operation:'auxiliary'}>):Promise<{compact:boolean;membership:boolean}>;
}
export class SelectedLensError extends Error {
  readonly code:string;
  constructor(code:string){super(`published lens continuation: ${code}`);this.code=code;}
}
export function lensError(error:unknown):never {
  if(typeof error==='string')throw new SelectedLensError(error);
  throw error;
}

/** The published owner selects a snapshot, not a current-policy grant. The
 * cooperative driver checks cancellation around bounded synchronous WASM and
 * physical I/O. The actual snapshot and auxiliary consistency checks run at
 * whole-call completion and final demand-driven body handoff. */
export async function respondLensSnapshot(runtime:PublishedLensModule,selected:PublishedLensReadAccess,
  request:Uint8Array,operation:'compile'|'focus'|'stored',signal?:AbortSignal,method='GET'):Promise<Response> {
  const encoder=new TextEncoder(),json=(value:unknown)=>encoder.encode(JSON.stringify(value));
  signal?.throwIfAborted();
  let session:PublishedLensSession;
  try {session=new runtime.LensSession(request,operation,selected.sourceRevision,selected.top,
    selected.metadata,selected.catalog,selected.publication,selected.admission);}
  catch(error){return lensError(error);}
  let packet:Uint8Array;
  try {
    while(true) {
      signal?.throwIfAborted();
      const raw=session.need();
      if(raw===undefined){packet=session.finish();break;}
      const need=JSON.parse(new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(raw)) as LensNeed;
      switch(need.operation) {
        case 'rows': {
          const result=await selected.payloads(need);signal?.throwIfAborted();
          session.resume_rows(result.rows,Uint32Array.from(result.sizes));break;
        }
        case 'candidates': {
          const rows=await selected.candidates(need);signal?.throwIfAborted();
          session.resume_candidates(json(rows.map(({id})=>({id}))));break;
        }
        case 'focus': {
          const rows=await selected.focus(need);signal?.throwIfAborted();
          session.resume_ids(json(rows.map(row=>row.id)));break;
        }
        case 'incident': {
          const rows=await selected.incident(need);signal?.throwIfAborted();
          session.resume_ids(json(rows.map(row=>row.id)));break;
        }
        case 'aliases': {
          const rows=await selected.aliases(need);signal?.throwIfAborted();
          session.resume_ids(json(rows.map(row=>row.id)));break;
        }
        case 'ordered': {
          const rows=await selected.ordered(need);signal?.throwIfAborted();
          session.resume_headers(json(rows.map(({id,sort_key,from_id,to_id})=>({id,sort_key,from_id,to_id}))));break;
        }
        case 'sources': {
          const rows=await selected.sources(need);signal?.throwIfAborted();
          session.resume_sources(json(rows.map(({id,source_graph})=>[id,source_graph])));break;
        }
        case 'count': {
          const total=await selected.count(need);signal?.throwIfAborted();
          session.resume_count(BigInt(total));break;
        }
        case 'auxiliary': {
          const stores=await selected.auxiliary(need);signal?.throwIfAborted();
          session.resume_stores(stores.compact,stores.membership);break;
        }
        default:throw new SelectedLensError('CorruptSelectedCarrier');
      }
    }
  } catch(error){return lensError(error);}
  finally{session.free();}
  signal?.throwIfAborted();
  return snapshotPacketResponse(packet,selected.checkSelected.bind(selected),signal,method);
}

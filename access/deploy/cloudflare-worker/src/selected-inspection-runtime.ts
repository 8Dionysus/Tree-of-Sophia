/** Physical continuation for the shared Rust node/relation inspection plan.
 * No alias resolution, packet construction, or source target projection lives
 * here. The D1 consumer owns verified publication selection and bounded I/O. */
import {snapshotPacketResponse} from './selected-temporal-runtime.ts';
export type InspectionNeed =
  | {operation:'lookup'; kind:'node'|'relation'; selector:'id'|'entity_id'|'native_id'; identifier:string; limit:number}
  | {operation:'incident'; ids:string[]; relation_limit:number}
  | {operation:'endpoints'; ids:string[]};
export interface InspectionSession {
  need(): Uint8Array | undefined;
  resume_lookup(rows: Uint8Array): void;
  resume_incident(total: bigint, rows: Uint8Array): void;
  resume_endpoints(rows: Uint8Array): void;
  finish(): Uint8Array;
  free(): void;
}
export interface InspectionModule {
  InspectionSession: new(request: Uint8Array, revision:string, top:Uint8Array, admission:Uint8Array) => InspectionSession;
  validate_inspect_request_wasm_v1(request:Uint8Array, admission:Uint8Array):void;
}
export interface InspectionReadAccess {
  readonly sourceRevision:string;
  readonly top:Uint8Array;
  readonly admission:Uint8Array;
  checkSelected():Promise<void>;
  lookup(need:Extract<InspectionNeed,{operation:'lookup'}>):Promise<Uint8Array>;
  incident(need:Extract<InspectionNeed,{operation:'incident'}>):Promise<{total:bigint;rows:Uint8Array}>;
  endpoints(need:Extract<InspectionNeed,{operation:'endpoints'}>):Promise<Uint8Array>;
}
export class SelectedInspectionError extends Error {
  readonly code:string;
  constructor(code:string) {super(`selected inspection continuation: ${code}`);this.code=code;}
}
export function inspectionError(error:unknown):never {
  if(typeof error==='string') throw new SelectedInspectionError(error);
  throw error;
}

export async function respondInspectionSnapshot(runtime:InspectionModule, selected:InspectionReadAccess,
  request:Uint8Array, signal?:AbortSignal, method='GET'):Promise<Response> {
  const check = async():Promise<void> => {
    signal?.throwIfAborted();await selected.checkSelected();signal?.throwIfAborted();
  };
  await check();
  let session:InspectionSession;
  try {session=new runtime.InspectionSession(request,selected.sourceRevision,selected.top,selected.admission);}
  catch(error){return inspectionError(error);}
  let bytes:Uint8Array;
  try {
    while(true) {
      await check();
      const raw=session.need();
      if(raw===undefined){bytes=session.finish();break;}
      // This is the pinned Rust physical need, not a caller-supplied query.
      const need=JSON.parse(new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(raw)) as InspectionNeed;
      switch(need.operation) {
        case 'lookup': {
          const rows=await selected.lookup(need);await check();session.resume_lookup(rows);break;
        }
        case 'incident': {
          const result=await selected.incident(need);await check();session.resume_incident(result.total,result.rows);break;
        }
        case 'endpoints': {
          const rows=await selected.endpoints(need);await check();session.resume_endpoints(rows);break;
        }
        default:throw new SelectedInspectionError('CorruptSelectedCarrier');
      }
    }
  } catch(error){return inspectionError(error);}
  finally{session.free();}
  return snapshotPacketResponse(bytes,check,signal,method);
}

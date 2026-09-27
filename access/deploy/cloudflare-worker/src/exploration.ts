import {HttpError,type Item} from './common.ts';
import {REQUEST_V2,RESULT_V2} from './exploration-origin.ts';
import {NativeD1Read,nativeD1Limits,nativeUnavailable} from './native-d1-read.ts';
import {readNativeInspectionPublication} from './native-inspection-store.ts';
import {PublishedExplorationD1Transport} from './native-exploration-store.ts';
import {advanceExploration,explorationError,SelectedExplorationError,type PublishedExplorationModule,
  type PublishedExplorationSession} from './selected-exploration-runtime.ts';
import {snapshotPacketResponse} from './selected-temporal-runtime.ts';
import {NativeBudgetExceeded} from '../../../shared/native-semantics.ts';
import {nativeField,nativePacketJson,type NativeRef} from './native-lens.ts';

const VERSION = 'tos-exploration-d1-execution-v6';
const TTL = 900_000;
const MAX_BYTES = 1_048_576;
const ADJACENCY_QUERIES = 24;
type Snapshot = {epoch: number; revision: string; source_revision: string; authority_boundary: NativeRef};
type Checkpoint = {token: string; expires: number; epoch: number; version: string; bytes:number; state: string | null; response: string | null};

function expired(): never { throw new HttpError(410, 'exploration expired or was evicted; restart from focus'); }
function conflict(): never { throw new HttpError(409, 'exploration snapshot changed; restart from focus'); }
function token(): string { return [...crypto.getRandomValues(new Uint8Array(32))].map(b => b.toString(16).padStart(2, '0')).join(''); }
function byteSize(text: string): number { return new TextEncoder().encode(text).length; }
const RESPONSE_COLUMN = `CASE WHEN typeof(response)='text' AND length(CAST(response AS BLOB))<=1048576 THEN response ELSE NULL END AS response,
  length(CAST(response AS BLOB)) AS response_bytes`;
async function checkpoint(db:D1Database,cursor:string,now:number):Promise<Checkpoint|null> {
  const row=await db.prepare(`SELECT token,
    CASE WHEN typeof(expires)='integer' THEN expires ELSE NULL END AS expires,
    CASE WHEN typeof(epoch)='integer' THEN epoch ELSE NULL END AS epoch,
    CASE WHEN typeof(bytes)='integer' AND bytes>=0 AND bytes<=1048576 THEN bytes ELSE NULL END AS bytes,
    CASE WHEN typeof(version)='text' AND length(CAST(version AS BLOB))<=128 THEN version ELSE NULL END AS version,
    CASE WHEN typeof(state)='text' AND length(CAST(state AS BLOB))<=1048576 THEN state ELSE NULL END AS state,
    length(CAST(state AS BLOB)) AS state_bytes,${RESPONSE_COLUMN}
    FROM knowledge_exploration_checkpoints WHERE token=? AND expires>? LIMIT 2`).bind(cursor,now)
    .all<Checkpoint & {state_bytes:number|null;response_bytes:number|null}>();
  if (!row.results.length) return null;
  if (row.results.length !== 1) nativeUnavailable('duplicate exploration checkpoint');
  const value=row.results[0]!;
  if ((value.state_bytes??0)>MAX_BYTES || (value.response_bytes??0)>MAX_BYTES) throw new NativeBudgetExceeded('exploration checkpoint exceeds 1 MiB');
  if (value.token!==cursor || !Number.isSafeInteger(value.epoch) || !Number.isSafeInteger(value.expires)
      || value.bytes!==(value.state_bytes??0)+(value.response_bytes??0)
      || typeof value.version!=='string' || (value.state===null)===(value.response===null)) nativeUnavailable('invalid exploration checkpoint framing');
  return value;
}
// No Sessions API: D1 bindings read the primary. A publication clock additionally
// detects A -> B -> A while a page spans several reads. No module-level state.
async function snapshot(read: NativeD1Read): Promise<Snapshot> {
  const clocks = await read.query<{epoch:number}>("SELECT CASE WHEN typeof(epoch)='integer' THEN epoch ELSE NULL END AS epoch FROM knowledge_exploration_clock WHERE singleton=1 LIMIT 2");
  const epoch = clocks[0]?.epoch;
  if (clocks.length !== 1 || !Number.isSafeInteger(epoch) || epoch! < 0) nativeUnavailable('invalid exploration publication clock');
  const revision = await read.metadata('data_revision',1024), top = await read.metadata('knowledge_exploration_top',65536);
  const digest=nativeField(revision.ref,'sha256').value, source=nativeField(top.ref,'source_revision').value;
  const authority=nativeField(top.ref,'authority_boundary');
  if (typeof digest !== 'string' || !/^[a-f0-9]{64}$/.test(digest) || typeof source !== 'string' || !/^[a-f0-9]{64}$/.test(source)
      || !authority.value || typeof authority.value !== 'object' || Array.isArray(authority.value)) nativeUnavailable('invalid exploration snapshot metadata');
  const after=await read.query<{epoch:number}>("SELECT CASE WHEN typeof(epoch)='integer' THEN epoch ELSE NULL END AS epoch FROM knowledge_exploration_clock WHERE singleton=1 LIMIT 2");
  if (after.length!==1||after[0]!.epoch!==epoch) conflict();
  return {epoch:epoch!,revision:digest,source_revision:source,authority_boundary:authority};
}

export async function explorationCapabilitiesD1(db: D1Database): Promise<Item> {
  const required = ['knowledge_exploration_clock', 'knowledge_exploration_checkpoints', 'knowledge_exploration_revision_insert',
    'knowledge_exploration_revision_update', 'knowledge_exploration_revision_delete', 'knowledge_relations_from_seek', 'knowledge_relations_to_seek',
    'knowledge_nodes_identity_seek'];
  const count = await db.prepare('SELECT count(*) AS count FROM sqlite_master WHERE name IN (SELECT value FROM json_each(?))')
    .bind(JSON.stringify(required)).first<number>('count');
  let available = count === required.length;
  if (available) {
    const top = await db.prepare("SELECT count(*) AS count FROM edge_meta WHERE key='knowledge_exploration_top'").first<number>('count');
    available = top === 1;
  }
  return {schema: 'tos_exploration_capabilities_v1', available, execution_version: VERSION,
    request_versions: ['tos_exploration_request_v1', REQUEST_V2], result_versions: ['tos_exploration_result_v1', RESULT_V2],
    v2_origin_kinds: ['node', 'relation'], v2_origin_context: {max_nodes: 2, max_relations: 1,
      page_budgets: 'incremental; mandatory origin closure is additional'},
    storage: 'shared-d1-checkpoints', ttl_seconds: TTL / 1000, restart_survival: true, writes_to_tree: false,
    http: {method: 'POST', path: '/api/knowledge/explore'}, max_checkpoints: 128,
    max_checkpoint_bytes: MAX_BYTES, max_cache_bytes: 32 * MAX_BYTES,
    limits: {depth: 10, page_nodes: 100, page_relations: 100, work_per_page: 512,
      adjacency_queries_per_page: ADJACENCY_QUERIES, session_nodes: 10000, session_relations: 20000},
    continuation: 'opaque-cursor-only; fixed query and page sizes',
    ordering: 'zero-distance declared identity carriers before relation-id ordered edges; all profile uses carrier BFS',
    identity_expansion: 'overview only; source-filtered declared tos.* IDs; page and session node budgets apply',
    reason: available ? null : 'apply exploration migration and build compatible read-model metadata'};
}

/** Existing published host limits, shared by the real route and its pure
 * pre-I/O validator. These are software admission, not publication authority. */
export function publishedExplorationAdmission():Uint8Array {
  const limits=nativeD1Limits;
  return new TextEncoder().encode(JSON.stringify({max_open_vm_steps:limits.maxSqlReads,max_read_vm_steps:limits.maxSqlReads,
    max_matches:limits.maxCandidates,max_rows:limits.maxRows,max_field_bytes:1048576,max_payload_bytes:1048576,
    max_decoded_bytes:limits.maxDecodedBytes,max_response_bytes:MAX_BYTES,max_json_bytes:MAX_BYTES,
    max_json_depth:64,max_json_visits:300000,max_integer_digits:4300,max_work_units:512,
    max_session_nodes:10000,max_session_relations:20000,max_state_bytes:MAX_BYTES,
    max_checkpoint_bytes:32*MAX_BYTES,max_checkpoints:128,max_cache_bytes:limits.maxCacheBytes,
    max_cache_entries:limits.maxCacheEntries}));
}
/** Actual published exploration: shared Rust rules, existing verified D1 rows,
 * disposable checkpoint CAS and snapshot-bound final whole-body handoff. */
export async function explorationSnapshotResponseD1(db:D1Database,runtime:PublishedExplorationModule,
  request:Uint8Array,signal?:AbortSignal,method='POST'):Promise<Response> {
  const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true});
  const limits=nativeD1Limits,admission=publishedExplorationAdmission();
  try {
    signal?.throwIfAborted();
    if(typeof runtime?.ExplorationSession!=='function'||typeof runtime.validate_exploration_request_wasm_v1!=='function'
      ||typeof runtime.validate_exploration_replay_wasm_v1!=='function'||typeof runtime.exploration_cache_version_wasm_v1!=='function') {
      throw new SelectedExplorationError('Unavailable');
    }
    let cursor:string|undefined;
    try {cursor=runtime.validate_exploration_request_wasm_v1(request,admission);}catch(error){return explorationError(error);}
    signal?.throwIfAborted();
    if(!(await explorationCapabilitiesD1(db)).available)throw new HttpError(503,'exploration read model is not prepared');
    signal?.throwIfAborted();
    const read=new NativeD1Read(db,limits,true,signal),snap=await snapshot(read),now=Date.now();
    const publication=await readNativeInspectionPublication(read,snap.revision);
    if(nativeField(publication.ref,'source_revision').value!==snap.source_revision
      ||nativePacketJson(nativeField(publication.ref,'authority_boundary'))!==nativePacketJson(snap.authority_boundary)) {
      nativeUnavailable('exploration publication headers disagree');
    }
    const version=runtime.exploration_cache_version_wasm_v1();
    const checkSelected=async():Promise<void>=>{
      signal?.throwIfAborted();const current=await snapshot(read);
      if(current.epoch!==snap.epoch||current.revision!==snap.revision||current.source_revision!==snap.source_revision)conflict();
      signal?.throwIfAborted();
    };
    const validate=(bytes:Uint8Array):void=>{
      try {runtime.validate_exploration_replay_wasm_v1(bytes,admission);}catch(error){return explorationError(error);}
    };
    const replay=(raw:string):Uint8Array=>{
      const bytes=encoder.encode(raw);validate(bytes);
      return bytes;
    };
    let record:Checkpoint|null=null;
    if(cursor!==undefined) {
      record=await checkpoint(db,cursor,now);signal?.throwIfAborted();
      if(!record)expired();
      if(record.epoch!==snap.epoch||record.version!==version)conflict();
      if(record.response!==null)return snapshotPacketResponse(replay(record.response),checkSelected,signal,method);
      if(!record.state)nativeUnavailable('exploration checkpoint state missing');
    }
    let session:PublishedExplorationSession;
    try {session=new runtime.ExplorationSession(request,snap.source_revision,snap.revision,BigInt(snap.epoch),
      encoder.encode(publication.raw),encoder.encode(record?.state??''),admission);}catch(error){return explorationError(error);}
    let packet:Uint8Array,response:string,next:string|null,nextState:string|null;
    try {
      await advanceExploration(session,new PublishedExplorationD1Transport(read),signal);
      signal?.throwIfAborted();next=session.paused()?token():null;
      nextState=next?decoder.decode(session.state()):null;
      packet=next===null?session.finish():session.finish(next);response=decoder.decode(packet);
      signal?.throwIfAborted();
    }catch(error){return explorationError(error);}finally{session.free();}
    validate(packet);
    const admitted=await commitExplorationCheckpoint(db,record,cursor,snap.epoch,version,now,response,next,nextState,checkSelected,signal);
    return snapshotPacketResponse(record?replay(admitted):packet,checkSelected,signal,method);
  }catch(error) {
    signal?.throwIfAborted();
    if(error instanceof HttpError)throw error;
    if(error instanceof NativeBudgetExceeded)throw new HttpError(413,error.message);
    if(error instanceof SelectedExplorationError) {
      const status=error.code==='UnknownIdentifier'?404:error.code==='CursorExpired'?410
        :error.code==='StaleSelection'||error.code==='StaleContinuation'?409:error.code==='BudgetExceeded'?413
        :error.code==='InvalidRequest'||error.code==='InvalidJson'?400:503;
      throw new HttpError(status,error.message);
    }
    return nativeUnavailable('prepared exploration publication or checkpoint is unavailable or invalid');
  }
}
/** Disposable checkpoint custody: epoch-guarded CAS, successor admission and
 * eviction remain a single D1 batch. Rust owns the state and packet content. */
async function commitExplorationCheckpoint(db:D1Database,record:Checkpoint|null,cursor:string|undefined,
  epoch:number,version:string,now:number,response:string,next:string|null,nextState:string|null,
  checkSelected:()=>Promise<void>,signal?:AbortSignal):Promise<string> {
  const expires = record?.expires ?? now + TTL;
  const finished = Date.now();
  if (expires <= finished) expired();
  await checkSelected();
  const statements: D1PreparedStatement[] = [];
  statements.push(db.prepare(`DELETE FROM knowledge_exploration_checkpoints WHERE expires<=?
    AND EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE singleton=1 AND epoch=?)`).bind(finished,epoch));
  if (record) {
    statements.push(db.prepare(`UPDATE knowledge_exploration_checkpoints SET state=NULL,response=?,successor=?,bytes=?
      WHERE token=? AND response IS NULL AND expires>? AND epoch=? AND version=?
      AND EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE epoch=? AND singleton=1)`)
      .bind(response, next, byteSize(response), cursor, finished, epoch, version, epoch));
  }
  if (next) {
    statements.push(db.prepare(`INSERT INTO knowledge_exploration_checkpoints(token,expires,epoch,version,state,bytes)
      SELECT ?,?,?,?,?,? WHERE EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE singleton=1 AND epoch=?)
      ${record ? 'AND EXISTS(SELECT 1 FROM knowledge_exploration_checkpoints WHERE token=? AND successor=?)' : ''}`)
      .bind(next, expires, epoch, version, nextState, byteSize(nextState!), epoch, ...(record ? [cursor, next] : [])));
  }
  // Global bounds are enforced in the same atomic batch as admission. Eviction
  // affects execution cache only, not source or read-model records.
  statements.push(db.prepare(`DELETE FROM knowledge_exploration_checkpoints WHERE token IN (
    SELECT token FROM (SELECT token,ROW_NUMBER() OVER(ORDER BY rowid DESC) AS n,
      SUM(bytes) OVER(ORDER BY rowid DESC) AS total FROM knowledge_exploration_checkpoints)
    WHERE n>128 OR total>33554432)
    AND EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE singleton=1 AND epoch=?)`).bind(epoch));
  statements.push(db.prepare('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1'));
  if (record) statements.push(db.prepare(`SELECT ${RESPONSE_COLUMN} FROM knowledge_exploration_checkpoints WHERE token=?`).bind(cursor));
  signal?.throwIfAborted();
  const committed = await db.batch(statements);
  signal?.throwIfAborted();
  const epochIndex = committed.length - (record ? 2 : 1);
  if ((committed[epochIndex]!.results[0] as {epoch: number} | undefined)?.epoch !== epoch) conflict();
  if (record) {
    const winner = committed.at(-1)!.results[0] as {response: string | null;response_bytes:number|null} | undefined;
    if (winner?.response_bytes && winner.response_bytes > MAX_BYTES) throw new NativeBudgetExceeded('exploration replay exceeds 1 MiB');
    if(winner&&winner.response===null&&winner.response_bytes!==null)nativeUnavailable('invalid exploration winning replay storage type');
    if (!winner?.response) expired();
    return winner.response;
  }
  return response;
}

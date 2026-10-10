import { HttpError, type Item } from "./common.ts";
import { rows } from "./store.ts";
import {NativeBudgetExceeded} from '../../../shared/native-semantics.ts';
import {NativeSearchDelivery, nativeSearchFailure} from './native-search-store.ts';
import {PublishedLensD1Transport,readPublishedLensMetadata} from './native-lens-store.ts';
import {admitAuxiliary,readLensPublicationBinding} from './native-lens-auxiliary.ts';
import {respondLensSnapshot,lensError,SelectedLensError,type PublishedLensModule} from './selected-lens-runtime.ts';
import {InspectionD1Transport, readNativeInspectionPublication} from './native-inspection-store.ts';
import {respondInspectionSnapshot, inspectionError, SelectedInspectionError, type InspectionModule} from './selected-inspection-runtime.ts';
import {respondTemporalSnapshot, SelectedTemporalError, type TemporalPublishedModule} from './selected-temporal-runtime.ts';
import {parseNativeRequest, nativeField,nativePacketJson, type NativeRef, type NativePacket} from './native-lens.ts';
import {NativeD1Read, NativeD1Rows, nativeD1Limits, readNativePublication, nativeSha256, nativeUnavailable,type NativeD1Limits} from './native-d1-read.ts';

export type WorkerSearchControlsRuntime = {
  worker_knowledge_search_controls_wasm_v1(request_json: Uint8Array): Uint8Array;
};
type WorkerSearchPolicy = {
  gram_code_points: number;
  max_candidates: number;
  max_verify_chars: number;
  max_intersection_grams: number;
  cursor_schema: string;
  indexed_schema: string;
  legacy_schema: string;
  rank_classes: number;
  cursor_token_max_chars: number;
  identity_max_bytes: number;
};
type WorkerRankPolicy = {
  sql: string;
  binding_count: number;
  order_by: string;
  candidate_order_by: string;
  window_order_by: string;
  window_reverse_order_by: string;
  continuation: string;
  window_bound: string;
  rank_classes: number;
};
type NormalizedWorkerSearch = {
  query: string;
  needle: string;
  sources: string[];
  kind_ids: string[];
  predicate_ids: string[];
  offset: number;
  limit: number;
  filters: IndexedFilters;
};

function workerSearchControl(runtime: WorkerSearchControlsRuntime | undefined, operation: string, fields: Item = {}): Item {
  if (typeof runtime?.worker_knowledge_search_controls_wasm_v1 !== 'function') {
    return nativeUnavailable('Worker knowledge search Rust controls are not installed');
  }
  let response: Item;
  try {
    const raw=runtime.worker_knowledge_search_controls_wasm_v1(new TextEncoder().encode(JSON.stringify({schema_version:1,operation,...fields})));
    response=parseNativeRequest(new TextDecoder('utf-8',{fatal:true,ignoreBOM:false}).decode(raw)).value as Item;
  } catch {
    return nativeUnavailable('Worker knowledge search Rust controls are unavailable');
  }
  if (response.schema_version!==1 || typeof response.ok!=='boolean') {
    return nativeUnavailable('Worker knowledge search Rust control response is invalid');
  }
  if (response.ok) {
    if (!response.value || typeof response.value!=='object' || Array.isArray(response.value))
      return nativeUnavailable('Worker knowledge search Rust control value is invalid');
    return response.value as Item;
  }
  const error=response.error as Item|undefined;
  const code=typeof error?.code==='string'?error.code:'invalid_request';
  const message=typeof error?.message==='string'?error.message:'Worker knowledge search request is invalid';
  const status=code==='cursor_stale'?409:code==='budget_exceeded'?413:code==='unavailable'?503:400;
  throw new HttpError(status,message);
}

function workerSearchPolicy(runtime: WorkerSearchControlsRuntime | undefined): WorkerSearchPolicy {
  return workerSearchControl(runtime,'policy') as unknown as WorkerSearchPolicy;
}

function normalizeWorkerSearch(runtime: WorkerSearchControlsRuntime | undefined, mode:'legacy'|'indexed', options: {
  query:string; sources:string[]|null; kindIds:string[]; predicateIds:string[]; offset:number; limit:number;
}): NormalizedWorkerSearch {
  return workerSearchControl(runtime,'normalize',{
    mode,query:options.query,sources:options.sources,kind_ids:options.kindIds,
    predicate_ids:options.predicateIds,offset:options.offset,limit:options.limit,
  }) as unknown as NormalizedWorkerSearch;
}

// A data_revision digest is not a publication identity: an import can move
// A -> B -> A while retaining the same bytes at the end.  The additive
// exploration clock is advanced by the maintenance publication triggers (and
// by the builder's identical-data bootstrap), so the pair below is the
// request's read-model identity.  Keep the metadata read as one SQL statement
// so a guard never combines a clock from one D1 read with a revision from
// another read.
// Published data_revision is a small sha256 envelope. Reject oversized/nontext
// chunks in SQL before GROUP_CONCAT; typed diagnostics cannot deliver raw text.
const KNOWLEDGE_SNAPSHOT_SQL = `
SELECT
  (SELECT COUNT(*) FROM knowledge_exploration_clock) AS clock_rows,
  (SELECT COUNT(*) FROM knowledge_exploration_clock WHERE singleton = 1) AS singleton_rows,
  (SELECT MIN(CASE WHEN typeof(epoch)='integer' THEN epoch ELSE NULL END) FROM knowledge_exploration_clock WHERE singleton = 1) AS epoch_min,
  (SELECT MAX(CASE WHEN typeof(epoch)='integer' THEN epoch ELSE NULL END) FROM knowledge_exploration_clock WHERE singleton = 1) AS epoch_max,
  CASE WHEN NOT EXISTS(SELECT 1 FROM edge_meta WHERE key='data_revision' AND typeof(json_chunk)!='text')
    AND (SELECT coalesce(sum(length(CAST(json_chunk AS BLOB))),0) FROM edge_meta WHERE key='data_revision')<=1024
    THEN (SELECT GROUP_CONCAT(json_chunk, '') FROM (
      SELECT json_chunk FROM edge_meta WHERE key = 'data_revision' ORDER BY part
    )) ELSE NULL END AS data_revision_json,
  (SELECT COUNT(*) FROM edge_meta WHERE key = 'data_revision') AS data_revision_parts,
  (SELECT COUNT(DISTINCT part) FROM edge_meta WHERE key = 'data_revision') AS data_revision_distinct_parts,
  (SELECT MIN(CASE WHEN typeof(part)='integer' THEN part ELSE NULL END) FROM edge_meta WHERE key = 'data_revision') AS data_revision_min_part,
  (SELECT MAX(CASE WHEN typeof(part)='integer' THEN part ELSE NULL END) FROM edge_meta WHERE key = 'data_revision') AS data_revision_max_part,
  (SELECT COUNT(*) FROM edge_meta WHERE key = 'data_revision' AND typeof(part) != 'integer') AS data_revision_non_integer_parts,
  (SELECT COUNT(*) FROM edge_meta WHERE key = 'data_revision' AND typeof(json_chunk) != 'text') AS data_revision_non_text_chunks
`;

type KnowledgeSnapshot = { epoch: number; revision: string };
type KnowledgeSnapshotRow = {
  clock_rows: unknown;
  singleton_rows: unknown;
  epoch_min: unknown;
  epoch_max: unknown;
  data_revision_json: unknown;
  data_revision_parts: unknown;
  data_revision_distinct_parts: unknown;
  data_revision_min_part: unknown;
  data_revision_max_part: unknown;
  data_revision_non_integer_parts: unknown;
  data_revision_non_text_chunks: unknown;
};

function snapshotCount(value: unknown): number | null {
  return Number.isSafeInteger(value) && Number(value) >= 0 ? Number(value) : null;
}

function invalidSnapshot(): never {
  throw new HttpError(503, "knowledge read model publication clock or data revision is unavailable");
}

async function knowledgeSnapshot(db: D1Database): Promise<KnowledgeSnapshot> {
  let row: KnowledgeSnapshotRow | null;
  try {
    row = await db.prepare(KNOWLEDGE_SNAPSHOT_SQL).first<KnowledgeSnapshotRow>();
  } catch {
    // Missing migration/table and malformed SQL-level metadata are readiness
    // failures, not successful reads and not generic Worker 500s.
    invalidSnapshot();
  }
  if (!row) invalidSnapshot();
  const clockRows = snapshotCount(row.clock_rows);
  const singletonRows = snapshotCount(row.singleton_rows);
  const parts = snapshotCount(row.data_revision_parts);
  const distinctParts = snapshotCount(row.data_revision_distinct_parts);
  const minPart = snapshotCount(row.data_revision_min_part);
  const maxPart = snapshotCount(row.data_revision_max_part);
  const nonIntegerParts = snapshotCount(row.data_revision_non_integer_parts);
  const nonTextChunks = snapshotCount(row.data_revision_non_text_chunks);
  const epoch = row.epoch_min;
  if (
    clockRows !== 1
    || singletonRows !== 1
    || !Number.isSafeInteger(epoch)
    || Number(epoch) < 0
    || row.epoch_max !== epoch
    || parts === null
    || parts < 1
    || distinctParts !== parts
    || minPart !== 0
    || maxPart !== parts - 1
    || nonIntegerParts !== 0
    || nonTextChunks !== 0
    || typeof row.data_revision_json !== "string"
  ) invalidSnapshot();
  let revision: unknown;
  try {
    revision = JSON.parse(row.data_revision_json);
  } catch {
    invalidSnapshot();
  }
  if (!revision || typeof revision !== "object" || Array.isArray(revision)
      || typeof (revision as Item).sha256 !== "string" || !(revision as Item).sha256) invalidSnapshot();
  return { epoch: Number(epoch), revision: (revision as Item).sha256 as string };
}

function sameKnowledgeSnapshot(left: KnowledgeSnapshot, right: KnowledgeSnapshot): boolean {
  return left.epoch === right.epoch && left.revision === right.revision;
}

export async function consistentRead<T>(
  db: D1Database,
  read: (snapshot: KnowledgeSnapshot) => Promise<T>,
): Promise<T> {
  const before = await knowledgeSnapshot(db);
  let result: T;
  try { result = await read(before); }
  catch (error) {
    const after = await knowledgeSnapshot(db);
    if (!sameKnowledgeSnapshot(before, after)) {
      throw new HttpError(409, "knowledge snapshot changed during query; retry against the current revision");
    }
    throw error;
  }
  const after = await knowledgeSnapshot(db);
  if (!sameKnowledgeSnapshot(before, after)) {
    throw new HttpError(409, "knowledge snapshot changed during query; retry against the current revision");
  }
  return result;
}

/** Whole published lens/focus/stored path. Rust owns normalization, selectors,
 * fast-path eligibility, traversal, presentation and packet construction. */
export async function lensSnapshotResponseD1(db:D1Database,runtime:PublishedLensModule,
  request:Uint8Array,operation:'compile'|'focus'|'stored',signal?:AbortSignal,method='GET',
  overrides:Partial<NativeD1Limits>={}):Promise<Response> {
  const limits={...nativeD1Limits,...overrides};
  if(Object.values(limits).some(value=>!Number.isSafeInteger(value)||value<1)||limits.blockSize>64)throw new Error('invalid native lens budgets');
  const encoder=new TextEncoder();
  const admission=encoder.encode(JSON.stringify({max_open_vm_steps:limits.maxSqlReads,
    max_read_vm_steps:limits.maxSqlReads,max_matches:limits.maxCandidates,max_rows:limits.maxRows,
    max_field_bytes:1048576,max_payload_bytes:1048576,max_decoded_bytes:limits.maxDecodedBytes,
    max_response_bytes:16*1048576,max_json_bytes:16*1048576,max_json_depth:64,
    max_json_visits:300000,max_integer_digits:4300,max_candidates:limits.maxCandidates,
    max_path_steps:limits.maxPathSteps,max_adjacency_rows:limits.maxSqlReads,block_size:limits.blockSize,
    max_callbacks:limits.maxCallbacks,max_sort_bytes:limits.maxSortBytes,
    max_cache_bytes:limits.maxCacheBytes,max_cache_entries:limits.maxCacheEntries}));
  try {
    signal?.throwIfAborted();
    if(typeof runtime?.LensSession!=='function'||typeof runtime.validate_lens_request_wasm_v1!=='function')throw new SelectedLensError('selected_runtime_unavailable');
    try {runtime.validate_lens_request_wasm_v1(request,operation,admission);}catch(error){lensError(error);}
    signal?.throwIfAborted();
    const read=new NativeD1Read(db,limits,true,signal);
    const publication=await consistentRead(db,async snapshot=>{
      const top=await readNativePublication(read,snapshot.revision);
      const metadata=await readPublishedLensMetadata(read,top);
      // Catalog custody remains the existing separate bounded reader. Rust
      // receives its exact bytes and selects the unique stored specification.
      const catalog=operation==='stored'?(await publishedCatalogDocument(db,snapshot.revision,signal)).raw:'';
      const binding=nativePacketJson(await readLensPublicationBinding(read,top,snapshot.epoch));
      return {snapshot,top,metadata,catalog,binding};
    });
    let verifyAuxiliary=async():Promise<void>=>{};
    const checkSelected=async():Promise<void>=>{
      signal?.throwIfAborted();
      const current=await knowledgeSnapshot(db);
      if(!sameKnowledgeSnapshot(current,publication.snapshot))throw new HttpError(409,'knowledge snapshot changed during query; retry against the current revision');
      await verifyAuxiliary();signal?.throwIfAborted();
    };
    const physical=new PublishedLensD1Transport(read);
    const selected={sourceRevision:nativeField(publication.top.ref,'source_revision').value as string,
      top:encoder.encode(publication.top.raw),metadata:encoder.encode(publication.metadata.raw),
      catalog:encoder.encode(publication.catalog),publication:encoder.encode(publication.binding),admission,checkSelected,
      payloads:physical.payloads.bind(physical),candidates:physical.candidates.bind(physical),focus:physical.focus.bind(physical),
      incident:physical.incident.bind(physical),ordered:physical.ordered.bind(physical),aliases:physical.aliases.bind(physical),
      sources:physical.sources.bind(physical),count:physical.count.bind(physical),
      async auxiliary(need:{compact:boolean;membership:boolean}):Promise<{compact:boolean;membership:boolean}> {
        const requested=[...(need.compact?['compact' as const]:[]),...(need.membership?['membership' as const]:[])];
        const auxiliary=await admitAuxiliary(read,publication.top,requested);
        const stores={compact:auxiliary.installed.includes('compact'),membership:auxiliary.installed.includes('membership')};
        verifyAuxiliary=auxiliary.verify;physical.setStores(stores);return stores;
      },
    };
    try {return await respondLensSnapshot(runtime,selected,request,operation,signal,method);}
    catch(error){await checkSelected();throw error;}
  }catch(error){
    signal?.throwIfAborted();
    if(error instanceof HttpError)throw error;
    if(error instanceof NativeBudgetExceeded)throw new HttpError(413,error.message);
    if(error instanceof SelectedLensError){const status=error.code==='UnknownIdentifier'?(operation==='stored'?404:400)
      :error.code==='StaleSelection'?409:error.code==='BudgetExceeded'?413
      :error.code==='InvalidRequest'||error.code==='InvalidJson'?400:503;throw new HttpError(status,error.message);}
    throw error;
  }
}

/** The publisher/import selects the public snapshot. This reader validates
 * publication framing and retained exact rows; it issues no runtime rights. */
export async function temporalSnapshotResponseD1(db: D1Database, runtime: TemporalPublishedModule,
  request: Uint8Array, signal?: AbortSignal): Promise<Response> {
  if (!runtime || typeof runtime.TemporalReplaySession !== 'function'
      || typeof runtime.validate_temporal_request_wasm_v1 !== 'function') {
    throw new SelectedTemporalError('selected_runtime_unavailable');
  }
  signal?.throwIfAborted();
  const encoder = new TextEncoder();
  const admission = encoder.encode(JSON.stringify({max_json_bytes:1048576, max_json_depth:64,
    max_json_visits:300000, max_integer_digits:4300, max_source_bytes:6*1048576,
    max_replay_bytes:7*(65536+6*1048576), max_output_bytes:16*1048576}));
  // Same shared request validator and parser admission as comparison. Invalid
  // shapes retain 400 before even publication metadata on an unavailable DB.
  try { runtime.validate_temporal_request_wasm_v1(request, admission); }
  catch (error) {
    if (typeof error === 'string') throw new SelectedTemporalError(error);
    throw error;
  }
  signal?.throwIfAborted();
  const read = new NativeD1Read(db, nativeD1Limits, true);
  const publication = await consistentRead(db, async snapshot => {
    const top = await readNativeInspectionPublication(read, snapshot.revision);
    return {snapshot, top};
  });
  const checkSelected = async (): Promise<void> => {
    signal?.throwIfAborted();
    const current = await knowledgeSnapshot(db);
    if (!sameKnowledgeSnapshot(current, publication.snapshot)) {
      throw new HttpError(409, 'knowledge snapshot changed during query; retry against the current revision');
    }
    signal?.throwIfAborted();
  };
  const rows = new NativeD1Rows(read, nativeD1Limits, false);
  const selected = {
    sourceRevision: nativeField(publication.top.ref, 'source_revision').value as string,
    // The published normalized source-claims contract owns this profile name;
    // no native selected receipt or runtime grant is inferred from the header.
    claimSourceGraph: 'source-claims',
    admission,
    checkSelected,
    async readExactNode(id: string): Promise<Uint8Array | null> {
      await checkSelected();
      if (!id.isWellFormed()) nativeUnavailable('prepared response contains invalid JSON values');
      const found = await read.textRows<{id:string}>(['id'], ['id'],
        'SELECT id FROM knowledge_nodes WHERE id=? ORDER BY id LIMIT 2', id);
      if (found.length > 1) throw new NativeBudgetExceeded('prepared temporal has too many exact identity matches');
      const raw = found.length ? encoder.encode(await rows.getRaw('node', id)) : null;
      await checkSelected();
      return raw;
    },
  };
  return respondTemporalSnapshot(runtime, selected, request, signal);
}

async function publishedCatalogDocument(db: D1Database, revision: string, signal?:AbortSignal): Promise<{raw:string;ref:NativeRef}> {
  const read=new NativeD1Read(db,nativeD1Limits,true,signal);
  const top=await readNativePublication(read,revision,'inspection');
  const catalog=await read.metadata('knowledge_catalog',8*1024*1024);
  if (await nativeSha256(catalog.raw)!==nativeField(top.ref,'catalog_sha256').value
      || nativeField(catalog.ref,'schema').value!=='tos_knowledge_catalog_v1'
      || nativeField(catalog.ref,'source_revision').value!==nativeField(top.ref,'source_revision').value) {
    nativeUnavailable('published knowledge catalog differs from its reader binding');
  }
  return catalog;
}
async function publishedCatalog(db:D1Database,revision:string):Promise<NativeRef> {
  return (await publishedCatalogDocument(db,revision)).ref;
}

export async function knowledgeCatalogD1(db: D1Database): Promise<NativeRef> {
  return consistentRead(db,snapshot=>publishedCatalog(db,snapshot.revision));
}

export async function knowledgeSearchD1(db: D1Database, options: Parameters<typeof knowledgeSearchD1Unchecked>[1],
  runtime?:WorkerSearchControlsRuntime): Promise<NativePacket> {
  return nativeSearchFailure(()=>consistentRead(db, snapshot => knowledgeSearchD1Unchecked(db, options, snapshot,runtime)));
}

export async function knowledgeSearchCapabilitiesD1(db: D1Database, runtime?:WorkerSearchControlsRuntime): Promise<Item> {
  return nativeSearchFailure(() => consistentRead(db, async snapshot => {
    const policy=workerSearchPolicy(runtime);
    const delivery = await NativeSearchDelivery.open(db, snapshot.revision);
    // Inspect the published reader and its search schema, never corpus rows.
    // Readiness is not completeness: actual queries still verify selected
    // carriers/payloads and enforce their independent work budgets.
    await delivery.read.query('SELECT kind,position,id,source_graph,kind_id,predicate_id,document_digest FROM knowledge_search_documents LIMIT 0');
    await delivery.read.query('SELECT kind,n,gram,position FROM knowledge_search_grams LIMIT 0');
    await delivery.read.query('SELECT kind,n,gram,postings FROM knowledge_search_gram_stats LIMIT 0');
    await delivery.read.query('SELECT id,source_graph,kind_id,search_text,json FROM knowledge_nodes LIMIT 0');
    await delivery.read.query('SELECT id,source_graph,predicate_id,search_text,json FROM knowledge_relations LIMIT 0');
    return {
      schema: 'tos_knowledge_search_capabilities_v1',
      default_mode: 'legacy',
      explicit_mode_required: false,
      writes_to_tree: false,
      modes: {
        legacy: {available: true, schema: policy.legacy_schema, verification: 'engine-selection-only', pagination: 'offset'},
        indexed: {available: true, schema: policy.indexed_schema, source_revision: nativeField(delivery.top, 'source_revision').value, verification: 'engine-selection-only', pagination: 'cursor', min_normalized_query_code_points: policy.gram_code_points},
        compressed: {available: false, schema: 'tos_knowledge_search_compressed_v3', reason: 'not-supported-by-d1-adapter', writes_to_tree: false},
      },
    };
  }));
}

export async function knowledgeSearchD1Indexed(
  db: D1Database,
  options: Parameters<typeof knowledgeSearchD1IndexedUnchecked>[1],
  runtime?:WorkerSearchControlsRuntime,
): Promise<NativePacket> {
  return nativeSearchFailure(()=>consistentRead(db, (snapshot) => knowledgeSearchD1IndexedUnchecked(db, options, snapshot,runtime)));
}

/** Full node/relation inspection of one publisher-selected public snapshot.
 * Rust owns request shape, alias precedence/completeness and packet semantics;
 * this adapter owns existing D1 integrity/physical admission and body handoff. */
export async function inspectionSnapshotResponseD1(db:D1Database,runtime:InspectionModule,
  kind:'node'|'relation',identifier:string,relationLimit:number,signal?:AbortSignal,method='GET'):Promise<Response> {
  const encoder=new TextEncoder();
  const admission=encoder.encode(JSON.stringify({max_open_vm_steps:nativeD1Limits.maxSqlReads,
    max_read_vm_steps:nativeD1Limits.maxSqlReads,max_matches:128,max_rows:nativeD1Limits.maxRows,
    max_field_bytes:1048576,max_payload_bytes:1048576,max_decoded_bytes:nativeD1Limits.maxDecodedBytes,
    max_response_bytes:16*1048576,max_json_bytes:16*1048576,max_json_depth:64,
    max_json_visits:300000,max_integer_digits:4300}));
  const request=encoder.encode(JSON.stringify({kind,identifier,relation_limit:relationLimit}));
  try {
    signal?.throwIfAborted();
    if(typeof runtime?.InspectionSession!=='function'||typeof runtime.validate_inspect_request_wasm_v1!=='function') {
      throw new SelectedInspectionError('selected_runtime_unavailable');
    }
    // Preserve malformed request refusal before even snapshot metadata I/O.
    try {runtime.validate_inspect_request_wasm_v1(request,admission);}catch(error){inspectionError(error);}
    signal?.throwIfAborted();
    const read=new NativeD1Read(db,nativeD1Limits,true,signal);
    const publication=await consistentRead(db,async snapshot=>({snapshot,
      top:await readNativeInspectionPublication(read,snapshot.revision)}));
    const checkSelected=async():Promise<void>=>{
      signal?.throwIfAborted();
      const current=await knowledgeSnapshot(db);
      if(!sameKnowledgeSnapshot(current,publication.snapshot)) {
        throw new HttpError(409,'knowledge snapshot changed during query; retry against the current revision');
      }
      signal?.throwIfAborted();
    };
    const physical=new InspectionD1Transport(read);
    const selected={sourceRevision:nativeField(publication.top.ref,'source_revision').value as string,
      top:encoder.encode(publication.top.raw),admission,checkSelected,
      lookup:physical.lookup.bind(physical),incident:physical.incident.bind(physical),endpoints:physical.endpoints.bind(physical)};
    try {return await respondInspectionSnapshot(runtime,selected,request,signal,method);}
    catch(error){await checkSelected();throw error;}
  } catch(error) {
    signal?.throwIfAborted();
    if(error instanceof HttpError)throw error;
    if(error instanceof NativeBudgetExceeded)throw new HttpError(413,error.message);
    if(error instanceof SelectedInspectionError) {
      const status=error.code==='UnknownIdentifier'?404:error.code==='StaleSelection'?409:
        error.code==='BudgetExceeded'?413:error.code==='InvalidRequest'||error.code==='InvalidJson'?400:503;
      throw new HttpError(status,error.message);
    }
    return nativeUnavailable('prepared inspection publication unavailable or invalid');
  }
}

type SqlFragment = { sql: string; bindings: unknown[] };
type CountRow = { count: number };


function sourceFragment(alias: string, sources: string[]): SqlFragment {
  return {
    sql: `${alias}.source_graph IN (SELECT value FROM json_each(?))`,
    bindings: [JSON.stringify(sources)],
  };
}


function joinFragments(parts: SqlFragment[]): SqlFragment {
  return {
    sql: parts.map((item) => `(${item.sql})`).join(" AND "),
    bindings: parts.flatMap((item) => item.bindings),
  };
}

// Bounded, correlated joins over the existing adjacency indexes. All values
// are bound; aliases and SQL operators are generated only by this compiler.

async function count(db: D1Database, table: string, where: SqlFragment): Promise<number> {
  const result = await rows<CountRow>(db, `SELECT COUNT(*) AS count FROM ${table} WHERE ${where.sql}`, ...where.bindings);
  return Number(result[0]?.count ?? 0);
}

function indexedCursorEncode(runtime:WorkerSearchControlsRuntime|undefined,policy:WorkerSearchPolicy,operation:'encode_cursor_kind'|'encode_cursor_outer',value:Item): string {
  if(typeof value.id==='string'&&new TextEncoder().encode(value.id).length>policy.cursor_token_max_chars)
    throw new NativeBudgetExceeded('indexed knowledge search cursor byte budget');
  const result=workerSearchControl(runtime,operation,value);
  if(typeof result.cursor_json!=='string')return nativeUnavailable('Worker knowledge search cursor encoding is invalid');
  const bytes = new TextEncoder().encode(result.cursor_json);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  const token=btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
  if(token.length>policy.cursor_token_max_chars)throw new NativeBudgetExceeded('indexed knowledge search cursor byte budget');
  return token;
}

function indexedCursorRaw(value: string,maxChars:number): string {
  if (!value || value.length > maxChars) throw new HttpError(400, "invalid indexed knowledge search cursor");
  try {
    const padded = value.replaceAll("-", "+").replaceAll("_", "/") + "=".repeat((4 - (value.length % 4)) % 4);
    const binary = atob(padded);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    return new TextDecoder('utf-8',{fatal:true,ignoreBOM:false}).decode(bytes);
  } catch(error) {
    if(error instanceof HttpError)throw error;
    throw new HttpError(400, "invalid indexed knowledge search cursor");
  }
}

function indexedRankPolicy(runtime:WorkerSearchControlsRuntime|undefined,hasNeedle=true):WorkerRankPolicy {
  const policy=workerSearchControl(runtime,'rank_sql',{alias:'s',has_needle:hasNeedle});
  if(typeof policy.sql!=='string'||typeof policy.order_by!=='string'||typeof policy.candidate_order_by!=='string'
      ||typeof policy.window_order_by!=='string'||typeof policy.window_reverse_order_by!=='string'
      ||typeof policy.continuation!=='string'||typeof policy.window_bound!=='string'
      ||!Number.isSafeInteger(policy.binding_count)||Number(policy.binding_count)<0
      ||!Number.isSafeInteger(policy.rank_classes)||Number(policy.rank_classes)<1)
    return nativeUnavailable('Worker knowledge search rank policy is invalid');
  return policy as unknown as WorkerRankPolicy;
}

function indexedRankBindings(needle:string,count:number):string[] {
  return Array.from({length:count},()=>needle);
}

function indexedQueryGrams(runtime:WorkerSearchControlsRuntime|undefined,needle:string):string[] {
  const value=workerSearchControl(runtime,'grams',{query:needle});
  if(!Array.isArray(value.grams)||!value.grams.every((gram)=>typeof gram==='string'))
    return nativeUnavailable('Worker knowledge search gram policy is invalid');
  return value.grams as string[];
}

type IndexedSearchOptions = {
  query: string;
  sources: string[] | null;
  kindIds: string[];
  predicateIds: string[];
  cursor?: string | null;
  limit: number;
};

type IndexedPage = {
  rows: NativeRef[];
  nextCursor: string | null;
  hasMore: boolean;
  work: { candidate_rows: number; verified_chars: number; rank_chars?: number; sql_pages: number; selection_rows_read?: number };
};

type IndexedFilters = {
  sources: string[];
  kind_ids: string[];
  predicate_ids: string[];
};

function indexedRowsRead(value: unknown): number | undefined {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : undefined;
}

function indexedWork(runtime:WorkerSearchControlsRuntime|undefined,fields:Item):IndexedPage['work'] {
  return workerSearchControl(runtime,'indexed_work',fields) as unknown as IndexedPage['work'];
}

async function indexedKindPage(
  db: D1Database,
  options: IndexedSearchOptions,
  kind: "nodes" | "relations",
  cursor: string | null,
  sourceRevision: string,
  snapshotEpoch: number,
  delivery: NativeSearchDelivery,
  runtime:WorkerSearchControlsRuntime|undefined,
  policy:WorkerSearchPolicy,
  needle:string,
  filters:IndexedFilters,
): Promise<IndexedPage> {
  let cursorRank = -1;
  let cursorId = "";
  let cursorPosition = -1;
  const rankPolicy=indexedRankPolicy(runtime);
  if (cursor) {
    const decoded=workerSearchControl(runtime,'cursor_kind',{
      raw:indexedCursorRaw(cursor,policy.cursor_token_max_chars),source_revision:sourceRevision,snapshot_epoch:snapshotEpoch,
      kind,query:needle,filters,
    });
    cursorRank=decoded.rank as number;
    cursorId=decoded.id as string;
    cursorPosition=decoded.position as number;
  }
  const grams = indexedQueryGrams(runtime,needle);
  const stats = await Promise.all(grams.map(async (gram) => {
    const row = await db.prepare(
      "SELECT CASE WHEN typeof(postings)='integer' THEN postings ELSE NULL END AS postings FROM knowledge_search_gram_stats WHERE kind=? AND n=? AND gram=?",
    ).bind(kind, policy.gram_code_points, gram).first<{ postings: number }>();
    const postings=row===null?0:row.postings;
    return { gram, postings };
  }));
  const plan=workerSearchControl(runtime,'gram_plan',{stats});
  const selected=plan.selected as {gram:string;postings:number}|undefined;
  const postingSelections=plan.selections as Array<{gram:string;postings:number}>|undefined;
  if(!selected||!Array.isArray(postingSelections)||!postingSelections.length
      ||typeof selected.gram!=='string'||typeof selected.postings!=='number'
      ||postingSelections.some(item=>!item||typeof item.gram!=='string'||typeof item.postings!=='number'))
    return nativeUnavailable('Worker knowledge search gram plan is invalid');
  // Validate every posting used for exclusion, including the zero-stat path.
  // A missing secondary posting must not silently remove a genuine match.
  for (const selection of postingSelections) {
    const posting=await db.prepare(`SELECT count(*) AS total,coalesce(sum(CASE WHEN document_position IS NULL THEN 1 ELSE 0 END),0) AS missing
    FROM (SELECT s.position AS document_position FROM knowledge_search_grams g
      LEFT JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
      WHERE g.kind=? AND g.n=? AND g.gram=? LIMIT ?)`).bind(kind,policy.gram_code_points,selection.gram,selection.postings+1)
    .first<{total:number;missing:number}>();
    if(!posting)throw new HttpError(503,'indexed knowledge search posting metadata is invalid');
    workerSearchControl(runtime,'posting_closure',{expected:selection.postings,total:posting.total,missing:posting.missing});
  }
  if(selected.postings===0)return {rows:[],nextCursor:null,hasMore:false,work:workerSearchControl(runtime,'indexed_work',{
    phase:'zero_postings',grams:grams.length,selections:postingSelections.length,
  }) as IndexedPage['work']};
  const rankExpression = rankPolicy.sql;
  const rankBindings=indexedRankBindings(needle,rankPolicy.binding_count);
  const baseTable = kind === "nodes" ? "knowledge_nodes" : "knowledge_relations";
  const filterSql: string[] = ["g.kind = ?", "g.n = ?", "g.gram = ?"];
  const filterBindings: unknown[] = [kind, policy.gram_code_points, selected.gram];
  for (const selection of postingSelections.slice(1)) {
    filterSql.push(`EXISTS (SELECT 1 FROM knowledge_search_grams intersection
      WHERE intersection.kind=g.kind AND intersection.n=g.n
        AND intersection.gram=? AND intersection.position=g.position)`);
    filterBindings.push(selection.gram);
  }
  if (filters.sources.length) {
    filterSql.push(`s.source_graph IN (SELECT value FROM json_each(?))`);
    filterBindings.push(JSON.stringify(filters.sources));
  }
  if (kind === "nodes" && filters.kind_ids.length) {
    filterSql.push(`s.kind_id IN (SELECT value FROM json_each(?))`);
    filterBindings.push(JSON.stringify(filters.kind_ids));
  }
  if (kind === "relations" && filters.predicate_ids.length) {
    filterSql.push(`s.predicate_id IN (SELECT value FROM json_each(?))`);
    filterBindings.push(JSON.stringify(filters.predicate_ids));
  }
  const continuationSql = cursor
    ? ` AND ${rankPolicy.continuation}`
    : "";
  const continuationBindings = cursor
    ? [...rankBindings, cursorRank, ...rankBindings, cursorRank, cursorId, cursorId, cursorPosition]
    : [];
  const preflightSql = `SELECT COUNT(*) AS candidate_rows, COALESCE(SUM(s.document_chars), 0) AS verified_chars,
    COALESCE(SUM(length(s.id)+length(s.id_lower)+length(s.native_id_lower)+length(s.identity_values)+length(s.visible_values)),0) AS rank_chars,
    COALESCE(SUM(CASE WHEN typeof(s.id)='text' AND typeof(s.id_lower)='text' AND typeof(s.native_id_lower)='text'
      AND typeof(s.identity_values)='text' AND typeof(s.visible_values)='text' THEN 0 ELSE 1 END),0) AS invalid_rank_metadata,
    COALESCE(SUM(CASE WHEN typeof(s.document_chars)='integer' AND s.document_chars>=0 THEN 0 ELSE 1 END),0) AS invalid_budgets
    FROM knowledge_search_grams g
    CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
    WHERE ${filterSql.join(" AND ")}`;
  let preflight;
  try {
    // This is deliberately a carrier-only budget gate. Do not evaluate the
    // rank expression/continuation (which walks JSON rank fields) until its
    // metadata budget is known to be bounded. Text is verified in a prefix.
    preflight = await db.prepare(preflightSql).bind(...filterBindings)
      .all<{ candidate_rows: number; verified_chars: number; rank_chars:number; invalid_budgets:number; invalid_rank_metadata:number }>();
  } catch (error) {
    throw new HttpError(503, `indexed knowledge search read model is unavailable: ${String(error)}`);
  }
  const aggregate = preflight.results[0];
  const preflightDecision=workerSearchControl(runtime,'preflight',{
    candidate_rows:aggregate?.candidate_rows??null,
    verified_chars:aggregate?.verified_chars??null,
    rank_chars:aggregate?.rank_chars??null,
    invalid_budgets:aggregate?.invalid_budgets??null,
    invalid_rank_metadata:aggregate?.invalid_rank_metadata??null,
  });
  let candidateRows = preflightDecision.candidate_rows as number;
  let verifiedChars = preflightDecision.verified_chars as number;
  const rankChars = preflightDecision.rank_chars as number;
  const preflightOutcome=preflightDecision.outcome as 'empty'|'full'|'window';
  const preflightRowsRead = indexedRowsRead(preflight.meta?.rows_read);
  if (preflightOutcome==='empty') {
    return {
      rows: [],
      nextCursor: null,
      hasMore: false,
      work: indexedWork(runtime,{phase:'preflight_empty',grams:grams.length,selections:postingSelections.length,
        rank_chars:rankChars,preflight_rows_read:preflightRowsRead??null}),
    };
  }
  // A numeric/metadata-only ordered window bounds text IO before joining the
  // native carriers. Its last verified candidate is a progress cursor even
  // when no candidate in this window contains the complete query string.
  type VerificationWindow = {id:string;id_lower:string;position:number;search_rank:number;prefix_chars:number;prefix_rows:number;remaining_rows:number;has_more:boolean};
  let windowLast: VerificationWindow|null = null;
  let windowHasMore = false;
  let windowRowsRead: number|undefined = 0;
  if (preflightOutcome==='window') {
    const windowSql = `WITH ranked AS MATERIALIZED (
      SELECT s.id,s.id_lower,s.position,s.document_chars,${rankExpression} AS search_rank
      FROM knowledge_search_grams g CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
      WHERE ${filterSql.join(' AND ')}${continuationSql}
    ), bounded_window AS (
      SELECT *,SUM(document_chars) OVER (ORDER BY ${rankPolicy.window_order_by} ROWS UNBOUNDED PRECEDING) AS prefix_chars,
        ROW_NUMBER() OVER (ORDER BY ${rankPolicy.window_order_by}) AS prefix_rows FROM ranked
    ) SELECT CASE WHEN length(CAST(last.id AS BLOB))<=${policy.identity_max_bytes} THEN last.id ELSE NULL END AS id,
      CASE WHEN length(CAST(last.id_lower AS BLOB))<=${policy.identity_max_bytes} THEN last.id_lower ELSE NULL END AS id_lower,
      last.position,last.search_rank,last.prefix_chars,last.prefix_rows,total.remaining_rows
      FROM (SELECT COUNT(*) AS remaining_rows FROM ranked) total
      LEFT JOIN (SELECT id,id_lower,position,search_rank,prefix_chars,prefix_rows FROM bounded_window
        WHERE prefix_chars<=? ORDER BY ${rankPolicy.window_reverse_order_by} LIMIT 1) last ON 1=1`;
    const window = await db.prepare(windowSql).bind(...rankBindings,...filterBindings,
      ...continuationBindings,policy.max_verify_chars-rankChars).all<VerificationWindow>();
    const rawWindow=window.results[0] ?? null;
    windowRowsRead = indexedRowsRead(window.meta?.rows_read);
    const windowDecision=workerSearchControl(runtime,'window',{
      id_bytes:typeof rawWindow?.id==='string'?new TextEncoder().encode(rawWindow.id).length:null,
      id_lower_bytes:typeof rawWindow?.id_lower==='string'?new TextEncoder().encode(rawWindow.id_lower).length:null,
      position:rawWindow?.position??null,
      search_rank:rawWindow?.search_rank??null,prefix_chars:rawWindow?.prefix_chars??null,
      prefix_rows:rawWindow?.prefix_rows??null,remaining_rows:rawWindow?.remaining_rows??null,
    });
    if(windowDecision.outcome==='empty')return {rows:[],nextCursor:null,hasMore:false,work:indexedWork(runtime,{
      phase:'window_empty',grams:grams.length,selections:postingSelections.length,rank_chars:rankChars,
      preflight_rows_read:preflightRowsRead??null,window_rows_read:windowRowsRead??null,
    })};
    windowLast={...windowDecision,id:rawWindow?.id,id_lower:rawWindow?.id_lower} as unknown as VerificationWindow;
    workerSearchControl(runtime,'validate_selected',{position:windowLast.position,search_rank:windowLast.search_rank,
      id:windowLast.id,id_lower:windowLast.id_lower,context:'window'});
    if(typeof windowLast.id!=='string'||typeof windowLast.id_lower!=='string')
      throw new HttpError(503,'indexed knowledge search verification window is invalid');
    candidateRows=windowLast.prefix_rows;
    verifiedChars=windowLast.prefix_chars;
    windowHasMore=windowLast.has_more;
  }
  let sql = `SELECT CASE WHEN typeof(s.id)='text' AND length(CAST(s.id AS BLOB))<=${policy.identity_max_bytes} THEN s.id ELSE NULL END AS id,
    CASE WHEN typeof(s.id_lower)='text' AND length(CAST(s.id_lower AS BLOB))<=${policy.identity_max_bytes} THEN s.id_lower ELSE NULL END AS id_lower,
    CASE WHEN typeof(s.position)='integer' THEN s.position ELSE NULL END AS position, ${rankExpression} AS search_rank
    FROM knowledge_search_grams g
    CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
    CROSS JOIN ${baseTable} b ON b.id=s.id
    WHERE ${filterSql.join(" AND ")} AND ${knowledgeTextMatch(kind === 'nodes' ? 'node' : 'relation', 'b')}${continuationSql}
    ORDER BY ${rankPolicy.order_by} LIMIT ?`;
  let bindings: unknown[] = [...rankBindings, ...filterBindings, needle, needle, ...continuationBindings, options.limit + 1];
  if (windowLast) {
    // MATERIALIZED is the IO boundary: optimizer predicate reordering must not
    // evaluate full search text for candidates outside the verified prefix.
    sql=`WITH selected_candidates AS MATERIALIZED (
      SELECT s.id,s.id_lower,s.position,${rankExpression} AS search_rank
      FROM knowledge_search_grams g CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
      WHERE ${filterSql.join(' AND ')}${continuationSql}
        AND ${rankPolicy.window_bound}
    ) SELECT c.id,c.id_lower,c.position,c.search_rank FROM selected_candidates c
      CROSS JOIN ${baseTable} b ON b.id=c.id WHERE ${knowledgeTextMatch(kind==='nodes'?'node':'relation','b')}
      ORDER BY ${rankPolicy.candidate_order_by} LIMIT ?`;
    bindings=[...rankBindings,...filterBindings,...continuationBindings,
      ...rankBindings,windowLast.search_rank,...rankBindings,windowLast.search_rank,
      windowLast.id_lower,windowLast.id_lower,windowLast.position,needle,needle,options.limit+1];
  }
  let result;
  try {
    result = await delivery.select(sql,bindings);
  } catch (error) {
    if(error instanceof HttpError || error instanceof NativeBudgetExceeded)throw error;
    throw new HttpError(503, `indexed knowledge search read model is unavailable: ${String(error)}`);
  }
  const pageState=workerSearchControl(runtime,'indexed_page',{
    result_rows:result.results.length,limit:options.limit,window_has_more:windowHasMore,
  });
  const selectedRows = result.results.slice(0,pageState.returned_rows as number);
  for(const row of result.results){
    workerSearchControl(runtime,'validate_selected',{position:row.position,search_rank:row.search_rank,id:row.id,id_lower:row.id_lower});
    if(typeof row.id!=='string'||typeof row.id_lower!=='string')
      throw new HttpError(503,'indexed knowledge search selected rank carrier is invalid');
  }
  const rowsValue = await delivery.items(kind,selectedRows);
  const hasMore=pageState.has_more as boolean;
  const last = pageState.last_source==='result' ? selectedRows[selectedRows.length-1]
    : pageState.last_source==='window' ? windowLast : null;
  const nextCursor = hasMore && last
    ? indexedCursorEncode(runtime,policy,'encode_cursor_kind',{source_revision:sourceRevision,snapshot_epoch:snapshotEpoch,
      kind,query:needle,filters,rank:last.search_rank,id:last.id_lower,position:last.position})
    : null;
  const resultRowsRead = indexedRowsRead(result.meta?.rows_read);
  return {
    rows: rowsValue,
    nextCursor,
    hasMore,
    work: {
      ...indexedWork(runtime,{phase:'page',grams:grams.length,selections:postingSelections.length,
        candidate_rows:candidateRows,verified_chars:verifiedChars,rank_chars:rankChars,
        has_window:windowLast!==null,preflight_rows_read:preflightRowsRead??null,
        result_rows_read:resultRowsRead??null,window_rows_read:windowRowsRead??null}),
    },
  };
}

async function knowledgeSearchD1IndexedUnchecked(
  db: D1Database,
  options: IndexedSearchOptions,
  snapshot: KnowledgeSnapshot,
  runtime:WorkerSearchControlsRuntime|undefined,
): Promise<NativePacket> {
  const normalized=normalizeWorkerSearch(runtime,'indexed',{...options,offset:0});
  const {needle,filters}=normalized;
  const limit=normalized.limit;
  const policy=workerSearchPolicy(runtime);
  const delivery = await NativeSearchDelivery.open(db,snapshot.revision);
  const sourceRevision = nativeField(delivery.top,'source_revision').value as string;
  let nodeExhausted = false;
  let relationExhausted = false;
  let nodeCursor: string | null = null;
  let relationCursor: string | null = null;
  if (options.cursor !== null && options.cursor !== undefined) {
    const decoded=workerSearchControl(runtime,'cursor_outer',{
      raw:indexedCursorRaw(options.cursor,policy.cursor_token_max_chars),source_revision:sourceRevision,snapshot_epoch:snapshot.epoch,
      query:needle,filters,
    });
    nodeExhausted=decoded.nodes_exhausted as boolean;
    relationExhausted=decoded.relations_exhausted as boolean;
    nodeCursor=decoded.nodes as string|null;
    relationCursor=decoded.relations as string|null;
  }
  const emptyPage = (): IndexedPage => ({
    rows: [],
    nextCursor: null,
    hasMore: false,
    work: workerSearchControl(runtime,'empty_page_work') as IndexedPage['work'],
  });
  const pageOptions={...options,query:needle,sources:normalized.sources,kindIds:normalized.kind_ids,predicateIds:normalized.predicate_ids,limit};
  const nodes=nodeExhausted?emptyPage():await indexedKindPage(db,pageOptions,'nodes',nodeCursor,sourceRevision,snapshot.epoch,delivery,runtime,policy,needle,filters);
  const relations=relationExhausted?emptyPage():await indexedKindPage(db,pageOptions,'relations',relationCursor,sourceRevision,snapshot.epoch,delivery,runtime,policy,needle,filters);
  const nextCursor = nodes.nextCursor || relations.nextCursor
    ? indexedCursorEncode(runtime,policy,'encode_cursor_outer',{
      source_revision: sourceRevision,
      snapshot_epoch: snapshot.epoch,
      query: needle,
      filters,
      nodes: nodes.nextCursor,
      relations: relations.nextCursor,
      nodes_exhausted: nodes.nextCursor === null,
      relations_exhausted: relations.nextCursor === null,
    })
    : null;
  const metadata=workerSearchControl(runtime,'indexed_packet_metadata',{
    source_revision:sourceRevision,query:options.query,filters,cursor:options.cursor??null,next_cursor:nextCursor,
    limit,has_cursor:options.cursor!==null&&options.cursor!==undefined,
    nodes_has_more:nodes.hasMore,relations_has_more:relations.hasMore,
    nodes_count:nodes.rows.length,relations_count:relations.rows.length,
    returned_nodes:nodes.rows.length,returned_relations:relations.rows.length,
    node_work:nodes.work,relation_work:relations.work,
  });
  return delivery.packet({...metadata,nodes:nodes.rows,relations:relations.rows},nodes.rows,relations.rows);
}

async function knowledgeSearchD1Unchecked(
  db: D1Database,
  options: { query: string; sources: string[] | null; kindIds: string[]; predicateIds: string[]; offset: number; limit: number },
  snapshot: KnowledgeSnapshot,
  runtime:WorkerSearchControlsRuntime|undefined,
): Promise<NativePacket> {
  const normalized=normalizeWorkerSearch(runtime,'legacy',options);
  const {query,needle,sources,offset,limit,filters}=normalized;
  const policy=workerSearchPolicy(runtime);
  options={...options,kindIds:normalized.kind_ids,predicateIds:normalized.predicate_ids};
  const rankPolicy=indexedRankPolicy(runtime,Boolean(needle));
  const rankBindings=indexedRankBindings(needle,rankPolicy.binding_count);
  const delivery = await NativeSearchDelivery.open(db,snapshot.revision);
  const nodeWhere = joinFragments([
    sourceFragment("n", sources),
    ...(options.kindIds.length ? [{ sql: "n.kind_id IN (SELECT value FROM json_each(?))", bindings: [JSON.stringify(options.kindIds)] }] : []),
    ...(needle ? [{ sql: knowledgeTextMatch('node', 'n'), bindings: [needle, needle] }] : []),
  ]);
  const relationWhere = joinFragments([
    sourceFragment("r", sources),
    ...(options.predicateIds.length ? [{ sql: "r.predicate_id IN (SELECT value FROM json_each(?))", bindings: [JSON.stringify(options.predicateIds)] }] : []),
    ...(needle ? [{ sql: knowledgeTextMatch('relation', 'r'), bindings: [needle, needle] }] : []),
  ]);
  const selected = async(kind:'nodes'|'relations',alias:string,where:SqlFragment) => {
    // Legacy exact count/selection retains its existing global work shape.
    // Do not apply selected-row native delivery quotas to this full scan.
    const rank=rankPolicy.sql;
    const result=(await delivery.select(
      `SELECT CASE WHEN typeof(${alias}.id)='text' AND length(CAST(${alias}.id AS BLOB))<=${policy.identity_max_bytes} THEN ${alias}.id ELSE NULL END AS id,
       CASE WHEN typeof(s.id_lower)='text' AND length(CAST(s.id_lower AS BLOB))<=${policy.identity_max_bytes} THEN s.id_lower ELSE NULL END AS id_lower,
       CASE WHEN typeof(s.position)='integer' THEN s.position ELSE NULL END AS position,${rank} AS search_rank
       FROM knowledge_search_documents s CROSS JOIN knowledge_${kind} ${alias} ON ${alias}.id=s.id
       WHERE s.kind=? AND ${where.sql} ORDER BY ${rankPolicy.order_by} LIMIT ? OFFSET ?`,
       [...rankBindings,kind,...where.bindings,limit,offset])).results;
    for(const row of result){
      workerSearchControl(runtime,'validate_legacy_selected',{position:row.position,id:row.id,id_lower:row.id_lower});
      if(typeof row.id!=='string'||typeof row.id_lower!=='string')
        throw new HttpError(503,'knowledge search selected rank carrier is invalid');
    }
    return delivery.items(kind,result);
  };
  const [nodeCount, relationCount] = await Promise.all([
    count(db, "knowledge_nodes n", nodeWhere),
    count(db, "knowledge_relations r", relationWhere),
  ]);
  const nodeRows=await selected('nodes','n',nodeWhere),relationRows=await selected('relations','r',relationWhere);
  workerSearchControl(runtime,'validate_legacy_closure',{limit,offset,matching_rows:nodeCount,returned_rows:nodeRows.length});
  workerSearchControl(runtime,'validate_legacy_closure',{limit,offset,matching_rows:relationCount,returned_rows:relationRows.length});
  const metadata=workerSearchControl(runtime,'legacy_packet_metadata',{
    query,filters,offset,limit,matching_nodes:nodeCount,matching_relations:relationCount,
    returned_nodes:nodeRows.length,returned_relations:relationRows.length,
  });
  return delivery.packet({...metadata,nodes:nodeRows,relations:relationRows},nodeRows,relationRows);
}

function knowledgeTextMatch(kind: 'node'|'relation', alias: string): string {
  return `(instr(${alias}.search_text, ?) > 0 OR (${alias}.json='' AND EXISTS(
    SELECT 1 FROM edge_meta overflow WHERE overflow.key='knowledge_${kind}_search:' || ${alias}.id
    AND instr(overflow.json_chunk, ?) > 0)))`;
}

import { HttpError, type Item } from "./common.ts";
import {
  focusLensSpec,
  type FocusKnowledgeOptions,
  type KnowledgeNode,
  type KnowledgeRelation,
} from "./knowledge.ts";
import { jsonRows, rows } from "./store.ts";
import {nativeLower, codePointCompare, nativeNumberInfo, NativeBudgetExceeded} from '../../../shared/native-semantics.ts';
import {nativeStrip} from '../../../shared/native-unicode.ts';
import {NativeSearchDelivery, nativeSearchFailure} from './native-search-store.ts';
import {executeNativeLensD1,PublishedLensD1Transport,readPublishedLensMetadata} from './native-lens-store.ts';
import {admitAuxiliary,readLensPublicationBinding} from './native-lens-auxiliary.ts';
import {respondLensSnapshot,lensError,SelectedLensError,type PublishedLensModule} from './selected-lens-runtime.ts';
import {InspectionD1Transport, readNativeInspectionPublication} from './native-inspection-store.ts';
import {respondInspectionSnapshot, inspectionError, SelectedInspectionError, type InspectionModule} from './selected-inspection-runtime.ts';
import {respondTemporalSnapshot, SelectedTemporalError, type TemporalPublishedModule} from './selected-temporal-runtime.ts';
import {parseNativeJson, parseNativeRequest, nativeField,nativePacketJson, arrayRefs, type NativeRef, type NativeLensResult, type NativePacket} from './native-lens.ts';
import {NativeD1Read, NativeD1Rows, nativeD1Limits, readNativePublication, nativeSha256, nativeUnavailable,type NativeD1Limits} from './native-d1-read.ts';

const KNOWLEDGE_SOURCES = new Set(["philosophy", "canon", "candidate-intake", "source-navigation", "source-claims", "semantic-interchange", "repository"]);
const SEARCH_NGRAM_SIZE = 3;
const SEARCH_MAX_CANDIDATES = 50_000;
const SEARCH_MAX_VERIFY_CHARS = 16_000_000;
const SEARCH_MAX_INTERSECTION_GRAMS = 3;
const SEARCH_CURSOR_SCHEMA = "tos_knowledge_search_indexed_cursor_v3";

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

export async function executeKnowledgeLensD1(db: D1Database, specValue: NativeRef): Promise<NativeLensResult> {
  return consistentRead(db, snapshot => executeNativeLensD1(db, specValue, {}, snapshot.revision));
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

export async function storedKnowledgeLensD1(db: D1Database, lensId: string): Promise<NativeLensResult> {
  return consistentRead(db,async snapshot=>{
    const catalog=await publishedCatalog(db,snapshot.revision);
    const lenses=nativeField(catalog,'lenses');
    if (!Array.isArray(lenses.value)) nativeUnavailable('published knowledge lens catalog is invalid');
    const matches=arrayRefs(lenses).filter(ref=>nativeField(ref,'lens_id').value===lensId);
    if (!matches.length) throw new HttpError(404,`unknown ToS knowledge lens: ${lensId}`);
    if (matches.length!==1) nativeUnavailable('published knowledge lens identity is ambiguous');
    return executeNativeLensD1(db,matches[0]!,{},snapshot.revision);
  });
}

export async function knowledgeSearchD1(db: D1Database, options: Parameters<typeof knowledgeSearchD1Unchecked>[1]): Promise<NativePacket> {
  return nativeSearchFailure(()=>consistentRead(db, snapshot => knowledgeSearchD1Unchecked(db, options, snapshot)));
}

export async function knowledgeSearchCapabilitiesD1(db: D1Database): Promise<Item> {
  return nativeSearchFailure(() => consistentRead(db, async snapshot => {
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
        legacy: {available: true, schema: 'tos_knowledge_search_v1', verification: 'engine-selection-only', pagination: 'offset'},
        indexed: {available: true, schema: 'tos_knowledge_search_indexed_v2', source_revision: nativeField(delivery.top, 'source_revision').value, verification: 'engine-selection-only', pagination: 'cursor', min_normalized_query_code_points: SEARCH_NGRAM_SIZE},
        compressed: {available: false, schema: 'tos_knowledge_search_compressed_v3', reason: 'not-supported-by-d1-adapter', writes_to_tree: false},
      },
    };
  }));
}

export async function knowledgeSearchD1Indexed(
  db: D1Database,
  options: Parameters<typeof knowledgeSearchD1IndexedUnchecked>[1],
): Promise<NativePacket> {
  return nativeSearchFailure(()=>consistentRead(db, (snapshot) => knowledgeSearchD1IndexedUnchecked(db, options, snapshot)));
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

function asNodes(items: Item[]): KnowledgeNode[] {
  return items as KnowledgeNode[];
}

function asRelations(items: Item[]): KnowledgeRelation[] {
  return items as KnowledgeRelation[];
}

export async function nodesByIds(db: D1Database, ids: Iterable<string>): Promise<KnowledgeNode[]> {
  const values = [...new Set(ids)];
  if (values.length === 0) return [];
  return asNodes(await jsonRows(
    db,
    "SELECT json FROM knowledge_nodes WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id",
    JSON.stringify(values),
  ));
}

export async function relationsByIds(db: D1Database, ids: Iterable<string>): Promise<KnowledgeRelation[]> {
  const values = [...new Set(ids)];
  if (values.length === 0) return [];
  return asRelations(await jsonRows(
    db,
    "SELECT json FROM knowledge_relations WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id",
    JSON.stringify(values),
  ));
}

export async function resolveFocusNodeD1(
  db: D1Database,
  requestedId: string | null,
  sources: string[],
): Promise<KnowledgeNode | null> {
  if (requestedId === null) return null;
  const source = sourceFragment("n", sources);
  const exact = asNodes(await jsonRows(
    db,
    `SELECT n.json FROM knowledge_nodes n WHERE ${source.sql} AND n.id = ? ORDER BY n.id`,
    ...source.bindings,
    requestedId,
  ));
  if (exact.length > 0) return exact[0]!;
  const entity = asNodes(await jsonRows(
    db,
    `SELECT n.json FROM knowledge_nodes n WHERE ${source.sql} AND n.entity_id = ? ORDER BY CASE n.source_graph WHEN 'source-navigation' THEN 0 WHEN 'canon' THEN 1 WHEN 'source-claims' THEN 2 WHEN 'philosophy' THEN 3 WHEN 'candidate-intake' THEN 4 WHEN 'repository' THEN 5 WHEN 'semantic-interchange' THEN 6 ELSE 99 END, n.id`,
    ...source.bindings,
    requestedId,
  ));
  if (entity.length > 0) return entity[0]!;
  const native = asNodes(await jsonRows(
    db,
    `SELECT n.json FROM knowledge_nodes n WHERE ${source.sql} AND n.native_id = ? ORDER BY n.id`,
    ...source.bindings,
    requestedId,
  ));
  if (native.length === 0) throw new HttpError(400, `unknown ToS knowledge focus: ${requestedId}`);
  if (native.length > 1) {
    throw new HttpError(
      400,
      `ambiguous ToS knowledge focus ${requestedId}: ${native.map((item) => item.id).join(", ")}; use a namespaced node id`,
    );
  }
  return native[0]!;
}


export async function focusKnowledgeNodeD1(
  db: D1Database,
  nodeId: string,
  options: FocusKnowledgeOptions = {},
): Promise<NativeLensResult> {
  return executeKnowledgeLensD1(db, parseNativeJson(JSON.stringify(focusLensSpec(nodeId, options))));
}

function normalizedSources(values: string[] | null): string[] {
  const result = values?.length ? [...new Set(values)] : [...KNOWLEDGE_SOURCES];
  const unknown = result.filter((value) => !KNOWLEDGE_SOURCES.has(value));
  if (unknown.length > 0) throw new HttpError(400, `unsupported knowledge sources: ${unknown.sort().join(", ")}`);
  return result;
}

function bounded(value: number, name: string, minimum: number, maximum: number): number {
  if (!Number.isInteger(value) || value < minimum || value > maximum) throw new HttpError(400, `${name} must be between ${minimum} and ${maximum}`);
  return value;
}

function indexedCursorEncode(value: Item): string {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  const encoded=btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
  if(encoded.length>8192)throw new NativeBudgetExceeded('indexed knowledge search cursor byte budget');
  return encoded;
}

function indexedCursorDecode(value: string): Item {
  if (!value || value.length > 8192) throw new HttpError(400, "invalid indexed knowledge search cursor");
  try {
    const padded = value.replaceAll("-", "+").replaceAll("_", "/") + "=".repeat((4 - (value.length % 4)) % 4);
    const binary = atob(padded);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    const ref = parseNativeRequest(new TextDecoder('utf-8',{fatal:true,ignoreBOM:false}).decode(bytes));
    const parsed: unknown = ref.value;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("not an object");
    if ((parsed as Item).schema === 'tos_knowledge_search_indexed_cursor_v2') throw new HttpError(409,'indexed knowledge search cursor predates native search; restart the query');
    if ((parsed as Item).schema !== SEARCH_CURSOR_SCHEMA) throw new Error("wrong cursor schema");
    for(const key of ['snapshot_epoch','rank','position']) if(key in (parsed as Item)) {
      if(typeof (parsed as Item)[key]!=='number'||nativeNumberInfo(nativeField(ref,key)).kind!=='int') throw new Error('cursor integer kind');
    }
    return parsed as Item;
  } catch(error) {
    if(error instanceof HttpError)throw error;
    throw new HttpError(400, "invalid indexed knowledge search cursor");
  }
}

function indexedRankExpression(alias: string): string {
  const exact = `${alias}.id_lower = ? OR ${alias}.native_id_lower = ? OR EXISTS (SELECT 1 FROM json_each(${alias}.identity_values) v WHERE v.value = ?)`;
  const prefix = `instr(${alias}.id_lower, ?) = 1 OR instr(${alias}.native_id_lower, ?) = 1 OR EXISTS (SELECT 1 FROM json_each(${alias}.identity_values) v WHERE instr(v.value, ?) = 1)`;
  const visible = `EXISTS (SELECT 1 FROM json_each(${alias}.visible_values) v WHERE instr(v.value, ?) > 0)`;
  return `CASE WHEN ${exact} THEN 0 WHEN ${prefix} THEN 1 WHEN ${visible} THEN 2 ELSE 3 END`;
}

function indexedRankBindings(needle: string): string[] {
  return [needle, needle, needle, needle, needle, needle, needle];
}

function indexedQueryGrams(needle: string): string[] {
  const codePoints = [...needle];
  const grams: string[] = [];
  for (let index = 0; index <= codePoints.length - SEARCH_NGRAM_SIZE; index += 1) {
    const gram = codePoints.slice(index, index + SEARCH_NGRAM_SIZE).join("");
    if (!grams.includes(gram)) grams.push(gram);
  }
  return grams;
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

function indexedFilters(value: unknown): value is IndexedFilters {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  const keys = Object.keys(candidate).sort();
  if (JSON.stringify(keys) !== JSON.stringify(["kind_ids", "predicate_ids", "sources"])) return false;
  return (["sources", "kind_ids", "predicate_ids"] as const).every((key) =>
    Array.isArray(candidate[key]) && candidate[key].every((item) => typeof item === "string")
  );
}

function indexedFiltersEqual(value: unknown, expected: IndexedFilters): boolean {
  return indexedFilters(value)
    && (['sources','kind_ids','predicate_ids'] as const).every(key=>JSON.stringify(value[key])===JSON.stringify(expected[key]));
}

function indexedRowsRead(value: unknown): number | undefined {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : undefined;
}

async function indexedKindPage(
  db: D1Database,
  options: IndexedSearchOptions,
  kind: "nodes" | "relations",
  cursor: string | null,
  sourceRevision: string,
  snapshotEpoch: number,
  delivery: NativeSearchDelivery,
): Promise<IndexedPage> {
  const needle = searchQuery(options.query,true).needle;
  if ([...needle].length < SEARCH_NGRAM_SIZE) {
    throw new HttpError(400, "indexed knowledge search requires a query of at least three characters");
  }
  const sources = normalizedSources(options.sources);
  const filters = {
    sources: [...new Set(sources)].sort(codePointCompare),
    kind_ids: [...new Set(options.kindIds)].sort(codePointCompare),
    predicate_ids: [...new Set(options.predicateIds)].sort(codePointCompare),
  };
  let cursorRank = -1;
  let cursorId = "";
  let cursorPosition = -1;
  if (cursor) {
    const decoded = indexedCursorDecode(cursor);
    const expectedKeys = ["filters", "id", "kind", "position", "query", "rank", "schema", "snapshot_epoch", "source_revision"];
    if (JSON.stringify(Object.keys(decoded).sort()) !== JSON.stringify(expectedKeys.sort())) {
      throw new HttpError(400, "invalid indexed knowledge search cursor");
    }
    if (
      typeof decoded.snapshot_epoch !== "number"
      || !Number.isSafeInteger(decoded.snapshot_epoch)
      || decoded.snapshot_epoch < 0
    ) {
      throw new HttpError(400, "invalid indexed knowledge search cursor");
    }
    if (
      decoded.source_revision !== sourceRevision
      || decoded.snapshot_epoch !== snapshotEpoch
      || decoded.kind !== kind
      || decoded.query !== needle
      || !indexedFiltersEqual(decoded.filters, filters)
    ) {
      throw new HttpError(409, "indexed knowledge search cursor does not match the current snapshot/query");
    }
    if (
      typeof decoded.rank !== "number"
      || !Number.isSafeInteger(decoded.rank)
      || Number(decoded.rank) < 0
      || Number(decoded.rank) > 3
      || typeof decoded.id !== "string"
      || decoded.id.length === 0
      || decoded.id !== nativeLower(decoded.id)
      || typeof decoded.position !== "number"
      || !Number.isSafeInteger(decoded.position)
      || Number(decoded.position) < 0
    ) {
      throw new HttpError(400, "invalid indexed knowledge search cursor");
    }
    cursorRank = decoded.rank;
    cursorId = decoded.id;
    cursorPosition = decoded.position;
  }
  const grams = indexedQueryGrams(needle);
  const stats = await Promise.all(grams.map(async (gram) => {
    const row = await db.prepare(
      "SELECT CASE WHEN typeof(postings)='integer' THEN postings ELSE NULL END AS postings FROM knowledge_search_gram_stats WHERE kind=? AND n=? AND gram=?",
    ).bind(kind, SEARCH_NGRAM_SIZE, gram).first<{ postings: number }>();
    const postings=row===null?0:row.postings;
    if(!Number.isSafeInteger(postings)||postings<0)throw new HttpError(503,'indexed knowledge search gram statistics are invalid');
    return { gram, postings };
  }));
  const selected = stats.reduce((best, candidate) => candidate.postings < best.postings ? candidate : best);
  if (selected.postings > SEARCH_MAX_CANDIDATES) {
    throw new HttpError(413, "indexed knowledge search candidate budget exceeded; narrow the query or use the legacy route");
  }
  // Intersect a bounded number of rare postings before visiting document
  // bodies. A rare substring alone can still select wide unrelated records.
  // Stable sort preserves the original query-order tie choice. The total
  // posting closure read stays inside the existing candidate budget, rather
  // than multiplying that budget by the number of intersected grams.
  const postingSelections = [selected];
  let closurePostings = selected.postings;
  if (selected.postings > 0) {
    for (const candidate of [...stats].sort((left, right) => left.postings - right.postings)) {
      if (candidate.gram === selected.gram) continue;
      if (postingSelections.length >= SEARCH_MAX_INTERSECTION_GRAMS
          || closurePostings + candidate.postings > SEARCH_MAX_CANDIDATES) break;
      postingSelections.push(candidate);
      closurePostings += candidate.postings;
    }
  }
  // Validate every posting used for exclusion, including the zero-stat path.
  // A missing secondary posting must not silently remove a genuine match.
  for (const selection of postingSelections) {
    const posting=await db.prepare(`SELECT count(*) AS total,coalesce(sum(CASE WHEN document_position IS NULL THEN 1 ELSE 0 END),0) AS missing
    FROM (SELECT s.position AS document_position FROM knowledge_search_grams g
      LEFT JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
      WHERE g.kind=? AND g.n=? AND g.gram=? LIMIT ?)`).bind(kind,SEARCH_NGRAM_SIZE,selection.gram,selection.postings+1)
    .first<{total:number;missing:number}>();
    if(!posting||!Number.isSafeInteger(posting.total)||!Number.isSafeInteger(posting.missing))throw new HttpError(503,'indexed knowledge search posting metadata is invalid');
    if(posting.total>SEARCH_MAX_CANDIDATES)throw new HttpError(413,'indexed knowledge search candidate budget exceeded');
    if(posting.total!==selection.postings||posting.missing!==0)throw new HttpError(503,'indexed knowledge search posting closure is incomplete');
  }
  if(selected.postings===0)return {rows:[],nextCursor:null,hasMore:false,work:{candidate_rows:0,verified_chars:0,sql_pages:grams.length+1}};
  const rankExpression = indexedRankExpression("s");
  const baseTable = kind === "nodes" ? "knowledge_nodes" : "knowledge_relations";
  const filterSql: string[] = ["g.kind = ?", "g.n = ?", "g.gram = ?"];
  const filterBindings: unknown[] = [kind, SEARCH_NGRAM_SIZE, selected.gram];
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
    ? ` AND (${rankExpression} > ? OR (${rankExpression} = ? AND (s.id_lower > ? OR (s.id_lower = ? AND s.position > ?))))`
    : "";
  const continuationBindings = cursor
    ? [...indexedRankBindings(needle), cursorRank, ...indexedRankBindings(needle), cursorRank, cursorId, cursorId, cursorPosition]
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
  let candidateRows = aggregate?.candidate_rows ?? 0;
  let verifiedChars = aggregate?.verified_chars ?? 0;
  const rankChars = aggregate?.rank_chars ?? 0;
  if (
    aggregate?.invalid_budgets!==0
    || aggregate?.invalid_rank_metadata!==0
    || !Number.isSafeInteger(rankChars)
    || rankChars < 0
    || !Number.isSafeInteger(candidateRows)
    || candidateRows < 0
    || !Number.isSafeInteger(verifiedChars)
    || verifiedChars < 0
  ) {
    throw new HttpError(503, "indexed knowledge search carrier has invalid document budgets");
  }
  const preflightRowsRead = indexedRowsRead(preflight.meta?.rows_read);
  if(candidateRows>SEARCH_MAX_CANDIDATES)throw new HttpError(413,'indexed knowledge search candidate budget exceeded');
  if (rankChars >= SEARCH_MAX_VERIFY_CHARS) {
    throw new HttpError(413, "indexed knowledge search rank metadata budget exceeded; narrow the query");
  }
  if (candidateRows === 0) {
    return {
      rows: [],
      nextCursor: null,
      hasMore: false,
      work: {
        candidate_rows: 0,
        verified_chars: 0,
        sql_pages: grams.length + postingSelections.length + 1,
        ...(preflightRowsRead === undefined ? {} : { selection_rows_read: preflightRowsRead }),
      },
    };
  }
  // A numeric/metadata-only ordered window bounds text IO before joining the
  // native carriers. Its last verified candidate is a progress cursor even
  // when no candidate in this window contains the complete query string.
  type VerificationWindow = {id:string;id_lower:string;position:number;search_rank:number;prefix_chars:number;prefix_rows:number;remaining_rows:number};
  let windowLast: VerificationWindow|null = null;
  let windowHasMore = false;
  let windowRowsRead: number|undefined = 0;
  if (verifiedChars + rankChars > SEARCH_MAX_VERIFY_CHARS) {
    const windowSql = `WITH ranked AS MATERIALIZED (
      SELECT s.id,s.id_lower,s.position,s.document_chars,${rankExpression} AS search_rank
      FROM knowledge_search_grams g CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
      WHERE ${filterSql.join(' AND ')}${continuationSql}
    ), bounded_window AS (
      SELECT *,SUM(document_chars) OVER (ORDER BY search_rank,id_lower,position ROWS UNBOUNDED PRECEDING) AS prefix_chars,
        ROW_NUMBER() OVER (ORDER BY search_rank,id_lower,position) AS prefix_rows FROM ranked
    ) SELECT CASE WHEN length(CAST(last.id AS BLOB))<=1048576 THEN last.id ELSE NULL END AS id,
      CASE WHEN length(CAST(last.id_lower AS BLOB))<=1048576 THEN last.id_lower ELSE NULL END AS id_lower,
      last.position,last.search_rank,last.prefix_chars,last.prefix_rows,total.remaining_rows
      FROM (SELECT COUNT(*) AS remaining_rows FROM ranked) total
      LEFT JOIN (SELECT id,id_lower,position,search_rank,prefix_chars,prefix_rows FROM bounded_window
        WHERE prefix_chars<=? ORDER BY search_rank DESC,id_lower DESC,position DESC LIMIT 1) last ON 1=1`;
    const window = await db.prepare(windowSql).bind(...indexedRankBindings(needle),...filterBindings,
      ...continuationBindings,SEARCH_MAX_VERIFY_CHARS-rankChars).all<VerificationWindow>();
    windowLast = window.results[0] ?? null;
    windowRowsRead = indexedRowsRead(window.meta?.rows_read);
    if (windowLast?.remaining_rows===0) {
      return {rows:[],nextCursor:null,hasMore:false,work:{candidate_rows:0,verified_chars:0,rank_chars:rankChars,
        sql_pages:grams.length+postingSelections.length+2,
        ...(preflightRowsRead===undefined||windowRowsRead===undefined?{}:{selection_rows_read:preflightRowsRead+windowRowsRead})}};
    }
    if (!windowLast || windowLast.prefix_rows===null) throw new HttpError(413,'indexed knowledge search first remaining document exceeds verification budget');
    if (windowLast.id===null || windowLast.id_lower===null) throw new HttpError(413,'indexed knowledge search window identity exceeds delivery budget');
    if (typeof windowLast.id!=='string'||windowLast.id_lower!==nativeLower(windowLast.id)
        || ![windowLast.position,windowLast.search_rank,windowLast.prefix_chars,windowLast.prefix_rows,windowLast.remaining_rows].every(Number.isSafeInteger)
        || windowLast.position<0||windowLast.search_rank<0||windowLast.search_rank>3
        || windowLast.prefix_chars<0||windowLast.prefix_rows<1||windowLast.remaining_rows<windowLast.prefix_rows) {
      throw new HttpError(503,'indexed knowledge search verification window is invalid');
    }
    candidateRows=windowLast.prefix_rows;
    verifiedChars=windowLast.prefix_chars;
    windowHasMore=windowLast.prefix_rows<windowLast.remaining_rows;
  }
  let sql = `SELECT CASE WHEN typeof(s.id)='text' AND length(CAST(s.id AS BLOB))<=1048576 THEN s.id ELSE NULL END AS id,
    CASE WHEN typeof(s.id_lower)='text' AND length(CAST(s.id_lower AS BLOB))<=1048576 THEN s.id_lower ELSE NULL END AS id_lower,
    CASE WHEN typeof(s.position)='integer' THEN s.position ELSE NULL END AS position, ${rankExpression} AS search_rank
    FROM knowledge_search_grams g
    CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
    CROSS JOIN ${baseTable} b ON b.id=s.id
    WHERE ${filterSql.join(" AND ")} AND ${knowledgeTextMatch(kind === 'nodes' ? 'node' : 'relation', 'b')}${continuationSql}
    ORDER BY search_rank, s.id_lower, s.position LIMIT ?`;
  let bindings: unknown[] = [...indexedRankBindings(needle), ...filterBindings, needle, needle, ...continuationBindings, options.limit + 1];
  if (windowLast) {
    // MATERIALIZED is the IO boundary: optimizer predicate reordering must not
    // evaluate full search text for candidates outside the verified prefix.
    sql=`WITH selected_candidates AS MATERIALIZED (
      SELECT s.id,s.id_lower,s.position,${rankExpression} AS search_rank
      FROM knowledge_search_grams g CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
      WHERE ${filterSql.join(' AND ')}${continuationSql}
        AND (${rankExpression}<? OR (${rankExpression}=? AND (s.id_lower<? OR (s.id_lower=? AND s.position<=?))))
    ) SELECT c.id,c.id_lower,c.position,c.search_rank FROM selected_candidates c
      CROSS JOIN ${baseTable} b ON b.id=c.id WHERE ${knowledgeTextMatch(kind==='nodes'?'node':'relation','b')}
      ORDER BY c.search_rank,c.id_lower,c.position LIMIT ?`;
    bindings=[...indexedRankBindings(needle),...filterBindings,...continuationBindings,
      ...indexedRankBindings(needle),windowLast.search_rank,...indexedRankBindings(needle),windowLast.search_rank,
      windowLast.id_lower,windowLast.id_lower,windowLast.position,needle,needle,options.limit+1];
  }
  let result;
  try {
    result = await delivery.select(sql,bindings);
  } catch (error) {
    if(error instanceof HttpError || error instanceof NativeBudgetExceeded)throw error;
    throw new HttpError(503, `indexed knowledge search read model is unavailable: ${String(error)}`);
  }
  const selectedRows = result.results.slice(0, options.limit);
  if(result.results.some(row=>typeof row.id!=='string'||typeof row.id_lower!=='string'||row.id_lower!==nativeLower(row.id)||!Number.isSafeInteger(row.position)||row.position<0||!Number.isSafeInteger(row.search_rank)||row.search_rank<0||row.search_rank>3))throw new HttpError(503,'indexed knowledge search selected rank carrier is invalid');
  const rowsValue = await delivery.items(kind,selectedRows);
  const moreMatches = result.results.length > options.limit;
  const hasMore = moreMatches || windowHasMore;
  const last = moreMatches ? selectedRows[selectedRows.length - 1] : windowHasMore ? windowLast : null;
  const nextCursor = hasMore && last
    ? indexedCursorEncode({ schema: SEARCH_CURSOR_SCHEMA, source_revision: sourceRevision, snapshot_epoch: snapshotEpoch, kind, query: needle, filters, rank: last.search_rank, id: last.id_lower, position: last.position })
    : null;
  const resultRowsRead = indexedRowsRead(result.meta?.rows_read);
  const rowsRead = preflightRowsRead === undefined || resultRowsRead === undefined || windowRowsRead === undefined
    ? undefined
    : preflightRowsRead + resultRowsRead + windowRowsRead;
  return {
    rows: rowsValue,
    nextCursor,
    hasMore,
    work: {
      candidate_rows: candidateRows,
      verified_chars: verifiedChars,
      rank_chars: rankChars,
      sql_pages: grams.length + postingSelections.length + 2 + Number(windowLast!==null),
      // This excludes independent gram-stat/posting-closure, metadata, and consistency
      // reads; it is not a total query-cost counter.
      ...(rowsRead === undefined ? {} : { selection_rows_read: rowsRead }),
    },
  };
}

async function knowledgeSearchD1IndexedUnchecked(
  db: D1Database,
  options: IndexedSearchOptions,
  snapshot: KnowledgeSnapshot,
): Promise<NativePacket> {
  const {needle} = searchQuery(options.query,true);
  const limit = bounded(options.limit, "limit", 1, 100);
  const sources=normalizedSources(options.sources);
  if ([options.kindIds,options.predicateIds].some(values=>values.length+sources.length>100||values.some(value=>typeof value!=='string'||[...value].length>256)))throw new HttpError(400,'knowledge search filters exceed bounded query input');
  const delivery = await NativeSearchDelivery.open(db,snapshot.revision);
  const sourceRevision = nativeField(delivery.top,'source_revision').value as string;
  const filters: IndexedFilters = {
    sources: [...sources].sort(codePointCompare),
    kind_ids: [...new Set(options.kindIds)].sort(codePointCompare),
    predicate_ids: [...new Set(options.predicateIds)].sort(codePointCompare),
  };
  const decodedCursor = options.cursor === null || options.cursor === undefined ? null : indexedCursorDecode(options.cursor);
  let nodeExhausted = false;
  let relationExhausted = false;
  let nodeCursor: string | null = null;
  let relationCursor: string | null = null;
  if (decodedCursor) {
    const expectedKeys = [
      "filters", "nodes", "nodes_exhausted", "query", "relations", "relations_exhausted", "schema", "snapshot_epoch", "source_revision",
    ];
    if (JSON.stringify(Object.keys(decodedCursor).sort()) !== JSON.stringify(expectedKeys.sort())) {
      throw new HttpError(400, "invalid indexed knowledge search cursor");
    }
    if (
      typeof decodedCursor.snapshot_epoch !== "number"
      || !Number.isSafeInteger(decodedCursor.snapshot_epoch)
      || decodedCursor.snapshot_epoch < 0
    ) {
      throw new HttpError(400, "invalid indexed knowledge search cursor");
    }
    if (
      decodedCursor?.source_revision !== sourceRevision
      || decodedCursor.snapshot_epoch !== snapshot.epoch
      || decodedCursor?.query !== needle
      || !indexedFiltersEqual(decodedCursor.filters, filters)
    ) {
      throw new HttpError(409, "indexed knowledge search cursor does not match the current snapshot/query");
    }
    if (typeof decodedCursor.nodes_exhausted !== "boolean" || typeof decodedCursor.relations_exhausted !== "boolean") {
      throw new HttpError(400, "invalid indexed knowledge search cursor");
    }
    nodeExhausted = decodedCursor.nodes_exhausted;
    relationExhausted = decodedCursor.relations_exhausted;
    const rawNodeCursor = decodedCursor.nodes;
    const rawRelationCursor = decodedCursor.relations;
    if (nodeExhausted) {
      if (rawNodeCursor !== null) throw new HttpError(400, "invalid indexed knowledge search cursor");
    } else if (typeof rawNodeCursor !== "string" || rawNodeCursor.length === 0) {
      throw new HttpError(400, "invalid indexed knowledge search cursor");
    } else {
      nodeCursor = rawNodeCursor;
    }
    if (relationExhausted) {
      if (rawRelationCursor !== null) throw new HttpError(400, "invalid indexed knowledge search cursor");
    } else if (typeof rawRelationCursor !== "string" || rawRelationCursor.length === 0) {
      throw new HttpError(400, "invalid indexed knowledge search cursor");
    } else {
      relationCursor = rawRelationCursor;
    }
  }
  const emptyPage = (): IndexedPage => ({
    rows: [],
    nextCursor: null,
    hasMore: false,
    work: { candidate_rows: 0, verified_chars: 0, sql_pages: 0 },
  });
  const nodes=nodeExhausted?emptyPage():await indexedKindPage(db,{...options,limit},'nodes',nodeCursor,sourceRevision,snapshot.epoch,delivery);
  const relations=relationExhausted?emptyPage():await indexedKindPage(db,{...options,limit},'relations',relationCursor,sourceRevision,snapshot.epoch,delivery);
  const nextCursor = nodes.nextCursor || relations.nextCursor
    ? indexedCursorEncode({
      schema: SEARCH_CURSOR_SCHEMA,
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
  return delivery.packet({
    schema: "tos_knowledge_search_indexed_v2",
    source_revision: sourceRevision,
    query: options.query,
    filters,
    page: { cursor: options.cursor ?? null, next_cursor: nextCursor, limit_per_kind: limit, ordering_scope: "global-rank", has_more: nextCursor !== null },
    counts: {
      matching_nodes: !options.cursor && !nodes.hasMore ? nodes.rows.length : null,
      matching_relations: !options.cursor && !relations.hasMore ? relations.rows.length : null,
      returned_nodes: nodes.rows.length,
      returned_relations: relations.rows.length,
      scope: "exact-if-kind-exhausted-without-continuation",
    },
    nodes: nodes.rows,
    relations: relations.rows,
    authority_boundary: null,
    work: {nodes: nodes.work, relations: relations.work},
  },nodes.rows,relations.rows);
}

async function knowledgeSearchD1Unchecked(
  db: D1Database,
  options: { query: string; sources: string[] | null; kindIds: string[]; predicateIds: string[]; offset: number; limit: number },
  snapshot: KnowledgeSnapshot,
): Promise<NativePacket> {
  const {query,needle} = searchQuery(options.query,false);
  const sources = normalizedSources(options.sources);
  const offset = bounded(options.offset, "offset", 0, 100_000);
  const limit = bounded(options.limit, "limit", 1, 100);
  options={...options,kindIds:[...new Set(options.kindIds.filter(value=>typeof value==='string'&&value))],predicateIds:[...new Set(options.predicateIds.filter(value=>typeof value==='string'&&value))]};
  if (options.kindIds.length > 100 || options.predicateIds.length > 100) {
    throw new HttpError(400, "knowledge search kind and predicate filters must contain at most 100 values");
  }
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
    const rank=needle?indexedRankExpression('s'):'CASE WHEN 1 THEN 3 END';
    const result=(await delivery.select(
      `SELECT CASE WHEN typeof(${alias}.id)='text' AND length(CAST(${alias}.id AS BLOB))<=1048576 THEN ${alias}.id ELSE NULL END AS id,
       CASE WHEN typeof(s.id_lower)='text' AND length(CAST(s.id_lower AS BLOB))<=1048576 THEN s.id_lower ELSE NULL END AS id_lower,
       CASE WHEN typeof(s.position)='integer' THEN s.position ELSE NULL END AS position,${rank} AS search_rank
       FROM knowledge_search_documents s CROSS JOIN knowledge_${kind} ${alias} ON ${alias}.id=s.id
       WHERE s.kind=? AND ${where.sql} ORDER BY search_rank,s.id_lower,s.position LIMIT ? OFFSET ?`,
       [...(needle?indexedRankBindings(needle):[]),kind,...where.bindings,limit,offset])).results;
    if(result.some(row=>typeof row.id!=='string'||row.id_lower!==nativeLower(row.id)||!Number.isSafeInteger(row.position)||row.position<0))throw new HttpError(503,'knowledge search selected rank carrier is invalid');
    return delivery.items(kind,result);
  };
  const [nodeCount, relationCount] = await Promise.all([
    count(db, "knowledge_nodes n", nodeWhere),
    count(db, "knowledge_relations r", relationWhere),
  ]);
  const nodeRows=await selected('nodes','n',nodeWhere),relationRows=await selected('relations','r',relationWhere);
  if(nodeRows.length!==Math.min(limit,Math.max(0,nodeCount-offset))||relationRows.length!==Math.min(limit,Math.max(0,relationCount-offset)))throw new HttpError(503,'knowledge search selected rank closure is incomplete');
  return delivery.packet({
    schema: "tos_knowledge_search_v1",
    source_revision: null,
    query,
    filters: { sources: [...sources].sort(codePointCompare), kind_ids: [...new Set(options.kindIds)].sort(codePointCompare), predicate_ids: [...new Set(options.predicateIds)].sort(codePointCompare) },
    page: { offset, limit_per_kind: limit },
    counts: { matching_nodes: nodeCount, matching_relations: relationCount, returned_nodes: nodeRows.length, returned_relations: relationRows.length },
    nodes: nodeRows,
    relations: relationRows,
    authority_boundary: null,
  },nodeRows,relationRows);
}

function knowledgeTextMatch(kind: 'node'|'relation', alias: string): string {
  return `(instr(${alias}.search_text, ?) > 0 OR (${alias}.json='' AND EXISTS(
    SELECT 1 FROM edge_meta overflow WHERE overflow.key='knowledge_${kind}_search:' || ${alias}.id
    AND instr(overflow.json_chunk, ?) > 0)))`;
}

function searchQuery(value:string,indexed:boolean):{query:string;needle:string} {
  if(typeof value!=='string')throw new HttpError(400,'knowledge search query must be a string');
  const query=nativeStrip(value),needle=nativeLower(query);
  if((indexed&&[...value].length>256)||[...query].length>256||(indexed&&[...needle].length>256))throw new HttpError(400,'knowledge search query exceeds 256 characters');
  if(indexed&&[...needle].length<SEARCH_NGRAM_SIZE)throw new HttpError(400,'indexed knowledge search requires a query of at least three characters');
  return {query,needle};
}

import {installSourceNavigationRules} from "./source-navigation-rules.ts";
import { boundedInt, HttpError, jsonResponse, listParam, withSecurity, type Item } from "./common";
import {
  buildHealth,
  corpusGraphView,
  corpusNodePacket,
  corpusRelationPack,
  corpusSearch,
  evidenceLens,
  philosophyClusters,
  philosophyEdgePacket,
  philosophyNeighborhood,
  philosophyNodePacket,
  philosophyPacket,
  philosophyPath,
  philosophySearch,
  philosophyView,
} from "./queries";
import { scaleExportResponse } from "./scale";
import {
  lensSnapshotResponseD1,
  inspectionSnapshotResponseD1,
  knowledgeSearchD1,
  knowledgeSearchCapabilitiesD1,
  knowledgeSearchD1Indexed,
  temporalSnapshotResponseD1,
  knowledgeCatalogD1,
} from "./knowledge-store";
import { SourceNavigationError, sourceDescendD1, sourceDossierD1 } from "./source-navigation-store";
import { meta, metaItem, requirePublicD1ProducerProfile } from "./store";
import { explorationSnapshotResponseD1, explorationCapabilitiesD1 } from "./exploration";
import {nativePacketResponse} from './native-lens-response.ts';
import {codePointCompare, installNativePythonRuntime, NativeBudgetExceeded} from '../../../shared/native-semantics.ts';
import {nativeStrip,nativeIntegerString} from '../../../shared/native-unicode.ts';
import {nativeSha256} from './native-d1-read.ts';
import { withWebAnalytics } from "./web-analytics";
import { publicPrototypeAsset, withReaderSecurity } from "./public-prototypes.ts";
import {SelectedTemporalError} from './selected-temporal-runtime.ts';
import {sourceTextGatewayCapability, sourceTextSearchGatewayResponse} from './source-text-gateway.ts';
import {installKnowledgeSceneRules} from '../../../shared/knowledge-scene.ts';
import {initSync, KnowledgeSceneSession, TemporalReplaySession, validate_temporal_request_wasm_v1,
  InspectionSession, validate_inspect_request_wasm_v1, LensSession, validate_lens_request_wasm_v1,
  ExplorationSession,validate_exploration_request_wasm_v1,validate_exploration_replay_wasm_v1,
  exploration_cache_version_wasm_v1,worker_knowledge_search_controls_wasm_v1} from '../generated/tos_web_rules.js';
import * as classicRuntime from "../generated/tos_web_rules.js";
import {installWorkerClassicRules} from "./worker-classic.ts";
import temporalWasm from '../generated/tos_web_rules_bg.wasm';

// wasm-bindgen owns module initialization; no second host cache or fetch.
initSync({module: temporalWasm});
installWorkerClassicRules(classicRuntime);
installNativePythonRuntime(classicRuntime);
installSourceNavigationRules(classicRuntime);
installKnowledgeSceneRules({KnowledgeSceneSession});
const temporalRuntime = {TemporalReplaySession, validate_temporal_request_wasm_v1};
const inspectionRuntime = {InspectionSession, validate_inspect_request_wasm_v1};
const lensRuntime={LensSession,validate_lens_request_wasm_v1};
const explorationRuntime={ExplorationSession,validate_exploration_request_wasm_v1,
  validate_exploration_replay_wasm_v1,exploration_cache_version_wasm_v1};
const workerSearchControlsRuntime={worker_knowledge_search_controls_wasm_v1};

const STATIC_CORPUS_LIMITS = new Set([1, 100, 700, 1000]);
const STATIC_PHILOSOPHY_LIMITS = new Set([1, 1000]);
const MAX_LENS_REQUEST_BYTES = 64 * 1024;

function segment(pathname: string, prefix: string): string {
  return decodeURIComponent(pathname.slice(prefix.length).split("/", 1)[0] ?? "");
}

function required(search: URLSearchParams, key: string): string {
  const value = (search.get(key) ?? "").trim();
  if (!value) throw new HttpError(400, `${key} is required`);
  return value;
}

function booleanParam(search: URLSearchParams, key: string, fallback = false): boolean {
  const value = (search.get(key) ?? (fallback ? "true" : "false")).trim().toLowerCase();
  if (["1", "true", "yes", "on"].includes(value)) return true;
  if (["0", "false", "no", "off"].includes(value)) return false;
  throw new HttpError(400, `${key} must be a boolean`);
}

async function staticApi(env: Env, request: Request, relativePath: string): Promise<Response> {
  const assetUrl = new URL(`/__edge/${relativePath}`, request.url);
  const asset = await env.ASSETS.fetch(new Request(assetUrl, { method: request.method }));
  if (!asset.ok) throw new Error(`generated edge asset is missing: ${relativePath}`);
  const headers = new Headers(asset.headers);
  headers.set("Cache-Control", "public, max-age=300, must-revalidate");
  return withSecurity(new Response(asset.body, { status: asset.status, headers }));
}

async function staticItem(env: Env, request: Request, relativePath: string): Promise<Item> {
  const assetUrl = new URL(`/__edge/${relativePath}`, request.url);
  const asset = await env.ASSETS.fetch(new Request(assetUrl));
  if (!asset.ok) throw new Error(`generated edge asset is missing: ${relativePath}`);
  const payload = await asset.json();
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
    throw new Error(`generated edge asset is invalid: ${relativePath}`);
  }
  return payload as Item;
}

type SourceGapManifestRecord = { source_ref: string; sha256: string; size_bytes: number };
type SourceGapManifest = {
  schema: string;
  source_owner: string;
  selection: string;
  max_records: number;
  max_record_bytes: number;
  records: SourceGapManifestRecord[];
};
type SourceGapChunkRow = { key: string; part: number; json_chunk: string | null };
const SOURCE_GAP_MAX_CHUNKS = 100 * Math.ceil(256_000 / (32_000 - 3));

async function selectedSourceGapRecords(db: D1Database): Promise<Array<SourceGapManifestRecord & {raw: string}>> {
  await requirePublicD1ProducerProfile(db);
  const manifest = await meta<SourceGapManifest>(db, "source_gap_records_v1");
  if (!manifest || manifest.schema !== "tos_public_source_gap_records_v1"
    || manifest.source_owner !== "Tree-of-Sophia"
    || manifest.selection !== "captured-public-ledger-members-v1"
    || manifest.max_records !== 100 || manifest.max_record_bytes !== 256_000
    || !Array.isArray(manifest.records) || manifest.records.length > manifest.max_records) {
    throw new HttpError(503, "source-gap selected source manifest is unavailable");
  }
  const entries = manifest.records;
  const keys: string[] = [];
  const keyByPath = new Map<string, string>();
  let expectedTotal = 0;
  let previous = "";
  for (const entry of entries) {
    const path = entry?.source_ref;
    const prefix = "ToS/source-witnesses/access-requests/public-ledger/";
    const filename = typeof path === "string" && path.startsWith(prefix) ? path.slice(prefix.length) : "";
    if (!filename || filename.includes("/") || filename === "." || filename === ".."
      || !filename.endsWith(".access-request.json") || path.includes("\\") || path.includes("\0")
      || (previous && codePointCompare(previous, path) >= 0)
      || typeof entry.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(entry.sha256)
      || !Number.isSafeInteger(entry.size_bytes) || entry.size_bytes < 1
      || entry.size_bytes > manifest.max_record_bytes) {
      throw new HttpError(503, "source-gap selected source manifest is invalid");
    }
    previous = path;
    expectedTotal += entry.size_bytes;
    if (expectedTotal > 64 * 1024 * 1024) {
      throw new HttpError(503, "source-gap selected input exceeds its source budget");
    }
    const key = `source_gap_record:${await nativeSha256(path)}`;
    keys.push(key);
    keyByPath.set(path, key);
  }
  if (keys.length === 0) return [];
  if (new Set(keys).size !== keys.length) {
    throw new HttpError(503, "source-gap selected source identities are duplicated");
  }
  const selected = await db.prepare(
    `SELECT key, part,
            CASE WHEN typeof(json_chunk)='text' THEN json_chunk ELSE NULL END AS json_chunk
       FROM edge_meta WHERE key IN (SELECT value FROM json_each(?)) ORDER BY key, part LIMIT ?`,
  ).bind(JSON.stringify(keys), SOURCE_GAP_MAX_CHUNKS + 1).all<SourceGapChunkRow>();
  if (selected.results.length > SOURCE_GAP_MAX_CHUNKS) {
    throw new HttpError(503, "source-gap selected source chunk count exceeds its profile");
  }
  const keySet = new Set(keys);
  const byKey = new Map<string, SourceGapChunkRow[]>();
  for (const row of selected.results) {
    if (!keySet.has(row.key) || !Number.isSafeInteger(row.part) || row.part < 0
      || typeof row.json_chunk !== "string") {
      throw new HttpError(503, "source-gap selected source chunks are invalid");
    }
    const rows = byKey.get(row.key) ?? [];
    rows.push(row);
    byKey.set(row.key, rows);
  }
  const hydrated: Array<SourceGapManifestRecord & {raw: string}> = [];
  const encoder = new TextEncoder();
  for (const entry of entries) {
    const key = keyByPath.get(entry.source_ref)!;
    const chunks = byKey.get(key);
    const maximumChunks = Math.ceil(entry.size_bytes / (32_000 - 3));
    if (!chunks?.length || chunks.length > maximumChunks) {
      throw new HttpError(503, "source-gap selected source member is missing or oversized");
    }
    let bytes = 0;
    for (let index = 0; index < chunks.length; index += 1) {
      const row = chunks[index]!;
      const chunkBytes = encoder.encode(row.json_chunk!).length;
      if (row.part !== index || chunkBytes > 32_000) {
        throw new HttpError(503, "source-gap selected source chunk order is invalid");
      }
      bytes += chunkBytes;
    }
    const raw = chunks.map((row) => row.json_chunk!).join("");
    if (bytes !== entry.size_bytes || await nativeSha256(raw) !== entry.sha256) {
      throw new HttpError(503, "source-gap selected source digest differs");
    }
    hydrated.push({...entry, raw});
  }
  return hydrated;
}

async function sourceGapResponse(request: Request, env: Env, search: URLSearchParams): Promise<Response> {
  const query = search.get("query") ?? "";
  const limit = boundedInt(search.get("limit"), 20, 1, 100);
  let packetBytes: Uint8Array;
  try {
    const worker = new classicRuntime.WorkerSourceGapSearch(query, limit);
    const records = await selectedSourceGapRecords(env.DB);
    for (const record of records) worker.add_record(record.source_ref, record.sha256, record.raw);
    packetBytes = worker.finish();
  } catch (error) {
    if (error instanceof HttpError) throw error;
    const detail = String(error);
    if (detail.includes("InvalidRequest") || detail.includes("must be 1..100")) {
      throw new HttpError(400, "source-gap query is invalid");
    }
    if (detail.includes("BudgetExceeded") || detail.includes("budget")) {
      throw new HttpError(413, "source-gap query exceeds its bounded profile");
    }
    throw new HttpError(503, "source-gap selected source set is unavailable");
  }
  let packet: Item;
  try {
    packet = JSON.parse(new TextDecoder().decode(packetBytes)) as Item;
  } catch {
    throw new HttpError(503, "source-gap query response is invalid");
  }
  return jsonResponse(packet, 200, request.method);
}

async function lensCompileResponse(request: Request, env: Env,
  operation: 'lens' | 'exploration' | 'temporal' = 'lens'): Promise<Response> {
  const contentType = ((request.headers.get("Content-Type") ?? "").split(";").at(0) ?? "").trim().toLowerCase();
  if (contentType !== "application/json") throw new HttpError(415, "lens request must use application/json");
  const declaredLength = request.headers.get("Content-Length");
  if (declaredLength !== null) {
    const normalizedLength = declaredLength.trim();
    if (!/^[0-9]+$/.test(normalizedLength)) throw new HttpError(400, "invalid lens request Content-Length");
    if (BigInt(normalizedLength) > BigInt(MAX_LENS_REQUEST_BYTES)) {
      throw new HttpError(413, `lens request must not exceed ${MAX_LENS_REQUEST_BYTES} bytes`);
    }
  }
  if (!request.body) throw new HttpError(400, "lens request body is required");
  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MAX_LENS_REQUEST_BYTES) {
      await reader.cancel();
      throw new HttpError(413, `lens request must not exceed ${MAX_LENS_REQUEST_BYTES} bytes`);
    }
    chunks.push(value);
  }
  if (size === 0) throw new HttpError(400, "lens request body is required");
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  // The maintained temporal route passes its generated shared Rust module.
  // Publication selection and exact retained bytes stay with the D1 reader.
  if (operation === 'temporal') {
    try { return withSecurity(await temporalSnapshotResponseD1(env.DB, temporalRuntime, bytes, request.signal)); }
    catch (error) {
      if (error instanceof NativeBudgetExceeded) throw new HttpError(413, error.message);
      if (error instanceof SelectedTemporalError) {
        const status = error.code === 'UnknownIdentifier' ? 404
          : error.code === 'StaleSelection' ? 409
          : error.code === 'BudgetExceeded' ? 413
          : error.code === 'InvalidRequest' || error.code === 'InvalidJson' ? 400 : 503;
        throw new HttpError(status, error.message);
      }
      throw error;
    }
  }
  if(operation==='lens')return withSecurity(await lensSnapshotResponseD1(env.DB,lensRuntime,bytes,'compile',request.signal,request.method));
  return withSecurity(await explorationSnapshotResponseD1(env.DB,explorationRuntime,bytes,request.signal,request.method));
}

async function apiResponse(request: Request, env: Env, url: URL): Promise<Response> {
  const path = url.pathname;
  const search = url.searchParams;
  const method = request.method;

  if (path === "/api/knowledge/search/capabilities") return jsonResponse(await knowledgeSearchCapabilitiesD1(env.DB,workerSearchControlsRuntime), 200, method);
  if (path === "/api/source/capabilities") return jsonResponse({
    schema_version: 'tos_source_read_capabilities_v1', available: false,
    issuer: 'Tree-of-Sophia/source-witnesses',
    layers: {metadata_record: [], claim_record: false, source_slot: false, authored_csv_record: false},
    source_epoch: null, representations: [],
    authority: {is_source: false, writes_to_source: false, grants_current_use: false, native_text_payload: false,
      note: 'D1 exposes derived source navigation only; a separately configured native PostgreSQL source-text gateway is reported below and does not make D1 a source reader.'},
    source_text_search: sourceTextGatewayCapability(env, request.url),
    status: 'unsupported', reason: 'source-owner-reader-not-configured',
  }, 200, method);
  if (path === "/api/knowledge/explore/capabilities") return jsonResponse(await explorationCapabilitiesD1(env.DB), 200, method);
  if (path === "/api/knowledge/explore/contracts") {
    const contracts = await staticItem(env, request, "knowledge/exploration-contracts.json");
    return jsonResponse({...contracts, capabilities: await explorationCapabilitiesD1(env.DB)}, 200, method);
  }

  if (path === "/health") {
    try {
      return jsonResponse(await buildHealth(env.DB), 200, method);
    } catch (error) {
      console.error("edge health failed", error);
      return jsonResponse(
        {
          service: "tree-of-sophia-access",
          ok: false,
          write_enabled: false,
          errors: ["Cloudflare read model is not ready"],
          runtime: "cloudflare-worker",
        },
        503,
        method,
      );
    }
  }

  if (path === "/api/zarathustra/word-analysis") {
    const query = (search.get("query") ?? "").trim();
    if (!query) throw new HttpError(400, "word-analysis query is required");
    if (query.length > 256) throw new HttpError(400, "word-analysis query exceeds 256 characters");
    const language = (search.get("language") || "ru").trim().toLowerCase();
    if (!new Set(["de", "ru", "en"]).has(language)) {
      throw new HttpError(400, `unsupported word-analysis language: ${language}`);
    }
    boundedInt(search.get("rank"), 1, 1, 100);
    booleanParam(search, "include_semantic_neighbors");
    return jsonResponse(await metaItem(env.DB, "word_analysis_capability"), 200, method);
  }
  if (path === "/api/source-gaps") return sourceGapResponse(request, env, search);
  const sourceNavigationPrefix = "/api/source/navigation/";
  if (path.startsWith(sourceNavigationPrefix)) {
    if (search.has("at_least_commit_seq")) {
      throw new HttpError(503, "unavailable: this D1 export has no comparable command sequence");
    }
    return jsonResponse(
      await sourceDescendD1(
        env.DB,
        segment(path, sourceNavigationPrefix),
        boundedInt(search.get("max_depth"), 8, 1, 8),
        boundedInt(search.get("limit"), 300, 1, 300),
      ),
      200,
      method,
    );
  }
  const sourceDossierPrefix = "/api/source/dossiers/";
  if (path.startsWith(sourceDossierPrefix)) {
    return jsonResponse(
      await sourceDossierD1(
        env.DB,
        segment(path, sourceDossierPrefix),
        boundedInt(search.get("limit"), 300, 1, 300),
      ),
      200,
      method,
    );
  }

  const fixedAssets: Record<string, string> = {
    "/api/corpus/status": "corpus/status.json",
    "/api/corpus/summary": "corpus/summary.json",
    "/api/knowledge/contracts": "knowledge/contracts.json",
    "/api/philosophy/status": "philosophy/status.json",
    "/api/philosophy/views": "philosophy/views.json",
    "/api/philosophy/layers": "philosophy/layers.json",
    "/api/philosophy/contracts": "philosophy/contracts.json",
    "/api/philosophy/snapshot": "philosophy/snapshot.json",
    "/api/philosophy/audit": "philosophy/audit.json",
  };
  const fixedAsset = fixedAssets[path];
  if (fixedAsset) return staticApi(env, request, fixedAsset);
  if (path === '/api/knowledge/catalog') return nativePacketResponse(await knowledgeCatalogD1(env.DB),200,method);

  if (path === "/api/knowledge/search") {
    // Match the Python HTTP adapter's list and integer parsing here; other
    // routes retain their independently owned input compatibility behavior.
    // parse_qs drops empty values before _single chooses the first value.
    const searchValue=(key:string)=>search.getAll(key).find(value=>value!=='')??null;
    const searchList=(key:string)=>(searchValue(key)??'').split(',').filter(Boolean);
    const searchInteger=(key:string,fallback:number,minimum:number,maximum:number)=>{
      const raw=searchValue(key);if(raw===null)return fallback;
      const parsed=nativeIntegerString(raw);
      // Python's default decimal-int conversion limit counts digits, not signs
      // or separators. A failed parse uses the transport's default then clamp.
      return Math.max(minimum,Math.min(maximum,Number.isNaN(parsed)||[...nativeStrip(raw).replace(/[_+-]/g,'')].length>4300?fallback:parsed));
    };
    const mode = searchValue("mode") ?? "legacy";
    if (mode === "indexed") {
      if (searchInteger('offset',0,0,100_000) !== 0) throw new HttpError(400, "indexed knowledge search uses cursor continuation, not offset");
      return nativePacketResponse(await knowledgeSearchD1Indexed(env.DB, {
        query: searchValue("query") ?? "",
        sources: searchList('sources').length ? searchList('sources') : null,
        kindIds: searchList('kind_ids'),
        predicateIds: searchList('predicate_ids'),
        cursor: searchValue("cursor"),
        limit: searchInteger('limit',40,1,100),
      },workerSearchControlsRuntime), 200, method);
    }
    if (mode !== "legacy") throw new HttpError(400, "knowledge search mode must be legacy or indexed");
    return nativePacketResponse(await knowledgeSearchD1(env.DB, {
      query: searchValue("query") ?? "",
      sources: searchList('sources').length ? searchList('sources') : null,
      kindIds: searchList('kind_ids'),
      predicateIds: searchList('predicate_ids'),
      offset: searchInteger('offset',0,0,100_000),
      limit: searchInteger('limit',40,1,100),
    },workerSearchControlsRuntime), 200, method);
  }
  const knowledgeNodePrefix = "/api/knowledge/nodes/";
  if (path.startsWith(knowledgeNodePrefix)) {
    return withSecurity(await inspectionSnapshotResponseD1(env.DB,inspectionRuntime,'node',
      segment(path,knowledgeNodePrefix),boundedInt(search.get("relation_limit"),200,0,1000),request.signal,method));
  }
  const knowledgeRelationPrefix = "/api/knowledge/relations/";
  if (path.startsWith(knowledgeRelationPrefix)) {
    return withSecurity(await inspectionSnapshotResponseD1(env.DB,inspectionRuntime,'relation',
      segment(path,knowledgeRelationPrefix),200,request.signal,method));
  }
  const knowledgeFocusPrefix = "/api/knowledge/focus/";
  if (path.startsWith(knowledgeFocusPrefix)) {
    const profile = search.get("profile") || "overview";
    const direction = (search.get("direction") || "either").trim().toLowerCase();
    const sources = listParam(search, "sources");
    const focus={node_id:segment(path,knowledgeFocusPrefix),
      ...(sources.length?{sources}:{}),depth:boundedInt(search.get("depth"),1,0,5),direction,profile,
      predicate_ids:listParam(search,"predicates"),node_limit:boundedInt(search.get("node_limit"),200,1,1000),
      relation_limit:boundedInt(search.get("relation_limit"),400,0,2000)};
    return withSecurity(await lensSnapshotResponseD1(env.DB,lensRuntime,new TextEncoder().encode(JSON.stringify(focus)),
      'focus',request.signal,method));
  }
  const knowledgeLensPrefix = "/api/knowledge/lenses/";
  if (path.startsWith(knowledgeLensPrefix)) {
    const lensId = segment(path, knowledgeLensPrefix);
    return withSecurity(await lensSnapshotResponseD1(env.DB,lensRuntime,new TextEncoder().encode(JSON.stringify(lensId)),
      'stored',request.signal,method));
  }


  if (path === "/api/philosophy/review-packet") {
    const viewId = (search.get("view_id") || "chronology").trim();
    return staticApi(env, request, `philosophy/review-packet/${encodeURIComponent(viewId)}.json`);
  }
  if (path === "/api/philosophy/unresolved") {
    const viewId = (search.get("view_id") ?? "").trim();
    return staticApi(env, request, `philosophy/unresolved/${viewId ? encodeURIComponent(viewId) : "all"}.json`);
  }

  if (path.startsWith("/api/philosophy/scale-export/")) {
    return scaleExportResponse(request, env.DB, url);
  }

  const corpusViewPrefix = "/api/corpus/graph-views/";
  if (path.startsWith(corpusViewPrefix)) {
    const viewId = segment(path, corpusViewPrefix);
    const limit = boundedInt(search.get("limit"), 100, 1, 1000);
    if (STATIC_CORPUS_LIMITS.has(limit)) {
      return staticApi(env, request, `corpus/graph-views/${encodeURIComponent(viewId)}/${limit}.json`);
    }
    return jsonResponse(await corpusGraphView(env.DB, viewId, limit), 200, method);
  }

  const philosophyViewPrefix = "/api/philosophy/views/";
  if (path.startsWith(philosophyViewPrefix)) {
    const viewId = segment(path, philosophyViewPrefix);
    const limit = boundedInt(search.get("limit"), 1000, 1, 1000);
    if (STATIC_PHILOSOPHY_LIMITS.has(limit)) {
      return staticApi(env, request, `philosophy/views/${encodeURIComponent(viewId)}/${limit}.json`);
    }
    return jsonResponse(await philosophyView(env.DB, viewId, limit), 200, method);
  }

  if (path === "/api/corpus/search") {
    return jsonResponse(await corpusSearch(env.DB, search.get("query") ?? "", boundedInt(search.get("limit"), 20, 1, 100), search.get("resource_kind")), 200, method);
  }
  if (path === "/api/philosophy/search") {
    return jsonResponse(await philosophySearch(env.DB, search.get("query") ?? "", boundedInt(search.get("limit"), 40, 1, 100)), 200, method);
  }
  if (path === "/api/philosophy/clusters") {
    return jsonResponse(
      await philosophyClusters(
        env.DB,
        search.get("view_id") || null,
        search.get("kind") || null,
        boundedInt(search.get("limit"), 80, 1, 1000),
      ),
      200,
      method,
    );
  }
  if (path === "/api/philosophy/packet") {
    return jsonResponse(
      await philosophyPacket(
        env.DB,
        search.get("query") ?? "",
        search.get("view_id") || null,
        boundedInt(search.get("limit"), 20, 1, 100),
      ),
      200,
      method,
    );
  }

  const corpusNodePrefix = "/api/corpus/nodes/";
  if (path.startsWith(corpusNodePrefix)) {
    return jsonResponse(await corpusNodePacket(env.DB, segment(path, corpusNodePrefix)), 200, method);
  }
  const corpusPackPrefix = "/api/corpus/relation-packs/";
  if (path.startsWith(corpusPackPrefix)) {
    return jsonResponse(await corpusRelationPack(env.DB, segment(path, corpusPackPrefix)), 200, method);
  }
  const philosophyNodePrefix = "/api/philosophy/nodes/";
  if (path.startsWith(philosophyNodePrefix)) {
    return jsonResponse(await philosophyNodePacket(env.DB, segment(path, philosophyNodePrefix)), 200, method);
  }
  const philosophyEdgePrefix = "/api/philosophy/edges/";
  if (path.startsWith(philosophyEdgePrefix)) {
    return jsonResponse(await philosophyEdgePacket(env.DB, segment(path, philosophyEdgePrefix)), 200, method);
  }

  const queryNeighborhoodPrefix = "/api/philosophy/query/neighborhood/";
  const neighborhoodPrefix = "/api/philosophy/neighborhood/";
  if (path.startsWith(queryNeighborhoodPrefix) || path.startsWith(neighborhoodPrefix)) {
    const prefix = path.startsWith(queryNeighborhoodPrefix) ? queryNeighborhoodPrefix : neighborhoodPrefix;
    const packet = await philosophyNeighborhood(
      env.DB,
      segment(path, prefix),
      boundedInt(search.get("depth"), 1, 1, 3),
      listParam(search, "layers"),
      listParam(search, "predicates"),
      boundedInt(search.get("limit"), 80, 1, 300),
    );
    if (prefix === queryNeighborhoodPrefix) packet.query_backend = "d1";
    return jsonResponse(packet, 200, method);
  }

  if (path === "/api/philosophy/query/paths" || path === "/api/philosophy/paths") {
    const packet = await philosophyPath(env.DB, {
      fromId: required(search, "from"),
      toId: required(search, "to"),
      layers: listParam(search, "layers"),
      predicates: listParam(search, "predicates"),
      maxDepth: boundedInt(search.get("max_depth"), 6, 1, 8),
      direction: (search.get("direction") || "outgoing").trim().toLowerCase(),
      viewId: search.get("view_id") || null,
      excludedEdgeIds: listParam(search, "exclude"),
      alternativeLimit: boundedInt(search.get("alternatives"), 1, 1, 5),
    });
    if (path === "/api/philosophy/query/paths") packet.query_backend = "d1";
    return jsonResponse(packet, 200, method);
  }

  const philosophyEpistemicPrefix = "/api/philosophy/query/epistemic/";
  if (path.startsWith(philosophyEpistemicPrefix)) {
    return jsonResponse(
      await evidenceLens(
        env.DB,
        "philosophy",
        segment(path, philosophyEpistemicPrefix),
        search.get("view_id") || null,
        boundedInt(search.get("limit"), 80, 1, 200),
      ),
      200,
      method,
    );
  }
  const corpusEpistemicPrefix = "/api/corpus/query/epistemic/";
  if (path.startsWith(corpusEpistemicPrefix)) {
    return jsonResponse(
      await evidenceLens(
        env.DB,
        "corpus",
        segment(path, corpusEpistemicPrefix),
        search.get("view_id") || "route-graph",
        boundedInt(search.get("limit"), 80, 1, 200),
      ),
      200,
      method,
    );
  }

  throw new HttpError(404, "not found");
}

export default {
  async fetch(request, env): Promise<Response> {
    const url = new URL(request.url);
    if (url.hostname.toLowerCase() === "www.treeofsophia.com") {
      url.hostname = "treeofsophia.com";
      return Response.redirect(url.toString(), 308);
    }
    if (request.method === "POST" && ["/api/knowledge/lenses/compile", "/api/knowledge/explore", "/api/knowledge/temporal/compare"].includes(url.pathname)) {
      try {
        const operation = url.pathname === '/api/knowledge/explore' ? 'exploration'
          : url.pathname === '/api/knowledge/temporal/compare' ? 'temporal' : 'lens';
        return await lensCompileResponse(request, env, operation);
      } catch (error) {
        if (error instanceof HttpError) return jsonResponse({ error: error.message }, error.status, request.method);
        console.error("Cloudflare edge lens request failed", error);
        return jsonResponse({ error: "Cloudflare edge lens request failed" }, 500, request.method);
      }
    }
    if (request.method === "POST" && url.pathname === "/api/source-text/search") {
      return sourceTextSearchGatewayResponse(request, env);
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      return jsonResponse({ error: "standalone access is read-only" }, 405, request.method);
    }
    const prototypeAsset = publicPrototypeAsset(url.pathname);
    if (prototypeAsset) {
      const assetUrl = new URL(prototypeAsset, request.url);
      const response = withReaderSecurity(await env.ASSETS.fetch(new Request(assetUrl, request)));
      return withWebAnalytics(request, response, env.WEB_ANALYTICS_TOKEN);
    }
    if (url.pathname === "/health" || url.pathname.startsWith("/api/")) {
      try {
        return await apiResponse(request, env, url);
      } catch (error) {
        if (error instanceof NativeBudgetExceeded && (['/api/knowledge/search','/api/knowledge/catalog'].includes(url.pathname)||['/api/knowledge/focus/','/api/knowledge/lenses/','/api/knowledge/nodes/','/api/knowledge/relations/'].some(prefix => url.pathname.startsWith(prefix)))) {
          return jsonResponse({error: error.message}, 413, request.method);
        }
        if (error instanceof HttpError || error instanceof SourceNavigationError) {
          return jsonResponse({ error: error.message }, error.status, request.method);
        }
        console.error("Cloudflare edge request failed", error);
        return jsonResponse({ error: "Cloudflare edge request failed" }, 500, request.method);
      }
    }
    return withWebAnalytics(request, withSecurity(await env.ASSETS.fetch(request)), env.WEB_ANALYTICS_TOKEN);
  },
} satisfies ExportedHandler<Env>;

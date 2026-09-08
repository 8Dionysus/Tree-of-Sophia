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
  executeKnowledgeLensD1,
  focusKnowledgeNodeD1,
  knowledgeNodeD1,
  knowledgeRelationD1,
  knowledgeSearchD1,
} from "./knowledge-store";
import { SourceNavigationError, sourceDescend, sourceDossier } from "./source-navigation";
import { metaItem } from "./store";
import { KnowledgeRevisionConflict } from "./lens-pagination";
import { exploreD1, explorationCapabilitiesD1 } from "./exploration";

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

async function sourceGapResponse(request: Request, env: Env, search: URLSearchParams): Promise<Response> {
  const query = (search.get("query") ?? "").trim();
  if (query.length > 256) throw new HttpError(400, "source-gap query exceeds 256 characters");
  const limit = boundedInt(search.get("limit"), 20, 1, 100);
  const assetUrl = new URL("/__edge/source-gaps/all.json", request.url);
  const asset = await env.ASSETS.fetch(new Request(assetUrl));
  if (!asset.ok) throw new Error("generated source-gap asset is missing");
  const packet = await asset.json() as Record<string, unknown>;
  const needle = query.toLocaleLowerCase();
  const all = Array.isArray(packet.gaps) ? packet.gaps as Array<Record<string, unknown>> : [];
  const gaps = all.filter((gap) => !needle || JSON.stringify(gap).toLocaleLowerCase().includes(needle)).slice(0, limit);
  return jsonResponse({ ...packet, query, result_count: gaps.length, gaps }, 200, request.method);
}

async function sourceNavigationPayload(request: Request, env: Env): Promise<Item> {
  return staticItem(env, request, "source-navigation/all.json");
}

async function lensCompileResponse(request: Request, env: Env, exploration = false): Promise<Response> {
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
  let spec: unknown;
  try {
    spec = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(bytes));
  } catch (error) {
    throw new HttpError(400, `invalid LensSpec JSON: ${error instanceof Error ? error.message : "decode failed"}`);
  }
  if (!spec || typeof spec !== "object" || Array.isArray(spec)) throw new HttpError(400, "lens spec must be an object");
  try {
    return jsonResponse(await (exploration ? exploreD1(env.DB, spec) : executeKnowledgeLensD1(env.DB, spec)), 200, request.method);
  } catch (error) {
    if (error instanceof KnowledgeRevisionConflict) throw error;
    if (error instanceof HttpError) throw error;
    if (!exploration && error instanceof Error) throw new HttpError(400, error.message);
    throw error;
  }
}

async function apiResponse(request: Request, env: Env, url: URL): Promise<Response> {
  const path = url.pathname;
  const search = url.searchParams;
  const method = request.method;

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
    return jsonResponse(
      sourceDescend(
        await sourceNavigationPayload(request, env),
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
      sourceDossier(
        await sourceNavigationPayload(request, env),
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
    "/api/knowledge/catalog": "knowledge/catalog.json",
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

  if (path === "/api/knowledge/search") {
    return jsonResponse(await knowledgeSearchD1(env.DB, {
      query: search.get("query") ?? "",
      sources: listParam(search, "sources").length ? listParam(search, "sources") : null,
      kindIds: listParam(search, "kind_ids"),
      predicateIds: listParam(search, "predicate_ids"),
      offset: boundedInt(search.get("offset"), 0, 0, 100_000),
      limit: boundedInt(search.get("limit"), 40, 1, 100),
    }), 200, method);
  }
  const knowledgeNodePrefix = "/api/knowledge/nodes/";
  if (path.startsWith(knowledgeNodePrefix)) {
    return jsonResponse(
      await knowledgeNodeD1(env.DB, segment(path, knowledgeNodePrefix), boundedInt(search.get("relation_limit"), 200, 0, 1000)),
      200,
      method,
    );
  }
  const knowledgeRelationPrefix = "/api/knowledge/relations/";
  if (path.startsWith(knowledgeRelationPrefix)) {
    return jsonResponse(await knowledgeRelationD1(env.DB, segment(path, knowledgeRelationPrefix)), 200, method);
  }
  const knowledgeFocusPrefix = "/api/knowledge/focus/";
  if (path.startsWith(knowledgeFocusPrefix)) {
    const profile = search.get("profile") || "overview";
    if (profile !== "overview" && profile !== "all") throw new HttpError(400, "profile must be overview or all");
    const direction = (search.get("direction") || "either").trim().toLowerCase();
    if (direction !== "outgoing" && direction !== "incoming" && direction !== "either") {
      throw new HttpError(400, "direction must be outgoing, incoming, or either");
    }
    const sources = listParam(search, "sources");
    return jsonResponse(await focusKnowledgeNodeD1(env.DB, segment(path, knowledgeFocusPrefix), {
      ...(sources.length ? { sources } : {}),
      depth: boundedInt(search.get("depth"), 1, 0, 5),
      direction,
      profile,
      predicateIds: listParam(search, "predicates"),
      nodeLimit: boundedInt(search.get("node_limit"), 200, 1, 1000),
      relationLimit: boundedInt(search.get("relation_limit"), 400, 0, 2000),
    }), 200, method);
  }
  const knowledgeLensPrefix = "/api/knowledge/lenses/";
  if (path.startsWith(knowledgeLensPrefix)) {
    const lensId = segment(path, knowledgeLensPrefix);
    const catalog = await staticItem(env, request, "knowledge/catalog.json");
    const lenses = Array.isArray(catalog.lenses) ? catalog.lenses : [];
    const spec = lenses.find((item) => item && typeof item === "object" && !Array.isArray(item) && (item as Item).lens_id === lensId);
    if (!spec) throw new HttpError(404, `unknown ToS knowledge lens: ${lensId}`);
    return jsonResponse(await executeKnowledgeLensD1(env.DB, spec), 200, method);
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
    return jsonResponse(await corpusSearch(env.DB, search.get("query") ?? "", boundedInt(search.get("limit"), 20, 1, 100)), 200, method);
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
    if (request.method === "POST" && ["/api/knowledge/lenses/compile", "/api/knowledge/explore"].includes(url.pathname)) {
      try {
        return await lensCompileResponse(request, env, url.pathname === "/api/knowledge/explore");
      } catch (error) {
        if (error instanceof KnowledgeRevisionConflict) return jsonResponse({ error: error.message }, 409, request.method);
        if (error instanceof HttpError) return jsonResponse({ error: error.message }, error.status, request.method);
        console.error("Cloudflare edge lens request failed", error);
        return jsonResponse({ error: "Cloudflare edge lens request failed" }, 500, request.method);
      }
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      return jsonResponse({ error: "standalone access is read-only" }, 405, request.method);
    }
    if (url.pathname === "/health" || url.pathname.startsWith("/api/")) {
      try {
        return await apiResponse(request, env, url);
      } catch (error) {
        if (error instanceof HttpError || error instanceof SourceNavigationError) {
          return jsonResponse({ error: error.message }, error.status, request.method);
        }
        console.error("Cloudflare edge request failed", error);
        return jsonResponse({ error: "Cloudflare edge request failed" }, 500, request.method);
      }
    }
    return withSecurity(await env.ASSETS.fetch(request));
  },
} satisfies ExportedHandler<Env>;

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
import { metaItem } from "./store";

const STATIC_CORPUS_LIMITS = new Set([1, 100, 700, 1000]);
const STATIC_PHILOSOPHY_LIMITS = new Set([1, 1000]);

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

async function apiResponse(request: Request, env: Env, url: URL): Promise<Response> {
  const path = url.pathname;
  const search = url.searchParams;
  const method = request.method;

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

  const fixedAssets: Record<string, string> = {
    "/api/corpus/status": "corpus/status.json",
    "/api/corpus/summary": "corpus/summary.json",
    "/api/philosophy/status": "philosophy/status.json",
    "/api/philosophy/views": "philosophy/views.json",
    "/api/philosophy/layers": "philosophy/layers.json",
    "/api/philosophy/contracts": "philosophy/contracts.json",
    "/api/philosophy/snapshot": "philosophy/snapshot.json",
    "/api/philosophy/audit": "philosophy/audit.json",
  };
  const fixedAsset = fixedAssets[path];
  if (fixedAsset) return staticApi(env, request, fixedAsset);

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
    if (request.method !== "GET" && request.method !== "HEAD") {
      return jsonResponse({ error: "standalone access is read-only" }, 405, request.method);
    }
    if (url.pathname === "/health" || url.pathname.startsWith("/api/")) {
      try {
        return await apiResponse(request, env, url);
      } catch (error) {
        if (error instanceof HttpError) return jsonResponse({ error: error.message }, error.status, request.method);
        console.error("Cloudflare edge request failed", error);
        return jsonResponse({ error: "Cloudflare edge request failed" }, 500, request.method);
      }
    }
    return withSecurity(await env.ASSETS.fetch(request));
  },
} satisfies ExportedHandler<Env>;

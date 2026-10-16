import { HttpError, type Item, parseItem, stringValue } from "./common.ts";

type JsonRow = { json: string };
type ChunkRow = { id: string; json: string };
type MetaChunkRow = { json_chunk: string };

export type EdgeRow = {
  id: string;
  ord: number;
  from_id: string;
  to_id: string;
  predicate_id: string;
  view_mask: number;
  layer_mask: number;
  json?: string;
};

export async function rows<T>(db: D1Database, sql: string, ...bindings: unknown[]): Promise<T[]> {
  const result = await db.prepare(sql).bind(...bindings).all<T>();
  return result.results;
}

export async function jsonRows(db: D1Database, sql: string, ...bindings: unknown[]): Promise<Item[]> {
  return (await rows<JsonRow>(db, sql, ...bindings)).map((row) => parseItem(row.json));
}

export async function meta<T = Item>(db: D1Database, key: string): Promise<T> {
  const chunks = await rows<MetaChunkRow>(
    db,
    "SELECT json_chunk FROM edge_meta WHERE key = ? ORDER BY part",
    key,
  );
  if (chunks.length === 0) throw new Error(`edge read model is missing metadata: ${key}`);
  return JSON.parse(chunks.map((row) => row.json_chunk).join("")) as T;
}

const PUBLIC_D1_PROFILE = "tos-rust-public-d1-full-v2";
const PUBLIC_D1_PROFILE_SCHEMA = "tos_public_d1_producer_profile_v1";

/** Refuse new source-value/source-gap semantics when D1 carries an older build. */
export async function requirePublicD1ProducerProfile(db: D1Database): Promise<void> {
  const unavailable = (): never => {
    throw new HttpError(503, `selected D1 data requires producer profile ${PUBLIC_D1_PROFILE}`);
  };
  type ProfileRow = { key: string; part: number; json_chunk: string | null; chunk_bytes: number | null };
  let selected: ProfileRow[];
  try {
    selected = await rows<ProfileRow>(
      db,
      `SELECT key, part,
              CASE WHEN typeof(json_chunk)='text' AND length(CAST(json_chunk AS BLOB))<=4096 THEN json_chunk ELSE NULL END AS json_chunk,
              length(CAST(json_chunk AS BLOB)) AS chunk_bytes
         FROM edge_meta
        WHERE key IN ('data_revision','public_d1_producer_profile')
        ORDER BY key, part LIMIT 3`,
    );
  } catch {
    return unavailable();
  }
  if (selected.length !== 2 || selected.some((row) => row.part !== 0
    || row.json_chunk === null || !Number.isSafeInteger(row.chunk_bytes)
    || row.chunk_bytes! <= 0 || row.chunk_bytes! > 4096)) return unavailable();
  let revision: Item;
  let profile: Item;
  try {
    revision = JSON.parse(selected.find((row) => row.key === "data_revision")!.json_chunk!) as Item;
    profile = JSON.parse(selected.find((row) => row.key === "public_d1_producer_profile")!.json_chunk!) as Item;
  } catch {
    return unavailable();
  }
  const dataRevision = revision.sha256;
  if (typeof dataRevision !== "string" || !/^[0-9a-f]{64}$/.test(dataRevision)
    || profile.schema !== PUBLIC_D1_PROFILE_SCHEMA
    || profile.producer !== PUBLIC_D1_PROFILE
    || profile.read_model_schema !== "tos_cloudflare_edge_read_model_v9"
    || profile.data_revision !== dataRevision) return unavailable();
}

export async function metaItem(db: D1Database, key: string): Promise<Item> {
  return meta<Item>(db, key);
}

export async function positions(db: D1Database, key: "view_positions" | "layer_positions"): Promise<Record<string, number>> {
  return meta<Record<string, number>>(db, key);
}

export function maskFor(filters: string[], mapping: Record<string, number>): number | null {
  if (filters.length === 0) return null;
  return filters.reduce((mask, value) => {
    const position = mapping[value];
    return position === undefined ? mask : mask | (1 << position);
  }, 0);
}

export async function viewMask(db: D1Database, viewId: string | null): Promise<number | null> {
  if (!viewId) return null;
  const mapping = await positions(db, "view_positions");
  const position = mapping[viewId];
  if (position === undefined) throw new HttpError(404, `unknown ToS philosophy graph view: ${viewId}`);
  return 1 << position;
}

export async function philosophyNode(db: D1Database, id: string): Promise<Item | null> {
  const row = await db.prepare("SELECT json FROM philosophy_nodes WHERE id = ?").bind(id).first<JsonRow>();
  return row ? parseItem(row.json) : null;
}

export async function philosophyEdge(db: D1Database, id: string): Promise<Item | null> {
  const row = await db.prepare("SELECT json FROM philosophy_edges WHERE id = ?").bind(id).first<JsonRow>();
  return row ? parseItem(row.json) : null;
}

export async function itemsByIds(
  db: D1Database,
  table: "philosophy_nodes" | "philosophy_edges",
  ids: string[],
): Promise<Item[]> {
  if (ids.length === 0) return [];
  return jsonRows(
    db,
    `SELECT json FROM ${table} WHERE id IN (SELECT value FROM json_each(?)) ORDER BY ord`,
    JSON.stringify(ids),
  );
}

export async function fullClusters(
  db: D1Database,
  options: { viewMask?: number | null; kind?: string | null; limit?: number } = {},
): Promise<Item[]> {
  const filterView = options.viewMask ?? null;
  const kind = options.kind ?? null;
  const limit = options.limit ?? 1000;
  const result = await rows<ChunkRow>(
    db,
    `SELECT id, GROUP_CONCAT(json_chunk, '') AS json
       FROM (
         SELECT id, json_chunk
           FROM philosophy_clusters
          WHERE (? IS NULL OR (view_mask & ?) != 0)
            AND (? IS NULL OR substr(sort_key, 1, instr(sort_key, '␟') - 1) = ?)
          ORDER BY sort_key, id, part
       )
      GROUP BY id
      LIMIT ?`,
    filterView,
    filterView,
    kind,
    kind,
    limit,
  );
  return result.map((row) => parseItem(row.json));
}

export async function compactView(db: D1Database, viewId: string): Promise<Item> {
  const row = await db
    .prepare("SELECT json FROM philosophy_aux WHERE collection = 'views' AND id = ?")
    .bind(viewId)
    .first<JsonRow>();
  if (!row) throw new HttpError(404, `unknown ToS philosophy graph view: ${viewId}`);
  return parseItem(row.json);
}

export async function corpusItem(db: D1Database, collection: string, id: string): Promise<Item[]> {
  return jsonRows(
    db,
    "SELECT json FROM corpus_items WHERE collection = ? AND id = ? ORDER BY ord",
    collection,
    id,
  );
}

export function rowJson(row: EdgeRow): Item {
  if (typeof row.json !== "string") throw new Error(`edge ${stringValue(row.id)} was fetched without JSON`);
  return parseItem(row.json);
}

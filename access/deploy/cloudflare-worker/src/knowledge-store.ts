import { HttpError, parseItem, type Item } from "./common.ts";
import {
  focusLensSpec,
  type FocusKnowledgeOptions,
  type KnowledgeNode,
  type KnowledgeRelation,
} from "./knowledge.ts";
import { jsonRows, meta, rows } from "./store.ts";
import {executeNativeLensD1} from './native-lens-store.ts';
import {parseNativeJson, type NativeRef, type NativeLensResult} from './native-lens.ts';
import { compareTemporalOperands, normalizeTemporalComparisonRequest, temporalNodeFromJson } from './temporal-comparison.ts';

const KNOWLEDGE_SOURCES = new Set(["philosophy", "canon", "candidate-intake", "source-navigation", "source-claims", "semantic-interchange", "repository"]);
const PAGE_SIZE = 2000;
const SEARCH_NGRAM_SIZE = 3;
const SEARCH_MAX_CANDIDATES = 50_000;
const SEARCH_MAX_VERIFY_CHARS = 16_000_000;
const SEARCH_CURSOR_SCHEMA = "tos_knowledge_search_indexed_cursor_v2";

// A data_revision digest is not a publication identity: an import can move
// A -> B -> A while retaining the same bytes at the end.  The additive
// exploration clock is advanced by the maintenance publication triggers (and
// by the builder's identical-data bootstrap), so the pair below is the
// request's read-model identity.  Keep the metadata read as one SQL statement
// so a guard never combines a clock from one D1 read with a revision from
// another read.
const KNOWLEDGE_SNAPSHOT_SQL = `
SELECT
  (SELECT COUNT(*) FROM knowledge_exploration_clock) AS clock_rows,
  (SELECT COUNT(*) FROM knowledge_exploration_clock WHERE singleton = 1) AS singleton_rows,
  (SELECT MIN(epoch) FROM knowledge_exploration_clock WHERE singleton = 1) AS epoch_min,
  (SELECT MAX(epoch) FROM knowledge_exploration_clock WHERE singleton = 1) AS epoch_max,
  (SELECT GROUP_CONCAT(json_chunk, '') FROM (
    SELECT json_chunk FROM edge_meta WHERE key = 'data_revision' ORDER BY part
  )) AS data_revision_json,
  (SELECT COUNT(*) FROM edge_meta WHERE key = 'data_revision') AS data_revision_parts,
  (SELECT COUNT(DISTINCT part) FROM edge_meta WHERE key = 'data_revision') AS data_revision_distinct_parts,
  (SELECT MIN(part) FROM edge_meta WHERE key = 'data_revision') AS data_revision_min_part,
  (SELECT MAX(part) FROM edge_meta WHERE key = 'data_revision') AS data_revision_max_part,
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

async function consistentRead<T>(
  db: D1Database,
  read: (snapshot: KnowledgeSnapshot) => Promise<T>,
): Promise<T> {
  const before = await knowledgeSnapshot(db);
  const result = await read(before);
  const after = await knowledgeSnapshot(db);
  if (!sameKnowledgeSnapshot(before, after)) {
    throw new HttpError(409, "knowledge snapshot changed during query; retry against the current revision");
  }
  return result;
}

export async function executeKnowledgeLensD1(db: D1Database, specValue: NativeRef): Promise<NativeLensResult> {
  return consistentRead(db, snapshot => executeNativeLensD1(db, specValue, {}, snapshot.revision));
}

export async function knowledgeSearchD1(db: D1Database, options: Parameters<typeof knowledgeSearchD1Unchecked>[1]): Promise<Item> {
  return consistentRead(db, () => knowledgeSearchD1Unchecked(db, options));
}

export async function knowledgeSearchD1Indexed(
  db: D1Database,
  options: Parameters<typeof knowledgeSearchD1IndexedUnchecked>[1],
): Promise<Item> {
  return consistentRead(db, (snapshot) => knowledgeSearchD1IndexedUnchecked(db, options, snapshot));
}

export async function knowledgeNodeD1(db: D1Database, id: string, relationLimit: number): Promise<Item> {
  return consistentRead(db, () => knowledgeNodeD1Unchecked(db, id, relationLimit));
}

export async function knowledgeRelationD1(db: D1Database, id: string): Promise<Item> {
  return consistentRead(db, () => knowledgeRelationD1Unchecked(db, id));
}

export async function knowledgeTemporalCompareD1(db: D1Database, request: unknown): Promise<Item> {
  const normalized = normalizeTemporalComparisonRequest(request);
  return consistentRead(db, async () => {
    const top = await meta<Item>(db, 'knowledge_top');
    return compareTemporalOperands(top.source_revision, normalized, async identifier =>
      (await rows<{ json: string }>(db, 'SELECT json FROM knowledge_nodes WHERE id = ? LIMIT 2', identifier))
        .map(row => temporalNodeFromJson(row.json)));
  });
}

type SqlFragment = { sql: string; bindings: unknown[] };
type JsonRow = { json: string };
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

function displayFieldCondition(alias: string, field: string, mode: "exact" | "prefix" | "contains"): string {
  const object = `CASE WHEN json_type(${alias}.json, '$.display.${field}') = 'object' THEN json_extract(${alias}.json, '$.display.${field}') ELSE '{}' END`;
  const value = "lower(CAST(display_value.value AS TEXT))";
  const predicate = mode === "exact" ? `${value} = ?`
    : mode === "prefix" ? `instr(${value}, ?) = 1`
    : `instr(${value}, ?) > 0`;
  return `EXISTS (SELECT 1 FROM json_each(${object}) AS display_value WHERE display_value.type = 'text' AND ${predicate})`;
}

function searchRank(
  alias: string,
  identityFields: string[],
  visibleFields: string[],
  needle: string,
): {sql: string; bindings: string[]} {
  const exact = [`lower(${alias}.id) = ?`, `lower(${alias}.native_id) = ?`, ...identityFields.map(field => displayFieldCondition(alias, field, "exact"))];
  const prefix = [`instr(lower(${alias}.id), ?) = 1`, `instr(lower(${alias}.native_id), ?) = 1`, ...identityFields.map(field => displayFieldCondition(alias, field, "prefix"))];
  const visible = visibleFields.map(field => displayFieldCondition(alias, field, "contains"));
  return {
    sql: `CASE WHEN ${exact.join(" OR ")} THEN 0 WHEN ${prefix.join(" OR ")} THEN 1 WHEN ${visible.join(" OR ")} THEN 2 ELSE 3 END, ${alias}.id`,
    bindings: [
      ...Array(exact.length).fill(needle),
      ...Array(prefix.length).fill(needle),
      ...Array(visible.length).fill(needle),
    ],
  };
}

function indexedCursorEncode(value: Item): string {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

function indexedCursorDecode(value: string): Item {
  if (!value || value.length > 8192) throw new HttpError(400, "invalid indexed knowledge search cursor");
  try {
    const padded = value.replaceAll("-", "+").replaceAll("_", "/") + "=".repeat((4 - (value.length % 4)) % 4);
    const binary = atob(padded);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    const parsed: unknown = JSON.parse(new TextDecoder().decode(bytes));
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("not an object");
    if ((parsed as Item).schema !== SEARCH_CURSOR_SCHEMA) throw new Error("wrong cursor schema");
    return parsed as Item;
  } catch {
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
  rows: Item[];
  nextCursor: string | null;
  hasMore: boolean;
  work: { candidate_rows: number; verified_chars: number; sql_pages: number; selection_rows_read?: number };
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
    && JSON.stringify(value) === JSON.stringify(expected);
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
): Promise<IndexedPage> {
  const query = options.query.trim();
  if (query.length > 256) throw new HttpError(400, "knowledge search query exceeds 256 characters");
  const needle = query.toLowerCase();
  if ([...needle].length < SEARCH_NGRAM_SIZE) {
    throw new HttpError(400, "indexed knowledge search requires a query of at least three characters");
  }
  const sources = normalizedSources(options.sources);
  const filters = {
    sources: [...new Set(sources)].sort(),
    kind_ids: [...new Set(options.kindIds)].sort(),
    predicate_ids: [...new Set(options.predicateIds)].sort(),
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
      || decoded.id !== decoded.id.toLowerCase()
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
      "SELECT postings FROM knowledge_search_gram_stats WHERE kind=? AND n=? AND gram=?",
    ).bind(kind, SEARCH_NGRAM_SIZE, gram).first<{ postings: number }>();
    return { gram, postings: Number(row?.postings ?? 0) };
  }));
  const selected = stats.reduce((best, candidate) => candidate.postings < best.postings ? candidate : best);
  if (selected.postings === 0) return { rows: [], nextCursor: null, hasMore: false, work: { candidate_rows: 0, verified_chars: 0, sql_pages: grams.length } };
  if (selected.postings > SEARCH_MAX_CANDIDATES) {
    throw new HttpError(413, "indexed knowledge search candidate budget exceeded; narrow the query or use the legacy route");
  }
  const rankExpression = indexedRankExpression("s");
  const baseTable = kind === "nodes" ? "knowledge_nodes" : "knowledge_relations";
  const filterSql: string[] = ["g.kind = ?", "g.n = ?", "g.gram = ?"];
  const filterBindings: unknown[] = [kind, SEARCH_NGRAM_SIZE, selected.gram];
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
  const preflightSql = `SELECT COUNT(*) AS candidate_rows, COALESCE(SUM(s.document_chars), 0) AS verified_chars
    FROM knowledge_search_grams g
    JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
    WHERE ${filterSql.join(" AND ")}`;
  let preflight;
  try {
    // This is deliberately a carrier-only budget gate. Do not evaluate the
    // rank expression/continuation (which walks JSON rank fields) until the
    // total candidate-document budget is known to be bounded.
    preflight = await db.prepare(preflightSql).bind(...filterBindings)
      .all<{ candidate_rows: number; verified_chars: number }>();
  } catch (error) {
    throw new HttpError(503, `indexed knowledge search read model is unavailable: ${String(error)}`);
  }
  const aggregate = preflight.results[0];
  const candidateRows = Number(aggregate?.candidate_rows ?? 0);
  const verifiedChars = Number(aggregate?.verified_chars ?? 0);
  if (
    !Number.isSafeInteger(candidateRows)
    || candidateRows < 0
    || !Number.isSafeInteger(verifiedChars)
    || verifiedChars < 0
  ) {
    throw new HttpError(503, "indexed knowledge search carrier has invalid document budgets");
  }
  const preflightRowsRead = indexedRowsRead(preflight.meta?.rows_read);
  if (verifiedChars > SEARCH_MAX_VERIFY_CHARS) {
    throw new HttpError(413, "indexed knowledge search verification budget exceeded; narrow the query or use the legacy route");
  }
  if (candidateRows === 0) {
    return {
      rows: [],
      nextCursor: null,
      hasMore: false,
      work: {
        candidate_rows: 0,
        verified_chars: 0,
        sql_pages: grams.length + 1,
        ...(preflightRowsRead === undefined ? {} : { selection_rows_read: preflightRowsRead }),
      },
    };
  }
  const sql = `SELECT b.json, s.position, ${rankExpression} AS search_rank
    FROM knowledge_search_grams g
    JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
    JOIN ${baseTable} b ON b.id=s.id
    WHERE ${filterSql.join(" AND ")} AND instr(b.search_text, ?) > 0${continuationSql}
    ORDER BY search_rank, s.id_lower, s.position LIMIT ?`;
  const bindings: unknown[] = [...indexedRankBindings(needle), ...filterBindings, needle, ...continuationBindings, options.limit + 1];
  let result;
  try {
    result = await db.prepare(sql).bind(...bindings).all<{ json: string; position: number; search_rank: number }>();
  } catch (error) {
    throw new HttpError(503, `indexed knowledge search read model is unavailable: ${String(error)}`);
  }
  const selectedRows = result.results.slice(0, options.limit);
  const rowsValue = selectedRows.map((row) => parseItem(row.json));
  const hasMore = result.results.length > options.limit;
  const last = selectedRows[selectedRows.length - 1];
  const nextCursor = hasMore && last
    ? indexedCursorEncode({ schema: SEARCH_CURSOR_SCHEMA, source_revision: sourceRevision, snapshot_epoch: snapshotEpoch, kind, query: needle, filters, rank: last.search_rank, id: String(rowsValue.at(-1)?.id ?? "").toLowerCase(), position: last.position })
    : null;
  const resultRowsRead = indexedRowsRead(result.meta?.rows_read);
  const rowsRead = preflightRowsRead === undefined || resultRowsRead === undefined
    ? undefined
    : preflightRowsRead + resultRowsRead;
  return {
    rows: rowsValue,
    nextCursor,
    hasMore,
    work: {
      candidate_rows: candidateRows,
      verified_chars: verifiedChars,
      sql_pages: grams.length + 2,
      // This excludes the independent gram-stat, metadata, and consistency
      // reads; it is not a total query-cost counter.
      ...(rowsRead === undefined ? {} : { selection_rows_read: rowsRead }),
    },
  };
}

async function knowledgeSearchD1IndexedUnchecked(
  db: D1Database,
  options: IndexedSearchOptions,
  snapshot: KnowledgeSnapshot,
): Promise<Item> {
  const query = options.query.trim();
  if (query.length > 256) throw new HttpError(400, "knowledge search query exceeds 256 characters");
  const needle = query.toLowerCase();
  const limit = bounded(options.limit, "limit", 1, 100);
  if (options.kindIds.length > 100 || options.predicateIds.length > 100) {
    throw new HttpError(400, "knowledge search kind and predicate filters must contain at most 100 values");
  }
  const knowledgeTop = await meta<Item>(db, "knowledge_top");
  const sourceRevision = String(knowledgeTop.source_revision ?? "");
  const filters: IndexedFilters = {
    sources: [...new Set(normalizedSources(options.sources))].sort(),
    kind_ids: [...new Set(options.kindIds)].sort(),
    predicate_ids: [...new Set(options.predicateIds)].sort(),
  };
  const decodedCursor = options.cursor ? indexedCursorDecode(options.cursor) : null;
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
  const [nodes, relations] = await Promise.all([
    nodeExhausted
      ? Promise.resolve(emptyPage())
      : indexedKindPage(db, {...options, limit}, "nodes", nodeCursor, sourceRevision, snapshot.epoch),
    relationExhausted
      ? Promise.resolve(emptyPage())
      : indexedKindPage(db, {...options, limit}, "relations", relationCursor, sourceRevision, snapshot.epoch),
  ]);
  const nextCursor = nodes.nextCursor || relations.nextCursor
    ? indexedCursorEncode({
      schema: SEARCH_CURSOR_SCHEMA,
      source_revision: sourceRevision,
      snapshot_epoch: snapshot.epoch,
      query: query.toLowerCase(),
      filters,
      nodes: nodes.nextCursor,
      relations: relations.nextCursor,
      nodes_exhausted: nodes.nextCursor === null,
      relations_exhausted: relations.nextCursor === null,
    })
    : null;
  return {
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
    authority_boundary: knowledgeTop.authority_boundary ?? {},
    work: {nodes: nodes.work, relations: relations.work},
  };
}

async function knowledgeSearchD1Unchecked(
  db: D1Database,
  options: { query: string; sources: string[] | null; kindIds: string[]; predicateIds: string[]; offset: number; limit: number },
): Promise<Item> {
  const query = options.query.trim();
  if (query.length > 256) throw new HttpError(400, "knowledge search query exceeds 256 characters");
  const sources = normalizedSources(options.sources);
  const offset = bounded(options.offset, "offset", 0, 100_000);
  const limit = bounded(options.limit, "limit", 1, 100);
  if (options.kindIds.length > 100 || options.predicateIds.length > 100) {
    throw new HttpError(400, "knowledge search kind and predicate filters must contain at most 100 values");
  }
  const needle = query.toLocaleLowerCase();
  const nodeWhere = joinFragments([
    sourceFragment("n", sources),
    ...(options.kindIds.length ? [{ sql: "n.kind_id IN (SELECT value FROM json_each(?))", bindings: [JSON.stringify(options.kindIds)] }] : []),
    ...(needle ? [{ sql: "instr(n.search_text, ?) > 0", bindings: [needle] }] : []),
  ]);
  const relationWhere = joinFragments([
    sourceFragment("r", sources),
    ...(options.predicateIds.length ? [{ sql: "r.predicate_id IN (SELECT value FROM json_each(?))", bindings: [JSON.stringify(options.predicateIds)] }] : []),
    ...(needle ? [{ sql: "instr(r.search_text, ?) > 0", bindings: [needle] }] : []),
  ]);
  const nodeTitle = ["title"];
  const nodeVisible = ["title", "kind_label", "summary"];
  const relationLabel = ["label"];
  const relationVisible = ["label", "inverse_label", "statement", "explanation"];
  const nodeRank = needle
    ? searchRank("n", nodeTitle, nodeVisible, needle)
    : {sql: "n.id", bindings: [] as string[]};
  const relationRank = needle
    ? searchRank("r", relationLabel, relationVisible, needle)
    : {sql: "r.id", bindings: [] as string[]};
  const [nodeCount, relationCount, nodeRows, relationRows, knowledgeTop] = await Promise.all([
    count(db, "knowledge_nodes n", nodeWhere),
    count(db, "knowledge_relations r", relationWhere),
    jsonRows(db, `SELECT n.json FROM knowledge_nodes n WHERE ${nodeWhere.sql} ORDER BY ${nodeRank.sql} LIMIT ? OFFSET ?`, ...nodeWhere.bindings, ...nodeRank.bindings, limit, offset),
    jsonRows(db, `SELECT r.json FROM knowledge_relations r WHERE ${relationWhere.sql} ORDER BY ${relationRank.sql} LIMIT ? OFFSET ?`, ...relationWhere.bindings, ...relationRank.bindings, limit, offset),
    meta<Item>(db, "knowledge_top"),
  ]);
  return {
    schema: "tos_knowledge_search_v1",
    source_revision: knowledgeTop.source_revision,
    query,
    filters: { sources: [...sources].sort(), kind_ids: [...new Set(options.kindIds)].sort(), predicate_ids: [...new Set(options.predicateIds)].sort() },
    page: { offset, limit_per_kind: limit },
    counts: { matching_nodes: nodeCount, matching_relations: relationCount, returned_nodes: nodeRows.length, returned_relations: relationRows.length },
    nodes: nodeRows,
    relations: relationRows,
    authority_boundary: knowledgeTop.authority_boundary ?? {},
  };
}

async function knowledgeNodeD1Unchecked(db: D1Database, id: string, relationLimit: number): Promise<Item> {
  const identifier = id.trim();
  if (!identifier) throw new HttpError(400, "knowledge node id is required");
  const exact = await rows<JsonRow>(db, "SELECT json FROM knowledge_nodes WHERE id = ? ORDER BY id", identifier);
  const entity = exact.length ? [] : await rows<JsonRow>(db, "SELECT json FROM knowledge_nodes WHERE entity_id = ? ORDER BY id", identifier);
  const matchedRows = exact.length
    ? exact
    : entity.length
    ? entity
    : await rows<JsonRow>(db, "SELECT json FROM knowledge_nodes WHERE native_id = ? ORDER BY id", identifier);
  if (matchedRows.length === 0) throw new HttpError(404, `unknown ToS knowledge node: ${identifier}`);
  const matches = matchedRows.map((row) => parseItem(row.json));
  const ids = matches.map((item) => String(item.id));
  const relationWhere: SqlFragment = {
    sql: "from_id IN (SELECT value FROM json_each(?)) OR to_id IN (SELECT value FROM json_each(?))",
    bindings: [JSON.stringify(ids), JSON.stringify(ids)],
  };
  const [relatedCount, selected, knowledgeTop] = await Promise.all([
    count(db, "knowledge_relations", relationWhere),
    jsonRows(
      db,
      `SELECT json FROM knowledge_relations WHERE ${relationWhere.sql} ORDER BY id LIMIT ?`,
      ...relationWhere.bindings,
      relationLimit,
    ),
    meta<Item>(db, "knowledge_top"),
  ]);
  const refs = [...new Set([...matches, ...selected].flatMap((item) => Array.isArray(item.source_refs) ? item.source_refs.map(String) : []))].sort();
  return {
    schema: "tos_knowledge_node_packet_v1",
    source_revision: knowledgeTop.source_revision,
    requested_id: identifier,
    ambiguous_native_id: exact.length === 0 && entity.length === 0 && matches.length > 1,
    shared_entity_id: entity.length > 1,
    matches,
    related_relations: selected,
    counts: { matches: matches.length, related_relations: relatedCount, returned_relations: selected.length },
    source_refs: refs,
    authority_boundary: knowledgeTop.authority_boundary ?? {},
  };
}

async function knowledgeRelationD1Unchecked(db: D1Database, id: string): Promise<Item> {
  const identifier = id.trim();
  if (!identifier) throw new HttpError(400, "knowledge relation id is required");
  const exact = await rows<JsonRow>(db, "SELECT json FROM knowledge_relations WHERE id = ? ORDER BY id", identifier);
  const matchedRows = exact.length ? exact : await rows<JsonRow>(db, "SELECT json FROM knowledge_relations WHERE native_id = ? ORDER BY id", identifier);
  if (matchedRows.length === 0) throw new HttpError(404, `unknown ToS knowledge relation: ${identifier}`);
  const matches = matchedRows.map((row) => parseItem(row.json));
  const endpointIds = [...new Set(matches.flatMap((item) => [String(item.from_id), String(item.to_id)]))];
  const endpoints = await nodesByIds(db, endpointIds);
  const knowledgeTop = await meta<Item>(db, "knowledge_top");
  const refs = [...new Set([...matches, ...endpoints].flatMap((item) => Array.isArray(item.source_refs) ? item.source_refs.map(String) : []))].sort();
  return {
    schema: "tos_knowledge_relation_packet_v1",
    source_revision: knowledgeTop.source_revision,
    requested_id: identifier,
    ambiguous_native_id: exact.length === 0 && matches.length > 1,
    matches,
    endpoints,
    counts: { matches: matches.length, endpoints: endpoints.length },
    source_refs: refs,
    authority_boundary: knowledgeTop.authority_boundary ?? {},
  };
}

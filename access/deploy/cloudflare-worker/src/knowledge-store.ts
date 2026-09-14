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
import {executeNativeLensD1} from './native-lens-store.ts';
import {inspectNativeD1} from './native-inspection-store.ts';
import {parseNativeJson, parseNativeRequest, nativeField, arrayRefs, type NativeRef, type NativeLensResult, type NativePacket} from './native-lens.ts';
import {NativeD1Read, nativeD1Limits, readNativePublication, nativeSha256, nativeUnavailable} from './native-d1-read.ts';
import {compareNativeTemporalD1} from './native-temporal-store.ts';
import {normalizeTemporalComparisonRequest} from './temporal-comparison.ts';

const KNOWLEDGE_SOURCES = new Set(["philosophy", "canon", "candidate-intake", "source-navigation", "source-claims", "semantic-interchange", "repository"]);
const SEARCH_NGRAM_SIZE = 3;
const SEARCH_MAX_CANDIDATES = 50_000;
const SEARCH_MAX_VERIFY_CHARS = 16_000_000;
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

async function publishedCatalog(db: D1Database, revision: string): Promise<NativeRef> {
  const read=new NativeD1Read(db,nativeD1Limits,true);
  const top=await readNativePublication(read,revision,'inspection');
  const catalog=await read.metadata('knowledge_catalog',8*1024*1024);
  if (await nativeSha256(catalog.raw)!==nativeField(top.ref,'catalog_sha256').value
      || nativeField(catalog.ref,'schema').value!=='tos_knowledge_catalog_v1'
      || nativeField(catalog.ref,'source_revision').value!==nativeField(top.ref,'source_revision').value) {
    nativeUnavailable('published knowledge catalog differs from its reader binding');
  }
  return catalog.ref;
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

export async function knowledgeSearchD1Indexed(
  db: D1Database,
  options: Parameters<typeof knowledgeSearchD1IndexedUnchecked>[1],
): Promise<NativePacket> {
  return nativeSearchFailure(()=>consistentRead(db, (snapshot) => knowledgeSearchD1IndexedUnchecked(db, options, snapshot)));
}

export async function knowledgeNodeD1(db: D1Database, id: string, relationLimit: number): Promise<NativePacket> {
  return consistentRead(db, snapshot => inspectNativeD1(db, 'node', id, relationLimit, snapshot.revision));
}

export async function knowledgeRelationD1(db: D1Database, id: string): Promise<NativePacket> {
  return consistentRead(db, snapshot => inspectNativeD1(db, 'relation', id, 200, snapshot.revision));
}

export async function knowledgeTemporalCompareD1(db: D1Database, request: unknown): Promise<NativePacket> {
  const normalized = normalizeTemporalComparisonRequest(request);
  return consistentRead(db, snapshot => compareNativeTemporalD1(db,normalized,snapshot.revision));
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
  // Validate only this bounded posting window, including the zero-stat path.
  // Missing metadata cannot masquerade as an empty successful search.
  const posting=await db.prepare(`SELECT count(*) AS total,coalesce(sum(CASE WHEN document_position IS NULL THEN 1 ELSE 0 END),0) AS missing
    FROM (SELECT s.position AS document_position FROM knowledge_search_grams g
      LEFT JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
      WHERE g.kind=? AND g.n=? AND g.gram=? LIMIT ?)`).bind(kind,SEARCH_NGRAM_SIZE,selected.gram,SEARCH_MAX_CANDIDATES+1)
    .first<{total:number;missing:number}>();
  if(!posting||!Number.isSafeInteger(posting.total)||!Number.isSafeInteger(posting.missing))throw new HttpError(503,'indexed knowledge search posting metadata is invalid');
  if(posting.total>SEARCH_MAX_CANDIDATES)throw new HttpError(413,'indexed knowledge search candidate budget exceeded');
  if(posting.total!==selected.postings||posting.missing!==0)throw new HttpError(503,'indexed knowledge search posting closure is incomplete');
  if(selected.postings===0)return {rows:[],nextCursor:null,hasMore:false,work:{candidate_rows:0,verified_chars:0,sql_pages:grams.length+1}};
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
  const preflightSql = `SELECT COUNT(*) AS candidate_rows, COALESCE(SUM(s.document_chars), 0) AS verified_chars,
    COALESCE(SUM(CASE WHEN typeof(s.document_chars)='integer' AND s.document_chars>=0 THEN 0 ELSE 1 END),0) AS invalid_budgets
    FROM knowledge_search_grams g
    CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
    WHERE ${filterSql.join(" AND ")}`;
  let preflight;
  try {
    // This is deliberately a carrier-only budget gate. Do not evaluate the
    // rank expression/continuation (which walks JSON rank fields) until the
    // total candidate-document budget is known to be bounded.
    preflight = await db.prepare(preflightSql).bind(...filterBindings)
      .all<{ candidate_rows: number; verified_chars: number; invalid_budgets:number }>();
  } catch (error) {
    throw new HttpError(503, `indexed knowledge search read model is unavailable: ${String(error)}`);
  }
  const aggregate = preflight.results[0];
  const candidateRows = aggregate?.candidate_rows ?? 0;
  const verifiedChars = aggregate?.verified_chars ?? 0;
  if (
    aggregate?.invalid_budgets!==0
    || !Number.isSafeInteger(candidateRows)
    || candidateRows < 0
    || !Number.isSafeInteger(verifiedChars)
    || verifiedChars < 0
  ) {
    throw new HttpError(503, "indexed knowledge search carrier has invalid document budgets");
  }
  const preflightRowsRead = indexedRowsRead(preflight.meta?.rows_read);
  if(candidateRows>SEARCH_MAX_CANDIDATES)throw new HttpError(413,'indexed knowledge search candidate budget exceeded');
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
        sql_pages: grams.length + 2,
        ...(preflightRowsRead === undefined ? {} : { selection_rows_read: preflightRowsRead }),
      },
    };
  }
  const sql = `SELECT CASE WHEN typeof(s.id)='text' AND length(CAST(s.id AS BLOB))<=1048576 THEN s.id ELSE NULL END AS id,
    CASE WHEN typeof(s.id_lower)='text' AND length(CAST(s.id_lower AS BLOB))<=1048576 THEN s.id_lower ELSE NULL END AS id_lower,
    CASE WHEN typeof(s.position)='integer' THEN s.position ELSE NULL END AS position, ${rankExpression} AS search_rank
    FROM knowledge_search_grams g
    CROSS JOIN knowledge_search_documents s ON s.kind=g.kind AND s.position=g.position
    CROSS JOIN ${baseTable} b ON b.id=s.id
    WHERE ${filterSql.join(" AND ")} AND instr(b.search_text, ?) > 0${continuationSql}
    ORDER BY search_rank, s.id_lower, s.position LIMIT ?`;
  const bindings: unknown[] = [...indexedRankBindings(needle), ...filterBindings, needle, ...continuationBindings, options.limit + 1];
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
  const hasMore = result.results.length > options.limit;
  const last = selectedRows[selectedRows.length - 1];
  const nextCursor = hasMore && last
    ? indexedCursorEncode({ schema: SEARCH_CURSOR_SCHEMA, source_revision: sourceRevision, snapshot_epoch: snapshotEpoch, kind, query: needle, filters, rank: last.search_rank, id: last.id_lower, position: last.position })
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
      sql_pages: grams.length + 3,
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
    ...(needle ? [{ sql: "instr(n.search_text, ?) > 0", bindings: [needle] }] : []),
  ]);
  const relationWhere = joinFragments([
    sourceFragment("r", sources),
    ...(options.predicateIds.length ? [{ sql: "r.predicate_id IN (SELECT value FROM json_each(?))", bindings: [JSON.stringify(options.predicateIds)] }] : []),
    ...(needle ? [{ sql: "instr(r.search_text, ?) > 0", bindings: [needle] }] : []),
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

function searchQuery(value:string,indexed:boolean):{query:string;needle:string} {
  if(typeof value!=='string')throw new HttpError(400,'knowledge search query must be a string');
  const query=nativeStrip(value),needle=nativeLower(query);
  if((indexed&&[...value].length>256)||[...query].length>256||(indexed&&[...needle].length>256))throw new HttpError(400,'knowledge search query exceeds 256 characters');
  if(indexed&&[...needle].length<SEARCH_NGRAM_SIZE)throw new HttpError(400,'indexed knowledge search requires a query of at least three characters');
  return {query,needle};
}

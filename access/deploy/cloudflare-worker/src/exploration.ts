import { HttpError, parseItem, type Item } from './common.ts';
import { OVERVIEW_EXCLUDED_PREDICATES, OVERVIEW_EXCLUDED_RELATION_TYPES, knowledgeScene } from './knowledge.ts';
import { nodesByIds, relationsByIds, resolveFocusNodeD1 } from './knowledge-store.ts';

const VERSION = 'tos-exploration-d1-execution-v3';
const SOURCES = ['philosophy', 'canon', 'candidate-intake', 'source-navigation', 'source-claims', 'semantic-interchange', 'repository'];
const TTL = 900_000;
const MAX_BYTES = 1_048_576;
const ADJACENCY_QUERIES = 24;
type Query = {
  focus_node_id: string; sources: string[]; predicate_ids: string[];
  direction: 'either' | 'incoming' | 'outgoing'; profile: 'overview' | 'all';
  max_depth: number; page_nodes: number; page_relations: number;
};
type State = {query: Query; queue: [string, number][]; head: number; after: string | null; seen_relations: string[]; page_number: number};
type Snapshot = {epoch: number; revision: string; source_revision: string; authority_boundary: Item};
type Checkpoint = {token: string; expires: number; epoch: number; version: string; state: string | null; response: string | null};
type Header = {id: string; from_id: string; to_id: string; source_graph: string; predicate_id: string; relation_type_id: string | null; from_source: string | null; to_source: string | null};

function bad(message: string): never { throw new HttpError(400, message); }
function expired(): never { throw new HttpError(410, 'exploration expired or was evicted; restart from focus'); }
function conflict(): never { throw new HttpError(409, 'exploration snapshot changed; restart from focus'); }
function token(): string { return [...crypto.getRandomValues(new Uint8Array(32))].map(b => b.toString(16).padStart(2, '0')).join(''); }
function byteSize(text: string): number { return new TextEncoder().encode(text).length; }
function encoded(value: unknown): string {
  const text = JSON.stringify(value);
  if (byteSize(text) > MAX_BYTES) throw new HttpError(413, 'exploration checkpoint exceeds 1 MiB; narrow the neighborhood or page size');
  return text;
}
function object(value: unknown): Item {
  if (!value || typeof value !== 'object' || Array.isArray(value)) bad('exploration request must be an object');
  return value as Item;
}
export function normalizeExploration(value: unknown): Query {
  const input = object(value);
  const allowed = ['focus_node_id', 'sources', 'direction', 'predicate_ids', 'profile', 'max_depth', 'page_nodes', 'page_relations'];
  if (Object.keys(input).some(k => !allowed.includes(k))) bad('unknown exploration fields; continue with cursor only');
  const focus = input.focus_node_id;
  if (typeof focus !== 'string' || !focus.trim() || [...focus].length > 1024) bad('invalid exploration focus_node_id');
  function integer(name: string, fallback: number, min: number, max: number): number {
    const v = input[name] ?? fallback;
    if (name in input && input[name] === null || typeof v !== 'number' || !Number.isInteger(v) || v < min || v > max) bad(`invalid exploration ${name}`);
    return v;
  }
  function strings(name: string, fallback: string[], max: number): string[] {
    const v = name in input ? input[name] : fallback;
    if (!Array.isArray(v) || v.length > max || v.some(i => typeof i !== 'string' || !i || [...i].length > 1024)) bad(`invalid exploration ${name}`);
    return [...new Set(v as string[])].sort();
  }
  const sources = strings('sources', SOURCES, 7);
  if (!sources.length || sources.some(s => !SOURCES.includes(s))) bad('exploration sources must be nonempty registered sources');
  const direction = 'direction' in input ? input.direction : 'either';
  const profile = 'profile' in input ? input.profile : 'overview';
  if (direction !== 'either' && direction !== 'incoming' && direction !== 'outgoing') bad('invalid exploration direction');
  if (profile !== 'overview' && profile !== 'all') bad('invalid exploration profile');
  return {focus_node_id: focus, sources, direction, profile, predicate_ids: strings('predicate_ids', [], 100),
    max_depth: integer('max_depth', 3, 0, 10), page_nodes: integer('page_nodes', 40, 1, 100), page_relations: integer('page_relations', 80, 1, 100)};
}

// No Sessions API: D1 bindings read the primary. A publication clock additionally
// detects A -> B -> A while a page spans several reads. No module-level state.
async function snapshot(db: D1Database): Promise<Snapshot> {
  const row = await db.prepare(`SELECT c.epoch, d.json_chunk AS revision, t.json_chunk AS top
    FROM knowledge_exploration_clock c JOIN edge_meta d ON d.key='data_revision' AND d.part=0
    JOIN edge_meta t ON t.key='knowledge_exploration_top' AND t.part=0 WHERE c.singleton=1`)
    .first<{epoch: number; revision: string; top: string}>();
  if (!row) throw new HttpError(503, 'exploration read model is not prepared');
  const revision = parseItem(row.revision).sha256, top = parseItem(row.top);
  if (typeof revision !== 'string' || typeof top.source_revision !== 'string') throw new Error('invalid exploration snapshot metadata');
  return {epoch: row.epoch, revision, source_revision: top.source_revision, authority_boundary: object(top.authority_boundary)};
}

export async function explorationCapabilitiesD1(db: D1Database): Promise<Item> {
  const required = ['knowledge_exploration_clock', 'knowledge_exploration_checkpoints', 'knowledge_exploration_revision_insert',
    'knowledge_exploration_revision_update', 'knowledge_exploration_revision_delete', 'knowledge_relations_from_seek', 'knowledge_relations_to_seek'];
  const count = await db.prepare('SELECT count(*) AS count FROM sqlite_master WHERE name IN (SELECT value FROM json_each(?))')
    .bind(JSON.stringify(required)).first<number>('count');
  let available = count === required.length;
  if (available) {
    const top = await db.prepare("SELECT count(*) AS count FROM edge_meta WHERE key='knowledge_exploration_top'").first<number>('count');
    available = top === 1;
  }
  return {schema: 'tos_exploration_capabilities_v1', available, execution_version: VERSION,
    storage: 'shared-d1-checkpoints', ttl_seconds: TTL / 1000, restart_survival: true, writes_to_tree: false,
    http: {method: 'POST', path: '/api/knowledge/explore'}, max_checkpoints: 128,
    max_checkpoint_bytes: MAX_BYTES, max_cache_bytes: 32 * MAX_BYTES,
    limits: {depth: 10, page_nodes: 100, page_relations: 100, work_per_page: 512,
      adjacency_queries_per_page: ADJACENCY_QUERIES, session_nodes: 10000, session_relations: 20000},
    continuation: 'opaque-cursor-only; fixed query and page sizes',
    ordering: 'breadth-first, relation-id ascending per expanded node',
    reason: available ? null : 'apply exploration migration and build compatible read-model metadata'};
}

// Two covering adjacency seeks avoid scanning/sorting the entire incident list
// at a high-degree node. UNION removes a self-loop's duplicate occurrence.
export const ADJACENCY_SQL = `SELECT r.id,r.from_id,r.to_id,r.source_graph,r.predicate_id,
    json_extract(r.json,'$.relation_type_id') AS relation_type_id,
    f.source_graph AS from_source,t.source_graph AS to_source
  FROM (SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek
           WHERE from_id=? AND id>? ORDER BY id LIMIT 32)
        UNION SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek
           WHERE to_id=? AND id>? ORDER BY id LIMIT 32)
        ORDER BY id LIMIT 32) ids
  JOIN knowledge_relations r ON r.id=ids.id
  LEFT JOIN knowledge_nodes f ON f.id=r.from_id LEFT JOIN knowledge_nodes t ON t.id=r.to_id ORDER BY r.id`;

async function advance(db: D1Database, state: State, snap: Snapshot) {
  const q = state.query, focus = q.focus_node_id;
  const primary = state.page_number === 0 ? [focus] : [];
  const emitted: string[] = [], selected = new Set<string>([focus]);
  const nodes = new Set(state.queue.map(([id]) => id)), edges = new Set(state.seen_relations);
  const nodeReasons: Record<string, Item> = {[focus]: {kind: 'focus'}}, edgeReasons: Record<string, Item> = {};
  let work = 0, reads = 0, limit: string | null = null;
  let cached: Header[] = [], cachedNode: string | null = null;
  while (state.head < state.queue.length && work < 512) {
    const [current, depth] = state.queue[state.head]!;
    if (depth >= q.max_depth) { state.head++; state.after = null; work++; continue; }
    if (cachedNode !== current || !cached.length) {
      if (reads >= ADJACENCY_QUERIES) break;
      const packet = await db.prepare(ADJACENCY_SQL).bind(current, state.after ?? '', current, state.after ?? '').all<Header>();
      cached = packet.results; cachedNode = current; reads++;
      if (!cached.length) { state.head++; state.after = null; work++; continue; }
    }
    const edge = cached[0]!;
    const target = edge.from_id === current ? edge.to_id : edge.from_id;
    work++;
    const eligible = !edges.has(edge.id) && q.sources.includes(edge.source_graph)
      && edge.from_source !== null && edge.to_source !== null && q.sources.includes(edge.from_source) && q.sources.includes(edge.to_source)
      && (q.direction !== 'outgoing' || edge.from_id === current) && (q.direction !== 'incoming' || edge.to_id === current)
      && (!q.predicate_ids.length || q.predicate_ids.includes(edge.predicate_id))
      && (q.profile !== 'overview' || (!OVERVIEW_EXCLUDED_PREDICATES.includes(edge.predicate_id)
        && !OVERVIEW_EXCLUDED_RELATION_TYPES.includes(edge.relation_type_id ?? '')));
    if (!eligible) { state.after = edge.id; cached.shift(); continue; }
    const fresh = !nodes.has(target);
    if (edges.size >= 20000 || fresh && nodes.size >= 10000) { limit = edges.size >= 20000 ? 'session_relations' : 'session_nodes'; break; }
    if (emitted.length >= q.page_relations || fresh && primary.length >= q.page_nodes) break;
    state.after = edge.id; cached.shift(); edges.add(edge.id); state.seen_relations.push(edge.id); emitted.push(edge.id);
    selected.add(edge.from_id); selected.add(edge.to_id);
    edgeReasons[edge.id] = {kind: 'traversal', via_node_id: current, depth: depth + 1};
    if (fresh) {
      nodes.add(target); state.queue.push([target, depth + 1]); primary.push(target);
      nodeReasons[target] = {kind: 'traversal', via_node_id: current, via_relation_id: edge.id, depth: depth + 1};
    }
  }
  state.page_number++;
  primary.forEach(id => selected.add(id));
  selected.forEach(id => { nodeReasons[id] ??= {kind: 'context-endpoint'}; });
  const status = limit ? 'limit_reached' : state.head === state.queue.length ? 'complete' : 'paused';
  const [selectedNodes, selectedEdges] = await Promise.all([nodesByIds(db, selected), relationsByIds(db, emitted)]);
  if (selectedNodes.length !== selected.size || selectedEdges.length !== emitted.length) conflict();
  const byId = new Map(selectedEdges.map(e => [e.id, e]));
  function compact(item: Item): Item { const {source_record: _, ...rest} = item; return {...rest, attributes: {}}; }
  const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(JSON.stringify([VERSION, snap.revision, snap.epoch])));
  return {schema: 'tos_exploration_result_v1', execution_version: VERSION,
    snapshot_revision: [...new Uint8Array(hash)].map(b => b.toString(16).padStart(2, '0')).join(''), source_revision: snap.source_revision,
    query: q, focus: {node_id: focus}, status, limit_reason: limit,
    nodes: selectedNodes.map(compact), relations: emitted.map(id => compact(byId.get(id)!)),
    scene: knowledgeScene(selectedNodes, selectedEdges, focus),
    page: {number: state.page_number, primary_node_ids: primary, context_node_ids: [...selected].filter(id => !primary.includes(id)).sort(),
      next_cursor: null as string | null, returned_nodes: selected.size, returned_relations: emitted.length, work_units: work, scope: 'resumable-neighborhood'},
    counts: {discovered_nodes: nodes.size, emitted_relations: edges.size, scope: 'cumulative-discovered-not-global-total'},
    inclusion: {nodes: nodeReasons, relations: edgeReasons, authority: 'query-execution-not-semantic-proof'},
    authority_boundary: snap.authority_boundary, writes_to_tree: false};
}

export async function exploreD1(db: D1Database, request: unknown): Promise<Item> {
  const input = object(request), continuing = 'cursor' in input;
  const cursor = input.cursor;
  if (continuing && (Object.keys(input).length !== 1 || typeof cursor !== 'string' || !/^[0-9a-f]{64}$/.test(cursor))) bad('continue exploration with one opaque cursor only');
  const query = continuing ? null : normalizeExploration(input);
  if (!(await explorationCapabilitiesD1(db)).available) throw new HttpError(503, 'exploration read model is not prepared');
  const snap = await snapshot(db), now = Date.now();
  let record: Checkpoint | null = null;
  if (continuing) {
    record = await db.prepare('SELECT * FROM knowledge_exploration_checkpoints WHERE token=? AND expires>?').bind(cursor, now).first<Checkpoint>();
    if (!record) expired();
    if (record.epoch !== snap.epoch || record.version !== VERSION) conflict();
    if (record.response !== null) {
      if ((await snapshot(db)).epoch !== snap.epoch) conflict();
      return parseItem(record.response);
    }
  }
  let state: State;
  if (record) {
    // This is server-authored state, admitted by this execution version only.
    if (!record.state) throw new Error('missing exploration checkpoint state');
    state = JSON.parse(record.state) as State;
  } else {
    const focus = await resolveFocusNodeD1(db, query!.focus_node_id, query!.sources);
    if (!focus) bad('unknown exploration focus');
    query!.focus_node_id = focus.id;
    state = {query: query!, queue: [[focus.id, 0]], head: 0, after: null, seen_relations: [], page_number: 0};
  }
  const result = await advance(db, state, snap);
  const next = result.status === 'paused' ? token() : null;
  result.page.next_cursor = next;
  const response = encoded(result), nextState = next ? encoded(state) : null;
  const expires = record?.expires ?? now + TTL;
  const finished = Date.now();
  if (expires <= finished) expired();
  if ((await snapshot(db)).epoch !== snap.epoch) conflict();
  const statements: D1PreparedStatement[] = [];
  statements.push(db.prepare('DELETE FROM knowledge_exploration_checkpoints WHERE expires<=?').bind(finished));
  if (record) {
    statements.push(db.prepare(`UPDATE knowledge_exploration_checkpoints SET state=NULL,response=?,successor=?,bytes=?
      WHERE token=? AND response IS NULL AND expires>? AND epoch=? AND version=?
      AND EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE epoch=? AND singleton=1)`)
      .bind(response, next, byteSize(response), cursor, finished, snap.epoch, VERSION, snap.epoch));
  }
  if (next) {
    statements.push(db.prepare(`INSERT INTO knowledge_exploration_checkpoints(token,expires,epoch,version,state,bytes)
      SELECT ?,?,?,?,?,? WHERE EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE singleton=1 AND epoch=?)
      ${record ? 'AND EXISTS(SELECT 1 FROM knowledge_exploration_checkpoints WHERE token=? AND successor=?)' : ''}`)
      .bind(next, expires, snap.epoch, VERSION, nextState, byteSize(nextState!), snap.epoch, ...(record ? [cursor, next] : [])));
  }
  // Global bounds are enforced in the same atomic batch as admission. Eviction
  // affects execution cache only, not source or read-model records.
  statements.push(db.prepare(`DELETE FROM knowledge_exploration_checkpoints WHERE token IN (
    SELECT token FROM (SELECT token,ROW_NUMBER() OVER(ORDER BY rowid DESC) AS n,
      SUM(bytes) OVER(ORDER BY rowid DESC) AS total FROM knowledge_exploration_checkpoints)
    WHERE n>128 OR total>33554432)`));
  statements.push(db.prepare('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1'));
  if (record) statements.push(db.prepare('SELECT response FROM knowledge_exploration_checkpoints WHERE token=?').bind(cursor));
  const committed = await db.batch(statements);
  const epochIndex = committed.length - (record ? 2 : 1);
  if ((committed[epochIndex]!.results[0] as {epoch: number} | undefined)?.epoch !== snap.epoch) conflict();
  if (record) {
    const winner = committed.at(-1)!.results[0] as {response: string | null} | undefined;
    if (!winner?.response) expired();
    return parseItem(winner.response);
  }
  return result;
}

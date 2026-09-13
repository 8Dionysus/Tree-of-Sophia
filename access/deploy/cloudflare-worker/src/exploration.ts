import { HttpError, type Item } from './common.ts';
import { OVERVIEW_EXCLUDED_PREDICATES, OVERVIEW_EXCLUDED_RELATION_TYPES, knowledgeScene } from './knowledge.ts';
import {bindOriginD1, normalizeOrigin, REQUEST_V2, RESULT_V2, type Origin, type ResolvedOrigin} from './exploration-origin.ts';
import {NativeD1Read,nativeD1Limits,nativeUnavailable} from './native-d1-read.ts';
import {readNativeInspectionPublication} from './native-inspection-store.ts';
import {NativeExplorationRows} from './native-exploration-store.ts';
import {nativeCarrier} from './native-lens-result.ts';
import {NativeBudgetExceeded,codePointCompare,nativeNumberInfo} from '../../../shared/native-semantics.ts';
import {nativeStrip} from '../../../shared/native-unicode.ts';
import {derived,nativeChild,nativeField,nativeKeys,nativePacketArray,nativePacketObject,nativePacketJson,parseNativeJson,
  type NativeRef,type NativePacketValue} from './native-lens.ts';

const VERSION = 'tos-exploration-d1-execution-v6';
// Private disposable-cache framing, not a new public traversal execution ABI.
const CACHE_VERSION = VERSION + '/native-json-v1/selected-relation-first-v1';
const SOURCES = ['philosophy', 'canon', 'candidate-intake', 'source-navigation', 'source-claims', 'semantic-interchange', 'repository'];
const TTL = 900_000;
const MAX_BYTES = 1_048_576;
const ADJACENCY_QUERIES = 24;
type Options = {
  sources: string[]; predicate_ids: string[];
  direction: 'either' | 'incoming' | 'outgoing'; profile: 'overview' | 'all';
  max_depth: number; page_nodes: number; page_relations: number;
};
type LegacyQuery = Options & {focus_node_id: string};
type Query = LegacyQuery | (Options & {schema_version: typeof REQUEST_V2; source_revision: string; origin: Origin});
type State = {query: Query; queue: [string, number][]; head: number; after: string | null;
  identity_after: string | null; identity_complete: boolean; identity_added: number;
  identity_entity: string | null; expanded_entities: string[];
  seen_relations: string[]; page_number: number;
  origin?: ResolvedOrigin; roots?: string[]; seed_relations?: string[]};
type Snapshot = {epoch: number; revision: string; source_revision: string; authority_boundary: NativeRef};
type Checkpoint = {token: string; expires: number; epoch: number; version: string; bytes:number; state: string | null; response: string | null};
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
function validateReplay(raw:string):void {
  let ref:NativeRef;
  try {ref=parseNativeJson(raw,{maxBytes:MAX_BYTES,maxMembers:300000});} catch {return nativeUnavailable('invalid exploration replay JSON');}
  if (!['tos_exploration_result_v1',RESULT_V2].includes(String(nativeField(ref,'schema').value))
      || nativeField(ref,'execution_version').value!==VERSION) nativeUnavailable('invalid exploration replay packet');
  const pending=[ref];
  while(pending.length){const value=pending.pop()!;
    if(typeof value.value==='string'&&!value.value.isWellFormed())nativeUnavailable('exploration response contains invalid Unicode');
    if(value.value&&typeof value.value==='object')for(const key of nativeKeys(value)){
      if(!key.isWellFormed())nativeUnavailable('exploration response contains invalid Unicode member name');
      pending.push(nativeChild(value,key));
    }
  }
}
function checkpointState(raw:string):State {
  let ref:NativeRef;
  try {ref=parseNativeJson(raw,{maxBytes:MAX_BYTES,maxMembers:300000});} catch {return nativeUnavailable('invalid exploration checkpoint state JSON');}
  const pending=[ref];
  while (pending.length) {
    const item=pending.pop()!;
    if (typeof item.value==='number' && (nativeNumberInfo(item).kind!=='int' || !Number.isSafeInteger(item.value))) nativeUnavailable('checkpoint counters must be bounded integers');
    if (typeof item.value==='string' && !item.value.isWellFormed()) nativeUnavailable('checkpoint text is not well formed');
    if (item.value && typeof item.value==='object') for (const key of nativeKeys(item)) pending.push(nativeChild(item,key));
  }
  const state=ref.value as State;
  if (!state || typeof state!=='object' || Array.isArray(state)) nativeUnavailable('invalid exploration checkpoint state');
  const keys=['query','queue','head','after','identity_after','identity_complete','identity_added','identity_entity','expanded_entities','seen_relations','page_number'];
  if (state.origin!==undefined) keys.push('origin','roots','seed_relations');
  if (Object.keys(state).sort().join(',')!==keys.sort().join(',')) nativeUnavailable('checkpoint state contains unexpected fields');
  const integer=(value:unknown,min:number,max:number)=>typeof value==='number'&&Number.isSafeInteger(value)&&value>=min&&value<=max;
  const strings=(value:unknown,max:number):value is string[]=>Array.isArray(value)&&value.length<=max&&value.every(v=>typeof v==='string'&&v.length>0)&&new Set(value).size===value.length;
  if (!Array.isArray(state.queue)||!state.queue.length||state.queue.length>10000
      ||state.queue.some(pair=>!Array.isArray(pair)||pair.length!==2||typeof pair[0]!=='string'||!pair[0]||!integer(pair[1],0,10))
      ||new Set(state.queue.map(pair=>pair[0])).size!==state.queue.length
      ||!integer(state.head,0,state.queue.length)||!integer(state.identity_added,0,state.queue.length)||!integer(state.page_number,1,Number.MAX_SAFE_INTEGER)
      ||typeof state.identity_complete!=='boolean'||![state.after,state.identity_after,state.identity_entity].every(v=>v===null||typeof v==='string')
      ||!strings(state.expanded_entities,10000)||!strings(state.seen_relations,20000)) nativeUnavailable('invalid exploration checkpoint traversal');
  let query:Query;
  try {query=normalizeExploration(state.query);} catch {return nativeUnavailable('invalid exploration checkpoint query');}
  if (JSON.stringify(query)!==JSON.stringify(state.query)) nativeUnavailable('checkpoint query is not normalized');
  if ('origin' in query) {
    const origin=state.origin;
    if (!origin||origin.kind!==query.origin.kind||origin.id!==query.origin.id||origin.content_revision!==query.origin.content_revision
        ||!strings(state.roots,2)||!strings(state.seed_relations,1)) nativeUnavailable('invalid exploration checkpoint origin');
    if (origin.kind==='node') {
      if (Object.keys(origin).sort().join(',')!=='content_revision,id,kind'||state.roots.join(',')!==origin.id||state.seed_relations.length) nativeUnavailable('invalid node origin state');
    } else {
      if (Object.keys(origin).sort().join(',')!=='content_revision,endpoints,id,kind'||!origin.endpoints
          ||Object.keys(origin.endpoints).sort().join(',')!=='from,to'||state.seed_relations.length!==1||state.seed_relations[0]!==origin.id) nativeUnavailable('invalid relation origin state');
      for (const endpoint of Object.values(origin.endpoints)) if (!endpoint||Object.keys(endpoint).sort().join(',')!=='content_revision,entity_id,node_id'
          ||typeof endpoint.node_id!=='string'||!endpoint.node_id||typeof endpoint.entity_id!=='string'||!endpoint.entity_id
          ||typeof endpoint.content_revision!=='string'||! /^[a-f0-9]{64}$/.test(endpoint.content_revision)) nativeUnavailable('invalid origin endpoint state');
      if (JSON.stringify(state.roots)!==JSON.stringify([...new Set([origin.endpoints.from.node_id,origin.endpoints.to.node_id])])) nativeUnavailable('origin root state differs');
    }
    if (state.roots.some(id=>!state.queue.some(([node])=>node===id))||state.seed_relations.some(id=>!state.seen_relations.includes(id))) nativeUnavailable('checkpoint origin closure missing');
  } else if (state.origin!==undefined||!state.queue.some(([id])=>id===query.focus_node_id)) nativeUnavailable('checkpoint focus closure missing');
  // Validation established an exclusively structural, safe-integer document.
  // The source parser freezes its input; only this private state is cloned.
  return structuredClone(state);
}
function object(value: unknown): Item {
  if (!value || typeof value !== 'object' || Array.isArray(value)) bad('exploration request must be an object');
  return value as Item;
}
export function normalizeExploration(value: unknown): Query {
  const input = object(value);
  if (input.schema_version === REQUEST_V2) {
    const origin = normalizeOrigin(input);
    const {schema_version: _schema, source_revision, origin: _origin, ...options} = input;
    if ('focus_node_id' in options) bad('exploration v2 uses origin, not legacy focus_node_id');
    const {focus_node_id: _focus, ...normalized} = normalizeExploration({focus_node_id: origin.id, ...options}) as LegacyQuery;
    return {schema_version: REQUEST_V2, source_revision: source_revision as string, origin, ...normalized};
  }
  const allowed = ['focus_node_id', 'sources', 'direction', 'predicate_ids', 'profile', 'max_depth', 'page_nodes', 'page_relations'];
  if (Object.keys(input).some(k => !allowed.includes(k))) bad('unknown exploration fields; continue with cursor only');
  const focus = input.focus_node_id;
  if (typeof focus !== 'string' || !nativeStrip(focus) || [...focus].length > 1024) bad('invalid exploration focus_node_id');
  function integer(name: string, fallback: number, min: number, max: number): number {
    const v = input[name] ?? fallback;
    if (name in input && input[name] === null || typeof v !== 'number' || !Number.isInteger(v) || v < min || v > max) bad(`invalid exploration ${name}`);
    return v;
  }
  function strings(name: string, fallback: string[], max: number): string[] {
    const v = name in input ? input[name] : fallback;
    if (!Array.isArray(v) || v.length > max || v.some(i => typeof i !== 'string' || !i || [...i].length > 1024)) bad(`invalid exploration ${name}`);
    return [...new Set(v as string[])].sort(codePointCompare);
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

// Two covering adjacency seeks avoid scanning/sorting the entire incident list
// at a high-degree node. UNION removes a self-loop's duplicate occurrence.
export const ADJACENCY_SQL = `SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek
           WHERE from_id=? AND id>? ORDER BY id LIMIT 32)
        UNION SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek
           WHERE to_id=? AND id>? ORDER BY id LIMIT 32)
        ORDER BY id LIMIT 32`;

export const IDENTITY_SQL = `SELECT id,entity_id FROM knowledge_nodes INDEXED BY knowledge_nodes_identity_seek
  WHERE entity_id=(SELECT entity_id FROM knowledge_nodes WHERE id=? AND substr(entity_id,1,4)='tos.'
                  AND entity_id NOT IN (SELECT value FROM json_each(?)))
    AND id>? AND source_graph IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT 32`;

async function advance(rows: NativeExplorationRows, state: State, snap: Snapshot) {
  const q = state.query, origin = state.origin;
  const focus = origin ? origin.kind === 'node' ? origin.id : null : (q as LegacyQuery).focus_node_id;
  const roots = origin ? state.roots! : [focus!], seedRelations = state.seed_relations ?? [];
  const primary: string[] = !origin && state.page_number === 0 ? [focus!] : [];
  const emitted: string[] = [], selected = new Set<string>(roots);
  const promoted = new Set<string>();
  const nodes = new Set(state.queue.map(([id]) => id)), edges = new Set(state.seen_relations);
  const nodeReasons: Record<string, Item> = Object.fromEntries(roots.map(id => [id,
    {kind: origin ? origin.kind === 'node' ? 'origin' : 'origin-endpoint' : 'focus'}]));
  const edgeReasons: Record<string, Item> = Object.fromEntries(seedRelations.map(id => [id, {kind: 'origin'}]));
  // Published IDs are data even when they coincide with Object prototype keys.
  Object.setPrototypeOf(nodeReasons,null); Object.setPrototypeOf(edgeReasons,null);
  let work = 0, reads = 0, limit: string | null = null;
  let cached: Header[] = [], cachedNode: string | null = null;
  let identities: {id: string; entity_id: string}[] = [], identityNode: string | null = null;
  while (state.head < state.queue.length && work < 512) {
    const [current, depth] = state.queue[state.head]!;
    if (depth >= q.max_depth) {
      state.head++; state.after = null; state.identity_after = null; state.identity_complete = false; state.identity_added = 0; state.identity_entity = null;
      work++; continue;
    }
    if (q.profile === 'overview' && !state.identity_complete) {
      if (identityNode !== current || !identities.length) {
        if (reads >= ADJACENCY_QUERIES) break;
        identities = await rows.identities(IDENTITY_SQL,current,state.expanded_entities,state.identity_after ?? '',q.sources);
        identityNode = current; reads++;
        if (!identities.length) {
          state.identity_complete = true;
          if (state.identity_entity !== null && !state.expanded_entities.includes(state.identity_entity)) state.expanded_entities.push(state.identity_entity);
          work++; continue;
        }
      }
      const alias = identities[0]!;
      state.identity_entity = alias.entity_id;
      work++;
      if (nodes.has(alias.id)) {
        const position = state.queue.findIndex(([id]) => id === alias.id);
        if (state.queue[position]![1] > depth) {
          if (!promoted.has(alias.id) && primary.length + promoted.size >= q.page_nodes) break;
          state.queue.splice(position, 1);
          state.queue.splice(state.head + 1 + state.identity_added, 0, [alias.id, depth]);
          state.identity_added++; promoted.add(alias.id); selected.add(alias.id);
          nodeReasons[alias.id] = {kind: 'identity-carrier', via_node_id: current, entity_id: alias.entity_id, depth};
        }
      } else {
        if (nodes.size >= 10000) { limit = 'session_nodes'; break; }
        if (primary.length + promoted.size >= q.page_nodes) break;
        nodes.add(alias.id); primary.push(alias.id);
        state.queue.splice(state.head + 1 + state.identity_added, 0, [alias.id, depth]); state.identity_added++;
        nodeReasons[alias.id] = {kind: 'identity-carrier', via_node_id: current, entity_id: alias.entity_id, depth};
      }
      state.identity_after = alias.id; identities.shift(); continue;
    }
    if (cachedNode !== current || !cached.length) {
      if (reads >= ADJACENCY_QUERIES) break;
      cached = await rows.adjacent(ADJACENCY_SQL,current,state.after ?? ''); cachedNode = current; reads++;
      if (!cached.length) {
        state.head++; state.after = null; state.identity_after = null; state.identity_complete = false; state.identity_added = 0; state.identity_entity = null;
        work++; continue;
      }
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
    if (emitted.length >= q.page_relations || fresh && primary.length + promoted.size >= q.page_nodes) break;
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
  const deliveredIds = [...seedRelations, ...emitted];
  const [selectedNodes, selectedEdges] = await Promise.all([rows.selected('node',selected),rows.selected('relation',deliveredIds)]);
  if (selectedNodes.size !== selected.size || selectedEdges.size !== deliveredIds.length) conflict();
  const deliveredNodes = [...selected].sort(codePointCompare).map(id => nativeCarrier(selectedNodes.get(id)!,{detail:'compact',language:'auto'}));
  const deliveredEdges = deliveredIds.map(id => nativeCarrier(selectedEdges.get(id)!,{detail:'compact',language:'auto'}));
  const hash = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(JSON.stringify([VERSION, snap.revision, snap.epoch])));
  return {schema: origin ? RESULT_V2 : 'tos_exploration_result_v1', execution_version: VERSION,
    snapshot_revision: [...new Uint8Array(hash)].map(b => b.toString(16).padStart(2, '0')).join(''), source_revision: snap.source_revision,
    query: q, ...(origin ? {origin} : {focus: {node_id: focus}}), status, limit_reason: limit,
    nodes: nativePacketArray(deliveredNodes.map(row=>row.packet)), relations: nativePacketArray(deliveredEdges.map(row=>row.packet)),
    scene: knowledgeScene(deliveredNodes.map(row=>row.scene), deliveredEdges.map(row=>row.scene), focus, origin?.kind === 'relation' ? origin.id : null),
    page: {number: state.page_number, primary_node_ids: primary, context_node_ids: [...selected].filter(id => !primary.includes(id)).sort(codePointCompare),
      ...(origin ? {primary_relation_ids: emitted, context_relation_ids: seedRelations} : {}),
      next_cursor: null as string | null, returned_nodes: selected.size, returned_relations: deliveredIds.length, work_units: work, scope: 'resumable-neighborhood'},
    counts: {discovered_nodes: nodes.size, emitted_relations: edges.size - seedRelations.length, scope: 'cumulative-discovered-not-global-total'},
    inclusion: {nodes: nodeReasons, relations: edgeReasons, authority: 'query-execution-not-semantic-proof'},
    authority_boundary: snap.authority_boundary, writes_to_tree: false};
}

export async function exploreD1(db: D1Database, request: unknown, nativeRequest?: NativeRef): Promise<string> {
  try {return await exploreNativeD1(db,request,nativeRequest);}
  catch(error){
    if(error instanceof HttpError||error instanceof NativeBudgetExceeded)throw error;
    return nativeUnavailable('prepared exploration publication or checkpoint is unavailable or invalid');
  }
}
async function exploreNativeD1(db: D1Database, request: unknown, nativeRequest?: NativeRef): Promise<string> {
  const input = object(request), continuing = 'cursor' in input;
  if (nativeRequest) for (const key of ['max_depth','page_nodes','page_relations']) if (nativeKeys(nativeRequest).includes(key)) {
    const ref=nativeChild(nativeRequest,key);
    if (typeof ref.value !== 'number' || nativeNumberInfo(ref).kind !== 'int') bad('invalid exploration '+key);
  }
  const cursor = input.cursor;
  if (continuing && (Object.keys(input).length !== 1 || typeof cursor !== 'string' || cursor.length !== 64
      || !/^[0-9a-f]{64}$/.test(cursor))) bad('continue exploration with one opaque cursor only');
  const query = continuing ? null : normalizeExploration(input);
  if (!(await explorationCapabilitiesD1(db)).available) throw new HttpError(503, 'exploration read model is not prepared');
  const read = new NativeD1Read(db,nativeD1Limits,true), rows=new NativeExplorationRows(read);
  const snap = await snapshot(read), now = Date.now();
  const publication = await readNativeInspectionPublication(read,snap.revision);
  if (nativeField(publication.ref,'source_revision').value !== snap.source_revision
      || nativePacketJson(nativeField(publication.ref,'authority_boundary')) !== nativePacketJson(snap.authority_boundary)) nativeUnavailable('exploration publication headers disagree');
  let record: Checkpoint | null = null;
  if (continuing) {
    record = await checkpoint(db,cursor as string,now);
    if (!record) expired();
    if (record.epoch !== snap.epoch || record.version !== CACHE_VERSION) conflict();
    if (record.response !== null) {
      validateReplay(record.response);
      if ((await snapshot(read)).epoch !== snap.epoch) conflict();
      return record.response;
    }
  }
  let state: State;
  if (record) {
    // This is server-authored state, admitted by this execution version only.
    if (!record.state) throw new Error('missing exploration checkpoint state');
    state = checkpointState(record.state);
  } else {
    let roots: string[], seedRelations: string[], origin: ResolvedOrigin | undefined;
    if ('origin' in query!) {
      ({origin, roots, seedRelations} = await bindOriginD1(db, query, snap.source_revision,(kind,id)=>rows.exact(kind,id)));
    } else {
      const legacy = query as LegacyQuery;
      const focus = await rows.focus(legacy.focus_node_id, legacy.sources);
      legacy.focus_node_id = focus;
      roots = [focus]; seedRelations = [];
    }
    state = {query: query!, queue: roots.map(id => [id, 0]), head: 0, after: null,
      identity_after: null, identity_complete: false, identity_added: 0, identity_entity: null, expanded_entities: [],
      seen_relations: [...seedRelations], page_number: 0,
      ...(origin ? {origin, roots, seed_relations: seedRelations} : {})};
  }
  const result = await advance(rows, state, snap);
  const next = result.status === 'paused' ? token() : null;
  result.page.next_cursor = next;
  const packet = nativePacketObject(Object.entries(result).map(([key,value])=>[key,
    key === 'nodes' || key === 'relations' || key === 'authority_boundary' ? value as NativePacketValue : derived(value)]));
  const response = nativePacketJson(packet,{maxBytes:MAX_BYTES}), nextState = next ? encoded(state) : null;
  // Validate the exact final text, without reserializing it. In particular,
  // Python's UTF-8 response boundary refuses retained lone-surrogate strings.
  validateReplay(response);
  const expires = record?.expires ?? now + TTL;
  const finished = Date.now();
  if (expires <= finished) expired();
  if ((await snapshot(read)).epoch !== snap.epoch) conflict();
  const statements: D1PreparedStatement[] = [];
  statements.push(db.prepare(`DELETE FROM knowledge_exploration_checkpoints WHERE expires<=?
    AND EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE singleton=1 AND epoch=?)`).bind(finished,snap.epoch));
  if (record) {
    statements.push(db.prepare(`UPDATE knowledge_exploration_checkpoints SET state=NULL,response=?,successor=?,bytes=?
      WHERE token=? AND response IS NULL AND expires>? AND epoch=? AND version=?
      AND EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE epoch=? AND singleton=1)`)
      .bind(response, next, byteSize(response), cursor, finished, snap.epoch, CACHE_VERSION, snap.epoch));
  }
  if (next) {
    statements.push(db.prepare(`INSERT INTO knowledge_exploration_checkpoints(token,expires,epoch,version,state,bytes)
      SELECT ?,?,?,?,?,? WHERE EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE singleton=1 AND epoch=?)
      ${record ? 'AND EXISTS(SELECT 1 FROM knowledge_exploration_checkpoints WHERE token=? AND successor=?)' : ''}`)
      .bind(next, expires, snap.epoch, CACHE_VERSION, nextState, byteSize(nextState!), snap.epoch, ...(record ? [cursor, next] : [])));
  }
  // Global bounds are enforced in the same atomic batch as admission. Eviction
  // affects execution cache only, not source or read-model records.
  statements.push(db.prepare(`DELETE FROM knowledge_exploration_checkpoints WHERE token IN (
    SELECT token FROM (SELECT token,ROW_NUMBER() OVER(ORDER BY rowid DESC) AS n,
      SUM(bytes) OVER(ORDER BY rowid DESC) AS total FROM knowledge_exploration_checkpoints)
    WHERE n>128 OR total>33554432)
    AND EXISTS(SELECT 1 FROM knowledge_exploration_clock WHERE singleton=1 AND epoch=?)`).bind(snap.epoch));
  statements.push(db.prepare('SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1'));
  if (record) statements.push(db.prepare(`SELECT ${RESPONSE_COLUMN} FROM knowledge_exploration_checkpoints WHERE token=?`).bind(cursor));
  const committed = await db.batch(statements);
  const epochIndex = committed.length - (record ? 2 : 1);
  if ((committed[epochIndex]!.results[0] as {epoch: number} | undefined)?.epoch !== snap.epoch) conflict();
  if (record) {
    const winner = committed.at(-1)!.results[0] as {response: string | null;response_bytes:number|null} | undefined;
    if (winner?.response_bytes && winner.response_bytes > MAX_BYTES) throw new NativeBudgetExceeded('exploration replay exceeds 1 MiB');
    if(winner&&winner.response===null&&winner.response_bytes!==null)nativeUnavailable('invalid exploration winning replay storage type');
    if (!winner?.response) expired();
    validateReplay(winner.response); return winner.response;
  }
  return response;
}

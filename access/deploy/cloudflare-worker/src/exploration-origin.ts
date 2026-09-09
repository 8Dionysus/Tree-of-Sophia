import {HttpError, type Item} from './common.ts';

export const REQUEST_V2 = 'tos_exploration_request_v2';
export const RESULT_V2 = 'tos_exploration_result_v2';
export type Origin = {kind: 'node' | 'relation'; id: string; content_revision: string};
type Endpoint = {node_id: string; entity_id: string; content_revision: string};
export type ResolvedOrigin = Origin & {endpoints?: {from: Endpoint; to: Endpoint}};

function object(value: unknown): value is Item {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
function revision(value: unknown): value is string {
  return typeof value === 'string' && value.length === 64 && /^[0-9a-f]{64}$/.test(value);
}
const EDGE_WHITESPACE = /^[\p{White_Space}\u001c-\u001f]|[\p{White_Space}\u001c-\u001f]$/u;
function invalid(message: string): never { throw new HttpError(503, message); }

export function normalizeOrigin(request: Item): Origin {
  if (request.schema_version !== REQUEST_V2 || !revision(request.source_revision)) {
    throw new HttpError(400, 'exploration v2 requires its schema version and exact source_revision');
  }
  const origin = request.origin;
  if (!object(origin) || Object.keys(origin).sort().join(',') !== 'content_revision,id,kind'
      || (origin.kind !== 'node' && origin.kind !== 'relation')
      || typeof origin.id !== 'string' || !origin.id || EDGE_WHITESPACE.test(origin.id) || [...origin.id].length > 1024
      || !revision(origin.content_revision)) {
    throw new HttpError(400, 'exploration origin requires kind, exact id and content_revision');
  }
  return {kind: origin.kind, id: origin.id, content_revision: origin.content_revision};
}

// The builder owns full carrier validation. These bounded guards reject broken
// origin binding or consumed containers, without repairing source information.
export function requireOriginCarrier(value: unknown, kind: Origin['kind']): asserts value is Item {
  if (!object(value)) invalid('exploration origin carrier is not an object');
  const fields = ['id', 'native_id', 'source_graph', ...(kind === 'node'
    ? ['entity_id', 'kind_id', 'type_id'] : ['from_id', 'to_id', 'predicate_id', 'relation_type_id'])];
  for (const name of fields) if (typeof value[name] !== 'string' || !value[name]) {
    invalid(`exploration origin carrier has invalid ${name}`);
  }
  if (!revision(value.content_revision)) invalid('exploration origin carrier has invalid content_revision');
  for (const name of ['attributes', 'semantics', 'display', 'epistemic', kind === 'node' ? 'type_mapping' : 'predicate_mapping']) {
    if (!object(value[name])) invalid(`exploration origin carrier has invalid ${name} container`);
  }
  for (const name of ['source_refs', 'graph_layers', 'view_ids']) {
    const values = value[name];
    if (!Array.isArray(values) || values.some(v => typeof v !== 'string' || !v)) {
      invalid(`exploration origin carrier has invalid ${name}`);
    }
    if (name === 'source_refs' && !values.length) invalid('exploration origin carrier has no source_refs');
  }
  const semantics = value.semantics as Item;
  for (const name of ['claim', 'time', 'space', 'responsibility', 'annotation', 'language_context', 'record_version']) {
    if (Object.hasOwn(semantics, name) && !object(semantics[name])) {
      invalid(`exploration origin carrier has invalid semantics.${name}`);
    }
  }
}

// Exact indexed lookups only: an entity/native alias must never change what the
// operator selected. LIMIT 2 fails closed even for an improperly prepared table.
async function exactCarrier(db: D1Database, kind: Origin['kind'], id: string): Promise<Item | null> {
  const table = kind === 'node' ? 'knowledge_nodes' : 'knowledge_relations';
  const rows = await db.prepare(`SELECT json FROM ${table} WHERE id=? LIMIT 2`).bind(id).all<{json: string}>();
  if (rows.results.length !== 1) return null;
  let value: unknown;
  try {value = JSON.parse(rows.results[0]!.json);} catch {invalid('exploration origin carrier is not valid JSON');}
  requireOriginCarrier(value, kind);
  return value;
}

export async function bindOriginD1(db: D1Database,
  query: {source_revision: string; origin: Origin; sources: string[]}, sourceRevision: string,
): Promise<{origin: ResolvedOrigin; roots: string[]; seedRelations: string[]}> {
  if (query.source_revision !== sourceRevision) {
    throw new HttpError(409, 'exploration source revision changed; select the origin again');
  }
  const requested = query.origin, item = await exactCarrier(db, requested.kind, requested.id);
  if (!item) throw new HttpError(404, 'unknown or ambiguous exact exploration origin');
  if (item.id !== requested.id) invalid('exploration origin lookup returned a different id');
  if (item.content_revision !== requested.content_revision) throw new HttpError(409, 'exploration origin content revision changed');
  if (!query.sources.includes(String(item.source_graph))) throw new HttpError(400, 'exploration sources exclude the selected origin');
  if (requested.kind === 'node') return {origin: {...requested}, roots: [requested.id], seedRelations: []};
  const cached = new Map<string, Item>(), roots: string[] = [];
  const endpoints = {} as {from: Endpoint; to: Endpoint};
  for (const [role, field] of [['from', 'from_id'], ['to', 'to_id']] as const) {
    const id = String(item[field]);
    let node = cached.get(id);
    if (!node) {
      const found = await exactCarrier(db, 'node', id);
      if (!found) invalid('exploration relation endpoint is missing or ambiguous');
      node = found; cached.set(id, node);
    }
    if (node.id !== id) invalid('exploration endpoint lookup returned a different id');
    if (!query.sources.includes(String(node.source_graph))) throw new HttpError(400, 'exploration sources exclude an origin endpoint');
    endpoints[role] = {node_id: id, entity_id: String(node.entity_id), content_revision: String(node.content_revision)};
    if (!roots.includes(id)) roots.push(id);
  }
  return {origin: {...requested, endpoints}, roots, seedRelations: [requested.id]};
}

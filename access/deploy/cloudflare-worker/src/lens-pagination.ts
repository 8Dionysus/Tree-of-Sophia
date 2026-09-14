import type { Item, KnowledgeNode, KnowledgeRelation } from './knowledge.ts';

export type Pagination = { nodes: number; relations: number; cursor: string | null };
export class KnowledgeRevisionConflict extends Error {}

export function normalizePagination(value: unknown): Pagination | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(k => !['nodes', 'relations', 'cursor'].includes(k))) {
    throw new Error('pagination must contain only nodes, relations, cursor');
  }
  const raw = value as Item;
  const sizes: Record<'nodes' | 'relations', number> = { nodes: 40, relations: 80 };
  for (const key of ['nodes', 'relations'] as const) {
    const size = raw[key] === undefined ? sizes[key] : raw[key];
    if (typeof size !== 'number' || !Number.isInteger(size) || size < 1 || size > 100) throw new Error(`pagination.${key} must be between 1 and 100`);
    sizes[key] = size;
  }
  const cursor = raw.cursor ?? null;
  if (cursor !== null && (typeof cursor !== 'string' || !/^[A-Za-z0-9_-]{1,512}$/.test(cursor))) throw new Error('invalid lens cursor');
  return { ...sizes, cursor };
}

type Pageable = Item & { nodes: KnowledgeNode[]; relations: KnowledgeRelation[]; fingerprint: string };

// Delivery pagination only: execution is still bounded by LensSpec.limits.
// Context endpoints/focus may repeat; primary IDs and relations do not.
export function paginateLens<T extends Pageable>(result: T, options: Pagination | null): T {
  if (!options) return result;
  let nodeOffset = 0, relationOffset = 0;
  if (options.cursor !== null) {
    let token: Item;
    try {
      const parsed: unknown = JSON.parse(atob(options.cursor.replace(/-/g, '+').replace(/_/g, '/')));
      if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) throw new Error();
      token = parsed as Item;
    } catch { throw new Error('invalid lens cursor'); }
    if (Object.keys(token).sort().join(',') !== 'fingerprint,n,r,v' || token.v !== 1
      || typeof token.fingerprint !== 'string' || !/^[0-9a-f]{64}$/.test(token.fingerprint)) throw new Error('invalid lens cursor');
    if (typeof token.n !== 'number' || !Number.isInteger(token.n) || token.n < 0 || token.n > 2000
      || typeof token.r !== 'number' || !Number.isInteger(token.r) || token.r < 0 || token.r > 2000) throw new Error('invalid lens cursor position');
    if (token.fingerprint !== result.fingerprint) throw new KnowledgeRevisionConflict('lens query or snapshot changed; restart pagination');
    nodeOffset = token.n; relationOffset = token.r;
    if (nodeOffset > result.nodes.length || relationOffset > result.relations.length) throw new Error('invalid lens cursor position');
  }
  const primary = result.nodes.slice(nodeOffset, nodeOffset + options.nodes);
  const relations = result.relations.slice(relationOffset, relationOffset + options.relations);
  const primaryIds = new Set(primary.map(n => n.id));
  const ids = new Set([...primaryIds, ...relations.flatMap(r => [r.from_id, r.to_id])]);
  const focus = result.focus as Item | null;
  if (focus) ids.add(String(focus.node_id));
  const nodes = result.nodes.filter(n => ids.has(n.id));
  const nextN = nodeOffset + primary.length, nextR = relationOffset + relations.length;
  const hasMore = nextN < result.nodes.length || nextR < result.relations.length;
  const nextCursor = hasMore ? btoa(JSON.stringify({v: 1, fingerprint: result.fingerprint, n: nextN, r: nextR})).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '') : null;
  const relationIds = new Set(relations.map(r => r.id));
  const groups = (result.groups as Item[]).flatMap(group => {
    const nodeIds = (group.node_ids as string[]).filter(id => ids.has(id));
    const edgeIds = (group.relation_ids as string[]).filter(id => relationIds.has(id));
    return nodeIds.length || edgeIds.length ? [{...group, node_ids: nodeIds, relation_ids: edgeIds, node_count: nodeIds.length, relation_count: edgeIds.length}] : [];
  });
  const page = { next_cursor: nextCursor, has_more: hasMore,
    primary_node_ids: primary.map(n => n.id), context_node_ids: nodes.filter(n => !primaryIds.has(n.id)).map(n => n.id),
    returned_nodes: nodes.length, returned_relations: relations.length,
    scope: 'bounded-lens-result', counts_scope: 'complete-bounded-result' };
  const inclusion = result.inclusion as { nodes: Record<string, Item>; relations: Record<string, Item>; authority: string } | undefined;
  return {...result, nodes, relations, groups, page,
    ...(inclusion ? {inclusion: {...inclusion,
      nodes: Object.fromEntries(Object.entries(inclusion.nodes).filter(([id])=>ids.has(id))),
      relations: Object.fromEntries(Object.entries(inclusion.relations).filter(([id])=>relationIds.has(id)))}} : {})};
}

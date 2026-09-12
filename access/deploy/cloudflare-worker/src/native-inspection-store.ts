/** Exact bounded inspection over published rows; never a lens execution. */
import {HttpError} from './common.ts';
import {NativeBudgetExceeded, codePointCompare} from '../../../shared/native-semantics.ts';
import {nativeStrip} from '../../../shared/native-unicode.ts';
import {NativeD1Read, NativeD1Rows, nativeD1Limits, readNativePublication, nativeUnavailable,
  type NativeKind} from './native-d1-read.ts';
import {arrayRefs, derived, nativeField, nativePacketArray, nativePacketObject,
  stringField, type NativeRef, type NativePacket} from './native-lens.ts';

const MAX_MATCHES = 128;
const INDEXES = ['knowledge_nodes_native_idx','knowledge_nodes_entity_idx','knowledge_relations_native_idx',
  'knowledge_relations_from_seek','knowledge_relations_to_seek'];
const V9_INDEXES = ['knowledge_nodes_identity_seek','knowledge_lens_order_sort','knowledge_lens_order_from',
  'knowledge_lens_order_to','knowledge_lens_order_pair'];
const compact = (value: unknown) => JSON.stringify(value);

function identifier(value: string): string {
  const result = nativeStrip(value);
  if (!result || Array.from(result).length > 4096) throw new HttpError(400, 'knowledge identifier must contain 1 to 4096 characters');
  return result;
}
function refs(items: NativeRef[]): string[] {
  const result = new Set<string>();
  for (const item of items) {
    const source = nativeField(item, 'source_refs');
    if (Array.isArray(source.value)) for (const ref of arrayRefs(source)) {
      if (typeof ref.value === 'string' && ref.value) result.add(ref.value);
    }
  }
  return [...result].sort(codePointCompare);
}

class Inspection {
  readonly rows: NativeD1Rows;
  readonly read: NativeD1Read;
  constructor(read: NativeD1Read) {this.read = read; this.rows = new NativeD1Rows(read, read.limits, false);}
  async identities(kind: NativeKind, selector: string, args: unknown[], limit: number): Promise<string[]> {
    // Select narrow IDs once, in ABI order, before any selected body is read.
    const rows = await this.read.textRows<{id: string}>(['id'], ['id'],
      `SELECT id FROM knowledge_${kind}s WHERE ${selector} ORDER BY id LIMIT ?`, ...args, limit + 1);
    if (rows.length > limit) throw new NativeBudgetExceeded('prepared inspection has too many identity matches');
    return rows.map(row => row.id);
  }
  async items(kind: NativeKind, ids: string[]): Promise<NativeRef[]> {
    const rows = await this.rows.load(kind, ids);
    return ids.map(id => rows.get(id)!);
  }
  async node(id: string, relationLimit: number, top: NativeRef): Promise<NativePacket> {
    const exact = await this.identities('node', 'id=?', [id], 1);
    const entity = exact.length ? [] : await this.identities('node', 'entity_id=?', [id], MAX_MATCHES);
    const ids = exact.length ? exact : entity.length ? entity : await this.identities('node', 'native_id=?', [id], MAX_MATCHES);
    if (!ids.length) throw new HttpError(404, `unknown ToS knowledge node: ${id}`);
    const matches = await this.items('node', ids), encoded = compact(ids);
    const incident = 'SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id IN (SELECT value FROM json_each(?)) '
      + 'UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id IN (SELECT value FROM json_each(?))';
    const counts = await this.read.query<{total: number}>(`SELECT count(*) AS total FROM (${incident})`, encoded, encoded);
    const total = counts[0]?.total;
    if (!Number.isSafeInteger(total) || total! < 0) nativeUnavailable('prepared relation count unavailable');
    const selectedIds = await this.read.textRows<{id: string}>(['id'], ['id'],
      `SELECT id FROM (${incident}) ORDER BY id LIMIT ?`, encoded, encoded, relationLimit);
    const selected = await this.items('relation', selectedIds.map(row => row.id));
    return nativePacketObject([
      ['schema','tos_knowledge_node_packet_v1'], ['source_revision',nativeField(top,'source_revision')],
      ['requested_id',id], ['ambiguous_native_id',!exact.length && !entity.length && matches.length > 1],
      ['shared_entity_id',entity.length > 1], ['matches',nativePacketArray(matches)],
      ['related_relations',nativePacketArray(selected)],
      ['counts',derived({matches:matches.length,related_relations:total,returned_relations:selected.length})],
      ['source_refs',derived(refs([...matches,...selected]))], ['authority_boundary',nativeField(top,'authority_boundary')],
    ]);
  }
  async relation(id: string, top: NativeRef): Promise<NativePacket> {
    const exact = await this.identities('relation', 'id=?', [id], 1);
    const ids = exact.length ? exact : await this.identities('relation', 'native_id=?', [id], MAX_MATCHES);
    if (!ids.length) throw new HttpError(404, `unknown ToS knowledge relation: ${id}`);
    const matches = await this.items('relation', ids);
    const endpointIds = [...new Set(matches.flatMap(item => [stringField(item,'from_id'),stringField(item,'to_id')]))].sort(codePointCompare);
    // Exact primary keys give closure checking without fetching or parsing any
    // selected relation body twice. A missing endpoint is damaged publication.
    const endpoints = await this.items('node', endpointIds);
    return nativePacketObject([
      ['schema','tos_knowledge_relation_packet_v1'], ['source_revision',nativeField(top,'source_revision')],
      ['requested_id',id], ['ambiguous_native_id',!exact.length && matches.length > 1],
      ['matches',nativePacketArray(matches)], ['endpoints',nativePacketArray(endpoints)],
      ['counts',derived({matches:matches.length,endpoints:endpoints.length})],
      ['source_refs',derived(refs([...matches,...endpoints]))], ['authority_boundary',nativeField(top,'authority_boundary')],
    ]);
  }
}

export async function inspectNativeD1(db: D1Database, kind: NativeKind, requested: string,
  relationLimit = 200, expectedRevision?: string): Promise<NativePacket> {
  const id = identifier(requested);
  if (!Number.isSafeInteger(relationLimit) || relationLimit < 0 || relationLimit > 1000) throw new HttpError(400, 'relation_limit must be an integer between 0 and 1000');
  try {
    const read = new NativeD1Read(db, nativeD1Limits, true);
    const top = await readNativeInspectionPublication(read, expectedRevision);
    const inspector = new Inspection(read);
    return kind === 'node' ? await inspector.node(id, relationLimit, top.ref) : await inspector.relation(id, top.ref);
  } catch (error) {
    if (error instanceof HttpError || error instanceof NativeBudgetExceeded) throw error;
    return nativeUnavailable('prepared inspection publication unavailable or invalid');
  }
}

/** Common published read header/index admission, without inspection or lenses. */
export async function readNativeInspectionPublication(read: NativeD1Read, expectedRevision?: string): Promise<{raw:string;ref:NativeRef}> {
  const top = await readNativePublication(read, expectedRevision, 'inspection');
  const required = [...INDEXES, ...(nativeField(top.ref,'read_model_schema').value === 'tos_cloudflare_edge_read_model_v9' ? V9_INDEXES : [])];
  const found = await read.textRows<{name:string}>(['name'],['name'],
    "SELECT name FROM sqlite_master WHERE type='index' AND name IN (SELECT value FROM json_each(?))",compact(required));
  if (found.length !== required.length || found.some(row => !required.includes(row.name))) nativeUnavailable('prepared reader adjacency/identity migration is unavailable');
  return top;
}

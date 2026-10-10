/** Physical bounded D1 inspection reads and shared publication admission. */
import {NativeBudgetExceeded} from '../../../shared/native-semantics.ts';
import {NativeD1Read, NativeD1Rows, readNativePublication, nativeUnavailable,
  type NativeKind} from './native-d1-read.ts';
import {nativeField, type NativeRef} from './native-lens.ts';
import type {InspectionNeed} from './selected-inspection-runtime.ts';

const MAX_MATCHES = 128;
const INDEXES = ['knowledge_nodes_native_idx','knowledge_nodes_entity_idx','knowledge_relations_native_idx',
  'knowledge_relations_from_seek','knowledge_relations_to_seek'];
const V9_INDEXES = ['knowledge_nodes_identity_seek','knowledge_lens_order_sort','knowledge_lens_order_from',
  'knowledge_lens_order_to','knowledge_lens_order_pair'];
const compact = (value: unknown) => JSON.stringify(value);

/** Execute only the concrete physical needs selected by the shared Rust plan.
 * Full emitted rows pass the existing SQL byte/digest/identity admission before
 * entering this raw array. Concatenation preserves original carrier lexemes;
 * JSON.stringify is used only for bound ID arrays, never for carrier bodies. */
export class InspectionD1Transport {
  private readonly rows:NativeD1Rows;
  private readonly encoder=new TextEncoder();
  private readonly read:NativeD1Read;
  constructor(read:NativeD1Read) {this.read=read;this.rows=new NativeD1Rows(read,read.limits,false);}
  private async payloads(kind:NativeKind,ids:string[]):Promise<Uint8Array> {
    const raw=new Map<string,string>();
    await this.rows.load(kind,ids,(id,body)=>raw.set(id,body));
    return this.encoder.encode('['+ids.map(id=>{
      const body=raw.get(id);if(body===undefined)nativeUnavailable('prepared inspection payload closure unavailable');
      return body;
    }).join(',')+']');
  }
  async lookup(need:Extract<InspectionNeed,{operation:'lookup'}>):Promise<Uint8Array> {
    if(!['node','relation'].includes(need.kind)||!['id','entity_id','native_id'].includes(need.selector)
      ||!Number.isSafeInteger(need.limit)||need.limit<1||need.limit>MAX_MATCHES
      ||!need.identifier.isWellFormed())nativeUnavailable('prepared inspection physical lookup invalid');
    const found=await this.read.textRows<{id:string}>(['id'],['id'],
      `SELECT id FROM knowledge_${need.kind}s WHERE ${need.selector}=? ORDER BY id LIMIT ?`,need.identifier,need.limit+1);
    // The Rust need owns the admitted whole alias count. Refuse its lookahead
    // before reading any full rows or digest metadata for a truncated set.
    if(found.length>need.limit)throw new NativeBudgetExceeded('prepared inspection has too many identity matches');
    return this.payloads(need.kind,found.map(row=>row.id));
  }
  async incident(need:Extract<InspectionNeed,{operation:'incident'}>):Promise<{total:bigint;rows:Uint8Array}> {
    const encoded=compact(need.ids);
    const incident='SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id IN (SELECT value FROM json_each(?)) '
      +'UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id IN (SELECT value FROM json_each(?))';
    const counts=await this.read.query<{total:number}>(`SELECT count(*) AS total FROM (${incident})`,encoded,encoded);
    const total=counts[0]?.total;
    if(!Number.isSafeInteger(total)||total!<0)nativeUnavailable('prepared relation count unavailable');
    const selected=await this.read.textRows<{id:string}>(['id'],['id'],
      `SELECT id FROM (${incident}) ORDER BY id LIMIT ?`,encoded,encoded,need.relation_limit);
    return {total:BigInt(total!),rows:await this.payloads('relation',selected.map(row=>row.id))};
  }
  endpoints(need:Extract<InspectionNeed,{operation:'endpoints'}>):Promise<Uint8Array> {
    return this.payloads('node',need.ids);
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

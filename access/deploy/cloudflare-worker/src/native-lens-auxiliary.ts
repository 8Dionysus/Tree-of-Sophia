/** Optional, independently versioned lens stores bound to the exact v9 epoch. */
import {HttpError} from './common.ts';
import {codePointCompare, nativeLower} from '../../../shared/native-semantics.ts';
import {derived, nativeChild, nativePacketJson, type NativeRef, type NativeSpec, type NativeGroup} from './native-lens.ts';
import {NativeD1Read, nativeSha256, nativeUnavailable} from './native-d1-read.ts';

type Kind = 'node' | 'relation';
type Where = {sql: string; args: unknown[]};
const TABLE = 'knowledge_lens_memberships';
const ORDER = 'knowledge_lens_memberships_order';
const OMITTED = ['attributes','source_record','readable_context','semantics.claim.source_canonical_json'];
const STATES = {compact: ['knowledge_compact_lens_state','tos_compact_lens_carrier_v1'],
  membership: ['knowledge_lens_membership_state','tos_lens_membership_index_v1']} as const;

export function compactCovered(spec: NativeSpec): boolean {
  if (spec.detail !== 'compact' || spec.seed.text_query) return false;
  const fields = [...spec.composition.group_by, ...spec.composition.sort_nodes.map(rule=>rule.field), ...spec.composition.sort_relations.map(rule=>rule.field)];
  const groups = [spec.node_query, spec.relation_query, ...spec.path_query.flatMap(path=>path.steps.flatMap(step=>[step.node_query,step.relation_query]))];
  for (const group of groups) for (const rule of group.filters) {if (!rule.field) return false; fields.push(rule.field);}
  return fields.every(field=>!OMITTED.some(omitted=>field===omitted || field.startsWith(omitted+'.') || omitted.startsWith(field+'.')));
}

export async function readLensPublicationBinding(read: NativeD1Read, top: {raw: string; ref: NativeRef}) {
  const clock=await read.query<{epoch:number; kind:string}>('SELECT epoch,typeof(epoch) AS kind FROM knowledge_exploration_clock WHERE singleton=1 LIMIT 2');
  if (clock.length!==1 || clock[0]!.kind!=='integer' || !Number.isSafeInteger(clock[0]!.epoch) || clock[0]!.epoch<0) nativeUnavailable('lens publication epoch invalid');
  return derived({schema:'tos_published_knowledge_snapshot_v1',publication_epoch:clock[0]!.epoch,
    metadata_sha256:await nativeSha256(top.raw),...Object.fromEntries(['read_model_schema','source_revision','data_revision','graph_schema','normalization_binding'].map(key=>[key,nativeChild(top.ref,key)]))});
}

export async function admitAuxiliary(read: NativeD1Read, top: {raw: string; ref: NativeRef}, requested: (keyof typeof STATES)[]) {
  if (!requested.length) return {installed:requested,verify:async()=>{}};
  const names=requested.map(key=>STATES[key][0]);
  const found=await read.textRows<{name:string}>(['name'],['name'],
    "SELECT name FROM sqlite_master WHERE type='table' AND name IN (SELECT value FROM json_each(?))",JSON.stringify(names));
  const installed=requested.filter(key=>found.some(row=>row.name===STATES[key][0]));
  if (!installed.length) return {installed, verify:async()=>{}};
  const expected=nativePacketJson(await readLensPublicationBinding(read,top));
  const verify=async (initial=false)=>{
    for (const key of installed) {
      const [table,schema]=STATES[key];
      const rows=await read.query<{schema:unknown; binding:unknown; valid:unknown}>(
        `SELECT CASE WHEN typeof(schema)='text' AND length(schema)<=128 THEN schema END AS schema,
        CASE WHEN typeof(binding)='text' AND length(CAST(binding AS BLOB))<=65536 THEN binding END AS binding,
        CASE WHEN typeof(valid)='integer' THEN valid END AS valid FROM ${table} WHERE singleton=1 LIMIT 2`);
      if (rows.length!==1 || rows[0]!.schema!==schema || rows[0]!.binding!==expected || rows[0]!.valid!==1) {
        if (initial) nativeUnavailable('native lens auxiliary store stale or incompatible: '+key);
        throw new HttpError(409,'native lens auxiliary store changed during query');
      }
    }
  };
  await verify(true);
  return {installed,verify:()=>verify(false)};
}

type Term = {field:string; mode:'all'|'any'; values:string[]};
export class NativeMembershipPlan {
  readonly drivers: [string,string][];
  readonly kind:Kind; readonly mode:'all'|'any'; readonly terms:Term[];
  constructor(kind:Kind, mode:'all'|'any', terms:Term[]) {
    this.kind=kind;this.mode=mode;this.terms=terms;
    const unique=new Map<string,[string,string]>();
    for (const term of (mode==='all'?terms.slice(0,1):terms)) for (const value of (term.mode==='all'?term.values.slice(0,1):term.values)) unique.set(JSON.stringify([term.field,value]),[term.field,value]);
    this.drivers=[...unique.values()];
  }
  condition(alias:string):Where {
    const args:unknown[]=[];
    const groups=this.terms.map(term=>'('+term.values.map(value=>{
      args.push(this.kind,term.field,value);
      return `EXISTS (SELECT 1 FROM ${TABLE} m WHERE m.kind=? AND m.field=? AND m.value=? AND m.id=${alias}.id)`;
    }).join(term.mode==='all'?' AND ':' OR ')+')');
    return {sql:'('+groups.join(this.mode==='all'?' AND ':' OR ')+')',args};
  }
  async count(read:NativeD1Read,where:Where):Promise<number> {
    const branches=this.drivers.map(()=>`SELECT id FROM ${TABLE} WHERE kind=? AND field=? AND value=?`);
    const rows=await read.query<{total:number}>('WITH candidates AS ('+branches.join(' UNION ')+`) SELECT count(*) AS total FROM candidates c CROSS JOIN knowledge_${this.kind}s r ON r.id=c.id WHERE ${where.sql}`,
      ...this.drivers.flatMap(([field,value])=>[this.kind,field,value]),...where.args);
    const total=rows[0]?.total;
    if (rows.length!==1 || typeof total!=='number' || !Number.isSafeInteger(total) || total<0) nativeUnavailable('native membership count invalid');
    return total;
  }
  async *ordered(read:NativeD1Read,where:Where,block:number) {
    let after=['',''];
    while (true) {
      const args:unknown[]=[];
      const branches=this.drivers.map(([field,value])=>{
        args.push(this.kind,field,value,...after,...where.args,block);
        return `SELECT id,sort_key FROM (SELECT m.id,m.sort_key FROM ${TABLE} m INDEXED BY ${ORDER} CROSS JOIN knowledge_${this.kind}s r ON r.id=m.id WHERE m.kind=? AND m.field=? AND m.value=? AND (m.sort_key,m.id)>(?,?) AND ${where.sql} ORDER BY m.sort_key,m.id LIMIT ?)`;
      });
      const endpoints=this.kind==='relation'?'r.from_id,r.to_id':"'' AS from_id,'' AS to_id";
      const rows=await read.textRows<{id:string;sort_key:string;from_id:string;to_id:string}>(['id','sort_key','from_id','to_id'],['sort_key','id'],
        'WITH candidates AS ('+branches.join(' UNION ')+`) SELECT c.id,c.sort_key,${endpoints} FROM candidates c CROSS JOIN knowledge_${this.kind}s r ON r.id=c.id ORDER BY c.sort_key,c.id LIMIT ?`,...args,block);
      if (!rows.length) return;
      for (const row of rows) {if (row.sort_key!==nativeLower(row.id)) nativeUnavailable('native membership order differs');yield row;}
      after=[rows.at(-1)!.sort_key,rows.at(-1)!.id];
    }
  }
}

export function compileMembership(kind:Kind,group:NativeGroup):NativeMembershipPlan|null {
  if (!group.enabled || !group.filters.length) return null;
  const terms:Term[]=[];
  for (const rule of group.filters) {
    if (rule._property_binding || !['view_ids','graph_layers'].includes(rule.field??'') || !['eq','in','contains'].includes(rule.op)) return null;
    const value=rule.valueRef.value;
    if (rule.op==='eq' && typeof value!=='string') return null;
    const values=Array.isArray(value)?value:[value];
    if (!values.length || values.some(entry=>typeof entry!=='string')) return null;
    terms.push({field:rule.field!,mode:rule.op==='contains'?'all':'any',values:[...new Set(values as string[])].sort(codePointCompare)});
  }
  const plan=new NativeMembershipPlan(kind,group.match,terms), predicates=terms.reduce((sum,term)=>sum+term.values.length,0);
  // D1's parameter ceiling is lower than local SQLite. Keep a bounded native
  // fallback rather than constructing an unserviceable expanded statement.
  if (predicates>32 || plan.drivers.length>16 || plan.drivers.length*(3*predicates+10)+6>100) return null;
  return plan;
}

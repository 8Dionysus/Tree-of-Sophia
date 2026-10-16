/** Published D1 custody and physical reads for the shared Rust lens plan.
 * Rust owns semantics, eligibility, traversal and complete packet construction.
 */
import {nativeLower, nativeUnicodeVersion, codePointCompare} from '../../../shared/native-semantics.ts';
import {arrayRefs, nativeField, nativeKeys, nativePacketJson, stringField, type NativeRef} from './native-lens.ts';
import type {LensHeaderQuery,LensNeed,LensHeader} from './selected-lens-runtime.ts';
import {NativeD1Read as Read, NativeD1Rows, nativeBytes as bytes, nativeSha256 as sha256,
  nativeUnavailable as unavailable} from './native-d1-read.ts';

const compact = (value: unknown) => JSON.stringify(value);
type Kind = 'node' | 'relation';
const DIMENSIONS = {node: ['source_graph', 'kind_id', 'type_id'], relation: ['source_graph', 'predicate_id', 'relation_type_id']} as const;
type IdentityField = 'id' | 'entity_id' | 'native_id';
const IDENTITY_INDEXES: Record<Kind, Partial<Record<IdentityField, string>>> = {
  node: {id: 'sqlite_autoindex_knowledge_nodes_1', entity_id: 'knowledge_nodes_identity_seek', native_id: 'knowledge_nodes_native_idx'},
  relation: {id: 'sqlite_autoindex_knowledge_relations_1', native_id: 'knowledge_relations_native_idx'},
};

const INDEXES = ['knowledge_lens_order_sort', 'knowledge_lens_order_from', 'knowledge_lens_order_to', 'knowledge_lens_order_pair',
  'knowledge_nodes_source_kind_idx', 'knowledge_relations_source_predicate_idx'];

/** Static SQL producer for concrete needs selected by the Rust lens plan.
 * The Rust plan owns its sole bounded parsed-row cache. This producer retains
 * only transient integrity references before returning original carrier bytes. */
export class PublishedLensD1Transport {
  private readonly rows:NativeD1Rows;
  private readonly read:Read;
  private readonly encoder=new TextEncoder();
  private compactInstalled=false;
  constructor(read:Read) {
    this.read=read;
    this.rows=new NativeD1Rows(read,read.limits,true,false);
  }
  setStores(stores:{compact:boolean;membership:boolean}):void {this.compactInstalled=stores.compact;}
  async payloads(need:Extract<LensNeed,{operation:'rows'}>):Promise<{rows:Uint8Array;sizes:number[]}> {
    if(need.representation==='covered_compact'&&!this.compactInstalled)unavailable('covered compact lens store unavailable');
    this.rows.compact=need.representation==='covered_compact';
    const raw=new Map<string,string>();
    await this.rows.load(need.kind,need.ids,(id,body)=>raw.set(id,body));
    const bodies=need.ids.map(id=>{const body=raw.get(id);if(body===undefined)unavailable('published lens payload closure unavailable');return body;});
    return {rows:this.encoder.encode('['+bodies.join(',')+']'),sizes:bodies.map(bytes)};
  }
  private scope(sources:string[],alias:string):{sql:string;args:unknown[]} {
    return {sql:`${alias}.source_graph IN (SELECT value FROM json_each(?))`,args:[compact(sources)]};
  }
  async candidates(need:Extract<LensNeed,{operation:'candidates'}>):Promise<{id:string}[]> {
    const kind=need.kind,alias=kind==='node'?'n':'r',scope=this.scope(need.sources,alias),after=need.after?.id??'';
    if(need.after&&need.after.kind!=='id')unavailable('published lens candidate cursor incompatible');
    const groups=need.identities.map(group=>({all:group.all,terms:group.terms.map(term=>{
      const index=IDENTITY_INDEXES[kind][term.field];if(!index)unavailable('published lens identity index incompatible');
      return {...term,index};
    })}));
    const conditions:string[]=[],conditionArgs:unknown[]=[];
    for(const group of groups) {
      conditions.push('('+group.terms.map(term=>{
        // An empty declared exact set compiles to the predecessor's constant
        // false SQL predicate; it has no addressed values or bound parameter.
        if(!term.values.length)return '0';
        conditionArgs.push(compact(term.values));return `${alias}.${term.field} IN (SELECT value FROM json_each(?))`;
      }).join(group.all?' AND ':' OR ')+')');
    }
    const where=scope.sql+(conditions.length?' AND '+conditions.join(' AND '):'');
    if(need.index.kind==='union') {
      const branches:string[]=[],args:unknown[]=[];
      for(const term of groups[0]?.terms??[]) {
        if(!term.values.length)continue;
        branches.push(`SELECT id FROM (SELECT ${alias}.id FROM knowledge_${kind}s ${alias} INDEXED BY ${term.index} WHERE ${alias}.${term.field} IN (SELECT value FROM json_each(?)) AND ${alias}.id>? AND ${where} ORDER BY ${alias}.id LIMIT ?)`);
        args.push(compact(term.values),after,...scope.args,...conditionArgs,need.limit);
      }
      if(!branches.length)return [];
      return this.read.textRows(['id'],['id'],'WITH candidates AS ('+branches.join(' UNION ')+') SELECT id FROM candidates ORDER BY id LIMIT ?',...args,need.limit);
    }
    const index=need.index.kind==='identity'?IDENTITY_INDEXES[kind][need.index.field]
      :kind==='node'?'knowledge_nodes_source_kind_idx':'knowledge_relations_source_predicate_idx';
    if(!index)unavailable('published lens candidate index incompatible');
    return this.read.textRows(['id'],['id'],`SELECT ${alias}.id FROM knowledge_${kind}s ${alias} INDEXED BY ${index} WHERE ${where} AND ${alias}.id>? ORDER BY ${alias}.id LIMIT ?`,...scope.args,...conditionArgs,after,need.limit);
  }
  focus(need:Extract<LensNeed,{operation:'focus'}>):Promise<{id:string}[]> {
    const index=IDENTITY_INDEXES.node[need.field];if(!index)unavailable('published lens focus index incompatible');
    const scope=this.scope(need.sources,'n');
    // Priority order is an explicit trusted published-v7 Rust vocabulary term.
    const order=need.field==='entity_id'?"coalesce((SELECT CAST(key AS INTEGER) FROM json_each(?) WHERE value=n.source_graph),99),n.id":'n.id';
    return this.read.textRows(['id'],['id'],`SELECT n.id FROM knowledge_nodes n INDEXED BY ${index} WHERE ${scope.sql} AND n.${need.field}=? ORDER BY ${order} LIMIT ?`,...scope.args,need.identifier,...(need.field==='entity_id'?[compact(need.source_priority)]:[]),need.limit);
  }
  async incident(need:Extract<LensNeed,{operation:'incident'}>):Promise<{id:string}[]> {
    const branches:string[]=[],args:unknown[]=[];
    for(const side of ['from','to'] as const) {
      branches.push(`SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_${side}_seek WHERE ${side}_id=? AND id>? ORDER BY id LIMIT ?)`);
      args.push(need.identifier,need.after,need.limit);
    }
    return this.read.textRows(['id'],['id'],'WITH incident AS ('+branches.join(' UNION ')+') SELECT id FROM incident ORDER BY id LIMIT ?',...args,need.limit);
  }
  aliases(need:Extract<LensNeed,{operation:'aliases'}>):Promise<{id:string}[]> {
    const scope=this.scope(need.sources,'n');
    return this.read.textRows(['id'],['id'],'SELECT n.id FROM knowledge_nodes n INDEXED BY knowledge_nodes_identity_seek WHERE n.entity_id IN (SELECT value FROM json_each(?)) AND '+scope.sql+' AND n.id NOT IN (SELECT value FROM json_each(?)) ORDER BY n.id LIMIT ?',compact(need.entities),...scope.args,compact(need.exclude),need.limit);
  }
  sources(need:Extract<LensNeed,{operation:'sources'}>):Promise<{id:string;source_graph:string}[]> {
    if(!need.ids.length)return Promise.resolve([]);
    return this.read.textRows(['id','source_graph'],['id'],'SELECT id,source_graph FROM knowledge_nodes WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?',compact(need.ids),need.ids.length+1);
  }
  private condition(query:LensHeaderQuery,alias:string):{sql:string;args:unknown[]} {
    const scope=this.scope(query.sources,alias),args=[...scope.args];let sql=scope.sql;
    if(query.dimensions!==undefined) {
      sql+=' AND EXISTS (SELECT 1 FROM json_each(?) cell WHERE '+DIMENSIONS[query.kind].map((field,i)=>`${alias}.${field}=json_extract(cell.value,'$[${i}]')`).join(' AND ')+')';args.push(compact(query.dimensions));
    }
    if(query.predicate_ids.length){sql+=` AND ${alias}.predicate_id IN (SELECT value FROM json_each(?))`;args.push(compact(query.predicate_ids));}
    for(const [field,values] of [['predicate_id',query.excluded_predicates],['relation_type_id',query.excluded_relation_types]] as const)if(values.length){sql+=` AND ${alias}.${field} NOT IN (SELECT value FROM json_each(?))`;args.push(compact(values));}
    if(query.membership) {
      const groups=query.membership.terms.map(term=>'('+term.values.map(value=>{
        args.push(query.kind,term.field,value);return `EXISTS (SELECT 1 FROM knowledge_lens_memberships m WHERE m.kind=? AND m.field=? AND m.value=? AND m.id=${alias}.id)`;
      }).join(term.all?' AND ':' OR ')+')');
      sql+=' AND ('+groups.join(query.membership.all?' AND ':' OR ')+')';
    }
    return {sql,args};
  }
  private eligible(query:LensHeaderQuery):{sql:string;args:unknown[]}|undefined {
    const eligible=query.eligible;if(!eligible)return undefined;
    const basis=compact(eligible.basis),traversed=compact(eligible.traversed);
    const sql=eligible.policy==='both'&&eligible.pair_index
      ?"SELECT l.id FROM json_each(?) a CROSS JOIN json_each(?) b CROSS JOIN knowledge_lens_order l INDEXED BY knowledge_lens_order_pair WHERE l.kind='relation' AND l.from_id=a.value AND l.to_id=b.value UNION SELECT value AS id FROM json_each(?)"
      :eligible.policy==='both'
      ?'SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id IN (SELECT value FROM json_each(?)) AND to_id IN (SELECT value FROM json_each(?)) UNION SELECT value AS id FROM json_each(?)'
      :'SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id IN (SELECT value FROM json_each(?)) UNION SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id IN (SELECT value FROM json_each(?)) UNION SELECT value AS id FROM json_each(?)';
    return {sql,args:[basis,basis,traversed]};
  }
  private membershipCandidates(query:LensHeaderQuery):{sql:string;args:unknown[]} {
    const drivers=query.membership!.drivers;
    return {sql:drivers.map(()=>`SELECT id FROM knowledge_lens_memberships WHERE kind=? AND field=? AND value=?`).join(' UNION '),args:drivers.flatMap(([field,value])=>[query.kind,field,value])};
  }
  async count(need:Extract<LensNeed,{operation:'count'}>):Promise<number> {
    const query=need.query,where=this.condition(query,'r'),selected=this.eligible(query)??(query.membership?this.membershipCandidates(query):undefined);
    const rows=await this.read.query<{total:number}>(selected
      ?`WITH candidates AS (${selected.sql}) SELECT count(*) AS total FROM candidates c CROSS JOIN knowledge_${query.kind}s r ON r.id=c.id WHERE ${where.sql}`
      :`SELECT count(*) AS total FROM knowledge_${query.kind}s r WHERE ${where.sql}`,...(selected?.args??[]),...where.args);
    const total=rows[0]?.total;if(rows.length!==1||!Number.isSafeInteger(total)||total!<0)unavailable('published lens count invalid');return total!;
  }
  async ordered(need:Extract<LensNeed,{operation:'ordered'}>):Promise<LensHeader[]> {
    const query=need.query,kind=query.kind,where=this.condition(query,'r'),after=need.after??['',''],eligible=this.eligible(query);
    let sql:string,args:unknown[];
    const endpoint=kind==='relation'?'r.from_id,r.to_id':"'' AS from_id,'' AS to_id";
    if(eligible) {
      sql=`WITH eligible AS (${eligible.sql}) SELECT r.id,r.from_id,r.to_id,l.sort_key,l.from_id AS order_from,l.to_id AS order_to FROM eligible e CROSS JOIN knowledge_lens_order l INDEXED BY sqlite_autoindex_knowledge_lens_order_1 ON l.kind='relation' AND l.id=e.id CROSS JOIN knowledge_relations r ON r.id=e.id WHERE ${where.sql} AND (l.sort_key,l.id)>(?,?) ORDER BY l.sort_key,l.id LIMIT ?`;
      args=[...eligible.args,...where.args,...after,need.limit];
    } else if(query.membership&&!query.endpoint) {
      const branches:string[]=[],bindings:unknown[]=[];
      for(const [field,value] of query.membership.drivers) {
        branches.push(`SELECT id,sort_key FROM (SELECT m.id,m.sort_key FROM knowledge_lens_memberships m INDEXED BY knowledge_lens_memberships_order CROSS JOIN knowledge_${kind}s r ON r.id=m.id WHERE m.kind=? AND m.field=? AND m.value=? AND (m.sort_key,m.id)>(?,?) AND ${where.sql} ORDER BY m.sort_key,m.id LIMIT ?)`);
        bindings.push(kind,field,value,...after,...where.args,need.limit);
      }
      sql='WITH candidates AS ('+branches.join(' UNION ')+`) SELECT c.id,c.sort_key,${endpoint},${kind==='relation'?'r.from_id':'\'\''} AS order_from,${kind==='relation'?'r.to_id':'\'\''} AS order_to FROM candidates c CROSS JOIN knowledge_${kind}s r ON r.id=c.id ORDER BY c.sort_key,c.id LIMIT ?`;
      args=[...bindings,need.limit];
    } else {
      const index=query.endpoint?'knowledge_lens_order_'+query.endpoint.side:'knowledge_lens_order_sort';
      sql=`SELECT l.id,l.sort_key,${endpoint},l.from_id AS order_from,l.to_id AS order_to FROM knowledge_lens_order l INDEXED BY ${index} CROSS JOIN knowledge_${kind}s r ON r.id=l.id WHERE l.kind=?${query.endpoint?' AND l.'+query.endpoint.side+'_id=?':''} AND (l.sort_key,l.id)>(?,?) AND ${where.sql} ORDER BY l.sort_key,l.id LIMIT ?`;
      args=[kind,...(query.endpoint?[query.endpoint.id]:[]),...after,...where.args,need.limit];
    }
    const rows=await this.read.textRows<LensHeader&{order_from:string;order_to:string}>(['id','sort_key','from_id','to_id','order_from','order_to'],['sort_key','id'],sql,...args);
    for(const row of rows)if(row.sort_key!==nativeLower(row.id)||row.from_id!==row.order_from||row.to_id!==row.order_to)unavailable('published lens ordered key or endpoints differ');
    return rows;
  }
}

/** Physical publication admission shared by the maintained lens reader and
 * its Rust continuation. Metadata bytes, checksum, index framing, and the
 * published execution version stay with the D1 producer. */
export async function readPublishedLensMetadata(read: Read, top: {raw: string; ref: NativeRef}): Promise<{raw: string; ref: NativeRef}> {
  const sourceRevision = stringField(top.ref, 'source_revision'), metadata = await read.metadata('knowledge_lens_top', 1048576);
  if (nativeField(top.ref, 'lens_sha256').value !== await sha256(metadata.raw)) unavailable('native lens metadata checksum differs');
  const lensKeys = ['schema','execution_version','source_revision','sort_key','unicode_version','query_properties','node_counts','relation_counts'];
  if ([...nativeKeys(metadata.ref)].sort().join(',') !== lensKeys.sort().join(',')) unavailable('native lens metadata framing invalid');
  if (nativeField(metadata.ref, 'schema').value !== 'tos_published_lens_metadata_v1' || nativeField(metadata.ref, 'execution_version').value !== 'tos-lens-execution-v7' || nativeField(metadata.ref, 'source_revision').value !== sourceRevision || nativeField(metadata.ref, 'sort_key').value !== 'python-str-or-empty-lower-v1' || nativeField(metadata.ref, 'unicode_version').value !== nativeUnicodeVersion) unavailable('native lens metadata incompatible');
  for (const name of ['node_counts', 'relation_counts']) {
    const refs = arrayRefs(nativeField(metadata.ref, name)); let previous: string[] | null = null;
    if (refs.length > 16384) unavailable('native lens histogram too large');
    for (const ref of refs) {
      const cell = arrayRefs(ref), key = cell.slice(0, 3).map(r => r.value as string), count = cell[3];
      if (cell.length !== 4 || key.some(k => typeof k !== 'string' || !k) || !count || typeof count.value !== 'number' || !Number.isSafeInteger(count.value) || count.value < 1 || /[.eE]/.test(nativePacketJson(count))) unavailable('native lens histogram cell invalid');
      if (previous) {let delta = 0; for (let i = 0; i < 3 && !delta; i++) delta = codePointCompare(previous[i]!, key[i]!); if (delta >= 0) unavailable('native lens histogram cells duplicated or unordered');}
      previous = key;
    }
  }
  const indexes = await read.textRows<{name: string}>(['name'], ['name'], "SELECT name FROM sqlite_master WHERE type='index' AND name IN (SELECT value FROM json_each(?))", compact(INDEXES));
  if (new Set(indexes.map(row => row.name)).size !== INDEXES.length) unavailable('native lens ordered-index migration unavailable');
  const definitions = nativeField(metadata.ref, 'query_properties').value;
  if (!Array.isArray(definitions) || definitions.length > 4096) unavailable('native lens query property metadata invalid');
  if (definitions.some(definition => !definition || typeof definition !== 'object' || Array.isArray(definition)
    || ['property_id','field','value_type'].some(key => typeof definition[key] !== 'string' || !definition[key]) || typeof definition.inherited !== 'boolean'
    || ['applies_to','operators'].some(key => !Array.isArray(definition[key]) || definition[key].some((value: unknown) => typeof value !== 'string' || !value)))) unavailable('native lens query property framing invalid');
  return metadata;
}

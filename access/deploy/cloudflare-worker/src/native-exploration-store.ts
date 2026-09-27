/** Request-local native payloads; disposable traversal state contains IDs only. */
import {HttpError, type Item} from './common.ts';
import {NativeD1Read, NativeD1Rows, nativeD1Limits, nativeUnavailable, type NativeKind} from './native-d1-read.ts';
import {nativeField, stringField, type NativeRef} from './native-lens.ts';
import type {ExplorationNeed} from './selected-exploration-runtime.ts';

export class NativeExplorationRows {
  readonly read: NativeD1Read; readonly rows: NativeD1Rows;
  constructor(read: NativeD1Read) {this.read = read; this.rows = new NativeD1Rows(read, nativeD1Limits, false);}
  async exact(kind: NativeKind, id: string): Promise<Item | null> {
    const found = await this.read.textRows<{id:string}>(['id'],['id'],
      `SELECT id FROM knowledge_${kind}s WHERE id=? ORDER BY id LIMIT 2`,id);
    return found.length === 1 ? (await this.rows.get(kind,id)).value as Item : null;
  }
  async focus(id: string, sources: string[]): Promise<string> {
    const allowed = JSON.stringify(sources);
    for (const [selector,order,limit] of [
      ['id=?','id',1],
      ['entity_id=?',"CASE source_graph WHEN 'source-navigation' THEN 0 WHEN 'canon' THEN 1 WHEN 'source-claims' THEN 2 WHEN 'philosophy' THEN 3 WHEN 'candidate-intake' THEN 4 WHEN 'repository' THEN 5 WHEN 'semantic-interchange' THEN 6 ELSE 99 END,id",1],
      ['native_id=?','id',2],
    ] as const) {
      const found = await this.read.textRows<{id:string}>(['id'],['id'],
        `SELECT id FROM knowledge_nodes WHERE ${selector} AND source_graph IN (SELECT value FROM json_each(?)) ORDER BY ${order} LIMIT ${limit}`,id,allowed);
      if (found.length > 1) throw new HttpError(400,'ambiguous ToS knowledge focus; use a namespaced node id');
      if (found.length) {await this.rows.get('node',found[0]!.id); return found[0]!.id;}
    }
    throw new HttpError(400,'unknown ToS knowledge focus');
  }
  async selected(kind: NativeKind, ids: Iterable<string>): Promise<Map<string,NativeRef>> {return this.rows.load(kind,ids);}
  async identities(sql: string, current: string, expanded: string[], after: string, sources: string[]) {
    const found = await this.read.textRows<{id:string;entity_id:string}>(['id','entity_id'],['id'],
      sql,current,JSON.stringify(expanded),after,JSON.stringify(sources));
    const refs = await this.rows.load('node',found.map(row=>row.id));
    for (const row of found) if (nativeField(refs.get(row.id)!,'entity_id').value !== row.entity_id) nativeUnavailable('exploration identity index differs from payload');
    return found;
  }
  async adjacent(sql: string, current: string, after: string) {
    const found = await this.read.textRows<{id:string}>(['id'],['id'],sql,current,after,current,after);
    const edges = await this.rows.load('relation',found.map(row=>row.id));
    const endpointIds=[...new Set([...edges.values()].flatMap(ref=>[stringField(ref,'from_id'),stringField(ref,'to_id')]))];
    // Ordinary traversal skips an edge whose endpoint is absent, just like
    // the original LEFT JOIN and Python published exploration. Mandatory
    // origin and delivered-packet closure still require exact full rows.
    const foundEndpoints=endpointIds.length ? await this.read.textRows<{id:string}>(['id'],['id'],
      'SELECT id FROM knowledge_nodes WHERE id IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?',JSON.stringify(endpointIds),endpointIds.length+1) : [];
    if(new Set(foundEndpoints.map(row=>row.id)).size!==foundEndpoints.length) nativeUnavailable('duplicate exploration endpoint');
    const endpoints = await this.rows.load('node',foundEndpoints.map(row=>row.id));
    return found.map(({id})=>{
      const edge=edges.get(id)!, from_id=stringField(edge,'from_id'), to_id=stringField(edge,'to_id');
      if (from_id !== current && to_id !== current) nativeUnavailable('exploration adjacency index differs from payload');
      return {id,from_id,to_id,source_graph:stringField(edge,'source_graph'),predicate_id:stringField(edge,'predicate_id'),
        relation_type_id:stringField(edge,'relation_type_id'),from_source:endpoints.has(from_id)?stringField(endpoints.get(from_id)!,'source_graph'):null,
        to_source:endpoints.has(to_id)?stringField(endpoints.get(to_id)!,'source_graph'):null};
    });
  }
}

/** Covering physical seeks used by the published Rust continuation. Limits
 * and the declared identity prefix are concrete terms supplied by the core. */
export const PUBLISHED_EXPLORATION_ADJACENCY_SQL = `SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_seek WHERE from_id=? AND id>? ORDER BY id LIMIT ?)
  UNION SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_seek WHERE to_id=? AND id>? ORDER BY id LIMIT ?)
  ORDER BY id LIMIT ?`;
export const PUBLISHED_EXPLORATION_IDENTITY_SQL = `SELECT id FROM knowledge_nodes INDEXED BY knowledge_nodes_identity_seek
  WHERE entity_id=(SELECT entity_id FROM knowledge_nodes WHERE id=? AND substr(entity_id,1,length(?))=? AND entity_id NOT IN (SELECT value FROM json_each(?)))
    AND id>? AND source_graph IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?`;

/** Custody and static SQL for concrete Rust exploration reads. The per-page
 * Rust algorithm owns its sole parsed-row cache; this producer retains only
 * transient verified references and returns original lexical carrier bytes. */
export class PublishedExplorationD1Transport {
  private readonly read:NativeD1Read;
  private readonly rows:NativeD1Rows;
  private readonly encoder=new TextEncoder();
  constructor(read:NativeD1Read) {
    this.read=read;
    this.rows=new NativeD1Rows(read,read.limits,false,false);
  }
  async payloads(need:Extract<ExplorationNeed,{operation:'rows'}>):Promise<{rows:Uint8Array;sizes:number[];ambiguous:string[]}> {
    if(!need.ids.length)return {rows:this.encoder.encode('[]'),sizes:[],ambiguous:[]};
    // Cardinality is custody data: the Rust caller interprets exact origin,
    // ordinary nullable endpoints and mandatory closure differently. Never
    // choose an arbitrary carrier from a duplicate exact ID.
    const available=await this.read.textRows<{id:string;matches:string}>(['id','matches'],['id'],
      `SELECT value AS id,CAST((SELECT count(*) FROM (SELECT id FROM knowledge_${need.kind}s WHERE id=requested.value LIMIT 2)) AS TEXT) AS matches
        FROM json_each(?) requested ORDER BY id`,JSON.stringify(need.ids));
    if(available.length>need.ids.length)nativeUnavailable('exploration exact availability exceeds requested closure');
    const exact=new Set<string>(),ambiguous:string[]=[];
    for(const row of available) {
      if(!need.ids.includes(row.id)||!['0','1','2'].includes(row.matches))nativeUnavailable('exploration exact availability invalid');
      if(row.matches==='1')exact.add(row.id);
      else if(row.matches==='2')ambiguous.push(row.id);
    }
    const ids=need.ids.filter(id=>exact.has(id)),raw=new Map<string,string>();
    await this.rows.load(need.kind,ids,(id,body)=>raw.set(id,body));
    const bodies=ids.map(id=>{const body=raw.get(id);if(body===undefined)nativeUnavailable('exploration payload closure unavailable');return body;});
    return {rows:this.encoder.encode('['+bodies.join(',')+']'),sizes:bodies.map(body=>this.encoder.encode(body).byteLength),ambiguous};
  }
  async focus(need:Extract<ExplorationNeed,{operation:'focus'}>):Promise<{matched:number;rows:Uint8Array;sizes:number[]}> {
    const priority=need.field==='entity_id'?need.source_priority:[];
    const order=priority.length?'CASE source_graph '+priority.map(()=>'WHEN ? THEN ?').join(' ')+' ELSE 99 END,id':'id';
    const found=await this.read.textRows<{id:string}>(['id'],['id'],
      `SELECT id FROM knowledge_nodes WHERE ${need.field}=? AND source_graph IN (SELECT value FROM json_each(?)) ORDER BY ${order} LIMIT ?`,
      need.id,JSON.stringify(need.sources),...priority.flat(),need.limit);
    // This profile declares native-id lookahead2. Report ambiguity without
    // fetching either payload; the shared core owns the request refusal.
    if(found.length!==1)return {matched:found.length,rows:this.encoder.encode('[]'),sizes:[]};
    const raw=new Map<string,string>();
    await this.rows.load('node',[found[0]!.id],(id,body)=>raw.set(id,body));
    const body=raw.get(found[0]!.id);if(body===undefined)nativeUnavailable('exploration focus payload missing');
    return {matched:1,rows:this.encoder.encode('['+body+']'),sizes:[this.encoder.encode(body).byteLength]};
  }
  identities(need:Extract<ExplorationNeed,{operation:'identity'}>):Promise<{id:string}[]> {
    if(need.entity_id===null) {
      if(need.declared_prefix===null)nativeUnavailable('exploration identity physical term missing');
      return this.read.textRows(['id'],['id'],PUBLISHED_EXPLORATION_IDENTITY_SQL,
        need.node_id,need.declared_prefix,need.declared_prefix,JSON.stringify(need.expanded_entities),need.after,JSON.stringify(need.sources),need.limit);
    }
    return this.read.textRows(['id'],['id'],
      `SELECT id FROM knowledge_nodes INDEXED BY knowledge_nodes_identity_seek WHERE entity_id=? AND id>? AND source_graph IN (SELECT value FROM json_each(?)) ORDER BY id LIMIT ?`,
      need.entity_id,need.after,JSON.stringify(need.sources),need.limit);
  }
  adjacency(need:Extract<ExplorationNeed,{operation:'adjacency'}>):Promise<{id:string}[]> {
    return this.read.textRows(['id'],['id'],
      PUBLISHED_EXPLORATION_ADJACENCY_SQL,need.node_id,need.after,need.limit,need.node_id,need.after,need.limit,need.limit);
  }
}

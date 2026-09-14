/** Request-local native payloads; disposable traversal state contains IDs only. */
import {HttpError, type Item} from './common.ts';
import {NativeD1Read, NativeD1Rows, nativeD1Limits, nativeUnavailable, type NativeKind} from './native-d1-read.ts';
import {nativeField, stringField, type NativeRef} from './native-lens.ts';

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

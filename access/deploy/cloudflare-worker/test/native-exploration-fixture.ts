/** Publication adapter for old, deliberately minimal synthetic topology tests.
 * Raw-number/Python emission evidence lives separately in native-exploration.test.mjs.
 */
import {createHash} from 'node:crypto';
import {exploreD1 as exploreRawD1} from '../src/exploration.ts';
import type {Item} from '../src/common.ts';
const initialized=new WeakSet<object>();
const sha=(raw:string)=>createHash('sha256').update(raw).digest('hex');
export async function publishExplorationFixture(db:D1Database):Promise<void> {
  if (!initialized.has(db)) {
    for (const [kind,fields] of [['node',['kind_id','type_id']],['relation',['native_id','relation_type_id']]] as const) {
      const columns=await db.prepare(`PRAGMA table_info(knowledge_${kind}s)`).all<{name:string}>();
      for (const field of fields) if (!columns.results.some(row=>row.name===field)) await db.prepare(`ALTER TABLE knowledge_${kind}s ADD COLUMN ${field} TEXT`).run();
    }
    await db.batch(['CREATE INDEX IF NOT EXISTS knowledge_nodes_native_idx ON knowledge_nodes(native_id)',
      'CREATE INDEX IF NOT EXISTS knowledge_nodes_entity_idx ON knowledge_nodes(entity_id)',
      'CREATE INDEX IF NOT EXISTS knowledge_relations_native_idx ON knowledge_relations(native_id)'].map(sql=>db.prepare(sql)));
    initialized.add(db);
  }
  for (const kind of ['node','relation'] as const) {
    const rows=await db.prepare(`SELECT rowid,json FROM knowledge_${kind}s ORDER BY id`).all<{rowid:number;json:string}>();
    for (const row of rows.results) {
      const value=JSON.parse(row.json);
      // Only these old test fixtures lack normalized identity columns. They
      // are synthetic defaults, never repair or synthesis in a serving path.
      if(kind==='node') {value.entity_id??=value.id;value.native_id??=value.id;value.kind_id??='test';value.type_id??='test';}
      else {value.native_id??=value.id;value.relation_type_id??='';}
      const raw=JSON.stringify(value),fields=kind==='node'?['entity_id','native_id','kind_id','type_id']:['native_id','relation_type_id'];
      await db.prepare(`UPDATE knowledge_${kind}s SET ${fields.map(key=>key+'=?').join(',')},json=? WHERE rowid=?`).bind(...fields.map(key=>value[key]),raw,row.rowid).run();
      await db.prepare('INSERT OR REPLACE INTO edge_meta VALUES (?,0,?)').bind(`knowledge_${kind}_digest:${value.id}`,JSON.stringify({sha256:sha(raw)})).run();
    }
  }
  const top=JSON.parse((await db.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_exploration_top'").first<string>('json_chunk'))!);
  const revision=JSON.parse((await db.prepare("SELECT json_chunk FROM edge_meta WHERE key='data_revision'").first<string>('json_chunk'))!);
  const authority={source_owner:'Tree-of-Sophia',is_source:false,is_canon:false,writes_to_tree:false,...top.authority_boundary};
  await db.batch([
    db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_exploration_top'").bind(JSON.stringify({...top,authority_boundary:authority})),
    db.prepare("INSERT OR REPLACE INTO edge_meta VALUES ('knowledge_reader_top',0,?)").bind(JSON.stringify({schema:'tos_published_knowledge_reader_v1',
      read_model_schema:'tos_cloudflare_edge_read_model_v8',source_revision:top.source_revision,data_revision:revision.sha256,
      graph_schema:'tos_knowledge_graph_v1',normalization_binding:{schema:'tos_knowledge_graph_normalization_binding_v1',processor_digest:'0'.repeat(64),entity_registry_digest:'0'.repeat(64),relation_registry_digest:'0'.repeat(64),configuration_digest:'0'.repeat(64)},
      catalog_sha256:'0'.repeat(64),row_integrity:'sha256-emitted-json-v1',authority_boundary:authority})),
  ]);
}
export async function exploreD1(db:D1Database,request:unknown):Promise<Item> {return JSON.parse(await exploreRawD1(db,request));}

/** Test-only v9 publication for legacy hand-built tiny D1 fixtures.
 * Numeric/Unicode production-emission parity is independently covered by
 * native-lens.test.mjs, whose raw rows and metadata come from Python owners.
 */
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {executeKnowledgeLensD1} from '../src/knowledge-store.ts';
import {nativePacketJson, parseNativeRequest} from '../src/native-lens.ts';
import {nativeLower, codePointCompare, nativeUnicodeVersion} from '../../../shared/native-semantics.ts';
import type {executeKnowledgeLens} from '../src/knowledge.ts';
const initialized = new WeakSet<object>();
const sha = (raw: string) => createHash('sha256').update(raw).digest('hex');

export async function publishNativeLensFixture(db: D1Database): Promise<void> {
  if (!initialized.has(db)) {
    await db.batch([
      'CREATE TABLE IF NOT EXISTS knowledge_lens_order(kind TEXT,id TEXT,sort_key TEXT,from_id TEXT,to_id TEXT,PRIMARY KEY(kind,id))',
      'CREATE INDEX IF NOT EXISTS knowledge_lens_order_sort ON knowledge_lens_order(kind,sort_key,id)',
      'CREATE INDEX IF NOT EXISTS knowledge_lens_order_from ON knowledge_lens_order(kind,from_id,sort_key,id)',
      'CREATE INDEX IF NOT EXISTS knowledge_lens_order_to ON knowledge_lens_order(kind,to_id,sort_key,id)',
      'CREATE INDEX IF NOT EXISTS knowledge_lens_order_pair ON knowledge_lens_order(kind,from_id,to_id,id)',
      'CREATE INDEX IF NOT EXISTS knowledge_nodes_native_idx ON knowledge_nodes(native_id)',
      'CREATE INDEX IF NOT EXISTS knowledge_nodes_entity_idx ON knowledge_nodes(entity_id)',
      'CREATE INDEX IF NOT EXISTS knowledge_relations_native_idx ON knowledge_relations(native_id)',
    ].map(sql => db.prepare(sql)));
    initialized.add(db);
  }
  const [nodes,relations,topRows,revisionRows] = await Promise.all([
    db.prepare('SELECT json FROM knowledge_nodes ORDER BY id').all<{json:string}>(), db.prepare('SELECT json FROM knowledge_relations ORDER BY id').all<{json:string}>(),
    db.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_top' ORDER BY part").all<{json_chunk:string}>(),
    db.prepare("SELECT json_chunk FROM edge_meta WHERE key='data_revision' ORDER BY part").all<{json_chunk:string}>(),
  ]);
  const top = JSON.parse(topRows.results.map(row => row.json_chunk).join(''));
  const revision = JSON.parse(revisionRows.results.map(row => row.json_chunk).join(''));
  const hist = (rows: {json:string}[], fields: string[]) => {
    const counts = new Map<string,{cell:string[];count:number}>();
    for (const row of rows) {const item = JSON.parse(row.json), cell=fields.map(field => item[field]), key=JSON.stringify(cell), prior=counts.get(key); if(prior) prior.count++; else counts.set(key,{cell,count:1});}
    return [...counts.values()].sort((a,b)=>{for(let i=0;i<3;i++){const d=codePointCompare(a.cell[i]!,b.cell[i]!);if(d)return d;}return 0;}).map(({cell,count})=>[...cell,count]);
  };
  const lens = {schema:'tos_published_lens_metadata_v1',execution_version:'tos-lens-execution-v7',source_revision:top.source_revision,
    sort_key:'python-str-or-empty-lower-v1',unicode_version:nativeUnicodeVersion,query_properties:top.query_properties??[],
    node_counts:hist(nodes.results,['source_graph','kind_id','type_id']),relation_counts:hist(relations.results,['source_graph','predicate_id','relation_type_id'])};
  const metadata = new Map<string,string>([['knowledge_lens_top',JSON.stringify(lens)],['knowledge_reader_top',JSON.stringify({
    schema:'tos_published_knowledge_reader_v2',read_model_schema:'tos_cloudflare_edge_read_model_v9',source_revision:top.source_revision,data_revision:revision.sha256,
    graph_schema:'tos_knowledge_graph_v1',normalization_binding:{schema:'tos_knowledge_graph_normalization_binding_v1',processor_digest:'0'.repeat(64),entity_registry_digest:'0'.repeat(64),relation_registry_digest:'0'.repeat(64),configuration_digest:'0'.repeat(64)},
    catalog_sha256:'0'.repeat(64),row_integrity:'sha256-emitted-json-v1',authority_boundary:top.authority_boundary,lens_sha256:sha(JSON.stringify(lens)),
  })]]);
  const statements = [db.prepare('DELETE FROM knowledge_lens_order')];
  for (const [kind,rows] of [['node',nodes.results],['relation',relations.results]] as const) for (const row of rows) {
    const n=JSON.parse(row.json); metadata.set('knowledge_'+kind+'_digest:'+n.id,JSON.stringify({sha256:sha(row.json)}));
    statements.push(db.prepare('INSERT INTO knowledge_lens_order VALUES (?,?,?,?,?)').bind(kind,n.id,nativeLower(n.id),kind==='relation'?n.from_id:'',kind==='relation'?n.to_id:''));
  }
  for (const [key,raw] of metadata) statements.push(db.prepare('DELETE FROM edge_meta WHERE key=?').bind(key),db.prepare('INSERT INTO edge_meta VALUES (?,0,?)').bind(key,raw));
  for (let at=0;at<statements.length;at+=64) await db.batch(statements.slice(at,at+64));
}
export async function executePublishedFixtureLens(db: D1Database, spec: unknown): Promise<Awaited<ReturnType<typeof executeKnowledgeLens>>> {
  await publishNativeLensFixture(db);
  return JSON.parse(nativePacketJson((await executeKnowledgeLensD1(db,parseNativeRequest(JSON.stringify(spec)))).packet,{maxBytes:16*1024*1024}));
}

/** Publish the current tiny fixture with the actual Python search emitters. */
export async function publishNativeSearchFixture(db:D1Database):Promise<void> {
  await publishNativeLensFixture(db);
  const raw:Record<string,string[]>={};
  for(const kind of ['nodes','relations'])raw[kind]=(await db.prepare(`SELECT json FROM knowledge_${kind} ORDER BY id`).all<{json:string}>()).results.map(row=>row.json);
  const emitted=JSON.parse(execFileSync('python3',['-B','-c',String.raw`
import sys,json,hashlib
sys.path.insert(0,'access/src')
from tos_access.search_read_model import SQLiteKnowledgeSearchReadModel as S
out=[]
for kind,raws in json.load(sys.stdin).items():
 for position,raw in enumerate(raws):
  n=json.loads(raw);text=S._searchable(n);rank=S._rank_fields(n,relation=kind=='relations')
  out.append({'kind':kind,'id':n['id'],'text':text,'doc':[kind,position,n['id'],n['source_graph'],n.get('kind_id',''),n.get('predicate_id',''),*rank,len(text),hashlib.sha256(text.encode('utf-8')).hexdigest()],
   'grams':list(dict.fromkeys(text[i:i+3] for i in range(len(text)-2)))})
print(json.dumps(out))`],{cwd:fileURLToPath(new URL('../../../../',import.meta.url)),input:JSON.stringify(raw),encoding:'utf8'})) as {kind:string;id:string;text:string;doc:(string|number)[];grams:string[]}[];
  await db.batch([
    'CREATE TABLE IF NOT EXISTS knowledge_search_documents(kind TEXT,position INTEGER,id TEXT,source_graph TEXT,kind_id TEXT,predicate_id TEXT,id_lower TEXT,native_id_lower TEXT,identity_values TEXT,visible_values TEXT,document_chars INTEGER,document_digest TEXT,PRIMARY KEY(kind,position))',
    'CREATE TABLE IF NOT EXISTS knowledge_search_grams(kind TEXT,n INTEGER,gram TEXT,position INTEGER,PRIMARY KEY(kind,n,gram,position))',
    'CREATE TABLE IF NOT EXISTS knowledge_search_gram_stats(kind TEXT,n INTEGER,gram TEXT,postings INTEGER,PRIMARY KEY(kind,n,gram))',
    'DELETE FROM knowledge_search_documents','DELETE FROM knowledge_search_grams','DELETE FROM knowledge_search_gram_stats',
  ].map(sql=>db.prepare(sql)));
  const statements:D1PreparedStatement[]=[],stats=new Map<string,number>(),grams:(string|number)[][]=[];
  for(const row of emitted){statements.push(db.prepare(`UPDATE knowledge_${row.kind} SET search_text=? WHERE id=?`).bind(row.text,row.id));
    for(const gram of row.grams){grams.push([row.kind,3,gram,row.doc[1]!]);const key=JSON.stringify([row.kind,gram]);stats.set(key,(stats.get(key)??0)+1);}
  }
  const append=(table:string,values:(string|number)[][])=>{if(!values.length)return;const size=Math.floor(96/values[0]!.length);
    for(let at=0;at<values.length;at+=size){const page=values.slice(at,at+size);statements.push(db.prepare(`INSERT INTO ${table} VALUES `+page.map(args=>'('+args.map(()=>'?').join(',')+')').join(',')).bind(...page.flat()));}
  };
  append('knowledge_search_documents',emitted.map(row=>row.doc));append('knowledge_search_grams',grams);
  append('knowledge_search_gram_stats',[...stats].map(([key,total])=>{const[kind,gram]=JSON.parse(key);return [kind,3,gram,total];}));
  for(let at=0;at<statements.length;at+=64)await db.batch(statements.slice(at,at+64));
}

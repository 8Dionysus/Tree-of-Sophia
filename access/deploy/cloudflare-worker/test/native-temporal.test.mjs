import assert from 'node:assert/strict';
import test from 'node:test';
import {execFileSync} from 'node:child_process';
import {mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {DatabaseSync} from 'node:sqlite';
import {createHash} from 'node:crypto';
import {build} from 'esbuild';
import {Miniflare, convertV4MiniflareOptions} from 'miniflare';
import {knowledgeTemporalCompareD1} from '../src/knowledge-store.ts';

const repo = fileURLToPath(new URL('../../../../',import.meta.url));
const sha = raw => createHash('sha256').update(raw).digest('hex');
const python = (code,input) => JSON.parse(execFileSync('python3',['-B','-c',
  "import sys,json;sys.path[:0]=['access/src','access/deploy/cloudflare-worker/scripts'];"+code],
  {cwd:repo,input:input===undefined?undefined:JSON.stringify(input),encoding:'utf8',timeout:30000,maxBuffer:32*1024*1024}));
const fixtures = python(String.raw`
sys.path.insert(0,'access/tests')
from test_temporal_comparison import TemporalComparisonTests
from tos_access.published_read_metadata import published_reader_metadata,published_lens_metadata,emitted_row_digest,published_row_digest_key
from build_runtime import compact_json
TemporalComparisonTests.setUpClass()
cases=[]
for case in TemporalComparisonTests().transport_cases():
 nodes=[json.loads(raw) for raw in case['raw_nodes']]
 graph={'schema':'tos_knowledge_graph_v1','source_revision':case['graph']['source_revision'],'nodes':nodes,'relations':[],
  'normalization_binding':{'schema':'tos_knowledge_graph_normalization_binding_v1',**{k:'0'*64 for k in ('processor_digest','entity_registry_digest','relation_registry_digest','configuration_digest')}},
  'authority_boundary':{'source_owner':'Tree-of-Sophia','is_source':False,'is_canon':False,'writes_to_tree':False}}
 metadata=published_reader_metadata(graph,{'schema':'tos_knowledge_catalog_v1','source_revision':graph['source_revision']},'tos_cloudflare_edge_read_model_v9','d'*64,lens_metadata=published_lens_metadata(graph))
 metadata['data_revision']={'sha256':'d'*64}
 for node,raw in zip(nodes,case['raw_nodes']):metadata[published_row_digest_key('node',node['id'])]=emitted_row_digest(raw)
 cases.append({'name':case['name'],'request':case['request'],'nodes':case['raw_nodes'],'relations':[],
  'metadata':{key:compact_json(value) for key,value in metadata.items()}})
print(json.dumps(cases))
`);
const schema = `CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part));
 CREATE TABLE knowledge_nodes(id TEXT,entity_id TEXT,native_id TEXT,source_graph TEXT,kind_id TEXT,type_id TEXT,title_text TEXT,search_text TEXT,json TEXT);
 CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,native_id TEXT,source_graph TEXT,from_id TEXT,to_id TEXT,predicate_id TEXT,relation_type_id TEXT,label_text TEXT,search_text TEXT,json TEXT);
 CREATE INDEX knowledge_nodes_native_idx ON knowledge_nodes(native_id);
 CREATE INDEX knowledge_nodes_entity_idx ON knowledge_nodes(entity_id);
 CREATE INDEX knowledge_relations_native_idx ON knowledge_relations(native_id);
 CREATE TABLE knowledge_lens_order(kind TEXT,id TEXT,sort_key TEXT,from_id TEXT,to_id TEXT,PRIMARY KEY(kind,id));
 CREATE INDEX knowledge_lens_order_sort ON knowledge_lens_order(kind,sort_key,id);
 CREATE INDEX knowledge_lens_order_from ON knowledge_lens_order(kind,from_id,sort_key,id);
 CREATE INDEX knowledge_lens_order_to ON knowledge_lens_order(kind,to_id,sort_key,id);
 CREATE INDEX knowledge_lens_order_pair ON knowledge_lens_order(kind,from_id,to_id,id);`;
const migration=readFileSync(new URL('../migrations/0001-exploration.sql',import.meta.url),'utf8');
function rowBindings(kind,raw) {
  const n=JSON.parse(raw);
  return kind==='node'?[n.id,n.entity_id,n.native_id,n.source_graph,n.kind_id,n.type_id,'','',raw]
    :[n.id,n.native_id,n.source_graph,n.from_id,n.to_id,n.predicate_id,n.relation_type_id,'','',raw];
}
function database(fixture) {
  const directory=mkdtempSync(join(tmpdir(),'tos-native-temporal-')), path=join(directory,'published.sqlite');
  const sqlite=new DatabaseSync(path);sqlite.exec(schema);sqlite.exec(migration);
  for(const [key,raw] of Object.entries(fixture.metadata))sqlite.prepare('INSERT INTO edge_meta VALUES (?,0,?)').run(key,raw);
  for(const kind of ['node','relation'])for(const raw of fixture[kind+'s'])sqlite.prepare(`INSERT INTO knowledge_${kind}s VALUES (${rowBindings(kind,raw).map(()=>'?').join(',')})`).run(...rowBindings(kind,raw));
  const statements=[], hook={after:null};
  const db={prepare(sql){let args=[];return {bind(...values){args=values;return this;},
    async all(){const results=sqlite.prepare(sql).all(...args);statements.push({sql,args,rows:results.length,stringBytes:results.flatMap(row=>Object.values(row).filter(v=>typeof v==='string').map(v=>Buffer.byteLength(v)))});hook.after?.(sql,args);return {results,meta:{rows_read:0}};},
    async first(){const result=sqlite.prepare(sql).get(...args)??null;statements.push({sql,args,rows:result?1:0,stringBytes:Object.values(result??{}).filter(v=>typeof v==='string').map(v=>Buffer.byteLength(v))});hook.after?.(sql,args);return result;}};}};
  const binding=()=>{const raw=sqlite.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_reader_top'").get().json_chunk,top=JSON.parse(raw);
    return {schema:'tos_published_knowledge_snapshot_v1',metadata_sha256:sha(raw),publication_epoch:sqlite.prepare('SELECT epoch FROM knowledge_exploration_clock').get().epoch,
      ...Object.fromEntries(['read_model_schema','source_revision','data_revision','graph_schema','normalization_binding'].map(k=>[k,top[k]]))};};
  return {sqlite,db,statements,hook,path,binding,close(){sqlite.close();rmSync(directory,{recursive:true,force:true});}};
}
function oracle(database,request=database.request,binding=database.binding()) {
  return python(String.raw`
from tos_access.published_read_model import PublishedKnowledgeReadModel,PublishedReadBudgetExceeded,PublishedSnapshotConflict,PublishedReadModelError
from tos_access.temporal_comparison import TemporalReadModelInvalid
from tos_access.lens_pagination import KnowledgeRevisionConflict
p=json.load(sys.stdin)
try:
 packet=PublishedKnowledgeReadModel(p['path'],p['binding']).temporal_compare(p['request'])
 print(json.dumps({'status':200,'raw':json.dumps(packet,ensure_ascii=False,separators=(',',':'),allow_nan=False)}))
except Exception as e:
 status=413 if isinstance(e,PublishedReadBudgetExceeded) else 409 if isinstance(e,(PublishedSnapshotConflict,KnowledgeRevisionConflict)) else 503 if isinstance(e,(PublishedReadModelError,TemporalReadModelInvalid)) else 404 if isinstance(e,KeyError) else 400 if isinstance(e,ValueError) else 500
 print(json.dumps({'status':status,'error':str(e)}))
`,{path:database.path,binding,request});
}
function assertPackets(actual,expected) {
  const diff=python(String.raw`
def diff(a,b,path='$'):
 if type(a)!=type(b):return [path+': kind '+type(a).__name__+' != '+type(b).__name__]
 if isinstance(a,dict):
  if list(a)!=list(b):return [path+': ordered keys differ']
  return [d for k in a for d in diff(a[k],b[k],path+'.'+k)]
 if isinstance(a,list):
  if len(a)!=len(b):return [path+': lengths differ']
  return [d for i,(x,y) in enumerate(zip(a,b)) for d in diff(x,y,path+'['+str(i)+']')]
 if isinstance(a,float):return [] if repr(a)==repr(b) else [path+': float repr differs']
 return [] if a==b else [path+': value differs']
p=json.load(sys.stdin);print(json.dumps(diff(json.loads(p['actual']),json.loads(p['expected']))))
`,{actual,expected});assert.deepEqual(diff,[]);
}
let workerPromise;
async function worker() {
  workerPromise??=build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022'})
    .then(async bundle=>(await import('data:text/javascript;base64,'+Buffer.from(bundle.outputFiles[0].text).toString('base64'))).default);
  return workerPromise;
}
async function response(database,request=database.request,options={}) {
 return (await worker()).fetch(new Request('https://tos.test/api/knowledge/temporal/compare',{
  method:'POST',headers:{'Content-Type':'application/json',...options.headers},
  body:options.body??JSON.stringify(request),
 }),{DB:database.db,ASSETS:{fetch(){throw new Error('temporal must not read static assets');}}},{});
}
function selected(name='before') {
 const fixture=fixtures.find(item=>item.name===name)??fixtures[0];
 const data=database(fixture);data.request=fixture.request;return data;
}
function replaceRow(database,kind,id,change) {
  const original=database.sqlite.prepare(`SELECT json FROM knowledge_${kind}s WHERE id=?`).get(id).json;
  const raw=change(original), columns=kind==='node'?['id','entity_id','native_id','source_graph','kind_id','type_id']:['id','native_id','source_graph','from_id','to_id','predicate_id','relation_type_id'];
  const item=JSON.parse(raw);
  database.sqlite.prepare(`UPDATE knowledge_${kind}s SET ${columns.map(c=>c+'=?').join(',')},json=? WHERE id=?`).run(...columns.map(c=>item[c]),raw,id);
  database.sqlite.prepare('INSERT OR REPLACE INTO edge_meta VALUES (?,0,?)').run(`knowledge_${kind}_digest:${id}`,JSON.stringify({sha256:sha(raw)}));
}

test('temporal actual raw Worker packets equal current published Python across the retained semantic corpus',async t=>{
 for(const fixture of fixtures)await t.test(fixture.name,async()=>{
  const data=database(fixture);data.request=fixture.request;
  try {
   const expected=oracle(data),actual=await response(data);
   assert.equal(actual.status,expected.status,fixture.name+': '+await (actual.status===200?Promise.resolve(''):actual.clone().text()));
   if(actual.status===200)assertPackets(await actual.text(),expected.raw);
   else await actual.arrayBuffer();
   assert.ok(data.statements.filter(s=>s.sql.includes('FROM knowledge_nodes WHERE id=?')).length<=6);
   assert.ok(data.statements.every(s=>!s.sql.includes('FROM knowledge_lens_order')&&!s.sql.includes('FROM knowledge_relations')));
   assert.ok(data.statements.filter(s=>s.sql.includes('AS json_bytes FROM knowledge_nodes')).length<=6);
  }finally{data.close();}
 });
});

test('temporal native equality, canonical numbers, Python source_line types and Unicode remain distinct',async t=>{
 const cases=[
  ['historical-unsafe-equal',false,String.raw`obj['extension']={'10':9007199254740993,'2':1.0,'01':-0.0};source['object']=obj;attrs['value']=obj;time['raw']=obj`],
  ['historical-unsafe-mismatch',false,String.raw`obj['extension']=9007199254740993;source['object']=obj;attrs['value']={**obj,'extension':9007199254740992};time['raw']=attrs['value']`],
  ['historical-bool-integer-mismatch',false,String.raw`obj['extension']=True;source['object']=obj;attrs['value']={**obj,'extension':1};time['raw']=attrs['value']`],
  ['historical-int-float-equal',false,String.raw`obj['extension']=1;source['object']=obj;attrs['value']={**obj,'extension':1.0};time['raw']=attrs['value']`],
  ['document-line-float',true,String.raw`claim['attributes']['source_line']=1.0;attrs['source_line']=1.0`],
  ['document-line-unsafe-integer',true,String.raw`claim['attributes']['source_line']=9007199254740993;attrs['source_line']=9007199254740993`],
  ['document-line-bool-right',true,String.raw`claim['attributes']['source_line']=1;attrs['source_line']=True`],
  ['unicode-source-refs',false,String.raw`claim['source_refs']=['',7,'😀','\ue000',''];value['source_refs']=['\ue000','😀',False]`],
  ['unknown-native-context',false,String.raw`claim['unknown']={'10':9007199254740993,'2':1.0,'01':-0.0,'__proto__':{'constructor':1e-7}};time['unknown']={'10':1.0,'2':9007199254740993}`],
 ];
 for(const [name,documentary,mutation] of cases)await t.test(name,async()=>{
  const data=selected(documentary?'document-native-numbers':fixtures[0].name);
  try {
   const ids=python(String.raw`
import sqlite3
p=json.load(sys.stdin)
with sqlite3.connect(p['path']) as db:
 claim=json.loads(db.execute('SELECT json FROM knowledge_nodes WHERE id=?',(p['claim'],)).fetchone()[0])
 value_id=claim['semantics']['claim']['object_node_id'];value=json.loads(db.execute('SELECT json FROM knowledge_nodes WHERE id=?',(value_id,)).fetchone()[0])
 source=claim['attributes']['source_claim'];attrs=value['attributes'];time=value['semantics']['time'];obj=dict(source['object'])
 exec(p['mutation'])
 import hashlib
 for node in (claim,value):
  raw=json.dumps(node,ensure_ascii=False,separators=(',',':'),allow_nan=False)
  db.execute('UPDATE knowledge_nodes SET json=? WHERE id=?',(raw,node['id']))
  db.execute('UPDATE edge_meta SET json_chunk=? WHERE key=?',(json.dumps({'sha256':hashlib.sha256(raw.encode()).hexdigest()}),'knowledge_node_digest:'+node['id']))
 db.commit()
print(json.dumps([claim['id'],value_id]))
`,{path:data.path,claim:data.request.left.node_id,mutation});
   const expected=oracle(data),actual=await response(data);
   assert.equal(actual.status,expected.status,name);
   if(actual.status===200)assertPackets(await actual.text(),expected.raw);
   else await actual.arrayBuffer();
   assert.equal(ids.length,2);
  }finally{data.close();}
 });
});

test('temporal exact identities, damaged rows, metadata and typed byte refusals match published Python',async t=>{
 const controls=[
  ['checksum',data=>{data.sqlite.prepare('UPDATE knowledge_nodes SET json=json||? WHERE id=?').run(' ',data.request.left.node_id);}],
  ['duplicate-source-member',data=>replaceRow(data,'node',data.request.left.node_id,raw=>raw.slice(0,-1)+',"id":'+JSON.stringify(data.request.left.node_id)+'}')],
  ['malformed-source',data=>{const id=data.request.left.node_id;data.sqlite.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').run('{bad',id);data.sqlite.prepare('UPDATE edge_meta SET json_chunk=? WHERE key=?').run(JSON.stringify({sha256:sha('{bad')}),'knowledge_node_digest:'+id);}],
  ['nonfinite-source',data=>replaceRow(data,'node',data.request.left.node_id,raw=>raw.slice(0,-1)+',"invalid":1e309}')],
  ['bad-container',data=>replaceRow(data,'node',data.request.left.node_id,raw=>JSON.stringify({...JSON.parse(raw),semantics:null}))],
  ['escaped-lone-surrogate',data=>replaceRow(data,'node',data.request.left.node_id,raw=>raw.slice(0,-1)+',"unknown":"\\ud800"}')],
  ['row-byte-budget',data=>replaceRow(data,'node',data.request.left.node_id,raw=>{const item=JSON.parse(raw);item.padding='';const base=JSON.stringify(item);item.padding='x'.repeat(1048577-Buffer.byteLength(base));return JSON.stringify(item);})],
  ['duplicate-exact-row',data=>data.sqlite.prepare('INSERT INTO knowledge_nodes SELECT * FROM knowledge_nodes WHERE id=?').run(data.request.left.node_id)],
  ['missing-index',data=>data.sqlite.exec('DROP INDEX knowledge_nodes_native_idx')],
  ['noncompact-header',data=>data.sqlite.prepare("UPDATE edge_meta SET json_chunk=' '||json_chunk WHERE key='knowledge_reader_top'").run()],
  ['digest-byte-budget',data=>data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key=?").run(' '.repeat(1025),'knowledge_node_digest:'+data.request.left.node_id)],
 ];
 for(const [name,mutate] of controls)await t.test(name,async()=>{
  const data=selected();try{
   mutate(data);const expected=oracle(data),actual=await response(data);
   assert.equal(actual.status,expected.status,name+': '+expected.error);
   assert.notEqual(actual.status,200);await actual.arrayBuffer();
   if(name.endsWith('byte-budget'))assert.equal(actual.status,413);
   if(name==='row-byte-budget')assert.ok(data.statements.flatMap(s=>s.stringBytes).every(n=>n<=1048576));
  }finally{data.close();}
 });
 const data=selected();try{
  for(const request of [{...data.request,source_revision:'0'.repeat(64)},
   {...data.request,left:{...data.request.left,content_revision:'0'.repeat(64)}},
   {...data.request,left:{...data.request.left,node_id:'not-an-alias'}},
   {...data.request,left:{...data.request.left,node_id:'\u0085bad'}},
   {...data.request,left:{...data.request.left,node_id:'\ufeffbad'}},
   {...data.request,left:{...data.request.left,node_id:'\ud800'}},
   {...data.request,calendar:'gregorian'}]){
    const expected=oracle(data,request),actual=await response(data,request);
    assert.equal(actual.status,expected.status);await actual.arrayBuffer();
  }
  const reordered={...data.request,left:{content_revision:data.request.left.content_revision,node_id:data.request.left.node_id}};
  const expected=oracle(data,reordered),actual=await response(data,reordered);
  assert.equal(actual.status,200);assertPackets(await actual.text(),expected.raw);
  for(const [body,headers,status] of [['{}',{'Content-Type':'text/plain'},415],['{',{},400],
   [' '.repeat(65537),{},413],['\ufeff'+JSON.stringify(data.request),{},400],[new Uint8Array([255]),{},400]]){
    const actual=await response(data,data.request,{body,headers});assert.equal(actual.status,status);await actual.arrayBuffer();
  }
 }finally{data.close();}
});

test('temporal publication ABA is refused after exact operand reads',async()=>{
 const data=selected();try{
  let changed=false;data.hook.after=sql=>{
   if(changed||!sql.includes('AS json_bytes FROM knowledge_nodes'))return;
   changed=true;data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(JSON.stringify({sha256:'f'.repeat(64)}));
   data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(JSON.stringify({sha256:'d'.repeat(64)}));
  };
  const result=await response(data);assert.equal(result.status,409);await result.arrayBuffer();assert.equal(changed,true);
 }finally{data.close();}
});

test('temporal equivalent source float spellings bind canonically while duplicate keys refuse',async()=>{
 const data=selected('document-native-numbers');try{
  for(const row of data.sqlite.prepare('SELECT id FROM knowledge_nodes').all())replaceRow(data,'node',row.id,
   raw=>' \n'+raw.replaceAll('"float":1.0','"float":1e0').replaceAll('"negative_zero":-0.0','"negative_zero":-0e0')+'\n');
  const expected=oracle(data),actual=await response(data);
  assert.equal(expected.status,200);assert.equal(actual.status,200);assertPackets(await actual.text(),expected.raw);
  assert.equal(JSON.parse(expected.raw).comparison.status,'comparable');
 }finally{data.close();}
});

test('temporal expanded canonical floats refuse source binding rather than imposing a hidden small-row cutoff',async()=>{
 const data=selected('document-native-numbers');try{
  replaceRow(data,'node',data.request.left.node_id,raw=>{
   const node=JSON.parse(raw);node.attributes.source_claim.object.extension='native-float-array';
   return JSON.stringify(node).replace('"native-float-array"','['+Array(80000).fill('1e15').join(',')+']');
  });
  const expected=oracle(data),actual=await response(data);
  assert.equal(expected.status,200);assert.equal(actual.status,200);
  const expectedPacket=JSON.parse(expected.raw);
  assert.equal(expectedPacket.comparison.status,'undetermined');
  assert.ok(expectedPacket.comparison.reasons.some(reason=>reason.code==='document-catalogue-exact-source-binding-inconsistent'));
  assertPackets(await actual.text(),expected.raw);
 }finally{data.close();}
});

test('temporal v8 admission needs no lens rows and bounded full packets may exceed one MiB',async()=>{
 const data=selected();try{
  const raw=python(String.raw`
from tos_access.published_read_metadata import _compact
p=json.load(sys.stdin);top=json.loads(p['raw']);top['schema']='tos_published_knowledge_reader_v1';top['read_model_schema']='tos_cloudflare_edge_read_model_v8';top.pop('lens_sha256');print(json.dumps(_compact(top)))
`,{raw:data.sqlite.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_reader_top'").get().json_chunk});
  data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(raw);
  data.sqlite.exec('DROP TABLE knowledge_lens_order');
  for(const row of data.sqlite.prepare('SELECT id FROM knowledge_nodes').all())replaceRow(data,'node',row.id,
   raw=>raw.slice(0,-1)+',"unknown_padding":"'+'x'.repeat(400000)+'"}');
  const expected=oracle(data),actual=await response(data);
  assert.equal(expected.status,200);assert.equal(actual.status,200);
  const wire=await actual.text();assert.ok(Buffer.byteLength(wire)>1048576);assertPackets(wire,expected.raw);
  const seen=new Set();for(const item of data.statements)if(item.sql.includes('AS json_bytes FROM knowledge_nodes')){
   for(const id of JSON.parse(item.args[0])){assert.equal(seen.has(id),false);seen.add(id);}
  }
 }finally{data.close();}
});

test('real Miniflare temporal D1 raw HTTP matches published Python and rejects ABA',async()=>{
 const fixture=fixtures.find(item=>item.name==='document-native-numbers'),data=database(fixture);data.request=fixture.request;
 const bundle=await build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022'});
 const mf=new Miniflare(convertV4MiniflareOptions({modules:true,script:bundle.outputFiles[0].text,compatibilityDate:'2026-09-03',d1Databases:['DB']}));
 try{
  const db=await mf.getD1Database('DB');
  await db.batch(schema.split(';').map(s=>s.trim()).filter(Boolean).map(s=>db.prepare(s)));
  await db.batch(migration.replace(/^--.*$/gm,'').trim().split(/\n(?=CREATE |INSERT )/).map(s=>db.prepare(s)));
  const commands=[];
  for(const [key,raw] of Object.entries(fixture.metadata))commands.push(db.prepare('INSERT INTO edge_meta VALUES (?,0,?)').bind(key,raw));
  for(const raw of fixture.nodes)commands.push(db.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').bind(...rowBindings('node',raw)));
  await db.batch(commands);
  const expected=oracle(data),actual=await mf.dispatchFetch('https://tos.test/api/knowledge/temporal/compare',{
   method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(data.request)});
  assert.equal(actual.status,200);assertPackets(await actual.text(),expected.raw);
  let changed=false;
  const guarded={prepare(sql){let statement=db.prepare(sql);return{bind(...args){statement=statement.bind(...args);return this;},async first(){return statement.first();},async all(){
   const result=await statement.all();if(!changed&&sql.includes('AS json_bytes FROM knowledge_nodes')){
    changed=true;await db.batch([
     db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256:'e'.repeat(64)})),
     db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(fixture.metadata.data_revision)]);
   }return result;
  }};}};
  await assert.rejects(knowledgeTemporalCompareD1(guarded,data.request),error=>error.status===409);assert.equal(changed,true);
 }finally{data.close();await mf.dispose();}
});

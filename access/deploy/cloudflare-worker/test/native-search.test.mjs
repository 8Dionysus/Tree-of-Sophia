import assert from 'node:assert/strict';
import test from 'node:test';
import {execFileSync} from 'node:child_process';
import {readFileSync,mkdtempSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {DatabaseSync} from 'node:sqlite';
import {createHash} from 'node:crypto';
import {build} from 'esbuild';
import {Miniflare,convertV4MiniflareOptions} from 'miniflare';
import {knowledgeSearchD1,knowledgeSearchD1Indexed} from '../src/knowledge-store.ts';
import {nativePacketJson} from '../src/native-lens.ts';

const repo=fileURLToPath(new URL('../../../../',import.meta.url));
const python=(code,input)=>JSON.parse(execFileSync('python3',['-B','-c',
  "import sys,json;sys.path[:0]=['access/src','access/deploy/cloudflare-worker/scripts'];"+code],
  {cwd:repo,input:input===undefined?undefined:JSON.stringify(input),encoding:'utf8',timeout:30000,maxBuffer:32*1024*1024}));
const sha=raw=>createHash('sha256').update(raw).digest('hex');
const fixture=python(String.raw`
import hashlib
from tos_access.knowledge import _normalize_node,_normalize_relation
from tos_access.search_read_model import SQLiteKnowledgeSearchReadModel as S
from tos_access.published_read_metadata import published_reader_metadata,published_lens_metadata,emitted_row_digest,published_row_digest_key
from build_runtime import compact_json
unknown={'10':9007199254740993,'2':1.0,'01':-0.0,'x':[False,None,1e-7], '__proto__':{'constructor':1.0,'2':-0.0}}
nodes=[]
for name,title in [('z','ΑΒΓ'),('a','Other'),('scalar','Scalar αβγ'),('tie','Common alpha'),('TIE','Common alpha'),
 ('straße','Straße'),('strasse','STRASSE'),('nfc','école'),('nfd','e\u0301cole'),('astral','😀'*256),('A','Common alpha')]:
 n=_normalize_node({'node_id':name,'label':title,'source_ref':'test:search'},'philosophy')
 n['display']['title']=title if name=='scalar' else {'default':title,'grc-Grek':title}
 n['unknown']=unknown;n['attributes']['match']='αβγ metadata' if name=='a' else 'common alpha metadata'
 nodes.append(n)
relations=[]
for name,title in [('r','ΑΒΓ'),('R','Alpha relation')]:
 r=_normalize_relation({'edge_id':name,'from_id':nodes[0]['id'],'to_id':nodes[1]['id'],'predicate_id':'links','source_ref':'test:search'},'philosophy',{n['id']:n for n in nodes})
 r['from_id']=nodes[0]['id'];r['to_id']=nodes[1]['id'];r['display']['label']={'default':title};r['unknown']=unknown;relations.append(r)
graph={'schema':'tos_knowledge_graph_v1','source_revision':'a'*64,'nodes':nodes,'relations':relations,
 'normalization_binding':{'schema':'tos_knowledge_graph_normalization_binding_v1',**{k:'0'*64 for k in ('processor_digest','entity_registry_digest','relation_registry_digest','configuration_digest')}},
 'authority_boundary':{'source_owner':'Tree-of-Sophia','is_source':False,'is_canon':False,'writes_to_tree':False,'unknown':unknown}}
out={'graph':compact_json(graph),'rows':{},'documents':[],'grams':[],'stats':[],'metadata':{}}
counts={}
for kind in ('nodes','relations'):
 out['rows'][kind]=[]
 for position,n in enumerate(graph[kind]):
  text=S._searchable(n);raw=compact_json(n);out['rows'][kind].append({'raw':raw,'text':text})
  out['documents'].append([kind,position,n['id'],n['source_graph'],n.get('kind_id',''),n.get('predicate_id',''),*S._rank_fields(n,relation=kind=='relations'),len(text),hashlib.sha256(text.encode('utf-8')).hexdigest()])
  for gram in dict.fromkeys(text[i:i+3] for i in range(len(text)-2)):
   out['grams'].append([kind,3,gram,position]);counts[(kind,gram)]=counts.get((kind,gram),0)+1
out['stats']=[[kind,3,gram,count] for (kind,gram),count in counts.items()]
for version in (8,9):
 m=published_reader_metadata(graph,{'schema':'tos_knowledge_catalog_v1','source_revision':graph['source_revision']},'tos_cloudflare_edge_read_model_v'+str(version),'d'*64,**({'lens_metadata':published_lens_metadata(graph)} if version==9 else {}))
 m['data_revision']={'sha256':'d'*64}
 for kind in ('nodes','relations'):
  for row in out['rows'][kind]:
   n=json.loads(row['raw']);m[published_row_digest_key(kind[:-1],n['id'])]=emitted_row_digest(row['raw'])
 out['metadata'][str(version)]={k:compact_json(v) for k,v in m.items() if k!='knowledge_lens_top'}
print(json.dumps(out))
`);
const schema=`CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part));
CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,entity_id TEXT,native_id TEXT,source_graph TEXT,kind_id TEXT,type_id TEXT,title_text TEXT,search_text TEXT,json TEXT);
CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,native_id TEXT,source_graph TEXT,from_id TEXT,to_id TEXT,predicate_id TEXT,relation_type_id TEXT,label_text TEXT,search_text TEXT,json TEXT);
CREATE INDEX knowledge_nodes_native_idx ON knowledge_nodes(native_id);
CREATE INDEX knowledge_nodes_entity_idx ON knowledge_nodes(entity_id);
CREATE INDEX knowledge_relations_native_idx ON knowledge_relations(native_id);
CREATE TABLE knowledge_lens_order(kind TEXT,id TEXT,sort_key TEXT,from_id TEXT,to_id TEXT,PRIMARY KEY(kind,id));
CREATE INDEX knowledge_lens_order_sort ON knowledge_lens_order(kind,sort_key,id);
CREATE INDEX knowledge_lens_order_from ON knowledge_lens_order(kind,from_id,sort_key,id);
CREATE INDEX knowledge_lens_order_to ON knowledge_lens_order(kind,to_id,sort_key,id);
CREATE INDEX knowledge_lens_order_pair ON knowledge_lens_order(kind,from_id,to_id,id);
CREATE TABLE knowledge_search_documents(kind TEXT,position INTEGER,id TEXT,source_graph TEXT,kind_id TEXT,predicate_id TEXT,id_lower TEXT,native_id_lower TEXT,identity_values TEXT,visible_values TEXT,document_chars INTEGER,document_digest TEXT,PRIMARY KEY(kind,position));
CREATE TABLE knowledge_search_grams(kind TEXT,n INTEGER,gram TEXT,position INTEGER,PRIMARY KEY(kind,n,gram,position));
CREATE TABLE knowledge_search_gram_stats(kind TEXT,n INTEGER,gram TEXT,postings INTEGER,PRIMARY KEY(kind,n,gram));`;
const migration=readFileSync(new URL('../migrations/0001-exploration.sql',import.meta.url),'utf8');
function bindings(kind,row){const n=JSON.parse(row.raw);return kind==='nodes'?[n.id,n.entity_id,n.native_id,n.source_graph,n.kind_id,n.type_id,'',row.text,row.raw]:[n.id,n.native_id,n.source_graph,n.from_id,n.to_id,n.predicate_id,n.relation_type_id,'',row.text,row.raw];}
function database(version=9){
 const sqlite=new DatabaseSync(':memory:');sqlite.exec(schema);sqlite.exec(migration);
 for(const [key,raw]of Object.entries(fixture.metadata[version]))sqlite.prepare('INSERT INTO edge_meta VALUES (?,0,?)').run(key,raw);
 for(const kind of ['nodes','relations'])for(const row of fixture.rows[kind]){const args=bindings(kind,row);sqlite.prepare(`INSERT INTO knowledge_${kind} VALUES (${args.map(()=>'?').join(',')})`).run(...args);}
 for(const [table,key]of [['knowledge_search_documents','documents'],['knowledge_search_grams','grams'],['knowledge_search_gram_stats','stats']])for(const args of fixture[key])sqlite.prepare(`INSERT INTO ${table} VALUES (${args.map(()=>'?').join(',')})`).run(...args);
 const statements=[],hook={after:null};
 const db={prepare(sql){let args=[];return {bind(...values){args=values;return this;},async all(){const results=sqlite.prepare(sql).all(...args);statements.push({sql,args,results});hook.after?.(sql,args);return {results,meta:{rows_read:0}};},async first(){const result=sqlite.prepare(sql).get(...args)??null;statements.push({sql,args,results:result?[result]:[]});hook.after?.(sql,args);return result;}};}};
 return {db,sqlite,statements,hook,close(){sqlite.close();}};
}
let workerPromise;
async function worker(){workerPromise??=build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022'}).then(async result=>(await import('data:text/javascript;base64,'+Buffer.from(result.outputFiles[0].text).toString('base64'))).default);return workerPromise;}
async function response(database,mode='legacy',query='',extra={},method='GET'){
 const params=new URLSearchParams({mode,query,...extra});
 return (await worker()).fetch(new Request('https://tos.test/api/knowledge/search?'+params,{method}),{DB:database.db,ASSETS:{fetch(){throw new Error('search must not fetch static or full-graph fallback');}}},{});
}
const options=(query='',extra={})=>({query,sources:null,kindIds:[],predicateIds:[],offset:0,limit:100,...extra});
async function direct(database,mode,query='',extra={}){return nativePacketJson(await (mode==='indexed'?knowledgeSearchD1Indexed:knowledgeSearchD1)(database.db,options(query,extra)));}
function oracle(mode,query,extra={}){return python(String.raw`
import tempfile
from pathlib import Path
from types import SimpleNamespace
from tos_access.knowledge import search_knowledge_graph
from tos_access.search_read_model import SQLiteKnowledgeSearchReadModel
from tos_access.core import ToSAccessCore
p=json.load(sys.stdin);graph=json.loads(p['graph']);options=p['extra'];out=[]
if p['mode']=='legacy':out=[search_knowledge_graph(graph,p['query'],**options)]
else:
 with tempfile.TemporaryDirectory(prefix='tos-native-search-oracle-') as temporary:
  model=SQLiteKnowledgeSearchReadModel.build(graph,Path(temporary)/'search.sqlite',max_bytes=8*1024*1024)
  try:
   core=SimpleNamespace(knowledge_graph=lambda:graph,_search_read_model_for_snapshot=lambda graph:model,search_read_model_max_verify_chars=16000000)
   cursor=None
   while True:
    page=ToSAccessCore.knowledge_search_indexed(core,p['query'],cursor=cursor,**options);out.append(page);cursor=page['page']['next_cursor']
    if cursor is None:break
  finally:model.close()
print(json.dumps([json.dumps(packet,ensure_ascii=False,separators=(',',':'),allow_nan=False) for packet in out]))
`,{graph:fixture.graph,mode,query,extra});}
function assertPackets(actual,expected,indexed=false){const differences=python(String.raw`
def diff(a,b,path='$'):
 if type(a)!=type(b):return [path+': type differs']
 if isinstance(a,dict):
  if list(a)!=list(b):return [path+': ordered keys differ']
  return [d for k in a for d in diff(a[k],b[k],path+'.'+k)]
 if isinstance(a,list):
  if len(a)!=len(b):return [path+': lengths differ']
  return [d for i,(x,y) in enumerate(zip(a,b)) for d in diff(x,y,path+'['+str(i)+']')]
 if isinstance(a,float):return [] if repr(a)==repr(b) else [path+': float repr differs']
 return [] if a==b else [path+': value differs']
p=json.load(sys.stdin);a=json.loads(p['actual']);b=json.loads(p['expected'])
if p['indexed']:
 for packet in (a,b):
  packet.pop('work');packet['page']['cursor']=packet['page']['cursor'] is not None;packet['page']['next_cursor']=packet['page']['next_cursor'] is not None
print(json.dumps(diff(a,b)))`,{actual,expected,indexed});assert.deepEqual(differences,[]);}

for(const version of [8,9])test(`v${version} full search rows retain Python kinds, ordered keys, metadata and Unicode rank`,async()=>{
 const d=database(version);try{
  for(const query of ['', 'αβγ','\u0085ΑΒΓ\u0085','\ufeffαβγ','scalar','straße','STRASSE','école','e\u0301cole','😀'.repeat(256)]){
   const raw=await direct(d,'legacy',query);assertPackets(raw,oracle('legacy',query,{limit:100})[0]);
  }
  assert.equal(d.statements.some(row=>row.sql.includes('knowledge_lens_top')),false);
  assert.equal(d.statements.some(row=>row.sql.includes('SELECT json FROM knowledge_')),false);
 }finally{d.close();}
});
test('indexed native first and continuation packets match actual Python core/read-model without restarting exhausted kinds',async()=>{
 const d=database();try{
  for(const query of ['alpha','αβγ','\u0085ΑΒΓ\u0085','straße','STRASSE','😀'.repeat(256)]){
   const expected=oracle('indexed',query,{limit:2});let cursor=null;
   for(const page of expected){const raw=await direct(d,'indexed',query,{limit:2,cursor});assertPackets(raw,page,true);assert.equal(await direct(d,'indexed',query,{limit:2,cursor}),raw);cursor=JSON.parse(raw).page.next_cursor;}
   assert.equal(cursor,null);
  }
 }finally{d.close();}
});
test('actual HTTP GET/HEAD serialize full native source only once and replay identical queries',async()=>{
 const d=database();try{for(const mode of ['legacy','indexed']){
  const first=await response(d,mode,'αβγ',{limit:'2'});assert.equal(first.status,200);const raw=await first.text();assertPackets(raw,oracle(mode,'αβγ',{limit:2})[0],mode==='indexed');
  assert.equal(await (await response(d,mode,'αβγ',{limit:'2'})).text(),raw);
  const head=await response(d,mode,'αβγ',{limit:'2'},'HEAD');assert.equal(head.status,200);assert.equal(await head.text(),'');
 }}finally{d.close();}
});

test('Python mode-specific whitespace, code-point lengths and bounded filters fail before selected delivery',async()=>{
 const d=database();try{
  for(const [mode,query,status]of [['legacy',' '+ 'x'.repeat(256)+' ',200],['indexed',' '+'x'.repeat(256)+' ',400],
   ['legacy','İ'.repeat(129),200],['indexed','İ'.repeat(129),400],['legacy','a',200],['indexed','a',400],
   ['legacy','x'.repeat(257),400],['indexed','😀'.repeat(257),400]]){
   const start=d.statements.length;assert.equal((await response(d,mode,query)).status,status);
   if(status===400)assert.equal(d.statements.slice(start).some(row=>row.sql.includes('json_bytes')),false);
  }
  for(const mode of ['legacy','indexed']){
   assert.equal((await response(d,mode,'alpha',{sources:'unknown'})).status,400);
   assert.equal((await response(d,mode,'alpha',{sources:'\u0085philosophy\u0085'})).status,400);
   assert.equal((await response(d,mode,'alpha',{sources:'\ufeffphilosophy'})).status,400);
  }
  assert.equal((await response(d,'indexed','alpha',{kind_ids:'x'.repeat(257)})).status,400);
 }finally{d.close();}
});
test('search HTTP numeric parsing follows Python whole integers, Unicode decimal digits and default/clamp',async()=>{
 const d=database();try{
  const values=['1','1.0','1suffix','1_0','١٢','\u00852\u0085','\u001c2','\ufeff2','-100','999999999999999999999','9'.repeat(4301)];
  const expected=python("from tos_access.http_server import _integer;p=json.load(sys.stdin);print(json.dumps([_integer({'limit':[v]},'limit',40,1,100) for v in p]))",values);
  for(const mode of ['legacy','indexed'])for(const [at,value]of values.entries()){
   const result=await response(d,mode,'alpha',{limit:value});assert.equal(result.status,200);assert.equal((await result.json()).page.limit_per_kind,expected[at],mode+': '+value.slice(0,20));
  }
 }finally{d.close();}
});
test('search HTTP blank/repeated parameters follow Python parse_qs before first-value selection',async()=>{
 const d=database();try{
  const result=await (await worker()).fetch(new Request('https://tos.test/api/knowledge/search?mode=&mode=indexed&query=&query=alpha&limit=&limit=1&cursor='),{DB:d.db,ASSETS:{fetch(){throw new Error('no fallback');}}},{});
  assert.equal(result.status,200);assertPackets(await result.text(),oracle('indexed','alpha',{limit:1})[0],true);
 }finally{d.close();}
});
test('private native cursor migration, integer kinds, filters and epochs never silently coerce or restart',async()=>{
 const d=database();try{
  const first=JSON.parse(await direct(d,'indexed','alpha',{limit:1})),cursor=first.page.next_cursor;
  await assert.rejects(()=>knowledgeSearchD1Indexed(d.db,options('alpha',{cursor:''})),error=>error.status===400);
  const decode=value=>JSON.parse(Buffer.from(value,'base64url').toString('utf8')),encode=value=>Buffer.from(JSON.stringify(value)).toString('base64url');
  const outer=decode(cursor);
  const reordered={...outer,filters:{predicate_ids:outer.filters.predicate_ids,kind_ids:outer.filters.kind_ids,sources:outer.filters.sources}};
  assert.equal((await response(d,'indexed','alpha',{limit:'1',cursor:encode(reordered)})).status,200);
  const duplicate=Buffer.from('{"schema":"superseded",'+JSON.stringify(outer).slice(1)).toString('base64url');
  assert.equal((await response(d,'indexed','alpha',{limit:'1',cursor:duplicate})).status,200);
  for(const [schema,status]of [['tos_knowledge_search_indexed_cursor_v2',409],['tos_knowledge_search_indexed_cursor_v1',400],['unknown',400]]){
   assert.equal((await response(d,'indexed','alpha',{limit:'1',cursor:encode({...outer,schema})})).status,status);
  }
  const floatOuter=Buffer.from(JSON.stringify(outer).replace(/"snapshot_epoch":(\d+)/,'"snapshot_epoch":$1.0')).toString('base64url');
  assert.equal((await response(d,'indexed','alpha',{cursor:floatOuter})).status,400);
  const innerRaw=Buffer.from(outer.nodes,'base64url').toString('utf8').replace(/"rank":(\d+)/,'"rank":$1.0');
  assert.equal((await response(d,'indexed','alpha',{cursor:encode({...outer,nodes:Buffer.from(innerRaw).toString('base64url')})})).status,400);
  assert.equal((await response(d,'indexed','alpha',{cursor,kind_ids:'different'})).status,409);
  assert.equal((await response(d,'indexed','alpha',{cursor:'not-a-cursor'})).status,400);
  d.sqlite.exec('UPDATE knowledge_exploration_clock SET epoch=epoch+2');
  assert.equal((await response(d,'indexed','alpha',{cursor})).status,409);
 }finally{d.close();}
});
test('generated continuation fits its own private decode budget or refuses before returning a page',async()=>{
 const d=database();try{
  const kind=JSON.parse(fixture.rows.nodes[0].raw).kind_id;
  const filters=[kind,...Array.from({length:40},(_,at)=>String(at)+'x'.repeat(100))].join(',');
  const result=await response(d,'indexed','alpha',{limit:'1',kind_ids:filters});assert.equal(result.status,413);assert.match(await result.text(),/cursor byte budget/);
 }finally{d.close();}
});
test('missing or damaged required source/header/search carriers fail closed without fallback',async()=>{
 const changes=[
  "DELETE FROM edge_meta WHERE key='knowledge_reader_top'",
  "DELETE FROM edge_meta WHERE key='knowledge_node_digest:philosophy:z'",
  "UPDATE knowledge_nodes SET json=replace(json,'9007199254740993','9007199254740994') WHERE id='philosophy:z'",
  "UPDATE knowledge_nodes SET native_id='wrong' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET document_digest='wrong' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET visible_values='[]' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET identity_values='not-json' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET document_chars='wrong' WHERE id='philosophy:z'",
  "UPDATE knowledge_search_documents SET id_lower='wrong' WHERE id='philosophy:z'",
  "DROP TABLE knowledge_search_documents",
 ];
 for(const mode of ['legacy','indexed'])for(const sql of changes){const d=database();try{d.sqlite.exec(sql);assert.equal((await response(d,mode,'philosophy:z')).status,503,mode+': '+sql);}finally{d.close();}}
 const d=database();try{d.sqlite.exec("DELETE FROM knowledge_search_documents WHERE kind='nodes'");assert.equal((await response(d,'legacy','')).status,503);}finally{d.close();}
 for(const sql of ["DELETE FROM knowledge_search_documents WHERE kind='nodes'","DELETE FROM knowledge_search_gram_stats WHERE kind='nodes'"]){const d=database();try{d.sqlite.exec(sql);assert.equal((await response(d,'indexed','αβγ')).status,503);}finally{d.close();}}
});
test('row/header byte limits and indexed preflight budgets reject before oversize text/rank evaluation',async()=>{
 for(const mode of ['legacy','indexed'])for(const kind of ['row','header']){const d=database();try{
  const oversized='x'.repeat(1048577);
  if(kind==='row')d.sqlite.prepare("UPDATE knowledge_nodes SET json=? WHERE id='philosophy:z'").run(oversized);
  else d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(oversized);
  assert.equal((await response(d,mode,'philosophy:z')).status,413);
  assert.equal(d.statements.some(entry=>entry.results.some(row=>Object.values(row).some(value=>typeof value==='string'&&Buffer.byteLength(value)>1048576))),false);
 }finally{d.close();}}
 for(const [sql,status]of [
  ["UPDATE knowledge_search_documents SET document_chars=16000001,identity_values='not-json' WHERE id='philosophy:z'",413],
  ["UPDATE knowledge_search_gram_stats SET postings=50001",413],
  ["UPDATE knowledge_search_gram_stats SET postings=-1",503],
  ["UPDATE knowledge_search_gram_stats SET postings=zeroblob(1048577) WHERE kind='nodes' AND gram='phi'",503],
  ["UPDATE knowledge_search_documents SET document_chars=-1 WHERE id='philosophy:z'",503],
 ]){const d=database();try{d.sqlite.exec(sql);assert.equal((await response(d,'indexed','philosophy:z')).status,status);}finally{d.close();}}
});
test('retained invalid Unicode strings and keys refuse the Python UTF-8 response boundary',async()=>{
 for(const mode of ['legacy','indexed'])for(const key of [false,true]){const d=database();try{
  const row=d.sqlite.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_reader_top'").get();
  const malformed=key?'"\\ud800":"bad",':'"bad":"\\ud800",';
  d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(row.json_chunk.replace('"unknown":',malformed+'"unknown":'));
  const result=await response(d,mode,'alpha');assert.equal(result.status,503);assert.match(await result.text(),/invalid Unicode/);
 }finally{d.close();}}
 assert.equal(python("p={'x':'\\ud800'};\ntry: json.dumps(p,ensure_ascii=False).encode('utf-8');print('false')\nexcept UnicodeEncodeError: print('true')"),true);
});
test('aggregate selected ID budget masks oversized headers in SQL before source bodies are read',async()=>{
 const d=database();try{
  for(const [index,row]of fixture.rows.nodes.entries()){
   const id=JSON.parse(row.raw).id,big=String(index).padStart(2,'0')+'x'.repeat(900000);
   d.sqlite.prepare('UPDATE knowledge_nodes SET id=? WHERE id=?').run(big,id);
   d.sqlite.prepare('UPDATE knowledge_search_documents SET id=?,id_lower=? WHERE id=?').run(big,big,id);
  }
  assert.equal((await response(d,'legacy','',{limit:'100'})).status,413);
  assert.equal(d.statements.some(row=>row.sql.includes('json_bytes')&&row.sql.includes('FROM knowledge_nodes')),false);
  const selection=d.statements.find(row=>row.sql.includes('AS _native_valid FROM selected'));
  assert.ok(selection);assert.ok(selection.results.some(row=>row._native_bytes>16*1024*1024&&row.id===null&&row.id_lower===null));
 }finally{d.close();}
});
test('publication A-to-B-to-A during either search mode is rejected at final read boundary',async()=>{
 for(const mode of ['legacy','indexed']){const d=database();try{
  let changed=false;d.hook.after=(sql)=>{if(!changed&&sql.includes('AS _native_valid FROM selected')){changed=true;
   d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(JSON.stringify({sha256:'b'.repeat(64)}));
   d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(JSON.stringify({sha256:'d'.repeat(64)}));
  }};
  assert.equal((await response(d,mode,'αβγ')).status,409);assert.equal(changed,true);
 }finally{d.close();}}
});
test('selection plans retain linear legacy carrier scan plus primary-key lookup and indexed posting seeks',async()=>{
 const d=database();try{
  await direct(d,'legacy','alpha');await direct(d,'indexed','alpha');
  const plans=d.statements.filter(row=>row.sql.includes('AS _native_valid FROM selected')).map(row=>d.sqlite.prepare('EXPLAIN QUERY PLAN '+row.sql).all(...row.args).map(step=>step.detail));
  assert.equal(plans.length,4);
  for(const [index,plan]of plans.entries()){
   assert.ok(plan.some(line=>/SEARCH (n|r|b) USING INDEX sqlite_autoindex_knowledge_(nodes|relations)_1 \(id=\?\)/.test(line)),JSON.stringify(plan));
   if(index<2)assert.ok(plan.some(line=>/SEARCH s USING INDEX sqlite_autoindex_knowledge_search_documents_1 \(kind=\?\)/.test(line)),JSON.stringify(plan));
   else assert.ok(plan.some(line=>/SEARCH g USING COVERING INDEX sqlite_autoindex_knowledge_search_grams_1/.test(line)),JSON.stringify(plan));
  }
 }finally{d.close();}
});
test('real D1 Worker first/continuation native bytes survive isolate restart without cache or schema writes',async()=>{
 const bundle=await build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022'});
 const directory=mkdtempSync(join(tmpdir(),'tos-native-search-d1-'));let firstRaw,cursor;
 const expected=oracle('indexed','αβγ',{limit:1});
 try{for(let launch=0;launch<2;launch++){
  const mf=new Miniflare(convertV4MiniflareOptions({modules:true,script:bundle.outputFiles[0].text,d1Databases:['DB'],resourcePersistencePath:directory}));
  try{const db=await mf.getD1Database('DB');
   if(launch===0){const statements=[...schema.split(';').filter(value=>value.trim()).map(sql=>db.prepare(sql)),
    ...migration.replace(/^--.*$/gm,'').trim().split(/\n(?=CREATE |INSERT )/).map(sql=>db.prepare(sql)),
    ...Object.entries(fixture.metadata[9]).map(([key,raw])=>db.prepare('INSERT INTO edge_meta VALUES (?,0,?)').bind(key,raw))];
    for(const kind of ['nodes','relations'])for(const row of fixture.rows[kind]){const args=bindings(kind,row);statements.push(db.prepare(`INSERT INTO knowledge_${kind} VALUES (${args.map(()=>'?').join(',')})`).bind(...args));}
    for(const [table,key]of [['knowledge_search_documents','documents'],['knowledge_search_grams','grams'],['knowledge_search_gram_stats','stats']]){
     const values=fixture[key],size=Math.floor(96/values[0].length);
     for(let at=0;at<values.length;at+=size){const page=values.slice(at,at+size);statements.push(db.prepare(`INSERT INTO ${table} VALUES `+page.map(args=>'('+args.map(()=>'?').join(',')+')').join(',')).bind(...page.flat()));}
    }
    for(let at=0;at<statements.length;at+=128)await db.batch(statements.slice(at,at+128));
   }
   const first=await mf.dispatchFetch('https://tos.test/api/knowledge/search?mode=indexed&query='+encodeURIComponent('αβγ')+'&limit=1');const raw=await first.text();assert.equal(first.status,200,raw);assertPackets(raw,expected[0],true);
   if(launch===0){firstRaw=raw;cursor=JSON.parse(raw).page.next_cursor;}else assert.equal(raw,firstRaw);
   const next=await mf.dispatchFetch('https://tos.test/api/knowledge/search?mode=indexed&query='+encodeURIComponent('αβγ')+'&limit=1&cursor='+encodeURIComponent(cursor));const nextRaw=await next.text();assert.equal(next.status,200,nextRaw);assertPackets(nextRaw,expected[1],true);
  }finally{await mf.dispose();}
 }}finally{rmSync(directory,{recursive:true,force:true});}
});

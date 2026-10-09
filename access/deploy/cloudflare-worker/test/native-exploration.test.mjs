import {parseLosslessJson,stringifyLosslessJson,compareLosslessValues} from './lossless-json-compare.mjs';
import {frozenPythonOracleExec} from './frozen-python-oracle.mjs';
import assert from 'node:assert/strict';
import test from 'node:test';
import {mkdtempSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {DatabaseSync} from 'node:sqlite';
import {createHash} from 'node:crypto';
import {publishedNodeFixtureWorker,publishedWorkerFixtureModules} from './native-lens-fixture.ts';
import {Miniflare,convertV4MiniflareOptions} from 'miniflare';
import {HttpError} from '../src/common.ts';

const repo=fileURLToPath(new URL('../../../../',import.meta.url));
const sha=raw=>createHash('sha256').update(raw).digest('hex');
const python=(code,input)=>JSON.parse(frozenPythonOracleExec(import.meta.url, ['-B','-c',"import sys,json;sys.path[:0]=['access/src','access/tests','access/deploy/cloudflare-worker/scripts'];"+code],
 {cwd:repo,input:input===undefined?undefined:JSON.stringify(input),encoding:'utf8',timeout:30000,maxBuffer:32*1024*1024}));
const fixture=python(String.raw`
from test_exploration_origin import origin_graph
from tos_access.published_read_metadata import published_reader_metadata,emitted_row_digest,published_row_digest_key
from build_runtime import compact_json
g=origin_graph(seed=1,identities=True)
g['schema']='tos_knowledge_graph_v1'
probe={'11':9007199254740993,'2':1.0,'zero':-0.0,'float':1e-7,'tiny':5e-324,'nested':[2**100,-(2**100),False,None,{'z':1,'a':1.0}]}
g['normalization_binding']={'schema':'tos_knowledge_graph_normalization_binding_v1',**{k:'0'*64 for k in ('processor_digest','entity_registry_digest','relation_registry_digest','configuration_digest')}}
g['authority_boundary']={'source_owner':'Tree-of-Sophia','is_source':False,'is_canon':False,'writes_to_tree':False,'transport':probe}
for item in g['nodes']+g['relations']:
 item['transport_probe']=probe
 item['semantics']['transport_probe']=probe
 item['attributes']['dropped_probe']=probe
 item['readable_context']={'dropped':probe}
metadata=published_reader_metadata(g,{'schema':'tos_knowledge_catalog_v1','source_revision':g['source_revision']},'tos_cloudflare_edge_read_model_v8','d'*64)
metadata['data_revision']={'sha256':'d'*64}
metadata['knowledge_exploration_top']={k:g[k] for k in ('source_revision','authority_boundary')}
for kind in ('node','relation'):
 for item in g[kind+'s']:metadata[published_row_digest_key(kind,item['id'])]=emitted_row_digest(compact_json(item))
print(json.dumps({'nodes':[compact_json(n) for n in g['nodes']],'relations':[compact_json(r) for r in g['relations']],
 'metadata':{k:compact_json(v) for k,v in metadata.items()}}))
`);
const migration=readFileSync(new URL('../migrations/0001-exploration.sql',import.meta.url),'utf8');
const schema=`CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part));
 CREATE TABLE knowledge_nodes(id TEXT,entity_id TEXT,native_id TEXT,source_graph TEXT,kind_id TEXT,type_id TEXT,json TEXT);
 CREATE TABLE knowledge_relations(id TEXT,native_id TEXT,source_graph TEXT,from_id TEXT,to_id TEXT,predicate_id TEXT,relation_type_id TEXT,json TEXT);
 CREATE INDEX knowledge_nodes_native_idx ON knowledge_nodes(native_id);
 CREATE INDEX knowledge_nodes_entity_idx ON knowledge_nodes(entity_id);
 CREATE INDEX knowledge_relations_native_idx ON knowledge_relations(native_id);`;
const fields={node:['id','entity_id','native_id','source_graph','kind_id','type_id'],relation:['id','native_id','source_graph','from_id','to_id','predicate_id','relation_type_id']};
const bindings=(kind,raw)=>{const value=JSON.parse(raw);return [...fields[kind].map(k=>value[k]),raw];};
function database() {
 const directory=mkdtempSync(join(tmpdir(),'tos-native-exploration-')),path=join(directory,'published.sqlite'),sqlite=new DatabaseSync(path);
 sqlite.exec(schema);sqlite.exec(migration);
 for(const [key,raw]of Object.entries(fixture.metadata))sqlite.prepare('INSERT INTO edge_meta VALUES (?,0,?)').run(key,raw);
 for(const kind of ['node','relation'])for(const raw of fixture[kind+'s'])sqlite.prepare(`INSERT INTO knowledge_${kind}s VALUES (${bindings(kind,raw).map(()=>'?').join(',')})`).run(...bindings(kind,raw));
 const statements=[],hook={after:null,beforeBatch:null};
 const db={prepare(sql){let args=[];const statement={bind(...values){args=values;return statement;},
  async all(){const results=sqlite.prepare(sql).all(...args);statements.push({sql,args,rows:results.length,stringBytes:results.flatMap(row=>Object.values(row).filter(v=>typeof v==='string').map(v=>Buffer.byteLength(v)))});hook.after?.(sql,args);return {results,meta:{rows_read:0}};},
  async first(column){const result=await statement.all();return column?result.results[0]?.[column]??null:result.results[0]??null;}};return statement;},
  async batch(batch){await hook.beforeBatch?.();sqlite.exec('BEGIN IMMEDIATE');try {const results=[];for(const statement of batch)results.push(await statement.all());sqlite.exec('COMMIT');return results;}catch(error){sqlite.exec('ROLLBACK');throw error;}}};
 const binding=()=>{const raw=sqlite.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_reader_top'").get().json_chunk,top=JSON.parse(raw);
  return {schema:'tos_published_knowledge_snapshot_v1',metadata_sha256:sha(raw),publication_epoch:sqlite.prepare('SELECT epoch FROM knowledge_exploration_clock').get().epoch,
   ...Object.fromEntries(['read_model_schema','source_revision','data_revision','graph_schema','normalization_binding'].map(k=>[k,top[k]]))};};
 return {sqlite,db,hook,statements,path,binding,close(){sqlite.close();rmSync(directory,{recursive:true,force:true});}};
}
const node=JSON.parse(fixture.nodes[0]),edge=JSON.parse(fixture.relations[0]);
const request=(kind='node',options={})=>({schema_version:'tos_exploration_request_v2',source_revision:'a'.repeat(64),
 origin:{kind,id:(kind==='node'?node:edge).id,content_revision:(kind==='node'?node:edge).content_revision},...options});
function oracle(data,request,stream=true) {
 return python(String.raw`
from tos_access.published_read_model import PublishedKnowledgeReadModel,PublishedReadBudgetExceeded,PublishedSnapshotConflict,PublishedReadModelError
from tos_access.published_exploration import PublishedExplorationService
from tos_access.exploration_origin import ExplorationReadModelInvalid
from tos_access.lens_pagination import KnowledgeRevisionConflict
p=json.load(sys.stdin)
try:
 service=PublishedExplorationService(PublishedKnowledgeReadModel(p['path'],p['binding']))
 packet=service.explore(p['request']);pages=[packet]
 while p['stream'] and packet['page']['next_cursor']:
  packet=service.explore({'cursor':packet['page']['next_cursor']});pages.append(packet)
  if len(pages)>500:raise RuntimeError('nonterminating bounded fixture')
 print(json.dumps({'status':200,'pages':[json.dumps(v,ensure_ascii=False,separators=(',',':'),allow_nan=False) for v in pages]}))
except Exception as e:
 status=413 if isinstance(e,PublishedReadBudgetExceeded) else 409 if isinstance(e,(PublishedSnapshotConflict,KnowledgeRevisionConflict)) else 503 if isinstance(e,(PublishedReadModelError,ExplorationReadModelInvalid)) else 404 if isinstance(e,KeyError) else 400 if isinstance(e,ValueError) else 500
 print(json.dumps({'status':status,'error':str(e)}))
`,{path:data.path,binding:data.binding(),request,stream});
}
const worker=publishedNodeFixtureWorker;
async function exploreD1(db,request) {
 const result=await response({db},request),raw=await result.text();
 if(result.status!==200)throw new HttpError(result.status,raw);
 return raw;
}
async function response(data,request,raw){return (await worker()).fetch(new Request('https://tos.test/api/knowledge/explore',{
 method:'POST',headers:{'Content-Type':'application/json'},body:raw??JSON.stringify(request)}),{DB:data.db,ASSETS:{fetch(){throw Error('no source/asset fallback');}}},{});}
async function stream(data,query){const pages=[];for(let count=0;count<500;count++){
 const result=await response(data,query),raw=await result.text();assert.equal(result.status,200,raw);pages.push(raw);
 if('cursor'in query)assert.equal(await(await response(data,query)).text(),raw,'replay is identical bytes');
 const cursor=JSON.parse(raw).page.next_cursor;if(!cursor)return pages;query={cursor};
 }throw Error('nonterminating bounded fixture');}
function compareStreams(actual,expected){
 const a=actual.map(parseLosslessJson),b=expected.map(parseLosslessJson);
 const field=(node,key)=>{assert.equal(node.kind,'dict');const pair=node.value.find(([name])=>name===key);assert.ok(pair,`missing ${key}`);return pair[1];};
 const nullValue={kind:'null',value:null};
 // Freeze independent scene outputs for exactly the carriers the former pure
 // Python function reads. The comparisons below always evaluate current data.
 const scenes=python(String.raw`
from tos_access.knowledge import knowledge_scene
p=json.load(sys.stdin);out=[]
for raw in p:
 c=json.loads(raw)
 out.append(json.dumps(knowledge_scene(c['nodes'],c['relations'],c['focus'],c['relation']),ensure_ascii=False,separators=(',',':'),allow_nan=False))
print(json.dumps(out))
`,a.map(page=>{
   const origin=page.value.find(([name])=>name==='origin')?.[1];
   const hasOrigin=origin?.kind==='dict';
   const kind=hasOrigin?field(origin,'kind').value:null;
   const focus=hasOrigin?(kind==='node'?field(origin,'id'):nullValue):field(field(page,'focus'),'node_id');
   const relation=kind==='relation'?field(origin,'id'):nullValue;
   return stringifyLosslessJson({kind:'dict',value:[['nodes',field(page,'nodes')],['relations',field(page,'relations')],['focus',focus],['relation',relation]]});
 }));
 assert.equal(scenes.length,a.length);
 for(let i=0;i<a.length;i++)assert.deepEqual(compareLosslessValues(field(a[i],'scene'),parseLosslessJson(scenes[i]),{unordered:()=>true}),[],`page ${i} scene`);
 function union(pages,kind){
   const result=new Map();
   for(const page of pages){const values=field(page,kind);assert.equal(values.kind,'list');for(const value of values.value){
     const id=field(value,'id');assert.equal(id.kind,'str');
     if(result.has(id.value))assert.deepEqual(compareLosslessValues(value,result.get(id.value)),[],`${kind}.${id.value} repeated source`);
     result.set(id.value,value);
   }}
   return result;
 }
 for(const kind of ['nodes','relations']){
   const left=union(a,kind),right=union(b,kind);
   assert.deepEqual([...left.keys()].sort(),[...right.keys()].sort(),`${kind} full traversal selection`);
   for(const [id,value]of left)assert.deepEqual(compareLosslessValues(value,right.get(id)),[],`${kind}.${id}`);
 }
 assert.ok(a.length&&b.length);
 for(const key of ['status','limit_reason','counts','authority_boundary'])assert.deepEqual(compareLosslessValues(field(a.at(-1),key),field(b.at(-1),key)),[],key);
}

test('native exploration full traversal selections and source packets equal current published Python',async t=>{
 for(const kind of ['node','relation'])for(const profile of ['all','overview'])for(const direction of ['either','incoming','outgoing'])await t.test(kind+' '+profile+' '+direction,async()=>{
  const data=database();try {const query=request(kind,{profile,direction,max_depth:2,page_nodes:2,page_relations:2});
   const expected=oracle(data,query);assert.equal(expected.status,200,expected.error);
   const pages=await stream(data,query);compareStreams(pages,expected.pages);
   for(const raw of pages){assert.match(raw,/9007199254740993/);assert.match(raw,/"zero":-0\.0/);assert.match(raw,/"2":1\.0/);
    const packet=JSON.parse(raw);for(const value of [...packet.nodes,...packet.relations]){assert.deepEqual(value.attributes,{});assert.equal('readable_context'in value,false);}}
   assert.equal(data.statements.some(s=>/knowledge_catalog|knowledge_top| OFFSET |COUNT\(/.test(s.sql)),false);
  }finally{data.close();}
 });
});

test('native exploration replay and concurrent CAS winner retain identical stored bytes',async()=>{
 const data=database();try {
  const first=JSON.parse(await exploreD1(data.db,request('relation',{page_nodes:1,page_relations:1}))),cursor=first.page.next_cursor;assert.ok(cursor);
  const raw=await exploreD1(data.db,{cursor});
  assert.equal(data.sqlite.prepare('SELECT response FROM knowledge_exploration_checkpoints WHERE token=?').get(cursor).response,raw);
  assert.equal(await exploreD1(data.db,{cursor}),raw);
  const state=data.sqlite.prepare('SELECT state FROM knowledge_exploration_checkpoints WHERE token=?').get(JSON.parse(raw).page.next_cursor).state;
  assert.doesNotMatch(state,/transport_probe|9007199254740993|authority_boundary|display_selection/);
  assert.equal(data.sqlite.prepare('SELECT version FROM knowledge_exploration_checkpoints WHERE token=?').get(cursor).version,'tos-exploration-d1-execution-v6/rust-state-v1');
 }finally{data.close();}
});

test('old scene continuation and replay versions are refused without changing stored rows',async()=>{
 for(const replay of [false,true]){
  const data=database();try{
   const first=JSON.parse(await exploreD1(data.db,request('relation',{page_nodes:1,page_relations:1}))),cursor=first.page.next_cursor;
   assert.ok(cursor);
   if(replay)await exploreD1(data.db,{cursor});
   data.sqlite.prepare("UPDATE knowledge_exploration_checkpoints SET version='tos-exploration-d1-execution-v6/native-json-v1' WHERE token=?").run(cursor);
   const before=data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints ORDER BY token').all();
   const result=await response(data,{cursor});assert.equal(result.status,409);
   assert.deepEqual(data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints ORDER BY token').all(),before);
   assert.equal((await response(data,request('relation',{page_nodes:1,page_relations:1}))).status,200);
  }finally{data.close();}
 }
});

test('old lossy cache version, ABA and source failure never admit a successful checkpoint',async()=>{
 for(const mutation of ['old-version','aba','digest','state-float','state-carrier','state-oversize','response-oversize','sql-unavailable','checkpoint-scalar']){
  const data=database();try{
   const initial=JSON.parse(await exploreD1(data.db,request('node',{page_nodes:1,page_relations:1}))),cursor=initial.page.next_cursor;
   if(mutation==='old-version')data.sqlite.prepare("UPDATE knowledge_exploration_checkpoints SET version='tos-exploration-d1-execution-v6' WHERE token=?").run(cursor);
   if(mutation==='aba')for(let i=0;i<2;i++)data.sqlite.exec("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'");
   if(mutation==='digest')data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key=?").run(JSON.stringify({sha256:'0'.repeat(64)}),'knowledge_node_digest:'+node.id);
   if(mutation==='state-float')data.sqlite.exec("UPDATE knowledge_exploration_checkpoints SET state=replace(state,'\"page_number\":1','\"page_number\":1.0')");
   if(mutation==='state-carrier')data.sqlite.exec("UPDATE knowledge_exploration_checkpoints SET state=json_set(state,'$.carrier',json('{}'))");
   if(['state-float','state-carrier'].includes(mutation))data.sqlite.exec('UPDATE knowledge_exploration_checkpoints SET bytes=length(CAST(state AS BLOB))');
   if(mutation==='state-oversize')data.sqlite.prepare('UPDATE knowledge_exploration_checkpoints SET state=?').run('x'.repeat(1048577));
   if(mutation==='response-oversize')data.sqlite.prepare('UPDATE knowledge_exploration_checkpoints SET state=NULL,response=?').run('x'.repeat(1048577));
   if(mutation==='sql-unavailable')data.sqlite.exec('ALTER TABLE knowledge_nodes RENAME COLUMN type_id TO unavailable_type_id');
   if(mutation==='checkpoint-scalar')data.sqlite.prepare('UPDATE knowledge_exploration_checkpoints SET expires=?').run('x'.repeat(1048577));
   const before=data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints ORDER BY token').all();
   const result=await response(data,{cursor});assert.equal(result.status,['old-version','aba'].includes(mutation)?409:mutation.endsWith('oversize')?413:503,mutation+': '+await result.text());
   assert.deepEqual(data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints ORDER BY token').all(),before,mutation);
   if(mutation==='checkpoint-scalar')assert.ok(data.statements.every(s=>s.stringBytes.every(bytes=>bytes<=1048576)));
  }finally{data.close();}
 }
});

test('raw exploration request integer kinds are rejected before any D1 access',async()=>{
 const db=new Proxy({},{get(){throw Error('unexpected D1 access');}});
 for(const raw of ['{"focus_node_id":"0","max_depth":1.0}','{"focus_node_id":"0","page_nodes":1e0}',
  '{"focus_node_id":"0","page_relations":true}','{"focus_node_id":"\\u0085"}', '\ufeff{"focus_node_id":"0"}']){
  const result=await response({db},null,raw);assert.equal(result.status,400,await result.text());
 }
});

function replaceRow(data,kind,id,change) {
 const old=data.sqlite.prepare(`SELECT json FROM knowledge_${kind}s WHERE id=?`).get(id).json,raw=change(old);
 const value=JSON.parse(raw);
 data.sqlite.prepare(`UPDATE knowledge_${kind}s SET ${fields[kind].map(k=>k+'=?').join(',')},json=? WHERE id=?`).run(...fields[kind].map(k=>value[k]),raw,id);
 data.sqlite.prepare('INSERT OR REPLACE INTO edge_meta VALUES (?,0,?)').run(`knowledge_${kind}_digest:${value.id}`,JSON.stringify({sha256:sha(raw)}));
}
test('exploration source and pre-commit packet byte limits reject without delivering oversized input or changing cache',async()=>{
 for(const mode of ['row','header','packet']){
  const data=database();try{
   const query=request('relation',{max_depth:0});
   if(mode==='row')replaceRow(data,'node',node.id,raw=>raw+' '.repeat(1048577-Buffer.byteLength(raw)));
   if(mode==='header'){
    const raw=data.sqlite.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_reader_top'").get().json_chunk;
    data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(raw.slice(0,-1)+',"padding":"'+'x'.repeat(65536)+'"}');
   }
   if(mode==='packet')for(const id of [edge.from_id,edge.to_id])replaceRow(data,'node',id,raw=>raw.slice(0,-1)+',"retained_padding":"'+'x'.repeat(550000)+'"}');
   const expected=oracle(data,query,false);assert.equal(expected.status,mode==='packet'?200:413,mode+': '+expected.error);
   const before=data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints').all();
   for(let retry=0;retry<2;retry++){
    const result=await response(data,query),raw=await result.text();assert.equal(result.status,413,mode+': '+raw);
    assert.deepEqual(data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints').all(),before);
   }
   if(mode==='row')assert.ok(data.statements.every(s=>s.stringBytes.every(bytes=>bytes<=1048576)));
   if(mode==='header')assert.ok(data.statements.every(s=>s.stringBytes.every(bytes=>bytes<=65536)));
  }finally{data.close();}
 }
});

test('exploration replay rejects ABA occurring inside its final bounded metadata read',async()=>{
 const data=database();try{
  const first=JSON.parse(await exploreD1(data.db,request('relation',{page_nodes:1,page_relations:1}))),cursor=first.page.next_cursor;
  await exploreD1(data.db,{cursor});let tops=0,changed=false;
  data.hook.after=(sql,args)=>{
   if(args.includes('knowledge_exploration_top')&&++tops===2){changed=true;for(let i=0;i<2;i++)data.sqlite.exec("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'");}
  };
  const before=data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints ORDER BY token').all();
  const result=await response(data,{cursor});assert.equal(result.status,409,await result.text());assert.ok(changed);
  assert.deepEqual(data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints ORDER BY token').all(),before);
 }finally{data.close();}
});

test('crossed source at the actual checkpoint batch boundary cannot consume a cursor',async()=>{
 const data=database();try{
  const first=JSON.parse(await exploreD1(data.db,request('node',{page_nodes:1,page_relations:1}))),cursor=first.page.next_cursor;
  data.sqlite.prepare('INSERT INTO knowledge_exploration_checkpoints(token,expires,epoch,version,state,bytes) VALUES (?,0,0,?,\'{}\',2)').run('0'.repeat(64),'expired-other-cache');
  let changed=false;data.hook.beforeBatch=()=>{changed=true;data.sqlite.exec("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'");};
  const before=data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints ORDER BY token').all();
  const result=await response(data,{cursor});assert.equal(result.status,409,await result.text());assert.ok(changed);
  assert.deepEqual(data.sqlite.prepare('SELECT * FROM knowledge_exploration_checkpoints ORDER BY token').all(),before);
 }finally{data.close();}
});

test('source-filtered identity traversal and absent ordinary endpoints retain published selection semantics',async()=>{
 for(const mode of ['filtered','missing-endpoint']){
  const data=database();try{
   if(mode==='filtered')for(const raw of fixture.nodes.slice(1,4)){const id=JSON.parse(raw).id;replaceRow(data,'node',id,text=>text.replace('"source_graph":"philosophy"','"source_graph":"canon"'));}
   else data.sqlite.prepare('DELETE FROM knowledge_nodes WHERE id=?').run(JSON.parse(fixture.nodes.at(-1)).id);
   const query=request('node',{profile:'overview',sources:['philosophy'],max_depth:3,page_nodes:1,page_relations:1});
   const expected=oracle(data,query);assert.equal(expected.status,200,expected.error);compareStreams(await stream(data,query),expected.pages);
  }finally{data.close();}
 }
});

test('v1 exact entity and native focus plus v9 headers retain published semantics without lens execution',async()=>{
 for(const kind of ['exact','entity','native','v9']){
  const data=database();try{
   if(kind==='v9'){
    const raw=data.sqlite.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_reader_top'").get().json_chunk;
    data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(raw.replace('tos_published_knowledge_reader_v1','tos_published_knowledge_reader_v2').replace('tos_cloudflare_edge_read_model_v8','tos_cloudflare_edge_read_model_v9').slice(0,-1)+',"lens_sha256":"'+'0'.repeat(64)+'"}');
    data.sqlite.exec(`CREATE TABLE knowledge_lens_order(kind TEXT,id TEXT,sort_key TEXT,from_id TEXT,to_id TEXT);
      CREATE INDEX knowledge_lens_order_sort ON knowledge_lens_order(kind,sort_key,id);
      CREATE INDEX knowledge_lens_order_from ON knowledge_lens_order(kind,from_id,sort_key,id);
      CREATE INDEX knowledge_lens_order_to ON knowledge_lens_order(kind,to_id,sort_key,id);
      CREATE INDEX knowledge_lens_order_pair ON knowledge_lens_order(kind,from_id,to_id,id);`);
   }
   const query={focus_node_id:kind==='entity'?node.entity_id:kind==='native'?node.native_id:node.id,max_depth:1,page_nodes:2,page_relations:2};
   const expected=oracle(data,query);assert.equal(expected.status,200,expected.error);compareStreams(await stream(data,query),expected.pages);
   assert.equal(data.statements.some(s=>/FROM knowledge_lens_order|knowledge_lens_top|knowledge_catalog|knowledge_top/.test(s.sql)),false);
  }finally{data.close();}
 }
});

test('invalid retained Unicode refuses before cache while compact-omitted attributes stay omitted',async()=>{
 for(const mode of ['retained-value','retained-key','omitted']){
  const data=database();try{
   replaceRow(data,'node',node.id,raw=>mode==='omitted'?raw.replace('"attributes":{','"attributes":{"invalid":"\\ud800",')
    :raw.slice(0,-1)+(mode==='retained-key'?',"\\ud800":"bad"}':',"invalid":"\\ud800"}'));
   const query=request('node',{max_depth:0}),expected=oracle(data,query,false),result=await response(data,query),raw=await result.text();
   assert.equal(expected.status,mode==='omitted'?200:503,expected.error);assert.equal(result.status,expected.status,raw);
   if(result.status===200)compareStreams([raw],expected.pages);
   assert.equal(data.sqlite.prepare('SELECT count(*) AS n FROM knowledge_exploration_checkpoints').get().n,0);
  }finally{data.close();}
 }
});

test('reserved source IDs stay owned inclusion keys rather than JavaScript prototype operations',async()=>{
 const data=database();try{
  const old=JSON.parse(fixture.nodes[1]).id,id='__proto__';
  replaceRow(data,'node',old,raw=>raw.replace('"id":"'+old+'"','"id":"'+id+'"'));
  for(const raw of fixture.relations){const relation=JSON.parse(raw);if(relation.from_id===old||relation.to_id===old)
   replaceRow(data,'relation',relation.id,text=>text.replace('"from_id":"'+old+'"','"from_id":"'+id+'"').replace('"to_id":"'+old+'"','"to_id":"'+id+'"'));}
  const query={focus_node_id:node.id,profile:'all',direction:'outgoing',max_depth:1},expected=oracle(data,query);
  assert.equal(expected.status,200,expected.error);const pages=await stream(data,query);compareStreams(pages,expected.pages);
  const packet=JSON.parse(pages[0]);assert.ok(packet.nodes.some(row=>row.id===id));assert.equal(Object.hasOwn(packet.inclusion.nodes,id),true);
  assert.equal(packet.inclusion.nodes[id].kind,'traversal');
 }finally{data.close();}
});

test('real D1 raw HTTP restart and concurrent retries preserve native checkpoint pages',async()=>{
 const directory=mkdtempSync(join(tmpdir(),'tos-native-explore-real-')),modules=await publishedWorkerFixtureModules();
 const options=()=>convertV4MiniflareOptions({...modules,d1Databases:['DB'],resourcePersistencePath:directory});
 let mf=new Miniflare(options());
 const post=query=>mf.dispatchFetch('https://tos.test/api/knowledge/explore',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(query)});
 try {
  let db=await mf.getD1Database('DB');
  for(const sql of schema.split(';').map(v=>v.trim()).filter(Boolean))await db.prepare(sql).run();
  for(const sql of migration.replace(/^--.*$/gm,'').trim().split(/\n(?=CREATE |INSERT )/))await db.prepare(sql).run();
  for(const [key,raw]of Object.entries(fixture.metadata))await db.prepare('INSERT INTO edge_meta VALUES (?,0,?)').bind(key,raw).run();
  for(const kind of ['node','relation'])for(const raw of fixture[kind+'s'])await db.prepare(`INSERT INTO knowledge_${kind}s VALUES (${bindings(kind,raw).map(()=>'?').join(',')})`).bind(...bindings(kind,raw)).run();
  const firstResponse=await post(request('relation',{page_nodes:1,page_relations:1}));assert.equal(firstResponse.status,200,await(firstResponse.status===200?Promise.resolve(''):firstResponse.text()));
  const first=await firstResponse.json(),cursor=first.page.next_cursor;assert.ok(cursor);
  await mf.dispose();mf=new Miniflare(options());db=await mf.getD1Database('DB');
  const pages=await Promise.all(Array.from({length:4},async()=>{const result=await post({cursor}),raw=await result.text();assert.equal(result.status,200,raw);return raw;}));
  for(const raw of pages)assert.equal(raw,pages[0]);assert.match(pages[0],/9007199254740993/);
  assert.equal(await db.prepare('SELECT response FROM knowledge_exploration_checkpoints WHERE token=?').bind(cursor).first('response'),pages[0]);
  assert.equal(await db.prepare('SELECT count(*) AS n FROM knowledge_exploration_checkpoints').first('n'),2);
  await mf.dispose();mf=new Miniflare(options());db=await mf.getD1Database('DB');assert.equal(await(await post({cursor})).text(),pages[0]);
  for(let i=0;i<2;i++)await db.prepare("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'").run();
  assert.equal((await post({cursor})).status,409);
 }finally{await mf.dispose();rmSync(directory,{recursive:true,force:true});}
});

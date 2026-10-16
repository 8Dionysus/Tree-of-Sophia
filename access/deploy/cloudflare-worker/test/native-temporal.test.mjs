import {compareLosslessJson} from './lossless-json-compare.mjs';
import {frozenReference} from './frozen-reference.mjs';
import assert from 'node:assert/strict';
import test from 'node:test';
import {mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {DatabaseSync} from 'node:sqlite';
import {createHash} from 'node:crypto';
import {build} from 'esbuild';
import {Miniflare, convertV4MiniflareOptions} from 'miniflare';

const sha = raw => createHash('sha256').update(raw).digest('hex');
const reference = (referenceId, input) => JSON.parse(frozenReference(import.meta.url, referenceId, input===undefined?undefined:JSON.stringify(input)));
const fixtures = reference("temporal/fixtures");
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
  const sqlite=new DatabaseSync(path);sqlite.exec('PRAGMA secure_delete=ON');sqlite.exec(schema);sqlite.exec(migration);
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
  return reference("temporal/compare",{path:database.path,binding,request});
}
function assertPackets(actual, expected, indexed = false) {
  assert.deepEqual(compareLosslessJson(actual, expected, {indexed}), []);
}

let workerPromise;
async function worker() {
  workerPromise??=build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022',
    plugins:[{name:'existing-node-wasm-module',setup(build){build.onLoad({filter:/\.wasm$/},({path})=>({
      contents:`export default new WebAssembly.Module(Uint8Array.from(atob(${JSON.stringify(readFileSync(path).toString('base64'))}),c=>c.charCodeAt(0)))`,loader:'js'}));}}]})
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

test('temporal actual raw Worker packets equal captured published reference packets across the retained semantic corpus',async t=>{
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
  ['historical-unsafe-equal', false],
  ['historical-unsafe-mismatch', false],
  ['historical-bool-integer-mismatch', false],
  ['historical-int-float-equal', false],
  ['document-line-float', true],
  ['document-line-unsafe-integer', true],
  ['document-line-bool-right', true],
  ['unicode-source-refs', false],
  ['unknown-native-context', false],
 ];
 for(const [name,documentary] of cases)await t.test(name,async()=>{
  const data=selected(documentary?'document-native-numbers':fixtures[0].name);
  try {
   const ids=reference("temporal/replace-time-value",{path:data.path,claim:data.request.left.node_id,mutation:name});
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

// Unique predecessor controls now exercise the default Rust HTTP route rather
// than keeping a second TS temporal algorithm as their executor.
test('invalid temporal requests fail 400 before any forbidden or unavailable D1 access',async()=>{
 const data=selected();try{
  let accesses=0;
  const forbidden=new Proxy({}, {get(){accesses++;throw Error('D1 access forbidden by request validation');}});
  for(const request of [null,[],{}, {...data.request,extra:true},
   {...data.request,source_revision:'invalid'}, {...data.request,left:{...data.request.left,node_id:' x'}},
   {...data.request,right:{...data.request.right,content_revision:'invalid'}},
   {...data.request,left:{...data.request.left,node_id:'😀'.repeat(1025)}}]){
   const actual=await response({db:forbidden,request:data.request},request);
   assert.equal(actual.status,400);await actual.arrayBuffer();
  }
  assert.equal(accesses,0);
  const actual=await response(data,{...data.request,left:{...data.request.left,node_id:'😀'.repeat(1024)}});
  assert.equal(actual.status,404);await actual.arrayBuffer();
 }finally{data.close();}
});

test('temporal escaped row keys and decoded duplicate identity use retained Rust route bytes',async()=>{
 for(const ambiguous of [false,true]){
  const data=selected('document-native-numbers');try{
   const claim=JSON.parse(data.sqlite.prepare('SELECT json FROM knowledge_nodes WHERE id=?').get(data.request.left.node_id).json);
   const valueId=claim.semantics.claim.object_node_id;
   for(const row of data.sqlite.prepare('SELECT id FROM knowledge_nodes').all())replaceRow(data,'node',row.id,raw=>{
    if(ambiguous)return row.id===valueId?raw.slice(0,-1)+',"\\u0061ttributes":'+JSON.stringify(JSON.parse(raw).attributes)+'}':raw;
    return ' \n'+raw.replace(/"(attributes|source_claim|semantics|time|raw|value)"(?=\s*:)/g,
     (_,key)=>'"\\u'+key.charCodeAt(0).toString(16).padStart(4,'0')+key.slice(1)+'"')+'\n ';
   });
   const actual=await response(data);
   if(ambiguous){assert.equal(actual.status,503);await actual.arrayBuffer();}
   else {const expected=oracle(data);assert.equal(actual.status,expected.status);assertPackets(await actual.text(),expected.raw);}
  }finally{data.close();}
 }
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
  const raw=reference("temporal/v8-metadata",{raw:data.sqlite.prepare("SELECT json_chunk FROM edge_meta WHERE key='knowledge_reader_top'").get().json_chunk});
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
 const bundle=await build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022',
  plugins:[{name:'existing-workerd-wasm-module',setup(build){build.onResolve({filter:/\.wasm$/},()=>({path:'./tos_web_rules_bg.wasm',external:true}));}}]});
 const modulesRoot=fileURLToPath(new URL('../generated/',import.meta.url));
 const mf=new Miniflare(convertV4MiniflareOptions({modulesRoot,modules:[
  {type:'ESModule',path:join(modulesRoot,'temporal-worker-test.mjs'),contents:bundle.outputFiles[0].text},
  {type:'CompiledWasm',path:join(modulesRoot,'tos_web_rules_bg.wasm'),contents:readFileSync(join(modulesRoot,'tos_web_rules_bg.wasm'))}],
  compatibilityDate:'2026-09-03',d1Databases:['DB']}));
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
  const changedResponse=await response({db:guarded,request:data.request});
  assert.equal(changedResponse.status,409);await changedResponse.arrayBuffer();assert.equal(changed,true);
 }finally{data.close();await mf.dispose();}
});

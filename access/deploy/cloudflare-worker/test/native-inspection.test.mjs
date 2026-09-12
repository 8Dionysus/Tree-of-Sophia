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
import {knowledgeNodeD1, knowledgeRelationD1} from '../src/knowledge-store.ts';
import {nativePacketResponse} from '../src/native-lens-response.ts';

const repo = fileURLToPath(new URL('../../../../',import.meta.url));
const sha = raw => createHash('sha256').update(raw).digest('hex');
const python = (code,input) => JSON.parse(execFileSync('python3',['-B','-c',
  "import sys,json;sys.path[:0]=['access/src','access/deploy/cloudflare-worker/scripts'];"+code],
  {cwd:repo,input:input===undefined?undefined:JSON.stringify(input),encoding:'utf8',timeout:30000,maxBuffer:32*1024*1024}));
const fixture = python(String.raw`
from tos_access.knowledge import _normalize_node,_normalize_relation
from tos_access.published_read_metadata import published_reader_metadata,published_lens_metadata,emitted_row_digest,published_row_digest_key
from build_runtime import compact_json
values={'10':9007199254740993,'2':1.0,'01':-0.0,'x':[9007199254740992,False,None,1e-7],
        '__proto__':{'constructor':1.0,'10':-0.0,'2':9007199254740993}}
nodes=[]
for source,name,native,entity in [('philosophy','a','shared-native','tos.entity.a'),('canon','b','shared-native','tos.entity.b'),
 ('philosophy','c','c','tos.entity.shared'),('source-navigation','d','d','tos.entity.shared'),
 ('philosophy','e','tos.entity.shared','tos.entity.e'),('philosophy','z','philosophy:a','tos.entity.z'),
 ('philosophy','\ue000','unicode','tos.entity.private'),('philosophy','😀','unicode','tos.entity.astral')]:
 n=_normalize_node({'node_id':name,'label':name,'properties':values,'source_ref':'test:inspection'},source)
 n['native_id']=native;n['entity_id']=entity;n['source_refs']=['test:inspection','',7,'😀','\ue000','test:inspection']
 n['unknown']={'10':1.0,'2':9007199254740993,'01':-0.0};nodes.append(n)
relations=[]
for source,name,a,b in [('philosophy','r',0,1),('canon','s',1,0),('philosophy','self',0,0),('philosophy','t',2,3)]:
 r=_normalize_relation({'edge_id':name,'from_id':nodes[a]['id'],'to_id':nodes[b]['id'],'predicate_id':'links','source_ref':'test:inspection'},source,{n['id']:n for n in nodes})
 r['from_id']=nodes[a]['id'];r['to_id']=nodes[b]['id'];r['native_id']='relation-alias' if name in ('r','s') else name
 r['unknown']=values;r['source_refs']=['test:relation','',False,'😀'];relations.append(r)
graph={'schema':'tos_knowledge_graph_v1','source_revision':'a'*64,'nodes':nodes,'relations':relations,
 'normalization_binding':{'schema':'tos_knowledge_graph_normalization_binding_v1',**{k:'0'*64 for k in ('processor_digest','entity_registry_digest','relation_registry_digest','configuration_digest')}},
 'authority_boundary':{'source_owner':'Tree-of-Sophia','is_source':False,'is_canon':False,'writes_to_tree':False,'unknown':values}}
metadata=published_reader_metadata(graph,{'schema':'tos_knowledge_catalog_v1','source_revision':graph['source_revision']},'tos_cloudflare_edge_read_model_v9','d'*64,lens_metadata=published_lens_metadata(graph))
metadata['data_revision']={'sha256':'d'*64}
raws={kind:[compact_json(item) for item in graph[key]] for kind,key in [('node','nodes'),('relation','relations')]}
for kind,key in [('node','nodes'),('relation','relations')]:
 for item,raw in zip(graph[key],raws[kind]):metadata[published_row_digest_key(kind,item['id'])]=emitted_row_digest(raw)
print(json.dumps({'nodes':raws['node'],'relations':raws['relation'],'metadata':{k:compact_json(v) for k,v in metadata.items()}}))
`);
const schema = `CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part));
 CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,entity_id TEXT,native_id TEXT,source_graph TEXT,kind_id TEXT,type_id TEXT,title_text TEXT,search_text TEXT,json TEXT);
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
function database() {
  const directory=mkdtempSync(join(tmpdir(),'tos-native-inspection-')), path=join(directory,'published.sqlite');
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
function oracle(database,kind,id,limit=200,binding=database.binding()) {
  return python(String.raw`
from tos_access.published_read_model import PublishedKnowledgeReadModel,PublishedReadBudgetExceeded,PublishedSnapshotConflict,PublishedReadModelError
p=json.load(sys.stdin)
try:
 reader=PublishedKnowledgeReadModel(p['path'],p['binding'])
 packet=reader.node(p['id'],p['limit']) if p['kind']=='node' else reader.relation(p['id'])
 print(json.dumps({'status':200,'raw':json.dumps(packet,ensure_ascii=False,separators=(',',':'),allow_nan=False)}))
except Exception as e:
 status=413 if isinstance(e,PublishedReadBudgetExceeded) else 409 if isinstance(e,PublishedSnapshotConflict) else 503 if isinstance(e,PublishedReadModelError) else 404 if isinstance(e,KeyError) else 400 if isinstance(e,ValueError) else 500
 print(json.dumps({'status':status,'error':str(e)}))
`,{path:database.path,binding,kind,id,limit});
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
async function response(database,kind,id,limit=200,method='GET') {
  return (await worker()).fetch(new Request(`https://tos.test/api/knowledge/${kind}s/${encodeURIComponent(id)}?relation_limit=${limit}`,{method}),
    {DB:database.db,ASSETS:{fetch(){throw new Error('inspection must not read catalog/static assets');}}},{});
}
function replaceRow(database,kind,id,change) {
  const original=database.sqlite.prepare(`SELECT json FROM knowledge_${kind}s WHERE id=?`).get(id).json;
  const raw=change(original), columns=kind==='node'?['id','entity_id','native_id','source_graph','kind_id','type_id']:['id','native_id','source_graph','from_id','to_id','predicate_id','relation_type_id'];
  const item=JSON.parse(raw);
  database.sqlite.prepare(`UPDATE knowledge_${kind}s SET ${columns.map(c=>c+'=?').join(',')},json=? WHERE id=?`).run(...columns.map(c=>item[c]),raw,id);
  database.sqlite.prepare('INSERT OR REPLACE INTO edge_meta VALUES (?,0,?)').run(`knowledge_${kind}_digest:${id}`,JSON.stringify({sha256:sha(raw)}));
}

test('native inspection actual Worker HTTP equals published Python full packets, alias precedence, limits and source order',async()=>{
  const data=database();try {
    for(const [kind,id,limit] of [['node','philosophy:a',200],['node','shared-native',1],['node','tos.entity.shared',0],
      ['node','unicode',1000],['relation','philosophy:r',200],['relation','relation-alias',200],['relation','philosophy:self',200]]) {
      const expected=oracle(data,kind,id,limit), result=await response(data,kind,id,limit);
      assert.equal(expected.status,200,expected.error);assert.equal(result.status,200);assertPackets(await result.text(),expected.raw);
    }
    const payloadReads=data.statements.filter(({sql})=>/WITH selected AS/.test(sql)&&/FROM knowledge_(nodes|relations)\s+WHERE id IN/.test(sql));
    assert.ok(payloadReads.length);assert.equal(data.statements.some(({sql,args})=>sql.includes('json_extract')||args.includes('knowledge_lens_top')||args.includes('knowledge_catalog')),false);
  }finally{data.close();}
});

test('inspection identifier Unicode stripping, unknown IDs and HEAD status agree with published Python',async()=>{
  const data=database();try {
    for(const kind of ['node','relation'])for(const id of ['', 'not-found','x'.repeat(4097), '\u0085'+(kind==='node'?'philosophy:a':'philosophy:r')+'\u001c', '\ufeff'+(kind==='node'?'philosophy:a':'philosophy:r')+'\ufeff']) {
      const expected=oracle(data,kind,id), result=await response(data,kind,id);
      assert.equal(result.status,expected.status,`${kind}/${JSON.stringify(id.slice(0,30))}`);
      if(expected.status===200)assertPackets(await result.text(),expected.raw);
      const head=await response(data,kind,id,200,'HEAD');assert.equal(head.status,expected.status);assert.equal(await head.text(),'');
    }
  }finally{data.close();}
});

test('inspection damaged digest, duplicate source members, incomplete endpoints and missing index refuse like published Python',async()=>{
  for(const mutation of [
    d=>d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_node_digest:philosophy:a'").run('{"sha256":"bad"}'),
    d=>replaceRow(d,'node','philosophy:a',raw=>raw.slice(0,-1)+',"unknown":1.0}'),
    d=>replaceRow(d,'relation','philosophy:r',raw=>{const r=JSON.parse(raw);r.to_id='missing';return JSON.stringify(r);}),
    d=>d.sqlite.exec('DROP INDEX knowledge_nodes_entity_idx'),
  ]) {const data=database();try {
    mutation(data);const kind=oracle(data,'relation','philosophy:r').status===503?'relation':'node',id=kind==='node'?'philosophy:a':'philosophy:r';
    const expected=oracle(data,kind,id), actual=await response(data,kind,id);assert.equal(expected.status,503,expected.error);assert.equal(actual.status,503);
  }finally{data.close();}}
});

test('inspection 129 alias matches refuse before full rows, while 128 matches stay complete',async()=>{
  for(const kind of ['node','relation']) {
    const data=database();try {
      const original=JSON.parse(fixture[kind+'s'][0]);
      for(let i=0;i<129;i++) {const item={...original,id:`philosophy:bulk-${String(i).padStart(3,'0')}`,native_id:'bulk-alias'},raw=JSON.stringify(item);
        data.sqlite.prepare(`INSERT INTO knowledge_${kind}s VALUES (${rowBindings(kind,raw).map(()=>'?').join(',')})`).run(...rowBindings(kind,raw));
        data.sqlite.prepare('INSERT INTO edge_meta VALUES (?,0,?)').run(`knowledge_${kind}_digest:${item.id}`,JSON.stringify({sha256:sha(raw)}));}
      assert.equal(oracle(data,kind,'bulk-alias').status,413);assert.equal((await response(data,kind,'bulk-alias')).status,413);
      assert.equal(data.statements.some(({sql})=>/SELECT id,.*json/.test(sql)&&/FROM knowledge_/.test(sql)),false,'refusal precedes payload loading');
      data.sqlite.prepare(`DELETE FROM knowledge_${kind}s WHERE id=?`).run('philosophy:bulk-128');
      const expected=oracle(data,kind,'bulk-alias'), actual=await response(data,kind,'bulk-alias');
      assert.equal(expected.status,200,expected.error);assert.equal(actual.status,200);assertPackets(await actual.text(),expected.raw);
    }finally{data.close();}
  }
});

test('inspection v8 is independent of lens metadata, and v9 does not read lens histograms or order rows',async()=>{
  for(const version of [8,9]) {const data=database();try {
    data.sqlite.exec("DELETE FROM edge_meta WHERE key='knowledge_lens_top'");
    if(version===8) {const raw=python("p=json.loads(json.load(sys.stdin));p['schema']='tos_published_knowledge_reader_v1';p['read_model_schema']='tos_cloudflare_edge_read_model_v8';p.pop('lens_sha256');print(json.dumps(json.dumps(p,ensure_ascii=False,separators=(',',':'))))",fixture.metadata.knowledge_reader_top);
      data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(raw);
      data.sqlite.exec('DROP TABLE knowledge_lens_order;DROP INDEX knowledge_nodes_identity_seek');}
    const expected=oracle(data,'relation','relation-alias'),actual=await response(data,'relation','relation-alias');
    assert.equal(expected.status,200,expected.error);assert.equal(actual.status,200);assertPackets(await actual.text(),expected.raw);
    assert.equal(data.statements.some(({sql,args})=>sql.includes('FROM knowledge_lens_order')||args.includes('knowledge_lens_top')),false);
  }finally{data.close();}}
});

test('inspection rejects non-emitted header framing even when selected raw metadata digest matches',async()=>{
  for(const mutate of [raw=>' '+raw,raw=>raw.replace('1e-07','1e-7'),raw=>raw.replace('Tree-of-Sophia','Tree-of-So\\u0070hia')]) {
    const data=database();try {
      const raw=mutate(fixture.metadata.knowledge_reader_top);
      data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(raw);
      const expected=oracle(data,'node','philosophy:a'),actual=await response(data,'node','philosophy:a');
      assert.equal(expected.status,503,expected.error);assert.equal(actual.status,503);
    }finally{data.close();}
  }
});

test('inspection clock guard rejects publication changes and A-B-A even when final bytes match',async()=>{
  for(const kind of ['node','relation'])for(const aba of [false,true]) {const data=database();try {
    const before=data.binding();let changed=false;
    data.hook.after=sql=>{if(!changed&&sql.includes('json_bytes FROM knowledge_')) {changed=true;
      data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(JSON.stringify({sha256:'e'.repeat(64)}));
      if(aba)data.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(fixture.metadata.data_revision);}};
    const actual=await response(data,kind,kind==='node'?'philosophy:a':'philosophy:r');assert.equal(changed,true);assert.equal(actual.status,409);
    assert.equal(oracle(data,kind,kind==='node'?'philosophy:a':'philosophy:r',200,before).status,aba?409:503);
  }finally{data.close();}}
});

test('inspection byte-budget classification matches published reader before text delivery',async()=>{
  for(const [name,mutate] of [
    ['row-one-byte-over',d=>replaceRow(d,'node','philosophy:a',raw=>{const item=JSON.parse(raw);item.padding='';const base=JSON.stringify(item);item.padding='x'.repeat(1048577-Buffer.byteLength(base));return JSON.stringify(item);})],
    ['digest-one-byte-over',d=>d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_node_digest:philosophy:a'").run(JSON.stringify({sha256:'x'.repeat(1012)}))],
    ['header-one-byte-over',d=>{const item=JSON.parse(fixture.metadata.knowledge_reader_top);item.authority_boundary.padding='';const base=JSON.stringify(item);item.authority_boundary.padding='x'.repeat(65537-Buffer.byteLength(base));d.sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='knowledge_reader_top'").run(JSON.stringify(item));}],
  ]) {const data=database();try {
    mutate(data);const expected=oracle(data,'node','philosophy:a'),actual=await response(data,'node','philosophy:a');
    assert.equal(expected.status,413,name+': '+expected.error);assert.equal(actual.status,expected.status,name);
  }finally{data.close();}}
});

test('inspection SQL masks oversized selected text and does not reread full selected rows',async()=>{
  const data=database();try {
    assert.equal((await response(data,'relation','relation-alias')).status,200);
    const selected=new Set();
    for(const {sql,args} of data.statements) if(sql.includes('json_bytes FROM knowledge_')) {
      const kind=sql.includes('json_bytes FROM knowledge_nodes')?'node':'relation';
      for(const id of JSON.parse(args[0])) {assert.equal(selected.has(kind+id),false,'full row read twice');selected.add(kind+id);}
    }
    assert.equal(selected.size,4,'two full relations and two unique full endpoints');
    data.statements.length=0;
    data.sqlite.prepare("UPDATE knowledge_nodes SET json=? WHERE id='philosophy:a'").run('x'.repeat(2*1024*1024));
    assert.equal((await response(data,'node','philosophy:a')).status,413);
    assert.ok(data.statements.every(s=>s.stringBytes.every(size=>size<=1048576)),'oversized source text is never delivered to Worker');
    // SQLite's hard runtime string limit fires before Python's typed check at
    // this size. D1 has no equivalent VM interruption; the refusal is explicit.
    assert.equal(oracle(data,'node','philosophy:a').status,503);
  }finally{data.close();}
});

test('inspection actual repeated source-ref response budget is 413 after bounded row delivery',async()=>{
  const data=database();try {
    const source=JSON.parse(fixture.nodes[0]);
    for(let i=0;i<12;i++) {const item={...source,id:'philosophy:wire-'+i,native_id:'wire-limit',source_refs:[String(i)+'x'.repeat(720000)]},raw=JSON.stringify(item);
      data.sqlite.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').run(...rowBindings('node',raw));
      data.sqlite.prepare('INSERT INTO edge_meta VALUES (?,0,?)').run('knowledge_node_digest:'+item.id,JSON.stringify({sha256:sha(raw)}));}
    assert.equal(oracle(data,'node','wire-limit',0).status,413);
    assert.equal((await response(data,'node','wire-limit',0)).status,413);
    assert.ok(data.statements.reduce((total,s)=>total+s.stringBytes.reduce((a,b)=>a+b,0),0)<16*1024*1024,'D1 delivery remained within budget; final repeated-ref writer refused');
    const head=await response(data,'node','wire-limit',0,'HEAD');assert.equal(head.status,413);assert.equal(await head.text(),'');
  }finally{data.close();}
});

test('real Miniflare D1 inspection preserves raw first HTTP serialization and ABA rejection',async()=>{
  const bundle=await build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022'});
  const mf=new Miniflare(convertV4MiniflareOptions({modules:true,script:bundle.outputFiles[0].text,compatibilityDate:'2026-09-03',d1Databases:['DB']}));
  const data=database();try {
    const db=await mf.getD1Database('DB');
    await db.batch(schema.split(';').map(s=>s.trim()).filter(Boolean).map(s=>db.prepare(s)));
    await db.batch(migration.replace(/^--.*$/gm,'').trim().split(/\n(?=CREATE |INSERT )/).map(s=>db.prepare(s)));
    const commands=[];
    for(const [key,raw] of Object.entries(fixture.metadata))commands.push(db.prepare('INSERT INTO edge_meta VALUES (?,0,?)').bind(key,raw));
    for(const kind of ['node','relation'])for(const raw of fixture[kind+'s'])commands.push(db.prepare(`INSERT INTO knowledge_${kind}s VALUES (${rowBindings(kind,raw).map(()=>'?').join(',')})`).bind(...rowBindings(kind,raw)));
    await db.batch(commands);
    for(const [kind,id] of [['node','shared-native'],['relation','relation-alias']]) {
      const expected=oracle(data,kind,id),result=await mf.dispatchFetch(`https://tos.test/api/knowledge/${kind}s/${id}`);
      assert.equal(result.status,200);assertPackets(await result.text(),expected.raw);
      const head=await mf.dispatchFetch(`https://tos.test/api/knowledge/${kind}s/${id}`,{method:'HEAD'});assert.equal(head.status,200);assert.equal(await head.text(),'');
    }
    for(const kind of ['node','relation']) {
      let changed=false;
      const guarded={prepare(sql){let statement=db.prepare(sql);return {bind(...args){statement=statement.bind(...args);return this;},async first(){return statement.first();},async all(){const result=await statement.all();
        if(!changed&&sql.includes('json_bytes FROM knowledge_')) {changed=true;await db.batch([
          db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(JSON.stringify({sha256:'e'.repeat(64)})),
          db.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").bind(fixture.metadata.data_revision)]);}
        return result;}};}};
      await assert.rejects(kind==='node'?knowledgeNodeD1(guarded,'philosophy:a',200):knowledgeRelationD1(guarded,'philosophy:r'),error=>error.status===409);assert.equal(changed,true);
    }
  }finally{data.close();await mf.dispose();}
});

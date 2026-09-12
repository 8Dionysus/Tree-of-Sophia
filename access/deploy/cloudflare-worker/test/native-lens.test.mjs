import assert from 'node:assert/strict';
import test from 'node:test';
import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {DatabaseSync} from 'node:sqlite';
import {createHash} from 'node:crypto';
import {build} from 'esbuild';
import {executeKnowledgeLensD1, focusKnowledgeNodeD1} from '../src/knowledge-store.ts';
import {executeNativeLensD1} from '../src/native-lens-store.ts';
import {nativeLensResponse} from '../src/native-lens-response.ts';
import {parseNativeRequest, parseNativeJson, nativePacketJson, nativeField, compileNativeSpec, nativeDigest,
  derived, nativePacketObject, nativePacketArray, nativeChild} from '../src/native-lens.ts';
import {NativeBudgetExceeded, nativeNumberInfo, nativeKeys} from '../../../shared/native-semantics.ts';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const python = (code, input) => JSON.parse(execFileSync('python3', ['-B', '-c', "import sys,json;sys.path[:0]=['access/src','access/deploy/cloudflare-worker/scripts'];" + code],
  {cwd: repo, input: input === undefined ? undefined : JSON.stringify(input), encoding: 'utf8', timeout: 30000, maxBuffer: 16 * 1024 * 1024}));
const fixture = python(String.raw`
import copy
from tos_access.knowledge import _normalize_node, _normalize_relation, execute_knowledge_lens
from tos_access.published_read_metadata import published_lens_metadata, published_reader_metadata, emitted_row_digest, published_row_digest_key
from build_runtime import compact_json, normalize_paths, REPO_ROOT
values=[('a-float',1.0),('b-int',1),('c-negative-zero',-0.0),('d-unsafe-int',2**53+1),('e-neighbor-int',2**53),
 ('f-dict',{'10':'ten','2':'two','01':'leading','z':1.0,'__proto__':{'constructor':-0.0}}),
 ('g-list',[True,1.0,None,{'z':0,'a':False}]),('h-false',False),('i-null',None),('j-zero',0),('k-empty',''),
 ('l-I-dot','İ'),('m-cyrillic','ЁЖ'),('n-sharp','ß'),('o-ss','ss'),('p-astral','😀tail'),('q-prefix-list',['x']),
 ('r-Z','Z'),('s-a','a'),('t-float-small',1e-5),('u-float-large',1e20)]
nodes=[_normalize_node({'node_id':name,'label':name,'properties':{'value':value},'source_ref':'test:native-lens'},'philosophy') for name,value in values]
subject={'id':'tos.agent.synthetic-native-forms','version':1,'digest':'sha256:'+'f'*64}
forms=[]
for index,(role,value) in enumerate([('name',1),('caption',1.0),('hover',2**53+1),('statement',2**53),('grounds',-0.0),('history',{'10':1.0,'2':2**53+1})]):
 forms.append({'schema_version':'tos_human_form_materialization_v1','form':{'id':'tos.form.synthetic-'+role,'version':1,'digest':'sha256:'+str(index)*64},
  'subject':subject,'role':role,'language':'en','state':'ready','display_text':'Synthetic '+role,'context':[{'slot':'test-only','binding':{'record':subject,'pointer':'/synthetic'},'value':value}],
  'standalone_reading':False,'performs_semantic_assessment':False})
form_properties={'source_record':{'record_id':subject['id'],'record_version':1},'source_sha256':'f'*64,'human_forms':forms,'human_forms_source_ref':'test:native-forms'}
nodes.append(_normalize_node({'node_id':'forms','label':'forms','properties':form_properties,'source_ref':'test:native-lens'},'philosophy'))
for name,mutation in [('float-form-version',lambda props:props['human_forms'][0]['form'].update(version=1.0)),('float-record-version',lambda props:props['source_record'].update(record_version=1.0))]:
 props=copy.deepcopy(form_properties);mutation(props)
 nodes.append(_normalize_node({'node_id':name,'label':name,'properties':props,'source_ref':'test:native-lens'},'philosophy'))
relations=[_normalize_relation({'edge_id':'edge-'+str(i),'from_id':nodes[a]['native_id'],'to_id':nodes[b]['native_id'],'predicate_id':'links','source_ref':'test:native-lens'},'philosophy',{n['id']:n for n in nodes}) for i,(a,b) in enumerate([(0,1),(1,0),(1,2),(2,2)])]
graph={'schema':'tos_knowledge_graph_v1','source_revision':'a'*64,'nodes':nodes,'relations':relations,'counts':{'nodes':len(nodes),'relations':len(relations)},
 'normalization_binding':{'schema':'tos_knowledge_graph_normalization_binding_v1',**{key:'0'*64 for key in ('processor_digest','entity_registry_digest','relation_registry_digest','configuration_digest')}},
 'authority_boundary':{'source_owner':'Tree-of-Sophia','is_source':False,'is_canon':False,'writes_to_tree':False}}
graph=normalize_paths(graph,REPO_ROOT)
graph['query_properties']=[{'property_id':'tos.property.native-number','field':'attributes.value','value_type':'number','applies_to':[nodes[0]['type_id']],'inherited':False,'operators':['eq','gt']},
 {'property_id':'tos.property.native-string','field':'attributes.value','value_type':'string','applies_to':[nodes[0]['type_id']],'inherited':False,'operators':['prefix']}]
metadata=published_reader_metadata(graph,{'schema':'tos_knowledge_catalog_v1','source_revision':graph['source_revision']},
 'tos_cloudflare_edge_read_model_v9','d'*64,lens_metadata=published_lens_metadata(graph))
metadata['data_revision']={'sha256':'d'*64}
metadata['knowledge_top']={key:graph[key] for key in ('source_revision','authority_boundary')}
raw_nodes=[compact_json(n) for n in graph['nodes']];raw_relations=[compact_json(r) for r in graph['relations']]
for kind,items,raws in [('node',graph['nodes'],raw_nodes),('relation',graph['relations'],raw_relations)]:
 for item,raw in zip(items,raws):metadata[published_row_digest_key(kind,item['id'])]=emitted_row_digest(raw)
base={'schema_version':'tos_lens_spec_v1','lens_id':'native-lens','sources':['philosophy'],'detail':'full','explain':True,
 'relation_query':{'enabled':False},'composition':{'sort_nodes':[{'field':'attributes.value','direction':'asc'}]}}
cases=[]
def add(name,spec,raw=None):
 raw=raw or compact_json(spec)
 try:expected=compact_json(execute_knowledge_lens(graph,json.loads(raw)));error=None
 except Exception as e:expected=None;error=type(e).__name__
 cases.append({'name':name,'rawSpec':raw,'expected':expected,'error':error})
add('all-native-sort',base)
add('native-groups',{**base,'composition':{'group_by':['attributes.value']}})
for name,value in [('unsafe',2**53+1),('bool',False),('null',None),('int',1),('float',1.0)]:
 add('eq-'+name,{**base,'node_query':{'filters':[{'field':'attributes.value','op':'eq','value':value}]}})
for op,value in [('contains','ё'),('prefix','i'),('prefix','😀'),('prefix','['),('contains',"{'z': 0, 'a': False}"),('gt',2**53),('in',['x'])]:
 add(op+'-'+str(value),{**base,'node_query':{'filters':[{'field':'attributes.value','op':op,'value':value}]}})
for enabled in (True,False):
 add('selector-'+str(enabled),{**base,'composition':{},'node_query':{'enabled':enabled},'relation_query':{'enabled':True},'traversal':{'depth':2}})
add('compact',{**base,'detail':'compact','composition':{'group_by':['attributes.value']}})
add('float-depth',{**base,'traversal':{'depth':1.9}})
add('bad-string-depth',{**base,'traversal':{'depth':'1.0'}})
add('bad-float-pagination',{**base,'pagination':{'nodes':1.0}})
for quantifier in ('exists','not_exists'):
 for steps in (1,2,4):
  add(quantifier+str(steps),{**base,'relation_query':{'enabled':True},'path_query':[{'path_id':'cycle','quantifier':quantifier,'steps':[{'direction':'outgoing'}]*steps}]})
add('seed-alias-union',{**base,'seed':{'node_ids':['a-float','philosophy:b-int']}})
add('unicode-text',{**base,'seed':{'text_query':'ё'}})
add('human-forms-native-context',{**base,'seed':{'node_ids':['forms','float-form-version','float-record-version']}})
add('property-unsafe-number',{**base,'node_query':{'filters':[{'property_id':'tos.property.native-number','op':'eq','value':2**53+1}]}})
add('property-case-sensitive',{**base,'node_query':{'filters':[{'property_id':'tos.property.native-string','op':'prefix','value':'i'}]}})
add('focus',{**base,'seed':{'focus_node_id':'a-float'},'node_query':{'enabled':False},'relation_query':{'enabled':True},'traversal':{'depth':2}})
add('last-wins',base,compact_json(base)[:-1]+',"node_query":{"filters":[{"field":"attributes.value","op":"eq","value":2,"value":9007199254740993}]} }')
paged={**base,'relation_query':{'enabled':True},'seed':{'focus_node_id':'a-float'},'composition':{'group_by':['attributes.value']},'pagination':{'nodes':5,'relations':1}}
for page_number in range(20):
 add('page-'+str(page_number),paged)
 cursor=json.loads(cases[-1]['expected'])['page']['next_cursor']
 if cursor is None:break
 paged={**paged,'pagination':{**paged['pagination'],'cursor':cursor}}
print(json.dumps({'rawGraph':compact_json(graph),'rawNodes':raw_nodes,'rawRelations':raw_relations,'metadata':{key:compact_json(value) for key,value in metadata.items()},'cases':cases}))
`);

function database(data = fixture) {
  const sqlite = new DatabaseSync(':memory:');
  sqlite.exec(`CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT,PRIMARY KEY(key,part));
    CREATE TABLE knowledge_nodes(id TEXT PRIMARY KEY,entity_id TEXT,native_id TEXT,source_graph TEXT,kind_id TEXT,type_id TEXT,title_text TEXT,search_text TEXT,json TEXT);
    CREATE TABLE knowledge_relations(id TEXT PRIMARY KEY,native_id TEXT,source_graph TEXT,from_id TEXT,to_id TEXT,predicate_id TEXT,relation_type_id TEXT,label_text TEXT,search_text TEXT,json TEXT);
    CREATE TABLE knowledge_lens_order(kind TEXT,id TEXT,sort_key TEXT,from_id TEXT,to_id TEXT,PRIMARY KEY(kind,id));
    CREATE INDEX knowledge_lens_order_sort ON knowledge_lens_order(kind,sort_key,id);
    CREATE INDEX knowledge_lens_order_from ON knowledge_lens_order(kind,from_id,sort_key,id);
    CREATE INDEX knowledge_lens_order_to ON knowledge_lens_order(kind,to_id,sort_key,id);
    CREATE INDEX knowledge_lens_order_pair ON knowledge_lens_order(kind,from_id,to_id,id);
    CREATE INDEX knowledge_nodes_native_idx ON knowledge_nodes(native_id);`);
  sqlite.exec(readFileSync(new URL('../migrations/0001-exploration.sql', import.meta.url), 'utf8'));
  for (const [key, raw] of Object.entries(data.metadata)) sqlite.prepare('INSERT INTO edge_meta VALUES (?,0,?)').run(key, raw);
  for (const kind of ['node', 'relation']) for (const raw of data[kind === 'node' ? 'rawNodes' : 'rawRelations']) {
    const n = JSON.parse(raw);
    if (kind === 'node') sqlite.prepare('INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?)').run(n.id,n.entity_id,n.native_id,n.source_graph,n.kind_id,n.type_id,'','',raw);
    else sqlite.prepare('INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?)').run(n.id,n.native_id,n.source_graph,n.from_id,n.to_id,n.predicate_id,n.relation_type_id,'','',raw);
    sqlite.prepare('INSERT INTO knowledge_lens_order VALUES (?,?,?,?,?)').run(kind,n.id,n.id.toLowerCase(),kind === 'relation' ? n.from_id : '',kind === 'relation' ? n.to_id : '');
  }
  const statements = [];
  const db = {prepare(sql) {let bindings=[]; return {
    bind(...values) {bindings=values;return this;},
    async all() {statements.push({sql,bindings}); return {results:sqlite.prepare(sql).all(...bindings),meta:{rows_read:0}};},
    async first() {statements.push({sql,bindings}); return sqlite.prepare(sql).get(...bindings) ?? null;},
  };}};
  return {db, sqlite, statements};
}
function differences(packets) {
  return python(String.raw`
def diff(a,b,path='$'):
 if type(a) is not type(b):return [path+': type '+type(a).__name__+' != '+type(b).__name__]
 if isinstance(a,dict):
  if set(a)!=set(b):return [path+': keys '+repr(set(a)^set(b))]
  found=[]
  for key in a:found.extend(diff(a[key],b[key],path+'.'+key))
  if ('.attributes' in path or '.source_record' in path) and list(a)!=list(b):found.append(path+': source member order differs')
  return found
 if isinstance(a,list):
  if len(a)!=len(b):return [path+': lengths '+str(len(a))+' != '+str(len(b))]
  return [d for i,(x,y) in enumerate(zip(a,b)) for d in diff(x,y,path+'['+str(i)+']')]
 if isinstance(a,float):return [] if repr(a)==repr(b) else [path+': '+repr(a)+' != '+repr(b)]
 return [] if a==b else [path+': '+repr(a)+' != '+repr(b)]
print(json.dumps([{'name':p['name'],'diff':diff(json.loads(p['expected']),json.loads(p['actual']))} for p in json.load(sys.stdin)]))
`, packets);
}

test('D1 native-v7 complete wire packets equal Python values, kinds, source order and fingerprints', async () => {
  const {db,sqlite,statements} = database(), packets=[];
  try {
    for (const item of fixture.cases) {
      if (item.error) {await assert.rejects(executeKnowledgeLensD1(db,parseNativeRequest(item.rawSpec)),item.name); continue;}
      const result = await executeKnowledgeLensD1(db,parseNativeRequest(item.rawSpec));
      assert.deepEqual(Object.keys(result).sort(),['packet','preview']);
      const response = nativeLensResponse(result); assert.equal(response.headers.get('content-type'),'application/json; charset=utf-8');
      packets.push({name:item.name,expected:item.expected,actual:await response.text()});
    }
    const unequal = differences(packets).filter(item => item.diff.length);
    assert.deepEqual(unequal,[]);
    assert.equal(statements.some(({sql})=>/json_extract\([^)]*\.json|lower\(|substr\(|OFFSET/i.test(sql)),false,'general semantics must not pass through lossy SQL');
    assert.ok(statements.some(({sql})=>sql.includes('knowledge_lens_order_sort')));
  } finally {sqlite.close();}
});

test('native source strict duplicates and request/cursor last-wins remain separate', () => {
  assert.throws(()=>parseNativeJson('{"x":1,"x":1.0}'),/duplicate/);
  const ref=parseNativeRequest('{"10":1,"2":2,"x":{"a":1},"x":1.0,"10":9007199254740993}');
  assert.deepEqual(nativeKeys(ref),['10','2','x']);
  assert.equal(nativeNumberInfo(nativeChild(ref,'x')).kind,'float');
  assert.equal(nativePacketJson(ref),'{"10":9007199254740993,"2":2,"x":1.0}');
  assert.throws(()=>compileNativeSpec(parseNativeRequest('{"schema_version":"tos_lens_spec_v1","lens_id":"x","pagination":{"nodes":1.0}}'),[]),/integer/);
});

test('native fingerprint preserves the v7 float64 coercion contract, not raw-number identity', async () => {
  assert.equal(await nativeDigest(parseNativeJson('1')),await nativeDigest(parseNativeJson('1.0')));
  assert.equal(await nativeDigest(parseNativeJson('9007199254740992')),await nativeDigest(parseNativeJson('9007199254740993')));
  assert.equal(await nativeDigest(parseNativeJson('-0.0')),await nativeDigest(parseNativeJson('0')));
  assert.notEqual(await nativeDigest(parseNativeJson('false')),await nativeDigest(parseNativeJson('0')));
  await assert.rejects(nativeDigest(parseNativeJson('"\\ud800"')),/surrogate/);
});

test('native D1 refuses candidate/path/row budgets and damaged order/payload metadata', async () => {
  for (const [budget,value] of [['maxCandidates',1],['maxRows',1],['maxDecodedBytes',100]]) {
    const {db,sqlite}=database();
    try {await assert.rejects(executeNativeLensD1(db,parseNativeRequest(fixture.cases[0].rawSpec),{[budget]:value}),NativeBudgetExceeded);}
    finally {sqlite.close();}
  }
  const {db,sqlite}=database();
  try {
    const path=fixture.cases.find(c=>c.name==='not_exists4');
    await assert.rejects(executeNativeLensD1(db,parseNativeRequest(path.rawSpec),{maxPathSteps:1}),/path work budget/);
    sqlite.prepare("UPDATE knowledge_lens_order SET sort_key='wrong' WHERE kind='node'").run();
    await assert.rejects(executeKnowledgeLensD1(db,parseNativeRequest(fixture.cases[0].rawSpec)),/ordered carrier differs/);
  } finally {sqlite.close();}
});

test('native response boundary explicitly permits 1-16 MiB and refuses larger aggregate packets', async () => {
  const source=parseNativeJson(JSON.stringify('ё'.repeat(400000)));
  const packet=nativePacketArray([source,source]);
  const result={packet,preview:{}};
  assert.ok(new TextEncoder().encode(await nativeLensResponse(result).text()).length > 1048576);
  assert.equal(await nativeLensResponse(result,200,'HEAD').text(),'');
  assert.throws(()=>nativeLensResponse({packet:nativePacketArray(Array(22).fill(source)),preview:{}}),NativeBudgetExceeded);
});

test('native lens/focus keep the publication-clock ABA guard across complete packet construction', async () => {
  for (const focus of [false,true]) {
    const {db,sqlite}=database();let snapshots=0;
    const intercepted={prepare(sql){const statement=db.prepare(sql);
      if (!sql.includes('knowledge_exploration_clock')) return statement;
      return {...statement,async first(){
        if (++snapshots===2) {
          sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run('{"sha256":"'+ 'b'.repeat(64)+'"}');
          sqlite.prepare("UPDATE edge_meta SET json_chunk=? WHERE key='data_revision'").run(fixture.metadata.data_revision);
        }
        return statement.first();
      }};
    }};
    try {
      await assert.rejects(focus ? focusKnowledgeNodeD1(intercepted,'a-float',{sources:['philosophy']})
        : executeKnowledgeLensD1(intercepted,parseNativeRequest(fixture.cases[0].rawSpec)),error=>error.status===409);
      assert.equal(snapshots,2);
    } finally {sqlite.close();}
  }
});

test('actual Worker HTTP receives raw numbers, strict cursors and publication failures without a lossy wire step', async () => {
  const bundle=await build({entryPoints:[fileURLToPath(new URL('../src/index.ts',import.meta.url))],bundle:true,write:false,format:'esm',platform:'browser',target:'es2022'});
  const worker=(await import('data:text/javascript;base64,'+Buffer.from(bundle.outputFiles[0].text).toString('base64'))).default;
  const {db,sqlite,statements}=database();
  const send=raw=>worker.fetch(new Request('https://test.invalid/api/knowledge/lenses/compile',{method:'POST',headers:{'Content-Type':'application/json'},body:raw}),{DB:db});
  const digest=raw=>createHash('sha256').update(raw).digest('hex');
  const metadata=(key,raw)=>sqlite.prepare('UPDATE edge_meta SET json_chunk=? WHERE key=?').run(raw,key);
  const updateLens=change=>{
    const lens=JSON.parse(fixture.metadata.knowledge_lens_top);change(lens);const raw=JSON.stringify(lens);
    metadata('knowledge_lens_top',raw);const top=JSON.parse(fixture.metadata.knowledge_reader_top);top.lens_sha256=digest(raw);metadata('knowledge_reader_top',JSON.stringify(top));
  };
  try {
    for (const name of ['eq-unsafe','last-wins','human-forms-native-context']) {
      const item=fixture.cases.find(c=>c.name===name),response=await send(item.rawSpec);
      assert.equal(response.status,200,name);assert.deepEqual(differences([{name,expected:item.expected,actual:await response.text()}])[0].diff,[]);
    }
    for (const raw of [new Uint8Array([0xff]),'\ufeff{}','{"pagination":{"nodes":1.0}}']) assert.equal((await send(raw)).status,400);
    assert.equal((await send(' '.repeat(65537))).status,413);
    const first=fixture.cases.find(c=>c.name==='page-0'),second=fixture.cases.find(c=>c.name==='page-1');
    const token=JSON.parse(Buffer.from(JSON.parse(first.expected).page.next_cursor,'base64url').toString());
    const spec=JSON.parse(second.rawSpec);
    for (const key of ['v','n','r']) {
      const rawToken=JSON.stringify(token).replace(new RegExp('"'+key+'":([0-9]+)'),'"'+key+'":$1.0');
      spec.pagination.cursor=Buffer.from(rawToken).toString('base64url');assert.equal((await send(JSON.stringify(spec))).status,400,key);
    }
    const duplicate=JSON.stringify(token).slice(0,-1)+',"n":'+token.n+'}';
    spec.pagination.cursor=Buffer.from(duplicate).toString('base64url');
    const response=await send(JSON.stringify(spec));assert.equal(response.status,200);
    const expectedDuplicate=python("from tos_access.knowledge import execute_knowledge_lens;data=json.load(sys.stdin);print(json.dumps(json.dumps(execute_knowledge_lens(json.loads(data['graph']),data['spec']),ensure_ascii=False,separators=(',',':'))))",{graph:fixture.rawGraph,spec});
    assert.deepEqual(differences([{name:'last-wins-cursor',expected:expectedDuplicate,actual:await response.text()}])[0].diff,[]);
    updateLens(lens=>{lens.unicode_version='15.0.0';});assert.equal((await send(first.rawSpec)).status,503);
    updateLens(lens=>{lens.unexpected=true;});assert.equal((await send(first.rawSpec)).status,503);
    metadata('knowledge_lens_top',fixture.metadata.knowledge_lens_top);metadata('knowledge_reader_top',fixture.metadata.knowledge_reader_top);
    const raw=fixture.rawNodes[0].slice(0,-1)+',"id":"duplicate"}';
    sqlite.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').run(raw,JSON.parse(fixture.rawNodes[0]).id);
    metadata('knowledge_node_digest:'+JSON.parse(fixture.rawNodes[0]).id,JSON.stringify({sha256:digest(raw)}));
    assert.equal((await send(first.rawSpec)).status,503,'verified bytes do not admit invalid source JSON');
    sqlite.prepare('UPDATE knowledge_nodes SET json=? WHERE id=?').run(fixture.rawNodes[0],JSON.parse(fixture.rawNodes[0]).id);
    metadata('knowledge_node_digest:'+JSON.parse(fixture.rawNodes[0]).id,fixture.metadata['knowledge_node_digest:'+JSON.parse(fixture.rawNodes[0]).id]);
    sqlite.exec('DROP INDEX knowledge_lens_order_sort');assert.equal((await send(first.rawSpec)).status,503);
    assert.ok(statements.length);
  } finally {sqlite.close();}
});

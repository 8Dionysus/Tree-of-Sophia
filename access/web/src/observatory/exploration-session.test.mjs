import {test} from 'vitest';
import assert from 'node:assert/strict';
import {KnowledgeClient,ContractError,RevisionError} from './knowledge-client.mjs';
import {ExplorationSession,bindExplorationDiscovery,explorationRequest,validateExplorationSearch} from './exploration-session.mjs';
import {R,S,C,pageFixture,secondPage} from './exploration-test-fixtures.mjs';
import {nativeStrip} from '../../../shared/native-unicode.ts';

const boundary={is_source:false,is_canon:false,writes_to_tree:false};
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
function discoveryFixture(mode='compressed'){
  const catalog={schema:'tos_knowledge_catalog_v1',source_revision:R,authority_boundary:boundary,
    predicates:[{predicate_id:'source-claims:related',display:{label:{ru:'Учебная связь'}}}],
    capabilities:{sources:['source-claims'],neighborhood_profiles:[{profile:'overview',definition:'Bounded overview.'},{profile:'all',definition:'All relation kinds.'}]}};
  const exploration={schema:'tos_exploration_capabilities_v1',available:true,writes_to_tree:false,
    request_versions:['tos_exploration_request_v2'],result_versions:['tos_exploration_result_v2'],v2_origin_kinds:['node','relation'],
    limits:{depth:10,page_nodes:100,page_relations:100}};
  const search={schema:'tos_knowledge_search_capabilities_v1',writes_to_tree:false,modes:{
    indexed:{available:mode==='indexed',schema:'tos_knowledge_search_indexed_v2'},
    compressed:{available:mode==='compressed',schema:'tos_knowledge_search_compressed_v3',source_revision:R}}};
  return {catalog,exploration,search};
}
function discovery(mode){const {catalog,exploration,search}=discoveryFixture(mode);return bindExplorationDiscovery(catalog,exploration,search);}
function harness({mode='compressed',explore,inspect}={}){
  const fixtures=discoveryFixture(mode),sent=[];let first=null;
  const client=new KnowledgeClient({fetcher:async(path,options)=>{
    const relative=path.replace('/api/knowledge',''),body=options.body?JSON.parse(options.body):null;sent.push({relative,body});
    let result;
    if(relative==='/catalog')result=fixtures.catalog;
    else if(relative==='/explore/capabilities')result=fixtures.exploration;
    else if(relative==='/search/capabilities')result=fixtures.search;
    else if(relative==='/explore'){
      if(body.cursor)result=secondPage(first);
      else {result=pageFixture(body.origin.kind);result.query=body;}
      if(explore)result=await explore(body,result);
      if(!body.cursor)first=structuredClone(result);
    }else if(relative.startsWith('/nodes/')){
      const raw=pageFixture().nodes[0];result={schema:'tos_knowledge_node_packet_v1',source_revision:R,matches:[raw]};
      if(inspect)result=await inspect(result);
    }else throw new Error('Unexpected route '+relative);
    return new Response(JSON.stringify(result),{headers:{'Content-Type':'application/json'}});
  }});
  return {session:new ExplorationSession({client}),client,sent,fixtures};
}

test('discovery binds actual catalog predicate IDs and explicit indexed/compressed engines, never legacy fallback',()=>{
  for(const mode of ['compressed','indexed']){
    const bound=discovery(mode);assert.equal(bound.searchMode,mode);assert.equal(Object.isFrozen(bound.catalog),true);
    assert.equal(explorationRequest(bound,pageFixture().query.origin,{predicate_ids:['source-claims:related']}).predicate_ids[0],'source-claims:related');
  }
  const fixture=discoveryFixture();fixture.search.modes.indexed.available=true;
  assert.equal(bindExplorationDiscovery(fixture.catalog,fixture.exploration,fixture.search).searchMode,'compressed');
  for(const mutate of [f=>f.catalog.authority_boundary={...boundary,is_source:true},f=>f.exploration.request_versions=['tos_exploration_request_v1'],
    f=>f.exploration.v2_origin_kinds=['node'],f=>f.search.modes.compressed.available=false,
    f=>f.search.modes.compressed.schema='unknown',f=>f.catalog.predicates=[{id:'not-the-producer-field'}]]){
    const f=discoveryFixture();mutate(f);assert.throws(()=>bindExplorationDiscovery(f.catalog,f.exploration,f.search),ContractError);
  }
  const changed=discoveryFixture();changed.search.modes.compressed.source_revision='f'.repeat(64);
  assert.throws(()=>bindExplorationDiscovery(changed.catalog,changed.exploration,changed.search),RevisionError);
});

test('query construction is discoverable, bounded and exact-origin without invented focus',()=>{
  const d=discovery(),origin=pageFixture('relation').query.origin;
  const query=explorationRequest(d,origin);assert.deepEqual(query.origin,origin);assert.equal(Object.hasOwn(query,'focus_node_id'),false);
  assert.equal(query.page_nodes,12);assert.equal(query.page_relations,18);
  for(const options of [{profile:'unknown'},{sources:[]},{sources:['other']},{predicate_ids:['other']},{direction:'sideways'},
    {max_depth:-1},{page_nodes:41},{page_relations:81},{max_depth:1.5},{focus_node_id:origin.id},{sources:['source-claims','source-claims']}]){
    assert.throws(()=>explorationRequest(d,origin,options),ContractError);
  }
  assert.throws(()=>explorationRequest(d,{...origin,content_revision:'old'}),ContractError);
});

test('ordinary transport opens both exact origin kinds and cursor-only continuation retains selection',async()=>{
  for(const kind of ['node','relation']){
    const {session,sent}=harness();await session.discover();
    const first=await session.open(pageFixture(kind).query.origin);
    assert.equal(first.origin.kind,kind);assert.equal(first.source_revision,R);assert.equal(first.snapshot_revision,S);
    assert.deepEqual(sent.at(-1).body.origin,pageFixture(kind).query.origin);
    if(kind==='node'){
      session.select({kind:'node',id:first.nodes[1].id});
      const next=await session.continue();assert.equal(next.nodes.length,3);assert.equal(next.selection.id,first.nodes[1].id);
      assert.deepEqual(sent.at(-1).body,{cursor:first.continuation.next_cursor});
    }
  }
});

test('URL exact ID inspection precedes exploration and stale IDs are not guessed from entity names',async()=>{
  const {session,sent}=harness();await session.discover();await session.open({kind:'node',id:'source-claims:one'});
  assert.match(sent.at(-2).relative,/^\/nodes\/source-claims%3Aone/);assert.equal(sent.at(-1).body.origin.content_revision,C);
  const last=session.snapshot();await assert.rejects(session.open({kind:'node',id:'scene:invented-identity'}),ContractError);
  assert.equal(session.snapshot(),last);assert.match(sent.at(-1).relative,/scene%3Ainvented-identity/);
});

test('late ignored-abort responses cannot replace a newer accepted scene',async()=>{
  const pending=deferred();let number=0;
  const {session}=harness({explore:async(_body,packet)=>++number===1?pending.promise:packet});await session.discover();
  const old=session.open(pageFixture().query.origin);await Promise.resolve();
  const current=await session.open(pageFixture('relation').query.origin);assert.equal(current.origin.kind,'relation');
  pending.resolve(pageFixture());assert.equal(await old,null);assert.equal(session.snapshot(),current);
});

test('failed new query preserves the prior accepted continuation including after explicit cancellation',async()=>{
  let fail=false;
  const {session}=harness({explore:async(body,packet)=>{if(fail&&!body.cursor)throw new TypeError('offline');return packet;}});
  await session.discover();const first=await session.open(pageFixture().query.origin);fail=true;
  await assert.rejects(session.open(pageFixture('relation').query.origin));assert.equal(session.snapshot(),first);
  session.cancelScene();const next=await session.continue();assert.equal(next.continuation.page,2);assert.equal(next.origin.kind,'node');
});

test('cancelling during identity inspection never starts a late exploration',async()=>{
  const pending=deferred(),{session,sent}=harness({inspect:()=>pending.promise});await session.discover();
  const job=session.open({kind:'node',id:'source-claims:one'});session.cancelScene();
  pending.resolve({schema:'tos_knowledge_node_packet_v1',source_revision:R,matches:[pageFixture().nodes[0]]});
  assert.equal(await job,null);assert.equal(sent.some(item=>item.relative==='/explore'),false);assert.equal(session.snapshot(),null);
});

function searchPacket(mode,query,cursor=null){
  const p=pageFixture();return {schema:mode==='compressed'?'tos_knowledge_search_compressed_v3':'tos_knowledge_search_indexed_v2',
    source_revision:R,query:mode==='compressed'?nativeStrip(query):query,nodes:p.nodes,relations:p.relations,authority_boundary:boundary,
    counts:{matching_nodes:null,matching_relations:null},page:{cursor,next_cursor:null,limit_per_kind:6,has_more:false}};
}
test('search preserves native mode-specific query echo and exact returned identities without inventing counts',async()=>{
  for(const mode of ['compressed','indexed'])for(const query of ['  Kant  ','\u0085Kant\u0085','\ufeffKant\ufeff']){
    const d=discovery(mode),packet=searchPacket(mode,query);
    assert.equal(validateExplorationSearch(packet,d,query),packet);
    const {session,client}=harness({mode});await session.discover();let path;
    client.request=async value=>{path=value;return packet;};
    const result=await session.search(query);assert.equal(result.counts.matching_nodes,null);
    assert.equal(new URL(path,'https://example.invalid').searchParams.get('query'),query);
    assert.equal(session.snapshot(),null);assert.equal(result.nodes[0].id,packet.nodes[0].id);
  }
  const p=searchPacket('compressed','Kant');p.source_revision='f'.repeat(64);
  assert.throws(()=>validateExplorationSearch(p,discovery(),'Kant'),RevisionError);
});

test('search cancellation suppresses stale results without discarding an accepted scene',async()=>{
  const {session,client}=harness();await session.discover();const before=await session.open(pageFixture().query.origin);
  const pending=deferred();client.request=()=>pending.promise;
  const query=session.search('Kant');session.cancelSearch();pending.resolve(searchPacket('compressed','Kant'));
  assert.equal(await query,null);assert.equal(session.snapshot(),before);
});

test('inspection passes exact raw ID and revisions; cancellation leaves scene and returns no late card',async()=>{
  const {session,client}=harness();await session.discover();const view=await session.open(pageFixture().query.origin),raw=view.nodes[0];
  const pending=deferred(),calls=[];client.readMaterial=async(...args)=>{calls.push(args);return pending.promise;};
  const card=session.inspect({kind:'node',id:raw.id},{language:'en'});session.cancelInspect();
  pending.resolve({packet:{source_revision:R,nodes:[raw],relations:[]},match:raw});assert.equal(await card,null);
  assert.deepEqual(calls[0].slice(0,2),['node',raw.id]);assert.deepEqual(calls[0].slice(3,5),[R,C]);
  assert.equal(calls[0][5].language,'en');assert.equal(session.snapshot(),view);
  await assert.rejects(session.inspect({kind:'node',id:'entity:tos.agent.fixture'}),ContractError);
});

test('dispose suppresses active scene and discovery; no subsequent open can revive the session',async()=>{
  const pending=deferred(),{session,sent}=harness({explore:()=>pending.promise});await session.discover();
  const job=session.open(pageFixture().query.origin);session.dispose();pending.resolve(pageFixture());
  assert.equal(await job,null);assert.equal(session.snapshot(),null);assert.equal(await session.discover(),null);
  const calls=sent.length;assert.equal(await session.search('Kant'),null);assert.equal(await session.inspect({kind:'node',id:'source-claims:one'}),null);assert.equal(sent.length,calls);
  assert.equal(await session.open(pageFixture().query.origin),null);
});

test('a changed local selection suppresses its previous ordinary inspection',async()=>{
  const {session,client}=harness();await session.discover();const view=await session.open(pageFixture().query.origin),raw=view.nodes[0];
  const pending=deferred();client.readMaterial=()=>pending.promise;
  const result=session.inspect({kind:'node',id:raw.id});session.select({kind:'node',id:view.nodes[1].id});
  pending.resolve({packet:{source_revision:R,nodes:[raw],relations:[]},match:raw});assert.equal(await result,null);
  assert.equal(session.snapshot().selection.id,view.nodes[1].id);
});

test('relation inspection refuses a changed endpoint even when source and relation revisions agree',async()=>{
  const {session,client}=harness();await session.discover();const view=await session.open(pageFixture('relation').query.origin),raw=view.relations[0];
  const nodes=structuredClone(view.nodes);nodes[1].content_revision='f'.repeat(64);
  client.readMaterial=async()=>({packet:{source_revision:R,nodes,relations:[raw]},match:raw,endpoints:nodes});
  await assert.rejects(session.inspect({kind:'relation',id:raw.id}),RevisionError);assert.equal(session.snapshot(),view);
});

test('search admits code-point length and native whitespace, never silently broadening whitespace to all records',async()=>{
  const {session,client}=harness();await session.discover();let calls=0;
  client.request=async path=>{calls++;const q=new URL(path,'https://example.invalid').searchParams.get('query');return searchPacket('compressed',q);};
  for(const query of ['\u0085','\u001c','x'.repeat(257),'İ'.repeat(129)])await assert.rejects(session.search(query),ContractError);
  assert.equal(calls,0);await session.search('𐐀'.repeat(256));await session.search('\ufeff');assert.equal(calls,2);
});
test('targeted card cancellation does not abort independent reading or comparison slots',async()=>{
  const {session,client}=harness();await session.discover();const view=await session.open(pageFixture().query.origin),raw=view.nodes[0];
  const pending=deferred();client.readMaterial=()=>pending.promise;
  const target={kind:'node',id:raw.id};
  const ordinary=session.inspect(target),left=session.inspect(target,{slot:'compare-left'}),right=session.inspect(target,{slot:'compare-right'});
  session.cancelInspect('compare-left');assert.throws(()=>session.cancelInspect('unknown'),ContractError);
  pending.resolve({packet:{source_revision:R,nodes:[raw],relations:[]},match:raw});
  assert.ok(await ordinary);assert.equal(await left,null);assert.ok(await right);
});

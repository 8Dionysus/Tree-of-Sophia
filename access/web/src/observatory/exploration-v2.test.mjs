import {test} from 'vitest';
import assert from 'node:assert/strict';
import {knowledgeScene} from '../../../shared/knowledge-scene.ts';
import {validateExploration,KnowledgeClient,ContractError,RevisionError} from './knowledge-client.mjs';
import {ExplorationSceneCache,SceneCapacityError} from './exploration-cache.mjs';

import {R,S,C,node,pageFixture,secondPage} from './exploration-test-fixtures.mjs';

test('v2 preserves exact node and relation origin with declared scene and endpoint revisions',()=>{
  for(const kind of ['node','relation']){
    const packet=pageFixture(kind),before=structuredClone(packet);
    assert.equal(validateExploration(packet,R),packet);assert.deepEqual(packet,before);
    assert.equal(Object.hasOwn(packet,'focus'),false);
    const next=structuredClone(packet);next.page.number=2;next.page.next_cursor='e'.repeat(64);
    next.query=Object.fromEntries(Object.entries(next.query).reverse());
    assert.equal(validateExploration(next,R,packet),next);
  }
});

test('v2 refuses missing origins, invented endpoint bindings, altered scenes and partition gaps',()=>{
  for(const change of [
    p=>p.origin.id='absent',p=>p.origin.content_revision='0'.repeat(64),
    p=>p.query.origin.id='other',p=>p.query.source_revision='0'.repeat(64),
    p=>p.origin.endpoints.from.node_id=p.origin.endpoints.to.node_id,
    p=>p.origin.endpoints.to.content_revision='0'.repeat(64),
    p=>p.origin.endpoints.to.entity_id='tos.agent.false-identity',
    p=>p.page.context_relation_ids=[],p=>p.page.primary_relation_ids=[p.origin.id],
    p=>p.page.context_node_ids=[p.nodes[0].id,p.nodes[0].id],
    p=>delete p.inclusion.nodes[p.nodes[0].id],p=>p.inclusion.relations[p.origin.id].kind='incident',
    p=>p.page.work_units=513,p=>p.page.next_cursor=null,p=>p.writes_to_tree=true,
    p=>p.focus={node_id:p.nodes[0].id},p=>p.scene.vertices[0].representative_node_id='invented',
    p=>p.scene.collapsed_relation_ids.push(p.origin.id),p=>delete p.scene,
    p=>p.execution_version='unknown-version',
  ]){const packet=pageFixture('relation');change(packet);assert.throws(()=>validateExploration(packet),ContractError);}
});

test('v2 binds continuation to source, snapshot, exact origin, execution and query',()=>{
  const first=pageFixture();
  for(const change of [p=>p.snapshot_revision='f'.repeat(64),p=>p.page.number=3,p=>p.page.next_cursor=first.page.next_cursor,
    p=>p.execution_version='tos-exploration-d1-execution-v6',p=>p.query.max_depth=3,
    p=>{p.origin.content_revision=p.query.origin.content_revision=p.nodes[0].content_revision='f'.repeat(64);},
  ]){const next=structuredClone(first);next.page.number=2;next.page.next_cursor='e'.repeat(64);change(next);assert.throws(()=>validateExploration(next,R,first),RevisionError);}
  const complete=structuredClone(first);complete.status='complete';complete.page.next_cursor=null;
  const next=structuredClone(first);next.page.number=2;
  assert.throws(()=>validateExploration(next,R,complete),RevisionError);
});

test('HTTP exploration adapter uses v2 as v2 and refuses a different start request',async()=>{
  const packet=pageFixture('relation'),sent=[];
  const client=new KnowledgeClient({fetcher:async(_url,options)=>{sent.push(JSON.parse(options.body));return {ok:true,json:async()=>structuredClone(packet)};}});
  const result=await client.explore(packet.query,undefined,R);
  assert.equal(result.schema,'tos_exploration_result_v2');assert.deepEqual(sent[0],packet.query);
  await client.explore({...packet.query,origin:Object.fromEntries(Object.entries(packet.query.origin).reverse())},undefined,R);
  await assert.rejects(client.explore({...packet.query,max_depth:1},undefined,R),ContractError);
});


test('bounded page accumulation retains exact carriers and selection instead of interpreting absence as deletion',()=>{
  const first=pageFixture(),second=secondPage(first),cache=new ExplorationSceneCache();
  const ticket=cache.begin(first.query),one=cache.accept(ticket,first);
  assert.equal(one.schema,'tos_browser_exploration_view_v1');assert.equal(one.writes_to_tree,false);
  cache.select({kind:'node',id:first.nodes[1].id});
  const two=cache.accept(ticket,second);
  assert.equal(two.nodes.length,3);assert.equal(two.relations.length,2);
  assert.deepEqual(two.selection,{kind:'node',id:first.nodes[1].id});
  assert.equal(two.contexts[0].pages,2);assert.equal(cache.accept(ticket,second),two);
  assert.deepEqual(cache.nextRequest(ticket),{cursor:second.page.next_cursor});
  assert.throws(()=>two.nodes.push(node('caller-mutation')),TypeError);
  first.nodes[0].display.title.ru='Caller mutation';assert.notEqual(two.nodes[0].display.title.ru,first.nodes[0].display.title.ru);
  assert.deepEqual(one.nodes.map(n=>n.id),['source-claims:one','source-claims:two']);
});

test('same-entity representatives compose by the producer rule across pages without duplicate vertices',()=>{
  const first=pageFixture();
  first.nodes[1]=node('source-navigation:alias',first.nodes[0].entity_id);first.nodes[1].source_graph='source-navigation';
  first.relations[0].to_id=first.nodes[1].id;first.relations[0].relation_type_id='tos.relation.projects';
  first.page.primary_node_ids=[first.nodes[1].id];
  first.inclusion.nodes={[first.nodes[0].id]:{kind:'origin'},[first.nodes[1].id]:{kind:'context-endpoint'}};
  first.scene=knowledgeScene(first.nodes,first.relations,first.origin.id);
  const second=secondPage(first);second.relations[0].relation_type_id='tos.relation.related-to';
  second.scene=knowledgeScene(second.nodes,second.relations,second.origin.id);
  const cache=new ExplorationSceneCache(),ticket=cache.begin(first.query);
  const before=cache.accept(ticket,first),after=cache.accept(ticket,second);
  assert.equal(before.scene.vertices.length,1);assert.equal(after.scene.vertices.length,2);
  for(const snapshot of [before,after]){
    const vertex=snapshot.scene.vertices.find(v=>v.entity_id===first.nodes[0].entity_id);
    assert.equal(vertex.representative_node_id,'source-navigation:alias');
    assert.equal(snapshot.selection.id,first.origin.id);
  }
});

test('stale generation, binding, conflicting bytes and incomplete pages leave the same last-good object',()=>{
  const first=pageFixture(),cache=new ExplorationSceneCache(),old=cache.begin(first.query);
  const one=cache.accept(old,first),active=cache.begin(first.query);
  assert.throws(()=>cache.accept(old,secondPage(first)),RevisionError);assert.equal(cache.snapshot(),one);
  for(const change of [p=>p.snapshot_revision='f'.repeat(64),p=>p.nodes[0].display.title.ru='conflicting bytes',
    p=>p.nodes[0].content_revision='f'.repeat(64),p=>p.relations[0].to_id='absent',p=>p.page.number=2]){
    const packet=structuredClone(first);change(packet);
    assert.throws(()=>cache.accept(active,packet));assert.equal(cache.snapshot(),one);
  }
  cache.cancel(active);assert.throws(()=>cache.accept(active,first),RevisionError);assert.equal(cache.snapshot(),one);
});

test('count, byte, context and page capacity refuse atomically, without eviction or selected-object loss',()=>{
  const first=pageFixture(),second=secondPage(first);
  for(const limits of [{nodes:2},{relations:1},{pagesPerContext:1}]){
    const cache=new ExplorationSceneCache({limits}),ticket=cache.begin(first.query),one=cache.accept(ticket,first);
    assert.throws(()=>cache.accept(ticket,second),SceneCapacityError);assert.equal(cache.snapshot(),one);
  }
  const cache=new ExplorationSceneCache({limits:{contexts:1}}),ticket=cache.begin(first.query),one=cache.accept(ticket,first);
  const next=cache.begin(first.query);assert.throws(()=>cache.accept(next,first),SceneCapacityError);assert.equal(cache.snapshot(),one);
  const tiny=new ExplorationSceneCache({limits:{bytes:1024}}),fresh=tiny.begin(first.query);
  assert.throws(()=>tiny.accept(fresh,first),SceneCapacityError);assert.equal(tiny.snapshot(),null);
  assert.throws(()=>new ExplorationSceneCache({limits:{nodes:201}}),TypeError);
});

test('explicit replacement is atomic and different same-source execution snapshots never mix',()=>{
  const first=pageFixture(),cache=new ExplorationSceneCache(),ticket=cache.begin(first.query),one=cache.accept(ticket,first);
  const next=structuredClone(first);next.source_revision=next.query.source_revision='f'.repeat(64);next.snapshot_revision='e'.repeat(64);
  assert.throws(()=>cache.begin(next.query),RevisionError);assert.equal(cache.snapshot(),one);
  const replacement=cache.begin(next.query,{replace:true});assert.equal(cache.snapshot(),one);
  const bad=structuredClone(next);bad.scene.arcs=[];
  assert.throws(()=>cache.accept(replacement,bad),ContractError);assert.equal(cache.snapshot(),one);
  const two=cache.accept(replacement,next);assert.equal(two.source_revision,next.source_revision);
  assert.equal(two.contexts.length,1);assert.notEqual(two,one);
  const continuation=secondPage(next);cache.accept(replacement,continuation);
  assert.equal(cache.snapshot().nodes.length,3);
});

test('failed new query keeps the prior cursor resumable under a new generation, never revives stale responses',()=>{
  const first=pageFixture(),cache=new ExplorationSceneCache(),a=cache.begin(first.query),one=cache.accept(a,first);
  const b=cache.begin({...first.query,max_depth:3});
  assert.throws(()=>cache.accept(b,first),RevisionError);cache.cancel(b);assert.equal(cache.snapshot(),one);
  const resumed=cache.resume();assert.deepEqual(cache.nextRequest(resumed),{cursor:first.page.next_cursor});
  assert.throws(()=>cache.accept(a,secondPage(first)),RevisionError);
  assert.throws(()=>cache.accept(b,first),RevisionError);
  const two=cache.accept(resumed,secondPage(first));
  assert.equal(two.contexts.length,1);assert.equal(two.contexts[0].pages,2);assert.equal(two.contexts[0].id,one.contexts[0].id);
});

test('server canonical set fields retain request meaning while ordered source arrays remain exact',async()=>{
  const packet=pageFixture();packet.query.sources=['source-claims','source-navigation'];packet.query.predicate_ids=['a','b'];
  const request={...packet.query,sources:['source-navigation','source-claims'],predicate_ids:['b','a','b']};
  const client=new KnowledgeClient({fetcher:async()=>({ok:true,json:async()=>structuredClone(packet)})});
  assert.equal((await client.explore(request,undefined,R)).schema,packet.schema);
  const cache=new ExplorationSceneCache(),ticket=cache.begin(request);
  assert.equal(cache.accept(ticket,packet).nodes.length,2);
  const other=cache.begin({...request,predicate_ids:['a','wrong']});
  assert.throws(()=>cache.accept(other,packet),RevisionError);
});

test('later context closure preserves a prior discovery path and each distinct bounded execution reason',()=>{
  const first=pageFixture();first.inclusion.nodes[first.nodes[1].id]={kind:'traversal',via_node_id:first.nodes[0].id,via_relation_id:first.relations[0].id,depth:1};
  const second=structuredClone(first);second.page.number=2;second.page.next_cursor='e'.repeat(64);
  second.inclusion.nodes[second.nodes[1].id]={kind:'context-endpoint'};
  const cache=new ExplorationSceneCache(),ticket=cache.begin(first.query);cache.accept(ticket,first);
  const view=cache.accept(ticket,second),reasons=view.contexts[0].inclusion.nodes[first.nodes[1].id];
  assert.deepEqual(reasons,[{page:1,reason:first.inclusion.nodes[first.nodes[1].id]},{page:2,reason:{kind:'context-endpoint'}}]);
  const third=structuredClone(second);third.page.number=3;third.page.next_cursor='f'.repeat(64);
  assert.equal(cache.accept(ticket,third).contexts[0].inclusion.nodes[first.nodes[1].id].length,2);
});

test('relation and Claim identities are not substituted by scene IDs at the selection boundary',()=>{
  const first=pageFixture('relation'),cache=new ExplorationSceneCache(),ticket=cache.begin(first.query),one=cache.accept(ticket,first);
  assert.deepEqual(one.selection,{kind:'relation',id:first.origin.id});
  assert.ok(one.scene.arcs.some(arc=>arc.relation_id===one.selection.id));
  assert.throws(()=>cache.select({kind:'node',id:one.scene.vertices[0].id}),ContractError);
  assert.throws(()=>cache.select({kind:'claim-path',id:'invented-claim-path'}),ContractError);
  assert.equal(cache.snapshot(),one);
});
